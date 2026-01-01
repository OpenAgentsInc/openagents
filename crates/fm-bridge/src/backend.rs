use async_trait::async_trait;
use local_inference::{
    CompletionRequest, CompletionResponse, LocalModelBackend, LocalModelError, ModelInfo, Result,
    StreamChunk,
};
use tokio::sync::mpsc;

use crate::{CompletionOptions, FMClient};

/// LocalModelBackend implementation for FMClient
#[async_trait]
impl LocalModelBackend for FMClient {
    async fn initialize(&mut self) -> Result<()> {
        // Verify the server is reachable
        let healthy = self
            .health()
            .await
            .map_err(|e| LocalModelError::InitializationError(e.to_string()))?;
        if !healthy {
            return Err(LocalModelError::InitializationError(
                "FM-bridge health check failed".to_string(),
            ));
        }
        self.mark_ready(true);
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let models = self
            .models()
            .await
            .map_err(|e| LocalModelError::BackendError(e.to_string()))?;

        Ok(models
            .into_iter()
            .map(|m| {
                let mut info = ModelInfo::new(m.id.clone(), m.id.clone(), 8192);
                info = info.with_description(m.owned_by);
                info
            })
            .collect())
    }

    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| LocalModelError::ModelNotFound(model_id.to_string()))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let options = CompletionOptions {
            model: Some(request.model.clone()),
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|v| v as u32),
            top_p: request.top_p,
            stop: request.stop,
        };

        let response = self
            .complete(request.prompt, Some(options))
            .await
            .map_err(|e| LocalModelError::InferenceError(e.to_string()))?;

        let text = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(CompletionResponse {
            id: response.id,
            model: response.model,
            text,
            finish_reason: response
                .choices
                .first()
                .and_then(|c| c.finish_reason.as_ref().map(|f| format!("{:?}", f))),
            usage: response.usage.map(|u| local_inference::UsageInfo {
                prompt_tokens: u.prompt_tokens.unwrap_or(0) as usize,
                completion_tokens: u.completion_tokens.unwrap_or(0) as usize,
                total_tokens: u.total_tokens.unwrap_or(0) as usize,
            }),
            extra: std::collections::HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let options = CompletionOptions {
            model: Some(request.model.clone()),
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|v| v as u32),
            top_p: request.top_p,
            stop: request.stop,
        };

        let client = self.clone();
        let prompt = request.prompt.clone();
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let stream = match client.stream(prompt, Some(options)).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx
                        .send(Err(LocalModelError::StreamError(e.to_string())))
                        .await;
                    return;
                }
            };

            use tokio_stream::StreamExt;
            let mut stream = Box::pin(stream);

            while let Some(result) = stream.next().await {
                let chunk = match result {
                    Ok(fm_chunk) => Ok(StreamChunk {
                        id: String::new(),
                        model: String::new(),
                        delta: fm_chunk.text,
                        finish_reason: fm_chunk.finish_reason.map(|f| format!("{:?}", f)),
                        extra: std::collections::HashMap::new(),
                    }),
                    Err(e) => Err(LocalModelError::StreamError(e.to_string())),
                };

                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }

    async fn is_ready(&self) -> bool {
        if !self.is_ready_flag() {
            return false;
        }
        self.health().await.unwrap_or(false)
    }

    async fn shutdown(&mut self) -> Result<()> {
        // No cleanup needed for HTTP client
        self.mark_ready(false);
        Ok(())
    }
}
