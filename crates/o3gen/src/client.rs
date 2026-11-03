use heck::ToSnakeCase;
use http::Method;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::emit;
use crate::helpers::to_ident;
use crate::ir::{ApiKeyLocation, SecuritySchemeIr};

#[derive(Debug, Clone)]
pub struct ParameterDetails {
    pub name: String,
    pub rust_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OperationDetails {
    pub operation_id: String,
    pub http_method: Method,
    pub response_type: Option<String>,
    pub parameters: Vec<ParameterDetails>,
    pub request_body_type: Option<String>,
    pub path: String,
    pub description: Option<String>,
}

fn compute_method_name(operation_id: &str, trait_ident: &Ident) -> String {
    let snake = operation_id.to_snake_case();
    if ["get", "post", "put", "delete", "patch"].contains(&snake.as_str()) {
        format!("{snake}_by_{}", trait_ident.to_string().to_snake_case())
    } else {
        snake
    }
}

fn type_string_to_tokens(type_str: &str) -> TokenStream {
    if matches!(type_str, "String" | "i64" | "f64" | "bool") {
        let ident = to_ident(type_str);
        quote! { #ident }
    } else if type_str == "serde_json::Value" {
        quote! { serde_json::Value }
    } else if type_str.contains("::") {
        let parts: Vec<_> = type_str.split("::").collect();
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
        let ident = to_ident(type_str);
        quote! { #ident }
    }
}

fn response_type_tokens(response_type: Option<&String>) -> TokenStream {
    if let Some(rt) = response_type {
        let ty_ident = to_ident(rt);
        quote! { #ty_ident }
    } else {
        quote! { serde_json::Value }
    }
}

#[must_use]
pub fn generate_client_traits(type_ident: &Ident, operations: &[OperationDetails]) -> TokenStream {
    let type_name = to_ident(type_ident.to_string().as_str());
    let mut methods = TokenStream::new();

    for op in operations {
        let final_method_name = compute_method_name(&op.operation_id, type_ident);
        let method_ident = to_ident(&final_method_name);
        let response_type = response_type_tokens(op.response_type.as_ref());
        let doc_attr = emit::emit_doc(op.description.as_deref());

        let mut params = TokenStream::new();
        params.extend(quote! { &self });

        for param in &op.parameters {
            let p_ident = to_ident(&param.name.to_snake_case());
            let type_tokens = type_string_to_tokens(&param.rust_type);
            params.extend(quote! { , #p_ident: #type_tokens });
        }

        if let Some(rb) = &op.request_body_type {
            let rb_ident = to_ident(rb);
            params.extend(quote! { , body: #rb_ident });
        }

        methods.extend(quote! {
            #doc_attr
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
    security_schemes: &[SecuritySchemeIr],
) -> TokenStream {
    let mut impl_methods = TokenStream::new();

    let mut auth_headers = TokenStream::new();
    for scheme in security_schemes {
        match scheme {
            SecuritySchemeIr::HttpBearer => {
                auth_headers.extend(quote! {
                    if let Some(key) = &self.api_key {
                        headers.push(("Authorization".to_string(), format!("Bearer {key}")));
                    }
                });
            }
            SecuritySchemeIr::ApiKey {
                location: ApiKeyLocation::Header,
                field_name,
            } => {
                auth_headers.extend(quote! {
                    if let Some(key) = &self.api_key {
                        headers.push((#field_name.to_string(), key.clone()));
                    }
                });
            }
            _ => {}
        }
    }

    for op in operations {
        let final_method_name = compute_method_name(&op.operation_id, trait_ident);
        let method_ident = to_ident(&final_method_name);
        let response_type = response_type_tokens(op.response_type.as_ref());

        let mut params = TokenStream::new();
        params.extend(quote! { &self });

        let mut path_replacements = TokenStream::new();
        let mut query_arg = quote! { None::<&()> };
        let mut body_arg = quote! { None::<&()> };

        for param in &op.parameters {
            let p_snake_name = param.name.to_snake_case();
            let p_ident = to_ident(&p_snake_name);
            let type_tokens = type_string_to_tokens(&param.rust_type);
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
        /// One HTTP exchange for a [`Transport`] to carry: the resolved
        /// URL, the headers (auth included), and the serialized body.
        /// Plain data on purpose — an injected transport (a `wasi:http`
        /// guest bridge, a test double) needs no HTTP crate to answer it.
        #[derive(Debug, Clone)]
        pub struct TransportRequest {
            pub method: String,
            pub url: String,
            pub headers: Vec<(String, String)>,
            pub body: Option<Vec<u8>>,
        }

        /// The transport's answer: a status code and the raw body. The
        /// client does the JSON decoding.
        #[derive(Debug, Clone)]
        pub struct TransportResponse {
            pub status: u16,
            pub body: Vec<u8>,
        }

        /// The transport's pending exchange: a boxed future (a trait
        /// returning `impl Future` is not dyn-compatible, and the
        /// client holds the transport as `Arc<dyn Transport>`).
        pub type TransportFuture = std::pin::Pin<
            Box<
                dyn std::future::Future<Output = std::result::Result<TransportResponse, String>>
                    + Send
                    + 'static,
            >,
        >;

        /// The pluggable HTTP backend. The default is reqwest (the
        /// `reqwest` feature); a target reqwest cannot serve — a wasm
        /// component, say — injects its own with
        /// [`#client_ident::with_transport`].
        pub trait Transport: Send + Sync + 'static {
            fn execute(&self, request: TransportRequest) -> TransportFuture;
        }

        pub struct #client_ident {
            #[cfg(feature = "reqwest")]
            client: reqwest::Client,
            base_url: String,
            api_key: Option<String>,
            transport: Option<std::sync::Arc<dyn Transport>>,
        }

        impl #client_ident {
            pub fn new(base_url: String) -> Self {
                Self {
                    #[cfg(feature = "reqwest")]
                    client: reqwest::Client::builder()
                        .user_agent("o3gen-client/0.1.0")
                        .build()
                        .unwrap_or_else(|_| reqwest::Client::new()),
                    base_url,
                    api_key: None,
                    transport: None,
                }
            }

            pub fn with_api_key(mut self, api_key: String) -> Self {
                self.api_key = Some(api_key);
                self
            }

            /// Carry every call over an injected [`Transport`] instead of
            /// the built-in reqwest client.
            pub fn with_transport(mut self, transport: std::sync::Arc<dyn Transport>) -> Self {
                self.transport = Some(transport);
                self
            }

            #[allow(dead_code)]
            async fn get<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request("GET", path, query, body).await
            }

            #[allow(dead_code)]
            async fn post<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request("POST", path, query, body).await
            }

            #[allow(dead_code)]
            async fn put<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request("PUT", path, query, body).await
            }

            #[allow(dead_code)]
            async fn patch<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request("PATCH", path, query, body).await
            }

            #[allow(dead_code)]
            async fn delete<Q, R, T>(&self, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                self.request("DELETE", path, query, body).await
            }

            #[allow(dead_code)]
            async fn request<Q, R, T>(&self, method: &str, path: &str, query: Option<&Q>, body: Option<&R>) -> Result<T>
            where Q: serde::Serialize, R: serde::Serialize, T: serde::de::DeserializeOwned
            {
                let url = format!("{}{}", self.base_url, path);
                let mut headers: Vec<(String, String)> = Vec::new();
                #auth_headers

                // The injected transport, when present, carries every
                // call — reqwest never links into its path.
                if let Some(transport) = &self.transport {
                    let mut full_url = url;
                    if let Some(q) = query {
                        let qs = transport_query(q)?;
                        if !qs.is_empty() {
                            full_url = format!("{full_url}?{qs}");
                        }
                    }
                    let body_bytes = match body {
                        Some(b) => Some(serde_json::to_vec(b).map_err(ApiError::Serde)?),
                        None => None,
                    };
                    if body_bytes.is_some() {
                        headers.push(("content-type".to_string(), "application/json".to_string()));
                    }
                    let response = transport
                        .execute(TransportRequest {
                            method: method.to_string(),
                            url: full_url,
                            headers,
                            body: body_bytes,
                        })
                        .await
                        .map_err(ApiError::Transport)?;
                    if !(200..300).contains(&response.status) {
                        return Err(ApiError::Status {
                            status: response.status,
                            body: String::from_utf8_lossy(&response.body).to_string(),
                        });
                    }
                    return Ok(serde_json::from_slice(&response.body)?);
                }

                #[cfg(feature = "reqwest")]
                {
                    // Retry transient connection failures (e.g. a busy local server
                    // refusing connections under bursty concurrency). Only connect-level
                    // errors are retried; status errors and delivered requests are not.
                    let mut last_connect_err: Option<reqwest::Error> = None;
                    for attempt in 0..3u32 {
                        let mut req = self.client.request(reqwest::Method::from_bytes(method.as_bytes()).expect("static method"), &url);
                        if let Some(q) = query { req = req.query(q); }
                        if let Some(b) = body { req = req.json(b); }
                        for (name, value) in &headers { req = req.header(name, value); }
                        match req.send().await {
                            Ok(resp) => {
                                if !resp.status().is_success() {
                                    let status = resp.status().as_u16();
                                    let body = resp.text().await.unwrap_or_default();
                                    return Err(ApiError::Status { status, body });
                                }
                                return Ok(resp.json::<T>().await?);
                            }
                            Err(e) if e.is_connect() && attempt < 2 => {
                                last_connect_err = Some(e);
                                tokio::time::sleep(std::time::Duration::from_millis(25 * (attempt + 1) as u64)).await;
                                continue;
                            }
                            Err(e) => return Err(ApiError::Reqwest(e)),
                        }
                    }
                    Err(ApiError::Reqwest(last_connect_err.expect("retry loop ended without an error")))
                }
                #[cfg(not(feature = "reqwest"))]
                {
                    Err(ApiError::Builder(
                        "no HTTP backend: build with the reqwest feature, or inject one with with_transport".to_string(),
                    ))
                }
            }
        }

        /// Query-string encoding for the transport path: the FLAT
        /// fields of the query struct (strings, numbers, bools, and
        /// arrays of those), application/x-www-form-urlencoded style —
        /// the same shape reqwest's `.query` serializes for the flat
        /// parameter sets the generated clients carry. Nested values
        /// refuse loudly rather than encode wrongly.
        fn transport_query<Q: serde::Serialize>(query: &Q) -> Result<String> {
            let value = serde_json::to_value(query).map_err(ApiError::Serde)?;
            let Some(obj) = value.as_object() else {
                return Err(ApiError::Builder("query must be a flat struct".to_string()));
            };
            let mut pairs: Vec<String> = Vec::new();
            for (key, item) in obj {
                let mut push_one = |v: &serde_json::Value| -> Result<()> {
                    let encoded = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => {
                            return Err(ApiError::Builder(format!(
                                "query field {key} is not a flat value"
                            )))
                        }
                    };
                    pairs.push(format!("{}={}", key.replace('-', "_"), transport_percent_encode(&encoded)));
                    Ok(())
                };
                match item {
                    serde_json::Value::Array(items) => {
                        for item in items {
                            push_one(item)?;
                        }
                    }
                    other => push_one(other)?,
                }
            }
            Ok(pairs.join("&"))
        }

        /// The query-string minimal encode: everything outside the
        /// unreserved set escapes as %XX.
        fn transport_percent_encode(input: &str) -> String {
            let mut out = String::with_capacity(input.len());
            for byte in input.as_bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(*byte as char)
                    }
                    _ => out.push_str(&format!("%{byte:02X}")),
                }
            }
            out
        }

        impl #trait_ident for #client_ident {
            #impl_methods
        }
    }
}
