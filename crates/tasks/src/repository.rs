//! Repository trait for task storage
//!
//! Defines the interface for task CRUD operations and queries.
//! Implements TASK-001..006 (creation), TASK-010..016 (listing),
//! TASK-020..026 (state management), TASK-030..034 (dependencies).

use crate::types::*;
use thiserror::Error;

/// Errors that can occur in task operations
#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Task not found: {0}")]
    NotFound(String),

    #[error("Invalid task data: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Dependency cycle detected: {0}")]
    CycleDetected(String),

    #[error("Task already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: TaskStatus, to: TaskStatus },
}

pub type TaskResult<T> = Result<T, TaskError>;

/// Repository trait for task storage operations
///
/// Implementations can use SQLite, in-memory storage, etc.
pub trait TaskRepository: Send + Sync {
    // =========================================================================
    // CRUD Operations (TASK-001..006)
    // =========================================================================

    /// Create a new task
    ///
    /// TASK-001: Create task with required fields
    /// TASK-002: Auto-generate ID if not provided
    /// TASK-003: Set timestamps automatically
    fn create(&self, task: TaskCreate) -> TaskResult<Task>;

    /// Create a task with a specific ID method
    ///
    /// TASK-004: Hash-based IDs for deduplication
    fn create_with_id_method(&self, task: TaskCreate, method: IdMethod, prefix: &str)
        -> TaskResult<Task>;

    /// Get a task by ID
    ///
    /// TASK-010: Retrieve single task
    fn get(&self, id: &str) -> TaskResult<Task>;

    /// Check if a task exists
    fn exists(&self, id: &str) -> TaskResult<bool>;

    /// Update a task
    ///
    /// TASK-020: Update task fields
    /// TASK-021: Automatically update `updated_at` timestamp
    fn update(&self, id: &str, update: TaskUpdate) -> TaskResult<Task>;

    /// Delete a task (soft delete)
    ///
    /// TASK-006: Archive/soft-delete tasks
    fn delete(&self, id: &str, reason: Option<&str>) -> TaskResult<()>;

    // =========================================================================
    // Listing & Filtering (TASK-010..016)
    // =========================================================================

    /// List all tasks matching filter
    ///
    /// TASK-011: Filter by status
    /// TASK-012: Filter by priority
    /// TASK-013: Filter by type
    /// TASK-014: Filter by assignee
    /// TASK-015: Filter by labels
    fn list(&self, filter: TaskFilter) -> TaskResult<Vec<Task>>;

    /// Get all tasks (no filter)
    fn all(&self) -> TaskResult<Vec<Task>> {
        self.list(TaskFilter::default())
    }

    /// Count tasks matching filter
    fn count(&self, filter: TaskFilter) -> TaskResult<usize>;

    /// Full-text search tasks
    ///
    /// TASK-016: Search by title/description
    fn search(&self, query: &str, filter: TaskFilter) -> TaskResult<Vec<Task>>;

    // =========================================================================
    // Ready Task Queue (TASK-020..026)
    // =========================================================================

    /// Get tasks that are ready to work on
    ///
    /// A task is "ready" if:
    /// - Status is `open`
    /// - Not deleted
    /// - No blocking dependencies (blocks, parent-child) are open/in_progress
    ///
    /// TASK-022: Ready task filtering
    /// TASK-023: Priority queue ordering
    fn ready_tasks(&self, filter: TaskFilter) -> TaskResult<Vec<Task>>;

    /// Pick the next task to work on
    ///
    /// Returns the highest priority ready task.
    /// TASK-024: Pick next task
    fn pick_next(&self, filter: TaskFilter) -> TaskResult<Option<Task>> {
        let mut f = filter;
        f.limit = Some(1);
        Ok(self.ready_tasks(f)?.into_iter().next())
    }

    /// Check if a specific task is ready
    ///
    /// TASK-025: Check task readiness
    fn is_ready(&self, id: &str) -> TaskResult<bool>;

    // =========================================================================
    // State Management (TASK-020..026)
    // =========================================================================

    /// Close a task
    ///
    /// TASK-026: Close task with commits
    fn close(&self, id: &str, reason: Option<&str>, commits: Vec<String>) -> TaskResult<Task>;

    /// Reopen a closed task
    fn reopen(&self, id: &str) -> TaskResult<Task>;

    /// Start working on a task (open -> in_progress)
    fn start(&self, id: &str) -> TaskResult<Task>;

    /// Block a task
    fn block(&self, id: &str, reason: Option<&str>) -> TaskResult<Task>;

    /// Unblock a task (blocked -> open)
    fn unblock(&self, id: &str) -> TaskResult<Task>;

    // =========================================================================
    // Dependencies (TASK-030..034)
    // =========================================================================

    /// Add a dependency
    ///
    /// TASK-030: Add dependency relationship
    fn add_dependency(&self, task_id: &str, dep: Dependency) -> TaskResult<()>;

    /// Remove a dependency
    ///
    /// TASK-031: Remove dependency
    fn remove_dependency(&self, task_id: &str, dep_id: &str) -> TaskResult<()>;

    /// Get all tasks that block this task
    ///
    /// TASK-032: Get blockers
    fn blockers(&self, task_id: &str) -> TaskResult<Vec<Task>>;

    /// Get all tasks blocked by this task
    ///
    /// TASK-033: Get blocked tasks
    fn blocked_by(&self, task_id: &str) -> TaskResult<Vec<Task>>;

    /// Check for dependency cycles
    ///
    /// TASK-034: Detect cycles
    fn has_cycle(&self, task_id: &str, dep_id: &str) -> TaskResult<bool>;

    // =========================================================================
    // Comments
    // =========================================================================

    /// Add a comment to a task
    fn add_comment(&self, task_id: &str, comment: Comment) -> TaskResult<()>;

    /// Get comments for a task
    fn comments(&self, task_id: &str) -> TaskResult<Vec<Comment>>;

    // =========================================================================
    // Crash Recovery
    // =========================================================================

    /// Get tasks in commit_pending state (for crash recovery)
    fn pending_commits(&self) -> TaskResult<Vec<Task>>;

    /// Complete a pending commit
    fn complete_pending_commit(&self, id: &str, sha: &str) -> TaskResult<Task>;

    // =========================================================================
    // Maintenance
    // =========================================================================

    /// Initialize the storage (create tables, etc.)
    fn init(&self) -> TaskResult<()>;

    /// Vacuum/optimize storage
    fn vacuum(&self) -> TaskResult<()>;
}
