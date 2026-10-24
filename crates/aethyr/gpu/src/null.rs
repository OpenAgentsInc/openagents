//! Placeholder backend for host builds without `native`, so CPU-side code and
//! tests compile. Every GPU call panics.

use crate::*;

pub type Handle = usize;

pub struct Device;

const NO_BACKEND: &str = "af-gpu was built without a backend; enable `native` or build for wasm32";

pub fn release(_: Kind, _: Handle) {}
pub fn create_buffer(_: &Device, _: &BufferDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_texture(_: &Device, _: &TextureDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_view(_: Handle, _: &TextureViewDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_sampler(_: &Device, _: &SamplerDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_shader_module(_: &Device, _: &str, _: &str) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_compute_pipeline(_: &Device, _: &str, _: Handle, _: &str) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_render_pipeline(_: &Device, _: &RenderPipelineDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn compute_bind_group_layout(_: Handle, _: u32) -> Handle { panic!("{NO_BACKEND}") }
pub fn render_bind_group_layout(_: Handle, _: u32) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_bind_group(_: &Device, _: &str, _: Handle, _: &[BindGroupEntry]) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_timestamp_queries(_: &Device, _: u32) -> Handle { panic!("{NO_BACKEND}") }
pub fn create_command_encoder(_: &Device) -> Handle { panic!("{NO_BACKEND}") }
pub fn write_buffer(_: &Device, _: Handle, _: u64, _: &[u8]) { panic!("{NO_BACKEND}") }
pub fn write_texture(_: &Device, _: Handle, _: u32, _: u32, _: &[u8], _: u32, _: Extent) { panic!("{NO_BACKEND}") }
pub fn submit(_: &Device, _: &[Handle]) { panic!("{NO_BACKEND}") }
pub fn begin_compute_pass(_: Handle, _: &str, _: Option<PassTimestamps>) -> Handle { panic!("{NO_BACKEND}") }
pub fn begin_render_pass(_: Handle, _: &RenderPassDescriptor) -> Handle { panic!("{NO_BACKEND}") }
pub fn copy_buffer_to_buffer(_: Handle, _: Handle, _: u64, _: Handle, _: u64, _: u64) { panic!("{NO_BACKEND}") }
pub fn clear_buffer(_: Handle, _: Handle, _: u64, _: u64) { panic!("{NO_BACKEND}") }
pub fn resolve_queries(_: Handle, _: Handle, _: u32, _: u32, _: Handle, _: u64) { panic!("{NO_BACKEND}") }
pub fn copy_texture_to_buffer(_: Handle, _: Handle, _: Handle, _: u32, _: Extent) { panic!("{NO_BACKEND}") }
pub fn finish(_: Handle) -> Handle { panic!("{NO_BACKEND}") }
pub fn compute_set_pipeline(_: Handle, _: Handle) { panic!("{NO_BACKEND}") }
pub fn render_set_pipeline(_: Handle, _: Handle) { panic!("{NO_BACKEND}") }
pub fn compute_set_bind_group(_: Handle, _: u32, _: Handle) { panic!("{NO_BACKEND}") }
pub fn render_set_bind_group(_: Handle, _: u32, _: Handle) { panic!("{NO_BACKEND}") }
pub fn dispatch(_: Handle, _: u32, _: u32, _: u32) { panic!("{NO_BACKEND}") }
pub fn dispatch_indirect(_: Handle, _: Handle, _: u64) { panic!("{NO_BACKEND}") }
pub fn end_compute_pass(_: Handle) {}
pub fn end_render_pass(_: Handle) {}
pub fn set_vertex_buffer(_: Handle, _: u32, _: Handle, _: u64, _: u64) { panic!("{NO_BACKEND}") }
pub fn set_index_buffer(_: Handle, _: Handle, _: IndexFormat, _: u64, _: u64) { panic!("{NO_BACKEND}") }
pub fn draw(_: Handle, _: u32, _: u32, _: u32, _: u32) { panic!("{NO_BACKEND}") }
pub fn draw_indexed(_: Handle, _: u32, _: u32, _: u32, _: i32, _: u32) { panic!("{NO_BACKEND}") }
pub fn draw_indirect(_: Handle, _: Handle, _: u64) { panic!("{NO_BACKEND}") }
pub fn draw_indexed_indirect(_: Handle, _: Handle, _: u64) { panic!("{NO_BACKEND}") }
pub fn read_buffer(_: &Device, _: Handle, _: u64, _: u64) -> ReadTicket { panic!("{NO_BACKEND}") }
pub fn completed_reads(_: &Device) -> Vec<(ReadTicket, Vec<u8>)> { panic!("{NO_BACKEND}") }
pub fn surface_view(_: &Device) -> Handle { panic!("{NO_BACKEND}") }
pub fn surface_format(_: &Device) -> TextureFormat { panic!("{NO_BACKEND}") }
pub fn present(_: &Device) {}
pub fn resize_surface(_: &Device, _: u32, _: u32) {}
pub fn wait_idle(_: &Device) {}
pub fn surface_texture(_: &Device) -> Option<Handle> { None }
pub fn take_errors(_: &Device) -> Vec<String> { Vec::new() }

pub fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

pub fn log(message: &str) {
    eprintln!("{message}");
}
