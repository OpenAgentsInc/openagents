pub mod auth;
pub mod blob;
pub mod chat;
pub mod config;
pub mod db;
pub mod device;
pub mod notifications;
pub mod owner_notifications;
pub mod proof;
pub mod push;
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
        .route(
            "/v1/notify",
            get(owner_notifications::latest)
                .post(owner_notifications::notify)
                .layer(axum::extract::DefaultBodyLimit::max(8192)),
        )
        .route(
            "/v1/notify/grants/{agent}",
            axum::routing::put(owner_notifications::grant),
        )
        .route(
            "/v1/notify/{id}/read",
            axum::routing::post(owner_notifications::read),
        )
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
        .route(
            "/chat2/{id}/checkpoint/part",
            axum::routing::post(chat::publish_checkpoint_part),
        )
        // Retired edge sidecar: shipped apps still PUT multi-MB tails here.
        .route("/chat2/{id}/tail", axum::routing::put(retired_tail))
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
    Json(serde_json::json!({"ok":true, "service":"coder-sync", "auth": match state.auth { AuthMode::Disabled=>"disabled", AuthMode::Dev=>"dev", AuthMode::OpenAgents=>"openagents" }, "revision":state.revision, "cached_admissions":auth::cached_admissions()})).into_response()
}

/// Upper bound on bytes read from a body we are about to refuse. Cloud Run
/// caps HTTP/1 requests at 32 MiB, so this covers every request it forwards.
const DRAIN_LIMIT: usize = 33 * 1024 * 1024;
const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Read and discard a request body before answering it. Answering early
/// leaves the upload unread, hyper resets the connection, and the Cloud Run
/// frontend then stops routing to this instance while its sockets keep it
/// counted as active: "no available instance" 429s (#12128).
pub(crate) async fn drain(body: axum::body::Body) {
    use axum::body::HttpBody;
    let mut body = std::pin::pin!(body);
    let mut seen = 0usize;
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, async {
        while let Some(Ok(frame)) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
            seen += frame.data_ref().map_or(0, |data| data.len());
            if seen > DRAIN_LIMIT {
                break;
            }
        }
    })
    .await;
}

async fn retired_tail(request: Request) -> Response {
    drain(request.into_body()).await;
    (StatusCode::GONE, Json(serde_json::json!({"error":"gone"}))).into_response()
}

async fn protected(State(state): State<Arc<StateData>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    drain(body).await;
    let request = Request::from_parts(parts, axum::body::Body::empty());
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// #12128: a multi-MB PUT to a route we refuse must not reset the
    /// connection; the next request on the same socket still gets an answer.
    #[tokio::test]
    async fn refused_uploads_keep_the_connection_open() {
        let app = router(StateData {
            proof_origin: None,
            database: db::Database::new("host=localhost dbname=unused").unwrap(),
            bucket: storage::Bucket::memory(),
            rooms: rooms::Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Disabled,
            accounts: None,
            revision: "fixture".into(),
            push: None,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let body = vec![b'x'; 6 * 1024 * 1024];
        let mut wire = Vec::new();
        for path in ["/chat2/whale/tail", "/no-such-route"] {
            wire.extend(
                format!(
                    "PUT {path} HTTP/1.1\r\nhost: sync\r\ncontent-length: {}\r\n\r\n",
                    body.len()
                )
                .into_bytes(),
            );
            wire.extend_from_slice(&body);
        }
        wire.extend_from_slice(
            b"GET /no-such-route HTTP/1.1\r\nhost: sync\r\nconnection: close\r\n\r\n",
        );
        let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (mut read, mut write) = socket.into_split();
        // Keep the write half open until the replies are read: dropping it
        // half-closes the socket, and hyper drops pipelined requests on EOF.
        let writer = tokio::spawn(async move { write.write_all(&wire).await.map(|()| write) });
        let mut replies = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            read.read_to_end(&mut replies),
        )
        .await
        .unwrap()
        .unwrap();
        drop(writer.await.unwrap().unwrap());
        let replies = String::from_utf8_lossy(&replies);
        // Bodies end without a newline, so find status lines by their prefix.
        let statuses: Vec<&str> = replies
            .match_indices("HTTP/1.1 ")
            .filter_map(|(at, _)| replies[at..].lines().next())
            .collect();
        assert_eq!(
            statuses,
            [
                "HTTP/1.1 410 Gone",
                "HTTP/1.1 401 Unauthorized",
                "HTTP/1.1 401 Unauthorized"
            ]
        );
    }
}
