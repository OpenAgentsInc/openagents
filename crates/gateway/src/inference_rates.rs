//! The public rate card (`docs/inference/gateway.md`, section 8):
//! `GET /v1/rates`, the meter's rate card as [`inference::rates::Card`],
//! open to anyone without a key. `GET /v1/models` carries the same rows
//! for each model (`serve::models`, through [`catalog`]).
//!
//! Sats are figured at `inference.sats_rate` when the config sets one;
//! without it the card has dollars only.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::routing::{MethodRouter, get};

use crate::serve::ServeState;

pub const PATH: &str = "/v1/rates";
pub const TOKENS: &str = "/v1/usage/tokens-served";

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(PATH, get(rates)), (TOKENS, get(tokens))]
}

/// The card this gateway charges by: its meter's rate rows.
#[must_use]
pub fn card(state: &ServeState) -> inference::rates::Card {
    let sats = state
        .config
        .inference
        .as_ref()
        .and_then(|config| config.sats_rate.as_ref());
    let rows = state
        .meter
        .as_ref()
        .map(|meter| meter.snapshot().0)
        .unwrap_or_default();
    inference::rates::Card::from_rates(&rows, sats)
}

/// The models this gateway can route to right now (adapters with their
/// key), in the OpenAI list shape with each provider's price rows and the
/// last hour's live rates. `None` when inference is not set up.
#[must_use]
pub fn catalog(state: &ServeState) -> Option<serde_json::Value> {
    let gateway = state.inference.as_ref()?;
    let sats = state
        .config
        .inference
        .as_ref()
        .and_then(|config| config.sats_rate.as_ref());
    let (rows, _) = gateway.meter().snapshot();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| u64::try_from(span.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default();
    let live = gateway.meter().rates(60 * 60_000, now);
    let summary = gateway.outcome_summary().ok()?;
    let mut catalog = inference::rates::catalog(&gateway.offerings(), &rows, &live, sats);
    for model in catalog["data"].as_array_mut()? {
        let id = model["id"].as_str()?.to_owned();
        model["openagents"]["accepted_outcomes"] = summary.model(&id);
    }
    Some(catalog)
}

async fn rates(State(state): State<Arc<ServeState>>) -> Json<inference::rates::Card> {
    Json(card(&state))
}

/// Anonymous aggregate read; no caller or key identifiers leave the meter.
async fn tokens(State(state): State<Arc<ServeState>>) -> axum::response::Response {
    use axum::response::IntoResponse;
    match state.meter.as_ref().and_then(|meter| meter.tokens_served()) {
        Some(report) => Json(report).into_response(),
        None => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(
                serde_json::json!({"error":{"message":"Token totals are unavailable right now."}}),
            ),
        )
            .into_response(),
    }
}
