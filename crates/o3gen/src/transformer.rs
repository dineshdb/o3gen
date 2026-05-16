use crate::helpers::to_pascal_case;
use heck::ToPascalCase;
use http::{Method, StatusCode};
use indexmap::IndexMap;
use openapiv3::{
    AnySchema, OpenAPI, ParameterData, ParameterSchemaOrContent, ReferenceOr, Schema, SchemaKind,
    Type,
};
use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::ir::{
    AliasIr, AnyOfIr, ApiIr, EnumIr, EnumVariantIr, FieldIr, Name, NewtypeIr, OperationIr,
    ParameterIr, ParameterLocation, PrimitiveType, ResponseIr, StructIr, TypeDefinitionIr, TypeIr,
    ValidationIr, VariantIr,
};

#[derive(Debug)]
pub struct Transformer<'a> {
    openapi: &'a OpenAPI,
    config: &'a Config,
    types: IndexMap<String, TypeDefinitionIr>,
    operations: Vec<OperationIr>,
    taken_names: HashSet<String>,
    fingerprints: HashMap<String, String>,
}

impl<'a> Transformer<'a> {
    fn resolve_ref_name(reference: &str) -> Result<&str, String> {
        reference
            .split('/')
            .next_back()
            .ok_or_else(|| format!("Invalid reference: {reference}"))
    }

    fn merge_derives(&self, name: &str, base: &[&str]) -> Vec<String> {
        let mut derives: Vec<String> = base.iter().map(|s| (*s).to_string()).collect();
        if let Some(extra) = self.config.derive_extra.get(name) {
            for tr in extra {
                if !derives.contains(tr) {
                    derives.push(tr.clone());
                }
            }
        }
        derives
    }

    /// # Errors
    /// Returns an error if the `OpenAPI` specification cannot be transformed.
    pub fn transform(openapi: &'a OpenAPI, config: &'a Config) -> Result<ApiIr, String> {
        let mut transformer = Self {
            openapi,
            config,
            types: IndexMap::new(),
            operations: Vec::new(),
            taken_names: HashSet::new(),
            fingerprints: HashMap::new(),
        };

        transformer.process_schemas()?;
        transformer.operations = transformer.process_paths()?;

        transformer.apply_ir_transformations();

        Ok(ApiIr {
            types: transformer.types,
            operations: transformer.operations,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn apply_ir_transformations(&mut self) {
        // 1. Refine type names: strip redundant prefixes from Generated types
        let mut type_renames = HashMap::new();
        let mut current_taken_names: HashSet<String> = self.types.keys().cloned().collect();

        let mut type_names_sorted: Vec<String> = self.types.keys().cloned().collect();
        // Sort by length descending to handle nested prefixing correctly
        type_names_sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));

        for name in type_names_sorted {
            let Some(def) = self.types.get(&name) else {
                continue;
            };
            if def.is_generated() {
                // Try to find a parent prefix to strip
                let mut best_prefix: Option<String> = None;
                for potential_prefix in self.types.keys() {
                    if name.starts_with(potential_prefix) && name.len() > potential_prefix.len() {
                        match &best_prefix {
                            Some(current_best) if potential_prefix.len() > current_best.len() => {
                                best_prefix = Some(potential_prefix.clone());
                            }
                            None => {
                                best_prefix = Some(potential_prefix.clone());
                            }
                            _ => {}
                        }
                    }
                }

                if let Some(prefix) = best_prefix {
                    let candidate_name = name[prefix.len()..].to_string();

                    if !candidate_name.is_empty()
                        && candidate_name
                            .chars()
                            .next()
                            .is_some_and(char::is_alphabetic)
                        && !Self::is_generic_name(&candidate_name)
                        && !current_taken_names.contains(&candidate_name)
                        && candidate_name.len() >= 3
                    {
                        type_renames.insert(name.clone(), candidate_name.clone());
                        current_taken_names.insert(candidate_name);
                    }
                }
            }
        }

        // Apply type renames
        for (old_name, new_name) in &type_renames {
            if let Some(mut def) = self.types.shift_remove(old_name) {
                def.set_name(new_name.clone());
                self.types.insert(new_name.clone(), def);
            }
        }

        // Update all references in the IR to the new type names
        for def in self.types.values_mut() {
            def.update_references(&type_renames);
        }
        for op in &mut self.operations {
            if let Some(rb) = &mut op.request_body {
                rb.update_reference(&type_renames);
            }
            for param in &mut op.parameters {
                param.type_info.update_reference(&type_renames);
            }
            for resp in &mut op.responses {
                if let Some(ti) = &mut resp.type_info {
                    ti.update_reference(&type_renames);
                }
            }
        }

        // 2. Refine anyOf variant names to be descriptive and unique
        for def in self.types.values_mut() {
            let parent_name = def.name().to_string();

            if let TypeDefinitionIr::AnyOf(any_of) = def {
                let mut seen_names = HashSet::new();
                for variant in &mut any_of.variants {
                    let mut descriptive_name = match &variant.type_info {
                        TypeIr::Reference(r) => r.clone(),
                        TypeIr::Enum(e) => e.clone(),
                        TypeIr::Primitive(p) => match p {
                            PrimitiveType::String => "String".to_string(),
                            PrimitiveType::Integer => "Integer".to_string(),
                            PrimitiveType::Number => "Number".to_string(),
                            PrimitiveType::Boolean => "Boolean".to_string(),
                            _ => variant.name.clone(),
                        },
                        TypeIr::Array(_) => "Array".to_string(),
                        TypeIr::Map(_) => "Map".to_string(),
                        TypeIr::Value => "Value".to_string(),
                    };

                    // Heuristic: strip shared prefix with parent
                    let common =
                        Self::find_common_prefix(&[parent_name.clone(), descriptive_name.clone()])
                            .unwrap_or_default();
                    if !common.is_empty() && descriptive_name.starts_with(&common) {
                        let stripped = descriptive_name[common.len()..].to_string();
                        if !stripped.is_empty()
                            && stripped.chars().next().is_some_and(char::is_alphabetic)
                        {
                            descriptive_name = stripped;
                        }
                    }

                    if descriptive_name.is_empty()
                        || descriptive_name.chars().all(|c| !c.is_alphanumeric())
                    {
                        descriptive_name.clone_from(&variant.name);
                    }

                    let mut final_name = descriptive_name.clone();
                    let mut counter = 2;
                    while seen_names.contains(&final_name) {
                        final_name = format!("{descriptive_name}{counter}");
                        counter += 1;
                    }
                    variant.name.clone_from(&final_name);
                    seen_names.insert(final_name);
                }
            }
        }
    }

    fn process_schemas(&mut self) -> Result<(), String> {
        if let Some(components) = &self.openapi.components {
            for (name, schema_ref) in &components.schemas {
                self.resolve_and_register_type_internal(name, schema_ref, false)?;
            }
        }
        Ok(())
    }

    fn resolve_and_register_type(
        &mut self,
        name: &str,
        schema_ref: &ReferenceOr<Schema>,
    ) -> Result<TypeIr, String> {
        self.resolve_and_register_type_internal(name, schema_ref, true)
    }

    fn resolve_and_register_type_internal(
        &mut self,
        name: &str,
        schema_ref: &ReferenceOr<Schema>,
        is_generated: bool,
    ) -> Result<TypeIr, String> {
        match schema_ref {
            ReferenceOr::Reference { reference } => {
                let ref_name = Self::resolve_ref_name(reference)?;
                Ok(TypeIr::Reference(self.resolve_final_name(ref_name)))
            }
            ReferenceOr::Item(schema) => {
                let candidate = self.resolve_final_name(name);

                if is_generated {
                    // Inline schema (auto-generated name)
                    let fp = serde_json::to_string(schema).unwrap_or_default();
                    if let Some(existing_name) = self.fingerprints.get(&fp) {
                        return Ok(TypeIr::Reference(existing_name.clone()));
                    }

                    let mut final_name = candidate.clone();
                    let mut counter = 2;
                    while self.taken_names.contains(&final_name) {
                        final_name = format!("{candidate}{counter}");
                        counter += 1;
                    }

                    self.fingerprints.insert(fp, final_name.clone());
                    self.taken_names.insert(final_name.clone());

                    let def = self.schema_to_definition(
                        &final_name,
                        schema,
                        Name::Generated(final_name.clone()),
                    )?;
                    self.types.insert(final_name.clone(), def);
                    Ok(TypeIr::Reference(final_name))
                } else {
                    // Named component schema
                    self.taken_names.insert(candidate.clone());
                    let def = self.schema_to_definition(
                        &candidate,
                        schema,
                        Name::Provided(candidate.clone()),
                    )?;
                    self.types.insert(candidate.clone(), def);
                    Ok(TypeIr::Reference(candidate))
                }
            }
        }
    }

    fn resolve_final_name(&self, name: &str) -> String {
        self.config
            .rename
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn schema_to_definition(
        &mut self,
        name: &str,
        schema: &Schema,
        ir_name: Name,
    ) -> Result<TypeDefinitionIr, String> {
        let description = schema.schema_data.description.clone();
        match &schema.schema_kind {
            SchemaKind::Type(Type::Object(obj)) => {
                self.schema_object_to_definition(name, obj, description, ir_name)
            }
            SchemaKind::Type(Type::String(s)) if !s.enumeration.is_empty() => {
                Ok(self.schema_enum_to_definition(name, s, description, ir_name))
            }

            SchemaKind::Type(Type::String(_) | Type::Integer(_))
                if name.ends_with("Id") || name == "Id" =>
            {
                let target = self.schema_to_type_ir(name, "Target", schema)?;
                Ok(TypeDefinitionIr::Newtype(NewtypeIr {
                    name: ir_name,
                    target,
                    derives: self.get_newtype_derives(name),
                    description,
                }))
            }
            SchemaKind::AnyOf { any_of } => {
                self.schema_any_of_to_definition(name, any_of, description, ir_name)
            }
            SchemaKind::OneOf { one_of } => {
                self.schema_any_of_to_definition(name, one_of, description.clone(), ir_name)
            }
            SchemaKind::AllOf { all_of } => {
                self.schema_all_of_to_definition(name, all_of, description, ir_name)
            }
            SchemaKind::Type(Type::Array(arr)) => {
                let target = if let Some(items) = &arr.items {
                    self.schema_ref_boxed_to_type_ir(name, "Item", items)?
                } else {
                    TypeIr::Value
                };
                Ok(TypeDefinitionIr::Alias(AliasIr {
                    name: ir_name,
                    target: TypeIr::Array(Box::new(target)),
                    description,
                }))
            }
            SchemaKind::Any(any) if !any.properties.is_empty() => {
                self.schema_any_to_definition(name, any, description, ir_name)
            }
            _ => {
                let target = self.schema_to_type_ir(name, "Target", schema)?;
                Ok(TypeDefinitionIr::Alias(AliasIr {
                    name: ir_name,
                    target,
                    description,
                }))
            }
        }
    }

    fn schema_any_to_definition(
        &mut self,
        name: &str,
        any: &AnySchema,
        description: Option<String>,
        ir_name: Name,
    ) -> Result<TypeDefinitionIr, String> {
        let mut fields = Vec::new();
        for (prop_name, prop_ref) in &any.properties {
            let field_type = self.schema_ref_boxed_to_type_ir(name, prop_name, prop_ref)?;
            let required = any.required.contains(prop_name) && !Self::is_nullable_ref(prop_ref);

            fields.push(FieldIr::new(
                prop_name,
                field_type,
                required,
                Self::extract_validation_from_boxed_ref(prop_ref),
                Self::extract_description_from_boxed_ref(prop_ref),
            ));
        }
        Ok(TypeDefinitionIr::Struct(StructIr {
            name: ir_name,
            fields,
            derives: self.get_struct_derives(name),
            description,
        }))
    }

    fn schema_object_to_definition(
        &mut self,
        name: &str,
        obj: &openapiv3::ObjectType,
        description: Option<String>,
        ir_name: Name,
    ) -> Result<TypeDefinitionIr, String> {
        let mut fields = Vec::new();
        for (prop_name, prop_ref) in &obj.properties {
            let field_type = self.schema_ref_boxed_to_type_ir(name, prop_name, prop_ref)?;
            let required = obj.required.contains(prop_name) && !Self::is_nullable_ref(prop_ref);

            fields.push(FieldIr::new(
                prop_name,
                field_type,
                required,
                Self::extract_validation_from_boxed_ref(prop_ref),
                Self::extract_description_from_boxed_ref(prop_ref),
            ));
        }
        Ok(TypeDefinitionIr::Struct(StructIr {
            name: ir_name,
            fields,
            derives: self.get_struct_derives(name),
            description,
        }))
    }

    fn schema_enum_to_definition(
        &self,
        name: &str,
        s: &openapiv3::StringType,
        description: Option<String>,
        ir_name: Name,
    ) -> TypeDefinitionIr {
        let mut variants = Vec::new();
        let mut raw_values = Vec::new();
        for v in s.enumeration.iter().flatten() {
            variants.push(EnumVariantIr {
                name: v.clone(),
                rust_name: v.to_pascal_case(),
                value: v.clone(),
                description: None,
            });
            raw_values.push(v.clone());
        }

        let rename_all = Self::detect_casing(&raw_values);

        TypeDefinitionIr::Enum(EnumIr {
            name: ir_name,
            variants,
            derives: self.get_enum_derives(name),
            rename_all,
            description,
        })
    }

    fn detect_casing(values: &[String]) -> Option<String> {
        let first = values.first()?;
        if first.chars().all(|c| c.is_lowercase() || c == '_') {
            Some("snake_case".to_string())
        } else if first.chars().all(|c| c.is_lowercase() || c == '-') {
            Some("kebab-case".to_string())
        } else {
            None
        }
    }

    fn schema_any_of_to_definition(
        &mut self,
        name: &str,
        any_of: &[ReferenceOr<Schema>],
        description: Option<String>,
        ir_name: Name,
    ) -> Result<TypeDefinitionIr, String> {
        let mut variants = Vec::new();
        for (i, sub_ref) in any_of.iter().enumerate() {
            let mut variant_name = match sub_ref {
                ReferenceOr::Reference { reference } => {
                    Self::resolve_ref_name(reference)?.to_string()
                }
                ReferenceOr::Item(s) => {
                    if let SchemaKind::Type(Type::String(st)) = &s.schema_kind
                        && !st.enumeration.is_empty()
                    {
                        let values: Vec<String> =
                            st.enumeration.iter().flatten().cloned().collect();
                        Self::find_common_prefix(&values).unwrap_or_else(|| format!("Variant{i}"))
                    } else if let Some(title) = &s.schema_data.title {
                        title.clone()
                    } else {
                        format!("Variant{i}")
                    }
                }
            };

            if variant_name.is_empty() || variant_name.chars().all(|c| !c.is_alphanumeric()) {
                variant_name = format!("Variant{i}");
            }

            variants.push(VariantIr {
                name: variant_name.clone(),
                type_info: self.schema_ref_to_type_ir(name, &variant_name, sub_ref)?,
            });
        }
        Ok(TypeDefinitionIr::AnyOf(AnyOfIr {
            name: ir_name,
            variants,
            derives: self.get_any_of_derives(name),
            description,
        }))
    }

    fn schema_all_of_to_definition(
        &mut self,
        name: &str,
        all_of: &[ReferenceOr<Schema>],
        description: Option<String>,
        ir_name: Name,
    ) -> Result<TypeDefinitionIr, String> {
        let mut fields = Vec::new();
        for sub_ref in all_of {
            let resolved = match sub_ref {
                ReferenceOr::Item(s) => s.clone(),
                ReferenceOr::Reference { reference } => {
                    let ref_name = Self::resolve_ref_name(reference)?;
                    self.openapi
                        .components
                        .as_ref()
                        .and_then(|c| c.schemas.get(ref_name))
                        .and_then(|r| r.as_item())
                        .cloned()
                        .unwrap_or_else(|| Schema {
                            schema_data: openapiv3::SchemaData::default(),
                            schema_kind: SchemaKind::Type(Type::Object(
                                openapiv3::ObjectType::default(),
                            )),
                        })
                }
            };
            if let SchemaKind::Type(Type::Object(obj)) = &resolved.schema_kind {
                for (prop_name, prop_ref) in &obj.properties {
                    let field_type = self.schema_ref_boxed_to_type_ir(name, prop_name, prop_ref)?;
                    let required = obj.required.contains(prop_name);
                    fields.push(FieldIr::new(
                        prop_name,
                        field_type,
                        required,
                        Self::extract_validation_from_boxed_ref(prop_ref),
                        Self::extract_description_from_boxed_ref(prop_ref),
                    ));
                }
            }
        }
        Ok(TypeDefinitionIr::Struct(StructIr {
            name: ir_name,
            fields,
            derives: self.get_struct_derives(name),
            description,
        }))
    }

    fn is_generic_name(name: &str) -> bool {
        let n = name.to_lowercase();
        matches!(
            n.as_str(),
            "status"
                | "role"
                | "type"
                | "mode"
                | "model"
                | "input"
                | "output"
                | "request"
                | "response"
                | "items"
                | "variant"
                | "variant0"
                | "variant1"
                | "variant2"
                | "variant3"
                | "variant4"
                | "variant5"
                | "data"
                | "value"
                | "error"
                | "object"
                | "properties"
                | "body"
                | "content"
                | "query"
                | "params"
                | "results"
                | "choices"
                | "usage"
                | "finishreason"
                | "finish_reason"
        )
    }

    fn schema_ref_to_type_ir(
        &mut self,
        parent: &str,
        field: &str,
        s_ref: &ReferenceOr<Schema>,
    ) -> Result<TypeIr, String> {
        match s_ref {
            ReferenceOr::Reference { reference } => {
                let ref_name = Self::resolve_ref_name(reference)?;
                Ok(TypeIr::Reference(self.resolve_final_name(ref_name)))
            }
            ReferenceOr::Item(s) => {
                if let Some(enum_def) = self.try_register_enum(parent, field, s_ref) {
                    return Ok(TypeIr::Enum(enum_def));
                }

                if Self::is_complex_schema(s) {
                    let candidate = format!("{parent}{}", to_pascal_case(field));
                    let child_name = self.resolve_final_name(&candidate);
                    self.resolve_and_register_type(&child_name, s_ref)
                } else {
                    self.schema_to_type_ir(parent, field, s)
                }
            }
        }
    }

    fn schema_ref_boxed_to_type_ir(
        &mut self,
        parent: &str,
        field: &str,
        s_ref: &ReferenceOr<Box<Schema>>,
    ) -> Result<TypeIr, String> {
        match s_ref {
            ReferenceOr::Reference { reference } => {
                let ref_name = Self::resolve_ref_name(reference)?;
                Ok(TypeIr::Reference(self.resolve_final_name(ref_name)))
            }
            ReferenceOr::Item(s) => {
                let unboxed = ReferenceOr::Item(*s.clone());
                self.schema_ref_to_type_ir(parent, field, &unboxed)
            }
        }
    }

    fn schema_to_type_ir(
        &mut self,
        parent: &str,
        field: &str,
        s: &Schema,
    ) -> Result<TypeIr, String> {
        match &s.schema_kind {
            SchemaKind::Type(Type::String(st)) => match &st.format {
                openapiv3::VariantOrUnknownOrEmpty::Item(openapiv3::StringFormat::Date) => {
                    Ok(TypeIr::Primitive(PrimitiveType::Date))
                }
                openapiv3::VariantOrUnknownOrEmpty::Item(openapiv3::StringFormat::DateTime) => {
                    Ok(TypeIr::Primitive(PrimitiveType::DateTime))
                }
                _ => Ok(TypeIr::Primitive(PrimitiveType::String)),
            },
            SchemaKind::Type(Type::Integer(_)) => Ok(TypeIr::Primitive(PrimitiveType::Integer)),
            SchemaKind::Type(Type::Number(_)) => Ok(TypeIr::Primitive(PrimitiveType::Number)),
            SchemaKind::Type(Type::Boolean(_)) => Ok(TypeIr::Primitive(PrimitiveType::Boolean)),
            SchemaKind::Type(Type::Array(arr)) => {
                let inner = if let Some(items) = &arr.items {
                    self.schema_ref_boxed_to_type_ir(parent, field, items)?
                } else {
                    TypeIr::Value
                };
                Ok(TypeIr::Array(Box::new(inner)))
            }
            _ => Ok(TypeIr::Value),
        }
    }

    fn is_complex_schema(s: &Schema) -> bool {
        match &s.schema_kind {
            SchemaKind::Type(Type::Object(_))
            | SchemaKind::AnyOf { .. }
            | SchemaKind::AllOf { .. }
            | SchemaKind::OneOf { .. } => true,
            SchemaKind::Type(Type::String(st)) if !st.enumeration.is_empty() => true,
            _ => false,
        }
    }

    fn try_register_enum(
        &mut self,
        parent: &str,
        field: &str,
        s_ref: &ReferenceOr<Schema>,
    ) -> Option<String> {
        let s = match s_ref {
            ReferenceOr::Item(s) => s,
            ReferenceOr::Reference { .. } => return None,
        };

        if let SchemaKind::Type(Type::String(st)) = &s.schema_kind
            && !st.enumeration.is_empty()
        {
            let candidate = format!("{parent}{}", to_pascal_case(field));
            let child_name = self.resolve_final_name(&candidate);
            let def = self.schema_enum_to_definition(
                &child_name,
                st,
                s.schema_data.description.clone(),
                Name::Generated(child_name.clone()),
            );
            self.types.insert(child_name.clone(), def);
            self.taken_names.insert(child_name.clone());
            return Some(child_name);
        }
        None
    }

    fn process_paths(&mut self) -> Result<Vec<OperationIr>, String> {
        let mut operations = Vec::new();
        for (path, item) in self.openapi.paths.iter() {
            let pi = item
                .as_item()
                .ok_or_else(|| format!("Path {path} is a reference, not supported yet"))?;
            for (method, op) in Self::path_item_to_operations(pi) {
                operations.push(self.process_operation(path, method, op, pi)?);
            }
        }
        Ok(operations)
    }

    fn path_item_to_operations(pi: &openapiv3::PathItem) -> Vec<(Method, &openapiv3::Operation)> {
        let mut ops = Vec::new();
        if let Some(op) = &pi.get {
            ops.push((Method::GET, op));
        }
        if let Some(op) = &pi.post {
            ops.push((Method::POST, op));
        }
        if let Some(op) = &pi.put {
            ops.push((Method::PUT, op));
        }
        if let Some(op) = &pi.delete {
            ops.push((Method::DELETE, op));
        }
        if let Some(op) = &pi.options {
            ops.push((Method::OPTIONS, op));
        }
        if let Some(op) = &pi.head {
            ops.push((Method::HEAD, op));
        }
        if let Some(op) = &pi.patch {
            ops.push((Method::PATCH, op));
        }
        if let Some(op) = &pi.trace {
            ops.push((Method::TRACE, op));
        }
        ops
    }

    #[allow(clippy::too_many_lines)]
    fn process_operation(
        &mut self,
        path: &str,
        method: Method,
        op: &openapiv3::Operation,
        pi: &openapiv3::PathItem,
    ) -> Result<OperationIr, String> {
        let operation_id = op.operation_id.clone().unwrap_or_else(|| {
            format!("{}{}", method.as_str().to_lowercase(), to_pascal_case(path))
        });
        let pascal_id = to_pascal_case(&operation_id);

        let query_params: Vec<_> = pi
            .parameters
            .iter()
            .chain(op.parameters.iter())
            .filter_map(|p| p.as_item())
            .filter(|p| matches!(p, openapiv3::Parameter::Query { .. }))
            .collect();

        let query_struct_name = if query_params.is_empty() {
            None
        } else {
            let name = match method {
                Method::PUT | Method::PATCH => format!("{pascal_id}Patch"),
                Method::POST => format!("{pascal_id}Request"),
                _ => format!("{pascal_id}Params"),
            };
            let mut fields = Vec::new();
            for p in query_params {
                if let openapiv3::Parameter::Query { parameter_data, .. } = p {
                    let field_type = self.resolve_param_type(parameter_data, &pascal_id)?;
                    fields.push(FieldIr::new(
                        &parameter_data.name,
                        field_type,
                        parameter_data.required,
                        Self::extract_validation(parameter_data),
                        parameter_data.description.clone(),
                    ));
                }
            }
            self.types.insert(
                name.clone(),
                TypeDefinitionIr::Struct(StructIr {
                    name: Name::Generated(name.clone()),
                    fields,
                    derives: self.get_struct_derives(&name),
                    description: None,
                }),
            );
            Some(name)
        };

        let mut parameters = Vec::new();
        let mut seen_param_names = HashSet::new();
        for p_ref in pi.parameters.iter().chain(op.parameters.iter()) {
            let ReferenceOr::Item(p) = p_ref else {
                continue;
            };
            let (location, data) = match p {
                openapiv3::Parameter::Path { parameter_data, .. } => {
                    (ParameterLocation::Path, parameter_data)
                }
                openapiv3::Parameter::Header { parameter_data, .. } => {
                    (ParameterLocation::Header, parameter_data)
                }
                openapiv3::Parameter::Cookie { parameter_data, .. } => {
                    (ParameterLocation::Cookie, parameter_data)
                }
                openapiv3::Parameter::Query { .. } => continue,
            };

            if seen_param_names.contains(&data.name) {
                continue;
            }
            seen_param_names.insert(data.name.clone());

            parameters.push(ParameterIr {
                name: data.name.clone(),
                location,
                required: data.required,
                type_info: self.resolve_param_type(data, &pascal_id)?,
                description: data.description.clone(),
            });
        }

        let request_body = if let Some(rb_ref) = &op.request_body {
            if let Some(rb) = rb_ref.as_item() {
                if let Some(content) = rb.content.get("application/json") {
                    if let Some(schema) = &content.schema {
                        Some(self.schema_ref_to_type_ir(&pascal_id, "Body", schema)?)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            query_struct_name.map(TypeIr::Reference)
        };

        let mut responses = Vec::new();
        for (code_val, resp_ref) in &op.responses.responses {
            let code = match code_val {
                openapiv3::StatusCode::Code(c) => {
                    StatusCode::from_u16(*c).map_err(|e| e.to_string())?
                }
                openapiv3::StatusCode::Range(_) => StatusCode::OK,
            };

            let type_info = if let Some(resp) = resp_ref.as_item() {
                if let Some(content) = resp.content.get("application/json") {
                    if let Some(schema) = &content.schema {
                        Some(self.schema_ref_to_type_ir(&pascal_id, "Response", schema)?)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            responses.push(ResponseIr { code, type_info });
        }

        Ok(OperationIr {
            operation_id,
            method,
            path: path.to_string(),
            parameters,
            request_body,
            responses,
            description: op.description.clone(),
        })
    }

    fn resolve_param_type(&mut self, data: &ParameterData, parent: &str) -> Result<TypeIr, String> {
        match &data.format {
            ParameterSchemaOrContent::Schema(s_ref) => {
                self.schema_ref_to_type_ir(parent, &data.name, s_ref)
            }
            ParameterSchemaOrContent::Content(_) => {
                Err("Parameter content not supported".to_string())
            }
        }
    }

    fn extract_validation(data: &ParameterData) -> Vec<ValidationIr> {
        match &data.format {
            ParameterSchemaOrContent::Schema(s_ref) => Self::extract_validation_from_ref(s_ref),
            ParameterSchemaOrContent::Content(_) => Vec::new(),
        }
    }

    fn extract_validation_from_ref(s_ref: &ReferenceOr<Schema>) -> Vec<ValidationIr> {
        let mut v = Vec::new();
        if let ReferenceOr::Item(s) = s_ref {
            match &s.schema_kind {
                SchemaKind::Type(Type::String(st))
                    if st.min_length.is_some() || st.max_length.is_some() =>
                {
                    v.push(ValidationIr::Length {
                        min: st.min_length.map(|l| l as u64),
                        max: st.max_length.map(|l| l as u64),
                    });
                }
                SchemaKind::Type(Type::Integer(st))
                    if st.minimum.is_some() || st.maximum.is_some() =>
                {
                    v.push(ValidationIr::IntRange {
                        min: st.minimum,
                        max: st.maximum,
                    });
                }
                SchemaKind::Type(Type::Number(st))
                    if st.minimum.is_some() || st.maximum.is_some() =>
                {
                    v.push(ValidationIr::FloatRange {
                        min: st.minimum,
                        max: st.maximum,
                    });
                }
                _ => {}
            }
        }
        v
    }

    fn extract_validation_from_boxed_ref(s_ref: &ReferenceOr<Box<Schema>>) -> Vec<ValidationIr> {
        match s_ref {
            ReferenceOr::Item(s) => {
                Self::extract_validation_from_ref(&ReferenceOr::Item(*s.clone()))
            }
            ReferenceOr::Reference { reference } => {
                Self::extract_validation_from_ref(&ReferenceOr::Reference {
                    reference: reference.clone(),
                })
            }
        }
    }

    fn extract_description_from_boxed_ref(s_ref: &ReferenceOr<Box<Schema>>) -> Option<String> {
        match s_ref {
            ReferenceOr::Item(s) => s.schema_data.description.clone(),
            ReferenceOr::Reference { .. } => None,
        }
    }

    fn is_nullable_ref(s_ref: &ReferenceOr<Box<Schema>>) -> bool {
        if let ReferenceOr::Item(s) = s_ref {
            s.schema_data.nullable
        } else {
            false
        }
    }

    fn get_struct_derives(&self, name: &str) -> Vec<String> {
        self.merge_derives(
            name,
            &[
                "Debug",
                "Clone",
                "Serialize",
                "Deserialize",
                "PartialEq",
                "Default",
                "Validate",
                "Builder",
            ],
        )
    }

    fn get_enum_derives(&self, name: &str) -> Vec<String> {
        self.merge_derives(
            name,
            &[
                "Debug",
                "Clone",
                "Serialize",
                "Deserialize",
                "PartialEq",
                "Default",
                "strum::Display",
                "strum::EnumString",
                "strum::EnumIter",
            ],
        )
    }

    fn get_newtype_derives(&self, name: &str) -> Vec<String> {
        self.merge_derives(
            name,
            &[
                "Debug",
                "Clone",
                "Serialize",
                "Deserialize",
                "PartialEq",
                "derive_more::Display",
                "derive_more::From",
            ],
        )
    }

    fn get_any_of_derives(&self, name: &str) -> Vec<String> {
        self.merge_derives(
            name,
            &[
                "Debug",
                "Clone",
                "Serialize",
                "Deserialize",
                "PartialEq",
                "Default",
                "derive_more::From",
            ],
        )
    }

    fn find_common_prefix(values: &[String]) -> Option<String> {
        let mut prefix = values.first()?.clone();
        for v in values.get(1..).unwrap_or_default() {
            while !v.starts_with(&prefix) && !prefix.is_empty() {
                prefix.pop();
            }
        }

        while prefix.ends_with('-') || prefix.ends_with('_') || prefix.ends_with('.') {
            prefix.pop();
        }

        if prefix.len() > 3 { Some(prefix) } else { None }
    }
}
