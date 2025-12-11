//! SDK message types from Claude Code CLI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// All SDK message types from CLI stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SdkMessage {
    /// Assistant response message
    #[serde(rename = "assistant")]
    Assistant(SdkAssistantMessage),

    /// User message (echo or replay)
    #[serde(rename = "user")]
    User(SdkUserMessage),

    /// Query result (success or error)
    #[serde(rename = "result")]
    Result(SdkResultMessage),

    /// System messages (init, status, etc.)
    #[serde(rename = "system")]
    System(SdkSystemMessage),

    /// Streaming partial message
    #[serde(rename = "stream_event")]
    StreamEvent(SdkStreamEvent),

    /// Tool progress update
    #[serde(rename = "tool_progress")]
    ToolProgress(SdkToolProgressMessage),

    /// Authentication status
    #[serde(rename = "auth_status")]
    AuthStatus(SdkAuthStatusMessage),
}

/// Assistant message from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkAssistantMessage {
    /// The API message content
    pub message: Value,
    /// Parent tool use ID if this is part of a tool call
    pub parent_tool_use_id: Option<String>,
    /// Error type if there was an error
    pub error: Option<AssistantMessageError>,
    /// Unique message ID
    pub uuid: String,
    /// Session ID
    pub session_id: String,
}

/// Assistant message error types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantMessageError {
    AuthenticationFailed,
    BillingError,
    RateLimit,
    InvalidRequest,
    ServerError,
    Unknown,
}

/// User message from Claude Code CLI (echoed/replayed user messages).
/// Note: When used as part of SdkMessage enum with tag="type", the type field
/// is consumed for enum dispatch, so we don't include it here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkUserMessage {
    /// The message content (APIUserMessage format)
    pub message: Value,
    /// Parent tool use ID if responding to a tool call
    pub parent_tool_use_id: Option<String>,
    /// Whether this is a synthetic (system-generated) message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_synthetic: Option<bool>,
    /// Tool use result if responding to a tool call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<Value>,
    /// Unique message ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Session ID
    pub session_id: String,
    /// True if this is a replay/acknowledgment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,
}

/// Outgoing user message to send to Claude Code CLI.
/// This struct includes the `type` field needed for sending via StdinMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkUserMessageOutgoing {
    /// Message type marker (always "user")
    #[serde(rename = "type")]
    pub msg_type: UserMessageType,
    /// The message content (APIUserMessage format)
    pub message: Value,
    /// Parent tool use ID if responding to a tool call
    pub parent_tool_use_id: Option<String>,
    /// Whether this is a synthetic (system-generated) message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_synthetic: Option<bool>,
    /// Tool use result if responding to a tool call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<Value>,
    /// Unique message ID (optional for new messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Session ID
    pub session_id: String,
    /// True if this is a replay/acknowledgment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserMessageType {
    User,
}

/// Query result message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum SdkResultMessage {
    /// Successful completion
    #[serde(rename = "success")]
    Success(ResultSuccess),

    /// Error during execution
    #[serde(rename = "error_during_execution")]
    ErrorDuringExecution(ResultError),

    /// Max turns exceeded
    #[serde(rename = "error_max_turns")]
    ErrorMaxTurns(ResultError),

    /// Max budget exceeded
    #[serde(rename = "error_max_budget_usd")]
    ErrorMaxBudget(ResultError),

    /// Max structured output retries exceeded
    #[serde(rename = "error_max_structured_output_retries")]
    ErrorMaxStructuredOutputRetries(ResultError),
}

/// Successful result data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSuccess {
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub is_error: bool,
    pub num_turns: u32,
    pub result: String,
    pub total_cost_usd: f64,
    pub usage: Usage,
    #[serde(rename = "modelUsage")]
    pub model_usage: HashMap<String, ModelUsage>,
    pub permission_denials: Vec<PermissionDenial>,
    pub structured_output: Option<Value>,
    pub uuid: String,
    pub session_id: String,
}

/// Error result data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultError {
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub is_error: bool,
    pub num_turns: u32,
    pub total_cost_usd: f64,
    pub usage: Usage,
    #[serde(rename = "modelUsage")]
    pub model_usage: HashMap<String, ModelUsage>,
    pub permission_denials: Vec<PermissionDenial>,
    pub errors: Vec<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

/// Per-model usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cacheReadInputTokens")]
    pub cache_read_input_tokens: u64,
    #[serde(rename = "cacheCreationInputTokens")]
    pub cache_creation_input_tokens: u64,
    #[serde(rename = "webSearchRequests")]
    pub web_search_requests: u64,
    #[serde(rename = "costUSD")]
    pub cost_usd: f64,
    #[serde(rename = "contextWindow")]
    pub context_window: u64,
}

/// Permission denial record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDenial {
    pub tool_name: String,
    pub tool_use_id: String,
    pub tool_input: Value,
}

/// System message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum SdkSystemMessage {
    /// Session initialization
    #[serde(rename = "init")]
    Init(SystemInit),

    /// Compact boundary marker
    #[serde(rename = "compact_boundary")]
    CompactBoundary(CompactBoundary),

    /// Status update
    #[serde(rename = "status")]
    Status(StatusUpdate),

    /// Hook response
    #[serde(rename = "hook_response")]
    HookResponse(HookResponse),
}

/// Session initialization data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInit {
    pub agents: Option<Vec<String>>,
    #[serde(rename = "apiKeySource")]
    pub api_key_source: String,
    pub betas: Option<Vec<String>>,
    pub claude_code_version: String,
    pub cwd: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<McpServerStatus>,
    pub model: String,
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
    pub slash_commands: Vec<String>,
    pub output_style: String,
    pub skills: Vec<String>,
    pub plugins: Vec<PluginInfo>,
    pub uuid: String,
    pub session_id: String,
}

/// MCP server status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub name: String,
    pub status: String,
}

/// Plugin info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub path: String,
}

/// Compact boundary marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundary {
    pub compact_metadata: CompactMetadata,
    pub uuid: String,
    pub session_id: String,
}

/// Compact metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactMetadata {
    pub trigger: String,
    pub pre_tokens: u64,
}

/// Status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub status: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Hook response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub hook_name: String,
    pub hook_event: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub uuid: String,
    pub session_id: String,
}

/// Streaming partial message event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkStreamEvent {
    pub event: Value,
    pub parent_tool_use_id: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Tool progress update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkToolProgressMessage {
    pub tool_use_id: String,
    pub tool_name: String,
    pub parent_tool_use_id: Option<String>,
    pub elapsed_time_seconds: f64,
    pub uuid: String,
    pub session_id: String,
}

/// Authentication status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkAuthStatusMessage {
    #[serde(rename = "isAuthenticating")]
    pub is_authenticating: bool,
    pub output: Vec<String>,
    pub error: Option<String>,
    pub uuid: String,
    pub session_id: String,
}
