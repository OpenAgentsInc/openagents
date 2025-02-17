use axum::{routing::get, Router};
use axum_test::TestServer;

#[tokio::test]
async fn test_health_check() {
    // Create a simple router with just the health check endpoint
    let app = Router::new().route("/health", get(|| async { "OK" }));
    let server = TestServer::new(app.into_make_service()).unwrap();

    // Test health check endpoint
    let response = server.get("/health").await;
    assert_eq!(response.status_code(), 200);
    assert_eq!(response.text(), "OK");
}
