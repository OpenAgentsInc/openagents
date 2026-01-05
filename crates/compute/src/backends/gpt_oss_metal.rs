use async_trait::async_trait;
use gpt_oss_metal::{GptOssMetalConfig, GptOssMetalEngine, GptOssMetalError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{
    BackendError, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result,
    StreamChunk, UsageInfo,
};

/// GPT-OSS Metal backend (macOS only, local model.bin inference).
pub struct GptOssMetalBackend {
    engine: Arc<GptOssMetalEngine>,
    model_id: String,
    context_length: usize,
}

impl GptOssMetalBackend {
    pub fn new(config: GptOssMetalConfig) -> Result<Self> {
        let engine = GptOssMetalEngine::new(config)
            .map_err(|err| BackendError::InitializationError(err.to_string()))?;
        let model_id = engine.model_id().to_string();
        let context_length = engine.context_length();

        Ok(Self {
            engine: Arc::new(engine),
            model_id,
            context_length,
        })
    }

    pub fn from_env() -> Result<Self> {
        let config = GptOssMetalConfig::from_env()
            .map_err(|err| BackendError::InitializationError(err.to_string()))?;
        Self::new(config)
    }

    fn ensure_model(&self, request: &CompletionRequest) -> Result<()> {
        if request.model.is_empty() || request.model == self.model_id {
            return Ok(());
        }

        Err(BackendError::ModelNotFound(request.model.clone()))
    }
}

#[async_trait]
impl InferenceBackend for GptOssMetalBackend {
    fn id(&self) -> &str {
        "gpt-oss-metal"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut info = ModelInfo::new(&self.model_id, &self.model_id, self.context_length);
        info.description = Some("gpt-oss metal".to_string());
        Ok(vec![info])
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.ensure_model(&request)?;

        let engine = Arc::clone(&self.engine);
        let prompt = request.prompt.clone();
        let max_tokens = request.max_tokens;
        let temperature = request.temperature;
        let stop = request.stop.clone();
        let use_harmony_prompt = request
            .extra
            .get("harmony")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);

        let completion = tokio::task::spawn_blocking(move || {
            engine.generate_with_callback(
                &prompt,
                max_tokens,
                temperature,
                stop.as_deref(),
                use_harmony_prompt,
                |_| Ok(()),
            )
        })
        .await
        .map_err(|err| BackendError::InferenceError(err.to_string()))?
        .map_err(|err| BackendError::InferenceError(err.to_string()))?;

        Ok(CompletionResponse {
            id: "gpt-oss-metal".to_string(),
            model: self.model_id.clone(),
            text: completion.text,
            finish_reason: Some(completion.finish_reason),
            usage: Some(UsageInfo {
                prompt_tokens: completion.prompt_tokens,
                completion_tokens: completion.completion_tokens,
                total_tokens: completion.prompt_tokens + completion.completion_tokens,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        self.ensure_model(&request)?;

        let (tx, rx) = mpsc::channel(100);
        let engine = Arc::clone(&self.engine);
        let prompt = request.prompt.clone();
        let max_tokens = request.max_tokens;
        let temperature = request.temperature;
        let stop = request.stop.clone();
        let model_id = self.model_id.clone();
        let use_harmony_prompt = request
            .extra
            .get("harmony")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);

        tokio::task::spawn_blocking(move || {
            let tx_callback = tx.clone();
            let result = engine.generate_with_callback(
                &prompt,
                max_tokens,
                temperature,
                stop.as_deref(),
                use_harmony_prompt,
                |delta| {
                    if delta.is_empty() {
                        return Ok(());
                    }

                    let chunk = StreamChunk {
                        id: String::new(),
                        model: model_id.clone(),
                        delta: delta.to_string(),
                        finish_reason: None,
                        extra: HashMap::new(),
                    };
                    tx_callback
                        .blocking_send(Ok(chunk))
                        .map_err(|_| {
                            GptOssMetalError::FfiError("stream closed".to_string())
                        })?;
                    Ok(())
                },
            );

            match result {
                Ok(completion) => {
                    let _ = tx.blocking_send(Ok(StreamChunk {
                        id: String::new(),
                        model: model_id,
                        delta: String::new(),
                        finish_reason: Some(completion.finish_reason),
                        extra: HashMap::new(),
                    }));
                }
                Err(err) => {
                    let _ = tx.blocking_send(Err(BackendError::InferenceError(err.to_string())));
                }
            }
        });

        Ok(rx)
    }
}
