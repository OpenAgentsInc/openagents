use std::cell::RefCell;
use std::io::{Cursor, Read};
use std::rc::Rc;

use bytemuck::cast_slice;
use futures::channel::oneshot;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wgpu::util::DeviceExt;

use crate::state::{AppState, GateStatus};

const DEFAULT_TENSOR: &str = "output.weight";
const DEFAULT_K: usize = 128;
const DEFAULT_N: usize = 64;
const DEFAULT_TOLERANCE: f32 = 0.01;
const METADATA_FETCH_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_ATTEMPTS: usize = 3;

const Q8_0_BLOCK_BYTES: usize = 34;
const Q8_0_BLOCK_VALUES: usize = 32;

pub(crate) fn init_ml_gate_runtime(state: Rc<RefCell<AppState>>) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };
    let search = match window.location().search() {
        Ok(search) => search,
        Err(_) => return,
    };
    let params = match web_sys::UrlSearchParams::new_with_str(&search) {
        Ok(params) => params,
        Err(_) => return,
    };

    let gguf_url = params.get("gguf").unwrap_or_default();
    if gguf_url.is_empty() {
        update_gate_state(&state, |gate| {
            gate.gate_status = GateStatus::Idle;
            gate.gate_message = Some("No GGUF URL provided (?gguf=...)".to_string());
            gate.gate_error = None;
            gate.gate_source = None;
        });
        return;
    }

    if state.borrow().ml_viz.gate_started {
        return;
    }

    let tensor = params.get("tensor").unwrap_or_else(|| DEFAULT_TENSOR.to_string());
    let k = parse_usize(params.get("k"), DEFAULT_K);
    let n = parse_usize(params.get("n"), DEFAULT_N);
    let tolerance = parse_f32(params.get("tolerance"), DEFAULT_TOLERANCE);

    update_gate_state(&state, |gate| {
        gate.gate_started = true;
        gate.gate_status = GateStatus::Running;
        gate.gate_message = Some("starting gate C/D".to_string());
        gate.gate_error = None;
        gate.gate_source = Some(gguf_url.clone());
        gate.gate_tensor = Some(tensor.clone());
        gate.gate_k = Some(k);
        gate.gate_n = Some(n);
        gate.gate_max_abs = None;
        gate.gate_mean_abs = None;
        gate.gate_bytes = None;
    });

    let config = GateConfig {
        gguf_url,
        tensor,
        k,
        n,
        tolerance,
    };

    let state_clone = state.clone();
    spawn_local(async move {
        if let Err(err) = run_gate(state_clone.clone(), &config).await {
            update_gate_state(&state_clone, |gate| {
                gate.gate_status = GateStatus::Failed;
                gate.gate_error = Some(err);
                gate.gate_message = Some("gate failed".to_string());
            });
        }
    });
}

struct GateConfig {
    gguf_url: String,
    tensor: String,
    k: usize,
    n: usize,
    tolerance: f32,
}

async fn run_gate(state: Rc<RefCell<AppState>>, config: &GateConfig) -> Result<(), String> {
    update_gate_state(&state, |gate| {
        gate.gate_message = Some("fetching GGUF metadata".to_string());
    });

    let index = fetch_and_parse_index(&config.gguf_url).await?;
    let tensor = index
        .tensors
        .iter()
        .find(|tensor| tensor.name == config.tensor)
        .ok_or_else(|| format!("tensor not found: {}", config.tensor))?;

    if config.k == 0 || config.n == 0 {
        return Err("k and n must be non-zero".to_string());
    }

    if tensor.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        ));
    }

    let values = config
        .k
        .checked_mul(config.n)
        .ok_or_else(|| "k*n overflow".to_string())?;
    if values % Q8_0_BLOCK_VALUES != 0 {
        return Err(format!(
            "k*n must be divisible by {} (got {values})",
            Q8_0_BLOCK_VALUES
        ));
    }

    let blocks = values / Q8_0_BLOCK_VALUES;
    let bytes_needed = blocks * Q8_0_BLOCK_BYTES;

    update_gate_state(&state, |gate| {
        gate.gate_message = Some("fetching Q8_0 slice".to_string());
        gate.gate_bytes = Some(bytes_needed);
    });

    let mut quant = fetch_range(
        &config.gguf_url,
        tensor.absolute_offset,
        bytes_needed as u64,
    )
    .await?;

    let x = build_input(config.k);
    let weights = dequant_q8_0(&quant, values)?;

    update_gate_state(&state, |gate| {
        gate.gate_message = Some("running CPU reference".to_string());
    });

    let y_cpu = matmul_cpu(&weights, &x, config.k, config.n);

    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    update_gate_state(&state, |gate| {
        gate.gate_message = Some("running WebGPU kernel".to_string());
    });

    let y_gpu = gpu_matmul_q8_0(&quant, &x, config.k, config.n).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    update_gate_state(&state, |gate| {
        gate.gate_status = if max_abs <= config.tolerance {
            GateStatus::Passed
        } else {
            GateStatus::Failed
        };
        gate.gate_message = Some("gate complete".to_string());
        gate.gate_max_abs = Some(max_abs);
        gate.gate_mean_abs = Some(mean_abs);
        gate.gate_error = if max_abs <= config.tolerance {
            None
        } else {
            Some(format!(
                "max_abs_diff {max_abs} exceeds tolerance {}",
                config.tolerance
            ))
        };
    });

    if max_abs > config.tolerance {
        return Err(format!(
            "max_abs_diff {max_abs} exceeds tolerance {}",
            config.tolerance
        ));
    }

    Ok(())
}

async fn fetch_and_parse_index(url: &str) -> Result<GgufIndex, String> {
    let mut fetch_len = METADATA_FETCH_BYTES;
    for _ in 0..MAX_METADATA_ATTEMPTS {
        let bytes = fetch_range(url, 0, fetch_len).await?;
        match parse_gguf_index(&bytes) {
            Ok(index) => return Ok(index),
            Err(ParseError::Incomplete) => {
                fetch_len = fetch_len.saturating_mul(2);
            }
            Err(ParseError::Invalid(msg)) => return Err(msg),
        }
    }
    Err("GGUF metadata parse incomplete after retries".to_string())
}

async fn fetch_range(url: &str, offset: u64, len: u64) -> Result<Vec<u8>, String> {
    if len == 0 {
        return Err("range length is zero".to_string());
    }

    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let end = offset.saturating_add(len.saturating_sub(1));
    let range_header = format!("bytes={offset}-{end}");

    let init = web_sys::RequestInit::new();
    init.set_method("GET");

    let headers = web_sys::Headers::new().map_err(|err| js_err(err))?;
    headers
        .set("Range", &range_header)
        .map_err(|err| js_err(err))?;
    init.set_headers(&headers);

    let request =
        web_sys::Request::new_with_str_and_init(url, &init).map_err(|err| js_err(err))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|err| js_err(err))?;
    let resp: web_sys::Response = resp_value.dyn_into().map_err(|err| js_err(err))?;

    if !resp.ok() {
        return Err(format!("fetch failed: {}", resp.status()));
    }

    let buffer = JsFuture::from(resp.array_buffer().map_err(|err| js_err(err))?)
        .await
        .map_err(|err| js_err(err))?;
    let array = js_sys::Uint8Array::new(&buffer);
    let mut data = vec![0u8; array.length() as usize];
    array.copy_to(&mut data);

    if data.len() > len as usize
        && resp
            .headers()
            .get("Content-Range")
            .map_err(|err| js_err(err))?
            .is_none()
    {
        return Err("range request not honored by server".to_string());
    }

    Ok(data)
}

fn build_input(k: usize) -> Vec<f32> {
    let mut x = Vec::with_capacity(k);
    for i in 0..k {
        let step = (i % 13) as f32 - 6.0;
        x.push(step * 0.01);
    }
    x
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

async fn gpu_matmul_q8_0(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
) -> Result<Vec<f32>, String> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "no WebGPU adapter available".to_string())?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("ml-gate"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await
        .map_err(|e| format!("failed to create WebGPU device: {e:?}"))?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_gate_c"),
        source: wgpu::ShaderSource::Wgsl(Q8_0_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_gate_bindings"),
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
        label: Some("q8_0_gate_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_gate_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_quant"),
        contents: quant,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let y_bytes = (n * std::mem::size_of::<f32>()) as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let params = [k as u32, n as u32, n as u32, 0u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_params"),
        contents: cast_slice(&params),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("q8_0_gate_bind_group"),
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
        label: Some("q8_0_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_gate_encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("q8_0_gate_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroup_size = 64u32;
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

fn update_gate_state<F>(state: &Rc<RefCell<AppState>>, updater: F)
where
    F: FnOnce(&mut crate::state::MlVizState),
{
    if let Ok(mut guard) = state.try_borrow_mut() {
        updater(&mut guard.ml_viz);
    }
}

fn parse_usize(value: Option<String>, default: usize) -> usize {
    value
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn parse_f32(value: Option<String>, default: f32) -> f32 {
    value
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default)
}

fn js_err(err: impl std::fmt::Debug) -> String {
    format!("{err:?}")
}

#[derive(Debug)]
enum ParseError {
    Incomplete,
    Invalid(String),
}

#[allow(dead_code)]
struct GgufTensor {
    name: String,
    ggml_type: u32,
    ggml_type_name: String,
    dims: Vec<u64>,
    offset: u64,
    absolute_offset: u64,
}

#[allow(dead_code)]
struct GgufIndex {
    version: u32,
    tensor_data_offset: u64,
    tensors: Vec<GgufTensor>,
}

fn parse_gguf_index(bytes: &[u8]) -> Result<GgufIndex, ParseError> {
    let mut cursor = Cursor::new(bytes);

    let magic = read_u32(&mut cursor)?;
    if magic != 0x4655_4747 && magic != 0x4747_5546 {
        return Err(ParseError::Invalid(format!(
            "invalid gguf magic: 0x{magic:08x}"
        )));
    }

    let version_raw = read_u32(&mut cursor)?;
    let version = match version_raw {
        1 | 2 | 3 => version_raw,
        _ => {
            return Err(ParseError::Invalid(format!(
                "unsupported gguf version: {version_raw}"
            )))
        }
    };

    let tensor_count = if version == 1 {
        read_u32(&mut cursor)? as u64
    } else {
        read_u64(&mut cursor)?
    };
    let kv_count = if version == 1 {
        read_u32(&mut cursor)? as u64
    } else {
        read_u64(&mut cursor)?
    };

    skip_kv_entries(&mut cursor, kv_count, version)?;

    let tensor_count_usize = usize::try_from(tensor_count)
        .map_err(|_| ParseError::Invalid("tensor count too large".to_string()))?;
    let mut tensors = Vec::with_capacity(tensor_count_usize);
    for _ in 0..tensor_count_usize {
        let name = read_string(&mut cursor, version)?;
        let n_dims = read_u32(&mut cursor)?;
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            let dim = if version == 1 {
                read_u32(&mut cursor)? as u64
            } else {
                read_u64(&mut cursor)?
            };
            dims.push(dim);
        }
        dims.reverse();
        let ggml_type = read_u32(&mut cursor)?;
        let offset = read_u64(&mut cursor)?;
        tensors.push(GgufTensor {
            name,
            ggml_type,
            ggml_type_name: ggml_type_name(ggml_type).to_string(),
            dims,
            offset,
            absolute_offset: 0,
        });
    }

    let tensor_data_offset = align_offset(cursor.position(), 32);
    for tensor in &mut tensors {
        tensor.absolute_offset = tensor_data_offset + tensor.offset;
    }

    Ok(GgufIndex {
        version,
        tensor_data_offset,
        tensors,
    })
}

fn skip_kv_entries<R: Read>(
    reader: &mut R,
    kv_count: u64,
    version: u32,
) -> Result<(), ParseError> {
    for _ in 0..kv_count {
        let _key = read_string(reader, version)?;
        let value_type = read_u32(reader)?;
        skip_value(reader, value_type, version)?;
    }
    Ok(())
}

fn skip_value<R: Read>(reader: &mut R, value_type: u32, version: u32) -> Result<(), ParseError> {
    match value_type {
        0 | 1 | 7 => skip_bytes(reader, 1),
        2 | 3 => skip_bytes(reader, 2),
        4 | 5 | 6 => skip_bytes(reader, 4),
        10 | 11 | 12 => skip_bytes(reader, 8),
        8 => {
            let len = read_string_len(reader, version)?;
            skip_bytes(reader, len)?;
            Ok(())
        }
        9 => {
            let elem_type = read_u32(reader)?;
            let len = read_u64(reader)?;
            skip_array(reader, elem_type, len, version)
        }
        _ => Err(ParseError::Invalid(format!(
            "unsupported gguf value type: {value_type}"
        ))),
    }
}

fn skip_array<R: Read>(
    reader: &mut R,
    elem_type: u32,
    len: u64,
    version: u32,
) -> Result<(), ParseError> {
    match elem_type {
        0 | 1 | 7 => skip_bytes(reader, len),
        2 | 3 => skip_bytes(reader, len * 2),
        4 | 5 | 6 => skip_bytes(reader, len * 4),
        10 | 11 | 12 => skip_bytes(reader, len * 8),
        8 => {
            for _ in 0..len {
                let slen = read_string_len(reader, version)?;
                skip_bytes(reader, slen)?;
            }
            Ok(())
        }
        _ => Err(ParseError::Invalid(format!(
            "unsupported gguf array type: {elem_type}"
        ))),
    }
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, ParseError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, ParseError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_string_len<R: Read>(reader: &mut R, version: u32) -> Result<u64, ParseError> {
    if version == 1 {
        Ok(read_u32(reader)? as u64)
    } else {
        read_u64(reader)
    }
}

fn read_string<R: Read>(reader: &mut R, version: u32) -> Result<String, ParseError> {
    let len = read_string_len(reader, version)?;
    if len > (usize::MAX as u64) {
        return Err(ParseError::Invalid("gguf string too long".to_string()));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    let value = String::from_utf8(buf)
        .map_err(|_| ParseError::Invalid("gguf string invalid utf8".to_string()))?;
    Ok(value)
}

fn skip_bytes<R: Read>(reader: &mut R, len: u64) -> Result<(), ParseError> {
    let mut remaining = len;
    let mut buf = [0u8; 1024];
    while remaining > 0 {
        let chunk = buf.len().min(remaining as usize);
        reader
            .read_exact(&mut buf[..chunk])
            .map_err(map_incomplete)?;
        remaining -= chunk as u64;
    }
    Ok(())
}

fn align_offset(offset: u64, align: u64) -> u64 {
    if align == 0 {
        return offset;
    }
    let rem = offset % align;
    if rem == 0 {
        offset
    } else {
        offset + (align - rem)
    }
}

fn ggml_type_name(type_id: u32) -> &'static str {
    match type_id {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        4 => "Q4_2",
        5 => "Q4_3",
        6 => "Q5_0",
        7 => "Q5_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "IQ2_XXS",
        17 => "IQ2_XS",
        18 => "IQ3_XXS",
        19 => "IQ1_S",
        20 => "IQ4_NL",
        21 => "IQ3_S",
        22 => "IQ2_S",
        23 => "IQ4_XS",
        24 => "I8",
        25 => "I16",
        26 => "I32",
        27 => "I64",
        28 => "F64",
        29 => "IQ1_M",
        30 => "BF16",
        _ => "unknown",
    }
}

fn map_incomplete(err: std::io::Error) -> ParseError {
    if err.kind() == std::io::ErrorKind::UnexpectedEof {
        ParseError::Incomplete
    } else {
        ParseError::Invalid(err.to_string())
    }
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

const Q8_0_SHADER: &str = r#"
struct Params {
    k: u32,
    n: u32,
    stride: u32,
    _pad: u32,
};

struct QuantBuffer {
    data: array<u32>,
};

@group(0) @binding(0) var<storage, read> quant: QuantBuffer;
@group(0) @binding(1) var<storage, read> x: array<f32>;
@group(0) @binding(2) var<storage, read_write> y: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

fn load_u8(offset: u32) -> u32 {
    let word = quant.data[offset >> 2u];
    let shift = (offset & 3u) * 8u;
    return (word >> shift) & 0xFFu;
}

fn load_i8(offset: u32) -> i32 {
    let b = load_u8(offset);
    return i32(b << 24u) >> 24u;
}

fn load_u16(offset: u32) -> u32 {
    let lo = load_u8(offset);
    let hi = load_u8(offset + 1u);
    return lo | (hi << 8u);
}

fn f16_to_f32(bits: u32) -> f32 {
    let sign = (bits >> 15u) & 1u;
    let exp = (bits >> 10u) & 0x1Fu;
    let frac = bits & 0x3FFu;
    var val: f32;
    if (exp == 0u) {
        if (frac == 0u) {
            val = 0.0;
        } else {
            val = f32(frac) * exp2(-24.0);
        }
    } else if (exp == 31u) {
        val = 0.0;
    } else {
        val = (1.0 + f32(frac) * 0.0009765625) * exp2(f32(exp) - 15.0);
    }
    if (sign == 1u) {
        val = -val;
    }
    return val;
}

fn q8_0_load(idx: u32) -> f32 {
    let block = idx / 32u;
    let lane = idx & 31u;
    let base = block * 34u;
    let d_bits = load_u16(base);
    let d = f16_to_f32(d_bits);
    let q = load_i8(base + 2u + lane);
    return d * f32(q);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    var acc: f32 = 0.0;
    for (var k: u32 = 0u; k < params.k; k = k + 1u) {
        let idx = k * params.stride + col;
        acc = acc + x[k] * q8_0_load(idx);
    }
    y[col] = acc;
}
"#;
