//! The Verse scene: a dark Greco-futurist plaza at night. The issue is an
//! obelisk at the center on a stepped podium, ringed by a colonnade. Each
//! candidate file stands on a plinth around it, grouped by directory, and
//! carries one bar per lane: its height is that lane's probability, it
//! glows past the threshold, and it flares as its answer lands. A plinth
//! flashes when the lanes split across the threshold, a laurel square marks
//! the files the fix changed, and beams run from the obelisk to the files
//! the lanes call relevant. The scoreboard, the issue, and the ranking are
//! drawn over the world.
use super::cases::{
    AMBER, Backend, COPPER, Case, LAUREL, LaneStatus, Origin, THRESHOLD, dir, ranking, registry,
    secs,
};
use super::run::Board;
use glam::{Mat4, Vec3, Vec4};
use std::f32::consts::{PI, TAU};
use verse::imported::{flat, lighting::Lighting};
use verse::ui::{Atlas, UiBatch};
use verse_engine::assets::{Pack, Surface, Topology, Vertex};
use verse_engine::lighting::HeightFog;
use verse_engine::presentation::Instance;

/// Brightness steps for lit models; level `l` is `0.12 * 1.38^l`.
pub const LEVELS: usize = 10;
const FILE_RING: f32 = 12.5;
const COLONNADE: f32 = 31.0;
const COLUMNS: usize = 20;
const BAR_MAX: f32 = 4.2;
const OBELISK: f32 = 8.0;

fn brightness(level: usize) -> f32 {
    0.12 * 1.38f32.powi(level as i32)
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn vertex(p: Vec3) -> Vertex {
    Vertex {
        position: p.to_array(),
        normal: [0., 1., 0.],
        uv: [0.5, 0.5],
        joints: [0; 4],
        weights: [1., 0., 0., 0.],
    }
}

fn triangle(s: &mut Surface, a: Vec3, b: Vec3, c: Vec3) {
    let base = s.vertices.len() as u32;
    s.vertices.extend([vertex(a), vertex(b), vertex(c)]);
    s.indices.extend([base, base + 1, base + 2]);
}

/// Light from the upper left, baked into the face tints: the scene is
/// unlit, so a face's shade is fixed by which way it points.
fn shade(normal: Vec3) -> f32 {
    let light = Vec3::new(-0.45, 0.75, 0.5).normalize();
    0.32 + 0.68 * normal.normalize().dot(light).max(0.0)
}

/// A frustum of `sides` faces from radius `r0` at y=0 to `r1` at `h`,
/// with an optional apex above the top, one tinted surface per face.
fn prism(
    white: usize,
    color: [f32; 3],
    sides: usize,
    r0: f32,
    r1: f32,
    h: f32,
    apex: Option<f32>,
    turn: f32,
) -> Vec<Surface> {
    let mut out = Vec::new();
    let at = |i: usize, r: f32, y: f32| {
        let a = turn + i as f32 / sides as f32 * TAU;
        Vec3::new(r * a.cos(), y, r * a.sin())
    };
    for i in 0..sides {
        let (a0, a1) = (at(i, r0, 0.), at(i + 1, r0, 0.));
        let (b0, b1) = (at(i, r1, h), at(i + 1, r1, h));
        let normal = (a1 - a0).cross(b0 - a0).normalize() * -1.0;
        let mut s = flat::surface(white, scale(color, shade(normal)), Topology::Triangles);
        flat::quad(&mut s, [a0, a1, b1, b0]);
        out.push(s);
        if let Some(top) = apex {
            let tip = Vec3::new(0., h + top, 0.);
            let n = (b1 - b0).cross(tip - b0).normalize() * -1.0;
            let mut s = flat::surface(white, scale(color, shade(n) * 1.05), Topology::Triangles);
            triangle(&mut s, b0, b1, tip);
            out.push(s);
        }
    }
    if apex.is_none() {
        let mut s = flat::surface(white, scale(color, shade(Vec3::Y)), Topology::Triangles);
        for i in 0..sides {
            triangle(&mut s, Vec3::new(0., h, 0.), at(i + 1, r1, h), at(i, r1, h));
        }
        out.push(s);
    }
    out
}

/// A unit box standing on y=0 (x and z from -0.5 to 0.5, y from 0 to 1),
/// its faces shaded and, when asked, its edges drawn in a brighter line.
fn shaded_box(white: usize, color: [f32; 3], edges: Option<[f32; 3]>) -> Vec<Surface> {
    let mut out = Vec::new();
    let c = |x: f32, y: f32, z: f32| Vec3::new(x - 0.5, y, z - 0.5);
    let faces: [(Vec3, [Vec3; 4]); 5] = [
        (
            Vec3::Y,
            [c(0., 1., 0.), c(1., 1., 0.), c(1., 1., 1.), c(0., 1., 1.)],
        ),
        (
            Vec3::X,
            [c(1., 0., 0.), c(1., 0., 1.), c(1., 1., 1.), c(1., 1., 0.)],
        ),
        (
            -Vec3::X,
            [c(0., 0., 1.), c(0., 0., 0.), c(0., 1., 0.), c(0., 1., 1.)],
        ),
        (
            Vec3::Z,
            [c(1., 0., 1.), c(0., 0., 1.), c(0., 1., 1.), c(1., 1., 1.)],
        ),
        (
            -Vec3::Z,
            [c(0., 0., 0.), c(1., 0., 0.), c(1., 1., 0.), c(0., 1., 0.)],
        ),
    ];
    for (normal, corners) in faces {
        let mut s = flat::surface(white, scale(color, shade(normal)), Topology::Triangles);
        flat::quad(&mut s, corners);
        out.push(s);
    }
    if let Some(edge) = edges {
        let mut s = flat::surface(white, edge, Topology::Lines);
        flat::cube_edges(&mut s, Mat4::from_translation(Vec3::new(0., 0.5, 0.)));
        out.push(s);
    }
    out
}

fn circle(s: &mut Surface, r: f32, y: f32, segments: usize) {
    for i in 0..segments {
        let a = |i: usize| i as f32 / segments as f32 * TAU;
        flat::line(
            s,
            Vec3::new(r * a(i).cos(), y, r * a(i).sin()),
            Vec3::new(r * a(i + 1).cos(), y, r * a(i + 1).sin()),
        );
    }
}

/// The limestone and bronze of the style guide, dimmed for night.
const STONE: [f32; 3] = [0.748 * 0.16, 0.638 * 0.16, 0.448 * 0.16];
const STONE_SHADE: [f32; 3] = [0.538 * 0.13, 0.448 * 0.13, 0.307 * 0.13];
const BRONZE: [f32; 3] = [0.047 * 1.4, 0.020 * 1.4, 0.009 * 1.4];
const FIELD: [f32; 3] = [0.0045, 0.0050, 0.0075];
/// Red-brown, lifted until it reads as a warning in the dark.
const SPLIT: [f32; 3] = [0.9, 0.12, 0.04];

/// Every model the scene draws, made in code; no file on disk but the
/// white texel.
pub fn pack(dir: &std::path::Path) -> Result<Pack, String> {
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "relevance-visualizer".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    let white = flat::white_texture(&mut pack, dir)?;
    let mut add = |name: String, surfaces: Vec<Surface>| {
        pack.models
            .insert(name.clone(), flat::model(&name, surfaces, 1.0));
    };

    // The plaza: a dark stone disc, paving joints, and rings of amber inlay.
    let mut floor = flat::surface(white, [0.016, 0.014, 0.012], Topology::Triangles);
    for i in 0..96 {
        let a = |i: usize| i as f32 / 96. * TAU;
        triangle(
            &mut floor,
            Vec3::ZERO,
            Vec3::new(38. * a(i + 1).cos(), 0., 38. * a(i + 1).sin()),
            Vec3::new(38. * a(i).cos(), 0., 38. * a(i).sin()),
        );
    }
    let mut joints = flat::surface(white, [0.035, 0.03, 0.024], Topology::Lines);
    for r in [9.0, 11.5, 17.0, 19.5, 22.0, 24.5, 27.0] {
        circle(&mut joints, r, 0.01, 128);
    }
    for i in 0..48 {
        let a = i as f32 / 48. * TAU;
        let d = Vec3::new(a.cos(), 0., a.sin());
        flat::line(
            &mut joints,
            d * 4.2 + Vec3::Y * 0.01,
            d * 29.0 + Vec3::Y * 0.01,
        );
    }
    let mut inlay = flat::surface(white, scale(AMBER, 0.22), Topology::Lines);
    circle(&mut inlay, 6.6, 0.02, 128);
    circle(&mut inlay, FILE_RING - 2.4, 0.02, 160);
    circle(&mut inlay, FILE_RING + 2.4, 0.02, 160);
    circle(&mut inlay, COLONNADE - 1.6, 0.02, 192);
    add("plaza".into(), vec![floor, joints, inlay]);

    // A dusk sky: bands from a warm horizon to a dark zenith, as the
    // belvedere's reference at dusk.
    let mut sky = Vec::new();
    let bands = 14;
    for b in 0..bands {
        let (e0, e1) = (
            b as f32 / bands as f32 * PI / 2.,
            (b + 1) as f32 / bands as f32 * PI / 2.,
        );
        let k = (b as f32 / (bands - 1) as f32).powf(0.6);
        let horizon = [0.075, 0.034, 0.014];
        let zenith = [0.004, 0.006, 0.014];
        let tint = [0, 1, 2].map(|i| horizon[i] + (zenith[i] - horizon[i]) * k);
        let mut s = flat::surface(white, tint, Topology::Triangles);
        let at = |a: f32, e: f32| {
            Vec3::new(
                160. * e.cos() * a.cos(),
                160. * e.sin() - 6.,
                160. * e.cos() * a.sin(),
            )
        };
        for i in 0..48 {
            let (a0, a1) = (i as f32 / 48. * TAU, (i + 1) as f32 / 48. * TAU);
            flat::quad(&mut s, [at(a0, e0), at(a0, e1), at(a1, e1), at(a1, e0)]);
        }
        sky.push(s);
    }
    add("sky".into(), sky);

    // The colonnade: smooth columns under square capitals, joined by an
    // architrave, as the style guide reduces the temple.
    let mut column = prism(white, STONE, 12, 0.45, 0.42, 7.0, None, 0.);
    column.extend(shaded_box(white, STONE, None).into_iter().map(|mut s| {
        for v in &mut s.vertices {
            let p = Vec3::from_array(v.position);
            v.position = (p * Vec3::new(1.26, 0.22, 1.26) + Vec3::Y * 7.0).to_array();
        }
        s
    }));
    column.extend(
        shaded_box(white, STONE_SHADE, None)
            .into_iter()
            .map(|mut s| {
                for v in &mut s.vertices {
                    let p = Vec3::from_array(v.position);
                    v.position = (p * Vec3::new(1.2, 0.3, 1.2)).to_array();
                }
                s
            }),
    );
    add("column".into(), column);
    add("stone".into(), shaded_box(white, STONE, None));
    add(
        "stone-shade".into(),
        shaded_box(white, STONE_SHADE, Some(scale(AMBER, 0.12))),
    );

    // The obelisk: bronze, a pyramidion, amber edges, copper circuit traces.
    let mut obelisk = prism(white, BRONZE, 4, 1.05, 0.72, OBELISK, Some(1.1), PI / 4.);
    let mut edges = flat::surface(white, scale(AMBER, 2.4), Topology::Lines);
    let corner = |i: usize, r: f32, y: f32| {
        let a = PI / 4. + i as f32 / 4. * TAU;
        Vec3::new(r * a.cos(), y, r * a.sin())
    };
    for i in 0..4 {
        flat::line(&mut edges, corner(i, 1.05, 0.), corner(i, 0.72, OBELISK));
        flat::line(
            &mut edges,
            corner(i, 0.72, OBELISK),
            corner(i + 1, 0.72, OBELISK),
        );
        flat::line(
            &mut edges,
            corner(i, 0.72, OBELISK),
            Vec3::new(0., OBELISK + 1.1, 0.),
        );
    }
    obelisk.push(edges);
    let mut traces = flat::surface(white, scale(COPPER, 1.6), Topology::Lines);
    for face in 0..4 {
        let a = face as f32 / 4. * TAU;
        let (out, side) = (
            Vec3::new(a.cos(), 0., a.sin()),
            Vec3::new(-a.sin(), 0., a.cos()),
        );
        let at = |y: f32, s: f32| {
            let r = 1.05 + (0.72 - 1.05) * (y / OBELISK) + 0.015;
            out * r + side * s + Vec3::Y * y
        };
        flat::line(&mut traces, at(0.8, 0.), at(6.9, 0.));
        for (y, w) in [(1.6, 0.42), (2.9, 0.3), (4.2, 0.36), (5.5, 0.22)] {
            for sign in [-1., 1.] {
                flat::line(&mut traces, at(y, 0.), at(y, sign * w));
                flat::line(&mut traces, at(y, sign * w), at(y - 0.35, sign * w));
                let pad = at(y - 0.35, sign * w);
                for (d0, d1) in [(-0.05, 0.05)] {
                    flat::line(&mut traces, pad + side * d0, pad + side * d1);
                }
            }
        }
        let pane = |y: f32, s: f32| at(y, s);
        flat::line(&mut traces, pane(7.15, -0.18), pane(7.15, 0.18));
        flat::line(&mut traces, pane(7.45, -0.12), pane(7.45, 0.12));
    }
    obelisk.push(traces);
    add("obelisk".into(), obelisk);

    // Lane bars and file plinths, one model per brightness step.
    for (i, backend) in registry().iter().enumerate() {
        for level in 0..LEVELS {
            let b = brightness(level);
            add(
                format!("bar/{i}/{level}"),
                shaded_box(
                    white,
                    scale(backend.color, b),
                    Some(scale(backend.color, b * 1.7)),
                ),
            );
        }
    }
    for level in 0..LEVELS {
        let b = brightness(level);
        let mut beam = flat::surface(white, scale(AMBER, b), Topology::Lines);
        flat::line(&mut beam, Vec3::ZERO, Vec3::Z);
        add(format!("beam/{level}"), vec![beam]);
        let mut split = flat::surface(white, scale(SPLIT, b), Topology::Lines);
        square(&mut split, 0.0);
        add(format!("split/{level}"), vec![split]);
        let mut halo = flat::surface(white, scale(AMBER, b), Topology::Lines);
        circle(&mut halo, 1.0, 0.0, 96);
        add(format!("halo/{level}"), vec![halo]);
    }
    let mut truth = flat::surface(white, scale(LAUREL, 3.0), Topology::Lines);
    square(&mut truth, 0.0);
    square(&mut truth, 0.04);
    add("truth".into(), vec![truth]);
    add("spark".into(), shaded_box(white, scale(AMBER, 7.0), None));
    pack.validate()?;
    Ok(pack)
}

/// A unit square outline in the xz plane at height `y`, inset by `inset`.
fn square(s: &mut Surface, inset: f32) {
    let h = 0.5 - inset;
    let p = [
        Vec3::new(-h, 0., -h),
        Vec3::new(h, 0., -h),
        Vec3::new(h, 0., h),
        Vec3::new(-h, 0., h),
    ];
    for i in 0..4 {
        flat::line(s, p[i], p[(i + 1) % 4]);
    }
}

pub fn instance(model: impl Into<String>, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

/// The plaza, the colonnade, the podium, and the obelisk: drawn every frame
/// without change.
pub fn static_instances() -> Vec<Instance> {
    let mut out = vec![
        instance("plaza", Mat4::IDENTITY),
        instance("sky", Mat4::IDENTITY),
    ];
    for i in 0..COLUMNS {
        let a = (i as f32 + 0.5) / COLUMNS as f32 * TAU;
        let at = Vec3::new(COLONNADE * a.cos(), 0., COLONNADE * a.sin());
        out.push(instance("column", Mat4::from_translation(at)));
        // The architrave from this column to the next.
        let b = (i as f32 + 1.5) / COLUMNS as f32 * TAU;
        let next = Vec3::new(COLONNADE * b.cos(), 0., COLONNADE * b.sin());
        let mid = (at + next) * 0.5 + Vec3::Y * 7.22;
        let span = (next - at).length() + 1.2;
        let yaw = -(next - at).z.atan2((next - at).x);
        out.push(instance(
            "stone",
            Mat4::from_translation(mid)
                * Mat4::from_rotation_y(yaw)
                * Mat4::from_scale(Vec3::new(span, 0.9, 1.1)),
        ));
    }
    for (i, (w, h)) in [(7.0, 0.3), (5.4, 0.3), (3.8, 0.3)].iter().enumerate() {
        out.push(instance(
            "stone-shade",
            Mat4::from_translation(Vec3::Y * (i as f32 * 0.3))
                * Mat4::from_scale(Vec3::new(*w, *h, *w)),
        ));
    }
    out.push(instance("obelisk", Mat4::from_translation(Vec3::Y * 0.9)));
    out
}

/// Where each file stands: angles around the ring, with a gap between
/// directories so each directory reads as a group.
pub fn layout(case: &Case) -> Vec<f32> {
    let n = case.candidates.len();
    if n == 0 {
        return vec![];
    }
    let mut slots = Vec::with_capacity(n);
    let mut at = 0.0f32;
    for (i, c) in case.candidates.iter().enumerate() {
        if i > 0 && dir(&case.candidates[i - 1].path) != dir(&c.path) {
            at += 0.7;
        }
        slots.push(at);
        at += 1.0;
    }
    let groups_gap = if dir(&case.candidates[0].path) != dir(&case.candidates[n - 1].path) {
        0.7
    } else {
        0.0
    };
    let total = at + groups_gap;
    slots.iter().map(|s| -PI / 2. + s / total * TAU).collect()
}

/// When each lane's answer for each file arrived, in scene seconds, for
/// the bars' rise and flare.
pub struct Arrivals(pub Vec<Vec<Option<f32>>>);

/// The camera: a slow orbit around the obelisk.
pub struct Camera {
    pub yaw: f32,
    pub distance: f32,
}

impl Camera {
    pub fn view(&self, aspect: f32) -> (verse::render::View, Mat4) {
        let eye = Vec3::new(
            self.yaw.cos() * self.distance,
            self.distance * 0.78,
            self.yaw.sin() * self.distance,
        );
        let target = Vec3::new(0., 1.2, 0.);
        // Off center: the plaza sits left of the scoreboard and ranking.
        let view_proj = Mat4::from_translation(Vec3::new(-0.2, 0.1, 0.))
            * Mat4::perspective_rh(52f32.to_radians(), aspect, 0.2, 400.)
            * Mat4::look_at_rh(eye, target, Vec3::Y);
        (verse::render::View { view_proj, eye }, view_proj)
    }
}

pub fn lighting() -> Lighting {
    Lighting {
        ambient: Vec3::ZERO,
        fog: Vec3::from_array(FIELD),
        density: 0.0,
        shadowed: 0,
        height_fog: Some(HeightFog {
            density: 0.018,
            base: 0.0,
            falloff: 0.05,
            start: 26.0,
            max_opacity: 0.92,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }),
        ..Default::default()
    }
}

/// The level of a bar for probability `p`, flaring for `age` seconds after
/// its answer lands.
pub fn bar_level(p: f64, age: f32) -> usize {
    let base = if p >= THRESHOLD {
        5 + ((p - THRESHOLD) / (1.0 - THRESHOLD) * 3.0).round() as usize
    } else {
        1 + (p / THRESHOLD * 2.0).round() as usize
    };
    let flare = if age < 0.9 {
        ((0.9 - age) / 0.9 * 3.0).round() as usize
    } else {
        0
    };
    (base + flare).min(LEVELS - 1)
}

/// The world's moving parts this frame.
pub fn instances(
    board: &Board,
    lane_ids: &[usize],
    angles: &[f32],
    arrivals: &Arrivals,
    t: f32,
) -> Vec<Instance> {
    let mut out = Vec::new();
    let lanes = board.lanes.len().max(1);
    let rows = ranking(&board.lanes, board.case.candidates.len());
    let answered_any = board.lanes.iter().any(|l| l.answered() > 0);
    // The obelisk's halo breathes while the lanes run.
    let pulse = (t * 2.2).sin() * 0.5 + 0.5;
    let halo_level = if board.finished() {
        5
    } else {
        3 + (pulse * 3.0) as usize
    };
    for (r, l) in [(1.9, halo_level), (2.5, halo_level.saturating_sub(2))] {
        out.push(instance(
            format!("halo/{l}"),
            Mat4::from_translation(Vec3::Y * 0.93) * Mat4::from_scale(Vec3::new(r, 1., r)),
        ));
    }
    for (file, &angle) in angles.iter().enumerate() {
        let out_dir = Vec3::new(angle.cos(), 0., angle.sin());
        let center = out_dir * FILE_RING;
        // Tangent along the ring; the plinth faces the obelisk.
        let yaw = -angle + PI / 2.;
        let frame = Mat4::from_translation(center) * Mat4::from_rotation_y(yaw);
        let width = 0.5 * lanes as f32 + 0.6;
        out.push(instance(
            "stone-shade",
            frame * Mat4::from_scale(Vec3::new(width, 0.5, 1.3)),
        ));
        if board.labels[file] == Some(true) {
            out.push(instance(
                "truth",
                frame
                    * Mat4::from_translation(Vec3::Y * 0.03)
                    * Mat4::from_scale(Vec3::new(width + 0.7, 1., 2.0)),
            ));
        }
        let ps: Vec<Option<f64>> = board.lanes.iter().map(|l| l.p[file]).collect();
        if super::cases::disagree(&ps) {
            let blink = ((t * 6.0).sin() * 0.5 + 0.5) * 7.0;
            out.push(instance(
                format!("split/{}", 2 + blink as usize),
                frame
                    * Mat4::from_translation(Vec3::Y * 0.52)
                    * Mat4::from_scale(Vec3::new(width + 0.25, 1., 1.55)),
            ));
        }
        for (k, lane) in board.lanes.iter().enumerate() {
            let x = (k as f32 - (lanes as f32 - 1.0) / 2.0) * 0.5;
            let reg = lane_ids[k];
            let (height, level) = match (lane.p[file], &lane.status) {
                (Some(p), _) => {
                    let age = arrivals.0[k][file].map_or(9.0, |a| t - a);
                    let rise = (age / 0.6).clamp(0.0, 1.0);
                    let ease = 1.0 - (1.0 - rise).powi(3);
                    (0.06 + p as f32 * BAR_MAX * ease, bar_level(p, age))
                }
                (None, LaneStatus::Offline(_)) => continue,
                (None, _) => (0.05, 0),
            };
            out.push(instance(
                format!("bar/{reg}/{level}"),
                frame
                    * Mat4::from_translation(Vec3::new(x, 0.5, 0.))
                    * Mat4::from_scale(Vec3::new(0.36, height, 0.6)),
            ));
        }
        // A beam from the obelisk to every file the lanes call relevant.
        let row = rows.iter().find(|r| r.file == file);
        if let Some(mean) = row.and_then(|r| r.mean)
            && mean >= THRESHOLD
            && answered_any
        {
            let from = Vec3::Y * (OBELISK * 0.82);
            let top = 0.6 + mean as f32 * BAR_MAX;
            let to = center - out_dir * 0.3 + Vec3::Y * top;
            let d = to - from;
            let level = 3 + ((mean - THRESHOLD) / (1.0 - THRESHOLD) * 4.0).round() as usize;
            let look = Mat4::from_translation(from)
                * look_along(d)
                * Mat4::from_scale(Vec3::new(1., 1., d.length()));
            out.push(instance(format!("beam/{}", level.min(LEVELS - 1)), look));
            for phase in [0.0, 0.5] {
                let f = ((t * 0.45 + file as f32 * 0.137 + phase) % 1.0).powf(1.3);
                let p = from + d * f;
                out.push(instance(
                    "spark",
                    Mat4::from_translation(p - Vec3::Y * 0.06)
                        * Mat4::from_scale(Vec3::splat(0.12)),
                ));
            }
        }
    }
    out
}

/// A rotation that turns +Z toward `d`.
fn look_along(d: Vec3) -> Mat4 {
    let f = d.normalize_or(Vec3::Z);
    let up = if f.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let r = up.cross(f).normalize();
    let u = f.cross(r);
    Mat4::from_cols(r.extend(0.), u.extend(0.), f.extend(0.), Vec4::W)
}

fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

const INK: [f32; 4] = [0.82, 0.80, 0.74, 1.0];
const DIM: [f32; 4] = [0.30, 0.28, 0.25, 1.0];
const PANEL: [f32; 4] = [0.004, 0.004, 0.006, 0.78];
const RULE: [f32; 4] = [0.30, 0.13, 0.03, 0.9];

fn panel(ui: &mut UiBatch, atlas: &Atlas, x: f32, y: f32, w: f32, h: f32) {
    ui.rect(atlas, x, y, w, h, PANEL);
    ui.rect(atlas, x, y, w, 1.0, RULE);
    ui.rect(atlas, x, y + h - 1.0, w, 1.0, RULE);
}

fn fit(atlas: &Atlas, text: &str, width: f32) -> String {
    if atlas.measure(text) <= width {
        return text.to_owned();
    }
    let mut s: String = text.to_owned();
    while !s.is_empty() && atlas.measure(&format!("..{s}")) > width {
        s.remove(0);
    }
    format!("..{s}")
}

fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => ' ',
        })
        .collect()
}

fn project(vp: Mat4, p: Vec3, size: [f32; 2]) -> Option<[f32; 2]> {
    let clip = vp * p.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some([(ndc.x + 1.0) * 0.5 * size[0], (1.0 - ndc.y) * 0.5 * size[1]])
}

/// What the overlay needs beyond the board.
pub struct Hud<'a> {
    pub backends: &'a [Backend],
    pub enabled: &'a [bool],
    pub message: &'a str,
    pub elapsed: f32,
}

/// The overlay, in a 720-pixel-high canvas `size` wide.
pub fn overlay(
    board: Option<&Board>,
    angles: &[f32],
    vp: Mat4,
    size: [f32; 2],
    atlas: &Atlas,
    hud: &Hud<'_>,
) -> UiBatch {
    let mut ui = UiBatch::default();
    let line = atlas.line.max(14.0);
    let [w, h] = size;
    // Keys, always.
    let keys =
        "R new issue   SPACE rerun   1-9 lanes on/off   LEFT/RIGHT orbit   UP/DOWN zoom   ESC quit";
    ui.text(atlas, 24.0, h - 30.0, keys, DIM);
    if !hud.message.is_empty() {
        ui.text(
            atlas,
            24.0,
            h - 30.0 - line * 6.4,
            &ascii(hud.message),
            rgba(AMBER, 1.0),
        );
    }
    let Some(board) = board else {
        return ui;
    };
    let case = &board.case;

    // File labels over the plinths.
    for (file, &angle) in angles.iter().enumerate() {
        let c = &case.candidates[file];
        let top = board
            .lanes
            .iter()
            .filter_map(|l| l.p[file])
            .fold(0.0f64, f64::max) as f32;
        let at =
            Vec3::new(angle.cos(), 0., angle.sin()) * FILE_RING + Vec3::Y * (1.1 + top * BAR_MAX);
        let Some([x, y]) = project(vp, at, size) else {
            continue;
        };
        let name = c.path.rsplit('/').next().unwrap_or(&c.path);
        let mean: Vec<f64> = board.lanes.iter().filter_map(|l| l.p[file]).collect();
        let hot = !mean.is_empty() && mean.iter().sum::<f64>() / mean.len() as f64 >= THRESHOLD;
        let color = if hot { INK } else { [0.42, 0.40, 0.36, 1.0] };
        let tw = atlas.measure(name);
        ui.text(atlas, x - tw / 2.0, y - line, name, color);
        if board.labels[file] == Some(true) {
            let tag = "fix";
            ui.text(
                atlas,
                x - atlas.measure(tag) / 2.0,
                y - line * 2.0,
                tag,
                rgba(LAUREL, 1.0).map(|v| (v * 3.0).min(1.0)),
            );
        }
    }
    // The issue number over the obelisk.
    if let Some([x, y]) = project(vp, Vec3::Y * (OBELISK + 2.6), size) {
        let tag = format!("#{}", case.issue.number);
        ui.text(
            atlas,
            x - atlas.measure(&tag) / 2.0,
            y,
            &tag,
            rgba(AMBER, 1.0),
        );
    }

    // The issue, top left.
    let pw = (w * 0.36).clamp(360.0, 560.0);
    let title = atlas.wrap(&ascii(&case.issue.title), pw - 32.0);
    let mut lines: Vec<(String, [f32; 4])> = vec![(
        format!("ISSUE #{}   {}", case.issue.number, case.issue.state),
        rgba(AMBER, 1.0),
    )];
    lines.extend(title.into_iter().take(3).map(|t| (t, INK)));
    match &case.fix {
        Some(f) => lines.push((
            format!(
                "ground truth: {} file{} changed by {}",
                f.files.len(),
                if f.files.len() == 1 { "" } else { "s" },
                f.commits
                    .iter()
                    .map(|c| &c[..9.min(c.len())])
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            rgba(LAUREL, 1.0).map(|v| (v * 3.0).min(1.0)),
        )),
        None => lines.push(("no fix commit on main: no ground truth".into(), DIM)),
    }
    let count = |o: Origin| case.candidates.iter().filter(|c| c.origin == o).count();
    lines.push((
        format!(
            "{} files: {} fix, {} sibling, {} recent, {} random",
            case.candidates.len(),
            count(Origin::Fix),
            count(Origin::Sibling),
            count(Origin::Recent),
            count(Origin::Random)
        ),
        DIM,
    ));
    let lines: Vec<(String, [f32; 4])> = lines
        .into_iter()
        .flat_map(|(t, c)| atlas.wrap(&t, pw - 32.0).into_iter().map(move |l| (l, c)))
        .collect();
    let ph = 20.0 + lines.len() as f32 * line;
    panel(&mut ui, atlas, 16.0, 16.0, pw, ph);
    for (i, (text, color)) in lines.iter().enumerate() {
        ui.text(
            atlas,
            32.0,
            26.0 + i as f32 * line,
            &fit(atlas, text, pw - 32.0),
            *color,
        );
    }

    // The scoreboard, top right.
    let ch = atlas.advance;
    let sw = (ch * 62.0 + 46.0).min(w * 0.5);
    let sx = w - sw - 16.0;
    let rows = hud.backends.len();
    panel(
        &mut ui,
        atlas,
        sx,
        16.0,
        sw,
        20.0 + (rows as f32 + 1.0) * line + 6.0,
    );
    let cols = [0.0, 16.0, 23.0, 30.0, 38.0, 45.0, 53.0].map(|c| c * ch);
    let head = ["LANE", "DONE", "DEC/S", "P50", "PREC", "RECALL", "ACC"];
    for (c, text) in cols.iter().zip(head) {
        ui.text(atlas, sx + 34.0 + c, 26.0, text, DIM);
    }
    let mut lane_iter = board.lanes.iter();
    for (i, backend) in hud.backends.iter().enumerate() {
        let y = 26.0 + (i as f32 + 1.0) * line + 4.0;
        let x = sx + 34.0;
        let on = hud.enabled[i];
        ui.text(atlas, sx + 12.0, y, &format!("{}", i + 1), DIM);
        ui.rect(
            atlas,
            sx + 24.0,
            y + 3.0,
            6.0,
            line - 6.0,
            rgba(backend.color, if on { 1.0 } else { 0.25 }),
        );
        let name_color = if on { INK } else { DIM };
        ui.text(atlas, x, y, backend.id, name_color);
        if !on {
            ui.text(atlas, x + cols[1], y, "off (press the number)", DIM);
            continue;
        }
        let Some(lane) = lane_iter.next() else {
            continue;
        };
        if let LaneStatus::Offline(why) = &lane.status {
            ui.text(
                atlas,
                x + cols[1],
                y,
                &fit(
                    atlas,
                    &format!("offline: {}", ascii(why)),
                    sw - 34.0 - cols[1] - 8.0,
                ),
                DIM,
            );
            continue;
        }
        let n = case.candidates.len();
        let done = format!("{}/{}", lane.answered(), n);
        let rate = lane.rate().map_or("-".into(), |r| format!("{r:.2}"));
        let p50 = lane.p50().map_or("-".into(), secs);
        let q = lane.quality(&board.labels);
        let pct = |v: Option<f64>| v.map_or("-".into(), |v| format!("{:.0}%", v * 100.0));
        let cells = [
            done,
            rate,
            p50,
            pct(q.and_then(|q| q.precision)),
            pct(q.and_then(|q| q.recall)),
            pct(q.map(|q| q.accuracy)),
        ];
        for (c, text) in cols[1..].iter().zip(cells) {
            ui.text(atlas, x + c, y, &text, INK);
        }
        if lane.errors > 0 {
            let e = format!("{} err", lane.errors);
            ui.text(
                atlas,
                sx + sw - atlas.measure(&e) - 8.0,
                y,
                &e,
                rgba(AMBER, 1.0),
            );
        }
    }

    // The ranking, bottom right.
    let ranked = ranking(&board.lanes, case.candidates.len());
    let shown = ranked.iter().filter(|r| r.mean.is_some()).take(10).count();
    if shown > 0 {
        let rh = 20.0 + (shown as f32 + 1.0) * line;
        let ry = h - rh - 56.0;
        panel(&mut ui, atlas, sx, ry, sw, rh);
        // Columns for the lanes that are running; an offline lane has none.
        let live: Vec<usize> = (0..board.lanes.len())
            .filter(|k| !matches!(board.lanes[*k].status, LaneStatus::Offline(_)))
            .collect();
        let pw_lane = ch * 5.0;
        let pcol = sw - 12.0 - live.len() as f32 * pw_lane;
        ui.text(atlas, sx + 12.0, ry + 10.0, "RANKED BY MEAN P", DIM);
        for (j, k) in live.iter().enumerate() {
            let color = board.lanes[*k].backend.color;
            ui.rect(
                atlas,
                sx + pcol + j as f32 * pw_lane,
                ry + 10.0 + line * 0.45,
                ch * 4.0,
                3.0,
                rgba(color, 1.0),
            );
        }
        let path_x = sx + 12.0 + ch * 15.0;
        for (i, row) in ranked
            .iter()
            .filter(|r| r.mean.is_some())
            .take(shown)
            .enumerate()
        {
            let y = ry + 10.0 + (i as f32 + 1.0) * line;
            let c = &case.candidates[row.file];
            let mean = row.mean.unwrap_or(0.0);
            let mark = match (board.labels[row.file], row.disagree) {
                (Some(true), _) => ("fix", rgba(LAUREL, 1.0).map(|v| (v * 3.0).min(1.0))),
                (_, true) => ("split", rgba(SPLIT, 1.0)),
                _ => ("", DIM),
            };
            let ink = if mean >= THRESHOLD { INK } else { DIM };
            ui.text(
                atlas,
                sx + 12.0,
                y,
                &format!("{:>2} {:.2}", i + 1, mean),
                ink,
            );
            ui.text(atlas, sx + 12.0 + ch * 9.0, y, mark.0, mark.1);
            ui.text(
                atlas,
                path_x,
                y,
                &fit(atlas, &c.path, sx + pcol - path_x - ch),
                ink,
            );
            for (j, k) in live.iter().enumerate() {
                let text = row.p[*k].map_or("  -".into(), |p| {
                    format!("{p:.2}").trim_start_matches('0').to_owned()
                });
                let color = board.lanes[*k].backend.color;
                let lift = color.map(|v| (v * 1.5 + 0.1).min(1.0));
                ui.text(
                    atlas,
                    sx + pcol + j as f32 * pw_lane,
                    y,
                    &text,
                    rgba(lift, 1.0),
                );
            }
        }
    }

    // Progress and agreement, bottom left.
    let total: usize = board
        .lanes
        .iter()
        .filter(|l| !matches!(l.status, LaneStatus::Offline(_)))
        .count()
        * case.candidates.len();
    let got: usize = board.lanes.iter().map(|l| l.answered()).sum();
    let all = ranked
        .iter()
        .filter(|r| r.p.iter().all(|p| p.is_some()))
        .count();
    let split = ranked.iter().filter(|r| r.disagree).count();
    let state = if board.finished() { "done" } else { "running" };
    let status = format!(
        "{state}: {got}/{total} decisions in {:.1}s; lanes split on {split} of {} files",
        hud.elapsed,
        case.candidates.len(),
    );
    let _ = all;
    let legend = [
        "bar = one lane's P(relevant), lit past 0.5; beam = mean P >= 0.5",
        "green square = changed by the fix; red outline = the lanes split",
    ];
    let bw = legend
        .iter()
        .map(|l| atlas.measure(l))
        .fold(atlas.measure(&status), f32::max)
        + 32.0;
    panel(
        &mut ui,
        atlas,
        16.0,
        h - 30.0 - line * 4.0 - 10.0,
        bw.min(sx - 32.0),
        line * 5.0 + 14.0,
    );
    ui.text(
        atlas,
        24.0,
        h - 30.0 - line * 4.0,
        &fit(atlas, &status, sx - 56.0),
        INK,
    );
    for (i, l) in legend.iter().enumerate() {
        ui.text(atlas, 24.0, h - 30.0 - line * (2.8 - i as f32), l, DIM);
    }
    ui.text(atlas, 24.0, h - 30.0, keys, DIM);
    ui
}

/// Renders one frame off screen and writes it as a PNG.
#[allow(clippy::too_many_arguments)]
pub fn capture(
    renderer: &mut verse::imported::Renderer,
    view: verse::render::View,
    world: &[Instance],
    ui: &UiBatch,
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> Result<(), String> {
    let pixels = renderer.draw(view, world, ui, &lighting())?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_glow_past_the_threshold_and_flare_on_arrival() {
        assert!(bar_level(0.2, 5.0) < bar_level(0.6, 5.0));
        assert!(bar_level(0.49, 5.0) <= 3);
        assert!(bar_level(0.5, 5.0) >= 5);
        assert!(bar_level(0.6, 0.0) > bar_level(0.6, 5.0));
        assert!(bar_level(1.0, 0.0) < LEVELS);
    }

    #[test]
    fn the_pack_validates_and_has_every_lane() {
        let dir = tempfile::tempdir().unwrap();
        let pack = pack(dir.path()).unwrap();
        for i in 0..registry().len() {
            assert!(pack.models.contains_key(&format!("bar/{i}/{}", LEVELS - 1)));
        }
        for i in static_instances() {
            assert!(pack.models.contains_key(&i.model), "{}", i.model);
        }
    }

    #[test]
    fn look_along_turns_z_onto_the_direction() {
        let d = Vec3::new(3., -1., 2.);
        let m = look_along(d);
        let z = m.transform_vector3(Vec3::Z);
        assert!((z - d.normalize()).length() < 1e-5);
    }
}
