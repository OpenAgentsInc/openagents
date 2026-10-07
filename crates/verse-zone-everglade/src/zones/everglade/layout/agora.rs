//! The Agora: the agent sales floor's trading hall
//! (`docs/sales/agent-sales-floor.md`), in the Greco-futurism style
//! (`docs/verse/greco-futurism.md`).
//!
//! The sales plan proposed it at Main Street's west end, facing the Civic
//! Hall, but that site doesn't fit: the snug's lot on Lantern Road reaches
//! to within 6 m of the street's axis, the orchard closes it from the
//! north, and past x = −110 the ground rises out of the clearing. So it
//! stands on the plan's fallback: open ground north of the market hall,
//! west of where the north trail leaves the Fountain Plaza, its stair
//! facing south toward the plaza, with a cobbled forecourt ([`SQUARE`])
//! and a walk ([`WALKS`]) from the plaza's north-west corner.
//!
//! `scripts/blender/greco_futurism.py` builds it as one model,
//! `generated/agora`, with a far level of detail: ten steps between hedged
//! planter walls to a podium, four smooth columns under an entablature
//! whose frieze carries small amber panes, and tall bronze doors standing
//! open on one tall trading floor. Three rows of standing desks, with no
//! chairs, face the leaderboard wall across the floor's head, each
//! station with two screens, a desk phone, and a headset; an amber ticker
//! band runs round the hall under its coffered ceiling, and the bell's
//! stele stands before the board. A lower wing on each side holds Paul's
//! glass-walled corner office and the training room, with its whiteboard
//! and four role-play booths.
//!
//! It stands on open ground like the Civic Hall ([`super::civic`]): its
//! walls, columns, planter walls, desks, lecterns, and screens block
//! walking by the boxes of its `agora.footprint.json`, its flat roofs are
//! surfaces to land on, and, like every generated model, it collides by
//! its own triangles and breaks (`demolition::carve`).
//!
//! Nobody works here yet. The places agents will stand are data:
//! [`DESKS`], [`PAUL`], [`OWNER`], [`STANDUP`], [`TEACHER`], and
//! [`BOOTHS`]. The bell is the zone's to swing ([`Bell`]): the model
//! carries its stele and yoke, and the zone draws the bell, so a settled
//! deal can ring it once the payment ledger is wired to [`Bell::ring`].

use super::estate::{Fixture, ROOM_GRADE};
use super::generated::{GableRoof, Instance, Model};
use crate::mesh::{Mesh, Vertex};
use crate::pbr::{Lamp, MAX_LAMPS, Neon};
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

/// The Agora, in its glTF frame: +z out of the front, the origin on the
/// ground at the center of the lowest step's front edge.
pub const AGORA_HALL: Model = Model {
    name: "generated/agora",
    blocks: &[
        // The hedged planter walls beside the stair.
        [-8.8, -4.6, -4.0, 0.0, 1.5],
        [4.6, 8.8, -4.0, 0.0, 1.5],
        // The portico's four columns.
        [-7.145, -6.055, -5.945, -4.855, 9.4],
        [-2.945, -1.855, -5.945, -4.855, 9.4],
        [1.855, 2.945, -5.945, -4.855, 9.4],
        [6.055, 7.145, -5.945, -4.855, 9.4],
        // The facade on each side of the door, the floor's side walls on
        // each side of the doorways into the wings, Paul's glass wall, and
        // the back wall.
        [-8.4, -1.5, -8.0, -7.6, 9.4],
        [1.5, 8.4, -8.0, -7.6, 9.4],
        [-8.4, -8.0, -10.8, -8.0, 9.4],
        [-8.4, -8.0, -28.0, -12.2, 9.4],
        [-8.3, -8.1, -10.8, -10.4, 5.2],
        [-8.3, -8.1, -20.6, -12.2, 5.2],
        [8.0, 8.4, -10.8, -8.0, 9.4],
        [8.0, 8.4, -28.0, -12.2, 9.4],
        [-8.4, 8.4, -28.0, -27.6, 9.4],
        // The wings' front, back, and outer walls.
        [-15.2, -8.4, -10.4, -10.0, 6.4],
        [-15.2, -8.4, -21.0, -20.6, 6.4],
        [-15.2, -14.8, -20.6, -10.4, 6.4],
        [8.4, 15.2, -10.4, -10.0, 6.4],
        [8.4, 15.2, -21.0, -20.6, 6.4],
        [14.8, 15.2, -20.6, -10.4, 6.4],
        // The lantern posts at the stair's foot.
        [-5.52, -5.28, 0.48, 0.72, 2.3],
        [5.28, 5.52, 0.48, 0.72, 2.3],
        // The desk banks, three rows either side of the center aisle.
        [-6.4, -1.6, -13.6, -12.8, 2.68],
        [1.6, 6.4, -13.6, -12.8, 2.68],
        [-6.4, -1.6, -17.2, -16.4, 2.68],
        [1.6, 6.4, -17.2, -16.4, 2.68],
        [-6.4, -1.6, -20.8, -20.0, 2.68],
        [1.6, 6.4, -20.8, -20.0, 2.68],
        // The bell's stele and yoke.
        [-0.7, 0.7, -24.42, -23.58, 4.38],
        // Paul's standing desk and the owner's lectern.
        [-12.6, -11.8, -16.7, -14.5, 2.7],
        [-10.45, -9.95, -18.85, -18.35, 2.68],
        // The booths' lecterns, and the screens between the booths.
        [9.85, 10.35, -14.65, -14.15, 2.68],
        [9.85, 10.35, -15.95, -15.45, 2.68],
        [12.85, 13.35, -14.65, -14.15, 2.68],
        [12.85, 13.35, -15.95, -15.45, 2.68],
        [9.85, 10.35, -18.55, -18.05, 2.68],
        [9.85, 10.35, -19.6, -19.1, 2.68],
        [12.85, 13.35, -18.55, -18.05, 2.68],
        [12.85, 13.35, -19.6, -19.1, 2.68],
        [11.55, 11.65, -20.6, -13.4, 3.42],
        [9.6, 11.5, -17.05, -16.95, 3.42],
        [11.7, 13.6, -17.05, -16.95, 3.42],
    ],
    roofs: &[
        GableRoof {
            center: [-11.8, -15.5],
            slopes_z: true,
            half: [5.5, 3.4],
            eave: 6.75,
            ridge: 6.76,
        },
        GableRoof {
            center: [11.8, -15.5],
            slopes_z: true,
            half: [5.5, 3.4],
            eave: 6.75,
            ridge: 6.76,
        },
        GableRoof {
            center: [0.0, -16.45],
            slopes_z: true,
            half: [11.05, 7.9],
            eave: 11.9,
            ridge: 11.91,
        },
    ],
    front: [0.0, 1.0],
    inside: Some(MIDDLE),
};

/// The trading floor's middle, on the entry axis, in the hall's frame.
pub const MIDDLE: [f32; 2] = [0.0, -18.0];

/// Where the hall stands: its stair's foot north of the market hall,
/// facing south toward the Fountain Plaza.
pub const AGORA: Instance = Instance::new("agora", &AGORA_HALL, [-24.0, 99.0], PI);

/// The cobbled forecourt before the stair: center and half extents, m.
pub const SQUARE: ([f32; 2], [f32; 2]) = ([-24.0, 95.5], [6.0, 3.0]);

/// The walk from the Fountain Plaza's north-west corner to the forecourt,
/// as roads: from, to, and half width, m.
pub const WALKS: [([f32; 2], [f32; 2], f32); 2] = [
    ([-11.0, 83.0], [-24.0, 83.0], 1.4),
    ([-24.0, 83.0], [-24.0, 92.5], 1.4),
];

/// The ground kept clear round the hall, so no tree's crown reaches
/// through its walls: center and half extents, m. It runs 5 m past the
/// walls behind and on the west, and to the chapel's lot on the east.
pub const CLEAR: ([f32; 2], [f32; 2]) = ([-25.1, 114.0], [19.4, 18.0]);

/// The floor over the hall's ground, m: the podium's top.
pub const FLOOR: f32 = 1.62;

/// An agent's place in the hall: where it stands and the point it faces,
/// in the hall's frame, x and z, m.
pub type Station = ([f32; 2], [f32; 2]);

/// The trading floor's desk stations, three rows of six, front row first,
/// west to east in the hall's frame: each agent stands behind its desk,
/// facing its screens and the leaderboard beyond.
pub const DESKS: [Station; 18] = {
    const XS: [f32; 6] = [-5.6, -4.0, -2.4, 2.4, 4.0, 5.6];
    const ROWS: [f32; 3] = [13.2, 16.8, 20.4];
    let mut out = [([0.0; 2], [0.0; 2]); 18];
    let mut i = 0;
    while i < 18 {
        let (x, row) = (XS[i % 6], ROWS[i / 6]);
        out[i] = ([x, -(row - 0.9)], [x, -(row + 1.0)]);
        i += 1;
    }
    out
};

/// Paul's place: behind his standing desk in the corner office, facing
/// the floor through the glass wall.
pub const PAUL: Station = ([-13.2, -15.6], [-8.2, -15.6]);

/// Where Paul stands for the morning stand-up: beside the bell before the
/// leaderboard, facing the floor.
pub const STANDUP: Station = ([2.6, -22.4], [2.6, -16.0]);

/// The owner's place: behind the lectern in Paul's office where sends and
/// hires wait for approval.
pub const OWNER: Station = ([-10.2, -19.4], [-10.2, -15.0]);

/// Where a teacher stands at the training room's whiteboard, facing the
/// room.
pub const TEACHER: Station = ([12.0, -11.3], [12.0, -14.0]);

/// The four role-play booths, each two places behind standing lecterns
/// facing each other across the booth.
pub const BOOTHS: [[Station; 2]; 4] = [
    [
        ([10.1, -13.65], [10.1, -16.45]),
        ([10.1, -16.45], [10.1, -13.65]),
    ],
    [
        ([13.1, -13.65], [13.1, -16.45]),
        ([13.1, -16.45], [13.1, -13.65]),
    ],
    [
        ([10.1, -17.55], [10.1, -20.1]),
        ([10.1, -20.1], [10.1, -17.55]),
    ],
    [
        ([13.1, -17.55], [13.1, -20.1]),
        ([13.1, -20.1], [13.1, -17.55]),
    ],
];

/// The leaderboard's middle on the back wall, in the hall's frame, and
/// its height over the hall's ground, m: where Verse draws the board.
pub const LEADERBOARD: ([f32; 2], f32) = ([0.0, -27.46], FLOOR + 3.7);

/// Where the bell hangs: its pivot under the yoke, in the hall's frame,
/// and its height over the hall's ground, m (`BELL_PIVOT` in the script).
pub const BELL_PIVOT: ([f32; 2], f32) = ([0.0, -24.0], FLOOR + 2.6);

/// A station in the world: where it stands, x and z, and its heading as
/// the controller's yaw.
#[must_use]
pub fn place((at, toward): Station) -> ([f32; 2], f32) {
    let [x, z] = AGORA.world(at);
    let [tx, tz] = AGORA.world(toward);
    ([x, z], (tx - x).atan2(tz - z))
}

/// The hall's lights, in its frame: x, height above its base, and z, m,
/// as `agora.footprint.json`'s `lights`.
pub const LIGHTS: [(Fixture, [f32; 3]); 18] = [
    (Fixture::Lantern, [-11.8, 4.52, -9.62]),
    (Fixture::Lantern, [11.8, 4.52, -9.62]),
    (Fixture::Lantern, [-5.4, 2.02, 0.6]),
    (Fixture::Uplight, [-2.0, 2.5, -6.95]),
    (Fixture::Lantern, [5.4, 2.02, 0.6]),
    (Fixture::Uplight, [2.0, 2.5, -6.95]),
    (Fixture::Sconce, [-7.62, 4.67, -27.34]),
    (Fixture::Sconce, [7.62, 4.67, -27.34]),
    (Fixture::Lamp, [-4.0, 5.9, -13.2]),
    (Fixture::Lamp, [4.0, 5.9, -13.2]),
    (Fixture::Lamp, [-4.0, 5.9, -16.8]),
    (Fixture::Lamp, [4.0, 5.9, -16.8]),
    (Fixture::Lamp, [-4.0, 5.9, -20.4]),
    (Fixture::Lamp, [4.0, 5.9, -20.4]),
    (Fixture::Candles, [-12.1, 3.155, -16.5]),
    (Fixture::Lamp, [-14.2, 3.17, -11.0]),
    (Fixture::Lamp, [14.3, 3.17, -11.0]),
    (Fixture::Sconce, [14.54, 4.27, -18.6]),
];

/// The flames, in the hall's frame, for their halos.
const FLAMES: [[f32; 3]; 6] = [
    [-7.62, 4.656, -27.34],
    [7.62, 4.656, -27.34],
    [-12.23, 3.141, -16.5],
    [-12.1, 3.201, -16.5],
    [-11.97, 3.141, -16.5],
    [14.54, 4.256, -18.6],
];

/// Where the hall's lights reach the player at all: its outside lights
/// within this distance of the hall, m...
const OUTDOOR_REACH: f32 = 70.0;
/// ...and the inside's within this.
const INDOOR_REACH: f32 = 34.0;

/// The hall's rooms, in its frame: x, z, and height above the base from,
/// to, m. The trading floor, Paul's office, and the training room; each
/// range starts below the floor, so a walker the podium hasn't lifted yet
/// counts as inside.
const ROOMS: [([f32; 2], [f32; 2], [f32; 2]); 3] = [
    ([-8.0, 8.0], [-27.6, -8.0], [-1.0, 8.6]),
    ([-14.8, -8.0], [-20.6, -10.4], [-1.0, 5.8]),
    ([8.0, 14.8], [-20.6, -10.4], [-1.0, 5.8]),
];

/// A point of the hall's frame, x, height, and z, in the world.
fn world(local: [f32; 3]) -> Vec3 {
    let [x, z] = AGORA.world([local[0], local[2]]);
    let [ax, az] = AGORA.at;
    Vec3::new(x, super::height(ax, az) + local[1], z)
}

/// A world point in the hall's frame, x, height, and z.
fn local(p: Vec3) -> [f32; 3] {
    let [ax, az] = AGORA.at;
    let q = Quat::from_rotation_y(-AGORA.yaw) * Vec3::new(p.x - ax, 0.0, p.z - az);
    [q.x, p.y - super::height(ax, az), q.z]
}

/// Whether `p` is inside the hall: on the trading floor, in Paul's
/// office, or in the training room.
#[must_use]
pub fn inside(p: Vec3) -> bool {
    let [x, y, z] = local(p);
    ROOMS.iter().any(|([x0, x1], [z0, z1], [y0, y1])| {
        (*x0..=*x1).contains(&x) && (*z0..=*z1).contains(&z) && (*y0..=*y1).contains(&y)
    })
}

/// The hall's lamps that reach a player at `at` at `time`, s: those on
/// the player's side of the walls first (the rooms' inside them, the
/// lanterns and uplights outside), then the nearest, at most
/// [`MAX_LAMPS`], the flames flickering. None when the player is far from
/// the hall.
#[must_use]
pub fn lamps(at: Vec3, time: f32) -> Vec<Lamp> {
    let center = world([0.0, 0.0, -16.0]);
    let away = Vec3::new(at.x - center.x, 0.0, at.z - center.z).length();
    let within = inside(at);
    let mut lit: Vec<((bool, f32), Lamp)> = LIGHTS
        .iter()
        .enumerate()
        .filter(|(_, (fixture, _))| {
            away < if fixture.indoors() {
                INDOOR_REACH
            } else {
                OUTDOOR_REACH
            }
        })
        .map(|(i, &(fixture, local))| {
            let (color, intensity, range) = fixture.light();
            let lamp = Lamp {
                position: world(local),
                color,
                intensity,
                range,
            };
            let lamp = if fixture.flame() {
                lamp.flickering(time, 120 + i as u32)
            } else {
                lamp
            };
            (
                (
                    fixture.indoors() != within,
                    lamp.position.distance_squared(at),
                ),
                lamp,
            )
        })
        .collect();
    lit.sort_by(|a, b| a.0.0.cmp(&b.0.0).then(a.0.1.total_cmp(&b.0.1)));
    lit.into_iter()
        .take(MAX_LAMPS)
        .map(|(_, lamp)| lamp)
        .collect()
}

/// Lights Everglade's stage `neon` for a player at `at`: the hall's lamps
/// in its free slots, and the great room's moodier grade while the player
/// is inside. A stage far from the hall is left as it was.
pub fn light(neon: &mut Neon, at: Vec3, time: f32) {
    let mut lamps = lamps(at, time).into_iter();
    for slot in neon.lamps.iter_mut().filter(|lamp| !lamp.lit()) {
        match lamps.next() {
            Some(lamp) => *slot = lamp,
            None => break,
        }
    }
    if inside(at) {
        neon.grade = ROOM_GRADE;
        neon.vignette = neon.vignette.max(0.32);
        neon.bloom = neon.bloom.max(0.07);
    }
}

/// Where the hall's flames burn, in the world, for their halos.
#[must_use]
pub fn flames() -> Vec<Vec3> {
    FLAMES.iter().copied().map(world).collect()
}

/// The floor's height in the world, m.
#[must_use]
pub fn floor() -> f32 {
    super::height(AGORA.at[0], AGORA.at[1]) + FLOOR
}

/// Where a player stands at the door, on the portico just outside the
/// open bronze doors, looking down the trading floor to the leaderboard,
/// and the heading, as the controller's yaw.
#[must_use]
pub fn at_the_door() -> ([f32; 2], f32) {
    place(([0.0, -6.8], [0.0, -27.6]))
}

/// Where a player stands on the forecourt at the foot of the stair,
/// looking up at the portico, and the heading.
#[must_use]
pub fn before_the_stair() -> ([f32; 2], f32) {
    place(([0.0, 8.0], MIDDLE))
}

/// How far the bell swings at first, radians.
const SWING: f32 = 0.62;
/// One full swing, s.
const PERIOD: f32 = 1.5;
/// How fast the swing dies away: its time constant, s.
const DAMPING: f32 = 3.0;
/// How long a ring lasts, s.
pub const RING: f32 = 9.0;
/// How far from the bell the zone draws it, m: it hangs indoors, so
/// beyond this it is out of sight.
const DRAW_REACH: f32 = 60.0;

/// The Agora's bell, which rings when the payment ledger records a
/// settled deal (`docs/sales/agent-sales-floor.md`, Making it lively,
/// honestly). Only a settled payment rings it: a verbal yes, a booked
/// meeting, or an invoice never does. Nothing calls [`Self::ring`] yet;
/// the ledger's wiring comes with the sales floor's records.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bell {
    /// How long ago it was rung, s, while it still swings.
    since: Option<f32>,
}

impl Bell {
    /// Rings the bell: it swings from rest, or swings anew if it already
    /// swings.
    pub fn ring(&mut self) {
        self.since = Some(0.0);
    }

    /// Advances its swing by `dt`, s.
    pub fn tick(&mut self, dt: f32) {
        self.since = self
            .since
            .map(|since| since + dt.max(0.0))
            .filter(|&since| since < RING);
    }

    /// Whether it is swinging.
    #[must_use]
    pub fn ringing(&self) -> bool {
        self.since.is_some()
    }

    /// Its swing from hanging straight down, radians, about the hall's x
    /// axis: toward the floor when positive.
    #[must_use]
    pub fn angle(&self) -> f32 {
        self.since.map_or(0.0, |t| {
            SWING * (-t / DAMPING).exp() * (TAU * t / PERIOD).sin()
        })
    }

    /// How many times its clapper has struck since it was rung: once at
    /// each end of every swing, for the sound to come.
    #[must_use]
    pub fn strikes(&self) -> u32 {
        self.since
            .map_or(0, |t| ((t / PERIOD - 0.25) * 2.0 + 1.0).max(0.0) as u32)
    }

    /// The bell as drawn from `eye`, hanging from its yoke at its swing:
    /// nothing when the eye is far from it.
    #[must_use]
    pub fn mesh(&self, eye: Vec3) -> Mesh {
        let mut mesh = Mesh::default();
        let (at, up) = BELL_PIVOT;
        let pivot = world([at[0], up, at[1]]);
        if pivot.distance(eye) > DRAW_REACH {
            return mesh;
        }
        let axis = Quat::from_rotation_y(AGORA.yaw) * Vec3::X;
        let turn = Quat::from_axis_angle(axis, self.angle());
        draw_bell(&mut mesh, pivot, turn);
        mesh
    }
}

/// Bronze and the copper lip, as the zone's shaded faces draw them.
const BRONZE: [f32; 3] = [0.3, 0.18, 0.09];
const COPPER: [f32; 3] = [0.62, 0.32, 0.14];
const SHADOW: [f32; 3] = [0.05, 0.03, 0.02];

/// The bell's profile from its crown down: radius and drop below the
/// pivot, m, as `bell_body` in the script draws the kit's bell.
const PROFILE: [(f32, f32); 6] = [
    (0.07, 0.0),
    (0.07, 0.1),
    (0.18, 0.1),
    (0.26, 0.34),
    (0.36, 0.62),
    (0.375, 0.66),
];

/// Appends the bell hanging from `pivot`, turned by `turn`: its crown,
/// shoulder, and flared mouth in bronze with a copper lip, dark inside,
/// and its clapper.
fn draw_bell(mesh: &mut Mesh, pivot: Vec3, turn: Quat) {
    const SIDES: usize = 10;
    let point = |r: f32, drop: f32, k: usize| {
        let a = TAU * k as f32 / SIDES as f32;
        pivot + turn * Vec3::new(r * a.cos(), -drop, r * a.sin())
    };
    for (i, pair) in PROFILE.windows(2).enumerate() {
        let [(r0, d0), (r1, d1)] = [pair[0], pair[1]];
        let color = if i == PROFILE.len() - 2 {
            COPPER
        } else {
            BRONZE
        };
        for k in 0..SIDES {
            let quad = [
                point(r0, d0, k),
                point(r0, d0, k + 1),
                point(r1, d1, k + 1),
                point(r1, d1, k),
            ];
            face(mesh, quad, color);
            // The inside of the mouth, dark, seen from under it.
            if i >= 3 {
                let inner = [
                    point(r0 * 0.92, d0, k),
                    point(r1 * 0.92, d1, k),
                    point(r1 * 0.92, d1, k + 1),
                    point(r0 * 0.92, d0, k + 1),
                ];
                face(mesh, inner, SHADOW);
            }
        }
    }
    // The crown's top.
    for k in 1..SIDES - 1 {
        face(
            mesh,
            [
                point(0.07, 0.0, 0),
                point(0.07, 0.0, k + 1),
                point(0.07, 0.0, k),
                point(0.07, 0.0, k),
            ],
            BRONZE,
        );
    }
    // The clapper, a short bar in the mouth.
    let c = |x: f32, drop: f32, z: f32| pivot + turn * Vec3::new(x, -drop, z);
    for (a, b) in [
        ([-0.04, -0.04], [0.04, -0.04]),
        ([0.04, -0.04], [0.04, 0.04]),
        ([0.04, 0.04], [-0.04, 0.04]),
        ([-0.04, 0.04], [-0.04, -0.04]),
    ] {
        face(
            mesh,
            [
                c(a[0], 0.3, a[1]),
                c(b[0], 0.3, b[1]),
                c(b[0], 0.6, b[1]),
                c(a[0], 0.6, a[1]),
            ],
            SHADOW,
        );
    }
}

/// Appends a shaded quad.
fn face(mesh: &mut Mesh, [a, b, c, d]: [Vec3; 4], color: [f32; 3]) {
    let color = super::super::draw::shade(color, a, b, c);
    for p in [a, b, c, a, c, d] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hall's lot in its frame: its blocks' extent, x and z, m.
    fn lot() -> ([f32; 2], [f32; 2]) {
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for &[x0, x1, z0, z1, _] in AGORA_HALL.blocks {
            min = [min[0].min(x0), min[1].min(z0)];
            max = [max[0].max(x1), max[1].max(z1)];
        }
        (min, max)
    }

    /// The lot's box in the world: min and max, x and z.
    fn world_lot() -> ([f32; 2], [f32; 2]) {
        let ([x0, z0], [x1, z1]) = lot();
        [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]
            .map(|c| AGORA.world(c))
            .iter()
            .fold(
                ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
                |(lo, hi), [x, z]| {
                    (
                        [lo[0].min(*x), lo[1].min(*z)],
                        [hi[0].max(*x), hi[1].max(*z)],
                    )
                },
            )
    }

    #[test]
    fn the_hall_stands_on_flat_open_ground_off_every_road_facing_the_plaza() {
        // It faces south, toward the Fountain Plaza.
        let out = AGORA.outward();
        assert!(out[1] < -0.99, "{out:?}");
        // Its walk reaches the forecourt, and the forecourt the stair.
        let ([sx, sz], [hx, hz]) = SQUARE;
        let front = AGORA.front();
        assert!((front[0] - sx).abs() < hx && (front[1] - sz).abs() <= hz + 0.01);
        let end = WALKS[1].1;
        assert!((end[0] - sx).abs() < hx && (end[1] - sz).abs() <= hz);
        // The walk starts on the plaza's paving.
        let ([px, pz], [phx, phz]) = super::super::PAVED_SQUARES[0];
        let start = WALKS[0].0;
        assert!((start[0] - px).abs() <= phx && (start[1] - pz).abs() <= phz);
        // The whole lot is inside the flat clearing.
        let (min, max) = world_lot();
        for [x, z] in [min, max, [min[0], max[1]], [max[0], min[1]]] {
            let rise = super::super::height(x, z);
            assert!(rise < 0.05, "{x}, {z}: {rise}");
        }
        // No road or walk runs under its blocks.
        for (f, _) in AGORA.blocks() {
            for &(a, b, half) in super::super::roads() {
                let corners = [f.min, f.max, [f.min[0], f.max[1]], [f.max[0], f.min[1]]];
                let center = [(f.min[0] + f.max[0]) / 2.0, (f.min[1] + f.max[1]) / 2.0];
                for [x, z] in corners.into_iter().chain([center]) {
                    let d = super::super::segment_distance(a, b, x, z);
                    assert!(d > half, "{f:?} on the road {a:?}..{b:?}");
                }
            }
        }
        // At least 3 m of open ground round its lot from every other
        // reserved footprint: the market hall, the chapel, and the rows.
        for (c, h) in super::super::city::reserved() {
            if c == CLEAR.0 {
                continue;
            }
            let gap_x = (c[0] - (min[0] + max[0]) / 2.0).abs() - h[0] - (max[0] - min[0]) / 2.0;
            let gap_z = (c[1] - (min[1] + max[1]) / 2.0).abs() - h[1] - (max[1] - min[1]) / 2.0;
            let own = (c[0] - AGORA.at[0]).abs() < 1.0;
            assert!(own || gap_x.max(gap_z) >= 3.0, "{c:?} {h:?}");
        }
    }

    #[test]
    fn the_entry_axis_stays_clear_from_the_forecourt_to_the_middle() {
        // From the stair's foot, between the inner columns, through the
        // open doors, and down the center aisle between the desk banks to
        // the floor's middle, a walk 2.5 m wide crosses no block.
        let half = 1.25;
        let (foot, middle) = (AGORA_HALL.front[1], MIDDLE[1]);
        for &[x0, x1, z0, z1, _] in AGORA_HALL.blocks {
            let across = x1 > -half && x0 < half;
            let along = z1 > middle && z0 < foot;
            assert!(
                !(across && along),
                "{:?} blocks the entry",
                [x0, x1, z0, z1]
            );
        }
    }

    /// Which points of the hall's frame a walker reaches from `from`, on
    /// a 0.1 m grid over the lot: a walker's body, `RADIUS` round its
    /// middle, clears every block, as walking itself tests.
    fn reach(from: [f32; 2]) -> impl Fn([f32; 2]) -> bool {
        use verse_world::social::controller::RADIUS;
        const STEP: f32 = 0.1;
        let ([x0, z0], [x1, z1]) = lot();
        let w = ((x1 - x0) / STEP) as usize + 1;
        let h = ((z1 - z0) / STEP) as usize + 1;
        let at = move |k: usize| [x0 + (k % w) as f32 * STEP, z0 + (k / w) as f32 * STEP];
        let open = |[x, z]: [f32; 2]| {
            AGORA_HALL.blocks.iter().all(|&[bx0, bx1, bz0, bz1, _]| {
                x <= bx0 - RADIUS || x >= bx1 + RADIUS || z <= bz0 - RADIUS || z >= bz1 + RADIUS
            })
        };
        let cell = move |[x, z]: [f32; 2]| {
            let (i, j) = (((x - x0) / STEP).round(), ((z - z0) / STEP).round());
            let inside = i >= 0.0 && j >= 0.0 && (i as usize) < w && (j as usize) < h;
            inside.then_some(j as usize * w + i as usize)
        };
        let start = cell(from).expect("the start is on the lot");
        assert!(open(at(start)), "the start is blocked");
        let mut seen = vec![false; w * h];
        seen[start] = true;
        let mut queue = vec![start];
        while let Some(k) = queue.pop() {
            let (i, j) = (k % w, k / w);
            let next = [
                (i > 0).then(|| k - 1),
                (i + 1 < w).then_some(k + 1),
                (j > 0).then(|| k - w),
                (j + 1 < h).then_some(k + w),
            ];
            for n in next.into_iter().flatten() {
                if !seen[n] && open(at(n)) {
                    seen[n] = true;
                    queue.push(n);
                }
            }
        }
        move |p| cell(p).is_some_and(|k| seen[k])
    }

    #[test]
    fn every_station_is_reachable_from_the_door() {
        // From the forecourt, up the stair, and in at the door.
        let reached = reach([0.0, 0.5]);
        let mut stations: Vec<Station> = DESKS.to_vec();
        stations.extend([PAUL, STANDUP, OWNER, TEACHER]);
        stations.extend(BOOTHS.iter().flatten());
        for (at, toward) in stations {
            // Inside one of the rooms, facing somewhere else.
            let [x, z] = at;
            assert!(inside(world([x, FLOOR + 0.1, z])), "{at:?} is outside");
            assert!((toward[0] - x).hypot(toward[1] - z) > 1.0);
            assert!(reached(at), "{at:?} is out of reach of the door");
        }
        // Each desk faces the leaderboard, and each booth's pair faces
        // each other.
        for (at, toward) in DESKS {
            assert!(toward[1] < at[1]);
        }
        for [(a, ta), (b, tb)] in BOOTHS {
            assert_eq!((ta, tb), (b, a));
        }
    }

    #[test]
    fn the_hall_lights_only_near_it_and_inside_within_the_lamp_limit() {
        // Far off in the town, nothing.
        assert!(lamps(Vec3::new(0.0, 0.0, -20.0), 0.0).is_empty());
        // Out on the walk from the plaza, the lanterns and uplights, not
        // the rooms.
        let street = world([0.0, 0.0, 20.0]);
        let outside = lamps(street, 0.0);
        let out = LIGHTS.iter().filter(|(f, _)| !f.indoors()).count();
        assert_eq!(outside.len(), out);
        // On the floor, every fixture, the rooms' own first.
        let floor = world([MIDDLE[0], FLOOR + 0.2, MIDDLE[1]]);
        assert!(inside(floor) && !inside(street));
        let all = lamps(floor, 3.0);
        assert_eq!(all.len(), LIGHTS.len());
        assert!(all.len() <= MAX_LAMPS);
        let rooms = LIGHTS.len() - out;
        for lamp in &all[..rooms] {
            assert!(inside(lamp.position), "{:?}", lamp.position);
        }
        assert!(all.iter().all(Lamp::lit));
        // The pendants over the desks come first, so the low tier's eight
        // light the floor.
        for lamp in &all[..6] {
            let [x, y, z] = local(lamp.position);
            assert!(x.abs() < 8.0 && z < -8.0 && y > 5.0, "{x}, {y}, {z}");
        }
        // Every desk row and room has a light within its reach.
        let mut spots: Vec<[f32; 2]> = DESKS.iter().map(|s| s.0).collect();
        spots.extend([PAUL.0, OWNER.0, TEACHER.0, BOOTHS[3][1].0]);
        for [x, z] in spots {
            let lit = LIGHTS.iter().any(|(f, at)| {
                let (_, _, range) = f.light();
                f.indoors() && (at[0] - x).hypot(at[2] - z) < range
            });
            assert!(lit, "{x}, {z} is dark");
        }
        // The grade changes only inside.
        let mut neon = Neon::plaza(0.0);
        let before = neon.grade;
        light(&mut neon, street, 0.0);
        assert_eq!(neon.grade, before);
        light(&mut neon, floor, 0.0);
        assert_eq!(neon.grade, ROOM_GRADE);
        // Its outside lights hang on the hall or stand on its lot.
        let ([x0, z0], [x1, z1]) = lot();
        for (fixture, [x, _, z]) in LIGHTS {
            if !fixture.indoors() {
                let on = (x0 - 0.5..=x1 + 0.5).contains(&x) && (z0..=z1 + 0.5).contains(&z);
                assert!(on, "{fixture:?} at {x}, {z}");
            }
        }
    }

    #[test]
    fn the_hall_the_house_and_the_civic_hall_never_light_past_the_limit() {
        // Each fills only the stage's free slots, wherever the player is.
        for at in [
            world([0.0, FLOOR + 0.2, -18.0]),
            Vec3::new(0.0, 0.0, 80.0),
            Vec3::new(100.0, 0.0, 10.0),
        ] {
            let mut neon = Neon::plaza(0.0);
            super::super::estate::light(&mut neon, at, 0.0);
            super::super::civic::light(&mut neon, at, 0.0);
            light(&mut neon, at, 0.0);
            let lit = neon.lamps.iter().filter(|lamp| lamp.lit()).count();
            assert!(lit <= MAX_LAMPS);
        }
    }

    #[test]
    fn nothing_else_stands_on_the_lot_and_no_tree_reaches_its_walls() {
        let (min, max) = world_lot();
        for p in super::super::placements() {
            let [x, z] = p.at;
            let on = x > min[0] && x < max[0] && z > min[1] && z < max[1];
            assert!(!on || p.model == AGORA_HALL.name, "{} at {x}, {z}", p.model);
        }
        // The clear ground covers the lot with room to spare...
        let ([cx, cz], [hx, hz]) = CLEAR;
        assert!(cx - hx <= min[0] - 4.0 && cx + hx >= max[0] + 2.5);
        assert!(cz + hz >= max[1] + 4.0 && cz - hz <= min[1]);
        // ...and no tree grows within 4.5 m of its sides and back.
        let trees = [
            "oak", "beech", "linden", "fir", "spruce", "pine", "birch", "poplar", "tree", "copse",
        ];
        let reach = 4.5;
        for p in super::super::placements() {
            let [x, z] = p.at;
            let near =
                x > min[0] - reach && x < max[0] + reach && z > min[1] + 4.0 && z < max[1] + reach;
            let tree = trees.iter().any(|t| p.model.contains(t));
            assert!(!(near && tree), "{} at {x}, {z}", p.model);
        }
    }

    #[test]
    fn the_bell_swings_when_rung_and_settles() {
        let mut bell = Bell::default();
        assert!(!bell.ringing());
        assert_eq!(bell.angle(), 0.0);
        assert_eq!(bell.strikes(), 0);
        // At rest it hangs under the yoke, between its posts.
        let eye = world([0.0, FLOOR + 1.7, -18.0]);
        let still = bell.mesh(eye);
        assert!(!still.faces.is_empty());
        let (at, up) = BELL_PIVOT;
        let pivot = world([at[0], up, at[1]]);
        for v in &still.faces {
            let p = Vec3::from(v.pos);
            assert!(p.y <= pivot.y + 0.01 && p.y > pivot.y - 0.7, "{p}");
            assert!((p - pivot).length() < 0.8);
        }
        // Rung, it swings both ways and strikes as it goes.
        bell.ring();
        let (mut most, mut least) = (0.0_f32, 0.0_f32);
        for step in 0..60 {
            bell.tick(0.05);
            most = most.max(bell.angle());
            least = least.min(bell.angle());
            if step == 7 {
                // Near the end of its first swing, its mouth has moved.
                let swung = bell.mesh(eye);
                let moved = swung.faces.iter().zip(&still.faces);
                assert!(
                    moved
                        .map(|(a, b)| Vec3::from(a.pos).distance(Vec3::from(b.pos)))
                        .fold(0.0, f32::max)
                        > 0.2
                );
            }
        }
        assert!(bell.ringing());
        assert!(most > 0.3 && least < -0.3, "{most} {least}");
        assert!(bell.strikes() >= 3, "{}", bell.strikes());
        // Then it settles and hangs still again.
        for _ in 0..200 {
            bell.tick(0.05);
        }
        assert!(!bell.ringing());
        assert_eq!(bell.angle(), 0.0);
        // Far off, it isn't drawn.
        assert!(bell.mesh(Vec3::new(0.0, 2.0, -20.0)).faces.is_empty());
    }
}
