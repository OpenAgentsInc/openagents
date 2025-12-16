//! Tool use entities for agent interactions.
//!
//! Tool uses represent the agent's use of external tools like
//! file operations, terminal commands, web searches, etc.

use crate::ids::ToolUseId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The status of a tool use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolUseStatus {
    /// Tool use has been initiated but not yet executed.
    Pending,
    /// Tool is currently executing.
    Running,
    /// Tool executed successfully.
    Success,
    /// Tool execution failed.
    Failed,
    /// Tool use was cancelled.
    Cancelled,
}

impl ToolUseStatus {
    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ToolUseStatus::Success | ToolUseStatus::Failed | ToolUseStatus::Cancelled
        )
    }
}

/// A tool use by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUse {
    /// Unique identifier for this tool use.
    pub id: ToolUseId,

    /// Name of the tool being used.
    pub tool_name: String,

    /// Input parameters (JSON).
    pub input: serde_json::Value,

    /// Current status of the tool use.
    pub status: ToolUseStatus,

    /// When the tool use was initiated.
    pub started_at: DateTime<Utc>,

    /// When the tool use completed (if terminal).
    pub completed_at: Option<DateTime<Utc>>,
}

impl ToolUse {
    /// Create a new pending tool use.
    pub fn new(tool_name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            id: ToolUseId::new(),
            tool_name: tool_name.into(),
            input,
            status: ToolUseStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Create a tool use with a specific ID.
    pub fn with_id(id: ToolUseId, tool_name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            id,
            tool_name: tool_name.into(),
            input,
            status: ToolUseStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark the tool use as running.
    pub fn start(&mut self) {
        self.status = ToolUseStatus::Running;
    }

    /// Mark the tool use as successful.
    pub fn succeed(&mut self) {
        self.status = ToolUseStatus::Success;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the tool use as failed.
    pub fn fail(&mut self) {
        self.status = ToolUseStatus::Failed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the tool use as cancelled.
    pub fn cancel(&mut self) {
        self.status = ToolUseStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Check if this tool use is complete.
    pub fn is_complete(&self) -> bool {
        self.status.is_terminal()
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The tool use this result is for.
    pub tool_use_id: ToolUseId,

    /// Whether the tool succeeded.
    pub success: bool,

    /// Output from the tool (could be text, JSON, etc.).
    pub output: ToolOutput,

    /// Error message if the tool failed.
    pub error: Option<String>,

    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ToolOutput {
    /// Plain text output.
    Text(String),

    /// JSON output.
    Json(serde_json::Value),

    /// Binary output (base64 encoded).
    Binary { mime_type: String, data: String },

    /// File reference.
    File {
        path: String,
        mime_type: Option<String>,
    },

    /// No output.
    Empty,
}

impl ToolResult {
    /// Create a successful tool result with text output.
    pub fn success_text(
        tool_use_id: ToolUseId,
        output: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_use_id,
            success: true,
            output: ToolOutput::Text(output.into()),
            error: None,
            duration_ms,
        }
    }

    /// Create a successful tool result with JSON output.
    pub fn success_json(
        tool_use_id: ToolUseId,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_use_id,
            success: true,
            output: ToolOutput::Json(output),
            error: None,
            duration_ms,
        }
    }

    /// Create a failed tool result.
    pub fn failure(tool_use_id: ToolUseId, error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            tool_use_id,
            success: false,
            output: ToolOutput::Empty,
            error: Some(error.into()),
            duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_lifecycle() {
        let mut tool = ToolUse::new("read_file", serde_json::json!({"path": "/test.txt"}));
        assert_eq!(tool.status, ToolUseStatus::Pending);
        assert!(!tool.is_complete());

        tool.start();
        assert_eq!(tool.status, ToolUseStatus::Running);
        assert!(!tool.is_complete());

        tool.succeed();
        assert_eq!(tool.status, ToolUseStatus::Success);
        assert!(tool.is_complete());
        assert!(tool.completed_at.is_some());
    }

    #[test]
    fn test_tool_result() {
        let tool_use_id = ToolUseId::new();
        let result = ToolResult::success_text(tool_use_id, "File contents here", 150);

        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.duration_ms, 150);

        match result.output {
            ToolOutput::Text(s) => assert_eq!(s, "File contents here"),
            _ => panic!("Expected text output"),
        }
    }
}
