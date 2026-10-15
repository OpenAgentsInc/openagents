//! Screen-space drawing of a particle liquid (`render.wgsl`): sphere
//! depth and thickness splats, a bilateral depth blur, and a shading pass
//! that composites the liquid over a rendered background.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub const SHADER: &str = include_str!("render.wgsl");

/// Depth values at or beyond this are empty.
const FAR: f32 = 1.0e5;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    inv_proj: [[f32; 4]; 4],
    inv_view: [[f32; 4]; 4],
    size: [f32; 2],
    radius: f32,
    far: f32,
    sun: [f32; 3],
    blur_direction: u32,
    absorb: [f32; 3],
    blur_scale: f32,
    tint: [f32; 3],
    thickness_scale: f32,
}

/// How to look at and colour the liquid.
#[derive(Clone, Copy, Debug)]
pub struct LiquidView {
    pub view: Mat4,
    pub proj: Mat4,
    /// Drawn sphere radius, m (about the particle spacing).
    pub radius: f32,
    /// Direction toward the sun, world space.
    pub sun: Vec3,
    /// Absorption per metre for red, green and blue (water: more red).
    pub absorb: Vec3,
    /// The colour thick liquid takes on.
    pub tint: Vec3,
    /// Scales summed sphere thickness to an optical depth, about 1/3 for
    /// a liquid drawn with radius equal to the spacing.
    pub thickness_scale: f32,
}

impl LiquidView {
    /// Clear water seen through `view` and `proj`.
    #[must_use]
    pub fn water(view: Mat4, proj: Mat4, radius: f32) -> Self {
        Self {
            view,
            proj,
            radius,
            sun: Vec3::new(0.4, 0.8, 0.3).normalize(),
            absorb: Vec3::new(9.0, 2.6, 1.6),
            tint: Vec3::new(0.25, 0.55, 0.7),
            thickness_scale: 0.5,
        }
    }
}

/// Targets and pipelines for one output size.
pub struct ScreenSpaceLiquid {
    width: u32,
    height: u32,
    sprite_depth: wgpu::RenderPipeline,
    sprite_thickness: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    shade: wgpu::RenderPipeline,
    view_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    shade_layout: wgpu::BindGroupLayout,
    depth: [wgpu::TextureView; 2],
    thickness: wgpu::TextureView,
    zbuffer: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

fn target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl ScreenSpaceLiquid {
    /// `output` is the colour format of the target `draw` writes.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        output: wgpu::TextureFormat,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("liquid surface"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("liquid view"),
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
        let texture = |binding, filterable| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("liquid blur"),
            entries: &[texture(0, false)],
        });
        let shade_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("liquid shade"),
            entries: &[
                texture(0, false),
                texture(1, false),
                texture(2, true),
                texture(3, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = |groups: &[&wgpu::BindGroupLayout]| {
            let groups: Vec<Option<&wgpu::BindGroupLayout>> =
                groups.iter().map(|g| Some(*g)).collect();
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("liquid"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            })
        };
        let sprite_layout = layout(&[&view_layout]);
        let instance = Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            }],
        });
        let sprite = |fragment: &str,
                      format: wgpu::TextureFormat,
                      blend: Option<wgpu::BlendState>,
                      depth: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&sprite_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("sprite_vs"),
                    compilation_options: Default::default(),
                    buffers: std::slice::from_ref(&instance),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: depth.then(|| wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let sprite_depth = sprite("depth_fs", wgpu::TextureFormat::R32Float, None, true);
        let sprite_thickness = sprite(
            "thickness_fs",
            wgpu::TextureFormat::R16Float,
            Some(wgpu::BlendState {
                color: add,
                alpha: add,
            }),
            false,
        );
        let screen = |fragment: &str, layout: &wgpu::PipelineLayout, format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("screen_vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let blur = screen(
            "blur_fs",
            &layout(&[&view_layout, &blur_layout]),
            wgpu::TextureFormat::R32Float,
        );
        let shade = screen("shade_fs", &layout(&[&view_layout, &shade_layout]), output);
        Self {
            width,
            height,
            sprite_depth,
            sprite_thickness,
            blur,
            shade,
            view_layout,
            blur_layout,
            shade_layout,
            depth: [0, 1].map(|_| {
                target(
                    device,
                    width,
                    height,
                    wgpu::TextureFormat::R32Float,
                    "liquid depth",
                )
            }),
            thickness: target(
                device,
                width,
                height,
                wgpu::TextureFormat::R16Float,
                "liquid thickness",
            ),
            zbuffer: target(
                device,
                width,
                height,
                wgpu::TextureFormat::Depth32Float,
                "liquid z",
            ),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("liquid background"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
        }
    }

    /// Draw `count` particles (16-byte `xyz_` records in `particles`) over
    /// `background`, hidden where `scene_depth` (view distance, R32Float)
    /// is nearer, into `output`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &LiquidView,
        particles: wgpu::BufferSlice<'_>,
        count: u32,
        background: &wgpu::TextureView,
        scene_depth: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        let uniform = |direction: u32| Uniform {
            view: view.view.to_cols_array_2d(),
            proj: view.proj.to_cols_array_2d(),
            inv_proj: view.proj.inverse().to_cols_array_2d(),
            inv_view: view.view.inverse().to_cols_array_2d(),
            size: [self.width as f32, self.height as f32],
            radius: view.radius,
            far: FAR,
            sun: view.sun.normalize().to_array(),
            blur_direction: direction,
            absorb: view.absorb.to_array(),
            blur_scale: 2.0 * self.height as f32 * view.proj.y_axis.y * 0.5,
            tint: view.tint.to_array(),
            thickness_scale: view.thickness_scale,
        };
        let groups = [0, 1].map(|direction| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("liquid view"),
                contents: bytemuck::bytes_of(&uniform(direction)),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("liquid view"),
                layout: &self.view_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        });
        let source = |texture: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("liquid blur"),
                layout: &self.blur_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture),
                }],
            })
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("liquid depth"),
                color_attachments: &[attachment(&self.depth[0], f64::from(FAR))],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.zbuffer,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.sprite_depth);
            pass.set_bind_group(0, &groups[0], &[]);
            pass.set_vertex_buffer(0, particles);
            pass.draw(0..6, 0..count);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("liquid thickness"),
                color_attachments: &[attachment(&self.thickness, 0.0)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.sprite_thickness);
            pass.set_bind_group(0, &groups[0], &[]);
            pass.set_vertex_buffer(0, particles);
            pass.draw(0..6, 0..count);
        }
        // Two rounds of horizontal then vertical smoothing.
        for round in 0..4 {
            let (from, to) = (round % 2, 1 - round % 2);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("liquid blur"),
                color_attachments: &[attachment(&self.depth[to], f64::from(FAR))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blur);
            pass.set_bind_group(0, &groups[round % 2], &[]);
            pass.set_bind_group(1, &source(&self.depth[from]), &[]);
            pass.draw(0..3, 0..1);
        }
        let shade = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("liquid shade"),
            layout: &self.shade_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.thickness),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(background),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(scene_depth),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("liquid shade"),
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
        pass.set_pipeline(&self.shade);
        pass.set_bind_group(0, &groups[0], &[]);
        pass.set_bind_group(1, &shade, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn attachment(view: &wgpu::TextureView, clear: f64) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color {
                r: clear,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }),
            store: wgpu::StoreOp::Store,
        },
    })
}
