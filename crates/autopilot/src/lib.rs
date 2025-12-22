//! Autopilot - Autonomous task runner with trajectory logging
//!
//! This crate provides a CLI tool that executes tasks via the Claude Agent SDK
//! and logs the complete trajectory in both rlog and JSON formats.

use std::path::PathBuf;

pub mod analyze;
pub mod auto_issues;
pub mod benchmark;
pub mod compaction;
pub mod daemon;
pub mod dashboard;
pub mod goals;
pub mod guardrails;
pub mod learning;
pub mod logs;
pub mod metrics;
pub mod nip_sa_trajectory;
pub mod nostr_agent;
pub mod planmode;
pub mod redact;
pub mod replay;
pub mod rlog;
pub mod state;
pub mod timestamp;
pub mod tool_patterns;
pub mod trajectory;
pub mod trajectory_publisher;
pub mod ui_renderer;

#[cfg(test)]
mod tests;

use chrono::Utc;
use claude_agent_sdk::{
    SdkAssistantMessage, SdkMessage, SdkResultMessage, SdkSystemMessage, SdkUserMessage,
};
use serde_json::Value;
use trajectory::{StepType, TokenUsage, Trajectory, TrajectoryResult};
use rlog::RlogWriter;

/// Callback invoked when session_id becomes available
pub type SessionIdCallback = Box<dyn FnOnce(&str) + Send>;

/// Collects SdkMessages into a Trajectory
pub struct TrajectoryCollector {
    trajectory: Trajectory,
    /// Optional rlog writer for streaming output
    rlog_writer: Option<RlogWriter>,
    /// Path to the rlog file for header updates
    rlog_path: Option<std::path::PathBuf>,
    /// Callback invoked when session_id is set
    session_id_callback: Option<SessionIdCallback>,
}

impl TrajectoryCollector {
    /// Create a new collector
    pub fn new(
        prompt: String,
        model: String,
        cwd: String,
        repo_sha: String,
        branch: Option<String>,
    ) -> Self {
        Self {
            trajectory: Trajectory::new(prompt, model, cwd, repo_sha, branch),
            rlog_writer: None,
            rlog_path: None,
            session_id_callback: None,
        }
    }

    /// Set a callback to be invoked when session_id becomes available
    pub fn on_session_id<F>(&mut self, callback: F)
    where
        F: FnOnce(&str) + Send + 'static,
    {
        self.session_id_callback = Some(Box::new(callback));
    }

    /// Enable streaming rlog output to a file
    pub fn enable_streaming(&mut self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        let mut writer = RlogWriter::new_streaming(path)?;
        writer.write_header(&self.trajectory)?;
        self.rlog_writer = Some(writer);
        self.rlog_path = Some(path.to_path_buf());
        Ok(())
    }

    /// Set the session_id (useful when resuming a session)
    pub fn set_session_id(&mut self, session_id: String) {
        self.trajectory.session_id = session_id;
    }

    /// Stream the last added step to rlog file (if streaming is enabled)
    fn stream_last_step(&mut self) {
        if let (Some(writer), Some(last_step)) = (&mut self.rlog_writer, self.trajectory.steps.last()) {
            let _ = writer.append_step(last_step);
        }
    }

    /// Process an SdkMessage and add to trajectory
    pub fn process_message(&mut self, msg: &SdkMessage) {
        match msg {
            SdkMessage::System(sys) => self.process_system(sys),
            SdkMessage::Assistant(asst) => self.process_assistant(asst),
            SdkMessage::User(user) => self.process_user(user),
            SdkMessage::Result(result) => self.process_result(result),
            SdkMessage::ToolProgress(_) => { /* Could track progress */ }
            SdkMessage::StreamEvent(_) => { /* Could track streaming */ }
            SdkMessage::AuthStatus(_) => { /* Could track auth */ }
        }
    }

    fn process_system(&mut self, sys: &SdkSystemMessage) {
        match sys {
            SdkSystemMessage::Init(init) => {
                self.trajectory.session_id = init.session_id.clone();
                self.trajectory.add_step(StepType::SystemInit {
                    model: init.model.clone(),
                });

                // Update the header now that we have the session_id
                if let (Some(writer), Some(path)) = (&mut self.rlog_writer, &self.rlog_path) {
                    let _ = writer.update_header(path, &self.trajectory);
                }

                // Invoke session_id callback if set
                if let Some(callback) = self.session_id_callback.take() {
                    callback(&init.session_id);
                }

                self.stream_last_step();
            }
            SdkSystemMessage::Status(status) => {
                if let Some(s) = &status.status {
                    self.trajectory.add_step(StepType::SystemStatus {
                        status: format!("{:?}", s),
                    });
                    self.stream_last_step();
                }
            }
            _ => {}
        }
    }

    fn process_assistant(&mut self, asst: &SdkAssistantMessage) {
        // Extract token info before adding steps (to avoid borrow issues)
        let (tokens_in, tokens_out, tokens_cached) = Self::extract_tokens(&asst.message);

        // Parse the message content which contains content blocks
        if let Some(content) = asst.message.get("content").and_then(|c| c.as_array()) {
            for block in content {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match block_type {
                    "thinking" => {
                        let text = block
                            .get("thinking")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        let sig = block
                            .get("signature")
                            .and_then(|s| s.as_str())
                            .map(String::from);
                        let step = self.trajectory.add_step(StepType::Thinking {
                            content: text.to_string(),
                            signature: sig,
                        });
                        step.tokens_in = tokens_in;
                        step.tokens_out = tokens_out;
                        step.tokens_cached = tokens_cached;
                        self.stream_last_step();
                    }
                    "text" => {
                        let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let step = self.trajectory.add_step(StepType::Assistant {
                            content: text.to_string(),
                        });
                        step.tokens_in = tokens_in;
                        step.tokens_out = tokens_out;
                        step.tokens_cached = tokens_cached;
                        self.stream_last_step();
                    }
                    "tool_use" => {
                        let tool_name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        let tool_id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let input = block.get("input").cloned().unwrap_or(Value::Null);

                        // Track issue_complete calls
                        if tool_name == "mcp__issues__issue_complete" {
                            if let Some(result) = &mut self.trajectory.result {
                                result.issues_completed += 1;
                            }
                        }

                        let step = self.trajectory.add_step(StepType::ToolCall {
                            tool: tool_name.to_string(),
                            tool_id: tool_id.to_string(),
                            input,
                        });
                        step.tokens_in = tokens_in;
                        step.tokens_out = tokens_out;
                        step.tokens_cached = tokens_cached;
                        self.stream_last_step();
                    }
                    _ => {}
                }
            }
        }

        // Update token usage from message usage field
        if let Some(inp) = tokens_in {
            self.trajectory.usage.input_tokens += inp;
        }
        if let Some(out) = tokens_out {
            self.trajectory.usage.output_tokens += out;
        }
        if let Some(cached) = tokens_cached {
            self.trajectory.usage.cache_read_tokens += cached;
        }
    }

    fn extract_tokens(message: &Value) -> (Option<u64>, Option<u64>, Option<u64>) {
        if let Some(usage) = message.get("usage") {
            (
                usage.get("input_tokens").and_then(|t| t.as_u64()),
                usage.get("output_tokens").and_then(|t| t.as_u64()),
                usage.get("cache_read_input_tokens").and_then(|t| t.as_u64()),
            )
        } else {
            (None, None, None)
        }
    }

    fn process_user(&mut self, user: &SdkUserMessage) {
        // Check if this is a tool result
        if let Some(content) = user.message.get("content") {
            match content {
                Value::Array(arr) => {
                    // Handle tool_result blocks
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                            let tool_id = block
                                .get("tool_use_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("");
                            let is_error = block
                                .get("is_error")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(false);
                            let output = self.extract_tool_result_content(block);

                            self.trajectory.add_step(StepType::ToolResult {
                                tool_id: tool_id.to_string(),
                                success: !is_error,
                                output: if output.is_empty() {
                                    None
                                } else {
                                    Some(output)
                                },
                            });
                            self.stream_last_step();
                        }
                    }
                }
                Value::String(s) => {
                    // Plain user message
                    self.trajectory.add_step(StepType::User {
                        content: s.clone(),
                    });
                    self.stream_last_step();
                }
                _ => {}
            }
        }
    }

    fn process_result(&mut self, result: &SdkResultMessage) {
        self.trajectory.ended_at = Some(Utc::now());

        match result {
            SdkResultMessage::Success(success) => {
                self.trajectory.usage = TokenUsage {
                    input_tokens: success.usage.input_tokens,
                    output_tokens: success.usage.output_tokens,
                    cache_read_tokens: success.usage.cache_read_input_tokens.unwrap_or(0),
                    cache_creation_tokens: success.usage.cache_creation_input_tokens.unwrap_or(0),
                    cost_usd: success.total_cost_usd,
                };
                self.trajectory.result = Some(TrajectoryResult {
                    success: !success.is_error,
                    duration_ms: success.duration_ms,
                    num_turns: success.num_turns,
                    result_text: Some(success.result.clone()),
                    errors: Vec::new(),
                    issues_completed: 0,
                });
            }
            SdkResultMessage::ErrorDuringExecution(err)
            | SdkResultMessage::ErrorMaxTurns(err)
            | SdkResultMessage::ErrorMaxBudget(err)
            | SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                self.trajectory.usage = TokenUsage {
                    input_tokens: err.usage.input_tokens,
                    output_tokens: err.usage.output_tokens,
                    cache_read_tokens: err.usage.cache_read_input_tokens.unwrap_or(0),
                    cache_creation_tokens: err.usage.cache_creation_input_tokens.unwrap_or(0),
                    cost_usd: err.total_cost_usd,
                };
                self.trajectory.result = Some(TrajectoryResult {
                    success: false,
                    duration_ms: err.duration_ms,
                    num_turns: err.num_turns,
                    result_text: None,
                    errors: err.errors.clone(),
                    issues_completed: 0,
                });
            }
        }
    }

    fn extract_tool_result_content(&self, block: &Value) -> String {
        match block.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }

    /// Process a Codex ThreadEvent and add to trajectory
    pub fn process_codex_event(&mut self, event: &codex_agent_sdk::ThreadEvent) {
        use codex_agent_sdk::{ThreadEvent, ThreadItemDetails};

        match event {
            ThreadEvent::ThreadStarted(e) => {
                // Store thread ID as session_id
                self.trajectory.session_id = e.thread_id.clone();

                // Update the header now that we have the thread_id
                if let (Some(writer), Some(path)) = (&mut self.rlog_writer, &self.rlog_path) {
                    let _ = writer.update_header(path, &self.trajectory);
                }
            }
            ThreadEvent::ItemCompleted(item_event) => {
                match &item_event.item.details {
                    ThreadItemDetails::AgentMessage(msg) => {
                        self.trajectory.add_step(StepType::Assistant {
                            content: msg.text.clone(),
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::Reasoning(reasoning) => {
                        self.trajectory.add_step(StepType::Thinking {
                            content: reasoning.text.clone(),
                            signature: None,
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::CommandExecution(cmd) => {
                        self.trajectory.add_step(StepType::ToolCall {
                            tool: "Bash".to_string(),
                            tool_id: item_event.item.id.clone(),
                            input: serde_json::json!({ "command": cmd.command }),
                        });
                        self.stream_last_step();

                        // Add tool result
                        let success = matches!(cmd.status, codex_agent_sdk::CommandExecutionStatus::Completed);
                        self.trajectory.add_step(StepType::ToolResult {
                            tool_id: item_event.item.id.clone(),
                            success,
                            output: if cmd.aggregated_output.is_empty() {
                                None
                            } else {
                                Some(cmd.aggregated_output.clone())
                            },
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::FileChange(fc) => {
                        let changes_json = serde_json::to_value(&fc.changes).unwrap_or(serde_json::json!([]));
                        self.trajectory.add_step(StepType::ToolCall {
                            tool: "Edit".to_string(),
                            tool_id: item_event.item.id.clone(),
                            input: serde_json::json!({ "changes": changes_json }),
                        });
                        self.stream_last_step();

                        // Add tool result
                        let success = matches!(fc.status, codex_agent_sdk::PatchApplyStatus::Completed);
                        self.trajectory.add_step(StepType::ToolResult {
                            tool_id: item_event.item.id.clone(),
                            success,
                            output: Some(format!("{} file(s) changed", fc.changes.len())),
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::McpToolCall(mcp) => {
                        self.trajectory.add_step(StepType::ToolCall {
                            tool: format!("{}:{}", mcp.server, mcp.tool),
                            tool_id: item_event.item.id.clone(),
                            input: mcp.arguments.clone(),
                        });
                        self.stream_last_step();

                        // Add tool result
                        let success = matches!(mcp.status, codex_agent_sdk::McpToolCallStatus::Completed);
                        let output = mcp.result.as_ref().map(|r| {
                            serde_json::to_string_pretty(r).unwrap_or_else(|_| format!("{:?}", r))
                        });
                        self.trajectory.add_step(StepType::ToolResult {
                            tool_id: item_event.item.id.clone(),
                            success,
                            output,
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::WebSearch(ws) => {
                        self.trajectory.add_step(StepType::ToolCall {
                            tool: "WebSearch".to_string(),
                            tool_id: item_event.item.id.clone(),
                            input: serde_json::json!({ "query": ws.query }),
                        });
                        self.stream_last_step();

                        // Add result
                        self.trajectory.add_step(StepType::ToolResult {
                            tool_id: item_event.item.id.clone(),
                            success: true,
                            output: Some(format!("Search: {}", ws.query)),
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::TodoList(todo) => {
                        let items_json = serde_json::to_value(&todo.items).unwrap_or(serde_json::json!([]));
                        self.trajectory.add_step(StepType::ToolCall {
                            tool: "TodoWrite".to_string(),
                            tool_id: item_event.item.id.clone(),
                            input: serde_json::json!({ "todos": items_json }),
                        });
                        self.stream_last_step();

                        self.trajectory.add_step(StepType::ToolResult {
                            tool_id: item_event.item.id.clone(),
                            success: true,
                            output: Some(format!("{} todo items", todo.items.len())),
                        });
                        self.stream_last_step();
                    }
                    ThreadItemDetails::Error(err) => {
                        self.trajectory.add_step(StepType::SystemStatus {
                            status: format!("Error: {}", err.message),
                        });
                        self.stream_last_step();
                    }
                }
            }
            ThreadEvent::TurnCompleted(tc) => {
                // Update token usage
                self.trajectory.usage.input_tokens += tc.usage.input_tokens as u64;
                self.trajectory.usage.output_tokens += tc.usage.output_tokens as u64;
                self.trajectory.usage.cache_read_tokens += tc.usage.cached_input_tokens as u64;

                // Add token info to the last step if available
                if let Some(step) = self.trajectory.steps.last_mut() {
                    step.tokens_in = Some(tc.usage.input_tokens as u64);
                    step.tokens_out = Some(tc.usage.output_tokens as u64);
                    step.tokens_cached = Some(tc.usage.cached_input_tokens as u64);
                }
            }
            ThreadEvent::TurnFailed(tf) => {
                self.trajectory.ended_at = Some(Utc::now());
                self.trajectory.add_step(StepType::SystemStatus {
                    status: format!("Turn failed: {}", tf.error.message),
                });
                self.stream_last_step();
            }
            ThreadEvent::Error(e) => {
                self.trajectory.add_step(StepType::SystemStatus {
                    status: format!("Error: {}", e.message),
                });
                self.stream_last_step();
            }
            _ => {
                // Ignore other events (TurnStarted, ItemStarted, ItemUpdated)
            }
        }
    }

    /// Finish collecting and return the trajectory
    pub fn finish(mut self) -> Trajectory {
        // Write footer to rlog if streaming is enabled
        if let Some(writer) = &mut self.rlog_writer {
            let _ = writer.write_footer(&self.trajectory);
            let _ = writer.close();
        }
        self.trajectory
    }

    /// Get a reference to the current trajectory
    pub fn trajectory(&self) -> &Trajectory {
        &self.trajectory
    }

    /// Consume the collector and return the trajectory (for testing)
    pub fn into_trajectory(self) -> Trajectory {
        self.trajectory
    }
}

/// Extract session_id from a trajectory JSON file.
pub fn extract_session_id_from_json(path: &std::path::Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)?;
    let traj: trajectory::Trajectory = serde_json::from_str(&content)?;
    if traj.session_id.is_empty() {
        anyhow::bail!("No session_id in trajectory file");
    }
    Ok(traj.session_id)
}

/// Extract session_id from rlog YAML header.
/// Returns None if the id field is empty or missing.
pub fn extract_session_id_from_rlog(path: &std::path::Path) -> anyhow::Result<Option<String>> {
    let content = std::fs::read_to_string(path)?;

    // Find YAML header between --- markers
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 || lines[0] != "---" {
        return Ok(None);
    }

    // Find end marker
    let end_idx = lines.iter().skip(1).position(|l| *l == "---");
    let end_idx = match end_idx {
        Some(idx) => idx + 1, // +1 because we skipped first line
        None => return Ok(None),
    };

    // Parse header lines looking for id:
    for line in &lines[1..end_idx] {
        if let Some(rest) = line.strip_prefix("id:") {
            let id = rest.trim();
            if !id.is_empty() {
                return Ok(Some(id.to_string()));
            }
        }
    }

    Ok(None)
}

/// Find the workspace root directory by looking for Cargo.toml with `[workspace]`.
///
/// Starts from the current directory and walks up until it finds a Cargo.toml
/// containing `[workspace]`. This ensures autopilot.db is always created at
/// the workspace root, not in subdirectories.
///
/// Returns the current directory if no workspace root is found (fallback).
pub fn find_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut current = cwd.as_path();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return current.to_path_buf();
                }
            }
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    // Fallback to current directory if no workspace found
    cwd
}

/// Get the default path for autopilot.db at the workspace root.
///
/// This ensures the database is always created in a consistent location
/// regardless of which subdirectory the command is run from.
pub fn default_db_path() -> PathBuf {
    find_workspace_root().join("autopilot.db")
}
