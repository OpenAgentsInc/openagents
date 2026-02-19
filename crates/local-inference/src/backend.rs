use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    CompletionRequest, CompletionResponse, LocalModelError, ModelInfo, Result, StreamChunk,
};

/// Core trait that all local model backends must implement.
///
/// This trait provides a unified interface for interacting with local inference engines
/// like fm-bridge and gpt-oss. Implementations handle model loading, inference execution,
/// and streaming responses.
#[async_trait]
pub trait LocalModelBackend: Send + Sync {
    /// Initialize the backend with the given configuration.
    ///
    /// This is called once during backend setup and should handle any expensive
    /// initialization like model loading, GPU setup, etc.
    async fn initialize(&mut self) -> Result<()>;

    /// List all available models that this backend can serve.
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Get information about a specific model.
    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo>;

    /// Execute a completion request and return the full response.
    ///
    /// For streaming requests, this will collect all chunks and return the complete response.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Execute a streaming completion request.
    ///
    /// Returns a channel receiver that will yield chunks as they are generated.
    /// The channel will be closed when the completion is finished or if an error occurs.
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>>;

    /// Check if the backend is ready to serve requests.
    ///
    /// Returns true if the backend has been initialized and is ready to handle completions.
    async fn is_ready(&self) -> bool;

    /// Gracefully shutdown the backend, cleaning up resources.
    async fn shutdown(&mut self) -> Result<()>;
}

/// Extension trait for common backend operations.
///
/// Provides convenience methods built on top of `LocalModelBackend`.
/// This trait is automatically implemented for all `LocalModelBackend` implementors.
#[allow(dead_code)] // Will be used once backends are integrated
#[async_trait]
pub trait LocalModelBackendExt: LocalModelBackend {
    /// Execute a simple completion with default parameters.
    async fn complete_simple(&self, model: &str, prompt: &str) -> Result<String> {
        let request = CompletionRequest::new(model, prompt);
        let response = self.complete(request).await?;
        Ok(response.text)
    }

    /// Check if a specific model is available.
    async fn has_model(&self, model_id: &str) -> Result<bool> {
        match self.get_model_info(model_id).await {
            Ok(_) => Ok(true),
            Err(LocalModelError::ModelNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

// Blanket implementation for all LocalModelBackend implementors
impl<T: LocalModelBackend + ?Sized> LocalModelBackendExt for T {}
