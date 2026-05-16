use o3gen_openai::types::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageRole,
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageRole,
    CreateChatCompletionRequest, CreateChatCompletionRequestModel, CreateEmbeddingRequest,
    CreateEmbeddingRequestInput, CreateEmbeddingRequestModel,
};
use o3gen_openai::{OpenAIApi, OpenAIApiClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== o3gen-openai Example ===\n");

    // Connect to local server at localhost:1234
    let client = OpenAIApiClient::new("http://localhost:1234/v1".to_string());

    // List available models
    println!("1. Listing available models...");
    let models = OpenAIApi::list_models(&client).await?;
    println!("   Found {} models:\n", models.data.len());

    if models.data.is_empty() {
        println!("   No models found on the server. Please ensure models are loaded.");
        return Ok(());
    }

    for model in &models.data {
        println!("   - {}", model.id);
    }

    let selected_model = models.data[0].id.clone();
    println!("\n   Using model: {}", selected_model);

    println!("\n2. Creating a chat completion...");

    let body = CreateChatCompletionRequest::builder()
        .model(CreateChatCompletionRequestModel::String(
            selected_model.clone(),
        ))
        .messages(vec![ChatCompletionRequestMessage::UserMessage(
            ChatCompletionRequestUserMessage::builder()
                .role(ChatCompletionRequestUserMessageRole::User)
                .content(ChatCompletionRequestUserMessageContent::String(
                    "What is Rust programming language?".to_string(),
                ))
                .build()
                .unwrap(),
        )])
        .build()
        .unwrap();

    let response = OpenAIApi::create_chat_completion(&client, body).await?;
    println!("   Model: {}", response.model);
    let mut messages = vec![];
    if let Some(content) = &response.choices[0].message.content {
        println!("\n   Assistant:\n   {}\n", content);
        messages.push(ChatCompletionRequestMessage::AssistantMessage(
            ChatCompletionRequestAssistantMessage::builder()
                .role(ChatCompletionRequestAssistantMessageRole::Assistant)
                .content(content.clone())
                .build()
                .unwrap(),
        ));
    }

    println!("2b. Continuing the conversation...");
    let follow_up = ChatCompletionRequestMessage::UserMessage(
        ChatCompletionRequestUserMessage::builder()
            .role(ChatCompletionRequestUserMessageRole::User)
            .content(ChatCompletionRequestUserMessageContent::String(
                "Can you give me a simple 'Hello World' example in Rust?".to_string(),
            ))
            .build()
            .unwrap(),
    );

    let mut all_messages = vec![ChatCompletionRequestMessage::UserMessage(
        ChatCompletionRequestUserMessage::builder()
            .role(ChatCompletionRequestUserMessageRole::User)
            .content(ChatCompletionRequestUserMessageContent::String(
                "What is Rust programming language?".to_string(),
            ))
            .build()
            .unwrap(),
    )];
    all_messages.extend(messages);
    all_messages.push(follow_up);

    let body = CreateChatCompletionRequest::builder()
        .model(CreateChatCompletionRequestModel::String(
            selected_model.clone(),
        ))
        .messages(all_messages)
        .build()
        .unwrap();

    let response = OpenAIApi::create_chat_completion(&client, body).await?;
    if let Some(content) = &response.choices[0].message.content {
        println!("\n   Assistant:\n   {}\n", content);
    }

    println!("3. Creating an embedding...");

    let embedding_body = CreateEmbeddingRequest::builder()
        .input(CreateEmbeddingRequestInput::String(
            "The quick brown fox jumps over the lazy dog".to_string(),
        ))
        .model(CreateEmbeddingRequestModel::String(selected_model))
        .build()
        .unwrap();

    match OpenAIApi::create_embedding(&client, embedding_body).await {
        Ok(embedding_response) => {
            println!("   Model: {}", embedding_response.model);
            println!(
                "   Vector dimension: {}",
                embedding_response.data[0].embedding.len()
            );
            println!(
                "   First 5 values: {:?}",
                &embedding_response.data[0].embedding
                    [0..5.min(embedding_response.data[0].embedding.len())]
            );
        }
        Err(e) => {
            println!(
                "   Embedding failed (likely not supported by local server): {}",
                e
            );
        }
    }

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
