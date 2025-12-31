use crate::geometry::Size;
use crate::input::Cursor;
use crate::scene::Scene;
use crate::text::TextSystem;

pub trait Platform {
    fn logical_size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn text_system(&mut self) -> &mut TextSystem;
    fn render(&mut self, scene: &Scene) -> Result<(), String>;
    fn request_redraw(&self);
    fn set_cursor(&self, cursor: Cursor);
    fn handle_resize(&mut self);
}

pub fn default_surface_config(
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web {
    use super::*;
    use crate::renderer::Renderer;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::HtmlCanvasElement;

    pub struct WebPlatform {
        canvas: HtmlCanvasElement,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        renderer: Renderer,
        text_system: TextSystem,
        scale_factor: f32,
        logical_size: Size,
    }

    impl WebPlatform {
        pub async fn init(canvas_id: &str) -> Result<Self, String> {
            let window = web_sys::window().ok_or("No window")?;
            let document = window.document().ok_or("No document")?;
            let canvas = document
                .get_element_by_id(canvas_id)
                .ok_or("Canvas not found")?
                .dyn_into::<HtmlCanvasElement>()
                .map_err(|_| "Element is not a canvas")?;

            Self::init_on_canvas(canvas).await
        }

        pub async fn init_on_canvas(canvas: HtmlCanvasElement) -> Result<Self, String> {
            console_error_panic_hook::set_once();

            let window = web_sys::window().ok_or("No window")?;
            let scale_factor = window.device_pixel_ratio() as f32;

            let rect = canvas.get_bounding_client_rect();
            let logical_width = rect.width() as f32;
            let logical_height = rect.height() as f32;

            let physical_width = (logical_width * scale_factor) as u32;
            let physical_height = (logical_height * scale_factor) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            // Try WebGPU first, fall back to WebGL2 if it fails
            let (device, queue, surface, adapter) =
                Self::try_init_gpu(&canvas, physical_width, physical_height).await?;

            let surface_caps = surface.get_capabilities(&adapter);

            // Log available formats for debugging
            let formats_str = surface_caps.formats.iter()
                .map(|f| format!("{:?}", f))
                .collect::<Vec<_>>()
                .join(", ");
            web_sys::console::log_1(&format!("Available formats: {}", formats_str).into());

            // Prefer non-sRGB format - our colors are already in sRGB space
            // Using sRGB surface would double-encode and wash out colors
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| !f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            web_sys::console::log_1(&format!("Using surface format: {:?}", surface_format).into());

            let surface_config =
                super::default_surface_config(physical_width, physical_height, surface_format);
            surface.configure(&device, &surface_config);

            let renderer = Renderer::new(&device, surface_format);
            renderer.resize(
                &queue,
                Size::new(logical_width, logical_height),
                scale_factor,
            );

            let text_system = TextSystem::new(scale_factor);
            let logical_size = Size::new(logical_width, logical_height);

            Ok(Self {
                canvas,
                device,
                queue,
                surface,
                surface_config,
                renderer,
                text_system,
                scale_factor,
                logical_size,
            })
        }

        fn is_webgpu_reliable() -> bool {
            // Check if we're on a platform where WebGPU is known to work
            // Linux WebGPU is experimental and often broken in Chromium
            let window = match web_sys::window() {
                Some(w) => w,
                None => return false,
            };
            let navigator = window.navigator();
            let user_agent = navigator.user_agent().unwrap_or_default();

            // Skip WebGPU on Linux - it's experimental and often broken
            if user_agent.contains("Linux") && !user_agent.contains("Android") {
                web_sys::console::log_1(&"Skipping WebGPU on Linux (experimental)".into());
                return false;
            }

            true
        }

        async fn try_init_gpu(
            canvas: &HtmlCanvasElement,
            _physical_width: u32,
            _physical_height: u32,
        ) -> Result<(wgpu::Device, wgpu::Queue, wgpu::Surface<'static>, wgpu::Adapter), String> {
            // Try WebGPU first (only on reliable platforms)
            if Self::is_webgpu_reliable() {
                if let Ok(result) = Self::try_backend(canvas, wgpu::Backends::BROWSER_WEBGPU).await {
                    web_sys::console::log_1(&"Using WebGPU backend".into());
                    return Ok(result);
                }
            }

            // Fall back to WebGL2
            web_sys::console::log_1(&"Using WebGL2 backend".into());
            if let Ok(result) = Self::try_backend(canvas, wgpu::Backends::GL).await {
                return Ok(result);
            }

            Err("Failed to initialize GPU".to_string())
        }

        async fn try_backend(
            canvas: &HtmlCanvasElement,
            backends: wgpu::Backends,
        ) -> Result<(wgpu::Device, wgpu::Queue, wgpu::Surface<'static>, wgpu::Adapter), String> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            });

            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                .map_err(|e| format!("Failed to create surface: {:?}", e))?;

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or("No adapter found")?;

            // Use the adapter's actual limits to ensure compatibility
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("wgpui-device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: adapter.limits(),
                        memory_hints: wgpu::MemoryHints::default(),
                    },
                    None,
                )
                .await
                .map_err(|e| format!("Failed to create device: {:?}", e))?;

            Ok((device, queue, surface, adapter))
        }

        pub fn device(&self) -> &wgpu::Device {
            &self.device
        }

        pub fn queue(&self) -> &wgpu::Queue {
            &self.queue
        }

        pub fn canvas(&self) -> &HtmlCanvasElement {
            &self.canvas
        }

        pub fn render_scene(&mut self, scene: &Scene) -> Result<(), String> {
            if self.text_system.is_dirty() {
                self.renderer.update_atlas(
                    &self.queue,
                    self.text_system.atlas_data(),
                    self.text_system.atlas_size(),
                );
                self.text_system.mark_clean();
            }

            self.renderer.prepare(&self.device, &self.queue, scene, self.scale_factor);

            let frame = self
                .surface
                .get_current_texture()
                .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

            self.renderer.render(&mut encoder, &view);

            self.queue.submit(std::iter::once(encoder.finish()));
            frame.present();

            Ok(())
        }
    }

    impl Platform for WebPlatform {
        fn logical_size(&self) -> Size {
            self.logical_size
        }

        fn scale_factor(&self) -> f32 {
            self.scale_factor
        }

        fn text_system(&mut self) -> &mut TextSystem {
            &mut self.text_system
        }

        fn render(&mut self, scene: &Scene) -> Result<(), String> {
            self.render_scene(scene)
        }

        fn request_redraw(&self) {
            // On web, requestAnimationFrame handles redraws
        }

        fn set_cursor(&self, cursor: Cursor) {
            let _ = self.canvas.style().set_property("cursor", cursor.as_css());
        }

        fn handle_resize(&mut self) {
            let window = web_sys::window().unwrap();
            self.scale_factor = window.device_pixel_ratio() as f32;

            let rect = self.canvas.get_bounding_client_rect();
            let logical_width = rect.width() as f32;
            let logical_height = rect.height() as f32;
            self.logical_size = Size::new(logical_width, logical_height);

            let physical_width = (logical_width * self.scale_factor) as u32;
            let physical_height = (logical_height * self.scale_factor) as u32;

            if physical_width > 0 && physical_height > 0 {
                self.canvas.set_width(physical_width);
                self.canvas.set_height(physical_height);

                self.surface_config.width = physical_width;
                self.surface_config.height = physical_height;
                self.surface.configure(&self.device, &self.surface_config);

                self.renderer
                    .resize(&self.queue, self.logical_size, self.scale_factor);

                self.text_system.set_scale_factor(self.scale_factor);
            }
        }
    }

    pub fn run_animation_loop<F>(mut callback: F)
    where
        F: FnMut() + 'static,
    {
        let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();

        *g.borrow_mut() = Some(Closure::new(move || {
            callback();
            request_animation_frame(f.borrow().as_ref().unwrap());
        }));

        request_animation_frame(g.borrow().as_ref().unwrap());
    }

    fn request_animation_frame(f: &Closure<dyn FnMut()>) {
        web_sys::window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .unwrap();
    }

    pub fn setup_resize_observer<F>(canvas: &HtmlCanvasElement, mut callback: F)
    where
        F: FnMut() + 'static,
    {
        let closure = Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| {
            callback();
        });

        let observer = web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref()).unwrap();
        observer.observe(canvas);

        closure.forget();
    }
}

#[cfg(feature = "desktop")]
pub mod desktop {
    use super::*;

    pub struct DesktopPlatform {
        size: Size,
        scale_factor: f32,
        text_system: TextSystem,
    }

    impl DesktopPlatform {
        pub fn new(width: f32, height: f32, scale_factor: f32) -> Self {
            Self {
                size: Size::new(width, height),
                scale_factor,
                text_system: TextSystem::new(scale_factor),
            }
        }
    }

    impl Platform for DesktopPlatform {
        fn logical_size(&self) -> Size {
            self.size
        }

        fn scale_factor(&self) -> f32 {
            self.scale_factor
        }

        fn text_system(&mut self) -> &mut TextSystem {
            &mut self.text_system
        }

        fn render(&mut self, _scene: &Scene) -> Result<(), String> {
            Ok(())
        }

        fn request_redraw(&self) {}

        fn set_cursor(&self, _cursor: Cursor) {}

        fn handle_resize(&mut self) {}
    }
}
