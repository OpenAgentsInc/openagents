//! The garden and everything in it, as meshes in metres: `x` east, `y` up,
//! `z` south. Models face `+z` and stand on `y = 0`.
//!
//! Everything is a gray from one ramp except the bunny and its food, the
//! only things in colour.

use bunny_rules::garden::Garden;
use bunny_rules::{CORRIDOR_HALF, LANE_WIDTH, UNIT};
use glam::{Mat4, Quat, Vec3};

use crate::mesh::{Mesh, Shape};

/// The page behind everything.
pub const SKY: u32 = 0xECECE9;
/// The ground: paper.
pub const PAPER: u32 = 0xF4F4F2;
/// The colour of every line, fading to `FAR_INK` with distance.
pub const INK: u32 = 0x1E1E1E;
pub const FAR_INK: u32 = 0x9A9A9A;
pub const HEDGE: u32 = 0xD3D3CE;
const MARK: u32 = 0xE2E2DE;
const SHADOW: u32 = 0xDADAD6;
const POT: u32 = 0xA9A9A4;
const SOIL: u32 = 0x6E6E6A;
const STONE: u32 = 0x8F8F8A;
const WOOD: u32 = 0xBDBDB8;
const BOARD: u32 = 0x9C9C97;
const METAL: u32 = 0x7C7C78;
const SHIRT: u32 = 0x8C8C88;
const TROUSERS: u32 = 0x5C5C59;
const SKIN: u32 = 0xD8D8D4;
const STRAW: u32 = 0xC2C2BD;
const NET: u32 = 0x3C3C3C;
const MESH_BAG: u32 = 0xE8E8E4;
pub const CARROT: u32 = 0xF28A1E;
pub const LEAF: u32 = 0x3FA34D;
pub const GOLD: u32 = 0xF2C230;
const RADISH: u32 = 0xD2306E;
const LETTUCE: u32 = 0x9BD46A;
const BERRY: u32 = 0xE0283A;
const PUMPKIN: u32 = 0xE0661A;
const SEEDLING: u32 = 0x58B947;
const INNER_EAR: u32 = 0xF4B6C2;
const EYE: u32 = 0x151515;

/// Hedges are taller than the biggest bunny.
pub const HEDGE_HEIGHT: f32 = 2.4;

/// Rules units to metres.
#[must_use]
pub fn metres(units: i32) -> f32 {
    units as f32 / UNIT as f32
}

/// A model placed at `at`, turned `yaw` about `y` (0 faces `+z`), and
/// scaled evenly.
#[must_use]
pub fn place(at: Vec3, yaw: f32, scale: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), Quat::from_rotation_y(yaw), at)
}

/// The yaw that faces the unit direction `(dx, dz)`.
#[must_use]
pub fn yaw_of(dx: f32, dz: f32) -> f32 {
    dx.atan2(dz)
}

fn at(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn part(scale: Vec3, rotation: Quat, centre: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(scale, rotation, centre)
}

/// The garden's bounds in metres: (min x, min z, max x, max z).
#[must_use]
pub fn bounds(garden: &Garden) -> (f32, f32, f32, f32) {
    let xs = garden.nodes.iter().map(|n| metres(n.x));
    let zs = garden.nodes.iter().map(|n| metres(n.z));
    (
        xs.clone().fold(f32::MAX, f32::min),
        zs.clone().fold(f32::MAX, f32::min),
        xs.fold(f32::MIN, f32::max),
        zs.fold(f32::MIN, f32::max),
    )
}

/// The ground under the whole garden and well beyond it, with the dashes
/// between the lanes drawn flat on it.
#[must_use]
pub fn ground(garden: &Garden) -> Mesh {
    let (x0, z0, x1, z1) = bounds(garden);
    let mut mesh = Mesh::new();
    mesh.block(
        at(x0 - 80.0, -0.2, z0 - 80.0),
        at(x1 + 80.0, 0.0, z1 + 80.0),
        PAPER,
        0.0,
    );
    // Dashes between the lanes.
    let line = metres(LANE_WIDTH) / 2.0;
    for (index, e) in garden.edges.iter().enumerate() {
        let len = metres(e.len);
        let mut s = 2.5;
        while s + 1.0 <= len - 2.5 {
            for side in [-line, line] {
                let (ax, az) =
                    garden.point(index, (s * UNIT as f32) as i32, (side * UNIT as f32) as i32);
                let (bx, bz) = garden.point(
                    index,
                    ((s + 1.0) * UNIT as f32) as i32,
                    (side * UNIT as f32) as i32,
                );
                let (ax, az, bx, bz) = (metres(ax), metres(az), metres(bx), metres(bz));
                let (lo, hi) = (
                    at(ax.min(bx) - 0.04, 0.005, az.min(bz) - 0.04),
                    at(ax.max(bx) + 0.04, 0.005, az.max(bz) + 0.04),
                );
                mesh.add(
                    Shape::Square,
                    part(
                        Vec3::new(hi.x - lo.x, 1.0, hi.z - lo.z),
                        Quat::IDENTITY,
                        (lo + hi) * 0.5,
                    ),
                    MARK,
                    0.0,
                );
            }
            s += 2.0;
        }
    }
    mesh
}

/// The hedges between and around the corridors.
///
/// The hedges fill each grid cell between junctions, the gap between two
/// neighbouring junctions with no corridor between them, and a border
/// around the whole garden.
#[must_use]
pub fn hedges(garden: &Garden) -> Mesh {
    let half = metres(CORRIDOR_HALF);
    let mut xs: Vec<i32> = garden.nodes.iter().map(|n| n.x).collect();
    let mut zs: Vec<i32> = garden.nodes.iter().map(|n| n.z).collect();
    xs.sort_unstable();
    xs.dedup();
    zs.sort_unstable();
    zs.dedup();
    let node_at = |x: i32, z: i32| garden.nodes.iter().position(|n| n.x == x && n.z == z);
    let joined = |a: usize, b: usize| {
        garden
            .edges
            .iter()
            .any(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a))
    };
    let mut mesh = Mesh::new();
    let mut hedge = |x0: f32, z0: f32, x1: f32, z1: f32| {
        if x1 - x0 > 0.01 && z1 - z0 > 0.01 {
            mesh.block(at(x0, 0.0, z0), at(x1, HEDGE_HEIGHT, z1), HEDGE, 1.0);
        }
    };
    for pair in xs.windows(2) {
        for row in zs.windows(2) {
            hedge(
                metres(pair[0]) + half,
                metres(row[0]) + half,
                metres(pair[1]) - half,
                metres(row[1]) - half,
            );
        }
    }
    for pair in xs.windows(2) {
        for z in &zs {
            if let (Some(a), Some(b)) = (node_at(pair[0], *z), node_at(pair[1], *z))
                && !joined(a, b)
            {
                let z = metres(*z);
                hedge(
                    metres(pair[0]) + half,
                    z - half,
                    metres(pair[1]) - half,
                    z + half,
                );
            }
        }
    }
    for pair in zs.windows(2) {
        for x in &xs {
            if let (Some(a), Some(b)) = (node_at(*x, pair[0]), node_at(*x, pair[1]))
                && !joined(a, b)
            {
                let x = metres(*x);
                hedge(
                    x - half,
                    metres(pair[0]) + half,
                    x + half,
                    metres(pair[1]) - half,
                );
            }
        }
    }
    let (x0, z0, x1, z1) = bounds(garden);
    let border = 3.0;
    hedge(
        x0 - half - border,
        z0 - half - border,
        x1 + half + border,
        z0 - half,
    );
    hedge(
        x0 - half - border,
        z1 + half,
        x1 + half + border,
        z1 + half + border,
    );
    hedge(x0 - half - border, z0 - half, x0 - half, z1 + half);
    hedge(x1 + half, z0 - half, x1 + half + border, z1 + half);
    mesh
}

/// A carrot, tip down, about half a metre tall, in `colour`.
fn carrot_in(colour: u32) -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Frustum {
            sides: 6,
            bottom: 0.5,
            top: 0.0,
        },
        part(
            at(0.2, 0.44, 0.2),
            Quat::from_rotation_x(std::f32::consts::PI),
            at(0.0, 0.3, 0.0),
        ),
        colour,
        1.5,
    );
    leaves(&mut mesh, 0.6, 1.0);
    mesh
}

/// A tuft of three leaves standing at height `y`.
fn leaves(mesh: &mut Mesh, y: f32, scale: f32) {
    for (i, lean) in [-0.45_f32, 0.0, 0.45].iter().enumerate() {
        let turn = Quat::from_rotation_y(i as f32 * 1.1) * Quat::from_rotation_z(*lean);
        mesh.add(
            Shape::Cube,
            part(
                at(0.05, 0.24, 0.09) * scale,
                turn,
                at(0.0, y, 0.0) + turn * at(0.0, 0.06 * scale, 0.0),
            ),
            LEAF,
            1.5,
        );
    }
}

/// A carrot.
#[must_use]
pub fn carrot() -> Mesh {
    carrot_in(CARROT)
}

/// Each edible's model, standing on the ground and facing `+z`.
#[must_use]
pub fn edible(kind: bunny_rules::EdibleKind) -> Mesh {
    use bunny_rules::EdibleKind as E;
    let sphere = Shape::Sphere {
        rings: 5,
        segments: 8,
    };
    let none = Quat::IDENTITY;
    match kind {
        E::Carrot => carrot_in(CARROT),
        E::Golden => carrot_in(GOLD),
        E::Seedling => {
            let mut mesh = Mesh::new();
            mesh.add(
                Shape::Cube,
                part(at(0.03, 0.14, 0.03), none, at(0.0, 0.07, 0.0)),
                SEEDLING,
                1.5,
            );
            for side in [-1.0_f32, 1.0] {
                let turn = Quat::from_rotation_z(0.9 * side);
                mesh.add(
                    Shape::Cube,
                    part(at(0.05, 0.12, 0.08), turn, at(0.05 * side, 0.15, 0.0)),
                    SEEDLING,
                    1.5,
                );
            }
            mesh
        }
        E::Radish => {
            let mut mesh = Mesh::new();
            mesh.add(
                sphere,
                part(at(0.26, 0.24, 0.26), none, at(0.0, 0.16, 0.0)),
                RADISH,
                1.5,
            );
            leaves(&mut mesh, 0.3, 0.8);
            mesh
        }
        E::Lettuce => {
            let mut mesh = Mesh::new();
            mesh.add(
                sphere,
                part(at(0.6, 0.42, 0.6), none, at(0.0, 0.21, 0.0)),
                LETTUCE,
                1.5,
            );
            for i in 0..4 {
                let turn = Quat::from_rotation_y(i as f32 * 1.57) * Quat::from_rotation_x(0.6);
                mesh.add(
                    Shape::Cube,
                    part(
                        at(0.3, 0.04, 0.26),
                        turn,
                        at(0.0, 0.2, 0.0) + turn * at(0.0, 0.0, 0.24),
                    ),
                    LETTUCE,
                    1.5,
                );
            }
            mesh
        }
        E::Strawberry => {
            let mut mesh = Mesh::new();
            mesh.add(
                Shape::Frustum {
                    sides: 7,
                    bottom: 0.5,
                    top: 0.0,
                },
                part(
                    at(0.24, 0.3, 0.24),
                    Quat::from_rotation_x(std::f32::consts::PI),
                    at(0.0, 0.17, 0.0),
                ),
                BERRY,
                1.5,
            );
            leaves(&mut mesh, 0.3, 0.6);
            mesh
        }
        E::Pumpkin | E::Bonus => {
            let mut mesh = Mesh::new();
            let colour = if kind == E::Pumpkin {
                PUMPKIN
            } else {
                0x7CC242
            };
            mesh.add(
                sphere,
                part(at(0.9, 0.6, 0.9), none, at(0.0, 0.3, 0.0)),
                colour,
                1.5,
            );
            mesh.add(
                Shape::Cube,
                part(at(0.06, 0.2, 0.06), none, at(0.0, 0.66, 0.0)),
                LEAF,
                1.5,
            );
            mesh
        }
    }
}

/// The bunny's body, head, tail and face, one unit tall, in `fur`.
#[must_use]
pub fn bunny(fur: u32) -> Mesh {
    let mut mesh = Mesh::new();
    let sphere = Shape::Sphere {
        rings: 5,
        segments: 8,
    };
    let none = Quat::IDENTITY;
    mesh.add(
        sphere,
        part(at(0.72, 0.6, 0.92), none, at(0.0, 0.33, -0.06)),
        fur,
        1.5,
    );
    mesh.add(
        sphere,
        part(at(0.5, 0.48, 0.52), none, at(0.0, 0.66, 0.34)),
        fur,
        1.5,
    );
    mesh.add(
        sphere,
        part(at(0.2, 0.2, 0.2), none, at(0.0, 0.42, -0.52)),
        0xFFFFFF,
        1.5,
    );
    for side in [-1.0_f32, 1.0] {
        mesh.add(
            sphere,
            part(at(0.18, 0.12, 0.3), none, at(0.2 * side, 0.06, 0.24)),
            fur,
            1.5,
        );
        mesh.add(
            Shape::Cube,
            part(at(0.07, 0.09, 0.05), none, at(0.13 * side, 0.73, 0.58)),
            EYE,
            0.0,
        );
    }
    mesh.add(
        Shape::Cube,
        part(at(0.07, 0.05, 0.05), none, at(0.0, 0.64, 0.61)),
        INNER_EAR,
        0.0,
    );
    mesh
}

/// One ear, standing from its base at the origin.
#[must_use]
pub fn ear(fur: u32) -> Mesh {
    let mut mesh = Mesh::new();
    let none = Quat::IDENTITY;
    mesh.add(
        Shape::Cube,
        part(at(0.12, 0.44, 0.05), none, at(0.0, 0.22, 0.0)),
        fur,
        1.5,
    );
    mesh.add(
        Shape::Cube,
        part(at(0.06, 0.32, 0.01), none, at(0.0, 0.22, 0.03)),
        INNER_EAR,
        0.0,
    );
    mesh
}

/// The farmer from the hips up.
#[must_use]
pub fn farmer() -> Mesh {
    let mut mesh = Mesh::new();
    let none = Quat::IDENTITY;
    mesh.add(
        Shape::Cube,
        part(at(0.5, 0.66, 0.3), none, at(0.0, 1.18, 0.0)),
        SHIRT,
        1.0,
    );
    for side in [-1.0_f32, 1.0] {
        let arm = Quat::from_rotation_x(-0.5);
        mesh.add(
            Shape::Cube,
            part(at(0.13, 0.6, 0.13), arm, at(0.33 * side, 1.2, 0.12)),
            SHIRT,
            1.0,
        );
    }
    mesh.add(
        Shape::Sphere {
            rings: 5,
            segments: 8,
        },
        part(at(0.32, 0.34, 0.32), none, at(0.0, 1.68, 0.0)),
        SKIN,
        1.0,
    );
    mesh.add(
        Shape::Frustum {
            sides: 10,
            bottom: 0.5,
            top: 0.5,
        },
        part(at(0.66, 0.03, 0.66), none, at(0.0, 1.82, 0.0)),
        STRAW,
        1.0,
    );
    mesh.add(
        Shape::Frustum {
            sides: 10,
            bottom: 0.5,
            top: 0.42,
        },
        part(at(0.34, 0.2, 0.34), none, at(0.0, 1.93, 0.0)),
        STRAW,
        1.0,
    );
    mesh
}

/// One leg, hanging from the hip at the origin.
#[must_use]
pub fn leg() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Cube,
        part(at(0.17, 0.86, 0.19), Quat::IDENTITY, at(0.0, -0.43, 0.0)),
        TROUSERS,
        1.0,
    );
    mesh
}

/// The long-handled net, from the hand at the origin up.
#[must_use]
pub fn net() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Cube,
        part(at(0.05, 1.5, 0.05), Quat::IDENTITY, at(0.0, 0.75, 0.0)),
        METAL,
        1.0,
    );
    mesh.add(
        Shape::Torus {
            segments: 12,
            sides: 4,
            radius: 0.5,
            tube: 0.06,
        },
        part(Vec3::splat(0.62), Quat::IDENTITY, at(0.0, 1.8, 0.0)),
        NET,
        1.0,
    );
    mesh.add(
        Shape::Frustum {
            sides: 8,
            bottom: 0.5,
            top: 0.0,
        },
        part(
            at(0.56, 0.45, 0.56),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            at(0.0, 1.8, -0.22),
        ),
        MESH_BAG,
        1.0,
    );
    mesh
}

/// The "!" over the farmer's head when he's after the bunny.
#[must_use]
pub fn alarm() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.block(at(-0.07, 0.25, -0.07), at(0.07, 0.75, 0.07), INK, 0.0);
    mesh.block(at(-0.07, 0.0, -0.07), at(0.07, 0.14, 0.07), INK, 0.0);
    mesh
}

/// The obstacle meshes, each filling one lane, facing `+z` along the
/// corridor.
#[must_use]
pub fn obstacle(kind: bunny_rules::ObstacleKind) -> Mesh {
    use bunny_rules::ObstacleKind as CellKind;
    let mut mesh = Mesh::new();
    let none = Quat::IDENTITY;
    let lane = metres(LANE_WIDTH);
    match kind {
        CellKind::Pot
        | CellKind::Hose
        | CellKind::Puddle
        | CellKind::Tray
        | CellKind::Can
        | CellKind::BirdNet => {
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.36,
                    top: 0.5,
                },
                part(at(0.62, 0.5, 0.62), none, at(0.0, 0.25, 0.0)),
                POT,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.5,
                    top: 0.5,
                },
                part(at(0.7, 0.08, 0.7), none, at(0.0, 0.52, 0.0)),
                POT,
                1.0,
            );
            mesh.add(
                Shape::Disc { sides: 8 },
                part(at(0.56, 1.0, 0.56), none, at(0.0, 0.53, 0.0)),
                SOIL,
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
                    STONE,
                    1.0,
                );
            }
        }
        CellKind::Gnome => {
            mesh.add(
                Shape::Frustum {
                    sides: 7,
                    bottom: 0.5,
                    top: 0.3,
                },
                part(at(0.46, 0.46, 0.46), none, at(0.0, 0.23, 0.0)),
                STONE,
                1.0,
            );
            mesh.add(
                Shape::Sphere {
                    rings: 4,
                    segments: 7,
                },
                part(Vec3::splat(0.32), none, at(0.0, 0.58, 0.0)),
                SKIN,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 6,
                    bottom: 0.5,
                    top: 0.0,
                },
                part(
                    at(0.2, 0.3, 0.2),
                    Quat::from_rotation_x(std::f32::consts::PI),
                    at(0.0, 0.42, 0.12),
                ),
                0xF0F0EE,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 7,
                    bottom: 0.5,
                    top: 0.0,
                },
                part(at(0.36, 0.5, 0.36), none, at(0.0, 0.94, 0.0)),
                TROUSERS,
                1.0,
            );
        }
        CellKind::Fence | CellKind::Gap | CellKind::Wire | CellKind::Tunnel => {
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * lane * 0.46 - 0.06, 0.0, -0.06),
                    at(side * lane * 0.46 + 0.06, 1.15, 0.06),
                    WOOD,
                    1.0,
                );
            }
            let boards: &[f32] = if kind == CellKind::Fence {
                &[0.22, 0.58, 0.94]
            } else {
                &[0.6, 0.94]
            };
            for y in boards {
                mesh.block(
                    at(-lane * 0.5, y - 0.1, -0.03),
                    at(lane * 0.5, y + 0.1, 0.03),
                    if kind == CellKind::Fence { WOOD } else { BOARD },
                    1.0,
                );
            }
        }
        CellKind::Barrow | CellKind::Scarecrow => {
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
                METAL,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 10,
                    bottom: 0.5,
                    top: 0.5,
                },
                part(
                    at(0.4, 0.1, 0.4),
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    at(0.0, 0.2, 0.5),
                ),
                TROUSERS,
                1.0,
            );
            for side in [-1.0_f32, 1.0] {
                mesh.block(
                    at(side * 0.3 - 0.04, 0.0, -0.5),
                    at(side * 0.3 + 0.04, 0.45, -0.42),
                    WOOD,
                    1.0,
                );
                mesh.block(
                    at(side * 0.3 - 0.04, 0.5, -1.0),
                    at(side * 0.3 + 0.04, 0.58, 0.3),
                    WOOD,
                    1.0,
                );
            }
        }
    }
    mesh
}

/// A soft round shadow on the ground.
#[must_use]
pub fn shadow() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Disc { sides: 14 },
        Mat4::from_translation(at(0.0, 0.03, 0.0)),
        SHADOW,
        0.0,
    );
    mesh
}

/// A plain cube for crumbs and shards.
#[must_use]
pub fn crumb() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(Shape::Cube, Mat4::IDENTITY, 0xFFFFFF, 0.0);
    mesh
}

/// A flat disc for the map's markers.
#[must_use]
pub fn dot() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(Shape::Disc { sides: 12 }, Mat4::IDENTITY, 0xFFFFFF, 0.0);
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunny_rules::ObstacleKind as CellKind;

    #[test]
    fn the_first_garden_has_hedges_dashes_and_a_ground() {
        let garden = bunny_rules::level::garden(1);
        let mesh = hedges(&garden);
        // 6 grid cells and 4 borders.
        assert_eq!(mesh.vertices(), (6 + 4) * 36);
        assert!(ground(&garden).vertices() > 36, "the dashes");
        let (x0, z0, x1, z1) = bounds(&garden);
        assert_eq!((x0, z0, x1, z1), (0.0, 0.0, 60.0, 40.0));
        // No hedge stands in a corridor: sample each corridor's centre line.
        let blocks: Vec<(Vec3, Vec3)> = mesh
            .data
            .chunks(crate::mesh::STRIDE * 36)
            .filter(|chunk| chunk[12] > 0.5)
            .map(|chunk| {
                let points = chunk
                    .chunks(crate::mesh::STRIDE)
                    .map(|v| Vec3::new(v[0], v[1], v[2]));
                (
                    points.clone().fold(Vec3::MAX, Vec3::min),
                    points.fold(Vec3::MIN, Vec3::max),
                )
            })
            .collect();
        for (index, e) in garden.edges.iter().enumerate() {
            for step in 0..=10 {
                let (x, z) = garden.point(index, e.len * step / 10, 0);
                let (x, z) = (metres(x), metres(z));
                assert!(
                    !blocks
                        .iter()
                        .any(|(lo, hi)| x > lo.x && x < hi.x && z > lo.z && z < hi.z),
                    "a hedge on corridor {index} at {x},{z}"
                );
            }
        }
    }

    #[test]
    fn every_model_is_made() {
        for mesh in [
            carrot(),
            bunny(0xFFFFFF),
            ear(0xFE6B04),
            farmer(),
            leg(),
            net(),
            alarm(),
            shadow(),
            crumb(),
            dot(),
            obstacle(CellKind::Pot),
            obstacle(CellKind::Gnome),
            obstacle(CellKind::Fence),
            obstacle(CellKind::Gap),
            obstacle(CellKind::Barrow),
        ] {
            assert!(mesh.vertices() >= 3);
        }
        assert!((yaw_of(1.0, 0.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        let turned = place(Vec3::ZERO, yaw_of(1.0, 0.0), 1.0).transform_vector3(Vec3::Z);
        assert!((turned - Vec3::X).length() < 1e-6, "+z turns to face east");
    }

    #[test]
    fn only_the_bunny_its_food_and_power_ups_are_in_colour() {
        use crate::look::admit_gray;
        let garden = Garden::clone(&bunny_rules::level::garden(1));
        for mesh in [
            hedges(&garden),
            ground(&garden),
            farmer(),
            leg(),
            net(),
            alarm(),
            shadow(),
        ] {
            admit_gray(&mesh).unwrap();
        }
        for kind in bunny_rules::ObstacleKind::ALL {
            admit_gray(&obstacle(kind)).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        }
        for kind in bunny_rules::EdibleKind::ALL {
            assert!(admit_gray(&edible(kind)).is_err(), "{kind:?} is in colour");
        }
    }
}
