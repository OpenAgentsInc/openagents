//! GPU resources and frame encoding for the physical path.
//!
//! One frame runs these passes: the sun's shadow maps; a floating-point scene pass
//! that draws the sky at infinity, catalogue stars, the Sun, Earth, and Moon in
//! distance order, lit surfaces, opaque and masked textured meshes, legacy
//! geometry, blended textured meshes, guide lines, and glows; a
//! bloom mip chain; exposure adaptation; the output transform into the
//! surface; and the HUD on top. When the adapter cannot render a floating-point
//! target, the scene pass tone-maps in place and post-processing is skipped.
//! The bloom, adaptation, and graded output passes live in [`super::output`],
//! which the summoning chamber shares.
//!
//! [`Capability`] also fixes the quality tier
//! ([`verse_engine::quality::Tier`]) from the adapter and the platform; the
//! tier selects the sun shadow filter and material detail through pipeline
//! constants, and the number of sun shadow cascades.
//!
//! On the high tier, [`verse_engine::render_graph::PhotoPlan`] adds three
//! passes between the shadow maps and the scene: a single-sample depth
//! prepass of the opaque and masked geometry, and [`super::screen`]'s
//! ambient occlusion and contact shadow trace and resolve. Lit and textured
//! surfaces read the result, through the `SCREEN` pipeline constant; other
//! tiers read a white texel and draw exactly as before.
//!
//! The sun's shadow is a 2D depth array, one layer per cascade, which WebGL2
//! supports. A stage key with a shadow distance gets cascades that follow the
//! camera ([`verse_engine::lighting::fit_cascades`]); every other shadow is
//! one map over a fixed region. Cascades after the first hold static casters
//! only and are redrawn only when their snapped matrix or the static scene
//! changes.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::environment::{SkyInputs, SkyLightGpu};
use super::output::{self, Look, Output, OutputTargets};
use super::screen::{self as screen_space, ScreenGpu, ScreenTargets, ScreenUniform};
use super::textured::{self, Pass, TexturedMaterial, TexturedScene, TexturedVertex};
use super::{GlowVertex, LitVertex, Neon, ProbeGrid, Sky, sky};
use verse_engine::lighting::{
    CascadeSettings, Cascades, Frustum, Grade, MAX_CASCADES, fit_box, fit_cascades,
};
use verse_engine::quality::{Platform, Probe, Quality, ShadowFilter, Tier};
use verse_engine::render_graph::{PhotoPass, PhotoPlan};

pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Frame {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    sun_disc: [f32; 4],
    earth: [f32; 4],
    earth_x: [f32; 4],
    earth_y: [f32; 4],
    earth_z: [f32; 4],
    moon: [f32; 4],
    moon_x: [f32; 4],
    moon_y: [f32; 4],
    moon_z: [f32; 4],
    celestial_x: [f32; 4],
    celestial_y: [f32; 4],
    celestial_z: [f32; 4],
    earth_light: [f32; 4],
    viewport: [f32; 4],
    probe_origin: [f32; 4],
    probe_dims: [f32; 4],
    params: [f32; 4],
    metering: [f32; 4],
    neon: [f32; 4],
    field: [f32; 4],
    /// The neon stage's daylight sky: zenith (w 1 when drawn), horizon (w
    /// cloud cover), and Sun tint (w disc angular radius).
    sky_zenith: [f32; 4],
    sky_horizon: [f32; 4],
    sky_sun: [f32; 4],
    /// The daylight sky's light: x 1 when lit surfaces take it, y the
    /// reflection cube's last level; and its irradiance as order-two
    /// spherical harmonics ([`super::environment`]).
    sky_light: [f32; 4],
    sky_sh: [[f32; 4]; 9],
    /// Height fog ([`super::HeightFog::uniform`]); `fog_lobe` w 1 when present.
    fog_shape: [f32; 4],
    fog_lobe: [f32; 4],
    /// The sun's shadow cascades, near to far: world to map clip space.
    cascades: [[[f32; 4]; 4]; MAX_CASCADES],
    /// Per cascade: texel edge, depth range along the light, and the view
    /// depth where it hands over to the next, m.
    cascade_texel: [f32; 4],
    cascade_depth: [f32; 4],
    cascade_end: [f32; 4],
    /// Count, blend band, fade start, and shadow distance.
    cascade_params: [f32; 4],
    /// The camera's view axis, along which view depth is measured.
    view_forward: [f32; 4],
    /// x lamp count; y the scale from emitted luminance to the shaded
    /// signal (the key's exposure on a neon stage, 1 in space).
    lamp_params: [f32; 4],
    /// Per lamp: position and range (m), then pre-exposed color times
    /// candela.
    lamps: [[f32; 4]; 2 * super::MAX_LAMPS],
    /// A neon stage's key light color (rgb), with w 1 when set; white
    /// otherwise.
    key_tint: [f32; 4],
}

impl Frame {
    /// Writes the sun's shadow maps into the uniform. The `light` matrix
    /// stays the first cascade's; each shadow pass gets its own copy.
    fn set_cascades(&mut self, cascades: &Cascades) {
        let mut texel = [0.0; 4];
        let mut depth = [0.0; 4];
        let mut end = [0.0; 4];
        for (i, cascade) in cascades.cascades.iter().take(MAX_CASCADES).enumerate() {
            self.cascades[i] = cascade.matrix.to_cols_array_2d();
            texel[i] = cascade.texel;
            depth[i] = cascade.depth_range;
            end[i] = cascade.end;
        }
        if let Some(first) = cascades.cascades.first() {
            self.light = first.matrix.to_cols_array_2d();
        }
        self.cascade_texel = texel;
        self.cascade_depth = depth;
        self.cascade_end = end;
        self.cascade_params = [
            cascades.cascades.len().min(MAX_CASCADES) as f32,
            cascades.blend,
            cascades.fade_start,
            cascades.distance,
        ];
        self.view_forward = cascades.forward.extend(0.0).to_array();
    }
}

/// What a cached cascade's map was drawn from: its matrix and the static
/// geometry's identity.
type CascadeKey = ([f32; 16], u64);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StarInstance {
    dir: [f32; 3],
    illuminance: f32,
    color: [f32; 3],
}

/// A growable vertex buffer.
pub struct Stream {
    pub buffer: wgpu::Buffer,
    pub count: u32,
    capacity: u64,
    label: &'static str,
}

impl Stream {
    pub fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 64 * 1024,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            count: 0,
            capacity: 64 * 1024,
            label,
        }
    }

    pub fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, items: &[T]) {
        let bytes: &[u8] = bytemuck::cast_slice(items);
        if bytes.len() as u64 > self.capacity {
            let capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.capacity = capacity;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
        self.count = items.len() as u32;
    }
}

/// How the adapter can run the physical path.
#[derive(Clone, Copy, Debug)]
pub struct Capability {
    /// The floating-point scene format, when one is renderable and filterable.
    pub hdr: Option<wgpu::TextureFormat>,
    pub samples: u32,
    /// Whether shaders compile to GLSL ES, which takes the `GLES` variants.
    pub gles: bool,
    /// The quality tier and everything it fixes. `VERSE_QUALITY` (`low`,
    /// `medium`, or `high`) lowers it; it never raises it past the device.
    pub quality: Quality,
}

impl Capability {
    /// Reserve both ordinary and physical targets before native allocation.
    /// Includes cascade maps and a conservative screen-space allowance; excludes swapchain images.
    pub fn target_reservation(&self, width: u32, height: u32, output_bytes: u64) -> u64 {
        let pixels = u64::from(width) * u64::from(height);
        let samples = u64::from(self.samples);
        let scene_bytes = self.hdr.map_or(output_bytes, |format| {
            u64::from(format.block_copy_size(None).unwrap_or(8))
        });
        let ordinary = pixels
            * (4 * samples
                + if samples > 1 {
                    output_bytes * samples
                } else {
                    0
                });
        let physical = pixels
            * (scene_bytes
                + 4 * samples
                + if samples > 1 {
                    scene_bytes * samples
                } else {
                    0
                });
        let post = if self.hdr.is_some() {
            (pixels * scene_bytes).div_ceil(3)
                + 2 * scene_bytes
                + (verse_engine::lighting::GRADE_LUT_SIZE as u64).pow(3) * 8
        } else {
            0
        };
        let screen = if self.quality.screen_space {
            pixels * 24
        } else {
            0
        };
        ordinary
            + physical
            + post
            + screen
            + u64::from(SHADOW_SIZE).pow(2) * u64::from(self.quality.cascades) * 4
    }
    pub fn probe(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        output: wgpu::TextureFormat,
        requested: u32,
    ) -> Self {
        let usable = |format: wgpu::TextureFormat| {
            let f = adapter.get_texture_format_features(format);
            f.allowed_usages.contains(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ) && f
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
                && f.flags.contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
        };
        let rg11 = wgpu::TextureFormat::Rg11b10Ufloat;
        let compact = std::env::var_os("VERSE_PHOTO_RGBA16").is_none();
        let hdr = if compact
            && device
                .features()
                .contains(wgpu::Features::RG11B10UFLOAT_RENDERABLE)
            && usable(rg11)
        {
            Some(rg11)
        } else if usable(wgpu::TextureFormat::Rgba16Float) {
            Some(wgpu::TextureFormat::Rgba16Float)
        } else {
            None
        };
        let scene = hdr.unwrap_or(output);
        let samples = if requested == 4
            && adapter
                .get_texture_format_features(scene)
                .flags
                .sample_count_supported(4)
            && adapter
                .get_texture_format_features(output)
                .flags
                .sample_count_supported(4)
            && adapter
                .get_texture_format_features(DEPTH)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };
        let gles = verse_gfx::gles::is_gles(adapter.get_info().backend);
        let probe = Probe {
            platform: Platform::current(),
            gles,
            float_target: hdr.is_some(),
            compute: adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
                && device.limits().max_storage_buffers_per_shader_stage > 0,
            samples,
        };
        let asked = std::env::var("VERSE_QUALITY")
            .ok()
            .and_then(|name| Tier::parse(&name));
        let quality = probe.select(asked).quality();
        Self {
            hdr,
            samples: samples.min(quality.sample_ceiling()),
            gles,
            quality,
        }
    }

    /// The pipeline constants the tier sets in `photo.wgsl`. `SCREEN` reads
    /// the screen-space terms, which only a tier with screen-space passes
    /// draws.
    fn tier_constants(&self) -> [(&'static str, f64); 3] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        [
            (
                "PCSS",
                flag(self.quality.shadow_filter == ShadowFilter::Soft),
            ),
            ("DETAIL", flag(self.quality.materials.detail_normals)),
            ("SCREEN", flag(self.quality.screen_space)),
        ]
    }
}

struct Pipelines {
    shadow: wgpu::RenderPipeline,
    lit: wgpu::RenderPipeline,
    background: wgpu::RenderPipeline,
    /// The neon stage's daylight sky, a full-screen triangle.
    daylight: wgpu::RenderPipeline,
    stars: wgpu::RenderPipeline,
    bodies: wgpu::RenderPipeline,
    flare: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    /// Particle sprites from every fx sheet, premultiplied (`crate::fx`).
    sprites: wgpu::RenderPipeline,
    legacy: wgpu::RenderPipeline,
    wide: wgpu::RenderPipeline,
    /// Textured meshes by [`Pass`], single-sided then double-sided.
    textured: [[wgpu::RenderPipeline; 2]; 3],
    /// Opaque textured meshes into the shadow map.
    textured_shadow: wgpu::RenderPipeline,
    /// Masked textured meshes into the shadow map, testing alpha.
    textured_shadow_masked: wgpu::RenderPipeline,
}

/// How many of a stage's lamps a tier shades, ranked by view contribution: each lamp
/// costs every lit fragment a loop iteration, so phones and WebGL2 shade
/// the stage's most important few and leave the rest out.
#[must_use]
pub fn lamp_budget(tier: Tier) -> usize {
    match tier {
        Tier::Low => 8,
        Tier::Medium => 16,
        Tier::High => super::MAX_LAMPS,
    }
}

/// The high tier's depth prepass: the shadow casters' shaders drawn with the
/// camera's matrix into a single-sample, reversed-depth buffer.
struct Prepass {
    lit: wgpu::RenderPipeline,
    textured: wgpu::RenderPipeline,
    masked: wgpu::RenderPipeline,
}

/// Textured static meshes on the GPU: merged vertices and indices uploaded
/// once, and one bind group per material.
pub struct TexturedGpu {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    batches: Vec<textured::Batch>,
    materials: Vec<TexturedMaterial>,
    groups: Vec<wgpu::BindGroup>,
    /// The scene's index edits applied so far
    /// ([`textured::IndexEdits::revision`]); part of the static casters'
    /// identity, so a cached shadow redraws after an edit.
    pub edits: u64,
    /// Whether each cell counted as near last frame ([`textured::Level`]).
    near: Vec<bool>,
    /// Whether `near` has been set from an eye yet.
    placed: bool,
    /// How many times a cell has changed level; part of the static casters'
    /// identity, so a cached shadow redraws with the cells' new levels.
    pub levels: u64,
}

impl TexturedGpu {
    /// Sets each cell's level of detail for a frame seen from `eye`. A cell
    /// keeps its level until the eye crosses [`textured::HYSTERESIS`] past
    /// its switch distance.
    pub fn update_levels(&mut self, eye: Vec3) {
        let placed = self.placed;
        let mut changed = false;
        for (batch, near) in self.batches.iter().zip(&mut self.near) {
            let now = batch.level.near(eye, placed.then_some(*near));
            changed |= now != *near;
            *near = now;
        }
        if changed || !placed {
            self.levels += 1;
        }
        self.placed = true;
    }

    /// Whether cell `i` draws at its current level.
    fn shown(&self, i: usize) -> bool {
        let batch = &self.batches[i];
        batch.level.drawn(self.near.get(i).copied().unwrap_or(true))
    }

    /// Rewrites the merged indices from `first` on, within the buffer.
    pub fn write_indices(&self, queue: &wgpu::Queue, first: u32, indices: &[u32]) {
        let bytes: &[u8] = bytemuck::cast_slice(indices);
        let offset = u64::from(first) * 4;
        if !bytes.is_empty() && offset + bytes.len() as u64 <= self.indices.size() {
            queue.write_buffer(&self.indices, offset, bytes);
        }
    }

    /// Rewrites a figure's vertices; the caller has checked their count
    /// against the uploaded mesh ([`textured::Figure::validate`]).
    pub fn write_vertices(&self, queue: &wgpu::Queue, vertices: &[TexturedVertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if !bytes.is_empty() && bytes.len() as u64 <= self.vertices.size() {
            queue.write_buffer(&self.vertices, 0, bytes);
        }
    }
}

/// Size-dependent targets for the physical path.
pub struct PhotoTargets {
    size: [u32; 2],
    msaa: Option<wgpu::TextureView>,
    scene: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// Bloom and adaptation, when the scene renders to a float target.
    output: Option<OutputTargets>,
    /// The adapted luminance for guides, indexed by the texture last
    /// written, and the screen-space terms lit surfaces read.
    guide_groups: [wgpu::BindGroup; 2],
    /// The high tier's prepass depth and screen-space terms.
    screen: Option<ScreenTargets>,
}

impl PhotoTargets {
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// The adapt texture the next frame writes.
    fn parity(&self) -> usize {
        self.output.as_ref().map_or(0, |output| output.parity)
    }
}

/// GPU state for the physical path, created on the first physical frame.
pub struct Photo {
    pub last_lighting: verse_engine::lighting::FrameLighting,
    capability: Capability,
    output_format: wgpu::TextureFormat,
    frame: wgpu::Buffer,
    /// Whether the Sun, Earth, Moon, and star data are uploaded.
    space_ready: bool,
    guide_layout: wgpu::BindGroupLayout,
    scene_layout: wgpu::BindGroupLayout,
    scene_group: wgpu::BindGroup,
    /// One copy of the frame uniform per cascade, at a dynamic offset, for
    /// the shadow passes that write the maps: each copy's `light` is its
    /// cascade's matrix.
    frame_group: wgpu::BindGroup,
    shadow_frames: wgpu::Buffer,
    /// Bytes between the copies in `shadow_frames`.
    shadow_stride: u64,
    /// Every cascade's layer, for sampling.
    shadow: wgpu::TextureView,
    /// One layer each, for the passes that draw them.
    shadow_layers: Vec<wgpu::TextureView>,
    /// What each cached layer holds; `None` when it holds no cached map.
    shadow_keys: Vec<Option<CascadeKey>>,
    shadow_compare: wgpu::Sampler,
    linear_clamp: wgpu::Sampler,
    linear_repeat: wgpu::Sampler,
    probes: [wgpu::TextureView; 3],
    probe_version: Option<u64>,
    sky_textures: [wgpu::TextureView; 5],
    /// The daylight sky's diffuse and glossy light, rebuilt when the sky or
    /// the key light changes.
    sky_light: SkyLightGpu,
    pipelines: Pipelines,
    /// A textured material's image, sampler, and factors (group 2).
    material_layout: wgpu::BindGroupLayout,
    /// Cooked and uploaded base-color images by name, size, a sample of
    /// their texels, and role: the world scene's and the figures' drawn
    /// since, so a figure whose scene changes (a town's chunks growing
    /// their buffers) reuses the images already uploaded instead of cooking
    /// their levels again. A new world scene starts it over, so leaving a
    /// zone frees its images.
    cooked: std::sync::Mutex<std::collections::BTreeMap<TextureKey, wgpu::TextureView>>,
    /// Fills group 1 for the masked shadow pipeline, which reads no guides.
    empty_group: wgpu::BindGroup,
    /// Repeating, trilinear sampling for base-color images.
    textured_sampler: wgpu::Sampler,
    post: Option<Output>,
    /// The high tier's depth prepass and screen-space passes.
    prepass: Option<Prepass>,
    screen: Option<ScreenGpu>,
    /// The screen-space terms on tiers that do not trace them: one white
    /// texel, so lit surfaces multiply their light by exactly one.
    screen_white: wgpu::TextureView,
    stars: wgpu::Buffer,
    star_count: u32,
    pub dynamic_lit: Stream,
    pub glow: Stream,
    /// This frame's particle sprite quads (`crate::fx::vertices`).
    pub sprites: Stream,
    fx_group: wgpu::BindGroup,
    /// The display's headroom over reference white for space frames.
    pub headroom: f32,
}

fn shader(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn lit_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x3, 5 => Float32x4
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LitVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}

fn textured_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Unorm8x4, 4 => Unorm8x4
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TexturedVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

const PREMULTIPLIED: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};

const ADDITIVE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

impl Photo {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        capability: Capability,
        output_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let direct = capability.hdr.is_none();
        let scene_format = capability.hdr.unwrap_or(output_format);
        let samples = capability.samples;
        let texture_entry = |binding, dimension, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: dimension,
                multisampled: false,
            },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let d2 = wgpu::TextureViewDimension::D2;
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse photo scene"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(
                    1,
                    wgpu::TextureViewDimension::D2Array,
                    wgpu::TextureSampleType::Depth,
                ),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                texture_entry(3, wgpu::TextureViewDimension::D3, float),
                texture_entry(4, wgpu::TextureViewDimension::D3, float),
                texture_entry(5, wgpu::TextureViewDimension::D3, float),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(7, d2, float),
                texture_entry(8, d2, float),
                texture_entry(9, d2, float),
                texture_entry(10, d2, float),
                texture_entry(11, d2, float),
                wgpu::BindGroupLayoutEntry {
                    binding: 12,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(13, wgpu::TextureViewDimension::Cube, float),
            ],
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse photo frame"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // At least two layers: the GL backend treats a one-layer texture as
        // a plain 2D texture, which cannot be viewed as an array.
        let layers = (capability.quality.cascades as usize).clamp(2, MAX_CASCADES) as u32;
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verse sun shadow cascades"),
            size: wgpu::Extent3d {
                width: SHADOW_SIZE,
                height: SHADOW_SIZE,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("verse sun shadow cascades"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(layers),
            ..Default::default()
        });
        let shadow_layers: Vec<wgpu::TextureView> = (0..layers)
            .map(|layer| {
                shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("verse sun shadow cascade"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let shadow_compare = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse shadow compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let linear = |mode, label| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: mode,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            })
        };
        let linear_clamp = linear(wgpu::AddressMode::ClampToEdge, "verse linear clamp");
        let linear_repeat = linear(wgpu::AddressMode::Repeat, "verse linear repeat");
        let probes = empty_probes(device, queue);
        // The Sun, Earth, Moon, and stars load on the first space frame; the
        // neon stage never needs them.
        let sky_textures = placeholder_sky(device, queue);
        let sky_light = SkyLightGpu::empty(device, queue);
        let scene_group = scene_group(
            device,
            &scene_layout,
            &frame,
            &shadow,
            &shadow_compare,
            &probes,
            &linear_clamp,
            &sky_textures,
            &linear_repeat,
            &sky_light.view,
        );

        let frame_size = std::mem::size_of::<Frame>() as u64;
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(1);
        let shadow_stride = frame_size.div_ceil(alignment) * alignment;
        // One copy per cascade, and one after them for the depth prepass.
        let shadow_frames = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse shadow frames"),
            size: shadow_stride * (MAX_CASCADES as u64 + 1),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse photo shadow frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(frame_size),
                },
                count: None,
            }],
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse photo shadow frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_frames,
                    offset: 0,
                    size: std::num::NonZeroU64::new(frame_size),
                }),
            }],
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse photo shadow"),
            bind_group_layouts: &[Some(&frame_layout)],
            immediate_size: 0,
        });
        let guide_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse photo guides"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // The screen-space terms (`screen_terms` in `photo.wgsl`).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let guide_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("verse photo guides"),
                bind_group_layouts: &[Some(&scene_layout), Some(&guide_layout)],
                immediate_size: 0,
            });
        let module = shader(
            device,
            "verse photo",
            &verse_gfx::gles::wgsl(
                &crate::shading::source(include_str!("photo.wgsl")),
                capability.gles,
            ),
        );
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse photo"),
            bind_group_layouts: &[Some(&scene_layout)],
            immediate_size: 0,
        });
        let debug = std::env::var("VERSE_PHOTO_DEBUG")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let [pcss, detail, screen_constant] = capability.tier_constants();
        let constants = [
            ("DIRECT", if direct { 1.0 } else { 0.0 }),
            ("DEBUG", f64::from(debug)),
            pcss,
            detail,
            screen_constant,
        ];
        let options = wgpu::PipelineCompilationOptions {
            constants: &constants,
            ..Default::default()
        };
        let color = |blend: Option<wgpu::BlendState>| {
            [Some(wgpu::ColorTargetState {
                format: scene_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })]
        };
        #[allow(clippy::too_many_arguments)]
        let make = |layout: &wgpu::PipelineLayout,
                    label: &str,
                    vs: &str,
                    fs: Option<&str>,
                    buffers: &[wgpu::VertexBufferLayout<'_>],
                    topology: wgpu::PrimitiveTopology,
                    depth: wgpu::DepthStencilState,
                    blend: Option<wgpu::BlendState>,
                    count: u32| {
            let targets = color(blend);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: options.clone(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: wgpu::MultisampleState {
                    count,
                    ..Default::default()
                },
                fragment: fs.map(|fs| wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: options.clone(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let triangles = wgpu::PrimitiveTopology::TriangleList;
        let sky_depth = depth_state(false, wgpu::CompareFunction::Always);
        let mut shadow_depth = depth_state(true, wgpu::CompareFunction::LessEqual);
        shadow_depth.bias = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 2.5,
            clamp: 0.0,
        };
        const STAR: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3];
        const GLOW: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        const LEGACY: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32];
        const WIDE: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
        let legacy_size = std::mem::size_of::<crate::mesh::Vertex>() as u64;
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse textured material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: float,
                        view_dimension: d2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse empty"),
            entries: &[],
        });
        let empty_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse empty"),
            layout: &empty_layout,
            entries: &[],
        });
        // Group 1 holds the guides' adapted luminance, which the pass binds
        // for the legacy faces anyway; textured shaders do not read it.
        let textured_layout_groups =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("verse photo textured"),
                bind_group_layouts: &[
                    Some(&scene_layout),
                    Some(&guide_layout),
                    Some(&material_layout),
                ],
                immediate_size: 0,
            });
        let masked_shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse textured shadow"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&empty_layout),
                Some(&material_layout),
            ],
            immediate_size: 0,
        });
        let textured_pipeline = |pass: Pass, double_sided: bool| {
            let raster = textured::raster(pass, double_sided);
            let targets = color(raster.blend.then_some(PREMULTIPLIED));
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("verse photo textured"),
                layout: Some(&textured_layout_groups),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_textured"),
                    compilation_options: options.clone(),
                    buffers: &[textured_layout()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: triangles,
                    cull_mode: raster.cull,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(
                    raster.depth_write,
                    wgpu::CompareFunction::GreaterEqual,
                )),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(raster.entry),
                    compilation_options: options.clone(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Both faces of every opaque or masked cell cast shadows.
        let textured_shadow = |layout: &wgpu::PipelineLayout, fs: Option<&str>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("verse textured shadow"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_shadow_textured"),
                    compilation_options: options.clone(),
                    buffers: &[textured_layout()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: triangles,
                    ..Default::default()
                },
                depth_stencil: Some(shadow_depth.clone()),
                multisample: wgpu::MultisampleState::default(),
                fragment: fs.map(|fs| wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: options.clone(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let textured_pipelines = Pass::ALL
            .map(|pass| [false, true].map(|double_sided| textured_pipeline(pass, double_sided)));
        let textured_shadow_opaque = textured_shadow(&shadow_layout, None);
        let textured_shadow_masked =
            textured_shadow(&masked_shadow_layout, Some("fs_shadow_masked"));
        // The high tier's prepass draws the casters' shaders with the
        // camera's reversed-depth matrix in the frame copy's `light`, single
        // sampled and without the shadow bias.
        let plan = PhotoPlan::build(&capability.quality)?;
        let prepass = plan.runs(PhotoPass::DepthPrepass).then(|| {
            let pipeline = |layout: &wgpu::PipelineLayout,
                            vs: &str,
                            buffers: &[wgpu::VertexBufferLayout<'_>],
                            fs: Option<&str>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("verse depth prepass"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some(vs),
                        compilation_options: options.clone(),
                        buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: triangles,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_state(true, wgpu::CompareFunction::GreaterEqual)),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: fs.map(|fs| wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(fs),
                        compilation_options: options.clone(),
                        targets: &[],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
            Prepass {
                lit: pipeline(&shadow_layout, "vs_shadow", &[lit_layout()], None),
                textured: pipeline(
                    &shadow_layout,
                    "vs_shadow_textured",
                    &[textured_layout()],
                    None,
                ),
                masked: pipeline(
                    &masked_shadow_layout,
                    "vs_shadow_textured",
                    &[textured_layout()],
                    Some("fs_shadow_masked"),
                ),
            }
        });
        let screen = plan
            .runs(PhotoPass::ScreenTrace)
            .then(|| ScreenGpu::new(device));
        // Every fx sheet as one layer of a texture array (group 3), so all
        // particles draw in one pipeline. Group 2 is the textured
        // material's slot in this shader module, so it stays empty here.
        let fx_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse fx sheets"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sprite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse photo sprites"),
            bind_group_layouts: &[
                Some(&scene_layout),
                Some(&guide_layout),
                Some(&empty_layout),
                Some(&fx_layout),
            ],
            immediate_size: 0,
        });
        // The low tier keeps the sheets at half size.
        let skip = u32::from(capability.quality.tier == verse_engine::quality::Tier::Low);
        let fx_view = upload_fx_sheets(device, queue, skip)?;
        let fx_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse fx sheets"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let fx_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse fx sheets"),
            layout: &fx_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fx_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&fx_sampler),
                },
            ],
        });
        const SPRITE: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x4, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4
        ];
        let pipelines = Pipelines {
            textured: textured_pipelines,
            textured_shadow: textured_shadow_opaque,
            textured_shadow_masked,
            shadow: make(
                &shadow_layout,
                "verse photo shadow",
                "vs_shadow",
                None,
                &[lit_layout()],
                triangles,
                shadow_depth,
                None,
                1,
            ),
            // Group 1 carries the screen-space terms lit surfaces read.
            lit: make(
                &guide_pipeline_layout,
                "verse photo lit",
                "vs_lit",
                Some("fs_lit"),
                &[lit_layout()],
                triangles,
                depth_state(true, wgpu::CompareFunction::GreaterEqual),
                None,
                samples,
            ),
            background: make(
                &layout,
                "verse photo sky",
                "vs_fullscreen",
                Some("fs_background"),
                &[],
                triangles,
                sky_depth.clone(),
                None,
                samples,
            ),
            daylight: make(
                &layout,
                "verse neon daylight",
                "vs_fullscreen",
                Some("fs_daylight"),
                &[],
                triangles,
                sky_depth.clone(),
                None,
                samples,
            ),
            stars: make(
                &layout,
                "verse photo stars",
                "vs_star",
                Some("fs_star"),
                &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<StarInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &STAR,
                }],
                triangles,
                sky_depth.clone(),
                Some(ADDITIVE),
                samples,
            ),
            bodies: make(
                &layout,
                "verse photo bodies",
                "vs_body",
                Some("fs_body"),
                &[],
                triangles,
                sky_depth.clone(),
                Some(PREMULTIPLIED),
                samples,
            ),
            flare: make(
                &layout,
                "verse photo flare",
                "vs_flare",
                Some("fs_flare"),
                &[],
                triangles,
                sky_depth,
                Some(ADDITIVE),
                samples,
            ),
            glow: make(
                &layout,
                "verse photo glow",
                "vs_glow",
                Some("fs_glow"),
                &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlowVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GLOW,
                }],
                triangles,
                depth_state(false, wgpu::CompareFunction::GreaterEqual),
                Some(ADDITIVE),
                samples,
            ),
            sprites: make(
                &sprite_layout,
                "verse photo sprites",
                "vs_sprite",
                Some("fs_sprite"),
                &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::fx::SpriteVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SPRITE,
                }],
                triangles,
                depth_state(false, wgpu::CompareFunction::GreaterEqual),
                Some(PREMULTIPLIED),
                samples,
            ),
            legacy: make(
                &guide_pipeline_layout,
                "verse photo legacy faces",
                "vs_legacy",
                Some("fs_legacy"),
                &[wgpu::VertexBufferLayout {
                    array_stride: legacy_size,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &LEGACY,
                }],
                triangles,
                depth_state(true, wgpu::CompareFunction::GreaterEqual),
                None,
                samples,
            ),
            wide: make(
                &guide_pipeline_layout,
                "verse photo lines",
                "vs_wide",
                Some("fs_wide"),
                &[wgpu::VertexBufferLayout {
                    array_stride: legacy_size * 2,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        WIDE[0],
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: legacy_size,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 24,
                            shader_location: 3,
                        },
                    ],
                }],
                triangles,
                depth_state(false, wgpu::CompareFunction::GreaterEqual),
                Some(PREMULTIPLIED),
                samples,
            ),
        };
        let _ = LEGACY;
        let textured_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse textured base color"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let post = capability
            .hdr
            .map(|hdr| Output::new(device, hdr, output_format));
        let star_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse stars"),
            size: std::mem::size_of::<StarInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        Ok(Self {
            last_lighting: Default::default(),
            capability,
            output_format,
            frame,
            space_ready: false,
            guide_layout,
            scene_layout,
            scene_group,
            frame_group,
            shadow_frames,
            shadow_stride,
            shadow,
            shadow_keys: vec![None; shadow_layers.len()],
            shadow_layers,
            shadow_compare,
            linear_clamp,
            linear_repeat,
            probes,
            probe_version: None,
            sky_textures,
            sky_light,
            pipelines,
            material_layout,
            cooked: std::sync::Mutex::default(),
            empty_group,
            textured_sampler,
            post,
            prepass,
            screen,
            screen_white: white_terms(device, queue),
            stars: star_buffer,
            star_count: 0,
            dynamic_lit: Stream::new(device, "verse dynamic lit"),
            glow: Stream::new(device, "verse glow"),
            sprites: Stream::new(device, "verse sprites"),
            fx_group,
            headroom: 1.0,
        })
    }

    /// Uploads the Sun, Earth, Moon, Milky Way, and star data once.
    pub fn prepare_space(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        if self.space_ready {
            return Ok(());
        }
        self.sky_textures = load_sky(device, queue)?;
        let stars = sky::parse_stars(include_bytes!("../../../verse/assets/lagrange/stars.bin"))?;
        let instances: Vec<StarInstance> = stars
            .iter()
            .map(|s| StarInstance {
                dir: s.dir.to_array(),
                illuminance: s.illuminance,
                color: s.color,
            })
            .collect();
        self.stars = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verse stars"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.star_count = instances.len() as u32;
        self.space_ready = true;
        self.rebuild_groups(device);
        Ok(())
    }

    fn rebuild_groups(&mut self, device: &wgpu::Device) {
        self.scene_group = scene_group(
            device,
            &self.scene_layout,
            &self.frame,
            &self.shadow,
            &self.shadow_compare,
            &self.probes,
            &self.linear_clamp,
            &self.sky_textures,
            &self.linear_repeat,
            &self.sky_light.view,
        );
    }

    fn update_probes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: Option<&ProbeGrid>,
    ) {
        let version = grid.map(|g| g.version);
        if version == self.probe_version {
            return;
        }
        self.probe_version = version;
        self.probes = match grid {
            Some(grid) => probe_textures(device, queue, grid),
            None => empty_probes(device, queue),
        };
        self.rebuild_groups(device);
    }

    /// Uploads a merged textured scene once: its vertices and indices, each
    /// image with its mip chain (coverage-preserving for masked materials,
    /// without levels above the device's texture limit), and each
    /// material's factors.
    pub fn upload_textured(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &TexturedScene,
        merged: &textured::Merged,
    ) -> TexturedGpu {
        // A light bake that finishes after the upload rewrites the vertices.
        self.upload_textured_with(
            device,
            queue,
            scene,
            merged,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            false,
        )
    }

    /// Uploads a [`textured::Figure`]'s images, materials, and indices, with
    /// a vertex buffer [`TexturedGpu::write_vertices`] rewrites each frame.
    pub fn upload_figure(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        figure: &textured::Figure,
    ) -> TexturedGpu {
        self.upload_textured_with(
            device,
            queue,
            &figure.scene,
            &figure.merged(),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            true,
        )
    }

    fn upload_textured_with(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &TexturedScene,
        merged: &textured::Merged,
        vertex_usage: wgpu::BufferUsages,
        figure: bool,
    ) -> TexturedGpu {
        let max = device.limits().max_texture_dimension_2d;
        let mut cooked = self
            .cooked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A world scene starts the cache over with its own images, which a
        // figure drawn over it (a town's chunks, in the same pack's images)
        // then reuses.
        if !figure || cooked.len() > MAX_COOKED {
            cooked.clear();
        }
        let images: std::collections::BTreeMap<_, wgpu::TextureView> = scene
            .mip_variants()
            .into_iter()
            .map(|variant| {
                let image = &scene.images[variant.texture];
                let key = TextureKey::of(image, variant.role);
                if let Some(view) = cooked.get(&key) {
                    return (variant, view.clone());
                }
                let levels = verse_engine::mips::cook(
                    image.width,
                    image.height,
                    &image.rgba,
                    variant.role,
                    max,
                )
                .expect("admitted image and material");
                let view = upload_levels(device, queue, &image.name, &levels);
                cooked.insert(key, view.clone());
                (variant, view)
            })
            .collect();
        drop(cooked);
        let white = upload_levels(
            device,
            queue,
            "verse textured white",
            &[(1, 1, vec![255; 4])],
        );
        let groups = scene
            .materials
            .iter()
            .map(|material| {
                let factors = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("verse textured material"),
                    contents: bytemuck::bytes_of(&textured::uniform(material)),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let view = material.image.map_or(&white, |texture| {
                    &images[&verse_engine::mips::Variant {
                        texture,
                        role: textured::material_role(material),
                    }]
                });
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("verse textured material"),
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.textured_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: factors.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();
        // Buffers cannot be empty; an empty scene draws no batches.
        let vertex_bytes: &[u8] = if merged.vertices.is_empty() {
            &[0; std::mem::size_of::<TexturedVertex>()]
        } else {
            bytemuck::cast_slice(&merged.vertices)
        };
        let index_bytes: &[u8] = if merged.indices.is_empty() {
            &[0; 4]
        } else {
            bytemuck::cast_slice(&merged.indices)
        };
        TexturedGpu {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("verse textured vertices"),
                contents: vertex_bytes,
                usage: vertex_usage,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("verse textured indices"),
                contents: index_bytes,
                // Zones rewrite ranges of a static scene's indices.
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            }),
            batches: merged.batches.clone(),
            materials: scene.materials.clone(),
            groups,
            edits: 0,
            near: vec![true; merged.batches.len()],
            placed: false,
            levels: 0,
        }
    }

    /// A figure's batches in drawing order, never culled.
    fn figure_order(
        figure: Option<&TexturedGpu>,
        view: verse_engine::presentation::View,
    ) -> Vec<usize> {
        figure.map_or_else(Vec::new, |gpu| {
            textured::draw_order(&gpu.batches, &gpu.materials, view.eye, |_, _| true)
        })
    }

    /// The textured cells this frame draws, in drawing order: those at their
    /// current level of detail and in view, and, when fog is total at `far`
    /// meters, those nearer than the fog and large enough to see
    /// ([`textured::drawn`]).
    fn textured_order(
        textured: Option<&TexturedGpu>,
        view: verse_engine::presentation::View,
        far: f32,
    ) -> Vec<usize> {
        textured.map_or_else(Vec::new, |gpu| {
            textured::draw_order(&gpu.batches, &gpu.materials, view.eye, |i, b| {
                gpu.shown(i) && textured::drawn(b.min, b.max, view.view_proj, view.eye, far)
            })
        })
    }

    /// Draws the cells of one textured pass, in `order`. Groups 0 and 1
    /// must be bound.
    fn draw_textured(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        textured: Option<&TexturedGpu>,
        order: &[usize],
        which: Pass,
    ) {
        let Some(gpu) = textured else {
            return;
        };
        let mut sides = None;
        let mut bound = None;
        for &i in order {
            let batch = &gpu.batches[i];
            let material = &gpu.materials[batch.material];
            if material.alpha.pass() != which {
                continue;
            }
            if sides.is_none() {
                pass.set_vertex_buffer(0, gpu.vertices.slice(..));
                pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
            }
            if sides != Some(material.double_sided) {
                let pipeline = &self.pipelines.textured[which as usize];
                pass.set_pipeline(&pipeline[usize::from(material.double_sided)]);
                sides = Some(material.double_sided);
            }
            if bound != Some(batch.material) {
                pass.set_bind_group(2, &gpu.groups[batch.material], &[]);
                bound = Some(batch.material);
            }
            pass.draw_indexed(batch.first..batch.first + batch.count, 0, 0..1);
        }
    }

    /// Size-dependent targets, rebuilt when the size changes.
    pub fn targets(&self, device: &wgpu::Device, width: u32, height: u32) -> PhotoTargets {
        let scene_format = self.capability.hdr.unwrap_or(self.output_format);
        let texture = |label, format, w, h, samples, mips, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: mips,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let sampled = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let samples = self.capability.samples;
        let msaa = (samples > 1).then(|| {
            texture(
                "verse photo msaa",
                scene_format,
                width,
                height,
                samples,
                1,
                attach,
            )
            .create_view(&Default::default())
        });
        let scene = texture(
            "verse photo scene",
            scene_format,
            width,
            height,
            1,
            1,
            sampled,
        )
        .create_view(&Default::default());
        let depth = texture(
            "verse photo depth",
            DEPTH,
            width,
            height,
            samples,
            1,
            attach,
        )
        .create_view(&Default::default());
        let adapt = output::adapt_textures(device, scene_format);
        let chain = self
            .post
            .as_ref()
            .map(|post| post.targets(device, &scene, &adapt, width, height));
        let screen = self
            .screen
            .as_ref()
            .map(|screen| screen.targets(device, width, height));
        let terms = screen
            .as_ref()
            .map_or(&self.screen_white, |screen| &screen.occlusion);
        let guide_groups = [0, 1].map(|k| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse photo guides"),
                layout: &self.guide_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&adapt[k]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(terms),
                    },
                ],
            })
        });
        PhotoTargets {
            guide_groups,
            screen,
            size: [width, height],
            msaa,
            scene,
            depth,
            output: chain,
        }
    }

    /// Encodes one physical frame into `output`, then the HUD.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut PhotoTargets,
        view: verse_engine::presentation::View,
        stage: Stage<'_>,
        world: Batches<'_>,
        ui: Option<(&wgpu::RenderPipeline, &wgpu::BindGroup, &wgpu::Buffer, u32)>,
    ) {
        match stage {
            Stage::Space(sky) => {
                self.encode_space(device, queue, encoder, output, targets, view, sky, world)
            }
            Stage::Neon(neon) => {
                self.encode_neon(device, queue, encoder, output, targets, view, neon, world)
            }
        }
        if let Some((pipeline, group, buffer, count)) = ui
            && count > 0
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse photo hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..count, 0..1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_space(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut PhotoTargets,
        view: verse_engine::presentation::View,
        sky: &Sky,
        world: Batches<'_>,
    ) {
        self.update_probes(device, queue, sky.probes.as_deref());
        let [width, height] = targets.size;
        let camera = &sky.camera;
        let exposure = camera.exposure();
        self.last_lighting = verse_engine::lighting::FrameLighting {
            profile: verse_engine::lighting::PointProfile::Candela,
            exposure,
            grade_stops: 0.0,
            selected_points: vec![],
            shadowed_points: vec![],
            shadow_views: 1,
            shadow_size: SHADOW_SIZE,
            ambient: "sun and Earth; optional irradiance probes",
        };
        let sun = sky.sun_dir.normalize();
        let shadow = fit_box(sun, sky.shadow_center, sky.shadow_half, SHADOW_SIZE);
        // The projection's vertical scale is 1 / tan(fov / 2); read it from the
        // combined matrix, whose rotation part is orthonormal.
        let m = view.view_proj;
        let scale = Vec3::new(m.x_axis.y, m.y_axis.y, m.z_axis.y)
            .length()
            .max(1e-6);
        let pixel_angle = 2.0 / (scale * height as f32);
        let earth_light = earth_illuminance(sky);
        // Reversed depth (Reed 2015): map depth d to 1 − d so a float buffer
        // keeps micrometer precision across the station. Solar cells sit a
        // millimeter proud of their panel.
        let reversed = reversed_depth() * view.view_proj;
        let probe = sky.probes.as_deref();
        let axes = |m: glam::Mat3, w: f32| {
            [
                m.x_axis.extend(0.0).to_array(),
                m.y_axis.extend(0.0).to_array(),
                m.z_axis.extend(w).to_array(),
            ]
        };
        let [ex, ey, ez] = axes(sky.earth.axes, sky.earth.distance as f32);
        let [mx, my, mz] = axes(sky.moon.axes, sky.moon.distance as f32);
        let [cx, cy, cz] = axes(sky.celestial, 0.0);
        let solid = std::f32::consts::PI * sky.sun_angular_radius.tan().powi(2);
        let mut frame = Frame {
            view_proj: reversed.to_cols_array_2d(),
            inv_view_proj: reversed.inverse().to_cols_array_2d(),
            light: Mat4::IDENTITY.to_cols_array_2d(),
            eye: view.eye.extend(exposure).to_array(),
            sun: sun.extend(sky.sun_illuminance).to_array(),
            sun_disc: [
                sky.sun_angular_radius,
                sky.sun_illuminance / solid,
                sky.sun_visible,
                0.0,
            ],
            earth: sky.earth.dir.extend(sky.earth.angular_radius).to_array(),
            earth_x: ex,
            earth_y: ey,
            earth_z: ez,
            moon: sky.moon.dir.extend(sky.moon.angular_radius).to_array(),
            moon_x: mx,
            moon_y: my,
            moon_z: mz,
            celestial_x: cx,
            celestial_y: cy,
            celestial_z: cz,
            earth_light: [earth_light[0], earth_light[1], earth_light[2], 0.0],
            viewport: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
            probe_origin: probe
                .map_or([0.0, 0.0, 0.0, 1.0], |p| p.origin.extend(p.cell).to_array()),
            probe_dims: probe.map_or([1.0, 1.0, 1.0, 0.0], |p| {
                [p.dims[0] as f32, p.dims[1] as f32, p.dims[2] as f32, 1.0]
            }),
            params: [camera.star_gain, sky.time, pixel_angle, 1.0],
            lamp_params: [0.0, 1.0, 0.0, 0.0],
            metering: [
                0.18,
                2f32.powf(camera.ev100 - camera.ev_max),
                2f32.powf(camera.ev100 - camera.ev_min),
                f32::from(u8::from(camera.auto_exposure && self.post.is_some())),
            ],
            neon: [0.0, 0.0, 1.6, 0.0],
            field: [0.0; 4],
            sky_zenith: [0.0; 4],
            sky_horizon: [0.0; 4],
            sky_sun: [0.0; 4],
            ..Frame::zeroed()
        };
        frame.set_cascades(&shadow);
        queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));

        self.encode_shadow(queue, encoder, &frame, &shadow, &world);
        self.encode_screen(queue, encoder, targets, &frame, Some(sun), &world);
        let order = Self::textured_order(world.textured, view, f32::INFINITY);
        let figure_order = Self::figure_order(world.figure, view);

        // Scene pass.
        let direct = self.post.is_none();
        let (target, resolve) = match (&targets.msaa, direct) {
            (Some(msaa), false) => (msaa, Some(&targets.scene)),
            (None, false) => (&targets.scene, None),
            (Some(msaa), true) => (msaa, Some(output)),
            (None, true) => (output, None),
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse photo scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.scene_group, &[]);
            pass.set_pipeline(&self.pipelines.background);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipelines.stars);
            pass.set_vertex_buffer(0, self.stars.slice(..));
            pass.draw(0..6, 0..self.star_count);
            // The Sun first, then the farther of the Earth and Moon.
            pass.set_pipeline(&self.pipelines.bodies);
            pass.draw(0..6, 0..1);
            let bodies = if sky.moon.distance > sky.earth.distance {
                [2, 1]
            } else {
                [1, 2]
            };
            for kind in bodies {
                pass.draw(0..6, kind..kind + 1);
            }
            // The texture written last frame holds the newest adaptation;
            // lit surfaces read the screen-space terms from the same group.
            pass.set_bind_group(1, &targets.guide_groups[targets.parity() ^ 1], &[]);
            pass.set_pipeline(&self.pipelines.lit);
            for (buffer, count) in [
                world.lit,
                (&self.dynamic_lit.buffer, self.dynamic_lit.count),
            ] {
                if count > 0 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..count, 0..1);
                }
            }
            for which in [Pass::Opaque, Pass::Masked] {
                self.draw_textured(&mut pass, world.textured, &order, which);
                self.draw_textured(&mut pass, world.figure, &figure_order, which);
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some((source, globals)) = world.streamed {
                source.draw(&mut pass, globals, true);
                pass.set_bind_group(0, &self.scene_group, &[]);
                pass.set_bind_group(1, &targets.guide_groups[targets.parity() ^ 1], &[]);
            }
            pass.set_pipeline(&self.pipelines.legacy);
            for (buffer, count) in world.faces {
                if count > 0 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..count, 0..1);
                }
            }
            self.draw_textured(&mut pass, world.textured, &order, Pass::Blended);
            self.draw_textured(&mut pass, world.figure, &figure_order, Pass::Blended);
            pass.set_pipeline(&self.pipelines.wide);
            for (buffer, count) in world.lines {
                if count >= 2 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..count / 2);
                }
            }
            if self.glow.count > 0 {
                pass.set_pipeline(&self.pipelines.glow);
                pass.set_vertex_buffer(0, self.glow.buffer.slice(..));
                pass.draw(0..self.glow.count, 0..1);
            }
            if self.sprites.count > 0 {
                pass.set_pipeline(&self.pipelines.sprites);
                pass.set_bind_group(2, &self.empty_group, &[]);
                pass.set_bind_group(3, &self.fx_group, &[]);
                pass.set_vertex_buffer(0, self.sprites.buffer.slice(..));
                pass.draw(0..self.sprites.count, 0..1);
            }
            if sky.sun_visible > 0.0 {
                pass.set_pipeline(&self.pipelines.flare);
                pass.draw(0..6, 0..1);
            }
        }

        let camera = sky.camera;
        self.post_chain(
            queue,
            encoder,
            output,
            targets,
            &Look {
                bloom: camera.bloom,
                local: camera.local_exposure,
                grain: camera.grain,
                vignette: camera.vignette,
                fringe: camera.fringe,
                ghosts: camera.ghosts,
                auto: camera.auto_exposure,
                gain_min: 2f32.powf(camera.ev100 - camera.ev_max),
                gain_max: 2f32.powf(camera.ev100 - camera.ev_min),
                grade: Grade {
                    balance: Vec3::from(white_balance(camera.white_balance)),
                    ceiling: self.headroom,
                    ..Grade::NEUTRAL
                },
                time: sky.time,
            },
        );
    }

    /// The shadow of a stage's key light: cascades that follow the camera
    /// when the key sets a shadow distance and the camera is a perspective
    /// one, else one map over the key's fixed region.
    fn key_shadow(&self, key: &super::Key, view: verse_engine::presentation::View) -> Cascades {
        let toward = key.dir.normalize_or(Vec3::Y);
        let fixed = || fit_box(toward, key.shadow_center, key.shadow_half, SHADOW_SIZE);
        let Some(distance) = key.shadow_distance else {
            return fixed();
        };
        let count = (self.capability.quality.cascades as usize).min(self.shadow_layers.len());
        let mut settings = CascadeSettings::new(count as u32, distance);
        settings.resolution = SHADOW_SIZE;
        if !key.cache_far_shadows {
            settings.cache_cell = 0;
        }
        Frustum::from_view_proj(view.view_proj, view.eye)
            .and_then(|frustum| fit_cascades(&frustum, toward, &settings).ok())
            .unwrap_or_else(fixed)
    }

    /// The sun or key light's shadow maps, one layer per cascade. A cascade
    /// draws every lit triangle and every opaque or masked textured cell. A
    /// cached cascade draws the static ones only, the world's lit triangles
    /// and textured scene, and only when its matrix or the static scene has
    /// changed since it was drawn; otherwise its layer keeps last frame's
    /// depth.
    fn encode_shadow(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        cascades: &Cascades,
        world: &Batches<'_>,
    ) {
        let identity = static_identity(world);
        let count = cascades.cascades.len().min(self.shadow_layers.len());
        for (layer, cascade) in cascades.cascades.iter().take(count).enumerate() {
            let key = (cascade.matrix.to_cols_array(), identity);
            if cascade.cached {
                if self.shadow_keys[layer] == Some(key) {
                    continue;
                }
                self.shadow_keys[layer] = Some(key);
            } else {
                self.shadow_keys[layer] = None;
            }
            let mut copy = *frame;
            copy.light = cascade.matrix.to_cols_array_2d();
            let offset = self.shadow_stride * layer as u64;
            queue.write_buffer(&self.shadow_frames, offset, bytemuck::bytes_of(&copy));
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse sun shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_layers[layer],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_group, &[offset as u32]);
            let dynamic = (&self.dynamic_lit.buffer, self.dynamic_lit.count);
            let lit = if cascade.cached {
                [world.lit, (dynamic.0, 0)]
            } else {
                [world.lit, dynamic]
            };
            let figure = world.figure.filter(|_| !cascade.cached);
            let casters = [
                &self.pipelines.shadow,
                &self.pipelines.textured_shadow,
                &self.pipelines.textured_shadow_masked,
            ];
            // A cell outside the cascade's sides casts nothing into its map.
            let matrix = cascade.matrix;
            let keep = |b: &textured::Batch| textured::in_slab(b.min, b.max, matrix);
            self.draw_casters(&mut pass, casters, lit, [world.textured, figure], &keep);
        }
    }

    /// Draws casters into a bound shadow or prepass pass: lit triangles,
    /// then the opaque and the masked cells of textured meshes, through
    /// `pipelines` for each in that order.
    fn draw_casters(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipelines: [&wgpu::RenderPipeline; 3],
        lit: [(&wgpu::Buffer, u32); 2],
        textured: [Option<&TexturedGpu>; 2],
        keep: &dyn Fn(&textured::Batch) -> bool,
    ) {
        let [lit_pipeline, opaque_pipeline, masked_pipeline] = pipelines;
        pass.set_pipeline(lit_pipeline);
        for (buffer, count) in lit {
            if count > 0 {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..count, 0..1);
            }
        }
        // `keep` culls the world's cells; the figure's always draw.
        for (k, gpu) in textured.into_iter().enumerate() {
            let Some(gpu) = gpu else {
                continue;
            };
            pass.set_vertex_buffer(0, gpu.vertices.slice(..));
            pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
            for masked in [false, true] {
                if masked {
                    pass.set_pipeline(masked_pipeline);
                    pass.set_bind_group(1, &self.empty_group, &[]);
                } else {
                    pass.set_pipeline(opaque_pipeline);
                }
                for (i, batch) in gpu.batches.iter().enumerate() {
                    let cell_pass = gpu.materials[batch.material].alpha.pass();
                    if textured::raster(cell_pass, false).shadow != Some(masked)
                        || !gpu.shown(i)
                        || (k == 0 && !keep(batch))
                    {
                        continue;
                    }
                    if masked {
                        pass.set_bind_group(2, &gpu.groups[batch.material], &[]);
                    }
                    pass.draw_indexed(batch.first..batch.first + batch.count, 0, 0..1);
                }
            }
        }
    }

    /// The high tier's depth prepass and screen-space passes, after the
    /// shadow maps and before the scene. The prepass draws what the shadow
    /// maps draw, through a copy of `frame` whose `light` is the camera's
    /// matrix; `light` points toward the light that casts contact shadows.
    /// Other tiers run nothing here.
    fn encode_screen(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &PhotoTargets,
        frame: &Frame,
        light: Option<Vec3>,
        world: &Batches<'_>,
    ) {
        let (Some(prepass), Some(screen), Some(screen_targets)) =
            (&self.prepass, &self.screen, &targets.screen)
        else {
            return;
        };
        let mut copy = *frame;
        copy.light = frame.view_proj;
        let offset = self.shadow_stride * MAX_CASCADES as u64;
        queue.write_buffer(&self.shadow_frames, offset, bytemuck::bytes_of(&copy));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse depth prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &screen_targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_group, &[offset as u32]);
            let dynamic = (&self.dynamic_lit.buffer, self.dynamic_lit.count);
            self.draw_casters(
                &mut pass,
                [&prepass.lit, &prepass.textured, &prepass.masked],
                [world.lit, dynamic],
                [world.textured, world.figure],
                &|_| true,
            );
        }
        let uniform = ScreenUniform::new(
            Mat4::from_cols_array_2d(&frame.view_proj),
            Vec3::new(frame.eye[0], frame.eye[1], frame.eye[2]),
            light,
            targets.size,
        );
        screen.encode(queue, encoder, screen_targets, &uniform);
    }

    /// The neon stage: faces, emissive lines, and the post chain with a
    /// hue-preserving curve. With a studio [`Key`](super::Key), lit geometry
    /// is shaded by it, with its shadow map and ambient probes, before the
    /// lines.
    #[allow(clippy::too_many_arguments)]
    fn encode_neon(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut PhotoTargets,
        view: verse_engine::presentation::View,
        neon: &Neon,
        world: Batches<'_>,
    ) {
        let [width, height] = targets.size;
        let reversed = reversed_depth() * view.view_proj;
        let frame = |view_proj: Mat4, width_px: f32, mode: f32| Frame {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            light: Mat4::IDENTITY.to_cols_array_2d(),
            eye: view.eye.extend(1.0).to_array(),
            sun: [0.0, 1.0, 0.0, 0.0],
            sun_disc: [0.0; 4],
            earth: [0.0, 0.0, 1.0, 0.0],
            earth_x: [1.0, 0.0, 0.0, 0.0],
            earth_y: [0.0, 1.0, 0.0, 0.0],
            earth_z: [0.0, 0.0, 1.0, 1.0],
            moon: [0.0, 0.0, 1.0, 0.0],
            moon_x: [1.0, 0.0, 0.0, 0.0],
            moon_y: [0.0, 1.0, 0.0, 0.0],
            moon_z: [0.0, 0.0, 1.0, 1.0],
            celestial_x: [1.0, 0.0, 0.0, 0.0],
            celestial_y: [0.0, 1.0, 0.0, 0.0],
            celestial_z: [0.0, 0.0, 1.0, 0.0],
            earth_light: [0.0, 0.0, 0.0, 1.0],
            viewport: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
            probe_origin: [0.0, 0.0, 0.0, 1.0],
            probe_dims: [1.0, 1.0, 1.0, 0.0],
            params: [0.0, neon.time, 0.0, neon.line_gain],
            metering: [0.18, 1.0, 1.0, 0.0],
            neon: [neon.fog_start, neon.fog_end, width_px, mode],
            field: [neon.field[0], neon.field[1], neon.field[2], 0.0],
            sky_zenith: [0.0; 4],
            sky_horizon: [0.0; 4],
            sky_sun: [0.0; 4],
            ..Frame::zeroed()
        };
        let mut uniform = frame(reversed, neon.line_width, 1.0);
        let daylight = neon.daylight.filter(super::Daylight::valid);
        if let Some(day) = &daylight {
            // The Sun stands where the key light comes from, or overhead.
            let sun = neon.key.map_or(Vec3::Y, |k| k.dir.normalize_or(Vec3::Y));
            let radius = neon.key.map_or(0.03, |k| k.angular_radius);
            uniform.sun = sun.extend(0.0).to_array();
            uniform.sky_zenith = [day.zenith[0], day.zenith[1], day.zenith[2], 1.0];
            uniform.sky_horizon = [day.horizon[0], day.horizon[1], day.horizon[2], day.clouds];
            uniform.sky_sun = [day.sun[0], day.sun[1], day.sun[2], radius];
            uniform.field[3] = day.glow;
        }
        self.last_lighting = verse_engine::lighting::FrameLighting {
            profile: verse_engine::lighting::PointProfile::Candela,
            grade_stops: neon.grade.exposure,
            ambient: if daylight.is_some() {
                "daylight sky and irradiance probes"
            } else {
                "hemispheric irradiance probes"
            },
            ..Default::default()
        };
        let lit = neon.key.filter(|_| {
            world.lit.1 > 0
                || self.dynamic_lit.count > 0
                || world.textured.is_some()
                || world.figure.is_some()
        });
        let mut shadow = None;
        if let Some(key) = &lit {
            // Pre-exposed lux: the stage's lines stay at unit exposure.
            let exposure = super::exposure(key.ev100);
            let probes = key.probes();
            self.update_probes(device, queue, Some(&probes));
            let cascades = self.key_shadow(key, view);
            self.last_lighting.exposure = exposure;
            self.last_lighting.shadow_views = cascades.cascades.len();
            self.last_lighting.shadow_size = SHADOW_SIZE;
            uniform.set_cascades(&cascades);
            shadow = Some(cascades);
            let rim = key.rim_illuminance * exposure;
            uniform.sun = key
                .dir
                .normalize()
                .extend(key.illuminance * exposure)
                .to_array();
            uniform.sun_disc = [key.angular_radius, 0.0, 0.0, 0.0];
            uniform.earth = key
                .rim_dir
                .normalize()
                .extend(key.rim_angular_radius)
                .to_array();
            uniform.earth_light = [
                rim * neon.rim_color[0],
                rim * neon.rim_color[1],
                rim * neon.rim_color[2],
                0.0,
            ];
            uniform.key_tint = [neon.key_color[0], neon.key_color[1], neon.key_color[2], 1.0];
            uniform.lamp_params = [0.0, exposure, 0.0, 0.0];
            let mut count = 0;
            let budget = lamp_budget(self.capability.quality.tier);
            let sources: Vec<_> = neon
                .lamps
                .iter()
                .map(|lamp| verse_engine::lighting::Light {
                    position: lamp.position,
                    color: glam::Vec3::from_array(lamp.color),
                    intensity: if lamp.lit() { lamp.intensity } else { 0.0 },
                    range: lamp.range,
                })
                .collect();
            self.last_lighting.selected_points =
                verse_engine::lighting::select_lights(&sources, view, budget);
            for &index in &self.last_lighting.selected_points {
                let lamp = &neon.lamps[index];
                let gain = lamp.intensity * exposure;
                uniform.lamps[count * 2] = lamp.position.extend(lamp.range).to_array();
                uniform.lamps[count * 2 + 1] = [
                    lamp.color[0] * gain,
                    lamp.color[1] * gain,
                    lamp.color[2] * gain,
                    0.0,
                ];
                count += 1;
            }
            uniform.lamp_params[0] = count as f32;
            uniform.probe_origin = probes.origin.extend(probes.cell).to_array();
            uniform.probe_dims = [
                probes.dims[0] as f32,
                probes.dims[1] as f32,
                probes.dims[2] as f32,
                1.0,
            ];
        }
        if let (Some(key), Some(day)) = (&lit, &daylight) {
            // Under a daylight sky, the sky lights the stage in place of the
            // key's uniform sky and ground, at the key's level.
            let exposure = super::exposure(key.ev100);
            let inputs = SkyInputs {
                daylight: *day,
                sun: key.dir.normalize_or(Vec3::Y),
                sun_illuminance: key.illuminance * exposure,
                level: key.sky * exposure,
            };
            let quality = self.capability.quality;
            if self.sky_light.update(device, queue, &inputs, &quality) {
                self.rebuild_groups(device);
            }
            let flash = if neon.sky_flash.is_finite() {
                neon.sky_flash.clamp(0.0, 1.0)
            } else {
                0.0
            };
            uniform.sky_light = [1.0, self.sky_light.max_lod, flash, 0.0];
            uniform.sky_sh = self.sky_light.sh;
        }
        if let Some(fog) = neon.height_fog.filter(|fog| fog.validate().is_ok()) {
            [uniform.fog_shape, uniform.fog_lobe] = fog.uniform();
        }
        queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&uniform));
        if let Some(shadow) = &shadow {
            self.encode_shadow(queue, encoder, &uniform, shadow, &world);
        }
        if let Some(key) = &lit {
            let toward = key.dir.normalize_or(Vec3::Y);
            self.encode_screen(queue, encoder, targets, &uniform, Some(toward), &world);
        }
        let figure_order = if lit.is_some() {
            Self::figure_order(world.figure, view)
        } else {
            Vec::new()
        };
        // Fog is total at the stage's fog end, so no cell beyond it shows.
        let far = if neon.fog_end > 0.0 {
            neon.fog_end
        } else {
            f32::INFINITY
        };
        let order = if lit.is_some() {
            Self::textured_order(world.textured, view, far)
        } else {
            Vec::new()
        };
        let direct = self.post.is_none();
        let (target, resolve) = match (&targets.msaa, direct) {
            (Some(msaa), false) => (msaa, Some(&targets.scene)),
            (None, false) => (&targets.scene, None),
            (Some(msaa), true) => (msaa, Some(output)),
            (None, true) => (output, None),
        };
        {
            let field = neon.field.map(f64::from);
            let clear = wgpu::Color {
                r: field[0],
                g: field[1],
                b: field[2],
                a: 1.0,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse neon scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(1, &targets.guide_groups[0], &[]);
            pass.set_bind_group(0, &self.scene_group, &[]);
            if daylight.is_some() {
                // Drawn first, at infinity, without depth: everything covers it.
                pass.set_pipeline(&self.pipelines.daylight);
                pass.draw(0..3, 0..1);
            }
            if lit.is_some() {
                pass.set_pipeline(&self.pipelines.lit);
                for (buffer, count) in [
                    world.lit,
                    (&self.dynamic_lit.buffer, self.dynamic_lit.count),
                ] {
                    if count > 0 {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..count, 0..1);
                    }
                }
                for which in [Pass::Opaque, Pass::Masked] {
                    self.draw_textured(&mut pass, world.textured, &order, which);
                    self.draw_textured(&mut pass, world.figure, &figure_order, which);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some((source, globals)) = world.streamed {
                source.draw(&mut pass, globals, true);
                pass.set_bind_group(0, &self.scene_group, &[]);
                pass.set_bind_group(1, &targets.guide_groups[0], &[]);
            }
            pass.set_pipeline(&self.pipelines.legacy);
            for (buffer, count) in world.faces {
                if count > 0 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..count, 0..1);
                }
            }
            self.draw_textured(&mut pass, world.textured, &order, Pass::Blended);
            self.draw_textured(&mut pass, world.figure, &figure_order, Pass::Blended);
            pass.set_pipeline(&self.pipelines.wide);
            for (buffer, count) in world.lines {
                if count >= 2 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..count / 2);
                }
            }
            if self.glow.count > 0 {
                pass.set_pipeline(&self.pipelines.glow);
                pass.set_vertex_buffer(0, self.glow.buffer.slice(..));
                pass.draw(0..self.glow.count, 0..1);
            }
            if self.sprites.count > 0 {
                pass.set_pipeline(&self.pipelines.sprites);
                pass.set_bind_group(2, &self.empty_group, &[]);
                pass.set_bind_group(3, &self.fx_group, &[]);
                pass.set_vertex_buffer(0, self.sprites.buffer.slice(..));
                pass.draw(0..self.sprites.count, 0..1);
            }
        }
        self.post_chain(
            queue,
            encoder,
            output,
            targets,
            &Look {
                bloom: neon.bloom,
                local: 0.0,
                grain: 0.0,
                vignette: neon.vignette,
                fringe: 0.0,
                ghosts: 0.0,
                auto: false,
                gain_min: 1.0,
                gain_max: 1.0,
                // The plaza keeps its standard-range look on HDR displays.
                grade: neon.grade,
                time: neon.time,
            },
        );
    }

    /// Bloom, exposure adaptation, and the graded output transform into
    /// `output`. Direct mode has no float target and skips it.
    fn post_chain(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut PhotoTargets,
        look: &Look,
    ) {
        if let (Some(post), Some(chain)) = (&mut self.post, &mut targets.output) {
            post.encode(queue, encoder, output, chain, look);
        }
    }
}

/// The scene a physical frame shows.
#[derive(Clone, Copy, Debug)]
pub enum Stage<'a> {
    Space(&'a Sky),
    Neon(&'a Neon),
}

/// Identifies the static casters a cached shadow cascade holds: the world's
/// lit triangles and its textured scene. A new upload is a new buffer, so a
/// changed scene changes the identity.
fn static_identity(world: &Batches<'_>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    world.lit.0.hash(&mut hasher);
    world.lit.1.hash(&mut hasher);
    if let Some(gpu) = world.textured {
        gpu.vertices.hash(&mut hasher);
        gpu.indices.hash(&mut hasher);
        gpu.batches.len().hash(&mut hasher);
        gpu.edits.hash(&mut hasher);
        gpu.levels.hash(&mut hasher);
    }
    hasher.finish()
}

/// Reversed depth (Reed 2015): map depth d to 1 − d so a float buffer keeps
/// micrometer precision near the camera and across the scene.
pub fn reversed_depth() -> Mat4 {
    Mat4::from_cols(
        glam::Vec4::X,
        glam::Vec4::Y,
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
    )
}

/// The retained geometry a physical frame draws.
pub struct Batches<'a> {
    #[cfg(not(target_arch = "wasm32"))]
    pub streamed: Option<(&'a crate::streaming::Source, &'a wgpu::BindGroup)>,
    pub lit: (&'a wgpu::Buffer, u32),
    pub faces: [(&'a wgpu::Buffer, u32); 2],
    pub lines: [(&'a wgpu::Buffer, u32); 2],
    pub textured: Option<&'a TexturedGpu>,
    /// The dynamic mesh's figure, its vertices written for this frame.
    pub figure: Option<&'a TexturedGpu>,
}

/// Illuminance at the station from the full Earth, lux per channel: a
/// Lambertian sphere of Bond albedo 0.3 seen at its phase angle.
fn earth_illuminance(sky: &Sky) -> [f32; 3] {
    let r = sky.earth.angular_radius.sin();
    let phase = sky.earth.dir.angle_between(-sky.sun_dir);
    // Lambert-sphere phase law, normalized to one at full phase.
    let law = ((std::f32::consts::PI - phase) * phase.cos() + phase.sin()) / std::f32::consts::PI;
    let e = 2.0 / 3.0 * 0.3 * sky.sun_illuminance * r * r * law.max(0.0);
    // Earthlight is bluer than sunlight.
    [e * 0.85, e, e * 1.2]
}

fn white_balance(kelvin: f32) -> [f32; 3] {
    let c = sky::blackbody_color(kelvin.clamp(2_000.0, 12_000.0));
    [1.0 / c[0], 1.0 / c[1], 1.0 / c[2]]
}

#[allow(clippy::too_many_arguments)]
fn scene_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frame: &wgpu::Buffer,
    shadow: &wgpu::TextureView,
    compare: &wgpu::Sampler,
    probes: &[wgpu::TextureView; 3],
    linear_clamp: &wgpu::Sampler,
    sky: &[wgpu::TextureView; 5],
    linear_repeat: &wgpu::Sampler,
    sky_light: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let view = |binding, view| wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("verse photo scene"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            },
            view(1, shadow),
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(compare),
            },
            view(3, &probes[0]),
            view(4, &probes[1]),
            view(5, &probes[2]),
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(linear_clamp),
            },
            view(7, &sky[0]),
            view(8, &sky[1]),
            view(9, &sky[2]),
            view(10, &sky[3]),
            view(11, &sky[4]),
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::Sampler(linear_repeat),
            },
            view(13, sky_light),
        ],
    })
}

/// IEEE 754 binary16 bits for `value`, rounding to nearest.
#[must_use]
pub fn half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x200 } else { 0 };
    }
    let e = exponent - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7bff;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = (mantissa | 0x0080_0000) >> (1 - e);
        return sign | ((m + 0x1000) >> 13) as u16;
    }
    let rounded = mantissa + 0x1000;
    if rounded & 0x0080_0000 != 0 {
        return sign | (((e + 1) as u16) << 10);
    }
    sign | ((e as u16) << 10) | (rounded >> 13) as u16
}

fn texture3d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dims: [u32; 3],
    texels: &[u16],
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: dims[0],
        height: dims[1],
        depth_or_array_layers: dims[2],
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse probes"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(texels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(dims[0] * 8),
            rows_per_image: Some(dims[1]),
        },
        size,
    );
    texture.create_view(&Default::default())
}

/// One white texel in the screen-space terms' format.
fn white_terms(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse screen white"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: screen_space::OCCLUSION,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(2),
            rows_per_image: Some(1),
        },
        size,
    );
    texture.create_view(&Default::default())
}

fn empty_probes(device: &wgpu::Device, queue: &wgpu::Queue) -> [wgpu::TextureView; 3] {
    [0, 1, 2].map(|_| texture3d(device, queue, [1, 1, 1], &[0; 4]))
}

fn probe_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    grid: &ProbeGrid,
) -> [wgpu::TextureView; 3] {
    [0, 1, 2].map(|channel| {
        let texels: Vec<u16> = grid
            .data
            .iter()
            .flat_map(|p| (0..4).map(move |k| half(p[channel * 4 + k])))
            .collect();
        texture3d(device, queue, grid.dims, &texels)
    })
}

/// Decodes a bundled PNG into RGBA8 (gray expanded) or single-channel R8.
fn decode(bytes: &[u8], single: bool) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("sky texture: {e}"))?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("sky texture too large")?];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|e| format!("sky texture: {e}"))?;
    let pixels = &buffer[..info.buffer_size()];
    let channels = info.color_type.samples();
    let count = (info.width * info.height) as usize;
    let out = if single {
        (0..count).map(|i| pixels[i * channels]).collect()
    } else {
        let mut out = Vec::with_capacity(count * 4);
        for i in 0..count {
            let p = &pixels[i * channels..(i + 1) * channels];
            match channels {
                1 | 2 => out.extend_from_slice(&[p[0], p[0], p[0], 255]),
                _ => out.extend_from_slice(&[p[0], p[1], p[2], 255]),
            }
        }
        out
    };
    Ok((info.width, info.height, out))
}

/// Uploads a texture with a box-filtered mip chain.
fn upload_mipped(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    (width, height, pixels): (u32, u32, Vec<u8>),
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    let channels = if format == wgpu::TextureFormat::R8Unorm {
        1
    } else {
        4
    };
    let levels = 32 - width.max(height).leading_zeros();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let (mut w, mut h, mut level) = (width, height, pixels);
    for mip in 0..levels {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * channels),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        if mip + 1 == levels {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; (nw * nh * channels) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..channels {
                    let at = |xx: u32, yy: u32| {
                        u32::from(
                            level[((yy.min(h - 1) * w + xx.min(w - 1)) * channels + c) as usize],
                        )
                    };
                    let sum = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[((y * nw + x) * channels + c) as usize] = ((sum + 2) / 4) as u8;
                }
            }
        }
        level = next;
        w = nw;
        h = nh;
    }
    texture.create_view(&Default::default())
}

/// Uploads every fx sheet as one layer of an sRGB texture array with its
/// mip chain, dropping the `skip` largest levels.
fn upload_fx_sheets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    skip: u32,
) -> Result<wgpu::TextureView, String> {
    let levels = crate::fx::sheet::mip_layers(skip)?;
    let layers = crate::fx::sheet::SHEETS.len() as u32;
    let size = levels[0].0;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse fx sheets"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, (edge, sheets)) in levels.iter().enumerate() {
        for (layer, pixels) in sheets.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(edge * 4),
                    rows_per_image: Some(*edge),
                },
                wgpu::Extent3d {
                    width: *edge,
                    height: *edge,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    Ok(texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("verse fx sheets"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    }))
}

/// Uploads RGBA8 sRGB mip levels, largest first, as one texture.
/// Most cooked images [`Photo`] keeps for reuse; past it the cache starts
/// over.
const MAX_COOKED: usize = 256;

/// What identifies a cooked image: its name, size, a sample of its texels,
/// and the role its levels were cooked for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextureKey {
    name: String,
    size: (u32, u32, usize),
    sample: u64,
    role: verse_engine::mips::Role,
}

impl TextureKey {
    fn of(image: &textured::BaseColorImage, role: verse_engine::mips::Role) -> Self {
        // FNV-1a over every 997th byte: cheap, and two different images
        // with one name and size differ in it.
        let mut sample: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in image.rgba.iter().step_by(997) {
            sample = (sample ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Self {
            name: image.name.clone(),
            size: (image.width, image.height, image.rgba.len()),
            sample,
            role,
        }
    }
}

fn upload_levels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    levels: &[(u32, u32, Vec<u8>)],
) -> wgpu::TextureView {
    let (width, height, _) = &levels[0];
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: *width,
            height: *height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, (w, h, pixels)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(*h),
            },
            wgpu::Extent3d {
                width: *w,
                height: *h,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// One-texel stand-ins until a space frame loads the real data.
fn placeholder_sky(device: &wgpu::Device, queue: &wgpu::Queue) -> [wgpu::TextureView; 5] {
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let r8 = wgpu::TextureFormat::R8Unorm;
    [
        upload_mipped(device, queue, "earth day", (1, 1, vec![0, 0, 0, 255]), srgb),
        upload_mipped(device, queue, "earth clouds", (1, 1, vec![0]), r8),
        upload_mipped(device, queue, "earth water", (1, 1, vec![0]), r8),
        upload_mipped(
            device,
            queue,
            "moon albedo",
            (1, 1, vec![0, 0, 0, 255]),
            srgb,
        ),
        upload_mipped(device, queue, "milky way", (1, 1, vec![0, 0, 0, 255]), srgb),
    ]
}

fn load_sky(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<[wgpu::TextureView; 5], String> {
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let r8 = wgpu::TextureFormat::R8Unorm;
    Ok([
        upload_mipped(
            device,
            queue,
            "earth day",
            decode(
                include_bytes!("../../../verse/assets/lagrange/earth_day.png"),
                false,
            )?,
            srgb,
        ),
        upload_mipped(
            device,
            queue,
            "earth clouds",
            decode(
                include_bytes!("../../../verse/assets/lagrange/earth_clouds.png"),
                true,
            )?,
            r8,
        ),
        upload_mipped(
            device,
            queue,
            "earth water",
            decode(
                include_bytes!("../../../verse/assets/lagrange/earth_water.png"),
                true,
            )?,
            r8,
        ),
        upload_mipped(
            device,
            queue,
            "moon albedo",
            decode(
                include_bytes!("../../../verse/assets/lagrange/moon_albedo.png"),
                false,
            )?,
            srgb,
        ),
        upload_mipped(
            device,
            queue,
            "milky way",
            decode(
                include_bytes!("../../../verse/assets/lagrange/milky_way.png"),
                false,
            )?,
            srgb,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_tiers_shade_fewer_lamps() {
        assert!(lamp_budget(Tier::Low) < lamp_budget(Tier::Medium));
        assert!(lamp_budget(Tier::Medium) < lamp_budget(Tier::High));
        assert_eq!(lamp_budget(Tier::High), super::super::MAX_LAMPS);
    }

    #[test]
    fn the_tier_sets_the_shadow_filter_and_material_detail_constants() {
        let capability = |tier: Tier| Capability {
            hdr: None,
            samples: 1,
            gles: true,
            quality: tier.quality(),
        };
        assert_eq!(
            capability(Tier::Low).tier_constants(),
            [("PCSS", 0.0), ("DETAIL", 0.0), ("SCREEN", 0.0)]
        );
        assert_eq!(
            capability(Tier::Medium).tier_constants(),
            [("PCSS", 1.0), ("DETAIL", 1.0), ("SCREEN", 0.0)]
        );
        assert_eq!(
            capability(Tier::High).tier_constants(),
            [("PCSS", 1.0), ("DETAIL", 1.0), ("SCREEN", 1.0)]
        );
    }

    /// Only the high tier declares the depth prepass and the screen-space
    /// passes.
    #[test]
    fn the_tier_gates_the_screen_space_passes() {
        for tier in Tier::ALL {
            let quality = tier.quality();
            let plan = PhotoPlan::build(&quality).unwrap();
            let high = tier == Tier::High;
            for pass in [
                PhotoPass::DepthPrepass,
                PhotoPass::ScreenTrace,
                PhotoPass::ScreenResolve,
            ] {
                assert_eq!(plan.runs(pass), high, "{tier:?} {pass:?}");
            }
            assert_eq!(quality.screen_space, high);
        }
    }

    /// The Rust frame uniform has the WGSL `Frame`'s size, so the cascade
    /// fields land where the shader reads them, and each shadow pass's copy
    /// sits at an offset the device accepts.
    #[test]
    fn the_frame_uniform_matches_the_shader() {
        for gles in [false, true] {
            let shared = crate::shading::source(include_str!("photo.wgsl"));
            let source = verse_gfx::gles::wgsl(&shared, gles);
            let module = naga::front::wgsl::parse_str(&source).unwrap();
            let mut layouter = naga::proc::Layouter::default();
            layouter.update(module.to_ctx()).unwrap();
            let (frame, _) = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some("Frame"))
                .expect("photo.wgsl declares Frame");
            assert_eq!(layouter[frame].size as usize, std::mem::size_of::<Frame>());
        }
        assert_eq!(std::mem::size_of::<Frame>() % 16, 0);
    }

    /// The fixed region's single map keeps the old fit: a cube's width of
    /// reach on both sides and a texel of the cube over the map.
    #[test]
    fn a_fixed_key_shadow_is_one_map_and_a_cascaded_one_follows_the_tier() {
        let cascades = fit_box(Vec3::Y, Vec3::ZERO, 40.0, SHADOW_SIZE);
        let mut frame = Frame::zeroed();
        frame.set_cascades(&cascades);
        assert_eq!(frame.cascade_params[0], 1.0);
        assert_eq!(frame.cascade_texel[0], 80.0 / SHADOW_SIZE as f32);
        assert_eq!(frame.cascade_depth[0], 160.0);
        assert_eq!(frame.light, frame.cascades[0]);
        assert_eq!(frame.view_forward, [0.0; 4]);
        for tier in Tier::ALL {
            let count = tier.quality().cascades as usize;
            assert!((2..=MAX_CASCADES).contains(&count), "{tier:?}");
        }
    }

    #[test]
    fn earthshine_at_l1_is_a_few_millionths_of_sunlight() {
        let body = |dir: Vec3, radius: f32| crate::pbr::Body {
            dir,
            angular_radius: radius,
            distance: 1.5e9,
            axes: glam::Mat3::IDENTITY,
        };
        let sky = Sky {
            sun_dir: -Vec3::Z,
            sun_illuminance: 130_000.0,
            sun_angular_radius: 0.0047,
            sun_visible: 1.0,
            earth: body(Vec3::Z, 6.371e6 / 1.5e9),
            moon: body(Vec3::Z, 1.7e6 / 1.5e9),
            celestial: glam::Mat3::IDENTITY,
            shadow_center: Vec3::ZERO,
            shadow_half: 30.0,
            camera: crate::pbr::Camera::helmet(),
            probes: None,
            time: 0.0,
        };
        let e = earth_illuminance(&sky)[1];
        let ratio = e / sky.sun_illuminance;
        assert!((2e-6..8e-6).contains(&ratio), "{ratio}");
    }

    #[test]
    fn half_floats_round_trip_the_values_the_probes_hold() {
        assert_eq!(half(0.0), 0);
        assert_eq!(half(1.0), 0x3c00);
        assert_eq!(half(-2.0), 0xc000);
        assert_eq!(half(65504.0), 0x7bff);
        assert_eq!(half(1.0e6), 0x7bff);
        assert_eq!(half(0.5), 0x3800);
    }

    #[test]
    fn bundled_sky_textures_decode_at_their_documented_sizes() {
        let (w, h, px) = decode(
            include_bytes!("../../../verse/assets/lagrange/earth_day.png"),
            false,
        )
        .unwrap();
        assert_eq!((w, h, px.len()), (2048, 1024, 2048 * 1024 * 4));
        let (w, h, px) = decode(
            include_bytes!("../../../verse/assets/lagrange/earth_clouds.png"),
            true,
        )
        .unwrap();
        assert_eq!((w, h, px.len()), (2048, 1024, 2048 * 1024));
        let (w, h, _) = decode(
            include_bytes!("../../../verse/assets/lagrange/moon_albedo.png"),
            false,
        )
        .unwrap();
        assert_eq!((w, h), (2048, 1024));
        let stars =
            sky::parse_stars(include_bytes!("../../../verse/assets/lagrange/stars.bin")).unwrap();
        assert!(stars.len() > 9_000);
    }
}
