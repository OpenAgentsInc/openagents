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

use super::scene::Paint;
use super::{HALL, PATH_HALF_WIDTH, RETURN_PORTAL, STATIONS, STRONGROOM, YARD, height, land};
use crate::controller::Footprint;
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use verse_world::social::everglade::DESK_SEATS;

pub mod agora;
pub mod belvedere;
pub mod city;
pub mod civic;
pub mod details;
pub mod districts;
pub mod estate;
pub mod foliage;
pub mod furnish;
pub mod generated;
pub mod greens;
pub mod kit_house;
pub mod parks;
pub mod pylon_field;
pub mod streets;
pub mod trails;

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
    pub const fn new(model: &'static str, at: [f32; 2], yaw: f32, collision: Collision) -> Self {
        Self {
            model,
            at,
            lift: 0.0,
            yaw,
            scale: 1.0,
            collision,
        }
    }

    pub const fn lift(mut self, lift: f32) -> Self {
        self.lift = lift;
        self
    }

    pub const fn scale(mut self, scale: f32) -> Self {
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

/// One seat's standing desk in the hall: where the seat stands, facing
/// the desk, and the monitor board on it, at a standing seat's eye level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desk {
    /// Standing point, x and z, m, facing +z (yaw zero).
    pub seat: [f32; 2],
    /// The monitor board that streams the seat's log tail.
    pub monitor: Board,
}

/// Where the row of standing desks stands, z, m.
const DESK_Z: f32 = 7.0;

const fn desk(seat: [f32; 2]) -> Desk {
    Desk {
        seat,
        monitor: Board {
            center: Vec3::new(seat[0], 1.4, 7.3),
            facing: PI,
            size: [0.84, 0.5],
        },
    }
}

/// One workbench per seat, in a row across the hall behind the desks
/// station, at the shared standing points a hosted instance walks seats to.
pub const DESKS: [Desk; 4] = [
    desk(DESK_SEATS[0]),
    desk(DESK_SEATS[1]),
    desk(DESK_SEATS[2]),
    desk(DESK_SEATS[3]),
];

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

/// The ponds: center and water radius, m. Lantern Pond lies on the
/// commons; Reed Pond in the long meadow by the Knowledge District. The
/// city's are the Thinking Pond in Walden Woods and the Fern Pond in
/// Fernhollow; the Fountain Plaza's fountain is a generated model
/// (`city::PLAZA_FOUNTAIN`). Every pond is swimmable: its bowl is carved
/// into the heightfield ([`verse_world::social::everglade_water`]), and
/// Glade Run's bed with it.
pub use verse_world::social::everglade_water::{
    POND_DEPTHS, POND_NAMES, PONDS, STREAM, STREAM_HALF, stream_distance,
};
/// The footbridge that carries Brownstone Row over the stream: its center
/// and heading, as the controller's yaw; the deck runs along the model's z.
pub const BRIDGE: ([f32; 2], f32) = ([9.0, -78.0], FRAC_PI_2);
/// The footbridge deck's half length and half width, m, and its rise.
const BRIDGE_HALF: [f32; 2] = [3.5, 1.0];
const BRIDGE_RISE: f32 = 0.55;

/// The footbridge's deck as steps to walk over: one footprint per plank
/// with its top, m, each within a step of the last.
#[must_use]
pub fn bridge_steps() -> Vec<(Footprint, f32)> {
    let ([bx, bz], yaw) = BRIDGE;
    let along = crate::controller::forward(yaw);
    let planks = 14;
    (0..planks)
        .map(|i| {
            let t = (i as f32 + 0.5) / planks as f32;
            let d = -BRIDGE_HALF[0] + 2.0 * BRIDGE_HALF[0] * t;
            let half = BRIDGE_HALF[0] / planks as f32;
            let (cx, cz) = (bx + along.x * d, bz + along.z * d);
            let (hx, hz) = (
                (along.x * half).abs() + (along.z * BRIDGE_HALF[1]).abs(),
                (along.z * half).abs() + (along.x * BRIDGE_HALF[1]).abs(),
            );
            // The deck spans the stream's carved bed from bank to bank.
            let top = land(cx, cz) + 0.24 + BRIDGE_RISE * (std::f32::consts::PI * t).sin();
            (
                Footprint {
                    min: [cx - hx, cz - hz],
                    max: [cx + hx, cz + hz],
                },
                top,
            )
        })
        .collect()
}

/// `placement` lifted so it stands on the uncarved land rather than in a
/// pond's bowl or the stream's bed: for zones that place Everglade's
/// models without its water, such as the Grove, and for pieces that span
/// the water, such as the footbridge.
#[must_use]
pub fn on_land(mut placement: Placement) -> Placement {
    let [x, z] = placement.at;
    placement.lift += land(x, z) - height(x, z);
    placement
}

/// `placement` lifted so its origin sits `above` meters over the water's
/// gameplay surface at its spot, or over the ground where there is none:
/// lily pads, rowboats, and anything else that floats.
#[must_use]
pub fn afloat(mut placement: Placement, above: f32) -> Placement {
    let [x, z] = placement.at;
    let ground = height(x, z);
    let top = verse_world::social::everglade_water::rest_surface(x, z).unwrap_or(ground);
    placement.lift = top + above - ground;
    placement
}

/// The town's roads, drawn as trodden dirt: each a segment and its half
/// width, m. Every road is a walked route that no blocker covers.
pub const ROADS: [([f32; 2], [f32; 2], f32); 22] = [
    // The commons walk, west of the hall, from the yard to Main Street.
    ([-11.0, -2.0], [-11.0, 46.0], 1.4),
    // Main Street, under the shops.
    ([-34.0, 46.0], [44.0, 46.0], 1.6),
    // Stoop Lane, past the homes, from Walden Woods to Main Street.
    ([-34.0, -40.0], [-34.0, 46.0], 1.4),
    // Hearth Road, from the yard to Stoop Lane.
    ([-34.0, -4.0], [-13.0, -4.0], 1.3),
    // Foundry Road, from the yard to the Server Barn's door.
    ([13.5, 0.0], [45.0, 0.0], 1.2),
    // Studio Road, from Main Street to the Makers' Hall.
    ([43.0, 46.0], [43.0, 36.0], 1.2),
    // Library Way, from the approach to the Knowledge District.
    ([1.8, -29.0], [39.0, -29.0], 1.4),
    ([32.0, -29.0], [32.0, -34.9], 1.2),
    ([39.0, -29.0], [39.0, -25.6], 1.2),
    // The Walden Woods path to the two cabins.
    ([-34.0, -36.0], [-23.0, -36.0], 1.1),
    ([-23.0, -36.0], [-23.0, -42.4], 1.1),
    ([-34.0, -40.0], [-39.4, -40.0], 1.1),
    // Each home's walk from Stoop Lane to its door.
    ([-34.0, -16.0], [-37.4, -16.0], 1.0),
    ([-34.0, 0.0], [-37.4, 0.0], 1.0),
    ([-34.0, 16.0], [-37.4, 16.0], 1.0),
    ([-34.0, 32.0], [-37.4, 32.0], 1.0),
    // Each shop's step from Main Street to its door.
    ([-25.0, 46.0], [-25.0, 49.4], 1.0),
    ([-9.0, 46.0], [-9.0, 49.4], 1.0),
    ([7.0, 46.0], [7.0, 49.4], 1.0),
    ([23.0, 46.0], [23.0, 49.4], 1.0),
    // The cottage's and the reading room's walks.
    ([-13.0, 4.0], [-16.4, 4.0], 1.0),
    ([21.0, 1.2], [21.0, 9.4], 1.0),
];

/// The cobbled streets, drawn over the dirt (`draw`): each a segment and
/// its half width, m. Main Street, Market Way, Library Way, Hearth Road,
/// Brownstone Row, and the commons walk are paved; lanes and paths stay
/// dirt.
pub const PAVED: [([f32; 2], [f32; 2], f32); 6] = [
    ([-100.0, 46.0], [104.0, 46.0], 2.5),
    ([0.0, 46.0], [0.0, 80.0], 2.0),
    ([1.8, -29.0], [104.0, -29.0], 2.0),
    ([-118.0, -8.0], [-34.0, -8.0], 1.9),
    ([-100.0, -78.0], [20.0, -78.0], 2.0),
    ([-11.0, -2.0], [-11.0, 46.0], 1.8),
];
/// The cobbled squares: center and half extents, m. The Fountain Plaza and
/// the market hall's forecourt, the little square before the clock
/// tower on Library Way, Well Square, the Civic Hall's plaza at Main
/// Street's east end, and the Agora's forecourt north of the market hall.
pub const PAVED_SQUARES: [([f32; 2], [f32; 2]); 5] = [
    ([0.0, 74.5], [11.0, 10.5]),
    ([24.0, -29.0], [6.5, 3.0]),
    WELL_SQUARE,
    civic::PLAZA,
    agora::SQUARE,
];
/// Well Square, the city's second plaza, with its well: south of Hearth
/// Road, between the Lantern Quarter and Stoop Lane, center and half
/// extents, m.
pub const WELL_SQUARE: ([f32; 2], [f32; 2]) = ([-60.0, -28.0], [6.0, 5.0]);

/// Every road of the town and the city ([`city::roads`]), with each city
/// building's walk.
#[must_use]
pub fn roads() -> &'static [([f32; 2], [f32; 2], f32)] {
    static ROADS_ALL: std::sync::OnceLock<Vec<([f32; 2], [f32; 2], f32)>> =
        std::sync::OnceLock::new();
    ROADS_ALL.get_or_init(|| {
        let mut out = ROADS.to_vec();
        out.extend(city::roads());
        out
    })
}

/// The generated models of the first town and the city (`generated`):
/// the library on Library Way, the bandshell on the commons, and the
/// city's ([`city::instances`]).
#[must_use]
pub fn generated() -> Vec<generated::Instance> {
    let mut out = FIRST_TOWN.to_vec();
    out.extend(city::instances());
    out
}

/// The first town's generated models: the Stacks and the bandshell.
const FIRST_TOWN: [generated::Instance; 2] = [
    generated::Instance::new("the stacks", &generated::LIBRARY, LIBRARY_AT, NORTH),
    generated::Instance::new("bandshell", &generated::BANDSHELL, BANDSHELL.0, WEST),
];

/// Every generated model whose door stays closed: its name and the point
/// on its walk outside the door, m.
#[must_use]
pub fn fronts() -> Vec<(&'static str, [f32; 2])> {
    generated()
        .into_iter()
        .filter(|i| i.model.inside.is_none())
        .map(|i| (i.name, i.front()))
        .collect()
}

/// Every closed building's doorway, the first town's ([`DOORS`]) and the
/// city's: a point outside it on its walk and a point inside, m.
#[must_use]
pub fn doors() -> Vec<(&'static str, [f32; 2], [f32; 2])> {
    let mut out = DOORS.to_vec();
    out.extend(city::doors());
    out
}

/// Each closed building's doorway: a point outside it on its walk and a
/// point inside, m. The straight line between them passes the doorway.
pub const DOORS: [(&str, [f32; 2], [f32; 2]); 15] = [
    ("cottage", [-15.5, 4.0], [-19.0, 4.0]),
    ("reading room", [21.0, 8.5], [21.0, 12.0]),
    ("bakery", [-25.0, 48.5], [-25.0, 52.0]),
    ("cafe", [-9.0, 48.5], [-9.0, 52.0]),
    ("bookshop", [7.0, 48.5], [7.0, 52.0]),
    ("grocer", [23.0, 48.5], [23.0, 52.0]),
    ("makers hall", [43.0, 36.5], [43.0, 33.0]),
    ("server barn", [44.5, 0.0], [48.0, 0.0]),
    ("old college", [39.0, -26.5], [39.0, -23.0]),
    ("home 1", [-36.5, -16.0], [-40.0, -16.0]),
    ("home 2", [-36.5, 0.0], [-40.0, 0.0]),
    ("home 3", [-36.5, 16.0], [-40.0, 16.0]),
    ("home 4", [-36.5, 32.0], [-40.0, 32.0]),
    ("writing cabin", [-38.5, -40.0], [-42.0, -40.0]),
    ("code cabin", [-23.0, -41.5], [-23.0, -45.0]),
];

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
pub const TREES: [&str; 5] = [
    "nature/CommonTree_5",
    "nature/Pine_2",
    "nature/CommonTree_3",
    "nature/Pine_1",
    "nature/CommonTree_4",
];
/// Trees in the ring.
/// Trees in the Grove's ring, which keeps the first glade's ring.
pub const RING_TREES: u32 = 22;
/// Trees in the town's ring, four times as far out as the Grove's.
const TOWN_RING_TREES: u32 = 25;
/// Bushes tried around the clearing's edge; those on a road or a plot are
/// left out.
const EDGE_BUSHES: u32 = 24;
/// Grass and clover clumps scattered in the clearing.
pub const GRASS: [&str; 4] = [
    "nature/Grass_Common_Short",
    "nature/Grass_Wispy_Short",
    "nature/Clover_1",
    "nature/Grass_Common_Tall",
];
const GRASS_CLUMPS: usize = 160;
/// Small plants at the clearing's edge.
pub const PLANTS: [(&str, f32); 4] = [
    ("nature/Plant_1", 1.0),
    ("nature/Plant_7", 1.2),
    ("nature/Plant_7_Big", 1.0),
    ("nature/Plant_1_Big", 0.5),
];
const EDGE_PLANTS: usize = 60;

/// A deterministic value in `0..1` for `n` in stream `salt`.
pub fn noise(n: u32, salt: u32) -> f32 {
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

/// Plaster colors a kit-built house may be painted, as sRGB; the first is
/// the kit's own.
const PLASTERS: [Option<[f32; 3]>; 9] = [
    None,
    Some([0.93, 0.86, 0.70]),
    Some([0.88, 0.70, 0.45]),
    Some([0.90, 0.74, 0.70]),
    Some([0.96, 0.95, 0.91]),
    Some([0.76, 0.82, 0.68]),
    Some([0.74, 0.82, 0.88]),
    Some([0.95, 0.87, 0.58]),
    Some([0.84, 0.60, 0.48]),
];
/// Roof-tile colors, as sRGB; the kit's own red comes up twice as often.
const ROOFS: [Option<[f32; 3]>; 8] = [
    None,
    None,
    Some([0.55, 0.36, 0.24]),
    Some([0.36, 0.40, 0.47]),
    Some([0.45, 0.53, 0.43]),
    Some([0.30, 0.29, 0.29]),
    Some([0.74, 0.54, 0.30]),
    Some([0.50, 0.33, 0.35]),
];
/// The mean sRGB value of the neutral plaster and tile images
/// (`scripts/blender/everglade_admit.py`), which a paint's factor divides
/// out.
const PLASTER_LUMA: f32 = 0.92;
const TILES_LUMA: f32 = 0.80;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The linear factor that turns a neutral image of mean `luma` into `srgb`.
fn tint(srgb: [f32; 3], luma: f32) -> [f32; 3] {
    srgb.map(|c| (srgb_to_linear(c) / srgb_to_linear(luma)).min(1.0))
}

/// Every kit-built house's footprint with its paint: the city's and the
/// first town's, but not the workshop hall.
fn painted() -> &'static [(([f32; 2], [f32; 2]), Paint)] {
    static PAINTED: std::sync::OnceLock<Vec<(([f32; 2], [f32; 2]), Paint)>> =
        std::sync::OnceLock::new();
    PAINTED.get_or_init(|| {
        let mut rects = vec![COTTAGE, PAVILION, READING_ROOM, MAKERS_HALL, OLD_COLLEGE];
        rects.extend(SHOPS);
        rects.extend(HOMES);
        rects.extend(CABINS);
        rects.extend(city::kit_rects());
        rects
            .into_iter()
            .enumerate()
            .map(|(k, rect)| {
                let k = k as u32;
                let plaster = PLASTERS[(noise(k, 70) * PLASTERS.len() as f32) as usize];
                let roof = ROOFS[(noise(k, 71) * ROOFS.len() as f32) as usize];
                let paint = Paint {
                    plaster: plaster.map(|c| tint(c, PLASTER_LUMA)),
                    roof: roof.map(|c| tint(c, TILES_LUMA)),
                };
                (rect, paint)
            })
            .collect()
    })
}

/// The market stall: one model whose awning `paint` colors
/// (`scene::PAINTED`), in place of the red, blue, and gold stalls the pack
/// carried before.
pub const STALL: &str = "generated/market_stall";
/// The stall's awnings, as `scripts/blender/market_stall.py` colors them:
/// the cloth and its first stripes, then the second stripes, linear. Red,
/// blue, and gold.
const AWNINGS: [[[f32; 3]; 2]; 3] = [
    [[0.72, 0.14, 0.12], [0.93, 0.87, 0.72]],
    [[0.16, 0.30, 0.60], [0.93, 0.87, 0.72]],
    [[0.85, 0.60, 0.10], [0.62, 0.16, 0.12]],
];

/// How `placement` is painted: a kit piece of a kit-built house takes its
/// house's plaster and roof colors (`scene::Paint`), and a sixth-round
/// house colors of its own; anything else keeps the kit's.
pub fn paint(placement: &Placement) -> Paint {
    // Each market stall takes one of the three awnings from where it
    // stands.
    if placement.model == STALL {
        let [x, z] = placement.at;
        let k = (x * 7.0 + z * 13.0).round().abs() as u32;
        let [cloth, stripe] = AWNINGS[(noise(k, 74) * AWNINGS.len() as f32) as usize];
        return Paint {
            plaster: Some(cloth),
            roof: Some(stripe),
        };
    }
    // The sixth and eighth rounds' houses each take colors of their own
    // from where they stand; a `None` keeps the model's.
    if furnish::LIGHT_HOUSES.contains(&placement.model)
        || furnish::TOWNHOUSES.contains(&placement.model)
    {
        let [x, z] = placement.at;
        let k = (x * 7.0 + z * 13.0).round().abs() as u32;
        let plaster = PLASTERS[(noise(k, 72) * PLASTERS.len() as f32) as usize];
        let roof = ROOFS[(noise(k, 73) * ROOFS.len() as f32) as usize];
        return Paint {
            plaster: plaster.map(|c| tint(c, PLASTER_LUMA)),
            roof: roof.map(|c| tint(c, TILES_LUMA)),
        };
    }
    if !placement.model.starts_with("village/") && placement.model != HOUSE_ROOF {
        return Paint::default();
    }
    let [x, z] = placement.at;
    painted()
        .iter()
        .find(|(rect, _)| inside(*rect, x, z, 0.7))
        .map_or_else(Paint::default, |(_, paint)| *paint)
}

/// The lane: three small buildings around the clearing, each a little of
/// the city the map imagines (`docs/verse/everglade-map.png`): a cottage
/// (Stoop Lane), an open café pavilion (Main Street), and a reading room
/// (the Knowledge District). Footprints as (center, half extents), m, on
/// the 2 m grid; each roof is an 8 x 10 round-tile roof.
pub const COTTAGE: ([f32; 2], [f32; 2]) = ([-21.0, 4.0], [4.0, 5.0]);
pub const PAVILION: ([f32; 2], [f32; 2]) = ([22.0, -7.0], [4.0, 5.0]);
pub const READING_ROOM: ([f32; 2], [f32; 2]) = ([20.0, 15.0], [4.0, 5.0]);

/// The town around the glade, after the map's districts. Each is (center,
/// half extents), m, on the 2 m grid. Main Street's four shops face south
/// onto it; the Makers' Hall is the Creative District's; the Server Barn
/// and its fab yard are the Foundry's; the Stacks and the Old College are
/// the Knowledge District's; four homes line Stoop Lane; two cabins stand
/// in Walden Woods.
pub const SHOPS: [([f32; 2], [f32; 2]); 4] = [
    ([-24.0, 55.0], [4.0, 5.0]),
    ([-8.0, 55.0], [4.0, 5.0]),
    ([8.0, 55.0], [4.0, 5.0]),
    ([24.0, 55.0], [4.0, 5.0]),
];
pub const MAKERS_HALL: ([f32; 2], [f32; 2]) = ([44.0, 30.0], [8.0, 5.0]);
pub const SERVER_BARN: ([f32; 2], [f32; 2]) = ([50.0, 0.0], [4.0, 5.0]);
pub const FAB_YARD: ([f32; 2], [f32; 2]) = ([51.0, -13.0], [4.0, 4.0]);
/// The Stacks, the generated library: its reserved ground, from the foot of
/// its steps to its back wall, and its origin at its front wall's center.
pub const STACKS: ([f32; 2], [f32; 2]) = ([32.0, -42.5], [8.0, 7.5]);
const LIBRARY_AT: [f32; 2] = [32.0, -39.5];
pub const OLD_COLLEGE: ([f32; 2], [f32; 2]) = ([40.0, -20.0], [4.0, 5.0]);
pub const HOMES: [([f32; 2], [f32; 2]); 4] = [
    ([-42.0, -16.0], [4.0, 5.0]),
    ([-42.0, 0.0], [4.0, 5.0]),
    ([-42.0, 16.0], [4.0, 5.0]),
    ([-42.0, 32.0], [4.0, 5.0]),
];
pub const CABINS: [([f32; 2], [f32; 2]); 2] =
    [([-44.0, -40.0], [4.0, 5.0]), ([-22.0, -48.0], [4.0, 5.0])];
/// The commons' open bandstand, the community gardens, and the orchard.
pub const BANDSHELL: ([f32; 2], [f32; 2]) = ([12.0, 32.0], [4.0, 5.0]);
pub const GARDENS: ([f32; 2], [f32; 2]) = ([-22.0, 22.0], [6.0, 5.0]);
pub const ORCHARD: ([f32; 2], [f32; 2]) = ([-22.0, 35.0], [6.0, 4.0]);

/// Every reserved footprint: buildings, the yards, and the planted plots.
fn reserved() -> Vec<([f32; 2], [f32; 2])> {
    let mut out = vec![
        HALL,
        STRONGROOM,
        COTTAGE,
        PAVILION,
        READING_ROOM,
        MAKERS_HALL,
        SERVER_BARN,
        FAB_YARD,
        STACKS,
        OLD_COLLEGE,
        BANDSHELL,
        GARDENS,
        ORCHARD,
    ];
    out.extend(SHOPS);
    out.extend(HOMES);
    out.extend(CABINS);
    out.extend(city::reserved());
    out
}

/// Distance from `(x, z)` to the segment from `a` to `b`, m.
#[must_use]
pub fn segment_distance(a: [f32; 2], b: [f32; 2], x: f32, z: f32) -> f32 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 {
        (((x - a[0]) * dx + (z - a[1]) * dz) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (x - a[0] - t * dx).hypot(z - a[1] - t * dz)
}

/// Whether `(x, z)` lies on a road or within `margin` of one.
fn on_road(x: f32, z: f32, margin: f32) -> bool {
    roads()
        .iter()
        .any(|&(a, b, half)| segment_distance(a, b, x, z) <= half + margin)
}

/// Where the lounge bench and the wagon stand.
const BENCH: [f32; 2] = [-25.6, -20.0];
/// The concrete lookout tower at the north-west edge of the clearing, in
/// open ground well off the roads and the workshop, for Meteor Swarm to
/// topple.
pub const TOWER: [f32; 2] = [-45.0, 84.0];
const WAGON: [f32; 2] = [11.0, -23.0];

/// Whether loose ground cover may grow at `(x, z)`: off the floors, the
/// yard, the paths, the station points, the lounge, and the wagon.
fn open_ground(x: f32, z: f32) -> bool {
    let path = x.abs() <= PATH_HALF_WIDTH + 0.8 && z <= YARD.0[1] - YARD.1[1] + 1.0;
    let near = |p: [f32; 2], r: f32| (p[0] - x).hypot(p[1] - z) < r;
    !reserved().iter().any(|r| inside(*r, x, z, 1.5))
        && !inside(YARD, x, z, 1.0)
        && !path
        && !on_road(x, z, 0.8)
        && !PONDS.iter().any(|(c, r)| near(*c, r + 1.8))
        && stream_distance(x, z) > STREAM_HALF + 1.6
        && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], 3.5)
        && !near(BENCH, 4.0)
        && !near([WAGON[0], WAGON[1] - 1.0], 4.5)
        && !STATIONS.iter().any(|s| near(s.at, 2.2))
        && !near(TOWER, 5.0)
}

/// Every placement in the glade but what floats on its water
/// ([`floats`]), which the zone draws and moves itself.
#[must_use]
pub fn placements() -> Vec<Placement> {
    every()
        .into_iter()
        .filter(|p| !floats_on_water(p))
        .collect()
}

/// What floats on the water: the lily pads and the moored rowboats, which
/// bob and which a player boards (`zones::everglade::boats`). The
/// boathouse's rowboat stays a static prop in [`placements`].
#[must_use]
pub fn floats() -> Vec<Placement> {
    every().into_iter().filter(floats_on_water).collect()
}

fn floats_on_water(p: &Placement) -> bool {
    p.model == "generated/lily_pads"
        || (p.model == "generated/rowboat"
            && parks::moorings()
                .iter()
                .any(|(at, _)| (at[0] - p.at[0]).abs() < 1e-4 && (at[1] - p.at[1]).abs() < 1e-4))
}

/// Every placement in the glade, what floats included.
fn every() -> Vec<Placement> {
    let mut out = Vec::with_capacity(512);
    hall(&mut out);
    stations(&mut out);
    strongroom(&mut out);
    yard(&mut out);
    lane(&mut out);
    town(&mut out);
    paths(&mut out);
    glade(&mut out);
    trails::clear(&mut out);
    belvedere::clear(&mut out);
    pylon_field::clear(&mut out);
    out.push(Placement::new(
        "generated/concrete_tower",
        TOWER,
        0.0,
        Collision::None,
    ));
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

/// A closed building on `rect` with `walls` listed south, north, west, east
/// (each from west to east, or from south to north), a wood floor, an
/// 8 x 10 round-tile roof with brick gables for every 8 m of its width,
/// corner posts, and an optional chimney.
fn house(
    out: &mut Vec<Placement>,
    rect: ([f32; 2], [f32; 2]),
    walls: [&[Piece]; 4],
    chimney: Option<[f32; 2]>,
) {
    let ([cx, cz], [hx, hz]) = rect;
    let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
    let [south_wall, north_wall, west_wall, east_wall] = walls;
    for (i, piece) in south_wall.iter().enumerate() {
        wall(out, *piece, [west + 1.0 + 2.0 * i as f32, south], SOUTH);
    }
    for (i, piece) in north_wall.iter().enumerate() {
        wall(out, *piece, [west + 1.0 + 2.0 * i as f32, north], NORTH);
    }
    for (i, piece) in west_wall.iter().enumerate() {
        wall(out, *piece, [west, south + 1.0 + 2.0 * i as f32], WEST);
    }
    for (i, piece) in east_wall.iter().enumerate() {
        wall(out, *piece, [east, south + 1.0 + 2.0 * i as f32], EAST);
    }
    for corner in [[west, south], [east, south], [west, north], [east, north]] {
        out.push(Placement::new(
            "village/Corner_Exterior_Wood",
            corner,
            0.0,
            Collision::None,
        ));
    }
    for i in 0..(hx as i32) {
        for j in 0..(hz as i32) {
            let at = [west + 1.0 + 2.0 * i as f32, south + 1.0 + 2.0 * j as f32];
            out.push(Placement::new("village/Floor_Brick", at, 0.0, Collision::None).lift(0.02));
        }
    }
    roof(out, rect);
    if let Some(at) = chimney {
        out.push(Placement::new("village/Prop_Chimney", at, 0.0, Collision::None).lift(4.9));
    }
}

/// An 8 x 10 round-tile roof for every 8 m of `rect`'s width, each with
/// brick gables at its south and north ends.
fn roof(out: &mut Vec<Placement>, rect: ([f32; 2], [f32; 2])) {
    roof_at(out, rect, WALL_TOP);
}

/// The round-tile roof every house but the workshop hall wears: the kit's
/// `Roof_RoundTiles_8x10` thinned to 55 percent of its triangles
/// (`scripts/blender/kit_lod.py`), which the town repeats dozens of times.
pub const HOUSE_ROOF: &str = "generated/roof_round_tiles_8x10";

/// [`roof`] with its eaves `lift` meters up, over a taller building.
fn roof_at(out: &mut Vec<Placement>, rect: ([f32; 2], [f32; 2]), lift: f32) {
    let ([cx, cz], [hx, hz]) = rect;
    let spans = ((2.0 * hx / 8.0).round() as i32).max(1);
    for k in 0..spans {
        let x = cx - hx + 4.0 + 8.0 * k as f32;
        let x = if spans == 1 { cx } else { x };
        out.push(Placement::new(HOUSE_ROOF, [x, cz], 0.0, Collision::None).lift(lift));
        for (z, facing) in [(cz - hz, SOUTH), (cz + hz, NORTH)] {
            out.push(
                Placement::new("village/Roof_Front_Brick8", [x, z], facing, Collision::None)
                    .lift(lift),
            );
        }
    }
}

/// The lane's three buildings.
fn lane(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Round, Timber};
    // Stoop Lane cottage: its door faces the yard to the east, with a chimney.
    house(
        out,
        COTTAGE,
        [
            &[Round, Plain, Plain, Round],
            &[Plain, Flat, Flat, Plain],
            &[Plain, Flat, Timber, Flat, Plain],
            &[Round, Timber, Door, Timber, Round],
        ],
        Some([COTTAGE.0[0] - 2.0, COTTAGE.0[1] + 2.5]),
    );
    let ([cx, cz], [hx, _]) = COTTAGE;
    out.push(
        Placement::new(
            "village/Door_4_Round",
            [cx + hx - 0.35, cz + 0.5],
            0.0,
            Collision::None,
        )
        .lift(0.02),
    );
    out.push(Placement::new(
        "nature/Bush_Common_Flowers",
        [cx + hx + 1.4, cz - 3.0],
        0.4,
        Collision::Core(0.5),
    ));
    out.push(Placement::new(
        "nature/Bush_Common_Flowers",
        [cx + hx + 1.4, cz + 3.0],
        2.1,
        Collision::Core(0.5),
    ));

    // Café pavilion: an open timber roof on corner posts over tables.
    let ([px, pz], [phx, phz]) = PAVILION;
    for corner in [
        [px - phx, pz - phz],
        [px + phx, pz - phz],
        [px - phx, pz + phz],
        [px + phx, pz + phz],
        [px - phx, pz],
        [px + phx, pz],
    ] {
        out.push(Placement::new(
            "village/Corner_Exterior_Wood",
            corner,
            0.0,
            Collision::Core(0.2),
        ));
    }
    roof(out, PAVILION);
    for z in [pz - 2.4, pz + 2.4] {
        out.push(Placement::new(
            "props/Table_Large",
            [px, z],
            FRAC_PI_2,
            Collision::Bounds,
        ));
        for side in [-1.3_f32, 1.3] {
            for along in [-0.9_f32, 0.9] {
                out.push(Placement::new(
                    "props/Stool",
                    [px + side, z + along],
                    if side < 0.0 { EAST } else { WEST },
                    Collision::None,
                ));
            }
        }
    }
    out.push(Placement::new("props/Banner_2", [px - phx, pz], WEST, Collision::None).lift(0.6));

    // Reading room: a brick-based timber hall with tall windows; its door
    // faces south toward the strongroom and the yard.
    house(
        out,
        READING_ROOM,
        [
            &[Base, Round, Door, Base],
            &[Timber, Flat, Flat, Timber],
            &[Base, Flat, Timber, Flat, Base],
            &[Base, Round, Timber, Round, Base],
        ],
        None,
    );
    let ([rx, rz], [_, rhz]) = READING_ROOM;
    out.push(
        Placement::new(
            "village/Door_4_Round",
            [rx + 1.0 - 0.5, rz - rhz + 0.35],
            -FRAC_PI_2,
            Collision::None,
        )
        .lift(0.02),
    );
    out.push(Placement::new(
        "nature/Bush_Common",
        [rx - 3.0, rz - rhz - 1.4],
        1.0,
        Collision::Core(0.5),
    ));
}

/// Shorthand for a placement that blocks by its bounds.
fn prop(out: &mut Vec<Placement>, model: &'static str, at: [f32; 2], yaw: f32) {
    out.push(Placement::new(model, at, yaw, Collision::Bounds));
}

/// Shorthand for ground cover or dressing that does not block.
fn dress(out: &mut Vec<Placement>, model: &'static str, at: [f32; 2], yaw: f32, scale: f32) {
    out.push(Placement::new(model, at, yaw, Collision::None).scale(scale));
}

/// A tree with its trunk blocking.
fn tree(out: &mut Vec<Placement>, model: &'static str, at: [f32; 2], yaw: f32, scale: f32) {
    out.push(
        Placement::new(model, at, yaw, Collision::Core(0.4))
            .scale(scale)
            .lift(-0.15),
    );
}

/// A lantern on a wall facing `outward`, `along` meters from `center` along
/// the wall.
fn wall_lantern(out: &mut Vec<Placement>, center: [f32; 2], outward: f32, along: f32) {
    let normal = crate::controller::forward(outward);
    let side = crate::controller::forward(outward + FRAC_PI_2);
    let at = [
        center[0] + normal.x * 0.1 + side.x * along,
        center[1] + normal.z * 0.1 + side.z * along,
    ];
    out.push(Placement::new("props/Lantern_Wall", at, outward, Collision::Bounds).lift(1.5));
}

/// The town around the glade (`docs/verse/everglade.md`, Layout): the
/// commons, Main Street, the Creative District, the Foundry, the Knowledge
/// District, Stoop Lane's homes, Walden Woods, the gardens, and the orchard.
fn town(out: &mut Vec<Placement>) {
    commons(out);
    main_street(out);
    makers_hall(out);
    foundry(out);
    knowledge(out);
    homes(out);
    woods(out);
    gardens(out);
    out.extend(FIRST_TOWN.iter().map(generated::Instance::placement));
    city::build(out);
    streets::build(out);
}

/// The commons: the great lawn north of the hall, with Lantern Pond, its
/// reeds and stones, benches facing the water, and the open bandshell.
fn commons(out: &mut Vec<Placement>) {
    for (k, &(center, r)) in PONDS.iter().enumerate() {
        let stones = 9 + 3 * k as u32;
        for n in 0..stones {
            let angle = (n as f32 + 0.5 * noise(n, 40 + k as u32)) / stones as f32 * TAU;
            let at = [
                center[0] + angle.cos() * (r + 0.4),
                center[1] + angle.sin() * (r + 0.4),
            ];
            if n % 3 == 0 {
                out.push(
                    Placement::new(
                        "nature/Rock_Medium_2",
                        at,
                        noise(n, 41) * TAU,
                        Collision::None,
                    )
                    .scale(0.28),
                );
            } else {
                let model = if n % 2 == 0 {
                    "nature/Grass_Common_Tall"
                } else {
                    "nature/Fern_1"
                };
                let scale = if n % 2 == 0 { 1.1 } else { 0.35 };
                dress(out, model, at, noise(n, 42) * TAU, scale);
            }
        }
    }
    let ([px, pz], r) = PONDS[0];
    // Benches face the water from the south and the east; the boathouse
    // stands on the north bank (`city::GROUNDS`).
    for (at, yaw) in [([px, pz - r - 2.6], 0.0), ([px + r + 2.6, pz], -FRAC_PI_2)] {
        prop(out, "props/Bench", at, yaw + PI);
    }
    // The bandshell faces the pond across the lawn (`generated`), with a
    // music stand and stools on its stage.
    let ([bx, bz], _) = BANDSHELL;
    for (at, model) in [
        ([bx + 0.4, bz], "props/BookStand"),
        ([bx + 1.8, bz - 1.6], "props/Stool"),
        ([bx + 1.8, bz + 1.4], "props/Stool"),
    ] {
        out.push(Placement::new(model, at, WEST, Collision::None).lift(0.9));
    }
    for (i, at) in [[-6.0, 20.0], [5.0, 22.0], [-7.0, 38.0], [3.0, 40.0]]
        .into_iter()
        .enumerate()
    {
        let model = if i % 2 == 0 {
            "nature/Flower_3_Group"
        } else {
            "nature/Flower_4_Group"
        };
        dress(out, model, at, noise(i as u32, 43) * TAU, 1.0);
    }
    tree(out, foliage::PARK_TREES[1], [-6.0, 41.0], 0.4, 1.0);
    tree(out, foliage::PARK_TREES[0], [22.0, 38.0], 2.2, 0.9);
}

/// Main Street: four shops on its north side (a bakery, a café, a bookshop,
/// and a grocer), market stalls and benches on its south side.
fn main_street(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Round, Timber};
    let fronts: [[Piece; 4]; 4] = [
        [Round, Door, Timber, Round],
        [Flat, Door, Flat, Timber],
        [Round, Door, Round, Plain],
        [Timber, Door, Flat, Round],
    ];
    for (k, (rect, front)) in SHOPS.iter().zip(fronts).enumerate() {
        let chimney = (k % 2 == 0).then(|| [rect.0[0] + 2.0, rect.0[1] + 2.5]);
        house(
            out,
            *rect,
            [
                &front,
                &[Plain, Flat, Flat, Plain],
                &[Base, Timber, Plain, Timber, Base],
                &[Base, Round, Timber, Plain, Base],
            ],
            chimney,
        );
        let ([cx, cz], [_, hz]) = *rect;
        let south = cz - hz;
        // A counter inside, seen through the door.
        prop(out, "props/Table_Large", [cx, cz + 2.6], 0.0);
        // What each shop sets out by its door.
        match k {
            0 => {
                prop(out, "village/Prop_Crate", [cx + 2.2, south - 1.0], 0.3);
                prop(out, "village/Prop_Crate", [cx + 3.2, south - 0.9], -0.2);
            }
            1 => {
                prop(out, "props/Table_Large", [cx + 2.0, south - 1.6], 0.0);
                for x in [cx + 0.2, cx + 3.8] {
                    prop(out, "props/Stool", [x, south - 1.6], 0.0);
                }
            }
            2 => {
                prop(out, "props/BookStand", [cx + 2.0, south - 0.8], SOUTH);
                out.push(
                    Placement::new(
                        "props/Book_Stack_1",
                        [cx - 0.6, cz + 2.6],
                        0.4,
                        Collision::None,
                    )
                    .lift(0.81),
                );
            }
            _ => {
                prop(out, "village/Prop_Crate", [cx + 2.0, south - 1.0], 0.1);
                prop(out, "village/Prop_Crate", [cx + 3.1, south - 1.1], 0.6);
                out.push(
                    Placement::new(
                        "village/Prop_Crate",
                        [cx + 2.0, south - 1.0],
                        0.9,
                        Collision::None,
                    )
                    .lift(1.06),
                );
            }
        }
    }
    // Market stalls and benches across the street, by the commons.
    for (k, x) in [4.0_f32, 18.0, 32.0].into_iter().enumerate() {
        prop(out, STALL, [x, 42.2], NORTH);
        prop(out, "generated/barrel", [x + 2.0, 42.4], 0.4 * k as f32);
    }
    for x in [-3.0_f32, 11.0, 25.0, 38.0] {
        prop(out, "props/Bench", [x, 43.0], PI);
    }
}

/// The Creative District's Makers' Hall: a long hall with double doors on
/// Studio Road and workbenches, an anvil, and crates inside.
fn makers_hall(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Round, Timber};
    house(
        out,
        MAKERS_HALL,
        [
            &[Base, Round, Timber, Round, Round, Timber, Round, Base],
            &[Plain, Round, Timber, Door, Door, Timber, Round, Plain],
            &[Base, Flat, Timber, Flat, Base],
            &[Base, Flat, Timber, Flat, Base],
        ],
        Some([MAKERS_HALL.0[0] - 5.0, MAKERS_HALL.0[1] - 2.5]),
    );
    let ([cx, cz], [_, hz]) = MAKERS_HALL;
    wall_lantern(out, [cx, cz + hz], NORTH, 0.0);
    for x in [cx - 5.0, cx + 5.0] {
        prop(out, "props/Workbench", [x, cz - 2.6], 0.0);
        prop(out, "props/Stool", [x, cz - 1.4], 0.0);
    }
    prop(out, "props/Anvil", [cx + 2.0, cz - 2.0], 0.3);
    prop(out, "props/Table_Large", [cx - 2.4, cz - 2.8], 0.0);
    prop(out, "village/Prop_Crate", [cx + 6.8, cz + 3.4], 0.2);
    out.push(Placement::new(
        "nature/Bush_Common_Flowers",
        [cx - 9.6, cz + 3.0],
        1.3,
        Collision::Core(0.5),
    ));
}

/// The Foundry: the Server Barn, its racks of metal crates, and the fab
/// yard's fenced bench, anvil, wagon, and stock.
fn foundry(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Timber};
    house(
        out,
        SERVER_BARN,
        [
            &[Timber, Timber, Timber, Timber],
            &[Timber, Base, Base, Timber],
            &[Base, Timber, Door, Timber, Base],
            &[Base, Timber, Timber, Timber, Base],
        ],
        Some([SERVER_BARN.0[0] + 2.0, SERVER_BARN.0[1] + 2.5]),
    );
    let ([cx, cz], _) = SERVER_BARN;
    // The racks: metal crates stacked against the back wall.
    prop(out, "props/Crate_Metal", [cx + 2.6, cz - 3.2], 0.0);
    out.push(
        Placement::new(
            "props/Crate_Metal",
            [cx + 2.6, cz - 3.2],
            0.0,
            Collision::None,
        )
        .lift(0.87),
    );
    prop(out, "village/Prop_Crate", [cx + 2.6, cz + 3.2], 0.3);
    // The fab yard: a fence on three sides, open toward the barn.
    let ([fx, fz], [fhx, fhz]) = FAB_YARD;
    let (west, east, south) = (fx - fhx, fx + fhx, fz - fhz);
    for i in 0..4 {
        let model = if i % 2 == 0 {
            "village/Prop_WoodenFence_Single"
        } else {
            "village/Prop_WoodenFence_Extension1"
        };
        let along = -fhx + 1.0 + 2.0 * i as f32;
        prop(out, model, [fx + along, south], 0.0);
        prop(out, model, [east, fz + along], FRAC_PI_2);
        // The west side leaves a gate by the barn.
        if i < 3 {
            prop(out, model, [west, fz + along], FRAC_PI_2);
        }
    }
    prop(out, "props/Workbench", [fx + 2.0, south + 1.2], PI);
    prop(out, "props/Anvil", [fx - 1.2, south + 1.6], 0.5);
    prop(out, "village/Prop_Wagon", [fx + 2.4, fz + 2.6], FRAC_PI_2);
    prop(out, "props/Crate_Metal", [fx - 2.4, fz + 0.6], 0.2);
    prop(out, "village/Prop_Crate", [fx - 2.6, fz - 0.8], 0.4);
    prop(out, "village/Prop_Crate", [fx - 1.4, fz - 1.4], -0.3);
}

/// The Knowledge District: the Stacks, the generated library up its steps
/// (`generated`), and the Old College beside Library Way.
fn knowledge(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Round, Timber};
    house(
        out,
        OLD_COLLEGE,
        [
            &[Base, Door, Round, Base],
            &[Timber, Flat, Flat, Timber],
            &[Base, Round, Timber, Round, Base],
            &[Base, Flat, Timber, Flat, Base],
        ],
        Some([OLD_COLLEGE.0[0] + 2.0, OLD_COLLEGE.0[1] + 2.5]),
    );
    let ([ox, oz], [_, ohz]) = OLD_COLLEGE;
    prop(out, "props/Bookcase_2", [ox + 2.0, oz + ohz - 0.6], PI);
    prop(
        out,
        "props/CandleStick_Triple",
        [ox - 2.6, oz + ohz - 0.5],
        PI,
    );
    out.push(Placement::new("village/Prop_Vine1", [ox - 4.1, oz], WEST, Collision::None).lift(2.8));
}

/// Stoop Lane: four homes facing the lane, each with a lantern by its door
/// and a little garden.
fn homes(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Round, Timber};
    let fronts: [[Piece; 5]; 4] = [
        [Round, Timber, Door, Timber, Round],
        [Flat, Plain, Door, Plain, Flat],
        [Round, Plain, Door, Timber, Flat],
        [Flat, Timber, Door, Plain, Round],
    ];
    for (k, (rect, front)) in HOMES.iter().zip(fronts).enumerate() {
        house(
            out,
            *rect,
            [
                &[Plain, Flat, Flat, Plain],
                &[Base, Round, Timber, Base],
                &[Plain, Flat, Timber, Flat, Plain],
                &front,
            ],
            Some([rect.0[0] - 2.0, rect.0[1] + 2.5]),
        );
        let ([cx, cz], [hx, _]) = *rect;
        let door = [cx + hx, cz];
        wall_lantern(out, door, EAST, 1.6);
        let flowers = if k % 2 == 0 {
            "nature/Bush_Common_Flowers"
        } else {
            "nature/Bush_Common"
        };
        out.push(Placement::new(
            flowers,
            [door[0] + 1.3, cz - 3.4],
            k as f32,
            Collision::Core(0.5),
        ));
        dress(
            out,
            "nature/Flower_3_Group",
            [door[0] + 1.2, cz + 3.4],
            k as f32 * 1.7,
            0.8,
        );
        if k % 2 == 1 {
            out.push(
                Placement::new("village/Prop_Vine1", [cx, cz + 5.1], NORTH, Collision::None)
                    .lift(2.8),
            );
        }
    }
}

/// Walden Woods: two timber cabins among pines, with a bench and mushrooms.
fn woods(out: &mut Vec<Placement>) {
    use Piece::{Base, Door, Flat, Plain, Timber};
    house(
        out,
        CABINS[0],
        [
            &[Timber, Flat, Timber, Plain],
            &[Timber, Plain, Timber, Timber],
            &[Timber, Plain, Flat, Plain, Timber],
            &[Timber, Flat, Door, Timber, Timber],
        ],
        Some([CABINS[0].0[0] - 2.0, CABINS[0].0[1] - 2.5]),
    );
    house(
        out,
        CABINS[1],
        [
            &[Timber, Plain, Flat, Timber],
            &[Base, Door, Flat, Timber],
            &[Timber, Flat, Timber, Plain, Timber],
            &[Timber, Plain, Flat, Timber, Timber],
        ],
        Some([CABINS[1].0[0] + 2.0, CABINS[1].0[1] - 2.5]),
    );
    for (i, (model, at, scale)) in [
        (foliage::FIR.0, [-31.0, -48.0], 1.1),
        (foliage::FIR.0, [-38.0, -51.0], 1.2),
        (foliage::PARK_TREES[1], [-53.0, -33.0], 1.1),
        (foliage::FIR.0, [-14.0, -53.0], 1.0),
        (foliage::PARK_TREES[0], [-31.0, -58.0], 1.0),
        (foliage::FIR.0, [-50.0, -48.0], 1.3),
        (foliage::PARK_TREES[2], [-12.0, -61.0], 1.1),
        (foliage::FIR.0, [-29.0, -29.0], 0.9),
    ]
    .into_iter()
    .enumerate()
    {
        tree(out, model, at, noise(i as u32, 44) * TAU, scale);
    }
    prop(out, "props/Bench", [-28.0, -40.2], FRAC_PI_2);
    for (i, at) in [
        [-30.0, -46.5],
        [-37.0, -49.6],
        [-15.4, -52.0],
        [-49.0, -46.6],
    ]
    .into_iter()
    .enumerate()
    {
        dress(
            out,
            "nature/Mushroom_Common",
            at,
            i as f32 * 1.9,
            1.3 + 0.2 * i as f32,
        );
    }
    for (i, at) in [[-35.5, -45.0], [-26.5, -55.0], [-47.0, -30.0]]
        .into_iter()
        .enumerate()
    {
        dress(out, "nature/Fern_1", at, i as f32 * 2.3, 0.35);
    }
}

/// The community gardens' fenced beds and the orchard's rows of young
/// fruit trees between the commons walk and Stoop Lane.
fn gardens(out: &mut Vec<Placement>) {
    garden_plot(out, GARDENS, 45);
    orchard(out);
}

/// A fenced garden on `rect`, gated in the middle of its east side, with
/// four rows of beds; `salt` varies the plants' headings.
fn garden_plot(out: &mut Vec<Placement>, rect: ([f32; 2], [f32; 2]), salt: u32) {
    let ([gx, gz], [ghx, ghz]) = rect;
    let (west, east, south, north) = (gx - ghx, gx + ghx, gz - ghz, gz + ghz);
    for i in 0..6 {
        let x = west + 1.0 + 2.0 * i as f32;
        prop(out, "village/Prop_WoodenFence_Single", [x, south], 0.0);
        prop(out, "village/Prop_WoodenFence_Single", [x, north], 0.0);
    }
    for j in 0..5 {
        let z = south + 1.0 + 2.0 * j as f32;
        prop(
            out,
            "village/Prop_WoodenFence_Extension1",
            [west, z],
            FRAC_PI_2,
        );
        // A gate in the middle of the east side, toward the commons walk.
        if j != 2 {
            prop(
                out,
                "village/Prop_WoodenFence_Extension1",
                [east, z],
                FRAC_PI_2,
            );
        }
    }
    let beds: [(&str, f32); 4] = [
        ("nature/Plant_7_Big", 1.0),
        ("nature/Plant_1", 0.9),
        ("nature/Grass_Common_Tall", 0.9),
        ("nature/Plant_7", 1.3),
    ];
    for (row, (model, scale)) in beds.into_iter().enumerate() {
        let z = south + 1.4 + 2.4 * row as f32;
        for k in 0..5 {
            let x = west + 1.6 + 2.0 * k as f32;
            dress(
                out,
                model,
                [x, z],
                noise(k + 8 * row as u32, salt) * TAU,
                scale,
            );
        }
    }
}

/// The first orchard's young fruit trees.
fn orchard(out: &mut Vec<Placement>) {
    let ([ox, oz], [ohx, ohz]) = ORCHARD;
    for i in 0..3 {
        for j in 0..2 {
            let at = [
                ox - ohx + 2.0 + 4.0 * i as f32,
                oz - ohz + 2.0 + 4.0 * j as f32,
            ];
            let model = if (i + j) % 2 == 0 {
                "nature/CommonTree_5"
            } else {
                "nature/CommonTree_3"
            };
            tree(out, model, at, noise(i * 2 + j, 46) * TAU, 0.6);
        }
    }
    dress(
        out,
        "nature/Mushroom_Common",
        [ox - 3.0, oz + 0.2],
        0.4,
        1.2,
    );
    dress(out, "nature/Clover_1", [ox + 1.0, oz - 0.3], 1.4, 1.0);
}

/// Furniture ahead of each station's standing point.
fn stations(out: &mut Vec<Placement>) {
    // Desks: one standing desk per seat, a workbench brought up to a
    // standing desk's height, with no stool: a seat stands to type. The
    // monitor boards are drawn by Verse.
    for desk in DESKS {
        out.push(
            Placement::new(
                "props/Workbench",
                [desk.seat[0], DESK_Z],
                PI,
                Collision::Bounds,
            )
            .scale(super::pose::DESK_SCALE),
        );
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
    for k in 0..TOWN_RING_TREES {
        let angle = (k as f32 + 0.4 * noise(k, 1)) / TOWN_RING_TREES as f32 * TAU;
        let r = 146.0 + (k % 3) as f32 * 9.0 + 4.0 * noise(k, 2);
        // Every third place keeps a kit tree; the others hold a trio of the
        // forest's cheap trees, which the edge stands around them match
        // (`streets::woods`).
        if k % 3 != 0 {
            for i in 0..3_u32 {
                let a = noise(k * 3 + i, 10) * TAU;
                let d = 2.5 + 3.0 * noise(k * 3 + i, 11);
                let model = [
                    "generated/spruce_low",
                    "generated/oak_low",
                    "generated/birch_low",
                ][((k + i) % 3) as usize];
                out.push(
                    Placement::new(
                        model,
                        [angle.cos() * r + a.cos() * d, angle.sin() * r + a.sin() * d],
                        noise(k * 3 + i, 12) * TAU,
                        Collision::Core(0.35),
                    )
                    .scale(1.0 + 0.4 * noise(k * 3 + i, 13))
                    .lift(-0.1),
                );
            }
            continue;
        }
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
    // Bushes around the clearing's edge, where the ground is open.
    for k in 0..EDGE_BUSHES {
        let angle = (k as f32 + 0.5 * noise(k, 5)) / EDGE_BUSHES as f32 * TAU;
        let at = [angle.cos() * 132.0, angle.sin() * 132.0];
        if !open_ground(at[0], at[1]) {
            continue;
        }
        let model = if k % 2 == 0 {
            "nature/Bush_Common"
        } else {
            "nature/Bush_Common_Flowers"
        };
        out.push(Placement::new(model, at, noise(k, 15) * TAU, Collision::Core(0.55)).scale(1.1));
    }
    for (i, (model, at)) in [
        ("nature/Rock_Medium_1", [-30.0, 64.0]),
        ("nature/Rock_Medium_3", [116.0, 14.0]),
        ("nature/Rock_Medium_2", [114.0, -44.0]),
        ("nature/Rock_Medium_1", [-124.0, 10.0]),
        ("nature/Rock_Medium_3", [26.0, -58.0]),
        ("nature/Rock_Medium_2", [-6.0, -50.0]),
        ("nature/Rock_Medium_1", [30.0, -112.0]),
        ("nature/Rock_Medium_3", [-48.0, 106.0]),
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Placement::new(model, at, noise(i as u32, 6) * TAU, Collision::Bounds).scale(0.6));
    }
    // The long meadow's flowers south of the Knowledge District.
    for (i, at) in [
        [20.0, -52.0],
        [8.0, -56.0],
        [34.0, -54.0],
        [44.0, -48.0],
        [-3.0, -54.0],
    ]
    .into_iter()
    .enumerate()
    {
        let model = if i % 2 == 0 {
            "nature/Flower_4_Group"
        } else {
            "nature/Flower_3_Group"
        };
        out.push(Placement::new(
            model,
            at,
            noise(i as u32, 16) * TAU,
            Collision::None,
        ));
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
    scatter(out, &GRASS.map(|m| (m, 1.0)), GRASS_CLUMPS, 12.0, 132.0, 20);
    scatter(out, &PLANTS, EDGE_PLANTS, 120.0, 138.0, 21);
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
