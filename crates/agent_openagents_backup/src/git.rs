//! Git Helper Functions
//!
//! Composable git operations for orchestrator runners.

use crate::error::{AgentError, AgentResult};
use std::process::{Command, Output, Stdio};

/// Result of a git command
#[derive(Debug, Clone)]
pub struct GitResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl GitResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Git error reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitErrorKind {
    CommandFailed,
    MergeFailed,
    CheckoutFailed,
    PushFailed,
    PullFailed,
    FetchFailed,
    DirtyWorktree,
}

/// Options for merge operation
#[derive(Debug, Clone)]
pub struct MergeOptions {
    pub target_branch: String,
    pub source_branch: String,
    pub push: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            target_branch: "main".to_string(),
            source_branch: String::new(),
            push: true,
        }
    }
}

/// Result of a merge operation
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub error: Option<String>,
}

/// Run a git command
pub fn run_git(cwd: &str, args: &[&str]) -> GitResult {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(out) => GitResult {
            exit_code: out.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        },
        Err(e) => GitResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: e.to_string(),
        },
    }
}

/// Run git command and return Result
pub fn run_git_checked(cwd: &str, args: &[&str]) -> AgentResult<GitResult> {
    let result = run_git(cwd, args);
    if result.exit_code != 0 {
        return Err(AgentError::Git(format!(
            "Git command failed: git {} - {}",
            args.join(" "),
            result.stderr
        )));
    }
    Ok(result)
}

/// Check if working tree has uncommitted changes
pub fn is_working_tree_dirty(cwd: &str) -> bool {
    let result = run_git(cwd, &["status", "--porcelain"]);
    !result.stdout.is_empty()
}

/// Get the current HEAD commit SHA
pub fn get_head_sha(cwd: &str) -> AgentResult<String> {
    let result = run_git_checked(cwd, &["rev-parse", "HEAD"])?;
    Ok(result.stdout)
}

/// Get the current branch name
pub fn get_current_branch(cwd: &str) -> AgentResult<String> {
    let result = run_git_checked(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(result.stdout)
}

/// Check if there are commits ahead of a base branch
pub fn has_commits_ahead(worktree_path: &str, base_branch: &str) -> bool {
    let result = run_git(
        worktree_path,
        &["rev-list", "--count", &format!("origin/{}..HEAD", base_branch)],
    );
    result.stdout.parse::<u32>().unwrap_or(0) > 0
}

/// Get list of staged files
pub fn get_staged_files(cwd: &str) -> Vec<String> {
    let result = run_git(cwd, &["diff", "--cached", "--name-only"]);
    if result.exit_code != 0 {
        return vec![];
    }
    result
        .stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Fetch a branch from remote
pub fn fetch_branch(cwd: &str, branch: &str, remote: &str) -> AgentResult<()> {
    let result = run_git(cwd, &["fetch", remote, branch]);
    if result.exit_code != 0 && !result.stderr.contains("couldn't find remote ref") {
        return Err(AgentError::Git(format!(
            "Failed to fetch {}/{}: {}",
            remote, branch, result.stderr
        )));
    }
    Ok(())
}

/// Pull a branch from remote
pub fn pull_branch(cwd: &str, branch: &str, remote: &str) -> AgentResult<()> {
    // Try fast-forward first
    let result = run_git(cwd, &["pull", "--ff-only", remote, branch]);
    if result.exit_code == 0 {
        return Ok(());
    }

    // Fall back to regular pull
    let result = run_git(cwd, &["pull", remote, branch]);
    if result.exit_code != 0 {
        return Err(AgentError::Git(format!(
            "Failed to pull {}/{}: {}",
            remote, branch, result.stderr
        )));
    }
    Ok(())
}

/// Checkout a branch
pub fn checkout_branch(cwd: &str, branch: &str) -> AgentResult<()> {
    let result = run_git(cwd, &["checkout", branch]);
    if result.exit_code != 0 {
        return Err(AgentError::Git(format!(
            "Failed to checkout {}: {}",
            branch, result.stderr
        )));
    }
    Ok(())
}

/// Push a branch to remote
pub fn push_branch(cwd: &str, branch: &str, remote: &str) -> AgentResult<()> {
    let result = run_git(cwd, &["push", remote, branch]);
    if result.exit_code != 0 {
        return Err(AgentError::Git(format!(
            "Failed to push to {}/{}: {}",
            remote, branch, result.stderr
        )));
    }
    Ok(())
}

/// Merge a source branch into target branch
pub fn merge_branch(repo_path: &str, options: &MergeOptions) -> MergeResult {
    // Checkout target branch
    let result = run_git(repo_path, &["checkout", &options.target_branch]);
    if result.exit_code != 0 {
        return MergeResult {
            success: false,
            commit_sha: None,
            error: Some(format!("Checkout {} failed: {}", options.target_branch, result.stderr)),
        };
    }

    // Pull latest
    let result = run_git(repo_path, &["pull", "--ff-only", "origin", &options.target_branch]);
    if result.exit_code != 0 {
        let result = run_git(repo_path, &["pull", "origin", &options.target_branch]);
        if result.exit_code != 0 {
            return MergeResult {
                success: false,
                commit_sha: None,
                error: Some(format!("Pull failed: {}", result.stderr)),
            };
        }
    }

    // Try fast-forward merge first
    let result = run_git(repo_path, &["merge", "--ff-only", &options.source_branch]);
    if result.exit_code != 0 {
        // Fall back to regular merge
        let result = run_git(repo_path, &["merge", "--no-edit", &options.source_branch]);
        if result.exit_code != 0 {
            return MergeResult {
                success: false,
                commit_sha: None,
                error: Some(format!("Merge failed: {}", result.stderr)),
            };
        }
    }

    // Get commit SHA
    let result = run_git(repo_path, &["rev-parse", "HEAD"]);
    let commit_sha = result.stdout;

    // Push if requested
    if options.push {
        let result = run_git(repo_path, &["push", "origin", &options.target_branch]);
        if result.exit_code != 0 {
            return MergeResult {
                success: false,
                commit_sha: Some(commit_sha),
                error: Some(format!("Push failed: {}", result.stderr)),
            };
        }
    }

    MergeResult {
        success: true,
        commit_sha: Some(commit_sha),
        error: None,
    }
}

/// Abort any in-progress merge or rebase
pub fn abort_merge_state(repo_path: &str, reset_to_sha: Option<&str>) {
    let _ = run_git(repo_path, &["merge", "--abort"]);
    let _ = run_git(repo_path, &["rebase", "--abort"]);
    if let Some(sha) = reset_to_sha {
        let _ = run_git(repo_path, &["reset", "--hard", sha]);
    }
}

/// Create a commit with the given message
pub fn create_commit(cwd: &str, message: &str) -> AgentResult<String> {
    let result = run_git_checked(cwd, &["commit", "-m", message])?;
    get_head_sha(cwd)
}

/// Stage all changes
pub fn stage_all(cwd: &str) -> AgentResult<()> {
    run_git_checked(cwd, &["add", "-A"])?;
    Ok(())
}

/// Stage specific files
pub fn stage_files(cwd: &str, files: &[&str]) -> AgentResult<()> {
    let mut args = vec!["add"];
    args.extend(files);
    run_git_checked(cwd, &args)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_result_success() {
        let result = GitResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: String::new(),
        };
        assert!(result.success());

        let result = GitResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error".to_string(),
        };
        assert!(!result.success());
    }

    #[test]
    fn test_merge_options_default() {
        let opts = MergeOptions::default();
        assert_eq!(opts.target_branch, "main");
        assert!(opts.push);
    }
}
