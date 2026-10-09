//! `psionic-serve` as an upstream: local Psionic on the caller's machine
//! (`docs/inference/gateway.md`, section 4), and the model server behind a
//! Pylon provider.
//!
//! `psionic-serve` answers `POST /v1/responses` on loopback, but its types
//! are its own, so this adapter translates both ways:
//!
//! | Open Responses | `psionic-serve` |
//! | --- | --- |
//! | `input` items (`message`) | `input` as chat messages (`role`, `content` text); `developer` becomes `system` |
//! | `instructions`, `max_output_tokens`, `temperature`, `top_p`, `seed`, `stop` | the same names |
//! | `stream: true` | always `stream: false`: its `/v1/responses` does not stream; the answer is replayed as events ([`super::whole`]) |
//! | a `message` with `output_text` | `output[].content[]` of type `output_text` |
//! | a `reasoning` item | `output[].content[]` of type `reasoning_text` inside the message |
//! | `usage` (`input_tokens`, `output_tokens`, details) | `usage` (`input_tokens`, `output_tokens`), no details |
//!
//! Function tools, images, files, and JSON schema are not offered (its
//! tool calls come back outside the item list), so [`super::check`]
//! refuses a request that needs them before anything is sent.
//!
//! Local Psionic is the upstream `local`, its models `local/<id>`, and it
//! takes only a request that asks for it with `openagents.route.only:
//! ["local"]`: a gateway serving others never sends their requests to a
//! model server on its own host by accident. It costs nothing and keeps
//! nothing (the text never leaves the machine), so it is eligible under
//! `strict`.

use serde_json::{Value, json};

use super::whole::{Whole, events};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream, check,
};
use crate::item::{ContentPart, Item, MessageContent, Role};
use crate::request::{CreateResponse, Stop};
use crate::response::{ResponseStatus, Usage};

/// The local upstream's name, the word `route.only` names it by.
pub const LOCAL: &str = "local";

/// How long one answer may take: `psionic-serve` sends nothing until the
/// whole answer is ready.
const ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// The `psionic-serve` request body for `request` to `upstream_model`.
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for an item `psionic-serve` cannot carry.
pub fn body(request: &CreateResponse, upstream_model: &str) -> Result<Value, AttemptError> {
    let mut messages = Vec::new();
    for item in request.input_items() {
        match item {
            Item::Message(message) => {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System | Role::Developer => "system",
                };
                let content = match &message.content {
                    MessageContent::Text(text) => text.clone(),
                    MessageContent::Parts(parts) => {
                        let mut text = Vec::new();
                        for part in parts {
                            match part {
                                ContentPart::InputText(_)
                                | ContentPart::OutputText(_)
                                | ContentPart::Text(_) => {
                                    text.push(part.text().unwrap_or_default().to_owned());
                                }
                                ContentPart::Refusal(_) => {}
                                _ => {
                                    return Err(AttemptError::new(
                                        ErrorClass::Unsupported,
                                        "psionic-serve takes text only",
                                    ));
                                }
                            }
                        }
                        text.join("\n")
                    }
                };
                messages.push(json!({"role": role, "content": content}));
            }
            // Earlier reasoning stays with the model that wrote it.
            Item::Reasoning(_) => {}
            other => {
                return Err(AttemptError::new(
                    ErrorClass::Unsupported,
                    format!("psionic-serve takes no `{}` items", other.type_name()),
                ));
            }
        }
    }
    let mut body = json!({
        "model": upstream_model,
        "input": messages,
        "stream": false,
    });
    let fields = body.as_object_mut().ok_or_else(|| {
        AttemptError::new(ErrorClass::Unsupported, "the request could not be encoded")
    })?;
    if let Some(instructions) = &request.instructions {
        fields.insert("instructions".into(), json!(instructions));
    }
    if let Some(max) = request.max_output_tokens {
        fields.insert("max_output_tokens".into(), json!(max));
    }
    if let Some(temperature) = request.temperature {
        fields.insert("temperature".into(), json!(temperature));
    }
    if let Some(top_p) = request.top_p {
        fields.insert("top_p".into(), json!(top_p));
    }
    if let Some(seed) = request.seed {
        fields.insert("seed".into(), json!(seed));
    }
    match &request.stop {
        Some(Stop::One(stop)) => {
            fields.insert("stop".into(), json!([stop]));
        }
        Some(Stop::Many(stops)) => {
            fields.insert("stop".into(), json!(stops));
        }
        None => {}
    }
    Ok(body)
}

/// A `psionic-serve` answer as a [`Whole`] for the public model `model`.
///
/// # Errors
///
/// [`ErrorClass::Decode`] when the answer is not the shape it serves.
pub fn answer(value: &Value, model: &str) -> Result<Whole, AttemptError> {
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AttemptError::new(ErrorClass::Decode, "psionic-serve sent no output"))?;
    let mut text = String::new();
    let mut reasoning = String::new();
    for item in output {
        for part in item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let piece = part.get("text").and_then(Value::as_str).unwrap_or_default();
            match part.get("type").and_then(Value::as_str) {
                Some("output_text") => text.push_str(piece),
                Some("reasoning_text") => reasoning.push_str(piece),
                _ => {}
            }
        }
    }
    if text.is_empty() {
        // An empty `output` still carries the answer in `output_text`.
        if let Some(whole) = value.get("output_text").and_then(Value::as_str) {
            text = whole.to_owned();
        }
    }
    let count = |field: &str| {
        value
            .get("usage")
            .and_then(|usage| usage.get(field))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let status = match value.get("status").and_then(Value::as_str) {
        Some("incomplete") => ResponseStatus::Incomplete,
        _ => ResponseStatus::Completed,
    };
    Ok(Whole {
        model: model.to_owned(),
        text,
        reasoning: (!reasoning.is_empty()).then_some(reasoning),
        calls: Vec::new(),
        usage: Usage::new(count("input_tokens"), 0, count("output_tokens"), 0),
        status,
    })
}

/// Whether the request asked for local models only.
#[must_use]
pub fn asks_for_local(request: &CreateResponse) -> bool {
    request
        .openagents
        .as_ref()
        .and_then(|options| options.route.as_ref())
        .is_some_and(|route| !route.only.is_empty() && route.only.iter().all(|only| only == LOCAL))
}

/// A model `psionic-serve` serves, as a row: no tools, images, or schema.
#[must_use]
pub fn row(public: &str, upstream_model: &str, context: u64, price: Price) -> ModelRow {
    ModelRow {
        id: public.to_owned(),
        upstream_model: upstream_model.to_owned(),
        capabilities: Capabilities {
            tools: false,
            reasoning: true,
            reasoning_always_on: false,
            json_schema: false,
            images: false,
            context,
            max_output: context,
        },
        price,
        price_source: "psionic-serve on the caller's machine: no charge",
    }
}

/// Sends `body` to a `psionic-serve` at `base` and reads its answer.
///
/// # Errors
///
/// The attempt's error: a transport failure, an HTTP status, or an answer
/// that could not be read.
pub async fn call(
    http: &reqwest::Client,
    base: &str,
    body: &Value,
    model: &str,
) -> Result<Whole, AttemptError> {
    let url = format!("{}/v1/responses", base.trim_end_matches('/'));
    let reply = http
        .post(url)
        .timeout(ANSWER_TIMEOUT)
        .json(body)
        .send()
        .await
        .map_err(|error| super::http::transport(&error))?;
    if !reply.status().is_success() {
        return Err(super::http::status_error(reply, &[]).await);
    }
    let value: Value = reply.json().await.map_err(|_| {
        AttemptError::new(
            ErrorClass::Decode,
            "psionic-serve sent an answer we could not read",
        )
    })?;
    answer(&value, model)
}

/// Local Psionic: `psionic-serve` on this machine.
pub struct LocalPsionic {
    base: Option<String>,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    http: reqwest::Client,
}

impl LocalPsionic {
    /// Local Psionic at `base` (`http://127.0.0.1:8080`) serving `models`
    /// (`psionic-serve`'s ids), each as `local/<id>`. `None` leaves it
    /// unconfigured.
    #[must_use]
    pub fn new(base: Option<String>, models: &[(String, u64)]) -> Self {
        Self {
            base,
            account: Account {
                id: LOCAL.to_owned(),
                basis: CostBasis::FreeCapacity,
            },
            privacy: PrivacyTerms::zero_retention(
                "the model runs on the caller's own machine; the text never leaves it",
            ),
            models: models
                .iter()
                .map(|(id, context)| {
                    row(
                        &format!("{LOCAL}/{id}"),
                        id,
                        *context,
                        Price::micro(0, 0, 0),
                    )
                })
                .collect(),
            http: super::http::client(super::http::CONNECT_TIMEOUT),
        }
    }

    /// Local Psionic from `PSIONIC_BASE_URL` and `PSIONIC_MODELS` (comma-
    /// separated `psionic-serve` ids, each optionally `id=context`).
    #[must_use]
    pub fn from_env() -> Self {
        let base = std::env::var("PSIONIC_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let models: Vec<(String, u64)> = std::env::var("PSIONIC_MODELS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| match id.split_once('=') {
                Some((id, context)) => (id.to_owned(), context.parse().unwrap_or(32_768)),
                None => (id.to_owned(), 32_768),
            })
            .collect();
        Self::new(base, &models)
    }
}

impl Upstream for LocalPsionic {
    fn name(&self) -> &str {
        LOCAL
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }

    fn models(&self) -> &[ModelRow] {
        &self.models
    }

    fn configured(&self) -> bool {
        self.base.is_some() && !self.models.is_empty()
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            if !asks_for_local(request) {
                return Err(AttemptError::new(
                    ErrorClass::Unsupported,
                    "local models answer only requests with `openagents.route.only: [\"local\"]`",
                ));
            }
            let Some(base) = &self.base else {
                return Err(super::unconfigured(LOCAL, "psionic-serve address"));
            };
            let body = body(request, &row.upstream_model)?;
            let meter = AttemptMeter::start(self, row);
            let whole = match call(&self.http, base, &body, &row.id).await {
                Ok(whole) => whole,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events: EventStream = Box::pin(futures_util::stream::iter(
                events(request, &whole).into_iter().map(Ok),
            ));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_become_psionic_messages() {
        let request: CreateResponse = serde_json::from_value(json!({
            "model": "local/qwen", "instructions": "Be brief.", "max_output_tokens": 64,
            "stop": "END", "input": [
                {"type": "message", "role": "developer", "content": "rules"},
                {"type": "message", "role": "user",
                 "content": [{"type": "input_text", "text": "hi"}]},
                {"type": "message", "role": "assistant", "content": "hello"},
                {"type": "reasoning", "summary": []}
            ]
        }))
        .unwrap();
        let body = body(&request, "qwen").unwrap();
        assert_eq!(body["model"], "qwen");
        assert_eq!(body["stream"], false);
        assert_eq!(body["instructions"], "Be brief.");
        assert_eq!(body["stop"], json!(["END"]));
        assert_eq!(
            body["input"],
            json!([{"role": "system", "content": "rules"},
                   {"role": "user", "content": "hi"},
                   {"role": "assistant", "content": "hello"}])
        );
        let tools: CreateResponse = serde_json::from_value(json!({"input": [
            {"type": "function_call_output", "call_id": "c", "output": "x"}]}))
        .unwrap();
        assert!(super::body(&tools, "qwen").is_err());
    }

    #[test]
    fn psionic_answers_become_items() {
        let value = json!({"id": "r-1", "object": "response", "status": "completed",
            "model": "qwen", "output_text": "hi there",
            "output": [{"id": "r-1-msg-0", "type": "message", "status": "completed",
                "role": "assistant", "content": [
                    {"type": "reasoning_text", "text": "think"},
                    {"type": "output_text", "text": "hi there"}]}],
            "usage": {"input_tokens": 4, "output_tokens": 3, "total_tokens": 7},
            "psionic_metrics": {}});
        let whole = answer(&value, "local/qwen").unwrap();
        assert_eq!(whole.text, "hi there");
        assert_eq!(whole.reasoning.as_deref(), Some("think"));
        assert_eq!(whole.usage.total_tokens, 7);
        let bare = json!({"output": [], "output_text": "only here", "usage": {}});
        assert_eq!(answer(&bare, "m").unwrap().text, "only here");
        assert!(answer(&json!({"error": "x"}), "m").is_err());
    }

    #[test]
    fn local_answers_only_requests_that_ask_for_local() {
        let asks: CreateResponse =
            serde_json::from_value(json!({"openagents": {"route": {"only": ["local"]}}})).unwrap();
        assert!(asks_for_local(&asks));
        let mixed: CreateResponse =
            serde_json::from_value(json!({"openagents": {"route": {"only": ["local", "vertex"]}}}))
                .unwrap();
        assert!(!asks_for_local(&mixed));
        assert!(!asks_for_local(&CreateResponse::default()));
        let unset = LocalPsionic::new(None, &[("qwen".into(), 8_192)]);
        assert!(!unset.configured());
        assert_eq!(unset.models()[0].id, "local/qwen");
        assert!(unset.privacy().allows(&crate::openagents::Privacy::Strict));
    }
}
