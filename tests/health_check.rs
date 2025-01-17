use axum::{routing::get, Json, Router};
use axum_test::TestServer;
use serde_json::json;

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = Router::new().route(
        "/health",
        get(|| async { Json(json!({"status": "healthy"})) }),
    );

    // Create test server
    let server = TestServer::new(app.into_make_service()).unwrap();

    // Act
    let response = server.get("/health").await;

    // Assert
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "healthy");
}
