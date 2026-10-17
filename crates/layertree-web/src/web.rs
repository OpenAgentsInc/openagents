//! The page: canvas, renderer, orbit input, and the frame loop.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use verse::grid_engine::GridEngine;
use verse::imported::Gpu;
use verse::ui::Atlas;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent, Window};

use crate::scene::{self, Orbit};
use crate::stage::Stage;

/// The page's canvas.
const CANVAS_ID: &str = "layertree-canvas";
/// The element that shows progress and errors.
const STATUS_ID: &str = "layertree-status";
/// The largest drawing-buffer side, in device pixels.
const MAX_SIDE: u32 = 4096;
/// The longest frame step; a hidden tab resumes without a jump.
const MAX_STEP: f32 = 0.1;
/// Seconds without input before the camera turns on its own again.
const IDLE: f32 = 5.0;

/// Starts the scene when the module's `init` finishes.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    if log::set_logger(&CONSOLE).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            web_sys::console::error_1(&JsValue::from_str(&error));
            status("The LayerTree demo couldn't start. Reload the page to try again.");
        }
    });
}

/// Forwards warnings and errors, such as wgpu's validation errors, to the
/// browser console.
struct Console;

static CONSOLE: Console = Console;

impl log::Log for Console {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let text = JsValue::from_str(&format!("{}: {}", record.target(), record.args()));
            web_sys::console::warn_1(&text);
        }
    }

    fn flush(&self) {}
}

#[derive(Debug)]
struct WebDisplay;

impl raw_window_handle::HasDisplayHandle for WebDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::web())
    }
}

fn status(text: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(STATUS_ID))
    {
        element.set_text_content(Some(text));
    }
}

fn drawing_size(canvas: &HtmlCanvasElement, scale: f32) -> (u32, u32) {
    let side = |css: i32| ((css.max(1) as f32 * scale).round() as u32).clamp(1, MAX_SIDE);
    (side(canvas.client_width()), side(canvas.client_height()))
}

async fn open(
    canvas: &HtmlCanvasElement,
    backends: wgpu::Backends,
    atlas: &Atlas,
    width: u32,
    height: u32,
) -> Result<GridEngine, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    descriptor.backends = backends;
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    let gpu = Gpu::open_async(instance, &surface).await?;
    GridEngine::on_surface_with(gpu, surface, scene::content()?, atlas, width, height)
}

/// Everything the frame loop owns.
struct Page {
    canvas: HtmlCanvasElement,
    engine: GridEngine,
    atlas: Atlas,
    stage: Stage,
    orbit: Orbit,
    scale: f32,
    size: (u32, u32),
    last: Option<f64>,
    /// Seconds since the last input.
    idle: f32,
    /// Pointers held down, by ID, at their last position.
    pointers: HashMap<i32, (f32, f32)>,
    /// Touch devices have no keyboard to switch runs.
    touch: bool,
    drawn: bool,
}

impl Page {
    fn frame(&mut self, now: f64) -> Result<(), String> {
        let dt = self
            .last
            .map_or(0.0, |last| ((now - last) / 1000.0) as f32)
            .clamp(0.0, MAX_STEP);
        self.last = Some(now);
        self.idle += dt;
        if self.idle > IDLE {
            self.orbit.yaw += dt * 0.05;
        }
        self.stage.advance(dt)?;
        let size = drawing_size(&self.canvas, self.scale);
        if size != self.size {
            self.canvas.set_width(size.0);
            self.canvas.set_height(size.1);
            self.engine.resize(size.0, size.1)?;
            self.size = size;
        }
        let pixels = [size.0 as f32, size.1 as f32];
        let view = self
            .orbit
            .view(&self.stage.shape, pixels[0] / pixels[1].max(1.0));
        let instances = scene::instances(&self.stage);
        let ui = scene::overlay(
            &self.stage,
            view,
            pixels,
            self.scale,
            &self.atlas,
            !self.touch,
        );
        self.engine
            .draw(view, &instances, &ui, &scene::lighting())?;
        if !self.drawn {
            self.drawn = true;
            status("");
        }
        Ok(())
    }

    fn touched(&mut self) {
        self.idle = 0.0;
    }

    fn pointer_down(&mut self, event: &PointerEvent) {
        self.touched();
        self.touch |= event.pointer_type() == "touch";
        self.pointers.insert(
            event.pointer_id(),
            (event.client_x() as f32, event.client_y() as f32),
        );
        let _ = self.canvas.set_pointer_capture(event.pointer_id());
    }

    fn pointer_move(&mut self, event: &PointerEvent) {
        let id = event.pointer_id();
        let Some(&(x0, y0)) = self.pointers.get(&id) else {
            return;
        };
        self.touched();
        let (x, y) = (event.client_x() as f32, event.client_y() as f32);
        if self.pointers.len() >= 2 {
            // Pinch: the spread between this pointer and another.
            let other = self
                .pointers
                .iter()
                .find(|(k, _)| **k != id)
                .map(|(_, p)| *p);
            if let Some((ox, oy)) = other {
                let before = (x0 - ox).hypot(y0 - oy).max(1.0);
                let after = (x - ox).hypot(y - oy).max(1.0);
                self.zoom(before / after);
            }
        } else {
            self.orbit.yaw -= (x - x0) * 0.006;
            self.orbit.pitch = (self.orbit.pitch + (y - y0) * 0.004).clamp(0.05, 1.35);
        }
        self.pointers.insert(id, (x, y));
    }

    fn pointer_up(&mut self, event: &PointerEvent) {
        self.pointers.remove(&event.pointer_id());
    }

    fn zoom(&mut self, factor: f32) {
        self.orbit.zoom = (self.orbit.zoom * factor).clamp(0.3, 2.5);
    }

    fn key(&mut self, event: &KeyboardEvent) -> Result<(), String> {
        self.touched();
        match event.key().as_str() {
            "ArrowLeft" => self.orbit.yaw += 0.08,
            "ArrowRight" => self.orbit.yaw -= 0.08,
            "ArrowUp" => self.zoom(0.9),
            "ArrowDown" => self.zoom(1.1),
            "r" | "R" => {
                let time = self.stage.time;
                self.stage.switch()?;
                self.stage.time = time;
            }
            _ => return Ok(()),
        }
        event.prevent_default();
        Ok(())
    }
}

fn listen<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    page: &Rc<RefCell<Page>>,
    handle: impl Fn(&mut Page, &E) + 'static,
) -> Result<(), String> {
    let page = page.clone();
    let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        if let (Ok(event), Ok(mut page)) = (event.dyn_into::<E>(), page.try_borrow_mut()) {
            handle(&mut page, &event);
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

async fn run() -> Result<(), String> {
    let window: Window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(CANVAS_ID)
        .ok_or("the page has no LayerTree canvas")?
        .dyn_into()
        .map_err(|_| "the LayerTree canvas is not a canvas")?;
    let _ = canvas.style().set_property("touch-action", "none");
    let query = window.location().search().unwrap_or_default();
    status("Building the tree\u{2026}");
    let stage = Stage::from_query(&query)?;

    let scale = (window.device_pixel_ratio() as f32).clamp(1.0, 3.0);
    let (width, height) = drawing_size(&canvas, scale);
    canvas.set_width(width);
    canvas.set_height(height);
    let atlas = Atlas::new((14.0 * scale).round());
    // `?gl` forces WebGL2, the path browsers without WebGPU take.
    let force_gl = query.trim_start_matches('?').split('&').any(|p| p == "gl");
    let backends = if force_gl {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    let engine = match open(&canvas, backends, &atlas, width, height).await {
        Ok(engine) => engine,
        Err(first) if !force_gl => {
            log::warn!("WebGPU failed ({first}); trying WebGL2");
            open(&canvas, wgpu::Backends::GL, &atlas, width, height).await?
        }
        Err(error) => return Err(error),
    };
    let touch = window.navigator().max_touch_points() > 0;
    let page = Rc::new(RefCell::new(Page {
        canvas: canvas.clone(),
        engine,
        atlas,
        stage,
        orbit: Orbit::default(),
        scale,
        size: (width, height),
        last: None,
        idle: IDLE,
        pointers: HashMap::new(),
        touch,
        drawn: false,
    }));

    let target: &web_sys::EventTarget = canvas.as_ref();
    listen::<PointerEvent>(target, "pointerdown", &page, |p, e| p.pointer_down(e))?;
    listen::<PointerEvent>(target, "pointermove", &page, |p, e| p.pointer_move(e))?;
    listen::<PointerEvent>(target, "pointerup", &page, |p, e| p.pointer_up(e))?;
    listen::<PointerEvent>(target, "pointercancel", &page, |p, e| p.pointer_up(e))?;
    listen::<WheelEvent>(target, "wheel", &page, |p, e| {
        e.prevent_default();
        p.touched();
        p.zoom((e.delta_y() as f32 * 0.0012).exp());
    })?;
    listen::<KeyboardEvent>(window.as_ref(), "keydown", &page, |p, e| {
        if let Err(error) = p.key(e) {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
    })?;

    // The frame loop.
    let tick: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let next = tick.clone();
    *tick.borrow_mut() = Some(Closure::new(move |now: f64| {
        let result = page.borrow_mut().frame(now);
        if let Err(error) = result {
            web_sys::console::error_1(&JsValue::from_str(&error));
            status("The LayerTree demo stopped. Reload the page to start it again.");
            return;
        }
        if let (Some(window), Some(callback)) = (web_sys::window(), next.borrow().as_ref()) {
            let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    window
        .request_animation_frame(
            tick.borrow()
                .as_ref()
                .ok_or("no frame callback")?
                .as_ref()
                .unchecked_ref(),
        )
        .map_err(|_| "cannot request a frame")?;
    Ok(())
}
