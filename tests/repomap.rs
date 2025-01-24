use axum::{
    routing::{get, post},
    Router,
};
use openagents::server::services::RepomapService;

#[tokio::test]
async fn test_repomap_endpoint() {
    // Create a new router with the repomap endpoint
    let app = Router::new()
        .route("/repomap", post(handle_repomap));

    // Create a test client
    let client = axum_test::TestClient::new(app);

    // Send a test request
    let response = client.post("/repomap")
        .json(&serde_json::json!({
            "repo": "test/repo",
            "path": "src/main.rs"
        }))
        .send()
        .await;

    // Assert the response
    assert_eq!(response.status(), 200);
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