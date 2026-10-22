//! The Pylon Bay in the Grid's depth-tested world pass (#12423).
//!
//! The Bay's floor and sign are a static placement. Each pylon is a
//! machine model chosen by its shape and load, moved to its spot each
//! frame; a working machine sends a spark up its beam, a job's receipt
//! sends a ring across the floor, a group's pipe carries a packet while
//! the group answers, and brackets mark the machine under the pointer or
//! with its card open.
//!
//! A model group stands as one machine (#12403): light conduits join its
//! head and parts into a loop, and its name stands on a sign over its lane,
//! with a preview tag when its launch calls itself one. While it reads a
//! prompt a long sweep runs through its parts in turn; while it answers a
//! pulse beats around the loop at its decode rate; idle, its conduits are
//! steady and quiet; starting, recovering, or down, they are broken. Each
//! part shows its device's emblem. Close by, the group's details and each
//! part's line are drawn over the world ([`draw`]).
//!
//! The pylons layer (`crates/pylons-web`) owns the live list and the
//! places; it hands them over as `globalThis.pylonsBay`, a `Float64Array`
//! in [`pylons_view::bay::encode`]'s layout, and the groups as
//! `globalThis.pylonsGroups`, [`machine::Scene`] as JSON. Every model is
//! built here at start from [`pylons_view::bay::art`], so the Bay adds no
//! download.

use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use pylons_view::bay::{
    self, Load, Shape,
    art::{self, Art, Flow},
    machine::{self, Device, GroupScene},
};
use verse::{imported::flat, palette};
use verse_engine::{
    assets::{Model, Pack, Placement, Surface, Topology, Vertex},
    inventory::{Asset, Binding, fingerprint},
    presentation::Instance,
};

/// The global the pylons layer writes the Bay to.
const GLOBAL: &str = "pylonsBay";
/// The floor, the section names, the lanes, and the sign.
const FLOOR: &str = "bay/floor";
const SPARK: &str = "bay/spark";
const FOCUS: &str = "bay/focus";
/// The global the pylons layer writes the model groups to.
const GROUPS: &str = "pylonsGroups";
const PULSE_LEAD: &str = "bay/pulse-lead";
const PULSE_TRAIL: &str = "bay/pulse-trail";
const GLOW: &str = "bay/glow";
const PREVIEW_TAG: &str = "bay/preview-tag";
const SIGN_POST: &str = "bay/sign-post";
/// How far in front of a part its emblem stands, toward the plaza, m, and
/// how high.
const EMBLEM_FRONT: f32 = 0.8;
const EMBLEM_Y: f32 = 0.45;
/// A pulse's rings, brightest first: it fades as it grows.
const RINGS: [&str; 3] = ["bay/ring-3", "bay/ring-2", "bay/ring-1"];
/// How long a pulse lasts, ms.
const PULSE_MS: f64 = 1400.0;
/// How long a spark takes to climb its beam, ms.
const SPARK_MS: f64 = 1600.0;
/// The dark faces' tint: the terminal's near-black.
const DARK: [f32; 3] = [0.002; 3];

fn machine_key(shape: Shape, load: Load) -> String {
    format!("bay/m{}-{}", shape.index(), load.index())
}

fn pipe_key(parts: u32, lit: bool) -> String {
    format!("bay/pipe{parts}-{}", u8::from(lit))
}

fn conduit_key(flow: Flow) -> String {
    format!("bay/conduit-{}", flow.index())
}

fn emblem_key(device: Device) -> String {
    format!("bay/emblem{}", device.index())
}

/// A sign letter's model.
fn letter_key(c: u8) -> String {
    format!("bay/letter-{}", c as char)
}

pub(crate) fn tint(tone: u8) -> [f32; 3] {
    let step = Intensity::ALL[usize::from(tone.min(3))];
    palette::neutral(palette::amber(step))
}

fn point(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
}

/// A flat surface that shares each corner among the lines and faces that
/// meet there. The Grid's start digests every vertex of every model
/// (#12423), so fewer vertices start the page sooner.
struct Shared {
    surface: Surface,
}

impl Shared {
    fn new(white: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: flat::surface(white, tint, topology),
        }
    }

    fn at(&mut self, p: [f64; 3]) -> u32 {
        let position = point(p).to_array();
        let vertices = &mut self.surface.vertices;
        // A model has a few hundred corners at most: a scan is enough.
        if let Some(found) = vertices.iter().position(|v| v.position == position) {
            return found as u32;
        }
        vertices.push(Vertex {
            position,
            normal: [0., 1., 0.],
            uv: [0.5, 0.5],
            joints: [0; 4],
            weights: [1., 0., 0., 0.],
        });
        (vertices.len() - 1) as u32
    }

    fn line(&mut self, a: [f64; 3], b: [f64; 3]) {
        let (a, b) = (self.at(a), self.at(b));
        self.surface.indices.extend([a, b]);
    }

    fn quad(&mut self, corners: [[f64; 3]; 4]) {
        let [a, b, c, d] = corners.map(|p| self.at(p));
        self.surface.indices.extend([a, b, c, a, c, d]);
    }

    fn done(self, out: &mut Vec<Surface>) {
        if !self.surface.indices.is_empty() {
            out.push(self.surface);
        }
    }
}

/// A model of `art`: the dark faces, a lines surface for each tone, and a
/// lit surface for each tone of the letters.
fn model(source: &str, art: &Art, white: usize, height: f32) -> Model {
    let mut surfaces = Vec::new();
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for corners in &art.faces {
        faces.quad(*corners);
    }
    faces.done(&mut surfaces);
    for tone in 0..4 {
        let mut lines = Shared::new(white, tint(tone), Topology::Lines);
        for (a, b, _) in art.lines.iter().filter(|l| l.2 == tone) {
            lines.line(*a, *b);
        }
        lines.done(&mut surfaces);
        let mut lit = Shared::new(white, tint(tone), Topology::Triangles);
        for (corners, _) in art.lit.iter().filter(|q| q.1 == tone) {
            lit.quad(*corners);
        }
        lit.done(&mut surfaces);
    }
    flat::model(source, surfaces, height)
}

/// Adds the Bay's models to the Grid's `pack`, whose white texel is slot
/// `white` and listed as `texture`, and stands its floor.
pub fn add(pack: &mut Pack, white: usize, texture: &str) -> Result<(), String> {
    let mut models: Vec<(String, Model)> = Vec::new();
    for shape in Shape::ALL {
        for load in Load::ALL {
            let key = machine_key(shape, load);
            let height = (shape.top() + load.beam()) as f32;
            models.push((
                key.clone(),
                model(&key, &bay::art::machine(shape, load), white, height),
            ));
        }
    }
    for parts in 1..=bay::MOST_PARTS {
        for lit in [false, true] {
            let key = pipe_key(parts, lit);
            models.push((
                key.clone(),
                model(&key, &bay::art::pipe(parts, lit), white, 0.1),
            ));
        }
    }
    for (tone, key) in [3, 2, 1].into_iter().zip(RINGS) {
        models.push((key.into(), model(key, &bay::art::ring(tone), white, 0.1)));
    }
    models.push((SPARK.into(), model(SPARK, &bay::art::spark(), white, 0.1)));
    for flow in Flow::ALL {
        let key = conduit_key(flow);
        models.push((key.clone(), model(&key, &art::conduit(flow), white, 1.4)));
    }
    for (key, lead) in [(PULSE_LEAD, true), (PULSE_TRAIL, false)] {
        models.push((key.into(), model(key, &art::pulse(lead), white, 0.3)));
    }
    models.push((GLOW.into(), model(GLOW, &art::glow(), white, 1.1)));
    for device in Device::ALL {
        let key = emblem_key(device);
        models.push((key.clone(), model(&key, &art::emblem(device), white, 0.4)));
    }
    for &c in art::SIGN_CHARS {
        let key = letter_key(c);
        models.push((
            key.clone(),
            model(&key, &art::letter(c, art::BRIGHT), white, 7.0),
        ));
    }
    models.push((
        PREVIEW_TAG.into(),
        model(PREVIEW_TAG, &art::preview_tag(), white, 0.5),
    ));
    models.push((
        SIGN_POST.into(),
        model(SIGN_POST, &art::sign_post(), white, 5.0),
    ));
    models.push((FOCUS.into(), model(FOCUS, &bay::art::focus(), white, 0.1)));
    models.push((FLOOR.into(), model(FLOOR, &bay::art::floor(), white, 2.5)));
    let inventory = pack
        .inventory
        .as_mut()
        .ok_or("The Grid's inventory is missing")?;
    let texture = inventory
        .assets
        .iter()
        .find(|a| a.id.as_str() == texture)
        .ok_or("The Grid's white texture is not listed")?
        .id
        .clone();
    for (key, model) in models {
        let (sha256, bytes) = fingerprint(&model)?;
        inventory.assets.push(Asset {
            id: verse::imported::inventory::id(&format!("verse:model:{key}"))?,
            binding: Binding::Model { key: key.clone() },
            sha256,
            bytes,
            dependencies: vec![texture.clone()],
        });
        pack.models.insert(key, model);
    }
    pack.placements.push(Placement {
        model: FLOOR.into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: 1.,
    });
    Ok(())
}

fn fixed(model: &str, transform: Mat4) -> Instance {
    verse::grid_frame::fixed(model, transform)
}

/// The Bay as the pylons layer last handed it over, if it has.
fn read() -> Option<(Vec<bay::Drawn>, Vec<bay::DrawnPipe>)> {
    let value = js_sys::Reflect::get(&js_sys::global(), &GLOBAL.into()).ok()?;
    let numbers = wasm_bindgen::JsCast::dyn_ref::<js_sys::Float64Array>(&value)?.to_vec();
    bay::decode(&numbers)
}

/// A steady phase in 0..1 for a machine at (`x`, `z`), so neighbors don't
/// pulse in step.
fn phase(x: f64, z: f64) -> f64 {
    ((x * 12.9898 + z * 78.233).sin() * 43_758.545)
        .fract()
        .abs()
}

/// This frame's Bay instances, at page time `now` (ms).
pub fn instances(now: f64, out: &mut Vec<Instance>) {
    let Some((stands, pipes)) = read() else {
        return;
    };
    for stand in &stands {
        let foot = Vec3::new(stand.x as f32, 0.0, stand.z as f32);
        out.push(fixed(
            &machine_key(stand.shape, stand.load),
            Mat4::from_translation(foot),
        ));
        if stand.load.busy() {
            let t = (now / SPARK_MS + phase(stand.x, stand.z)).fract();
            let from = stand.shape.top() + 0.25;
            let y = from + t * (stand.shape.top() + stand.load.beam() - from);
            out.push(fixed(
                SPARK,
                Mat4::from_translation(foot + Vec3::Y * y as f32),
            ));
        }
        let age = now - stand.pulse_at;
        if stand.pulse_at >= 0.0 && (0.0..PULSE_MS).contains(&age) {
            let t = age / PULSE_MS;
            let ring = RINGS[((t * 3.0) as usize).min(2)];
            let scale = (0.9 + t * 1.9) as f32;
            out.push(fixed(
                ring,
                Mat4::from_translation(foot) * Mat4::from_scale(Vec3::new(scale, 1.0, scale)),
            ));
        }
        if stand.focused {
            out.push(fixed(FOCUS, Mat4::from_translation(foot)));
        }
    }
    for pipe in &pipes {
        if pipe.parts == 0 {
            continue;
        }
        let head = Vec3::new(pipe.x as f32, 0.0, pipe.z as f32);
        out.push(fixed(
            &pipe_key(pipe.parts, pipe.lit),
            Mat4::from_translation(head),
        ));
    }
    GROUP_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.refresh();
        for group in &cache.scene.groups {
            group_instances(group, now, out);
        }
    });
}

/// The groups as the pylons layer last handed them over, parsed once per
/// handoff.
#[derive(Default)]
struct GroupCache {
    json: String,
    scene: machine::Scene,
}

impl GroupCache {
    fn refresh(&mut self) {
        let Some(json) = js_sys::Reflect::get(&js_sys::global(), &GROUPS.into())
            .ok()
            .and_then(|v| v.as_string())
        else {
            return;
        };
        if json != self.json {
            self.scene = serde_json::from_str(&json).unwrap_or_default();
            self.json = json;
        }
    }
}

thread_local! {
    static GROUP_CACHE: std::cell::RefCell<GroupCache> = std::cell::RefCell::default();
}

/// The machines' labels as the layer last handed them over (#12459),
/// parsed once per handoff.
#[derive(Default)]
struct LabelCache {
    json: String,
    labels: Vec<bay::Label>,
}

impl LabelCache {
    fn refresh(&mut self) {
        let Some(json) = js_sys::Reflect::get(&js_sys::global(), &bay::LABELS_GLOBAL.into())
            .ok()
            .and_then(|v| v.as_string())
        else {
            return;
        };
        if json != self.json {
            self.labels = serde_json::from_str(&json).unwrap_or_default();
            self.json = json;
        }
    }
}

thread_local! {
    static LABEL_CACHE: std::cell::RefCell<LabelCache> = std::cell::RefCell::default();
}

/// Each labeled machine's name, or its name, details, and slots up close,
/// over its top, laid out in CSS pixels on a `size` canvas (#12459). The
/// layer chose how much each says and kept them from covering each other;
/// here they follow this frame's camera.
pub fn draw_labels(
    ui: &mut verse::ui::UiBatch,
    atlas: &verse::ui::Atlas,
    size: [f32; 2],
    view_proj: Mat4,
) {
    let ink = |alpha: f32| crate::theme::linear(coder_ui::coder_noir::CONTENT, alpha);
    let quiet = |alpha: f32| crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, alpha);
    LABEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.refresh();
        for label in &cache.labels {
            let Some([x, y]) = verse::hud::project(view_proj, size, point(label.at)) else {
                continue;
            };
            let pad = if label.full { [10.0, 6.0] } else { [6.0, 3.0] };
            let box_size = (atlas.line * 0.6).round().max(5.0);
            let slots_h = if label.total > 0 { box_size + 4.0 } else { 0.0 };
            let slots_w = label.total as f32 * (box_size + 3.0);
            let width = label
                .lines
                .iter()
                .map(|line| atlas.measure(line))
                .fold(atlas.measure(&label.title), f32::max)
                .max(slots_w)
                + pad[0] * 2.0;
            let height = atlas.line * (1 + label.lines.len()) as f32 + slots_h + pad[1] * 2.0;
            let left = (x - width / 2.0).clamp(4.0, (size[0] - width - 4.0).max(4.0));
            let top = y - height - 4.0;
            if top + height < 0.0 || top > size[1] {
                continue;
            }
            ui.rect(
                atlas,
                left,
                top,
                width,
                height,
                verse::ui::field(if label.full { 0.82 } else { 0.55 }),
            );
            if label.full {
                let edge = if label.focused { ink(0.6) } else { quiet(0.35) };
                ui.frame(atlas, left, top, width, height, 1.0, edge);
            }
            ui.text(
                atlas,
                left + pad[0],
                top + pad[1],
                &label.title,
                ink(if label.full || label.focused {
                    1.0
                } else {
                    0.85
                }),
            );
            for (i, line) in label.lines.iter().enumerate() {
                ui.text(
                    atlas,
                    left + pad[0],
                    top + pad[1] + atlas.line * (i + 1) as f32,
                    line,
                    quiet(1.0),
                );
            }
            let row = top + pad[1] + atlas.line * (1 + label.lines.len()) as f32 + 3.0;
            for slot in 0..label.total {
                let at = left + pad[0] + slot as f32 * (box_size + 3.0);
                if slot < label.busy {
                    ui.rect(atlas, at, row, box_size, box_size, ink(1.0));
                } else {
                    ui.frame(atlas, at, row, box_size, box_size, 1.0, ink(0.7));
                }
            }
        }
    });
}

/// Where a group member stands: the head (0) at the group's foot, part `i`
/// at its stage's foot, or where the lane puts it when its stage is gone.
fn member_foot(group: &GroupScene, member: u32) -> Vec3 {
    let head = point(group.at);
    if member == 0 {
        return head;
    }
    group.stages.iter().find(|s| s.part == member).map_or_else(
        || {
            let along = bay::member_at(Some(member), group.parts) - bay::HEAD_AT;
            head + Vec3::Z * along as f32
        },
        |s| point(s.at),
    )
}

/// One group's loop, pulses, glow, emblems, and sign at page time `now`.
fn group_instances(group: &GroupScene, now: f64, out: &mut Vec<Instance>) {
    let head = point(group.at);
    let phase = machine::live_phase(group, now);
    if group.gone {
        // Where its machines stood, dark, with their beams broken.
        out.push(fixed(
            &machine_key(Shape::Head, Load::Draining),
            Mat4::from_translation(head),
        ));
        for part in 1..=group.parts {
            out.push(fixed(
                &machine_key(Shape::Part, Load::Draining),
                Mat4::from_translation(member_foot(group, part)),
            ));
        }
        out.push(fixed(
            &pipe_key(group.parts.clamp(1, bay::MOST_PARTS), false),
            Mat4::from_translation(head),
        ));
    }
    let parts = group.parts.clamp(1, bay::MOST_PARTS);
    let flow = conduit_key(Flow::of(phase));
    for hop in 0..parts {
        let (from, long) = machine::hop(parts, hop);
        out.push(fixed(
            &flow,
            Mat4::from_translation(head + Vec3::Z * from as f32)
                * Mat4::from_scale(Vec3::new(1.0, 1.0, long as f32)),
        ));
    }
    let motion = machine::activity(phase, group.parts, now, group.busy_since, group.profile);
    for pulse in &motion.pulses {
        out.push(fixed(
            if pulse.lead { PULSE_LEAD } else { PULSE_TRAIL },
            Mat4::from_translation(head + point(pulse.at)),
        ));
    }
    if let Some(member) = motion.working {
        out.push(fixed(
            GLOW,
            Mat4::from_translation(member_foot(group, member)),
        ));
    }
    for stage in &group.stages {
        let foot = point(stage.at);
        out.push(fixed(
            &emblem_key(stage.device()),
            Mat4::from_translation(foot + Vec3::new(-EMBLEM_FRONT, EMBLEM_Y, 0.0)),
        ));
    }
    // The sign over the lane's middle, behind the machines.
    let (name, tag) = machine::sign(group);
    let middle = head
        + Vec3::new(
            art::SIGN_BACK as f32,
            0.0,
            machine::middle(group.parts) as f32,
        );
    let cell = art::SIGN_NAME_CELL as f32;
    let letters = art::spell(&name, art::SIGN_NAME_CELL);
    for (c, along) in &letters {
        let at = middle + Vec3::new(0.0, art::SIGN_NAME_Y as f32, *along as f32);
        out.push(fixed(
            &letter_key(*c),
            Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(cell)),
        ));
    }
    if tag.is_some() {
        out.push(fixed(
            PREVIEW_TAG,
            Mat4::from_translation(middle + Vec3::Y * art::SIGN_TAG_Y as f32),
        ));
    }
    let half = letters
        .last()
        .map_or(1.0, |(_, along)| *along as f32 + 5.0 * cell)
        + 0.3;
    for side in [-half, half] {
        out.push(fixed(
            SIGN_POST,
            Mat4::from_translation(middle + Vec3::Z * side),
        ));
    }
}

/// The site's clock, s, from the page's and the layer's offset.
fn site_now(offset: f64) -> u64 {
    (js_sys::Date::now() / 1000.0 + offset).max(0.0) as u64
}

/// Each nearby group's details over its sign, and each nearby part's line
/// under it, laid out in CSS pixels on a `size` canvas.
/// `player` is where the player stands.
pub fn draw(
    ui: &mut verse::ui::UiBatch,
    atlas: &verse::ui::Atlas,
    size: [f32; 2],
    view_proj: Mat4,
    player: Vec3,
    now: f64,
) {
    GROUP_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.refresh();
        let today = site_now(cache.scene.offset);
        for group in &cache.scene.groups {
            draw_group(ui, atlas, size, view_proj, player, now, today, group);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_group(
    ui: &mut verse::ui::UiBatch,
    atlas: &verse::ui::Atlas,
    size: [f32; 2],
    view_proj: Mat4,
    player: Vec3,
    now: f64,
    today: u64,
    group: &GroupScene,
) {
    let head = point(group.at);
    let middle = head + Vec3::Z * machine::middle(group.parts) as f32;
    let flat = |p: Vec3| Vec3::new(p.x, 0.0, p.z).distance(Vec3::new(player.x, 0.0, player.z));
    if !machine::details_near(group, player.to_array().map(f64::from)) {
        return;
    }
    let ink = |alpha: f32| crate::theme::linear(coder_ui::coder_noir::CONTENT, alpha);
    let quiet = |alpha: f32| crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, alpha);
    // The details, over the sign.
    let mut shown = group.clone();
    shown.phase = machine::live_phase(group, now);
    let lines = machine::panel(&shown, today);
    let anchor = middle
        + Vec3::new(
            art::SIGN_BACK as f32,
            (art::SIGN_NAME_Y + 7.0 * art::SIGN_NAME_CELL + 0.35) as f32,
            0.0,
        );
    if let Some([x, y]) = verse::hud::project(view_proj, size, anchor) {
        let pad = 8.0;
        let width = lines
            .iter()
            .map(|(_, line)| atlas.measure(line))
            .fold(0.0_f32, f32::max)
            + pad * 2.0;
        let height = atlas.line * lines.len() as f32 + pad * 2.0;
        let left = (x - width / 2.0).clamp(4.0, (size[0] - width - 4.0).max(4.0));
        let top = y - height;
        ui.rect(atlas, left, top, width, height, verse::ui::field(0.82));
        let edge = if shown.phase.up() {
            ink(0.55)
        } else {
            quiet(0.4)
        };
        ui.frame(atlas, left, top, width, height, 1.0, edge);
        for (i, (kind, line)) in lines.iter().enumerate() {
            let color = match kind {
                machine::Ink::Title => ink(1.0),
                machine::Ink::Note => ink(0.85),
                machine::Ink::State if shown.phase.up() => ink(0.95),
                machine::Ink::State => quiet(0.9),
                machine::Ink::Plain => ink(0.72),
            };
            ui.text(
                atlas,
                left + pad,
                top + pad + atlas.line * i as f32,
                line,
                color,
            );
        }
    }
    // Each part's label under it, toward the plaza: its layers and its
    // device. Nearest first; one that would cover another waits.
    let mut chips: Vec<(f32, Vec3, &machine::StageScene)> = group
        .stages
        .iter()
        .map(|stage| (flat(point(stage.at)), point(stage.at), stage))
        .filter(|(d, _, _)| *d <= machine::STAGE_REACH as f32)
        .collect();
    chips.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut taken: Vec<[f32; 4]> = Vec::new();
    for (_, foot, stage) in chips {
        let at = foot + Vec3::new(-EMBLEM_FRONT, 0.05, 0.0);
        let Some([x, y]) = verse::hud::project(view_proj, size, at) else {
            continue;
        };
        let lines = machine::stage_chip(stage);
        let width = lines
            .iter()
            .map(|l| atlas.measure(l))
            .fold(0.0_f32, f32::max);
        let height = atlas.line * lines.len() as f32;
        let rect = [
            x - width / 2.0 - 3.0,
            y + 4.0,
            x + width / 2.0 + 3.0,
            y + 4.0 + height,
        ];
        if taken
            .iter()
            .any(|r| rect[0] < r[2] && r[0] < rect[2] && rect[1] < r[3] && r[1] < rect[3])
        {
            continue;
        }
        taken.push(rect);
        for (i, line) in lines.iter().enumerate() {
            let w = atlas.measure(line);
            let color = if i == 0 { ink(0.85) } else { quiet(1.0) };
            ui.text(
                atlas,
                x - w / 2.0,
                y + 4.0 + atlas.line * i as f32,
                line,
                color,
            );
        }
    }
}
