use std::env;
use std::path::PathBuf;

use ml::{
    apply_bias, apply_rope, attention_head_weights, attention_with_cache, find_tensor,
    load_gguf_model, matmul_q8_0, read_f32_tensor, read_meta_f32, read_meta_f32_optional,
    read_meta_u32, read_meta_u32_optional, read_q8_0_row, rms_norm, KvCache, MlError, Result,
};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut layer: usize = 0;
    let mut seq_len: usize = 4;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--layer" => {
                layer = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            "--seq-len" => {
                seq_len = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(4);
            }
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: test_attention --gguf <path> --layer 0 --seq-len 4".to_string(),
        )
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let config_heads = read_meta_u32(&model.metadata, "llama.attention.head_count")? as usize;
    let config_kv_heads =
        read_meta_u32(&model.metadata, "llama.attention.head_count_kv")? as usize;
    let rope_dim = read_meta_u32(&model.metadata, "llama.rope.dimension_count")?;
    let rope_theta = read_meta_f32(&model.metadata, "llama.rope.freq_base")?;
    let rope_scaling_factor =
        read_meta_f32_optional(&model.metadata, "gpt-oss.rope.scaling.factor").unwrap_or(1.0);
    let rope_scaling_original_context = read_meta_u32_optional(
        &model.metadata,
        "gpt-oss.rope.scaling.original_context_length",
    )
    .unwrap_or(0);
    let rms_eps = read_meta_f32(&model.metadata, "llama.attention.layer_norm_rms_epsilon")?;
    let sliding_window = read_meta_u32(&model.metadata, "llama.sliding_window")? as usize;
    let block_count = read_meta_u32(&model.metadata, "llama.block_count")? as usize;
    if layer >= block_count {
        return Err(MlError::Model(format!("layer {layer} out of range")));
    }

    let path = PathBuf::from(&gguf_path);
    let token_embd = find_tensor(&model.index, "token_embd.weight")?;
    let attn_norm = find_tensor(&model.index, &format!("blk.{layer}.attn_norm.weight"))?;
    let attn_q_w = find_tensor(&model.index, &format!("blk.{layer}.attn_q.weight"))?;
    let attn_q_b = find_tensor(&model.index, &format!("blk.{layer}.attn_q.bias"))?;
    let attn_k_w = find_tensor(&model.index, &format!("blk.{layer}.attn_k.weight"))?;
    let attn_k_b = find_tensor(&model.index, &format!("blk.{layer}.attn_k.bias"))?;
    let attn_v_w = find_tensor(&model.index, &format!("blk.{layer}.attn_v.weight"))?;
    let attn_v_b = find_tensor(&model.index, &format!("blk.{layer}.attn_v.bias"))?;
    let attn_sinks = find_tensor(&model.index, &format!("blk.{layer}.attn_sinks.weight"))?;

    let attn_norm_w = read_f32_tensor(&path, attn_norm)?;
    let q_bias = read_f32_tensor(&path, attn_q_b)?;
    let k_bias = read_f32_tensor(&path, attn_k_b)?;
    let v_bias = read_f32_tensor(&path, attn_v_b)?;
    let sinks = read_f32_tensor(&path, attn_sinks)?;

    let mut cache = KvCache::new(block_count, seq_len.max(1));
    let mut last_q = Vec::new();
    let mut last_head_dim = 0usize;

    for pos in 0..seq_len {
        let hidden = read_q8_0_row(&path, token_embd, pos)?;
        let normed = rms_norm(&hidden, &attn_norm_w, rms_eps)?;
        let mut q = matmul_q8_0(&path, attn_q_w, &normed)?;
        let mut k = matmul_q8_0(&path, attn_k_w, &normed)?;
        let v = matmul_q8_0(&path, attn_v_w, &normed)?;
        apply_bias(&mut q, &q_bias);
        apply_bias(&mut k, &k_bias);
        let mut v = v;
        apply_bias(&mut v, &v_bias);

        let head_dim = q.len() / config_heads.max(1);
        apply_rope(
            &mut q,
            config_heads,
            head_dim,
            pos,
            rope_theta,
            rope_dim,
            rope_scaling_factor,
            rope_scaling_original_context,
        )?;
        apply_rope(
            &mut k,
            config_kv_heads,
            head_dim,
            pos,
            rope_theta,
            rope_dim,
            rope_scaling_factor,
            rope_scaling_original_context,
        )?;

        let layer_cache = cache.layer_mut(layer)?;
        layer_cache.append(&k, &v, config_kv_heads, head_dim)?;
        let seq = layer_cache.token_count(config_kv_heads, head_dim);
        let window = if sliding_window > 0 && layer % 2 == 0 {
            sliding_window
        } else {
            seq
        };
        let window = window.max(1).min(seq);
        let _attn_out = attention_with_cache(
            &q,
            layer_cache,
            &sinks,
            config_heads,
            config_kv_heads,
            head_dim,
            window,
        )?;
        last_q = q;
        last_head_dim = head_dim;
        cache.seq_len = pos + 1;
    }

    let layer_cache = cache.layer_mut(layer)?;
    let head_weights = attention_head_weights(
        &last_q,
        layer_cache,
        &sinks,
        0,
        config_heads,
        config_kv_heads,
        last_head_dim,
        layer_cache.token_count(config_kv_heads, last_head_dim),
    )?;

    let preview: Vec<String> = head_weights.iter().take(8).map(|v| format!("{v:.4}")).collect();
    println!(
        "layer={layer} seq_len={seq_len} head0_weights=[{}]",
        preview.join(", ")
    );
    Ok(())
}
