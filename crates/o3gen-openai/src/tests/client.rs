use crate::{OpenAIApi, OpenAIApiClient};
use mockito::ServerGuard;
use serde::Serialize;

fn mock_get(
    server: &mut ServerGuard,
    path: &str,
    status: usize,
    body: &impl Serialize,
) -> mockito::Mock {
    server
        .mock("GET", path)
        .with_status(status)
        .with_body(serde_json::to_string(body).unwrap())
        .create()
}

fn mock_post(
    server: &mut ServerGuard,
    path: &str,
    status: usize,
    body: &impl Serialize,
) -> mockito::Mock {
    server
        .mock("POST", path)
        .with_status(status)
        .with_body(serde_json::to_string(body).unwrap())
        .create()
}

// ── List Models ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_models() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let mock = mock_get(
        &mut server,
        "/models",
        200,
        &crate::types::ListModelsResponse {
            object: crate::types::ListModelsResponseObject::List,
            data: vec![
                crate::types::Model {
                    id: "gpt-4o".to_string(),
                    object: crate::types::ModelObject::Model,
                    created: 1661989079,
                    owned_by: "openai".to_string(),
                },
                crate::types::Model {
                    id: "gpt-3.5-turbo".to_string(),
                    object: crate::types::ModelObject::Model,
                    created: 1677610605,
                    owned_by: "openai".to_string(),
                },
            ],
        },
    );

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 2);
    assert_eq!(resp.data[0].id, "gpt-4o");
    assert_eq!(resp.data[1].id, "gpt-3.5-turbo");

    mock.assert_async().await;
}

// ── Create Chat Completion ─────────────────────────────────────────────

#[tokio::test]
async fn test_create_chat_completion() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let mock = mock_post(
        &mut server,
        "/chat/completions",
        200,
        &crate::types::CreateChatCompletionResponse {
            id: "chatcmpl-abc123".into(),
            object: crate::types::CreateChatCompletionResponseObject::ChatCompletion,
            created: 1677610605,
            model: "gpt-3.5-turbo".into(),
            system_fingerprint: None,
            choices: vec![crate::types::CreateChatCompletionResponseChoices {
                index: 0,
                finish_reason: crate::types::CreateChatCompletionResponseChoicesFinishReason::Stop,
                message: crate::types::ChatCompletionResponseMessage {
                    role: crate::types::ChatCompletionResponseMessageRole::Assistant,
                    content: Some("Hello! How can I help you today?".into()),
                    tool_calls: None,
                    function_call: None,
                },
            }],
            usage: Some(crate::types::CompletionUsage {
                prompt_tokens: 9,
                completion_tokens: 12,
                total_tokens: 21,
            }),
        },
    );

    let body = crate::types::ChatCompletionRequest::builder()
        .model("gpt-3.5-turbo".to_string())
        .messages(vec![
            crate::types::ChatCompletionRequestUserMessage::builder()
                .role(crate::types::ChatCompletionRequestUserMessageRole::User)
                .content(
                    crate::types::ChatCompletionRequestUserMessageContent::String(
                        "Hello".to_string(),
                    ),
                )
                .build()
                .unwrap()
                .into(),
        ])
        .build()
        .unwrap();

    let resp = OpenAIApi::create_chat_completion(&client, body)
        .await
        .unwrap();
    assert_eq!(resp.id, "chatcmpl-abc123");
    assert_eq!(resp.model, "gpt-3.5-turbo");
    assert_eq!(resp.choices.len(), 1);
    assert_eq!(
        resp.choices[0].message.content,
        Some("Hello! How can I help you today?".to_string())
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_embedding() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let mock = mock_post(
        &mut server,
        "/embeddings",
        200,
        &crate::types::CreateEmbeddingResponse {
            object: crate::types::CreateEmbeddingResponseObject::List,
            model: "text-embedding-ada-002".into(),
            data: vec![crate::types::Embedding {
                object: crate::types::EmbeddingObject::Embedding,
                index: 0,
                embedding: vec![0.0023, -0.0094, 0.0151],
            }],
            usage: crate::types::CreateEmbeddingResponseUsage {
                prompt_tokens: 8,
                total_tokens: 8,
            },
        },
    );

    let body = crate::types::CreateEmbeddingRequest::builder()
        .input("Hello world".to_string())
        .model("text-embedding-ada-002".to_string())
        .build()
        .unwrap();

    let resp = OpenAIApi::create_embedding(&client, body).await.unwrap();
    assert_eq!(resp.model, "text-embedding-ada-002");
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].index, 0);
    assert_eq!(resp.data[0].embedding.len(), 3);
    assert_eq!(resp.usage.prompt_tokens, 8);

    mock.assert_async().await;
}

// ── HTTP Error Handling ────────────────────────────────────────────────

#[tokio::test]
async fn test_http_error_returns_status() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let _mock = mock_get(
        &mut server,
        "/models",
        401,
        &serde_json::json!({"error": {"message": "Invalid API key", "type": "invalid_request_error"}}),
    );

    let err = OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED)
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_not_found_error() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let _mock = mock_get(
        &mut server,
        "/models",
        404,
        &serde_json::json!({"error": "not found"}),
    );

    let err = OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::NOT_FOUND)
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

// ── Auth / API Key ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_with_api_key_sends_bearer_token() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url()).with_api_key("sk-test-key".into());

    let mock = server
        .mock("GET", "/models")
        .match_header("Authorization", "Bearer sk-test-key")
        .with_status(200)
        .with_body(
            serde_json::to_string(&crate::types::ListModelsResponse {
                object: crate::types::ListModelsResponseObject::List,
                data: vec![crate::types::Model {
                    id: "gpt-4o".to_string(),
                    object: crate::types::ModelObject::Model,
                    created: 1661989079,
                    owned_by: "openai".to_string(),
                }],
            })
            .unwrap(),
        )
        .create();

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_without_api_key_omits_auth_header() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());

    let mock = server
        .mock("GET", "/models")
        .match_header("Authorization", mockito::Matcher::Missing)
        .with_status(200)
        .with_body(
            serde_json::to_string(&crate::types::ListModelsResponse {
                object: crate::types::ListModelsResponseObject::List,
                data: vec![crate::types::Model {
                    id: "gpt-4o".to_string(),
                    object: crate::types::ModelObject::Model,
                    created: 1661989079,
                    owned_by: "openai".to_string(),
                }],
            })
            .unwrap(),
        )
        .create();

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    mock.assert_async().await;
}
