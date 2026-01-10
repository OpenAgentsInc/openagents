fn apply_rope(
    values: &mut [f32],
    heads: usize,
    head_dim: usize,
    position: usize,
    theta: f32,
    rope_dim: u32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
) -> Result<(), String> {
    if head_dim == 0 || heads == 0 {
        return Err("rope invalid head dims".to_string());
    }
    let expected = heads
        .checked_mul(head_dim)
        .ok_or_else(|| "rope shape overflow".to_string())?;
    if values.len() != expected {
        return Err(format!(
            "rope shape mismatch values={} heads={} head_dim={}",
            values.len(),
            heads,
            head_dim
        ));
    }
    let rope_dim = rope_dim.min(head_dim as u32) as usize;
    if rope_dim == 0 {
        return Ok(());
    }
    if rope_dim % 2 != 0 {
        return Err("rope_dim must be even".to_string());
    }

    let scaling = compute_rope_scaling(
        theta,
        rope_dim,
        rope_scaling_factor,
        rope_scaling_original_context,
    );
    let theta = scaling.theta;
    let scaling_factor = scaling.scaling_factor;
    let use_yarn = scaling.use_yarn;
    let concentration = scaling.concentration;
    let low = scaling.low;
    let high = scaling.high;
    for h in 0..heads {
        let base = h * head_dim;
        for i in (0..rope_dim).step_by(2) {
            let freq = theta.powf(i as f32 / rope_dim as f32);
            let mut inv_freq = 1.0 / freq;
            if use_yarn && high > low {
                let t = (i / 2) as f32;
                let ramp = (t - low) / (high - low);
                let mask = 1.0 - ramp.clamp(0.0, 1.0);
                let interp = 1.0 / (scaling_factor * freq);
                let extrap = 1.0 / freq;
                inv_freq = interp * (1.0 - mask) + extrap * mask;
            }
            let angle = position as f32 * inv_freq;
            let (sin, cos) = angle.sin_cos();
            let sin = sin * concentration;
            let cos = cos * concentration;
            let a = values[base + i];
            let b = values[base + i + 1];
            values[base + i] = a * cos - b * sin;
            values[base + i + 1] = a * sin + b * cos;
        }
    }
    Ok(())
}

fn dequant_q8_0(data: &[u8], values: usize) -> Result<Vec<f32>, String> {
    if values % Q8_0_BLOCK_VALUES != 0 {
        return Err("value count not divisible by Q8_0 block size".to_string());
    }
    let blocks = values / Q8_0_BLOCK_VALUES;
    let needed = blocks * Q8_0_BLOCK_BYTES;
    if data.len() < needed {
        return Err(format!(
            "insufficient Q8_0 data: need {needed}, have {}",
            data.len()
        ));
    }

    let mut out = vec![0.0f32; values];
    for block in 0..blocks {
        let base = block * Q8_0_BLOCK_BYTES;
        let scale_bits = u16::from_le_bytes([data[base], data[base + 1]]);
        let scale = f16_to_f32(scale_bits);
        for i in 0..Q8_0_BLOCK_VALUES {
            let q = data[base + 2 + i] as i8;
            out[block * Q8_0_BLOCK_VALUES + i] = scale * q as f32;
        }
    }
    Ok(out)
}

fn dequant_mxfp4(data: &[u8], values: usize) -> Result<Vec<f32>, String> {
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("value count not divisible by MXFP4 block size".to_string());
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let needed = blocks * MXFP4_BLOCK_BYTES;
    if data.len() < needed {
        return Err(format!(
            "insufficient MXFP4 data: need {needed}, have {}",
            data.len()
        ));
    }

    let mut out = vec![0.0f32; values];
    let half = MXFP4_BLOCK_VALUES / 2;
    for block in 0..blocks {
        let base = block * MXFP4_BLOCK_BYTES;
        let scale_byte = data[base];
        let scale = mxfp4_scale(scale_byte);
        for j in 0..half {
            let byte = data[base + 1 + j];
            let lo = (byte & 0x0F) as usize;
            let hi = (byte >> 4) as usize;
            out[block * MXFP4_BLOCK_VALUES + j] = MXFP4_VALUES[lo] * scale;
            out[block * MXFP4_BLOCK_VALUES + half + j] = MXFP4_VALUES[hi] * scale;
        }
    }
    Ok(out)
}

fn mxfp4_scale(scale_byte: u8) -> f32 {
    // Matches ggml_e8m0_to_fp32_half.
    let bits = if scale_byte < 2 {
        0x0020_0000u32 << scale_byte
    } else {
        (scale_byte as u32 - 1) << 23
    };
    f32::from_bits(bits)
}

fn matmul_cpu(weights: &[f32], x: &[f32], k: usize, n: usize) -> Vec<f32> {
    let mut y = vec![0.0f32; n];
    for col in 0..n {
        let mut acc = 0.0f32;
        for row in 0..k {
            acc += x[row] * weights[row * n + col];
        }
        y[col] = acc;
    }
    y
}

fn linear_f32_with_bias(
    weights: &[f32],
    bias: &[f32],
    x: &[f32],
    tensor: &GgufTensor,
) -> Result<Vec<f32>, String> {
    let dims = &tensor.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if x.len() != k || n == 0 {
        return Err(format!(
            "linear shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            x.len()
        ));
    }
    if weights.len() != k * n {
        return Err(format!(
            "linear weight size mismatch for {} (have={}, want={})",
            tensor.name,
            weights.len(),
            k * n
        ));
    }
    let mut y = matmul_cpu(weights, x, k, n);
    if bias.len() == n {
        for (out, b) in y.iter_mut().zip(bias.iter()) {
            *out += *b;
        }
    }
    Ok(y)
}

async fn linear_f32_with_bias_gpu(
    weights: &[f32],
    bias: &[f32],
    x: &[f32],
    tensor: &GgufTensor,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    let dims = &tensor.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if x.len() != k || n == 0 {
        return Err(format!(
            "linear shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            x.len()
        ));
    }
    if weights.len() != k * n {
        return Err(format!(
            "linear weight size mismatch for {} (have={}, want={})",
            tensor.name,
            weights.len(),
            k * n
        ));
    }
    let mut y = gpu_matmul_f32(weights, x, k, n, gpu, gpu_tracker).await?;
    if bias.len() == n {
        apply_bias_gpu(&mut y, bias, gpu, gpu_tracker, allow_cpu_fallback).await?;
    }
    Ok(y)
}

fn diff_stats(y_cpu: &[f32], y_gpu: &[f32]) -> (f32, f32) {
    let mut max_abs = 0.0f32;
    let mut mean_abs = 0.0f32;
    let len = y_cpu.len().min(y_gpu.len());
    if len == 0 {
        return (0.0, 0.0);
    }
    for (cpu, gpu) in y_cpu.iter().zip(y_gpu.iter()) {
        let diff = (cpu - gpu).abs();
        max_abs = max_abs.max(diff);
        mean_abs += diff;
    }
    mean_abs /= len as f32;
    (max_abs, mean_abs)
}

fn pick_workgroup_size(device: &wgpu::Device) -> u32 {
    let limits = device.limits();
    let max_x = limits.max_compute_workgroup_size_x;
    let max_invocations = limits.max_compute_invocations_per_workgroup;
    let max_size = max_x.min(max_invocations).max(1);
    for candidate in [256u32, 128, 64, 32, 16, 8, 4, 2, 1] {
        if candidate <= max_size {
            return candidate;
        }
    }
    1
}

fn kv_limit_for_gpu(
    config: &GptOssConfig,
    gpu: &GpuContext,
    budget_bytes: u64,
) -> Option<KvLimit> {
    let head_count = config.head_count.max(1) as usize;
    let kv_heads = config.head_count_kv.max(1) as usize;
    let embed = config.embedding_length as usize;
    if embed == 0 {
        return None;
    }
    let head_dim = (embed / head_count).max(1);
    let stride = kv_heads.checked_mul(head_dim)?;
    let bytes_per_token = u64::try_from(stride)
        .ok()?
        .checked_mul(std::mem::size_of::<f32>() as u64)?;
    if bytes_per_token == 0 {
        return None;
    }
    let limits = gpu.device.limits();
    let max_storage = limits.max_storage_buffer_binding_size as usize;
    let max_buffer = limits.max_buffer_size as usize;
    let max_bytes = match (max_storage, max_buffer) {
        (0, 0) => return None,
        (0, other) => other,
        (other, 0) => other,
        (storage, buffer) => storage.min(buffer),
    };
    if max_bytes == 0 {
        return None;
    }
    let per_layer_max_u64 = (max_bytes as u64) / bytes_per_token;
    let per_layer_max = usize::try_from(per_layer_max_u64).unwrap_or(usize::MAX);
    let layer_count = config.block_count.max(1) as usize;
    let budget_max_u64 = if layer_count == 0 || budget_bytes == 0 {
        None
    } else {
        let per_token_all_layers = bytes_per_token
            .checked_mul(layer_count as u64)
            .filter(|value| *value > 0)?;
        Some(budget_bytes / per_token_all_layers)
    };
    let budget_max = budget_max_u64
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0);
    let mut max_tokens = per_layer_max.max(1);
    if let Some(budget_tokens) = budget_max {
        max_tokens = max_tokens.min(budget_tokens.max(1));
    }
    Some(KvLimit {
        max_tokens,
        per_layer_max: per_layer_max.max(1),
        budget_max,
        budget_bytes,
    })
}

fn shader_with_workgroup(source: &str, workgroup_size: u32) -> String {
    source.replace("{{WORKGROUP_SIZE}}", &workgroup_size.to_string())
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RmsNormParams {
    n: u32,
    _pad0: u32,
    eps: f32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RopeParams {
    heads: u32,
    head_dim: u32,
    rope_dim: u32,
    position: u32,
    theta: f32,
    scaling_factor: f32,
    low: f32,
    high: f32,
    concentration: f32,
    use_yarn: u32,
    _pad0: u32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AttentionParams {
    heads: u32,
    kv_heads: u32,
    head_dim: u32,
    seq_len: u32,
    window_start: u32,
    capacity: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DequantParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VecAddParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SwigluParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct F32MatmulParams {
    k: u32,
    n: u32,
    _pad0: u32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AttentionWeightsParams {
    head_index: u32,
    heads: u32,
    kv_heads: u32,
    head_dim: u32,
    seq_len: u32,
    window_start: u32,
    capacity: u32,
    _pad0: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScaleAddParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    weight: f32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
}

async fn rms_norm_gpu(
    input: &[f32],
    weight: &[f32],
    eps: f32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if input.len() != weight.len() {
        return Err("rms_norm shape mismatch".to_string());
    }
    let n = input.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let bytes = n * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("rms_norm input", bytes, max_storage)?;
    ensure_storage_limit("rms_norm weight", bytes, max_storage)?;
    ensure_storage_limit("rms_norm output", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(RMSNORM_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rmsnorm"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rmsnorm_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rmsnorm_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rmsnorm_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_input"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let weight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_weight"),
        contents: cast_slice(weight),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rmsnorm_output"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = RmsNormParams {
        n: u32::try_from(n).map_err(|_| "rms_norm length overflow".to_string())?,
        _pad0: 0,
        eps,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<RmsNormParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rmsnorm_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: weight_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rmsnorm_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 4 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rmsnorm_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rmsnorm_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("rmsnorm map_async failed: {err:?}")),
        Err(_) => return Err("rmsnorm map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn apply_rope_gpu(
    values: &[f32],
    heads: usize,
    head_dim: usize,
    position: usize,
    theta: f32,
    rope_dim: u32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if head_dim == 0 || heads == 0 {
        return Err("rope invalid head dims".to_string());
    }
    let expected = heads
        .checked_mul(head_dim)
        .ok_or_else(|| "rope shape overflow".to_string())?;
    if values.len() != expected {
        return Err(format!(
            "rope shape mismatch values={} heads={} head_dim={}",
            values.len(),
            heads,
            head_dim
        ));
    }
    let rope_dim = rope_dim.min(head_dim as u32) as usize;
    if rope_dim == 0 {
        return Ok(values.to_vec());
    }
    if rope_dim % 2 != 0 {
        return Err("rope_dim must be even".to_string());
    }
    let pairs = heads
        .checked_mul(rope_dim / 2)
        .ok_or_else(|| "rope pair overflow".to_string())?;
    if pairs == 0 {
        return Ok(values.to_vec());
    }

    let scaling = compute_rope_scaling(theta, rope_dim, rope_scaling_factor, rope_scaling_original_context);
    let device = &gpu.device;
    let queue = &gpu.queue;
    let bytes = values.len() * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("rope values", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(ROPE_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rope"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rope_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rope_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rope_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let values_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rope_values"),
        contents: cast_slice(values),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });
    let params = RopeParams {
        heads: u32::try_from(heads).map_err(|_| "rope heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "rope head_dim overflow".to_string())?,
        rope_dim: u32::try_from(rope_dim).map_err(|_| "rope rope_dim overflow".to_string())?,
        position: u32::try_from(position).map_err(|_| "rope position overflow".to_string())?,
        theta: scaling.theta,
        scaling_factor: scaling.scaling_factor,
        low: scaling.low,
        high: scaling.high,
        concentration: scaling.concentration,
        use_yarn: if scaling.use_yarn { 1 } else { 0 },
        _pad0: 0,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<RopeParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rope_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rope_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: values_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rope_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 2 + params_bytes, 3);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rope_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rope_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let pair_count = u32::try_from(pairs).map_err(|_| "rope pair count overflow".to_string())?;
        let groups = (pair_count + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&values_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("rope map_async failed: {err:?}")),
        Err(_) => return Err("rope map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn dequant_q8_0_gpu(
    quant: &[u8],
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let out_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("q8_0 dequant input", quant.len(), max_storage)?;
    ensure_storage_limit("q8_0 dequant output", out_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_DEQUANT_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_dequant"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_dequant_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("q8_0_dequant_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_dequant_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_dequant_quant"),
        contents: quant,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_dequant_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = DequantParams {
        n: u32::try_from(n).map_err(|_| "q8_0 dequant n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<DequantParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_dequant_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("q8_0_dequant_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: quant_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_dequant_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(quant.len() + out_bytes * 2 + params_bytes, 4);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_dequant_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("q8_0_dequant_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("q8_0 dequant map_async failed: {err:?}")),
        Err(_) => return Err("q8_0 dequant map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn vector_add_gpu(
    a: &[f32],
    b: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if a.len() != b.len() {
        return Err("vec_add shape mismatch".to_string());
    }
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("vec_add a", bytes, max_storage)?;
    ensure_storage_limit("vec_add b", bytes, max_storage)?;
    ensure_storage_limit("vec_add out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(VEC_ADD_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vec_add"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vec_add_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("vec_add_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vec_add_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let a_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_a"),
        contents: cast_slice(a),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let b_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_b"),
        contents: cast_slice(b),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vec_add_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = VecAddParams {
        n: u32::try_from(n).map_err(|_| "vec_add n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<VecAddParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vec_add_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: a_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: b_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vec_add_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vec_add_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vec_add_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("vec_add map_async failed: {err:?}")),
        Err(_) => return Err("vec_add map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn scale_add_gpu(
    acc: &[f32],
    input: &[f32],
    weight: f32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if acc.len() != input.len() {
        return Err("scale_add shape mismatch".to_string());
    }
    let n = acc.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("scale_add acc", bytes, max_storage)?;
    ensure_storage_limit("scale_add input", bytes, max_storage)?;
    ensure_storage_limit("scale_add out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(SCALE_ADD_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scale_add"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scale_add_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("scale_add_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("scale_add_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let acc_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_acc"),
        contents: cast_slice(acc),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_input"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scale_add_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = ScaleAddParams {
        n: u32::try_from(n).map_err(|_| "scale_add n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
        weight,
        _pad3: 0,
        _pad4: 0,
        _pad5: 0,
    };
    let params_bytes = std::mem::size_of::<ScaleAddParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scale_add_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: acc_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scale_add_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("scale_add_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("scale_add_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("scale_add map_async failed: {err:?}")),
        Err(_) => return Err("scale_add map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn swiglu_gpu(
    gate: &[f32],
    up: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if gate.len() != up.len() {
        return Err("swiglu shape mismatch".to_string());
    }
    let n = gate.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("swiglu gate", bytes, max_storage)?;
    ensure_storage_limit("swiglu up", bytes, max_storage)?;
    ensure_storage_limit("swiglu out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(SWIGLU_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("swiglu"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("swiglu_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("swiglu_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("swiglu_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let gate_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_gate"),
        contents: cast_slice(gate),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let up_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_up"),
        contents: cast_slice(up),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swiglu_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = SwigluParams {
        n: u32::try_from(n).map_err(|_| "swiglu n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<SwigluParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("swiglu_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: gate_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: up_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swiglu_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("swiglu_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("swiglu_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("swiglu map_async failed: {err:?}")),
        Err(_) => return Err("swiglu map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_f32(
    weights: &[f32],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let weights_bytes = weights.len() * std::mem::size_of::<f32>();
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("f32 weights", weights_bytes, max_storage)?;
    ensure_storage_limit("f32 input", x_bytes, max_storage)?;
    ensure_storage_limit("f32 output", y_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(MATMUL_F32_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("f32_matmul"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("f32_matmul_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("f32_matmul_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("f32_matmul_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let weights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_weights"),
        contents: cast_slice(weights),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("f32_matmul_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = F32MatmulParams {
        k: u32::try_from(k).map_err(|_| "f32 matmul k overflow".to_string())?,
        n: u32::try_from(n).map_err(|_| "f32 matmul n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<F32MatmulParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("f32_matmul_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: weights_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: x_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: y_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("f32_matmul_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(weights_bytes + x_bytes + (y_bytes as usize) * 2 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("f32_matmul_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("f32_matmul_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&y_buffer, 0, &readback, 0, y_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("f32 matmul map_async failed: {err:?}")),
        Err(_) => return Err("f32 matmul map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn attention_with_cache_gpu(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
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
    let window_start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let k_buf = cache
        .gpu_k
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?;
    let v_buf = cache
        .gpu_v
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?;

    let sink_len = heads.max(1);
    let mut sink_values = vec![0.0f32; sink_len];
    for (idx, value) in sinks.iter().enumerate().take(sink_len) {
        sink_values[idx] = *value;
    }

    let device = &gpu.device;
    let queue = &gpu.queue;
    let q_bytes = q.len() * std::mem::size_of::<f32>();
    let k_bytes = cache
        .capacity
        .checked_mul(stride)
        .ok_or_else(|| "attention cache overflow".to_string())?
        * std::mem::size_of::<f32>();
    let v_bytes = k_bytes;
    let sink_bytes = sink_values.len() * std::mem::size_of::<f32>();
    let out_bytes = q_bytes;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("attn q", q_bytes, max_storage)?;
    ensure_storage_limit("attn k", k_bytes, max_storage)?;
    ensure_storage_limit("attn v", v_bytes, max_storage)?;
    ensure_storage_limit("attn sinks", sink_bytes, max_storage)?;
    ensure_storage_limit("attn out", out_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device)
        .min(u32::try_from(heads).unwrap_or(1))
        .max(1);
    let shader_source = shader_with_workgroup(ATTENTION_DECODE_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("attn_decode"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("attn_decode_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("attn_decode_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("attn_decode_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let q_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_q"),
        contents: cast_slice(q),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let k_buffer = k_buf.clone();
    let v_buffer = v_buf.clone();
    let sink_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_sinks"),
        contents: cast_slice(&sink_values),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = AttentionParams {
        heads: u32::try_from(heads).map_err(|_| "attn heads overflow".to_string())?,
        kv_heads: u32::try_from(kv_heads).map_err(|_| "attn kv_heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "attn head_dim overflow".to_string())?,
        seq_len: u32::try_from(window).map_err(|_| "attn seq_len overflow".to_string())?,
        window_start: u32::try_from(window_start)
            .map_err(|_| "attn window_start overflow".to_string())?,
        capacity: u32::try_from(cache.capacity)
            .map_err(|_| "attn capacity overflow".to_string())?,
    };
    let params_bytes = std::mem::size_of::<AttentionParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("attn_decode_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: q_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: k_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: v_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: sink_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(
        q_bytes + k_bytes + v_bytes + sink_bytes + out_bytes * 2 + params_bytes,
        7,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("attn_decode_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attn_decode_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let head_count = u32::try_from(heads).map_err(|_| "attn head count overflow".to_string())?;
        let groups = (head_count + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("attn map_async failed: {err:?}")),
        Err(_) => return Err("attn map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn attention_head_weights_gpu(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    head_index: usize,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let window_start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;

    let q_len = head_index
        .checked_add(1)
        .ok_or_else(|| "attention head index overflow".to_string())?
        .checked_mul(head_dim)
        .ok_or_else(|| "attention head dim overflow".to_string())?;
    if q.len() < q_len {
        return Err("attention q shape mismatch".to_string());
    }
    let sink_len = head_index + 1;
    if sinks.len() < sink_len {
        return Err("attention sinks shape mismatch".to_string());
    }

    let device = &gpu.device;
    let queue = &gpu.queue;
    let q_bytes = q.len() * std::mem::size_of::<f32>();
    let k_bytes = cache
        .capacity
        .checked_mul(stride)
        .ok_or_else(|| "attention cache overflow".to_string())?
        * std::mem::size_of::<f32>();
    let sink_bytes = sinks.len() * std::mem::size_of::<f32>();
    let out_bytes = window * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("attn weights q", q_bytes, max_storage)?;
    ensure_storage_limit("attn weights k", k_bytes, max_storage)?;
    ensure_storage_limit("attn weights sinks", sink_bytes, max_storage)?;
    ensure_storage_limit("attn weights out", out_bytes, max_storage)?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("attn_weights"),
        source: wgpu::ShaderSource::Wgsl(ATTENTION_WEIGHTS_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("attn_weights_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("attn_weights_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("attn_weights_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let q_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_q"),
        contents: cast_slice(q),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let k_buffer = cache
        .gpu_k
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?
        .clone();
    let sink_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_sinks"),
        contents: cast_slice(sinks),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_weights_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = AttentionWeightsParams {
        head_index: u32::try_from(head_index)
            .map_err(|_| "attention head index overflow".to_string())?,
        heads: u32::try_from(heads).map_err(|_| "attention heads overflow".to_string())?,
        kv_heads: u32::try_from(kv_heads).map_err(|_| "attention kv_heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "attention head_dim overflow".to_string())?,
        seq_len: u32::try_from(window).map_err(|_| "attention seq_len overflow".to_string())?,
        window_start: u32::try_from(window_start)
            .map_err(|_| "attention window_start overflow".to_string())?,
        capacity: u32::try_from(cache.capacity)
            .map_err(|_| "attention capacity overflow".to_string())?,
        _pad0: 0,
    };
    let params_bytes = std::mem::size_of::<AttentionWeightsParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("attn_weights_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: q_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: k_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: sink_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_weights_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(q_bytes + sink_bytes + out_bytes * 2 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("attn_weights_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attn_weights_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("attn weights map_async failed: {err:?}")),
        Err(_) => return Err("attn weights map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_q8_0(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("q8_0 weights", quant.len(), max_storage)?;
    ensure_storage_limit("q8_0 input", x_bytes, max_storage)?;
    ensure_storage_limit("q8_0 output", y_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_probe"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_probe_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("q8_0_probe_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_probe_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_quant"),
        contents: quant,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_probe_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = [k as u32, n as u32, 0u32, 0u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_params"),
        contents: cast_slice(&params),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("q8_0_probe_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: quant_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: x_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: y_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_probe_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        quant.len()
            + x.len() * std::mem::size_of::<f32>()
            + (y_bytes as usize) * 2
            + params_bytes,
        5,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_probe_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("q8_0_probe_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&y_buffer, 0, &readback, 0, y_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map_async failed: {err:?}")),
        Err(_) => return Err("map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_q8_0_chunked(
    gguf_url: &GgufSource,
    weight: &GgufTensor,
    input: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
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
    if n % Q8_0_BLOCK_VALUES != 0 {
        return Err("q8_0 n not divisible by block size".to_string());
    }

    let row_bytes = (n / Q8_0_BLOCK_VALUES) * Q8_0_BLOCK_BYTES;
    let max_bytes = gpu.device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = input.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    if row_bytes > max_bytes {
        return Err(format!(
            "q8_0 row bytes {} exceed max storage {}",
            format_bytes(row_bytes as u64),
            format_bytes(max_bytes as u64)
        ));
    }
    ensure_storage_limit("q8_0 input", x_bytes, max_bytes)?;
    ensure_storage_limit("q8_0 output", y_bytes, max_bytes)?;
    let max_rows = (max_bytes.saturating_sub(3) / row_bytes).max(1);
    let chunk_rows = max_rows;

    let device = &gpu.device;
    let queue = &gpu.queue;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_chunked"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_chunked_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("q8_0_chunked_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_chunked_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_chunked_x"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let y_bytes = y_bytes as u64;
    let mut zeroes = vec![0.0f32; n];
    let y_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_chunked_y"),
        contents: cast_slice(&zeroes),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });
    zeroes.clear();

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_chunked_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        input.len() * std::mem::size_of::<f32>()
            + y_bytes as usize
            + y_bytes as usize,
        3,
    );

    let mut row_offset = 0usize;
    while row_offset < k {
        let rows = (k - row_offset).min(chunk_rows);
        let offset = weight
            .absolute_offset
            .saturating_add((row_offset * row_bytes) as u64);
        let len = rows * row_bytes;
        let mut quant = fetch_range_source(gguf_url, offset, len as u64).await?;
        if quant.len() % 4 != 0 {
            let padded = (quant.len() + 3) / 4 * 4;
            quant.resize(padded, 0);
        }
        ensure_storage_limit("q8_0 weights", quant.len(), max_bytes)?;

        let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("q8_0_chunked_quant"),
            contents: &quant,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(quant.len(), 1);
        let params = [rows as u32, n as u32, row_offset as u32, if row_offset == 0 { 0 } else { 1 }];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("q8_0_chunked_params"),
            contents: cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(params_bytes, 1);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("q8_0_chunked_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quant_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: x_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: y_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("q8_0_chunked_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("q8_0_chunked_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));

        row_offset += rows;
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_chunked_readback_encoder"),
    });
    encoder.copy_buffer_to_buffer(&y_buffer, 0, &readback, 0, y_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map_async failed: {err:?}")),
        Err(_) => return Err("map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_mxfp4(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    if n % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 n not divisible by block size".to_string());
    }
    let row_bytes = (n / MXFP4_BLOCK_VALUES) * MXFP4_BLOCK_BYTES;
    let expected_bytes = row_bytes
        .checked_mul(k)
        .ok_or_else(|| "mxfp4 weight byte overflow".to_string())?;
    if quant.len() < expected_bytes {
        return Err("mxfp4 weights truncated".to_string());
    }
    ensure_storage_limit("mxfp4 input", x_bytes, max_storage)?;
    ensure_storage_limit("mxfp4 output", y_bytes, max_storage)?;
    let chunked = expected_bytes > max_storage;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(MXFP4_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mxfp4_probe"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mxfp4_probe_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mxfp4_probe_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("mxfp4_probe_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mxfp4_probe_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mxfp4_probe_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mxfp4_probe_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        x.len() * std::mem::size_of::<f32>() + (y_bytes as usize) * 2,
        3,
    );

    let mut row_offset = 0usize;
    while row_offset < k {
        let rows = if chunked {
            let max_rows = (max_storage / row_bytes).max(1);
            (k - row_offset).min(max_rows)
        } else {
            k
        };
        let start = row_offset * row_bytes;
        let end = start + rows * row_bytes;
        let slice = &quant[start..end];
        let mut quant_chunk = slice.to_vec();
        if quant_chunk.len() % 4 != 0 {
            let padded = (quant_chunk.len() + 3) / 4 * 4;
            quant_chunk.resize(padded, 0);
        }
        if chunked {
            ensure_storage_limit("mxfp4 weights", quant_chunk.len(), max_storage)?;
        } else {
            ensure_storage_limit("mxfp4 weights", quant_chunk.len(), max_storage)?;
        }

        let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mxfp4_probe_quant"),
            contents: &quant_chunk,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(quant_chunk.len(), 1);
        let params = [
            rows as u32,
            n as u32,
            row_offset as u32,
            if row_offset == 0 { 0 } else { 1 },
        ];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mxfp4_probe_params"),
            contents: cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(params_bytes, 1);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mxfp4_probe_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quant_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: x_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: y_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mxfp4_probe_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mxfp4_probe_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));

        if chunked {
            row_offset += rows;
        } else {
            break;
        }
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mxfp4_probe_readback_encoder"),
    });
    encoder.copy_buffer_to_buffer(&y_buffer, 0, &readback, 0, y_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map_async failed: {err:?}")),
        Err(_) => return Err("map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x03ff) as u32;
    let mut val = if exp == 0 {
        if frac == 0 {
            0.0
        } else {
            (frac as f32) * 2f32.powi(-24)
        }
    } else if exp == 31 {
        f32::INFINITY
    } else {
        (1.0 + (frac as f32) * 0.000_976_562_5) * 2f32.powi(exp - 15)
    };
    if sign == 1 {
        val = -val;
    }
    val
}

