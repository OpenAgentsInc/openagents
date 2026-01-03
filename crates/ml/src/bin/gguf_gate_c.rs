#[cfg(all(feature = "candle", feature = "wgpu"))]
mod gate {
    use std::borrow::Cow;
    use std::env;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::PathBuf;

    use bytemuck::cast_slice;
    use ml::{load_gguf_index, MlError, Result};
    use wgpu::util::DeviceExt;

    const Q8_0_BLOCK_BYTES: usize = 34;
    const Q8_0_BLOCK_VALUES: usize = 32;

    const DEFAULT_TENSOR: &str = "output.weight";
    const DEFAULT_K: usize = 128;
    const DEFAULT_N: usize = 64;
    const DEFAULT_TOLERANCE: f32 = 0.01;

    pub fn run() -> Result<()> {
        let args: Vec<String> = env::args().skip(1).collect();
        if args.is_empty() {
            return Err(MlError::InvalidConfig(
                "usage: gguf_gate_c <path> [--tensor NAME | --tensor-index N] [--k N] [--n N] [--tolerance F] [--dump N]".to_string(),
            ));
        }

        let mut path: Option<PathBuf> = None;
        let mut tensor_name: Option<String> = Some(DEFAULT_TENSOR.to_string());
        let mut tensor_index: Option<usize> = None;
        let mut k: usize = DEFAULT_K;
        let mut n: usize = DEFAULT_N;
        let mut tolerance: f32 = DEFAULT_TOLERANCE;
        let mut dump: usize = 0;

        let mut idx = 0;
        while idx < args.len() {
            match args[idx].as_str() {
                "--tensor" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tensor".to_string())
                    })?;
                    tensor_name = Some(value.to_string());
                    tensor_index = None;
                }
                "--tensor-index" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tensor-index".to_string())
                    })?;
                    tensor_index = Some(parse_usize(value, "--tensor-index")?);
                    tensor_name = None;
                }
                "--k" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --k".to_string())
                    })?;
                    k = parse_usize(value, "--k")?;
                }
                "--n" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --n".to_string())
                    })?;
                    n = parse_usize(value, "--n")?;
                }
                "--tolerance" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tolerance".to_string())
                    })?;
                    tolerance = parse_f32(value, "--tolerance")?;
                }
                "--dump" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --dump".to_string())
                    })?;
                    dump = parse_usize(value, "--dump")?;
                }
                arg => {
                    if path.is_some() {
                        return Err(MlError::InvalidConfig(format!(
                            "unexpected argument: {arg}"
                        )));
                    }
                    path = Some(PathBuf::from(arg));
                }
            }
            idx += 1;
        }

        let path = path.ok_or_else(|| MlError::InvalidConfig("missing gguf path".to_string()))?;

        if tensor_name.is_some() && tensor_index.is_some() {
            return Err(MlError::InvalidConfig(
                "use either --tensor or --tensor-index, not both".to_string(),
            ));
        }

        let index = load_gguf_index(&path)?;
        let tensor = if let Some(name) = tensor_name {
            index
                .tensors
                .iter()
                .find(|tensor| tensor.name == name)
                .ok_or_else(|| MlError::InvalidConfig(format!("tensor not found: {name}")))?
        } else if let Some(tidx) = tensor_index {
            index.tensors.get(tidx).ok_or_else(|| {
                MlError::InvalidConfig(format!("tensor index out of range: {tidx}"))
            })?
        } else {
            return Err(MlError::InvalidConfig(
                "missing tensor selection".to_string(),
            ));
        };

        if tensor.ggml_type != 8 {
            return Err(MlError::InvalidConfig(format!(
                "tensor {} is {}, expected Q8_0",
                tensor.name, tensor.ggml_type_name
            )));
        }

        if k == 0 || n == 0 {
            return Err(MlError::InvalidConfig(
                "k and n must be non-zero".to_string(),
            ));
        }

        let values = k
            .checked_mul(n)
            .ok_or_else(|| MlError::InvalidConfig("k*n overflow".to_string()))?;
        if values % Q8_0_BLOCK_VALUES != 0 {
            return Err(MlError::InvalidConfig(format!(
                "k*n must be divisible by {} (got {values})",
                Q8_0_BLOCK_VALUES
            )));
        }

        let blocks = values / Q8_0_BLOCK_VALUES;
        let bytes_needed = blocks * Q8_0_BLOCK_BYTES;
        if bytes_needed as u64 > tensor.nbytes {
            return Err(MlError::InvalidConfig(format!(
                "tensor slice too small: need {bytes_needed} bytes, have {}",
                tensor.nbytes
            )));
        }

        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(tensor.absolute_offset))?;
        let mut quant = vec![0u8; bytes_needed];
        file.read_exact(&mut quant)?;

        let mut x = Vec::with_capacity(k);
        for i in 0..k {
            let step = (i % 13) as f32 - 6.0;
            x.push(step * 0.01);
        }

        let weights = dequant_q8_0(&quant, values)?;
        let y_cpu = matmul_cpu(&weights, &x, k, n);

        let mut quant_padded = quant;
        if quant_padded.len() % 4 != 0 {
            let padded = (quant_padded.len() + 3) / 4 * 4;
            quant_padded.resize(padded, 0);
        }

        let y_gpu = gpu_matmul_q8_0(&quant_padded, &x, k, n)?;

        let mut max_abs = 0.0f32;
        let mut mean_abs = 0.0f32;
        for (cpu, gpu) in y_cpu.iter().zip(y_gpu.iter()) {
            let diff = (cpu - gpu).abs();
            max_abs = max_abs.max(diff);
            mean_abs += diff;
        }
        mean_abs /= n as f32;

        println!("gguf: {}", path.display());
        println!("tensor: {} ({})", tensor.name, tensor.ggml_type_name);
        println!("k: {k} n: {n}");
        println!("bytes_read: {bytes_needed}");
        println!("max_abs_diff: {max_abs}");
        println!("mean_abs_diff: {mean_abs}");
        println!("tolerance: {tolerance}");

        if dump > 0 {
            let count = dump.min(n);
            println!("sample:");
            for i in 0..count {
                let cpu = y_cpu[i];
                let gpu = y_gpu[i];
                let diff = (cpu - gpu).abs();
                println!("{i:4} cpu={cpu:.6} gpu={gpu:.6} diff={diff:.6}");
            }
        }

        if max_abs > tolerance {
            return Err(MlError::Model(format!(
                "max_abs_diff {max_abs} exceeds tolerance {tolerance}"
            )));
        }

        Ok(())
    }

    fn dequant_q8_0(data: &[u8], values: usize) -> Result<Vec<f32>> {
        if values % Q8_0_BLOCK_VALUES != 0 {
            return Err(MlError::InvalidConfig(
                "value count not divisible by Q8_0 block size".to_string(),
            ));
        }
        let blocks = values / Q8_0_BLOCK_VALUES;
        let needed = blocks * Q8_0_BLOCK_BYTES;
        if data.len() < needed {
            return Err(MlError::InvalidConfig(format!(
                "insufficient Q8_0 data: need {needed}, have {}",
                data.len()
            )));
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

    fn gpu_matmul_q8_0(quant: &[u8], x: &[f32], k: usize, n: usize) -> Result<Vec<f32>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| MlError::Device("no WebGPU adapter available".to_string()))?;

        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("gguf_gate_c"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|e| MlError::Device(format!("failed to create WebGPU device: {e:?}")))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("q8_0_gate_c"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(Q8_0_SHADER)),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("q8_0_gate_c_bindings"),
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
            label: Some("q8_0_gate_c_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("q8_0_gate_c_pipeline"),
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
            label: Some("q8_0_gate_c_bind_group"),
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
            label: Some("q8_0_gate_c_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("q8_0_gate_c_pass"),
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
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        match rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                return Err(MlError::Device(format!("map_async failed: {err:?}")));
            }
            Err(err) => {
                return Err(MlError::Device(format!(
                    "map_async channel failed: {err}"
                )));
            }
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

    fn parse_usize(value: &str, flag: &str) -> Result<usize> {
        value.parse().map_err(|_| {
            MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
        })
    }

    fn parse_f32(value: &str, flag: &str) -> Result<f32> {
        value.parse().map_err(|_| {
            MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
        })
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
}

#[cfg(all(feature = "candle", feature = "wgpu"))]
fn main() -> ml::Result<()> {
    gate::run()
}

#[cfg(not(all(feature = "candle", feature = "wgpu")))]
fn main() {
    eprintln!("gguf_gate_c requires the candle and wgpu features");
    std::process::exit(1);
}
