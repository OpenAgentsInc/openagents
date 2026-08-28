//! Control request/response types for bidirectional communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Control request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlRequest {
    #[serde(rename = "type")]
    pub msg_type: ControlRequestType,
    pub request_id: String,
    pub request: ControlRequestData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRequestType {
    ControlRequest,
}

/// Control request data variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlRequestData {
    /// Initialize the SDK session
    #[serde(rename = "initialize")]
    Initialize(InitializeRequest),

    /// Interrupt the current query
    #[serde(rename = "interrupt")]
    Interrupt,

    /// Permission check for tool use
    #[serde(rename = "can_use_tool")]
    CanUseTool(CanUseToolRequest),

    /// Set permission mode
    #[serde(rename = "set_permission_mode")]
    SetPermissionMode(SetPermissionModeRequest),

    /// Set model
    #[serde(rename = "set_model")]
    SetModel(SetModelRequest),

    /// Set max thinking tokens
    #[serde(rename = "set_max_thinking_tokens")]
    SetMaxThinkingTokens(SetMaxThinkingTokensRequest),

    /// Get MCP server status
    #[serde(rename = "mcp_status")]
    McpStatus,

    /// Hook callback
    #[serde(rename = "hook_callback")]
    HookCallback(HookCallbackRequest),

    /// MCP message
    #[serde(rename = "mcp_message")]
    McpMessage(McpMessageRequest),

    /// Rewind files to a specific message
    #[serde(rename = "rewind_files")]
    RewindFiles(RewindFilesRequest),

    /// Merge settings into the flag settings layer.
    #[serde(rename = "apply_flag_settings")]
    ApplyFlagSettings(ApplyFlagSettingsRequest),

    /// Replace dynamically managed MCP servers.
    #[serde(rename = "mcp_set_servers")]
    McpSetServers(McpSetServersRequest),

    /// Stop a running task.
    #[serde(rename = "stop_task")]
    StopTask(StopTaskRequest),

    /// Context-window usage by category.
    #[serde(rename = "get_context_usage")]
    GetContextUsage,

    /// Background in-flight foreground tasks (optional one tool_use).
    #[serde(rename = "background_tasks")]
    BackgroundTasks(BackgroundTasksRequest),

    /// Drop a pending async user message by uuid.
    #[serde(rename = "cancel_async_message")]
    CancelAsyncMessage(CancelAsyncMessageRequest),

    /// Session cost totals.
    #[serde(rename = "get_session_cost")]
    GetSessionCost,

    /// Structured `/usage` payload.
    #[serde(rename = "get_usage")]
    GetUsage,

    /// Remote CLI binary version.
    #[serde(rename = "get_binary_version")]
    GetBinaryVersion,

    /// At-mention file autocomplete.
    #[serde(rename = "file_suggestions")]
    FileSuggestions(FileSuggestionsRequest),

    /// Reload plugins, commands, and MCP status.
    #[serde(rename = "reload_plugins")]
    ReloadPlugins,

    /// Reload skills.
    #[serde(rename = "reload_skills")]
    ReloadSkills,

    /// Reconnect one MCP server.
    #[serde(rename = "mcp_reconnect")]
    McpReconnect(McpReconnectRequest),

    /// Enable or disable one MCP server.
    #[serde(rename = "mcp_toggle")]
    McpToggle(McpToggleRequest),

    /// Set the session title.
    #[serde(rename = "rename_session")]
    RenameSession(RenameSessionRequest),
}

/// Initialize request data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitializeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_mcp_servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Value>,
}

/// Permission check request from CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanUseToolRequest {
    pub tool_name: String,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_suggestions: Option<Vec<PermissionUpdate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    pub tool_use_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// Permission update action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PermissionUpdate {
    #[serde(rename = "addRules")]
    AddRules {
        rules: Vec<PermissionRule>,
        behavior: PermissionBehavior,
        destination: String,
    },
    #[serde(rename = "replaceRules")]
    ReplaceRules {
        rules: Vec<PermissionRule>,
        behavior: PermissionBehavior,
        destination: String,
    },
    #[serde(rename = "removeRules")]
    RemoveRules {
        rules: Vec<PermissionRule>,
        behavior: PermissionBehavior,
        destination: String,
    },
    #[serde(rename = "setMode")]
    SetMode { mode: String, destination: String },
    #[serde(rename = "addDirectories")]
    AddDirectories {
        directories: Vec<String>,
        destination: String,
    },
    #[serde(rename = "removeDirectories")]
    RemoveDirectories {
        directories: Vec<String>,
        destination: String,
    },
}

/// Permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    #[serde(rename = "toolName")]
    pub tool_name: String,
    #[serde(rename = "ruleContent", skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

/// Permission behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// Set permission mode request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPermissionModeRequest {
    pub mode: PermissionMode,
}

/// Permission mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    DontAsk,
}

/// Set model request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModelRequest {
    pub model: Option<String>,
}

/// Set max thinking tokens request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMaxThinkingTokensRequest {
    pub max_thinking_tokens: Option<u32>,
}

/// Hook callback request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCallbackRequest {
    pub callback_id: String,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// MCP message request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessageRequest {
    pub server_name: String,
    pub message: Value,
}

/// Rewind files request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewindFilesRequest {
    pub user_message_id: String,
}

/// Merge settings into the flag settings layer (TS `applyFlagSettings`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyFlagSettingsRequest {
    pub settings: Value,
}

/// Replace dynamically managed MCP servers (TS `setMcpServers`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSetServersRequest {
    pub servers: Value,
}

/// Stop a running task (TS `stopTask`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTaskRequest {
    pub task_id: String,
}

/// Background in-flight foreground tasks (TS `backgroundTasks`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackgroundTasksRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// Drop a queued async user message (TS `cancelAsyncMessage`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAsyncMessageRequest {
    pub message_uuid: String,
}

/// At-mention file autocomplete (TS `fileSuggestions` / control subtype).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSuggestionsRequest {
    pub query: String,
}

/// Reconnect one MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReconnectRequest {
    #[serde(rename = "serverName")]
    pub server_name: String,
}

/// Enable or disable one MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToggleRequest {
    #[serde(rename = "serverName")]
    pub server_name: String,
    pub enabled: bool,
}

/// Set the session title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSessionRequest {
    pub title: String,
}

/// Control response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlResponse {
    #[serde(rename = "type")]
    pub msg_type: ControlResponseType,
    pub response: ControlResponseData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlResponseType {
    ControlResponse,
}

/// Control response data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlResponseData {
    /// Success response
    #[serde(rename = "success")]
    Success {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<Value>,
    },

    /// Error response
    #[serde(rename = "error")]
    Error {
        request_id: String,
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_permission_requests: Option<Vec<SdkControlRequest>>,
    },
}

/// Permission result to send back for can_use_tool request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum PermissionResult {
    /// Allow the tool use
    #[serde(rename = "allow")]
    Allow {
        #[serde(rename = "updatedInput")]
        updated_input: Value,
        #[serde(rename = "updatedPermissions", skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
        #[serde(rename = "toolUseID", skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },

    /// Deny the tool use
    #[serde(rename = "deny")]
    Deny {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
        #[serde(rename = "toolUseID", skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
}

impl PermissionResult {
    /// Create an allow result with the original input.
    pub fn allow(input: Value) -> Self {
        Self::Allow {
            updated_input: input,
            updated_permissions: None,
            tool_use_id: None,
        }
    }

    /// Create a deny result with a message.
    pub fn deny(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
            interrupt: None,
            tool_use_id: None,
        }
    }

    /// Create a deny result that interrupts execution.
    pub fn deny_and_interrupt(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
            interrupt: Some(true),
            tool_use_id: None,
        }
    }
}
