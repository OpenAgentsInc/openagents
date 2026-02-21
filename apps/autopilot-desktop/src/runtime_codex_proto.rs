use serde::Deserialize;
use serde_json::Value;

pub use openagents_client_core::codex_worker::{
    extract_desktop_handshake_ack_id, extract_ios_handshake_id,
};
pub use openagents_client_core::khala_protocol::{
    RuntimeStreamEvent as RuntimeCodexStreamEvent, build_phoenix_frame as build_khala_frame,
    extract_runtime_stream_events as extract_runtime_events_from_khala_update, merge_retry_cursor,
    parse_phoenix_frame as parse_khala_frame, runtime_stream_event_seq as stream_event_seq,
    sync_error_code as khala_error_code,
};

const WORKER_EVENT_TYPE: &str = "worker.event";
const IOS_HANDSHAKE_SOURCE: &str = "autopilot-ios";
const IOS_USER_MESSAGE_METHOD: &str = "ios/user_message";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosUserMessage {
    pub message_id: String,
    pub text: String,
    pub model: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProtoWorkerEventEnvelope {
    #[serde(rename = "eventType", alias = "event_type")]
    event_type: String,
    payload: Value,
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

    let params = worker_payload
        .get("params")
        .and_then(|value| value.as_object());

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
        ProtoWorkerEventEnvelope, RuntimeCodexStreamEvent, WORKER_EVENT_TYPE, build_khala_frame,
        extract_desktop_handshake_ack_id, extract_ios_handshake_id, extract_ios_user_message,
        extract_runtime_events_from_khala_update, handshake_dedupe_key, khala_error_code,
        merge_retry_cursor, parse_khala_frame, stream_event_seq,
    };
    use serde_json::{Value, json};
    use std::collections::HashSet;

    #[test]
    fn parse_khala_frame_roundtrips_phoenix_frame_shape() {
        let raw = build_khala_frame(
            None,
            Some("42"),
            "sync:v1",
            "sync:update_batch",
            json!({"updates": []}),
        );
        let frame = parse_khala_frame(&raw).expect("frame should parse");
        assert_eq!(frame.reference.as_deref(), Some("42"));
        assert_eq!(frame.topic, "sync:v1");
        assert_eq!(frame.event, "sync:update_batch");
    }

    #[test]
    fn extract_runtime_events_from_khala_update_filters_topic_and_worker() {
        let payload = json!({
            "updates": [
                {
                    "topic": "runtime.codex_worker_events",
                    "watermark": 12,
                    "payload": {
                        "workerId": "desktopw:shared",
                        "seq": 12,
                        "eventType": "worker.event",
                        "payload": {
                            "method": "ios/handshake",
                            "handshake_id": "hs-12",
                            "source": "autopilot-ios",
                            "device_id": "ios-device",
                            "occurred_at": "2026-02-20T00:00:00Z"
                        }
                    }
                },
                {
                    "topic": "runtime.codex_worker_events",
                    "watermark": 13,
                    "payload": {
                        "workerId": "desktopw:other",
                        "seq": 13
                    }
                }
            ]
        });

        let events = extract_runtime_events_from_khala_update(
            &payload,
            "runtime.codex_worker_events",
            "desktopw:shared",
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some(12));
        assert_eq!(
            events[0]
                .payload
                .get("eventType")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "worker.event"
        );
    }

    #[test]
    fn khala_error_code_extracts_sync_error_code() {
        assert_eq!(
            khala_error_code(&json!({"code": "stale_cursor"})).as_deref(),
            Some("stale_cursor")
        );
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

    #[test]
    fn codex_contract_fixture_is_decodable_by_desktop_parser() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../docs/protocol/fixtures/codex-worker-events-v1.json"
        ))
        .expect("codex fixture should be valid JSON");

        let notifications = fixture["notification_events"]
            .as_array()
            .expect("notification_events must be an array");

        assert!(notifications.len() >= 10);

        let mut saw_ios_handshake = false;
        let mut saw_desktop_ack = false;
        let mut saw_ios_user_message = false;

        for notification in notifications {
            let seq = notification["seq"]
                .as_u64()
                .expect("notification seq must be numeric");
            let payload = notification["payload"].clone();

            let stream_payload = json!({
                "seq": seq,
                "eventType": WORKER_EVENT_TYPE,
                "payload": payload
            });

            let parsed = serde_json::from_value::<ProtoWorkerEventEnvelope>(stream_payload.clone())
                .expect("runtime stream payload should deserialize");
            assert_eq!(parsed.event_type, WORKER_EVENT_TYPE);

            if extract_ios_handshake_id(&stream_payload).is_some() {
                saw_ios_handshake = true;
            }
            if extract_desktop_handshake_ack_id(&stream_payload).is_some() {
                saw_desktop_ack = true;
            }
            if extract_ios_user_message(&stream_payload).is_some() {
                saw_ios_user_message = true;
            }
        }

        assert!(saw_ios_handshake);
        assert!(saw_desktop_ack);
        assert!(saw_ios_user_message);
    }
}
