//! Tick result and resource usage types.

use crate::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Result of a tick execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResult {
    /// Whether tick completed successfully.
    pub success: bool,

    /// Duration of tick execution.
    pub duration: Duration,

    /// Resources consumed.
    pub usage: ResourceUsage,

    /// Messages sent during tick.
    pub messages_sent: usize,

    /// Events emitted during tick.
    pub events_emitted: usize,

    /// Next scheduled alarm (if any).
    pub next_alarm: Option<Timestamp>,

    /// Error if tick failed.
    pub error: Option<String>,

    /// Agent should hibernate after this tick.
    pub should_hibernate: bool,
}

impl TickResult {
    /// Create a successful tick result with default counters.
    pub fn success() -> Self {
        Self {
            success: true,
            ..Default::default()
        }
    }
}

impl Default for TickResult {
    fn default() -> Self {
        Self {
            success: true,
            duration: Duration::from_millis(0),
            usage: ResourceUsage::default(),
            messages_sent: 0,
            events_emitted: 0,
            next_alarm: None,
            error: None,
            should_hibernate: false,
        }
    }
}

/// Resource usage for a tick.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    /// Compute time in milliseconds.
    pub compute_ms: u64,
    /// Number of storage reads.
    pub storage_reads: u64,
    /// Number of storage writes.
    pub storage_writes: u64,
    /// Total bytes written to storage.
    pub storage_bytes_written: u64,
    /// Number of messages sent.
    pub messages_sent: u64,
    /// Number of external API calls.
    pub api_calls: u64,
}
