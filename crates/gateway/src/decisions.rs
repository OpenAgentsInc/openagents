//! `POST /v1/decisions`: OpenAI's Decisions API shape over the NIP-DEC
//! decision `POST /v1/systemone` answers (docs/api/decisions.md).
//!
//! A request is parsed and checked here, translated into one NIP-DEC body
//! (`state`, `questions` keyed by name), answered by
//! [`crate::decision_dispatch::Dispatch::decide`], and the answers are
//! translated back: ordered `answers` with `probability`, `choice`, or
//! `score`, `usage`, and our own `openagents` object. The admission is
//! `/v1/systemone`'s: keyless and free, no key needed.
//!
//! The order: `openagents/decide` asks our hosted Clef (the H100) first,
//! then Jev, then Vertex; `clef-flash` or `clef` asks only our hosted Clef;
//! a Jev name asks as `/v1/systemone` does. `X-Decision-Order: pylon` or
//! `hosted` overrides it ([`crate::decision_dispatch::Order`]).
//!
//! Images are read, counted, and bounded, but no door reads them yet (our
//! CUDA Clef refuses images), so a request with an image answers each
//! question with a `refusal` (`unsupported_input`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::decision_dispatch::{Decided, Order};
use crate::serve::ServeState;

/// The route.
pub const PATH: &str = "/v1/decisions";
/// The largest body, images included.
pub const MAX_BODY: usize = 8 << 20;
/// The most images one request carries.
pub const MAX_IMAGES: usize = 4;
/// The most questions one request asks (NIP-DEC).
pub const MAX_QUESTIONS: usize = nostr::decision::MAX_QUESTIONS;
/// The text input bound (NIP-DEC's state bound).
pub const MAX_TEXT: usize = nostr::decision::MAX_STATE_BYTES;
/// The model names this route admits besides Jev's.
pub const MODELS: &[&str] = &["openagents/decide", "clef-flash", "clef"];

/// How long an `Idempotency-Key` replays its first answer.
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 3600);
/// The most answers kept for replay.
const IDEMPOTENCY_KEEP: usize = 4096;

/// One question, as asked.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub name: String,
    pub instructions: String,
    pub kind: Kind,
}

/// A question's type and options.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Predicate {
        yes: Option<String>,
        no: Option<String>,
    },
    /// `(value, description)`, in the caller's order.
    Choice(Vec<(String, Option<String>)>),
    /// `(label, description)`, lowest first.
    Score(Vec<(String, Option<String>)>),
}

/// A parsed request.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub model: String,
    /// The text parts, joined in order.
    pub text: String,
    /// How many images the input carried.
    pub images: usize,
    pub questions: Vec<Question>,
    pub metadata: Option<Map<String, Value>>,
}

/// Why a request is refused before any model runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    pub param: Option<String>,
}

fn invalid(param: &str, message: impl Into<String>) -> Invalid {
    Invalid {
        status: 400,
        code: "invalid_request",
        message: message.into(),
        param: Some(param.to_owned()),
    }
}

fn limit(param: &str, message: impl Into<String>) -> Invalid {
    Invalid {
        status: 413,
        code: "limit_exceeded",
        message: message.into(),
        param: Some(param.to_owned()),
    }
}

fn text_of(value: &Value, param: &str) -> Result<String, Invalid> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid(param, format!("`{param}` is a string.")))
}

fn optional_text(value: Option<&Value>, param: &str) -> Result<Option<String>, Invalid> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(v) => text_of(v, param).map(Some),
    }
}

/// Whether `model` is one this route answers.
#[must_use]
pub fn admits(model: &str) -> bool {
    MODELS.contains(&model) || model.starts_with("jev-") || model.starts_with("typesafe/")
}

/// Parse and check a request body.
///
/// # Errors
///
/// What is wrong, as the error object's fields.
pub fn parse(body: &Value) -> Result<Request, Invalid> {
    let object = body
        .as_object()
        .ok_or_else(|| invalid("body", "The body is a JSON object."))?;
    let model = text_of(
        object
            .get("model")
            .ok_or_else(|| invalid("model", "`model` is required."))?,
        "model",
    )?;
    if !admits(&model) {
        return Err(Invalid {
            status: 404,
            code: "model_not_found",
            message: format!(
                "No decision model `{model}`. Use `openagents/decide`, `clef-flash`, `clef`, or `jev-latest`."
            ),
            param: Some("model".into()),
        });
    }
    let (text, images) = input(
        object
            .get("input")
            .ok_or_else(|| invalid("input", "`input` is required."))?,
    )?;
    let asked = object
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("questions", "`questions` is an array of 1 to 64 questions."))?;
    if asked.is_empty() {
        return Err(invalid("questions", "Ask at least one question."));
    }
    if asked.len() > MAX_QUESTIONS {
        return Err(Invalid {
            status: 400,
            code: "too_many_questions",
            message: format!(
                "{} questions; one request asks at most {MAX_QUESTIONS}.",
                asked.len()
            ),
            param: Some("questions".into()),
        });
    }
    let mut questions = Vec::with_capacity(asked.len());
    let mut names = HashSet::new();
    for (i, q) in asked.iter().enumerate() {
        let question = self::question(q, i)?;
        if !names.insert(question.name.clone()) {
            return Err(invalid(
                &format!("questions[{i}].name"),
                format!("The name `{}` is used twice.", question.name),
            ));
        }
        questions.push(question);
    }
    let metadata = match object.get("metadata") {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) => {
            if map.len() > 16 {
                return Err(invalid("metadata", "`metadata` holds at most 16 pairs."));
            }
            for (key, value) in map {
                if key.len() > 64 || !value.as_str().is_some_and(|v| v.len() <= 512) {
                    return Err(invalid(
                        &format!("metadata.{key}"),
                        "Metadata keys are at most 64 bytes and values strings of at most 512.",
                    ));
                }
            }
            Some(map.clone())
        }
        Some(_) => return Err(invalid("metadata", "`metadata` is an object of strings.")),
    };
    Ok(Request {
        model,
        text,
        images,
        questions,
        metadata,
    })
}

/// The text (parts joined in order) and image count of `input`.
fn input(value: &Value) -> Result<(String, usize), Invalid> {
    let (text, images) = match value {
        Value::String(text) => (text.clone(), 0),
        Value::Array(messages) => {
            let mut parts = Vec::new();
            let mut images = 0;
            for (m, message) in messages.iter().enumerate() {
                let param = format!("input[{m}]");
                let role = message
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("user");
                if role != "user" {
                    return Err(invalid(
                        &format!("{param}.role"),
                        "Input messages have `role: \"user\"`.",
                    ));
                }
                match message.get("content") {
                    Some(Value::String(text)) => parts.push(text.clone()),
                    Some(Value::Array(content)) => {
                        for (c, part) in content.iter().enumerate() {
                            let param = format!("{param}.content[{c}]");
                            match part.get("type").and_then(Value::as_str) {
                                Some("input_text") => parts.push(text_of(
                                    part.get("text").unwrap_or(&Value::Null),
                                    &format!("{param}.text"),
                                )?),
                                Some("input_image") => {
                                    let url = part
                                        .get("image_url")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default();
                                    let kind = url
                                        .strip_prefix("data:image/")
                                        .and_then(|rest| rest.split(';').next())
                                        .unwrap_or_default();
                                    if !matches!(kind, "png" | "jpeg" | "jpg" | "webp")
                                        || !url.contains(";base64,")
                                    {
                                        return Err(invalid(
                                            &format!("{param}.image_url"),
                                            "Images are base64 data URLs (PNG, JPEG, or WebP); hosted URLs and file ids are not fetched.",
                                        ));
                                    }
                                    images += 1;
                                }
                                _ => {
                                    return Err(invalid(
                                        &format!("{param}.type"),
                                        "A content part is `input_text` or `input_image`.",
                                    ));
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(invalid(
                            &format!("{param}.content"),
                            "A message's `content` is a string or an array of parts.",
                        ));
                    }
                }
            }
            (parts.join("\n\n"), images)
        }
        _ => {
            return Err(invalid(
                "input",
                "`input` is a string or an array of user messages.",
            ));
        }
    };
    if images > MAX_IMAGES {
        return Err(limit(
            "input",
            format!("{images} images; one request carries at most {MAX_IMAGES}."),
        ));
    }
    if text.len() > MAX_TEXT {
        return Err(limit(
            "input",
            format!(
                "The text input is {} bytes; the bound is {MAX_TEXT}.",
                text.len()
            ),
        ));
    }
    Ok((text, images))
}

fn question(q: &Value, i: usize) -> Result<Question, Invalid> {
    let at = |field: &str| format!("questions[{i}].{field}");
    if !q.is_object() {
        return Err(invalid(
            &format!("questions[{i}]"),
            "A question is an object.",
        ));
    }
    let name = match q.get("name") {
        None | Some(Value::Null) => format!("q{i}"),
        Some(v) => text_of(v, &at("name"))?,
    };
    if name.is_empty() || name.len() > nostr::decision::MAX_QUESTION_ID_BYTES {
        return Err(invalid(&at("name"), "A name is 1 to 64 bytes."));
    }
    let instructions = text_of(
        q.get("instructions")
            .ok_or_else(|| invalid(&at("instructions"), "`instructions` is required."))?,
        &at("instructions"),
    )?;
    if instructions.trim().is_empty() {
        return Err(invalid(&at("instructions"), "`instructions` is empty."));
    }
    let too_many = |count: usize, most: usize, what: &str| Invalid {
        status: 400,
        code: "too_many_options",
        message: format!("{count} {what}; a question has at most {most}."),
        param: Some(at(what)),
    };
    let kind = match q.get("type").and_then(Value::as_str) {
        Some("predicate") => Kind::Predicate {
            yes: optional_text(q.get("true"), &at("true"))?,
            no: optional_text(q.get("false"), &at("false"))?,
        },
        Some("choice") => {
            let choices = q
                .get("choices")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(&at("choices"), "A choice question has `choices`."))?;
            if choices.len() > nostr::decision::MAX_CHOICE_OPTIONS {
                return Err(too_many(
                    choices.len(),
                    nostr::decision::MAX_CHOICE_OPTIONS,
                    "choices",
                ));
            }
            if choices.len() < 2 {
                return Err(invalid(
                    &at("choices"),
                    "A choice question has 2 to 255 choices.",
                ));
            }
            let mut seen = HashSet::new();
            let mut out = Vec::with_capacity(choices.len());
            for (c, choice) in choices.iter().enumerate() {
                let param = at(&format!("choices[{c}].value"));
                let value = text_of(choice.get("value").unwrap_or(&Value::Null), &param)?;
                if value.is_empty() || value.len() > nostr::decision::MAX_OPTION_ID_BYTES {
                    return Err(invalid(&param, "A choice's value is 1 to 128 bytes."));
                }
                if !seen.insert(value.clone()) {
                    return Err(invalid(
                        &param,
                        format!("The value `{value}` is used twice."),
                    ));
                }
                let description = optional_text(
                    choice.get("description"),
                    &at(&format!("choices[{c}].description")),
                )?;
                out.push((value, description));
            }
            Kind::Choice(out)
        }
        Some("score") => {
            let levels = q
                .get("levels")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(&at("levels"), "A score question has `levels`."))?;
            if levels.len() > nostr::decision::MAX_SCORE_LEVELS {
                return Err(too_many(
                    levels.len(),
                    nostr::decision::MAX_SCORE_LEVELS,
                    "levels",
                ));
            }
            if levels.len() < nostr::decision::MIN_SCORE_LEVELS {
                return Err(invalid(
                    &at("levels"),
                    "A score question has 2 to 10 levels.",
                ));
            }
            let mut out = Vec::with_capacity(levels.len());
            for (l, level) in levels.iter().enumerate() {
                let param = at(&format!("levels[{l}].label"));
                let label = text_of(level.get("label").unwrap_or(&Value::Null), &param)?;
                if label.trim().is_empty() {
                    return Err(invalid(&param, "A level's label is empty."));
                }
                let description = optional_text(
                    level.get("description"),
                    &at(&format!("levels[{l}].description")),
                )?;
                out.push((label, description));
            }
            Kind::Score(out)
        }
        _ => {
            return Err(invalid(
                &at("type"),
                "A question's `type` is `predicate`, `choice`, or `score`.",
            ));
        }
    };
    Ok(Question {
        name,
        instructions,
        kind,
    })
}

/// The NIP-DEC `state` and `questions` for a request (docs/api/decisions.md,
/// "The translation").
#[must_use]
pub fn to_nip_dec(request: &Request) -> (Value, Value) {
    let mut questions = Map::new();
    for q in &request.questions {
        let body = match &q.kind {
            Kind::Predicate { yes, no } => {
                let mut body = json!({"type": "noul", "instructions": q.instructions});
                if yes.is_some() || no.is_some() {
                    let mut criteria = Map::new();
                    if let Some(yes) = yes {
                        criteria.insert("true".into(), json!(yes));
                    }
                    if let Some(no) = no {
                        criteria.insert("false".into(), json!(no));
                    }
                    body["criteria"] = Value::Object(criteria);
                }
                body
            }
            Kind::Choice(choices) => {
                let criteria: Map<String, Value> = choices
                    .iter()
                    .map(|(value, description)| (value.clone(), json!(description)))
                    .collect();
                json!({"type": "choice", "instructions": q.instructions, "criteria": criteria})
            }
            Kind::Score(levels) => {
                let criteria: Vec<Value> = levels
                    .iter()
                    .map(|(label, description)| match description {
                        Some(d) if !d.is_empty() => json!(format!("{label}: {d}")),
                        _ => json!(label),
                    })
                    .collect();
                json!({"type": "score", "instructions": q.instructions, "criteria": criteria})
            }
        };
        questions.insert(q.name.clone(), body);
    }
    (
        Value::String(request.text.clone()),
        Value::Object(questions),
    )
}

/// The answer probabilities for `keys` from a NIP-DEC `probabilities`
/// object, in `keys`' order, summing to 1.
fn ordered(keys: &[String], got: &Value) -> Vec<f64> {
    let raw: Vec<f64> = keys
        .iter()
        .map(|k| {
            got.get(k)
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite())
                .unwrap_or(0.0)
                .max(0.0)
        })
        .collect();
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        return vec![1.0 / keys.len().max(1) as f64; keys.len()];
    }
    raw.iter().map(|p| p / sum).collect()
}

/// The first index of the largest value.
fn top(probabilities: &[f64]) -> usize {
    let mut best = 0;
    for (i, p) in probabilities.iter().enumerate() {
        if *p > probabilities[best] {
            best = i;
        }
    }
    best
}

/// The `answers` array for a door's NIP-DEC `answers` object, in request
/// order.
///
/// # Errors
///
/// A question the door did not answer, or answered as another type.
pub fn answers(request: &Request, nip_dec: &Value) -> Result<Vec<Value>, String> {
    request
        .questions
        .iter()
        .map(|q| {
            let got = nip_dec
                .get(&q.name)
                .ok_or_else(|| format!("question `{}` is not answered", q.name))?;
            Ok(match &q.kind {
                Kind::Predicate { .. } => {
                    let p = got
                        .get("noul")
                        .and_then(Value::as_f64)
                        .filter(|p| p.is_finite())
                        .ok_or_else(|| format!("question `{}` has no probability", q.name))?
                        .clamp(0.0, 1.0);
                    json!({"type": "predicate", "name": q.name, "probability": p})
                }
                Kind::Choice(choices) => {
                    let keys: Vec<String> = choices.iter().map(|(v, _)| v.clone()).collect();
                    let ps = ordered(&keys, &got["probabilities"]);
                    let best = top(&ps);
                    json!({
                        "type": "choice",
                        "name": q.name,
                        "choice": keys[best],
                        "probabilities": keys.iter().zip(&ps)
                            .map(|(value, p)| json!({"value": value, "probability": p}))
                            .collect::<Vec<_>>(),
                        "confidence": ps[best],
                    })
                }
                Kind::Score(levels) => {
                    let keys: Vec<String> = (0..levels.len()).map(|i| i.to_string()).collect();
                    let ps = ordered(&keys, &got["probabilities"]);
                    let best = top(&ps);
                    let score: f64 = ps.iter().enumerate().map(|(i, p)| i as f64 * p).sum();
                    json!({
                        "type": "score",
                        "name": q.name,
                        "score": score,
                        "probabilities": levels.iter().zip(&ps).enumerate()
                            .map(|(i, ((label, _), p))| json!({"value": i, "label": label, "probability": p}))
                            .collect::<Vec<_>>(),
                        "confidence": ps[best],
                    })
                }
            })
        })
        .collect()
}

/// One `refusal` answer per question.
#[must_use]
pub fn refusals(request: &Request, code: &str, message: &str) -> Vec<Value> {
    request
        .questions
        .iter()
        .map(|q| json!({"type": "refusal", "name": q.name, "code": code, "message": message}))
        .collect()
}

/// The response body for a door's answer.
#[must_use]
pub fn response(
    request: &Request,
    id: &str,
    door: Option<&str>,
    nip_dec: &Value,
    answers: Vec<Value>,
) -> Value {
    let mut usage = json!({"input_tokens": nip_dec["usage"]["input_tokens"].as_u64().unwrap_or(0)});
    if let Some(cached) = nip_dec["usage"]["cached_input_tokens"]
        .as_u64()
        .filter(|c| *c > 0)
    {
        usage["cached_input_tokens"] = json!(cached);
    }
    let mut openagents = json!({
        "door": door,
        "latency_ms": nip_dec["latency_ms"].as_u64().unwrap_or(0),
        "cost": {"price_sats": 0, "price_usd": "0.000000"},
    });
    if let Some(backend) = nip_dec["service"]["backend"].as_str() {
        openagents["backend"] = json!(backend);
    }
    if let Some(receipt) = nip_dec["service"]["receipt"].as_str() {
        openagents["receipt"] = json!(receipt);
    }
    let mut body = json!({
        "id": id,
        "object": "decision",
        "model": nip_dec["model"].as_str().unwrap_or(&request.model),
        "answers": answers,
        "usage": usage,
        "openagents": openagents,
    });
    if let Some(metadata) = &request.metadata {
        body["metadata"] = Value::Object(metadata.clone());
    }
    body
}

/// The order a request asks its doors in.
#[must_use]
pub fn order(model: &str, headers: &HeaderMap) -> Order {
    Order::from_header(headers).unwrap_or(match model {
        "openagents/decide" => Order::ClefFirst,
        "clef-flash" | "clef" => Order::HostedOnly,
        _ => Order::JevFirst,
    })
}

/// A new decision id: `dec_` and 26 Crockford base32 digits.
fn new_id() -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let bytes: [u8; 26] = secp256k1::rand::random();
    let tail: String = bytes
        .iter()
        .map(|b| DIGITS[usize::from(*b) % 32] as char)
        .collect();
    format!("dec_{tail}")
}

fn error(status: u16, code: &str, message: &str, param: Option<&str>) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST),
        [("content-type", "application/json")],
        serde_json::to_vec(&json!({"error": {"code": code, "message": message, "param": param}}))
            .unwrap_or_default(),
    )
        .into_response()
}

/// The answer, with `X-Decision-Door` and, for our hosted Clef,
/// `X-Decision-Backend` (the machine that answered).
fn ok(body: &Value) -> Response {
    let mut out = (
        StatusCode::OK,
        [("content-type", "application/json")],
        serde_json::to_vec(body).unwrap_or_default(),
    )
        .into_response();
    for (header, field) in [
        ("x-decision-door", "door"),
        ("x-decision-backend", "backend"),
    ] {
        if let Some(value) = body["openagents"][field]
            .as_str()
            .and_then(|v| v.parse().ok())
        {
            out.headers_mut().insert(header, value);
        }
    }
    out
}

/// Answers kept for `Idempotency-Key` replay: key → (body digest, when, body).
type Replays = Mutex<HashMap<String, (String, Instant, Value)>>;

fn replays() -> &'static Replays {
    static REPLAYS: OnceLock<Replays> = OnceLock::new();
    REPLAYS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn digest(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}

/// `POST /v1/decisions`.
pub(crate) async fn create(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(dispatch) = state.decisions.clone() else {
        return error(
            503,
            "unavailable",
            "This gateway answers no decisions.",
            None,
        );
    };
    // The replay key is scoped to the caller's credential, so one caller's
    // key never replays another's answer.
    let replay = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .map(|key| {
            let credential = headers
                .get("authorization")
                .map(|v| v.as_bytes())
                .unwrap_or_default();
            (digest(&[credential, key.as_bytes()]), digest(&[&body]))
        });
    if let Some((key, body_digest)) = &replay {
        let kept = replays().lock().ok().and_then(|map| map.get(key).cloned());
        if let Some((first, at, answer)) = kept
            && at.elapsed() < IDEMPOTENCY_TTL
        {
            if &first != body_digest {
                return error(
                    409,
                    "idempotency_conflict",
                    "This Idempotency-Key was used with a different body.",
                    None,
                );
            }
            return ok(&answer);
        }
    }
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return error(400, "invalid_request", "The body isn't valid JSON.", None);
    };
    let request = match parse(&value) {
        Ok(request) => request,
        Err(why) => return error(why.status, why.code, &why.message, why.param.as_deref()),
    };
    let id = new_id();
    let answer = if request.images > 0 {
        // No door reads images yet: our CUDA Clef refuses them.
        let answers = refusals(
            &request,
            "unsupported_input",
            "This model does not read images yet.",
        );
        response(
            &request,
            &id,
            None,
            &json!({"model": request.model}),
            answers,
        )
    } else {
        let (nip_state, nip_questions) = to_nip_dec(&request);
        let order = order(&request.model, &headers);
        match dispatch
            .decide(&headers, &request.model, nip_state, nip_questions, order)
            .await
        {
            Decided::Answered {
                door,
                response: nip,
            } => match answers(&request, &nip["answers"]) {
                Ok(answers) => response(&request, &id, Some(&door), &nip, answers),
                Err(why) => {
                    eprintln!("decisions: {door} answered in a shape we can't read: {why}");
                    return error(
                        502,
                        "door_unavailable",
                        "The answering door sent an unreadable answer. Try again.",
                        None,
                    );
                }
            },
            Decided::Refused { status, body } => {
                return (
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                    [("content-type", "application/json")],
                    serde_json::to_vec(&body).unwrap_or_default(),
                )
                    .into_response();
            }
        }
    };
    if let Some((key, body_digest)) = replay
        && let Ok(mut map) = replays().lock()
    {
        if map.len() >= IDEMPOTENCY_KEEP {
            map.retain(|_, (_, at, _)| at.elapsed() < IDEMPOTENCY_TTL);
            if map.len() >= IDEMPOTENCY_KEEP
                && let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, (_, at, _))| *at)
                    .map(|(k, _)| k.clone())
            {
                map.remove(&oldest);
            }
        }
        map.insert(key, (body_digest, Instant::now(), answer.clone()));
    }
    ok(&answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Value {
        json!({
            "model": "openagents/decide",
            "input": "I was charged twice for my order.",
            "questions": [
                {"type": "choice", "name": "department", "instructions": "Which department?",
                 "choices": [{"value": "billing", "description": "Payments."},
                             {"value": "technical", "description": "Product problems."},
                             {"value": "other"}]},
                {"type": "predicate", "instructions": "Is it urgent?", "true": "Money is lost."},
                {"type": "score", "name": "severity", "instructions": "How severe?",
                 "levels": [{"label": "Cosmetic", "description": "Appearance only."},
                            {"label": "Workaround"},
                            {"label": "Blocked", "description": "No way around it."}]}
            ],
            "metadata": {"ticket": "T-1"}
        })
    }

    #[test]
    fn each_question_type_translates_to_nip_dec_in_order() {
        let request = parse(&body()).unwrap();
        assert_eq!(request.questions[1].name, "q1");
        let (state, questions) = to_nip_dec(&request);
        assert_eq!(state, json!("I was charged twice for my order."));
        let names: Vec<&String> = questions.as_object().unwrap().keys().collect();
        assert_eq!(names, ["department", "q1", "severity"]);
        assert_eq!(
            questions["department"],
            json!({"type": "choice", "instructions": "Which department?",
                   "criteria": {"billing": "Payments.", "technical": "Product problems.", "other": null}})
        );
        let keys: Vec<&String> = questions["department"]["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(keys, ["billing", "technical", "other"]);
        assert_eq!(
            questions["q1"],
            json!({"type": "noul", "instructions": "Is it urgent?", "criteria": {"true": "Money is lost."}})
        );
        assert_eq!(
            questions["severity"]["criteria"],
            json!([
                "Cosmetic: Appearance only.",
                "Workaround",
                "Blocked: No way around it."
            ])
        );
        nostr::decision::check_body("openagents/decide", &state, &questions).unwrap();
    }

    #[test]
    fn a_plain_predicate_sends_no_criteria() {
        let request = parse(&json!({"model": "clef", "input": "x",
            "questions": [{"type": "predicate", "instructions": "Yes?"}]}))
        .unwrap();
        let (_, questions) = to_nip_dec(&request);
        assert_eq!(
            questions["q0"],
            json!({"type": "noul", "instructions": "Yes?"})
        );
    }

    #[test]
    fn answers_come_back_in_request_order_and_sum_to_one() {
        let request = parse(&body()).unwrap();
        let nip = json!({
            "severity": {"type": "score", "probabilities": {"2": 0.2, "0": 0.1, "1": 0.7}},
            "q1": {"type": "noul", "noul": 0.92},
            "department": {"type": "choice", "choice": "billing",
                           "probabilities": {"other": 0.02, "technical": 0.03, "billing": 0.9}}
        });
        let out = answers(&request, &nip).unwrap();
        assert_eq!(out[0]["name"], "department");
        assert_eq!(out[0]["choice"], "billing");
        let values: Vec<&str> = out[0]["probabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["value"].as_str().unwrap())
            .collect();
        assert_eq!(values, ["billing", "technical", "other"]);
        let sum: f64 = out[0]["probabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["probability"].as_f64().unwrap())
            .sum();
        assert!((sum - 1.0).abs() < 1e-6);
        assert!((out[0]["confidence"].as_f64().unwrap() - 0.9 / 0.95).abs() < 1e-9);
        assert_eq!(
            out[1],
            json!({"type": "predicate", "name": "q1", "probability": 0.92})
        );
        assert_eq!(out[2]["type"], "score");
        assert!((out[2]["score"].as_f64().unwrap() - 1.1).abs() < 1e-9);
        assert_eq!(out[2]["probabilities"][1]["value"], 1);
        assert_eq!(out[2]["probabilities"][1]["label"], "Workaround");
        assert!((out[2]["probabilities"][1]["probability"].as_f64().unwrap() - 0.7).abs() < 1e-9);
        assert!((out[2]["confidence"].as_f64().unwrap() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn a_missing_answer_is_an_error() {
        let request = parse(&body()).unwrap();
        assert!(answers(&request, &json!({"department": {}})).is_err());
    }

    #[test]
    fn the_response_carries_id_object_usage_and_ours() {
        let request = parse(&body()).unwrap();
        let nip = json!({"model": "clef-flash", "usage": {"input_tokens": 212, "cached_input_tokens": 0},
                         "latency_ms": 184, "service": {"door": "clef"}});
        let out = response(&request, "dec_X", Some("clef"), &nip, vec![]);
        assert_eq!(out["object"], "decision");
        assert_eq!(out["model"], "clef-flash");
        assert_eq!(out["usage"], json!({"input_tokens": 212}));
        assert_eq!(out["openagents"]["door"], "clef");
        assert_eq!(out["openagents"]["latency_ms"], 184);
        assert_eq!(out["openagents"]["cost"]["price_sats"], 0);
        assert_eq!(out["metadata"], json!({"ticket": "T-1"}));
    }

    #[test]
    fn images_refuse_each_question() {
        let request = parse(&json!({"model": "openagents/decide", "input": [{"role": "user", "content": [
            {"type": "input_image", "image_url": "data:image/png;base64,AAAA"},
            {"type": "input_text", "text": "Is the box damaged?"}]}],
            "questions": [{"type": "predicate", "name": "damage", "instructions": "Visible damage?"}]}))
        .unwrap();
        assert_eq!(request.images, 1);
        assert_eq!(request.text, "Is the box damaged?");
        let out = refusals(&request, "unsupported_input", "no");
        assert_eq!(out[0]["type"], "refusal");
        assert_eq!(out[0]["name"], "damage");
    }

    #[test]
    fn bounds_are_refused_with_their_codes() {
        let mut b = body();
        b["questions"] = json!(
            (0..65)
                .map(|i| json!({"type": "predicate", "name": format!("n{i}"), "instructions": "x"}))
                .collect::<Vec<_>>()
        );
        assert_eq!(parse(&b).unwrap_err().code, "too_many_questions");
        let mut b = body();
        b["questions"][0]["choices"] = json!(
            (0..256)
                .map(|i| json!({"value": format!("v{i}")}))
                .collect::<Vec<_>>()
        );
        assert_eq!(parse(&b).unwrap_err().code, "too_many_options");
        let mut b = body();
        b["questions"][2]["levels"] = json!(
            (0..11)
                .map(|i| json!({"label": format!("l{i}")}))
                .collect::<Vec<_>>()
        );
        assert_eq!(parse(&b).unwrap_err().code, "too_many_options");
        let mut b = body();
        b["questions"][0]["choices"] = json!([{"value": "only"}]);
        assert_eq!(parse(&b).unwrap_err().code, "invalid_request");
        let mut b = body();
        b["questions"][1]["name"] = json!("department");
        assert_eq!(
            parse(&b).unwrap_err().param.as_deref(),
            Some("questions[1].name")
        );
        let mut b = body();
        b["input"] = json!("x".repeat(MAX_TEXT + 1));
        let why = parse(&b).unwrap_err();
        assert_eq!((why.status, why.code), (413, "limit_exceeded"));
        let mut b = body();
        b["model"] = json!("gpt-5");
        assert_eq!(parse(&b).unwrap_err().code, "model_not_found");
        let mut b = body();
        b["input"] = json!([{"role": "user", "content": [{"type": "input_image", "image_url": "https://example.com/a.png"}]}]);
        assert_eq!(
            parse(&b).unwrap_err().param.as_deref(),
            Some("input[0].content[0].image_url")
        );
        let mut b = body();
        b["input"] = json!([{"role": "user", "content": (0..5).map(|_| json!({"type": "input_image", "image_url": "data:image/png;base64,AA"})).collect::<Vec<_>>()}]);
        assert_eq!(parse(&b).unwrap_err().code, "limit_exceeded");
    }

    #[test]
    fn the_order_follows_the_model_and_the_header() {
        let none = HeaderMap::new();
        assert_eq!(order("openagents/decide", &none), Order::ClefFirst);
        assert_eq!(order("clef-flash", &none), Order::HostedOnly);
        assert_eq!(order("clef", &none), Order::HostedOnly);
        assert_eq!(order("jev-latest", &none), Order::JevFirst);
        let mut pylon = HeaderMap::new();
        pylon.insert("x-decision-order", "pylon".parse().unwrap());
        assert_eq!(order("openagents/decide", &pylon), Order::PylonFirst);
        let mut hosted = HeaderMap::new();
        hosted.insert("x-decision-order", "hosted".parse().unwrap());
        assert_eq!(order("openagents/decide", &hosted), Order::HostedOnly);
        let mut tpu = HeaderMap::new();
        tpu.insert("x-decision-order", "tpu".parse().unwrap());
        assert_eq!(order("openagents/decide", &tpu), Order::TpuOnly);
        assert_eq!(order("clef-flash", &tpu), Order::TpuOnly);
    }
}
