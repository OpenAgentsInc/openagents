async fn run_q8_0_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
) -> Result<(), String> {
    let tensor = index
        .tensors
        .iter()
        .find(|t| t.name == "output.weight")
        .or_else(|| index.tensors.iter().find(|t| t.ggml_type == 8))
        .ok_or_else(|| "no Q8_0 tensor found".to_string())?;

    let dims = &tensor.dims;
    let mut n = dims.get(0).copied().unwrap_or(64) as usize;
    let mut k = dims.get(1).copied().unwrap_or(128) as usize;
    n = n.min(64).max(1);
    k = k.min(128).max(1);
    let mut values = k * n;
    if values % Q8_0_BLOCK_VALUES != 0 {
        let rem = values % Q8_0_BLOCK_VALUES;
        if n > rem {
            n -= rem;
        }
        values = k * n;
    }
    if values == 0 || values % Q8_0_BLOCK_VALUES != 0 {
        return Err("invalid q8_0 probe shape".to_string());
    }

    let blocks = values / Q8_0_BLOCK_VALUES;
    let bytes_needed = blocks * Q8_0_BLOCK_BYTES;
    if bytes_needed as u64 > tensor.nbytes {
        return Err("q8_0 probe slice exceeds tensor size".to_string());
    }

    emit_inference_stage(
        state,
        "q8_0_probe",
        StageStatus::Started,
        None,
        None,
        Some(format!("tensor={}", tensor.name)),
    );

    let mut quant =
        fetch_range_source(gguf_source, tensor.absolute_offset, bytes_needed as u64).await?;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    let x = build_input(k);
    let weights = dequant_q8_0(&quant, values)?;
    let y_cpu = matmul_cpu(&weights, &x, k, n);

    let mut gpu_tracker = GpuAllocTracker::default();
    let y_gpu = gpu_matmul_q8_0(&quant, &x, k, n, gpu, &mut gpu_tracker).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::MemoryUsage {
            gpu_allocated: gpu_tracker.bytes,
            cache_total: 0,
            activations: 0,
        },
    );

    emit_inference_stage(
        state,
        "q8_0_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );

    Ok(())
}

async fn run_mxfp4_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
) -> Result<(), String> {
    let tensor = index
        .tensors
        .iter()
        .find(|t| t.name == "blk.0.ffn_gate_exps.weight")
        .or_else(|| index.tensors.iter().find(|t| t.ggml_type == 39))
        .ok_or_else(|| "no MXFP4 tensor found".to_string())?;

    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }

    if tensor.dims.len() != 3 {
        return Err(format!(
            "mxfp4 tensor {} dims {:?} expected 3d",
            tensor.name, tensor.dims
        ));
    }

    let experts = tensor.dims[0] as usize;
    let rows = tensor.dims[1] as usize;
    let cols = tensor.dims[2] as usize;
    if experts == 0 || rows == 0 || cols == 0 {
        return Err("mxfp4 tensor has empty dims".to_string());
    }

    let expert_idx = 0usize;
    let k = rows.min(32).max(1);
    let n = cols;
    let values = k
        .checked_mul(n)
        .ok_or_else(|| "mxfp4 probe shape overflow".to_string())?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 probe values not divisible by block size".to_string());
    }

    let expert_values = rows
        .checked_mul(cols)
        .ok_or_else(|| "mxfp4 expert shape overflow".to_string())?;
    if expert_values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 expert values not divisible by block size".to_string());
    }

    let expert_blocks = expert_values / MXFP4_BLOCK_VALUES;
    let expert_bytes = expert_blocks * MXFP4_BLOCK_BYTES;
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    if bytes_needed > expert_bytes {
        return Err("mxfp4 probe slice exceeds expert size".to_string());
    }

    emit_inference_stage(
        state,
        "mxfp4_probe",
        StageStatus::Started,
        None,
        None,
        Some(format!(
            "tensor={} expert={} of {} k={} n={}",
            tensor.name, expert_idx, experts, k, n
        )),
    );

    let offset = tensor
        .absolute_offset
        .saturating_add((expert_idx * expert_bytes) as u64);
    let mut quant = fetch_range_source(gguf_source, offset, bytes_needed as u64).await?;
    emit_inference_stage(
        state,
        "mxfp4_header",
        StageStatus::Completed,
        None,
        None,
        Some(hex_preview(&quant, 64)),
    );
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    let x = build_input(k);
    let weights = dequant_mxfp4(&quant, values)?;
    let y_cpu = matmul_cpu(&weights, &x, k, n);
    let mut gpu_tracker = GpuAllocTracker::default();
    let y_gpu = gpu_matmul_mxfp4(&quant, &x, k, n, gpu, &mut gpu_tracker).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    emit_inference_stage(
        state,
        "mxfp4_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );

    Ok(())
}

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

async fn run_generation(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
    prompt_tokens: &[u32],
    active_layers: usize,
    moe_fallback: bool,
    max_kv_tokens: usize,
    max_new_tokens: usize,
    force_dense: bool,
    sampling: SamplingConfig,
    stop_tokens: Vec<u32>,
) -> Result<(), String> {
    if prompt_tokens.is_empty() {
        return Err("prompt token list is empty".to_string());
    }
    let allow_cpu_fallback = false;
    let generation_start_ms = now_ms();

    emit_inference_stage(
        state,
        "generation",
        StageStatus::Started,
        None,
        None,
        Some(format!("prompt_tokens={} new={max_new_tokens}", prompt_tokens.len())),
    );

    let total_prefill = prompt_tokens.len();
    let mut cache = KvCache::new(config.block_count as usize, max_kv_tokens);
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let mut last_logits: Option<Vec<f32>> = None;
    let mut last_step_ms = 1u64;
    let attention_mode = if force_dense {
        "dense (override)".to_string()
    } else if config.sliding_window > 0 {
        format!("window={} even", config.sliding_window)
    } else {
        "dense".to_string()
    };
    let moe_mode = if moe_fallback {
        "fallback expert=0".to_string()
    } else if config.expert_count > 0 {
        format!("experts={} topk={}", config.expert_count, config.experts_per_token)
    } else {
        "disabled".to_string()
    };
    let sample_mode = if sampling.enabled {
        let top_k_label = if sampling.top_k == 0 {
            "auto".to_string()
        } else {
            sampling.top_k.to_string()
        };
        format!(
            "temp={:.2},top_k={},top_p={:.2}",
            sampling.temperature, top_k_label, sampling.top_p
        )
    } else {
        "greedy".to_string()
    };
    emit_inference_stage(
        state,
        "runtime_mode",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "layers={active_layers} attn={attention_mode} moe={moe_mode} sample={sample_mode} kv_max={max_kv_tokens} new={max_new_tokens} cpu_fallback={}",
            if allow_cpu_fallback { "on" } else { "off" }
        )),
    );

    emit_inference_stage(
        state,
        "prefill",
        StageStatus::Started,
        Some(0),
        Some(total_prefill),
        Some(format!("tokens={total_prefill}")),
    );
    let prefill_start_ms = now_ms();

    for (idx, &token_id) in prompt_tokens.iter().enumerate() {
        let position = cache.seq_len;
        if position >= cache.max_len {
            return Err("kv cache max length exceeded".to_string());
        }
        let start_ms = now_ms();
        let logits = run_forward_token(
            state,
            gguf_url,
            index,
            gpu,
            config,
            token_id,
            position,
            &mut cache,
            &mut caches,
            &mut gpu_tracker,
            active_layers,
            moe_fallback,
            force_dense,
            true,
            allow_cpu_fallback,
        )
        .await?;
        last_step_ms = now_ms().saturating_sub(start_ms).max(1);
        last_logits = Some(logits);
        emit_inference_stage(
            state,
            "prefill",
            StageStatus::Progress,
            Some(idx + 1),
            Some(total_prefill),
            Some(format!("token_id={token_id}")),
        );
        yield_to_browser().await;
    }

    let prefill_ms = now_ms().saturating_sub(prefill_start_ms).max(1);
    let prefill_tok_s = if total_prefill > 0 {
        total_prefill as f32 / (prefill_ms as f32 / 1000.0)
    } else {
        0.0
    };
    emit_inference_stage(
        state,
        "prefill",
        StageStatus::Completed,
        Some(total_prefill),
        Some(total_prefill),
        Some(format!("ok ms={prefill_ms} tok/s={prefill_tok_s:.1}")),
    );

    let mut logits = last_logits.ok_or_else(|| "prefill produced no logits".to_string())?;
    let mut generated = 0usize;
    let mut stop_reason = "max_new".to_string();
    let mut decoded = String::new();

    emit_inference_stage(
        state,
        "decode",
        StageStatus::Started,
        Some(0),
        Some(max_new_tokens),
        None,
    );
    let decode_start_ms = now_ms();

    while generated < max_new_tokens {
        let (top_k, entropy, next_id, next_text) =
            sample_from_logits(&logits, tokenizer, sampling, 5)?;
        let stop_token = stop_tokens.contains(&next_id);
        let token_text = if stop_token {
            String::new()
        } else {
            next_text
        };
        if !stop_token {
            decoded.push_str(&token_text);
        }
        let tokens_per_sec = 1000.0 / last_step_ms as f32;

        emit_inference_event(
            state,
            GptOssInferenceTelemetry::TokenGenerated {
                token_id: next_id,
                token_text,
                top_k,
                entropy,
                tokens_per_sec,
            },
        );

        generated += 1;
        emit_inference_stage(
            state,
            "decode",
            StageStatus::Progress,
            Some(generated),
            Some(max_new_tokens),
            Some(format!("token_id={next_id}")),
        );
        yield_to_browser().await;

        if stop_token {
            stop_reason = "stop_token".to_string();
            break;
        }

        let position = cache.seq_len;
        let start_ms = now_ms();
        logits = run_forward_token(
            state,
            gguf_url,
            index,
            gpu,
            config,
            next_id,
            position,
            &mut cache,
            &mut caches,
            &mut gpu_tracker,
            active_layers,
            moe_fallback,
            force_dense,
            false,
            allow_cpu_fallback,
        )
        .await?;
        last_step_ms = now_ms().saturating_sub(start_ms).max(1);
    }

    let decode_ms = now_ms().saturating_sub(decode_start_ms).max(1);
    let decode_tok_s = if generated > 0 {
        generated as f32 / (decode_ms as f32 / 1000.0)
    } else {
        0.0
    };
    emit_inference_stage(
        state,
        "decode",
        StageStatus::Completed,
        Some(generated),
        Some(max_new_tokens),
        Some(format!(
            "{stop_reason} ms={decode_ms} tok/s={decode_tok_s:.1}",
        )),
    );

    let avg_ms = if generated > 0 {
        decode_ms / generated as u64
    } else {
        0
    };
    let budget_label = if generated == 0 || avg_ms <= 30_000 {
        "ok"
    } else {
        "slow"
    };
    emit_inference_stage(
        state,
        "decode_budget",
        StageStatus::Completed,
        None,
        None,
        Some(format!("avg_ms={avg_ms} label={budget_label}")),
    );

    let coherence = coherence_score(&decoded);
    let coherence_label = if decoded.is_empty() {
        "empty"
    } else if coherence >= 0.6 {
        "ok"
    } else {
        "low"
    };
    emit_inference_stage(
        state,
        "coherence_check",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "score={coherence:.2} label={coherence_label} chars={}",
            decoded.len()
        )),
    );

    let generation_ms = now_ms().saturating_sub(generation_start_ms).max(1);
    emit_inference_stage(
        state,
        "generation",
        StageStatus::Completed,
        Some(generated),
        Some(max_new_tokens),
        Some(format!("{stop_reason} ms={generation_ms}")),
    );

    Ok(())
}

async fn run_forward_token(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    config: &GptOssConfig,
    token_id: u32,
    position: usize,
    cache: &mut KvCache,
    caches: &mut RuntimeCaches,
    gpu_tracker: &mut GpuAllocTracker,
    active_layers: usize,
    moe_fallback: bool,
    force_dense: bool,
    is_prefill: bool,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    gpu_tracker.reset();
    let token_embd = find_tensor(index, "token_embd.weight")?;
    emit_inference_stage(
        state,
        "token_embd",
        StageStatus::Started,
        None,
        None,
        Some(format!("token_id={token_id}")),
    );
    let emb_start = now_ms();
    let (mut hidden, emb_hit, emb_mode) =
        match fetch_q8_0_row_cached_gpu(
            gguf_url,
            token_embd,
            token_id as usize,
            &mut caches.token_embd,
            gpu,
            gpu_tracker,
        )
        .await
        {
            Ok((data, hit)) => {
                let mode = if hit { "cache" } else { "gpu" };
                (data, hit, mode)
            }
            Err(err) => {
                if !allow_cpu_fallback {
                    return Err(format!("token_embd gpu failed: {err}"));
                }
                let (data, hit) = fetch_q8_0_row_cached(
                    gguf_url,
                    token_embd,
                    token_id as usize,
                    &mut caches.token_embd,
                )
                .await?;
                let mode = if hit { "cache" } else { "cpu" };
                (data, hit, mode)
            }
        };
    let emb_ms = now_ms().saturating_sub(emb_start).max(1);
    emit_inference_stage(
        state,
        "token_embd",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "token_id={token_id} {emb_mode} ms={emb_ms} cache={}",
            if emb_hit { "hit" } else { "miss" }
        )),
    );

    let layer_limit = active_layers.min(config.block_count as usize);
    let mut applied_layers = 0usize;
    for layer in 0..layer_limit as u32 {
        let fallback = hidden.clone();
        match run_transformer_layer(
            state,
            gguf_url,
            index,
            gpu,
            config,
            layer,
            position,
            hidden,
            cache,
            caches,
            gpu_tracker,
            layer_limit,
            moe_fallback,
            force_dense,
            is_prefill,
            allow_cpu_fallback,
        )
        .await
        {
            Ok(next_hidden) => {
                hidden = next_hidden;
                applied_layers = applied_layers.saturating_add(1);
            }
            Err(err) => {
                hidden = fallback;
                emit_inference_stage(
                    state,
                    "layer_fallback",
                    StageStatus::Failed,
                    Some(applied_layers),
                    Some(layer_limit),
                    Some(format!("layer={layer} err={err}")),
                );
                break;
            }
        }
        yield_to_browser().await;
    }
    if applied_layers < layer_limit {
        emit_inference_stage(
            state,
            "layer_fallback",
            StageStatus::Completed,
            Some(applied_layers),
            Some(layer_limit),
            Some("skipped remaining layers".to_string()),
        );
    }

    let output_norm_tensor = find_tensor(index, "output_norm.weight")?;
    let output_norm_start = now_ms();
    let output_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, output_norm_tensor, &mut caches.tensors).await?;
    let (final_hidden, output_norm_mode) = match rms_norm_gpu(
        &hidden,
        &output_norm_w,
        config.rms_epsilon,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("output_norm gpu failed: {err}"));
            }
            (rms_norm(&hidden, &output_norm_w, config.rms_epsilon)?, "cpu")
        }
    };
    let output_norm_ms = now_ms().saturating_sub(output_norm_start).max(1);
    emit_inference_stage(
        state,
        "output_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{output_norm_mode} ms={output_norm_ms}")),
    );

    let output_weight = find_tensor(index, "output.weight")?;
    let logits_start = now_ms();
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!("{} bytes={}", output_weight.name, format_bytes(output_weight.nbytes))),
    );
    let logits = gpu_matmul_q8_0_chunked(
        gguf_url,
        output_weight,
        &final_hidden,
        gpu,
        gpu_tracker,
    )
    .await?;
    let logits_ms = now_ms().saturating_sub(logits_start).max(1);
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} ok ms={logits_ms}", output_weight.name)),
    );

    cache.seq_len = position.saturating_add(1);
    let token_stats = caches.token_embd.stats();
    let tensor_stats = caches.tensors.stats();
    let quant_stats = caches.quant.stats();
    let expert_stats = caches.experts.stats();
    emit_inference_stage(
        state,
        "token_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            token_stats.hits,
            token_stats.misses,
            token_stats.evictions,
            token_stats.skipped,
            format_bytes(token_stats.bytes as u64),
            token_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "tensor_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            tensor_stats.hits,
            tensor_stats.misses,
            tensor_stats.evictions,
            tensor_stats.skipped,
            format_bytes(tensor_stats.bytes as u64),
            tensor_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "q8_0_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            quant_stats.hits,
            quant_stats.misses,
            quant_stats.evictions,
            quant_stats.skipped,
            format_bytes(quant_stats.bytes as u64),
            quant_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "expert_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            expert_stats.hits,
            expert_stats.misses,
            expert_stats.evictions,
            expert_stats.skipped,
            format_bytes(expert_stats.bytes as u64),
            expert_stats.entries
        )),
    );
    let gpu_bytes = gpu_tracker.bytes.saturating_add(cache.total_bytes());
    emit_inference_stage(
        state,
        "gpu_alloc",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "bytes={} buffers={} kv={}",
            format_bytes(gpu_bytes as u64),
            gpu_tracker.buffers,
            format_bytes(cache.total_bytes() as u64)
        )),
    );
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::MemoryUsage {
            gpu_allocated: gpu_bytes,
            cache_total: cache.total_bytes()
                + token_stats.bytes
                + tensor_stats.bytes
                + quant_stats.bytes
                + expert_stats.bytes,
            activations: final_hidden.len() * std::mem::size_of::<f32>(),
        },
    );

    Ok(logits)
}

async fn run_single_token_full(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
    token_id: u32,
) -> Result<(), String> {
    let mut cache = KvCache::new(config.block_count as usize, 1);
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let logits = run_forward_token(
        state,
        gguf_url,
        index,
        gpu,
        config,
        token_id,
        0,
        &mut cache,
        &mut caches,
        &mut gpu_tracker,
        config.block_count as usize,
        false,
        false,
        true,
        true,
    )
    .await?;
    let (top_k, entropy, next_id, next_text) = top_k_from_logits(&logits, tokenizer, 5)?;
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TokenGenerated {
            token_id: next_id,
            token_text: next_text,
            top_k,
            entropy,
            tokens_per_sec: 0.0,
        },
    );
    Ok(())
}

async fn run_transformer_layer(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    config: &GptOssConfig,
    layer: u32,
    position: usize,
    mut hidden: Vec<f32>,
    cache: &mut KvCache,
    caches: &mut RuntimeCaches,
    gpu_tracker: &mut GpuAllocTracker,
    total_layers: usize,
    moe_fallback: bool,
    force_dense: bool,
    is_prefill: bool,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
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
    let post_attn_norm = find_tensor(index, &format!("blk.{layer}.post_attention_norm.weight"))?;

    let layer_attn_start = now_ms();
    emit_inference_stage(
        state,
        "layer_attn",
        StageStatus::Started,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("layer={layer}")),
    );

    let attn_norm_start = now_ms();
    let attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, attn_norm, &mut caches.tensors).await?;
    let (mut normed, attn_norm_mode) =
        match rms_norm_gpu(&hidden, &attn_norm_w, config.rms_epsilon, gpu, gpu_tracker).await {
            Ok(value) => (value, "gpu"),
            Err(err) => {
                if !allow_cpu_fallback {
                    return Err(format!("attn_norm gpu failed: {err}"));
                }
                (rms_norm(&hidden, &attn_norm_w, config.rms_epsilon)?, "cpu")
            }
        };
    let attn_norm_ms = now_ms().saturating_sub(attn_norm_start).max(1);
    emit_inference_stage(
        state,
        "attn_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {attn_norm_mode} ms={attn_norm_ms}")),
    );

    let q_start = now_ms();
    let mut q = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_q_w,
        attn_q_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let q_ms = now_ms().saturating_sub(q_start).max(1);
    emit_inference_stage(
        state,
        "attn_q",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={q_ms}")),
    );
    let k_start = now_ms();
    let mut k = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_k_w,
        attn_k_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let k_ms = now_ms().saturating_sub(k_start).max(1);
    emit_inference_stage(
        state,
        "attn_k",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={k_ms}")),
    );
    let v_start = now_ms();
    let v = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_v_w,
        attn_v_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let v_ms = now_ms().saturating_sub(v_start).max(1);
    emit_inference_stage(
        state,
        "attn_v",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={v_ms}")),
    );

    let heads = config.head_count as usize;
    let kv_heads = config.head_count_kv as usize;
    let q_head_dim = q.len() / heads.max(1);
    let k_head_dim = k.len() / kv_heads.max(1);
    if q_head_dim == 0 || k_head_dim == 0 || q_head_dim != k_head_dim {
        return Err(format!(
            "attention head dim mismatch q_dim={q_head_dim} k_dim={k_head_dim}"
        ));
    }
    let head_dim = q_head_dim;
    let rope_start = now_ms();
    let q_mode = match apply_rope_gpu(
        &q,
        heads,
        q_head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            q = value;
            "gpu"
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("rope q gpu failed: {err}"));
            }
            apply_rope(
                &mut q,
                heads,
                q_head_dim,
                position,
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
        kv_heads,
        k_head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            k = value;
            "gpu"
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("rope k gpu failed: {err}"));
            }
            apply_rope(
                &mut k,
                kv_heads,
                k_head_dim,
                position,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };
    let rope_ms = now_ms().saturating_sub(rope_start).max(1);
    emit_inference_stage(
        state,
        "rope",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "layer={layer} q={q_mode} k={k_mode} ms={rope_ms}"
        )),
    );

    let sinks =
        fetch_f32_tensor_cached(state, gguf_url, attn_sinks, &mut caches.tensors).await?;
    let max_len = cache.max_len;
    let layer_cache = cache.layer_mut(layer as usize)?;
    layer_cache.append(
        &k,
        &v,
        kv_heads,
        head_dim,
        max_len,
        gpu,
        allow_cpu_fallback,
    )?;
    let seq_len = layer_cache.token_count();
    let window = if force_dense || config.sliding_window == 0 {
        seq_len
    } else if layer % 2 == 0 {
        config.sliding_window as usize
    } else {
        seq_len
    };
    let window = window.max(1).min(seq_len);
    let offset = if layer_cache.capacity > 0 {
        (layer_cache.start + seq_len.saturating_sub(window)) % layer_cache.capacity
    } else {
        0
    };
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::CacheStatus {
            layer: layer as usize,
            seq_len,
            max_len,
            offset,
            memory_bytes: layer_cache.memory_bytes(),
        },
    );

    let attn_start = now_ms();
    let mut attn_fallback = false;
    let mut attn_mode = "gpu";
    let phase = if is_prefill { "prefill" } else { "decode" };
    let attn_out = match attention_with_cache_gpu(
        &q,
        layer_cache,
        &sinks,
        heads,
        kv_heads,
        head_dim,
        window,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(out) => out,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("gpu attention failed: {err}"));
            }
            attn_mode = "cpu";
            match attention_with_cache(
                &q,
                layer_cache,
                &sinks,
                heads,
                kv_heads,
                head_dim,
                window,
            ) {
                Ok(out) => out,
                Err(err) => {
                    attn_fallback = true;
                    emit_inference_stage(
                        state,
                        "attn_fallback",
                        StageStatus::Completed,
                        None,
                        None,
                        Some(format!("layer={layer} phase={phase} fallback=single_token err={err}")),
                    );
                    attention_single_token(&q, &k, &v, &sinks, heads, kv_heads, head_dim)?
                }
            }
        }
    };
    let attn_ms = now_ms().saturating_sub(attn_start).max(1);
    let attn_detail = if attn_fallback {
        format!("layer={layer} {attn_mode} phase={phase} window={window} ms={attn_ms} fallback")
    } else {
        format!("layer={layer} {attn_mode} phase={phase} window={window} ms={attn_ms}")
    };
    emit_inference_stage(
        state,
        "attn_score",
        StageStatus::Completed,
        None,
        None,
        Some(attn_detail),
    );
    let (selected_layer, selected_head) = state
        .try_borrow()
        .ok()
        .map(|guard| {
            (
                guard.gptoss.attention_selected_layer,
                guard.gptoss.attention_selected_head,
            )
        })
        .unwrap_or((0, 0));
    if selected_layer == layer as usize {
        if let Ok(weights) = attention_head_weights_gpu(
            &q,
            layer_cache,
            &sinks,
            selected_head,
            heads,
            kv_heads,
            head_dim,
            window,
            gpu,
            gpu_tracker,
        )
        .await
        {
            emit_inference_event(
                state,
                GptOssInferenceTelemetry::AttentionWeights {
                    layer: layer as usize,
                    head: selected_head,
                    weights: vec![weights],
                },
            );
        } else if allow_cpu_fallback {
            if let Ok(weights) = attention_head_weights(
                &q,
                layer_cache,
                &sinks,
                selected_head,
                heads,
                kv_heads,
                head_dim,
                window,
            ) {
                emit_inference_event(
                    state,
                    GptOssInferenceTelemetry::AttentionWeights {
                        layer: layer as usize,
                        head: selected_head,
                        weights: vec![weights],
                    },
                );
            }
        }
    }

    let proj_start = now_ms();
    let attn_proj = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_out_w,
        attn_out_b,
        &attn_out,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let proj_ms = now_ms().saturating_sub(proj_start).max(1);
    emit_inference_stage(
        state,
        "attn_out",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={proj_ms}")),
    );
    hidden = match vector_add_gpu(&hidden, &attn_proj, gpu, gpu_tracker).await {
        Ok(value) => value,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("attn_residual gpu failed: {err}"));
            }
            for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
                *out += *base;
            }
            hidden
        }
    };

    let layer_attn_ms = now_ms().saturating_sub(layer_attn_start).max(1);
    emit_inference_stage(
        state,
        "layer_attn",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("ok ms={layer_attn_ms}")),
    );

    let post_norm_start = now_ms();
    let post_attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, post_attn_norm, &mut caches.tensors).await?;
    let (next_normed, post_norm_mode) = match rms_norm_gpu(
        &hidden,
        &post_attn_norm_w,
        config.rms_epsilon,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("post_attn_norm gpu failed: {err}"));
            }
            (rms_norm(&hidden, &post_attn_norm_w, config.rms_epsilon)?, "cpu")
        }
    };
    normed = next_normed;
    let post_norm_ms = now_ms().saturating_sub(post_norm_start).max(1);
    emit_inference_stage(
        state,
        "post_attn_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {post_norm_mode} ms={post_norm_ms}")),
    );

    let layer_mlp_start = now_ms();
    emit_inference_stage(
        state,
        "layer_mlp",
        StageStatus::Started,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("layer={layer}")),
    );

    let gate_inp_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.weight"))?;
    let gate_inp_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.bias"))?;
    let gate_start = now_ms();
    let gate_w =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_w, &mut caches.tensors).await?;
    let gate_b =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_b, &mut caches.tensors).await?;
    let (gate_scores, gate_mode) = match linear_f32_with_bias_gpu(
        &gate_w,
        &gate_b,
        &normed,
        gate_inp_w,
        gpu,
        gpu_tracker,
        allow_cpu_fallback,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("moe_gate gpu failed: {err}"));
            }
            (
                linear_f32_with_bias(&gate_w, &gate_b, &normed, gate_inp_w)?,
                "cpu",
            )
        }
    };
    let gate_ms = now_ms().saturating_sub(gate_start).max(1);
    emit_inference_stage(
        state,
        "moe_gate",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {gate_mode} ms={gate_ms}")),
    );
    let moe_fallback_active = moe_fallback || caches.moe_disabled;
    let (expert_indices, expert_weights) = if moe_fallback_active {
        (vec![0usize], vec![1.0f32])
    } else {
        top_k_softmax(&gate_scores, config.experts_per_token as usize)?
    };
    emit_inference_stage(
        state,
        "moe_router",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!(
            "layer={layer} experts={:?}",
            expert_indices
        )),
    );

    let gate_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.weight"))?;
    let gate_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.bias"))?;
    let up_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.weight"))?;
    let up_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.bias"))?;
    let down_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.weight"))?;
    let down_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.bias"))?;

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    let mut moe_error: Option<String> = None;
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let expert_start = now_ms();
        let gate_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            gate_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut gate_out =
            match matmul_mxfp4_expert(&gate_quant, &normed, gate_exps_w, gpu, gpu_tracker).await {
                Ok(value) => value,
                Err(err) => {
                    moe_error = Some(err);
                    break;
                }
            };
        let gate_bias = match fetch_f32_row(gguf_url, gate_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut gate_out, &gate_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        let up_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            up_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut up_out =
            match matmul_mxfp4_expert(&up_quant, &normed, up_exps_w, gpu, gpu_tracker).await {
                Ok(value) => value,
                Err(err) => {
                    moe_error = Some(err);
                    break;
                }
            };
        let up_bias = match fetch_f32_row(gguf_url, up_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut up_out, &up_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        let swiglu_out = match swiglu_gpu(&gate_out, &up_out, gpu, gpu_tracker).await {
            Ok(value) => Some(value),
            Err(err) => {
                if !allow_cpu_fallback {
                    moe_error = Some(format!("swiglu gpu failed: {err}"));
                    None
                } else {
                    match swiglu(&gate_out, &up_out) {
                        Ok(value) => Some(value),
                        Err(err) => {
                            moe_error = Some(err);
                            None
                        }
                    }
                }
            }
        };
        let swiglu_out = match swiglu_out {
            Some(value) => value,
            None => break,
        };
        let down_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            down_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut down_out = match matmul_mxfp4_expert(
            &down_quant,
            &swiglu_out,
            down_exps_w,
            gpu,
            gpu_tracker,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let down_bias = match fetch_f32_row(gguf_url, down_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut down_out, &down_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        match scale_add_gpu(&mlp_accum, &down_out, *weight, gpu, gpu_tracker).await {
            Ok(value) => {
                mlp_accum = value;
            }
            Err(err) => {
                if !allow_cpu_fallback {
                    moe_error = Some(format!("moe_accum gpu failed: {err}"));
                    break;
                }
                for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
                    *acc += *val * *weight;
                }
            }
        }
        let expert_ms = now_ms().saturating_sub(expert_start).max(1);
        emit_inference_stage(
            state,
            "moe_expert",
            StageStatus::Completed,
            None,
            None,
            Some(format!("layer={layer} expert={expert_idx} ms={expert_ms}")),
        );
    }
    if let Some(err) = moe_error {
        caches.moe_disabled = true;
        emit_inference_stage(
            state,
            "moe_expert",
            StageStatus::Failed,
            None,
            None,
            Some(format!("layer={layer} err={err}")),
        );
        emit_inference_stage(
            state,
            "moe_mode",
            StageStatus::Completed,
            None,
            None,
            Some("fallback (mlp skipped)".to_string()),
        );
    }

    hidden = match vector_add_gpu(&hidden, &mlp_accum, gpu, gpu_tracker).await {
        Ok(value) => value,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("mlp_residual gpu failed: {err}"));
            }
            for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
                *out += *add;
            }
            hidden
        }
    };

    let layer_mlp_ms = now_ms().saturating_sub(layer_mlp_start).max(1);
    emit_inference_stage(
        state,
        "layer_mlp",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("ok ms={layer_mlp_ms}")),
    );

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::LayerActivation {
            layer: layer as usize,
            attention_norm: l2_norm(&attn_proj),
            mlp_norm: l2_norm(&mlp_accum),
            output_norm: l2_norm(&hidden),
        },
    );

    Ok(hidden)
}

fn build_input(k: usize) -> Vec<f32> {
    let mut x = Vec::with_capacity(k);
    for i in 0..k {
        let step = (i % 13) as f32 - 6.0;
        x.push(step * 0.01);
    }
    x
}

fn find_tensor<'a>(index: &'a GgufIndex, name: &str) -> Result<&'a GgufTensor, String> {
    index
        .tensors
        .iter()
        .find(|tensor| tensor.name == name)
        .ok_or_else(|| format!("tensor not found: {name}"))
}

async fn fetch_f32_tensor_raw(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 0 {
        return Err(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let bytes = fetch_range_source(gguf_url, tensor.absolute_offset, tensor.nbytes).await?;
    if bytes.len() % 4 != 0 {
        return Err(format!("tensor {} f32 byte len mismatch", tensor.name));
    }
    let mut floats = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

async fn fetch_f32_tensor_cached(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    cache: &mut TensorCache,
) -> Result<Vec<f32>, String> {
    if let Some(hit) = cache.get(&tensor.name) {
        emit_inference_stage(
            state,
            "tensor_fetch",
            StageStatus::Completed,
            None,
            None,
            Some(format!("{} cache=hit", tensor.name)),
        );
        return Ok(hit);
    }
    let start_ms = now_ms();
    emit_inference_stage(
        state,
        "tensor_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!(
            "{} bytes={}",
            tensor.name,
            format_bytes(tensor.nbytes)
        )),
    );
    let data = fetch_f32_tensor_raw(gguf_url, tensor).await?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_tensor_resident(state, tensor.name.clone(), data.len() * 4, "f32");
    cache.insert(tensor.name.clone(), data.clone());
    emit_inference_stage(
        state,
        "tensor_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} cache=miss ms={elapsed_ms}", tensor.name)),
    );
    Ok(data)
}

async fn fetch_f32_row(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 0 {
        return Err(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let rows = dims.get(0).copied().unwrap_or(0) as usize;
    let cols = dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(format!("row {row} out of range for {}", tensor.name));
    }
    let row_bytes = cols * 4;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    if bytes.len() % 4 != 0 {
        return Err(format!("tensor {} f32 row len mismatch", tensor.name));
    }
    let mut floats = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

async fn fetch_q8_0_row(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let rows = dims.get(0).copied().unwrap_or(0) as usize;
    let cols = dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(format!("row {row} out of range for {}", tensor.name));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err("q8_0 row cols not divisible by block size".to_string());
    }
    let blocks_per_row = cols / Q8_0_BLOCK_VALUES;
    let row_bytes = blocks_per_row * Q8_0_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    let values = cols;
    let mut quant = bytes;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    dequant_q8_0(&quant, values)
}

async fn fetch_q8_0_row_gpu(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let rows = dims.get(0).copied().unwrap_or(0) as usize;
    let cols = dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(format!("row {row} out of range for {}", tensor.name));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err("q8_0 row cols not divisible by block size".to_string());
    }
    let blocks_per_row = cols / Q8_0_BLOCK_VALUES;
    let row_bytes = blocks_per_row * Q8_0_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    let mut quant = bytes;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    dequant_q8_0_gpu(&quant, cols, gpu, gpu_tracker).await
}

async fn fetch_q8_0_row_cached(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    cache: &mut TokenCache,
) -> Result<(Vec<f32>, bool), String> {
    if let Ok(token_id) = u32::try_from(row) {
        if let Some(hit) = cache.get(token_id) {
            return Ok((hit, true));
        }
        let data = fetch_q8_0_row(gguf_url, tensor, row).await?;
        cache.insert(token_id, data.clone());
        return Ok((data, false));
    }
    let data = fetch_q8_0_row(gguf_url, tensor, row).await?;
    Ok((data, false))
}

async fn fetch_q8_0_row_cached_gpu(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    cache: &mut TokenCache,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<(Vec<f32>, bool), String> {
    if let Ok(token_id) = u32::try_from(row) {
        if let Some(hit) = cache.get(token_id) {
            return Ok((hit, true));
        }
        let data = fetch_q8_0_row_gpu(gguf_url, tensor, row, gpu, gpu_tracker).await?;
        cache.insert(token_id, data.clone());
        return Ok((data, false));
    }
    let data = fetch_q8_0_row_gpu(gguf_url, tensor, row, gpu, gpu_tracker).await?;
    Ok((data, false))
}

async fn fetch_mxfp4_expert_raw(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    expert_idx: usize,
) -> Result<Vec<u8>, String> {
    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let experts = dims.get(0).copied().unwrap_or(0) as usize;
    let n = dims.get(1).copied().unwrap_or(0) as usize;
    let k = dims.get(2).copied().unwrap_or(0) as usize;
    if expert_idx >= experts || n == 0 || k == 0 {
        return Err(format!("expert {expert_idx} out of range for {}", tensor.name));
    }
    let values = n
        .checked_mul(k)
        .ok_or_else(|| "mxfp4 expert value overflow".to_string())?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 expert values not divisible by block size".to_string());
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((expert_idx * bytes_needed) as u64);
    let mut bytes = fetch_range_source(gguf_url, offset, bytes_needed as u64).await?;
    if bytes.len() % 4 != 0 {
        let padded = (bytes.len() + 3) / 4 * 4;
        bytes.resize(padded, 0);
    }
    Ok(bytes)
}

async fn fetch_mxfp4_expert_cached(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    expert_idx: usize,
    cache: &mut ExpertCache,
) -> Result<Vec<u8>, String> {
    let key = format!("{}#{}", tensor.name, expert_idx);
    if let Some(hit) = cache.get(&key) {
        emit_inference_stage(
            state,
            "expert_fetch",
            StageStatus::Completed,
            None,
            None,
            Some(format!("{} cache=hit", key)),
        );
        return Ok(hit);
    }
    let start_ms = now_ms();
    emit_inference_stage(
        state,
        "expert_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!("{} bytes=~{}", key, format_bytes(tensor.nbytes))),
    );
    let data = fetch_mxfp4_expert_raw(gguf_url, tensor, expert_idx).await?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_tensor_resident(state, key.clone(), data.len(), "mxfp4");
    cache.insert(key.clone(), data.clone());
    emit_inference_stage(
        state,
        "expert_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} cache=miss ms={elapsed_ms}", key)),
    );
    Ok(data)
}

async fn matmul_q8_0_with_bias(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    weight: &GgufTensor,
    bias: &GgufTensor,
    input: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    cache: &mut TensorCache,
    quant_cache: &mut QuantCache,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    if weight.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            weight.name, weight.ggml_type_name
        ));
    }
    let dims = &weight.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if input.len() != k || n == 0 {
        return Err(format!(
            "matmul shape mismatch for {} (k={}, n={}, input={})",
            weight.name,
            k,
            n,
            input.len()
        ));
    }
    let max_storage = gpu.device.limits().max_storage_buffer_binding_size as usize;
    let weight_bytes = usize::try_from(weight.nbytes).unwrap_or(usize::MAX);
    let chunked = weight_bytes > max_storage;
    let start_ms = now_ms();
    let mut cached_quant = None;
    let cache_note = if chunked {
        "cache=skip"
    } else {
        cached_quant = quant_cache.get(&weight.name);
        if cached_quant.is_some() {
            "cache=hit"
        } else {
            "cache=miss"
        }
    };
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Started,
        None,
        None,
        Some(if chunked {
            format!(
                "{} bytes={} (chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else {
            format!(
                "{} bytes={} {cache_note}",
                weight.name,
                format_bytes(weight.nbytes)
            )
        }),
    );
    let mut retry_chunked = false;
    let mut out = if chunked {
        gpu_matmul_q8_0_chunked(gguf_url, weight, input, gpu, gpu_tracker).await?
    } else {
        let quant = if let Some(hit) = cached_quant {
            hit
        } else {
            let mut quant =
                fetch_range_source(gguf_url, weight.absolute_offset, weight.nbytes).await?;
            if quant.len() % 4 != 0 {
                let padded = (quant.len() + 3) / 4 * 4;
                quant.resize(padded, 0);
            }
            emit_tensor_resident(state, weight.name.clone(), quant.len(), "q8_0");
            quant_cache.insert(weight.name.clone(), quant.clone());
            quant
        };
        match gpu_matmul_q8_0(&quant, input, k, n, gpu, gpu_tracker).await {
            Ok(out) => out,
            Err(err) => {
                retry_chunked = true;
                emit_inference_stage(
                    state,
                    "weights_retry",
                    StageStatus::Progress,
                    None,
                    None,
                    Some(format!("{} retry=chunked err={err}", weight.name)),
                );
                gpu_matmul_q8_0_chunked(gguf_url, weight, input, gpu, gpu_tracker).await?
            }
        }
    };
    let fetch_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(if chunked {
            format!(
                "{} bytes={} ms={fetch_ms} (chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else if retry_chunked {
            format!(
                "{} bytes={} ms={fetch_ms} (retry chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else {
            format!(
                "{} bytes={} ms={fetch_ms} {cache_note}",
                weight.name,
                format_bytes(weight.nbytes)
            )
        }),
    );
    let bias_vals = fetch_f32_tensor_cached(state, gguf_url, bias, cache).await?;
    if bias_vals.len() == out.len() {
        if let Err(err) =
            apply_bias_gpu(&mut out, &bias_vals, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            return Err(err);
        }
    }
    Ok(out)
}

async fn matmul_mxfp4_expert(
    quant: &[u8],
    input: &[f32],
    tensor: &GgufTensor,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let n = dims.get(1).copied().unwrap_or(0) as usize;
    let k = dims.get(2).copied().unwrap_or(0) as usize;
    if input.len() != k || n == 0 {
        return Err(format!(
            "matmul shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            input.len()
        ));
    }
    gpu_matmul_mxfp4(quant, input, k, n, gpu, gpu_tracker).await
}

fn rms_norm(input: &[f32], weight: &[f32], eps: f32) -> Result<Vec<f32>, String> {
    if input.len() != weight.len() {
        return Err("rms_norm shape mismatch".to_string());
    }
    let mut sum_sq = 0.0f32;
    for v in input {
        sum_sq += v * v;
    }
    let mean = sum_sq / input.len().max(1) as f32;
    let inv = (mean + eps).sqrt().recip();
    let mut out = Vec::with_capacity(input.len());
    for (v, w) in input.iter().zip(weight.iter()) {
        out.push(v * inv * w);
    }
    Ok(out)
}

fn l2_norm(values: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for v in values {
        sum += v * v;
    }
    sum.sqrt()
}

fn apply_bias(values: &mut [f32], bias: &[f32]) {
    if bias.len() != values.len() {
        return;
    }
    for (v, b) in values.iter_mut().zip(bias.iter()) {
        *v += *b;
    }
}

async fn apply_bias_gpu(
    values: &mut Vec<f32>,
    bias: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    allow_cpu_fallback: bool,
) -> Result<(), String> {
    if bias.len() != values.len() {
        return Ok(());
    }
    match vector_add_gpu(values, bias, gpu, gpu_tracker).await {
        Ok(out) => {
            *values = out;
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("bias gpu failed: {err}"));
            }
            apply_bias(values, bias);
        }
    }
    Ok(())
}

fn swiglu(gate: &[f32], up: &[f32]) -> Result<Vec<f32>, String> {
    if gate.len() != up.len() {
        return Err("swiglu shape mismatch".to_string());
    }
    let mut out = Vec::with_capacity(gate.len());
    for (&g, &u) in gate.iter().zip(up.iter()) {
        let g_clamped = g.min(SWIGLU_LIMIT);
        let u_clamped = u.max(-SWIGLU_LIMIT).min(SWIGLU_LIMIT);
        let sigmoid = 1.0 / (1.0 + (-SWIGLU_ALPHA * g_clamped).exp());
        let glu = g_clamped * sigmoid;
        out.push(glu * (u_clamped + 1.0));
    }
    Ok(out)
}

fn attention_single_token(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }
    if k.len() != kv_heads * head_dim || v.len() != kv_heads * head_dim {
        return Err("attention kv shape mismatch".to_string());
    }

    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    if heads % kv_heads != 0 {
        return Err(format!(
            "GQA: heads {} not divisible by kv_heads {}",
            heads, kv_heads
        ));
    }
    let group_size = heads / kv_heads;
    let mut out = vec![0.0f32; heads * head_dim];
    for h in 0..heads {
        let q_base = h * head_dim;
        let kv = h / group_size;
        let k_base = kv * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * k[k_base + i];
        }
        let score = dot * sm_scale;
        let sink = sinks.get(h).copied().unwrap_or(0.0);
        let w = score.exp();
        let s = sink.exp();
        let weight = w / (w + s);
        let v_base = k_base;
        for i in 0..head_dim {
            out[q_base + i] = v[v_base + i] * weight;
        }
    }
    Ok(out)
}

fn attention_with_cache(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }

    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    if heads % kv_heads != 0 {
        return Err(format!(
            "GQA: heads {} not divisible by kv_heads {}",
            heads, kv_heads
        ));
    }
    let group_size = heads / kv_heads;
    let mut out = vec![0.0f32; heads * head_dim];

    for h in 0..heads {
        let q_base = h * head_dim;
        let kv = h / group_size;
        let sink = sinks.get(h).copied().unwrap_or(0.0);
        let mut max_score = sink;

        for t in 0..window {
            let token = (start + t) % cache.capacity;
            let k_base = (token * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            if score > max_score {
                max_score = score;
            }
        }

        let mut weights = Vec::with_capacity(window);
        let mut denom = (sink - max_score).exp();
        for t in 0..window {
            let token = (start + t) % cache.capacity;
            let k_base = (token * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            let w = (score - max_score).exp();
            weights.push(w);
            denom += w;
        }

        if denom <= 0.0 {
            return Err("attention softmax denom is zero".to_string());
        }

        for (idx, w) in weights.iter().enumerate() {
            let weight = *w / denom;
            let token = (start + idx) % cache.capacity;
            let v_base = (token * kv_heads + kv) * head_dim;
            for i in 0..head_dim {
                out[q_base + i] += cache.v[v_base + i] * weight;
            }
        }
    }

    Ok(out)
}

fn attention_head_weights(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    head_index: usize,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    if head_index >= heads {
        return Err("attention head index out of range".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }

    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    if heads % kv_heads != 0 {
        return Err(format!(
            "GQA: heads {} not divisible by kv_heads {}",
            heads, kv_heads
        ));
    }
    let group_size = heads / kv_heads;

    let q_base = head_index * head_dim;
    let kv = head_index / group_size;
    let sink = sinks.get(head_index).copied().unwrap_or(0.0);
    let mut max_score = sink;
    for t in 0..window {
        let token = (start + t) % cache.capacity;
        let k_base = (token * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        if score > max_score {
            max_score = score;
        }
    }

    let mut weights = Vec::with_capacity(window);
    let mut denom = (sink - max_score).exp();
    for t in 0..window {
        let token = (start + t) % cache.capacity;
        let k_base = (token * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        let w = (score - max_score).exp();
        weights.push(w);
        denom += w;
    }

    if denom <= 0.0 {
        return Err("attention softmax denom is zero".to_string());
    }
    for weight in &mut weights {
        *weight /= denom;
    }
    Ok(weights)
}

struct RopeScaling {
    theta: f32,
    scaling_factor: f32,
    concentration: f32,
    low: f32,
    high: f32,
    use_yarn: bool,
}

fn compute_rope_scaling(
    theta: f32,
    rope_dim: usize,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
) -> RopeScaling {
    let theta = if theta <= 0.0 { 10000.0 } else { theta };
    let scaling_factor = rope_scaling_factor.max(1.0);
    let original_context = rope_scaling_original_context as f32;
    let use_yarn = scaling_factor > 1.0 && original_context > 0.0;
    let concentration = if use_yarn {
        0.1 * scaling_factor.ln() + 1.0
    } else {
        1.0
    };
    let theta_log = theta.ln();
    let d_half = rope_dim as f32 / 2.0;
    let mut low = 0.0f32;
    let mut high = 0.0f32;
    if use_yarn {
        let denom = theta_log.max(1e-6);
        low = d_half
            * (original_context / (ROPE_NTK_BETA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        high = d_half
            * (original_context / (ROPE_NTK_ALPHA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        if !(low > 0.0 && high > low && high < d_half - 1.0) {
            low = 0.0;
            high = 0.0;
        }
    }
    RopeScaling {
        theta,
        scaling_factor,
        concentration,
        low,
        high,
        use_yarn,
    }
}

