//! Claude executor using claude-agent-sdk.
//!
//! Uses Claude Pro/Max subscription for full agentic execution.
//! Falls back to Cerebras TieredExecutor if Claude is not authenticated.
//!
//! ## RLM Integration
//!
//! The executor supports RLM (Recursive Language Model) integration via:
//! - `execute_with_rlm()`: Adds RLM tools and agent for deep analysis
//! - MCP server: Claude can invoke `rlm_query` and `rlm_fanout` tools

use crate::rlm_agent::rlm_agent_definition;
use crate::{AdjutantError, Task, TaskResult, ToolRegistry};
use claude_agent_sdk::{
    query, McpServerConfig, QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig,
};
use futures::StreamExt;
use std::path::Path;

/// Executor that uses Claude via claude-agent-sdk.
///
/// This executor spawns the Claude CLI as a subprocess and communicates
/// via JSONL over stdin/stdout. It uses the CLI's own authentication,
/// so users with Claude Pro/Max subscriptions can leverage their existing
/// subscription without additional API keys.
pub struct ClaudeExecutor {
    workspace_root: std::path::PathBuf,
}

impl ClaudeExecutor {
    /// Create a new Claude executor.
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    /// Execute a task using Claude.
    ///
    /// This method:
    /// 1. Builds a prompt from the task
    /// 2. Spawns the Claude CLI with appropriate options
    /// 3. Streams results and collects the final output
    /// 4. Returns a TaskResult with success/failure and usage stats
    pub async fn execute(
        &self,
        task: &Task,
        _context: &str,
        _tools: &mut ToolRegistry,
    ) -> Result<TaskResult, AdjutantError> {
        tracing::info!(
            "ClaudeExecutor: Using Claude Pro/Max for task '{}'",
            task.title
        );

        // Build prompt from task
        let prompt = task.to_prompt();

        // Configure query options
        // Use all default Claude Code tools via preset
        let options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .tools(ToolsConfig::claude_code_preset())
            .max_turns(20);

        // Run query with all permissions allowed
        let mut stream =
            query(&prompt, options)
                .await
                .map_err(|e| {
                    AdjutantError::ExecutionFailed(format!("Claude query failed: {}", e))
                })?;

        // Collect results
        let mut summary = String::new();
        let modified_files = Vec::new();
        let mut success = false;

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(SdkMessage::Result(result)) => match result {
                    SdkResultMessage::Success(s) => {
                        success = true;
                        summary = s.result;

                        tracing::info!(
                            "Claude completed successfully: {} turns, ${:.4} cost, {} in / {} out tokens",
                            s.num_turns,
                            s.total_cost_usd,
                            s.usage.input_tokens,
                            s.usage.output_tokens
                        );
                    }
                    SdkResultMessage::ErrorDuringExecution(e) => {
                        let error_msg = e.errors.join("; ");
                        tracing::error!("Claude error during execution: {}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Claude error: {}", error_msg)),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxTurns(e) => {
                        let error_msg = format!(
                            "Max turns ({}) exceeded: {}",
                            e.num_turns,
                            e.errors.join("; ")
                        );
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxBudget(e) => {
                        let error_msg = format!(
                            "Max budget (${:.2}) exceeded: {}",
                            e.total_cost_usd,
                            e.errors.join("; ")
                        );
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => {
                        let error_msg =
                            format!("Structured output retries exceeded: {}", e.errors.join("; "));
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                },
                Ok(SdkMessage::Assistant(msg)) => {
                    // Track progress
                    tracing::debug!("Claude assistant message: {:?}", msg.message);
                }
                Ok(SdkMessage::System(sys)) => {
                    tracing::debug!("Claude system message: {:?}", sys);
                }
                Err(e) => {
                    tracing::warn!("Stream error: {}", e);
                }
                _ => {}
            }
        }

        Ok(TaskResult {
            success,
            summary,
            modified_files,
            commit_hash: None,
            error: None,
            session_id: None,
        })
    }

    /// Execute a task with RLM (Recursive Language Model) support.
    ///
    /// This method configures Claude with:
    /// - An RLM custom agent for deep recursive analysis
    /// - The RLM MCP server providing `rlm_query` and `rlm_fanout` tools
    /// - Higher max turns to allow iterative analysis
    ///
    /// Use this for complex tasks that benefit from:
    /// - Iterative code execution to verify hypotheses
    /// - Large document analysis requiring chunking
    /// - Multi-step reasoning with verification
    ///
    /// # Arguments
    /// * `task` - The task to execute
    /// * `context` - Additional context to include in the prompt
    /// * `enable_rlm_tools` - Whether to enable the RLM MCP server tools
    pub async fn execute_with_rlm(
        &self,
        task: &Task,
        context: &str,
        enable_rlm_tools: bool,
    ) -> Result<TaskResult, AdjutantError> {
        tracing::info!(
            "ClaudeExecutor: Using Claude with RLM support for task '{}'",
            task.title
        );

        // Configure query options with RLM support
        let mut options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .tools(ToolsConfig::claude_code_preset())
            .max_turns(30) // More turns for RLM-style iteration
            .include_partial_messages(true);

        // Add RLM custom agent
        options = options.agent("rlm-analyzer", rlm_agent_definition());

        // Add RLM MCP server if tools enabled
        if enable_rlm_tools {
            options = options.mcp_server(
                "rlm",
                McpServerConfig::Stdio {
                    command: "rlm-mcp-server".to_string(),
                    args: None,
                    env: None,
                },
            );
        }

        // Build prompt with context
        let prompt = if context.is_empty() {
            task.to_prompt()
        } else {
            format!(
                "{}\n\n## Additional Context\n\n{}",
                task.to_prompt(),
                context
            )
        };

        // Run query
        let mut stream = query(&prompt, options)
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Claude query failed: {}", e)))?;

        // Collect results (same as execute())
        let mut summary = String::new();
        let modified_files = Vec::new();
        let mut success = false;

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(SdkMessage::Result(result)) => match result {
                    SdkResultMessage::Success(s) => {
                        success = true;
                        summary = s.result;

                        tracing::info!(
                            "Claude RLM completed: {} turns, ${:.4} cost, {} in / {} out tokens",
                            s.num_turns,
                            s.total_cost_usd,
                            s.usage.input_tokens,
                            s.usage.output_tokens
                        );
                    }
                    SdkResultMessage::ErrorDuringExecution(e) => {
                        let error_msg = e.errors.join("; ");
                        tracing::error!("Claude RLM error: {}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Claude RLM error: {}", error_msg)),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxTurns(e) => {
                        let error_msg = format!(
                            "RLM max turns ({}) exceeded: {}",
                            e.num_turns,
                            e.errors.join("; ")
                        );
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxBudget(e) => {
                        let error_msg = format!(
                            "RLM budget (${:.2}) exceeded: {}",
                            e.total_cost_usd,
                            e.errors.join("; ")
                        );
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => {
                        let error_msg =
                            format!("RLM structured output retries exceeded: {}", e.errors.join("; "));
                        tracing::error!("{}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: String::new(),
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(error_msg),
                            session_id: None,
                        });
                    }
                },
                Ok(SdkMessage::Assistant(msg)) => {
                    tracing::debug!("Claude RLM assistant: {:?}", msg.message);
                }
                Ok(SdkMessage::System(sys)) => {
                    tracing::debug!("Claude RLM system: {:?}", sys);
                }
                Err(e) => {
                    tracing::warn!("Stream error: {}", e);
                }
                _ => {}
            }
        }

        Ok(TaskResult {
            success,
            summary,
            modified_files,
            commit_hash: None,
            error: None,
            session_id: None,
        })
    }
}
