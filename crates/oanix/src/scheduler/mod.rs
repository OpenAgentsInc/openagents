//! Job Scheduler for OANIX environments
//!
//! Manages job execution across environments with priority-based scheduling,
//! concurrent execution limits, and job state tracking.
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::{Scheduler, JobSpec, JobKind, EnvBuilder, MemFs};
//!
//! // Create scheduler
//! let mut scheduler = Scheduler::new();
//!
//! // Create an environment
//! let env = EnvBuilder::new()
//!     .mount("/tmp", MemFs::new())
//!     .build()?;
//! let env_id = env.id();
//! scheduler.register_env(env);
//!
//! // Submit a job
//! let job = JobSpec::new(env_id, JobKind::Script {
//!     script: "echo hello".to_string(),
//! });
//! scheduler.submit(job)?;
//!
//! // Process jobs
//! scheduler.tick()?;
//! ```

mod job;

pub use job::{JobKind, JobSpec, JobStatus};

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::env::OanixEnv;
use crate::error::OanixError;

#[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
use crate::wasi::RunConfig;

/// Job scheduler for managing environment execution
///
/// Features:
/// - Priority-based job queue
/// - Concurrent execution limits
/// - Environment registry
/// - Job state tracking
pub struct Scheduler {
    /// Registered environments
    environments: HashMap<Uuid, Arc<RwLock<OanixEnv>>>,
    /// Pending jobs (priority queue)
    pending: VecDeque<JobSpec>,
    /// Currently running jobs
    running: HashMap<Uuid, JobSpec>,
    /// Completed jobs (for history)
    completed: Vec<JobSpec>,
    /// Maximum concurrent jobs
    max_concurrent: usize,
    /// Job results
    results: HashMap<Uuid, JobResult>,
}

/// Result of a completed job
#[derive(Debug, Clone)]
pub struct JobResult {
    /// Job ID
    pub job_id: Uuid,
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Error message if failed
    pub error: Option<String>,
    /// Completion timestamp
    pub completed_at: u64,
}

impl Scheduler {
    /// Create a new scheduler with default settings
    pub fn new() -> Self {
        Self {
            environments: HashMap::new(),
            pending: VecDeque::new(),
            running: HashMap::new(),
            completed: Vec::new(),
            max_concurrent: 4,
            results: HashMap::new(),
        }
    }

    /// Create with custom concurrency limit
    pub fn with_max_concurrent(max: usize) -> Self {
        Self {
            environments: HashMap::new(),
            pending: VecDeque::new(),
            running: HashMap::new(),
            completed: Vec::new(),
            max_concurrent: max,
            results: HashMap::new(),
        }
    }

    /// Register an environment with the scheduler
    pub fn register_env(&mut self, env: OanixEnv) -> Uuid {
        let id = env.id();
        self.environments.insert(id, Arc::new(RwLock::new(env)));
        id
    }

    /// Get an environment by ID
    pub fn get_env(&self, id: &Uuid) -> Option<Arc<RwLock<OanixEnv>>> {
        self.environments.get(id).cloned()
    }

    /// Remove an environment
    pub fn remove_env(&mut self, id: &Uuid) -> Option<Arc<RwLock<OanixEnv>>> {
        self.environments.remove(id)
    }

    /// Submit a job to the queue
    pub fn submit(&mut self, job: JobSpec) -> Result<Uuid, OanixError> {
        // Verify environment exists
        if !self.environments.contains_key(&job.env_id) {
            return Err(OanixError::Job(format!(
                "environment {} not found",
                job.env_id
            )));
        }

        let job_id = job.id;

        // Insert based on priority (higher priority first)
        let insert_pos = self
            .pending
            .iter()
            .position(|j| j.priority < job.priority)
            .unwrap_or(self.pending.len());

        self.pending.insert(insert_pos, job);

        Ok(job_id)
    }

    /// Get the next job to run (if under concurrency limit)
    pub fn next(&mut self) -> Option<JobSpec> {
        if self.running.len() >= self.max_concurrent {
            return None;
        }

        let mut job = self.pending.pop_front()?;
        job.status = JobStatus::Running {
            started_at: now(),
        };
        self.running.insert(job.id, job.clone());
        Some(job)
    }

    /// Mark a job as completed
    pub fn complete(&mut self, job_id: &Uuid, exit_code: i32) {
        if let Some(mut job) = self.running.remove(job_id) {
            job.status = JobStatus::Completed {
                finished_at: now(),
                exit_code,
            };
            self.completed.push(job);
            self.results.insert(*job_id, JobResult {
                job_id: *job_id,
                exit_code,
                error: None,
                completed_at: now(),
            });
        }
    }

    /// Mark a job as failed
    pub fn fail(&mut self, job_id: &Uuid, error: impl Into<String>) {
        let error_str = error.into();
        if let Some(mut job) = self.running.remove(job_id) {
            job.status = JobStatus::Failed {
                finished_at: now(),
                error: error_str.clone(),
            };
            self.completed.push(job);
            self.results.insert(*job_id, JobResult {
                job_id: *job_id,
                exit_code: 1,
                error: Some(error_str),
                completed_at: now(),
            });
        }
    }

    /// Get job result
    pub fn get_result(&self, job_id: &Uuid) -> Option<&JobResult> {
        self.results.get(job_id)
    }

    /// Get pending job count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get running job count
    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Get completed job count
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Get all pending jobs
    pub fn pending_jobs(&self) -> &VecDeque<JobSpec> {
        &self.pending
    }

    /// Get all running jobs
    pub fn running_jobs(&self) -> &HashMap<Uuid, JobSpec> {
        &self.running
    }

    /// Get scheduler status
    pub fn status(&self) -> SchedulerStatus {
        SchedulerStatus {
            env_count: self.environments.len(),
            pending_count: self.pending.len(),
            running_count: self.running.len(),
            completed_count: self.completed.len(),
            max_concurrent: self.max_concurrent,
        }
    }

    /// Execute next pending WASI job (requires `wasi` feature)
    ///
    /// Returns the job result if a job was executed
    #[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
    pub fn tick(&mut self) -> Result<Option<JobResult>, OanixError> {
        // Get next job
        let job = match self.next() {
            Some(j) => j,
            None => return Ok(None),
        };

        let job_id = job.id;
        let env_id = job.env_id;

        // Get the environment
        let env_arc = self.get_env(&env_id).ok_or_else(|| {
            OanixError::Job(format!("environment {} not found", env_id))
        })?;

        // Execute based on job kind
        let result = match &job.kind {
            JobKind::Wasi { wasm_bytes, args } => {
                let mut env = env_arc.write().unwrap();
                let config = RunConfig {
                    args: args.clone(),
                    env: job.env_vars.clone(),
                    working_dir: job.working_dir.clone(),
                };
                env.run_wasi(wasm_bytes, config)
            }
            JobKind::Script { script } => {
                // For now, scripts just log and return success
                // Future: integrate with a script executor
                tracing::info!("Script job: {}", script);
                Ok(crate::wasi::RunResult {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
            JobKind::Custom { name, data: _ } => {
                tracing::info!("Custom job type: {}", name);
                Ok(crate::wasi::RunResult {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        };

        // Update job status
        match result {
            Ok(run_result) => {
                self.complete(&job_id, run_result.exit_code);
                Ok(self.results.get(&job_id).cloned())
            }
            Err(e) => {
                self.fail(&job_id, e.to_string());
                Ok(self.results.get(&job_id).cloned())
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Scheduler status summary
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerStatus {
    /// Number of registered environments
    pub env_count: usize,
    /// Number of pending jobs
    pub pending_count: usize,
    /// Number of currently running jobs
    pub running_count: usize,
    /// Number of completed jobs
    pub completed_count: usize,
    /// Maximum concurrent jobs
    pub max_concurrent: usize,
}

/// Get current Unix timestamp
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::EnvBuilder;
    use crate::services::MemFs;

    fn create_test_env() -> OanixEnv {
        EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap()
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new();
        assert_eq!(scheduler.pending_count(), 0);
        assert_eq!(scheduler.running_count(), 0);
        assert_eq!(scheduler.max_concurrent, 4);
    }

    #[test]
    fn test_register_env() {
        let mut scheduler = Scheduler::new();
        let env = create_test_env();
        let env_id = env.id();

        let registered_id = scheduler.register_env(env);
        assert_eq!(registered_id, env_id);
        assert!(scheduler.get_env(&env_id).is_some());
    }

    #[test]
    fn test_submit_job() {
        let mut scheduler = Scheduler::new();
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        let job = JobSpec::new(env_id, JobKind::Script {
            script: "test".into(),
        });
        let job_id = scheduler.submit(job).unwrap();

        assert_eq!(scheduler.pending_count(), 1);
        assert!(scheduler.pending_jobs().iter().any(|j| j.id == job_id));
    }

    #[test]
    fn test_submit_job_invalid_env() {
        let mut scheduler = Scheduler::new();
        let fake_env_id = Uuid::new_v4();

        let job = JobSpec::new(fake_env_id, JobKind::Script {
            script: "test".into(),
        });
        let result = scheduler.submit(job);

        assert!(result.is_err());
    }

    #[test]
    fn test_priority_ordering() {
        let mut scheduler = Scheduler::new();
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        // Submit jobs with different priorities
        let low = JobSpec::new(env_id, JobKind::Script { script: "low".into() })
            .with_priority(-10);
        let high = JobSpec::new(env_id, JobKind::Script { script: "high".into() })
            .with_priority(10);
        let medium = JobSpec::new(env_id, JobKind::Script { script: "medium".into() })
            .with_priority(0);

        scheduler.submit(low).unwrap();
        scheduler.submit(high).unwrap();
        scheduler.submit(medium).unwrap();

        // High priority should come first
        let first = scheduler.next().unwrap();
        assert_eq!(first.priority, 10);

        let second = scheduler.next().unwrap();
        assert_eq!(second.priority, 0);

        let third = scheduler.next().unwrap();
        assert_eq!(third.priority, -10);
    }

    #[test]
    fn test_concurrency_limit() {
        let mut scheduler = Scheduler::with_max_concurrent(2);
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        // Submit 3 jobs
        for _ in 0..3 {
            let job = JobSpec::new(env_id, JobKind::Script { script: "test".into() });
            scheduler.submit(job).unwrap();
        }

        // Can only get 2
        assert!(scheduler.next().is_some());
        assert!(scheduler.next().is_some());
        assert!(scheduler.next().is_none()); // Blocked by concurrency limit

        assert_eq!(scheduler.running_count(), 2);
        assert_eq!(scheduler.pending_count(), 1);
    }

    #[test]
    fn test_complete_job() {
        let mut scheduler = Scheduler::new();
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        let job = JobSpec::new(env_id, JobKind::Script { script: "test".into() });
        scheduler.submit(job).unwrap();

        let running_job = scheduler.next().unwrap();
        let job_id = running_job.id;

        scheduler.complete(&job_id, 0);

        assert_eq!(scheduler.running_count(), 0);
        assert_eq!(scheduler.completed_count(), 1);

        let result = scheduler.get_result(&job_id).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_fail_job() {
        let mut scheduler = Scheduler::new();
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        let job = JobSpec::new(env_id, JobKind::Script { script: "test".into() });
        scheduler.submit(job).unwrap();

        let running_job = scheduler.next().unwrap();
        let job_id = running_job.id;

        scheduler.fail(&job_id, "Something went wrong");

        let result = scheduler.get_result(&job_id).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.error.as_ref().unwrap().contains("wrong"));
    }

    #[test]
    fn test_scheduler_status() {
        let mut scheduler = Scheduler::with_max_concurrent(2);
        let env = create_test_env();
        let env_id = scheduler.register_env(env);

        for _ in 0..3 {
            let job = JobSpec::new(env_id, JobKind::Script { script: "test".into() });
            scheduler.submit(job).unwrap();
        }

        scheduler.next();
        scheduler.next();

        let status = scheduler.status();
        assert_eq!(status.env_count, 1);
        assert_eq!(status.pending_count, 1);
        assert_eq!(status.running_count, 2);
        assert_eq!(status.max_concurrent, 2);
    }
}
