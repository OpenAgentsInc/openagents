//! Chat update types for streaming to the UI.
//!
//! These types represent all possible updates that can occur during a chat session,
//! providing a unified stream for the UI to consume.

use coder_domain::PermissionId;
use coder_domain::ids::{MessageId, SessionId, ThreadId};
use coder_permission::PermissionRequest;
use serde::{Deserialize, Serialize};

/// A single update from the chat service to the UI.
///
/// The UI consumes a stream of these updates to render the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatUpdate {
    // =========================================================================
    // Session Lifecycle
    // =========================================================================
    /// A new session has started.
    SessionStarted {
        session_id: SessionId,
        thread_id: ThreadId,
    },

    /// Session status changed.
    SessionStatusChanged {
        session_id: SessionId,
        status: SessionStatus,
    },

    /// Session has ended.
    SessionEnded {
        session_id: SessionId,
        /// Whether the session ended successfully.
        success: bool,
        /// Error message if not successful.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // =========================================================================
    // Message Streaming
    // =========================================================================
    /// A new message has started (assistant response beginning).
    MessageStarted {
        session_id: SessionId,
        message_id: MessageId,
        role: MessageRole,
    },

    /// Text content delta (streaming text).
    TextDelta {
        session_id: SessionId,
        message_id: MessageId,
        /// The text chunk to append.
        delta: String,
    },

    /// Reasoning/thinking delta (for models with extended thinking).
    ReasoningDelta {
        session_id: SessionId,
        message_id: MessageId,
        /// The reasoning text chunk to append.
        delta: String,
    },

    /// Message has completed.
    MessageCompleted {
        session_id: SessionId,
        message_id: MessageId,
        /// Reason for completion (stop, tool_use, length, etc.).
        finish_reason: String,
    },

    // =========================================================================
    // Tool Use
    // =========================================================================
    /// A tool use has started.
    ToolStarted {
        session_id: SessionId,
        message_id: MessageId,
        /// Unique ID for this tool call.
        tool_call_id: String,
        /// Name of the tool being called.
        tool_name: String,
    },

    /// Tool input is being streamed (partial JSON).
    ToolInputDelta {
        session_id: SessionId,
        tool_call_id: String,
        /// Partial JSON input delta.
        delta: String,
    },

    /// Tool input is complete, execution is starting.
    ToolExecuting {
        session_id: SessionId,
        tool_call_id: String,
        /// Complete parsed input (for display).
        input: serde_json::Value,
    },

    /// Tool execution progress update.
    ToolProgress {
        session_id: SessionId,
        tool_call_id: String,
        /// Progress message (e.g., "Reading file...", "50% complete").
        message: String,
    },

    /// Tool execution has completed.
    ToolCompleted {
        session_id: SessionId,
        tool_call_id: String,
        /// Tool output (may be truncated for display).
        output: String,
        /// Whether this was an error result.
        is_error: bool,
        /// Execution duration in milliseconds.
        duration_ms: u64,
    },

    // =========================================================================
    // Permission
    // =========================================================================
    /// Permission is required before proceeding.
    ///
    /// The UI should display a dialog and call `respond_permission()`.
    PermissionRequired {
        session_id: SessionId,
        permission_id: PermissionId,
        request: PermissionRequest,
    },

    /// Permission has been resolved (granted or denied).
    PermissionResolved {
        session_id: SessionId,
        permission_id: PermissionId,
        /// Whether permission was granted.
        granted: bool,
    },

    // =========================================================================
    // Errors
    // =========================================================================
    /// An error occurred.
    Error {
        session_id: SessionId,
        /// Error message.
        message: String,
        /// Error code (for programmatic handling).
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// Whether the session can continue after this error.
        recoverable: bool,
    },

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Token usage update.
    UsageUpdate {
        session_id: SessionId,
        /// Total tokens used in this session.
        total_tokens: u64,
        /// Estimated cost in USD.
        cost_usd: f64,
    },

    /// Agent information (sent at session start).
    AgentInfo {
        session_id: SessionId,
        /// Agent ID being used.
        agent_id: String,
        /// Model being used.
        model_id: String,
        /// Provider being used.
        provider_id: String,
    },
}

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is idle, waiting for input.
    Idle,
    /// Session is processing a request.
    Processing,
    /// Session is waiting for permission.
    WaitingForPermission,
    /// Session is executing a tool.
    ExecutingTool,
    /// Session is in error state.
    Error,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ChatUpdate {
    /// Get the session ID for this update.
    pub fn session_id(&self) -> SessionId {
        match self {
            ChatUpdate::SessionStarted { session_id, .. }
            | ChatUpdate::SessionStatusChanged { session_id, .. }
            | ChatUpdate::SessionEnded { session_id, .. }
            | ChatUpdate::MessageStarted { session_id, .. }
            | ChatUpdate::TextDelta { session_id, .. }
            | ChatUpdate::ReasoningDelta { session_id, .. }
            | ChatUpdate::MessageCompleted { session_id, .. }
            | ChatUpdate::ToolStarted { session_id, .. }
            | ChatUpdate::ToolInputDelta { session_id, .. }
            | ChatUpdate::ToolExecuting { session_id, .. }
            | ChatUpdate::ToolProgress { session_id, .. }
            | ChatUpdate::ToolCompleted { session_id, .. }
            | ChatUpdate::PermissionRequired { session_id, .. }
            | ChatUpdate::PermissionResolved { session_id, .. }
            | ChatUpdate::Error { session_id, .. }
            | ChatUpdate::UsageUpdate { session_id, .. }
            | ChatUpdate::AgentInfo { session_id, .. } => *session_id,
        }
    }

    /// Check if this is an error update.
    pub fn is_error(&self) -> bool {
        matches!(self, ChatUpdate::Error { .. })
    }

    /// Check if this update indicates the session has ended.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ChatUpdate::SessionEnded { .. }
                | ChatUpdate::Error {
                    recoverable: false,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_domain::ids::SessionId;

    #[test]
    fn test_chat_update_session_id() {
        let session_id = SessionId::new();
        let thread_id = ThreadId::new();

        let update = ChatUpdate::SessionStarted {
            session_id,
            thread_id,
        };

        assert_eq!(update.session_id(), session_id);
    }

    #[test]
    fn test_chat_update_is_terminal() {
        let session_id = SessionId::new();

        let ended = ChatUpdate::SessionEnded {
            session_id,
            success: true,
            error: None,
        };
        assert!(ended.is_terminal());

        let fatal_error = ChatUpdate::Error {
            session_id,
            message: "Fatal".into(),
            code: None,
            recoverable: false,
        };
        assert!(fatal_error.is_terminal());

        let recoverable_error = ChatUpdate::Error {
            session_id,
            message: "Recoverable".into(),
            code: None,
            recoverable: true,
        };
        assert!(!recoverable_error.is_terminal());
    }
}
