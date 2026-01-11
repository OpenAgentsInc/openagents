//! Configuration types for OpenAgents

use serde::{Deserialize, Serialize};

/// Codex permission mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Standard behavior
    Default,
    /// Accept suggested edits
    AcceptEdits,
    /// Skip permission checks (default for autonomous operation)
    #[default]
    BypassPermissions,
    /// Plan-only mode
    Plan,
    /// Don't ask for permissions
    DontAsk,
}

/// Sandbox backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxBackend {
    /// Auto-detect best backend
    #[default]
    Auto,
    /// macOS container (Apple's containerization)
    MacosContainer,
    /// Docker container
    Docker,
    /// macOS Seatbelt sandboxing
    Seatbelt,
    /// No sandboxing
    None,
}

/// Healer mode (self-healing aggressiveness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HealerMode {
    /// Conservative healing (fewer interventions)
    #[default]
    Conservative,
    /// Moderate healing
    Moderate,
    /// Aggressive healing (more interventions)
    Aggressive,
}

/// Merge strategy for parallel execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum MergeStrategy {
    /// Auto-detect best strategy
    #[default]
    Auto,
    /// Sequential merging
    Sequential,
    /// Parallel merging
    Parallel,
}

/// Codex configuration
///
/// CONF-020..024: Codex settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCodeConfig {
    /// Whether Codex is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Prefer Codex for complex tasks
    #[serde(default = "default_true")]
    pub prefer_for_complex_tasks: bool,

    /// Maximum turns per subtask
    #[serde(default = "default_max_turns")]
    pub max_turns_per_subtask: u32,

    /// Permission mode
    #[serde(default)]
    pub permission_mode: PermissionMode,

    /// Fall back to minimal mode on errors
    #[serde(default = "default_true")]
    pub fallback_to_minimal: bool,
}

impl Default for CodexCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefer_for_complex_tasks: true,
            max_turns_per_subtask: 300,
            permission_mode: PermissionMode::BypassPermissions,
            fallback_to_minimal: true,
        }
    }
}

/// Sandbox configuration
///
/// CONF-030..033: Sandbox settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxConfig {
    /// Whether sandboxing is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Sandbox backend to use
    #[serde(default)]
    pub backend: SandboxBackend,

    /// Docker image (if using Docker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Memory limit (e.g., "8G")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,

    /// CPU limit (number of cores)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<f32>,

    /// Timeout in milliseconds
    #[serde(default = "default_sandbox_timeout")]
    pub timeout_ms: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: SandboxBackend::Auto,
            image: None,
            memory_limit: None,
            cpu_limit: None,
            timeout_ms: 300_000, // 5 minutes
        }
    }
}

/// Healer scenario configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealerScenarioConfig {
    /// Heal on initialization failure
    #[serde(default = "default_true")]
    pub on_init_failure: bool,

    /// Heal on verification failure
    #[serde(default = "default_true")]
    pub on_verification_failure: bool,

    /// Heal on subtask failure
    #[serde(default = "default_true")]
    pub on_subtask_failure: bool,

    /// Heal on runtime error
    #[serde(default = "default_true")]
    pub on_runtime_error: bool,

    /// Heal on stuck subtask
    #[serde(default)]
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

/// Healer configuration (self-healing)
///
/// CONF-010..013: Safety/recovery settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealerConfig {
    /// Whether healer is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum invocations per session
    #[serde(default = "default_max_invocations_session")]
    pub max_invocations_per_session: u32,

    /// Maximum invocations per subtask
    #[serde(default = "default_max_invocations_subtask")]
    pub max_invocations_per_subtask: u32,

    /// Healing scenarios
    #[serde(default)]
    pub scenarios: HealerScenarioConfig,

    /// Healer mode (aggressiveness)
    #[serde(default)]
    pub mode: HealerMode,

    /// Hours before considering a subtask stuck
    #[serde(default = "default_stuck_threshold")]
    pub stuck_threshold_hours: u32,
}

impl Default for HealerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_invocations_per_session: 2,
            max_invocations_per_subtask: 1,
            scenarios: HealerScenarioConfig::default(),
            mode: HealerMode::Conservative,
            stuck_threshold_hours: 2,
        }
    }
}

/// Failure cleanup configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureCleanupConfig {
    /// Revert tracked files on failure
    #[serde(default = "default_true")]
    pub revert_tracked_files: bool,

    /// Delete untracked files on failure
    #[serde(default)]
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

/// Parallel execution configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelExecutionConfig {
    /// Whether parallel execution is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Maximum parallel agents
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,

    /// Memory per agent in MB
    #[serde(default = "default_agent_memory")]
    pub per_agent_memory_mb: u32,

    /// Host memory reserve in MB
    #[serde(default = "default_host_memory")]
    pub host_memory_reserve_mb: u32,

    /// Worktree timeout in ms
    #[serde(default = "default_worktree_timeout")]
    pub worktree_timeout: u64,

    /// Install timeout in ms
    #[serde(default = "default_install_timeout")]
    pub install_timeout_ms: u64,

    /// Install arguments
    #[serde(default = "default_install_args")]
    pub install_args: Vec<String>,

    /// Merge strategy
    #[serde(default)]
    pub merge_strategy: MergeStrategy,

    /// Merge threshold (commits before merging)
    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: u32,

    /// PR threshold (changes before PR)
    #[serde(default = "default_pr_threshold")]
    pub pr_threshold: u32,
}

impl Default for ParallelExecutionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_agents: 2,
            per_agent_memory_mb: 4096,
            host_memory_reserve_mb: 6144,
            worktree_timeout: 30 * 60 * 1000,   // 30 minutes
            install_timeout_ms: 15 * 60 * 1000, // 15 minutes
            install_args: vec!["--frozen-lockfile".into()],
            merge_strategy: MergeStrategy::Auto,
            merge_threshold: 4,
            pr_threshold: 50,
        }
    }
}

/// Learning configuration for Terminal-Bench
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningConfig {
    /// Enable skills learning
    #[serde(default = "default_true")]
    pub skills: bool,

    /// Enable memory learning
    #[serde(default = "default_true")]
    pub memory: bool,

    /// Enable reflexion learning
    #[serde(default = "default_true")]
    pub reflexion: bool,

    /// Enable general learning
    #[serde(default = "default_true")]
    pub learn: bool,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            skills: true,
            memory: true,
            reflexion: true,
            learn: true,
        }
    }
}

/// Terminal-Bench configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TBenchConfig {
    /// Default model for benchmarks
    #[serde(default = "default_tbench_model")]
    pub default_model: String,

    /// Default test suite
    #[serde(default = "default_tbench_suite")]
    pub default_suite: String,

    /// Default timeout in seconds
    #[serde(default = "default_tbench_timeout")]
    pub default_timeout: u32,

    /// Default max turns
    #[serde(default = "default_max_turns")]
    pub default_max_turns: u32,

    /// Learning configuration
    #[serde(default)]
    pub default_learning: LearningConfig,
}

impl Default for TBenchConfig {
    fn default() -> Self {
        Self {
            default_model: "fm".into(),
            default_suite: "docs/tb-tasks/fm-mini-suite.json".into(),
            default_timeout: 3600,
            default_max_turns: 300,
            default_learning: LearningConfig::default(),
        }
    }
}

/// Trajectory recording configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryConfig {
    /// Whether trajectory recording is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Output directory for trajectories
    #[serde(default = "default_trajectory_dir")]
    pub output_dir: String,
}

impl Default for TrajectoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            output_dir: ".openagents/trajectories".into(),
        }
    }
}

/// Reflexion configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflexionConfig {
    /// Whether reflexion is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum reflexion iterations
    #[serde(default = "default_reflexion_max")]
    pub max_iterations: u32,
}

impl Default for ReflexionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_iterations: 3,
        }
    }
}

/// Cloud configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct CloudConfig {
    /// API endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// API key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Main project configuration
///
/// CONF-001..005: Core project settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    /// Config version
    #[serde(default = "default_version")]
    pub version: u32,

    /// Project identifier
    pub project_id: String,

    /// Default git branch
    #[serde(default = "default_branch")]
    pub default_branch: String,

    /// Work branch (if different from default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_branch: Option<String>,

    /// Default LLM model
    #[serde(default = "default_model")]
    pub default_model: String,

    /// Project root directory
    #[serde(default = "default_root_dir")]
    pub root_dir: String,

    /// Typecheck commands
    #[serde(default)]
    pub typecheck_commands: Vec<String>,

    /// Test commands
    #[serde(default)]
    pub test_commands: Vec<String>,

    /// Sandbox test commands
    #[serde(default)]
    pub sandbox_test_commands: Vec<String>,

    /// E2E test commands
    #[serde(default)]
    pub e2e_commands: Vec<String>,

    /// Allow git push
    #[serde(default = "default_true")]
    pub allow_push: bool,

    /// Allow git force push
    #[serde(default)]
    pub allow_force_push: bool,

    /// Maximum tasks per run
    #[serde(default = "default_max_tasks")]
    pub max_tasks_per_run: u32,

    /// Maximum runtime in minutes
    #[serde(default = "default_max_runtime")]
    pub max_runtime_minutes: u32,

    /// Task ID prefix
    #[serde(default = "default_id_prefix")]
    pub id_prefix: String,

    /// Session storage directory
    #[serde(default = "default_session_dir")]
    pub session_dir: String,

    /// Run log directory
    #[serde(default = "default_run_log_dir")]
    pub run_log_dir: String,

    // Nested configurations
    /// Codex settings
    #[serde(default)]
    pub codex_code: CodexCodeConfig,

    /// Sandbox settings
    #[serde(default)]
    pub sandbox: SandboxConfig,

    /// Parallel execution settings
    #[serde(default)]
    pub parallel_execution: ParallelExecutionConfig,

    /// Trajectory recording settings
    #[serde(default)]
    pub trajectory: TrajectoryConfig,

    /// Healer (self-healing) settings
    #[serde(default)]
    pub healer: HealerConfig,

    /// Reflexion settings
    #[serde(default)]
    pub reflexion: ReflexionConfig,

    /// Failure cleanup settings
    #[serde(default)]
    pub failure_cleanup: FailureCleanupConfig,

    /// Terminal-Bench settings
    #[serde(default)]
    pub tbench: TBenchConfig,

    /// Cloud settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<CloudConfig>,
}

impl ProjectConfig {
    /// Create a new ProjectConfig with defaults
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            version: 1,
            project_id: project_id.into(),
            default_branch: "main".into(),
            work_branch: None,
            default_model: "x-ai/grok-4.1-fast:free".into(),
            root_dir: ".".into(),
            typecheck_commands: vec![],
            test_commands: vec![],
            sandbox_test_commands: vec![],
            e2e_commands: vec![],
            allow_push: true,
            allow_force_push: false,
            max_tasks_per_run: 3,
            max_runtime_minutes: 240,
            id_prefix: "oa".into(),
            session_dir: ".openagents/sessions".into(),
            run_log_dir: ".openagents/run-logs".into(),
            codex_code: CodexCodeConfig::default(),
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

// Default value functions for serde
fn default_true() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

fn default_branch() -> String {
    "main".into()
}

fn default_model() -> String {
    "x-ai/grok-4.1-fast:free".into()
}

fn default_root_dir() -> String {
    ".".into()
}

fn default_max_tasks() -> u32 {
    3
}

fn default_max_runtime() -> u32 {
    240
}

fn default_id_prefix() -> String {
    "oa".into()
}

fn default_session_dir() -> String {
    ".openagents/sessions".into()
}

fn default_run_log_dir() -> String {
    ".openagents/run-logs".into()
}

fn default_max_turns() -> u32 {
    300
}

fn default_sandbox_timeout() -> u64 {
    300_000
}

fn default_max_invocations_session() -> u32 {
    2
}

fn default_max_invocations_subtask() -> u32 {
    1
}

fn default_stuck_threshold() -> u32 {
    2
}

fn default_max_agents() -> u32 {
    2
}

fn default_agent_memory() -> u32 {
    4096
}

fn default_host_memory() -> u32 {
    6144
}

fn default_worktree_timeout() -> u64 {
    30 * 60 * 1000
}

fn default_install_timeout() -> u64 {
    15 * 60 * 1000
}

fn default_install_args() -> Vec<String> {
    vec!["--frozen-lockfile".into()]
}

fn default_merge_threshold() -> u32 {
    4
}

fn default_pr_threshold() -> u32 {
    50
}

fn default_tbench_model() -> String {
    "fm".into()
}

fn default_tbench_suite() -> String {
    "docs/tb-tasks/fm-mini-suite.json".into()
}

fn default_tbench_timeout() -> u32 {
    3600
}

fn default_trajectory_dir() -> String {
    ".openagents/trajectories".into()
}

fn default_reflexion_max() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ProjectConfig::new("test-project");
        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.version, 1);
        assert_eq!(config.default_branch, "main");
        assert!(config.allow_push);
        assert!(!config.allow_force_push);
        assert!(config.codex_code.enabled);
        assert!(config.sandbox.enabled);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = ProjectConfig::new("test");
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.project_id, parsed.project_id);
    }

    #[test]
    fn test_deserialize_minimal() {
        let json = r#"{"projectId": "minimal"}"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.project_id, "minimal");
        assert_eq!(config.version, 1);
        assert_eq!(config.default_branch, "main");
    }

    #[test]
    fn test_permission_modes() {
        let json = r#"{"projectId": "test", "codexCode": {"permissionMode": "plan"}}"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.codex_code.permission_mode, PermissionMode::Plan);
    }

    #[test]
    fn test_sandbox_backends() {
        let json = r#"{"projectId": "test", "sandbox": {"backend": "macos-container"}}"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sandbox.backend, SandboxBackend::MacosContainer);
    }
}
