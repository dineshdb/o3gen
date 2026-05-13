use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{File, Ident, parse2};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    pub path: String,
    #[serde(default)]
    pub rename: HashMap<String, String>,
    #[serde(default)]
    pub derive_extra: HashMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct Generator {
    config: Config,
}

impl Generator {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Creates a new builder for type generation from the given `OpenAPI` specification path.
    #[must_use]
    pub fn builder(path: impl Into<String>) -> Self {
        Self {
            config: Config {
                path: path.into(),
                ..Config::default()
            },
        }
    }

    /// Renames an `OpenAPI` type to a custom Rust name.
    #[must_use]
    pub fn rename(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.config.rename.insert(from.into(), to.into());
        self
    }

    /// Adds extra derives to a generated Rust type.
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

    /// Generates Rust types from the `OpenAPI` specification.
    ///
    /// # Errors
    ///
    /// Returns an error if the `OpenAPI` file cannot be read or parsed.
    pub fn generate(&self) -> Result<String, String> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let full_path = Path::new(&manifest_dir).join(&self.config.path);

        let spec_str = fs::read_to_string(&full_path).map_err(|e| {
            format!(
                "Failed to read `OpenAPI` file at {}: {e}",
                full_path.display()
            )
        })?;

        let openapi: OpenAPI = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Failed to parse `OpenAPI` JSON: {e}"))?;

        let mut types_tokens = TokenStream::new();

        if let Some(components) = &openapi.components {
            for (name, schema_ref) in &components.schemas {
                types_tokens.extend(self.generate_schema_tokens(name, schema_ref));
            }
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

    /// Generates types and writes them to the specified file path.
    /// Also emits `cargo:rerun-if-changed` instructions.
    ///
    /// # Errors
    ///
    /// Returns an error if generation fails or writing to the file fails.
    pub fn write_to_file(self, path: impl AsRef<Path>) -> Result<(), String> {
        let code = self.generate()?;
        let path = path.as_ref();
        fs::write(path, code).map_err(|e| format!("Failed to write to {}: {e}", path.display()))?;

        println!("cargo:rerun-if-changed={}", self.config.path);
        Ok(())
    }

    /// Generates types and writes them to a file within `OUT_DIR`.
    /// Also emits `cargo:rerun-if-changed` instructions.
    ///
    /// # Errors
    ///
    /// Returns an error if `OUT_DIR` is not set, generation fails, or writing to the file fails.
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

    fn generate_schema_tokens(&self, name: &str, schema_ref: &ReferenceOr<Schema>) -> TokenStream {
        let schema = match schema_ref {
            ReferenceOr::Reference { .. } => return TokenStream::new(),
            ReferenceOr::Item(s) => s,
        };

        let final_name = self.config.rename.get(name).map_or(name, String::as_str);
        let ident = to_ident(final_name);

        match &schema.schema_kind {
            SchemaKind::Type(Type::Object(obj)) => self.generate_object_tokens(name, &ident, obj),
            SchemaKind::Type(Type::String(s)) => self.generate_string_tokens(name, &ident, s),
            SchemaKind::AnyOf { any_of } => self.generate_any_of_tokens(name, &ident, any_of),
            _ => quote! { pub type #ident = serde_json::Value; },
        }
    }

    fn generate_object_tokens(
        &self,
        name: &str,
        ident: &Ident,
        obj: &openapiv3::ObjectType,
    ) -> TokenStream {
        let mut fields = TokenStream::new();
        for (prop_name, prop_ref) in &obj.properties {
            let prop_ident = to_ident(prop_name);
            let mut prop_type = get_rust_type_tokens_boxed(prop_ref, &self.config.rename);

            if !obj.required.contains(prop_name) {
                prop_type = quote! { Option<#prop_type> };
            }

            fields.extend(quote! {
                pub #prop_ident: #prop_type,
            });
        }

        let derives = self.get_derives(name, false);
        quote! {
            #derives
            #[serde(deny_unknown_fields)]
            pub struct #ident {
                #fields
            }
        }
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

                impl #ident {
                    pub fn as_str(&self) -> &str {
                        &self.0
                    }
                }

                impl std::fmt::Display for #ident {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{}", self.as_str())
                    }
                }

                impl From<String> for #ident {
                    fn from(s: String) -> Self {
                        Self(s)
                    }
                }

                impl From<&str> for #ident {
                    fn from(s: &str) -> Self {
                        Self(s.to_string())
                    }
                }

                impl From<#ident> for String {
                    fn from(val: #ident) -> Self {
                        val.0
                    }
                }
            }
        } else {
            let mut variants = TokenStream::new();
            let mut try_from_matches = TokenStream::new();
            let mut as_str_matches = TokenStream::new();

            for (i, val) in s.enumeration.iter().enumerate() {
                if let Some(v) = val {
                    let variant_ident = to_ident(&to_pascal_case(v));
                    let default_attr = if i == 0 {
                        quote! { #[default] }
                    } else {
                        TokenStream::new()
                    };
                    variants.extend(quote! {
                        #default_attr
                        #[serde(rename = #v)]
                        #variant_ident,
                    });
                    try_from_matches.extend(quote! {
                        #v => Ok(Self::#variant_ident),
                    });
                    as_str_matches.extend(quote! {
                        Self::#variant_ident => #v,
                    });
                }
            }

            let derives = self.get_derives(name, false);
            quote! {
                #derives
                #[serde(rename_all = "snake_case")]
                pub enum #ident {
                    #variants
                }

                impl #ident {
                    pub fn as_str(&self) -> &'static str {
                        match self {
                            #as_str_matches
                        }
                    }
                }

                impl std::fmt::Display for #ident {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{}", self.as_str())
                    }
                }

                impl TryFrom<&str> for #ident {
                    type Error = String;
                    fn try_from(s: &str) -> Result<Self, Self::Error> {
                        match s {
                            #try_from_matches
                            _ => Err(format!("Unknown variant: {s}")),
                        }
                    }
                }

                impl TryFrom<&String> for #ident {
                    type Error = String;
                    fn try_from(s: &String) -> Result<Self, Self::Error> {
                        Self::try_from(s.as_str())
                    }
                }

                impl From<#ident> for String {
                    fn from(val: #ident) -> Self {
                        val.to_string()
                    }
                }
            }
        }
    }

    fn generate_any_of_tokens(
        &self,
        name: &str,
        ident: &Ident,
        any_of: &[ReferenceOr<Schema>],
    ) -> TokenStream {
        if any_of.is_empty() {
            return quote! { pub type #ident = serde_json::Value; };
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
                        .extend(self.generate_schema_tokens(&sub_type_name_orig, sub_schema_ref));
                    (
                        final_sub_type_name.to_string(),
                        to_ident(final_sub_type_name).to_token_stream(),
                    )
                }
            };

            let variant_ident = to_ident(&variant_name);
            variants.extend(quote! {
                #variant_ident(#rust_type),
            });

            from_impls.extend(quote! {
                impl From<#rust_type> for #ident {
                    fn from(v: #rust_type) -> Self {
                        Self::#variant_ident(v)
                    }
                }
            });
        }

        let Some(first_sub_schema) = any_of.first() else {
            return quote! { pub type #ident = serde_json::Value; };
        };
        let (first_variant_ident, first_rust_type, is_string_like) =
            self.get_first_variant_info(name, first_sub_schema);
        let mut extra_impls = TokenStream::new();

        if is_string_like {
            extra_impls.extend(quote! {
                impl From<String> for #ident {
                    fn from(s: String) -> Self {
                        Self::#first_variant_ident(s.into())
                    }
                }

                impl From<&str> for #ident {
                    fn from(s: &str) -> Self {
                        Self::#first_variant_ident(s.into())
                    }
                }
            });
        }

        let derives = self.get_derives(name, true);
        quote! {
            #extra_types
            #derives
            #[serde(untagged)]
            pub enum #ident {
                #variants
            }

            impl Default for #ident {
                fn default() -> Self {
                    Self::#first_variant_ident(#first_rust_type::default())
                }
            }

            #from_impls
            #extra_impls
        }
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

/// Generates Rust types from the `OpenAPI` specification using the provided configuration.
///
/// # Errors
///
/// Returns an error if the `OpenAPI` file cannot be read or parsed.
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
