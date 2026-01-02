mod attention;
mod binary;
mod matmul;
mod reduce;
mod unary;

pub use attention::{attention, AttentionParams};
pub use binary::{binary, BinaryOp};
pub use matmul::matmul;
pub use reduce::{reduce, ReduceOp};
pub use unary::{unary, UnaryOp};

use candle_core::{Error, Result};
use futures::channel::oneshot;

pub(crate) fn byte_len(len: usize) -> wgpu::BufferAddress {
    (len * std::mem::size_of::<f32>()) as wgpu::BufferAddress
}

pub(crate) fn finish_and_read_f32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    output: &wgpu::Buffer,
    len: usize,
) -> Result<Vec<f32>> {
    if len == 0 {
        return Ok(Vec::new());
    }

    let size = byte_len(len);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("candle-wgpu-readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_buffer_to_buffer(output, 0, &readback, 0, size);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });

    device.poll(wgpu::Maintain::Wait);
    let map_result = pollster::block_on(rx)
        .map_err(|_| Error::Msg("readback canceled".to_string()))?;
    if let Err(err) = map_result {
        return Err(Error::Msg(format!("readback failed: {err:?}")));
    }

    let data = slice.get_mapped_range();
    let values = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();

    Ok(values)
}
