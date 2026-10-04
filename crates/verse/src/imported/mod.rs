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
mod culling;
pub mod inventory;
pub mod lighting;
pub mod original;
pub mod overlay;
pub mod play;
use lighting::{Frame, Lighting};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    time::Instant,
};
use verse_engine::{
    animation,
    assets::{Pack, Vertex},
};
use wgpu::util::DeviceExt;

pub use verse_engine::presentation::{Instance, ResolvedInstances};
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
    texture: verse_engine::residency::TextureHandle,
    blend: u8,
    emissive: bool,
}
struct Actor {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
}
struct Grounding {
    basis: [[f32; 4]; 3],
    bones: Box<[[[f32; 4]; 4]; 256]>,
    lift: f32,
}
impl Grounding {
    fn matches(&self, palette: &Pose) -> bool {
        self.basis == [palette.model[0], palette.model[1], palette.model[2]]
            && *self.bones == palette.bones
    }
}
fn ground_lift(model: &verse_engine::assets::Model, palette: &Pose) -> f32 {
    let mut transform = Mat4::from_cols_array_2d(&palette.model);
    transform.w_axis = glam::Vec4::W;
    let bones: Vec<_> = palette.bones.iter().map(Mat4::from_cols_array_2d).collect();
    let mut lowest = f32::INFINITY;
    for vertex in model.surfaces.iter().flat_map(|s| &s.vertices) {
        let point =
            vertex
                .joints
                .iter()
                .zip(vertex.weights)
                .fold(Vec3::ZERO, |p, (joint, weight)| {
                    p + bones[*joint as usize].transform_point3(vertex.position.into()) * weight
                });
        lowest = lowest.min(transform.transform_point3(point).y);
    }
    (0.05 - lowest).clamp(
        0.,
        model.height.max(0.6) * transform.y_axis.truncate().length(),
    )
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct FrameTimings {
    pub prepare_ms: f64,
    pub encode_ms: f64,
    pub gpu_wait_ms: f64,
    pub readback_copy_ms: f64,
    pub total_ms: f64,
    pub instances: usize,
    pub grounded_vertices: usize,
    pub readback: bool,
    pub shadow_draws: usize,
}
#[derive(Clone)]
struct GpuContext {
    #[cfg(feature = "imported-desktop")]
    instance: wgpu::Instance,
    #[cfg(feature = "imported-desktop")]
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
    id: verse_engine::residency::CatalogId,
}
/// A worker-safe snapshot of the GPU and the catalog a replacement must supersede.
pub struct ReloadSource {
    context: GpuContext,
    base: verse_engine::residency::CatalogId,
    width: u32,
    height: u32,
    pack: std::sync::Arc<Pack>,
}
/// Completely uploaded replacement; active state stays unchanged until commit.
pub struct ReloadCandidate {
    base: verse_engine::residency::CatalogId,
    renderer: Renderer,
    keep_playback: std::collections::BTreeSet<String>,
}
impl ReloadCandidate {
    pub fn pack(&self) -> std::sync::Arc<Pack> {
        self.renderer.pack.clone()
    }
}
fn motion_digest(model: &verse_engine::assets::Model) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(
        &mut writer,
        &(&model.bones, &model.skin, &model.clips, &model.states),
    )
    .map_err(|e| e.to_string())?;
    Ok(writer.0.finalize().into())
}
impl ReloadSource {
    pub fn prepare(
        self,
        prepared: verse_engine::loading::Prepared,
        atlas: &Atlas,
        static_instances: &[Instance],
    ) -> Result<ReloadCandidate, String> {
        let mut keep_playback = std::collections::BTreeSet::new();
        for (name, model) in &self.pack.models {
            if let Some(next) = prepared.pack().models.get(name) {
                if motion_digest(model)? == motion_digest(next)? {
                    keep_playback.insert(name.clone());
                }
            }
        }
        let device = self.context.device.clone();
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = Renderer::build(
            prepared,
            self.width,
            self.height,
            atlas,
            static_instances,
            Some(self.context),
        );
        let errors = [validation.pop(), memory.pop(), internal.pop()];
        for error in errors {
            if let Some(error) = pollster::block_on(error) {
                return Err(format!("Renderer reload upload failed: {error}"));
            }
        }
        Ok(ReloadCandidate {
            base: self.base,
            renderer: result?,
            keep_playback,
        })
    }
}
/// Persistent offscreen renderer; frames come directly from owned GPU passes.
pub struct Renderer {
    #[cfg(feature = "imported-desktop")]
    instance: wgpu::Instance,
    #[cfg(feature = "imported-desktop")]
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pack: std::sync::Arc<Pack>,
    gpu_id: verse_engine::residency::CatalogId,
    pub pack_receipt: verse_engine::loading::Receipt,
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
    catalog: verse_engine::residency::Catalog,
    models: HashMap<verse_engine::residency::ModelHandle, Vec<Batch>>,
    static_batches: Vec<Batch>,
    actors: Vec<Actor>,
    playback: HashMap<(verse_engine::core::LifeId, String), animation::Playback>,
    grounding: HashMap<(Option<verse_engine::core::LifeId>, String), Grounding>,
    bounds: HashMap<String, Option<culling::BoneBounds>>,
    ui_pipeline: wgpu::RenderPipeline,
    ui_group: wgpu::BindGroup,
    _ui_screen: wgpu::Buffer,
    ui_buffer: wgpu::Buffer,
    pub adapter_name: String,
    pub last_timings: FrameTimings,
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
fn upload(
    device: &wgpu::Device,
    catalog: &verse_engine::residency::Catalog,
    merged: Merged,
) -> Vec<Batch> {
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
            texture: catalog.texture(texture).expect("Validated surface texture"),
            blend,
            emissive,
        })
        .collect()
}
fn make_pose(pack: &Pack, instance: Option<&Instance>) -> Result<Pose, String> {
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
        for (dst, m) in pose.bones.iter_mut().zip(if i.actor.is_none() {
            animation::pose_selected(&pack.models[&i.model], i.animation, i.time)?
        } else {
            vec![]
        }) {
            *dst = m.to_cols_array_2d();
        }
    }
    Ok(pose)
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
        let prepared = verse_engine::loading::Prepared::load(pack, dir, Default::default())?;
        Self::from_prepared(prepared, width, height, atlas, static_instances)
    }
    /// Uploads a completely admitted pack; this path performs no file reads or decoding.
    pub fn from_prepared(
        prepared: verse_engine::loading::Prepared,
        width: u32,
        height: u32,
        atlas: &Atlas,
        static_instances: &[Instance],
    ) -> Result<Self, String> {
        Self::build(prepared, width, height, atlas, static_instances, None)
    }
    fn build(
        prepared: verse_engine::loading::Prepared,
        width: u32,
        height: u32,
        atlas: &Atlas,
        static_instances: &[Instance],
        context: Option<GpuContext>,
    ) -> Result<Self, String> {
        let (pack, decoded, pack_receipt) = prepared.into_parts();
        let catalog = verse_engine::residency::Catalog::new(&pack)?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Invalid imported viewport".into());
        }
        if static_instances
            .iter()
            .any(|i| !pack.models.contains_key(&i.model) || !i.transform.is_finite())
        {
            return Err("Invalid static placement".into());
        }
        let context = match context {
            Some(context) => context,
            None => {
                let instance = wgpu::Instance::new(
                    wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
                );
                let adapter = pollster::block_on(
                    instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
                )
                .map_err(|e| e.to_string())?;
                let adapter_name = adapter.get_info().name;
                let (device, queue) =
                    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                        label: Some("Verse imported world"),
                        required_limits: adapter.limits(),
                        ..Default::default()
                    }))
                    .map_err(|e| e.to_string())?;
                GpuContext {
                    #[cfg(feature = "imported-desktop")]
                    instance,
                    #[cfg(feature = "imported-desktop")]
                    adapter,
                    device,
                    queue,
                    name: adapter_name,
                    id: catalog.id(),
                }
            }
        };
        let GpuContext {
            #[cfg(feature = "imported-desktop")]
            instance,
            #[cfg(feature = "imported-desktop")]
            adapter,
            device,
            queue,
            name: adapter_name,
            id: gpu_id,
        } = context;
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
        for (t, pixels) in pack.textures.iter().zip(&decoded) {
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
                pixels.rgba(),
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
        let static_batches = upload(&device, &catalog, merge(&pack, static_instances));
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
            models.insert(catalog.model(name)?, upload(&device, &catalog, merged));
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
        let bounds = pack
            .models
            .iter()
            .map(|(key, model)| (key.clone(), culling::BoneBounds::compile(model)))
            .collect();
        Ok(Self {
            #[cfg(feature = "imported-desktop")]
            instance,
            #[cfg(feature = "imported-desktop")]
            adapter,
            device,
            queue,
            pack: std::sync::Arc::new(pack),
            gpu_id,
            pack_receipt,
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
            catalog,
            models,
            static_batches,
            actors: vec![],
            ui_pipeline,
            ui_group,
            _ui_screen: ui_screen,
            ui_buffer,
            playback: HashMap::new(),
            grounding: HashMap::new(),
            bounds,
            adapter_name,
            last_timings: FrameTimings::default(),
        })
    }
    pub fn reload_source(&self) -> ReloadSource {
        ReloadSource {
            context: GpuContext {
                #[cfg(feature = "imported-desktop")]
                instance: self.instance.clone(),
                #[cfg(feature = "imported-desktop")]
                adapter: self.adapter.clone(),
                device: self.device.clone(),
                queue: self.queue.clone(),
                name: self.adapter_name.clone(),
                id: self.gpu_id,
            },
            base: self.catalog.id(),
            width: self.width,
            height: self.height,
            pack: self.pack.clone(),
        }
    }
    /// Call between frames. The caller can dispose of retired resources on a worker.
    pub fn commit_reload(&mut self, mut candidate: ReloadCandidate) -> Result<Self, String> {
        self.catalog.check(candidate.base)?;
        let mut playback = std::mem::take(&mut self.playback);
        playback.retain(|(_, model), _| candidate.keep_playback.contains(model));
        candidate.renderer.playback = playback;
        Ok(std::mem::replace(self, candidate.renderer))
    }
    pub fn draw(
        &mut self,
        view: View,
        instances: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<Vec<u8>, String> {
        let resolved = self.resolve_instances(instances)?;
        self.draw_resolved(view, &resolved, ui, lighting)
    }
    /// Keeps interactive frames on the GPU; captures explicitly request readback.
    pub fn draw_live(
        &mut self,
        view: View,
        instances: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<(), String> {
        let resolved = self.resolve_instances(instances)?;
        self.draw_live_resolved(view, &resolved, ui, lighting)
    }
    pub fn resolve_instances<'a>(
        &self,
        instances: &'a [Instance],
    ) -> Result<ResolvedInstances<'a>, String> {
        ResolvedInstances::extract(&self.catalog, instances)
    }
    pub fn draw_resolved(
        &mut self,
        view: View,
        instances: &ResolvedInstances<'_>,
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<Vec<u8>, String> {
        self.draw_frame(view, instances, ui, lighting, true)
    }
    pub fn draw_live_resolved(
        &mut self,
        view: View,
        instances: &ResolvedInstances<'_>,
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<(), String> {
        self.draw_frame(view, instances, ui, lighting, false)
            .map(|_| ())
    }
    fn draw_frame(
        &mut self,
        view: View,
        resolved: &ResolvedInstances<'_>,
        ui: &UiBatch,
        lighting: &Lighting,
        capture: bool,
    ) -> Result<Vec<u8>, String> {
        let started = Instant::now();
        // Validate residency before writing any GPU buffer.
        resolved.validate(&self.catalog)?;
        let instances = resolved.instances();
        let mut grounded_vertices = 0;
        if instances.len() > 256
            || instances.iter().any(|i| !i.transform.is_finite())
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
                bytemuck::bytes_of(&make_pose(&self.pack, None)?),
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
            bytemuck::bytes_of(&make_pose(&self.pack, None)?),
        );
        self.playback.retain(|(id, model), _| {
            instances
                .iter()
                .any(|i| i.actor == Some(*id) && i.model == *model)
        });
        self.grounding.retain(|(id, model), _| {
            instances
                .iter()
                .any(|i| i.actor == *id && i.model == *model && i.animation.grounded())
        });
        let mut adventurer_pose: Option<Pose> = None;
        let mut actor_bounds = Vec::with_capacity(instances.len());
        for (i, instance) in instances.iter().enumerate() {
            let mut palette = make_pose(&self.pack, Some(instance))?;
            if let Some(id) = instance.actor {
                let bones = self
                    .playback
                    .entry((id, instance.model.clone()))
                    .or_default()
                    .update_for_life(
                        id,
                        &self.pack.models[&instance.model],
                        instance.animation,
                        instance.time,
                        lighting.time,
                    )?;
                for (dst, bone) in palette.bones.iter_mut().zip(bones) {
                    *dst = bone.to_cols_array_2d();
                }
            }
            if instance.animation.grounded() {
                // Ground fallen and prone bodies using their posed geometry.
                let model = &self.pack.models[&instance.model];
                let key = (instance.actor, instance.model.clone());
                let basis = [palette.model[0], palette.model[1], palette.model[2]];
                if self
                    .grounding
                    .get(&key)
                    .is_none_or(|g| !g.matches(&palette))
                {
                    grounded_vertices += model
                        .surfaces
                        .iter()
                        .map(|s| s.vertices.len())
                        .sum::<usize>();
                    self.grounding.insert(
                        key.clone(),
                        Grounding {
                            basis,
                            bones: Box::new(palette.bones),
                            lift: ground_lift(model, &palette),
                        },
                    );
                }
                let lift = self.grounding[&key].lift;
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
            actor_bounds.push(
                self.bounds[&instance.model]
                    .as_ref()
                    .and_then(|b| b.posed(&palette)),
            );
            self.queue
                .write_buffer(&self.actors[i + 1].buffer, 0, bytemuck::bytes_of(&palette));
        }
        let prepared = Instant::now();
        let mut shadow_draws = 0;
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
                shadow_draws += 1;
            }
            for (i, _) in instances.iter().enumerate() {
                if actor_bounds[i]
                    .is_some_and(|b| !b.visible(Mat4::from_cols_array_2d(&frame.shadow[layer])))
                {
                    continue;
                }
                pass.set_bind_group(2, &self.actors[i + 1].group, &[]);
                for batch in self.models[&resolved.models()[i]]
                    .iter()
                    .filter(|b| b.blend < 2 && !b.emissive)
                {
                    self.draw_batch(&mut pass, batch);
                    shadow_draws += 1;
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
                for (i, _) in order {
                    pass.set_bind_group(2, &self.actors[i + 1].group, &[]);
                    for batch in self.models[&resolved.models()[i]]
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
        if capture {
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
        }
        self.queue.submit([encoder.finish()]);
        let submitted = Instant::now();
        if !capture {
            self.last_timings = FrameTimings {
                prepare_ms: prepared.duration_since(started).as_secs_f64() * 1000.,
                encode_ms: submitted.duration_since(prepared).as_secs_f64() * 1000.,
                total_ms: started.elapsed().as_secs_f64() * 1000.,
                instances: instances.len(),
                grounded_vertices,
                readback: false,
                shadow_draws,
                ..Default::default()
            };
            return Ok(vec![]);
        }
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| e.to_string())?;
        let waited = Instant::now();
        let mapped = slice.get_mapped_range();
        let mut out = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height as usize {
            out.extend_from_slice(
                &mapped[y * self.row as usize..y * self.row as usize + self.width as usize * 4],
            );
        }
        drop(mapped);
        self.readback.unmap();
        self.last_timings = FrameTimings {
            prepare_ms: prepared.duration_since(started).as_secs_f64() * 1000.,
            encode_ms: submitted.duration_since(prepared).as_secs_f64() * 1000.,
            gpu_wait_ms: waited.duration_since(submitted).as_secs_f64() * 1000.,
            readback_copy_ms: waited.elapsed().as_secs_f64() * 1000.,
            total_ms: started.elapsed().as_secs_f64() * 1000.,
            instances: instances.len(),
            grounded_vertices,
            readback: true,
            shadow_draws,
        };
        Ok(out)
    }
    fn draw_batch<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, batch: &'a Batch) {
        pass.set_bind_group(
            1,
            &self.textures[self
                .catalog
                .texture_slot(batch.texture)
                .expect("Resident texture handle")
                * 8
                + usize::from(batch.emissive) * 4
                + usize::from(batch.blend)],
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
    fn reload_motion_identity_ignores_source_paths_and_tracks_animation_changes() {
        use verse_engine::assets::{Clip, Model};
        let mut model = Model {
            states: Default::default(),
            skin: None,
            source: "old.glb".into(),
            source_sha256: String::new(),
            surfaces: vec![],
            bones: vec![],
            clips: vec![],
            height: 2.,
            attachments: vec![],
        };
        let original = super::motion_digest(&model).unwrap();
        model.source = "moved.glb".into();
        assert_eq!(super::motion_digest(&model).unwrap(), original);
        model.clips.push(Clip {
            id: 0,
            duration: 1.,
            bones: vec![],
        });
        assert_ne!(super::motion_digest(&model).unwrap(), original);
    }
    #[test]
    fn corpse_grounding_ignores_world_translation_but_tracks_skin_and_basis_changes() {
        use super::*;
        use verse_engine::assets::{Model, Surface, Vertex};
        let model = Model {
            states: Default::default(),
            skin: None,
            source: "fixture".into(),
            source_sha256: String::new(),
            height: 2.,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                vertices: [[-0.2, -1., 0.], [0.3, 0., 0.], [0.2, 2., 0.]]
                    .into_iter()
                    .map(|position| Vertex {
                        position,
                        normal: [0., 1., 0.],
                        uv: [0.; 2],
                        joints: [1; 4],
                        weights: [1., 0., 0., 0.],
                    })
                    .collect(),
                indices: vec![0, 1, 2],
                texture: 0,
                blend: 0,
                emissive: false,
                tint: [1.; 3],
            }],
        };
        let mut pose = Pose {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            params: [0.; 4],
            bones: [Mat4::IDENTITY.to_cols_array_2d(); 256],
        };
        let cached = Grounding {
            basis: [pose.model[0], pose.model[1], pose.model[2]],
            bones: Box::new(pose.bones),
            lift: ground_lift(&model, &pose),
        };
        assert!((cached.lift - 1.05).abs() < 1e-6);
        pose.model[3] = [50., 80., -30., 1.];
        assert!(cached.matches(&pose));
        assert_eq!(cached.lift, ground_lift(&model, &pose));
        pose.bones[1] = Mat4::from_translation(-Vec3::Y * 0.25).to_cols_array_2d();
        assert!(!cached.matches(&pose));
        assert!((ground_lift(&model, &pose) - 1.3).abs() < 1e-6);
        pose.model = Mat4::from_scale(Vec3::splat(2.)).to_cols_array_2d();
        assert!(!cached.matches(&pose));
        assert!((ground_lift(&model, &pose) - 2.55).abs() < 1e-6);
    }
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
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    gpu_id: verse_engine::residency::CatalogId,
    catalog: verse_engine::residency::CatalogId,
    presented_catalog: Option<verse_engine::residency::CatalogId>,
    presented_frames: u64,
}
#[cfg(feature = "imported-desktop")]
impl WindowPresenter {
    pub fn current_frame_presented(&self, renderer: &Renderer) -> bool {
        self.gpu_id == renderer.gpu_id && self.presented_catalog == Some(renderer.catalog.id())
    }
    pub fn presented_frames(&self) -> u64 {
        self.presented_frames
    }
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
            layout,
            sampler,
            gpu_id: self.gpu_id,
            catalog: self.catalog.id(),
            presented_catalog: None,
            presented_frames: 0,
        })
    }
    pub fn present_window(&self, p: &mut WindowPresenter, size: [u32; 2]) -> Result<(), String> {
        if p.gpu_id != self.gpu_id {
            return Err("Presenter belongs to another GPU device".into());
        }
        if p.catalog != self.catalog.id() {
            p.group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Reloaded scene presentation"),
                layout: &p.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.target_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&p.sampler),
                    },
                ],
            });
            p.catalog = self.catalog.id();
        }
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
        p.presented_catalog = Some(self.catalog.id());
        p.presented_frames = p.presented_frames.saturating_add(1);
        Ok(())
    }
}
