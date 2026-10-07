//! The output pass every floating-point frame ends with: bloom, exposure
//! adaptation, the color grade, and the tone curve, from a scene texture into
//! a display texture.
//!
//! The physical path and the summoning chamber share it, so a light reads the
//! same in both. The grade's scene-referred part (white balance, exposure
//! offset, saturation, contrast, and color gain) is a 3D lookup table baked
//! on the CPU from a [`Grade`]; the curve and the display ceiling follow it
//! analytically. Every pass is a full-screen triangle with fragment work
//! only, so the chain runs on WebGL2 wherever a float target renders.
//!
//! The output pipeline writes linear display values. An sRGB target encodes
//! them; on a linear surface, `render::Present` encodes them, as it does for
//! every other pass.

use bytemuck::{Pod, Zeroable};
use verse_engine::lighting::{Curve, GRADE_LUT_FLOOR, GRADE_LUT_SIZE, Grade, grade_lut_range};
use wgpu::util::DeviceExt;

/// The most bloom mip levels, from half resolution down.
pub const BLOOM_LEVELS: u32 = 6;

/// The uniform block of `post.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Post {
    source: [f32; 4],
    look: [f32; 4],
    lens: [f32; 4],
    balance: [f32; 4],
    adapt: [f32; 4],
    /// x the output ceiling; y the grade table's floor; z one over its span
    /// in stops; w its edge length.
    output: [f32; 4],
    /// The view under the water ([`Look::water`]).
    water: [[f32; 4]; 2],
}

impl Post {
    /// The fixed block of a bloom pass.
    fn bloom(source_size: [u32; 2], karis: bool) -> Self {
        Self {
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
            output: table_output(1.0),
            water: [[0.0; 4]; 2],
        }
    }
}

/// The output block's fields: the ceiling, then the table's shaper.
fn table_output(ceiling: f32) -> [f32; 4] {
    [
        ceiling,
        GRADE_LUT_FLOOR,
        1.0 / grade_lut_range(),
        GRADE_LUT_SIZE as f32,
    ]
}

/// What the output pass does to one frame.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// Fraction of bloom energy mixed in.
    pub bloom: f32,
    /// Local exposure strength; 0 turns it off.
    pub local: f32,
    pub grain: f32,
    pub vignette: f32,
    /// Lateral chromatic aberration at the corners, pixels.
    pub fringe: f32,
    pub ghosts: f32,
    /// Whether exposure adapts to the metered scene.
    pub auto: bool,
    /// The adaptation gain's range.
    pub gain_min: f32,
    pub gain_max: f32,
    /// The zone's grade, its curve, and the display ceiling.
    pub grade: Grade,
    /// Seconds, for adaptation speed and grain.
    pub time: f32,
    /// The view under the water (`crate::water::under`, Medium and High):
    /// the waterline across the screen ([`crate::water::under::line`]),
    /// then how far the view under it wavers (screen fractions), 1 for
    /// the meniscus where the line crosses, and two zeros. All zero for a
    /// view with no water.
    pub water: [[f32; 4]; 2],
}

/// Pipelines and the grade table for one device and pair of formats.
pub struct Output {
    hdr: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    adapt: wgpu::RenderPipeline,
    output: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    lut: wgpu::Texture,
    lut_view: wgpu::TextureView,
    /// The grade whose table the texture holds.
    baked: Option<Grade>,
    last_time: Option<f32>,
}

/// Size-dependent bloom targets and bind groups for one scene texture.
pub struct OutputTargets {
    bloom_views: Vec<wgpu::TextureView>,
    /// The adapted-luminance textures the adapt pass writes in turn.
    adapt: [wgpu::TextureView; 2],
    /// Bind groups for each bloom down pass, each up pass, the two adapt
    /// passes, and the two output passes (one per adapt texture).
    down: Vec<wgpu::BindGroup>,
    up: Vec<wgpu::BindGroup>,
    adapt_groups: [wgpu::BindGroup; 2],
    output_groups: [wgpu::BindGroup; 2],
    frame_post: wgpu::Buffer,
    size: [u32; 2],
    /// The adapt texture the next frame writes; the other holds the newest
    /// adapted luminance.
    pub parity: usize,
}

/// The two 1×1 adapted-luminance textures the adapt pass alternates between.
pub fn adapt_textures(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> [wgpu::TextureView; 2] {
    [0, 1].map(|_| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("verse adapt"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    })
}

/// Additive blending for the bloom up passes.
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

impl Output {
    /// Pipelines that read an `hdr` scene and write `output`.
    pub fn new(
        device: &wgpu::Device,
        hdr: wgpu::TextureFormat,
        output: wgpu::TextureFormat,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let tex = |binding, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: float,
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let d2 = wgpu::TextureViewDimension::D2;
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
                tex(1, d2),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex(3, d2),
                tex(4, d2),
                tex(5, wgpu::TextureViewDimension::D3),
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse post linear clamp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let n = GRADE_LUT_SIZE as u32;
        // Half floats: filterable on every backend, WebGL2 included, and
        // signed, since the table holds changes to its input.
        let lut = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verse grade table"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let lut_view = lut.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            hdr,
            down: make("verse bloom down", "fs_down", hdr, None),
            up: make("verse bloom up", "fs_up", hdr, Some(ADDITIVE)),
            adapt: make("verse adapt", "fs_adapt", hdr, None),
            output: make("verse output", "fs_output", output, None),
            layout,
            sampler,
            lut,
            lut_view,
            baked: None,
            last_time: None,
        }
    }

    /// Bloom targets and bind groups for `scene`, a `width` × `height`
    /// texture in this pass's floating-point format, metering into `adapt`.
    pub fn targets(
        &self,
        device: &wgpu::Device,
        scene: &wgpu::TextureView,
        adapt: &[wgpu::TextureView; 2],
        width: u32,
        height: u32,
    ) -> OutputTargets {
        let (bw, bh) = ((width / 2).max(1), (height / 2).max(1));
        let levels = BLOOM_LEVELS.min(32 - bw.min(bh).leading_zeros()).max(1);
        let bloom = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verse bloom"),
            size: wgpu::Extent3d {
                width: bw,
                height: bh,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.hdr,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let bloom_views: Vec<_> = (0..levels)
            .map(|level| {
                bloom.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let bloom_all = bloom.create_view(&wgpu::TextureViewDescriptor::default());
        let frame_post = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse post frame"),
            size: std::mem::size_of::<Post>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = |uniform: &wgpu::Buffer,
                     source: &wgpu::TextureView,
                     bloom: &wgpu::TextureView,
                     adapted: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse post"),
                layout: &self.layout,
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
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(bloom),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(adapted),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.lut_view),
                    },
                ],
            })
        };
        let fixed = |source_size: [u32; 2], karis: bool| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("verse bloom pass"),
                contents: bytemuck::bytes_of(&Post::bloom(source_size, karis)),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };
        let mip_size = |level: u32| [(bw >> level).max(1), (bh >> level).max(1)];
        let mut down = Vec::new();
        for level in 0..levels {
            let (source, size, karis) = if level == 0 {
                (scene, [width, height], true)
            } else {
                (&bloom_views[level as usize - 1], mip_size(level - 1), false)
            };
            down.push(group(&fixed(size, karis), source, &adapt[0], &adapt[1]));
        }
        let mut up = Vec::new();
        for level in 0..levels.saturating_sub(1) {
            let source = &bloom_views[level as usize + 1];
            up.push(group(
                &fixed(mip_size(level + 1), false),
                source,
                &adapt[0],
                &adapt[1],
            ));
        }
        let adapt_groups = [
            group(&frame_post, scene, &bloom_all, &adapt[1]),
            group(&frame_post, scene, &bloom_all, &adapt[0]),
        ];
        let output_groups = [
            group(&frame_post, scene, &bloom_all, &adapt[0]),
            group(&frame_post, scene, &bloom_all, &adapt[1]),
        ];
        OutputTargets {
            bloom_views,
            adapt: adapt.clone(),
            down,
            up,
            adapt_groups,
            output_groups,
            frame_post,
            size: [width, height],
            parity: 0,
        }
    }

    /// Rebakes the grade table when the grade's scene-referred part changed.
    fn bake(&mut self, queue: &wgpu::Queue, grade: &Grade) {
        if self.baked.is_some_and(|baked| baked.same_table(grade)) {
            return;
        }
        let texels: Vec<u16> = grade
            .bake()
            .into_iter()
            .flat_map(|texel| texel.map(super::gpu::half))
            .collect();
        let n = GRADE_LUT_SIZE as u32;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.lut,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 8),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
        );
        self.baked = Some(*grade);
    }

    /// Bloom, exposure adaptation, and the graded output transform from the
    /// scene texture `targets` was made for into `output`.
    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut OutputTargets,
        look: &Look,
    ) {
        // An invalid grade would bake a table of NaNs; draw ungraded instead.
        let grade = if look.grade.validate().is_ok() {
            look.grade
        } else {
            Grade::NEUTRAL
        };
        self.bake(queue, &grade);
        let [width, height] = targets.size;
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
            balance: [1.0, 1.0, 1.0, 1.0 - (-dt * 1.5).exp()],
            adapt: [
                0.18,
                look.gain_min,
                look.gain_max,
                f32::from(u8::from(grade.curve == Curve::HueShoulder)),
            ],
            output: table_output(grade.ceiling),
            water: look.water,
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
                &self.down,
                group,
                clear,
            );
        }
        for level in (0..targets.up.len()).rev() {
            pass(
                encoder,
                &targets.bloom_views[level],
                &self.up,
                &targets.up[level],
                wgpu::LoadOp::Load,
            );
        }
        // Adapt into one texture while the output reads it this frame.
        let write = targets.parity;
        pass(
            encoder,
            &targets.adapt[write],
            &self.adapt,
            &targets.adapt_groups[write],
            clear,
        );
        pass(
            encoder,
            output,
            &self.output,
            &targets.output_groups[write],
            clear,
        );
        targets.parity ^= 1;
    }
}
