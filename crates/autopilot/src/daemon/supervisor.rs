//! Worker supervisor - spawns, monitors, and restarts the autopilot worker

use crate::daemon::config::{DaemonConfig, WorkerCommand};
use crate::daemon::memory::{MemoryMonitor, MemoryStatus};
use crate::daemon::state::{WorkerState, WorkerStatus};
use anyhow::Result;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Supervisor that manages the worker process
pub struct WorkerSupervisor {
    config: DaemonConfig,
    state: WorkerState,
    memory_monitor: MemoryMonitor,
    child: Option<Child>,
    /// Process group ID for killing all children
    pgid: Option<u32>,
}

impl WorkerSupervisor {
    /// Create a new supervisor
    pub fn new(config: DaemonConfig) -> Self {
        let memory_config = config.memory.clone();
        Self {
            config,
            state: WorkerState::new(),
            memory_monitor: MemoryMonitor::new(memory_config),
            child: None,
            pgid: None,
        }
    }

    /// Get current worker state
    pub fn state(&self) -> &WorkerState {
        &self.state
    }

    /// Get daemon config
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Spawn the worker process
    pub fn spawn_worker(&mut self) -> Result<()> {
        if self.child.is_some() {
            anyhow::bail!("Worker already running");
        }

        let mut cmd = self.build_command();

        // Inherit environment
        cmd.env("AUTOPILOTD_SUPERVISED", "1");

        // Inherit stderr so we can see worker errors, pipe stdout
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        // Create new process group on Unix
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        eprintln!("Spawning worker...");
        let child = cmd.spawn()?;
        let pid = child.id();

        eprintln!("Worker started with PID {}", pid);

        self.pgid = Some(pid);
        self.child = Some(child);
        self.state.record_start(pid);

        Ok(())
    }

    /// Build the command to spawn the worker
    fn build_command(&self) -> Command {
        match &self.config.worker_command {
            WorkerCommand::Cargo { manifest_path } => {
                let mut cmd = Command::new("cargo");
                cmd.arg("run");
                cmd.arg("-p").arg("autopilot");
                cmd.arg("--bin").arg("autopilot");
                cmd.arg("--");
                cmd.arg("run");
                cmd.arg("--full-auto");
                cmd.arg("--with-issues");
                cmd.arg("--model").arg(&self.config.model);
                // Only pass budget if explicitly set (> 0)
                if self.config.max_budget > 0.0 {
                    cmd.arg("--max-budget")
                        .arg(self.config.max_budget.to_string());
                }
                cmd.arg("--max-turns")
                    .arg(self.config.max_turns.to_string());

                if let Some(project) = &self.config.project {
                    cmd.arg("--project").arg(project);
                }

                if let Some(manifest) = manifest_path {
                    cmd.arg("--manifest-path").arg(manifest);
                }

                // The initial prompt
                cmd.arg("Call issue_ready NOW to get the first issue and begin working.");

                cmd.current_dir(&self.config.working_dir);
                cmd
            }
            WorkerCommand::Binary { path } => {
                let mut cmd = Command::new(path);
                cmd.arg("run");
                cmd.arg("--full-auto");
                cmd.arg("--with-issues");
                cmd.arg("--model").arg(&self.config.model);
                // Only pass budget if explicitly set (> 0)
                if self.config.max_budget > 0.0 {
                    cmd.arg("--max-budget")
                        .arg(self.config.max_budget.to_string());
                }
                cmd.arg("--max-turns")
                    .arg(self.config.max_turns.to_string());

                if let Some(project) = &self.config.project {
                    cmd.arg("--project").arg(project);
                }

                cmd.arg("Call issue_ready NOW to get the first issue and begin working.");

                cmd.current_dir(&self.config.working_dir);
                cmd
            }
        }
    }

    /// Check if worker is still running and handle exit
    pub fn check_worker(&mut self) -> Option<ExitStatus> {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    eprintln!("Worker exited with status: {:?}", status);
                    self.child = None;
                    self.pgid = None;
                    Some(status)
                }
                Ok(None) => None, // Still running
                Err(e) => {
                    eprintln!("Error checking worker: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Handle worker exit
    pub fn handle_exit(&mut self, status: ExitStatus) {
        let success_threshold = Duration::from_millis(self.config.restart.success_threshold_ms);

        if status.success() {
            self.state.record_clean_exit(success_threshold);
            eprintln!("Worker exited cleanly");
        } else {
            // Check if we ran long enough to reset backoff
            if let WorkerStatus::Running { started_at, .. } = self.state.status {
                if started_at.elapsed() >= success_threshold {
                    self.state.reset_backoff();
                }
            }

            self.state.record_failure(
                self.config.restart.backoff_multiplier,
                self.config.restart.max_backoff_ms,
            );

            if self
                .state
                .can_restart(self.config.restart.max_consecutive_restarts)
            {
                let backoff = self.state.current_backoff;
                eprintln!(
                    "Worker crashed (attempt {}), restarting in {:?}",
                    self.state.consecutive_failures, backoff
                );
                self.state.status = WorkerStatus::Restarting {
                    attempt: self.state.consecutive_failures,
                    next_attempt_at: Instant::now() + backoff,
                };
            } else {
                let reason = format!(
                    "Max consecutive restarts ({}) exceeded",
                    self.config.restart.max_consecutive_restarts
                );
                eprintln!("{}", reason);
                self.state.status = WorkerStatus::Failed {
                    reason,
                    failed_at: Instant::now(),
                };
            }
        }
    }

    /// Check if worker is stalled (no log activity for too long)
    /// Returns true if worker was stalled and killed
    pub fn check_stall(&mut self) -> bool {
        if !self.state.is_running() {
            return false;
        }

        // Grace period: don't check for stalls in the first 2 minutes after worker start
        // This allows time for compilation and initial setup
        let grace_period = Duration::from_secs(120);
        if let Some(uptime) = self.state.uptime() {
            if uptime < grace_period {
                return false;
            }
        }

        let stall_timeout = Duration::from_millis(self.config.restart.stall_timeout_ms);

        // Get worker start time
        let worker_started_at = match &self.state.status {
            WorkerStatus::Running { started_at, .. } => *started_at,
            _ => return false,
        };

        // Find the latest log file
        if let Some(log_path) = self.find_latest_log() {
            if let Ok(metadata) = std::fs::metadata(&log_path) {
                if let Ok(modified) = metadata.modified() {
                    // Convert SystemTime to Instant for comparison
                    // If the log file was modified before the worker started,
                    // it's from a previous worker session - skip stall check
                    if let Ok(elapsed_since_modified) = modified.elapsed() {
                        let worker_uptime = worker_started_at.elapsed();

                        // If log file is older than worker start, worker hasn't created its log yet
                        if elapsed_since_modified > worker_uptime {
                            // Log file is from previous worker, skip stall check
                            return false;
                        }

                        // Log file is from current worker, check if stalled
                        if elapsed_since_modified > stall_timeout {
                            eprintln!(
                                "Worker stalled: no log activity for {:?} (log: {:?})",
                                elapsed_since_modified, log_path
                            );
                            self.stop_worker();
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Find the latest .rlog file in docs/logs/
    fn find_latest_log(&self) -> Option<std::path::PathBuf> {
        let logs_dir = self.config.working_dir.join("docs").join("logs");

        // Find today's directory (or most recent)
        let mut dirs: Vec<_> = std::fs::read_dir(&logs_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        dirs.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

        let latest_dir = dirs.first()?.path();

        // Find the most recently modified .rlog file
        let mut logs: Vec<_> = std::fs::read_dir(&latest_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "rlog")
                    .unwrap_or(false)
            })
            .collect();

        logs.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        logs.first().map(|e| e.path())
    }

    /// Check memory and take action if needed
    /// Returns true if worker was force-restarted
    pub fn check_memory(&mut self) -> bool {
        let status = self.memory_monitor.check();

        match status {
            MemoryStatus::Ok(_) => false,
            MemoryStatus::Low(available) => {
                eprintln!(
                    "Memory low: {} available, attempting cleanup",
                    format_bytes(available)
                );
                // Get worker PID to protect from kill
                let exclude_pids = if let Some(ref child) = self.child {
                    vec![child.id()]
                } else {
                    vec![]
                };
                self.memory_monitor.kill_memory_hogs(&exclude_pids);
                false
            }
            MemoryStatus::Critical(available) => {
                eprintln!(
                    "Memory critical: {} available, force restarting worker",
                    format_bytes(available)
                );
                // Kill memory hogs first (exclude worker PID)
                let exclude_pids = if let Some(ref child) = self.child {
                    vec![child.id()]
                } else {
                    vec![]
                };
                self.memory_monitor.kill_memory_hogs(&exclude_pids);

                // Check again
                let new_available = self.memory_monitor.available_memory();
                if new_available < self.config.memory.critical_threshold_bytes {
                    // Still critical, kill worker
                    self.stop_worker();
                    true
                } else {
                    eprintln!("Memory recovered to {}", format_bytes(new_available));
                    false
                }
            }
        }
    }

    /// Stop the worker gracefully
    pub fn stop_worker(&mut self) {
        if let Some(ref mut child) = self.child {
            self.state.status = WorkerStatus::Stopping;

            // First try SIGTERM via process group
            #[cfg(unix)]
            if let Some(pgid) = self.pgid {
                eprintln!("Sending SIGTERM to process group {}", pgid);
                self.memory_monitor.kill_process_group(pgid);
            }

            // Give it time to exit gracefully
            std::thread::sleep(Duration::from_secs(5));

            // Check if it exited
            match child.try_wait() {
                Ok(Some(_)) => {
                    eprintln!("Worker stopped gracefully");
                }
                Ok(None) => {
                    // Still running, force kill
                    eprintln!("Worker didn't stop, sending SIGKILL");
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(e) => {
                    eprintln!("Error waiting for worker: {}", e);
                }
            }

            self.child = None;
            self.pgid = None;
            self.state.status = WorkerStatus::Stopped;
        }
    }

    /// Force restart the worker (stop then start)
    pub fn restart_worker(&mut self) -> Result<()> {
        self.stop_worker();
        std::thread::sleep(Duration::from_secs(1));
        self.spawn_worker()
    }

    /// Get metrics for status reporting
    pub fn get_metrics(&mut self) -> DaemonMetrics {
        DaemonMetrics {
            worker_status: self.state.status.as_str().to_string(),
            worker_pid: self.state.pid(),
            uptime_seconds: self.state.uptime().map(|d| d.as_secs()).unwrap_or(0),
            total_restarts: self.state.total_restarts,
            consecutive_failures: self.state.consecutive_failures,
            memory_available_bytes: self.memory_monitor.available_memory(),
            memory_total_bytes: self.memory_monitor.total_memory(),
        }
    }

    /// Run the main monitoring loop
    ///
    /// If `shared_metrics` is provided, it will be updated on each poll iteration
    /// so that control socket can read metrics without blocking this loop.
    pub async fn run(
        &mut self,
        mut shutdown_rx: mpsc::Receiver<()>,
        shared_metrics: Option<SharedMetrics>,
    ) -> Result<()> {
        let poll_interval = Duration::from_millis(self.config.memory.poll_interval_ms);

        // Spawn worker on start
        self.spawn_worker()?;

        // Update shared metrics initially
        if let Some(ref metrics) = shared_metrics {
            if let Ok(mut guard) = metrics.write() {
                *guard = self.get_metrics();
            }
        }

        loop {
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {
                    // Check if worker exited
                    if let Some(status) = self.check_worker() {
                        self.handle_exit(status);

                        // If we should restart, do it
                        if let WorkerStatus::Restarting { next_attempt_at, .. } = self.state.status {
                            let delay = next_attempt_at.saturating_duration_since(Instant::now());
                            if delay > Duration::ZERO {
                                tokio::time::sleep(delay).await;
                            }
                            if let Err(e) = self.spawn_worker() {
                                eprintln!("Failed to restart worker: {}", e);
                            }
                        }
                    }

                    // Check memory
                    if self.check_memory() {
                        // Worker was killed due to memory, restart it
                        std::thread::sleep(Duration::from_secs(2));
                        if let Err(e) = self.spawn_worker() {
                            eprintln!("Failed to restart worker after memory cleanup: {}", e);
                        }
                    }

                    // Check for stalled worker (no log activity)
                    if self.check_stall() {
                        // Worker was killed due to stall, restart it
                        eprintln!("Restarting stalled worker...");
                        std::thread::sleep(Duration::from_secs(2));
                        if let Err(e) = self.spawn_worker() {
                            eprintln!("Failed to restart stalled worker: {}", e);
                        }
                    }

                    // Check if we're in failed state - recover after cooldown
                    if let WorkerStatus::Failed { ref reason, failed_at } = self.state.status {
                        let failed_duration = failed_at.elapsed();
                        let recovery_cooldown = Duration::from_millis(self.config.restart.recovery_cooldown_ms);

                        if failed_duration >= recovery_cooldown {
                            eprintln!("Worker failed state cooldown expired ({:?}), resetting and retrying...", failed_duration);
                            self.state.consecutive_failures = 0;
                            self.state.current_backoff = Duration::from_secs(1);
                            self.state.status = WorkerStatus::Stopped;

                            if let Err(e) = self.spawn_worker() {
                                eprintln!("Failed to restart worker after cooldown: {}", e);
                            }
                        } else {
                            let remaining = recovery_cooldown - failed_duration;
                            eprintln!("Worker in failed state: {} (retry in {:?})", reason, remaining);
                        }
                    }

                    // Update shared metrics for control socket
                    if let Some(ref metrics) = shared_metrics {
                        if let Ok(mut guard) = metrics.write() {
                            *guard = self.get_metrics();
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    eprintln!("Shutdown signal received");
                    self.stop_worker();
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Metrics for daemon status
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DaemonMetrics {
    pub worker_status: String,
    pub worker_pid: Option<u32>,
    pub uptime_seconds: u64,
    pub total_restarts: u64,
    pub consecutive_failures: u32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
}

/// Shared metrics that can be read without locking the supervisor
pub type SharedMetrics = std::sync::Arc<std::sync::RwLock<DaemonMetrics>>;

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}
