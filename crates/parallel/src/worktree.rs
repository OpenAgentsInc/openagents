//! Git worktree management for isolated agent execution
//!
//! Implements PAR-010..013: Worktree isolation

use crate::{ParallelError, ParallelResult};
use git2::Repository;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Manages git worktrees for parallel agent execution
pub struct WorktreeManager {
    /// Path to the main repository
    repo_path: PathBuf,
    /// Base directory for worktrees
    worktree_base: PathBuf,
    /// Active worktrees
    active_worktrees: Vec<WorktreeInfo>,
}

/// Information about a worktree
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Unique identifier for this worktree
    pub id: String,
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Branch name for this worktree
    pub branch: String,
    /// Whether this worktree is currently in use
    pub in_use: bool,
}

impl WorktreeManager {
    /// Create a new worktree manager
    ///
    /// PAR-010: Create isolated worktrees for each agent
    pub fn new(repo_path: impl AsRef<Path>) -> ParallelResult<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();
        let worktree_base = repo_path.join(".worktrees");

        // Ensure worktree base directory exists
        if !worktree_base.exists() {
            std::fs::create_dir_all(&worktree_base)?;
        }

        Ok(Self {
            repo_path,
            worktree_base,
            active_worktrees: Vec::new(),
        })
    }

    /// Create a new worktree for an agent
    ///
    /// PAR-011: Manage worktree lifecycle (create)
    pub fn create_worktree(&mut self, agent_id: &str) -> ParallelResult<WorktreeInfo> {
        let repo = Repository::open(&self.repo_path)?;

        // Generate unique branch name
        let branch_name = format!("agent/{}", agent_id);
        let worktree_path = self.worktree_base.join(agent_id);

        info!("Creating worktree for agent {} at {:?}", agent_id, worktree_path);

        // Get HEAD commit
        let head = repo.head()?;
        let head_commit = head.peel_to_commit()?;

        // Create new branch from HEAD
        let _branch = repo.branch(&branch_name, &head_commit, false)
            .map_err(|e| {
                if e.code() == git2::ErrorCode::Exists {
                    // Branch already exists, get it
                    debug!("Branch {} already exists, reusing", branch_name);
                    return ParallelError::worktree(format!("Branch {} already exists", branch_name));
                }
                ParallelError::GitError(e)
            })?;

        // Use git command since git2 worktree support is limited
        let output = std::process::Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-B", &branch_name])
            .arg(&worktree_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ParallelError::worktree(format!(
                "Failed to create worktree: {}",
                stderr
            )));
        }

        let info = WorktreeInfo {
            id: agent_id.to_string(),
            path: worktree_path,
            branch: branch_name,
            in_use: true,
        };

        self.active_worktrees.push(info.clone());

        Ok(info)
    }

    /// Remove a worktree
    ///
    /// PAR-011: Manage worktree lifecycle (cleanup)
    pub fn remove_worktree(&mut self, agent_id: &str) -> ParallelResult<()> {
        let worktree_path = self.worktree_base.join(agent_id);

        info!("Removing worktree for agent {} at {:?}", agent_id, worktree_path);

        // Remove worktree using git command
        let output = std::process::Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "remove", "--force"])
            .arg(&worktree_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to remove worktree cleanly: {}", stderr);

            // Try to remove directory manually
            if worktree_path.exists() {
                std::fs::remove_dir_all(&worktree_path)?;
            }
        }

        // Prune worktree references
        let _ = std::process::Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "prune"])
            .output();

        // Remove from active list
        self.active_worktrees.retain(|w| w.id != agent_id);

        Ok(())
    }

    /// Merge worktree changes back to main branch
    ///
    /// PAR-012: Merge completed work back to main
    pub fn merge_to_main(&self, agent_id: &str, commit_message: &str) -> ParallelResult<String> {
        let worktree = self.active_worktrees
            .iter()
            .find(|w| w.id == agent_id)
            .ok_or_else(|| ParallelError::worktree(format!("Worktree {} not found", agent_id)))?;

        info!("Merging worktree {} to main", agent_id);

        // First, commit any changes in the worktree
        let _ = std::process::Command::new("git")
            .current_dir(&worktree.path)
            .args(["add", "-A"])
            .output()?;

        let _ = std::process::Command::new("git")
            .current_dir(&worktree.path)
            .args(["commit", "-m", commit_message, "--allow-empty"])
            .output()?;

        // Get the commit SHA
        let rev_output = std::process::Command::new("git")
            .current_dir(&worktree.path)
            .args(["rev-parse", "HEAD"])
            .output()?;

        let commit_sha = String::from_utf8_lossy(&rev_output.stdout).trim().to_string();

        // Merge to main
        let merge_output = std::process::Command::new("git")
            .current_dir(&self.repo_path)
            .args(["merge", &worktree.branch, "--no-ff", "-m"])
            .arg(format!("Merge {} from agent {}", commit_sha, agent_id))
            .output()?;

        if !merge_output.status.success() {
            let stderr = String::from_utf8_lossy(&merge_output.stderr);

            // Check for merge conflicts
            if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
                // Get list of conflicted files
                let status_output = std::process::Command::new("git")
                    .current_dir(&self.repo_path)
                    .args(["diff", "--name-only", "--diff-filter=U"])
                    .output()?;

                let conflict_files: Vec<String> = String::from_utf8_lossy(&status_output.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect();

                // Abort the merge
                let _ = std::process::Command::new("git")
                    .current_dir(&self.repo_path)
                    .args(["merge", "--abort"])
                    .output();

                return Err(ParallelError::merge_conflict(conflict_files));
            }

            return Err(ParallelError::worktree(format!(
                "Failed to merge: {}",
                stderr
            )));
        }

        Ok(commit_sha)
    }

    /// Get all active worktrees
    pub fn active_worktrees(&self) -> &[WorktreeInfo] {
        &self.active_worktrees
    }

    /// Clean up all worktrees
    pub fn cleanup_all(&mut self) -> ParallelResult<()> {
        let ids: Vec<String> = self.active_worktrees.iter().map(|w| w.id.clone()).collect();

        for id in ids {
            if let Err(e) = self.remove_worktree(&id) {
                warn!("Failed to remove worktree {}: {}", id, e);
            }
        }

        Ok(())
    }
}

impl Drop for WorktreeManager {
    fn drop(&mut self) {
        if let Err(e) = self.cleanup_all() {
            warn!("Failed to cleanup worktrees on drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["init"])
            .output()
            .unwrap();

        // Configure git
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "-A"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_worktree_manager_creation() {
        let (_temp_dir, repo_path) = setup_test_repo();
        let manager = WorktreeManager::new(&repo_path).unwrap();
        assert!(manager.active_worktrees.is_empty());
    }
}
