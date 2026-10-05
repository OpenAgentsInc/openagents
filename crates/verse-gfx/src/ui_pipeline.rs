//! GPU pipeline for admitted atlas and UI batches.
use crate::ui::{Atlas, UiVertex};
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The UI pipeline: screen-space quads sampling the glyph atlas, alpha
/// blended, drawn last with no depth test.
pub fn ui_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    samples: u32,
    atlas: &Atlas,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::BindGroup,
    wgpu::Buffer,
) {
    let (pipeline, photo, group, screen, _) =
        ui_pipeline_with_texture(device, queue, format, samples, atlas);
    (pipeline, photo, group, screen)
}

/// Writes `atlas` again into `texture`, the atlas texture
/// [`ui_pipeline_with_texture`] made, when it still has the texture's size.
/// Returns false when the size changed and the pipeline must be rebuilt.
pub fn write_atlas(queue: &wgpu::Queue, texture: &wgpu::Texture, atlas: &Atlas) -> bool {
    if texture.width() != atlas.width || texture.height() != atlas.height {
        return false;
    }
    upload(queue, texture, atlas);
    true
}

fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, atlas: &Atlas) {
    let rgba = atlas.rgba.clone().unwrap_or_else(|| {
        atlas
            .pixels
            .iter()
            .flat_map(|a| [255, 255, 255, *a])
            .collect()
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas.width * 4),
            rows_per_image: Some(atlas.height),
        },
        extent(atlas.width, atlas.height),
    );
}

/// [`ui_pipeline`], and the atlas texture for [`write_atlas`].
#[allow(clippy::type_complexity)]
pub fn ui_pipeline_with_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    samples: u32,
    atlas: &Atlas,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::BindGroup,
    wgpu::Buffer,
    wgpu::Texture,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse glyph atlas"),
        size: extent(atlas.width, atlas.height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    upload(queue, &texture, atlas);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("verse glyph sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let screen = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("verse ui screen"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("verse ui"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("verse ui"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: screen.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("verse ui"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("verse ui"),
        source: wgpu::ShaderSource::Wgsl(crate::ui::SHADER.into()),
    });
    let pipeline = |samples: u32, depth: bool| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("verse ui"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth.then(|| wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    (
        pipeline(samples, true),
        pipeline(1, false),
        bind_group,
        screen,
        texture,
    )
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
