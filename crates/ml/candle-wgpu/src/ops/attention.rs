use crate::device::WgpuDevice;
use crate::ops::finish_and_read_f32;
use bytemuck::{Pod, Zeroable};
use candle_core::{Error, Result};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/attention.wgsl"));

#[derive(Debug, Clone, Copy)]
pub struct AttentionParams {
    pub batch: usize,
    pub heads: usize,
    pub seq_len: usize,
    pub head_dim: usize,
    pub causal: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParams {
    batch_size: u32,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
    scale: f32,
    causal: u32,
    _pad0: u32,
    _pad1: u32,
}

pub fn attention(
    device: &WgpuDevice,
    q: &[f32],
    k: &[f32],
    v: &[f32],
    params: AttentionParams,
) -> Result<Vec<f32>> {
    let total = params
        .batch
        .saturating_mul(params.heads)
        .saturating_mul(params.seq_len)
        .saturating_mul(params.head_dim);
    if total == 0 {
        return Ok(Vec::new());
    }
    if q.len() != total || k.len() != total || v.len() != total {
        return Err(Error::Msg("attention shape mismatch".to_string()));
    }

    if device.has_wgpu() && params.seq_len <= 256 {
        if let Ok(out) = attention_gpu(device, q, k, v, params) {
            return Ok(out);
        }
    }

    Ok(attention_cpu(q, k, v, params))
}

fn attention_gpu(
    device: &WgpuDevice,
    q: &[f32],
    k: &[f32],
    v: &[f32],
    params: AttentionParams,
) -> Result<Vec<f32>> {
    let wgpu_device = device
        .wgpu_device()
        .ok_or_else(|| Error::Msg("wgpu device not available".to_string()))?;
    let queue = device
        .wgpu_queue()
        .ok_or_else(|| Error::Msg("wgpu queue not available".to_string()))?;

    let scale = 1.0f32 / (params.head_dim as f32).sqrt();
    let gpu_params = GpuParams {
        batch_size: params.batch as u32,
        num_heads: params.heads as u32,
        seq_len: params.seq_len as u32,
        head_dim: params.head_dim as u32,
        scale,
        causal: if params.causal { 1 } else { 0 },
        _pad0: 0,
        _pad1: 0,
    };

    let q_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attention-q"),
        contents: bytemuck::cast_slice(q),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let k_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attention-k"),
        contents: bytemuck::cast_slice(k),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let v_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attention-v"),
        contents: bytemuck::cast_slice(v),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_buf = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attention-out"),
        size: (q.len() * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attention-params"),
        contents: bytemuck::bytes_of(&gpu_params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let pipeline = device
        .pipeline("attention", |device| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("attention-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("attention-pipeline"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        })
        .ok_or_else(|| Error::Msg("wgpu pipeline cache unavailable".to_string()))?;

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("attention-bind-group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: q_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: k_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: v_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: out_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("attention-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attention-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            params.seq_len as u32,
            params.heads as u32,
            params.batch as u32,
        );
    }

    finish_and_read_f32(wgpu_device, queue, encoder, &out_buf, q.len())
}

fn attention_cpu(q: &[f32], k: &[f32], v: &[f32], params: AttentionParams) -> Vec<f32> {
    let total = params
        .batch
        .saturating_mul(params.heads)
        .saturating_mul(params.seq_len)
        .saturating_mul(params.head_dim);
    let mut out = vec![0.0f32; total];
    if total == 0 {
        return out;
    }

    let scale = 1.0f32 / (params.head_dim as f32).sqrt();
    let mut scores = vec![0.0f32; params.seq_len];

    for b in 0..params.batch {
        for h in 0..params.heads {
            for q_pos in 0..params.seq_len {
                let q_base = (((b * params.heads + h) * params.seq_len) + q_pos) * params.head_dim;

                let mut max = f32::NEG_INFINITY;
                for j in 0..params.seq_len {
                    if params.causal && j > q_pos {
                        scores[j] = f32::NEG_INFINITY;
                        continue;
                    }
                    let k_base =
                        (((b * params.heads + h) * params.seq_len) + j) * params.head_dim;
                    let mut score = 0.0f32;
                    for d in 0..params.head_dim {
                        score += q[q_base + d] * k[k_base + d];
                    }
                    score *= scale;
                    scores[j] = score;
                    if score > max {
                        max = score;
                    }
                }

                let mut sum = 0.0f32;
                for j in 0..params.seq_len {
                    let value = (scores[j] - max).exp();
                    scores[j] = value;
                    sum += value;
                }
                if sum <= 0.0 {
                    continue;
                }
                let inv_sum = 1.0 / sum;

                for d in 0..params.head_dim {
                    let mut acc = 0.0f32;
                    for j in 0..params.seq_len {
                        let v_base =
                            (((b * params.heads + h) * params.seq_len) + j) * params.head_dim;
                        acc += scores[j] * inv_sum * v[v_base + d];
                    }
                    out[q_base + d] = acc;
                }
            }
        }
    }

    out
}
