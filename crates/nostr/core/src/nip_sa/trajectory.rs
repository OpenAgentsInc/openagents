//! Trajectory Events (kinds:38030, 38031)
//!
//! Trajectory events provide a transparent record of agent decision-making.
//! They map to the TrajectoryCollector infrastructure in autopilot.
//!
//! ## Trajectory Session (kind:38030)
//!
//! Addressable event that describes a complete trajectory session.
//!
//! Tags:
//! - `["d", "<session-id>"]` - Unique session identifier
//! - `["tick", "<tick-request-id>"]` - Links to tick request
//! - `["started_at", "1703000000"]` - Start timestamp
//! - `["model", "claude-sonnet-4.5"]` - Model used
//! - `["visibility", "public|private"]` - Public or private trajectory
//!
//! Content: Session metadata
//!
//! ```json
//! {
//!   "session_id": "session-123",
//!   "started_at": 1703000000,
//!   "ended_at": 1703001000,
//!   "model": "claude-sonnet-4.5",
//!   "total_events": 42,
//!   "trajectory_hash": "sha256-of-all-events"
//! }
//! ```
//!
//! ## Trajectory Event (kind:38031)
//!
//! Individual step in the trajectory.
//!
//! Tags:
//! - `["session", "<session-id>"]` - Links to session
//! - `["tick", "<tick-request-id>"]` - Links to tick
//! - `["seq", "5"]` - Sequence number in session
//! - `["step", "ToolUse|ToolResult|Message|Thinking"]` - Step type
//!
//! Content: Step data (JSON)
//!
//! For ToolUse:
//! ```json
//! {
//!   "type": "ToolUse",
//!   "tool": "Read",
//!   "input": {"file_path": "/path/to/file"}
//! }
//! ```
//!
//! For ToolResult:
//! ```json
//! {
//!   "type": "ToolResult",
//!   "tool": "Read",
//!   "output": "file contents...",
//!   "success": true
//! }
//! ```
//!
//! For Thinking:
//! ```json
//! {
//!   "type": "Thinking",
//!   "content": "<redacted>",
//!   "hash": "sha256-of-content"
//! }
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for trajectory session event
pub const KIND_TRAJECTORY_SESSION: u16 = 38030;

/// Kind for trajectory event
pub const KIND_TRAJECTORY_EVENT: u16 = 38031;

/// Errors that can occur during NIP-SA trajectory operations
#[derive(Debug, Error)]
pub enum TrajectoryError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("invalid hash: {0}")]
    InvalidHash(String),
}

/// Trajectory visibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryVisibility {
    /// Public trajectory (NIP-28 channel)
    Public,
    /// Private trajectory (NIP-EE group)
    Private,
}

/// Trajectory session metadata (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectorySessionContent {
    /// Unique session identifier
    pub session_id: String,
    /// Start timestamp (Unix seconds)
    pub started_at: u64,
    /// End timestamp (Unix seconds, None if ongoing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    /// Model used
    pub model: String,
    /// Total number of events
    pub total_events: u32,
    /// SHA-256 hash of all events (for verification)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_hash: Option<String>,
}

/// Trajectory step type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    /// Tool invocation
    ToolUse,
    /// Tool result
    ToolResult,
    /// Agent message/response
    Message,
    /// Agent thinking (may be redacted)
    Thinking,
}

/// Trajectory event content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEventContent {
    /// Step type
    #[serde(rename = "type")]
    pub step_type: StepType,
    /// Step data (varies by type)
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

impl TrajectorySessionContent {
    /// Create new trajectory session
    pub fn new(session_id: impl Into<String>, started_at: u64, model: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            started_at,
            ended_at: None,
            model: model.into(),
            total_events: 0,
            trajectory_hash: None,
        }
    }

    /// Set end timestamp
    pub fn with_end_time(mut self, ended_at: u64) -> Self {
        self.ended_at = Some(ended_at);
        self
    }

    /// Set total events
    pub fn with_total_events(mut self, total_events: u32) -> Self {
        self.total_events = total_events;
        self
    }

    /// Set trajectory hash
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.trajectory_hash = Some(hash.into());
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, TrajectoryError> {
        serde_json::to_string(self).map_err(|e| TrajectoryError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, TrajectoryError> {
        serde_json::from_str(json).map_err(|e| TrajectoryError::Deserialization(e.to_string()))
    }
}

impl TrajectoryEventContent {
    /// Create new trajectory event
    pub fn new(step_type: StepType) -> Self {
        Self {
            step_type,
            data: serde_json::Map::new(),
        }
    }

    /// Add data field
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, TrajectoryError> {
        serde_json::to_string(self).map_err(|e| TrajectoryError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, TrajectoryError> {
        serde_json::from_str(json).map_err(|e| TrajectoryError::Deserialization(e.to_string()))
    }
}

/// Trajectory session event wrapper
#[derive(Debug, Clone)]
pub struct TrajectorySession {
    /// Session content
    pub content: TrajectorySessionContent,
    /// Tick request ID
    pub tick_id: String,
    /// Visibility
    pub visibility: TrajectoryVisibility,
}

impl TrajectorySession {
    /// Create new trajectory session
    pub fn new(
        content: TrajectorySessionContent,
        tick_id: impl Into<String>,
        visibility: TrajectoryVisibility,
    ) -> Self {
        Self {
            content,
            tick_id: tick_id.into(),
            visibility,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.content.session_id.clone()],
            vec!["tick".to_string(), self.tick_id.clone()],
            vec![
                "started_at".to_string(),
                self.content.started_at.to_string(),
            ],
            vec!["model".to_string(), self.content.model.clone()],
            vec!["visibility".to_string(), self.visibility_to_string()],
        ];

        if let Some(ended_at) = self.content.ended_at {
            tags.push(vec!["ended_at".to_string(), ended_at.to_string()]);
        }

        tags
    }

    fn visibility_to_string(&self) -> String {
        match self.visibility {
            TrajectoryVisibility::Public => "public".to_string(),
            TrajectoryVisibility::Private => "private".to_string(),
        }
    }
}

/// Trajectory event wrapper
#[derive(Debug, Clone)]
pub struct TrajectoryEvent {
    /// Event content
    pub content: TrajectoryEventContent,
    /// Session ID
    pub session_id: String,
    /// Tick request ID
    pub tick_id: String,
    /// Sequence number
    pub sequence: u32,
}

impl TrajectoryEvent {
    /// Create new trajectory event
    pub fn new(
        content: TrajectoryEventContent,
        session_id: impl Into<String>,
        tick_id: impl Into<String>,
        sequence: u32,
    ) -> Self {
        Self {
            content,
            session_id: session_id.into(),
            tick_id: tick_id.into(),
            sequence,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        vec![
            vec!["session".to_string(), self.session_id.clone()],
            vec!["tick".to_string(), self.tick_id.clone()],
            vec!["seq".to_string(), self.sequence.to_string()],
            vec!["step".to_string(), self.step_to_string()],
        ]
    }

    fn step_to_string(&self) -> String {
        match self.content.step_type {
            StepType::ToolUse => "ToolUse".to_string(),
            StepType::ToolResult => "ToolResult".to_string(),
            StepType::Message => "Message".to_string(),
            StepType::Thinking => "Thinking".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_session_content_creation() {
        let content = TrajectorySessionContent::new("session-123", 1703000000, "claude-sonnet-4.5");
        assert_eq!(content.session_id, "session-123");
        assert_eq!(content.started_at, 1703000000);
        assert_eq!(content.ended_at, None);
        assert_eq!(content.model, "claude-sonnet-4.5");
        assert_eq!(content.total_events, 0);
        assert_eq!(content.trajectory_hash, None);
    }

    #[test]
    fn test_trajectory_session_content_builder() {
        let content = TrajectorySessionContent::new("session-123", 1703000000, "claude-sonnet-4.5")
            .with_end_time(1703001000)
            .with_total_events(42)
            .with_hash("sha256-test");

        assert_eq!(content.ended_at, Some(1703001000));
        assert_eq!(content.total_events, 42);
        assert_eq!(content.trajectory_hash, Some("sha256-test".to_string()));
    }

    #[test]
    fn test_trajectory_session_content_serialization() {
        let content = TrajectorySessionContent::new("session-123", 1703000000, "claude-sonnet-4.5")
            .with_end_time(1703001000)
            .with_total_events(42);

        let json = content.to_json().unwrap();
        let parsed = TrajectorySessionContent::from_json(&json).unwrap();

        assert_eq!(parsed.session_id, "session-123");
        assert_eq!(parsed.started_at, 1703000000);
        assert_eq!(parsed.ended_at, Some(1703001000));
        assert_eq!(parsed.total_events, 42);
    }

    #[test]
    fn test_trajectory_event_content_creation() {
        let content = TrajectoryEventContent::new(StepType::ToolUse);
        assert_eq!(content.step_type, StepType::ToolUse);
        assert_eq!(content.data.len(), 0);
    }

    #[test]
    fn test_trajectory_event_content_with_data() {
        let content = TrajectoryEventContent::new(StepType::ToolUse)
            .with_data("tool", serde_json::Value::String("Read".to_string()))
            .with_data(
                "input",
                serde_json::json!({"file_path": "/path/to/file"}),
            );

        assert_eq!(content.data.len(), 2);
        assert_eq!(
            content.data.get("tool"),
            Some(&serde_json::Value::String("Read".to_string()))
        );
    }

    #[test]
    fn test_trajectory_event_content_serialization() {
        let content = TrajectoryEventContent::new(StepType::ToolResult)
            .with_data("tool", serde_json::Value::String("Read".to_string()))
            .with_data("success", serde_json::Value::Bool(true));

        let json = content.to_json().unwrap();
        let parsed = TrajectoryEventContent::from_json(&json).unwrap();

        assert_eq!(parsed.step_type, StepType::ToolResult);
        assert_eq!(parsed.data.len(), 2);
    }

    #[test]
    fn test_trajectory_session_creation() {
        let content = TrajectorySessionContent::new("session-123", 1703000000, "claude-sonnet-4.5");
        let session = TrajectorySession::new(content, "tick-456", TrajectoryVisibility::Public);

        assert_eq!(session.content.session_id, "session-123");
        assert_eq!(session.tick_id, "tick-456");
        assert_eq!(session.visibility, TrajectoryVisibility::Public);
    }

    #[test]
    fn test_trajectory_session_tags() {
        let content = TrajectorySessionContent::new("session-123", 1703000000, "claude-sonnet-4.5")
            .with_end_time(1703001000);
        let session = TrajectorySession::new(content, "tick-456", TrajectoryVisibility::Private);

        let tags = session.build_tags();

        assert_eq!(tags[0], vec!["d", "session-123"]);
        assert_eq!(tags[1], vec!["tick", "tick-456"]);
        assert_eq!(tags[2], vec!["started_at", "1703000000"]);
        assert_eq!(tags[3], vec!["model", "claude-sonnet-4.5"]);
        assert_eq!(tags[4], vec!["visibility", "private"]);
        assert_eq!(tags[5], vec!["ended_at", "1703001000"]);
    }

    #[test]
    fn test_trajectory_event_creation() {
        let content = TrajectoryEventContent::new(StepType::ToolUse);
        let event = TrajectoryEvent::new(content, "session-123", "tick-456", 5);

        assert_eq!(event.session_id, "session-123");
        assert_eq!(event.tick_id, "tick-456");
        assert_eq!(event.sequence, 5);
    }

    #[test]
    fn test_trajectory_event_tags() {
        let content = TrajectoryEventContent::new(StepType::Thinking);
        let event = TrajectoryEvent::new(content, "session-123", "tick-456", 10);

        let tags = event.build_tags();

        assert_eq!(tags[0], vec!["session", "session-123"]);
        assert_eq!(tags[1], vec!["tick", "tick-456"]);
        assert_eq!(tags[2], vec!["seq", "10"]);
        assert_eq!(tags[3], vec!["step", "Thinking"]);
    }

    #[test]
    fn test_step_type_serialization() {
        let tool_use = StepType::ToolUse;
        let json = serde_json::to_string(&tool_use).unwrap();
        assert_eq!(json, "\"ToolUse\"");

        let tool_result = StepType::ToolResult;
        let json = serde_json::to_string(&tool_result).unwrap();
        assert_eq!(json, "\"ToolResult\"");

        let message = StepType::Message;
        let json = serde_json::to_string(&message).unwrap();
        assert_eq!(json, "\"Message\"");

        let thinking = StepType::Thinking;
        let json = serde_json::to_string(&thinking).unwrap();
        assert_eq!(json, "\"Thinking\"");
    }

    #[test]
    fn test_trajectory_visibility_serialization() {
        let public = TrajectoryVisibility::Public;
        let json = serde_json::to_string(&public).unwrap();
        assert_eq!(json, "\"public\"");

        let private = TrajectoryVisibility::Private;
        let json = serde_json::to_string(&private).unwrap();
        assert_eq!(json, "\"private\"");
    }
}
