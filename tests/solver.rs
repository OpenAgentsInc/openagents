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

#[tokio::test]
async fn test_solver_generates_repomap() {
    // Set up test environment
    env::set_var("AIDER_API_KEY", "test_key");
    
    let solver_service = SolverService::new();
    
    // Test both issue URL and repo URL formats
    let test_urls = vec![
        "https://github.com/OpenAgentsInc/openagents/issues/1",
        "https://github.com/OpenAgentsInc/openagents",
        "https://github.com/OpenAgentsInc/openagents/",
    ];

    for url in test_urls {
        let result = solver_service.solve_issue(url.to_string()).await.unwrap();
        assert!(result.solution.contains("Repository Map Preview:"));
        assert!(result.solution.len() > 30);
    }

    // Test invalid URL
    let result = solver_service
        .solve_issue("https://invalid.com/repo".to_string())
        .await;
    assert!(result.is_err());
}
