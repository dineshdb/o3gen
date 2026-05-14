use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{File, Ident, parse2};

use crate::any_of::generate_any_of_tokens;
use crate::config::Config;
use crate::helpers::{get_rust_type_tokens_boxed, to_ident};
use crate::object::{generate_all_of_tokens, generate_object_tokens};
use crate::string::generate_string_tokens;

#[derive(Debug)]
pub struct Generator {
    config: Config,
    schemas: HashMap<String, ReferenceOr<Schema>>,
}

impl Generator {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            schemas: HashMap::new(),
        }
    }

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

    #[must_use]
    pub fn rename(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.config.rename.insert(from.into(), to.into());
        self
    }

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

    /// # Errors
    ///
    /// Returns an error if the `OpenAPI` file cannot be read, parsed, or if code generation fails.
    pub fn generate(&mut self) -> Result<String, String> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let full_path = Path::new(&manifest_dir).join(&self.config.path);

        let spec_str = fs::read_to_string(&full_path).map_err(|e| {
            format!(
                "Failed to read OpenAPI file at {}: {e}",
                full_path.display()
            )
        })?;

        let mut openapi: OpenAPI = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Failed to parse OpenAPI JSON: {e}"))?;

        crate::flatten::flatten(&mut openapi, &self.config.rename);

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

    /// # Errors
    ///
    /// Returns an error if generation fails or writing to the file fails.
    pub fn write_to_file(mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let code = self.generate()?;
        let path = path.as_ref();
        fs::write(path, code).map_err(|e| format!("Failed to write to {}: {e}", path.display()))?;
        println!("cargo:rerun-if-changed={}", self.config.path);
        Ok(())
    }

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
            SchemaKind::Type(Type::Object(obj)) => {
                let derives = self.get_derives(name, false);
                generate_object_tokens(&ident, obj, &derives, &self.config.rename)
            }
            SchemaKind::Type(Type::String(s)) => {
                let derives = self.get_derives(name, false);
                Ok(generate_string_tokens(name, &ident, s, &derives))
            }
            SchemaKind::AnyOf { any_of } => {
                let derives = self.get_derives(name, true);
                generate_any_of_tokens(
                    name,
                    &ident,
                    any_of,
                    &derives,
                    &self.config.rename,
                    &mut |_, _| Ok(TokenStream::new()),
                )
            }
            SchemaKind::AllOf { all_of } => {
                let derives = self.get_derives(name, false);
                generate_all_of_tokens(
                    &ident,
                    all_of,
                    &derives,
                    &self.config.rename,
                    &self.schemas,
                    &mut |n, s| self.generate_schema_tokens(n, s),
                )
            }
            SchemaKind::Type(Type::Array(arr)) => {
                if let Some(items) = &arr.items {
                    let inner_type = get_rust_type_tokens_boxed(items, &self.config.rename);
                    Ok(quote! { pub type #ident = Vec<#inner_type>; })
                } else {
                    Ok(quote! { pub type #ident = Vec<serde_json::Value>; })
                }
            }
            _ => Ok(quote! { pub type #ident = serde_json::Value; }),
        }
    }
}

/// # Errors
///
/// Returns an error if generation fails.
pub fn generate(config: Config) -> Result<String, String> {
    Generator::new(config).generate()
}
