//! Codex provider via codex-agent-sdk.
//!
//! Uses the Codex CLI headless mode for inference.
//! This is the primary provider (Claude is deprecated).
//!
//! IMPORTANT: This implementation reuses a single CLI process across
//! all completions to avoid memory leaks.

use anyhow::Result;
use codex_agent_sdk::{
    Codex, ThreadOptions, TurnOptions, ThreadEvent, ThreadItemDetails, Thread,
};
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};
use crate::callbacks::DspyCallback;
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// Global thread for Codex CLI process reuse.
///
/// This prevents spawning a new Codex CLI process for each completion,
/// which would leak memory (6GB+ per process with MCP servers).
static CODEX_SESSION: OnceLock<Mutex<Option<CodexSession>>> = OnceLock::new();

/// Holds the Codex client and active thread.
struct CodexSession {
    #[allow(dead_code)]
    codex: Codex,
    thread: Thread,
}

/// Check if Codex CLI is available.
///
/// Checks for `codex` in PATH.
pub fn has_codex_cli() -> bool {
    which::which("codex").is_ok()
}

/// Codex completion model via codex-agent-sdk.
///
/// This provider uses Codex CLI's headless mode for inference.
///
/// IMPORTANT: This reuses a single CLI process across all completions
/// via the Thread API to avoid memory leaks.
#[derive(Clone)]
pub struct CodexSdkModel {
    /// Maximum turns (not used by Codex, kept for API parity)
    pub max_turns: Option<u32>,
}

impl Default for CodexSdkModel {
    fn default() -> Self {
        Self { max_turns: Some(1) }
    }
}

impl CodexSdkModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute completion via codex-agent-sdk.
    ///
    /// Streams the response and returns the final result text.
    pub async fn complete(&self, prompt: &str) -> Result<String, CompletionError> {
        self.complete_streaming(prompt, None).await
    }

    /// Execute completion with streaming callback.
    ///
    /// Reuses a single CLI process across all completions via Thread API.
    pub async fn complete_streaming(
        &self,
        prompt: &str,
        callback: Option<(&dyn DspyCallback, uuid::Uuid)>,
    ) -> Result<String, CompletionError> {
        // Get or create the global session
        let session_lock = CODEX_SESSION.get_or_init(|| Mutex::new(None));
        let mut session_guard = session_lock.lock().await;

        // Create session if needed
        if session_guard.is_none() {
            tracing::info!("CodexSdk: creating persistent thread (spawning CLI process)");
            let codex = Codex::new();
            let thread_options = ThreadOptions::new()
                .skip_git_repo_check(true);
            let thread = codex.start_thread(thread_options);
            *session_guard = Some(CodexSession { codex, thread });
        }

        tracing::info!("CodexSdk: sending prompt to existing thread");

        // Scope the session borrow
        let result = {
            let session = session_guard.as_mut().unwrap();

            // Run streamed turn
            let turn_options = TurnOptions::default();
            let mut streamed = match session.thread.run_streamed(prompt, turn_options).await {
                Ok(s) => s,
                Err(e) => {
                    return Err(CompletionError::ProviderError(format!(
                        "Failed to start turn: {}",
                        e
                    )));
                }
            };

            // Collect response text from AgentMessage events
            let mut result_text = String::new();
            let mut last_text_len = 0;

            while let Some(event) = streamed.next().await {
                match event {
                    Ok(ThreadEvent::ItemUpdated(item)) => {
                        if let ThreadItemDetails::AgentMessage(msg) = &item.item.details {
                            // Calculate delta for streaming callback
                            let current_text = &msg.text;
                            if current_text.len() > last_text_len {
                                let delta = &current_text[last_text_len..];
                                if let Some((cb, call_id)) = callback {
                                    cb.on_lm_token(call_id, delta);
                                }
                                last_text_len = current_text.len();
                            }
                            result_text = current_text.clone();
                        }
                    }
                    Ok(ThreadEvent::ItemCompleted(item)) => {
                        // Final text from completed AgentMessage
                        if let ThreadItemDetails::AgentMessage(msg) = &item.item.details {
                            // Stream any remaining delta
                            if msg.text.len() > last_text_len {
                                let delta = &msg.text[last_text_len..];
                                if let Some((cb, call_id)) = callback {
                                    cb.on_lm_token(call_id, delta);
                                }
                            }
                            result_text = msg.text.clone();
                        }
                    }
                    Ok(ThreadEvent::TurnCompleted(_)) => {
                        tracing::debug!("CodexSdk: turn completed");
                        break;
                    }
                    Ok(ThreadEvent::TurnFailed(err)) => {
                        return Err(CompletionError::ProviderError(format!(
                            "Turn failed: {}",
                            err.error.message
                        )));
                    }
                    Ok(ThreadEvent::Error(err)) => {
                        return Err(CompletionError::ProviderError(format!(
                            "Stream error: {}",
                            err.message
                        )));
                    }
                    Ok(_) => {
                        // Ignore other events (TurnStarted, ThreadStarted, etc.)
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
                    "No response from Codex".into(),
                ))
            } else {
                Ok(result_text)
            }
        };

        result
    }
}

// Implement CompletionProvider
use super::CompletionProvider;

impl CompletionProvider for CodexSdkModel {
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
