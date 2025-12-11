//! Claude Code Subagent
//!
//! Integration with Claude Code Agent SDK for running subtasks.
//! Implements:
//! - Exponential backoff for rate limits
//! - Retry for server errors
//! - Immediate failure for auth errors
//! - Session management and resumption

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentType, SubagentResult, Subtask, SubtaskStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Retry configuration for Claude Code
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 32000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Claude Code subagent options
#[derive(Debug, Clone)]
pub struct ClaudeCodeSubagentOptions {
    /// Working directory
    pub cwd: String,
    /// Maximum turns
    pub max_turns: Option<u32>,
    /// System prompt override
    pub system_prompt: Option<String>,
    /// Permission mode
    pub permission_mode: Option<String>,
    /// OpenAgents directory
    pub openagents_dir: Option<String>,
    /// Allowed tools
    pub allowed_tools: Option<Vec<String>>,
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Resume a prior Claude Code session
    pub resume_session_id: Option<String>,
    /// Fork the resumed session
    pub fork_session: bool,
    /// Additional context (e.g., AGENTS.md)
    pub additional_context: Option<String>,
    /// Reflections from previous failures
    pub reflections: Option<String>,
    /// TB run ID for ATIF
    pub run_id: Option<String>,
}

impl Default for ClaudeCodeSubagentOptions {
    fn default() -> Self {
        Self {
            cwd: ".".to_string(),
            max_turns: Some(300),
            system_prompt: None,
            permission_mode: None,
            openagents_dir: None,
            allowed_tools: None,
            timeout_ms: Some(50 * 60 * 1000), // 50 minutes
            resume_session_id: None,
            fork_session: false,
            additional_context: None,
            reflections: None,
            run_id: None,
        }
    }
}

/// SDK assistant message error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SDKAssistantMessageError {
    /// Authentication failed
    AuthenticationFailed,
    /// Billing error
    BillingError,
    /// Rate limit hit
    RateLimit,
    /// Invalid request
    InvalidRequest,
    /// Server error
    ServerError,
    /// Unknown error
    Unknown,
}

/// Error message descriptions
#[derive(Debug, Clone)]
pub struct ErrorDescription {
    /// Error message
    pub message: String,
    /// Suggestion for resolution
    pub suggestion: Option<String>,
}


// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Get error message descriptions
pub fn describe_assistant_error(error_type: Option<SDKAssistantMessageError>) -> ErrorDescription {
    match error_type {
        Some(SDKAssistantMessageError::AuthenticationFailed) => ErrorDescription {
            message: "Claude Code authentication failed".to_string(),
            suggestion: Some("Set a valid ANTHROPIC_API_KEY before retrying".to_string()),
        },
        Some(SDKAssistantMessageError::BillingError) => ErrorDescription {
            message: "Claude Code billing error".to_string(),
            suggestion: Some("Check billing status for Anthropic API access".to_string()),
        },
        Some(SDKAssistantMessageError::RateLimit) => ErrorDescription {
            message: "Claude Code hit rate limits".to_string(),
            suggestion: Some(
                "Back off and retry later or fall back to the minimal subagent".to_string(),
            ),
        },
        Some(SDKAssistantMessageError::InvalidRequest) => ErrorDescription {
            message: "Claude Code rejected the request".to_string(),
            suggestion: Some(
                "Inspect Claude Code inputs and configuration for invalid parameters".to_string(),
            ),
        },
        Some(SDKAssistantMessageError::ServerError) => ErrorDescription {
            message: "Claude Code server error".to_string(),
            suggestion: Some(
                "Retry after a short delay or fall back to the minimal subagent".to_string(),
            ),
        },
        Some(SDKAssistantMessageError::Unknown) | None => ErrorDescription {
            message: "Claude Code reported an error".to_string(),
            suggestion: None,
        },
    }
}

/// Check if an error is retryable
pub fn is_retryable_error(error_type: Option<SDKAssistantMessageError>) -> bool {
    matches!(
        error_type,
        Some(SDKAssistantMessageError::RateLimit) | Some(SDKAssistantMessageError::ServerError)
    )
}

/// Check if an error is a fatal auth error
pub fn is_fatal_auth_error(error_type: Option<SDKAssistantMessageError>) -> bool {
    matches!(
        error_type,
        Some(SDKAssistantMessageError::AuthenticationFailed)
            | Some(SDKAssistantMessageError::BillingError)
    )
}

/// Calculate delay for exponential backoff
pub fn calculate_backoff_delay(attempt: u32, config: &RetryConfig) -> u64 {
    let delay =
        (config.initial_delay_ms as f64) * config.backoff_multiplier.powi(attempt as i32);
    (delay as u64).min(config.max_delay_ms)
}

/// Build prompt for subtask
pub fn build_prompt(
    subtask: &Subtask,
    additional_context: Option<&str>,
    reflections: Option<&str>,
    cwd: Option<&str>,
) -> String {
    let mut prompt = String::new();

    // Prepend additional context if provided
    if let Some(ctx) = additional_context {
        prompt.push_str("## Project Context\n\n");
        prompt.push_str(ctx);
        prompt.push_str("\n\n---\n\n");
    }

    // Worktree guidance if running in isolated worktree
    if let Some(cwd_path) = cwd {
        if cwd_path.contains("/.worktrees/") {
            prompt.push_str("## IMPORTANT: Worktree Isolation\n\n");
            prompt.push_str(&format!(
                "You are running in an ISOLATED git worktree at:\n`{}`\n\n",
                cwd_path
            ));
            prompt.push_str("This is your workspace. ALL your work happens here:\n");
            prompt.push_str("- Make all file changes in this worktree directory\n");
            prompt.push_str("- Run all commands (tests, typecheck, git) in this worktree\n");
            prompt.push_str("- DO NOT try to work in \"the main repo\" or switch directories\n");
            prompt.push_str("- The worktree IS the repository - it has all project files\n");
            prompt.push_str("- Your changes will be merged to main after you complete\n\n");
            prompt.push_str(
                "Stay in this worktree for all operations. This isolation prevents conflicts with other agents.\n\n",
            );
            prompt.push_str("---\n\n");
        }
    }

    // Subtask description
    prompt.push_str(&format!("## Subtask: {}\n\n", subtask.id));
    prompt.push_str(&subtask.description);
    prompt.push_str("\n\nFocus on minimal, correct changes.");

    // Include failure context if this is a retry
    if let (Some(failure_count), Some(last_failure)) =
        (subtask.failure_count, &subtask.last_failure_reason)
    {
        if failure_count > 0 {
            prompt.push_str(&format!(
                "\n\n## IMPORTANT: Previous Attempt Failed\n\n\
                This subtask has failed {} time(s). The last failure was:\n```\n{}\n```\n\n\
                You MUST address this error before proceeding. Do NOT repeat the same approach that caused the failure.\n\
                Run `bun run typecheck` and `bun test` to verify your changes pass before completing.",
                failure_count, last_failure
            ));
        }
    }

    // Include reflections from previous failures
    if let Some(ref_text) = reflections {
        if !ref_text.trim().is_empty() {
            prompt.push_str(&format!(
                "\n\n## Learning from Previous Failures\n\n\
                The following reflections were generated from previous failed attempts. \
                Use these insights to avoid repeating the same mistakes:\n\n{}",
                ref_text
            ));
        }
    }

    prompt
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Entry Point
// ─────────────────────────────────────────────────────────────────────────────

/// Run a subtask using Claude Code Agent SDK
///
/// NOTE: This is a stub implementation. Actual SDK integration requires
/// either FFI bindings or a native Rust SDK.
pub fn run_claude_code_subagent(
    subtask: &Subtask,
    options: &ClaudeCodeSubagentOptions,
) -> AgentResult<SubagentResult> {
    // Build the prompt
    let _prompt = build_prompt(
        subtask,
        options.additional_context.as_deref(),
        options.reflections.as_deref(),
        Some(&options.cwd),
    );

    // TODO: Implement actual SDK integration
    // For now, return a placeholder result indicating SDK not available
    Ok(SubagentResult {
        success: false,
        subtask_id: subtask.id.clone(),
        files_modified: Vec::new(),
        turns: 0,
        error: Some("Claude Code SDK integration not yet implemented in Rust".to_string()),
        agent: Some(AgentType::ClaudeCode),
        claude_code_session_id: None,
        claude_code_forked_from_session_id: None,
        token_usage: None,
        verification_outputs: None,
        session_metadata: None,
        learning_metrics: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionMetadata;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_calculate_backoff_delay() {
        let config = RetryConfig::default();

        // First retry: 1000 * 2^0 = 1000
        assert_eq!(calculate_backoff_delay(0, &config), 1000);

        // Second retry: 1000 * 2^1 = 2000
        assert_eq!(calculate_backoff_delay(1, &config), 2000);

        // Third retry: 1000 * 2^2 = 4000
        assert_eq!(calculate_backoff_delay(2, &config), 4000);
    }

    #[test]
    fn test_calculate_backoff_delay_max() {
        let config = RetryConfig {
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            ..Default::default()
        };

        // 1000 * 2^3 = 8000, but capped at 5000
        assert_eq!(calculate_backoff_delay(3, &config), 5000);
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error(Some(SDKAssistantMessageError::RateLimit)));
        assert!(is_retryable_error(Some(SDKAssistantMessageError::ServerError)));
        assert!(!is_retryable_error(Some(
            SDKAssistantMessageError::AuthenticationFailed
        )));
        assert!(!is_retryable_error(None));
    }

    #[test]
    fn test_is_fatal_auth_error() {
        assert!(is_fatal_auth_error(Some(
            SDKAssistantMessageError::AuthenticationFailed
        )));
        assert!(is_fatal_auth_error(Some(SDKAssistantMessageError::BillingError)));
        assert!(!is_fatal_auth_error(Some(SDKAssistantMessageError::RateLimit)));
        assert!(!is_fatal_auth_error(None));
    }

    #[test]
    fn test_describe_assistant_error() {
        let desc = describe_assistant_error(Some(SDKAssistantMessageError::RateLimit));
        assert!(desc.message.contains("rate limit"));
        assert!(desc.suggestion.is_some());
    }

    #[test]
    fn test_describe_assistant_error_unknown() {
        let desc = describe_assistant_error(None);
        assert!(desc.message.contains("error"));
        assert!(desc.suggestion.is_none());
    }

    #[test]
    fn test_build_prompt_basic() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix the bug".to_string(),
            status: SubtaskStatus::Pending,
            ..Default::default()
        };

        let prompt = build_prompt(&subtask, None, None, None);
        assert!(prompt.contains("sub-1"));
        assert!(prompt.contains("Fix the bug"));
    }

    #[test]
    fn test_build_prompt_with_context() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix the bug".to_string(),
            status: SubtaskStatus::Pending,
            ..Default::default()
        };

        let prompt = build_prompt(&subtask, Some("Project uses Bun"), None, None);
        assert!(prompt.contains("Project Context"));
        assert!(prompt.contains("Project uses Bun"));
    }

    #[test]
    fn test_build_prompt_with_worktree() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix the bug".to_string(),
            status: SubtaskStatus::Pending,
            ..Default::default()
        };

        let prompt = build_prompt(&subtask, None, None, Some("/repo/.worktrees/task-1"));
        assert!(prompt.contains("Worktree Isolation"));
        assert!(prompt.contains("/.worktrees/"));
    }

    #[test]
    fn test_build_prompt_with_failure() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix the bug".to_string(),
            status: SubtaskStatus::Pending,
            failure_count: Some(2),
            last_failure_reason: Some("Type error in line 42".to_string()),
            ..Default::default()
        };

        let prompt = build_prompt(&subtask, None, None, None);
        assert!(prompt.contains("Previous Attempt Failed"));
        assert!(prompt.contains("Type error in line 42"));
    }

    #[test]
    fn test_build_prompt_with_reflections() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix the bug".to_string(),
            status: SubtaskStatus::Pending,
            ..Default::default()
        };

        let prompt = build_prompt(&subtask, None, Some("Always run tests before committing"), None);
        assert!(prompt.contains("Learning from Previous Failures"));
        assert!(prompt.contains("Always run tests"));
    }

    #[test]
    fn test_session_metadata_serialization() {
        let metadata = SessionMetadata {
            session_id: Some("sess-123".to_string()),
            tools_used: Some([("Edit".to_string(), 5)].into_iter().collect()),
            ..Default::default()
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("sess-123"));
        assert!(json.contains("Edit"));
    }

    #[test]
    fn test_claude_code_subagent_options_default() {
        let options = ClaudeCodeSubagentOptions::default();
        assert_eq!(options.max_turns, Some(300));
        assert!(!options.fork_session);
    }
}
