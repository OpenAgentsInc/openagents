//! Envelope types for filesystem inbox/outbox.

use crate::types::{EnvelopeId, Timestamp};
use serde::{Deserialize, Serialize};

/// Minimal envelope for inbox storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    /// Envelope id.
    pub id: EnvelopeId,
    /// Timestamp the envelope was created.
    pub timestamp: Timestamp,
    /// Payload data.
    pub payload: serde_json::Value,
}
