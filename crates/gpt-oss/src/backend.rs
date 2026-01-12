use async_trait::async_trait;
use local_inference::{
    CompletionRequest, CompletionResponse, LocalModelBackend, LocalModelError, ModelInfo, Result,
    StreamChunk,
};
use tokio::sync::mpsc;

use crate::{GptOssClient, GptOssRequest};

/// LocalModelBackend implementation for GptOssClient
#[async_trait]
impl LocalModelBackend for GptOssClient {
    async fn initialize(&mut self) -> Result<()> {
        // Verify the server is reachable
        let healthy = self
            .health()
            .await
            .map_err(|e| LocalModelError::InitializationError(e.to_string()))?;
        if !healthy {
            self.set_initialized(false);
            return Err(LocalModelError::InitializationError(
                "GPT-OSS server is not healthy".to_string(),
            ));
        }

        self.set_initialized(true);
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
                ModelInfo::new(m.id.clone(), m.display_name(), m.context_length)
                    .with_description(m.description.unwrap_or_default())
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
        let gpt_request = GptOssRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop,
            stream: false,
            json_schema: None,
        };

        let response = self
            .complete(gpt_request)
            .await
            .map_err(|e| LocalModelError::InferenceError(e.to_string()))?;

        Ok(CompletionResponse {
            id: response.id,
            model: response.model,
            text: response.text,
            finish_reason: response.finish_reason,
            usage: response.usage.map(|u| local_inference::UsageInfo {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            extra: std::collections::HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let gpt_request = GptOssRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop,
            stream: true,
            json_schema: None,
        };

        let client = self.clone();
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let stream = match client.stream(gpt_request).await {
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
                    Ok(gpt_chunk) => {
                        let delta = gpt_chunk.delta().to_string();
                        let finish_reason = gpt_chunk
                            .choices
                            .first()
                            .and_then(|c| c.finish_reason.clone());
                        Ok(StreamChunk {
                            id: gpt_chunk.id,
                            model: gpt_chunk.model,
                            delta,
                            finish_reason,
                            extra: std::collections::HashMap::new(),
                        })
                    }
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
        if !self.is_initialized() {
            return false;
        }

        self.health().await.unwrap_or(false)
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.set_initialized(false);
        Ok(())
    }
}
