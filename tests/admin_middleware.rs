use axum::{routing::get, Router};
use axum_test::TestServer;
use openagents::server::admin::middleware::admin_auth;
use serde_json::json;

async fn test_endpoint() -> axum::Json<serde_json::Value> {
    axum::Json(json!({"status": "ok"}))
}

#[tokio::test]
async fn test_admin_auth_valid_token() {
    let app = Router::new()
        .route("/admin/test", get(test_endpoint))
        .layer(axum::middleware::from_fn(admin_auth));

    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/admin/test")
        .add_header("Authorization", "Bearer admin-token")
        .await;

    assert_eq!(response.status_code(), 200);
}

#[tokio::test]
async fn test_admin_auth_invalid_token() {
    std::env::set_var("APP_ENVIRONMENT", "production");

    let app = Router::new()
        .route("/admin/test", get(test_endpoint))
        .layer(axum::middleware::from_fn(admin_auth));

    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/admin/test")
        .add_header("Authorization", "Bearer wrong-token")
        .await;

    assert_eq!(response.status_code(), 401);

    std::env::remove_var("APP_ENVIRONMENT");
}

#[tokio::test]
async fn test_admin_auth_missing_token() {
    let original_env = std::env::var("APP_ENVIRONMENT").ok();
    std::env::set_var("APP_ENVIRONMENT", "production");

    let app = Router::new()
        .route("/admin/test", get(test_endpoint))
        .layer(axum::middleware::from_fn(admin_auth));

    let server = TestServer::new(app).unwrap();

    let response = server.get("/admin/test").await;

    assert_eq!(response.status_code(), 401);

    // Restore original environment
    if let Some(env) = original_env {
        std::env::set_var("APP_ENVIRONMENT", env);
    } else {
        std::env::remove_var("APP_ENVIRONMENT");
    }
}