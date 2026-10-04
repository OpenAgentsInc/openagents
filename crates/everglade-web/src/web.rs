//! The page: canvas, pack download, renderer, events, and the frame loop.
use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Reflect, Uint8Array};
use verse::render::{DrawStatus, RenderOptions, Renderer};
use verse::runtime::{Action, WorldRuntime};
use verse::ui::Atlas;
use verse::zones::{self, everglade_pack};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent, RequestCredentials,
    RequestInit, RequestRedirect, Response, WheelEvent, Window,
};

use crate::input::Input;

/// The page's canvas.
const CANVAS_ID: &str = "everglade-canvas";
/// The element that shows progress and errors. Created when the page has
/// none.
const STATUS_ID: &str = "everglade-status";
/// The largest drawing-buffer side, in device pixels.
const MAX_SIDE: u32 = 4096;
/// The longest frame step; a hidden tab resumes without a jump.
const MAX_STEP: f32 = 0.1;

/// Starts Everglade when the module's `init` finishes.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            web_sys::console::error_1(&JsValue::from_str(&error));
            status(&format!("Everglade could not start: {error}"));
        }
    });
}

/// Everything the frame loop owns.
struct Page {
    canvas: HtmlCanvasElement,
    runtime: WorldRuntime,
    renderer: Renderer,
    hud: zones::hud::Hud,
    hud_atlas: Option<Atlas>,
    input: Input,
    rendered_revision: u64,
    /// Device pixels per CSS pixel.
    scale: f32,
    /// The previous frame's `performance.now()`, in milliseconds.
    last: Option<f64>,
    stopped: bool,
}

async fn run() -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| format!("the page has no #{CANVAS_ID}"))?
        .dyn_into()
        .map_err(|_| format!("#{CANVAS_ID} is not a canvas"))?;
    // The canvas takes drags and pinches instead of the page scrolling.
    let _ = canvas.style().set_property("touch-action", "none");

    let bytes = download(&window).await?;
    status("Opening Everglade…");
    let mut runtime = WorldRuntime::new();
    runtime.install_everglade_bytes(&bytes)?;
    drop(bytes);

    let scale = (window.device_pixel_ratio() as f32).clamp(1.0, 3.0);
    let (width, height) = drawing_size(&canvas, scale);
    canvas.set_width(width);
    canvas.set_height(height);
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
    // WebGPU when the browser offers an adapter, otherwise WebGL2.
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    let atlas = Atlas::new((14.0 * scale).round());
    let mut renderer = Renderer::from_surface_async(
        instance,
        surface,
        width,
        height,
        &runtime.world.mesh,
        &atlas,
        RenderOptions {
            max_extent: MAX_SIDE,
            ..RenderOptions::default()
        },
    )
    .await?;
    renderer.set_atmosphere(zones::atmosphere(runtime.zone))?;
    hide_status(&document);

    let page = Rc::new(RefCell::new(Page {
        canvas,
        rendered_revision: runtime.zone_revision,
        runtime,
        renderer,
        hud: zones::hud::Hud::default(),
        hud_atlas: atlas.layout_at_scale(scale),
        input: Input::default(),
        scale,
        last: None,
        stopped: false,
    }));
    listen(&window, &page)?;
    animate(window, page);
    Ok(())
}

/// Fetches the pinned pack from this origin with progress, bounded by its
/// pinned length. The runtime checks its length and digest on install.
async fn download(window: &Window) -> Result<Vec<u8>, String> {
    let url = format!(
        "/everglade/pack/{}.{}",
        everglade_pack::PACK_SHA256,
        everglade_pack::PACK_EXTENSION
    );
    let total = everglade_pack::PACK_BYTES;
    progress(0, total);
    let init = RequestInit::new();
    init.set_redirect(RequestRedirect::Error);
    init.set_credentials(RequestCredentials::Omit);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init(&url, &init))
        .await
        .map_err(|_| "the pack download could not connect".to_owned())?
        .dyn_into()
        .map_err(|_| "the pack download returned no response".to_owned())?;
    if !response.ok() {
        return Err(format!(
            "the pack download failed with HTTP {}",
            response.status()
        ));
    }
    let reader: web_sys::ReadableStreamDefaultReader = response
        .body()
        .ok_or("the pack download has no body")?
        .get_reader()
        .dyn_into()
        .map_err(|_| "the pack download cannot be read".to_owned())?;
    let mut bytes = Vec::with_capacity(total as usize);
    let mut reported = 0;
    loop {
        let chunk = JsFuture::from(reader.read())
            .await
            .map_err(|_| "the pack download was interrupted".to_owned())?;
        let done = Reflect::get(&chunk, &JsValue::from_str("done"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if done {
            break;
        }
        let value: Uint8Array = Reflect::get(&chunk, &JsValue::from_str("value"))
            .ok()
            .and_then(|v| v.dyn_into().ok())
            .ok_or("the pack download sent an unreadable chunk")?;
        let start = bytes.len();
        let end = start + value.length() as usize;
        if end as u64 > total {
            let _ = reader.cancel();
            return Err("the pack download exceeds its pinned size".into());
        }
        bytes.resize(end, 0);
        value.copy_to(&mut bytes[start..]);
        // About a hundred updates per download.
        if end - reported >= (total / 100) as usize || end as u64 == total {
            progress(end as u64, total);
            reported = end;
        }
    }
    if bytes.len() as u64 != total {
        return Err("the pack download ended early".into());
    }
    Ok(bytes)
}

/// Device-pixel drawing size for the canvas's laid-out CSS size.
fn drawing_size(canvas: &HtmlCanvasElement, scale: f32) -> (u32, u32) {
    let side = |css: i32| ((css.max(1) as f32 * scale).round() as u32).clamp(1, MAX_SIDE);
    (side(canvas.client_width()), side(canvas.client_height()))
}

impl Page {
    fn frame(&mut self, now: f64) {
        let dt = self
            .last
            .map_or(0.0, |last| ((now - last) / 1000.0) as f32)
            .clamp(0.0, MAX_STEP);
        self.last = Some(now);

        let (width, height) = drawing_size(&self.canvas, self.scale);
        if (width, height) != (self.canvas.width(), self.canvas.height()) {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            if let Err(error) = self.renderer.resize(width, height) {
                self.fail(&error);
                return;
            }
        }
        self.runtime.zone_tick();
        if self.runtime.zone_revision != self.rendered_revision {
            let replaced = self
                .renderer
                .replace_world(&self.runtime.world.mesh)
                .and_then(|()| {
                    self.renderer
                        .set_atmosphere(zones::atmosphere(self.runtime.zone))
                });
            if let Err(error) = replaced {
                self.fail(&error);
                return;
            }
            self.rendered_revision = self.runtime.zone_revision;
        }
        let input = self.input.take();
        self.runtime
            .tick_with_mode(&input, dt, self.input.orbit, true);

        let size = self.renderer.size();
        let aspect = self.renderer.aspect();
        let view = self.runtime.view(aspect);
        let dynamic = self.runtime.dynamic_mesh();
        let ui = match &self.hud_atlas {
            Some(atlas) => {
                let logical = size.map(|v| v / self.scale);
                let frame = self
                    .hud
                    .snapshot(logical, &self.runtime.zone_snapshot(aspect), true);
                self.hud.draw(atlas, &frame, self.scale)
            }
            None => verse::ui::UiBatch::default(),
        };
        if let DrawStatus::Error(error) = self.renderer.draw(view, &dynamic, &ui) {
            self.fail(&error);
        }
    }

    fn apply(&mut self, action: Option<Action>) {
        if let Some(action) = action {
            let _ = self.runtime.apply(action);
        }
    }

    fn fail(&mut self, error: &str) {
        self.stopped = true;
        web_sys::console::error_1(&JsValue::from_str(error));
        status(&format!("Everglade stopped: {error}"));
    }
}

/// Runs [`Page::frame`] on every animation frame until it stops.
fn animate(window: Window, page: Rc<RefCell<Page>>) {
    type Tick = Closure<dyn FnMut(f64)>;
    let tick: Rc<RefCell<Option<Tick>>> = Rc::new(RefCell::new(None));
    let next = tick.clone();
    let scheduler = window.clone();
    *tick.borrow_mut() = Some(Closure::new(move |now: f64| {
        let mut page = page.borrow_mut();
        page.frame(now);
        if page.stopped {
            // Not rescheduling ends the loop.
            return;
        }
        if let Some(callback) = next.borrow().as_ref() {
            let _ = scheduler.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    if let Some(callback) = tick.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

/// Wires keyboard, mouse, wheel, and touch events for the page's lifetime.
fn listen(window: &Window, page: &Rc<RefCell<Page>>) -> Result<(), String> {
    let canvas = page.borrow().canvas.clone();
    let key = |down: bool| {
        let page = page.clone();
        move |event: KeyboardEvent| {
            if event.ctrl_key() || event.meta_key() || event.alt_key() {
                return;
            }
            if page.borrow_mut().input.key(&event.code(), down) {
                event.prevent_default();
            }
        }
    };
    on(window, "keydown", key(true))?;
    on(window, "keyup", key(false))?;
    {
        let page = page.clone();
        on(window, "blur", move |_: web_sys::Event| {
            page.borrow_mut().input.release()
        })?;
    }
    {
        let page = page.clone();
        let target = canvas.clone();
        on(&canvas, "pointerdown", move |event: PointerEvent| {
            event.prevent_default();
            let _ = target.focus();
            let _ = target.set_pointer_capture(event.pointer_id());
            let mut page = page.borrow_mut();
            if event.pointer_type() == "touch" {
                let at = [event.client_x() as f32, event.client_y() as f32];
                page.input.touch_start(event.pointer_id(), at);
            } else {
                match event.button() {
                    0 => page.input.orbit = true,
                    2 => page.input.look = true,
                    _ => {}
                }
            }
        })?;
    }
    {
        let page = page.clone();
        on(&canvas, "pointermove", move |event: PointerEvent| {
            let mut page = page.borrow_mut();
            let action = if event.pointer_type() == "touch" {
                let at = [event.client_x() as f32, event.client_y() as f32];
                page.input.touch_move(event.pointer_id(), at)
            } else {
                page.input
                    .drag(event.movement_x() as f32, event.movement_y() as f32)
            };
            page.apply(action);
        })?;
    }
    for name in ["pointerup", "pointercancel"] {
        let page = page.clone();
        on(&canvas, name, move |event: PointerEvent| {
            let mut page = page.borrow_mut();
            if event.pointer_type() == "touch" {
                page.input.touch_end(event.pointer_id());
            } else {
                // `buttons` is what stays held after this release.
                page.input.orbit = event.buttons() & 1 != 0;
                page.input.look = event.buttons() & 2 != 0;
            }
        })?;
    }
    {
        let page = page.clone();
        on(&canvas, "wheel", move |event: WheelEvent| {
            event.prevent_default();
            // Positive lines move the camera closer; a wheel line is about
            // 100 pixels.
            let pixels = match event.delta_mode() {
                WheelEvent::DOM_DELTA_LINE => event.delta_y() * 100.0,
                WheelEvent::DOM_DELTA_PAGE => event.delta_y() * 800.0,
                _ => event.delta_y(),
            };
            let lines = (-pixels / 100.0).clamp(-10.0, 10.0) as f32;
            page.borrow_mut().apply(Some(Action::Zoom { lines }));
        })?;
    }
    on(&canvas, "contextmenu", |event: web_sys::Event| {
        event.prevent_default()
    })?;
    Ok(())
}

/// Adds a listener that lives as long as the page. Wheel and touch
/// listeners are not passive, so they may prevent scrolling.
fn on<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) -> Result<(), String> {
    let mut handler = handler;
    let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        if let Ok(event) = event.dyn_into::<E>() {
            handler(event);
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            name,
            callback.as_ref().unchecked_ref(),
            &options,
        )
        .map_err(|_| format!("cannot listen for {name}"))?;
    callback.forget();
    Ok(())
}

/// Shows `text` in the status element, creating it over the canvas when the
/// page has none.
fn status(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(element) = status_element(&document) {
        element.set_text_content(Some(text));
        let _ = element.style().set_property("display", "block");
    }
}

fn progress(received: u64, total: u64) {
    let mb = |bytes: u64| bytes as f64 / 1_000_000.0;
    let percent = (received * 100).checked_div(total).unwrap_or(0);
    status(&format!(
        "Loading Everglade… {percent}% ({:.1} of {:.1} MB)",
        mb(received),
        mb(total)
    ));
}

fn hide_status(document: &Document) {
    if let Some(element) = document
        .get_element_by_id(STATUS_ID)
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    {
        let _ = element.style().set_property("display", "none");
    }
}

fn status_element(document: &Document) -> Option<HtmlElement> {
    if let Some(element) = document.get_element_by_id(STATUS_ID) {
        return element.dyn_into().ok();
    }
    let element: HtmlElement = document.create_element("div").ok()?.dyn_into().ok()?;
    element.set_id(STATUS_ID);
    let style = element.style();
    for (name, value) in [
        ("position", "fixed"),
        ("left", "16px"),
        ("bottom", "16px"),
        ("max-width", "calc(100vw - 32px)"),
        ("padding", "8px 12px"),
        ("border-radius", "6px"),
        ("background", "rgba(10, 14, 8, 0.8)"),
        ("color", "#e8f0d8"),
        ("font", "14px/1.4 system-ui, sans-serif"),
        ("pointer-events", "none"),
    ] {
        let _ = style.set_property(name, value);
    }
    document.body()?.append_child(&element).ok()?;
    Some(element)
}
