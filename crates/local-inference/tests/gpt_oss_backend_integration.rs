/// Integration tests for GPT-OSS LocalModelBackend implementation
///
/// These tests verify that GptOssClient correctly implements the LocalModelBackend trait
/// using a mock HTTP server to simulate the GPT-OSS Responses API.
use gpt_oss::GptOssClient;
use local_inference::{CompletionRequest, LocalModelBackend, LocalModelBackendExt};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
async fn test_gpt_oss_initialize() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
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
async fn test_gpt_oss_initialize_failure() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint to return error
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "error": "Service unavailable"
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
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
async fn test_gpt_oss_list_models() {
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
            "data": [
                {
                    "id": "gpt-oss-20b",
                    "name": "GPT-OSS 20B",
                    "context_length": 128000,
                    "description": "Small but capable model"
                },
                {
                    "id": "gpt-oss-120b",
                    "name": "GPT-OSS 120B",
                    "context_length": 128000,
                    "description": "Large multimodal model"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    let models = client.list_models().await.expect("Should list models");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "gpt-oss-20b");
    assert_eq!(models[0].context_length, 128000);
    assert_eq!(models[1].id, "gpt-oss-120b");
}

#[tokio::test]
async fn test_gpt_oss_get_model_info() {
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
            "data": [
                {
                    "id": "gpt-oss-20b",
                    "name": "GPT-OSS 20B",
                    "context_length": 128000,
                    "description": "Small but capable model"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    let model = client
        .get_model_info("gpt-oss-20b")
        .await
        .expect("Should get model info");

    assert_eq!(model.id, "gpt-oss-20b");
    assert_eq!(model.name, "GPT-OSS 20B");
    assert_eq!(model.context_length, 128000);

    let result = client.get_model_info("nonexistent").await;
    assert!(result.is_err(), "Should fail for nonexistent model");
}

#[tokio::test]
async fn test_gpt_oss_complete() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/completions endpoint
    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "completion-123",
            "model": "gpt-oss-20b",
            "text": "This is a test response from GPT-OSS 20B.",
            "finish_reason": "stop",
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 10,
                "total_tokens": 15
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("gpt-oss-20b", "What is Rust?");
    let response = LocalModelBackend::complete(&client, request)
        .await
        .expect("Completion should succeed");

    assert_eq!(response.id, "completion-123");
    assert_eq!(response.model, "gpt-oss-20b");
    assert_eq!(response.text, "This is a test response from GPT-OSS 20B.");
    assert_eq!(response.finish_reason, Some("stop".to_string()));

    let usage = response.usage.expect("Should have usage info");
    assert_eq!(usage.prompt_tokens, 5);
    assert_eq!(usage.completion_tokens, 10);
    assert_eq!(usage.total_tokens, 15);
}

#[tokio::test]
async fn test_gpt_oss_complete_120b() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/completions endpoint
    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "completion-120b",
            "model": "gpt-oss-120b",
            "text": "This is a test response from GPT-OSS 120B.",
            "finish_reason": "stop",
            "usage": {
                "prompt_tokens": 7,
                "completion_tokens": 12,
                "total_tokens": 19
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("gpt-oss-120b", "Explain FROST signatures.");
    let response = LocalModelBackend::complete(&client, request)
        .await
        .expect("Completion should succeed");

    assert_eq!(response.id, "completion-120b");
    assert_eq!(response.model, "gpt-oss-120b");
    assert_eq!(response.text, "This is a test response from GPT-OSS 120B.");
    assert_eq!(response.finish_reason, Some("stop".to_string()));

    let usage = response.usage.expect("Should have usage info");
    assert_eq!(usage.prompt_tokens, 7);
    assert_eq!(usage.completion_tokens, 12);
    assert_eq!(usage.total_tokens, 19);
}

#[tokio::test]
async fn test_gpt_oss_complete_simple() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/completions endpoint
    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "completion-456",
            "model": "gpt-oss-20b",
            "text": "Rust is a systems programming language.",
            "finish_reason": "stop",
            "usage": {
                "prompt_tokens": 3,
                "completion_tokens": 7,
                "total_tokens": 10
            }
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    // Test the convenience method from LocalModelBackendExt
    let text = client
        .complete_simple("gpt-oss-20b", "What is Rust?")
        .await
        .expect("Simple completion should succeed");

    assert_eq!(text, "Rust is a systems programming language.");
}

#[tokio::test]
async fn test_gpt_oss_complete_stream() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    // Mock the /v1/completions endpoint with SSE stream
    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    "data: {\"id\":\"chunk-1\",\"model\":\"gpt-oss-20b\",\"delta\":\"Rust \",\"finish_reason\":null}\n\n\
                     data: {\"id\":\"chunk-2\",\"model\":\"gpt-oss-20b\",\"delta\":\"is \",\"finish_reason\":null}\n\n\
                     data: {\"id\":\"chunk-3\",\"model\":\"gpt-oss-20b\",\"delta\":\"great!\",\"finish_reason\":null}\n\n\
                     data: {\"id\":\"final\",\"model\":\"gpt-oss-20b\",\"delta\":\"\",\"finish_reason\":\"stop\"}\n\n"
                )
                .insert_header("content-type", "text/event-stream")
        )
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    client
        .initialize()
        .await
        .expect("Initialization should succeed");

    let request = CompletionRequest::new("gpt-oss-20b", "What is Rust?");
    let mut rx = client
        .complete_stream(request)
        .await
        .expect("Stream should start");

    let mut chunks = Vec::new();
    while let Some(result) = rx.recv().await {
        let chunk = result.expect("Chunk should be Ok");
        chunks.push(chunk);
    }

    assert_eq!(chunks.len(), 4, "Should receive 4 chunks");
    assert_eq!(chunks[0].delta, "Rust ");
    assert_eq!(chunks[1].delta, "is ");
    assert_eq!(chunks[2].delta, "great!");
    assert_eq!(chunks[3].finish_reason, Some("stop".to_string()));
}

#[tokio::test]
async fn test_gpt_oss_has_model() {
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
            "data": [
                {
                    "id": "gpt-oss-20b",
                    "name": "GPT-OSS 20B",
                    "context_length": 128000,
                    "description": "Small model"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = GptOssClient::builder()
        .base_url(&mock_server.uri())
        .build()
        .expect("Failed to build client");

    // Test the convenience method from LocalModelBackendExt
    assert!(
        client
            .has_model("gpt-oss-20b")
            .await
            .expect("has_model should succeed"),
        "Should have gpt-oss-20b"
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
async fn test_gpt_oss_shutdown() {
    let mock_server = MockServer::start().await;

    // Mock the /health endpoint
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok"
        })))
        .mount(&mock_server)
        .await;

    let mut client = GptOssClient::builder()
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
