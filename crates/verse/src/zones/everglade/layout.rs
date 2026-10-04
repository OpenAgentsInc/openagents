//! The glade and workshop as placement data (`docs/verse/everglade.md`,
//! Layout).
//!
//! Every placement names a model in the pinned Everglade pack and how it
//! blocks walking. The workshop hall is built from the Medieval Village kit's
//! modular pieces on its 2 m grid: walls 2 m wide and 3.12 m tall whose
//! exterior face is the piece's +z side, wide windows, two doorways that make
//! the double doors, a wood floor, and twin round-tile roofs with brick
//! gables. The yard, the strongroom, the wagon, the lounge, the tree ring,
//! and the undergrowth surround it. Scattered ground cover and the tree ring
//! are placed by a fixed hash, so the layout is the same on every device.
//!
//! The station standing points in [`super::STATIONS`] are fixed; furniture
//! stands ahead of them, and validation tests check that every station
//! stays reachable and that no blocker covers a path.

use super::{HALL, PATH_HALF_WIDTH, RETURN_PORTAL, STATIONS, STRONGROOM, YARD, height};
use crate::controller::Footprint;
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// How a placement blocks walking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collision {
    /// Ground cover, paths, roofs, and pieces that another blocker covers.
    None,
    /// The model's bounds on the ground after its transform.
    Bounds,
    /// A square this many model meters either side of the origin: a trunk,
    /// the core of a bush, or a rock's base.
    Core(f32),
    /// The model's bounds less a central opening this many model meters
    /// either side of the origin along the model's x axis: a doorway or an
    /// arch, which leaves its two jambs.
    Opening(f32),
}

/// One model placed in the glade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The pack's `set/source-name`, such as `village/Wall_Plaster_Straight`.
    pub model: &'static str,
    /// Position on the ground, x and z, m.
    pub at: [f32; 2],
    /// Height above the ground at `at`, m.
    pub lift: f32,
    /// Rotation about +Y, as the controller's yaw: the model's +z faces
    /// `controller::forward(yaw)`.
    pub yaw: f32,
    /// Uniform scale.
    pub scale: f32,
    pub collision: Collision,
}

impl Placement {
    const fn new(model: &'static str, at: [f32; 2], yaw: f32, collision: Collision) -> Self {
        Self {
            model,
            at,
            lift: 0.0,
            yaw,
            scale: 1.0,
            collision,
        }
    }

    const fn lift(mut self, lift: f32) -> Self {
        self.lift = lift;
        self
    }

    const fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Model space to the glade.
    #[must_use]
    pub fn transform(&self) -> Mat4 {
        let [x, z] = self.at;
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_y(self.yaw),
            Vec3::new(x, height(x, z) + self.lift, z),
        )
    }

    /// The navigation blockers for a model with model-space `bounds`.
    #[must_use]
    pub fn footprints(&self, bounds: ([f32; 3], [f32; 3])) -> Vec<Footprint> {
        let (min, max) = bounds;
        let boxes = match self.collision {
            Collision::None => vec![],
            Collision::Bounds => vec![([min[0], min[2]], [max[0], max[2]])],
            Collision::Core(half) => vec![([-half, -half], [half, half])],
            Collision::Opening(half) => vec![
                ([min[0], min[2]], [-half, max[2]]),
                ([half, min[2]], [max[0], max[2]]),
            ],
        };
        let transform = self.transform();
        boxes
            .into_iter()
            .filter(|(lo, hi)| lo[0] < hi[0] && lo[1] <= hi[1])
            .map(|(lo, hi)| {
                let corners = [
                    [lo[0], lo[1]],
                    [hi[0], lo[1]],
                    [hi[0], hi[1]],
                    [lo[0], hi[1]],
                ]
                .map(|[x, z]| transform.transform_point3(Vec3::new(x, 0.0, z)));
                let (mut low, mut high) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
                for c in corners {
                    low = [low[0].min(c.x), low[1].min(c.z)];
                    high = [high[0].max(c.x), high[1].max(c.z)];
                }
                Footprint {
                    min: low,
                    max: high,
                }
            })
            .collect()
    }
}

/// An in-world board Verse draws itself, as the Gym draws its boards: a
/// dark slate in a wooden frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Board {
    /// Center of the board's face, m.
    pub center: Vec3,
    /// Heading of the face's outward normal, as the controller's yaw.
    pub facing: f32,
    /// Width and height, m.
    pub size: [f32; 2],
}

impl Board {
    /// Board space to the glade. Board space has its face in the XY plane
    /// facing -Z, so lettering from `doors::scene_label` reads from the
    /// front; the viewer's right is board -X.
    #[must_use]
    pub fn transform(&self) -> Mat4 {
        Mat4::from_translation(self.center) * Mat4::from_rotation_y(self.facing + PI)
    }
}

/// One seat's desk in the hall: where the seat stands, facing the
/// workbench, and the monitor board on the bench.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desk {
    /// Standing point, x and z, m, facing +z (yaw zero).
    pub seat: [f32; 2],
    /// The monitor board that streams the seat's log tail.
    pub monitor: Board,
}

/// Where the row of workbenches stands, z, m.
const DESK_Z: f32 = 7.0;

const fn desk(x: f32) -> Desk {
    Desk {
        seat: [x, 5.8],
        monitor: Board {
            center: Vec3::new(x, 1.3, 7.3),
            facing: PI,
            size: [0.84, 0.5],
        },
    }
}

/// One workbench per seat, in a row across the hall behind the desks
/// station.
pub const DESKS: [Desk; 4] = [desk(-3.3), desk(-1.1), desk(1.1), desk(3.3)];

/// The Task Wall: the yard's notice board, facing the task wall station.
pub const TASK_WALL: Board = Board {
    center: Vec3::new(-9.1, 1.55, -9.0),
    facing: FRAC_PI_2,
    size: [3.2, 1.7],
};

/// The Task Wall's columns, from the viewer's left.
pub const TASK_COLUMNS: [&str; 5] = ["PLANNED", "RUNNING", "REVIEW", "DONE", "BLOCKED"];

/// The goal board: the atrium inside the yard's gate, east of the path,
/// facing the approach, so a player coming from the portal reads the goal,
/// its progress ring, and its task counts first.
pub const GOAL_BOARD: Board = Board {
    center: Vec3::new(3.9, 1.75, -11.5),
    facing: PI,
    size: [2.4, 1.4],
};

/// The boards' own blockers: each slate, frame, and legs.
#[must_use]
pub fn board_blockers() -> Vec<Footprint> {
    let half = TASK_WALL.size[0] / 2.0 + 0.1;
    let goal = GOAL_BOARD.size[0] / 2.0 + 0.1;
    vec![
        Footprint {
            min: [TASK_WALL.center.x - 0.2, TASK_WALL.center.z - half],
            max: [TASK_WALL.center.x + 0.2, TASK_WALL.center.z + half],
        },
        Footprint {
            min: [GOAL_BOARD.center.x - goal, GOAL_BOARD.center.z - 0.2],
            max: [GOAL_BOARD.center.x + goal, GOAL_BOARD.center.z + 0.2],
        },
    ]
}

/// The walked routes, as segments a player-wide body must pass along
/// without touching a blocker: the approach from the return portal, the
/// yard, and each doorway into the hall.
pub const PATHS: [[[f32; 2]; 2]; 4] = [
    [[0.0, -31.0], [0.0, -14.5]],
    [[0.0, -14.5], [0.0, -1.4]],
    [[-1.0, -1.4], [-1.0, 3.0]],
    [[1.0, -1.4], [1.0, 3.0]],
];

/// Where the wall pieces stand: their top, and the roofs' base, m.
pub const WALL_TOP: f32 = 3.12;
/// Half the clear width of a doorway or the gallery arch, m.
const DOOR_HALF: f32 = 0.6;
const ARCH_HALF: f32 = 0.8;

/// Outward headings of the hall's four walls.
const SOUTH: f32 = PI;
const NORTH: f32 = 0.0;
const WEST: f32 = -FRAC_PI_2;
const EAST: f32 = FRAC_PI_2;

/// A wall piece and what fills it.
#[derive(Clone, Copy)]
enum Piece {
    Plain,
    Base,
    Timber,
    /// A wide round window with glass and open shutters.
    Round,
    /// A wide flat window with glass.
    Flat,
    Door,
}

/// The tree ring's models, cheapest first; each tree takes the next.
const TREES: [&str; 5] = [
    "nature/CommonTree_5",
    "nature/Pine_2",
    "nature/CommonTree_3",
    "nature/Pine_1",
    "nature/CommonTree_4",
];
/// Trees in the ring.
const RING_TREES: u32 = 22;
/// Grass and clover clumps scattered in the clearing.
const GRASS: [&str; 4] = [
    "nature/Grass_Common_Short",
    "nature/Grass_Wispy_Short",
    "nature/Clover_1",
    "nature/Grass_Common_Tall",
];
const GRASS_CLUMPS: usize = 32;
/// Small plants at the clearing's edge.
const PLANTS: [(&str, f32); 4] = [
    ("nature/Plant_1", 1.0),
    ("nature/Plant_7", 1.2),
    ("nature/Plant_7_Big", 1.0),
    ("nature/Plant_1_Big", 0.5),
];
const EDGE_PLANTS: usize = 12;

/// A deterministic value in `0..1` for `n` in stream `salt`.
fn noise(n: u32, salt: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn inside(rect: ([f32; 2], [f32; 2]), x: f32, z: f32, margin: f32) -> bool {
    let (center, half) = rect;
    (x - center[0]).abs() <= half[0] + margin && (z - center[1]).abs() <= half[1] + margin
}

/// Where the lounge bench and the wagon stand.
const BENCH: [f32; 2] = [-25.6, -20.0];
const WAGON: [f32; 2] = [11.0, -23.0];

/// Whether loose ground cover may grow at `(x, z)`: off the floors, the
/// yard, the paths, the station points, the lounge, and the wagon.
fn open_ground(x: f32, z: f32) -> bool {
    let path = x.abs() <= PATH_HALF_WIDTH + 0.8 && z <= YARD.0[1] - YARD.1[1] + 1.0;
    let near = |p: [f32; 2], r: f32| (p[0] - x).hypot(p[1] - z) < r;
    !inside(HALL, x, z, 1.5)
        && !inside(STRONGROOM, x, z, 1.5)
        && !inside(YARD, x, z, 1.0)
        && !path
        && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], 3.5)
        && !near(BENCH, 4.0)
        && !near([WAGON[0], WAGON[1] - 1.0], 4.5)
        && !STATIONS.iter().any(|s| near(s.at, 2.2))
}

/// Every placement in the glade.
#[must_use]
pub fn placements() -> Vec<Placement> {
    let mut out = Vec::with_capacity(512);
    hall(&mut out);
    stations(&mut out);
    strongroom(&mut out);
    yard(&mut out);
    paths(&mut out);
    glade(&mut out);
    out
}

/// One wall piece of the hall at `center` on the wall line, facing
/// `outward`, with its window glass, shutters, or door frame.
fn wall(out: &mut Vec<Placement>, piece: Piece, center: [f32; 2], outward: f32) {
    let put = |model, collision| Placement::new(model, center, outward, collision);
    match piece {
        Piece::Plain => out.push(put("village/Wall_Plaster_Straight", Collision::Bounds)),
        Piece::Base => out.push(put("village/Wall_Plaster_Straight_Base", Collision::Bounds)),
        Piece::Timber => out.push(put("village/Wall_Plaster_WoodGrid", Collision::Bounds)),
        Piece::Round => {
            out.push(put(
                "village/Wall_Plaster_Window_Wide_Round",
                Collision::Bounds,
            ));
            out.push(put("village/Window_Wide_Round1", Collision::None));
            out.push(put(
                "village/WindowShutters_Wide_Round_Open",
                Collision::None,
            ));
        }
        Piece::Flat => {
            out.push(put(
                "village/Wall_Plaster_Window_Wide_Flat",
                Collision::Bounds,
            ));
            out.push(put("village/Window_Wide_Flat1", Collision::None));
        }
        Piece::Door => {
            out.push(put(
                "village/Wall_Plaster_Door_Round",
                Collision::Opening(DOOR_HALF),
            ));
            out.push(put("village/DoorFrame_Round_WoodDark", Collision::None));
        }
    }
}

/// The workshop hall: walls, floor, double doors, roofs, and dressing.
fn hall(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Round, Timber};
    let ([cx, cz], [hx, hz]) = HALL;
    let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
    // Pieces from west to east, and from south to north.
    let south_wall = [Round, Timber, Round, Door, Door, Round, Timber, Round];
    let north_wall = [Plain, Flat, Timber, Flat, Flat, Timber, Flat, Plain];
    let west_wall = [Base, Round, Timber, Plain, Plain];
    let east_wall = [Plain, Flat, Timber, Base, Base];
    for (i, piece) in south_wall.into_iter().enumerate() {
        wall(out, piece, [west + 1.0 + 2.0 * i as f32, south], SOUTH);
    }
    for (i, piece) in north_wall.into_iter().enumerate() {
        wall(out, piece, [west + 1.0 + 2.0 * i as f32, north], NORTH);
    }
    for (i, piece) in west_wall.into_iter().enumerate() {
        wall(out, piece, [west, south + 1.0 + 2.0 * i as f32], WEST);
    }
    for (i, piece) in east_wall.into_iter().enumerate() {
        wall(out, piece, [east, south + 1.0 + 2.0 * i as f32], EAST);
    }
    for corner in [[west, south], [east, south], [west, north], [east, north]] {
        out.push(Placement::new(
            "village/Corner_Exterior_Wood",
            corner,
            0.0,
            Collision::None,
        ));
    }
    // The double doors: one leaf in each doorway, swung open into the hall
    // against its outer jamb, which the doorway's blocker already covers.
    for (x, hinge) in [(-1.0_f32, -0.5_f32), (1.0, 0.5)] {
        out.push(
            Placement::new(
                "village/Door_4_Round",
                [x + hinge, south + 0.35],
                -FRAC_PI_2,
                Collision::None,
            )
            .lift(0.02),
        );
    }
    // Wood floor on the 2 m grid, with brick under the hearth.
    for i in 0..8 {
        for j in 0..5 {
            let at = [west + 1.0 + 2.0 * i as f32, south + 1.0 + 2.0 * j as f32];
            let hearth = i == 7 && j >= 3;
            let model = if hearth {
                "village/Floor_Brick"
            } else {
                "village/Floor_WoodDark"
            };
            out.push(Placement::new(model, at, 0.0, Collision::None).lift(0.02));
        }
    }
    // Twin round-tile roofs with brick gables at both ends.
    for x in [cx - hx / 2.0, cx + hx / 2.0] {
        out.push(
            Placement::new(
                "village/Roof_RoundTiles_8x10",
                [x, cz],
                0.0,
                Collision::None,
            )
            .lift(WALL_TOP),
        );
        out.push(
            Placement::new(
                "village/Roof_Front_Brick8",
                [x, south],
                SOUTH,
                Collision::None,
            )
            .lift(WALL_TOP),
        );
        out.push(
            Placement::new(
                "village/Roof_Front_Brick8",
                [x, north],
                NORTH,
                Collision::None,
            )
            .lift(WALL_TOP),
        );
    }
    // The hearth's chimney rises through the east roof above the cauldron.
    out.push(Placement::new("village/Prop_Chimney", [6.6, 9.0], 0.0, Collision::None).lift(4.9));
    // A stone border along the north and west walls, and vines.
    for i in 0..8 {
        out.push(Placement::new(
            "village/Prop_ExteriorBorder_Straight1",
            [west + 1.0 + 2.0 * i as f32, north + 0.1],
            NORTH,
            Collision::None,
        ));
    }
    for j in 0..5 {
        out.push(Placement::new(
            "village/Prop_ExteriorBorder_Straight1",
            [west - 0.1, south + 1.0 + 2.0 * j as f32],
            WEST,
            Collision::None,
        ));
    }
    out.push(Placement::new(
        "village/Prop_ExteriorBorder_Corner",
        [west - 0.1, north + 0.1],
        NORTH,
        Collision::None,
    ));
    out.push(
        Placement::new(
            "village/Prop_Vine1",
            [west - 0.1, 6.0],
            WEST,
            Collision::None,
        )
        .lift(2.8),
    );
    out.push(
        Placement::new(
            "village/Prop_Vine1",
            [5.0, south - 0.1],
            SOUTH,
            Collision::None,
        )
        .lift(2.8),
    );
    // Lanterns: on the pier between the doors, and inside on the north wall.
    out.push(
        Placement::new(
            "props/Lantern_Wall",
            [0.0, south - 0.1],
            SOUTH,
            Collision::Bounds,
        )
        .lift(1.5),
    );
    out.push(
        Placement::new(
            "props/Lantern_Wall",
            [-3.0, north - 0.25],
            SOUTH,
            Collision::Bounds,
        )
        .lift(1.5),
    );
}

/// Furniture ahead of each station's standing point.
fn stations(out: &mut Vec<Placement>) {
    // Desks: one workbench per seat; the monitor boards are drawn by Verse.
    for desk in DESKS {
        out.push(Placement::new(
            "props/Workbench",
            [desk.seat[0], DESK_Z],
            PI,
            Collision::Bounds,
        ));
        // The stool the desk's seat sits on to type. Walking passes it, so
        // the seat reaches its place at the bench.
        let [x, z] = super::studio::at_desk(&desk);
        out.push(Placement::new(
            "props/Stool",
            [x, z + super::pose::SEAT_FORWARD],
            0.0,
            Collision::None,
        ));
    }
    // Library: the gallery's bookcases behind a timber arch, and a reading
    // table with books and a scroll.
    for z in [7.9, 9.7] {
        out.push(Placement::new(
            "props/Bookcase_2",
            [-7.4, z],
            FRAC_PI_2,
            Collision::Bounds,
        ));
    }
    out.push(Placement::new(
        "village/Wall_Arch",
        [-6.6, 8.8],
        FRAC_PI_2,
        Collision::Opening(ARCH_HALF),
    ));
    out.push(Placement::new(
        "props/BookStand",
        [-6.9, 6.3],
        FRAC_PI_2,
        Collision::Bounds,
    ));
    out.push(Placement::new(
        "props/Table_Large",
        [-5.3, 3.4],
        0.0,
        Collision::Bounds,
    ));
    out.push(Placement::new("props/Book_Stack_1", [-5.9, 3.3], 0.3, Collision::None).lift(0.81));
    out.push(Placement::new("props/Scroll_1", [-4.7, 3.5], 1.2, Collision::None).lift(0.81));
    // Oracle: the cauldron on the hearth's brick, with candles.
    out.push(Placement::new(
        "props/Cauldron",
        [6.9, 9.0],
        0.0,
        Collision::Bounds,
    ));
    out.push(Placement::new(
        "props/CandleStick_Triple",
        [7.35, 10.3],
        FRAC_PI_2,
        Collision::Bounds,
    ));
    // Podium: a book stand outside the door, under a banner on the wall.
    out.push(Placement::new(
        "props/BookStand",
        [-3.0, -1.25],
        PI,
        Collision::Bounds,
    ));
    out.push(Placement::new("props/Banner_2", [-4.2, 0.78], SOUTH, Collision::Bounds).lift(1.35));
    // Task Wall: a rail behind the board and banners at its ends; the
    // board itself is drawn by Verse and blocks by `board_blockers`.
    for z in [-9.95, -8.05] {
        out.push(Placement::new(
            "village/Prop_WoodenFence_Single",
            [-9.55, z],
            FRAC_PI_2,
            Collision::Bounds,
        ));
    }
    for z in [-10.65, -5.7] {
        out.push(
            Placement::new("props/Banner_2", [-9.2, z], FRAC_PI_2, Collision::Bounds).lift(1.3),
        );
    }
    // Proving ground: a dummy in a ring of stones, with an anvil and a rock.
    out.push(Placement::new(
        "props/Dummy",
        [11.0, -8.0],
        -FRAC_PI_2,
        Collision::Bounds,
    ));
    out.push(Placement::new(
        "props/Anvil",
        [12.5, -6.2],
        0.4,
        Collision::Bounds,
    ));
    out.push(
        Placement::new("nature/Rock_Medium_2", [13.9, -8.0], 2.0, Collision::Bounds).scale(0.4),
    );
    let pebbles = [
        "nature/Pebble_Round_1",
        "nature/Pebble_Round_2",
        "nature/Pebble_Round_3",
    ];
    for k in 0..12 {
        // The ring opens toward the station on its west side.
        let angle = (15.0 + 30.0 * k as f32).to_radians();
        let at = [11.0 + 2.3 * angle.cos(), -8.0 + 2.3 * angle.sin()];
        out.push(
            Placement::new(
                pebbles[k % 3],
                at,
                noise(k as u32, 7) * TAU,
                Collision::None,
            )
            .scale(2.4),
        );
    }
    // Lounge: a bench and stools under two trees, with mushrooms.
    out.push(Placement::new(
        "props/Bench",
        BENCH,
        FRAC_PI_2,
        Collision::Bounds,
    ));
    for at in [[-25.1, -22.5], [-25.2, -17.6]] {
        out.push(Placement::new("props/Stool", at, 0.0, Collision::Bounds));
    }
    out.push(
        Placement::new(
            "nature/CommonTree_3",
            [-28.6, -21.8],
            0.7,
            Collision::Core(0.4),
        )
        .scale(1.1),
    );
    out.push(Placement::new(
        "nature/Pine_2",
        [-27.6, -16.4],
        2.1,
        Collision::Core(0.4),
    ));
    for (i, at) in [[-27.4, -21.0], [-26.6, -16.2], [-28.6, -19.0]]
        .into_iter()
        .enumerate()
    {
        out.push(
            Placement::new(
                "nature/Mushroom_Common",
                at,
                i as f32 * 2.1,
                Collision::None,
            )
            .scale(1.4),
        );
    }
    // Workbench: the wagon by the gate, with crates.
    out.push(Placement::new(
        "village/Prop_Wagon",
        WAGON,
        0.0,
        Collision::Bounds,
    ));
    for (at, yaw) in [([13.0, -21.5], 0.2), ([13.1, -22.7], -0.15)] {
        out.push(Placement::new(
            "village/Prop_Crate",
            at,
            yaw,
            Collision::Bounds,
        ));
    }
    out.push(Placement::new("village/Prop_Crate", [13.0, -21.5], 0.9, Collision::None).lift(1.06));
}

/// The strongroom: a brick-floored enclosure of metal fence east of the
/// hall, open to the yard on its south side, with the merge station's
/// metal crates.
fn strongroom(out: &mut Vec<Placement>) {
    let ([cx, cz], [hx, hz]) = STRONGROOM;
    let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
    for i in 0..3 {
        for j in 0..4 {
            let at = [west + 1.0 + 2.0 * i as f32, south + 1.0 + 2.0 * j as f32];
            out.push(Placement::new("village/Floor_Brick", at, 0.0, Collision::None).lift(0.02));
        }
    }
    let simple = "village/Prop_MetalFence_Simple";
    let ornament = "village/Prop_MetalFence_Ornament";
    for (j, model) in [simple, ornament, simple, simple].into_iter().enumerate() {
        out.push(Placement::new(
            model,
            [east, south + 1.0 + 2.0 * j as f32],
            EAST,
            Collision::Bounds,
        ));
    }
    for i in 0..3 {
        out.push(Placement::new(
            simple,
            [west + 1.0 + 2.0 * i as f32, north],
            NORTH,
            Collision::Bounds,
        ));
    }
    // The gap between the two south pieces is the entrance.
    for x in [west + 1.0, east - 1.0] {
        out.push(Placement::new(simple, [x, south], SOUTH, Collision::Bounds));
    }
    out.push(Placement::new(
        "props/Crate_Metal",
        [12.3, 5.0],
        FRAC_PI_2,
        Collision::Bounds,
    ));
    out.push(Placement::new(
        "props/Crate_Metal",
        [13.2, 8.8],
        0.3,
        Collision::Bounds,
    ));
}

/// The yard's south fence, with the gate where the approach path enters.
fn yard(out: &mut Vec<Placement>) {
    let south = YARD.0[1] - YARD.1[1];
    for (i, x) in [4.0_f32, 6.0, 8.0, 10.0, 12.0].into_iter().enumerate() {
        let model = if i % 2 == 1 {
            "village/Prop_WoodenFence_Extension1"
        } else {
            "village/Prop_WoodenFence_Single"
        };
        for side in [-1.0, 1.0] {
            out.push(Placement::new(
                model,
                [side * x, south],
                0.0,
                Collision::Bounds,
            ));
        }
    }
}

/// Stepping stones from the return portal through the gate to the doors.
fn paths(out: &mut Vec<Placement>) {
    let small = [
        "nature/RockPath_Round_Small_1",
        "nature/RockPath_Round_Small_2",
        "nature/RockPath_Round_Small_3",
    ];
    for k in 0..9_u32 {
        let z = -31.0 + 2.0 * k as f32;
        let model = match k {
            0 => "nature/RockPath_Round_Thin",
            8 => "nature/RockPath_Round_Wide",
            _ => small[k as usize % 3],
        };
        let x = (noise(k, 11) - 0.5) * 0.5;
        out.push(Placement::new(
            model,
            [x, z],
            noise(k, 12) * TAU,
            Collision::None,
        ));
    }
    for k in 0..6_u32 {
        let z = -12.6 + 2.15 * k as f32;
        let x = (noise(k, 13) - 0.5) * 0.4;
        out.push(Placement::new(
            small[k as usize % 3],
            [x, z],
            noise(k, 14) * TAU,
            Collision::None,
        ));
    }
}

/// The tree ring, the undergrowth at the clearing's edge, ground cover,
/// and flowers along the approach.
fn glade(out: &mut Vec<Placement>) {
    for k in 0..RING_TREES {
        let angle = (k as f32 + 0.4 * noise(k, 1)) / RING_TREES as f32 * TAU;
        let r = 41.0 + (k % 3) as f32 * 5.0 + 2.0 * noise(k, 2);
        out.push(
            Placement::new(
                TREES[k as usize % TREES.len()],
                [angle.cos() * r, angle.sin() * r],
                noise(k, 3) * TAU,
                Collision::Core(0.4),
            )
            .scale(1.0 + 0.3 * noise(k, 4))
            // Sunk a little so roots meet the slope.
            .lift(-0.15),
        );
    }
    // Bushes around the clearing's edge, clear of the approach (270°), the
    // lounge (220°), and the wagon (290°).
    for (i, degrees) in [15.0_f32, 50.0, 95.0, 130.0, 165.0, 195.0, 245.0, 330.0]
        .into_iter()
        .enumerate()
    {
        let angle = degrees.to_radians();
        let model = if i % 2 == 0 {
            "nature/Bush_Common"
        } else {
            "nature/Bush_Common_Flowers"
        };
        out.push(
            Placement::new(
                model,
                [angle.cos() * 33.5, angle.sin() * 33.5],
                noise(i as u32, 5) * TAU,
                Collision::Core(0.55),
            )
            .scale(1.1),
        );
    }
    for (i, (model, at)) in [
        ("nature/Rock_Medium_1", [-18.0, 26.5]),
        ("nature/Rock_Medium_3", [24.0, 19.5]),
        ("nature/Rock_Medium_2", [30.5, -4.0]),
        ("nature/Rock_Medium_1", [-31.0, 6.0]),
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Placement::new(model, at, noise(i as u32, 6) * TAU, Collision::Bounds).scale(0.6));
    }
    // Flowers and ferns along the approach path.
    for (i, at) in [
        [-2.9, -29.5],
        [2.8, -27.0],
        [-2.7, -22.5],
        [3.0, -20.0],
        [-3.2, -17.0],
        [2.9, -16.2],
    ]
    .into_iter()
    .enumerate()
    {
        let model = if i % 2 == 0 {
            "nature/Flower_3_Group"
        } else {
            "nature/Flower_4_Group"
        };
        out.push(Placement::new(
            model,
            at,
            noise(i as u32, 8) * TAU,
            Collision::None,
        ));
    }
    for (i, at) in [
        [3.6, -30.0],
        [-3.8, -24.5],
        [3.8, -23.0],
        [-4.0, -19.0],
        [-27.0, -23.6],
        [-28.2, -17.8],
        [13.0, -26.5],
        [-15.0, 6.0],
    ]
    .into_iter()
    .enumerate()
    {
        out.push(
            Placement::new(
                "nature/Fern_1",
                at,
                noise(i as u32, 9) * TAU,
                Collision::None,
            )
            .scale(0.3),
        );
    }
    scatter(out, &GRASS.map(|m| (m, 1.0)), GRASS_CLUMPS, 12.0, 33.0, 20);
    scatter(out, &PLANTS, EDGE_PLANTS, 26.0, 34.0, 21);
}

/// Places `count` pieces of ground cover, cycling through `models`, at
/// hashed points of open ground between radii `near` and `far`.
fn scatter(
    out: &mut Vec<Placement>,
    models: &[(&'static str, f32)],
    count: usize,
    near: f32,
    far: f32,
    salt: u32,
) {
    let mut placed = 0;
    for n in 0..count as u32 * 16 {
        if placed == count {
            break;
        }
        let angle = noise(n, salt) * TAU;
        let r = near + (far - near) * noise(n, salt + 100).sqrt();
        let (x, z) = (angle.cos() * r, angle.sin() * r);
        if !open_ground(x, z) {
            continue;
        }
        let (model, scale) = models[placed % models.len()];
        out.push(
            Placement::new(model, [x, z], noise(n, salt + 200) * TAU, Collision::None)
                .scale(scale * (0.8 + 0.4 * noise(n, salt + 300))),
        );
        placed += 1;
    }
}
