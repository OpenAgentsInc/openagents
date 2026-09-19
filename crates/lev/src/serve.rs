//! The door: the System One contract over HTTP.
//!
//! A `crates/jev` client reaches this with a `base_url` change and no other
//! edit, which is the point of building a third implementation of one
//! contract.
//!
//! **What the numbers mean.** The `probabilities` a Lev answer carries are
//! the frequency with which the model selected each option across `N` seeded
//! samples. They measure how consistently it answers, not how often it is
//! right, and the behavior record in `docs/lev/measurements/` shows the model
//! holding a wrong answer at 0.81 as steadily as a right one. Every response
//! says so in `extensions.calibration`, and `GET /v1/models` says so too. A
//! caller that will not accept an uncalibrated number sends
//! `extensions.require_calibration` and gets a typed refusal instead.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::api::{MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, SystemOneRequest, SystemOneResponse, Usage};
use crate::bridge::Pool;
use crate::error::{Refusal, RefusalCode};
use crate::estimator::{Estimator, answer, l2_pool_with};
use crate::schema::compile;

/// How many seeded samples one question draws by default.
pub const DEFAULT_SAMPLES: u64 = 8;

/// What the door serves.
pub struct Door {
    pool: Pool,
    model: String,
    samples: u64,
    adapter: Option<String>,
}

impl Door {
    /// Builds a door over a pool of helper processes.
    #[must_use]
    pub fn new(pool: Pool, model: impl Into<String>, samples: u64) -> Self {
        Self { pool, model: model.into(), samples: samples.max(1), adapter: None }
    }

    /// Serves every call through a `.fmadapter` package.
    ///
    /// The package is checked and pinned before the door starts, not per
    /// request: a signature mismatch is a deployment error, not a caller
    /// error.
    #[must_use]
    pub fn with_adapter(mut self, path: impl Into<String>) -> Self {
        self.adapter = Some(path.into());
        self
    }

    /// How many helpers back this door.
    #[must_use]
    pub fn pool_width(&self) -> usize {
        self.pool.width()
    }

    /// The router, ready to serve.
    #[must_use]
    pub fn router(self: Arc<Self>) -> axum::Router {
        axum::Router::new()
            .route("/v1/systemone", post(system_one))
            .route("/v1/models", get(models))
            .with_state(self)
    }
}

/// A refusal on the wire.
struct Wire(Refusal);

impl IntoResponse for Wire {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.code.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = json!({
            "error": {
                "code": self.0.code.label(),
                "message": self.0.message,
                "question": self.0.question,
            }
        });
        (status, Json(body)).into_response()
    }
}

/// The base signature this door is pinned to, read from its adapter package.
///
/// A door with no adapter reports nothing: the runtime exposes its signature
/// only through the adapter-compatibility call, and inventing one here would
/// let a record claim a match it never checked.
fn base_signature(door: &Door) -> String {
    door.adapter
        .as_deref()
        .and_then(|path| crate::adapter::Package::open(path).ok())
        .map(|package| package.metadata.base_model_signature)
        .unwrap_or_default()
}

async fn models(State(door): State<Arc<Door>>) -> Response {
    let availability = door.pool.availability();
    let (status, reason) = match availability {
        Ok(availability) => (availability.status, availability.reason),
        Err(refusal) => ("unknown".to_string(), Some(refusal.message)),
    };
    Json(json!({
        "models": [{
            "name": door.model,
            // `jev::ModelCard` reads name, description, and release_date, so a
            // client's `models().list()` works against this door too.
            "release_date": "2026-09-19",
            "description": "Apple's on-device foundation model, answering the System One contract. \
                            Its probabilities are seeded-sampling frequencies, not calibrated \
                            predictive probabilities.",
            "availability": status,
            "unavailable_reason": reason,
            "estimator": Estimator::L2.label(),
            "samples": door.samples,
            "pool_width": door.pool.width(),
            "resolution": 1.0 / door.samples as f64,
            "adapter": door.adapter,
            // The signature a calibration record has to match to serve here.
            "base_model_signature": base_signature(&door),
            "calibration": "none",
            "calibrated_families": [],
            "question_types": ["noul", "choice", "score"],
            "max_options": MAX_CHOICE_OPTIONS,
            "max_levels": MAX_SCORE_LEVELS,
        }]
    }))
    .into_response()
}

async fn system_one(State(door): State<Arc<Door>>, body: String) -> Response {
    let request: SystemOneRequest = match serde_json::from_str(&body) {
        Ok(request) => request,
        Err(error) => {
            return Wire(Refusal::new(
                RefusalCode::InvalidRequest,
                format!("the request body did not parse: {error}"),
            ))
            .into_response();
        }
    };

    match answer_request(&door, &request) {
        Ok(response) => Json(response).into_response(),
        Err(refusal) => Wire(refusal).into_response(),
    }
}

fn answer_request(door: &Door, request: &SystemOneRequest) -> crate::error::Result<SystemOneResponse> {
    if request.extensions.require_calibration {
        return Err(Refusal::new(
            RefusalCode::Uncalibrated,
            "this door serves seeded-sampling frequencies and holds no fitted calibration map. \
             See docs/lev/calibration.md.",
        ));
    }

    let compiled = compile(request)?;
    let availability = door.pool.availability()?;
    if !availability.is_available() {
        return Err(Refusal::new(
            RefusalCode::ModelUnavailable,
            format!(
                "the on-device model is {}{}",
                availability.status,
                availability.reason.map(|reason| format!(": {reason}")).unwrap_or_default()
            ),
        ));
    }

    let mut answers = IndexMap::new();
    let mut estimates = IndexMap::new();
    for (id, question) in &compiled {
        let raw = l2_pool_with(&door.pool, question, door.samples, door.adapter.as_deref())
            .map_err(|refusal| with_question(refusal, id))?;
        let typed = answer(question.kind, &raw.frequency, &question.legend)
            .map_err(|refusal| with_question(refusal, id))?;
        answers.insert(id.clone(), typed);
        if request.extensions.estimator {
            estimates.insert(
                id.clone(),
                json!({
                    "estimator": raw.estimator.label(),
                    "samples": door.samples,
            "pool_width": door.pool.width(),
                    "seeds": raw.seeds,
                    "resolution": raw.resolution,
                    "refused_draws": raw.refused,
                    "latency_ms": raw.latency_ms,
                }),
            );
        }
    }

    Ok(SystemOneResponse {
        model: door.model.clone(),
        answers,
        // Apple bills no tokens and the runtime surfaces no counts, so this
        // stays empty rather than carrying a character-count fiction.
        usage: Usage::default(),
        extensions: extensions(request, estimates),
    })
}

fn extensions(request: &SystemOneRequest, estimates: IndexMap<String, Value>) -> Option<Value> {
    let calibration = json!({
        "state": "uncalibrated",
        "meaning": "probabilities are the frequency with which the model selected each option \
                    across seeded samples. They measure decoding consistency, not correctness. \
                    Do not gate an action on them without fitting a map on your own labelled \
                    outcomes.",
    });
    if request.extensions.estimator {
        Some(json!({ "calibration": calibration, "estimator": estimates }))
    } else {
        Some(json!({ "calibration": calibration }))
    }
}

fn with_question(mut refusal: Refusal, id: &str) -> Refusal {
    if refusal.question.is_none() {
        refusal.question = Some(id.to_string());
    }
    refusal
}
