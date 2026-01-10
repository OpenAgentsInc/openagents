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

