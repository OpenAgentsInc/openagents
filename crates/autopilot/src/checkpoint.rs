//! Session checkpoint for resuming autopilot sessions.
//!
//! Enables session resume across:
//! - Full Auto toggle off/on
//! - Application restarts
//! - Claude SDK conversation resume

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use agent_client_protocol_schema as acp;

use crate::claude::ClaudeEvent;
use crate::startup::{ClaudeModel, LogLine, StartupPhase};
use crate::verification::TerminationChecklist;

/// Current checkpoint format version.
pub const CHECKPOINT_VERSION: u32 = 3;

/// Session checkpoint for resume functionality.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    /// Format version for migration support.
    pub version: u32,

    /// Unique session identifier.
    pub session_id: String,

    /// When this checkpoint was created.
    pub checkpoint_time: DateTime<Local>,

    /// When the original session started.
    pub original_start_time: DateTime<Local>,

    /// Current phase of the session.
    pub phase: StartupPhase,

    /// Elapsed time when phase started (for timing reconstruction).
    pub phase_started_offset: f32,

    /// Current iteration number (for fix loops).
    pub iteration: u32,

    /// Which Claude model is being used.
    pub model: ClaudeModel,

    /// Claude SDK session ID for plan phase (for API resume).
    pub claude_session_id: Option<String>,

    /// Claude SDK session ID for execution phase.
    pub exec_session_id: Option<String>,

    /// Claude SDK session ID for review phase.
    pub review_session_id: Option<String>,

    /// Claude SDK session ID for fix phase.
    pub fix_session_id: Option<String>,

    /// Events from the planning phase.
    pub claude_events: Vec<ClaudeEvent>,

    /// Full accumulated text from planning phase.
    pub claude_full_text: String,

    /// Events from the execution phase.
    pub exec_events: Vec<ClaudeEvent>,

    /// Full accumulated text from execution phase.
    pub exec_full_text: String,

    /// Events from the review phase.
    pub review_events: Vec<ClaudeEvent>,

    /// Full accumulated text from review phase.
    pub review_full_text: String,

    /// Events from the fix phase.
    pub fix_events: Vec<ClaudeEvent>,

    /// Full accumulated text from fix phase.
    pub fix_full_text: String,

    /// Cursor for plan events (for incremental delivery).
    pub plan_cursor: usize,

    /// Cursor for exec events.
    pub exec_cursor: usize,

    /// Cursor for review events.
    pub review_cursor: usize,

    /// Cursor for fix events.
    pub fix_cursor: usize,

    /// Cursor for ACP events (unified stream).
    #[serde(default)]
    pub acp_cursor: usize,

    /// Unified ACP event buffer.
    #[serde(default)]
    pub acp_events: Vec<acp::SessionNotification>,

    /// ACP tool id counter.
    #[serde(default)]
    pub acp_tool_counter: u64,

    /// All log lines.
    pub lines: Vec<LogLine>,

    /// Path to the plan markdown file, if written.
    pub plan_path: Option<PathBuf>,

    /// Last verification checklist result.
    pub last_checklist: Option<TerminationChecklist>,

    /// Working directory for the session.
    pub working_dir: PathBuf,

    /// Whether the session was force-stopped.
    pub force_stopped: bool,

    /// Reason for force stop, if applicable.
    pub force_stop_reason: Option<String>,
}

/// Summary info for listing resumable sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub checkpoint_time: DateTime<Local>,
    pub original_start_time: DateTime<Local>,
    pub phase: StartupPhase,
    pub iteration: u32,
    pub working_dir: PathBuf,
}

impl SessionCheckpoint {
    /// Get the checkpoint storage directory.
    pub fn sessions_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".openagents")
            .join("sessions")
    }

    /// Get the checkpoint file path for a session.
    pub fn checkpoint_path(session_id: &str) -> PathBuf {
        Self::sessions_dir()
            .join(session_id)
            .join("checkpoint.json")
    }

    /// Save this checkpoint to disk.
    pub fn save(&self) -> Result<PathBuf, std::io::Error> {
        let path = Self::checkpoint_path(&self.session_id);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, json)?;

        Ok(path)
    }

    /// Load a checkpoint from disk.
    pub fn load(session_id: &str) -> Result<Self, std::io::Error> {
        let path = Self::checkpoint_path(session_id);
        let json = fs::read_to_string(&path)?;

        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Delete a checkpoint from disk.
    pub fn delete(session_id: &str) -> Result<(), std::io::Error> {
        let path = Self::checkpoint_path(session_id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// List all resumable sessions.
    pub fn list_sessions() -> Result<Vec<SessionSummary>, std::io::Error> {
        let sessions_dir = Self::sessions_dir();
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let checkpoint_file = path.join("checkpoint.json");
                if checkpoint_file.exists() {
                    if let Ok(json) = fs::read_to_string(&checkpoint_file) {
                        if let Ok(checkpoint) = serde_json::from_str::<SessionCheckpoint>(&json) {
                            summaries.push(SessionSummary {
                                session_id: checkpoint.session_id,
                                checkpoint_time: checkpoint.checkpoint_time,
                                original_start_time: checkpoint.original_start_time,
                                phase: checkpoint.phase,
                                iteration: checkpoint.iteration,
                                working_dir: checkpoint.working_dir,
                            });
                        }
                    }
                }
            }
        }

        // Sort by checkpoint time, most recent first
        summaries.sort_by(|a, b| b.checkpoint_time.cmp(&a.checkpoint_time));

        Ok(summaries)
    }

    /// Check if a checkpoint exists for a session.
    pub fn exists(session_id: &str) -> bool {
        Self::checkpoint_path(session_id).exists()
    }

    /// Check if this checkpoint is stale (older than 24 hours).
    pub fn is_stale(&self) -> bool {
        let age = Local::now().signed_duration_since(self.checkpoint_time);
        age.num_hours() > 24
    }

    /// Check if the working directory still exists.
    pub fn is_valid(&self) -> bool {
        self.working_dir.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_path() {
        let path = SessionCheckpoint::checkpoint_path("test-session-123");
        assert!(path.ends_with("sessions/test-session-123/checkpoint.json"));
    }

    #[test]
    fn test_sessions_dir() {
        let dir = SessionCheckpoint::sessions_dir();
        assert!(dir.ends_with(".openagents/sessions"));
    }
}
