use serde_json::Value;

use crate::app::catalog::mcp::McpServerStatus;
use crate::app::session::RateLimits;
use crate::autopilot_loop::DspyStage;

use super::super::HookLogEntry;
use super::super::chat::MessageMetadata;

/// Events from the async query task.
#[allow(dead_code)]
pub(crate) enum ResponseEvent {
    Chunk(String),
    ThoughtChunk(String),
    ToolCallStart {
        name: String,
        tool_use_id: String,
    },
    ToolCallInput {
        json: String,
    },
    ToolCallEnd,
    ToolResult {
        content: String,
        is_error: bool,
        tool_use_id: Option<String>,
        exit_code: Option<i32>,
        output_value: Option<Value>,
    },
    ToolProgress {
        tool_use_id: String,
        tool_name: String,
        elapsed_secs: f64,
    },
    UserMessageId {
        uuid: String,
    },
    SystemMessage(String),
    Complete {
        metadata: Option<MessageMetadata>,
    },
    Error(String),
    SystemInit {
        model: String,
        permission_mode: String,
        session_id: String,
        codex_thread_id: Option<String>,
        tool_count: usize,
        tools: Vec<String>,
        output_style: String,
        slash_commands: Vec<String>,
        mcp_servers: Vec<McpServerStatus>,
    },
    McpStatus {
        servers: Vec<McpServerStatus>,
        error: Option<String>,
    },
    RateLimitsUpdated {
        limits: RateLimits,
    },
    HookLog(HookLogEntry),
    DspyStage(DspyStage),
}

#[allow(dead_code)]
pub(crate) enum QueryControl {
    Interrupt,
    RewindFiles {
        user_message_id: String,
    },
    #[allow(dead_code)]
    Abort,
    FetchMcpStatus,
}
