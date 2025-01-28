use openagents::server::services::{
    Gateway, GatewayMetadata, OpenRouterService, StreamUpdate,
};

#[tokio::test]
async fn test_openrouter_metadata() {
    let service = OpenRouterService::new("test-key".to_string());
    let metadata = service.metadata();
    
    assert_eq!(metadata.name, "OpenRouter");
    assert!(metadata.openai_compatible);
    assert!(metadata.supported_features.contains(&"chat".to_string()));
    assert!(metadata.supported_features.contains(&"streaming".to_string()));
}

#[tokio::test]
async fn test_openrouter_chat() {
    let service = OpenRouterService::new("test-key".to_string());
    let result = service.chat("test prompt".to_string(), false).await.unwrap();
    
    assert_eq!(result.0, "test prompt");
    assert!(result.1.is_none());

    // Test with reasoner
    let result = service.chat("test prompt".to_string(), true).await.unwrap();
    assert_eq!(result.0, "test prompt");
    assert_eq!(result.1, Some("Reasoning".to_string()));
}

#[tokio::test]
async fn test_openrouter_stream() {
    let service = OpenRouterService::new("test-key".to_string());
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