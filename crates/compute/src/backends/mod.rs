//! Backends for the compute provider
//!
//! Two categories of backends:
//!
//! ## Inference Backends
//! Simple prompt → completion for NIP-90 text generation:
//! - Ollama (localhost:11434)
//! - Apple Foundation Models (localhost:11435)
//! - Llama.cpp / GPT-OSS (localhost:8080)
//! - GPT-OSS Metal (local model.bin, macOS)
//!
//! ## Agent Backends
//! Complex agentic tasks for Bazaar jobs (NIP-90 kinds 5930-5933):
//! - Codex CLI
//! - Future: SWE-agent, Aider, etc.

pub mod agent;
mod apple_fm;
#[cfg(all(feature = "gpt-oss-metal", target_os = "macos"))]
mod gpt_oss_metal;
mod llamacpp;
mod ollama;

pub use agent::{
    AgentBackend, AgentCapabilities, AgentError, AgentRegistry, AgentBackendStatus, JobProgress,
};
pub use apple_fm::{AppleFmBackend, FmSession, FmToolDefinition, FmTranscriptMessage};
#[cfg(all(feature = "gpt-oss-metal", target_os = "macos"))]
pub use gpt_oss_metal::GptOssMetalBackend;
pub use llamacpp::LlamaCppBackend;
pub use ollama::OllamaBackend;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, mpsc};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("Inference failed: {0}")]
    InferenceError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("JSON error: {0}")]
    JsonError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Backend unavailable: {0}")]
    Unavailable(String),
}

impl From<reqwest::Error> for BackendError {
    fn from(err: reqwest::Error) -> Self {
        BackendError::HttpError(err.to_string())
    }
}

impl From<serde_json::Error> for BackendError {
    fn from(err: serde_json::Error) -> Self {
        BackendError::JsonError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BackendError>;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl CompletionRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            extra: HashMap::new(),
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    pub text: String,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageInfo>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub model: String,
    pub delta: String,
    pub finish_reason: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_length: usize,
    pub capabilities: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>, context_length: usize) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            context_length,
            capabilities: Vec::new(),
            extra: HashMap::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// ============================================================================
// Backend Trait
// ============================================================================

/// Core trait that all inference backends must implement
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Backend identifier (e.g., "ollama", "apple_fm", "llamacpp")
    fn id(&self) -> &str;

    /// Check if the backend is ready to serve requests
    async fn is_ready(&self) -> bool;

    /// List all available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Execute a completion request
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Execute a streaming completion request
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>>;

    /// Initialize the backend (optional setup)
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Shutdown the backend (optional cleanup)
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Backend Registry
// ============================================================================

/// Registry of available inference backends with auto-detection
pub struct BackendRegistry {
    backends: HashMap<String, Arc<RwLock<dyn InferenceBackend>>>,
    default_backend: Option<String>,
}

impl BackendRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: None,
        }
    }

    /// Probe localhost for available backends and register them
    pub async fn detect() -> Self {
        let mut registry = Self::new();

        // Try GPT-OSS Metal (macOS, local model.bin)
        #[cfg(all(feature = "gpt-oss-metal", target_os = "macos"))]
        match GptOssMetalBackend::from_env() {
            Ok(backend) => {
                if backend.is_ready().await {
                    tracing::info!("Detected GPT-OSS Metal backend");
                    registry.register_with_id("gpt-oss-metal", Arc::new(RwLock::new(backend)));
                }
            }
            Err(e) => {
                tracing::debug!("GPT-OSS Metal not available: {}", e);
            }
        }

        // Try Ollama at :11434
        match OllamaBackend::new("http://localhost:11434") {
            Ok(backend) => {
                if backend.is_ready().await {
                    tracing::info!("Detected Ollama backend at localhost:11434");
                    registry.register_with_id("ollama", Arc::new(RwLock::new(backend)));
                }
            }
            Err(e) => {
                tracing::debug!("Ollama not available: {}", e);
            }
        }

        // Try Apple FM at :11435
        match AppleFmBackend::new("http://localhost:11435") {
            Ok(backend) => {
                if backend.is_ready().await {
                    tracing::info!("Detected Apple FM backend at localhost:11435");
                    registry.register_with_id("apple_fm", Arc::new(RwLock::new(backend)));
                }
            }
            Err(e) => {
                tracing::debug!("Apple FM not available: {}", e);
            }
        }

        // Try Llama.cpp at :8080
        match LlamaCppBackend::new("http://localhost:8080") {
            Ok(backend) => {
                if backend.is_ready().await {
                    tracing::info!("Detected Llama.cpp backend at localhost:8080");
                    registry.register_with_id("llamacpp", Arc::new(RwLock::new(backend)));
                }
            }
            Err(e) => {
                tracing::debug!("Llama.cpp not available: {}", e);
            }
        }

        registry
    }

    /// Register a backend with the given ID
    pub fn register_with_id(&mut self, id: &str, backend: Arc<RwLock<dyn InferenceBackend>>) {
        if self.default_backend.is_none() {
            self.default_backend = Some(id.to_string());
        }
        self.backends.insert(id.to_string(), backend);
    }

    /// Get a backend by ID
    pub fn get(&self, id: &str) -> Option<Arc<RwLock<dyn InferenceBackend>>> {
        self.backends.get(id).cloned()
    }

    /// Get the default backend
    pub fn default(&self) -> Option<Arc<RwLock<dyn InferenceBackend>>> {
        self.default_backend
            .as_ref()
            .and_then(|id| self.backends.get(id).cloned())
    }

    /// Get the default backend ID
    pub fn default_id(&self) -> Option<&str> {
        self.default_backend.as_deref()
    }

    /// Set the default backend
    pub fn set_default(&mut self, id: &str) -> bool {
        if self.backends.contains_key(id) {
            self.default_backend = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// List all available backend IDs
    pub fn available_backends(&self) -> Vec<&str> {
        self.backends.keys().map(|s| s.as_str()).collect()
    }

    /// Check if any backends are available
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }

    /// List all models from all backends
    pub async fn list_all_models(&self) -> Vec<(String, ModelInfo)> {
        let mut all_models = Vec::new();

        for (backend_id, backend) in &self.backends {
            let backend = backend.read().await;
            if let Ok(models) = backend.list_models().await {
                for model in models {
                    all_models.push((backend_id.clone(), model));
                }
            }
        }

        all_models
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}
