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

/// One loaded checkpoint: its weights plus the card fields `/v1/models`
/// reports.
pub struct Variant {
    /// The loaded decision model.
    pub model: DecisionModel,
    /// The id requests name, such as `kev-0.5b`.
    pub model_id: String,
    /// The artifact location the listing reports as `run`.
    pub run: String,
    /// The base model id the listing reports.
    pub base: String,
    /// The adapter rank `/api/info` reports.
    pub lora: usize,
}

/// Everything one running server knows beyond the weights.
pub struct ServeState {
    /// Every loaded variant; requests pick one by `model` id.
    pub variants: Vec<Variant>,
    /// The variant `kev-latest` and an absent `model` field resolve to.
    pub default: usize,
    /// Wire-compatible aliases the listing advertises on the default.
    pub aliases: Vec<String>,
    /// The device name `/api/info` reports.
    pub device: String,
}

impl ServeState {
    /// Resolve a request's `model` field to a loaded variant: an exact id,
    /// `kev-latest` for the default, or an absent field.
    fn select(&self, model: &str) -> Result<&Variant, Error> {
        if model == "kev-latest" || model.is_empty() {
            return Ok(&self.variants[self.default]);
        }
        self.variants
            .iter()
            .find(|v| v.model_id == model)
            .ok_or_else(|| Error::UnknownModel {
                model: model.to_string(),
                known: self
                    .variants
                    .iter()
                    .map(|v| v.model_id.clone())
                    .collect(),
            })
    }

    /// The variant a request resolves to.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownModel`] when `model` names no loaded variant.
    pub fn pick<'a>(&'a self, request: &crate::api::SystemOneRequest) -> Result<&'a Variant, Error> {
        self.select(&request.model)
    }
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

/// `GET /v1/models`: every loaded variant, with kev's own artifact fields
/// beside the card fields the TypeSafe client reads.
async fn models(State(state): State<Arc<ServeState>>) -> Json<Value> {
    let cards: Vec<Value> = state
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            json!({
                "id": v.model_id,
                "name": v.model_id,
                "description": format!("kev decision model on {}, served by the openagents Rust port", v.base),
                "release_date": "2026-09-19",
                "aliases": if i == state.default { state.aliases.clone() } else { Vec::<String>::new() },
                "run": v.run,
                "base": v.base,
            })
        })
        .collect();
    Json(json!({ "models": cards }))
}

/// `GET /api/info`: what is loaded, for operators.
async fn info(State(state): State<Arc<ServeState>>) -> Json<Value> {
    Json(json!({
        "device": state.device,
        "default": state.variants[state.default].model_id,
        "variants": state
            .variants
            .iter()
            .map(|v| json!({
                "model_id": v.model_id,
                "run": v.run,
                "base": v.base,
                "lora": v.lora,
                "option_isolation": v.model.option_isolation,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// `POST /api/predict`: a rendered record in, raw option distributions out.
/// The optional `model` field selects a variant the same way
/// `/v1/systemone` does.
async fn predict(
    State(state): State<Arc<ServeState>>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body: Value = serde_json::from_slice(&body)
        .map_err(|e| unprocessable(format!("request body: {e}")))?;
    let model_field = body["model"].as_str().unwrap_or("kev-latest").to_string();
    let record: Record = serde_json::from_value(body.clone())
        .map_err(|e| unprocessable(format!("request body: {e}")))?;
    tokio::task::spawn_blocking(move || {
        let variant = state.select(&model_field)?;
        let enc = variant
            .model
            .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        let tokens = enc.ids.len();
        let state_tokens = enc.seg.iter().filter(|s| **s == 0).count();
        let start = Instant::now();
        let probs = variant.model.probs(&enc)?;
        Ok::<_, Error>(json!({
            "model": variant.model_id,
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
    let variant = state.pick(request)?;
    let (record, meta) = to_record(request)?;
    let enc = variant
        .model
        .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
    let tokens = enc.ids.len();
    let start = Instant::now();
    let probs = variant.model.probs(&enc)?;
    let latency = start.elapsed().as_secs_f64() * 1000.0;
    let answers = to_answers(&probs, &meta);
    Ok(response_body(
        &variant.model_id,
        &answers,
        tokens,
        output_tokens(&variant.model.tokenizer, &answers),
        latency,
    ))
}

/// One forward per question, merged back in request order.
fn evaluate_separate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let variant = state.pick(request)?;
    let mut answers = IndexMap::new();
    let mut tokens = 0usize;
    let mut latency = 0.0;
    for qid in request.questions.keys() {
        let mut one = request.clone();
        one.questions.retain(|id, _| id == qid);
        let (record, meta) = to_record(&one)?;
        let enc = variant
            .model
            .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        tokens += enc.ids.len();
        let start = Instant::now();
        let probs = variant.model.probs(&enc)?;
        latency += start.elapsed().as_secs_f64() * 1000.0;
        answers.extend(to_answers(&probs, &meta));
    }
    Ok(response_body(
        &variant.model_id,
        &answers,
        tokens,
        output_tokens(&variant.model.tokenizer, &answers),
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
