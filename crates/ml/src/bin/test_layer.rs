use std::env;
use std::path::PathBuf;

use ml::{
    apply_bias, apply_rope, attention_with_cache, find_tensor, load_gguf_model, matmul_mxfp4_expert,
    matmul_q8_0, matmul_f32, read_f32_row, read_f32_tensor, read_meta_f32, read_meta_u32,
    read_q8_0_row, rms_norm, swiglu, top_k_softmax, KvCache, MlError, Result,
};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut layer: usize = 0;
    let mut token_id: u32 = 0;
    let mut moe_fallback = false;

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
            "--token" => {
                token_id = args
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
            }
            "--moe-fallback" => moe_fallback = true,
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: test_layer --gguf <path> --layer 0 --token 0 [--moe-fallback]".to_string(),
        )
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let block_count = read_meta_u32(&model.metadata, "llama.block_count")? as usize;
    let head_count = read_meta_u32(&model.metadata, "llama.attention.head_count")? as usize;
    let head_count_kv =
        read_meta_u32(&model.metadata, "llama.attention.head_count_kv")? as usize;
    let rope_dim = read_meta_u32(&model.metadata, "llama.rope.dimension_count")?;
    let rope_theta = read_meta_f32(&model.metadata, "llama.rope.freq_base")?;
    let rms_eps = read_meta_f32(&model.metadata, "llama.attention.layer_norm_rms_epsilon")?;
    let experts_per_token = read_meta_u32(&model.metadata, "llama.expert_used_count")? as usize;
    if layer >= block_count {
        return Err(MlError::Model(format!("layer {layer} out of range")));
    }

    let path = PathBuf::from(&gguf_path);
    let token_embd = find_tensor(&model.index, "token_embd.weight")?;
    let mut hidden = read_q8_0_row(&path, token_embd, token_id as usize)?;

    let attn_norm = find_tensor(&model.index, &format!("blk.{layer}.attn_norm.weight"))?;
    let attn_q_w = find_tensor(&model.index, &format!("blk.{layer}.attn_q.weight"))?;
    let attn_q_b = find_tensor(&model.index, &format!("blk.{layer}.attn_q.bias"))?;
    let attn_k_w = find_tensor(&model.index, &format!("blk.{layer}.attn_k.weight"))?;
    let attn_k_b = find_tensor(&model.index, &format!("blk.{layer}.attn_k.bias"))?;
    let attn_v_w = find_tensor(&model.index, &format!("blk.{layer}.attn_v.weight"))?;
    let attn_v_b = find_tensor(&model.index, &format!("blk.{layer}.attn_v.bias"))?;
    let attn_out_w = find_tensor(&model.index, &format!("blk.{layer}.attn_output.weight"))?;
    let attn_out_b = find_tensor(&model.index, &format!("blk.{layer}.attn_output.bias"))?;
    let attn_sinks = find_tensor(&model.index, &format!("blk.{layer}.attn_sinks.weight"))?;
    let post_attn_norm = find_tensor(&model.index, &format!("blk.{layer}.post_attention_norm.weight"))?;

    let attn_norm_w = read_f32_tensor(&path, attn_norm)?;
    let mut normed = rms_norm(&hidden, &attn_norm_w, rms_eps)?;

    let mut q = matmul_q8_0(&path, attn_q_w, &normed)?;
    let mut k = matmul_q8_0(&path, attn_k_w, &normed)?;
    let mut v = matmul_q8_0(&path, attn_v_w, &normed)?;
    let q_bias = read_f32_tensor(&path, attn_q_b)?;
    let k_bias = read_f32_tensor(&path, attn_k_b)?;
    let v_bias = read_f32_tensor(&path, attn_v_b)?;
    apply_bias(&mut q, &q_bias);
    apply_bias(&mut k, &k_bias);
    apply_bias(&mut v, &v_bias);

    let head_dim = q.len() / head_count.max(1);
    apply_rope(&mut q, head_count, head_dim, 0, rope_theta, rope_dim)?;
    apply_rope(&mut k, head_count_kv, head_dim, 0, rope_theta, rope_dim)?;

    let sinks = read_f32_tensor(&path, attn_sinks)?;
    let mut cache = KvCache::new(block_count, 1);
    let layer_cache = cache.layer_mut(layer)?;
    layer_cache.append(&k, &v, head_count_kv, head_dim)?;
    let attn_out = attention_with_cache(
        &q,
        layer_cache,
        &sinks,
        head_count,
        head_count_kv,
        head_dim,
        1,
    )?;

    let mut attn_proj = matmul_q8_0(&path, attn_out_w, &attn_out)?;
    let attn_out_b = read_f32_tensor(&path, attn_out_b)?;
    apply_bias(&mut attn_proj, &attn_out_b);
    for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
        *out += *base;
    }

    let post_attn_norm_w = read_f32_tensor(&path, post_attn_norm)?;
    normed = rms_norm(&hidden, &post_attn_norm_w, rms_eps)?;

    let gate_inp_w = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_inp.weight"))?;
    let gate_inp_b = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_inp.bias"))?;
    let gate_w = read_f32_tensor(&path, gate_inp_w)?;
    let gate_b = read_f32_tensor(&path, gate_inp_b)?;
    let n = gate_inp_w.dims.get(0).copied().unwrap_or(0) as usize;
    let k = gate_inp_w.dims.get(1).copied().unwrap_or(0) as usize;
    if k != normed.len() || n == 0 {
        return Err(MlError::Model(format!(
            "router shape mismatch n={n} k={k} hidden={}",
            normed.len()
        )));
    }
    let mut gate_scores = matmul_f32(&gate_w, &normed, k, n);
    apply_bias(&mut gate_scores, &gate_b);
    let (mut expert_indices, mut expert_weights) =
        top_k_softmax(&gate_scores, experts_per_token.max(1))?;
    if moe_fallback {
        expert_indices = vec![0];
        expert_weights = vec![1.0];
    }

    let gate_exps_w = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_exps.weight"))?;
    let gate_exps_b = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_exps.bias"))?;
    let up_exps_w = find_tensor(&model.index, &format!("blk.{layer}.ffn_up_exps.weight"))?;
    let up_exps_b = find_tensor(&model.index, &format!("blk.{layer}.ffn_up_exps.bias"))?;
    let down_exps_w = find_tensor(&model.index, &format!("blk.{layer}.ffn_down_exps.weight"))?;
    let down_exps_b = find_tensor(&model.index, &format!("blk.{layer}.ffn_down_exps.bias"))?;

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let mut gate_out = matmul_mxfp4_expert(&path, gate_exps_w, *expert_idx, &normed)?;
        let gate_bias = read_f32_row(&path, gate_exps_b, *expert_idx)?;
        apply_bias(&mut gate_out, &gate_bias);

        let mut up_out = matmul_mxfp4_expert(&path, up_exps_w, *expert_idx, &normed)?;
        let up_bias = read_f32_row(&path, up_exps_b, *expert_idx)?;
        apply_bias(&mut up_out, &up_bias);

        let swiglu_out = swiglu(&gate_out, &up_out)?;
        let mut down_out = matmul_mxfp4_expert(&path, down_exps_w, *expert_idx, &swiglu_out)?;
        let down_bias = read_f32_row(&path, down_exps_b, *expert_idx)?;
        apply_bias(&mut down_out, &down_bias);

        for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
            *acc += *val * *weight;
        }
    }

    for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
        *out += *add;
    }

    println!(
        "layer={layer} token={token_id} attn_norm={:.4} mlp_norm={:.4} out_norm={:.4}",
        l2_norm(&attn_proj),
        l2_norm(&mlp_accum),
        l2_norm(&hidden)
    );
    Ok(())
}

fn l2_norm(values: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for v in values {
        sum += v * v;
    }
    sum.sqrt()
}
