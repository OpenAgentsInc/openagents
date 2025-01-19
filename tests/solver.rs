use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use openagents::{handle_solver, server::services::SolverService};
use std::{sync::Arc, env};
use tower::ServiceExt;

#[tokio::test]
async fn test_solver_endpoint() {
    // Ensure API keys are set for test
    env::set_var("AIDER_API_KEY", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key"); 
    env::set_var("GITHUB_TOKEN", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key");
    
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
    // Get required tokens from environment
    let github_token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set for tests");
    
    // Set up test environment with required API keys
    env::set_var("AIDER_API_KEY", "test_key"); // Mock API key ok for aider test
    env::set_var("OPENROUTER_API_KEY", "test_key"); // Mock API key ok for openrouter test
    env::set_var("GITHUB_TOKEN", &github_token); // Use real GitHub token
    
    let solver_service = SolverService::new();
    
    // Test both issue URL and repo URL formats
    let test_urls = vec![
        "https://github.com/OpenAgentsInc/openagents/issues/1",
        "https://github.com/OpenAgentsInc/openagents",
        "https://github.com/OpenAgentsInc/openagents/",
    ];

    for url in test_urls {
        let result = solver_service.solve_issue(url.to_string()).await.unwrap();
        println!("Response content for {}: {}", url, result.solution);
        assert!(result.solution.contains("Relevant files:"));
        assert!(result.solution.contains("Proposed solution:"));
        assert!(result.solution.len() > 30);
    }

    // Test invalid URL
    let result = solver_service
        .solve_issue("https://invalid.com/repo".to_string())
        .await;
    assert!(result.is_err());
}
