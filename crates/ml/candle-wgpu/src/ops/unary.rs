use crate::device::WgpuDevice;
use crate::ops::finish_and_read_f32;
use bytemuck::{Pod, Zeroable};
use candle_core::{Error, Result};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/unary.wgsl"));

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Exp,
    Log,
    Relu,
    Silu,
    Gelu,
    Tanh,
}

impl UnaryOp {
    fn entry_point(&self) -> &'static str {
        match self {
            Self::Exp => "exp_kernel",
            Self::Log => "log_kernel",
            Self::Relu => "relu_kernel",
            Self::Silu => "silu_kernel",
            Self::Gelu => "gelu_kernel",
            Self::Tanh => "tanh_kernel",
        }
    }

    fn cache_key(&self) -> String {
        format!("unary:{}", self.entry_point())
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

pub fn unary(device: &WgpuDevice, op: UnaryOp, input: &[f32]) -> Result<Vec<f32>> {
    if input.is_empty() {
        return Ok(Vec::new());
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
        label: Some("unary-input"),
        contents: bytemuck::cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_buf = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("unary-out"),
        size: (input.len() * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("unary-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let pipeline = device
        .pipeline(&op.cache_key(), |device| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("unary-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("unary-pipeline"),
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
        label: Some("unary-bind-group"),
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
        label: Some("unary-encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("unary-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups = (input.len() as u32 + 255) / 256;
        pass.dispatch_workgroups(workgroups, 1, 1);
    }

    finish_and_read_f32(wgpu_device, queue, encoder, &out_buf, input.len())
}
