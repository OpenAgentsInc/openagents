//! Core types for the task system

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

// ============================================================================
// Project Configuration Types
// ============================================================================

/// Claude Code integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Whether Claude Code is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Prefer for complex tasks
    #[serde(rename = "preferForComplexTasks", default = "default_true")]
    pub prefer_for_complex_tasks: bool,
    /// Maximum turns per subtask
    #[serde(rename = "maxTurnsPerSubtask", default = "default_300")]
    pub max_turns_per_subtask: u32,
    /// Permission mode
    #[serde(rename = "permissionMode", default = "default_permission_mode")]
    pub permission_mode: String,
    /// Fall back to minimal subagent
    #[serde(rename = "fallbackToMinimal", default = "default_true")]
    pub fallback_to_minimal: bool,
}

fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_300() -> u32 { 300 }
fn default_permission_mode() -> String { "bypassPermissions".to_string() }

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefer_for_complex_tasks: true,
            max_turns_per_subtask: 300,
            permission_mode: "bypassPermissions".to_string(),
            fallback_to_minimal: true,
        }
    }
}

/// Sandbox execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Enable sandboxed execution
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Backend to use: auto, macos-container, docker, seatbelt, none
    #[serde(default = "default_auto")]
    pub backend: String,
    /// Container image to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Memory limit with suffix (e.g., "4G")
    #[serde(rename = "memoryLimit", skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
    /// Number of CPUs to allocate
    #[serde(rename = "cpuLimit", skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<f32>,
    /// Timeout in milliseconds
    #[serde(rename = "timeoutMs", default = "default_sandbox_timeout")]
    pub timeout_ms: u64,
}

fn default_auto() -> String { "auto".to_string() }
fn default_sandbox_timeout() -> u64 { 300_000 }

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "auto".to_string(),
            image: None,
            memory_limit: None,
            cpu_limit: None,
            timeout_ms: 300_000,
        }
    }
}

/// Merge strategy for parallel execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    #[default]
    Auto,
    Direct,
    Queue,
    Pr,
}

/// Parallel execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExecutionConfig {
    /// Enable parallel execution
    #[serde(default)]
    pub enabled: bool,
    /// Maximum agents in parallel
    #[serde(rename = "maxAgents", default = "default_max_agents")]
    pub max_agents: u32,
    /// Memory per agent in MiB
    #[serde(rename = "perAgentMemoryMb", default = "default_agent_memory")]
    pub per_agent_memory_mb: u32,
    /// Host memory reserve in MiB
    #[serde(rename = "hostMemoryReserveMb", default = "default_host_memory")]
    pub host_memory_reserve_mb: u32,
    /// Worktree timeout in milliseconds
    #[serde(rename = "worktreeTimeout", default = "default_worktree_timeout")]
    pub worktree_timeout: u64,
    /// Install timeout in milliseconds
    #[serde(rename = "installTimeoutMs", default = "default_install_timeout")]
    pub install_timeout_ms: u64,
    /// Install args
    #[serde(rename = "installArgs", default = "default_install_args")]
    pub install_args: Vec<String>,
    /// Merge strategy
    #[serde(rename = "mergeStrategy", default)]
    pub merge_strategy: MergeStrategy,
    /// Merge threshold (agents before queue)
    #[serde(rename = "mergeThreshold", default = "default_merge_threshold")]
    pub merge_threshold: u32,
    /// PR threshold (agents before PR)
    #[serde(rename = "prThreshold", default = "default_pr_threshold")]
    pub pr_threshold: u32,
}

fn default_max_agents() -> u32 { 2 }
fn default_agent_memory() -> u32 { 4096 }
fn default_host_memory() -> u32 { 6144 }
fn default_worktree_timeout() -> u64 { 30 * 60 * 1000 }
fn default_install_timeout() -> u64 { 15 * 60 * 1000 }
fn default_install_args() -> Vec<String> { vec!["--frozen-lockfile".to_string()] }
fn default_merge_threshold() -> u32 { 4 }
fn default_pr_threshold() -> u32 { 50 }

impl Default for ParallelExecutionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_agents: 2,
            per_agent_memory_mb: 4096,
            host_memory_reserve_mb: 6144,
            worktree_timeout: 30 * 60 * 1000,
            install_timeout_ms: 15 * 60 * 1000,
            install_args: vec!["--frozen-lockfile".to_string()],
            merge_strategy: MergeStrategy::Auto,
            merge_threshold: 4,
            pr_threshold: 50,
        }
    }
}

/// Trajectory capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryConfig {
    /// Enable trajectory capture
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Retention days
    #[serde(rename = "retentionDays", default = "default_30")]
    pub retention_days: u32,
    /// Max storage size in GB
    #[serde(rename = "maxSizeGB", default = "default_5")]
    pub max_size_gb: u32,
    /// Include tool arguments
    #[serde(rename = "includeToolArgs", default = "default_true")]
    pub include_tool_args: bool,
    /// Include tool results
    #[serde(rename = "includeToolResults", default = "default_true")]
    pub include_tool_results: bool,
    /// Directory for trajectories
    #[serde(default = "default_trajectories_dir")]
    pub directory: String,
}

fn default_30() -> u32 { 30 }
fn default_5() -> u32 { 5 }
fn default_trajectories_dir() -> String { "trajectories".to_string() }

impl Default for TrajectoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            max_size_gb: 5,
            include_tool_args: true,
            include_tool_results: true,
            directory: "trajectories".to_string(),
        }
    }
}

/// Healer scenario configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerScenarioConfig {
    #[serde(rename = "onInitFailure", default = "default_true")]
    pub on_init_failure: bool,
    #[serde(rename = "onVerificationFailure", default = "default_true")]
    pub on_verification_failure: bool,
    #[serde(rename = "onSubtaskFailure", default = "default_true")]
    pub on_subtask_failure: bool,
    #[serde(rename = "onRuntimeError", default = "default_true")]
    pub on_runtime_error: bool,
    #[serde(rename = "onStuckSubtask", default)]
    pub on_stuck_subtask: bool,
}

impl Default for HealerScenarioConfig {
    fn default() -> Self {
        Self {
            on_init_failure: true,
            on_verification_failure: true,
            on_subtask_failure: true,
            on_runtime_error: true,
            on_stuck_subtask: false,
        }
    }
}

/// Healer spells configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealerSpellsConfig {
    /// Allowed spells (empty = all allowed)
    #[serde(default)]
    pub allowed: Vec<String>,
    /// Forbidden spells
    #[serde(default)]
    pub forbidden: Vec<String>,
}

/// Healer mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HealerMode {
    #[default]
    Conservative,
    Aggressive,
}

/// Healer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerConfig {
    /// Enable healer
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Max invocations per session
    #[serde(rename = "maxInvocationsPerSession", default = "default_2")]
    pub max_invocations_per_session: u32,
    /// Max invocations per subtask
    #[serde(rename = "maxInvocationsPerSubtask", default = "default_1")]
    pub max_invocations_per_subtask: u32,
    /// Scenario configuration
    #[serde(default)]
    pub scenarios: HealerScenarioConfig,
    /// Spells configuration
    #[serde(default)]
    pub spells: HealerSpellsConfig,
    /// Healer mode
    #[serde(default)]
    pub mode: HealerMode,
    /// Stuck threshold hours
    #[serde(rename = "stuckThresholdHours", default = "default_2")]
    pub stuck_threshold_hours: u32,
}

fn default_2() -> u32 { 2 }
fn default_1() -> u32 { 1 }

impl Default for HealerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_invocations_per_session: 2,
            max_invocations_per_subtask: 1,
            scenarios: HealerScenarioConfig::default(),
            spells: HealerSpellsConfig::default(),
            mode: HealerMode::Conservative,
            stuck_threshold_hours: 2,
        }
    }
}

/// Reflexion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflexionConfig {
    /// Enable reflexion
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Max reflections per retry
    #[serde(rename = "maxReflectionsPerRetry", default = "default_3")]
    pub max_reflections_per_retry: u32,
    /// Generation timeout in ms
    #[serde(rename = "generationTimeoutMs", default = "default_30000")]
    pub generation_timeout_ms: u64,
    /// Retention days
    #[serde(rename = "retentionDays", default = "default_30")]
    pub retention_days: u32,
}

fn default_3() -> u32 { 3 }
fn default_30000() -> u64 { 30000 }

impl Default for ReflexionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_reflections_per_retry: 3,
            generation_timeout_ms: 30000,
            retention_days: 30,
        }
    }
}

/// Failure cleanup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCleanupConfig {
    /// Revert tracked files on failure
    #[serde(rename = "revertTrackedFiles", default = "default_true")]
    pub revert_tracked_files: bool,
    /// Delete untracked files on failure
    #[serde(rename = "deleteUntrackedFiles", default)]
    pub delete_untracked_files: bool,
}

impl Default for FailureCleanupConfig {
    fn default() -> Self {
        Self {
            revert_tracked_files: true,
            delete_untracked_files: false,
        }
    }
}

/// Terminal-Bench learning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBenchLearningConfig {
    /// Enable skill injection
    #[serde(default = "default_true")]
    pub skills: bool,
    /// Enable memory retrieval
    #[serde(default)]
    pub memory: bool,
    /// Enable reflexion
    #[serde(default)]
    pub reflexion: bool,
    /// Enable post-iteration learning
    #[serde(default)]
    pub learn: bool,
}

impl Default for TBenchLearningConfig {
    fn default() -> Self {
        Self {
            skills: true,
            memory: false,
            reflexion: false,
            learn: false,
        }
    }
}

/// Terminal-Bench configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBenchConfig {
    /// Default model
    #[serde(rename = "defaultModel", default = "default_tbench_model")]
    pub default_model: String,
    /// Default suite path
    #[serde(rename = "defaultSuite", skip_serializing_if = "Option::is_none")]
    pub default_suite: Option<String>,
    /// Default timeout in seconds
    #[serde(rename = "defaultTimeout", default = "default_3600")]
    pub default_timeout: u64,
    /// Default max turns
    #[serde(rename = "defaultMaxTurns", default = "default_300")]
    pub default_max_turns: u32,
    /// Default learning options
    #[serde(rename = "defaultLearning", default)]
    pub default_learning: TBenchLearningConfig,
}

fn default_tbench_model() -> String { "claude-code".to_string() }
fn default_3600() -> u64 { 3600 }

impl Default for TBenchConfig {
    fn default() -> Self {
        Self {
            default_model: "claude-code".to_string(),
            default_suite: None,
            default_timeout: 3600,
            default_max_turns: 300,
            default_learning: TBenchLearningConfig::default(),
        }
    }
}

/// Cloud configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudConfig {
    /// Use gateway
    #[serde(rename = "useGateway", default)]
    pub use_gateway: bool,
    /// Send telemetry
    #[serde(rename = "sendTelemetry", default)]
    pub send_telemetry: bool,
    /// Relay URL
    #[serde(rename = "relayUrl", skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
}

/// Project configuration matching .openagents/project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Config version
    #[serde(default = "default_1")]
    pub version: u32,
    /// Project identifier
    #[serde(rename = "projectId")]
    pub project_id: String,
    /// Default branch name
    #[serde(rename = "defaultBranch", default = "default_main")]
    pub default_branch: String,
    /// Working branch name
    #[serde(rename = "workBranch", skip_serializing_if = "Option::is_none")]
    pub work_branch: Option<String>,
    /// Default model
    #[serde(rename = "defaultModel", default = "default_model")]
    pub default_model: String,
    /// Root directory
    #[serde(rename = "rootDir", default = "default_dot")]
    pub root_dir: String,
    /// Type check commands
    #[serde(rename = "typecheckCommands", default)]
    pub typecheck_commands: Vec<String>,
    /// Test commands
    #[serde(rename = "testCommands", default)]
    pub test_commands: Vec<String>,
    /// Sandbox test commands
    #[serde(rename = "sandboxTestCommands", default)]
    pub sandbox_test_commands: Vec<String>,
    /// E2E commands
    #[serde(rename = "e2eCommands", default)]
    pub e2e_commands: Vec<String>,
    /// Allow push
    #[serde(rename = "allowPush", default = "default_true")]
    pub allow_push: bool,
    /// Allow force push
    #[serde(rename = "allowForcePush", default)]
    pub allow_force_push: bool,
    /// Max tasks per run
    #[serde(rename = "maxTasksPerRun", default = "default_3")]
    pub max_tasks_per_run: u32,
    /// Max runtime in minutes
    #[serde(rename = "maxRuntimeMinutes", default = "default_240")]
    pub max_runtime_minutes: u32,
    /// ID prefix
    #[serde(rename = "idPrefix", default = "default_id_prefix")]
    pub id_prefix: String,
    /// Session directory
    #[serde(rename = "sessionDir", default = "default_session_dir")]
    pub session_dir: String,
    /// Run log directory
    #[serde(rename = "runLogDir", default = "default_run_log_dir")]
    pub run_log_dir: String,
    /// Claude Code config
    #[serde(rename = "claudeCode", default)]
    pub claude_code: ClaudeCodeConfig,
    /// Sandbox config
    #[serde(default)]
    pub sandbox: SandboxConfig,
    /// Parallel execution config
    #[serde(rename = "parallelExecution", default)]
    pub parallel_execution: ParallelExecutionConfig,
    /// Trajectory config
    #[serde(default)]
    pub trajectory: TrajectoryConfig,
    /// Healer config
    #[serde(default)]
    pub healer: HealerConfig,
    /// Reflexion config
    #[serde(default)]
    pub reflexion: ReflexionConfig,
    /// Failure cleanup config
    #[serde(rename = "failureCleanup", default)]
    pub failure_cleanup: FailureCleanupConfig,
    /// Terminal-Bench config
    #[serde(default)]
    pub tbench: TBenchConfig,
    /// Cloud config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<CloudConfig>,
}

fn default_main() -> String { "main".to_string() }
fn default_model() -> String { "x-ai/grok-4.1-fast:free".to_string() }
fn default_dot() -> String { ".".to_string() }
fn default_240() -> u32 { 240 }
fn default_id_prefix() -> String { "oa".to_string() }
fn default_session_dir() -> String { ".openagents/sessions".to_string() }
fn default_run_log_dir() -> String { ".openagents/run-logs".to_string() }

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: 1,
            project_id: String::new(),
            default_branch: "main".to_string(),
            work_branch: None,
            default_model: "x-ai/grok-4.1-fast:free".to_string(),
            root_dir: ".".to_string(),
            typecheck_commands: Vec::new(),
            test_commands: Vec::new(),
            sandbox_test_commands: Vec::new(),
            e2e_commands: Vec::new(),
            allow_push: true,
            allow_force_push: false,
            max_tasks_per_run: 3,
            max_runtime_minutes: 240,
            id_prefix: "oa".to_string(),
            session_dir: ".openagents/sessions".to_string(),
            run_log_dir: ".openagents/run-logs".to_string(),
            claude_code: ClaudeCodeConfig::default(),
            sandbox: SandboxConfig::default(),
            parallel_execution: ParallelExecutionConfig::default(),
            trajectory: TrajectoryConfig::default(),
            healer: HealerConfig::default(),
            reflexion: ReflexionConfig::default(),
            failure_cleanup: FailureCleanupConfig::default(),
            tbench: TBenchConfig::default(),
            cloud: None,
        }
    }
}

// ============================================================================
// ID Generation Functions
// ============================================================================

/// Maximum hierarchy depth (prevents over-decomposition)
pub const MAX_HIERARCHY_DEPTH: u32 = 3;

/// Generate a child ID in hierarchical format.
/// Format: parent.N (e.g., "oa-abc123.1", "oa-abc123.1.2")
pub fn generate_child_id(parent_id: &str, child_number: u32) -> String {
    format!("{}.{}", parent_id, child_number)
}

/// Parsed hierarchical ID info
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchicalIdInfo {
    /// The root task ID
    pub root_id: String,
    /// The parent ID (if not root)
    pub parent_id: Option<String>,
    /// Depth in hierarchy (0 = root)
    pub depth: u32,
}

/// Parse a hierarchical ID to extract root, parent, and depth.
///
/// Examples:
///   "oa-abc123" → { root_id: "oa-abc123", parent_id: None, depth: 0 }
///   "oa-abc123.1" → { root_id: "oa-abc123", parent_id: Some("oa-abc123"), depth: 1 }
///   "oa-abc123.1.2" → { root_id: "oa-abc123", parent_id: Some("oa-abc123.1"), depth: 2 }
pub fn parse_hierarchical_id(id: &str) -> HierarchicalIdInfo {
    let parts: Vec<&str> = id.split('.').collect();

    if parts.len() == 1 {
        return HierarchicalIdInfo {
            root_id: id.to_string(),
            parent_id: None,
            depth: 0,
        };
    }

    let root_id = parts[0].to_string();
    let parent_id = parts[..parts.len() - 1].join(".");
    let depth = (parts.len() - 1) as u32;

    HierarchicalIdInfo {
        root_id,
        parent_id: Some(parent_id),
        depth,
    }
}

/// Check if a task ID matches or is a child of a given parent ID.
pub fn is_child_of(task_id: &str, parent_id: &str) -> bool {
    task_id.starts_with(&format!("{}.", parent_id))
}

/// Get the immediate parent ID of a hierarchical ID.
/// Returns None if the ID has no parent.
pub fn get_parent_id(id: &str) -> Option<String> {
    parse_hierarchical_id(id).parent_id
}

/// Check if an ID can have children (not already at max depth).
pub fn can_have_children(id: &str) -> bool {
    parse_hierarchical_id(id).depth < MAX_HIERARCHY_DEPTH
}

/// Find the next available child number for a parent.
pub fn find_next_child_number(parent_id: &str, existing_ids: &[String]) -> u32 {
    let prefix = format!("{}.", parent_id);
    let mut max_child = 0u32;

    for id in existing_ids {
        if let Some(suffix) = id.strip_prefix(&prefix) {
            if let Some(first_part) = suffix.split('.').next() {
                if let Ok(n) = first_part.parse::<u32>() {
                    if n > max_child {
                        max_child = n;
                    }
                }
            }
        }
    }

    max_child + 1
}

/// Task readiness check
pub fn is_task_ready(task: &Task, all_tasks: &[Task]) -> bool {
    // Tasks that are closed, blocked, or in commit_pending state are not ready
    if matches!(
        task.status,
        TaskStatus::Closed | TaskStatus::Blocked | TaskStatus::CommitPending
    ) {
        return false;
    }

    // Check blocking dependencies
    for dep in &task.deps {
        if dep.dep_type.blocks_readiness() {
            let dep_task = all_tasks.iter().find(|t| t.id == dep.id);
            if let Some(dt) = dep_task {
                if dt.status != TaskStatus::Closed {
                    return false;
                }
            }
        }
    }

    true
}

/// Deletion entry for tracking deleted tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionEntry {
    /// Task ID that was deleted
    #[serde(rename = "taskId")]
    pub task_id: String,
    /// When deleted
    #[serde(rename = "deletedAt")]
    pub deleted_at: DateTime<Utc>,
    /// Who deleted it
    #[serde(rename = "deletedBy", skip_serializing_if = "Option::is_none")]
    pub deleted_by: Option<String>,
    /// Reason for deletion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hierarchical_id_root() {
        let info = parse_hierarchical_id("oa-abc123");
        assert_eq!(info.root_id, "oa-abc123");
        assert_eq!(info.parent_id, None);
        assert_eq!(info.depth, 0);
    }

    #[test]
    fn test_parse_hierarchical_id_child() {
        let info = parse_hierarchical_id("oa-abc123.1");
        assert_eq!(info.root_id, "oa-abc123");
        assert_eq!(info.parent_id, Some("oa-abc123".to_string()));
        assert_eq!(info.depth, 1);
    }

    #[test]
    fn test_parse_hierarchical_id_grandchild() {
        let info = parse_hierarchical_id("oa-abc123.1.2");
        assert_eq!(info.root_id, "oa-abc123");
        assert_eq!(info.parent_id, Some("oa-abc123.1".to_string()));
        assert_eq!(info.depth, 2);
    }

    #[test]
    fn test_generate_child_id() {
        assert_eq!(generate_child_id("oa-abc", 1), "oa-abc.1");
        assert_eq!(generate_child_id("oa-abc.1", 2), "oa-abc.1.2");
    }

    #[test]
    fn test_is_child_of() {
        assert!(is_child_of("oa-abc.1", "oa-abc"));
        assert!(is_child_of("oa-abc.1.2", "oa-abc"));
        assert!(is_child_of("oa-abc.1.2", "oa-abc.1"));
        assert!(!is_child_of("oa-abc", "oa-abc"));
        assert!(!is_child_of("oa-xyz.1", "oa-abc"));
    }

    #[test]
    fn test_can_have_children() {
        assert!(can_have_children("oa-abc"));
        assert!(can_have_children("oa-abc.1"));
        assert!(can_have_children("oa-abc.1.2"));
        assert!(!can_have_children("oa-abc.1.2.3"));
    }

    #[test]
    fn test_find_next_child_number() {
        let existing = vec![
            "oa-abc.1".to_string(),
            "oa-abc.2".to_string(),
            "oa-abc.3".to_string(),
        ];
        assert_eq!(find_next_child_number("oa-abc", &existing), 4);

        let empty: Vec<String> = vec![];
        assert_eq!(find_next_child_number("oa-abc", &empty), 1);

        let with_gaps = vec!["oa-abc.1".to_string(), "oa-abc.5".to_string()];
        assert_eq!(find_next_child_number("oa-abc", &with_gaps), 6);
    }

    #[test]
    fn test_project_config_default() {
        let config = ProjectConfig::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.default_branch, "main");
        assert!(config.allow_push);
        assert!(!config.allow_force_push);
        assert!(config.healer.enabled);
    }

    #[test]
    fn test_project_config_serde() {
        let json = r#"{
            "projectId": "test-project",
            "defaultBranch": "develop",
            "healer": {
                "enabled": false
            }
        }"#;

        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.default_branch, "develop");
        assert!(!config.healer.enabled);
        // Defaults should be applied
        assert!(config.allow_push);
    }

    #[test]
    fn test_is_task_ready() {
        let task = Task {
            id: "oa-123".to_string(),
            title: "Test".to_string(),
            description: String::new(),
            status: TaskStatus::Open,
            priority: TaskPriority::Medium,
            task_type: TaskType::Task,
            assignee: None,
            labels: vec![],
            deps: vec![],
            commits: vec![],
            comments: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
            close_reason: None,
            source: None,
            design: None,
            acceptance_criteria: None,
            notes: None,
            estimated_minutes: None,
            pending_commit: None,
        };

        assert!(is_task_ready(&task, &[]));

        let closed_task = Task {
            status: TaskStatus::Closed,
            ..task.clone()
        };
        assert!(!is_task_ready(&closed_task, &[]));
    }
}
