//! Agent events.

use super::{AgentId, AgentState};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    StateChanged {
        from: AgentState,
        to: AgentState,
        timestamp: u64,
    },
    Connected {
        relays: Vec<String>,
        timestamp: u64,
    },
    Disconnected {
        relays: Vec<String>,
        reason: Option<String>,
        timestamp: u64,
    },
    JobReceived {
        job_id: String,
        kind: u16,
        customer: String,
        timestamp: u64,
    },
    JobStarted {
        job_id: String,
        timestamp: u64,
    },
    JobProgress {
        job_id: String,
        progress: f32,
        step: Option<String>,
        timestamp: u64,
    },
    JobCompleted {
        job_id: String,
        duration_ms: u64,
        result_summary: Option<String>,
        timestamp: u64,
    },
    JobFailed {
        job_id: String,
        error: String,
        retryable: bool,
        timestamp: u64,
    },
    PaymentReceived {
        job_id: String,
        amount_millisats: u64,
        method: String,
        timestamp: u64,
    },
    PaymentRequired {
        job_id: String,
        amount_millisats: u64,
        bolt11: String,
        timestamp: u64,
    },
    ToolInvoked {
        job_id: String,
        tool_name: String,
        permission_required: bool,
        timestamp: u64,
    },
    ToolCompleted {
        job_id: String,
        tool_name: String,
        success: bool,
        duration_ms: u64,
        timestamp: u64,
    },
    AgentMessage {
        from: AgentId,
        message_type: String,
        content: String,
        timestamp: u64,
    },
    Error {
        message: String,
        code: Option<String>,
        context: Option<String>,
        timestamp: u64,
    },
    Warning {
        message: String,
        context: Option<String>,
        timestamp: u64,
    },
    Metrics {
        jobs_completed: u64,
        jobs_failed: u64,
        total_earnings_millisats: u64,
        memory_usage: Option<u64>,
        timestamp: u64,
    },
}

impl AgentEvent {
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

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            AgentEvent::Error { .. } | AgentEvent::JobFailed { .. }
        )
    }

    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn state_changed(from: AgentState, to: AgentState) -> Self {
        Self::StateChanged {
            from,
            to,
            timestamp: Self::now(),
        }
    }

    pub fn job_started(job_id: impl Into<String>) -> Self {
        Self::JobStarted {
            job_id: job_id.into(),
            timestamp: Self::now(),
        }
    }

    pub fn job_progress(job_id: impl Into<String>, progress: f32, step: Option<String>) -> Self {
        Self::JobProgress {
            job_id: job_id.into(),
            progress,
            step,
            timestamp: Self::now(),
        }
    }

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

    pub fn job_failed(
        job_id: impl Into<String>,
        error: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::JobFailed {
            job_id: job_id.into(),
            error: error.into(),
            retryable,
            timestamp: Self::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: None,
            context: None,
            timestamp: Self::now(),
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl AgentEvent {
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
}
