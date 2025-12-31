//! Tick trigger definitions.

use crate::types::{AgentId, EnvelopeId, Timestamp};
use serde::{Deserialize, Serialize};

/// What caused an agent tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trigger {
    /// Incoming message from another agent or user.
    Message(MessageTrigger),

    /// Scheduled alarm fired.
    Alarm(AlarmTrigger),

    /// External event (webhook, file change, etc.).
    Event(EventTrigger),

    /// Manual invocation (API call, CLI).
    Manual(ManualTrigger),

    /// First tick after creation.
    Initialize(InitializeTrigger),
}

impl Trigger {
    /// Get envelope id for deduplication.
    pub fn envelope_id(&self) -> &EnvelopeId {
        match self {
            Trigger::Message(t) => &t.meta.envelope_id,
            Trigger::Alarm(t) => &t.meta.envelope_id,
            Trigger::Event(t) => &t.meta.envelope_id,
            Trigger::Manual(t) => &t.meta.envelope_id,
            Trigger::Initialize(t) => &t.meta.envelope_id,
        }
    }
}

/// Common metadata on all triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerMeta {
    /// Envelope id for idempotency.
    pub envelope_id: EnvelopeId,

    /// Source system (driver name, relay URL, etc.).
    pub source: String,

    /// Sequence number for ordering (if available).
    pub seq: Option<u64>,

    /// Timestamp when envelope was created.
    pub created_at: Timestamp,
}

/// Message trigger payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTrigger {
    /// Trigger metadata.
    pub meta: TriggerMeta,
    /// Sender agent id.
    pub from: AgentId,
    /// Message payload.
    pub message: serde_json::Value,
    /// Reply envelope id, if any.
    pub reply_to: Option<EnvelopeId>,
}

/// Alarm trigger payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmTrigger {
    /// Trigger metadata.
    pub meta: TriggerMeta,
    /// Alarm identifier.
    pub alarm_id: String,
    /// Scheduled timestamp.
    pub scheduled_at: Timestamp,
    /// Actual fired timestamp.
    pub fired_at: Timestamp,
    /// Optional payload.
    pub payload: Option<Vec<u8>>,
}

/// Event trigger payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTrigger {
    /// Trigger metadata.
    pub meta: TriggerMeta,
    /// Event type name.
    pub event_type: String,
    /// Event payload.
    pub payload: serde_json::Value,
}

/// Manual trigger payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTrigger {
    /// Trigger metadata.
    pub meta: TriggerMeta,
    /// Optional invoker identifier.
    pub invoked_by: Option<String>,
    /// Optional reason string.
    pub reason: Option<String>,
}

/// Initialize trigger payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeTrigger {
    /// Trigger metadata.
    pub meta: TriggerMeta,
}
