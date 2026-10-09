//! The inference meter's admin status (`docs/inference/gateway.md`,
//! sections 5 and 6): per-account credit burn-downs, the alerts that hold
//! now, and live rates per upstream and model.
//!
//! `GET /v1/admin/inference/status` answers only a bearer equal to the
//! environment variable `inference.admin_token_env` names; with that
//! variable unset it refuses everyone. Mounted only when `inference` is
//! configured. The meter itself raises alerts as log lines; this route
//! is where an operator reads the same state.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use serde_json::json;

use crate::serve::ServeState;

/// The status route's path.
pub const PATH: &str = "/v1/admin/inference/status";

/// How often the meter's daily hook runs (it reconciles once a day and
/// checks time-based alerts such as credit nearing expiry).
const TICK: Duration = Duration::from_secs(3_600);

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(PATH, get(status))]
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_millis() as u64)
        .unwrap_or_default()
}

/// Start the hourly tick inside the runtime. No provider billing source
/// is wired yet; the adapters (#11061) bring them.
pub(crate) fn resume(state: &Arc<ServeState>) {
    let Some(meter) = state.meter.clone() else {
        return;
    };
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        let mut every = tokio::time::interval(TICK);
        loop {
            every.tick().await;
            meter.tick(&[], now_ms());
        }
    });
}

fn authorized(state: &ServeState, headers: &HeaderMap) -> bool {
    let Some(config) = &state.config.inference else {
        return false;
    };
    let Some(expected) = std::env::var(&config.admin_token_env)
        .ok()
        .filter(|token| !token.is_empty())
    else {
        return false;
    };
    let Some(given) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    let (a, b) = (given.as_bytes(), expected.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn status(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }
    match state
        .meter
        .as_ref()
        .and_then(|meter| meter.status(now_ms()))
    {
        Some(status) => Json(status).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "meter unavailable"})),
        )
            .into_response(),
    }
}
