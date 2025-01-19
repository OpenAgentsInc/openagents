use axum::{
    routing::get,
    Router,
    Json,
    http::StatusCode,
};
use openagents::server::services::GitHubService;
use serde_json::json;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_github_api() {
    // Start mock server
    let app = Router::new().route(
        "/repos/test-owner/test-repo/issues/1", 
        get(mock_github_handler)
    );
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let _addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Test URL parsing
    let (owner, repo, number) = GitHubService::parse_issue_url(
        "https://github.com/test-owner/test-repo/issues/1"
    ).unwrap();
    assert_eq!(owner, "test-owner");
    assert_eq!(repo, "test-repo");
    assert_eq!(number, 1);

    // Test invalid URL
    assert!(GitHubService::parse_issue_url("https://github.com/invalid").is_err());

    // Create service with base URL pointing to mock server
    let service = GitHubService::new_with_base_url(
        "mock_token".to_string(),
        format!("http://{}", addr)
    );
    
    // Test getting issue
    let issue = service.get_issue("test-owner", "test-repo", 1).await.unwrap();
    assert_eq!(issue.title, "Test Issue");
    assert_eq!(issue.body, "Test description");
    assert_eq!(issue.number, 1);
    assert_eq!(issue.state, "open");
}

async fn mock_github_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "number": 1,
            "title": "Test Issue",
            "body": "Test description",
            "state": "open"
        }))
    )
}
