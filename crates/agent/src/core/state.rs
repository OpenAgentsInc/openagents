//! Agent runtime state.
//!
//! State represents the current operational status of an agent.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Current state of an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentState {
    /// Agent is not running.
    Idle,

    /// Agent is starting up.
    Starting {
        /// Start timestamp.
        #[serde(skip)]
        started_at: Option<Instant>,
    },

    /// Agent is online and ready to accept jobs.
    Online {
        /// Connected Nostr relays.
        #[serde(default)]
        relays: Vec<String>,
        /// Number of active sessions.
        #[serde(default)]
        active_sessions: u32,
    },

    /// Agent is currently processing a job.
    Working {
        /// Current job ID.
        job_id: String,
        /// Progress (0.0 - 1.0).
        progress: f32,
        /// Current step description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<String>,
    },

    /// Agent is paused (not accepting new jobs).
    Paused {
        /// Reason for pause.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// Agent is shutting down.
    ShuttingDown {
        /// Reason for shutdown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// Agent encountered an error.
    Error {
        /// Error message.
        message: String,
        /// Whether the error is recoverable.
        #[serde(default)]
        recoverable: bool,
    },
}

impl Default for AgentState {
    fn default() -> Self {
        Self::Idle
    }
}

impl AgentState {
    /// Check if the agent is available to accept jobs.
    pub fn is_available(&self) -> bool {
        matches!(self, AgentState::Online { .. })
    }

    /// Check if the agent is currently working.
    pub fn is_working(&self) -> bool {
        matches!(self, AgentState::Working { .. })
    }

    /// Check if the agent is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self, AgentState::Error { .. })
    }

    /// Check if the agent is idle.
    pub fn is_idle(&self) -> bool {
        matches!(self, AgentState::Idle)
    }

    /// Get the current job ID if working.
    pub fn current_job(&self) -> Option<&str> {
        match self {
            AgentState::Working { job_id, .. } => Some(job_id),
            _ => None,
        }
    }

    /// Get the current progress if working.
    pub fn progress(&self) -> Option<f32> {
        match self {
            AgentState::Working { progress, .. } => Some(*progress),
            _ => None,
        }
    }

    /// Create an online state.
    pub fn online(relays: Vec<String>) -> Self {
        Self::Online {
            relays,
            active_sessions: 0,
        }
    }

    /// Create a working state.
    pub fn working(job_id: impl Into<String>) -> Self {
        Self::Working {
            job_id: job_id.into(),
            progress: 0.0,
            step: None,
        }
    }

    /// Create an error state.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            recoverable: false,
        }
    }

    /// Create a recoverable error state.
    pub fn recoverable_error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            recoverable: true,
        }
    }
}

/// Statistics about agent operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    /// Total jobs completed.
    pub jobs_completed: u64,
    /// Total jobs failed.
    pub jobs_failed: u64,
    /// Total earnings in millisats.
    pub total_earnings_millisats: u64,
    /// Average job duration in milliseconds.
    pub avg_job_duration_ms: u64,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Current session count.
    pub active_sessions: u32,
}

impl AgentStats {
    /// Calculate success rate.
    pub fn success_rate(&self) -> f64 {
        let total = self.jobs_completed + self.jobs_failed;
        if total == 0 {
            1.0
        } else {
            self.jobs_completed as f64 / total as f64
        }
    }

    /// Record a completed job.
    pub fn record_completion(&mut self, duration_ms: u64, earnings_millisats: u64) {
        self.jobs_completed += 1;
        self.total_earnings_millisats += earnings_millisats;

        // Update running average
        let total = self.jobs_completed + self.jobs_failed;
        self.avg_job_duration_ms =
            (self.avg_job_duration_ms * (total - 1) + duration_ms) / total;
    }

    /// Record a failed job.
    pub fn record_failure(&mut self) {
        self.jobs_failed += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_checks() {
        let idle = AgentState::Idle;
        assert!(idle.is_idle());
        assert!(!idle.is_available());

        let online = AgentState::online(vec!["wss://relay.damus.io".into()]);
        assert!(online.is_available());
        assert!(!online.is_working());

        let working = AgentState::working("job123");
        assert!(working.is_working());
        assert!(!working.is_available());
        assert_eq!(working.current_job(), Some("job123"));

        let error = AgentState::error("something went wrong");
        assert!(error.is_error());
    }

    #[test]
    fn test_agent_stats() {
        let mut stats = AgentStats::default();

        stats.record_completion(1000, 10000);
        stats.record_completion(2000, 20000);
        stats.record_failure();

        assert_eq!(stats.jobs_completed, 2);
        assert_eq!(stats.jobs_failed, 1);
        assert_eq!(stats.total_earnings_millisats, 30000);
        assert!((stats.success_rate() - 0.666).abs() < 0.01);
    }
}
