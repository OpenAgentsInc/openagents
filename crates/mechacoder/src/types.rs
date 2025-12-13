use serde::{Deserialize, Serialize};

/// Client -> Server messages over WebSocket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    SendMessage { content: String, cwd: String },
    Cancel,
}

/// Server -> Client messages over WebSocket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    SessionInit {
        session_id: String,
    },
    TextDelta {
        text: String,
    },
    ToolStart {
        tool_use_id: String,
        tool_name: String,
    },
    ToolInput {
        tool_use_id: String,
        partial_json: String,
    },
    ToolProgress {
        tool_use_id: String,
        elapsed_seconds: f64,
    },
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    Done {
        error: Option<String>,
    },
}

/// A chat message in the thread
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: usize,
    pub role: String,
    pub content: String,
}

/// A tool use entry in the thread
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolUse {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: String,
    pub output: Option<String>,
    pub status: ToolStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolStatus {
    Running,
    Completed,
    Error,
}

/// Thread entry - either a message or tool use
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThreadEntry {
    Message(Message),
    ToolUse(ToolUse),
}
