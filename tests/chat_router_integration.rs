use std::sync::Arc;

use openagents::server::{
    services::{
        deepseek::DeepSeekService,
        github_issue::GitHubService,
        model_router::ModelRouter,
    },
    tools::create_tools,
    ws::{
        handlers::{chat::ChatHandler, MessageHandler},
        transport::WebSocketState,
    },
};
use tokio::sync::broadcast;
use tracing::info;

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

fn create_test_tools() -> Vec<serde_json::Value> {
    create_tools()
}

#[tokio::test]
async fn test_chat_router_integration() {
    init_logging();

    // Create test services
    let tool_model = Arc::new(DeepSeekService::new("test_key".to_string()));
    let chat_model = Arc::new(DeepSeekService::new("test_key".to_string()));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string())).expect("Failed to create GitHub service"),
    );

    // Create tools
    let tools = create_test_tools();

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
    let msg = serde_json::json!({
        "type": "chat",
        "content": "Hello, world!",
        "conversation_id": "test_conv"
    });

    chat_handler.handle_message("test_conn", msg).await;

    // Check response
    if let Ok(response) = rx.try_recv() {
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["type"], "chat");
        assert!(response["content"].is_string());
    } else {
        panic!("No response received");
    }
}

#[tokio::test]
async fn test_chat_router_streaming() {
    init_logging();

    // Create test services
    let tool_model = Arc::new(DeepSeekService::new("test_key".to_string()));
    let chat_model = Arc::new(DeepSeekService::new("test_key".to_string()));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string())).expect("Failed to create GitHub service"),
    );

    // Create tools
    let tools = create_test_tools();

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
    let msg = serde_json::json!({
        "type": "chat",
        "content": "Stream this response",
        "conversation_id": "test_conv",
        "stream": true
    });

    chat_handler.handle_message("test_conn", msg).await;

    // Check streaming responses
    let mut responses = Vec::new();
    while let Ok(response) = rx.try_recv() {
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["type"], "chat");
        responses.push(response);
    }

    assert!(!responses.is_empty(), "No streaming responses received");
}