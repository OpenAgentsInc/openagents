//! Web window implementation using wgpu

use crate::{
    platform::{
        AtlasTile, Capslock, Decorations, Modifiers, PlatformAtlas, PlatformDisplay,
        PlatformInputHandler, PlatformWindow, PromptButton, PromptLevel, RequestFrameOptions,
        WindowBounds, WindowControls,
    },
    AnyWindowHandle, Bounds, DevicePixels, DispatchEventResult, GpuSpecs, Pixels, PlatformInput,
    Point, Scene, Size, WindowBackgroundAppearance, WindowAppearance, WindowControlArea,
    WindowParams,
};
use anyhow::Result;
use futures::channel::oneshot;
use parking_lot::Mutex;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WebDisplayHandle, WebWindowHandle, WindowHandle,
};
use std::{borrow::Cow, rc::Rc, sync::Arc};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::platform::WebDisplay;

/// Web window implementation
pub struct WebWindow {
    handle: AnyWindowHandle,
    canvas: HtmlCanvasElement,
    bounds: Mutex<Bounds<Pixels>>,
    scale_factor: f32,
    input_handler: Mutex<Option<PlatformInputHandler>>,
    callbacks: Mutex<WebWindowCallbacks>,
    atlas: Arc<WebAtlas>,
    // wgpu resources
    #[allow(dead_code)]
    surface: Option<wgpu::Surface<'static>>,
    #[allow(dead_code)]
    device: Option<wgpu::Device>,
    #[allow(dead_code)]
    queue: Option<wgpu::Queue>,
}

#[derive(Default)]
struct WebWindowCallbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    hover_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved: Option<Box<dyn FnMut()>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    hit_test: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
}

impl WebWindow {
    pub fn new(handle: AnyWindowHandle, options: WindowParams) -> Result<Self> {
        let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("No window object"))?;
        let document = window
            .document()
            .ok_or_else(|| anyhow::anyhow!("No document"))?;

        // Create or get canvas
        let canvas: HtmlCanvasElement = document
            .create_element("canvas")?
            .dyn_into()
            .map_err(|_| anyhow::anyhow!("Failed to create canvas"))?;

        canvas.set_id(&format!("gpui-canvas-{}", handle.window_id().as_u64()));

        // Style the canvas
        let style = canvas.style();
        style.set_property("width", "100%")?;
        style.set_property("height", "100%")?;
        style.set_property("display", "block")?;

        // Set canvas size based on options or viewport
        let width = options.bounds.size.width.0;
        let height = options.bounds.size.height.0;
        let scale_factor = window.device_pixel_ratio() as f32;

        canvas.set_width((width * scale_factor) as u32);
        canvas.set_height((height * scale_factor) as u32);

        // Append to body
        if let Some(body) = document.body() {
            body.append_child(&canvas)?;
        }

        let bounds = Mutex::new(Bounds {
            origin: Point::zero(),
            size: Size {
                width: Pixels(width),
                height: Pixels(height),
            },
        });

        // TODO: Initialize wgpu asynchronously
        // For now, we'll leave surface/device/queue as None
        // and initialize them lazily or via an async init method

        Ok(Self {
            handle,
            canvas,
            bounds,
            scale_factor,
            input_handler: Mutex::new(None),
            callbacks: Mutex::new(WebWindowCallbacks::default()),
            atlas: Arc::new(WebAtlas::new()),
            surface: None,
            device: None,
            queue: None,
        })
    }
}

impl HasWindowHandle for WebWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // Web windows use a canvas element ID
        let mut handle = WebWindowHandle::new(self.handle.window_id().as_u64() as u32);
        let raw = RawWindowHandle::Web(handle);
        // SAFETY: The canvas element remains valid for the lifetime of WebWindow
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for WebWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let handle = WebDisplayHandle::new();
        let raw = RawDisplayHandle::Web(handle);
        // SAFETY: Web display is always valid
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}

impl PlatformWindow for WebWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        *self.bounds.lock()
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(*self.bounds.lock())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.lock().size
    }

    fn resize(&mut self, size: Size<Pixels>) {
        self.bounds.lock().size = size;
        self.canvas
            .set_width((size.width.0 * self.scale_factor) as u32);
        self.canvas
            .set_height((size.height.0 * self.scale_factor) as u32);
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(media_query)) = window.match_media("(prefers-color-scheme: dark)") {
                if media_query.matches() {
                    return WindowAppearance::Dark;
                }
            }
        }
        WindowAppearance::Light
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(WebDisplay::new()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::zero()
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::Disabled
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.lock() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.lock().take()
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        // Use browser's confirm/alert dialogs
        let full_msg = if let Some(detail) = detail {
            format!("{}\n\n{}", msg, detail)
        } else {
            msg.to_string()
        };

        if let Some(window) = web_sys::window() {
            if answers.len() <= 1 {
                let _ = window.alert_with_message(&full_msg);
                let (tx, rx) = oneshot::channel();
                let _ = tx.send(0);
                return Some(rx);
            } else {
                let result = window.confirm_with_message(&full_msg).unwrap_or(false);
                let (tx, rx) = oneshot::channel();
                let _ = tx.send(if result { 0 } else { 1 });
                return Some(rx);
            }
        }

        None
    }

    fn activate(&self) {
        let _ = self.canvas.focus();
    }

    fn is_active(&self) -> bool {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(active) = document.active_element() {
                return active == *self.canvas.clone().dyn_into::<web_sys::Element>().unwrap();
            }
        }
        false
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn set_title(&mut self, title: &str) {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.set_title(title);
        }
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {
        // Could set canvas background color
    }

    fn minimize(&self) {
        // No-op on web
    }

    fn zoom(&self) {
        // Could toggle fullscreen
    }

    fn toggle_fullscreen(&self) {
        let _ = self.canvas.request_fullscreen();
    }

    fn is_fullscreen(&self) -> bool {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            return document.fullscreen();
        }
        false
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.callbacks.lock().request_frame = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.callbacks.lock().input = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.lock().active_status_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.lock().hover_status_change = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.callbacks.lock().resize = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.lock().moved = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.callbacks.lock().should_close = Some(callback);
    }

    fn on_hit_test_window_control(
        &self,
        callback: Box<dyn FnMut() -> Option<WindowControlArea>>,
    ) {
        self.callbacks.lock().hit_test = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.callbacks.lock().close = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.lock().appearance_changed = Some(callback);
    }

    fn draw(&self, _scene: &Scene) {
        // TODO: Render scene using wgpu
        // For now, just clear to a color
        log::debug!("WebWindow::draw called");
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        // TODO: Get actual GPU info from wgpu adapter
        Some(GpuSpecs {
            is_software_emulated: false,
            device_name: Some("WebGPU".into()),
            driver_name: None,
            driver_info: None,
        })
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {
        // TODO: Position IME candidate window
    }
}

/// Web atlas for texture management
pub struct WebAtlas {
    // TODO: Implement texture atlas
}

impl WebAtlas {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WebAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAtlas for WebAtlas {
    fn get_or_insert_with<'a>(
        &self,
        _key: &crate::platform::AtlasKey,
        _build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        // TODO: Implement atlas
        Ok(None)
    }

    fn remove(&self, _key: &crate::platform::AtlasKey) {
        // TODO: Implement atlas removal
    }
}
