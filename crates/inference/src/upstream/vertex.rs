//! Google Vertex AI (Gemini), billed to the prepaid Google credit account.
//!
//! Wire: Vertex's native `streamGenerateContent?alt=sse`, not its
//! OpenAI-compatible endpoint, because the native API returns thought
//! summaries as marked parts, function calls whole with their thought
//! signatures, and cached-token counts — all of which the compatible
//! endpoint flattens. Auth is an OAuth token from a [`TokenSource`]: the
//! metadata server on Cloud Run, a service-account key elsewhere.
//!
//! Mapping, request side: `instructions` and system or developer messages
//! become `systemInstruction`; user and assistant messages become `user`
//! and `model` contents; a function call becomes a `functionCall` part on
//! a `model` turn, carrying the `thoughtSignature` from the reasoning item
//! before it (or Google's documented placeholder when the caller sent
//! none); a function call output becomes a `functionResponse` part. Tools
//! become `functionDeclarations` with `parametersJsonSchema`; `tool_choice`
//! becomes `functionCallingConfig`; a JSON schema becomes
//! `responseJsonSchema`; reasoning effort becomes a thinking level (Gemini
//! 3) or budget (Gemini 2.5), always with thought summaries on.
//!
//! Stream side: thought parts stream as a reasoning item's summary, text
//! parts as the answer, each `functionCall` part as a function call item,
//! and a call's `thoughtSignature` as the reasoning item's
//! `encrypted_content`.
//!
//! Privacy: Google does not train on Vertex requests, and with the
//! project's prompt cache disabled and no abuse-logging exemption Vertex
//! keeps nothing (Vertex AI "Zero data retention" page). Those are project
//! settings, not request fields, so this adapter sends nothing for privacy;
//! the owner step in the spec (section 14) names the project.

use std::collections::HashMap;

use futures_util::StreamExt;
use serde_json::{Map, Value, json};

use super::emit::{Emitter, Finish};
use super::gate::Gate;
use super::google::TokenSource;
use super::http::{self, Frames, clean};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream, check, unconfigured,
};
use crate::error::ResponseError;
use crate::item::{ContentPart, Item, MessageContent, Role, ToolOutput};
use crate::request::{
    CreateResponse, ReasoningEffort, Stop, TextFormat, Tool, ToolChoice, ToolChoiceMode,
};
use crate::response::{IncompleteReason, Usage};
use crate::wire::Extra;

/// The default project: the one the automation service account works in.
/// `VERTEX_PROJECT` names the project that holds the credit.
pub const DEFAULT_PROJECT: &str = "openagentsgemini";

/// `global`, where Gemini 3 models are served.
pub const DEFAULT_LOCATION: &str = "global";

/// Google's documented placeholder for a function call whose thought
/// signature the caller did not keep (a conversation from another model or
/// a stateless client). Gemini 3 refuses a function call turn without one.
pub const SKIP_SIGNATURE: &str = "skip_thought_signature_validator";

/// How a model takes a reasoning setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thinking {
    /// `thinkingLevel` (Gemini 3).
    Level,
    /// `thinkingBudget` in tokens (Gemini 2.5).
    Budget,
}

/// The Vertex adapter's settings.
#[derive(Clone, Debug)]
pub struct Config {
    pub project: String,
    pub location: String,
    pub token: TokenSource,
    /// Replaces the Vertex host, for tests.
    pub base_url: Option<String>,
    pub account: Account,
    pub privacy: PrivacyTerms,
    pub models: Vec<(ModelRow, Thinking)>,
}

/// The models served through Vertex, with their list prices.
#[must_use]
pub fn default_models() -> Vec<(ModelRow, Thinking)> {
    vec![
        (
            ModelRow {
                id: "google/gemini-3.8-flash".into(),
                upstream_model: "gemini-3.8-flash".into(),
                capabilities: Capabilities {
                    tools: true,
                    reasoning: true,
                    reasoning_always_on: false,
                    json_schema: true,
                    images: true,
                    context: 1_048_576,
                    max_output: 65_536,
                },
                price: Price::micro(750_000, 75_000, 3_750_000),
                price_source: "derived from Vercel's marketCost for 25 input and 130 output tokens \
                               in crates/coder/fixtures/gateway/google-gemini-3.8-flash.sse \
                               (2026-09-19); cached input at Google's usual 10%",
            },
            Thinking::Level,
        ),
        (
            ModelRow {
                id: "google/gemini-2.5-flash-lite".into(),
                upstream_model: "gemini-2.5-flash-lite".into(),
                capabilities: Capabilities {
                    tools: true,
                    reasoning: true,
                    reasoning_always_on: false,
                    json_schema: true,
                    images: true,
                    context: 1_048_576,
                    max_output: 65_536,
                },
                price: Price::micro(100_000, 10_000, 400_000),
                price_source: "Vertex AI pricing page, Gemini 2.5 Flash-Lite",
            },
            Thinking::Budget,
        ),
    ]
}

impl Config {
    /// Settings from the environment: `VERTEX_PROJECT`, `VERTEX_LOCATION`,
    /// and the token source [`TokenSource::from_env`] finds.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        Self {
            project: var("VERTEX_PROJECT").unwrap_or_else(|| DEFAULT_PROJECT.to_owned()),
            location: var("VERTEX_LOCATION").unwrap_or_else(|| DEFAULT_LOCATION.to_owned()),
            token: TokenSource::from_env(),
            base_url: var("VERTEX_BASE_URL"),
            account: Account {
                id: "google-credit".into(),
                basis: CostBasis::PrepaidCredit,
            },
            privacy: PrivacyTerms::zero_retention(
                "Vertex AI zero data retention: no training on customer data; nothing kept with \
                 the project's prompt cache off and no abuse-logging exemption",
            ),
            models: default_models(),
        }
    }
}

/// The Vertex adapter.
pub struct Vertex {
    config: Config,
    rows: Vec<ModelRow>,
    thinking: HashMap<String, Thinking>,
    http: reqwest::Client,
}

impl Vertex {
    #[must_use]
    pub fn new(config: Config) -> Self {
        let rows = config.models.iter().map(|(row, _)| row.clone()).collect();
        let thinking = config
            .models
            .iter()
            .map(|(row, thinking)| (row.id.clone(), *thinking))
            .collect();
        Self {
            config,
            rows,
            thinking,
            http: http::client(http::CONNECT_TIMEOUT),
        }
    }

    /// The `streamGenerateContent` URL for `model`.
    #[must_use]
    pub fn url(&self, model: &str) -> String {
        let Config {
            project, location, ..
        } = &self.config;
        let host = match &self.config.base_url {
            Some(base) => base.trim_end_matches('/').to_owned(),
            None if location == "global" => "https://aiplatform.googleapis.com".to_owned(),
            None => format!("https://{location}-aiplatform.googleapis.com"),
        };
        format!(
            "{host}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent?alt=sse"
        )
    }
}

impl Upstream for Vertex {
    fn name(&self) -> &'static str {
        "vertex"
    }

    fn account(&self) -> &Account {
        &self.config.account
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.config.privacy
    }

    fn models(&self) -> &[ModelRow] {
        &self.rows
    }

    fn configured(&self) -> bool {
        self.config.token.present()
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            if !self.configured() {
                return Err(unconfigured("vertex", "Google credential"));
            }
            let thinking = self.thinking.get(model).copied().unwrap_or(Thinking::Level);
            let body = body(request, row, thinking)?;
            let meter = AttemptMeter::start(self, row);
            let token = match self.config.token.token().await {
                Ok(token) => token,
                Err(why) => {
                    let error = AttemptError::new(ErrorClass::Auth, why);
                    meter.fail(&error);
                    return Err(error);
                }
            };
            let call = self
                .http
                .post(self.url(&row.upstream_model))
                .bearer_auth(token.expose())
                .header("accept", "text/event-stream")
                .json(&body);
            let frames = match http::open_stream(call, &[]).await {
                Ok(frames) => frames,
                Err(error) => {
                    if error.class == ErrorClass::Auth {
                        self.config.token.forget().await;
                    }
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events = events(frames, Emitter::new(&row.id, request), &row.id);
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

fn unsupported(message: impl Into<String>) -> AttemptError {
    AttemptError::new(ErrorClass::Unsupported, message)
}

fn image_part(url: &str) -> Value {
    if let Some(rest) = url.strip_prefix("data:")
        && let Some((meta, data)) = rest.split_once(',')
    {
        let mime = meta.trim_end_matches(";base64");
        return json!({"inlineData": {"mimeType": mime, "data": data}});
    }
    let lower = url.to_ascii_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/jpeg"
    };
    json!({"fileData": {"mimeType": mime, "fileUri": url}})
}

fn parts_of(content: &MessageContent) -> Result<Vec<Value>, AttemptError> {
    match content {
        MessageContent::Text(text) => Ok(vec![json!({"text": text})]),
        MessageContent::Parts(parts) => {
            let mut out = Vec::new();
            for part in parts {
                match part {
                    ContentPart::InputImage(image) => {
                        let url = image
                            .image_url
                            .as_deref()
                            .ok_or_else(|| unsupported("an image by file id needs stored files"))?;
                        out.push(image_part(url));
                    }
                    other => match other.text() {
                        Some(text) => out.push(json!({"text": text})),
                        None => {
                            return Err(unsupported(format!(
                                "Vertex takes no `{}` input here",
                                other.type_name()
                            )));
                        }
                    },
                }
            }
            Ok(out)
        }
    }
}

fn text_of(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(ContentPart::text)
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn push(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    if let Some(last) = contents.last_mut()
        && last["role"] == role
        && let Some(existing) = last["parts"].as_array_mut()
    {
        existing.extend(parts);
        return;
    }
    contents.push(json!({"role": role, "parts": parts}));
}

/// The `generateContent` body for `request` to `row`.
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for an input Vertex cannot carry here.
pub fn body(
    request: &CreateResponse,
    row: &ModelRow,
    thinking: Thinking,
) -> Result<Value, AttemptError> {
    let mut system: Vec<String> = request
        .instructions
        .iter()
        .filter(|text| !text.is_empty())
        .cloned()
        .collect();
    let mut contents: Vec<Value> = Vec::new();
    let mut names: HashMap<String, String> = HashMap::new();
    let mut signature: Option<String> = None;
    // Whether the current model turn already has a function call: only the
    // first call of a turn carries a signature.
    let mut turn_has_call = false;
    for item in request.input_items() {
        match item {
            Item::Message(message) => match message.role {
                Role::System | Role::Developer => system.push(text_of(&message.content)),
                Role::User => {
                    push(&mut contents, "user", parts_of(&message.content)?);
                    turn_has_call = false;
                }
                Role::Assistant => {
                    let text = text_of(&message.content);
                    if !text.is_empty() {
                        push(&mut contents, "model", vec![json!({"text": text})]);
                    }
                }
            },
            Item::Reasoning(reasoning) => {
                if let Some(encrypted) = reasoning.encrypted_content {
                    signature = Some(encrypted);
                }
            }
            Item::FunctionCall(call) => {
                names.insert(call.call_id.clone(), call.name.clone());
                let args: Value =
                    serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({}));
                let mut part = json!({"functionCall": {"name": call.name, "args": args}});
                if !turn_has_call {
                    part["thoughtSignature"] = json!(
                        signature
                            .take()
                            .unwrap_or_else(|| SKIP_SIGNATURE.to_owned())
                    );
                }
                turn_has_call = true;
                push(&mut contents, "model", vec![part]);
            }
            Item::FunctionCallOutput(output) => {
                let name = names
                    .get(&output.call_id)
                    .cloned()
                    .ok_or_else(|| unsupported("a function call output with no matching call"))?;
                let text = match &output.output {
                    ToolOutput::Text(text) => text.clone(),
                    ToolOutput::Parts(parts) => parts
                        .iter()
                        .filter_map(ContentPart::text)
                        .collect::<Vec<_>>()
                        .join(""),
                };
                let part =
                    json!({"functionResponse": {"name": name, "response": {"output": text}}});
                push(&mut contents, "user", vec![part]);
                turn_has_call = false;
            }
            Item::ItemReference(_) | Item::Compaction(_) => {
                return Err(unsupported(
                    "item references and compaction need stored responses",
                ));
            }
            Item::Unknown(value) => {
                let kind = value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(unsupported(format!("Vertex takes no `{kind}` item")));
            }
        }
    }
    let mut body = Map::new();
    body.insert("contents".into(), Value::Array(contents));
    let system: Vec<String> = system.into_iter().filter(|text| !text.is_empty()).collect();
    if !system.is_empty() {
        body.insert(
            "systemInstruction".into(),
            json!({"parts": [{"text": system.join("\n\n")}]}),
        );
    }
    let (declarations, config) = tools(request);
    if !declarations.is_empty() {
        body.insert(
            "tools".into(),
            json!([{"functionDeclarations": declarations}]),
        );
        if let Some(config) = config {
            body.insert(
                "toolConfig".into(),
                json!({"functionCallingConfig": config}),
            );
        }
    }
    let mut generation = Map::new();
    if let Some(value) = request.temperature {
        generation.insert("temperature".into(), json!(value));
    }
    if let Some(value) = request.top_p {
        generation.insert("topP".into(), json!(value));
    }
    if let Some(value) = request.max_output_tokens {
        generation.insert("maxOutputTokens".into(), json!(value));
    }
    if let Some(value) = request.presence_penalty {
        generation.insert("presencePenalty".into(), json!(value));
    }
    if let Some(value) = request.frequency_penalty {
        generation.insert("frequencyPenalty".into(), json!(value));
    }
    if let Some(value) = request.seed {
        generation.insert("seed".into(), json!(value));
    }
    match &request.stop {
        Some(Stop::One(stop)) => {
            generation.insert("stopSequences".into(), json!([stop]));
        }
        Some(Stop::Many(stops)) => {
            generation.insert("stopSequences".into(), json!(stops));
        }
        None => {}
    }
    match request.text.as_ref().and_then(|text| text.format.as_ref()) {
        Some(TextFormat::JsonObject(_)) => {
            generation.insert("responseMimeType".into(), json!("application/json"));
        }
        Some(TextFormat::JsonSchema(format)) => {
            generation.insert("responseMimeType".into(), json!("application/json"));
            generation.insert("responseJsonSchema".into(), format.schema.clone());
        }
        _ => {}
    }
    if row.capabilities.reasoning {
        let mut thinking_config = Map::new();
        thinking_config.insert("includeThoughts".into(), json!(true));
        if let Some(effort) = request.reasoning.as_ref().and_then(|r| r.effort.as_ref()) {
            match thinking {
                Thinking::Level => {
                    let level = match effort {
                        ReasoningEffort::None | ReasoningEffort::Minimal => "minimal",
                        ReasoningEffort::Low => "low",
                        ReasoningEffort::Medium => "medium",
                        _ => "high",
                    };
                    thinking_config.insert("thinkingLevel".into(), json!(level));
                }
                Thinking::Budget => {
                    let budget = match effort {
                        ReasoningEffort::None => 0,
                        ReasoningEffort::Minimal => 512,
                        ReasoningEffort::Low => 1_024,
                        ReasoningEffort::Medium => 8_192,
                        _ => 24_576,
                    };
                    thinking_config.insert("thinkingBudget".into(), json!(budget));
                    if budget == 0 {
                        thinking_config.insert("includeThoughts".into(), json!(false));
                    }
                }
            }
        }
        generation.insert("thinkingConfig".into(), Value::Object(thinking_config));
    }
    if !generation.is_empty() {
        body.insert("generationConfig".into(), Value::Object(generation));
    }
    Ok(Value::Object(body))
}

fn tools(request: &CreateResponse) -> (Vec<Value>, Option<Value>) {
    let allowed: Option<&[String]> = match &request.tool_choice {
        Some(ToolChoice::AllowedTools { tools, .. }) => Some(tools),
        _ => None,
    };
    let declarations: Vec<Value> = request
        .tools
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|tool| match tool {
            Tool::Function(function) => Some(function),
            Tool::Unknown(_) => None,
        })
        .filter(|function| allowed.is_none_or(|names| names.contains(&function.name)))
        .map(|function| {
            let mut declaration = Map::new();
            declaration.insert("name".into(), json!(function.name));
            if let Some(description) = &function.description {
                declaration.insert("description".into(), json!(description));
            }
            if let Some(parameters) = &function.parameters {
                declaration.insert("parametersJsonSchema".into(), parameters.clone());
            }
            Value::Object(declaration)
        })
        .collect();
    let mode = |mode: &ToolChoiceMode| match mode {
        ToolChoiceMode::None => "NONE",
        ToolChoiceMode::Auto => "AUTO",
        ToolChoiceMode::Required => "ANY",
    };
    let config = match &request.tool_choice {
        None => None,
        Some(ToolChoice::Mode(choice)) => Some(json!({"mode": mode(choice)})),
        Some(ToolChoice::Function { name }) => {
            Some(json!({"mode": "ANY", "allowedFunctionNames": [name]}))
        }
        Some(ToolChoice::AllowedTools {
            mode: choice,
            tools,
        }) => {
            let word = mode(choice);
            if word == "ANY" {
                Some(json!({"mode": word, "allowedFunctionNames": tools}))
            } else {
                Some(json!({"mode": word}))
            }
        }
    };
    (declarations, config)
}

/// Gemini's usage metadata as Open Responses usage.
#[must_use]
pub fn usage(metadata: &Value) -> Usage {
    let count = |key: &str| metadata[key].as_u64().unwrap_or_default();
    let input = count("promptTokenCount") + count("toolUsePromptTokenCount");
    let thoughts = count("thoughtsTokenCount");
    Usage::new(
        input,
        count("cachedContentTokenCount"),
        count("candidatesTokenCount") + thoughts,
        thoughts,
    )
}

fn finish_of(reason: &str) -> Finish {
    match reason {
        "MAX_TOKENS" => Finish::Incomplete(IncompleteReason::MaxOutputTokens),
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY"
        | "LANGUAGE" => Finish::Incomplete(IncompleteReason::ContentFilter),
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" => Finish::Failed(ResponseError {
            code: "model_error".into(),
            message: "the model wrote a function call that could not be read".into(),
            extra: Extra::new(),
        }),
        _ => Finish::Completed,
    }
}

struct Reader {
    frames: Frames,
    emitter: Emitter,
    gate: Gate,
    finish: Option<Finish>,
    calls: u64,
}

impl Reader {
    fn chunk(&mut self, data: &str) -> Result<(), AttemptError> {
        let chunk: Value = serde_json::from_str(data).map_err(|_| {
            AttemptError::new(ErrorClass::Decode, "Vertex sent a chunk that is not JSON")
        })?;
        let chunk = chunk
            .as_array()
            .and_then(|list| list.first())
            .unwrap_or(&chunk)
            .clone();
        if let Some(error) = chunk.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Vertex failed");
            return Err(AttemptError::new(ErrorClass::Upstream, clean(message, &[])));
        }
        if let Some(metadata) = chunk.get("usageMetadata").filter(|value| value.is_object()) {
            self.emitter.usage(usage(metadata));
        }
        if chunk["promptFeedback"]["blockReason"].is_string() {
            self.finish = Some(Finish::Incomplete(IncompleteReason::ContentFilter));
        }
        let Some(candidate) = chunk["candidates"].as_array().and_then(|list| list.first()) else {
            return Ok(());
        };
        let mut events = Vec::new();
        let parts = candidate["content"]["parts"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        for part in parts {
            if let Some(call) = part.get("functionCall") {
                if let Some(signature) = part["thoughtSignature"].as_str() {
                    events.extend(self.emitter.reasoning_signature(signature));
                }
                let key = self.calls.to_string();
                self.calls += 1;
                let name = call["name"].as_str().unwrap_or_default();
                let id = call["id"].as_str().unwrap_or_default();
                let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
                events.extend(self.emitter.call_start(&key, id, name));
                events.extend(self.emitter.call_arguments(&key, &args.to_string()));
            } else if let Some(text) = part["text"].as_str() {
                if part["thought"].as_bool() == Some(true) {
                    events.extend(self.emitter.reasoning_summary(text));
                } else {
                    events.extend(self.emitter.text(text));
                }
            }
        }
        if let Some(reason) = candidate["finishReason"].as_str() {
            self.finish = Some(finish_of(reason));
        }
        for event in events {
            self.gate.push(event);
        }
        Ok(())
    }

    /// The upstream's stream ended.
    fn end(&mut self) {
        if let Some(finish) = self.finish.take() {
            for event in self.emitter.finish(finish) {
                self.gate.push(event);
            }
        }
        self.gate.end();
    }

    /// A failure: the router's before the first token, the caller's after.
    fn fail(&mut self, error: AttemptError) {
        if self.gate.output_started() {
            let failed = Finish::Failed(ResponseError {
                code: "upstream_failed".into(),
                message: error.message.clone(),
                extra: Extra::new(),
            });
            for event in self.emitter.finish(failed) {
                self.gate.push(event);
            }
        }
        self.gate.fail(error);
    }
}

/// A `streamGenerateContent` stream as Open Responses events, built by
/// `emitter` for the caller of `model` (the public id).
#[must_use]
pub fn events(frames: Frames, emitter: Emitter, model: &str) -> EventStream {
    let reader = Reader {
        frames,
        emitter,
        gate: Gate::new(model, &[]),
        finish: None,
        calls: 0,
    };
    Box::pin(futures_util::stream::unfold(
        reader,
        |mut reader| async move {
            loop {
                if let Some(item) = reader.gate.next() {
                    return Some((item, reader));
                }
                if reader.gate.closed() {
                    return None;
                }
                match reader.frames.next().await {
                    None => reader.end(),
                    Some(Err(error)) => reader.fail(error),
                    Some(Ok(frame)) => {
                        let data = frame.data.trim();
                        if data == "[DONE]" {
                            reader.end();
                        } else if !data.is_empty()
                            && let Err(error) = reader.chunk(data)
                        {
                            reader.fail(error);
                        }
                    }
                }
            }
        },
    ))
}
