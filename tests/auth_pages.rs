use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use openagents::server::config::configure_app;

#[tokio::test]
async fn test_login_page() {
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
    let body = to_bytes(response.into_body()).await.unwrap();
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
    let body = to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Check for expected content
    assert!(body_str.contains("Sign up for OpenAgents"));
    assert!(body_str.contains("Email address"));
    assert!(body_str.contains("Password"));
    assert!(body_str.contains("Confirm password"));
    assert!(body_str.contains("Terms of Service"));
}