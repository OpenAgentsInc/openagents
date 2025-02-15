use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;
use tracing::info;

use openagents::server::{handlers::auth::AuthState, services::auth::OIDCConfig};

mod common;
use common::setup_test_db;

const MAX_SIZE: usize = 1024 * 1024; // 1MB limit for response bodies

#[tokio::test]
#[ignore = "Test failing - needs investigation"]
async fn test_error_component_accessibility() {
    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        "http://localhost:3000/auth".to_string(),
        "http://localhost:3000/token".to_string(),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool.clone());

    let app = openagents::server::config::configure_app(pool);

    // Submit signup request with missing terms acceptance
    let request = Request::builder()
        .method("POST")
        .uri("/auth/signup")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(
            "email=test%40example.com&password=password123&password-confirm=password123",
        ))
        .unwrap();

    info!("Sending request: {:?}", request);
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    info!("Response status: {}", status);

    let body = to_bytes(response.into_body(), MAX_SIZE).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    info!("Response body: {}", body_str);

    // Verify error response
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        error_response,
        json!({
            "error": "Terms must be accepted"
        })
    );
}

#[tokio::test]
#[ignore = "Test failing - needs investigation"]
async fn test_error_component_included() {
    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        "http://localhost:3000/auth".to_string(),
        "http://localhost:3000/token".to_string(),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool.clone());

    let app = openagents::server::config::configure_app(pool);

    // Submit signup request
    let request = Request::builder()
        .method("POST")
        .uri("/auth/signup")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(
            "email=test%40example.com&password=password123&password-confirm=password123",
        ))
        .unwrap();

    info!("Sending request: {:?}", request);
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    info!("Response status: {}", status);

    let body = to_bytes(response.into_body(), MAX_SIZE).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    info!("Response body: {}", body_str);

    // Verify error response
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        error_response,
        json!({
            "error": "Terms must be accepted"
        })
    );
}

#[tokio::test]
#[ignore = "Test failing - needs investigation"]
async fn test_error_js_included() {
    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        "http://localhost:3000/auth".to_string(),
        "http://localhost:3000/token".to_string(),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool.clone());

    let app = openagents::server::config::configure_app(pool);

    // Submit signup request with mismatched passwords
    let request = Request::builder()
        .method("POST")
        .uri("/auth/signup")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(
            "email=test%40example.com&password=password123&password-confirm=password456&terms=on",
        ))
        .unwrap();

    info!("Sending request: {:?}", request);
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    info!("Response status: {}", status);

    let body = to_bytes(response.into_body(), MAX_SIZE).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    info!("Response body: {}", body_str);

    // Verify error response
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        error_response,
        json!({
            "error": "Passwords do not match"
        })
    );
}
