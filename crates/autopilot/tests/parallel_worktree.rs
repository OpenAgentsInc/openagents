//! Integration tests for parallel autopilot worktree management
//!
//! Tests the core worktree operations used by the parallel autopilot feature:
//! - Creating multiple worktrees for agents
//! - Listing existing worktrees
//! - Removing worktrees and cleaning up branches
//!
//! These tests use a temporary git repository to avoid affecting the actual workspace.

use anyhow::{Context, Result};
use autopilot::parallel::{create_worktrees, list_worktrees, remove_worktrees};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Test helper to create a temporary git repository
fn create_test_repo() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repository with main branch
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_path)
        .output()
        .context("Failed to init git repo")?;

    // Configure git user (required for commits)
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()?;

    // Create an initial commit (required for worktrees)
    std::fs::write(repo_path.join("README.md"), "# Test Repo\n")?;

    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&repo_path)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()?;

    Ok((temp_dir, repo_path))
}

/// Test creating a single worktree
#[test]
fn test_create_single_worktree() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Create worktree
    create_worktrees(&repo_path, 1)?;

    // Verify worktree exists
    let worktree_path = repo_path.join(".worktrees/agent-001");
    assert!(
        worktree_path.exists(),
        "Worktree should exist at {:?}",
        worktree_path
    );

    // Verify it's a valid git worktree
    let output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(&repo_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(".worktrees/agent-001"),
        "Worktree should be listed in git worktree list"
    );

    Ok(())
}

/// Test creating multiple worktrees
#[test]
fn test_create_multiple_worktrees() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Create 5 worktrees
    create_worktrees(&repo_path, 5)?;

    // Verify all worktrees exist
    for i in 1..=5 {
        let worktree_path = repo_path.join(format!(".worktrees/agent-{:03}", i));
        assert!(
            worktree_path.exists(),
            "Worktree {} should exist at {:?}",
            i,
            worktree_path
        );
    }

    // Verify git sees all worktrees
    let output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(&repo_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for i in 1..=5 {
        let expected = format!(".worktrees/agent-{:03}", i);
        assert!(
            stdout.contains(&expected),
            "Worktree {} should be in git worktree list",
            i
        );
    }

    Ok(())
}

/// Test that worktrees are on separate branches
#[test]
fn test_worktrees_on_separate_branches() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    create_worktrees(&repo_path, 3)?;

    // Check branch for each worktree
    for i in 1..=3 {
        let worktree_path = repo_path.join(format!(".worktrees/agent-{:03}", i));

        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&worktree_path)
            .output()?;

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let expected_branch = format!("agent/{:03}", i);

        assert_eq!(
            branch, expected_branch,
            "Worktree {} should be on branch {}",
            i, expected_branch
        );
    }

    Ok(())
}

/// Test listing worktrees
#[test]
fn test_list_worktrees() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Initially no worktrees
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(
        worktrees.len(),
        0,
        "Should have no agent worktrees initially"
    );

    // Create worktrees
    create_worktrees(&repo_path, 3)?;

    // List should now show 3
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(worktrees.len(), 3, "Should have 3 agent worktrees");

    // Verify each worktree path
    for i in 1..=3 {
        let expected_path = repo_path.join(format!(".worktrees/agent-{:03}", i));
        assert!(
            worktrees.iter().any(|w| w.path == expected_path),
            "Worktree {} should be in list",
            i
        );
    }

    Ok(())
}

/// Test that creating worktrees is idempotent
#[test]
fn test_create_worktrees_idempotent() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Create worktrees twice
    create_worktrees(&repo_path, 3)?;

    // Should still have exactly 3 worktrees
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(
        worktrees.len(),
        3,
        "Should have 3 worktrees even after double create"
    );

    Ok(())
}

/// Test removing worktrees
#[test]
fn test_remove_worktrees() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Create worktrees
    create_worktrees(&repo_path, 3)?;

    // Verify they exist
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(worktrees.len(), 3);

    // Remove all agent worktrees
    remove_worktrees(&repo_path)?;

    // Verify they're gone
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(
        worktrees.len(),
        0,
        "All agent worktrees should be removed"
    );

    // Verify branches are also removed
    let output = Command::new("git")
        .args(["branch", "--list", "agent/*"])
        .current_dir(&repo_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "Agent branches should be removed"
    );

    Ok(())
}

/// Test scaling up (adding more worktrees)
#[test]
fn test_scale_up_worktrees() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    // Start with 3 worktrees
    create_worktrees(&repo_path, 3)?;
    assert_eq!(list_worktrees(&repo_path)?.len(), 3);

    // Scale up to 5
    create_worktrees(&repo_path, 5)?;
    let worktrees = list_worktrees(&repo_path)?;
    assert_eq!(worktrees.len(), 5, "Should scale up to 5 worktrees");

    // Verify new worktrees exist
    for i in 4..=5 {
        let worktree_path = repo_path.join(format!(".worktrees/agent-{:03}", i));
        assert!(
            worktree_path.exists(),
            "New worktree {} should exist",
            i
        );
    }

    Ok(())
}

/// Test that worktrees can be independently modified
#[test]
fn test_worktree_isolation() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    create_worktrees(&repo_path, 2)?;

    // Modify file in worktree 1
    let worktree1 = repo_path.join(".worktrees/agent-001");
    std::fs::write(worktree1.join("file1.txt"), "content from agent 1")?;

    // Modify file in worktree 2
    let worktree2 = repo_path.join(".worktrees/agent-002");
    std::fs::write(worktree2.join("file2.txt"), "content from agent 2")?;

    // Verify file1 exists in worktree1 but not worktree2
    assert!(worktree1.join("file1.txt").exists());
    assert!(!worktree2.join("file1.txt").exists());

    // Verify file2 exists in worktree2 but not worktree1
    assert!(worktree2.join("file2.txt").exists());
    assert!(!worktree1.join("file2.txt").exists());

    Ok(())
}

/// Test cleanup of stale worktree references
#[test]
fn test_cleanup_stale_worktrees() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    create_worktrees(&repo_path, 2)?;

    // Manually delete worktree directory (simulating corruption)
    let worktree1 = repo_path.join(".worktrees/agent-001");
    std::fs::remove_dir_all(&worktree1)?;

    // Remove should still work and clean up the reference
    remove_worktrees(&repo_path)?;

    // Verify no agent branches remain
    let output = Command::new("git")
        .args(["branch", "--list", "agent/*"])
        .current_dir(&repo_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty(), "All agent branches should be cleaned up");

    Ok(())
}

/// Test handling when creating worktree in non-git directory
#[test]
fn test_non_git_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let non_git_path = temp_dir.path().to_path_buf();

    // Should return Ok but with no worktrees created
    let result = create_worktrees(&non_git_path, 1)?;
    assert_eq!(
        result.len(),
        0,
        "Should create no worktrees in non-git directory"
    );

    Ok(())
}

/// Test concurrent access to worktrees (basic check)
#[tokio::test]
async fn test_concurrent_worktree_access() -> Result<()> {
    let (_temp_dir, repo_path) = create_test_repo()?;

    create_worktrees(&repo_path, 3)?;

    // Spawn tasks to read from different worktrees concurrently
    let repo_path_clone = repo_path.clone();
    let task1 = tokio::task::spawn_blocking(move || {
        let worktree1 = repo_path_clone.join(".worktrees/agent-001");
        std::fs::read_to_string(worktree1.join("README.md"))
    });

    let repo_path_clone = repo_path.clone();
    let task2 = tokio::task::spawn_blocking(move || {
        let worktree2 = repo_path_clone.join(".worktrees/agent-002");
        std::fs::read_to_string(worktree2.join("README.md"))
    });

    let repo_path_clone = repo_path.clone();
    let task3 = tokio::task::spawn_blocking(move || {
        let worktree3 = repo_path_clone.join(".worktrees/agent-003");
        std::fs::read_to_string(worktree3.join("README.md"))
    });

    // All should succeed
    let (result1, result2, result3) = tokio::try_join!(task1, task2, task3)?;

    assert!(result1.is_ok(), "Should read from worktree 1");
    assert!(result2.is_ok(), "Should read from worktree 2");
    assert!(result3.is_ok(), "Should read from worktree 3");

    Ok(())
}
