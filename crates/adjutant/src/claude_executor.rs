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

use crate::autopilot_loop::AcpEventSender;
use crate::rlm_agent::rlm_agent_definition;
use crate::{AdjutantError, Task, TaskResult, ToolRegistry};
use claude_agent_sdk::{
    query, McpServerConfig, QueryOptions, SdkMessage, SdkResultMessage, SdkStreamEvent, ToolsConfig,
};
use futures::StreamExt;
use std::path::Path;
use tokio::sync::mpsc;

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

    /// Execute a task with streaming output.
    ///
    /// This method streams tokens to the UI in real-time as they arrive from Claude,
    /// rather than waiting for the complete response.
    pub async fn execute_streaming(
        &self,
        task: &Task,
        token_tx: mpsc::UnboundedSender<String>,
        _acp_sender: Option<AcpEventSender>,
    ) -> Result<TaskResult, AdjutantError> {
        tracing::info!(
            "ClaudeExecutor: Streaming Claude Pro/Max for task '{}'",
            task.title
        );

        // Build prompt from task
        let prompt = task.to_prompt();

        // Configure query options
        let options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .tools(ToolsConfig::claude_code_preset())
            .max_turns(20)
            .include_partial_messages(true); // Enable streaming events

        // Run query
        let mut stream = query(&prompt, options)
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Claude query failed: {}", e)))?;

        // Track results
        let mut summary = String::new();
        let modified_files = Vec::new();
        let mut success = false;
        let mut full_response = String::new();

        tracing::debug!("Starting Claude SDK stream processing");
        let mut chunk_count = 0u64;

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                // Stream events - extract and send text deltas immediately
                Ok(SdkMessage::StreamEvent(event)) => {
                    if let Some(text) = extract_stream_text(&event) {
                        // Filter out XML tool use tags from display
                        let filtered = filter_tool_xml(&text);
                        if !filtered.is_empty() {
                            chunk_count += 1;
                            tracing::debug!(chunk = chunk_count, len = filtered.len(), "Stream chunk");
                            let _ = token_tx.send(filtered);
                        }
                        full_response.push_str(&text);
                    }
                }
                // Assistant messages - also stream their content
                Ok(SdkMessage::Assistant(msg)) => {
                    if let Some(text) = extract_assistant_text(&msg.message) {
                        tracing::debug!(len = text.len(), "Assistant message chunk");
                        // Don't double-send if we already got it via StreamEvent
                        // Only send if it's new content
                        if !full_response.ends_with(&text) {
                            // Filter out XML tool use tags from display
                            let filtered = filter_tool_xml(&text);
                            if !filtered.is_empty() {
                                let _ = token_tx.send(filtered);
                            }
                            full_response.push_str(&text);
                        }
                    }
                }
                // Final result - capture success/failure
                Ok(SdkMessage::Result(result)) => match result {
                    SdkResultMessage::Success(s) => {
                        success = true;
                        summary = s.result.clone();
                        tracing::info!(
                            "Claude streaming completed: {} turns, ${:.4} cost",
                            s.num_turns,
                            s.total_cost_usd
                        );
                    }
                    SdkResultMessage::ErrorDuringExecution(e) => {
                        let error_msg = e.errors.join("; ");
                        tracing::error!("Claude error during streaming: {}", error_msg);
                        return Ok(TaskResult {
                            success: false,
                            summary: full_response,
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Claude error: {}", error_msg)),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxTurns(e) => {
                        return Ok(TaskResult {
                            success: false,
                            summary: full_response,
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Max turns ({}) exceeded", e.num_turns)),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxBudget(e) => {
                        return Ok(TaskResult {
                            success: false,
                            summary: full_response,
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Max budget (${:.2}) exceeded", e.total_cost_usd)),
                            session_id: None,
                        });
                    }
                    SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => {
                        return Ok(TaskResult {
                            success: false,
                            summary: full_response,
                            modified_files: Vec::new(),
                            commit_hash: None,
                            error: Some(format!("Structured output retries exceeded: {}", e.errors.join("; "))),
                            session_id: None,
                        });
                    }
                },
                // Tool progress - log for now
                Ok(SdkMessage::ToolProgress(prog)) => {
                    tracing::debug!(tool = %prog.tool_name, elapsed = prog.elapsed_time_seconds, "Tool progress");
                }
                // System messages - log
                Ok(SdkMessage::System(sys)) => {
                    tracing::debug!("System message: {:?}", sys);
                }
                // Auth status - log
                Ok(SdkMessage::AuthStatus(auth)) => {
                    tracing::debug!("Auth status: authenticating={}", auth.is_authenticating);
                }
                // User messages - echo, ignore
                Ok(SdkMessage::User(_)) => {}
                // Errors
                Err(e) => {
                    tracing::warn!("Stream error: {}", e);
                }
            }
        }

        Ok(TaskResult {
            success,
            summary: if summary.is_empty() { full_response } else { summary },
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

/// Extract text from a Claude stream event.
///
/// Stream events contain SSE-style deltas. The text is typically in:
/// - `event.delta.text` for content block deltas
/// - `event.text` for direct text events
fn extract_stream_text(event: &SdkStreamEvent) -> Option<String> {
    // Try content_block_delta format: event.delta.text
    if let Some(delta) = event.event.get("delta") {
        if let Some(text) = delta.get("text") {
            if let Some(s) = text.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }

    // Try direct text field
    if let Some(text) = event.event.get("text") {
        if let Some(s) = text.as_str() {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }

    // Try content array format (for full messages)
    if let Some(content) = event.event.get("content") {
        if let Some(arr) = content.as_array() {
            let mut combined = String::new();
            for item in arr {
                if let Some(text) = item.get("text") {
                    if let Some(s) = text.as_str() {
                        combined.push_str(s);
                    }
                }
            }
            if !combined.is_empty() {
                return Some(combined);
            }
        }
    }

    None
}

/// Extract text content from an assistant message.
///
/// The message is typically an API response with content blocks.
fn extract_assistant_text(message: &serde_json::Value) -> Option<String> {
    // Try content array
    if let Some(content) = message.get("content") {
        if let Some(arr) = content.as_array() {
            let mut combined = String::new();
            for item in arr {
                // Check for text content blocks
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text") {
                        if let Some(s) = text.as_str() {
                            combined.push_str(s);
                        }
                    }
                }
            }
            if !combined.is_empty() {
                return Some(combined);
            }
        }
    }

    // Try direct text field
    if let Some(text) = message.get("text") {
        if let Some(s) = text.as_str() {
            return Some(s.to_string());
        }
    }

    // Try message as string directly
    if let Some(s) = message.as_str() {
        return Some(s.to_string());
    }

    None
}

/// Filter out XML tool use tags and DSPy format markers from display text.
///
/// Claude outputs tool calls in XML format like `<function_calls>...</function_calls>`.
/// DSPy outputs format markers like `[[ ## fieldname ## ]]`.
/// These should not be shown to the user in the streaming output.
fn filter_tool_xml(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TOOL_XML_RE: OnceLock<Regex> = OnceLock::new();
    static DSPY_MARKER_RE: OnceLock<Regex> = OnceLock::new();

    let xml_re = TOOL_XML_RE.get_or_init(|| {
        // Match various XML-like tool patterns
        Regex::new(r"(?s)<(function_calls|antml:function_calls|antml:invoke|antml:parameter)[^>]*>.*?</\1>|<(function_calls|antml:function_calls|antml:invoke|antml:parameter)[^>]*/?>|</(function_calls|antml:function_calls|antml:invoke|antml:parameter)>").unwrap()
    });

    let dspy_re = DSPY_MARKER_RE.get_or_init(|| {
        // Match DSPy format markers like [[ ## fieldname ## ]]
        Regex::new(r"\[\[\s*##\s*[^#]+\s*##\s*\]\]").unwrap()
    });

    let filtered = xml_re.replace_all(text, "");
    let filtered = dspy_re.replace_all(&filtered, "");

    // Also trim any resulting whitespace-only strings
    let result = filtered.trim();
    if result.is_empty() {
        String::new()
    } else {
        result.to_string()
    }
}
