//! Fixed water views through both Verse renderers at every quality tier
//! (`docs/verse/water.md`, phases W2, W4, and W5).
//!
//! Usage: water_capture OUTPUT_DIRECTORY [--baseline DIRECTORY]...
//!
//! Captures stay out of git (#11110): after a run under `bench/`, upload
//! the PNGs with `scripts/bench-artifacts.py push OUTPUT_DIRECTORY`, which
//! leaves a sha256 manifest beside the JSON. A committed baseline's PNGs
//! come back with `scripts/bench-artifacts.py restore DIRECTORY`.
//!
//! Phase W7 adds `waterline` (the eye at the pond's surface, the view
//! split per pixel), `snell` (the surface from below: Snell's window), and
//! `posts-under` (caustics on the posts and the bed, from under the
//! water), and `shafts` (the sun's shafts, looking toward it from under
//! the water); its captures change every pond picture, whose beds now take
//! caustics on every tier.
//!
//! The physical renderer draws each view three ways: without water (into
//! `dry/`), with the water every tier drew before W5 (its two halves over
//! the scene, into `w2/`, Medium and High only), and with this tier's
//! water (W5: refraction, the planar mirror, and on High screen-space
//! reflection over the scene copies). Each is also timed at 1920 by 1080,
//! the fastest of several batches of frames, and `validation.json` records
//! what the copies add per tier in time and memory against
//! `docs/verse/water.md`'s budgets. With `--baseline`, each Low picture is
//! compared with the same picture in the first earlier capture that has
//! it, which it must match.
//!
//! Renders six views (a pond at noon and at dusk, a river around two rocks,
//! a sea rolling onto a beach toward a low sun, the pond from under its
//! surface, and the still pond with posts standing in it, phase W5), and
//! the spectral sea in its calm, moderate, and storm states (`sea-calm`,
//! `sea-moderate`, `sea-storm`, and the storm's surf, `sea-storm-surf`,
//! phase W4), the same three over open water 30 m deep (`open-calm`,
//! `open-moderate`, and `open-storm`, #10918), through the physical renderer and the imported renderer at
//! Low, Medium, and High, each with and without its water (the latter into
//! `dry/`, for the comparison only), and writes the PNGs,
//! `capture.json` (one record a picture, with its digest and frame times),
//! and `validation.json` (what each picture shows was checked). Water bodies
//! are `physics::water` bodies baked by `verse_pbr::water::bake` at each
//! tier's spacing, with the built-in presets. The imported renderer reads
//! its tier from `VERSE_QUALITY` when it starts, so this program runs
//! itself once a tier for it.

use std::f64::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use glam::{DVec2, Mat4, Vec2, Vec3};
use physics::water::{
    Course, FlowGrid, Kind as BodyKind, Level, Obstacle, Outline, WaterBody, WaterId,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse_engine::{presentation::View, quality::Tier};
use verse_pbr::{
    pbr::{
        Daylight, Key, LitVertex, Material, Neon,
        gpu::{Batches, Capability, Photo, Stage},
    },
    water::{
        Body, Kind, Preset, Sky, Swell, Water, WaterSurface,
        bake::{self, Bake},
        frame::wind_sea,
        under::EyeSurface,
    },
};
use wgpu::util::DeviceExt;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
/// Frames timed a view, after one to warm up (the imported renderer).
const TIMED: usize = 12;
/// Rounds of [`BATCH`] frames timed a physical view at the budget size,
/// after one to warm up; the fastest stands.
const ROUNDS: usize = 7;
const BATCH: usize = 10;
/// The size the budgets in `docs/verse/water.md` are stated for.
const BUDGET_SIZE: [u32; 2] = [1920, 1080];
const TIERS: [Tier; 3] = [Tier::Low, Tier::Medium, Tier::High];

/// The water clock every view shows, s.
const TIME: f32 = 41.25;

#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Pond,
    River,
    Beach,
    /// A beach under a spectral sea in the named state
    /// (`assets/verse/water/seas/`).
    Sea(&'static str),
    /// Open water 30 m deep with no shore in view, under a spectral sea in
    /// the named state, so the sea's own waves and whitecaps show without
    /// shoaling or surf (#10918).
    Open(&'static str),
}

/// One fixed view.
struct ViewSpec {
    name: &'static str,
    eye: Vec3,
    target: Vec3,
    ground: Ground,
    /// Toward the sun.
    sun: Vec3,
    dusk: bool,
    /// The eye is in the water.
    under: bool,
    /// Posts stand in and around the pond (W5): for refraction, the mirror,
    /// and contact foam.
    posts: bool,
    /// The water has no swell, so the mirror takes it.
    still: bool,
    /// The eye and its target are heights over the surface at the eye
    /// (W7): the near plane straddles the surface, splitting the view.
    waterline: bool,
}

/// The views, or those `WATER_CAPTURE_VIEWS` (a comma-separated list) names.
fn views() -> Vec<ViewSpec> {
    let only = std::env::var("WATER_CAPTURE_VIEWS").ok();
    let mut all: Vec<ViewSpec> = all_views().into_iter().map(placed).collect();
    if let Some(only) = only {
        all.retain(|v| only.split(',').any(|name| name == v.name));
    }
    all
}

fn all_views() -> Vec<ViewSpec> {
    vec![
        ViewSpec {
            name: "pond-noon",
            eye: Vec3::new(-10.0, 5.5, 10.0),
            target: Vec3::new(0.5, -0.8, -0.5),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "pond-dusk",
            eye: Vec3::new(0.0, 2.0, 13.0),
            target: Vec3::new(0.0, -0.2, -4.0),
            ground: Ground::Pond,
            sun: Vec3::new(0.08, 0.13, -1.0).normalize(),
            dusk: true,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "river",
            eye: Vec3::new(4.0, 3.6, 10.5),
            target: Vec3::new(-1.0, -0.6, 0.5),
            ground: Ground::River,
            sun: Vec3::new(-0.4, 0.75, -0.5).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "sea",
            eye: Vec3::new(0.0, 3.4, 30.0),
            target: Vec3::new(0.0, 0.0, -30.0),
            ground: Ground::Beach,
            sun: Vec3::new(0.12, 0.2, -1.0).normalize(),
            dusk: true,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "sea-calm",
            eye: Vec3::new(0.0, 5.0, 31.0),
            target: Vec3::new(0.0, -0.5, -30.0),
            ground: Ground::Sea("calm"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "sea-moderate",
            eye: Vec3::new(0.0, 5.0, 31.0),
            target: Vec3::new(0.0, -0.5, -30.0),
            ground: Ground::Sea("moderate"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "sea-storm",
            eye: Vec3::new(0.0, 5.0, 31.0),
            target: Vec3::new(0.0, -0.5, -30.0),
            ground: Ground::Sea("storm"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "open-calm",
            eye: Vec3::new(0.0, 12.0, 95.0),
            target: Vec3::new(0.0, -2.0, -60.0),
            ground: Ground::Open("calm"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "open-moderate",
            eye: Vec3::new(0.0, 12.0, 95.0),
            target: Vec3::new(0.0, -2.0, -60.0),
            ground: Ground::Open("moderate"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "open-storm",
            eye: Vec3::new(0.0, 12.0, 95.0),
            target: Vec3::new(0.0, -2.0, -60.0),
            ground: Ground::Open("storm"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "sea-storm-surf",
            eye: Vec3::new(-10.0, 9.0, 36.0),
            target: Vec3::new(4.0, -1.0, 6.0),
            ground: Ground::Sea("storm"),
            sun: Vec3::new(0.35, 0.42, -0.84).normalize(),
            dusk: false,
            under: false,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "under",
            eye: Vec3::new(0.0, -1.3, 4.0),
            target: Vec3::new(0.0, 1.2, -5.0),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: true,
            posts: false,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "pond-posts",
            eye: Vec3::new(-9.5, 2.6, 9.0),
            target: Vec3::new(1.5, -0.6, -1.5),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: false,
            posts: true,
            still: true,
            waterline: false,
        },
        // W7: the eye at the surface, the view split at the waterline; the
        // surface from below in Snell's window; and the posts and the bed
        // under the water with their caustics.
        ViewSpec {
            name: "waterline",
            eye: Vec3::new(-5.0, 0.0, 5.5),
            target: Vec3::new(2.0, -0.02, -1.5),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: false,
            posts: true,
            still: false,
            waterline: true,
        },
        ViewSpec {
            name: "snell",
            eye: Vec3::new(0.5, -1.7, 2.0),
            target: Vec3::new(0.2, 2.6, -1.2),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: true,
            posts: true,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "shafts",
            eye: Vec3::new(-2.5, -1.0, 4.5),
            target: Vec3::new(-0.6, 1.2, -1.0),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: true,
            posts: true,
            still: false,
            waterline: false,
        },
        ViewSpec {
            name: "posts-under",
            eye: Vec3::new(5.0, -1.1, -4.0),
            target: Vec3::new(1.5, -1.5, -1.0),
            ground: Ground::Pond,
            sun: Vec3::new(0.35, 0.86, -0.38).normalize(),
            dusk: false,
            under: true,
            posts: true,
            still: false,
            waterline: false,
        },
    ]
}

/// `spec` with a waterline view's eye and target raised by the surface's
/// height over the eye, so the eye stands exactly at the surface.
fn placed(mut spec: ViewSpec) -> ViewSpec {
    if spec.waterline {
        let world = world(spec.ground);
        let water = water(&world, &spec);
        let h = water.bodies[0].level
            + water.bodies[0]
                .displacement(Vec2::new(spec.eye.x, spec.eye.z), 10.0, water.time)
                .y;
        spec.eye.y += h;
        spec.target.y += h;
        spec.waterline = false;
    }
    spec
}

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Low => "low",
        Tier::Medium => "medium",
        Tier::High => "high",
    }
}

fn camera(spec: &ViewSpec) -> View {
    View {
        eye: spec.eye,
        view_proj: Mat4::perspective_rh(0.9, WIDTH as f32 / HEIGHT as f32, 0.1, 400.0)
            * Mat4::look_at_rh(spec.eye, spec.target, Vec3::Y),
    }
}

// ---- The worlds.

/// The pond: a round basin 9 m across at level 0, 2.5 m deep in the middle.
fn pond() -> WaterBody {
    let ring = (0..40)
        .map(|i| {
            let a = i as f64 / 40.0 * TAU;
            DVec2::new(a.cos(), a.sin()) * (9.0 + 0.6 * (3.0 * a).sin())
        })
        .collect();
    WaterBody::pond(WaterId(0), ring, 0.0).with_waves(wind_sea(0.7, 2.6, 0.45, 4))
}

fn pond_bed(x: f64, z: f64) -> f64 {
    let a = z.atan2(x);
    let r = x.hypot(z) / (9.0 + 0.6 * (3.0 * a).sin());
    if r < 1.0 {
        -2.5 * (1.0 - r * r).powf(0.7) + 0.05
    } else {
        (0.05 + 0.5 * (r - 1.0) * 9.0).min(0.45)
    }
}

fn river_course() -> Course {
    Course::spline(
        &[
            DVec2::new(-22.0, -6.0),
            DVec2::new(-8.0, -2.0),
            DVec2::new(2.0, 1.5),
            DVec2::new(14.0, -1.0),
            DVec2::new(26.0, 3.0),
        ],
        &[5.0, 5.5, 6.0, 5.0, 5.5],
        8,
    )
}

const ROCKS: [(f64, f64, f64); 2] = [(-1.5, 0.6, 0.7), (5.5, -0.2, 0.55)];

fn river() -> WaterBody {
    let course = river_course();
    let obstacles: Vec<Obstacle> = ROCKS
        .iter()
        .map(|&(x, z, r)| Obstacle {
            center: DVec2::new(x, z),
            radius: r,
        })
        .collect();
    let flow = FlowGrid::river(&course, &obstacles, 0.5, 1.1);
    let length = course.length();
    WaterBody::new(
        WaterId(0),
        BodyKind::River,
        Outline::River { course },
        Level::Profile {
            points: vec![[0.0, 0.25], [length, -0.25]],
        },
    )
    .with_flow(flow)
}

fn river_bed(x: f64, z: f64) -> f64 {
    let course = river_course();
    let s = course.locate(DVec2::new(x, z));
    let level = 0.25 - 0.5 * s.along / course.length();
    let t = (s.offset.abs() / (s.half_width + 0.8)).min(1.4);
    let channel = level - 0.9 * (1.0 - t * t).max(0.0) - 0.05;
    let bank = level + 0.15 + 0.9 * (t - 1.0).max(0.0);
    let mut h = if t < 1.0 { channel } else { bank.max(channel) };
    for &(rx, rz, r) in &ROCKS {
        let d = (x - rx).hypot(z - rz);
        if d < r {
            h = h.max(level + 0.35 * (1.0 - (d / r).powi(2)).sqrt() + 0.05);
        }
    }
    h
}

fn sea() -> WaterBody {
    WaterBody::ocean(WaterId(0), 0.0).with_waves(wind_sea(std::f32::consts::PI, 16.0, 1.3, 8))
}

/// An ocean whose waves are sea state `name`'s spectrum, rolling toward
/// the beach (+z) over water 30 m deep offshore.
fn spectral_sea(name: &str) -> WaterBody {
    let state = verse_pbr::water::SeaState::named(name).expect("a built-in sea state");
    let waves = physics::water::WaveSet::calm()
        .with_spectrum(state.spectrum(0.3, 0x5EA, 30.0))
        .expect("a valid spectrum");
    WaterBody::ocean(WaterId(0), 0.0).with_waves(waves)
}

/// A flat bed 30 m down, the spectral sea's depth.
fn open_bed(_x: f64, _z: f64) -> f64 {
    -30.0
}

fn beach_bed(x: f64, z: f64) -> f64 {
    // Sand rising toward the camera from a shelf 6 m down.
    let ripple = 0.08 * (x * 0.7).sin() * (z * 0.4).cos();
    (-6.0 + 0.24 * (z + 4.0)).clamp(-6.0, 2.5) + ripple
}

struct World {
    body: WaterBody,
    preset: &'static str,
    kind: Kind,
    bed: fn(f64, f64) -> f64,
    extent: Option<(Vec2, Vec2)>,
    /// The bed's square, m.
    bounds: (Vec2, Vec2),
}

fn world(ground: Ground) -> World {
    match ground {
        Ground::Pond => World {
            body: pond(),
            preset: "pond",
            kind: Kind::Body(1.0),
            bed: pond_bed,
            extent: None,
            bounds: (Vec2::splat(-24.0), Vec2::splat(24.0)),
        },
        Ground::River => World {
            body: river(),
            preset: "river",
            kind: Kind::Stream,
            bed: river_bed,
            extent: None,
            bounds: (Vec2::new(-24.0, -16.0), Vec2::new(24.0, 18.0)),
        },
        Ground::Beach => World {
            body: sea(),
            preset: "ocean",
            kind: Kind::Body(1.0),
            bed: beach_bed,
            extent: Some((Vec2::new(-90.0, -150.0), Vec2::new(90.0, 22.0))),
            bounds: (Vec2::new(-90.0, -150.0), Vec2::new(90.0, 40.0)),
        },
        Ground::Sea(name) => World {
            body: spectral_sea(name),
            preset: "ocean",
            kind: Kind::Body(1.0),
            bed: beach_bed,
            extent: Some((Vec2::new(-90.0, -150.0), Vec2::new(90.0, 22.0))),
            bounds: (Vec2::new(-90.0, -150.0), Vec2::new(90.0, 40.0)),
        },
        Ground::Open(name) => World {
            body: spectral_sea(name),
            preset: "ocean",
            kind: Kind::Body(1.0),
            bed: open_bed,
            extent: Some((Vec2::new(-130.0, -170.0), Vec2::new(130.0, 120.0))),
            bounds: (Vec2::new(-130.0, -170.0), Vec2::new(130.0, 120.0)),
        },
    }
}

fn surface(world: &World, tier: Tier) -> Result<WaterSurface, String> {
    let patch = bake::body(
        &world.body,
        &world.bed,
        &Bake {
            spacing: bake::spacing(tier),
            pad: 1.0,
            kind: world.kind,
            body: 0,
            extent: world.extent,
        },
    )?;
    Ok(WaterSurface {
        patches: vec![patch],
        ocean: None,
    })
}

/// The frame's water for `world` seen from `spec`, with a sky and sun for
/// the imported renderer in its linear units.
fn water(world: &World, spec: &ViewSpec) -> Water {
    let preset = Preset::named(world.preset).cloned().unwrap_or_default();
    let mut water = Water::calm(0.0);
    water.bodies[0] = Body {
        eye_inside: spec.under,
        ..Body::from_physics(&world.body, &preset)
    };
    if spec.still {
        water.bodies[0].swell = Swell::default();
    }
    water.time = TIME;
    // The surface over the eye, as a zone's surface query gives it (W7):
    // the swell's height there and its slope, by central differences.
    let at = |x: f32, z: f32| {
        water.bodies[0].level + water.bodies[0].displacement(Vec2::new(x, z), 10.0, TIME).y
    };
    let (x, z, e) = (spec.eye.x, spec.eye.z, 0.25);
    if x.hypot(z) < 9.0 && world.preset == "pond" {
        water.eye = Some(EyeSurface {
            body: 0,
            height: at(x, z),
            slope: [
                (at(x + e, z) - at(x - e, z)) / (2.0 * e),
                (at(x, z + e) - at(x, z - e)) / (2.0 * e),
            ],
        });
    }
    let (zenith, horizon, color) = palette(spec.dusk);
    water.sky = Some(Sky {
        zenith: zenith.map(|c| c * 0.9),
        horizon: horizon.map(|c| c * 0.9),
        sun_dir: spec.sun.to_array(),
        sun_illuminance: if spec.dusk { 2.4 } else { 3.2 },
        sun_color: color,
    });
    water
}

fn palette(dusk: bool) -> ([f32; 3], [f32; 3], [f32; 3]) {
    if dusk {
        ([0.07, 0.15, 0.40], [0.80, 0.60, 0.44], [1.0, 0.62, 0.34])
    } else {
        ([0.10, 0.30, 0.74], [0.66, 0.76, 0.84], [1.0, 0.95, 0.86])
    }
}

/// Grass, sand, and painted wood: what a triangle of the bed is.
const GRASS: u8 = 0;
const SAND: u8 = 1;
const POST: u8 = 2;

/// Where the posts stand (x, z), and their half width, m.
const POSTS: [(f32, f32, f32); 5] = [
    (1.5, -1.0, 0.3),
    (-2.5, 2.0, 0.25),
    (4.0, 3.0, 0.35),
    (-0.5, -5.5, 0.3),
    (-6.0, -2.0, 0.3),
];

/// A box from `bottom` to `top` around (x, z), its sides and top.
fn post(out: &mut Vec<(Vec3, Vec3, u8)>, x: f32, z: f32, s: f32, bottom: f32, top: f32) {
    let corners: Vec<Vec3> = (0..8)
        .map(|i| {
            Vec3::new(
                x + if i & 1 == 0 { -s } else { s },
                if i & 2 == 0 { bottom } else { top },
                z + if i & 4 == 0 { -s } else { s },
            )
        })
        .collect();
    for (a, b, c, d, n) in [
        (0, 1, 3, 2, -Vec3::Z),
        (4, 6, 7, 5, Vec3::Z),
        (0, 2, 6, 4, -Vec3::X),
        (1, 5, 7, 3, Vec3::X),
        (2, 3, 7, 6, Vec3::Y),
    ] {
        for i in [a, b, c, a, c, d] {
            out.push((corners[i], n, POST));
        }
    }
}

/// The bed as triangles with smooth normals: a grid of `step` m, sand under
/// and near the water, grass above it, and the posts when `posts`.
fn bed_triangles(world: &World, step: f32, posts: bool) -> Vec<(Vec3, Vec3, u8)> {
    let (lo, hi) = world.bounds;
    let cols = ((hi.x - lo.x) / step) as usize + 1;
    let rows = ((hi.y - lo.y) / step) as usize + 1;
    let h = |x: f32, z: f32| (world.bed)(f64::from(x), f64::from(z)) as f32;
    let at = |c: usize, r: usize| {
        let x = lo.x + c as f32 * step;
        let z = lo.y + r as f32 * step;
        let p = Vec3::new(x, h(x, z), z);
        let e = 0.2;
        let n = Vec3::new(
            h(x - e, z) - h(x + e, z),
            2.0 * e,
            h(x, z - e) - h(x, z + e),
        )
        .normalize();
        (p, n)
    };
    let mut out = Vec::new();
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            let quad = [at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1)];
            let sand = if quad.iter().any(|(p, _)| p.y < 0.6) {
                SAND
            } else {
                GRASS
            };
            for i in [0, 2, 1, 0, 3, 2] {
                out.push((quad[i].0, quad[i].1, sand));
            }
        }
    }
    // The river's rocks stand out of the water.
    if world.preset == "river" {
        for &(x, z, r) in &ROCKS {
            let y = h(x as f32, z as f32);
            let s = r as f32;
            let centre = Vec3::new(x as f32, y - 0.2, z as f32);
            let corners: Vec<Vec3> = (0..8)
                .map(|i| {
                    centre
                        + Vec3::new(
                            if i & 1 == 0 { -s } else { s },
                            if i & 2 == 0 { -0.2 } else { 0.55 },
                            if i & 4 == 0 { -s } else { s },
                        )
                })
                .collect();
            for (a, b, c, d, n) in [
                (0, 1, 3, 2, -Vec3::Z),
                (4, 6, 7, 5, Vec3::Z),
                (0, 2, 6, 4, -Vec3::X),
                (1, 5, 7, 3, Vec3::X),
                (2, 3, 7, 6, Vec3::Y),
            ] {
                for i in [a, b, c, a, c, d] {
                    out.push((corners[i], n, GRASS));
                }
            }
        }
    }
    if posts {
        for &(x, z, s) in &POSTS {
            let bottom = h(x, z) - 0.3;
            post(&mut out, x, z, s, bottom, 1.4 + 0.3 * (x * 1.7).sin().abs());
        }
    }
    out
}

fn lit(triangles: &[(Vec3, Vec3, u8)]) -> Vec<LitVertex> {
    triangles
        .iter()
        .map(|&(p, n, kind)| LitVertex {
            pos: p.to_array(),
            normal: n.to_array(),
            tangent: n.any_orthonormal_vector().to_array(),
            local: p.to_array(),
            color: match kind {
                SAND => [0.62, 0.55, 0.42],
                POST => [0.62, 0.16, 0.10],
                _ => [0.22, 0.34, 0.14],
            },
            params: [0.0, 0.9, Material::Stage.code(), 1.0],
        })
        .collect()
}

// ---- The physical renderer.

fn stage(spec: &ViewSpec, water: Option<Water>) -> Neon {
    let (zenith, horizon, color) = palette(spec.dusk);
    let illuminance = if spec.dusk { 2_800.0 } else { 5_200.0 };
    let sky = if spec.dusk { 520.0 } else { 1_500.0 };
    Neon {
        field: horizon,
        fog_start: 80.0,
        fog_end: 380.0,
        line_gain: 1.0,
        line_width: 1.4,
        bloom: 0.06,
        vignette: 0.25,
        time: TIME,
        key: Some(Key {
            dir: spec.sun,
            illuminance,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.5, 0.45, 0.7).normalize(),
            rim_illuminance: illuminance * 0.15,
            rim_angular_radius: 0.2,
            sky,
            ground: sky * 0.35,
            ev100: if spec.dusk { 10.0 } else { 11.0 },
            shadow_center: Vec3::ZERO,
            shadow_half: 60.0,
            shadow_distance: Some(120.0),
            cache_far_shadows: false,
        }),
        daylight: Some(Daylight {
            zenith,
            horizon,
            sun: color,
            clouds: 0.25,
            ground: [0.3, 0.27, 0.2],
            glow: if spec.dusk { 0.55 } else { 0.0 },
        }),
        key_color: color,
        rim_color: [0.55, 0.65, 1.0],
        water,
        ..Neon::plaza(TIME)
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
}

fn gpu() -> Result<Gpu, String> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let info = adapter.get_info();
    Ok(Gpu {
        device,
        queue,
        adapter: format!("{} ({:?})", info.name, info.backend),
    })
}

/// The physical renderer's capability at `tier`: Low has no floating-point
/// target and one sample, as on WebGL2 and OpenGL ES.
fn capability(tier: Tier) -> Capability {
    Capability {
        hdr: (tier != Tier::Low).then_some(wgpu::TextureFormat::Rgba16Float),
        samples: if tier == Tier::Low { 1 } else { 4 },
        gles: false,
        quality: tier.quality(),
    }
}

/// How the physical renderer draws a view's water.
#[derive(Clone, Copy, PartialEq)]
enum Wet {
    /// No water.
    Dry,
    /// The two halves every tier drew before W5.
    Halves,
    /// This tier's water: on Medium and High, over the scene copies.
    Tier,
}

/// A view through the physical renderer at `size`: its pixels (at the
/// capture size only), its frame time (the fastest of [`ROUNDS`]
/// batches), the water's index count, and the bytes its scene copies and
/// surface hold.
fn physical(
    gpu: &Gpu,
    photo: &mut Photo,
    tier: Tier,
    spec: &ViewSpec,
    wet: Wet,
    size: [u32; 2],
) -> Result<(Vec<u8>, f64, u32, u64), String> {
    physical_sampled(gpu, photo, tier, spec, wet, size, None)
}

#[allow(clippy::too_many_arguments)]
fn physical_sampled(
    gpu: &Gpu,
    photo: &mut Photo,
    tier: Tier,
    spec: &ViewSpec,
    wet: Wet,
    size: [u32; 2],
    mut measurements: Option<&mut Vec<verse_pbr::water::timing::Measurements>>,
) -> Result<(Vec<u8>, f64, u32, u64), String> {
    let world = world(spec.ground);
    let mut surface = surface(&world, tier)?;
    if measurements.is_some() && matches!(spec.ground, Ground::Open(_)) {
        surface.patches.clear();
        surface.ocean = Some(verse_pbr::water::frame::Ocean {
            body: 0,
            sea: true,
            field: None,
        });
    }
    let water_gpu = photo.upload_water(&gpu.device, &surface);
    let geometry = lit(&bed_triangles(&world, 0.5, spec.posts));
    let buffer = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water capture bed"),
            contents: bytemuck::cast_slice(&geometry),
            usage: wgpu::BufferUsages::VERTEX,
        });
    let with_water = wet != Wet::Dry;
    photo.water_copies = wet == Wet::Tier;
    let mut neon = stage(spec, with_water.then(|| water(&world, spec)));
    let [width, height] = size;
    let mut targets = photo.targets(&gpu.device, width, height);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water capture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output = texture.create_view(&Default::default());
    let capture = size == [WIDTH, HEIGHT];
    let mut pixels = Vec::new();
    // Every frame shows the spectral sea at exactly the clock's tick.
    let sampled = measurements.is_some();
    photo.ocean.exact = !sampled || capture;
    if sampled {
        photo.enable_water_timing(&gpu.device, &gpu.queue);
    }
    let mut times = Vec::new();
    let mut view = camera(spec);
    view.view_proj = Mat4::perspective_rh(0.9, width as f32 / height as f32, 0.1, 400.0)
        * Mat4::look_at_rh(spec.eye, spec.target, Vec3::Y);
    let batches = &Batches {
        streamed: None,
        lit: (&buffer, geometry.len() as u32),
        faces: [(&buffer, 0); 2],
        lines: [(&buffer, 0); 2],
        textured: None,
        figure: None,
        water: with_water.then_some(&water_gpu),
    };
    let mut frame = |photo: &mut Photo| {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        if sampled && let Some(water) = &mut neon.water {
            water.time += 1.0 / 60.0;
        }
        photo.encode(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &output,
            &mut targets,
            view,
            Stage::Neon(&neon),
            Batches { ..*batches },
            None,
        );
        if let Some(records) = measurements.as_mut() {
            records.push(photo.water_measurements());
        }
        encoder
    };
    let wait = || {
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(20)),
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    if capture {
        let warm = frame(photo);
        gpu.queue.submit([warm.finish()]);
        photo.submitted();
        wait()?;
        pixels = read(gpu, &texture, frame(photo))?;
        photo.submitted();
    } else if sampled {
        // Use actual 60 Hz frame intervals so a busy worker cannot appear
        // cheaper merely because display frames were submitted in bursts.
        // Keep the queue fence separate and exclude the cadence sleep.
        let interval = std::time::Duration::from_secs_f64(1.0 / 60.0);
        for index in 0..360 {
            let frame_started = Instant::now();
            let encoder = frame(photo);
            let submitted = Instant::now();
            gpu.queue.submit([encoder.finish()]);
            photo.submitted();
            wait()?;
            if index >= 120 {
                times.push(submitted.elapsed().as_secs_f64() * 1e3);
            }
            std::thread::sleep(interval.saturating_sub(frame_started.elapsed()));
        }
        // Read the frame that supplied the final measurement. A smaller
        // viewport could admit optics that the measured viewport dropped.
        pixels = read(
            gpu,
            &texture,
            gpu.device.create_command_encoder(&Default::default()),
        )?;
    } else {
        // The retained capture bench measures throughput, separately from
        // W11's paced completed-job measurements.
        for round in 0..=ROUNDS {
            let started = Instant::now();
            for _ in 0..BATCH {
                let encoder = frame(photo);
                gpu.queue.submit([encoder.finish()]);
                photo.submitted();
            }
            wait()?;
            if round > 0 {
                times.push(started.elapsed().as_secs_f64() * 1e3 / BATCH as f64);
            }
        }
    }
    photo.water_copies = true;
    // The fastest round: the one other work on a shared machine disturbed
    // least.
    let fastest = times.iter().copied().fold(f64::INFINITY, f64::min);
    let fastest = if fastest.is_finite() { fastest } else { 0.0 };
    Ok((
        pixels,
        fastest,
        water_gpu.0.count(),
        photo.water_bytes(&targets, water_gpu.bytes()),
    ))
}

fn read(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    mut encoder: wgpu::CommandEncoder,
) -> Result<Vec<u8>, String> {
    let (width, height) = (texture.width(), texture.height());
    let row = (width * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(20)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(20))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let bytes = readback.slice(..).get_mapped_range();
    Ok(bytes
        .chunks(row as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect())
}

// ---- The imported renderer, in a process of its own per tier.

fn imported(directory: &Path, tier: Tier) -> Result<Vec<Value>, String> {
    use verse_engine::{
        assets::{Surface, Topology, Vertex},
        lighting::{Light, Lighting},
        presentation::Instance,
    };
    use verse_pbr::{
        imported::{Renderer, flat},
        ui::{Atlas, UiBatch},
    };
    let mut records = Vec::new();
    for spec in views() {
        let world = world(spec.ground);
        let assets = tempfile::tempdir().map_err(|e| e.to_string())?;
        let mut pack = verse_content::compiler::original::generate(assets.path())?;
        let texture = flat::white_texture(&mut pack, assets.path())?;
        let triangles = bed_triangles(&world, 0.5, spec.posts);
        let mut sand = flat::surface(texture, [0.62, 0.55, 0.42], Topology::Triangles);
        let mut grass = flat::surface(texture, [0.22, 0.34, 0.14], Topology::Triangles);
        for s in [&mut sand, &mut grass] {
            s.unlit = false;
        }
        let push = |s: &mut Surface, p: Vec3, n: Vec3| {
            s.indices.push(s.vertices.len() as u32);
            s.vertices.push(Vertex {
                position: p.to_array(),
                normal: n.to_array(),
                uv: [0.5, 0.5],
                joints: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            });
        };
        for &(p, n, kind) in &triangles {
            push(if kind == SAND { &mut sand } else { &mut grass }, p, n);
        }
        pack.models.insert(
            "water-bed".into(),
            flat::model("verse/fixture/water-bed", vec![sand, grass], 1.0),
        );
        let bed = Instance {
            model: "water-bed".into(),
            actor: None,
            mount: None,
            transform: Mat4::IDENTITY,
            animation: 0.into(),
            time: 0.0,
            animation_epoch: None,
            emission: Vec3::ONE,
        };
        let mut renderer =
            Renderer::new(pack, assets.path(), WIDTH, HEIGHT, &Atlas::new(1.0), &[bed])?;
        renderer.water_ocean().exact = true;
        let surface = surface(&world, tier)?;
        let count: u32 = surface.indices(if tier == Tier::Low { 2 } else { 1 }).len() as u32;
        renderer.set_water_surface(Some(Arc::new(surface)));
        let (zenith, horizon, color) = palette(spec.dusk);
        // A lamp high toward the sun stands in for it: this renderer lights
        // with point lights only.
        let sun = spec.target + spec.sun * 20.0;
        let lighting = Lighting {
            ambient: Vec3::from(zenith) * 0.25 + Vec3::splat(0.04),
            exposure: 1.0,
            fog: Vec3::from(horizon),
            density: 0.004,
            time: TIME,
            lights: vec![Light {
                position: sun,
                color: Vec3::from(color),
                intensity: 900.0,
                range: 60.0,
            }],
            shadowed: 0,
            height_fog: None,
        };
        for with_water in [false, true] {
            renderer.water = with_water.then(|| water(&world, &spec));
            let mut pixels = Vec::new();
            let mut total = 0.0;
            for frame in 0..=TIMED {
                let started = Instant::now();
                pixels = renderer.draw(camera(&spec), &[], &UiBatch::default(), &lighting)?;
                if frame > 0 {
                    total += started.elapsed().as_secs_f64();
                }
            }
            let name = format!(
                "{}imported-{}-{}.png",
                if with_water { "" } else { "dry/" },
                tier_name(tier),
                spec.name,
            );
            png(&directory.join(&name), &pixels)?;
            records.push(json!({
                "image": name,
                "renderer": "imported",
                "tier": tier_name(tier),
                "view": spec.name,
                "water": with_water,
                "water_indices": if with_water { count } else { 0 },
                "frame_ms_with_readback": total / TIMED as f64 * 1e3,
                "ocean_worker_us": renderer.water_ocean().micros,
                "sha256": digest(&directory.join(&name))?,
                "resolution": [WIDTH, HEIGHT],
                "adapter": renderer.adapter_name.clone(),
            }));
        }
    }
    Ok(records)
}

// ---- Output.

fn png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    png_at(path, pixels, [WIDTH, HEIGHT])
}

fn png_at(path: &Path, pixels: &[u8], [width, height]: [u32; 2]) -> Result<(), String> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).map_err(|e| e.to_string())?,
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}

fn digest(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// The share of pixels whose color differs by more than two 8-bit steps.
fn changed(a: &[u8], b: &[u8]) -> f64 {
    let differ = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(p, q)| {
            p.iter()
                .zip(q.iter())
                .take(3)
                .any(|(x, y)| x.abs_diff(*y) > 2)
        })
        .count();
    differ as f64 / (a.len() / 4) as f64
}

fn decode(path: &Path) -> Result<Vec<u8>, String> {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).map_err(|e| e.to_string())?,
    ));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("no buffer size")?];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    buffer.truncate(info.buffer_size());
    Ok(buffer)
}

/// `docs/verse/water.md`'s budgets for water at 1080p: GPU time, ms, and
/// GPU memory, bytes.
fn budget(tier: Tier) -> (f64, u64) {
    let budget = verse_engine::quality::WaterBudget::of(tier);
    (budget.gpu_ms, budget.gpu_bytes)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    std::fs::create_dir_all(directory.join("dry")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(directory.join("w2")).map_err(|e| e.to_string())?;
    let mut baseline: Vec<PathBuf> = Vec::new();
    match args.next().as_deref() {
        Some("--imported") => {
            let tier = Tier::parse(&args.next().ok_or("--imported needs a tier")?)
                .ok_or("unknown tier")?;
            let records = imported(&directory, tier)?;
            std::fs::write(
                directory.join(format!("imported-{}.json", tier_name(tier))),
                serde_json::to_vec_pretty(&records).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
        Some("--baseline") => {
            baseline.push(PathBuf::from(
                args.next().ok_or("--baseline needs a directory")?,
            ));
            while args.next().as_deref() == Some("--baseline") {
                baseline.push(PathBuf::from(
                    args.next().ok_or("--baseline needs a directory")?,
                ));
            }
        }
        _ => {}
    }
    for base in &baseline {
        baseline_restored(base)?;
    }
    let gpu = gpu()?;
    let mut records = Vec::new();
    let mut checks = Vec::new();
    let mut tiers = Vec::new();
    // `WATER_CAPTURE_TIERS` (a comma-separated list) narrows the tiers, and
    // `WATER_CAPTURE_IMPORTED=0` skips the imported renderer, for a quick
    // look; the committed capture runs everything.
    let only = std::env::var("WATER_CAPTURE_TIERS").ok();
    let imported_too = std::env::var("WATER_CAPTURE_IMPORTED").as_deref() != Ok("0");
    for tier in TIERS {
        if only
            .as_deref()
            .is_some_and(|only| !only.split(',').any(|name| name == tier_name(tier)))
        {
            continue;
        }
        let mut photo = Photo::new(
            &gpu.device,
            &gpu.queue,
            capability(tier),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )?;
        let mut worst_ms: f64 = 0.0;
        let mut worst_added: f64 = 0.0;
        let mut worst_bytes = 0;
        let mut over = Vec::new();
        let (ms_budget, bytes_budget) = budget(tier);
        for spec in views() {
            let mut shots = Vec::new();
            let ways: &[Wet] = if tier == Tier::Low {
                &[Wet::Dry, Wet::Tier]
            } else {
                &[Wet::Dry, Wet::Halves, Wet::Tier]
            };
            for &wet in ways {
                let (pixels, _, count, _) =
                    physical(&gpu, &mut photo, tier, &spec, wet, [WIDTH, HEIGHT])?;
                let (_, ms, _, bytes) = physical(&gpu, &mut photo, tier, &spec, wet, BUDGET_SIZE)?;
                let folder = match wet {
                    Wet::Dry => "dry/",
                    Wet::Halves => "w2/",
                    Wet::Tier => "",
                };
                let name = format!("{folder}physical-{}-{}.png", tier_name(tier), spec.name);
                png(&directory.join(&name), &pixels)?;
                eprintln!("{name}: {ms:.2} ms a frame at 1080p");
                records.push(json!({
                    "image": name,
                    "renderer": "physical",
                    "tier": tier_name(tier),
                    "view": spec.name,
                    "water": wet != Wet::Dry,
                    "screen_copies": wet == Wet::Tier && tier != Tier::Low,
                    "water_indices": if wet == Wet::Dry { 0 } else { count },
                    "scene_target": if tier == Tier::Low { "Rgba8UnormSrgb (no floating-point target)" } else { "Rgba16Float" },
                    "samples": capability(tier).samples,
                    "ocean_worker_us": photo.ocean.micros,
                    "frame_ms_1080p": ms,
                    "water_gpu_bytes_1080p": if wet == Wet::Dry { 0 } else { bytes },
                    "sha256": digest(&directory.join(&name))?,
                    "resolution": [WIDTH, HEIGHT],
                    "adapter": gpu.adapter,
                }));
                shots.push((wet, name, pixels, ms, bytes));
            }
            let find = |wet: Wet| shots.iter().find(|s| s.0 == wet);
            let (Some(dry), Some(wet)) = (find(Wet::Dry), find(Wet::Tier)) else {
                return Err("a view is missing a picture".into());
            };
            let halves = find(Wet::Halves);
            let water_ms = wet.3 - dry.3;
            let added_ms = halves.map_or(0.0, |h| wet.3 - h.3);
            worst_ms = worst_ms.max(water_ms);
            worst_added = worst_added.max(added_ms);
            worst_bytes = worst_bytes.max(wet.4);
            if water_ms > ms_budget {
                over.push(json!({
                    "view": spec.name,
                    "water_ms_1080p": water_ms,
                    "two_halves_water_ms_1080p": halves.map(|h| h.3 - dry.3),
                }));
            }
            let mut check = json!({
                "image": wet.1,
                "renderer": "physical",
                "tier": tier_name(tier),
                "view": spec.name,
                "pixels_changed_by_water": changed(&wet.2, &dry.2),
                "water_drawn": changed(&wet.2, &dry.2) > 0.02,
                "water_ms_1080p": water_ms,
                "added_by_copies_ms_1080p": added_ms,
                "water_gpu_bytes_1080p": wet.4,
            });
            if let Some(h) = halves {
                // Refraction, the mirror, and the trace change what the
                // water shows over the two halves. From under the surface
                // the copies show what the halves did (W7 refracts it).
                check["pixels_changed_by_copies"] = json!(changed(&wet.2, &h.2));
                if !spec.under {
                    check["copies_change_the_water"] = json!(changed(&wet.2, &h.2) > 0.01);
                }
            }
            if tier == Tier::Low
                && let Some(earlier) = baseline
                    .iter()
                    .map(|base| base.join(&wet.1))
                    .find(|earlier| earlier.exists())
            {
                let share = changed(&wet.2, &decode(&earlier)?);
                check["pixels_changed_from_baseline"] = json!(share);
                check["baseline"] = json!(earlier.display().to_string());
                check["low_unchanged"] = json!(share < 0.001);
            }
            checks.push(check);
        }
        tiers.push(json!({
            "tier": tier_name(tier),
            "passes_added": match tier {
                Tier::Low => "none",
                Tier::Medium => "a half-resolution planar mirror, the scene's resolve into a color copy, and a depth copy",
                Tier::High => "a full-resolution planar mirror, the scene's resolve into a color copy, and a depth copy; screen-space reflection runs inside the water's own draw",
            },
            "water_gpu_bytes_1080p": worst_bytes,
            "copies_and_mirror_bytes_1080p": verse_pbr::water::screen::Plan::of(tier).bytes(1920, 1080, 8),
            "gpu_bytes_budget": bytes_budget,
            "worst_water_ms_1080p": worst_ms,
            "worst_added_by_copies_ms_1080p": worst_added,
            "gpu_ms_budget": ms_budget,
            // What W5 adds: its targets within the memory budget, and its
            // time no more than the budget's whole allowance.
            "added_within_budget": worst_bytes <= bytes_budget && worst_added <= ms_budget,
            // Views whose water costs more than the budget on this machine
            // whichever way it draws: the cost lies in the surface itself.
            "views_over_time_budget": over,
        }));
        if !imported_too {
            continue;
        }
        let status =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg(&directory)
                .arg("--imported")
                .arg(tier_name(tier))
                .env("VERSE_QUALITY", tier_name(tier))
                .status()
                .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!(
                "The imported renderer's {} captures failed",
                tier_name(tier)
            ));
        }
        let part = directory.join(format!("imported-{}.json", tier_name(tier)));
        let imported: Vec<Value> =
            serde_json::from_slice(&std::fs::read(&part).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        std::fs::remove_file(&part).map_err(|e| e.to_string())?;
        records.extend(imported);
    }
    // The imported renderer keeps the two halves on every tier.
    for record in records
        .iter()
        .filter(|r| r["water"] == true && r["renderer"] == "imported")
    {
        let wet = record["image"].as_str().unwrap_or_default();
        let dry = format!("dry/{wet}");
        let share = changed(
            &decode(&directory.join(wet))?,
            &decode(&directory.join(&dry))?,
        );
        checks.push(json!({
            "image": wet,
            "renderer": "imported",
            "tier": record["tier"],
            "view": record["view"],
            "pixels_changed_by_water": share,
            "water_drawn": share > 0.02,
        }));
    }
    let all = checks.iter().all(|c| c["water_drawn"] == true);
    let copies = checks
        .iter()
        .all(|c| c.get("copies_change_the_water").is_none_or(|v| v == true));
    let low = checks
        .iter()
        .all(|c| c.get("low_unchanged").is_none_or(|v| v == true));
    std::fs::write(
        directory.join("capture.json"),
        serde_json::to_vec_pretty(&records).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        directory.join("validation.json"),
        serde_json::to_vec_pretty(&json!({
            "capture_command": "cargo run --release -p verse-pbr --example water_capture -- OUTPUT_DIRECTORY --baseline bench/verse/2026-10-07/water-w4",
            "capture": "Real GPU output from both renderers at each tier, each view without water (dry/), with the two-half water every tier drew before W5 (w2/, physical Medium and High), and with the tier's water; no grading or editing.",
            "timing": "Each frame time is at 1920 by 1080: the fastest of seven rounds of ten frames submitted back to back and waited on once, divided by ten, run under the build lease's quiet lease; water_ms is the wet frame less the dry one, and added_by_copies_ms the tier's water less the two halves.",
            "water_drawn_in_every_view": all,
            "copies_change_every_medium_and_high_view": copies,
            "low_unchanged_from_baseline": low,
            "low_tier": "Low draws into an 8-bit sRGB scene target with one sample and adds no pass or render target: its water is the two halves inside the scene pass, as before W5.",
            "tiers": tiers,
            "views": checks,
        }))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    eprintln!(
        "Captures stay out of git: scripts/bench-artifacts.py push {}",
        directory.display()
    );
    if !all {
        return Err("A view's water drew nothing".into());
    }
    Ok(())
}

/// A baseline whose captures live in the bench bucket must be restored
/// first; otherwise its Low pictures would silently go uncompared.
fn baseline_restored(base: &std::path::Path) -> Result<(), String> {
    let Ok(text) = std::fs::read_to_string(base.join("bench-artifacts.json")) else {
        return Ok(());
    };
    let manifest: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let missing = manifest["files"]
        .as_object()
        .into_iter()
        .flatten()
        .any(|(path, _)| !base.join(path).exists());
    if missing {
        return Err(format!(
            "The baseline's captures are in the bench bucket; run scripts/bench-artifacts.py restore {}",
            base.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod w11 {
    use super::*;

    /// CPU synthesis and raster only. Compile with a filtered `cargo test
    /// --release --example water_capture --no-run`; execute under a quiet
    /// lease, with WATER_W11_OUTPUT in the assigned scratch directory.
    #[test]
    #[ignore = "fixed-view native raster measurement under a quiet lease"]
    fn water_w11_fixed_views() {
        let directory = PathBuf::from(std::env::var("WATER_W11_OUTPUT").expect("WATER_W11_OUTPUT"));
        std::fs::create_dir_all(&directory).unwrap();
        let gpu = gpu().unwrap();
        let mut records = Vec::new();
        let selected = std::env::var("WATER_W11_CASE").ok();
        for tier in TIERS {
            for spec in all_views().into_iter().map(placed).filter(|v| {
                [
                    "pond-noon",
                    "pond-posts",
                    "open-storm",
                    "waterline",
                    "under",
                ]
                .contains(&v.name)
            }) {
                let case = format!("{}/{}", tier_name(tier), spec.name);
                if selected.as_ref().is_some_and(|selected| selected != &case) {
                    continue;
                }
                let mut photo = Photo::new(
                    &gpu.device,
                    &gpu.queue,
                    capability(tier),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                )
                .unwrap();
                let mut dry_photo = Photo::new(
                    &gpu.device,
                    &gpu.queue,
                    capability(tier),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                )
                .unwrap();
                let mut dry_samples = Vec::new();
                let (dry, dry_fence_ms, _, _) = physical_sampled(
                    &gpu,
                    &mut dry_photo,
                    tier,
                    &spec,
                    Wet::Dry,
                    BUDGET_SIZE,
                    Some(&mut dry_samples),
                )
                .unwrap();
                let mut samples = Vec::new();
                let (wet, fence_ms, _, bytes) = physical_sampled(
                    &gpu,
                    &mut photo,
                    tier,
                    &spec,
                    Wet::Tier,
                    BUDGET_SIZE,
                    Some(&mut samples),
                )
                .unwrap();
                let steady = &samples[samples.len().saturating_sub(96)..];
                let summary = |values: Vec<f64>| {
                    let mut values: Vec<_> = values.into_iter().filter(|v| v.is_finite()).collect();
                    values.sort_by(f64::total_cmp);
                    let n = values.len();
                    json!({"count":n, "mean":(n > 0).then(|| values.iter().sum::<f64>() / n as f64),
                        "p95":values.get(n.saturating_sub(1)*95/100), "max":values.last()})
                };
                let gpu_samples: Vec<_> = steady.iter().filter_map(|s| s.gpu).collect();
                let gpu_ms = summary(gpu_samples.iter().map(|s| s.water_ms).collect());
                let main = summary(
                    steady
                        .iter()
                        .map(|s| s.main_cpu_ms.unwrap_or(s.main_ms))
                        .collect(),
                );
                let worker = summary(steady.iter().map(|s| s.worker_ms).collect());
                let last = steady.last().unwrap();
                let jobs = steady.iter().map(|s| s.completed_jobs).sum::<u64>();
                let synthesis_ms = steady.iter().map(|s| s.completed_synthesis_ms).sum::<f64>();
                let worker_ms = steady.iter().map(|s| s.worker_ms).sum::<f64>();
                let budget = verse_engine::quality::WaterBudget::of(tier);
                let reduced = last.effects_reduced;
                let fence_water_ms = fence_ms - dry_fence_ms;
                // A queue-fence difference is not a substitute for a missing
                // timestamp. Keep budget admission unknown without GPU data.
                let within = gpu_ms["mean"].as_f64().map(|gpu_cost| {
                    bytes <= budget.gpu_bytes
                        && main["mean"].as_f64().unwrap_or(0.0) <= budget.main_ms
                        && worker["mean"].as_f64().unwrap_or(0.0) <= budget.worker_ms
                        && gpu_cost <= budget.gpu_ms
                });
                let share = changed(&wet, &dry);
                let filename = format!("native-{}-{}.png", tier_name(tier), spec.name);
                png_at(&directory.join(&filename), &wet, BUDGET_SIZE).unwrap();
                records.push(json!({"tier":tier_name(tier),"view":spec.name,"adapter":gpu.adapter,
                    "measured_at_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64,
                    "size":BUDGET_SIZE,"cadence_hz":60,"steady_frames":steady.len(),
                    "timing":"paced native pass timestamps when valid; submit-to-fence wall time excludes cadence sleep",
                    "gpu_timestamp_scope":"mirror, color/depth copies, and surface passes; shared opaque underwater shading and implicit queue texture uploads are not isolated",
                    "gpu_ms":gpu_ms,"gpu_valid_samples":gpu_samples.len(),
                    "gpu_sample_fraction":gpu_samples.len() as f64 / steady.len() as f64,
                    "gpu_passes_ms":["mirror","opaque (excluded)","color and depth copies","surface"],
                    "gpu_bytes":bytes,"main_ms":main,"worker_ms":worker,
                    "main_cpu_clock":last.main_cpu_ms.is_some(),"worker_cpu_clock":last.worker_cpu_supported,
                    "completed_jobs":jobs,"completed_synthesis_ms":synthesis_ms,
                    "mean_completed_job_elapsed_ms":(jobs > 0).then(|| synthesis_ms / jobs as f64),
                    "mean_completed_job_cpu_ms":(jobs > 0 && last.worker_cpu_supported).then(|| worker_ms / jobs as f64),
                    "observed_last_job_elapsed_ms":summary(steady.iter().filter(|s| s.completed_jobs > 0).map(|s| s.synthesis_ms).collect()),
                    "observed_last_job_cpu_ms":summary(steady.iter().filter(|s| s.completed_jobs > 0).filter_map(|s| s.synthesis_cpu_ms).collect()),
                    "queue_fence_frame_ms_fastest":fence_ms,"queue_fence_water_ms_estimate":fence_water_ms,"last":last,
                    "within_budget":within,"effects_reduced":reduced,"water_changed_pixels":share,
                    "capture":filename,"capture_sha256":digest(&directory.join(&filename)).unwrap(),
                    "samples":samples}));
                // Keep failed-view evidence before evaluating acceptance.
                std::fs::write(
                    directory.join("native.json"),
                    serde_json::to_vec_pretty(&records).unwrap(),
                )
                .unwrap();
                assert!(
                    share > 0.01,
                    "{} {} keeps water visible",
                    tier_name(tier),
                    spec.name
                );
                assert!(
                    !last.gpu_timestamps || gpu_samples.len() >= steady.len() / 2,
                    "{} {} needs GPU samples from at least half its steady frames",
                    tier_name(tier),
                    spec.name
                );
                assert!(
                    within == Some(true) || reduced || !last.gpu_timestamps,
                    "{} {} admits cost or reduces effects",
                    tier_name(tier),
                    spec.name
                );
                assert!(
                    bytes <= budget.gpu_bytes,
                    "{} {} water residency",
                    tier_name(tier),
                    spec.name
                );
            }
        }
        assert!(
            !records.is_empty(),
            "WATER_W11_CASE must select a known view"
        );
    }
}
