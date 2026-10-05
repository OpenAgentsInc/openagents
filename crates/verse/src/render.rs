//! The wgpu renderer: one shader, a face pipeline, and a line pipeline.
//!
//! Each frame clears to the active zone atmosphere, draws faces with a depth
//! bias so coincident edges win, then draws lines on top. A frame whose
//! dynamic mesh carries a [`crate::pbr::Sky`] takes the physical path in
//! `pbr/gpu.rs` instead: lit geometry in real units, the sky at infinity,
//! shadows, bounce light, bloom, exposure, and tone mapping. Static geometry uploads
//! once per zone. Animated models rewrite bounded dynamic buffers each frame. [`Renderer`] presents to a window; `capture` renders the same
//! scene to a PNG without one.

#[cfg(feature = "capture")]
use std::path::Path;
#[cfg(feature = "desktop")]
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
#[cfg(test)]
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
#[cfg(feature = "desktop")]
use winit::window::Window;

use crate::mesh::{Mesh, Vertex};
use crate::pbr::LitVertex;
use crate::pbr::gpu::{Batches, Capability, Photo, PhotoTargets, Stage, TexturedGpu};
use crate::pbr::textured::{BakedVertices, Merged, TexturedScene};
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
/// Where the bare world's fog starts, in meters. With nothing but the grid
/// to hide, the fade begins near the player, so the distant grid dims
/// gradually toward the horizon instead of drawing as bright as the near
/// lines.
pub const BARE_FOG_START: f32 = 6.0;
/// Where the bare world's fog is total, in meters: inside the grid's edge,
/// so the grid ends in the field without a visible border.
pub const BARE_FOG_END: f32 = 110.0;

pub use verse_engine::presentation::View;

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

fn dynamic_batch(device: &wgpu::Device, label: &str) -> Batch {
    Batch {
        buffer: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: INITIAL_DYNAMIC_BYTES,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        count: 0,
        capacity: INITIAL_DYNAMIC_BYTES,
    }
}

/// Pipelines and buffers for one color format and sample count.
struct Scene {
    atmosphere: crate::zones::Atmosphere,
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
    /// The HUD pipeline for the physical path: one sample and no depth.
    ui_photo: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    capability: Capability,
    /// Physically lit zone geometry, uploaded with the world.
    world_lit: (wgpu::Buffer, u32),
    /// The world's textured meshes, merged and waiting for the physical
    /// path's first frame, which uploads them.
    textured_pending: Option<PreparedTextured>,
    /// The world's textured meshes on the GPU.
    textured: Option<TexturedGpu>,
    /// Where a background light bake delivers the textured meshes' baked
    /// vertices, written over the uploaded ones when they arrive.
    textured_baked: Option<BakedVertices>,
    /// The dynamic mesh's figure on the GPU, with the scene it was uploaded
    /// from; a different scene uploads again.
    figure: Option<(std::sync::Arc<TexturedScene>, TexturedGpu)>,
    /// Created on the first frame that carries a sky.
    photo: Option<Photo>,
    photo_failed: bool,
    /// Extended-range headroom for photographic highlights, 1.0 in SDR.
    headroom: f32,
}

/// Color and depth attachments at one size.
struct Targets {
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    size: [f32; 2],
    photo: Option<PhotoTargets>,
}

/// Bounded renderer configuration. A requested 4x sample count falls back to
/// one sample when the adapter does not support it for the chosen format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    pub sample_count: u32,
    pub max_extent: u32,
    /// Request an extended-range surface (RGBA16F, linear) for an HDR
    /// display. The host must give its layer an extended linear sRGB color
    /// space; [`Renderer::hdr`] reports whether the surface accepted it.
    pub hdr: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_count: 4,
            max_extent: 8192,
            hdr: false,
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
    /// The sRGB encoding pass into a linear surface, on OpenGL ES.
    present: Option<Present>,
    max_extent: u32,
    drawable: bool,
    hdr: bool,
    /// A panel drawn over the finished frame, created when first shown.
    overlay: Option<crate::overlay::Overlay>,
    /// Why the physical renderer is unavailable, when known at creation.
    physical_error: Option<String>,
}

/// OpenGL ES presentation. wgpu's GLES backend offers an sRGB surface only
/// through an EGL window colorspace, which some drivers (including the
/// Android emulator's) accept and then ignore, so frames would reach the
/// display without their sRGB encoding. The frame is drawn exactly as on
/// other backends into an sRGB texture, which every OpenGL ES 3.0 device can
/// render to, and `present.wgsl` encodes it into a linear surface.
struct Present {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    frame: wgpu::TextureView,
    group: wgpu::BindGroup,
}

impl Present {
    /// The format the scene draws into.
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    fn new(device: &wgpu::Device, surface: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse present"),
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse present"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse present"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("verse present"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse present"),
            ..Default::default()
        });
        let (frame, group) = Self::frame(device, &layout, &sampler, width, height);
        Self {
            pipeline,
            layout,
            sampler,
            frame,
            group,
        }
    }

    fn frame(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> (wgpu::TextureView, wgpu::BindGroup) {
        let frame = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("verse present frame"),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse present"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&frame),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (frame, group)
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        (self.frame, self.group) = Self::frame(device, &self.layout, &self.sampler, width, height);
    }

    fn encode(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("verse present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
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
        #[cfg(target_os = "macos")]
        let options = RenderOptions {
            hdr: crate::edr::potential_headroom() > 1.01 && std::env::var_os("VERSE_SDR").is_none(),
            ..RenderOptions::default()
        };
        #[cfg(not(target_os = "macos"))]
        let options = RenderOptions::default();
        let renderer = Self::from_surface(
            instance,
            surface,
            size.width,
            size.height,
            world,
            atlas,
            options,
        )?;
        #[cfg(target_os = "macos")]
        if renderer.hdr {
            // SAFETY: the surface is Metal on macOS, alive for this call, and
            // used on the main thread.
            unsafe {
                if let Some(hal) = renderer.surface.as_hal::<wgpu::hal::api::Metal>() {
                    let layer = hal.render_layer().lock();
                    crate::edr::tag_extended_linear(
                        objc2::rc::Retained::as_ptr(&layer).cast_mut().cast(),
                    );
                }
            }
        }
        Ok(renderer)
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

    /// Creates an Android surface from an acquired native window.
    ///
    /// A device tries Vulkan first and falls back to OpenGL ES when Vulkan
    /// has no adapter or device for the window. The emulator uses OpenGL ES
    /// only: enumerating Vulkan can stall inside emulator drivers before a
    /// fallback is possible. Set the `debug.verse.backend` system property to
    /// `vulkan` or `gl` to force one, for example with
    /// `adb shell setprop debug.verse.backend vulkan`.
    ///
    /// # Safety
    /// `window` must point to a valid ANativeWindow on its owning UI thread.
    /// The caller must retain the window until after this renderer is dropped,
    /// stop frame callbacks before teardown, and serialize all renderer calls.
    #[cfg(target_os = "android")]
    pub unsafe fn from_android_window(
        window: *mut core::ffi::c_void,
        width: u32,
        height: u32,
        world: &Mesh,
        atlas: &Atlas,
        options: RenderOptions,
    ) -> Result<Self, String> {
        let window = std::ptr::NonNull::new(window).ok_or("native Android window is null")?;
        options.validate()?;
        validate_extent(width, height, options.max_extent)?;
        let mut failures = Vec::new();
        for backends in android::backends() {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            descriptor.backends = backends;
            let instance = wgpu::Instance::new(descriptor);
            let handle = wgpu::rwh::AndroidNdkWindowHandle::new(window);
            // SAFETY: the caller retains the acquired window through renderer
            // drop. A failed attempt drops its surface before the next one.
            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(wgpu::rwh::AndroidDisplayHandle::new().into()),
                    raw_window_handle: handle.into(),
                })
            };
            let result = surface
                .map_err(|error| format!("cannot create an Android surface: {error}"))
                .and_then(|surface| {
                    Self::from_surface(instance, surface, width, height, world, atlas, options)
                });
            match result {
                Ok(renderer) => return Ok(renderer),
                Err(error) => failures.push(format!("{backends:?}: {error}")),
            }
        }
        Err(failures.join("; "))
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
        let opened = open(&instance, Some(&surface))?;
        Self::assemble(surface, opened, width, height, world, atlas, options)
    }

    /// [`Self::from_surface`] without blocking: a browser cannot wait for its
    /// adapter and device, so the page awaits this instead.
    #[allow(clippy::too_many_arguments)]
    pub async fn from_surface_async(
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
        let opened = open_async(&instance, Some(&surface)).await?;
        let mut renderer = Self::assemble(surface, opened, width, height, world, atlas, options)?;
        renderer.physical_error = renderer
            .scene
            .prepare_photo_async(&renderer.device, &renderer.queue)
            .await;
        Ok(renderer)
    }

    /// Why the physical renderer is unavailable, when
    /// [`Self::from_surface_async`] found it so; the amber renderer draws.
    #[must_use]
    pub fn physical_error(&self) -> Option<&str> {
        self.physical_error.as_deref()
    }

    /// Configures `surface` on an opened device and uploads the world.
    fn assemble(
        surface: wgpu::Surface<'static>,
        (adapter, device, queue): (wgpu::Adapter, wgpu::Device, wgpu::Queue),
        width: u32,
        height: u32,
        world: &Mesh,
        atlas: &Atlas,
        options: RenderOptions,
    ) -> Result<Self, String> {
        let max_extent = options
            .max_extent
            .min(device.limits().max_texture_dimension_2d);
        validate_extent(width, height, max_extent)?;
        let caps = surface.get_capabilities(&adapter);
        let extended = wgpu::TextureFormat::Rgba16Float;
        let hdr = options.hdr && caps.formats.contains(&extended);
        let gles = crate::gles::is_gles(adapter.get_info().backend);
        // A browser's WebGPU canvas offers no sRGB format; draw as OpenGL ES
        // does, into an sRGB texture encoded into the linear surface.
        let linear_only = !caps.formats.iter().any(wgpu::TextureFormat::is_srgb);
        let encode = gles || linear_only;
        let format = if hdr {
            extended
        } else if encode {
            // OpenGL ES and WebGPU canvases draw into an sRGB texture and
            // encode it into a linear surface; see `Present`.
            caps.formats
                .iter()
                .copied()
                .find(|f| !f.is_srgb() && f.components() == 4 && !f.has_depth_aspect())
                .ok_or("the surface reports no linear 8-bit format")?
        } else {
            caps.formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
                .or_else(|| caps.formats.first().copied())
                .ok_or("the surface reports no formats")?
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            // The world is opaque. Android's Vulkan surfaces may offer only
            // Inherit, which the window's own opaque format then decides.
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else {
                caps.alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Auto)
            },
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let present = (encode && !hdr).then(|| Present::new(&device, format, width, height));
        let drawn = present.as_ref().map_or(format, |_| Present::FORMAT);
        let scene = Scene::new(
            &device,
            &queue,
            &adapter,
            drawn,
            world,
            atlas,
            options.sample_count,
        );
        let targets = Targets::new(&device, drawn, width, height, scene.samples);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            scene,
            targets,
            present,
            max_extent,
            drawable: true,
            hdr,
            overlay: None,
            physical_error: None,
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

    /// Whether the surface is extended range: output values above 1.0 reach
    /// the display as highlights brighter than reference white.
    #[must_use]
    pub fn hdr(&self) -> bool {
        self.hdr
    }

    /// The display's current headroom: how many times brighter than
    /// reference white it can show now. Photographic frames roll their
    /// highlights off toward it; 1.0 keeps standard range.
    pub fn set_headroom(&mut self, headroom: f32) {
        self.scene.headroom = if self.hdr && headroom.is_finite() {
            headroom.clamp(1.0, 16.0)
        } else {
            1.0
        };
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
        if let Some(present) = &mut self.present {
            present.resize(&self.device, width, height);
        }
        self.targets = Targets::new(
            &self.device,
            self.scene.format,
            width,
            height,
            self.scene.samples,
        );
        self.drawable = true;
        Ok(())
    }

    /// Replace zone-owned geometry only after the loader has verified its pack.
    /// Replacing these buffers releases the previous zone's GPU allocations.
    pub fn replace_world(&mut self, world: &Mesh) -> Result<(), String> {
        const LIMIT: usize = 96 * 1024 * 1024;
        for vertices in [&world.faces, &world.lines] {
            if vertices
                .len()
                .checked_mul(std::mem::size_of::<Vertex>())
                .is_none_or(|n| n > LIMIT)
                || vertices
                    .iter()
                    .any(|v| v.pos.iter().chain(v.color.iter()).any(|x| !x.is_finite()))
            {
                return Err("Zone geometry exceeds its GPU bounds".into());
            }
        }
        if lit_bytes(&world.lit).is_none_or(|n| n > LIMIT) || !lit_finite(&world.lit) {
            return Err("Zone geometry exceeds its GPU bounds".into());
        }
        let textured = prepare_textured(world)?;
        let upload = |vertices: &[Vertex], label| {
            let bytes = bytemuck::cast_slice(vertices);
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });
            Batch {
                buffer,
                count: vertices.len() as u32,
                capacity: bytes.len() as u64,
            }
        };
        let faces = upload(&world.faces, "verse zone faces");
        let lines = upload(&world.lines, "verse zone lines");
        self.scene.world_faces = faces;
        self.scene.world_lines = lines;
        self.scene.world_lit = upload_lit(&self.device, &world.lit);
        self.scene.textured_pending = textured;
        self.scene.textured = None;
        self.scene.textured_baked = None;
        self.scene.figure = None;
        // Animated models can be much larger than plaza avatars. A return
        // releases their buffer capacity instead of retaining the largest zone.
        self.scene.dynamic_faces = dynamic_batch(&self.device, "verse dynamic faces");
        self.scene.dynamic_lines = dynamic_batch(&self.device, "verse dynamic lines");
        Ok(())
    }

    pub fn set_atmosphere(&mut self, atmosphere: crate::zones::Atmosphere) -> Result<(), String> {
        self.scene.atmosphere = atmosphere.validate()?;
        Ok(())
    }

    /// Shows `image` over every following frame, or no panel for `None`.
    ///
    /// # Errors
    ///
    /// Returns a message when the image breaks its bounds.
    pub fn set_overlay(
        &mut self,
        image: Option<&crate::overlay::OverlayImage>,
    ) -> Result<(), String> {
        if image.is_none() && self.overlay.is_none() {
            return Ok(());
        }
        let format = self.scene.format;
        self.overlay
            .get_or_insert_with(|| crate::overlay::Overlay::new(&self.device, format))
            .set(&self.device, &self.queue, image)
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
        let drawn = self.present.as_ref().map_or(&output, |p| &p.frame);
        self.scene.encode(
            &self.device,
            &self.queue,
            &mut encoder,
            drawn,
            &mut self.targets,
            view,
            dynamic,
            ui,
        );
        if let Some(overlay) = &self.overlay {
            overlay.encode(&self.queue, &mut encoder, drawn, self.targets.size);
        }
        if let Some(present) = &self.present {
            present.encode(&mut encoder, &output);
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
        DrawStatus::Presented
    }
}

/// The same scene drawn into a texture a host owns, on the host's own
/// device: a window that composites the world under its own interface (the
/// desktop app's backdrop) keeps one device and one surface. The host
/// submits the encoder and presents.
pub struct Layer {
    scene: Scene,
    targets: Targets,
    format: wgpu::TextureFormat,
    size: (u32, u32),
}

impl Layer {
    /// A layer drawing `world` into `format` textures `width` by `height`
    /// pixels, with `samples` (1 or 4) where the format supports them.
    ///
    /// # Errors
    ///
    /// Returns a message when the extent is out of bounds or the device's
    /// limits cannot run the scene.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        world: &Mesh,
        atlas: &Atlas,
        atmosphere: crate::zones::Atmosphere,
        samples: u32,
    ) -> Result<Self, String> {
        let limit = device.limits().max_texture_dimension_2d.min(8192);
        validate_extent(width, height, limit)?;
        scene_limits(device.limits())?;
        let mut scene = Scene::new(device, queue, adapter, format, world, atlas, samples);
        scene.atmosphere = atmosphere.validate()?;
        let targets = Targets::new(device, format, width, height, scene.samples);
        Ok(Self {
            scene,
            targets,
            format,
            size: (width, height),
        })
    }

    /// The size the layer draws at, in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Draws at a new size from the next frame.
    ///
    /// # Errors
    ///
    /// Returns a message when the extent is out of bounds.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> Result<(), String> {
        if (width, height) == self.size {
            return Ok(());
        }
        validate_extent(
            width,
            height,
            device.limits().max_texture_dimension_2d.min(8192),
        )?;
        self.targets = Targets::new(device, self.format, width, height, self.scene.samples);
        self.size = (width, height);
        Ok(())
    }

    /// Records one frame into `output`, a view of a texture in the layer's
    /// format and size.
    ///
    /// # Errors
    ///
    /// Returns a message when the frame exceeds the renderer's bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        view: View,
        dynamic: &Mesh,
        ui: &UiBatch,
    ) -> Result<(), String> {
        validate_frame(view, dynamic, ui)?;
        self.scene.encode(
            device,
            queue,
            encoder,
            output,
            &mut self.targets,
            view,
            dynamic,
            ui,
        );
        Ok(())
    }
}

/// Which graphics APIs an Android surface tries, in order.
#[cfg(target_os = "android")]
pub(crate) mod android {
    /// A system property's value, when it is set and non-empty.
    fn property(name: &std::ffi::CStr) -> Option<String> {
        // PROP_VALUE_MAX in <sys/system_properties.h>.
        let mut value = [0 as libc::c_char; 92];
        // SAFETY: the name is NUL-terminated and the buffer holds
        // PROP_VALUE_MAX bytes, the most the call writes.
        let length = unsafe { libc::__system_property_get(name.as_ptr(), value.as_mut_ptr()) };
        if length <= 0 {
            return None;
        }
        // SAFETY: the call NUL-terminates the value it wrote.
        let value = unsafe { std::ffi::CStr::from_ptr(value.as_ptr()) };
        Some(value.to_string_lossy().into_owned())
    }

    pub(crate) fn backends() -> Vec<wgpu::Backends> {
        match property(c"debug.verse.backend").as_deref() {
            Some("gl") => return vec![wgpu::Backends::GL],
            Some("vulkan") => return vec![wgpu::Backends::VULKAN],
            _ => {}
        }
        let emulator = [c"ro.boot.qemu", c"ro.kernel.qemu"]
            .into_iter()
            .any(|name| property(name).as_deref() == Some("1"));
        if emulator {
            vec![wgpu::Backends::GL]
        } else {
            vec![wgpu::Backends::VULKAN, wgpu::Backends::GL]
        }
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
    if !mesh_bytes(dynamic.lit.len() * 3 / 2)
        || !mesh_bytes(dynamic.glow.len())
        || !lit_finite(&dynamic.lit)
        || dynamic.glow.iter().any(|g| {
            g.pos
                .iter()
                .chain(&g.radiance)
                .chain(&g.uv)
                .any(|x| !x.is_finite())
        })
    {
        return Err("frame exceeds its physical geometry bounds".into());
    }
    if let Some(figure) = &dynamic.figure {
        figure.validate()?;
    }
    Ok(())
}

fn lit_bytes(vertices: &[LitVertex]) -> Option<usize> {
    vertices.len().checked_mul(std::mem::size_of::<LitVertex>())
}

fn lit_finite(vertices: &[LitVertex]) -> bool {
    vertices.iter().all(|v| {
        v.pos
            .iter()
            .chain(&v.normal)
            .chain(&v.tangent)
            .chain(&v.local)
            .chain(&v.color)
            .chain(&v.params)
            .all(|x| x.is_finite())
    })
}

/// A world's textured scene with its merged cells.
type PreparedTextured = (std::sync::Arc<TexturedScene>, Merged);

/// Merges the world's textured meshes into cells for upload.
///
/// # Errors
///
/// Returns a message when the textured scene is out of bounds.
fn prepare_textured(world: &Mesh) -> Result<Option<PreparedTextured>, String> {
    world
        .textured
        .as_ref()
        .map(|scene| scene.merge().map(|merged| (scene.clone(), merged)))
        .transpose()
}

fn upload_lit(device: &wgpu::Device, vertices: &[LitVertex]) -> (wgpu::Buffer, u32) {
    let bytes: &[u8] = if vertices.is_empty() {
        &[0; std::mem::size_of::<LitVertex>()]
    } else {
        bytemuck::cast_slice(vertices)
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("verse world lit"),
        contents: bytes,
        usage: wgpu::BufferUsages::VERTEX,
    });
    (buffer, vertices.len() as u32)
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
    capture_with_atmosphere(
        path,
        width,
        height,
        world,
        view,
        dynamic,
        ui,
        atlas,
        crate::zones::atmosphere(crate::zones::ZoneId::Plaza),
    )
}

/// Render a zone with the same atmosphere used by its native surface.
#[cfg(feature = "capture")]
#[allow(clippy::too_many_arguments)]
pub fn capture_with_atmosphere(
    path: &Path,
    width: u32,
    height: u32,
    world: &Mesh,
    view: View,
    dynamic: &Mesh,
    ui: &UiBatch,
    atlas: &Atlas,
    atmosphere: crate::zones::Atmosphere,
) -> Result<(), String> {
    capture_with_overlay(
        path, width, height, world, view, dynamic, ui, atlas, atmosphere, None,
    )
}

/// Render a frame with `overlay`, a rasterized panel, composited over the
/// world and the HUD as the window draws it.
#[cfg(feature = "capture")]
#[allow(clippy::too_many_arguments)]
pub fn capture_with_overlay(
    path: &Path,
    width: u32,
    height: u32,
    world: &Mesh,
    view: View,
    dynamic: &Mesh,
    ui: &UiBatch,
    atlas: &Atlas,
    atmosphere: crate::zones::Atmosphere,
    overlay: Option<&crate::overlay::OverlayImage>,
) -> Result<(), String> {
    let pixels = offscreen(
        width,
        height,
        world,
        view,
        dynamic,
        ui,
        atlas,
        atmosphere,
        CAPTURE_FORMAT,
        1.0,
        overlay,
    )?;
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

/// Renders one frame offscreen in `format` and returns its tightly packed
/// texels, `headroom` giving photographic frames their output ceiling.
#[cfg(feature = "capture")]
#[allow(clippy::too_many_arguments)]
fn offscreen(
    width: u32,
    height: u32,
    world: &Mesh,
    view: View,
    dynamic: &Mesh,
    ui: &UiBatch,
    atlas: &Atlas,
    atmosphere: crate::zones::Atmosphere,
    format: wgpu::TextureFormat,
    headroom: f32,
    overlay: Option<&crate::overlay::OverlayImage>,
) -> Result<Vec<u8>, String> {
    let texel = format
        .block_copy_size(None)
        .ok_or("capture format has no fixed size")?;
    let atmosphere = atmosphere.validate()?;
    validate_extent(width, height, RenderOptions::default().max_extent)?;
    validate_frame(view, dynamic, ui)?;
    if let Some(scene) = &world.textured {
        scene.validate()?;
    }
    let instance = instance();
    let (adapter, device, queue) = open(&instance, None)?;
    validate_extent(width, height, device.limits().max_texture_dimension_2d)?;
    let mut scene = Scene::new(&device, &queue, &adapter, format, world, atlas, 4);
    scene.atmosphere = atmosphere;
    scene.headroom = headroom;
    let mut targets = Targets::new(&device, format, width, height, scene.samples);
    let panel = match overlay {
        Some(image) => {
            let mut panel = crate::overlay::Overlay::new(&device, format);
            panel.set(&device, &queue, Some(image))?;
            Some(panel)
        }
        None => None,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse capture"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let row = (width * texel).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("verse capture readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // A physical frame adapts its exposure over frames; settle it first.
    if dynamic.sky.is_some() {
        for _ in 0..3 {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("verse capture warm-up"),
            });
            scene.encode(
                &device,
                &queue,
                &mut encoder,
                &output,
                &mut targets,
                view,
                dynamic,
                ui,
            );
            queue.submit([encoder.finish()]);
        }
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("verse capture"),
    });
    scene.encode(
        &device,
        &queue,
        &mut encoder,
        &output,
        &mut targets,
        view,
        dynamic,
        ui,
    );
    if let Some(panel) = &panel {
        panel.encode(&queue, &mut encoder, &output, targets.size);
    }
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
    let mut pixels = Vec::with_capacity((width * height * texel) as usize);
    for y in 0..height as usize {
        let start = y * row as usize;
        pixels.extend_from_slice(&mapped[start..start + width as usize * texel as usize]);
    }
    drop(mapped);
    readback.unmap();

    Ok(pixels)
}

/// Renders one frame into an extended-range (RGBA16F, linear) target, as an
/// HDR display surface receives it, and returns linear RGBA values. Values
/// above 1.0 are highlights brighter than reference white; `headroom` is the
/// display's ceiling.
///
/// # Errors
///
/// Returns a message when no GPU is available.
#[cfg(feature = "capture")]
#[allow(clippy::too_many_arguments)]
pub fn capture_extended(
    width: u32,
    height: u32,
    world: &Mesh,
    view: View,
    dynamic: &Mesh,
    ui: &UiBatch,
    atlas: &Atlas,
    atmosphere: crate::zones::Atmosphere,
    headroom: f32,
) -> Result<Vec<[f32; 4]>, String> {
    let bytes = offscreen(
        width,
        height,
        world,
        view,
        dynamic,
        ui,
        atlas,
        atmosphere,
        wgpu::TextureFormat::Rgba16Float,
        headroom.clamp(1.0, 16.0),
        None,
    )?;
    Ok(bytes
        .chunks_exact(8)
        .map(|t| std::array::from_fn(|i| half_to_f32(u16::from_le_bytes([t[i * 2], t[i * 2 + 1]]))))
        .collect())
}

/// IEEE 754 binary16 to f32.
#[cfg(feature = "capture")]
fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x03ff);
    sign * match exponent {
        0 => mantissa * 2f32.powi(-24),
        0x1f => f32::INFINITY,
        e => (1.0 + mantissa / 1024.0) * 2f32.powi(e - 15),
    }
}

#[cfg(any(target_vendor = "apple", feature = "desktop", feature = "capture"))]
fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
}

fn open(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    pollster::block_on(open_async(instance, surface))
}

async fn open_async(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
        })
        .await
        .map_err(|e| format!("no graphics adapter: {e}"))?;
    let required_limits = scene_limits(adapter.limits())?;
    // The physical path prefers a compact 32-bit floating-point scene target.
    let required_features = adapter.features() & wgpu::Features::RG11B10UFLOAT_RENDERABLE;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("verse"),
            required_limits,
            required_features,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no graphics device: {e}"))?;
    Ok((adapter, device, queue))
}

/// An error scope's result, waited for natively. A browser cannot block:
/// WebGL settles a scope at once and is read here, and a WebGPU scope that
/// has not settled reads as no error, which the browser's console reports.
fn scope_error(
    scope: impl std::future::Future<Output = Option<wgpu::Error>>,
) -> Option<wgpu::Error> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(scope)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut scope = std::pin::pin!(scope);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match scope.as_mut().poll(&mut context) {
            std::task::Poll::Ready(error) => error,
            std::task::Poll::Pending => None,
        }
    }
}

// The world shader passes color, world position, and fog at locations 0..2;
// the physical lit shader passes six values plus built-ins; the HUD shader passes UV and
// color. Every downlevel adapter offers at least 15. None uses compute or
// storage.
const INTER_STAGE: u32 = 8;

// Every backend requests the OpenGL ES 3.0 floor (the WebGL 2 limits, which
// have no storage buffers or compute), so a GLES-only device qualifies and
// desktop validation refuses anything such a device could not run.
fn scene_limits(available: wgpu::Limits) -> Result<wgpu::Limits, String> {
    let mut required =
        wgpu::Limits::downlevel_webgl2_defaults().using_resolution(available.clone());
    required.max_inter_stage_shader_variables = INTER_STAGE;
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
        let (ui_pipeline, ui_photo, ui_bind_group, ui_screen) =
            ui_pipeline(device, queue, format, samples, atlas);
        let capability = Capability::probe(adapter, device, format, requested_samples);
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
            atmosphere: crate::zones::atmosphere(crate::zones::ZoneId::Plaza),
            ui_photo,
            format,
            capability,
            world_lit: upload_lit(device, &world.lit),
            textured_pending: prepare_textured(world).unwrap_or_else(|error| {
                eprintln!("verse: textured meshes unavailable: {error}");
                None
            }),
            textured: None,
            textured_baked: None,
            figure: None,
            photo: None,
            photo_failed: false,
            headroom: 1.0,
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
            dynamic_faces: dynamic_batch(device, "verse dynamic faces"),
            dynamic_lines: dynamic_batch(device, "verse dynamic lines"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut Targets,
        view: View,
        dynamic: &Mesh,
        ui: &UiBatch,
    ) {
        let stage = match (&dynamic.sky, &dynamic.neon) {
            (Some(sky), _) => Some(Stage::Space(sky)),
            (None, Some(neon)) => Some(Stage::Neon(neon)),
            (None, None) => None,
        };
        if let Some(stage) = stage
            && !self.photo_failed
            && self.encode_photo(
                device, queue, encoder, output, targets, view, dynamic, stage, ui,
            )
        {
            return;
        }
        let field = self.atmosphere.color;
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
            eye: [eye.x, eye.y, eye.z, self.atmosphere.fog_start],
            fog: [field[0], field[1], field[2], self.atmosphere.fog_end],
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

impl Scene {
    /// Creates the physical renderer now, awaiting its error scopes. A
    /// browser resolves an error scope only after the current task yields, so
    /// [`Scene::encode_photo`]'s synchronous check would miss a pipeline the
    /// browser rejects; the page calls this during its asynchronous start
    /// instead. Returns the error when the physical path is unavailable and
    /// the amber renderer draws.
    async fn prepare_photo_async(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<String> {
        if self.photo.is_some() || self.photo_failed {
            return None;
        }
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let created = Photo::new(device, queue, self.capability, self.format);
        let internal = internal.pop().await;
        let validation = validation.pop().await;
        match (created, internal.or(validation)) {
            (Ok(photo), None) => {
                self.photo = Some(photo);
                None
            }
            (Err(error), _) => {
                self.capability.hdr = None;
                self.photo_failed = true;
                Some(error)
            }
            (Ok(_), Some(error)) => {
                self.photo_failed = true;
                Some(error.to_string())
            }
        }
    }
}

impl Scene {
    /// Draws a physical frame. Returns false when the physical path cannot be
    /// created, so the caller falls back to the amber renderer.
    #[allow(clippy::too_many_arguments)]
    fn encode_photo(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        targets: &mut Targets,
        view: View,
        dynamic: &Mesh,
        stage: Stage<'_>,
        ui: &UiBatch,
    ) -> bool {
        if self.photo.is_none() {
            // Validation failures (an adapter limit) and internal ones (a
            // driver that rejects a translated shader) fall back to the amber
            // renderer instead of aborting.
            let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
            let created = Photo::new(device, queue, self.capability, self.format);
            let internal = scope_error(internal.pop());
            let failure = internal.or(scope_error(validation.pop()));
            match (created, failure) {
                (Ok(photo), None) => self.photo = Some(photo),
                (Err(error), _) => {
                    eprintln!("verse: physical renderer unavailable: {error}");
                    self.capability.hdr = None;
                    self.photo_failed = true;
                    return false;
                }
                (Ok(_), Some(error)) => {
                    eprintln!("verse: physical renderer unavailable: {error}");
                    self.photo_failed = true;
                    return false;
                }
            }
        }
        let Some(photo) = &mut self.photo else {
            return false;
        };
        if let Some((scene, merged)) = self.textured_pending.take() {
            self.textured = Some(photo.upload_textured(device, queue, &scene, &merged));
            self.textured_baked = Some(scene.baked.clone());
        }
        // A finished bake replaces the vertices once; the merge order is the
        // bake's own, so only the light channel changes.
        let baked = match (&self.textured, &self.textured_baked) {
            (Some(_), Some(slot)) => slot.take(),
            _ => None,
        };
        if let (Some(gpu), Some(vertices)) = (&self.textured, baked) {
            gpu.write_vertices(queue, &vertices);
            self.textured_baked = None;
        }
        if let Some(figure) = &dynamic.figure {
            if self
                .figure
                .as_ref()
                .is_none_or(|(scene, _)| !std::sync::Arc::ptr_eq(scene, &figure.scene))
            {
                self.figure = Some((
                    figure.scene.clone(),
                    photo.upload_figure(device, queue, figure),
                ));
            }
            if let Some((_, gpu)) = &self.figure {
                gpu.write_vertices(queue, &figure.vertices);
            }
        }
        if matches!(stage, Stage::Space(_))
            && let Err(error) = photo.prepare_space(device, queue)
        {
            eprintln!("verse: space sky data unavailable: {error}");
            self.photo_failed = true;
            return false;
        }
        let size = [targets.size[0] as u32, targets.size[1] as u32];
        if targets.photo.as_ref().is_none_or(|t| t.size() != size) {
            targets.photo = Some(photo.targets(device, size[0], size[1]));
        }
        let Some(photo_targets) = &mut targets.photo else {
            return false;
        };
        queue.write_buffer(
            &self.ui_screen,
            0,
            bytemuck::cast_slice(&[targets.size[0], targets.size[1], 0.0, 0.0]),
        );
        let ui_bytes: &[u8] = bytemuck::cast_slice(&ui.vertices);
        if !ui_bytes.is_empty() {
            queue.write_buffer(&self.ui.buffer, 0, ui_bytes);
        }
        self.ui.count = (ui_bytes.len() / std::mem::size_of::<UiVertex>()) as u32;
        write(device, queue, &mut self.dynamic_faces, &dynamic.faces);
        write(device, queue, &mut self.dynamic_lines, &dynamic.lines);
        photo.dynamic_lit.write(device, queue, &dynamic.lit);
        photo.glow.write(device, queue, &dynamic.glow);
        let batches = Batches {
            lit: (&self.world_lit.0, self.world_lit.1),
            faces: [
                (&self.world_faces.buffer, self.world_faces.count),
                (&self.dynamic_faces.buffer, self.dynamic_faces.count),
            ],
            lines: [
                (&self.world_lines.buffer, self.world_lines.count),
                (&self.dynamic_lines.buffer, self.dynamic_lines.count),
            ],
            textured: self.textured.as_ref(),
            figure: dynamic
                .figure
                .as_ref()
                .and(self.figure.as_ref().map(|(_, gpu)| gpu)),
        };
        photo.headroom = self.headroom;
        photo.encode(
            device,
            queue,
            encoder,
            output,
            photo_targets,
            view,
            stage,
            batches,
            Some((
                &self.ui_photo,
                &self.ui_bind_group,
                &self.ui.buffer,
                self.ui.count,
            )),
        );
        true
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
            photo: None,
        }
    }
}

/// The UI pipeline: screen-space quads sampling the glyph atlas, alpha
/// blended, drawn last with no depth test.
pub(crate) fn ui_pipeline(
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
    let rgba = atlas.rgba.clone().unwrap_or_else(|| {
        atlas
            .pixels
            .iter()
            .flat_map(|a| [255, 255, 255, *a])
            .collect()
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
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
    )
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
        // An OpenGL ES 3.0 device: no storage buffers or compute.
        let mut mobile = wgpu::Limits::downlevel_webgl2_defaults();
        mobile.max_texture_dimension_2d = 4096;
        let required = scene_limits(mobile.clone()).unwrap();
        assert_eq!(required.max_inter_stage_shader_variables, INTER_STAGE);
        assert_eq!(required.max_texture_dimension_2d, 4096);
        assert!(required.check_limits(&mobile));
        mobile.max_inter_stage_shader_variables = INTER_STAGE - 1;
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
                max_extent: 8192,
                hdr: false,
            }
            .validate()
            .is_err()
        );
        assert!(
            RenderOptions {
                sample_count: 1,
                max_extent: 0,
                hdr: false,
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

    /// Textured meshes rendered through the physical path on this machine's
    /// GPU. Each test skips when no adapter can run the scene.
    #[cfg(feature = "capture")]
    mod textured_frames {
        use super::*;
        use crate::pbr::textured::{
            AlphaMode, BaseColorImage, Primitive, TexturedMaterial, TexturedMesh, TexturedScene,
            TexturedVertex,
        };

        const SIZE: u32 = 64;
        /// Half the visible height at the quads, 5 m from the eye.
        const HALF_VIEW: f32 = 2.418;

        /// A quad of half side `half` in the XY plane, facing the camera or,
        /// with `away`, wound and lit to face away from it. Image coordinates
        /// run left to right across the quad.
        fn quad(material: usize, half: f32, away: bool) -> TexturedMesh {
            let normal = if away { Vec3::NEG_Z } else { Vec3::Z };
            let v = |x: f32, y: f32| {
                TexturedVertex::new(
                    Vec3::new(x * half, y * half, 0.0),
                    normal,
                    [(x + 1.0) * 0.5, (1.0 - y) * 0.5],
                )
            };
            TexturedMesh {
                primitives: vec![Primitive {
                    vertices: vec![v(-1.0, -1.0), v(1.0, -1.0), v(1.0, 1.0), v(-1.0, 1.0)],
                    indices: if away {
                        vec![0, 2, 1, 0, 3, 2]
                    } else {
                        vec![0, 1, 2, 0, 2, 3]
                    },
                    material,
                }],
            }
        }

        /// Renders `scene` from 5 m in front of the origin on a neutral neon
        /// stage under a studio key, or `None` without a usable GPU.
        fn render(scene: TexturedScene) -> Option<Vec<u8>> {
            let world = Mesh {
                textured: Some(std::sync::Arc::new(scene)),
                ..Mesh::default()
            };
            let eye = Vec3::new(0.0, 0.0, 5.0);
            let view = View {
                view_proj: Mat4::perspective_rh(0.9, 1.0, 0.1, 100.0)
                    * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
                eye,
            };
            let mut neon = crate::pbr::Neon::neutral(0.0);
            neon.key = Some(crate::pbr::Key {
                dir: Vec3::new(0.2, 0.4, 1.0).normalize(),
                illuminance: 4_000.0,
                angular_radius: 0.03,
                rim_dir: Vec3::new(-0.3, 0.3, 1.0).normalize(),
                rim_illuminance: 1_000.0,
                rim_angular_radius: 0.1,
                sky: 800.0,
                ground: 400.0,
                ev100: 10.0,
                shadow_center: Vec3::ZERO,
                shadow_half: 10.0,
                shadow_distance: None,
                cache_far_shadows: false,
            });
            let dynamic = Mesh {
                neon: Some(neon),
                ..Mesh::default()
            };
            match offscreen(
                SIZE,
                SIZE,
                &world,
                view,
                &dynamic,
                &UiBatch::default(),
                &Atlas::new(16.0),
                crate::zones::atmosphere(crate::zones::ZoneId::Plaza),
                CAPTURE_FORMAT,
                1.0,
                None,
            ) {
                Ok(pixels) => Some(pixels),
                Err(error) if error.contains("graphics") => {
                    eprintln!("skipped without a GPU: {error}");
                    None
                }
                Err(error) => panic!("{error}"),
            }
        }

        /// The RGB at world point (`x`, `y`) on the plane through the origin.
        fn at(pixels: &[u8], x: f32, y: f32) -> [u8; 3] {
            let column = ((x / HALF_VIEW + 1.0) * 0.5 * SIZE as f32) as usize;
            let row = ((1.0 - y / HALF_VIEW) * 0.5 * SIZE as f32) as usize;
            let i = (row * SIZE as usize + column) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        }

        fn brightness(rgb: [u8; 3]) -> u8 {
            rgb.into_iter().max().unwrap_or(0)
        }

        /// White on the left half at alpha 0.2 and the right half at 0.9.
        fn split_alpha() -> BaseColorImage {
            BaseColorImage {
                name: "split".into(),
                width: 8,
                height: 1,
                rgba: (0..8)
                    .flat_map(|x| [255, 255, 255, if x < 4 { 51 } else { 230 }])
                    .collect(),
            }
        }

        fn one_quad(alpha: AlphaMode) -> TexturedScene {
            let mut scene = TexturedScene::default();
            let image = scene.add_image(split_alpha());
            let material = scene.add_material(TexturedMaterial {
                image: Some(image),
                alpha,
                ..TexturedMaterial::default()
            });
            let mesh = scene.add_mesh(quad(material, 1.0, false));
            scene.place(mesh, Mat4::IDENTITY);
            scene
        }

        #[test]
        fn masked_texels_below_the_cutoff_are_not_drawn() {
            let Some(masked) = render(one_quad(AlphaMode::Mask { cutoff: 0.5 })) else {
                return;
            };
            let Some(opaque) = render(one_quad(AlphaMode::Opaque)) else {
                return;
            };
            let Some(empty) = render(TexturedScene::default()) else {
                return;
            };
            // Alpha 0.2 everywhere is under the cutoff: the stage shows
            // through. A uniform texture keeps the passing half's glow out of
            // the comparison, and each point is compared with the same point
            // of an empty frame because the stage's lighting varies.
            let mut hidden = one_quad(AlphaMode::Mask { cutoff: 0.5 });
            hidden.images[0]
                .rgba
                .chunks_mut(4)
                .for_each(|texel| texel[3] = 51);
            let Some(hidden) = render(hidden) else {
                return;
            };
            for x in [-0.5, 0.5] {
                let background = brightness(at(&empty, x, 0.0));
                assert!(
                    brightness(at(&hidden, x, 0.0)) <= background + 8,
                    "{:?} over {background}",
                    at(&hidden, x, 0.0)
                );
            }
            let background = brightness(at(&empty, 0.5, 0.0));
            // Alpha 0.9 passes, and an opaque material ignores alpha.
            assert!(brightness(at(&masked, 0.5, 0.0)) > background + 60);
            assert!(brightness(at(&opaque, -0.5, 0.0)) > background + 60);
        }

        #[test]
        fn only_double_sided_materials_show_their_back_faces() {
            let mut scene = TexturedScene::default();
            for (x, double_sided) in [(-1.2, true), (1.2, false)] {
                let material = scene.add_material(TexturedMaterial {
                    double_sided,
                    ..TexturedMaterial::default()
                });
                let mesh = scene.add_mesh(quad(material, 0.8, true));
                scene.place(mesh, Mat4::from_translation(Vec3::new(x, 0.0, 0.0)));
            }
            let Some(pixels) = render(scene) else {
                return;
            };
            let background = brightness(at(&pixels, 0.0, 2.0));
            assert!(brightness(at(&pixels, -1.2, 0.0)) > background + 60);
            assert!(brightness(at(&pixels, 1.2, 0.0)) <= background + 8);
        }

        #[test]
        fn nearer_glass_composites_over_farther_glass() {
            let mut scene = TexturedScene::default();
            // Green glass is first in merge order but nearer the eye, so
            // only distance sorting draws it last.
            for (color, z) in [([0.0, 1.0, 0.0, 0.5], 0.5), ([1.0, 0.0, 0.0, 0.5], -0.5)] {
                let material = scene.add_material(TexturedMaterial {
                    base_color: color,
                    alpha: AlphaMode::Blend,
                    ..TexturedMaterial::default()
                });
                let mesh = scene.add_mesh(quad(material, 1.0, false));
                scene.place(mesh, Mat4::from_translation(Vec3::new(0.0, 0.0, z)));
            }
            let Some(pixels) = render(scene) else {
                return;
            };
            let [red, green, _] = at(&pixels, 0.0, 0.0);
            assert!(green > red.saturating_add(20), "red {red}, green {green}");
            assert!(red > 0, "the far glass shows through the near glass");
        }
    }
}
