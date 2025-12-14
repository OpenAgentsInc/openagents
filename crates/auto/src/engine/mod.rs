//! Execution engine for auto mode.

mod task_runner;

pub use task_runner::TaskRunner;

use crate::config::AutoConfig;
use crate::detection::Detection;
use crate::discovery::DiscoveredTask;
use crate::progress::ProgressTracker;
use crate::{AutoError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// The main execution engine for auto mode.
pub struct AutoEngine {
    config: AutoConfig,
    detection: Detection,
    progress: ProgressTracker,
    runner: TaskRunner,
    cancelled: Arc<AtomicBool>,
}

impl AutoEngine {
    /// Create a new engine.
    pub fn new(config: AutoConfig, detection: Detection) -> Self {
        let db_path = config.working_directory.join("taskmaster.db");
        let progress = ProgressTracker::new(
            if db_path.exists() {
                Some(&db_path)
            } else {
                None
            },
            "auto",
        )
        .unwrap_or_else(|_| ProgressTracker::without_taskmaster("auto"));

        let runner = TaskRunner::new(
            config.clone(),
            detection.clone(),
        );

        Self {
            config,
            detection,
            progress,
            runner,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run a single task.
    pub async fn run_task(&self, task: &DiscoveredTask) -> Result<Vec<String>> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(AutoError::Cancelled);
        }

        // Mark task as started
        if self.config.update_taskmaster {
            self.progress.task_started(task)?;
        }

        // Run the task
        match self.runner.run(task).await {
            Ok(commits) => {
                // Mark task as completed
                if self.config.update_taskmaster {
                    self.progress.task_completed(task, commits.clone())?;
                }
                Ok(commits)
            }
            Err(e) => {
                // Record failure
                if self.config.update_taskmaster {
                    self.progress.task_failed(task, &e.to_string())?;
                }
                Err(e)
            }
        }
    }

    /// Stop the engine gracefully.
    pub async fn stop(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if the engine is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
