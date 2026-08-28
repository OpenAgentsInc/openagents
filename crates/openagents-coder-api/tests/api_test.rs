use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use openagents_coder_api::{router, App, Config, Store};
use tempfile::tempdir;
use tower::ServiceExt;

fn test_app(gateway_key: Option<&str>) -> App {
    let dir = tempdir().unwrap();
    let db = dir.path().join("state.sqlite");
    // Keep the tempdir alive by leaking; tests are short-lived processes.
    std::mem::forget(dir);
    let store = Store::open(&db).unwrap();
    App {
        config: Config {
            bind: "127.0.0.1:0".into(),
            db_path: db,
            public_origin: "http://127.0.0.1:4010".into(),
            ai_gateway_api_key: gateway_key.map(str::to_string),
            openrouter_api_key: None,
            credit_allowance_microusd: 20_000_000,
            require_bearer: false,
        },
        store: Arc::new(store),
    }
}

async fn json(
    app: App,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    let response = router(app).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, value)
}

#[tokio::test]
async fn a_public_host_refuses_unsigned_api_calls_but_health_stays_open() {
    let mut app = test_app(None);
    app.config.require_bearer = true;
    let (status, _) = json(app.clone(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = json(
        app,
        "POST",
        "/api/v1/threads",
        Some(serde_json::json!({"objective":"x","lane":"thread"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn health_names_this_process() {
    let (status, body) = json(test_app(None), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["service"], "openagents-coder-api");
}

#[tokio::test]
async fn catalog_lists_gemini_unavailable_without_a_gateway_key() {
    let (status, body) = json(test_app(None), "GET", "/api/v1/models", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["default"], "glm-5.3-flash");
    let gemini = body["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "gemini-3.7-flash")
        .unwrap();
    assert_eq!(gemini["availability"], "unavailable");
}

#[tokio::test]
async fn catalog_lists_gemini_available_with_a_gateway_key() {
    let (status, body) = json(test_app(Some("test-key")), "GET", "/api/v1/models", None).await;
    assert_eq!(status, StatusCode::OK);
    let gemini = body["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "gemini-3.7-flash")
        .unwrap();
    assert_eq!(gemini["availability"], "available");
}

#[tokio::test]
async fn opening_a_thread_without_a_provider_is_model_unavailable() {
    let (status, body) = json(
        test_app(None),
        "POST",
        "/api/v1/threads",
        Some(serde_json::json!({
            "objective": "test",
            "lane": "thread",
            "model": "gemini-3.7-flash"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "model_unavailable");
}

#[tokio::test]
async fn opening_a_thread_with_a_configured_gateway_mints_a_grant() {
    let (status, body) = json(
        test_app(Some("test-key")),
        "POST",
        "/api/v1/threads",
        Some(serde_json::json!({
            "objective": "test",
            "lane": "thread",
            "model": "gemini-3.7-flash"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["grant"]["model"], "gemini-3.7-flash");
    assert!(body["grant"]["token"].as_str().unwrap().starts_with("sig_"));
    assert_eq!(
        body["grant"]["url"],
        "http://127.0.0.1:4010/api/inference/proxy"
    );
    assert!(body["thread"]["id"].as_str().unwrap().starts_with("th_"));
}

#[tokio::test]
async fn a_local_lane_mints_nothing() {
    let (status, body) = json(
        test_app(None),
        "POST",
        "/api/v1/threads",
        Some(serde_json::json!({"objective": "offline", "lane": "local"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body.get("grant").is_none());
}

#[tokio::test]
async fn credit_is_an_envelope() {
    let (status, body) = json(test_app(None), "GET", "/api/v1/credit", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["credit"]["allowance_microusd"], 20_000_000);
    assert_eq!(body["credit"]["complete"], true);
}

#[tokio::test]
async fn proxy_without_a_grant_is_revoked() {
    let (status, body) = json(
        test_app(Some("test-key")),
        "POST",
        "/api/inference/proxy",
        Some(serde_json::json!({"model": "glm-5.3-flash", "messages": []})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "grant_revoked");
}

#[tokio::test]
async fn health_reports_provider_configuration_without_keys() {
    let (status, body) = json(test_app(None), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["providers"]["vercel_gateway"], false);
    assert_eq!(body["providers"]["openrouter"], false);
}

#[tokio::test]
async fn user_without_a_bearer_is_unauthorized() {
    let (status, body) = json(test_app(None), "GET", "/api/v1/user", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn a_bearer_resolves_as_the_local_principal() {
    let app = test_app(None);
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/user")
        .header("authorization", "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let response = router(app).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["login"], "local");
}

#[tokio::test]
async fn a_grant_refuses_a_mismatched_model() {
    let app = test_app(Some("test-key"));
    let (status, opened) = json(
        app.clone(),
        "POST",
        "/api/v1/threads",
        Some(serde_json::json!({
            "objective": "test",
            "lane": "thread",
            "model": "gemini-3.7-flash"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{opened}");
    let token = opened["grant"]["token"].as_str().unwrap();
    let request = Request::builder()
        .method("POST")
        .uri("/api/inference/proxy")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            serde_json::json!({"model": "glm-5.3-flash", "messages": []}).to_string(),
        ))
        .unwrap();
    let response = router(app).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "model_mismatch");
}

#[allow(dead_code)]
fn _timeout() -> Duration {
    Duration::from_secs(1)
}
