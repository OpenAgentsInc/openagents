//! The first Verse world: a line grid, a city of amber towers, and a pylon.
//!
//! Everything is generated from a fixed seed, so every launch builds the
//! same city. The player spawns on an open plaza facing the pylon.

use coder_ui::theme::Intensity;
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
/// Where the quest board stands, facing the plaza.
pub const QUEST_BOARD: Vec3 = Vec3::new(-22.0, 0.0, 0.0);
/// The plaza's center, where a replay's agents start and finish.
pub const PLAZA: Vec3 = Vec3::new(0.0, 0.0, -2.0);
/// The workbench, where an agent's model steps and commands run.
pub const WORKBENCH: Vec3 = Vec3::new(18.0, 0.0, 0.0);
/// The oracle, a door where an agent asks Jev a typed question.
pub const ORACLE: Vec3 = Vec3::new(30.0, 0.0, 28.0);
/// The library, where an agent retrieves and reads knowledge entries.
pub const LIBRARY: Vec3 = Vec3::new(-30.0, 0.0, 28.0);
/// The proving ground, where acceptance tests and the verifier check work.
pub const PROVING_GROUND: Vec3 = Vec3::new(0.0, 0.0, 44.0);
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
    quest_board(&mut world);
    workbench(&mut world);
    oracle(&mut world);
    library(&mut world);
    proving_ground(&mut world.mesh);
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

/// The quest board: a line-drawn notice board on two posts, facing the
/// plaza center, with rows of "notices" and a ring on the ground in front.
/// Press `B` near it to read the live NIP-XP quests.
fn quest_board(world: &mut World) {
    let mesh = &mut world.mesh;
    let at = |z: f32, y: f32| QUEST_BOARD + Vec3::new(0.0, y, z);
    let (half, low, high) = (3.2, 1.3, 4.4);
    for z in [-half, half] {
        mesh.line(at(z, 0.0), at(z, high + 0.6), Intensity::ThreeQuarters);
    }
    let corners = [
        at(-half, low),
        at(half, low),
        at(half, high),
        at(-half, high),
    ];
    mesh.quad(corners);
    mesh.polyline_loop(&corners, Intensity::Full);
    // A header bar and the notices under it.
    mesh.line(
        at(-half + 0.3, high - 0.45),
        at(half - 0.3, high - 0.45),
        Intensity::Full,
    );
    let lengths = [5.4, 4.1, 4.8, 3.2, 5.0, 2.6];
    for (i, len) in lengths.iter().enumerate() {
        let y = high - 0.85 - i as f32 * 0.42;
        let step = if i % 2 == 0 {
            Intensity::ThreeQuarters
        } else {
            Intensity::Half
        };
        mesh.line(at(-half + 0.4, y), at(-half + 0.4 + len, y), step);
    }
    mesh.ring(
        QUEST_BOARD + Vec3::new(3.0, 0.0, 0.0),
        2.0,
        40,
        Intensity::Half,
    );
    world.blockers.push(Footprint {
        min: [QUEST_BOARD.x - 0.3, QUEST_BOARD.z - half],
        max: [QUEST_BOARD.x + 0.3, QUEST_BOARD.z + half],
    });
}

/// The workbench: a line-drawn table with a lamp, where a replay's model
/// steps and commands happen.
fn workbench(world: &mut World) {
    let mesh = &mut world.mesh;
    let at = |x: f32, y: f32, z: f32| WORKBENCH + Vec3::new(x, y, z);
    let (hx, hz, top) = (1.8, 0.8, 1.0);
    mesh.cube(
        Mat4::from_translation(at(0.0, top, 0.0))
            * Mat4::from_scale(Vec3::new(2.0 * hx, 0.12, 2.0 * hz)),
        Intensity::ThreeQuarters,
    );
    for (x, z) in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)] {
        let (x, z) = (x * 0.9, z * 0.8);
        mesh.line(at(x, 0.0, z), at(x, top - 0.06, z), Intensity::Half);
    }
    // A lamp arm over the bench, and a small block on it.
    mesh.line(
        at(hx - 0.3, top, hz - 0.2),
        at(hx - 0.3, top + 1.4, hz - 0.2),
        Intensity::Half,
    );
    mesh.line(
        at(hx - 0.3, top + 1.4, hz - 0.2),
        at(0.4, top + 1.7, 0.0),
        Intensity::Half,
    );
    mesh.cube(
        Mat4::from_translation(at(-0.6, top + 0.26, 0.0))
            * Mat4::from_scale(Vec3::new(0.7, 0.4, 0.5)),
        Intensity::Full,
    );
    mesh.ring(WORKBENCH, 3.0, 40, Intensity::Half);
    world.blockers.push(Footprint {
        min: [WORKBENCH.x - hx, WORKBENCH.z - hz],
        max: [WORKBENCH.x + hx, WORKBENCH.z + hz],
    });
}

/// The oracle: a door facing the plaza, with a diamond over its lintel.
/// Which door an agent visits would show which decision door it chose;
/// today every replayed question goes to Jev.
fn oracle(world: &mut World) {
    let mesh = &mut world.mesh;
    let facing = (PLAZA - ORACLE).with_y(0.0).normalize();
    let side = facing.cross(Vec3::Y);
    let at = |s: f32, y: f32| ORACLE + side * s + Vec3::Y * y;
    let (outer, inner, high) = (1.6, 1.1, 4.6);
    mesh.polyline_loop(
        &[
            at(-outer, 0.0),
            at(-outer, high),
            at(outer, high),
            at(outer, 0.0),
        ],
        Intensity::Full,
    );
    mesh.polyline_loop(
        &[
            at(-inner, 0.0),
            at(-inner, high - 0.5),
            at(inner, high - 0.5),
            at(inner, 0.0),
        ],
        Intensity::Half,
    );
    mesh.quad([
        at(-inner, 0.0),
        at(-inner, high - 0.5),
        at(inner, high - 0.5),
        at(inner, 0.0),
    ]);
    let top = high + 1.0;
    mesh.polyline_loop(
        &[
            at(0.0, top - 0.6),
            at(0.5, top),
            at(0.0, top + 0.6),
            at(-0.5, top),
        ],
        Intensity::Full,
    );
    mesh.line(at(0.0, high), at(0.0, top - 0.6), Intensity::Half);
    mesh.ring(ORACLE + facing * 3.0, 2.2, 40, Intensity::Half);
    for s in [-outer, outer] {
        let p = ORACLE + side * s;
        world.blockers.push(Footprint {
            min: [p.x - 0.25, p.z - 0.25],
            max: [p.x + 0.25, p.z + 0.25],
        });
    }
}

/// The library: a low hall whose face toward the spawn side (-Z) is rows
/// of shelves.
fn library(world: &mut World) {
    let mesh = &mut world.mesh;
    let facing = Vec3::NEG_Z;
    let side = facing.cross(Vec3::Y);
    let (w, d, h) = (9.0, 3.0, 4.2);
    let yaw = side.z.atan2(side.x);
    mesh.cube(
        Mat4::from_translation(LIBRARY + Vec3::Y * (h / 2.0))
            * Mat4::from_rotation_y(-yaw)
            * Mat4::from_scale(Vec3::new(w, h, d)),
        Intensity::ThreeQuarters,
    );
    let face = |s: f32, y: f32| LIBRARY + facing * (d / 2.0 + 0.02) + side * s + Vec3::Y * y;
    for y in [1.0, 2.0, 3.0] {
        mesh.line(
            face(-w / 2.0 + 0.3, y),
            face(w / 2.0 - 0.3, y),
            Intensity::Half,
        );
    }
    // Book spines on each shelf, of uneven heights.
    for (row, y) in [0.1f32, 1.1, 2.1].iter().enumerate() {
        let mut s = -w / 2.0 + 0.5;
        let mut i = row;
        while s < w / 2.0 - 0.5 {
            let tall = 0.5 + ((i * 7) % 5) as f32 * 0.08;
            mesh.line(face(s, *y), face(s, y + tall), Intensity::Quarter);
            s += 0.35 + ((i * 3) % 4) as f32 * 0.05;
            i += 1;
        }
    }
    let roof = [-1.0, 1.0].map(|f: f32| {
        [-1.0, 1.0]
            .map(|g: f32| LIBRARY + facing * (f * d / 2.0) + side * (g * w / 2.0) + Vec3::Y * h)
    });
    mesh.polyline_loop(
        &[roof[0][0], roof[0][1], roof[1][1], roof[1][0]],
        Intensity::Full,
    );
    mesh.ring(LIBRARY + facing * 4.0, 2.2, 40, Intensity::Half);
    let corners: Vec<Vec3> = roof.iter().flatten().copied().collect();
    let (min_x, max_x) = corners
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (min_z, max_z) = corners
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
    world.blockers.push(Footprint {
        min: [min_x, min_z],
        max: [max_x, max_z],
    });
}

/// The proving ground: an open ring of posts where work is checked.
fn proving_ground(mesh: &mut Mesh) {
    let c = PROVING_GROUND;
    mesh.ring(c, 7.0, 72, Intensity::Half);
    mesh.ring(c, 4.5, 56, Intensity::ThreeQuarters);
    mesh.ring(c + Vec3::Y * 1.6, 7.0, 72, Intensity::Quarter);
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let p = c + Vec3::new(a.cos() * 7.0, 0.0, a.sin() * 7.0);
        mesh.line(p, p + Vec3::Y * 1.6, Intensity::Full);
    }
    // A target mark in the middle.
    mesh.line(
        c + Vec3::new(-1.0, 0.02, 0.0),
        c + Vec3::new(1.0, 0.02, 0.0),
        Intensity::Full,
    );
    mesh.line(
        c + Vec3::new(0.0, 0.02, -1.0),
        c + Vec3::new(0.0, 0.02, 1.0),
        Intensity::Full,
    );
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
    fn the_quest_board_stands_on_the_open_plaza() {
        let world = build();
        let board = world
            .blockers
            .iter()
            .filter(|b| b.contains(QUEST_BOARD.x, QUEST_BOARD.z, 0.0))
            .count();
        assert_eq!(board, 1, "only the board's own footprint is there");
        assert!(!world.blockers.iter().any(|b| b.contains(
            QUEST_BOARD.x + 3.0,
            QUEST_BOARD.z,
            1.0
        )));
    }

    #[test]
    fn the_replay_landmarks_stand_clear_of_the_city() {
        let world = build();
        let city = build_city_only();
        for place in [WORKBENCH, ORACLE, LIBRARY, PROVING_GROUND, PLAZA] {
            for block in &city {
                assert!(!block.contains(place.x, place.z, 8.0), "{place} {block:?}");
            }
        }
        for block in &world.blockers {
            assert!(
                !block.contains(PLAZA.x, PLAZA.z, 2.0),
                "the plaza stays open"
            );
            assert!(!block.contains(SPAWN.x, SPAWN.z, 2.0));
        }
    }

    fn build_city_only() -> Vec<Footprint> {
        let mut world = World::default();
        city(&mut world);
        world.blockers
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
