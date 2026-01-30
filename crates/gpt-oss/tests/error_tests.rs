//! Error handling tests for GPT-OSS client

use std::time::Duration;

use gpt_oss::{GptOssClient, GptOssError, GptOssRequest};

#[test]
fn test_error_display() {
    let error = GptOssError::ApiError {
        status: 404,
        message: "Not found".to_string(),
    };

    let display_str = format!("{}", error);
    assert!(display_str.contains("404"));
    assert!(display_str.contains("Not found"));
}

#[test]
fn test_error_debug() {
    let error = GptOssError::ApiError {
        status: 500,
        message: "Internal error".to_string(),
    };

    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ApiError"));
}

#[tokio::test]
async fn test_network_error_handling() {
    let client = GptOssClient::builder()
        .base_url("http://invalid.localhost:9999")
        .timeout(Duration::from_secs(1)) // Short timeout
        .build()
        .unwrap();

    let request = GptOssRequest {
        model: "test".to_string(),
        prompt: "test".to_string(),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        stream: false,
        json_schema: None,
    };

    let result = client.complete(request).await;

    assert!(result.is_err(), "Should fail with network error");

    // Verify it's an HTTP/network error (not an API error)
    if let Err(GptOssError::HttpError(_)) = result {
        // Expected
    } else {
        panic!("Expected HttpError variant");
    }
}

#[tokio::test]
async fn test_timeout_error() {
    let client = GptOssClient::builder()
        .base_url("http://httpbin.org/delay/10") // Delayed response
        .timeout(Duration::from_secs(1)) // Short timeout
        .build()
        .unwrap();

    let request = GptOssRequest {
        model: "test".to_string(),
        prompt: "test".to_string(),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        stream: false,
        json_schema: None,
    };

    let result = client.complete(request).await;

    // Should timeout and return error
    assert!(result.is_err(), "Should timeout");
}

#[test]
fn test_error_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<GptOssError>();
    assert_sync::<GptOssError>();
}

#[tokio::test]
async fn test_error_from_reqwest() {
    // Test that reqwest errors convert properly
    let reqwest_error = reqwest::get("http://[::1").await.unwrap_err();
    let gpt_oss_error: GptOssError = reqwest_error.into();

    // Should be HttpError variant
    if let GptOssError::HttpError(_) = gpt_oss_error {
        // Expected
    } else {
        panic!("Expected HttpError from reqwest error");
    }
}

#[tokio::test]
async fn test_multiple_sequential_errors() {
    let client = GptOssClient::builder()
        .base_url("http://localhost:9999")
        .build()
        .unwrap();

    let request = GptOssRequest {
        model: "test".to_string(),
        prompt: "test".to_string(),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        stream: false,
        json_schema: None,
    };

    // Multiple errors should be independent
    let result1 = client.complete(request.clone()).await;
    let result2 = client.complete(request.clone()).await;
    let result3 = client.complete(request).await;

    assert!(result1.is_err());
    assert!(result2.is_err());
    assert!(result3.is_err());
}

#[test]
fn test_result_type_alias() {
    // Verify Result type alias works
    fn returns_result() -> gpt_oss::Result<String> {
        Ok("test".to_string())
    }

    let result = returns_result();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test");
}
