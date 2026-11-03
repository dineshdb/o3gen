use crate::test_helpers::mock::MockServer;
use crate::{
    Categories, CategoryScores, ChatCompletionRequest, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageRole,
    ChatCompletionResponseMessage, ChatCompletionResponseMessageRole, CompletionUsage,
    CreateChatCompletionResponse, CreateChatCompletionResponseChoices,
    CreateChatCompletionResponseChoicesFinishReason, CreateChatCompletionResponseObject,
    CreateCompletionRequest, CreateCompletionResponse, CreateCompletionResponseChoices,
    CreateCompletionResponseChoicesFinishReason, CreateCompletionResponseObject,
    CreateEmbeddingRequest, CreateEmbeddingResponse, CreateEmbeddingResponseObject,
    CreateEmbeddingResponseUsage, CreateModerationRequest, CreateModerationRequestInput,
    CreateModerationResponse, CreateModerationResponseResults, DeleteModelResponse, Embedding,
    EmbeddingObject, ListModelsResponse, ListModelsResponseObject, Model, ModelId, ModelObject,
    OpenAIApi, Prompt,
};

#[tokio::test]
async fn test_list_models() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_get(
        "/models",
        200,
        &ListModelsResponse {
            object: ListModelsResponseObject::List,
            data: vec![
                Model {
                    id: "gpt-4o".to_string(),
                    object: ModelObject::Model,
                    created: 1_661_989_079,
                    owned_by: "openai".to_string(),
                },
                Model {
                    id: "gpt-3.5-turbo".to_string(),
                    object: ModelObject::Model,
                    created: 1_677_610_605,
                    owned_by: "openai".to_string(),
                },
            ],
        },
    );

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 2);
    assert_eq!(resp.data[0].id, "gpt-4o");
    assert_eq!(resp.data[1].id, "gpt-3.5-turbo");

    m.assert_async().await;
}

#[tokio::test]
async fn test_list_models_empty() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_get(
        "/models",
        200,
        &ListModelsResponse {
            object: ListModelsResponseObject::List,
            data: vec![],
        },
    );

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert!(resp.data.is_empty());

    m.assert_async().await;
}

#[tokio::test]
async fn test_retrieve_model() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_get(
        "/models/gpt-4o",
        200,
        &Model {
            id: "gpt-4o".to_string(),
            object: ModelObject::Model,
            created: 1_661_989_079,
            owned_by: "openai".to_string(),
        },
    );

    let resp = OpenAIApi::retrieve_model(&client, "gpt-4o".into())
        .await
        .unwrap();
    assert_eq!(resp.id, "gpt-4o");
    assert_eq!(resp.owned_by, "openai");

    m.assert_async().await;
}

#[tokio::test]
async fn test_delete_model() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_delete(
        "/models/ft-gpt-3.5-turbo",
        200,
        &DeleteModelResponse {
            id: "ft-gpt-3.5-turbo".to_string(),
            deleted: true,
            object: "model".to_string(),
        },
    );

    let resp = OpenAIApi::delete_model(&client, "ft-gpt-3.5-turbo".into())
        .await
        .unwrap();
    assert!(resp.deleted);
    assert_eq!(resp.id, "ft-gpt-3.5-turbo");

    m.assert_async().await;
}

// ── Create Chat Completion ─────────────────────────────────────────────

#[tokio::test]
async fn test_create_chat_completion() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_post(
        "/chat/completions",
        200,
        &CreateChatCompletionResponse {
            id: "chatcmpl-abc123".into(),
            object: CreateChatCompletionResponseObject::ChatCompletion,
            created: 1_677_610_605,
            model: "gpt-3.5-turbo".into(),
            system_fingerprint: None,
            choices: vec![CreateChatCompletionResponseChoices {
                index: 0,
                finish_reason: CreateChatCompletionResponseChoicesFinishReason::Stop,
                message: ChatCompletionResponseMessage {
                    role: ChatCompletionResponseMessageRole::Assistant,
                    content: Some("Hello! How can I help you today?".into()),
                    tool_calls: None,
                    function_call: None,
                },
            }],
            usage: Some(CompletionUsage {
                prompt_tokens: 9,
                completion_tokens: 12,
                total_tokens: 21,
            }),
        },
    );

    let body = ChatCompletionRequest::builder()
        .model("gpt-3.5-turbo".to_string())
        .messages(vec![
            ChatCompletionRequestUserMessage::builder()
                .role(ChatCompletionRequestUserMessageRole::User)
                .content(ChatCompletionRequestUserMessageContent::String(
                    "Hello".to_string(),
                ))
                .build()
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

    m.assert_async().await;
}

#[tokio::test]
async fn test_create_embedding() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_post(
        "/embeddings",
        200,
        &CreateEmbeddingResponse {
            object: CreateEmbeddingResponseObject::List,
            model: "text-embedding-ada-002".into(),
            data: vec![Embedding {
                object: EmbeddingObject::Embedding,
                index: 0,
                embedding: vec![0.0023, -0.0094, 0.0151],
            }],
            usage: CreateEmbeddingResponseUsage {
                prompt_tokens: 8,
                total_tokens: 8,
            },
        },
    );

    let body = CreateEmbeddingRequest::builder()
        .input("Hello world".to_string())
        .model("text-embedding-ada-002".to_string())
        .build();

    let resp = OpenAIApi::create_embedding(&client, body).await.unwrap();
    assert_eq!(resp.model, "text-embedding-ada-002");
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].index, 0);
    assert_eq!(resp.data[0].embedding.len(), 3);
    assert_eq!(resp.usage.prompt_tokens, 8);

    m.assert_async().await;
}

#[tokio::test]
async fn test_create_completion() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_post(
        "/completions",
        200,
        &CreateCompletionResponse {
            id: "cmpl-abc123".into(),
            object: CreateCompletionResponseObject::TextCompletion,
            created: 1_677_610_605,
            model: "gpt-3.5-turbo-instruct".into(),
            system_fingerprint: None,
            choices: vec![CreateCompletionResponseChoices {
                index: 0,
                text: "The answer is 42.".into(),
                logprobs: None,
                finish_reason: CreateCompletionResponseChoicesFinishReason::Stop,
            }],
            usage: Some(CompletionUsage {
                prompt_tokens: 5,
                completion_tokens: 7,
                total_tokens: 12,
            }),
        },
    );

    let body = CreateCompletionRequest::builder()
        .model(ModelId::Gpt35TurboInstruct)
        .prompt(Prompt::String("What is the answer?".into()))
        .build()
        .unwrap();

    let resp = OpenAIApi::create_completion(&client, body).await.unwrap();
    assert_eq!(resp.id, "cmpl-abc123");
    assert_eq!(resp.choices.len(), 1);
    assert_eq!(resp.choices[0].text, "The answer is 42.");

    m.assert_async().await;
}

#[tokio::test]
async fn test_create_moderation() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let m = mock.mock_post(
        "/moderations",
        200,
        &CreateModerationResponse {
            id: "modr-abc123".into(),
            model: "text-moderation-006".into(),
            results: vec![CreateModerationResponseResults {
                flagged: false,
                categories: Categories {
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
                },
                category_scores: CategoryScores {
                    hate: 0.0001,
                    hate_threatening: 0.00001,
                    harassment: 0.0002,
                    harassment_threatening: 0.00002,
                    self_harm: 0.000001,
                    self_harm_intent: 0.000001,
                    self_harm_instructions: 0.000001,
                    sexual: 0.0003,
                    sexual_minors: 0.00001,
                    violence: 0.0004,
                    violence_graphic: 0.0001,
                },
            }],
        },
    );

    let body = CreateModerationRequest::builder()
        .input(CreateModerationRequestInput::String(
            "I want to hurt myself".into(),
        ))
        .build();

    let resp = OpenAIApi::create_moderation(&client, body).await.unwrap();
    assert_eq!(resp.id, "modr-abc123");
    assert!(!resp.results[0].flagged);

    m.assert_async().await;
}

// ── HTTP Error Handling ────────────────────────────────────────────────

#[tokio::test]
async fn test_http_unauthorized() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let _m = mock.mock_get(
        "/models",
        401,
        &serde_json::json!({"error": {"message": "Invalid API key", "type": "invalid_request_error"}}),
    );

    let err = OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, .. } => {
            assert_eq!(status, 401)
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_http_rate_limited() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let _m = mock.mock_get(
        "/models",
        429,
        &serde_json::json!({"error": {"message": "Rate limit exceeded", "type": "rate_limit_error"}}),
    );

    let err = OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, .. } => {
            assert_eq!(status, 429)
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_http_not_found() {
    let mut mock = MockServer::new().await;
    let client = mock.client();
    let _m = mock.mock_get("/models", 404, &serde_json::json!({"error": "not found"}));

    let err = OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, .. } => {
            assert_eq!(status, 404)
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

// ── Auth / API Key ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_with_api_key_sends_bearer_token() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test-key");

    let m = mock
        .server
        .mock("GET", "/models")
        .match_header("Authorization", "Bearer sk-test-key")
        .with_status(200)
        .with_body(
            serde_json::to_string(&ListModelsResponse {
                object: ListModelsResponseObject::List,
                data: vec![Model {
                    id: "gpt-4o".to_string(),
                    object: ModelObject::Model,
                    created: 1_661_989_079,
                    owned_by: "openai".to_string(),
                }],
            })
            .unwrap(),
        )
        .create();

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    m.assert_async().await;
}

#[tokio::test]
async fn test_with_api_key_sends_bearer_on_stream() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test-key");

    let m = mock
        .server
        .mock("POST", "/chat/completions")
        .match_header("Authorization", "Bearer sk-test-key")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n\n")
        .create();

    let body = ChatCompletionRequest::builder()
        .model("gpt-4o".to_string())
        .messages(vec![
            ChatCompletionRequestUserMessage::builder()
                .role(ChatCompletionRequestUserMessageRole::User)
                .content(ChatCompletionRequestUserMessageContent::String(
                    "Hello".to_string(),
                ))
                .build()
                .into(),
        ])
        .build()
        .unwrap();

    let _stream = client.stream_chat(body).await.unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn test_without_api_key_omits_auth_header() {
    let mut mock = MockServer::new().await;
    let client = mock.client();

    let m = mock
        .server
        .mock("GET", "/models")
        .match_header("Authorization", mockito::Matcher::Missing)
        .with_status(200)
        .with_body(
            serde_json::to_string(&ListModelsResponse {
                object: ListModelsResponseObject::List,
                data: vec![Model {
                    id: "gpt-4o".to_string(),
                    object: ModelObject::Model,
                    created: 1_661_989_079,
                    owned_by: "openai".to_string(),
                }],
            })
            .unwrap(),
        )
        .create();

    let resp = OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    m.assert_async().await;
}

// ── JSON extraction ────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct Person {
    name: String,
    age: u8,
    city: String,
}

fn make_chat_request() -> ChatCompletionRequest {
    ChatCompletionRequest::builder()
        .model("gpt-4o".to_string())
        .messages(vec![
            ChatCompletionRequestUserMessage::builder()
                .role(ChatCompletionRequestUserMessageRole::User)
                .content(ChatCompletionRequestUserMessageContent::String(
                    "John is 30 and lives in NYC".into(),
                ))
                .build()
                .into(),
        ])
        .build()
        .unwrap()
}

fn make_chat_response(content: Option<&str>) -> CreateChatCompletionResponse {
    CreateChatCompletionResponse {
        id: "chatcmpl-abc123".into(),
        object: CreateChatCompletionResponseObject::ChatCompletion,
        created: 1_677_610_605,
        model: "gpt-4o".into(),
        system_fingerprint: None,
        choices: vec![CreateChatCompletionResponseChoices {
            index: 0,
            finish_reason: CreateChatCompletionResponseChoicesFinishReason::Stop,
            message: ChatCompletionResponseMessage {
                role: ChatCompletionResponseMessageRole::Assistant,
                content: content.map(String::from),
                tool_calls: None,
                function_call: None,
            },
        }],
        usage: Some(CompletionUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        }),
    }
}

#[tokio::test]
async fn test_json_extraction() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test");

    let resp_body = make_chat_response(Some(r#"{"name":"John","age":30,"city":"NYC"}"#));
    let m = mock.mock_post("/chat/completions", 200, &resp_body);

    let person: Person = client.json(make_chat_request()).await.unwrap();
    assert_eq!(person.name, "John");
    assert_eq!(person.age, 30);
    assert_eq!(person.city, "NYC");

    m.assert_async().await;
}

#[tokio::test]
async fn test_json_extraction_no_content() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test");

    let m = mock.mock_post("/chat/completions", 200, &make_chat_response(None));

    let err = client
        .json::<Person>(make_chat_request())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, crate::ApiError::Builder(msg) if msg == "No content in response"),
        "expected Builder error, got {err:?}",
    );

    m.assert_async().await;
}

#[tokio::test]
async fn test_json_extraction_array() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test");

    let resp_body = make_chat_response(Some(
        r#"[{"name":"John","age":30,"city":"NYC"},{"name":"Sarah","age":25,"city":"Boston"}]"#,
    ));
    let m = mock.mock_post("/chat/completions", 200, &resp_body);

    let people: Vec<Person> = client.json(make_chat_request()).await.unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!(people[0].name, "John");
    assert_eq!(people[0].age, 30);
    assert_eq!(people[1].name, "Sarah");
    assert_eq!(people[1].city, "Boston");

    m.assert_async().await;
}

#[tokio::test]
async fn test_json_extraction_invalid_json() {
    let mut mock = MockServer::new().await;
    let client = mock.client_with_key("sk-test");

    let resp_body = make_chat_response(Some(r#"{invalid}"#));
    let m = mock.mock_post("/chat/completions", 200, &resp_body);

    let err = client
        .json::<Person>(make_chat_request())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, crate::ApiError::Serde(_)),
        "expected Serde error, got {err:?}",
    );

    m.assert_async().await;
}

// ── the injected Transport path (no reqwest in the call at all) ──────

struct FixedTransport {
    status: u16,
    body: Vec<u8>,
    seen: std::sync::Mutex<Option<crate::TransportRequest>>,
}

struct FixedBody(Vec<u8>);

impl crate::TransportBody for FixedBody {
    fn next_chunk(&mut self) -> crate::TransportFuture<Option<Vec<u8>>> {
        let chunk = std::mem::take(&mut self.0);
        Box::pin(async move { Ok::<_, String>(if chunk.is_empty() { None } else { Some(chunk) }) })
    }
}

impl crate::Transport for FixedTransport {
    fn execute_streaming(
        &self,
        request: crate::TransportRequest,
    ) -> crate::TransportFuture<crate::TransportStreamResponse> {
        *self.seen.lock().unwrap() = Some(request);
        let status = self.status;
        let body = self.body.clone();
        Box::pin(async move {
            Ok(crate::TransportStreamResponse {
                status,
                headers: Vec::new(),
                body: Box::new(FixedBody(body)),
            })
        })
    }
}

#[tokio::test]
async fn test_transport_injected_call() {
    let transport = std::sync::Arc::new(FixedTransport {
        status: 200,
        body: br#"{"object":"list","data":[{"id":"gpt-4o","object":"model","created":1700000000,"owned_by":"openai"}]}"#.to_vec(),
        seen: std::sync::Mutex::new(None),
    });
    let client = crate::OpenAIApiClient::new("http://transport.test/v1".into())
        .with_api_key("sk-test".into())
        .with_transport(transport.clone());

    let resp = crate::OpenAIApi::list_models(&client).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "gpt-4o");

    let seen = transport
        .seen
        .lock()
        .unwrap()
        .take()
        .expect("no request seen");
    assert_eq!(seen.method, "GET");
    assert_eq!(seen.url, "http://transport.test/v1/models");
    assert!(
        seen.headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"),
        "auth header must ride the transport request: {seen:?}"
    );
}

#[tokio::test]
async fn test_transport_status_error() {
    let transport = std::sync::Arc::new(FixedTransport {
        status: 401,
        body: br#"{"error":{"message":"bad key"}}"#.to_vec(),
        seen: std::sync::Mutex::new(None),
    });
    let client =
        crate::OpenAIApiClient::new("http://transport.test/v1".into()).with_transport(transport);

    let err = crate::OpenAIApi::list_models(&client).await.unwrap_err();
    match err {
        crate::ApiError::Status { status, body } => {
            assert_eq!(status, 401);
            assert!(body.contains("bad key"));
        }
        other => panic!("expected Status error, got: {other:?}"),
    }
}

// ── the streaming face (execute_streaming) ───────────────────────────

struct ChunkedTransport {
    status: u16,
    chunks: std::collections::VecDeque<Vec<u8>>,
}

impl crate::Transport for ChunkedTransport {
    fn execute_streaming(
        &self,
        _request: crate::TransportRequest,
    ) -> crate::TransportFuture<crate::TransportStreamResponse> {
        let status = self.status;
        let chunks = self.chunks.clone();
        Box::pin(async move {
            struct ChunkedBody(std::collections::VecDeque<Vec<u8>>);
            impl crate::TransportBody for ChunkedBody {
                fn next_chunk(&mut self) -> crate::TransportFuture<Option<Vec<u8>>> {
                    let chunk = self.0.pop_front();
                    Box::pin(async move { Ok::<_, String>(chunk) })
                }
            }
            Ok(crate::TransportStreamResponse {
                status,
                headers: vec![("content-type".to_string(), "text/event-stream".to_string())],
                body: Box::new(ChunkedBody(chunks)),
            })
        })
    }
}

#[tokio::test]
async fn test_transport_streaming_chunks() {
    let transport = ChunkedTransport {
        status: 200,
        chunks: [
            b"data: {\"a\":1}".to_vec(),
            b"\n\ndata: ".to_vec(),
            b"{\"a\":2}\n\n".to_vec(),
        ]
        .into(),
    };
    let mut response = crate::Transport::execute_streaming(
        &transport,
        crate::TransportRequest {
            method: "POST".into(),
            url: "http://transport.test/v1/chat/completions".into(),
            headers: vec![],
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(
        response
            .headers
            .iter()
            .find(|(k, _)| k == "content-type")
            .map(|(_, v)| v.as_str()),
        Some("text/event-stream")
    );
    let mut assembled = Vec::new();
    let mut chunks = 0;
    while let Some(chunk) = response.body.next_chunk().await.unwrap() {
        chunks += 1;
        assembled.extend_from_slice(&chunk);
    }
    assert_eq!(chunks, 3, "each upstream chunk answers as its own chunk");
    assert_eq!(assembled, b"data: {\"a\":1}\n\ndata: {\"a\":2}\n\n");
}
