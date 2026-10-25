use super::{Camera, Mesh, Result, failed};
use glam::{Mat4, Vec3, Vec4};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
use wgpu::util::DeviceExt;
#[derive(Default)]
struct Completions {
    generation: u64,
    submitted: u64,
    count: u64,
    start: Option<Instant>,
    times: VecDeque<f64>,
}

struct Context {
    device: wgpu::Device,
    queue: wgpu::Queue,
    completions: Arc<Mutex<Completions>>,
    error: Arc<Mutex<Option<String>>>,
}
#[derive(Default)]
struct ActivityState {
    context: Option<Context>,
    paused: (u64, u64),
    quiesced: bool,
}
/// The lifecycle boundary includes texture setup, render submits, and presents.
/// CPU loading/encoding never holds this lock. UIKit can stop GPU work without
/// waiting for a shader compilation or for the town's scene-state lock.
pub(super) struct Activity {
    allowed: AtomicBool,
    epoch: AtomicU64,
    state: Mutex<ActivityState>,
}
impl Activity {
    pub fn new() -> Self {
        Self {
            allowed: AtomicBool::new(true),
            epoch: AtomicU64::new(1),
            state: Mutex::new(ActivityState::default()),
        }
    }
    pub fn active(&self) -> bool {
        self.allowed.load(Ordering::Acquire)
    }
    pub fn activate(&self) {
        let _guard = self.state.lock().unwrap();
        if !self.active() {
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
        self.allowed.store(true, Ordering::Release);
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    fn check(&self) -> Result<()> {
        if self.active() {
            Ok(())
        } else {
            Err(super::MedievalTownError::Suspended)
        }
    }
    fn run<T>(&self, work: impl FnOnce() -> T) -> Option<T> {
        if !self.active() {
            return None;
        }
        let mut state = self.state.lock().unwrap();
        if !self.active() {
            return None;
        }
        state.quiesced = false;
        Some(work())
    }
    fn register(&self, context: Context) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        self.check()?;
        if let Some(old) = state.context.as_ref() {
            Self::drain(old);
        }
        state.context = Some(context);
        state.paused = (0, 0);
        state.quiesced = false;
        Ok(())
    }
    fn drain(context: &Context) {
        // Fence after Metal presentation too, not just the render submission.
        let fence = context.queue.submit([]);
        if let Err(error) = context.device.poll(wgpu::PollType::Wait {
            submission_index: Some(fence),
            timeout: Some(std::time::Duration::from_secs(2)),
        }) {
            eprintln!("Medieval Town graphics pause: {error}");
            *context.error.lock().unwrap() = Some(error.to_string());
        }
    }
    pub fn suspend(&self) {
        self.allowed.store(false, Ordering::Release);
        let mut state = self.state.lock().unwrap();
        // Repeated notifications still join an in-progress pause, but must not
        // submit another fence once iOS has already entered the background.
        if !state.quiesced {
            if let Some(context) = state.context.as_ref() {
                Self::drain(context);
                let c = context.completions.lock().unwrap();
                let paused = (c.submitted, c.count);
                drop(c);
                state.paused = paused;
            }
            state.quiesced = true;
        }
    }
    pub fn paused(&self) -> (u64, u64) {
        self.state.lock().unwrap().paused
    }
    pub fn detach(&self) {
        self.state.lock().unwrap().context = None;
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    matrix: [[f32; 4]; 4],
    inverse_matrix: [[f32; 4]; 4],
    near_shadow: [[f32; 4]; 4],
    far_shadow: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    sun_color: [f32; 4],
    ambient: [f32; 4],
    fog: [f32; 4],
    settings: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    model: [f32; 16],
    rect: [f32; 4],
    uv: [f32; 4],
    base_color: [f32; 4],
    material: [f32; 4],
}
pub(super) struct Gpu {
    activity: Arc<Activity>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    needs_configure: bool,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    shadow_group: wgpu::BindGroup,
    near_shadow: wgpu::TextureView,
    far_shadow: wgpu::TextureView,
    shadow_pipelines: [wgpu::RenderPipeline; 2],
    cached_shadows: Option<[Mat4; 2]>,
    draw_groups: Vec<wgpu::BindGroup>,
    world: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    completions: Arc<Mutex<Completions>>,
    error: Arc<Mutex<Option<String>>>,
}
impl Gpu {
    pub fn new(
        layer: u64,
        width: u32,
        height: u32,
        activity: Arc<Activity>,
        mesh: &Mesh,
        progress: impl Fn(&'static str, f32),
    ) -> Result<Self> {
        activity.check()?;
        let started = Instant::now();
        let phase = |stage: &'static str, fraction: f32| {
            eprintln!(
                "Medieval Town graphics: {stage}; elapsed={:.3}s",
                started.elapsed().as_secs_f64()
            );
            progress(stage, fraction);
        };
        phase("Opening graphics", 0.);
        #[cfg(not(target_vendor = "apple"))]
        {
            let _ = (layer, width, height, mesh, activity);
            return Err(failed("This town view requires an Apple Metal display."));
        }
        #[cfg(target_vendor = "apple")]
        {
            if layer == 0 {
                return Err(failed("The town display could not open."));
            }
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::METAL;
            let instance = wgpu::Instance::new(descriptor);
            // The host retains this layer until detach and serializes every surface call.
            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    layer as *mut std::ffi::c_void,
                ))
            }
            .map_err(|error| {
                eprintln!("Medieval Town create_surface: {error}");
                failed("The town display could not open.")
            })?;
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .map_err(|error| {
                    eprintln!("Medieval Town request_adapter: {error}");
                    failed("Metal could not open the town on this device.")
                })?;
            eprintln!(
                "Medieval Town adapter: {:?}; limits: {:?}",
                adapter.get_info(),
                adapter.limits()
            );
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("Medieval Town"),
                    required_features: wgpu::Features::empty(),
                    // Match the app’s working offscreen renderer: the simulator may
                    // provide a lower unused limit than wgpu’s desktop defaults.
                    required_limits: adapter.limits(),
                    ..Default::default()
                }))
                .map_err(|error| {
                    eprintln!("Medieval Town request_device: {error}");
                    failed("Metal could not open the town on this device.")
                })?;
            let error = Arc::new(Mutex::new(None));
            let captured = error.clone();
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                eprintln!("Medieval Town graphics: {e}");
                *captured.lock().unwrap() = Some(e.to_string());
            }));
            let completions = Arc::new(Mutex::new(Completions::default()));
            activity.register(Context {
                device: device.clone(),
                queue: queue.clone(),
                completions: completions.clone(),
                error: error.clone(),
            })?;
            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb)
                .unwrap_or(caps.formats[0]);
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            };
            activity
                .run(|| surface.configure(&device, &config))
                .ok_or(super::MedievalTownError::Suspended)?;
            phase("Loading models", 0.08);
            let expanded;
            let vertex_data = if mesh.vertex_stride == 32 {
                expanded = expand_legacy_vertices(&mesh.vertices);
                &expanded
            } else {
                &mesh.vertices
            };
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Town vertices"),
                contents: bytemuck::cast_slice(vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Town indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            phase("Loading textures", 0.2);
            let (base_view, normal_view, mra_view) = if let Some(arrays) = &mesh.texture_arrays {
                let total = arrays.base_color.bytes.len()
                    + arrays.normal.bytes.len()
                    + arrays.mra.bytes.len();
                (
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town base colors",
                        &arrays.base_color,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        4,
                        |bytes| {
                            phase("Loading textures", texture_progress(bytes, total));
                        },
                    )?,
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town tangent normals",
                        &arrays.normal,
                        wgpu::TextureFormat::Rg8Unorm,
                        2,
                        |bytes| {
                            phase(
                                "Loading textures",
                                texture_progress(arrays.base_color.bytes.len() + bytes, total),
                            );
                        },
                    )?,
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town metallic roughness occlusion",
                        &arrays.mra,
                        wgpu::TextureFormat::Rgba8Unorm,
                        4,
                        |bytes| {
                            phase(
                                "Loading textures",
                                texture_progress(
                                    arrays.base_color.bytes.len()
                                        + arrays.normal.bytes.len()
                                        + bytes,
                                    total,
                                ),
                            );
                        },
                    )?,
                )
            } else {
                let atlas = super::TextureArray {
                    bytes: mesh.atlas.clone(),
                    width: mesh.atlas_width,
                    height: mesh.atlas_height,
                    layers: 1,
                    mip_count: 1,
                };
                let normal = super::TextureArray {
                    bytes: vec![128, 128],
                    width: 1,
                    height: 1,
                    layers: 1,
                    mip_count: 1,
                };
                let mra = super::TextureArray {
                    bytes: vec![0, 210, 255, 255],
                    width: 1,
                    height: 1,
                    layers: 1,
                    mip_count: 1,
                };
                let total = atlas.bytes.len() + normal.bytes.len() + mra.bytes.len();
                (
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town legacy colors",
                        &atlas,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        4,
                        |bytes| phase("Loading textures", texture_progress(bytes, total)),
                    )?,
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town legacy normals",
                        &normal,
                        wgpu::TextureFormat::Rg8Unorm,
                        2,
                        |bytes| {
                            phase(
                                "Loading textures",
                                texture_progress(atlas.bytes.len() + bytes, total),
                            );
                        },
                    )?,
                    upload_array(
                        &device,
                        &queue,
                        &activity,
                        "Town legacy roughness",
                        &mra,
                        wgpu::TextureFormat::Rgba8Unorm,
                        4,
                        |bytes| {
                            phase(
                                "Loading textures",
                                texture_progress(
                                    atlas.bytes.len() + normal.bytes.len() + bytes,
                                    total,
                                ),
                            );
                        },
                    )?,
                )
            };
            phase("Preparing materials", 0.65);
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Town anisotropic material filtering"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: 8,
                ..Default::default()
            });
            let near_shadow = make_shadow(&device, 2048, "Town near sun shadow");
            let far_shadow = make_shadow(&device, 1024, "Town distant sun shadow");
            let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Town shadow comparisons"),
                compare: Some(wgpu::CompareFunction::LessEqual),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let frame = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Town camera and sunlight"),
                size: std::mem::size_of::<FrameUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let global_entries = [
                uniform_binding(0),
                image_binding(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                image_binding(3),
                image_binding(4),
                shadow_binding(5),
                shadow_binding(6),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ];
            let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Town surface lighting layout"),
                entries: &global_entries,
            });
            // Shadow passes do not bind their own depth attachment for sampling.
            let shadow_globals =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Town shadow material layout"),
                    entries: &global_entries[..3],
                });
            let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Town shared instance transforms"),
                contents: bytemuck::cast_slice(&mesh.instances),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let local = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Town material and instances"),
                entries: &[
                    uniform_binding(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let frame_entries = [
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&mra_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&near_shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&far_shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ];
            let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Town surface lighting"),
                layout: &globals,
                entries: &frame_entries,
            });
            let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Town shadow materials"),
                layout: &shadow_globals,
                entries: &frame_entries[..3],
            });
            let mut materials: HashMap<[u8; 128], wgpu::BindGroup> = HashMap::new();
            let draw_groups = mesh
                .draws
                .iter()
                .map(|draw| {
                    let uniform = DrawUniform {
                        model: draw.model,
                        rect: draw.atlas_rect,
                        uv: [
                            draw.uv_scale[0],
                            draw.uv_scale[1],
                            draw.alpha_cutoff,
                            draw.material_layer as f32,
                        ],
                        base_color: [1.; 4],
                        material: [1.; 4],
                    };
                    let key: [u8; 128] = bytemuck::bytes_of(&uniform).try_into().unwrap();
                    if let Some(group) = materials.get(&key) {
                        return group.clone();
                    }
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Town material"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Town shared material"),
                        layout: &local,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: instances.as_entire_binding(),
                            },
                        ],
                    });
                    materials.insert(key, group.clone());
                    group
                })
                .collect();
            phase("Preparing sunlight", 0.75);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&globals), Some(&local)],
                immediate_size: 0,
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Town lighting and sky"),
                source: wgpu::ShaderSource::Wgsl(include_str!("town.wgsl").into()),
            });
            let attributes =
                wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x4];
            let buffers = [wgpu::VertexBufferLayout {
                array_stride: 48,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }];
            let pipeline = |sky: bool| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(if sky { "Town sky" } else { "Town architecture" }),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(if sky { "skyVertex" } else { "worldVertex" }),
                        compilation_options: Default::default(),
                        buffers: if sky { &[] } else { &buffers },
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(!sky),
                        depth_compare: Some(if sky {
                            wgpu::CompareFunction::Always
                        } else {
                            wgpu::CompareFunction::LessEqual
                        }),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(if sky { "skyFragment" } else { "worldFragment" }),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
            let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Town directional shadows"),
                bind_group_layouts: &[Some(&shadow_globals), Some(&local)],
                immediate_size: 0,
            });
            let shadow_pipeline = |entry: &'static str| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&shadow_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        buffers: &buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::LessEqual),
                        stencil: Default::default(),
                        bias: wgpu::DepthBiasState {
                            constant: 1,
                            slope_scale: 1.,
                            clamp: 0.,
                        },
                    }),
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("shadowFragment"),
                        compilation_options: Default::default(),
                        targets: &[],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
            let shadow_pipelines = [
                shadow_pipeline("nearShadowVertex"),
                shadow_pipeline("farShadowVertex"),
            ];
            phase("Preparing sunlight", 0.84);
            let world = pipeline(false);
            phase("Preparing sunlight", 0.92);
            let sky = pipeline(true);
            phase("Drawing the town", 0.98);
            let depth = make_depth(&device, width, height);
            phase("Drawing the town", 1.);
            activity.check()?;
            Ok(Self {
                activity,
                surface,
                device,
                queue,
                config,
                needs_configure: false,
                vertices,
                indices,
                frame,
                frame_group,
                shadow_group,
                near_shadow,
                far_shadow,
                shadow_pipelines,
                cached_shadows: None,
                draw_groups,
                world,
                sky,
                depth,
                completions,
                error,
            })
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.reconfigure();
    }
    fn reconfigure(&mut self) {
        let activity = self.activity.clone();
        self.needs_configure = activity
            .run(|| {
                self.surface.configure(&self.device, &self.config);
                self.depth = make_depth(&self.device, self.config.width, self.config.height);
            })
            .is_none();
    }
    pub fn poll(&self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
    }
    pub fn reset_metrics(&self) {
        let mut c = self.completions.lock().unwrap();
        c.generation += 1;
        c.submitted = 0;
        c.count = 0;
        c.start = None;
        c.times.clear();
    }
    pub fn completed(&self) -> u64 {
        self.poll();
        self.completions.lock().unwrap().count
    }
    pub fn live_fps(&self) -> f64 {
        self.poll();
        let completions = self.completions.lock().unwrap();
        let Some(start) = completions.start else {
            return 0.;
        };
        let now = start.elapsed().as_secs_f64();
        let window = now.min(1.);
        if window < 0.1 {
            return 0.;
        }
        let count = completions
            .times
            .iter()
            .rev()
            .take_while(|&&time| time > now - window)
            .count();
        count as f64 / window
    }
    pub fn cadence(&self) -> (f64, u64) {
        let c = self.completions.lock().unwrap();
        let mut periods: Vec<_> = c
            .times
            .iter()
            .zip(c.times.iter().skip(1))
            .map(|(a, b)| (b - a) * 1000.)
            .collect();
        let missed = periods.iter().filter(|&&dt| dt > 25.).count() as u64;
        periods.sort_by(f64::total_cmp);
        (
            periods
                .get((periods.len() as f64 * 0.95) as usize)
                .copied()
                .unwrap_or(0.),
            missed,
        )
    }
    pub fn draw(&mut self, camera: &Camera, mesh: &Mesh) -> Result<Option<(u32, u32)>> {
        if !self.activity.active() {
            return Ok(None);
        }
        if self.needs_configure {
            self.reconfigure();
            if self.needs_configure {
                return Ok(None);
            }
        }
        self.poll();
        if self.error.lock().unwrap().is_some() {
            return Err(failed(
                "The town display stopped working. Close it and try again.",
            ));
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                drop(frame);
                self.reconfigure();
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.reconfigure();
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Validation => {
                return Err(failed(
                    "The town display stopped working. Close it and try again.",
                ));
            }
        };
        let matrix = Mat4::perspective_rh(
            65f32.to_radians(),
            self.config.width as f32 / self.config.height as f32,
            0.07,
            4000.,
        ) * Mat4::look_to_rh(camera.eye(), camera.direction(), Vec3::Y);
        let sun = Vec3::new(-0.50, 0.78, -0.37).normalize();
        let shadows = shadow_matrices(camera.eye(), mesh.spawn, sun);
        self.queue.write_buffer(
            &self.frame,
            0,
            bytemuck::bytes_of(&FrameUniform {
                matrix: matrix.to_cols_array_2d(),
                inverse_matrix: matrix.inverse().to_cols_array_2d(),
                near_shadow: shadows[0].to_cols_array_2d(),
                far_shadow: shadows[1].to_cols_array_2d(),
                eye: camera.eye().extend(1.).to_array(),
                sun: sun.extend(1.).to_array(),
                sun_color: [4.3, 3.95, 3.35, 1.],
                ambient: [0.20, 0.26, 0.34, 1.],
                // Light aerial perspective leaves the distant castle’s materials visible.
                fog: [0.00045, 12., 0.012, 65.],
                settings: [0.95, 0., 0., 0.],
            }),
        );
        let target = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Town frame"),
            });
        let mut draws = 0;
        let mut triangles = 0;
        for cascade in 0..2 {
            if self
                .cached_shadows
                .is_some_and(|old| old[cascade] == shadows[cascade])
            {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if cascade == 0 {
                    "Town near sunlight"
                } else {
                    "Town distant sunlight"
                }),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: if cascade == 0 {
                        &self.near_shadow
                    } else {
                        &self.far_shadow
                    },
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipelines[cascade]);
            pass.set_bind_group(0, &self.shadow_group, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (i, draw) in mesh.draws.iter().enumerate() {
                if visible(shadows[cascade], &draw.bounds) {
                    pass.set_bind_group(1, &self.draw_groups[i], &[]);
                    pass.draw_indexed(
                        draw.first_index..draw.first_index + draw.index_count,
                        0,
                        draw.first_instance..draw.first_instance + draw.instance_count,
                    );
                    draws += 1;
                    triangles += draw.index_count / 3 * draw.instance_count;
                }
            }
        }
        {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view: &target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Town world"),
                color_attachments: &colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.sky);
            pass.set_bind_group(0, &self.frame_group, &[]);
            pass.set_bind_group(1, &self.draw_groups[0], &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.world);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (i, draw) in mesh.draws.iter().enumerate() {
                if !visible(matrix, &draw.bounds) {
                    continue;
                }
                pass.set_bind_group(1, &self.draw_groups[i], &[]);
                pass.draw_indexed(
                    draw.first_index..draw.first_index + draw.index_count,
                    0,
                    draw.first_instance..draw.first_instance + draw.instance_count,
                );
                draws += 1;
                triangles += draw.index_count / 3 * draw.instance_count;
            }
        }
        let commands = encoder.finish();
        let submitted = self.activity.run(|| {
            self.queue.submit(Some(commands));
            frame.present();
            let completions = self.completions.clone();
            let generation = {
                let mut c = completions.lock().unwrap();
                c.submitted += 1;
                c.generation
            };
            self.queue.on_submitted_work_done(move || {
                let mut c = completions.lock().unwrap();
                if c.generation != generation {
                    return;
                }
                let now = Instant::now();
                let start = *c.start.get_or_insert(now);
                c.count += 1;
                c.times.push_back(now.duration_since(start).as_secs_f64());
                if c.times.len() > 3600 {
                    c.times.pop_front();
                }
            });
        });
        if submitted.is_none() {
            return Ok(None);
        }
        self.cached_shadows = Some(shadows);
        Ok(Some((draws, triangles)))
    }
}
fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Town depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
fn visible(matrix: Mat4, bounds: &super::Bounds) -> bool {
    let corners: [Vec4; 8] = std::array::from_fn(|n| {
        matrix
            * Vec3::new(
                if n & 1 == 0 {
                    bounds.min[0]
                } else {
                    bounds.max[0]
                },
                if n & 2 == 0 {
                    bounds.min[1]
                } else {
                    bounds.max[1]
                },
                if n & 4 == 0 {
                    bounds.min[2]
                } else {
                    bounds.max[2]
                },
            )
            .extend(1.)
    });
    !(0..6).any(|plane| {
        corners.iter().all(|p| match plane {
            0 => p.x < -p.w,
            1 => p.x > p.w,
            2 => p.y < -p.w,
            3 => p.y > p.w,
            4 => p.z < 0.,
            _ => p.z > p.w,
        })
    })
}

fn uniform_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn image_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}
fn shadow_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
fn upload_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    activity: &Activity,
    label: &str,
    array: &super::TextureArray,
    format: wgpu::TextureFormat,
    channels: u32,
    progress: impl Fn(usize),
) -> Result<wgpu::TextureView> {
    activity.check()?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: array.width,
            height: array.height,
            depth_or_array_layers: array.layers,
        },
        mip_level_count: array.mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut offset = 0;
    for mip in 0..array.mip_count {
        let width = (array.width >> mip).max(1);
        let height = (array.height >> mip).max(1);
        let length = (width * height * array.layers * channels) as usize;
        for slab in texture_slabs(width, height, array.layers, channels) {
            activity
                .run(|| {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: mip,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: slab.first_row,
                                z: slab.first_layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &array.bytes[offset + slab.offset..offset + slab.offset + slab.bytes],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(width * channels),
                            rows_per_image: Some(slab.rows),
                        },
                        wgpu::Extent3d {
                            width,
                            height: slab.rows,
                            depth_or_array_layers: slab.layers,
                        },
                    );
                    // Queue writes otherwise retain every staging allocation until the
                    // first frame. Start bounded transfers while the CPU prepares more.
                    queue.submit([]);
                    let _ = device.poll(wgpu::PollType::Poll);
                })
                .ok_or(super::MedievalTownError::Suspended)?;
            progress(offset + slab.offset + slab.bytes);
        }
        offset += length;
    }
    Ok(texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    }))
}
const TEXTURE_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug)]
struct TextureSlab {
    offset: usize,
    bytes: usize,
    first_layer: u32,
    layers: u32,
    first_row: u32,
    rows: u32,
}
fn texture_slabs(width: u32, height: u32, layers: u32, channels: u32) -> Vec<TextureSlab> {
    let row_bytes = (width * channels) as usize;
    let layer_bytes = row_bytes * height as usize;
    let mut slabs = Vec::new();
    if layer_bytes <= TEXTURE_UPLOAD_BYTES {
        let per_slab = (TEXTURE_UPLOAD_BYTES / layer_bytes) as u32;
        for first_layer in (0..layers).step_by(per_slab as usize) {
            let count = per_slab.min(layers - first_layer);
            slabs.push(TextureSlab {
                offset: first_layer as usize * layer_bytes,
                bytes: count as usize * layer_bytes,
                first_layer,
                layers: count,
                first_row: 0,
                rows: height,
            });
        }
    } else {
        let per_slab = (TEXTURE_UPLOAD_BYTES / row_bytes).max(1) as u32;
        for first_layer in 0..layers {
            for first_row in (0..height).step_by(per_slab as usize) {
                let rows = per_slab.min(height - first_row);
                slabs.push(TextureSlab {
                    offset: first_layer as usize * layer_bytes + first_row as usize * row_bytes,
                    bytes: rows as usize * row_bytes,
                    first_layer,
                    layers: 1,
                    first_row,
                    rows,
                });
            }
        }
    }
    slabs
}
fn texture_progress(bytes: usize, total: usize) -> f32 {
    0.2 + 0.45 * (bytes as f64 / total.max(1) as f64).clamp(0., 1.) as f32
}
fn make_shadow(device: &wgpu::Device, resolution: u32, label: &str) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
fn expand_legacy_vertices(vertices: &[f32]) -> Vec<f32> {
    let mut expanded = Vec::with_capacity(vertices.len() / 8 * 12);
    for vertex in vertices.chunks_exact(8) {
        expanded.extend_from_slice(vertex);
        let n = Vec3::from_slice(&vertex[3..6]).normalize();
        let axis = if n.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
        expanded.extend_from_slice(&axis.cross(n).normalize().extend(1.).to_array());
    }
    expanded
}
fn shadow_matrices(eye: Vec3, spawn: Vec3, sun: Vec3) -> [Mat4; 2] {
    let right = Vec3::Y.cross(sun).normalize();
    let up = sun.cross(right);
    let focus = Vec3::new(eye.x, eye.y + 10., eye.z);
    // Whole-texel five-meter steps keep sunlight stable and reuse the near map
    // while walking within a tile; reconstructing the center avoids float drift.
    let quantum = 80. / 2048. * 128.;
    let center = [right, up, sun]
        .into_iter()
        .fold(Vec3::ZERO, |center, axis| {
            center + axis * ((focus.dot(axis) / quantum).round() * quantum)
        });
    [
        Mat4::orthographic_rh(-40., 40., -40., 40., 1., 800.)
            * Mat4::look_at_rh(center + sun * 350., center, Vec3::Y),
        {
            let center = spawn + Vec3::new(0., 55., -165.);
            Mat4::orthographic_rh(-480., 480., -480., 480., 1., 1800.)
                * Mat4::look_at_rh(center + sun * 800., center, Vec3::Y)
        },
    ]
}

#[cfg(test)]
mod shader_tests {
    use super::*;
    #[test]
    fn pause_joins_the_current_submit_and_rejects_queued_work() {
        let activity = Arc::new(Activity::new());
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = activity.clone();
        let submit = std::thread::spawn(move || {
            worker
                .run(|| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                })
                .unwrap();
        });
        entered_rx.recv().unwrap();
        let (paused_tx, paused_rx) = std::sync::mpsc::channel();
        let pauser = activity.clone();
        let pause = std::thread::spawn(move || {
            pauser.suspend();
            paused_tx.send(()).unwrap();
        });
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while activity.active() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!activity.active());
        assert_eq!(
            paused_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        assert!(
            activity
                .run(|| panic!("a queued upload must not run after pause"))
                .is_none()
        );
        release_tx.send(()).unwrap();
        paused_rx.recv().unwrap();
        submit.join().unwrap();
        pause.join().unwrap();
        activity.suspend();
        assert!(matches!(
            activity.check(),
            Err(super::super::MedievalTownError::Suspended)
        ));
        activity.activate();
        assert_eq!(activity.run(|| 42), Some(42));
    }
    #[test]
    fn texture_slabs_cover_every_mip_layer_and_tail_without_pixel_changes() {
        for (size, layers, channels) in
            [(512u32, 103, 4), (256, 103, 2), (256, 103, 4), (2048, 1, 4)]
        {
            for mip in 0..=size.ilog2() {
                let size = (size >> mip).max(1);
                let mut covered = 0;
                for slab in texture_slabs(size, size, layers, channels) {
                    assert_eq!(slab.offset, covered);
                    assert!(slab.bytes > 0 && slab.bytes <= TEXTURE_UPLOAD_BYTES);
                    assert!(slab.first_layer + slab.layers <= layers);
                    assert!(slab.first_row + slab.rows <= size);
                    assert_eq!(
                        slab.bytes,
                        (size * slab.rows * slab.layers * channels) as usize
                    );
                    assert!(slab.layers == 1 || (slab.first_row == 0 && slab.rows == size));
                    assert_eq!(
                        slab.offset,
                        ((slab.first_layer * size * size + slab.first_row * size) * channels)
                            as usize
                    );
                    covered += slab.bytes;
                }
                assert_eq!(covered, (size * size * layers * channels) as usize);
            }
        }
    }
    #[test]
    fn texture_progress_counts_all_material_bytes_and_stays_in_its_phase() {
        let base = 144_004_300;
        let normal = 18_000_486;
        let mra = 36_000_972;
        let total = base + normal + mra;
        let samples = [0, base / 2, base, base + normal, total, total + 1]
            .map(|bytes| texture_progress(bytes, total));
        assert!(samples.windows(2).all(|values| values[0] <= values[1]));
        assert!((samples[0] - 0.2).abs() < 0.0001);
        assert!((samples[4] - 0.65).abs() < 0.0001);
        assert_eq!(samples[4], samples[5]);
        assert!((0.2..=0.65).contains(&texture_progress(0, 0)));
    }
    #[test]
    fn shader_uniform_offsets_match_native_buffers() {
        let module = naga::front::wgsl::parse_str(include_str!("town.wgsl")).unwrap();
        for (name, size, offsets) in [
            (
                "Frame",
                352,
                vec![0, 64, 128, 192, 256, 272, 288, 304, 320, 336],
            ),
            ("Draw", 128, vec![0, 64, 80, 96, 112]),
            ("Instance", 112, vec![0, 64, 80, 96]),
        ] {
            let ty = module
                .types
                .iter()
                .map(|(_, ty)| ty)
                .find(|ty| ty.name.as_deref() == Some(name))
                .unwrap();
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("uniform must be a struct")
            };
            assert_eq!(*span, size);
            assert_eq!(
                members
                    .iter()
                    .map(|member| member.offset)
                    .collect::<Vec<_>>(),
                offsets
            );
        }
    }
    #[test]
    fn uniform_layout_and_shadow_ranges_match_wgsl() {
        assert_eq!(std::mem::size_of::<FrameUniform>(), 352);
        assert_eq!(std::mem::size_of::<DrawUniform>(), 128);
        let sun = Vec3::new(-0.5, 0.78, -0.37).normalize();
        let eye = Vec3::new(6., 3.37, 20.);
        let matrices = shadow_matrices(eye, eye, sun);
        for matrix in matrices {
            assert!(matrix.is_finite());
        }
        let origin = matrices[0].project_point3(eye);
        assert!(origin.x.abs() < 1. && origin.y.abs() < 1. && (0. ..1.).contains(&origin.z));
        // The castle remains inside the far sunlight volume, rather than being
        // whitened by fog to hide absent lighting.
        let castle = matrices[1].project_point3(Vec3::new(0., 100., -380.));
        assert!(castle.x.abs() < 1. && castle.y.abs() < 1. && (0. ..1.).contains(&castle.z));
        assert_eq!(matrices, shadow_matrices(eye + Vec3::X * 0.001, eye, sun));
    }
    #[test]
    fn frustum_culling_keeps_intersections_and_rejects_behind_camera() {
        let matrix = Mat4::perspective_rh(65f32.to_radians(), 1., 0.1, 100.);
        assert!(visible(
            matrix,
            &super::super::Bounds {
                min: [-1., -1., -4.],
                max: [1., 1., -2.]
            }
        ));
        assert!(!visible(
            matrix,
            &super::super::Bounds {
                min: [-1., -1., 2.],
                max: [1., 1., 4.]
            }
        ));
        assert!(!visible(
            matrix,
            &super::super::Bounds {
                min: [500., -1., -4.],
                max: [501., 1., -2.]
            }
        ));
    }
}
