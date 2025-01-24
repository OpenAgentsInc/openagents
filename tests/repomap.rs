use axum::{
    routing::post,
    Router,
    http::{Request, StatusCode},
    body::Body,
};
use openagents::server::services::RepomapService;
use tower::ServiceExt;

#[tokio::test]
async fn test_repomap_endpoint() {
    // Create a new router with the repomap endpoint
    let app = Router::new()
        .route("/repomap", post(handle_repomap));

    // Create test request
    let request = Request::builder()
        .method("POST")
        .uri("/repomap")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "repo": "test/repo",
                "path": "src/main.rs"
            }).to_string()
        ))
        .unwrap();

    // Send request and get response
    let response = app
        .oneshot(request)
        .await
        .unwrap();

    // Assert the response
    assert_eq!(response.status(), StatusCode::OK);
}

async fn handle_repomap(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    let service = RepomapService::new("test_key".to_string());
    match service.generate_repomap(body.to_string()).await {
        Ok(result) => axum::Json(serde_json::json!({ "result": result })),
        Err(e) => axum::Json(serde_json::json!({ "error": e.to_string() })),
    }
}