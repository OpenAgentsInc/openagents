//! HUD WebSocket Protocol Types
//!
//! Rust equivalents of the TypeScript protocol defined in `src/hud/protocol.ts`.
//! These types are used for injecting messages into GPUI components during tests.

use serde::{Deserialize, Serialize};

// ============================================================================
// Core Data Types
// ============================================================================

/// Task status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    Pending,
    InProgress,
    Done,
    Failed,
    Blocked,
}

/// Orchestrator phase enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestratorPhase {
    Init,
    Selecting,
    Decomposing,
    Executing,
    Verifying,
    Committing,
    Pushing,
    Complete,
    Failed,
}

/// Agent type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    ClaudeCode,
    Minimal,
    Orchestrator,
}

/// Task information for HUD display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudTaskInfo {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: i32,
}

/// Subtask information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudSubtaskInfo {
    pub id: String,
    pub description: String,
    pub status: SubtaskStatus,
}

/// Subagent execution result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudSubagentResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentType>,
    pub files_modified: Vec<String>,
    pub turns: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for HudSubagentResult {
    fn default() -> Self {
        Self {
            success: true,
            agent: None,
            files_modified: Vec::new(),
            turns: 1,
            error: None,
        }
    }
}

// ============================================================================
// HUD Message Types
// ============================================================================

/// All HUD message types - tagged enum for JSON serialization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HudMessage {
    // Session lifecycle
    SessionStart {
        #[serde(rename = "sessionId")]
        session_id: String,
        timestamp: String,
    },
    SessionComplete {
        success: bool,
        summary: String,
    },

    // Task flow
    TaskSelected {
        task: HudTaskInfo,
    },
    TaskDecomposed {
        subtasks: Vec<HudSubtaskInfo>,
    },
    SubtaskStart {
        subtask: HudSubtaskInfo,
    },
    SubtaskComplete {
        subtask: HudSubtaskInfo,
        result: HudSubagentResult,
    },
    SubtaskFailed {
        subtask: HudSubtaskInfo,
        error: String,
    },

    // Verification
    VerificationStart {
        command: String,
    },
    VerificationComplete {
        command: String,
        passed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },

    // Git operations
    CommitCreated {
        sha: String,
        message: String,
    },
    PushComplete {
        branch: String,
    },

    // Phase and errors
    PhaseChange {
        phase: OrchestratorPhase,
    },
    Error {
        phase: OrchestratorPhase,
        error: String,
    },

    // Streaming output
    TextOutput {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<AgentType>,
    },
    ToolCall {
        #[serde(rename = "toolName")]
        tool_name: String,
        arguments: String,
        #[serde(rename = "callId", skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
    },
    ToolResult {
        #[serde(rename = "toolName")]
        tool_name: String,
        result: String,
        #[serde(rename = "isError")]
        is_error: bool,
        #[serde(rename = "callId", skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
    },

    // APM (Actions Per Minute)
    ApmUpdate {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "sessionAPM")]
        session_apm: f64,
        #[serde(rename = "recentAPM")]
        recent_apm: f64,
        #[serde(rename = "totalActions")]
        total_actions: u64,
        #[serde(rename = "durationMinutes")]
        duration_minutes: f64,
    },

    // Usage metrics
    UsageUpdate {
        usage: UsageInfo,
    },

    // Development
    DevReload {
        #[serde(rename = "changedFile", skip_serializing_if = "Option::is_none")]
        changed_file: Option<String>,
    },
}

/// Usage information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageInfo {
    pub session_id: String,
    pub project_id: String,
    pub timestamp: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    pub subtasks: u32,
    pub duration_ms: u64,
    pub agent: String,
}

// ============================================================================
// Helpers
// ============================================================================

impl HudMessage {
    /// Get the message type as a string
    pub fn type_name(&self) -> &'static str {
        match self {
            HudMessage::SessionStart { .. } => "session_start",
            HudMessage::SessionComplete { .. } => "session_complete",
            HudMessage::TaskSelected { .. } => "task_selected",
            HudMessage::TaskDecomposed { .. } => "task_decomposed",
            HudMessage::SubtaskStart { .. } => "subtask_start",
            HudMessage::SubtaskComplete { .. } => "subtask_complete",
            HudMessage::SubtaskFailed { .. } => "subtask_failed",
            HudMessage::VerificationStart { .. } => "verification_start",
            HudMessage::VerificationComplete { .. } => "verification_complete",
            HudMessage::CommitCreated { .. } => "commit_created",
            HudMessage::PushComplete { .. } => "push_complete",
            HudMessage::PhaseChange { .. } => "phase_change",
            HudMessage::Error { .. } => "error",
            HudMessage::TextOutput { .. } => "text_output",
            HudMessage::ToolCall { .. } => "tool_call",
            HudMessage::ToolResult { .. } => "tool_result",
            HudMessage::ApmUpdate { .. } => "apm_update",
            HudMessage::UsageUpdate { .. } => "usage_update",
            HudMessage::DevReload { .. } => "dev_reload",
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("HudMessage should serialize")
    }

    /// Parse from JSON string
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_start_serialization() {
        let msg = HudMessage::SessionStart {
            session_id: "test-123".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let json = msg.to_json();
        assert!(json.contains(r#""type":"session_start""#));
        assert!(json.contains(r#""sessionId":"test-123""#));

        let parsed = HudMessage::from_json(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_apm_update_serialization() {
        let msg = HudMessage::ApmUpdate {
            session_id: "test".to_string(),
            session_apm: 15.5,
            recent_apm: 18.2,
            total_actions: 42,
            duration_minutes: 3.0,
        };

        let json = msg.to_json();
        assert!(json.contains(r#""type":"apm_update""#));
        assert!(json.contains(r#""sessionAPM":15.5"#));
    }

    #[test]
    fn test_task_selected_serialization() {
        let msg = HudMessage::TaskSelected {
            task: HudTaskInfo {
                id: "oa-123".to_string(),
                title: "Test Task".to_string(),
                status: "in_progress".to_string(),
                priority: 1,
            },
        };

        let json = msg.to_json();
        assert!(json.contains(r#""type":"task_selected""#));
        assert!(json.contains(r#""id":"oa-123""#));
    }

    #[test]
    fn test_error_serialization() {
        let msg = HudMessage::Error {
            phase: OrchestratorPhase::Verifying,
            error: "Test failed".to_string(),
        };

        let json = msg.to_json();
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains(r#""phase":"verifying""#));
    }
}
