//! The browser side: WebGL2 drawing, the pointer, keys and the HUD,
//! sounds, and storage.

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Mat4, Vec2, Vec3};
use snap_rules::game::{Event, Input, Status};
use snap_rules::progress::Progress;
use snap_rules::{HZ, PieceKind, V, level, mm, nearest_turn};
use verse_game::{CommunityGame, GameInput};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, HtmlCanvasElement, KeyboardEvent, PointerEvent, WebGl2RenderingContext as Gl,
    WebGlBuffer, WebGlProgram, WebGlUniformLocation, WebGlVertexArrayObject, WheelEvent, Window,
};

use crate::copy;
use crate::hud::{Action, Button, Card, Hud, Mode, css, element, show};
use crate::kit::{self, Crew, Mood, Style};
use crate::mesh::{Mesh, STRIDE};
use crate::options::Options;
use crate::scene::{self, Frame, Held, Spark};
use crate::sound::{Effect, Sound};
use crate::tutorial::{self, Tour};
use crate::zone::{Slot, SnapGame};

const VERTEX: &str = r"#version 300 es
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_colour;
uniform mat4 u_vp;
out vec3 v_normal;
out vec3 v_colour;
void main() {
  v_normal = a_normal;
  v_colour = a_colour;
  gl_Position = u_vp * vec4(a_position, 1.0);
}
";

const FRAGMENT: &str = r"#version 300 es
precision mediump float;
in vec3 v_normal;
in vec3 v_colour;
uniform float u_unlit;
uniform float u_alpha;
out vec4 o_colour;
void main() {
  vec3 c = v_colour;
  if (u_unlit < 0.5) {
    vec3 n = normalize(v_normal);
    float d = max(dot(n, normalize(vec3(-0.35, 0.55, 0.85))), 0.0);
    float rim = pow(1.0 - max(n.z, 0.0), 2.0) * 0.12;
    c = c * (0.42 + 0.62 * d) + rim;
  }
  o_colour = vec4(c, u_alpha);
}
";

const STEP: f64 = 1.0 / HZ as f64;
#[cfg(not(feature = "autoplay"))]
const MAX_STEPS: u32 = 8;
/// Captures in a headless browser draw few frames; catch up on all of it.
#[cfg(feature = "autoplay")]
const MAX_STEPS: u32 = 600;
const SAVE_KEY: &str = "snap.progress.v1";
/// The camera's vertical field of view, degrees.
const FOV: f32 = 40.0;
/// How far a press may move and still be a tap, in CSS pixels.
const TAP: f32 = 8.0;
/// How long after a run ends the results card opens, in seconds.
const RESULTS_AFTER: f64 = 1.2;

struct GpuMesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    count: i32,
}

struct Gpu {
    gl: Gl,
    u_vp: Option<WebGlUniformLocation>,
    u_unlit: Option<WebGlUniformLocation>,
    u_alpha: Option<WebGlUniformLocation>,
}

impl Gpu {
    fn new(gl: Gl) -> Result<Self, String> {
        let program = program(&gl, VERTEX, FRAGMENT)?;
        gl.use_program(Some(&program));
        let at = |name: &str| gl.get_uniform_location(&program, name);
        let gpu = Self {
            u_vp: at("u_vp"),
            u_unlit: at("u_unlit"),
            u_alpha: at("u_alpha"),
            gl,
        };
        gpu.gl.enable(Gl::DEPTH_TEST);
        Ok(gpu)
    }

    fn upload(&self, mesh: &Mesh) -> GpuMesh {
        let gl = &self.gl;
        let vao = gl.create_vertex_array().expect("a vertex array");
        let buffer = gl.create_buffer().expect("a buffer");
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        let stride = (STRIDE * 4) as i32;
        for (index, offset) in [0, 3, 6].iter().enumerate() {
            gl.enable_vertex_attrib_array(index as u32);
            gl.vertex_attrib_pointer_with_i32(
                index as u32,
                3,
                Gl::FLOAT,
                false,
                stride,
                offset * 4,
            );
        }
        let mut gpu = GpuMesh {
            vao,
            buffer,
            count: 0,
        };
        self.refill(&mut gpu, mesh);
        gl.bind_vertex_array(None);
        gpu
    }

    fn refill(&self, gpu: &mut GpuMesh, mesh: &Mesh) {
        let gl = &self.gl;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&gpu.buffer));
        let array = js_sys::Float32Array::from(mesh.data.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::DYNAMIC_DRAW);
        gpu.count = mesh.vertices() as i32;
    }

    fn draw(&self, mesh: &GpuMesh, unlit: bool, alpha: f32) {
        if mesh.count == 0 {
            return;
        }
        let gl = &self.gl;
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.uniform1f(self.u_unlit.as_ref(), if unlit { 1.0 } else { 0.0 });
        gl.uniform1f(self.u_alpha.as_ref(), alpha);
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

fn program(gl: &Gl, vertex: &str, fragment: &str) -> Result<WebGlProgram, String> {
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

/// How a held piece is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grip {
    /// Following a pressed pointer; letting go drops it.
    Drag,
    /// Picked with a tap; the next tap on the station drops it.
    Tap,
}

/// A piece in the player's hand.
#[derive(Clone, Copy, Debug)]
struct Hand {
    kind: PieceKind,
    turn: u8,
    /// The pointer, in CSS pixels.
    screen: (f32, f32),
    /// From the pointer to the piece's middle, in metres.
    grab: V,
    grip: Grip,
    /// How far the pointer has moved since the press.
    moved: f32,
}

/// Which card is open over the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Overlay {
    Jobs,
    Ask,
    Menu,
    Results,
    None,
}

/// What the camera looks at: the middle of the view and pixels a metre.
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    centre: Vec2,
    scale: f32,
}

struct App {
    window: Window,
    canvas: HtmlCanvasElement,
    gpu: Gpu,
    hud: Hud,
    sound: Sound,
    options: Options,
    backdrop: (GpuMesh, GpuMesh),
    lit: GpuMesh,
    glow: GpuMesh,
    ghost: GpuMesh,
    progress: Progress,
    snap: SnapGame,
    job: usize,
    overlay: Overlay,
    tour: Option<Tour>,
    hand: Option<Hand>,
    /// The keyboard was used last, so its cursor and slot show.
    keyboard: bool,
    view: View,
    view_ready: bool,
    sparks: Vec<Spark>,
    pending: Vec<Input>,
    last: f64,
    carry: f64,
    time: f64,
    over_at: Option<f64>,
    recorded: bool,
    shown_view: String,
    nope_until: f64,
    #[cfg(feature = "autoplay")]
    auto: Option<(Vec<Input>, usize, f64)>,
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
    if let Some(storage) = window.local_storage().ok().flatten() {
        let _ = storage.set_item(SAVE_KEY, &progress.encode());
    }
}

fn tutorial_seen(window: &Window) -> Option<String> {
    window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(tutorial::SEEN_KEY).ok().flatten())
}

fn mark_tutorial_seen(window: &Window) {
    if let Some(storage) = window.local_storage().ok().flatten() {
        let _ = storage.set_item(tutorial::SEEN_KEY, tutorial::SEEN);
    }
}

fn v2(p: V) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

/// The turn a placed piece sits at, from its corners.
fn turn_of(piece: &snap_rules::Placed) -> u8 {
    let shape = piece.kind.shape();
    let local = (shape[1] - shape[0]).unit();
    let world = (piece.at[1] - piece.at[0]).unit();
    nearest_turn(local.dot(world), local.cross(world))
}

impl App {
    fn start_job(&mut self, job: usize, ask: bool) {
        self.job = job.clamp(1, level::COUNT);
        self.snap = SnapGame::new(level::job(self.job));
        self.overlay = if ask { Overlay::Ask } else { Overlay::None };
        self.hand = None;
        self.sparks.clear();
        self.pending.clear();
        self.over_at = None;
        self.recorded = false;
        self.view_ready = false;
        self.carry = 0.0;
        #[cfg(feature = "autoplay")]
        {
            self.auto = snap_rules::bot::solve(&self.snap.game.level).map(|plan| (plan, 0, 0.0));
        }
    }

    fn crew(&self) -> Crew {
        scene::crew(&self.snap.game)
    }

    fn building(&self) -> bool {
        self.snap.game.status == Status::Building
    }

    fn act(&mut self, action: Action) {
        self.sound.wake();
        match action {
            Action::Play(n) => {
                if self.progress.open(n as u8) {
                    self.start_job(n, true);
                }
            }
            Action::Pick(kind, x, y) => {
                if !self.building() || self.overlay != Overlay::None || self.tour.is_some() {
                    return;
                }
                if self
                    .hand
                    .is_some_and(|h| h.kind == kind && h.grip == Grip::Tap)
                {
                    self.hand = None;
                    return;
                }
                if self.snap.game.left(kind) == 0 {
                    self.sound.play(Effect::Nope);
                    return;
                }
                self.keyboard = false;
                self.snap.carrying = None;
                self.hand = Some(Hand {
                    kind,
                    turn: 0,
                    screen: (x, y),
                    grab: V::ZERO,
                    grip: Grip::Drag,
                    moved: 0.0,
                });
                self.sound.play(Effect::Lift);
            }
            Action::Turn => {
                if let Some(hand) = &mut self.hand {
                    hand.turn = (hand.turn + 1) % snap_rules::TURNS;
                    self.sound.play(Effect::Lift);
                }
            }
            Action::Undo => {
                let build = &self.snap.game.build;
                if let Some(last) = (0..build.pieces.len())
                    .rev()
                    .find(|i| !build.pieces[*i].given)
                {
                    let at = build.pieces[last].centre();
                    self.pending.push(Input::Remove {
                        x: mm(at.x),
                        y: mm(at.y),
                    });
                }
            }
            Action::Go => {
                self.hand = None;
                match self.snap.game.status {
                    Status::Building => self.pending.push(Input::Go),
                    Status::Running => self.pending.push(Input::Fix),
                    _ => {}
                }
            }
            Action::Fix => {
                self.pending.push(Input::Fix);
                self.overlay = Overlay::None;
                self.over_at = None;
                self.recorded = false;
            }
            Action::NextJob => {
                if self.job < level::COUNT {
                    self.start_job(self.job + 1, true);
                } else {
                    self.overlay = Overlay::Jobs;
                }
            }
            Action::AllJobs => {
                self.overlay = Overlay::Jobs;
                self.hand = None;
            }
            Action::Menu => {
                if self.overlay == Overlay::None {
                    self.overlay = Overlay::Menu;
                    self.hand = None;
                }
            }
            Action::Resume | Action::Close => {
                if matches!(self.overlay, Overlay::Menu | Overlay::Ask) {
                    self.overlay = Overlay::None;
                }
            }
            Action::Restart => self.start_job(self.job, false),
            Action::ToggleSound => {
                self.progress.sound = !self.progress.sound;
                self.sound.on = self.progress.sound;
                save_progress(&self.window, &self.progress);
            }
            Action::ToggleContrast => {
                self.progress.high_contrast = !self.progress.high_contrast;
                save_progress(&self.window, &self.progress);
            }
            Action::Home => {
                let _ = self.window.location().set_href("/");
            }
            Action::Ask => {
                if self.overlay == Overlay::None && self.tour.is_none() {
                    self.overlay = Overlay::Ask;
                }
            }
            Action::Tour => {
                self.tour = Some(Tour::new());
                self.hand = None;
            }
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
        }
    }

    fn contrast(&self) -> bool {
        self.options.contrast || self.progress.high_contrast
    }

    /// The point in the build's plane under a screen point.
    fn world(&self, x: f32, y: f32) -> V {
        let rect = self.canvas.get_bounding_client_rect();
        let (w, h) = (rect.width() as f32, rect.height() as f32);
        let px = x - rect.left() as f32 - w / 2.0;
        let py = y - rect.top() as f32 - h / 2.0;
        let View { centre, scale } = self.view;
        V::new(
            f64::from(centre.x + px / scale),
            f64::from(centre.y - py / scale),
        )
    }

    fn held(&self) -> Option<Held> {
        if let Some(hand) = self.hand {
            let at = self.world(hand.screen.0, hand.screen.1) + hand.grab;
            return Some(Held {
                kind: hand.kind,
                at,
                turn: hand.turn,
            });
        }
        let kind = self.snap.carrying?;
        Some(Held {
            kind,
            at: self.snap.cursor,
            turn: self.snap.best_turn(kind),
        })
    }

    fn drop_hand(&mut self, x: f32, y: f32) {
        let Some(hand) = self.hand.take() else {
            return;
        };
        let at = self.world(x, y) + hand.grab;
        self.pending.push(Input::Place {
            kind: hand.kind,
            x: mm(at.x),
            y: mm(at.y),
            turn: hand.turn,
        });
    }

    fn pointer_down(&mut self, x: f32, y: f32) {
        self.sound.wake();
        if self.overlay != Overlay::None || self.tour.is_some() || !self.building() {
            return;
        }
        if self.hand.is_some_and(|h| h.grip == Grip::Tap) {
            self.drop_hand(x, y);
            return;
        }
        let at = self.world(x, y);
        if let Some(index) = self.snap.game.build.piece_at(at) {
            let piece = &self.snap.game.build.pieces[index];
            self.hand = Some(Hand {
                kind: piece.kind,
                turn: turn_of(piece),
                screen: (x, y),
                grab: piece.centre() - at,
                grip: Grip::Drag,
                moved: 0.0,
            });
            self.keyboard = false;
            self.pending.push(Input::Remove {
                x: mm(at.x),
                y: mm(at.y),
            });
            self.sound.play(Effect::Lift);
        }
    }

    fn pointer_move(&mut self, x: f32, y: f32) {
        if let Some(hand) = &mut self.hand {
            hand.moved += (x - hand.screen.0).abs() + (y - hand.screen.1).abs();
            hand.screen = (x, y);
        }
    }

    fn pointer_up(&mut self, x: f32, y: f32) {
        let Some(hand) = self.hand else {
            return;
        };
        if hand.grip != Grip::Drag {
            return;
        }
        if hand.moved < TAP {
            // A tap picks the piece up; the next tap puts it down.
            if let Some(hand) = &mut self.hand {
                hand.grip = Grip::Tap;
            }
            return;
        }
        if self.hud.covers(x, y) {
            // Back to the tray.
            self.hand = None;
            self.sound.play(Effect::Remove);
            return;
        }
        self.drop_hand(x, y);
    }

    fn key(&mut self, event: &KeyboardEvent) {
        self.sound.wake();
        let key = event.key();
        if self.tour.is_some() {
            let action = match key.as_str() {
                "Escape" => Some(Action::TourEnd(false)),
                "ArrowRight" => Some(Action::TourNext),
                "ArrowLeft" => Some(Action::TourBack),
                _ => None,
            };
            if let Some(action) = action {
                event.prevent_default();
                self.act(action);
            }
            return;
        }
        if self.overlay != Overlay::None {
            if key == "Escape" && matches!(self.overlay, Overlay::Menu | Overlay::Ask) {
                self.act(Action::Close);
            }
            return;
        }
        if key == "?" {
            self.act(Action::Tour);
            return;
        }
        match key.as_str() {
            "r" | "R" => {
                if let Some(hand) = &mut self.hand {
                    let turns = snap_rules::TURNS;
                    hand.turn = if event.shift_key() {
                        (hand.turn + turns - 1) % turns
                    } else {
                        (hand.turn + 1) % turns
                    };
                }
                return;
            }
            "g" | "G" => {
                self.act(Action::Go);
                return;
            }
            "Escape" => {
                if self.hand.take().is_none() && self.snap.carrying.take().is_none() {
                    self.act(Action::Menu);
                }
                return;
            }
            _ => {}
        }
        let Some(input) = GameInput::from_key(&key) else {
            return;
        };
        if input == GameInput::Pause {
            self.act(Action::Menu);
            return;
        }
        event.prevent_default();
        if event.repeat() && input == GameInput::Use {
            return;
        }
        self.keyboard = true;
        self.hand = None;
        let inputs = self.snap.convert(input);
        self.pending.extend(inputs);
    }

    fn burst(&mut self, at: Vec2, count: usize, colour: u32, speed: f32) {
        for i in 0..count {
            let a = i as f32 / count as f32 * std::f32::consts::TAU + self.time as f32;
            let s = speed * (0.6 + 0.4 * ((i * 7 % 5) as f32 / 5.0));
            self.sparks.push(Spark {
                at: Vec3::new(at.x, at.y, 0.4),
                velocity: Vec3::new(a.cos() * s, a.sin() * s, 0.0),
                colour,
                age: 0.0,
            });
        }
        if self.sparks.len() > 400 {
            let extra = self.sparks.len() - 400;
            self.sparks.drain(..extra);
        }
    }

    fn effects(&mut self) {
        let events = self.snap.game.events.clone();
        let game = &self.snap.game;
        let centre = |piece: usize| -> Vec2 {
            match &game.world {
                Some(world) => v2(world.centre(piece)),
                None => game
                    .build
                    .pieces
                    .get(piece)
                    .map_or(Vec2::ZERO, |p| v2(p.centre())),
            }
        };
        let thrusters = game
            .build
            .pieces
            .iter()
            .any(|p| p.kind == PieceKind::Thruster);
        let stormy = game.level.storm.is_some();
        let mut bursts = Vec::new();
        for event in events {
            match event {
                Event::Snapped(piece) => {
                    self.sound.play(Effect::Snap);
                    bursts.push((centre(piece), 10, 0xFFFFFF, 1.6));
                }
                Event::Braced(piece) => {
                    self.sound.play(Effect::Brace);
                    bursts.push((centre(piece), 6, 0xF2F2F2, 1.2));
                }
                Event::Bolted(_) => self.sound.play(Effect::Bolt),
                Event::NoSnap => {
                    self.sound.play(Effect::Nope);
                    self.nope_until = self.time + 2.0;
                }
                Event::Removed(_) => {}
                Event::Go => {
                    self.sound.play(Effect::Go);
                    if thrusters {
                        self.sound.play(Effect::Thrust);
                    }
                    if stormy {
                        self.sound.play(Effect::Storm);
                    }
                }
                Event::StormCame => {}
                Event::Broke(at) => {
                    self.sound.play(Effect::Break);
                    bursts.push((v2(at), 14, 0xFFD166, 2.6));
                }
                Event::Popped(piece) => {
                    self.sound.play(Effect::Pop);
                    bursts.push((centre(piece), 16, 0xFF5D5D, 2.2));
                }
                Event::Bumped(at) => {
                    self.sound.play(Effect::Bump);
                    bursts.push((v2(at), 12, 0xFFFFFF, 2.0));
                }
                Event::Moored(true) => self.sound.play(Effect::Bolt),
                Event::Moored(false) => self.sound.play(Effect::Nope),
                Event::Won => self.sound.play(Effect::Won),
                Event::Failed(_) => self.sound.play(Effect::Failed),
            }
        }
        for (at, count, colour, speed) in bursts {
            self.burst(at, count, colour, speed);
        }
    }

    fn tick(&mut self) {
        let inputs = std::mem::take(&mut self.pending);
        self.snap.step_rules(&inputs);
        self.effects();
    }

    fn jobs_card(&self) -> Card {
        let mut buttons = Vec::new();
        for n in 1..=level::COUNT {
            let job = level::job(n);
            let open = self.progress.open(n as u8);
            let stars = self.progress.best(n as u8).map_or(0, |b| b.stars);
            let mut b = Button::new(
                format!(
                    "{}  {}",
                    copy::job_button(n as u8, &job.name),
                    copy::stars(stars)
                ),
                Action::Play(n),
                false,
            );
            b.enabled = open;
            buttons.push(b);
        }
        buttons.push(Button::new(copy::HOW_TO_PLAY, Action::Tour, false));
        Card {
            title: copy::TITLE.to_owned(),
            crew: Some(Crew::Cog),
            lines: vec![copy::TAGLINE.to_owned(), copy::JOBS.to_owned()],
            stars: None,
            link: None,
            buttons,
            list: true,
        }
    }

    fn card(&self) -> Option<Card> {
        if let Some(tour) = self.tour {
            let (title, lines) = tour.words(self.hud.touch());
            let mut buttons = Vec::new();
            if !tour.is_first() {
                buttons.push(Button::new(copy::TOUR_BACK, Action::TourBack, false));
            }
            if tour.is_last() {
                buttons.push(Button::new(copy::TOUR_DONE, Action::TourEnd(true), true));
            } else {
                buttons.push(Button::new(copy::TOUR_SKIP, Action::TourEnd(false), false));
                buttons.push(Button::new(copy::TOUR_NEXT, Action::TourNext, true));
            }
            let mut lines: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
            lines.push(copy::tour_step_line(tour.step + 1, tutorial::STEPS));
            return Some(Card {
                title: title.to_owned(),
                crew: Some(Crew::ALL[tour.step % Crew::ALL.len()]),
                lines,
                stars: None,
                link: Some((copy::GUIDE.to_owned(), tutorial::GUIDE.to_owned())),
                buttons,
                list: false,
            });
        }
        let game = &self.snap.game;
        let crew = self.crew();
        match self.overlay {
            Overlay::None => None,
            Overlay::Jobs => Some(self.jobs_card()),
            Overlay::Ask => Some(Card {
                title: game.level.name.clone(),
                crew: Some(crew),
                lines: vec![
                    format!("{}, {}:", copy::crew(crew), copy::crew_role(crew)),
                    copy::ask(game.level.number).to_owned(),
                ],
                stars: None,
                link: None,
                buttons: vec![Button::new(copy::TOUR_DONE, Action::Close, true)],
                list: false,
            }),
            Overlay::Menu => Some(Card {
                title: copy::PAUSED.to_owned(),
                crew: None,
                lines: Vec::new(),
                stars: None,
                link: Some((copy::GUIDE.to_owned(), tutorial::GUIDE.to_owned())),
                buttons: vec![
                    Button::new(copy::RESUME, Action::Resume, true),
                    Button::new(copy::RESTART, Action::Restart, false),
                    Button::new(copy::HOW_TO_PLAY, Action::Tour, false),
                    Button::new(copy::sound(self.progress.sound), Action::ToggleSound, false),
                    Button::new(
                        copy::contrast(self.contrast()),
                        Action::ToggleContrast,
                        false,
                    ),
                    Button::new(copy::ALL_JOBS, Action::AllJobs, false),
                    Button::new(copy::HOME, Action::Home, false),
                ],
                list: true,
            }),
            Overlay::Results => Some(match game.status {
                Status::Won => {
                    let mut buttons = Vec::new();
                    if self.job < level::COUNT {
                        buttons.push(Button::new(copy::NEXT_JOB, Action::NextJob, true));
                    }
                    buttons.push(Button::new(
                        copy::TRY_AGAIN,
                        Action::Fix,
                        self.job >= level::COUNT,
                    ));
                    buttons.push(Button::new(copy::ALL_JOBS, Action::AllJobs, false));
                    Card {
                        title: copy::JOB_DONE.to_owned(),
                        crew: Some(crew),
                        lines: vec![
                            copy::thanks(crew),
                            copy::used_line(game.build.used(), game.level.par, game.stars),
                        ],
                        stars: Some(game.stars),
                        link: None,
                        buttons,
                        list: false,
                    }
                }
                Status::Failed(why) => Card {
                    title: copy::NOT_YET.to_owned(),
                    crew: Some(crew),
                    lines: vec![copy::fail(why).to_owned()],
                    stars: None,
                    link: None,
                    buttons: vec![
                        Button::new(copy::FIX, Action::Fix, true),
                        Button::new(copy::ALL_JOBS, Action::AllJobs, false),
                    ],
                    list: false,
                },
                _ => return None,
            }),
        }
    }

    fn hint(&self) -> String {
        let game = &self.snap.game;
        if game.status != Status::Building || self.overlay != Overlay::None || self.tour.is_some() {
            return String::new();
        }
        if self.time < self.nope_until {
            return copy::NO_FIT.to_owned();
        }
        if let Some(held) = self.held() {
            return copy::piece(held.kind).to_owned();
        }
        if game.build.used() == 0 {
            return if self.hud.touch() {
                copy::DRAG_HINT.to_owned()
            } else {
                format!("{} {}", copy::DRAG_HINT, copy::KEYS_HINT)
            };
        }
        String::new()
    }

    fn update_hud(&mut self) {
        let crew = self.crew();
        let number = self.snap.game.level.number;
        self.hud.set_job(number, crew);
        self.hud
            .set_playing(self.overlay != Overlay::Jobs && !self.options.kit);
        let tray = self.snap.game.tray.clone();
        let focus = if self.keyboard && self.snap.carrying.is_none() && self.building() {
            let slots = self.snap.slots();
            match slots.get(self.snap.slot) {
                Some(Slot::Piece(kind)) => tray.iter().position(|(k, _)| k == kind),
                Some(Slot::Go) => Some(tray.len()),
                None => None,
            }
        } else {
            None
        };
        self.hud.set_tray(&tray, focus);
        self.hud.set_mode(match self.snap.game.status {
            Status::Building => Mode::Building,
            Status::Running => Mode::Running,
            _ => Mode::Over,
        });
        let elements = self.snap.hud();
        let (low, high) = self.snap.game.level.gravity;
        let top = high * 1.25;
        let band = (low > 0.0 || high > 0.0).then(|| ((low / top) as f32, (high / top) as f32));
        let storm = self
            .snap
            .game
            .storm_left()
            .map_or(0, |ticks| ticks.div_ceil(HZ));
        self.hud.set_elements(&elements, band, storm);
        if let Some((felt, _)) = self.snap.gravity() {
            self.hud.set_gravity_needle((felt / top) as f32);
        }
        let hint = self.hint();
        self.hud.set_hint(&hint);
        let card = self.card();
        self.hud.set_card(card);
    }

    fn resize(&mut self) -> (f32, f32) {
        let ratio = self.window.device_pixel_ratio().min(2.0) as f32;
        let rect = self.canvas.get_bounding_client_rect();
        let (w, h) = (rect.width() as f32, rect.height() as f32);
        let (pw, ph) = ((w * ratio) as u32, (h * ratio) as u32);
        if self.canvas.width() != pw || self.canvas.height() != ph {
            self.canvas.set_width(pw.max(1));
            self.canvas.set_height(ph.max(1));
        }
        (w.max(1.0), h.max(1.0))
    }

    /// Frames the build: its bounds fitted between the HUD's top and the
    /// tray, eased toward while the build changes.
    fn frame_view(&mut self, w: f32, h: f32, dt: f32) {
        if self.hand.is_some() && self.view_ready {
            return;
        }
        let (low, high) = if self.options.kit {
            (Vec2::new(-1.0, -1.5), Vec2::new(13.0, 8.0))
        } else {
            scene::bounds(&self.snap.game)
        };
        // Below the job card, which wraps taller on a narrow screen.
        let top = if w < 600.0 { 140.0 } else { 90.0 };
        let tray = self.hud.tray_top();
        let bottom = if tray <= 0.0 {
            40.0
        } else {
            (h - tray + 30.0).clamp(60.0, h * 0.4)
        };
        let room = Vec2::new(w - 24.0, h - top - bottom).max(Vec2::splat(40.0));
        let size = (high - low).max(Vec2::splat(1.0));
        let scale = (room.x / size.x).min(room.y / size.y).clamp(12.0, 140.0);
        let mid = (low + high) / 2.0;
        // The free space's middle sits (top - bottom) / 2 pixels below the
        // canvas's middle.
        let centre = Vec2::new(mid.x, mid.y + (top - bottom) / 2.0 / scale);
        let target = View { centre, scale };
        if !self.view_ready {
            self.view = target;
            self.view_ready = true;
            return;
        }
        let k = 1.0 - (-dt * 3.0).exp();
        self.view.centre += (target.centre - self.view.centre) * k;
        self.view.scale += (target.scale - self.view.scale) * k;
    }

    fn frame(&mut self, now: f64) {
        let dt = if self.last == 0.0 {
            STEP
        } else {
            ((now - self.last) / 1000.0).min(0.25)
        };
        self.last = now;
        self.time += dt;
        for action in self.hud.take() {
            self.act(action);
        }
        #[cfg(feature = "autoplay")]
        self.autoplay();
        let playing = self.overlay == Overlay::None && self.tour.is_none() && !self.options.kit;
        if playing {
            self.carry += dt;
            let mut steps = 0;
            while self.carry >= STEP && steps < MAX_STEPS {
                self.carry -= STEP;
                steps += 1;
                self.tick();
            }
            if steps == MAX_STEPS {
                self.carry = 0.0;
            }
        } else if !self.pending.is_empty() && self.overlay == Overlay::None {
            self.tick();
        }
        let over = matches!(self.snap.game.status, Status::Won | Status::Failed(_));
        if over {
            let at = *self.over_at.get_or_insert(self.time);
            if self.time - at > RESULTS_AFTER && self.overlay == Overlay::None {
                self.overlay = Overlay::Results;
            }
            if !self.recorded && self.snap.game.status == Status::Won {
                self.recorded = true;
                let game = &self.snap.game;
                self.progress
                    .record(game.level.number, game.stars, game.build.used());
                save_progress(&self.window, &self.progress);
            }
        } else {
            self.over_at = None;
            if self.overlay == Overlay::Results {
                self.overlay = Overlay::None;
            }
        }
        for spark in &mut self.sparks {
            spark.age += dt as f32 * 1.4;
            spark.at += spark.velocity * dt as f32;
        }
        self.sparks.retain(|s| s.age < 1.0);
        self.update_hud();
        let (w, h) = self.resize();
        self.frame_view(w, h, dt as f32);
        self.render(w, h);
    }

    #[cfg(feature = "autoplay")]
    fn autoplay(&mut self) {
        if self.overlay != Overlay::None || self.tour.is_some() {
            return;
        }
        let time = self.time;
        let Some((plan, next, at)) = &mut self.auto else {
            return;
        };
        if self.snap.game.status != Status::Building || time < *at {
            return;
        }
        if let Some(input) = plan.get(*next) {
            self.pending.push(*input);
            *next += 1;
            *at = time + 0.35;
        }
    }

    fn render(&mut self, w: f32, h: f32) {
        let gl = &self.gpu.gl;
        gl.viewport(
            0,
            0,
            self.canvas.width() as i32,
            self.canvas.height() as i32,
        );
        let contrast = self.contrast();
        if contrast {
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
        } else {
            gl.clear_color(0.027, 0.043, 0.118, 1.0);
        }
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        let View { centre, scale } = self.view;
        let half = (FOV / 2.0).to_radians().tan();
        let distance = (h / 2.0) / (scale * half);
        let eye = Vec3::new(centre.x, centre.y, distance);
        let projection = Mat4::perspective_rh_gl(FOV.to_radians(), w / h, 0.3, distance + 220.0)
            * Mat4::look_at_rh(eye, Vec3::new(centre.x, centre.y, 0.0), Vec3::Y);
        gl.uniform_matrix4fv_with_f32_array(
            self.gpu.u_vp.as_ref(),
            false,
            &projection.to_cols_array(),
        );
        // The view's numbers, for checks that drive the page.
        let rect = self.canvas.get_bounding_client_rect();
        let view = format!(
            "{:.3} {:.3} {:.3} {:.1} {:.1}",
            centre.x,
            centre.y,
            scale,
            rect.left(),
            rect.top()
        );
        if view != self.shown_view {
            let _ = self.canvas.set_attribute("data-view", &view);
            self.shown_view = view;
        }
        if !contrast {
            self.gpu.draw(&self.backdrop.0, false, 1.0);
            self.gpu.draw(&self.backdrop.1, true, 1.0);
        }
        let drawn = if self.options.kit {
            kit_sheet(self.time as f32)
        } else {
            scene::draw(&Frame {
                game: &self.snap.game,
                time: self.time as f32,
                held: self.held(),
                contrast,
                sparks: &self.sparks,
            })
        };
        self.gpu.refill(&mut self.lit, &drawn.lit);
        self.gpu.refill(&mut self.glow, &drawn.glow);
        self.gpu.refill(&mut self.ghost, &drawn.ghost);
        self.gpu.draw(&self.lit, false, 1.0);
        self.gpu.draw(&self.glow, true, 1.0);
        let gl = &self.gpu.gl;
        gl.enable(Gl::BLEND);
        gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        self.gpu.draw(&self.ghost, false, 0.5);
        let gl = &self.gpu.gl;
        gl.depth_mask(true);
        gl.disable(Gl::BLEND);
    }
}

/// The kit sheet (`#kit`): every piece, a leaning square, struts and
/// bolts, the crew, the ship and the beacon, for review.
fn kit_sheet(time: f32) -> scene::Drawn {
    let mut drawn = scene::Drawn::default();
    let tiles = [
        PieceKind::Square,
        PieceKind::Triangle,
        PieceKind::Wedge,
        PieceKind::Panel,
        PieceKind::Thruster,
        PieceKind::Cargo,
        PieceKind::Pod,
        PieceKind::Module,
        PieceKind::Solar,
    ];
    for (i, kind) in tiles.iter().enumerate() {
        let at = V::new(0.8 + i as f64 * 1.5, 6.0);
        let corners: Vec<Vec2> = snap_rules::build::Build::held(*kind, at, 0)
            .into_iter()
            .map(v2)
            .collect();
        kit::tile(&mut drawn.lit, *kind, &corners, Style::default());
        if *kind == PieceKind::Thruster {
            kit::flame(
                &mut drawn.glow,
                &corners,
                1.0,
                (time * 20.0).sin() * 0.5 + 0.5,
            );
        }
    }
    let leaning = [
        Vec2::new(0.3, 3.0),
        Vec2::new(1.3, 3.0),
        Vec2::new(1.65, 3.94),
        Vec2::new(0.65, 3.94),
    ];
    kit::tile(
        &mut drawn.lit,
        PieceKind::Square,
        &leaning,
        Style::default(),
    );
    let braced = [
        Vec2::new(2.3, 3.0),
        Vec2::new(3.3, 3.0),
        Vec2::new(3.3, 4.0),
        Vec2::new(2.3, 4.0),
    ];
    kit::tile(&mut drawn.lit, PieceKind::Square, &braced, Style::default());
    kit::strut(&mut drawn.lit, braced[0], braced[2], Style::default());
    kit::bolt(&mut drawn.lit, Vec2::new(3.3, 3.5), Style::default());
    let popped: Vec<Vec2> = braced.iter().map(|p| *p + Vec2::new(2.0, 0.0)).collect();
    kit::tile(
        &mut drawn.lit,
        PieceKind::Square,
        &popped,
        Style {
            broken: true,
            ..Style::default()
        },
    );
    kit::ship(
        &mut drawn.lit,
        &mut drawn.glow,
        Vec2::new(6.5, 2.9),
        Vec2::new(9.1, 3.7),
        1.0,
    );
    kit::beacon(&mut drawn.glow, Vec2::new(11.0, 3.4), 0.8, time);
    for (i, crew) in Crew::ALL.iter().enumerate() {
        kit::robot(
            &mut drawn.lit,
            *crew,
            Vec2::new(1.0 + i as f32 * 2.6, 0.4),
            time,
            if i % 2 == 0 {
                Mood::Waiting
            } else {
                Mood::Cheering
            },
        );
    }
    drawn
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
            ("background", "#101630"),
            ("color", "#f4f7ff"),
            ("border-radius", "12px"),
        ],
    );
    let _ = parent.append_child(&note);
}

pub fn start() {
    let window = web_sys::window().expect("a window");
    let document = window.document().expect("a document");
    let canvas: HtmlCanvasElement = match document.get_element_by_id("snap-canvas") {
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
                    ("background", "#070b1e"),
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
    if let Some(status) = document.get_element_by_id("snap-status") {
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
    let touch = window
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let hud = Hud::new(&document, &parent, touch);
    let options = Options::parse(&window.location().hash().unwrap_or_default());
    let progress = load_progress(&window);
    let (back_lit, back_glow) = kit::backdrop();
    let backdrop = (gpu.upload(&back_lit), gpu.upload(&back_glow));
    let empty = Mesh::new();
    let lit = gpu.upload(&empty);
    let glow = gpu.upload(&empty);
    let ghost = gpu.upload(&empty);
    let sound = Sound::new(progress.sound && !options.quiet);
    let mut app = App {
        window: window.clone(),
        canvas: canvas.clone(),
        gpu,
        hud,
        sound,
        options,
        backdrop,
        lit,
        glow,
        ghost,
        progress,
        snap: SnapGame::new(level::job(1)),
        job: 1,
        overlay: Overlay::Jobs,
        tour: None,
        hand: None,
        keyboard: false,
        view: View {
            centre: Vec2::ZERO,
            scale: 60.0,
        },
        view_ready: false,
        sparks: Vec::new(),
        pending: Vec::new(),
        last: 0.0,
        carry: 0.0,
        time: 0.0,
        over_at: None,
        recorded: false,
        shown_view: String::new(),
        nope_until: 0.0,
        #[cfg(feature = "autoplay")]
        auto: None,
    };
    match app.options.job {
        Some(job) => app.start_job(job, false),
        None => app.overlay = Overlay::Jobs,
    }
    if app.options.kit {
        app.overlay = Overlay::None;
        show(&app.hud.root, false);
    }
    if tutorial::opens_by_itself(
        tutorial_seen(&window).as_deref(),
        app.options.job.is_some(),
        app.options.kit,
    ) {
        app.tour = Some(Tour::new());
    }
    let app = Rc::new(RefCell::new(app));

    {
        let app = app.clone();
        listen::<KeyboardEvent>(window.as_ref(), "keydown", move |event| {
            app.borrow_mut().key(&event);
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(canvas.as_ref(), "pointerdown", move |event| {
            event.prevent_default();
            app.borrow_mut()
                .pointer_down(event.client_x() as f32, event.client_y() as f32);
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(window.as_ref(), "pointermove", move |event| {
            app.borrow_mut()
                .pointer_move(event.client_x() as f32, event.client_y() as f32);
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(window.as_ref(), "pointerup", move |event| {
            app.borrow_mut()
                .pointer_up(event.client_x() as f32, event.client_y() as f32);
        });
    }
    {
        let app = app.clone();
        listen::<PointerEvent>(window.as_ref(), "pointercancel", move |_| {
            let mut app = app.borrow_mut();
            if app.hand.is_some_and(|h| h.grip == Grip::Drag) {
                app.hand = None;
            }
        });
    }
    {
        let app = app.clone();
        listen::<WheelEvent>(canvas.as_ref(), "wheel", move |event| {
            let mut app = app.borrow_mut();
            if let Some(hand) = &mut app.hand {
                event.prevent_default();
                let turns = snap_rules::TURNS;
                hand.turn = if event.delta_y() > 0.0 {
                    (hand.turn + 1) % turns
                } else {
                    (hand.turn + turns - 1) % turns
                };
            }
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
