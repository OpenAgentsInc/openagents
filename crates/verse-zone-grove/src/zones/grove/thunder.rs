//! Thunderwave's blast: a wave of thunderous force erupting from the caster
//! through the SRD's 15-foot cube.
//!
//! In the first quarter second a translucent pressure front swells from the
//! caster to the cube's far face, a cube-ish dome of pale blue-white glow
//! brightest at its rim. A flash bursts at the caster's hands and spreads
//! over the grass, shock rings race over the ground, and rippling rings of
//! disturbed air ride out with the front. Crackling arcs and sparks fly from
//! the caster; dust, grass blades, and pebbles kick up along the ground
//! where the front passes and fall back.
//!
//! As in Reverse Gravity's particles ([`motes`]), every position is a
//! closed-form function of the blast's age and a seed, so the frame rate
//! never changes it and a replay draws the same frame. The renderer admits
//! only a few hundred glow quads a frame at each quality tier, so the blast
//! spends them on what must be translucent (the flash, the front, spark
//! heads, and dust; `GLOW_QUADS` at most) and draws the rings, ripples,
//! arcs, and spark trails as lines and the debris as shaded faces. Opaque
//! lines would darken if dimmed, so they fade by breaking up instead. No
//! new shader is needed, and the blast ends after [`LENGTH`] seconds.

use crate::mesh::{Mesh, Vertex};
use crate::pbr::GlowVertex;
use crate::zones::everglade::spells::motes::{self, blob, face, pebble, seeded};
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

/// How long the pressure front takes to reach the cube's far face, s.
pub const FRONT: f32 = 0.25;
/// How long the whole blast draws, s.
pub const LENGTH: f32 = 0.9;
/// The cube's edge, m.
const CUBE: f32 = verse_world::spells::thunderwave::CUBE as f32;
/// How high the cube's origin stands over the caster's feet, m.
const CENTER: f32 = 0.9;

/// Points over the whole sphere whose forward half makes the front.
const SHELL: usize = 64;
/// Shock rings over the ground, and segments in each.
const RINGS: usize = 3;
const RING_SEGMENTS: usize = 40;
/// Rippling rings of disturbed air riding out with the front, and segments
/// in each.
const RIPPLES: usize = 2;
const RIPPLE_SEGMENTS: usize = 40;
/// Crackling arcs of lightning and their kinks.
const ARCS: usize = 6;
const KINKS: usize = 7;
/// Sparks flying from the caster; the first few carry a glowing head.
const SPARKS: usize = 40;
const SPARK_HEADS: usize = 8;
/// Puffs of dust kicked up off the ground.
const DUST: usize = 8;
/// Grass blades and pebbles thrown up.
const BLADES: usize = 30;
const PEBBLES: usize = 12;

/// The most glow quads one blast draws in a frame: the flash's three, the
/// front's forward half, spark heads, and dust.
pub const GLOW_QUADS: usize = 3 + SHELL / 2 + SPARK_HEADS + DUST;
/// The most line vertices one blast draws in a frame.
#[cfg(test)]
pub const MAX_LINE_VERTICES: usize =
    2 * (RINGS * RING_SEGMENTS + RIPPLES * RIPPLE_SEGMENTS + ARCS * KINKS * 3 + SPARKS);
/// The most shaded-face vertices one blast draws in a frame.
#[cfg(test)]
pub const MAX_FACE_VERTICES: usize = BLADES * 6 + PEBBLES * 24;

/// Thunder's pale blue-white.
const THUNDER: [f32; 3] = [0.62, 0.8, 1.0];
/// The hot core of the flash.
const WHITE: [f32; 3] = [0.9, 0.95, 1.0];
/// Line colors: the shock rings and ripples, and the arcs' hot cores.
const RING_LINE: [f32; 3] = [0.72, 0.88, 1.0];
const ARC_LINE: [f32; 3] = [0.93, 0.97, 1.0];
const ARC_EDGE: [f32; 3] = [0.55, 0.72, 1.0];
/// Sunlit dust.
const DUST_TINT: [f32; 3] = [0.9, 0.84, 0.72];
/// Glow luminances before exposure, cd/m^2, against Everglade's daylight
/// (`motes` uses the same scale).
const FLASH: f32 = 40.0;
const HALO: f32 = 4.0;
const SHELL_LUMINANCE: f32 = 0.8;
const SPARK_LUMINANCE: f32 = 8.0;
const DUST_LUMINANCE: f32 = 0.45;
/// Grass and stone colors for the thrown faces.
const GRASS: [[f32; 3]; 3] = [[0.24, 0.42, 0.12], [0.34, 0.5, 0.16], [0.42, 0.46, 0.18]];
const STONES: [[f32; 3]; 2] = [[0.45, 0.42, 0.38], [0.34, 0.32, 0.3]];
const GRAVITY: f32 = 9.8;

/// The blast's frame of reference: the cube's origin at the caster's
/// chest, its forward over the ground, its side, and the ground under it.
struct Frame {
    origin: Vec3,
    forward: Vec3,
    side: Vec3,
    ground: f32,
}

impl Frame {
    fn new(origin: Vec3, forward: Vec3) -> Self {
        let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
        Self {
            origin,
            forward,
            side: Vec3::new(-forward.z, 0.0, forward.x),
            ground: origin.y - CENTER,
        }
    }

    /// The ground under `p`, m: the meadow's heightfield, never below the
    /// caster's feet.
    fn floor(&self, p: Vec3) -> f32 {
        crate::zones::everglade::height(p.x, p.z).max(self.ground)
    }

    /// A point `x` m to the side, `y` m up, and `z` m forward of the origin.
    fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.origin + self.side * x + Vec3::Y * y + self.forward * z
    }
}

/// How far the front has swelled `age` seconds in, 0 to 1: fast out of the
/// caster and easing as it reaches the far face.
#[must_use]
pub fn reach(age: f32) -> f32 {
    let k = (age / FRONT).clamp(0.0, 1.0);
    1.0 - (1.0 - k).powi(3)
}

/// Draws the blast from `origin` (the cube's origin, at the caster's chest)
/// toward `forward`, `age` seconds after the cast, seen from `eye`. `seed`
/// varies one blast from the next.
pub fn draw(mesh: &mut Mesh, origin: Vec3, forward: Vec3, age: f32, eye: Vec3, seed: u64) {
    if !(0.0..LENGTH).contains(&age) {
        return;
    }
    let f = Frame::new(origin, forward);
    // The glow first, most telling first, so a renderer that runs out of
    // glow quads drops the least of it.
    flash(&mut mesh.glow, &f, age, eye);
    shell(&mut mesh.glow, &f, age, eye);
    sparks(mesh, &f, age, eye, seed);
    dust(&mut mesh.glow, &f, age, eye, seed);
    rings(&mut mesh.lines, &f, age, seed);
    ripples(&mut mesh.lines, &f, age, seed);
    arcs(&mut mesh.lines, &f, age, eye, seed);
    debris(mesh, &f, age, seed);
}

/// The camera's jolt `age` seconds after a blast, m: a quick shudder that
/// dies out in about a third of a second.
#[must_use]
pub fn shake(age: f32) -> Vec3 {
    if !(0.0..0.4).contains(&age) {
        return Vec3::ZERO;
    }
    let k = 0.07 * (-age / 0.1).exp();
    Vec3::new(
        (age * 97.0).sin(),
        0.7 * (age * 83.0 + 1.3).sin(),
        (age * 71.0 + 2.1).sin(),
    ) * k
}

/// The flash at the caster's hands, a white core and a blue halo kept
/// clear of the ground, and a flare spreading flat over the grass.
fn flash(out: &mut Vec<GlowVertex>, f: &Frame, age: f32, eye: Vec3) {
    let at = f.at(0.0, 0.35, 0.5);
    let core = (-age / 0.04).exp();
    let halo = (-age / 0.08).exp();
    // A camera-facing glow that dipped into the grass would be cut off
    // along a straight line, so the halo stays smaller than its height.
    let clear = (at.y - f.floor(at)) * 0.95;
    let white = WHITE.map(|c| c * FLASH * core);
    blob(out, at, (0.5 + 1.6 * age).min(clear), white, eye);
    let blue = THUNDER.map(|c| c * HALO * halo);
    blob(out, at, (1.0 + 3.0 * age).min(clear), blue, eye);
    let foot = f.at(0.0, 0.0, 1.4);
    let foot = Vec3::new(foot.x, f.floor(foot) + 0.1, foot.z);
    let r = 1.2 + 7.0 * age;
    let flare = THUNDER.map(|c| c * HALO * 0.6 * (-age / 0.15).exp());
    motes::quad(out, foot, f.side * r, f.forward * r, flare);
}

/// The pressure front: big soft glows over the forward half of a sphere,
/// pushed toward the cube's faces, swelling out from the caster and
/// brightest at the rim where the dome turns away from the eye.
fn shell(out: &mut Vec<GlowVertex>, f: &Frame, age: f32, eye: Vec3) {
    let e = reach(age);
    // Bright while it swells, then thinning out as it dissipates.
    let fade = 1.0 - smooth(FRONT * 0.7, FRONT + 0.3, age);
    if fade <= 0.0 {
        return;
    }
    let golden = PI * (3.0 - 5f32.sqrt());
    // The forward half of a Fibonacci lattice over the sphere.
    for i in 0..SHELL / 2 {
        let z = 1.0 - (i as f32 + 0.5) / SHELL as f32 * 2.0;
        let r = (1.0 - z * z).sqrt();
        let a = golden * i as f32;
        let d = Vec3::new(r * a.cos(), r * a.sin(), z);
        // Out to the cube's face along `d`, rounded toward a sphere.
        let cube = 1.0 / (d.x.abs() / 0.5).max(d.y.abs() / 0.5).max(d.z).max(1e-3);
        let extent = CUBE * (0.65 * cube.min(1.2) + 0.35 * 0.8) * e;
        let local = d * extent;
        // The ground stops the front: below it the dome is not drawn.
        if local.y < -CENTER {
            continue;
        }
        let half = (0.45 + 0.95 * e) * (0.8 + 0.2 * d.z);
        let mut at = f.at(local.x, local.y, local.z);
        // Kept clear of the grass, so the ground does not cut a glow off
        // along a straight line.
        at.y = at.y.max(f.floor(at) + half * 0.8);
        let normal = (f.side * d.x + Vec3::Y * d.y + f.forward * d.z).normalize_or(f.forward);
        let toward = (eye - at).normalize_or(Vec3::Z);
        let rim = 1.0 - normal.dot(toward).abs();
        let level = SHELL_LUMINANCE * fade * (0.12 + 1.1 * rim * rim);
        blob(out, at, half, THUNDER.map(|c| c * level), eye);
    }
}

/// Sparks flung outward from the caster: each a bright trail, the first
/// few with a glowing head.
fn sparks(mesh: &mut Mesh, f: &Frame, age: f32, eye: Vec3, seed: u64) {
    for i in 0..SPARKS {
        let h = seeded(seed ^ 0x5a4c, i);
        let g = seeded(seed ^ 0x5a4d, i);
        let delay = 0.06 * g[0];
        let life = 0.22 + 0.25 * g[1];
        let t = age - delay;
        if !(0.0..life).contains(&t) {
            continue;
        }
        let a = (h[0] - 0.5) * 2.6;
        let up = (h[1] - 0.25) * 1.2;
        let dir = (f.forward * a.cos() + f.side * a.sin() + Vec3::Y * up).normalize_or(f.forward);
        let speed = 10.0 + 10.0 * h[2];
        // Air drag slows them, and gravity bends the slow ones down.
        let drag = 4.0;
        let path = |t: f32| {
            let travel = speed * (1.0 - (-drag * t).exp()) / drag;
            f.at(0.0, 0.3, 0.5) + dir * travel - Vec3::Y * (0.5 * GRAVITY * 0.4 * t * t)
        };
        let at = path(t);
        let tail = path((t - 0.035).max(0.0));
        line(&mut mesh.lines, tail, at, ARC_LINE);
        if i < SPARK_HEADS {
            let fade = 1.0 - t / life;
            let level = SPARK_LUMINANCE * fade * fade * (0.6 + 0.4 * h[3]);
            blob(&mut mesh.glow, at, 0.07, THUNDER.map(|c| c * level), eye);
        }
    }
}

/// Dust kicked up where the front passes over the ground.
fn dust(out: &mut Vec<GlowVertex>, f: &Frame, age: f32, eye: Vec3, seed: u64) {
    for i in 0..DUST {
        let h = seeded(seed ^ 0x0d05, i);
        let along = 0.8 + (CUBE - 0.8) * h[0];
        let across = (h[1] - 0.5) * CUBE;
        let t = age - arrival(along);
        let life = 0.5 + 0.3 * h[2];
        if !(0.0..life).contains(&t) {
            continue;
        }
        let base = f.at(across, 0.0, along);
        let away = Vec3::new(base.x - f.origin.x, 0.0, base.z - f.origin.z).normalize_or(f.forward);
        let drift = (1.0 - (-3.0 * t).exp()) / 3.0;
        let k = t / life;
        let half = 0.4 + 0.9 * k;
        let mut at = base + away * (3.0 + 3.0 * h[3]) * drift;
        at.y = f.floor(at) + half * 0.85 + (1.2 + 1.0 * h[2]) * drift;
        let level = DUST_LUMINANCE * (1.0 - k) * (k * 8.0).min(1.0);
        blob(out, at, half, DUST_TINT.map(|c| c * level), eye);
    }
}

/// Shock rings racing over the grass ahead of the caster, rippling as they
/// go and breaking up as they fade.
fn rings(out: &mut Vec<Vertex>, f: &Frame, age: f32, seed: u64) {
    let span = 1.3 * PI;
    for ring in 0..RINGS {
        let t = age - 0.05 * ring as f32;
        if t <= 0.0 {
            continue;
        }
        let k = (t / 0.5).min(1.0);
        let radius = 0.5 + CUBE * 1.1 * (1.0 - (1.0 - k).powi(2));
        let fade = (1.0 - k) * (1.0 - 0.2 * ring as f32);
        let point = |s: usize| {
            let a = -span / 2.0 + span * s as f32 / RING_SEGMENTS as f32;
            let ripple = 1.0 + 0.04 * (a * 9.0 + t * 50.0 + ring as f32 * 2.0).sin();
            let p =
                f.at(0.0, 0.0, 0.0) + (f.forward * a.cos() + f.side * a.sin()) * radius * ripple;
            Vec3::new(p.x, f.floor(p) + 0.06, p.z)
        };
        for s in 0..RING_SEGMENTS {
            // Fainter toward the back, where the cube is not.
            let a = -span / 2.0 + span * (s as f32 + 0.5) / RING_SEGMENTS as f32;
            let keep = fade * (0.4 + 0.6 * a.cos().max(0.0));
            if kept(seed ^ 0x7123 ^ ring as u64, s, keep) {
                line(out, point(s), point(s + 1), RING_LINE);
            }
        }
    }
}

/// Rounded-square rings of rippling air standing across the cube, riding
/// out with the front and wobbling as they go.
fn ripples(out: &mut Vec<Vertex>, f: &Frame, age: f32, seed: u64) {
    for ripple in 0..RIPPLES {
        let t = age - 0.07 * ripple as f32;
        if t <= 0.0 {
            continue;
        }
        let k = (t / 0.4).min(1.0);
        let fade = (1.0 - k) * (1.0 - 0.3 * ripple as f32);
        let e = reach(t);
        let along = 0.4 + (CUBE - 0.4) * e * 0.95;
        let half = 0.4 + CUBE * 0.5 * e;
        let point = |s: usize| {
            let a = TAU * s as f32 / RIPPLE_SEGMENTS as f32;
            let wobble = 1.0 + 0.06 * (a * 6.0 + t * 55.0 + ripple as f32 * 1.7).sin();
            // A superellipse: between a circle and the cube's square face.
            let (c, s) = (a.cos(), a.sin());
            let x = c.signum() * c.abs().powf(0.6) * half * wobble;
            let y = s.signum() * s.abs().powf(0.6) * half * wobble;
            let mut p = f.at(x, y, along);
            p.y = p.y.max(f.floor(p) + 0.06);
            p
        };
        for s in 0..RIPPLE_SEGMENTS {
            if kept(seed ^ 0x3b17 ^ ripple as u64, s, fade) {
                line(out, point(s), point(s + 1), RING_LINE);
            }
        }
    }
}

/// Crackling arcs from the caster to the swelling front, jumping to a new
/// path thirty times a second.
fn arcs(out: &mut Vec<Vertex>, f: &Frame, age: f32, eye: Vec3, seed: u64) {
    let life = 0.35;
    if age >= life {
        return;
    }
    let flicker = (age * 30.0) as usize;
    let e = reach(age).max(0.2);
    let start = f.at(0.0, 0.35, 0.5);
    for arc in 0..ARCS {
        let h = seeded(seed ^ 0xa2c5, arc * 64 + flicker);
        // Some frames an arc is dark, and more as the blast ages, so they
        // crackle rather than glow.
        if h[3] < 0.15 + age / life * 0.6 {
            continue;
        }
        let a = (h[0] - 0.5) * 2.4;
        let up = (h[1] - 0.35) * 1.4;
        let end = f.at(
            a.sin() * CUBE * 0.5 * e,
            up * CUBE * 0.4 * e,
            a.cos() * CUBE * 0.95 * e,
        );
        let end = Vec3::new(end.x, end.y.max(f.floor(end) + 0.1), end.z);
        let mut previous = start;
        for kink in 1..=KINKS {
            let t = kink as f32 / KINKS as f32;
            let j = seeded(seed ^ 0x77e1, (arc * 64 + flicker) * 16 + kink);
            let jitter = if kink == KINKS {
                Vec3::ZERO
            } else {
                Vec3::new(j[0] - 0.5, j[1] - 0.5, j[2] - 0.5) * (0.6 * e)
            };
            let next = start.lerp(end, t) + jitter;
            // A hot core between two blue edges, a touch apart across the
            // view, so the bolt reads thicker than one line.
            let toward = (eye - next).normalize_or(Vec3::Z);
            let across = (next - previous).cross(toward).normalize_or(Vec3::Y) * 0.025;
            line(out, previous, next, ARC_LINE);
            line(out, previous + across, next + across, ARC_EDGE);
            line(out, previous - across, next - across, ARC_EDGE);
            previous = next;
        }
    }
}

/// Grass blades and pebbles thrown up by the front and falling back.
fn debris(mesh: &mut Mesh, f: &Frame, age: f32, seed: u64) {
    for i in 0..BLADES + PEBBLES {
        let h = seeded(seed ^ 0x9eb1, i);
        let g = seeded(seed ^ 0x9eb2, i);
        let along = 0.5 + (CUBE - 0.5) * h[0];
        let across = (h[1] - 0.5) * CUBE * 0.95;
        let t = age - arrival(along);
        if t < 0.0 {
            continue;
        }
        let base = f.at(across, 0.0, along);
        let base = Vec3::new(base.x, f.floor(base) + 0.04, base.z);
        let away = Vec3::new(base.x - f.origin.x, 0.0, base.z - f.origin.z).normalize_or(f.forward);
        let out = 2.0 + 3.5 * h[2];
        let rise = 2.5 + 3.5 * h[3];
        let mut at = base + away * out * t + Vec3::Y * (rise * t - 0.5 * GRAVITY * t * t);
        // Once it falls back to the grass it is gone.
        let floor = f.floor(at);
        if at.y <= floor {
            continue;
        }
        at.y = at.y.max(floor + 0.02);
        let fade = 1.0 - (age / LENGTH).powi(4);
        let axis = Vec3::new(g[0] - 0.5, g[1] - 0.5, g[2] - 0.5).normalize_or(Vec3::Y);
        let turn = Quat::from_axis_angle(axis, TAU * g[3] + t * (8.0 + 10.0 * g[0]));
        if i < BLADES {
            let size = (0.16 + 0.1 * g[1]) * fade;
            let color = GRASS[i % GRASS.len()];
            let p = |x: f32, y: f32| at + turn * Vec3::new(x, y, 0.0) * size;
            // A thin blade, both faces so it shows from either side.
            let (root_l, root_r, tip) = (p(-0.18, -1.0), p(0.18, -1.0), p(0.0, 1.0));
            face(mesh, [root_l, root_r, tip], color);
            face(mesh, [root_r, root_l, tip], color);
        } else {
            let size = (0.07 + 0.08 * g[1]) * fade;
            pebble(mesh, at, size, turn, STONES[i % STONES.len()], g);
        }
    }
}

/// When the front reaches ground `along` m ahead of the caster, s.
fn arrival(along: f32) -> f32 {
    FRONT * (along / CUBE).clamp(0.0, 1.0).powf(0.6) * 0.8
}

/// Whether piece `i` of a fading line still shows: a fraction `keep` of the
/// pieces, the same ones from frame to frame.
fn kept(seed: u64, i: usize, keep: f32) -> bool {
    seeded(seed, i)[0] < keep
}

fn line(out: &mut Vec<Vertex>, a: Vec3, b: Vec3, color: [f32; 3]) {
    for p in [a, b] {
        out.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Vec3 = Vec3::new(1.0, 0.9, -4.0);
    const EYE: Vec3 = Vec3::new(1.0, 3.0, -10.0);

    fn frame(age: f32, seed: u64) -> Mesh {
        let mut mesh = Mesh::default();
        draw(&mut mesh, ORIGIN, Vec3::Z, age, EYE, seed);
        mesh
    }

    #[test]
    fn the_same_age_and_seed_draw_the_same_frame() {
        for age in [0.0, 0.03, 0.12, 0.25, 0.5, 0.8] {
            let (a, b) = (frame(age, 7), frame(age, 7));
            assert_eq!(a.glow, b.glow, "{age}");
            assert_eq!(a.lines, b.lines, "{age}");
            assert_eq!(a.faces, b.faces, "{age}");
        }
        assert_ne!(frame(0.3, 7).faces, frame(0.3, 8).faces);
    }

    #[test]
    fn every_frame_is_bounded_and_finite_and_the_blast_ends() {
        let mut most = [0; 3];
        for step in 0..=100 {
            let age = step as f32 * 0.01;
            let mesh = frame(age, 3);
            assert!(
                mesh.glow.len() <= GLOW_QUADS * 6,
                "{age}: {}",
                mesh.glow.len()
            );
            assert!(mesh.lines.len() <= MAX_LINE_VERTICES, "{age}");
            assert!(mesh.faces.len() <= MAX_FACE_VERTICES, "{age}");
            assert!(
                mesh.glow
                    .iter()
                    .all(|g| g.pos.iter().chain(&g.radiance).all(|x| x.is_finite()))
            );
            assert!(
                mesh.lines
                    .iter()
                    .all(|v| v.pos.iter().all(|x| x.is_finite()))
            );
            most[0] = most[0].max(mesh.glow.len() / 6);
            most[1] = most[1].max(mesh.lines.len() / 2);
            most[2] = most[2].max(mesh.faces.len() / 3);
        }
        // A rich blast that fits the medium tier's 192 glow quads three
        // times over.
        assert!(most[0] >= 30 && GLOW_QUADS * 3 <= 192, "{most:?}");
        assert!(most[1] > 150 && most[2] > 60, "{most:?}");
        assert!(frame(LENGTH, 3).glow.is_empty() && frame(LENGTH, 3).lines.is_empty());
        assert!(frame(-0.01, 3).glow.is_empty());
        assert_eq!(shake(0.5), Vec3::ZERO);
        assert!(shake(0.02).length() > 0.01);
    }

    #[test]
    fn the_front_fills_the_cube_ahead_of_the_caster_in_a_quarter_second() {
        assert_eq!(reach(0.0), 0.0);
        assert_eq!(reach(FRONT), 1.0);
        let reach_at = |age: f32| {
            let mut glow = Vec::new();
            shell(&mut glow, &Frame::new(ORIGIN, Vec3::Z), age, EYE);
            glow.iter()
                .map(|g| g.pos[2] - ORIGIN.z)
                .fold(f32::MIN, f32::max)
        };
        let (early, full) = (reach_at(0.05), reach_at(FRONT));
        assert!(early < full * 0.7, "{early} then {full}");
        assert!(full > CUBE * 0.9 && full < CUBE * 1.5, "{full}");
        // Nothing of the blast is drawn below the ground.
        let mesh = frame(0.4, 5);
        let ground = ORIGIN.y - CENTER;
        assert!(mesh.faces.iter().all(|v| v.pos[1] >= ground - 0.2));
        assert!(mesh.lines.iter().all(|v| v.pos[1] >= ground));
    }
}
