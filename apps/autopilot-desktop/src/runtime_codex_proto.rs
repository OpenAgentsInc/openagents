use serde::Deserialize;
use serde_json::Value;

const WORKER_EVENT_TYPE: &str = "worker.event";
const IOS_HANDSHAKE_SOURCE: &str = "autopilot-ios";
const IOS_HANDSHAKE_METHOD: &str = "ios/handshake";
const IOS_USER_MESSAGE_METHOD: &str = "ios/user_message";
const DESKTOP_ACK_SOURCE: &str = "autopilot-desktop";
const DESKTOP_ACK_METHOD: &str = "desktop/handshake_ack";

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexStreamEvent {
    pub id: Option<u64>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosUserMessage {
    pub message_id: String,
    pub text: String,
    pub model: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtoHandshakeKind {
    IosHandshake,
    DesktopHandshakeAck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProtoHandshakeEnvelope {
    kind: ProtoHandshakeKind,
    handshake_id: String,
}

#[derive(Debug, Deserialize)]
struct ProtoWorkerEventEnvelope {
    #[serde(rename = "eventType", alias = "event_type")]
    event_type: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct ProtoWorkerHandshakePayload {
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

pub fn parse_runtime_stream_events(raw: &str) -> Vec<RuntimeCodexStreamEvent> {
    let normalized = raw.replace("\r\n", "\n");
    let mut events = Vec::new();
    let mut id: Option<u64> = None;
    let mut data_lines: Vec<String> = Vec::new();
    let mut saw_event_fields = false;

    for line in normalized.split('\n') {
        if line.is_empty() {
            flush_stream_event(&mut events, &mut id, &mut data_lines);
            saw_event_fields = false;
            continue;
        }

        if let Some(value) = line.strip_prefix("id:") {
            // Some upstream streams omit blank separators; treat a new `id:` as a frame boundary.
            if !data_lines.is_empty() {
                flush_stream_event(&mut events, &mut id, &mut data_lines);
            }
            id = value.trim().parse::<u64>().ok();
            saw_event_fields = true;
            continue;
        }

        if line.strip_prefix("event:").is_some() {
            saw_event_fields = true;
            continue;
        }

        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim().to_string());
            saw_event_fields = true;
            continue;
        }

        // If a folded payload line arrives without `data:`, append it to the current data block.
        if saw_event_fields {
            if let Some(last) = data_lines.last_mut() {
                last.push('\n');
                last.push_str(line);
            } else {
                data_lines.push(line.to_string());
            }
        }
    }

    flush_stream_event(&mut events, &mut id, &mut data_lines);
    events
}

fn flush_stream_event(
    events: &mut Vec<RuntimeCodexStreamEvent>,
    id: &mut Option<u64>,
    data_lines: &mut Vec<String>,
) {
    if data_lines.is_empty() {
        *id = None;
        return;
    }

    let raw_data = data_lines.join("\n");
    let payload = serde_json::from_str::<Value>(&raw_data).unwrap_or(Value::String(raw_data));
    events.push(RuntimeCodexStreamEvent { id: *id, payload });

    *id = None;
    data_lines.clear();
}

pub fn stream_event_seq(event: &RuntimeCodexStreamEvent) -> Option<u64> {
    event
        .id
        .or_else(|| event.payload.get("seq").and_then(|value| value.as_u64()))
}

pub fn extract_ios_handshake_id(payload: &Value) -> Option<String> {
    let envelope = extract_proto_handshake_envelope(payload)?;
    if envelope.kind == ProtoHandshakeKind::IosHandshake {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

pub fn extract_desktop_handshake_ack_id(payload: &Value) -> Option<String> {
    let envelope = extract_proto_handshake_envelope(payload)?;
    if envelope.kind == ProtoHandshakeKind::DesktopHandshakeAck {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

pub fn extract_ios_user_message(payload: &Value) -> Option<IosUserMessage> {
    let envelope = serde_json::from_value::<ProtoWorkerEventEnvelope>(payload.clone()).ok()?;
    if envelope.event_type != WORKER_EVENT_TYPE {
        return None;
    }

    let worker_payload = envelope.payload.as_object()?;
    let source = non_empty(worker_payload.get("source")?.as_str()?)?;
    let method = non_empty(worker_payload.get("method")?.as_str()?)?;
    if source != IOS_HANDSHAKE_SOURCE || method != IOS_USER_MESSAGE_METHOD {
        return None;
    }

    let params = worker_payload.get("params").and_then(|value| value.as_object());

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

pub fn handshake_dedupe_key(worker_id: &str, handshake_id: &str) -> String {
    format!("{worker_id}::{handshake_id}")
}

pub fn merge_retry_cursor(current: Option<u64>, failed_seq: u64) -> u64 {
    let replay_cursor = failed_seq.saturating_sub(1);
    current
        .map(|cursor| cursor.min(replay_cursor))
        .unwrap_or(replay_cursor)
}

fn extract_proto_handshake_envelope(payload: &Value) -> Option<ProtoHandshakeEnvelope> {
    let envelope = serde_json::from_value::<ProtoWorkerEventEnvelope>(payload.clone()).ok()?;
    if envelope.event_type != WORKER_EVENT_TYPE {
        return None;
    }

    let worker_payload =
        serde_json::from_value::<ProtoWorkerHandshakePayload>(envelope.payload).ok()?;
    let handshake_id = non_empty(worker_payload.handshake_id.as_str())?.to_string();

    match (
        worker_payload.source.as_str(),
        worker_payload.method.as_str(),
    ) {
        (IOS_HANDSHAKE_SOURCE, IOS_HANDSHAKE_METHOD)
            if required_value(worker_payload.device_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(ProtoHandshakeEnvelope {
                kind: ProtoHandshakeKind::IosHandshake,
                handshake_id,
            })
        }
        (DESKTOP_ACK_SOURCE, DESKTOP_ACK_METHOD)
            if required_value(worker_payload.desktop_session_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(ProtoHandshakeEnvelope {
                kind: ProtoHandshakeKind::DesktopHandshakeAck,
                handshake_id,
            })
        }
        _ => None,
    }
}

fn required_value(raw: Option<&str>) -> bool {
    non_empty(raw.unwrap_or_default()).is_some()
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
        .filter_map(|value| value.and_then(|raw| raw.as_str()))
        .filter_map(non_empty)
        .map(|value| value.to_string())
        .next()
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCodexStreamEvent, extract_desktop_handshake_ack_id, extract_ios_handshake_id,
        extract_ios_user_message, handshake_dedupe_key, merge_retry_cursor,
        parse_runtime_stream_events, stream_event_seq,
    };
    use serde_json::{Value, json};
    use std::collections::HashSet;

    #[test]
    fn parse_runtime_stream_events_parses_sse_frames() {
        let raw = "event: codex.worker.event\r\nid: 41\r\ndata: {\"seq\":41,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"ios/handshake\"}}\r\n\r\n\
event: codex.worker.event\r\nid: 42\r\ndata: {\"seq\":42,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"desktop/handshake_ack\"}}\r\n\r\n";

        let events = parse_runtime_stream_events(raw);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, Some(41));
        assert_eq!(events[1].id, Some(42));
    }

    #[test]
    fn parse_runtime_stream_events_tolerates_missing_blank_separators() {
        let raw = "event: codex.worker.event\nid: 41\ndata: {\"seq\":41,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"ios/handshake\"}}\n\
event: codex.worker.event\nid: 42\ndata: {\"seq\":42,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"desktop/handshake_ack\"}}\n";

        let events = parse_runtime_stream_events(raw);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, Some(41));
        assert_eq!(events[1].id, Some(42));
    }

    #[test]
    fn extract_ios_handshake_id_requires_proto_fields() {
        let payload = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-123",
                "device_id": "ios-device",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        assert_eq!(
            extract_ios_handshake_id(&payload).as_deref(),
            Some("hs-123")
        );

        let invalid = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-123",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        assert_eq!(extract_ios_handshake_id(&invalid), None);
    }

    #[test]
    fn extract_desktop_handshake_ack_id_requires_proto_fields() {
        let payload = json!({
            "event_type": "worker.event",
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-123",
                "desktop_session_id": "session-42",
                "occurred_at": "2026-02-20T00:00:02Z"
            }
        });

        assert_eq!(
            extract_desktop_handshake_ack_id(&payload).as_deref(),
            Some("hs-123")
        );

        let invalid = json!({
            "event_type": "worker.event",
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-123",
                "occurred_at": "2026-02-20T00:00:02Z"
            }
        });

        assert_eq!(extract_desktop_handshake_ack_id(&invalid), None);
    }

    #[test]
    fn merge_retry_cursor_prefers_oldest_replay_position() {
        assert_eq!(merge_retry_cursor(None, 50), 49);
        assert_eq!(merge_retry_cursor(Some(32), 50), 32);
        assert_eq!(merge_retry_cursor(Some(49), 12), 11);
        assert_eq!(merge_retry_cursor(Some(0), 0), 0);
    }

    #[test]
    fn handshake_retry_harness_replays_until_ack_is_observed_once() {
        let worker_id = "desktopw:test";
        let handshake_payload = json!({
            "seq": 200,
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-200",
                "device_id": "ios-device",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });
        let ack_payload = json!({
            "seq": 201,
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-200",
                "desktop_session_id": "session-42",
                "occurred_at": "2026-02-20T00:00:02Z"
            }
        });

        let mut acked = HashSet::<String>::new();

        // First processing pass emits ack but fails before persistence.
        let first_emit = process_handshake_event(worker_id, &handshake_payload, &mut acked, false);
        assert!(first_emit);
        assert!(acked.is_empty());

        let rewind_cursor = merge_retry_cursor(None, 200);
        assert_eq!(rewind_cursor, 199);

        // Replay pass emits ack again and persists dedupe marker.
        let replay_emit = process_handshake_event(worker_id, &handshake_payload, &mut acked, true);
        assert!(replay_emit);
        assert_eq!(acked.len(), 1);

        // When stream catches up with the emitted ack event, it is treated as observed and not re-emitted.
        let ack_emit = process_handshake_event(worker_id, &ack_payload, &mut acked, true);
        assert!(!ack_emit);
        assert_eq!(acked.len(), 1);

        // Any further replay of the same iOS handshake must not emit again.
        let duplicate_emit =
            process_handshake_event(worker_id, &handshake_payload, &mut acked, true);
        assert!(!duplicate_emit);
        assert_eq!(acked.len(), 1);
    }

    fn process_handshake_event(
        worker_id: &str,
        payload: &Value,
        acked: &mut HashSet<String>,
        emit_succeeds: bool,
    ) -> bool {
        if let Some(handshake_id) = extract_desktop_handshake_ack_id(payload) {
            acked.insert(handshake_dedupe_key(worker_id, &handshake_id));
            return false;
        }

        let Some(handshake_id) = extract_ios_handshake_id(payload) else {
            return false;
        };

        let dedupe_key = handshake_dedupe_key(worker_id, &handshake_id);
        if acked.contains(&dedupe_key) {
            return false;
        }

        if emit_succeeds {
            acked.insert(dedupe_key);
        }

        true
    }

    #[test]
    fn stream_event_seq_falls_back_to_payload_seq() {
        let event = RuntimeCodexStreamEvent {
            id: None,
            payload: json!({"seq": 77}),
        };
        assert_eq!(stream_event_seq(&event), Some(77));
    }

    #[test]
    fn extract_ios_user_message_parses_worker_event_payload() {
        let payload = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/user_message",
                "message_id": "iosmsg-1",
                "params": {
                    "text": "hi from ios",
                    "model": "gpt-5.2-codex",
                    "reasoning": "low"
                }
            }
        });

        let message = extract_ios_user_message(&payload).expect("expected ios message");
        assert_eq!(message.message_id, "iosmsg-1");
        assert_eq!(message.text, "hi from ios");
        assert_eq!(message.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(message.reasoning.as_deref(), Some("low"));
    }

    #[test]
    fn extract_ios_user_message_requires_non_empty_message_id_and_text() {
        let missing_text = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/user_message",
                "message_id": "iosmsg-2",
                "params": {}
            }
        });
        assert!(extract_ios_user_message(&missing_text).is_none());

        let missing_id = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/user_message",
                "params": {
                    "text": "hello"
                }
            }
        });
        assert!(extract_ios_user_message(&missing_id).is_none());
    }
}
