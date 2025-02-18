use std::sync::Arc;

use axum::extract::ws::Message;
use openagents::server::{
    services::{
        deepseek::DeepSeekService,
        github_issue::GitHubService,
        openrouter::{OpenRouterConfig, OpenRouterService},
        solver::SolverService,
    },
    tools::create_tools,
    ws::{
        handlers::{chat::ChatHandler, MessageHandler},
        transport::WebSocketState,
        types::ChatMessage,
    },
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashSet;
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

async fn setup_test_db() -> sqlx::PgPool {
    // Use test database URL from environment or fall back to default test database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/postgres".to_string());

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres")
}

#[tokio::test]
async fn test_chat_router_integration() {
    init_logging();

    // Set up test database
    let pool = setup_test_db().await;

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
        format!("{}/v1", mock_server.uri()),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        format!("{}/v1", mock_server.uri()),
    ));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string()))
            .expect("Failed to create GitHub service"),
    );

    // Create OpenRouter service for solver
    let openrouter_config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };
    let openrouter = OpenRouterService::with_config("test_key".to_string(), openrouter_config);
    let solver_service = Arc::new(SolverService::new(pool.clone(), openrouter));

    // Create tools
    let tools = create_tools();

    // Create WebSocket state
    let ws_state = WebSocketState::new(
        tool_model.clone(),
        chat_model.clone(),
        github_service.clone(),
        solver_service.clone(),
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

    chat_handler
        .handle_message(msg, "test_conn".to_string())
        .await
        .unwrap();

    // Check response
    if let Ok(response) = rx.try_recv() {
        match response {
            Message::Text(text) => {
                let response: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(response["type"], "chat");
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

    // Set up test database
    let pool = setup_test_db().await;

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
        format!("{}/v1", mock_server.uri()),
    ));
    let chat_model = Arc::new(DeepSeekService::with_base_url(
        "test_key".to_string(),
        format!("{}/v1", mock_server.uri()),
    ));
    let github_service = Arc::new(
        GitHubService::new(Some("test_token".to_string()))
            .expect("Failed to create GitHub service"),
    );

    // Create OpenRouter service for solver
    let openrouter_config = OpenRouterConfig {
        model: "test-model".to_string(),
        use_reasoner: false,
        test_mode: true,
        rate_limited_models: HashSet::new(),
    };
    let openrouter = OpenRouterService::with_config("test_key".to_string(), openrouter_config);
    let solver_service = Arc::new(SolverService::new(pool.clone(), openrouter));

    // Create tools
    let tools = create_tools();

    // Create WebSocket state
    let ws_state = WebSocketState::new(
        tool_model.clone(),
        chat_model.clone(),
        github_service.clone(),
        solver_service.clone(),
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

    chat_handler
        .handle_message(msg, "test_conn".to_string())
        .await
        .unwrap();

    // Check streaming responses
    let mut responses = Vec::new();
    while let Ok(response) = rx.try_recv() {
        match response {
            Message::Text(text) => {
                let response: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(response["type"], "chat");
                responses.push(response);
            }
            _ => panic!("Expected text message"),
        }
    }

    assert!(!responses.is_empty(), "No streaming responses received");
}
