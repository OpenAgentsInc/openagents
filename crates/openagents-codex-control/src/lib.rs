//! Shared Codex worker control primitives used by desktop/iOS hosts.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Map, Value, json};

pub const WORKER_EVENT_TYPE: &str = "worker.event";
pub const WORKER_REQUEST_EVENT_TYPE: &str = "worker.request";

pub const IOS_SOURCE: &str = "autopilot-ios";
pub const DESKTOP_SOURCE: &str = "autopilot-desktop";

pub const IOS_HANDSHAKE_METHOD: &str = "ios/handshake";
pub const IOS_USER_MESSAGE_METHOD: &str = "ios/user_message";
pub const DESKTOP_HANDSHAKE_ACK_METHOD: &str = "desktop/handshake_ack";

const EVENT_METHOD_RUNTIME_REQUEST: &str = "runtime/request";
const EVENT_METHOD_IOS_CONTROL_REQUEST: &str = "ios/control_request";
const EVENT_METHOD_WORKER_REQUEST: &str = "worker/request";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosUserMessage {
    pub message_id: String,
    pub text: String,
    pub model: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeKind {
    IosHandshake,
    DesktopHandshakeAck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandshakeEnvelope {
    kind: HandshakeKind,
    handshake_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlMethod {
    ThreadStart,
    ThreadResume,
    TurnStart,
    TurnInterrupt,
    ThreadList,
    ThreadRead,
}

impl ControlMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::TurnStart => "turn/start",
            Self::TurnInterrupt => "turn/interrupt",
            Self::ThreadList => "thread/list",
            Self::ThreadRead => "thread/read",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Self> {
        match raw.trim() {
            "thread/start" => Some(Self::ThreadStart),
            "thread/resume" => Some(Self::ThreadResume),
            "turn/start" => Some(Self::TurnStart),
            "turn/interrupt" => Some(Self::TurnInterrupt),
            "thread/list" => Some(Self::ThreadList),
            "thread/read" => Some(Self::ThreadRead),
            _ => None,
        }
    }

    pub fn requires_thread_target(self) -> bool {
        matches!(
            self,
            Self::ThreadResume | Self::TurnStart | Self::TurnInterrupt | Self::ThreadRead
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlRequestEnvelope {
    pub request_id: String,
    pub method: ControlMethod,
    pub params: Value,
    pub source: Option<String>,
    pub request_version: Option<String>,
    pub sent_at: Option<String>,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRequestParseError {
    pub code: &'static str,
    pub message: String,
    pub request_id: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerReceipt {
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetResolutionInput {
    pub requested_session_id: Option<String>,
    pub requested_thread_id: Option<String>,
    pub mapped_session_id: Option<String>,
    pub mapped_thread_id: Option<String>,
    pub shared_active_session_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetResolution {
    pub session_id: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetResolutionError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct RequestReplayState {
    seen_requests: HashSet<String>,
    terminal_receipts: HashSet<String>,
}

impl RequestReplayState {
    pub fn should_process_request(&mut self, worker_id: &str, request_id: &str) -> bool {
        self.seen_requests
            .insert(request_dedupe_key(worker_id, request_id))
    }

    pub fn mark_terminal_receipt(&mut self, worker_id: &str, request_id: &str) -> bool {
        self.terminal_receipts
            .insert(terminal_receipt_dedupe_key(worker_id, request_id))
    }

    pub fn has_terminal_receipt(&self, worker_id: &str, request_id: &str) -> bool {
        self.terminal_receipts
            .contains(&terminal_receipt_dedupe_key(worker_id, request_id))
    }
}

#[derive(Debug, Deserialize)]
struct WorkerEventEnvelope {
    #[serde(rename = "eventType", alias = "event_type")]
    event_type: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct WorkerHandshakePayload {
    source: String,
    method: String,
    #[serde(rename = "handshake_id", alias = "handshakeId")]
    handshake_id: String,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    desktop_session_id: Option<String>,
    #[serde(default)]
    occurred_at: Option<String>,
}

pub fn extract_ios_handshake_id(payload: &Value) -> Option<String> {
    let envelope = extract_handshake_envelope(payload)?;
    if envelope.kind == HandshakeKind::IosHandshake {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

pub fn extract_desktop_handshake_ack_id(payload: &Value) -> Option<String> {
    let envelope = extract_handshake_envelope(payload)?;
    if envelope.kind == HandshakeKind::DesktopHandshakeAck {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

pub fn extract_ios_user_message(payload: &Value) -> Option<IosUserMessage> {
    let envelope = serde_json::from_value::<WorkerEventEnvelope>(payload.clone()).ok()?;
    if envelope.event_type != WORKER_EVENT_TYPE {
        return None;
    }

    let worker_payload = envelope.payload.as_object()?;
    let source = non_empty(worker_payload.get("source")?.as_str()?)?;
    let method = non_empty(worker_payload.get("method")?.as_str()?)?;
    if source != IOS_SOURCE || method != IOS_USER_MESSAGE_METHOD {
        return None;
    }

    let params = worker_payload.get("params").and_then(Value::as_object);

    let message_id = first_non_empty_string(&[
        worker_payload.get("message_id"),
        worker_payload.get("messageId"),
        params.and_then(|value| value.get("message_id")),
        params.and_then(|value| value.get("messageId")),
    ])?;

    let text = first_non_empty_string(&[
        worker_payload.get("text"),
        worker_payload.get("message"),
        params.and_then(|value| value.get("text")),
        params.and_then(|value| value.get("message")),
    ])?;

    let model = first_non_empty_string(&[
        worker_payload.get("model"),
        params.and_then(|value| value.get("model")),
    ]);

    let reasoning = first_non_empty_string(&[
        worker_payload.get("reasoning"),
        worker_payload.get("reasoning_effort"),
        params.and_then(|value| value.get("reasoning")),
        params.and_then(|value| value.get("reasoning_effort")),
    ]);

    Some(IosUserMessage {
        message_id,
        text,
        model,
        reasoning,
    })
}

pub fn extract_control_request(
    payload: &Value,
) -> Result<Option<ControlRequestEnvelope>, ControlRequestParseError> {
    let envelope = match serde_json::from_value::<WorkerEventEnvelope>(payload.clone()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let request_value = match envelope.event_type.as_str() {
        WORKER_REQUEST_EVENT_TYPE => envelope
            .payload
            .get("request")
            .cloned()
            .unwrap_or(envelope.payload),
        WORKER_EVENT_TYPE => {
            let worker_payload = match envelope.payload.as_object() {
                Some(value) => value,
                None => return Ok(None),
            };
            let source = worker_payload
                .get("source")
                .and_then(Value::as_str)
                .and_then(non_empty);
            let method = worker_payload
                .get("method")
                .and_then(Value::as_str)
                .and_then(non_empty);

            let is_control_event = source == Some(IOS_SOURCE)
                && matches!(
                    method,
                    Some(EVENT_METHOD_RUNTIME_REQUEST)
                        | Some(EVENT_METHOD_IOS_CONTROL_REQUEST)
                        | Some(EVENT_METHOD_WORKER_REQUEST)
                );

            if !is_control_event {
                return Ok(None);
            }

            let params = worker_payload.get("params");
            match params {
                Some(Value::Object(obj)) => obj
                    .get("request")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(obj.clone())),
                Some(value) => value.clone(),
                None => return Ok(None),
            }
        }
        _ => return Ok(None),
    };

    parse_control_request_value(&request_value)
}

pub fn resolve_request_target(
    method: ControlMethod,
    input: TargetResolutionInput,
) -> Result<TargetResolution, TargetResolutionError> {
    let mut active_shared_sessions = input
        .shared_active_session_ids
        .into_iter()
        .filter_map(|value| normalized_owned(value))
        .collect::<Vec<_>>();
    active_shared_sessions.sort();
    active_shared_sessions.dedup();

    let session_id = input
        .requested_session_id
        .and_then(normalized_owned)
        .or_else(|| input.mapped_session_id.and_then(normalized_owned))
        .or_else(|| {
            if active_shared_sessions.len() == 1 {
                Some(active_shared_sessions[0].clone())
            } else {
                None
            }
        });

    let Some(session_id) = session_id else {
        if active_shared_sessions.len() > 1 {
            return Err(TargetResolutionError {
                code: "conflict",
                message: "multiple active shared sessions; explicit session_id required"
                    .to_string(),
            });
        }

        return Err(TargetResolutionError {
            code: "worker_unavailable",
            message: "desktop session mapping unavailable".to_string(),
        });
    };

    let thread_id = input
        .requested_thread_id
        .and_then(normalized_owned)
        .or_else(|| input.mapped_thread_id.and_then(normalized_owned));

    if method.requires_thread_target() && thread_id.is_none() {
        return Err(TargetResolutionError {
            code: "invalid_request",
            message: format!("{} requires thread_id", method.as_str()),
        });
    }

    Ok(TargetResolution {
        session_id,
        thread_id,
    })
}

pub fn success_receipt(
    request_id: &str,
    method: ControlMethod,
    response: Value,
    occurred_at: &str,
) -> WorkerReceipt {
    WorkerReceipt {
        event_type: "worker.response".to_string(),
        payload: json!({
            "request_id": request_id,
            "method": method.as_str(),
            "ok": true,
            "response": response,
            "occurred_at": occurred_at,
        }),
    }
}

pub fn error_receipt(
    request_id: &str,
    method: &str,
    code: &str,
    message: &str,
    retryable: bool,
    details: Option<Value>,
    occurred_at: &str,
) -> WorkerReceipt {
    let mut payload = Map::new();
    payload.insert(
        "request_id".to_string(),
        Value::String(request_id.to_string()),
    );
    payload.insert("method".to_string(), Value::String(method.to_string()));
    payload.insert("code".to_string(), Value::String(code.to_string()));
    payload.insert("message".to_string(), Value::String(message.to_string()));
    payload.insert("retryable".to_string(), Value::Bool(retryable));
    payload.insert(
        "occurred_at".to_string(),
        Value::String(occurred_at.to_string()),
    );
    if let Some(details) = details {
        payload.insert("details".to_string(), details);
    }

    WorkerReceipt {
        event_type: "worker.error".to_string(),
        payload: Value::Object(payload),
    }
}

pub fn handshake_dedupe_key(worker_id: &str, handshake_id: &str) -> String {
    format!("{worker_id}::{handshake_id}")
}

pub fn ios_message_dedupe_key(worker_id: &str, message_id: &str) -> String {
    format!("{worker_id}::{message_id}")
}

pub fn request_dedupe_key(worker_id: &str, request_id: &str) -> String {
    format!("{worker_id}::{request_id}")
}

pub fn terminal_receipt_dedupe_key(worker_id: &str, request_id: &str) -> String {
    format!("{worker_id}::terminal::{request_id}")
}

fn extract_handshake_envelope(payload: &Value) -> Option<HandshakeEnvelope> {
    let envelope = serde_json::from_value::<WorkerEventEnvelope>(payload.clone()).ok()?;
    if envelope.event_type != WORKER_EVENT_TYPE {
        return None;
    }

    let worker_payload = serde_json::from_value::<WorkerHandshakePayload>(envelope.payload).ok()?;
    let handshake_id = non_empty(worker_payload.handshake_id.as_str())?.to_string();

    match (
        worker_payload.source.as_str(),
        worker_payload.method.as_str(),
    ) {
        (IOS_SOURCE, IOS_HANDSHAKE_METHOD)
            if required_value(worker_payload.device_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(HandshakeEnvelope {
                kind: HandshakeKind::IosHandshake,
                handshake_id,
            })
        }
        (DESKTOP_SOURCE, DESKTOP_HANDSHAKE_ACK_METHOD)
            if required_value(worker_payload.desktop_session_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(HandshakeEnvelope {
                kind: HandshakeKind::DesktopHandshakeAck,
                handshake_id,
            })
        }
        _ => None,
    }
}

fn parse_control_request_value(
    raw: &Value,
) -> Result<Option<ControlRequestEnvelope>, ControlRequestParseError> {
    let request = match raw.as_object() {
        Some(value) => value,
        None => {
            return Err(ControlRequestParseError {
                code: "invalid_request",
                message: "control request payload must be an object".to_string(),
                request_id: None,
                method: None,
            });
        }
    };

    let request_id = first_non_empty_string(&[request.get("request_id"), request.get("requestId")]);
    let method_raw = first_non_empty_string(&[request.get("method")]);

    let Some(request_id) = request_id else {
        return Err(ControlRequestParseError {
            code: "invalid_request",
            message: "request_id is required".to_string(),
            request_id: None,
            method: method_raw,
        });
    };

    let Some(method_raw) = method_raw else {
        return Err(ControlRequestParseError {
            code: "invalid_request",
            message: "method is required".to_string(),
            request_id: Some(request_id),
            method: None,
        });
    };

    let Some(method) = ControlMethod::from_raw(&method_raw) else {
        return Err(ControlRequestParseError {
            code: "unsupported_method",
            message: format!("unsupported control method: {method_raw}"),
            request_id: Some(request_id),
            method: Some(method_raw),
        });
    };

    let params = request
        .get("params")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    let mut thread_id =
        first_non_empty_string(&[request.get("thread_id"), request.get("threadId")]);
    if thread_id.is_none() {
        thread_id = extract_thread_id_from_params(&params);
    }

    if method.requires_thread_target() && thread_id.is_none() {
        return Err(ControlRequestParseError {
            code: "invalid_request",
            message: format!("{} requires thread_id", method.as_str()),
            request_id: Some(request_id),
            method: Some(method_raw),
        });
    }

    let envelope = ControlRequestEnvelope {
        request_id,
        method,
        params,
        source: first_non_empty_string(&[request.get("source")]),
        request_version: first_non_empty_string(&[
            request.get("request_version"),
            request.get("requestVersion"),
        ]),
        sent_at: first_non_empty_string(&[request.get("sent_at"), request.get("sentAt")]),
        session_id: first_non_empty_string(&[request.get("session_id"), request.get("sessionId")]),
        thread_id,
    };

    Ok(Some(envelope))
}

fn extract_thread_id_from_params(params: &Value) -> Option<String> {
    let object = params.as_object()?;

    first_non_empty_string(&[
        object.get("thread_id"),
        object.get("threadId"),
        object
            .get("thread")
            .and_then(Value::as_object)
            .and_then(|thread| thread.get("id")),
        object
            .get("msg")
            .and_then(Value::as_object)
            .and_then(|msg| msg.get("thread_id")),
        object
            .get("msg")
            .and_then(Value::as_object)
            .and_then(|msg| msg.get("threadId")),
    ])
}

fn required_value(raw: Option<&str>) -> bool {
    non_empty(raw.unwrap_or_default()).is_some()
}

fn normalized_owned(raw: String) -> Option<String> {
    non_empty(raw.as_str()).map(ToString::to_string)
}

fn non_empty(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn first_non_empty_string(values: &[Option<&Value>]) -> Option<String> {
    values
        .iter()
        .filter_map(|value| value.and_then(Value::as_str))
        .filter_map(non_empty)
        .map(ToString::to_string)
        .next()
}

#[cfg(test)]
mod tests {
    use super::{
        ControlMethod, RequestReplayState, TargetResolutionInput, WORKER_EVENT_TYPE,
        WORKER_REQUEST_EVENT_TYPE, error_receipt, extract_control_request,
        extract_desktop_handshake_ack_id, extract_ios_handshake_id, extract_ios_user_message,
        handshake_dedupe_key, ios_message_dedupe_key, request_dedupe_key, resolve_request_target,
        success_receipt,
    };
    use serde_json::json;

    #[test]
    fn handshake_extractors_require_expected_fields() {
        let ios_payload = json!({
            "eventType": WORKER_EVENT_TYPE,
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-1",
                "device_id": "ios-device",
                "occurred_at": "2026-02-21T00:00:00Z"
            }
        });
        assert_eq!(
            extract_ios_handshake_id(&ios_payload).as_deref(),
            Some("hs-1")
        );

        let desktop_ack = json!({
            "eventType": WORKER_EVENT_TYPE,
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-2",
                "desktop_session_id": "session-1",
                "occurred_at": "2026-02-21T00:00:01Z"
            }
        });
        assert_eq!(
            extract_desktop_handshake_ack_id(&desktop_ack).as_deref(),
            Some("hs-2")
        );
    }

    #[test]
    fn extract_ios_user_message_reads_payload_and_params_variants() {
        let payload = json!({
            "eventType": WORKER_EVENT_TYPE,
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/user_message",
                "message_id": "iosmsg-1",
                "params": {
                    "text": "hello",
                    "model": "gpt-5.2-codex",
                    "reasoning": "low"
                }
            }
        });

        let parsed = extract_ios_user_message(&payload);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap_or_else(|| unreachable!());
        assert_eq!(parsed.message_id, "iosmsg-1");
        assert_eq!(parsed.text, "hello");
        assert_eq!(parsed.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(parsed.reasoning.as_deref(), Some("low"));
    }

    #[test]
    fn extract_control_request_parses_worker_request_envelope() {
        let payload = json!({
            "eventType": WORKER_REQUEST_EVENT_TYPE,
            "payload": {
                "request_id": "req-1",
                "method": "turn/start",
                "params": {
                    "thread_id": "thread-1",
                    "input": [{"type": "text", "text": "hi"}]
                },
                "source": "autopilot-ios",
                "request_version": "v1"
            }
        });

        let parsed = extract_control_request(&payload);
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap_or_else(|_| unreachable!());
        assert!(parsed.is_some());
        let parsed = parsed.unwrap_or_else(|| unreachable!());
        assert_eq!(parsed.request_id, "req-1");
        assert_eq!(parsed.method, ControlMethod::TurnStart);
        assert_eq!(parsed.thread_id.as_deref(), Some("thread-1"));
    }

    #[test]
    fn extract_control_request_parses_worker_event_wrapper() {
        let payload = json!({
            "eventType": WORKER_EVENT_TYPE,
            "payload": {
                "source": "autopilot-ios",
                "method": "runtime/request",
                "params": {
                    "request": {
                        "request_id": "req-2",
                        "method": "thread/list",
                        "params": {"limit": 10}
                    }
                }
            }
        });

        let parsed = extract_control_request(&payload);
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap_or_else(|_| unreachable!());
        assert!(parsed.is_some());
        let parsed = parsed.unwrap_or_else(|| unreachable!());
        assert_eq!(parsed.request_id, "req-2");
        assert_eq!(parsed.method, ControlMethod::ThreadList);
    }

    #[test]
    fn extract_control_request_rejects_invalid_or_unsupported_envelopes() {
        let missing_id = json!({
            "eventType": WORKER_REQUEST_EVENT_TYPE,
            "payload": {
                "method": "turn/start",
                "params": {"thread_id": "thread-1"}
            }
        });
        let invalid = extract_control_request(&missing_id);
        assert!(invalid.is_err());
        let invalid = invalid.err().unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code, "invalid_request");

        let unsupported = json!({
            "eventType": WORKER_REQUEST_EVENT_TYPE,
            "payload": {
                "request_id": "req-3",
                "method": "run/command",
                "params": {}
            }
        });
        let unsupported_result = extract_control_request(&unsupported);
        assert!(unsupported_result.is_err());
        let unsupported_result = unsupported_result.err().unwrap_or_else(|| unreachable!());
        assert_eq!(unsupported_result.code, "unsupported_method");
    }

    #[test]
    fn resolve_request_target_applies_shared_worker_fallback_and_validation() {
        let resolved = resolve_request_target(
            ControlMethod::ThreadStart,
            TargetResolutionInput {
                requested_session_id: None,
                requested_thread_id: None,
                mapped_session_id: None,
                mapped_thread_id: None,
                shared_active_session_ids: vec!["session-1".to_string()],
            },
        );
        assert!(resolved.is_ok());
        let resolved = resolved.unwrap_or_else(|_| unreachable!());
        assert_eq!(resolved.session_id, "session-1");

        let ambiguous = resolve_request_target(
            ControlMethod::ThreadList,
            TargetResolutionInput {
                requested_session_id: None,
                requested_thread_id: None,
                mapped_session_id: None,
                mapped_thread_id: None,
                shared_active_session_ids: vec!["s1".to_string(), "s2".to_string()],
            },
        );
        assert!(ambiguous.is_err());
        let ambiguous = ambiguous.err().unwrap_or_else(|| unreachable!());
        assert_eq!(ambiguous.code, "conflict");

        let missing_thread = resolve_request_target(
            ControlMethod::TurnInterrupt,
            TargetResolutionInput {
                requested_session_id: Some("session-1".to_string()),
                requested_thread_id: None,
                mapped_session_id: None,
                mapped_thread_id: None,
                shared_active_session_ids: vec![],
            },
        );
        assert!(missing_thread.is_err());
        let missing_thread = missing_thread.err().unwrap_or_else(|| unreachable!());
        assert_eq!(missing_thread.code, "invalid_request");
    }

    #[test]
    fn receipt_helpers_emit_contract_shape() {
        let success = success_receipt(
            "req-1",
            ControlMethod::TurnStart,
            json!({"turn": {"id": "turn-1"}}),
            "2026-02-21T00:00:00Z",
        );
        assert_eq!(success.event_type, "worker.response");
        assert_eq!(success.payload["request_id"], json!("req-1"));
        assert_eq!(success.payload["method"], json!("turn/start"));
        assert_eq!(success.payload["ok"], json!(true));

        let error = error_receipt(
            "req-1",
            "turn/start",
            "invalid_request",
            "thread_id is required",
            false,
            Some(json!({"field": "thread_id"})),
            "2026-02-21T00:00:01Z",
        );
        assert_eq!(error.event_type, "worker.error");
        assert_eq!(error.payload["code"], json!("invalid_request"));
        assert_eq!(error.payload["retryable"], json!(false));
    }

    #[test]
    fn replay_state_dedupes_requests_and_terminal_receipts() {
        let mut state = RequestReplayState::default();
        assert!(state.should_process_request("desktopw:shared", "req-1"));
        assert!(!state.should_process_request("desktopw:shared", "req-1"));

        assert!(state.mark_terminal_receipt("desktopw:shared", "req-1"));
        assert!(!state.mark_terminal_receipt("desktopw:shared", "req-1"));
        assert!(state.has_terminal_receipt("desktopw:shared", "req-1"));
    }

    #[test]
    fn dedupe_keys_are_stable() {
        assert_eq!(handshake_dedupe_key("w", "h"), "w::h");
        assert_eq!(ios_message_dedupe_key("w", "m"), "w::m");
        assert_eq!(request_dedupe_key("w", "r"), "w::r");
    }
}
