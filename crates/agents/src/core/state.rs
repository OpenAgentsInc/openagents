//! Agent runtime state.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Current state of an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentState {
    Idle,

    Starting {
        #[serde(skip)]
        started_at: Option<Instant>,
    },

    Online {
        #[serde(default)]
        relays: Vec<String>,
        #[serde(default)]
        active_sessions: u32,
    },

    Working {
        job_id: String,
        progress: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<String>,
    },

    Paused {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    ShuttingDown {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    Error {
        message: String,
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
    pub fn is_available(&self) -> bool {
        matches!(self, AgentState::Online { .. })
    }

    pub fn is_working(&self) -> bool {
        matches!(self, AgentState::Working { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, AgentState::Error { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, AgentState::Idle)
    }

    pub fn current_job(&self) -> Option<&str> {
        match self {
            AgentState::Working { job_id, .. } => Some(job_id),
            _ => None,
        }
    }

    pub fn progress(&self) -> Option<f32> {
        match self {
            AgentState::Working { progress, .. } => Some(*progress),
            _ => None,
        }
    }

    pub fn online(relays: Vec<String>) -> Self {
        Self::Online {
            relays,
            active_sessions: 0,
        }
    }

    pub fn working(job_id: impl Into<String>) -> Self {
        Self::Working {
            job_id: job_id.into(),
            progress: 0.0,
            step: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            recoverable: false,
        }
    }

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
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub total_earnings_millisats: u64,
    pub avg_job_duration_ms: u64,
    pub uptime_secs: u64,
    pub active_sessions: u32,
}

impl AgentStats {
    pub fn success_rate(&self) -> f64 {
        let total = self.jobs_completed + self.jobs_failed;
        if total == 0 {
            1.0
        } else {
            self.jobs_completed as f64 / total as f64
        }
    }

    pub fn record_completion(&mut self, duration_ms: u64, earnings_millisats: u64) {
        self.jobs_completed += 1;
        self.total_earnings_millisats += earnings_millisats;

        let total = self.jobs_completed + self.jobs_failed;
        self.avg_job_duration_ms =
            (self.avg_job_duration_ms * (total - 1) + duration_ms) / total;
    }

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
