//! Codex completion model adapter.
//!
//! Wraps the Codex app-server JSON-RPC API to work as a DSPy completion provider.
//! This allows using Codex as the LLM backend for issue validation and other DSPy pipelines.

use anyhow::Result;
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, ClientInfo, ThreadStartParams,
    TurnStartParams, UserInput, is_codex_available,
};
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::AssistantContent;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Re-export availability check for external use.
pub use codex_client::is_codex_available as check_codex_available;

/// Codex completion model that wraps the Codex app-server.
///
/// Uses the JSON-RPC API over stdin/stdout to communicate with Codex.
/// Each completion request spawns a new turn and collects the response.
#[derive(Clone)]
pub struct CodexCompletionModel {
    model: String,
    /// Shared client connection (lazy-initialized)
    client: Arc<Mutex<Option<CodexClientState>>>,
}

struct CodexClientState {
    client: AppServerClient,
    channels: AppServerChannels,
    thread_id: String,
}

impl CodexCompletionModel {
    /// Create a new Codex completion model.
    ///
    /// The model name is informational; Codex uses its configured model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if Codex is available on this system.
    pub fn is_available() -> bool {
        is_codex_available()
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Initialize the Codex client if not already connected.
    async fn ensure_client(&self) -> Result<(), CompletionError> {
        let mut guard = self.client.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        // Spawn the app-server
        let config = AppServerConfig::default();
        let (client, channels) = AppServerClient::spawn(config)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Initialize the client
        let info = ClientInfo {
            name: "dsrs-codex-adapter".to_string(),
            title: Some("DSPy Codex Adapter".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        client
            .initialize(info)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Start a thread for this session
        let thread_response = client
            .thread_start(ThreadStartParams::default())
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        *guard = Some(CodexClientState {
            client,
            channels,
            thread_id: thread_response.thread.id,
        });

        Ok(())
    }

    /// Convert a CompletionRequest to a prompt string for Codex.
    fn convert_to_prompt(request: &CompletionRequest) -> String {
        let mut prompt = String::new();

        // Add preamble as system context
        if let Some(preamble) = &request.preamble {
            prompt.push_str("System: ");
            prompt.push_str(preamble);
            prompt.push_str("\n\n");
        }

        // Add chat history
        for msg in request.chat_history.iter() {
            prompt.push_str(&format_message(msg));
        }

        prompt
    }

    /// Collect response from Codex notifications until turn completes.
    async fn collect_response(channels: &mut AppServerChannels) -> Result<String, CompletionError> {
        Self::collect_response_streaming(channels, None::<fn(&str)>).await
    }

    /// Collect response from Codex with streaming callback.
    async fn collect_response_streaming<F>(
        channels: &mut AppServerChannels,
        on_token: Option<F>,
    ) -> Result<String, CompletionError>
    where
        F: Fn(&str),
    {
        let mut response = String::new();

        loop {
            tokio::select! {
                notification = channels.notifications.recv() => {
                    match notification {
                        Some(notif) => {
                            match notif.method.as_str() {
                                "item/agentMessage/delta" => {
                                    // Parse delta notification
                                    if let Some(params) = notif.params
                                        && let Some(delta) = params.get("delta").and_then(|v| v.as_str()) {
                                            // Stream token via callback
                                            if let Some(ref cb) = on_token {
                                                cb(delta);
                                            }
                                            response.push_str(delta);
                                        }
                                }
                                "turn/completed" => {
                                    // Turn finished
                                    break;
                                }
                                "turn/error" => {
                                    // Turn failed
                                    let error_msg = notif.params
                                        .and_then(|p| p.get("message").and_then(|v| v.as_str().map(String::from)))
                                        .unwrap_or_else(|| "Unknown turn error".to_string());
                                    return Err(CompletionError::ProviderError(error_msg));
                                }
                                _ => {
                                    // Ignore other notifications
                                }
                            }
                        }
                        None => {
                            return Err(CompletionError::ProviderError(
                                "Codex notification channel closed unexpectedly".to_string()
                            ));
                        }
                    }
                }
                // Timeout after 120 seconds
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(120)) => {
                    return Err(CompletionError::ProviderError(
                        "Codex turn timed out after 120 seconds".to_string()
                    ));
                }
            }
        }

        Ok(response)
    }

    /// Completion with streaming callback.
    pub async fn completion_streaming<F>(
        &self,
        request: CompletionRequest,
        on_token: Option<F>,
    ) -> Result<CompletionResponse<()>, CompletionError>
    where
        F: Fn(&str),
    {
        // Ensure client is connected
        self.ensure_client().await?;

        let prompt = Self::convert_to_prompt(&request);

        // Get access to client state
        let mut guard = self.client.lock().await;
        let state = guard.as_mut().ok_or_else(|| {
            CompletionError::ProviderError("Codex client not initialized".to_string())
        })?;

        // Start a turn with the prompt
        // TEMP: Force gpt-5.1-codex-mini model override
        let turn_params = TurnStartParams {
            thread_id: state.thread_id.clone(),
            input: vec![UserInput::Text { text: prompt }],
            model: Some("gpt-5.1-codex-mini".to_string()),
            effort: None,
            summary: None,
            approval_policy: None,
            sandbox_policy: None,
            cwd: None,
        };

        state
            .client
            .turn_start(turn_params)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Collect response with streaming callback
        let response_text = Self::collect_response_streaming(&mut state.channels, on_token).await?;

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(rig::message::Text {
                text: response_text,
            })),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            raw_response: (),
        })
    }
}

fn format_message(msg: &rig::message::Message) -> String {
    match msg {
        rig::message::Message::User { content } => {
            let text = content
                .iter()
                .map(extract_user_content_text)
                .collect::<Vec<_>>()
                .join(" ");
            format!("User: {}\n\n", text)
        }
        rig::message::Message::Assistant { content, .. } => {
            let text = content
                .iter()
                .map(extract_assistant_content_text)
                .collect::<Vec<_>>()
                .join(" ");
            format!("Assistant: {}\n\n", text)
        }
    }
}

fn extract_user_content_text(content: &rig::message::UserContent) -> String {
    match content {
        rig::message::UserContent::Text(t) => t.text.clone(),
        _ => String::new(),
    }
}

fn extract_assistant_content_text(content: &rig::message::AssistantContent) -> String {
    match content {
        rig::message::AssistantContent::Text(t) => t.text.clone(),
        rig::message::AssistantContent::Reasoning(r) => r.reasoning.join("\n"),
        _ => String::new(),
    }
}

impl super::client_registry::CompletionProvider for CodexCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        // Ensure client is connected
        self.ensure_client().await?;

        let prompt = Self::convert_to_prompt(&request);

        // Get access to client state
        let mut guard = self.client.lock().await;
        let state = guard.as_mut().ok_or_else(|| {
            CompletionError::ProviderError("Codex client not initialized".to_string())
        })?;

        // Start a turn with the prompt
        // TEMP: Force gpt-5.1-codex-mini model override
        let turn_params = TurnStartParams {
            thread_id: state.thread_id.clone(),
            input: vec![UserInput::Text { text: prompt }],
            model: Some("gpt-5.1-codex-mini".to_string()),
            effort: None,
            summary: None,
            approval_policy: None,
            sandbox_policy: None,
            cwd: None,
        };

        state
            .client
            .turn_start(turn_params)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Collect response until turn completes
        let response_text = Self::collect_response(&mut state.channels).await?;

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(rig::message::Text {
                text: response_text,
            })),
            usage: Usage {
                input_tokens: 0, // Codex doesn't report token usage
                output_tokens: 0,
                total_tokens: 0,
            },
            raw_response: (),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_availability_check() {
        // Just verify the function doesn't panic
        let _ = CodexCompletionModel::is_available();
    }

    #[test]
    fn test_codex_model_creation() {
        let model = CodexCompletionModel::new("claude-3.5-sonnet");
        assert_eq!(model.model(), "claude-3.5-sonnet");
    }
}
