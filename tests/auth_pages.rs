use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use openagents::server::config::configure_app;

fn setup_test_env() {
    // Load .env file
    dotenvy::dotenv().ok();

    // Set required test environment variables if not already set
    if std::env::var("DEEPSEEK_API_KEY").is_err() {
        std::env::set_var("DEEPSEEK_API_KEY", "test_key");
    }
    if std::env::var("GITHUB_TOKEN").is_err() {
        std::env::set_var("GITHUB_TOKEN", "test_token");
    }
    if std::env::var("FIRECRAWL_API_KEY").is_err() {
        std::env::set_var("FIRECRAWL_API_KEY", "test_key");
    }
    if std::env::var("OIDC_CLIENT_ID").is_err() {
        std::env::set_var("OIDC_CLIENT_ID", "test_client_id");
    }
}

#[tokio::test]
async fn test_login_page() {
    // Load environment variables
    setup_test_env();

    // Create mock server for DeepSeek API
    let mock_server = MockServer::start().await;

    // Mock the DeepSeek API response
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{
                "message": {
                    "content": "Hello! How can I help you?",
                    "role": "assistant"
                }
            }]
        })))
        .mount(&mock_server)
        .await;

    // Initialize the app
    let app = configure_app();

    // Create a request to the login page
    let request = Request::builder()
        .uri("/login")
        .header("Accept", "text/html")
        .body(Body::empty())
        .unwrap();

    // Send the request and get response
    let response = app.oneshot(request).await.unwrap();

    // Check status code
    assert_eq!(response.status(), StatusCode::OK);

    // Get response body
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Check for expected content
    assert!(body_str.contains("Log in to OpenAgents"));
    assert!(body_str.contains("Email address"));
    assert!(body_str.contains("Password"));
    assert!(body_str.contains("Remember me"));
    assert!(body_str.contains("Forgot your password?"));
}

#[tokio::test]
async fn test_signup_page() {
    // Load environment variables
    setup_test_env();

    // Create mock server for DeepSeek API
    let mock_server = MockServer::start().await;

    // Mock the DeepSeek API response
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{
                "message": {
                    "content": "Hello! How can I help you?",
                    "role": "assistant"
                }
            }]
        })))
        .mount(&mock_server)
        .await;

    // Initialize the app
    let app = configure_app();

    // Create a request to the signup page
    let request = Request::builder()
        .uri("/signup")
        .header("Accept", "text/html")
        .body(Body::empty())
        .unwrap();

    // Send the request and get response
    let response = app.oneshot(request).await.unwrap();

    // Check status code
    assert_eq!(response.status(), StatusCode::OK);

    // Get response body
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Check for expected content
    assert!(body_str.contains("Sign up for OpenAgents"));
    assert!(body_str.contains("Email address"));
    assert!(body_str.contains("Password"));
    assert!(body_str.contains("Confirm password"));
    assert!(body_str.contains("Terms of Service"));
}