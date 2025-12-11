//! Worktree Runner
//!
//! Runs MechaCoder in an isolated git worktree for a single task.
//! This is the integration between worktrees and the orchestrator.

use crate::agent_lock::{acquire_worktree_lock, release_worktree_lock};
use crate::error::{AgentError, AgentResult};
use crate::git::{has_commits_ahead, merge_branch, MergeOptions};
use crate::install_deps::{install_deps, InstallSettings};
use crate::worktree::{
    create_worktree, ensure_valid_worktree, remove_worktree, WorktreeConfig, WorktreeInfo,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Options for running in a worktree
#[derive(Debug, Clone, Default)]
pub struct WorktreeRunnerOptions {
    /// Path to the repository
    pub repo_path: String,
    /// Task ID to run (optional - will be auto-generated if not provided)
    pub task_id: Option<String>,
    /// Session ID (optional - will be auto-generated if not provided)
    pub session_id: Option<String>,
    /// Whether to run in dry-run mode (no actual execution)
    pub dry_run: bool,
    /// Base branch to create worktree from
    pub base_branch: Option<String>,
    /// Test commands to run
    pub test_commands: Vec<String>,
    /// Typecheck commands to run
    pub typecheck_commands: Vec<String>,
    /// Install settings
    pub install_settings: Option<InstallSettings>,
}

/// Result of running in a worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRunResult {
    /// Whether the run succeeded
    pub success: bool,
    /// Task ID that was run
    pub task_id: Option<String>,
    /// Path to the worktree
    pub worktree_path: Option<String>,
    /// Commit SHA if commits were created
    pub commit_sha: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether changes were merged to target branch
    pub merged: bool,
}

impl Default for WorktreeRunResult {
    fn default() -> Self {
        Self {
            success: false,
            task_id: None,
            worktree_path: None,
            commit_sha: None,
            error: None,
            merged: false,
        }
    }
}

impl WorktreeRunResult {
    /// Create a failure result with an error message
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            ..Default::default()
        }
    }

    /// Create a success result
    pub fn ok() -> Self {
        Self {
            success: true,
            ..Default::default()
        }
    }
}

/// Context for a worktree run
pub struct WorktreeRunContext {
    /// Repository path
    pub repo_path: String,
    /// Task ID
    pub task_id: String,
    /// Session ID
    pub session_id: String,
    /// Worktree info
    pub worktree_info: WorktreeInfo,
    /// Target branch for merging
    pub target_branch: String,
    /// OpenAgents directory path
    pub openagents_dir: String,
}

/// Setup a worktree for running
pub fn setup_worktree(options: &WorktreeRunnerOptions) -> AgentResult<WorktreeRunContext> {
    let repo_path = &options.repo_path;
    let openagents_dir = Path::new(repo_path).join(".openagents");
    let openagents_dir_str = openagents_dir.to_str().ok_or_else(|| {
        AgentError::Git("Invalid openagents directory path".to_string())
    })?;

    // Generate task ID if not provided
    let task_id = options
        .task_id
        .clone()
        .unwrap_or_else(|| format!("task-{:x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()));

    // Generate session ID if not provided
    let session_id = options
        .session_id
        .clone()
        .unwrap_or_else(|| format!("worktree-{}", chrono::Utc::now().timestamp_millis()));

    let target_branch = options
        .base_branch
        .clone()
        .unwrap_or_else(|| "main".to_string());

    // Create worktree config
    let worktree_config = WorktreeConfig {
        task_id: task_id.clone(),
        session_id: session_id.clone(),
        base_branch: target_branch.clone(),
        timeout_ms: 240 * 60 * 1000, // 4 hours default
    };

    // Ensure valid worktree
    let worktree_info = ensure_valid_worktree(repo_path, &worktree_config)?;

    Ok(WorktreeRunContext {
        repo_path: repo_path.clone(),
        task_id,
        session_id,
        worktree_info,
        target_branch,
        openagents_dir: openagents_dir_str.to_string(),
    })
}

/// Acquire lock for worktree run
pub fn acquire_worktree_run_lock(ctx: &WorktreeRunContext) -> bool {
    acquire_worktree_lock(&ctx.openagents_dir, &ctx.task_id, &ctx.session_id)
}

/// Release lock for worktree run
pub fn release_worktree_run_lock(ctx: &WorktreeRunContext) {
    release_worktree_lock(&ctx.openagents_dir, &ctx.task_id);
}

/// Copy tasks file to worktree
pub fn copy_tasks_to_worktree(ctx: &WorktreeRunContext) -> AgentResult<()> {
    let src_tasks = Path::new(&ctx.openagents_dir).join("tasks.jsonl");
    let worktree_openagents = Path::new(&ctx.worktree_info.path).join(".openagents");
    let dst_tasks = worktree_openagents.join("tasks.jsonl");

    if !dst_tasks.exists() && src_tasks.exists() {
        fs::create_dir_all(&worktree_openagents).map_err(|e| {
            AgentError::Git(format!("Failed to create worktree openagents dir: {}", e))
        })?;

        fs::copy(&src_tasks, &dst_tasks).map_err(|e| {
            AgentError::Git(format!("Failed to copy tasks.jsonl: {}", e))
        })?;
    }

    Ok(())
}

/// Install dependencies in worktree
pub fn install_worktree_deps(
    ctx: &WorktreeRunContext,
    settings: &InstallSettings,
) -> AgentResult<()> {
    if settings.skip_install {
        return Ok(());
    }

    let result = install_deps(&ctx.worktree_info.path, settings);
    if !result.success {
        return Err(AgentError::Git(
            result.error.unwrap_or_else(|| "Dependency installation failed".to_string()),
        ));
    }

    Ok(())
}

/// Merge worktree changes back to target branch
pub fn merge_worktree_changes(ctx: &WorktreeRunContext) -> AgentResult<Option<String>> {
    // Check if there are commits to merge
    let has_commits = has_commits_ahead(&ctx.worktree_info.path, &ctx.target_branch);

    if !has_commits {
        return Ok(None);
    }

    // Merge changes
    let merge_result = merge_branch(
        &ctx.repo_path,
        &MergeOptions {
            target_branch: ctx.target_branch.clone(),
            source_branch: ctx.worktree_info.branch.clone(),
            push: false, // Don't push from worktree
        },
    );

    if !merge_result.success {
        return Err(AgentError::Git(
            merge_result.error.unwrap_or_else(|| "Merge failed".to_string()),
        ));
    }

    Ok(merge_result.commit_sha)
}

/// Cleanup worktree after run
pub fn cleanup_worktree(ctx: &WorktreeRunContext) {
    // Release lock
    release_worktree_run_lock(ctx);

    // Remove worktree (ignore errors)
    let _ = remove_worktree(&ctx.repo_path, &ctx.task_id);
}

/// Run the full worktree workflow
///
/// This is a high-level function that:
/// 1. Sets up the worktree
/// 2. Acquires lock
/// 3. Copies tasks
/// 4. Installs dependencies
/// 5. (Caller runs orchestrator)
/// 6. Merges changes
/// 7. Cleans up
pub fn run_in_worktree<F>(
    options: WorktreeRunnerOptions,
    run_orchestrator: F,
) -> WorktreeRunResult
where
    F: FnOnce(&WorktreeRunContext) -> AgentResult<bool>,
{
    // Setup worktree
    let ctx = match setup_worktree(&options) {
        Ok(ctx) => ctx,
        Err(e) => {
            return WorktreeRunResult {
                success: false,
                task_id: options.task_id,
                error: Some(format!("Failed to setup worktree: {}", e)),
                ..Default::default()
            };
        }
    };

    let mut result = WorktreeRunResult {
        task_id: Some(ctx.task_id.clone()),
        worktree_path: Some(ctx.worktree_info.path.clone()),
        ..Default::default()
    };

    // Acquire lock
    if !acquire_worktree_run_lock(&ctx) {
        cleanup_worktree(&ctx);
        result.error = Some("Failed to acquire lock - another runner is active".to_string());
        return result;
    }

    // Copy tasks
    if let Err(e) = copy_tasks_to_worktree(&ctx) {
        cleanup_worktree(&ctx);
        result.error = Some(format!("Failed to copy tasks: {}", e));
        return result;
    }

    // Install dependencies
    if let Some(ref settings) = options.install_settings {
        if let Err(e) = install_worktree_deps(&ctx, settings) {
            cleanup_worktree(&ctx);
            result.error = Some(format!("Failed to install dependencies: {}", e));
            return result;
        }
    }

    // Run orchestrator (or dry run)
    if options.dry_run {
        result.success = true;
    } else {
        match run_orchestrator(&ctx) {
            Ok(success) => {
                result.success = success;
            }
            Err(e) => {
                result.error = Some(format!("Orchestrator error: {}", e));
            }
        }
    }

    // Merge changes if successful
    if result.success && !options.dry_run {
        match merge_worktree_changes(&ctx) {
            Ok(Some(sha)) => {
                result.merged = true;
                result.commit_sha = Some(sha);
            }
            Ok(None) => {
                // No commits to merge
            }
            Err(e) => {
                result.error = Some(format!("Merge failed: {}", e));
            }
        }
    }

    // Cleanup
    cleanup_worktree(&ctx);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_run_result_default() {
        let result = WorktreeRunResult::default();
        assert!(!result.success);
        assert!(!result.merged);
        assert!(result.task_id.is_none());
    }

    #[test]
    fn test_worktree_run_result_failed() {
        let result = WorktreeRunResult::failed("test error");
        assert!(!result.success);
        assert_eq!(result.error, Some("test error".to_string()));
    }

    #[test]
    fn test_worktree_run_result_ok() {
        let result = WorktreeRunResult::ok();
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_worktree_runner_options_default() {
        let options = WorktreeRunnerOptions::default();
        assert!(options.repo_path.is_empty());
        assert!(options.task_id.is_none());
        assert!(!options.dry_run);
    }
}
