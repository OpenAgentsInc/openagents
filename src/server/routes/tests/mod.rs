use axum::{
    body::Body,
    extract::connect_info::MockConnectInfo,
    http::{Request, StatusCode},
};
use std::net::SocketAddr;
use tower::ServiceExt;

use crate::server::{
    routes::chat::routes,
    ws::handlers::chat::ChatHandler,
};

#[tokio::test]
async fn test_chat_home() {
    let app = routes();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_chat_session() {
    let app = routes();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/123e4567-e89b-12d3-a456-426614174000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_tool_toggle() {
    let app = routes();

    // Test enabling tool
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tools/toggle")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("tool=test_tool&enabled=true"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test disabling tool
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tools/toggle")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("tool=test_tool&enabled=false"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_tool_toggle_invalid() {
    let app = routes();

    // Test with missing tool name
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tools/toggle")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("enabled=true"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}