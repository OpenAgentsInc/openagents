use axum::{body::Body, http::Request, routing::get, Json, Router};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = Router::new().route(
        "/health",
        get(|| async { Json(json!({"status": "healthy"})) }),
    );

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);

    // Use a reasonable size limit for the health check response (1MB)
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "healthy");
}
