use heck::{ToPascalCase, ToSnakeCase};
use http::{Method, StatusCode};
use indexmap::IndexMap;
use openapiv3::{
    OpenAPI, ParameterData, ParameterSchemaOrContent, ReferenceOr, Schema, SchemaKind, Type,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use crate::config::Config;
use crate::helpers::to_pascal_case;
use crate::ir::{
    AliasIr, AnyOfIr, ApiIr, EnumIr, EnumVariantIr, FieldIr, OperationIr, ParameterIr,
    ParameterLocation, PrimitiveType, ResponseIr, StructIr, TypeDefinitionIr, TypeIr, ValidationIr,
};

#[derive(Debug)]
pub struct Transformer<'a> {
    openapi: &'a OpenAPI,
    config: &'a Config,
    types: IndexMap<String, TypeDefinitionIr>,
    taken_names: HashSet<String>,
    fingerprints: HashMap<String, String>,
}

impl<'a> Transformer<'a> {
    /// # Errors
    /// Returns an error if the `OpenAPI` specification cannot be transformed.
    pub fn transform(openapi: &'a OpenAPI, config: &'a Config) -> Result<ApiIr, String> {
        let mut transformer = Self {
            openapi,
            config,
            types: IndexMap::new(),
            taken_names: HashSet::new(),
            fingerprints: HashMap::new(),
        };

        transformer.process_schemas()?;
        let operations = transformer.process_paths()?;

        Ok(ApiIr {
            types: transformer.types,
            operations,
        })
    }

    fn process_schemas(&mut self) -> Result<(), String> {
        if let Some(components) = &self.openapi.components {
            for (name, schema_ref) in &components.schemas {
                self.resolve_and_register_type(name, schema_ref)?;
            }
        }
        Ok(())
    }

    fn resolve_and_register_type(
        &mut self,
        name: &str,
        schema_ref: &ReferenceOr<Schema>,
    ) -> Result<TypeIr, String> {
        match schema_ref {
            ReferenceOr::Reference { reference } => {
                let ref_name = reference
                    .split('/')
                    .next_back()
                    .ok_or_else(|| format!("Invalid reference: {reference}"))?;
                Ok(TypeIr::Reference(self.resolve_final_name(ref_name)))
            }
            ReferenceOr::Item(schema) => {
                let fp = serde_json::to_string(schema).unwrap_or_default();
                if let Some(existing_name) = self.fingerprints.get(&fp) {
                    return Ok(TypeIr::Reference(existing_name.clone()));
                }

                let candidate = self.resolve_final_name(name);
                let mut final_name = candidate.clone();
                let mut counter = 2;
                while self.taken_names.contains(&final_name) {
                    final_name = format!("{candidate}{counter}");
                    counter += 1;
                }

                self.fingerprints.insert(fp, final_name.clone());
                self.taken_names.insert(final_name.clone());

                let def = self.schema_to_definition(&final_name, schema)?;
                self.types.insert(final_name.clone(), def);
                Ok(TypeIr::Reference(final_name))
            }
        }
    }

    fn schema_to_definition(
        &mut self,
        name: &str,
        schema: &Schema,
    ) -> Result<TypeDefinitionIr, String> {
        match &schema.schema_kind {
            SchemaKind::Type(Type::Object(obj)) => self.schema_object_to_definition(name, obj),
            SchemaKind::Type(Type::String(s)) if !s.enumeration.is_empty() => {
                Ok(Self::schema_enum_to_definition(name, s))
            }
            SchemaKind::AnyOf { any_of } => self.schema_any_of_to_definition(name, any_of),
            SchemaKind::AllOf { all_of } => self.schema_all_of_to_definition(name, all_of),
            SchemaKind::Type(Type::Array(arr)) => {
                let target = if let Some(items) = &arr.items {
                    self.schema_ref_boxed_to_type_ir(name, "Item", items)?
                } else {
                    TypeIr::Value
                };
                Ok(TypeDefinitionIr::Alias(AliasIr {
                    name: name.to_string(),
                    target: TypeIr::Array(Box::new(target)),
                }))
            }
            _ => {
                let target = self.schema_to_type_ir(name, "Target", schema)?;
                Ok(TypeDefinitionIr::Alias(AliasIr {
                    name: name.to_string(),
                    target,
                }))
            }
        }
    }

    fn schema_object_to_definition(
        &mut self,
        name: &str,
        obj: &openapiv3::ObjectType,
    ) -> Result<TypeDefinitionIr, String> {
        let mut fields = Vec::new();
        for (prop_name, prop_ref) in &obj.properties {
            let rust_name = prop_name.to_snake_case();
            let field_type = self.schema_ref_boxed_to_type_ir(name, prop_name, prop_ref)?;
            let required = obj.required.contains(prop_name);

            fields.push(FieldIr {
                name: prop_name.clone(),
                rust_name,
                type_info: field_type,
                required,
                validation: Self::extract_validation_from_ref(prop_ref),
                serde_rename: if prop_name == &prop_name.to_snake_case() {
                    None
                } else {
                    Some(prop_name.clone())
                },
            });
        }
        Ok(TypeDefinitionIr::Struct(StructIr {
            name: name.to_string(),
            fields,
            derives: self.get_struct_derives(name),
        }))
    }

    fn schema_enum_to_definition(name: &str, s: &openapiv3::StringType) -> TypeDefinitionIr {
        let mut variants = Vec::new();
        for v in s.enumeration.iter().flatten() {
            variants.push(EnumVariantIr {
                name: v.clone(),
                rust_name: v.to_pascal_case(),
                value: v.clone(),
            });
        }
        TypeDefinitionIr::Enum(EnumIr {
            name: name.to_string(),
            variants,
            derives: vec![
                "Debug".to_string(),
                "Clone".to_string(),
                "Serialize".to_string(),
                "Deserialize".to_string(),
                "PartialEq".to_string(),
                "Default".to_string(),
            ],
        })
    }

    fn schema_any_of_to_definition(
        &mut self,
        name: &str,
        any_of: &[ReferenceOr<Schema>],
    ) -> Result<TypeDefinitionIr, String> {
        let mut variants = Vec::new();
        for (i, sub_ref) in any_of.iter().enumerate() {
            let variant_name = match sub_ref {
                ReferenceOr::Reference { reference } => reference
                    .split('/')
                    .next_back()
                    .ok_or("Invalid reference")?
                    .to_string(),
                ReferenceOr::Item(_) => format!("Variant{i}"),
            };
            variants.push(self.schema_ref_to_type_ir(name, &variant_name, sub_ref)?);
        }
        Ok(TypeDefinitionIr::AnyOf(AnyOfIr {
            name: name.to_string(),
            variants,
            derives: self.get_any_of_derives(name),
        }))
    }

    fn schema_all_of_to_definition(
        &mut self,
        name: &str,
        all_of: &[ReferenceOr<Schema>],
    ) -> Result<TypeDefinitionIr, String> {
        let mut fields = Vec::new();
        for sub_ref in all_of {
            let resolved = match sub_ref {
                ReferenceOr::Item(s) => s.clone(),
                ReferenceOr::Reference { reference } => {
                    let ref_name = reference
                        .split('/')
                        .next_back()
                        .ok_or("Invalid reference")?;
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
                    let rust_name = prop_name.to_snake_case();
                    let field_type = self.schema_ref_boxed_to_type_ir(name, prop_name, prop_ref)?;
                    let required = obj.required.contains(prop_name);
                    fields.push(FieldIr {
                        name: prop_name.clone(),
                        rust_name,
                        type_info: field_type,
                        required,
                        validation: Self::extract_validation_from_ref(prop_ref),
                        serde_rename: if prop_name == &prop_name.to_snake_case() {
                            None
                        } else {
                            Some(prop_name.clone())
                        },
                    });
                }
            }
        }
        Ok(TypeDefinitionIr::Struct(StructIr {
            name: name.to_string(),
            fields,
            derives: self.get_struct_derives(name),
        }))
    }

    fn schema_ref_to_type_ir(
        &mut self,
        parent: &str,
        field: &str,
        s_ref: &ReferenceOr<Schema>,
    ) -> Result<TypeIr, String> {
        match s_ref {
            ReferenceOr::Reference { reference } => {
                let ref_name = reference
                    .split('/')
                    .next_back()
                    .ok_or("Invalid reference")?;
                Ok(TypeIr::Reference(self.resolve_final_name(ref_name)))
            }
            ReferenceOr::Item(s) => {
                if Self::is_complex_schema(s) {
                    let child_name = format!("{parent}{}", to_pascal_case(field));
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
                let ref_name = reference
                    .split('/')
                    .next_back()
                    .ok_or("Invalid reference")?;
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

    fn process_paths(&mut self) -> Result<Vec<OperationIr>, String> {
        let mut operations = Vec::new();
        for (path, p) in self.openapi.paths.iter() {
            let pi = match p {
                ReferenceOr::Item(item) => item,
                ReferenceOr::Reference { .. } => continue,
            };

            let methods = [
                (Method::GET, &pi.get),
                (Method::POST, &pi.post),
                (Method::PUT, &pi.put),
                (Method::DELETE, &pi.delete),
                (Method::PATCH, &pi.patch),
            ];

            for (method, op_opt) in methods {
                if let Some(op) = op_opt {
                    operations.push(self.process_operation(path, pi, method, op)?);
                }
            }
        }
        Ok(operations)
    }

    fn process_operation(
        &mut self,
        path: &str,
        pi: &openapiv3::PathItem,
        method: Method,
        op: &openapiv3::Operation,
    ) -> Result<OperationIr, String> {
        let op_id = op.operation_id.as_deref().unwrap_or(path).to_string();
        let pascal_id = to_pascal_case(&op_id);

        // 1. Extract Query Params into a Struct
        let query_params: Vec<_> = pi
            .parameters
            .iter()
            .chain(op.parameters.iter())
            .filter_map(|p_ref| p_ref.as_item())
            .filter(|p| matches!(p, openapiv3::Parameter::Query { .. }))
            .collect();

        let query_struct_name = if query_params.is_empty() {
            None
        } else {
            let name = match method {
                Method::GET => format!("{pascal_id}Query"),
                Method::PUT | Method::PATCH => format!("{pascal_id}Patch"),
                Method::POST => format!("{pascal_id}Request"),
                _ => format!("{pascal_id}Params"),
            };
            let mut fields = Vec::new();
            for p in query_params {
                if let openapiv3::Parameter::Query { parameter_data, .. } = p {
                    let field_type = self.resolve_param_type(parameter_data, &pascal_id)?;
                    fields.push(FieldIr {
                        name: parameter_data.name.clone(),
                        rust_name: parameter_data.name.to_snake_case(),
                        type_info: field_type,
                        required: parameter_data.required,
                        validation: Self::extract_validation(parameter_data),
                        serde_rename: if parameter_data.name == parameter_data.name.to_snake_case()
                        {
                            None
                        } else {
                            Some(parameter_data.name.clone())
                        },
                    });
                }
            }
            self.types.insert(
                name.clone(),
                TypeDefinitionIr::Struct(StructIr {
                    name: name.clone(),
                    fields,
                    derives: self.get_struct_derives(&name),
                }),
            );
            Some(name)
        };

        // 2. Extract Path/Header Params
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
            let rust_name = data.name.to_snake_case();
            if seen_param_names.contains(&rust_name) {
                continue;
            }
            seen_param_names.insert(rust_name);
            parameters.push(ParameterIr {
                name: data.name.clone(),
                location,
                required: data.required,
                type_info: self.resolve_param_type(data, &pascal_id)?,
            });
        }

        if let Some(q_name) = query_struct_name {
            parameters.push(ParameterIr {
                name: "query".to_string(),
                location: ParameterLocation::Query,
                required: true,
                type_info: TypeIr::Reference(q_name),
            });
        }

        let request_body = self.extract_request_body(op, &pascal_id)?;
        let responses = self.extract_responses(op, &pascal_id)?;

        Ok(OperationIr {
            operation_id: op_id,
            method,
            path: path.to_string(),
            parameters,
            request_body,
            responses,
        })
    }

    fn extract_request_body(
        &mut self,
        op: &openapiv3::Operation,
        pascal_id: &str,
    ) -> Result<Option<TypeIr>, String> {
        if let Some(rb_ref) = &op.request_body {
            match rb_ref {
                ReferenceOr::Item(rb) => {
                    if let Some(mt) = rb.content.get("application/json") {
                        if let Some(s_ref) = &mt.schema {
                            Ok(Some(self.schema_ref_to_type_ir(pascal_id, "Body", s_ref)?))
                        } else {
                            Ok(None)
                        }
                    } else {
                        Ok(None)
                    }
                }
                ReferenceOr::Reference { reference } => Ok(Some(TypeIr::Reference(
                    self.resolve_final_name(
                        reference
                            .split('/')
                            .next_back()
                            .ok_or("Invalid reference")?,
                    ),
                ))),
            }
        } else {
            Ok(None)
        }
    }

    fn extract_responses(
        &mut self,
        op: &openapiv3::Operation,
        pascal_id: &str,
    ) -> Result<Vec<ResponseIr>, String> {
        let mut responses = Vec::new();
        for (code_val, resp_ref) in &op.responses.responses {
            let code_str = code_val.to_string();
            let Ok(code) = StatusCode::from_str(&code_str) else {
                continue;
            };
            let type_info = match resp_ref {
                ReferenceOr::Item(r) => {
                    if let Some(mt) = r.content.get("application/json") {
                        if let Some(s_ref) = &mt.schema {
                            Some(self.schema_ref_to_type_ir(pascal_id, "Response", s_ref)?)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                ReferenceOr::Reference { reference } => Some(TypeIr::Reference(
                    self.resolve_final_name(
                        reference
                            .split('/')
                            .next_back()
                            .ok_or("Invalid reference")?,
                    ),
                )),
            };
            responses.push(ResponseIr { code, type_info });
        }
        Ok(responses)
    }

    fn resolve_param_type(
        &mut self,
        data: &ParameterData,
        pascal_id: &str,
    ) -> Result<TypeIr, String> {
        match &data.format {
            ParameterSchemaOrContent::Schema(s_ref) => {
                self.schema_ref_to_type_ir(pascal_id, &data.name, s_ref)
            }
            ParameterSchemaOrContent::Content(_) => Ok(TypeIr::Primitive(PrimitiveType::String)),
        }
    }

    fn resolve_final_name(&self, name: &str) -> String {
        self.config
            .rename
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn get_struct_derives(&self, name: &str) -> Vec<String> {
        let mut derives = vec![
            "Debug".to_string(),
            "Clone".to_string(),
            "Serialize".to_string(),
            "Deserialize".to_string(),
            "PartialEq".to_string(),
            "Default".to_string(),
            "Validate".to_string(),
            "Builder".to_string(),
        ];
        if let Some(extra) = self.config.derive_extra.get(name) {
            for tr in extra {
                if !derives.contains(tr) {
                    derives.push(tr.clone());
                }
            }
        }
        derives
    }

    fn get_any_of_derives(&self, name: &str) -> Vec<String> {
        let mut derives = vec![
            "Debug".to_string(),
            "Clone".to_string(),
            "Serialize".to_string(),
            "Deserialize".to_string(),
            "PartialEq".to_string(),
        ];
        if let Some(extra) = self.config.derive_extra.get(name) {
            for tr in extra {
                if !derives.contains(tr) {
                    derives.push(tr.clone());
                }
            }
        }
        derives
    }

    fn extract_validation_from_ref(s_ref: &ReferenceOr<Box<Schema>>) -> Vec<ValidationIr> {
        if let ReferenceOr::Item(s) = s_ref {
            Self::extract_validation_from_schema(s)
        } else {
            Vec::new()
        }
    }

    fn extract_validation(data: &ParameterData) -> Vec<ValidationIr> {
        match &data.format {
            ParameterSchemaOrContent::Schema(ReferenceOr::Item(s)) => {
                Self::extract_validation_from_schema(s)
            }
            _ => Vec::new(),
        }
    }

    fn extract_validation_from_schema(schema: &Schema) -> Vec<ValidationIr> {
        let mut v = Vec::new();
        if let SchemaKind::Type(Type::String(s)) = &schema.schema_kind {
            if s.min_length.is_some() || s.max_length.is_some() {
                v.push(ValidationIr::Length {
                    min: s.min_length.map(|m| m as u64),
                    max: s.max_length.map(|m| m as u64),
                });
            }
            if let Some(pat) = &s.pattern {
                v.push(ValidationIr::Regex(pat.clone()));
            }
        }
        if let SchemaKind::Type(Type::Integer(i)) = &schema.schema_kind
            && (i.minimum.is_some() || i.maximum.is_some())
        {
            v.push(ValidationIr::IntRange {
                min: i.minimum,
                max: i.maximum,
            });
        }
        if let SchemaKind::Type(Type::Number(n)) = &schema.schema_kind
            && (n.minimum.is_some() || n.maximum.is_some())
        {
            v.push(ValidationIr::FloatRange {
                min: n.minimum,
                max: n.maximum,
            });
        }
        v
    }
}
