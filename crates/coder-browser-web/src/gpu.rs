//! A standalone atlas pipeline. No Verse world, presence, or application is mounted.

use verse_gfx::ui::{Atlas, UiBatch};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlCanvasElement};

const VERTEX_MAX: usize = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 2048;

#[derive(Debug)]
struct WebDisplay;
impl raw_window_handle::HasDisplayHandle for WebDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::web())
    }
}

pub struct Gpu {
    pub canvas: HtmlCanvasElement,
    pub atlas: Atlas,
    pub backend: &'static str,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    screen: wgpu::Buffer,
    vertices: wgpu::Buffer,
    capacity: usize,
    lifecycle: rust_native::surface::SurfaceLifecycle,
}

impl Gpu {
    pub async fn open(parent: &Element) -> Result<Self, JsValue> {
        let document = parent.owner_document().ok_or_else(failure)?;
        for backends in [
            wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            wgpu::Backends::GL,
        ] {
            // A failed WebGPU context cannot become WebGL on the same canvas.
            let canvas = document
                .create_element("canvas")?
                .dyn_into::<HtmlCanvasElement>()?;
            canvas.set_id("cloud-terminal-canvas");
            canvas.set_attribute(
                "aria-label",
                "Native host terminal rendered by the shared Rust GPU renderer",
            )?;
            canvas.set_attribute("tabindex", "0")?;
            canvas.set_attribute("style", "display:block;width:100%;height:420px;max-width:100%;background:#101010;touch-action:none")?;
            canvas.set_width(parent.client_width().clamp(240, MAX_SIDE as i32) as u32);
            canvas.set_height(420);
            parent.append_child(&canvas)?;
            match Self::open_canvas(canvas.clone(), backends).await {
                Ok(gpu) => return Ok(gpu),
                Err(_) => {
                    canvas.set_width(1);
                    canvas.set_height(1);
                    canvas.remove();
                }
            }
        }
        Err(failure())
    }

    async fn open_canvas(
        canvas: HtmlCanvasElement,
        backends: wgpu::Backends,
    ) -> Result<Self, JsValue> {
        let mut descriptor =
            wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
        descriptor.backends = backends;
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|_| failure())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_| failure())?;
        let backend = if adapter.get_info().backend == wgpu::Backend::Gl {
            "WebGL2"
        } else {
            "WebGPU"
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Cloud shared terminal renderer"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|_| failure())?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(failure)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: canvas.width(),
            height: canvas.height(),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let atlas = Atlas::new(16.0);
        let (_, pipeline, group, screen) =
            verse_gfx::ui_pipeline::ui_pipeline(&device, &queue, format, 1, &atlas);
        let capacity = 64 * 1024;
        let vertices = buffer(&device, capacity);
        let viewport = rust_native::surface::Viewport::new(config.width, config.height, 1.0)
            .map_err(|_| failure())?;
        let mut lifecycle =
            rust_native::surface::SurfaceLifecycle::new("cloud-terminal-grid", viewport)
                .map_err(|_| failure())?;
        lifecycle.set_active(true).map_err(|_| failure())?;
        Ok(Self {
            canvas,
            atlas,
            backend,
            surface,
            device,
            queue,
            config,
            pipeline,
            group,
            screen,
            vertices,
            capacity,
            lifecycle,
        })
    }

    pub fn size(&self) -> [f32; 2] {
        [self.config.width as f32, self.config.height as f32]
    }

    pub fn clear(&mut self) {
        self.canvas.set_width(1);
        self.canvas.set_height(1);
        self.config.width = 1;
        self.config.height = 1;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn draw(&mut self, batch: &UiBatch) -> Result<(), JsValue> {
        let width = self.canvas.client_width().clamp(240, MAX_SIDE as i32) as u32;
        let height = self.canvas.client_height().clamp(160, 640) as u32;
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.surface.configure(&self.device, &self.config);
        }
        self.lifecycle
            .resize(rust_native::surface::Viewport::new(width, height, 1.0).map_err(|_| failure())?)
            .map_err(|_| failure())?;
        if self
            .lifecycle
            .frame_delta(js_sys::Date::now() / 1000.0)
            .map_err(|_| failure())?
            .is_none()
        {
            return Ok(());
        }
        let bytes = bytemuck::cast_slice(&batch.vertices);
        if bytes.len() > VERTEX_MAX {
            return Err(failure());
        }
        if bytes.len() > self.capacity {
            self.capacity = bytes.len().next_power_of_two().min(VERTEX_MAX);
            self.vertices = buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(&self.vertices, 0, bytes);
        self.queue.write_buffer(
            &self.screen,
            0,
            bytemuck::cast_slice(&[
                self.config.width as f32,
                self.config.height as f32,
                0.0,
                0.0,
            ]),
        );
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            _ => return Err(failure()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Cloud shared terminal frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cloud terminal glyph grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.006,
                            g: 0.006,
                            b: 0.006,
                            a: 1.0,
                        }),
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
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..batch.vertices.len() as u32, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        self.lifecycle.destroy();
        self.canvas.set_width(1);
        self.canvas.set_height(1);
        self.device.destroy();
    }
}

fn buffer(device: &wgpu::Device, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Cloud terminal vertices"),
        size: size as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn failure() -> JsValue {
    JsValue::from_str("The shared terminal renderer is unavailable. WebGPU or WebGL2 is required.")
}
