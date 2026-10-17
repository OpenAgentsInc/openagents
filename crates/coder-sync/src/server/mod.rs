pub mod auth;
pub mod blob;
pub mod chat;
pub mod config;
pub mod db;
pub mod device;
pub mod notifications;
pub mod push;
pub mod proof;
pub mod registry;
pub mod rooms;
pub mod storage;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use config::AuthMode;
use std::sync::Arc;

#[derive(Clone)]
pub struct StateData {
    pub database: db::Database,
    pub bucket: storage::Bucket,
    pub rooms: rooms::Rooms,
    pub devices: device::Devices,
    pub auth: AuthMode,
    pub accounts: Option<auth::Accounts>,
    pub revision: String,
    /// Session notifications; `None` when push isn't configured.
    pub push: Option<Arc<push::Push>>,
    pub proof_origin: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub account: String,
    pub workspace: Option<String>,
}

pub fn authenticate(mode: AuthMode, token: &str) -> Option<Identity> {
    if mode != AuthMode::Dev
        || token.is_empty()
        || token.len() > 256
        || token.chars().any(char::is_control)
    {
        return None;
    }
    let (account, workspace) = match token.split_once('@') {
        Some((a, w)) if !a.is_empty() && !w.is_empty() => (a, Some(w.to_owned())),
        Some(_) => return None,
        None => (token, None),
    };
    Some(Identity {
        account: account.to_owned(),
        workspace,
    })
}

pub fn router(state: StateData) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/health", get(health))
        .route("/blob/{id}/{part}", get(blob::read).put(blob::put))
        .route(
            "/attachments/{*path}",
            axum::routing::put(blob::retired_attachment),
        )
        .route(
            "/device/{id}/key",
            get(proof::read_key)
                .post(proof::register)
                .delete(proof::revoke),
        )
        .route("/device/{id}/ws", get(device::websocket))
        .route("/device/{id}/status", get(device::status))
        .route(
            "/device/{id}/nudge",
            axum::routing::post(device::nudge).layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .route(
            "/device/{id}/sidecar/{name}",
            get(device::sidecar)
                .post(device::publish_sidecar)
                .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024)),
        )
        .route(
            "/device/{id}/grants/{account}",
            axum::routing::put(device::grant).delete(device::revoke),
        )
        .route("/registry/{org}/ws", get(registry::websocket))
        .route("/registry/{org}/rows", get(registry::rows))
        .route(
            "/registry/{org}/push",
            axum::routing::post(registry::push)
                .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route("/registry/{org}/stats", get(registry::stats))
        .route(
            "/registry/{org}/reset",
            axum::routing::post(registry::reset),
        )
        .route("/registry/{org}/push-config", get(registry::push_config))
        .route(
            "/registry/{org}/push-grant",
            axum::routing::post(registry::push_grant)
                .delete(registry::delete_grant)
                .layer(axum::extract::DefaultBodyLimit::max(16384)),
        )
        .route(
            "/registry/{org}/push-target",
            axum::routing::post(registry::push_target)
                .delete(registry::delete_target)
                .layer(axum::extract::DefaultBodyLimit::max(16384)),
        )
        .route("/chat2/{id}", get(chat::websocket))
        .route("/chat2/{id}/ws", get(chat::websocket))
        .route("/chat2/{id}/rows", get(chat::rows).post(chat::push))
        .route(
            "/chat2/{id}/checkpoint",
            get(chat::checkpoint).post(chat::publish_checkpoint),
        )
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024 * 1024))
        .fallback(protected)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth::guard))
        .layer(middleware::from_fn(request_id))
}

async fn health(State(state): State<Arc<StateData>>) -> Response {
    if !state.database.healthy().await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok":false})),
        )
            .into_response();
    }
    Json(serde_json::json!({"ok":true, "service":"coder-sync", "auth": match state.auth { AuthMode::Disabled=>"disabled", AuthMode::Dev=>"dev", AuthMode::OpenAgents=>"openagents" }, "revision":state.revision})).into_response()
}

async fn protected(State(state): State<Arc<StateData>>, request: Request) -> Response {
    let header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let query_token = request.uri().query().and_then(|q| {
        reqwest::Url::parse(&format!("http://localhost/?{q}"))
            .ok()
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.into_owned())
            })
    });
    let token = header
        .and_then(|h| h.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, value)| value.trim().to_owned())
        .or(query_token);
    if state.auth == AuthMode::OpenAgents && request.headers().contains_key(auth::ACCOUNT_HEADER) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    }
    if token
        .as_deref()
        .and_then(|t| authenticate(state.auth, t))
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"unauthenticated"})),
        )
            .into_response();
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error":"not_found"})),
    )
        .into_response()
}

async fn request_id(request: Request, next: Next) -> Response {
    // Generate IDs here; never put bearer query parameters or headers in logs.
    let id = uuid::Uuid::new_v4().to_string();
    let method = request.method().to_string();
    let started = std::time::Instant::now();
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&id).expect("UUID is a header value"),
    );
    tracing::info!(request_id=%id, method, status=response.status().as_u16(), elapsed_ms=started.elapsed().as_millis() as u64, "HTTP request");
    response
}
