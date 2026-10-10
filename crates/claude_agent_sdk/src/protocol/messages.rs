//! SDK message types from Claude Code CLI.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

/// Known wire `type` values modelled as first-class [`SdkMessage`] variants.
///
/// Control/keepalive frames are not in this list; they belong on
/// [`crate::protocol::StdoutMessage`].
pub const KNOWN_SDK_MESSAGE_TYPES: &[&str] = &[
    "assistant",
    "user",
    "result",
    "system",
    "stream_event",
    "tool_progress",
    "auth_status",
    "prompt_suggestion",
    "rate_limit_event",
    "tool_use_summary",
    "active_goal",
    "conversation_reset",
];

/// Internally tagged union of recognized SDK stdout messages (0.3.296 set).
/// Unknown future `type` values become [`SdkMessage::Unknown`] instead of a
/// deserialize failure, so hosts can count protocol drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KnownSdkMessage {
    #[serde(rename = "assistant")]
    Assistant(SdkAssistantMessage),
    #[serde(rename = "user")]
    User(SdkUserMessage),
    #[serde(rename = "result")]
    Result(SdkResultMessage),
    #[serde(rename = "system")]
    System(SdkSystemMessage),
    #[serde(rename = "stream_event")]
    StreamEvent(SdkStreamEvent),
    #[serde(rename = "tool_progress")]
    ToolProgress(SdkToolProgressMessage),
    #[serde(rename = "auth_status")]
    AuthStatus(SdkAuthStatusMessage),
    #[serde(rename = "prompt_suggestion")]
    PromptSuggestion(SdkPromptSuggestionMessage),
    #[serde(rename = "rate_limit_event")]
    RateLimitEvent(SdkRateLimitEvent),
    #[serde(rename = "tool_use_summary")]
    ToolUseSummary(SdkToolUseSummaryMessage),
    #[serde(rename = "active_goal")]
    ActiveGoal(SdkActiveGoalMessage),
    #[serde(rename = "conversation_reset")]
    ConversationReset(SdkConversationResetMessage),
}

/// All SDK message types from CLI stdout.
#[derive(Debug, Clone)]
pub enum SdkMessage {
    /// Assistant response message
    Assistant(SdkAssistantMessage),

    /// User message (echo or replay)
    User(SdkUserMessage),

    /// Query result (success or error)
    Result(SdkResultMessage),

    /// System messages (init, status, hooks, tasks, etc.)
    System(SdkSystemMessage),

    /// Streaming partial message
    StreamEvent(SdkStreamEvent),

    /// Tool progress update
    ToolProgress(SdkToolProgressMessage),

    /// Authentication status
    AuthStatus(SdkAuthStatusMessage),

    /// Predicted next user prompt (`promptSuggestions` enabled)
    PromptSuggestion(SdkPromptSuggestionMessage),

    /// Rate-limit state change for claude.ai subscription users
    RateLimitEvent(SdkRateLimitEvent),

    /// Per-tool-group rollup summary
    ToolUseSummary(SdkToolUseSummaryMessage),

    /// The session's active goal changed or cleared (0.3.289)
    ActiveGoal(SdkActiveGoalMessage),

    /// The conversation was reset under a new conversation ID (0.3.289)
    ConversationReset(SdkConversationResetMessage),

    /// Valid JSON whose `type` is not a modelled SDK variant.
    ///
    /// The original object is preserved in `raw` so hosts can log or count
    /// drift without dropping the line.
    Unknown { type_name: String, raw: Value },
}

impl SdkMessage {
    /// Map a parsed JSON object onto a typed variant, or [`Self::Unknown`].
    pub fn from_wire_value(value: Value) -> Self {
        match serde_json::from_value::<KnownSdkMessage>(value.clone()) {
            Ok(known) => known.into(),
            Err(_) => {
                let type_name = value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned();
                SdkMessage::Unknown {
                    type_name,
                    raw: value,
                }
            }
        }
    }

    /// Wire `type` string for this message.
    pub fn type_name(&self) -> &str {
        match self {
            SdkMessage::Assistant(_) => "assistant",
            SdkMessage::User(_) => "user",
            SdkMessage::Result(_) => "result",
            SdkMessage::System(_) => "system",
            SdkMessage::StreamEvent(_) => "stream_event",
            SdkMessage::ToolProgress(_) => "tool_progress",
            SdkMessage::AuthStatus(_) => "auth_status",
            SdkMessage::PromptSuggestion(_) => "prompt_suggestion",
            SdkMessage::RateLimitEvent(_) => "rate_limit_event",
            SdkMessage::ToolUseSummary(_) => "tool_use_summary",
            SdkMessage::ActiveGoal(_) => "active_goal",
            SdkMessage::ConversationReset(_) => "conversation_reset",
            SdkMessage::Unknown { type_name, .. } => type_name,
        }
    }
}

impl From<KnownSdkMessage> for SdkMessage {
    fn from(known: KnownSdkMessage) -> Self {
        match known {
            KnownSdkMessage::Assistant(v) => SdkMessage::Assistant(v),
            KnownSdkMessage::User(v) => SdkMessage::User(v),
            KnownSdkMessage::Result(v) => SdkMessage::Result(v),
            KnownSdkMessage::System(v) => SdkMessage::System(v),
            KnownSdkMessage::StreamEvent(v) => SdkMessage::StreamEvent(v),
            KnownSdkMessage::ToolProgress(v) => SdkMessage::ToolProgress(v),
            KnownSdkMessage::AuthStatus(v) => SdkMessage::AuthStatus(v),
            KnownSdkMessage::PromptSuggestion(v) => SdkMessage::PromptSuggestion(v),
            KnownSdkMessage::RateLimitEvent(v) => SdkMessage::RateLimitEvent(v),
            KnownSdkMessage::ToolUseSummary(v) => SdkMessage::ToolUseSummary(v),
            KnownSdkMessage::ActiveGoal(v) => SdkMessage::ActiveGoal(v),
            KnownSdkMessage::ConversationReset(v) => SdkMessage::ConversationReset(v),
        }
    }
}

impl Serialize for SdkMessage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            SdkMessage::Unknown { raw, .. } => raw.serialize(serializer),
            SdkMessage::Assistant(v) => KnownSdkMessage::Assistant(v.clone()).serialize(serializer),
            // The user message carries its own `type` field.
            SdkMessage::User(v) => v.serialize(serializer),
            SdkMessage::Result(v) => KnownSdkMessage::Result(v.clone()).serialize(serializer),
            SdkMessage::System(v) => KnownSdkMessage::System(v.clone()).serialize(serializer),
            SdkMessage::StreamEvent(v) => {
                KnownSdkMessage::StreamEvent(v.clone()).serialize(serializer)
            }
            SdkMessage::ToolProgress(v) => {
                KnownSdkMessage::ToolProgress(v.clone()).serialize(serializer)
            }
            SdkMessage::AuthStatus(v) => {
                KnownSdkMessage::AuthStatus(v.clone()).serialize(serializer)
            }
            SdkMessage::PromptSuggestion(v) => {
                KnownSdkMessage::PromptSuggestion(v.clone()).serialize(serializer)
            }
            SdkMessage::RateLimitEvent(v) => {
                KnownSdkMessage::RateLimitEvent(v.clone()).serialize(serializer)
            }
            SdkMessage::ToolUseSummary(v) => {
                KnownSdkMessage::ToolUseSummary(v.clone()).serialize(serializer)
            }
            SdkMessage::ActiveGoal(v) => {
                KnownSdkMessage::ActiveGoal(v.clone()).serialize(serializer)
            }
            SdkMessage::ConversationReset(v) => {
                KnownSdkMessage::ConversationReset(v.clone()).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for SdkMessage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(SdkMessage::from_wire_value(value))
    }
}

/// Assistant message from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkAssistantMessage {
    /// The API message content
    pub message: Value,
    /// Parent tool use ID if this is part of a tool call
    pub parent_tool_use_id: Option<String>,
    /// Error type if there was an error
    #[serde(default)]
    pub error: Option<AssistantMessageError>,
    /// The user message this reply answers (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuid: Option<String>,
    /// Every user message this reply answers when turns were coalesced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuids: Option<Vec<String>>,
    /// Why the turn resumed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_reason: Option<String>,
    /// `true` when the message was cut off by an abort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aborted: Option<bool>,
    /// ISO 8601 time the message was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Context-window usage snapshot, when the CLI attaches one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_usage: Option<Value>,
    /// Usage report, when the CLI attaches one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_report: Option<Value>,
    /// The subagent that produced this message: the `task_id` of its task
    /// events, unchanged when the subagent resumes. Absent on main-thread
    /// messages (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Subagent type that produced this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    /// Description of the subagent task that produced this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
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
    OauthOrgNotAllowed,
    BillingError,
    RateLimit,
    Overloaded,
    InvalidRequest,
    ModelNotFound,
    ServerError,
    MaxOutputTokens,
    AccountOnHold,
    VerificationRequired,
    CloudCredentialError,
    #[serde(other)]
    Unknown,
}

/// User message to send to Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkUserMessage {
    /// Message type marker. Defaulted when parsing: inside [`SdkMessage`]
    /// the `type` tag is consumed by the enum.
    #[serde(rename = "type", default)]
    pub msg_type: UserMessageType,
    /// The message content (APIUserMessage format)
    pub message: Value,
    /// Parent tool use ID if responding to a tool call
    pub parent_tool_use_id: Option<String>,
    /// Whether this is a synthetic (system-generated) message (TS
    /// `isSynthetic`).
    #[serde(
        rename = "isSynthetic",
        alias = "is_synthetic",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_synthetic: Option<bool>,
    /// Tool use result if responding to a tool call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<Value>,
    /// Unique message ID (optional for new messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Session ID
    pub session_id: String,
    /// True if this is a replay/acknowledgment (TS `isReplay`).
    #[serde(
        rename = "isReplay",
        alias = "is_replay",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_replay: Option<bool>,
    /// Deliver the text as written: no `@path` expansion and no
    /// slash-command dispatch (TS `client_composed`, 0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_composed: Option<bool>,
    /// The subagent that produced this message (0.3.292); absent on
    /// main-thread messages and on prompts the host sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserMessageType {
    #[default]
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
    /// HTTP status when the turn ended on an API error (TS `api_error_status`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_error_status: Option<i64>,
    pub num_turns: u32,
    pub result: String,
    pub total_cost_usd: f64,
    pub usage: Usage,
    #[serde(rename = "modelUsage")]
    pub model_usage: HashMap<String, ModelUsage>,
    pub permission_denials: Vec<PermissionDenial>,
    #[serde(default)]
    pub structured_output: Option<Value>,
    /// Why the turn ended (TS `terminal_reason`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<TerminalReason>,
    #[serde(flatten)]
    pub turn: ResultTurnFields,
    pub uuid: String,
    pub session_id: String,
}

/// Optional result fields shared by success and error results (0.3.289).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultTurnFields {
    /// The user message this result answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuid: Option<String>,
    /// Every user message this result answers when turns were coalesced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuids: Option<Vec<String>>,
    /// Why the turn resumed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_reason: Option<String>,
    /// Turns still queued behind this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_turn_count: Option<u64>,
    /// Position of this result among results for one input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_index: Option<u64>,
    /// Why fast mode was off for this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode_disabled_reason: Option<String>,
    /// Why the session failed to start (error results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_failure_reason: Option<String>,
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
    /// Why the turn ended (TS `terminal_reason`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<TerminalReason>,
    #[serde(flatten)]
    pub turn: ResultTurnFields,
    pub uuid: String,
    pub session_id: String,
}

/// Structured turn-end reason on result messages (0.3.289 `TerminalReason`).
///
/// Later CLI values deserialize as [`TerminalReason::Unknown`] so the result
/// stays a typed [`SdkResultMessage`] instead of [`SdkMessage::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalReason {
    BlockingLimit,
    RapidRefillBreaker,
    PromptTooLong,
    ImageError,
    ModelError,
    ApiError,
    MalformedToolUseExhausted,
    AbortedStreaming,
    AbortedTools,
    StopHookPrevented,
    HookStopped,
    ToolDeferred,
    MaxTurns,
    BackgroundRequested,
    Completed,
    BudgetExhausted,
    StructuredOutputRetryExhausted,
    ToolDeferredUnavailable,
    TurnSetupFailed,
    #[serde(other)]
    Unknown,
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
    /// Per-model output cap (TS `maxOutputTokens`, required at 0.3.172).
    /// Optional on the wire so older fixtures without the field still parse.
    #[serde(
        rename = "maxOutputTokens",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_output_tokens: Option<u64>,
    /// Thinking tokens, when reported (0.3.289).
    #[serde(
        rename = "thinkingTokens",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub thinking_tokens: Option<u64>,
    /// Canonical model ID behind an alias (0.3.289).
    #[serde(
        rename = "canonicalModel",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub canonical_model: Option<String>,
    /// Serving provider (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// `list`, `managed`, or `unknown` (0.3.289).
    #[serde(rename = "costBasis", default, skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<String>,
}

/// Permission denial record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDenial {
    pub tool_name: String,
    pub tool_use_id: String,
    pub tool_input: Value,
}

/// System message types (`type: "system"` plus a `subtype` discriminator).
///
/// Subtypes follow `@anthropic-ai/claude-agent-sdk` 0.3.296. An unrecognized
/// subtype fails this enum and is recovered as [`SdkMessage::Unknown`].
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

    /// In-turn API retry progress
    #[serde(rename = "api_retry")]
    ApiRetry(SdkApiRetryMessage),

    /// Primary model refused; turn retried on a fallback model
    #[serde(rename = "model_refusal_fallback")]
    ModelRefusalFallback(SdkModelRefusalFallbackMessage),

    /// Output from a local slash command
    #[serde(rename = "local_command_output")]
    LocalCommandOutput(SdkLocalCommandOutputMessage),

    /// Hook invocation started
    #[serde(rename = "hook_started")]
    HookStarted(SdkHookStartedMessage),

    /// Hook invocation streaming progress
    #[serde(rename = "hook_progress")]
    HookProgress(SdkHookProgressMessage),

    /// Hook invocation finished
    #[serde(rename = "hook_response")]
    HookResponse(HookResponse),

    /// Plugin install lifecycle
    #[serde(rename = "plugin_install")]
    PluginInstall(SdkPluginInstallMessage),

    /// Background/subagent task settled
    #[serde(rename = "task_notification")]
    TaskNotification(SdkTaskNotificationMessage),

    /// Background/subagent task started
    #[serde(rename = "task_started")]
    TaskStarted(SdkTaskStartedMessage),

    /// Background/subagent task fields changed
    #[serde(rename = "task_updated")]
    TaskUpdated(SdkTaskUpdatedMessage),

    /// Background/subagent task live progress
    #[serde(rename = "task_progress")]
    TaskProgress(SdkTaskProgressMessage),

    /// Live thinking-token estimate
    #[serde(rename = "thinking_tokens")]
    ThinkingTokens(SdkThinkingTokensMessage),

    /// Session idle/running/requires_action
    #[serde(rename = "session_state_changed")]
    SessionStateChanged(SdkSessionStateChangedMessage),

    /// Slash-command inventory changed
    #[serde(rename = "commands_changed")]
    CommandsChanged(SdkCommandsChangedMessage),

    /// Loop-side text notification
    #[serde(rename = "notification")]
    Notification(SdkNotificationMessage),

    /// Files persisted to the session store
    #[serde(rename = "files_persisted")]
    FilesPersisted(SdkFilesPersistedEvent),

    /// Memory-recall surface
    #[serde(rename = "memory_recall")]
    MemoryRecall(SdkMemoryRecallMessage),

    /// MCP elicitation completed
    #[serde(rename = "elicitation_complete")]
    ElicitationComplete(SdkElicitationCompleteMessage),

    /// Tool permission denied
    #[serde(rename = "permission_denied")]
    PermissionDenied(SdkPermissionDeniedMessage),

    /// Transcript-mirror batch failed after retry
    #[serde(rename = "mirror_error")]
    MirrorError(SdkMirrorErrorMessage),

    /// Progress on a control request the SDK sent (0.3.289)
    #[serde(rename = "control_request_progress")]
    ControlRequestProgress(SdkControlRequestProgressMessage),

    /// Primary model refused and no fallback ran (0.3.289)
    #[serde(rename = "model_refusal_no_fallback")]
    ModelRefusalNoFallback(SdkModelRefusalNoFallbackMessage),

    /// The set of background tasks changed (0.3.289)
    #[serde(rename = "background_tasks_changed")]
    BackgroundTasksChanged(SdkBackgroundTasksChangedMessage),

    /// The worker serving the session is shutting down (0.3.289)
    #[serde(rename = "worker_shutting_down")]
    WorkerShuttingDown(SdkWorkerShuttingDownMessage),

    /// Informational notice for the user (0.3.289)
    #[serde(rename = "informational")]
    Informational(SdkInformationalMessage),
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
    /// Plugins that failed to load (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_errors: Option<Vec<Value>>,
    /// Effort level in force (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Why fast mode is off (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode_disabled_reason: Option<String>,
    /// Session capabilities the CLI advertises (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_id: Option<String>,
    pub hook_name: String,
    pub hook_event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
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
    /// Liveness tick with no new progress (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<bool>,
    /// Subagent type when the tool runs a subagent (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    /// Subagent API retry state (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_retry: Option<Value>,
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

/// Predicted next user prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPromptSuggestionMessage {
    pub suggestion: String,
    pub uuid: String,
    pub session_id: String,
}

/// Rate-limit event emitted when subscription rate-limit info changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkRateLimitEvent {
    pub rate_limit_info: SdkRateLimitInfo,
    pub uuid: String,
    pub session_id: String,
}

/// Rate-limit information for claude.ai subscription users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkRateLimitInfo {
    pub status: String,
    #[serde(rename = "resetsAt", default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    #[serde(
        rename = "rateLimitType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub rate_limit_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utilization: Option<f64>,
    #[serde(
        rename = "overageStatus",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub overage_status: Option<String>,
    #[serde(
        rename = "overageResetsAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub overage_resets_at: Option<i64>,
    #[serde(
        rename = "overageDisabledReason",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub overage_disabled_reason: Option<String>,
    #[serde(
        rename = "isUsingOverage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_using_overage: Option<bool>,
    #[serde(
        rename = "overageInUse",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub overage_in_use: Option<bool>,
    /// On usage-limit warnings: whether the account has extra usage turned
    /// on (0.3.295).
    #[serde(
        rename = "overageEnabled",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub overage_enabled: Option<bool>,
    #[serde(
        rename = "surpassedThreshold",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub surpassed_threshold: Option<f64>,
    /// `service`, `channel`, or `group_pool` (0.3.289).
    #[serde(
        rename = "limitScope",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub limit_scope: Option<String>,
    /// `credits_required` (0.3.289).
    #[serde(rename = "errorCode", default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(
        rename = "canUserPurchaseCredits",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub can_user_purchase_credits: Option<bool>,
    #[serde(
        rename = "hasChargeableSavedPaymentMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub has_chargeable_saved_payment_method: Option<bool>,
}

/// Per-tool-group rollup summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkToolUseSummaryMessage {
    pub summary: String,
    pub preceding_tool_use_ids: Vec<String>,
    pub uuid: String,
    pub session_id: String,
}

/// In-turn API retry progress (`subtype: api_retry`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkApiRetryMessage {
    pub attempt: u32,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub error_status: Option<i64>,
    pub error: AssistantMessageError,
    /// Present when the retry follows a request that got no response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_response: Option<SdkApiRetryNoResponse>,
    pub uuid: String,
    pub session_id: String,
}

/// Wait times for an `api_retry` after no response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkApiRetryNoResponse {
    pub waited_ms: u64,
    pub retry_wait_ms: u64,
}

/// Model-refusal fallback notice (`subtype: model_refusal_fallback`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkModelRefusalFallbackMessage {
    pub trigger: String,
    /// `session` or `local` (0.3.289).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub direction: String,
    pub original_model: String,
    pub fallback_model: String,
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_refusal_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_refusal_explanation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retracted_message_uuids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_user_message_uuid: Option<String>,
    pub content: String,
    pub uuid: String,
    pub session_id: String,
}

/// Output from a local slash command (`subtype: local_command_output`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkLocalCommandOutputMessage {
    pub content: String,
    pub uuid: String,
    pub session_id: String,
}

/// Hook invocation started (`subtype: hook_started`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkHookStartedMessage {
    pub hook_id: String,
    pub hook_name: String,
    pub hook_event: String,
    pub uuid: String,
    pub session_id: String,
}

/// Hook invocation streaming progress (`subtype: hook_progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkHookProgressMessage {
    pub hook_id: String,
    pub hook_name: String,
    pub hook_event: String,
    pub stdout: String,
    pub stderr: String,
    pub output: String,
    pub uuid: String,
    pub session_id: String,
}

/// Plugin install lifecycle (`subtype: plugin_install`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPluginInstallMessage {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Background/subagent task settled (`subtype: task_notification`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskNotificationMessage {
    pub task_id: String,
    /// One run of the task; a resumed task keeps its `task_id` and gets a
    /// new `run_id` (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    pub status: String,
    pub output_file: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<SdkTaskUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_transcript: Option<bool>,
    /// `worker_restart` when the task settled because its worker restarted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_links: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<bool>,
    pub uuid: String,
    pub session_id: String,
}

/// Background/subagent task started (`subtype: task_started`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskStartedMessage {
    pub task_id: String,
    /// One run of the task; a resumed task keeps its `task_id` and gets a
    /// new `run_id` (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_transcript: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_backgrounded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_depth: Option<u32>,
    /// `task_id` of the subagent task that launched this task; absent when
    /// the main thread launched it (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<bool>,
    pub uuid: String,
    pub session_id: String,
}

/// Background/subagent task fields changed (`subtype: task_updated`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskUpdatedMessage {
    pub task_id: String,
    /// One run of the task; a resumed task keeps its `task_id` and gets a
    /// new `run_id` (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub patch: SdkTaskUpdatedPatch,
    pub uuid: String,
    pub session_id: String,
}

/// Wire-safe subset of task fields that changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskUpdatedPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_paused_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_backgrounded: Option<bool>,
}

/// Background/subagent task live progress (`subtype: task_progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskProgressMessage {
    pub task_id: String,
    /// One run of the task; a resumed task keeps its `task_id` and gets a
    /// new `run_id` (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    pub usage: SdkTaskUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Token/tool/duration counters on task messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkTaskUsage {
    pub total_tokens: u64,
    pub tool_uses: u64,
    pub duration_ms: u64,
}

/// Live thinking-token estimate (`subtype: thinking_tokens`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkThinkingTokensMessage {
    pub estimated_tokens: u64,
    pub estimated_tokens_delta: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuid: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// Session idle/running/requires_action (`subtype: session_state_changed`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkSessionStateChangedMessage {
    pub state: String,
    pub uuid: String,
    pub session_id: String,
}

/// Slash-command inventory changed (`subtype: commands_changed`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkCommandsChangedMessage {
    pub commands: Vec<SdkSlashCommand>,
    pub uuid: String,
    pub session_id: String,
}

/// Slash command advertised on `commands_changed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkSlashCommand {
    pub name: String,
    pub description: String,
    #[serde(rename = "argumentHint", default)]
    pub argument_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
}

/// Loop-side text notification (`subtype: notification`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkNotificationMessage {
    pub key: String,
    pub text: String,
    pub priority: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    pub uuid: String,
    pub session_id: String,
}

/// Files persisted to the session store (`subtype: files_persisted`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkFilesPersistedEvent {
    pub files: Vec<SdkPersistedFile>,
    pub failed: Vec<SdkPersistFailedFile>,
    pub processed_at: String,
    pub uuid: String,
    pub session_id: String,
}

/// Successfully persisted file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPersistedFile {
    pub filename: String,
    pub file_id: String,
}

/// File that failed to persist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPersistFailedFile {
    pub filename: String,
    pub error: String,
}

/// Memory-recall surface (`subtype: memory_recall`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMemoryRecallMessage {
    pub mode: String,
    pub memories: Vec<SdkRecalledMemory>,
    pub uuid: String,
    pub session_id: String,
}

/// One recalled memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkRecalledMemory {
    pub path: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// MCP elicitation completed (`subtype: elicitation_complete`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkElicitationCompleteMessage {
    pub mcp_server_name: String,
    pub elicitation_id: String,
    pub uuid: String,
    pub session_id: String,
}

/// Tool permission denied (`subtype: permission_denied`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPermissionDeniedMessage {
    pub tool_name: String,
    pub tool_use_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    pub message: String,
    pub uuid: String,
    pub session_id: String,
}

/// Transcript-mirror batch failed (`subtype: mirror_error`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMirrorErrorMessage {
    pub error: String,
    pub key: SdkMirrorErrorKey,
    pub uuid: String,
    pub session_id: String,
}

/// Session-store key on a mirror error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMirrorErrorKey {
    #[serde(rename = "projectKey")]
    pub project_key: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
}

/// Progress on a control request the SDK sent
/// (`subtype: control_request_progress`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkControlRequestProgressMessage {
    pub request_id: String,
    /// `started` or `api_retry`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_delay_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_status: Option<i64>,
    pub uuid: String,
    pub session_id: String,
}

/// Primary model refused and no fallback ran
/// (`subtype: model_refusal_no_fallback`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkModelRefusalNoFallbackMessage {
    pub original_model: String,
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_refusal_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_refusal_explanation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_user_message_uuid: Option<String>,
    pub content: String,
    pub uuid: String,
    pub session_id: String,
}

/// The set of background tasks changed
/// (`subtype: background_tasks_changed`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkBackgroundTasksChangedMessage {
    pub tasks: Vec<SdkBackgroundTask>,
    pub uuid: String,
    pub session_id: String,
}

/// One running background task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkBackgroundTask {
    pub task_id: String,
    /// One run of the task; a resumed task keeps its `task_id` and gets a
    /// new `run_id` (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub task_type: String,
    /// Agent type of a `local_agent` task (0.3.293).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    pub description: String,
    /// `task_id` of the subagent task that launched this one (0.3.292).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<bool>,
}

/// The worker serving the session is shutting down
/// (`subtype: worker_shutting_down`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkWorkerShuttingDownMessage {
    pub reason: String,
    pub uuid: String,
    pub session_id: String,
}

/// Informational notice (`subtype: informational`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkInformationalMessage {
    pub content: String,
    /// `info`, `notice`, `suggestion`, or `warning`.
    pub level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prevent_continuation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub uuid: String,
    pub session_id: String,
}

/// The session's active goal (`type: active_goal`, 0.3.289). `value` is
/// `None` when the goal was cleared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkActiveGoalMessage {
    pub value: Option<SdkActiveGoal>,
    pub uuid: String,
    pub session_id: String,
}

/// Active goal state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkActiveGoal {
    pub condition: String,
    pub iterations: u64,
    pub set_at: f64,
    pub tokens_at_start: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reason: Option<String>,
}

/// The conversation was reset (`type: conversation_reset`, 0.3.289).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConversationResetMessage {
    pub new_conversation_id: String,
    /// `clear`, `plan_mode_exit`, `fresh_session`, or `onboarding`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    pub uuid: String,
    pub session_id: String,
}
