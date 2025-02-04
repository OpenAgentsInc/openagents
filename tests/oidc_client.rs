use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;
use tower::ServiceExt;
use tracing::info;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

mod common;
use common::setup_test_db;

const MAX_SIZE: usize = 1024 * 1024; // 1MB limit for response bodies

async fn setup_test_env(mock_server: &MockServer) {
    // Set up test database
    let _pool = setup_test_db().await;

    // Set required environment variables for app configuration
    std::env::set_var("DEEPSEEK_API_KEY", "test_key");
    std::env::set_var("GITHUB_TOKEN", "test_token");
    std::env::set_var("FIRECRAWL_API_KEY", "test_key");
    std::env::set_var("OIDC_CLIENT_ID", "test_client");
    std::env::set_var("OIDC_CLIENT_SECRET", "test_secret");
    std::env::set_var("OIDC_AUTH_URL", format!("{}/auth", mock_server.uri()));
    std::env::set_var("OIDC_TOKEN_URL", format!("{}/token", mock_server.uri()));
    std::env::set_var(
        "OIDC_REDIRECT_URI",
        "http://localhost:8000/auth/callback".to_string(),
    );
    std::env::set_var(
        "DATABASE_URL",
        "postgres://postgres:postgres@localhost:5432/test",
    );
}

fn create_test_jwt() -> String {
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(r#"{"sub":"test_user"}"#);
    format!("{}.{}.signature", header, claims)
}

#[tokio::test]
async fn test_full_auth_flow() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;
    info!("Mock server started at: {}", mock_server.uri());

    // Set up test environment
    setup_test_env(&mock_server).await;

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": create_test_jwt(),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    info!("Mock token endpoint configured");

    let app = openagents::server::config::configure_app();
    info!("App configured");

    // Test login redirect
    info!("Testing login redirect");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("Login redirect response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    info!("Login redirect location: {}", location);
    assert!(location.contains("/auth"));
    assert!(location.contains("flow=login"));

    // Test callback
    info!("Testing callback");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code&flow=login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("Callback response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    info!("Callback set-cookie: {}", cookie);
    assert!(cookie.contains("session="));

    // Test logout
    info!("Testing logout");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("Logout response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    info!("Logout set-cookie: {}", cookie);
    assert!(cookie.contains("session=;"));
}

#[tokio::test]
async fn test_invalid_callback() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;
    info!("Mock server started at: {}", mock_server.uri());

    // Set up test environment
    setup_test_env(&mock_server).await;

    // Mock token endpoint with error
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "Invalid authorization code"
        })))
        .mount(&mock_server)
        .await;

    info!("Mock token endpoint configured with error response");

    let app = openagents::server::config::configure_app();
    info!("App configured");

    // Test callback with invalid code
    info!("Testing callback with invalid code");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=invalid_code&flow=login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("Invalid callback response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let body = to_bytes(response.into_body(), MAX_SIZE).await.unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    info!("Invalid callback error response: {:?}", error_response);

    assert!(error_response["error"]
        .as_str()
        .unwrap()
        .contains("Token exchange failed"));
}

#[tokio::test]
async fn test_duplicate_login() {
    // Create mock OIDC server
    let mock_server = MockServer::start().await;
    info!("Mock server started at: {}", mock_server.uri());

    // Set up test environment
    setup_test_env(&mock_server).await;

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": create_test_jwt(),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    info!("Mock token endpoint configured");

    let app = openagents::server::config::configure_app();
    info!("App configured");

    // First login should succeed
    info!("Testing first login");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code&flow=signup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("First login response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

    // Second login should also succeed (update last_login_at)
    info!("Testing second login");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/callback?code=test_code&flow=signup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    info!("Second login response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
}