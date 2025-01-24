use dotenvy::dotenv;
use openagents::server::services::{
    deepseek::{DeepSeekService, Tool, ToolChoice},
    github_issue::GitHubService,
    model_router::ModelRouter,
};
use openagents::server::ws::{transport::WebSocketState, types::ChatMessage};
use openagents::server::ws::handlers::chat::ChatHandler;
use serde_json::json;
use std::{env, sync::Arc};
use tokio::sync::broadcast;
use tracing::{info, Level};
use tracing_subscriber;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

// Helper function to initialize logging once
fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .try_init();
}

// Helper function to create test tools
fn create_test_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository"
                    },
                    "issue_number": {
                        "type": "integer",
                        "description": "The issue number"
                    }
                },
                "required": ["owner", "repo", "issue_number"]
            }),
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        ),
    ]
}

#[tokio::test]
async fn test_chat_router_integration() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock server
    let mock_server = MockServer::start().await;

    // Mock routing decision response
    let routing_decision_response = json!({
        "choices": [{
            "message": {
                "content": json!({
                    "needs_tool": true,
                    "reasoning": "User is requesting to view a GitHub issue",
                    "suggested_tool": "read_github_issue"
                }).to_string(),
                "role": "assistant"
            }
        }]
    });

    // Mock tool execution response
    let tool_execution_response = json!({
        "choices": [{
            "message": {
                "content": "Let me fetch that GitHub issue for you.",
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {
                        "name": "read_github_issue",
                        "arguments": "{\"owner\":\"OpenAgentsInc\",\"repo\":\"openagents\",\"issue_number\":595}"
                    }
                }]
            }
        }]
    });

    // Set up mocks
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&routing_decision_response))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&tool_execution_response))
        .mount(&mock_server)
        .await;

    // Create services with mock server
    let tool_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let github_service = Arc::new(GitHubService::new("test_token".to_string()));

    // Create tools
    let tools = create_test_tools();

    // Create model router
    let router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));

    // Create WebSocket state
    let (tx, _rx) = broadcast::channel(100);
    let ws_state = Arc::new(WebSocketState::new(tx));

    // Create chat handler with router
    let chat_handler = ChatHandler::new(ws_state.clone(), router, github_service);

    // Test message that should trigger GitHub tool
    let test_message = ChatMessage::UserMessage {
        content: "Can you check issue #595?".to_string(),
    };

    // Process message
    let result = chat_handler
        .handle_message(test_message, "test_conn".to_string())
        .await;

    // Verify success
    assert!(result.is_ok(), "Message handling should succeed");

    // Test message that should use chat model
    let test_message = ChatMessage::UserMessage {
        content: "Hello, how are you?".to_string(),
    };

    // Process message
    let result = chat_handler
        .handle_message(test_message, "test_conn".to_string())
        .await;

    // Verify success
    assert!(result.is_ok(), "Message handling should succeed");
}

#[tokio::test]
async fn test_chat_router_streaming() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock server
    let mock_server = MockServer::start().await;

    // Mock streaming response
    let stream_response = json!({
        "choices": [{
            "message": {
                "content": "Hello! I'm doing well, thank you for asking.",
                "role": "assistant"
            }
        }]
    });

    // Set up mock
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&stream_response))
        .mount(&mock_server)
        .await;

    // Create services with mock server
    let tool_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        mock_server.uri(),
    ));
    let github_service = Arc::new(GitHubService::new("test_token".to_string()));

    // Create tools
    let tools = create_test_tools();

    // Create model router
    let router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));

    // Create WebSocket state
    let (tx, mut rx) = broadcast::channel(100);
    let ws_state = Arc::new(WebSocketState::new(tx));

    // Create chat handler with router
    let chat_handler = ChatHandler::new(ws_state.clone(), router, github_service);

    // Test message that should use streaming
    let test_message = ChatMessage::UserMessage {
        content: "Hello, how are you?".to_string(),
    };

    // Process message in background
    let handle = tokio::spawn({
        let chat_handler = chat_handler.clone();
        async move {
            chat_handler
                .handle_message(test_message, "test_conn".to_string())
                .await
        }
    });

    // Collect streamed messages
    let mut messages = Vec::new();
    while let Ok(msg) = rx.recv().await {
        messages.push(msg);
        if msg.contains("\"status\":\"complete\"") {
            break;
        }
    }

    // Wait for handler to complete
    let result = handle.await.unwrap();
    assert!(result.is_ok(), "Message handling should succeed");

    // Verify messages
    assert!(!messages.is_empty(), "Should receive streamed messages");
    assert!(
        messages.iter().any(|m| m.contains("\"status\":\"typing\"")),
        "Should receive typing status"
    );
    assert!(
        messages.iter().any(|m| m.contains("\"status\":\"complete\"")),
        "Should receive complete status"
    );
}