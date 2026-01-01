/// Integration tests for FM-Bridge LocalModelBackend implementation
///
/// These tests verify that FMClient correctly implements the LocalModelBackend trait
/// using a mock HTTP server to simulate the Apple Foundation Models API.
use fm_bridge::FMClient;
use local_inference::{CompletionRequest, LocalModelBackend, LocalModelBackendExt};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
async fn test_fm_bridge_initialize() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    assert!(
        !client.is_ready().await,
        "Should not be ready before initialization"
    );

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    assert!(
        client.is_ready().await,
        "Should be ready after initialization"
    );
}

#[tokio::test]
async fn test_fm_bridge_initialize_failure() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint to return error
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "error": "Service unavailable"
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    let result = client.initialize().await;
    assert!(
        result.is_err(),
        "Initialization should fail when service is down"
    );
}

#[tokio::test]
async fn test_fm_bridge_list_models() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/models endpoint
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {
                    "id": "apple-intelligence-1",
                    "object": "model",
                    "owned_by": "Apple",
                    "created": 1234567890
                },
                {
                    "id": "apple-intelligence-2",
                    "object": "model",
                    "owned_by": "Apple",
                    "created": 1234567891
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    let models = client.list_models().await.expect("Should list models");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "apple-intelligence-1");
    assert_eq!(models[1].id, "apple-intelligence-2");
}

#[tokio::test]
async fn test_fm_bridge_get_model_info() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/models endpoint
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {
                    "id": "apple-intelligence-1",
                    "object": "model",
                    "owned_by": "Apple",
                    "created": 1234567890
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    let model = client
        .get_model_info("apple-intelligence-1")
        .await
        .expect("Should get model info");

    assert_eq!(model.id, "apple-intelligence-1");
    assert_eq!(model.name, "apple-intelligence-1");

    let result = client.get_model_info("nonexistent").await;
    assert!(result.is_err(), "Should fail for nonexistent model");
}

#[tokio::test]
async fn test_fm_bridge_complete() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/chat/completions endpoint
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "completion-abc",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "apple-intelligence-1",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Rust is a systems programming language focused on safety and performance."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 8,
                "completion_tokens": 15,
                "total_tokens": 23
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("apple-intelligence-1", "What is Rust?");
    let response = LocalModelBackend::complete(&client, request)
        .await
        .expect("Completion should succeed");

    assert_eq!(response.id, "completion-abc");
    assert_eq!(response.model, "apple-intelligence-1");
    assert_eq!(
        response.text,
        "Rust is a systems programming language focused on safety and performance."
    );
    assert_eq!(response.finish_reason, Some("Stop".to_string()));

    let usage = response.usage.expect("Should have usage info");
    assert_eq!(usage.prompt_tokens, 8);
    assert_eq!(usage.completion_tokens, 15);
    assert_eq!(usage.total_tokens, 23);
}

#[tokio::test]
async fn test_fm_bridge_complete_simple() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/chat/completions endpoint
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "completion-def",
            "object": "chat.completion",
            "created": 1234567891,
            "model": "apple-intelligence-1",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Rust is great for systems programming."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 8,
                "total_tokens": 13
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    // Test the convenience method from LocalModelBackendExt
    let text = client
        .complete_simple("apple-intelligence-1", "What is Rust?")
        .await
        .expect("Simple completion should succeed");

    assert_eq!(text, "Rust is great for systems programming.");
}

#[tokio::test]
async fn test_fm_bridge_complete_stream() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/chat/completions endpoint with SSE stream
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Rust\"},\"finish_reason\":null}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\" is\"},\"finish_reason\":null}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\" amazing\"},\"finish_reason\":null}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\"!\"},\"finish_reason\":\"stop\"}]}\n\n\
                     data: [DONE]\n\n",
                )
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("apple-intelligence-1", "What is Rust?");
    let mut rx = client
        .complete_stream(request)
        .await
        .expect("Stream should start");

    let mut chunks = Vec::new();
    while let Some(result) = rx.recv().await {
        let chunk = result.expect("Chunk should be Ok");
        chunks.push(chunk);
    }

    assert!(chunks.len() >= 4, "Should receive at least 4 chunks");

    // Collect all text
    let full_text: String = chunks.iter().map(|c| c.delta.as_str()).collect();
    assert!(full_text.contains("Rust"), "Text should contain 'Rust'");

    // Last chunk should have finish_reason
    let last_chunk = chunks.last().unwrap();
    assert!(
        last_chunk.finish_reason.is_some(),
        "Last chunk should have finish_reason"
    );
}

#[tokio::test]
async fn test_fm_bridge_has_model() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/models endpoint
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {
                    "id": "apple-intelligence-1",
                    "object": "model",
                    "owned_by": "Apple",
                    "created": 1234567890
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    // Test the convenience method from LocalModelBackendExt
    assert!(
        client
            .has_model("apple-intelligence-1")
            .await
            .expect("has_model should succeed"),
        "Should have apple-intelligence-1"
    );

    assert!(
        !client
            .has_model("nonexistent")
            .await
            .expect("has_model should succeed"),
        "Should not have nonexistent model"
    );
}

#[tokio::test]
async fn test_fm_bridge_shutdown() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");
    assert!(client.is_ready().await);

    client.shutdown().await.expect("Shutdown should succeed");

    // Note: For HTTP clients, shutdown is a no-op, but we verify the trait works
}

#[tokio::test]
async fn test_fm_bridge_error_handling() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/chat/completions endpoint to return error
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": {
                "message": "Internal server error",
                "type": "server_error"
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = FMClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("apple-intelligence-1", "Test");
    let result = LocalModelBackend::complete(&client, request).await;

    assert!(result.is_err(), "Should fail with server error");
}
