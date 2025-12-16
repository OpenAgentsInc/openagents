//! Updates streamed from auto mode execution.

use mechacoder::router::Backend;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Updates emitted during auto mode execution.
///
/// These events allow the UI to display real-time progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutoUpdate {
    // =========================================================================
    // Initialization Events
    // =========================================================================
    /// Auto mode initialized with detection results.
    Initialized {
        /// Available backends detected.
        backends_detected: Vec<Backend>,
        /// Backend that will be used.
        selected_backend: Option<Backend>,
        /// Working directory.
        working_directory: PathBuf,
    },

    /// Backend selected for execution.
    BackendSelected {
        /// The selected backend.
        backend: Backend,
        /// Reason for selection.
        reason: String,
    },

    // =========================================================================
    // Discovery Events
    // =========================================================================
    /// Tasks discovered from source.
    TasksDiscovered {
        /// Number of tasks found.
        count: usize,
        /// Source description.
        source: String,
    },

    /// No tasks found.
    NoTasksFound {
        /// Reason no tasks were found.
        reason: String,
    },

    // =========================================================================
    // Task Execution Events
    // =========================================================================
    /// Task execution started.
    TaskStarted {
        /// Task ID.
        task_id: String,
        /// Task title.
        title: String,
        /// Current task index (1-based).
        index: usize,
        /// Total tasks to execute.
        total: usize,
    },

    /// Text delta from LLM.
    TextDelta {
        /// Task ID.
        task_id: String,
        /// Text content.
        delta: String,
    },

    /// Reasoning/thinking delta from LLM.
    ReasoningDelta {
        /// Task ID.
        task_id: String,
        /// Reasoning content.
        delta: String,
    },

    /// Tool execution started.
    ToolStarted {
        /// Task ID.
        task_id: String,
        /// Tool name.
        tool_name: String,
        /// Tool call ID.
        tool_call_id: String,
        /// Tool input (JSON).
        input: serde_json::Value,
    },

    /// Tool execution completed.
    ToolCompleted {
        /// Task ID.
        task_id: String,
        /// Tool call ID.
        tool_call_id: String,
        /// Tool output.
        output: String,
        /// Whether tool returned an error.
        is_error: bool,
    },

    /// Git commit created.
    CommitCreated {
        /// Task ID.
        task_id: String,
        /// Commit SHA.
        sha: String,
        /// Commit message.
        message: String,
    },

    /// Task execution completed.
    TaskCompleted {
        /// Task ID.
        task_id: String,
        /// Whether task succeeded.
        success: bool,
        /// Commits made during task.
        commits: Vec<String>,
    },

    // =========================================================================
    // Completion Events
    // =========================================================================
    /// Auto mode finished.
    Finished {
        /// Number of tasks completed successfully.
        tasks_completed: usize,
        /// Number of tasks that failed.
        tasks_failed: usize,
    },

    /// Error occurred.
    Error {
        /// Error message.
        error: String,
    },

    /// Execution cancelled.
    Cancelled {
        /// Reason for cancellation.
        reason: Option<String>,
    },
}

impl AutoUpdate {
    /// Check if this is a terminal event (finished, error, or cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AutoUpdate::Finished { .. } | AutoUpdate::Error { .. } | AutoUpdate::Cancelled { .. }
        )
    }

    /// Check if this is an error event.
    pub fn is_error(&self) -> bool {
        matches!(self, AutoUpdate::Error { .. })
    }

    /// Get the task ID if this event is task-related.
    pub fn task_id(&self) -> Option<&str> {
        match self {
            AutoUpdate::TaskStarted { task_id, .. }
            | AutoUpdate::TextDelta { task_id, .. }
            | AutoUpdate::ReasoningDelta { task_id, .. }
            | AutoUpdate::ToolStarted { task_id, .. }
            | AutoUpdate::ToolCompleted { task_id, .. }
            | AutoUpdate::CommitCreated { task_id, .. }
            | AutoUpdate::TaskCompleted { task_id, .. } => Some(task_id),
            _ => None,
        }
    }
}
