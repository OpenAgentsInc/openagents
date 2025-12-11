//! Agent loop implementation
//!
//! The core agentic loop that:
//! - Sends messages to an LLM
//! - Executes tool calls
//! - Tracks verification state
//! - Emits events for monitoring

use crate::error::AgentError;
use llm::{ChatOptions, ContentPart, LlmProvider, Message};
use serde::{Deserialize, Serialize};

// ============================================================================
// Event Types
// ============================================================================

/// Events emitted during the agent loop
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopEvent {
    TurnStart {
        turn: u32,
    },
    LlmRequest {
        turn: u32,
        message_count: usize,
        tool_names: Vec<String>,
    },
    LlmResponse {
        turn: u32,
        has_tool_calls: bool,
        tool_call_count: usize,
    },
    ToolCall {
        tool: String,
        tool_call_id: String,
        args: String,
    },
    ToolResult {
        tool: String,
        tool_call_id: String,
        ok: bool,
        output: String,
    },
    ToolOutput {
        tool: String,
        tool_call_id: String,
        chunk: String,
    },
    EditDetected {
        tool: String,
    },
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the agent loop
#[derive(Debug, Clone, Default)]
pub struct LoopConfig {
    /// Model to use for LLM calls
    pub model: Option<String>,
    /// System prompt
    pub system_prompt: Option<String>,
    /// Maximum number of turns before stopping
    pub max_turns: Option<u32>,
    /// Temperature for LLM calls
    pub temperature: Option<f32>,
}

impl LoopConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_max_turns(mut self, max: u32) -> Self {
        self.max_turns = Some(max);
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

// ============================================================================
// Turn and Result Types
// ============================================================================

/// A single turn in the agent loop
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopTurn {
    pub role: LoopTurnRole,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LoopToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<LoopToolResult>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopTurnRole {
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl LoopToolCall {
    /// Create from a ContentPart::ToolUse
    pub fn from_content_part(part: &ContentPart) -> Option<Self> {
        match part {
            ContentPart::ToolUse { id, name, input } => Some(Self {
                id: id.clone(),
                name: name.clone(),
                arguments: serde_json::to_string(input).unwrap_or_default(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub output: String,
    pub is_error: bool,
}

/// Verification state tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyState {
    /// Any edit/write since last successful typecheck+tests
    pub dirty_since_verify: bool,
    /// Whether typecheck passed
    pub typecheck_ok: bool,
    /// Whether tests passed
    pub tests_ok: bool,
}

/// Result of running the agent loop
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopResult {
    pub turns: Vec<LoopTurn>,
    pub final_message: Option<String>,
    pub total_turns: u32,
    pub verify_state: VerifyState,
}

// ============================================================================
// Tool Executor Trait
// ============================================================================

/// Trait for executing tools
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Get the list of available tool names
    fn tool_names(&self) -> Vec<String>;

    /// Execute a tool call
    async fn execute(
        &self,
        tool_call: &LoopToolCall,
        on_output: Option<&(dyn Fn(String) + Send + Sync)>,
    ) -> LoopToolResult;
}

// ============================================================================
// Agent Loop
// ============================================================================

/// Run the agent loop
pub async fn agent_loop<P: LlmProvider, T: ToolExecutor>(
    user_message: &str,
    provider: &P,
    tool_executor: &T,
    config: &LoopConfig,
    mut on_event: Option<impl FnMut(LoopEvent)>,
) -> Result<LoopResult, AgentError> {
    let max_turns = config.max_turns.unwrap_or(10);
    let mut turns: Vec<LoopTurn> = Vec::new();
    let mut messages: Vec<Message> = Vec::new();

    // Verification state tracking
    let mut verify_state = VerifyState::default();

    // Add system prompt if provided
    if let Some(ref system_prompt) = config.system_prompt {
        messages.push(Message::system(system_prompt));
    }

    // Add user message
    messages.push(Message::user(user_message));

    let mut turn_count = 0u32;
    let mut continue_loop = true;

    while continue_loop && turn_count < max_turns {
        turn_count += 1;

        // Emit turn_start
        if let Some(ref mut emit) = on_event {
            emit(LoopEvent::TurnStart { turn: turn_count });
        }

        // Build chat options
        let mut options = ChatOptions::default();
        if let Some(ref model) = config.model {
            options.model = Some(model.clone());
        }
        if let Some(temp) = config.temperature {
            options.temperature = Some(temp);
        }

        // Emit llm_request
        if let Some(ref mut emit) = on_event {
            emit(LoopEvent::LlmRequest {
                turn: turn_count,
                message_count: messages.len(),
                tool_names: tool_executor.tool_names(),
            });
        }

        // Call LLM
        let response = provider
            .chat(&messages, Some(options))
            .await
            .map_err(AgentError::Llm)?;

        let tool_uses = response.tool_uses();
        let has_tool_calls = response.has_tool_use();

        // Emit llm_response
        if let Some(ref mut emit) = on_event {
            emit(LoopEvent::LlmResponse {
                turn: turn_count,
                has_tool_calls,
                tool_call_count: tool_uses.len(),
            });
        }

        // Get text content
        let assistant_content = response.text();

        // Add assistant message
        messages.push(Message::assistant(&assistant_content));

        // Build turn
        let tool_calls: Vec<LoopToolCall> = tool_uses
            .iter()
            .filter_map(|part| LoopToolCall::from_content_part(part))
            .collect();

        let mut turn = LoopTurn {
            role: LoopTurnRole::Assistant,
            content: if assistant_content.is_empty() {
                None
            } else {
                Some(assistant_content.clone())
            },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls.clone())
            },
            tool_results: None,
        };

        if !has_tool_calls {
            // No tool calls, end loop
            turns.push(turn);
            continue_loop = false;
        } else {
            // Execute tool calls
            let mut tool_results: Vec<LoopToolResult> = Vec::new();

            for tool_call in &tool_calls {
                // Emit tool_call
                if let Some(ref mut emit) = on_event {
                    emit(LoopEvent::ToolCall {
                        tool: tool_call.name.clone(),
                        tool_call_id: tool_call.id.clone(),
                        args: tool_call.arguments.clone(),
                    });
                }

                // Execute tool
                let result = tool_executor.execute(tool_call, None).await;

                // Emit tool_result
                if let Some(ref mut emit) = on_event {
                    emit(LoopEvent::ToolResult {
                        tool: result.name.clone(),
                        tool_call_id: result.tool_call_id.clone(),
                        ok: !result.is_error,
                        output: result.output.clone(),
                    });
                }

                // Add tool result message
                messages.push(Message::tool_result(&tool_call.id, &result.output));

                // Track verification state
                let tool_name = tool_call.name.to_lowercase();

                // Edit/write marks as dirty
                if (tool_name == "edit" || tool_name == "write") && !result.is_error {
                    verify_state.dirty_since_verify = true;
                    verify_state.typecheck_ok = false;
                    verify_state.tests_ok = false;

                    if let Some(ref mut emit) = on_event {
                        emit(LoopEvent::EditDetected {
                            tool: tool_name.clone(),
                        });
                    }
                }

                // Check for typecheck/test commands
                if tool_name == "bash" && !result.is_error {
                    if is_typecheck_command(&tool_call.arguments) {
                        if is_typecheck_success(&result.output) {
                            verify_state.typecheck_ok = true;
                        }
                    }
                    if is_test_command(&tool_call.arguments) {
                        if is_test_success(&result.output) {
                            verify_state.tests_ok = true;
                        }
                    }
                    // If both pass after edits, we're no longer dirty
                    if verify_state.typecheck_ok && verify_state.tests_ok {
                        verify_state.dirty_since_verify = false;
                    }
                }

                tool_results.push(result);
            }

            turn.tool_results = Some(tool_results);
            turns.push(turn);
        }
    }

    if turn_count >= max_turns && continue_loop {
        return Err(AgentError::Timeout(format!(
            "Exceeded maximum of {} turns",
            max_turns
        )));
    }

    let final_message = turns.last().and_then(|t| t.content.clone());

    Ok(LoopResult {
        turns,
        final_message,
        total_turns: turn_count,
        verify_state,
    })
}

// ============================================================================
// Verification Helpers
// ============================================================================

fn is_typecheck_command(args: &str) -> bool {
    args.contains("bun run typecheck")
        || args.contains("tsc")
        || args.contains("cargo check")
        || args.contains("cargo build")
}

fn is_test_command(args: &str) -> bool {
    args.contains("bun test")
        || args.contains("bun run test")
        || args.contains("cargo test")
        || args.contains("pytest")
}

fn is_typecheck_success(output: &str) -> bool {
    // No TypeScript errors and no exit code errors
    !output.contains("error TS")
        && !output.contains("exited with code 1")
        && !output.contains("exited with code 2")
        && !output.contains("error[E")
}

fn is_test_success(output: &str) -> bool {
    // Contains "pass" and "0 fail" or no "fail" count
    (output.contains("pass") || output.contains("ok."))
        && (output.contains("0 fail") || !output.contains(" fail"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_typecheck_command() {
        assert!(is_typecheck_command("bun run typecheck"));
        assert!(is_typecheck_command("tsc --noEmit"));
        assert!(is_typecheck_command("cargo check"));
        assert!(!is_typecheck_command("echo hello"));
    }

    #[test]
    fn test_is_test_command() {
        assert!(is_test_command("bun test"));
        assert!(is_test_command("bun run test"));
        assert!(is_test_command("cargo test"));
        assert!(is_test_command("pytest"));
        assert!(!is_test_command("echo hello"));
    }

    #[test]
    fn test_is_typecheck_success() {
        assert!(is_typecheck_success("Build succeeded"));
        assert!(!is_typecheck_success("error TS2304: Cannot find name"));
        assert!(!is_typecheck_success("Process exited with code 1"));
    }

    #[test]
    fn test_is_test_success() {
        assert!(is_test_success("9 tests passed, 0 fail"));
        assert!(is_test_success("test result: ok. 5 passed"));
        assert!(!is_test_success("2 tests failed"));
    }

    #[test]
    fn test_loop_config_builder() {
        let config = LoopConfig::new()
            .with_model("claude-3")
            .with_system_prompt("You are helpful")
            .with_max_turns(5)
            .with_temperature(0.7);

        assert_eq!(config.model, Some("claude-3".to_string()));
        assert_eq!(config.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(config.max_turns, Some(5));
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_loop_event_serialization() {
        let event = LoopEvent::TurnStart { turn: 1 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"turn_start\""));
        assert!(json.contains("\"turn\":1"));
    }

    #[test]
    fn test_loop_tool_call_from_content_part() {
        let part = ContentPart::ToolUse {
            id: "tool-123".to_string(),
            name: "read".to_string(),
            input: serde_json::json!({"path": "/tmp/test.txt"}),
        };
        let tool_call = LoopToolCall::from_content_part(&part).unwrap();
        assert_eq!(tool_call.id, "tool-123");
        assert_eq!(tool_call.name, "read");
        assert!(tool_call.arguments.contains("path"));
    }
}
