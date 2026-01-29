//! Relay protocol message types
//!
//! Defines the WebSocket message format for communication between:
//! - Browser (web client)
//! - Cloudflare Worker (relay)
//! - Tunnel client (user's machine)

use serde::{Deserialize, Serialize};

/// Top-level relay message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    // === Connection Management ===
    /// Tunnel client has connected to relay
    TunnelConnected {
        version: String,
        capabilities: Vec<String>,
    },

    /// Tunnel client has disconnected
    TunnelDisconnected {
        reason: Option<String>,
    },

    /// Ping/pong for keepalive
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },

    // === Browser → Tunnel ===
    /// Start a new autopilot task
    StartTask {
        task_id: String,
        repo: String,
        task: String,
        /// Use user's Codex API key (true) or pay via credits (false)
        use_own_key: bool,
    },

    /// Cancel a running task
    CancelTask {
        task_id: String,
    },

    /// Send user input during task execution
    SendInput {
        task_id: String,
        text: String,
    },

    // === Tunnel → Browser (Autopilot Streaming) ===
    /// Text chunk from Codex
    AutopilotChunk {
        task_id: String,
        chunk: String,
    },

    /// Tool use started
    ToolStart {
        task_id: String,
        tool_name: String,
        tool_id: String,
        params: serde_json::Value,
    },

    /// Tool execution completed
    ToolDone {
        task_id: String,
        tool_id: String,
        output: String,
        is_error: bool,
    },

    /// Tool execution progress update
    ToolProgress {
        task_id: String,
        tool_id: String,
        elapsed_secs: f32,
        message: Option<String>,
    },

    /// Usage statistics update
    Usage {
        task_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
        cost_usd: f64,
    },

    /// Task completed successfully
    TaskDone {
        task_id: String,
        summary: String,
    },

    /// Task failed with error
    TaskError {
        task_id: String,
        error: String,
        recoverable: bool,
    },

    // === Browser ↔ Tunnel (Codex Sessions) ===
    /// Create a new Codex session over the tunnel.
    CodexCreateSession {
        session_id: String,
        request: CodexRequest,
    },
    /// Codex session created and ready.
    CodexSessionCreated {
        session_id: String,
    },
    /// Send a prompt to an existing Codex session.
    CodexPrompt {
        session_id: String,
        content: String,
    },
    /// Streaming chunk from Codex.
    CodexChunk {
        chunk: CodexChunk,
    },
    /// Codex requests tool approval.
    CodexToolApproval {
        session_id: String,
        tool: String,
        params: serde_json::Value,
    },
    /// Tool approval decision from browser.
    CodexToolApprovalResponse {
        session_id: String,
        approved: bool,
    },
    /// Stop a Codex session.
    CodexStop {
        session_id: String,
    },
    /// Pause a Codex session (best-effort).
    CodexPause {
        session_id: String,
    },
    /// Resume a Codex session (best-effort).
    CodexResume {
        session_id: String,
    },
    /// Codex session error.
    CodexError {
        session_id: String,
        error: String,
    },

    // === Error Handling ===
    /// Generic error message
    Error {
        code: String,
        message: String,
    },
}

impl RelayMessage {
    /// Parse a JSON string into a RelayMessage
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Session registration request (HTTP, not WebSocket)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterSessionRequest {
    /// Repository to work on (owner/repo)
    pub repo: String,
}

/// Session registration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterSessionResponse {
    /// Unique session ID
    pub session_id: String,
    /// Token for tunnel client to authenticate
    pub tunnel_token: String,
    /// WebSocket URL for tunnel client to connect to
    pub tunnel_url: String,
}

/// Session status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatus {
    pub session_id: String,
    pub tunnel_connected: bool,
    pub repo: Option<String>,
    pub active_task: Option<String>,
}

// === Codex Tunnel Types ===

/// Codex session autonomy level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodexSessionAutonomy {
    Full,
    #[default]
    Supervised,
    Restricted,
    ReadOnly,
}

/// Tool definition for Codex sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub config: Option<serde_json::Value>,
}

/// Request to create a Codex session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRequest {
    pub model: String,
    pub system_prompt: Option<String>,
    pub initial_prompt: Option<String>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    pub max_cost_usd: Option<u64>,
    #[serde(default)]
    pub autonomy: Option<CodexSessionAutonomy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_required_tools: Option<Vec<String>>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
}

/// Tool chunk information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChunk {
    pub name: String,
    pub params: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Chunk type for streaming responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Text,
    ToolStart,
    ToolOutput,
    ToolDone,
    Done,
    Error,
}

/// Streaming chunk from Codex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexChunk {
    pub session_id: String,
    pub chunk_type: ChunkType,
    pub delta: Option<String>,
    pub tool: Option<ToolChunk>,
    pub usage: Option<CodexUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = RelayMessage::TunnelConnected {
            version: "1.0.0".to_string(),
            capabilities: vec!["autopilot".to_string()],
        };

        let json = msg.to_json();
        assert!(json.contains("tunnel_connected"));
        assert!(json.contains("1.0.0"));

        let parsed = RelayMessage::from_json(&json).unwrap();
        match parsed {
            RelayMessage::TunnelConnected { version, .. } => {
                assert_eq!(version, "1.0.0");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_start_task() {
        let msg = RelayMessage::StartTask {
            task_id: "task-123".to_string(),
            repo: "owner/repo".to_string(),
            task: "Fix the bug in login".to_string(),
            use_own_key: true,
        };

        let json = msg.to_json();
        let parsed = RelayMessage::from_json(&json).unwrap();

        match parsed {
            RelayMessage::StartTask {
                task_id,
                repo,
                task,
                use_own_key,
            } => {
                assert_eq!(task_id, "task-123");
                assert_eq!(repo, "owner/repo");
                assert_eq!(task, "Fix the bug in login");
                assert!(use_own_key);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_codex_chunk_roundtrip() {
        let chunk = CodexChunk {
            session_id: "session-1".to_string(),
            chunk_type: ChunkType::Text,
            delta: Some("hello".to_string()),
            tool: None,
            usage: None,
        };
        let msg = RelayMessage::CodexChunk { chunk };

        let json = msg.to_json();
        let parsed = RelayMessage::from_json(&json).unwrap();

        match parsed {
            RelayMessage::CodexChunk { chunk } => {
                assert_eq!(chunk.session_id, "session-1");
                assert!(matches!(chunk.chunk_type, ChunkType::Text));
                assert_eq!(chunk.delta.as_deref(), Some("hello"));
            }
            _ => panic!("Wrong message type"),
        }
    }
}
