//! The HTTP surface: TypeSafe-compatible `POST /v1/systemone` and
//! `GET /v1/models`, the separate-eval and debug routes the reference
//! carries, and deterministic refusals.
//!
//! The served model identifies itself as kev; the `jev-latest` alias keeps
//! the wire contract reachable for callers that name TypeSafe's model id,
//! matching the reference server's alias list.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokenizers::Tokenizer;

use crate::api::{Answer, Record, SystemOneRequest, to_answers, to_record};
use crate::decision::DecisionModel;
use crate::error::Error;

/// The state budget serving admits; the reference serves with the training
/// bounds relaxed to the branch ceiling.
pub const INFER_MAX_STATE: usize = 8192;
/// The branch budget serving admits.
pub const INFER_MAX_BRANCH: usize = 8192;

/// Everything one running server knows beyond the weights.
pub struct ServeState {
    /// The loaded decision model.
    pub model: DecisionModel,
    /// The id `/v1/systemone` requests name, and `/v1/models` reports.
    pub model_id: String,
    /// Wire-compatible aliases the listing advertises.
    pub aliases: Vec<String>,
    /// The artifact location the listing reports as `run`.
    pub run: String,
    /// The base model id the listing reports.
    pub base: String,
    /// The adapter rank `/api/info` reports.
    pub lora: usize,
    /// The device name `/api/info` reports.
    pub device: String,
}

/// The error body every refusal shares: FastAPI's `{"detail": …}`.
fn refuse(status: StatusCode, detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "detail": detail.into() })))
}

fn unprocessable(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    refuse(StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
}

/// `POST /v1/systemone`: typed questions in, typed answers out, one prefill.
async fn systemone(
    State(state): State<Arc<ServeState>>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request: SystemOneRequest = serde_json::from_slice(&body)
        .map_err(|e| unprocessable(format!("request body: {e}")))?;
    let out = tokio::task::spawn_blocking(move || evaluate(&state, &request))
        .await
        .map_err(|e| refuse(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(unprocessable)?;
    Ok(Json(out))
}

/// `POST /v1/systemone/separate`: each question in its own pass against the
/// same state, for packed-vs-separate comparison.
async fn systemone_separate(
    State(state): State<Arc<ServeState>>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request: SystemOneRequest = serde_json::from_slice(&body)
        .map_err(|e| unprocessable(format!("request body: {e}")))?;
    tokio::task::spawn_blocking(move || evaluate_separate(&state, &request))
        .await
        .map_err(|e| refuse(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(unprocessable)
        .map(Json)
}

/// `GET /v1/models`: the listing the TypeSafe client reads, with kev's own
/// artifact fields beside the card fields.
async fn models(State(state): State<Arc<ServeState>>) -> Json<Value> {
    Json(json!({
        "models": [{
            "id": state.model_id,
            "name": state.model_id,
            "description": "kev decision model, served by the openagents Rust port",
            "release_date": "2026-09-19",
            "aliases": state.aliases,
            "run": state.run,
            "base": state.base,
        }]
    }))
}

/// `GET /api/info`: what is loaded, for operators.
async fn info(State(state): State<Arc<ServeState>>) -> Json<Value> {
    Json(json!({
        "run": state.run,
        "base": state.base,
        "device": state.device,
        "lora": state.lora,
        "model_id": state.model_id,
        "option_isolation": state.model.option_isolation,
    }))
}

/// `POST /api/predict`: a rendered record in, raw option distributions out.
async fn predict(
    State(state): State<Arc<ServeState>>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record: Record = serde_json::from_slice(&body)
        .map_err(|e| unprocessable(format!("request body: {e}")))?;
    tokio::task::spawn_blocking(move || {
        let enc = state
            .model
            .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        let tokens = enc.ids.len();
        let state_tokens = enc.seg.iter().filter(|s| **s == 0).count();
        let start = Instant::now();
        let probs = state.model.probs(&enc)?;
        Ok::<_, Error>(json!({
            "probs": probs,
            "tokens": tokens,
            "state_tokens": state_tokens,
            "latency_ms": start.elapsed().as_secs_f64() * 1000.0,
        }))
    })
    .await
    .map_err(|e| refuse(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(unprocessable)
    .map(Json)
}

/// One packed forward and the shaped response body.
fn evaluate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let (record, meta) = to_record(request)?;
    let enc = state.model.encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
    let tokens = enc.ids.len();
    let start = Instant::now();
    let probs = state.model.probs(&enc)?;
    let latency = start.elapsed().as_secs_f64() * 1000.0;
    let answers = to_answers(&probs, &meta);
    Ok(response_body(
        &request.model,
        &answers,
        tokens,
        output_tokens(&state.model.tokenizer, &answers),
        latency,
    ))
}

/// One forward per question, merged back in request order.
fn evaluate_separate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let mut answers = IndexMap::new();
    let mut tokens = 0usize;
    let mut latency = 0.0;
    for qid in request.questions.keys() {
        let mut one = request.clone();
        one.questions.retain(|id, _| id == qid);
        let (record, meta) = to_record(&one)?;
        let enc = state.model.encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        tokens += enc.ids.len();
        let start = Instant::now();
        let probs = state.model.probs(&enc)?;
        latency += start.elapsed().as_secs_f64() * 1000.0;
        answers.extend(to_answers(&probs, &meta));
    }
    Ok(response_body(
        &request.model,
        &answers,
        tokens,
        output_tokens(&state.model.tokenizer, &answers),
        latency,
    ))
}

fn response_body(
    model: &str,
    answers: &IndexMap<String, Answer>,
    input_tokens: usize,
    output_tokens: usize,
    latency_ms: f64,
) -> Value {
    json!({
        "model": model,
        "answers": answers,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        },
        "latency_ms": (latency_ms * 10.0).round() / 10.0,
    })
}

/// The billing-style output count: tokens of the serialized answers, using
/// the reference's `json.dumps` formatting so the figure is comparable.
fn output_tokens(tokenizer: &Tokenizer, answers: &IndexMap<String, Answer>) -> usize {
    let answers = serde_json::to_value(answers).unwrap_or(Value::Null);
    tokenizer
        .encode(python_json(&answers).as_str(), false)
        .map(|enc| enc.get_ids().len())
        .unwrap_or(0)
}

/// `json.dumps` with the defaults Python applies: `", "` and `": "` separators
/// and `ensure_ascii` escaping.
fn python_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => python_string(s),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(python_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", python_string(k), python_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A JSON string literal the way `json.dumps` writes one: short escapes for
/// the common controls, `\uXXXX` for the rest, surrogate pairs for astral.
fn python_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch if ch.is_ascii() => out.push(ch),
            ch => {
                let mut buf = [0u16; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// The routes the binary mounts.
pub fn router(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/systemone/separate", post(systemone_separate))
        .route("/v1/models", get(models))
        .route("/api/info", get(info))
        .route("/api/predict", post(predict))
        .with_state(state)
}
