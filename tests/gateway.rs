use openagents::server::services::{
    Gateway, OpenRouterService, StreamUpdate,
    openrouter::types::OpenRouterConfig,
};
use std::env;

fn setup() {
    dotenvy::dotenv().ok();
    env::set_var("OPENROUTER_TEST_MODE", "1");
}

#[tokio::test]
async fn test_openrouter_metadata() {
    setup();
    let service = OpenRouterService::new().unwrap();
    let metadata = service.metadata();
    
    assert_eq!(metadata.name, "OpenRouter");
    assert!(metadata.openai_compatible);
    assert!(metadata.supported_features.contains(&"chat".to_string()));
    assert!(metadata.supported_features.contains(&"streaming".to_string()));
}

#[tokio::test]
async fn test_openrouter_chat() {
    setup();
    let mut service = OpenRouterService::new().unwrap();
    let result = service.chat("test prompt".to_string(), false).await.unwrap();
    
    assert_eq!(result.0, "test prompt");
    assert!(result.1.is_none());

    // Test with reasoner
    let result = service.chat("test prompt".to_string(), true).await.unwrap();
    assert_eq!(result.0, "test prompt");
    assert_eq!(result.1, None);
}

#[tokio::test]
async fn test_openrouter_stream() {
    setup();
    let mut service = OpenRouterService::new().unwrap();
    let mut stream = service.chat_stream("test prompt".to_string(), true).await;
    
    // Test content
    if let Some(StreamUpdate::Content(content)) = stream.recv().await {
        assert_eq!(content, "test prompt");
    } else {
        panic!("Expected content update");
    }

    // Test reasoning
    if let Some(StreamUpdate::Reasoning(reasoning)) = stream.recv().await {
        assert_eq!(reasoning, "Test reasoning");
    } else {
        panic!("Expected reasoning update");
    }

    // Test done
    if let Some(StreamUpdate::Done) = stream.recv().await {
        // Success
    } else {
        panic!("Expected done update");
    }
}

#[tokio::test]
async fn test_openrouter_with_config() {
    setup();
    let config = OpenRouterConfig {
        temperature: 0.5,
        max_tokens: Some(100),
        top_p: Some(0.9),
        frequency_penalty: Some(0.0),
        presence_penalty: Some(0.0),
        stop: Some(vec!["STOP".to_string()]),
    };

    let mut service = OpenRouterService::with_config(config).unwrap();
    let result = service.chat("test prompt".to_string(), false).await.unwrap();
    assert_eq!(result.0, "test prompt");
}

#[tokio::test]
async fn test_openrouter_conversation() {
    setup();
    let mut service = OpenRouterService::new().unwrap();
    
    // First message
    let result = service.chat("Hello".to_string(), false).await.unwrap();
    assert_eq!(result.0, "Hello");

    // Second message should include conversation history
    let result = service.chat("How are you?".to_string(), false).await.unwrap();
    assert_eq!(result.0, "How are you?");
}