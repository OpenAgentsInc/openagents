//! Core types for the task system
//!
//! Mirrors the TypeScript schema from src/tasks/schema.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task status - 5-state model
///
/// State transitions:
/// - open -> in_progress, blocked, closed
/// - in_progress -> open, blocked, closed, commit_pending
/// - blocked -> open, closed
/// - closed -> open (reopen)
/// - commit_pending -> closed (crash recovery)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Not started, ready to work
    #[default]
    Open,
    /// Active work in progress
    InProgress,
    /// Waiting on dependency
    Blocked,
    /// Completed/resolved
    Closed,
    /// Transient state for two-phase commit crash recovery
    CommitPending,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Open => "open",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Closed => "closed",
            TaskStatus::CommitPending => "commit_pending",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "open" => Some(TaskStatus::Open),
            "in_progress" => Some(TaskStatus::InProgress),
            "blocked" => Some(TaskStatus::Blocked),
            "closed" => Some(TaskStatus::Closed),
            "commit_pending" => Some(TaskStatus::CommitPending),
            _ => None,
        }
    }
}

/// Task priority - 0 is highest, 4 is lowest
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum TaskPriority {
    /// P0 - Critical
    Critical = 0,
    /// P1 - High
    High = 1,
    /// P2 - Medium (default)
    #[default]
    Medium = 2,
    /// P3 - Low
    Low = 3,
    /// P4 - Backlog
    Backlog = 4,
}

impl TaskPriority {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(TaskPriority::Critical),
            1 => Some(TaskPriority::High),
            2 => Some(TaskPriority::Medium),
            3 => Some(TaskPriority::Low),
            4 => Some(TaskPriority::Backlog),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Task type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Bug fix
    Bug,
    /// New feature
    Feature,
    /// General task
    #[default]
    Task,
    /// Large multi-task initiative
    Epic,
    /// Maintenance/cleanup
    Chore,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Bug => "bug",
            TaskType::Feature => "feature",
            TaskType::Task => "task",
            TaskType::Epic => "epic",
            TaskType::Chore => "chore",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "bug" => Some(TaskType::Bug),
            "feature" => Some(TaskType::Feature),
            "task" => Some(TaskType::Task),
            "epic" => Some(TaskType::Epic),
            "chore" => Some(TaskType::Chore),
            _ => None,
        }
    }
}

/// Dependency relationship type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyType {
    /// Task A blocks Task B (B waits for A to close)
    Blocks,
    /// Informational link (doesn't block)
    Related,
    /// Hierarchical relationship (child waits for parent)
    ParentChild,
    /// Traceability link (where did this task originate)
    DiscoveredFrom,
}

impl DependencyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyType::Blocks => "blocks",
            DependencyType::Related => "related",
            DependencyType::ParentChild => "parent-child",
            DependencyType::DiscoveredFrom => "discovered-from",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "blocks" => Some(DependencyType::Blocks),
            "related" => Some(DependencyType::Related),
            "parent-child" => Some(DependencyType::ParentChild),
            "discovered-from" => Some(DependencyType::DiscoveredFrom),
            _ => None,
        }
    }

    /// Returns true if this dependency type affects task readiness
    pub fn blocks_readiness(&self) -> bool {
        matches!(self, DependencyType::Blocks | DependencyType::ParentChild)
    }
}

/// A dependency relationship
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Task ID being depended upon
    pub id: String,
    /// Type of dependency relationship
    #[serde(rename = "type")]
    pub dep_type: DependencyType,
}

/// A comment in the task thread
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Unique comment ID
    pub id: String,
    /// Comment text content
    pub text: String,
    /// Author username/agent name
    pub author: String,
    /// When the comment was created
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

/// Source tracking information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSource {
    /// Repository name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Task ID this was discovered from
    #[serde(rename = "discoveredFrom", skip_serializing_if = "Option::is_none")]
    pub discovered_from: Option<String>,
    /// External reference (GitHub issue, etc.)
    #[serde(rename = "externalRef", skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,
}

/// Pending commit metadata for crash recovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingCommit {
    /// Commit message
    pub message: String,
    /// When the commit was initiated
    pub timestamp: DateTime<Utc>,
    /// Branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Commit SHA (filled after git commit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
}

/// Full task entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID (e.g., "oa-abc123")
    pub id: String,

    /// Task title (1-500 chars)
    pub title: String,

    /// Optional description
    #[serde(default)]
    pub description: String,

    /// Current status
    pub status: TaskStatus,

    /// Priority level (0 = highest)
    pub priority: TaskPriority,

    /// Task type classification
    #[serde(rename = "type")]
    pub task_type: TaskType,

    /// Assigned user/agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Labels/tags
    #[serde(default)]
    pub labels: Vec<String>,

    /// Task dependencies
    #[serde(default)]
    pub deps: Vec<Dependency>,

    /// Git commit SHAs that resolve this task
    #[serde(default)]
    pub commits: Vec<String>,

    /// Comment thread
    #[serde(default)]
    pub comments: Vec<Comment>,

    /// When the task was created
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    /// When the task was last updated
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    /// When the task was closed (if closed)
    #[serde(rename = "closedAt", skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    /// Reason for closing
    #[serde(rename = "closeReason", skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<String>,

    /// Source tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<TaskSource>,

    /// Design doc reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design: Option<String>,

    /// Acceptance criteria
    #[serde(rename = "acceptanceCriteria", skip_serializing_if = "Option::is_none")]
    pub acceptance_criteria: Option<String>,

    /// Free-form notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Time estimate in minutes
    #[serde(rename = "estimatedMinutes", skip_serializing_if = "Option::is_none")]
    pub estimated_minutes: Option<i32>,

    /// Pending commit for crash recovery
    #[serde(rename = "pendingCommit", skip_serializing_if = "Option::is_none")]
    pub pending_commit: Option<PendingCommit>,
}

/// Data for creating a new task
#[derive(Debug, Clone, Default)]
pub struct TaskCreate {
    /// Task title (required)
    pub title: String,
    /// Optional description
    pub description: Option<String>,
    /// Priority level
    pub priority: TaskPriority,
    /// Task type
    pub task_type: TaskType,
    /// Assigned user/agent
    pub assignee: Option<String>,
    /// Labels/tags
    pub labels: Vec<String>,
    /// Dependencies
    pub deps: Vec<Dependency>,
    /// Source tracking
    pub source: Option<TaskSource>,
    /// Design doc reference
    pub design: Option<String>,
    /// Acceptance criteria
    pub acceptance_criteria: Option<String>,
    /// Notes
    pub notes: Option<String>,
    /// Time estimate in minutes
    pub estimated_minutes: Option<i32>,
}

/// Data for updating an existing task
#[derive(Debug, Clone, Default)]
pub struct TaskUpdate {
    /// Update title
    pub title: Option<String>,
    /// Update description
    pub description: Option<String>,
    /// Update status
    pub status: Option<TaskStatus>,
    /// Update priority
    pub priority: Option<TaskPriority>,
    /// Update task type
    pub task_type: Option<TaskType>,
    /// Update assignee
    pub assignee: Option<Option<String>>,
    /// Update labels (replaces all)
    pub labels: Option<Vec<String>>,
    /// Update deps (replaces all)
    pub deps: Option<Vec<Dependency>>,
    /// Add commits
    pub add_commits: Option<Vec<String>>,
    /// Update close reason
    pub close_reason: Option<Option<String>>,
    /// Update design reference
    pub design: Option<Option<String>>,
    /// Update acceptance criteria
    pub acceptance_criteria: Option<Option<String>>,
    /// Update notes
    pub notes: Option<Option<String>>,
    /// Update time estimate
    pub estimated_minutes: Option<Option<i32>>,
    /// Update pending commit
    pub pending_commit: Option<Option<PendingCommit>>,
}

/// Sorting policy for ready tasks
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortPolicy {
    /// Priority first, then oldest (default)
    #[default]
    Hybrid,
    /// Pure priority, ties use newest
    Priority,
    /// FIFO - oldest first
    Oldest,
    /// Reverse FIFO - newest first
    Newest,
}

/// Filter options for listing tasks
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    /// Filter by status
    pub status: Option<TaskStatus>,
    /// Filter by priority
    pub priority: Option<TaskPriority>,
    /// Filter by type
    pub task_type: Option<TaskType>,
    /// Filter by assignee
    pub assignee: Option<String>,
    /// Filter by labels (any match)
    pub labels: Option<Vec<String>>,
    /// Sorting policy
    pub sort: SortPolicy,
    /// Max results
    pub limit: Option<usize>,
}

/// ID generation method
#[derive(Debug, Clone, Copy, Default)]
pub enum IdMethod {
    /// Hash-based (deterministic from title+description)
    Hash,
    /// Random UUID
    #[default]
    Random,
}
