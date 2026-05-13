use openapiv3::{ObjectType, ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::Ident;

use crate::helpers::{get_rust_type_tokens_boxed, to_ident, to_pascal_case};

/// # Errors
///
/// Returns an error if generation of inline types fails.
#[allow(clippy::implicit_hasher)]
pub fn generate_object_tokens(
    name: &str,
    ident: &Ident,
    obj: &ObjectType,
    derives: &TokenStream,
    rename: &HashMap<String, String>,
    generate_inline: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<TokenStream, String> {
    let mut fields = TokenStream::new();
    let mut extra_types = TokenStream::new();
    let mut prop_names: Vec<_> = obj.properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = obj.properties.get(prop_name) else {
            continue;
        };
        let prop_ident = to_ident(prop_name);
        let mut prop_type = resolve_property_type(
            name,
            prop_name,
            prop_ref,
            rename,
            &mut extra_types,
            generate_inline,
        )?;
        if !obj.required.contains(prop_name) {
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

/// # Errors
///
/// Returns an error if generation of inline types fails.
#[allow(clippy::implicit_hasher)]
pub fn generate_all_of_tokens(
    name: &str,
    ident: &Ident,
    all_of: &[ReferenceOr<Schema>],
    derives: &TokenStream,
    rename: &HashMap<String, String>,
    schemas: &HashMap<String, ReferenceOr<Schema>>,
    generate_inline: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<TokenStream, String> {
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    for schema_ref in all_of {
        collect_properties(schema_ref, &mut properties, &mut required, schemas);
    }

    let mut fields = TokenStream::new();
    let mut extra_types = TokenStream::new();
    let mut prop_names: Vec<_> = properties.keys().collect();
    prop_names.sort();

    for prop_name in prop_names {
        let Some(prop_ref) = properties.get(prop_name) else {
            continue;
        };
        let prop_ident = to_ident(prop_name);
        let mut prop_type = resolve_property_type(
            name,
            prop_name,
            prop_ref,
            rename,
            &mut extra_types,
            generate_inline,
        )?;
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

/// Resolve the Rust type for a property. For primitives and refs, returns directly.
/// For inline objects/anyOf/allOf, generates a named struct via the callback.
fn resolve_property_type(
    parent_name: &str,
    prop_name: &str,
    prop_ref: &ReferenceOr<Box<Schema>>,
    rename: &HashMap<String, String>,
    extra_types: &mut TokenStream,
    generate_inline: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<TokenStream, String> {
    match prop_ref {
        ReferenceOr::Reference { .. } => Ok(get_rust_type_tokens_boxed(prop_ref, rename)),
        ReferenceOr::Item(schema) => match &schema.schema_kind {
            SchemaKind::Type(Type::Array(arr)) => {
                if let Some(items) = &arr.items {
                    let inner_type = resolve_property_type(
                        parent_name,
                        prop_name,
                        items,
                        rename,
                        extra_types,
                        generate_inline,
                    )?;
                    Ok(quote! { Vec<#inner_type> })
                } else {
                    Ok(quote! { Vec<serde_json::Value> })
                }
            }
            SchemaKind::Type(
                Type::String(_) | Type::Number(_) | Type::Integer(_) | Type::Boolean(_),
            ) => Ok(get_rust_type_tokens_boxed(prop_ref, rename)),
            _ => {
                let sub_type_name = format!("{}{}", parent_name, to_pascal_case(prop_name));
                let generated =
                    generate_inline(&sub_type_name, &ReferenceOr::Item(*schema.clone()))?;
                extra_types.extend(generated);
                let sub_ident = to_ident(&sub_type_name);
                Ok(quote! { #sub_ident })
            }
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
