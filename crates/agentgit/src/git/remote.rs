//! Git remote operations

use anyhow::{anyhow, Result};
use git2::{PushOptions, RemoteCallbacks, Repository};
use std::path::Path;

/// Push a branch to a remote
pub fn push_branch(
    repo_path: &Path,
    remote_name: &str,
    branch_name: &str,
) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    // Find the remote
    let mut remote = repo.find_remote(remote_name)?;

    // Build refspec
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    // Set up callbacks for authentication
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });

    // Push
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    remote.push(&[&refspec], Some(&mut push_opts))?;

    Ok(())
}

/// Fetch from a remote
pub fn fetch_remote(repo_path: &Path, remote_name: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    // Find the remote
    let mut remote = repo.find_remote(remote_name)?;

    // Set up callbacks for authentication
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });

    // Fetch
    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;

    Ok(())
}

/// Add a remote
pub fn add_remote(repo_path: &Path, remote_name: &str, url: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    repo.remote(remote_name, url)?;
    Ok(())
}

/// List all remotes
pub fn list_remotes(repo_path: &Path) -> Result<Vec<String>> {
    let repo = Repository::open(repo_path)?;
    let remotes = repo.remotes()?;

    let remote_names: Vec<String> = remotes
        .iter()
        .flatten()
        .map(|name| name.to_string())
        .collect();

    Ok(remote_names)
}

/// Get the URL of a remote
pub fn get_remote_url(repo_path: &Path, remote_name: &str) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let remote = repo.find_remote(remote_name)?;

    remote.url()
        .ok_or_else(|| anyhow!("Remote URL not found"))
        .map(|s| s.to_string())
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

        (dir, repo)
    }

    #[test]
    fn test_add_remote() {
        let (_dir, repo) = create_test_repo();

        add_remote(repo.path(), "origin", "https://github.com/test/repo.git").unwrap();

        let remotes = list_remotes(repo.path()).unwrap();
        assert!(remotes.contains(&"origin".to_string()));
    }

    #[test]
    fn test_get_remote_url() {
        let (_dir, repo) = create_test_repo();

        let url = "https://github.com/test/repo.git";
        add_remote(repo.path(), "origin", url).unwrap();

        let fetched_url = get_remote_url(repo.path(), "origin").unwrap();
        assert_eq!(fetched_url, url);
    }

    #[test]
    fn test_list_remotes() {
        let (_dir, repo) = create_test_repo();

        add_remote(repo.path(), "origin", "https://github.com/test/repo.git").unwrap();
        add_remote(repo.path(), "upstream", "https://github.com/upstream/repo.git").unwrap();

        let remotes = list_remotes(repo.path()).unwrap();
        assert_eq!(remotes.len(), 2);
        assert!(remotes.contains(&"origin".to_string()));
        assert!(remotes.contains(&"upstream".to_string()));
    }
}
