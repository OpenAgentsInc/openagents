pub use openagents_client_core::khala_protocol::{
    RuntimeStreamEvent as RuntimeCodexStreamEvent, build_phoenix_frame as build_khala_frame,
    extract_runtime_stream_events as extract_runtime_events_from_khala_update, merge_retry_cursor,
    parse_phoenix_frame as parse_khala_frame, runtime_stream_event_seq as stream_event_seq,
    sync_error_code as khala_error_code,
};

#[allow(unused_imports)]
pub use openagents_codex_control::{
    ControlMethod, ControlRequestEnvelope, ControlRequestParseError, IosUserMessage,
    RequestReplayState, TargetResolutionInput, WorkerReceipt, error_receipt as build_error_receipt,
    extract_control_request, extract_desktop_handshake_ack_id, extract_ios_handshake_id,
    extract_ios_user_message, handshake_dedupe_key, ios_message_dedupe_key, request_dedupe_key,
    resolve_request_target, success_receipt as build_success_receipt, terminal_receipt_dedupe_key,
};

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCodexStreamEvent, build_khala_frame, extract_control_request,
        extract_desktop_handshake_ack_id, extract_ios_handshake_id, extract_ios_user_message,
        extract_runtime_events_from_khala_update, handshake_dedupe_key, khala_error_code,
        merge_retry_cursor, parse_khala_frame, stream_event_seq, terminal_receipt_dedupe_key,
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
    fn parse_khala_frame_roundtrips_phoenix_frame_shape() {
        let raw = build_khala_frame(
            None,
            Some("42"),
            "sync:v1",
            "sync:update_batch",
            json!({"updates": []}),
        );
        let frame = parse_khala_frame(&raw);
        assert!(frame.is_some());
        let frame = frame.unwrap_or_else(|| unreachable!());
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

        let first_emit = process_handshake_event(worker_id, &handshake_payload, &mut acked, false);
        assert!(first_emit);
        assert!(acked.is_empty());

        let rewind_cursor = merge_retry_cursor(None, 200);
        assert_eq!(rewind_cursor, 199);

        let replay_emit = process_handshake_event(worker_id, &handshake_payload, &mut acked, true);
        assert!(replay_emit);
        assert_eq!(acked.len(), 1);

        let ack_emit = process_handshake_event(worker_id, &ack_payload, &mut acked, true);
        assert!(!ack_emit);
        assert_eq!(acked.len(), 1);

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

        let message = extract_ios_user_message(&payload);
        assert!(message.is_some());
        let message = message.unwrap_or_else(|| unreachable!());
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
    fn extract_control_request_parses_runtime_request_envelope() {
        let payload = json!({
            "eventType": "worker.request",
            "payload": {
                "request_id": "req-1",
                "method": "thread/list",
                "params": {"limit": 10}
            }
        });

        let request = extract_control_request(&payload);
        assert!(request.is_ok());
        let request = request.unwrap_or_else(|_| unreachable!());
        assert!(request.is_some());
        let request = request.unwrap_or_else(|| unreachable!());
        assert_eq!(request.request_id, "req-1");
    }

    #[test]
    fn terminal_receipt_dedupe_key_is_stable_across_restart_resume_replay() {
        let worker_id = "desktopw:shared";
        let request_id = "req-terminal-1";
        let replayed_key = terminal_receipt_dedupe_key(worker_id, request_id);

        let mut seen = HashSet::new();
        assert!(seen.insert(replayed_key.clone()));
        assert!(!seen.insert(replayed_key));

        let next_request_key = terminal_receipt_dedupe_key(worker_id, "req-terminal-2");
        assert!(seen.insert(next_request_key));
    }

    #[test]
    fn codex_contract_fixture_is_decodable_by_desktop_parser() {
        let fixture_result: Result<Value, _> = serde_json::from_str(include_str!(
            "../../../docs/protocol/fixtures/codex-worker-events-v1.json"
        ));
        assert!(fixture_result.is_ok());
        let fixture = fixture_result.unwrap_or_else(|_| unreachable!());

        let notifications = fixture["notification_events"]
            .as_array()
            .unwrap_or_else(|| unreachable!());

        assert!(notifications.len() >= 10);

        let mut saw_ios_handshake = false;
        let mut saw_desktop_ack = false;
        let mut saw_ios_user_message = false;

        for notification in notifications {
            let seq = notification["seq"]
                .as_u64()
                .unwrap_or_else(|| unreachable!());
            let payload = notification["payload"].clone();

            let stream_payload = json!({
                "seq": seq,
                "eventType": WORKER_EVENT_TYPE,
                "payload": payload
            });

            let parsed = serde_json::from_value::<ProtoWorkerEventEnvelope>(stream_payload.clone());
            assert!(parsed.is_ok());
            let parsed = parsed.unwrap_or_else(|_| unreachable!());
            assert_eq!(parsed.event_type, WORKER_EVENT_TYPE);
            assert!(parsed.payload.is_object());

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
