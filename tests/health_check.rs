use axum::{
    routing::get,
    Router,
    Json,
};
use serde_json::json;
use tower::ServiceExt;
use http::Request;

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "healthy"})) }));

    // Act
    let response = app
        .oneshot(Request::builder().uri("/health").body(()).unwrap())
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "healthy");
}