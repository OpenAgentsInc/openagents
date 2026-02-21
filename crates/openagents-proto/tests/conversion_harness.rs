use openagents_proto::domain::{
    CodexNotification, CodexNotificationMethod, ControlAuthSession, ControlSessionStatus,
    ConversionError, KhalaFrame, RuntimeRunEvent, RuntimeRunEventPayloadKind,
};
use openagents_proto::wire::openagents::codex::v1::{
    CodexDesktopHandshakeAckPayload, CodexIosHandshakePayload, CodexNotificationEnvelope,
    CodexNotificationMethod as WireCodexNotificationMethod, CodexThreadStartedPayload,
    CodexTurnStartedPayload, CodexUserMessagePayload, codex_notification_envelope,
};
use openagents_proto::wire::openagents::control::v1::{
    AuthSession, SessionStatus as WireSessionStatus,
};
use openagents_proto::wire::openagents::runtime::v1::{
    RuntimeRunEvent as WireRuntimeRunEvent, RuntimeRunFinishedPayload, RuntimeRunStatus,
    RuntimeTextDeltaPayload, runtime_run_event,
};
use openagents_proto::wire::openagents::sync::v1::{KhalaFrame as WireKhalaFrame, KhalaFrameKind};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/conversion-harness-v1.json"))
        .expect("fixture JSON must parse")
}

fn scenario<'a>(root: &'a Value, path: &str) -> &'a Value {
    let mut node = root;
    for segment in path.split('.') {
        node = node
            .get(segment)
            .unwrap_or_else(|| panic!("missing fixture path segment: {path}"));
    }
    node
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn payload_object(value: &Value) -> &Value {
    value.get("payload").unwrap_or(&Value::Null)
}

fn build_wire_khala_frame(value: &Value) -> WireKhalaFrame {
    let kind = match string_field(value, "kind").as_str() {
        "SUBSCRIBED" => KhalaFrameKind::Subscribed as i32,
        "UPDATE_BATCH" => KhalaFrameKind::UpdateBatch as i32,
        "HEARTBEAT" => KhalaFrameKind::Heartbeat as i32,
        "ERROR" => KhalaFrameKind::Error as i32,
        _ => KhalaFrameKind::Unspecified as i32,
    };

    let payload_bytes = value
        .get("payload_bytes")
        .and_then(Value::as_array)
        .map(|bytes| {
            bytes
                .iter()
                .filter_map(Value::as_u64)
                .map(|raw| raw as u8)
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    WireKhalaFrame {
        topic: string_field(value, "topic"),
        seq: u64_field(value, "seq"),
        kind,
        payload_bytes,
        schema_version: value
            .get("schema_version")
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32,
    }
}

fn build_wire_auth_session(value: &Value) -> AuthSession {
    let status = match string_field(value, "status").as_str() {
        "ACTIVE" => WireSessionStatus::Active as i32,
        "REAUTH_REQUIRED" => WireSessionStatus::ReauthRequired as i32,
        "EXPIRED" => WireSessionStatus::Expired as i32,
        "REVOKED" => WireSessionStatus::Revoked as i32,
        _ => WireSessionStatus::Unspecified as i32,
    };

    AuthSession {
        session_id: string_field(value, "session_id"),
        user_id: string_field(value, "user_id"),
        device_id: string_field(value, "device_id"),
        status,
        access_token: string_field(value, "access_token"),
        refresh_token: string_field(value, "refresh_token"),
        active_org_id: string_field(value, "active_org_id"),
        reauth_required: bool_field(value, "reauth_required"),
        ..Default::default()
    }
}

fn build_wire_codex_notification(value: &Value) -> CodexNotificationEnvelope {
    let method = match string_field(value, "method").as_str() {
        "THREAD_STARTED" => WireCodexNotificationMethod::ThreadStarted as i32,
        "TURN_STARTED" => WireCodexNotificationMethod::TurnStarted as i32,
        "TURN_COMPLETED" => WireCodexNotificationMethod::TurnCompleted as i32,
        "ITEM_STARTED" => WireCodexNotificationMethod::ItemStarted as i32,
        "ITEM_COMPLETED" => WireCodexNotificationMethod::ItemCompleted as i32,
        "ITEM_AGENT_MESSAGE_DELTA" => WireCodexNotificationMethod::ItemAgentMessageDelta as i32,
        "ITEM_REASONING_DELTA" => WireCodexNotificationMethod::ItemReasoningDelta as i32,
        "CODEX_ERROR" => WireCodexNotificationMethod::CodexError as i32,
        "IOS_HANDSHAKE" => WireCodexNotificationMethod::IosHandshake as i32,
        "DESKTOP_HANDSHAKE_ACK" => WireCodexNotificationMethod::DesktopHandshakeAck as i32,
        "USER_MESSAGE" => WireCodexNotificationMethod::UserMessage as i32,
        _ => WireCodexNotificationMethod::Unspecified as i32,
    };

    let payload = payload_object(value);
    let payload_variant = match string_field(value, "payload_type").as_str() {
        "thread_started" => Some(codex_notification_envelope::Payload::ThreadStarted(
            CodexThreadStartedPayload {
                thread_id: string_field(payload, "thread_id"),
            },
        )),
        "turn_started" => Some(codex_notification_envelope::Payload::TurnStarted(
            CodexTurnStartedPayload {
                thread_id: string_field(payload, "thread_id"),
                turn_id: string_field(payload, "turn_id"),
                model: string_field(payload, "model"),
                reasoning_effort: string_field(payload, "reasoning_effort"),
            },
        )),
        "ios_handshake" => Some(codex_notification_envelope::Payload::IosHandshake(
            CodexIosHandshakePayload {
                handshake_id: string_field(payload, "handshake_id"),
                device_id: string_field(payload, "device_id"),
                occurred_at: None,
            },
        )),
        "desktop_handshake_ack" => Some(codex_notification_envelope::Payload::DesktopHandshakeAck(
            CodexDesktopHandshakeAckPayload {
                handshake_id: string_field(payload, "handshake_id"),
                desktop_session_id: string_field(payload, "desktop_session_id"),
                occurred_at: None,
            },
        )),
        "user_message" => Some(codex_notification_envelope::Payload::UserMessage(
            CodexUserMessagePayload {
                thread_id: string_field(payload, "thread_id"),
                turn_id: string_field(payload, "turn_id"),
                message_id: string_field(payload, "message_id"),
                text: string_field(payload, "text"),
                model: string_field(payload, "model"),
                reasoning_effort: string_field(payload, "reasoning_effort"),
            },
        )),
        "none" | "" => None,
        other => panic!("unsupported codex payload_type in fixture: {other}"),
    };

    CodexNotificationEnvelope {
        worker_id: string_field(value, "worker_id"),
        seq: u64_field(value, "seq"),
        method,
        payload: payload_variant,
        ..Default::default()
    }
}

fn build_wire_runtime_run_event(value: &Value) -> WireRuntimeRunEvent {
    let payload = payload_object(value);

    let payload_variant = match string_field(value, "payload_type").as_str() {
        "text_delta" => Some(runtime_run_event::Payload::TextDelta(
            RuntimeTextDeltaPayload {
                delta: string_field(payload, "delta"),
                frame_id: string_field(payload, "frame_id"),
            },
        )),
        "run_finished" => {
            let status = match string_field(payload, "status").as_str() {
                "CREATED" => RuntimeRunStatus::Created as i32,
                "RUNNING" => RuntimeRunStatus::Running as i32,
                "CANCELING" => RuntimeRunStatus::Canceling as i32,
                "CANCELED" => RuntimeRunStatus::Canceled as i32,
                "SUCCEEDED" => RuntimeRunStatus::Succeeded as i32,
                "FAILED" => RuntimeRunStatus::Failed as i32,
                _ => RuntimeRunStatus::Unspecified as i32,
            };

            Some(runtime_run_event::Payload::RunFinished(
                RuntimeRunFinishedPayload {
                    status,
                    reason_class: string_field(payload, "reason_class"),
                    reason: string_field(payload, "reason"),
                    ..Default::default()
                },
            ))
        }
        "none" | "" => None,
        other => panic!("unsupported runtime payload_type in fixture: {other}"),
    };

    WireRuntimeRunEvent {
        run_id: string_field(value, "run_id"),
        seq: u64_field(value, "seq"),
        event_type: string_field(value, "event_type"),
        payload: payload_variant,
        ..Default::default()
    }
}

#[test]
fn sync_conversion_fixture_success_and_failure() {
    let root = fixture();

    let valid_wire = build_wire_khala_frame(scenario(&root, "sync.valid"));
    let domain =
        KhalaFrame::try_from(valid_wire.clone()).expect("valid sync conversion should pass");
    assert_eq!(domain.topic, "runtime.codex_worker_events");

    let round_trip = WireKhalaFrame::from(domain);
    assert_eq!(round_trip, valid_wire);

    let invalid_wire = build_wire_khala_frame(scenario(&root, "sync.invalid_missing_topic"));
    let error = KhalaFrame::try_from(invalid_wire).expect_err("missing topic should fail");
    assert_eq!(
        error,
        ConversionError::MissingField {
            message: "KhalaFrame",
            field: "topic"
        }
    );
}

#[test]
fn auth_session_conversion_fixture_success_and_failure() {
    let root = fixture();

    let valid_wire = build_wire_auth_session(scenario(&root, "control_auth_session.valid"));
    let domain = ControlAuthSession::try_from(valid_wire).expect("valid auth session should pass");
    assert_eq!(domain.status, ControlSessionStatus::Active);

    let missing_wire = build_wire_auth_session(scenario(
        &root,
        "control_auth_session.invalid_missing_session_id",
    ));
    let missing_error =
        ControlAuthSession::try_from(missing_wire).expect_err("missing session id should fail");
    assert_eq!(
        missing_error,
        ConversionError::MissingField {
            message: "AuthSession",
            field: "session_id"
        }
    );

    let status_wire = build_wire_auth_session(scenario(
        &root,
        "control_auth_session.invalid_status_unspecified",
    ));
    let status_error =
        ControlAuthSession::try_from(status_wire).expect_err("unspecified status should fail");
    assert_eq!(
        status_error,
        ConversionError::InvalidEnum {
            message: "AuthSession",
            field: "status",
            value: WireSessionStatus::Unspecified as i32
        }
    );
}

#[test]
fn codex_notification_conversion_fixture_success_and_failure() {
    let root = fixture();

    let valid_wire =
        build_wire_codex_notification(scenario(&root, "codex_notification.valid_turn_started"));
    let domain =
        CodexNotification::try_from(valid_wire).expect("valid codex notification should pass");
    assert_eq!(domain.method, CodexNotificationMethod::TurnStarted);
    assert_eq!(domain.turn_id.as_deref(), Some("turn_01"));

    let mismatch_wire = build_wire_codex_notification(scenario(
        &root,
        "codex_notification.invalid_payload_mismatch",
    ));
    let mismatch_error =
        CodexNotification::try_from(mismatch_wire).expect_err("payload mismatch should fail");
    assert_eq!(
        mismatch_error,
        ConversionError::PayloadMismatch {
            message: "CodexNotificationEnvelope",
            expected: "turn_started",
            actual: "thread_started"
        }
    );

    let missing_text_wire = build_wire_codex_notification(scenario(
        &root,
        "codex_notification.invalid_user_message_missing_text",
    ));
    let missing_text_error = CodexNotification::try_from(missing_text_wire)
        .expect_err("missing user message text should fail");
    assert_eq!(
        missing_text_error,
        ConversionError::MissingField {
            message: "CodexUserMessagePayload",
            field: "text"
        }
    );
}

#[test]
fn runtime_run_event_conversion_fixture_success_and_failure() {
    let root = fixture();

    let valid_wire =
        build_wire_runtime_run_event(scenario(&root, "runtime_run_event.valid_text_delta"));
    let domain = RuntimeRunEvent::try_from(valid_wire).expect("valid runtime event should pass");
    assert_eq!(domain.payload_kind, RuntimeRunEventPayloadKind::TextDelta);

    let missing_payload_wire =
        build_wire_runtime_run_event(scenario(&root, "runtime_run_event.invalid_missing_payload"));
    let missing_payload_error =
        RuntimeRunEvent::try_from(missing_payload_wire).expect_err("missing payload should fail");
    assert_eq!(
        missing_payload_error,
        ConversionError::MissingPayload {
            message: "RuntimeRunEvent",
            payload: "payload"
        }
    );

    let invalid_status_wire = build_wire_runtime_run_event(scenario(
        &root,
        "runtime_run_event.invalid_run_finished_status_unspecified",
    ));
    let invalid_status_error = RuntimeRunEvent::try_from(invalid_status_wire)
        .expect_err("unspecified run finished status should fail");
    assert_eq!(
        invalid_status_error,
        ConversionError::InvalidEnum {
            message: "RuntimeRunFinishedPayload",
            field: "status",
            value: RuntimeRunStatus::Unspecified as i32
        }
    );
}
