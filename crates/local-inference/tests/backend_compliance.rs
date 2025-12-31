use async_trait::async_trait;
use local_inference::{
    CompletionRequest, CompletionResponse, LocalModelBackend, LocalModelError, ModelInfo, Result,
    StreamChunk, UsageInfo,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Mock backend for testing LocalModelBackend trait compliance.
///
/// This mock implementation allows us to:
/// 1. Verify the trait contract is correctly defined
/// 2. Test error handling paths
/// 3. Validate streaming behavior
/// 4. Ensure all methods can be properly implemented
#[derive(Clone)]
struct MockBackend {
    initialized: Arc<Mutex<bool>>,
    models: Vec<ModelInfo>,
    should_fail: Arc<Mutex<HashMap<String, LocalModelError>>>,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            initialized: Arc::new(Mutex::new(false)),
            models: vec![
                ModelInfo::new("mock-model-1", "Mock Model 1", 2048)
                    .with_description("Test model 1"),
                ModelInfo::new("mock-model-2", "Mock Model 2", 4096)
                    .with_description("Test model 2"),
            ],
            should_fail: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Configure the mock to fail on the next call to a specific method
    async fn set_failure(&self, method: &str, error: LocalModelError) {
        self.should_fail
            .lock()
            .await
            .insert(method.to_string(), error);
    }

    async fn check_failure(&self, method: &str) -> Result<()> {
        if let Some(error) = self.should_fail.lock().await.remove(method) {
            return Err(error);
        }
        Ok(())
    }
}

#[async_trait]
impl LocalModelBackend for MockBackend {
    async fn initialize(&mut self) -> Result<()> {
        self.check_failure("initialize").await?;
        *self.initialized.lock().await = true;
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.check_failure("list_models").await?;
        Ok(self.models.clone())
    }

    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo> {
        self.check_failure("get_model_info").await?;
        self.models
            .iter()
            .find(|m| m.id == model_id)
            .cloned()
            .ok_or_else(|| LocalModelError::ModelNotFound(model_id.to_string()))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.check_failure("complete").await?;

        if !*self.initialized.lock().await {
            return Err(LocalModelError::BackendError(
                "Backend not initialized".to_string(),
            ));
        }

        Ok(CompletionResponse {
            id: "test-completion-1".to_string(),
            model: request.model.clone(),
            text: format!("Mock response to: {}", request.prompt),
            finish_reason: Some("stop".to_string()),
            usage: Some(UsageInfo {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        self.check_failure("complete_stream").await?;

        if !*self.initialized.lock().await {
            return Err(LocalModelError::BackendError(
                "Backend not initialized".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(100);
        let response_text = format!("Mock response to: {}", request.prompt);
        let model = request.model.clone();

        tokio::spawn(async move {
            // Send response as chunks
            for (i, word) in response_text.split_whitespace().enumerate() {
                let chunk = StreamChunk {
                    id: format!("chunk-{}", i),
                    model: model.clone(),
                    delta: format!("{} ", word),
                    finish_reason: None,
                    extra: HashMap::new(),
                };

                if tx.send(Ok(chunk)).await.is_err() {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            // Send final chunk with finish reason
            let final_chunk = StreamChunk {
                id: "final".to_string(),
                model: model.clone(),
                delta: String::new(),
                finish_reason: Some("stop".to_string()),
                extra: HashMap::new(),
            };

            let _ = tx.send(Ok(final_chunk)).await;
        });

        Ok(rx)
    }

    async fn is_ready(&self) -> bool {
        *self.initialized.lock().await
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.check_failure("shutdown").await?;
        *self.initialized.lock().await = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize() {
        let mut backend = MockBackend::new();

        assert!(
            !backend.is_ready().await,
            "Backend should not be ready before initialization"
        );

        backend
            .initialize()
            .await
            .expect("Initialize should succeed");

        assert!(
            backend.is_ready().await,
            "Backend should be ready after initialization"
        );
    }

    #[tokio::test]
    async fn test_initialize_failure() {
        let mut backend = MockBackend::new();
        backend
            .set_failure(
                "initialize",
                LocalModelError::InitializationError("Test error".to_string()),
            )
            .await;

        let result = backend.initialize().await;
        assert!(result.is_err(), "Initialize should fail when configured");
        assert!(
            !backend.is_ready().await,
            "Backend should not be ready after failed initialization"
        );
    }

    #[tokio::test]
    async fn test_list_models() {
        let backend = MockBackend::new();
        let models = backend
            .list_models()
            .await
            .expect("list_models should succeed");

        assert_eq!(models.len(), 2, "Should return 2 models");
        assert_eq!(models[0].id, "mock-model-1");
        assert_eq!(models[1].id, "mock-model-2");
    }

    #[tokio::test]
    async fn test_get_model_info_success() {
        let backend = MockBackend::new();
        let model = backend
            .get_model_info("mock-model-1")
            .await
            .expect("get_model_info should succeed");

        assert_eq!(model.id, "mock-model-1");
        assert_eq!(model.name, "Mock Model 1");
        assert_eq!(model.context_length, 2048);
    }

    #[tokio::test]
    async fn test_get_model_info_not_found() {
        let backend = MockBackend::new();
        let result = backend.get_model_info("nonexistent").await;

        assert!(result.is_err(), "Should fail for nonexistent model");
        match result.unwrap_err() {
            LocalModelError::ModelNotFound(id) => assert_eq!(id, "nonexistent"),
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_complete_success() {
        let mut backend = MockBackend::new();
        backend
            .initialize()
            .await
            .expect("Initialize should succeed");

        let request = CompletionRequest::new("mock-model-1", "Test prompt");
        let response = backend
            .complete(request)
            .await
            .expect("complete should succeed");

        assert_eq!(response.model, "mock-model-1");
        assert!(response.text.contains("Test prompt"));
        assert_eq!(response.finish_reason, Some("stop".to_string()));
        assert!(response.usage.is_some());

        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[tokio::test]
    async fn test_complete_not_initialized() {
        let backend = MockBackend::new();
        let request = CompletionRequest::new("mock-model-1", "Test prompt");
        let result = backend.complete(request).await;

        assert!(result.is_err(), "Should fail when not initialized");
    }

    #[tokio::test]
    async fn test_complete_stream_success() {
        let mut backend = MockBackend::new();
        backend
            .initialize()
            .await
            .expect("Initialize should succeed");

        let request = CompletionRequest::new("mock-model-1", "Test prompt");
        let mut rx = backend
            .complete_stream(request)
            .await
            .expect("complete_stream should succeed");

        let mut chunks = Vec::new();
        while let Some(result) = rx.recv().await {
            let chunk = result.expect("Chunk should be Ok");
            chunks.push(chunk);
        }

        assert!(!chunks.is_empty(), "Should receive chunks");

        // Last chunk should have finish_reason
        let last_chunk = chunks.last().unwrap();
        assert_eq!(last_chunk.finish_reason, Some("stop".to_string()));

        // Earlier chunks should have deltas
        for chunk in chunks.iter().take(chunks.len() - 1) {
            assert!(
                !chunk.delta.is_empty(),
                "Non-final chunks should have delta"
            );
        }
    }

    #[tokio::test]
    async fn test_complete_stream_not_initialized() {
        let backend = MockBackend::new();
        let request = CompletionRequest::new("mock-model-1", "Test prompt");
        let result = backend.complete_stream(request).await;

        assert!(result.is_err(), "Should fail when not initialized");
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut backend = MockBackend::new();
        backend
            .initialize()
            .await
            .expect("Initialize should succeed");

        assert!(backend.is_ready().await);

        backend.shutdown().await.expect("Shutdown should succeed");

        assert!(
            !backend.is_ready().await,
            "Backend should not be ready after shutdown"
        );
    }

    #[tokio::test]
    async fn test_backend_error_handling() {
        let mut backend = MockBackend::new();
        backend
            .set_failure(
                "complete",
                LocalModelError::InferenceError("Model overloaded".to_string()),
            )
            .await;

        backend
            .initialize()
            .await
            .expect("Initialize should succeed");

        let request = CompletionRequest::new("mock-model-1", "Test");
        let result = backend.complete(request).await;

        assert!(result.is_err(), "Should propagate inference error");
        match result.unwrap_err() {
            LocalModelError::InferenceError(msg) => assert_eq!(msg, "Model overloaded"),
            _ => panic!("Expected InferenceError"),
        }
    }

    #[tokio::test]
    async fn test_stream_error_handling() {
        let backend = MockBackend::new();
        backend
            .set_failure(
                "complete_stream",
                LocalModelError::StreamError("Connection lost".to_string()),
            )
            .await;

        let request = CompletionRequest::new("mock-model-1", "Test");
        let result = backend.complete_stream(request).await;

        assert!(result.is_err(), "Should fail with stream error");
        match result.unwrap_err() {
            LocalModelError::StreamError(msg) => assert_eq!(msg, "Connection lost"),
            _ => panic!("Expected StreamError"),
        }
    }
}
