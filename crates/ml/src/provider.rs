use crate::device::MlDevice;
use crate::error::{MlError, Result};
use crate::model::{GenerationOutcome, LoadedModel, ModelKind, ModelSource};
use crate::sampling::GenerationConfig;
use crate::telemetry::InferenceHook;
use async_trait::async_trait;
use compute::backends::{
    BackendError, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result as BackendResult,
    StreamChunk, UsageInfo,
};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct MlProviderConfig {
    pub models: Vec<ModelSource>,
}

impl MlProviderConfig {
    pub fn from_env() -> Result<Self> {
        if let Ok(path) = std::env::var("ML_MODELS_FILE") {
            let content = std::fs::read_to_string(path)?;
            let models: Vec<ModelSource> = serde_json::from_str(&content)?;
            return Ok(Self { models });
        }

        if let Ok(json) = std::env::var("ML_MODELS_JSON") {
            let models: Vec<ModelSource> = serde_json::from_str(&json)?;
            return Ok(Self { models });
        }

        if let Ok(weights) = std::env::var("ML_MODEL_PATH") {
            let id = std::env::var("ML_MODEL_ID").unwrap_or_else(|_| "ml-model".to_string());
            let kind = std::env::var("ML_MODEL_KIND").unwrap_or_else(|_| "llama2c-quantized".to_string());
            let tokenizer = std::env::var("ML_TOKENIZER_PATH").ok();
            let config = std::env::var("ML_MODEL_CONFIG").ok();

            let kind = match kind.as_str() {
                "llama2c" | "llama2c-quantized" | "llama2c-gguf" => ModelKind::Llama2CQuantized,
                "gemma3" | "gemma-3" => ModelKind::Gemma3,
                other => {
                    return Err(MlError::InvalidConfig(format!(
                        "unsupported ML_MODEL_KIND: {other}"
                    )))
                }
            };

            let weights = weights
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();

            let model = ModelSource {
                id,
                kind,
                weights,
                tokenizer,
                config,
            };

            return Ok(Self { models: vec![model] });
        }

        Err(MlError::InvalidConfig(
            "no model configuration provided".to_string(),
        ))
    }
}

#[derive(Clone)]
pub struct MlProvider {
    device: MlDevice,
    models: Arc<RwLock<HashMap<String, Arc<Mutex<LoadedModel>>>>>,
    telemetry_hook: Option<Arc<dyn InferenceHook>>,
}

impl MlProvider {
    pub async fn new(config: MlProviderConfig) -> Result<Self> {
        let device = MlDevice::best_available().await?;
        let models = Arc::new(RwLock::new(HashMap::new()));

        for source in config.models {
            let loaded = LoadedModel::load(&source, &device).await?;
            models
                .write()
                .insert(source.id.clone(), Arc::new(Mutex::new(loaded)));
        }

        Ok(Self {
            device,
            models,
            telemetry_hook: None,
        })
    }

    pub fn set_telemetry_hook(&mut self, hook: Option<Arc<dyn InferenceHook>>) {
        self.telemetry_hook = hook;
    }

    pub fn device(&self) -> &MlDevice {
        &self.device
    }

    pub fn available_models(&self) -> Vec<String> {
        self.models
            .read()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn model_info(model_id: &str, model: &LoadedModel) -> ModelInfo {
        let mut info = ModelInfo::new(model_id, model_id, model.max_seq_len);
        info.description = Some(format!("candle {:?}", model.kind));
        info
    }

    fn build_generation_config(
        request: &CompletionRequest,
        model: &LoadedModel,
    ) -> GenerationConfig {
        let mut config = GenerationConfig::default();
        if let Some(max_tokens) = request.max_tokens {
            config.max_new_tokens = max_tokens;
        }
        if let Some(temp) = request.temperature {
            config.temperature = temp;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }

        if let Some(value) = request.extra.get("top_k") {
            if let Some(v) = value.as_u64() {
                config.top_k = v as usize;
            }
        }

        if let Some(value) = request.extra.get("repetition_penalty") {
            if let Some(v) = value.as_f64() {
                config.repetition_penalty = v as f32;
            }
        }

        if let Some(value) = request.extra.get("seed") {
            if let Some(v) = value.as_u64() {
                config.seed = Some(v);
            }
        }

        let mut stop_tokens = Vec::new();
        if let Some(stop) = &request.stop {
            for token in stop {
                if let Some(id) = model.tokenizer.token_id(token) {
                    stop_tokens.push(id);
                }
            }
        }
        if !stop_tokens.is_empty() {
            config.stop_tokens = stop_tokens;
        }

        config
    }

    fn generate_sync(
        model: &Arc<Mutex<LoadedModel>>,
        request: &CompletionRequest,
        stream: bool,
        on_token: Option<&mut dyn FnMut(String)>,
        hook: Option<Arc<dyn InferenceHook>>,
    ) -> Result<(GenerationOutcome, GenerationConfig)> {
        let mut model = model.lock();
        let config = Self::build_generation_config(request, &model);
        let mut on_token = if stream { on_token } else { None };
        let outcome = model.generate_with_hook(
            &request.prompt,
            &config,
            &mut on_token,
            hook.as_deref(),
        )?;
        Ok((outcome, config))
    }
}

#[async_trait]
impl InferenceBackend for MlProvider {
    fn id(&self) -> &str {
        "ml-candle"
    }

    async fn is_ready(&self) -> bool {
        !self.models.read().is_empty()
    }

    async fn list_models(&self) -> BackendResult<Vec<ModelInfo>> {
        let models = self.models.read();
        Ok(models
            .iter()
            .map(|(id, model)| Self::model_info(id, &model.lock()))
            .collect())
    }

    async fn complete(&self, request: CompletionRequest) -> BackendResult<CompletionResponse> {
        let model = {
            let models = self.models.read();
            models
                .get(&request.model)
                .cloned()
                .ok_or_else(|| BackendError::ModelNotFound(request.model.clone()))?
        };

        let request_id = uuid::Uuid::new_v4().to_string();
        let request_for_task = request.clone();

        let hook = self.telemetry_hook.clone();
        let (outcome, _config) = tokio::task::spawn_blocking(move || {
            Self::generate_sync(&model, &request_for_task, false, None, hook)
        })
        .await
        .map_err(|e| BackendError::InferenceError(e.to_string()))?
        .map_err(|e| BackendError::InferenceError(e.to_string()))?;

        Ok(CompletionResponse {
            id: request_id,
            model: request.model,
            text: outcome.text,
            finish_reason: Some("stop".to_string()),
            usage: Some(UsageInfo {
                prompt_tokens: outcome.prompt_tokens,
                completion_tokens: outcome.generated_tokens,
                total_tokens: outcome.prompt_tokens + outcome.generated_tokens,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> BackendResult<mpsc::Receiver<BackendResult<StreamChunk>>> {
        let model = {
            let models = self.models.read();
            models
                .get(&request.model)
                .cloned()
                .ok_or_else(|| BackendError::ModelNotFound(request.model.clone()))?
        };

        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(64);
        let model_id = request.model.clone();
        let prompt = request.prompt.clone();
        let hook = self.telemetry_hook.clone();

        tokio::task::spawn_blocking(move || {
            let mut finish_reason = "stop".to_string();
            let mut sent_any = false;

            let mut on_token = |text: String| {
                let chunk = StreamChunk {
                    id: request_id.clone(),
                    model: model_id.clone(),
                    delta: text,
                    finish_reason: None,
                    extra: HashMap::new(),
                };
                sent_any = true;
                let _ = tx.blocking_send(Ok(chunk));
            };

            let request = CompletionRequest { prompt, ..request };
            let outcome = Self::generate_sync(&model, &request, true, Some(&mut on_token), hook);

            match outcome {
                Ok((result, config)) => {
                    if result.generated_tokens >= config.max_new_tokens {
                        finish_reason = "length".to_string();
                    }
                    let _ = tx.blocking_send(Ok(StreamChunk {
                        id: request_id.clone(),
                        model: model_id.clone(),
                        delta: String::new(),
                        finish_reason: Some(finish_reason),
                        extra: HashMap::new(),
                    }));
                }
                Err(err) => {
                    let _ = tx.blocking_send(Err(BackendError::InferenceError(err.to_string())));
                }
            }

            if !sent_any {
                let _ = tx.blocking_send(Ok(StreamChunk {
                    id: request_id.clone(),
                    model: model_id.clone(),
                    delta: String::new(),
                    finish_reason: Some("stop".to_string()),
                    extra: HashMap::new(),
                }));
            }
        });

        Ok(rx)
    }
}
