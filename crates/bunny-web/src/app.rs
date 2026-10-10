//! The browser side: WebGL2 drawing, input, the HUD, and storage.

use std::cell::RefCell;
use std::rc::Rc;

use bunny_rules::game::Move;
use bunny_rules::{
    EdibleKind, Event, FarmerState, Game, HZ, Input, ObstacleKind, PowerKind, Status, TIER_HEIGHT,
    TIER_JUMP, UNIT, level, shade,
};
use glam::{Mat4, Quat, Vec2, Vec3};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, HtmlCanvasElement, KeyboardEvent, PointerEvent, WebGl2RenderingContext as Gl,
    WebGlBuffer, WebGlProgram, WebGlUniformLocation, WebGlVertexArrayObject, Window,
};

use crate::hud::{Action, Card, Hud, css, element, show};
use crate::kit::{self, Piece};
use crate::look::{Options, Tier};
use crate::meadow::{self, Spot, Walker};
use crate::mesh::{Mesh, STRIDE, rgb};
use crate::{copy, scene};
use bunny_rules::progress::Progress;

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
    /// The bunny at each size tier.
    bunny: Vec<GpuMesh>,
    ear: GpuMesh,
    farmer: GpuMesh,
    leg: GpuMesh,
    net: GpuMesh,
    alarm: GpuMesh,
    shadow: GpuMesh,
    crumb: GpuMesh,
    dot: GpuMesh,
    obstacles: Vec<(ObstacleKind, GpuMesh)>,
    powers: Vec<(PowerKind, GpuMesh)>,
    pieces: Vec<(Piece, GpuMesh)>,
    meadow: GpuMesh,
    /// A ladder stone, gray so a tint colours it.
    stone: GpuMesh,
}

impl Meshes {
    fn obstacle(&self, kind: ObstacleKind) -> &GpuMesh {
        pick(&self.obstacles, kind)
    }

    fn power(&self, kind: PowerKind) -> &GpuMesh {
        pick(&self.powers, kind)
    }

    fn edible(&self, kind: EdibleKind) -> &GpuMesh {
        pick(&self.edibles, kind)
    }
}

fn pick<K: PartialEq>(meshes: &[(K, GpuMesh)], kind: K) -> &GpuMesh {
    meshes
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(&meshes[0].1, |(_, mesh)| mesh)
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
    /// Hopping around Warren Meadow; `panel` may show a spot's card.
    Meadow,
    Playing,
    Paused,
    Over,
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
    /// A touch screen: swipes and the turn-back button.
    coarse: bool,
    game: Game,
    /// The garden being played, 1-based.
    garden: usize,
    /// Runs started, for each run's seed.
    runs: u64,
    phase: Phase,
    progress: Progress,
    walker: Walker,
    /// The meadow spot whose card is open.
    panel: Option<Spot>,
    /// Keys held for walking in the meadow: forward, back, left, right.
    held: [bool; 4],
    /// A drag on a touch screen, as a stick: start and current point.
    drag: Option<((f32, f32), (f32, f32))>,
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

fn load_progress(window: &Window) -> Progress {
    window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(SAVE_KEY).ok().flatten())
        .map(|text| Progress::decode(&text))
        .unwrap_or_default()
}

fn save_progress(window: &Window, progress: &Progress) {
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(SAVE_KEY, &progress.encode());
    }
}

impl App {
    fn fur(&self) -> u32 {
        shade::fur(self.progress.wins)
    }

    fn rebuild_bunny(&mut self) {
        let fur = self.fur();
        for (tier, mesh) in self.meshes.bunny.iter_mut().enumerate() {
            self.gpu.refill(mesh, &scene::bunny(fur, tier as u8));
        }
        self.gpu.refill(&mut self.meshes.ear, &scene::ear(fur));
    }

    fn new_run(&mut self, garden: usize) {
        let garden = garden.clamp(1, level::COUNT);
        if garden != self.garden {
            let g = level::garden(garden);
            self.gpu.refill(&mut self.meshes.ground, &scene::ground(&g));
            self.gpu.refill(&mut self.meshes.hedges, &scene::hedges(&g));
            self.garden = garden;
        }
        self.runs += 1;
        let seed = (js_sys::Math::random() * 1e15) as u64 ^ self.runs;
        self.game = Game::with_seed(level::garden(garden), seed, self.progress.gentle);
        if self.options.quiet {
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

    fn play(&mut self, garden: usize) {
        self.new_run(garden);
        self.phase = Phase::Playing;
        let _ = self.canvas.focus();
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Play(garden) => self.play(garden),
            Action::Restart => self.play(self.garden),
            Action::Pause if self.phase == Phase::Playing => self.phase = Phase::Paused,
            Action::Resume if self.phase == Phase::Paused => {
                self.phase = Phase::Playing;
                self.last = 0.0;
                let _ = self.canvas.focus();
            }
            Action::Leave => {
                self.game.step(&[Input::Leave]);
                self.enter_meadow();
            }
            Action::TurnBack => self.press(Input::Back),
            Action::Meadow => {
                if self.phase == Phase::Title {
                    self.walker = Walker::default();
                    self.phase = Phase::Meadow;
                    let _ = self.canvas.focus();
                } else {
                    self.enter_meadow();
                }
            }
            Action::Close => {
                if let Some(spot) = self.panel.take() {
                    self.walker.out_of(spot, level::COUNT);
                }
                self.phase = Phase::Meadow;
            }
            Action::ToggleGentle => {
                self.progress.gentle = !self.progress.gentle;
                save_progress(&self.window, &self.progress);
            }
            Action::ToggleContrast => {
                self.progress.high_contrast = !self.progress.high_contrast;
                save_progress(&self.window, &self.progress);
            }
            Action::Home => {
                let _ = self.window.location().set_href("/");
            }
            _ => {}
        }
    }

    /// Back to the meadow, out of the hole of the garden just played.
    fn enter_meadow(&mut self) {
        self.phase = Phase::Meadow;
        self.panel = None;
        self.held = [false; 4];
        self.walker.out_of(Spot::Hole(self.garden), level::COUNT);
        self.last = 0.0;
        let _ = self.canvas.focus();
    }

    fn high_contrast(&self) -> bool {
        self.options.high_contrast || self.progress.high_contrast
    }

    /// Walks the meadow for `dt` seconds and enters what the bunny walks
    /// into.
    fn walk(&mut self, dt: f32) {
        if self.panel.is_some() {
            return;
        }
        let [up, down, left, right] = self.held;
        let mut forward = f32::from(u8::from(up)) - f32::from(u8::from(down));
        let mut turn = f32::from(u8::from(right)) - f32::from(u8::from(left));
        if let Some((from, to)) = self.drag {
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);
            if dx.abs().max(dy.abs()) > 12.0 {
                forward = (-dy / 60.0).clamp(-1.0, 1.0);
                turn = (dx / 60.0).clamp(-1.0, 1.0);
            }
        }
        self.walker.step(dt, forward, turn);
        if self.walker.moving {
            self.hop += dt * 10.0;
        }
        match self.walker.at(level::COUNT) {
            Some(Spot::Hole(n)) => self.play(n),
            Some(spot) => {
                self.panel = Some(spot);
                self.held = [false; 4];
                self.drag = None;
            }
            None => {}
        }
    }

    /// The card for a meadow spot.
    fn panel_card(&self, spot: Spot) -> Card {
        let close = (copy::CLOSE.to_owned(), Action::Close, false);
        match spot {
            Spot::Burrow | Spot::Hole(_) => {
                let p = &self.progress;
                let mut lines = vec![
                    copy::wins_line(p.wins),
                    copy::shade_line(shade::name(p.shade()), p.shade()),
                ];
                lines.push(if p.to_next_shade().is_some() {
                    copy::NEXT_SHADE.to_owned()
                } else {
                    copy::LAST_SHADE.to_owned()
                });
                Card {
                    title: copy::BURROW.to_owned(),
                    swatch: Some(self.fur()),
                    lines,
                    hint: None,
                    buttons: vec![
                        (copy::gentle(p.gentle), Action::ToggleGentle, false),
                        (
                            copy::contrast(p.high_contrast),
                            Action::ToggleContrast,
                            false,
                        ),
                        (copy::CLOSE.to_owned(), Action::Close, true),
                    ],
                }
            }
            Spot::Board => {
                let lines = (1..=level::COUNT)
                    .map(|n| {
                        let name = level::garden(n).name;
                        match self.progress.best(n as u8) {
                            Some(best) => copy::best_line(n, &name, best.ticks / HZ, best.score),
                            None => copy::not_cleared(n, &name),
                        }
                    })
                    .collect();
                Card {
                    title: copy::BOARD.to_owned(),
                    swatch: None,
                    lines,
                    hint: None,
                    buttons: vec![(copy::CLOSE.to_owned(), Action::Close, true)],
                }
            }
            Spot::Arch => Card {
                title: copy::ARCH.to_owned(),
                swatch: None,
                lines: Vec::new(),
                hint: None,
                buttons: vec![(copy::HOME.to_owned(), Action::Home, false), close],
            },
        }
    }

    /// The card for the current phase, if one shows.
    fn card(&self) -> Option<Card> {
        let garden_buttons = || {
            (1..=level::COUNT)
                .map(|n| {
                    (
                        copy::garden_button(n, &level::garden(n).name),
                        Action::Play(n),
                        false,
                    )
                })
                .collect::<Vec<_>>()
        };
        match self.phase {
            Phase::Playing => None,
            Phase::Meadow => self.panel.map(|spot| self.panel_card(spot)),
            Phase::Title => {
                let _ = garden_buttons;
                Some(Card {
                    title: copy::TITLE.to_owned(),
                    swatch: (self.progress.wins > 0).then(|| self.fur()),
                    lines: vec![copy::GOAL.to_owned(), self.meadow_hint().to_owned()],
                    hint: Some(self.hint().to_owned()),
                    buttons: vec![(copy::PLAY.to_owned(), Action::Meadow, true)],
                })
            }
            Phase::Paused => Some(Card {
                title: copy::PAUSED.to_owned(),
                swatch: None,
                lines: vec![self.game.garden.name.clone()],
                hint: Some(self.hint().to_owned()),
                buttons: vec![
                    (copy::RESUME.to_owned(), Action::Resume, true),
                    (copy::RESTART.to_owned(), Action::Restart, false),
                    (copy::LEAVE.to_owned(), Action::Leave, false),
                ],
            }),
            Phase::Over => {
                if self.time - self.over_at < 0.8 {
                    return None;
                }
                let won = self.game.status == Status::Won;
                let mut lines = Vec::new();
                if won {
                    lines.push(copy::won_line(self.progress.wins));
                    let seconds = self.game.clear_tick.unwrap_or(self.game.tick) / HZ;
                    lines.push(copy::time_line(seconds));
                } else {
                    lines.push(copy::CAUGHT_LINE.to_owned());
                }
                lines.push(copy::score_line(self.game.score));
                let mut buttons = Vec::new();
                if won && self.garden < level::COUNT {
                    buttons.push((copy::NEXT.to_owned(), Action::Play(self.garden + 1), true));
                    buttons.push((copy::PLAY_AGAIN.to_owned(), Action::Restart, false));
                } else {
                    buttons.push((copy::PLAY_AGAIN.to_owned(), Action::Restart, true));
                }
                buttons.push((copy::MEADOW.to_owned(), Action::Meadow, false));
                Some(Card {
                    title: (if won { copy::WON } else { copy::CAUGHT }).to_owned(),
                    swatch: won.then(|| self.fur()),
                    lines,
                    hint: None,
                    buttons,
                })
            }
        }
    }

    fn meadow_hint(&self) -> &'static str {
        if self.coarse {
            copy::MEADOW_DRAG
        } else {
            copy::MEADOW_KEYS
        }
    }

    fn hint(&self) -> &'static str {
        if self.coarse {
            copy::SWIPES
        } else {
            copy::KEYS
        }
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
        for action in self.hud.take_actions() {
            self.act(action);
        }
        if self.phase == Phase::Meadow {
            self.walk(dtf);
        }
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
        let playing = matches!(self.phase, Phase::Playing | Phase::Paused);
        self.hud.show_game(playing && !self.options.kit);
        let prompt = (self.phase == Phase::Meadow && self.panel.is_none())
            .then(|| self.walker.near(level::COUNT, 5.0))
            .flatten()
            .map(|spot| match spot {
                Spot::Hole(n) => copy::garden_button(n, &level::garden(n).name),
                Spot::Burrow => copy::BURROW.to_owned(),
                Spot::Board => copy::BOARD.to_owned(),
                Spot::Arch => copy::ARCH_PROMPT.to_owned(),
            });
        self.hud.meadow(
            self.phase == Phase::Meadow && !self.options.kit,
            self.fur(),
            &copy::wins_line(self.progress.wins),
            prompt.as_deref(),
        );
        self.hud.show_run_buttons(self.phase == Phase::Playing);
        if playing {
            let elements = crate::zone::BunnyGame::hud_of(&self.game);
            let (w, h) = (
                self.canvas.client_width() as f32,
                self.canvas.client_height() as f32,
            );
            self.hud.update(&elements, w, h);
        }
        let card = if self.options.kit { None } else { self.card() };
        self.hud.card(card);
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
                self.burst(at, 6, scene::edible_colour(c.kind), 0.07, 1.6);
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
            Event::AteBonus(_) => self.burst(bunny, 16, 0x7CC242, 0.09, 2.6),
            Event::PowerUp(_) => self.burst(bunny, 12, scene::GOLD, 0.07, 2.2),
            Event::Bumped(_) => {
                let at = point(self.game.farmer_point());
                self.burst(Vec3::new(at.x, 0.8, at.y), 14, scene::GOLD, 0.09, 2.6);
            }
            _ => {}
        }
    }

    fn finish(&mut self) {
        self.phase = Phase::Over;
        self.over_at = self.time;
        let ticks = self.game.clear_tick.unwrap_or(self.game.tick);
        self.progress
            .record(self.garden as u8, self.game.status, ticks, self.game.score);
        if self.game.status == Status::Won {
            save_progress(&self.window, &self.progress);
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
        if self.options.kit {
            let view_projection = kit_camera(aspect);
            self.gpu.camera(&view_projection, vw, vh);
            self.draw_kit();
        } else if matches!(self.phase, Phase::Meadow | Phase::Title) {
            let view_projection = self.meadow_camera(aspect);
            self.gpu.camera(&view_projection, vw, vh);
            self.draw_meadow();
        } else {
            self.draw_world(alpha, bunny, farmer);
        }
        if let (true, Some(outline)) = (offscreen, self.outline.as_ref()) {
            outline.finish(
                &gl,
                (width as i32, height as i32),
                NEAR,
                FAR,
                self.tier,
                self.high_contrast(),
                rgb(scene::INK),
                rgb(scene::FAR_INK),
            );
            gl.use_program(Some(&self.gpu.program));
            gl.clear(Gl::DEPTH_BUFFER_BIT);
        }
        self.gpu.lined.set(false);
        self.gpu.look.set(Look::Gray);
        if !self.options.kit && matches!(self.phase, Phase::Playing | Phase::Paused) {
            self.draw_map(width, height, bunny, farmer);
        }
    }

    fn camera(&self, bunny: Vec2, farmer: Vec2, aspect: f32) -> Mat4 {
        let t = self.camera_size;
        let mut back = 3.5 + 2.5 * t;
        let mut up = 1.6 + 1.6 * t;
        if aspect < 1.0 {
            back *= 1.25;
            up *= 1.7;
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
        let ahead = if aspect < 1.0 { 2.0 } else { 4.0 };
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
        gpu.look.set(Look::Chroma);
        if game.bonus_out() {
            let spot = garden.bonus;
            let at = point(garden.point(spot.edge, spot.s, i32::from(spot.lane) * lane));
            let pop = (self.time * 4.0).sin() * 0.05 + 0.1;
            let model = scene::place(Vec3::new(at.x, pop, at.y), self.time * 2.0, 1.0);
            gpu.draw(
                m.edible(EdibleKind::Bonus),
                &model,
                WHITE,
                Some((ink, LINE * 1.5)),
            );
        }
        for (index, p) in garden.powers.iter().enumerate() {
            if game.taken[index] {
                continue;
            }
            let at = point(garden.point(p.edge, p.s, i32::from(p.lane) * lane));
            let bob = 0.25 + 0.08 * (self.time * 2.5 + index as f32).sin();
            let model = scene::place(Vec3::new(at.x, bob, at.y), self.time * 1.8, 1.2);
            gpu.draw(m.power(p.kind), &model, WHITE, Some((ink, LINE * 1.5)));
        }
        gpu.look.set(Look::Gray);
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
        gpu.draw(&m.bunny[usize::from(b.tier)], &body, WHITE, outline);
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

    fn meadow_camera(&self, aspect: f32) -> Mat4 {
        let w = &self.walker;
        let forward = Vec3::new(w.yaw.sin(), 0.0, w.yaw.cos());
        let at = Vec3::new(w.x, 0.0, w.z);
        let (back, up) = if aspect < 1.0 { (6.5, 4.2) } else { (5.5, 2.8) };
        let eye = at - forward * back + Vec3::Y * up;
        let look = at + forward * 3.0 + Vec3::Y * 0.6;
        let fov = if aspect < 1.0 { 75.0_f32 } else { 60.0 };
        Mat4::perspective_rh_gl(fov.to_radians(), aspect, NEAR, FAR)
            * Mat4::look_at_rh(eye, look, Vec3::Y)
    }

    /// Warren Meadow.
    fn draw_meadow(&self) {
        let gpu = &self.gpu;
        let m = &self.meshes;
        let ink = rgb(scene::INK);
        let line = Some((ink, LINE));
        let piece = |kind: Piece| pick(&m.pieces, kind);
        gpu.look.set(Look::Ground);
        gpu.draw(&m.meadow, &Mat4::IDENTITY, WHITE, None);
        gpu.draw(
            piece(Piece::Pond),
            &scene::place(
                Vec3::new(meadow::POND_AT.0, 0.0, meadow::POND_AT.1),
                0.0,
                1.0,
            ),
            WHITE,
            None,
        );
        gpu.look.set(Look::Gray);
        gpu.draw(piece(Piece::Mound), &Mat4::IDENTITY, WHITE, line);
        for (spot, (x, z), _) in meadow::spots(level::COUNT) {
            let at = Vec3::new(x, 0.0, z);
            match spot {
                Spot::Hole(n) => {
                    gpu.draw(piece(Piece::Hole), &scene::place(at, 0.0, 1.0), WHITE, line);
                    let sign = at + Vec3::new(1.4, 0.0, -1.2);
                    gpu.draw(
                        piece(Piece::Signpost),
                        &scene::place(sign, -1.2, 1.0),
                        WHITE,
                        line,
                    );
                    // Dots on the sign count the garden.
                    gpu.look.set(Look::Chroma);
                    for i in 0..n {
                        let dot = sign
                            + Vec3::new(0.0, 1.55 + 0.0 * i as f32, 0.0)
                            + Vec3::new((-1.2_f32).sin(), 0.0, (-1.2_f32).cos()) * 0.07
                            + Vec3::new((-1.2_f32).cos(), 0.0, -(-1.2_f32).sin())
                                * ((i as f32 - (n as f32 - 1.0) / 2.0) * 0.24);
                        gpu.draw(
                            &m.crumb,
                            &(Mat4::from_translation(dot) * Mat4::from_scale(Vec3::splat(0.16))),
                            rgb(scene::CARROT),
                            None,
                        );
                    }
                    gpu.look.set(Look::Gray);
                }
                Spot::Burrow => {}
                Spot::Board => gpu.draw(
                    piece(Piece::Board),
                    &scene::place(at, std::f32::consts::PI, 1.0),
                    WHITE,
                    line,
                ),
                Spot::Arch => gpu.draw(
                    piece(Piece::Arch),
                    &scene::place(at, std::f32::consts::FRAC_PI_2, 1.0),
                    WHITE,
                    line,
                ),
            }
        }
        for (i, (x, z)) in meadow::TREES.iter().enumerate() {
            gpu.draw(
                piece(Piece::Tree),
                &scene::place(Vec3::new(*x, 0.0, *z), i as f32, 1.0),
                WHITE,
                line,
            );
            let f = Vec3::new(x + 2.2, 0.0, z + 1.0);
            gpu.draw(
                piece(Piece::Flowers),
                &scene::place(f, i as f32, 1.0),
                WHITE,
                line,
            );
        }
        gpu.draw(
            piece(Piece::Log),
            &scene::place(Vec3::new(-10.0, 0.0, 30.0), 0.4, 1.0),
            WHITE,
            line,
        );
        // The ladder: 21 stones around the pond, lit up to the bunny's
        // shade, the current one raised.
        let shade_now = self.progress.shade() as usize;
        for i in 0..meadow::STONES {
            let (x, z) = meadow::stone(i);
            let lit = i <= shade_now;
            let raise = if i == shade_now {
                0.25 + 0.06 * (self.time * 3.0).sin()
            } else {
                0.0
            };
            let model = scene::place(Vec3::new(x, raise, z), i as f32, 1.0);
            if lit {
                gpu.look.set(Look::Chroma);
                gpu.draw(&m.stone, &model, rgb(shade::SHADES[i]), Some((ink, LINE)));
                gpu.look.set(Look::Gray);
            } else {
                gpu.draw(&m.stone, &model, [0.62, 0.62, 0.6], line);
            }
        }
        // The bunny, at Bunny size.
        let w = &self.walker;
        let size = scene::metres(TIER_HEIGHT[1]) * DRAWN * 1.4;
        let hop = if w.moving {
            self.hop.sin().abs() * 0.3 * size
        } else {
            0.0
        };
        gpu.look.set(Look::Ground);
        gpu.draw(
            &m.shadow,
            &scene::place(Vec3::new(w.x, 0.0, w.z), 0.0, size * 1.3),
            WHITE,
            None,
        );
        gpu.look.set(Look::Chroma);
        let fur = rgb(self.fur());
        let outline = Some(([fur[0] * 0.38, fur[1] * 0.36, fur[2] * 0.36], LINE));
        let body = Mat4::from_translation(Vec3::new(w.x, hop, w.z))
            * Mat4::from_rotation_y(w.yaw)
            * Mat4::from_scale(Vec3::splat(size));
        gpu.draw(&m.bunny[1], &body, WHITE, outline);
        for side in [-1.0_f32, 1.0] {
            let ear = body
                * Mat4::from_translation(Vec3::new(0.1 * side, 0.84, 0.3))
                * Mat4::from_quat(
                    Quat::from_rotation_z(-0.22 * side) * Quat::from_rotation_x(-0.15),
                );
            gpu.draw(&m.ear, &ear, WHITE, outline);
        }
        for p in &self.particles {
            let scale = p.size * (p.life / p.span).max(0.2);
            gpu.draw(
                &m.crumb,
                &(Mat4::from_translation(p.at) * Mat4::from_scale(Vec3::splat(scale))),
                p.colour,
                None,
            );
        }
        gpu.look.set(Look::Gray);
    }

    /// The kit sheet (`#kit`): every model in rows, for review.
    fn draw_kit(&self) {
        let gpu = &self.gpu;
        let m = &self.meshes;
        let ink = rgb(scene::INK);
        let line = Some((ink, LINE));
        let place = |row: f32, column: usize, count: usize, spacing: f32| {
            let x = (column as f32 - (count as f32 - 1.0) / 2.0) * spacing;
            Vec3::new(x, 0.0, row)
        };
        gpu.look.set(Look::Ground);
        gpu.draw(
            &m.ground,
            &Mat4::from_translation(Vec3::new(-30.0, 0.0, -20.0)),
            WHITE,
            None,
        );
        gpu.look.set(Look::Gray);
        let count = m.obstacles.len();
        for (i, (_, mesh)) in m.obstacles.iter().enumerate() {
            gpu.draw(
                mesh,
                &scene::place(place(0.0, i, count, 1.6), 0.5, 1.0),
                WHITE,
                line,
            );
        }
        gpu.look.set(Look::Chroma);
        let count = m.edibles.len() + m.powers.len() + 5;
        for (i, (_, mesh)) in m.edibles.iter().enumerate() {
            gpu.draw(
                mesh,
                &scene::place(place(3.0, i, count, 1.1), 0.5, 1.0),
                WHITE,
                Some((ink, LINE * 1.5)),
            );
        }
        for (i, (_, mesh)) in m.powers.iter().enumerate() {
            let at = place(3.0, m.edibles.len() + i, count, 1.1);
            gpu.draw(
                mesh,
                &scene::place(at, 0.5, 1.2),
                WHITE,
                Some((ink, LINE * 1.5)),
            );
        }
        for (tier, mesh) in m.bunny.iter().enumerate() {
            let at = place(3.0, m.edibles.len() + m.powers.len() + tier, count, 1.1);
            let size = scene::metres(TIER_HEIGHT[tier]) * DRAWN;
            let fur = rgb(shade::SHADES[tier * 5]);
            gpu.draw(
                mesh,
                &scene::place(at, 0.6, size),
                WHITE,
                Some(([fur[0] * 0.38, fur[1] * 0.36, fur[2] * 0.36], LINE)),
            );
        }
        gpu.look.set(Look::Gray);
        gpu.draw(
            &m.farmer,
            &scene::place(Vec3::new(-9.0, 0.0, 7.0), 0.5, 1.0),
            WHITE,
            line,
        );
        gpu.draw(
            &m.net,
            &(scene::place(Vec3::new(-9.0, 0.0, 7.0), 0.5, 1.0)
                * Mat4::from_translation(Vec3::new(0.36, 1.0, 0.28))),
            WHITE,
            line,
        );
        let count = m.pieces.len();
        for (i, (kind, mesh)) in m.pieces.iter().enumerate() {
            let scale = match kind {
                Piece::Mound | Piece::Pond => 0.18,
                Piece::Tree | Piece::Arch | Piece::Gate | Piece::Board => 0.45,
                _ => 0.8,
            };
            gpu.draw(
                mesh,
                &scene::place(place(8.0, i, count, 2.2), 0.5, scale),
                WHITE,
                line,
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
        let mut base = scene::place(Vec3::new(at.x, 0.0, at.y), self.farmer_yaw + stagger, 1.0);
        // Spooked he crouches and wobbles, flickering in his last 2 s;
        // dazed he lies in the compost with stars over him.
        let mut tint = WHITE;
        match f.state {
            FarmerState::Spooked { left } => {
                let wobble = (self.time * 14.0).sin() * 0.08;
                base = base
                    * Mat4::from_rotation_z(wobble)
                    * Mat4::from_scale(Vec3::new(1.0, 0.82, 1.0));
                let flicker = left < 2 * HZ && (self.time * 10.0).sin() > 0.0;
                tint = if flicker { WHITE } else { [1.25, 1.25, 1.25] };
            }
            FarmerState::Dazed { .. } => {
                base = base
                    * Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0))
                    * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                gpu.look.set(Look::Chroma);
                for i in 0..4 {
                    let a = self.time * 3.0 + i as f32 * std::f32::consts::FRAC_PI_2;
                    let star = Vec3::new(at.x + a.cos() * 0.5, 0.9, at.y + a.sin() * 0.5);
                    gpu.draw(
                        &m.crumb,
                        &(Mat4::from_translation(star)
                            * Mat4::from_rotation_y(a)
                            * Mat4::from_scale(Vec3::splat(0.12))),
                        rgb(scene::GOLD),
                        None,
                    );
                }
            }
            _ => {}
        }
        gpu.look.set(Look::Ground);
        gpu.draw(
            &m.shadow,
            &scene::place(Vec3::new(at.x, 0.0, at.y), 0.0, 0.9),
            WHITE,
            None,
        );
        gpu.look.set(Look::Gray);
        gpu.draw(&m.farmer, &base, tint, line);
        let swing = self.stride.sin() * 0.55;
        for (side, phase) in [(-1.0_f32, 1.0_f32), (1.0, -1.0)] {
            let leg = base
                * Mat4::from_translation(Vec3::new(0.12 * side, 0.86, 0.0))
                * Mat4::from_rotation_x(swing * phase);
            gpu.draw(&m.leg, &leg, tint, line);
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
}

/// Which meadow walking key a key is: forward, back, left, right.
fn held_index(key: &str) -> Option<usize> {
    match verse_game::GameInput::from_key(key)? {
        verse_game::GameInput::Up => Some(0),
        verse_game::GameInput::Down => Some(1),
        verse_game::GameInput::Left => Some(2),
        verse_game::GameInput::Right => Some(3),
        _ => None,
    }
}

/// A key's game input, through the community-game input channel.
fn input_for(key: &str) -> Option<Input> {
    verse_game::GameInput::from_key(key).and_then(crate::zone::input)
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
    let progress = load_progress(&window);
    let fur = shade::fur(progress.wins);
    let obstacle = |kind| gpu.upload(&scene::obstacle(kind));
    let meshes = Meshes {
        ground: gpu.upload(&scene::ground(&garden)),
        hedges: gpu.upload(&scene::hedges(&garden)),
        edibles: EdibleKind::ALL
            .iter()
            .map(|kind| (*kind, gpu.upload(&scene::edible(*kind))))
            .collect(),
        bunny: (0..5)
            .map(|tier| gpu.upload(&scene::bunny(fur, tier)))
            .collect(),
        ear: gpu.upload(&scene::ear(fur)),
        farmer: gpu.upload(&scene::farmer()),
        leg: gpu.upload(&scene::leg()),
        net: gpu.upload(&scene::net()),
        alarm: gpu.upload(&scene::alarm()),
        shadow: gpu.upload(&scene::shadow()),
        crumb: gpu.upload(&scene::crumb()),
        dot: gpu.upload(&scene::dot()),
        obstacles: ObstacleKind::ALL
            .iter()
            .map(|kind| (*kind, obstacle(*kind)))
            .collect(),
        powers: PowerKind::ALL
            .iter()
            .map(|kind| (*kind, gpu.upload(&kit::power(*kind))))
            .collect(),
        pieces: Piece::ALL
            .iter()
            .map(|kind| (*kind, gpu.upload(&kit::piece(*kind))))
            .collect(),
        meadow: gpu.upload(&scene::meadow_ground()),
        stone: gpu.upload(&kit::ladder_stone()),
    };
    let touch = window
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let hud = Hud::new(&document, &parent, touch);
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
        garden: 1,
        runs: 0,
        coarse: touch,
        phase: Phase::Title,
        progress,
        walker: Walker::default(),
        panel: None,
        held: [false; 4],
        drag: None,
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
    };
    let first = app.options.garden.unwrap_or(1).clamp(1, level::COUNT);
    app.new_run(first);
    if app.options.garden.is_some() && !app.options.kit {
        app.phase = Phase::Playing;
    }
    if app.options.kit {
        show(&app.hud.root, false);
    }
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
            if app.phase == Phase::Meadow && app.panel.is_none() {
                if let Some(index) = held_index(&key) {
                    event.prevent_default();
                    app.held[index] = true;
                }
                return;
            }
            let pause = matches!(key.as_str(), "Escape" | "p" | "P");
            if pause && app.phase == Phase::Playing {
                app.act(Action::Pause);
            } else if pause && app.phase == Phase::Paused {
                app.act(Action::Resume);
            } else if (key == "r" || key == "R") && app.phase == Phase::Playing {
                app.act(Action::Restart);
            }
        });
    }
    {
        let app = app.clone();
        listen::<KeyboardEvent>(window.as_ref(), "keyup", move |event| {
            if let Some(index) = held_index(&event.key()) {
                app.borrow_mut().held[index] = false;
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerdown", move |event| {
            let mut app = app.borrow_mut();
            let at = (event.client_x() as f32, event.client_y() as f32);
            app.touch = Some((at.0, at.1, event.time_stamp()));
            if app.phase == Phase::Meadow {
                app.drag = Some((at, at));
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointermove", move |event| {
            let mut app = app.borrow_mut();
            if let Some((from, _)) = app.drag {
                app.drag = Some((from, (event.client_x() as f32, event.client_y() as f32)));
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerup", move |event| {
            let mut app = app.borrow_mut();
            app.drag = None;
            let Some((x, y, at)) = app.touch.take() else {
                return;
            };
            let (dx, dy) = (event.client_x() as f32 - x, event.client_y() as f32 - y);
            let swipe = verse_game::Swipe::default().read(dx, dy, event.time_stamp() - at);
            if let Some(input) = swipe.and_then(crate::zone::input) {
                app.press(input);
            }
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointercancel", move |_| {
            let mut app = app.borrow_mut();
            app.touch = None;
            app.drag = None;
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

/// The kit sheet's fixed camera, over three rows of models.
fn kit_camera(aspect: f32) -> Mat4 {
    let eye = Vec3::new(0.0, 9.0, 17.0);
    let look = Vec3::new(0.0, 0.0, 4.0);
    Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect, NEAR, FAR)
        * Mat4::look_at_rh(eye, look, Vec3::Y)
}

/// The animation-frame callback, kept so it can ask for the next frame.
type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
