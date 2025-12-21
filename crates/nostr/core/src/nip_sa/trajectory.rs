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

// TODO: Map autopilot::trajectory::StepType to NIP-SA StepType
// TODO: Implement Event builders for kinds 38030, 38031
// TODO: Implement trajectory hash calculation and verification
// TODO: Add support for public (NIP-28) and private (NIP-EE) channels
// TODO: Add unit tests
