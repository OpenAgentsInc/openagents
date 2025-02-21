use openagents::server::services::gateway::Gateway;
use openagents::server::services::groq::GroqService;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_groq_metadata() {
    let service = GroqService::new("test-key".to_string());
    let metadata = service.metadata();
    assert_eq!(metadata.name, "Groq");
    assert!(metadata.openai_compatible);
    assert!(metadata.supported_features.contains(&"chat".to_string()));
    assert!(metadata.supported_features.contains(&"streaming".to_string()));
    assert!(metadata.supported_features.contains(&"reasoning".to_string()));
}

#[tokio::test]
async fn test_groq_chat() {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY must be set");
    let mut service = GroqService::new(api_key);

    // Override default model for tests
    service.set_model("deepseek-r1-distill-qwen-32b".to_string());

    // Test without reasoning
    let (response, reasoning) = service.chat("Say hello".to_string(), false).await.unwrap();
    assert!(!response.is_empty());
    assert!(reasoning.is_none());

    // Test with reasoning
    let (response, reasoning) = service.chat("What is 2+2 and why?".to_string(), true).await.unwrap();
    assert!(!response.is_empty());
    assert!(reasoning.is_some());
    assert!(!reasoning.unwrap().is_empty());
}

#[tokio::test]
async fn test_groq_chat_stream() {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY must be set");
    let mut service = GroqService::new(api_key);

    // Override default model for tests
    service.set_model("deepseek-r1-distill-qwen-32b".to_string());

    // Test streaming without reasoning
    let mut stream = service
        .chat_stream("Say hello".to_string(), false)
        .await
        .unwrap();

    let mut response = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        response.push_str(&chunk);
    }
    assert!(!response.is_empty());
    assert!(!response.contains("Reasoning:"));

    // Test streaming with reasoning
    let mut stream = service
        .chat_stream("What is 2+2 and why?".to_string(), true)
        .await
        .unwrap();

    let mut response = String::new();
    let mut has_reasoning = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        response.push_str(&chunk);
        if chunk.contains("Reasoning:") {
            has_reasoning = true;
        }
    }
    assert!(!response.is_empty());
    assert!(has_reasoning);
}

#[tokio::test]
async fn test_groq_error_handling() {
    let service = GroqService::new("invalid-key".to_string());
    let result = service.chat("Test message".to_string(), false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_groq_with_base_url() {
    let service = GroqService::with_base_url(
        "test-key".to_string(),
        "https://api.groq.example.com/v1".to_string(),
    );
    assert_eq!(service.metadata().name, "Groq");
}