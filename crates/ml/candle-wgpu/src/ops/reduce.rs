use crate::device::WgpuDevice;
use crate::ops::finish_and_read_f32;
use bytemuck::{Pod, Zeroable};
use candle_core::{Error, Result};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/reduce.wgsl"));

#[derive(Debug, Clone, Copy)]
pub enum ReduceOp {
    Sum,
    Max,
    Min,
}

impl ReduceOp {
    fn entry_point(&self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Max => "max",
            Self::Min => "min",
        }
    }

    fn cache_key(&self) -> String {
        format!("reduce:{}", self.entry_point())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    len: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

pub fn reduce(device: &WgpuDevice, op: ReduceOp, input: &[f32]) -> Result<f32> {
    if input.is_empty() {
        return Err(Error::Msg("reduce requires non-empty input".to_string()));
    }

    if !device.has_wgpu() || input.len() > 256 {
        return Ok(reduce_cpu(op, input));
    }

    let wgpu_device = device
        .wgpu_device()
        .ok_or_else(|| Error::Msg("wgpu device not available".to_string()))?;
    let queue = device
        .wgpu_queue()
        .ok_or_else(|| Error::Msg("wgpu queue not available".to_string()))?;

    let params = Params {
        len: input.len() as u32,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };

    let input_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("reduce-input"),
        contents: bytemuck::cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_buf = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("reduce-out"),
        size: std::mem::size_of::<f32>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("reduce-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let pipeline = device
        .pipeline(&op.cache_key(), |device| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("reduce-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("reduce-pipeline"),
                layout: None,
                module: &module,
                entry_point: Some(op.entry_point()),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        })
        .ok_or_else(|| Error::Msg("wgpu pipeline cache unavailable".to_string()))?;

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("reduce-bind-group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: out_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("reduce-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("reduce-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    let out = finish_and_read_f32(wgpu_device, queue, encoder, &out_buf, 1)?;
    Ok(out.get(0).copied().unwrap_or(0.0))
}

fn reduce_cpu(op: ReduceOp, input: &[f32]) -> f32 {
    match op {
        ReduceOp::Sum => input.iter().copied().sum(),
        ReduceOp::Max => input
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max),
        ReduceOp::Min => input
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min),
    }
}
