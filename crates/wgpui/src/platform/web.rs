//! Web platform implementation using web-sys and wgpu.

use crate::geometry::Size;
use crate::input::Cursor;
use crate::platform::Platform;
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::text::TextSystem;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// Web platform for running wgpui in a browser.
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
    /// Initialize the platform on the given canvas element.
    pub async fn init(canvas_id: &str) -> Result<Self, String> {
        // Get canvas element
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("Canvas not found")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Element is not a canvas")?;

        Self::init_on_canvas(canvas).await
    }

    /// Initialize the platform on an existing canvas element.
    pub async fn init_on_canvas(canvas: HtmlCanvasElement) -> Result<Self, String> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let scale_factor = window.device_pixel_ratio() as f32;

        // Get logical size from CSS
        let rect = canvas.get_bounding_client_rect();
        let logical_width = rect.width() as f32;
        let logical_height = rect.height() as f32;

        // Set physical canvas size
        let physical_width = (logical_width * scale_factor) as u32;
        let physical_height = (logical_height * scale_factor) as u32;
        canvas.set_width(physical_width);
        canvas.set_height(physical_height);

        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
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

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .map_err(|e| format!("Failed to create device: {:?}", e))?;

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: physical_width,
            height: physical_height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create renderer
        let renderer = Renderer::new(&device, surface_format);
        renderer.resize(
            &queue,
            Size::new(logical_width, logical_height),
            scale_factor,
        );

        // Create text system
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

    /// Get the device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get the queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Get the text system.
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text_system
    }

    /// Get the logical size.
    pub fn logical_size(&self) -> Size {
        self.logical_size
    }

    /// Get the scale factor.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Get the canvas element.
    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// Render a frame.
    pub fn render_scene(&mut self, scene: &Scene) -> Result<(), String> {
        // Update atlas if needed
        if self.text_system.is_dirty() {
            self.renderer.update_atlas(
                &self.queue,
                self.text_system.atlas_data(),
                self.text_system.atlas_size(),
            );
            self.text_system.mark_clean();
        }

        // Prepare scene
        self.renderer.prepare(&self.device, scene);

        // Get frame
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Render
        self.renderer.render(&mut encoder, &view);

        // Submit
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
        // This is a no-op since we use run_animation_loop
    }

    fn set_cursor(&self, cursor: Cursor) {
        if let Ok(style) = self.canvas.style().set_property("cursor", cursor.as_css()) {
            let _ = style;
        }
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

/// Run the animation loop with the given render callback.
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

/// Set up resize observer on the given canvas.
pub fn setup_resize_observer<F>(canvas: &HtmlCanvasElement, mut callback: F)
where
    F: FnMut() + 'static,
{
    let closure = Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| {
        callback();
    });

    let observer = web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref()).unwrap();
    observer.observe(canvas);

    // Prevent closure from being dropped
    closure.forget();
}
