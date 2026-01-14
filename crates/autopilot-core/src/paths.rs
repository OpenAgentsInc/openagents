use std::path::{Path, PathBuf};

/// Canonical OpenAgents storage paths (ADR-0008).
#[derive(Debug, Clone)]
pub struct OpenAgentsPaths {
    root: PathBuf,
}

impl Default for OpenAgentsPaths {
    fn default() -> Self {
        Self {
            root: openagents_home(),
        }
    }
}

impl OpenAgentsPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir().join(session_id)
    }
}

/// Resolve OPENAGENTS_HOME with fallback to ~/.openagents.
pub fn openagents_home() -> PathBuf {
    if let Ok(root) = std::env::var("OPENAGENTS_HOME") {
        return PathBuf::from(root);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openagents");
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile).join(".openagents");
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
}
