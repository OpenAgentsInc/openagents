use std::sync::Arc;

use openagents::server::{
    config::AppState,
    services::{
        deepseek::DeepSeekService,
        github_issue::GitHubService,
        groq::GroqService,
        model_router::ModelRouter,
        oauth::{github::GitHubOAuth, scramble::ScrambleOAuth, OAuthConfig},
        openrouter::{OpenRouterConfig, OpenRouterService},
    },
    tools::create_tools,
    ws::{
        handlers::chat::{ChatHandler, ChatMessage},
        transport::WebSocketState,
    },
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing_subscriber;
use uuid::Uuid;
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
    let mock_groq_server = MockServer::start().await;

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

    // Mock the Groq API response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test_response",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "mixtral-8x7b-32768",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_groq_server)
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
    let _openrouter = OpenRouterService::with_config("test_key".to_string(), openrouter_config);

    // Create tools
    let tools = create_tools();

    // Create model router
    let model_router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));

    // Create WebSocket state
    let ws_state = WebSocketState::new(github_service.clone(), model_router);
    let ws_state = Arc::new(ws_state);

    // Create test channel
    let (tx, mut rx) = mpsc::channel::<String>(32);
    ws_state
        .add_connection("test_conn".to_string(), tx.clone())
        .await
        .unwrap();

    // Create OAuth configs
    let github_config = OAuthConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        redirect_url: "http://localhost:3000/auth/github/callback".to_string(),
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
    };

    let scramble_config = OAuthConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        redirect_url: "http://localhost:3000/auth/scramble/callback".to_string(),
        auth_url: "https://scramble.com/oauth/authorize".to_string(),
        token_url: "https://scramble.com/oauth/token".to_string(),
    };

    // Create app state
    let app_state = AppState {
        ws_state: ws_state.clone(),
        github_oauth: Arc::new(
            GitHubOAuth::new(pool.clone(), github_config).expect("Failed to create GitHub OAuth"),
        ),
        scramble_oauth: Arc::new(
            ScrambleOAuth::new(pool.clone(), scramble_config)
                .expect("Failed to create Scramble OAuth"),
        ),
        pool: pool.clone(),
        frontend_url: "http://localhost:3000".to_string(),
        groq: Arc::new(GroqService::with_base_url(
            "test_key".to_string(),
            format!("{}/v1", mock_groq_server.uri()),
        )),
    };

    // Create chat handler
    let mut chat_handler = ChatHandler::new(tx, app_state, "test_user".to_string());

    // Test message handling
    let msg = ChatMessage::Message {
        id: Uuid::new_v4(),
        conversation_id: None,
        content: "Hello, world!".to_string(),
        repos: None,
        use_reasoning: None,
    };

    chat_handler.handle_message(msg).await.unwrap();

    // Check response
    if let Ok(text) = rx.try_recv() {
        let response: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(response["type"].is_string());
        assert!(response["message_id"].is_string());
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
    let mock_groq_server = MockServer::start().await;

    // Test streaming message
    let msg_id = Uuid::new_v4();
    let msg = ChatMessage::Message {
        id: msg_id,
        conversation_id: None,
        content: "Stream this response".to_string(),
        repos: None,
        use_reasoning: Some(true),
    };

    // Mock the Groq API streaming response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(concat!(
                    "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"mixtral-8x7b-32768\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"This is \"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"mixtral-8x7b-32768\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a streaming \"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"mixtral-8x7b-32768\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"response\"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"mixtral-8x7b-32768\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning\":\"This is a test reasoning\"},\"finish_reason\":\"stop\"}]}\n\n",
                    "data: [DONE]\n\n"
                )),
        )
        .mount(&mock_groq_server)
        .await;

    // Mock the DeepSeek API streaming response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(concat!(
                "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"deepseek-chat\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"This is \"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"deepseek-chat\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a streaming \"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"test_response\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"deepseek-chat\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"response\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"
            )))
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
    let _openrouter = OpenRouterService::with_config("test_key".to_string(), openrouter_config);

    // Create tools
    let tools = create_tools();

    // Create model router
    let model_router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));

    // Create WebSocket state
    let ws_state = WebSocketState::new(github_service.clone(), model_router);
    let ws_state = Arc::new(ws_state);

    // Create test channel
    let (tx, mut rx) = mpsc::channel::<String>(32);
    ws_state
        .add_connection("test_conn".to_string(), tx.clone())
        .await
        .unwrap();

    // Create OAuth configs
    let github_config = OAuthConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        redirect_url: "http://localhost:3000/auth/github/callback".to_string(),
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
    };

    let scramble_config = OAuthConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        redirect_url: "http://localhost:3000/auth/scramble/callback".to_string(),
        auth_url: "https://scramble.com/oauth/authorize".to_string(),
        token_url: "https://scramble.com/oauth/token".to_string(),
    };

    // Create app state
    let app_state = AppState {
        ws_state: ws_state.clone(),
        github_oauth: Arc::new(
            GitHubOAuth::new(pool.clone(), github_config).expect("Failed to create GitHub OAuth"),
        ),
        scramble_oauth: Arc::new(
            ScrambleOAuth::new(pool.clone(), scramble_config)
                .expect("Failed to create Scramble OAuth"),
        ),
        pool: pool.clone(),
        frontend_url: "http://localhost:3000".to_string(),
        groq: Arc::new(GroqService::with_base_url(
            "test_key".to_string(),
            format!("{}/v1", mock_groq_server.uri()),
        )),
    };

    // Create chat handler
    let mut chat_handler = ChatHandler::new(tx, app_state, "test_user".to_string());

    // Test streaming message
    chat_handler.handle_message(msg).await.unwrap();

    // Check streaming responses
    let mut responses = Vec::new();
    while let Ok(text) = rx.try_recv() {
        let response: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(response["type"].is_string());
        responses.push(response);
    }

    assert!(!responses.is_empty(), "No streaming responses received");

    // Verify we got both Update and Complete messages
    let has_update = responses.iter().any(|r| r["type"] == "Update");
    let has_complete = responses.iter().any(|r| r["type"] == "Complete");
    assert!(has_update, "No Update message received");
    assert!(has_complete, "No Complete message received");
}
