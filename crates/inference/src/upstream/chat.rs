//! Chat Completions upstreams (Z.ai, the Pro door).
//!
//! The request goes out through the crate's own translation
//! ([`crate::chat::from_responses_request`]) and the chunks come back
//! through [`crate::chat::EventWriter`] and the first-token [`Gate`], so
//! this module holds only what differs between Chat Completions upstreams:
//! a [`Dialect`].

use futures_util::StreamExt;
use serde_json::{Value, json};

use super::gate::Gate;
use super::http::{Frames, clean};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, ErrorClass, EventStream, ModelRow,
    PrivacyTerms, Sent, Upstream, check, secret::Secret, unconfigured,
};
use crate::chat::{
    ChatMessage, ChatRequest, ChatShape, ChatToolChoice, EventWriter, decode_chunk,
    from_responses_request,
};
use crate::request::CreateResponse;

/// How an upstream takes a reasoning setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasoningStyle {
    /// `reasoning_effort: "<effort>"` (OpenAI).
    Effort,
    /// `thinking: {"type": "enabled"}`, always: Z.ai's GLM thinks on every
    /// request, and its docs say Flash cannot turn it off.
    AlwaysOn,
}

/// The differences between Chat Completions upstreams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dialect {
    /// Send the output cap as `max_tokens` rather than
    /// `max_completion_tokens`.
    pub legacy_max_tokens: bool,
    pub reasoning: ReasoningStyle,
    /// Drop `temperature: 0`, which OpenAI's reasoning models refuse (the
    /// Pro door did this).
    pub drop_zero_temperature: bool,
    /// Whether the upstream takes OpenAI's `developer` role,
    /// `allowed_tools` tool choice, and `parallel_tool_calls`; without
    /// them, `developer` becomes `system` and `allowed_tools` becomes the
    /// allowed subset of the tools with its mode.
    pub openai_extensions: bool,
}

/// The Chat Completions body for `request` to `upstream_model`: always a
/// stream with usage, without our extension object or any field that asks
/// the upstream to store anything.
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for an input Chat Completions cannot carry.
pub fn body(
    request: &CreateResponse,
    upstream_model: &str,
    dialect: &Dialect,
) -> Result<ChatRequest, AttemptError> {
    let shape = ChatShape {
        include_usage: true,
        legacy_max_tokens: dialect.legacy_max_tokens,
        ..ChatShape::default()
    };
    let mut chat = from_responses_request(request, &shape)
        .map_err(|error| AttemptError::new(ErrorClass::Unsupported, error.message))?;
    chat.model = Some(upstream_model.to_owned());
    chat.stream = Some(true);
    chat.openagents = None;
    chat.store = None;
    chat.metadata = None;
    chat.service_tier = None;
    chat.n = None;
    for message in &mut chat.messages {
        match message {
            // Our reasoning extension on assistant messages is OpenRouter's;
            // these upstreams take no such field.
            ChatMessage::Assistant { reasoning, .. } => *reasoning = None,
            ChatMessage::Developer { content, name } if !dialect.openai_extensions => {
                *message = ChatMessage::System {
                    content: content.clone(),
                    name: name.clone(),
                };
            }
            _ => {}
        }
    }
    if !dialect.openai_extensions {
        chat.parallel_tool_calls = None;
        chat.safety_identifier = None;
        chat.prompt_cache_key = None;
        if let Some(ChatToolChoice::AllowedTools { mode, tools }) = chat.tool_choice.clone() {
            if let Some(list) = chat.tools.as_mut() {
                list.retain(|tool| tools.contains(&tool.function.name));
            }
            chat.tool_choice = Some(ChatToolChoice::Mode(mode));
        }
    }
    if dialect.drop_zero_temperature && chat.temperature == Some(0.0) {
        chat.temperature = None;
    }
    match dialect.reasoning {
        ReasoningStyle::Effort => {}
        ReasoningStyle::AlwaysOn => {
            chat.reasoning_effort = None;
            chat.extra
                .insert("thinking".into(), json!({"type": "enabled"}));
        }
    }
    Ok(chat)
}

/// Moves a chunk's `reasoning_content` (Z.ai, DeepSeek) to `reasoning`,
/// the field the event writer reads.
fn normalize_chunk(value: &mut Value) {
    let Some(choices) = value.get_mut("choices").and_then(Value::as_array_mut) else {
        return;
    };
    for choice in choices {
        let Some(delta) = choice.get_mut("delta").and_then(Value::as_object_mut) else {
            continue;
        };
        if let Some(text) = delta.remove("reasoning_content")
            && !delta.get("reasoning").is_some_and(Value::is_string)
        {
            delta.insert("reasoning".into(), text);
        }
    }
}

struct Reader {
    frames: Frames,
    writer: EventWriter,
    gate: Gate,
    scrub: &'static [&'static str],
    /// Whether a chunk has said how the answer finished.
    finished: bool,
}

impl Reader {
    fn chunk(&mut self, data: &str) {
        let mut value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => {
                return self.gate.fail(AttemptError::new(
                    ErrorClass::Decode,
                    "the upstream sent a chunk that is not JSON",
                ));
            }
        };
        if let Some(error) = value.get("error").filter(|error| error.is_object()) {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the upstream failed");
            let message = clean(message, self.scrub);
            if !self.gate.output_started() {
                return self
                    .gate
                    .fail(AttemptError::new(ErrorClass::Upstream, message));
            }
            value = json!({
                "id": "",
                "choices": [],
                "error": {"type": "upstream_failed", "code": "upstream_failed", "param": null, "message": message},
            });
        }
        normalize_chunk(&mut value);
        if let Some(object) = value.as_object_mut() {
            // The response keeps the public id the writer started with.
            object.remove("model");
            object.entry("id").or_insert_with(|| json!(""));
        }
        let chunk = match decode_chunk(&value.to_string()) {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return,
            Err(_) => {
                return self.gate.fail(AttemptError::new(
                    ErrorClass::Decode,
                    "the upstream sent a chunk we could not read",
                ));
            }
        };
        if chunk.error.is_some() || chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
            self.finished = true;
        }
        let failed = chunk.error.is_some();
        for event in self.writer.push(&chunk) {
            self.gate.push(event);
        }
        if failed {
            self.finish();
        }
    }

    /// The upstream's stream ended (`[DONE]` or the connection closed).
    fn finish(&mut self) {
        if self.finished {
            for event in self.writer.finish(super::emit::unix_now()) {
                self.gate.push(event);
            }
        }
        self.gate.end();
    }
}

/// A Chat Completions stream as Open Responses events, for the caller of
/// `request` served as `model` (the public id).
#[must_use]
pub fn events(
    frames: Frames,
    request: &CreateResponse,
    model: &str,
    scrub: &'static [&'static str],
) -> EventStream {
    let mut public = request.clone();
    public.model = Some(model.to_owned());
    let reader = Reader {
        frames,
        writer: EventWriter::new(&public),
        gate: Gate::new(model, scrub),
        scrub,
        finished: false,
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
                    None => reader.finish(),
                    Some(Err(error)) => reader.gate.fail(error),
                    Some(Ok(frame)) => {
                        let data = frame.data.trim();
                        if data == "[DONE]" {
                            reader.finish();
                        } else if !data.is_empty() {
                            reader.chunk(data);
                        }
                    }
                }
            }
        },
    ))
}

/// The settings of one Chat Completions upstream.
#[derive(Clone, Debug)]
pub struct ChatConfig {
    /// `zai`, `pro`.
    pub name: &'static str,
    /// The full `.../chat/completions` URL.
    pub url: String,
    pub key: Option<Secret>,
    /// Extra headers (name, value), such as the Pro door's customer id.
    pub headers: Vec<(String, String)>,
    pub dialect: Dialect,
    pub account: Account,
    pub privacy: PrivacyTerms,
    pub models: Vec<ModelRow>,
    /// Words removed from upstream error messages before anyone sees them.
    pub scrub: &'static [&'static str],
}

/// An adapter for a Chat Completions upstream.
pub struct ChatUpstream {
    config: ChatConfig,
    http: reqwest::Client,
}

impl ChatUpstream {
    #[must_use]
    pub fn new(config: ChatConfig) -> Self {
        Self {
            config,
            http: super::http::client(super::http::CONNECT_TIMEOUT),
        }
    }

    /// The settings.
    #[must_use]
    pub fn config(&self) -> &ChatConfig {
        &self.config
    }
}

impl Upstream for ChatUpstream {
    fn name(&self) -> &'static str {
        self.config.name
    }

    fn account(&self) -> &Account {
        &self.config.account
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.config.privacy
    }

    fn models(&self) -> &[ModelRow] {
        &self.config.models
    }

    fn configured(&self) -> bool {
        self.config.key.is_some()
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            let Some(key) = &self.config.key else {
                return Err(unconfigured(self.config.name, "API key"));
            };
            let body = body(request, &row.upstream_model, &self.config.dialect)?;
            let meter = AttemptMeter::start(self, row);
            let mut call = self
                .http
                .post(&self.config.url)
                .bearer_auth(key.expose())
                .header("accept", "text/event-stream")
                .json(&body);
            for (name, value) in &self.config.headers {
                call = call.header(name, value);
            }
            let frames = match super::http::open_stream(call, self.config.scrub).await {
                Ok(frames) => frames,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events = events(frames, request, &row.id, self.config.scrub);
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}
