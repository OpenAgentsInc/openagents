use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum SpacetimeServerMessage {
    SubscribeApplied { payload: Value },
    TransactionUpdate { payload: Value },
    Error { payload: Value },
    Heartbeat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexStreamEvent {
    pub id: Option<u64>,
    pub payload: Value,
}

#[must_use]
pub fn build_subscribe_request(stream_id: &str, after_seq: u64, request_id: u64) -> String {
    let query = format!(
        "SELECT * FROM sync_event WHERE stream_id = '{}' AND seq > {} ORDER BY seq ASC",
        stream_id.replace('"', "\\\""),
        after_seq
    );

    let payload = Value::Object(
        [(
            "SubscribeMulti".to_string(),
            Value::Object(
                [
                    (
                        "query_strings".to_string(),
                        Value::Array(vec![Value::String(query)]),
                    ),
                    (
                        "request_id".to_string(),
                        Value::Number(serde_json::Number::from(request_id)),
                    ),
                    (
                        "query_id".to_string(),
                        Value::Number(serde_json::Number::from(request_id)),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    );

    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

#[must_use]
pub fn build_heartbeat_request(request_id: u64) -> String {
    let payload = Value::Object(
        [(
            "OneOffQuery".to_string(),
            Value::Object(
                [
                    (
                        "message_id".to_string(),
                        Value::Array(vec![Value::Number(serde_json::Number::from(request_id))]),
                    ),
                    (
                        "query_string".to_string(),
                        Value::String("SELECT 1".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    );

    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

pub fn parse_spacetime_server_message(raw: &str) -> Option<SpacetimeServerMessage> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let object = parsed.as_object()?;

    if object.get("SubscribeApplied").is_some()
        || object.get("SubscribeMultiApplied").is_some()
        || object
            .get("kind")
            .and_then(Value::as_str)
            .map(|value| value.eq_ignore_ascii_case("subscribe_applied"))
            .unwrap_or(false)
    {
        return Some(SpacetimeServerMessage::SubscribeApplied {
            payload: parsed.clone(),
        });
    }

    if object.get("TransactionUpdate").is_some()
        || object.get("TransactionUpdateLight").is_some()
        || object
            .get("kind")
            .and_then(Value::as_str)
            .map(|value| value.eq_ignore_ascii_case("transaction_update"))
            .unwrap_or(false)
    {
        return Some(SpacetimeServerMessage::TransactionUpdate {
            payload: parsed.clone(),
        });
    }

    if object.get("SubscriptionError").is_some()
        || object.get("Error").is_some()
        || object
            .get("kind")
            .and_then(Value::as_str)
            .map(|value| value.eq_ignore_ascii_case("error"))
            .unwrap_or(false)
    {
        return Some(SpacetimeServerMessage::Error {
            payload: parsed.clone(),
        });
    }

    if object
        .get("kind")
        .and_then(Value::as_str)
        .map(|value| value.eq_ignore_ascii_case("heartbeat"))
        .unwrap_or(false)
    {
        return Some(SpacetimeServerMessage::Heartbeat);
    }

    None
}

pub fn extract_runtime_events_from_spacetime_update(
    payload: &Value,
    expected_stream_id: &str,
    worker_id: &str,
) -> Vec<RuntimeCodexStreamEvent> {
    let mut updates = payload
        .as_object()
        .and_then(|object| object.get("updates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if updates.is_empty() {
        updates = payload
            .pointer("/TransactionUpdate/status/Committed/tables")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
    }

    updates
        .into_iter()
        .filter_map(|update| {
            let stream_id = update
                .get("stream_id")
                .and_then(Value::as_str)
                .or_else(|| update.get("topic").and_then(Value::as_str));
            if stream_id != Some(expected_stream_id) {
                return None;
            }

            let stream_payload = update
                .get("payload")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            let event_worker_id = stream_payload
                .get("workerId")
                .and_then(Value::as_str)
                .or_else(|| stream_payload.get("worker_id").and_then(Value::as_str));
            if event_worker_id != Some(worker_id) {
                return None;
            }

            let payload = Value::Object(stream_payload);
            let seq = payload
                .get("seq")
                .and_then(Value::as_u64)
                .or_else(|| update.get("seq").and_then(Value::as_u64))
                .or_else(|| update.get("watermark").and_then(Value::as_u64));

            Some(RuntimeCodexStreamEvent { id: seq, payload })
        })
        .collect()
}

pub fn stream_event_seq(event: &RuntimeCodexStreamEvent) -> Option<u64> {
    event
        .id
        .or_else(|| event.payload.get("seq").and_then(Value::as_u64))
}

#[must_use]
pub fn merge_retry_cursor(current: Option<u64>, failed_seq: u64) -> u64 {
    let replay_cursor = failed_seq.saturating_sub(1);
    current
        .map(|cursor| cursor.min(replay_cursor))
        .unwrap_or(replay_cursor)
}

pub fn spacetime_error_code(payload: &Value) -> Option<String> {
    payload
        .pointer("/SubscriptionError/error")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .get("code")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}

pub use openagents_codex_control::{
    ControlMethod, error_receipt as build_error_receipt, extract_control_request,
    extract_desktop_handshake_ack_id, extract_ios_handshake_id, extract_ios_user_message,
    handshake_dedupe_key, request_dedupe_key, success_receipt as build_success_receipt,
    terminal_receipt_dedupe_key,
};

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCodexStreamEvent, SpacetimeServerMessage, build_subscribe_request,
        extract_control_request, extract_desktop_handshake_ack_id, extract_ios_handshake_id,
        extract_ios_user_message, extract_runtime_events_from_spacetime_update,
        handshake_dedupe_key, merge_retry_cursor, parse_spacetime_server_message,
        request_dedupe_key, spacetime_error_code, stream_event_seq,
        terminal_receipt_dedupe_key,
    };
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::collections::HashSet;

    const WORKER_EVENT_TYPE: &str = "worker.event";

    #[derive(Debug, Deserialize)]
    struct ProtoWorkerEventEnvelope {
        #[serde(rename = "eventType", alias = "event_type")]
        event_type: String,
        payload: Value,
    }

    #[test]
    fn build_subscribe_request_emits_stream_query_payload() {
        let raw = build_subscribe_request("runtime.codex_worker_events", 42, 7);
        let parsed: Value = serde_json::from_str(raw.as_str()).expect("json should parse");
        let query = parsed
            .pointer("/SubscribeMulti/query_strings/0")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(query.contains("sync_event"));
        assert!(query.contains("runtime.codex_worker_events"));
        assert!(query.contains("seq > 42"));
    }

    #[test]
    fn parse_spacetime_server_message_uses_subscribe_transaction_error_kinds() {
        let subscribe =
            parse_spacetime_server_message(r#"{"SubscribeApplied":{"request_id":1,"query_id":1}}"#);
        assert!(matches!(
            subscribe,
            Some(SpacetimeServerMessage::SubscribeApplied { .. })
        ));

        let update = parse_spacetime_server_message(
            r#"{"TransactionUpdate":{"status":{"Committed":{"tables":[]}}}}"#,
        );
        assert!(matches!(
            update,
            Some(SpacetimeServerMessage::TransactionUpdate { .. })
        ));

        let error =
            parse_spacetime_server_message(r#"{"SubscriptionError":{"error":"stale_cursor"}}"#);
        assert!(matches!(error, Some(SpacetimeServerMessage::Error { .. })));
    }

    #[test]
    fn parse_spacetime_server_message_rejects_legacy_phoenix_frames() {
        let legacy = parse_spacetime_server_message(
            r#"[null,"1","sync:v1","sync:update_batch",{"updates":[]}]"#,
        );
        assert!(legacy.is_none());
    }

    #[test]
    fn extract_runtime_events_from_spacetime_update_filters_stream_and_worker() {
        let payload = json!({
            "updates": [
                {
                    "stream_id": "runtime.codex_worker_events",
                    "seq": 12,
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
                    "stream_id": "runtime.codex_worker_events",
                    "seq": 13,
                    "payload": {
                        "workerId": "desktopw:other",
                        "seq": 13
                    }
                }
            ]
        });

        let events = extract_runtime_events_from_spacetime_update(
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
    fn spacetime_error_code_extracts_sync_error_code() {
        assert_eq!(
            spacetime_error_code(&json!({"SubscriptionError": {"error": "stale_cursor"}}))
                .as_deref(),
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

        let first_emit = process_handshake_event(worker_id, &handshake_payload, &mut acked, false);
        assert!(first_emit);
        assert!(acked.is_empty());

        let rewind_cursor = merge_retry_cursor(None, 200);
        assert_eq!(rewind_cursor, 199);

        let duplicate_emit =
            process_handshake_event(worker_id, &handshake_payload, &mut acked, false);
        assert!(duplicate_emit);

        let ack_emit = process_handshake_event(worker_id, &ack_payload, &mut acked, true);
        assert!(ack_emit);

        let after_ack_emit =
            process_handshake_event(worker_id, &handshake_payload, &mut acked, false);
        assert!(!after_ack_emit);
    }

    #[test]
    fn control_request_dedupe_key_is_worker_and_request_scoped() {
        assert_eq!(
            request_dedupe_key("desktopw:shared", "req-1"),
            "desktopw:shared::req-1"
        );
        assert_eq!(
            request_dedupe_key("desktopw:shared", " req-1 "),
            "desktopw:shared:: req-1 "
        );
    }

    #[test]
    fn terminal_receipt_dedupe_key_is_worker_and_request_scoped() {
        assert_eq!(
            terminal_receipt_dedupe_key("desktopw:shared", "req-1"),
            "desktopw:shared::terminal::req-1"
        );
        assert_eq!(
            terminal_receipt_dedupe_key("desktopw:shared", " req-1 "),
            "desktopw:shared::terminal:: req-1 "
        );
    }

    #[test]
    fn stream_event_seq_prefers_explicit_id_then_payload_seq() {
        let event = RuntimeCodexStreamEvent {
            id: Some(11),
            payload: json!({"seq": 22}),
        };
        assert_eq!(stream_event_seq(&event), Some(11));

        let payload_only = RuntimeCodexStreamEvent {
            id: None,
            payload: json!({"seq": 33}),
        };
        assert_eq!(stream_event_seq(&payload_only), Some(33));
    }

    #[test]
    fn handshake_and_control_extractors_reject_non_worker_events() {
        let invalid_event = json!({
            "eventType": "worker.started",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-x",
                "device_id": "ios-device",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });
        assert_eq!(extract_ios_handshake_id(&invalid_event), None);

        let control_like = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "runtime/request",
                "params": {
                    "request": {
                        "request_id": "req-1",
                        "method": "thread/list",
                        "params": {
                            "limit": 5
                        }
                    }
                }
            }
        });
        assert!(matches!(
            extract_control_request(&control_like),
            Ok(Some(_))
        ));
    }

    #[test]
    fn ios_user_message_extracts_text_and_optional_model_reasoning() {
        let payload = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/user_message",
                "message_id": "msg-1",
                "text": "hello",
                "model": "gpt-5.2-codex",
                "reasoning": "high",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        let extracted = extract_ios_user_message(&payload).expect("user message should parse");
        assert_eq!(extracted.message_id, "msg-1");
        assert_eq!(extracted.text, "hello");
        assert_eq!(extracted.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(extracted.reasoning.as_deref(), Some("high"));
    }

    #[test]
    fn handshake_dedupe_key_is_worker_and_handshake_scoped() {
        assert_eq!(
            handshake_dedupe_key("desktopw:shared", "hs-1"),
            "desktopw:shared::hs-1"
        );
        assert_eq!(
            handshake_dedupe_key("desktopw:shared", " hs-1 "),
            "desktopw:shared:: hs-1 "
        );
    }

    #[test]
    fn proto_worker_event_envelope_supports_event_type_aliases() {
        let camel_case = json!({
            "eventType": WORKER_EVENT_TYPE,
            "payload": {
                "method": "ios/user_message",
                "message_id": "msg-1",
                "text": "hello",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });
        let snake_case = json!({
            "event_type": WORKER_EVENT_TYPE,
            "payload": {
                "method": "ios/user_message",
                "message_id": "msg-2",
                "text": "hi",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        let camel_decoded: ProtoWorkerEventEnvelope =
            serde_json::from_value(camel_case).expect("camelCase envelope should decode");
        let snake_decoded: ProtoWorkerEventEnvelope =
            serde_json::from_value(snake_case).expect("snake_case envelope should decode");

        assert_eq!(camel_decoded.event_type, WORKER_EVENT_TYPE);
        assert_eq!(snake_decoded.event_type, WORKER_EVENT_TYPE);
        assert!(camel_decoded.payload.is_object());
        assert!(snake_decoded.payload.is_object());
    }

    fn process_handshake_event(
        worker_id: &str,
        payload: &Value,
        acked: &mut HashSet<String>,
        mark_ack: bool,
    ) -> bool {
        if let Some(handshake_id) = extract_desktop_handshake_ack_id(payload)
            && mark_ack
        {
            acked.insert(handshake_dedupe_key(worker_id, &handshake_id));
            return true;
        }

        if let Some(handshake_id) = extract_ios_handshake_id(payload) {
            let key = handshake_dedupe_key(worker_id, &handshake_id);
            return !acked.contains(key.as_str());
        }

        false
    }
}
