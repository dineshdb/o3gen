use openapiv3::{ObjectType, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::Ident;

use crate::helpers::{get_rust_type_tokens_boxed, to_ident};

#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn generate_object_tokens(
    _name: &str,
    ident: &Ident,
    obj: &ObjectType,
    derives: &TokenStream,
    rename: &HashMap<String, String>,
) -> TokenStream {
    let mut fields = TokenStream::new();
    let mut prop_names: Vec<_> = obj.properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = obj.properties.get(prop_name) else {
            continue;
        };
        let prop_ident = to_ident(prop_name);
        let mut prop_type = get_rust_type_tokens_boxed(prop_ref, rename);
        if !obj.required.contains(prop_name) {
            prop_type = quote! { Option<#prop_type> };
        }
        fields.extend(quote! { pub #prop_ident: #prop_type, });
    }

    quote! {
        #derives
        #[serde(deny_unknown_fields)]
        pub struct #ident { #fields }
    }
}

#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn generate_all_of_tokens(
    _name: &str,
    ident: &Ident,
    all_of: &[ReferenceOr<Schema>],
    derives: &TokenStream,
    rename: &HashMap<String, String>,
    schemas: &HashMap<String, ReferenceOr<Schema>>,
) -> TokenStream {
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    for schema_ref in all_of {
        collect_properties(schema_ref, &mut properties, &mut required, schemas);
    }

    let mut fields = TokenStream::new();
    let mut prop_names: Vec<_> = properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = properties.get(prop_name) else {
            continue;
        };
        let prop_ident = to_ident(prop_name);
        let mut prop_type = get_rust_type_tokens_boxed(prop_ref, rename);
        if !required.contains(prop_name) {
            prop_type = quote! { Option<#prop_type> };
        }
        fields.extend(quote! { pub #prop_ident: #prop_type, });
    }

    quote! {
        #derives
        #[serde(deny_unknown_fields)]
        pub struct #ident { #fields }
    }
}

fn collect_properties(
    schema_ref: &ReferenceOr<Schema>,
    properties: &mut HashMap<String, ReferenceOr<Box<Schema>>>,
    required: &mut Vec<String>,
    schemas: &HashMap<String, ReferenceOr<Schema>>,
) {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            let name = reference.split('/').next_back().unwrap_or("Unknown");
            if let Some(resolved) = schemas.get(name) {
                collect_properties(resolved, properties, required, schemas);
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
                    collect_properties(sub, properties, required, schemas);
                }
            }
            _ => {}
        },
    }
}
