//! Git worktree management for parallel agents

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Information about a git worktree for a parallel agent
///
/// Each parallel agent works in its own isolated git worktree, which is a separate
/// working directory linked to the same .git database. This allows multiple agents
/// to work on different branches simultaneously without conflicts.
///
/// # Example
///
/// ```
/// use autopilot::parallel::WorktreeInfo;
/// use std::path::PathBuf;
///
/// let worktree = WorktreeInfo {
///     path: PathBuf::from("/workspace/.worktrees/agent-001"),
///     branch: "agent/001".to_string(),
///     agent_id: "001".to_string(),
/// };
///
/// println!("Agent {} works in {:?} on branch {}",
///     worktree.agent_id, worktree.path, worktree.branch);
/// ```
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Path to the worktree directory
    ///
    /// Example: `/workspace/.worktrees/agent-001`
    pub path: PathBuf,

    /// Git branch name for this worktree
    ///
    /// Example: `agent/001`
    pub branch: String,

    /// Zero-padded agent identifier
    ///
    /// Example: `001`, `002`, `010`
    pub agent_id: String,
}

/// Create git worktrees for N agents
///
/// Creates worktrees at `.worktrees/agent-{N}` with branches `agent/{N}`.
pub fn create_worktrees(project_root: &Path, count: usize) -> Result<Vec<WorktreeInfo>> {
    let worktrees_dir = project_root.join(".worktrees");
    std::fs::create_dir_all(&worktrees_dir)?;

    let mut created = Vec::new();

    for i in 1..=count {
        let agent_id = format!("{:03}", i);
        let worktree_path = worktrees_dir.join(format!("agent-{}", agent_id));
        let branch_name = format!("agent/{}", agent_id);

        if worktree_path.exists() {
            // Already exists, just add to list
            created.push(WorktreeInfo {
                path: worktree_path,
                branch: branch_name,
                agent_id,
            });
            continue;
        }

        // Try to create new worktree with new branch
        let result = Command::new("git")
            .args([
                "worktree", "add",
                &worktree_path.to_string_lossy(),
                "-b", &branch_name,
                "main",
            ])
            .current_dir(project_root)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                created.push(WorktreeInfo {
                    path: worktree_path,
                    branch: branch_name,
                    agent_id,
                });
            }
            Ok(_) => {
                // Branch might already exist, try without -b
                let output = Command::new("git")
                    .args([
                        "worktree", "add",
                        &worktree_path.to_string_lossy(),
                        &branch_name,
                    ])
                    .current_dir(project_root)
                    .output()?;

                if output.status.success() {
                    created.push(WorktreeInfo {
                        path: worktree_path,
                        branch: branch_name,
                        agent_id,
                    });
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("Warning: Failed to create worktree for agent-{}: {}", agent_id, stderr);
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to run git worktree add: {}", e);
            }
        }
    }

    Ok(created)
}

/// Remove all agent worktrees
pub fn remove_worktrees(project_root: &Path) -> Result<()> {
    // First, prune stale worktrees
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(project_root)
        .output();

    // List worktrees and remove agent ones
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_root)
        .output()
        .context("Failed to list worktrees")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktree_path: Option<String> = None;

    for line in stdout.lines() {
        if line.starts_with("worktree ") {
            worktree_path = Some(line.strip_prefix("worktree ").unwrap_or("").to_string());
        } else if line.starts_with("branch ") {
            if let Some(ref path) = worktree_path {
                if path.contains(".worktrees/agent-") {
                    // Remove this worktree
                    let _ = Command::new("git")
                        .args(["worktree", "remove", "--force", path])
                        .current_dir(project_root)
                        .output();
                }
            }
            worktree_path = None;
        }
    }

    // Prune again after removal
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(project_root)
        .output();

    // Remove agent branches
    let output = Command::new("git")
        .args(["branch", "--list", "agent/*"])
        .current_dir(project_root)
        .output()
        .context("Failed to list branches")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let branch = line.trim().trim_start_matches("* ");
        if branch.starts_with("agent/") {
            let _ = Command::new("git")
                .args(["branch", "-D", branch])
                .current_dir(project_root)
                .output();
        }
    }

    // Remove .worktrees directory if empty
    let worktrees_dir = project_root.join(".worktrees");
    if worktrees_dir.exists() {
        let _ = std::fs::remove_dir(&worktrees_dir);
    }

    Ok(())
}

/// List existing agent worktrees
pub fn list_worktrees(project_root: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_root)
        .output()
        .context("Failed to list worktrees")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if line.starts_with("worktree ") {
            current_path = Some(PathBuf::from(line.strip_prefix("worktree ").unwrap_or("")));
        } else if line.starts_with("branch ") {
            current_branch = Some(
                line.strip_prefix("branch refs/heads/")
                    .unwrap_or("")
                    .to_string()
            );
        } else if line.is_empty() {
            // End of entry
            if let (Some(path), Some(branch)) = (&current_path, &current_branch) {
                // Check if this is an agent worktree
                if let Some(filename) = path.file_name() {
                    let name = filename.to_string_lossy();
                    if name.starts_with("agent-") {
                        let agent_id = name.strip_prefix("agent-").unwrap_or("").to_string();
                        worktrees.push(WorktreeInfo {
                            path: path.clone(),
                            branch: branch.clone(),
                            agent_id,
                        });
                    }
                }
            }
            current_path = None;
            current_branch = None;
        }
    }

    // Sort by agent ID
    worktrees.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    Ok(worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_info() {
        let info = WorktreeInfo {
            path: PathBuf::from("/test/.worktrees/agent-001"),
            branch: "agent/001".to_string(),
            agent_id: "001".to_string(),
        };
        assert_eq!(info.agent_id, "001");
    }

    // Note: Full integration tests require a git repository
}
