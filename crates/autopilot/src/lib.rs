//! Autopilot - Autonomous task runner with trajectory logging
//!
//! This crate provides a CLI tool that executes tasks via the Claude Agent SDK
//! and logs the complete trajectory in both rlog and JSON formats.

pub mod replay;
pub mod rlog;
pub mod timestamp;
pub mod trajectory;

use chrono::Utc;
use claude_agent_sdk::{
    SdkAssistantMessage, SdkMessage, SdkResultMessage, SdkSystemMessage, SdkUserMessage,
};
use serde_json::Value;
use trajectory::{StepType, TokenUsage, Trajectory, TrajectoryResult};
use rlog::RlogWriter;

/// Collects SdkMessages into a Trajectory
pub struct TrajectoryCollector {
    trajectory: Trajectory,
    /// Optional rlog writer for streaming output
    rlog_writer: Option<RlogWriter>,
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
        }
    }

    /// Enable streaming rlog output to a file
    pub fn enable_streaming(&mut self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let mut writer = RlogWriter::new_streaming(path)?;
        writer.write_header(&self.trajectory)?;
        self.rlog_writer = Some(writer);
        Ok(())
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
}
