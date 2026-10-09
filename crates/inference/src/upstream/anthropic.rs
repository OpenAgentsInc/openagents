//! Anthropic Messages on the caller's key, translated to Open Responses.

use std::collections::BTreeSet;

use futures_util::StreamExt;
use serde_json::{Value, json};

use super::emit::{Emitter, Finish};
use super::gate::Gate;
use super::http::Frames;
use super::secret::Secret;
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream, check,
};
use crate::item::{ContentPart, Item, MessageContent, Role, ToolOutput};
use crate::request::{CreateResponse, ReasoningEffort, Stop, Tool, ToolChoice, ToolChoiceMode};
use crate::response::{IncompleteReason, Usage};

pub struct Anthropic {
    key: Secret,
    pub url: String,
    account: Account,
    privacy: PrivacyTerms,
    rows: Vec<ModelRow>,
    http: reqwest::Client,
}

impl Anthropic {
    pub fn new(key: Secret) -> Self {
        Self {
            key,
            url: "https://api.anthropic.com/v1/messages".into(),
            account: Account {
                id: crate::run::CALLER_KEY.into(),
                basis: CostBasis::PayAsYouGo,
            },
            privacy: PrivacyTerms::unverified(
                "Anthropic: the caller's retention agreement is not verified",
            ),
            rows: vec![ModelRow {
                id: "anthropic/claude-sonnet-5-5".into(),
                upstream_model: "claude-sonnet-5-5".into(),
                capabilities: Capabilities {
                    tools: true,
                    reasoning: true,
                    reasoning_always_on: false,
                    json_schema: false,
                    images: true,
                    context: 1_000_000,
                    max_output: 128_000,
                },
                price: Price::micro(2_000_000, 200_000, 10_000_000),
                price_source: "https://platform.claude.com/docs/en/models/overview (2026-10-09)",
            }],
            http: super::http::client(super::http::CONNECT_TIMEOUT),
        }
    }
}

fn unsupported(text: &str) -> AttemptError {
    AttemptError::new(ErrorClass::Unsupported, text)
}

fn parts(content: &MessageContent) -> Result<Vec<Value>, AttemptError> {
    match content {
        MessageContent::Text(text) => Ok(vec![json!({"type": "text", "text": text})]),
        MessageContent::Parts(parts) => parts
            .iter()
            .map(|part| {
                if let Some(text) = part.text() {
                    return Ok(json!({"type": "text", "text": text}));
                }
                if let ContentPart::InputImage(image) = part {
                    let url = image
                        .image_url
                        .as_deref()
                        .ok_or_else(|| unsupported("An image needs a URL."))?;
                    let source = if let Some(data) = url.strip_prefix("data:") {
                        let (mime, data) = data
                            .split_once(";base64,")
                            .ok_or_else(|| unsupported("Use a base64 image or an image URL."))?;
                        json!({"type": "base64", "media_type": mime, "data": data})
                    } else {
                        json!({"type": "url", "url": url})
                    };
                    return Ok(json!({"type": "image", "source": source}));
                }
                Err(unsupported("Anthropic can't use this content type."))
            })
            .collect(),
    }
}

fn push(messages: &mut Vec<Value>, role: &str, content: Vec<Value>) {
    if let Some(last) = messages.last_mut().filter(|last| last["role"] == role) {
        last["content"].as_array_mut().unwrap().extend(content);
    } else {
        messages.push(json!({"role": role, "content": content}));
    }
}

/// Translates text, images, tool history, and signed thinking history.
pub fn body(request: &CreateResponse, row: &ModelRow) -> Result<Value, AttemptError> {
    if request.previous_response_id.is_some() {
        return Err(unsupported("previous_response_id needs stored responses"));
    }
    if request
        .text
        .as_ref()
        .and_then(|text| text.format.as_ref())
        .is_some_and(|format| !matches!(format, crate::request::TextFormat::Text(_)))
    {
        return Err(unsupported("Anthropic can't use this output format."));
    }
    let mut system = request.instructions.clone().into_iter().collect::<Vec<_>>();
    let mut messages = Vec::new();
    for item in request.input_items() {
        match item {
            Item::Message(message) if matches!(message.role, Role::System | Role::Developer) => {
                let content = parts(&message.content)?;
                if content.iter().any(|part| part["type"] != "text") {
                    return Err(unsupported("System instructions must be text."));
                }
                system.extend(
                    content
                        .iter()
                        .filter_map(|part| part["text"].as_str().map(str::to_owned)),
                );
            }
            Item::Message(message) => push(
                &mut messages,
                if message.role == Role::Assistant {
                    "assistant"
                } else {
                    "user"
                },
                parts(&message.content)?,
            ),
            Item::FunctionCall(call) => {
                let input: Value = serde_json::from_str(&call.arguments)
                    .map_err(|_| unsupported("Tool arguments must be JSON."))?;
                if !input.is_object() {
                    return Err(unsupported("Tool arguments must be a JSON object."));
                }
                push(
                    &mut messages,
                    "assistant",
                    vec![
                        json!({"type": "tool_use", "id": call.call_id, "name": call.name, "input": input}),
                    ],
                );
            }
            Item::FunctionCallOutput(output) => {
                let content = match &output.output {
                    ToolOutput::Text(text) => vec![json!({"type": "text", "text": text})],
                    ToolOutput::Parts(content) => parts(&MessageContent::Parts(content.clone()))?,
                };
                push(
                    &mut messages,
                    "user",
                    vec![
                        json!({"type": "tool_result", "tool_use_id": output.call_id, "content": content}),
                    ],
                );
            }
            Item::Reasoning(reasoning) => {
                let Some(signature) = &reasoning.encrypted_content else {
                    return Err(unsupported("Anthropic needs its signed thinking history."));
                };
                let text = reasoning
                    .summary
                    .iter()
                    .filter_map(ContentPart::text)
                    .collect::<String>();
                push(
                    &mut messages,
                    "assistant",
                    vec![json!({"type": "thinking", "thinking": text, "signature": signature})],
                );
            }
            _ => return Err(unsupported("Anthropic can't use this input item.")),
        }
    }
    let mut body = json!({"model": row.upstream_model, "max_tokens": request.max_output_tokens.unwrap_or(4096), "stream": true, "messages": messages});
    if !system.is_empty() {
        body["system"] = json!(system.join("\n\n"));
    }
    if let Some(value) = request.temperature {
        body["temperature"] = json!(value);
    }
    if let Some(value) = request.top_p {
        body["top_p"] = json!(value);
    }
    match &request.stop {
        Some(Stop::One(stop)) => body["stop_sequences"] = json!([stop]),
        Some(Stop::Many(stops)) => body["stop_sequences"] = json!(stops),
        None => {}
    }
    let allowed = match &request.tool_choice {
        Some(ToolChoice::AllowedTools { tools, .. }) => Some(tools),
        _ => None,
    };
    let tools: Vec<_> = request.tools.as_deref().unwrap_or_default().iter().filter_map(|tool| match tool {
        Tool::Function(tool) if allowed.is_none_or(|names| names.contains(&tool.name)) => Some(json!({"name": tool.name, "description": tool.description, "input_schema": tool.parameters.clone().unwrap_or_else(|| json!({"type": "object"}))})),
        _ => None,
    }).collect();
    if !tools.is_empty() {
        body["tools"] = json!(tools);
        let choice = match &request.tool_choice {
            Some(ToolChoice::Function { name }) => json!({"type": "tool", "name": name}),
            Some(ToolChoice::Mode(mode)) | Some(ToolChoice::AllowedTools { mode, .. }) => {
                json!({"type": match mode { ToolChoiceMode::None => "none", ToolChoiceMode::Required => "any", _ => "auto" }})
            }
            None => json!({"type": "auto"}),
        };
        body["tool_choice"] = choice;
    }
    if let Some(effort) = request.reasoning.as_ref().and_then(|r| r.effort.as_ref()) {
        if matches!(effort, ReasoningEffort::None) {
            body["thinking"] = json!({"type": "disabled"});
        } else {
            if matches!(
                &request.tool_choice,
                Some(ToolChoice::Function { .. })
                    | Some(ToolChoice::Mode(ToolChoiceMode::Required))
                    | Some(ToolChoice::AllowedTools {
                        mode: ToolChoiceMode::Required,
                        ..
                    })
            ) {
                return Err(unsupported(
                    "Anthropic can't combine thinking with a forced tool.",
                ));
            }
            body["thinking"] = json!({"type": "adaptive"});
            body["output_config"] = json!({"effort": match effort { ReasoningEffort::Minimal | ReasoningEffort::Low => "low", ReasoningEffort::Medium => "medium", _ => "high" }});
        }
    }
    Ok(body)
}

impl Upstream for Anthropic {
    fn name(&self) -> &str {
        "anthropic"
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.rows
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            let body = body(request, row)?;
            let meter = AttemptMeter::start(self, row);
            let call = self
                .http
                .post(&self.url)
                .header("x-api-key", self.key.expose())
                .header("anthropic-version", "2023-06-01")
                .header("accept", "text/event-stream")
                .json(&body);
            let frames = match super::http::open_stream(call, &[self.key.expose()]).await {
                Ok(frames) => frames,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events = events(frames, Emitter::new(model, request), model);
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

struct Reader {
    frames: Frames,
    emitter: Emitter,
    gate: Gate,
    usage: Value,
    finish: Option<Finish>,
    empty_calls: BTreeSet<String>,
}

impl Reader {
    fn chunk(&mut self, data: &str) -> Result<(), AttemptError> {
        let chunk: Value = serde_json::from_str(data)
            .map_err(|_| AttemptError::new(ErrorClass::Decode, "Anthropic sent invalid JSON."))?;
        let key = chunk["index"].as_u64().unwrap_or_default().to_string();
        let mut events = Vec::new();
        match chunk["type"].as_str().unwrap_or_default() {
            "content_block_stop" => {
                if self.empty_calls.remove(&key) {
                    events.extend(self.emitter.call_arguments(&key, "{}"));
                }
                events.extend(self.emitter.end_block());
            }
            "message_start" => self.usage = chunk["message"]["usage"].clone(),
            "content_block_start" => {
                let block = &chunk["content_block"];
                match block["type"].as_str().unwrap_or_default() {
                    "text" => events.extend(
                        self.emitter
                            .text(block["text"].as_str().unwrap_or_default()),
                    ),
                    "tool_use" => {
                        self.empty_calls.insert(key.clone());
                        events.extend(self.emitter.call_start(
                            &key,
                            block["id"].as_str().unwrap_or_default(),
                            block["name"].as_str().unwrap_or_default(),
                        ));
                        if let Some(input) =
                            block["input"].as_object().filter(|input| !input.is_empty())
                        {
                            self.empty_calls.remove(&key);
                            events.extend(
                                self.emitter.call_arguments(&key, &json!(input).to_string()),
                            );
                        }
                    }
                    "thinking" => events.extend(
                        self.emitter
                            .reasoning_summary(block["thinking"].as_str().unwrap_or_default()),
                    ),
                    _ => return Err(unsupported("Anthropic sent an unsupported content block.")),
                }
            }
            "content_block_delta" => {
                let delta = &chunk["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => events.extend(
                        self.emitter
                            .text(delta["text"].as_str().unwrap_or_default()),
                    ),
                    "input_json_delta" => {
                        self.empty_calls.remove(&key);
                        events.extend(self.emitter.call_arguments(
                            &key,
                            delta["partial_json"].as_str().unwrap_or_default(),
                        ))
                    }
                    "thinking_delta" => events.extend(
                        self.emitter
                            .reasoning_summary(delta["thinking"].as_str().unwrap_or_default()),
                    ),
                    "signature_delta" => events.extend(
                        self.emitter
                            .reasoning_signature(delta["signature"].as_str().unwrap_or_default()),
                    ),
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(usage) = chunk["usage"].as_object() {
                    if !self.usage.is_object() {
                        self.usage = json!({});
                    }
                    for (name, value) in usage {
                        self.usage[name] = value.clone();
                    }
                }
                self.finish = Some(match chunk["delta"]["stop_reason"].as_str() {
                    Some("max_tokens") => Finish::Incomplete(IncompleteReason::MaxOutputTokens),
                    Some("refusal") => Finish::Incomplete(IncompleteReason::ContentFilter),
                    Some("end_turn" | "stop_sequence" | "tool_use" | "pause_turn") => {
                        Finish::Completed
                    }
                    _ => {
                        return Err(AttemptError::new(
                            ErrorClass::Decode,
                            "Anthropic sent an unknown stop reason.",
                        ));
                    }
                });
            }
            "message_stop" => {
                let count = |name: &str| self.usage[name].as_u64().unwrap_or_default();
                let cached = count("cache_read_input_tokens");
                let write = count("cache_creation_input_tokens");
                let mut usage = Usage::new(
                    count("input_tokens") + cached + write,
                    cached,
                    count("output_tokens"),
                    0,
                );
                usage
                    .input_tokens_details
                    .extra
                    .insert("cache_write_tokens".into(), json!(write));
                if self.usage["input_tokens"].is_u64() && self.usage["output_tokens"].is_u64() {
                    self.emitter.usage(usage);
                }
                let finish = self.finish.take().ok_or_else(|| {
                    AttemptError::new(ErrorClass::Decode, "Anthropic ended without a stop reason.")
                })?;
                events.extend(self.emitter.finish(finish));
            }
            "error" => {
                return Err(AttemptError::new(
                    ErrorClass::Upstream,
                    "Anthropic couldn't finish this response.",
                ));
            }
            _ => {}
        }
        for event in events {
            self.gate.push(event);
        }
        Ok(())
    }
}

/// Reads Messages SSE, preserving first-output failure behavior.
pub fn events(frames: Frames, emitter: Emitter, model: &str) -> EventStream {
    Box::pin(futures_util::stream::unfold(
        Reader {
            frames,
            emitter,
            gate: Gate::new(model, &[]),
            usage: Value::Null,
            finish: None,
            empty_calls: BTreeSet::new(),
        },
        |mut reader| async move {
            loop {
                if let Some(event) = reader.gate.next() {
                    return Some((event, reader));
                }
                if reader.gate.closed() {
                    return None;
                }
                match reader.frames.next().await {
                    None => reader.gate.end(),
                    Some(Err(error)) => reader.gate.fail(error),
                    Some(Ok(frame)) => {
                        if let Err(error) = reader.chunk(&frame.data) {
                            reader.gate.fail(error);
                        }
                    }
                }
            }
        },
    ))
}
