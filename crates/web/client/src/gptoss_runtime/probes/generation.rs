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

