//! Full-Auto Mode: Autonomous Task Execution for OpenAgents
//!
//! This crate provides autonomous task execution capabilities:
//! - Smart backend detection (Claude Code CLI, API keys, OpenRouter)
//! - Task discovery from taskmaster or Claude plans
//! - Automatic execution with progress tracking
//! - Real-time streaming updates
//!
//! # Example
//!
//! ```no_run
//! use auto::{AutoMode, AutoConfig};
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), auto::AutoError> {
//! // Auto-detect everything and run
//! let auto = AutoMode::auto().await?;
//!
//! // Stream updates
//! let mut updates = auto.run();
//! while let Some(update) = updates.next().await {
//!     println!("{:?}", update);
//! }
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod detection;
pub mod discovery;
pub mod engine;
pub mod progress;
pub mod update;

pub use config::{AutoConfig, ExecutionMode, TaskSource};
pub use detection::Detection;
pub use discovery::Discovery;
pub use engine::AutoEngine;
pub use progress::ProgressTracker;
pub use update::AutoUpdate;

use mechacoder::router::Backend;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

/// Errors that can occur during auto mode operation.
#[derive(Debug, Error)]
pub enum AutoError {
    /// No backend available.
    #[error(
        "No AI backend available. Set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or install Claude CLI"
    )]
    NoBackend,

    /// No tasks found.
    #[error("No tasks found: {0}")]
    NoTasks(String),

    /// Backend error.
    #[error("Backend error: {0}")]
    Backend(String),

    /// Taskmaster error.
    #[error("Taskmaster error: {0}")]
    Taskmaster(#[from] taskmaster::TaskmasterError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Session error.
    #[error("Session error: {0}")]
    Session(String),

    /// Cancelled by user.
    #[error("Cancelled")]
    Cancelled,
}

/// Result type for auto mode operations.
pub type Result<T> = std::result::Result<T, AutoError>;

/// Full-Auto Mode controller.
///
/// Orchestrates autonomous task execution with backend detection,
/// task discovery, and progress tracking.
pub struct AutoMode {
    config: AutoConfig,
    detection: Detection,
    engine: Option<AutoEngine>,
}

impl AutoMode {
    /// Create with auto-detection of everything.
    ///
    /// This will:
    /// 1. Detect available backends (Claude Code, API keys, OpenRouter)
    /// 2. Select the best backend
    /// 3. Discover tasks from taskmaster or plans
    pub async fn auto() -> Result<Self> {
        let config = AutoConfig::default();
        Self::with_config(config).await
    }

    /// Create with custom configuration.
    pub async fn with_config(config: AutoConfig) -> Result<Self> {
        let detection = Detection::detect(&config)?;

        if !detection.has_backend() {
            return Err(AutoError::NoBackend);
        }

        Ok(Self {
            config,
            detection,
            engine: None,
        })
    }

    /// Get the detected backend.
    pub fn backend(&self) -> Option<Backend> {
        self.detection.selected_backend()
    }

    /// Get the detection results.
    pub fn detection(&self) -> &Detection {
        &self.detection
    }

    /// Get the configuration.
    pub fn config(&self) -> &AutoConfig {
        &self.config
    }

    /// Run auto mode, returning a stream of updates.
    pub fn run(&mut self) -> impl futures::Stream<Item = AutoUpdate> {
        let config = self.config.clone();
        let detection = self.detection.clone();

        async_stream::stream! {
            // Emit initialization
            yield AutoUpdate::Initialized {
                backends_detected: detection.available_backends().to_vec(),
                selected_backend: detection.selected_backend(),
                working_directory: config.working_directory.clone(),
            };

            // Select backend
            let backend = match detection.selected_backend() {
                Some(b) => {
                    yield AutoUpdate::BackendSelected {
                        backend: b,
                        reason: detection.selection_reason().to_string(),
                    };
                    b
                }
                None => {
                    yield AutoUpdate::Error {
                        error: "No backend available".to_string(),
                    };
                    return;
                }
            };

            // Discover tasks
            let discovery = match Discovery::discover(&config) {
                Ok(d) => d,
                Err(e) => {
                    yield AutoUpdate::NoTasksFound {
                        reason: e.to_string(),
                    };
                    return;
                }
            };

            let tasks = discovery.tasks();
            if tasks.is_empty() {
                yield AutoUpdate::NoTasksFound {
                    reason: "No ready tasks found".to_string(),
                };
                return;
            }

            yield AutoUpdate::TasksDiscovered {
                count: tasks.len(),
                source: format!("{:?}", config.task_source),
            };

            // Create engine and run
            let engine = AutoEngine::new(config.clone(), detection.clone());
            let mut task_index = 0;
            let max_tasks = match config.execution_mode {
                ExecutionMode::Single => 1,
                ExecutionMode::Batch { count } => count,
                ExecutionMode::Continuous => usize::MAX,
            };

            let mut completed = 0;
            let mut failed = 0;

            for task in tasks.into_iter().take(max_tasks) {
                task_index += 1;

                yield AutoUpdate::TaskStarted {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    index: task_index,
                    total: max_tasks.min(discovery.task_count()),
                };

                // Run the task through the engine
                match engine.run_task(&task).await {
                    Ok(commits) => {
                        completed += 1;
                        yield AutoUpdate::TaskCompleted {
                            task_id: task.id.clone(),
                            success: true,
                            commits,
                        };
                    }
                    Err(e) => {
                        failed += 1;
                        yield AutoUpdate::TaskCompleted {
                            task_id: task.id.clone(),
                            success: false,
                            commits: vec![],
                        };
                        yield AutoUpdate::Error {
                            error: format!("Task {} failed: {}", task.id, e),
                        };
                    }
                }
            }

            yield AutoUpdate::Finished {
                tasks_completed: completed,
                tasks_failed: failed,
            };
        }
    }

    /// Stop auto mode gracefully.
    pub async fn stop(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.stop().await;
        }
    }
}
