use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::collections::HashMap;
use syn::Ident;

use crate::helpers::to_ident;

/// Resolves a sub-schema within anyOf into a (`variant_name`, `rust_type`) pair.
/// For inline schemas, also generates the sub-type definition into `extra_types`.
#[allow(clippy::missing_errors_doc)]
fn resolve_variant(
    parent_name: &str,
    index: usize,
    sub_schema_ref: &ReferenceOr<Schema>,
    rename: &HashMap<String, String>,
    extra_types: &mut TokenStream,
    generate_sub_schema: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<(String, TokenStream), String> {
    match sub_schema_ref {
        ReferenceOr::Reference { reference } => {
            let ref_name = reference.split('/').next_back().unwrap_or("Unknown");
            let final_ref_name = rename.get(ref_name).map_or(ref_name, String::as_str);
            Ok((
                final_ref_name.to_string(),
                to_ident(final_ref_name).to_token_stream(),
            ))
        }
        ReferenceOr::Item(sub_schema) => {
            let is_enum = matches!(
                &sub_schema.schema_kind,
                SchemaKind::Type(Type::String(s)) if !s.enumeration.is_empty()
            );
            let suffix = if is_enum { "Enum" } else { "Subtype" };
            let sub_type_name_orig = format!("{}{}{}", parent_name, suffix, index + 1);
            let final_sub_type_name = rename
                .get(&sub_type_name_orig)
                .map_or(sub_type_name_orig.as_str(), String::as_str);
            extra_types.extend(generate_sub_schema(&sub_type_name_orig, sub_schema_ref)?);
            Ok((
                final_sub_type_name.to_string(),
                to_ident(final_sub_type_name).to_token_stream(),
            ))
        }
    }
}

#[allow(clippy::missing_errors_doc, clippy::implicit_hasher)]
pub fn generate_any_of_tokens(
    name: &str,
    ident: &Ident,
    any_of: &[ReferenceOr<Schema>],
    derives: &TokenStream,
    rename: &HashMap<String, String>,
    generate_sub_schema: &mut impl FnMut(&str, &ReferenceOr<Schema>) -> Result<TokenStream, String>,
) -> Result<TokenStream, String> {
    if any_of.is_empty() {
        return Ok(quote! { pub type #ident = serde_json::Value; });
    }

    let mut variants = TokenStream::new();
    let mut extra_types = TokenStream::new();
    let mut from_impls = TokenStream::new();

    for (i, sub_schema_ref) in any_of.iter().enumerate() {
        let (variant_name, rust_type) = resolve_variant(
            name,
            i,
            sub_schema_ref,
            rename,
            &mut extra_types,
            generate_sub_schema,
        )?;
        let variant_ident = to_ident(&variant_name);
        variants.extend(quote! { #variant_ident(#rust_type), });
        from_impls.extend(quote! {
            impl From<#rust_type> for #ident {
                fn from(v: #rust_type) -> Self { Self::#variant_ident(v) }
            }
        });
    }

    let Some(first_sub_schema) = any_of.first() else {
        return Ok(quote! { pub type #ident = serde_json::Value; });
    };
    let (first_variant_name, first_rust_type) = resolve_variant(
        name,
        0,
        first_sub_schema,
        rename,
        &mut TokenStream::new(),
        &mut |_, _| Ok(TokenStream::new()),
    )?;
    let first_variant_ident = to_ident(&first_variant_name);

    let is_string_like = matches!(
        first_sub_schema,
        ReferenceOr::Item(sub) if matches!(
            &sub.schema_kind,
            SchemaKind::Type(Type::String(s)) if s.enumeration.is_empty()
        )
    );

    let mut extra_impls = TokenStream::new();
    if is_string_like {
        extra_impls.extend(quote! {
            impl From<String> for #ident {
                fn from(s: String) -> Self { Self::#first_variant_ident(s.into()) }
            }
            impl From<&str> for #ident {
                fn from(s: &str) -> Self { Self::#first_variant_ident(s.into()) }
            }
        });
    }

    Ok(quote! {
        #extra_types
        #derives
        #[serde(untagged)]
        pub enum #ident { #variants }
        impl Default for #ident {
            fn default() -> Self { Self::#first_variant_ident(#first_rust_type::default()) }
        }
        #from_impls
        #extra_impls
    })
}
