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
                    "{snake_case_op_id}_by_{}",
                    type_ident.to_string().to_snake_case()
                )
            } else {
                snake_case_op_id
            };
        let method_ident = to_ident(&final_method_name);

        let response_type = if let Some(rt) = &op.response_type {
            let ty_ident = to_ident(rt);
            quote! { #ty_ident }
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
                    let first = parts.first().map_or_else(
                        || Ident::new("unknown", proc_macro2::Span::call_site()),
                        |p| Ident::new(p, proc_macro2::Span::call_site()),
                    );
                    let second = parts.get(1).map_or_else(
                        || Ident::new("unknown", proc_macro2::Span::call_site()),
                        |p| Ident::new(p, proc_macro2::Span::call_site()),
                    );
                    quote! { #first :: #second }
                } else {
                    let p_type = to_ident(&param.rust_type);
                    quote! { #p_type }
                };
            params.extend(quote! { , #p_ident: #type_tokens });
        }

        if let Some(rb) = &op.request_body_type {
            let rb_ident = to_ident(rb);
            params.extend(quote! { , body: #rb_ident });
        }

        methods.extend(quote! {
            fn #method_ident(#params) -> impl std::future::Future<Output = Result<#response_type>> + Send;
        });
    }

    quote! {
        pub trait #type_name {
            #methods
        }
    }
}

/// Generate a default reqwest-based client implementation
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_client_impl(
    trait_ident: &Ident,
    client_ident: &Ident,
    operations: &[OperationDetails],
) -> TokenStream {
    let mut impl_methods = TokenStream::new();

    for op in operations {
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
            quote! { #ty_ident }
        } else {
            quote! { serde_json::Value }
        };

        let mut params = TokenStream::new();
        params.extend(quote! { &self });

        let mut path_replacements = TokenStream::new();
        let mut query_arg = quote! { None::<&()> };
        let mut body_arg = quote! { None::<&()> };

        for param in &op.parameters {
            let p_snake_name = param.name.to_snake_case();
            let p_ident = to_ident(&p_snake_name);
            let type_tokens =
                if matches!(param.rust_type.as_str(), "String" | "i64" | "f64" | "bool") {
                    let p_type = to_ident(&param.rust_type);
                    quote! { #p_type }
                } else if param.rust_type == "serde_json::Value" {
                    quote! { serde_json::Value }
                } else if param.rust_type.contains("::") {
                    let parts: Vec<_> = param.rust_type.split("::").collect();
                    let first = parts.first().map_or_else(
                        || Ident::new("unknown", proc_macro2::Span::call_site()),
                        |p| Ident::new(p, proc_macro2::Span::call_site()),
                    );
                    let second = parts.get(1).map_or_else(
                        || Ident::new("unknown", proc_macro2::Span::call_site()),
                        |p| Ident::new(p, proc_macro2::Span::call_site()),
                    );
                    quote! { #first :: #second }
                } else {
                    let p_type = to_ident(&param.rust_type);
                    quote! { #p_type }
                };
            params.extend(quote! { , #p_ident: #type_tokens });

            if op.path.contains(&format!("{{{}}}", param.name)) {
                let pattern = format!("{{{}}}", param.name);
                path_replacements.extend(quote! {
                    path = path.replace(#pattern, &#p_ident.to_string());
                });
            } else if p_snake_name == "query" {
                query_arg = quote! { Some(&query) };
            }
        }

        if let Some(rb) = &op.request_body_type {
            let _rb_ident = to_ident(rb);
            params.extend(quote! { , body: #_rb_ident });
            body_arg = quote! { Some(&body) };
        }

        let method_str = op.http_method.as_str();
        let helper_method = to_ident(&method_str.to_lowercase());

        let path_str = &op.path;

        let path_assignment = if path_replacements.is_empty() {
            quote! { let path = #path_str; }
        } else {
            quote! {
                let mut path = #path_str.to_string();
                #path_replacements
            }
        };

        impl_methods.extend(quote! {
            #[allow(unused_mut)]
            async fn #method_ident(#params) -> Result<#response_type> {
                #path_assignment
                self.#helper_method(path.as_ref(), #query_arg, #body_arg).await
            }
        });
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

            #[allow(dead_code)]
            async fn get<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request(reqwest::Method::GET, path, query, body).await
            }

            #[allow(dead_code)]
            async fn post<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request(reqwest::Method::POST, path, query, body).await
            }

            #[allow(dead_code)]
            async fn put<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request(reqwest::Method::PUT, path, query, body).await
            }

            #[allow(dead_code)]
            async fn patch<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request(reqwest::Method::PATCH, path, query, body).await
            }

            #[allow(dead_code)]
            async fn delete<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request(reqwest::Method::DELETE, path, query, body).await
            }

            #[allow(dead_code)]
            async fn request<Q, R, T>(&self, method: reqwest::Method, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                let url = format!("{}{}", self.base_url, path);
                let mut req = self.client.request(method, &url);
                if let Some(q) = query { req = req.query(q); }
                if let Some(b) = body { req = req.json(b); }

                let resp = req.send().await?;
                if !resp.status().is_success() {
                    return Err(ApiError::Status(resp.status()));
                }

                Ok(resp.json::<T>().await?)
            }
        }

        impl #trait_ident for #client_ident {
            #impl_methods
        }
    }
}
