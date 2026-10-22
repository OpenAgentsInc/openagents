//! The browser side: the frame loop, input, lighting, triggers, sounds,
//! the live client, and the words over the view.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use glam::{Mat4, Quat, Vec3};
use haunt_assets::{Catalog, Kind, Surface};
use haunt_rules::doc::{Action, Area, AreaKind, House};
use haunt_rules::trigger::Triggers;
use haunt_rules::walk::{Walls, area_at, left_by_exit};
use js_sys::Reflect;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    HtmlCanvasElement, KeyboardEvent, PointerEvent, WebGl2RenderingContext as Gl, WebGlTexture,
};

use crate::arch::{self, token};
use crate::gpu::{Camera, Gpu, GpuMesh, Lighting};
use crate::live::{Assets, Live, Version};
use crate::scene::{self, Placed};
use crate::sound::{Levels, Sound};
use crate::ui::{Ui, View};
use crate::walker::{Move, Walker};

/// Seconds after arrival before the cue plays behind you.
const CUE_AFTER: f32 = 2.0;
/// How far ahead the view sees, m.
const FAR: f32 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Title,
    Playing,
    Menu,
    Ended,
}

/// A trigger's change to an area's light, until a time.
#[derive(Clone, Debug)]
struct LightOverride {
    preset: Option<String>,
    dim: Option<f32>,
    until: Option<f32>,
}

/// One area's GPU geometry.
struct AreaGpu {
    arch: GpuMesh,
    props: Vec<Placed>,
}

struct App {
    window: web_sys::Window,
    canvas: HtmlCanvasElement,
    gpu: Gpu,
    ui: Ui,
    ui_texture: Option<WebGlTexture>,
    sound: Sound,
    catalog: &'static Catalog,
    house: House,
    walls: Walls,
    walker: Walker,
    triggers: Triggers,
    areas: BTreeMap<String, AreaGpu>,
    scenery: Option<GpuMesh>,
    meshes: HashMap<String, GpuMesh>,
    cards: HashMap<String, (WebGlTexture, f32)>,
    mode: Mode,
    time: f32,
    last: f64,
    arrived_at: f32,
    cue_played: bool,
    keys: BTreeSet<String>,
    mouse: [f32; 2],
    pointer_locked: bool,
    touch_move: Option<(i32, [f64; 2], [f64; 2])>,
    touch_look: Option<(i32, [f64; 2])>,
    gentle: bool,
    muted: bool,
    music_scale: f32,
    note: (String, f32),
    overrides: BTreeMap<String, LightOverride>,
    states: BTreeMap<String, (String, f32)>,
    flash: f32,
    next_flash: f32,
    flicker_until: f32,
    area: Option<String>,
    fired: Vec<String>,
    live: Option<Live>,
    follow: Option<crate::follow::Follow>,
    /// The mic button, in a house Game Lab shows on the Grid.
    mic: Option<crate::mic::Mic>,
    /// The library page shown, in library mode.
    library: Option<usize>,
    /// The sound test's progress: the next sound and when, and what played.
    sound_test: Option<(usize, f32)>,
    played: Vec<serde_json::Value>,
    /// The last frame's camera, for placing words.
    last_view: Mat4,
    bot: Option<usize>,
    route: Vec<[f32; 2]>,
    areas_seen: Vec<String>,
    applied: Vec<serde_json::Value>,
    published: u32,
    seed: u32,
}

fn now(window: &web_sys::Window) -> f64 {
    window.performance().map_or(0.0, |p| p.now())
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

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

impl App {
    fn new(
        window: web_sys::Window,
        canvas: HtmlCanvasElement,
        gl: Gl,
        house: House,
    ) -> Result<Self, String> {
        let document = window.document().ok_or("no document")?;
        let gpu = Gpu::new(gl)?;
        let ui = Ui::new(&document).ok_or("no 2D canvas")?;
        let catalog = haunt_assets::catalog();
        let start = house.arrival_point().unwrap_or([0.0, 0.0]);
        let walker = Walker::new(start, house.arrival.yaw);
        let mut app = Self {
            window,
            canvas,
            gpu,
            ui,
            ui_texture: None,
            sound: Sound::new(),
            catalog,
            walls: Walls::of(&house, catalog),
            walker,
            triggers: Triggers::new(),
            areas: BTreeMap::new(),
            scenery: None,
            meshes: HashMap::new(),
            cards: HashMap::new(),
            mode: Mode::Title,
            time: 0.0,
            last: 0.0,
            arrived_at: 0.0,
            cue_played: false,
            keys: BTreeSet::new(),
            mouse: [0.0; 2],
            pointer_locked: false,
            touch_move: None,
            touch_look: None,
            gentle: false,
            muted: false,
            music_scale: 1.0,
            note: (String::new(), 0.0),
            overrides: BTreeMap::new(),
            states: BTreeMap::new(),
            flash: 0.0,
            next_flash: 4.0,
            flicker_until: 0.0,
            area: None,
            fired: Vec::new(),
            live: None,
            follow: None,
            mic: None,
            library: None,
            sound_test: None,
            played: Vec::new(),
            last_view: Mat4::IDENTITY,
            bot: None,
            route: Vec::new(),
            areas_seen: Vec::new(),
            applied: Vec::new(),
            published: 0,
            seed: 7,
            house,
        };
        app.rebuild_all();
        Ok(app)
    }

    /// Builds every area's geometry, the scenery, and any mesh or card the
    /// house needs that isn't loaded yet.
    fn rebuild_all(&mut self) {
        let ids: Vec<String> = self.house.areas.iter().map(|a| a.id.clone()).collect();
        for id in &ids {
            self.rebuild_area(id);
        }
        self.areas.retain(|id, _| ids.contains(id));
        if let Some(old) = self.scenery.take() {
            self.gpu.free(old);
        }
        self.scenery = self.gpu.upload_arch(&arch::scenery(&self.house).data);
        self.walls = Walls::of(&self.house, self.catalog);
    }

    fn rebuild_area(&mut self, id: &str) {
        let Some(area) = self.house.area(id).cloned() else {
            return;
        };
        let geometry = arch::area(&self.house, &area, self.catalog);
        let mut props = scene::props(&area, self.catalog);
        props.extend(scene::yard_scenery(&area));
        for p in &props {
            self.ensure_asset(p);
        }
        if let Some(gpu) = self.gpu.upload_arch(&geometry.data)
            && let Some(old) = self
                .areas
                .insert(area.id.clone(), AreaGpu { arch: gpu, props })
        {
            self.gpu.free(old.arch);
        }
    }

    /// Makes a placed thing's mesh or card in the page when no chunk
    /// brought it.
    fn ensure_asset(&mut self, p: &Placed) {
        match p.kind {
            Kind::Mesh => {
                let key = p.mesh_key();
                if !self.meshes.contains_key(&key) {
                    let state = key
                        .split('#')
                        .nth(1)
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&p.state);
                    if let Some(mesh) = haunt_assets::meshes::build(&p.asset, state)
                        && let Some(gpu) = self.gpu.upload_mesh(&mesh.data)
                    {
                        self.meshes.insert(key, gpu);
                    }
                }
            }
            Kind::Card => {
                if !self.cards.contains_key(&p.asset)
                    && let Some(image) = haunt_assets::cards::draw(&p.asset)
                {
                    self.install_card(&p.asset, &image);
                }
            }
            Kind::Effect => {
                for part in ["", "drop"] {
                    let key = format!("effect:{}#{part}", p.asset);
                    if !self.meshes.contains_key(&key)
                        && let Some(mesh) = haunt_assets::meshes::effect(&p.asset, part)
                        && let Some(gpu) = self.gpu.upload_mesh(&mesh.data)
                    {
                        self.meshes.insert(key, gpu);
                    }
                }
            }
        }
    }

    fn install_card(&mut self, id: &str, image: &icecream_zone::image::Image) {
        if let Some(t) = self.gpu.texture(
            u32::from(image.width),
            u32::from(image.height),
            &image.rgba(),
        ) {
            let aspect = f32::from(image.width) / f32::from(image.height).max(1.0);
            self.cards.insert(id.to_owned(), (t, aspect));
        }
    }

    fn install_assets(&mut self, assets: Assets) {
        for part in assets.meshes {
            if let Some(gpu) = self.gpu.upload_mesh(&part.data)
                && let Some(old) = self.meshes.insert(part.name, gpu)
            {
                self.gpu.free(old);
            }
        }
        for (id, image) in assets.cards {
            self.install_card(&id, &image);
        }
    }

    fn levels(&self) -> Levels {
        let a = &self.house.audio;
        Levels {
            master: if self.muted { 0.0 } else { a.master },
            music: (a.music * self.music_scale).clamp(0.0, 1.0),
            ambience: a.ambience,
            sfx: a.sfx,
            voice: a.voice,
        }
    }

    fn say(&mut self, text: &str) {
        self.note = (text.to_owned(), self.time + 2.5);
    }

    fn begin(&mut self) {
        self.sound.wake();
        self.sound.set_levels(self.levels(), 0.0);
        if self.mode == Mode::Title || self.mode == Mode::Ended {
            self.restart();
        }
        self.mode = Mode::Playing;
        let _ = self.canvas.focus();
        if self.bot.is_none() {
            self.canvas.request_pointer_lock();
        }
    }

    fn restart(&mut self) {
        let start = self.house.arrival_point().unwrap_or([0.0, 0.0]);
        self.walker = Walker::new(start, self.house.arrival.yaw);
        self.triggers = Triggers::new();
        self.arrived_at = self.time;
        self.cue_played = false;
        self.overrides.clear();
        self.states.clear();
        self.area = None;
        self.fired.clear();
        self.areas_seen.clear();
        self.sound.music(false);
        if self.bot.is_some() {
            self.bot = Some(0);
        }
    }

    fn back_to_grid(&self) {
        let parent = self.window.parent().ok().flatten();
        let framed = parent
            .as_ref()
            .is_some_and(|p| !js_sys::Object::is(p, &self.window));
        if let (true, Some(parent)) = (framed, parent) {
            let message = js_sys::Object::new();
            let _ = Reflect::set(&message, &"type".into(), &"hosted-back".into());
            let origin = self.window.location().origin().unwrap_or_default();
            let _ = parent.post_message(&message, &origin);
        } else {
            let here = self.window.location().origin().unwrap_or_default();
            let came = self
                .window
                .document()
                .map(|d| d.referrer())
                .unwrap_or_default();
            if !here.is_empty() && came.starts_with(&here) {
                let _ = self.window.history().and_then(|h| h.back());
            } else {
                let _ = self.window.location().set_href("/");
            }
        }
    }

    fn act(&mut self, action: &str) {
        match action {
            "resume" | "begin" => self.begin(),
            "again" => {
                self.mode = Mode::Title;
                self.begin();
            }
            "gentle" => {
                self.gentle = !self.gentle;
                self.say(if self.gentle {
                    "Gentle mode is on: softer edges and a brighter lantern"
                } else {
                    "Gentle mode is off"
                });
            }
            "quieter" => {
                self.music_scale = (self.music_scale - 0.25).max(0.0);
                self.sound.set_levels(self.levels(), 0.1);
                self.say("Music quieter");
            }
            "louder" => {
                self.music_scale = (self.music_scale + 0.25).min(1.5);
                self.sound.set_levels(self.levels(), 0.1);
                self.say("Music louder");
            }
            "grid" => self.back_to_grid(),
            "mic" => {
                if let Some(mic) = &self.mic {
                    self.sound.wake();
                    mic.toggle();
                    if mic.listening() {
                        self.say("Listening… say what to change");
                    }
                }
            }
            _ => {}
        }
    }

    fn key(&mut self, name: &str, down: bool) {
        if !down {
            self.keys.remove(name);
            return;
        }
        self.keys.insert(name.to_owned());
        if let Some(page) = self.library {
            let pages = scene::library_pages(self.catalog);
            match name {
                "ArrowRight" => self.set_library((page + 1) % pages),
                "ArrowLeft" => self.set_library((page + pages - 1) % pages),
                _ => {}
            }
            return;
        }
        match name {
            "Enter" | " " if matches!(self.mode, Mode::Title | Mode::Ended) => self.begin(),
            "Escape" | "Tab" if self.mode == Mode::Playing => self.mode = Mode::Menu,
            "Escape" if self.mode == Mode::Menu => self.begin(),
            "g" | "G" => self.act("gentle"),
            "v" | "V" => self.act("mic"),
            "m" | "M" => {
                self.muted = !self.muted;
                self.sound.set_levels(self.levels(), 0.05);
                self.say(if self.muted { "Sound off" } else { "Sound on" });
            }
            "-" | "_" => self.act("quieter"),
            "=" | "+" => self.act("louder"),
            "r" | "R" if self.mode == Mode::Ended => self.act("again"),
            "b" | "B" if matches!(self.mode, Mode::Ended | Mode::Menu) => self.act("grid"),
            _ => {}
        }
    }

    fn held(&self, names: &[&str]) -> bool {
        names.iter().any(|n| self.keys.contains(*n))
    }

    fn input(&mut self, dt: f32) -> Move {
        let mut m = Move::default();
        if self.mode != Mode::Playing {
            return m;
        }
        if self.held(&["w", "W", "ArrowUp"]) {
            m.forward += 1.0;
        }
        if self.held(&["s", "S", "ArrowDown"]) {
            m.forward -= 1.0;
        }
        if self.held(&["d", "D"]) {
            m.strafe += 1.0;
        }
        if self.held(&["a", "A"]) {
            m.strafe -= 1.0;
        }
        if self.held(&["ArrowRight"]) {
            m.turn += 140.0 * dt;
        }
        if self.held(&["ArrowLeft"]) {
            m.turn -= 140.0 * dt;
        }
        m.run = self.held(&["Shift"]);
        m.turn += self.mouse[0] * 0.12;
        m.look -= self.mouse[1] * 0.12;
        self.mouse = [0.0; 2];
        if let Some((_, centre, thumb)) = self.touch_move {
            let dx = ((thumb[0] - centre[0]) / 52.0).clamp(-1.0, 1.0) as f32;
            let dy = ((thumb[1] - centre[1]) / 52.0).clamp(-1.0, 1.0) as f32;
            m.forward -= dy;
            m.strafe += dx;
            m.run = dx.hypot(dy) > 0.95;
        }
        m
    }

    /// The bot's input: wait out the cue facing away, turn around, then
    /// follow the route through every area and out the back.
    fn bot_input(&mut self, dt: f32) -> Option<Move> {
        let step = self.bot?;
        if self.mode != Mode::Playing {
            return Some(Move::default());
        }
        let since = self.time - self.arrived_at;
        if since < CUE_AFTER + 0.8 {
            return Some(Move::default());
        }
        if step >= self.route.len() {
            return Some(Move {
                forward: 1.0,
                ..Move::default()
            });
        }
        let target = self.route[step];
        let facing = self.walker.face(target, 220.0, dt);
        if (self.walker.pos[0] - target[0]).hypot(self.walker.pos[1] - target[1]) < 0.45 {
            self.bot = Some(step + 1);
        }
        Some(Move {
            forward: if facing || step > 0 { 1.0 } else { 0.0 },
            ..Move::default()
        })
    }

    fn area_lighting(&self, area: &Area) -> Lighting {
        let o = self
            .overrides
            .get(&area.id)
            .filter(|o| o.until.is_none_or(|u| u > self.time));
        let preset_id = o
            .and_then(|o| o.preset.clone())
            .unwrap_or_else(|| area.light.preset.clone());
        let preset = self.catalog.preset(&preset_id);
        let dim = o.and_then(|o| o.dim).unwrap_or(0.0).max(area.light.dim);
        let keep = 1.0 - dim.clamp(0.0, 1.0);
        let (amb_token, mut level) = match &area.light.ambient {
            Some(a) => (a.color.clone(), a.level),
            None => preset.map_or(("plum".into(), 0.1), |p| (p.ambient.clone(), p.level)),
        };
        let t = self.time;
        let wobble = |seed: f32, amount: f32| {
            1.0 - amount * (0.5 + 0.5 * ((t * 9.0 + seed).sin() * (t * 5.7 + seed * 1.3).sin()))
        };
        if preset_id == "jack-o-glow" {
            level *= wobble(1.0, 0.35);
        }
        if self.flicker_until > t {
            level *= wobble(3.0, 0.8);
        }
        if self.gentle {
            level += 0.12;
        }
        let c = token(&amb_token);
        let ambient = [
            c[0] * level * keep,
            c[1] * level * keep,
            c[2] * level * keep,
        ];
        let mut lights = Vec::new();
        for (i, l) in area.light.lights.iter().enumerate() {
            let [x, z] = area.world(l.at);
            let col = token(&l.color);
            let k = l.intensity * keep * wobble(i as f32 * 2.3, l.flicker * 0.6);
            lights.push((
                [x, l.height, z, l.radius],
                [col[0] * k, col[1] * k, col[2] * k],
            ));
        }
        // Lit pumpkins and candles glow.
        if let Some(g) = self.areas.get(&area.id) {
            for p in &g.props {
                let glow = match (p.asset.as_str(), self.state_of(p).as_str()) {
                    ("jack-o-lantern", _) => Some(("pumpkin", 0.7, 3.5)),
                    ("candle", "lit") | ("candelabra", "lit") => Some(("candle", 0.6, 3.0)),
                    _ => None,
                };
                if let Some((tok, k, r)) = glow
                    && lights.len() < 8
                {
                    let col = token(tok);
                    let k = k * keep * wobble(p.at.x * 3.1, 0.3);
                    lights.push((
                        [p.at.x, p.at.y + 0.4, p.at.z, r],
                        [col[0] * k, col[1] * k, col[2] * k],
                    ));
                }
            }
        }
        let outdoors = area.ceiling.is_none();
        let moon = if outdoors {
            let d = token("dusk");
            [d[0] * 0.3, d[1] * 0.3, d[2] * 0.35]
        } else {
            [0.0; 3]
        };
        let flash = if preset_id == "lightning" || self.flash > 0.0 {
            self.flash
        } else {
            0.0
        };
        Lighting {
            ambient,
            lights,
            moon_dir: Vec3::new(0.2, -0.55, -0.8).normalize().to_array(),
            moon,
            flash,
            lantern: if self.house.look.lantern { 1.0 } else { 0.0 },
            lantern_cos: if self.gentle { 0.78 } else { 0.86 },
            fog: if outdoors {
                [0.10, 0.11, 0.17]
            } else {
                [0.09, 0.07, 0.12]
            },
            fog_range: if self.library.is_some() {
                [40.0, 90.0]
            } else if outdoors {
                [18.0, 70.0]
            } else {
                [7.0, 22.0]
            },
            shadow: [0.42, 0.36, 0.56],
        }
    }

    fn state_of(&self, p: &Placed) -> String {
        self.states
            .get(&p.id)
            .map_or_else(|| p.state.clone(), |(s, _)| s.clone())
    }

    fn apply_actions(&mut self, area_id: &str, prop: Option<&str>, actions: &[Action]) {
        let area = self.house.area(area_id).cloned();
        let at_prop = prop.and_then(|id| {
            self.areas
                .get(area_id)
                .and_then(|g| g.props.iter().find(|p| p.id == id))
                .map(|p| p.at + Vec3::Y * 1.0)
        });
        let centre = area.as_ref().map_or(Vec3::ZERO, |a| {
            let [x, z] = a.centre();
            Vec3::new(x, 1.6, z)
        });
        for action in actions {
            match action {
                Action::Sfx(id) => {
                    self.sound.play(id, Some(at_prop.unwrap_or(centre)));
                }
                Action::Light(change) => {
                    self.overrides.insert(
                        area_id.to_owned(),
                        LightOverride {
                            preset: change.preset.clone(),
                            dim: change.dim,
                            until: change.seconds.map(|s| self.time + s),
                        },
                    );
                }
                Action::State(change) => {
                    self.states
                        .insert(change.prop.clone(), (change.to.clone(), self.time));
                }
                Action::Effect(effect) => match effect.id.as_str() {
                    "lightning-flash" => self.flash = 1.3,
                    "light-flicker" => self.flicker_until = self.time + 1.5,
                    "door-slam" => {
                        self.sound
                            .play("door-slam", Some(at_prop.unwrap_or(centre)));
                    }
                    _ => {}
                },
            }
        }
    }

    fn step(&mut self, dt: f32) {
        self.time += dt;
        // The arrival cue, behind you.
        if self.mode == Mode::Playing
            && !self.cue_played
            && self.time - self.arrived_at >= CUE_AFTER
        {
            self.cue_played = true;
            let sounds = self
                .catalog
                .cue(&self.house.arrival.cue)
                .map(|c| c.sounds.clone())
                .unwrap_or_default();
            let behind = self
                .house
                .area(&self.house.arrival.area)
                .and_then(|a| a.doors.first().map(|d| a.door_centre(d)))
                .map_or_else(
                    || {
                        let [hx, hz] = self.walker.heading();
                        Vec3::new(
                            self.walker.pos[0] - hx * 6.0,
                            2.0,
                            self.walker.pos[1] - hz * 6.0,
                        )
                    },
                    |[x, z]| Vec3::new(x, 2.0, z),
                );
            self.sound.cue(&sounds, behind);
            self.sound.music(true);
            self.fired.push("arrival cue".into());
        }
        if self.library.is_some() {
            self.step_sound_test();
            self.sound.tick();
            return;
        }
        let m = match self.bot_input(dt) {
            Some(m) => m,
            None => self.input(dt),
        };
        if self.mode == Mode::Playing {
            self.walker.step(&self.walls, m, dt);
        }
        // Lightning in a lightning area: a double flash, then thunder.
        let here = area_at(&self.house, self.walker.pos).cloned();
        self.flash = (self.flash - dt * 5.0).max(0.0);
        if let Some(a) = &here {
            let preset = self
                .overrides
                .get(&a.id)
                .and_then(|o| o.preset.clone())
                .unwrap_or_else(|| a.light.preset.clone());
            if preset == "lightning" && self.time > self.next_flash {
                self.flash = 1.2;
                self.seed += 1;
                self.next_flash = self.time + 5.0 + hash(self.seed) * 7.0;
                let [x, z] = a.centre();
                self.sound
                    .play("thunder", Some(Vec3::new(x + 20.0, 10.0, z)));
            }
        }
        if self.mode == Mode::Playing {
            let fired = self
                .triggers
                .step(&self.house, self.walker.pos, self.walker.yaw);
            for f in fired {
                self.fired.push(format!("{}#{}", f.area, f.index));
                self.apply_actions(&f.area, f.prop.as_deref(), &f.actions);
            }
        }
        // Entering an area changes the bed and the music's shape.
        let id = here.as_ref().map(|a| a.id.clone());
        if id != self.area {
            self.area = id.clone();
            if let Some(a) = &here {
                if self.areas_seen.last() != Some(&a.id) {
                    self.areas_seen.push(a.id.clone());
                }
                self.sound.bed(a.sound.bed.as_deref());
                let intensity = f32::from(a.intent.intensity[0] + a.intent.intensity[1]) / 2.0;
                self.sound.area(a.sound.music_gain, intensity);
            }
        }
        if self.mode == Mode::Playing && left_by_exit(&self.house, self.walker.pos) {
            self.mode = Mode::Ended;
            self.sound.play("music-box", None);
            if self.pointer_locked
                && let Some(doc) = self.window.document()
            {
                doc.exit_pointer_lock();
            }
        }
        self.sound
            .listener(self.walker.eye(), self.walker.forward());
        self.sound.tick();
    }

    fn view(&self) -> View {
        let mut view = View::default();
        let touch = self.touch_move.is_some() || self.touch_look.is_some();
        match self.mode {
            Mode::Title => {
                view.title = Some((
                    self.house.title.to_uppercase(),
                    "Click to walk in. Sound on.".into(),
                ));
                view.hint = "W A S D or arrows to walk, mouse to look, Shift to run, G for gentle, M for sound".into();
            }
            Mode::Menu => {
                view.menu = Some(vec![
                    ("Keep walking".into(), "resume"),
                    (
                        if self.gentle {
                            "Gentle: on".into()
                        } else {
                            "Gentle: off".into()
                        },
                        "gentle",
                    ),
                    ("Music quieter".into(), "quieter"),
                    ("Music louder".into(), "louder"),
                    ("Back to the Grid".into(), "grid"),
                ]);
            }
            Mode::Ended => {
                view.ending = Some((
                    "You made it out!".into(),
                    vec![
                        ("Walk again".into(), "again"),
                        ("Back to the Grid".into(), "grid"),
                    ],
                ));
            }
            Mode::Playing => {
                if !touch && !self.pointer_locked && self.bot.is_none() {
                    view.hint = "Click to look around. Esc for the menu.".into();
                }
            }
        }
        if self.note.1 > self.time {
            view.note = self.note.0.clone();
        }
        if let Some((_, centre, thumb)) = self.touch_move {
            view.stick = Some((centre, thumb));
        }
        if matches!(self.mode, Mode::Playing | Mode::Menu)
            && let Some(mic) = &self.mic
        {
            view.mic = Some(mic.listening());
        }
        if let Some(page) = self.library {
            view.title = None;
            view.labels = self.library_labels();
            view.hint = format!(
                "The library, page {} of {}: left and right arrows turn the page",
                page + 1,
                scene::library_pages(self.catalog)
            );
        }
        view
    }

    fn resize(&mut self) -> (f32, f32) {
        let scale = self.window.device_pixel_ratio().min(2.0);
        let w = f64::from(self.canvas.client_width()).max(1.0);
        let h = f64::from(self.canvas.client_height()).max(1.0);
        let (pw, ph) = ((w * scale) as u32, (h * scale) as u32);
        if self.canvas.width() != pw || self.canvas.height() != ph {
            self.canvas.set_width(pw);
            self.canvas.set_height(ph);
        }
        self.gpu.scale = scale as f32;
        (pw as f32, ph as f32)
    }

    fn render(&mut self) {
        let (w, h) = self.resize();
        let gl = self.gpu.gl.clone();
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear_color(0.07, 0.06, 0.1, 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        let eye = self.walker.eye();
        let look = self.walker.forward();
        let flip = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
        let flipped = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
        let view = Mat4::look_at_rh(flipped(eye), flipped(eye + look), Vec3::Y) * flip;
        let projection = Mat4::perspective_rh_gl(70f32.to_radians(), w / h, 0.05, FAR);
        let camera = Camera {
            view_projection: projection * view,
            eye,
            look,
            width: w,
            height: h,
            time: self.time,
        };
        self.last_view = camera.view_projection;
        self.gpu
            .sky(&camera, token("plum"), token("dusk"), [0.08, 0.09, 0.13]);
        let yard = self
            .house
            .areas
            .iter()
            .find(|a| a.kind == AreaKind::Yard)
            .cloned();
        if let (Some(scenery), Some(yard)) = (&self.scenery, &yard) {
            let l = self.area_lighting(yard);
            self.gpu.arch(scenery, &camera, &l);
        }
        let areas: Vec<Area> = self.house.areas.clone();
        for area in &areas {
            let Some(g) = self.areas.get(&area.id) else {
                continue;
            };
            let lighting = self.area_lighting(area);
            self.gpu.arch(&g.arch, &camera, &lighting);
            for p in &g.props {
                self.draw_prop(p, &camera, &lighting);
            }
        }
        let vignette = if self.library.is_some() {
            0.0
        } else {
            self.house.look.vignette * if self.gentle { 0.45 } else { 1.0 }
        };
        let fade = match self.mode {
            Mode::Ended => 0.55,
            _ => 0.0,
        };
        self.gpu.post(
            &camera,
            vignette,
            self.house.look.grain,
            fade,
            [0.13, 0.10, 0.18],
        );
        let css = (
            f64::from(self.canvas.client_width()),
            f64::from(self.canvas.client_height()),
        );
        let view = self.view();
        self.ui.draw(&view, css.0, css.1, f64::from(self.gpu.scale));
        if self.ui.dirty {
            self.ui_texture = self
                .gpu
                .canvas_texture(self.ui_texture.as_ref(), &self.ui.canvas);
            self.ui.dirty = false;
        }
        if let Some(t) = &self.ui_texture {
            self.gpu.ui(t);
        }
    }

    fn draw_prop(&self, p: &Placed, camera: &Camera, lighting: &Lighting) {
        let state = self.state_of(p);
        let since = self.states.get(&p.id).map_or(0.0, |(_, t)| self.time - t);
        let t = self.time;
        let surface = self.catalog.asset(&p.asset).map(|a| a.surface);
        match p.kind {
            Kind::Mesh => {
                let mut key = p.mesh_key();
                if state != p.state {
                    let shaped = haunt_assets::meshes::shaped_states(&p.asset);
                    if shaped.contains(&state.as_str()) {
                        key = format!("{}#{state}", p.asset);
                    }
                }
                let Some(mesh) = self
                    .meshes
                    .get(&key)
                    .or_else(|| self.meshes.get(&p.mesh_key()))
                else {
                    return;
                };
                let mut model = p.model();
                if p.asset == "moon" && self.library.is_none() {
                    // The moon stands high and far, facing the house.
                    let dir = (Vec3::new(camera.eye.x, 0.0, camera.eye.z)
                        - Vec3::new(p.at.x, 0.0, p.at.z))
                    .normalize_or_zero();
                    let yaw = dir.x.atan2(dir.z);
                    model = Mat4::from_scale_rotation_translation(
                        Vec3::splat(1.6),
                        Quat::from_rotation_y(yaw),
                        Vec3::new(p.at.x, 18.0, p.at.z),
                    );
                    self.gpu.mesh(mesh, &model, camera, lighting, true);
                    return;
                }
                match state.as_str() {
                    "rocking" => {
                        model *= Mat4::from_rotation_x((t * 2.2).sin() * 0.14);
                    }
                    "swaying" => model *= Mat4::from_rotation_z((t * 1.3).sin() * 0.08),
                    "rattling" => {
                        model *= Mat4::from_translation(Vec3::new(
                            (t * 40.0).sin() * 0.03 * (-since).exp(),
                            0.0,
                            0.0,
                        ));
                    }
                    "banging" => {
                        model *= Mat4::from_rotation_y(
                            ((t * 14.0).sin() * 0.12).max(0.0) * (-since * 0.3).exp(),
                        );
                    }
                    "ghost" => {
                        // The sheet comes off its line and glides across.
                        let k = (since / 2.5).min(1.0);
                        model = Mat4::from_translation(Vec3::new(
                            -2.5 * k,
                            (t * 3.0).sin() * 0.08,
                            0.0,
                        )) * model;
                    }
                    _ => {}
                }
                self.gpu.mesh(mesh, &model, camera, lighting, false);
            }
            Kind::Card => {
                let Some((texture, aspect)) = self.cards.get(&p.asset) else {
                    return;
                };
                let height = p.size[2];
                let width = height * aspect;
                let mut base = p.at;
                let mut lift = 0.0;
                match state.as_str() {
                    "lunge" => {
                        let to = (camera.eye - base).with_y(0.0).normalize_or_zero();
                        base += to * (since * 3.0).min(1.0) * 0.8;
                    }
                    "glide" => {
                        let k = (since / 2.0).min(1.0);
                        let side = Quat::from_rotation_y(p.yaw.to_radians()) * Vec3::X;
                        base += side * (k * 3.0);
                        lift = 0.15 + (t * 3.0).sin() * 0.06;
                    }
                    "swoop" => lift = (t * 4.0).sin() * 0.3 - 0.3 * (since * 2.0).min(1.0),
                    "rise" => lift = -0.8 + (since * 1.2).min(0.8),
                    "hiss" => lift = ((since * 18.0).sin() * 0.03).abs(),
                    _ => lift = (t * 1.5 + p.at.x).sin() * 0.03,
                }
                if surface == Some(Surface::Ceiling) {
                    lift -= height;
                }
                self.gpu
                    .card(texture, base, [width, height], lift, camera, lighting);
            }
            Kind::Effect => self.draw_effect(p, camera, lighting),
        }
    }

    /// An effect placed in the house: fog drifts, a drop falls into its
    /// puddle, eyes blink, a cup floats, footprints appear one by one.
    fn draw_effect(&self, p: &Placed, camera: &Camera, lighting: &Lighting) {
        let t = self.time;
        let mesh = |part: &str| self.meshes.get(&format!("effect:{}#{part}", p.asset));
        let base = p.model();
        match p.asset.as_str() {
            "ground-fog" => {
                if let Some(m) = mesh("") {
                    let drift = Mat4::from_rotation_y((t * 0.05).sin() * 0.4)
                        * Mat4::from_translation(Vec3::new((t * 0.3).sin() * 0.3, 0.0, 0.0));
                    self.gpu.mesh(m, &(base * drift), camera, lighting, false);
                }
            }
            "dripping" => {
                if let Some(m) = mesh("") {
                    self.gpu.mesh(m, &base, camera, lighting, false);
                }
                if let Some(m) = mesh("drop") {
                    let height = self
                        .area
                        .as_ref()
                        .and_then(|id| self.house.area(id))
                        .and_then(|a| a.ceiling)
                        .unwrap_or(3.0);
                    let fall = (t * 1.2).fract();
                    let y = height * (1.0 - fall * fall);
                    let model = Mat4::from_translation(Vec3::new(p.at.x, y, p.at.z));
                    self.gpu.mesh(m, &model, camera, lighting, false);
                }
            }
            "glowing-eyes" => {
                let blink = (t * 0.33).fract() > 0.95;
                if let (Some(m), false) = (mesh(""), blink) {
                    let lifted = base * Mat4::from_translation(Vec3::Y * 0.9);
                    self.gpu.mesh(m, &lifted, camera, lighting, true);
                }
            }
            "floating-object" => {
                if let Some(m) = mesh("") {
                    let bob = Mat4::from_translation(Vec3::Y * (1.1 + (t * 1.7).sin() * 0.12))
                        * Mat4::from_rotation_y(t * 0.6);
                    self.gpu.mesh(m, &(base * bob), camera, lighting, false);
                }
            }
            "footprints" => {
                if let Some(m) = mesh("") {
                    let shown = ((t * 1.5) as usize % 10).max(1);
                    for i in 0..shown.min(8) {
                        let side = if i % 2 == 0 { -0.12 } else { 0.12 };
                        let step = Mat4::from_translation(Vec3::new(side, 0.0, i as f32 * 0.42));
                        self.gpu.mesh(m, &(base * step), camera, lighting, false);
                    }
                }
            }
            _ => {}
        }
    }

    /// Shows library page `page`: the room and that page's assets.
    fn set_library(&mut self, page: usize) {
        let pages = scene::library_pages(self.catalog);
        let page = page % pages.max(1);
        let (house, placed) = scene::library(self.catalog, page);
        self.house = house;
        self.rebuild_all();
        for p in &placed {
            self.ensure_asset(p);
        }
        if let Some(g) = self.areas.get_mut("library") {
            g.props = placed;
        }
        self.library = Some(page);
        self.walker = Walker::new([12.0, 9.5], 0.0);
        self.walker.pitch = -4.0;
        self.publish_library();
    }

    /// The sound test: every library sound in turn, 1.5 s apart, noting
    /// whether the page made it.
    fn step_sound_test(&mut self) {
        let Some((next, at)) = self.sound_test else {
            return;
        };
        if self.time < at {
            return;
        }
        let sounds = &self.catalog.sounds;
        let Some(sound) = sounds.get(next) else {
            self.sound.music(false);
            self.sound.bed(None);
            self.sound_test = None;
            self.publish_library();
            return;
        };
        let made = match sound.kind {
            haunt_assets::SoundKind::Effect => self.sound.play(&sound.id, None),
            haunt_assets::SoundKind::Bed => {
                self.sound.bed(Some(&sound.id));
                crate::sounds::BEDS.contains(&sound.id.as_str())
            }
            haunt_assets::SoundKind::Music => {
                self.sound.music(true);
                crate::sounds::MUSIC.contains(&sound.id.as_str())
            }
        };
        self.played
            .push(serde_json::json!({"id": sound.id, "made": made, "awake": self.sound.awake()}));
        self.sound_test = Some((next + 1, self.time + 1.5));
        self.publish_library();
    }

    fn publish_library(&self) {
        let Some(page) = self.library else { return };
        let assets: Vec<String> = self
            .areas
            .get("library")
            .map(|g| g.props.iter().map(|p| p.asset.clone()).collect())
            .unwrap_or_default();
        let value = serde_json::json!({
            "page": page,
            "pages": scene::library_pages(self.catalog),
            "assets": assets,
            "played": self.played,
            "sounds": self.catalog.sounds.len(),
            "testing": self.sound_test.is_some(),
            "labels": self.library_labels(),
        });
        let _ = Reflect::set(
            &self.window,
            &"haunt_library".into(),
            &JsValue::from_str(&value.to_string()),
        );
    }

    /// The names under the library's assets, on the screen.
    fn library_labels(&self) -> Vec<(f64, f64, String)> {
        let Some(g) = self.areas.get("library").filter(|_| self.library.is_some()) else {
            return Vec::new();
        };
        let (w, h) = (
            f64::from(self.canvas.client_width()),
            f64::from(self.canvas.client_height()),
        );
        g.props
            .iter()
            .filter_map(|p| {
                let at = self.last_view * glam::Vec4::new(p.at.x, -0.25, p.at.z, 1.0);
                if at.w <= 0.0 {
                    return None;
                }
                let (x, y) = (at.x / at.w, at.y / at.w);
                Some((
                    (f64::from(x) * 0.5 + 0.5) * w,
                    (0.5 - f64::from(y) * 0.5) * h,
                    p.asset.clone(),
                ))
            })
            .collect()
    }

    /// `window.haunt_state` for checks and recordings.
    fn publish(&mut self, force: bool) {
        self.published += 1;
        if !force && self.published % 10 != 0 {
            return;
        }
        self.publish_library();
        let a = &self.house.audio;
        let mode = match self.mode {
            Mode::Title => "title",
            Mode::Playing => "playing",
            Mode::Menu => "menu",
            Mode::Ended => "ended",
        };
        let light = self
            .area
            .as_ref()
            .and_then(|id| self.house.area(id))
            .map(|a| a.light.preset.clone());
        let state = serde_json::json!({
            "mode": mode,
            "area": self.area,
            "pos": [self.walker.pos[0], self.walker.pos[1]],
            "yaw": self.walker.yaw,
            "pitch": self.walker.pitch,
            "time": self.time,
            "cue": self.cue_played,
            "audio": {"master": a.master, "music": a.music, "ambience": a.ambience, "sfx": a.sfx, "voice": a.voice, "awake": self.sound.awake()},
            "look": {"vignette": self.house.look.vignette, "lantern": self.house.look.lantern, "grain": self.house.look.grain, "gentle": self.gentle},
            "light": light,
            "areas": self.house.areas.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
            "areas_seen": self.areas_seen,
            "fired": self.fired,
            "manifest": self.live.as_ref().map(|l| l.manifest.clone()),
            "seq": self.live.as_ref().map(|l| l.seq),
            "applied": self.applied,
            "props": self.house.areas.iter().map(|a| a.props.len()).sum::<usize>(),
            "bot": self.bot,
        });
        let _ = Reflect::set(
            &self.window,
            &"haunt_state".into(),
            &JsValue::from_str(&state.to_string()),
        );
    }

    /// Applies a new version of the house between two frames: volumes and
    /// looks at once, changed areas rebuilt, the walk carrying on.
    fn apply_house(&mut self, house: House, changed: &[String]) {
        let audio_changed = house.audio != self.house.audio;
        let look_changed = house.look != self.house.look;
        let old_ids: Vec<String> = self.house.areas.iter().map(|a| a.id.clone()).collect();
        self.house = house;
        for id in changed {
            self.rebuild_area(id);
        }
        let ids: Vec<String> = self.house.areas.iter().map(|a| a.id.clone()).collect();
        if ids != old_ids {
            self.rebuild_all();
        } else {
            self.walls = Walls::of(&self.house, self.catalog);
        }
        if audio_changed {
            self.sound.set_levels(self.levels(), 0.15);
            self.say("The sound changed");
        }
        if look_changed {
            self.say("The look changed");
        }
        // Areas whose light changed: forget a trigger's override.
        for id in changed {
            self.overrides.remove(id);
        }
        if let Some(a) = self
            .area
            .clone()
            .and_then(|id| self.house.area(&id).cloned())
        {
            self.sound.bed(a.sound.bed.as_deref());
            let intensity = f32::from(a.intent.intensity[0] + a.intent.intensity[1]) / 2.0;
            self.sound.area(a.sound.music_gain, intensity);
        }
    }

    fn pump_live(&mut self) {
        let Some(mut live) = self.live.take() else {
            return;
        };
        live.pump(self.walker.pos);
        let streamed = live.take_streamed();
        if !streamed.is_empty() {
            let mut changed = Vec::new();
            for (area, assets) in streamed {
                self.install_assets(assets);
                changed.push(area.id.clone());
            }
            let house = live.house();
            self.apply_house(house, &changed);
        }
        while let Some(ready) = live.take_ready() {
            match ready {
                Ok(mut version) => {
                    let assets = std::mem::take(&mut version.assets);
                    self.install_assets(assets);
                    live.adopt(&version);
                    let mut changed: Vec<String> =
                        version.areas.iter().map(|a| a.id.clone()).collect();
                    changed.extend(version.removed.iter().cloned());
                    // A changed asset redraws the areas that use it.
                    let house = live.house();
                    self.apply_house(house, &changed);
                    let applied_ms = live_web::now() - version.since;
                    let (bytes, files) = live_web::transferred(&version.urls, version.since);
                    let note = serde_json::json!({
                        "applied_ms": (applied_ms * 10.0).round() / 10.0,
                        "fetched_bytes": bytes,
                        "fetched_files": files,
                        "areas": changed,
                        "audio": serde_json::to_value(&self.house.audio).unwrap_or_default(),
                        "same_walk": self.mode != Mode::Title,
                        "pos": [self.walker.pos[0], self.walker.pos[1]],
                    });
                    self.applied.push(serde_json::json!({"manifest": version.manifest, "seq": version.seq, "applied_ms": note["applied_ms"], "areas": note["areas"]}));
                    live.ack("swap", note);
                    self.publish(true);
                }
                Err(error) => {
                    web_sys::console::warn_1(&format!("a new version didn't load: {error}").into());
                }
            }
        }
        self.live = Some(live);
    }

    /// Sends what the mic heard to Game Lab's chat.
    fn pump_mic(&mut self) {
        let Some(mic) = &self.mic else { return };
        match mic.take() {
            crate::mic::Heard::Words(text) => {
                let sent = crate::mic::say(&text);
                self.say(&if sent {
                    format!("Sent: \u{201c}{text}\u{201d}")
                } else {
                    "Game Lab's chat isn't open. Open it to change the house.".to_owned()
                });
            }
            crate::mic::Heard::Missed => self.say("The mic didn't catch that. Try again."),
            _ => {}
        }
    }

    /// A newer version from the followed address, applied in place.
    fn pump_follow(&mut self) {
        let Some(found) = self.follow.as_mut().and_then(crate::follow::Follow::pump) else {
            return;
        };
        let changed: Vec<String> = found
            .house
            .areas
            .iter()
            .filter(|a| self.house.area(&a.id) != Some(a))
            .map(|a| a.id.clone())
            .collect();
        self.apply_house(found.house, &changed);
        let applied_ms = live_web::now() - found.asked_at;
        self.applied.push(serde_json::json!({
            "version": found.version,
            "applied_ms": (applied_ms * 10.0).round() / 10.0,
            "areas": changed,
        }));
        self.say("The house changed");
        self.publish(true);
    }

    fn frame(&mut self, now_ms: f64) {
        let dt = if self.last == 0.0 {
            1.0 / 60.0
        } else {
            ((now_ms - self.last) / 1000.0).clamp(0.0, 0.1) as f32
        };
        self.last = now_ms;
        self.pump_live();
        self.pump_follow();
        self.pump_mic();
        self.step(dt);
        self.render();
        self.publish(false);
    }
}

fn key_name(event: &KeyboardEvent) -> String {
    event.key()
}

/// Starts the page: the built-in house, or the live tenant's.
pub fn start() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(canvas) = document
        .get_element_by_id("haunt-canvas")
        .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
    else {
        web_sys::console::error_1(&"The page has no haunt-canvas.".into());
        return;
    };
    let tenant = canvas
        .get_attribute("data-live-tenant")
        .filter(|t| !t.is_empty());
    if let Some(url) = canvas
        .get_attribute("data-house-url")
        .filter(|u| !u.is_empty())
    {
        wasm_bindgen_futures::spawn_local(async move {
            let found = crate::follow::first(&url).await;
            launch_following(window, canvas, url, found);
        });
        return;
    }
    match tenant {
        Some(tenant) => wasm_bindgen_futures::spawn_local(async move {
            match Live::boot(tenant).await {
                Ok((live, version)) => launch(window, canvas, Some((live, version))),
                Err(error) => {
                    web_sys::console::error_1(&format!("The house couldn't load: {error}").into());
                    launch(window, canvas, None);
                }
            }
        }),
        None => launch(window, canvas, None),
    }
}

/// Starts with the house the address gave (or version 1 when it gave
/// none) and keeps following it.
fn launch_following(
    window: web_sys::Window,
    canvas: HtmlCanvasElement,
    url: String,
    found: Option<crate::follow::Fetched>,
) {
    let follow = found
        .as_ref()
        .map(|f| crate::follow::Follow::new(url, f.version));
    let house = found.map(|f| f.house);
    launch_with(window, canvas, None, house, follow);
}

fn launch(window: web_sys::Window, canvas: HtmlCanvasElement, boot: Option<(Live, Version)>) {
    launch_with(window, canvas, boot, None, None);
}

fn launch_with(
    window: web_sys::Window,
    canvas: HtmlCanvasElement,
    boot: Option<(Live, Version)>,
    followed: Option<House>,
    follow: Option<crate::follow::Follow>,
) {
    let Some(gl) = canvas
        .get_context("webgl2")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into::<Gl>().ok())
    else {
        web_sys::console::error_1(&"This browser can't draw the house (no WebGL2).".into());
        return;
    };
    let (house, live, assets) = match boot {
        Some((live, mut version)) => {
            let assets = std::mem::take(&mut version.assets);
            (live.house(), Some(live), Some(assets))
        }
        None => (followed.unwrap_or_else(haunt_rules::v1), None, None),
    };
    let mut app = match App::new(window.clone(), canvas.clone(), gl, house) {
        Ok(app) => app,
        Err(error) => {
            web_sys::console::error_1(&format!("The house couldn't start: {error}").into());
            return;
        }
    };
    if let Some(assets) = assets {
        app.install_assets(assets);
    }
    app.live = live;
    app.mic = follow.as_ref().and_then(|_| crate::mic::Mic::new());
    app.follow = follow;
    let hash = window.location().hash().unwrap_or_default();
    // `#at=X,Z[,YAW]` stands the player there, and `#begin` starts at
    // once: for captures and checks.
    let param = |name: &str| {
        hash.split('&').find_map(|p| {
            p.trim_start_matches('#')
                .strip_prefix(name)
                .map(str::to_owned)
        })
    };
    // `#library` (or `#library=N`) shows the prepared library a page at a
    // time, and `&sounds` plays every sound in turn.
    if let Some(page) = param("library=")
        .and_then(|v| v.parse::<usize>().ok())
        .or(hash.contains("library").then_some(0))
    {
        app.set_library(page);
        if hash.contains("sounds") {
            app.sound_test = Some((0, 1.0));
        }
    }
    if hash.contains("walk") {
        app.route = scene::route(&app.house);
        app.bot = Some(0);
    }
    let at: Option<Vec<f32>> =
        param("at=").map(|v| v.split(',').filter_map(|n| n.parse().ok()).collect());
    let app = Rc::new(RefCell::new(app));
    if app.borrow().bot.is_some() || hash.contains("begin") || at.is_some() {
        app.borrow_mut().begin();
    }
    if let Some(at) = at
        && at.len() >= 2
    {
        let mut a = app.borrow_mut();
        a.walker.pos = [at[0], at[1]];
        if let Some(yaw) = at.get(2) {
            a.walker.yaw = *yaw;
        }
        a.arrived_at = -10.0;
    }
    if let Some(live) = &app.borrow().live {
        let (bytes, files) = live_web::transferred(&[], 0.0);
        live.ack(
            "first_play",
            serde_json::json!({"first_play_ms": live_web::now(), "bytes": bytes, "files": files}),
        );
    }
    let target: &web_sys::EventTarget = window.as_ref();
    {
        let app = app.clone();
        listen(target, "keydown", move |e: KeyboardEvent| {
            let name = key_name(&e);
            if matches!(
                name.as_str(),
                "Tab" | " " | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
            ) {
                e.prevent_default();
            }
            let mut a = app.borrow_mut();
            a.sound.wake();
            a.key(&name, true);
        });
    }
    {
        let app = app.clone();
        listen(target, "keyup", move |e: KeyboardEvent| {
            app.borrow_mut().key(&key_name(&e), false);
        });
    }
    {
        let app = app.clone();
        listen(target, "blur", move |_: web_sys::Event| {
            app.borrow_mut().keys.clear();
        });
    }
    let canvas_target: &web_sys::EventTarget = canvas.as_ref();
    {
        let app = app.clone();
        listen(canvas_target, "pointerdown", move |e: PointerEvent| {
            let mut a = app.borrow_mut();
            let (x, y) = (f64::from(e.offset_x()), f64::from(e.offset_y()));
            if let Some(b) = a.ui.buttons.iter().find(|b| b.hit(x, y)).cloned() {
                a.act(b.action);
                return;
            }
            match a.mode {
                Mode::Title | Mode::Ended => a.begin(),
                Mode::Menu => a.begin(),
                Mode::Playing if e.pointer_type() == "touch" => {
                    let w = f64::from(a.canvas.client_width());
                    if x < w / 2.0 && a.touch_move.is_none() {
                        a.touch_move = Some((e.pointer_id(), [x, y], [x, y]));
                    } else {
                        a.touch_look = Some((e.pointer_id(), [x, y]));
                    }
                }
                Mode::Playing => {
                    if !a.pointer_locked {
                        a.canvas.request_pointer_lock();
                    }
                }
            }
        });
    }
    {
        let app = app.clone();
        listen(canvas_target, "pointermove", move |e: PointerEvent| {
            let mut a = app.borrow_mut();
            let (x, y) = (f64::from(e.offset_x()), f64::from(e.offset_y()));
            if a.pointer_locked {
                a.mouse[0] += e.movement_x() as f32;
                a.mouse[1] += e.movement_y() as f32;
                return;
            }
            if let Some((id, centre, _)) = a.touch_move
                && id == e.pointer_id()
            {
                a.touch_move = Some((id, centre, [x, y]));
            }
            if let Some((id, last)) = a.touch_look
                && id == e.pointer_id()
            {
                a.mouse[0] += ((x - last[0]) * 2.0) as f32;
                a.mouse[1] += ((y - last[1]) * 2.0) as f32;
                a.touch_look = Some((id, [x, y]));
            }
        });
    }
    {
        let app = app.clone();
        let end = move |e: PointerEvent| {
            let mut a = app.borrow_mut();
            if a.touch_move.is_some_and(|(id, _, _)| id == e.pointer_id()) {
                a.touch_move = None;
            }
            if a.touch_look.is_some_and(|(id, _)| id == e.pointer_id()) {
                a.touch_look = None;
            }
        };
        let end2 = end.clone();
        listen(canvas_target, "pointerup", end);
        listen(canvas_target, "pointercancel", end2);
    }
    if let Some(document) = window.document() {
        let app = app.clone();
        let doc_target: &web_sys::EventTarget = document.as_ref();
        let doc = document.clone();
        listen(doc_target, "pointerlockchange", move |_: web_sys::Event| {
            let mut a = app.borrow_mut();
            let locked = doc.pointer_lock_element().is_some();
            if a.pointer_locked && !locked && a.mode == Mode::Playing {
                a.mode = Mode::Menu;
            }
            a.pointer_locked = locked;
        });
    }
    // The loop.
    let tick: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let again = tick.clone();
    let w = window.clone();
    *again.borrow_mut() = Some(Closure::new(move |t: f64| {
        app.borrow_mut().frame(t);
        if let Some(cb) = tick.borrow().as_ref() {
            let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }));
    if let Some(cb) = again.borrow().as_ref() {
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
    }
    let _ = now(&window);
}
