//! Core Pi agent runtime
//!
//! The `PiAgent` provides an autonomous coding agent that streams LLM responses
//! and executes tools in a loop until the task is complete.

use std::sync::Arc;

use async_stream::try_stream;
use futures::Stream;
use llm::{
    ChatOptions, ChatStream, ContentPart, LlmProvider, LlmResult, Message, ProviderConfig,
    StreamChunk, Usage,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::PiConfig;
use crate::error::{PiError, PiResult};
use crate::events::{AgentEvent, AgentOutcome, StopReason};
use crate::state::AgentState;

/// Pi coding agent
///
/// An autonomous agent that executes tasks by calling LLMs and tools in a loop.
pub struct PiAgent {
    /// Agent configuration
    config: PiConfig,

    /// LLM provider for chat completions
    provider: Arc<dyn LlmProvider>,

    /// Current agent state
    state: AgentState,

    /// Conversation history
    messages: Vec<Message>,

    /// Cancellation token for aborting operations
    cancel_token: CancellationToken,

    /// Session ID for persistence
    session_id: String,

    /// Total turns executed
    total_turns: u32,

    /// Total cost in USD
    total_cost_usd: f64,
}

impl PiAgent {
    /// Create a new Pi agent with the given provider and configuration
    pub fn new(provider: Arc<dyn LlmProvider>, config: PiConfig) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();

        Self {
            config,
            provider,
            state: AgentState::Idle,
            messages: Vec::new(),
            cancel_token: CancellationToken::new(),
            session_id,
            total_turns: 0,
            total_cost_usd: 0.0,
        }
    }

    /// Create a Pi agent using Anthropic provider from environment
    pub fn anthropic(config: PiConfig) -> PiResult<Self> {
        let provider_config = ProviderConfig::anthropic_from_env()
            .ok_or_else(|| PiError::Config("ANTHROPIC_API_KEY not set".to_string()))?;

        let provider = llm::AnthropicProvider::new(provider_config)?;
        Ok(Self::new(Arc::new(provider), config))
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the current state
    pub fn state(&self) -> &AgentState {
        &self.state
    }

    /// Get the conversation history
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get the cancellation token for external cancellation
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Cancel the current operation
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Run the agent with a user prompt
    ///
    /// Returns a stream of `AgentEvent`s that can be consumed for UI updates.
    pub fn run(
        &mut self,
        prompt: impl Into<String>,
    ) -> impl Stream<Item = PiResult<AgentEvent>> + '_ {
        let prompt = prompt.into();

        try_stream! {
            // Reset state for new run
            self.state = AgentState::Thinking { turn: 1 };
            self.total_turns = 0;
            self.total_cost_usd = 0.0;

            // Create new cancellation token for this run
            self.cancel_token = CancellationToken::new();

            // Emit started event
            yield AgentEvent::Started {
                session_id: self.session_id.clone(),
                model: self.config.model.clone(),
            };

            // Add user message
            self.messages.push(Message::user(&prompt));

            // Main agent loop
            let mut current_turn = 1u32;

            loop {
                // Check cancellation
                if self.cancel_token.is_cancelled() {
                    self.state = AgentState::Cancelled;
                    yield AgentEvent::Cancelled;
                    return;
                }

                // Check max turns
                if current_turn > self.config.max_turns {
                    self.state = AgentState::Completed;
                    yield AgentEvent::Completed {
                        total_turns: self.total_turns,
                        total_cost_usd: self.total_cost_usd,
                        outcome: AgentOutcome::MaxTurnsExceeded { turns: current_turn - 1 },
                    };
                    return;
                }

                info!(turn = current_turn, "Starting turn");
                yield AgentEvent::TurnStart { turn: current_turn };

                self.state = AgentState::Thinking { turn: current_turn };

                // Build chat options
                let options = self.build_chat_options();

                // Stream LLM response
                let stream_result = self.provider.chat_stream(&self.messages, Some(options)).await;

                let mut stream = match stream_result {
                    Ok(s) => s,
                    Err(e) => {
                        error!(error = %e, "LLM error");
                        let retryable = e.is_retryable();
                        yield AgentEvent::Error {
                            message: e.to_string(),
                            retryable,
                        };

                        if !retryable {
                            self.state = AgentState::Error { message: e.to_string() };
                            return;
                        }

                        // Retry logic would go here
                        continue;
                    }
                };

                self.state = AgentState::Streaming { turn: current_turn };

                // Process stream
                let mut accumulated_text = String::new();
                let mut tool_calls: Vec<PendingToolCall> = Vec::new();
                let mut current_tool: Option<PendingToolCall> = None;
                let mut usage = Usage::default();
                let mut stop_reason = StopReason::EndTurn;

                use futures::StreamExt;
                while let Some(chunk_result) = stream.next().await {
                    // Check cancellation
                    if self.cancel_token.is_cancelled() {
                        self.state = AgentState::Cancelled;
                        yield AgentEvent::Cancelled;
                        return;
                    }

                    let chunk = match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            warn!(error = %e, "Stream error");
                            yield AgentEvent::Error {
                                message: e.to_string(),
                                retryable: e.is_retryable(),
                            };
                            continue;
                        }
                    };

                    match chunk {
                        StreamChunk::Start { id: _, model: _ } => {
                            // Response started
                        }
                        StreamChunk::Text(text) => {
                            accumulated_text.push_str(&text);
                            yield AgentEvent::TextDelta { text };
                        }
                        StreamChunk::ToolUseStart { id, name } => {
                            debug!(id = %id, name = %name, "Tool use started");
                            current_tool = Some(PendingToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                input_json: String::new(),
                            });
                            yield AgentEvent::ToolUseStart { id, name };
                        }
                        StreamChunk::ToolInputDelta(json) => {
                            if let Some(ref mut tool) = current_tool {
                                tool.input_json.push_str(&json);
                                yield AgentEvent::ToolInputDelta {
                                    id: tool.id.clone(),
                                    json,
                                };
                            }
                        }
                        StreamChunk::ToolUseEnd => {
                            if let Some(tool) = current_tool.take() {
                                tool_calls.push(tool);
                            }
                        }
                        StreamChunk::Done { stop_reason: sr, usage: u } => {
                            usage = u;
                            if let Some(sr) = sr {
                                stop_reason = sr.into();
                            }
                        }
                        StreamChunk::Error(e) => {
                            yield AgentEvent::Error {
                                message: e,
                                retryable: false,
                            };
                        }
                    }
                }

                // Calculate cost for this turn
                let turn_cost = self.calculate_cost(&usage);
                self.total_cost_usd += turn_cost;
                self.total_turns = current_turn;

                // Emit message complete if we have text
                if !accumulated_text.is_empty() {
                    yield AgentEvent::MessageComplete { text: accumulated_text.clone() };
                }

                // Build assistant message with content
                let mut content_parts = Vec::new();
                if !accumulated_text.is_empty() {
                    content_parts.push(ContentPart::text(&accumulated_text));
                }
                for tool in &tool_calls {
                    let input: serde_json::Value = serde_json::from_str(&tool.input_json)
                        .unwrap_or(serde_json::Value::Null);
                    content_parts.push(ContentPart::tool_use(&tool.id, &tool.name, input));
                }

                // Add assistant message to history
                self.messages.push(Message::new(
                    llm::Role::Assistant,
                    llm::Content::Parts(content_parts),
                ));

                yield AgentEvent::TurnComplete {
                    turn: current_turn,
                    usage: usage.clone(),
                    cost_usd: turn_cost,
                    stop_reason,
                };

                // If no tool calls, we're done
                if tool_calls.is_empty() {
                    self.state = AgentState::Completed;
                    yield AgentEvent::Completed {
                        total_turns: self.total_turns,
                        total_cost_usd: self.total_cost_usd,
                        outcome: AgentOutcome::Success { response: accumulated_text },
                    };
                    return;
                }

                // Execute tool calls
                for tool in tool_calls {
                    // Check cancellation before each tool
                    if self.cancel_token.is_cancelled() {
                        self.state = AgentState::Cancelled;
                        yield AgentEvent::Cancelled;
                        return;
                    }

                    self.state = AgentState::ExecutingTool {
                        turn: current_turn,
                        tool_id: tool.id.clone(),
                        tool_name: tool.name.clone(),
                    };

                    let input: serde_json::Value = serde_json::from_str(&tool.input_json)
                        .unwrap_or(serde_json::Value::Null);

                    yield AgentEvent::ToolExecuting {
                        id: tool.id.clone(),
                        name: tool.name.clone(),
                        input: input.clone(),
                    };

                    // Execute tool (placeholder - Phase 2 will implement this)
                    let (output, is_error) = self.execute_tool(&tool.name, &input).await;

                    yield AgentEvent::ToolResult {
                        id: tool.id.clone(),
                        name: tool.name.clone(),
                        output: output.clone(),
                        is_error,
                    };

                    // Add tool result to messages
                    if is_error {
                        self.messages.push(Message::tool_error(&tool.id, &output));
                    } else {
                        self.messages.push(Message::tool_result(&tool.id, &output));
                    }
                }

                current_turn += 1;
            }
        }
    }

    /// Build chat options for the LLM call
    fn build_chat_options(&self) -> ChatOptions {
        let mut options = ChatOptions::default();

        options.model = Some(self.config.model.clone());
        options.max_tokens = Some(self.config.max_tokens);

        // Build system prompt
        if let Some(ref system) = self.config.system_prompt {
            options.system = Some(system.clone());
        }

        // Add tool definitions
        options.tools = self.build_tool_definitions();

        if self.config.enable_thinking {
            options.enable_thinking = true;
        }

        options
    }

    /// Build tool definitions for the LLM
    fn build_tool_definitions(&self) -> Vec<llm::ToolDefinition> {
        use llm::ToolDefinition;

        // Default tools - Phase 2 will make this dynamic
        vec![
            ToolDefinition {
                name: "bash".to_string(),
                description: "Execute a bash command".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The bash command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write".to_string(),
                description: "Write content to a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to write to"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "edit".to_string(),
                description: "Edit a file by replacing text".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit"
                        },
                        "old_text": {
                            "type": "string",
                            "description": "Text to replace"
                        },
                        "new_text": {
                            "type": "string",
                            "description": "Replacement text"
                        }
                    },
                    "required": ["path", "old_text", "new_text"]
                }),
            },
        ]
    }

    /// Execute a tool (placeholder implementation)
    async fn execute_tool(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> (String, bool) {
        // Phase 2 will implement proper tool execution
        // For now, return a placeholder
        debug!(tool = name, "Executing tool (placeholder)");

        match name {
            "bash" => {
                let command = input.get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Use the tools crate for bash execution
                match tools::bash::execute(command, &self.config.working_directory) {
                    Ok(output) => (output.stdout, false),
                    Err(e) => (e.to_string(), true),
                }
            }
            "read" => {
                let path = input.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let full_path = self.config.working_directory.join(path);
                match tools::read::read_file(&full_path) {
                    Ok(content) => (content, false),
                    Err(e) => (e.to_string(), true),
                }
            }
            "write" => {
                let path = input.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = input.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let full_path = self.config.working_directory.join(path);
                match tools::write::write_file(&full_path, content) {
                    Ok(()) => ("File written successfully".to_string(), false),
                    Err(e) => (e.to_string(), true),
                }
            }
            "edit" => {
                let path = input.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let old_text = input.get("old_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_text = input.get("new_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let full_path = self.config.working_directory.join(path);
                match tools::edit::edit_file(&full_path, old_text, new_text) {
                    Ok(()) => ("File edited successfully".to_string(), false),
                    Err(e) => (e.to_string(), true),
                }
            }
            _ => {
                (format!("Unknown tool: {}", name), true)
            }
        }
    }

    /// Calculate cost for token usage
    fn calculate_cost(&self, usage: &Usage) -> f64 {
        // Use pricing from ai crate if available
        // For now, use Claude Sonnet pricing as default
        let input_cost_per_mtok = 3.0; // $3/million input tokens
        let output_cost_per_mtok = 15.0; // $15/million output tokens

        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * input_cost_per_mtok;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * output_cost_per_mtok;

        input_cost + output_cost
    }
}

/// A pending tool call being streamed
#[derive(Debug)]
struct PendingToolCall {
    id: String,
    name: String,
    input_json: String,
}
