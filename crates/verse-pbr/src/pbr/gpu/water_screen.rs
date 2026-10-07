//! The physical renderer's half of [`crate::water::screen`] (W5): on Medium
//! and High, the planar mirror's pipelines and target, the scene's color
//! and depth copies, and the water pipeline that reads them. Low creates
//! none of it; its water keeps the two halves inside the scene pass.

use super::{DEPTH, Frame, Pass, depth_state, lit_layout, textured, textured_layout};
use crate::water::screen::{DEPTH_COPY, Plan, copy_source};

/// Pipelines and buffers made once for a tier that copies.
pub(super) struct WaterScreen {
    pub plan: Plan,
    copy_layout: wgpu::BindGroupLayout,
    copy: wgpu::RenderPipeline,
    /// The zone's water in one draw over the copies (`fs_water_screen`).
    pub water: wgpu::RenderPipeline,
    /// The mirror's own frame uniform and the scene group over it.
    pub mirror_frame: wgpu::Buffer,
    pub mirror_scene_group: wgpu::BindGroup,
    pub mirror: MirrorPipelines,
}

/// The opaque scene's pipelines, single sampled, for the mirror: faces wind
/// the other way in a mirror, so culled ones cull the other side, and the
/// screen-space terms (which the mirror never traces) are off.
pub(super) struct MirrorPipelines {
    pub daylight: wgpu::RenderPipeline,
    pub lit: wgpu::RenderPipeline,
    pub legacy: wgpu::RenderPipeline,
    /// Opaque then masked, single-sided then double-sided.
    pub textured: [[wgpu::RenderPipeline; 2]; 2],
}

/// What [`WaterScreen::new`] builds from.
pub(super) struct Parts<'a> {
    pub module: &'a wgpu::ShaderModule,
    pub constants: &'a [(&'a str, f64)],
    pub scene_format: wgpu::TextureFormat,
    pub samples: u32,
    pub sky_layout: &'a wgpu::PipelineLayout,
    pub guide_layout: &'a wgpu::PipelineLayout,
    pub textured_layout: &'a wgpu::PipelineLayout,
    pub legacy: wgpu::VertexBufferLayout<'a>,
    pub water: wgpu::RenderPipeline,
    pub mirror_scene_group: wgpu::BindGroup,
    pub mirror_frame: wgpu::Buffer,
}

impl WaterScreen {
    pub fn new(device: &wgpu::Device, plan: Plan, parts: Parts<'_>) -> Self {
        let options = wgpu::PipelineCompilationOptions {
            constants: parts.constants,
            ..Default::default()
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: parts.scene_format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        vs: &str,
                        fs: &str,
                        buffers: &[wgpu::VertexBufferLayout<'_>],
                        cull: Option<wgpu::Face>,
                        depth: wgpu::DepthStencilState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: parts.module,
                    entry_point: Some(vs),
                    compilation_options: options.clone(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: cull,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: parts.module,
                    entry_point: Some(fs),
                    compilation_options: options.clone(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let write = depth_state(true, wgpu::CompareFunction::GreaterEqual);
        let buffers = textured_layout();
        let textured = [Pass::Opaque, Pass::Masked].map(|pass| {
            [false, true].map(|double_sided| {
                let raster = textured::raster(pass, double_sided);
                pipeline(
                    "verse water mirror textured",
                    parts.textured_layout,
                    "vs_textured",
                    raster.entry,
                    &buffers,
                    raster.cull,
                    depth_state(raster.depth_write, wgpu::CompareFunction::GreaterEqual),
                )
            })
        });
        let mirror = MirrorPipelines {
            daylight: pipeline(
                "verse water mirror sky",
                parts.sky_layout,
                "vs_fullscreen",
                "fs_daylight",
                &[],
                None,
                depth_state(false, wgpu::CompareFunction::Always),
            ),
            lit: pipeline(
                "verse water mirror lit",
                parts.guide_layout,
                "vs_lit",
                "fs_lit",
                &[lit_layout()],
                None,
                write.clone(),
            ),
            legacy: pipeline(
                "verse water mirror faces",
                parts.guide_layout,
                "vs_legacy",
                "fs_legacy",
                std::slice::from_ref(&parts.legacy),
                None,
                write,
            ),
            textured,
        };

        let copy_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse water depth copy"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: parts.samples > 1,
                    },
                    count: None,
                },
            ],
        });
        let copy_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse water depth copy"),
            source: wgpu::ShaderSource::Wgsl(copy_source(parts.samples).into()),
        });
        let copy_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse water depth copy"),
            bind_group_layouts: &[Some(&copy_layout)],
            immediate_size: 0,
        });
        let copy = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("verse water depth copy"),
            layout: Some(&copy_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &copy_module,
                entry_point: Some("vs_copy"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &copy_module,
                entry_point: Some("fs_copy"),
                compilation_options: Default::default(),
                targets: &[Some(DEPTH_COPY.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            plan,
            copy_layout,
            copy,
            water: parts.water,
            mirror_frame: parts.mirror_frame,
            mirror_scene_group: parts.mirror_scene_group,
            mirror,
        }
    }

    /// The copies and the mirror for a view `width` by `height`: what
    /// [`super::PhotoTargets`] keeps. `depth` is the scene's depth buffer,
    /// which must allow texture binding; `frame` the frame uniform.
    #[allow(clippy::too_many_arguments)]
    pub fn targets(
        &self,
        device: &wgpu::Device,
        scene_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        depth: &wgpu::TextureView,
        frame: &wgpu::Buffer,
        guide_layout: &wgpu::BindGroupLayout,
        adapt: &wgpu::TextureView,
        white: &wgpu::TextureView,
    ) -> WaterTargets {
        let texture = |label, format, [w, h]: [u32; 2], usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let sampled = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let copy = texture(
            "verse water scene copy",
            scene_format,
            [width, height],
            sampled | wgpu::TextureUsages::COPY_DST,
        );
        let depth_copy = texture(
            "verse water depth copy",
            DEPTH_COPY,
            [width, height],
            sampled,
        )
        .create_view(&Default::default());
        let mirror_size = self.plan.mirror_size(width, height).unwrap_or([1, 1]);
        let mirror = texture("verse water mirror", scene_format, mirror_size, sampled)
            .create_view(&Default::default());
        let mirror_depth = texture(
            "verse water mirror depth",
            DEPTH,
            mirror_size,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let copy_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse water depth copy"),
            layout: &self.copy_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        });
        let mirror_guides = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse water mirror guides"),
            layout: guide_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(adapt),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(white),
                },
            ],
        });
        WaterTargets {
            copy_view: copy.create_view(&Default::default()),
            copy,
            depth_copy,
            copy_group,
            mirror,
            mirror_depth,
            mirror_size,
            mirror_guides,
        }
    }

    /// Writes the depth copy from the scene's depth buffer.
    pub fn encode_copy(&self, encoder: &mut wgpu::CommandEncoder, targets: &WaterTargets) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("verse water depth copy"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.depth_copy,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.copy);
        pass.set_bind_group(0, &targets.copy_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The copies and the mirror at one view size.
pub(super) struct WaterTargets {
    /// The opaque scene, resolved: refraction and the reflection's march
    /// read it.
    pub copy: wgpu::Texture,
    pub copy_view: wgpu::TextureView,
    /// Each pixel's view depth, m.
    pub depth_copy: wgpu::TextureView,
    copy_group: wgpu::BindGroup,
    pub mirror: wgpu::TextureView,
    pub mirror_depth: wgpu::TextureView,
    pub mirror_size: [u32; 2],
    /// The mirror pass's group 1: the adapted luminance and a white
    /// screen-space term.
    pub mirror_guides: wgpu::BindGroup,
}

/// The mirror's frame: `main` seen from `mirror`, at `size` pixels, with
/// no screen-space terms, copies, or mirror of its own.
pub(super) fn mirror_frame(
    main: &Frame,
    mirror: &crate::water::screen::Mirror,
    size: [u32; 2],
) -> Frame {
    let mut frame = *main;
    frame.view_proj = mirror.view_proj.to_cols_array_2d();
    frame.inv_view_proj = mirror.unclipped.inverse().to_cols_array_2d();
    frame.eye = [mirror.eye.x, mirror.eye.y, mirror.eye.z, main.eye[3]];
    let f = main.view_forward;
    frame.view_forward = [f[0], -f[1], f[2], f[3]];
    frame.viewport = [
        size[0] as f32,
        size[1] as f32,
        1.0 / size[0] as f32,
        1.0 / size[1] as f32,
    ];
    frame.water_screen = [0.0; 4];
    frame.water_mirror = [0.0; 4];
    frame
}
