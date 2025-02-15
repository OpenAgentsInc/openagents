use futures_util::StreamExt;
use openagents::server::services::{
    gateway::Gateway,
    openrouter::{OpenRouterConfig, OpenRouterService},
};
use std::{collections::HashSet, time::Duration};
use tokio::time::timeout;

#[tokio::test]
async fn test_openrouter_metadata() {
    let config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };

    let service = OpenRouterService::with_config("test-key".to_string(), config);
    let metadata = service.metadata();

    assert_eq!(metadata.name, "OpenRouter");
    assert!(metadata.supported_features.contains(&"chat".to_string()));
}

#[tokio::test]
async fn test_openrouter_chat() {
    let config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };

    let service = OpenRouterService::with_config("test-key".to_string(), config);
    let (response, _) = service
        .chat("Test prompt".to_string(), false)
        .await
        .unwrap();

    assert_eq!(response, "Test response");
}

#[tokio::test]
async fn test_openrouter_chat_stream() {
    let config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };

    let service = OpenRouterService::with_config("test-key".to_string(), config);
    let mut stream = service
        .chat_stream("Test prompt".to_string(), false)
        .await
        .unwrap();

    // Set a timeout for the test
    let timeout_duration = Duration::from_secs(5);
    let result = timeout(timeout_duration, async {
        let mut response = String::new();
        while let Some(chunk) = stream.next().await {
            response.push_str(&chunk.unwrap());
        }
        response
    })
    .await
    .unwrap();

    assert_eq!(result, "Test response");
}

#[tokio::test]
async fn test_openrouter_with_config() {
    let config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: true,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };

    let service = OpenRouterService::with_config("test-key".to_string(), config);
    let (response, _) = service.chat("Test prompt".to_string(), true).await.unwrap();

    assert_eq!(response, "Test response");
}

#[tokio::test]
async fn test_openrouter_error_handling() {
    let config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: false, // Not in test mode to test error handling
        rate_limited_models: HashSet::new(),
    };

    let service = OpenRouterService::with_config("invalid-key".to_string(), config);
    let result = service.chat("Test prompt".to_string(), false).await;

    assert!(result.is_err());
}
