//! The scene on Verse's engine renderer: the tree near its root, the shared
//! output, the signer, the light clients, the blocks, the unroll, and each
//! event's animation as 3D instances; the members, the levels nearest them,
//! and the words as the overlay.
//!
//! The renderer draws at most 1,024 instances a frame, so a tree with
//! thousands of members draws its members and its lowest levels as points
//! projected into the overlay, and the levels above them as spheres and
//! links.

use glam::{Mat4, Quat, Vec3};
use verse::grid_engine::{Content, Kind};
use verse::imported::{flat, lighting::Lighting};
use verse::render::View;
use verse::ui::{Atlas, UiBatch};
use verse_engine::assets::{Pack, Surface, Topology, Vertex};
use verse_engine::presentation::Instance;

use crate::layout::Shape;
use crate::stage::{Fx, Stage};
use crate::words;

pub const GOLD: [f32; 3] = [1.0, 206.0 / 255.0, 84.0 / 255.0];
pub const WHITE: [f32; 3] = [236.0 / 255.0, 240.0 / 255.0, 1.0];
pub const RED: [f32; 3] = [1.0, 77.0 / 255.0, 66.0 / 255.0];
pub const CANVAS: [f32; 3] = [10.0 / 255.0; 3];
const DIM: [f32; 4] = [0.55, 0.57, 0.62, 1.0];

/// Instances kept for the frame's fixed parts; animations use the rest.
const INSTANCES: usize = 1000;
/// Overlay vertices a frame may use (the renderer's buffer holds 4 MiB of
/// 32-byte vertices); members past it are left out of that frame.
const UI_VERTICES: usize = 120_000;
/// Trees with at most this many slots draw every member as a sphere.
const SPHERE_SLOTS: u64 = 64;
/// Tree levels drawn as spheres above the members.
const SPHERE_LEVELS: u32 = 7;

/// The pinned Grid pack's two textures, built into the module.
const WHITE_PNG: &[u8] = include_bytes!("../../../assets/verse/grid/verse-flat-white.png");
const PORTAL_PNG: &[u8] = include_bytes!("../../../assets/verse/grid/everglade-portal.png");

fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

fn instance(model: &str, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.0,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

fn vertex(p: Vec3) -> Vertex {
    Vertex {
        position: p.to_array(),
        normal: [0.0, 1.0, 0.0],
        uv: [0.5, 0.5],
        joints: [0; 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    }
}

fn triangle(surface: &mut Surface, a: Vec3, b: Vec3, c: Vec3) {
    let base = surface.vertices.len() as u32;
    surface.vertices.extend([vertex(a), vertex(b), vertex(c)]);
    surface.indices.extend([base, base + 1, base + 2]);
}

/// A unit sphere: an octahedron split twice and pushed out. `inside` turns
/// its faces inward, for the backdrop seen from within.
fn sphere(surface: &mut Surface, inside: bool) {
    let ring = [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z];
    let mut faces = Vec::new();
    for i in 0..4 {
        faces.push([Vec3::Y, ring[(i + 1) % 4], ring[i]]);
        faces.push([-Vec3::Y, ring[i], ring[(i + 1) % 4]]);
    }
    for _ in 0..2 {
        faces = faces
            .into_iter()
            .flat_map(|[a, b, c]| {
                let (ab, bc, ca) = (
                    (a + b).normalize(),
                    (b + c).normalize(),
                    (c + a).normalize(),
                );
                [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
            })
            .collect();
    }
    for [a, b, c] in faces {
        if inside {
            triangle(surface, a, c, b);
        } else {
            triangle(surface, a, b, c);
        }
    }
}

fn cube(surface: &mut Surface) {
    let p = [
        [-1., -1., -1.],
        [1., -1., -1.],
        [1., 1., -1.],
        [-1., 1., -1.],
        [-1., -1., 1.],
        [1., -1., 1.],
        [1., 1., 1.],
        [-1., 1., 1.],
    ]
    .map(Vec3::from_array);
    for [a, b, c] in [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [3, 7, 6],
        [3, 6, 2],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ] {
        triangle(surface, p[a], p[b], p[c]);
    }
}

/// Invert the imported renderer's grade so its background reaches the
/// product's 10/255 sRGB canvas after tone mapping.
fn canvas_tint() -> [f32; 3] {
    let target = CANVAS[0] / 12.92;
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..24 {
        let middle = (low + high) * 0.5;
        if verse_engine::lighting::Grade::CHAMBER
            .display(Vec3::splat(middle))
            .x
            < target
        {
            low = middle;
        } else {
            high = middle;
        }
    }
    [(low + high) * 0.5; 3]
}

fn pack() -> Result<Pack, String> {
    let mut pack = verse::grid_pack::embedded()?;
    pack.inventory = None;
    pack.placements.clear();
    pack.models
        .retain(|name, _| name == verse::grid_pack::FLOOR);
    for model in pack.models.values_mut() {
        for surface in &mut model.surfaces {
            surface.tint = surface.tint.map(|c| c * 0.15);
        }
    }
    pack.source_revision = "layertree-web".into();
    for (name, tint) in [
        ("value", GOLD),
        ("signature", WHITE),
        ("faint", [0.2; 3]),
        ("danger", RED),
    ] {
        let mut ball = flat::surface(0, tint, Topology::Triangles);
        sphere(&mut ball, false);
        pack.models
            .insert(name.into(), flat::model(name, vec![ball], 2.0));
        let mut wire = flat::surface(0, tint.map(|c| c * 0.42), Topology::Lines);
        flat::line(&mut wire, Vec3::ZERO, Vec3::Y);
        let link = format!("{name}-link");
        pack.models
            .insert(link.clone(), flat::model(&link, vec![wire], 1.0));
    }
    for (name, tint) in [
        ("signer", WHITE),
        ("signer-bad", RED),
        ("signer-dim", [0.2; 3]),
        ("box-gold", GOLD),
    ] {
        let mut block = flat::surface(0, tint, Topology::Triangles);
        cube(&mut block);
        pack.models
            .insert(name.into(), flat::model(name, vec![block], 2.0));
    }
    let mut canvas = flat::surface(0, canvas_tint(), Topology::Triangles);
    sphere(&mut canvas, true);
    pack.models
        .insert("canvas".into(), flat::model("canvas", vec![canvas], 2.0));
    pack.validate()?;
    Ok(pack)
}

/// The pack and the statics drawn under every frame.
pub fn content() -> Result<Content, String> {
    let prepared = verse_engine::loading::Prepared::from_bytes(
        pack()?,
        &[
            ("verse-flat-white.png", WHITE_PNG),
            (verse::grid_pack::PORTAL_VIEW, PORTAL_PNG),
        ],
        Default::default(),
    )?;
    Ok(Content {
        prepared,
        statics: vec![
            instance("canvas", Mat4::from_scale(Vec3::splat(1500.0))),
            instance(verse::grid_pack::FLOOR, Mat4::IDENTITY),
        ],
        kind: Kind::Grid,
    })
}

pub fn lighting() -> Lighting {
    Lighting {
        ambient: Vec3::ZERO,
        fog: Vec3::from_array(CANVAS),
        density: 0.0,
        shadowed: 0,
        ..Default::default()
    }
}

/// The orbit camera.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    /// A multiple of the tree's own viewing distance.
    pub zoom: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.2,
            zoom: 1.0,
        }
    }
}

impl Orbit {
    pub fn view(&self, shape: &Shape, aspect: f32) -> View {
        let fit = (shape.radius + 12.0) * if aspect < 1.0 { 3.2 } else { 1.95 };
        let distance = fit * self.zoom;
        let target = Vec3::new(0.0, (shape.root + shape.crown) * 0.45, 0.0);
        let eye = target
            + Vec3::new(
                self.yaw.sin() * self.pitch.cos(),
                self.pitch.sin(),
                self.yaw.cos() * self.pitch.cos(),
            ) * distance;
        View {
            eye,
            view_proj: Mat4::perspective_rh(46f32.to_radians(), aspect, 0.1, 4000.0)
                * Mat4::look_at_rh(eye, target, Vec3::Y),
        }
    }
}

struct Frame {
    out: Vec<Instance>,
}

impl Frame {
    fn full(&self) -> bool {
        self.out.len() >= INSTANCES
    }

    fn ball(&mut self, model: &str, at: Vec3, radius: f32) {
        if !self.full() && radius > 0.0 {
            self.out.push(instance(
                model,
                Mat4::from_scale_rotation_translation(Vec3::splat(radius), Quat::IDENTITY, at),
            ));
        }
    }

    fn block(&mut self, model: &str, at: Vec3, size: f32, turn: f32) {
        if !self.full() {
            self.out.push(instance(
                model,
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(size),
                    Quat::from_rotation_y(turn),
                    at,
                ),
            ));
        }
    }

    fn link(&mut self, model: &str, a: Vec3, b: Vec3) {
        let delta = b - a;
        if !self.full() && delta.length() > 1e-4 {
            self.out.push(instance(
                model,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(1.0, delta.length(), 1.0),
                    Quat::from_rotation_arc(Vec3::Y, delta.normalize()),
                    a,
                ),
            ));
        }
    }
}

/// An inner node's size: it grows with its balance, but stays small enough
/// that a ring of them doesn't merge.
fn inner_size(sum: u64, dust: u64) -> f32 {
    (0.06 + 0.018 * (1.0 + sum as f64 / dust.max(1) as f64).log2() as f32).min(0.5)
}

/// A member's size for its balance.
fn size(sum: u64, dust: u64) -> f32 {
    (0.1 + 0.04 * (1.0 + sum as f64 / dust.max(1) as f64).log2() as f32).min(1.2)
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn arc(a: Vec3, b: Vec3, lift: f32, t: f32) -> Vec3 {
    a.lerp(b, t) + Vec3::Y * (t * std::f32::consts::PI).sin() * lift
}

/// The lowest level drawn as spheres.
fn sphere_floor(stage: &Stage) -> u32 {
    let depth = stage.state.depth;
    if (1u64 << depth) <= SPHERE_SLOTS {
        0
    } else {
        depth.saturating_sub(SPHERE_LEVELS).max(1)
    }
}

/// Every instance for this frame.
pub fn instances(stage: &Stage) -> Vec<Instance> {
    let mut f = Frame {
        out: Vec::with_capacity(INSTANCES),
    };
    let shape = &stage.shape;
    let state = &stage.state;
    if state.depth == 0 {
        return f.out;
    }
    let now = stage.time;
    let down = state.escape.down;
    let dust = state.dust;
    let low = sphere_floor(stage);

    // The coin, the signer, and their links come first so a full frame
    // never drops them.
    let coin = shape.coin() + Vec3::Y * (now * 1.3).sin() * 0.15;
    let renewing = stage
        .effects
        .iter()
        .filter(|e| e.fx == Fx::Refresh)
        .map(|e| (e.progress(now) * std::f32::consts::PI).sin())
        .fold(0.0f32, f32::max);
    let spent = state.escape.steps.values().any(|s| s.confirmed);
    f.ball(
        if spent { "faint" } else { "signature" },
        coin,
        1.1 + renewing * 0.4,
    );
    f.link(
        "signature-link",
        Vec3::new(0.0, shape.root, 0.0),
        shape.coin(),
    );
    let signer = shape.signer();
    let killed = stage.killed_at.map(|at| now - at);
    let failed =
        !state.watch.attestation.result.is_empty() && state.watch.attestation.result != "attested";
    match killed {
        Some(since) => {
            let flash = since < 2.4 && (since * 6.0).sin() > 0.0;
            let sink = ease((since / 4.0).min(1.0)) * 2.5;
            f.block(
                if flash { "signer-bad" } else { "signer-dim" },
                signer - Vec3::Y * sink,
                1.1,
                0.3,
            );
        }
        None => {
            let model = if failed {
                "signer-bad"
            } else if state.watch.attestation.result.is_empty() {
                "signer-dim"
            } else {
                "signer"
            };
            f.block(model, signer, 1.1, now * 0.25);
            if !failed && !state.watch.attestation.result.is_empty() {
                f.ball(
                    "value",
                    signer + Vec3::Y * (2.0 + (now * 2.0).sin() * 0.15),
                    0.28,
                );
            }
            f.link("faint-link", signer, shape.coin());
        }
    }

    // The tree from the sphere floor up.
    for h in low..=state.depth {
        let count = 1u64 << (state.depth - h);
        for i in 0..count {
            let sum = stage.sum(h, i);
            if sum == 0 {
                continue;
            }
            let at = shape.position(h, i);
            let paid = h == 0 && stage.paid.contains(&i);
            let model = if paid || (down && spent) {
                "faint"
            } else {
                "value"
            };
            let radius = if h == state.depth {
                0.85
            } else if h == 0 {
                size(sum, dust) * 1.4
            } else {
                inner_size(sum, dust)
            };
            f.ball(model, at, radius);
            if paid {
                // The payout, landed on the floor below its slot.
                f.ball("value", shape.landing(i), radius);
            }
            if h < state.depth {
                let parent = shape.position(h + 1, i / 2);
                f.link(
                    if down && spent {
                        "faint-link"
                    } else {
                        "value-link"
                    },
                    at,
                    parent,
                );
            }
        }
    }

    // Light clients in a slow ring, gold for a minute after a good check.
    let n = stage.follower_count();
    for (i, member) in stage.followers.iter().enumerate() {
        let at = shape.follower(i, n, now);
        let model = match stage.follow.get(member) {
            Some((false, _)) => "danger",
            Some((true, when)) if now - when < 60.0 && killed.is_none() => "value",
            _ => "faint",
        };
        f.ball(model, at, 0.4);
    }

    // Recent blocks on an arc, newest first.
    for (i, block) in state.watch.blocks.iter().rev().take(14).enumerate() {
        let at = shape.block(i);
        let newest = i == 0;
        f.block(if newest { "signer" } else { "signer-dim" }, at, 0.45, 0.0);
        if block.hit {
            f.ball("value", at + Vec3::Y * 0.9, 0.18);
        }
    }

    // The unroll, level by level.
    if down {
        let mut levels: [Vec<bool>; 3] = Default::default();
        for step in state.escape.steps.values() {
            levels[step.level.min(2) as usize].push(step.confirmed);
        }
        for (level, steps) in levels.iter().enumerate() {
            let n = steps.len();
            for (i, confirmed) in steps.iter().enumerate() {
                let at = shape.package(level as u32, i, n);
                f.block(
                    if *confirmed { "box-gold" } else { "signer" },
                    at,
                    [0.7, 0.45, 0.26][level],
                    0.0,
                );
                if level > 0 {
                    let parents = levels[level - 1].len().max(1);
                    let parent = shape.package(level as u32 - 1, i * parents / n.max(1), parents);
                    f.link("signature-link", parent, at);
                }
            }
        }
        if state.escape.steps.is_empty() {
            f.block("signer-dim", shape.package(0, 0, 1), 0.7, 0.0);
        }
    }

    // Animations, newest last.
    for effect in &stage.effects {
        if f.full() {
            break;
        }
        let t = effect.progress(now);
        match effect.fx {
            Fx::Join { pos } => {
                let slot = shape.position(0, pos);
                if t < 0.4 {
                    let drop = ease(t / 0.4);
                    f.ball("value", slot + Vec3::Y * (1.0 - drop) * 9.0, 0.35);
                } else {
                    let path = shape.path(pos);
                    f.ball("value", Shape::along(&path, (t - 0.4) / 0.6), 0.28);
                }
            }
            Fx::Update { pos } => {
                let slot = shape.position(0, pos);
                if t < 0.45 {
                    let k = ease(t / 0.45);
                    for side in [-1.0, 1.0] {
                        let from = slot + Vec3::new(side * 3.0, 3.0, 0.0);
                        f.ball("signature", from.lerp(slot, k), 0.2);
                    }
                } else {
                    let path = shape.path(pos);
                    f.ball("signature", Shape::along(&path, (t - 0.45) / 0.55), 0.22);
                }
            }
            Fx::Claim { from, to } | Fx::Split { from, to } => {
                let (a, b) = (shape.position(0, from), shape.position(0, to));
                let lift = 3.0 + a.distance(b) * 0.25;
                let model = if matches!(effect.fx, Fx::Claim { .. }) {
                    "value"
                } else {
                    "signature"
                };
                for trail in 0..4 {
                    let k = (ease(t) - trail as f32 * 0.04).max(0.0);
                    f.ball(model, arc(a, b, lift, k), 0.32 - trail as f32 * 0.06);
                }
            }
            Fx::Exit { pos, amount } => {
                let slot = shape.position(0, pos);
                let away = Vec3::new(slot.x, 0.0, slot.z).normalize_or_zero();
                let end = slot + away * 14.0 + Vec3::Y * 6.0;
                f.ball(
                    "value",
                    arc(slot, end, 4.0, ease(t)),
                    size(amount, dust) * 1.6,
                );
                if t > 0.7 {
                    f.ball("signature", end, (t - 0.7) * 2.0);
                }
            }
            Fx::Refresh => {
                let level = ((t * stage.state.depth as f32) as u32).min(stage.state.depth);
                let h = stage.state.depth - level;
                let count = 1u64 << level;
                let step = (count / 24).max(1);
                for i in (0..count).step_by(step as usize) {
                    f.ball("signature", shape.position(h, i), 0.16);
                }
            }
            Fx::Sign if killed.is_none() => {
                f.ball("signature", signer.lerp(shape.coin(), ease(t)), 0.2);
            }
            Fx::Check { ok } => {
                f.ball(
                    if ok { "value" } else { "danger" },
                    signer + Vec3::Y * (1.2 + t * 2.5),
                    0.3 * (1.0 - t) + 0.05,
                );
            }
            Fx::Follow { member, ok } => {
                if let Some(i) = stage.followers.iter().position(|m| *m == member) {
                    let from = shape.follower(i, n, now);
                    let to = shape.block(0);
                    f.ball(
                        if ok { "value" } else { "danger" },
                        arc(from, to, 3.0, ease(t)),
                        0.14,
                    );
                }
            }
            Fx::Kill => {
                for i in 0..10 {
                    let angle = std::f32::consts::TAU * i as f32 / 10.0;
                    let dir = Vec3::new(angle.cos(), 0.4, angle.sin());
                    f.ball("danger", signer + dir * t * 6.0, 0.25 * (1.0 - t));
                }
            }
            Fx::Broadcast { level } => {
                let n = state
                    .escape
                    .steps
                    .values()
                    .filter(|s| s.level == level)
                    .count();
                let at = shape.package(level, n.saturating_sub(1), n);
                f.ball("signature", shape.coin().lerp(at, ease(t)), 0.2);
            }
            Fx::Payout { pos, amount } => {
                let slot = shape.position(0, pos);
                let end = shape.landing(pos);
                f.ball(
                    "value",
                    arc(slot, end, 2.0, ease(t)),
                    size(amount, dust) * 1.2,
                );
            }
            _ => {}
        }
    }
    f.out
}

fn project(view: View, position: Vec3, size: [f32; 2]) -> Option<([f32; 2], f32)> {
    let p = view.view_proj * position.extend(1.0);
    if p.w <= 0.1 {
        return None;
    }
    let n = p.truncate() / p.w;
    if n.x.abs() > 1.05 || n.y.abs() > 1.05 {
        return None;
    }
    Some((
        [(n.x + 1.0) * size[0] * 0.5, (1.0 - n.y) * size[1] * 0.5],
        p.w,
    ))
}

/// Text centered on `x`.
fn centered(ui: &mut UiBatch, atlas: &Atlas, x: f32, y: f32, text: &str, color: [f32; 4]) {
    let w = atlas.measure(text);
    ui.text(atlas, x - w * 0.5, y, text, color);
}

/// The members and lower tree as points, then every word on the page.
/// `size` is the drawing buffer in pixels and `scale` its pixels per CSS
/// pixel.
pub fn overlay(
    stage: &Stage,
    view: View,
    size: [f32; 2],
    scale: f32,
    atlas: &Atlas,
    show_hints: bool,
) -> UiBatch {
    let mut ui = UiBatch::default();
    let state = &stage.state;
    if state.depth == 0 {
        return ui;
    }
    let shape = &stage.shape;
    let s = scale;
    let now = stage.time;
    let down = state.escape.down;
    let spent = state.escape.steps.values().any(|st| st.confirmed);
    let low = sphere_floor(stage);
    let gold = rgba(GOLD, 1.0);
    let white = rgba(WHITE, 1.0);
    let line_h = atlas.measure("M") * 1.9;

    // The lower tree's links and nodes, faint, under the members.
    if low > 0 {
        for h in (1..low).rev() {
            for i in 0..1u64 << (state.depth - h) {
                if ui.vertices.len() > UI_VERTICES || stage.sum(h, i) == 0 {
                    continue;
                }
                let (Some((a, _)), Some((b, _))) = (
                    project(view, shape.position(h, i), size),
                    project(view, shape.position(h + 1, i / 2), size),
                ) else {
                    continue;
                };
                let alpha = if down && spent { 0.06 } else { 0.14 };
                ui.line(atlas, a, b, 1.0 * s, rgba(GOLD, alpha));
                ui.rect(
                    atlas,
                    a[0] - s,
                    a[1] - s,
                    2.0 * s,
                    2.0 * s,
                    rgba(GOLD, alpha * 2.0),
                );
            }
        }
        // Members: gold, white for a moment after they change, gray while
        // they wait for the unroll, and gold again where their payout lands.
        for (&pos, leaf) in &state.leaves {
            if ui.vertices.len() > UI_VERTICES {
                break;
            }
            let paid = stage.paid.contains(&pos);
            let world = if paid {
                shape.landing(pos)
            } else {
                shape.position(0, pos)
            };
            let Some((at, w)) = project(view, world, size) else {
                continue;
            };
            let near = (90.0 / w).clamp(0.5, 2.5);
            let r = (0.45
                + 0.13 * (1.0 + leaf.amount as f64 / state.dust.max(1) as f64).log2() as f32)
                * s
                * near;
            let fresh = stage
                .changed
                .get(&pos)
                .map_or(0.0, |when| (1.0 - (now - when) / 1.2).max(0.0));
            let color = if paid {
                rgba(GOLD, 0.95)
            } else if down {
                [0.5, 0.45, 0.32, 0.8]
            } else {
                let g = rgba(GOLD, 0.95);
                let mut c = [0.0; 4];
                for k in 0..4 {
                    c[k] = g[k] + (white[k] - g[k]) * fresh;
                }
                c
            };
            if let Some(parent) = (!paid)
                .then(|| project(view, shape.position(1, pos / 2), size))
                .flatten()
            {
                ui.line(
                    atlas,
                    at,
                    parent.0,
                    0.8 * s,
                    rgba(GOLD, if down { 0.03 } else { 0.06 }),
                );
            }
            ui.rect(atlas, at[0] - r, at[1] - r, 2.0 * r, 2.0 * r, color);
        }
    }

    // Labels on the scene.
    let pad = 24.0 * s;
    if let Some((at, _)) = project(view, shape.coin() - Vec3::Y * 1.3, size) {
        // Below the shared output, so the label never covers the trunk.
        centered(
            &mut ui,
            atlas,
            at[0],
            at[1] + line_h * 0.4,
            words::COIN,
            white,
        );
        let total = state.observed_output_amount.unwrap_or(state.output_amount);
        centered(
            &mut ui,
            atlas,
            at[0],
            at[1] + line_h * 1.4,
            &words::sats(total),
            gold,
        );
    }
    if let Some((at, _)) = project(view, shape.signer() + Vec3::Y * 2.6, size) {
        let status = if stage.killed_at.is_some() {
            words::SIGNER_OFF
        } else if state.watch.attestation.result.is_empty() {
            words::SIGNER_UNCHECKED
        } else if state.watch.attestation.result == "attested" {
            words::SIGNER_OK
        } else {
            words::CHECK_FAILED
        };
        let at = [at[0], at[1] - line_h * 2.0];
        centered(&mut ui, atlas, at[0], at[1], words::SIGNER, white);
        let color = if stage.killed_at.is_some() || status == words::CHECK_FAILED {
            rgba(RED, 1.0)
        } else {
            DIM
        };
        centered(&mut ui, atlas, at[0], at[1] + line_h, status, color);
    }
    if stage.follower_count() > 0 {
        let n = stage.follower_count();
        if let Some((at, _)) = project(view, shape.follower(n / 4, n, now) - Vec3::Y * 0.8, size) {
            centered(
                &mut ui,
                atlas,
                at[0],
                at[1] + line_h * 0.4,
                words::FOLLOWERS,
                DIM,
            );
        }
    }
    if let Some(block) = state.watch.blocks.last() {
        if let Some((at, _)) = project(view, shape.block(0) - Vec3::Y * 0.8, size) {
            centered(
                &mut ui,
                atlas,
                at[0],
                at[1] + line_h * 0.4,
                &words::block(block.height),
                DIM,
            );
        }
    }

    // The header.
    let mut y = pad;
    ui.text(atlas, pad, y, words::TITLE, gold);
    y += line_h;
    ui.text(atlas, pad, y, words::subtitle(stage.source), DIM);
    y += line_h * 1.3;
    let members = if down {
        stage.owed_members
    } else {
        state.leaves.len()
    };
    ui.text(atlas, pad, y, &words::members(members), white);
    y += line_h;
    ui.text(atlas, pad, y, &words::sats(state.output_amount), gold);
    if let Some(height) = state.height {
        y += line_h;
        ui.text(atlas, pad, y, &words::block(height), DIM);
    }

    // The ticker of recent transactions, fading with age.
    let right = size[0] - pad;
    let mut ty = pad;
    for (text, when) in stage.ticker.iter().rev() {
        let alpha = (1.0 - (now - when) / 12.0).clamp(0.15, 1.0);
        let w = atlas.measure(text);
        ui.text(atlas, right - w, ty, text, rgba(WHITE, alpha));
        ty += line_h;
    }

    // The caption, centered near the foot, on a dark band.
    let caption = &stage.caption.0;
    if !caption.is_empty() {
        let max = size[0] - 2.0 * pad;
        let lines = atlas.wrap(caption, max.min(900.0 * s));
        let age = now - stage.caption.1;
        let alpha = (age * 4.0).clamp(0.0, 1.0);
        let block_h = line_h * lines.len() as f32;
        let top = size[1] - pad * 2.2 - block_h;
        let widest = lines.iter().map(|l| atlas.measure(l)).fold(0.0, f32::max);
        ui.rect(
            atlas,
            size[0] * 0.5 - widest * 0.5 - pad * 0.6,
            top - line_h * 0.45,
            widest + pad * 1.2,
            block_h + line_h * 0.5,
            [0.02, 0.02, 0.03, 0.85 * alpha],
        );
        for (i, line) in lines.iter().enumerate() {
            centered(
                &mut ui,
                atlas,
                size[0] * 0.5,
                top + i as f32 * line_h,
                line,
                rgba(WHITE, alpha),
            );
        }
    }

    // The legend, bottom left, when there is room for it.
    if size[0] > 700.0 * s {
        let swatches: [[f32; 4]; 5] = [gold, white, white, rgba(GOLD, 0.6), gold];
        let mut ly = size[1] - pad - line_h * words::LEGEND.len() as f32;
        for (i, (text, color)) in words::LEGEND.iter().zip(swatches).enumerate() {
            let cx = pad + 6.0 * s;
            let cy = ly + line_h * 0.3;
            match i {
                2 => ui.rect(atlas, cx - 5.0 * s, cy - 5.0 * s, 10.0 * s, 10.0 * s, color),
                3 => ui.ring(atlas, cx, cy, 5.0 * s, 1.5 * s, color),
                4 => {
                    ui.rect(atlas, cx - 3.0 * s, cy - 3.0 * s, 6.0 * s, 6.0 * s, color);
                    ui.line(
                        atlas,
                        [cx, cy + 4.0 * s],
                        [cx, cy + 9.0 * s],
                        1.5 * s,
                        color,
                    );
                }
                _ => ui.disc(atlas, cx, cy, if i == 1 { 6.0 * s } else { 4.0 * s }, color),
            }
            ui.text(atlas, pad + 20.0 * s, ly, text, DIM);
            ly += line_h;
        }
    }

    if show_hints {
        let hints = [words::HINT, words::switch_hint(stage.source)];
        let mut hy = size[1] - pad - line_h * hints.len() as f32;
        for hint in hints {
            let w = atlas.measure(hint);
            ui.text(atlas, right - w, hy, hint, DIM);
            hy += line_h;
        }
    }
    ui
}
