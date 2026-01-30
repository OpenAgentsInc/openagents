//! Unit tests for GPT-OSS client

use std::time::Duration;

use gpt_oss::{GptOssClient, GptOssRequest, GptOssResponsesRequest};

#[test]
fn test_client_builder() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:8000")
        .default_model("gpt-oss-20b")
        .build();

    assert!(client.is_ok(), "Client builder should succeed");
}

#[test]
fn test_client_with_base_url() {
    let client = GptOssClient::with_base_url("http://localhost:9000");
    assert!(
        client.is_ok(),
        "Client creation with base URL should succeed"
    );
}

#[test]
fn test_client_new_default() {
    let client = GptOssClient::new();
    assert!(client.is_ok(), "Default client creation should succeed");
}

#[tokio::test]
async fn test_client_health_check() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:8000")
        .build()
        .unwrap();

    // Health check may fail if server is not running, but should not panic
    let result = client.health().await;
    // Just verify it returns a result (ok or err)
    let _ = result;
}

#[tokio::test]
async fn test_complete_with_invalid_server() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:9999") // Invalid port
        .build()
        .unwrap();

    let request = GptOssRequest {
        model: "gpt-oss-20b".to_string(),
        prompt: "Test".to_string(),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        stream: false,
        json_schema: None,
    };

    let result = client.complete(request).await;
    assert!(
        result.is_err(),
        "Request to invalid server should fail gracefully"
    );
}

#[tokio::test]
async fn test_complete_simple_with_invalid_server() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:9999")
        .build()
        .unwrap();

    let result = client.complete_simple("gpt-oss-20b", "Test prompt").await;
    assert!(
        result.is_err(),
        "Simple complete should fail with invalid server"
    );
}

#[tokio::test]
async fn test_responses_with_invalid_server() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:9999")
        .build()
        .unwrap();

    let request = GptOssResponsesRequest::new("gpt-oss-20b", "Test");

    let result = client.responses(request).await;
    assert!(
        result.is_err(),
        "Responses API should fail with invalid server"
    );
}

#[test]
fn test_builder_chaining() {
    let result = GptOssClient::builder()
        .base_url("http://localhost:8000")
        .default_model("custom-model")
        .timeout(Duration::from_secs(60))
        .build();

    assert!(result.is_ok(), "Builder chaining should work");
}

#[test]
fn test_client_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<GptOssClient>();
    assert_sync::<GptOssClient>();
}

#[test]
fn test_client_clone() {
    let client = GptOssClient::new().unwrap();
    let cloned = client.clone();

    // Should be able to clone without error
    drop(client);
    drop(cloned);
}
