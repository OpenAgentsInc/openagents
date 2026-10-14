//! The low-poly models, made in code: each room of the house (walls in
//! pastel flavours, waffle-cone trim, melting drips, sprinkle carpet and
//! furniture), its ceiling lights, the blocky avatar and its tools, the
//! Ice Cream Man, and the little things (keys, fudge, sprinkles, clouds).
//!
//! Models face +z in their own space; the world's floor is y = 0.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Mat4, Quat, Vec3};
use icecream_rules::geom::{Block, V2};
use icecream_rules::layout::{
    CELL, DOOR, Flavor, House, Module, PropKind, Side, WALL_HEIGHT, centre,
};

use crate::mesh::{Mesh, Shape};

/// The palette.
pub const WAFFLE: u32 = 0xD9A05B;
pub const WAFFLE_DARK: u32 = 0xB57B3A;
pub const FUDGE: u32 = 0x5A2E12;
pub const VANILLA: u32 = 0xFFF3D9;
pub const FOREHEAD: u32 = 0xFFE4D0;
pub const CEILING: u32 = 0xF6EFE4;
pub const GOLD: u32 = 0xF7C52B;
pub const WHITE: u32 = 0xF7F7F4;
pub const INK: u32 = 0x2B1D16;
pub const CLOUD: u32 = 0xBFD86A;
pub const SKIN: u32 = 0xF2C9A0;
pub const SHIRT: u32 = 0x4F9DDE;
pub const PANTS: u32 = 0x334E7A;
pub const SPRINKLES: [u32; 5] = [0xFF6FA8, 0x5EC8F2, 0xFFE15C, 0x7EDB7A, 0xFFFFFF];

/// A flavour's wall colour.
#[must_use]
pub fn wall(flavor: Flavor) -> u32 {
    match flavor {
        Flavor::Strawberry => 0xF6B3C8,
        Flavor::Mint => 0xBDEBD4,
        Flavor::Vanilla => 0xFFF1CF,
        Flavor::Chocolate => 0xB98466,
    }
}

/// A flavour's carpet: a deeper shade of the walls.
#[must_use]
pub fn carpet(flavor: Flavor) -> u32 {
    match flavor {
        Flavor::Strawberry => 0xE88AA6,
        Flavor::Mint => 0x8FD3B4,
        Flavor::Vanilla => 0xE9D3A4,
        Flavor::Chocolate => 0x8A5A40,
    }
}

/// Whether a room is lit cold (freezer blue) rather than warm.
#[must_use]
pub fn cold(module: Module) -> bool {
    matches!(
        module,
        Module::Hallway | Module::Stairwell | Module::FreezerRoom | Module::Laundry
    )
}

fn hash(a: u32, b: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x
}

/// A number from 0 to 1 from two others.
fn noise(a: u32, b: u32) -> f32 {
    (hash(a, b) % 10_000) as f32 / 10_000.0
}

fn boxed(mesh: &mut Mesh, min: Vec3, max: Vec3, colour: u32, weight: f32) {
    mesh.block(min, max, colour, weight);
}

/// The face of a box that looks toward `toward`, as a unit axis.
fn front(block: &Block, toward: V2) -> Vec3 {
    let c = (block.min + block.max) * 0.5;
    let d = toward - c;
    if d.x.abs() > d.z.abs() {
        Vec3::new(d.x.signum(), 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, d.z.signum())
    }
}

fn v3(p: V2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.z)
}

/// One room: floor, ceiling, its side of each wall with doorways, trim,
/// drips, the carpet's sprinkles, and its furniture. The front door is
/// drawn in its room.
#[must_use]
pub fn room(house: &House, index: usize) -> Mesh {
    let mut mesh = Mesh::new();
    let room = &house.rooms[index];
    let c = centre(room.cell);
    let x0 = c.x - CELL / 2.0;
    let z0 = c.z - CELL / 2.0;
    let colour = wall(room.flavor);
    let half = 0.15;
    // Floor and ceiling.
    boxed(
        &mut mesh,
        Vec3::new(x0, -0.1, z0),
        Vec3::new(x0 + CELL, 0.0, z0 + CELL),
        carpet(room.flavor),
        0.0,
    );
    boxed(
        &mut mesh,
        Vec3::new(x0, WALL_HEIGHT, z0),
        Vec3::new(x0 + CELL, WALL_HEIGHT + 0.1, z0 + CELL),
        CEILING,
        0.0,
    );
    // Sprinkles on the carpet.
    for i in 0..36 {
        let x = x0 + 0.4 + noise(index as u32, i * 2) * (CELL - 0.8);
        let z = z0 + 0.4 + noise(index as u32, i * 2 + 1) * (CELL - 0.8);
        let turn = noise(index as u32 + 99, i) * PI;
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(0.05, 0.02, 0.2),
            Quat::from_rotation_y(turn),
            Vec3::new(x, 0.01, z),
        );
        mesh.add(Shape::Cube, transform, SPRINKLES[(i % 5) as usize], 0.0);
    }
    // Walls, one side at a time: two pieces and a lintel at a doorway.
    for (side_index, side) in Side::ALL.iter().enumerate() {
        let open = room.doors[side_index];
        let out = side.outward();
        let horizontal = matches!(side, Side::North | Side::South);
        let line = if horizontal {
            c.z + out.z * CELL / 2.0
        } else {
            c.x + out.x * CELL / 2.0
        };
        // The inner face of the wall and the slab behind it.
        let (inner, outer) = if horizontal {
            (line - out.z * half, line)
        } else {
            (line - out.x * half, line)
        };
        let (a0, a1) = if horizontal {
            (x0, x0 + CELL)
        } else {
            (z0, z0 + CELL)
        };
        let mid = (a0 + a1) / 2.0;
        let gap = DOOR / 2.0;
        let piece =
            |mesh: &mut Mesh, s0: f32, s1: f32, y0: f32, y1: f32, colour: u32, grow: f32| {
                let (lo, hi) = (inner.min(outer), inner.max(outer));
                let (lo, hi) = if inner < outer {
                    (lo - grow, hi)
                } else {
                    (lo, hi + grow)
                };
                if horizontal {
                    boxed(
                        mesh,
                        Vec3::new(s0, y0, lo),
                        Vec3::new(s1, y1, hi),
                        colour,
                        0.0,
                    );
                } else {
                    boxed(
                        mesh,
                        Vec3::new(lo, y0, s0),
                        Vec3::new(hi, y1, s1),
                        colour,
                        0.0,
                    );
                }
            };
        let spans: Vec<(f32, f32)> = if open {
            vec![(a0, mid - gap), (mid + gap, a1)]
        } else {
            vec![(a0, a1)]
        };
        for (s0, s1) in &spans {
            piece(&mut mesh, *s0, *s1, 0.0, WALL_HEIGHT, colour, 0.0);
            // Waffle-cone trim: a baseboard and a crown.
            piece(&mut mesh, *s0, *s1, 0.0, 0.28, WAFFLE, 0.05);
            piece(
                &mut mesh,
                *s0,
                *s1,
                WALL_HEIGHT - 0.22,
                WALL_HEIGHT,
                WAFFLE,
                0.05,
            );
        }
        if open {
            piece(
                &mut mesh,
                mid - gap,
                mid + gap,
                2.45,
                WALL_HEIGHT,
                colour,
                0.0,
            );
            // A waffle frame round the doorway.
            piece(
                &mut mesh,
                mid - gap - 0.12,
                mid - gap,
                0.0,
                2.45,
                WAFFLE_DARK,
                0.06,
            );
            piece(
                &mut mesh,
                mid + gap,
                mid + gap + 0.12,
                0.0,
                2.45,
                WAFFLE_DARK,
                0.06,
            );
            piece(
                &mut mesh,
                mid - gap - 0.12,
                mid + gap + 0.12,
                2.33,
                2.45,
                WAFFLE_DARK,
                0.06,
            );
        }
        // Drips from the crown: melting flavour.
        for i in 0..6 {
            let along = a0 + 0.6 + noise(index as u32 * 7 + side_index as u32, i) * (CELL - 1.2);
            if open && (along - mid).abs() < gap + 0.3 {
                continue;
            }
            let length = 0.15 + noise(index as u32, 50 + i) * 0.45;
            let at = if horizontal {
                Vec3::new(
                    along,
                    WALL_HEIGHT - 0.22 - length / 2.0,
                    inner - out.z * 0.04,
                )
            } else {
                Vec3::new(
                    inner - out.x * 0.04,
                    WALL_HEIGHT - 0.22 - length / 2.0,
                    along,
                )
            };
            let transform = Mat4::from_scale_rotation_translation(
                Vec3::new(0.16, length, 0.16),
                Quat::from_rotation_x(PI),
                at,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 6,
                    bottom: 0.5,
                    top: 0.15,
                },
                transform,
                colour,
                0.0,
            );
            let tip = Mat4::from_scale_rotation_translation(
                Vec3::splat(0.11),
                Quat::IDENTITY,
                at - Vec3::Y * (length / 2.0),
            );
            mesh.add(
                Shape::Sphere {
                    rings: 4,
                    segments: 6,
                },
                tip,
                colour,
                0.0,
            );
        }
    }
    // Ceiling drips in the middle of the room.
    for i in 0..5 {
        let x = c.x - 3.5 + noise(index as u32 + 7, i) * 7.0;
        let z = c.z - 3.5 + noise(index as u32 + 9, i) * 7.0;
        let length = 0.2 + noise(index as u32 + 3, i) * 0.35;
        let at = Vec3::new(x, WALL_HEIGHT - length / 2.0, z);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(0.18, length, 0.18),
            Quat::from_rotation_x(PI),
            at,
        );
        let flavour = wall(Flavor::ALL[(hash(index as u32, i) % 4) as usize]);
        mesh.add(
            Shape::Frustum {
                sides: 6,
                bottom: 0.5,
                top: 0.1,
            },
            transform,
            flavour,
            0.0,
        );
    }
    for (p, block) in props_of(house, index) {
        prop(&mut mesh, p, &block, c, room.flavor);
    }
    if house.front.room == index {
        front_door(&mut mesh, house);
    }
    mesh
}

/// A room's furniture in world space.
fn props_of(house: &House, index: usize) -> Vec<(PropKind, Block)> {
    let c = centre(house.rooms[index].cell);
    house
        .props(index)
        .iter()
        .map(|p| {
            (
                p.kind,
                Block {
                    min: c + p.min,
                    max: c + p.max,
                    height: p.height,
                },
            )
        })
        .collect()
}

fn prop(mesh: &mut Mesh, kind: PropKind, b: &Block, room: V2, flavor: Flavor) {
    let lo = v3(b.min, 0.0);
    let hi = v3(b.max, b.height);
    let mid = (lo + hi) * 0.5;
    let size = hi - lo;
    let f = front(b, room);
    // A point on the front face at a height.
    let face = |y: f32| Vec3::new(mid.x + f.x * size.x / 2.0, y, mid.z + f.z * size.z / 2.0);
    let along_face = Vec3::new(f.z.abs(), 0.0, f.x.abs());
    let w = 1.0;
    match kind {
        PropKind::Bed => {
            boxed(mesh, lo, Vec3::new(hi.x, 0.35, hi.z), 0x8A5A40, w);
            boxed(
                mesh,
                lo + Vec3::new(0.08, 0.35, 0.08),
                Vec3::new(hi.x - 0.08, 0.55, hi.z - 0.08),
                WHITE,
                w,
            );
            // The blanket covers the half toward the room; the pillow
            // sits at the wall.
            let long_x = size.x > size.z;
            let (blanket_lo, blanket_hi, pillow_lo, pillow_hi) = if long_x {
                let toward = if room.x > mid.x { 1.0 } else { -1.0 };
                let split = mid.x - toward * size.x * 0.1;
                let (a, bnd) = if toward > 0.0 {
                    (split, hi.x)
                } else {
                    (lo.x, split)
                };
                let wall_end = if toward > 0.0 { lo.x } else { hi.x };
                (
                    Vec3::new(a, 0.55, lo.z + 0.05),
                    Vec3::new(bnd, 0.62, hi.z - 0.05),
                    Vec3::new(
                        wall_end.min(wall_end + toward * 0.7),
                        0.55,
                        mid.z - size.z * 0.3,
                    ),
                    Vec3::new(
                        wall_end.max(wall_end + toward * 0.7),
                        0.72,
                        mid.z + size.z * 0.3,
                    ),
                )
            } else {
                let toward = if room.z > mid.z { 1.0 } else { -1.0 };
                let split = mid.z - toward * size.z * 0.1;
                let (a, bnd) = if toward > 0.0 {
                    (split, hi.z)
                } else {
                    (lo.z, split)
                };
                let wall_end = if toward > 0.0 { lo.z } else { hi.z };
                (
                    Vec3::new(lo.x + 0.05, 0.55, a),
                    Vec3::new(hi.x - 0.05, 0.62, bnd),
                    Vec3::new(
                        mid.x - size.x * 0.3,
                        0.55,
                        wall_end.min(wall_end + toward * 0.7),
                    ),
                    Vec3::new(
                        mid.x + size.x * 0.3,
                        0.72,
                        wall_end.max(wall_end + toward * 0.7),
                    ),
                )
            };
            boxed(mesh, blanket_lo, blanket_hi, wall(Flavor::Strawberry), w);
            boxed(mesh, pillow_lo, pillow_hi, 0xFFDCE8, w);
        }
        PropKind::Closet => {
            boxed(mesh, lo, hi, 0xE8C99B, w);
            // Two doors: a seam and two knobs on the front.
            let seam = face(hi.y / 2.0);
            boxed(
                mesh,
                seam - Vec3::new(0.03, hi.y / 2.0 - 0.1, 0.03),
                seam + Vec3::new(0.03, hi.y / 2.0 - 0.1, 0.03) + f * 0.02,
                0x9C7650,
                0.0,
            );
            for s in [-1.0, 1.0] {
                let knob = face(1.2) + along_face * (0.18 * s) + f * 0.05;
                boxed(
                    mesh,
                    knob - Vec3::splat(0.05),
                    knob + Vec3::splat(0.05),
                    GOLD,
                    0.0,
                );
            }
        }
        PropKind::Dresser => {
            boxed(mesh, lo, hi, 0xC98B5E, w);
            for i in 0..3 {
                let y = 0.2 + i as f32 * 0.3;
                let line = face(y);
                boxed(
                    mesh,
                    line - along_face * (size.x.max(size.z) * 0.4) - Vec3::Y * 0.02,
                    line + along_face * (size.x.max(size.z) * 0.4) + Vec3::Y * 0.02 + f * 0.02,
                    0x8A5A40,
                    0.0,
                );
            }
        }
        PropKind::Counter => {
            boxed(mesh, lo, Vec3::new(hi.x, hi.y - 0.1, hi.z), 0x9ED9C0, w);
            boxed(
                mesh,
                Vec3::new(lo.x - 0.03, hi.y - 0.1, lo.z - 0.03),
                hi + Vec3::new(0.03, 0.0, 0.03),
                WHITE,
                w,
            );
        }
        PropKind::Fridge => {
            boxed(mesh, lo, hi, 0xF2F5F8, w);
            let handle = face(hi.y * 0.55) + along_face * (size.x.min(size.z) * 0.3) + f * 0.04;
            boxed(
                mesh,
                handle - Vec3::new(0.04, 0.35, 0.04),
                handle + Vec3::new(0.04, 0.35, 0.04),
                0x9AA4AE,
                0.0,
            );
            let magnet = face(hi.y * 0.75) - along_face * 0.2 + f * 0.03;
            boxed(
                mesh,
                magnet - Vec3::splat(0.07),
                magnet + Vec3::splat(0.07),
                0xFF6FA8,
                0.0,
            );
            let seam = face(hi.y * 0.7) + f * 0.01;
            boxed(
                mesh,
                seam - along_face * (size.x.min(size.z) * 0.48) - Vec3::Y * 0.015,
                seam + along_face * (size.x.min(size.z) * 0.48) + Vec3::Y * 0.015,
                0xB8C2CC,
                0.0,
            );
        }
        PropKind::Table => {
            let top = hi.y;
            boxed(mesh, Vec3::new(lo.x, top - 0.08, lo.z), hi, 0xC98B5E, w);
            for (x, z) in [
                (lo.x + 0.1, lo.z + 0.1),
                (hi.x - 0.1, lo.z + 0.1),
                (lo.x + 0.1, hi.z - 0.1),
                (hi.x - 0.1, hi.z - 0.1),
            ] {
                boxed(
                    mesh,
                    Vec3::new(x - 0.05, 0.0, z - 0.05),
                    Vec3::new(x + 0.05, top - 0.08, z + 0.05),
                    0x8A5A40,
                    w,
                );
            }
            // A bowl of ice cream on top.
            let bowl = Vec3::new(mid.x, top + 0.08, mid.z);
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.3,
                    top: 0.5,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.4, 0.16, 0.4),
                    Quat::IDENTITY,
                    bowl,
                ),
                WHITE,
                w,
            );
            mesh.add(
                Shape::Sphere {
                    rings: 4,
                    segments: 8,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.26),
                    Quat::IDENTITY,
                    bowl + Vec3::Y * 0.12,
                ),
                wall(Flavor::Strawberry),
                w,
            );
        }
        PropKind::Sofa => {
            let colour = wall(Flavor::Strawberry);
            boxed(mesh, lo, Vec3::new(hi.x, 0.45, hi.z), 0xE77FA0, w);
            // The back on the side away from the room.
            let back_lo = Vec3::new(
                if f.x > 0.0 {
                    lo.x
                } else if f.x < 0.0 {
                    hi.x - 0.25
                } else {
                    lo.x
                },
                0.45,
                if f.z > 0.0 {
                    lo.z
                } else if f.z < 0.0 {
                    hi.z - 0.25
                } else {
                    lo.z
                },
            );
            let back_hi = Vec3::new(
                if f.x > 0.0 { lo.x + 0.25 } else { hi.x },
                hi.y,
                if f.z > 0.0 { lo.z + 0.25 } else { hi.z },
            );
            boxed(mesh, back_lo, back_hi, colour, w);
            boxed(
                mesh,
                lo + Vec3::new(0.1, 0.45, 0.1),
                Vec3::new(hi.x - 0.1, 0.55, hi.z - 0.1),
                0xFFDCE8,
                w,
            );
        }
        PropKind::Tv => {
            boxed(mesh, lo, Vec3::new(hi.x, 0.6, hi.z), 0x8A5A40, w);
            let screen_lo = Vec3::new(
                mid.x - along_face.x * size.x * 0.45 - f.x.abs() * 0.05,
                0.6,
                mid.z - along_face.z * size.z * 0.45 - f.z.abs() * 0.05,
            );
            let screen_hi = Vec3::new(
                mid.x + along_face.x * size.x * 0.45 + f.x.abs() * 0.05,
                hi.y,
                mid.z + along_face.z * size.z * 0.45 + f.z.abs() * 0.05,
            );
            boxed(mesh, screen_lo, screen_hi, 0x22242A, w);
            let glow = Vec3::new(mid.x, (0.6 + hi.y) / 2.0, mid.z) + f * 0.06;
            boxed(
                mesh,
                glow - along_face * (size.x.max(size.z) * 0.4) - Vec3::Y * 0.3,
                glow + along_face * (size.x.max(size.z) * 0.4) + Vec3::Y * 0.3,
                0x7FC8E8,
                0.0,
            );
        }
        PropKind::Tub => {
            boxed(mesh, lo, hi, WHITE, w);
            boxed(
                mesh,
                lo + Vec3::new(0.15, 0.0, 0.15),
                Vec3::new(hi.x - 0.15, hi.y + 0.01, hi.z - 0.15),
                0x9FD8F0,
                0.0,
            );
        }
        PropKind::Toilet => {
            boxed(
                mesh,
                lo + Vec3::new(0.1, 0.0, 0.1),
                Vec3::new(hi.x - 0.1, 0.45, hi.z - 0.1),
                WHITE,
                w,
            );
            let tank_lo = Vec3::new(
                if f.x > 0.0 {
                    lo.x
                } else if f.x < 0.0 {
                    hi.x - 0.25
                } else {
                    lo.x
                },
                0.45,
                if f.z > 0.0 {
                    lo.z
                } else if f.z < 0.0 {
                    hi.z - 0.25
                } else {
                    lo.z
                },
            );
            let tank_hi = Vec3::new(
                if f.x > 0.0 { lo.x + 0.25 } else { hi.x },
                hi.y,
                if f.z > 0.0 { lo.z + 0.25 } else { hi.z },
            );
            boxed(mesh, tank_lo, tank_hi, WHITE, w);
        }
        PropKind::Sink => {
            boxed(mesh, lo, hi, WHITE, w);
            boxed(
                mesh,
                Vec3::new(mid.x - 0.25, hi.y, mid.z - 0.25),
                Vec3::new(mid.x + 0.25, hi.y + 0.02, mid.z + 0.25),
                0x9FD8F0,
                0.0,
            );
        }
        PropKind::Washer => {
            boxed(mesh, lo, hi, WHITE, w);
            let door = face(hi.y * 0.5) + f * 0.02;
            let turn = if f.x.abs() > 0.0 {
                Quat::from_rotation_z(FRAC_PI_2 * f.x.signum() * -1.0)
            } else {
                Quat::from_rotation_x(FRAC_PI_2 * f.z.signum())
            };
            mesh.add(
                Shape::Disc { sides: 12 },
                Mat4::from_scale_rotation_translation(Vec3::splat(0.7), turn, door),
                0x3C4A5A,
                0.0,
            );
        }
        PropKind::Shelf => {
            boxed(mesh, lo, hi, 0x8A5A40, w);
            for i in 0..5 {
                let x = lo.x + 0.3 + i as f32 * (size.x - 0.6).max(0.1) / 4.0;
                let tub = Vec3::new(x, hi.y + 0.12, mid.z);
                mesh.add(
                    Shape::Frustum {
                        sides: 8,
                        bottom: 0.4,
                        top: 0.5,
                    },
                    Mat4::from_scale_rotation_translation(
                        Vec3::new(0.35, 0.25, 0.35),
                        Quat::IDENTITY,
                        tub,
                    ),
                    wall(Flavor::ALL[i % 4]),
                    w,
                );
            }
        }
        PropKind::Stairs => {
            // Steps that climb into the ceiling and go nowhere.
            let steps = 8;
            let long_x = size.x > size.z;
            for i in 0..steps {
                let t0 = i as f32 / steps as f32;
                let t1 = (i + 1) as f32 / steps as f32;
                let y = hi.y * t1;
                let (a, bnd) = if long_x {
                    (
                        Vec3::new(lo.x + size.x * t0, 0.0, lo.z),
                        Vec3::new(lo.x + size.x * t1, y, hi.z),
                    )
                } else {
                    (
                        Vec3::new(lo.x, 0.0, lo.z + size.z * t0),
                        Vec3::new(hi.x, y, lo.z + size.z * t1),
                    )
                };
                boxed(
                    mesh,
                    a,
                    bnd,
                    if i % 2 == 0 {
                        carpet(flavor)
                    } else {
                        wall(flavor)
                    },
                    w,
                );
            }
        }
        PropKind::Freezer => {
            boxed(mesh, lo, Vec3::new(hi.x, hi.y - 0.08, hi.z), 0xDDEFFF, w);
            boxed(
                mesh,
                Vec3::new(lo.x - 0.03, hi.y - 0.08, lo.z - 0.03),
                hi + Vec3::new(0.03, 0.0, 0.03),
                0xBFE3FF,
                w,
            );
        }
        PropKind::Pillar => {
            boxed(mesh, lo, hi, wall(flavor), 0.0);
            boxed(
                mesh,
                lo - Vec3::new(0.05, 0.0, 0.05),
                Vec3::new(hi.x + 0.05, 0.28, hi.z + 0.05),
                WAFFLE,
                0.0,
            );
            boxed(
                mesh,
                Vec3::new(lo.x - 0.05, hi.y - 0.22, lo.z - 0.05),
                hi + Vec3::new(0.05, 0.0, 0.05),
                WAFFLE,
                0.0,
            );
        }
        PropKind::Plant => {
            // An ice cream plant: a waffle pot with a scoop on top.
            let pot = Vec3::new(mid.x, 0.35, mid.z);
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.3,
                    top: 0.5,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.8, 0.7, 0.8),
                    Quat::IDENTITY,
                    pot,
                ),
                WAFFLE,
                w,
            );
            mesh.add(
                Shape::Sphere {
                    rings: 5,
                    segments: 8,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.7),
                    Quat::IDENTITY,
                    Vec3::new(mid.x, 0.85, mid.z),
                ),
                wall(Flavor::Mint),
                w,
            );
        }
    }
}

/// The front door: a chocolate door with three keyholes and a green
/// sign above it.
fn front_door(mesh: &mut Mesh, house: &House) {
    let f = house.front;
    let out = Vec3::new(f.side.outward().x, 0.0, f.side.outward().z);
    let along = Vec3::new(out.z.abs(), 0.0, out.x.abs());
    let face = Vec3::new(f.at.x, 0.0, f.at.z) - out * 0.17;
    let lo = face - along * 0.75 - out * 0.06;
    let hi = face + along * 0.75 + Vec3::Y * 2.3;
    boxed(mesh, lo.min(hi), lo.max(hi), 0x6B3A1E, 1.0);
    for i in 0..3 {
        let hole = face + Vec3::Y * (0.9 + i as f32 * 0.22) + along * 0.5 - out * 0.08;
        boxed(
            mesh,
            hole - Vec3::splat(0.07),
            hole + Vec3::splat(0.07),
            GOLD,
            0.0,
        );
    }
    let knob = face + Vec3::Y * 1.1 + along * 0.55 - out * 0.12;
    mesh.add(
        Shape::Sphere {
            rings: 4,
            segments: 8,
        },
        Mat4::from_scale_rotation_translation(Vec3::splat(0.14), Quat::IDENTITY, knob),
        GOLD,
        1.0,
    );
    let sign = face + Vec3::Y * 2.6 - out * 0.06;
    let half = along * 0.5 + Vec3::Y * 0.15 + out * 0.04;
    boxed(mesh, sign - half, sign + half, 0x3BB273, 1.0);
}

/// A room's ceiling lights: flat panels drawn in their own colour.
#[must_use]
pub fn lights(house: &House, index: usize) -> Mesh {
    let mut mesh = Mesh::new();
    let c = centre(house.rooms[index].cell);
    for offset in [-2.5_f32, 2.5] {
        let at = Vec3::new(c.x + offset, WALL_HEIGHT - 0.03, c.z);
        boxed(
            &mut mesh,
            at - Vec3::new(0.35, 0.03, 0.9),
            at + Vec3::new(0.35, 0.03, 0.9),
            WHITE,
            0.0,
        );
    }
    mesh
}

/// The avatar's parts.
pub struct Avatar {
    pub torso: Mesh,
    pub head: Mesh,
    pub arm: Mesh,
    pub leg: Mesh,
}

/// The blocky avatar, facing +z; the arm and leg hang from their joint at
/// the origin.
#[must_use]
pub fn avatar() -> Avatar {
    let mut torso = Mesh::new();
    boxed(
        &mut torso,
        Vec3::new(-0.4, 0.8, -0.2),
        Vec3::new(0.4, 1.55, 0.2),
        SHIRT,
        1.0,
    );
    // A cone on the shirt.
    boxed(
        &mut torso,
        Vec3::new(-0.08, 1.05, 0.2),
        Vec3::new(0.08, 1.3, 0.22),
        WAFFLE,
        0.0,
    );
    boxed(
        &mut torso,
        Vec3::new(-0.12, 1.3, 0.2),
        Vec3::new(0.12, 1.42, 0.22),
        wall(Flavor::Strawberry),
        0.0,
    );
    let mut head = Mesh::new();
    boxed(
        &mut head,
        Vec3::new(-0.28, 1.57, -0.26),
        Vec3::new(0.28, 2.07, 0.26),
        SKIN,
        1.0,
    );
    for x in [-0.11, 0.11] {
        boxed(
            &mut head,
            Vec3::new(x - 0.04, 1.84, 0.26),
            Vec3::new(x + 0.04, 1.94, 0.28),
            INK,
            0.0,
        );
    }
    // A smile.
    boxed(
        &mut head,
        Vec3::new(-0.12, 1.7, 0.26),
        Vec3::new(0.12, 1.73, 0.28),
        INK,
        0.0,
    );
    boxed(
        &mut head,
        Vec3::new(-0.15, 1.73, 0.26),
        Vec3::new(-0.11, 1.77, 0.28),
        INK,
        0.0,
    );
    boxed(
        &mut head,
        Vec3::new(0.11, 1.73, 0.26),
        Vec3::new(0.15, 1.77, 0.28),
        INK,
        0.0,
    );
    // Hair.
    boxed(
        &mut head,
        Vec3::new(-0.3, 1.98, -0.28),
        Vec3::new(0.3, 2.12, 0.28),
        0x6B3A1E,
        1.0,
    );
    let mut arm = Mesh::new();
    boxed(
        &mut arm,
        Vec3::new(-0.13, -0.38, -0.13),
        Vec3::new(0.13, 0.0, 0.13),
        SHIRT,
        1.0,
    );
    boxed(
        &mut arm,
        Vec3::new(-0.13, -0.72, -0.13),
        Vec3::new(0.13, -0.38, 0.13),
        SKIN,
        1.0,
    );
    let mut leg = Mesh::new();
    boxed(
        &mut leg,
        Vec3::new(-0.18, -0.8, -0.18),
        Vec3::new(0.18, 0.0, 0.18),
        PANTS,
        1.0,
    );
    boxed(
        &mut leg,
        Vec3::new(-0.19, -0.8, -0.2),
        Vec3::new(0.19, -0.68, 0.24),
        0x3A3A3A,
        1.0,
    );
    Avatar {
        torso,
        head,
        arm,
        leg,
    }
}

/// A tool in the hand, pointing along +z from the grip.
#[must_use]
pub fn tool(tool: icecream_rules::Tool) -> Mesh {
    let mut mesh = Mesh::new();
    match tool {
        icecream_rules::Tool::Fudge => {
            boxed(
                &mut mesh,
                Vec3::new(-0.1, -0.12, -0.15),
                Vec3::new(0.1, 0.1, 0.45),
                0x6B3A1E,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.5,
                    top: 0.3,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.16, 0.3, 0.16),
                    Quat::from_rotation_x(FRAC_PI_2),
                    Vec3::new(0.0, 0.0, 0.6),
                ),
                0xC9A27E,
                1.0,
            );
            // The fudge tank.
            mesh.add(
                Shape::Sphere {
                    rings: 4,
                    segments: 8,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.26),
                    Quat::IDENTITY,
                    Vec3::new(0.0, 0.2, 0.05),
                ),
                FUDGE,
                1.0,
            );
        }
        icecream_rules::Tool::Fart => {
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.3,
                    top: 0.3,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.24, 0.6, 0.24),
                    Quat::from_rotation_x(FRAC_PI_2),
                    Vec3::new(0.0, 0.0, 0.2),
                ),
                0x8DC63F,
                1.0,
            );
            mesh.add(
                Shape::Frustum {
                    sides: 8,
                    bottom: 0.25,
                    top: 0.5,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.4, 0.2, 0.4),
                    Quat::from_rotation_x(FRAC_PI_2),
                    Vec3::new(0.0, 0.0, 0.58),
                ),
                0xB6E06A,
                1.0,
            );
        }
        icecream_rules::Tool::Keys => {
            mesh.add(
                Shape::Torus {
                    segments: 12,
                    sides: 4,
                    radius: 0.14,
                    tube: 0.025,
                },
                Mat4::from_translation(Vec3::new(0.0, 0.0, 0.1)),
                GOLD,
                1.0,
            );
            for i in 0..3 {
                let a = i as f32 * 0.5 - 0.5;
                boxed(
                    &mut mesh,
                    Vec3::new(a * 0.2 - 0.025, -0.3, 0.08),
                    Vec3::new(a * 0.2 + 0.025, -0.1, 0.12),
                    GOLD,
                    1.0,
                );
            }
        }
    }
    mesh
}

/// The Ice Cream Man's parts.
pub struct ManModel {
    /// The waffle-cone body and its rim.
    pub body: Mesh,
    /// One leg, hanging from the hip at the origin.
    pub leg: Mesh,
    /// One arm, hanging from the shoulder at the origin.
    pub arm: Mesh,
    /// The vanilla scoop with its tiny eyes, big grin and cheeks.
    pub head: Mesh,
    /// The huge bald forehead, drawn shiny.
    pub forehead: Mesh,
}

/// The Ice Cream Man, facing +z, about 3 m tall with his forehead.
#[must_use]
pub fn man() -> ManModel {
    let mut body = Mesh::new();
    let cone = Mat4::from_scale_rotation_translation(
        Vec3::new(1.0, 1.25, 1.0),
        Quat::IDENTITY,
        Vec3::new(0.0, 1.38, 0.0),
    );
    body.add(
        Shape::Frustum {
            sides: 12,
            bottom: 0.2,
            top: 0.6,
        },
        cone,
        WAFFLE,
        1.0,
    );
    // The waffle pattern: darker rings round the cone.
    for i in 0..4 {
        let t = (i as f32 + 0.6) / 4.6;
        let y = 0.76 + t * 1.25;
        let r = 0.2 + t * 0.4 + 0.01;
        body.add(
            Shape::Torus {
                segments: 14,
                sides: 3,
                radius: r,
                tube: 0.025,
            },
            Mat4::from_rotation_translation(
                Quat::from_rotation_x(FRAC_PI_2),
                Vec3::new(0.0, y, 0.0),
            ),
            WAFFLE_DARK,
            0.0,
        );
    }
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let lo = Vec3::new(0.0, 0.8, 0.0) + dir * 0.24;
        let hi = Vec3::new(0.0, 1.98, 0.0) + dir * 0.6;
        let mid = (lo + hi) * 0.5;
        let up = (hi - lo).normalize();
        let rotation = Quat::from_rotation_arc(Vec3::Y, up);
        body.add(
            Shape::Cube,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.035, (hi - lo).length(), 0.035),
                rotation,
                mid + dir * 0.012,
            ),
            WAFFLE_DARK,
            0.0,
        );
    }
    body.add(
        Shape::Torus {
            segments: 14,
            sides: 4,
            radius: 0.6,
            tube: 0.07,
        },
        Mat4::from_rotation_translation(Quat::from_rotation_x(FRAC_PI_2), Vec3::new(0.0, 2.0, 0.0)),
        0xE8B878,
        1.0,
    );
    let mut leg = Mesh::new();
    leg.add(
        Shape::Frustum {
            sides: 8,
            bottom: 0.5,
            top: 0.5,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.2, 0.8, 0.2),
            Quat::IDENTITY,
            Vec3::new(0.0, -0.4, 0.0),
        ),
        WAFFLE,
        1.0,
    );
    boxed(
        &mut leg,
        Vec3::new(-0.13, -0.85, -0.12),
        Vec3::new(0.13, -0.72, 0.22),
        0x6B3A1E,
        1.0,
    );
    let mut arm = Mesh::new();
    arm.add(
        Shape::Frustum {
            sides: 8,
            bottom: 0.4,
            top: 0.6,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.16, 0.7, 0.16),
            Quat::IDENTITY,
            Vec3::new(0.0, -0.35, 0.0),
        ),
        WAFFLE,
        1.0,
    );
    arm.add(
        Shape::Sphere {
            rings: 5,
            segments: 8,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::splat(0.24),
            Quat::IDENTITY,
            Vec3::new(0.0, -0.75, 0.0),
        ),
        WHITE,
        1.0,
    );
    let mut head = Mesh::new();
    // The scoop sits in the cone; its rim drips over.
    head.add(
        Shape::Sphere {
            rings: 8,
            segments: 14,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.3, 0.9, 1.3),
            Quat::IDENTITY,
            Vec3::new(0.0, 2.3, 0.0),
        ),
        VANILLA,
        1.0,
    );
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        let r = 0.6;
        let drop = 0.12 + ((i * 7) % 4) as f32 * 0.05;
        head.add(
            Shape::Sphere {
                rings: 4,
                segments: 6,
            },
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.2, 0.2 + drop, 0.2),
                Quat::IDENTITY,
                Vec3::new(a.cos() * r, 2.0 - drop / 2.0, a.sin() * r),
            ),
            VANILLA,
            0.0,
        );
    }
    // Where the scoop's front surface is, at a height and across.
    let surface = |x: f32, y: f32| {
        let (rx, ry) = (0.65_f32, 0.45_f32);
        rx * (1.0 - (x / rx).powi(2) - ((y - 2.3) / ry).powi(2))
            .max(0.0)
            .sqrt()
    };
    // Tiny eyes, low on the face.
    for x in [-0.12_f32, 0.12] {
        head.add(
            Shape::Sphere {
                rings: 4,
                segments: 6,
            },
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.1),
                Quat::IDENTITY,
                Vec3::new(x, 2.5, surface(x, 2.5) + 0.01),
            ),
            INK,
            0.0,
        );
    }
    // A wide grin: a dark arc with a row of teeth.
    for i in 0..11 {
        let t = i as f32 / 10.0;
        let a = PI * (1.15 + 0.7 * t);
        let p = Vec3::new(a.cos() * 0.34, 2.44 + a.sin() * 0.24, 0.0);
        let z = surface(p.x, p.y);
        boxed(
            &mut head,
            Vec3::new(p.x - 0.045, p.y - 0.04, z - 0.03),
            Vec3::new(p.x + 0.045, p.y + 0.04, z + 0.03),
            0x8A1E2E,
            0.0,
        );
        if (2..9).contains(&i) {
            boxed(
                &mut head,
                Vec3::new(p.x - 0.03, p.y + 0.03, z - 0.02),
                Vec3::new(p.x + 0.03, p.y + 0.07, z + 0.035),
                WHITE,
                0.0,
            );
        }
    }
    // Pink cheeks with sprinkles stuck on them.
    for side in [-1.0_f32, 1.0] {
        let cheek = Vec3::new(side * 0.38, 2.32, surface(side * 0.38, 2.32) - 0.01);
        head.add(
            Shape::Sphere {
                rings: 4,
                segments: 8,
            },
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.2, 0.14, 0.08),
                Quat::IDENTITY,
                cheek,
            ),
            0xFF9DB8,
            0.0,
        );
        for i in 0..4 {
            let o = Vec3::new(
                side * (0.05 * (i % 2) as f32 - 0.03),
                0.06 * (i as f32 - 1.5),
                0.05,
            );
            let turn = Quat::from_rotation_z(0.6 * i as f32 + side);
            head.add(
                Shape::Cube,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.025, 0.08, 0.025),
                    turn,
                    cheek + o,
                ),
                SPRINKLES[i % 5],
                0.0,
            );
        }
    }
    let mut forehead = Mesh::new();
    // Most of the scoop is forehead: a big shiny dome.
    forehead.add(
        Shape::Sphere {
            rings: 10,
            segments: 16,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.2, 1.25, 1.15),
            Quat::IDENTITY,
            Vec3::new(0.0, 2.72, -0.04),
        ),
        FOREHEAD,
        1.0,
    );
    forehead.add(
        Shape::Sphere {
            rings: 4,
            segments: 8,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.2, 0.12, 0.06),
            Quat::from_rotation_x(-0.6),
            Vec3::new(0.18, 3.06, 0.4),
        ),
        0xFFFFFF,
        0.0,
    );
    ManModel {
        body,
        leg,
        arm,
        head,
        forehead,
    }
}

/// A check-out key, standing up, spun about y when drawn.
#[must_use]
pub fn key() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Torus {
            segments: 12,
            sides: 4,
            radius: 0.16,
            tube: 0.05,
        },
        Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0)),
        GOLD,
        1.0,
    );
    boxed(
        &mut mesh,
        Vec3::new(-0.04, -0.3, -0.04),
        Vec3::new(0.04, 0.1, 0.04),
        GOLD,
        1.0,
    );
    boxed(
        &mut mesh,
        Vec3::new(0.04, -0.3, -0.03),
        Vec3::new(0.14, -0.22, 0.03),
        GOLD,
        1.0,
    );
    boxed(
        &mut mesh,
        Vec3::new(0.04, -0.16, -0.03),
        Vec3::new(0.12, -0.09, 0.03),
        GOLD,
        1.0,
    );
    mesh
}

/// A ball of one colour, radius 0.5.
#[must_use]
pub fn ball(colour: u32) -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Sphere {
            rings: 5,
            segments: 8,
        },
        Mat4::IDENTITY,
        colour,
        1.0,
    );
    mesh
}

/// A sprinkle, long along +z.
#[must_use]
pub fn sprinkle(colour: u32) -> Mesh {
    let mut mesh = Mesh::new();
    boxed(
        &mut mesh,
        Vec3::new(-0.04, -0.04, -0.13),
        Vec3::new(0.04, 0.04, 0.13),
        colour,
        0.6,
    );
    mesh
}

/// A flat splat on the floor, radius 0.5.
#[must_use]
pub fn splat(colour: u32) -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(Shape::Disc { sides: 10 }, Mat4::IDENTITY, colour, 0.0);
    mesh
}

/// The cone you end up in when scooped.
#[must_use]
pub fn big_cone() -> Mesh {
    let mut mesh = Mesh::new();
    mesh.add(
        Shape::Frustum {
            sides: 12,
            bottom: 0.05,
            top: 0.6,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.6, 1.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.8, 0.0),
        ),
        WAFFLE,
        1.0,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_room_of_a_house_has_walls_furniture_and_lights() {
        let house = House::new(7);
        for index in 0..house.rooms.len() {
            let mesh = room(&house, index);
            assert!(mesh.vertices() > 500, "room {index}");
            assert!(mesh.vertices().is_multiple_of(3));
            assert!(lights(&house, index).vertices() > 0);
        }
    }

    #[test]
    fn the_characters_and_items_are_built() {
        let a = avatar();
        for mesh in [&a.torso, &a.head, &a.arm, &a.leg] {
            assert!(mesh.vertices() >= 36);
        }
        let m = man();
        for mesh in [&m.body, &m.leg, &m.arm, &m.head, &m.forehead] {
            assert!(mesh.vertices() >= 36);
        }
        // He is tall and his forehead tops him out.
        let top = m
            .forehead
            .data
            .chunks(crate::mesh::STRIDE)
            .map(|v| v[1])
            .fold(0.0_f32, f32::max);
        assert!(top > 3.0 && top < 3.4, "{top}");
        for t in icecream_rules::Tool::ALL {
            assert!(tool(t).vertices() > 0);
        }
        for mesh in [
            key(),
            ball(FUDGE),
            sprinkle(SPRINKLES[0]),
            splat(FUDGE),
            big_cone(),
        ] {
            assert!(mesh.vertices() > 0);
        }
    }

    #[test]
    fn the_walls_are_pastel_ice_cream() {
        let strawberry = wall(Flavor::Strawberry);
        assert!(strawberry >> 16 > 0xE0, "pink: lots of red");
        assert_ne!(wall(Flavor::Mint), wall(Flavor::Vanilla));
        assert!(cold(Module::FreezerRoom) && !cold(Module::Bedroom));
    }
}
