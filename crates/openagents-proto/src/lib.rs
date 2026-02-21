//! OpenAgents proto wire contracts.
//!
//! This crate owns generated Rust wire types for all `proto/openagents/*/v1/*` packages.
//! It intentionally separates wire types from richer domain models.

/// Proto-generated wire contracts.
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/openagents.rs"));
}

/// Domain-layer wrappers and conversions over generated wire contracts.
pub mod domain {
    use thiserror::Error;

    use crate::wire::openagents::codex::v1::{
        CodexNotificationEnvelope as WireCodexNotificationEnvelope,
        CodexNotificationMethod as WireCodexNotificationMethod, codex_notification_envelope,
    };
    use crate::wire::openagents::control::v1::{
        AuthSession as WireAuthSession, SessionStatus as WireSessionStatus,
    };
    use crate::wire::openagents::runtime::v1::{
        RuntimeRunEvent as WireRuntimeRunEvent, RuntimeRunStatus as WireRuntimeRunStatus,
        runtime_run_event,
    };
    use crate::wire::openagents::sync::v1::{KhalaFrame as WireKhalaFrame, KhalaFrameKind};

    /// Conversion failures from wire-level payloads.
    #[derive(Debug, Clone, Error, PartialEq, Eq)]
    pub enum ConversionError {
        #[error("{message}.{field} is required")]
        MissingField {
            message: &'static str,
            field: &'static str,
        },
        #[error("{message}.{field} has invalid enum value: {value}")]
        InvalidEnum {
            message: &'static str,
            field: &'static str,
            value: i32,
        },
        #[error("{message} missing payload: {payload}")]
        MissingPayload {
            message: &'static str,
            payload: &'static str,
        },
        #[error("{message} payload mismatch: expected {expected}, got {actual}")]
        PayloadMismatch {
            message: &'static str,
            expected: &'static str,
            actual: &'static str,
        },
        #[error("{message}.{field} is invalid: {reason}")]
        InvalidValue {
            message: &'static str,
            field: &'static str,
            reason: &'static str,
        },
    }

    /// Domain-level representation for a Khala frame.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KhalaFrame {
        pub topic: String,
        pub seq: u64,
        pub kind: KhalaFrameDomainKind,
        pub payload_bytes: Vec<u8>,
        pub schema_version: u32,
    }

    /// Stable domain enum mapped from proto `KhalaFrameKind`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum KhalaFrameDomainKind {
        Subscribed,
        UpdateBatch,
        Heartbeat,
        Error,
        Unknown(i32),
    }

    /// Domain-level auth session model.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ControlAuthSession {
        pub session_id: String,
        pub user_id: String,
        pub device_id: String,
        pub status: ControlSessionStatus,
        pub access_token: String,
        pub refresh_token: String,
        pub active_org_id: String,
        pub reauth_required: bool,
    }

    /// Domain session status enum.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ControlSessionStatus {
        Active,
        ReauthRequired,
        Expired,
        Revoked,
    }

    /// Domain view of a codex notification envelope.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CodexNotification {
        pub worker_id: String,
        pub seq: u64,
        pub method: CodexNotificationMethod,
        pub thread_id: Option<String>,
        pub turn_id: Option<String>,
        pub item_id: Option<String>,
    }

    /// Supported codex notification methods in domain space.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CodexNotificationMethod {
        ThreadStarted,
        TurnStarted,
        TurnCompleted,
        ItemStarted,
        ItemCompleted,
        ItemAgentMessageDelta,
        ItemReasoningDelta,
        CodexError,
        IosHandshake,
        DesktopHandshakeAck,
        UserMessage,
    }

    /// Domain view of runtime run event envelopes.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RuntimeRunEvent {
        pub run_id: String,
        pub seq: u64,
        pub event_type: String,
        pub payload_kind: RuntimeRunEventPayloadKind,
    }

    /// Runtime run payload categories used by domain reducers.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RuntimeRunEventPayloadKind {
        RunStarted,
        TextDelta,
        RunFinished,
        Other,
    }

    impl TryFrom<WireKhalaFrame> for KhalaFrame {
        type Error = ConversionError;

        fn try_from(value: WireKhalaFrame) -> Result<Self, Self::Error> {
            require_non_empty(&value.topic, "KhalaFrame", "topic")?;

            Ok(Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_from_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            })
        }
    }

    impl From<KhalaFrame> for WireKhalaFrame {
        fn from(value: KhalaFrame) -> Self {
            Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_to_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            }
        }
    }

    impl TryFrom<WireAuthSession> for ControlAuthSession {
        type Error = ConversionError;

        fn try_from(value: WireAuthSession) -> Result<Self, Self::Error> {
            require_non_empty(&value.session_id, "AuthSession", "session_id")?;
            require_non_empty(&value.user_id, "AuthSession", "user_id")?;
            require_non_empty(&value.device_id, "AuthSession", "device_id")?;
            require_non_empty(&value.access_token, "AuthSession", "access_token")?;
            require_non_empty(&value.refresh_token, "AuthSession", "refresh_token")?;
            require_non_empty(&value.active_org_id, "AuthSession", "active_org_id")?;

            let status = session_status_from_i32(value.status)?;

            Ok(Self {
                session_id: value.session_id,
                user_id: value.user_id,
                device_id: value.device_id,
                status,
                access_token: value.access_token,
                refresh_token: value.refresh_token,
                active_org_id: value.active_org_id,
                reauth_required: value.reauth_required,
            })
        }
    }

    impl TryFrom<WireCodexNotificationEnvelope> for CodexNotification {
        type Error = ConversionError;

        fn try_from(value: WireCodexNotificationEnvelope) -> Result<Self, Self::Error> {
            require_non_empty(&value.worker_id, "CodexNotificationEnvelope", "worker_id")?;

            let method = codex_method_from_i32(value.method)?;
            let payload = value.payload.ok_or(ConversionError::MissingPayload {
                message: "CodexNotificationEnvelope",
                payload: "payload",
            })?;

            let (thread_id, turn_id, item_id) = match (method, payload) {
                (
                    CodexNotificationMethod::ThreadStarted,
                    codex_notification_envelope::Payload::ThreadStarted(payload),
                ) => {
                    require_non_empty(
                        &payload.thread_id,
                        "CodexThreadStartedPayload",
                        "thread_id",
                    )?;
                    (Some(payload.thread_id), None, None)
                }
                (
                    CodexNotificationMethod::TurnStarted,
                    codex_notification_envelope::Payload::TurnStarted(payload),
                ) => {
                    require_non_empty(&payload.thread_id, "CodexTurnStartedPayload", "thread_id")?;
                    require_non_empty(&payload.turn_id, "CodexTurnStartedPayload", "turn_id")?;
                    (Some(payload.thread_id), Some(payload.turn_id), None)
                }
                (
                    CodexNotificationMethod::TurnCompleted,
                    codex_notification_envelope::Payload::TurnCompleted(payload),
                ) => {
                    require_non_empty(
                        &payload.thread_id,
                        "CodexTurnCompletedPayload",
                        "thread_id",
                    )?;
                    require_non_empty(&payload.turn_id, "CodexTurnCompletedPayload", "turn_id")?;
                    (Some(payload.thread_id), Some(payload.turn_id), None)
                }
                (
                    CodexNotificationMethod::ItemStarted,
                    codex_notification_envelope::Payload::ItemLifecycle(payload),
                )
                | (
                    CodexNotificationMethod::ItemCompleted,
                    codex_notification_envelope::Payload::ItemLifecycle(payload),
                ) => {
                    require_non_empty(
                        &payload.thread_id,
                        "CodexItemLifecyclePayload",
                        "thread_id",
                    )?;
                    require_non_empty(&payload.turn_id, "CodexItemLifecyclePayload", "turn_id")?;
                    require_non_empty(&payload.item_id, "CodexItemLifecyclePayload", "item_id")?;
                    (
                        Some(payload.thread_id),
                        Some(payload.turn_id),
                        Some(payload.item_id),
                    )
                }
                (
                    CodexNotificationMethod::ItemAgentMessageDelta,
                    codex_notification_envelope::Payload::TextDelta(payload),
                )
                | (
                    CodexNotificationMethod::ItemReasoningDelta,
                    codex_notification_envelope::Payload::TextDelta(payload),
                ) => {
                    require_non_empty(&payload.thread_id, "CodexTextDeltaPayload", "thread_id")?;
                    require_non_empty(&payload.turn_id, "CodexTextDeltaPayload", "turn_id")?;
                    require_non_empty(&payload.item_id, "CodexTextDeltaPayload", "item_id")?;
                    require_non_empty(&payload.delta, "CodexTextDeltaPayload", "delta")?;
                    (
                        Some(payload.thread_id),
                        Some(payload.turn_id),
                        Some(payload.item_id),
                    )
                }
                (
                    CodexNotificationMethod::CodexError,
                    codex_notification_envelope::Payload::CodexError(payload),
                ) => {
                    require_non_empty(&payload.message, "CodexErrorPayload", "message")?;
                    (
                        non_empty(payload.thread_id),
                        non_empty(payload.turn_id),
                        None,
                    )
                }
                (
                    CodexNotificationMethod::IosHandshake,
                    codex_notification_envelope::Payload::IosHandshake(payload),
                ) => {
                    require_non_empty(
                        &payload.handshake_id,
                        "CodexIosHandshakePayload",
                        "handshake_id",
                    )?;
                    require_non_empty(&payload.device_id, "CodexIosHandshakePayload", "device_id")?;
                    (None, None, None)
                }
                (
                    CodexNotificationMethod::DesktopHandshakeAck,
                    codex_notification_envelope::Payload::DesktopHandshakeAck(payload),
                ) => {
                    require_non_empty(
                        &payload.handshake_id,
                        "CodexDesktopHandshakeAckPayload",
                        "handshake_id",
                    )?;
                    require_non_empty(
                        &payload.desktop_session_id,
                        "CodexDesktopHandshakeAckPayload",
                        "desktop_session_id",
                    )?;
                    (None, None, None)
                }
                (
                    CodexNotificationMethod::UserMessage,
                    codex_notification_envelope::Payload::UserMessage(payload),
                ) => {
                    require_non_empty(&payload.thread_id, "CodexUserMessagePayload", "thread_id")?;
                    require_non_empty(&payload.turn_id, "CodexUserMessagePayload", "turn_id")?;
                    require_non_empty(
                        &payload.message_id,
                        "CodexUserMessagePayload",
                        "message_id",
                    )?;
                    require_non_empty(&payload.text, "CodexUserMessagePayload", "text")?;
                    (Some(payload.thread_id), Some(payload.turn_id), None)
                }
                (method, payload) => {
                    return Err(ConversionError::PayloadMismatch {
                        message: "CodexNotificationEnvelope",
                        expected: expected_codex_payload(method),
                        actual: codex_payload_name(&payload),
                    });
                }
            };

            Ok(Self {
                worker_id: value.worker_id,
                seq: value.seq,
                method,
                thread_id,
                turn_id,
                item_id,
            })
        }
    }

    impl TryFrom<WireRuntimeRunEvent> for RuntimeRunEvent {
        type Error = ConversionError;

        fn try_from(value: WireRuntimeRunEvent) -> Result<Self, Self::Error> {
            require_non_empty(&value.run_id, "RuntimeRunEvent", "run_id")?;
            require_non_empty(&value.event_type, "RuntimeRunEvent", "event_type")?;

            if value.seq == 0 {
                return Err(ConversionError::InvalidValue {
                    message: "RuntimeRunEvent",
                    field: "seq",
                    reason: "must be > 0",
                });
            }

            let payload = value.payload.ok_or(ConversionError::MissingPayload {
                message: "RuntimeRunEvent",
                payload: "payload",
            })?;

            let payload_kind = match payload {
                runtime_run_event::Payload::RunStarted(payload) => {
                    require_non_empty(&payload.actor, "RuntimeRunStartedPayload", "actor")?;
                    RuntimeRunEventPayloadKind::RunStarted
                }
                runtime_run_event::Payload::TextDelta(payload) => {
                    require_non_empty(&payload.delta, "RuntimeTextDeltaPayload", "delta")?;
                    RuntimeRunEventPayloadKind::TextDelta
                }
                runtime_run_event::Payload::RunFinished(payload) => {
                    runtime_status_from_i32(payload.status)?;
                    RuntimeRunEventPayloadKind::RunFinished
                }
                _ => RuntimeRunEventPayloadKind::Other,
            };

            Ok(Self {
                run_id: value.run_id,
                seq: value.seq,
                event_type: value.event_type,
                payload_kind,
            })
        }
    }

    fn kind_from_i32(raw: i32) -> KhalaFrameDomainKind {
        if raw == KhalaFrameKind::Subscribed as i32 {
            KhalaFrameDomainKind::Subscribed
        } else if raw == KhalaFrameKind::UpdateBatch as i32 {
            KhalaFrameDomainKind::UpdateBatch
        } else if raw == KhalaFrameKind::Heartbeat as i32 {
            KhalaFrameDomainKind::Heartbeat
        } else if raw == KhalaFrameKind::Error as i32 {
            KhalaFrameDomainKind::Error
        } else {
            KhalaFrameDomainKind::Unknown(raw)
        }
    }

    fn kind_to_i32(kind: KhalaFrameDomainKind) -> i32 {
        match kind {
            KhalaFrameDomainKind::Subscribed => KhalaFrameKind::Subscribed as i32,
            KhalaFrameDomainKind::UpdateBatch => KhalaFrameKind::UpdateBatch as i32,
            KhalaFrameDomainKind::Heartbeat => KhalaFrameKind::Heartbeat as i32,
            KhalaFrameDomainKind::Error => KhalaFrameKind::Error as i32,
            KhalaFrameDomainKind::Unknown(raw) => raw,
        }
    }

    fn session_status_from_i32(value: i32) -> Result<ControlSessionStatus, ConversionError> {
        if value == WireSessionStatus::Active as i32 {
            Ok(ControlSessionStatus::Active)
        } else if value == WireSessionStatus::ReauthRequired as i32 {
            Ok(ControlSessionStatus::ReauthRequired)
        } else if value == WireSessionStatus::Expired as i32 {
            Ok(ControlSessionStatus::Expired)
        } else if value == WireSessionStatus::Revoked as i32 {
            Ok(ControlSessionStatus::Revoked)
        } else {
            Err(ConversionError::InvalidEnum {
                message: "AuthSession",
                field: "status",
                value,
            })
        }
    }

    fn codex_method_from_i32(value: i32) -> Result<CodexNotificationMethod, ConversionError> {
        if value == WireCodexNotificationMethod::ThreadStarted as i32 {
            Ok(CodexNotificationMethod::ThreadStarted)
        } else if value == WireCodexNotificationMethod::TurnStarted as i32 {
            Ok(CodexNotificationMethod::TurnStarted)
        } else if value == WireCodexNotificationMethod::TurnCompleted as i32 {
            Ok(CodexNotificationMethod::TurnCompleted)
        } else if value == WireCodexNotificationMethod::ItemStarted as i32 {
            Ok(CodexNotificationMethod::ItemStarted)
        } else if value == WireCodexNotificationMethod::ItemCompleted as i32 {
            Ok(CodexNotificationMethod::ItemCompleted)
        } else if value == WireCodexNotificationMethod::ItemAgentMessageDelta as i32 {
            Ok(CodexNotificationMethod::ItemAgentMessageDelta)
        } else if value == WireCodexNotificationMethod::ItemReasoningDelta as i32 {
            Ok(CodexNotificationMethod::ItemReasoningDelta)
        } else if value == WireCodexNotificationMethod::CodexError as i32 {
            Ok(CodexNotificationMethod::CodexError)
        } else if value == WireCodexNotificationMethod::IosHandshake as i32 {
            Ok(CodexNotificationMethod::IosHandshake)
        } else if value == WireCodexNotificationMethod::DesktopHandshakeAck as i32 {
            Ok(CodexNotificationMethod::DesktopHandshakeAck)
        } else if value == WireCodexNotificationMethod::UserMessage as i32 {
            Ok(CodexNotificationMethod::UserMessage)
        } else {
            Err(ConversionError::InvalidEnum {
                message: "CodexNotificationEnvelope",
                field: "method",
                value,
            })
        }
    }

    fn expected_codex_payload(method: CodexNotificationMethod) -> &'static str {
        match method {
            CodexNotificationMethod::ThreadStarted => "thread_started",
            CodexNotificationMethod::TurnStarted => "turn_started",
            CodexNotificationMethod::TurnCompleted => "turn_completed",
            CodexNotificationMethod::ItemStarted => "item_lifecycle",
            CodexNotificationMethod::ItemCompleted => "item_lifecycle",
            CodexNotificationMethod::ItemAgentMessageDelta => "text_delta",
            CodexNotificationMethod::ItemReasoningDelta => "text_delta",
            CodexNotificationMethod::CodexError => "codex_error",
            CodexNotificationMethod::IosHandshake => "ios_handshake",
            CodexNotificationMethod::DesktopHandshakeAck => "desktop_handshake_ack",
            CodexNotificationMethod::UserMessage => "user_message",
        }
    }

    fn codex_payload_name(payload: &codex_notification_envelope::Payload) -> &'static str {
        match payload {
            codex_notification_envelope::Payload::ThreadStarted(_) => "thread_started",
            codex_notification_envelope::Payload::TurnStarted(_) => "turn_started",
            codex_notification_envelope::Payload::TurnCompleted(_) => "turn_completed",
            codex_notification_envelope::Payload::TurnFailure(_) => "turn_failure",
            codex_notification_envelope::Payload::ItemLifecycle(_) => "item_lifecycle",
            codex_notification_envelope::Payload::TextDelta(_) => "text_delta",
            codex_notification_envelope::Payload::ToolOutputDelta(_) => "tool_output_delta",
            codex_notification_envelope::Payload::CodexError(_) => "codex_error",
            codex_notification_envelope::Payload::IosHandshake(_) => "ios_handshake",
            codex_notification_envelope::Payload::DesktopHandshakeAck(_) => "desktop_handshake_ack",
            codex_notification_envelope::Payload::UserMessage(_) => "user_message",
            codex_notification_envelope::Payload::Unknown(_) => "unknown",
        }
    }

    fn runtime_status_from_i32(value: i32) -> Result<(), ConversionError> {
        if value == WireRuntimeRunStatus::Created as i32
            || value == WireRuntimeRunStatus::Running as i32
            || value == WireRuntimeRunStatus::Canceling as i32
            || value == WireRuntimeRunStatus::Canceled as i32
            || value == WireRuntimeRunStatus::Succeeded as i32
            || value == WireRuntimeRunStatus::Failed as i32
        {
            Ok(())
        } else {
            Err(ConversionError::InvalidEnum {
                message: "RuntimeRunFinishedPayload",
                field: "status",
                value,
            })
        }
    }

    fn require_non_empty(
        value: &str,
        message: &'static str,
        field: &'static str,
    ) -> Result<(), ConversionError> {
        if value.trim().is_empty() {
            Err(ConversionError::MissingField { message, field })
        } else {
            Ok(())
        }
    }

    fn non_empty(value: String) -> Option<String> {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        ControlAuthSession, ControlSessionStatus, ConversionError, KhalaFrame, RuntimeRunEvent,
        RuntimeRunEventPayloadKind,
    };
    use crate::wire::openagents::codex::v1::{
        CodexNotificationEnvelope, codex_notification_envelope,
    };
    use crate::wire::openagents::control::v1::{AuthSession, SessionStatus};
    use crate::wire::openagents::runtime::v1::{
        RuntimeRunEvent as WireRuntimeRunEvent, RuntimeRunFinishedPayload, runtime_run_event,
    };
    use crate::wire::openagents::sync::v1::KhalaFrame as WireKhalaFrame;

    #[test]
    fn khala_frame_wire_domain_round_trip() {
        let wire = WireKhalaFrame {
            topic: "runtime.codex_worker_events".to_string(),
            seq: 42,
            kind: 2,
            payload_bytes: vec![1, 2, 3],
            schema_version: 1,
        };

        let domain = KhalaFrame::try_from(wire.clone()).expect("wire to domain should succeed");
        assert_eq!(domain.topic, "runtime.codex_worker_events");
        assert_eq!(domain.seq, 42);

        let round_trip = WireKhalaFrame::from(domain);
        assert_eq!(round_trip, wire);
    }

    #[test]
    fn auth_session_requires_status_and_ids() {
        let wire = AuthSession {
            session_id: "sess_1".to_string(),
            user_id: "1".to_string(),
            device_id: "ios:device".to_string(),
            status: SessionStatus::Active as i32,
            access_token: "pat_local".to_string(),
            refresh_token: "rt_local".to_string(),
            active_org_id: "user:1".to_string(),
            reauth_required: false,
            ..Default::default()
        };

        let domain = ControlAuthSession::try_from(wire).expect("auth session should convert");
        assert_eq!(domain.status, ControlSessionStatus::Active);

        let invalid = AuthSession::default();
        let error = ControlAuthSession::try_from(invalid).expect_err("missing ids should fail");
        assert_eq!(
            error,
            ConversionError::MissingField {
                message: "AuthSession",
                field: "session_id"
            }
        );
    }

    #[test]
    fn codex_notification_rejects_payload_mismatch() {
        let wire = CodexNotificationEnvelope {
            worker_id: "desktopw:test".to_string(),
            seq: 10,
            method: 2,
            payload: Some(codex_notification_envelope::Payload::ThreadStarted(
                crate::wire::openagents::codex::v1::CodexThreadStartedPayload {
                    thread_id: "thr_1".to_string(),
                },
            )),
            ..Default::default()
        };

        let error = crate::domain::CodexNotification::try_from(wire)
            .expect_err("payload mismatch should fail");

        assert_eq!(
            error,
            ConversionError::PayloadMismatch {
                message: "CodexNotificationEnvelope",
                expected: "turn_started",
                actual: "thread_started"
            }
        );
    }

    #[test]
    fn runtime_run_event_validates_payload_status_enum() {
        let wire = WireRuntimeRunEvent {
            run_id: "run_1".to_string(),
            seq: 1,
            event_type: "run.finished".to_string(),
            payload: Some(runtime_run_event::Payload::RunFinished(
                RuntimeRunFinishedPayload {
                    status: 999,
                    ..Default::default()
                },
            )),
            ..Default::default()
        };

        let error = RuntimeRunEvent::try_from(wire).expect_err("invalid status should fail");
        assert_eq!(
            error,
            ConversionError::InvalidEnum {
                message: "RuntimeRunFinishedPayload",
                field: "status",
                value: 999
            }
        );

        let ok = WireRuntimeRunEvent {
            run_id: "run_2".to_string(),
            seq: 2,
            event_type: "text.delta".to_string(),
            payload: Some(runtime_run_event::Payload::TextDelta(
                crate::wire::openagents::runtime::v1::RuntimeTextDeltaPayload {
                    delta: "hello".to_string(),
                    frame_id: "frame_1".to_string(),
                },
            )),
            ..Default::default()
        };

        let domain = RuntimeRunEvent::try_from(ok).expect("text delta should convert");
        assert_eq!(domain.payload_kind, RuntimeRunEventPayloadKind::TextDelta);
    }
}
