use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use openagents::server::config::configure_app;

#[tokio::test]
async fn test_error_component_included() {
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
    assert_eq!(response.status(), StatusCode::OK);

    // Get response body
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Check that error component is present but hidden
    assert!(html.contains(r#"id="auth-error""#));
    assert!(html.contains(r#"class="hidden"#));
    assert!(html.contains(r#"id="auth-error-message""#));
}

#[tokio::test]
async fn test_error_js_included() {
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
    assert_eq!(response.status(), StatusCode::OK);

    // Get response body
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Check that error handling JS is included
    assert!(html.contains("function showAuthError"));
    assert!(html.contains("function clearAuthError"));
    assert!(html.contains("function handleAuthError"));
}

#[tokio::test]
async fn test_error_component_accessibility() {
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
    assert_eq!(response.status(), StatusCode::OK);

    // Get response body
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Check accessibility attributes
    assert!(html.contains(r#"role="alert""#));
    assert!(html.contains(r#"role="button""#));
    assert!(html.contains(r#"<title>Close</title>"#));
}