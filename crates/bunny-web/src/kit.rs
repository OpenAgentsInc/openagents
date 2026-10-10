//! The game's kits, made in code: every obstacle, the power-ups, and the
//! meadow's pieces (`docs/verse/games/grow-little-bunny.md`, Assets).
//!
//! Gray things take their fill from the spec's 8-step ramp, paper to
//! charcoal; only the power-ups are in colour. Models stand on `y = 0`,
//! face `+z`, and are in metres; obstacles fill one 1.2 m lane.

use bunny_rules::{LANE_WIDTH, ObstacleKind, PowerKind};
use glam::{Mat4, Quat, Vec3};

use crate::mesh::{Mesh, Shape};

/// The gray ramp, `#F4F4F2` (paper) to `#3A3A3A`.
pub const RAMP: [u32; 8] = [
    0xF4F4F2, 0xDCDCD9, 0xC4C4C0, 0xACACA8, 0x949490, 0x7C7C78, 0x5A5A57, 0x3A3A3A,
];

const LEAF: u32 = 0x3FA34D;
const CLOVER: u32 = 0x2FB15A;
const PUFF: u32 = 0xFFF3C8;
const STRAW: u32 = 0xF2CF55;
const MAGNET: u32 = 0xB02CC0;
const STEEL: u32 = 0xD8E4F0;

fn at(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn part(scale: Vec3, rotation: Quat, centre: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(scale, rotation, centre)
}

fn upright(size: Vec3, centre: Vec3) -> Mat4 {
    part(size, Quat::IDENTITY, centre)
}

const SPHERE: Shape = Shape::Sphere {
    rings: 5,
    segments: 8,
};

fn cylinder(sides: u32) -> Shape {
    Shape::Frustum {
        sides,
        bottom: 0.5,
        top: 0.5,
    }
}

fn cone(sides: u32) -> Shape {
    Shape::Frustum {
        sides,
        bottom: 0.5,
        top: 0.0,
    }
}

/// A pole from `a` to `b`, `thick` across.
fn pole(mesh: &mut Mesh, a: Vec3, b: Vec3, thick: f32, colour: u32, weight: f32) {
    let d = b - a;
    let rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize());
    mesh.add(
        cylinder(5),
        part(Vec3::new(thick, d.length(), thick), rotation, (a + b) * 0.5),
        colour,
        weight,
    );
}

fn fence_posts(mesh: &mut Mesh, height: f32) {
    let lane = LANE_WIDTH as f32 / bunny_rules::UNIT as f32;
    for side in [-1.0_f32, 1.0] {
        mesh.block(
            at(side * lane * 0.46 - 0.06, 0.0, -0.06),
            at(side * lane * 0.46 + 0.06, height, 0.06),
            RAMP[2],
            1.0,
        );
    }
}

/// An obstacle's model.
#[must_use]
pub fn obstacle(kind: ObstacleKind) -> Mesh {
    let lane = LANE_WIDTH as f32 / bunny_rules::UNIT as f32;
    let none = Quat::IDENTITY;
    let mut mesh = Mesh::new();
    match kind {
        ObstacleKind::Fence | ObstacleKind::Gap => {
            fence_posts(&mut mesh, 1.15);
            let boards: &[f32] = if kind == ObstacleKind::Fence {
                &[0.22, 0.58, 0.94]
            } else {
                &[0.6, 0.94]
            };
            for y in boards {
                mesh.block(
                    at(-lane * 0.5, y - 0.1, -0.03),
                    at(lane * 0.5, y + 0.1, 0.03),
                    RAMP[if kind == ObstacleKind::Fence { 2 } else { 3 }],
                    1.0,
                );
            }
            if kind == ObstacleKind::Gap {
                // The dug-out hollow under the boards.
                mesh.add(
                    Shape::Disc { sides: 10 },
                    part(at(0.7, 1.0, 0.5), none, at(0.0, 0.004, 0.0)),
                    RAMP[4],
                    0.0,
                );
            }
        }
        ObstacleKind::Tunnel => {
            // Bean poles leaning together over a low passage.
            for z in [-0.45_f32, -0.15, 0.15, 0.45] {
                for side in [-1.0_f32, 1.0] {
                    pole(
                        &mut mesh,
                        at(side * lane * 0.42, 0.0, z),
                        at(-side * 0.05, 1.7, z),
                        0.05,
                        RAMP[3],
                        1.0,
                    );
                }
            }
            pole(
                &mut mesh,
                at(0.0, 1.62, -0.6),
                at(0.0, 1.62, 0.6),
                0.05,
                RAMP[3],
                1.0,
            );
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * 0.42 - 0.02, 0.0, -0.5),
                    at(side * 0.42 + 0.02, 0.5, 0.5),
                    RAMP[1],
                    1.0,
                );
            }
        }
        ObstacleKind::Hose => {
            // A tube lying across the lane in a gentle wave.
            let segments = 8;
            for i in 0..segments {
                let x0 = -lane * 0.5 + lane * i as f32 / segments as f32;
                let x1 = x0 + lane / segments as f32;
                let wave = |x: f32| 0.08 * (x * 5.0).sin();
                pole(
                    &mut mesh,
                    at(x0, 0.06, wave(x0)),
                    at(x1, 0.06, wave(x1)),
                    0.11,
                    RAMP[5],
                    1.0,
                );
            }
        }
        ObstacleKind::Puddle => {
            for (x, z, r) in [(0.0, 0.0, 1.0), (0.25, 0.15, 0.6), (-0.25, -0.1, 0.7)] {
                mesh.add(
                    Shape::Disc { sides: 12 },
                    part(at(r, 1.0, r * 0.7), none, at(x, 0.006, z)),
                    RAMP[2],
                    0.0,
                );
            }
        }
        ObstacleKind::Tray => {
            mesh.block(at(-0.45, 0.0, -0.25), at(0.45, 0.12, 0.25), RAMP[4], 1.0);
            for i in 0..4 {
                for j in 0..2 {
                    mesh.add(
                        cone(4),
                        upright(
                            at(0.08, 0.14, 0.08),
                            at(-0.3 + i as f32 * 0.2, 0.19, -0.1 + j as f32 * 0.2),
                        ),
                        RAMP[3],
                        1.0,
                    );
                }
            }
        }
        ObstacleKind::Pot => {
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.36,
                    top: 0.5,
                },
                upright(at(0.62, 0.5, 0.62), at(0.0, 0.25, 0.0)),
                RAMP[3],
                1.0,
            );
            mesh.add(
                cylinder(8),
                upright(at(0.7, 0.08, 0.7), at(0.0, 0.52, 0.0)),
                RAMP[3],
                1.0,
            );
            mesh.add(
                Shape::Disc { sides: 8 },
                upright(at(0.56, 1.0, 0.56), at(0.0, 0.53, 0.0)),
                RAMP[6],
                0.0,
            );
            for i in 0..3 {
                let turn = Quat::from_rotation_y(i as f32 * 2.1) * Quat::from_rotation_z(0.5);
                mesh.add(
                    Shape::Cube,
                    part(
                        at(0.06, 0.34, 0.1),
                        turn,
                        at(0.0, 0.62, 0.0) + turn * at(0.0, 0.12, 0.0),
                    ),
                    RAMP[4],
                    1.0,
                );
            }
        }
        ObstacleKind::Can => {
            mesh.add(
                cylinder(10),
                upright(at(0.42, 0.42, 0.42), at(0.0, 0.21, 0.0)),
                RAMP[4],
                1.0,
            );
            pole(
                &mut mesh,
                at(0.0, 0.25, 0.15),
                at(0.0, 0.55, 0.55),
                0.07,
                RAMP[4],
                1.0,
            );
            mesh.add(
                cone(6),
                part(
                    at(0.14, 0.1, 0.14),
                    Quat::from_rotation_x(-0.8),
                    at(0.0, 0.57, 0.58),
                ),
                RAMP[4],
                1.0,
            );
            mesh.add(
                Shape::Torus {
                    segments: 10,
                    sides: 4,
                    radius: 0.5,
                    tube: 0.08,
                },
                part(
                    Vec3::splat(0.32),
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                    at(0.0, 0.5, -0.05),
                ),
                RAMP[5],
                1.0,
            );
        }
        ObstacleKind::Gnome => {
            mesh.add(
                Shape::Frustum {
                    sides: 7,
                    bottom: 0.5,
                    top: 0.3,
                },
                upright(Vec3::splat(0.46), at(0.0, 0.23, 0.0)),
                RAMP[4],
                1.0,
            );
            mesh.add(
                SPHERE,
                upright(Vec3::splat(0.32), at(0.0, 0.58, 0.0)),
                RAMP[1],
                1.0,
            );
            mesh.add(
                cone(6),
                part(
                    at(0.2, 0.3, 0.2),
                    Quat::from_rotation_x(std::f32::consts::PI),
                    at(0.0, 0.42, 0.12),
                ),
                RAMP[0],
                1.0,
            );
            mesh.add(
                cone(7),
                upright(at(0.36, 0.5, 0.36), at(0.0, 0.94, 0.0)),
                RAMP[6],
                1.0,
            );
        }
        ObstacleKind::Wire => {
            fence_posts(&mut mesh, 1.05);
            // A diamond mesh of thin wires between the posts.
            for i in 0..6 {
                let x = -lane * 0.45 + i as f32 * lane * 0.18;
                pole(
                    &mut mesh,
                    at(x, 0.0, 0.0),
                    at(x + 0.45, 1.0, 0.0),
                    0.02,
                    RAMP[5],
                    0.5,
                );
                pole(
                    &mut mesh,
                    at(x + 0.45, 0.0, 0.0),
                    at(x, 1.0, 0.0),
                    0.02,
                    RAMP[5],
                    0.5,
                );
            }
            mesh.block(
                at(-lane * 0.5, 0.98, -0.03),
                at(lane * 0.5, 1.04, 0.03),
                RAMP[3],
                1.0,
            );
        }
        ObstacleKind::Barrow => {
            mesh.add(
                Shape::Frustum {
                    sides: 4,
                    bottom: 0.45,
                    top: 0.62,
                },
                part(
                    at(0.95, 0.34, 1.25),
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
                    at(0.0, 0.6, 0.0),
                ),
                RAMP[5],
                1.0,
            );
            mesh.add(
                cylinder(10),
                part(
                    at(0.4, 0.1, 0.4),
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    at(0.0, 0.2, 0.5),
                ),
                RAMP[6],
                1.0,
            );
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * 0.3 - 0.04, 0.0, -0.5),
                    at(side * 0.3 + 0.04, 0.45, -0.42),
                    RAMP[2],
                    1.0,
                );
                mesh.block(
                    at(side * 0.3 - 0.04, 0.5, -1.0),
                    at(side * 0.3 + 0.04, 0.58, 0.3),
                    RAMP[2],
                    1.0,
                );
            }
        }
        ObstacleKind::Scarecrow => {
            pole(
                &mut mesh,
                at(0.0, 0.0, 0.0),
                at(0.0, 1.9, 0.0),
                0.08,
                RAMP[3],
                1.0,
            );
            pole(
                &mut mesh,
                at(-0.6, 1.3, 0.0),
                at(0.6, 1.3, 0.0),
                0.07,
                RAMP[3],
                1.0,
            );
            mesh.block(at(-0.24, 0.75, -0.13), at(0.24, 1.4, 0.13), RAMP[5], 1.0);
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * 0.24, 1.18, -0.1),
                    at(side * 0.62, 1.38, 0.1),
                    RAMP[5],
                    1.0,
                );
            }
            mesh.add(
                SPHERE,
                upright(Vec3::splat(0.3), at(0.0, 1.6, 0.0)),
                RAMP[1],
                1.0,
            );
            mesh.add(
                cylinder(10),
                upright(at(0.62, 0.03, 0.62), at(0.0, 1.74, 0.0)),
                RAMP[2],
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 10,
                    bottom: 0.5,
                    top: 0.4,
                },
                upright(at(0.3, 0.2, 0.3), at(0.0, 1.85, 0.0)),
                RAMP[2],
                1.0,
            );
        }
        ObstacleKind::BirdNet => {
            for side in [-1.0_f32, 1.0] {
                pole(
                    &mut mesh,
                    at(side * lane * 0.47, 0.0, 0.0),
                    at(side * lane * 0.47, 1.1, 0.0),
                    0.05,
                    RAMP[3],
                    1.0,
                );
            }
            // The net sags across at a small bunny's head height: duck.
            mesh.block(
                at(-lane * 0.47, 0.42, -0.02),
                at(lane * 0.47, 0.85, 0.02),
                RAMP[1],
                1.0,
            );
            for i in 0..5 {
                let x = -lane * 0.4 + i as f32 * lane * 0.2;
                pole(
                    &mut mesh,
                    at(x, 0.42, 0.03),
                    at(x, 0.85, 0.03),
                    0.015,
                    RAMP[5],
                    0.5,
                );
            }
            pole(
                &mut mesh,
                at(-lane * 0.47, 1.08, 0.0),
                at(lane * 0.47, 1.08, 0.0),
                0.03,
                RAMP[4],
                1.0,
            );
        }
    }
    mesh
}

/// A power-up's model, about half a metre tall, floating at its height
/// when drawn.
#[must_use]
pub fn power(kind: PowerKind) -> Mesh {
    let none = Quat::IDENTITY;
    let mut mesh = Mesh::new();
    match kind {
        PowerKind::Clover => {
            pole(
                &mut mesh,
                at(0.0, 0.0, 0.0),
                at(0.0, 0.3, 0.0),
                0.04,
                LEAF,
                1.5,
            );
            for i in 0..3 {
                let turn = Quat::from_rotation_y(i as f32 * std::f32::consts::TAU / 3.0);
                for side in [-1.0_f32, 1.0] {
                    mesh.add(
                        Shape::Disc { sides: 8 },
                        part(
                            at(0.16, 1.0, 0.16),
                            turn * Quat::from_rotation_x(-0.3),
                            at(0.0, 0.32, 0.0) + turn * at(side * 0.06, 0.0, 0.12),
                        ),
                        CLOVER,
                        1.5,
                    );
                }
            }
        }
        PowerKind::Dandelion => {
            pole(
                &mut mesh,
                at(0.0, 0.0, 0.0),
                at(0.0, 0.35, 0.0),
                0.03,
                LEAF,
                1.5,
            );
            mesh.add(
                SPHERE,
                upright(Vec3::splat(0.32), at(0.0, 0.46, 0.0)),
                PUFF,
                1.5,
            );
        }
        PowerKind::SunHat => {
            mesh.add(
                cylinder(12),
                upright(at(0.62, 0.03, 0.62), at(0.0, 0.1, 0.0)),
                STRAW,
                1.5,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 12,
                    bottom: 0.5,
                    top: 0.38,
                },
                upright(at(0.34, 0.2, 0.34), at(0.0, 0.22, 0.0)),
                STRAW,
                1.5,
            );
            mesh.add(
                cylinder(12),
                upright(at(0.35, 0.05, 0.35), at(0.0, 0.15, 0.0)),
                0xE04848,
                1.5,
            );
        }
        PowerKind::Magnet => {
            mesh.add(
                SPHERE,
                upright(at(0.3, 0.28, 0.3), at(0.0, 0.2, 0.0)),
                MAGNET,
                1.5,
            );
            for side in [-1.0_f32, 1.0] {
                mesh.add(
                    Shape::Cube,
                    upright(at(0.08, 0.16, 0.08), at(side * 0.1, 0.42, 0.0)),
                    STEEL,
                    1.5,
                );
            }
            mesh.add(
                Shape::Cube,
                part(at(0.06, 0.2, 0.06), none, at(0.0, 0.4, 0.0)),
                LEAF,
                1.5,
            );
        }
    }
    mesh
}

/// The meadow's pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Mound,
    Hole,
    Signpost,
    Stone,
    Pond,
    Tree,
    Flowers,
    Log,
    Gate,
    Board,
    Arch,
}

impl Piece {
    pub const ALL: [Self; 11] = [
        Self::Mound,
        Self::Hole,
        Self::Signpost,
        Self::Stone,
        Self::Pond,
        Self::Tree,
        Self::Flowers,
        Self::Log,
        Self::Gate,
        Self::Board,
        Self::Arch,
    ];
}

/// A meadow piece's model.
#[must_use]
pub fn piece(kind: Piece) -> Mesh {
    let none = Quat::IDENTITY;
    let mut mesh = Mesh::new();
    match kind {
        Piece::Mound => {
            mesh.add(
                Shape::Sphere {
                    rings: 6,
                    segments: 14,
                },
                upright(at(14.0, 4.0, 14.0), at(0.0, 0.0, 0.0)),
                RAMP[1],
                1.0,
            );
            for i in 0..4 {
                let turn = Quat::from_rotation_y(i as f32 * std::f32::consts::FRAC_PI_2 + 0.4);
                mesh.add(
                    Shape::Disc { sides: 10 },
                    part(
                        at(1.0, 1.0, 1.3),
                        turn * Quat::from_rotation_x(1.2),
                        turn * at(0.0, 0.9, 6.7),
                    ),
                    RAMP[6],
                    0.0,
                );
            }
        }
        Piece::Hole => {
            // A dark opening in a ring of turned-up earth.
            mesh.add(
                Shape::Disc { sides: 14 },
                upright(at(1.9, 1.0, 1.9), at(0.0, 0.01, 0.0)),
                RAMP[7],
                0.0,
            );
            for i in 0..10 {
                let a = i as f32 / 10.0 * std::f32::consts::TAU;
                mesh.add(
                    Shape::Sphere {
                        rings: 3,
                        segments: 6,
                    },
                    part(
                        at(0.7, 0.32, 0.45),
                        Quat::from_rotation_y(-a),
                        at(a.sin() * 1.15, 0.05, a.cos() * 1.15),
                    ),
                    RAMP[2],
                    1.0,
                );
            }
        }
        Piece::Signpost => {
            pole(
                &mut mesh,
                at(0.0, 0.0, 0.0),
                at(0.0, 2.0, 0.0),
                0.12,
                RAMP[3],
                1.0,
            );
            mesh.block(at(-0.7, 1.4, -0.05), at(0.7, 2.0, 0.05), RAMP[1], 1.0);
        }
        Piece::Stone => {
            mesh.add(
                Shape::Sphere {
                    rings: 4,
                    segments: 7,
                },
                upright(at(1.0, 0.5, 0.8), at(0.0, 0.1, 0.0)),
                RAMP[3],
                1.0,
            );
        }
        Piece::Pond => {
            mesh.add(
                Shape::Disc { sides: 24 },
                upright(at(16.0, 1.0, 12.0), at(0.0, 0.01, 0.0)),
                RAMP[1],
                0.0,
            );
        }
        Piece::Tree => {
            mesh.add(
                cylinder(7),
                upright(at(0.5, 2.4, 0.5), at(0.0, 1.2, 0.0)),
                RAMP[5],
                1.0,
            );
            for (y, r) in [(2.6, 3.4), (3.8, 2.6), (4.9, 1.8)] {
                mesh.add(
                    cone(8),
                    upright(at(r, 1.8, r), at(0.0, y, 0.0)),
                    RAMP[3],
                    1.0,
                );
            }
        }
        Piece::Flowers => {
            for i in 0..5 {
                let a = i as f32 * 1.3;
                let p = at(a.cos() * 0.4, 0.0, a.sin() * 0.4);
                pole(&mut mesh, p, p + at(0.0, 0.4, 0.0), 0.03, RAMP[4], 1.0);
                mesh.add(
                    Shape::Disc { sides: 6 },
                    upright(at(0.16, 1.0, 0.16), p + at(0.0, 0.41, 0.0)),
                    RAMP[0],
                    1.0,
                );
            }
        }
        Piece::Log => {
            mesh.add(
                cylinder(9),
                part(
                    at(0.7, 3.0, 0.7),
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    at(0.0, 0.35, 0.0),
                ),
                RAMP[4],
                1.0,
            );
        }
        Piece::Gate => {
            for side in [-1.0_f32, 1.0] {
                pole(
                    &mut mesh,
                    at(side * 2.0, 0.0, 0.0),
                    at(side * 2.0, 2.6, 0.0),
                    0.18,
                    RAMP[3],
                    1.0,
                );
            }
            mesh.block(at(-2.2, 2.1, -0.06), at(2.2, 2.6, 0.06), RAMP[1], 1.0);
            mesh.add(
                Shape::Square,
                upright(at(2.6, 1.0, 1.4), at(0.0, 0.01, 0.9)),
                RAMP[2],
                0.0,
            );
        }
        Piece::Board => {
            for side in [-1.0_f32, 1.0] {
                pole(
                    &mut mesh,
                    at(side * 1.1, 0.0, 0.0),
                    at(side * 1.1, 2.4, 0.0),
                    0.12,
                    RAMP[4],
                    1.0,
                );
            }
            mesh.block(at(-1.3, 0.9, -0.06), at(1.3, 2.3, 0.06), RAMP[0], 1.0);
            for i in 0..4 {
                let y = 1.1 + i as f32 * 0.3;
                mesh.block(
                    at(-1.0, y, 0.06),
                    at(0.6 - i as f32 * 0.2, y + 0.08, 0.08),
                    RAMP[5],
                    0.0,
                );
            }
        }
        Piece::Arch => {
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * 2.0 - 0.3, 0.0, -0.3),
                    at(side * 2.0 + 0.3, 3.2, 0.3),
                    RAMP[2],
                    1.0,
                );
            }
            mesh.add(
                Shape::Torus {
                    segments: 12,
                    sides: 4,
                    radius: 0.5,
                    tube: 0.08,
                },
                part(at(4.0, 4.0, 2.0), none, at(0.0, 3.0, 0.0)),
                RAMP[2],
                1.0,
            );
        }
    }
    mesh
}

/// A stone of the carrot ladder, white so a tint colours it.
#[must_use]
pub fn ladder_stone() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Frustum {
            sides: 7,
            bottom: 0.5,
            top: 0.42,
        },
        upright(at(1.1, 0.35, 1.1), at(0.0, 0.17, 0.0)),
        0xFFFFFF,
        1.0,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{admit_gray, chroma};
    use crate::mesh::rgb;

    fn triangles(mesh: &Mesh) -> usize {
        mesh.vertices() / 3
    }

    #[test]
    fn the_ramp_is_gray_and_runs_light_to_dark() {
        for pair in RAMP.windows(2) {
            assert!(pair[1] & 0xFF < pair[0] & 0xFF);
        }
        for colour in RAMP {
            assert!(chroma(rgb(colour)) <= crate::look::GRAY_CHROMA);
        }
    }

    #[test]
    fn every_kit_piece_is_made_within_its_triangle_budget_and_colour_rule() {
        for kind in ObstacleKind::ALL {
            let mesh = obstacle(kind);
            assert!((1..=1_200).contains(&triangles(&mesh)), "{kind:?}");
            admit_gray(&mesh).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        }
        for kind in PowerKind::ALL {
            let mesh = power(kind);
            assert!((1..=300).contains(&triangles(&mesh)), "{kind:?}");
            assert!(admit_gray(&mesh).is_err(), "{kind:?} is in colour");
        }
        for kind in Piece::ALL {
            let mesh = piece(kind);
            assert!((1..=2_500).contains(&triangles(&mesh)), "{kind:?}");
            admit_gray(&mesh).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        }
    }

    #[test]
    fn a_bird_net_hangs_where_a_ducking_bunny_passes_and_a_hose_is_low() {
        let lowest = |mesh: &Mesh, min_x: f32| {
            mesh.data
                .chunks(crate::mesh::STRIDE)
                .filter(|v| v[0].abs() < min_x)
                .map(|v| v[1])
                .fold(f32::MAX, f32::min)
        };
        // Under the net, in the lane's middle, nothing hangs below 0.4 m.
        assert!(lowest(&obstacle(ObstacleKind::BirdNet), 0.4) >= 0.4);
        let top = obstacle(ObstacleKind::Hose)
            .data
            .chunks(crate::mesh::STRIDE)
            .map(|v| v[1])
            .fold(f32::MIN, f32::max);
        assert!(top < 0.15, "a Kit's jump clears it: {top}");
    }
}
