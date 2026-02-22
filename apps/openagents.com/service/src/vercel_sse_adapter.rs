use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedLegacyStreamRequest {
    pub thread_id: String,
    pub worker_id: Option<String>,
    pub user_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdapterError {
    #[error("conversation id is required")]
    MissingConversationId,
    #[error("legacy stream payload must include user message text")]
    MissingUserText,
    #[error("start-step emitted before start")]
    StartStepBeforeStart,
    #[error("finish-step emitted before start-step")]
    FinishStepBeforeStartStep,
    #[error("finish emitted before finish-step")]
    FinishBeforeStep,
    #[error("duplicate start event")]
    DuplicateStart,
    #[error("duplicate start-step event")]
    DuplicateStartStep,
    #[error("duplicate finish-step event")]
    DuplicateFinishStep,
    #[error("duplicate finish event")]
    DuplicateFinish,
    #[error("tool output emitted before tool input for {0}")]
    ToolOutputBeforeInput(String),
    #[error("missing finish event")]
    MissingFinish,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexCompatibilityEvent {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SerializedVercelStream {
    pub events: Vec<Value>,
    pub wire: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdapterInputEvent {
    Start {
        thread_id: String,
    },
    StartStep {
        thread_id: String,
        turn_id: String,
        model: String,
    },
    TextStart {
        id: String,
        turn_id: String,
    },
    TextDelta {
        id: String,
        delta: String,
        channel: &'static str,
    },
    ToolInput {
        tool_call_id: String,
        tool_name: String,
        input: Value,
    },
    ToolOutput {
        tool_call_id: String,
        delta: Option<String>,
        status: Option<String>,
    },
    FinishStep {
        turn_id: String,
        status: String,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
    Finish {
        status: String,
    },
}

#[derive(Debug, Default)]
pub struct VercelSseAdapter {
    emitted_start: bool,
    emitted_start_step: bool,
    emitted_finish_step: bool,
    emitted_finish: bool,
    text_started: HashSet<String>,
    tool_inputs: HashSet<String>,
}

impl VercelSseAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: AdapterInputEvent) -> Result<Vec<Value>, AdapterError> {
        match event {
            AdapterInputEvent::Start { thread_id } => {
                if self.emitted_start {
                    return Err(AdapterError::DuplicateStart);
                }
                self.emitted_start = true;
                Ok(vec![json!({
                    "type": "start",
                    "threadId": thread_id,
                    "source": "codex"
                })])
            }
            AdapterInputEvent::StartStep {
                thread_id,
                turn_id,
                model,
            } => {
                if !self.emitted_start {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                if self.emitted_start_step {
                    return Err(AdapterError::DuplicateStartStep);
                }
                self.emitted_start_step = true;
                Ok(vec![json!({
                    "type": "start-step",
                    "threadId": thread_id,
                    "turnId": turn_id,
                    "model": model
                })])
            }
            AdapterInputEvent::TextStart { id, turn_id } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                if self.text_started.insert(id.clone()) {
                    Ok(vec![json!({
                        "type": "text-start",
                        "id": id,
                        "turnId": turn_id
                    })])
                } else {
                    Ok(vec![])
                }
            }
            AdapterInputEvent::TextDelta { id, delta, channel } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                let mut out = Vec::new();
                if self.text_started.insert(id.clone()) {
                    out.push(json!({
                        "type": "text-start",
                        "id": id,
                        "turnId": ""
                    }));
                }
                out.push(json!({
                    "type": "text-delta",
                    "id": id,
                    "channel": channel,
                    "delta": delta
                }));
                Ok(out)
            }
            AdapterInputEvent::ToolInput {
                tool_call_id,
                tool_name,
                input,
            } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                self.tool_inputs.insert(tool_call_id.clone());
                Ok(vec![json!({
                    "type": "tool-input",
                    "toolCallId": tool_call_id,
                    "toolName": tool_name,
                    "input": input,
                })])
            }
            AdapterInputEvent::ToolOutput {
                tool_call_id,
                delta,
                status,
            } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                if !self.tool_inputs.contains(&tool_call_id) {
                    return Err(AdapterError::ToolOutputBeforeInput(tool_call_id));
                }
                let mut payload = Map::new();
                payload.insert("type".to_string(), Value::String("tool-output".to_string()));
                payload.insert(
                    "toolCallId".to_string(),
                    Value::String(tool_call_id.to_string()),
                );
                if let Some(delta) = delta {
                    payload.insert("delta".to_string(), Value::String(delta));
                }
                if let Some(status) = status {
                    payload.insert("status".to_string(), Value::String(status));
                }
                Ok(vec![Value::Object(payload)])
            }
            AdapterInputEvent::FinishStep { turn_id, status } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::FinishStepBeforeStartStep);
                }
                if self.emitted_finish_step {
                    return Err(AdapterError::DuplicateFinishStep);
                }
                self.emitted_finish_step = true;
                Ok(vec![json!({
                    "type": "finish-step",
                    "turnId": turn_id,
                    "status": status,
                })])
            }
            AdapterInputEvent::Error {
                code,
                message,
                retryable,
            } => {
                if !self.emitted_start_step {
                    return Err(AdapterError::StartStepBeforeStart);
                }
                Ok(vec![json!({
                    "type": "error",
                    "code": code,
                    "message": message,
                    "retryable": retryable,
                })])
            }
            AdapterInputEvent::Finish { status } => {
                if self.emitted_finish {
                    return Err(AdapterError::DuplicateFinish);
                }
                if status != "error" && !self.emitted_finish_step {
                    return Err(AdapterError::FinishBeforeStep);
                }
                self.emitted_finish = true;
                Ok(vec![json!({
                    "type": "finish",
                    "status": status,
                })])
            }
        }
    }
}

pub fn normalize_legacy_stream_request(
    conversation_id: Option<&str>,
    payload: &Value,
) -> Result<NormalizedLegacyStreamRequest, AdapterError> {
    let thread_id = match conversation_id {
        Some(id) => normalize_thread_id(id)?,
        None => payload_thread_id(payload)
            .unwrap_or_else(|| format!("thread_{}", Uuid::new_v4().simple())),
    };
    let user_text = payload_user_text(payload).ok_or(AdapterError::MissingUserText)?;
    let worker_id = payload_worker_id(payload);

    Ok(NormalizedLegacyStreamRequest {
        thread_id,
        worker_id,
        user_text,
    })
}

pub fn translate_events(
    events: &[AdapterInputEvent],
) -> Result<SerializedVercelStream, AdapterError> {
    let mut adapter = VercelSseAdapter::new();
    let mut out = Vec::new();

    for event in events {
        out.extend(adapter.push(event.clone())?);
    }

    if !adapter.emitted_finish {
        return Err(AdapterError::MissingFinish);
    }

    Ok(SerializedVercelStream {
        wire: serialize_sse(&out),
        events: out,
    })
}

pub fn translate_codex_events(
    events: &[CodexCompatibilityEvent],
) -> Result<SerializedVercelStream, AdapterError> {
    let mut mapped = Vec::new();

    for event in events {
        mapped.extend(map_codex_event(event));
    }

    translate_events(&mapped)
}

pub fn build_turn_start_preview(
    thread_id: &str,
    turn_id: Option<&str>,
) -> Result<SerializedVercelStream, AdapterError> {
    let normalized_turn_id = non_empty(turn_id.unwrap_or_default().to_string())
        .unwrap_or_else(|| "turn_pending".to_string());

    translate_events(&[
        AdapterInputEvent::Start {
            thread_id: thread_id.to_string(),
        },
        AdapterInputEvent::StartStep {
            thread_id: thread_id.to_string(),
            turn_id: normalized_turn_id.clone(),
            model: "gpt-5.2-codex".to_string(),
        },
        AdapterInputEvent::FinishStep {
            turn_id: normalized_turn_id,
            status: "accepted".to_string(),
        },
        AdapterInputEvent::Finish {
            status: "accepted".to_string(),
        },
    ])
}

pub fn serialize_sse(events: &[Value]) -> String {
    let mut wire = String::new();
    for event in events {
        wire.push_str("data: ");
        wire.push_str(&canonical_json(event));
        wire.push_str("\n\n");
    }
    wire.push_str("data: [DONE]\n\n");
    wire
}

fn map_codex_event(event: &CodexCompatibilityEvent) -> Vec<AdapterInputEvent> {
    let method = event.method.as_str();
    match method {
        "thread/started" => vec![AdapterInputEvent::Start {
            thread_id: json_non_empty_string(event.params.get("thread_id"))
                .unwrap_or_else(|| "thread_unknown".to_string()),
        }],
        "turn/started" => vec![AdapterInputEvent::StartStep {
            thread_id: json_non_empty_string(event.params.get("thread_id"))
                .unwrap_or_else(|| "thread_unknown".to_string()),
            turn_id: json_non_empty_string(event.params.get("turn_id"))
                .unwrap_or_else(|| "turn_unknown".to_string()),
            model: json_non_empty_string(event.params.get("model"))
                .unwrap_or_else(|| "gpt-5.2-codex".to_string()),
        }],
        "item/started" => {
            let item_kind = json_non_empty_string(event.params.get("item_kind"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            if item_kind == "agent_message" {
                vec![AdapterInputEvent::TextStart {
                    id: json_non_empty_string(event.params.get("item_id"))
                        .unwrap_or_else(|| "item_unknown".to_string()),
                    turn_id: json_non_empty_string(event.params.get("turn_id"))
                        .unwrap_or_else(|| "turn_unknown".to_string()),
                }]
            } else if item_kind == "mcp_tool_call" {
                vec![AdapterInputEvent::ToolInput {
                    tool_call_id: json_non_empty_string(event.params.get("item_id"))
                        .unwrap_or_else(|| "tool_unknown".to_string()),
                    tool_name: json_non_empty_string(event.params.pointer("/item/tool_name"))
                        .or_else(|| json_non_empty_string(event.params.pointer("/item/toolName")))
                        .unwrap_or_else(|| "unknown_tool".to_string()),
                    input: event
                        .params
                        .pointer("/item/arguments")
                        .cloned()
                        .or_else(|| event.params.pointer("/item/input").cloned())
                        .unwrap_or_else(|| json!({})),
                }]
            } else {
                Vec::new()
            }
        }
        "item/agentMessage/delta" => vec![AdapterInputEvent::TextDelta {
            id: json_non_empty_string(event.params.get("item_id"))
                .unwrap_or_else(|| "item_unknown".to_string()),
            delta: json_non_empty_string(event.params.get("delta")).unwrap_or_default(),
            channel: "assistant",
        }],
        "item/reasoning/summaryTextDelta" => vec![AdapterInputEvent::TextDelta {
            id: json_non_empty_string(event.params.get("item_id"))
                .unwrap_or_else(|| "item_unknown".to_string()),
            delta: json_non_empty_string(event.params.get("delta")).unwrap_or_default(),
            channel: "reasoning",
        }],
        "item/toolOutput/delta" => vec![AdapterInputEvent::ToolOutput {
            tool_call_id: json_non_empty_string(event.params.get("item_id"))
                .unwrap_or_else(|| "tool_unknown".to_string()),
            delta: json_non_empty_string(event.params.get("delta")),
            status: None,
        }],
        "item/completed" => {
            let item_kind = json_non_empty_string(event.params.get("item_kind"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            if item_kind == "mcp_tool_call" {
                vec![AdapterInputEvent::ToolOutput {
                    tool_call_id: json_non_empty_string(event.params.get("item_id"))
                        .unwrap_or_else(|| "tool_unknown".to_string()),
                    delta: None,
                    status: Some(
                        json_non_empty_string(event.params.get("item_status"))
                            .unwrap_or_else(|| "completed".to_string()),
                    ),
                }]
            } else {
                Vec::new()
            }
        }
        "turn/completed" => {
            let turn_id = json_non_empty_string(event.params.get("turn_id"))
                .unwrap_or_else(|| "turn_unknown".to_string());
            let status = json_non_empty_string(event.params.get("status"))
                .unwrap_or_else(|| "completed".to_string());
            vec![
                AdapterInputEvent::FinishStep {
                    turn_id,
                    status: status.clone(),
                },
                AdapterInputEvent::Finish { status },
            ]
        }
        "turn/failed" | "turn/aborted" | "turn/interrupted" => vec![
            AdapterInputEvent::Error {
                code: method.replace('/', "_"),
                message: json_non_empty_string(event.params.get("message"))
                    .unwrap_or_else(|| "turn failed".to_string()),
                retryable: event
                    .params
                    .get("will_retry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            AdapterInputEvent::Finish {
                status: "error".to_string(),
            },
        ],
        "codex/error" => vec![
            AdapterInputEvent::Error {
                code: "codex_error".to_string(),
                message: json_non_empty_string(event.params.get("message"))
                    .unwrap_or_else(|| "codex error".to_string()),
                retryable: event
                    .params
                    .get("will_retry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            AdapterInputEvent::Finish {
                status: "error".to_string(),
            },
        ],
        _ => Vec::new(),
    }
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(&sorted_value(value)).unwrap_or_else(|_| "{}".to_string())
}

fn sorted_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut ordered = BTreeMap::new();
            for (key, value) in map {
                ordered.insert(key.clone(), sorted_value(value));
            }
            let mut normalized = Map::new();
            for (key, value) in ordered {
                normalized.insert(key, value);
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted_value).collect()),
        _ => value.clone(),
    }
}

fn normalize_thread_id(raw: &str) -> Result<String, AdapterError> {
    non_empty(raw.to_string()).ok_or(AdapterError::MissingConversationId)
}

fn payload_thread_id(payload: &Value) -> Option<String> {
    json_non_empty_string(payload.get("thread_id"))
        .or_else(|| json_non_empty_string(payload.get("threadId")))
        .or_else(|| json_non_empty_string(payload.get("conversation_id")))
        .or_else(|| json_non_empty_string(payload.get("conversationId")))
        .or_else(|| json_non_empty_string(payload.get("id")))
}

fn payload_worker_id(payload: &Value) -> Option<String> {
    json_non_empty_string(payload.get("worker_id"))
        .or_else(|| json_non_empty_string(payload.get("workerId")))
}

fn payload_user_text(payload: &Value) -> Option<String> {
    if let Some(text) = json_non_empty_string(payload.get("text"))
        .or_else(|| json_non_empty_string(payload.get("message")))
    {
        return Some(text);
    }

    let messages = payload.get("messages")?.as_array()?;
    for message in messages.iter().rev() {
        let role = json_non_empty_string(message.get("role"))
            .unwrap_or_else(|| "user".to_string())
            .to_ascii_lowercase();
        if role != "user" {
            continue;
        }
        if let Some(text) = payload_message_text(message) {
            return Some(text);
        }
    }

    None
}

fn payload_message_text(message: &Value) -> Option<String> {
    if let Some(text) = json_non_empty_string(message.get("text"))
        .or_else(|| json_non_empty_string(message.get("message")))
    {
        return Some(text);
    }

    match message.get("content") {
        Some(Value::String(content)) => non_empty(content.to_string()),
        Some(Value::Object(content)) => json_non_empty_string(content.get("text")),
        Some(Value::Array(parts)) => parts.iter().find_map(payload_content_part_text),
        _ => None,
    }
}

fn payload_content_part_text(part: &Value) -> Option<String> {
    if let Some(text) = json_non_empty_string(part.get("text")) {
        return Some(text);
    }

    let part_type = json_non_empty_string(part.get("type"))?.to_ascii_lowercase();
    if part_type == "text" {
        return json_non_empty_string(part.get("value"))
            .or_else(|| json_non_empty_string(part.get("content")));
    }

    None
}

fn json_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .and_then(|text| non_empty(text.to_string()))
}

fn non_empty(value: String) -> Option<String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_enforces_single_start_and_terminal_order() {
        let stream = translate_events(&[
            AdapterInputEvent::Start {
                thread_id: "thread_1".to_string(),
            },
            AdapterInputEvent::StartStep {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                model: "gpt-5.2-codex".to_string(),
            },
            AdapterInputEvent::TextDelta {
                id: "item_1".to_string(),
                delta: "hello".to_string(),
                channel: "assistant",
            },
            AdapterInputEvent::FinishStep {
                turn_id: "turn_1".to_string(),
                status: "completed".to_string(),
            },
            AdapterInputEvent::Finish {
                status: "completed".to_string(),
            },
        ])
        .expect("stream should serialize");

        let event_types: Vec<&str> = stream
            .events
            .iter()
            .filter_map(|event| event.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(
            event_types,
            vec![
                "start",
                "start-step",
                "text-start",
                "text-delta",
                "finish-step",
                "finish"
            ]
        );
        assert!(stream.wire.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn adapter_rejects_tool_output_before_input() {
        let error = translate_events(&[
            AdapterInputEvent::Start {
                thread_id: "thread_1".to_string(),
            },
            AdapterInputEvent::StartStep {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                model: "gpt-5.2-codex".to_string(),
            },
            AdapterInputEvent::ToolOutput {
                tool_call_id: "tool_1".to_string(),
                delta: Some("{}".to_string()),
                status: None,
            },
            AdapterInputEvent::Finish {
                status: "error".to_string(),
            },
        ])
        .expect_err("tool output before tool input should fail");

        assert!(matches!(error, AdapterError::ToolOutputBeforeInput(_)));
    }

    #[test]
    fn adapter_allows_error_then_finish_without_finish_step() {
        let stream = translate_events(&[
            AdapterInputEvent::Start {
                thread_id: "thread_1".to_string(),
            },
            AdapterInputEvent::StartStep {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                model: "gpt-5.2-codex".to_string(),
            },
            AdapterInputEvent::Error {
                code: "codex_error".to_string(),
                message: "boom".to_string(),
                retryable: false,
            },
            AdapterInputEvent::Finish {
                status: "error".to_string(),
            },
        ])
        .expect("error stream should serialize");

        let event_types: Vec<&str> = stream
            .events
            .iter()
            .filter_map(|event| event.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(event_types, vec!["start", "start-step", "error", "finish"]);
    }

    #[test]
    fn codex_mapping_preserves_tool_then_finish_order() {
        let stream = translate_codex_events(&[
            CodexCompatibilityEvent {
                method: "thread/started".to_string(),
                params: json!({"thread_id":"thread_1"}),
            },
            CodexCompatibilityEvent {
                method: "turn/started".to_string(),
                params: json!({"thread_id":"thread_1","turn_id":"turn_1","model":"gpt-5.2-codex"}),
            },
            CodexCompatibilityEvent {
                method: "item/started".to_string(),
                params: json!({
                    "item_id":"tool_1",
                    "item_kind":"mcp_tool_call",
                    "item":{"tool_name":"web.search","arguments":{"q":"status"}}
                }),
            },
            CodexCompatibilityEvent {
                method: "item/toolOutput/delta".to_string(),
                params: json!({"item_id":"tool_1","delta":"{\"ok\":true}"}),
            },
            CodexCompatibilityEvent {
                method: "item/completed".to_string(),
                params: json!({"item_id":"tool_1","item_kind":"mcp_tool_call","item_status":"completed"}),
            },
            CodexCompatibilityEvent {
                method: "turn/completed".to_string(),
                params: json!({"turn_id":"turn_1","status":"completed"}),
            },
        ])
        .expect("mapped stream should serialize");

        let event_types: Vec<&str> = stream
            .events
            .iter()
            .filter_map(|event| event.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(
            event_types,
            vec![
                "start",
                "start-step",
                "tool-input",
                "tool-output",
                "tool-output",
                "finish-step",
                "finish"
            ]
        );
    }

    #[test]
    fn normalize_request_extracts_user_text_and_worker() {
        let normalized = normalize_legacy_stream_request(
            Some("thread_abc"),
            &json!({
                "workerId": "desktopw:shared",
                "messages": [{"role":"user","content":[{"type":"text","text":"hello"}]}]
            }),
        )
        .expect("payload should normalize");

        assert_eq!(normalized.thread_id, "thread_abc");
        assert_eq!(normalized.worker_id.as_deref(), Some("desktopw:shared"));
        assert_eq!(normalized.user_text, "hello");
    }
}
