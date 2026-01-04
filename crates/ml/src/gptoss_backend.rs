use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use compute::backends::{
    BackendError, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo,
    Result as BackendResult, StreamChunk, UsageInfo,
};

use crate::{
    telemetry::{telemetry_timestamp_ms, InferenceTelemetry, ModelLifecycleTelemetry, StageStatus},
    GptOssEngine, GptOssEngineConfig, GptOssTokenEvent, MlError,
};

#[derive(Debug, Clone)]
struct GptOssDefaults {
    max_new_tokens: Option<usize>,
    layer_limit: Option<usize>,
    max_kv: Option<usize>,
    moe_fallback: bool,
    use_harmony_prompt: bool,
}

/// GPT-OSS GGUF backend (CPU, cross-platform).
pub struct GptOssGgufBackend {
    engine: Arc<Mutex<GptOssEngine>>,
    model_id: String,
    context_length: usize,
    defaults: GptOssDefaults,
}

impl GptOssGgufBackend {
    pub fn new(path: PathBuf, model_id: Option<String>) -> BackendResult<Self> {
        eprintln!("[gpt-oss] Loading model from {:?}...", path);
        let engine = GptOssEngine::load(&path).map_err(map_ml_error)?;
        eprintln!("[gpt-oss] Model loaded successfully");
        let resolved_id = model_id.unwrap_or_else(|| engine.model_id().to_string());
        let context_length = engine.context_length();
        let defaults = GptOssDefaults {
            max_new_tokens: None,
            layer_limit: None,
            max_kv: None,
            moe_fallback: false,
            use_harmony_prompt: true,
        };

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            model_id: resolved_id,
            context_length,
            defaults,
        })
    }

    pub fn from_env() -> BackendResult<Self> {
        let path = std::env::var("GPT_OSS_GGUF_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(default_gguf_path)
            .ok_or_else(|| {
                BackendError::InitializationError("GPT_OSS_GGUF_PATH not set".to_string())
            })?;
        let model_id = std::env::var("GPT_OSS_GGUF_MODEL_ID").ok();
        let mut backend = Self::new(path, model_id)?;

        backend.defaults.max_new_tokens = parse_env_usize("GPT_OSS_GGUF_MAX_TOKENS");
        backend.defaults.layer_limit = parse_env_usize("GPT_OSS_GGUF_LAYER_LIMIT");
        backend.defaults.max_kv = parse_env_usize("GPT_OSS_GGUF_MAX_KV")
            .or_else(|| parse_env_usize("GPT_OSS_GGUF_CONTEXT_LENGTH"));
        backend.defaults.moe_fallback =
            parse_env_bool("GPT_OSS_GGUF_MOE_FALLBACK").unwrap_or(false);
        backend.defaults.use_harmony_prompt =
            parse_env_bool("GPT_OSS_GGUF_HARMONY").unwrap_or(true);

        if let Some(context_length) = parse_env_usize("GPT_OSS_GGUF_CONTEXT_LENGTH") {
            backend.context_length = context_length;
        }

        Ok(backend)
    }

    fn ensure_model(&self, request: &CompletionRequest) -> BackendResult<()> {
        if request.model.is_empty() || request.model == self.model_id {
            return Ok(());
        }

        Err(BackendError::ModelNotFound(request.model.clone()))
    }

    fn build_engine_config(&self, request: &CompletionRequest) -> GptOssEngineConfig {
        let mut config = GptOssEngineConfig::default();
        config.use_harmony_prompt = self.defaults.use_harmony_prompt;
        config.layer_limit = self.defaults.layer_limit;
        config.max_kv = self.defaults.max_kv;
        config.moe_fallback = self.defaults.moe_fallback;
        if let Some(max_tokens) = self.defaults.max_new_tokens {
            config.generation.max_new_tokens = max_tokens;
        }

        if let Some(max_tokens) = request.max_tokens {
            config.generation.max_new_tokens = max_tokens;
        }
        if let Some(temp) = request.temperature {
            config.generation.temperature = temp;
        }
        if let Some(top_p) = request.top_p {
            config.generation.top_p = top_p;
        }

        if let Some(value) = request.extra.get("top_k").and_then(|v| v.as_u64()) {
            config.generation.top_k = value as usize;
        }
        if let Some(value) = request
            .extra
            .get("repetition_penalty")
            .and_then(|v| v.as_f64())
        {
            config.generation.repetition_penalty = value as f32;
        }
        if let Some(value) = request.extra.get("seed").and_then(|v| v.as_u64()) {
            config.generation.seed = Some(value);
        }

        if let Some(value) = request.extra.get("layers").and_then(|v| v.as_u64()) {
            config.layer_limit = Some(value as usize);
        }
        if let Some(value) = request.extra.get("max_kv").and_then(|v| v.as_u64()) {
            config.max_kv = Some(value as usize);
        }
        if let Some(value) = request.extra.get("moe_fallback").and_then(|v| v.as_bool()) {
            config.moe_fallback = value;
        }
        if let Some(value) = request.extra.get("harmony").and_then(|v| v.as_bool()) {
            config.use_harmony_prompt = value;
        }
        if let Some(value) = request.extra.get("telemetry_top_k").and_then(|v| v.as_u64()) {
            config.telemetry_top_k = value as usize;
        }
        if let Some(sample) = request.extra.get("sample").and_then(|v| v.as_bool()) {
            if !sample {
                config.generation.temperature = 0.0;
                config.generation.top_k = 1;
                config.generation.top_p = 1.0;
            }
        }

        config
    }
}

#[async_trait]
impl InferenceBackend for GptOssGgufBackend {
    fn id(&self) -> &str {
        "gpt-oss-gguf"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> BackendResult<Vec<ModelInfo>> {
        let mut info = ModelInfo::new(&self.model_id, &self.model_id, self.context_length);
        info.description = Some("gpt-oss gguf".to_string());
        Ok(vec![info])
    }

    async fn complete(&self, request: CompletionRequest) -> BackendResult<CompletionResponse> {
        self.ensure_model(&request)?;

        let engine = Arc::clone(&self.engine);
        let mut config = self.build_engine_config(&request);
        let stop = request.stop.clone();
        let prompt = request.prompt.clone();
        let model_id = self.model_id.clone();

        eprintln!("[gpt-oss] Starting inference...");
        let completion = tokio::task::spawn_blocking(move || {
            let mut engine = engine.lock().map_err(|_| {
                BackendError::InferenceError("gpt-oss engine lock poisoned".to_string())
            })?;

            if let Some(tokens) = stop {
                for token in tokens {
                    if let Some(id) = engine.token_id(&token) {
                        config.generation.stop_tokens.push(id);
                    }
                }
            }

            engine
                .generate_with_callback(&prompt, &config, None, None)
                .map_err(map_ml_error)
        })
        .await
        .map_err(|err| BackendError::InferenceError(err.to_string()))??;

        Ok(CompletionResponse {
            id: "gpt-oss-gguf".to_string(),
            model: model_id,
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
    ) -> BackendResult<mpsc::Receiver<BackendResult<StreamChunk>>> {
        self.ensure_model(&request)?;

        let (tx, rx) = mpsc::channel(100);
        let engine = Arc::clone(&self.engine);
        let mut config = self.build_engine_config(&request);
        let stop = request.stop.clone();
        let prompt = request.prompt.clone();
        let model_id = self.model_id.clone();

        eprintln!("[gpt-oss] Starting streaming inference...");
        tokio::task::spawn_blocking(move || {
            eprintln!("[gpt-oss] Inside spawn_blocking, acquiring lock...");
            let mut engine = match engine.lock() {
                Ok(engine) => {
                    eprintln!("[gpt-oss] Lock acquired");
                    engine
                }
                Err(_) => {
                    eprintln!("[gpt-oss] Lock poisoned!");
                    let _ = tx.blocking_send(Err(BackendError::InferenceError(
                        "gpt-oss engine lock poisoned".to_string(),
                    )));
                    return;
                }
            };

            let send_telemetry = |telemetry: ModelLifecycleTelemetry| -> Result<(), MlError> {
                let mut extra = HashMap::new();
                if let Ok(value) = serde_json::to_value(telemetry) {
                    extra.insert("telemetry".to_string(), value);
                }
                let chunk = StreamChunk {
                    id: String::new(),
                    model: model_id.clone(),
                    delta: String::new(),
                    finish_reason: None,
                    extra,
                };
                tx.blocking_send(Ok(chunk))
                    .map_err(|_| MlError::Model("stream closed".to_string()))?;
                Ok(())
            };

            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "load_start".to_string(),
                status: StageStatus::Started,
                detail: Some("source=pylon".to_string()),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "load_complete".to_string(),
                status: StageStatus::Completed,
                detail: Some("backend=gpt-oss-gguf".to_string()),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });

            let model_config = engine.model_config();
            let active_layers = config
                .layer_limit
                .unwrap_or(model_config.block_count as usize)
                .max(1);
            let runtime_detail = format!(
                "cpu_fallback=pylon sample={} layers={active_layers}",
                if config.generation.temperature <= 0.0 {
                    "off"
                } else {
                    "on"
                }
            );
            let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                stage: "runtime_mode".to_string(),
                status: StageStatus::Completed,
                step: None,
                total_steps: None,
                detail: Some(runtime_detail),
                ts_ms: telemetry_timestamp_ms(),
            });
            let model_detail = format!(
                "blocks={} heads={}",
                model_config.block_count, model_config.head_count
            );
            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "model_config".to_string(),
                status: StageStatus::Completed,
                detail: Some(model_detail),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let max_kv = config
                .max_kv
                .unwrap_or_else(|| model_config.context_length as usize)
                .max(1);
            let max_new = config.generation.max_new_tokens.max(1);
            let max_prompt = max_kv.saturating_sub(max_new);
            let token_limits = format!("kv={max_kv} prompt={max_prompt} new={max_new}");
            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "token_limits".to_string(),
                status: StageStatus::Completed,
                detail: Some(token_limits),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let moe_detail = if config.moe_fallback {
                "fallback expert=0".to_string()
            } else {
                "gguf".to_string()
            };
            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "moe_mode".to_string(),
                status: StageStatus::Completed,
                detail: Some(moe_detail),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let vocab = engine.tokenizer().vocab();
            let token_detail = format!(
                "vocab={} merges={} model={} pre={} template={}b bos={} eos={} pad={}",
                vocab.tokens.len(),
                vocab.merges.len(),
                vocab.model.as_deref().unwrap_or("-"),
                vocab.pre.as_deref().unwrap_or("-"),
                vocab.chat_template.as_ref().map(|value| value.len()).unwrap_or(0),
                vocab
                    .bos_token_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                vocab
                    .eos_token_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                vocab
                    .pad_token_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            );
            let _ = send_telemetry(ModelLifecycleTelemetry::LoadStage {
                stage: "tokenizer_load".to_string(),
                status: StageStatus::Completed,
                detail: Some(token_detail),
                bytes: None,
                total_bytes: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                stage: "prompt_encode".to_string(),
                status: StageStatus::Started,
                step: None,
                total_steps: None,
                detail: Some("format=harmony".to_string()),
                ts_ms: telemetry_timestamp_ms(),
            });

            if let Some(tokens) = stop {
                for token in tokens {
                    if let Some(id) = engine.token_id(&token) {
                        config.generation.stop_tokens.push(id);
                    }
                }
            }

            let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                stage: "prompt_encode".to_string(),
                status: StageStatus::Completed,
                step: None,
                total_steps: None,
                detail: None,
                ts_ms: telemetry_timestamp_ms(),
            });
            let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                stage: "prefill".to_string(),
                status: StageStatus::Started,
                step: None,
                total_steps: None,
                detail: None,
                ts_ms: telemetry_timestamp_ms(),
            });

            let mut decode_started = false;
            let mut prefill_completed = false;
            let mut callback = |event: &GptOssTokenEvent| {
                if !decode_started {
                    decode_started = true;
                    if !prefill_completed {
                        prefill_completed = true;
                        let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                            stage: "prefill".to_string(),
                            status: StageStatus::Completed,
                            step: None,
                            total_steps: None,
                            detail: None,
                            ts_ms: telemetry_timestamp_ms(),
                        });
                    }
                    let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                        stage: "decode".to_string(),
                        status: StageStatus::Started,
                        step: None,
                        total_steps: None,
                        detail: None,
                        ts_ms: telemetry_timestamp_ms(),
                    });
                }

                if event.token_text.is_empty() {
                    return Ok(());
                }

                let mut extra = HashMap::new();
                extra.insert(
                    "token_id".to_string(),
                    serde_json::Value::from(event.token_id as u64),
                );
                extra.insert(
                    "entropy".to_string(),
                    serde_json::Value::from(event.entropy),
                );
                extra.insert(
                    "tokens_per_sec".to_string(),
                    serde_json::Value::from(event.tokens_per_sec),
                );
                if !event.top_k.is_empty() {
                    let top_k = event
                        .top_k
                        .iter()
                        .map(|candidate| {
                            serde_json::json!({
                                "token_id": candidate.token_id,
                                "token_text": candidate.token_text,
                                "probability": candidate.probability,
                            })
                        })
                        .collect::<Vec<_>>();
                    extra.insert("top_k".to_string(), serde_json::Value::from(top_k));
                }

                let telemetry = ModelLifecycleTelemetry::InferenceEvent {
                    event: InferenceTelemetry::TokenGenerated {
                        token_id: event.token_id,
                        token_text: event.token_text.clone(),
                        top_k: event.top_k.clone(),
                        entropy: event.entropy,
                        tokens_per_sec: event.tokens_per_sec,
                    },
                    ts_ms: telemetry_timestamp_ms(),
                };
                if let Ok(value) = serde_json::to_value(telemetry) {
                    extra.insert("telemetry".to_string(), value);
                }

                let chunk = StreamChunk {
                    id: String::new(),
                    model: model_id.clone(),
                    delta: event.token_text.clone(),
                    finish_reason: None,
                    extra,
                };
                if tx.blocking_send(Ok(chunk)).is_err() {
                    return Err(MlError::Model("stream closed".to_string()));
                }
                Ok(())
            };

            eprintln!("[gpt-oss] Starting generate_with_callback...");
            let result = engine.generate_with_callback(&prompt, &config, Some(&mut callback), None);
            eprintln!("[gpt-oss] generate_with_callback returned");

            match result {
                Ok(completion) => {
                    eprintln!("[gpt-oss] Generation complete: {} tokens, reason={}",
                        completion.completion_tokens, completion.finish_reason);
                    if !prefill_completed {
                        let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                            stage: "prefill".to_string(),
                            status: StageStatus::Completed,
                            step: None,
                            total_steps: None,
                            detail: None,
                            ts_ms: telemetry_timestamp_ms(),
                        });
                    }
                    let _ = send_telemetry(ModelLifecycleTelemetry::InferenceStage {
                        stage: "decode".to_string(),
                        status: StageStatus::Completed,
                        step: None,
                        total_steps: None,
                        detail: Some(completion.finish_reason.clone()),
                        ts_ms: telemetry_timestamp_ms(),
                    });
                    let _ = tx.blocking_send(Ok(StreamChunk {
                        id: String::new(),
                        model: model_id,
                        delta: String::new(),
                        finish_reason: Some(completion.finish_reason),
                        extra: HashMap::new(),
                    }));
                }
                Err(err) => {
                    eprintln!("[gpt-oss] Generation error: {:?}", err);
                    let _ = tx.blocking_send(Err(map_ml_error(err)));
                }
            }
        });

        Ok(rx)
    }
}

fn map_ml_error(err: MlError) -> BackendError {
    match err {
        MlError::InvalidConfig(message) => BackendError::InvalidRequest(message),
        MlError::Model(message) => BackendError::InferenceError(message),
        MlError::Tokenizer(message) => BackendError::InferenceError(message),
        MlError::Network(message) => BackendError::InferenceError(message),
        MlError::Serialization(message) => BackendError::JsonError(message),
        MlError::Device(message) => BackendError::InferenceError(message),
        MlError::Nostr(message) => BackendError::InferenceError(message),
    }
}

fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok().and_then(|v| v.parse::<usize>().ok())
}

fn parse_env_bool(key: &str) -> Option<bool> {
    std::env::var(key).ok().and_then(|value| {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn default_gguf_path() -> Option<PathBuf> {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("gpt-oss-20b")
        .join("gpt-oss-20b-Q8_0.gguf");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}
