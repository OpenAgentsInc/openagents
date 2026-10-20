//! The runner: raw WebGL2 through web-sys, no engine.
//!
//! It starts when the module is instantiated. With no WebGL2, or when a
//! shader fails, it returns and leaves the page's static background. Every
//! step returns `Option` rather than panicking, which also keeps the panic
//! machinery out of the module.
//!
//! Performance marks for measuring: `lander5-compile-start` and
//! `lander5-compile-end` around compiling and linking both passes,
//! `lander5-first-frame` after the half-resolution first frame,
//! `lander5-full-frame` after the first full-resolution frame, and
//! `lander5-presented` on the animation frame after it.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
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

#[wasm_bindgen(start)]
pub fn start() {
    let _ = run();
}

struct Stage {
    window: Window,
    perf: Performance,
    canvas: HtmlCanvasElement,
    gl: Gl,
    resolution: Option<WebGlUniformLocation>,
    time: Option<WebGlUniformLocation>,
    frame: Option<WebGlUniformLocation>,
    frames: i32,
    scale: f64,
    first_now: f64,
    last_now: f64,
    best: f64,
    average: f64,
    slow: u32,
    fast: u32,
}

impl Stage {
    /// Draws one frame of the scene at `time`, at `scale` of the canvas's
    /// capped device resolution.
    fn draw(&mut self, time: f64, scale: f64) -> Option<()> {
        let dpr = self.window.device_pixel_ratio().min(MAX_DPR) * scale;
        let css_width = self.window.inner_width().ok()?.as_f64()?;
        let css_height = self.window.inner_height().ok()?.as_f64()?;
        let width = (css_width * dpr).round().max(1.0) as u32;
        let height = (css_height * dpr).round().max(1.0) as u32;
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        let gl = &self.gl;
        gl.viewport(0, 0, width as i32, height as i32);
        gl.uniform3f(self.resolution.as_ref(), width as f32, height as f32, 1.0);
        gl.uniform1f(self.time.as_ref(), time as f32);
        gl.uniform1i(self.frame.as_ref(), self.frames);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        self.frames += 1;
        Some(())
    }

    /// One animation frame: the first full-resolution frame, its
    /// presentation, and from then on dynamic resolution. The scale drops
    /// when frames run consistently slower than the fastest interval seen
    /// (the display's refresh), and climbs back after a long run at it.
    fn tick(&mut self, now: f64) -> Option<()> {
        if self.frames == 1 {
            self.first_now = now;
            self.last_now = now;
            self.draw(START_TIME, self.scale)?;
            let _ = self.perf.mark("lander5-full-frame");
            return Some(());
        }
        let interval = now - self.last_now;
        self.last_now = now;
        if self.frames == 2 {
            let _ = self.perf.mark("lander5-presented");
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
        self.draw(START_TIME + (now - self.first_now) / 1000.0, self.scale)
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

fn request_frame(window: &Window, callback: &Closure<dyn FnMut(f64)>) {
    let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
}

fn run() -> Option<()> {
    let window = web_sys::window()?;
    let perf = window.performance()?;
    let document = window.document()?;
    let canvas: HtmlCanvasElement = document.get_element_by_id(CANVAS_ID)?.dyn_into().ok()?;
    let sky_source = document.get_element_by_id(SKY_ID)?.text_content()?;
    let grid_source = document.get_element_by_id(GRID_ID)?.text_content()?;

    let options = js_sys::Object::new();
    for key in ["antialias", "alpha", "depth", "stencil"] {
        js_sys::Reflect::set(&options, &key.into(), &false.into()).ok()?;
    }
    let gl: Gl = canvas
        .get_context_with_context_options("webgl2", &options)
        .ok()??
        .dyn_into()
        .ok()?;

    let _ = perf.mark("lander5-compile-start");
    let sky = program(&gl, &sky_source)?;
    let grid = program(&gl, &grid_source)?;
    let _ = perf.mark("lander5-compile-end");

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
    let still = window
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let mut stage = Stage {
        resolution: gl.get_uniform_location(&grid, "iResolution"),
        time: gl.get_uniform_location(&grid, "iTime"),
        frame: gl.get_uniform_location(&grid, "iFrame"),
        window: window.clone(),
        perf,
        canvas,
        gl,
        frames: 0,
        scale: 1.0,
        first_now: 0.0,
        last_now: 0.0,
        best: f64::INFINITY,
        average: 16.7,
        slow: 0,
        fast: 0,
    };

    if still {
        // One full frame, drawn again when the window changes size.
        stage.draw(START_TIME, 1.0)?;
        let _ = stage.perf.mark("lander5-full-frame");
        let presented = stage.perf.clone();
        let mark = Closure::once_into_js(move || {
            let _ = presented.mark("lander5-presented");
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

    stage.draw(START_TIME, FIRST_SCALE)?;
    let _ = stage.perf.mark("lander5-first-frame");
    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let next = callback.clone();
    *callback.borrow_mut() = Some(Closure::new(move |now: f64| {
        if stage.tick(now).is_some()
            && let Some(again) = next.borrow().as_ref()
        {
            request_frame(&stage.window, again);
        }
    }));
    request_frame(&window, callback.borrow().as_ref()?);
    Some(())
}
