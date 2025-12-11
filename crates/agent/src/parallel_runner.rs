//! Parallel Agent Runner
//!
//! Coordinates N agents running on N isolated git worktrees simultaneously.
//! Each agent works on a different task in its own worktree.
//!
//! Features:
//! - Creates isolated worktrees for each agent
//! - Per-worktree locking to prevent conflicts
//! - Supports direct, queue, and PR merge strategies
//! - Enforces Golden Loop invariants per worktree

use crate::agent_lock::{acquire_worktree_lock, prune_worktree_locks, release_worktree_lock};
use crate::error::{AgentError, AgentResult};
use crate::sandbox_runner::SandboxConfig;
use crate::types::{ClaudeCodeSettings, Task};
use crate::worktree::{
    create_worktree, prune_stale_worktrees, remove_worktree, WorktreeConfig, WorktreeInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Merge strategy for parallel agent results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// Auto-select based on agent count
    #[default]
    Auto,
    /// Direct commit to main (≤4 agents)
    Direct,
    /// Local merge queue (≤50 agents)
    Queue,
    /// Create PRs for human review (>50 agents)
    Pr,
}

/// Parallel runner configuration
#[derive(Debug, Clone)]
pub struct ParallelRunnerConfig {
    /// Path to the repository root
    pub repo_path: String,
    /// Path to .openagents directory
    pub openagents_dir: String,
    /// Maximum number of agents to run in parallel
    pub max_agents: usize,
    /// Tasks to distribute to agents
    pub tasks: Vec<Task>,
    /// Base branch to create worktrees from
    pub base_branch: String,
    /// Session ID for lock tracking
    pub session_id: String,
    /// Merge strategy (auto-selected if Auto)
    pub merge_strategy: MergeStrategy,
    /// Number of agents before switching from direct to queue (when auto)
    pub merge_threshold: usize,
    /// Number of agents before switching from queue to PR (when auto)
    pub pr_threshold: usize,
    /// Timeout per agent in ms
    pub timeout_ms: u64,
    /// Test commands from project.json
    pub test_commands: Option<Vec<String>>,
    /// Typecheck commands from project.json
    pub typecheck_commands: Option<Vec<String>>,
    /// E2E commands from project.json
    pub e2e_commands: Option<Vec<String>>,
    /// Claude Code settings
    pub claude_code: Option<ClaudeCodeSettings>,
    /// Sandbox configuration
    pub sandbox: Option<SandboxConfig>,
    /// Model for subagents
    pub subagent_model: Option<String>,
    /// Use Claude Code only (no fallback)
    pub cc_only: bool,
    /// Allow push after merge
    pub allow_push: bool,
}

impl Default for ParallelRunnerConfig {
    fn default() -> Self {
        Self {
            repo_path: ".".to_string(),
            openagents_dir: ".openagents".to_string(),
            max_agents: 4,
            tasks: Vec::new(),
            base_branch: "main".to_string(),
            session_id: format!("parallel-{}", chrono::Utc::now().timestamp()),
            merge_strategy: MergeStrategy::Auto,
            merge_threshold: 4,
            pr_threshold: 50,
            timeout_ms: 30 * 60 * 1000, // 30 minutes
            test_commands: None,
            typecheck_commands: None,
            e2e_commands: None,
            claude_code: None,
            sandbox: None,
            subagent_model: None,
            cc_only: false,
            allow_push: false,
        }
    }
}

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Waiting to start
    Pending,
    /// Currently executing
    Running,
    /// Successfully completed
    Completed,
    /// Failed with error
    Failed,
}

/// Agent execution slot
#[derive(Debug, Clone)]
pub struct AgentSlot {
    /// Slot ID (same as task ID)
    pub id: String,
    /// Worktree for this agent
    pub worktree: Option<WorktreeInfo>,
    /// Task assigned to this agent
    pub task: Task,
    /// Current status
    pub status: AgentStatus,
    /// Result after completion
    pub result: Option<AgentResult_>,
    /// Start timestamp
    pub started_at: Option<String>,
    /// Completion timestamp
    pub completed_at: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Result of agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult_ {
    /// Whether the agent succeeded
    pub success: bool,
    /// Files modified by the agent
    pub files_modified: Vec<String>,
    /// Commit SHA after merge
    pub commit_sha: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Number of agent turns
    pub turns: Option<u32>,
}

/// Events emitted during parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent started execution
    AgentStarted {
        task_id: String,
        worktree_path: String,
    },
    /// Agent completed successfully
    AgentCompleted {
        task_id: String,
        success: bool,
        files_modified: Vec<String>,
    },
    /// Agent failed
    AgentFailed { task_id: String, error: String },
    /// Worktree created
    WorktreeCreated { task_id: String, path: String },
    /// Worktree removed
    WorktreeRemoved { task_id: String },
    /// Merge started
    MergeStarted {
        task_id: String,
        strategy: MergeStrategy,
    },
    /// Merge completed
    MergeCompleted {
        task_id: String,
        commit_sha: Option<String>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Merge Strategy Selection
// ─────────────────────────────────────────────────────────────────────────────

/// Select the optimal merge strategy based on agent count
///
/// - ≤merge_threshold (default 4): Direct commit to main
/// - ≤pr_threshold (default 50): Local merge queue
/// - >pr_threshold: PR flow for rate limiting
pub fn select_merge_strategy(config: &ParallelRunnerConfig) -> MergeStrategy {
    if config.merge_strategy != MergeStrategy::Auto {
        return config.merge_strategy;
    }

    let agent_count = config.tasks.len().min(config.max_agents);

    if agent_count <= config.merge_threshold {
        MergeStrategy::Direct
    } else if agent_count <= config.pr_threshold {
        MergeStrategy::Queue
    } else {
        MergeStrategy::Pr
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Git Operations
// ─────────────────────────────────────────────────────────────────────────────

/// Run a git command in a directory
fn run_git(cwd: &str, args: &[&str]) -> AgentResult<(i32, String, String)> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(AgentError::Io)?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let exit_code = output.status.code().unwrap_or(1);

    Ok((exit_code, stdout, stderr))
}

/// Merge agent branch to main using fast-forward merge
pub fn merge_agent_branch(
    repo_path: &str,
    branch: &str,
    worktree_path: Option<&str>,
) -> AgentResult<Option<String>> {
    // Resolve branch or commit
    let target = {
        let (exit_code, stdout, _) = run_git(repo_path, &["rev-parse", branch])?;
        if exit_code == 0 && !stdout.is_empty() {
            branch.to_string()
        } else if let Some(wt_path) = worktree_path {
            let (exit_code, stdout, _) = run_git(wt_path, &["rev-parse", "HEAD"])?;
            if exit_code == 0 && !stdout.is_empty() {
                stdout
            } else {
                return Err(AgentError::Git(format!(
                    "Could not resolve branch {}",
                    branch
                )));
            }
        } else {
            return Err(AgentError::Git(format!(
                "Could not resolve branch {}",
                branch
            )));
        }
    };

    // Check for dirty working tree
    let (_, status_out, _) = run_git(repo_path, &["status", "--porcelain"])?;
    if !status_out.is_empty() {
        return Err(AgentError::Git(
            "Main working tree is dirty before merge; aborting to avoid conflicts.".to_string(),
        ));
    }

    // Save initial HEAD for rollback
    let (_, initial_head, _) = run_git(repo_path, &["rev-parse", "HEAD"])?;

    // Fetch latest main
    let _ = run_git(repo_path, &["fetch", "origin", "main"]);

    // Checkout main
    run_git(repo_path, &["checkout", "main"])?;

    // Pull latest
    let _ = run_git(repo_path, &["pull", "--ff-only", "origin", "main"]);

    // Try fast-forward merge
    let (ff_exit, _, _) = run_git(repo_path, &["merge", "--ff-only", &target])?;

    if ff_exit != 0 {
        // Fallback to regular merge
        let (merge_exit, _, merge_stderr) = run_git(repo_path, &["merge", &target])?;
        if merge_exit != 0 {
            // Cleanup and fail
            let _ = run_git(repo_path, &["merge", "--abort"]);
            let _ = run_git(repo_path, &["reset", "--hard", &initial_head]);
            return Err(AgentError::Git(format!(
                "Merge failed: {}",
                merge_stderr
            )));
        }
    }

    // Get the merge commit SHA
    let (_, head, _) = run_git(repo_path, &["rev-parse", "HEAD"])?;
    Ok(if head.is_empty() { None } else { Some(head) })
}

// ─────────────────────────────────────────────────────────────────────────────
// Agent Execution
// ─────────────────────────────────────────────────────────────────────────────

/// Run a single agent in a worktree
///
/// Implements Golden Loop invariants:
/// - Skip init script (worktree inherits from main repo)
/// - Force new subtasks (avoid stale state)
/// - Pre-assign task (prevent all agents picking same task)
/// - Don't push from worktree (merge handles push)
pub fn run_agent_in_worktree(
    worktree: &WorktreeInfo,
    task: &Task,
    _config: &ParallelRunnerConfig,
) -> AgentResult<AgentResult_> {
    let worktree_openagents_dir = Path::new(&worktree.path).join(".openagents");

    // Ensure .openagents directory exists
    if !worktree_openagents_dir.exists() {
        std::fs::create_dir_all(&worktree_openagents_dir).map_err(AgentError::Io)?;
    }

    // TODO: Implement actual orchestrator execution
    // For now, return a placeholder result
    Ok(AgentResult_ {
        success: true,
        files_modified: Vec::new(),
        commit_sha: None,
        error: None,
        turns: Some(0),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Parallel Runner Core
// ─────────────────────────────────────────────────────────────────────────────

/// Run N agents in parallel on N isolated worktrees
///
/// Flow:
/// 1. Prune stale worktrees and locks
/// 2. Create worktrees for each task (up to maxAgents)
/// 3. Acquire per-worktree locks
/// 4. Spawn agents in parallel
/// 5. Collect results
/// 6. Merge changes based on strategy
/// 7. Cleanup worktrees
pub fn run_parallel_agents(config: &ParallelRunnerConfig) -> AgentResult<Vec<AgentSlot>> {
    let strategy = select_merge_strategy(config);

    // Step 1: Prune stale resources
    let _ = prune_stale_worktrees(&config.repo_path, 3600000);
    prune_worktree_locks(&config.openagents_dir);

    // Step 2: Limit tasks to max_agents
    let tasks_to_run: Vec<_> = config.tasks.iter().take(config.max_agents).collect();
    let mut slots: Vec<AgentSlot> = tasks_to_run
        .iter()
        .map(|task| AgentSlot {
            id: task.id.clone(),
            worktree: None,
            task: (*task).clone(),
            status: AgentStatus::Pending,
            result: None,
            started_at: None,
            completed_at: None,
            error: None,
        })
        .collect();

    // Step 3: Create worktrees and acquire locks
    for slot in &mut slots {
        let worktree_config = WorktreeConfig {
            task_id: slot.task.id.clone(),
            session_id: config.session_id.clone(),
            base_branch: config.base_branch.clone(),
            timeout_ms: config.timeout_ms,
        };

        // Create worktree
        match create_worktree(&config.repo_path, &worktree_config) {
            Ok(worktree) => {
                slot.worktree = Some(worktree);
            }
            Err(e) => {
                slot.status = AgentStatus::Failed;
                slot.error = Some(format!("Worktree creation failed: {}", e));
                continue;
            }
        }

        // Acquire lock
        if !acquire_worktree_lock(&config.openagents_dir, &slot.task.id, &config.session_id) {
            slot.status = AgentStatus::Failed;
            slot.error = Some(format!("Could not acquire lock for {}", slot.task.id));
        }
    }

    // Step 4: Run agents (sequentially for now, TODO: parallelize)
    for slot in &mut slots {
        if slot.status == AgentStatus::Failed {
            continue;
        }

        let Some(ref worktree) = slot.worktree else {
            slot.status = AgentStatus::Failed;
            slot.error = Some("No worktree available".to_string());
            continue;
        };

        slot.status = AgentStatus::Running;
        slot.started_at = Some(chrono::Utc::now().to_rfc3339());

        match run_agent_in_worktree(worktree, &slot.task, config) {
            Ok(result) => {
                slot.status = if result.success {
                    AgentStatus::Completed
                } else {
                    AgentStatus::Failed
                };
                if let Some(ref err) = result.error {
                    slot.error = Some(err.clone());
                }
                slot.result = Some(result);
            }
            Err(e) => {
                slot.status = AgentStatus::Failed;
                slot.error = Some(e.to_string());
            }
        }

        slot.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    // Step 5: Merge based on strategy (only for successful agents)
    // Collect indices of successful slots to avoid borrow issues
    let successful_indices: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, s)| s.status == AgentStatus::Completed && s.worktree.is_some())
        .map(|(i, _)| i)
        .collect();

    for idx in successful_indices {
        let (worktree_branch, worktree_path, task_id, task_title, task_desc) = {
            let slot = &slots[idx];
            let wt = slot.worktree.as_ref().unwrap();
            (
                wt.branch.clone(),
                wt.path.clone(),
                slot.task.id.clone(),
                slot.task.title.clone(),
                slot.task.description.clone(),
            )
        };

        match strategy {
            MergeStrategy::Direct | MergeStrategy::Queue => {
                match merge_agent_branch(&config.repo_path, &worktree_branch, Some(&worktree_path))
                {
                    Ok(commit_sha) => {
                        if let Some(ref mut result) = slots[idx].result {
                            result.commit_sha = commit_sha;
                        }
                    }
                    Err(e) => {
                        slots[idx].error = Some(e.to_string());
                    }
                }
            }
            MergeStrategy::Pr => {
                // Push branch and create PR
                let _ = run_git(&config.repo_path, &["push", "-u", "origin", &worktree_branch]);

                // Create PR using gh CLI
                let title = format!("{}: {}", task_id, task_title);
                let body = format!(
                    "## Summary\n\n{}\n\n🤖 Generated with [OpenAgents](https://openagents.com)",
                    task_desc.as_deref().unwrap_or("Automated task completion")
                );

                let output = Command::new("gh")
                    .args([
                        "pr",
                        "create",
                        "--title",
                        &title,
                        "--body",
                        &body,
                        "--head",
                        &worktree_branch,
                    ])
                    .current_dir(&config.repo_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();

                if let Ok(output) = output {
                    if output.status.success() {
                        let pr_url =
                            String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if let Some(ref mut result) = slots[idx].result {
                            result.commit_sha = Some(pr_url);
                        }
                    }
                }
            }
            MergeStrategy::Auto => unreachable!("Auto should be resolved by select_merge_strategy"),
        }
    }

    // Step 6: Cleanup worktrees and release locks
    for slot in &slots {
        release_worktree_lock(&config.openagents_dir, &slot.task.id);

        if slot.worktree.is_some() {
            let _ = remove_worktree(&config.repo_path, &slot.task.id);
        }
    }

    Ok(slots)
}

// ─────────────────────────────────────────────────────────────────────────────
// Resource Guardrails
// ─────────────────────────────────────────────────────────────────────────────

/// Calculate a safe max agent count based on host memory
pub fn calculate_safe_max_agents(
    total_mem_bytes: u64,
    per_agent_memory_mb: u64,
    host_reserve_mb: u64,
) -> usize {
    let total_mb = total_mem_bytes / (1024 * 1024);
    let available = total_mb.saturating_sub(host_reserve_mb);
    let safe = available / per_agent_memory_mb;
    (safe as usize).max(1)
}

/// Parallel execution configuration from project.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelExecutionConfig {
    /// Whether parallel execution is enabled
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Maximum number of agents to run in parallel
    #[serde(default)]
    pub max_agents: Option<usize>,
    /// Merge strategy
    #[serde(default)]
    pub merge_strategy: Option<MergeStrategy>,
    /// Threshold for switching from direct to queue
    #[serde(default)]
    pub merge_threshold: Option<usize>,
    /// Threshold for switching from queue to PR
    #[serde(default)]
    pub pr_threshold: Option<usize>,
    /// Timeout per worktree in ms
    #[serde(default)]
    pub worktree_timeout: Option<u64>,
    /// Memory per agent in MB
    #[serde(default)]
    pub per_agent_memory_mb: Option<u64>,
    /// Reserved host memory in MB
    #[serde(default)]
    pub host_memory_reserve_mb: Option<u64>,
    /// Install timeout in ms
    #[serde(default)]
    pub install_timeout_ms: Option<u64>,
    /// Install arguments
    #[serde(default)]
    pub install_args: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_strategy_default() {
        let config = ParallelRunnerConfig::default();
        assert_eq!(config.merge_strategy, MergeStrategy::Auto);
    }

    #[test]
    fn test_select_merge_strategy_direct() {
        let config = ParallelRunnerConfig {
            tasks: vec![Task::default(); 2],
            merge_threshold: 4,
            pr_threshold: 50,
            ..Default::default()
        };
        assert_eq!(select_merge_strategy(&config), MergeStrategy::Direct);
    }

    #[test]
    fn test_select_merge_strategy_queue() {
        let config = ParallelRunnerConfig {
            tasks: vec![Task::default(); 10],
            max_agents: 10,
            merge_threshold: 4,
            pr_threshold: 50,
            ..Default::default()
        };
        assert_eq!(select_merge_strategy(&config), MergeStrategy::Queue);
    }

    #[test]
    fn test_select_merge_strategy_pr() {
        let config = ParallelRunnerConfig {
            tasks: vec![Task::default(); 100],
            max_agents: 100,
            merge_threshold: 4,
            pr_threshold: 50,
            ..Default::default()
        };
        assert_eq!(select_merge_strategy(&config), MergeStrategy::Pr);
    }

    #[test]
    fn test_select_merge_strategy_explicit() {
        let config = ParallelRunnerConfig {
            tasks: vec![Task::default(); 100],
            merge_strategy: MergeStrategy::Direct,
            ..Default::default()
        };
        assert_eq!(select_merge_strategy(&config), MergeStrategy::Direct);
    }

    #[test]
    fn test_calculate_safe_max_agents() {
        // 16GB total, 4GB per agent, 6GB reserved = (16-6)/4 = 2.5 = 2
        let result = calculate_safe_max_agents(16 * 1024 * 1024 * 1024, 4096, 6144);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_calculate_safe_max_agents_minimum() {
        // Very low memory should still return 1
        let result = calculate_safe_max_agents(1024 * 1024 * 1024, 4096, 6144);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_agent_slot_creation() {
        let task = Task::default();
        let slot = AgentSlot {
            id: "test".to_string(),
            worktree: None,
            task,
            status: AgentStatus::Pending,
            result: None,
            started_at: None,
            completed_at: None,
            error: None,
        };
        assert_eq!(slot.status, AgentStatus::Pending);
    }

    #[test]
    fn test_agent_result_success() {
        let result = AgentResult_ {
            success: true,
            files_modified: vec!["src/main.rs".to_string()],
            commit_sha: Some("abc123".to_string()),
            error: None,
            turns: Some(5),
        };
        assert!(result.success);
        assert_eq!(result.files_modified.len(), 1);
    }

    #[test]
    fn test_parallel_execution_config_defaults() {
        let config = ParallelExecutionConfig::default();
        assert!(config.enabled.is_none());
        assert!(config.max_agents.is_none());
    }

    #[test]
    fn test_merge_strategy_serialization() {
        let strategy = MergeStrategy::Direct;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"direct\"");
    }

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEvent::AgentStarted {
            task_id: "task-1".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("agent_started"));
        assert!(json.contains("task-1"));
    }

    #[test]
    fn test_parallel_runner_config_default() {
        let config = ParallelRunnerConfig::default();
        assert_eq!(config.max_agents, 4);
        assert_eq!(config.merge_threshold, 4);
        assert_eq!(config.pr_threshold, 50);
        assert!(!config.cc_only);
    }
}
