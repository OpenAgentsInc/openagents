//! Ollama inference service
//!
//! Wraps the existing LLM provider for Ollama to handle inference requests.

use crate::state::OllamaModel;
use futures::StreamExt;
use llm::provider::{LlmProvider, OllamaProvider};
use llm::stream::{CompletionStream, StreamEvent};
use llm::{CompletionRequest, Message};
use std::sync::Arc;
use thiserror::Error;

/// Errors from the Ollama service
#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("Ollama not available: {0}")]
    NotAvailable(String),

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),
}

/// Service for interacting with Ollama
pub struct OllamaService {
    provider: Option<Arc<OllamaProvider>>,
}

impl OllamaService {
    /// Create a new Ollama service
    pub fn new() -> Self {
        let provider = OllamaProvider::new().ok().map(Arc::new);
        Self { provider }
    }

    /// Check if Ollama is available
    pub async fn is_available(&self) -> bool {
        match &self.provider {
            Some(p) => p.is_available().await,
            None => false,
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>, OllamaError> {
        let provider = self.provider.as_ref()
            .ok_or_else(|| OllamaError::NotAvailable("Ollama provider not initialized".into()))?;

        let models = provider
            .list_models()
            .await
            .map_err(|e| OllamaError::NotAvailable(e.to_string()))?;

        Ok(models
            .into_iter()
            .map(|m| OllamaModel {
                name: m.id.clone(),
                size: format_size(m.limits.context_window as u64 * 1024), // Approximate
                quantization: None,
                selected: false,
            })
            .collect())
    }

    /// Generate a completion (non-streaming)
    pub async fn generate(&self, model: &str, prompt: &str) -> Result<String, OllamaError> {
        let provider = self.provider.as_ref()
            .ok_or_else(|| OllamaError::NotAvailable("Ollama provider not initialized".into()))?;

        if !self.is_available().await {
            return Err(OllamaError::NotAvailable("Ollama is not running".into()));
        }

        let request = CompletionRequest::new(model).message(Message::user(prompt));

        let mut stream = provider
            .stream(request)
            .await
            .map_err(|e| OllamaError::InferenceFailed(e.to_string()))?;

        let mut result = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(event) => {
                    if let StreamEvent::TextDelta { delta, .. } = event {
                        result.push_str(&delta);
                    }
                }
                Err(e) => {
                    return Err(OllamaError::InferenceFailed(e.to_string()));
                }
            }
        }

        Ok(result)
    }

    /// Generate a streaming completion
    pub async fn generate_stream(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<CompletionStream, OllamaError> {
        let provider = self.provider.as_ref()
            .ok_or_else(|| OllamaError::NotAvailable("Ollama provider not initialized".into()))?;

        if !self.is_available().await {
            return Err(OllamaError::NotAvailable("Ollama is not running".into()));
        }

        let request = CompletionRequest::new(model).message(Message::user(prompt));

        provider
            .stream(request)
            .await
            .map_err(|e| OllamaError::InferenceFailed(e.to_string()))
    }
}

impl Default for OllamaService {
    fn default() -> Self {
        Self::new()
    }
}

/// Format bytes as human-readable size
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
