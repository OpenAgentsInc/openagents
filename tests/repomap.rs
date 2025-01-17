use axum::{
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
    routing::{get, post},
    Router,
};
use openagents::{
    server::services::RepomapService,
    repomap, generate_repomap,
};
use std::sync::Arc;
use tower::ServiceExt;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use serde_json::json;

#[tokio::test]
async fn test_get_repomap() {
    let app = Router::new()
        .route("/repomap", get(repomap));

    let response = app
        .oneshot(Request::builder().uri("/repomap").body(Body::empty()).unwrap())
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
    let mut repomap_service = RepomapService::new(aider_api_key);
    repomap_service.set_base_url(&mock_server.uri());
    let repomap_service = Arc::new(repomap_service);

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