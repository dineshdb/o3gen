use heck::ToPascalCase;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::collections::HashMap;
use syn::Ident;

#[allow(clippy::implicit_hasher)]
pub fn get_rust_type_tokens_boxed(
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

#[must_use]
pub fn to_ident(name: &str) -> Ident {
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
        if matches!(sanitized.as_str(), "self" | "Self" | "super" | "crate") {
            Ident::new(&format!("{sanitized}_"), Span::call_site())
        } else {
            Ident::new_raw(&sanitized, Span::call_site())
        }
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

#[must_use]
pub fn to_pascal_case(s: &str) -> String {
    let mut result = s.to_pascal_case();
    if result.is_empty() {
        return "Empty".to_string();
    }
    if result.starts_with(|c: char| !c.is_alphabetic()) {
        result.insert(0, 'V');
    }
    result
}
