use crate::device::WgpuDevice;
use crate::ops::finish_and_read_f32;
use bytemuck::{Pod, Zeroable};
use candle_core::{Error, Result};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/binary.wgsl"));

#[derive(Debug, Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Minimum,
    Maximum,
}

impl BinaryOp {
    fn entry_point(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
        }
    }

    fn cache_key(&self) -> String {
        format!("binary:{}", self.entry_point())
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

pub fn binary(device: &WgpuDevice, op: BinaryOp, lhs: &[f32], rhs: &[f32]) -> Result<Vec<f32>> {
    if lhs.len() != rhs.len() {
        return Err(Error::Msg("binary op requires equal lengths".to_string()));
    }
    if lhs.is_empty() {
        return Ok(Vec::new());
    }

    let wgpu_device = device
        .wgpu_device()
        .ok_or_else(|| Error::Msg("wgpu device not available".to_string()))?;
    let queue = device
        .wgpu_queue()
        .ok_or_else(|| Error::Msg("wgpu queue not available".to_string()))?;

    let params = Params {
        len: lhs.len() as u32,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };

    let a_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("binary-a"),
        contents: bytemuck::cast_slice(lhs),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let b_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("binary-b"),
        contents: bytemuck::cast_slice(rhs),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_buf = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("binary-out"),
        size: (lhs.len() * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("binary-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let pipeline = device
        .pipeline(&op.cache_key(), |device| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("binary-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("binary-pipeline"),
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
        label: Some("binary-bind-group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: a_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: b_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("binary-encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("binary-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups = (lhs.len() as u32 + 255) / 256;
        pass.dispatch_workgroups(workgroups, 1, 1);
    }

    finish_and_read_f32(wgpu_device, queue, encoder, &out_buf, lhs.len())
}
