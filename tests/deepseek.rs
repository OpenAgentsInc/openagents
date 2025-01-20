use anyhow::Result;
use mockito::mock;
use openagents::server::services::deepseek::DeepSeekService;
use serde_json::json;

#[tokio::test]
async fn test_chat_basic() {
    let mut server = mockito::Server::new();
    let mock_response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Hello! How can I help you?",
                "reasoning_content": null
            },
            "finish_reason": "stop"
        }]
    });

    let _m = server.mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response.to_string())
        .create();

    let service = DeepSeekService::with_base_url(
        "test_key".to_string(),
        server.url(),
    );

    let (response, reasoning) = service.chat("Hello".to_string(), false).await.unwrap();
    assert_eq!(response, "Hello! How can I help you?");
    assert_eq!(reasoning, None);
}

#[tokio::test]
async fn test_chat_with_reasoning() {
    let mut server = mockito::Server::new();
    let mock_response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "9.11 is greater than 9.8",
                "reasoning_content": "Let's compare these numbers:\n9.11 vs 9.8\n9.11 = 9 + 0.11\n9.8 = 9 + 0.8\n0.8 is greater than 0.11\nTherefore, 9.8 is greater than 9.11"
            },
            "finish_reason": "stop"
        }]
    });

    let _m = server.mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response.to_string())
        .create();

    let service = DeepSeekService::with_base_url(
        "test_key".to_string(),
        server.url(),
    );

    let (response, reasoning) = service.chat("Compare 9.11 and 9.8".to_string(), true).await.unwrap();
    assert_eq!(response, "9.11 is greater than 9.8");
    assert!(reasoning.is_some());
    assert!(reasoning.unwrap().contains("Let's compare these numbers"));
}