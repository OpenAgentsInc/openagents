//! The HTTP surface: Boat's v1 paths under `/api/v1`, in Boat's JSON shapes
//! (the SDK's own models), with Boat's error envelope.
//!
//! Every path but `/healthz` needs `Authorization: Bearer <token>`. The
//! operations Boat has that this service does not (integrated agents,
//! desktops, hosted ports, webhooks, keys) answer 501 `not_supported`.

use crate::gce::Compute;
use crate::remote::Remote;
use crate::service::{ApiErr, Reply, Res, Service};
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use boat::models::*;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// The bearer token every call must carry, compared by digest.
#[derive(Clone)]
pub struct Token(Arc<[u8; 32]>);

impl Token {
    pub fn new(secret: &str) -> Self {
        Self(Arc::new(sha(secret.trim())))
    }
}

fn sha(s: &str) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(s.as_bytes()).into()
}

fn request_id() -> String {
    use rand::Rng;
    format!("req_{:016x}", rand::rng().random::<u64>())
}

impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::BAD_GATEWAY);
        let body = json!({
            "ok": false,
            "type": "error",
            "status": self.status,
            "code": self.code,
            "message": self.message,
            "requestId": request_id(),
            "error": {"code": self.code, "message": self.message, "status": self.status},
        });
        (status, axum::Json(body)).into_response()
    }
}

fn ok<T: serde::Serialize>(r: Res<T>) -> Response {
    match r {
        Ok(v) => axum::Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn auth(State(token): State<Token>, req: Request, next: Next) -> Response {
    if req.uri().path() == "/healthz" {
        return next.run(req).await;
    }
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| sha(v.trim()));
    // Digests of equal length, compared without an early exit.
    let good = given.is_some_and(|g| {
        g.iter()
            .zip(token.0.iter())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0
    });
    if !good {
        return ApiErr::new(401, "unauthorized", "A valid API key is required.").into_response();
    }
    next.run(req).await
}

type S<C, R> = State<Arc<Service<C, R>>>;

fn body<T: for<'de> Deserialize<'de> + Default>(bytes: &axum::body::Bytes) -> Res<T> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(bytes).map_err(|_| {
        ApiErr::new(
            400,
            "invalid_request",
            "The request body is not valid JSON for this operation.",
        )
    })
}

fn header<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

async fn healthz() -> Response {
    axum::Json(json!({"ok": true, "service": "oa-boat"})).into_response()
}

async fn limits<C: Compute, R: Remote>(State(s): S<C, R>) -> Response {
    ok(s.limits().await)
}

#[derive(Deserialize, Default)]
struct ListQuery {
    state: Option<String>,
    limit: Option<i64>,
}

async fn list<C: Compute, R: Remote>(State(s): S<C, R>, Query(q): Query<ListQuery>) -> Response {
    ok(s.list(q.state.as_deref(), q.limit).await)
}

async fn create<C: Compute, R: Remote>(
    State(s): S<C, R>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let req: CreateSandboxRequest = match body(&bytes) {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let key = header(&headers, "idempotency-key").map(str::to_owned);
    match s
        .create(req, key, header(&headers, "x-oa-provisioning"))
        .await
    {
        Ok(v) => (StatusCode::CREATED, axum::Json(v)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_one<C: Compute, R: Remote>(State(s): S<C, R>, Path(id): Path<String>) -> Response {
    ok(s.get(&id).await)
}

async fn patch_one<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    match body::<UpdateSandboxRequest>(&bytes) {
        Ok(r) => ok(s.update(&id, r).await),
        Err(e) => e.into_response(),
    }
}

async fn delete_one<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    match s
        .delete(&id, header(&headers, "x-ascii-confirm-delete"))
        .await
    {
        Ok(v) => (StatusCode::ACCEPTED, axum::Json(v)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn stop<C: Compute, R: Remote>(State(s): S<C, R>, Path(id): Path<String>) -> Response {
    ok(s.stop(&id).await)
}

async fn resume<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    match body::<ResumeRequest>(&bytes) {
        Ok(r) => ok(s.resume(&id, r).await),
        Err(e) => e.into_response(),
    }
}

async fn fork<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let req: ForkParamsBody = match body(&bytes) {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let key = header(&headers, "idempotency-key").map(str::to_owned);
    ok(s.fork(&id, req, key, header(&headers, "x-oa-provisioning"))
        .await)
}

async fn command<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let req: CommandRequest = match body(&bytes) {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    match s.command(&id, req).await {
        Err(e) => e.into_response(),
        Ok(Reply::Finished(r)) => axum::Json(r).into_response(),
        Ok(Reply::Started(r)) => (StatusCode::ACCEPTED, axum::Json(r)).into_response(),
        Ok(Reply::Stream(rx)) => {
            let stream = futures_util::stream::unfold(rx, |mut rx| async move {
                let frame = rx.recv().await?;
                let mut line = serde_json::to_vec(&frame).unwrap_or_default();
                line.push(b'\n');
                Some((Ok::<_, std::io::Error>(bytes::Bytes::from(line)), rx))
            });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-ndjson")
                .header("cache-control", "no-cache")
                .header("x-accel-buffering", "no")
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StatusQuery {
    tail_bytes: Option<i64>,
}

async fn command_status<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path((id, pid)): Path<(String, i64)>,
    Query(q): Query<StatusQuery>,
) -> Response {
    ok(s.command_status(&id, pid, q.tail_bytes).await)
}

#[derive(Deserialize, Default)]
struct FileQuery {
    path: Option<String>,
    encoding: Option<String>,
}

async fn read_file<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    Query(q): Query<FileQuery>,
) -> Response {
    let Some(path) = q.path.filter(|p| !p.is_empty()) else {
        return ApiErr::new(400, "invalid_path", "path is required.").into_response();
    };
    ok(s.read_file(&id, &path, q.encoding.as_deref()).await)
}

async fn write_file<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    match body::<FileWriteRequest>(&bytes) {
        Ok(r) if !r.path.is_empty() => ok(s.write_file(&id, r).await),
        Ok(_) => ApiErr::new(400, "invalid_path", "path is required.").into_response(),
        Err(e) => e.into_response(),
    }
}

async fn usage<C: Compute, R: Remote>(State(s): S<C, R>, Path(id): Path<String>) -> Response {
    ok(s.usage(&id).await)
}

async fn latest<C: Compute, R: Remote>(State(s): S<C, R>, Path(id): Path<String>) -> Response {
    ok(s.latest_snapshot(&id).await)
}

async fn deletion<C: Compute, R: Remote>(State(s): S<C, R>, Path(op): Path<String>) -> Response {
    ok(s.deletion(&op).await)
}

async fn named_list<C: Compute, R: Remote>(State(s): S<C, R>) -> Response {
    ok(s.list_named().await)
}

async fn named_save<C: Compute, R: Remote>(
    State(s): S<C, R>,
    bytes: axum::body::Bytes,
) -> Response {
    match body::<NamedSnapshotSaveRequest>(&bytes) {
        Ok(r) => match s.save_named(r).await {
            Ok(v) => (StatusCode::ACCEPTED, axum::Json(v)).into_response(),
            Err(e) => e.into_response(),
        },
        Err(e) => e.into_response(),
    }
}

async fn named_get<C: Compute, R: Remote>(State(s): S<C, R>, Path(name): Path<String>) -> Response {
    ok(s.get_named(&name).await)
}

async fn named_delete<C: Compute, R: Remote>(
    State(s): S<C, R>,
    Path(name): Path<String>,
) -> Response {
    ok(s.delete_named(&name).await)
}

async fn environments() -> Response {
    axum::Json(json!({"ok": true, "type": "environments", "environments": []})).into_response()
}

async fn unsupported() -> Response {
    ApiErr::new(
        501,
        "not_supported",
        "This operation is not offered by the OpenAgents sandbox service.",
    )
    .into_response()
}

async fn missing() -> Response {
    ApiErr::new(404, "not_found", "No such operation.").into_response()
}

/// The router: Boat's paths under `/api/v1`, and `/healthz`.
pub fn router<C: Compute, R: Remote>(service: Arc<Service<C, R>>, token: Token) -> Router {
    let v1 = Router::new()
        .route("/limits", get(limits::<C, R>))
        .route("/environments", get(environments))
        .route("/sandboxes", get(list::<C, R>).post(create::<C, R>))
        .route(
            "/sandboxes/{id}",
            get(get_one::<C, R>)
                .patch(patch_one::<C, R>)
                .delete(delete_one::<C, R>),
        )
        .route("/sandboxes/{id}/stop", post(stop::<C, R>))
        .route("/sandboxes/{id}/resume", post(resume::<C, R>))
        .route("/sandboxes/{id}/fork", post(fork::<C, R>))
        .route("/sandboxes/{id}/commands", post(command::<C, R>))
        .route(
            "/sandboxes/{id}/commands/{pid}",
            get(command_status::<C, R>),
        )
        .route(
            "/sandboxes/{id}/files",
            get(read_file::<C, R>).put(write_file::<C, R>),
        )
        .route("/sandboxes/{id}/usage", get(usage::<C, R>))
        .route("/sandboxes/{id}/snapshots/latest", get(latest::<C, R>))
        .route("/deletion-operations/{op}", get(deletion::<C, R>))
        .route(
            "/named-snapshots",
            get(named_list::<C, R>).post(named_save::<C, R>),
        )
        .route(
            "/named-snapshots/{name}",
            get(named_get::<C, R>).delete(named_delete::<C, R>),
        )
        .route("/sandboxes/{id}/prompt", any(unsupported))
        .route("/sandboxes/{id}/steer", any(unsupported))
        .route("/sandboxes/{id}/interrupt", any(unsupported))
        .route("/sandboxes/{id}/events", any(unsupported))
        .route("/sandboxes/{id}/desktop", any(unsupported))
        .route("/sandboxes/{id}/host", any(unsupported))
        .route("/sandboxes/{id}/sshkey", any(unsupported))
        .route("/sandboxes/{id}/share", any(unsupported))
        .fallback(missing)
        .with_state(service);
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", v1)
        .fallback(missing)
        .layer(DefaultBodyLimit::max(80 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(token, auth))
}
