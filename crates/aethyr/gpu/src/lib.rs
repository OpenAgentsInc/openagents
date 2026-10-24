//! A thin GPU interface shaped like `webgpu.h`.
//!
//! Each object and call mirrors one `webgpu.h` object or function, restricted to
//! the subset the engine uses. Two backends implement it:
//!
//! - `wasm32`: the browser's WebGPU, reached through a small JavaScript import
//!   table (`apps/render-lab/gpu-imports.ts`). The browser keeps full validation.
//!   Pass commands are batched by default: each pass's commands are written
//!   to Wasm memory ([`batch`]) and replayed by one call (ADR 0004);
//!   [`set_batched_calls`] switches back to one import call each, for A/B runs.
//! - `native` feature: a native `webgpu.h` implementation, Dawn or wgpu-native,
//!   linked through bindings generated from that implementation's header.
//!
//! Without either backend the crate still builds so CPU-side code can be tested;
//! any GPU call then panics.
//!
//! Decision record: `docs/decisions/0006-cross-platform-engine-core.md`.

use serde::Serialize;
use std::cell::RefCell;

pub mod batch;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web as backend;

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
use native as backend;
#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
pub use native::{NativeOptions, SurfaceSource};

#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
mod null;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
use null as backend;

pub use backend::Handle;

/// Buffer usage bits. Values match `GPUBufferUsage` and `WGPUBufferUsage`.
pub mod buffer_usage {
    pub const MAP_READ: u32 = 0x1;
    pub const MAP_WRITE: u32 = 0x2;
    pub const COPY_SRC: u32 = 0x4;
    pub const COPY_DST: u32 = 0x8;
    pub const INDEX: u32 = 0x10;
    pub const VERTEX: u32 = 0x20;
    pub const UNIFORM: u32 = 0x40;
    pub const STORAGE: u32 = 0x80;
    pub const INDIRECT: u32 = 0x100;
    pub const QUERY_RESOLVE: u32 = 0x200;
}

/// Texture usage bits. Values match `GPUTextureUsage` and `WGPUTextureUsage`.
pub mod texture_usage {
    pub const COPY_SRC: u32 = 0x1;
    pub const COPY_DST: u32 = 0x2;
    pub const TEXTURE_BINDING: u32 = 0x4;
    pub const STORAGE_BINDING: u32 = 0x8;
    pub const RENDER_ATTACHMENT: u32 = 0x10;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TextureFormat {
    #[serde(rename = "rgba8unorm")]
    Rgba8Unorm,
    #[serde(rename = "bgra8unorm")]
    Bgra8Unorm,
    #[serde(rename = "rgba16float")]
    Rgba16Float,
    #[serde(rename = "rgba32float")]
    Rgba32Float,
    #[serde(rename = "r32uint")]
    R32Uint,
    #[serde(rename = "rg32uint")]
    Rg32Uint,
    #[serde(rename = "r32float")]
    R32Float,
    #[serde(rename = "rg16float")]
    Rg16Float,
    #[serde(rename = "depth32float")]
    Depth32Float,
}

impl TextureFormat {
    pub fn bytes_per_texel(self) -> u32 {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm | Self::R32Uint | Self::R32Float => 4,
            Self::Rg16Float | Self::Depth32Float => 4,
            Self::Rgba16Float | Self::Rg32Uint => 8,
            Self::Rgba32Float => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TextureViewDimension {
    #[serde(rename = "2d")]
    D2,
    #[serde(rename = "2d-array")]
    D2Array,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum VertexFormat {
    #[serde(rename = "float32x2")]
    Float32x2,
    #[serde(rename = "float32x3")]
    Float32x3,
    #[serde(rename = "float32x4")]
    Float32x4,
    #[serde(rename = "uint32")]
    Uint32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum IndexFormat {
    #[serde(rename = "uint16")]
    Uint16,
    #[serde(rename = "uint32")]
    Uint32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CompareFunction {
    #[serde(rename = "less")]
    Less,
    #[serde(rename = "less-equal")]
    LessEqual,
    #[serde(rename = "greater")]
    Greater,
    #[serde(rename = "greater-equal")]
    GreaterEqual,
    #[serde(rename = "always")]
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CullMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "back")]
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FilterMode {
    #[serde(rename = "nearest")]
    Nearest,
    #[serde(rename = "linear")]
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AddressMode {
    #[serde(rename = "clamp-to-edge")]
    ClampToEdge,
    #[serde(rename = "repeat")]
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum VertexStepMode {
    #[serde(rename = "vertex")]
    Vertex,
    #[serde(rename = "instance")]
    Instance,
}

/// Optional device features the engine may use. Startup selects a profile from
/// what the adapter offers; every feature has a baseline fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Feature {
    #[serde(rename = "timestamp-query")]
    TimestampQuery,
    #[serde(rename = "subgroups")]
    Subgroups,
    #[serde(rename = "indirect-first-instance")]
    IndirectFirstInstance,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
    #[serde(rename = "depthOrArrayLayers")]
    pub depth_or_array_layers: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferDescriptor<'a> {
    pub label: &'a str,
    pub size: u64,
    pub usage: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureDescriptor<'a> {
    pub label: &'a str,
    pub size: Extent,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub format: TextureFormat,
    pub usage: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureViewDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<TextureViewDimension>,
    pub base_mip_level: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mip_level_count: Option<u32>,
    pub base_array_layer: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_layer_count: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplerDescriptor {
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: FilterMode,
    pub address_mode_u: AddressMode,
    pub address_mode_v: AddressMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_anisotropy: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexAttribute {
    pub format: VertexFormat,
    pub offset: u64,
    pub shader_location: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexBufferLayout {
    pub array_stride: u64,
    pub step_mode: VertexStepMode,
    pub attributes: Vec<VertexAttribute>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthState {
    pub format: TextureFormat,
    pub depth_write_enabled: bool,
    pub depth_compare: CompareFunction,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorTarget {
    pub format: TextureFormat,
    /// Premultiplied-alpha "over" blending when set.
    pub blend: bool,
}

pub struct RenderPipelineDescriptor<'a> {
    pub label: &'a str,
    pub module: &'a ShaderModule,
    pub vertex_entry: &'a str,
    pub fragment_entry: Option<&'a str>,
    pub buffers: Vec<VertexBufferLayout>,
    pub cull_mode: CullMode,
    pub depth: Option<DepthState>,
    pub sample_count: u32,
    pub targets: Vec<ColorTarget>,
}

pub enum BindingResource<'a> {
    Buffer { buffer: &'a Buffer, offset: u64, size: Option<u64> },
    Sampler(&'a Sampler),
    TextureView(&'a TextureView),
}

pub struct BindGroupEntry<'a> {
    pub binding: u32,
    pub resource: BindingResource<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum LoadOp {
    Clear([f64; 4]),
    Load,
}

pub struct ColorAttachment<'a> {
    pub view: &'a TextureView,
    pub resolve_target: Option<&'a TextureView>,
    pub load: LoadOp,
    pub store: bool,
}

pub struct DepthAttachment<'a> {
    pub view: &'a TextureView,
    /// `Some(value)` clears depth; `None` loads it.
    pub clear: Option<f32>,
    pub store: bool,
}

/// Records a pass's start and end time into a timestamp query set.
#[derive(Clone, Copy)]
pub struct PassTimestamps<'a> {
    pub query_set: &'a QuerySet,
    pub begin: u32,
    pub end: u32,
}

pub struct RenderPassDescriptor<'a> {
    pub label: &'a str,
    pub color: Vec<ColorAttachment<'a>>,
    pub depth: Option<DepthAttachment<'a>>,
    pub timestamps: Option<PassTimestamps<'a>>,
}

/// Counts of encoded commands, so CPU submission cost can be reported next to
/// GPU time.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct CommandCounts {
    pub draws: u32,
    pub dispatches: u32,
    pub pipeline_binds: u32,
    pub bind_group_binds: u32,
    pub buffer_binds: u32,
    pub passes: u32,
}

thread_local! {
    static COUNTS: RefCell<CommandCounts> = RefCell::new(CommandCounts::default());
}

/// Returns and resets the commands counted since the last call.
pub fn take_command_counts() -> CommandCounts {
    COUNTS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

fn count(f: impl FnOnce(&mut CommandCounts)) {
    COUNTS.with(|c| f(&mut c.borrow_mut()));
}

macro_rules! handle_object {
    ($name:ident, $kind:expr) => {
        pub struct $name {
            raw: Handle,
        }
        impl $name {
            pub fn raw(&self) -> Handle {
                self.raw
            }
        }
        impl Drop for $name {
            fn drop(&mut self) {
                backend::release($kind, self.raw);
            }
        }
    };
}

/// Object kinds, used to release handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Buffer,
    Texture,
    TextureView,
    Sampler,
    ShaderModule,
    BindGroupLayout,
    BindGroup,
    ComputePipeline,
    RenderPipeline,
    QuerySet,
    CommandEncoder,
    CommandBuffer,
    ComputePass,
    RenderPass,
}

handle_object!(Buffer, Kind::Buffer);
handle_object!(Texture, Kind::Texture);
handle_object!(TextureView, Kind::TextureView);
handle_object!(Sampler, Kind::Sampler);
handle_object!(ShaderModule, Kind::ShaderModule);
handle_object!(BindGroupLayout, Kind::BindGroupLayout);
handle_object!(BindGroup, Kind::BindGroup);
handle_object!(ComputePipeline, Kind::ComputePipeline);
handle_object!(RenderPipeline, Kind::RenderPipeline);
handle_object!(QuerySet, Kind::QuerySet);
handle_object!(CommandBuffer, Kind::CommandBuffer);

pub struct CommandEncoder {
    raw: Handle,
}

pub struct ComputePass<'e> {
    raw: Handle,
    _encoder: std::marker::PhantomData<&'e mut CommandEncoder>,
}

pub struct RenderPass<'e> {
    raw: Handle,
    _encoder: std::marker::PhantomData<&'e mut CommandEncoder>,
}

/// A ticket for an asynchronous buffer read; see [`Gpu::read_buffer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadTicket(pub u32);

/// Adapter details reported at startup.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AdapterInfo {
    pub backend: String,
    pub vendor: String,
    pub architecture: String,
    pub description: String,
    pub features: Vec<String>,
}

/// One device and queue.
pub struct Gpu {
    raw: backend::Device,
    features: Vec<Feature>,
    info: AdapterInfo,
}

impl Gpu {
    /// On the web, wraps the device the JavaScript host created before
    /// instantiating the module.
    #[cfg(target_arch = "wasm32")]
    pub fn from_host() -> Gpu {
        let (raw, features, info) = backend::from_host();
        Gpu { raw, features, info }
    }

    /// Natively, creates an instance, adapter, and device through `webgpu.h`.
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    pub fn new_native(options: &NativeOptions) -> Result<Gpu, String> {
        let (raw, features, info) = backend::new_device(options)?;
        Ok(Gpu { raw, features, info })
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }

    pub fn info(&self) -> &AdapterInfo {
        &self.info
    }

    pub fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer {
        Buffer { raw: backend::create_buffer(&self.raw, desc) }
    }

    /// Creates a buffer and fills it with `data`. Adds `COPY_DST` to the usage.
    pub fn create_buffer_init(&self, label: &str, usage: u32, data: &[u8]) -> Buffer {
        let size = (data.len().max(4) as u64).next_multiple_of(4);
        let buffer = self.create_buffer(&BufferDescriptor {
            label,
            size,
            usage: usage | buffer_usage::COPY_DST,
        });
        if !data.is_empty() {
            let mut padded;
            let bytes = if data.len().is_multiple_of(4) {
                data
            } else {
                padded = data.to_vec();
                padded.resize(size as usize, 0);
                &padded
            };
            self.write_buffer(&buffer, 0, bytes);
        }
        buffer
    }

    pub fn create_texture(&self, desc: &TextureDescriptor) -> Texture {
        Texture { raw: backend::create_texture(&self.raw, desc) }
    }

    pub fn create_view(&self, texture: &Texture, desc: &TextureViewDescriptor) -> TextureView {
        TextureView { raw: backend::create_view(texture.raw, desc) }
    }

    pub fn create_sampler(&self, desc: &SamplerDescriptor) -> Sampler {
        Sampler { raw: backend::create_sampler(&self.raw, desc) }
    }

    pub fn create_shader_module(&self, label: &str, wgsl: &str) -> ShaderModule {
        ShaderModule { raw: backend::create_shader_module(&self.raw, label, wgsl) }
    }

    pub fn create_compute_pipeline(&self, label: &str, module: &ShaderModule, entry: &str) -> ComputePipeline {
        ComputePipeline { raw: backend::create_compute_pipeline(&self.raw, label, module.raw, entry) }
    }

    pub fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> RenderPipeline {
        RenderPipeline { raw: backend::create_render_pipeline(&self.raw, desc) }
    }

    pub fn compute_bind_group_layout(&self, pipeline: &ComputePipeline, index: u32) -> BindGroupLayout {
        BindGroupLayout { raw: backend::compute_bind_group_layout(pipeline.raw, index) }
    }

    pub fn render_bind_group_layout(&self, pipeline: &RenderPipeline, index: u32) -> BindGroupLayout {
        BindGroupLayout { raw: backend::render_bind_group_layout(pipeline.raw, index) }
    }

    pub fn create_bind_group(&self, label: &str, layout: &BindGroupLayout, entries: &[BindGroupEntry]) -> BindGroup {
        BindGroup { raw: backend::create_bind_group(&self.raw, label, layout.raw, entries) }
    }

    /// Creates a timestamp query set, or `None` without `timestamp-query`.
    pub fn create_timestamp_queries(&self, count: u32) -> Option<QuerySet> {
        self.has(Feature::TimestampQuery)
            .then(|| QuerySet { raw: backend::create_timestamp_queries(&self.raw, count) })
    }

    pub fn create_command_encoder(&self) -> CommandEncoder {
        CommandEncoder { raw: backend::create_command_encoder(&self.raw) }
    }

    pub fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]) {
        debug_assert!(data.len().is_multiple_of(4), "write_buffer needs a multiple of 4 bytes");
        backend::write_buffer(&self.raw, buffer.raw, offset, data);
    }

    /// Writes `layer` of a 2D or array texture's first mip level.
    pub fn write_texture(&self, texture: &Texture, mip: u32, layer: u32, data: &[u8], bytes_per_row: u32, size: Extent) {
        backend::write_texture(&self.raw, texture.raw, mip, layer, data, bytes_per_row, size);
    }

    pub fn submit(&self, buffers: Vec<CommandBuffer>) {
        let raws: Vec<Handle> = buffers.iter().map(|b| b.raw).collect();
        backend::submit(&self.raw, &raws);
        drop(buffers);
    }

    /// Starts an asynchronous read of a `MAP_READ` buffer. The result arrives
    /// from a later [`Gpu::completed_reads`] call.
    pub fn read_buffer(&self, buffer: &Buffer, offset: u64, size: u64) -> ReadTicket {
        backend::read_buffer(&self.raw, buffer.raw, offset, size)
    }

    /// Returns reads that finished since the last call. Natively this also
    /// processes pending device events.
    pub fn completed_reads(&self) -> Vec<(ReadTicket, Vec<u8>)> {
        backend::completed_reads(&self.raw)
    }

    /// The current presentable texture view (or the offscreen target natively).
    pub fn surface_view(&self) -> TextureView {
        TextureView { raw: backend::surface_view(&self.raw) }
    }

    pub fn surface_format(&self) -> TextureFormat {
        backend::surface_format(&self.raw)
    }

    /// Presents the current surface texture, when there is one.
    pub fn present(&self) {
        backend::present(&self.raw);
    }

    /// Natively, configures a window surface; on the web the host owns the canvas.
    pub fn resize_surface(&self, width: u32, height: u32) {
        backend::resize_surface(&self.raw, width, height);
    }

    /// Blocks until queued work finishes. Native only; a no-op on the web.
    pub fn wait_idle(&self) {
        backend::wait_idle(&self.raw);
    }

    /// Copies a texture's first mip into a mappable buffer (rows padded to 256 bytes).
    pub fn copy_texture_to_buffer(&self, encoder: &mut CommandEncoder, texture: &Texture, buffer: &Buffer, bytes_per_row: u32, size: Extent) {
        backend::copy_texture_to_buffer(encoder.raw, texture.raw, buffer.raw, bytes_per_row, size);
    }

    /// The offscreen color target natively, for screenshots and image checks.
    pub fn surface_texture(&self) -> Option<Texture> {
        backend::surface_texture(&self.raw).map(|raw| Texture { raw })
    }

    pub fn shader_errors(&self) -> Vec<String> {
        backend::take_errors(&self.raw)
    }
}

impl CommandEncoder {
    pub fn begin_compute_pass(&mut self, label: &str, timestamps: Option<PassTimestamps>) -> ComputePass<'_> {
        count(|c| c.passes += 1);
        ComputePass { raw: backend::begin_compute_pass(self.raw, label, timestamps), _encoder: Default::default() }
    }

    pub fn begin_render_pass(&mut self, desc: &RenderPassDescriptor) -> RenderPass<'_> {
        count(|c| c.passes += 1);
        RenderPass { raw: backend::begin_render_pass(self.raw, desc), _encoder: Default::default() }
    }

    pub fn copy_buffer_to_buffer(&mut self, src: &Buffer, src_offset: u64, dst: &Buffer, dst_offset: u64, size: u64) {
        backend::copy_buffer_to_buffer(self.raw, src.raw, src_offset, dst.raw, dst_offset, size);
    }

    pub fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64) {
        backend::clear_buffer(self.raw, buffer.raw, offset, size);
    }

    pub fn resolve_queries(&mut self, set: &QuerySet, first: u32, count: u32, dst: &Buffer, offset: u64) {
        backend::resolve_queries(self.raw, set.raw, first, count, dst.raw, offset);
    }

    pub fn finish(self) -> CommandBuffer {
        let raw = backend::finish(self.raw);
        std::mem::forget(self);
        CommandBuffer { raw }
    }
}

impl Drop for CommandEncoder {
    fn drop(&mut self) {
        backend::release(Kind::CommandEncoder, self.raw);
    }
}

impl ComputePass<'_> {
    pub fn set_pipeline(&mut self, pipeline: &ComputePipeline) {
        count(|c| c.pipeline_binds += 1);
        backend::compute_set_pipeline(self.raw, pipeline.raw);
    }

    pub fn set_bind_group(&mut self, index: u32, group: &BindGroup) {
        count(|c| c.bind_group_binds += 1);
        backend::compute_set_bind_group(self.raw, index, group.raw);
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        count(|c| c.dispatches += 1);
        backend::dispatch(self.raw, x, y, z);
    }

    pub fn dispatch_indirect(&mut self, buffer: &Buffer, offset: u64) {
        count(|c| c.dispatches += 1);
        backend::dispatch_indirect(self.raw, buffer.raw, offset);
    }

    /// Hands the commands recorded so far to the GPU API now, rather than at
    /// `end`; for timing. Only batched web calls have anything to hand over.
    pub fn flush(&mut self) {
        #[cfg(target_arch = "wasm32")]
        web::flush();
    }

    pub fn end(self) {
        backend::end_compute_pass(self.raw);
        std::mem::forget(self);
    }
}

impl Drop for ComputePass<'_> {
    fn drop(&mut self) {
        backend::end_compute_pass(self.raw);
    }
}

impl RenderPass<'_> {
    pub fn raw(&self) -> Handle {
        self.raw
    }

    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        count(|c| c.pipeline_binds += 1);
        backend::render_set_pipeline(self.raw, pipeline.raw);
    }

    pub fn set_bind_group(&mut self, index: u32, group: &BindGroup) {
        count(|c| c.bind_group_binds += 1);
        backend::render_set_bind_group(self.raw, index, group.raw);
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &Buffer, offset: u64, size: u64) {
        count(|c| c.buffer_binds += 1);
        backend::set_vertex_buffer(self.raw, slot, buffer.raw, offset, size);
    }

    pub fn set_index_buffer(&mut self, buffer: &Buffer, format: IndexFormat, offset: u64, size: u64) {
        count(|c| c.buffer_binds += 1);
        backend::set_index_buffer(self.raw, buffer.raw, format, offset, size);
    }

    pub fn draw(&mut self, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) {
        count(|c| c.draws += 1);
        backend::draw(self.raw, vertices, instances, first_vertex, first_instance);
    }

    pub fn draw_indexed(&mut self, indices: u32, instances: u32, first_index: u32, base_vertex: i32, first_instance: u32) {
        count(|c| c.draws += 1);
        backend::draw_indexed(self.raw, indices, instances, first_index, base_vertex, first_instance);
    }

    pub fn draw_indirect(&mut self, buffer: &Buffer, offset: u64) {
        count(|c| c.draws += 1);
        backend::draw_indirect(self.raw, buffer.raw, offset);
    }

    pub fn draw_indexed_indirect(&mut self, buffer: &Buffer, offset: u64) {
        count(|c| c.draws += 1);
        backend::draw_indexed_indirect(self.raw, buffer.raw, offset);
    }

    /// Hands the commands recorded so far to the GPU API now, rather than at
    /// `end`; for timing. Only batched web calls have anything to hand over.
    pub fn flush(&mut self) {
        #[cfg(target_arch = "wasm32")]
        web::flush();
    }

    pub fn end(self) {
        backend::end_render_pass(self.raw);
        std::mem::forget(self);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        backend::end_render_pass(self.raw);
    }
}

/// Batches web pass commands (the default) or makes one import call per
/// command. Only the browser backend has calls to batch; elsewhere this does
/// nothing.
pub fn set_batched_calls(on: bool) {
    #[cfg(target_arch = "wasm32")]
    web::set_batched(on);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = on;
}

/// Whether web pass commands are batched; always `false` off the web.
pub fn batched_calls() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::batched();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Monotonic milliseconds for CPU timing on either platform.
pub fn now_ms() -> f64 {
    backend::now_ms()
}

/// Writes a line to the host's log.
pub fn log(message: &str) {
    backend::log(message);
}
