//! Git branch operations

use anyhow::{Result, anyhow};
use git2::{BranchType, Repository};
use std::path::Path;

/// Create a new branch from the current HEAD
///
/// Returns the name of the created branch
pub fn create_branch(repo_path: &Path, branch_name: &str) -> Result<String> {
    let repo = Repository::open(repo_path)?;

    // Get current HEAD commit
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;

    // Create the branch
    let branch = repo.branch(branch_name, &commit, false)?;

    let branch_name = branch
        .name()?
        .ok_or_else(|| anyhow!("Branch name is not valid UTF-8"))?
        .to_string();

    Ok(branch_name)
}

/// Switch to an existing branch
pub fn checkout_branch(repo_path: &Path, branch_name: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    // Find the branch
    let branch = repo.find_branch(branch_name, BranchType::Local)?;
    let reference = branch.get();

    // Get the tree for the branch
    let tree = reference.peel_to_tree()?;

    // Checkout the tree
    repo.checkout_tree(tree.as_object(), None)?;

    // Update HEAD to point to the branch
    repo.set_head(&format!("refs/heads/{}", branch_name))?;

    Ok(())
}

/// List all local branches
pub fn list_branches(repo_path: &Path) -> Result<Vec<String>> {
    let repo = Repository::open(repo_path)?;
    let branches = repo.branches(Some(BranchType::Local))?;

    let mut branch_names = Vec::new();
    for branch in branches {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            branch_names.push(name.to_string());
        }
    }

    Ok(branch_names)
}

/// Delete a local branch
pub fn delete_branch(repo_path: &Path, branch_name: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    let mut branch = repo.find_branch(branch_name, BranchType::Local)?;
    branch.delete()?;

    Ok(())
}

/// Get the current branch name
pub fn current_branch(repo_path: &Path) -> Result<String> {
    let repo = Repository::open(repo_path)?;

    let head = repo.head()?;
    if !head.is_branch() {
        return Err(anyhow!("HEAD is not pointing to a branch (detached HEAD)"));
    }

    let branch_name = head
        .shorthand()
        .ok_or_else(|| anyhow!("Branch name is not valid UTF-8"))?
        .to_string();

    Ok(branch_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_create_branch() {
        let (_dir, repo) = create_test_repo();

        let branch_name = create_branch(repo.path(), "feature").unwrap();
        assert_eq!(branch_name, "feature");

        // Verify branch exists
        assert!(repo.find_branch("feature", BranchType::Local).is_ok());
    }

    #[test]
    fn test_list_branches() {
        let (_dir, repo) = create_test_repo();

        create_branch(repo.path(), "feature1").unwrap();
        create_branch(repo.path(), "feature2").unwrap();

        let branches = list_branches(repo.path()).unwrap();
        assert!(branches.len() >= 2);
        assert!(branches.contains(&"feature1".to_string()));
        assert!(branches.contains(&"feature2".to_string()));
    }

    #[test]
    fn test_current_branch() {
        let (_dir, repo) = create_test_repo();

        let current = current_branch(repo.path()).unwrap();
        // Git default branch depends on system config (main or master)
        assert!(
            current == "main" || current == "master",
            "expected main or master, got {}",
            current
        );
    }

    #[test]
    fn test_checkout_branch() {
        let (_dir, repo) = create_test_repo();

        create_branch(repo.path(), "feature").unwrap();
        checkout_branch(repo.path(), "feature").unwrap();

        let current = current_branch(repo.path()).unwrap();
        assert_eq!(current, "feature");
    }

    #[test]
    fn test_delete_branch() {
        let (_dir, repo) = create_test_repo();

        create_branch(repo.path(), "temp").unwrap();
        assert!(repo.find_branch("temp", BranchType::Local).is_ok());

        delete_branch(repo.path(), "temp").unwrap();
        assert!(repo.find_branch("temp", BranchType::Local).is_err());
    }
}
