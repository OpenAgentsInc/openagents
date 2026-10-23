use super::{Camera, Mesh, Result, failed};
use glam::{Mat4, Vec3, Vec4};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};
use wgpu::util::DeviceExt;
#[derive(Default)]
struct Completions {
    generation: u64,
    count: u64,
    start: Option<Instant>,
    times: VecDeque<f64>,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    matrix: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    fog: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    model: [f32; 16],
    rect: [f32; 4],
    uv: [f32; 4],
}
pub(super) struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    draw_groups: Vec<wgpu::BindGroup>,
    world: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    completions: Arc<Mutex<Completions>>,
    error: Arc<Mutex<Option<String>>>,
}
impl Gpu {
    pub fn new(layer: u64, width: u32, height: u32, mesh: &Mesh) -> Result<Self> {
        #[cfg(not(target_vendor = "apple"))]
        {
            let _ = (layer, width, height, mesh);
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
            surface.configure(&device, &config);
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Town vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Town indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let atlas = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Town atlas"),
                size: wgpu::Extent3d {
                    width: mesh.atlas_width,
                    height: mesh.atlas_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mesh.atlas,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mesh.atlas_width * 4),
                    rows_per_image: Some(mesh.atlas_height),
                },
                wgpu::Extent3d {
                    width: mesh.atlas_width,
                    height: mesh.atlas_height,
                    depth_or_array_layers: 1,
                },
            );
            let view = atlas.create_view(&Default::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Town texture filtering"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            });
            let frame = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Town camera"),
                size: std::mem::size_of::<FrameUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
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
            let local = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
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
                label: None,
                layout: &globals,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: frame.as_entire_binding(),
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
            let draw_groups = mesh
                .draws
                .iter()
                .map(|draw| {
                    let uniform = DrawUniform {
                        model: draw.model,
                        rect: draw.atlas_rect,
                        uv: [draw.uv_scale[0], draw.uv_scale[1], draw.alpha_cutoff, 0.],
                    };
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Town material"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &local,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        }],
                    })
                })
                .collect();
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&globals), Some(&local)],
                immediate_size: 0,
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Town lighting and sky"),
                source: wgpu::ShaderSource::Wgsl(include_str!("town.wgsl").into()),
            });
            let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2];
            let buffers = [wgpu::VertexBufferLayout {
                array_stride: 32,
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
            let world = pipeline(false);
            let sky = pipeline(true);
            let depth = make_depth(&device, width, height);
            Ok(Self {
                surface,
                device,
                queue,
                config,
                vertices,
                indices,
                frame,
                frame_group,
                draw_groups,
                world,
                sky,
                depth,
                completions: Arc::new(Mutex::new(Completions::default())),
                error,
            })
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth = make_depth(&self.device, width, height);
    }
    pub fn poll(&self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
    }
    pub fn reset_metrics(&self) {
        let mut c = self.completions.lock().unwrap();
        c.generation += 1;
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
                self.surface.configure(&self.device, &self.config);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
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
            700.,
        ) * Mat4::look_to_rh(camera.eye(), camera.direction(), Vec3::Y);
        self.queue.write_buffer(
            &self.frame,
            0,
            bytemuck::bytes_of(&FrameUniform {
                matrix: matrix.to_cols_array_2d(),
                eye: camera.eye().extend(1.).to_array(),
                sun: [-0.5, 0.8, -0.3, 0.],
                fog: [45., 420., 0., 0.],
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
                    0..1,
                );
                draws += 1;
                triangles += draw.index_count / 3;
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        let completions = self.completions.clone();
        let generation = completions.lock().unwrap().generation;
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
    let corners: Vec<Vec4> = (0..8)
        .map(|n| {
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
        })
        .collect();
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
