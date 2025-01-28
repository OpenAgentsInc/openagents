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

#[tokio::test]
async fn test_login_page() {
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

    // Set environment variables for testing
    std::env::set_var("DEEPSEEK_API_KEY", "test_key");
    std::env::set_var("GITHUB_TOKEN", "test_token");
    std::env::set_var("FIRECRAWL_API_KEY", "test_key");

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

    // Set environment variables for testing
    std::env::set_var("DEEPSEEK_API_KEY", "test_key");
    std::env::set_var("GITHUB_TOKEN", "test_token");
    std::env::set_var("FIRECRAWL_API_KEY", "test_key");

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
