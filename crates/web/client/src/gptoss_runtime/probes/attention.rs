struct ProbeQkv {
    q: Vec<f32>,
    k: Vec<f32>,
    v: Vec<f32>,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
}

async fn build_probe_qkv(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<ProbeQkv, String> {
    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let attn_q_w = find_tensor(index, "blk.0.attn_q.weight")?;
    let attn_q_b = find_tensor(index, "blk.0.attn_q.bias")?;
    let attn_k_w = find_tensor(index, "blk.0.attn_k.weight")?;
    let attn_k_b = find_tensor(index, "blk.0.attn_k.bias")?;
    let attn_v_w = find_tensor(index, "blk.0.attn_v.weight")?;
    let attn_v_b = find_tensor(index, "blk.0.attn_v.bias")?;

    let token_row = fetch_q8_0_row(gguf_source, token_embd, 0).await?;
    let attn_norm_w = fetch_f32_tensor_raw(gguf_source, attn_norm).await?;
    let normed = rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?;

    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let q = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_q_w,
        attn_q_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;
    let k = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_k_w,
        attn_k_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;
    let v = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_v_w,
        attn_v_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;

    let heads = config.head_count.max(1) as usize;
    let kv_heads = config.head_count_kv.max(1) as usize;
    let q_head_dim = q.len() / heads.max(1);
    let k_head_dim = k.len() / kv_heads.max(1);
    if q_head_dim == 0 || k_head_dim == 0 || q_head_dim != k_head_dim {
        return Err(format!(
            "probe q/k head dims mismatch q_dim={q_head_dim} k_dim={k_head_dim}"
        ));
    }
    let head_dim = q_head_dim;
    if q.len() != heads * head_dim || k.len() != kv_heads * head_dim || v.len() != kv_heads * head_dim
    {
        return Err("probe qkv shape mismatch".to_string());
    }

    Ok(ProbeQkv {
        q,
        k,
        v,
        heads,
        kv_heads,
        head_dim,
    })
}

fn ensure_probe_tolerance(label: &str, max_abs: f32, mean_abs: f32) -> Result<(), String> {
    if max_abs > PROBE_TOLERANCE {
        return Err(format!(
            "{label} max_abs {max_abs:.4} exceeds tolerance {PROBE_TOLERANCE:.4} (mean={mean_abs:.4})"
        ));
    }
    Ok(())
}

async fn run_rmsnorm_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "rmsnorm_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0.attn_norm".to_string()),
    );

    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let token_row = fetch_q8_0_row(gguf_source, token_embd, 0).await?;
    let attn_norm_w = fetch_f32_tensor_raw(gguf_source, attn_norm).await?;
    let cpu = rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?;
    let mut gpu_tracker = GpuAllocTracker::default();
    let gpu_out =
        rms_norm_gpu(&token_row, &attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await?;
    let (max_abs, mean_abs) = diff_stats(&cpu, &gpu_out);
    ensure_probe_tolerance("rmsnorm_probe", max_abs, mean_abs)?;

    emit_inference_stage(
        state,
        "rmsnorm_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );
    Ok(())
}

async fn run_rope_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "rope_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0.attn_q/attn_k".to_string()),
    );

    let probe = build_probe_qkv(state, gguf_source, index, config, gpu).await?;
    let mut q_cpu = probe.q.clone();
    let mut k_cpu = probe.k.clone();
    apply_rope(
        &mut q_cpu,
        probe.heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_cpu,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;

    let mut gpu_tracker = GpuAllocTracker::default();
    let q_gpu = apply_rope_gpu(
        &probe.q,
        probe.heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let k_gpu = apply_rope_gpu(
        &probe.k,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await?;

    let (q_max, q_mean) = diff_stats(&q_cpu, &q_gpu);
    let (k_max, k_mean) = diff_stats(&k_cpu, &k_gpu);
    ensure_probe_tolerance("rope_probe q", q_max, q_mean)?;
    ensure_probe_tolerance("rope_probe k", k_max, k_mean)?;

    emit_inference_stage(
        state,
        "rope_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "q_max={q_max:.4} q_mean={q_mean:.4} k_max={k_max:.4} k_mean={k_mean:.4}"
        )),
    );
    Ok(())
}

async fn run_attention_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "attn_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0 attn".to_string()),
    );

    let probe = build_probe_qkv(state, gguf_source, index, config, gpu).await?;
    let attn_sinks = find_tensor(index, "blk.0.attn_sinks.weight")?;
    let sinks = fetch_f32_tensor_raw(gguf_source, attn_sinks).await?;
    if sinks.len() < probe.heads {
        return Err("attn_probe sinks length mismatch".to_string());
    }

    let mut q_pos1 = probe.q.clone();
    let mut k_pos0 = probe.k.clone();
    let mut k_pos1 = probe.k.clone();
    apply_rope(
        &mut q_pos1,
        probe.heads,
        probe.head_dim,
        1,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_pos0,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_pos1,
        probe.kv_heads,
        probe.head_dim,
        1,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;

    let mut layer_cache = LayerKvCache::new();
    let max_len = 4usize;
    layer_cache.append(
        &k_pos0,
        &probe.v,
        probe.kv_heads,
        probe.head_dim,
        max_len,
        gpu,
        true,
    )?;
    layer_cache.append(
        &k_pos1,
        &probe.v,
        probe.kv_heads,
        probe.head_dim,
        max_len,
        gpu,
        true,
    )?;
    let window = layer_cache.token_count();
    let cpu_out = attention_with_cache(
        &q_pos1,
        &layer_cache,
        &sinks,
        probe.heads,
        probe.kv_heads,
        probe.head_dim,
        window,
    )?;
    let mut gpu_tracker = GpuAllocTracker::default();
    let gpu_out = attention_with_cache_gpu(
        &q_pos1,
        &layer_cache,
        &sinks,
        probe.heads,
        probe.kv_heads,
        probe.head_dim,
        window,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let (max_abs, mean_abs) = diff_stats(&cpu_out, &gpu_out);
    ensure_probe_tolerance("attn_probe", max_abs, mean_abs)?;

    emit_inference_stage(
        state,
        "attn_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );
    Ok(())
}

async fn run_block0_attention_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
) -> Result<(), String> {
    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let attn_q_w = find_tensor(index, "blk.0.attn_q.weight")?;
    let attn_q_b = find_tensor(index, "blk.0.attn_q.bias")?;
    let attn_k_w = find_tensor(index, "blk.0.attn_k.weight")?;
    let attn_k_b = find_tensor(index, "blk.0.attn_k.bias")?;
    let attn_v_w = find_tensor(index, "blk.0.attn_v.weight")?;
    let attn_v_b = find_tensor(index, "blk.0.attn_v.bias")?;
    let attn_out_w = find_tensor(index, "blk.0.attn_output.weight")?;
    let attn_out_b = find_tensor(index, "blk.0.attn_output.bias")?;
    let attn_sinks = find_tensor(index, "blk.0.attn_sinks.weight")?;
    let post_attn_norm = find_tensor(index, "blk.0.post_attention_norm.weight")?;
    let output_norm_tensor = find_tensor(index, "output_norm.weight")?;
    let output_weight = find_tensor(index, "output.weight")?;
    emit_tensor_resident(
        state,
        output_weight.name.clone(),
        output_weight.nbytes as usize,
        "q8_0",
    );
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();

    let token_row = match fetch_q8_0_row_gpu(gguf_url, token_embd, 0, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => fetch_q8_0_row(gguf_url, token_embd, 0).await?,
    };
    let attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, attn_norm, &mut caches.tensors).await?;
    let mut hidden =
        match rms_norm_gpu(&token_row, &attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await
        {
            Ok(value) => value,
            Err(_) => rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?,
        };

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Started,
        None,
        None,
        Some("qkv".to_string()),
    );

    let mut q = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_q_w,
        attn_q_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    let mut k = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_k_w,
        attn_k_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    let v = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_v_w,
        attn_v_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Completed,
        None,
        None,
        Some(format!("q={} k={} v={}", q.len(), k.len(), v.len())),
    );

    emit_inference_stage(
        state,
        "blk0_rope",
        StageStatus::Started,
        None,
        None,
        Some("pos=0".to_string()),
    );

    let q_head_dim = (q.len() / config.head_count.max(1) as usize).max(1);
    let k_head_dim = (k.len() / config.head_count_kv.max(1) as usize).max(1);
    let q_mode = match apply_rope_gpu(
        &q,
        config.head_count as usize,
        q_head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            q = value;
            "gpu"
        }
        Err(_) => {
            apply_rope(
                &mut q,
                config.head_count as usize,
                q_head_dim,
                0,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };
    let k_mode = match apply_rope_gpu(
        &k,
        config.head_count_kv as usize,
        k_head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            k = value;
            "gpu"
        }
        Err(_) => {
            apply_rope(
                &mut k,
                config.head_count_kv as usize,
                k_head_dim,
                0,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };

    emit_inference_stage(
        state,
        "blk0_rope",
        StageStatus::Completed,
        None,
        None,
        Some(format!("q={q_mode} k={k_mode}")),
    );

    let sinks =
        fetch_f32_tensor_cached(state, gguf_url, attn_sinks, &mut caches.tensors).await?;
    let attn_out = attention_single_token(
        &q,
        &k,
        &v,
        &sinks,
        config.head_count as usize,
        config.head_count_kv as usize,
        q_head_dim,
    )?;
    let attn_proj = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_out_w,
        attn_out_b,
        &attn_out,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    hidden = match vector_add_gpu(&hidden, &attn_proj, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => {
            for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
                *out += *base;
            }
            hidden
        }
    };

    let post_attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, post_attn_norm, &mut caches.tensors).await?;
    hidden = match rms_norm_gpu(&hidden, &post_attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
        .await
    {
        Ok(value) => value,
        Err(_) => rms_norm(&hidden, &post_attn_norm_w, config.rms_epsilon)?,
    };

    let gate_inp_w = find_tensor(index, "blk.0.ffn_gate_inp.weight")?;
    let gate_inp_b = find_tensor(index, "blk.0.ffn_gate_inp.bias")?;
    let gate_w =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_w, &mut caches.tensors).await?;
    let gate_b =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_b, &mut caches.tensors).await?;
    let gate_scores = match linear_f32_with_bias_gpu(
        &gate_w,
        &gate_b,
        &hidden,
        gate_inp_w,
        gpu,
        &mut gpu_tracker,
        true,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => linear_f32_with_bias(&gate_w, &gate_b, &hidden, gate_inp_w)?,
    };
    let (expert_indices, expert_weights) =
        top_k_softmax(&gate_scores, config.experts_per_token as usize)?;
    emit_inference_stage(
        state,
        "moe_router",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "top={:?} w={:?}",
            expert_indices, expert_weights
        )),
    );

    let gate_exps_w = find_tensor(index, "blk.0.ffn_gate_exps.weight")?;
    let gate_exps_b = find_tensor(index, "blk.0.ffn_gate_exps.bias")?;
    let up_exps_w = find_tensor(index, "blk.0.ffn_up_exps.weight")?;
    let up_exps_b = find_tensor(index, "blk.0.ffn_up_exps.bias")?;
    let down_exps_w = find_tensor(index, "blk.0.ffn_down_exps.weight")?;
    let down_exps_b = find_tensor(index, "blk.0.ffn_down_exps.bias")?;

    emit_inference_stage(
        state,
        "moe_mlp",
        StageStatus::Started,
        None,
        None,
        Some(format!("experts={}", expert_indices.len())),
    );

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let gate_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            gate_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut gate_out =
            matmul_mxfp4_expert(&gate_quant, &hidden, gate_exps_w, gpu, &mut gpu_tracker).await?;
        let gate_bias = fetch_f32_row(gguf_url, gate_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut gate_out, &gate_bias, gpu, &mut gpu_tracker, true).await?;

        let up_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            up_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut up_out =
            matmul_mxfp4_expert(&up_quant, &hidden, up_exps_w, gpu, &mut gpu_tracker).await?;
        let up_bias = fetch_f32_row(gguf_url, up_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut up_out, &up_bias, gpu, &mut gpu_tracker, true).await?;

        let swiglu_out = match swiglu_gpu(&gate_out, &up_out, gpu, &mut gpu_tracker).await {
            Ok(value) => value,
            Err(_) => swiglu(&gate_out, &up_out)?,
        };
        let down_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            down_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut down_out = matmul_mxfp4_expert(
            &down_quant,
            &swiglu_out,
            down_exps_w,
            gpu,
            &mut gpu_tracker,
        )
        .await?;
        let down_bias = fetch_f32_row(gguf_url, down_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut down_out, &down_bias, gpu, &mut gpu_tracker, true).await?;

        if let Ok(value) =
            scale_add_gpu(&mlp_accum, &down_out, *weight, gpu, &mut gpu_tracker).await
        {
            mlp_accum = value;
        } else {
            for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
                *acc += *val * *weight;
            }
        }
    }

    hidden = match vector_add_gpu(&hidden, &mlp_accum, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => {
            for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
                *out += *add;
            }
            hidden
        }
    };

    emit_inference_stage(
        state,
        "moe_mlp",
        StageStatus::Completed,
        None,
        None,
        Some("ok".to_string()),
    );

    let output_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, output_norm_tensor, &mut caches.tensors).await?;
    let final_hidden =
        match rms_norm_gpu(&hidden, &output_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await
        {
            Ok(value) => value,
            Err(_) => rms_norm(&hidden, &output_norm_w, config.rms_epsilon)?,
        };

    let attention_norm = l2_norm(&attn_proj);
    let output_norm = l2_norm(&hidden);
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::LayerActivation {
            layer: 0,
            attention_norm,
            mlp_norm: 0.0,
            output_norm,
        },
    );

    emit_inference_stage(
        state,
        "logits_probe",
        StageStatus::Started,
        None,
        None,
        Some("output.weight".to_string()),
    );

    let start_ms = now_ms();
    let logits = gpu_matmul_q8_0_chunked(
        gguf_url,
        output_weight,
        &final_hidden,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let (top_k, entropy, token_id, token_text) = top_k_from_logits(&logits, tokenizer, 5)?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    let tokens_per_sec = 1000.0 / elapsed_ms as f32;

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TokenGenerated {
            token_id,
            token_text,
            top_k,
            entropy,
            tokens_per_sec,
        },
    );

    emit_inference_stage(
        state,
        "logits_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("elapsed={}ms", elapsed_ms)),
    );

    Ok(())
}

