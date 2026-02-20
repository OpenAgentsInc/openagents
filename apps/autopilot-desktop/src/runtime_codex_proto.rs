use serde::Deserialize;
use serde_json::Value;

const WORKER_EVENT_TYPE: &str = "worker.event";
const IOS_HANDSHAKE_SOURCE: &str = "autopilot-ios";
const IOS_HANDSHAKE_METHOD: &str = "ios/handshake";
const DESKTOP_ACK_SOURCE: &str = "autopilot-desktop";
const DESKTOP_ACK_METHOD: &str = "desktop/handshake_ack";

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexStreamEvent {
    pub id: Option<u64>,
    pub payload: Value,
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

    for chunk in normalized.split("\n\n") {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        let mut id: Option<u64> = None;
        let mut data_lines: Vec<String> = Vec::new();

        for line in chunk.lines() {
            if let Some(value) = line.strip_prefix("id:") {
                id = value.trim().parse::<u64>().ok();
            } else if let Some(value) = line.strip_prefix("data:") {
                data_lines.push(value.trim().to_string());
            }
        }

        if data_lines.is_empty() {
            continue;
        }

        let raw_data = data_lines.join("\n");
        let payload = serde_json::from_str::<Value>(&raw_data).unwrap_or(Value::String(raw_data));
        events.push(RuntimeCodexStreamEvent { id, payload });
    }

    events
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

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCodexStreamEvent, extract_desktop_handshake_ack_id, extract_ios_handshake_id,
        handshake_dedupe_key, merge_retry_cursor, parse_runtime_stream_events, stream_event_seq,
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
}
