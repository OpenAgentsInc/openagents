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
    use std::ffi::{c_char, c_void};
    use std::time::Duration;

    use crate::animation::AnimatorState;
    use crate::color::Hsla;
    use crate::components::hud::{DotShape, DotsGrid, PuffsBackground};
    use crate::components::{Button, ButtonVariant, Component, Text, TextInput};
    use crate::geometry::{Bounds, Size};
    use crate::renderer::Renderer;
    use crate::scene::{Quad, Scene};
    use crate::theme;
    use crate::{PaintContext, TextSystem};
    use web_time::Instant;

    /// Distance between grid dots (matches desktop autopilot_ui GRID_DOT_DISTANCE).
    const GRID_DOT_DISTANCE: f32 = 32.0;

    const EDGE_PADDING: f32 = 14.0;
    const PANEL_CORNER_RADIUS: f32 = 12.0;
    const TITLE_HEIGHT: f32 = 28.0;
    const TOP_CONTEXT_HEIGHT: f32 = 34.0;
    const ACTION_ROW_HEIGHT: f32 = 34.0;
    const COMPOSER_HEIGHT: f32 = 42.0;
    const CONTROL_GAP: f32 = 8.0;
    const BUTTON_WIDTH: f32 = 112.0;
    const SEND_BUTTON_WIDTH: f32 = 86.0;
    const MESSAGE_GAP: f32 = 8.0;
    const BUBBLE_PADDING_X: f32 = 10.0;
    const BUBBLE_PADDING_Y: f32 = 8.0;
    const MESSAGE_FONT_SIZE: f32 = 14.0;
    const SMALL_TEXT_SIZE: f32 = 12.0;
    const STREAMING_LABEL_SIZE: f32 = 11.0;
    const BUBBLE_MIN_HEIGHT: f32 = 30.0;
    const BUBBLE_MAX_WIDTH_RATIO: f32 = 0.72;
    const MAX_RENDERED_MESSAGES: usize = 42;
    const MAX_STORED_MESSAGES: usize = 220;
    const OPS_TOGGLE_WIDTH: f32 = 86.0;
    const OPS_TOGGLE_HEIGHT: f32 = 30.0;
    const OPS_PANEL_WIDTH_RATIO: f32 = 0.9;
    const OPS_PANEL_HEIGHT_RATIO: f32 = 0.64;
    const OPS_PANEL_PADDING: f32 = 12.0;
    const OPS_INPUT_HEIGHT: f32 = 34.0;
    const OPS_ROW_HEIGHT: f32 = 32.0;
    const OPS_TEXT_BLOCK_HEIGHT: f32 = 54.0;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CodexMessageRole {
        User = 0,
        Assistant = 1,
        Reasoning = 2,
        Tool = 3,
        System = 4,
        Error = 5,
    }

    impl CodexMessageRole {
        fn from_u8(value: u8) -> Self {
            match value {
                0 => Self::User,
                1 => Self::Assistant,
                2 => Self::Reasoning,
                3 => Self::Tool,
                5 => Self::Error,
                _ => Self::System,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct CodexMessage {
        pub role: CodexMessageRole,
        pub text: String,
        pub is_streaming: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum InputTarget {
        None = 0,
        Composer = 1,
        AuthEmail = 2,
        AuthCode = 3,
    }

    impl InputTarget {
        fn from_u8(value: u8) -> Self {
            match value {
                1 => Self::Composer,
                2 => Self::AuthEmail,
                3 => Self::AuthCode,
                _ => Self::None,
            }
        }

        fn as_u8(self) -> u8 {
            self as u8
        }
    }

    /// State for the iOS WGPUI Codex renderer.
    pub struct IosBackgroundState {
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        renderer: Renderer,
        text_system: TextSystem,
        size: Size,
        scale: f32,
        /// Puffs floating up along grid lines (Arwes-style).
        puffs: PuffsBackground,
        last_frame: Option<Instant>,
        pub codex_messages: Vec<CodexMessage>,
        pub composer_text: String,
        pub active_thread_label: String,
        pub active_turn_label: String,
        pub model_label: String,
        pub reasoning_label: String,
        pub empty_title: String,
        pub empty_detail: String,
        pub auth_email: String,
        pub auth_code: String,
        pub auth_status: String,
        pub worker_status: String,
        pub stream_status: String,
        pub handshake_status: String,
        pub device_status: String,
        pub telemetry_text: String,
        pub events_text: String,
        pub control_text: String,
        send_requested: bool,
        new_thread_requested: bool,
        interrupt_requested: bool,
        model_cycle_requested: bool,
        reasoning_cycle_requested: bool,
        send_code_requested: bool,
        verify_code_requested: bool,
        sign_out_requested: bool,
        refresh_workers_requested: bool,
        connect_stream_requested: bool,
        disconnect_stream_requested: bool,
        send_handshake_requested: bool,
        operator_panel_visible: bool,
        active_input_target: InputTarget,
    }

    impl IosBackgroundState {
        fn logical_to_physical(value: u32, scale: f32) -> u32 {
            ((value.max(1) as f32) * scale.max(1.0)).ceil() as u32
        }

        /// Create WGPUI render state from a CAMetalLayer pointer and dimensions.
        /// `width`/`height` are logical points; surface config uses physical pixels via `scale`.
        /// Call from main thread. Uses Metal backend.
        /// Safety: `layer_ptr` must be a valid CAMetalLayer and outlive this state.
        pub unsafe fn new(
            layer_ptr: *mut c_void,
            width: u32,
            height: u32,
            scale: f32,
        ) -> Result<Box<Self>, String> {
            let scale = scale.max(1.0);
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..Default::default()
            });

            let surface = unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_ptr))
            }
            .map_err(|e| format!("create_surface_unsafe: {:?}", e))?;

            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .ok_or("no adapter")?;

            let (device, queue) = pollster::block_on(
                adapter.request_device(&wgpu::DeviceDescriptor::default(), None),
            )
            .map_err(|e| format!("request_device: {:?}", e))?;

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .or_else(|| caps.formats.first().copied())
                .ok_or("no surface format")?;

            let logical_width = width.max(1);
            let logical_height = height.max(1);
            let physical_width = Self::logical_to_physical(logical_width, scale);
            let physical_height = Self::logical_to_physical(logical_height, scale);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: physical_width,
                height: physical_height,
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
            let size = Size::new(logical_width as f32, logical_height as f32);
            let mut puffs = PuffsBackground::new()
                .color(theme::text::MUTED.with_alpha(0.15))
                .quantity(18)
                .padding(0.0)
                .grid_distance(Some(GRID_DOT_DISTANCE))
                .y_offset((-8.0, -80.0))
                .radius_initial(3.0)
                .radius_offset((4.0, 24.0))
                .sets(5)
                .layers(6)
                .cycle_duration(Duration::from_secs_f32(3.0));
            puffs.set_state(AnimatorState::Entered);

            Ok(Box::new(Self {
                device,
                queue,
                surface,
                config,
                renderer,
                text_system,
                size,
                scale,
                puffs,
                last_frame: None,
                codex_messages: Vec::new(),
                composer_text: String::new(),
                active_thread_label: "thread: none".to_string(),
                active_turn_label: "turn: none".to_string(),
                model_label: "model:auto".to_string(),
                reasoning_label: "reasoning:auto".to_string(),
                empty_title: "No Codex Messages Yet".to_string(),
                empty_detail: "Waiting for Codex events from desktop.".to_string(),
                auth_email: String::new(),
                auth_code: String::new(),
                auth_status: "signed out".to_string(),
                worker_status: "No workers loaded".to_string(),
                stream_status: "idle".to_string(),
                handshake_status: "idle".to_string(),
                device_status: String::new(),
                telemetry_text: "No reconnect telemetry yet".to_string(),
                events_text: "No recent events".to_string(),
                control_text: "No control requests".to_string(),
                send_requested: false,
                new_thread_requested: false,
                interrupt_requested: false,
                model_cycle_requested: false,
                reasoning_cycle_requested: false,
                send_code_requested: false,
                verify_code_requested: false,
                sign_out_requested: false,
                refresh_workers_requested: false,
                connect_stream_requested: false,
                disconnect_stream_requested: false,
                send_handshake_requested: false,
                operator_panel_visible: false,
                active_input_target: InputTarget::None,
            }))
        }

        fn set_utf8_string(target: &mut String, ptr: *const u8, len: usize) {
            if ptr.is_null() || len == 0 {
                target.clear();
                return;
            }
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            if let Ok(value) = std::str::from_utf8(slice) {
                target.clear();
                target.push_str(value);
            } else {
                target.clear();
            }
        }

        fn primary_panel_bounds(&self) -> Bounds {
            Bounds::new(
                EDGE_PADDING,
                EDGE_PADDING + 6.0,
                (self.size.width - EDGE_PADDING * 2.0).max(120.0),
                (self.size.height - EDGE_PADDING * 2.0 - 12.0).max(120.0),
            )
        }

        fn controls_layout(&self) -> (Bounds, Bounds, Bounds, Bounds, Bounds, Bounds) {
            let panel = self.primary_panel_bounds();
            let control_y_start = panel.y() + panel.height()
                - (TOP_CONTEXT_HEIGHT
                    + CONTROL_GAP
                    + ACTION_ROW_HEIGHT
                    + CONTROL_GAP
                    + COMPOSER_HEIGHT
                    + 14.0);
            let context_bounds = Bounds::new(
                panel.x() + 10.0,
                control_y_start,
                panel.width() - 20.0,
                TOP_CONTEXT_HEIGHT,
            );
            let action_y = context_bounds.y() + context_bounds.height() + CONTROL_GAP;
            let new_thread_bounds = Bounds::new(
                context_bounds.x(),
                action_y,
                BUTTON_WIDTH,
                ACTION_ROW_HEIGHT,
            );
            let interrupt_bounds = Bounds::new(
                new_thread_bounds.x() + new_thread_bounds.width() + CONTROL_GAP,
                action_y,
                BUTTON_WIDTH,
                ACTION_ROW_HEIGHT,
            );
            let composer_y = action_y + ACTION_ROW_HEIGHT + CONTROL_GAP;
            let send_bounds = Bounds::new(
                context_bounds.x() + context_bounds.width() - SEND_BUTTON_WIDTH,
                composer_y,
                SEND_BUTTON_WIDTH,
                COMPOSER_HEIGHT,
            );
            let composer_bounds = Bounds::new(
                context_bounds.x(),
                composer_y,
                (context_bounds.width() - SEND_BUTTON_WIDTH - CONTROL_GAP).max(80.0),
                COMPOSER_HEIGHT,
            );
            let transcript_bounds = Bounds::new(
                panel.x() + 10.0,
                panel.y() + 8.0 + TITLE_HEIGHT + 6.0,
                panel.width() - 20.0,
                (control_y_start - (panel.y() + TITLE_HEIGHT + 14.0)).max(80.0),
            );

            (
                transcript_bounds,
                context_bounds,
                new_thread_bounds,
                interrupt_bounds,
                composer_bounds,
                send_bounds,
            )
        }

        fn context_chip_bounds(&self, context_bounds: Bounds) -> (Bounds, Bounds) {
            let chip_width = (context_bounds.width() * 0.55).max(140.0);
            let chip_height = (TOP_CONTEXT_HEIGHT - 4.0) * 0.5;
            let model_bounds = Bounds::new(
                context_bounds.x(),
                context_bounds.y(),
                chip_width,
                chip_height,
            );
            let reasoning_bounds = Bounds::new(
                context_bounds.x(),
                context_bounds.y() + chip_height + 4.0,
                chip_width,
                chip_height,
            );
            (model_bounds, reasoning_bounds)
        }

        fn ops_toggle_bounds(&self) -> Bounds {
            let panel = self.primary_panel_bounds();
            Bounds::new(
                panel.x() + panel.width() - OPS_TOGGLE_WIDTH - 12.0,
                panel.y() + 10.0,
                OPS_TOGGLE_WIDTH,
                OPS_TOGGLE_HEIGHT,
            )
        }

        fn operator_panel_bounds(&self) -> Bounds {
            let panel = self.primary_panel_bounds();
            let width = (panel.width() * OPS_PANEL_WIDTH_RATIO).max(240.0);
            let height = (panel.height() * OPS_PANEL_HEIGHT_RATIO).max(260.0);
            Bounds::new(
                panel.x() + (panel.width() - width) * 0.5,
                panel.y() + (panel.height() - height) * 0.18,
                width,
                height,
            )
        }

        fn operator_layout(
            &self,
            panel: Bounds,
        ) -> (
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
            Bounds,
        ) {
            let content_x = panel.x() + OPS_PANEL_PADDING;
            let content_w = panel.width() - OPS_PANEL_PADDING * 2.0;
            let mut y = panel.y() + OPS_PANEL_PADDING + 20.0;

            let auth_email_bounds = Bounds::new(content_x, y, content_w * 0.58, OPS_INPUT_HEIGHT);
            let send_code_bounds = Bounds::new(
                auth_email_bounds.x() + auth_email_bounds.width() + 8.0,
                y,
                (content_w - auth_email_bounds.width() - 8.0).max(88.0),
                OPS_INPUT_HEIGHT,
            );

            y += OPS_INPUT_HEIGHT + 8.0;
            let auth_code_bounds = Bounds::new(content_x, y, content_w * 0.58, OPS_INPUT_HEIGHT);
            let verify_bounds = Bounds::new(
                auth_code_bounds.x() + auth_code_bounds.width() + 8.0,
                y,
                (content_w - auth_code_bounds.width() - 8.0).max(88.0),
                OPS_INPUT_HEIGHT,
            );

            y += OPS_INPUT_HEIGHT + 8.0;
            let signout_bounds = Bounds::new(content_x, y, 120.0, OPS_ROW_HEIGHT);
            let refresh_workers_bounds = Bounds::new(content_x + 128.0, y, 140.0, OPS_ROW_HEIGHT);
            let connect_bounds = Bounds::new(content_x + 276.0, y, 108.0, OPS_ROW_HEIGHT);
            let disconnect_bounds = Bounds::new(content_x + 392.0, y, 108.0, OPS_ROW_HEIGHT);
            let handshake_bounds = Bounds::new(content_x + 508.0, y, 128.0, OPS_ROW_HEIGHT);

            let clamp_row = |bounds: Bounds| -> Bounds {
                Bounds::new(
                    bounds.x(),
                    bounds.y(),
                    bounds
                        .width()
                        .min(panel.x() + panel.width() - OPS_PANEL_PADDING - bounds.x()),
                    bounds.height(),
                )
            };

            let signout_bounds = clamp_row(signout_bounds);
            let refresh_workers_bounds = clamp_row(refresh_workers_bounds);
            let connect_bounds = clamp_row(connect_bounds);
            let disconnect_bounds = clamp_row(disconnect_bounds);
            let handshake_bounds = clamp_row(handshake_bounds);

            y += OPS_ROW_HEIGHT + 8.0;
            let status_block_bounds = Bounds::new(content_x, y, content_w, OPS_TEXT_BLOCK_HEIGHT);
            y += OPS_TEXT_BLOCK_HEIGHT + 6.0;
            let telemetry_block_bounds =
                Bounds::new(content_x, y, content_w, OPS_TEXT_BLOCK_HEIGHT);
            y += OPS_TEXT_BLOCK_HEIGHT + 6.0;
            let events_block_bounds = Bounds::new(content_x, y, content_w, OPS_TEXT_BLOCK_HEIGHT);
            y += OPS_TEXT_BLOCK_HEIGHT + 6.0;
            let controls_block_bounds = Bounds::new(content_x, y, content_w, OPS_TEXT_BLOCK_HEIGHT);

            (
                auth_email_bounds,
                auth_code_bounds,
                send_code_bounds,
                verify_bounds,
                signout_bounds,
                refresh_workers_bounds,
                connect_bounds,
                disconnect_bounds,
                handshake_bounds,
                status_block_bounds,
                telemetry_block_bounds,
                events_block_bounds,
                controls_block_bounds,
            )
        }

        /// Handle tap at logical coordinates (same as render: origin top-left, units = pixels).
        pub fn handle_tap(&mut self, x: f32, y: f32) {
            use crate::geometry::Point;
            let (
                _transcript_bounds,
                context_bounds,
                new_thread_bounds,
                interrupt_bounds,
                composer_bounds,
                send_bounds,
            ) = self.controls_layout();
            let (model_bounds, reasoning_bounds) = self.context_chip_bounds(context_bounds);
            let p = Point::new(x, y);

            let ops_toggle_bounds = self.ops_toggle_bounds();
            if ops_toggle_bounds.contains(p) {
                self.operator_panel_visible = !self.operator_panel_visible;
                self.active_input_target = InputTarget::None;
                return;
            }

            if self.operator_panel_visible {
                let ops_panel = self.operator_panel_bounds();
                let (
                    auth_email_bounds,
                    auth_code_bounds,
                    send_code_bounds,
                    verify_bounds,
                    signout_bounds,
                    refresh_workers_bounds,
                    connect_bounds,
                    disconnect_bounds,
                    handshake_bounds,
                    _status_block_bounds,
                    _telemetry_block_bounds,
                    _events_block_bounds,
                    _controls_block_bounds,
                ) = self.operator_layout(ops_panel);

                if auth_email_bounds.contains(p) {
                    self.active_input_target = InputTarget::AuthEmail;
                    return;
                }
                if auth_code_bounds.contains(p) {
                    self.active_input_target = InputTarget::AuthCode;
                    return;
                }
                if send_code_bounds.contains(p) {
                    self.send_code_requested = true;
                    return;
                }
                if verify_bounds.contains(p) {
                    self.verify_code_requested = true;
                    return;
                }
                if signout_bounds.contains(p) {
                    self.sign_out_requested = true;
                    return;
                }
                if refresh_workers_bounds.contains(p) {
                    self.refresh_workers_requested = true;
                    return;
                }
                if connect_bounds.contains(p) {
                    self.connect_stream_requested = true;
                    return;
                }
                if disconnect_bounds.contains(p) {
                    self.disconnect_stream_requested = true;
                    return;
                }
                if handshake_bounds.contains(p) {
                    self.send_handshake_requested = true;
                    return;
                }
                if !ops_panel.contains(p) {
                    self.operator_panel_visible = false;
                    self.active_input_target = InputTarget::None;
                }
                return;
            }

            if model_bounds.contains(p) {
                self.model_cycle_requested = true;
                return;
            }
            if reasoning_bounds.contains(p) {
                self.reasoning_cycle_requested = true;
                return;
            }
            if composer_bounds.contains(p) {
                self.active_input_target = InputTarget::Composer;
                return;
            }
            if send_bounds.contains(p) {
                self.send_requested = true;
                return;
            }
            if new_thread_bounds.contains(p) {
                self.new_thread_requested = true;
                return;
            }
            if interrupt_bounds.contains(p) {
                self.interrupt_requested = true;
                return;
            }
            self.active_input_target = InputTarget::None;
        }

        pub fn clear_codex_messages(&mut self) {
            self.codex_messages.clear();
        }

        pub fn push_codex_message(
            &mut self,
            role: u8,
            text: *const u8,
            len: usize,
            is_streaming: bool,
        ) {
            if self.codex_messages.len() >= MAX_STORED_MESSAGES {
                let overflow = self.codex_messages.len() - MAX_STORED_MESSAGES + 1;
                self.codex_messages.drain(0..overflow);
            }
            let mut payload = String::new();
            Self::set_utf8_string(&mut payload, text, len);
            self.codex_messages.push(CodexMessage {
                role: CodexMessageRole::from_u8(role),
                text: payload,
                is_streaming,
            });
        }

        pub fn set_composer_text_utf8(&mut self, ptr: *const u8, len: usize) {
            Self::set_utf8_string(&mut self.composer_text, ptr, len);
        }

        pub fn set_codex_context_utf8(
            &mut self,
            thread_ptr: *const u8,
            thread_len: usize,
            turn_ptr: *const u8,
            turn_len: usize,
            model_ptr: *const u8,
            model_len: usize,
            reasoning_ptr: *const u8,
            reasoning_len: usize,
        ) {
            Self::set_utf8_string(&mut self.active_thread_label, thread_ptr, thread_len);
            Self::set_utf8_string(&mut self.active_turn_label, turn_ptr, turn_len);
            Self::set_utf8_string(&mut self.model_label, model_ptr, model_len);
            Self::set_utf8_string(&mut self.reasoning_label, reasoning_ptr, reasoning_len);
        }

        pub fn set_empty_state_utf8(
            &mut self,
            title_ptr: *const u8,
            title_len: usize,
            detail_ptr: *const u8,
            detail_len: usize,
        ) {
            Self::set_utf8_string(&mut self.empty_title, title_ptr, title_len);
            Self::set_utf8_string(&mut self.empty_detail, detail_ptr, detail_len);
        }

        pub fn set_auth_fields_utf8(
            &mut self,
            email_ptr: *const u8,
            email_len: usize,
            code_ptr: *const u8,
            code_len: usize,
            auth_status_ptr: *const u8,
            auth_status_len: usize,
        ) {
            Self::set_utf8_string(&mut self.auth_email, email_ptr, email_len);
            Self::set_utf8_string(&mut self.auth_code, code_ptr, code_len);
            Self::set_utf8_string(&mut self.auth_status, auth_status_ptr, auth_status_len);
        }

        pub fn set_operator_status_utf8(
            &mut self,
            worker_status_ptr: *const u8,
            worker_status_len: usize,
            stream_status_ptr: *const u8,
            stream_status_len: usize,
            handshake_status_ptr: *const u8,
            handshake_status_len: usize,
            device_status_ptr: *const u8,
            device_status_len: usize,
            telemetry_ptr: *const u8,
            telemetry_len: usize,
            events_ptr: *const u8,
            events_len: usize,
            control_ptr: *const u8,
            control_len: usize,
        ) {
            Self::set_utf8_string(
                &mut self.worker_status,
                worker_status_ptr,
                worker_status_len,
            );
            Self::set_utf8_string(
                &mut self.stream_status,
                stream_status_ptr,
                stream_status_len,
            );
            Self::set_utf8_string(
                &mut self.handshake_status,
                handshake_status_ptr,
                handshake_status_len,
            );
            Self::set_utf8_string(
                &mut self.device_status,
                device_status_ptr,
                device_status_len,
            );
            Self::set_utf8_string(&mut self.telemetry_text, telemetry_ptr, telemetry_len);
            Self::set_utf8_string(&mut self.events_text, events_ptr, events_len);
            Self::set_utf8_string(&mut self.control_text, control_ptr, control_len);
        }

        pub fn composer_focused(&self) -> bool {
            self.active_input_target == InputTarget::Composer
        }

        pub fn set_composer_focused(&mut self, focused: bool) {
            self.active_input_target = if focused {
                InputTarget::Composer
            } else {
                InputTarget::None
            };
        }

        fn active_input_target(&self) -> InputTarget {
            self.active_input_target
        }

        fn set_active_input_target(&mut self, target: InputTarget) {
            self.active_input_target = target;
        }

        pub fn consume_send_requested(&mut self) -> bool {
            let requested = self.send_requested;
            self.send_requested = false;
            requested
        }

        pub fn consume_new_thread_requested(&mut self) -> bool {
            let requested = self.new_thread_requested;
            self.new_thread_requested = false;
            requested
        }

        pub fn consume_interrupt_requested(&mut self) -> bool {
            let requested = self.interrupt_requested;
            self.interrupt_requested = false;
            requested
        }

        pub fn consume_model_cycle_requested(&mut self) -> bool {
            let requested = self.model_cycle_requested;
            self.model_cycle_requested = false;
            requested
        }

        pub fn consume_reasoning_cycle_requested(&mut self) -> bool {
            let requested = self.reasoning_cycle_requested;
            self.reasoning_cycle_requested = false;
            requested
        }

        pub fn consume_send_code_requested(&mut self) -> bool {
            let requested = self.send_code_requested;
            self.send_code_requested = false;
            requested
        }

        pub fn consume_verify_code_requested(&mut self) -> bool {
            let requested = self.verify_code_requested;
            self.verify_code_requested = false;
            requested
        }

        pub fn consume_sign_out_requested(&mut self) -> bool {
            let requested = self.sign_out_requested;
            self.sign_out_requested = false;
            requested
        }

        pub fn consume_refresh_workers_requested(&mut self) -> bool {
            let requested = self.refresh_workers_requested;
            self.refresh_workers_requested = false;
            requested
        }

        pub fn consume_connect_stream_requested(&mut self) -> bool {
            let requested = self.connect_stream_requested;
            self.connect_stream_requested = false;
            requested
        }

        pub fn consume_disconnect_stream_requested(&mut self) -> bool {
            let requested = self.disconnect_stream_requested;
            self.disconnect_stream_requested = false;
            requested
        }

        pub fn consume_send_handshake_requested(&mut self) -> bool {
            let requested = self.send_handshake_requested;
            self.send_handshake_requested = false;
            requested
        }

        fn bubble_colors(role: CodexMessageRole) -> (Hsla, Hsla) {
            match role {
                CodexMessageRole::User => (theme::accent::PRIMARY, Hsla::white()),
                CodexMessageRole::Assistant => (theme::bg::SURFACE, theme::text::PRIMARY),
                CodexMessageRole::Reasoning => (theme::bg::MUTED, theme::text::PRIMARY),
                CodexMessageRole::Tool => (theme::bg::CODE, theme::text::PRIMARY),
                CodexMessageRole::System => (theme::bg::SURFACE, theme::text::MUTED),
                CodexMessageRole::Error => {
                    (theme::status::ERROR.with_alpha(0.22), theme::status::ERROR)
                }
            }
        }

        fn estimated_lines(text: &str, max_width: f32, font_size: f32) -> u32 {
            let chars_per_line = (max_width / (font_size * 0.62)).max(12.0) as usize;
            let mut lines: u32 = 0;
            for line in text.lines() {
                let length = line.chars().count();
                if length == 0 {
                    lines += 1;
                } else {
                    lines += ((length + chars_per_line.saturating_sub(1)) / chars_per_line) as u32;
                }
            }
            lines.max(1)
        }

        fn render_messages(
            messages: &[CodexMessage],
            empty_title: &str,
            empty_detail: &str,
            transcript_bounds: Bounds,
            paint: &mut PaintContext<'_>,
        ) {
            if messages.is_empty() {
                let title_text = if empty_title.trim().is_empty() {
                    "No Codex Messages Yet"
                } else {
                    empty_title
                };
                let detail_text = if empty_detail.trim().is_empty() {
                    "Waiting for Codex events from desktop."
                } else {
                    empty_detail
                };

                let mut title = Text::new(title_text)
                    .font_size(22.0)
                    .color(theme::text::PRIMARY)
                    .no_wrap();
                title.paint(
                    Bounds::new(
                        transcript_bounds.x() + 14.0,
                        transcript_bounds.y() + 52.0,
                        transcript_bounds.width() - 28.0,
                        32.0,
                    ),
                    paint,
                );
                let mut sub = Text::new(detail_text)
                    .font_size(15.0)
                    .color(theme::text::MUTED)
                    .no_wrap();
                sub.paint(
                    Bounds::new(
                        transcript_bounds.x() + 14.0,
                        transcript_bounds.y() + 82.0,
                        transcript_bounds.width() - 28.0,
                        20.0,
                    ),
                    paint,
                );
                return;
            }

            let start_index = messages.len().saturating_sub(MAX_RENDERED_MESSAGES);
            let visible = &messages[start_index..];

            let mut cursor_y = transcript_bounds.y() + transcript_bounds.height() - 8.0;
            for message in visible.iter().rev() {
                let max_bubble_width = transcript_bounds.width() * BUBBLE_MAX_WIDTH_RATIO;
                let text = if message.text.trim().is_empty() {
                    "…"
                } else {
                    message.text.as_str()
                };
                let lines = Self::estimated_lines(
                    text,
                    max_bubble_width - BUBBLE_PADDING_X * 2.0,
                    MESSAGE_FONT_SIZE,
                );
                let mut bubble_height =
                    BUBBLE_PADDING_Y * 2.0 + MESSAGE_FONT_SIZE * 1.35 * lines as f32;
                if message.is_streaming {
                    bubble_height += STREAMING_LABEL_SIZE * 1.5;
                }
                bubble_height = bubble_height.max(BUBBLE_MIN_HEIGHT);

                let bubble_width = max_bubble_width;
                cursor_y -= bubble_height;
                if cursor_y < transcript_bounds.y() + 4.0 {
                    break;
                }

                let bubble_x = if matches!(message.role, CodexMessageRole::User) {
                    transcript_bounds.x() + transcript_bounds.width() - bubble_width - 6.0
                } else {
                    transcript_bounds.x() + 6.0
                };
                let bubble_bounds = Bounds::new(bubble_x, cursor_y, bubble_width, bubble_height);
                let (bubble_bg, bubble_fg) = Self::bubble_colors(message.role);
                paint.scene.draw_quad(
                    Quad::new(bubble_bounds)
                        .with_background(bubble_bg)
                        .with_corner_radius(12.0),
                );

                let mut msg_text = Text::new(text)
                    .font_size(MESSAGE_FONT_SIZE)
                    .color(bubble_fg);
                msg_text.paint(
                    Bounds::new(
                        bubble_bounds.x() + BUBBLE_PADDING_X,
                        bubble_bounds.y() + BUBBLE_PADDING_Y,
                        bubble_bounds.width() - BUBBLE_PADDING_X * 2.0,
                        bubble_bounds.height() - BUBBLE_PADDING_Y * 2.0,
                    ),
                    paint,
                );

                if message.is_streaming {
                    let mut label = Text::new("streaming")
                        .font_size(STREAMING_LABEL_SIZE)
                        .color(theme::text::MUTED)
                        .no_wrap();
                    label.paint(
                        Bounds::new(
                            bubble_bounds.x() + BUBBLE_PADDING_X,
                            bubble_bounds.y() + bubble_bounds.height() - STREAMING_LABEL_SIZE - 5.0,
                            bubble_bounds.width() - BUBBLE_PADDING_X * 2.0,
                            STREAMING_LABEL_SIZE + 2.0,
                        ),
                        paint,
                    );
                }

                cursor_y -= MESSAGE_GAP;
            }
        }

        /// Render one frame: black background + dots + codex transcript + controls.
        pub fn render(&mut self) -> Result<(), String> {
            let panel_bounds = self.primary_panel_bounds();
            let (
                transcript_bounds,
                context_bounds,
                new_thread_bounds,
                interrupt_bounds,
                composer_bounds,
                send_bounds,
            ) = self.controls_layout();
            let (model_bounds, reasoning_bounds) = self.context_chip_bounds(context_bounds);

            let message_snapshot = self.codex_messages.clone();
            let empty_title = self.empty_title.clone();
            let empty_detail = self.empty_detail.clone();
            let model_label = self.model_label.clone();
            let reasoning_label = self.reasoning_label.clone();
            let active_thread_label = self.active_thread_label.clone();
            let active_turn_label = self.active_turn_label.clone();
            let composer_text = self.composer_text.clone();
            let ops_toggle_bounds = self.ops_toggle_bounds();
            let operator_panel_visible = self.operator_panel_visible;
            let auth_email = self.auth_email.clone();
            let auth_code = self.auth_code.clone();
            let auth_status = self.auth_status.clone();
            let worker_status = self.worker_status.clone();
            let stream_status = self.stream_status.clone();
            let handshake_status = self.handshake_status.clone();
            let device_status = self.device_status.clone();
            let telemetry_text = self.telemetry_text.clone();
            let events_text = self.events_text.clone();
            let control_text = self.control_text.clone();
            let active_input_target = self.active_input_target;
            let operator_panel_bounds = self.operator_panel_bounds();
            let operator_layout = self.operator_layout(operator_panel_bounds);

            let mut scene = Scene::new();
            let mut paint = PaintContext::new(&mut scene, &mut self.text_system, self.scale);

            paint.scene.draw_quad(
                Quad::new(Bounds::new(0.0, 0.0, self.size.width, self.size.height))
                    .with_background(crate::color::Hsla::black()),
            );

            // Puffs floating up along grid lines (Arwes-style)
            let now = Instant::now();
            let delta = self
                .last_frame
                .map(|t| now.saturating_duration_since(t))
                .unwrap_or(Duration::ZERO);
            self.last_frame = Some(now);
            self.puffs.update_with_delta(AnimatorState::Entered, delta);
            let grid_bounds = Bounds::new(0.0, 0.0, self.size.width, self.size.height);
            self.puffs.paint(grid_bounds, &mut paint);

            // Dots grid (same params as autopilot_ui MinimalRoot)
            let mut dots_grid = DotsGrid::new()
                .shape(DotShape::Circle)
                .color(theme::text::MUTED)
                .opacity(0.12)
                .distance(GRID_DOT_DISTANCE)
                .size(1.5);
            dots_grid.paint(grid_bounds, &mut paint);

            paint.scene.draw_quad(
                Quad::new(panel_bounds)
                    .with_background(theme::bg::APP.with_alpha(0.88))
                    .with_border(theme::border::DEFAULT, 1.0)
                    .with_corner_radius(PANEL_CORNER_RADIUS),
            );

            let mut title = Text::new("Codex")
                .font_size(24.0)
                .color(theme::text::PRIMARY)
                .no_wrap();
            title.paint(
                Bounds::new(
                    panel_bounds.x() + 12.0,
                    panel_bounds.y() + 10.0,
                    panel_bounds.width() - 24.0,
                    TITLE_HEIGHT,
                ),
                &mut paint,
            );

            let mut ops_toggle = Button::new(if operator_panel_visible {
                "Close Ops"
            } else {
                "Open Ops"
            })
            .variant(ButtonVariant::Secondary);
            ops_toggle.paint(ops_toggle_bounds, &mut paint);

            paint.scene.draw_quad(
                Quad::new(transcript_bounds)
                    .with_background(theme::bg::SURFACE.with_alpha(0.92))
                    .with_border(theme::border::DEFAULT.with_alpha(0.8), 1.0)
                    .with_corner_radius(10.0),
            );
            Self::render_messages(
                &message_snapshot,
                &empty_title,
                &empty_detail,
                transcript_bounds,
                &mut paint,
            );

            paint.scene.draw_quad(
                Quad::new(model_bounds)
                    .with_background(theme::bg::SURFACE.with_alpha(0.9))
                    .with_border(theme::border::DEFAULT.with_alpha(0.85), 1.0)
                    .with_corner_radius(8.0),
            );
            paint.scene.draw_quad(
                Quad::new(reasoning_bounds)
                    .with_background(theme::bg::SURFACE.with_alpha(0.9))
                    .with_border(theme::border::DEFAULT.with_alpha(0.85), 1.0)
                    .with_corner_radius(8.0),
            );

            let mut model = Text::new(&model_label)
                .font_size(SMALL_TEXT_SIZE)
                .color(theme::text::PRIMARY)
                .no_wrap();
            model.paint(
                Bounds::new(
                    model_bounds.x() + 8.0,
                    model_bounds.y() + 3.0,
                    model_bounds.width() - 12.0,
                    model_bounds.height() - 6.0,
                ),
                &mut paint,
            );
            let mut reasoning = Text::new(&reasoning_label)
                .font_size(SMALL_TEXT_SIZE)
                .color(theme::text::PRIMARY)
                .no_wrap();
            reasoning.paint(
                Bounds::new(
                    reasoning_bounds.x() + 8.0,
                    reasoning_bounds.y() + 3.0,
                    reasoning_bounds.width() - 12.0,
                    reasoning_bounds.height() - 6.0,
                ),
                &mut paint,
            );

            let mut thread_text = Text::new(&active_thread_label)
                .font_size(SMALL_TEXT_SIZE)
                .color(theme::text::MUTED)
                .no_wrap();
            thread_text.paint(
                Bounds::new(
                    context_bounds.x() + context_bounds.width() * 0.58,
                    context_bounds.y(),
                    context_bounds.width() * 0.42,
                    SMALL_TEXT_SIZE + 3.0,
                ),
                &mut paint,
            );
            let mut turn_text = Text::new(&active_turn_label)
                .font_size(SMALL_TEXT_SIZE)
                .color(theme::text::MUTED)
                .no_wrap();
            turn_text.paint(
                Bounds::new(
                    context_bounds.x() + context_bounds.width() * 0.58,
                    context_bounds.y() + SMALL_TEXT_SIZE + 4.0,
                    context_bounds.width() * 0.42,
                    SMALL_TEXT_SIZE + 3.0,
                ),
                &mut paint,
            );

            let mut new_thread = Button::new("New Thread").variant(ButtonVariant::Secondary);
            new_thread.paint(new_thread_bounds, &mut paint);
            let mut interrupt = Button::new("Interrupt").variant(ButtonVariant::Secondary);
            interrupt.paint(interrupt_bounds, &mut paint);

            let mut composer = TextInput::new().placeholder("Message Codex");
            composer.set_value(&composer_text);
            composer.set_focused(active_input_target == InputTarget::Composer);
            composer.paint(composer_bounds, &mut paint);

            let mut send = Button::new("Send").variant(ButtonVariant::Primary);
            send.paint(send_bounds, &mut paint);

            if operator_panel_visible {
                paint.scene.draw_quad(
                    Quad::new(operator_panel_bounds)
                        .with_background(theme::bg::APP.with_alpha(0.96))
                        .with_border(theme::border::DEFAULT, 1.0)
                        .with_corner_radius(12.0),
                );

                let mut ops_title = Text::new("Codex Operator")
                    .font_size(16.0)
                    .color(theme::text::PRIMARY)
                    .no_wrap();
                ops_title.paint(
                    Bounds::new(
                        operator_panel_bounds.x() + OPS_PANEL_PADDING,
                        operator_panel_bounds.y() + 8.0,
                        operator_panel_bounds.width() - OPS_PANEL_PADDING * 2.0,
                        18.0,
                    ),
                    &mut paint,
                );

                let (
                    auth_email_bounds,
                    auth_code_bounds,
                    send_code_bounds,
                    verify_bounds,
                    signout_bounds,
                    refresh_workers_bounds,
                    connect_bounds,
                    disconnect_bounds,
                    handshake_bounds,
                    status_block_bounds,
                    telemetry_block_bounds,
                    events_block_bounds,
                    controls_block_bounds,
                ) = operator_layout;

                let mut email_input = TextInput::new().placeholder("Email");
                email_input.set_value(&auth_email);
                email_input.set_focused(active_input_target == InputTarget::AuthEmail);
                email_input.paint(auth_email_bounds, &mut paint);

                let mut code_input = TextInput::new().placeholder("Verification Code");
                code_input.set_value(&auth_code);
                code_input.set_focused(active_input_target == InputTarget::AuthCode);
                code_input.paint(auth_code_bounds, &mut paint);

                let mut send_code = Button::new("Send Code").variant(ButtonVariant::Secondary);
                send_code.paint(send_code_bounds, &mut paint);
                let mut verify = Button::new("Verify").variant(ButtonVariant::Secondary);
                verify.paint(verify_bounds, &mut paint);
                let mut sign_out = Button::new("Sign Out").variant(ButtonVariant::Secondary);
                sign_out.paint(signout_bounds, &mut paint);
                let mut refresh_workers =
                    Button::new("Load Workers").variant(ButtonVariant::Secondary);
                refresh_workers.paint(refresh_workers_bounds, &mut paint);
                let mut connect = Button::new("Connect").variant(ButtonVariant::Secondary);
                connect.paint(connect_bounds, &mut paint);
                let mut disconnect = Button::new("Disconnect").variant(ButtonVariant::Secondary);
                disconnect.paint(disconnect_bounds, &mut paint);
                let mut send_handshake = Button::new("Handshake").variant(ButtonVariant::Secondary);
                send_handshake.paint(handshake_bounds, &mut paint);

                for block in [
                    status_block_bounds,
                    telemetry_block_bounds,
                    events_block_bounds,
                    controls_block_bounds,
                ] {
                    paint.scene.draw_quad(
                        Quad::new(block)
                            .with_background(theme::bg::SURFACE.with_alpha(0.92))
                            .with_border(theme::border::DEFAULT.with_alpha(0.9), 1.0)
                            .with_corner_radius(8.0),
                    );
                }

                let status_text = format!(
                    "auth: {}\nworker: {}\nstream: {} | handshake: {}\n{}",
                    auth_status, worker_status, stream_status, handshake_status, device_status
                );
                let mut status_view = Text::new(status_text)
                    .font_size(11.5)
                    .color(theme::text::MUTED);
                status_view.paint(status_block_bounds.inset(8.0), &mut paint);

                let mut telemetry_view = Text::new(format!("telemetry\n{}", telemetry_text))
                    .font_size(11.5)
                    .color(theme::text::MUTED);
                telemetry_view.paint(telemetry_block_bounds.inset(8.0), &mut paint);

                let mut events_view = Text::new(format!("events\n{}", events_text))
                    .font_size(11.5)
                    .color(theme::text::MUTED);
                events_view.paint(events_block_bounds.inset(8.0), &mut paint);

                let mut controls_view = Text::new(format!("control requests\n{}", control_text))
                    .font_size(11.5)
                    .color(theme::text::MUTED);
                controls_view.paint(controls_block_bounds.inset(8.0), &mut paint);
            }

            self.renderer.resize(&self.queue, self.size, self.scale);

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
            let logical_width = width.max(1);
            let logical_height = height.max(1);
            self.config.width = Self::logical_to_physical(logical_width, self.scale);
            self.config.height = Self::logical_to_physical(logical_height, self.scale);
            self.surface.configure(&self.device, &self.config);
            self.size = Size::new(logical_width as f32, logical_height as f32);
        }
    }

    /// C FFI for Swift: create renderer from CAMetalLayer pointer.
    /// `width`/`height` are logical points.
    /// Returns opaque pointer to IosBackgroundState, or null on error.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_create(
        layer_ptr: *mut c_void,
        width: u32,
        height: u32,
        scale: f32,
    ) -> *mut IosBackgroundState {
        eprintln!(
            "[WGPUI Rust] wgpui_ios_background_create called width={} height={} scale={}",
            width, height, scale
        );
        if layer_ptr.is_null() {
            eprintln!("[WGPUI Rust] create: layer_ptr is null");
            return std::ptr::null_mut();
        }
        match unsafe { IosBackgroundState::new(layer_ptr, width, height, scale) } {
            Ok(state) => {
                eprintln!("[WGPUI Rust] create: OK");
                Box::into_raw(state)
            }
            Err(e) => {
                eprintln!("[WGPUI Rust] create FAILED: {}", e);
                std::ptr::null_mut()
            }
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
            Err(e) => {
                eprintln!("[WGPUI Rust] render FAILED: {}", e);
                0
            }
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

    /// C FFI: handle tap at logical point coordinates (origin top-left). Call from Swift on tap.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_handle_tap(
        state: *mut IosBackgroundState,
        x: f32,
        y: f32,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.handle_tap(x, y);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_clear_codex_messages(state: *mut IosBackgroundState) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.clear_codex_messages();
    }

    /// role: user=0 assistant=1 reasoning=2 tool=3 system=4 error=5
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_push_codex_message(
        state: *mut IosBackgroundState,
        role: u8,
        text_ptr: *const c_char,
        text_len: usize,
        streaming: i32,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        let ptr_u8 = if text_ptr.is_null() {
            std::ptr::null()
        } else {
            text_ptr as *const u8
        };
        state.push_codex_message(role, ptr_u8, text_len, streaming != 0);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_codex_context(
        state: *mut IosBackgroundState,
        thread_ptr: *const c_char,
        thread_len: usize,
        turn_ptr: *const c_char,
        turn_len: usize,
        model_ptr: *const c_char,
        model_len: usize,
        reasoning_ptr: *const c_char,
        reasoning_len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_codex_context_utf8(
            if thread_ptr.is_null() {
                std::ptr::null()
            } else {
                thread_ptr as *const u8
            },
            thread_len,
            if turn_ptr.is_null() {
                std::ptr::null()
            } else {
                turn_ptr as *const u8
            },
            turn_len,
            if model_ptr.is_null() {
                std::ptr::null()
            } else {
                model_ptr as *const u8
            },
            model_len,
            if reasoning_ptr.is_null() {
                std::ptr::null()
            } else {
                reasoning_ptr as *const u8
            },
            reasoning_len,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_empty_state(
        state: *mut IosBackgroundState,
        title_ptr: *const c_char,
        title_len: usize,
        detail_ptr: *const c_char,
        detail_len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_empty_state_utf8(
            if title_ptr.is_null() {
                std::ptr::null()
            } else {
                title_ptr as *const u8
            },
            title_len,
            if detail_ptr.is_null() {
                std::ptr::null()
            } else {
                detail_ptr as *const u8
            },
            detail_len,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_auth_fields(
        state: *mut IosBackgroundState,
        email_ptr: *const c_char,
        email_len: usize,
        code_ptr: *const c_char,
        code_len: usize,
        auth_status_ptr: *const c_char,
        auth_status_len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_auth_fields_utf8(
            if email_ptr.is_null() {
                std::ptr::null()
            } else {
                email_ptr as *const u8
            },
            email_len,
            if code_ptr.is_null() {
                std::ptr::null()
            } else {
                code_ptr as *const u8
            },
            code_len,
            if auth_status_ptr.is_null() {
                std::ptr::null()
            } else {
                auth_status_ptr as *const u8
            },
            auth_status_len,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_operator_status(
        state: *mut IosBackgroundState,
        worker_status_ptr: *const c_char,
        worker_status_len: usize,
        stream_status_ptr: *const c_char,
        stream_status_len: usize,
        handshake_status_ptr: *const c_char,
        handshake_status_len: usize,
        device_status_ptr: *const c_char,
        device_status_len: usize,
        telemetry_ptr: *const c_char,
        telemetry_len: usize,
        events_ptr: *const c_char,
        events_len: usize,
        control_ptr: *const c_char,
        control_len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_operator_status_utf8(
            if worker_status_ptr.is_null() {
                std::ptr::null()
            } else {
                worker_status_ptr as *const u8
            },
            worker_status_len,
            if stream_status_ptr.is_null() {
                std::ptr::null()
            } else {
                stream_status_ptr as *const u8
            },
            stream_status_len,
            if handshake_status_ptr.is_null() {
                std::ptr::null()
            } else {
                handshake_status_ptr as *const u8
            },
            handshake_status_len,
            if device_status_ptr.is_null() {
                std::ptr::null()
            } else {
                device_status_ptr as *const u8
            },
            device_status_len,
            if telemetry_ptr.is_null() {
                std::ptr::null()
            } else {
                telemetry_ptr as *const u8
            },
            telemetry_len,
            if events_ptr.is_null() {
                std::ptr::null()
            } else {
                events_ptr as *const u8
            },
            events_len,
            if control_ptr.is_null() {
                std::ptr::null()
            } else {
                control_ptr as *const u8
            },
            control_len,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_composer_text(
        state: *mut IosBackgroundState,
        ptr: *const c_char,
        len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        let ptr_u8 = if ptr.is_null() {
            std::ptr::null()
        } else {
            ptr as *const u8
        };
        state.set_composer_text_utf8(ptr_u8, len);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_auth_email(
        state: *mut IosBackgroundState,
        ptr: *const c_char,
        len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        let ptr_u8 = if ptr.is_null() {
            std::ptr::null()
        } else {
            ptr as *const u8
        };
        IosBackgroundState::set_utf8_string(&mut state.auth_email, ptr_u8, len);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_auth_code(
        state: *mut IosBackgroundState,
        ptr: *const c_char,
        len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        let ptr_u8 = if ptr.is_null() {
            std::ptr::null()
        } else {
            ptr as *const u8
        };
        IosBackgroundState::set_utf8_string(&mut state.auth_code, ptr_u8, len);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_active_input_target(
        state: *mut IosBackgroundState,
    ) -> u8 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &*state };
        state.active_input_target().as_u8()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_active_input_target(
        state: *mut IosBackgroundState,
        target: u8,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_active_input_target(InputTarget::from_u8(target));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_composer_focused(state: *mut IosBackgroundState) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &*state };
        if state.composer_focused() { 1 } else { 0 }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_composer_focused(
        state: *mut IosBackgroundState,
        focused: i32,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_composer_focused(focused != 0);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_send_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_send_requested() { 1 } else { 0 }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_new_thread_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_new_thread_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_interrupt_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_interrupt_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_model_cycle_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_model_cycle_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_reasoning_cycle_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_reasoning_cycle_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_send_code_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_send_code_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_verify_code_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_verify_code_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_sign_out_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_sign_out_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_refresh_workers_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_refresh_workers_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_connect_stream_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_connect_stream_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_disconnect_stream_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_disconnect_stream_requested() {
            1
        } else {
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_send_handshake_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_send_handshake_requested() {
            1
        } else {
            0
        }
    }

    /// Backward-compatible alias for older iOS bridge code.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_login_submit_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.send_code_requested { 1 } else { 0 }
    }

    /// Backward-compatible alias for older iOS bridge code.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_consume_submit_requested(
        state: *mut IosBackgroundState,
    ) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &mut *state };
        if state.consume_send_code_requested() {
            1
        } else {
            0
        }
    }

    /// Backward-compatible alias for older iOS bridge code.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_email_focused(state: *mut IosBackgroundState) -> i32 {
        if state.is_null() {
            return 0;
        }
        let state = unsafe { &*state };
        if state.active_input_target() == InputTarget::AuthEmail {
            1
        } else {
            0
        }
    }

    /// Backward-compatible alias for older iOS bridge code.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_email_focused(
        state: *mut IosBackgroundState,
        focused: i32,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        state.set_active_input_target(if focused != 0 {
            InputTarget::AuthEmail
        } else {
            InputTarget::None
        });
    }

    /// Backward-compatible alias for older iOS bridge code.
    #[unsafe(no_mangle)]
    pub extern "C" fn wgpui_ios_background_set_login_email(
        state: *mut IosBackgroundState,
        ptr: *const c_char,
        len: usize,
    ) {
        if state.is_null() {
            return;
        }
        let state = unsafe { &mut *state };
        if ptr.is_null() || len == 0 {
            IosBackgroundState::set_utf8_string(&mut state.auth_email, std::ptr::null(), 0);
            return;
        }
        let ptr_u8 = ptr as *const u8;
        IosBackgroundState::set_utf8_string(&mut state.auth_email, ptr_u8, len);
    }
}
