//! Daemon-specific metrics collection and monitoring
//!
//! Tracks daemon and worker behavior for performance tuning and debugging.

use crate::daemon::state::{WorkerState, WorkerStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Metrics collected for the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonMetrics {
    /// When metrics collection started
    pub started_at: DateTime<Utc>,
    /// Total number of worker restarts
    pub total_restarts: u64,
    /// Restart reasons (crash, memory, manual)
    pub restart_reasons: RestartReasons,
    /// Current memory usage (MB)
    pub current_memory_mb: f64,
    /// Peak memory usage (MB)
    pub peak_memory_mb: f64,
    /// Worker uptime (seconds)
    pub worker_uptime_seconds: f64,
    /// Total daemon uptime (seconds)
    pub daemon_uptime_seconds: f64,
    /// Recent restart history (last 10)
    pub recent_restarts: Vec<RestartEvent>,
}

/// Categorized restart reasons
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RestartReasons {
    pub crash: u64,
    pub memory_pressure: u64,
    pub manual: u64,
    pub timeout: u64,
}

/// Individual restart event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartEvent {
    pub timestamp: DateTime<Utc>,
    pub reason: String,
    pub uptime_seconds: f64,
    pub memory_mb: Option<f64>,
}

/// Metrics collector for daemon operations
pub struct DaemonMetricsCollector {
    started_at: DateTime<Utc>,
    daemon_started: Instant,
    restart_reasons: Arc<RwLock<RestartReasons>>,
    recent_restarts: Arc<RwLock<VecDeque<RestartEvent>>>,
    peak_memory_mb: Arc<RwLock<f64>>,
}

impl DaemonMetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            started_at: Utc::now(),
            daemon_started: Instant::now(),
            restart_reasons: Arc::new(RwLock::new(RestartReasons::default())),
            recent_restarts: Arc::new(RwLock::new(VecDeque::with_capacity(10))),
            peak_memory_mb: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Record a worker restart
    pub fn record_restart(&self, reason: &str, uptime: Duration, memory_mb: Option<f64>) {
        let mut reasons = self.restart_reasons.write().unwrap();

        // Categorize restart reason
        if reason.contains("crash") || reason.contains("failed") {
            reasons.crash += 1;
        } else if reason.contains("memory") {
            reasons.memory_pressure += 1;
        } else if reason.contains("manual") || reason.contains("requested") {
            reasons.manual += 1;
        } else if reason.contains("timeout") || reason.contains("stall") {
            reasons.timeout += 1;
        } else {
            reasons.crash += 1; // Default to crash
        }

        // Add to recent restarts
        let event = RestartEvent {
            timestamp: Utc::now(),
            reason: reason.to_string(),
            uptime_seconds: uptime.as_secs_f64(),
            memory_mb,
        };

        let mut recent = self.recent_restarts.write().unwrap();
        if recent.len() >= 10 {
            recent.pop_front();
        }
        recent.push_back(event);
    }

    /// Update memory tracking
    pub fn update_memory(&self, current_mb: f64) {
        let mut peak = self.peak_memory_mb.write().unwrap();
        if current_mb > *peak {
            *peak = current_mb;
        }
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self, worker_state: &WorkerState, current_memory_mb: f64) -> DaemonMetrics {
        let reasons = self.restart_reasons.read().unwrap();
        let recent = self.recent_restarts.read().unwrap();
        let peak = *self.peak_memory_mb.read().unwrap();

        let worker_uptime_seconds = match &worker_state.status {
            WorkerStatus::Running { started_at, .. } => started_at.elapsed().as_secs_f64(),
            _ => 0.0,
        };

        DaemonMetrics {
            started_at: self.started_at,
            total_restarts: worker_state.total_restarts,
            restart_reasons: reasons.clone(),
            current_memory_mb,
            peak_memory_mb: peak,
            worker_uptime_seconds,
            daemon_uptime_seconds: self.daemon_started.elapsed().as_secs_f64(),
            recent_restarts: recent.iter().cloned().collect(),
        }
    }

    /// Get total restart count
    pub fn total_restarts(&self) -> u64 {
        let reasons = self.restart_reasons.read().unwrap();
        reasons.crash + reasons.memory_pressure + reasons.manual + reasons.timeout
    }
}

impl Default for DaemonMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = DaemonMetricsCollector::new();
        assert_eq!(collector.total_restarts(), 0);
    }

    #[test]
    fn test_record_restart_crash() {
        let collector = DaemonMetricsCollector::new();

        collector.record_restart("worker crash", Duration::from_secs(60), Some(512.0));

        let reasons = collector.restart_reasons.read().unwrap();
        assert_eq!(reasons.crash, 1);
        assert_eq!(reasons.memory_pressure, 0);
        assert_eq!(reasons.manual, 0);

        let recent = collector.recent_restarts.read().unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].uptime_seconds, 60.0);
        assert_eq!(recent[0].memory_mb, Some(512.0));
    }

    #[test]
    fn test_record_restart_memory() {
        let collector = DaemonMetricsCollector::new();

        collector.record_restart("memory pressure", Duration::from_secs(120), Some(1024.0));

        let reasons = collector.restart_reasons.read().unwrap();
        assert_eq!(reasons.crash, 0);
        assert_eq!(reasons.memory_pressure, 1);
    }

    #[test]
    fn test_record_restart_manual() {
        let collector = DaemonMetricsCollector::new();

        collector.record_restart("manual restart requested", Duration::from_secs(300), None);

        let reasons = collector.restart_reasons.read().unwrap();
        assert_eq!(reasons.manual, 1);
    }

    #[test]
    fn test_recent_restarts_limit() {
        let collector = DaemonMetricsCollector::new();

        // Add 15 restarts
        for i in 0..15 {
            collector.record_restart("crash", Duration::from_secs(60), Some(i as f64));
        }

        let recent = collector.recent_restarts.read().unwrap();
        assert_eq!(recent.len(), 10); // Should only keep last 10

        // Verify it's the most recent 10
        assert_eq!(recent[0].memory_mb, Some(5.0)); // 6th restart
        assert_eq!(recent[9].memory_mb, Some(14.0)); // 15th restart
    }

    #[test]
    fn test_update_memory() {
        let collector = DaemonMetricsCollector::new();

        collector.update_memory(512.0);
        assert_eq!(*collector.peak_memory_mb.read().unwrap(), 512.0);

        collector.update_memory(256.0); // Lower - shouldn't update peak
        assert_eq!(*collector.peak_memory_mb.read().unwrap(), 512.0);

        collector.update_memory(1024.0); // Higher - should update peak
        assert_eq!(*collector.peak_memory_mb.read().unwrap(), 1024.0);
    }

    #[test]
    fn test_snapshot() {
        let collector = DaemonMetricsCollector::new();
        let worker_state = WorkerState::new();

        collector.record_restart("crash", Duration::from_secs(60), Some(512.0));
        collector.update_memory(768.0);

        let snapshot = collector.snapshot(&worker_state, 512.0);

        assert_eq!(snapshot.total_restarts, 0); // WorkerState hasn't been updated
        assert_eq!(snapshot.restart_reasons.crash, 1);
        assert_eq!(snapshot.current_memory_mb, 512.0);
        assert_eq!(snapshot.peak_memory_mb, 768.0);
        assert_eq!(snapshot.recent_restarts.len(), 1);
    }
}
