//! The browser side: WebGL2 drawing, input, the HUD, and storage.

use std::cell::RefCell;
use std::rc::Rc;

use bunny_rules::game::Move;
use bunny_rules::{
    EdibleKind, Event, FarmerState, Game, HZ, Input, ObstacleKind, Status, TIER_HEIGHT, TIER_JUMP,
    TIER_NAMES, UNIT, level, shade,
};
use glam::{Mat4, Quat, Vec2, Vec3};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent,
    WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlUniformLocation,
    WebGlVertexArrayObject, Window,
};

use crate::look::{Options, Tier};
use crate::mesh::{Mesh, STRIDE, rgb};
use crate::{copy, scene};

const VERTEX: &str = r"#version 300 es
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_hull;
layout(location = 3) in vec3 a_colour;
layout(location = 4) in float a_weight;
uniform mat4 u_view_projection;
uniform mat4 u_model;
uniform float u_outline;
uniform vec2 u_viewport;
out vec3 v_normal;
out vec3 v_colour;
void main() {
  vec4 clip = u_view_projection * u_model * vec4(a_position, 1.0);
  if (u_outline > 0.0) {
    if (a_weight <= 0.0) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
    vec4 push = u_view_projection * vec4(mat3(u_model) * a_hull, 0.0);
    vec2 dir = push.xy;
    float len = length(dir);
    if (len > 1e-6) { dir /= len; }
    float near = clamp(8.0 / clip.w, 0.35, 1.0);
    clip.xy += dir * u_outline * a_weight * near * 2.0 / u_viewport * clip.w;
  }
  v_normal = mat3(u_model) * a_normal;
  v_colour = a_colour;
  gl_Position = clip;
}
";

const FRAGMENT: &str = r"#version 300 es
precision mediump float;
in vec3 v_normal;
in vec3 v_colour;
uniform vec3 u_tint;
uniform float u_flat;
uniform float u_id;
layout(location = 0) out vec4 o_colour;
layout(location = 1) out vec4 o_normal;
void main() {
  if (u_flat > 0.5) {
    o_colour = vec4(u_tint, 1.0);
    o_normal = vec4(0.5, 0.5, 0.5, 1.0);
    return;
  }
  vec3 n = normalize(v_normal);
  float light = dot(n, normalize(vec3(0.45, 1.0, 0.3)));
  float band = light > 0.2 ? 1.0 : 0.88;
  o_colour = vec4(v_colour * u_tint * band, 1.0);
  o_normal = vec4(n * 0.5 + 0.5, u_id);
}
";

/// What a draw is, for the line pass: the ground and things flat on it get
/// no lines of their own, gray things and coloured things get their own ids.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Ground,
    Gray,
    Chroma,
}

/// The line width at weight 1, in CSS pixels.
const LINE: f32 = 2.0;
const STEP: f64 = 1.0 / HZ as f64;
/// The longest frame the game catches up on; a slower one runs slow.
#[cfg(not(feature = "autoplay"))]
const MAX_FRAME: f64 = 0.1;
#[cfg(not(feature = "autoplay"))]
const MAX_STEPS: u32 = 8;
/// Captures in a headless browser draw few frames; catch up on all of it.
#[cfg(feature = "autoplay")]
const MAX_FRAME: f64 = 10.0;
#[cfg(feature = "autoplay")]
const MAX_STEPS: u32 = 1000;
const SAVE_KEY: &str = "bunny.progress.v1";
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
/// The camera's near and far planes, in metres.
const NEAR: f32 = 0.1;
const FAR: f32 = 220.0;
/// The bunny is drawn a little larger than its body height, ears and all,
/// so a Kit reads on a phone.
const DRAWN: f32 = 1.25;

struct GpuMesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    count: i32,
}

struct Gpu {
    gl: Gl,
    program: WebGlProgram,
    u_id: Option<WebGlUniformLocation>,
    look: std::cell::Cell<Look>,
    next_id: std::cell::Cell<u32>,
    /// Whether the screen-space line pass draws the gray things' lines.
    lined: std::cell::Cell<bool>,
    u_view_projection: Option<WebGlUniformLocation>,
    u_model: Option<WebGlUniformLocation>,
    u_outline: Option<WebGlUniformLocation>,
    u_viewport: Option<WebGlUniformLocation>,
    u_tint: Option<WebGlUniformLocation>,
    u_flat: Option<WebGlUniformLocation>,
    /// Device pixels per CSS pixel.
    scale: f32,
}

impl Gpu {
    fn new(gl: Gl) -> Result<Self, String> {
        let program = link(&gl)?;
        gl.use_program(Some(&program));
        let at = |name: &str| gl.get_uniform_location(&program, name);
        let gpu = Self {
            program: program.clone(),
            u_id: at("u_id"),
            look: std::cell::Cell::new(Look::Gray),
            next_id: std::cell::Cell::new(0),
            lined: std::cell::Cell::new(false),
            u_view_projection: at("u_view_projection"),
            u_model: at("u_model"),
            u_outline: at("u_outline"),
            u_viewport: at("u_viewport"),
            u_tint: at("u_tint"),
            u_flat: at("u_flat"),
            gl,
            scale: 1.0,
        };
        gpu.gl.enable(Gl::DEPTH_TEST);
        gpu.gl.enable(Gl::CULL_FACE);
        Ok(gpu)
    }

    fn upload(&self, mesh: &Mesh) -> GpuMesh {
        let gl = &self.gl;
        let vao = gl.create_vertex_array().expect("a vertex array");
        let buffer = gl.create_buffer().expect("a buffer");
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        let stride = (STRIDE * 4) as i32;
        for (index, (size, offset)) in [(3, 0), (3, 3), (3, 6), (3, 9), (1, 12)].iter().enumerate()
        {
            gl.enable_vertex_attrib_array(index as u32);
            gl.vertex_attrib_pointer_with_i32(
                index as u32,
                *size,
                Gl::FLOAT,
                false,
                stride,
                offset * 4,
            );
        }
        let gpu = GpuMesh {
            vao,
            buffer,
            count: 0,
        };
        let mut gpu = gpu;
        self.refill(&mut gpu, mesh);
        gl.bind_vertex_array(None);
        gpu
    }

    fn refill(&self, gpu: &mut GpuMesh, mesh: &Mesh) {
        let gl = &self.gl;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&gpu.buffer));
        let array = js_sys::Float32Array::from(mesh.data.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::STATIC_DRAW);
        gpu.count = mesh.vertices() as i32;
    }

    fn camera(&self, view_projection: &Mat4, width: f32, height: f32) {
        self.gl.uniform_matrix4fv_with_f32_array(
            self.u_view_projection.as_ref(),
            false,
            &view_projection.to_cols_array(),
        );
        self.gl.uniform2f(self.u_viewport.as_ref(), width, height);
    }

    /// Draws `mesh` filled in its colours times `tint`, with an outline in
    /// `line` colour `width` CSS pixels wide when given.
    fn draw(&self, mesh: &GpuMesh, model: &Mat4, tint: [f32; 3], line: Option<([f32; 3], f32)>) {
        if mesh.count == 0 {
            return;
        }
        let gl = &self.gl;
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.uniform_matrix4fv_with_f32_array(self.u_model.as_ref(), false, &model.to_cols_array());
        let look = self.look.get();
        let n = self.next_id.get().wrapping_add(1);
        self.next_id.set(n);
        let id = match look {
            Look::Ground => 0,
            Look::Gray => 1 + n % 126,
            Look::Chroma => 128 + n % 126,
        };
        gl.uniform1f(self.u_id.as_ref(), id as f32 / 255.0);
        // The line pass draws gray things' ordinary lines; only heavier
        // ones (the net in a wind-up) and coloured things keep their own.
        let line =
            line.filter(|(_, width)| !(self.lined.get() && look == Look::Gray && *width <= LINE));
        if let Some((colour, width)) = line {
            gl.cull_face(Gl::FRONT);
            gl.uniform1f(self.u_outline.as_ref(), width * self.scale);
            gl.uniform1f(self.u_flat.as_ref(), 1.0);
            gl.uniform3f(self.u_tint.as_ref(), colour[0], colour[1], colour[2]);
            gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
        }
        gl.cull_face(Gl::BACK);
        gl.uniform1f(self.u_outline.as_ref(), 0.0);
        gl.uniform1f(self.u_flat.as_ref(), 0.0);
        gl.uniform3f(self.u_tint.as_ref(), tint[0], tint[1], tint[2]);
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
    }
}

fn compile(gl: &Gl, kind: u32, source: &str) -> Result<web_sys::WebGlShader, String> {
    let shader = gl.create_shader(kind).ok_or("no shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default())
    }
}

fn link(gl: &Gl) -> Result<WebGlProgram, String> {
    program(gl, VERTEX, FRAGMENT)
}

/// Compiles and links a program.
pub(crate) fn program(gl: &Gl, vertex: &str, fragment: &str) -> Result<WebGlProgram, String> {
    let vertex = compile(gl, Gl::VERTEX_SHADER, vertex)?;
    let fragment = compile(gl, Gl::FRAGMENT_SHADER, fragment)?;
    let program = gl.create_program().ok_or("no program")?;
    gl.attach_shader(&program, &vertex);
    gl.attach_shader(&program, &fragment);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default())
    }
}

struct Meshes {
    ground: GpuMesh,
    hedges: GpuMesh,
    edibles: Vec<(EdibleKind, GpuMesh)>,
    bunny: GpuMesh,
    ear: GpuMesh,
    farmer: GpuMesh,
    leg: GpuMesh,
    net: GpuMesh,
    alarm: GpuMesh,
    shadow: GpuMesh,
    crumb: GpuMesh,
    dot: GpuMesh,
    pot: GpuMesh,
    gnome: GpuMesh,
    fence: GpuMesh,
    gap: GpuMesh,
    barrow: GpuMesh,
}

impl Meshes {
    fn obstacle(&self, kind: ObstacleKind) -> &GpuMesh {
        match kind {
            ObstacleKind::Gnome => &self.gnome,
            ObstacleKind::Fence => &self.fence,
            ObstacleKind::Gap | ObstacleKind::Tunnel | ObstacleKind::Wire => &self.gap,
            ObstacleKind::Barrow | ObstacleKind::Scarecrow => &self.barrow,
            _ => &self.pot,
        }
    }

    fn edible(&self, kind: EdibleKind) -> &GpuMesh {
        self.edibles
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(&self.edibles[0].1, |(_, mesh)| mesh)
    }
}

struct Particle {
    at: Vec3,
    velocity: Vec3,
    life: f32,
    span: f32,
    colour: [f32; 3],
    size: f32,
    spin: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Title,
    Playing,
    Over,
}

struct Hud {
    root: HtmlElement,
    stats: HtmlElement,
    carrots: HtmlElement,
    size: HtmlElement,
    pips: Vec<HtmlElement>,
    restart: HtmlElement,
    card: HtmlElement,
    heading: HtmlElement,
    line: HtmlElement,
    hint: HtmlElement,
    swatch: HtmlElement,
    action: HtmlElement,
    map_label: HtmlElement,
}

struct App {
    window: Window,
    canvas: HtmlCanvasElement,
    gpu: Gpu,
    outline: Option<crate::outline::Outline>,
    tier: Tier,
    options: Options,
    meshes: Meshes,
    hud: Hud,
    game: Game,
    phase: Phase,
    wins: u32,
    queue: Vec<Input>,
    last: f64,
    carry: f64,
    time: f32,
    over_at: f32,
    prev_bunny: Vec2,
    prev_farmer: Vec2,
    bunny_slide: Vec2,
    farmer_slide: Vec2,
    bunny_yaw: f32,
    farmer_yaw: f32,
    camera_yaw: f32,
    size: f32,
    camera_size: f32,
    hop: f32,
    stride: f32,
    particles: Vec<Particle>,
    touch: Option<(f32, f32, f64)>,
    shown: (usize, u8, Phase),
}

fn element(document: &Document, tag: &str) -> HtmlElement {
    document
        .create_element(tag)
        .expect("an element")
        .dyn_into()
        .expect("an HTML element")
}

fn css(element: &HtmlElement, pairs: &[(&str, &str)]) {
    let style = element.style();
    for (name, value) in pairs {
        let _ = style.set_property(name, value);
    }
}

fn hex(colour: u32) -> String {
    format!("#{colour:06x}")
}

fn wrap(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

fn approach(value: f32, target: f32, rate: f32, dt: f32) -> f32 {
    value + (target - value) * (1.0 - (-rate * dt).exp())
}

fn approach_angle(value: f32, target: f32, rate: f32, dt: f32) -> f32 {
    value + wrap(target - value) * (1.0 - (-rate * dt).exp())
}

fn point(units: (i32, i32)) -> Vec2 {
    Vec2::new(scene::metres(units.0), scene::metres(units.1))
}

fn load_wins(window: &Window) -> u32 {
    let Ok(Some(storage)) = window.local_storage() else {
        return 0;
    };
    let Ok(Some(saved)) = storage.get_item(SAVE_KEY) else {
        return 0;
    };
    let digits: String = saved
        .split("\"wins\"")
        .nth(1)
        .unwrap_or("")
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap_or(0)
}

fn save_wins(window: &Window, wins: u32) {
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(SAVE_KEY, &format!("{{\"wins\":{wins}}}"));
    }
}

fn build_hud(document: &Document, parent: &web_sys::Element, touch: bool) -> Hud {
    let root = element(document, "div");
    css(
        &root,
        &[
            ("position", "absolute"),
            ("inset", "0"),
            ("pointer-events", "none"),
            (
                "font-family",
                "system-ui, -apple-system, 'Segoe UI', sans-serif",
            ),
            ("color", "#1e1e1e"),
            ("user-select", "none"),
            ("-webkit-user-select", "none"),
        ],
    );
    let panel = [
        ("position", "absolute"),
        ("background", "rgba(244, 244, 242, 0.9)"),
        ("border", "2px solid #1e1e1e"),
        ("border-radius", "12px"),
        ("padding", "8px 12px"),
    ];
    let stats = element(document, "div");
    css(&stats, &panel);
    css(
        &stats,
        &[
            ("top", "calc(12px + env(safe-area-inset-top))"),
            ("left", "calc(12px + env(safe-area-inset-left))"),
            ("min-width", "104px"),
        ],
    );
    let label = element(document, "div");
    label.set_text_content(Some(copy::CARROTS));
    css(
        &label,
        &[
            ("font-size", "12px"),
            ("letter-spacing", "0.04em"),
            ("text-transform", "uppercase"),
        ],
    );
    let carrots = element(document, "div");
    css(
        &carrots,
        &[
            ("font-size", "30px"),
            ("font-weight", "700"),
            ("line-height", "1.1"),
            ("color", "#d86f00"),
            ("font-variant-numeric", "tabular-nums"),
        ],
    );
    let size = element(document, "div");
    css(
        &size,
        &[
            ("font-size", "14px"),
            ("font-weight", "600"),
            ("margin-top", "6px"),
        ],
    );
    let row = element(document, "div");
    css(
        &row,
        &[("display", "flex"), ("gap", "4px"), ("margin-top", "4px")],
    );
    let pips: Vec<HtmlElement> = (0..TIER_NAMES.len())
        .map(|_| {
            let pip = element(document, "span");
            css(
                &pip,
                &[
                    ("width", "14px"),
                    ("height", "14px"),
                    ("border", "2px solid #1e1e1e"),
                    ("border-radius", "50%"),
                    ("box-sizing", "border-box"),
                ],
            );
            let _ = row.append_child(&pip);
            pip
        })
        .collect();
    for child in [&label, &carrots, &size, &row] {
        let _ = stats.append_child(child);
    }
    let button = [
        ("font", "inherit"),
        ("font-weight", "650"),
        ("color", "#1e1e1e"),
        ("border", "2px solid #1e1e1e"),
        ("border-radius", "999px"),
        ("cursor", "pointer"),
        ("pointer-events", "auto"),
    ];
    let restart = element(document, "button");
    restart.set_text_content(Some(copy::RESTART));
    css(&restart, &button);
    css(
        &restart,
        &[
            ("position", "absolute"),
            ("top", "calc(12px + env(safe-area-inset-top))"),
            ("right", "calc(12px + env(safe-area-inset-right))"),
            ("padding", "8px 16px"),
            ("font-size", "15px"),
            ("background", "rgba(244, 244, 242, 0.9)"),
        ],
    );
    let card = element(document, "div");
    css(&card, &panel);
    css(
        &card,
        &[
            ("left", "50%"),
            ("top", "50%"),
            ("transform", "translate(-50%, -50%)"),
            ("width", "min(86vw, 400px)"),
            ("box-sizing", "border-box"),
            ("padding", "20px 22px"),
            ("text-align", "center"),
            ("pointer-events", "auto"),
            ("background", "rgba(244, 244, 242, 0.96)"),
        ],
    );
    let heading = element(document, "h1");
    css(
        &heading,
        &[
            ("margin", "0 0 6px"),
            ("font-size", "28px"),
            ("line-height", "1.15"),
        ],
    );
    let swatch = element(document, "div");
    css(
        &swatch,
        &[
            ("width", "34px"),
            ("height", "34px"),
            ("margin", "8px auto"),
            ("border", "3px solid #1e1e1e"),
            ("border-radius", "50%"),
        ],
    );
    let line = element(document, "p");
    css(&line, &[("margin", "0 0 8px"), ("font-size", "16px")]);
    let hint = element(document, "p");
    hint.set_text_content(Some(if touch { copy::SWIPES } else { copy::KEYS }));
    css(
        &hint,
        &[
            ("margin", "0 0 14px"),
            ("font-size", "14px"),
            ("color", "#55554f"),
        ],
    );
    let action = element(document, "button");
    css(&action, &button);
    css(
        &action,
        &[
            ("padding", "10px 30px"),
            ("font-size", "18px"),
            ("background", "#f28a1e"),
        ],
    );
    for child in [&heading, &swatch, &line, &hint, &action] {
        let _ = card.append_child(child);
    }
    let map_label = element(document, "span");
    map_label.set_text_content(Some(copy::MAP));
    css(
        &map_label,
        &[
            ("position", "absolute"),
            ("width", "1px"),
            ("height", "1px"),
            ("overflow", "hidden"),
            ("clip", "rect(0 0 0 0)"),
        ],
    );
    for child in [&stats, &restart, &card, &map_label] {
        let _ = root.append_child(child);
    }
    let _ = parent.append_child(&root);
    Hud {
        root,
        stats,
        carrots,
        size,
        pips,
        restart,
        card,
        heading,
        line,
        hint,
        swatch,
        action,
        map_label,
    }
}

fn show(element: &HtmlElement, visible: bool) {
    css(element, &[("display", if visible { "" } else { "none" })]);
}

impl App {
    fn fur(&self) -> u32 {
        shade::fur(self.wins)
    }

    fn rebuild_bunny(&mut self) {
        let fur = self.fur();
        self.gpu.refill(&mut self.meshes.bunny, &scene::bunny(fur));
        self.gpu.refill(&mut self.meshes.ear, &scene::ear(fur));
    }

    fn new_run(&mut self) {
        self.game = Game::new(level::garden(1));
        #[cfg(feature = "autoplay")]
        if self
            .window
            .location()
            .hash()
            .is_ok_and(|hash| hash == "#quiet")
        {
            self.game.farmer_on = false;
        }
        self.queue.clear();
        self.particles.clear();
        self.carry = 0.0;
        self.prev_bunny = point(self.game.bunny_point());
        self.prev_farmer = point(self.game.farmer_point());
        self.bunny_slide = Vec2::ZERO;
        self.farmer_slide = Vec2::ZERO;
        let (hx, hz) = self.game.bunny.heading(&self.game.garden);
        self.bunny_yaw = scene::yaw_of(hx as f32, hz as f32);
        self.camera_yaw = self.bunny_yaw;
        let (fx, fz) = self.game.farmer.facing;
        self.farmer_yaw = scene::yaw_of(fx as f32, fz as f32);
        self.size = scene::metres(TIER_HEIGHT[0]) * DRAWN;
        self.camera_size = 0.0;
    }

    fn play(&mut self) {
        self.new_run();
        self.phase = Phase::Playing;
        let _ = self.canvas.focus();
    }

    fn press(&mut self, input: Input) {
        if self.phase == Phase::Playing {
            self.queue.push(input);
        }
    }

    fn frame(&mut self, now: f64) {
        let dt = if self.last == 0.0 {
            0.0
        } else {
            ((now - self.last) / 1000.0).min(MAX_FRAME)
        };
        self.last = now;
        let dtf = dt as f32;
        self.time += dtf;
        if self.phase == Phase::Playing {
            self.carry += dt;
            let mut steps = 0;
            while self.carry >= STEP && steps < MAX_STEPS {
                self.carry -= STEP;
                steps += 1;
                self.tick();
                if self.game.status != Status::Playing {
                    self.finish();
                    break;
                }
            }
        }
        let alpha = if self.phase == Phase::Playing {
            (self.carry / STEP) as f32
        } else {
            1.0
        };
        self.animate(dtf);
        self.render(alpha, dtf);
        self.update_hud();
    }

    fn tick(&mut self) {
        let before_bunny = point(self.game.bunny_point());
        let before_farmer = point(self.game.farmer_point());
        #[cfg(feature = "autoplay")]
        if self.queue.is_empty()
            && let Some(input) = bunny_rules::bot::input(&self.game)
        {
            self.queue.push(input);
        }
        let inputs: Vec<Input> = self.queue.drain(..).collect();
        self.game.step(&inputs);
        let after_bunny = point(self.game.bunny_point());
        let after_farmer = point(self.game.farmer_point());
        // A turn at a junction keeps the bunny's lane, which can move it
        // sideways at once; slide it over instead of jumping.
        if (after_bunny - before_bunny).length() > 0.5 {
            self.bunny_slide += before_bunny - after_bunny;
        }
        if (after_farmer - before_farmer).length() > 0.5 {
            self.farmer_slide += before_farmer - after_farmer;
        }
        self.prev_bunny = before_bunny;
        self.prev_farmer = before_farmer;
        let moved = (after_farmer - before_farmer).length();
        self.stride += moved * 3.2;
        let events = self.game.events.clone();
        for event in events {
            self.effect(event);
        }
    }

    fn burst(&mut self, at: Vec3, count: usize, colour: u32, size: f32, speed: f32) {
        let colour = rgb(colour);
        for i in 0..count {
            let seed = (self.particles.len() * 7 + i * 13) as f32 + self.time * 37.0;
            let angle = seed * 2.399;
            let lift = 0.5 + (seed * 0.731).sin().abs();
            self.particles.push(Particle {
                at,
                velocity: Vec3::new(angle.cos() * speed, lift * speed * 1.4, angle.sin() * speed),
                life: 0.7,
                span: 0.7,
                colour,
                size,
                spin: seed,
            });
        }
    }

    fn effect(&mut self, event: Event) {
        let bunny = point(self.game.bunny_point());
        let bunny = Vec3::new(bunny.x, 0.2, bunny.y);
        match event {
            Event::Ate(index, _) => {
                let c = self.game.garden.edibles[index];
                let at = point(self.game.garden.point(
                    c.edge,
                    c.s,
                    i32::from(c.lane) * bunny_rules::LANE_WIDTH,
                ));
                let at = Vec3::new(at.x, 0.3, at.y);
                self.burst(at, 6, scene::CARROT, 0.07, 1.6);
                self.burst(at, 3, scene::LEAF, 0.06, 1.4);
            }
            Event::Smashed(index, _) => {
                let c = self.game.garden.obstacles[index];
                let at = point(self.game.garden.point(
                    c.edge,
                    c.s,
                    i32::from(c.lane) * bunny_rules::LANE_WIDTH,
                ));
                self.burst(Vec3::new(at.x, 0.4, at.y), 12, 0x9A9A96, 0.12, 2.6);
            }
            Event::Tumbled => self.burst(bunny, 8, 0xC9C9C4, 0.1, 1.8),
            Event::Grew(_) => self.burst(bunny, 10, self.fur(), 0.08, 2.0),
            Event::Escaped => self.burst(bunny, 12, 0x6E6E6A, 0.08, 3.0),
            _ => {}
        }
    }

    fn finish(&mut self) {
        self.phase = Phase::Over;
        self.over_at = self.time;
        if self.game.status == Status::Won {
            self.wins += 1;
            save_wins(&self.window, self.wins);
            self.rebuild_bunny();
            let fur = self.fur();
            let at = point(self.game.bunny_point());
            self.burst(Vec3::new(at.x, 0.3, at.y), 24, fur, 0.1, 3.0);
            self.burst(Vec3::new(at.x, 0.3, at.y), 12, scene::CARROT, 0.08, 2.6);
        }
    }

    fn animate(&mut self, dt: f32) {
        let decay = (-14.0 * dt).exp();
        self.bunny_slide *= decay;
        self.farmer_slide *= decay;
        let b = &self.game.bunny;
        let (hx, hz) = b.heading(&self.game.garden);
        self.bunny_yaw = approach_angle(
            self.bunny_yaw,
            scene::yaw_of(hx as f32, hz as f32),
            16.0,
            dt,
        );
        let (fx, fz) = self.game.farmer.facing;
        self.farmer_yaw = approach_angle(
            self.farmer_yaw,
            scene::yaw_of(fx as f32, fz as f32),
            12.0,
            dt,
        );
        self.camera_yaw = approach_angle(
            self.camera_yaw,
            scene::yaw_of(hx as f32, hz as f32),
            9.0,
            dt,
        );
        self.size = approach(
            self.size,
            scene::metres(TIER_HEIGHT[usize::from(b.tier)]) * DRAWN,
            8.0,
            dt,
        );
        self.camera_size = approach(self.camera_size, f32::from(b.tier) / 4.0, 3.0, dt);
        if self.phase == Phase::Playing && b.mv == Move::Run {
            self.hop += dt * (9.0 + f32::from(b.tier));
        }
        for p in &mut self.particles {
            p.velocity.y -= 9.0 * dt;
            p.at += p.velocity * dt;
            if p.at.y < 0.0 {
                p.at.y = 0.0;
                p.velocity *= 0.4;
            }
            p.life -= dt;
            p.spin += dt * 6.0;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    fn resize(&mut self) -> (f32, f32) {
        let ratio = self
            .tier
            .pixel_ratio(self.window.device_pixel_ratio() as f32);
        let width = (self.canvas.client_width().max(1) as f32 * ratio).round() as u32;
        let height = (self.canvas.client_height().max(1) as f32 * ratio).round() as u32;
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        self.gpu.scale = ratio;
        (width as f32, height as f32)
    }

    fn bunny_at(&self, alpha: f32) -> Vec2 {
        self.prev_bunny.lerp(point(self.game.bunny_point()), alpha) + self.bunny_slide
    }

    fn farmer_at(&self, alpha: f32) -> Vec2 {
        self.prev_farmer
            .lerp(point(self.game.farmer_point()), alpha)
            + self.farmer_slide
    }

    fn render(&mut self, alpha: f32, _dt: f32) {
        let (width, height) = self.resize();
        let sky = rgb(scene::SKY);
        let gl = self.gpu.gl.clone();
        gl.disable(Gl::SCISSOR_TEST);
        gl.use_program(Some(&self.gpu.program));
        let scale = self.tier.target_scale();
        let target = (
            ((width * scale).round() as i32).max(1),
            ((height * scale).round() as i32).max(1),
        );
        let offscreen = match self.outline.as_mut() {
            Some(outline) => match outline.begin(&gl, target.0, target.1, sky) {
                Ok(()) => true,
                Err(error) => {
                    web_sys::console::error_1(&error.into());
                    self.outline = None;
                    false
                }
            },
            None => false,
        };
        if !offscreen {
            gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
            gl.viewport(0, 0, width as i32, height as i32);
            gl.clear_color(sky[0], sky[1], sky[2], 1.0);
            gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        }
        self.gpu.lined.set(offscreen);
        let aspect = width / height;
        let bunny = self.bunny_at(alpha);
        let farmer = self.farmer_at(alpha);
        let view_projection = self.camera(bunny, farmer, aspect);
        let (vw, vh) = if offscreen {
            (target.0 as f32, target.1 as f32)
        } else {
            (width, height)
        };
        self.gpu.camera(&view_projection, vw, vh);
        self.gpu.next_id.set(0);
        self.draw_world(alpha, bunny, farmer);
        if let (true, Some(outline)) = (offscreen, self.outline.as_ref()) {
            outline.finish(
                &gl,
                (width as i32, height as i32),
                NEAR,
                FAR,
                self.tier,
                self.options.high_contrast,
                rgb(scene::INK),
                rgb(scene::FAR_INK),
            );
            gl.use_program(Some(&self.gpu.program));
            gl.clear(Gl::DEPTH_BUFFER_BIT);
        }
        self.gpu.lined.set(false);
        self.gpu.look.set(Look::Gray);
        self.draw_map(width, height, bunny, farmer);
    }

    fn camera(&self, bunny: Vec2, farmer: Vec2, aspect: f32) -> Mat4 {
        let t = self.camera_size;
        let mut back = 3.5 + 2.5 * t;
        let mut up = 1.6 + 1.6 * t;
        if aspect < 1.0 {
            back *= 1.35;
            up *= 1.15;
        }
        let forward = Vec3::new(self.camera_yaw.sin(), 0.0, self.camera_yaw.cos());
        let target_ground = Vec3::new(bunny.x, 0.0, bunny.y);
        let behind = farmer - bunny;
        if behind.length() < 12.0 && behind.dot(Vec2::new(forward.x, forward.z)) < 0.0 {
            up *= 1.1;
        }
        // Keep the eye inside the corridors so it never sits in a hedge.
        let garden = &self.game.garden;
        let mut reach = back;
        while reach > 0.5 {
            let eye = target_ground - forward * reach;
            let units = |m: f32| (m * UNIT as f32) as i32;
            if garden.in_corridor(units(eye.x), units(eye.z)) {
                break;
            }
            reach -= 0.25;
        }
        let eye = target_ground - forward * reach + Vec3::Y * up;
        let ahead = if aspect < 1.0 { 3.0 } else { 4.0 };
        let look = target_ground + forward * ahead + Vec3::Y * (0.4 + 0.3 * t);
        let fov = if aspect < 1.0 {
            (2.0 * ((35.0_f32).to_radians().tan() / aspect).atan()).min(80.0_f32.to_radians())
        } else {
            60.0_f32.to_radians()
        };
        Mat4::perspective_rh_gl(fov, aspect, NEAR, FAR) * Mat4::look_at_rh(eye, look, Vec3::Y)
    }

    fn draw_world(&self, alpha: f32, bunny: Vec2, farmer: Vec2) {
        let gpu = &self.gpu;
        let m = &self.meshes;
        let ink = rgb(scene::INK);
        let line = Some((ink, LINE));
        let game = &self.game;
        let garden = &game.garden;
        gpu.look.set(Look::Ground);
        gpu.draw(&m.ground, &Mat4::IDENTITY, WHITE, None);
        gpu.look.set(Look::Gray);
        gpu.draw(&m.hedges, &Mat4::IDENTITY, WHITE, line);
        let lane = bunny_rules::LANE_WIDTH;
        for (index, c) in garden.edibles.iter().enumerate() {
            if game.eaten[index] {
                continue;
            }
            let at = point(garden.point(c.edge, c.s, i32::from(c.lane) * lane));
            if (at - bunny).length() > 45.0 {
                continue;
            }
            let lift = if c.air { 0.6 } else { 0.0 };
            let bob = lift + 0.04 + 0.04 * (self.time * 3.0 + index as f32).sin();
            let model = scene::place(
                Vec3::new(at.x, bob, at.y),
                self.time * 1.3 + index as f32,
                1.0,
            );
            gpu.look.set(Look::Ground);
            gpu.draw(
                &m.shadow,
                &scene::place(Vec3::new(at.x, 0.0, at.y), 0.0, 0.45),
                WHITE,
                None,
            );
            gpu.look.set(Look::Chroma);
            gpu.draw(m.edible(c.kind), &model, WHITE, Some((ink, LINE * 1.5)));
            gpu.look.set(Look::Gray);
        }
        for (index, cell) in garden.obstacles.iter().enumerate() {
            if !game.alive[index] {
                continue;
            }
            let at = point(garden.point(cell.edge, cell.s, i32::from(cell.lane) * lane));
            let e = &garden.edges[cell.edge];
            let model = scene::place(
                Vec3::new(at.x, 0.0, at.y),
                scene::yaw_of(e.dx as f32, e.dz as f32),
                1.0,
            );
            gpu.draw(m.obstacle(cell.kind), &model, WHITE, line);
        }
        // The bunny.
        let b = &game.bunny;
        let fur = rgb(self.fur());
        let size = self.size;
        let jump = b.jump_progress();
        let hop = if b.air > 0 {
            let peak = scene::metres(TIER_JUMP[usize::from(b.tier)]) + 0.1;
            4.0 * jump * (1.0 - jump) * peak
        } else if self.phase == Phase::Playing && b.mv == Move::Run {
            self.hop.sin().abs() * 0.35 * size
        } else {
            0.0
        };
        let squash: f32 = if b.duck > 0 { 0.55 } else { 1.0 };
        let roll = match b.mv {
            Move::Tumble { left } => {
                let done = 1.0 - left as f32 / bunny_rules::game::TUMBLE as f32;
                (done * 6.0).min(1.0) * std::f32::consts::FRAC_PI_2
            }
            _ => 0.0,
        };
        gpu.look.set(Look::Ground);
        gpu.draw(
            &m.shadow,
            &scene::place(Vec3::new(bunny.x, 0.0, bunny.y), 0.0, size * 1.3),
            WHITE,
            None,
        );
        gpu.look.set(Look::Chroma);
        let body = Mat4::from_translation(Vec3::new(bunny.x, hop, bunny.y))
            * Mat4::from_quat(Quat::from_rotation_y(self.bunny_yaw) * Quat::from_rotation_z(roll))
            * Mat4::from_scale(Vec3::new(
                size * (2.0 - squash).min(1.25),
                size * squash,
                size,
            ));
        let outline = Some(([fur[0] * 0.38, fur[1] * 0.36, fur[2] * 0.36], LINE));
        gpu.draw(&m.bunny, &body, WHITE, outline);
        let flop = if b.mv == Move::Run {
            -0.45
        } else {
            -0.15 + 0.08 * (self.time * 2.0).sin()
        };
        for side in [-1.0_f32, 1.0] {
            let ear = body
                * Mat4::from_translation(Vec3::new(0.1 * side, 0.84, 0.3))
                * Mat4::from_quat(
                    Quat::from_rotation_z(-0.22 * side) * Quat::from_rotation_x(flop),
                );
            gpu.draw(&m.ear, &ear, WHITE, outline);
        }
        gpu.look.set(Look::Gray);
        if game.farmer_on {
            self.draw_farmer(alpha, farmer);
        }
        gpu.look.set(Look::Chroma);
        for p in &self.particles {
            let scale = p.size * (p.life / p.span).max(0.2);
            gpu.draw(
                &m.crumb,
                &(Mat4::from_translation(p.at)
                    * Mat4::from_quat(
                        Quat::from_rotation_y(p.spin) * Quat::from_rotation_x(p.spin * 0.7),
                    )
                    * Mat4::from_scale(Vec3::splat(scale))),
                p.colour,
                None,
            );
        }
    }

    fn draw_farmer(&self, _alpha: f32, at: Vec2) {
        let gpu = &self.gpu;
        let m = &self.meshes;
        let f = &self.game.farmer;
        let ink = rgb(scene::INK);
        let line = Some((ink, LINE));
        let stagger = if f.stagger > 0 {
            (self.time * 18.0).sin() * 0.25
        } else {
            0.0
        };
        let base = scene::place(Vec3::new(at.x, 0.0, at.y), self.farmer_yaw + stagger, 1.0);
        gpu.look.set(Look::Ground);
        gpu.draw(
            &m.shadow,
            &scene::place(Vec3::new(at.x, 0.0, at.y), 0.0, 0.9),
            WHITE,
            None,
        );
        gpu.look.set(Look::Gray);
        gpu.draw(&m.farmer, &base, WHITE, line);
        let swing = self.stride.sin() * 0.55;
        for (side, phase) in [(-1.0_f32, 1.0_f32), (1.0, -1.0)] {
            let leg = base
                * Mat4::from_translation(Vec3::new(0.12 * side, 0.86, 0.0))
                * Mat4::from_rotation_x(swing * phase);
            gpu.draw(&m.leg, &leg, WHITE, line);
        }
        let windup = self.game.windup() as f32;
        let (angle, width) = if f.windup > 0 {
            let progress = 1.0 - f.windup as f32 / windup;
            let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
            (0.35 - 1.5 * eased, LINE * 2.2)
        } else if f.since_swing < 30 {
            let progress = (f.since_swing as f32 / 5.0).min(1.0);
            (-1.15 + 3.05 * progress, LINE * 1.5)
        } else {
            (0.35 + 0.05 * (self.time * 2.0).sin(), LINE)
        };
        let net = base
            * Mat4::from_translation(Vec3::new(0.36, 1.0, 0.28))
            * Mat4::from_rotation_x(angle);
        gpu.draw(&m.net, &net, WHITE, Some((ink, width)));
        let alarmed = f.windup > 0 || matches!(f.state, FarmerState::Chase { .. });
        if alarmed {
            let bob = 0.06 * (self.time * 8.0).sin();
            let mark = scene::place(Vec3::new(at.x, 2.25 + bob, at.y), self.camera_yaw, 1.0);
            gpu.draw(&m.alarm, &mark, WHITE, None);
        }
    }

    fn draw_map(&self, width: f32, height: f32, bunny: Vec2, farmer: Vec2) {
        let gpu = &self.gpu;
        let gl = &gpu.gl;
        let scale = gpu.scale;
        let side = ((width.min(height) / scale) * 0.3).clamp(110.0, 200.0) * scale;
        let margin = 12.0 * scale;
        let (x, y) = (margin, margin);
        let border = (2.0 * scale).round() as i32;
        gl.enable(Gl::SCISSOR_TEST);
        let ink = rgb(scene::INK);
        gl.scissor(
            x as i32 - border,
            y as i32 - border,
            side as i32 + 2 * border,
            side as i32 + 2 * border,
        );
        gl.clear_color(ink[0], ink[1], ink[2], 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        gl.scissor(x as i32, y as i32, side as i32, side as i32);
        let paper = rgb(scene::PAPER);
        gl.clear_color(paper[0], paper[1], paper[2], 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        gl.viewport(x as i32, y as i32, side as i32, side as i32);
        let (x0, z0, x1, z1) = scene::bounds(&self.game.garden);
        let centre = Vec3::new((x0 + x1) / 2.0, 0.0, (z0 + z1) / 2.0);
        let half = ((x1 - x0).max(z1 - z0)) / 2.0 + 3.0;
        let projection = Mat4::orthographic_rh_gl(-half, half, -half, half, 1.0, 120.0);
        let view = Mat4::look_at_rh(centre + Vec3::Y * 60.0, centre, Vec3::NEG_Z);
        gpu.camera(&(projection * view), side, side);
        let m = &self.meshes;
        gpu.draw(&m.hedges, &Mat4::IDENTITY, [0.78, 0.78, 0.76], None);
        let game = &self.game;
        let garden = &game.garden;
        let carrot = rgb(scene::CARROT);
        for (index, c) in garden.edibles.iter().enumerate() {
            if game.eaten[index] {
                continue;
            }
            let at = point(garden.point(c.edge, c.s, i32::from(c.lane) * bunny_rules::LANE_WIDTH));
            gpu.draw(
                &m.dot,
                &scene::place(Vec3::new(at.x, 3.0, at.y), 0.0, 1.3),
                carrot,
                None,
            );
        }
        let fur = rgb(self.fur());
        if game.farmer_on {
            gpu.draw(
                &m.dot,
                &scene::place(Vec3::new(farmer.x, 4.0, farmer.y), 0.0, 4.2),
                ink,
                None,
            );
            gpu.draw(
                &m.dot,
                &scene::place(Vec3::new(farmer.x, 4.1, farmer.y), 0.0, 2.8),
                [0.55, 0.55, 0.53],
                None,
            );
        }
        gpu.draw(
            &m.dot,
            &scene::place(Vec3::new(bunny.x, 5.0, bunny.y), 0.0, 4.2),
            ink,
            None,
        );
        gpu.draw(
            &m.dot,
            &scene::place(Vec3::new(bunny.x, 5.1, bunny.y), 0.0, 3.0),
            fur,
            None,
        );
        gl.disable(Gl::SCISSOR_TEST);
    }

    fn update_hud(&mut self) {
        let left = self.game.food_left;
        let tier = self.game.bunny.tier;
        let shown = (left, tier, self.phase);
        let late = self.phase == Phase::Over && self.time - self.over_at > 0.7;
        let card_visible = self.phase == Phase::Title || late;
        let hud = &self.hud;
        let showing_card = hud
            .card
            .style()
            .get_property_value("display")
            .unwrap_or_default()
            != "none";
        if shown == self.shown && card_visible == showing_card {
            return;
        }
        self.shown = shown;
        hud.carrots.set_text_content(Some(&left.to_string()));
        hud.size
            .set_text_content(Some(TIER_NAMES[usize::from(tier)]));
        let fur = hex(self.fur());
        for (index, pip) in hud.pips.iter().enumerate() {
            let filled = index <= usize::from(tier);
            css(
                pip,
                &[("background", if filled { "#1e1e1e" } else { "transparent" })],
            );
        }
        show(&hud.stats, self.phase != Phase::Title);
        show(&hud.restart, self.phase == Phase::Playing);
        show(&hud.card, card_visible);
        match self.phase {
            Phase::Title => {
                hud.heading.set_text_content(Some(copy::TITLE));
                hud.line.set_text_content(Some(copy::GOAL));
                hud.action.set_text_content(Some(copy::PLAY));
                show(&hud.hint, true);
                show(&hud.swatch, self.wins > 0);
                css(&hud.swatch, &[("background", fur.as_str())]);
            }
            Phase::Over => {
                let won = self.game.status == Status::Won;
                hud.heading
                    .set_text_content(Some(if won { copy::WON } else { copy::CAUGHT }));
                let text = if won {
                    copy::won_line(self.wins)
                } else {
                    copy::CAUGHT_LINE.to_owned()
                };
                hud.line.set_text_content(Some(&text));
                hud.action.set_text_content(Some(copy::PLAY_AGAIN));
                show(&hud.hint, false);
                show(&hud.swatch, won);
                css(&hud.swatch, &[("background", fur.as_str())]);
            }
            Phase::Playing => {}
        }
        let _ = &hud.root;
        let _ = &hud.map_label;
    }
}

fn input_for(key: &str) -> Option<Input> {
    match key {
        "ArrowLeft" | "a" | "A" => Some(Input::Left),
        "ArrowRight" | "d" | "D" => Some(Input::Right),
        "ArrowUp" | "w" | "W" | " " => Some(Input::Jump),
        "ArrowDown" | "s" | "S" => Some(Input::Duck),
        "x" | "X" | "Backspace" => Some(Input::Back),
        _ => None,
    }
}

fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}

fn fail(document: &Document, parent: &web_sys::Element, text: &str) {
    let note = element(document, "p");
    note.set_text_content(Some(text));
    css(
        &note,
        &[
            ("position", "absolute"),
            ("left", "50%"),
            ("top", "50%"),
            ("transform", "translate(-50%, -50%)"),
            ("margin", "0"),
            ("padding", "16px 20px"),
            ("max-width", "80vw"),
            ("font-family", "system-ui, sans-serif"),
            ("background", "#f4f4f2"),
            ("color", "#1e1e1e"),
            ("border", "2px solid #1e1e1e"),
            ("border-radius", "12px"),
        ],
    );
    let _ = parent.append_child(&note);
}

pub fn start() {
    let window = web_sys::window().expect("a window");
    let document = window.document().expect("a document");
    let canvas: HtmlCanvasElement = match document.get_element_by_id("bunny-canvas") {
        Some(found) => found.dyn_into().expect("a canvas"),
        None => {
            let made: HtmlCanvasElement = document
                .create_element("canvas")
                .expect("a canvas")
                .dyn_into()
                .expect("a canvas");
            let body = document.body().expect("a body");
            css(
                body.as_ref(),
                &[
                    ("margin", "0"),
                    ("overflow", "hidden"),
                    ("background", "#ecece9"),
                ],
            );
            let _ = body.append_child(&made);
            made
        }
    };
    css(
        canvas.as_ref(),
        &[
            ("display", "block"),
            ("width", "100vw"),
            ("height", "100vh"),
            ("height", "100dvh"),
            ("touch-action", "none"),
            ("outline", "none"),
        ],
    );
    let _ = canvas.set_attribute("tabindex", "0");
    let parent = canvas.parent_element().expect("a parent");
    if let Some(status) = document.get_element_by_id("bunny-status") {
        status.set_text_content(Some(""));
    }
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"antialias".into(), &true.into());
    let gl = canvas
        .get_context_with_context_options("webgl2", &options)
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into::<Gl>().ok());
    let Some(gl) = gl else {
        fail(&document, &parent, copy::NO_WEBGL);
        return;
    };
    let gpu = match Gpu::new(gl) {
        Ok(gpu) => gpu,
        Err(error) => {
            web_sys::console::error_1(&error.into());
            fail(&document, &parent, copy::NO_WEBGL);
            return;
        }
    };
    let garden = level::garden(1);
    let wins = load_wins(&window);
    let fur = shade::fur(wins);
    let obstacle = |kind| gpu.upload(&scene::obstacle(kind));
    let meshes = Meshes {
        ground: gpu.upload(&scene::ground(&garden)),
        hedges: gpu.upload(&scene::hedges(&garden)),
        edibles: EdibleKind::ALL
            .iter()
            .map(|kind| (*kind, gpu.upload(&scene::edible(*kind))))
            .collect(),
        bunny: gpu.upload(&scene::bunny(fur)),
        ear: gpu.upload(&scene::ear(fur)),
        farmer: gpu.upload(&scene::farmer()),
        leg: gpu.upload(&scene::leg()),
        net: gpu.upload(&scene::net()),
        alarm: gpu.upload(&scene::alarm()),
        shadow: gpu.upload(&scene::shadow()),
        crumb: gpu.upload(&scene::crumb()),
        dot: gpu.upload(&scene::dot()),
        pot: obstacle(ObstacleKind::Pot),
        gnome: obstacle(ObstacleKind::Gnome),
        fence: obstacle(ObstacleKind::Fence),
        gap: obstacle(ObstacleKind::Gap),
        barrow: obstacle(ObstacleKind::Barrow),
    };
    let touch = window
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let hud = build_hud(&document, &parent, touch);
    let options = Options::parse(&window.location().hash().unwrap_or_default());
    let tier = options.tier.unwrap_or(Tier::default_for(touch));
    let outline = match crate::outline::Outline::new(&gpu.gl) {
        Ok(outline) => Some(outline),
        Err(error) => {
            web_sys::console::error_1(&error.into());
            None
        }
    };
    let mut app = App {
        window: window.clone(),
        canvas: canvas.clone(),
        gpu,
        outline,
        tier,
        options,
        meshes,
        hud,
        game: Game::new(garden),
        phase: Phase::Title,
        wins,
        queue: Vec::new(),
        last: 0.0,
        carry: 0.0,
        time: 0.0,
        over_at: 0.0,
        prev_bunny: Vec2::ZERO,
        prev_farmer: Vec2::ZERO,
        bunny_slide: Vec2::ZERO,
        farmer_slide: Vec2::ZERO,
        bunny_yaw: 0.0,
        farmer_yaw: 0.0,
        camera_yaw: 0.0,
        size: 0.25,
        camera_size: 0.0,
        hop: 0.0,
        stride: 0.0,
        particles: Vec::new(),
        touch: None,
        shown: (usize::MAX, 0, Phase::Over),
    };
    app.new_run();
    let app = Rc::new(RefCell::new(app));

    {
        let app = app.clone();
        listen::<KeyboardEvent>(window.as_ref(), "keydown", move |event| {
            let key = event.key();
            let mut app = app.borrow_mut();
            if app.phase == Phase::Playing
                && let Some(input) = input_for(&key)
            {
                event.prevent_default();
                if !event.repeat() {
                    app.press(input);
                }
                return;
            }
            let start = matches!(key.as_str(), "Enter" | " " | "ArrowUp" | "w" | "W");
            if start && app.phase != Phase::Playing {
                event.prevent_default();
                app.play();
            } else if (key == "r" || key == "R") && app.phase == Phase::Playing {
                app.play();
            } else if key == " " {
                event.prevent_default();
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerdown", move |event| {
            let mut app = app.borrow_mut();
            app.touch = Some((
                event.client_x() as f32,
                event.client_y() as f32,
                event.time_stamp(),
            ));
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerup", move |event| {
            let mut app = app.borrow_mut();
            let Some((x, y, at)) = app.touch.take() else {
                return;
            };
            let (dx, dy) = (event.client_x() as f32 - x, event.client_y() as f32 - y);
            if event.time_stamp() - at > 600.0 || dx.abs().max(dy.abs()) < 24.0 {
                return;
            }
            let input = if dx.abs() > dy.abs() {
                Some(if dx < 0.0 { Input::Left } else { Input::Right })
            } else if dy > 0.0 {
                Some(Input::Back)
            } else {
                Some(Input::Jump)
            };
            if let Some(input) = input {
                app.press(input);
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointercancel", move |_| {
            app.borrow_mut().touch = None;
        });
    }
    {
        let app_click = app.clone();
        let action = app.borrow().hud.action.clone();
        listen::<web_sys::Event>(action.as_ref(), "click", move |_| {
            app_click.borrow_mut().play();
        });
        let app_restart = app.clone();
        let restart = app.borrow().hud.restart.clone();
        listen::<web_sys::Event>(restart.as_ref(), "click", move |_| {
            app_restart.borrow_mut().play();
        });
    }

    let next: FrameLoop = Rc::new(RefCell::new(None));
    let first = next.clone();
    let looping = window.clone();
    *first.borrow_mut() = Some(Closure::new(move |now: f64| {
        app.borrow_mut().frame(now);
        if let Some(callback) = next.borrow().as_ref() {
            let _ = looping.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    if let Some(callback) = first.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

/// The animation-frame callback, kept so it can ask for the next frame.
type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
