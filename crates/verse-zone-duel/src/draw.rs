//! The duel's geometry, physically lit under Everglade's daylight: the
//! meadow, its grass and far props, and the two fighters posed from
//! [`crate::rig`].
//!
//! Everything is simple shapes: tubes for limbs, boxes for the torso and
//! the blade, a cylinder for the great helm, a cone for the spangenhelm,
//! and each shield's own outline as a thin solid. Motion carries the
//! read, not detail.

use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec3};
use verse_pbr::mesh::Mesh;
use verse_pbr::pbr::{GlowVertex, LitVertex};

use crate::hits;
use crate::rig::Rig;
use crate::rules::{FLASH_LIFE, Fight, Fighter, Kit, Spark};

/// A surface: linear base color, metalness, roughness, and occlusion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub color: [f32; 3],
    pub metal: f32,
    pub rough: f32,
    pub ao: f32,
    /// The shader's material code (`verse_pbr::pbr::Material`).
    pub code: f32,
}

const fn paint(color: [f32; 3], metal: f32, rough: f32) -> Paint {
    Paint {
        color,
        metal,
        rough,
        ao: 1.0,
        code: 0.0,
    }
}

const fn cloth(color: [f32; 3]) -> Paint {
    Paint {
        color,
        metal: 0.0,
        rough: 0.92,
        ao: 1.0,
        code: 5.0,
    }
}

pub const STEEL: Paint = paint([0.56, 0.57, 0.59], 1.0, 0.3);
pub const MAIL: Paint = paint([0.36, 0.37, 0.39], 0.9, 0.62);
pub const DARK_STEEL: Paint = paint([0.2, 0.2, 0.21], 1.0, 0.5);
pub const BRASS: Paint = paint([0.72, 0.52, 0.22], 1.0, 0.38);
pub const LEATHER: Paint = paint([0.16, 0.08, 0.035], 0.0, 0.7);
pub const WOOD: Paint = paint([0.3, 0.18, 0.08], 0.0, 0.75);
pub const SLIT: Paint = paint([0.01, 0.01, 0.01], 0.0, 0.9);
pub const SURCOAT: Paint = cloth([0.76, 0.75, 0.7]);
pub const CROSS: Paint = cloth([0.025, 0.025, 0.028]);
pub const SKIN: Paint = paint([0.55, 0.34, 0.24], 0.0, 0.6);
pub const BEARD: Paint = cloth([0.13, 0.08, 0.04]);
pub const MADDER: Paint = cloth([0.34, 0.07, 0.04]);
pub const OCHRE: Paint = cloth([0.42, 0.3, 0.13]);
pub const WOOL: Paint = cloth([0.1, 0.09, 0.075]);
pub const WRAPS: Paint = cloth([0.46, 0.39, 0.27]);
pub const LINDEN: Paint = paint([0.52, 0.38, 0.2], 0.0, 0.72);
pub const SHIELD_PAINT: Paint = paint([0.36, 0.08, 0.05], 0.0, 0.7);
pub const HEATER_FACE: Paint = paint([0.78, 0.77, 0.72], 0.0, 0.66);

fn vertex(p: Vec3, n: Vec3, paint: Paint) -> LitVertex {
    let t = n.any_orthonormal_vector();
    LitVertex {
        pos: p.to_array(),
        local: p.to_array(),
        normal: n.to_array(),
        tangent: t.to_array(),
        color: paint.color,
        params: [paint.metal, paint.rough, paint.code, paint.ao],
    }
}

/// One flat triangle.
pub fn tri(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, c: Vec3, paint: Paint) {
    let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
    out.extend([
        vertex(a, n, paint),
        vertex(b, n, paint),
        vertex(c, n, paint),
    ]);
}

/// One triangle with a normal per corner.
fn tri_n(out: &mut Vec<LitVertex>, v: [(Vec3, Vec3); 3], paint: Paint) {
    out.extend(v.map(|(p, n)| vertex(p, n, paint)));
}

fn quad(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, c: Vec3, d: Vec3, paint: Paint) {
    tri(out, a, b, c, paint);
    tri(out, a, c, d, paint);
}

/// A box of half extents `half` at `center` turned by `rot`.
pub fn cuboid(out: &mut Vec<LitVertex>, center: Vec3, rot: Quat, half: Vec3, paint: Paint) {
    let c: [Vec3; 8] = std::array::from_fn(|i| {
        let x = if i & 1 == 0 { -half.x } else { half.x };
        let y = if i & 2 == 0 { -half.y } else { half.y };
        let z = if i & 4 == 0 { -half.z } else { half.z };
        center + rot * Vec3::new(x, y, z)
    });
    for [a, b, d, e] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        quad(out, c[a], c[b], c[d], c[e], paint);
    }
}

/// A box spanning `a` to `b` with half widths `w` (across) and `t`
/// (thickness), its width along `across`.
fn slab(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, across: Vec3, w: f32, t: f32, paint: Paint) {
    let axis = (b - a).normalize_or(Vec3::Y);
    let x = (across - axis * across.dot(axis)).normalize_or(axis.any_orthonormal_vector());
    let z = axis.cross(x);
    let rot = Quat::from_mat3(&glam::Mat3::from_cols(x, axis, z)).normalize();
    cuboid(
        out,
        (a + b) * 0.5,
        rot,
        Vec3::new(w, (b - a).length() * 0.5, t),
        paint,
    );
}

/// A tapered tube from `a` (radius `ra`) to `b` (radius `rb`) with
/// smooth sides and flat caps.
pub fn tube(
    out: &mut Vec<LitVertex>,
    a: Vec3,
    b: Vec3,
    ra: f32,
    rb: f32,
    sides: usize,
    paint: Paint,
) {
    let axis = (b - a).normalize_or(Vec3::Y);
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let ring = |i: usize| {
        let t = i as f32 / sides as f32 * TAU;
        u * t.cos() + v * t.sin()
    };
    let slope = (ra - rb) / (b - a).length().max(1e-4);
    for i in 0..sides {
        let (d0, d1) = (ring(i), ring(i + 1));
        let n0 = (d0 + axis * slope).normalize();
        let n1 = (d1 + axis * slope).normalize();
        let (p0, p1, q0, q1) = (a + d0 * ra, a + d1 * ra, b + d0 * rb, b + d1 * rb);
        tri_n(out, [(p0, n0), (p1, n1), (q1, n1)], paint);
        tri_n(out, [(p0, n0), (q1, n1), (q0, n0)], paint);
        if ra > 0.0 {
            tri(out, a, p1, p0, paint);
        }
        if rb > 0.0 {
            tri(out, b, q0, q1, paint);
        }
    }
}

/// A low sphere.
pub fn ball(out: &mut Vec<LitVertex>, c: Vec3, r: Vec3, rot: Quat, paint: Paint) {
    let (rings, segs) = (6, 10);
    let at = |i: usize, j: usize| {
        let th = i as f32 / rings as f32 * PI;
        let ph = j as f32 / segs as f32 * TAU;
        let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
        (c + rot * (n * r), rot * (n / r).normalize())
    };
    for i in 0..rings {
        for j in 0..segs {
            let (a, b, cc, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            tri_n(out, [a, b, cc], paint);
            tri_n(out, [a, cc, d], paint);
        }
    }
}

/// A flat solid of `outline` (in its own x and y) at `center`, turned by
/// `rot`, `half` thick along its local z; the front gets `face`, the rest
/// `rim`.
fn plate(
    out: &mut Vec<LitVertex>,
    outline: &[[f32; 2]],
    center: Vec3,
    rot: Quat,
    half: f32,
    face: &dyn Fn(usize) -> Paint,
    rim: Paint,
) {
    let p = |q: [f32; 2], z: f32| center + rot * Vec3::new(q[0], q[1], z);
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        tri(out, p([0.0, 0.0], half), p(a, half), p(b, half), face(i));
        tri(out, p([0.0, 0.0], -half), p(b, -half), p(a, -half), rim);
        quad(out, p(a, half), p(a, -half), p(b, -half), p(b, half), rim);
    }
}

// ---------------------------------------------------------------- world

fn hash(x: i32, z: i32, k: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (z as u32).wrapping_mul(0xD816_3841)
        ^ k.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65_535.0
}

fn smooth_noise(x: f32, z: f32, k: u32) -> f32 {
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - x.floor(), z - z.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sz) = (s(fx), s(fz));
    let a = hash(xi, zi, k);
    let b = hash(xi + 1, zi, k);
    let c = hash(xi, zi + 1, k);
    let d = hash(xi + 1, zi + 1, k);
    (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sz
}

/// The meadow's height at `(x, z)`, m: flat where they fight, rising in
/// long low swells beyond.
#[must_use]
pub fn height(x: f32, z: f32) -> f32 {
    let r = x.hypot(z);
    let rise = ((r - 18.0) / 30.0).clamp(0.0, 1.0);
    let swell =
        smooth_noise(x * 0.035, z * 0.035, 3) * 2.2 + smooth_noise(x * 0.09, z * 0.09, 4) * 0.5;
    rise * rise * swell
}

fn ground_color(x: f32, z: f32) -> [f32; 3] {
    let n = smooth_noise(x * 0.18, z * 0.18, 1);
    let m = smooth_noise(x * 0.6, z * 0.6, 2);
    // Everglade's late-summer meadow: deep green with dry, yellowing
    // patches; the ground they fight on is trampled paler.
    let green = Vec3::new(0.08, 0.16, 0.03);
    let dry = Vec3::new(0.22, 0.2, 0.07);
    let worn = Vec3::new(0.19, 0.17, 0.08);
    let mut c = green.lerp(dry, (n * 0.8 + m * 0.3 - 0.35).clamp(0.0, 0.7));
    let r = x.hypot(z);
    let trampled = (1.0 - ((r - 5.0) / 6.0).clamp(0.0, 1.0)) * (0.35 + 0.3 * m);
    c = c.lerp(worn, trampled);
    c.to_array()
}

fn ground(out: &mut Vec<LitVertex>) {
    let half = crate::HALF_EXTENT + 30.0;
    let cell = 1.5;
    let n = (half * 2.0 / cell) as i32;
    let at = |i: i32, j: i32| {
        let x = -half + i as f32 * cell;
        let z = -half + j as f32 * cell;
        Vec3::new(x, height(x, z), z)
    };
    let normal = |p: Vec3| {
        let e = 0.5;
        Vec3::new(
            height(p.x - e, p.z) - height(p.x + e, p.z),
            2.0 * e,
            height(p.x, p.z - e) - height(p.x, p.z + e),
        )
        .normalize()
    };
    for i in 0..n {
        for j in 0..n {
            let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let v = corners.map(|p| {
                let c = ground_color(p.x, p.z);
                let mut paint = paint(c, 0.0, 0.95);
                paint.ao = 0.9;
                vertex(p, normal(p), paint)
            });
            out.extend([v[0], v[2], v[1], v[0], v[3], v[2]]);
        }
    }
}

fn grass(out: &mut Vec<LitVertex>) {
    let radius = 30.0_f32;
    let step = 0.42_f32;
    let n = (radius / step) as i32;
    for i in -n..=n {
        for j in -n..=n {
            let jitter_x = hash(i, j, 11) - 0.5;
            let jitter_z = hash(i, j, 12) - 0.5;
            let x = (i as f32 + jitter_x) * step;
            let z = (j as f32 + jitter_z) * step;
            let r = x.hypot(z);
            if r > radius {
                continue;
            }
            // Sparse and short where the ground is trampled.
            let worn = (1.0 - ((r - 4.0) / 7.0).clamp(0.0, 1.0)).powi(2);
            if hash(i, j, 13) < worn * 0.75 {
                continue;
            }
            let tall = (0.16 + 0.32 * hash(i, j, 14)) * (1.0 - 0.6 * worn);
            let base = Vec3::new(x, height(x, z), z);
            let tint = hash(i, j, 15);
            let color = Vec3::new(0.1, 0.22, 0.04)
                .lerp(Vec3::new(0.3, 0.28, 0.09), (tint - 0.55).max(0.0) * 1.8)
                .to_array();
            for b in 0..3 {
                let a = hash(i, j, 20 + b) * TAU;
                let lean =
                    Vec3::new(a.cos(), 0.0, a.sin()) * tall * (0.25 + 0.3 * hash(i, j, 30 + b));
                let side = Vec3::new(-a.sin(), 0.0, a.cos()) * 0.018;
                let foot = base + Vec3::new(a.cos(), 0.0, a.sin()) * 0.04 * b as f32;
                let top = foot + Vec3::Y * tall + lean;
                // Lit as the meadow under it is, a little darker at the
                // root.
                let up = Vec3::Y;
                let mut root = paint(color.map(|c| c * 0.8), 0.0, 0.85);
                root.ao = 0.75;
                let mut tip = paint(color.map(|c| c * 1.25), 0.0, 0.8);
                tip.ao = 1.0;
                out.push(vertex(foot - side, up, root));
                out.push(vertex(foot + side, up, root));
                out.push(vertex(top, up, tip));
            }
        }
    }
}

fn pine(out: &mut Vec<LitVertex>, at: Vec3, tall: f32) {
    tube(out, at, at + Vec3::Y * tall * 0.4, 0.16, 0.12, 6, WOOD);
    let needles = paint([0.03, 0.07, 0.03], 0.0, 0.9);
    for k in 0..3 {
        let y = tall * (0.25 + 0.22 * k as f32);
        let r = tall * (0.24 - 0.06 * k as f32);
        tube(
            out,
            at + Vec3::Y * y,
            at + Vec3::Y * (y + tall * 0.42),
            r,
            0.0,
            8,
            needles,
        );
    }
}

fn birch(out: &mut Vec<LitVertex>, at: Vec3, tall: f32) {
    let bark = paint([0.7, 0.68, 0.62], 0.0, 0.7);
    tube(out, at, at + Vec3::Y * tall * 0.75, 0.11, 0.06, 6, bark);
    let leaves = paint([0.12, 0.2, 0.05], 0.0, 0.85);
    for (dx, dy, dz, r) in [
        (0.0, 0.72, 0.0, 0.3),
        (0.5, 0.62, 0.2, 0.22),
        (-0.4, 0.66, -0.3, 0.24),
        (0.1, 0.86, 0.1, 0.2),
    ] {
        ball(
            out,
            at + Vec3::new(dx, dy * tall, dz),
            Vec3::splat(r * tall),
            Quat::IDENTITY,
            leaves,
        );
    }
}

/// A line of sharpened stakes from `a` to `b` on the ground, with a gate
/// gap around `gate` along the line.
fn palisade(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, gate: f32) {
    let len = a.distance(b);
    let dir = (b - a) / len;
    let count = (len / 0.3) as usize;
    for i in 0..count {
        let s = i as f32 * 0.3;
        if (s - gate).abs() < 2.2 {
            continue;
        }
        let h = 2.6 + 0.35 * hash(i as i32, 7, 40);
        let p = a + dir * s;
        let p = Vec3::new(p.x, height(p.x, p.z) - 0.2, p.z);
        let log = paint(
            [0.27, 0.18, 0.1].map(|c| c * (0.85 + 0.3 * hash(i as i32, 2, 41))),
            0.0,
            0.85,
        );
        tube(out, p, p + Vec3::Y * h, 0.14, 0.13, 6, log);
        tube(
            out,
            p + Vec3::Y * h,
            p + Vec3::Y * (h + 0.35),
            0.13,
            0.0,
            6,
            log,
        );
    }
    // The gate's two tall posts and the beam across.
    let g = a + dir * gate;
    for s in [-2.3_f32, 2.3] {
        let p = g + dir * s;
        let p = Vec3::new(p.x, height(p.x, p.z) - 0.2, p.z);
        tube(out, p, p + Vec3::Y * 3.6, 0.2, 0.18, 6, WOOD);
    }
    let y = height(g.x, g.z) + 3.3;
    tube(
        out,
        (g - dir * 2.6).with_y(y),
        (g + dir * 2.6).with_y(y),
        0.16,
        0.16,
        6,
        WOOD,
    );
}

/// A wooden waymarker: a post with a small gabled cap and a cross arm
/// pointing the way.
fn waymarker(out: &mut Vec<LitVertex>, at: Vec3) {
    let at = at.with_y(height(at.x, at.z));
    tube(out, at, at + Vec3::Y * 1.7, 0.08, 0.07, 6, WOOD);
    let roof = paint([0.18, 0.12, 0.07], 0.0, 0.8);
    let top = at + Vec3::Y * 1.75;
    quad(
        out,
        top + Vec3::new(-0.16, 0.0, -0.18),
        top + Vec3::new(-0.16, 0.0, 0.18),
        top + Vec3::new(0.0, 0.14, 0.18),
        top + Vec3::new(0.0, 0.14, -0.18),
        roof,
    );
    quad(
        out,
        top + Vec3::new(0.16, 0.0, -0.18),
        top + Vec3::new(0.0, 0.14, -0.18),
        top + Vec3::new(0.0, 0.14, 0.18),
        top + Vec3::new(0.16, 0.0, 0.18),
        roof,
    );
    cuboid(
        out,
        at + Vec3::new(0.25, 1.35, 0.0),
        Quat::IDENTITY,
        Vec3::new(0.3, 0.06, 0.02),
        roof,
    );
}

fn stone(out: &mut Vec<LitVertex>, at: Vec3, r: f32) {
    let grey = paint([0.26, 0.25, 0.23], 0.0, 0.85);
    ball(
        out,
        at.with_y(height(at.x, at.z) + r * 0.25),
        Vec3::new(r, r * 0.55, r * 0.8),
        Quat::from_rotation_y(at.x * 0.7),
        grey,
    );
}

/// The meadow's static mesh: ground, grass, the palisade to the north,
/// a waymarker, stones, and trees on the swells.
#[must_use]
pub fn world() -> Mesh {
    let mut lit = Vec::new();
    ground(&mut lit);
    grass(&mut lit);
    palisade(
        &mut lit,
        Vec3::new(-34.0, 0.0, 38.0),
        Vec3::new(30.0, 0.0, 33.0),
        30.0,
    );
    waymarker(&mut lit, Vec3::new(5.5, 0.0, 9.0));
    for (x, z, r) in [
        (-7.0, 6.0, 0.35),
        (9.0, -5.0, 0.28),
        (-12.0, -9.0, 0.5),
        (14.0, 12.0, 0.42),
    ] {
        stone(&mut lit, Vec3::new(x, 0.0, z), r);
    }
    for i in 0..44 {
        let a = i as f32 / 44.0 * TAU + hash(i, 1, 50) * 0.12;
        // Clear of the palisade's side of the field.
        let r = 44.0 + 14.0 * hash(i, 2, 51);
        let at = Vec3::new(a.sin() * r, 0.0, a.cos() * r);
        let at = at.with_y(height(at.x, at.z) - 0.1);
        let tall = 7.0 + 5.0 * hash(i, 3, 52);
        if hash(i, 4, 53) < 0.65 {
            pine(&mut lit, at, tall);
        } else {
            birch(&mut lit, at, tall * 0.8);
        }
    }
    Mesh {
        lit,
        ..Mesh::default()
    }
}

// ------------------------------------------------------------- fighters

/// Brightens a paint for a hit's flash.
fn flashed(p: Paint, k: f32) -> Paint {
    Paint {
        color: p.color.map(|c| (c * (1.0 + k)).min(1.0)),
        ..p
    }
}

fn limb(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, ra: f32, rb: f32, paint: Paint) {
    tube(out, a, b, ra, rb, 8, paint);
    ball(out, b, Vec3::splat(rb), Quat::IDENTITY, paint);
}

/// One fighter at `rig`.
pub fn fighter(out: &mut Vec<LitVertex>, f: &Fighter, rig: &Rig, now: f32) {
    let k = (1.0 - (now - f.hit_at) / 0.12).clamp(0.0, 1.0) * 0.5;
    let p = |paint: Paint| flashed(paint, k);
    let up = (rig.neck - rig.pelvis).normalize_or(Vec3::Y);
    let right = rig.torso * Vec3::NEG_X;
    let fwd = rig.torso * Vec3::Z;
    let tq = rig.torso;
    let (body, arms_upper, arms_lower, legs_upper, legs_lower, feet) = match f.kit {
        Kit::Knight => (MAIL, MAIL, MAIL, MAIL, MAIL, MAIL),
        Kit::Prussian => (MAIL, MAIL, OCHRE, WOOL, WRAPS, LEATHER),
    };
    // Legs.
    for i in 0..2 {
        limb(out, rig.hips[i], rig.knees[i], 0.085, 0.07, p(legs_upper));
        limb(
            out,
            rig.knees[i],
            rig.feet[i] + Vec3::Y * 0.04,
            0.065,
            0.055,
            p(legs_lower),
        );
        let toe = rig.feet[i] + (rig.body * Vec3::Z) * 0.12;
        slab(
            out,
            rig.feet[i] - (rig.body * Vec3::Z) * 0.06,
            toe,
            right,
            0.05,
            0.04,
            p(feet),
        );
    }
    // The torso: padded body under mail, and the garment over it.
    let mid = (rig.pelvis + rig.neck) * 0.5;
    let tall = rig.pelvis.distance(rig.neck) * 0.5;
    cuboid(out, mid, tq, Vec3::new(0.18, tall, 0.12), p(body));
    match f.kit {
        Kit::Knight => {
            // The white surcoat, over the chest and falling to the knees,
            // with the Order's black cross front and back.
            cuboid(
                out,
                mid - up * 0.03,
                tq,
                Vec3::new(0.19, tall * 0.92, 0.13),
                p(SURCOAT),
            );
            let waist = rig.pelvis + up * 0.05;
            tube(out, waist, waist - up * 0.5, 0.21, 0.27, 10, p(SURCOAT));
            for side in [1.0_f32, -1.0] {
                let face = mid + fwd * 0.135 * side + up * 0.04;
                cuboid(out, face, tq, Vec3::new(0.03, 0.17, 0.006), p(CROSS));
                cuboid(
                    out,
                    face + up * 0.05,
                    tq,
                    Vec3::new(0.13, 0.03, 0.006),
                    p(CROSS),
                );
            }
            tube(
                out,
                waist + up * 0.02,
                waist + up * 0.06,
                0.205,
                0.205,
                10,
                p(LEATHER),
            );
        }
        Kit::Prussian => {
            // A padded coat under a short mail shirt, the coat's skirt
            // below it.
            let waist = rig.pelvis + up * 0.04;
            tube(
                out,
                waist + up * 0.1,
                waist - up * 0.36,
                0.2,
                0.25,
                10,
                p(MADDER),
            );
            cuboid(
                out,
                mid + up * 0.02,
                tq,
                Vec3::new(0.19, tall * 0.85, 0.13),
                p(MAIL),
            );
            tube(out, waist, waist + up * 0.05, 0.2, 0.2, 10, p(LEATHER));
        }
    }
    // Shoulders and arms.
    for i in 0..2 {
        ball(out, rig.shoulders[i], Vec3::splat(0.085), tq, p(arms_upper));
        limb(
            out,
            rig.shoulders[i],
            rig.elbows[i],
            0.065,
            0.055,
            p(arms_upper),
        );
        limb(
            out,
            rig.elbows[i],
            rig.hands[i],
            0.052,
            0.045,
            p(arms_lower),
        );
        let hand = match f.kit {
            Kit::Knight => MAIL,
            Kit::Prussian => SKIN,
        };
        ball(out, rig.hands[i], Vec3::splat(0.05), tq, p(hand));
    }
    // The neck and head.
    match f.kit {
        Kit::Knight => {
            tube(out, rig.neck, rig.head - up * 0.08, 0.075, 0.08, 8, p(MAIL));
            // The great helm: a flat-topped iron cylinder with a slit for
            // the eyes, breaths below, and a cross-shaped strap on the
            // face.
            let hq = rig.torso;
            let h = rig.head;
            tube(out, h - up * 0.16, h + up * 0.14, 0.135, 0.13, 12, p(STEEL));
            let face = hq * Vec3::Z;
            cuboid(
                out,
                h + up * 0.03 + face * 0.128,
                hq,
                Vec3::new(0.11, 0.012, 0.008),
                p(SLIT),
            );
            cuboid(
                out,
                h - up * 0.02 + face * 0.132,
                hq,
                Vec3::new(0.018, 0.12, 0.008),
                p(BRASS),
            );
            cuboid(
                out,
                h + up * 0.075 + face * 0.131,
                hq,
                Vec3::new(0.12, 0.014, 0.007),
                p(BRASS),
            );
            for row in 0..2 {
                for col in 0..3 {
                    let x = (col as f32 - 1.0) * 0.03 - 0.06;
                    let at = h - up * (0.07 + row as f32 * 0.03) + face * 0.13 + right * x;
                    cuboid(out, at, hq, Vec3::new(0.006, 0.006, 0.006), p(SLIT));
                }
            }
        }
        Kit::Prussian => {
            tube(out, rig.neck, rig.head - up * 0.06, 0.06, 0.065, 8, p(SKIN));
            let h = rig.head;
            let hq = rig.torso;
            let face = hq * Vec3::Z;
            ball(out, h - up * 0.01, Vec3::new(0.1, 0.12, 0.105), hq, p(SKIN));
            // A full beard.
            ball(
                out,
                h - up * 0.09 + face * 0.05,
                Vec3::new(0.085, 0.075, 0.06),
                hq,
                p(BEARD),
            );
            // The spangenhelm: riveted plates rising to a point over a
            // bronze browband, with a nasal down the face.
            tube(
                out,
                h + up * 0.03,
                h + up * 0.07,
                0.128,
                0.126,
                12,
                p(BRASS),
            );
            tube(
                out,
                h + up * 0.07,
                h + up * 0.25,
                0.126,
                0.0,
                12,
                p(DARK_STEEL),
            );
            cuboid(
                out,
                h - up * 0.01 + face * 0.122,
                hq,
                Vec3::new(0.012, 0.055, 0.008),
                p(DARK_STEEL),
            );
            // A mail curtain over the neck behind.
            tube(
                out,
                h + up * 0.03 - face * 0.03,
                h - up * 0.14 - face * 0.05,
                0.12,
                0.13,
                10,
                p(MAIL),
            );
        }
    }
    weapon(out, f.kit, rig);
    shield(out, f.kit, rig);
}

fn weapon(out: &mut Vec<LitVertex>, kit: Kit, rig: &Rig) {
    let d = rig.blade;
    let g = rig.grip;
    // The blade's flat faces across the body; its edge points along the
    // swing.
    let flat = d
        .cross(rig.torso * Vec3::Y)
        .normalize_or(d.any_orthonormal_vector());
    let edge = flat.cross(d).normalize();
    match kit {
        Kit::Knight => {
            tube(out, g - d * 0.09, g + d * 0.07, 0.016, 0.016, 6, LEATHER);
            ball(out, g - d * 0.1, Vec3::splat(0.028), Quat::IDENTITY, BRASS);
            slab(
                out,
                g + d * 0.07 - edge * 0.1,
                g + d * 0.07 + edge * 0.1,
                flat,
                0.012,
                0.012,
                STEEL,
            );
            // The blade, tapering to its point.
            let base = g + d * 0.09;
            let tip = g + d * rig.length;
            let w0 = edge * 0.024;
            let w1 = edge * 0.012;
            let t = flat * 0.004;
            let near = tip - d * 0.12;
            for s in [1.0_f32, -1.0] {
                quad(
                    out,
                    base - w0 + t * s,
                    base + w0 + t * s,
                    near + w1 + t * s,
                    near - w1 + t * s,
                    STEEL,
                );
                tri(out, near - w1 + t * s, near + w1 + t * s, tip, STEEL);
            }
            quad(
                out,
                base + w0 + t,
                base + w0 - t,
                near + w1 - t,
                near + w1 + t,
                STEEL,
            );
            quad(
                out,
                base - w0 + t,
                near - w1 + t,
                near - w1 - t,
                base - w0 - t,
                STEEL,
            );
        }
        Kit::Prussian => {
            let end = g + d * rig.length;
            tube(out, g - d * 0.1, end + d * 0.03, 0.017, 0.015, 6, WOOD);
            // The bearded head: a narrow neck at the haft and a long edge
            // whose beard hangs back toward the hand.
            let at = |along: f32, out_: f32| g + d * along + edge * out_;
            let outline = [
                at(rig.length - 0.05, 0.02),
                at(rig.length + 0.03, 0.02),
                at(rig.length + 0.05, 0.1),
                at(rig.length + 0.04, 0.17),
                at(rig.length - 0.16, 0.19),
                at(rig.length - 0.11, 0.11),
                at(rig.length - 0.06, 0.05),
            ];
            let t = flat * 0.008;
            let c = at(rig.length - 0.03, 0.09);
            for i in 0..outline.len() {
                let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                tri(out, c + t, a + t, b + t, STEEL);
                tri(out, c - t, b - t, a - t, STEEL);
                quad(out, a + t, a - t, b - t, b + t, DARK_STEEL);
            }
        }
    }
}

fn shield(out: &mut Vec<LitVertex>, kit: Kit, rig: &Rig) {
    let outline = hits::outline(kit);
    let half = hits::SHIELD_HALF_THICKNESS;
    let q = rig.shield_rot;
    let c = rig.shield_at;
    let face = q * Vec3::Z;
    match kit {
        Kit::Knight => {
            plate(out, &outline, c, q, half, &|_| HEATER_FACE, LEATHER);
            // The black cross on white.
            let f = c + face * (half + 0.004);
            cuboid(
                out,
                f + q * Vec3::new(0.0, 0.03, 0.0),
                q,
                Vec3::new(0.035, 0.29, 0.004),
                CROSS,
            );
            cuboid(
                out,
                f + q * Vec3::new(0.0, 0.12, 0.0),
                q,
                Vec3::new(0.23, 0.035, 0.004),
                CROSS,
            );
        }
        Kit::Prussian => {
            // Painted in alternating boards, a leather rim, an iron boss.
            plate(
                out,
                &outline,
                c,
                q,
                half,
                &|i| {
                    if (i / 5) % 2 == 0 {
                        SHIELD_PAINT
                    } else {
                        LINDEN
                    }
                },
                LEATHER,
            );
            ball(
                out,
                c + face * half,
                Vec3::new(0.075, 0.075, 0.06),
                q,
                DARK_STEEL,
            );
        }
    }
}

/// A flash where a blow met something: a bright star that fades.
fn flash(
    out: &mut Vec<GlowVertex>,
    at: Vec3,
    eye_side: Vec3,
    kind: Spark,
    strength: f32,
    age: f32,
) {
    let t = (age / FLASH_LIFE).clamp(0.0, 1.0);
    let fade = (1.0 - t) * (1.0 - t);
    let (color, size) = match kind {
        Spark::Steel => ([1.0, 0.86, 0.6], 0.14),
        Spark::Wood => ([0.95, 0.75, 0.45], 0.12),
        Spark::Flesh => ([1.0, 0.55, 0.4], 0.12),
    };
    let radiance = color.map(|c| c * 60_000.0 * strength * fade);
    let up = Vec3::Y;
    let side = eye_side.cross(up).normalize_or(Vec3::X);
    let r = size * (0.6 + 0.8 * t) * strength.max(0.5);
    let corner = |u: f32, v: f32| GlowVertex {
        pos: (at + side * u * r + up * v * r).to_array(),
        radiance,
        uv: [u, v],
    };
    let (a, b, c, d) = (
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    );
    out.extend([a, b, c, a, c, d]);
}

/// Everything that moves: both fighters and the flashes.
#[must_use]
pub fn dynamic(fight: &Fight) -> Mesh {
    let mut mesh = Mesh::default();
    for (f, rig) in fight.fighters.iter().zip(&fight.rigs) {
        fighter(&mut mesh.lit, f, rig, fight.now);
    }
    // Flashes face along the line between the fighters, which the camera
    // looks down.
    let across = (fight.enemy().pos - fight.player().pos).normalize_or(Vec3::Z);
    for f in &fight.flashes {
        flash(
            &mut mesh.glow,
            f.at,
            across,
            f.kind,
            f.strength,
            fight.now - f.born,
        );
    }
    mesh
}
