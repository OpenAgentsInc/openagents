//! Worker state machine

use std::time::{Duration, Instant};

/// Current status of the worker process
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus {
    /// Worker is stopped
    Stopped,
    /// Worker is starting up
    Starting,
    /// Worker is running
    Running { pid: u32, started_at: Instant },
    /// Worker is being stopped
    Stopping,
    /// Worker is restarting after a failure
    Restarting { attempt: u32, next_attempt_at: Instant },
    /// Worker has failed too many times
    Failed { reason: String },
}

impl WorkerStatus {
    /// Get a human-readable status string
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerStatus::Stopped => "stopped",
            WorkerStatus::Starting => "starting",
            WorkerStatus::Running { .. } => "running",
            WorkerStatus::Stopping => "stopping",
            WorkerStatus::Restarting { .. } => "restarting",
            WorkerStatus::Failed { .. } => "failed",
        }
    }
}

/// State of the worker process
pub struct WorkerState {
    /// Current status
    pub status: WorkerStatus,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Current backoff duration
    pub current_backoff: Duration,
    /// Total number of restarts since daemon started
    pub total_restarts: u64,
    /// Time of last successful run (ran for at least success_threshold)
    pub last_successful_run: Option<Instant>,
}

impl WorkerState {
    /// Create a new worker state
    pub fn new() -> Self {
        Self {
            status: WorkerStatus::Stopped,
            consecutive_failures: 0,
            current_backoff: Duration::from_secs(1),
            total_restarts: 0,
            last_successful_run: None,
        }
    }

    /// Check if the worker is currently running
    pub fn is_running(&self) -> bool {
        matches!(self.status, WorkerStatus::Running { .. })
    }

    /// Check if the worker can be restarted
    pub fn can_restart(&self, max_consecutive: u32) -> bool {
        self.consecutive_failures < max_consecutive
    }

    /// Record a successful start
    pub fn record_start(&mut self, pid: u32) {
        self.status = WorkerStatus::Running {
            pid,
            started_at: Instant::now(),
        };
    }

    /// Record a failure and calculate next backoff
    pub fn record_failure(&mut self, backoff_multiplier: f64, max_backoff_ms: u64) {
        self.consecutive_failures += 1;
        self.total_restarts += 1;

        // Calculate next backoff with exponential increase
        let next_backoff_ms = (self.current_backoff.as_millis() as f64 * backoff_multiplier) as u64;
        self.current_backoff = Duration::from_millis(next_backoff_ms.min(max_backoff_ms));
    }

    /// Record a clean exit (successful run)
    pub fn record_clean_exit(&mut self, success_threshold: Duration) {
        if let WorkerStatus::Running { started_at, .. } = self.status {
            if started_at.elapsed() >= success_threshold {
                // Ran long enough to count as successful
                self.consecutive_failures = 0;
                self.current_backoff = Duration::from_secs(1);
                self.last_successful_run = Some(Instant::now());
            }
        }
        self.status = WorkerStatus::Stopped;
    }

    /// Reset failure counters (called after successful threshold)
    pub fn reset_backoff(&mut self) {
        self.consecutive_failures = 0;
        self.current_backoff = Duration::from_secs(1);
    }

    /// Get the worker PID if running
    pub fn pid(&self) -> Option<u32> {
        match &self.status {
            WorkerStatus::Running { pid, .. } => Some(*pid),
            _ => None,
        }
    }

    /// Get uptime if running
    pub fn uptime(&self) -> Option<Duration> {
        match &self.status {
            WorkerStatus::Running { started_at, .. } => Some(started_at.elapsed()),
            _ => None,
        }
    }
}

impl Default for WorkerState {
    fn default() -> Self {
        Self::new()
    }
}
