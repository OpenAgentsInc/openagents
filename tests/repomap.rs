use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    routing::{get, post},
    Router,
};
use openagents::{generate_repomap, repomap, server::services::RepomapService};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_get_repomap() {
    let app = Router::new().route("/repomap", get(repomap));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/repomap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_generate_repomap() {
    // Start mock server
    let mock_server = MockServer::start().await;

    // Create mock response
    Mock::given(method("POST"))
        .and(path("/api/v1/repomap/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repo_map": "# Test Repository Map\n\nThis is a test map.",
            "metadata": {}
        })))
        .mount(&mock_server)
        .await;

    // Create service with mock server URL
    let aider_api_key = "test_key".to_string();
    let repomap_service = Arc::new(RepomapService::with_base_url(
        aider_api_key,
        mock_server.uri(),
    ));

    let app = Router::new()
        .route("/repomap/generate", post(generate_repomap))
        .with_state(repomap_service);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/repomap/generate")
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("repo_url=https://github.com/test/repo"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
