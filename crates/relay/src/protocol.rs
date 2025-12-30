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
    TunnelDisconnected { reason: Option<String> },

    /// Ping/pong for keepalive
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },

    // === Browser → Tunnel ===
    /// Start a new autopilot task
    StartTask {
        task_id: String,
        repo: String,
        task: String,
        /// Use user's Claude API key (true) or pay via credits (false)
        use_own_key: bool,
    },

    /// Cancel a running task
    CancelTask { task_id: String },

    /// Send user input during task execution
    SendInput { task_id: String, text: String },

    // === Tunnel → Browser (Autopilot Streaming) ===
    /// Text chunk from Claude
    AutopilotChunk { task_id: String, chunk: String },

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

    // === Error Handling ===
    /// Generic error message
    Error { code: String, message: String },
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
}
