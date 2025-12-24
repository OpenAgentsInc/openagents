//! Ollama inference service
//!
//! === BLOCKED: Ollama HTTP API integration required ===
//!
//! Per d-012 (No Stubs - Production-Ready Code Only), the Ollama service
//! integration is not yet implemented. All methods return explicit errors.
//!
//! When implementing:
//! 1. Add reqwest dependency for HTTP client
//! 2. Implement connection to Ollama API (default: http://localhost:11434)
//! 3. Add /api/tags endpoint for list_models()
//! 4. Add /api/generate endpoint for generate()
//! 5. Add streaming support for real-time completions
//! 6. Add proper error handling for connection failures
//!
//! Reference: https://github.com/ollama/ollama/blob/main/docs/api.md

use thiserror::Error;

/// Errors from the Ollama service
#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("Ollama integration not implemented. Requires HTTP API client for localhost:11434")]
    NotAvailable(String),

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),
}

/// Model metadata exposed by the Ollama service.
#[derive(Debug, Clone)]
pub struct OllamaModel {
    /// Model name (e.g., "llama3:8b")
    pub name: String,
    /// Model size (e.g., "4.7 GB")
    pub size: String,
    /// Quantization level (e.g., "Q4_0")
    pub quantization: Option<String>,
}

/// Service for interacting with Ollama
///
/// This service is a placeholder that returns errors until Ollama HTTP API
/// integration is implemented per the blocker comments above.
pub struct OllamaService;

impl OllamaService {
    /// Create a new Ollama service
    pub fn new() -> Self {
        Self
    }

    /// Check if Ollama is available
    ///
    /// Always returns false until Ollama integration is implemented.
    /// When implemented, this should check http://localhost:11434/api/tags
    pub async fn is_available(&self) -> bool {
        false
    }

    /// List available models
    ///
    /// Returns error until Ollama integration is implemented.
    /// When implemented, call GET http://localhost:11434/api/tags
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>, OllamaError> {
        Err(OllamaError::NotAvailable(
            "Ollama integration not implemented. Requires HTTP client for /api/tags endpoint.".into(),
        ))
    }

    /// Generate a completion (non-streaming)
    ///
    /// Returns error until Ollama integration is implemented.
    /// When implemented, call POST http://localhost:11434/api/generate
    pub async fn generate(&self, _model: &str, _prompt: &str) -> Result<String, OllamaError> {
        Err(OllamaError::NotAvailable(
            "Ollama integration not implemented. Requires HTTP client for /api/generate endpoint.".into(),
        ))
    }
}

impl Default for OllamaService {
    fn default() -> Self {
        Self::new()
    }
}
