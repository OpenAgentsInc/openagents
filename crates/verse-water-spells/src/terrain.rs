//! The cove's ground: a sandy bay between two rocky headlands, a sea floor
//! that shelves away with a shallow reef, grassy hills behind the beach,
//! and in the west a plateau whose river falls over a cliff into a plunge
//! pool, then runs across the beach to the sea.
//!
//! [`ground`] is the one height function everything reads: the character's
//! feet, the floating bodies, the water's depth, and the drawn terrain.

use glam::{Mat4, Vec2, Vec3};
use verse_pbr::pbr::textured::{
    BaseColorImage, Primitive, TexturedMaterial, TexturedMesh, TexturedScene, TexturedVertex,
};

/// The sea's rest level, m.
pub const LEVEL: f32 = 0.0;

/// A stretch of river: centerline points (x, z) with the water surface's
/// height at each, the channel's half width, and its depth at the middle.
pub struct Reach {
    pub points: &'static [[f32; 3]],
    pub half_width: f32,
    pub depth: f32,
    /// Flow speed along the reach, m/s.
    pub speed: f32,
}

/// The river on the plateau, from the hills to the lip of the falls.
pub const UPPER: Reach = Reach {
    points: &[
        [-78.0, 102.0, 19.3],
        [-66.0, 84.0, 19.0],
        [-56.0, 70.0, 18.7],
        [-47.0, 57.4, 18.4],
    ],
    half_width: 2.6,
    depth: 0.9,
    speed: 1.3,
};

/// The stream from the plunge pool across the beach to the sea.
pub const LOWER: Reach = Reach {
    points: &[
        [-45.0, 47.5, 3.0],
        [-41.5, 38.0, 2.3],
        [-37.5, 26.0, 1.35],
        [-35.0, 13.0, 0.55],
        [-34.0, 0.0, 0.05],
        [-34.0, -6.0, 0.0],
    ],
    half_width: 2.0,
    depth: 0.55,
    speed: 0.9,
};

/// The plunge pool under the falls: center, radius, and surface height.
pub const POOL: [f32; 2] = [-46.0, 51.0];
pub const POOL_RADIUS: f32 = 5.6;
pub const POOL_LEVEL: f32 = 3.0;
/// Where the falls leave the lip, its heading, and its half width.
pub const LIP: Vec3 = Vec3::new(-46.9, 18.4, 56.4);
pub const FALL_HEADING: Vec2 = Vec2::new(0.08, -1.0);
pub const FALL_HALF_WIDTH: f32 = 2.2;
/// How fast the water leaves the lip, m/s.
pub const FALL_SPEED: f32 = 1.7;

/// The shallow reef in the bay: center, radius, and the depth of its top.
pub const REEF: [f32; 3] = [16.0, -30.0, 0.7];

/// The waterline of the bay's beach: z at each x, curving seaward toward
/// the headlands.
#[must_use]
pub fn shoreline(x: f32) -> f32 {
    (4.0 - 0.0095 * x * x).max(-70.0)
}

/// The cove's ground height at (x, z), m.
#[must_use]
pub fn ground(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let s = z - shoreline(x);
    let mut h = if s < 0.0 {
        let d = -s;
        let shelf = -(0.055 * d + 0.0017 * d * d).min(16.0);
        // Sand ripples and gentle undulation on the floor.
        let ripples =
            0.05 * (x * 1.1 + 0.6 * (z * 0.37).sin()).sin() * (0.5 + 0.5 * (z * 0.21).cos());
        shelf + ripples + 0.6 * (fbm(x * 0.03, z * 0.03, 3) - 0.5) * smoothstep(3.0, 20.0, d)
    } else {
        let beach = 0.05 * s.min(20.0) + 0.02 * (s - 20.0).max(0.0);
        let hills = smoothstep(14.0, 62.0, s) * (3.5 + 3.5 * fbm(x * 0.02 + 5.0, z * 0.02, 4))
            + smoothstep(50.0, 120.0, s) * 4.0;
        beach + hills
    };
    // The headlands rise straight from the water at both ends of the bay.
    let headland = smoothstep(30.0, 62.0, x.abs())
        * smoothstep(-9.0, 3.0, s)
        * (1.0 - smoothstep(25.0, 45.0, s));
    h += headland * (5.0 + 5.0 * fbm(x * 0.05, z * 0.05 + 3.0, 4));
    // The reef: a low mound under the bay whose top comes within a meter of
    // the surface.
    let reef = (Vec2::new(x, z) - Vec2::new(REEF[0], REEF[1])).length();
    if reef < 14.0 {
        let k = smoothstep(14.0, 2.0, reef);
        h = h.max(LEVEL - REEF[2] - 0.6 * (1.0 - k) * 4.0 + 0.4 * (fbm(x * 0.4, z * 0.4, 2) - 0.5));
    }
    // The plateau in the west, ending in a cliff over the pool.
    let edge = z - (56.0 + 0.12 * (x + 46.0));
    let west = 1.0 - smoothstep(-34.0, -22.0, x);
    let wall = smoothstep(-0.6, 0.9, edge) * west;
    if wall > 0.0 {
        let top = 19.6 + 1.6 * fbm(x * 0.05, z * 0.05 + 9.0, 3);
        h = h + (top.max(h) - h) * wall;
    }
    carve(x, z, h)
}

/// Lowers the ground into the river's channels, the pool's bowl, and the
/// banks around them.
fn carve(x: f32, z: f32, mut h: f32) -> f32 {
    let p = Vec2::new(x, z);
    for reach in [&UPPER, &LOWER] {
        if let Some((dist, surface, _)) = nearest(reach, p) {
            let w = reach.half_width;
            let target = if dist < w {
                let t = dist / w;
                surface - reach.depth * (1.0 - t * t)
            } else {
                surface + 0.12 + (dist - w) * 0.55
            };
            h = h.min(target);
        }
    }
    let r = p.distance(Vec2::from(POOL));
    // The bank rises gently toward the stream and the beach, and as a
    // cliff toward the falls.
    let toward = (p - Vec2::from(POOL)).normalize_or_zero();
    let lip = (Vec2::new(LIP.x, LIP.z) - Vec2::from(POOL)).normalize();
    let steep = 0.5 + 6.0 * smoothstep(0.1, 0.7, toward.dot(lip));
    let target = if r < POOL_RADIUS {
        let t = r / POOL_RADIUS;
        POOL_LEVEL - 2.6 * (1.0 - t * t).sqrt()
    } else {
        POOL_LEVEL + 0.15 + (r - POOL_RADIUS) * steep
    };
    h.min(target)
}

/// The nearest point of `reach`'s centerline to `p`: its distance, the
/// water's height there, and the unit direction of flow.
#[must_use]
pub fn nearest(reach: &Reach, p: Vec2) -> Option<(f32, f32, Vec2)> {
    let mut best: Option<(f32, f32, Vec2)> = None;
    for pair in reach.points.windows(2) {
        let a = Vec2::new(pair[0][0], pair[0][1]);
        let b = Vec2::new(pair[1][0], pair[1][1]);
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let d = p.distance(a + ab * t);
        if best.is_none_or(|(bd, _, _)| d < bd) {
            let y = pair[0][2] + (pair[1][2] - pair[0][2]) * t;
            best = Some((d, y, ab.normalize()));
        }
    }
    best
}

/// The surface height and flow of the river or pool at `p`, when `p` is
/// over one.
#[must_use]
pub fn fresh_water(p: Vec2) -> Option<(f32, Vec2)> {
    let r = p.distance(Vec2::from(POOL));
    if r < POOL_RADIUS + 0.5 {
        // The pool drains toward the stream's head.
        let out = (Vec2::new(LOWER.points[0][0], LOWER.points[0][1]) - p).normalize_or_zero();
        return Some((POOL_LEVEL, out * 0.35));
    }
    for reach in [&UPPER, &LOWER] {
        if let Some((d, y, dir)) = nearest(reach, p)
            && d < reach.half_width + 0.4
            && y > LEVEL + 0.02
        {
            return Some((y, dir * reach.speed));
        }
    }
    None
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash(ix: i32, iz: i32, salt: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x8da6_b343)
        ^ (iz as u32).wrapping_mul(0xd816_3841)
        ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_215.0
}

/// Smooth value noise in [0, 1].
#[must_use]
pub fn noise(x: f32, z: f32, salt: u32) -> f32 {
    let (ix, iz) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - x.floor(), z - z.floor());
    let (ux, uz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = hash(ix, iz, salt);
    let b = hash(ix + 1, iz, salt);
    let c = hash(ix, iz + 1, salt);
    let d = hash(ix + 1, iz + 1, salt);
    let top = a + (b - a) * ux;
    let bottom = c + (d - c) * ux;
    top + (bottom - top) * uz
}

/// Fractal value noise in [0, 1].
#[must_use]
pub fn fbm(x: f32, z: f32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
    let (mut px, mut pz) = (x, z);
    for o in 0..octaves {
        sum += amp * noise(px, pz, o + 1);
        norm += amp;
        amp *= 0.5;
        let (nx, nz) = (px * 1.6 - pz * 1.2, px * 1.2 + pz * 1.6);
        px = nx + 17.3;
        pz = nz + 9.1;
    }
    sum / norm
}

/// A coordinate across the zone that is dense near the bay and spreads
/// out toward the horizon: `u` in [-1, 1] to meters.
#[must_use]
pub fn warp(u: f32, reach: f32) -> f32 {
    let a = u.abs();
    u.signum() * reach * (0.45 * a + 0.55 * a * a * a)
}

/// The center of the warped grids: the middle of the bay.
pub const CENTER: [f32; 2] = [0.0, 10.0];
/// How far the ground and the sea reach from [`CENTER`], m.
pub const REACH: f32 = 300.0;

/// The ground's color at a point: wet and dry sand, grass, and rock by
/// height and slope, linear.
fn paint(p: Vec3, n: Vec3) -> [f32; 3] {
    let slope = 1.0 - n.y;
    let speck = fbm(p.x * 0.35, p.z * 0.35, 2);
    let wet = [0.36, 0.30, 0.22];
    let dry = [0.78, 0.68, 0.50];
    let under = [0.70, 0.64, 0.50];
    let grass = [0.20, 0.30, 0.09];
    let lush = [0.13, 0.24, 0.07];
    let rock = [0.34, 0.32, 0.30];
    let mix3 = |a: [f32; 3], b: [f32; 3], t: f32| {
        let t = t.clamp(0.0, 1.0);
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let above = p.y - LEVEL;
    let mut c = if above < -0.3 {
        mix3(under, wet, smoothstep(-2.0, -0.3, above) * 0.5)
    } else {
        mix3(wet, dry, smoothstep(0.05, 0.55, above))
    };
    // Grass starts above the beach, patchy at first.
    let green = smoothstep(1.3, 2.6, above + speck * 1.4);
    c = mix3(c, mix3(grass, lush, speck), green);
    // Steep ground and the headlands are rock.
    let stone = smoothstep(0.32, 0.55, slope + 0.15 * (speck - 0.5));
    c = mix3(c, mix3(rock, [0.45, 0.42, 0.38], speck), stone);
    // Banks of the river and pool are darker with wet earth.
    c.map(|v| v * (0.9 + 0.2 * speck))
}

fn to_byte(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A tiling grain for the ground's base color: soft speckle around white,
/// so the vertex colors carry the hue and the image the texture.
fn grain_image() -> BaseColorImage {
    let size = 256u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // Tileable: noise on a torus by wrapping the lattice.
            let f = |s: f32, salt: u32| {
                let px = x as f32 / size as f32 * s;
                let py = y as f32 / size as f32 * s;
                let (ix, iy) = (px.floor() as i32, py.floor() as i32);
                let (fx, fy) = (px.fract(), py.fract());
                let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
                let w = s as i32;
                let h = |i: i32, j: i32| hash(i.rem_euclid(w), j.rem_euclid(w), salt);
                let top = h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * ux;
                let bottom = h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * ux;
                top + (bottom - top) * uy
            };
            let v = 0.55 * f(16.0, 3) + 0.3 * f(64.0, 5) + 0.15 * f(128.0, 7);
            let g = 0.72 + 0.5 * v;
            let b = to_byte(g.min(1.0) * 255.0 / 255.0);
            rgba.extend_from_slice(&[b, b, b, 255]);
        }
    }
    BaseColorImage {
        name: "water-lab grain".into(),
        width: size,
        height: size,
        rgba,
    }
}

/// The cove's ground as textured meshes: warped tiles around the bay,
/// each its own placement so the renderer culls them.
pub fn add_terrain(scene: &mut TexturedScene) {
    let image = scene.add_image(grain_image());
    let material = scene.add_material(TexturedMaterial {
        image: Some(image),
        roughness: 0.92,
        ..TexturedMaterial::default()
    });
    // 256 cells across, in 8 × 8 tiles.
    let cells = 256usize;
    let tiles = 8usize;
    let per = cells / tiles;
    let at = |i: usize, j: usize| {
        let u = i as f32 / cells as f32 * 2.0 - 1.0;
        let v = j as f32 / cells as f32 * 2.0 - 1.0;
        let x = CENTER[0] + warp(u, REACH);
        let z = CENTER[1] + warp(v, REACH);
        Vec3::new(x, ground(x, z), z)
    };
    let normal_at = |p: Vec3| {
        let e = 0.35;
        let dx = ground(p.x + e, p.z) - ground(p.x - e, p.z);
        let dz = ground(p.x, p.z + e) - ground(p.x, p.z - e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    };
    for ti in 0..tiles {
        for tj in 0..tiles {
            let mut vertices = Vec::with_capacity((per + 1) * (per + 1));
            for j in 0..=per {
                for i in 0..=per {
                    let p = at(ti * per + i, tj * per + j);
                    let n = normal_at(p);
                    let c = paint(p, n);
                    let mut v = TexturedVertex::new(p, n, [p.x * 0.31, p.z * 0.31]);
                    v.color = [to_byte(c[0]), to_byte(c[1]), to_byte(c[2]), 255];
                    vertices.push(v);
                }
            }
            let mut indices = Vec::with_capacity(per * per * 6);
            let row = (per + 1) as u32;
            for j in 0..per as u32 {
                for i in 0..per as u32 {
                    let a = j * row + i;
                    indices.extend_from_slice(&[a, a + row, a + 1, a + 1, a + row, a + row + 1]);
                }
            }
            let mesh = scene.add_mesh(TexturedMesh {
                primitives: vec![Primitive {
                    vertices,
                    indices,
                    material,
                }],
            });
            scene.place(mesh, Mat4::IDENTITY);
        }
    }
}
