use crate::svg::SvgRenderer;
use bytemuck::{Pod, Zeroable};
use std::cell::RefCell;
use std::collections::HashMap;
use web_time::Instant;
use wgpu::util::DeviceExt;
use wgpui_core::scene::{
    GpuImageQuad, GpuLine, GpuQuad, GpuTextQuad, MESH_EDGE_FLAG_SELECTED,
    MESH_EDGE_FLAG_SILHOUETTE, MeshPrimitive, Scene,
};
use wgpui_core::{Hsla, Point, Size};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    scale: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct GpuMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// Cache key for GPU-uploaded SVG textures.
#[derive(Clone, Eq, PartialEq, Hash)]
struct SvgTextureKey {
    /// Hash of SVG data
    data_hash: u64,
    /// Physical width
    width: u32,
    /// Physical height
    height: u32,
}

/// GPU resources for a rasterized SVG.
struct SvgGpuResources {
    #[expect(
        dead_code,
        reason = "texture handle retained to preserve bind-group backing lifetime"
    )]
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Prepared SVG for rendering.
struct PreparedSvg {
    instance_buffer: wgpu::Buffer,
    cache_key: SvgTextureKey,
}

/// Prepared mesh batch for rendering.
struct PreparedMeshBatch {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    edge_overlay_buffer: Option<wgpu::Buffer>,
    edge_overlay_count: u32,
}

/// Prepared layer data for rendering.
struct PreparedLayer {
    quad_buffer: Option<wgpu::Buffer>,
    quad_count: u32,
    text_buffer: Option<wgpu::Buffer>,
    text_count: u32,
    line_buffer: Option<wgpu::Buffer>,
    line_count: u32,
    mesh_batches: Vec<PreparedMeshBatch>,
    mesh_count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct RenderMetrics {
    pub layer_count: usize,
    pub quad_instances: u32,
    pub line_instances: u32,
    pub text_instances: u32,
    pub svg_instances: u32,
    pub mesh_primitives: u32,
    pub mesh_vertices: u32,
    pub mesh_triangles: u32,
    pub mesh_draw_calls: u32,
    pub mesh_skipped: u32,
    pub mesh_edge_overlays: u32,
    pub draw_calls: u32,
    pub prepare_cpu_ms: f64,
    pub render_cpu_ms: f64,
}

pub struct Renderer {
    quad_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    #[expect(
        dead_code,
        reason = "bind group layout retained for renderer extension points"
    )]
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    image_bind_group_layout: wgpu::BindGroupLayout,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    quad_instance_buffer: Option<wgpu::Buffer>,
    text_instance_buffer: Option<wgpu::Buffer>,
    quad_count: u32,
    text_count: u32,
    // Layer-based rendering
    prepared_layers: Vec<PreparedLayer>,
    // SVG rendering
    svg_rasterizer: SvgRenderer,
    svg_texture_cache: HashMap<SvgTextureKey, SvgGpuResources>,
    prepared_svgs: Vec<PreparedSvg>,
    image_sampler: wgpu::Sampler,
    metrics: RefCell<RenderMetrics>,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Quad Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/quad.wgsl").into()),
        });

        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });

        let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image.wgsl").into()),
        });

        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                viewport: [800.0, 600.0],
                scale: 1.0,
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Atlas Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Atlas Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Atlas Bind Group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Image bind group layout (for SVGs and images)
        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Image Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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

        // Sampler for images (bilinear filtering for smooth scaling)
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Quad Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Quad Pipeline"),
            layout: Some(&quad_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 48,
                            shader_location: 4,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 52, // corner_radius
                            shader_location: 5,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 56,
                            shader_location: 6,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 64,
                            shader_location: 7,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Line pipeline for rendering anti-aliased lines
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/line.wgsl").into()),
        });

        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Line Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuLine>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0, // start
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8, // end
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 16, // width
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24, // color (after width + padding)
                            shader_location: 3,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuTextQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 48,
                            shader_location: 4,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 56,
                            shader_location: 5,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Image pipeline (for SVGs and other textured quads)
        let image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Image Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &image_bind_group_layout],
                push_constant_ranges: &[],
            });

        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Pipeline"),
            layout: Some(&image_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &image_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuImageQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // position: vec2<f32>
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        // size: vec2<f32>
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        // uv: vec4<f32>
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        // tint: vec4<f32>
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 3,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &image_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Pipeline"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mesh_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuMeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            quad_pipeline,
            text_pipeline,
            line_pipeline,
            image_pipeline,
            mesh_pipeline,
            uniform_buffer,
            uniform_bind_group,
            uniform_bind_group_layout,
            image_bind_group_layout,
            atlas_texture,
            atlas_bind_group,
            quad_instance_buffer: None,
            text_instance_buffer: None,
            quad_count: 0,
            text_count: 0,
            prepared_layers: Vec::new(),
            svg_rasterizer: SvgRenderer::new(),
            svg_texture_cache: HashMap::new(),
            prepared_svgs: Vec::new(),
            image_sampler,
            metrics: RefCell::new(RenderMetrics::default()),
        }
    }

    pub fn resize(&self, queue: &wgpu::Queue, size: Size, scale: f32) {
        // Viewport must be in PHYSICAL pixels since gpu_quads/gpu_text_quads
        // scale positions to physical. Otherwise NDC calculation is wrong.
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms {
                viewport: [size.width * scale, size.height * scale],
                scale,
                _padding: 0.0,
            }]),
        );
    }

    pub fn update_atlas(&self, queue: &wgpu::Queue, data: &[u8], size: u32) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Prepare scene for rendering.
    /// The scale_factor is used to convert both quads and text from logical to physical pixels at the GPU boundary.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        scale_factor: f32,
    ) {
        let prepare_start = Instant::now();
        let mut quad_instances: u32 = 0;
        let mut text_instances: u32 = 0;
        let mut line_instances: u32 = 0;
        let mut mesh_primitives: u32 = 0;
        let mut mesh_vertices: u32 = 0;
        let mut mesh_triangles: u32 = 0;
        let mut mesh_skipped: u32 = 0;
        let mut mesh_edge_overlays: u32 = 0;

        // Prepare layers in order
        self.prepared_layers.clear();
        let layers = scene.layers();

        for layer in layers {
            // Get quads (no longer merging curve quads - curves render as lines)
            let quads = scene.gpu_quads_for_layer(layer, scale_factor);
            let text_quads = scene.gpu_text_quads_for_layer(layer, scale_factor);
            let lines = scene.curve_lines_for_layer(layer, scale_factor);
            let meshes = scene.mesh_primitives_for_layer(layer);

            let quad_buffer = if quads.is_empty() {
                None
            } else {
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Quad Instance Buffer Layer {}", layer)),
                        contents: bytemuck::cast_slice(&quads),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            };

            let text_buffer = if text_quads.is_empty() {
                None
            } else {
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Text Instance Buffer Layer {}", layer)),
                        contents: bytemuck::cast_slice(&text_quads),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            };

            let line_buffer = if lines.is_empty() {
                None
            } else {
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Line Instance Buffer Layer {}", layer)),
                        contents: bytemuck::cast_slice(&lines),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            };

            quad_instances += quads.len() as u32;
            text_instances += text_quads.len() as u32;
            line_instances += lines.len() as u32;
            mesh_primitives += meshes.len() as u32;
            let mut mesh_batches = Vec::<PreparedMeshBatch>::new();
            for mesh in &meshes {
                if should_skip_mesh(mesh) {
                    mesh_skipped = mesh_skipped.saturating_add(1);
                    continue;
                }
                mesh_vertices = mesh_vertices.saturating_add(mesh.vertices.len() as u32);
                mesh_triangles = mesh_triangles.saturating_add((mesh.indices.len() / 3) as u32);

                let gpu_vertices = mesh_to_gpu_vertices(mesh, scale_factor);
                if gpu_vertices.is_empty() {
                    mesh_skipped = mesh_skipped.saturating_add(1);
                    continue;
                }
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Mesh Vertex Buffer Layer {}", layer)),
                    contents: bytemuck::cast_slice(&gpu_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Mesh Index Buffer Layer {}", layer)),
                    contents: bytemuck::cast_slice(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let edge_overlays = mesh_edge_overlay_lines(mesh, &gpu_vertices, scale_factor);
                mesh_edge_overlays = mesh_edge_overlays.saturating_add(edge_overlays.len() as u32);
                let edge_overlay_buffer = if edge_overlays.is_empty() {
                    None
                } else {
                    Some(
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Mesh Edge Overlay Buffer Layer {}", layer)),
                            contents: bytemuck::cast_slice(&edge_overlays),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                    )
                };
                mesh_batches.push(PreparedMeshBatch {
                    vertex_buffer,
                    index_buffer,
                    index_count: mesh.indices.len() as u32,
                    edge_overlay_count: edge_overlays.len() as u32,
                    edge_overlay_buffer,
                });
            }

            self.prepared_layers.push(PreparedLayer {
                quad_buffer,
                quad_count: quads.len() as u32,
                text_buffer,
                text_count: text_quads.len() as u32,
                line_buffer,
                line_count: lines.len() as u32,
                mesh_count: mesh_batches.len() as u32,
                mesh_batches,
            });
        }

        // Keep legacy buffers for backward compatibility (unused now)
        self.quad_instance_buffer = None;
        self.quad_count = 0;
        self.text_instance_buffer = None;
        self.text_count = 0;

        // Prepare SVG quads
        self.prepared_svgs.clear();
        for svg_quad in scene.svg_quads() {
            // Calculate physical size
            let physical_width = (svg_quad.bounds.size.width * scale_factor).ceil() as u32;
            let physical_height = (svg_quad.bounds.size.height * scale_factor).ceil() as u32;

            if physical_width == 0 || physical_height == 0 {
                continue;
            }

            // Hash the SVG data for cache key
            let data_hash = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                svg_quad.svg_data.hash(&mut hasher);
                hasher.finish()
            };

            let cache_key = SvgTextureKey {
                data_hash,
                width: physical_width,
                height: physical_height,
            };

            // Check if we need to rasterize and upload
            if !self.svg_texture_cache.contains_key(&cache_key) {
                // Rasterize the SVG
                if let Some(rasterized) = self.svg_rasterizer.rasterize(
                    &svg_quad.svg_data,
                    svg_quad.bounds.size.width as u32,
                    svg_quad.bounds.size.height as u32,
                    scale_factor,
                ) {
                    // Create GPU texture
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("SVG Texture"),
                        size: wgpu::Extent3d {
                            width: rasterized.width,
                            height: rasterized.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    // Upload pixel data
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &rasterized.pixels,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(rasterized.width * 4),
                            rows_per_image: Some(rasterized.height),
                        },
                        wgpu::Extent3d {
                            width: rasterized.width,
                            height: rasterized.height,
                            depth_or_array_layers: 1,
                        },
                    );

                    // Create bind group
                    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("SVG Bind Group"),
                        layout: &self.image_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&texture_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                            },
                        ],
                    });

                    self.svg_texture_cache.insert(
                        cache_key.clone(),
                        SvgGpuResources {
                            texture,
                            bind_group,
                        },
                    );
                }
            }

            // Create instance buffer if texture was uploaded successfully
            if self.svg_texture_cache.contains_key(&cache_key) {
                // Create GPU quad data - convert HSLA to RGBA for GPU tinting
                let tint = svg_quad
                    .tint
                    .map_or([1.0, 1.0, 1.0, 1.0], |color| color.to_rgba());

                let gpu_quad = GpuImageQuad {
                    position: [
                        svg_quad.bounds.origin.x * scale_factor,
                        svg_quad.bounds.origin.y * scale_factor,
                    ],
                    size: [physical_width as f32, physical_height as f32],
                    uv: [0.0, 0.0, 1.0, 1.0], // Full texture
                    tint,
                };

                let instance_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("SVG Instance Buffer"),
                        contents: bytemuck::bytes_of(&gpu_quad),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                self.prepared_svgs.push(PreparedSvg {
                    instance_buffer,
                    cache_key,
                });
            }
        }

        let mut metrics = self.metrics.borrow_mut();
        metrics.layer_count = self.prepared_layers.len();
        metrics.quad_instances = quad_instances;
        metrics.text_instances = text_instances;
        metrics.line_instances = line_instances;
        metrics.svg_instances = self.prepared_svgs.len() as u32;
        metrics.mesh_primitives = mesh_primitives;
        metrics.mesh_vertices = mesh_vertices;
        metrics.mesh_triangles = mesh_triangles;
        metrics.mesh_skipped = mesh_skipped;
        metrics.mesh_edge_overlays = mesh_edge_overlays;
        metrics.mesh_draw_calls = 0;
        metrics.prepare_cpu_ms = prepare_start.elapsed().as_secs_f64() * 1_000.0;
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        self.render_with_clear(encoder, view, wgpu::Color::BLACK);
    }

    pub fn render_with_clear(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        clear_color: wgpu::Color,
    ) {
        let render_start = Instant::now();
        let mut draw_calls: u32 = 0;
        let mut mesh_draw_calls: u32 = 0;
        let mut mesh_skipped: u32 = 0;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Render layers in order: quads -> meshes -> edge overlays/lines -> text.
        for layer in &self.prepared_layers {
            // Render quads first (background)
            if let Some(buffer) = &layer.quad_buffer {
                render_pass.set_pipeline(&self.quad_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..4, 0..layer.quad_count);
                draw_calls += 1;
            }

            // Render prepared mesh batches in-layer.
            if layer.mesh_count > 0 {
                render_pass.set_pipeline(&self.mesh_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                for batch in &layer.mesh_batches {
                    if batch.index_count == 0 {
                        mesh_skipped = mesh_skipped.saturating_add(1);
                        continue;
                    }
                    render_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    render_pass
                        .set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..batch.index_count, 0, 0..1);
                    draw_calls = draw_calls.saturating_add(1);
                    mesh_draw_calls = mesh_draw_calls.saturating_add(1);
                }
            }

            // Render mesh edge overlays and regular lines on top of mesh fill.
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            for batch in &layer.mesh_batches {
                if let Some(buffer) = &batch.edge_overlay_buffer {
                    if batch.edge_overlay_count == 0 {
                        continue;
                    }
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..4, 0..batch.edge_overlay_count);
                    draw_calls = draw_calls.saturating_add(1);
                }
            }
            if let Some(buffer) = &layer.line_buffer {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..4, 0..layer.line_count);
                draw_calls = draw_calls.saturating_add(1);
            }

            // Render text on top.
            if let Some(buffer) = &layer.text_buffer {
                render_pass.set_pipeline(&self.text_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..4, 0..layer.text_count);
                draw_calls = draw_calls.saturating_add(1);
            }
        }

        // Render SVGs (each SVG has its own texture, so we draw them one at a time)
        if !self.prepared_svgs.is_empty() {
            render_pass.set_pipeline(&self.image_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            for prepared in &self.prepared_svgs {
                if let Some(resources) = self.svg_texture_cache.get(&prepared.cache_key) {
                    render_pass.set_bind_group(1, &resources.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, prepared.instance_buffer.slice(..));
                    render_pass.draw(0..4, 0..1);
                    draw_calls += 1;
                }
            }
        }

        drop(render_pass);

        let mut metrics = self.metrics.borrow_mut();
        metrics.draw_calls = draw_calls;
        metrics.mesh_draw_calls = mesh_draw_calls;
        metrics.mesh_skipped = metrics.mesh_skipped.saturating_add(mesh_skipped);
        metrics.render_cpu_ms = render_start.elapsed().as_secs_f64() * 1_000.0;
    }

    /// Clear the SVG texture cache.
    /// Call this periodically to free unused textures.
    pub fn clear_svg_cache(&mut self) {
        self.svg_texture_cache.clear();
        self.svg_rasterizer.clear_cache();
    }

    /// Get the number of cached SVG textures.
    pub fn svg_cache_size(&self) -> usize {
        self.svg_texture_cache.len()
    }

    pub fn render_metrics(&self) -> RenderMetrics {
        self.metrics.borrow().clone()
    }
}

fn mesh_to_gpu_vertices(mesh: &MeshPrimitive, scale_factor: f32) -> Vec<GpuMeshVertex> {
    mesh.vertices
        .iter()
        .map(|vertex| GpuMeshVertex {
            position: [
                vertex.position[0] * scale_factor,
                vertex.position[1] * scale_factor,
                vertex.position[2] * scale_factor,
            ],
            normal: stable_shading_normal(vertex.normal),
            color: vertex.color,
        })
        .collect()
}

fn stable_shading_normal(normal: [f32; 3]) -> [f32; 3] {
    if !normal.iter().all(|component| component.is_finite()) {
        return [0.0, 0.0, 1.0];
    }
    let length_sq = normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];
    if length_sq <= f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    let inv = length_sq.sqrt().recip();
    quantized_unit_normal([normal[0] * inv, normal[1] * inv, normal[2] * inv])
}

fn quantized_unit_normal(normal: [f32; 3]) -> [f32; 3] {
    const SCALE: f32 = 1_000_000.0;
    [
        (normal[0] * SCALE).round() / SCALE,
        (normal[1] * SCALE).round() / SCALE,
        (normal[2] * SCALE).round() / SCALE,
    ]
}

fn mesh_edge_overlay_lines(
    mesh: &MeshPrimitive,
    gpu_vertices: &[GpuMeshVertex],
    scale_factor: f32,
) -> Vec<GpuLine> {
    mesh.edges
        .iter()
        .filter_map(|edge| {
            let start = gpu_vertices.get(edge.start as usize)?;
            let end = gpu_vertices.get(edge.end as usize)?;
            let start_point = Point::new(start.position[0], start.position[1]);
            let end_point = Point::new(end.position[0], end.position[1]);
            let avg_nz = ((start.normal[2] + end.normal[2]) * 0.5).abs();
            let flagged_silhouette = edge.flags & MESH_EDGE_FLAG_SILHOUETTE != 0;
            let selected = edge.flags & MESH_EDGE_FLAG_SELECTED != 0;
            let inferred_silhouette = avg_nz < 0.45;
            let is_silhouette = flagged_silhouette || inferred_silhouette;
            let width = if selected {
                2.05
            } else if is_silhouette {
                1.35
            } else {
                0.95
            };
            let color = if selected {
                Hsla::new(0.14, 0.82, 0.66, 0.96)
            } else if is_silhouette {
                Hsla::new(0.58, 0.30, 0.90, 0.92)
            } else {
                Hsla::new(0.60, 0.12, 0.72, 0.88)
            };
            Some(GpuLine::new(
                start_point,
                end_point,
                width,
                color,
                scale_factor,
            ))
        })
        .collect()
}

fn should_skip_mesh(mesh: &MeshPrimitive) -> bool {
    mesh.vertices.is_empty() || mesh.indices.is_empty()
}

#[cfg(test)]
mod tests {
    use super::{mesh_edge_overlay_lines, mesh_to_gpu_vertices, should_skip_mesh};
    use wgpui_core::scene::{
        MESH_EDGE_FLAG_SELECTED, MESH_EDGE_FLAG_SILHOUETTE, MeshEdge, MeshPrimitive, MeshVertex,
    };

    #[test]
    fn mesh_vertex_conversion_scales_positions_deterministically() {
        let mesh = MeshPrimitive::new(
            vec![
                MeshVertex::new([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], [0.1, 0.2, 0.3, 1.0]),
                MeshVertex::new([4.0, 5.0, 6.0], [0.0, 1.0, 0.0], [0.4, 0.5, 0.6, 1.0]),
                MeshVertex::new([7.0, 8.0, 9.0], [1.0, 0.0, 0.0], [0.7, 0.8, 0.9, 1.0]),
            ],
            vec![0, 1, 2],
        );
        let first = mesh_to_gpu_vertices(&mesh, 2.0);
        let second = mesh_to_gpu_vertices(&mesh, 2.0);
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].position, [2.0, 4.0, 6.0]);
        assert_eq!(first[1].position, [8.0, 10.0, 12.0]);
        assert_eq!(first[2].position, [14.0, 16.0, 18.0]);
    }

    #[test]
    fn mesh_vertex_conversion_stabilizes_shading_normals() {
        let mesh = MeshPrimitive::new(
            vec![
                MeshVertex::new([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.2, 0.3, 0.4, 1.0]),
                MeshVertex::new([1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [0.2, 0.3, 0.4, 1.0]),
                MeshVertex::new([2.0, 2.0, 2.0], [f32::NAN, 1.0, 0.0], [0.2, 0.3, 0.4, 1.0]),
            ],
            vec![0, 1, 2],
        );
        let first = mesh_to_gpu_vertices(&mesh, 1.0);
        let second = mesh_to_gpu_vertices(&mesh, 1.0);
        assert_eq!(first, second);
        assert_eq!(first[0].normal, [1.0, 0.0, 0.0]);
        assert_eq!(first[1].normal, [0.0, 0.0, 1.0]);
        assert_eq!(first[2].normal, [0.0, 0.0, 1.0]);
        assert!(
            first
                .iter()
                .all(|vertex| vertex.normal.iter().all(|component| component.is_finite()))
        );
    }

    #[test]
    fn edge_overlay_emphasizes_selected_edges() {
        let mesh = MeshPrimitive::new(
            vec![
                MeshVertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.8, 0.8, 0.8, 1.0]),
                MeshVertex::new([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.8, 0.8, 0.8, 1.0]),
                MeshVertex::new([10.0, 10.0, 0.0], [0.0, 0.0, 0.0], [0.8, 0.8, 0.8, 1.0]),
            ],
            vec![0, 1, 2],
        )
        .with_edges(vec![
            MeshEdge::new(0, 1).with_flags(MESH_EDGE_FLAG_SELECTED),
            MeshEdge::new(1, 2).with_flags(MESH_EDGE_FLAG_SILHOUETTE),
            MeshEdge::new(2, 0),
        ]);
        let gpu_vertices = mesh_to_gpu_vertices(&mesh, 1.0);
        let overlays = mesh_edge_overlay_lines(&mesh, &gpu_vertices, 1.0);
        assert_eq!(overlays.len(), 3);
        assert!(overlays[0].width > overlays[1].width);
        assert!(overlays[1].width >= overlays[2].width);
    }

    #[test]
    fn empty_mesh_payload_is_marked_for_fallback_skip() {
        let mesh = MeshPrimitive::new(Vec::new(), Vec::new());
        assert!(should_skip_mesh(&mesh));
    }
}
