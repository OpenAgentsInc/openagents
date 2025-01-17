use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use openagents::{
    server::services::RepomapService,
    repomap, generate_repomap,
};
use std::sync::Arc;
use tower::ServiceExt;

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
    let aider_api_key = "test_key".to_string();
    let repomap_service = Arc::new(RepomapService::new(aider_api_key));

    let app = Router::new()
        .route("/repomap/generate", post(generate_repomap))
        .with_state(repomap_service);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/repomap/generate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_url":"https://github.com/test/repo"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}