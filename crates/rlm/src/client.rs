//! LLM client trait for abstracted LLM access.
//!
//! This module defines the `LlmClient` trait which provides a common interface
//! for different LLM backends. This allows `RlmEngine` to work with any backend
//! (FM Bridge, OpenAI, OpenRouter, etc.) through LmRouter.

use async_trait::async_trait;

use crate::error::RlmError;

/// Response from an LLM completion request.
///
/// This format is compatible with both FMClient and LmRouterClient responses.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The completion choices (typically just one).
    pub choices: Vec<LlmChoice>,
    /// Token usage statistics.
    pub usage: Option<LlmUsage>,
}

impl LlmResponse {
    /// Create a new LlmResponse with a single choice.
    pub fn new(content: String) -> Self {
        Self {
            choices: vec![LlmChoice {
                message: LlmMessage { content },
            }],
            usage: None,
        }
    }

    /// Create with usage information.
    pub fn with_usage(mut self, prompt_tokens: usize, completion_tokens: usize) -> Self {
        self.usage = Some(LlmUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        });
        self
    }

    /// Get the content of the first choice.
    pub fn content(&self) -> &str {
        self.choices
            .first()
            .map(|c| c.message.content.as_str())
            .unwrap_or("")
    }
}

/// A choice in the LLM response.
#[derive(Debug, Clone)]
pub struct LlmChoice {
    /// The message content.
    pub message: LlmMessage,
}

/// A message in the LLM response.
#[derive(Debug, Clone)]
pub struct LlmMessage {
    /// The text content.
    pub content: String,
}

/// Token usage statistics.
#[derive(Debug, Clone)]
pub struct LlmUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: usize,
    /// Number of tokens in the completion.
    pub completion_tokens: usize,
    /// Total tokens (prompt + completion).
    pub total_tokens: usize,
}

/// Trait for LLM clients.
///
/// This trait provides a common interface for different LLM backends,
/// allowing `RlmEngine` to work with any backend through this abstraction.
///
/// # Example
///
/// ```rust,ignore
/// use rlm::{LlmClient, LlmResponse};
///
/// struct MyClient;
///
/// #[async_trait]
/// impl LlmClient for MyClient {
///     async fn complete(&self, prompt: &str, max_tokens: Option<usize>)
///         -> Result<LlmResponse, RlmError>
///     {
///         // Implementation here
///     }
/// }
/// ```
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Complete a prompt and return the response.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to complete
    /// * `max_tokens` - Optional maximum tokens to generate
    ///
    /// # Returns
    ///
    /// The completion response containing the generated text and usage info.
    async fn complete(
        &self,
        prompt: &str,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, RlmError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_response_new() {
        let response = LlmResponse::new("Hello, world!".to_string());
        assert_eq!(response.content(), "Hello, world!");
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_llm_response_with_usage() {
        let response = LlmResponse::new("Test".to_string()).with_usage(10, 5);
        assert_eq!(response.content(), "Test");
        assert!(response.usage.is_some());
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }
}
