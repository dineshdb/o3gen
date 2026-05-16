use crate::types::CreateModerationRequestModel;
use crate::{
    CreateChatCompletionRequestModel, CreateEmbeddingRequestModel, OpenAIApi, OpenAIApiClient,
};
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

    let body = crate::types::CreateChatCompletionRequest::builder()
        .model(CreateChatCompletionRequestModel::String(
            "gpt-3.5-turbo".to_string(),
        ))
        .messages(vec![
            crate::types::ChatCompletionRequestMessage::UserMessage(
                crate::types::ChatCompletionRequestUserMessage::builder()
                    .role(crate::types::ChatCompletionRequestUserMessageRole::User)
                    .content(
                        crate::types::ChatCompletionRequestUserMessageContent::String(
                            "Hello".to_string(),
                        ),
                    )
                    .build()
                    .unwrap(),
            ),
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

// ── Create Moderation ──────────────────────────────────────────────────

#[tokio::test]
async fn test_create_moderation() {
    let mut server = mockito::Server::new_async().await;
    let client = OpenAIApiClient::new(server.url());
    let no_categories = crate::types::Categories {
        hate: false,
        hate_threatening: false,
        harassment: false,
        harassment_threatening: false,
        self_harm: false,
        self_harm_intent: false,
        self_harm_instructions: false,
        sexual: false,
        sexual_minors: false,
        violence: false,
        violence_graphic: false,
    };
    let zero_scores = crate::types::CategoryScores {
        hate: 0.0,
        hate_threatening: 0.0,
        harassment: 0.0,
        harassment_threatening: 0.0,
        self_harm: 0.0,
        self_harm_intent: 0.0,
        self_harm_instructions: 0.0,
        sexual: 0.0,
        sexual_minors: 0.0,
        violence: 0.0,
        violence_graphic: 0.0,
    };
    let mock = mock_post(
        &mut server,
        "/moderations",
        200,
        &crate::types::CreateModerationResponse {
            id: "modr-abc123".into(),
            model: "text-moderation-stable".into(),
            results: vec![crate::types::CreateModerationResponseResults {
                flagged: false,
                categories: no_categories,
                category_scores: zero_scores,
            }],
        },
    );

    let body = crate::types::CreateModerationRequest::builder()
        .input(crate::types::CreateModerationRequestInput::String(
            "I want to kill them.".to_string(),
        ))
        .model(CreateModerationRequestModel::TextModeration(
            crate::types::TextModeration::TextModerationStable,
        ))
        .build()
        .unwrap();

    let resp = OpenAIApi::create_moderation(&client, body).await.unwrap();
    assert_eq!(resp.id, "modr-abc123");
    assert_eq!(resp.results.len(), 1);
    assert!(!resp.results[0].flagged);
    assert!(!resp.results[0].categories.hate);
    assert!(!resp.results[0].categories.violence);

    mock.assert_async().await;
}

// ── Create Embedding ───────────────────────────────────────────────────

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
        .input(crate::types::CreateEmbeddingRequestInput::String(
            "Hello world".to_string(),
        ))
        .model(CreateEmbeddingRequestModel::String(
            "text-embedding-ada-002".to_string(),
        ))
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
