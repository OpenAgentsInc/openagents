//! GPU resources and frame encoding for the physical path.
//!
//! One frame runs these passes: a sun shadow map; a floating-point scene pass
//! that draws the sky at infinity, catalogue stars, the Sun, Earth, and Moon in
//! distance order, lit surfaces, legacy geometry, guide lines, and glows; a
//! bloom mip chain; exposure adaptation; the output transform into the
//! surface; and the HUD on top. When the adapter cannot render a floating-point
//! target, the scene pass tone-maps in place and post-processing is skipped.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::{GlowVertex, LitVertex, Neon, ProbeGrid, Sky, sky};

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: u32 = 2048;
const BLOOM_LEVELS: u32 = 6;

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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Post {
    source: [f32; 4],
    look: [f32; 4],
    lens: [f32; 4],
    balance: [f32; 4],
    adapt: [f32; 4],
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
        Self { hdr, samples }
    }
}

struct Pipelines {
    shadow: wgpu::RenderPipeline,
    lit: wgpu::RenderPipeline,
    background: wgpu::RenderPipeline,
    stars: wgpu::RenderPipeline,
    bodies: wgpu::RenderPipeline,
    flare: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    legacy: wgpu::RenderPipeline,
    wide: wgpu::RenderPipeline,
    floor: wgpu::RenderPipeline,
}

struct PostPipelines {
    layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    adapt: wgpu::RenderPipeline,
    output: wgpu::RenderPipeline,
}

/// Size-dependent targets for the physical path.
pub(crate) struct PhotoTargets {
    size: [u32; 2],
    msaa: Option<wgpu::TextureView>,
    scene: wgpu::TextureView,
    depth: wgpu::TextureView,
    bloom_views: Vec<wgpu::TextureView>,
    bloom_all: wgpu::TextureView,
    adapt: [wgpu::TextureView; 2],
    /// Bind groups for each bloom down pass, each up pass, the two adapt
    /// passes, and the two output passes (one per adapt texture).
    down: Vec<wgpu::BindGroup>,
    up: Vec<wgpu::BindGroup>,
    adapt_groups: [wgpu::BindGroup; 2],
    output_groups: [wgpu::BindGroup; 2],
    /// The adapted luminance for guides, indexed by the texture last written.
    guide_groups: [wgpu::BindGroup; 2],
    frame_post: wgpu::Buffer,
    parity: usize,
}

impl PhotoTargets {
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
}

/// GPU state for the physical path, created on the first physical frame.
pub(crate) struct Photo {
    capability: Capability,
    output_format: wgpu::TextureFormat,
    frame: wgpu::Buffer,
    /// The frame seen through the floor, for the neon stage's reflection.
    mirror_frame: wgpu::Buffer,
    mirror_group: wgpu::BindGroup,
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
    post: Option<PostPipelines>,
    stars: wgpu::Buffer,
    star_count: u32,
    pub dynamic_lit: Stream,
    pub glow: Stream,
    last_time: Option<f32>,
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
        let mirror_frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse photo mirror frame"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mirror_group = self::scene_group(
            device,
            &scene_layout,
            &mirror_frame,
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
        let module = shader(device, "verse photo", include_str!("photo.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse photo"),
            bind_group_layouts: &[Some(&scene_layout)],
            immediate_size: 0,
        });
        let debug = std::env::var("VERSE_PHOTO_DEBUG")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let constants = [
            ("DIRECT", if direct { 1.0 } else { 0.0 }),
            ("DEBUG", f64::from(debug)),
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
        let pipelines = Pipelines {
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
            floor: make(
                &layout,
                "verse neon floor",
                "vs_floor",
                Some("fs_floor"),
                &[],
                triangles,
                depth_state(true, wgpu::CompareFunction::GreaterEqual),
                Some(PREMULTIPLIED),
                samples,
            ),
        };
        let _ = LEGACY;
        let post = capability
            .hdr
            .map(|hdr| post_pipelines(device, hdr, output_format));
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
            mirror_frame,
            mirror_group,
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
            post,
            stars: star_buffer,
            star_count: 0,
            dynamic_lit: Stream::new(device, "verse dynamic lit"),
            glow: Stream::new(device, "verse glow"),
            last_time: None,
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
        for (frame, space) in [(&self.frame, true), (&self.mirror_frame, false)] {
            let group = scene_group(
                device,
                &self.scene_layout,
                frame,
                &self.shadow,
                &self.shadow_compare,
                &self.probes,
                &self.linear_clamp,
                &self.sky_textures,
                &self.linear_repeat,
            );
            if space {
                self.scene_group = group;
            } else {
                self.mirror_group = group;
            }
        }
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
        let (bw, bh) = ((width / 2).max(1), (height / 2).max(1));
        let levels = BLOOM_LEVELS.min(32 - bw.min(bh).leading_zeros()).max(1);
        let bloom = texture("verse bloom", scene_format, bw, bh, 1, levels, sampled);
        let bloom_views: Vec<_> = (0..levels)
            .map(|level| {
                bloom.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let bloom_all = bloom.create_view(&Default::default());
        let adapt = [0, 1].map(|_| {
            texture("verse adapt", scene_format, 1, 1, 1, 1, sampled)
                .create_view(&Default::default())
        });
        let frame_post = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse post frame"),
            size: std::mem::size_of::<Post>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let empty = || Vec::new();
        let (mut down, mut up) = (empty(), empty());
        let mut adapt_groups = None;
        let mut output_groups = None;
        if let Some(post) = &self.post {
            let group = |uniform: &wgpu::Buffer,
                         source: &wgpu::TextureView,
                         bloom: &wgpu::TextureView,
                         adapted: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("verse post"),
                    layout: &post.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(source),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.linear_clamp),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(bloom),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(adapted),
                        },
                    ],
                })
            };
            let fixed = |source_size: [u32; 2], karis: bool| {
                let post = Post {
                    source: [
                        1.0 / source_size[0] as f32,
                        1.0 / source_size[1] as f32,
                        f32::from(u8::from(karis)),
                        0.0,
                    ],
                    look: [0.0; 4],
                    lens: [0.0; 4],
                    balance: [1.0, 1.0, 1.0, 0.0],
                    adapt: [0.0; 4],
                };
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("verse bloom pass"),
                    contents: bytemuck::bytes_of(&post),
                    usage: wgpu::BufferUsages::UNIFORM,
                })
            };
            let mip_size = |level: u32| [(bw >> level).max(1), (bh >> level).max(1)];
            for level in 0..levels {
                let (source, size, karis) = if level == 0 {
                    (&scene, [width, height], true)
                } else {
                    (&bloom_views[level as usize - 1], mip_size(level - 1), false)
                };
                down.push(group(&fixed(size, karis), source, &adapt[0], &adapt[1]));
            }
            for level in 0..levels.saturating_sub(1) {
                let source = &bloom_views[level as usize + 1];
                up.push(group(
                    &fixed(mip_size(level + 1), false),
                    source,
                    &adapt[0],
                    &adapt[1],
                ));
            }
            adapt_groups = Some([
                group(&frame_post, &scene, &bloom_all, &adapt[1]),
                group(&frame_post, &scene, &bloom_all, &adapt[0]),
            ]);
            output_groups = Some([
                group(&frame_post, &scene, &bloom_all, &adapt[0]),
                group(&frame_post, &scene, &bloom_all, &adapt[1]),
            ]);
        }
        let placeholder = || {
            // Direct mode never samples post groups; bind the scene layout's
            // simplest group so the struct stays uniform.
            let dummy = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse post unused"),
                layout: &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[],
                }),
                entries: &[],
            });
            [dummy.clone(), dummy]
        };
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
            bloom_views,
            bloom_all,
            adapt,
            down,
            up,
            adapt_groups: adapt_groups.unwrap_or_else(placeholder),
            output_groups: output_groups.unwrap_or_else(placeholder),
            frame_post,
            parity: 0,
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
                self.encode_neon(queue, encoder, output, targets, view, neon, world)
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
        // Sun shadow: an orthographic map fitted to the station, snapped to
        // whole texels so edges do not shimmer as the camera moves.
        let sun = sky.sun_dir.normalize();
        let half = sky.shadow_half.max(1.0);
        let reach = half * 2.0;
        let up = if sun.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let look = Mat4::look_to_rh(Vec3::ZERO, -sun, up);
        let texel = 2.0 * half / SHADOW_SIZE as f32;
        let center = look.transform_point3(sky.shadow_center);
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
        let light = proj * look;
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
        };
        queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));

        // Shadow pass.
        {
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
                world.lit,
                (&self.dynamic_lit.buffer, self.dynamic_lit.count),
            ] {
                if count > 0 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..count, 0..1);
                }
            }
        }

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
            let order = if sky.moon.distance > sky.earth.distance {
                [2, 1]
            } else {
                [1, 2]
            };
            for kind in order {
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
            pass.set_bind_group(1, &targets.guide_groups[targets.parity ^ 1], &[]);
            pass.set_pipeline(&self.pipelines.legacy);
            for (buffer, count) in world.faces {
                if count > 0 {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..count, 0..1);
                }
            }
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
                balance: white_balance(camera.white_balance),
                gain_min: 2f32.powf(camera.ev100 - camera.ev_max),
                gain_max: 2f32.powf(camera.ev100 - camera.ev_min),
                hue_preserving: false,
                time: sky.time,
            },
        );
    }

    /// The neon stage: the city mirrored in a polished floor, the floor,
    /// faces, emissive lines, and the post chain with a hue-preserving curve.
    #[allow(clippy::too_many_arguments)]
    fn encode_neon(
        &mut self,
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
        let mirror = reversed * Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
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
            field: [
                neon.field[0],
                neon.field[1],
                neon.field[2],
                neon.reflectivity,
            ],
        };
        queue.write_buffer(
            &self.frame,
            0,
            bytemuck::bytes_of(&frame(reversed, neon.line_width, 1.0)),
        );
        // Reflections are softer: the floor's micro-roughness spreads them.
        queue.write_buffer(
            &self.mirror_frame,
            0,
            bytemuck::bytes_of(&frame(mirror, neon.line_width * 2.2, 2.0)),
        );
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
            let draw_world = |pass: &mut wgpu::RenderPass<'_>| {
                pass.set_pipeline(&self.pipelines.legacy);
                for (buffer, count) in world.faces {
                    if count > 0 {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..count, 0..1);
                    }
                }
                pass.set_pipeline(&self.pipelines.wide);
                for (buffer, count) in world.lines {
                    if count >= 2 {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..6, 0..count / 2);
                    }
                }
            };
            pass.set_bind_group(1, &targets.guide_groups[0], &[]);
            if neon.reflectivity > 0.0 {
                pass.set_bind_group(0, &self.mirror_group, &[]);
                draw_world(&mut pass);
            }
            pass.set_bind_group(0, &self.scene_group, &[]);
            pass.set_pipeline(&self.pipelines.floor);
            pass.draw(0..6, 0..1);
            draw_world(&mut pass);
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
                balance: [1.0; 3],
                gain_min: 1.0,
                gain_max: 1.0,
                hue_preserving: true,
                time: neon.time,
            },
        );
    }

    /// Bloom, exposure adaptation, and the output transform into `output`.
    fn post_chain(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut PhotoTargets,
        look: &Look,
    ) {
        let [width, height] = targets.size;
        if let Some(post) = &self.post {
            let dt = self
                .last_time
                .map_or(0.0, |last| (look.time - last).clamp(0.0, 0.5));
            self.last_time = Some(look.time);
            let levels = targets.bloom_views.len();
            let uniform = Post {
                source: [
                    1.0 / width as f32,
                    1.0 / height as f32,
                    0.0,
                    1.0 / levels as f32,
                ],
                look: [look.bloom, look.local, look.grain, look.vignette],
                lens: [
                    look.fringe,
                    look.ghosts,
                    look.time,
                    f32::from(u8::from(look.auto)),
                ],
                balance: [
                    look.balance[0],
                    look.balance[1],
                    look.balance[2],
                    1.0 - (-dt * 1.5).exp(),
                ],
                adapt: [
                    0.18,
                    look.gain_min,
                    look.gain_max,
                    f32::from(u8::from(look.hue_preserving)),
                ],
            };
            queue.write_buffer(&targets.frame_post, 0, bytemuck::bytes_of(&uniform));
            let pass = |encoder: &mut wgpu::CommandEncoder,
                        view: &wgpu::TextureView,
                        pipeline: &wgpu::RenderPipeline,
                        group: &wgpu::BindGroup,
                        load: wgpu::LoadOp<wgpu::Color>| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("verse post"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load,
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
                pass.draw(0..3, 0..1);
            };
            let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
            for (level, group) in targets.down.iter().enumerate() {
                pass(
                    encoder,
                    &targets.bloom_views[level],
                    &post.down,
                    group,
                    clear,
                );
            }
            for level in (0..targets.up.len()).rev() {
                pass(
                    encoder,
                    &targets.bloom_views[level],
                    &post.up,
                    &targets.up[level],
                    wgpu::LoadOp::Load,
                );
            }
            // Adapt into one texture while the output reads it this frame.
            let write = targets.parity;
            pass(
                encoder,
                &targets.adapt[write],
                &post.adapt,
                &targets.adapt_groups[write],
                clear,
            );
            pass(
                encoder,
                output,
                &post.output,
                &targets.output_groups[write],
                clear,
            );
            targets.parity ^= 1;
        }
        let _ = &targets.bloom_all;
    }
}

/// What the post chain does to one frame.
struct Look {
    bloom: f32,
    local: f32,
    grain: f32,
    vignette: f32,
    fringe: f32,
    ghosts: f32,
    auto: bool,
    balance: [f32; 3],
    gain_min: f32,
    gain_max: f32,
    hue_preserving: bool,
    time: f32,
}

/// The scene a physical frame shows.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Stage<'a> {
    Space(&'a Sky),
    Neon(&'a Neon),
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

fn post_pipelines(
    device: &wgpu::Device,
    hdr: wgpu::TextureFormat,
    output: wgpu::TextureFormat,
) -> PostPipelines {
    let module = shader(device, "verse post", include_str!("post.wgsl"));
    let float = wgpu::TextureSampleType::Float { filterable: true };
    let tex = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: float,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("verse post"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            tex(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            tex(3),
            tex(4),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("verse post"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let make = |label, fs, format, blend| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    PostPipelines {
        down: make("verse bloom down", "fs_down", hdr, None),
        up: make("verse bloom up", "fs_up", hdr, Some(ADDITIVE)),
        adapt: make("verse adapt", "fs_adapt", hdr, None),
        output: make("verse output", "fs_output", output, None),
        layout,
    }
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
