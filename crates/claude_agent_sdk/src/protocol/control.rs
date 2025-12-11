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

    /// Get supported slash commands
    #[serde(rename = "supported_commands")]
    SupportedCommands,

    /// Get supported models
    #[serde(rename = "supported_models")]
    SupportedModels,

    /// Get account information
    #[serde(rename = "account_info")]
    AccountInfo,
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

// ============================================================================
// Supporting types for control responses
// ============================================================================

/// Slash command information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    /// Command name (without leading slash).
    pub name: String,
    /// Description of what the command does.
    pub description: String,
    /// Hint for expected argument format (e.g., "<file>").
    #[serde(rename = "argumentHint", default)]
    pub argument_hint: String,
}

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// API model identifier.
    pub value: String,
    /// Human-readable display name.
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// Description of the model's capabilities.
    pub description: String,
}

/// Account information for the authenticated user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Organization name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Subscription type (e.g., "pro", "enterprise").
    #[serde(rename = "subscriptionType", skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    /// Token source (e.g., "api_key", "oauth").
    #[serde(rename = "tokenSource", skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
    /// API key source (e.g., "environment", "config").
    #[serde(rename = "apiKeySource", skip_serializing_if = "Option::is_none")]
    pub api_key_source: Option<String>,
}
