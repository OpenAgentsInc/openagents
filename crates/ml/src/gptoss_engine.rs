use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::sampling::{sample_from_logits, GenerationConfig};
use crate::telemetry::{InferenceHook, InferenceTelemetry, TokenCandidate};
use crate::{
    apply_bias, apply_rope, attention_with_cache, find_tensor, load_gguf_model, matmul_f32,
    matmul_mxfp4_expert, matmul_q8_0, read_f32_row, read_f32_tensor, read_meta_f32,
    read_meta_f32_optional, read_meta_u32, read_meta_u32_optional, read_q8_0_row, rms_norm,
    swiglu, top_k_softmax, GgufMetadata, GgufScalar, GgufTensorDump, GptOssTokenizer, KvCache,
    MlError, Result,
};

const CURRENT_DATE: &str = "2026-01-02";
const DEFAULT_TELEMETRY_TOP_K: usize = 5;

#[derive(Debug, Clone)]
pub struct GptOssModelConfig {
    pub block_count: u32,
    pub context_length: u32,
    pub embedding_length: u32,
    pub feed_forward_length: u32,
    pub head_count: u32,
    pub head_count_kv: u32,
    pub rope_dimension_count: u32,
    pub rope_theta: f32,
    pub rope_scaling_factor: f32,
    pub rope_scaling_original_context: u32,
    pub rms_epsilon: f32,
    pub sliding_window: u32,
    pub expert_count: u32,
    pub experts_per_token: u32,
}

impl GptOssModelConfig {
    pub fn from_metadata(meta: &GgufMetadata) -> Result<Self> {
        Ok(Self {
            block_count: read_meta_u32(meta, "llama.block_count")?,
            context_length: read_meta_u32_optional(meta, "gpt-oss.context_length")
                .or_else(|| read_meta_u32_optional(meta, "llama.context_length"))
                .unwrap_or(0),
            embedding_length: read_meta_u32(meta, "llama.embedding_length")?,
            feed_forward_length: read_meta_u32(meta, "llama.feed_forward_length")?,
            head_count: read_meta_u32(meta, "llama.attention.head_count")?,
            head_count_kv: read_meta_u32(meta, "llama.attention.head_count_kv")?,
            rope_dimension_count: read_meta_u32(meta, "llama.rope.dimension_count")?,
            rope_theta: read_meta_f32(meta, "llama.rope.freq_base")?,
            rope_scaling_factor: read_meta_f32_optional(meta, "gpt-oss.rope.scaling.factor")
                .unwrap_or(1.0),
            rope_scaling_original_context: read_meta_u32_optional(
                meta,
                "gpt-oss.rope.scaling.original_context_length",
            )
            .unwrap_or(0),
            rms_epsilon: read_meta_f32(meta, "llama.attention.layer_norm_rms_epsilon")?,
            sliding_window: read_meta_u32(meta, "llama.sliding_window")?,
            expert_count: read_meta_u32(meta, "llama.expert_count")?,
            experts_per_token: read_meta_u32(meta, "llama.expert_used_count")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GptOssEngineConfig {
    pub generation: GenerationConfig,
    pub layer_limit: Option<usize>,
    pub max_kv: Option<usize>,
    pub moe_fallback: bool,
    pub use_harmony_prompt: bool,
    pub telemetry_top_k: usize,
}

impl Default for GptOssEngineConfig {
    fn default() -> Self {
        Self {
            generation: GenerationConfig::default(),
            layer_limit: None,
            max_kv: None,
            moe_fallback: false,
            use_harmony_prompt: true,
            telemetry_top_k: DEFAULT_TELEMETRY_TOP_K,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GptOssCompletion {
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub finish_reason: String,
}

#[derive(Debug, Clone)]
pub struct GptOssTokenEvent {
    pub token_id: u32,
    pub token_text: String,
    pub top_k: Vec<TokenCandidate>,
    pub entropy: f32,
    pub tokens_per_sec: f32,
}

pub struct GptOssEngine {
    path: PathBuf,
    index: crate::GgufIndex,
    tokenizer: GptOssTokenizer,
    config: GptOssModelConfig,
    f32_cache: HashMap<String, Vec<f32>>,
    model_id: String,
}

impl GptOssEngine {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let model = load_gguf_model(&path)?;
        let tokenizer_meta = model.metadata.tokenizer.clone().ok_or_else(|| {
            MlError::Model("gguf tokenizer metadata missing".to_string())
        })?;
        let tokenizer = GptOssTokenizer::from_gguf(tokenizer_meta).map_err(MlError::Model)?;
        let config = GptOssModelConfig::from_metadata(&model.metadata)?;
        let model_id = infer_model_id(&model.metadata, &path);

        Ok(Self {
            path,
            index: model.index,
            tokenizer,
            config,
            f32_cache: HashMap::new(),
            model_id,
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn context_length(&self) -> usize {
        self.config.context_length as usize
    }

    pub fn token_id(&self, token: &str) -> Option<u32> {
        self.tokenizer.token_id(token)
    }

    pub fn tokenizer(&self) -> &GptOssTokenizer {
        &self.tokenizer
    }

    pub fn generate_with_callback(
        &mut self,
        prompt: &str,
        config: &GptOssEngineConfig,
        mut on_token: Option<&mut dyn FnMut(&GptOssTokenEvent) -> Result<()>>,
        hook: Option<&dyn InferenceHook>,
    ) -> Result<GptOssCompletion> {
        let prompt = if config.use_harmony_prompt {
            build_harmony_prompt(prompt)
        } else {
            prompt.to_string()
        };
        let mut tokens = self
            .tokenizer
            .encode_with_special_tokens(&prompt)
            .map_err(MlError::Model)?;
        if tokens.is_empty() {
            return Err(MlError::Model("prompt token list is empty".to_string()));
        }

        let mut max_kv = config
            .max_kv
            .unwrap_or_else(|| tokens.len() + config.generation.max_new_tokens);
        if self.config.context_length > 0 {
            max_kv = max_kv.min(self.config.context_length as usize);
        }

        let max_prompt_tokens = max_kv.saturating_sub(config.generation.max_new_tokens);
        if max_prompt_tokens == 0 {
            return Err(MlError::Model(
                "prompt token limit is zero (raise max_kv or lower max_new)".to_string(),
            ));
        }
        if tokens.len() > max_prompt_tokens {
            let total = tokens.len();
            tokens = tokens.split_off(total - max_prompt_tokens);
        }

        let mut stop_tokens = collect_stop_tokens(&self.tokenizer);
        stop_tokens.extend(config.generation.stop_tokens.iter().copied());
        stop_tokens.sort_unstable();
        stop_tokens.dedup();

        let active_layers = config
            .layer_limit
            .unwrap_or(self.config.block_count as usize)
            .min(self.config.block_count as usize)
            .max(1);

        let prompt_tokens = tokens.len();
        let mut cache = KvCache::new(self.config.block_count as usize, max_kv);
        let mut last_logits = None;

        for (idx, &token_id) in tokens.iter().enumerate() {
            let logits = run_forward_token(
                &self.path,
                &self.index,
                &mut self.f32_cache,
                &self.config,
                token_id,
                idx,
                &mut cache,
                active_layers,
                config.moe_fallback,
            )?;
            last_logits = Some(logits);
        }

        let mut logits = last_logits.ok_or_else(|| MlError::Model("no logits".to_string()))?;
        let mut generated = 0usize;
        let mut output = String::new();
        let mut rng = seeded_rng(config.generation.seed);
        let start = Instant::now();

        while generated < config.generation.max_new_tokens {
            let (top_k, entropy) = if config.telemetry_top_k == 0 {
                (Vec::new(), 0.0)
            } else {
                top_k_from_logits(&logits, &self.tokenizer, config.telemetry_top_k)?
            };

            let next_id =
                sample_from_logits(&logits, &config.generation, &tokens, &mut rng)?;
            if stop_tokens.contains(&next_id) {
                return Ok(GptOssCompletion {
                    text: output,
                    prompt_tokens,
                    completion_tokens: generated,
                    finish_reason: "stop".to_string(),
                });
            }

            let token_text = self.tokenizer.decode_utf8_lossy(&[next_id]);
            output.push_str(&token_text);
            generated += 1;

            let tokens_per_sec = if generated == 0 {
                0.0
            } else {
                let elapsed = start.elapsed().as_secs_f32().max(1e-6);
                generated as f32 / elapsed
            };

            if let Some(hook) = hook {
                hook.on_telemetry(InferenceTelemetry::TokenGenerated {
                    token_id: next_id,
                    token_text: token_text.clone(),
                    top_k: top_k.clone(),
                    entropy,
                    tokens_per_sec,
                });
            }

            if let Some(callback) = on_token.as_mut() {
                let event = GptOssTokenEvent {
                    token_id: next_id,
                    token_text: token_text.clone(),
                    top_k: top_k.clone(),
                    entropy,
                    tokens_per_sec,
                };
                callback(&event)?;
            }

            tokens.push(next_id);
            logits = run_forward_token(
                &self.path,
                &self.index,
                &mut self.f32_cache,
                &self.config,
                next_id,
                tokens.len().saturating_sub(1),
                &mut cache,
                active_layers,
                config.moe_fallback,
            )?;
        }

        Ok(GptOssCompletion {
            text: output,
            prompt_tokens,
            completion_tokens: generated,
            finish_reason: "length".to_string(),
        })
    }
}

fn infer_model_id(metadata: &GgufMetadata, path: &Path) -> String {
    if let Some(GgufScalar::String(value)) = metadata.values.get("general.name") {
        return value.clone();
    }
    if let Some(GgufScalar::String(value)) = metadata.values.get("general.model") {
        return value.clone();
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("gpt-oss-gguf")
        .to_string()
}

fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| rand::random::<u64>());
    StdRng::seed_from_u64(seed)
}

fn build_harmony_prompt(user_prompt: &str) -> String {
    let system_prompt = format!(
        "You are ChatGPT, a large language model trained by OpenAI.\n\
Knowledge cutoff: 2024-06\n\
Current date: {CURRENT_DATE}\n\n\
Reasoning: low\n\n\
# Valid channels: analysis, commentary, final. Channel must be included for every message.\n\
Calls to these tools must go to the commentary channel: 'functions'."
    );
    let developer_prompt = "# Instructions\n\n".to_string();

    let mut prompt = String::new();
    prompt.push_str("<|start|>system<|message|>");
    prompt.push_str(&system_prompt);
    prompt.push_str("<|end|><|start|>developer<|message|>");
    prompt.push_str(&developer_prompt);
    prompt.push_str("<|end|><|start|>user<|message|>");
    prompt.push_str(user_prompt);
    prompt.push_str("<|end|><|start|>assistant<|channel|>final<|message|>");
    prompt
}

fn collect_stop_tokens(tokenizer: &GptOssTokenizer) -> Vec<u32> {
    let mut tokens = Vec::new();
    for name in ["<|return|>", "<|call|>"] {
        if let Some(id) = tokenizer.token_id(name) {
            tokens.push(id);
        }
    }
    if let Some(id) = tokenizer.eos_token_id() {
        tokens.push(id);
    }
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

fn top_k_from_logits(
    logits: &[f32],
    tokenizer: &GptOssTokenizer,
    k: usize,
) -> Result<(Vec<TokenCandidate>, f32)> {
    if logits.is_empty() {
        return Err(MlError::Model("empty logits".to_string()));
    }
    let mut max_logit = f32::NEG_INFINITY;
    for &logit in logits {
        if logit > max_logit {
            max_logit = logit;
        }
    }
    let mut sum_exp = 0.0f32;
    for &logit in logits {
        sum_exp += (logit - max_logit).exp();
    }
    if sum_exp <= 0.0 {
        return Err(MlError::Model("softmax sum is zero".to_string()));
    }
    let mut entropy = 0.0f32;
    for &logit in logits {
        let p = (logit - max_logit).exp() / sum_exp;
        if p > 0.0 {
            entropy -= p * p.ln();
        }
    }
    let mut pairs: Vec<(usize, f32)> = logits
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, logit)| (idx, (logit - max_logit).exp() / sum_exp))
        .collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut top = Vec::new();
    for (idx, prob) in pairs.into_iter().take(k.max(1)) {
        let token_id = idx as u32;
        top.push(TokenCandidate {
            token_id,
            token_text: tokenizer.decode_utf8_lossy(&[token_id]),
            probability: prob,
        });
    }
    Ok((top, entropy))
}

fn fetch_f32_cached(
    path: &Path,
    tensor: &GgufTensorDump,
    cache: &mut HashMap<String, Vec<f32>>,
) -> Result<Vec<f32>> {
    if let Some(hit) = cache.get(&tensor.name) {
        return Ok(hit.clone());
    }
    let data = read_f32_tensor(path, tensor)?;
    cache.insert(tensor.name.clone(), data.clone());
    Ok(data)
}

fn run_forward_token(
    path: &Path,
    index: &crate::GgufIndex,
    f32_cache: &mut HashMap<String, Vec<f32>>,
    config: &GptOssModelConfig,
    token_id: u32,
    position: usize,
    cache: &mut KvCache,
    active_layers: usize,
    moe_fallback: bool,
) -> Result<Vec<f32>> {
    let token_embd = find_tensor(index, "token_embd.weight")?;
    let mut hidden = read_q8_0_row(path, token_embd, token_id as usize)?;

    let layer_limit = active_layers
        .min(config.block_count as usize)
        .max(1);
    for layer in 0..layer_limit as u32 {
        hidden = run_transformer_layer(
            path,
            index,
            f32_cache,
            config,
            layer,
            position,
            hidden,
            cache,
            moe_fallback,
        )?;
    }

    let output_norm = find_tensor(index, "output_norm.weight")?;
    let output_norm_w = fetch_f32_cached(path, output_norm, f32_cache)?;
    let final_hidden = rms_norm(&hidden, &output_norm_w, config.rms_epsilon)?;

    let output_weight = find_tensor(index, "output.weight")?;
    let logits = matmul_q8_0(path, output_weight, &final_hidden)?;
    cache.seq_len = position.saturating_add(1);
    Ok(logits)
}

fn run_transformer_layer(
    path: &Path,
    index: &crate::GgufIndex,
    f32_cache: &mut HashMap<String, Vec<f32>>,
    config: &GptOssModelConfig,
    layer: u32,
    position: usize,
    mut hidden: Vec<f32>,
    cache: &mut KvCache,
    moe_fallback: bool,
) -> Result<Vec<f32>> {
    let attn_norm = find_tensor(index, &format!("blk.{layer}.attn_norm.weight"))?;
    let attn_q_w = find_tensor(index, &format!("blk.{layer}.attn_q.weight"))?;
    let attn_q_b = find_tensor(index, &format!("blk.{layer}.attn_q.bias"))?;
    let attn_k_w = find_tensor(index, &format!("blk.{layer}.attn_k.weight"))?;
    let attn_k_b = find_tensor(index, &format!("blk.{layer}.attn_k.bias"))?;
    let attn_v_w = find_tensor(index, &format!("blk.{layer}.attn_v.weight"))?;
    let attn_v_b = find_tensor(index, &format!("blk.{layer}.attn_v.bias"))?;
    let attn_out_w = find_tensor(index, &format!("blk.{layer}.attn_output.weight"))?;
    let attn_out_b = find_tensor(index, &format!("blk.{layer}.attn_output.bias"))?;
    let attn_sinks = find_tensor(index, &format!("blk.{layer}.attn_sinks.weight"))?;
    let post_attn_norm =
        find_tensor(index, &format!("blk.{layer}.post_attention_norm.weight"))?;

    let attn_norm_w = fetch_f32_cached(path, attn_norm, f32_cache)?;
    let mut normed = rms_norm(&hidden, &attn_norm_w, config.rms_epsilon)?;

    let mut q = matmul_q8_0(path, attn_q_w, &normed)?;
    let mut k = matmul_q8_0(path, attn_k_w, &normed)?;
    let mut v = matmul_q8_0(path, attn_v_w, &normed)?;
    let q_bias = fetch_f32_cached(path, attn_q_b, f32_cache)?;
    let k_bias = fetch_f32_cached(path, attn_k_b, f32_cache)?;
    let v_bias = fetch_f32_cached(path, attn_v_b, f32_cache)?;
    apply_bias(&mut q, &q_bias);
    apply_bias(&mut k, &k_bias);
    apply_bias(&mut v, &v_bias);

    let heads = config.head_count as usize;
    let kv_heads = config.head_count_kv as usize;
    let head_dim = q.len() / heads.max(1);
    apply_rope(
        &mut q,
        heads,
        head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k,
        kv_heads,
        head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;

    let sinks = fetch_f32_cached(path, attn_sinks, f32_cache)?;
    let max_len = cache.max_len;
    let layer_cache = cache.layer_mut(layer as usize)?;
    if layer_cache.token_count(kv_heads, head_dim) >= max_len {
        return Err(MlError::Model("kv cache max length exceeded".to_string()));
    }
    layer_cache.append(&k, &v, kv_heads, head_dim)?;
    let seq_len = layer_cache.token_count(kv_heads, head_dim);
    let window = if config.sliding_window > 0 && layer % 2 == 0 {
        config.sliding_window as usize
    } else {
        seq_len
    };
    let attn_out = attention_with_cache(
        &q,
        layer_cache,
        &sinks,
        heads,
        kv_heads,
        head_dim,
        window.max(1),
    )?;

    let mut attn_proj = matmul_q8_0(path, attn_out_w, &attn_out)?;
    let attn_out_b = fetch_f32_cached(path, attn_out_b, f32_cache)?;
    apply_bias(&mut attn_proj, &attn_out_b);
    for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
        *out += *base;
    }

    let post_attn_norm_w = fetch_f32_cached(path, post_attn_norm, f32_cache)?;
    normed = rms_norm(&hidden, &post_attn_norm_w, config.rms_epsilon)?;

    let gate_inp_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.weight"))?;
    let gate_inp_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.bias"))?;
    let gate_w = fetch_f32_cached(path, gate_inp_w, f32_cache)?;
    let gate_b = fetch_f32_cached(path, gate_inp_b, f32_cache)?;
    let n = gate_inp_w.dims.get(0).copied().unwrap_or(0) as usize;
    let k = gate_inp_w.dims.get(1).copied().unwrap_or(0) as usize;
    let mut gate_scores = matmul_f32(&gate_w, &normed, k, n);
    apply_bias(&mut gate_scores, &gate_b);
    let (mut expert_indices, mut expert_weights) =
        top_k_softmax(&gate_scores, config.experts_per_token as usize)?;
    if moe_fallback {
        expert_indices = vec![0];
        expert_weights = vec![1.0];
    }

    let gate_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.weight"))?;
    let gate_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.bias"))?;
    let up_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.weight"))?;
    let up_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.bias"))?;
    let down_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.weight"))?;
    let down_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.bias"))?;

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let mut gate_out = matmul_mxfp4_expert(path, gate_exps_w, *expert_idx, &normed)?;
        let gate_bias = read_f32_row(path, gate_exps_b, *expert_idx)?;
        apply_bias(&mut gate_out, &gate_bias);

        let mut up_out = matmul_mxfp4_expert(path, up_exps_w, *expert_idx, &normed)?;
        let up_bias = read_f32_row(path, up_exps_b, *expert_idx)?;
        apply_bias(&mut up_out, &up_bias);

        let swiglu_out = swiglu(&gate_out, &up_out)?;
        let mut down_out = matmul_mxfp4_expert(path, down_exps_w, *expert_idx, &swiglu_out)?;
        let down_bias = read_f32_row(path, down_exps_b, *expert_idx)?;
        apply_bias(&mut down_out, &down_bias);

        for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
            *acc += *val * *weight;
        }
    }

    for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
        *out += *add;
    }

    Ok(hidden)
}
