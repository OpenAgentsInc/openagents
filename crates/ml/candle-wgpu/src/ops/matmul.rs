use crate::device::WgpuDevice;
use crate::ops::finish_and_read_f32;
use bytemuck::{Pod, Zeroable};
use candle_core::{Error, Result};
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/matmul.wgsl"));

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    m: u32,
    n: u32,
    k: u32,
    _pad: u32,
}

pub fn matmul(
    device: &WgpuDevice,
    lhs: &[f32],
    rhs: &[f32],
    m: usize,
    n: usize,
    k: usize,
) -> Result<Vec<f32>> {
    if m == 0 || n == 0 || k == 0 {
        return Ok(vec![0.0; m * n]);
    }
    if lhs.len() != m * k || rhs.len() != k * n {
        return Err(Error::Msg("matmul shape mismatch".to_string()));
    }

    let wgpu_device = device
        .wgpu_device()
        .ok_or_else(|| Error::Msg("wgpu device not available".to_string()))?;
    let queue = device
        .wgpu_queue()
        .ok_or_else(|| Error::Msg("wgpu queue not available".to_string()))?;

    let params = Params {
        m: m as u32,
        n: n as u32,
        k: k as u32,
        _pad: 0,
    };

    let a_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("matmul-a"),
        contents: bytemuck::cast_slice(lhs),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let b_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("matmul-b"),
        contents: bytemuck::cast_slice(rhs),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_buf = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("matmul-out"),
        size: (m * n * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buf = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("matmul-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let pipeline = device
        .pipeline("matmul", |device| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("matmul-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("matmul-pipeline"),
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
        label: Some("matmul-bind-group"),
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
        label: Some("matmul-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("matmul-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups_x = (n as u32 + 15) / 16;
        let workgroups_y = (m as u32 + 15) / 16;
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    finish_and_read_f32(wgpu_device, queue, encoder, &out_buf, m * n)
}
