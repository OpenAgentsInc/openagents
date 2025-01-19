use axum::{routing::get, Router};
use axum_test::TestServer;
use openagents::server::{
    admin::routes::admin_routes,
    services::{solver::SolverService, solver_ws::{SolverStage, SolverUpdate}},
};
use std::{env, sync::Arc};

#[tokio::test]
async fn test_solver_ws_endpoint() {
    // Set up test environment
    env::set_var("AIDER_API_KEY", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key");
    env::set_var("GITHUB_TOKEN", "test_key");

    // Create app with solver service
    let solver_service = Arc::new(SolverService::new());
    let app = admin_routes();

    // Create test server
    let server = TestServer::builder()
        .build(app.into_make_service())
        .unwrap();

    // Test WebSocket connection
    let ws_client = server.ws("/admin/ws").await;
    assert!(ws_client.is_ok());

    let mut ws_client = ws_client.unwrap();

    // Send solver request
    ws_client
        .send_json(&serde_json::json!({
            "action": "start",
            "issue_url": "https://github.com/test/repo/issues/1"
        }))
        .await
        .unwrap();

    // Verify we receive progress updates
    let mut received_init = false;
    let mut received_repomap = false;

    while let Ok(msg) = ws_client.recv().await {
        let text = msg.to_string();
        if text.contains("Init") {
            received_init = true;
        }
        if text.contains("Repomap") {
            received_repomap = true;
            break;
        }
    }

    assert!(received_init);
    assert!(received_repomap);
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

    // Create test server
    let server = TestServer::builder()
        .build(app.into_make_service())
        .unwrap();

    // Test WebSocket connection
    let ws_client = server.ws("/admin/ws").await;
    assert!(ws_client.is_ok());

    let mut ws_client = ws_client.unwrap();

    // Send invalid request
    ws_client
        .send_json(&serde_json::json!({
            "action": "start",
            "issue_url": "invalid_url"
        }))
        .await
        .unwrap();

    // Verify we receive error message
    let mut received_error = false;
    while let Ok(msg) = ws_client.recv().await {
        let text = msg.to_string();
        if text.contains("Error") {
            received_error = true;
            break;
        }
    }

    assert!(received_error);
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

    // Create test server
    let server = TestServer::builder()
        .build(app.into_make_service())
        .unwrap();

    // Test multiple WebSocket connections
    let ws_client1 = server.ws("/admin/ws").await.unwrap();
    let ws_client2 = server.ws("/admin/ws").await.unwrap();

    // Verify both connections are established
    assert!(ws_client1.is_connected());
    assert!(ws_client2.is_connected());
}