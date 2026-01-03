use crate::device::MlDevice;
use crate::error::{MlError, Result};
use crate::http::fetch_bytes;
use crate::sampling::{apply_repetition_penalty, sample_from_logits, softmax, GenerationConfig};
use crate::telemetry::{
    telemetry_timestamp_ms, InferenceHook, InferenceTelemetry, ModelLifecycleHook,
    ModelLifecycleTelemetry, StageStatus, TokenCandidate,
};
use crate::tokenizer::Tokenizer;
use candle_core::{DType, IndexOp, Tensor};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
#[cfg(feature = "native")]
use std::path::PathBuf;
use web_time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelKind {
    Llama2CQuantized,
    Gemma3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSource {
    pub id: String,
    pub kind: ModelKind,
    pub weights: Vec<String>,
    pub tokenizer: Option<String>,
    pub config: Option<String>,
}

impl ModelSource {
    pub fn llama2c_gguf(
        id: impl Into<String>,
        weights: impl Into<String>,
        tokenizer: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ModelKind::Llama2CQuantized,
            weights: vec![weights.into()],
            tokenizer: Some(tokenizer.into()),
            config: None,
        }
    }

    pub fn gemma3_safetensors(
        id: impl Into<String>,
        weights: Vec<String>,
        tokenizer: impl Into<String>,
        config: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: ModelKind::Gemma3,
            weights,
            tokenizer: Some(tokenizer.into()),
            config: Some(config.into()),
        }
    }
}

#[derive(Debug)]
pub struct GenerationOutcome {
    pub text: String,
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
}

#[derive(Debug)]
pub struct LoadedModel {
    pub id: String,
    pub kind: ModelKind,
    pub tokenizer: Tokenizer,
    pub max_seq_len: usize,
    pub vocab_size: usize,
    device: candle_core::Device,
    inner: ModelVariant,
}

#[derive(Debug)]
enum ModelVariant {
    Llama2C {
        model: candle_transformers::models::quantized_llama2_c::QLlama,
        config: candle_transformers::models::llama2_c::Config,
    },
    #[cfg(feature = "native")]
    Gemma3 {
        model: candle_transformers::models::gemma3::Model,
        config: candle_transformers::models::gemma3::Config,
    },
}

impl LoadedModel {
    pub async fn load(source: &ModelSource, device: &MlDevice) -> Result<Self> {
        Self::load_with_hook(source, device, None).await
    }

    pub async fn load_with_hook(
        source: &ModelSource,
        device: &MlDevice,
        hook: Option<&dyn ModelLifecycleHook>,
    ) -> Result<Self> {
        emit_load_stage(
            hook,
            "load_start",
            StageStatus::Started,
            Some(format!("model_id={}", source.id)),
            None,
            None,
        );
        let device = device.candle_device();
        let tokenizer_path = source
            .tokenizer
            .as_ref()
            .ok_or_else(|| MlError::InvalidConfig("missing tokenizer".to_string()))?;
        emit_load_stage(
            hook,
            "tokenizer_load",
            StageStatus::Started,
            Some(tokenizer_path.to_string()),
            None,
            None,
        );
        let tokenizer = load_tokenizer(tokenizer_path).await?;
        emit_load_stage(
            hook,
            "tokenizer_load",
            StageStatus::Completed,
            None,
            None,
            None,
        );

        let result: Result<Self> = match source.kind {
            ModelKind::Llama2CQuantized => {
                let weights_path = source
                    .weights
                    .first()
                    .ok_or_else(|| MlError::InvalidConfig("missing gguf weights".to_string()))?;
                emit_load_stage(
                    hook,
                    "weights_load",
                    StageStatus::Started,
                    Some(weights_path.to_string()),
                    None,
                    None,
                );
                let (model, config) =
                    load_llama2c_gguf(weights_path, &device, hook).await?;
                emit_load_stage(
                    hook,
                    "weights_load",
                    StageStatus::Completed,
                    None,
                    None,
                    None,
                );
                let max_seq_len = config.seq_len;
                let vocab_size = config.vocab_size;
                Ok(Self {
                    id: source.id.clone(),
                    kind: source.kind.clone(),
                    tokenizer,
                    max_seq_len,
                    vocab_size,
                    device,
                    inner: ModelVariant::Llama2C { model, config },
                })
            }
            ModelKind::Gemma3 => {
                #[cfg(feature = "native")]
                {
                    let config_path = source
                        .config
                        .as_ref()
                        .ok_or_else(|| MlError::InvalidConfig("missing config".to_string()))?;
                    emit_load_stage(
                        hook,
                        "weights_load",
                        StageStatus::Started,
                        Some(config_path.to_string()),
                        None,
                        None,
                    );
                    let (model, config) = load_gemma3_safetensors(
                        &source.weights,
                        config_path,
                        &device,
                        hook,
                    )?;
                    emit_load_stage(
                        hook,
                        "weights_load",
                        StageStatus::Completed,
                        None,
                        None,
                        None,
                    );
                    let max_seq_len = config.max_position_embeddings;
                    let vocab_size = config.vocab_size;
                    Ok(Self {
                        id: source.id.clone(),
                        kind: source.kind.clone(),
                        tokenizer,
                        max_seq_len,
                        vocab_size,
                        device,
                        inner: ModelVariant::Gemma3 { model, config },
                    })
                }
                #[cfg(not(feature = "native"))]
                {
                    let config_path = source
                        .config
                        .as_ref()
                        .ok_or_else(|| MlError::InvalidConfig("missing config".to_string()))?;
                    Err(MlError::InvalidConfig(format!(
                        "gemma3 loading requires native feature (config: {config_path})"
                    )))
                }
            }
        };

        match result {
            Ok(model) => {
                emit_load_stage(hook, "load_complete", StageStatus::Completed, None, None, None);
                Ok(model)
            }
            Err(err) => {
                emit_load_stage(
                    hook,
                    "load_failed",
                    StageStatus::Failed,
                    Some(err.to_string()),
                    None,
                    None,
                );
                Err(err)
            }
        }
    }

    pub fn generate(
        &mut self,
        prompt: &str,
        config: &GenerationConfig,
        mut on_token: Option<&mut dyn FnMut(String)>,
    ) -> Result<GenerationOutcome> {
        self.generate_with_hook(prompt, config, &mut on_token, None, None)
    }

    pub fn generate_with_hook(
        &mut self,
        prompt: &str,
        config: &GenerationConfig,
        on_token: &mut Option<&mut dyn FnMut(String)>,
        hook: Option<&dyn InferenceHook>,
        lifecycle_hook: Option<&dyn ModelLifecycleHook>,
    ) -> Result<GenerationOutcome> {
        match &mut self.inner {
            ModelVariant::Llama2C {
                model,
                config: cfg,
            } => generate_llama2c(
                model,
                cfg,
                &self.tokenizer,
                &self.device,
                prompt,
                config,
                on_token,
                hook,
                lifecycle_hook,
            ),
            #[cfg(feature = "native")]
            ModelVariant::Gemma3 { model, config: cfg } => generate_gemma3(
                model,
                cfg,
                &self.tokenizer,
                &self.device,
                prompt,
                config,
                on_token,
                hook,
                lifecycle_hook,
            ),
        }
    }
}

fn build_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| rand::random::<u64>());
    StdRng::seed_from_u64(seed)
}

fn generate_llama2c(
    model: &candle_transformers::models::quantized_llama2_c::QLlama,
    cfg: &candle_transformers::models::llama2_c::Config,
    tokenizer: &Tokenizer,
    device: &candle_core::Device,
    prompt: &str,
    config: &GenerationConfig,
    on_token: &mut Option<&mut dyn FnMut(String)>,
    hook: Option<&dyn InferenceHook>,
    lifecycle_hook: Option<&dyn ModelLifecycleHook>,
) -> Result<GenerationOutcome> {
    emit_inference_stage(
        lifecycle_hook,
        "tokenize_prompt",
        StageStatus::Started,
        None,
        None,
        Some(format!("prompt_bytes={}", prompt.len())),
    );
    let mut tokens = tokenizer.encode(prompt, true)?;
    let prompt_tokens = tokens.len();
    emit_inference_stage(
        lifecycle_hook,
        "tokenize_prompt",
        StageStatus::Completed,
        None,
        None,
        Some(format!("prompt_tokens={prompt_tokens}")),
    );
    let mut output = String::new();
    let mut rng = build_rng(config.seed);
    let start_time = Instant::now();

    emit_inference_stage(
        lifecycle_hook,
        "cache_init",
        StageStatus::Started,
        None,
        None,
        None,
    );
    let cache = build_llama2c_cache(cfg, device)?;
    let mut cache = cache;
    emit_inference_stage(
        lifecycle_hook,
        "cache_init",
        StageStatus::Completed,
        None,
        None,
        None,
    );
    let mut index_pos = 0usize;

    for step in 0..config.max_new_tokens {
        let context_size = if step > 0 { 1 } else { tokens.len() };
        let context_start = tokens.len().saturating_sub(context_size);
        let ctxt = tokens[context_start..].to_vec();
        let input = Tensor::new(ctxt.as_slice(), device)?.unsqueeze(0)?;
        if step == 0 {
            emit_inference_stage(
                lifecycle_hook,
                "prefill",
                StageStatus::Started,
                Some(step),
                Some(config.max_new_tokens),
                None,
            );
        }
        let logits = model.forward(&input, index_pos, &mut cache)?;
        let logits = logits.i((0, logits.dim(1)? - 1))?;
        let logits = logits.to_dtype(DType::F32)?;
        let logits = logits.to_vec1::<f32>()?;
        let next_token = sample_from_logits(&logits, config, &tokens, &mut rng)?;

        if step == 0 {
            emit_inference_stage(
                lifecycle_hook,
                "prefill",
                StageStatus::Completed,
                Some(step + 1),
                Some(config.max_new_tokens),
                None,
            );
        }

        if config.stop_tokens.contains(&next_token) {
            break;
        }

        if hook.is_some() || lifecycle_hook.is_some() {
            let generated_tokens = tokens.len().saturating_sub(prompt_tokens) + 1;
            emit_token_telemetry(
                hook,
                lifecycle_hook,
                &logits,
                config,
                &tokens,
                tokenizer,
                next_token,
                generated_tokens,
                &start_time,
            )?;
            emit_llama_cache_status(hook, lifecycle_hook, &cache, tokens.len(), cfg.seq_len);
        }

        emit_inference_stage(
            lifecycle_hook,
            "decode_step",
            StageStatus::Progress,
            Some(step + 1),
            Some(config.max_new_tokens),
            None,
        );

        tokens.push(next_token);
        let token_text = tokenizer.decode(&[next_token], true)?;
        output.push_str(&token_text);
        if let Some(callback) = on_token.as_mut() {
            callback(token_text);
        }

        index_pos += ctxt.len();
        if tokens.len() >= cfg.seq_len {
            break;
        }
    }

    emit_inference_stage(
        lifecycle_hook,
        "inference_complete",
        StageStatus::Completed,
        Some(tokens.len().saturating_sub(prompt_tokens)),
        None,
        None,
    );

    Ok(GenerationOutcome {
        text: output,
        prompt_tokens,
        generated_tokens: tokens.len().saturating_sub(prompt_tokens),
    })
}

#[cfg(feature = "native")]
fn generate_gemma3(
    model: &mut candle_transformers::models::gemma3::Model,
    cfg: &candle_transformers::models::gemma3::Config,
    tokenizer: &Tokenizer,
    device: &candle_core::Device,
    prompt: &str,
    config: &GenerationConfig,
    on_token: &mut Option<&mut dyn FnMut(String)>,
    hook: Option<&dyn InferenceHook>,
    lifecycle_hook: Option<&dyn ModelLifecycleHook>,
) -> Result<GenerationOutcome> {
    model.clear_kv_cache();

    emit_inference_stage(
        lifecycle_hook,
        "tokenize_prompt",
        StageStatus::Started,
        None,
        None,
        Some(format!("prompt_bytes={}", prompt.len())),
    );
    let mut tokens = tokenizer.encode(prompt, true)?;
    let prompt_tokens = tokens.len();
    emit_inference_stage(
        lifecycle_hook,
        "tokenize_prompt",
        StageStatus::Completed,
        None,
        None,
        Some(format!("prompt_tokens={prompt_tokens}")),
    );
    let mut output = String::new();
    let mut rng = build_rng(config.seed);
    let start_time = Instant::now();

    for step in 0..config.max_new_tokens {
        let context_size = if step > 0 { 1 } else { tokens.len() };
        let start_pos = tokens.len().saturating_sub(context_size);
        let ctxt = tokens[start_pos..].to_vec();
        let input = Tensor::new(ctxt.as_slice(), device)?.unsqueeze(0)?;
        if step == 0 {
            emit_inference_stage(
                lifecycle_hook,
                "prefill",
                StageStatus::Started,
                Some(step),
                Some(config.max_new_tokens),
                None,
            );
        }
        let logits = model.forward(&input, start_pos)?;
        let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
        let logits = logits.to_vec1::<f32>()?;
        let next_token = sample_from_logits(&logits, config, &tokens, &mut rng)?;

        if step == 0 {
            emit_inference_stage(
                lifecycle_hook,
                "prefill",
                StageStatus::Completed,
                Some(step + 1),
                Some(config.max_new_tokens),
                None,
            );
        }

        if config.stop_tokens.contains(&next_token) {
            break;
        }

        if hook.is_some() || lifecycle_hook.is_some() {
            let generated_tokens = tokens.len().saturating_sub(prompt_tokens) + 1;
            emit_token_telemetry(
                hook,
                lifecycle_hook,
                &logits,
                config,
                &tokens,
                tokenizer,
                next_token,
                generated_tokens,
                &start_time,
            )?;
        }

        emit_inference_stage(
            lifecycle_hook,
            "decode_step",
            StageStatus::Progress,
            Some(step + 1),
            Some(config.max_new_tokens),
            None,
        );

        tokens.push(next_token);
        let token_text = tokenizer.decode(&[next_token], true)?;
        output.push_str(&token_text);
        if let Some(callback) = on_token.as_mut() {
            callback(token_text);
        }

        if tokens.len() >= cfg.max_position_embeddings {
            break;
        }
    }

    emit_inference_stage(
        lifecycle_hook,
        "inference_complete",
        StageStatus::Completed,
        Some(tokens.len().saturating_sub(prompt_tokens)),
        None,
        None,
    );

    Ok(GenerationOutcome {
        text: output,
        prompt_tokens,
        generated_tokens: tokens.len().saturating_sub(prompt_tokens),
    })
}

fn build_llama2c_cache(
    cfg: &candle_transformers::models::llama2_c::Config,
    device: &candle_core::Device,
) -> Result<candle_transformers::models::llama2_c::Cache> {
    let tensors = HashMap::new();
    let vb = candle_nn::VarBuilder::from_tensors(tensors, DType::F32, device);
    Ok(candle_transformers::models::llama2_c::Cache::new(true, cfg, vb)?)
}

fn emit_token_telemetry(
    hook: Option<&dyn InferenceHook>,
    lifecycle_hook: Option<&dyn ModelLifecycleHook>,
    logits: &[f32],
    config: &GenerationConfig,
    prev_tokens: &[u32],
    tokenizer: &Tokenizer,
    token_id: u32,
    output_len: usize,
    start_time: &Instant,
) -> Result<()> {
    let mut scores = logits.to_vec();
    apply_repetition_penalty(&mut scores, config.repetition_penalty, prev_tokens);

    if config.temperature > 0.0 {
        for score in &mut scores {
            *score /= config.temperature;
        }
    }

    let probs = softmax(&scores);
    let entropy = probs
        .iter()
        .copied()
        .filter(|p| *p > 0.0)
        .map(|p| -p * p.ln())
        .sum::<f32>();

    let top_k = build_top_k(&probs, tokenizer, config.top_k);

    let elapsed = start_time.elapsed().as_secs_f32().max(1e-3);
    let tokens_per_sec = output_len as f32 / elapsed;

    let token_text = tokenizer
        .decode(&[token_id], true)
        .unwrap_or_else(|_| format!("<{token_id}>"));
    let telemetry = InferenceTelemetry::TokenGenerated {
        token_id,
        token_text,
        top_k,
        entropy,
        tokens_per_sec,
    };
    dispatch_inference_telemetry(hook, lifecycle_hook, telemetry);
    Ok(())
}

fn dispatch_inference_telemetry(
    hook: Option<&dyn InferenceHook>,
    lifecycle_hook: Option<&dyn ModelLifecycleHook>,
    telemetry: InferenceTelemetry,
) {
    if let Some(hook) = hook {
        hook.on_telemetry(telemetry.clone());
    }
    if let Some(hook) = lifecycle_hook {
        hook.on_lifecycle(ModelLifecycleTelemetry::InferenceEvent {
            event: telemetry,
            ts_ms: telemetry_timestamp_ms(),
        });
    }
}

fn build_top_k(
    probs: &[f32],
    tokenizer: &Tokenizer,
    requested_k: usize,
) -> Vec<TokenCandidate> {
    let k = if requested_k == 0 { 5 } else { requested_k.min(5) };
    let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    indexed
        .into_iter()
        .take(k)
        .map(|(idx, prob)| {
            let token_text = tokenizer
                .decode(&[idx as u32], true)
                .unwrap_or_else(|_| format!("<{idx}>"));
            TokenCandidate {
                token_id: idx as u32,
                token_text,
                probability: prob,
            }
        })
        .collect()
}

fn emit_llama_cache_status(
    hook: Option<&dyn InferenceHook>,
    lifecycle_hook: Option<&dyn ModelLifecycleHook>,
    cache: &candle_transformers::models::llama2_c::Cache,
    seq_len: usize,
    max_len: usize,
) {
    for (layer, entry) in cache.kvs.iter().enumerate() {
        let memory_bytes = entry
            .as_ref()
            .map(|(k, v)| tensor_bytes(k) + tensor_bytes(v))
            .unwrap_or(0);

        dispatch_inference_telemetry(
            hook,
            lifecycle_hook,
            InferenceTelemetry::CacheStatus {
                layer,
                seq_len,
                max_len,
                offset: 0,
                memory_bytes,
            },
        );
    }
}

fn emit_load_stage(
    hook: Option<&dyn ModelLifecycleHook>,
    stage: &str,
    status: StageStatus,
    detail: Option<String>,
    bytes: Option<u64>,
    total_bytes: Option<u64>,
) {
    if let Some(hook) = hook {
        hook.on_lifecycle(ModelLifecycleTelemetry::LoadStage {
            stage: stage.to_string(),
            status,
            detail,
            bytes,
            total_bytes,
            ts_ms: telemetry_timestamp_ms(),
        });
    }
}

fn emit_inference_stage(
    hook: Option<&dyn ModelLifecycleHook>,
    stage: &str,
    status: StageStatus,
    step: Option<usize>,
    total_steps: Option<usize>,
    detail: Option<String>,
) {
    if let Some(hook) = hook {
        hook.on_lifecycle(ModelLifecycleTelemetry::InferenceStage {
            stage: stage.to_string(),
            status,
            step,
            total_steps,
            detail,
            ts_ms: telemetry_timestamp_ms(),
        });
    }
}

fn tensor_bytes(tensor: &Tensor) -> usize {
    tensor.shape().elem_count() * tensor.dtype().size_in_bytes()
}

fn is_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

async fn load_tokenizer(path: &str) -> Result<Tokenizer> {
    if is_url(path) {
        Tokenizer::from_url(path).await
    } else {
        #[cfg(feature = "native")]
        {
            return Tokenizer::from_file(path);
        }
        #[cfg(not(feature = "native"))]
        {
            return Err(MlError::InvalidConfig(
                "tokenizer file loading requires native feature".to_string(),
            ));
        }
    }
}

async fn load_llama2c_gguf(
    path: &str,
    device: &candle_core::Device,
    hook: Option<&dyn ModelLifecycleHook>,
) -> Result<(
    candle_transformers::models::quantized_llama2_c::QLlama,
    candle_transformers::models::llama2_c::Config,
)> {
    let vb = if is_url(path) {
        emit_load_stage(
            hook,
            "weights_fetch",
            StageStatus::Started,
            Some(path.to_string()),
            None,
            None,
        );
        let bytes = fetch_bytes(path).await?;
        emit_load_stage(
            hook,
            "weights_fetch",
            StageStatus::Completed,
            Some(format!("bytes={}", bytes.len())),
            Some(bytes.len() as u64),
            None,
        );
        emit_load_stage(
            hook,
            "gguf_parse",
            StageStatus::Started,
            None,
            None,
            None,
        );
        candle_transformers::models::quantized_llama2_c::VarBuilder::from_gguf_buffer(
            &bytes, device,
        )?
    } else {
        #[cfg(target_arch = "wasm32")]
        {
            return Err(MlError::InvalidConfig(
                "gguf file loading requires a URL in wasm".to_string(),
            ));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Path::new(path);
            if path.exists() {
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                emit_load_stage(
                    hook,
                    "weights_map",
                    StageStatus::Started,
                    Some(path.display().to_string()),
                    Some(size),
                    None,
                );
                emit_load_stage(
                    hook,
                    "gguf_parse",
                    StageStatus::Started,
                    None,
                    None,
                    None,
                );
                let vb = candle_transformers::models::quantized_llama2_c::VarBuilder::from_gguf(
                    path, device,
                )?;
                emit_load_stage(
                    hook,
                    "weights_map",
                    StageStatus::Completed,
                    None,
                    Some(size),
                    None,
                );
                vb
            } else {
                return Err(MlError::InvalidConfig(format!(
                    "gguf file not found: {}",
                    path.display()
                )));
            }
        }
    };

    emit_load_stage(
        hook,
        "gguf_parse",
        StageStatus::Completed,
        None,
        None,
        None,
    );

    let embed = vb
        .get_no_shape("model.embed_tokens.weight")
        .map_err(|_| {
            MlError::InvalidConfig(
                "gguf missing model.embed_tokens.weight (unsupported naming)".to_string(),
            )
        })?;
    let (_vocab, dim) = embed.shape().dims2()?;
    let config = match dim {
        64 => candle_transformers::models::llama2_c::Config::tiny_260k(),
        288 => candle_transformers::models::llama2_c::Config::tiny_15m(),
        512 => candle_transformers::models::llama2_c::Config::tiny_42m(),
        768 => candle_transformers::models::llama2_c::Config::tiny_110m(),
        _ => {
            return Err(MlError::InvalidConfig(format!(
                "unsupported llama2-c dim: {dim}"
            )))
        }
    };

    emit_load_stage(
        hook,
        "model_init",
        StageStatus::Started,
        None,
        None,
        None,
    );
    let model = candle_transformers::models::quantized_llama2_c::QLlama::load(vb, config.clone())?;
    emit_load_stage(
        hook,
        "model_init",
        StageStatus::Completed,
        None,
        None,
        None,
    );
    Ok((model, config))
}

#[cfg(feature = "native")]
fn load_gemma3_safetensors(
    weights: &[String],
    config_path: &str,
    device: &candle_core::Device,
    hook: Option<&dyn ModelLifecycleHook>,
) -> Result<(
    candle_transformers::models::gemma3::Model,
    candle_transformers::models::gemma3::Config,
)> {
    if weights.is_empty() {
        return Err(MlError::InvalidConfig("missing weights".to_string()));
    }

    let dtype = if device.supports_bf16() {
        DType::BF16
    } else {
        DType::F32
    };

    let config_file = std::fs::File::open(config_path)?;
    let config: candle_transformers::models::gemma3::Config =
        serde_json::from_reader(config_file)?;

    let files: Vec<PathBuf> = weights.iter().map(PathBuf::from).collect();
    let mut total_bytes = 0u64;
    for file in &files {
        if let Ok(meta) = std::fs::metadata(file) {
            total_bytes = total_bytes.saturating_add(meta.len());
        }
    }
    emit_load_stage(
        hook,
        "weights_map",
        StageStatus::Started,
        Some(format!("files={}", files.len())),
        Some(total_bytes),
        None,
    );
    let vb = unsafe { candle_nn::VarBuilder::from_mmaped_safetensors(&files, dtype, device)? };
    emit_load_stage(
        hook,
        "weights_map",
        StageStatus::Completed,
        None,
        Some(total_bytes),
        None,
    );
    emit_load_stage(
        hook,
        "model_init",
        StageStatus::Started,
        None,
        None,
        None,
    );
    let model = candle_transformers::models::gemma3::Model::new(false, &config, vb)?;
    emit_load_stage(
        hook,
        "model_init",
        StageStatus::Completed,
        None,
        None,
        None,
    );

    Ok((model, config))
}
