//! E2E tests for HttpExecutor
//!
//! Note: These tests use `#[test]` instead of `#[tokio::test]` because
//! ExecutorManager creates its own tokio runtime. All async operations
//! are run via `fixture.block_on()`.

use crate::fixtures::{
    ExecutorTestFixture, HttpMockServer, fast_test_config, retry_test_config, timeout_test_config,
    wait_for_completion, wait_for_failure, wait_for_response,
};
use oanix::services::{HttpMethod, HttpRequest};
use std::collections::HashMap;
use std::time::Duration;

/// Test basic GET request flow
#[test]
fn test_http_get_full_flow() {
    // Setup executor first (creates runtime)
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    // Start mock server on fixture's runtime
    let mock = fixture.block_on(HttpMockServer::start());
    fixture.block_on(mock.mount_get("/api/data", 200, r#"{"result": "success"}"#));

    // Start executor
    fixture.start().unwrap();

    // Submit request via HttpFs
    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: format!("{}/api/data", mock.uri()),
        headers: HashMap::new(),
        body: None,
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    // Wait for response (using fixture's runtime)
    let http_fs = &fixture.http_fs;
    let got_response =
        fixture.block_on(wait_for_response(http_fs, &req_id, Duration::from_secs(5)));
    assert!(got_response, "Response should arrive within timeout");

    // Verify response
    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("success"));

    fixture.shutdown().unwrap();
}

/// Test POST request with JSON body
#[test]
fn test_http_post_with_body() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    fixture.block_on(mock.mount_post("/api/submit", 201, r#"{"created": true}"#));

    fixture.start().unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Post,
        url: format!("{}/api/submit", mock.uri()),
        headers,
        body: Some(r#"{"data": 42}"#.to_string()),
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    let http_fs = &fixture.http_fs;
    let got_response =
        fixture.block_on(wait_for_response(http_fs, &req_id, Duration::from_secs(5)));
    assert!(got_response);

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 201);
    assert!(response.body.contains("created"));

    fixture.shutdown().unwrap();
}

/// Test timeout handling
#[test]
fn test_http_timeout() {
    let mut fixture = ExecutorTestFixture::new(timeout_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    // 10 second delay, but our timeout is 100ms
    fixture.block_on(mock.mount_slow_response("/slow", 10000));

    fixture.start().unwrap();

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: format!("{}/slow", mock.uri()),
        headers: HashMap::new(),
        body: None,
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    // Wait for failure (should timeout)
    let http_fs = &fixture.http_fs;
    let failed = fixture.block_on(wait_for_failure(http_fs, &req_id, Duration::from_secs(5)));
    assert!(failed, "Request should fail due to timeout");

    // Verify it's a timeout error
    let error = fixture.http_fs.get_failure(&req_id).unwrap();
    assert!(
        error.to_lowercase().contains("timeout"),
        "Error should mention timeout: {}",
        error
    );

    fixture.shutdown().unwrap();
}

/// Test retry on 5xx errors
#[test]
fn test_http_retry_on_error() {
    let mut fixture = ExecutorTestFixture::new(retry_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    // Mount success first (lower priority), then error with limited times (higher priority)
    // After 2 error responses, wiremock falls back to the success response
    fixture.block_on(mock.mount_get("/flaky", 200, r#"{"success": true}"#));
    fixture.block_on(mock.mount_error("/flaky", 500, 2));

    fixture.start().unwrap();

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: format!("{}/flaky", mock.uri()),
        headers: HashMap::new(),
        body: None,
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    // Should eventually succeed after retries
    let http_fs = &fixture.http_fs;
    let got_response =
        fixture.block_on(wait_for_response(http_fs, &req_id, Duration::from_secs(10)));
    assert!(got_response, "Request should succeed after retries");

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);

    fixture.shutdown().unwrap();
}

/// Test multiple concurrent requests
#[test]
fn test_http_concurrent_requests() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    for i in 0..10 {
        fixture.block_on(mock.mount_get(
            &format!("/api/{}", i),
            200,
            &format!(r#"{{"index": {}}}"#, i),
        ));
    }

    fixture.start().unwrap();

    let mut req_ids = Vec::new();
    for i in 0..10 {
        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: format!("{}/api/{}", mock.uri(), i),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        };
        req_ids.push(fixture.http_fs.submit_request(request));
    }

    // Wait for all responses
    for req_id in &req_ids {
        let http_fs = &fixture.http_fs;
        let got_response =
            fixture.block_on(wait_for_response(http_fs, req_id, Duration::from_secs(10)));
        assert!(got_response, "Request {} should complete", req_id);
    }

    // Verify all succeeded
    for req_id in &req_ids {
        let response = fixture.http_fs.get_response(req_id).unwrap();
        assert_eq!(response.status, 200);
    }

    fixture.shutdown().unwrap();
}

/// Test custom headers are sent correctly
#[test]
fn test_http_headers() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    fixture.block_on(mock.mount_get("/headers", 200, r#"{"received": true}"#));

    fixture.start().unwrap();

    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "test-value".to_string());
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Get,
        url: format!("{}/headers", mock.uri()),
        headers,
        body: None,
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    let http_fs = &fixture.http_fs;
    let got_response =
        fixture.block_on(wait_for_response(http_fs, &req_id, Duration::from_secs(5)));
    assert!(got_response);

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);

    // Verify the mock received the request (headers were sent)
    let received = fixture.block_on(mock.received_requests());
    assert!(received > 0);

    fixture.shutdown().unwrap();
}

/// Test PUT method
#[test]
fn test_http_put_method() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    fixture.block_on(mock.mount_put("/api/resource", 200, r#"{"updated": true}"#));

    fixture.start().unwrap();

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Put,
        url: format!("{}/api/resource", mock.uri()),
        headers: HashMap::new(),
        body: Some(r#"{"value": "new"}"#.to_string()),
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    let http_fs = &fixture.http_fs;
    let got_response =
        fixture.block_on(wait_for_response(http_fs, &req_id, Duration::from_secs(5)));
    assert!(got_response);

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("updated"));

    fixture.shutdown().unwrap();
}

/// Test DELETE method
#[test]
fn test_http_delete_method() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock = fixture.block_on(HttpMockServer::start());
    fixture.block_on(mock.mount_delete("/api/resource/123", 204));

    fixture.start().unwrap();

    let request = HttpRequest {
        id: String::new(),
        method: HttpMethod::Delete,
        url: format!("{}/api/resource/123", mock.uri()),
        headers: HashMap::new(),
        body: None,
        timeout_secs: None,
        created_at: 0,
    };
    let req_id = fixture.http_fs.submit_request(request);

    let http_fs = &fixture.http_fs;
    let completed = fixture.block_on(wait_for_completion(
        http_fs,
        &req_id,
        Duration::from_secs(5),
    ));
    assert!(completed);

    let response = fixture.http_fs.get_response(&req_id).unwrap();
    assert_eq!(response.status, 204);

    fixture.shutdown().unwrap();
}
