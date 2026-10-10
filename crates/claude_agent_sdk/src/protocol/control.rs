//! Control request and response types for the bidirectional control
//! protocol (`@anthropic-ai/claude-agent-sdk` 0.3.296).
//!
//! The SDK sends most subtypes to the CLI. The CLI sends `can_use_tool`,
//! `hook_callback`, `mcp_message`, `elicitation`, `request_user_dialog`,
//! and the auth refresh subtypes to the SDK.

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

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
///
/// A subtype this crate does not model deserializes as
/// [`ControlRequestData::Unsupported`], so an inbound request is still
/// answered (with an error) rather than dropped.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlRequestData {
    /// Start the SDK session.
    #[serde(rename = "initialize")]
    Initialize(InitializeRequest),

    /// Interrupt the current turn.
    #[serde(rename = "interrupt")]
    Interrupt(InterruptRequest),

    /// Permission check for tool use (CLI to SDK).
    #[serde(rename = "can_use_tool")]
    CanUseTool(CanUseToolRequest),

    /// Set the permission mode.
    #[serde(rename = "set_permission_mode")]
    SetPermissionMode(SetPermissionModeRequest),

    /// Set the model.
    #[serde(rename = "set_model")]
    SetModel(SetModelRequest),

    /// Set the thinking-token cap.
    #[serde(rename = "set_max_thinking_tokens")]
    SetMaxThinkingTokens(SetMaxThinkingTokensRequest),

    /// Set the session title.
    #[serde(rename = "rename_session")]
    RenameSession(RenameSessionRequest),

    /// Set the session color.
    #[serde(rename = "set_color")]
    SetColor(SetColorRequest),

    /// MCP server status.
    #[serde(rename = "mcp_status")]
    McpStatus,

    /// Context-window usage by category.
    #[serde(rename = "get_context_usage")]
    GetContextUsage,

    /// Session cost totals.
    #[serde(rename = "get_session_cost")]
    GetSessionCost,

    /// Models the session can switch to.
    #[serde(rename = "list_models")]
    ListModels,

    /// Structured `/usage` payload.
    #[serde(rename = "get_usage")]
    GetUsage,

    /// CLI binary version.
    #[serde(rename = "get_binary_version")]
    GetBinaryVersion,

    /// Call an MCP tool through the CLI.
    #[serde(rename = "mcp_call")]
    McpCall(McpCallRequest),

    /// At-mention file autocomplete.
    #[serde(rename = "file_suggestions")]
    FileSuggestions(FileSuggestionsRequest),

    /// Hook callback (CLI to SDK).
    #[serde(rename = "hook_callback")]
    HookCallback(HookCallbackRequest),

    /// MCP message for an SDK-hosted server (CLI to SDK).
    #[serde(rename = "mcp_message")]
    McpMessage(McpMessageRequest),

    /// Rewind files to a user message.
    #[serde(rename = "rewind_files")]
    RewindFiles(RewindFilesRequest),

    /// Drop a pending async user message by UUID.
    #[serde(rename = "cancel_async_message")]
    CancelAsyncMessage(CancelAsyncMessageRequest),

    /// Read a file through the CLI.
    #[serde(rename = "read_file")]
    ReadFile(ReadFileRequest),

    /// Seed the CLI's read-file state.
    #[serde(rename = "seed_read_state")]
    SeedReadState(SeedReadStateRequest),

    /// Replace dynamically managed MCP servers.
    #[serde(rename = "mcp_set_servers")]
    McpSetServers(McpSetServersRequest),

    /// Register an additional repository root.
    #[serde(rename = "register_repo_root")]
    RegisterRepoRoot(RegisterRepoRootRequest),

    /// Reload plugins, commands, and MCP status.
    #[serde(rename = "reload_plugins")]
    ReloadPlugins,

    /// Reload skills.
    #[serde(rename = "reload_skills")]
    ReloadSkills,

    /// Reload output styles.
    #[serde(rename = "reload_output_styles")]
    ReloadOutputStyles,

    /// Reconnect one MCP server.
    #[serde(rename = "mcp_reconnect")]
    McpReconnect(McpReconnectRequest),

    /// Enable or disable one MCP server.
    #[serde(rename = "mcp_toggle")]
    McpToggle(McpToggleRequest),

    /// Stop a running task.
    #[serde(rename = "stop_task")]
    StopTask(StopTaskRequest),

    /// Background in-flight foreground tasks.
    #[serde(rename = "background_tasks")]
    BackgroundTasks(BackgroundTasksRequest),

    /// Output of a background task.
    #[serde(rename = "get_task_output")]
    GetTaskOutput(GetTaskOutputRequest),

    /// Merge settings into the flag settings layer.
    #[serde(rename = "apply_flag_settings")]
    ApplyFlagSettings(ApplyFlagSettingsRequest),

    /// Effective settings.
    #[serde(rename = "get_settings")]
    GetSettings,

    /// Configured hooks.
    #[serde(rename = "get_hooks_listing")]
    GetHooksListing,

    /// Write settings to the user or local settings file.
    #[serde(rename = "update_settings")]
    UpdateSettings(UpdateSettingsRequest),

    /// MCP elicitation (CLI to SDK).
    #[serde(rename = "elicitation")]
    Elicitation(ElicitationRequest),

    /// A dialog the CLI wants the host to show (CLI to SDK).
    #[serde(rename = "request_user_dialog")]
    RequestUserDialog(RequestUserDialogRequest),

    /// Permission rules in force.
    #[serde(rename = "list_permission_rules")]
    ListPermissionRules,

    /// Read an MCP resource.
    #[serde(rename = "mcp_read_resource")]
    McpReadResource(McpReadResourceRequest),

    /// OAuth access-token refresh (CLI to SDK). Not in the published type
    /// declarations; the TS SDK answers it from an internal callback.
    #[serde(rename = "oauth_token_refresh")]
    OauthTokenRefresh,

    /// Host auth-token refresh (CLI to SDK). Not in the published type
    /// declarations; the TS SDK answers it from an internal callback.
    #[serde(rename = "host_auth_token_refresh")]
    HostAuthTokenRefresh,

    /// A subtype this crate does not model.
    #[serde(other)]
    Unsupported,
}

impl ControlRequestData {
    /// Wire `subtype` for this request, or `None` for
    /// [`ControlRequestData::Unsupported`].
    pub fn subtype(&self) -> Option<String> {
        if matches!(self, Self::Unsupported) {
            return None;
        }
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.get("subtype")?.as_str().map(str::to_string))
    }
}

/// `control_cancel_request`: the CLI withdraws a control request it sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlCancelRequest {
    #[serde(rename = "type")]
    pub msg_type: ControlCancelRequestType,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlCancelRequestType {
    ControlCancelRequest,
}

/// An inbound `control_request` whose subtype this crate does not model.
///
/// Serializes as the original frame.
#[derive(Debug, Clone)]
pub struct UnsupportedControlRequest {
    pub request_id: String,
    pub subtype: String,
    pub raw: Value,
}

impl Serialize for UnsupportedControlRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

/// Hook events (0.3.289 `HookEvent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PostToolBatch,
    Notification,
    UserPromptSubmit,
    UserPromptExpansion,
    SessionStart,
    SessionEnd,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    PreModelSwitch,
    PostModelSwitch,
    PermissionRequest,
    PermissionDenied,
    Setup,
    TeammateIdle,
    TaskCreated,
    TaskCompleted,
    Elicitation,
    ElicitationResult,
    ConfigChange,
    WorktreeCreate,
    WorktreeRemove,
    InstructionsLoaded,
    CwdChanged,
    FileChanged,
    DirectoryAdded,
    MessageDisplay,
}

/// One hook matcher as registered in `initialize` (TS
/// `SDKHookCallbackMatcher`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkHookCallbackMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hook_callback_ids: Vec<String>,
    /// Seconds the CLI waits for the callbacks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

/// Initialize request data (0.3.289 `SDKControlInitializeRequest`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HashMap<HookEvent, Vec<SdkHookCallbackMatcher>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_mcp_servers: Option<Vec<String>>,
    /// Per-server settings (`timeout`) for the servers in `sdk_mcp_servers`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_mcp_server_configs: Option<HashMap<String, Value>>,
    /// Each server's own `initialize` and `tools/list` results, so the CLI
    /// skips those `mcp_message` round trips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_mcp_server_manifests: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_snapshot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_aliases: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_dynamic_sections: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_suggestions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_progress_summaries: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_subagent_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_dialog_kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_task_stop_affordance: Option<bool>,
    /// Plugins delivered over stdin (`pluginDelivery: 'initialize'`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Value>,
}

/// Interrupt request data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterruptRequest {
    /// Also drop queued user messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_queued: Option<bool>,
}

/// Permission check request from the CLI (0.3.289
/// `SDKControlPermissionRequest`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanUseToolRequest {
    pub tool_name: String,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_server: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_suggestions: Option<Vec<PermissionUpdate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    /// `rule`, `mode`, `hook`, `classifier`, and others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_approvable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_always_allow_rule: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_to_no: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_ask_rule: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub tool_use_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_user_interaction: Option<bool>,
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

/// Permission mode (unchanged from 0.3.172 to 0.3.296).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    DontAsk,
    /// Classifier-approved permission prompts (TS `'auto'`).
    Auto,
}

impl PermissionMode {
    /// Wire and CLI string for this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
            Self::Plan => "plan",
            Self::DontAsk => "dontAsk",
            Self::Auto => "auto",
        }
    }
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

/// Set the session color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetColorRequest {
    pub color: String,
}

/// Call an MCP tool through the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallRequest {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_files: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_files: Option<Value>,
}

/// Hook callback request (CLI to SDK).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCallbackRequest {
    pub callback_id: String,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

impl HookCallbackRequest {
    /// `input.hook_event_name`, when present.
    pub fn hook_event_name(&self) -> Option<&str> {
        self.input.get("hook_event_name").and_then(Value::as_str)
    }
}

/// Synchronous hook output (TS `SyncHookJSONOutput`, 0.3.289).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncHookJSONOutput {
    #[serde(rename = "continue", default, skip_serializing_if = "Option::is_none")]
    pub continue_execution: Option<bool>,
    #[serde(
        rename = "suppressOutput",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub suppress_output: Option<bool>,
    #[serde(
        rename = "stopReason",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub stop_reason: Option<String>,
    /// `approve` or `block`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(
        rename = "systemMessage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub system_message: Option<String>,
    #[serde(
        rename = "terminalSequence",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub terminal_sequence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Event-specific output, such as a PreToolUse
    /// `{"hookEventName": "PreToolUse", "permissionDecision": "deny"}`.
    #[serde(
        rename = "hookSpecificOutput",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hook_specific_output: Option<Value>,
}

impl SyncHookJSONOutput {
    /// `{"continue": true}` with no hook-specific payload.
    pub fn continue_execution() -> Self {
        Self {
            continue_execution: Some(true),
            ..Self::default()
        }
    }
}

/// Asynchronous hook output (TS `AsyncHookJSONOutput`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsyncHookJSONOutput {
    /// Always `true` on the wire.
    #[serde(rename = "async")]
    pub is_async: bool,
    #[serde(
        rename = "asyncTimeout",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub async_timeout: Option<u64>,
}

/// What a hook callback returns (TS `HookJSONOutput`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookJSONOutput {
    Async(AsyncHookJSONOutput),
    Sync(SyncHookJSONOutput),
}

impl From<SyncHookJSONOutput> for HookJSONOutput {
    fn from(output: SyncHookJSONOutput) -> Self {
        Self::Sync(output)
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
}

/// Read a file through the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    /// `utf-8` or `base64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

/// Seed the CLI's read-file state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedReadStateRequest {
    pub path: String,
    pub mtime: f64,
}

/// Register an additional repository root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRepoRootRequest {
    pub directory: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload_claude_md: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload_plugins: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload_skills: Option<bool>,
}

/// Merge settings into the flag settings layer (TS `applyFlagSettings`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyFlagSettingsRequest {
    pub settings: Value,
}

/// Write settings to a settings file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    /// `localSettings` or `userSettings`.
    pub source: String,
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

/// Output of a background task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTaskOutputRequest {
    pub task_id: String,
}

/// Background in-flight foreground tasks (TS `backgroundTasks`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackgroundTasksRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// Drop a queued async user message (TS `cancelAsyncMessage`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAsyncMessageRequest {
    pub message_uuid: String,
}

/// At-mention file autocomplete.
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

/// Read an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReadResourceRequest {
    #[serde(rename = "serverName")]
    pub server_name: String,
    pub uri: String,
}

/// Set the session title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSessionRequest {
    pub title: String,
}

/// MCP elicitation request from the CLI (0.3.289
/// `SDKControlElicitationRequest`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationRequest {
    pub mcp_server_name: String,
    pub message: String,
    /// `form` or `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elicitation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// How the user answered an elicitation (MCP `ElicitResult.action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

/// Elicitation answer (MCP `ElicitResult`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElicitationResult {
    pub action: ElicitationAction,
    /// Form values when `action` is `accept`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}

impl ElicitationResult {
    /// `{"action": "decline"}`, the TS SDK's answer when no handler is set.
    pub fn decline() -> Self {
        Self {
            action: ElicitationAction::Decline,
            content: None,
        }
    }
}

/// A dialog the CLI asks the host to show (0.3.289
/// `SDKControlRequestUserDialogRequest`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestUserDialogRequest {
    pub dialog_kind: String,
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
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

impl SdkControlResponse {
    /// A `success` response.
    pub fn success(request_id: impl Into<String>, response: Option<Value>) -> Self {
        Self {
            msg_type: ControlResponseType::ControlResponse,
            response: ControlResponseData::Success {
                request_id: request_id.into(),
                response,
                pending_permission_requests: None,
                pending_user_dialog_requests: None,
            },
        }
    }

    /// An `error` response.
    pub fn error(request_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            msg_type: ControlResponseType::ControlResponse,
            response: ControlResponseData::Error {
                request_id: request_id.into(),
                error: error.into(),
                pending_permission_requests: None,
                pending_user_dialog_requests: None,
            },
        }
    }
}

/// Control response data.
///
/// Either form may carry permission or dialog requests that were waiting
/// while the response was produced; the SDK handles them as if they had
/// arrived on their own.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlResponseData {
    #[serde(rename = "success")]
    Success {
        request_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_permission_requests: Option<Vec<SdkControlRequest>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_user_dialog_requests: Option<Vec<SdkControlRequest>>,
    },

    #[serde(rename = "error")]
    Error {
        request_id: String,
        error: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_permission_requests: Option<Vec<SdkControlRequest>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_user_dialog_requests: Option<Vec<SdkControlRequest>>,
    },
}

/// Permission result to send back for a `can_use_tool` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum PermissionResult {
    #[serde(rename = "allow")]
    Allow {
        #[serde(rename = "updatedInput")]
        updated_input: Value,
        #[serde(rename = "updatedPermissions", skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
        #[serde(rename = "toolUseID", skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        /// `user_temporary`, `user_permanent`, or `user_reject` (0.3.289).
        #[serde(
            rename = "decisionClassification",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        decision_classification: Option<String>,
    },

    #[serde(rename = "deny")]
    Deny {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
        #[serde(rename = "toolUseID", skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(
            rename = "decisionClassification",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        decision_classification: Option<String>,
    },
}

/// Most `updatedPermissions` entries a permission answer may carry, counting
/// updates, rules, and directories together. Claude Code 2.1.295 and later
/// treat an answer over this limit as a denial, so the SDK sends such an
/// answer as an explicit deny instead (see [`PermissionResult::within_limits`]).
pub const MAX_UPDATED_PERMISSIONS: usize = 4096;

impl PermissionUpdate {
    /// Rules or directories this update carries, plus one for the update.
    fn entries(&self) -> usize {
        1 + match self {
            Self::AddRules { rules, .. }
            | Self::ReplaceRules { rules, .. }
            | Self::RemoveRules { rules, .. } => rules.len(),
            Self::AddDirectories { directories, .. }
            | Self::RemoveDirectories { directories, .. } => directories.len(),
            Self::SetMode { .. } => 0,
        }
    }
}

impl PermissionResult {
    /// Updates, rules, and directories in `updatedPermissions`, counted
    /// together.
    pub fn updated_permission_entries(&self) -> usize {
        match self {
            Self::Allow {
                updated_permissions: Some(updates),
                ..
            } => updates.iter().map(PermissionUpdate::entries).sum(),
            _ => 0,
        }
    }

    /// This answer, or a deny when its `updatedPermissions` is over
    /// [`MAX_UPDATED_PERMISSIONS`]: the CLI would count it as a denial, so
    /// the host's answer says so instead of being dropped silently.
    pub fn within_limits(self) -> Self {
        let entries = self.updated_permission_entries();
        if entries <= MAX_UPDATED_PERMISSIONS {
            return self;
        }
        let tool_use_id = match &self {
            Self::Allow { tool_use_id, .. } | Self::Deny { tool_use_id, .. } => tool_use_id.clone(),
        };
        Self::Deny {
            message: format!(
                "Permission answer denied: updatedPermissions holds {entries} entries, over the limit of {MAX_UPDATED_PERMISSIONS}"
            ),
            interrupt: None,
            tool_use_id,
            decision_classification: None,
        }
    }

    /// Allow with the original input.
    pub fn allow(input: Value) -> Self {
        Self::Allow {
            updated_input: input,
            updated_permissions: None,
            tool_use_id: None,
            decision_classification: None,
        }
    }

    /// Deny with a message.
    pub fn deny(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
            interrupt: None,
            tool_use_id: None,
            decision_classification: None,
        }
    }

    /// Deny and interrupt the turn.
    pub fn deny_and_interrupt(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
            interrupt: Some(true),
            tool_use_id: None,
            decision_classification: None,
        }
    }
}
