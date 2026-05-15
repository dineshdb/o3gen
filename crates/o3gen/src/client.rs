use heck::ToSnakeCase;
use http::Method;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::helpers::to_ident;

#[derive(Debug, Clone)]
pub struct ParameterDetails {
    pub name: String,
    pub rust_type: String,
}

#[derive(Debug, Clone)]
pub struct OperationDetails {
    pub operation_id: String,
    pub http_method: Method,
    pub response_type: Option<String>,
    pub parameters: Vec<ParameterDetails>,
    pub request_body_type: Option<String>,
    pub path: String,
}

/// Generate client trait methods for API operations
#[must_use]
pub fn generate_client_traits(type_ident: &Ident, operations: &[OperationDetails]) -> TokenStream {
    let type_name = to_ident(type_ident.to_string().as_str());
    let mut methods = TokenStream::new();

    for op in operations {
        let snake_case_op_id = op.operation_id.to_snake_case();
        let final_method_name =
            if ["get", "post", "put", "delete", "patch"].contains(&snake_case_op_id.as_str()) {
                format!(
                    "{}_by_{}",
                    snake_case_op_id,
                    type_ident.to_string().to_snake_case()
                )
            } else {
                snake_case_op_id
            };
        let method_ident = to_ident(&final_method_name);

        let response_type = if let Some(rt) = &op.response_type {
            let ty_ident = to_ident(rt);
            quote! { crate::types::#ty_ident }
        } else {
            quote! { serde_json::Value }
        };

        let mut params = TokenStream::new();
        params.extend(quote! { &self });

        for param in &op.parameters {
            let p_ident = to_ident(&param.name.to_snake_case());
            let type_tokens =
                if matches!(param.rust_type.as_str(), "String" | "i64" | "f64" | "bool") {
                    let p_type = to_ident(&param.rust_type);
                    quote! { #p_type }
                } else if param.rust_type == "serde_json::Value" {
                    quote! { serde_json::Value }
                } else if param.rust_type.contains("::") {
                    let parts: Vec<_> = param.rust_type.split("::").collect();
                    if let (Some(f), Some(s)) = (parts.first(), parts.get(1)) {
                        let first = Ident::new(f, proc_macro2::Span::call_site());
                        let second = Ident::new(s, proc_macro2::Span::call_site());
                        quote! { #first :: #second }
                    } else {
                        let p_type = to_ident(&param.rust_type);
                        quote! { crate::types::#p_type }
                    }
                } else {
                    let p_type = to_ident(&param.rust_type);
                    quote! { crate::types::#p_type }
                };
            params.extend(quote! { , #p_ident: #type_tokens });
        }

        if let Some(rb) = &op.request_body_type {
            let rb_ident = to_ident(rb);
            params.extend(quote! { , body: crate::types::#rb_ident });
        }

        methods.extend(quote! {
            fn #method_ident(#params) -> impl std::future::Future<Output = Result<#response_type, Self::ApiError>> + Send;
        });
    }

    quote! {
        pub trait #type_name {
            type ApiError: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync;
            #methods
        }
    }
}

/// Generate a default reqwest-based client implementation
#[must_use]
pub fn generate_client_impl(
    trait_ident: &Ident,
    client_ident: &Ident,
    operations: &[OperationDetails],
) -> TokenStream {
    let mut impl_methods = TokenStream::new();

    for op in operations {
        impl_methods.extend(generate_client_method(trait_ident, op));
    }

    quote! {
        pub struct #client_ident {
            client: reqwest::Client,
            base_url: String,
        }

        impl #client_ident {
            pub fn new(base_url: String) -> Self {
                Self {
                    client: reqwest::Client::builder()
                        .user_agent("o3gen-client/0.1.0")
                        .build()
                        .unwrap_or_else(|_| reqwest::Client::new()),
                    base_url,
                }
            }
        }

        impl #trait_ident for #client_ident {
            type ApiError = ApiError;
            #impl_methods
        }

        #[derive(Debug)]
        pub enum ApiError {
            Reqwest(reqwest::Error),
            Status(reqwest::StatusCode),
        }

        impl std::fmt::Display for ApiError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Reqwest(e) => write!(f, "Request error: {e}"),
                    Self::Status(s) => write!(f, "Status error: {s}"),
                }
            }
        }

        impl std::error::Error for ApiError {}

        impl From<reqwest::Error> for ApiError {
            fn from(e: reqwest::Error) -> Self {
                Self::Reqwest(e)
            }
        }

        impl From<reqwest::StatusCode> for ApiError {
            fn from(s: reqwest::StatusCode) -> Self {
                Self::Status(s)
            }
        }
    }
}

fn generate_client_method(trait_ident: &Ident, op: &OperationDetails) -> TokenStream {
    let snake_case_op_id = op.operation_id.to_snake_case();
    let final_method_name =
        if ["get", "post", "put", "delete", "patch"].contains(&snake_case_op_id.as_str()) {
            format!(
                "{snake_case_op_id}_by_{}",
                trait_ident.to_string().to_snake_case()
            )
        } else {
            snake_case_op_id
        };
    let method_ident = to_ident(&final_method_name);

    let response_type = if let Some(rt) = &op.response_type {
        let ty_ident = to_ident(rt);
        quote! { crate::types::#ty_ident }
    } else {
        quote! { serde_json::Value }
    };

    let mut params = TokenStream::new();
    params.extend(quote! { &self });

    let mut path_replacements = TokenStream::new();
    let mut query_call = TokenStream::new();

    for param in &op.parameters {
        let p_snake_name = param.name.to_snake_case();
        let p_ident = to_ident(&p_snake_name);
        let type_tokens = if matches!(param.rust_type.as_str(), "String" | "i64" | "f64" | "bool") {
            let p_type = to_ident(&param.rust_type);
            quote! { #p_type }
        } else if param.rust_type == "serde_json::Value" {
            quote! { serde_json::Value }
        } else if param.rust_type.contains("::") {
            let parts: Vec<_> = param.rust_type.split("::").collect();
            if let (Some(f), Some(s)) = (parts.first(), parts.get(1)) {
                let first = Ident::new(f, proc_macro2::Span::call_site());
                let second = Ident::new(s, proc_macro2::Span::call_site());
                quote! { #first :: #second }
            } else {
                let p_type = to_ident(&param.rust_type);
                quote! { crate::types::#p_type }
            }
        } else {
            let p_type = to_ident(&param.rust_type);
            quote! { crate::types::#p_type }
        };
        params.extend(quote! { , #p_ident: #type_tokens });

        if op.path.contains(&format!("{{{}}}", param.name)) {
            let pattern = format!("{{{}}}", param.name);
            path_replacements.extend(quote! {
                path = path.replace(#pattern, &#p_ident.to_string());
            });
        } else if p_snake_name == "query" {
            query_call = quote! { req = req.query(&query); };
        }
    }

    if let Some(rb) = &op.request_body_type {
        let rb_ident = to_ident(rb);
        params.extend(quote! { , body: crate::types::#rb_ident });
    }

    let method_str = op.http_method.as_str();
    let http_method = match method_str {
        "GET" => quote! { reqwest::Method::GET },
        "POST" => quote! { reqwest::Method::POST },
        "PUT" => quote! { reqwest::Method::PUT },
        "DELETE" => quote! { reqwest::Method::DELETE },
        "PATCH" => quote! { reqwest::Method::PATCH },
        "HEAD" => quote! { reqwest::Method::HEAD },
        "OPTIONS" => quote! { reqwest::Method::OPTIONS },
        "TRACE" => quote! { reqwest::Method::TRACE },
        _ => quote! { reqwest::Method::from_bytes(#method_str.as_bytes()).unwrap() },
    };

    let path_str = &op.path;
    let body_call = if op.request_body_type.is_some() {
        quote! { req = req.json(&body); }
    } else {
        TokenStream::new()
    };

    let mut_path = if path_replacements.is_empty() {
        TokenStream::new()
    } else {
        quote! { mut }
    };

    let mut_req = if query_call.is_empty() && body_call.is_empty() {
        TokenStream::new()
    } else {
        quote! { mut }
    };

    quote! {
        async fn #method_ident(#params) -> Result<#response_type, Self::ApiError> {
            let #mut_path path = #path_str.to_string();
            #path_replacements
            let url = format!("{}{}", self.base_url, path);
            let #mut_req req = self.client.request(#http_method, &url);
            #query_call
            #body_call
            let resp = req.send().await?;
            if !resp.status().is_success() {
                return Err(ApiError::Status(resp.status()));
            }
            Ok(resp.json::<#response_type>().await?)
        }
    }
}
