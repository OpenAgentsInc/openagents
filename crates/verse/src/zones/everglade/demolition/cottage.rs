//! The demolition yard's two cottages, built from the Medieval Village kit
//! as Everglade's `layout::house` builds the Stoop Lane cottage: 2 m wall
//! sections on an 8 m by 10 m footprint, corner posts, a round-tile roof
//! with brick gables, and a chimney. Each structural piece carries the
//! dressing that breaks with it (glass, shutters, a door frame and leaf),
//! the colliders the solver and the player see, and how it is cut into
//! chunks.

use super::site::{Cuboid, Matter, PieceSpec, Role, Side};
use crate::zones::everglade::height;
use glam::{DVec3, Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

/// Wall tops and the roof's base, m (`layout::WALL_TOP`).
pub const WALL_TOP: f32 = 3.12;
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

/// Kit model bounds, model meters, from the pinned pack. Every wall
/// section shares one outline.
const WALL: ([f32; 3], [f32; 3]) = ([-1.0, 0.0, -0.31], [1.0, 3.12, 0.09]);
const POST: ([f32; 3], [f32; 3]) = ([-0.11, 0.0, -0.12], [0.11, 3.0, 0.12]);
const ROOF: ([f32; 3], [f32; 3]) = ([-4.98, -0.78, -5.85], [4.98, 6.0, 6.01]);
/// How far a wall section's collider stops short of a corner, m: the
/// crossing wall's thickness, so no two colliders overlap.
const CORNER_TRIM: f32 = 0.34;
/// How far a wall section's collider stops short of its neighbor, m.
const SEAM: f32 = 0.01;
/// Half the clear width of a doorway, and its lintel's underside, m.
const DOOR_HALF: f32 = 0.6;
const LINTEL: f32 = 2.5;
/// Roof slab thickness under the tiles, m.
const ROOF_SLAB: f32 = 0.2;

/// One structural piece as placed, before it is raised.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub building: usize,
    pub role: Role,
    pub matter: Matter,
    /// The host model's placement: model space to the world.
    pub placement: Mat4,
    /// Every model drawn with the piece, in the host's model space.
    pub models: Vec<(&'static str, Mat4)>,
    /// Colliders in the host's model space.
    pub colliders: Vec<Cuboid>,
    /// The body's origin in the host's model space.
    pub origin: Vec3,
    pub mass: f64,
    pub hit_points: i32,
}

/// One region of a piece cut into a grid of chunks: the triangles on
/// `side` of the model's x = 0 plane (or all), in a frame `frame` (model
/// space to cut space), split into `grid` cells along the cut axes. With
/// `thickness`, a chunk's box is at most that half thick across an uncut
/// axis, centered on its triangles: a roof's beams and trim would
/// otherwise make every chunk a thick block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub side: Option<bool>,
    pub frame: Quat,
    pub grid: [usize; 3],
    pub thickness: Option<f32>,
}

impl Draft {
    /// How the piece is cut into chunks.
    #[must_use]
    pub fn cuts(&self) -> Vec<Cut> {
        let whole = |grid| {
            vec![Cut {
                side: None,
                frame: Quat::IDENTITY,
                grid,
                thickness: None,
            }]
        };
        match self.role {
            Role::Wall { .. } => whole([3, 3, 1]),
            Role::Post { .. } => whole([1, 3, 1]),
            Role::Gable { .. } => vec![Cut {
                side: None,
                frame: Quat::IDENTITY,
                grid: [3, 2, 1],
                thickness: Some(0.2),
            }],
            Role::Chimney => whole([1, 2, 1]),
            Role::Roof => {
                let angle = slope_angle();
                vec![
                    Cut {
                        side: Some(false),
                        frame: Quat::from_rotation_z(-angle),
                        grid: [2, 1, 3],
                        thickness: Some(0.12),
                    },
                    Cut {
                        side: Some(true),
                        frame: Quat::from_rotation_z(angle),
                        grid: [2, 1, 3],
                        thickness: Some(0.12),
                    },
                ]
            }
        }
    }

    /// Chunk boxes from a plain grid over the colliders' bounds, for a
    /// yard raised without the pack's meshes, in the body frame.
    #[must_use]
    pub fn grid_chunks(&self) -> Vec<Cuboid> {
        let (mut min, mut max) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        for c in &self.colliders {
            let corners = (0..8).map(|i| {
                let s = DVec3::new(
                    if i & 1 == 0 { -1.0 } else { 1.0 },
                    if i & 2 == 0 { -1.0 } else { 1.0 },
                    if i & 4 == 0 { -1.0 } else { 1.0 },
                );
                c.center + c.rotation * (c.half * s)
            });
            for p in corners {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let grid = self.cuts()[0].grid;
        let cell = (max - min) / DVec3::new(grid[0] as f64, grid[1] as f64, grid[2] as f64);
        let origin = self.origin.as_dvec3();
        let mut chunks = Vec::new();
        for i in 0..grid[0] {
            for j in 0..grid[1] {
                for k in 0..grid[2] {
                    let lo = min + cell * DVec3::new(i as f64, j as f64, k as f64);
                    chunks.push(Cuboid::between(lo - origin, lo + cell - origin));
                }
            }
        }
        chunks
    }

    /// The piece ready to raise, with `chunks` in the body frame.
    #[must_use]
    pub fn spec(&self, chunks: Vec<Cuboid>) -> PieceSpec {
        let (scale, rotation, translation) = self.placement.to_scale_rotation_translation();
        debug_assert!((scale - Vec3::ONE).length() < 1e-4);
        let center = translation + rotation * self.origin;
        let origin = self.origin.as_dvec3();
        let colliders: Vec<Cuboid> = self
            .colliders
            .iter()
            .map(|c| Cuboid {
                center: c.center - origin,
                ..*c
            })
            .collect();
        let size = match self.role {
            Role::Wall { .. } => DVec3::new(2.0, 3.12, 0.4),
            Role::Post { .. } => DVec3::new(0.22, 3.0, 0.24),
            Role::Roof => DVec3::new(10.0, 6.8, 11.9),
            Role::Gable { .. } => DVec3::new(6.0, 4.0, 0.5),
            Role::Chimney => DVec3::new(1.0, 3.2, 1.0),
        };
        PieceSpec {
            building: self.building,
            role: self.role,
            matter: self.matter,
            center: center.as_dvec3(),
            orientation: rotation.as_dquat(),
            mass: self.mass,
            size,
            hit_points: self.hit_points,
            colliders,
            chunks,
            blocks: matches!(self.role, Role::Wall { .. } | Role::Post { .. }),
        }
    }
}

/// The roof slope's angle from horizontal, radians.
fn slope_angle() -> f32 {
    let (min, max) = ROOF;
    (max[1] - min[1]).atan2(max[0])
}

/// A placement at `(x, z)`, `lift` above the ground, facing `yaw`.
fn place(at: [f32; 2], lift: f32, yaw: f32) -> Mat4 {
    let [x, z] = at;
    Mat4::from_rotation_translation(
        Quat::from_rotation_y(yaw),
        Vec3::new(x, height(x, z) + lift, z),
    )
}

fn between(min: [f32; 3], max: [f32; 3]) -> Cuboid {
    Cuboid::between(Vec3::from(min).as_dvec3(), Vec3::from(max).as_dvec3())
}

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
        let (min, max) = POST;
        out.push(Draft {
            building,
            role: Role::Post { a, b },
            matter: Matter::Timber,
            placement: place(at, 0.0, 0.0),
            models: vec![("village/Corner_Exterior_Wood", Mat4::IDENTITY)],
            colliders: vec![between(min, max)],
            origin: Vec3::new(0.0, 1.5, 0.0),
            mass: 60.0,
            hit_points: 18,
        });
    }
    out.push(roof(building, place([cx, cz], WALL_TOP, 0.0)));
    for (side, z, yaw) in [(Side::South, south, PI), (Side::North, north, 0.0)] {
        out.push(Draft {
            building,
            role: Role::Gable { side },
            matter: Matter::Brick,
            placement: place([cx, z], WALL_TOP, yaw),
            models: vec![("village/Roof_Front_Brick8", Mat4::IDENTITY)],
            // Two steps under the roof's slopes.
            colliders: vec![
                between([-2.4, 0.05, -0.25], [2.4, 2.2, 0.35]),
                between([-1.0, 2.2, -0.25], [1.0, 4.0, 0.35]),
            ],
            origin: Vec3::new(0.0, 1.6, 0.05),
            mass: 500.0,
            hit_points: 27,
        });
    }
    out.push(Draft {
        building,
        role: Role::Chimney,
        matter: Matter::Brick,
        placement: place([cx - 2.0, cz + 2.5], 4.9, 0.0),
        models: vec![("village/Prop_Chimney", Mat4::IDENTITY)],
        // Only the stack above the tiles collides, so it starts clear of
        // the roof's slab.
        colliders: vec![between([-0.45, 2.1, -0.45], [0.45, 3.18, 0.45])],
        origin: Vec3::new(0.0, 2.6, 0.0),
        mass: 400.0,
        hit_points: 27,
    });
}

/// A wall section and its dressing, its collider stopping `trim` short of
/// its low (-x) and high (+x) ends.
fn wall(building: usize, role: Role, fill: Fill, placement: Mat4, trim: [f32; 2]) -> Draft {
    let (min, max) = WALL;
    let lo = min[0] + trim[0];
    let hi = max[0] - trim[1];
    let solid = vec![between([lo, min[1], min[2]], [hi, max[1], max[2]])];
    let (host, dressing, colliders, matter, hit_points): (_, Vec<&'static str>, _, _, _) =
        match fill {
            Fill::Plain => (
                "village/Wall_Plaster_Straight",
                vec![],
                solid,
                Matter::Plaster,
                27,
            ),
            Fill::Timber => (
                "village/Wall_Plaster_WoodGrid",
                vec![],
                solid,
                Matter::Timber,
                27,
            ),
            Fill::Round => (
                "village/Wall_Plaster_Window_Wide_Round",
                vec![
                    "village/Window_Wide_Round1",
                    "village/WindowShutters_Wide_Round_Open",
                ],
                solid,
                Matter::Plaster,
                22,
            ),
            Fill::Flat => (
                "village/Wall_Plaster_Window_Wide_Flat",
                vec!["village/Window_Wide_Flat1"],
                solid,
                Matter::Plaster,
                22,
            ),
            Fill::Door => (
                "village/Wall_Plaster_Door_Round",
                vec!["village/DoorFrame_Round_WoodDark"],
                vec![
                    between([lo, min[1], min[2]], [-DOOR_HALF, max[1], max[2]]),
                    between([DOOR_HALF, min[1], min[2]], [hi, max[1], max[2]]),
                    between([-DOOR_HALF, LINTEL, min[2]], [DOOR_HALF, max[1], max[2]]),
                ],
                Matter::Plaster,
                22,
            ),
        };
    let mut models = vec![(host, Mat4::IDENTITY)];
    models.extend(dressing.into_iter().map(|m| (m, Mat4::IDENTITY)));
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
    Draft {
        building,
        role,
        matter,
        placement,
        models,
        colliders,
        origin: Vec3::new(0.0, 1.56, -0.11),
        mass: 600.0,
        hit_points,
    }
}

/// The 8 by 10 round-tile roof: two slabs under its tiles, meeting at the
/// ridge.
fn roof(building: usize, placement: Mat4) -> Draft {
    let (min, max) = ROOF;
    let angle = slope_angle();
    let length = (max[0]).hypot(max[1] - min[1]);
    let slab = |sign: f32| {
        // From the eave at x = sign * 4.98 up to the ridge, the slab's top
        // face under the tiles.
        let eave = Vec3::new(sign * max[0], min[1], 0.0);
        let ridge = Vec3::new(0.0, max[1], 0.0);
        let rotation = Quat::from_rotation_z(-sign * angle);
        let up = rotation * Vec3::Y;
        let center = (eave + ridge) * 0.5 - up * (ROOF_SLAB * 0.5 + 0.02);
        Cuboid {
            center: center.as_dvec3(),
            rotation: rotation.as_dquat(),
            half: DVec3::new(
                f64::from(length) * 0.5,
                f64::from(ROOF_SLAB) * 0.5,
                f64::from(max[2].min(-min[2])),
            ),
        }
    };
    Draft {
        building,
        role: Role::Roof,
        matter: Matter::Tile,
        placement,
        models: vec![("village/Roof_RoundTiles_8x10", Mat4::IDENTITY)],
        colliders: vec![slab(-1.0), slab(1.0)],
        origin: Vec3::new(0.0, 2.0, 0.0),
        mass: 2000.0,
        hit_points: 27,
    }
}

/// Every piece of both cottages ready to raise, chunked by a plain grid.
#[must_use]
pub fn specs_without_meshes() -> Vec<PieceSpec> {
    drafts()
        .iter()
        .map(|draft| draft.spec(draft.grid_chunks()))
        .collect()
}
