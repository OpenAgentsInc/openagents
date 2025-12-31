//! # Claude Agent SDK for Rust
//!
//! A Rust SDK for programmatically building AI agents with Claude Code's capabilities.
//! Create autonomous agents that can understand codebases, edit files, run commands,
//! and execute complex workflows.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use claude_agent_sdk::{query, QueryOptions};
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), claude_agent_sdk::Error> {
//!     // Simple one-shot query
//!     let mut stream = query("What files are in this directory?", QueryOptions::new()).await?;
//!
//!     while let Some(message) = stream.next().await {
//!         match message? {
//!             claude_agent_sdk::SdkMessage::Assistant(msg) => {
//!                 println!("Claude: {:?}", msg.message);
//!             }
//!             claude_agent_sdk::SdkMessage::Result(result) => {
//!                 println!("Query completed: {:?}", result);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## With Custom Permissions
//!
//! ```rust,no_run
//! use claude_agent_sdk::{query_with_permissions, QueryOptions, PermissionRules};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), claude_agent_sdk::Error> {
//!     let permissions = PermissionRules::new()
//!         .allow("Read")
//!         .allow("Glob")
//!         .deny("Bash")
//!         .default_allow(false)
//!         .build();
//!
//!     let options = QueryOptions::new()
//!         .model("claude-sonnet-4-5-20250929")
//!         .max_turns(10);
//!
//!     let stream = query_with_permissions(
//!         "List all Rust files",
//!         options,
//!         Arc::new(permissions),
//!     ).await?;
//!
//!     // Process stream...
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Streaming responses**: Process messages as they arrive via async streams
//! - **Custom permissions**: Fine-grained control over tool usage
//! - **Session management**: Continue or resume conversations
//! - **MCP servers**: Add custom tools via Model Context Protocol
//! - **Custom agents**: Define sub-agents with specific capabilities
//!
//! ## Protocol
//!
//! This SDK communicates with the Claude Code CLI via JSONL over stdin/stdout.
//! The CLI is spawned as a child process with `--output-format stream-json`.

pub mod error;
pub mod hooks;
pub mod options;
pub mod permissions;
pub mod protocol;
pub mod query;
pub mod session;
pub mod transport;

// Re-export main types at crate root
pub use error::{Error, Result};
pub use hooks::{
    AsyncHookOutput, BaseHookInput, CompactTrigger, FnHookCallback, HookCallback,
    HookCallbackMatcher, HookDecision, HookEvent, HookInput, HookOutput, HookPermissionDecision,
    HookPermissionResult, HookSpecificOutput, NotificationHookInput, PermissionRequestHookInput,
    PostToolUseFailureHookInput, PostToolUseHookInput, PostToolUseSpecificOutput,
    PreCompactHookInput, PreToolUseHookInput, PreToolUseSpecificOutput, SessionEndHookInput,
    SessionStartHookInput, SessionStartSource, SessionStartSpecificOutput, StopHookInput,
    SubagentStartHookInput, SubagentStartSpecificOutput, SubagentStopHookInput, SyncHookOutput,
    UserPromptSubmitHookInput, UserPromptSubmitSpecificOutput,
};
pub use options::{
    AgentDefinition, AgentModel, McpServerConfig, OutputFormat, PluginConfig, QueryOptions,
    RipgrepConfig, SandboxNetworkConfig, SandboxSettings, SdkBeta, SettingSource,
    SystemPromptConfig, ToolPreset, ToolsConfig,
};
pub use permissions::{
    AllowAllPermissions, CallbackPermissionHandler, DenyAllPermissions, PermissionHandler,
    PermissionRequest, PermissionRules, RulesPermissionHandler, permission_handler,
};
pub use protocol::{
    AccountInfo, ApiErrorMessage, AssistantMessageError, InformationalMessage, KeepAliveMessage,
    LocalCommandMessage, ModelInfo, ModelUsage, PermissionBehavior, PermissionDenial,
    PermissionMode, PermissionResult, PermissionRule, PermissionUpdate, ResultError, ResultSuccess,
    SdkAssistantMessage, SdkAuthStatusMessage, SdkControlRequest, SdkControlResponse, SdkMessage,
    SdkResultMessage, SdkStatus, SdkStreamEvent, SdkSystemMessage, SdkToolProgressMessage,
    SdkUserMessage, SlashCommand, StdinMessage, StdoutMessage, StopHookSummaryMessage, Usage,
};
pub use query::Query;
pub use session::{
    Session, unstable_v2_create_session, unstable_v2_prompt, unstable_v2_resume_session,
};
pub use transport::{ExecutableConfig, ProcessTransport};

use std::sync::Arc;

/// Create a new query with default permissions (allow all).
///
/// # Arguments
/// * `prompt` - The prompt to send to Claude
/// * `options` - Query configuration options
///
/// # Returns
/// A `Query` stream that yields `SdkMessage` items.
///
/// # Example
/// ```rust,no_run
/// use claude_agent_sdk::{query, QueryOptions};
/// use futures::StreamExt;
///
/// # async fn example() -> Result<(), claude_agent_sdk::Error> {
/// let mut stream = query("What is 2 + 2?", QueryOptions::new()).await?;
///
/// while let Some(msg) = stream.next().await {
///     println!("{:?}", msg?);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn query(prompt: impl Into<String>, options: QueryOptions) -> Result<Query> {
    Query::new(prompt, options, Some(Arc::new(AllowAllPermissions))).await
}

/// Create a new query with custom permissions.
///
/// # Arguments
/// * `prompt` - The prompt to send to Claude
/// * `options` - Query configuration options
/// * `permissions` - Permission handler for tool use requests
///
/// # Example
/// ```rust,no_run
/// use claude_agent_sdk::{query_with_permissions, QueryOptions, PermissionRules};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), claude_agent_sdk::Error> {
/// let permissions = PermissionRules::new()
///     .allow("Read")
///     .deny("Bash")
///     .default_allow(false)
///     .build();
///
/// let stream = query_with_permissions(
///     "Read the README",
///     QueryOptions::new(),
///     Arc::new(permissions),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_with_permissions(
    prompt: impl Into<String>,
    options: QueryOptions,
    permissions: Arc<dyn PermissionHandler>,
) -> Result<Query> {
    Query::new(prompt, options, Some(permissions)).await
}

/// Create a new query without any permission handling.
///
/// This will deny all tool use requests unless the CLI is configured
/// to bypass permissions.
///
/// # Warning
/// This is primarily useful for testing or when using `permissionMode: 'bypassPermissions'`.
pub async fn query_no_permissions(
    prompt: impl Into<String>,
    options: QueryOptions,
) -> Result<Query> {
    Query::new(prompt, options, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_rules_builder() {
        let rules = PermissionRules::new()
            .allow("Read")
            .allow("Glob")
            .deny("Bash")
            .default_allow(false)
            .build();

        // Just verify it builds without error
        let _ = rules;
    }

    #[test]
    fn test_query_options_builder() {
        let options = QueryOptions::new()
            .model("claude-sonnet-4-5-20250929")
            .max_turns(10)
            .max_budget_usd(5.0)
            .include_partial_messages(true);

        assert_eq!(
            options.model,
            Some("claude-sonnet-4-5-20250929".to_string())
        );
        assert_eq!(options.max_turns, Some(10));
        assert_eq!(options.max_budget_usd, Some(5.0));
        assert!(options.include_partial_messages);
    }

    #[test]
    fn test_permission_result_helpers() {
        let allow = PermissionResult::allow(serde_json::json!({"command": "ls"}));
        let deny = PermissionResult::deny("Not allowed");
        let deny_interrupt = PermissionResult::deny_and_interrupt("Stop everything");

        match allow {
            PermissionResult::Allow { updated_input, .. } => {
                assert_eq!(updated_input, serde_json::json!({"command": "ls"}));
            }
            _ => panic!("Expected Allow"),
        }

        match deny {
            PermissionResult::Deny {
                message, interrupt, ..
            } => {
                assert_eq!(message, "Not allowed");
                assert!(interrupt.is_none());
            }
            _ => panic!("Expected Deny"),
        }

        match deny_interrupt {
            PermissionResult::Deny {
                message, interrupt, ..
            } => {
                assert_eq!(message, "Stop everything");
                assert_eq!(interrupt, Some(true));
            }
            _ => panic!("Expected Deny with interrupt"),
        }
    }
}
