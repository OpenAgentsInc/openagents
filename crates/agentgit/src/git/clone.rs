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
/// # Arguments
/// * `url` - The Git URL to clone from
/// * `dest_path` - The destination path to clone to
/// * `progress` - Optional progress tracker
pub fn clone_repository(
    url: &str,
    dest_path: &Path,
    progress: Option<Arc<Mutex<CloneProgress>>>,
) -> Result<Repository> {
    info!("Cloning repository from {} to {:?}", url, dest_path);

    // Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create workspace directory")?;
    }

    // Set up callbacks for progress tracking
    let mut callbacks = RemoteCallbacks::new();

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
}
