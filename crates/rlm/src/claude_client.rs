//! Claude LLM client for RLM.
//!
//! Allows RlmEngine to use Claude (Pro/Max) as its inference backend.
//! This implements Mode B of the RLM + Claude integration: Claude IS the RLM.
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::{RlmEngine, ClaudeLlmClient, PythonExecutor, RlmConfig};
//!
//! let client = ClaudeLlmClient::new("/path/to/workspace");
//! let executor = PythonExecutor::new()?;
//! let engine = RlmEngine::new(client, executor);
//!
//! let result = engine.run("Analyze this codebase").await?;
//! ```

use async_trait::async_trait;
use claude_agent_sdk::{query, QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig};
use futures::StreamExt;
use std::path::PathBuf;

use crate::client::{LlmClient, LlmResponse};
use crate::error::RlmError;

/// LLM client that uses Claude via claude-agent-sdk.
///
/// This client wraps the Claude CLI to provide completions for the RLM engine.
/// It disables tools for raw completion mode, allowing Claude to focus on
/// generating code and reasoning without invoking external tools.
pub struct ClaudeLlmClient {
    workspace_root: PathBuf,
    model: Option<String>,
    /// Whether to persist sessions (default: false for RLM mode)
    persist_session: bool,
}

impl ClaudeLlmClient {
    /// Create a new Claude LLM client.
    ///
    /// # Arguments
    /// * `workspace_root` - The working directory for Claude sessions
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            model: None,
            persist_session: false,
        }
    }

    /// Set the model to use (e.g., "claude-sonnet-4-5-20250929").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Enable session persistence.
    pub fn with_persistence(mut self, persist: bool) -> Self {
        self.persist_session = persist;
        self
    }
}

#[async_trait]
impl LlmClient for ClaudeLlmClient {
    async fn complete(
        &self,
        prompt: &str,
        _max_tokens: Option<usize>,
    ) -> Result<LlmResponse, RlmError> {
        // Build query options for raw completion mode
        let mut options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .max_turns(1) // Single turn for RLM queries
            .tools(ToolsConfig::none()); // No tools for raw completion

        if let Some(ref model) = self.model {
            options = options.model(model.clone());
        }

        if !self.persist_session {
            // Don't persist sessions for RLM mode
            options.persist_session = false;
        }

        // Execute the query
        let mut stream = query(prompt, options)
            .await
            .map_err(|e| RlmError::ClientError(format!("Claude query failed: {}", e)))?;

        let mut content = String::new();
        let mut prompt_tokens = 0usize;
        let mut completion_tokens = 0usize;

        // Process the stream
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(SdkMessage::Assistant(msg)) => {
                    // Accumulate assistant text
                    if let Some(ref text) = msg.message.content {
                        content.push_str(text);
                    }
                }
                Ok(SdkMessage::Result(result)) => {
                    match result {
                        SdkResultMessage::Success(s) => {
                            // Use the result if no content accumulated
                            if content.is_empty() {
                                content = s.result;
                            }
                            // Extract usage if available
                            if let Some(usage) = s.total_usage {
                                prompt_tokens = usage.input_tokens as usize;
                                completion_tokens = usage.output_tokens as usize;
                            }
                            break;
                        }
                        SdkResultMessage::ErrorDuringExecution(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude execution error: {}",
                                e.error
                            )));
                        }
                        SdkResultMessage::ErrorMaxTurns(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude max turns exceeded: {}",
                                e.error
                            )));
                        }
                        SdkResultMessage::ErrorMaxBudget(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude budget exceeded: {}",
                                e.error
                            )));
                        }
                        SdkResultMessage::Interrupted(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude interrupted: {}",
                                e.reason.unwrap_or_else(|| "unknown".to_string())
                            )));
                        }
                    }
                }
                Ok(SdkMessage::System(sys)) => {
                    // Could log system messages if needed
                    tracing::debug!("Claude system message: {:?}", sys);
                }
                Err(e) => {
                    return Err(RlmError::ClientError(format!("Stream error: {}", e)));
                }
                _ => {
                    // Ignore other message types (User, Control, etc.)
                }
            }
        }

        // Build response
        let mut response = LlmResponse::new(content);
        if prompt_tokens > 0 || completion_tokens > 0 {
            response = response.with_usage(prompt_tokens, completion_tokens);
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_client_builder() {
        let client = ClaudeLlmClient::new("/tmp")
            .with_model("claude-sonnet-4-5-20250929")
            .with_persistence(true);

        assert_eq!(
            client.model,
            Some("claude-sonnet-4-5-20250929".to_string())
        );
        assert!(client.persist_session);
    }

    #[test]
    fn test_claude_client_default() {
        let client = ClaudeLlmClient::new("/workspace");

        assert_eq!(client.workspace_root, PathBuf::from("/workspace"));
        assert!(client.model.is_none());
        assert!(!client.persist_session);
    }
}
