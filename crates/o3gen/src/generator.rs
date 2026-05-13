use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{File, Ident, parse2};

/// Configuration for the generator.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Path to the `OpenAPI` specification file.
    pub path: String,
    /// Mapping for renaming types.
    #[serde(default)]
    pub rename: HashMap<String, String>,
    /// Extra traits to derive for specific types.
    #[serde(default)]
    pub derive_extra: HashMap<String, Vec<String>>,
}

/// The core generator for Rust types.
#[derive(Debug)]
pub struct Generator {
    config: Config,
    schemas: HashMap<String, ReferenceOr<Schema>>,
}

impl Generator {
    /// Creates a new generator with the given configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            schemas: HashMap::new(),
        }
    }

    /// Creates a new generator builder for the given spec path.
    #[must_use]
    pub fn builder(path: impl Into<String>) -> Self {
        Self {
            config: Config {
                path: path.into(),
                ..Config::default()
            },
            schemas: HashMap::new(),
        }
    }

    /// Adds a renaming rule.
    #[must_use]
    pub fn rename(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.config.rename.insert(from.into(), to.into());
        self
    }

    /// Adds extra derives for a type.
    #[must_use]
    pub fn derive_extra(
        mut self,
        type_name: impl Into<String>,
        traits: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.config.derive_extra.insert(
            type_name.into(),
            traits.into_iter().map(Into::into).collect(),
        );
        self
    }

    /// Generates the Rust code as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the `OpenAPI` file cannot be read, parsed, or if code generation fails.
    ///
    /// # Panics
    ///
    /// This function should not panic under normal circumstances.
    pub fn generate(&mut self) -> Result<String, String> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let full_path = Path::new(&manifest_dir).join(&self.config.path);

        let spec_str = fs::read_to_string(&full_path).map_err(|e| {
            format!(
                "Failed to read OpenAPI file at {}: {e}",
                full_path.display()
            )
        })?;

        let openapi: OpenAPI = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Failed to parse OpenAPI JSON: {e}"))?;

        self.schemas.clear();
        if let Some(components) = &openapi.components {
            for (name, schema_ref) in &components.schemas {
                self.schemas.insert(name.clone(), schema_ref.clone());
            }
        }

        let mut types_tokens = TokenStream::new();
        let mut schema_names: Vec<_> = self.schemas.keys().cloned().collect();
        schema_names.sort();

        for name in schema_names {
            let schema_ref = self
                .schemas
                .get(&name)
                .ok_or_else(|| format!("Schema {name} not found"))?
                .clone();
            types_tokens.extend(self.generate_schema_tokens(&name, &schema_ref)?);
        }

        let final_tokens = quote! {
            #[allow(dead_code)]
            pub mod types {
                use serde::{Serialize, Deserialize};
                #types_tokens
            }
        };

        let file = parse2::<File>(final_tokens)
            .map_err(|e| format!("Failed to parse generated tokens: {e}"))?;

        Ok(prettyplease::unparse(&file))
    }

    /// Writes the generated code to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if generation fails or if the file cannot be written.
    pub fn write_to_file(mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let code = self.generate()?;
        let path = path.as_ref();
        fs::write(path, code).map_err(|e| format!("Failed to write to {}: {e}", path.display()))?;
        println!("cargo:rerun-if-changed={}", self.config.path);
        Ok(())
    }

    /// Writes the generated code to the cargo `OUT_DIR`.
    ///
    /// # Errors
    ///
    /// Returns an error if `OUT_DIR` is not set or if `write_to_file` fails.
    pub fn write_to_out_dir(self, filename: impl AsRef<Path>) -> Result<(), String> {
        let out_dir = std::env::var_os("OUT_DIR")
            .ok_or_else(|| "OUT_DIR environment variable is not set".to_string())?;
        let dest_path = PathBuf::from(out_dir).join(filename);
        self.write_to_file(dest_path)
    }

    fn get_derives(&self, name: &str, manual_default: bool) -> TokenStream {
        let mut derives = vec!["Debug", "Clone", "Serialize", "Deserialize", "PartialEq"];
        if !manual_default {
            derives.push("Default");
        }

        let final_name = self.config.rename.get(name).map_or(name, String::as_str);
        if let Some(extra) = self
            .config
            .derive_extra
            .get(final_name)
            .or(self.config.derive_extra.get(name))
        {
            for tr in extra {
                if !derives.contains(&tr.as_str()) {
                    derives.push(tr);
                }
            }
        }

        let idents: Vec<Ident> = derives
            .iter()
            .map(|d| Ident::new(d, Span::call_site()))
            .collect();
        quote! { #[derive(#(#idents),*)] }
    }

    fn generate_schema_tokens(
        &self,
        name: &str,
        schema_ref: &ReferenceOr<Schema>,
    ) -> Result<TokenStream, String> {
        let schema = match schema_ref {
            ReferenceOr::Reference { .. } => return Ok(TokenStream::new()),
            ReferenceOr::Item(s) => s,
        };

        let final_name = self.config.rename.get(name).map_or(name, String::as_str);
        let ident = to_ident(final_name);

        match &schema.schema_kind {
            SchemaKind::Type(Type::Object(obj)) => self.generate_object_tokens(name, &ident, obj),
            SchemaKind::Type(Type::String(s)) => Ok(self.generate_string_tokens(name, &ident, s)),
            SchemaKind::AnyOf { any_of } => self.generate_any_of_tokens(name, &ident, any_of),
            SchemaKind::AllOf { all_of } => self.generate_all_of_tokens(name, &ident, all_of),
            _ => Ok(quote! { pub type #ident = serde_json::Value; }),
        }
    }

    fn generate_all_of_tokens(
        &self,
        name: &str,
        ident: &Ident,
        all_of: &[ReferenceOr<Schema>],
    ) -> Result<TokenStream, String> {
        let mut properties = HashMap::new();
        let mut required = Vec::new();

        for schema_ref in all_of {
            self.collect_properties(schema_ref, &mut properties, &mut required);
        }

        let mut fields = TokenStream::new();
        let mut prop_names: Vec<_> = properties.keys().collect();
        prop_names.sort();

        for prop_name in prop_names {
            let prop_ref = properties
                .get(prop_name)
                .ok_or_else(|| format!("Property {prop_name} not found"))?;
            let prop_ident = to_ident(prop_name);
            let mut prop_type = get_rust_type_tokens_boxed(prop_ref, &self.config.rename);
            if !required.contains(prop_name) {
                prop_type = quote! { Option<#prop_type> };
            }
            fields.extend(quote! { pub #prop_ident: #prop_type, });
        }

        let derives = self.get_derives(name, false);
        Ok(quote! {
            #derives
            #[serde(deny_unknown_fields)]
            pub struct #ident { #fields }
        })
    }

    fn collect_properties(
        &self,
        schema_ref: &ReferenceOr<Schema>,
        properties: &mut HashMap<String, ReferenceOr<Box<Schema>>>,
        required: &mut Vec<String>,
    ) {
        match schema_ref {
            ReferenceOr::Reference { reference } => {
                let name = reference.split('/').next_back().unwrap_or("Unknown");
                if let Some(resolved) = self.schemas.get(name) {
                    self.collect_properties(resolved, properties, required);
                }
            }
            ReferenceOr::Item(schema) => match &schema.schema_kind {
                SchemaKind::Type(Type::Object(obj)) => {
                    for (name, prop) in &obj.properties {
                        properties.insert(name.clone(), prop.clone());
                    }
                    for req in &obj.required {
                        if !required.contains(req) {
                            required.push(req.clone());
                        }
                    }
                }
                SchemaKind::AllOf { all_of } => {
                    for sub in all_of {
                        self.collect_properties(sub, properties, required);
                    }
                }
                _ => {}
            },
        }
    }

    fn generate_object_tokens(
        &self,
        name: &str,
        ident: &Ident,
        obj: &openapiv3::ObjectType,
    ) -> Result<TokenStream, String> {
        let mut fields = TokenStream::new();
        let mut prop_names: Vec<_> = obj.properties.keys().collect();
        prop_names.sort();

        for prop_name in prop_names {
            let prop_ref = obj
                .properties
                .get(prop_name)
                .ok_or_else(|| format!("Property {prop_name} not found"))?;
            let prop_ident = to_ident(prop_name);
            let mut prop_type = get_rust_type_tokens_boxed(prop_ref, &self.config.rename);
            if !obj.required.contains(prop_name) {
                prop_type = quote! { Option<#prop_type> };
            }
            fields.extend(quote! { pub #prop_ident: #prop_type, });
        }

        let derives = self.get_derives(name, false);
        Ok(quote! {
            #derives
            #[serde(deny_unknown_fields)]
            pub struct #ident { #fields }
        })
    }

    fn generate_string_tokens(
        &self,
        name: &str,
        ident: &Ident,
        s: &openapiv3::StringType,
    ) -> TokenStream {
        if s.enumeration.is_empty() {
            let derives = self.get_derives(name, false);
            quote! {
                #derives
                #[serde(transparent)]
                pub struct #ident(pub String);
                impl #ident { pub fn as_str(&self) -> &str { &self.0 } }
                impl std::fmt::Display for #ident { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) } }
                impl From<String> for #ident { fn from(s: String) -> Self { Self(s) } }
                impl From<&str> for #ident { fn from(s: &str) -> Self { Self(s.to_string()) } }
                impl From<#ident> for String { fn from(val: #ident) -> Self { val.0 } }
            }
        } else {
            let mut variants = TokenStream::new();
            let mut try_from_matches = TokenStream::new();
            let mut as_str_matches = TokenStream::new();
            let mut sorted_enums: Vec<_> = s.enumeration.iter().flatten().collect();
            sorted_enums.sort();

            for (i, v) in sorted_enums.iter().enumerate() {
                let variant_ident = to_ident(&to_pascal_case(v));
                let default_attr = if i == 0 {
                    quote! { #[default] }
                } else {
                    TokenStream::new()
                };
                variants.extend(quote! { #default_attr #[serde(rename = #v)] #variant_ident, });
                try_from_matches.extend(quote! { #v => Ok(Self::#variant_ident), });
                as_str_matches.extend(quote! { Self::#variant_ident => #v, });
            }

            let derives = self.get_derives(name, false);
            quote! {
                #derives
                #[serde(rename_all = "snake_case")]
                pub enum #ident { #variants }
                impl #ident { pub fn as_str(&self) -> &'static str { match self { #as_str_matches } } }
                impl std::fmt::Display for #ident { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) } }
                impl TryFrom<&str> for #ident { type Error = String; fn try_from(s: &str) -> Result<Self, Self::Error> { match s { #try_from_matches _ => Err(format!("Unknown variant: {s}")), } } }
                impl TryFrom<&String> for #ident { type Error = String; fn try_from(s: &String) -> Result<Self, Self::Error> { Self::try_from(s.as_str()) } }
                impl From<#ident> for String { fn from(val: #ident) -> Self { val.to_string() } }
            }
        }
    }

    fn generate_any_of_tokens(
        &self,
        name: &str,
        ident: &Ident,
        any_of: &[ReferenceOr<Schema>],
    ) -> Result<TokenStream, String> {
        if any_of.is_empty() {
            return Ok(quote! { pub type #ident = serde_json::Value; });
        }
        let mut variants = TokenStream::new();
        let mut extra_types = TokenStream::new();
        let mut from_impls = TokenStream::new();

        for (i, sub_schema_ref) in any_of.iter().enumerate() {
            let (variant_name, rust_type) = match sub_schema_ref {
                ReferenceOr::Reference { reference } => {
                    let ref_name = reference.split('/').next_back().unwrap_or("Unknown");
                    let final_ref_name = self
                        .config
                        .rename
                        .get(ref_name)
                        .map_or(ref_name, String::as_str);
                    (
                        final_ref_name.to_string(),
                        to_ident(final_ref_name).to_token_stream(),
                    )
                }
                ReferenceOr::Item(sub_schema) => {
                    let is_enum = match &sub_schema.schema_kind {
                        SchemaKind::Type(Type::String(s)) => !s.enumeration.is_empty(),
                        _ => false,
                    };
                    let suffix = if is_enum { "Enum" } else { "Subtype" };
                    let sub_type_name_orig = format!("{}{}{}", name, suffix, i + 1);
                    let final_sub_type_name = self
                        .config
                        .rename
                        .get(&sub_type_name_orig)
                        .map_or(sub_type_name_orig.as_str(), String::as_str);
                    extra_types
                        .extend(self.generate_schema_tokens(&sub_type_name_orig, sub_schema_ref)?);
                    (
                        final_sub_type_name.to_string(),
                        to_ident(final_sub_type_name).to_token_stream(),
                    )
                }
            };
            let variant_ident = to_ident(&variant_name);
            variants.extend(quote! { #variant_ident(#rust_type), });
            from_impls.extend(quote! { impl From<#rust_type> for #ident { fn from(v: #rust_type) -> Self { Self::#variant_ident(v) } } });
        }

        let Some(first_sub_schema) = any_of.first() else {
            return Ok(quote! { pub type #ident = serde_json::Value; });
        };
        let (first_variant_ident, first_rust_type, is_string_like) =
            self.get_first_variant_info(name, first_sub_schema);
        let mut extra_impls = TokenStream::new();

        if is_string_like {
            extra_impls.extend(quote! {
                impl From<String> for #ident { fn from(s: String) -> Self { Self::#first_variant_ident(s.into()) } }
                impl From<&str> for #ident { fn from(s: &str) -> Self { Self::#first_variant_ident(s.into()) } }
            });
        }

        let derives = self.get_derives(name, true);
        Ok(quote! {
            #extra_types
            #derives
            #[serde(untagged)]
            pub enum #ident { #variants }
            impl Default for #ident { fn default() -> Self { Self::#first_variant_ident(#first_rust_type::default()) } }
            #from_impls
            #extra_impls
        })
    }

    fn get_first_variant_info(
        &self,
        name: &str,
        first_sub_schema: &ReferenceOr<Schema>,
    ) -> (Ident, TokenStream, bool) {
        let (first_variant_name, first_rust_type) = match first_sub_schema {
            ReferenceOr::Reference { reference } => {
                let ref_name = reference.split('/').next_back().unwrap_or("Unknown");
                let final_ref_name = self
                    .config
                    .rename
                    .get(ref_name)
                    .map_or(ref_name, String::as_str);
                (
                    final_ref_name.to_string(),
                    to_ident(final_ref_name).to_token_stream(),
                )
            }
            ReferenceOr::Item(sub_schema) => {
                let is_enum = match &sub_schema.schema_kind {
                    SchemaKind::Type(Type::String(s)) => !s.enumeration.is_empty(),
                    _ => false,
                };
                let suffix = if is_enum { "Enum" } else { "Subtype" };
                let sub_type_name_orig = format!("{}{}{}", name, suffix, 1);
                let final_sub_type_name = self
                    .config
                    .rename
                    .get(&sub_type_name_orig)
                    .map_or(sub_type_name_orig.as_str(), String::as_str);
                (
                    final_sub_type_name.to_string(),
                    to_ident(final_sub_type_name).to_token_stream(),
                )
            }
        };
        let is_string_like = match first_sub_schema {
            ReferenceOr::Reference { .. } => false,
            ReferenceOr::Item(sub_schema) => match &sub_schema.schema_kind {
                SchemaKind::Type(Type::String(s)) => s.enumeration.is_empty(),
                _ => false,
            },
        };
        (
            to_ident(&first_variant_name),
            first_rust_type,
            is_string_like,
        )
    }
}

/// Helper function to generate Rust code from a config.
///
/// # Errors
///
/// Returns an error if generation fails.
pub fn generate(config: Config) -> Result<String, String> {
    Generator::new(config).generate()
}

fn get_rust_type_tokens_boxed(
    schema_ref: &ReferenceOr<Box<Schema>>,
    rename: &HashMap<String, String>,
) -> TokenStream {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            let name = reference.split('/').next_back().unwrap_or("Unknown");
            let final_name = rename.get(name).map_or(name, String::as_str);
            let ident = to_ident(final_name);
            quote! { #ident }
        }
        ReferenceOr::Item(schema) => {
            let s: &Schema = schema;
            match &s.schema_kind {
                SchemaKind::Type(Type::String(_)) => quote! { String },
                SchemaKind::Type(Type::Number(_)) => quote! { f64 },
                SchemaKind::Type(Type::Integer(_)) => quote! { i64 },
                SchemaKind::Type(Type::Boolean(_)) => quote! { bool },
                SchemaKind::Type(Type::Array(arr)) => {
                    if let Some(items) = &arr.items {
                        let inner_type = get_rust_type_tokens_boxed(items, rename);
                        quote! { Vec<#inner_type> }
                    } else {
                        quote! { Vec<serde_json::Value> }
                    }
                }
                _ => quote! { serde_json::Value },
            }
        }
    }
}

fn to_ident(name: &str) -> Ident {
    let mut sanitized = String::new();
    if name.is_empty() {
        return Ident::new("v_empty", Span::call_site());
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        sanitized.push('v');
    }
    for c in name.chars() {
        if c.is_alphanumeric() {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }
    if is_reserved(&sanitized) {
        Ident::new_raw(&sanitized, Span::call_site())
    } else {
        Ident::new(&sanitized, Span::call_site())
    }
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "as"
            | "async"
            | "await"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "do"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "override"
            | "priv"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "typeof"
            | "unsafe"
            | "unsized"
            | "use"
            | "virtual"
            | "where"
            | "while"
            | "yield"
            | "try"
    )
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '-' || c == '_' || c == '.' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    if result.is_empty() {
        return "Empty".to_string();
    }
    if result.starts_with(|c: char| !c.is_alphabetic()) {
        result.insert(0, 'V');
    }
    result
}
