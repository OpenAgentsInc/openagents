//! Claude provider via claude-agent-sdk.
//!
//! Uses the existing SDK which wraps Claude CLI headless mode.
//! Priority: Best quality inference when Claude CLI is available.

use anyhow::Result;
use claude_agent_sdk::{QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig, query};
use futures::StreamExt;
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};

/// Check if Claude CLI is available.
///
/// Checks both PATH and the standard install location.
pub fn has_claude_cli() -> bool {
    which::which("claude").is_ok()
        || dirs::home_dir()
            .map(|h| h.join(".claude/local/claude").exists())
            .unwrap_or(false)
}

/// Claude completion model via claude-agent-sdk.
///
/// This provider uses Claude Code's headless mode for inference,
/// leveraging the user's existing Claude subscription (Pro/Max).
#[derive(Clone)]
pub struct ClaudeSdkModel {
    /// Maximum turns for the query (1 = single completion)
    pub max_turns: Option<u32>,
}

impl Default for ClaudeSdkModel {
    fn default() -> Self {
        Self { max_turns: Some(1) } // Single turn for pure completion
    }
}

impl ClaudeSdkModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute completion via claude-agent-sdk.
    ///
    /// Streams the response and returns the final result text.
    pub async fn complete(&self, prompt: &str) -> Result<String, CompletionError> {
        let options = QueryOptions::new()
            .max_turns(self.max_turns.unwrap_or(1))
            .tools(ToolsConfig::none()); // No tools for pure LM completion

        let mut stream = query(prompt, options)
            .await
            .map_err(|e| CompletionError::ProviderError(format!("Failed to start query: {}", e)))?;

        let mut result_text = String::new();

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(SdkMessage::Result(result)) => {
                    // Final result - extract the text
                    match result {
                        SdkResultMessage::Success(success) => {
                            result_text = success.result;
                        }
                        SdkResultMessage::ErrorDuringExecution(err) => {
                            return Err(CompletionError::ProviderError(format!(
                                "Claude error: {}",
                                err.errors.join(", ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxTurns(err) => {
                            return Err(CompletionError::ProviderError(format!(
                                "Max turns exceeded: {}",
                                err.errors.join(", ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxBudget(err) => {
                            return Err(CompletionError::ProviderError(format!(
                                "Max budget exceeded: {}",
                                err.errors.join(", ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                            return Err(CompletionError::ProviderError(format!(
                                "Structured output retries exceeded: {}",
                                err.errors.join(", ")
                            )));
                        }
                    }
                    break;
                }
                Ok(SdkMessage::Assistant(_)) => {
                    // Streaming assistant messages - we wait for the final result
                }
                Ok(_) => {} // Ignore other message types
                Err(e) => {
                    return Err(CompletionError::ProviderError(format!(
                        "Stream error: {}",
                        e
                    )));
                }
            }
        }

        if result_text.is_empty() {
            return Err(CompletionError::ProviderError(
                "No response from Claude".into(),
            ));
        }

        Ok(result_text)
    }
}

// Implement CompletionProvider
use super::CompletionProvider;

impl CompletionProvider for ClaudeSdkModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let prompt = build_prompt_from_request(&request);
        let result = self.complete(&prompt).await?;

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: result })),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            raw_response: (),
        })
    }
}

/// Build a prompt string from a rig CompletionRequest.
fn build_prompt_from_request(request: &CompletionRequest) -> String {
    let mut parts = Vec::new();

    // Add preamble/system prompt
    if let Some(preamble) = &request.preamble {
        parts.push(format!("System: {}", preamble));
    }

    // Add chat history
    for msg in request.chat_history.iter() {
        match msg {
            rig::message::Message::User { content } => {
                for c in content.iter() {
                    if let rig::message::UserContent::Text(text) = c {
                        parts.push(format!("User: {}", text.text));
                    }
                }
            }
            rig::message::Message::Assistant { content, .. } => {
                for c in content.iter() {
                    if let rig::message::AssistantContent::Text(text) = c {
                        parts.push(format!("Assistant: {}", text.text));
                    }
                }
            }
        }
    }

    parts.join("\n\n")
}
