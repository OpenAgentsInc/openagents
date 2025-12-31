//! Git clone operations using libgit2

use anyhow::{Context, Result, bail};
use git2::{FetchOptions, Progress, RemoteCallbacks, Repository};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// Sanitize a repository identifier to prevent path traversal attacks
///
/// This function validates that the identifier:
/// 1. Does not contain path traversal sequences (../, ..\)
/// 2. Does not start with / or contain absolute path components
/// 3. Only contains safe characters and path separators
///
/// Returns the sanitized identifier or an error if validation fails.
fn sanitize_repository_identifier(identifier: &str) -> Result<PathBuf> {
    // Reject empty identifiers
    if identifier.is_empty() {
        bail!("Repository identifier cannot be empty");
    }

    // Create a path from the identifier
    let path = PathBuf::from(identifier);

    // Validate each component
    for component in path.components() {
        match component {
            Component::Normal(_) => {
                // Normal components are OK
            }
            Component::ParentDir => {
                // Reject parent directory references (..)
                bail!("Repository identifier cannot contain '..' path components");
            }
            Component::RootDir | Component::Prefix(_) => {
                // Reject absolute paths
                bail!("Repository identifier cannot be an absolute path");
            }
            Component::CurDir => {
                // Current directory (.) is technically safe but unnecessary
                bail!("Repository identifier cannot contain '.' path components");
            }
        }
    }

    // Additional validation: ensure the path is still relative
    if path.is_absolute() {
        bail!("Repository identifier must be a relative path");
    }

    Ok(path)
}

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
/// - `file://` - Local filesystem clone
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
/// use gitafter::git::clone_repository;
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

    // Ensure parent directory exists with restrictive permissions
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create workspace directory")?;

        // Set restrictive permissions (0700 = owner only) on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(parent, permissions)
                .context("Failed to set directory permissions")?;
        }
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
        || url.starts_with("file://")
    {
        return Ok(());
    }

    anyhow::bail!(
        "Unsupported URL scheme. Supported: https://, ssh://, git://, file://, or git@host:path"
    );
}

/// Get the default workspace path for cloned repositories
///
/// Creates the workspace directory with restrictive permissions (0700) if it doesn't exist.
pub fn get_workspace_path() -> PathBuf {
    let path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitafter")
        .join("repos");

    // Create workspace with restrictive permissions if it doesn't exist
    if !path.exists() {
        if let Err(e) = std::fs::create_dir_all(&path) {
            eprintln!("Warning: Failed to create workspace directory: {}", e);
            return path;
        }

        // Set restrictive permissions (0700 = owner only) on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o700);
            if let Err(e) = std::fs::set_permissions(&path, permissions) {
                eprintln!("Warning: Failed to set workspace permissions: {}", e);
            }
        }
    }

    path
}

/// Check if a repository is already cloned
pub fn is_repository_cloned(repo_identifier: &str) -> bool {
    match sanitize_repository_identifier(repo_identifier) {
        Ok(sanitized) => {
            let path = get_workspace_path().join(sanitized);
            path.exists() && Repository::open(&path).is_ok()
        }
        Err(_) => false, // Invalid identifier = not cloned
    }
}

/// Get the local path for a cloned repository
///
/// This function sanitizes the repository identifier to prevent path traversal attacks.
/// Returns an error if the identifier contains unsafe path components.
pub fn get_repository_path(repo_identifier: &str) -> Result<PathBuf> {
    let sanitized =
        sanitize_repository_identifier(repo_identifier).context("Invalid repository identifier")?;
    Ok(get_workspace_path().join(sanitized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_path() {
        let path = get_workspace_path();
        assert!(path.to_string_lossy().contains("gitafter"));
        assert!(path.to_string_lossy().contains("repos"));
    }

    #[test]
    fn test_repository_path() {
        let path = get_repository_path("test-repo").unwrap();
        assert!(path.to_string_lossy().contains("test-repo"));
    }

    #[test]
    fn test_repository_path_traversal() {
        // Test that path traversal is rejected
        assert!(get_repository_path("../etc/passwd").is_err());
        assert!(get_repository_path("../../secret").is_err());
        assert!(get_repository_path("/etc/passwd").is_err());
        assert!(get_repository_path("test/../admin").is_err());

        // Test that safe paths work
        assert!(get_repository_path("myrepo").is_ok());
        assert!(get_repository_path("org/repo").is_ok());
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
    fn test_validate_file_url() {
        assert!(validate_clone_url("file:///tmp/repo").is_ok());
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
