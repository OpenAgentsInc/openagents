//! OANIX state persistence.
//!
//! Manages session state that persists across restarts.

use crate::manifest::OanixManifest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;
use tokio::fs;
use tracing::{debug, warn};

/// OANIX operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OanixMode {
    /// Idle - waiting for work
    #[default]
    Idle,
    /// Working on a task
    Working,
    /// Provider mode - processing jobs from the swarm
    Provider,
    /// Paused - user requested pause
    Paused,
}

/// An active task being worked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTask {
    /// Task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Task type (issue, directive, job, etc.)
    pub task_type: String,
    /// Progress (0-100)
    pub progress: u8,
    /// When the task started
    pub started_at_epoch_ms: u64,
}

/// Persisted session data (saved to disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    /// Session ID
    pub session_id: String,
    /// Current mode
    pub mode: OanixMode,
    /// Active task (if any)
    pub active_task: Option<ActiveTask>,
    /// Last activity timestamp (epoch ms)
    pub last_activity_ms: u64,
}

/// Full OANIX state (includes non-serializable fields).
pub struct OanixState {
    /// The discovered environment manifest
    pub manifest: OanixManifest,
    /// Unique session identifier
    pub session_id: String,
    /// Current operating mode
    pub mode: OanixMode,
    /// Currently active task
    pub active_task: Option<ActiveTask>,
    /// When the state was last refreshed
    pub last_refresh: Instant,
    /// Path to state file
    state_path: PathBuf,
}

impl OanixState {
    /// Create a new state with the given manifest.
    pub fn new(manifest: OanixManifest) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        let state_path = Self::default_state_path();

        Self {
            manifest,
            session_id,
            mode: OanixMode::Idle,
            active_task: None,
            last_refresh: Instant::now(),
            state_path,
        }
    }

    /// Get the default state file path.
    pub fn default_state_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagents")
            .join("oanix")
            .join("session.json")
    }

    /// Load existing state or create new.
    pub async fn load_or_create(manifest: OanixManifest) -> anyhow::Result<Self> {
        let state_path = Self::default_state_path();

        if state_path.exists() {
            match Self::load_from_file(&state_path, manifest.clone()).await {
                Ok(state) => {
                    debug!("Loaded existing session: {}", state.session_id);
                    return Ok(state);
                }
                Err(e) => {
                    warn!("Failed to load session, creating new: {}", e);
                }
            }
        }

        Ok(Self::new(manifest))
    }

    /// Load state from a file.
    async fn load_from_file(path: &PathBuf, manifest: OanixManifest) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path).await?;
        let persisted: PersistedState = serde_json::from_str(&content)?;

        Ok(Self {
            manifest,
            session_id: persisted.session_id,
            mode: persisted.mode,
            active_task: persisted.active_task,
            last_refresh: Instant::now(),
            state_path: path.clone(),
        })
    }

    /// Save state to disk.
    pub async fn save(&self) -> anyhow::Result<()> {
        let persisted = PersistedState {
            session_id: self.session_id.clone(),
            mode: self.mode,
            active_task: self.active_task.clone(),
            last_activity_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        // Ensure directory exists
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(&persisted)?;
        fs::write(&self.state_path, content).await?;

        debug!("Saved session state to {:?}", self.state_path);
        Ok(())
    }

    /// Update the manifest with fresh discovery.
    pub async fn refresh_manifest(&mut self) -> anyhow::Result<()> {
        self.manifest = crate::boot().await?;
        self.last_refresh = Instant::now();
        Ok(())
    }

    /// Set the current mode.
    pub fn set_mode(&mut self, mode: OanixMode) {
        self.mode = mode;
    }

    /// Start working on a task.
    pub fn start_task(&mut self, id: String, description: String, task_type: String) {
        self.active_task = Some(ActiveTask {
            id,
            description,
            task_type,
            progress: 0,
            started_at_epoch_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });
        self.mode = OanixMode::Working;
    }

    /// Update task progress.
    pub fn update_progress(&mut self, progress: u8) {
        if let Some(task) = &mut self.active_task {
            task.progress = progress.min(100);
        }
    }

    /// Complete the current task.
    pub fn complete_task(&mut self) {
        self.active_task = None;
        self.mode = OanixMode::Idle;
    }

    /// Get the next actionable issue from workspace.
    pub fn next_actionable_issue(&self) -> Option<&crate::manifest::IssueSummary> {
        self.manifest.workspace.as_ref().and_then(|ws| {
            ws.issues
                .iter()
                .find(|issue| issue.status == "open" && !issue.is_blocked)
        })
    }

    /// Check if we should refresh the manifest.
    pub fn needs_refresh(&self, max_age_secs: u64) -> bool {
        self.last_refresh.elapsed().as_secs() > max_age_secs
    }

    /// Check if in provider mode.
    pub fn is_provider_mode(&self) -> bool {
        self.mode == OanixMode::Provider
    }

    /// Check if paused.
    pub fn is_paused(&self) -> bool {
        self.mode == OanixMode::Paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        ComputeManifest, HardwareManifest, IdentityManifest, NetworkManifest,
    };

    fn mock_manifest() -> OanixManifest {
        OanixManifest {
            hardware: HardwareManifest::unknown(),
            compute: ComputeManifest::empty(),
            network: NetworkManifest::offline(),
            identity: IdentityManifest::unknown(),
            workspace: None,
            discovered_at: Instant::now(),
        }
    }

    #[test]
    fn test_state_new() {
        let manifest = mock_manifest();
        let state = OanixState::new(manifest);

        assert_eq!(state.mode, OanixMode::Idle);
        assert!(state.active_task.is_none());
        assert!(!state.session_id.is_empty());
    }

    #[test]
    fn test_task_lifecycle() {
        let manifest = mock_manifest();
        let mut state = OanixState::new(manifest);

        // Start task
        state.start_task("issue-1".into(), "Fix bug".into(), "issue".into());
        assert_eq!(state.mode, OanixMode::Working);
        assert!(state.active_task.is_some());

        // Update progress
        state.update_progress(50);
        assert_eq!(state.active_task.as_ref().unwrap().progress, 50);

        // Complete task
        state.complete_task();
        assert_eq!(state.mode, OanixMode::Idle);
        assert!(state.active_task.is_none());
    }
}
