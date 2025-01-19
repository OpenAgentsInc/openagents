use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use openagents::{handle_solver, server::services::SolverService};
use std::{sync::Arc, env};
use tower::ServiceExt;

#[tokio::test]
async fn test_solver_endpoint() {
    // Ensure AIDER_API_KEY is set for test
    env::set_var("AIDER_API_KEY", "test_key");
    
    // Create app with solver service
    let solver_service = Arc::new(SolverService::new());
    let app = axum::Router::new()
        .route("/", axum::routing::post(handle_solver))
        .with_state(solver_service);

    // Create test request
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from("issue_url=https://github.com/test/repo/issues/1"))
        .unwrap();

    // Send request and get response
    let response = app.oneshot(request).await.unwrap();

    // Assert status
    assert_eq!(response.status(), StatusCode::OK);
}
