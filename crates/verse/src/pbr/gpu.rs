//! GPU resources and frame encoding for the physical path.
//!
//! One frame runs these passes: a sun shadow map; a floating-point scene pass
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
//! constants.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::output::{self, Look, Output, OutputTargets};
use super::textured::{self, Pass, TexturedMaterial, TexturedScene, TexturedVertex};
use super::{GlowVertex, LitVertex, Neon, ProbeGrid, Sky, sky};
use verse_engine::lighting::Grade;
use verse_engine::quality::{Platform, Probe, Quality, ShadowFilter, Tier};

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StarInstance {
    dir: [f32; 3],
    illuminance: f32,
    color: [f32; 3],
}

/// A growable vertex buffer.
pub(crate) struct Stream {
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
pub(crate) struct Capability {
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
        {
            4
        } else {
            1
        };
        let gles = crate::gles::is_gles(adapter.get_info().backend);
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
        Self {
            hdr,
            samples,
            gles,
            quality: probe.select(asked).quality(),
        }
    }

    /// The pipeline constants the tier sets in `photo.wgsl`.
    fn tier_constants(&self) -> [(&'static str, f64); 2] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        [
            (
                "PCSS",
                flag(self.quality.shadow_filter == ShadowFilter::Soft),
            ),
            ("DETAIL", flag(self.quality.materials.detail_normals)),
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
    legacy: wgpu::RenderPipeline,
    wide: wgpu::RenderPipeline,
    /// Textured meshes by [`Pass`], single-sided then double-sided.
    textured: [[wgpu::RenderPipeline; 2]; 3],
    /// Opaque textured meshes into the shadow map.
    textured_shadow: wgpu::RenderPipeline,
    /// Masked textured meshes into the shadow map, testing alpha.
    textured_shadow_masked: wgpu::RenderPipeline,
}

/// Textured static meshes on the GPU: merged vertices and indices uploaded
/// once, and one bind group per material.
pub(crate) struct TexturedGpu {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    batches: Vec<textured::Batch>,
    materials: Vec<TexturedMaterial>,
    groups: Vec<wgpu::BindGroup>,
}

impl TexturedGpu {
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
pub(crate) struct PhotoTargets {
    size: [u32; 2],
    msaa: Option<wgpu::TextureView>,
    scene: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// Bloom and adaptation, when the scene renders to a float target.
    output: Option<OutputTargets>,
    /// The adapted luminance for guides, indexed by the texture last written.
    guide_groups: [wgpu::BindGroup; 2],
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
pub(crate) struct Photo {
    capability: Capability,
    output_format: wgpu::TextureFormat,
    frame: wgpu::Buffer,
    /// Whether the Sun, Earth, Moon, and star data are uploaded.
    space_ready: bool,
    guide_layout: wgpu::BindGroupLayout,
    scene_layout: wgpu::BindGroupLayout,
    scene_group: wgpu::BindGroup,
    /// The frame uniform alone, for the shadow pass that writes the map.
    frame_group: wgpu::BindGroup,
    shadow: wgpu::TextureView,
    shadow_compare: wgpu::Sampler,
    linear_clamp: wgpu::Sampler,
    linear_repeat: wgpu::Sampler,
    probes: [wgpu::TextureView; 3],
    probe_version: Option<u64>,
    sky_textures: [wgpu::TextureView; 5],
    pipelines: Pipelines,
    /// A textured material's image, sampler, and factors (group 2).
    material_layout: wgpu::BindGroupLayout,
    /// Fills group 1 for the masked shadow pipeline, which reads no guides.
    empty_group: wgpu::BindGroup,
    /// Repeating, trilinear sampling for base-color images.
    textured_sampler: wgpu::Sampler,
    post: Option<Output>,
    stars: wgpu::Buffer,
    star_count: u32,
    pub dynamic_lit: Stream,
    pub glow: Stream,
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
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Unorm8x4
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
                texture_entry(1, d2, wgpu::TextureSampleType::Depth),
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
            ],
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse photo frame"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("verse sun shadow"),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
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
        );

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse photo frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse photo frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            }],
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse photo shadow"),
            bind_group_layouts: &[Some(&frame_layout)],
            immediate_size: 0,
        });
        let guide_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse photo guides"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
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
            &crate::gles::wgsl(include_str!("photo.wgsl"), capability.gles),
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
        let [pcss, detail] = capability.tier_constants();
        let constants = [
            ("DIRECT", if direct { 1.0 } else { 0.0 }),
            ("DEBUG", f64::from(debug)),
            pcss,
            detail,
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
            lit: make(
                &layout,
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
            capability,
            output_format,
            frame,
            space_ready: false,
            guide_layout,
            scene_layout,
            scene_group,
            frame_group,
            shadow,
            shadow_compare,
            linear_clamp,
            linear_repeat,
            probes,
            probe_version: None,
            sky_textures,
            pipelines,
            material_layout,
            empty_group,
            textured_sampler,
            post,
            stars: star_buffer,
            star_count: 0,
            dynamic_lit: Stream::new(device, "verse dynamic lit"),
            glow: Stream::new(device, "verse glow"),
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
        let stars = sky::parse_stars(include_bytes!("../../assets/lagrange/stars.bin"))?;
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
        self.upload_textured_with(device, queue, scene, merged, wgpu::BufferUsages::VERTEX)
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
        )
    }

    fn upload_textured_with(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &TexturedScene,
        merged: &textured::Merged,
        vertex_usage: wgpu::BufferUsages,
    ) -> TexturedGpu {
        let max = device.limits().max_texture_dimension_2d;
        let images: Vec<wgpu::TextureView> = scene
            .images
            .iter()
            .enumerate()
            .map(|(i, image)| {
                let levels = textured::mip_chain(image, scene.mask_cutoff(i), max);
                upload_levels(device, queue, &image.name, &levels)
            })
            .collect();
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
                let view = material.image.map_or(&white, |i| &images[i]);
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
                usage: wgpu::BufferUsages::INDEX,
            }),
            batches: merged.batches.clone(),
            materials: scene.materials.clone(),
            groups,
        }
    }

    /// A figure's batches in drawing order, never culled.
    fn figure_order(figure: Option<&TexturedGpu>, view: crate::render::View) -> Vec<usize> {
        figure.map_or_else(Vec::new, |gpu| {
            textured::draw_order(&gpu.batches, &gpu.materials, view.eye, |_| true)
        })
    }

    /// The textured cells this frame draws, in drawing order.
    fn textured_order(textured: Option<&TexturedGpu>, view: crate::render::View) -> Vec<usize> {
        textured.map_or_else(Vec::new, |gpu| {
            textured::draw_order(&gpu.batches, &gpu.materials, view.eye, |b| {
                textured::in_frustum(b.min, b.max, view.view_proj)
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
        let guide_groups = [0, 1].map(|k| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse photo guides"),
                layout: &self.guide_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&adapt[k]),
                }],
            })
        });
        PhotoTargets {
            guide_groups,
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
        view: crate::render::View,
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
        view: crate::render::View,
        sky: &Sky,
        world: Batches<'_>,
    ) {
        self.update_probes(device, queue, sky.probes.as_deref());
        let [width, height] = targets.size;
        let camera = &sky.camera;
        let exposure = camera.exposure();
        let sun = sky.sun_dir.normalize();
        let (light, texel, reach) = shadow_fit(sun, sky.shadow_center, sky.shadow_half);
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
        let frame = Frame {
            view_proj: reversed.to_cols_array_2d(),
            inv_view_proj: reversed.inverse().to_cols_array_2d(),
            light: light.to_cols_array_2d(),
            eye: view.eye.extend(exposure).to_array(),
            sun: sun.extend(sky.sun_illuminance).to_array(),
            sun_disc: [
                sky.sun_angular_radius,
                sky.sun_illuminance / solid,
                sky.sun_visible,
                texel,
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
            earth_light: [earth_light[0], earth_light[1], earth_light[2], reach * 2.0],
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
        };
        queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));

        self.encode_shadow(encoder, world.lit, [world.textured, world.figure]);
        let order = Self::textured_order(world.textured, view);
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
            // The texture written last frame holds the newest adaptation.
            pass.set_bind_group(1, &targets.guide_groups[targets.parity() ^ 1], &[]);
            for which in [Pass::Opaque, Pass::Masked] {
                self.draw_textured(&mut pass, world.textured, &order, which);
                self.draw_textured(&mut pass, world.figure, &figure_order, which);
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

    /// The sun or key light's shadow map, from every lit triangle and every
    /// opaque or masked textured cell.
    fn encode_shadow(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        world_lit: (&wgpu::Buffer, u32),
        textured: [Option<&TexturedGpu>; 2],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("verse sun shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow,
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
        pass.set_bind_group(0, &self.frame_group, &[]);
        pass.set_pipeline(&self.pipelines.shadow);
        for (buffer, count) in [
            world_lit,
            (&self.dynamic_lit.buffer, self.dynamic_lit.count),
        ] {
            if count > 0 {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..count, 0..1);
            }
        }
        for gpu in textured.into_iter().flatten() {
            pass.set_vertex_buffer(0, gpu.vertices.slice(..));
            pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
            for masked in [false, true] {
                if masked {
                    pass.set_pipeline(&self.pipelines.textured_shadow_masked);
                    pass.set_bind_group(1, &self.empty_group, &[]);
                } else {
                    pass.set_pipeline(&self.pipelines.textured_shadow);
                }
                for batch in &gpu.batches {
                    let cell_pass = gpu.materials[batch.material].alpha.pass();
                    if textured::raster(cell_pass, false).shadow != Some(masked) {
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
        view: crate::render::View,
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
        }
        let lit = neon.key.filter(|_| {
            world.lit.1 > 0
                || self.dynamic_lit.count > 0
                || world.textured.is_some()
                || world.figure.is_some()
        });
        if let Some(key) = &lit {
            // Pre-exposed lux: the stage's lines stay at unit exposure.
            let exposure = super::exposure(key.ev100);
            let probes = key.probes();
            self.update_probes(device, queue, Some(&probes));
            let (light, texel, reach) =
                shadow_fit(key.dir.normalize(), key.shadow_center, key.shadow_half);
            let rim = key.rim_illuminance * exposure;
            uniform.light = light.to_cols_array_2d();
            uniform.sun = key
                .dir
                .normalize()
                .extend(key.illuminance * exposure)
                .to_array();
            uniform.sun_disc = [key.angular_radius, 0.0, 0.0, texel];
            uniform.earth = key
                .rim_dir
                .normalize()
                .extend(key.rim_angular_radius)
                .to_array();
            uniform.earth_light = [rim, rim, rim, reach * 2.0];
            uniform.probe_origin = probes.origin.extend(probes.cell).to_array();
            uniform.probe_dims = [
                probes.dims[0] as f32,
                probes.dims[1] as f32,
                probes.dims[2] as f32,
                1.0,
            ];
        }
        queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&uniform));
        if lit.is_some() {
            self.encode_shadow(encoder, world.lit, [world.textured, world.figure]);
        }
        let figure_order = if lit.is_some() {
            Self::figure_order(world.figure, view)
        } else {
            Vec::new()
        };
        let order = if lit.is_some() {
            Self::textured_order(world.textured, view)
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
                grade: Grade::STAGE,
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
pub(crate) enum Stage<'a> {
    Space(&'a Sky),
    Neon(&'a Neon),
}

/// An orthographic shadow map along `toward` (unit, toward the light) fitted
/// to a cube of half extent `half` about `center`, snapped to whole texels so
/// edges do not shimmer as the camera moves. Returns the light matrix, the
/// texel size, and the depth reach, in meters.
fn shadow_fit(toward: Vec3, center: Vec3, half: f32) -> (Mat4, f32, f32) {
    let half = half.max(1.0);
    let reach = half * 2.0;
    let up = if toward.y.abs() > 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let look = Mat4::look_to_rh(Vec3::ZERO, -toward, up);
    let texel = 2.0 * half / SHADOW_SIZE as f32;
    let center = look.transform_point3(center);
    let snapped = Vec3::new(
        (center.x / texel).round() * texel,
        (center.y / texel).round() * texel,
        center.z,
    );
    let proj = Mat4::orthographic_rh(
        snapped.x - half,
        snapped.x + half,
        snapped.y - half,
        snapped.y + half,
        -snapped.z - reach,
        -snapped.z + reach,
    );
    (proj * look, texel, reach)
}

/// Reversed depth (Reed 2015): map depth d to 1 − d so a float buffer keeps
/// micrometer precision near the camera and across the scene.
fn reversed_depth() -> Mat4 {
    Mat4::from_cols(
        glam::Vec4::X,
        glam::Vec4::Y,
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
    )
}

/// The retained geometry a physical frame draws.
pub(crate) struct Batches<'a> {
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
        ],
    })
}

/// IEEE 754 binary16 bits for `value`, rounding to nearest.
#[must_use]
pub(crate) fn half(value: f32) -> u16 {
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

/// Uploads RGBA8 sRGB mip levels, largest first, as one texture.
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
            decode(include_bytes!("../../assets/lagrange/earth_day.png"), false)?,
            srgb,
        ),
        upload_mipped(
            device,
            queue,
            "earth clouds",
            decode(
                include_bytes!("../../assets/lagrange/earth_clouds.png"),
                true,
            )?,
            r8,
        ),
        upload_mipped(
            device,
            queue,
            "earth water",
            decode(
                include_bytes!("../../assets/lagrange/earth_water.png"),
                true,
            )?,
            r8,
        ),
        upload_mipped(
            device,
            queue,
            "moon albedo",
            decode(
                include_bytes!("../../assets/lagrange/moon_albedo.png"),
                false,
            )?,
            srgb,
        ),
        upload_mipped(
            device,
            queue,
            "milky way",
            decode(include_bytes!("../../assets/lagrange/milky_way.png"), false)?,
            srgb,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

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
            [("PCSS", 0.0), ("DETAIL", 0.0)]
        );
        for tier in [Tier::Medium, Tier::High] {
            assert_eq!(
                capability(tier).tier_constants(),
                [("PCSS", 1.0), ("DETAIL", 1.0)]
            );
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
        let (w, h, px) =
            decode(include_bytes!("../../assets/lagrange/earth_day.png"), false).unwrap();
        assert_eq!((w, h, px.len()), (2048, 1024, 2048 * 1024 * 4));
        let (w, h, px) = decode(
            include_bytes!("../../assets/lagrange/earth_clouds.png"),
            true,
        )
        .unwrap();
        assert_eq!((w, h, px.len()), (2048, 1024, 2048 * 1024));
        let (w, h, _) = decode(
            include_bytes!("../../assets/lagrange/moon_albedo.png"),
            false,
        )
        .unwrap();
        assert_eq!((w, h), (2048, 1024));
        let stars = sky::parse_stars(include_bytes!("../../assets/lagrange/stars.bin")).unwrap();
        assert!(stars.len() > 9_000);
    }
}
