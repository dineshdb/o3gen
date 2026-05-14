use heck::ToSnakeCase;
use openapiv3::{ObjectType, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::Ident;

use crate::helpers::{get_rust_type_tokens_boxed, to_ident};

/// # Errors
///
/// Returns an error if generation of inline types fails.
#[allow(clippy::implicit_hasher)]
pub fn generate_object_tokens(
    ident: &Ident,
    obj: &ObjectType,
    derives: &TokenStream,
    rename: &HashMap<String, String>,
) -> Result<TokenStream, String> {
    let mut fields = TokenStream::new();
    let extra_types = TokenStream::new();
    let mut prop_names: Vec<_> = obj.properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = obj.properties.get(prop_name) else {
            continue;
        };
        let snake_case_name = prop_name.to_snake_case();
        let prop_ident = to_ident(&snake_case_name);
        let mut prop_type = resolve_property_type(prop_ref, rename)?;
        if !obj.required.contains(prop_name) {
            prop_type = quote! { Option<#prop_type> };
        }

        let original_name = prop_name;
        fields.extend(quote! {
            #[serde(rename = #original_name)]
            pub #prop_ident: #prop_type,
        });
    }

    Ok(quote! {
        #extra_types
        #derives
        #[serde(deny_unknown_fields)]
        pub struct #ident { #fields }
    })
}

/// # Errors
///
/// Returns an error if generation of inline types fails.
#[allow(clippy::implicit_hasher)]
pub fn generate_all_of_tokens(
    ident: &Ident,
    all_of: &[ReferenceOr<Schema>],
    derives: &TokenStream,
    rename: &HashMap<String, String>,
    schemas: &HashMap<String, ReferenceOr<Schema>>,
    _generate_inline: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<TokenStream, String> {
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    for schema_ref in all_of {
        collect_properties(schema_ref, &mut properties, &mut required, schemas);
    }

    let mut fields = TokenStream::new();
    let extra_types = TokenStream::new();
    let mut prop_names: Vec<_> = properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = properties.get(prop_name) else {
            continue;
        };
        let prop_ident = to_ident(prop_name);
        let mut prop_type = resolve_property_type(prop_ref, rename)?;
        if !required.contains(prop_name) {
            prop_type = quote! { Option<#prop_type> };
        }
        fields.extend(quote! { pub #prop_ident: #prop_type, });
    }

    Ok(quote! {
        #extra_types
        #derives
        #[serde(deny_unknown_fields)]
        pub struct #ident { #fields }
    })
}

/// Resolve the Rust type for a property. After flatten, all complex inline schemas are $refs.
fn resolve_property_type(
    prop_ref: &ReferenceOr<Box<Schema>>,
    rename: &HashMap<String, String>,
) -> Result<TokenStream, String> {
    match prop_ref {
        ReferenceOr::Reference { .. } => Ok(get_rust_type_tokens_boxed(prop_ref, rename)),
        ReferenceOr::Item(schema) => match &schema.schema_kind {
            SchemaKind::Type(Type::Array(arr)) => {
                if let Some(items) = &arr.items {
                    let inner_type = resolve_property_type(items, rename)?;
                    Ok(quote! { Vec<#inner_type> })
                } else {
                    Ok(quote! { Vec<serde_json::Value> })
                }
            }
            SchemaKind::Type(
                Type::String(_) | Type::Number(_) | Type::Integer(_) | Type::Boolean(_),
            ) => Ok(get_rust_type_tokens_boxed(prop_ref, rename)),
            // After flatten, we should not reach here for complex types
            // They should all be $refs now. If we do, use serde_json::Value as fallback.
            _ => Ok(quote! { serde_json::Value }),
        },
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
