//! Configuration for auto mode.

use mechacoder::router::Backend;
use std::path::PathBuf;

/// Execution mode for auto mode.
#[derive(Debug, Clone, Default)]
pub enum ExecutionMode {
    /// Execute a single task, then stop.
    #[default]
    Single,

    /// Execute up to N tasks, then stop.
    Batch {
        /// Maximum number of tasks to execute.
        count: usize,
    },

    /// Execute tasks continuously until stopped or no more tasks.
    Continuous,
}

/// Source of tasks to execute.
#[derive(Debug, Clone)]
pub enum TaskSource {
    /// Use taskmaster database for tasks.
    Taskmaster {
        /// Path to taskmaster database.
        db_path: PathBuf,
    },

    /// Use Claude plan files.
    Plans {
        /// Path to .claude directory containing plans.
        claude_dir: PathBuf,
    },

    /// Use explicit task IDs.
    Explicit {
        /// List of task IDs to execute.
        task_ids: Vec<String>,
    },

    /// Auto-detect: try taskmaster first, fall back to plans.
    Auto,
}

impl Default for TaskSource {
    fn default() -> Self {
        Self::Auto
    }
}

/// Configuration for auto mode.
#[derive(Debug, Clone)]
pub struct AutoConfig {
    /// Working directory for execution.
    pub working_directory: PathBuf,

    /// Execution mode (single, batch, continuous).
    pub execution_mode: ExecutionMode,

    /// Source of tasks.
    pub task_source: TaskSource,

    /// Preferred backend (if available, use this).
    pub preferred_backend: Option<Backend>,

    /// Path to .env.local file for additional credentials.
    pub env_local_path: Option<PathBuf>,

    /// Maximum turns per task (prevents infinite loops).
    pub max_turns_per_task: usize,

    /// Whether to auto-commit changes.
    pub auto_commit: bool,

    /// Commit message prefix.
    pub commit_prefix: Option<String>,

    /// Whether to update taskmaster with progress.
    pub update_taskmaster: bool,
}

impl Default for AutoConfig {
    fn default() -> Self {
        let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            working_directory: working_directory.clone(),
            execution_mode: ExecutionMode::Single,
            task_source: TaskSource::Auto,
            preferred_backend: None,
            env_local_path: Some(working_directory.join(".env.local")),
            max_turns_per_task: 50,
            auto_commit: true,
            commit_prefix: None,
            update_taskmaster: true,
        }
    }
}

impl AutoConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working directory.
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = path.into();
        self
    }

    /// Set the execution mode.
    pub fn execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    /// Set the task source.
    pub fn task_source(mut self, source: TaskSource) -> Self {
        self.task_source = source;
        self
    }

    /// Set the preferred backend.
    pub fn preferred_backend(mut self, backend: Backend) -> Self {
        self.preferred_backend = Some(backend);
        self
    }

    /// Set the path to .env.local file.
    pub fn env_local_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.env_local_path = Some(path.into());
        self
    }

    /// Set the maximum turns per task.
    pub fn max_turns_per_task(mut self, turns: usize) -> Self {
        self.max_turns_per_task = turns;
        self
    }

    /// Enable or disable auto-commit.
    pub fn auto_commit(mut self, enabled: bool) -> Self {
        self.auto_commit = enabled;
        self
    }

    /// Set the commit message prefix.
    pub fn commit_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.commit_prefix = Some(prefix.into());
        self
    }

    /// Enable or disable taskmaster updates.
    pub fn update_taskmaster(mut self, enabled: bool) -> Self {
        self.update_taskmaster = enabled;
        self
    }

    /// Run a single task.
    pub fn single(self) -> Self {
        self.execution_mode(ExecutionMode::Single)
    }

    /// Run N tasks.
    pub fn batch(self, count: usize) -> Self {
        self.execution_mode(ExecutionMode::Batch { count })
    }

    /// Run continuously.
    pub fn continuous(self) -> Self {
        self.execution_mode(ExecutionMode::Continuous)
    }
}
