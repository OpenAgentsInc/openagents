//! Plan panel state for sidebar display.
//!
//! Displays TodoList and Planning stages in a persistent right sidebar
//! rather than inline in the chat feed.

/// Status of a plan task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// A single task in the active plan.
#[derive(Debug, Clone)]
pub struct PlanTask {
    pub description: String,
    pub status: TaskStatus,
}

/// The currently active plan displayed in the sidebar.
#[derive(Debug, Clone)]
pub struct ActivePlan {
    /// Optional explanation/analysis text shown above the task list.
    pub explanation: Option<String>,
    /// List of tasks with their current status.
    pub tasks: Vec<PlanTask>,
}

impl ActivePlan {
    /// Create a new plan from a list of task descriptions.
    #[allow(dead_code)]
    pub fn from_tasks(tasks: Vec<PlanTask>) -> Self {
        Self {
            explanation: None,
            tasks,
        }
    }

    /// Create a new plan with an explanation and task descriptions.
    #[allow(dead_code)]
    pub fn with_explanation(explanation: String, task_descriptions: Vec<String>) -> Self {
        Self {
            explanation: Some(explanation),
            tasks: task_descriptions
                .into_iter()
                .map(|desc| PlanTask {
                    description: desc,
                    status: TaskStatus::Pending,
                })
                .collect(),
        }
    }

    /// Count of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count()
    }

    /// Count of total tasks.
    pub fn total_count(&self) -> usize {
        self.tasks.len()
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.tasks.is_empty() {
            0.0
        } else {
            self.completed_count() as f32 / self.total_count() as f32
        }
    }
}
