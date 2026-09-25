//! The first Verse world: a line grid, a city of amber towers, and a pylon.
//!
//! Everything is generated from a fixed seed, so every launch builds the
//! same city. The player spawns on an open plaza facing the pylon.

use coder_terminal::Intensity;
use glam::{Mat4, Vec3};

use crate::controller::Footprint;
use crate::mesh::Mesh;

/// Half the width of the walkable square, in meters.
pub const HALF: f32 = 264.0;
/// Width of one city lot, in meters.
pub const LOT: f32 = 24.0;
/// Spacing of the fine ground grid, in meters.
pub const GRID: f32 = 4.0;
/// Where the player starts.
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -10.0);
/// Where the pylon stands.
pub const PYLON: Vec3 = Vec3::new(0.0, 0.0, 14.0);
/// Distance of the horizon ridge, in meters.
pub const HORIZON: f32 = 900.0;

const SEED: u64 = 0x5eed_7a55_ad0a;
const LOTS: i32 = 10;

/// The static world: its geometry and what the player cannot walk through.
#[derive(Clone, Debug, Default)]
pub struct World {
    /// Lines and faces uploaded once.
    pub mesh: Mesh,
    /// Building and pylon footprints.
    pub blockers: Vec<Footprint>,
}

/// Builds the world.
#[must_use]
pub fn build() -> World {
    let mut world = World::default();
    ground(&mut world.mesh);
    city(&mut world);
    pylon(&mut world);
    horizon(&mut world.mesh);
    world
}

fn ground(mesh: &mut Mesh) {
    let n = (HALF / GRID) as i32;
    for i in -n..=n {
        let v = i as f32 * GRID;
        let street = ((v - LOT / 2.0) / LOT).fract().abs() < 1e-4;
        let step = if street {
            Intensity::Half
        } else {
            Intensity::Quarter
        };
        mesh.line(Vec3::new(v, 0.0, -HALF), Vec3::new(v, 0.0, HALF), step);
        mesh.line(Vec3::new(-HALF, 0.0, v), Vec3::new(HALF, 0.0, v), step);
    }
}

fn city(world: &mut World) {
    for i in -LOTS..=LOTS {
        for j in -LOTS..=LOTS {
            if i.abs() <= 2 && j.abs() <= 2 {
                continue;
            }
            let mut rng = Rng::new(SEED ^ ((i as u64) << 32) ^ (j as u64 & 0xffff_ffff));
            if rng.next() < 0.35 {
                continue;
            }
            let center = Vec3::new(i as f32 * LOT, 0.0, j as f32 * LOT);
            let w = 8.0 + rng.next() * 10.0;
            let d = 8.0 + rng.next() * 10.0;
            let near = 1.0 - (center.length() / (LOTS as f32 * LOT)).min(1.0);
            let h = 6.0 + rng.next().powi(2) * 50.0 + near * 30.0;
            building(world, center, w, d, h, &mut rng);
        }
    }
}

fn building(world: &mut World, center: Vec3, w: f32, d: f32, h: f32, rng: &mut Rng) {
    let mesh = &mut world.mesh;
    let at = |y: f32| center + Vec3::Y * y;
    let transform = Mat4::from_translation(at(h / 2.0)) * Mat4::from_scale(Vec3::new(w, h, d));
    mesh.cube(transform, Intensity::ThreeQuarters);

    let (x0, x1) = (center.x - w / 2.0, center.x + w / 2.0);
    let (z0, z1) = (center.z - d / 2.0, center.z + d / 2.0);
    let rect = |y: f32| {
        [
            Vec3::new(x0, y, z0),
            Vec3::new(x1, y, z0),
            Vec3::new(x1, y, z1),
            Vec3::new(x0, y, z1),
        ]
    };
    let mut y = 4.0;
    while y < h - 1.0 {
        mesh.polyline_loop(&rect(y), Intensity::Quarter);
        y += 4.0;
    }
    mesh.polyline_loop(&rect(h), Intensity::Full);

    if rng.next() < 0.4 {
        let mast = 3.0 + rng.next() * 8.0;
        mesh.line(at(h), at(h + mast), Intensity::Full);
    }

    world.blockers.push(Footprint {
        min: [x0, z0],
        max: [x1, z1],
    });
}

fn pylon(world: &mut World) {
    let mesh = &mut world.mesh;
    let base = 1.6;
    let height = 22.0;
    let top = PYLON + Vec3::Y * height;
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, z)| PYLON + Vec3::new(x * base, 0.0, z * base));
    for (i, &c) in corners.iter().enumerate() {
        let next = corners[(i + 1) % 4];
        mesh.quad([c, next, top, top]);
        mesh.line(c, next, Intensity::Full);
        mesh.line(c, top, Intensity::Full);
    }
    for r in [4.0, 7.0, 10.0] {
        mesh.ring(PYLON, r, 64, Intensity::Half);
    }
    mesh.line(top, top + Vec3::Y * 40.0, Intensity::Half);
    world.blockers.push(Footprint {
        min: [PYLON.x - base, PYLON.z - base],
        max: [PYLON.x + base, PYLON.z + base],
    });
}

fn horizon(mesh: &mut Mesh) {
    let mut rng = Rng::new(SEED.rotate_left(17));
    let segments = 180;
    let points: Vec<Vec3> = (0..segments)
        .map(|i| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let peak = if i % 3 == 0 {
                30.0 + rng.next() * 90.0
            } else {
                rng.next() * 25.0
            };
            Vec3::new(a.cos() * HORIZON, peak, a.sin() * HORIZON)
        })
        .collect();
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        mesh.line_with_fog(a, b, Intensity::Half, 0.0);
    }
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let at = |t: f32| Vec3::new(t.cos() * HORIZON, 0.0, t.sin() * HORIZON);
        mesh.line_with_fog(at(a), at(b), Intensity::Quarter, 0.0);
    }
}

/// SplitMix64: a small deterministic generator for city layout.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// A uniform value in `[0, 1)`.
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette;

    #[test]
    fn the_city_is_the_same_every_launch() {
        let a = build();
        let b = build();
        assert_eq!(a.blockers, b.blockers);
        assert_eq!(a.mesh.lines, b.mesh.lines);
    }

    #[test]
    fn the_city_has_buildings() {
        assert!(build().blockers.len() > 100);
    }

    #[test]
    fn the_spawn_is_clear() {
        let world = build();
        for block in &world.blockers {
            assert!(!block.contains(SPAWN.x, SPAWN.z, 2.0), "{block:?}");
        }
    }

    #[test]
    fn every_color_is_on_the_amber_ladder() {
        let world = build();
        let mut allowed: Vec<[f32; 3]> =
            Intensity::ALL.iter().map(|&s| palette::amber(s)).collect();
        allowed.push(palette::field());
        for v in world.mesh.lines.iter().chain(&world.mesh.faces) {
            assert!(
                allowed.contains(&v.color),
                "off-palette color {:?}",
                v.color
            );
        }
    }

    #[test]
    fn faces_are_near_black() {
        let world = build();
        assert!(world.mesh.faces.iter().all(|v| v.color == palette::field()));
    }
}
