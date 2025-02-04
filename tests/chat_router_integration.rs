use std::sync::Arc;

use axum::extract::ws::Message;
use openagents::server::{
    services::{
        deepseek::DeepSeekService,
        github_issue::GitHubService,
    },
    tools::create_tools,
    ws::{
        handlers::{chat::ChatHandler, MessageHandler},
        types::ChatMessage,
        transport::WebSocketState,
    },
};
use serde_json::json;
use tracing_subscriber;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .try_init();
}

#[tokio::test]
async fn test_chat_router_integration() {
    init_logging();

    // Start mock DeepSeek API server
    let mock_server = MockServer::start().await;

    // Mock the DeepSeek API response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test_response",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "deepseek-chat",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    // Create test services with mock server URL
    let tool_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string())).expect("Failed to create GitHub service"),
    );

    // Create tools
    let tools = create_tools();

    // Create WebSocket state
    let ws_state = WebSocketState::new(
        tool_model.clone(),
        chat_model.clone(),
        github_service.clone(),
        tools,
    );
    let ws_state = Arc::new(ws_state);

    // Add test connection
    let mut rx = ws_state.add_test_connection("test_conn", 1).await;

    // Create chat handler
    let chat_handler = ChatHandler::new(ws_state.clone(), github_service.clone());

    // Test message handling
    let msg = ChatMessage::UserMessage {
        content: "Hello, world!".to_string(),
    };

    chat_handler.handle_message(msg, "test_conn".to_string()).await.unwrap();

    // Check response
    if let Ok(response) = rx.try_recv() {
        match response {
            Message::Text(text) => {
                let response: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(response["type"], "assistant");
                assert!(response["content"].is_string());
            }
            _ => panic!("Expected text message"),
        }
    } else {
        panic!("No response received");
    }
}

#[tokio::test]
async fn test_chat_router_streaming() {
    init_logging();

    // Start mock DeepSeek API server
    let mock_server = MockServer::start().await;

    // Mock the DeepSeek API streaming response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test_response",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "deepseek-chat",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "This is a streaming response"
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    // Create test services with mock server URL
    let tool_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string())).expect("Failed to create GitHub service"),
    );

    // Create tools
    let tools = create_tools();

    // Create WebSocket state
    let ws_state = WebSocketState::new(
        tool_model.clone(),
        chat_model.clone(),
        github_service.clone(),
        tools,
    );
    let ws_state = Arc::new(ws_state);

    // Add test connection
    let mut rx = ws_state.add_test_connection("test_conn", 1).await;

    // Create chat handler
    let chat_handler = ChatHandler::new(ws_state.clone(), github_service.clone());

    // Test streaming message
    let msg = ChatMessage::UserMessage {
        content: "Stream this response".to_string(),
    };

    chat_handler.handle_message(msg, "test_conn".to_string()).await.unwrap();

    // Check streaming responses
    let mut responses = Vec::new();
    while let Ok(response) = rx.try_recv() {
        match response {
            Message::Text(text) => {
                let response: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(response["type"], "assistant");
                responses.push(response);
            }
            _ => panic!("Expected text message"),
        }
    }

    assert!(!responses.is_empty(), "No streaming responses received");
}