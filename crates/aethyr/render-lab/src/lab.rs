//! The render lab: one renderer with three visibility paths, three
//! anti-aliasing modes, LOD, occlusion culling, and crowd tiers, all switchable
//! at runtime so each technique in decision 0007 can be measured against the
//! CPU-batched path the current engine resembles.

use crate::camera::{self, CameraMode, Orbit};
use crate::crowd;
use crate::scene::{self, Geometry, Instance, LODS, MATERIALS};
use af_gpu::*;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};

const COMMON: &str = include_str!("../shaders/common.wgsl");
const FORWARD: &str = include_str!("../shaders/forward.wgsl");
const CULL: &str = include_str!("../shaders/cull.wgsl");
const HIZ: &str = include_str!("../shaders/hiz.wgsl");
const VISBUFFER: &str = include_str!("../shaders/visbuffer.wgsl");
const CROWD: &str = include_str!("../shaders/crowd.wgsl");
const POST: &str = include_str!("../shaders/post.wgsl");
/// The culling pass compiled from Slang (decision 0008); see shaders/slang.
const CULL_SLANG: &str = include_str!("../shaders/generated/cull.slang.wgsl");

const STATS_WORDS: u64 = 8;
const TIMESTAMP_SLOTS: u32 = 12;
const READ_RING: usize = 3;
/// Query resolves need 256-byte aligned destinations, so each slot's pair of
/// timestamps resolves into its own 256-byte block.
const RESOLVE_STRIDE: u64 = 256;
const HDR: TextureFormat = TextureFormat::Rgba16Float;
const DEPTH: TextureFormat = TextureFormat::Depth32Float;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Path {
    /// CPU frustum culling, per-material binds, one draw per mesh LOD and
    /// material: the shape of the current engine's submission.
    Cpu,
    /// GPU culling and indirect draws per mesh LOD, forward shading.
    Gpu,
    /// GPU culling, cluster culling, ID raster, and compute material resolve.
    Visbuffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aa {
    None,
    Msaa4,
    Taa,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoder {
    #[default]
    Rust,
    /// The CPU path's draws are encoded by TypeScript instead (web only).
    Ts,
}

/// How the web backend reaches WebGPU (ADR 0004): one replay call per pass
/// (the default) or one import call per command, for A/B runs. Native hosts
/// call `webgpu.h` directly either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Calls {
    #[default]
    Batched,
    PerCall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShaderSource {
    Wgsl,
    Slang,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub path: Path,
    pub aa: Aa,
    pub render_scale: f32,
    pub occlusion: bool,
    pub lod: bool,
    pub instances: u32,
    pub variants: u32,
    pub crowd: u32,
    pub crowd_tiers: bool,
    pub camera: CameraMode,
    pub encoder: Encoder,
    pub calls: Calls,
    pub cull_shader: ShaderSource,
    pub seed: u64,
    /// Fixed scene time in seconds, for repeatable captures.
    pub time: Option<f32>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            path: Path::Gpu,
            aa: Aa::Taa,
            render_scale: 1.0,
            occlusion: true,
            lod: true,
            instances: 50_000,
            variants: 16,
            crowd: 20_000,
            crowd_tiers: true,
            camera: CameraMode::Street,
            encoder: Encoder::Rust,
            calls: Calls::Batched,
            cull_shader: ShaderSource::Wgsl,
            seed: 1,
            time: None,
        }
    }
}

impl Config {
    /// Settings that cannot combine fall back to ones that can.
    pub fn normalized(mut self) -> Config {
        self.render_scale = self.render_scale.clamp(0.25, 1.0);
        self.instances = self.instances.clamp(1_000, 2_000_000);
        self.variants = self.variants.clamp(1, 32);
        self.crowd = self.crowd.min(1_000_000);
        if self.path == Path::Visbuffer && self.aa == Aa::Msaa4 {
            self.aa = Aa::Taa;
        }
        if self.path == Path::Cpu {
            self.occlusion = false;
        }
        if self.path != Path::Cpu {
            self.encoder = Encoder::Rust;
        }
        if !slang_available() {
            self.cull_shader = ShaderSource::Wgsl;
        }
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [f32; 16],
    view_proj_nj: [f32; 16],
    prev_view_proj: [f32; 16],
    inv_view_proj: [f32; 16],
    camera: [f32; 4],
    sun: [f32; 4],
    planes: [[f32; 4]; 6],
    viewport: [f32; 4],
    jitter: [f32; 4],
    flags: [u32; 4],
    lod: [f32; 4],
    tiers: [f32; 4],
}

/// Per-pass GPU time in milliseconds, from timestamp queries.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PassTimes {
    pub cull_1: f64,
    pub expand_1: f64,
    pub raster_1: f64,
    pub hiz: f64,
    pub cull_2: f64,
    pub expand_2: f64,
    pub raster_2: f64,
    pub resolve: f64,
    pub crowd_cull: f64,
    pub crowd: f64,
    pub taa: f64,
    pub blit: f64,
    /// First pass start to last pass end.
    pub total: f64,
}

/// Where the CPU path's main-pass encode time went, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
struct EncodeSplit {
    total: f64,
    replay: f64,
    log: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Stats {
    pub frame: u64,
    /// Increments on every configuration change or resize.
    pub epoch: u32,
    /// The epoch the GPU counters and timings below were measured in. They
    /// arrive a few frames late; checks wait until this equals `epoch`.
    pub gpu_epoch: u32,
    pub render_size: [u32; 2],
    pub output_size: [u32; 2],
    pub visible_instances: u32,
    pub triangles: u32,
    pub clusters: u32,
    pub occluded_instances: u32,
    pub crowd_near: u32,
    pub crowd_mid: u32,
    pub crowd_far: u32,
    pub total_instances: u32,
    /// CPU milliseconds inside the lab's frame function.
    pub cpu_frame_ms: f64,
    /// CPU milliseconds spent culling and bucketing on the CPU path.
    pub cpu_cull_ms: f64,
    /// CPU milliseconds encoding the CPU path's main pass draws.
    pub main_encode_ms: f64,
    /// Of `main_encode_ms`, the part spent replaying batched calls in
    /// JavaScript (zero when calls are not batched or TypeScript encodes).
    pub main_replay_ms: f64,
    /// Rust milliseconds building the draw log the TypeScript encoder reads;
    /// not part of `main_encode_ms`, which times only the TypeScript loop.
    pub main_log_ms: f64,
    pub encoder: Encoder,
    /// Whether the web backend batched its calls this frame.
    pub batched_calls: bool,
    pub commands: CommandCounts,
    pub gpu: PassTimes,
    pub gpu_timestamps: bool,
    pub adapter: AdapterInfo,
    pub errors: Vec<String>,
    pub config: Config,
}

struct Assets {
    geometry: Geometry,
    vertex: Buffer,
    index: Buffer,
    lods: Buffer,
    bounds: Buffer,
    clusters: Buffer,
    materials: Buffer,
    _textures: Texture,
    textures_view: TextureView,
    sampler: Sampler,
    clamp_sampler: Sampler,
    material_uniforms: Vec<Buffer>,
    char_vertex: Buffer,
    char_bones: Buffer,
    char_index: Buffer,
    char_index_count: u32,
    low_index: Buffer,
    low_index_count: u32,
    _vat: Texture,
    vat_view: TextureView,
    _atlas: Texture,
    atlas_view: TextureView,
}

struct World {
    instances: Vec<Instance>,
    side: f32,
    total: u32,
    instance_buf: Buffer,
    visibility: Buffer,
    bucket_base: Vec<u32>,
    bucket_cap: Vec<u32>,
    bucket_base_buf: Buffer,
    list_a: Buffer,
    list_b: Buffer,
    args_a: Buffer,
    args_b: Buffer,
    bucket_template: Buffer,
    zero_template: Buffer,
    args_size: u64,
    dispatch_args: Buffer,
    clusters_a: Buffer,
    clusters_b: Buffer,
    draw_args_a: Buffer,
    draw_args_b: Buffer,
    cluster_template: Buffer,
    crowd_near: Buffer,
    crowd_mid: Buffer,
    crowd_far: Buffer,
    crowd_args: Buffer,
    crowd_template: Buffer,
    tag_a: Buffer,
    tag_b: Buffer,
}

struct Targets {
    out: [u32; 2],
    size: [u32; 2],
    _hdr: Texture,
    hdr_view: TextureView,
    _hdr_msaa: Option<Texture>,
    hdr_msaa_view: Option<TextureView>,
    _depth: Texture,
    depth_view: TextureView,
    _depth_msaa: Option<Texture>,
    depth_msaa_view: Option<TextureView>,
    _vis: Texture,
    vis_view: TextureView,
    _hiz: Texture,
    hiz_view: TextureView,
    hiz_mips: Vec<TextureView>,
    _history: [Texture; 2],
    history_views: [TextureView; 2],
}

/// One sample count's forward and crowd pipelines.
struct RasterSet {
    sky: RenderPipeline,
    forward_cpu: RenderPipeline,
    forward_gpu: RenderPipeline,
    near: RenderPipeline,
    mid: RenderPipeline,
    far: RenderPipeline,
}

struct Pipelines {
    raster: [RasterSet; 2],
    cull: Vec<(ShaderSource, &'static str, ComputePipeline)>,
    hiz_first: ComputePipeline,
    hiz_first_msaa: ComputePipeline,
    hiz_down: ComputePipeline,
    prepare_dispatch: ComputePipeline,
    expand: ComputePipeline,
    raster_ids: RenderPipeline,
    resolve: ComputePipeline,
    crowd_cull: ComputePipeline,
    taa: ComputePipeline,
    blit: RenderPipeline,
}

#[derive(Default)]
struct Bindings {
    sky: Vec<BindGroup>,
    forward_cpu: Vec<BindGroup>,
    forward_gpu: Vec<BindGroup>,
    materials: Vec<Vec<BindGroup>>,
    cull: Vec<BindGroup>,
    hiz_first: Option<BindGroup>,
    hiz_down: Vec<BindGroup>,
    prepare: Vec<BindGroup>,
    expand: Vec<BindGroup>,
    raster_ids: Vec<BindGroup>,
    resolve: Option<BindGroup>,
    crowd_cull: Option<BindGroup>,
    near: Vec<BindGroup>,
    mid: Vec<BindGroup>,
    far: Vec<BindGroup>,
    taa: Vec<BindGroup>,
    blit_hdr: Option<BindGroup>,
    blit_history: Vec<BindGroup>,
}

struct Readback {
    stats: [Buffer; READ_RING],
    times: [Buffer; READ_RING],
    pending: [Option<(ReadTicket, ReadTicket, u32)>; READ_RING],
    partial: Vec<(ReadTicket, Vec<u8>)>,
}

#[derive(Clone, Copy)]
struct CpuDraw {
    material: u32,
    offset: u32,
    count: u32,
    index_count: u32,
    first_index: u32,
    base_vertex: u32,
}

/// Hook for encoding the CPU path's draws outside Rust (TypeScript on the web).
pub type ExternalEncoder = fn(pass: Handle, log: &[u32]) -> Option<f64>;

pub struct Lab {
    gpu: Gpu,
    config: Config,
    assets: Assets,
    world: World,
    targets: Targets,
    pipelines: Pipelines,
    bindings: Bindings,
    frame_buf: Buffer,
    taa_buf: Buffer,
    stats_buf: Buffer,
    queries: Option<QuerySet>,
    query_resolve: Option<Buffer>,
    readback: Readback,
    frame: u64,
    prev_view_proj: Mat4,
    taa_reset: bool,
    orbit: Orbit,
    stats: Stats,
    scratch_keys: Vec<u32>,
    scratch_counts: Vec<u32>,
    scratch_list: Vec<u32>,
    cpu_draws: Vec<CpuDraw>,
    external_encoder: Option<ExternalEncoder>,
    /// Timestamp slots the current frame's passes write, one bit per slot.
    written_slots: std::cell::Cell<u32>,
}

fn storage_buffer(gpu: &Gpu, label: &str, size: u64, extra: u32) -> Buffer {
    gpu.create_buffer(&BufferDescriptor { label, size: size.max(16).next_multiple_of(16), usage: buffer_usage::STORAGE | buffer_usage::COPY_DST | extra })
}

fn texture(gpu: &Gpu, label: &str, size: [u32; 2], format: TextureFormat, usage: u32, samples: u32, mips: u32) -> (Texture, TextureView) {
    let t = gpu.create_texture(&TextureDescriptor {
        label,
        size: Extent { width: size[0].max(1), height: size[1].max(1), depth_or_array_layers: 1 },
        mip_level_count: mips,
        sample_count: samples,
        format,
        usage,
    });
    let v = gpu.create_view(&t, &TextureViewDescriptor::default());
    (t, v)
}

fn buf<'a>(b: &'a Buffer) -> BindingResource<'a> {
    BindingResource::Buffer { buffer: b, offset: 0, size: None }
}

fn entries<'a>(list: Vec<(u32, BindingResource<'a>)>) -> Vec<BindGroupEntry<'a>> {
    list.into_iter().map(|(binding, resource)| BindGroupEntry { binding, resource }).collect()
}

impl Lab {
    pub fn new(gpu: Gpu, config: Config, width: u32, height: u32) -> Lab {
        let config = config.normalized();
        af_gpu::set_batched_calls(config.calls == Calls::Batched);
        let assets = Self::build_assets(&gpu, config.variants);
        let world = Self::build_world(&gpu, &assets, &config);
        let targets = Self::build_targets(&gpu, &config, width, height);
        let pipelines = Self::build_pipelines(&gpu);
        let frame_buf = gpu.create_buffer(&BufferDescriptor { label: "frame", size: size_of::<FrameUniform>() as u64, usage: buffer_usage::UNIFORM | buffer_usage::COPY_DST });
        let taa_buf = gpu.create_buffer(&BufferDescriptor { label: "taa", size: 16, usage: buffer_usage::UNIFORM | buffer_usage::COPY_DST });
        let stats_buf = storage_buffer(&gpu, "stats", STATS_WORDS * 4, buffer_usage::COPY_SRC);
        let queries = gpu.create_timestamp_queries(TIMESTAMP_SLOTS * 2);
        let query_resolve = queries.as_ref().map(|_| {
            gpu.create_buffer(&BufferDescriptor { label: "query resolve", size: TIMESTAMP_SLOTS as u64 * RESOLVE_STRIDE, usage: buffer_usage::QUERY_RESOLVE | buffer_usage::COPY_SRC })
        });
        let read = |label: &str, size: u64| gpu.create_buffer(&BufferDescriptor { label, size, usage: buffer_usage::MAP_READ | buffer_usage::COPY_DST });
        let readback = Readback {
            stats: std::array::from_fn(|_| read("stats read", STATS_WORDS * 4)),
            times: std::array::from_fn(|_| read("times read", TIMESTAMP_SLOTS as u64 * 16)),
            pending: [None; READ_RING],
            partial: Vec::new(),
        };
        let mut lab = Lab {
            stats: Stats { adapter: gpu.info().clone(), gpu_timestamps: queries.is_some(), ..Default::default() },
            gpu,
            config,
            assets,
            world,
            targets,
            pipelines,
            bindings: Bindings::default(),
            frame_buf,
            taa_buf,
            stats_buf,
            queries,
            query_resolve,
            readback,
            frame: 0,
            prev_view_proj: Mat4::IDENTITY,
            taa_reset: true,
            orbit: Orbit::default(),
            scratch_keys: Vec::new(),
            scratch_counts: Vec::new(),
            scratch_list: Vec::new(),
            cpu_draws: Vec::new(),
            external_encoder: None,
            written_slots: std::cell::Cell::new(0),
        };
        lab.bake_impostors();
        lab.rebuild_bindings();
        lab
    }

    pub fn set_external_encoder(&mut self, hook: ExternalEncoder) {
        self.external_encoder = Some(hook);
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn orbit(&mut self, dx: f32, dy: f32, zoom: f32) {
        self.orbit.yaw += dx * 0.005;
        self.orbit.pitch = (self.orbit.pitch + dy * 0.005).clamp(0.05, 1.5);
        self.orbit.distance = (self.orbit.distance * (1.0 + zoom * 0.001)).clamp(5.0, 1500.0);
    }

    pub fn set_config(&mut self, config: Config) {
        let config = config.normalized();
        af_gpu::set_batched_calls(config.calls == Calls::Batched);
        let old = std::mem::replace(&mut self.config, config.clone());
        if old.variants != config.variants {
            self.assets = Self::build_assets(&self.gpu, config.variants);
            self.bake_impostors();
        }
        if old.variants != config.variants || old.instances != config.instances || old.crowd != config.crowd || old.seed != config.seed {
            self.world = Self::build_world(&self.gpu, &self.assets, &self.config);
        }
        if old.aa != config.aa || old.render_scale != config.render_scale {
            self.targets = Self::build_targets(&self.gpu, &self.config, self.targets.out[0], self.targets.out[1]);
        }
        self.taa_reset = true;
        self.stats.epoch += 1;
        self.rebuild_bindings();
    }

    /// Reads back the last presented image as tightly packed RGBA8. Only
    /// backends that render offscreen (native without a window) support it.
    pub fn capture(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let texture = self.gpu.surface_texture()?;
        let [w, h] = self.targets.out;
        let row = (w * 4).next_multiple_of(256);
        let size = row as u64 * h as u64;
        let buffer = self.gpu.create_buffer(&BufferDescriptor { label: "capture", size, usage: buffer_usage::MAP_READ | buffer_usage::COPY_DST });
        let mut encoder = self.gpu.create_command_encoder();
        self.gpu.copy_texture_to_buffer(&mut encoder, &texture, &buffer, row, Extent { width: w, height: h, depth_or_array_layers: 1 });
        self.gpu.submit(vec![encoder.finish()]);
        let ticket = self.gpu.read_buffer(&buffer, 0, size);
        let mut bytes = None;
        for _ in 0..100 {
            self.gpu.wait_idle();
            for (t, data) in self.gpu.completed_reads() {
                if t == ticket {
                    bytes = Some(data);
                } else {
                    // The frame loop's stats reads; keep them for collect_reads.
                    self.readback.partial.push((t, data));
                }
            }
            if bytes.is_some() {
                break;
            }
        }
        let bytes = bytes?;
        let bgra = self.gpu.surface_format() == TextureFormat::Bgra8Unorm;
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            for px in bytes[y * row as usize..][..w as usize * 4].as_chunks::<4>().0 {
                let (r, b) = if bgra { (px[2], px[0]) } else { (px[0], px[2]) };
                out.extend_from_slice(&[r, px[1], b, 255]);
            }
        }
        Some((w, h, out))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if [width, height] == self.targets.out {
            return;
        }
        self.gpu.resize_surface(width, height);
        self.targets = Self::build_targets(&self.gpu, &self.config, width, height);
        self.taa_reset = true;
        self.stats.epoch += 1;
        self.rebuild_bindings();
    }

    fn build_assets(gpu: &Gpu, variants: u32) -> Assets {
        let geometry = Geometry::build(variants);
        let usage_vs = buffer_usage::VERTEX | buffer_usage::STORAGE;
        let vertex = gpu.create_buffer_init("vertices", usage_vs, cast_slice(&geometry.vertices));
        let index = gpu.create_buffer_init("indices", buffer_usage::INDEX | buffer_usage::STORAGE, cast_slice(&geometry.indices));
        let lods = gpu.create_buffer_init("lods", buffer_usage::STORAGE, cast_slice(&geometry.lods));
        let bounds = gpu.create_buffer_init("bounds", buffer_usage::STORAGE, cast_slice(&geometry.bounds));
        let clusters = gpu.create_buffer_init("clusters", buffer_usage::STORAGE, cast_slice(&geometry.clusters));
        let materials = gpu.create_buffer_init("materials", buffer_usage::STORAGE, cast_slice(&scene::materials()));
        let textures = gpu.create_texture(&TextureDescriptor {
            label: "material textures",
            size: Extent { width: scene::TEXTURE_SIZE, height: scene::TEXTURE_SIZE, depth_or_array_layers: MATERIALS as u32 },
            mip_level_count: scene::TEXTURE_MIPS,
            sample_count: 1,
            format: TextureFormat::Rgba8Unorm,
            usage: texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST,
        });
        for (layer, mips) in scene::textures().iter().enumerate() {
            for (mip, data) in mips.iter().enumerate() {
                let s = scene::TEXTURE_SIZE >> mip;
                gpu.write_texture(&textures, mip as u32, layer as u32, data, s * 4, Extent { width: s, height: s, depth_or_array_layers: 1 });
            }
        }
        let textures_view = gpu.create_view(&textures, &TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..Default::default() });
        let sampler = gpu.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            max_anisotropy: Some(8),
        });
        let clamp_sampler = gpu.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            max_anisotropy: None,
        });
        let material_uniforms = (0..MATERIALS as u32)
            .map(|m| gpu.create_buffer_init("material index", buffer_usage::UNIFORM, bytes_of(&[m, 0u32, 0, 0])))
            .collect();
        let character = scene::character();
        let char_vertex = gpu.create_buffer_init("character vertices", buffer_usage::VERTEX, cast_slice(&character.mesh.vertices));
        let char_bones = gpu.create_buffer_init("character bones", buffer_usage::VERTEX, cast_slice(&character.bones));
        let char_index = gpu.create_buffer_init("character indices", buffer_usage::INDEX, cast_slice(&character.mesh.indices));
        let low_index = gpu.create_buffer_init("low character indices", buffer_usage::INDEX, cast_slice(&character.low.indices));
        let vat_width = character.low.vertices.len() as u32;
        let vat_data = crowd::bake_vat(&character.low, &character.low_bones);
        let vat = gpu.create_texture(&TextureDescriptor {
            label: "crowd vat",
            size: Extent { width: vat_width, height: crowd::VAT_FRAMES * 2, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            format: TextureFormat::Rgba32Float,
            usage: texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST,
        });
        gpu.write_texture(&vat, 0, 0, cast_slice(&vat_data), vat_width * 16, Extent { width: vat_width, height: crowd::VAT_FRAMES * 2, depth_or_array_layers: 1 });
        let vat_view = gpu.create_view(&vat, &TextureViewDescriptor::default());
        let (atlas, atlas_view) = texture(
            gpu,
            "impostor atlas",
            [crowd::ATLAS_SIZE.0, crowd::ATLAS_SIZE.1],
            TextureFormat::Rgba8Unorm,
            texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT,
            1,
            1,
        );
        Assets {
            char_index_count: character.mesh.indices.len() as u32,
            low_index_count: character.low.indices.len() as u32,
            geometry,
            vertex,
            index,
            lods,
            bounds,
            clusters,
            materials,
            _textures: textures,
            textures_view,
            sampler,
            clamp_sampler,
            material_uniforms,
            char_vertex,
            char_bones,
            char_index,
            low_index,
            _vat: vat,
            vat_view,
            _atlas: atlas,
            atlas_view,
        }
    }

    fn build_world(gpu: &Gpu, assets: &Assets, config: &Config) -> World {
        let g = &assets.geometry;
        let instances = scene::place_instances(g, config.instances, config.seed);
        let total = instances.len() as u32;
        let buckets = g.mesh_count() * LODS;
        let mut per_mesh = vec![0u32; g.mesh_count()];
        for i in &instances {
            per_mesh[i.mesh as usize] += 1;
        }
        let mut bucket_base = Vec::with_capacity(buckets);
        let mut bucket_cap = Vec::with_capacity(buckets);
        let mut template = Vec::with_capacity(buckets * 5);
        let mut offset = 0u32;
        for (mesh, &count) in per_mesh.iter().enumerate() {
            for lod in 0..LODS {
                let level = g.lods[mesh * LODS + lod];
                bucket_base.push(offset);
                bucket_cap.push(count);
                offset += count;
                template.extend_from_slice(&[level.index_count, 0, level.first_index, level.base_vertex, 0]);
            }
        }
        let list_words = (total as u64 * LODS as u64).max(total as u64 * 2);
        let args_size = (buckets as u64 * 20).max(16).next_multiple_of(16);
        let args = |label| storage_buffer(gpu, label, args_size, buffer_usage::INDIRECT | buffer_usage::COPY_SRC);
        let mut zero = vec![0u32; (args_size / 4) as usize];
        zero[1] = 0;
        let cluster_capacity = (total as u64 * 48).clamp(262_144, 4_194_304);
        let crowd_capacity = config.crowd.max(1) as u64;
        let crowd_template: [u32; 16] = [assets.char_index_count, 0, 0, 0, 0, assets.low_index_count, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0];
        let vertex_list = |label| storage_buffer(gpu, label, crowd_capacity * 4, buffer_usage::VERTEX);
        World {
            side: scene::world_side(config.instances),
            total,
            instance_buf: gpu.create_buffer_init("instances", buffer_usage::STORAGE, cast_slice(&instances)),
            instances,
            visibility: storage_buffer(gpu, "visibility", total as u64 * 4, 0),
            bucket_base_buf: gpu.create_buffer_init("bucket base", buffer_usage::STORAGE, cast_slice(&bucket_base)),
            bucket_base,
            bucket_cap,
            list_a: storage_buffer(gpu, "list a", list_words * 4, buffer_usage::VERTEX),
            list_b: storage_buffer(gpu, "list b", list_words * 4, buffer_usage::VERTEX),
            args_a: args("args a"),
            args_b: args("args b"),
            bucket_template: gpu.create_buffer_init("bucket template", buffer_usage::COPY_SRC, cast_slice(&{
                let mut t = template.clone();
                t.resize((args_size / 4) as usize, 0);
                t
            })),
            zero_template: gpu.create_buffer_init("zero template", buffer_usage::COPY_SRC, cast_slice(&zero)),
            args_size,
            dispatch_args: storage_buffer(gpu, "dispatch args", 16, buffer_usage::INDIRECT),
            clusters_a: storage_buffer(gpu, "clusters a", cluster_capacity * 8, 0),
            clusters_b: storage_buffer(gpu, "clusters b", cluster_capacity * 8, 0),
            draw_args_a: storage_buffer(gpu, "draw args a", 16, buffer_usage::INDIRECT),
            draw_args_b: storage_buffer(gpu, "draw args b", 16, buffer_usage::INDIRECT),
            cluster_template: gpu.create_buffer_init("cluster template", buffer_usage::COPY_SRC, cast_slice(&[(scene::CLUSTER_TRIANGLES * 3) as u32, 0, 0, 0])),
            crowd_near: vertex_list("crowd near"),
            crowd_mid: vertex_list("crowd mid"),
            crowd_far: vertex_list("crowd far"),
            crowd_args: storage_buffer(gpu, "crowd args", 64, buffer_usage::INDIRECT),
            crowd_template: gpu.create_buffer_init("crowd template", buffer_usage::COPY_SRC, cast_slice(&crowd_template)),
            tag_a: gpu.create_buffer_init("tag a", buffer_usage::UNIFORM, bytes_of(&[0u32; 4])),
            tag_b: gpu.create_buffer_init("tag b", buffer_usage::UNIFORM, bytes_of(&[0x8000_0000u32, 0, 0, 0])),
        }
    }

    fn build_targets(gpu: &Gpu, config: &Config, width: u32, height: u32) -> Targets {
        let out = [width.max(1), height.max(1)];
        let size = [((out[0] as f32 * config.render_scale).round() as u32).max(1), ((out[1] as f32 * config.render_scale).round() as u32).max(1)];
        let rt = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        let (hdr, hdr_view) = texture(gpu, "hdr", size, HDR, rt | texture_usage::STORAGE_BINDING, 1, 1);
        let (depth, depth_view) = texture(gpu, "depth", size, DEPTH, rt, 1, 1);
        let msaa = config.aa == Aa::Msaa4;
        let (hdr_msaa, hdr_msaa_view) = if msaa {
            let (t, v) = texture(gpu, "hdr msaa", size, HDR, texture_usage::RENDER_ATTACHMENT, 4, 1);
            (Some(t), Some(v))
        } else {
            (None, None)
        };
        let (depth_msaa, depth_msaa_view) = if msaa {
            let (t, v) = texture(gpu, "depth msaa", size, DEPTH, rt, 4, 1);
            (Some(t), Some(v))
        } else {
            (None, None)
        };
        let (vis, vis_view) = texture(gpu, "visibility ids", size, TextureFormat::Rg32Uint, rt, 1, 1);
        let hiz_size = [size[0].div_ceil(2), size[1].div_ceil(2)];
        let mips = 32 - hiz_size[0].max(hiz_size[1]).leading_zeros();
        let hiz = gpu.create_texture(&TextureDescriptor {
            label: "hiz",
            size: Extent { width: hiz_size[0], height: hiz_size[1], depth_or_array_layers: 1 },
            mip_level_count: mips,
            sample_count: 1,
            format: TextureFormat::R32Float,
            usage: texture_usage::TEXTURE_BINDING | texture_usage::STORAGE_BINDING,
        });
        let hiz_view = gpu.create_view(&hiz, &TextureViewDescriptor::default());
        let hiz_mips = (0..mips)
            .map(|m| gpu.create_view(&hiz, &TextureViewDescriptor { base_mip_level: m, mip_level_count: Some(1), ..Default::default() }))
            .collect();
        let history_usage = texture_usage::TEXTURE_BINDING | texture_usage::STORAGE_BINDING;
        let (h0, v0) = texture(gpu, "history 0", out, HDR, history_usage, 1, 1);
        let (h1, v1) = texture(gpu, "history 1", out, HDR, history_usage, 1, 1);
        Targets {
            out,
            size,
            _hdr: hdr,
            hdr_view,
            _hdr_msaa: hdr_msaa,
            hdr_msaa_view,
            _depth: depth,
            depth_view,
            _depth_msaa: depth_msaa,
            depth_msaa_view,
            _vis: vis,
            vis_view,
            _hiz: hiz,
            hiz_view,
            hiz_mips,
            _history: [h0, h1],
            history_views: [v0, v1],
        }
    }

    fn build_pipelines(gpu: &Gpu) -> Pipelines {
        let module = |label: &str, body: &str| gpu.create_shader_module(label, &format!("{COMMON}\n{body}"));
        let forward_cpu_src = FORWARD
            .replace("//MATERIAL_GROUP", "@group(1) @binding(0) var<uniform> material_index: vec4u;")
            .replace("//MATERIAL_SELECT", "let material = material_index.x; _ = input.material;");
        let forward_gpu_src = FORWARD.replace("//MATERIAL_SELECT", "let material = input.material;");
        let fwd_cpu = module("forward cpu", &forward_cpu_src);
        let fwd_gpu = module("forward gpu", &forward_gpu_src);
        let crowd_module = module("crowd", CROWD);
        let mesh_layout = || {
            vec![
                VertexBufferLayout {
                    array_stride: 32,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                        VertexAttribute { format: VertexFormat::Float32x2, offset: 24, shader_location: 2 },
                    ],
                },
                VertexBufferLayout { array_stride: 4, step_mode: VertexStepMode::Instance, attributes: vec![VertexAttribute { format: VertexFormat::Uint32, offset: 0, shader_location: 3 }] },
            ]
        };
        let agent_at = |location| VertexBufferLayout { array_stride: 4, step_mode: VertexStepMode::Instance, attributes: vec![VertexAttribute { format: VertexFormat::Uint32, offset: 0, shader_location: location }] };
        let character_layout = || {
            vec![
                VertexBufferLayout {
                    array_stride: 32,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                    ],
                },
                VertexBufferLayout { array_stride: 4, step_mode: VertexStepMode::Vertex, attributes: vec![VertexAttribute { format: VertexFormat::Uint32, offset: 0, shader_location: 2 }] },
            ]
        };
        let depth = |write: bool| Some(DepthState { format: DEPTH, depth_write_enabled: write, depth_compare: CompareFunction::Less });
        let raster_set = |samples: u32| {
            let render = |label: &str, module: &ShaderModule, vs: &str, fs: &str, buffers: Vec<VertexBufferLayout>| {
                gpu.create_render_pipeline(&RenderPipelineDescriptor {
                    label,
                    module,
                    vertex_entry: vs,
                    fragment_entry: Some(fs),
                    buffers,
                    cull_mode: CullMode::Back,
                    depth: depth(true),
                    sample_count: samples,
                    targets: vec![ColorTarget { format: HDR, blend: false }],
                })
            };
            let mut near_buffers = character_layout();
            near_buffers.push(agent_at(3));
            RasterSet {
                sky: gpu.create_render_pipeline(&RenderPipelineDescriptor {
                    label: "sky",
                    module: &fwd_gpu,
                    vertex_entry: "sky_vs",
                    fragment_entry: Some("sky_fs"),
                    buffers: vec![],
                    cull_mode: CullMode::None,
                    depth: Some(DepthState { format: DEPTH, depth_write_enabled: false, depth_compare: CompareFunction::LessEqual }),
                    sample_count: samples,
                    targets: vec![ColorTarget { format: HDR, blend: false }],
                }),
                forward_cpu: render("forward cpu", &fwd_cpu, "vs_main", "fs_main", mesh_layout()),
                forward_gpu: render("forward gpu", &fwd_gpu, "vs_main", "fs_main", mesh_layout()),
                near: render("crowd near", &crowd_module, "near_vs", "body_fs", near_buffers),
                mid: render("crowd mid", &crowd_module, "mid_vs", "body_fs", vec![agent_at(0)]),
                far: gpu.create_render_pipeline(&RenderPipelineDescriptor {
                    label: "crowd far",
                    module: &crowd_module,
                    vertex_entry: "far_vs",
                    fragment_entry: Some("far_fs"),
                    buffers: vec![agent_at(0)],
                    cull_mode: CullMode::None,
                    depth: depth(true),
                    sample_count: samples,
                    targets: vec![ColorTarget { format: HDR, blend: false }],
                }),
            }
        };
        let cull_wgsl = module("cull", CULL);
        let cull_slang = slang_available().then(|| gpu.create_shader_module("cull slang", CULL_SLANG));
        let mut cull = Vec::new();
        for entry in ["single_buckets", "phase1_buckets", "phase2_buckets", "single_list", "phase1_list", "phase2_list"] {
            cull.push((ShaderSource::Wgsl, entry, gpu.create_compute_pipeline(entry, &cull_wgsl, entry)));
            if let Some(m) = &cull_slang {
                cull.push((ShaderSource::Slang, entry, gpu.create_compute_pipeline(entry, m, entry)));
            }
        }
        let hiz = module("hiz", HIZ);
        let vis = module("visbuffer", VISBUFFER);
        let post = module("post", POST);
        let surface = gpu.surface_format();
        Pipelines {
            raster: [raster_set(1), raster_set(4)],
            cull,
            hiz_first: gpu.create_compute_pipeline("hiz first", &hiz, "first"),
            hiz_first_msaa: gpu.create_compute_pipeline("hiz first msaa", &hiz, "first_msaa"),
            hiz_down: gpu.create_compute_pipeline("hiz down", &hiz, "down"),
            prepare_dispatch: gpu.create_compute_pipeline("prepare dispatch", &vis, "prepare_dispatch"),
            expand: gpu.create_compute_pipeline("expand clusters", &vis, "expand"),
            raster_ids: gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label: "raster ids",
                module: &vis,
                vertex_entry: "raster_vs",
                fragment_entry: Some("raster_fs"),
                buffers: vec![],
                cull_mode: CullMode::Back,
                depth: depth(true),
                sample_count: 1,
                targets: vec![ColorTarget { format: TextureFormat::Rg32Uint, blend: false }],
            }),
            resolve: gpu.create_compute_pipeline("resolve", &vis, "resolve"),
            crowd_cull: gpu.create_compute_pipeline("crowd cull", &crowd_module, "crowd_cull"),
            taa: gpu.create_compute_pipeline("taa", &post, "taa_resolve"),
            blit: gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label: "blit",
                module: &post,
                vertex_entry: "blit_vs",
                fragment_entry: Some("blit_fs"),
                buffers: vec![],
                cull_mode: CullMode::None,
                depth: None,
                sample_count: 1,
                targets: vec![ColorTarget { format: surface, blend: false }],
            }),
        }
    }

    fn bake_impostors(&mut self) {
        let crowd_module = self.gpu.create_shader_module("crowd bake", &format!("{COMMON}\n{CROWD}"));
        let bake = self.gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "impostor bake",
            module: &crowd_module,
            vertex_entry: "bake_vs",
            fragment_entry: Some("bake_fs"),
            buffers: vec![
                VertexBufferLayout {
                    array_stride: 32,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        VertexAttribute { format: VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                    ],
                },
                VertexBufferLayout { array_stride: 4, step_mode: VertexStepMode::Vertex, attributes: vec![VertexAttribute { format: VertexFormat::Uint32, offset: 0, shader_location: 2 }] },
            ],
            cull_mode: CullMode::Back,
            depth: Some(DepthState { format: DEPTH, depth_write_enabled: true, depth_compare: CompareFunction::Less }),
            sample_count: 1,
            targets: vec![ColorTarget { format: TextureFormat::Rgba8Unorm, blend: false }],
        });
        let (_depth, depth_view) = texture(&self.gpu, "bake depth", [crowd::ATLAS_SIZE.0, crowd::ATLAS_SIZE.1], DEPTH, texture_usage::RENDER_ATTACHMENT, 1, 1);
        let a = &self.assets;
        let mut encoder = self.gpu.create_command_encoder();
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: "impostor bake",
                color: vec![ColorAttachment { view: &a.atlas_view, resolve_target: None, load: LoadOp::Clear([0.0; 4]), store: true }],
                depth: Some(DepthAttachment { view: &depth_view, clear: Some(1.0), store: false }),
                timestamps: None,
            });
            pass.set_pipeline(&bake);
            pass.set_vertex_buffer(0, &a.char_vertex, 0, a_size(&a.char_vertex));
            pass.set_vertex_buffer(1, &a.char_bones, 0, a_size(&a.char_bones));
            pass.set_index_buffer(&a.char_index, IndexFormat::Uint32, 0, a.char_index_count as u64 * 4);
            pass.draw_indexed(a.char_index_count, (crowd::ATLAS_SIZE.0 / 80) * (crowd::ATLAS_SIZE.1 / 128), 0, 0, 0);
            pass.end();
        }
        self.gpu.submit(vec![encoder.finish()]);
        take_command_counts();
    }

    fn rebuild_bindings(&mut self) {
        let gpu = &self.gpu;
        let a = &self.assets;
        let w = &self.world;
        let t = &self.targets;
        let p = &self.pipelines;
        let mut b = Bindings::default();
        for set in &p.raster {
            let common = |pipeline: &RenderPipeline| {
                gpu.create_bind_group(
                    "forward",
                    &gpu.render_bind_group_layout(pipeline, 0),
                    &entries(vec![
                        (0, buf(&self.frame_buf)),
                        (1, buf(&w.instance_buf)),
                        (2, buf(&a.materials)),
                        (3, BindingResource::TextureView(&a.textures_view)),
                        (4, BindingResource::Sampler(&a.sampler)),
                    ]),
                )
            };
            b.forward_cpu.push(common(&set.forward_cpu));
            b.sky.push(gpu.create_bind_group("sky", &gpu.render_bind_group_layout(&set.sky, 0), &entries(vec![(0, buf(&self.frame_buf))])));
            b.forward_gpu.push(common(&set.forward_gpu));
            let layout = gpu.render_bind_group_layout(&set.forward_cpu, 1);
            b.materials.push(a.material_uniforms.iter().map(|u| gpu.create_bind_group("material", &layout, &entries(vec![(0, buf(u))]))).collect());
            b.near.push(gpu.create_bind_group("crowd near", &gpu.render_bind_group_layout(&set.near, 0), &entries(vec![(0, buf(&self.frame_buf))])));
            b.mid.push(gpu.create_bind_group(
                "crowd mid",
                &gpu.render_bind_group_layout(&set.mid, 0),
                &entries(vec![(0, buf(&self.frame_buf)), (7, BindingResource::TextureView(&a.vat_view))]),
            ));
            b.far.push(gpu.create_bind_group(
                "crowd far",
                &gpu.render_bind_group_layout(&set.far, 0),
                &entries(vec![(0, buf(&self.frame_buf)), (8, BindingResource::TextureView(&a.atlas_view)), (9, BindingResource::Sampler(&a.clamp_sampler))]),
            ));
        }
        for (_, _, pipeline) in &p.cull {
            b.cull.push(gpu.create_bind_group(
                "cull",
                &gpu.compute_bind_group_layout(pipeline, 0),
                &entries(vec![
                    (0, buf(&self.frame_buf)),
                    (1, buf(&w.instance_buf)),
                    (2, buf(&a.bounds)),
                    (3, buf(&a.lods)),
                    (4, buf(&w.visibility)),
                    (5, buf(&w.args_a)),
                    (6, buf(&w.list_a)),
                    (7, buf(&w.args_b)),
                    (8, buf(&w.list_b)),
                    (9, buf(&w.bucket_base_buf)),
                    (10, BindingResource::TextureView(&t.hiz_view)),
                    (11, buf(&self.stats_buf)),
                ]),
            ));
        }
        let msaa = self.config.aa == Aa::Msaa4;
        b.hiz_first = Some(if msaa {
            gpu.create_bind_group(
                "hiz first msaa",
                &gpu.compute_bind_group_layout(&p.hiz_first_msaa, 0),
                &entries(vec![(1, BindingResource::TextureView(t.depth_msaa_view.as_ref().unwrap())), (3, BindingResource::TextureView(&t.hiz_mips[0]))]),
            )
        } else {
            gpu.create_bind_group(
                "hiz first",
                &gpu.compute_bind_group_layout(&p.hiz_first, 0),
                &entries(vec![(0, BindingResource::TextureView(&t.depth_view)), (3, BindingResource::TextureView(&t.hiz_mips[0]))]),
            )
        });
        let down_layout = gpu.compute_bind_group_layout(&p.hiz_down, 0);
        for m in 1..t.hiz_mips.len() {
            b.hiz_down.push(gpu.create_bind_group(
                "hiz down",
                &down_layout,
                &entries(vec![(2, BindingResource::TextureView(&t.hiz_mips[m - 1])), (3, BindingResource::TextureView(&t.hiz_mips[m]))]),
            ));
        }
        let prepare_layout = gpu.compute_bind_group_layout(&p.prepare_dispatch, 0);
        let expand_layout = gpu.compute_bind_group_layout(&p.expand, 0);
        let raster_layout = gpu.render_bind_group_layout(&p.raster_ids, 0);
        for (args, list, clusters, draw_args, tag) in [
            (&w.args_a, &w.list_a, &w.clusters_a, &w.draw_args_a, &w.tag_a),
            (&w.args_b, &w.list_b, &w.clusters_b, &w.draw_args_b, &w.tag_b),
        ] {
            b.prepare.push(gpu.create_bind_group("prepare", &prepare_layout, &entries(vec![(5, buf(args)), (6, buf(&w.dispatch_args))])));
            b.expand.push(gpu.create_bind_group(
                "expand",
                &expand_layout,
                &entries(vec![
                    (0, buf(&self.frame_buf)),
                    (1, buf(&w.instance_buf)),
                    (2, buf(&a.lods)),
                    (3, buf(&a.clusters)),
                    (4, buf(list)),
                    (5, buf(args)),
                    (7, buf(clusters)),
                    (8, buf(draw_args)),
                    (9, buf(&self.stats_buf)),
                ]),
            ));
            b.raster_ids.push(gpu.create_bind_group(
                "raster ids",
                &raster_layout,
                &entries(vec![
                    (0, buf(&self.frame_buf)),
                    (1, buf(&w.instance_buf)),
                    (3, buf(&a.clusters)),
                    (10, buf(&a.vertex)),
                    (11, buf(&a.index)),
                    (12, buf(clusters)),
                    (18, buf(tag)),
                ]),
            ));
        }
        b.resolve = Some(gpu.create_bind_group(
            "resolve",
            &gpu.compute_bind_group_layout(&p.resolve, 0),
            &entries(vec![
                (0, buf(&self.frame_buf)),
                (1, buf(&w.instance_buf)),
                (3, buf(&a.clusters)),
                (10, buf(&a.vertex)),
                (11, buf(&a.index)),
                (12, buf(&w.clusters_a)),
                (13, BindingResource::TextureView(&t.vis_view)),
                (14, buf(&a.materials)),
                (15, BindingResource::TextureView(&a.textures_view)),
                (16, BindingResource::Sampler(&a.sampler)),
                (17, BindingResource::TextureView(&t.hdr_view)),
                (19, buf(&w.clusters_b)),
            ]),
        ));
        b.crowd_cull = Some(gpu.create_bind_group(
            "crowd cull",
            &gpu.compute_bind_group_layout(&p.crowd_cull, 0),
            &entries(vec![
                (0, buf(&self.frame_buf)),
                (1, BindingResource::TextureView(&t.hiz_view)),
                (2, buf(&w.crowd_near)),
                (3, buf(&w.crowd_mid)),
                (4, buf(&w.crowd_far)),
                (5, buf(&w.crowd_args)),
                (6, buf(&self.stats_buf)),
            ]),
        ));
        let taa_layout = gpu.compute_bind_group_layout(&p.taa, 0);
        for k in 0..2 {
            b.taa.push(gpu.create_bind_group(
                "taa",
                &taa_layout,
                &entries(vec![
                    (0, buf(&self.frame_buf)),
                    (1, BindingResource::TextureView(&t.hdr_view)),
                    (2, BindingResource::TextureView(&t.depth_view)),
                    (3, BindingResource::TextureView(&t.history_views[1 - k])),
                    (4, BindingResource::Sampler(&a.clamp_sampler)),
                    (5, BindingResource::TextureView(&t.history_views[k])),
                    (6, buf(&self.taa_buf)),
                ]),
            ));
        }
        let blit_layout = gpu.render_bind_group_layout(&p.blit, 0);
        let blit = |view: &TextureView| gpu.create_bind_group("blit", &blit_layout, &entries(vec![(4, BindingResource::Sampler(&a.clamp_sampler)), (7, BindingResource::TextureView(view))]));
        b.blit_hdr = Some(blit(&t.hdr_view));
        b.blit_history = t.history_views.iter().map(blit).collect();
        self.bindings = b;
    }

    fn cull_index(&self, entry: &str) -> usize {
        self.pipelines.cull.iter().position(|(s, e, _)| *s == self.config.cull_shader && *e == entry).unwrap()
    }

    fn timestamps(&self, slot: u32) -> Option<PassTimestamps<'_>> {
        let q = self.queries.as_ref()?;
        self.written_slots.set(self.written_slots.get() | 1 << slot);
        Some(PassTimestamps { query_set: q, begin: slot * 2, end: slot * 2 + 1 })
    }

    /// Renders one frame at `time_ms` (host clock) and returns the latest stats.
    pub fn frame(&mut self, time_ms: f64) -> &Stats {
        let start = now_ms();
        self.written_slots.set(0);
        let cfg = self.config.clone();
        let t = cfg.time.unwrap_or((time_ms / 1000.0) as f32);
        let [w, h] = self.targets.size;
        let (eye, forward) = camera::pose(cfg.camera, t, self.world.side, &self.orbit);
        let view = camera::view(eye, forward);
        let proj = camera::projection(w as f32 / h as f32);
        let vp_nj = proj * view;
        let (jx, jy) = if cfg.aa == Aa::Taa { camera::halton_jitter(self.frame as u32) } else { (0.0, 0.0) };
        let jitter = [jx * 2.0 / w as f32, jy * 2.0 / h as f32];
        let vp = Mat4::from_translation(Vec3::new(jitter[0], jitter[1], 0.0)) * vp_nj;
        if self.frame == 0 || self.taa_reset {
            self.prev_view_proj = vp_nj;
        }
        let planes = camera::planes(vp_nj);
        let pixels_per_unit = h as f32 / (2.0 * (camera::FOV_Y * 0.5).tan());
        let hiz_mips = self.targets.hiz_mips.len() as f32;
        let uniform = FrameUniform {
            view_proj: vp.to_cols_array(),
            view_proj_nj: vp_nj.to_cols_array(),
            prev_view_proj: self.prev_view_proj.to_cols_array(),
            inv_view_proj: vp_nj.inverse().to_cols_array(),
            camera: [eye.x, eye.y, eye.z, t],
            sun: Vec4::new(0.45, 0.75, 0.35, 0.0).truncate().normalize().extend(2.6).to_array(),
            planes: planes.map(|p| p.to_array()),
            viewport: [w as f32, h as f32, 1.0 / w as f32, 1.0 / h as f32],
            jitter: [jitter[0], jitter[1], 0.0, 0.0],
            flags: [cfg.lod as u32, cfg.occlusion as u32, self.world.total, cfg.crowd],
            lod: [pixels_per_unit, 1.0, hiz_mips, 30.0],
            tiers: [140.0, 900.0, if cfg.crowd_tiers { 1.0 } else { 0.0 }, 0.0],
        };
        self.gpu.write_buffer(&self.frame_buf, 0, bytes_of(&uniform));
        self.gpu.write_buffer(&self.taa_buf, 0, bytes_of(&[0.9f32, if self.taa_reset { 1.0 } else { 0.0 }, 0.0, 0.0]));

        let mut cpu_cull_ms = 0.0;
        if cfg.path == Path::Cpu {
            let c0 = now_ms();
            self.cpu_cull(&planes, eye, pixels_per_unit);
            cpu_cull_ms = now_ms() - c0;
        }

        let mut encoder = self.gpu.create_command_encoder();
        let wd = &self.world;
        let template = if cfg.path == Path::Gpu { &wd.bucket_template } else { &wd.zero_template };
        encoder.copy_buffer_to_buffer(template, 0, &wd.args_a, 0, wd.args_size);
        encoder.copy_buffer_to_buffer(template, 0, &wd.args_b, 0, wd.args_size);
        encoder.copy_buffer_to_buffer(&wd.cluster_template, 0, &wd.draw_args_a, 0, 16);
        encoder.copy_buffer_to_buffer(&wd.cluster_template, 0, &wd.draw_args_b, 0, 16);
        encoder.copy_buffer_to_buffer(&wd.crowd_template, 0, &wd.crowd_args, 0, 64);
        encoder.clear_buffer(&self.stats_buf, 0, STATS_WORDS * 4);
        let mut split = EncodeSplit::default();
        match cfg.path {
            Path::Cpu => split = self.encode_cpu(&mut encoder),
            Path::Gpu => self.encode_gpu(&mut encoder),
            Path::Visbuffer => self.encode_visbuffer(&mut encoder),
        }
        if cfg.crowd > 0 {
            self.encode_crowd(&mut encoder);
        }
        let source = if cfg.aa == Aa::Taa {
            let k = (self.frame % 2) as usize;
            let [ow, oh] = self.targets.out;
            let mut pass = encoder.begin_compute_pass("taa", self.timestamps(10));
            pass.set_pipeline(&self.pipelines.taa);
            pass.set_bind_group(0, &self.bindings.taa[k]);
            pass.dispatch(ow.div_ceil(8), oh.div_ceil(8), 1);
            pass.end();
            &self.bindings.blit_history[k]
        } else {
            self.bindings.blit_hdr.as_ref().unwrap()
        };
        let surface = self.gpu.surface_view();
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: "blit",
                color: vec![ColorAttachment { view: &surface, resolve_target: None, load: LoadOp::Clear([0.0, 0.0, 0.0, 1.0]), store: true }],
                depth: None,
                timestamps: self.timestamps(11),
            });
            pass.set_pipeline(&self.pipelines.blit);
            pass.set_bind_group(0, source);
            pass.draw(3, 1, 0, 0);
            pass.end();
        }
        let ring = (self.frame % READ_RING as u64) as usize;
        let read_now = self.readback.pending[ring].is_none();
        if read_now {
            encoder.copy_buffer_to_buffer(&self.stats_buf, 0, &self.readback.stats[ring], 0, STATS_WORDS * 4);
            if let (Some(q), Some(r)) = (&self.queries, &self.query_resolve) {
                // Resolve only the slots this frame wrote: resolving a query no
                // pass wrote is undefined on some native drivers (the NVIDIA
                // Vulkan driver faults). Unwritten slots read back as zero.
                let times = &self.readback.times[ring];
                encoder.clear_buffer(times, 0, TIMESTAMP_SLOTS as u64 * 16);
                let written = self.written_slots.get();
                for slot in (0..TIMESTAMP_SLOTS).filter(|s| written & 1 << s != 0) {
                    encoder.resolve_queries(q, slot * 2, 2, r, slot as u64 * RESOLVE_STRIDE);
                    encoder.copy_buffer_to_buffer(r, slot as u64 * RESOLVE_STRIDE, times, slot as u64 * 16, 16);
                }
            }
        }
        let commands = take_command_counts();
        self.gpu.submit(vec![encoder.finish()]);
        drop(surface);
        self.gpu.present();
        if read_now {
            let s = self.gpu.read_buffer(&self.readback.stats[ring], 0, STATS_WORDS * 4);
            let tm = self.gpu.read_buffer(&self.readback.times[ring], 0, TIMESTAMP_SLOTS as u64 * 16);
            self.readback.pending[ring] = Some((s, tm, self.stats.epoch));
        }
        self.collect_reads();
        self.prev_view_proj = vp_nj;
        self.taa_reset = false;
        self.frame += 1;
        let errors = self.gpu.shader_errors();
        let st = &mut self.stats;
        st.frame = self.frame;
        st.render_size = self.targets.size;
        st.output_size = self.targets.out;
        st.total_instances = self.world.total;
        st.cpu_frame_ms = now_ms() - start;
        st.cpu_cull_ms = cpu_cull_ms;
        st.main_encode_ms = split.total;
        st.main_replay_ms = split.replay;
        st.main_log_ms = split.log;
        st.encoder = cfg.encoder;
        st.batched_calls = af_gpu::batched_calls();
        st.commands = commands;
        st.config = cfg;
        st.errors.extend(errors);
        if st.errors.len() > 20 {
            let excess = st.errors.len() - 20;
            st.errors.drain(..excess);
        }
        &self.stats
    }

    fn collect_reads(&mut self) {
        let mut done = self.gpu.completed_reads();
        self.readback.partial.append(&mut done);
        for ring in 0..READ_RING {
            let Some((s, t, epoch)) = self.readback.pending[ring] else { continue };
            let si = self.readback.partial.iter().position(|(k, _)| *k == s);
            let ti = self.readback.partial.iter().position(|(k, _)| *k == t);
            let (Some(si), Some(ti)) = (si, ti) else { continue };
            let stats_bytes = self.readback.partial[si].1.clone();
            let time_bytes = self.readback.partial[ti].1.clone();
            self.readback.partial.retain(|(k, _)| *k != s && *k != t);
            self.readback.pending[ring] = None;
            let words: &[u32] = cast_slice(&stats_bytes);
            let st = &mut self.stats;
            if epoch != st.epoch {
                continue;
            }
            st.gpu_epoch = epoch;
            if self.config.path != Path::Cpu {
                st.visible_instances = words[0];
                st.triangles = words[1];
            }
            st.clusters = words[2];
            st.crowd_near = words[3];
            st.crowd_mid = words[4];
            st.crowd_far = words[5];
            st.occluded_instances = words[6];
            if self.queries.is_some() {
                let ts: Vec<u64> = time_bytes.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
                let pass = |slot: usize| {
                    let (b, e) = (ts[slot * 2], ts[slot * 2 + 1]);
                    if b == 0 || e <= b { 0.0 } else { (e - b) as f64 / 1.0e6 }
                };
                let written: Vec<u64> = ts.as_chunks::<2>().0.iter().filter(|p| p[0] != 0 && p[1] > p[0]).flatten().copied().collect();
                let total = match (written.iter().min(), written.iter().max()) {
                    (Some(lo), Some(hi)) => (hi - lo) as f64 / 1.0e6,
                    _ => 0.0,
                };
                st.gpu = PassTimes {
                    cull_1: pass(0),
                    expand_1: pass(1),
                    raster_1: pass(2),
                    hiz: pass(3),
                    cull_2: pass(4),
                    expand_2: pass(5),
                    raster_2: pass(6),
                    resolve: pass(7),
                    crowd_cull: pass(8),
                    crowd: pass(9),
                    taa: pass(10),
                    blit: pass(11),
                    total,
                };
            }
        }
    }

    fn cpu_cull(&mut self, planes: &[Vec4; 6], eye: Vec3, pixels_per_unit: f32) {
        let g = &self.assets.geometry;
        let buckets = g.mesh_count() * LODS * MATERIALS;
        self.scratch_counts.clear();
        self.scratch_counts.resize(buckets + 1, 0);
        self.scratch_keys.clear();
        for inst in &self.world.instances {
            let b = g.bounds[inst.mesh as usize];
            let c = Vec3::from(b.center);
            let s = Vec3::new(inst.scale, inst.scale * inst.height_scale, inst.scale);
            let q = c * s;
            let (sn, cs) = inst.yaw.sin_cos();
            let center = Vec3::new(cs * q.x - sn * q.z, q.y, sn * q.x + cs * q.z) + Vec3::from(inst.position);
            let radius = b.radius * inst.scale * inst.height_scale.max(1.0);
            if !camera::sphere_visible(planes, center, radius) {
                self.scratch_keys.push(u32::MAX);
                continue;
            }
            let lod = if self.config.lod {
                let px = radius / center.distance(eye).max(0.1) * pixels_per_unit;
                if px > 160.0 { 0 } else if px > 60.0 { 1 } else if px > 24.0 { 2 } else { 3 }
            } else {
                0
            };
            let key = ((inst.mesh as usize * LODS + lod) * MATERIALS + inst.material as usize) as u32;
            self.scratch_keys.push(key);
            self.scratch_counts[key as usize + 1] += 1;
        }
        for k in 1..self.scratch_counts.len() {
            self.scratch_counts[k] += self.scratch_counts[k - 1];
        }
        let visible = self.scratch_counts[buckets] as usize;
        self.scratch_list.clear();
        self.scratch_list.resize(visible.max(1), 0);
        self.cpu_draws.clear();
        let mut cursor = self.scratch_counts.clone();
        for (i, &key) in self.scratch_keys.iter().enumerate() {
            if key != u32::MAX {
                let slot = &mut cursor[key as usize];
                self.scratch_list[*slot as usize] = i as u32;
                *slot += 1;
            }
        }
        let mut triangles = 0u64;
        for key in 0..buckets {
            let (lo, hi) = (self.scratch_counts[key], self.scratch_counts[key + 1]);
            if hi == lo {
                continue;
            }
            let lod = g.lods[key / MATERIALS];
            triangles += (lod.index_count / 3) as u64 * (hi - lo) as u64;
            self.cpu_draws.push(CpuDraw {
                material: (key % MATERIALS) as u32,
                offset: lo,
                count: hi - lo,
                index_count: lod.index_count,
                first_index: lod.first_index,
                base_vertex: lod.base_vertex,
            });
        }
        let bytes: &[u8] = cast_slice(&self.scratch_list);
        self.gpu.write_buffer(&self.world.list_a, 0, bytes);
        self.stats.visible_instances = visible as u32;
        self.stats.triangles = triangles.min(u32::MAX as u64) as u32;
    }

    fn sample_index(&self) -> usize {
        (self.config.aa == Aa::Msaa4) as usize
    }

    fn main_pass<'a>(&'a self, encoder: &'a mut CommandEncoder, label: &'a str, clear: bool, slot: u32) -> RenderPass<'a> {
        let t = &self.targets;
        let msaa = self.config.aa == Aa::Msaa4;
        let load = if clear { LoadOp::Clear([0.0; 4]) } else { LoadOp::Load };
        let color = if msaa {
            ColorAttachment { view: t.hdr_msaa_view.as_ref().unwrap(), resolve_target: Some(&t.hdr_view), load, store: true }
        } else {
            ColorAttachment { view: &t.hdr_view, resolve_target: None, load, store: true }
        };
        let depth_view = if msaa { t.depth_msaa_view.as_ref().unwrap() } else { &t.depth_view };
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label,
            color: vec![color],
            depth: Some(DepthAttachment { view: depth_view, clear: clear.then_some(1.0), store: true }),
            timestamps: self.timestamps(slot),
        });
        if clear {
            let s = self.sample_index();
            pass.set_pipeline(&self.pipelines.raster[s].sky);
            pass.set_bind_group(0, &self.bindings.sky[s]);
            pass.draw(3, 1, 0, 0);
        }
        pass
    }

    // Handles are u32 on the web (the only host with an external encoder) and usize natively.
    #[allow(clippy::unnecessary_cast)]
    fn encode_cpu(&self, encoder: &mut CommandEncoder) -> EncodeSplit {
        let s = self.sample_index();
        let a = &self.assets;
        let mut pass = self.main_pass(encoder, "main", true, 2);
        pass.set_pipeline(&self.pipelines.raster[s].forward_cpu);
        pass.set_bind_group(0, &self.bindings.forward_cpu[s]);
        pass.set_vertex_buffer(0, &a.vertex, 0, a_size(&a.vertex));
        pass.set_index_buffer(&a.index, IndexFormat::Uint32, 0, a.geometry.indices.len() as u64 * 4);
        // The setup above is not part of the draw encode, whichever path makes the calls.
        pass.flush();
        let mut split = EncodeSplit::default();
        let start = now_ms();
        let external = (self.config.encoder == Encoder::Ts).then_some(self.external_encoder).flatten();
        let mut elapsed = None;
        if let Some(hook) = external {
            let mut log = Vec::with_capacity(self.cpu_draws.len() * 7 + 1);
            log.push(self.world.list_a.raw() as u32);
            for d in &self.cpu_draws {
                log.extend_from_slice(&[
                    self.bindings.materials[s][d.material as usize].raw() as u32,
                    d.offset * 4,
                    d.count * 4,
                    d.index_count,
                    d.count,
                    d.first_index,
                    d.base_vertex,
                ]);
            }
            split.log = now_ms() - start;
            elapsed = hook(pass_handle(&pass), &log);
        }
        if elapsed.is_none() {
            let mut last = u32::MAX;
            for d in &self.cpu_draws {
                if d.material != last {
                    pass.set_bind_group(1, &self.bindings.materials[s][d.material as usize]);
                    last = d.material;
                }
                pass.set_vertex_buffer(1, &self.world.list_a, d.offset as u64 * 4, d.count as u64 * 4);
                pass.draw_indexed(d.index_count, d.count, d.first_index, d.base_vertex as i32, 0);
            }
            // Batched web calls reach WebGPU here; count that in the encode.
            let replay = now_ms();
            pass.flush();
            split.replay = now_ms() - replay;
        }
        split.total = elapsed.unwrap_or(now_ms() - start);
        pass.end();
        split
    }

    fn encode_gpu_raster(&self, encoder: &mut CommandEncoder, list: &Buffer, args: &Buffer, clear: bool, slot: u32) {
        let s = self.sample_index();
        let a = &self.assets;
        let w = &self.world;
        let mut pass = self.main_pass(encoder, "gpu raster", clear, slot);
        pass.set_pipeline(&self.pipelines.raster[s].forward_gpu);
        pass.set_bind_group(0, &self.bindings.forward_gpu[s]);
        pass.set_vertex_buffer(0, &a.vertex, 0, a_size(&a.vertex));
        pass.set_index_buffer(&a.index, IndexFormat::Uint32, 0, a.geometry.indices.len() as u64 * 4);
        for (bucket, (&base, &cap)) in w.bucket_base.iter().zip(&w.bucket_cap).enumerate() {
            if cap == 0 {
                continue;
            }
            pass.set_vertex_buffer(1, list, base as u64 * 4, cap as u64 * 4);
            pass.draw_indexed_indirect(args, bucket as u64 * 20);
        }
        pass.end();
    }

    fn dispatch_instances(&self, pass: &mut ComputePass, count: u32) {
        let groups = count.div_ceil(64);
        pass.dispatch(groups.min(65535), groups.div_ceil(65535).max(1), 1);
    }

    fn encode_cull(&self, encoder: &mut CommandEncoder, entry: &str, slot: u32, count: u32) {
        let i = self.cull_index(entry);
        let mut pass = encoder.begin_compute_pass(entry, self.timestamps(slot));
        pass.set_pipeline(&self.pipelines.cull[i].2);
        pass.set_bind_group(0, &self.bindings.cull[i]);
        self.dispatch_instances(&mut pass, count);
        pass.end();
    }

    fn encode_hiz(&self, encoder: &mut CommandEncoder) {
        let t = &self.targets;
        let mut pass = encoder.begin_compute_pass("hiz", self.timestamps(3));
        let first = if self.config.aa == Aa::Msaa4 { &self.pipelines.hiz_first_msaa } else { &self.pipelines.hiz_first };
        pass.set_pipeline(first);
        pass.set_bind_group(0, self.bindings.hiz_first.as_ref().unwrap());
        let mut size = [t.size[0].div_ceil(2), t.size[1].div_ceil(2)];
        pass.dispatch(size[0].div_ceil(8), size[1].div_ceil(8), 1);
        pass.set_pipeline(&self.pipelines.hiz_down);
        for group in &self.bindings.hiz_down {
            size = [(size[0] / 2).max(1), (size[1] / 2).max(1)];
            pass.set_bind_group(0, group);
            pass.dispatch(size[0].div_ceil(8), size[1].div_ceil(8), 1);
        }
        pass.end();
    }

    fn encode_gpu(&self, encoder: &mut CommandEncoder) {
        let w = &self.world;
        if self.config.occlusion {
            self.encode_cull(encoder, "phase1_buckets", 0, w.total);
            self.encode_gpu_raster(encoder, &w.list_a, &w.args_a, true, 2);
            self.encode_hiz(encoder);
            self.encode_cull(encoder, "phase2_buckets", 4, w.total);
            self.encode_gpu_raster(encoder, &w.list_b, &w.args_b, false, 6);
        } else {
            self.encode_cull(encoder, "single_buckets", 0, w.total);
            self.encode_gpu_raster(encoder, &w.list_a, &w.args_a, true, 2);
        }
    }

    fn encode_expand(&self, encoder: &mut CommandEncoder, k: usize, slot: u32) {
        let mut pass = encoder.begin_compute_pass("expand", self.timestamps(slot));
        pass.set_pipeline(&self.pipelines.prepare_dispatch);
        pass.set_bind_group(0, &self.bindings.prepare[k]);
        pass.dispatch(1, 1, 1);
        pass.end();
        let mut pass = encoder.begin_compute_pass("expand", None);
        pass.set_pipeline(&self.pipelines.expand);
        pass.set_bind_group(0, &self.bindings.expand[k]);
        pass.dispatch_indirect(&self.world.dispatch_args, 0);
        pass.end();
    }

    fn encode_raster_ids(&self, encoder: &mut CommandEncoder, k: usize, clear: bool, slot: u32) {
        let t = &self.targets;
        let load = if clear { LoadOp::Clear([0.0; 4]) } else { LoadOp::Load };
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: "raster ids",
            color: vec![ColorAttachment { view: &t.vis_view, resolve_target: None, load, store: true }],
            depth: Some(DepthAttachment { view: &t.depth_view, clear: clear.then_some(1.0), store: true }),
            timestamps: self.timestamps(slot),
        });
        pass.set_pipeline(&self.pipelines.raster_ids);
        pass.set_bind_group(0, &self.bindings.raster_ids[k]);
        let args = if k == 0 { &self.world.draw_args_a } else { &self.world.draw_args_b };
        pass.draw_indirect(args, 0);
        pass.end();
    }

    fn encode_visbuffer(&self, encoder: &mut CommandEncoder) {
        let total = self.world.total;
        if self.config.occlusion {
            self.encode_cull(encoder, "phase1_list", 0, total);
            self.encode_expand(encoder, 0, 1);
            self.encode_raster_ids(encoder, 0, true, 2);
            self.encode_hiz(encoder);
            self.encode_cull(encoder, "phase2_list", 4, total);
            self.encode_expand(encoder, 1, 5);
            self.encode_raster_ids(encoder, 1, false, 6);
        } else {
            self.encode_cull(encoder, "single_list", 0, total);
            self.encode_expand(encoder, 0, 1);
            self.encode_raster_ids(encoder, 0, true, 2);
        }
        let [w, h] = self.targets.size;
        let mut pass = encoder.begin_compute_pass("resolve", self.timestamps(7));
        pass.set_pipeline(&self.pipelines.resolve);
        pass.set_bind_group(0, self.bindings.resolve.as_ref().unwrap());
        pass.dispatch(w.div_ceil(8), h.div_ceil(8), 1);
        pass.end();
    }

    fn encode_crowd(&self, encoder: &mut CommandEncoder) {
        let w = &self.world;
        let a = &self.assets;
        let s = self.sample_index();
        {
            let mut pass = encoder.begin_compute_pass("crowd cull", self.timestamps(8));
            pass.set_pipeline(&self.pipelines.crowd_cull);
            pass.set_bind_group(0, self.bindings.crowd_cull.as_ref().unwrap());
            self.dispatch_instances(&mut pass, self.config.crowd);
            pass.end();
        }
        let set = &self.pipelines.raster[s];
        let mut pass = self.main_pass(encoder, "crowd", false, 9);
        pass.set_pipeline(&set.near);
        pass.set_bind_group(0, &self.bindings.near[s]);
        pass.set_vertex_buffer(0, &a.char_vertex, 0, a_size(&a.char_vertex));
        pass.set_vertex_buffer(1, &a.char_bones, 0, a_size(&a.char_bones));
        pass.set_vertex_buffer(2, &w.crowd_near, 0, a_size(&w.crowd_near));
        pass.set_index_buffer(&a.char_index, IndexFormat::Uint32, 0, a.char_index_count as u64 * 4);
        pass.draw_indexed_indirect(&w.crowd_args, 0);
        if self.config.crowd_tiers {
            pass.set_pipeline(&set.mid);
            pass.set_bind_group(0, &self.bindings.mid[s]);
            pass.set_vertex_buffer(0, &w.crowd_mid, 0, a_size(&w.crowd_mid));
            pass.set_index_buffer(&a.low_index, IndexFormat::Uint32, 0, a.low_index_count as u64 * 4);
            pass.draw_indexed_indirect(&w.crowd_args, 20);
            pass.set_pipeline(&set.far);
            pass.set_bind_group(0, &self.bindings.far[s]);
            pass.set_vertex_buffer(0, &w.crowd_far, 0, a_size(&w.crowd_far));
            pass.draw_indirect(&w.crowd_args, 40);
        }
        pass.end();
    }
}

/// Whether the Slang-compiled culling shader has been generated.
pub fn slang_available() -> bool {
    !CULL_SLANG.trim().is_empty()
}

/// Binds the whole buffer: `u64::MAX` is `WGPU_WHOLE_SIZE` in `webgpu.h`,
/// and the web backend omits the size.
fn a_size(_: &Buffer) -> u64 {
    u64::MAX
}

fn pass_handle(pass: &RenderPass) -> Handle {
    pass.raw()
}
