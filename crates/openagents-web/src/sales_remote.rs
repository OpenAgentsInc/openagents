//! Loopback HTTP transport for the resident sales-owner adapter
//! ([`coder::task::sales::remote`]). It runs on the sales-owner host beside
//! the private pipeline; a deployment terminates authenticated TLS before
//! this listener. Browser requests (any `Origin` or cookie) are refused, so
//! only a server-side caller holding a per-binding bearer reaches it.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use coder::task::sales::remote::{self, Service};
use serde_json::json;
use std::sync::Arc;

pub const PATH: &str = "/v1/sales";

pub fn router(service: Arc<Service>) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({"service":remote::RESPONSE_SCHEMA,"alive":true})) }),
        )
        .route(PATH, post(call))
        .layer(DefaultBodyLimit::max(remote::BODY_MAX))
        .with_state(service)
}

async fn call(State(service): State<Arc<Service>>, headers: HeaderMap, body: Bytes) -> Response {
    if headers.contains_key(header::ORIGIN) || headers.contains_key(header::COOKIE) {
        return answer(
            remote::Code::AccessDenied.status(),
            json!({"schema":remote::RESPONSE_SCHEMA,"error":"access_denied"}),
        );
    }
    let binding = headers
        .get("x-sales-binding")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mut bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_owned();
    let reply = tokio::task::spawn_blocking(move || {
        let reply = service.call(&binding, &bearer, &body);
        let mut bytes = std::mem::take(&mut bearer).into_bytes();
        bytes.fill(0);
        reply
    })
    .await;
    match reply {
        Ok(reply) => answer(reply.status, reply.body),
        Err(_) => answer(
            503,
            json!({"schema":remote::RESPONSE_SCHEMA,"error":"unavailable"}),
        ),
    }
}

fn answer(status: u16, body: serde_json::Value) -> Response {
    let mut response = (
        StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        Json(body),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
