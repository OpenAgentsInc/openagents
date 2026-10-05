//! The Medieval Village kit's structural pieces as the demolition rules see
//! them, shared by the yard's cottages ([`super::cottage`]) and Everglade's
//! town ([`super::town`]): a 2 m wall section and its dressing, a corner
//! post, an 8 m by 10 m round-tile roof span, a brick gable, and a chimney.
//! Each piece carries the models drawn with it, the colliders the solver
//! and the player see, its mass and hit points, and how it is cut into
//! chunks.

use super::site::{Cuboid, Matter, PieceSpec, Role};
use crate::zones::everglade::height;
use glam::{DVec3, Mat4, Quat, Vec3};

/// Wall tops and the roof's base, m (`layout::WALL_TOP`).
pub const WALL_TOP: f32 = 3.12;
/// Kit model bounds, model meters, from the pinned pack. Every wall
/// section shares one outline.
const WALL: ([f32; 3], [f32; 3]) = ([-1.0, 0.0, -0.31], [1.0, 3.12, 0.09]);
const POST: ([f32; 3], [f32; 3]) = ([-0.11, 0.0, -0.12], [0.11, 3.0, 0.12]);
const ROOF: ([f32; 3], [f32; 3]) = ([-4.98, -0.78, -5.85], [4.98, 6.0, 6.01]);
/// How far a wall section's collider stops short of a corner, m: the
/// crossing wall's thickness, so no two colliders overlap.
pub const CORNER_TRIM: f32 = 0.34;
/// How far a wall section's collider stops short of its neighbor, m.
pub const SEAM: f32 = 0.01;
/// Half the clear width of a doorway, and its lintel's underside, m.
const DOOR_HALF: f32 = 0.6;
const LINTEL: f32 = 2.5;
/// Roof slab thickness under the tiles, m.
const ROOF_SLAB: f32 = 0.2;

/// The kit's wall sections: model name, what it is made of, its hit
/// points, and whether it is a doorway.
const WALLS: [(&str, Matter, i32, bool); 6] = [
    ("village/Wall_Plaster_Straight", Matter::Plaster, 27, false),
    (
        "village/Wall_Plaster_Straight_Base",
        Matter::Brick,
        27,
        false,
    ),
    ("village/Wall_Plaster_WoodGrid", Matter::Timber, 27, false),
    (
        "village/Wall_Plaster_Window_Wide_Round",
        Matter::Plaster,
        22,
        false,
    ),
    (
        "village/Wall_Plaster_Window_Wide_Flat",
        Matter::Plaster,
        22,
        false,
    ),
    ("village/Wall_Plaster_Door_Round", Matter::Plaster, 22, true),
];

/// Whether `model` is one of the kit's wall sections.
#[must_use]
pub fn is_wall(model: &str) -> bool {
    WALLS.iter().any(|(name, ..)| *name == model)
}

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
            Role::Chimney { .. } => whole([1, 2, 1]),
            Role::Roof { .. } => {
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

    /// What decides the piece's chunks: its models, its cuts, and its
    /// origin. Two pieces with the same shape cut into the same chunks.
    #[must_use]
    pub fn shape(&self) -> String {
        let mut key = format!("{:?}", self.cuts());
        for (name, transform) in &self.models {
            key.push_str(name);
            key.push_str(&format!("{:?}", transform.to_cols_array()));
        }
        key.push_str(&format!("{:?}", self.origin));
        key
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
            Role::Roof { .. } => DVec3::new(10.0, 6.8, 11.9),
            Role::Gable { .. } => DVec3::new(6.0, 4.0, 0.5),
            Role::Chimney { .. } => DVec3::new(1.0, 3.2, 1.0),
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
#[must_use]
pub fn place(at: [f32; 2], lift: f32, yaw: f32) -> Mat4 {
    let [x, z] = at;
    Mat4::from_rotation_translation(
        Quat::from_rotation_y(yaw),
        Vec3::new(x, height(x, z) + lift, z),
    )
}

fn between(min: [f32; 3], max: [f32; 3]) -> Cuboid {
    Cuboid::between(Vec3::from(min).as_dvec3(), Vec3::from(max).as_dvec3())
}

/// A wall section of the kit's `host` model and its `dressing` (glass,
/// shutters, a door frame and leaf, each in the host's model space), its
/// collider stopping `trim` short of its low (-x) and high (+x) ends. A
/// host the kit doesn't know is a plain plaster section.
#[must_use]
pub fn wall(
    building: usize,
    role: Role,
    host: &'static str,
    dressing: Vec<(&'static str, Mat4)>,
    placement: Mat4,
    trim: [f32; 2],
) -> Draft {
    let (min, max) = WALL;
    let lo = min[0] + trim[0];
    let hi = max[0] - trim[1];
    let (matter, hit_points, door) = WALLS
        .iter()
        .find(|(name, ..)| *name == host)
        .map_or((Matter::Plaster, 27, false), |&(_, m, hp, door)| {
            (m, hp, door)
        });
    let colliders = if door {
        vec![
            between([lo, min[1], min[2]], [-DOOR_HALF, max[1], max[2]]),
            between([DOOR_HALF, min[1], min[2]], [hi, max[1], max[2]]),
            between([-DOOR_HALF, LINTEL, min[2]], [DOOR_HALF, max[1], max[2]]),
        ]
    } else {
        vec![between([lo, min[1], min[2]], [hi, max[1], max[2]])]
    };
    let mut models = vec![(host, Mat4::IDENTITY)];
    models.extend(dressing);
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

/// A corner post.
#[must_use]
pub fn post(building: usize, role: Role, placement: Mat4) -> Draft {
    let (min, max) = POST;
    Draft {
        building,
        role,
        matter: Matter::Timber,
        placement,
        models: vec![("village/Corner_Exterior_Wood", Mat4::IDENTITY)],
        colliders: vec![between(min, max)],
        origin: Vec3::new(0.0, 1.5, 0.0),
        mass: 60.0,
        hit_points: 18,
    }
}

/// A brick gable over an end wall.
#[must_use]
pub fn gable(building: usize, role: Role, placement: Mat4) -> Draft {
    Draft {
        building,
        role,
        matter: Matter::Brick,
        placement,
        models: vec![("village/Roof_Front_Brick8", Mat4::IDENTITY)],
        // Two steps under the roof's slopes.
        colliders: vec![
            between([-2.4, 0.05, -0.25], [2.4, 2.2, 0.35]),
            between([-1.0, 2.2, -0.25], [1.0, 4.0, 0.35]),
        ],
        origin: Vec3::new(0.0, 1.6, 0.05),
        mass: 500.0,
        hit_points: 27,
    }
}

/// A chimney through a roof.
#[must_use]
pub fn chimney(building: usize, role: Role, placement: Mat4) -> Draft {
    Draft {
        building,
        role,
        matter: Matter::Brick,
        placement,
        models: vec![("village/Prop_Chimney", Mat4::IDENTITY)],
        // Only the stack above the tiles collides, so it starts clear of
        // the roof's slab.
        colliders: vec![between([-0.45, 2.1, -0.45], [0.45, 3.18, 0.45])],
        origin: Vec3::new(0.0, 2.6, 0.0),
        mass: 400.0,
        hit_points: 27,
    }
}

/// The 8 by 10 round-tile roof: two slabs under its tiles, meeting at the
/// ridge.
#[must_use]
pub fn roof(building: usize, role: Role, placement: Mat4) -> Draft {
    roof_of("village/Roof_RoundTiles_8x10", building, role, placement)
}

/// [`roof`] drawn with `model`, a copy of the kit's roof such as the town's
/// thinned one (`layout::HOUSE_ROOF`).
#[must_use]
pub fn roof_of(model: &'static str, building: usize, role: Role, placement: Mat4) -> Draft {
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
        role,
        matter: Matter::Tile,
        placement,
        models: vec![(model, Mat4::IDENTITY)],
        colliders: vec![slab(-1.0), slab(1.0)],
        origin: Vec3::new(0.0, 2.0, 0.0),
        mass: 2000.0,
        hit_points: 27,
    }
}
