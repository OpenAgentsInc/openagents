use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use ml::{
    apply_bias, apply_rope, attention_with_cache, find_tensor, load_gguf_model, matmul_mxfp4_expert,
    matmul_q8_0, matmul_f32, read_f32_row, read_f32_tensor, read_meta_f32, read_meta_u32,
    read_q8_0_row, rms_norm, swiglu, top_k_softmax, GptOssTokenizer, KvCache, MlError, Result,
};

const CURRENT_DATE: &str = "2026-01-02";

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut prompt = "Hello from GPT-OSS.".to_string();
    let mut max_tokens: usize = 20;
    let mut layer_limit: Option<usize> = None;
    let mut moe_fallback = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--prompt" => prompt = args.next().unwrap_or_default(),
            "--max-tokens" => {
                max_tokens = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(20);
            }
            "--layers" => {
                layer_limit = args.next().and_then(|v| v.parse::<usize>().ok());
            }
            "--moe-fallback" => moe_fallback = true,
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: gptoss_cli --gguf <path> --prompt \"hi\" --max-tokens 20".to_string(),
        )
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let tokenizer_meta = model.metadata.tokenizer.clone().ok_or_else(|| {
        MlError::Model("gguf tokenizer metadata missing".to_string())
    })?;
    let tokenizer = GptOssTokenizer::from_gguf(tokenizer_meta)
        .map_err(MlError::Model)?;

    let config = GptOssConfig::from_metadata(&model.metadata)?;
    let active_layers = layer_limit.unwrap_or(config.block_count as usize).max(1);

    let prompt = build_harmony_prompt(&prompt);
    let mut tokens = tokenizer
        .encode_with_special_tokens(&prompt)
        .map_err(MlError::Model)?;
    if tokens.is_empty() {
        return Err(MlError::Model("prompt token list is empty".to_string()));
    }

    let stop_tokens = collect_stop_tokens(&tokenizer);

    let path = PathBuf::from(&gguf_path);
    let mut cache = KvCache::new(config.block_count as usize, tokens.len() + max_tokens);
    let mut f32_cache: HashMap<String, Vec<f32>> = HashMap::new();

    let mut last_logits = None;
    for (idx, &token_id) in tokens.iter().enumerate() {
        let logits = run_forward_token(
            &path,
            &model.index,
            &mut f32_cache,
            &config,
            token_id,
            idx,
            &mut cache,
            active_layers,
            moe_fallback,
        )?;
        last_logits = Some(logits);
    }

    let mut logits = last_logits.ok_or_else(|| MlError::Model("no logits".to_string()))?;
    let mut generated = 0usize;
    let mut output = String::new();

    while generated < max_tokens {
        let (top_k, entropy, next_id, next_text) = top_k_from_logits(&logits, &tokenizer, 5)?;
        println!(
            "step={} token={} entropy={:.3} top1={}",
            generated, next_id, entropy, next_text.replace('\n', "\\n")
        );
        if stop_tokens.contains(&next_id) {
            break;
        }
        output.push_str(&next_text);
        tokens.push(next_id);
        generated += 1;

        logits = run_forward_token(
            &path,
            &model.index,
            &mut f32_cache,
            &config,
            next_id,
            tokens.len().saturating_sub(1),
            &mut cache,
            active_layers,
            moe_fallback,
        )?;
    }

    println!("output:\n{output}");
    Ok(())
}

struct GptOssConfig {
    block_count: u32,
    embedding_length: u32,
    feed_forward_length: u32,
    head_count: u32,
    head_count_kv: u32,
    rope_dimension_count: u32,
    rope_theta: f32,
    rms_epsilon: f32,
    sliding_window: u32,
    expert_count: u32,
    experts_per_token: u32,
}

impl GptOssConfig {
    fn from_metadata(meta: &ml::GgufMetadata) -> Result<Self> {
        Ok(Self {
            block_count: read_meta_u32(meta, "llama.block_count")?,
            embedding_length: read_meta_u32(meta, "llama.embedding_length")?,
            feed_forward_length: read_meta_u32(meta, "llama.feed_forward_length")?,
            head_count: read_meta_u32(meta, "llama.attention.head_count")?,
            head_count_kv: read_meta_u32(meta, "llama.attention.head_count_kv")?,
            rope_dimension_count: read_meta_u32(meta, "llama.rope.dimension_count")?,
            rope_theta: read_meta_f32(meta, "llama.rope.freq_base")?,
            rms_epsilon: read_meta_f32(meta, "llama.attention.layer_norm_rms_epsilon")?,
            sliding_window: read_meta_u32(meta, "llama.sliding_window")?,
            expert_count: read_meta_u32(meta, "llama.expert_count")?,
            experts_per_token: read_meta_u32(meta, "llama.expert_used_count")?,
        })
    }
}

fn build_harmony_prompt(user_prompt: &str) -> String {
    let system_prompt = format!(
        "You are ChatGPT, a large language model trained by OpenAI.\n\
Knowledge cutoff: 2024-06\n\
Current date: {CURRENT_DATE}\n\n\
Reasoning: low\n\n\
# Valid channels: analysis, commentary, final. Channel must be included for every message."
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
) -> Result<(Vec<(u32, f32)>, f32, u32, String)> {
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
        top.push((idx as u32, prob));
    }
    let best_id = top.first().map(|(id, _)| *id).unwrap_or(0);
    let best_text = tokenizer.token_text(best_id);
    Ok((top, entropy, best_id, best_text))
}

fn fetch_f32_cached(
    path: &PathBuf,
    tensor: &ml::GgufTensorDump,
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
    path: &PathBuf,
    index: &ml::GgufIndex,
    f32_cache: &mut HashMap<String, Vec<f32>>,
    config: &GptOssConfig,
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
    path: &PathBuf,
    index: &ml::GgufIndex,
    f32_cache: &mut HashMap<String, Vec<f32>>,
    config: &GptOssConfig,
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
    )?;
    apply_rope(
        &mut k,
        kv_heads,
        head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
    )?;

    let sinks = fetch_f32_cached(path, attn_sinks, f32_cache)?;
    let max_len = cache.max_len;
    let layer_cache = cache.layer_mut(layer as usize)?;
    if layer_cache.token_count(kv_heads, head_dim) >= max_len {
        return Err(MlError::Model("kv cache max length exceeded".to_string()));
    }
    layer_cache.append(&k, &v, kv_heads, head_dim)?;
    let seq_len = layer_cache.token_count(kv_heads, head_dim);
    let window = if config.sliding_window > 0 {
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
