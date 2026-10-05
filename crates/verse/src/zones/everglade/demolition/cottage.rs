//! The demolition yard's two cottages, built from the Medieval Village kit
//! as Everglade's `layout::house` builds the Stoop Lane cottage: 2 m wall
//! sections on an 8 m by 10 m footprint, corner posts, a round-tile roof
//! with brick gables, and a chimney. The pieces themselves are the shared
//! [`super::kit`]'s.

use super::kit::{self, CORNER_TRIM, SEAM, WALL_TOP, place};
use super::site::{PieceSpec, Role, Side};
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

pub use super::kit::{Cut, Draft};

/// The cottages: footprint center and half extents, m. Both stand in the
/// flat clearing north of the spawn point, clear of the hall's interior,
/// whose camera rule would hold the view inside a hall that isn't here.
pub const COTTAGES: [([f32; 2], [f32; 2]); 2] =
    [([-6.0, -8.0], [4.0, 5.0]), ([7.0, -5.0], [4.0, 5.0])];

/// How a wall section is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    Plain,
    Timber,
    Round,
    Flat,
    Door,
}

/// Each cottage's walls: south and north from the west end, west and east
/// from the south end.
const WALLS: [[&[Fill]; 4]; 2] = {
    use Fill::{Door, Flat, Plain, Round, Timber};
    [
        [
            &[Timber, Door, Plain, Round],
            &[Plain, Flat, Flat, Plain],
            &[Plain, Flat, Timber, Flat, Plain],
            &[Round, Timber, Plain, Timber, Round],
        ],
        [
            &[Flat, Plain, Door, Flat],
            &[Plain, Round, Round, Plain],
            &[Round, Plain, Timber, Plain, Round],
            &[Plain, Flat, Plain, Flat, Plain],
        ],
    ]
};

/// Every piece of both cottages.
#[must_use]
pub fn drafts() -> Vec<Draft> {
    let mut out = Vec::new();
    for (building, (rect, walls)) in COTTAGES.iter().zip(WALLS).enumerate() {
        cottage(&mut out, building, *rect, walls);
    }
    out
}

fn cottage(out: &mut Vec<Draft>, building: usize, rect: ([f32; 2], [f32; 2]), walls: [&[Fill]; 4]) {
    let ([cx, cz], [hx, hz]) = rect;
    let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
    let lines = [
        (Side::South, PI),
        (Side::North, 0.0),
        (Side::West, -FRAC_PI_2),
        (Side::East, FRAC_PI_2),
    ];
    for ((side, yaw), fills) in lines.into_iter().zip(walls) {
        let count = fills.len();
        for (index, fill) in fills.iter().enumerate() {
            let along = 1.0 + 2.0 * index as f32;
            let at = match side {
                Side::South => [west + along, south],
                Side::North => [west + along, north],
                Side::West => [west, south + along],
                Side::East => [east, south + along],
            };
            // The model's +x runs east on the north line and south on the
            // west line, so which end is the line's first depends on it.
            let reversed = matches!(side, Side::South | Side::East);
            let first_end = index == 0;
            let last_end = index + 1 == count;
            let (low, high) = if reversed {
                (last_end, first_end)
            } else {
                (first_end, last_end)
            };
            out.push(wall(
                building,
                Role::Wall {
                    side,
                    index: index as u8,
                    count: count as u8,
                    story: 0,
                },
                *fill,
                place(at, 0.0, yaw),
                [
                    if low { CORNER_TRIM } else { SEAM },
                    if high { CORNER_TRIM } else { SEAM },
                ],
            ));
        }
    }
    for (a, b, at) in [
        (Side::South, Side::West, [west, south]),
        (Side::South, Side::East, [east, south]),
        (Side::North, Side::West, [west, north]),
        (Side::North, Side::East, [east, north]),
    ] {
        out.push(kit::post(
            building,
            Role::Post { a, b, story: 0 },
            place(at, 0.0, 0.0),
        ));
    }
    out.push(kit::roof(
        building,
        Role::Roof { span: 0, spans: 1 },
        place([cx, cz], WALL_TOP, 0.0),
    ));
    for (side, z, yaw) in [(Side::South, south, PI), (Side::North, north, 0.0)] {
        out.push(kit::gable(
            building,
            Role::Gable { side, span: 0 },
            place([cx, z], WALL_TOP, yaw),
        ));
    }
    out.push(kit::chimney(
        building,
        Role::Chimney { span: 0 },
        place([cx - 2.0, cz + 2.5], 4.9, 0.0),
    ));
}

/// A wall section and its dressing, its collider stopping `trim` short of
/// its low (-x) and high (+x) ends.
fn wall(building: usize, role: Role, fill: Fill, placement: Mat4, trim: [f32; 2]) -> Draft {
    let (host, dressing): (_, Vec<&'static str>) = match fill {
        Fill::Plain => ("village/Wall_Plaster_Straight", vec![]),
        Fill::Timber => ("village/Wall_Plaster_WoodGrid", vec![]),
        Fill::Round => (
            "village/Wall_Plaster_Window_Wide_Round",
            vec![
                "village/Window_Wide_Round1",
                "village/WindowShutters_Wide_Round_Open",
            ],
        ),
        Fill::Flat => (
            "village/Wall_Plaster_Window_Wide_Flat",
            vec!["village/Window_Wide_Flat1"],
        ),
        Fill::Door => (
            "village/Wall_Plaster_Door_Round",
            vec!["village/DoorFrame_Round_WoodDark"],
        ),
    };
    let mut models: Vec<(&'static str, Mat4)> =
        dressing.into_iter().map(|m| (m, Mat4::IDENTITY)).collect();
    if fill == Fill::Door {
        // The leaf stands open inside, hinged at the doorway's side.
        models.push((
            "village/Door_4_Round",
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(FRAC_PI_2),
                Vec3::new(-0.62, 0.02, -0.25),
            ),
        ));
    }
    kit::wall(building, role, host, models, placement, trim)
}

/// Every piece of both cottages ready to raise, chunked by a plain grid.
#[must_use]
pub fn specs_without_meshes() -> Vec<PieceSpec> {
    drafts()
        .iter()
        .map(|draft| draft.spec(draft.grid_chunks()))
        .collect()
}
