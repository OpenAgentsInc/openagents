//! Ollama inference service (stubbed for cleanup).
//!
//! The full LLM integration will be wired back in once the desktop stack is rebuilt.

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
pub struct OllamaService {
    available: bool,
}

impl OllamaService {
    /// Create a new Ollama service
    pub fn new() -> Self {
        Self { available: false }
    }

    /// Check if Ollama is available
    pub async fn is_available(&self) -> bool {
        self.available
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>, OllamaError> {
        if self.available {
            Ok(Vec::new())
        } else {
            Err(OllamaError::NotAvailable(
                "Ollama integration is currently disabled".into(),
            ))
        }
    }

    /// Generate a completion (non-streaming)
    pub async fn generate(&self, model: &str, prompt: &str) -> Result<String, OllamaError> {
        let _ = (model, prompt);
        Err(OllamaError::NotAvailable(
            "Ollama integration is currently disabled".into(),
        ))
    }
}

impl Default for OllamaService {
    fn default() -> Self {
        Self::new()
    }
}
