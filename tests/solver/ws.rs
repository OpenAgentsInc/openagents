use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use openagents::server::{
    admin::routes::admin_routes,
    services::{solver::SolverService, solver_ws::{SolverStage, SolverUpdate}},
};
use std::{env, sync::Arc};
use tower::ServiceExt;

#[tokio::test]
async fn test_solver_ws_endpoint() {
    // Set up test environment
    env::set_var("AIDER_API_KEY", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key");
    env::set_var("GITHUB_TOKEN", "test_key");

    // Create app with solver service
    let solver_service = Arc::new(SolverService::new());
    let app = admin_routes();

    // Create test request
    let request = axum::http::Request::builder()
        .uri("/admin/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(axum::body::Body::empty())
        .unwrap();

    // Send request and get response
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn test_solver_ws_error_handling() {
    // Set up test environment with invalid keys
    env::set_var("AIDER_API_KEY", "invalid_key");
    env::set_var("OPENROUTER_API_KEY", "invalid_key");
    env::set_var("GITHUB_TOKEN", "invalid_key");

    // Create app with solver service
    let solver_service = Arc::new(SolverService::new());
    let app = admin_routes();

    // Create test request
    let request = axum::http::Request::builder()
        .uri("/admin/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(axum::body::Body::empty())
        .unwrap();

    // Send request and get response
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn test_solver_ws_connection_handling() {
    // Set up test environment
    env::set_var("AIDER_API_KEY", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key");
    env::set_var("GITHUB_TOKEN", "test_key");

    // Create app with solver service
    let solver_service = Arc::new(SolverService::new());
    let app = admin_routes();

    // Create test requests
    let request1 = axum::http::Request::builder()
        .uri("/admin/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(axum::body::Body::empty())
        .unwrap();

    let request2 = axum::http::Request::builder()
        .uri("/admin/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(axum::body::Body::empty())
        .unwrap();

    // Send requests and get responses
    let response1 = app.clone().oneshot(request1).await.unwrap();
    let response2 = app.oneshot(request2).await.unwrap();

    assert_eq!(response1.status(), axum::http::StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(response2.status(), axum::http::StatusCode::SWITCHING_PROTOCOLS);
}