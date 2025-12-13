//! Live HTTP smoke tests against httpbin.org
//!
//! Run with: `cargo test --features "net-executor,nostr" -p oanix -- --ignored http_live`

use crate::fixtures::{ExecutorTestFixture, wait_for_response};
use oanix::executor::ExecutorConfig;
use oanix::services::{HttpMethod, HttpRequest};
use std::collections::HashMap;
use std::time::Duration;

fn live_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(50))
        .http_timeout(Duration::from_secs(30))
        .ws_connect_timeout(Duration::from_secs(10))
        .build()
}

/// Live test: GET request to httpbin.org
#[tokio::test]
#[ignore] // Run with --ignored
async fn test_http_live_httpbin_get() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    let mut headers = HashMap::new();
    headers.insert("User-Agent".to_string(), "oanix-test/1.0".to_string());

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: "https://httpbin.org/get".to_string(),
        headers,
        body: None,
        timeout_secs: Some(30),
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    assert!(
        wait_for_response(&fixture.http_fs, &req_id, Duration::from_secs(30)).await,
        "Response should arrive from httpbin.org"
    );

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200, "httpbin.org should return 200");
    assert!(
        response.body.contains("httpbin.org") || response.body.contains("origin"),
        "Response should contain expected content"
    );

    println!("Live HTTP GET test passed!");
    println!("Response status: {}", response.status);
    println!("Response body length: {} bytes", response.body.len());

    fixture.shutdown().unwrap();
}

/// Live test: POST request to httpbin.org
#[tokio::test]
#[ignore]
async fn test_http_live_httpbin_post() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Post,
        url: "https://httpbin.org/post".to_string(),
        headers,
        body: Some(r#"{"test": "data", "from": "oanix"}"#.to_string()),
        timeout_secs: Some(30),
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    assert!(
        wait_for_response(&fixture.http_fs, &req_id, Duration::from_secs(30)).await,
        "Response should arrive from httpbin.org"
    );

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);
    assert!(
        response.body.contains("test") && response.body.contains("oanix"),
        "Response should echo our data"
    );

    println!("Live HTTP POST test passed!");

    fixture.shutdown().unwrap();
}

/// Live test: HTTP headers are correctly sent
#[tokio::test]
#[ignore]
async fn test_http_live_headers() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    let mut headers = HashMap::new();
    headers.insert(
        "X-Custom-Header".to_string(),
        "oanix-test-value".to_string(),
    );
    headers.insert(
        "Authorization".to_string(),
        "Bearer test-token-123".to_string(),
    );

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: "https://httpbin.org/headers".to_string(),
        headers,
        body: None,
        timeout_secs: Some(30),
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    assert!(wait_for_response(&fixture.http_fs, &req_id, Duration::from_secs(30)).await);

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);
    // httpbin.org echoes headers back in the response
    assert!(
        response.body.contains("X-Custom-Header") || response.body.contains("x-custom-header"),
        "Custom header should be echoed back"
    );

    println!("Live HTTP headers test passed!");

    fixture.shutdown().unwrap();
}

/// Live test: HTTP status codes
#[tokio::test]
#[ignore]
async fn test_http_live_status_codes() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    // Test various status codes
    let codes = [200, 201, 204, 400, 404, 500];

    for code in codes {
        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: format!("https://httpbin.org/status/{}", code),
            headers: HashMap::new(),
            body: None,
            timeout_secs: Some(30),
            created_at: 0,
        };
        let req_id = fixture.http_fs.submit_request(request);

        assert!(
            wait_for_response(&fixture.http_fs, &req_id, Duration::from_secs(30)).await,
            "Should get response for status {}",
            code
        );

        let response = fixture.http_fs.get_response(&req_id).unwrap();
        assert_eq!(response.status, code, "Should return status {}", code);

        println!("Status {} test passed", code);
    }

    fixture.shutdown().unwrap();
}
