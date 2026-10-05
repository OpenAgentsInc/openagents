//! Reverse Gravity's particles: what the reversed field carries.
//!
//! Inside the cylinder, dust, pebbles, leaves, and faint sparks lift off
//! the ground and fall upward, faster as they climb, then gather and hover
//! just under the top plane before they fade and lift again from the
//! ground. A soft glow swirls up the cylinder's wall, and a shimmering ring
//! marks its base and its top.
//!
//! Every position is a closed-form function of the spell's age and a
//! particle's seed, so the effect never depends on the frame rate and a
//! replay draws the same frame. Dust, sparks, the wall glow, and the rings
//! are additive glow quads turned toward the eye; pebbles and leaves are a
//! few shaded faces each. The counts are fixed: 320 particles in all.

use crate::mesh::{Mesh, Vertex};
use crate::pbr::GlowVertex;
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};
use verse_world::reverse_gravity::Cylinder;

/// Glowing dust motes.
const DUST: usize = 150;
/// Faint violet sparks.
const SPARKS: usize = 60;
/// Small tumbling stones.
const PEBBLES: usize = 50;
/// Fluttering leaves.
const LEAVES: usize = 60;
/// Every particle the effect draws, not counting the wall glow and rings.
#[cfg(test)]
const PARTICLES: usize = DUST + SPARKS + PEBBLES + LEAVES;
/// Soft glow streaks rising around the cylinder's wall.
const STREAKS: usize = 48;
/// Shimmering blobs on each of the base and top rings.
const RING: usize = 192;

/// How long a particle takes to fade in at the ground and out at the top, s.
const FADE: f32 = 0.6;
/// The spell's violet, deeper than the chamber's guide lines so it keeps its hue in daylight.
const VIOLET: [f32; 3] = [0.62, 0.4, 1.0];
/// Sunlit dust.
const DUST_TINT: [f32; 3] = [1.0, 0.9, 0.72];
/// Luminances before exposure, cd/m^2. Everglade's sunlit grass is a few
/// hundred; these stay readable over it without washing it out.
const DUST_LUMINANCE: f32 = 1.6;
const SPARK_LUMINANCE: f32 = 4.0;
const STREAK_LUMINANCE: f32 = 0.8;
const RING_LUMINANCE: f32 = 1.2;
/// Pebble and leaf colors, shaded like the zone's other spell faces.
const STONES: [[f32; 3]; 3] = [[0.42, 0.4, 0.37], [0.52, 0.47, 0.4], [0.33, 0.31, 0.3]];
const FOLIAGE: [[f32; 3]; 4] = [
    [0.26, 0.42, 0.12],
    [0.4, 0.5, 0.14],
    [0.6, 0.48, 0.16],
    [0.46, 0.3, 0.13],
];

/// Draws Reverse Gravity's particles in `cylinder`, `age` seconds after the
/// cast, seen from `eye`.
pub(super) fn draw(mesh: &mut Mesh, cylinder: &Cylinder, age: f32, eye: Vec3) {
    let field = Field::of(cylinder);
    for i in 0..DUST {
        if let Some(m) = field.mote(0x0d05, i, age, 2.0) {
            let h = seeded(0x0d06, i);
            let half = 0.09 + 0.1 * h[0];
            let flicker = 0.75 + 0.25 * (age * (2.0 + 3.0 * h[1]) + TAU * h[2]).sin();
            let radiance = DUST_TINT.map(|c| c * DUST_LUMINANCE * flicker * m.fade);
            blob(&mut mesh.glow, m.at, half, radiance, eye);
        }
    }
    for i in 0..SPARKS {
        if let Some(m) = field.mote(0x5a4c, i, age, 2.6) {
            let h = seeded(0x5a4d, i);
            let half = 0.14 + 0.16 * h[0];
            // A slow twinkle with a sharp peak.
            let twinkle =
                0.3 + 0.7 * (0.5 + 0.5 * (age * (3.0 + 4.0 * h[1]) + TAU * h[2]).sin()).powi(3);
            let radiance = VIOLET.map(|c| c * SPARK_LUMINANCE * twinkle * m.fade);
            blob(&mut mesh.glow, m.at, half, radiance, eye);
        }
    }
    for i in 0..PEBBLES {
        if let Some(m) = field.mote(0x9eb1, i, age, 1.0) {
            let h = seeded(0x9eb2, i);
            let size = (0.05 + 0.08 * h[0]) * m.fade;
            let axis = Vec3::new(h[1] - 0.5, h[2] - 0.5, h[3] - 0.5).normalize_or(Vec3::Y);
            let turn = Quat::from_axis_angle(axis, m.age * (1.0 + 2.5 * h[2]));
            let color = STONES[i % STONES.len()];
            pebble(mesh, m.at, size, turn, color, h);
        }
    }
    for i in 0..LEAVES {
        if let Some(m) = field.mote(0x1eaf, i, age, 1.2) {
            let h = seeded(0x1eb0, i);
            let size = (0.09 + 0.07 * h[0]) * m.fade;
            // A leaf rocks about its length and spins slowly about the
            // vertical as it climbs.
            let spin = Quat::from_rotation_y(TAU * h[1] + m.age * (0.8 + h[2]));
            let rock = Quat::from_rotation_x(0.9 * (m.age * (2.0 + 2.0 * h[3]) + TAU * h[3]).sin());
            let color = FOLIAGE[i % FOLIAGE.len()];
            leaf(mesh, m.at, size, spin * rock, color);
        }
    }
    field.wall(&mut mesh.glow, age, eye);
    field.rings(&mut mesh.glow, age, eye);
}

/// The cylinder in render units.
struct Field {
    center: Vec3,
    radius: f32,
    height: f32,
}

/// One particle at one moment.
struct Mote {
    at: Vec3,
    /// 0 when it has faded out, 1 when it is whole.
    fade: f32,
    /// Seconds since it last lifted off.
    age: f32,
}

impl Field {
    fn of(c: &Cylinder) -> Self {
        Self {
            center: c.base.as_vec3(),
            radius: c.radius as f32,
            height: c.height as f32,
        }
    }

    /// Particle `i` of the family `seed`, `age` seconds into the spell, or
    /// `None` while it has not yet lifted off. `lift` scales how hard the
    /// field pulls it, m/s^2, so heavier things climb slower than dust.
    fn mote(&self, seed: u64, i: usize, age: f32, lift: f32) -> Option<Mote> {
        let h = seeded(seed, i);
        let g = seeded(seed ^ 0xa5a5, i);
        // Denser toward the caster at the center, so the motes stand
        // around the player rather than only at the far wall.
        let r = self.radius * 0.95 * h[0].powf(0.8);
        let start = TAU * h[1];
        // Gather a little under the top plane, each at its own depth.
        let depth = 0.4 + 2.2 * g[0];
        let climb = self.height - depth;
        let accel = lift * (0.8 + 0.6 * h[2]);
        let rise = (2.0 * climb / accel).sqrt();
        let hover = 2.0 + 4.0 * g[1];
        let period = rise + hover + FADE;
        // Staggered lift-off over the first few seconds; after that each
        // particle cycles: rise, hover, fade, and lift again.
        let delay = 3.0 * h[3];
        let since = age - delay;
        if since < 0.0 {
            return None;
        }
        let t = since.rem_euclid(period);
        // A slow swirl about the axis, a touch faster higher up.
        let (y, fade) = if t < rise {
            (0.5 * accel * t * t, (t / FADE).min(1.0))
        } else if t < rise + hover {
            let bob = 0.25 * ((t - rise) * (0.9 + g[2]) + TAU * g[3]).sin();
            // Settle into the hover with a short ease from the climb.
            let settle = (-(t - rise) * 3.0).exp();
            (climb + bob * (1.0 - settle), 1.0)
        } else {
            (climb, 1.0 - (t - rise - hover) / FADE)
        };
        let angle = start + since * (0.08 + 0.1 * g[2]) + 0.35 * y / self.height;
        let ground = self.center.y + 0.08;
        Some(Mote {
            at: Vec3::new(
                self.center.x + r * angle.cos(),
                ground + y,
                self.center.z + r * angle.sin(),
            ),
            fade: fade.clamp(0.0, 1.0),
            age: t,
        })
    }

    /// Tall soft streaks rising up the wall on a slow helix.
    fn wall(&self, out: &mut Vec<GlowVertex>, age: f32, eye: Vec3) {
        let half_height = 5.0;
        let span = self.height + 2.0 * half_height;
        for k in 0..STREAKS {
            let h = seeded(0x3a11, k);
            let speed = 2.2 + 1.6 * h[0];
            let s = (age * speed + h[1] * span).rem_euclid(span);
            let y = s - half_height;
            let angle =
                TAU * (k as f32 + 0.5 * h[2]) / STREAKS as f32 + 0.22 * age + 0.6 * y / self.height;
            let at = Vec3::new(
                self.center.x + self.radius * angle.cos(),
                self.center.y + y,
                self.center.z + self.radius * angle.sin(),
            );
            // Fades in from the ground and out past the top.
            let ends = (PI * s / span).sin();
            // Brighter where it climbs faster, so the column reads as moving.
            let radiance = VIOLET.map(|c| c * STREAK_LUMINANCE * ends * (0.7 + 0.3 * h[3]));
            let toward = flat(eye - at);
            let right = Vec3::Y.cross(toward).normalize_or(Vec3::X) * (0.45 + 0.35 * h[3]);
            quad(out, at, right, Vec3::Y * half_height, radiance);
        }
    }

    /// The shimmering rings at the base and the top.
    fn rings(&self, out: &mut Vec<GlowVertex>, age: f32, eye: Vec3) {
        for (y, phase) in [(0.25, 0.0), (self.height, 1.7)] {
            for k in 0..RING {
                let angle = TAU * k as f32 / RING as f32;
                let shimmer = (angle * 7.0 + age * 2.3 + phase).sin()
                    * (angle * 3.0 - age * 1.1 + phase).sin();
                let level = 0.2 + 0.8 * shimmer.abs();
                let at = Vec3::new(
                    self.center.x + self.radius * angle.cos(),
                    self.center.y + y,
                    self.center.z + self.radius * angle.sin(),
                );
                let radiance = VIOLET.map(|c| c * RING_LUMINANCE * level);
                blob(out, at, 0.7, radiance, eye);
            }
        }
    }
}

/// `v` on the ground plane, unit length.
fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or(Vec3::Z)
}

/// A round glow of half size `half` at `at`, turned toward `eye`.
fn blob(out: &mut Vec<GlowVertex>, at: Vec3, half: f32, radiance: [f32; 3], eye: Vec3) {
    // A glow brushing the camera would fill the view; it fades out instead.
    let near = ((eye.distance(at) - 1.0) / 3.0).clamp(0.0, 1.0);
    if near <= 0.0 {
        return;
    }
    let radiance = radiance.map(|c| c * near);
    let toward = (eye - at).normalize_or(Vec3::Z);
    let right = Vec3::Y.cross(toward).normalize_or(Vec3::X);
    let up = toward.cross(right);
    quad(out, at, right * half, up * half, radiance);
}

/// A glow quad centered at `at` spanning `right` and `up` each way.
fn quad(out: &mut Vec<GlowVertex>, at: Vec3, right: Vec3, up: Vec3, radiance: [f32; 3]) {
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        out.push(GlowVertex {
            pos: (at + right * x + up * y).to_array(),
            radiance,
            uv: [x, y],
        });
    }
}

/// A rough stone: an octahedron with uneven points, turned by `turn`.
fn pebble(mesh: &mut Mesh, at: Vec3, size: f32, turn: Quat, color: [f32; 3], h: [f32; 4]) {
    let points = [
        Vec3::X * (0.8 + 0.4 * h[0]),
        -Vec3::X * (0.8 + 0.4 * h[1]),
        Vec3::Y * (0.6 + 0.3 * h[2]),
        -Vec3::Y * (0.6 + 0.3 * h[3]),
        Vec3::Z * (0.7 + 0.4 * h[2]),
        -Vec3::Z * (0.7 + 0.4 * h[0]),
    ]
    .map(|p| at + turn * p * size);
    for (a, b, c) in [
        (0, 2, 4),
        (4, 2, 1),
        (1, 2, 5),
        (5, 2, 0),
        (0, 4, 3),
        (4, 1, 3),
        (1, 5, 3),
        (5, 0, 3),
    ] {
        face(mesh, [points[a], points[b], points[c]], color);
    }
}

/// A leaf: a pointed blade, bent a little along its midrib.
fn leaf(mesh: &mut Mesh, at: Vec3, size: f32, turn: Quat, color: [f32; 3]) {
    let p = |x: f32, y: f32, z: f32| at + turn * Vec3::new(x, y, z) * size;
    let (tip, stem) = (p(0.0, 0.0, 1.0), p(0.0, 0.0, -1.0));
    let (left, right) = (p(-0.45, 0.12, 0.0), p(0.45, 0.12, 0.0));
    face(mesh, [stem, left, tip], color);
    face(mesh, [stem, tip, right], color);
}

fn face(mesh: &mut Mesh, [a, b, c]: [Vec3; 3], color: [f32; 3]) {
    let color = super::super::draw::shade(color, a, b, c);
    for p in [a, b, c] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

/// Four deterministic values in [0, 1) from a family seed and an index
/// (`SplitMix64`).
fn seeded(seed: u64, i: usize) -> [f32; 4] {
    let mut x = seed ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut out = [0.0; 4];
    for o in &mut out {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        *o = (z >> 40) as f32 / (1u64 << 24) as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn cylinder() -> Cylinder {
        Cylinder::at(DVec3::new(2.0, 0.5, -3.0))
    }

    #[test]
    fn particles_lift_off_climb_inside_and_gather_under_the_top() {
        let field = Field::of(&cylinder());
        let (top, base) = (field.center.y + field.height, field.center.y);
        let mut near_top = 0;
        let mut seen = 0;
        for i in 0..DUST {
            for age in [0.5_f32, 4.0, 12.0, 40.0] {
                let Some(m) = field.mote(0x0d05, i, age, 2.0) else {
                    continue;
                };
                seen += 1;
                let off = Vec3::new(m.at.x - field.center.x, 0.0, m.at.z - field.center.z);
                assert!(off.length() <= field.radius, "{off}");
                assert!(m.at.y >= base && m.at.y <= top + 0.5, "{}", m.at.y);
                if age >= 12.0 && m.at.y > top - 3.0 {
                    near_top += 1;
                }
            }
        }
        assert!(seen > DUST * 3);
        // Half a cycle or more is spent hovering, so many gather up there.
        assert!(near_top > DUST / 3, "{near_top}");
    }

    #[test]
    fn the_same_age_draws_the_same_frame_and_the_count_is_bounded() {
        let eye = Vec3::new(0.0, 3.0, -8.0);
        let mut a = Mesh::default();
        let mut b = Mesh::default();
        draw(&mut a, &cylinder(), 17.25, eye);
        draw(&mut b, &cylinder(), 17.25, eye);
        assert_eq!(a.glow, b.glow);
        assert_eq!(a.faces, b.faces);
        let glows = (DUST + SPARKS + STREAKS + 2 * RING) * 6;
        assert!(a.glow.len() <= glows);
        assert!(a.faces.len() <= PEBBLES * 24 + LEAVES * 6);
        assert_eq!(PARTICLES, 320);
        assert!(a.glow.iter().all(|g| g.pos.iter().all(|x| x.is_finite())));
    }
}
