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
/// The shared computer stands five meters in front of the initial spawn.
pub const COMPUTER: Vec3 = Vec3::new(0.0, 0.0, -5.0);
/// Center of the monitor's front face, used to anchor native controls.
pub const COMPUTER_SCREEN: Vec3 = Vec3::new(0.0, 2.4, -5.16);
/// Maximum ground-plane distance at which the computer can be opened.
pub const COMPUTER_RANGE: f32 = 3.0;
/// Center of the Gym's walkable hall, east of the plaza.
pub const GYM_CENTER: Vec3 = Vec3::new(48.0, 0.0, 0.0);
/// The west doorway faces the plaza and has six meters of clear width.
pub const GYM_ENTRANCE: Vec3 = Vec3::new(36.0, 0.0, 0.0);
/// Center of the main bulletin board, facing the hall's entrance.
pub const GYM_BOARD: Vec3 = Vec3::new(58.8, 2.8, 0.0);
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
    computer(&mut world);
    gym(&mut world);
    pylon(&mut world);
    quest_board(&mut world);
    workbench(&mut world);
    oracle(&mut world);
    library(&mut world);
    proving_ground(&mut world.mesh);
    horizon(&mut world.mesh);
    world
}

fn computer(world: &mut World) {
    let c = COMPUTER;
    let mesh = &mut world.mesh;
    let block = |mesh: &mut Mesh, center: Vec3, size: Vec3, intensity| {
        mesh.cube(
            Mat4::from_translation(c + center) * Mat4::from_scale(size),
            intensity,
        );
    };
    // A desk, four legs, monitor and stand, keyboard, and mouse. All surfaces
    // share the world's hidden-line geometry and palette.
    block(
        mesh,
        Vec3::new(0.0, 1.35, 0.0),
        Vec3::new(3.6, 0.16, 1.6),
        Intensity::Half,
    );
    for x in [-1.55, 1.55] {
        for z in [-0.58, 0.58] {
            block(
                mesh,
                Vec3::new(x, 0.64, z),
                Vec3::new(0.13, 1.28, 0.13),
                Intensity::Quarter,
            );
        }
    }
    block(
        mesh,
        Vec3::new(0.0, 1.48, 0.15),
        Vec3::new(0.9, 0.1, 0.5),
        Intensity::Half,
    );
    block(
        mesh,
        Vec3::new(0.0, 1.77, 0.05),
        Vec3::new(0.16, 0.5, 0.16),
        Intensity::Half,
    );
    block(
        mesh,
        Vec3::new(0.0, 2.4, 0.0),
        Vec3::new(2.7, 1.6, 0.3),
        Intensity::Full,
    );
    let face = |x, y| COMPUTER_SCREEN + Vec3::new(x, y, 0.0);
    mesh.polyline_loop(
        &[
            face(-1.2, -0.65),
            face(1.2, -0.65),
            face(1.2, 0.65),
            face(-1.2, 0.65),
        ],
        Intensity::Full,
    );
    // The terminal prompt makes the front of the monitor recognizable.
    mesh.line(face(-0.95, 0.35), face(-0.7, 0.2), Intensity::Full);
    mesh.line(face(-0.7, 0.2), face(-0.95, 0.05), Intensity::Full);
    mesh.line(face(-0.5, 0.03), face(-0.1, 0.03), Intensity::Full);
    for y in [-0.2, -0.4] {
        mesh.line(face(-0.95, y), face(0.8, y), Intensity::Quarter);
    }
    block(
        mesh,
        Vec3::new(-0.25, 1.47, -0.5),
        Vec3::new(1.7, 0.08, 0.4),
        Intensity::Full,
    );
    for x in -3..=3 {
        let x = x as f32 * 0.2 - 0.25;
        mesh.line(
            c + Vec3::new(x, 1.516, -0.67),
            c + Vec3::new(x, 1.516, -0.33),
            Intensity::Half,
        );
    }
    for z in [-0.56, -0.44] {
        mesh.line(
            c + Vec3::new(-1.05, 1.516, z),
            c + Vec3::new(0.55, 1.516, z),
            Intensity::Half,
        );
    }
    block(
        mesh,
        Vec3::new(1.05, 1.5, -0.5),
        Vec3::new(0.27, 0.14, 0.4),
        Intensity::Half,
    );
    world.blockers.push(Footprint {
        min: [c.x - 1.8, c.z - 0.8],
        max: [c.x + 1.8, c.z + 0.8],
    });
}

fn gym(world: &mut World) {
    // Low solid walls keep the third-person camera usable. Upper posts and
    // beams define a hall without an opaque roof hiding the player.
    for (min, max) in [
        ([36.0, -9.0], [60.0, -8.5]),
        ([36.0, 8.5], [60.0, 9.0]),
        ([59.5, -8.5], [60.0, 8.5]),
        ([36.0, -8.5], [36.5, -3.0]),
        ([36.0, 3.0], [36.5, 8.5]),
    ] {
        let center = Vec3::new((min[0] + max[0]) / 2.0, 0.6, (min[1] + max[1]) / 2.0);
        world.mesh.cube(
            Mat4::from_translation(center)
                * Mat4::from_scale(Vec3::new(max[0] - min[0], 1.2, max[1] - min[1])),
            Intensity::Half,
        );
        world.blockers.push(Footprint { min, max });
    }
    let mesh = &mut world.mesh;
    for (x, z) in [
        (36.25, -8.75),
        (36.25, -3.25),
        (36.25, 3.25),
        (36.25, 8.75),
        (59.75, -8.75),
        (59.75, 8.75),
    ] {
        mesh.cube(
            Mat4::from_translation(Vec3::new(x, 3.0, z))
                * Mat4::from_scale(Vec3::new(0.35, 6.0, 0.35)),
            Intensity::ThreeQuarters,
        );
    }
    for y in [1.25, 6.0] {
        mesh.polyline_loop(
            &[
                Vec3::new(36.25, y, -8.75),
                Vec3::new(59.75, y, -8.75),
                Vec3::new(59.75, y, 8.75),
                Vec3::new(36.25, y, 8.75),
            ],
            Intensity::ThreeQuarters,
        );
    }
    // Threshold stripes mark the physical entrance without blocking it.
    for x in [35.0, 35.5, 36.0] {
        mesh.line(
            Vec3::new(x, 0.025, -2.8),
            Vec3::new(x, 0.025, 2.8),
            Intensity::Full,
        );
    }
    gym_sign(mesh);
    gym_boards(mesh);
}

fn gym_sign(mesh: &mut Mesh) {
    let sign = |u: f32, v: f32, depth: f32| Vec3::new(35.9 + depth, 6.4 + v * 1.8, u);
    // Extruded line strokes spell GYM on the west face. They are geometry,
    // independent of the desktop glyph atlas and the mobile native fonts.
    let letters: [(&[(f32, f32)], f32); 3] = [
        (
            &[
                (1.0, 1.0),
                (0.0, 1.0),
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0, 0.5),
                (0.5, 0.5),
            ],
            -3.8,
        ),
        (
            &[(0.0, 1.0), (0.5, 0.5), (1.0, 1.0), (0.5, 0.5), (0.5, 0.0)],
            -1.0,
        ),
        (
            &[(0.0, 0.0), (0.0, 1.0), (0.5, 0.4), (1.0, 1.0), (1.0, 0.0)],
            1.8,
        ),
    ];
    for (stroke, offset) in letters {
        for pair in stroke.windows(2) {
            for depth in [0.0, 0.18] {
                mesh.line(
                    sign(offset + pair[0].0 * 2.0, pair[0].1, depth),
                    sign(offset + pair[1].0 * 2.0, pair[1].1, depth),
                    Intensity::Full,
                );
            }
        }
        for &(u, v) in stroke {
            mesh.line(
                sign(offset + u * 2.0, v, 0.0),
                sign(offset + u * 2.0, v, 0.18),
                Intensity::Half,
            );
        }
    }
}

fn gym_boards(mesh: &mut Mesh) {
    let at = |u: f32, v: f32| GYM_BOARD + Vec3::new(0.0, v, u);
    let panel = |mesh: &mut Mesh, center: f32, half_width: f32| {
        let corners = [
            at(center - half_width, -1.55),
            at(center + half_width, -1.55),
            at(center + half_width, 1.55),
            at(center - half_width, 1.55),
        ];
        mesh.quad(corners);
        mesh.polyline_loop(&corners, Intensity::Full);
        mesh.line(
            at(center - half_width, 1.0),
            at(center + half_width, 1.0),
            Intensity::Half,
        );
    };
    // A central bulletin and two plot panels. Empty rows and axes denote
    // surfaces awaiting admitted data; they do not depict invented results.
    panel(mesh, 0.0, 2.5);
    for v in [-0.8, -0.2, 0.4] {
        mesh.line(at(-2.15, v), at(2.15, v), Intensity::Quarter);
    }
    for center in [-5.7, 5.7] {
        panel(mesh, center, 2.3);
        mesh.line(
            at(center - 1.85, -1.1),
            at(center + 1.8, -1.1),
            Intensity::Half,
        );
        mesh.line(
            at(center - 1.85, -1.1),
            at(center - 1.85, 0.65),
            Intensity::Half,
        );
        for i in 1..=4 {
            let u = center - 1.85 + i as f32 * 0.8;
            mesh.line(at(u, -1.15), at(u, -1.05), Intensity::Quarter);
        }
    }
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
    fn gym_hall_and_door_are_clear_and_signage_is_world_geometry() {
        let world = build();
        for point in [GYM_CENTER, GYM_ENTRANCE, Vec3::new(54.0, 0.0, 0.0)] {
            assert!(
                !world.blockers.iter().any(|wall| wall.contains(
                    point.x,
                    point.z,
                    crate::controller::RADIUS
                )),
                "{point:?}"
            );
        }
        assert!(
            world
                .blockers
                .iter()
                .any(|wall| wall.contains(36.25, 6.0, 0.0))
        );
        assert!(
            world
                .blockers
                .iter()
                .any(|wall| wall.contains(48.0, 8.75, 0.0))
        );
        let sign_vertices = world
            .mesh
            .lines
            .iter()
            .filter(|vertex| {
                (35.89..36.09).contains(&vertex.pos[0])
                    && (6.39..8.21).contains(&vertex.pos[1])
                    && vertex.pos[2].abs() < 4.0
            })
            .count();
        assert!(sign_vertices > 60, "the GYM sign has three extruded glyphs");
        let boards = world
            .mesh
            .faces
            .iter()
            .filter(|vertex| {
                (vertex.pos[0] - GYM_BOARD.x).abs() < 0.001
                    && vertex.pos[2].abs() < 8.1
                    && vertex.pos[1] > 1.2
            })
            .count();
        assert!(boards >= 18, "three physical board faces remain present");
    }

    #[test]
    fn the_computer_has_one_solid_footprint_and_clear_approach() {
        let world = build();
        assert_eq!(
            world
                .blockers
                .iter()
                .filter(|block| block.contains(COMPUTER.x, COMPUTER.z, 0.0))
                .count(),
            1
        );
        assert!(!world.blockers.iter().any(|block| block.contains(
            COMPUTER.x,
            COMPUTER.z - 2.0,
            crate::controller::RADIUS
        )));
        let screen_lines = world
            .mesh
            .lines
            .iter()
            .filter(|vertex| {
                (vertex.pos[2] - COMPUTER_SCREEN.z).abs() < 0.001
                    && (vertex.pos[1] - COMPUTER_SCREEN.y).abs() < 0.7
                    && vertex.pos[0].abs() < 1.3
            })
            .count();
        assert!(
            screen_lines >= 18,
            "monitor bezel and terminal prompt remain visible"
        );
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
