//! Owned textured and skeletal rendering for imported worlds.
//!
//! Static geometry is merged by spatial cell and material and uploaded once. Each animated actor
//! updates only a bounded bone palette and placement. The same Verse glyph
//! pipeline draws screen-space overlays after world rendering.
use crate::{
    render::View,
    ui::{Atlas, UiBatch},
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
pub mod chamber;
#[cfg(feature = "remote-chamber")]
pub mod character_panel;
pub mod characters;
pub mod combat;
pub mod controls;
mod culling;
pub mod giver_panel;
pub mod icons;
pub mod inventory;
pub mod lighting;
mod material_gpu;
pub mod original;
pub mod overlay;
pub mod play;
pub mod props;
#[cfg(feature = "remote-chamber")]
pub mod remote_content;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub mod remote_record;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub mod remote_window;
mod shadow_cache;
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
    bounds: Option<culling::Bounds>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    material: material_gpu::Key,
    blend: u8,
    emissive: bool,
}
struct Actor {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    shadow_model: Option<verse_engine::residency::ModelHandle>,
    shadow_bundles: Vec<Option<wgpu::RenderBundle>>,
    shadow_count: usize,
    frozen: shadow_cache::Frozen<(
        verse_engine::residency::ModelHandle,
        Option<verse_engine::core::LifeId>,
    )>,
    world_bundles: Vec<Option<wgpu::RenderBundle>>,
    world_counts: [usize; 4],
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
    pub command_encode_ms: f64,
    pub shadow_encode_ms: f64,
    pub world_encode_ms: f64,
    pub overlay_encode_ms: f64,
    pub command_finish_ms: f64,
    pub queue_submit_ms: f64,
    pub gpu_wait_ms: f64,
    pub readback_copy_ms: f64,
    pub total_ms: f64,
    pub instances: usize,
    pub grounded_vertices: usize,
    pub readback: bool,
    pub shadow_draws: usize,
    pub cached_shadow_casters: usize,
    pub static_shadow_refreshes: usize,
    pub marker_events: usize,
    pub graph_instances: usize,
}
/// A presentation marker sampled at the actor's current world placement.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MarkerEvent {
    pub model: String,
    pub position: [f32; 3],
    pub event: verse_engine::markers::Event,
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
        &(
            &model.bones,
            &model.skin,
            &model.clips,
            &model.states,
            &model.markers,
            &model.graph,
        ),
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
    multisample_view: wgpu::TextureView,
    target_revision: u64,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    row: u32,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    shadow_views: Vec<wgpu::TextureView>,
    shadow_texture: wgpu::Texture,
    static_shadow_texture: wgpu::Texture,
    static_shadow_views: Vec<wgpu::TextureView>,
    static_shadow_keys: Vec<Option<shadow_cache::Face>>,
    shadow_groups: Vec<wgpu::BindGroup>,
    shadow_buffers: Vec<wgpu::Buffer>,
    shadow_pipeline: wgpu::RenderPipeline,
    pose_layout: wgpu::BindGroupLayout,
    materials: BTreeMap<material_gpu::Key, wgpu::BindGroup>,
    shadow_materials: BTreeMap<material_gpu::Key, wgpu::BindGroup>,
    pipelines: Vec<wgpu::RenderPipeline>,
    catalog: verse_engine::residency::Catalog,
    models: HashMap<verse_engine::residency::ModelHandle, Vec<Batch>>,
    static_batches: Vec<Batch>,
    static_world_bundles: Vec<wgpu::RenderBundle>,
    actors: Vec<Actor>,
    marker_events: Vec<MarkerEvent>,
    playback: HashMap<(verse_engine::core::LifeId, String), animation::Playback>,
    graphs: HashMap<String, std::sync::Arc<verse_engine::animation_graph::Semantic>>,
    #[cfg(test)]
    evaluated_poses: Vec<Pose>,
    graph_playback:
        HashMap<(verse_engine::core::LifeId, String), verse_engine::animation_graph::Playback>,
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
type Merged = BTreeMap<(material_gpu::Key, i32, i32), (Vec<GpuVertex>, Vec<u32>)>;
fn merge(pack: &Pack, instances: &[Instance]) -> Merged {
    let mut merged = Merged::new();
    for instance in instances {
        let model = &pack.models[&instance.model];
        for s in &model.surfaces {
            let (v, i) = merged
                .entry((
                    material_gpu::Key::from_surface(s),
                    (instance.transform.w_axis.x / 8.).floor() as i32,
                    (instance.transform.w_axis.z / 8.).floor() as i32,
                ))
                .or_default();
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
        .map(|((material, _, _), (v, i))| Batch {
            bounds: culling::Bounds::from_points(v.iter().map(|v| Vec3::from(v.position))),
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
            material,
            blend: material.blend,
            emissive: material.emissive,
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
/// A submitted copy of the last rendered target; mapping can run off the render thread.
pub struct PendingCapture {
    device: wgpu::Device,
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    row: u32,
}
impl PendingCapture {
    pub fn finish(self) -> Result<Vec<u8>, String> {
        let slice = self.buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| e.to_string())?;
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height as usize {
            pixels.extend_from_slice(
                &mapped[y * self.row as usize..y * self.row as usize + self.width as usize * 4],
            );
        }
        drop(mapped);
        self.buffer.unmap();
        Ok(pixels)
    }
}

impl Renderer {
    /// Consumes this frame's presentation events; these grant no gameplay authority.
    pub fn take_marker_events(&mut self) -> Vec<MarkerEvent> {
        std::mem::take(&mut self.marker_events)
    }

    /// Submit a copy without redrawing, GPU waiting, or PNG encoding on the render thread.
    pub fn capture_submitted(&self) -> PendingCapture {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Verse asynchronous capture"),
            size: self.row as u64 * self.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Verse capture copy"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: Some(self.height),
                },
            },
            extent(self.width, self.height),
        );
        self.queue.submit([encoder.finish()]);
        PendingCapture {
            device: self.device.clone(),
            buffer,
            width: self.width,
            height: self.height,
            row: self.row,
        }
    }

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
        let graphs = pack
            .models
            .iter()
            .filter_map(|(name, model)| {
                model.graph.as_ref().map(|graph| {
                    verse_engine::animation_graph::Semantic::new(graph, model)
                        .map(|compiled| (name.clone(), std::sync::Arc::new(compiled)))
                })
            })
            .collect::<Result<HashMap<_, _>, String>>()?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Invalid imported viewport".into());
        }
        if static_instances.iter().any(|i| {
            !pack.models.contains_key(&i.model) || !i.transform.is_finite() || i.mount.is_some()
        }) {
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
        let mut material_entries = vec![
            uniform(2),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        for binding in [0, 3, 4, 5, 6] {
            material_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imported material"),
            entries: &material_entries,
        });
        let shadow_material_entries: Vec<_> = material_entries
            .iter()
            .filter(|entry| entry.binding <= 2)
            .cloned()
            .collect();
        let shadow_material_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Verse shadow alpha material"),
                entries: &shadow_material_entries,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let static_shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Verse immutable static shadow cache"),
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 24,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let static_shadow_views = (0..24)
            .map(|layer| {
                static_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
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
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        });
        let mut texture_views = Vec::new();
        for (t, pixels) in pack.textures.iter().zip(&decoded) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&t.file),
                size: extent(t.width, t.height),
                mip_level_count: 32 - t.width.max(t.height).leading_zeros(),
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
            });
            let (mut width, mut height) = (t.width, t.height);
            let mut rgba = pixels.rgba().to_vec();
            for level in 0..tex.mip_level_count() {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
                        mip_level: level,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                    extent(width, height),
                );
                if level + 1 < tex.mip_level_count() {
                    let (next_width, next_height) = ((width / 2).max(1), (height / 2).max(1));
                    let mut next = Vec::with_capacity((next_width * next_height * 4) as usize);
                    for y in 0..next_height {
                        for x in 0..next_width {
                            let mut sum = [0u32; 4];
                            let mut count = 0;
                            for sy in y * height / next_height..(y + 1) * height / next_height {
                                for sx in x * width / next_width..(x + 1) * width / next_width {
                                    let offset = ((sy * width + sx) * 4) as usize;
                                    for channel in 0..4 {
                                        sum[channel] += u32::from(rgba[offset + channel]);
                                    }
                                    count += 1;
                                }
                            }
                            next.extend(sum.map(|value| ((value + count / 2) / count) as u8));
                        }
                    }
                    rgba = next;
                    width = next_width;
                    height = next_height;
                }
            }
            let srgb = tex.create_view(&Default::default());
            let linear = tex.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8Unorm),
                ..Default::default()
            });
            texture_views.push((srgb, linear));
        }
        let keys: std::collections::BTreeSet<_> = pack
            .models
            .values()
            .flat_map(|model| model.surfaces.iter().map(material_gpu::Key::from_surface))
            .collect();
        let mut materials = BTreeMap::new();
        let mut shadow_materials = BTreeMap::new();
        for key in keys {
            let uniform = buffer(
                &device,
                "Verse authored material",
                bytemuck::cast_slice(&key.uniform()),
                wgpu::BufferUsages::UNIFORM,
            );
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_views[key.texture].0),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ];
            shadow_materials.insert(
                key,
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Verse shadow alpha resources"),
                    layout: &shadow_material_layout,
                    entries: &entries,
                }),
            );
            for (channel, slot) in key.maps.into_iter().enumerate() {
                let views = &texture_views[slot.unwrap_or(key.texture)];
                entries.push(wgpu::BindGroupEntry {
                    binding: channel as u32 + 3,
                    resource: wgpu::BindingResource::TextureView(if channel == 3 {
                        &views.0
                    } else {
                        &views.1
                    }),
                });
            }
            materials.insert(
                key,
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Verse authored material images"),
                    layout: &texture_layout,
                    entries: &entries,
                }),
            );
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
            pipelines.push(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("Verse textured skin"),layout:Some(&layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<GpuVertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32x4,4=>Float32x4,5=>Float32x3]}]},primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(blend<2),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),multisample:wgpu::MultisampleState{count:4,..Default::default()},fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:match blend{2=>Some(wgpu::BlendState::ALPHA_BLENDING),3=>Some(wgpu::BlendState{color:wgpu::BlendComponent{src_factor:wgpu::BlendFactor::SrcAlpha,dst_factor:wgpu::BlendFactor::One,operation:wgpu::BlendOperation::Add},alpha:wgpu::BlendComponent::OVER}),_=>None},write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None}));
        }
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    Some(&shadow_layout),
                    Some(&shadow_material_layout),
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
                let (v, i) = merged
                    .entry((material_gpu::Key::from_surface(s), 0, 0))
                    .or_default();
                let offset = v.len() as u32;
                v.extend(s.vertices.iter().map(|p| {
                    let mut v = GpuVertex::from(p);
                    v.tint = s.tint;
                    v
                }));
                i.extend(s.indices.iter().map(|i| i + offset));
            }
            models.insert(catalog.model(name)?, upload(&device, merged));
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
        let multisample_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Verse four-sample color"),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 4,
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
            crate::render::ui_pipeline(&device, &queue, format, 4, atlas);
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
            multisample_view,
            target_revision: 0,
            depth,
            readback,
            row,
            frame,
            frame_group,
            shadow_views,
            shadow_texture,
            static_shadow_texture,
            static_shadow_views,
            static_shadow_keys: vec![None; 24],
            shadow_groups,
            shadow_buffers,
            shadow_pipeline,
            pose_layout,
            materials,
            shadow_materials,
            pipelines,
            catalog,
            models,
            static_batches,
            static_world_bundles: vec![],
            actors: vec![],
            ui_pipeline,
            ui_group,
            _ui_screen: ui_screen,
            ui_buffer,
            marker_events: Vec::new(),
            playback: HashMap::new(),
            graphs,
            #[cfg(test)]
            evaluated_poses: Vec::new(),
            graph_playback: HashMap::new(),
            grounding: HashMap::new(),
            bounds,
            adapter_name,
            last_timings: FrameTimings::default(),
        })
    }
    /// Reallocate viewport attachments without reloading scene assets or shadow caches.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if width > 4096 || height > 4096 {
            return Err("Viewport exceeds 4096 pixels".into());
        }
        if (width, height) == (self.width, self.height) {
            return Ok(());
        }
        let texture = |label, format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        self.target = texture(
            "Verse native-resolution color",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        self.target_view = self.target.create_view(&Default::default());
        self.multisample_view = texture(
            "Verse four-sample color",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            4,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        self.depth = texture(
            "Verse four-sample depth",
            wgpu::TextureFormat::Depth32Float,
            4,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        self.row = (width * 4).div_ceil(256) * 256;
        self.readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Verse native-resolution capture"),
            size: u64::from(self.row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.width = width;
        self.height = height;
        self.set_overlay_size(width as f32, height as f32);
        self.target_revision = self.target_revision.wrapping_add(1);
        Ok(())
    }
    pub fn set_overlay_size(&self, width: f32, height: f32) {
        self.queue.write_buffer(
            &self._ui_screen,
            0,
            bytemuck::cast_slice(&[width, height, 0.0, 0.0]),
        );
    }
    /// Finish the loading frame before starting the interactive simulation clock.
    pub fn finish_loading_frame(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn dimensions(&self) -> [u32; 2] {
        [self.width, self.height]
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
        for name in &candidate.keep_playback {
            if let Some(graph) = self.graphs.get(name) {
                candidate
                    .renderer
                    .graphs
                    .insert(name.clone(), graph.clone());
            }
        }
        let mut graphs = std::mem::take(&mut self.graph_playback);
        graphs.retain(|(_, model), _| candidate.renderer.graphs.contains_key(model));
        candidate.renderer.graph_playback = graphs;
        candidate.renderer.resize(self.width, self.height)?;
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
        let world = verse_engine::render_world::RenderWorld::from_resolved(
            &self.catalog,
            view,
            instances,
            &ui.vertices,
            lighting,
        )?;
        self.draw_world(&world)
    }
    pub fn draw_live_resolved(
        &mut self,
        view: View,
        instances: &ResolvedInstances<'_>,
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<(), String> {
        let world = verse_engine::render_world::RenderWorld::from_resolved(
            &self.catalog,
            view,
            instances,
            &ui.vertices,
            lighting,
        )?;
        self.draw_live_world(&world)
    }
    /// Captures the same admitted frame used by interactive rendering.
    pub fn draw_world(
        &mut self,
        world: &verse_engine::render_world::RenderWorld<'_>,
    ) -> Result<Vec<u8>, String> {
        self.draw_frame(world, true)
    }
    pub fn draw_live_world(
        &mut self,
        world: &verse_engine::render_world::RenderWorld<'_>,
    ) -> Result<(), String> {
        self.draw_frame(world, false).map(|_| ())
    }
    fn draw_frame(
        &mut self,
        world: &verse_engine::render_world::RenderWorld<'_>,
        capture: bool,
    ) -> Result<Vec<u8>, String> {
        let started = Instant::now();
        world.validate(&self.catalog)?;
        self.marker_events.clear();
        let view = world.view();
        let lighting = world.lighting();
        let resolved = world.instances();
        let instances = resolved.instances();
        let mut grounded_vertices = 0;
        let ui_bytes = bytemuck::cast_slice(world.overlay().vertices());
        let frame = lighting::frame(view, lighting)?;
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
            self.actors.push(Actor {
                buffer,
                group,
                shadow_model: None,
                shadow_bundles: (0..24).map(|_| None).collect(),
                shadow_count: 0,
                frozen: Default::default(),
                world_bundles: (0..4).map(|_| None).collect(),
                world_counts: [0; 4],
            });
        }
        self.playback.retain(|(id, model), _| {
            instances
                .iter()
                .any(|i| i.actor == Some(*id) && i.model == *model)
        });
        self.graph_playback.retain(|(id, model), _| {
            instances
                .iter()
                .any(|instance| instance.actor == Some(*id) && instance.model == *model)
        });
        self.grounding.retain(|(id, model), _| {
            instances
                .iter()
                .any(|i| i.actor == *id && i.model == *model && i.animation.grounded())
        });
        let mut graph_instances = 0;

        let mut actor_bounds = Vec::with_capacity(instances.len());
        let mut palettes = Vec::with_capacity(instances.len());
        let mut frozen = Vec::with_capacity(instances.len());
        for instance in instances {
            let mut palette = make_pose(&self.pack, Some(instance))?;
            if let Some(id) = instance.actor {
                let (bones, events) = if let Some(graph) = self.graphs.get(&instance.model) {
                    graph_instances += 1;
                    let verse_engine::motion::Selection::Named(state) = instance.animation else {
                        return Err("Authored animation graph requires a semantic selection".into());
                    };
                    let values = graph.values(state)?;
                    let frame = self
                        .graph_playback
                        .entry((id, instance.model.clone()))
                        .or_default()
                        .update_sampled(
                            graph.admitted(),
                            id,
                            &values,
                            f64::from(instance.time),
                            f64::from(lighting.time),
                        )?;
                    (frame.matrices, frame.markers)
                } else {
                    self.playback
                        .entry((id, instance.model.clone()))
                        .or_default()
                        .update_with_markers(
                            id,
                            &self.pack.models[&instance.model],
                            instance.animation,
                            instance.time,
                            lighting.time,
                        )?
                };
                if self.marker_events.len() + events.len() > 4096 {
                    return Err("Frame animation markers exceed the presentation budget".into());
                }
                self.marker_events
                    .extend(events.into_iter().map(|event| MarkerEvent {
                        model: instance.model.clone(),
                        position: instance.transform.w_axis.truncate().to_array(),
                        event,
                    }));
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
            palettes.push(palette);
        }
        // Resolve leaf attachments after every body has its final animation and grounding.
        let mut socket_palettes: Vec<Option<Vec<Mat4>>> = vec![None; instances.len()];
        for (i, instance) in instances.iter().enumerate() {
            let Some(parent_index) = resolved.parents()[i] else {
                continue;
            };
            let mount = instance.mount.as_ref().unwrap();
            let parent = palettes[parent_index];
            let model = &self.pack.models[&instances[parent_index].model];
            let matrices = socket_palettes[parent_index].get_or_insert_with(|| {
                parent.bones[..model.bones.len().max(1)]
                    .iter()
                    .map(Mat4::from_cols_array_2d)
                    .collect()
            });
            let bones = verse_engine::sockets::Palette::admit(model, matrices)?;
            let sockets = verse_engine::sockets::Sockets::admit(model)?;
            let body = Mat4::from_cols_array_2d(&parent.model);
            let transform = if instance.model == "bow" && mount.socket == 2 {
                chamber::mounted_bow(
                    sockets,
                    bones,
                    body,
                    chamber::bow_drawn(instances[parent_index].animation),
                )? * mount.local
            } else {
                sockets.frame(bones, body, mount.socket, mount.local)?
            };
            if !transform.is_finite() {
                return Err("Mounted render transform overflowed".into());
            }
            palettes[i].model = transform.to_cols_array_2d();
        }
        #[cfg(test)]
        {
            self.evaluated_poses = palettes.clone();
        }
        for (i, instance) in instances.iter().enumerate() {
            let palette = palettes[i];
            actor_bounds.push(
                self.bounds[&instance.model]
                    .as_ref()
                    .and_then(|b| b.posed(&palette)),
            );
            frozen.push(self.actors[i + 1].frozen.update(
                (resolved.models()[i], instance.actor),
                bytemuck::bytes_of(&palette),
                instance.animation.grounded(),
            )?);
        }
        for (i, model) in resolved.models().iter().enumerate() {
            let actor = &mut self.actors[i + 1];
            if actor.shadow_model != Some(*model) {
                actor.shadow_model = Some(*model);
                actor.world_counts = std::array::from_fn(|blend| {
                    self.models[model]
                        .iter()
                        .filter(|b| usize::from(b.blend) == blend)
                        .count()
                });
                actor
                    .shadow_bundles
                    .iter_mut()
                    .for_each(|bundle| *bundle = None);
                actor
                    .world_bundles
                    .iter_mut()
                    .for_each(|bundle| *bundle = None);
                actor.shadow_count = self.models[model]
                    .iter()
                    .filter(|b| b.blend < 2 && !b.emissive)
                    .count();
            }
            if actor.shadow_count == 0 {
                continue;
            }
            for layer in 0..lighting.shadow_count() * 6 {
                if actor.shadow_bundles[layer].is_some()
                    || actor_bounds[i]
                        .is_some_and(|b| !b.visible(Mat4::from_cols_array_2d(&frame.shadow[layer])))
                {
                    continue;
                }
                let mut bundle = self.device.create_render_bundle_encoder(
                    &wgpu::RenderBundleEncoderDescriptor {
                        label: Some("Verse reusable actor shadow commands"),
                        color_formats: &[],
                        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                            format: wgpu::TextureFormat::Depth32Float,
                            depth_read_only: false,
                            stencil_read_only: true,
                        }),
                        sample_count: 1,
                        multiview: None,
                    },
                );
                bundle.set_pipeline(&self.shadow_pipeline);
                bundle.set_bind_group(0, &self.shadow_groups[layer], &[]);
                bundle.set_bind_group(2, &actor.group, &[]);
                for batch in self.models[model]
                    .iter()
                    .filter(|b| b.blend < 2 && !b.emissive)
                {
                    bundle.set_bind_group(1, &self.shadow_materials[&batch.material], &[]);
                    bundle.set_vertex_buffer(0, batch.vertices.slice(..));
                    bundle.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
                    bundle.draw_indexed(0..batch.count, 0, 0..1);
                }
                actor.shadow_bundles[layer] = Some(bundle.finish(&wgpu::RenderBundleDescriptor {
                    label: Some("Verse actor shadow commands"),
                }));
            }
        }
        if self.static_world_bundles.is_empty() {
            self.static_world_bundles = self
                .static_batches
                .iter()
                .map(|batch| {
                    self.world_bundle(
                        &self.actors[0].group,
                        std::slice::from_ref(batch),
                        batch.blend,
                    )
                })
                .collect();
        }
        for (i, model) in resolved.models().iter().enumerate() {
            for blend in 0..4 {
                if self.actors[i + 1].world_counts[blend] > 0
                    && self.actors[i + 1].world_bundles[blend].is_none()
                {
                    let bundle = self.world_bundle(
                        &self.actors[i + 1].group,
                        &self.models[model],
                        blend as u8,
                    );
                    self.actors[i + 1].world_bundles[blend] = Some(bundle);
                }
            }
        }
        let shadow_keys: Vec<_> = (0..lighting.shadow_count() * 6)
            .map(|layer| {
                let matrix = frame.shadow[layer];
                let view = Mat4::from_cols_array_2d(&matrix);
                let casters = frozen
                    .iter()
                    .enumerate()
                    .filter_map(|(i, revision)| {
                        let revision = (*revision)?;
                        (self.actors[i + 1].shadow_count > 0
                            && actor_bounds[i].is_none_or(|bounds| bounds.visible(view)))
                        .then_some((i, revision))
                    })
                    .collect();
                shadow_cache::Face { matrix, casters }
            })
            .collect();
        let refresh: Vec<_> = shadow_keys
            .iter()
            .enumerate()
            .map(|(layer, key)| self.static_shadow_keys[layer].as_ref() != Some(key))
            .collect();
        let plan = verse_engine::render_graph::ChamberPlan::build(&refresh, capture)?;
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
        self.queue.write_buffer(
            &self.actors[0].buffer,
            0,
            bytemuck::bytes_of(&make_pose(&self.pack, None)?),
        );
        for (i, palette) in palettes.iter().enumerate() {
            self.queue
                .write_buffer(&self.actors[i + 1].buffer, 0, bytemuck::bytes_of(palette));
        }
        let prepared = Instant::now();
        let mut shadow_draws = 0;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        use verse_engine::render_graph::ChamberPass;
        let mut shadows_encoded = prepared;
        let mut world_encoded = prepared;
        let mut overlay_encoded = prepared;
        for action in plan.actions() {
            match action {
                ChamberPass::RefreshShadow { layer } => {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Verse static shadow cache refresh"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.static_shadow_views[layer],
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.),
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
                        if batch.bounds.is_some_and(|b| {
                            !b.visible(Mat4::from_cols_array_2d(&frame.shadow[layer]))
                        }) {
                            continue;
                        }
                        self.draw_batch(&mut pass, batch);
                        shadow_draws += 1;
                    }
                    pass.execute_bundles(shadow_keys[layer].casters.iter().map(|(i, _)| {
                        let actor = &self.actors[i + 1];
                        shadow_draws += actor.shadow_count;
                        actor.shadow_bundles[layer].as_ref().unwrap()
                    }));
                    drop(pass);
                    self.static_shadow_keys[layer] = Some(shadow_keys[layer].clone());
                }
                ChamberPass::CopyShadow { layer } => {
                    let origin = wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    };
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.static_shadow_texture,
                            mip_level: 0,
                            origin,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.shadow_texture,
                            mip_level: 0,
                            origin,
                            aspect: wgpu::TextureAspect::All,
                        },
                        extent(512, 512),
                    );
                }
                ChamberPass::DrawShadow { layer } => {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Verse dynamic shadow face"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.shadow_views[layer],
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                    pass.set_pipeline(&self.shadow_pipeline);
                    pass.set_bind_group(0, &self.shadow_groups[layer], &[]);
                    let shadow_view = Mat4::from_cols_array_2d(&frame.shadow[layer]);
                    pass.execute_bundles(instances.iter().enumerate().filter_map(|(i, _)| {
                        let actor = &self.actors[i + 1];
                        if actor.shadow_count == 0
                            || frozen[i].is_some()
                            || actor_bounds[i].is_some_and(|bounds| !bounds.visible(shadow_view))
                        {
                            return None;
                        }
                        shadow_draws += actor.shadow_count;
                        Some(actor.shadow_bundles[layer].as_ref().unwrap())
                    }));
                }
                ChamberPass::WorldResolve => {
                    shadows_encoded = Instant::now();

                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Verse imported world"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.multisample_view,
                            depth_slice: None,
                            resolve_target: Some(&self.target_view),
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Discard,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Discard,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                    pass.set_bind_group(0, &self.frame_group, &[]);
                    for blend in 0..4 {
                        let static_bundles =
                            self.static_batches
                                .iter()
                                .enumerate()
                                .filter_map(|(index, batch)| {
                                    (batch.blend == blend as u8
                                        && batch
                                            .bounds
                                            .is_none_or(|bounds| bounds.visible(view.view_proj)))
                                    .then_some(&self.static_world_bundles[index])
                                });
                        let mut order: Vec<_> = instances.iter().enumerate().collect();
                        if blend == 2 {
                            order.sort_by(|(_, a), (_, b)| {
                                b.transform
                                    .w_axis
                                    .truncate()
                                    .distance_squared(view.eye)
                                    .total_cmp(
                                        &a.transform.w_axis.truncate().distance_squared(view.eye),
                                    )
                            });
                        }
                        let actor_bundles = order.into_iter().filter_map(|(i, _)| {
                            let actor = &self.actors[i + 1];
                            if actor.world_counts[blend] == 0
                                || actor_bounds[i]
                                    .is_some_and(|bounds| !bounds.visible(view.view_proj))
                            {
                                return None;
                            }
                            Some(actor.world_bundles[blend].as_ref().unwrap())
                        });
                        pass.execute_bundles(static_bundles.chain(actor_bundles));
                    }

                    drop(pass);
                    world_encoded = Instant::now();
                }
                ChamberPass::Overlay => {
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
                    pass.draw(0..world.overlay().vertices().len() as u32, 0..1);

                    drop(pass);
                    overlay_encoded = Instant::now();
                }
                ChamberPass::Readback => {
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
            }
        }
        let finish_started = Instant::now();
        let commands = encoder.finish();
        let encoded = Instant::now();
        self.queue.submit([commands]);
        let submitted = Instant::now();
        if !capture {
            self.last_timings = FrameTimings {
                prepare_ms: prepared.duration_since(started).as_secs_f64() * 1000.,
                encode_ms: submitted.duration_since(prepared).as_secs_f64() * 1000.,
                command_encode_ms: encoded.duration_since(prepared).as_secs_f64() * 1000.,
                shadow_encode_ms: shadows_encoded.duration_since(prepared).as_secs_f64() * 1000.,
                world_encode_ms: world_encoded.duration_since(shadows_encoded).as_secs_f64()
                    * 1000.,
                overlay_encode_ms: overlay_encoded.duration_since(world_encoded).as_secs_f64()
                    * 1000.,
                command_finish_ms: encoded.duration_since(finish_started).as_secs_f64() * 1000.,
                queue_submit_ms: submitted.duration_since(encoded).as_secs_f64() * 1000.,
                total_ms: started.elapsed().as_secs_f64() * 1000.,
                instances: instances.len(),
                grounded_vertices,
                readback: false,
                shadow_draws,
                cached_shadow_casters: frozen.iter().filter(|revision| revision.is_some()).count(),
                static_shadow_refreshes: refresh.iter().filter(|value| **value).count(),
                marker_events: self.marker_events.len(),
                graph_instances,
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
            command_encode_ms: encoded.duration_since(prepared).as_secs_f64() * 1000.,
            shadow_encode_ms: shadows_encoded.duration_since(prepared).as_secs_f64() * 1000.,
            world_encode_ms: world_encoded.duration_since(shadows_encoded).as_secs_f64() * 1000.,
            overlay_encode_ms: overlay_encoded.duration_since(world_encoded).as_secs_f64() * 1000.,
            command_finish_ms: encoded.duration_since(finish_started).as_secs_f64() * 1000.,
            queue_submit_ms: submitted.duration_since(encoded).as_secs_f64() * 1000.,
            gpu_wait_ms: waited.duration_since(submitted).as_secs_f64() * 1000.,
            readback_copy_ms: waited.elapsed().as_secs_f64() * 1000.,
            total_ms: started.elapsed().as_secs_f64() * 1000.,
            instances: instances.len(),
            grounded_vertices,
            readback: true,
            shadow_draws,
            cached_shadow_casters: frozen.iter().filter(|revision| revision.is_some()).count(),
            static_shadow_refreshes: refresh.iter().filter(|value| **value).count(),
            marker_events: self.marker_events.len(),
            graph_instances,
        };
        Ok(out)
    }
    fn world_bundle(
        &self,
        pose: &wgpu::BindGroup,
        batches: &[Batch],
        blend: u8,
    ) -> wgpu::RenderBundle {
        let mut bundle =
            self.device
                .create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("Verse reusable world commands"),
                    color_formats: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb)],
                    depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_read_only: false,
                        stencil_read_only: true,
                    }),
                    sample_count: 4,
                    multiview: None,
                });
        bundle.set_pipeline(&self.pipelines[usize::from(blend)]);
        bundle.set_bind_group(0, &self.frame_group, &[]);
        bundle.set_bind_group(2, pose, &[]);
        for batch in batches.iter().filter(|b| b.blend == blend) {
            bundle.set_bind_group(1, &self.materials[&batch.material], &[]);
            bundle.set_vertex_buffer(0, batch.vertices.slice(..));
            bundle.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
            bundle.draw_indexed(0..batch.count, 0, 0..1);
        }
        bundle.finish(&wgpu::RenderBundleDescriptor {
            label: Some("Verse world commands"),
        })
    }
    fn draw_batch<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, batch: &'a Batch) {
        pass.set_bind_group(1, &self.shadow_materials[&batch.material], &[]);
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
            graph: None,
            markers: Vec::new(),
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
        model.bones.push(verse_engine::assets::Bone {
            parent: -1,
            pivot: [0.; 3],
        });
        model.states.insert(
            verse_engine::motion::State::Idle,
            verse_engine::motion::Binding {
                clip: 0,
                mode: verse_engine::motion::Mode::Loop,
                transition_seconds: 0.2,
            },
        );
        model.graph = Some(verse_engine::animation_graph::Authored::from_bindings(
            &model,
        ));
        model.graph.as_ref().unwrap().validate(&model).unwrap();
        let graph = super::motion_digest(&model).unwrap();
        if let verse_engine::animation_graph::Node::Clip { rate, .. } =
            &mut model.graph.as_mut().unwrap().graph.nodes[0]
        {
            *rate = 1.5;
        }
        assert_ne!(super::motion_digest(&model).unwrap(), graph);
    }
    #[test]
    fn corpse_grounding_ignores_world_translation_but_tracks_skin_and_basis_changes() {
        use super::*;
        use verse_engine::assets::{Model, Surface, Vertex};
        let model = Model {
            graph: None,
            markers: Vec::new(),
            states: Default::default(),
            skin: None,
            source: "fixture".into(),
            source_sha256: String::new(),
            height: 2.,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                material: Default::default(),
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
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        naga::back::msl::write_string(
            &module,
            &info,
            &naga::back::msl::Options {
                lang_version: (2, 3),
                ..Default::default()
            },
            &naga::back::msl::PipelineOptions::default(),
        )
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
    target_revision: u64,
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
            target_revision: self.target_revision,
        })
    }
    pub fn present_window(&self, p: &mut WindowPresenter, size: [u32; 2]) -> Result<(), String> {
        if p.gpu_id != self.gpu_id {
            return Err("Presenter belongs to another GPU device".into());
        }
        if p.catalog != self.catalog.id() || p.target_revision != self.target_revision {
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
            p.target_revision = self.target_revision;
            p.presented_catalog = None;
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
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Window surface validation failed".into());
            }
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
