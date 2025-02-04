use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use openagents::server::{
    handlers::auth::AuthState,
    services::auth::OIDCConfig,
};

mod common;
use common::setup_test_db;

#[tokio::test]
async fn test_full_auth_flow() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;

    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/auth", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool);

    let app = openagents::server::config::configure_app();

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": "test_id_token",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // Test login redirect
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert!(response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("/auth"));

    // Test callback
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert!(response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("session="));

    // Test logout
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert!(response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("session=;"));
}

#[tokio::test]
async fn test_invalid_callback() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;

    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/auth", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool);

    let app = openagents::server::config::configure_app();

    // Mock token endpoint with error
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "Invalid authorization code"
        })))
        .mount(&mock_server)
        .await;

    // Test callback with invalid code
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=invalid_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let body = to_bytes(response.into_body()).await.unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(error_response["error"]
        .as_str()
        .unwrap()
        .contains("Token exchange failed"));
}

#[tokio::test]
async fn test_duplicate_login() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;

    // Create test app
    let pool = setup_test_db().await;
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/auth", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();
    let _auth_state = AuthState::new(config, pool);

    let app = openagents::server::config::configure_app();

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": "test_id_token",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // First login should succeed
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

    // Second login should also succeed (update last_login_at)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
}