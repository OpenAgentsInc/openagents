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

