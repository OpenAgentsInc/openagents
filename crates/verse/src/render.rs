//! The wgpu renderer: one shader, a face pipeline, and a line pipeline.
//!
//! Each frame clears to the near-black field, draws faces with a depth
//! bias so coincident edges win, then draws amber lines on top. The static
//! world uploads once. The avatar rewrites a small dynamic buffer every
//! frame. [`Renderer`] presents to a window; `capture` renders the same
//! scene to a PNG without one.

#[cfg(feature = "capture")]
use std::path::Path;
#[cfg(feature = "desktop")]
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
#[cfg(feature = "desktop")]
use winit::window::Window;

use crate::mesh::{Mesh, Vertex};
use crate::palette;
use crate::ui::{Atlas, UiBatch, UiVertex};

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
#[cfg(feature = "capture")]
const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const INITIAL_DYNAMIC_BYTES: u64 = 256 * 1024;
// A full 512-entity crowd needs about 18 MiB of faces with current geometry.
// Each stream grows independently; the line stream usually needs much less.
const MAX_DYNAMIC_BYTES: u64 = 32 * 1024 * 1024;
const UI_BYTES: u64 = 2 * 1024 * 1024;
/// Distance where fog starts, in meters.
pub const FOG_START: f32 = 60.0;
/// Distance where fog is total, in meters.
pub const FOG_END: f32 = 250.0;

/// Where the camera is for one frame.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Projection times view.
    pub view_proj: Mat4,
    /// Eye position, for fog.
    pub eye: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    fog: [f32; 4],
}

/// A vertex buffer and how many vertices it holds.
struct Batch {
    buffer: wgpu::Buffer,
    count: u32,
    capacity: u64,
}

/// Pipelines and buffers for one color format and sample count.
struct Scene {
    samples: u32,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    faces: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    world_faces: Batch,
    world_lines: Batch,
    dynamic_faces: Batch,
    dynamic_lines: Batch,
    ui_pipeline: wgpu::RenderPipeline,
    ui_bind_group: wgpu::BindGroup,
    ui_screen: wgpu::Buffer,
    ui: Batch,
}

/// Color and depth attachments at one size.
struct Targets {
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    size: [f32; 2],
}

/// Bounded renderer configuration. A requested 4x sample count falls back to
/// one sample when the adapter does not support it for the chosen format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    pub sample_count: u32,
    pub max_extent: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_count: 4,
            max_extent: 8192,
        }
    }
}

impl RenderOptions {
    fn validate(self) -> Result<Self, String> {
        if !matches!(self.sample_count, 1 | 4) || self.max_extent == 0 || self.max_extent > 8192 {
            return Err(
                "renderer requires 1 or 4 samples and an extent bound from 1 to 8192".into(),
            );
        }
        Ok(self)
    }
}

/// Whether a requested frame reached the native surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawStatus {
    Presented,
    Skipped(&'static str),
    Error(String),
}

/// GPU state for one surface lifetime. The platform owns scheduling and input.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    scene: Scene,
    targets: Targets,
    max_extent: u32,
    drawable: bool,
}

impl Renderer {
    /// Creates a desktop surface and uploads the shared world geometry.
    #[cfg(feature = "desktop")]
    pub fn new(window: Arc<Window>, world: &Mesh, atlas: &Atlas) -> Result<Self, String> {
        let instance = instance();
        let size = window.inner_size();
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("cannot create a surface: {e}"))?;
        Self::from_surface(
            instance,
            surface,
            size.width,
            size.height,
            world,
            atlas,
            RenderOptions::default(),
        )
    }

    /// Creates an Apple surface from the native host's CAMetalLayer.
    ///
    /// # Safety
    /// `layer` must point to a valid CAMetalLayer on its owning UI thread. The
    /// native host must keep the layer alive and attached for this renderer's
    /// entire lifetime, and must drop the renderer before destroying the layer.
    /// All calls and teardown must stay on that owning thread.
    #[cfg(target_vendor = "apple")]
    pub unsafe fn from_metal_layer(
        layer: *mut core::ffi::c_void,
        width: u32,
        height: u32,
        world: &Mesh,
        atlas: &Atlas,
        options: RenderOptions,
    ) -> Result<Self, String> {
        if layer.is_null() {
            return Err("native Metal layer is null".into());
        }
        options.validate()?;
        validate_extent(width, height, options.max_extent)?;
        let instance = instance();
        // SAFETY: the caller owns the layer lifetime and UI-thread confinement.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
        }
        .map_err(|e| format!("cannot create a Metal surface: {e}"))?;
        Self::from_surface(instance, surface, width, height, world, atlas, options)
    }

    /// Builds the same renderer around a platform-created surface.
    #[allow(clippy::too_many_arguments)]
    pub fn from_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        world: &Mesh,
        atlas: &Atlas,
        options: RenderOptions,
    ) -> Result<Self, String> {
        let options = options.validate()?;
        validate_extent(width, height, options.max_extent)?;
        let (adapter, device, queue) = open(&instance, Some(&surface))?;
        let max_extent = options
            .max_extent
            .min(device.limits().max_texture_dimension_2d);
        validate_extent(width, height, max_extent)?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or("the surface reports no formats")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let scene = Scene::new(
            &device,
            &queue,
            &adapter,
            format,
            world,
            atlas,
            options.sample_count,
        );
        let targets = Targets::new(&device, format, width, height, scene.samples);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            scene,
            targets,
            max_extent,
            drawable: true,
        })
    }

    #[must_use]
    pub fn size(&self) -> [f32; 2] {
        if self.drawable {
            [self.config.width as f32, self.config.height as f32]
        } else {
            [0.0, 0.0]
        }
    }

    #[must_use]
    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    #[must_use]
    pub fn sample_count(&self) -> u32 {
        self.scene.samples
    }

    /// A zero extent suspends presentation until a valid size returns.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            self.drawable = false;
            return Ok(());
        }
        validate_extent(width, height, self.max_extent)?;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.targets = Targets::new(
            &self.device,
            self.config.format,
            width,
            height,
            self.scene.samples,
        );
        self.drawable = true;
        Ok(())
    }

    /// Draws one frame. Lost surfaces require a fresh native attachment;
    /// skipped frames never report that they were presented.
    pub fn draw(&mut self, view: View, dynamic: &Mesh, ui: &UiBatch) -> DrawStatus {
        if !self.drawable {
            return DrawStatus::Skipped("surface has no drawable extent");
        }
        if let Err(error) = validate_frame(view, dynamic, ui) {
            return DrawStatus::Error(error);
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return DrawStatus::Skipped("surface was reconfigured");
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                return DrawStatus::Skipped("drawable timed out");
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return DrawStatus::Skipped("surface is occluded");
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return DrawStatus::Error("surface was lost; attach a new native surface".into());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return DrawStatus::Error("surface validation failed".into());
            }
        };
        let output = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("verse frame"),
            });
        self.scene.encode(
            &self.device,
            &self.queue,
            &mut encoder,
            &output,
            &self.targets,
            view,
            dynamic,
            ui,
        );
        self.queue.submit([encoder.finish()]);
        frame.present();
        DrawStatus::Presented
    }
}

fn validate_extent(width: u32, height: u32, limit: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > limit
        || height > limit
        || u64::from(width) * u64::from(height) > rust_native::surface::MAX_PIXELS
    {
        return Err(format!(
            "drawable extent must be within 1..={limit} pixels per dimension"
        ));
    }
    Ok(())
}

fn validate_frame(view: View, dynamic: &Mesh, ui: &UiBatch) -> Result<(), String> {
    if !view.view_proj.is_finite() || !view.eye.is_finite() {
        return Err("camera contains nonfinite values".into());
    }
    let mesh_bytes = |n: usize| {
        n.checked_mul(std::mem::size_of::<Vertex>())
            .is_some_and(|n| n <= MAX_DYNAMIC_BYTES as usize)
    };
    if !mesh_bytes(dynamic.faces.len())
        || !mesh_bytes(dynamic.lines.len())
        || !ui
            .vertices
            .len()
            .checked_mul(std::mem::size_of::<UiVertex>())
            .is_some_and(|n| n <= UI_BYTES as usize)
    {
        return Err("frame exceeds retained GPU geometry or HUD capacity".into());
    }
    Ok(())
}

/// Renders one frame without a window and writes it to `path` as a PNG.
///
/// # Errors
///
/// Returns a message when no GPU is available or the file cannot be written.
#[cfg(feature = "capture")]
#[allow(clippy::too_many_arguments)]
pub fn capture(
    path: &Path,
    width: u32,
    height: u32,
    world: &Mesh,
    view: View,
    dynamic: &Mesh,
    ui: &UiBatch,
    atlas: &Atlas,
) -> Result<(), String> {
    validate_extent(width, height, RenderOptions::default().max_extent)?;
    validate_frame(view, dynamic, ui)?;
    let instance = instance();
    let (adapter, device, queue) = open(&instance, None)?;
    validate_extent(width, height, device.limits().max_texture_dimension_2d)?;
    let mut scene = Scene::new(&device, &queue, &adapter, CAPTURE_FORMAT, world, atlas, 4);
    let targets = Targets::new(&device, CAPTURE_FORMAT, width, height, scene.samples);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse capture"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: CAPTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("verse capture readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("verse capture"),
    });
    scene.encode(
        &device,
        &queue,
        &mut encoder,
        &output,
        &targets,
        view,
        dynamic,
        ui,
    );
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        extent(width, height),
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|e| format!("the GPU did not finish: {e}"))?;
    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height as usize {
        let start = y * row as usize;
        pixels.extend_from_slice(&mapped[start..start + width as usize * 4]);
    }
    drop(mapped);
    readback.unmap();

    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&pixels))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
}

fn open(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: surface,
    }))
    .map_err(|e| format!("no graphics adapter: {e}"))?;
    let required_limits = scene_limits(adapter.limits())?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("verse"),
        required_limits,
        ..Default::default()
    }))
    .map_err(|e| format!("no graphics device: {e}"))?;
    Ok((adapter, device, queue))
}

// The world shader passes color, world position, and fog at locations 0..2;
// the HUD shader passes only UV and color. Neither uses compute or storage.
fn scene_limits(available: wgpu::Limits) -> Result<wgpu::Limits, String> {
    let mut required = wgpu::Limits::downlevel_defaults().using_resolution(available.clone());
    required.max_inter_stage_shader_variables = 3;
    let mut unsupported = Vec::new();
    required.check_limits_with_fail_fn(&available, false, |name, requested, supported| {
        unsupported.push(format!(
            "{name} needs {requested}, adapter supports {supported}"
        ));
    });
    if unsupported.is_empty() {
        Ok(required)
    } else {
        Err(format!(
            "graphics adapter cannot run the Verse scene: {}",
            unsupported.join("; ")
        ))
    }
}

impl Scene {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
        world: &Mesh,
        atlas: &Atlas,
        requested_samples: u32,
    ) -> Self {
        let samples = if requested_samples == 4
            && adapter
                .get_texture_format_features(format)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse globals"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse globals"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline = |topology, bias: wgpu::DepthBiasState, label| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x3,
                            2 => Float32,
                        ],
                    }],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias,
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
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let faces = pipeline(
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::DepthBiasState {
                constant: 4,
                slope_scale: 2.0,
                clamp: 0.0,
            },
            "verse faces",
        );
        let lines = pipeline(
            wgpu::PrimitiveTopology::LineList,
            wgpu::DepthBiasState::default(),
            "verse lines",
        );

        let upload = |vertices: &[Vertex], label| Batch {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            count: vertices.len() as u32,
            capacity: std::mem::size_of_val(vertices) as u64,
        };
        let dynamic = |label| Batch {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: INITIAL_DYNAMIC_BYTES,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            count: 0,
            capacity: INITIAL_DYNAMIC_BYTES,
        };
        let (ui_pipeline, ui_bind_group, ui_screen) =
            ui_pipeline(device, queue, format, samples, atlas);
        let ui = Batch {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("verse ui"),
                size: UI_BYTES,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            count: 0,
            capacity: UI_BYTES,
        };
        Self {
            ui_pipeline,
            ui_bind_group,
            ui_screen,
            ui,
            samples,
            globals,
            bind_group,
            faces,
            lines,
            world_faces: upload(&world.faces, "verse world faces"),
            world_lines: upload(&world.lines, "verse world lines"),
            dynamic_faces: dynamic("verse dynamic faces"),
            dynamic_lines: dynamic("verse dynamic lines"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &Targets,
        view: View,
        dynamic: &Mesh,
        ui: &UiBatch,
    ) {
        let field = palette::field();
        queue.write_buffer(
            &self.ui_screen,
            0,
            bytemuck::cast_slice(&[targets.size[0], targets.size[1], 0.0, 0.0]),
        );
        let ui_bytes: &[u8] = bytemuck::cast_slice(&ui.vertices);
        let fit = ui_bytes.len();
        if fit > 0 {
            queue.write_buffer(&self.ui.buffer, 0, &ui_bytes[..fit]);
        }
        self.ui.count = (fit / std::mem::size_of::<UiVertex>()) as u32;
        let eye = view.eye;
        let globals = Globals {
            view_proj: view.view_proj.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, FOG_START],
            fog: [field[0], field[1], field[2], FOG_END],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        write(device, queue, &mut self.dynamic_faces, &dynamic.faces);
        write(device, queue, &mut self.dynamic_lines, &dynamic.lines);

        let (target, resolve) = match &targets.msaa {
            Some(msaa) => (msaa, Some(output)),
            None => (output, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("verse"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(field[0]),
                        g: f64::from(field[1]),
                        b: f64::from(field[2]),
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
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
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.faces);
        for batch in [&self.world_faces, &self.dynamic_faces] {
            draw_batch(&mut pass, batch);
        }
        pass.set_pipeline(&self.lines);
        for batch in [&self.world_lines, &self.dynamic_lines] {
            draw_batch(&mut pass, batch);
        }
        if self.ui.count > 0 {
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_bind_group(0, &self.ui_bind_group, &[]);
            draw_batch(&mut pass, &self.ui);
        }
    }
}

impl Targets {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        samples: u32,
    ) -> Self {
        let texture = |format, label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent(width, height),
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        Self {
            msaa: (samples > 1).then(|| texture(format, "verse msaa")),
            depth: texture(DEPTH, "verse depth"),
            size: [width as f32, height as f32],
        }
    }
}

/// The UI pipeline: screen-space quads sampling the glyph atlas, alpha
/// blended, drawn last with no depth test.
fn ui_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    samples: u32,
    atlas: &Atlas,
) -> (wgpu::RenderPipeline, wgpu::BindGroup, wgpu::Buffer) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse glyph atlas"),
        size: extent(atlas.width, atlas.height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &atlas.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas.width),
            rows_per_image: Some(atlas.height),
        },
        extent(atlas.width, atlas.height),
    );
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
        source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
        depth_stencil: Some(wgpu::DepthStencilState {
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
    });
    (pipeline, bind_group, screen)
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn draw_batch(pass: &mut wgpu::RenderPass<'_>, batch: &Batch) {
    if batch.count > 0 {
        pass.set_vertex_buffer(0, batch.buffer.slice(..));
        pass.draw(0..batch.count, 0..1);
    }
}

fn dynamic_capacity(bytes: usize) -> Option<u64> {
    let requested = u64::try_from(bytes).ok()?;
    if requested > MAX_DYNAMIC_BYTES {
        return None;
    }
    Some(requested.max(INITIAL_DYNAMIC_BYTES).next_power_of_two())
}

fn write(device: &wgpu::Device, queue: &wgpu::Queue, batch: &mut Batch, vertices: &[Vertex]) {
    let bytes: &[u8] = bytemuck::cast_slice(vertices);
    if bytes.len() as u64 > batch.capacity {
        // Frame validation already admitted the complete batch under the cap.
        let capacity = dynamic_capacity(bytes.len()).expect("validated dynamic frame capacity");
        batch.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse resized dynamic geometry"),
            size: capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        batch.capacity = capacity;
    }
    let fit = bytes.len();
    if fit > 0 {
        queue.write_buffer(&batch.buffer, 0, &bytes[..fit]);
    }
    batch.count = (fit / std::mem::size_of::<Vertex>()) as u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_limits_accept_mobile_varyings_without_asking_for_unused_desktop_limits() {
        let mut mobile = wgpu::Limits::downlevel_defaults();
        mobile.max_texture_dimension_2d = 4096;
        let required = scene_limits(mobile.clone()).unwrap();
        assert_eq!(required.max_inter_stage_shader_variables, 3);
        assert_eq!(required.max_texture_dimension_2d, 4096);
        assert!(required.check_limits(&mobile));
        mobile.max_inter_stage_shader_variables = 2;
        assert!(
            scene_limits(mobile)
                .unwrap_err()
                .contains("max_inter_stage_shader_variables")
        );
    }

    #[test]
    fn renderer_options_and_extents_are_bounded_before_gpu_work() {
        assert!(RenderOptions::default().validate().is_ok());
        assert!(
            RenderOptions {
                sample_count: 2,
                max_extent: 8192
            }
            .validate()
            .is_err()
        );
        assert!(
            RenderOptions {
                sample_count: 1,
                max_extent: 0
            }
            .validate()
            .is_err()
        );
        assert!(validate_extent(0, 100, 8192).is_err());
        assert!(validate_extent(8193, 100, 8192).is_err());
        assert!(validate_extent(1024, 768, 8192).is_ok());
    }

    #[test]
    fn dynamic_buffers_grow_to_hold_the_full_bounded_crowd() {
        assert_eq!(dynamic_capacity(1), Some(INITIAL_DYNAMIC_BYTES));
        assert_eq!(
            dynamic_capacity(INITIAL_DYNAMIC_BYTES as usize + 1),
            Some(2 * INITIAL_DYNAMIC_BYTES)
        );
        assert_eq!(
            dynamic_capacity(MAX_DYNAMIC_BYTES as usize),
            Some(MAX_DYNAMIC_BYTES)
        );
        assert_eq!(dynamic_capacity(MAX_DYNAMIC_BYTES as usize + 1), None);
        let runtime = crate::runtime::WorldRuntime::new();
        let avatar = crate::avatar::mesh(&runtime.player, &runtime.gait);
        let agent = runtime.agent.mesh();
        let mut crowd = runtime.dynamic_mesh();
        // Each remote entity can be an avatar or spade. Bound each geometry
        // stream by the larger shape, which also covers every mixed crowd.
        for _ in 0..512 {
            crowd
                .faces
                .extend_from_slice(if avatar.faces.len() > agent.faces.len() {
                    &avatar.faces
                } else {
                    &agent.faces
                });
            crowd
                .lines
                .extend_from_slice(if avatar.lines.len() > agent.lines.len() {
                    &avatar.lines
                } else {
                    &agent.lines
                });
        }
        assert!(std::mem::size_of_val(crowd.lines.as_slice()) > INITIAL_DYNAMIC_BYTES as usize);
        assert!(
            validate_frame(runtime.view(1.0), &crowd, &UiBatch::default()).is_ok(),
            "full crowd bytes: faces {}, lines {}",
            std::mem::size_of_val(crowd.faces.as_slice()),
            std::mem::size_of_val(crowd.lines.as_slice())
        );
    }

    #[test]
    fn over_capacity_frames_fail_instead_of_silently_clipping() {
        let view = View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        };
        let mut mesh = Mesh::default();
        let mut ui = UiBatch::default();
        assert!(validate_frame(view, &mesh, &ui).is_ok());
        mesh.lines.resize(
            MAX_DYNAMIC_BYTES as usize / std::mem::size_of::<Vertex>() + 1,
            Vertex {
                pos: [0.0; 3],
                color: [0.0; 3],
                fog: 0.0,
            },
        );
        assert!(validate_frame(view, &mesh, &ui).is_err());
        mesh.lines.clear();
        ui.vertices.resize(
            UI_BYTES as usize / std::mem::size_of::<UiVertex>() + 1,
            UiVertex {
                pos: [0.0; 2],
                uv: [0.0; 2],
                color: [0.0; 4],
            },
        );
        assert!(validate_frame(view, &mesh, &ui).is_err());
    }
}
