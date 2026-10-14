//! The browser side: WebGL2 drawing, Roblox-style input and camera, the
//! HUD, sounds, and the tutorial's storage.

use std::cell::RefCell;
use std::f32::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

use glam::{Mat4, Quat, Vec3, Vec4};
use icecream_rules::bot::Bot;
use icecream_rules::game::{DropKind, ShotKind};
use icecream_rules::layout::{cell_of, centre};
use icecream_rules::{
    DT, Event, Game, HZ, Input, ManState, Options as RuleOptions, Pickup, Prompt, Status, Tool, V2,
    v,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, HtmlCanvasElement, KeyboardEvent, PointerEvent, WebGl2RenderingContext as Gl,
    WebGlBuffer, WebGlProgram, WebGlUniformLocation, WebGlVertexArrayObject, WheelEvent, Window,
};

use crate::camera::{Camera, View, ray_to_height};
use crate::copy;
use crate::hud::{Action, Card, Hold, Hud, css, element};
use crate::mesh::{Mesh, STRIDE, rgb};
use crate::options::Options;
use crate::scene;
use crate::sound::{Effect, Sound};
use crate::tutorial::{self, Tour};

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
out vec3 v_world;
void main() {
  vec4 world = u_model * vec4(a_position, 1.0);
  vec4 clip = u_view_projection * world;
  if (u_outline > 0.0) {
    if (a_weight <= 0.0) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
    vec4 push = u_view_projection * vec4(mat3(u_model) * a_hull, 0.0);
    vec2 dir = push.xy;
    float len = length(dir);
    if (len > 1e-6) { dir /= len; }
    float near = clamp(6.0 / clip.w, 0.35, 1.0);
    clip.xy += dir * u_outline * a_weight * near * 2.0 / u_viewport * clip.w;
  }
  v_normal = mat3(u_model) * a_normal;
  v_colour = a_colour;
  v_world = world.xyz;
  gl_Position = clip;
}
";

const FRAGMENT: &str = r"#version 300 es
precision mediump float;
in vec3 v_normal;
in vec3 v_colour;
in vec3 v_world;
uniform vec3 u_tint;
uniform float u_flat;
uniform vec3 u_light;
uniform vec3 u_eye;
uniform vec3 u_fog;
uniform vec3 u_fog_range;
uniform float u_shine;
out vec4 o_colour;
void main() {
  float dist = length(u_eye - v_world);
  float fog = smoothstep(u_fog_range.x, u_fog_range.y, dist) * u_fog_range.z;
  if (u_flat > 0.5) {
    o_colour = vec4(mix(u_tint, u_fog, fog * 0.6), 1.0);
    return;
  }
  vec3 n = normalize(v_normal);
  vec3 l = normalize(vec3(0.35, 1.0, 0.25));
  float d = dot(n, l);
  float band = d > 0.25 ? 1.0 : (d > -0.35 ? 0.86 : 0.74);
  vec3 c = v_colour * u_tint * u_light * band;
  if (u_shine > 0.0) {
    vec3 v = normalize(u_eye - v_world);
    vec3 h = normalize(l + v);
    float s = pow(max(dot(n, h), 0.0), 28.0);
    c += vec3(1.0) * smoothstep(0.4, 0.5, s) * 0.6 * u_shine + vec3(0.06) * u_shine;
  }
  o_colour = vec4(mix(c, u_fog, fog), 1.0);
}
";

const STEP: f64 = DT as f64;
const MAX_FRAME: f64 = 0.1;
/// Dim freezer-blue haze far down the hallways.
const FOG: [f32; 3] = [0.30, 0.33, 0.46];
/// The Backrooms' haze: yellow-grey.
const BACK_FOG: [f32; 3] = [0.56, 0.52, 0.33];
/// Fluorescent tubes, a little green-yellow.
const FLUORESCENT: [f32; 3] = [1.0, 0.97, 0.80];
const WARM: [f32; 3] = [1.0, 0.94, 0.86];
const COLD: [f32; 3] = [0.74, 0.84, 1.0];
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const INK: [f32; 3] = [0.17, 0.11, 0.08];
/// The outline's width, in CSS pixels.
const LINE: f32 = 2.5;
/// How long the end of a run plays before its card, in seconds.
const ENDING: f32 = 1.8;

struct GpuMesh {
    vao: WebGlVertexArrayObject,
    count: i32,
}

struct Gpu {
    gl: Gl,
    u_view_projection: Option<WebGlUniformLocation>,
    u_model: Option<WebGlUniformLocation>,
    u_outline: Option<WebGlUniformLocation>,
    u_viewport: Option<WebGlUniformLocation>,
    u_tint: Option<WebGlUniformLocation>,
    u_flat: Option<WebGlUniformLocation>,
    u_light: Option<WebGlUniformLocation>,
    u_eye: Option<WebGlUniformLocation>,
    u_fog: Option<WebGlUniformLocation>,
    u_fog_range: Option<WebGlUniformLocation>,
    u_shine: Option<WebGlUniformLocation>,
    scale: f32,
    /// The fog: its colour, and where it starts, is thickest, and how
    /// thick.
    fog: [f32; 3],
    fog_range: [f32; 3],
    _buffers: Vec<WebGlBuffer>,
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

fn program(gl: &Gl) -> Result<WebGlProgram, String> {
    let vertex = compile(gl, Gl::VERTEX_SHADER, VERTEX)?;
    let fragment = compile(gl, Gl::FRAGMENT_SHADER, FRAGMENT)?;
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

impl Gpu {
    fn new(gl: Gl) -> Result<Self, String> {
        let program = program(&gl)?;
        gl.use_program(Some(&program));
        let at = |name: &str| gl.get_uniform_location(&program, name);
        let gpu = Self {
            u_view_projection: at("u_view_projection"),
            u_model: at("u_model"),
            u_outline: at("u_outline"),
            u_viewport: at("u_viewport"),
            u_tint: at("u_tint"),
            u_flat: at("u_flat"),
            u_light: at("u_light"),
            u_eye: at("u_eye"),
            u_fog: at("u_fog"),
            u_fog_range: at("u_fog_range"),
            u_shine: at("u_shine"),
            gl,
            scale: 1.0,
            fog: FOG,
            fog_range: [9.0, 34.0, 0.82],
            _buffers: Vec::new(),
        };
        gpu.gl.enable(Gl::DEPTH_TEST);
        gpu.gl.enable(Gl::CULL_FACE);
        Ok(gpu)
    }

    fn upload(&mut self, mesh: &Mesh) -> GpuMesh {
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
        let array = js_sys::Float32Array::from(mesh.data.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::STATIC_DRAW);
        gl.bind_vertex_array(None);
        self._buffers.push(buffer);
        GpuMesh {
            vao,
            count: mesh.vertices() as i32,
        }
    }

    fn frame(&self, view_projection: &Mat4, eye: Vec3, width: f32, height: f32) {
        let gl = &self.gl;
        gl.uniform_matrix4fv_with_f32_array(
            self.u_view_projection.as_ref(),
            false,
            &view_projection.to_cols_array(),
        );
        gl.uniform2f(self.u_viewport.as_ref(), width, height);
        gl.uniform3f(self.u_eye.as_ref(), eye.x, eye.y, eye.z);
        gl.uniform3f(self.u_fog.as_ref(), self.fog[0], self.fog[1], self.fog[2]);
        let r = self.fog_range;
        gl.uniform3f(self.u_fog_range.as_ref(), r[0], r[1], r[2]);
    }

    /// Draws `mesh` in its colours times `tint` under `light`, glossy by
    /// `shine`, with an ink outline when `outline` is set.
    fn draw(
        &self,
        mesh: &GpuMesh,
        model: &Mat4,
        tint: [f32; 3],
        light: [f32; 3],
        shine: f32,
        outline: bool,
    ) {
        if mesh.count == 0 {
            return;
        }
        let gl = &self.gl;
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.uniform_matrix4fv_with_f32_array(self.u_model.as_ref(), false, &model.to_cols_array());
        if outline {
            gl.cull_face(Gl::FRONT);
            gl.uniform1f(self.u_outline.as_ref(), LINE * self.scale);
            gl.uniform1f(self.u_flat.as_ref(), 1.0);
            gl.uniform3f(self.u_tint.as_ref(), INK[0], INK[1], INK[2]);
            gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
        }
        gl.cull_face(Gl::BACK);
        gl.uniform1f(self.u_outline.as_ref(), 0.0);
        gl.uniform1f(self.u_flat.as_ref(), 0.0);
        gl.uniform1f(self.u_shine.as_ref(), shine);
        gl.uniform3f(self.u_tint.as_ref(), tint[0], tint[1], tint[2]);
        gl.uniform3f(self.u_light.as_ref(), light[0], light[1], light[2]);
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
    }

    /// Draws `mesh` in one flat colour (lights glowing).
    fn draw_flat(&self, mesh: &GpuMesh, model: &Mat4, colour: [f32; 3]) {
        let gl = &self.gl;
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.uniform_matrix4fv_with_f32_array(self.u_model.as_ref(), false, &model.to_cols_array());
        gl.uniform1f(self.u_outline.as_ref(), 0.0);
        gl.uniform1f(self.u_flat.as_ref(), 1.0);
        gl.uniform3f(self.u_tint.as_ref(), colour[0], colour[1], colour[2]);
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
    }
}

struct Meshes {
    rooms: Vec<GpuMesh>,
    lights: Vec<GpuMesh>,
    torso: GpuMesh,
    head: GpuMesh,
    arm: GpuMesh,
    leg: GpuMesh,
    tools: Vec<GpuMesh>,
    man_body: GpuMesh,
    man_leg: GpuMesh,
    man_arm: GpuMesh,
    man_head: GpuMesh,
    man_forehead: GpuMesh,
    key: GpuMesh,
    fudge: GpuMesh,
    cloud: GpuMesh,
    sparkle: GpuMesh,
    star: GpuMesh,
    sprinkles: Vec<GpuMesh>,
    splat: GpuMesh,
    drip: GpuMesh,
    cone: GpuMesh,
    treat: GpuMesh,
    cone_shot: GpuMesh,
    sugar: GpuMesh,
    buttons: Vec<GpuMesh>,
    cabinet: GpuMesh,
    puddle: GpuMesh,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Title,
    Playing,
    Menu,
    /// The end of a run playing out, for this long so far.
    Ending(f32),
    Over,
}

/// A short-lived thing: a splat, a drip, a puff.
struct Particle {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size: f32,
    kind: ParticleKind,
}

#[derive(Clone, Copy, PartialEq)]
enum ParticleKind {
    /// Fudge on a wall or the floor.
    Splat,
    /// A vanilla drip he leaves behind.
    Drip,
    /// A flying blob of fudge.
    Blob,
    /// The sprinkle staff's long sprinkle: from `pos`, along `vel`.
    Beam,
    /// A puff of sugar.
    Puff,
}

struct App {
    window: Window,
    canvas: HtmlCanvasElement,
    gpu: Gpu,
    meshes: Meshes,
    hud: Hud,
    sound: Sound,
    options: Options,
    game: Game,
    bot: Option<Bot>,
    seed: u64,
    phase: Phase,
    tour: Option<Tour>,
    touch: bool,
    camera: Camera,
    /// Keys held, by `KeyboardEvent.key`, lower case.
    held: Vec<String>,
    /// Keys pressed since the last step, so a tap shorter than a frame
    /// still counts.
    taps: Vec<String>,
    select: Option<Tool>,
    fire: bool,
    sprint_toggle: bool,
    touch_jump: bool,
    touch_fire: bool,
    touch_use: bool,
    /// The right mouse button's drag: the last point.
    orbit: Option<(f32, f32)>,
    /// The mouse over the canvas, in CSS pixels.
    mouse: Option<(f32, f32)>,
    /// The touch stick: pointer id, its start, the offset now.
    stick: Option<(i32, (f32, f32), (f32, f32))>,
    /// A finger looking around: pointer id and its last point.
    look: Option<(i32, (f32, f32))>,
    last: f64,
    carry: f64,
    time: f32,
    hurt: f32,
    daylight: f32,
    yaw: f32,
    man_yaw: f32,
    stride: f32,
    man_stride: f32,
    drip_wait: f32,
    man_hit: f32,
    particles: Vec<Particle>,
    view: Option<(View, Mat4)>,
    frames: u32,
    /// How often he has spotted you this run, for browser checks.
    spotted: u32,
    /// A swing of the sword or spear: time left.
    swing: f32,
    /// Minions' strides, by their place in the list.
    minion_stride: f32,
}

fn yaw_of(dir: V2) -> f32 {
    dir.x.atan2(dir.z)
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(2.0 * PI) - PI
}

fn approach_angle(value: f32, target: f32, rate: f32, dt: f32) -> f32 {
    value + wrap(target - value) * (1.0 - (-rate * dt).exp())
}

fn hash(a: u32, b: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

fn new_seed() -> u64 {
    let a = (js_sys::Math::random() * 4_294_967_296.0) as u64;
    let b = (js_sys::Math::random() * 4_294_967_296.0) as u64;
    a << 32 | b
}

fn tutorial_seen(window: &Window) -> Option<String> {
    window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(tutorial::SEEN_KEY).ok().flatten())
}

fn mark_tutorial_seen(window: &Window) {
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(tutorial::SEEN_KEY, tutorial::SEEN);
    }
}

impl App {
    fn rule_options(&self) -> RuleOptions {
        RuleOptions {
            man: !self.options.calm,
        }
    }

    fn new_run(&mut self) {
        self.seed = self.options.seed.unwrap_or_else(new_seed);
        self.game = Game::tuned(self.seed, self.rule_options(), self.options.tuning());
        if self.options.no_farts {
            self.game.player.farts = 0;
        }
        if self.options.arm {
            self.game.arm();
        }
        if self.options.free_roam {
            self.game.status = Status::Escaped;
            self.game.won = Some(Status::Escaped);
            self.game.keep_playing();
            self.game.events.clear();
        }
        if self.options.cone {
            let at = self.game.player.pos + self.game.player.facing * 2.0;
            self.game.drops.push(icecream_rules::Drop {
                at,
                kind: DropKind::Cone,
                age: 0.0,
            });
        }
        if let Some(name) = &self.options.tool
            && let Some(tool) = Tool::ALL
                .iter()
                .find(|t| format!("{t:?}").eq_ignore_ascii_case(name))
            && self.game.has(*tool)
        {
            self.game.player.tool = *tool;
        }
        if self.options.portrait {
            self.game.man.pos = self.game.house.spawn + v(0.4, -3.4);
            self.game.man.facing = v(-0.1, 1.0).normalized();
            self.game.player.facing = v(0.0, -1.0);
        }
        self.rebuild_house();
        self.bot = self.options.bot.then(Bot::default);
        self.camera = Camera::default();
        self.yaw = yaw_of(self.game.player.facing);
        self.camera.yaw = self.yaw;
        if self.options.first_person || self.game.tuning.first_person == 1 {
            self.camera.toggle_view();
        }
        let t = self.game.tuning;
        self.sound.set_buzz(t.buzz);
        self.man_yaw = 0.0;
        self.particles.clear();
        self.spotted = 0;
        self.hurt = 0.0;
        self.daylight = 0.0;
        self.carry = 0.0;
        self.select = None;
    }

    fn rebuild_house(&mut self) {
        let house = &self.game.house;
        let t = self.game.tuning;
        let yellow = t.yellow;
        let rooms: Vec<Mesh> = (0..house.rooms.len())
            .map(|i| scene::room(house, i, yellow))
            .collect();
        let lights: Vec<Mesh> = (0..house.rooms.len())
            .map(|i| scene::lights(house, i, yellow))
            .collect();
        let blend = |a: [f32; 3], b: [f32; 3]| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * yellow);
        self.gpu.fog = blend(FOG, BACK_FOG);
        self.gpu.fog_range = [
            t.fog_near,
            t.fog_far.max(t.fog_near + 0.5),
            0.82 + 0.14 * yellow,
        ];
        self.meshes.rooms = rooms.iter().map(|m| self.gpu.upload(m)).collect();
        self.meshes.lights = lights.iter().map(|m| self.gpu.upload(m)).collect();
    }

    fn start_playing(&mut self) {
        self.phase = Phase::Playing;
        self.sound.quiet(false);
        let note = if self.game.free_roam {
            copy::FREE_ROAM
        } else if self.game.weapons_on() {
            copy::WEAPONS_HINT
        } else {
            copy::NOTE
        };
        self.hud.toast(note, self.now());
    }

    /// V: first person or third person.
    fn toggle_view(&mut self) {
        let first = self.camera.toggle_view();
        let document = self.window.document().expect("a document");
        if first {
            self.canvas.request_pointer_lock();
        } else if !self.camera.shift_lock && document.pointer_lock_element().is_some() {
            document.exit_pointer_lock();
        }
        let now = self.now();
        self.hud.toast(
            if first {
                copy::FIRST_PERSON_ON
            } else {
                copy::THIRD_PERSON_ON
            },
            now,
        );
    }

    fn now(&self) -> f64 {
        self.window.performance().map_or(0.0, |p| p.now() / 1000.0)
    }

    fn set_shift_lock(&mut self, on: bool) {
        self.camera.shift_lock = on;
        let document = self.window.document().expect("a document");
        if on {
            self.canvas.request_pointer_lock();
        } else if document.pointer_lock_element().is_some() {
            document.exit_pointer_lock();
        }
        let now = self.now();
        self.hud.toast(
            if on {
                copy::SHIFT_LOCK_ON
            } else {
                copy::SHIFT_LOCK_OFF
            },
            now,
        );
    }

    fn act(&mut self, action: Action) {
        self.sound.wake();
        match action {
            Action::Play => {
                if tutorial::opens_by_itself(
                    tutorial_seen(&self.window).as_deref(),
                    self.options.skip,
                ) {
                    self.tour = Some(Tour::new());
                }
                self.start_playing();
            }
            Action::Resume => {
                if self.phase == Phase::Menu {
                    self.phase = Phase::Playing;
                    self.sound.quiet(false);
                }
            }
            Action::Menu => {
                if self.phase == Phase::Playing {
                    self.phase = Phase::Menu;
                    self.sound.quiet(true);
                    self.release();
                } else if self.phase == Phase::Menu {
                    self.act(Action::Resume);
                }
            }
            Action::NewHouse => {
                self.new_run();
                self.start_playing();
            }
            Action::Tour => self.tour = Some(Tour::new()),
            Action::TourNext => {
                self.tour = self.tour.and_then(Tour::next);
                if self.tour.is_none() {
                    mark_tutorial_seen(&self.window);
                }
            }
            Action::TourBack => self.tour = self.tour.map(Tour::back),
            Action::TourEnd(_) => {
                self.tour = None;
                mark_tutorial_seen(&self.window);
            }
            Action::Select(tool) => self.select = Some(tool),
            Action::KeepPlaying => {
                if self.game.keep_playing() {
                    self.daylight = 0.0;
                    self.phase = Phase::Playing;
                    self.sound.quiet(false);
                    let now = self.now();
                    self.hud.toast(copy::FREE_ROAM, now);
                }
            }
            Action::SprintToggle => self.sprint_toggle = !self.sprint_toggle,
            Action::Held(hold, down) => match hold {
                Hold::Jump => self.touch_jump = down,
                Hold::Fire => self.touch_fire = down,
                Hold::Use => self.touch_use = down,
            },
        }
    }

    /// Lets go of everything held (a menu, a lost focus).
    fn release(&mut self) {
        self.held.clear();
        self.taps.clear();
        self.fire = false;
        self.orbit = None;
        self.touch_fire = false;
        self.touch_jump = false;
        self.touch_use = false;
        self.stick = None;
        self.hud.stick(0.0, 0.0);
    }

    fn holding(&self, keys: &[&str]) -> bool {
        keys.iter()
            .any(|k| self.held.iter().chain(&self.taps).any(|h| h == k))
    }

    /// This step's input from the keys, the mouse, touch and the camera.
    fn input(&mut self) -> Input {
        if let Some(bot) = &mut self.bot {
            return bot.input(&self.game);
        }
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let mut ahead = axis(
            self.holding(&["w", "arrowup"]),
            self.holding(&["s", "arrowdown"]),
        );
        let mut side = axis(
            self.holding(&["d", "arrowright"]),
            self.holding(&["a", "arrowleft"]),
        );
        if let Some((_, _, (dx, dy))) = self.stick {
            ahead -= dy / 50.0;
            side += dx / 50.0;
        }
        let walk = self
            .camera
            .walk(ahead.clamp(-1.0, 1.0), side.clamp(-1.0, 1.0));
        let player = &self.game.player;
        let mut aim = if self.camera.aims() {
            self.camera.forward()
        } else if let (Some((x, y)), Some((view, matrix)), false) =
            (self.mouse, self.view, self.touch)
        {
            let width = self.canvas.client_width().max(1) as f32;
            let height = self.canvas.client_height().max(1) as f32;
            let ndc = (2.0 * x / width - 1.0, 1.0 - 2.0 * y / height);
            let inverse = matrix.inverse();
            let near = inverse * Vec4::new(ndc.0, ndc.1, -1.0, 1.0);
            let far = inverse * Vec4::new(ndc.0, ndc.1, 1.0, 1.0);
            let near = near.truncate() / near.w;
            let far = far.truncate() / far.w;
            match ray_to_height(view.eye, far - near, 1.1) {
                Some(point) => (point - player.pos).normalized(),
                None => self.camera.forward(),
            }
        } else {
            player.facing
        };
        // A little help: snap to him, or a minion of his, when nearly in
        // line.
        let cone = if self.camera.aims() { 0.97 } else { 0.85 };
        let mut targets: Vec<V2> = self
            .game
            .minions
            .iter()
            .filter(|m| !m.friendly)
            .map(|m| m.pos)
            .collect();
        if self.game.man_here() {
            targets.push(self.game.man.pos);
        }
        let best = targets
            .into_iter()
            .filter(|at| at.distance(player.pos) < 18.0)
            .map(|at| (at - player.pos).normalized())
            .filter(|dir| aim.dot(*dir) > cone)
            .max_by(|a, b| aim.dot(*a).total_cmp(&aim.dot(*b)));
        if let Some(dir) = best {
            aim = dir;
        }
        Input {
            walk,
            sprint: self.holding(&["shift"]) || self.sprint_toggle,
            jump: self.holding(&[" "]) || self.touch_jump,
            fire: self.fire || self.touch_fire || self.taps.iter().any(|k| k == "click"),
            aim,
            interact: self.holding(&["e"]) || self.touch_use,
            select: self.select.take(),
        }
    }

    fn frame(&mut self, now: f64) {
        let seconds = now / 1000.0;
        let dt = if self.last == 0.0 {
            0.0
        } else {
            (seconds - self.last).min(MAX_FRAME)
        };
        self.last = seconds;
        self.time += dt as f32;
        for action in self.hud.take_actions() {
            self.act(action);
        }
        let running = self.phase == Phase::Playing && self.tour.is_none() && !self.options.portrait;
        if running {
            self.carry += dt * f64::from(self.options.speed);
            let mut steps = 0;
            while self.carry >= STEP && steps < 8 * self.options.speed {
                self.carry -= STEP;
                steps += 1;
                let input = self.input();
                if input.fire && !self.camera.aims() && input.aim != V2::ZERO {
                    self.yaw = yaw_of(input.aim);
                }
                self.game.step(input);
                self.taps.clear();
                let events = self.game.events.clone();
                for event in events {
                    self.effect(event, seconds);
                }
                if self.game.status != Status::Playing {
                    self.phase = Phase::Ending(0.0);
                    self.sound.quiet(true);
                    self.release();
                    if self
                        .window
                        .document()
                        .and_then(|d| d.pointer_lock_element())
                        .is_some()
                    {
                        self.window
                            .document()
                            .expect("a document")
                            .exit_pointer_lock();
                    }
                    break;
                }
            }
        } else {
            self.carry = 0.0;
        }
        if let Phase::Ending(t) = self.phase {
            let t = t + dt as f32;
            self.phase = if t >= ENDING {
                Phase::Over
            } else {
                Phase::Ending(t)
            };
            if self.game.status == Status::Escaped {
                self.daylight = (t / ENDING * 1.4).min(1.0);
            }
        }
        self.animate(dt as f32);
        self.render();
        self.update_hud(seconds);
        self.frames += 1;
        if self.frames % 6 == 0 {
            self.publish();
        }
    }

    /// Sounds, words and bits flying for one event.
    fn effect(&mut self, event: Event, now: f64) {
        let player = self.game.player.pos;
        match event {
            Event::FudgeFired => self.sound.play(Effect::Fudge),
            Event::FudgeHit => {
                self.sound.play(Effect::Splat);
                self.man_hit = 0.25;
                let at = self.game.man.pos;
                for i in 0..5 {
                    let a = i as f32 * 1.3;
                    self.particles.push(Particle {
                        pos: Vec3::new(at.x, 1.6, at.z),
                        vel: Vec3::new(a.cos() * 1.5, 2.0, a.sin() * 1.5),
                        age: 0.0,
                        life: 0.6,
                        size: 0.14,
                        kind: ParticleKind::Blob,
                    });
                }
            }
            Event::FudgeSplat(at) => self.particles.push(Particle {
                pos: Vec3::new(at.x, 0.02, at.z),
                vel: Vec3::ZERO,
                age: 0.0,
                life: 4.0,
                size: 0.5,
                kind: ParticleKind::Splat,
            }),
            Event::Frozen => {
                self.sound.play(Effect::Frozen);
                self.hud.toast(copy::FROZEN, now);
            }
            Event::Fart(_) => self.sound.play(Effect::Fart),
            Event::FartHit => self.hud.toast(copy::FART_HIT, now),
            Event::Jingle => {
                self.sound.play(Effect::Jingle);
                self.hud.toast(copy::JINGLE, now);
            }
            Event::Empty(tool) => {
                self.sound.play(Effect::Empty);
                match tool {
                    Tool::Fudge => self.hud.toast(copy::EMPTY_FUDGE, now),
                    Tool::Fart => self.hud.toast(copy::EMPTY_FART, now),
                    Tool::Pistol => self.hud.toast(copy::EMPTY_CONES, now),
                    Tool::Sugar => self.hud.toast(copy::EMPTY_SUGAR, now),
                    _ => {}
                }
            }
            Event::Volley => self.sound.play(Effect::Bloop),
            Event::SprinkleHit => {
                self.sound.play(Effect::Hit);
                self.hurt = self.hurt.max(0.5);
            }
            Event::Grabbed | Event::PulledOut => {
                self.sound.play(Effect::Grab);
                self.hurt = 1.0;
                if event == Event::PulledOut {
                    self.hud.toast(copy::PULLED_OUT, now);
                }
            }
            Event::Spotted => {
                self.spotted += 1;
                self.hud.toast(copy::SPOTTED, now);
            }
            Event::Lost => self.hud.toast(copy::LOST, now),
            Event::Key(count) => {
                self.sound.play(Effect::Key);
                let text = if count >= 3 {
                    copy::FOUND_ALL.to_owned()
                } else {
                    copy::key_found(count)
                };
                self.hud.toast(&text, now);
            }
            Event::Pickup(pickup) => {
                self.sound.play(Effect::Pickup);
                self.hud.toast(copy::pickup(pickup), now);
                let _ = Pickup::Fudge;
            }
            Event::Hid => self.hud.toast(copy::HID, now),
            Event::Unhid => {}
            Event::DoorLocked => {
                self.sound.play(Effect::Locked);
                self.hud.toast(copy::LOCKED, now);
            }
            Event::Jumped => self.sound.play(Effect::Jump),
            Event::Escaped | Event::Melted => {
                self.sound.play(Effect::Escape);
                if event == Event::Melted && self.game.free_roam {
                    self.hud.toast(copy::MELTED, now);
                }
            }
            Event::Scooped => self.sound.play(Effect::Scooped),
            Event::Swing(_) => {
                self.swing = 0.25;
                self.sound.play(Effect::Jump);
            }
            Event::ConeFired => self.sound.play(Effect::Bloop),
            Event::SugarFired => self.sound.play(Effect::Fudge),
            Event::SugarBurst(at) => {
                self.sound.play(Effect::Fart);
                self.particles.push(Particle {
                    pos: Vec3::new(at.x, 1.0, at.z),
                    vel: Vec3::ZERO,
                    age: 0.0,
                    life: 0.8,
                    size: self.game.tuning.sugar_radius,
                    kind: ParticleKind::Puff,
                });
            }
            Event::Staff(from, to) => {
                self.sound.play(Effect::Frozen);
                self.particles.push(Particle {
                    pos: Vec3::new(from.x, 1.2, from.z),
                    vel: Vec3::new(to.x - from.x, 0.0, to.z - from.z),
                    age: 0.0,
                    life: 0.7,
                    size: 1.0,
                    kind: ParticleKind::Beam,
                });
            }
            Event::FriendsCame(_) => self.hud.toast(copy::FRIENDS, now),
            Event::ManHit => {
                self.man_hit = 0.25;
                self.sound.play(Effect::Splat);
            }
            Event::MinionCame(_) => self.hud.toast(copy::MINION_CAME, now),
            Event::MinionHit(_) => self.sound.play(Effect::Splat),
            Event::MinionMelted(at) => {
                self.sound.play(Effect::Pickup);
                self.particles.push(Particle {
                    pos: Vec3::new(at.x, 0.016, at.z),
                    vel: Vec3::ZERO,
                    age: 0.0,
                    life: 6.0,
                    size: 0.6,
                    kind: ParticleKind::Drip,
                });
            }
            Event::MinionBite => {
                self.sound.play(Effect::Hit);
                self.hurt = self.hurt.max(0.6);
                self.hud.toast(copy::MINION_BITE, now);
            }
            Event::Earned(tool) => {
                self.sound.play(Effect::Key);
                let line = copy::earned(tool);
                if !line.is_empty() {
                    self.hud.toast(line, now);
                }
            }
            Event::WeaponDropped(tool, _) => self.hud.toast(copy::earned(tool), now),
            Event::ConeDropped(_) => self.hud.toast(copy::CONE_DROPPED, now),
            Event::ConeEaten => {
                self.sound.play(Effect::Pickup);
                self.hud.toast(copy::CONE_EATEN, now);
            }
            Event::BoostOver => self.hud.toast(copy::BOOST_OVER, now),
            Event::Button(count) => {
                self.sound.play(Effect::Key);
                self.hud.toast(&copy::button_lit(count), now);
            }
            Event::PuzzleWrong => {
                self.sound.play(Effect::Locked);
                self.hud.toast(copy::PUZZLE_WRONG, now);
            }
            Event::PuzzleSolved => {
                self.sound.play(Effect::Escape);
                self.hud.toast(copy::PUZZLE_SOLVED, now);
            }
            Event::CabinetLocked => {
                self.sound.play(Effect::Locked);
                self.hud.toast(copy::CABINET_LOCKED, now);
            }
            Event::FreeRoam => {}
        }
        let _ = player;
    }

    fn animate(&mut self, dt: f32) {
        let game = &self.game;
        let player = &game.player;
        let target = if self.camera.aims() {
            yaw_of(self.camera.forward())
        } else if player.moving {
            yaw_of(player.facing)
        } else {
            self.yaw
        };
        if self.phase == Phase::Playing {
            self.yaw = approach_angle(self.yaw, target, 14.0, dt);
        }
        if self.camera.aims() {
            self.yaw = target;
        }
        if self.options.watch && self.phase == Phase::Playing {
            let to = self.game.man.pos - self.game.player.pos;
            self.camera.shift_lock = true;
            self.camera.zoom = 6.5;
            self.camera.pitch = 0.05;
            self.camera.yaw = approach_angle(self.camera.yaw, yaw_of(to) + 0.45, 6.0, dt);
        } else if self.bot.is_some() && self.phase == Phase::Playing {
            // While the bot plays (captures), the camera follows behind it.
            self.camera.yaw = approach_angle(self.camera.yaw, self.yaw, 2.5, dt);
        }
        if player.moving {
            self.stride += dt * if player.sprinting { 13.0 } else { 9.0 };
        } else {
            self.stride *= 0.8;
        }
        let man = &game.man;
        if man.moving {
            self.man_stride += dt * 7.0;
        }
        self.man_yaw = if self.options.portrait {
            yaw_of(man.facing)
        } else {
            approach_angle(self.man_yaw, yaw_of(man.facing), 10.0, dt)
        };
        self.hurt = (self.hurt - dt * 1.5).max(0.0);
        self.man_hit = (self.man_hit - dt).max(0.0);
        self.swing = (self.swing - dt).max(0.0);
        self.minion_stride += dt * 11.0;
        // He leaves vanilla drips behind him.
        if game.man_here() && man.moving && self.phase == Phase::Playing {
            self.drip_wait -= dt;
            if self.drip_wait <= 0.0 {
                self.drip_wait = 0.7;
                self.particles.push(Particle {
                    pos: Vec3::new(man.pos.x, 0.015, man.pos.z),
                    vel: Vec3::ZERO,
                    age: 0.0,
                    life: 8.0,
                    size: 0.35,
                    kind: ParticleKind::Drip,
                });
            }
        }
        for p in &mut self.particles {
            p.age += dt;
            if p.kind == ParticleKind::Blob {
                p.vel.y -= 9.0 * dt;
                p.pos += p.vel * dt;
            }
        }
        self.particles.retain(|p| p.age < p.life);
        if self.particles.len() > 120 {
            let extra = self.particles.len() - 120;
            self.particles.drain(..extra);
        }
    }

    fn resize(&mut self) -> (f32, f32) {
        let ratio = self.window.device_pixel_ratio().clamp(1.0, 2.0) as f32;
        let ratio = if self.touch { ratio.min(1.5) } else { ratio };
        let width = (self.canvas.client_width().max(1) as f32 * ratio) as u32;
        let height = (self.canvas.client_height().max(1) as f32 * ratio) as u32;
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        self.gpu.scale = ratio;
        (width as f32, height as f32)
    }

    /// A room's light now: warm or freezer blue, with a flicker.
    fn room_light(&self, index: usize) -> [f32; 3] {
        let room = &self.game.house.rooms[index];
        let t = &self.game.tuning;
        let backrooms = t.yellow >= 0.5;
        let base = if backrooms {
            FLUORESCENT
        } else if scene::cold(room.module) {
            COLD
        } else {
            WARM
        };
        let tick = (self.time * 14.0) as u32;
        // How often the tubes flicker (the version's number; 0.03 in the
        // original). Brief flickers never come more often than the
        // original's 5%; more than that becomes slow half-second dips, two
        // a second at most, so the lights never strobe.
        let fast = (t.flicker.min(0.05) * 10_000.0) as u32;
        let slow = ((t.flicker - 0.05).max(0.0) * 10_000.0) as u32;
        let flicker = if hash(index as u32, tick) % 10_000 < fast {
            if backrooms { 0.45 } else { 0.55 }
        } else if !backrooms
            && scene::cold(room.module)
            && hash(index as u32 + 50, tick / 3) % 100 < 6
        {
            0.8
        } else if slow > 0 && hash(index as u32 + 70, (self.time * 2.0) as u32) % 10_000 < slow {
            0.62
        } else {
            1.0
        };
        base.map(|c| c * flicker * 0.95)
    }

    fn render(&mut self) {
        let (width, height) = self.resize();
        let gl = &self.gpu.gl;
        gl.viewport(0, 0, width as i32, height as i32);
        let fog = self.gpu.fog;
        gl.clear_color(fog[0], fog[1], fog[2], 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        let game = &self.game;
        let player = &game.player;
        let mut camera = self.camera;
        let scooped = game.status == Status::Scooped;
        if let Phase::Ending(t) = self.phase {
            camera.yaw += t * 0.6;
            camera.zoom = camera.zoom.max(4.0);
            camera.shift_lock = false;
        }
        if self.options.portrait {
            camera.yaw = PI + 0.35;
            camera.zoom = 5.5;
            camera.pitch = 0.1;
            camera.shift_lock = true;
        }
        if matches!(self.phase, Phase::Title) {
            camera.yaw = self.time * 0.15;
            camera.zoom = 6.0;
        }
        let view = camera.view(player.pos, player.height, &game.house.blocks);
        let aspect = width / height;
        let matrix = Camera::matrix(&view, aspect);
        self.view = Some((view, matrix));
        self.gpu.frame(&matrix, view.eye, width, height);
        let gpu = &self.gpu;
        let m = &self.meshes;

        for (i, mesh) in m.rooms.iter().enumerate() {
            let light = self.room_light(i);
            gpu.draw(mesh, &Mat4::IDENTITY, WHITE, light, 0.0, false);
            let glow = light.map(|c| (c * 1.05).min(1.0));
            gpu.draw_flat(&m.lights[i], &Mat4::IDENTITY, glow);
        }
        let light_at = |p: V2| self.room_light(icecream_rules::layout::index(cell_of(p)));

        // Keys, bobbing and turning, each with a sparkle.
        for (i, key) in game.house.keys.iter().enumerate() {
            if game.taken[i] {
                continue;
            }
            let bob = (self.time * 2.5 + i as f32).sin() * 0.1;
            let model = Mat4::from_scale_rotation_translation(
                Vec3::splat(1.3),
                Quat::from_rotation_y(self.time * 2.0 + i as f32),
                Vec3::new(key.x, 0.75 + bob, key.z),
            );
            gpu.draw(&m.key, &model, WHITE, [1.2, 1.15, 1.0], 1.0, true);
            let a = self.time * 3.0 + i as f32;
            let sparkle = Mat4::from_scale_rotation_translation(
                Vec3::splat(0.12 + 0.05 * (self.time * 6.0).sin().abs()),
                Quat::IDENTITY,
                Vec3::new(key.x + a.cos() * 0.45, 1.0 + bob, key.z + a.sin() * 0.45),
            );
            gpu.draw_flat(&m.sparkle, &sparkle, [1.0, 1.0, 0.85]);
        }
        // Unopened fridges have a pink sparkle on top.
        for (i, (spot, _)) in game.house.fridges.iter().enumerate() {
            if game.opened[i] {
                continue;
            }
            let c = (spot.block.min + spot.block.max) * 0.5;
            let top = spot.block.height + 0.35 + (self.time * 3.0 + i as f32).sin() * 0.08;
            let model = Mat4::from_scale_rotation_translation(
                Vec3::splat(0.22),
                Quat::IDENTITY,
                Vec3::new(c.x, top, c.z),
            );
            gpu.draw_flat(&m.sparkle, &model, [1.0, 0.55, 0.72]);
        }
        // With every key, the way out glows.
        if player.keys as usize >= game.house.keys.len() {
            let at = game.house.front.stand;
            let pulse = 0.3 + 0.08 * (self.time * 5.0).sin();
            let model = Mat4::from_scale_rotation_translation(
                Vec3::splat(pulse),
                Quat::IDENTITY,
                Vec3::new(at.x, 2.6, at.z),
            );
            gpu.draw_flat(&m.sparkle, &model, [1.0, 0.86, 0.3]);
        }

        // The spear puzzle: its buttons, lit ones glowing, and the cone
        // cabinet with the spear inside until it's taken.
        if game.weapons_on() {
            for (i, at) in game.house.buttons.iter().enumerate() {
                let model = Mat4::from_translation(Vec3::new(at.x, 0.0, at.z));
                gpu.draw(
                    &m.buttons[i % m.buttons.len()],
                    &model,
                    WHITE,
                    light_at(*at).map(|c| c.max(0.9)),
                    0.5,
                    true,
                );
                if (i as u32) < game.lit || game.solved {
                    let glow = Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.22 + 0.04 * (self.time * 6.0).sin()),
                        Quat::IDENTITY,
                        Vec3::new(at.x, 1.35, at.z),
                    );
                    let c = rgb(scene::BUTTON_COLOURS[i % 3]);
                    gpu.draw_flat(&m.sparkle, &glow, c);
                }
            }
            let at = game.house.cabinet;
            let model = Mat4::from_translation(Vec3::new(at.x, 0.0, at.z));
            gpu.draw(&m.cabinet, &model, WHITE, light_at(at), 0.6, true);
            if !game.solved {
                let spear = Mat4::from_translation(Vec3::new(at.x, 0.35, at.z))
                    * Mat4::from_rotation_x(-FRAC_PI_2);
                let index = Tool::ALL
                    .iter()
                    .position(|t| *t == Tool::Spear)
                    .unwrap_or(0);
                gpu.draw(&m.tools[index], &spear, WHITE, light_at(at), 0.4, true);
            }
        }
        // Cones and weapons on the floor.
        for (i, d) in game.drops.iter().enumerate() {
            let bob = (self.time * 3.0 + i as f32).sin() * 0.08;
            let turn = Quat::from_rotation_y(self.time * 1.8 + i as f32);
            match d.kind {
                DropKind::Cone => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(1.2),
                        turn,
                        Vec3::new(d.at.x, 0.15 + bob, d.at.z),
                    );
                    gpu.draw(&m.treat, &model, WHITE, [1.1, 1.08, 1.0], 0.6, true);
                }
                DropKind::Weapon(tool) => {
                    let index = Tool::ALL.iter().position(|t| *t == tool).unwrap_or(0);
                    let model = Mat4::from_translation(Vec3::new(d.at.x, 0.9 + bob, d.at.z))
                        * Mat4::from_quat(turn)
                        * Mat4::from_rotation_x(-0.6);
                    gpu.draw(&m.tools[index], &model, WHITE, [1.15, 1.1, 1.0], 0.8, true);
                }
            }
            let a = self.time * 3.0 + i as f32;
            let sparkle = Mat4::from_scale_rotation_translation(
                Vec3::splat(0.1),
                Quat::IDENTITY,
                Vec3::new(d.at.x + a.cos() * 0.5, 1.0 + bob, d.at.z + a.sin() * 0.5),
            );
            gpu.draw_flat(&m.sparkle, &sparkle, [1.0, 1.0, 0.85]);
        }
        // He melted: a puddle of vanilla where he stood.
        if game.man_melted {
            let at = game.man.pos;
            let model = Mat4::from_scale_rotation_translation(
                Vec3::new(1.6, 1.0, 1.3),
                Quat::IDENTITY,
                Vec3::new(at.x, 0.02, at.z),
            );
            gpu.draw(&m.puddle, &model, WHITE, light_at(at), 0.8, false);
        }

        // The floor's splats and drips.
        for p in &self.particles {
            let fade = 1.0 - (p.age / p.life).powi(3);
            match p.kind {
                ParticleKind::Splat | ParticleKind::Drip => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::new(p.size * fade, 1.0, p.size * fade),
                        Quat::IDENTITY,
                        p.pos,
                    );
                    let mesh = if p.kind == ParticleKind::Splat {
                        &m.splat
                    } else {
                        &m.drip
                    };
                    gpu.draw(
                        mesh,
                        &model,
                        WHITE,
                        light_at(v(p.pos.x, p.pos.z)),
                        0.4,
                        false,
                    );
                }
                ParticleKind::Blob => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(p.size),
                        Quat::IDENTITY,
                        p.pos,
                    );
                    gpu.draw(&m.fudge, &model, WHITE, WHITE, 0.8, false);
                }
                ParticleKind::Beam => {
                    // One super long sprinkle, stretched from the staff.
                    let length = p.vel.length().max(0.1);
                    let mid = p.pos + p.vel * 0.5;
                    let thick = 1.6 * (1.0 - p.age / p.life).max(0.2);
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::new(thick, thick, length / 0.26),
                        Quat::from_rotation_y(p.vel.x.atan2(p.vel.z)),
                        mid,
                    );
                    gpu.draw(&m.sprinkles[0], &model, WHITE, [1.3, 1.2, 1.25], 0.6, false);
                }
                ParticleKind::Puff => {
                    // A ring of sugar puffs swelling out to the burst's
                    // size, then shrinking away.
                    let grow = (p.age / p.life).min(1.0);
                    let fade = (1.0 - grow).max(0.0).sqrt();
                    for i in 0..9 {
                        let a = i as f32 / 9.0 * 2.0 * PI;
                        let r = p.size * (0.3 + grow * 0.7);
                        let at = p.pos
                            + Vec3::new(
                                a.cos() * r,
                                ((i * 7) % 3) as f32 * 0.25 - 0.3,
                                a.sin() * r,
                            );
                        let model = Mat4::from_scale_rotation_translation(
                            Vec3::splat(0.55 * fade + 0.1),
                            Quat::IDENTITY,
                            at,
                        );
                        gpu.draw(&m.sugar, &model, WHITE, [1.2, 1.2, 1.2], 0.2, false);
                    }
                }
            }
        }

        self.draw_man();
        self.draw_minions();
        self.draw_avatar(&view, scooped);
        self.draw_held(&view);

        // Shots in the air.
        for shot in &game.shots {
            let at = Vec3::new(shot.pos.x, shot.height, shot.pos.z);
            match shot.kind {
                ShotKind::Fudge => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.26),
                        Quat::IDENTITY,
                        at,
                    );
                    gpu.draw(&m.fudge, &model, WHITE, WHITE, 0.9, true);
                }
                ShotKind::Cone => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(1.3),
                        Quat::from_rotation_y(yaw_of(shot.vel)),
                        at,
                    );
                    gpu.draw(&m.cone_shot, &model, WHITE, WHITE, 0.5, true);
                }
                ShotKind::Sugar => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.34),
                        Quat::IDENTITY,
                        at,
                    );
                    gpu.draw(&m.sugar, &model, WHITE, [1.2, 1.2, 1.2], 0.9, true);
                }
                ShotKind::Sprinkle(colour) => {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(1.4),
                        Quat::from_rotation_y(yaw_of(shot.vel)),
                        at,
                    );
                    gpu.draw(
                        &m.sprinkles[colour as usize % 5],
                        &model,
                        WHITE,
                        WHITE,
                        0.3,
                        true,
                    );
                }
            }
        }
        // Fart clouds: puffs that swell, then shrink away.
        for (n, cloud) in game.clouds.iter().enumerate() {
            let life = game.tuning.cloud_life;
            let grow = (cloud.age / 0.35).min(1.0);
            let fade = ((life - cloud.age) / 0.6).clamp(0.0, 1.0);
            for i in 0..8 {
                let a = i as f32 / 8.0 * 2.0 * PI + n as f32;
                let r = 1.2 * grow + (self.time * 1.5 + i as f32).sin() * 0.1;
                let size = (0.9 + 0.35 * ((i * 5) % 3) as f32) * grow * fade;
                let at = Vec3::new(
                    cloud.at.x + a.cos() * r,
                    0.7 + ((i * 3) % 4) as f32 * 0.3 + cloud.age * 0.2,
                    cloud.at.z + a.sin() * r,
                );
                let model =
                    Mat4::from_scale_rotation_translation(Vec3::splat(size), Quat::IDENTITY, at);
                gpu.draw(&m.cloud, &model, WHITE, WHITE, 0.0, false);
            }
        }
    }

    fn draw_man(&self) {
        let game = &self.game;
        if !game.man_here() {
            return;
        }
        let gpu = &self.gpu;
        let m = &self.meshes;
        let man = &game.man;
        // He is the one detailed model: lit so he always reads.
        let room = self.room_light(icecream_rules::layout::index(cell_of(man.pos)));
        let light = room.map(|c| c.max(0.97));
        let frozen = man.state == ManState::Frozen;
        let bob = if man.moving {
            (self.man_stride * 2.0).sin().abs() * 0.1
        } else {
            0.0
        };
        let shake = if frozen {
            (self.time * 40.0).sin() * 0.02
        } else {
            0.0
        };
        let squash = 1.0 + self.man_hit * 0.4;
        let base = Mat4::from_translation(Vec3::new(man.pos.x + shake, bob, man.pos.z))
            * Mat4::from_rotation_y(self.man_yaw)
            * Mat4::from_scale(Vec3::new(squash, 1.0 / squash, squash));
        let (tint, shine) = if frozen {
            ([0.62, 0.42, 0.3], 1.3)
        } else if man.slow > 0.0 {
            ([0.92, 0.8, 0.7], 0.6)
        } else {
            (WHITE, 0.0)
        };
        let swing = if man.moving && !frozen {
            (self.man_stride * 2.0).sin() * 0.6
        } else {
            0.0
        };
        for side in [-1.0_f32, 1.0] {
            let leg = base
                * Mat4::from_translation(Vec3::new(side * 0.17, 0.85, 0.0))
                * Mat4::from_rotation_x(swing * side);
            gpu.draw(&m.man_leg, &leg, tint, light, shine, true);
            let reach = if man.state == ManState::Chase && !frozen {
                -1.35 + (self.time * 9.0 + side).sin() * 0.15
            } else {
                -swing * side * 0.7
            };
            let arm = base
                * Mat4::from_translation(Vec3::new(side * 0.62, 1.85, 0.0))
                * Mat4::from_rotation_z(side * 0.25)
                * Mat4::from_rotation_x(reach);
            gpu.draw(&m.man_arm, &arm, tint, light, shine, true);
        }
        gpu.draw(&m.man_body, &base, tint, light, shine, true);
        gpu.draw(&m.man_head, &base, tint, light, shine, true);
        // The forehead always shines.
        gpu.draw(&m.man_forehead, &base, tint, light, 1.0 + shine * 0.5, true);
        if man.state == ManState::Confused {
            for i in 0..3 {
                let a = self.time * 4.0 + i as f32 * 2.1;
                let star = Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.22),
                    Quat::from_rotation_y(a * 2.0),
                    Vec3::new(man.pos.x + a.cos() * 0.7, 3.5, man.pos.z + a.sin() * 0.7),
                );
                gpu.draw_flat(&m.star, &star, [1.0, 0.88, 0.3]);
            }
        }
    }

    /// The minions: little Ice Cream Men. His are vanilla; yours are
    /// mint and glow.
    fn draw_minions(&self) {
        let gpu = &self.gpu;
        let m = &self.meshes;
        for (i, minion) in self.game.minions.iter().enumerate() {
            let room = self.room_light(icecream_rules::layout::index(cell_of(minion.pos)));
            let light = room.map(|c| c.max(0.95));
            let stride = self.minion_stride + i as f32 * 1.7;
            let bob = if minion.moving {
                (stride * 2.0).sin().abs() * 0.06
            } else {
                0.0
            };
            let scale = 0.34;
            let base = Mat4::from_translation(Vec3::new(minion.pos.x, bob, minion.pos.z))
                * Mat4::from_rotation_y(yaw_of(minion.facing))
                * Mat4::from_scale(Vec3::splat(scale));
            let tint = if minion.flash > 0.0 {
                [1.0, 0.5, 0.5]
            } else if minion.friendly {
                [0.7, 1.05, 0.85]
            } else {
                WHITE
            };
            let swing = if minion.moving {
                (stride * 2.0).sin() * 0.7
            } else {
                0.0
            };
            for side in [-1.0_f32, 1.0] {
                let leg = base
                    * Mat4::from_translation(Vec3::new(side * 0.17, 0.85, 0.0))
                    * Mat4::from_rotation_x(swing * side);
                gpu.draw(&m.man_leg, &leg, tint, light, 0.0, true);
                let arm = base
                    * Mat4::from_translation(Vec3::new(side * 0.62, 1.85, 0.0))
                    * Mat4::from_rotation_z(side * 0.25)
                    * Mat4::from_rotation_x(-1.2 + (stride * 3.0 + side).sin() * 0.2);
                gpu.draw(&m.man_arm, &arm, tint, light, 0.0, true);
            }
            gpu.draw(&m.man_body, &base, tint, light, 0.0, true);
            gpu.draw(&m.man_head, &base, tint, light, 0.0, true);
            gpu.draw(&m.man_forehead, &base, tint, light, 1.0, true);
            if minion.friendly {
                let glow = Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.1),
                    Quat::IDENTITY,
                    Vec3::new(minion.pos.x, 1.35, minion.pos.z),
                );
                gpu.draw_flat(&m.sparkle, &glow, [0.6, 1.0, 0.75]);
            }
        }
    }

    /// In first person: the tool in your hand at the bottom right of the
    /// view, jabbing when you swing.
    fn draw_held(&self, view: &View) {
        let game = &self.game;
        if !view.first_person || game.player.hidden.is_some() || self.phase != Phase::Playing {
            return;
        }
        let jab = (self.swing / 0.25).clamp(0.0, 1.0);
        let jab = (jab * PI).sin();
        let model = Mat4::from_translation(view.eye)
            * Mat4::from_rotation_y(self.camera.yaw)
            * Mat4::from_rotation_x(self.camera.pitch)
            * Mat4::from_translation(Vec3::new(-0.3, -0.3, 0.5 + jab * 0.35))
            * Mat4::from_rotation_x(-0.12 - jab * 0.3)
            * Mat4::from_rotation_y(0.16)
            * Mat4::from_scale(Vec3::splat(0.42));
        let index = Tool::ALL
            .iter()
            .position(|t| *t == game.player.tool)
            .unwrap_or(0);
        let room = self.room_light(icecream_rules::layout::index(cell_of(game.player.pos)));
        self.gpu.draw(
            &self.meshes.tools[index],
            &model,
            WHITE,
            room.map(|c| c.max(0.9)),
            0.4,
            true,
        );
    }

    fn draw_avatar(&self, view: &View, scooped: bool) {
        let game = &self.game;
        let player = &game.player;
        if player.hidden.is_some() || view.first_person {
            return;
        }
        let gpu = &self.gpu;
        let m = &self.meshes;
        let room = self.room_light(icecream_rules::layout::index(cell_of(player.pos)));
        let light = room.map(|c| c.max(0.97));
        let mut sink = 0.0;
        let mut shrink = 1.0;
        let ending = match self.phase {
            Phase::Ending(t) => Some(t),
            Phase::Over => Some(ENDING),
            _ => None,
        };
        if let (Some(t), true) = (ending, scooped) {
            let t = (t / ENDING).min(1.0);
            sink = t * 0.9;
            shrink = 1.0 - t * 0.45;
            let cone = Mat4::from_scale_rotation_translation(
                Vec3::splat(1.2),
                Quat::IDENTITY,
                Vec3::new(player.pos.x, 0.0, player.pos.z),
            );
            gpu.draw(&m.cone, &cone, WHITE, light, 0.3, true);
        }
        let flash = if self.hurt > 0.3 && (self.time * 30.0).sin() > 0.0 {
            [1.0, 0.6, 0.6]
        } else {
            WHITE
        };
        let base = Mat4::from_translation(Vec3::new(
            player.pos.x,
            player.height + 0.4 - sink - 0.4 * shrink.min(1.0) + 0.4 * shrink,
            player.pos.z,
        )) * Mat4::from_rotation_y(self.yaw)
            * Mat4::from_scale(Vec3::splat(shrink));
        let swing = self.stride.sin() * 0.7 * if player.moving { 1.0 } else { 0.0 };
        let air = player.height > 0.05;
        gpu.draw(&m.torso, &base, flash, light, 0.0, true);
        gpu.draw(&m.head, &base, flash, light, 0.0, true);
        for side in [-1.0_f32, 1.0] {
            let leg_swing = if air { 0.4 * side } else { swing * side };
            let leg = base
                * Mat4::from_translation(Vec3::new(side * 0.2, 0.8, 0.0))
                * Mat4::from_rotation_x(leg_swing);
            gpu.draw(&m.leg, &leg, flash, light, 0.0, true);
            // The right arm (x < 0 facing +z) holds the tool out.
            let holding = side < 0.0;
            let angle = if holding {
                -FRAC_PI_2 - (self.swing / 0.25).clamp(0.0, 1.0) * 0.9
            } else if air {
                -2.6
            } else {
                -swing * side
            };
            let arm = base
                * Mat4::from_translation(Vec3::new(side * 0.53, 1.5, 0.0))
                * Mat4::from_rotation_x(angle);
            gpu.draw(&m.arm, &arm, flash, light, 0.0, true);
        }
        let tool = base * Mat4::from_translation(Vec3::new(-0.53, 1.45, 0.7));
        let index = Tool::ALL
            .iter()
            .position(|t| *t == player.tool)
            .unwrap_or(0);
        gpu.draw(&m.tools[index], &tool, WHITE, light, 0.4, true);
    }

    fn tour_card(&self, tour: Tour) -> Card {
        let (title, lines) = tour.words(self.touch);
        let mut buttons = Vec::new();
        if !tour.is_first() {
            buttons.push((copy::TOUR_BACK.to_owned(), Action::TourBack, false));
        }
        if tour.is_last() {
            buttons.push((copy::TOUR_DONE.to_owned(), Action::TourEnd(true), true));
        } else {
            buttons.push((copy::TOUR_SKIP.to_owned(), Action::TourEnd(false), false));
            buttons.push((copy::TOUR_NEXT.to_owned(), Action::TourNext, true));
        }
        let mut all = vec![copy::tour_step_line(tour.step + 1, tutorial::STEPS)];
        all.extend(lines.iter().map(|l| (*l).to_owned()));
        Card {
            title: title.to_owned(),
            lines: all,
            buttons,
            links: vec![(copy::GUIDE.to_owned(), tutorial::GUIDE.to_owned())],
            ..Card::default()
        }
    }

    fn card(&self) -> Option<Card> {
        if let Some(tour) = self.tour {
            return Some(self.tour_card(tour));
        }
        let links = vec![
            (copy::HOW_TO_PLAY.to_owned(), tutorial::GUIDE.to_owned()),
            (copy::STUDIO.to_owned(), tutorial::STUDIO.to_owned()),
        ];
        match self.phase {
            Phase::Title => Some(Card {
                title: self.options.word("title").unwrap_or(copy::TITLE).to_owned(),
                lines: vec![
                    self.options
                        .word("tagline")
                        .unwrap_or(copy::TAGLINE)
                        .to_owned(),
                ],
                note: Some(copy::NOTE.to_owned()),
                buttons: vec![
                    (copy::HOW_TO_PLAY.to_owned(), Action::Tour, false),
                    (copy::PLAY.to_owned(), Action::Play, true),
                ],
                links: vec![(copy::STUDIO.to_owned(), tutorial::STUDIO.to_owned())],
                ..Card::default()
            }),
            Phase::Menu => Some(Card {
                title: copy::PAUSED.to_owned(),
                buttons: vec![
                    (copy::HOW_TO_PLAY.to_owned(), Action::Tour, false),
                    (copy::NEW_HOUSE.to_owned(), Action::NewHouse, false),
                    (copy::RESUME.to_owned(), Action::Resume, true),
                ],
                links,
                ..Card::default()
            }),
            Phase::Over => {
                let run = self.game.run_card();
                let won = run.escaped || self.game.status == Status::Melted;
                let mut buttons = Vec::new();
                if won && self.game.tuning.free_roam == 1 {
                    buttons.push((copy::KEEP_PLAYING.to_owned(), Action::KeepPlaying, true));
                }
                let (title, line, again) = if self.game.status == Status::Melted {
                    (copy::MELTED, copy::MELTED_LINE, copy::PLAY_AGAIN)
                } else if run.escaped {
                    (
                        copy::ESCAPED,
                        self.options
                            .word("escaped_line")
                            .unwrap_or(copy::ESCAPED_LINE),
                        copy::PLAY_AGAIN,
                    )
                } else {
                    (
                        copy::SCOOPED,
                        self.options
                            .word("scooped_line")
                            .unwrap_or(copy::SCOOPED_LINE),
                        copy::TRY_AGAIN,
                    )
                };
                let mut lines = vec![line.to_owned()];
                lines.extend(copy::card_lines(&run));
                let main = buttons.is_empty();
                buttons.insert(0, (again.to_owned(), Action::NewHouse, main));
                Some(Card {
                    title: title.to_owned(),
                    lines,
                    buttons,
                    links,
                    sunny: won,
                    scooped: !won,
                    ..Card::default()
                })
            }
            Phase::Playing | Phase::Ending(_) => None,
        }
    }

    fn update_hud(&mut self, now: f64) {
        let game = &self.game;
        let player = &game.player;
        let playing = matches!(self.phase, Phase::Playing | Phase::Menu | Phase::Ending(_));
        self.hud.set_playing(playing);
        // The bar is a share of full health, whatever full is in this version.
        self.hud.health(
            player.health * 100.0 / game.tuning.max_health.max(1.0),
            player.boost > 0.0,
        );
        self.hud.keys(player.keys);
        let (fudge, farts, keys, cones, sugar) = (
            player.fudge,
            player.farts,
            player.keys,
            player.cones,
            player.sugar,
        );
        let room = self.canvas.client_width() as f32;
        self.hud
            .hot_bar(player.tool, &player.owned, room, |tool| match tool {
                Tool::Fudge => fudge.to_string(),
                Tool::Fart => farts.to_string(),
                Tool::Keys => format!("{keys}/3"),
                Tool::Pistol => cones.to_string(),
                Tool::Sugar => sugar.to_string(),
                Tool::Sword | Tool::Spear | Tool::Staff => String::new(),
            });
        let boss =
            (game.weapons_on() && game.man_here() && game.man.health < game.tuning.man_health)
                .then(|| game.man.health / game.tuning.man_health.max(1.0));
        self.hud.boss(boss);
        self.hud
            .crosshair(self.phase == Phase::Playing && self.camera.aims());
        let prompt = if self.phase == Phase::Playing {
            game.prompt()
        } else {
            None
        };
        let unlock = (prompt == Some(Prompt::Unlock) && player.unlock > 0.0)
            .then(|| player.unlock / game.tuning.unlock.max(1e-3));
        let text = match (prompt, unlock) {
            (Some(Prompt::Unlock), Some(_)) => Some(copy::UNLOCKING),
            (Some(p), _) => Some(copy::prompt(p)),
            (None, _) => None,
        };
        self.hud.prompt(text, unlock, self.touch);
        self.hud.sprint(self.sprint_toggle);
        let hidden = player.hidden.is_some() && self.phase == Phase::Playing;
        self.hud.tick(now, self.hurt, hidden, self.daylight);
        let card = self.card();
        self.hud.card(card);
        // The truck's jingle: louder as he gets closer.
        let closeness = if self.phase == Phase::Playing && self.tour.is_none() && game.man_here() {
            (1.0 - game.man.pos.distance(player.pos) / 30.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.sound
            .truck(closeness, game.man.state == ManState::Chase);
    }

    /// The run's state on the canvas's data attributes, for browser
    /// checks.
    fn publish(&self) {
        let game = &self.game;
        let phase = match self.phase {
            Phase::Title => "title",
            Phase::Playing => "playing",
            Phase::Menu => "menu",
            Phase::Ending(_) => "ending",
            Phase::Over => "over",
        };
        let status = match game.status {
            Status::Playing => "playing",
            Status::Escaped => "escaped",
            Status::Melted => "melted",
            Status::Scooped => "scooped",
        };
        let state = format!("{:?}", game.man.state).to_lowercase();
        for (name, value) in [
            ("data-phase", phase.to_owned()),
            ("data-status", status.to_owned()),
            ("data-seed", self.seed.to_string()),
            ("data-tour", self.tour.is_some().to_string()),
            (
                "data-player",
                format!("{:.2},{:.2}", game.player.pos.x, game.player.pos.z),
            ),
            (
                "data-man",
                format!("{:.2},{:.2}", game.man.pos.x, game.man.pos.z),
            ),
            ("data-man-state", state),
            ("data-keys", game.player.keys.to_string()),
            ("data-health", format!("{:.0}", game.player.health)),
            ("data-fudge-hits", game.stats.fudge_hits.to_string()),
            (
                "data-tool",
                format!("{:?}", game.player.tool).to_lowercase(),
            ),
            ("data-shift-lock", self.camera.shift_lock.to_string()),
            ("data-hidden", game.player.hidden.is_some().to_string()),
            ("data-clouds", game.clouds.len().to_string()),
            ("data-spotted", self.spotted.to_string()),
            (
                "data-minions",
                game.minions
                    .iter()
                    .filter(|m| !m.friendly)
                    .count()
                    .to_string(),
            ),
            (
                "data-friends",
                game.minions
                    .iter()
                    .filter(|m| m.friendly)
                    .count()
                    .to_string(),
            ),
            (
                "data-tools",
                game.player
                    .owned
                    .iter()
                    .map(|t| format!("{t:?}").to_lowercase())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            ("data-boost", format!("{:.1}", game.player.boost)),
            (
                "data-minion-near",
                format!(
                    "{:.1}",
                    game.minions
                        .iter()
                        .filter(|m| !m.friendly)
                        .map(|m| m.pos.distance(game.player.pos))
                        .fold(999.0_f32, f32::min)
                ),
            ),
            ("data-man-health", format!("{:.0}", game.man.health)),
            ("data-free-roam", game.free_roam.to_string()),
            ("data-lit", game.lit.to_string()),
            ("data-drops", game.drops.len().to_string()),
            (
                "data-camera",
                if self.camera.first_person() {
                    "first".to_owned()
                } else {
                    "third".to_owned()
                },
            ),
            // Game Lab's "note at this time and place".
            ("data-seconds", format!("{:.1}", game.seconds())),
            (
                "data-room",
                game.house.rooms[icecream_rules::layout::index(icecream_rules::layout::cell_of(
                    game.player.pos,
                ))]
                .module
                .name()
                .to_owned(),
            ),
            (
                "data-prompt",
                game.prompt()
                    .map_or("none".to_owned(), |p| format!("{p:?}").to_lowercase()),
            ),
        ] {
            let _ = self.canvas.set_attribute(name, &value);
        }
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
            ("background", "#fff6ec"),
            ("color", "#3a2418"),
            ("border", "3px solid #3a2418"),
            ("border-radius", "14px"),
        ],
    );
    let _ = parent.append_child(&note);
}

/// The key as the game keeps it: lower case, so Shift doesn't change W.
fn key_name(event: &KeyboardEvent) -> String {
    event.key().to_lowercase()
}

pub fn start() {
    let window = web_sys::window().expect("a window");
    let document = window.document().expect("a document");
    let canvas: HtmlCanvasElement = match document.get_element_by_id("icecream-canvas") {
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
                    ("background", "#4d5575"),
                ],
            );
            let _ = made.set_attribute("id", "icecream-canvas");
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
    if let Some(status) = document.get_element_by_id("icecream-status") {
        status.set_text_content(Some(""));
    }
    let context_options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&context_options, &"antialias".into(), &true.into());
    let gl = canvas
        .get_context_with_context_options("webgl2", &context_options)
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into::<Gl>().ok());
    let Some(gl) = gl else {
        fail(&document, &parent, copy::NO_WEBGL);
        return;
    };
    let mut gpu = match Gpu::new(gl) {
        Ok(gpu) => gpu,
        Err(error) => {
            web_sys::console::error_1(&error.into());
            fail(&document, &parent, copy::NO_WEBGL);
            return;
        }
    };
    let avatar = scene::avatar();
    let man = scene::man();
    let meshes = Meshes {
        rooms: Vec::new(),
        lights: Vec::new(),
        torso: gpu.upload(&avatar.torso),
        head: gpu.upload(&avatar.head),
        arm: gpu.upload(&avatar.arm),
        leg: gpu.upload(&avatar.leg),
        tools: Tool::ALL
            .iter()
            .map(|t| gpu.upload(&scene::tool(*t)))
            .collect(),
        man_body: gpu.upload(&man.body),
        man_leg: gpu.upload(&man.leg),
        man_arm: gpu.upload(&man.arm),
        man_head: gpu.upload(&man.head),
        man_forehead: gpu.upload(&man.forehead),
        key: gpu.upload(&scene::key()),
        fudge: gpu.upload(&scene::ball(scene::FUDGE)),
        cloud: gpu.upload(&scene::ball(scene::CLOUD)),
        sparkle: gpu.upload(&scene::ball(0xFFFFFF)),
        star: gpu.upload(&scene::sprinkle(scene::GOLD)),
        sprinkles: scene::SPRINKLES
            .iter()
            .map(|c| gpu.upload(&scene::sprinkle(*c)))
            .collect(),
        splat: gpu.upload(&scene::splat(scene::FUDGE)),
        drip: gpu.upload(&scene::splat(scene::VANILLA)),
        cone: gpu.upload(&scene::big_cone()),
        treat: gpu.upload(&scene::treat()),
        cone_shot: gpu.upload(&scene::cone_shot()),
        sugar: gpu.upload(&scene::ball(0xFFFFFF)),
        buttons: scene::BUTTON_COLOURS
            .iter()
            .map(|c| gpu.upload(&scene::button(*c)))
            .collect(),
        cabinet: gpu.upload(&scene::cabinet()),
        puddle: gpu.upload(&scene::splat(scene::VANILLA)),
    };
    let _ = rgb(0);
    let touch = window
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let hud = Hud::new(&document, &parent, touch);
    let options = Options::parse(&window.location().hash().unwrap_or_default());
    let seed = options.seed.unwrap_or(1);
    let mut app = App {
        window: window.clone(),
        canvas: canvas.clone(),
        gpu,
        meshes,
        hud,
        sound: Sound::new(),
        game: Game::tuned(seed, RuleOptions { man: !options.calm }, options.tuning()),
        options,
        bot: None,
        seed,
        phase: Phase::Title,
        tour: None,
        touch,
        camera: Camera::default(),
        held: Vec::new(),
        taps: Vec::new(),
        select: None,
        fire: false,
        sprint_toggle: false,
        touch_jump: false,
        touch_fire: false,
        touch_use: false,
        orbit: None,
        mouse: None,
        stick: None,
        look: None,
        last: 0.0,
        carry: 0.0,
        time: 0.0,
        hurt: 0.0,
        daylight: 0.0,
        yaw: 0.0,
        man_yaw: 0.0,
        stride: 0.0,
        man_stride: 0.0,
        drip_wait: 0.0,
        man_hit: 0.0,
        particles: Vec::new(),
        view: None,
        frames: 0,
        spotted: 0,
        swing: 0.0,
        minion_stride: 0.0,
    };
    app.new_run();
    if app.options.skip {
        app.phase = Phase::Playing;
    } else if tutorial::opens_by_itself(tutorial_seen(&window).as_deref(), false) {
        app.tour = Some(Tour::new());
    }
    let app = Rc::new(RefCell::new(app));

    {
        let app = app.clone();
        listen::<KeyboardEvent>(window.as_ref(), "keydown", move |event| {
            let key = key_name(&event);
            let mut app = app.borrow_mut();
            app.sound.wake();
            if app.tour.is_some() {
                let action = match key.as_str() {
                    "escape" => Some(Action::TourEnd(false)),
                    "arrowright" | "enter" => Some(Action::TourNext),
                    "arrowleft" => Some(Action::TourBack),
                    _ => None,
                };
                if let Some(action) = action {
                    event.prevent_default();
                    app.act(action);
                }
                return;
            }
            match key.as_str() {
                "?" => {
                    event.prevent_default();
                    app.act(Action::Tour);
                    return;
                }
                "escape" => {
                    event.prevent_default();
                    app.act(Action::Menu);
                    return;
                }
                _ => {}
            }
            if app.phase != Phase::Playing {
                return;
            }
            match key.as_str() {
                "control" => {
                    event.prevent_default();
                    if !event.repeat() {
                        let on = !app.camera.shift_lock;
                        app.set_shift_lock(on);
                    }
                }
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" => {
                    let index = key.parse::<usize>().unwrap_or(1) - 1;
                    app.select = app.game.player.owned.get(index).copied();
                }
                "v" => {
                    event.prevent_default();
                    if !event.repeat() {
                        app.toggle_view();
                    }
                }
                "w" | "a" | "s" | "d" | "arrowup" | "arrowdown" | "arrowleft" | "arrowright"
                | " " | "shift" | "e" => {
                    event.prevent_default();
                    if !app.held.contains(&key) {
                        app.held.push(key.clone());
                    }
                    app.taps.push(key);
                }
                _ => {}
            }
        });
    }
    {
        let app = app.clone();
        listen::<KeyboardEvent>(window.as_ref(), "keyup", move |event| {
            let key = key_name(&event);
            let mut app = app.borrow_mut();
            app.held.retain(|k| *k != key);
            // Shift held changes letters' case on some systems: let go of
            // both.
            if key == "shift" {
                app.held.retain(|k| k.len() > 1 || k == " ");
            }
        });
    }
    {
        let app = app.clone();
        listen::<web_sys::Event>(window.as_ref(), "blur", move |_| {
            app.borrow_mut().release();
        });
    }
    listen::<web_sys::Event>(canvas.as_ref(), "contextmenu", |event| {
        event.prevent_default()
    });
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerdown", move |event| {
            let mut app = app.borrow_mut();
            app.sound.wake();
            let at = (event.client_x() as f32, event.client_y() as f32);
            if event.pointer_type() == "touch" {
                let width = app.canvas.client_width() as f32;
                if at.0 < width * 0.45 && app.stick.is_none() {
                    app.stick = Some((event.pointer_id(), at, (0.0, 0.0)));
                } else if app.look.is_none() {
                    app.look = Some((event.pointer_id(), at));
                }
                return;
            }
            let _ = app.canvas.focus();
            match event.button() {
                0 => {
                    app.fire = true;
                    app.taps.push("click".to_owned());
                    if app.camera.aims() {
                        let locked = app
                            .window
                            .document()
                            .and_then(|d| d.pointer_lock_element())
                            .is_some();
                        if !locked {
                            app.canvas.request_pointer_lock();
                        }
                    }
                }
                2 => app.orbit = Some(at),
                _ => {}
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointermove", move |event| {
            let mut app = app.borrow_mut();
            let at = (event.client_x() as f32, event.client_y() as f32);
            if event.pointer_type() == "touch" {
                if let Some((id, start, _)) = app.stick
                    && id == event.pointer_id()
                {
                    let (dx, dy) = (at.0 - start.0, at.1 - start.1);
                    let length = (dx * dx + dy * dy).sqrt().max(1.0);
                    let scale = (50.0 / length).min(1.0);
                    let offset = (dx * scale, dy * scale);
                    app.stick = Some((id, start, offset));
                    app.hud.stick(offset.0, offset.1);
                } else if let Some((id, last)) = app.look
                    && id == event.pointer_id()
                {
                    app.camera
                        .orbit((at.0 - last.0) * 1.3, (at.1 - last.1) * 1.3);
                    app.look = Some((id, at));
                }
                return;
            }
            app.mouse = Some(at);
            let locked = app
                .window
                .document()
                .and_then(|d| d.pointer_lock_element())
                .is_some();
            if locked {
                app.camera
                    .orbit(event.movement_x() as f32, event.movement_y() as f32);
            } else if let Some(last) = app.orbit {
                app.camera.orbit(at.0 - last.0, at.1 - last.1);
                app.orbit = Some(at);
            }
        });
    }
    for name in ["pointerup", "pointercancel"] {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), name, move |event| {
            let mut app = app.borrow_mut();
            if event.pointer_type() == "touch" {
                if app.stick.is_some_and(|(id, _, _)| id == event.pointer_id()) {
                    app.stick = None;
                    app.hud.stick(0.0, 0.0);
                }
                if app.look.is_some_and(|(id, _)| id == event.pointer_id()) {
                    app.look = None;
                }
                return;
            }
            match event.button() {
                0 => app.fire = false,
                2 => app.orbit = None,
                _ => {
                    app.fire = false;
                    app.orbit = None;
                }
            }
        });
    }
    {
        let app = app.clone();
        let closure = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();
            app.borrow_mut().camera.wheel(event.delta_y() as f32);
        });
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(false);
        let _ = canvas.add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            closure.as_ref().unchecked_ref(),
            &options,
        );
        closure.forget();
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
    let _ = centre((0, 0));
}

/// The animation-frame callback, kept so it can ask for the next frame.
type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

#[allow(dead_code)]
const _STEPS_A_SECOND: u32 = HZ;
