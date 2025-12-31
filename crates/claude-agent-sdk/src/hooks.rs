//! Hook system for extending Claude Code behavior.
//!
//! Hooks allow you to intercept and modify behavior at various points during query execution.
//! This module provides types for defining hook callbacks that respond to events like tool usage,
//! session lifecycle, and permission requests.

use crate::error::Result;
use crate::protocol::PermissionUpdate;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Hook events that can trigger callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    /// Before a tool is executed.
    PreToolUse,
    /// After a tool completes successfully.
    PostToolUse,
    /// After a tool fails.
    PostToolUseFailure,
    /// When a notification is triggered.
    Notification,
    /// When the user submits a prompt.
    UserPromptSubmit,
    /// When a session starts.
    SessionStart,
    /// When a session ends.
    SessionEnd,
    /// When execution stops.
    Stop,
    /// When a subagent starts.
    SubagentStart,
    /// When a subagent stops.
    SubagentStop,
    /// Before context is compacted.
    PreCompact,
    /// When a permission is requested.
    PermissionRequest,
}

impl HookEvent {
    /// Get the string name used in the protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::PostToolUseFailure => "PostToolUseFailure",
            HookEvent::Notification => "Notification",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::SessionStart => "SessionStart",
            HookEvent::SessionEnd => "SessionEnd",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStart => "SubagentStart",
            HookEvent::SubagentStop => "SubagentStop",
            HookEvent::PreCompact => "PreCompact",
            HookEvent::PermissionRequest => "PermissionRequest",
        }
    }
}

/// Base fields common to all hook inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseHookInput {
    /// Session ID.
    pub session_id: String,
    /// Path to the transcript file.
    pub transcript_path: String,
    /// Current working directory.
    pub cwd: String,
    /// Current permission mode.
    pub permission_mode: Option<String>,
}

/// Input for PreToolUse hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreToolUseHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Tool being used.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: Value,
    /// Unique ID for this tool use.
    pub tool_use_id: String,
}

/// Input for PostToolUse hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Tool that was used.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: Value,
    /// Tool response/output.
    pub tool_response: Value,
    /// Unique ID for this tool use.
    pub tool_use_id: String,
}

/// Input for PostToolUseFailure hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseFailureHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Tool that failed.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: Value,
    /// Unique ID for this tool use.
    pub tool_use_id: String,
    /// Error message.
    pub error: String,
    /// Whether this was an interrupt.
    pub is_interrupt: Option<bool>,
}

/// Input for Notification hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Notification message.
    pub message: String,
    /// Optional title.
    pub title: Option<String>,
    /// Type of notification.
    pub notification_type: String,
}

/// Input for UserPromptSubmit hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPromptSubmitHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// The user's prompt.
    pub prompt: String,
}

/// Source of session start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStartSource {
    Startup,
    Resume,
    Clear,
    Compact,
}

/// Input for SessionStart hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Source of session start.
    pub source: SessionStartSource,
}

/// Input for SessionEnd hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Reason for session ending.
    pub reason: String,
}

/// Input for Stop hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Whether stop hook is active.
    pub stop_hook_active: bool,
}

/// Input for SubagentStart hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStartHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Agent ID.
    pub agent_id: String,
    /// Agent type.
    pub agent_type: String,
}

/// Input for SubagentStop hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStopHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Whether stop hook is active.
    pub stop_hook_active: bool,
    /// Agent ID.
    pub agent_id: String,
    /// Path to agent transcript.
    pub agent_transcript_path: String,
}

/// Trigger for compact operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactTrigger {
    Manual,
    Auto,
}

/// Input for PreCompact hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCompactHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// What triggered the compact.
    pub trigger: CompactTrigger,
    /// Custom instructions for compact.
    pub custom_instructions: Option<String>,
}

/// Input for PermissionRequest hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestHookInput {
    /// Base hook fields.
    #[serde(flatten)]
    pub base: BaseHookInput,
    /// Name of the hook event.
    pub hook_event_name: String,
    /// Tool requesting permission.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: Value,
    /// Suggested permission updates.
    pub permission_suggestions: Option<Vec<PermissionUpdate>>,
}

/// Union of all hook input types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookInput {
    PreToolUse(PreToolUseHookInput),
    PostToolUse(PostToolUseHookInput),
    PostToolUseFailure(PostToolUseFailureHookInput),
    Notification(NotificationHookInput),
    UserPromptSubmit(UserPromptSubmitHookInput),
    SessionStart(SessionStartHookInput),
    SessionEnd(SessionEndHookInput),
    Stop(StopHookInput),
    SubagentStart(SubagentStartHookInput),
    SubagentStop(SubagentStopHookInput),
    PreCompact(PreCompactHookInput),
    PermissionRequest(PermissionRequestHookInput),
}

/// Hook output for async processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncHookOutput {
    /// Indicates this is async.
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Timeout in seconds for async processing.
    #[serde(rename = "asyncTimeout", skip_serializing_if = "Option::is_none")]
    pub async_timeout: Option<u32>,
}

/// Permission decision for hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPermissionDecision {
    Allow,
    Deny,
    Ask,
}

/// Hook decision (approve/block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    Approve,
    Block,
}

/// PreToolUse hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreToolUseSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Permission decision.
    #[serde(rename = "permissionDecision", skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<HookPermissionDecision>,
    /// Reason for permission decision.
    #[serde(
        rename = "permissionDecisionReason",
        skip_serializing_if = "Option::is_none"
    )]
    pub permission_decision_reason: Option<String>,
    /// Updated tool input.
    #[serde(rename = "updatedInput", skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,
}

/// UserPromptSubmit hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPromptSubmitSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Additional context to add.
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// SessionStart hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Additional context to add.
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// SubagentStart hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStartSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Additional context to add.
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// PostToolUse hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Additional context to add.
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    /// Updated MCP tool output.
    #[serde(
        rename = "updatedMCPToolOutput",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_mcp_tool_output: Option<Value>,
}

/// PostToolUseFailure hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseFailureSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Additional context to add.
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// Permission decision from hook for PermissionRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum HookPermissionResult {
    #[serde(rename = "allow")]
    Allow {
        #[serde(rename = "updatedInput", skip_serializing_if = "Option::is_none")]
        updated_input: Option<Value>,
        #[serde(rename = "updatedPermissions", skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    #[serde(rename = "deny")]
    Deny {
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}

/// PermissionRequest hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestSpecificOutput {
    /// Hook event name.
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    /// Permission decision.
    pub decision: HookPermissionResult,
}

/// Union of hook-specific outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookSpecificOutput {
    PreToolUse(PreToolUseSpecificOutput),
    UserPromptSubmit(UserPromptSubmitSpecificOutput),
    SessionStart(SessionStartSpecificOutput),
    SubagentStart(SubagentStartSpecificOutput),
    PostToolUse(PostToolUseSpecificOutput),
    PostToolUseFailure(PostToolUseFailureSpecificOutput),
    PermissionRequest(PermissionRequestSpecificOutput),
}

/// Synchronous hook output.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncHookOutput {
    /// Whether to continue execution.
    #[serde(rename = "continue", skip_serializing_if = "Option::is_none")]
    pub continue_execution: Option<bool>,
    /// Whether to suppress output.
    #[serde(rename = "suppressOutput", skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
    /// Reason for stopping.
    #[serde(rename = "stopReason", skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Decision (approve/block).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<HookDecision>,
    /// System message to inject.
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    /// Reason for decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Hook-specific output data.
    #[serde(rename = "hookSpecificOutput", skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

impl SyncHookOutput {
    /// Create output that continues execution.
    pub fn continue_execution() -> Self {
        Self {
            continue_execution: Some(true),
            ..Default::default()
        }
    }

    /// Create output that stops execution.
    pub fn stop(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: Some(false),
            stop_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Create output that approves.
    pub fn approve() -> Self {
        Self {
            decision: Some(HookDecision::Approve),
            ..Default::default()
        }
    }

    /// Create output that blocks.
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            decision: Some(HookDecision::Block),
            reason: Some(reason.into()),
            ..Default::default()
        }
    }
}

/// Hook output (async or sync).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookOutput {
    /// Async hook processing.
    Async(AsyncHookOutput),
    /// Sync hook output.
    Sync(SyncHookOutput),
}

impl From<SyncHookOutput> for HookOutput {
    fn from(output: SyncHookOutput) -> Self {
        HookOutput::Sync(output)
    }
}

/// Trait for implementing hook callbacks.
#[async_trait]
pub trait HookCallback: Send + Sync {
    /// Called when the hook event occurs.
    ///
    /// # Arguments
    /// * `input` - The hook input containing event-specific data
    /// * `tool_use_id` - Optional tool use ID if this is tool-related
    ///
    /// # Returns
    /// A `HookOutput` indicating how to proceed.
    async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> Result<HookOutput>;
}

/// A hook callback matcher that routes events to callbacks.
#[derive(Clone)]
pub struct HookCallbackMatcher {
    /// Optional regex pattern to match against (e.g., tool names).
    pub matcher: Option<String>,
    /// Callbacks to invoke when the event matches.
    pub hooks: Vec<Arc<dyn HookCallback>>,
    /// Timeout in seconds for all hooks in this matcher.
    pub timeout: Option<u32>,
}

impl std::fmt::Debug for HookCallbackMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookCallbackMatcher")
            .field("matcher", &self.matcher)
            .field("hooks_count", &self.hooks.len())
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl HookCallbackMatcher {
    /// Create a new matcher with no pattern (matches all).
    pub fn new() -> Self {
        Self {
            matcher: None,
            hooks: Vec::new(),
            timeout: None,
        }
    }

    /// Create a new matcher with a pattern.
    pub fn with_matcher(pattern: impl Into<String>) -> Self {
        Self {
            matcher: Some(pattern.into()),
            hooks: Vec::new(),
            timeout: None,
        }
    }

    /// Add a hook callback.
    pub fn hook(mut self, callback: Arc<dyn HookCallback>) -> Self {
        self.hooks.push(callback);
        self
    }

    /// Set the timeout for this matcher.
    pub fn timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
    }
}

impl Default for HookCallbackMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple function-based hook callback implementation.
pub struct FnHookCallback<F>
where
    F: Fn(
            HookInput,
            Option<String>,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<HookOutput>> + Send>>
        + Send
        + Sync,
{
    func: F,
}

impl<F> FnHookCallback<F>
where
    F: Fn(
            HookInput,
            Option<String>,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<HookOutput>> + Send>>
        + Send
        + Sync,
{
    /// Create a new function-based callback.
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

#[async_trait]
impl<F> HookCallback for FnHookCallback<F>
where
    F: Fn(
            HookInput,
            Option<String>,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<HookOutput>> + Send>>
        + Send
        + Sync,
{
    async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> Result<HookOutput> {
        (self.func)(input, tool_use_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_as_str() {
        assert_eq!(HookEvent::PreToolUse.as_str(), "PreToolUse");
        assert_eq!(HookEvent::SessionStart.as_str(), "SessionStart");
        assert_eq!(HookEvent::PermissionRequest.as_str(), "PermissionRequest");
    }

    #[test]
    fn test_sync_hook_output_builders() {
        let output = SyncHookOutput::continue_execution();
        assert_eq!(output.continue_execution, Some(true));

        let output = SyncHookOutput::stop("user cancelled");
        assert_eq!(output.continue_execution, Some(false));
        assert_eq!(output.stop_reason, Some("user cancelled".to_string()));

        let output = SyncHookOutput::approve();
        assert_eq!(output.decision, Some(HookDecision::Approve));

        let output = SyncHookOutput::block("not allowed");
        assert_eq!(output.decision, Some(HookDecision::Block));
        assert_eq!(output.reason, Some("not allowed".to_string()));
    }

    #[test]
    fn test_hook_callback_matcher() {
        let matcher = HookCallbackMatcher::with_matcher("Bash").timeout(30);
        assert_eq!(matcher.matcher, Some("Bash".to_string()));
        assert_eq!(matcher.timeout, Some(30));
    }
}
