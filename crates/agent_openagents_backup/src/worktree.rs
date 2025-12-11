//! Git Worktree Management Service
//!
//! Manages isolated git worktrees for parallel agent execution.
//! Each agent gets its own worktree with a unique branch.
//!
//! Directory structure:
//! ```text
//! repo/
//! ├── .git/                    # Shared object database
//! ├── .worktrees/              # Agent worktrees
//! │   ├── oa-abc123/           # Worktree for task oa-abc123
//! │   ├── oa-def456/           # Worktree for task oa-def456
//! │   └── ...
//! └── [main working tree]
//! ```

use crate::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Directory name for worktrees
const WORKTREES_DIR: &str = ".worktrees";
/// Branch prefix for agent worktrees
const BRANCH_PREFIX: &str = "agent/";

/// Worktree configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeConfig {
    /// Task ID
    pub task_id: String,
    /// Session ID
    pub session_id: String,
    /// Base branch to create worktree from
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_base_branch() -> String {
    "main".to_string()
}

fn default_timeout_ms() -> u64 {
    30 * 60 * 1000 // 30 minutes
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            session_id: String::new(),
            base_branch: default_base_branch(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

/// Information about a worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Task ID this worktree belongs to
    pub task_id: String,
    /// Absolute path to worktree
    pub path: String,
    /// Branch name (agent/{taskId})
    pub branch: String,
    /// ISO timestamp when created
    pub created_at: String,
    /// Base branch this was created from
    pub base_branch: String,
}

/// Worktree validation issue types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorktreeIssue {
    /// Missing .git file
    MissingGit { message: String },
    /// Branch mismatch
    BranchMismatch { expected: String, actual: String },
    /// Detached HEAD state
    DetachedHead { message: String },
    /// Missing worktree directory
    MissingDirectory { message: String },
}

/// Result of validating a worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeValidationResult {
    /// Whether the worktree is valid
    pub valid: bool,
    /// List of issues found
    pub issues: Vec<WorktreeIssue>,
}

/// Git command result
struct GitResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Run a git command and return the result
fn run_git(repo_path: &str, args: &[&str]) -> AgentResult<GitResult> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AgentError::Git(format!("Failed to run git: {}", e)))?;

    Ok(GitResult {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// Get the worktrees directory path
pub fn get_worktrees_dir(repo_path: &str) -> PathBuf {
    Path::new(repo_path).join(WORKTREES_DIR)
}

/// Get the path for a specific worktree
pub fn get_worktree_path(repo_path: &str, task_id: &str) -> PathBuf {
    get_worktrees_dir(repo_path).join(task_id)
}

/// Get the branch name for a task
pub fn get_branch_name(task_id: &str) -> String {
    format!("{}{}", BRANCH_PREFIX, task_id)
}

/// Create a new worktree for a task
pub fn create_worktree(repo_path: &str, config: &WorktreeConfig) -> AgentResult<WorktreeInfo> {
    let worktree_path = get_worktree_path(repo_path, &config.task_id);
    let worktree_path_str = worktree_path.to_str().ok_or_else(|| {
        AgentError::Git("Invalid worktree path".to_string())
    })?;
    let branch_name = get_branch_name(&config.task_id);

    // Check if worktree directory already exists
    if worktree_path.exists() {
        let existing = list_worktrees(repo_path)?;
        let is_registered = existing.iter().any(|wt| {
            PathBuf::from(&wt.path).canonicalize().ok()
                == worktree_path.canonicalize().ok()
        });

        if is_registered {
            return Err(AgentError::Git(format!(
                "Worktree already exists at {}",
                worktree_path_str
            )));
        }

        // Orphaned directory - clean it up
        fs::remove_dir_all(&worktree_path).ok();
    }

    // Ensure .worktrees directory exists
    let worktrees_dir = get_worktrees_dir(repo_path);
    if !worktrees_dir.exists() {
        fs::create_dir_all(&worktrees_dir).map_err(|e| {
            AgentError::Git(format!("Failed to create worktrees directory: {}", e))
        })?;
    }

    // Fetch latest from remote (ignore errors)
    let _ = run_git(repo_path, &["fetch", "origin", &config.base_branch]);

    // Create worktree with new branch
    let result = run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            &branch_name,
            worktree_path_str,
            &format!("origin/{}", config.base_branch),
        ],
    )?;

    if result.exit_code != 0 {
        // If branch already exists, try without -b flag
        if result.stderr.contains("already exists") {
            let retry_result = run_git(
                repo_path,
                &["worktree", "add", worktree_path_str, &branch_name],
            )?;

            if retry_result.exit_code != 0 {
                return Err(AgentError::Git(format!(
                    "Failed to create worktree: {}",
                    retry_result.stderr
                )));
            }
        } else {
            return Err(AgentError::Git(format!(
                "Failed to create worktree: {}",
                result.stderr
            )));
        }
    }

    Ok(WorktreeInfo {
        task_id: config.task_id.clone(),
        path: worktree_path_str.to_string(),
        branch: branch_name,
        created_at: chrono::Utc::now().to_rfc3339(),
        base_branch: config.base_branch.clone(),
    })
}

/// Remove a worktree and its branch
pub fn remove_worktree(repo_path: &str, task_id: &str) -> AgentResult<()> {
    let worktree_path = get_worktree_path(repo_path, task_id);
    let worktree_path_str = worktree_path.to_str().ok_or_else(|| {
        AgentError::Git("Invalid worktree path".to_string())
    })?;
    let branch_name = get_branch_name(task_id);

    // Remove worktree
    let remove_result = run_git(
        repo_path,
        &["worktree", "remove", "--force", worktree_path_str],
    )?;

    if remove_result.exit_code != 0 && !remove_result.stderr.contains("is not a working tree") {
        return Err(AgentError::Git(format!(
            "Failed to remove worktree: {}",
            remove_result.stderr
        )));
    }

    // Delete the branch (ignore errors)
    let _ = run_git(repo_path, &["branch", "-D", &branch_name]);

    // Prune worktree entries
    let _ = run_git(repo_path, &["worktree", "prune"]);

    Ok(())
}

/// List all agent worktrees in the repository
pub fn list_worktrees(repo_path: &str) -> AgentResult<Vec<WorktreeInfo>> {
    let result = run_git(repo_path, &["worktree", "list", "--porcelain"])?;

    if result.exit_code != 0 {
        return Err(AgentError::Git(format!(
            "Failed to list worktrees: {}",
            result.stderr
        )));
    }

    let mut worktrees = Vec::new();
    let entries: Vec<&str> = result.stdout.split("\n\n").filter(|s| !s.is_empty()).collect();

    for entry in entries {
        let lines: Vec<&str> = entry.lines().collect();
        let mut worktree_path = String::new();
        let mut branch = String::new();

        for line in lines {
            if let Some(path) = line.strip_prefix("worktree ") {
                worktree_path = path.to_string();
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                branch = b.to_string();
            }
        }

        // Only include agent worktrees
        if worktree_path.contains(WORKTREES_DIR) && branch.starts_with(BRANCH_PREFIX) {
            let task_id = branch.strip_prefix(BRANCH_PREFIX).unwrap_or(&branch);
            worktrees.push(WorktreeInfo {
                task_id: task_id.to_string(),
                path: worktree_path,
                branch,
                created_at: String::new(),
                base_branch: "main".to_string(),
            });
        }
    }

    Ok(worktrees)
}

/// Prune stale worktrees
pub fn prune_stale_worktrees(repo_path: &str, max_age_ms: u64) -> AgentResult<u32> {
    // First, run git worktree prune
    let _ = run_git(repo_path, &["worktree", "prune"]);

    // Get all worktrees
    let worktrees = list_worktrees(repo_path)?;
    let mut pruned = 0u32;

    // Remove orphaned directories
    let registered_paths: std::collections::HashSet<PathBuf> = worktrees
        .iter()
        .filter_map(|wt| PathBuf::from(&wt.path).canonicalize().ok())
        .collect();

    let worktrees_dir = get_worktrees_dir(repo_path);
    if worktrees_dir.exists() {
        if let Ok(entries) = fs::read_dir(&worktrees_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Ok(canonical) = entry.path().canonicalize() {
                        if !registered_paths.contains(&canonical) {
                            if fs::remove_dir_all(entry.path()).is_ok() {
                                pruned += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Check each worktree for staleness
    for worktree in worktrees {
        let worktree_path = Path::new(&worktree.path);

        if !worktree_path.exists() {
            continue;
        }

        // Check modification time
        if let Ok(metadata) = fs::metadata(worktree_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed.as_millis() as u64 > max_age_ms {
                        if remove_worktree(repo_path, &worktree.task_id).is_ok() {
                            pruned += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(pruned)
}

/// Check if a worktree exists for a task
pub fn worktree_exists(repo_path: &str, task_id: &str) -> bool {
    get_worktree_path(repo_path, task_id).exists()
}

/// Get info about a specific worktree
pub fn get_worktree_info(repo_path: &str, task_id: &str) -> AgentResult<WorktreeInfo> {
    let worktree_path = get_worktree_path(repo_path, task_id);
    let branch_name = get_branch_name(task_id);

    if !worktree_path.exists() {
        return Err(AgentError::Git(format!(
            "Worktree not found for task {}",
            task_id
        )));
    }

    let worktree_path_str = worktree_path.to_str().ok_or_else(|| {
        AgentError::Git("Invalid worktree path".to_string())
    })?;

    // Get the current branch
    let result = run_git(worktree_path_str, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = if result.exit_code == 0 {
        result.stdout
    } else {
        branch_name
    };

    Ok(WorktreeInfo {
        task_id: task_id.to_string(),
        path: worktree_path_str.to_string(),
        branch,
        created_at: String::new(),
        base_branch: "main".to_string(),
    })
}

/// Validate a worktree for corruption issues
pub fn validate_worktree(repo_path: &str, task_id: &str) -> AgentResult<WorktreeValidationResult> {
    let worktree_path = get_worktree_path(repo_path, task_id);
    let expected_branch = get_branch_name(task_id);
    let mut issues = Vec::new();

    // Check if worktree directory exists
    if !worktree_path.exists() {
        issues.push(WorktreeIssue::MissingDirectory {
            message: format!("Worktree directory not found at {:?}", worktree_path),
        });
        return Ok(WorktreeValidationResult {
            valid: false,
            issues,
        });
    }

    // Check for .git file
    let git_path = worktree_path.join(".git");
    if !git_path.exists() {
        issues.push(WorktreeIssue::MissingGit {
            message: format!("Missing .git file in worktree at {:?}", worktree_path),
        });
        return Ok(WorktreeValidationResult {
            valid: false,
            issues,
        });
    }

    // Check if .git is a file (valid worktree) not a directory
    if git_path.is_dir() {
        issues.push(WorktreeIssue::MissingGit {
            message: format!(
                ".git is a directory instead of file at {:?} - not a valid worktree",
                worktree_path
            ),
        });
        return Ok(WorktreeValidationResult {
            valid: false,
            issues,
        });
    }

    let worktree_path_str = worktree_path.to_str().ok_or_else(|| {
        AgentError::Git("Invalid worktree path".to_string())
    })?;

    // Check current branch
    let branch_result = run_git(worktree_path_str, &["rev-parse", "--abbrev-ref", "HEAD"])?;

    if branch_result.exit_code != 0 {
        issues.push(WorktreeIssue::MissingGit {
            message: format!("Git error in worktree: {}", branch_result.stderr),
        });
        return Ok(WorktreeValidationResult {
            valid: false,
            issues,
        });
    }

    let current_branch = branch_result.stdout;

    // Check for detached HEAD
    if current_branch == "HEAD" {
        issues.push(WorktreeIssue::DetachedHead {
            message: "Worktree is in detached HEAD state".to_string(),
        });
    }

    // Check for branch mismatch
    if current_branch != expected_branch && current_branch != "HEAD" {
        issues.push(WorktreeIssue::BranchMismatch {
            expected: expected_branch,
            actual: current_branch,
        });
    }

    Ok(WorktreeValidationResult {
        valid: issues.is_empty(),
        issues,
    })
}

/// Repair a corrupted worktree by removing and recreating it
pub fn repair_worktree(repo_path: &str, config: &WorktreeConfig) -> AgentResult<WorktreeInfo> {
    let worktree_path = get_worktree_path(repo_path, &config.task_id);
    let branch_name = get_branch_name(&config.task_id);

    // Step 1: Try to cleanly remove existing worktree
    if let Some(path_str) = worktree_path.to_str() {
        let _ = run_git(repo_path, &["worktree", "remove", "--force", path_str]);
    }

    // If still exists, manually remove
    if worktree_path.exists() {
        fs::remove_dir_all(&worktree_path).ok();
    }

    // Step 2: Prune stale worktree entries
    let _ = run_git(repo_path, &["worktree", "prune"]);

    // Step 3: Try to delete the branch
    let _ = run_git(repo_path, &["branch", "-D", &branch_name]);

    // Step 4: Recreate the worktree
    create_worktree(repo_path, config).map_err(|e| {
        AgentError::Git(format!("Failed to recreate worktree after repair: {}", e))
    })
}

/// Validate worktree and repair if necessary
pub fn ensure_valid_worktree(repo_path: &str, config: &WorktreeConfig) -> AgentResult<WorktreeInfo> {
    let worktree_path = get_worktree_path(repo_path, &config.task_id);

    // If worktree doesn't exist, create it
    if !worktree_path.exists() {
        return create_worktree(repo_path, config);
    }

    // Validate existing worktree
    let validation = validate_worktree(repo_path, &config.task_id)?;

    if validation.valid {
        return get_worktree_info(repo_path, &config.task_id);
    }

    // Repair if invalid
    repair_worktree(repo_path, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_worktrees_dir() {
        let dir = get_worktrees_dir("/repo");
        assert_eq!(dir, PathBuf::from("/repo/.worktrees"));
    }

    #[test]
    fn test_get_worktree_path() {
        let path = get_worktree_path("/repo", "task-123");
        assert_eq!(path, PathBuf::from("/repo/.worktrees/task-123"));
    }

    #[test]
    fn test_get_branch_name() {
        assert_eq!(get_branch_name("task-123"), "agent/task-123");
        assert_eq!(get_branch_name("oa-abc"), "agent/oa-abc");
    }

    #[test]
    fn test_worktree_config_default() {
        let config = WorktreeConfig::default();
        assert_eq!(config.base_branch, "main");
        assert_eq!(config.timeout_ms, 30 * 60 * 1000);
    }

    #[test]
    fn test_worktree_validation_result() {
        let result = WorktreeValidationResult {
            valid: true,
            issues: vec![],
        };
        assert!(result.valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_worktree_issue_serialization() {
        let issue = WorktreeIssue::BranchMismatch {
            expected: "agent/task-1".to_string(),
            actual: "main".to_string(),
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("branch_mismatch"));
        assert!(json.contains("agent/task-1"));
    }
}
