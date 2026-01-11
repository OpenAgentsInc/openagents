//! Claude provider via claude-agent-sdk.
//!
//! Uses the existing SDK which wraps Claude CLI headless mode.
//! Priority: Best quality inference when Claude CLI is available.
//!
//! IMPORTANT: This implementation reuses a single CLI process across
//! all completions to avoid memory leaks. Each `query()` call would
//! spawn a new process, so we use the Session API instead.

use anyhow::Result;
use claude_agent_sdk::{QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig, Session, unstable_v2_create_session};
use futures::StreamExt;
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};
use crate::callbacks::DspyCallback;
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// Global session for CLI process reuse.
///
/// This prevents spawning a new Claude CLI process for each completion,
/// which would leak memory (6GB+ per process with MCP servers).
static CLAUDE_SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();

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
///
/// IMPORTANT: This reuses a single CLI process across all completions
/// via the Session API to avoid memory leaks.
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
        self.complete_streaming(prompt, None).await
    }

    /// Execute completion with streaming callback.
    ///
    /// Reuses a single CLI process across all completions via Session API.
    pub async fn complete_streaming(
        &self,
        prompt: &str,
        callback: Option<(&dyn DspyCallback, uuid::Uuid)>,
    ) -> Result<String, CompletionError> {
        // Get or create the global session
        let session_lock = CLAUDE_SESSION.get_or_init(|| Mutex::new(None));
        let mut session_guard = session_lock.lock().await;

        // Create session if needed
        if session_guard.is_none() {
            tracing::info!("ClaudeSdk: creating persistent session (spawning CLI process)");
            let options = QueryOptions::new()
                .max_turns(self.max_turns.unwrap_or(1))
                .tools(ToolsConfig::none())
                .include_partial_messages(true);

            match unstable_v2_create_session(options).await {
                Ok(session) => {
                    *session_guard = Some(session);
                }
                Err(e) => {
                    return Err(CompletionError::ProviderError(format!(
                        "Failed to create session: {}",
                        e
                    )));
                }
            }
        }

        tracing::info!("ClaudeSdk: sending prompt to existing session");

        // Scope the session borrow to avoid borrow checker issues
        let result = {
            let session = session_guard.as_mut().unwrap();

            // Send the prompt
            if let Err(e) = session.send(prompt).await {
                return Err(CompletionError::ProviderError(format!(
                    "Failed to send prompt: {}",
                    e
                )));
            }

            // Receive response
            let mut result_text = String::new();

            while let Some(msg) = session.receive().next().await {
                match msg {
                    Ok(SdkMessage::Result(result)) => {
                        tracing::debug!("ClaudeSdk: received Result message");
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
                    Ok(SdkMessage::Assistant(assistant)) => {
                        tracing::debug!("ClaudeSdk: received Assistant message");
                        if let Some((cb, call_id)) = callback {
                            if let Some(text) = extract_assistant_text(&assistant) {
                                tracing::info!(
                                    "ClaudeSdk streaming {} chars from Assistant",
                                    text.len()
                                );
                                cb.on_lm_token(call_id, &text);
                            }
                        }
                    }
                    Ok(SdkMessage::StreamEvent(event)) => {
                        if let Some((cb, call_id)) = callback {
                            if let Some(text) = extract_stream_text(&event) {
                                tracing::trace!("ClaudeSdk streaming delta: {} chars", text.len());
                                cb.on_lm_token(call_id, &text);
                            } else {
                                let event_type = event
                                    .event
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown");
                                tracing::trace!("ClaudeSdk: StreamEvent type={}, no text", event_type);
                            }
                        }
                    }
                    Ok(_other) => {
                        tracing::debug!("ClaudeSdk: ignoring message type");
                    }
                    Err(e) => {
                        return Err(CompletionError::ProviderError(format!(
                            "Stream error: {}",
                            e
                        )));
                    }
                }
            }

            if result_text.is_empty() {
                Err(CompletionError::ProviderError(
                    "No response from Claude".into(),
                ))
            } else {
                Ok(result_text)
            }
        };

        result
    }
}

/// Extract text from assistant message.
fn extract_assistant_text(assistant: &claude_agent_sdk::SdkAssistantMessage) -> Option<String> {
    let content = assistant.message.get("content")?;
    let blocks = content.as_array()?;
    let mut text = String::new();
    for block in blocks {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract text from stream event.
fn extract_stream_text(event: &claude_agent_sdk::SdkStreamEvent) -> Option<String> {
    let event_type = event.event.get("type").and_then(|t| t.as_str())?;

    // Standard Claude API: content_block_delta with text_delta
    if event_type == "content_block_delta" {
        let delta = event.event.get("delta")?;
        if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
            return delta.get("text").and_then(|t| t.as_str()).map(|s| s.to_string());
        }
        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
            return Some(text.to_string());
        }
    }

    // Alternative: message_delta with text
    if event_type == "message_delta" {
        if let Some(delta) = event.event.get("delta") {
            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                return Some(text.to_string());
            }
        }
    }

    // Alternative: direct text field in event
    if let Some(text) = event.event.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    None
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
