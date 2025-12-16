//! Tool execution context with cancellation support.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;

/// A token that can be used to cancel tool execution.
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: watch::Receiver<bool>,
}

impl CancellationToken {
    /// Create a new cancellation token and its trigger.
    pub fn new() -> (Self, CancellationTrigger) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = watch::channel(false);

        let token = Self {
            cancelled: cancelled.clone(),
            notify: rx,
        };

        let trigger = CancellationTrigger {
            cancelled,
            notify: tx,
        };

        (token, trigger)
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Wait until cancellation is requested.
    ///
    /// Returns immediately if already cancelled.
    pub async fn cancelled(&mut self) {
        if self.is_cancelled() {
            return;
        }

        // Wait for the notification
        let _ = self.notify.changed().await;
    }

    /// Check cancellation and return error if cancelled.
    pub fn check(&self) -> Result<(), crate::ToolError> {
        if self.is_cancelled() {
            Err(crate::ToolError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new().0
    }
}

/// Handle to trigger cancellation.
pub struct CancellationTrigger {
    cancelled: Arc<AtomicBool>,
    notify: watch::Sender<bool>,
}

impl CancellationTrigger {
    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        let _ = self.notify.send(true);
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Context provided to tools during execution.
#[derive(Clone)]
pub struct ToolContext {
    /// The working directory for file operations.
    pub working_dir: PathBuf,

    /// Cancellation token to check for abort requests.
    pub cancellation: CancellationToken,

    /// Session ID for permission tracking.
    pub session_id: Option<String>,

    /// Additional metadata for the tool execution.
    pub metadata: serde_json::Value,
}

impl ToolContext {
    /// Create a new tool context with the given working directory.
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        let provided = working_dir.into();
        let canonical = provided.canonicalize().unwrap_or_else(|_| provided.clone());
        Self {
            working_dir: canonical,
            cancellation: CancellationToken::default(),
            session_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a context with cancellation support.
    pub fn with_cancellation(
        working_dir: impl Into<PathBuf>,
        cancellation: CancellationToken,
    ) -> Self {
        let provided = working_dir.into();
        let canonical = provided.canonicalize().unwrap_or_else(|_| provided.clone());
        Self {
            working_dir: canonical,
            cancellation,
            session_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Check cancellation and return error if cancelled.
    pub fn check_cancelled(&self) -> Result<(), crate::ToolError> {
        self.cancellation.check()
    }

    /// Resolve a path relative to the working directory.
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let expanded = shellexpand::tilde(path).to_string();
        let path = PathBuf::from(&expanded);

        if path.is_absolute() {
            path
        } else {
            self.working_dir.join(path)
        }
    }

    /// Check if a path is within the working directory or its subdirectories.
    pub fn is_path_allowed(&self, path: &PathBuf) -> bool {
        // Normalize both paths
        let working = self
            .working_dir
            .canonicalize()
            .unwrap_or_else(|_| self.working_dir.clone());

        let resolved = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => normalize_with_base(&working, path),
        };

        resolved.starts_with(&working)
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

/// Normalize a path, resolving `.`/`..` components relative to a base when the
/// target may not exist (canonicalize would fail).
fn normalize_with_base(base: &Path, path: &Path) -> PathBuf {
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        base.to_path_buf()
    };

    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => {
                normalized = PathBuf::from(prefix.as_os_str());
            }
            Component::RootDir => {
                normalized = PathBuf::from(comp.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cancellation_token() {
        let (token, trigger) = CancellationToken::new();

        assert!(!token.is_cancelled());
        assert!(!trigger.is_cancelled());

        trigger.cancel();

        assert!(token.is_cancelled());
        assert!(trigger.is_cancelled());
    }

    #[test]
    fn test_cancellation_check() {
        let (token, trigger) = CancellationToken::new();

        assert!(token.check().is_ok());

        trigger.cancel();

        assert!(token.check().is_err());
    }

    #[tokio::test]
    async fn test_cancellation_wait() {
        let (mut token, trigger) = CancellationToken::new();

        // Spawn a task to cancel after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            trigger.cancel();
        });

        // This should complete when cancelled
        token.cancelled().await;

        assert!(token.is_cancelled());
    }

    #[test]
    fn test_context_resolve_path() {
        let ctx = ToolContext::new("/home/user/project");

        // Relative path
        assert_eq!(
            ctx.resolve_path("src/main.rs"),
            PathBuf::from("/home/user/project/src/main.rs")
        );

        // Absolute path
        assert_eq!(
            ctx.resolve_path("/etc/config"),
            PathBuf::from("/etc/config")
        );
    }

    #[test]
    fn test_is_path_allowed_nonexistent_inside_workspace() {
        let base = tempdir().unwrap();
        let workspace = base.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let ctx = ToolContext::new(&workspace);
        let target = ctx.resolve_path("new_file.txt");
        assert!(ctx.is_path_allowed(&target));
    }

    #[test]
    fn test_is_path_allowed_rejects_escape() {
        let base = tempdir().unwrap();
        let workspace = base.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let ctx = ToolContext::new(&workspace);
        let target = ctx.resolve_path("../outside.txt");
        assert!(!ctx.is_path_allowed(&target));
    }
}
