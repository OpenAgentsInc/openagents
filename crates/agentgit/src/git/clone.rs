//! Git clone operations using libgit2

use anyhow::{Context, Result};
use git2::{FetchOptions, Progress, RemoteCallbacks, Repository};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// Progress callback for clone operations
pub struct CloneProgress {
    pub total_objects: usize,
    pub indexed_objects: usize,
    pub received_objects: usize,
    pub received_bytes: usize,
}

impl CloneProgress {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            total_objects: 0,
            indexed_objects: 0,
            received_objects: 0,
            received_bytes: 0,
        }
    }

    #[allow(dead_code)]
    pub fn percentage(&self) -> f32 {
        if self.total_objects == 0 {
            return 0.0;
        }
        (self.received_objects as f32 / self.total_objects as f32) * 100.0
    }
}

/// Clone a Git repository to the local workspace
///
/// Supports multiple URL schemes:
/// - `https://` - HTTPS clone (no auth)
/// - `ssh://` or `git@` - SSH clone with SSH agent authentication
/// - `git://` - Git protocol (no auth, deprecated)
///
/// # Arguments
/// * `url` - The Git URL to clone from
/// * `dest_path` - The destination path to clone to
/// * `progress` - Optional progress tracker
///
/// # Authentication
/// For SSH URLs, this function uses the system SSH agent for authentication.
/// Ensure your SSH key is added to the agent before cloning private repositories.
///
/// # Examples
/// ```no_run
/// use agentgit::git::clone_repository;
/// use std::path::Path;
///
/// // Clone via HTTPS
/// clone_repository(
///     "https://github.com/user/repo.git",
///     Path::new("/tmp/repo"),
///     None
/// ).expect("clone failed");
///
/// // Clone via SSH (requires SSH agent)
/// clone_repository(
///     "git@github.com:user/repo.git",
///     Path::new("/tmp/repo"),
///     None
/// ).expect("clone failed");
/// ```
pub fn clone_repository(
    url: &str,
    dest_path: &Path,
    progress: Option<Arc<Mutex<CloneProgress>>>,
) -> Result<Repository> {
    info!("Cloning repository from {} to {:?}", url, dest_path);

    // Validate URL scheme
    validate_clone_url(url)?;

    // Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create workspace directory")?;
    }

    // Set up callbacks for progress tracking and authentication
    let mut callbacks = RemoteCallbacks::new();

    // Add SSH authentication callback
    callbacks.credentials(|url_str, username_from_url, allowed_types| {
        debug!(
            "Credentials requested for {} (user: {:?}, types: {:?})",
            url_str, username_from_url, allowed_types
        );

        // For SSH URLs, use SSH agent
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            debug!("Using SSH agent for authentication as user: {}", username);
            return git2::Cred::ssh_key_from_agent(username);
        }

        // For HTTPS, we don't provide credentials (public repos only)
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            debug!("HTTPS authentication required but not configured");
            return Err(git2::Error::from_str(
                "HTTPS authentication not supported. Use SSH URLs for private repositories.",
            ));
        }

        Err(git2::Error::from_str("No suitable authentication method"))
    });

    if let Some(progress_tracker) = progress {
        callbacks.transfer_progress(move |stats: Progress| {
            let mut p = progress_tracker.lock().unwrap();
            p.total_objects = stats.total_objects();
            p.indexed_objects = stats.indexed_objects();
            p.received_objects = stats.received_objects();
            p.received_bytes = stats.received_bytes();

            debug!(
                "Clone progress: {}/{} objects ({:.1}%)",
                stats.received_objects(),
                stats.total_objects(),
                (stats.received_objects() as f32 / stats.total_objects() as f32) * 100.0
            );
            true
        });
    }

    // Configure fetch options
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    // Build clone options
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);

    // Clone the repository
    let repo = builder
        .clone(url, dest_path)
        .context(format!("Failed to clone repository from {}", url))?;

    info!("Successfully cloned repository to {:?}", dest_path);
    Ok(repo)
}

/// Validate that a Git URL has a supported scheme
fn validate_clone_url(url: &str) -> Result<()> {
    // SSH URLs in git@ format
    if url.starts_with("git@") {
        return Ok(());
    }

    // Standard URL schemes
    if url.starts_with("https://")
        || url.starts_with("ssh://")
        || url.starts_with("git://")
    {
        return Ok(());
    }

    anyhow::bail!(
        "Unsupported URL scheme. Supported: https://, ssh://, git://, or git@host:path"
    );
}

/// Get the default workspace path for cloned repositories
pub fn get_workspace_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agentgit")
        .join("repos")
}

/// Check if a repository is already cloned
pub fn is_repository_cloned(repo_identifier: &str) -> bool {
    let path = get_workspace_path().join(repo_identifier);
    path.exists() && Repository::open(&path).is_ok()
}

/// Get the local path for a cloned repository
pub fn get_repository_path(repo_identifier: &str) -> PathBuf {
    get_workspace_path().join(repo_identifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_path() {
        let path = get_workspace_path();
        assert!(path.to_string_lossy().contains("agentgit"));
        assert!(path.to_string_lossy().contains("repos"));
    }

    #[test]
    fn test_repository_path() {
        let path = get_repository_path("test-repo");
        assert!(path.to_string_lossy().contains("test-repo"));
    }

    #[test]
    fn test_validate_https_url() {
        assert!(validate_clone_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_clone_url("https://gitlab.com/user/repo.git").is_ok());
    }

    #[test]
    fn test_validate_ssh_url() {
        assert!(validate_clone_url("git@github.com:user/repo.git").is_ok());
        assert!(validate_clone_url("git@gitlab.com:user/repo.git").is_ok());
        assert!(validate_clone_url("ssh://git@github.com/user/repo.git").is_ok());
    }

    #[test]
    fn test_validate_git_protocol() {
        assert!(validate_clone_url("git://github.com/user/repo.git").is_ok());
    }

    #[test]
    fn test_validate_invalid_url() {
        assert!(validate_clone_url("ftp://example.com/repo.git").is_err());
        assert!(validate_clone_url("http://example.com/repo.git").is_err());
        assert!(validate_clone_url("invalid-url").is_err());
        assert!(validate_clone_url("").is_err());
    }

    #[test]
    fn test_clone_progress_percentage() {
        let mut progress = CloneProgress::new();
        assert_eq!(progress.percentage(), 0.0);

        progress.total_objects = 100;
        progress.received_objects = 50;
        assert_eq!(progress.percentage(), 50.0);

        progress.received_objects = 100;
        assert_eq!(progress.percentage(), 100.0);
    }

    #[test]
    fn test_is_repository_cloned() {
        // Non-existent repo should return false
        assert!(!is_repository_cloned("non-existent-repo-12345"));
    }
}
