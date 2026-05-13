use openapiv3::StringType;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::helpers::{to_ident, to_pascal_case};

/// Generates `as_str`, `Display`, `From<String>`, `From<&str>`, `From<Self> for String` for a
/// string newtype wrapper.
fn string_wrapper_tokens(ident: &Ident) -> TokenStream {
    quote! {
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
}

/// Generates `as_str`, `Display`, `TryFrom<&str>`, `TryFrom<&String>`, `From<Self> for String`
/// for an enum with string variants.
fn enum_conversion_tokens(
    ident: &Ident,
    as_str_matches: &TokenStream,
    try_from_matches: &TokenStream,
) -> TokenStream {
    quote! {
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

#[must_use]
pub fn generate_string_tokens(
    _name: &str,
    ident: &Ident,
    s: &StringType,
    derives: &TokenStream,
) -> TokenStream {
    if s.enumeration.is_empty() {
        let wrapper = string_wrapper_tokens(ident);
        quote! {
            #derives
            #[serde(transparent)]
            pub struct #ident(pub String);
            #wrapper
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

        let conversions = enum_conversion_tokens(ident, &as_str_matches, &try_from_matches);
        quote! {
            #derives
            #[serde(rename_all = "snake_case")]
            pub enum #ident { #variants }
            #conversions
        }
    }
}
