//! OpenAgents proto wire contracts.
//!
//! This crate owns generated Rust wire types for all `proto/openagents/*/v1/*` packages.
//! It intentionally separates wire types from richer domain models.

pub mod aegis;
pub mod hydra_credit;
pub mod hydra_fx;
pub mod hydra_routing;

/// Proto-generated wire contracts.
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/openagents.rs"));
}

/// Domain-layer wrappers and conversions over generated wire contracts.
pub mod domain {
    use thiserror::Error;

    use crate::wire::openagents::codex::v1::{
        codex_notification_envelope, CodexNotificationEnvelope as WireCodexNotificationEnvelope,
        CodexNotificationMethod as WireCodexNotificationMethod,
    };
    use crate::wire::openagents::control::v1::{
        AuthSession as WireAuthSession, SessionStatus as WireSessionStatus,
    };
    use crate::wire::openagents::runtime::v1::{
        runtime_run_event, RuntimeRunEvent as WireRuntimeRunEvent,
        RuntimeRunStatus as WireRuntimeRunStatus,
    };
    use crate::wire::openagents::sync::v1::{
        SpacetimeFrame as WireSpacetimeFrame, SpacetimeFrameKind,
    };
    use crate::wire::openagents::sync::v2::{
        StreamCheckpoint as WireStreamCheckpoint, SyncErrorCode as WireSyncErrorCode,
        SyncFrame as WireSyncFrame, SyncFrameKind as WireSyncFrameKind,
    };

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

    /// Domain-level representation for a legacy sync frame.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SpacetimeFrame {
        pub topic: String,
        pub seq: u64,
        pub kind: SpacetimeFrameDomainKind,
        pub payload_bytes: Vec<u8>,
        pub schema_version: u32,
    }

    /// Stable domain enum mapped from proto `SpacetimeFrameKind`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpacetimeFrameDomainKind {
        Subscribed,
        UpdateBatch,
        Heartbeat,
        Error,
        Unknown(i32),
    }

    /// Stream-first sync frame model aligned with `openagents.sync.v2.SyncFrame`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SyncStreamFrame {
        pub channel: String,
        pub frame_seq: u64,
        pub kind: SyncStreamFrameKind,
        pub payload_bytes: Vec<u8>,
        pub schema_version: u32,
    }

    /// Stable domain enum mapped from proto `openagents.sync.v2.SyncFrameKind`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SyncStreamFrameKind {
        SubscribeApplied,
        TransactionBatch,
        Heartbeat,
        Error,
        Unknown(i32),
    }

    /// Stream checkpoint domain shape aligned with `openagents.sync.v2.StreamCheckpoint`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SyncStreamCheckpoint {
        pub stream_id: String,
        pub last_applied_seq: u64,
        pub durable_offset: u64,
    }

    /// Typed sync failure classes aligned with `openagents.sync.v2.SyncErrorCode`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SyncFailureClass {
        Unauthorized,
        ForbiddenStream,
        BadSubscription,
        StaleCursor,
        PayloadTooLarge,
        RateLimited,
        UnsupportedProtocolVersion,
        UnsupportedSchemaVersion,
        UpgradeRequired,
        Internal,
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

    impl TryFrom<WireSpacetimeFrame> for SpacetimeFrame {
        type Error = ConversionError;

        fn try_from(value: WireSpacetimeFrame) -> Result<Self, Self::Error> {
            require_non_empty(&value.topic, "SpacetimeFrame", "topic")?;

            Ok(Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_from_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            })
        }
    }

    impl From<SpacetimeFrame> for WireSpacetimeFrame {
        fn from(value: SpacetimeFrame) -> Self {
            Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_to_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            }
        }
    }

    impl TryFrom<WireSyncFrame> for SyncStreamFrame {
        type Error = ConversionError;

        fn try_from(value: WireSyncFrame) -> Result<Self, Self::Error> {
            require_non_empty(&value.channel, "SyncFrame", "channel")?;

            Ok(Self {
                channel: value.channel,
                frame_seq: value.frame_seq,
                kind: sync_v2_kind_from_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            })
        }
    }

    impl From<SyncStreamFrame> for WireSyncFrame {
        fn from(value: SyncStreamFrame) -> Self {
            Self {
                channel: value.channel,
                frame_seq: value.frame_seq,
                kind: sync_v2_kind_to_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            }
        }
    }

    impl TryFrom<WireStreamCheckpoint> for SyncStreamCheckpoint {
        type Error = ConversionError;

        fn try_from(value: WireStreamCheckpoint) -> Result<Self, Self::Error> {
            require_non_empty(&value.stream_id, "StreamCheckpoint", "stream_id")?;
            Ok(Self {
                stream_id: value.stream_id,
                last_applied_seq: value.last_applied_seq,
                durable_offset: value.durable_offset,
            })
        }
    }

    impl From<SyncStreamCheckpoint> for WireStreamCheckpoint {
        fn from(value: SyncStreamCheckpoint) -> Self {
            Self {
                stream_id: value.stream_id,
                last_applied_seq: value.last_applied_seq,
                durable_offset: value.durable_offset,
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

    fn kind_from_i32(raw: i32) -> SpacetimeFrameDomainKind {
        if raw == SpacetimeFrameKind::Subscribed as i32 {
            SpacetimeFrameDomainKind::Subscribed
        } else if raw == SpacetimeFrameKind::UpdateBatch as i32 {
            SpacetimeFrameDomainKind::UpdateBatch
        } else if raw == SpacetimeFrameKind::Heartbeat as i32 {
            SpacetimeFrameDomainKind::Heartbeat
        } else if raw == SpacetimeFrameKind::Error as i32 {
            SpacetimeFrameDomainKind::Error
        } else {
            SpacetimeFrameDomainKind::Unknown(raw)
        }
    }

    fn kind_to_i32(kind: SpacetimeFrameDomainKind) -> i32 {
        match kind {
            SpacetimeFrameDomainKind::Subscribed => SpacetimeFrameKind::Subscribed as i32,
            SpacetimeFrameDomainKind::UpdateBatch => SpacetimeFrameKind::UpdateBatch as i32,
            SpacetimeFrameDomainKind::Heartbeat => SpacetimeFrameKind::Heartbeat as i32,
            SpacetimeFrameDomainKind::Error => SpacetimeFrameKind::Error as i32,
            SpacetimeFrameDomainKind::Unknown(raw) => raw,
        }
    }

    fn sync_v2_kind_from_i32(raw: i32) -> SyncStreamFrameKind {
        if raw == WireSyncFrameKind::SubscribeApplied as i32 {
            SyncStreamFrameKind::SubscribeApplied
        } else if raw == WireSyncFrameKind::TransactionBatch as i32 {
            SyncStreamFrameKind::TransactionBatch
        } else if raw == WireSyncFrameKind::Heartbeat as i32 {
            SyncStreamFrameKind::Heartbeat
        } else if raw == WireSyncFrameKind::Error as i32 {
            SyncStreamFrameKind::Error
        } else {
            SyncStreamFrameKind::Unknown(raw)
        }
    }

    fn sync_v2_kind_to_i32(kind: SyncStreamFrameKind) -> i32 {
        match kind {
            SyncStreamFrameKind::SubscribeApplied => WireSyncFrameKind::SubscribeApplied as i32,
            SyncStreamFrameKind::TransactionBatch => WireSyncFrameKind::TransactionBatch as i32,
            SyncStreamFrameKind::Heartbeat => WireSyncFrameKind::Heartbeat as i32,
            SyncStreamFrameKind::Error => WireSyncFrameKind::Error as i32,
            SyncStreamFrameKind::Unknown(raw) => raw,
        }
    }

    #[must_use]
    pub fn sync_failure_class_from_i32(raw: i32) -> SyncFailureClass {
        if raw == WireSyncErrorCode::Unauthorized as i32 {
            SyncFailureClass::Unauthorized
        } else if raw == WireSyncErrorCode::ForbiddenStream as i32 {
            SyncFailureClass::ForbiddenStream
        } else if raw == WireSyncErrorCode::BadSubscription as i32 {
            SyncFailureClass::BadSubscription
        } else if raw == WireSyncErrorCode::StaleCursor as i32 {
            SyncFailureClass::StaleCursor
        } else if raw == WireSyncErrorCode::PayloadTooLarge as i32 {
            SyncFailureClass::PayloadTooLarge
        } else if raw == WireSyncErrorCode::RateLimited as i32 {
            SyncFailureClass::RateLimited
        } else if raw == WireSyncErrorCode::UnsupportedProtocolVersion as i32 {
            SyncFailureClass::UnsupportedProtocolVersion
        } else if raw == WireSyncErrorCode::UnsupportedSchemaVersion as i32 {
            SyncFailureClass::UnsupportedSchemaVersion
        } else if raw == WireSyncErrorCode::UpgradeRequired as i32 {
            SyncFailureClass::UpgradeRequired
        } else if raw == WireSyncErrorCode::Internal as i32 {
            SyncFailureClass::Internal
        } else {
            SyncFailureClass::Unknown(raw)
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
        sync_failure_class_from_i32, ControlAuthSession, ControlSessionStatus, ConversionError,
        RuntimeRunEvent, RuntimeRunEventPayloadKind, SpacetimeFrame, SyncStreamFrame,
    };
    use crate::wire::openagents::codex::v1::{
        codex_notification_envelope, CodexNotificationEnvelope,
    };
    use crate::wire::openagents::control::v1::{AuthSession, SessionStatus};
    use crate::wire::openagents::runtime::v1::{
        runtime_run_event, RuntimeRunEvent as WireRuntimeRunEvent, RuntimeRunFinishedPayload,
    };
    use crate::wire::openagents::sync::v1::SpacetimeFrame as WireSpacetimeFrame;
    use crate::wire::openagents::sync::v2::{
        SyncErrorCode, SyncFrame as WireSyncFrame, SyncFrameKind,
    };

    #[test]
    fn spacetime_frame_wire_domain_round_trip() {
        let wire = WireSpacetimeFrame {
            topic: "runtime.codex_worker_events".to_string(),
            seq: 42,
            kind: 2,
            payload_bytes: vec![1, 2, 3],
            schema_version: 1,
        };

        let domain = SpacetimeFrame::try_from(wire.clone()).expect("wire to domain should succeed");
        assert_eq!(domain.topic, "runtime.codex_worker_events");
        assert_eq!(domain.seq, 42);

        let round_trip = WireSpacetimeFrame::from(domain);
        assert_eq!(round_trip, wire);
    }

    #[test]
    fn sync_stream_frame_wire_domain_round_trip() {
        let wire = WireSyncFrame {
            channel: "runtime.codex.worker.events.desktop".to_string(),
            frame_seq: 9,
            kind: SyncFrameKind::TransactionBatch as i32,
            payload_bytes: vec![4, 5, 6],
            schema_version: 2,
        };
        let domain = SyncStreamFrame::try_from(wire.clone()).expect("wire to domain should work");
        assert_eq!(domain.channel, "runtime.codex.worker.events.desktop");
        assert_eq!(domain.frame_seq, 9);

        let round_trip = WireSyncFrame::from(domain);
        assert_eq!(round_trip, wire);
    }

    #[test]
    fn sync_failure_class_mapping_is_typed_for_v2_error_codes() {
        let stale = sync_failure_class_from_i32(SyncErrorCode::StaleCursor as i32);
        assert!(matches!(
            stale,
            crate::domain::SyncFailureClass::StaleCursor
        ));
        let unknown = sync_failure_class_from_i32(9999);
        assert!(matches!(
            unknown,
            crate::domain::SyncFailureClass::Unknown(9999)
        ));
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
