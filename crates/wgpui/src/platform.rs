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

#[cfg(feature = "web")]
#[allow(dead_code)]
pub(crate) fn is_webgpu_reliable_user_agent(user_agent: &str) -> bool {
    // Linux desktop WebGPU remains unstable in Chromium for our supported matrix.
    !(user_agent.contains("Linux") && !user_agent.contains("Android"))
}

#[cfg(all(test, feature = "web"))]
mod webgpu_policy_tests {
    use super::is_webgpu_reliable_user_agent;

    #[test]
    fn linux_desktop_is_marked_unreliable() {
        assert!(!is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        ));
    }

    #[test]
    fn android_linux_is_allowed() {
        assert!(is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36",
        ));
    }

    #[test]
    fn macos_is_allowed() {
        assert!(is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_3) AppleWebKit/605.1.15",
        ));
    }
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web {
    use super::*;
    use crate::renderer::Renderer;
    use js_sys::Reflect;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::{JsValue, prelude::*};
    use web_sys::HtmlCanvasElement;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WebBackendKind {
        WebGpu,
        WebGl2,
    }

    impl WebBackendKind {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::WebGpu => "webgpu",
                Self::WebGl2 => "webgl2",
            }
        }
    }

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
        backend: WebBackendKind,
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
            let (device, queue, surface, adapter, backend) =
                Self::try_init_gpu(&canvas, physical_width, physical_height).await?;

            let surface_caps = surface.get_capabilities(&adapter);

            // Log available formats for debugging
            let formats_str = surface_caps
                .formats
                .iter()
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
                backend,
            })
        }

        fn is_webgpu_reliable() -> bool {
            if let Some(forced) = Self::forced_backend_hint() {
                match forced {
                    WebBackendKind::WebGpu => {
                        web_sys::console::log_1(&"Forcing WebGPU backend by host hint".into());
                        return true;
                    }
                    WebBackendKind::WebGl2 => {
                        web_sys::console::log_1(
                            &"Forcing WebGL2 backend by host hint (WebGPU skipped)".into(),
                        );
                        return false;
                    }
                }
            }

            // Check if we're on a platform where WebGPU is known to work
            // Linux WebGPU is experimental and often broken in Chromium
            let window = match web_sys::window() {
                Some(w) => w,
                None => return false,
            };
            let navigator = window.navigator();
            let user_agent = navigator.user_agent().unwrap_or_default();

            if !super::is_webgpu_reliable_user_agent(&user_agent) {
                web_sys::console::log_1(&"Skipping WebGPU on Linux (experimental)".into());
                return false;
            }

            true
        }

        fn forced_backend_hint() -> Option<WebBackendKind> {
            let window = web_sys::window()?;
            let value = Reflect::get(&window, &JsValue::from_str("__OA_GPU_MODE__")).ok()?;
            let mode = value.as_string()?.trim().to_ascii_lowercase();
            match mode.as_str() {
                "webgpu" => Some(WebBackendKind::WebGpu),
                "webgl2" | "webgl" => Some(WebBackendKind::WebGl2),
                _ => None,
            }
        }

        async fn try_init_gpu(
            canvas: &HtmlCanvasElement,
            _physical_width: u32,
            _physical_height: u32,
        ) -> Result<
            (
                wgpu::Device,
                wgpu::Queue,
                wgpu::Surface<'static>,
                wgpu::Adapter,
                WebBackendKind,
            ),
            String,
        > {
            // Try WebGPU first (only on reliable platforms)
            if Self::is_webgpu_reliable() {
                if let Ok(result) = Self::try_backend(canvas, wgpu::Backends::BROWSER_WEBGPU).await
                {
                    web_sys::console::log_1(&"Using WebGPU backend".into());
                    return Ok((
                        result.0,
                        result.1,
                        result.2,
                        result.3,
                        WebBackendKind::WebGpu,
                    ));
                }
            }

            // Fall back to WebGL2
            web_sys::console::log_1(&"Using WebGL2 backend".into());
            if let Ok(result) = Self::try_backend(canvas, wgpu::Backends::GL).await {
                return Ok((
                    result.0,
                    result.1,
                    result.2,
                    result.3,
                    WebBackendKind::WebGl2,
                ));
            }

            Err("Failed to initialize GPU".to_string())
        }

        async fn try_backend(
            canvas: &HtmlCanvasElement,
            backends: wgpu::Backends,
        ) -> Result<
            (
                wgpu::Device,
                wgpu::Queue,
                wgpu::Surface<'static>,
                wgpu::Adapter,
            ),
            String,
        > {
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

        pub fn backend_kind(&self) -> WebBackendKind {
            self.backend
        }

        pub fn backend_name(&self) -> &'static str {
            self.backend.as_str()
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

            self.renderer
                .prepare(&self.device, &self.queue, scene, self.scale_factor);

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

/// iOS platform: WGPUI background renderer (dots grid) from a CAMetalLayer.
/// Requires `ios` feature. Uses wgpu create_surface_unsafe(CoreAnimationLayer).
#[cfg(feature = "ios")]
pub mod ios {
    use std::ffi::c_void;

    use crate::components::hud::{DotShape, DotsGrid};
    use crate::components::Component;
    use crate::geometry::{Bounds, Size};
    use crate::renderer::Renderer;
    use crate::scene::{Quad, Scene};
    use crate::theme;
    use crate::{PaintContext, TextSystem};

    /// Distance between grid dots (matches desktop autopilot_ui GRID_DOT_DISTANCE).
    const GRID_DOT_DISTANCE: f32 = 32.0;

    /// State for the iOS WGPUI background renderer (black + dots grid).
    pub struct IosBackgroundState {
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        renderer: Renderer,
        text_system: TextSystem,
        size: Size,
        scale: f32,
    }

    impl IosBackgroundState {
        /// Create WGPUI render state from a CAMetalLayer pointer and dimensions.
        /// Call from main thread. Uses Metal backend.
        /// Safety: `layer_ptr` must be a valid CAMetalLayer and outlive this state.
        pub unsafe fn new(
            layer_ptr: *mut c_void,
            width: u32,
            height: u32,
            scale: f32,
        ) -> Result<Box<Self>, String> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..Default::default()
            });

            let surface = unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_ptr))
            }
            .map_err(|e| format!("create_surface_unsafe: {:?}", e))?;

            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .ok_or("no adapter")?;

            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor::default(),
                None,
            ))
            .map_err(|e| format!("request_device: {:?}", e))?;

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .or_else(|| caps.formats.first().copied())
                .ok_or("no surface format")?;

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.max(1),
                height: height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: caps
                    .alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, format);
            let text_system = TextSystem::new(scale);
            let size = Size::new(width as f32, height as f32);

            Ok(Box::new(Self {
                device,
                queue,
                surface,
                config,
                renderer,
                text_system,
                size,
                scale,
            }))
        }

        /// Render one frame: black background + dots grid (same as desktop).
        pub fn render(&mut self) -> Result<(), String> {
            let mut scene = Scene::new();
            let mut paint = PaintContext::new(&mut scene, &mut self.text_system, self.scale);

            // Black background (matches desktop)
            paint.scene.draw_quad(
                Quad::new(Bounds::new(0.0, 0.0, self.size.width, self.size.height))
                    .with_background(crate::color::Hsla::black()),
            );

            // Dots grid (same params as autopilot_ui MinimalRoot)
            let mut dots_grid = DotsGrid::new()
                .shape(DotShape::Circle)
                .color(theme::text::MUTED)
                .opacity(0.12)
                .distance(GRID_DOT_DISTANCE)
                .size(1.5);
            let grid_bounds = Bounds::new(0.0, 0.0, self.size.width, self.size.height);
            dots_grid.paint(grid_bounds, &mut paint);

            self.renderer
                .resize(&self.queue, self.size, self.scale);

            if self.text_system.is_dirty() {
                self.renderer.update_atlas(
                    &self.queue,
                    self.text_system.atlas_data(),
                    self.text_system.atlas_size(),
                );
                self.text_system.mark_clean();
            }

            let output = match self.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost) => {
                    self.surface.configure(&self.device, &self.config);
                    return Ok(());
                }
                Err(e) => return Err(format!("surface get_current_texture: {:?}", e)),
            };

            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("wgpui-ios"),
                });

            self.renderer
                .prepare(&self.device, &self.queue, &scene, self.scale);
            self.renderer.render(&mut encoder, &view);
            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();

            Ok(())
        }

        /// Resize the surface (e.g. on layout change).
        pub fn resize(&mut self, width: u32, height: u32) {
            self.config.width = width.max(1);
            self.config.height = height.max(1);
            self.surface.configure(&self.device, &self.config);
            self.size = Size::new(self.config.width as f32, self.config.height as f32);
        }
    }

    /// C FFI for Swift: create renderer from CAMetalLayer pointer.
    /// Returns opaque pointer to IosBackgroundState, or null on error.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_create(
        layer_ptr: *mut c_void,
        width: u32,
        height: u32,
        scale: f32,
    ) -> *mut IosBackgroundState {
        if layer_ptr.is_null() {
            return std::ptr::null_mut();
        }
        match unsafe { IosBackgroundState::new(layer_ptr, width, height, scale) } {
            Ok(state) => Box::into_raw(state),
            Err(_) => std::ptr::null_mut(),
        }
    }

    /// C FFI: render one frame. Returns 1 on success, 0 on error.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_render(state: *mut IosBackgroundState) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        match state.render() {
            Ok(()) => 1,
            Err(_) => 0,
        }
    }

    /// C FFI: resize the surface.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_resize(
        state: *mut IosBackgroundState,
        width: u32,
        height: u32,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.resize(width, height);
    }

    /// C FFI: destroy state and free memory.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_destroy(state: *mut IosBackgroundState) {
        if state.is_null() {
            return;
        }
        let _ = unsafe { Box::from_raw(state) };
    }
}
