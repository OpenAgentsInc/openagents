//! The runner: raw WebGL2 through web-sys, no engine.
//!
//! On a page it starts when the module is instantiated, reading the canvas
//! and the two shader sources from the page. With no WebGL2, or when a
//! shader fails, it returns and leaves the page's static background. Every
//! step returns `Option` rather than panicking, which also keeps the panic
//! machinery out of the module.
//!
//! The same module also runs in a dedicated worker ([`start_worker`]), on
//! an `OffscreenCanvas` the page transferred, so the Grid keeps its frame
//! rate while the page's main thread is busy (`/lander6` hands over to it
//! before loading its demo card). The page then stops its own loop
//! ([`pause`]), and the worker carries on from [`scene_time`].
//!
//! Performance marks for measuring (on a page): `lander5-compile-start` and
//! `lander5-compile-end` around compiling and linking both passes,
//! `lander5-first-frame` after the half-resolution first frame,
//! `lander5-full-frame` after the first full-resolution frame, and
//! `lander5-presented` on the animation frame after it. A worker posts
//! `"presented"` to the page instead, and [`frame_times`] returns the times
//! of its recent frames.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
#[cfg(feature = "worker")]
use web_sys::{DedicatedWorkerGlobalScope, OffscreenCanvas};
use web_sys::{
    HtmlCanvasElement, Performance, WebGl2RenderingContext as Gl, WebGlProgram,
    WebGlUniformLocation, Window,
};

use crate::shaders::{CANVAS_ID, GRID_ID, SKY_ID, SKY_TABLE_SIZE, VERTEX};

/// Seconds into the scene at the first frame, and the still frame's time.
const START_TIME: f64 = 31.0;
/// The most device pixels per CSS pixel the canvas renders.
const MAX_DPR: f64 = 1.5;
/// The first frame's resolution scale: it shows while the full one renders.
const FIRST_SCALE: f64 = 0.5;
/// The lowest scale dynamic resolution goes to.
const MIN_SCALE: f64 = 0.5;
/// How many recent frame times [`frame_times`] keeps.
#[cfg(feature = "worker")]
const FRAME_LOG: usize = 1024;

thread_local! {
    /// Set by [`pause`]: the page's loop stops at its next frame.
    static PAUSED: Cell<bool> = const { Cell::new(false) };
    /// The scene time of the last frame drawn.
    static SCENE: Cell<f64> = const { Cell::new(START_TIME) };
    /// A worker's view: CSS width and height and device pixel ratio.
    static VIEW: Cell<(f64, f64, f64)> = const { Cell::new((1.0, 1.0, 1.0)) };
    /// Recent frame times (`timeOrigin + now`, ms), as a ring.
    static FRAMES: RefCell<(Vec<f64>, usize)> = const { RefCell::new((Vec::new(), 0)) };
}

#[wasm_bindgen(start)]
pub fn start() {
    let _ = run();
}

/// Stops the page's loop at its next frame (a worker has taken over).
#[cfg(feature = "worker")]
#[wasm_bindgen]
pub fn pause() {
    PAUSED.with(|paused| paused.set(true));
}

/// The scene time of the last frame drawn, in seconds.
#[cfg(feature = "worker")]
#[wasm_bindgen]
pub fn scene_time() -> f64 {
    SCENE.with(Cell::get)
}

/// The times of the recent frames (`timeOrigin + now`, ms), oldest first.
#[cfg(feature = "worker")]
#[wasm_bindgen]
pub fn frame_times() -> Vec<f64> {
    FRAMES.with(|frames| {
        let (log, next) = &*frames.borrow();
        let (newer, older) = log.split_at(*next % log.len().max(1));
        older.iter().chain(newer).copied().collect()
    })
}

/// A worker's new view size, when the page's window changes.
#[cfg(feature = "worker")]
#[wasm_bindgen]
pub fn resize_worker(width: f64, height: f64, dpr: f64) {
    VIEW.with(|view| view.set((width, height, dpr)));
}

/// Runs the Grid in this dedicated worker on `canvas`, from scene `time`.
#[cfg(feature = "worker")]
#[wasm_bindgen]
pub fn start_worker(
    canvas: OffscreenCanvas,
    sky: &str,
    grid: &str,
    width: f64,
    height: f64,
    dpr: f64,
    time: f64,
) {
    VIEW.with(|view| view.set((width, height, dpr)));
    let _ = run_worker(canvas, sky, grid, time);
}

/// Where the runner draws: a page's window or a dedicated worker.
#[derive(Clone)]
enum Host {
    Page(Window),
    #[cfg(feature = "worker")]
    Worker(DedicatedWorkerGlobalScope),
}

impl Host {
    /// CSS width, height, and device pixel ratio.
    fn view(&self) -> Option<(f64, f64, f64)> {
        match self {
            Host::Page(window) => Some((
                window.inner_width().ok()?.as_f64()?,
                window.inner_height().ok()?.as_f64()?,
                window.device_pixel_ratio(),
            )),
            #[cfg(feature = "worker")]
            Host::Worker(_) => Some(VIEW.with(Cell::get)),
        }
    }

    fn request_frame(&self, callback: &Closure<dyn FnMut(f64)>) {
        let callback = callback.as_ref().unchecked_ref();
        let _ = match self {
            Host::Page(window) => window.request_animation_frame(callback),
            #[cfg(feature = "worker")]
            Host::Worker(scope) => scope.request_animation_frame(callback),
        };
    }
}

enum Surface {
    Html(HtmlCanvasElement),
    #[cfg(feature = "worker")]
    Offscreen(OffscreenCanvas),
}

impl Surface {
    fn size(&self) -> (u32, u32) {
        match self {
            Surface::Html(canvas) => (canvas.width(), canvas.height()),
            #[cfg(feature = "worker")]
            Surface::Offscreen(canvas) => (canvas.width(), canvas.height()),
        }
    }

    fn set_size(&self, width: u32, height: u32) {
        match self {
            Surface::Html(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
            #[cfg(feature = "worker")]
            Surface::Offscreen(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }
    }
}

struct Stage {
    host: Host,
    /// Marks go on a page's timeline only.
    perf: Option<Performance>,
    /// The host's `performance.timeOrigin`, for [`frame_times`].
    #[cfg_attr(not(feature = "worker"), allow(dead_code))]
    time_origin: f64,
    canvas: Surface,
    gl: Gl,
    resolution: Option<WebGlUniformLocation>,
    time: Option<WebGlUniformLocation>,
    frame: Option<WebGlUniformLocation>,
    frames: i32,
    scale: f64,
    start_time: f64,
    first_now: f64,
    last_now: f64,
    best: f64,
    average: f64,
    slow: u32,
    fast: u32,
}

impl Stage {
    fn mark(&self, name: &str) {
        if let Some(perf) = &self.perf {
            let _ = perf.mark(name);
        }
    }

    /// Draws one frame of the scene at `time`, at `scale` of the canvas's
    /// capped device resolution.
    fn draw(&mut self, time: f64, scale: f64) -> Option<()> {
        let (css_width, css_height, dpr) = self.host.view()?;
        let dpr = dpr.min(MAX_DPR) * scale;
        let width = (css_width * dpr).round().max(1.0) as u32;
        let height = (css_height * dpr).round().max(1.0) as u32;
        if self.canvas.size() != (width, height) {
            self.canvas.set_size(width, height);
        }
        let gl = &self.gl;
        gl.viewport(0, 0, width as i32, height as i32);
        gl.uniform3f(self.resolution.as_ref(), width as f32, height as f32, 1.0);
        gl.uniform1f(self.time.as_ref(), time as f32);
        gl.uniform1i(self.frame.as_ref(), self.frames);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        self.frames += 1;
        SCENE.with(|scene| scene.set(time));
        Some(())
    }

    /// One animation frame: the first full-resolution frame, its
    /// presentation, and from then on dynamic resolution. The scale drops
    /// when frames run consistently slower than the fastest interval seen
    /// (the display's refresh), and climbs back after a long run at it.
    fn tick(&mut self, now: f64) -> Option<()> {
        #[cfg(feature = "worker")]
        {
            if matches!(self.host, Host::Page(_)) && PAUSED.with(Cell::get) {
                return None;
            }
            FRAMES.with(|frames| {
                let (log, next) = &mut *frames.borrow_mut();
                if log.len() < FRAME_LOG {
                    log.push(self.time_origin + now);
                } else {
                    log[*next % FRAME_LOG] = self.time_origin + now;
                }
                *next = (*next + 1) % FRAME_LOG;
            });
        }
        if self.frames == 1 {
            self.first_now = now;
            self.last_now = now;
            self.draw(self.start_time, self.scale)?;
            self.mark("lander5-full-frame");
            return Some(());
        }
        let interval = now - self.last_now;
        self.last_now = now;
        if self.frames == 2 {
            self.mark("lander5-presented");
            #[cfg(feature = "worker")]
            if let Host::Worker(scope) = &self.host {
                let _ = scope.post_message(&"presented".into());
            }
        } else if interval < 250.0 {
            self.best = self.best.min(interval.max(4.0));
            self.average = 0.9 * self.average + 0.1 * interval.min(100.0);
            self.slow = if self.average > 1.35 * self.best {
                self.slow + 1
            } else {
                0
            };
            self.fast = if self.average < 1.1 * self.best {
                self.fast + 1
            } else {
                0
            };
            if self.slow > 20 && self.scale > MIN_SCALE {
                self.scale = (self.scale * 0.85).max(MIN_SCALE);
                self.slow = 0;
                self.average = self.best;
            } else if self.fast > 180 && self.scale < 1.0 {
                self.scale = (self.scale / 0.9).min(1.0);
                self.fast = 0;
            }
        }
        self.draw(
            self.start_time + (now - self.first_now) / 1000.0,
            self.scale,
        )
    }
}

fn shader(gl: &Gl, kind: u32, source: &str) -> Option<web_sys::WebGlShader> {
    let shader = gl.create_shader(kind)?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    Some(shader)
}

/// Compiles and links a pass. Reading the link status waits for both.
fn program(gl: &Gl, fragment: &str) -> Option<WebGlProgram> {
    let program = gl.create_program()?;
    gl.attach_shader(&program, &shader(gl, Gl::VERTEX_SHADER, VERTEX)?);
    gl.attach_shader(&program, &shader(gl, Gl::FRAGMENT_SHADER, fragment)?);
    gl.link_program(&program);
    gl.get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .filter(|linked| *linked)
        .map(|_| program)
}

/// Compiles both passes on `gl`, fills the sky table, and leaves the grid
/// pass in use. Returns the grid pass.
fn prepare(
    gl: &Gl,
    sky_source: &str,
    grid_source: &str,
    perf: Option<&Performance>,
) -> Option<WebGlProgram> {
    if let Some(perf) = perf {
        let _ = perf.mark("lander5-compile-start");
    }
    let sky = program(gl, sky_source)?;
    let grid = program(gl, grid_source)?;
    if let Some(perf) = perf {
        let _ = perf.mark("lander5-compile-end");
    }

    // The sky table: drawn once into an RGBA8 texture the grid pass reads.
    let (table_width, table_height) = SKY_TABLE_SIZE;
    let table = gl.create_texture()?;
    gl.bind_texture(Gl::TEXTURE_2D, Some(&table));
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA8 as i32,
        table_width,
        table_height,
        0,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        None,
    )
    .ok()?;
    for (name, value) in [
        (Gl::TEXTURE_MIN_FILTER, Gl::LINEAR),
        (Gl::TEXTURE_MAG_FILTER, Gl::LINEAR),
        (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE),
        (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameteri(Gl::TEXTURE_2D, name, value as i32);
    }
    let target = gl.create_framebuffer()?;
    gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&target));
    gl.framebuffer_texture_2d(
        Gl::FRAMEBUFFER,
        Gl::COLOR_ATTACHMENT0,
        Gl::TEXTURE_2D,
        Some(&table),
        0,
    );
    gl.viewport(0, 0, table_width, table_height);
    gl.use_program(Some(&sky));
    gl.uniform3f(
        gl.get_uniform_location(&sky, "iResolution").as_ref(),
        table_width as f32,
        table_height as f32,
        1.0,
    );
    gl.draw_arrays(Gl::TRIANGLES, 0, 3);
    gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
    gl.delete_framebuffer(Some(&target));
    gl.delete_program(Some(&sky));

    gl.use_program(Some(&grid));
    gl.uniform1i(gl.get_uniform_location(&grid, "iChannel0").as_ref(), 0);
    Some(grid)
}

/// The context options: no antialiasing, alpha, depth, or stencil.
fn context_options() -> Option<js_sys::Object> {
    let options = js_sys::Object::new();
    for key in ["antialias", "alpha", "depth", "stencil"] {
        js_sys::Reflect::set(&options, &key.into(), &false.into()).ok()?;
    }
    Some(options)
}

fn stage(
    host: Host,
    perf: Option<Performance>,
    time_origin: f64,
    canvas: Surface,
    gl: Gl,
    grid: &WebGlProgram,
    start_time: f64,
) -> Stage {
    Stage {
        resolution: gl.get_uniform_location(grid, "iResolution"),
        time: gl.get_uniform_location(grid, "iTime"),
        frame: gl.get_uniform_location(grid, "iFrame"),
        host,
        perf,
        time_origin,
        canvas,
        gl,
        frames: 0,
        scale: 1.0,
        start_time,
        first_now: 0.0,
        last_now: 0.0,
        best: f64::INFINITY,
        average: 16.7,
        slow: 0,
        fast: 0,
    }
}

/// Draws `stage`'s first frame at `scale`, then a frame on every animation
/// frame until the page pauses it.
fn animate(mut stage: Stage, scale: f64) -> Option<()> {
    let host = stage.host.clone();
    stage.draw(stage.start_time, scale)?;
    stage.mark("lander5-first-frame");
    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let next = callback.clone();
    *callback.borrow_mut() = Some(Closure::new(move |now: f64| {
        if stage.tick(now).is_some()
            && let Some(again) = next.borrow().as_ref()
        {
            stage.host.request_frame(again);
        }
    }));
    host.request_frame(callback.borrow().as_ref()?);
    Some(())
}

#[cfg(feature = "worker")]
fn run_worker(canvas: OffscreenCanvas, sky: &str, grid: &str, time: f64) -> Option<()> {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().dyn_into().ok()?;
    let time_origin = scope.performance().map_or(0.0, |perf| perf.time_origin());
    let gl: Gl = canvas
        .get_context_with_context_options("webgl2", &context_options()?.into())
        .ok()??
        .dyn_into()
        .ok()?;
    let program = prepare(&gl, sky, grid, None)?;
    let stage = stage(
        Host::Worker(scope),
        None,
        time_origin,
        Surface::Offscreen(canvas),
        gl,
        &program,
        time,
    );
    animate(stage, 1.0)
}

fn run() -> Option<()> {
    let window = web_sys::window()?;
    let perf = window.performance()?;
    let document = window.document()?;
    let canvas: HtmlCanvasElement = document.get_element_by_id(CANVAS_ID)?.dyn_into().ok()?;
    let sky_source = document.get_element_by_id(SKY_ID)?.text_content()?;
    let grid_source = document.get_element_by_id(GRID_ID)?.text_content()?;
    let gl: Gl = canvas
        .get_context_with_context_options("webgl2", &context_options()?.into())
        .ok()??
        .dyn_into()
        .ok()?;
    let grid = prepare(&gl, &sky_source, &grid_source, Some(&perf))?;
    let still = window
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let time_origin = perf.time_origin();
    let mut stage = stage(
        Host::Page(window.clone()),
        Some(perf),
        time_origin,
        Surface::Html(canvas),
        gl,
        &grid,
        START_TIME,
    );

    if still {
        // One full frame, drawn again when the window changes size.
        stage.draw(START_TIME, 1.0)?;
        stage.mark("lander5-full-frame");
        let presented = stage.perf.clone();
        let mark = Closure::once_into_js(move || {
            if let Some(perf) = presented {
                let _ = perf.mark("lander5-presented");
            }
        });
        let _ = window.request_animation_frame(mark.unchecked_ref());
        let stage = RefCell::new(stage);
        let redraw = Closure::<dyn FnMut()>::new(move || {
            let _ = stage.borrow_mut().draw(START_TIME, 1.0);
        });
        let _ = window.add_event_listener_with_callback("resize", redraw.as_ref().unchecked_ref());
        redraw.forget();
        return Some(());
    }
    animate(stage, FIRST_SCALE)
}
