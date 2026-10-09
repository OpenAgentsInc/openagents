//! Loopback HTTP JSON transport. A deployment terminates authenticated TLS
//! before this listener; the service never exposes an unauthenticated proxy.

use crate::{
    Backend, Error, Service,
    types::{BODY_MAX, Request, SCHEMA},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use openagents_wallet::LightningWallet;
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub fn router<B: Backend, W: LightningWallet + Send + Sync + 'static>(
    service: Arc<Service<B, W>>,
) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({"service":SCHEMA,"alive":true})) }),
        )
        .route("/v1/retail", post(call::<B, W>))
        .route("/operator/status", get(operator::<B, W>))
        .route_layer(middleware::from_fn_with_state(
            Arc::new(tokio::sync::Semaphore::new(32)),
            bounded,
        ))
        .layer(DefaultBodyLimit::max(BODY_MAX))
        .with_state(service)
}
async fn operator<B: Backend, W: LightningWallet + Send + Sync + 'static>(
    State(service): State<Arc<Service<B, W>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if headers.contains_key("origin") {
        return response(Err(Error::Denied));
    }
    let secret = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_owned();
    let result = tokio::task::spawn_blocking(move || service.operator_status(&secret, now()))
        .await
        .unwrap_or(Err(Error::Unavailable("operator status unavailable")));
    response(result)
}
async fn bounded(
    State(slots): State<Arc<tokio::sync::Semaphore>>,
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let Ok(_permit) = slots.try_acquire_owned() else {
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"schema":SCHEMA,"error":"busy"})),
        )
            .into_response();
        response.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static("no-store"),
        );
        return response;
    };
    next.run(request).await
}
async fn call<B: Backend, W: LightningWallet + Send + Sync + 'static>(
    State(service): State<Arc<Service<B, W>>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // No cookies or browser ambient authority. Cross-origin browser requests
    // must not carry credentials to the customer's loopback service.
    if headers.contains_key("origin") {
        return response(Err(Error::Denied));
    }
    let principal = headers
        .get("x-retail-principal")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mut secret = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_owned();
    let request = match serde_json::from_slice::<Request>(&body) {
        Ok(request) => request,
        Err(_) => return response(Err(Error::Invalid("invalid bounded retail request"))),
    };
    let result = tokio::task::spawn_blocking(move || {
        let result = service.call(&principal, &secret, request, now());
        let mut bytes = std::mem::take(&mut secret).into_bytes();
        bytes.fill(0);
        result
    })
    .await
    .unwrap_or(Err(Error::Unavailable("retail operation is unavailable")));
    response(result)
}
fn response(result: crate::Result<serde_json::Value>) -> axum::response::Response {
    let (status, value) = match result {
        Ok(value) => (StatusCode::OK, value),
        Err(error) => {
            // Environment refusals carry the plain sentence a person reads.
            let message = match &error {
                Error::Lifecycle(retail_cloud::Error::Environment(r)) => Some(r.message()),
                _ => None,
            };
            let (status, code) = match error {
                Error::Denied => (StatusCode::FORBIDDEN, "access_denied"),
                Error::Invalid(_) | Error::Json(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
                Error::Conflict(_) => (StatusCode::CONFLICT, "terms_conflict"),
                Error::Lifecycle(retail_cloud::Error::Ledger(
                    pay_ledger::Error::Insufficient { .. },
                ))
                | Error::Ledger(pay_ledger::Error::Insufficient { .. })
                | Error::Insufficient => (StatusCode::PAYMENT_REQUIRED, "insufficient_balance"),
                Error::Lifecycle(retail_cloud::Error::Denied(_)) => {
                    (StatusCode::FORBIDDEN, "access_denied")
                }
                Error::Lifecycle(retail_cloud::Error::Conflict(_)) => {
                    (StatusCode::CONFLICT, "terms_conflict")
                }
                Error::Lifecycle(retail_cloud::Error::Environment(r)) => {
                    use retail_cloud::environment::Refusal as R;
                    match r {
                        R::NotYours | R::NoSpendRight => (StatusCode::FORBIDDEN, "access_denied"),
                        R::Changed | R::Phase | R::MachinesBusy { .. } => {
                            (StatusCode::CONFLICT, "terms_conflict")
                        }
                        R::NoPlan | R::AllowanceUsed { .. } | R::CapReached { .. } => {
                            (StatusCode::PAYMENT_REQUIRED, "allowance_used")
                        }
                        R::StorageFull { .. } | R::TooManyVersions { .. } => {
                            (StatusCode::CONFLICT, "storage_full")
                        }
                        R::Unsupported { .. } | R::Malformed { .. } => {
                            (StatusCode::BAD_REQUEST, "invalid_request")
                        }
                        R::Closed => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
                    }
                }
                _ => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            };
            let mut body = json!({"schema":SCHEMA,"error":code});
            if let Some(message) = message {
                body["message"] = json!(message);
            }
            (status, body)
        }
    };
    let mut response = (status, Json(value)).into_response();
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
