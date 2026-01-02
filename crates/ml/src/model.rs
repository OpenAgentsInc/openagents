use crate::device::MlDevice;
use crate::error::{MlError, Result};
use crate::http::fetch_bytes;
use crate::sampling::{sample_from_logits, GenerationConfig};
use crate::tokenizer::Tokenizer;
use candle_core::{DType, IndexOp, Tensor};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
#[cfg(feature = "native")]
use std::path::PathBuf;

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
        let device = device.candle_device();
        let tokenizer_path = source
            .tokenizer
            .as_ref()
            .ok_or_else(|| MlError::InvalidConfig("missing tokenizer".to_string()))?;
        let tokenizer = load_tokenizer(tokenizer_path).await?;

        match source.kind {
            ModelKind::Llama2CQuantized => {
                let weights_path = source
                    .weights
                    .first()
                    .ok_or_else(|| MlError::InvalidConfig("missing gguf weights".to_string()))?;
                let (model, config) = load_llama2c_gguf(weights_path, &device).await?;
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
                    let (model, config) =
                        load_gemma3_safetensors(&source.weights, config_path, &device)?;
                    let max_seq_len = config.max_position_embeddings;
                    let vocab_size = config.vocab_size;
                    return Ok(Self {
                        id: source.id.clone(),
                        kind: source.kind.clone(),
                        tokenizer,
                        max_seq_len,
                        vocab_size,
                        device,
                        inner: ModelVariant::Gemma3 { model, config },
                    });
                }
                #[cfg(not(feature = "native"))]
                {
                    let config_path = source
                        .config
                        .as_ref()
                        .ok_or_else(|| MlError::InvalidConfig("missing config".to_string()))?;
                    return Err(MlError::InvalidConfig(format!(
                        "gemma3 loading requires native feature (config: {config_path})"
                    )));
                }
            }
        }
    }

    pub fn generate(
        &mut self,
        prompt: &str,
        config: &GenerationConfig,
        mut on_token: Option<&mut dyn FnMut(String)>,
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
                &mut on_token,
            ),
            #[cfg(feature = "native")]
            ModelVariant::Gemma3 { model, config: cfg } => generate_gemma3(
                model,
                cfg,
                &self.tokenizer,
                &self.device,
                prompt,
                config,
                &mut on_token,
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
) -> Result<GenerationOutcome> {
    let mut tokens = tokenizer.encode(prompt, true)?;
    let prompt_tokens = tokens.len();
    let mut output = String::new();
    let mut rng = build_rng(config.seed);

    let cache = build_llama2c_cache(cfg, device)?;
    let mut cache = cache;
    let mut index_pos = 0usize;

    for step in 0..config.max_new_tokens {
        let context_size = if step > 0 { 1 } else { tokens.len() };
        let context_start = tokens.len().saturating_sub(context_size);
        let ctxt = tokens[context_start..].to_vec();
        let input = Tensor::new(ctxt.as_slice(), device)?.unsqueeze(0)?;
        let logits = model.forward(&input, index_pos, &mut cache)?;
        let logits = logits.i((0, logits.dim(1)? - 1))?;
        let logits = logits.to_dtype(DType::F32)?;
        let logits = logits.to_vec1::<f32>()?;
        let next_token = sample_from_logits(&logits, config, &tokens, &mut rng)?;

        if config.stop_tokens.contains(&next_token) {
            break;
        }

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
) -> Result<GenerationOutcome> {
    model.clear_kv_cache();

    let mut tokens = tokenizer.encode(prompt, true)?;
    let prompt_tokens = tokens.len();
    let mut output = String::new();
    let mut rng = build_rng(config.seed);

    for step in 0..config.max_new_tokens {
        let context_size = if step > 0 { 1 } else { tokens.len() };
        let start_pos = tokens.len().saturating_sub(context_size);
        let ctxt = tokens[start_pos..].to_vec();
        let input = Tensor::new(ctxt.as_slice(), device)?.unsqueeze(0)?;
        let logits = model.forward(&input, start_pos)?;
        let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
        let logits = logits.to_vec1::<f32>()?;
        let next_token = sample_from_logits(&logits, config, &tokens, &mut rng)?;

        if config.stop_tokens.contains(&next_token) {
            break;
        }

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
) -> Result<(
    candle_transformers::models::quantized_llama2_c::QLlama,
    candle_transformers::models::llama2_c::Config,
)> {
    let vb = if is_url(path) {
        let bytes = fetch_bytes(path).await?;
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
                candle_transformers::models::quantized_llama2_c::VarBuilder::from_gguf(
                    path, device,
                )?
            } else {
                return Err(MlError::InvalidConfig(format!(
                    "gguf file not found: {}",
                    path.display()
                )));
            }
        }
    };

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

    let model = candle_transformers::models::quantized_llama2_c::QLlama::load(vb, config.clone())?;
    Ok((model, config))
}

#[cfg(feature = "native")]
fn load_gemma3_safetensors(
    weights: &[String],
    config_path: &str,
    device: &candle_core::Device,
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
    let vb = unsafe { candle_nn::VarBuilder::from_mmaped_safetensors(&files, dtype, device)? };
    let model = candle_transformers::models::gemma3::Model::new(false, &config, vb)?;

    Ok((model, config))
}
