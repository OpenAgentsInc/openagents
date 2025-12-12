//! Agent events.
//!
//! Events are emitted by agents during their lifecycle and can be
//! subscribed to for monitoring, logging, and reactive programming.

use super::{AgentId, AgentState};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Events emitted by an agent during operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent state changed.
    StateChanged {
        /// Previous state.
        from: AgentState,
        /// New state.
        to: AgentState,
        /// Timestamp.
        timestamp: u64,
    },

    /// Agent connected to relay(s).
    Connected {
        /// Connected relays.
        relays: Vec<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Agent disconnected from relay(s).
    Disconnected {
        /// Disconnected relays.
        relays: Vec<String>,
        /// Reason.
        reason: Option<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Job received.
    JobReceived {
        /// Job ID.
        job_id: String,
        /// Job kind (NIP-90).
        kind: u16,
        /// Customer's public key.
        customer: String,
        /// Timestamp.
        timestamp: u64,
    },

    /// Job started.
    JobStarted {
        /// Job ID.
        job_id: String,
        /// Timestamp.
        timestamp: u64,
    },

    /// Job progress update.
    JobProgress {
        /// Job ID.
        job_id: String,
        /// Progress (0.0 - 1.0).
        progress: f32,
        /// Current step description.
        step: Option<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Job completed successfully.
    JobCompleted {
        /// Job ID.
        job_id: String,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Result summary.
        result_summary: Option<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Job failed.
    JobFailed {
        /// Job ID.
        job_id: String,
        /// Error message.
        error: String,
        /// Whether the error is retryable.
        retryable: bool,
        /// Timestamp.
        timestamp: u64,
    },

    /// Payment received.
    PaymentReceived {
        /// Job ID.
        job_id: String,
        /// Amount in millisats.
        amount_millisats: u64,
        /// Payment method.
        method: String,
        /// Timestamp.
        timestamp: u64,
    },

    /// Payment required (sent to customer).
    PaymentRequired {
        /// Job ID.
        job_id: String,
        /// Amount in millisats.
        amount_millisats: u64,
        /// BOLT11 invoice.
        bolt11: String,
        /// Timestamp.
        timestamp: u64,
    },

    /// Tool invoked.
    ToolInvoked {
        /// Job ID.
        job_id: String,
        /// Tool name.
        tool_name: String,
        /// Whether permission was required.
        permission_required: bool,
        /// Timestamp.
        timestamp: u64,
    },

    /// Tool completed.
    ToolCompleted {
        /// Job ID.
        job_id: String,
        /// Tool name.
        tool_name: String,
        /// Success/failure.
        success: bool,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Timestamp.
        timestamp: u64,
    },

    /// Message from another agent.
    AgentMessage {
        /// Sender agent ID.
        from: AgentId,
        /// Message type.
        message_type: String,
        /// Message content.
        content: String,
        /// Timestamp.
        timestamp: u64,
    },

    /// Error occurred.
    Error {
        /// Error message.
        message: String,
        /// Error code.
        code: Option<String>,
        /// Context (job_id, etc).
        context: Option<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Warning.
    Warning {
        /// Warning message.
        message: String,
        /// Context.
        context: Option<String>,
        /// Timestamp.
        timestamp: u64,
    },

    /// Metrics update.
    Metrics {
        /// Jobs completed.
        jobs_completed: u64,
        /// Jobs failed.
        jobs_failed: u64,
        /// Total earnings in millisats.
        total_earnings_millisats: u64,
        /// Current memory usage in bytes.
        memory_usage: Option<u64>,
        /// Timestamp.
        timestamp: u64,
    },
}

impl AgentEvent {
    /// Get the timestamp of the event.
    pub fn timestamp(&self) -> u64 {
        match self {
            AgentEvent::StateChanged { timestamp, .. } => *timestamp,
            AgentEvent::Connected { timestamp, .. } => *timestamp,
            AgentEvent::Disconnected { timestamp, .. } => *timestamp,
            AgentEvent::JobReceived { timestamp, .. } => *timestamp,
            AgentEvent::JobStarted { timestamp, .. } => *timestamp,
            AgentEvent::JobProgress { timestamp, .. } => *timestamp,
            AgentEvent::JobCompleted { timestamp, .. } => *timestamp,
            AgentEvent::JobFailed { timestamp, .. } => *timestamp,
            AgentEvent::PaymentReceived { timestamp, .. } => *timestamp,
            AgentEvent::PaymentRequired { timestamp, .. } => *timestamp,
            AgentEvent::ToolInvoked { timestamp, .. } => *timestamp,
            AgentEvent::ToolCompleted { timestamp, .. } => *timestamp,
            AgentEvent::AgentMessage { timestamp, .. } => *timestamp,
            AgentEvent::Error { timestamp, .. } => *timestamp,
            AgentEvent::Warning { timestamp, .. } => *timestamp,
            AgentEvent::Metrics { timestamp, .. } => *timestamp,
        }
    }

    /// Get the job ID if this event is job-related.
    pub fn job_id(&self) -> Option<&str> {
        match self {
            AgentEvent::JobReceived { job_id, .. } => Some(job_id),
            AgentEvent::JobStarted { job_id, .. } => Some(job_id),
            AgentEvent::JobProgress { job_id, .. } => Some(job_id),
            AgentEvent::JobCompleted { job_id, .. } => Some(job_id),
            AgentEvent::JobFailed { job_id, .. } => Some(job_id),
            AgentEvent::PaymentReceived { job_id, .. } => Some(job_id),
            AgentEvent::PaymentRequired { job_id, .. } => Some(job_id),
            AgentEvent::ToolInvoked { job_id, .. } => Some(job_id),
            AgentEvent::ToolCompleted { job_id, .. } => Some(job_id),
            _ => None,
        }
    }

    /// Check if this is an error event.
    pub fn is_error(&self) -> bool {
        matches!(self, AgentEvent::Error { .. } | AgentEvent::JobFailed { .. })
    }

    /// Get current timestamp in seconds since Unix epoch.
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    // Convenience constructors

    /// Create a state changed event.
    pub fn state_changed(from: AgentState, to: AgentState) -> Self {
        Self::StateChanged {
            from,
            to,
            timestamp: Self::now(),
        }
    }

    /// Create a job started event.
    pub fn job_started(job_id: impl Into<String>) -> Self {
        Self::JobStarted {
            job_id: job_id.into(),
            timestamp: Self::now(),
        }
    }

    /// Create a job progress event.
    pub fn job_progress(job_id: impl Into<String>, progress: f32, step: Option<String>) -> Self {
        Self::JobProgress {
            job_id: job_id.into(),
            progress,
            step,
            timestamp: Self::now(),
        }
    }

    /// Create a job completed event.
    pub fn job_completed(
        job_id: impl Into<String>,
        duration_ms: u64,
        result_summary: Option<String>,
    ) -> Self {
        Self::JobCompleted {
            job_id: job_id.into(),
            duration_ms,
            result_summary,
            timestamp: Self::now(),
        }
    }

    /// Create a job failed event.
    pub fn job_failed(job_id: impl Into<String>, error: impl Into<String>, retryable: bool) -> Self {
        Self::JobFailed {
            job_id: job_id.into(),
            error: error.into(),
            retryable,
            timestamp: Self::now(),
        }
    }

    /// Create an error event.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: None,
            context: None,
            timestamp: Self::now(),
        }
    }

    /// Create an error event with context.
    pub fn error_with_context(
        message: impl Into<String>,
        code: Option<String>,
        context: Option<String>,
    ) -> Self {
        Self::Error {
            message: message.into(),
            code,
            context,
            timestamp: Self::now(),
        }
    }
}

/// Severity level for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    /// Debug information.
    Debug,
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// Critical error.
    Critical,
}

impl AgentEvent {
    /// Get the severity of this event.
    pub fn severity(&self) -> EventSeverity {
        match self {
            AgentEvent::Error { .. } | AgentEvent::JobFailed { .. } => EventSeverity::Error,
            AgentEvent::Warning { .. } => EventSeverity::Warning,
            AgentEvent::StateChanged { .. }
            | AgentEvent::Connected { .. }
            | AgentEvent::Disconnected { .. }
            | AgentEvent::JobCompleted { .. }
            | AgentEvent::PaymentReceived { .. } => EventSeverity::Info,
            _ => EventSeverity::Debug,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = AgentEvent::job_started("job123");
        assert_eq!(event.job_id(), Some("job123"));
        assert!(!event.is_error());

        let error = AgentEvent::job_failed("job123", "something went wrong", true);
        assert!(error.is_error());
        assert_eq!(error.severity(), EventSeverity::Error);
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentEvent::job_progress("job123", 0.5, Some("Processing...".into()));

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("job_progress"));
        assert!(json.contains("job123"));

        let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.job_id(), Some("job123"));
    }
}
