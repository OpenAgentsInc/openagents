//! Owned textured and skeletal rendering for imported worlds.
//!
//! Static geometry is merged by material and uploaded once. Each animated actor
//! updates only a bounded bone palette and placement. The same Verse glyph
//! pipeline draws screen-space overlays after world rendering.
use crate::{
    render::View,
    ui::{Atlas, UiBatch},
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
pub mod chamber;
pub mod characters;
pub mod combat;
pub mod controls;
pub mod lighting;
pub mod original;
pub mod overlay;
pub mod play;
use lighting::{Frame, Lighting};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};
use verse_engine::{
    animation,
    assets::{Pack, Vertex},
};
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct Instance {
    pub actor: Option<u64>,
    pub model: String,
    pub transform: Mat4,
    pub animation: u16,
    pub time: f32,
    pub emission: Vec3,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    joints: [u32; 4],
    weights: [f32; 4],
    tint: [f32; 3],
}
impl From<&Vertex> for GpuVertex {
    fn from(v: &Vertex) -> Self {
        Self {
            position: v.position,
            normal: v.normal,
            uv: v.uv,
            joints: v.joints,
            weights: v.weights,
            tint: [1.0; 3],
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Pose {
    model: [[f32; 4]; 4],
    params: [f32; 4],
    bones: [[[f32; 4]; 4]; 256],
}
struct Batch {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    texture: usize,
    blend: u8,
    emissive: bool,
}
struct Actor {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
}
/// Persistent offscreen renderer; frames come directly from owned GPU passes.
pub struct Renderer {
    #[cfg(feature = "imported-desktop")]
    instance: wgpu::Instance,
    #[cfg(feature = "imported-desktop")]
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pack: Pack,
    width: u32,
    height: u32,
    target: wgpu::Texture,
    target_view: wgpu::TextureView,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    row: u32,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    shadow_views: Vec<wgpu::TextureView>,
    shadow_groups: Vec<wgpu::BindGroup>,
    shadow_buffers: Vec<wgpu::Buffer>,
    shadow_pipeline: wgpu::RenderPipeline,
    pose_layout: wgpu::BindGroupLayout,
    textures: Vec<wgpu::BindGroup>,
    pipelines: Vec<wgpu::RenderPipeline>,
    models: HashMap<String, Vec<Batch>>,
    static_batches: Vec<Batch>,
    actors: Vec<Actor>,
    playback: HashMap<(u64, String), animation::Playback>,
    ui_pipeline: wgpu::RenderPipeline,
    ui_group: wgpu::BindGroup,
    _ui_screen: wgpu::Buffer,
    ui_buffer: wgpu::Buffer,
    pub adapter_name: String,
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage,
    })
}
fn extent(w: u32, h: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    }
}
type Merged = BTreeMap<(usize, u8, bool), (Vec<GpuVertex>, Vec<u32>)>;
fn merge(pack: &Pack, instances: &[Instance]) -> Merged {
    let mut merged = Merged::new();
    for instance in instances {
        let model = &pack.models[&instance.model];
        for s in &model.surfaces {
            let (v, i) = merged.entry((s.texture, s.blend, s.emissive)).or_default();
            let offset = v.len() as u32;
            v.extend(s.vertices.iter().map(|p| {
                let mut v = GpuVertex::from(p);
                v.tint = if s.emissive || s.blend == 3 {
                    (Vec3::from(s.tint) * instance.emission).to_array()
                } else {
                    s.tint
                };
                v.position = instance
                    .transform
                    .transform_point3(p.position.into())
                    .to_array();
                v.normal = instance
                    .transform
                    .transform_vector3(p.normal.into())
                    .normalize_or_zero()
                    .to_array();
                v.joints = [0; 4];
                v.weights = [1.0, 0.0, 0.0, 0.0];
                v
            }));
            i.extend(s.indices.iter().map(|i| i + offset));
        }
    }
    merged
}
fn upload(device: &wgpu::Device, merged: Merged) -> Vec<Batch> {
    merged
        .into_iter()
        .filter(|(_, (_, i))| !i.is_empty())
        .map(|((texture, blend, emissive), (v, i))| Batch {
            vertices: buffer(
                device,
                "imported mesh",
                bytemuck::cast_slice(&v),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                device,
                "imported indices",
                bytemuck::cast_slice(&i),
                wgpu::BufferUsages::INDEX,
            ),
            count: i.len() as u32,
            texture,
            blend,
            emissive,
        })
        .collect()
}
fn make_pose(pack: &Pack, instance: Option<&Instance>) -> Pose {
    let mut pose = Pose {
        model: Mat4::IDENTITY.to_cols_array_2d(),
        params: [0.0; 4],
        bones: [Mat4::IDENTITY.to_cols_array_2d(); 256],
    };
    if let Some(i) = instance {
        pose.model = i.transform.to_cols_array_2d();
        if pack.models[&i.model]
            .source
            .starts_with("verse/procedural/")
        {
            pose.params = [1.0, i.emission.x.clamp(0.0, 1.0), i.time, 0.0];
        }
        if pack.models[&i.model].source.starts_with("verse/particles/") {
            pose.params = [2.0, i.emission.x.clamp(0.0, 1.0), i.time, 0.0];
        }
        if pack.models[&i.model].source.starts_with("verse/ground/") {
            pose.params = [3.0, i.emission.x.clamp(0.0, 1.0), i.time, 0.0];
        }
        if pack.models[&i.model].source.starts_with("verse/ribbon/") {
            pose.params = [4.0, i.emission.x.clamp(0.0, 1.0), i.time, 0.0];
        }
        for (dst, m) in
            pose.bones
                .iter_mut()
                .zip(animation::pose(&pack.models[&i.model], i.animation, i.time))
        {
            *dst = m.to_cols_array_2d();
        }
    }
    pose
}
impl Renderer {
    pub fn new(
        pack: Pack,
        dir: &Path,
        width: u32,
        height: u32,
        atlas: &Atlas,
        static_instances: &[Instance],
    ) -> Result<Self, String> {
        pack.validate()?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Invalid imported viewport".into());
        }
        if static_instances
            .iter()
            .any(|i| !pack.models.contains_key(&i.model) || !i.transform.is_finite())
        {
            return Err("Invalid static placement".into());
        }
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|e| e.to_string())?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Verse imported world"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imported frame"),
            entries: &[
                uniform(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let pose_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imported skeleton"),
            entries: &[uniform(0)],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imported material"),
            entries: &[
                uniform(2),
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
        let frame = buffer(
            &device,
            "imported camera",
            bytemuck::bytes_of(&Frame::zeroed()),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Verse local cube shadows"),
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 24,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_all = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &frame_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_all),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Verse shadow frame"),
            entries: &[uniform(0)],
        });
        let mut shadow_views = Vec::new();
        let mut shadow_groups = Vec::new();
        let mut shadow_buffers = Vec::new();
        for layer in 0..24 {
            shadow_views.push(shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            }));
            let b = buffer(
                &device,
                "Verse shadow camera",
                bytemuck::bytes_of(&Frame::zeroed()),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            shadow_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &shadow_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: b.as_entire_binding(),
                }],
            }));
            shadow_buffers.push(b);
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut textures = Vec::new();
        for t in &pack.textures {
            let bytes = std::fs::read(dir.join(&t.file)).map_err(|e| e.to_string())?;
            if format!("{:x}", sha2::Sha256::digest(&bytes)) != t.sha256 {
                return Err("Texture digest mismatch".into());
            }
            let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
            let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
            let mut pixels = vec![0; reader.output_buffer_size().ok_or("Invalid texture size")?];
            let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
            if info.width != t.width
                || info.height != t.height
                || info.color_type != png::ColorType::Rgba
            {
                return Err("Invalid private RGBA texture".into());
            }
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&t.file),
                size: extent(t.width, t.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(t.width * 4),
                    rows_per_image: Some(t.height),
                },
                extent(t.width, t.height),
            );
            let view = tex.create_view(&Default::default());
            for flags in 0..8 {
                let material = buffer(
                    &device,
                    "Verse imported material",
                    bytemuck::cast_slice(&[(flags / 4) as f32, (flags % 4) as f32, 0.0, 0.0]),
                    wgpu::BufferUsages::UNIFORM,
                );
                textures.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: material.as_entire_binding(),
                        },
                    ],
                }));
            }
        }
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Verse imported WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&texture_layout),
                Some(&pose_layout),
            ],
            immediate_size: 0,
        });
        let mut pipelines = Vec::new();
        for blend in 0..4 {
            pipelines.push(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("Verse textured skin"),layout:Some(&layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<GpuVertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32x4,4=>Float32x4,5=>Float32x3]}]},primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(blend<2),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:match blend{2=>Some(wgpu::BlendState::ALPHA_BLENDING),3=>Some(wgpu::BlendState{color:wgpu::BlendComponent{src_factor:wgpu::BlendFactor::SrcAlpha,dst_factor:wgpu::BlendFactor::One,operation:wgpu::BlendOperation::Add},alpha:wgpu::BlendComponent::OVER}),_=>None},write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None}));
        }
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    Some(&shadow_layout),
                    Some(&texture_layout),
                    Some(&pose_layout),
                ],
                immediate_size: 0,
            });
        let shadow_pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("Verse skinned local shadow"),layout:Some(&shadow_pipeline_layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<GpuVertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32x4,4=>Float32x4,5=>Float32x3]}]},primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:wgpu::DepthBiasState{constant:1,slope_scale:1.0,clamp:0.0}}),multisample:Default::default(),fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("shadow_fs"),compilation_options:Default::default(),targets:&[]}),multiview_mask:None,cache:None});
        let static_batches = upload(&device, merge(&pack, static_instances));
        let mut models = HashMap::new();
        for (name, model) in &pack.models {
            let mut merged = Merged::new();
            for s in &model.surfaces {
                let (v, i) = merged.entry((s.texture, s.blend, s.emissive)).or_default();
                let offset = v.len() as u32;
                v.extend(s.vertices.iter().map(|p| {
                    let mut v = GpuVertex::from(p);
                    v.tint = s.tint;
                    v
                }));
                i.extend(s.indices.iter().map(|i| i + offset));
            }
            models.insert(name.clone(), upload(&device, merged));
        }
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Verse imported capture"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let row = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let (_, ui_pipeline, ui_group, ui_screen) =
            crate::render::ui_pipeline(&device, &queue, format, 1, atlas);
        queue.write_buffer(
            &ui_screen,
            0,
            bytemuck::cast_slice(&[width as f32, height as f32, 0.0, 0.0]),
        );
        let ui_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        use sha2::Digest;
        Ok(Self {
            #[cfg(feature = "imported-desktop")]
            instance,
            #[cfg(feature = "imported-desktop")]
            adapter,
            device,
            queue,
            pack,
            width,
            height,
            target,
            target_view,
            depth,
            readback,
            row,
            frame,
            frame_group,
            shadow_views,
            shadow_groups,
            shadow_buffers,
            shadow_pipeline,
            pose_layout,
            textures,
            pipelines,
            models,
            static_batches,
            actors: vec![],
            ui_pipeline,
            ui_group,
            _ui_screen: ui_screen,
            ui_buffer,
            playback: HashMap::new(),
            adapter_name,
        })
    }
    pub fn draw(
        &mut self,
        view: View,
        instances: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<Vec<u8>, String> {
        if instances.len() > 256
            || instances
                .iter()
                .any(|i| !self.models.contains_key(&i.model) || !i.transform.is_finite())
            || !view.view_proj.is_finite()
        {
            return Err("Invalid imported frame".into());
        }
        let ui_bytes = bytemuck::cast_slice(&ui.vertices);
        if ui_bytes.len() > 4 * 1024 * 1024 {
            return Err("Imported overlay exceeds 4 MiB".into());
        }
        let frame = lighting::frame(view, lighting)?;
        self.queue
            .write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));
        for layer in 0..lighting.lights.len().min(lighting.shadowed).min(4) * 6 {
            let mut shadow_frame = frame;
            shadow_frame.view = frame.shadow[layer];
            self.queue.write_buffer(
                &self.shadow_buffers[layer],
                0,
                bytemuck::bytes_of(&shadow_frame),
            );
        }
        self.queue.write_buffer(&self.ui_buffer, 0, ui_bytes);
        while self.actors.len() <= instances.len() {
            let buffer = buffer(
                &self.device,
                "Verse actor palette",
                bytemuck::bytes_of(&make_pose(&self.pack, None)),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pose_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            self.actors.push(Actor { buffer, group });
        }
        self.queue.write_buffer(
            &self.actors[0].buffer,
            0,
            bytemuck::bytes_of(&make_pose(&self.pack, None)),
        );
        self.playback.retain(|(id, model), _| {
            instances
                .iter()
                .any(|i| i.actor == Some(*id) && i.model == *model)
        });
        let mut adventurer_pose: Option<Pose> = None;
        for (i, instance) in instances.iter().enumerate() {
            let mut palette = make_pose(&self.pack, Some(instance));
            if let Some(id) = instance.actor {
                let bones = self
                    .playback
                    .entry((id, instance.model.clone()))
                    .or_default()
                    .update(
                        &self.pack.models[&instance.model],
                        instance.animation,
                        instance.time,
                        lighting.time,
                    );
                for (dst, bone) in palette.bones.iter_mut().zip(bones) {
                    *dst = bone.to_cols_array_2d();
                }
            }
            if matches!(instance.animation, 1 | 100) {
                // Ground fallen and prone bodies using their posed geometry.
                let model = &self.pack.models[&instance.model];
                let transform = Mat4::from_cols_array_2d(&palette.model);
                let bones: Vec<_> = palette.bones.iter().map(Mat4::from_cols_array_2d).collect();
                let mut lowest = f32::INFINITY;
                for vertex in model.surfaces.iter().flat_map(|s| &s.vertices) {
                    let point = vertex.joints.iter().zip(vertex.weights).fold(
                        Vec3::ZERO,
                        |p, (joint, weight)| {
                            p + bones[*joint as usize].transform_point3(vertex.position.into())
                                * weight
                        },
                    );
                    lowest = lowest.min(transform.transform_point3(point).y);
                }
                let lift = (instance.transform.w_axis.y + 0.05 - lowest).clamp(
                    0.0,
                    model.height.max(0.6) * instance.transform.y_axis.truncate().length(),
                );
                palette.model[3][1] += lift;
            }
            if instance.model == "adventurer" {
                adventurer_pose = Some(palette);
            }
            if instance.model == "bow" {
                if let (Some(parent), Some(hand)) = (
                    adventurer_pose,
                    self.pack.models["adventurer"]
                        .attachments
                        .iter()
                        .find(|a| a.id == 2),
                ) {
                    let transform = Mat4::from_cols_array_2d(&parent.model)
                        * Mat4::from_cols_array_2d(&parent.bones[hand.bone])
                        * Mat4::from_translation(hand.position.into())
                        * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                        * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2);
                    palette.model = transform.to_cols_array_2d();
                }
            }
            self.queue
                .write_buffer(&self.actors[i + 1].buffer, 0, bytemuck::bytes_of(&palette));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for layer in 0..lighting.lights.len().min(lighting.shadowed).min(4) * 6 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Verse cube shadow face"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_views[layer],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_groups[layer], &[]);
            pass.set_bind_group(2, &self.actors[0].group, &[]);
            for batch in self
                .static_batches
                .iter()
                .filter(|b| b.blend < 2 && !b.emissive)
            {
                self.draw_batch(&mut pass, batch);
            }
            for (i, instance) in instances.iter().enumerate() {
                pass.set_bind_group(2, &self.actors[i + 1].group, &[]);
                for batch in self.models[&instance.model]
                    .iter()
                    .filter(|b| b.blend < 2 && !b.emissive)
                {
                    self.draw_batch(&mut pass, batch);
                }
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Verse imported world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            for blend in 0..4 {
                pass.set_pipeline(&self.pipelines[blend]);
                pass.set_bind_group(2, &self.actors[0].group, &[]);
                for batch in self
                    .static_batches
                    .iter()
                    .filter(|b| b.blend == blend as u8)
                {
                    self.draw_batch(&mut pass, batch);
                }
                let mut order: Vec<_> = instances.iter().enumerate().collect();
                if blend == 2 {
                    order.sort_by(|(_, a), (_, b)| {
                        b.transform
                            .w_axis
                            .truncate()
                            .distance_squared(view.eye)
                            .total_cmp(&a.transform.w_axis.truncate().distance_squared(view.eye))
                    });
                }
                for (i, instance) in order {
                    pass.set_bind_group(2, &self.actors[i + 1].group, &[]);
                    for batch in self.models[&instance.model]
                        .iter()
                        .filter(|b| b.blend == blend as u8)
                    {
                        self.draw_batch(&mut pass, batch);
                    }
                }
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Verse imported names and dialogue"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_bind_group(0, &self.ui_group, &[]);
            pass.set_vertex_buffer(0, self.ui_buffer.slice(..));
            pass.draw(0..ui.vertices.len() as u32, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: Some(self.height),
                },
            },
            extent(self.width, self.height),
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| e.to_string())?;
        let mapped = slice.get_mapped_range();
        let mut out = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height as usize {
            out.extend_from_slice(
                &mapped[y * self.row as usize..y * self.row as usize + self.width as usize * 4],
            );
        }
        drop(mapped);
        self.readback.unmap();
        Ok(out)
    }
    fn draw_batch<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, batch: &'a Batch) {
        pass.set_bind_group(
            1,
            &self.textures
                [batch.texture * 8 + usize::from(batch.emissive) * 4 + usize::from(batch.blend)],
            &[],
        );
        pass.set_vertex_buffer(0, batch.vertices.slice(..));
        pass.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..batch.count, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn textured_skin_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("scene.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        assert_eq!(std::mem::size_of::<super::GpuVertex>(), 76);
    }
}

#[cfg(feature = "imported-desktop")]
pub struct WindowPresenter {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
}
#[cfg(feature = "imported-desktop")]
impl Renderer {
    pub fn attach_window(
        &self,
        window: std::sync::Arc<winit::window::Window>,
    ) -> Result<WindowPresenter, String> {
        let size = window.inner_size();
        let surface = self
            .instance
            .create_surface(window)
            .map_err(|e| e.to_string())?;
        let mut config = surface
            .get_default_config(&self.adapter, size.width.max(1), size.height.max(1))
            .ok_or("Unsupported imported scene surface")?;
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&self.device, &config);
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Imported scene presentation"),
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
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader=self.device.create_shader_module(wgpu::ShaderModuleDescriptor {label:Some("Imported window blit"),source:wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed("@group(0) @binding(0) var image:texture_2d<f32>; @group(0) @binding(1) var samp:sampler; struct Out {@builtin(position) pos:vec4<f32>,@location(0) uv:vec2<f32>}; @vertex fn vs(@builtin(vertex_index) i:u32)->Out {let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.)); var o:Out;o.pos=vec4(p[i],0.,1.);o.uv=vec2((p[i].x+1.)*.5,(1.-p[i].y)*.5);return o;} @fragment fn fs(v:Out)->@location(0) vec4<f32> {return textureSample(image,samp,v.uv);}"))});
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        Ok(WindowPresenter {
            surface,
            config,
            pipeline,
            group,
        })
    }
    pub fn present_window(&self, p: &mut WindowPresenter, size: [u32; 2]) -> Result<(), String> {
        if size[0] == 0 || size[1] == 0 {
            return Ok(());
        }
        if p.config.width != size[0] || p.config.height != size[1] {
            p.config.width = size[0];
            p.config.height = size[1];
            p.surface.configure(&self.device, &p.config);
        }
        let texture = match p.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                p.surface.configure(&self.device, &p.config);
                return Ok(());
            }
        };
        let view = texture.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Imported interactive presentation"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&p.pipeline);
            pass.set_bind_group(0, &p.group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        texture.present();
        Ok(())
    }
}
