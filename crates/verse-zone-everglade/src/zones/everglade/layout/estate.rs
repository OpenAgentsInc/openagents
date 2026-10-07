//! The owner's house: the first Greco-futurism building
//! (`docs/verse/greco-futurism.md`), a two-storey estate on a podium at the
//! east end of Library Way, on open ground near the clearing's edge with
//! the east woods behind it and Observatory Hill to the south.
//!
//! `scripts/blender/greco_futurism.py` builds it as one model,
//! `generated/greco_house`, with a far level of detail. It stands on open
//! ground like the boathouse ([`super::city::GROUNDS`]): its walls,
//! columns, planter walls, and furniture block walking by the boxes of its
//! `greco_house.footprint.json`, its flat roof is a surface to land on, and
//! its bronze doors stand open, so a walk leads from Library Way up the
//! stair, between the round columns, and into the great room. Like every
//! generated model, it collides by its own triangles and breaks
//! (`demolition::carve`). It doesn't smoke: its chimneys are plain blocks
//! on a quiet house.
//!
//! The house is lit like the crypt: candles, sconces, a brazier, and lamps
//! warm the great room in pools, lanterns and uplights light the portico
//! and the stair, and its flames and inlays glow (`Emit...` materials).
//! [`light`] gives Everglade's stage the fixtures' point lights near the
//! player, nearest first, so a low tier's first eight are the ones that
//! matter, and a moodier grade while the player is in the great room;
//! everywhere else Everglade's look is unchanged. Candle halos run in the
//! town's particles ([`flames`]).

use super::generated::{GableRoof, Instance, Model};
use crate::pbr::{Grade, Lamp, MAX_LAMPS, Neon};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

/// The owner's house, in its glTF frame: +z out of the front, the origin
/// on the ground at the center of the lowest step's front edge.
pub const GRECO_HOUSE: Model = Model {
    name: "generated/greco_house",
    blocks: &[
        // The hedged planter walls beside the lower and upper flights.
        [-10.4, -4.5, -1.68, 0.0, 1.2],
        [4.5, 10.4, -1.68, 0.0, 1.2],
        [-7.2, -4.4, -7.88, -5.6, 2.7],
        [4.4, 7.2, -7.88, -5.6, 2.7],
        // The portico's square piers and round columns.
        [-9.95, -8.85, -9.15, -8.05, 9.8],
        [-7.15, -6.05, -9.15, -8.05, 9.8],
        [6.05, 7.15, -9.15, -8.05, 9.8],
        [8.85, 9.95, -9.15, -8.05, 9.8],
        [-2.67, -1.53, -9.17, -8.03, 9.8],
        [1.53, 2.67, -9.17, -8.03, 9.8],
        // The side walls, the facade on each side of the door, and the
        // back wall.
        [-10.05, -9.55, -25.6, -11.6, 9.8],
        [9.55, 10.05, -25.6, -11.6, 9.8],
        [-10.0, -1.35, -12.0, -11.6, 9.8],
        [1.35, 10.0, -12.0, -11.6, 9.8],
        [-10.0, 10.0, -25.6, -25.2, 9.8],
        // The lantern posts beside the lower and upper flights.
        [-5.12, -4.88, 0.48, 0.72, 2.3],
        [-4.97, -4.73, -5.32, -5.08, 2.7],
        [4.88, 5.12, 0.48, 0.72, 2.3],
        [4.73, 4.97, -5.32, -5.08, 2.7],
        // The great room's desk, the brazier, and the sofa against its
        // west wall.
        [-1.4, 1.4, -22.1, -21.1, 2.38],
        [6.95, 7.85, -19.05, -18.15, 2.6],
        // The workshop agent's workstation at the spot kept for it, the
        // console she works at by the east wall, and the lectern where she
        // waits for an approval.
        [-6.1, -3.1, -23.1, -22.1, 2.62],
        [8.85, 9.55, -22.3, -20.9, 2.6],
        [4.75, 5.25, -15.25, -14.75, 2.66],
        // The reception's chair and its desk, turned toward the door.
        [-3.56, -2.84, -16.56, -15.84, 2.52],
        [-3.74, -1.82, -16.6, -14.8, 2.52],
        [-9.25, -8.25, -20.8, -16.4, 2.44],
    ],
    roofs: &[GableRoof {
        center: [0.0, -17.1],
        slopes_z: true,
        half: [8.5, 9.5],
        eave: 12.4,
        ridge: 12.41,
    }],
    front: [0.0, 1.0],
    inside: Some([0.0, -18.0]),
};

/// Where the house stands: its stair's foot at Library Way's east end,
/// facing west down the street.
pub const OWNERS_HOUSE: Instance =
    Instance::new("owner's house", &GRECO_HOUSE, [109.0, -29.0], -FRAC_PI_2);

/// The walk from Library Way's end to the foot of the stair, as a road:
/// from, to, and half width, m.
pub const WALK: ([f32; 2], [f32; 2], f32) = ([104.0, -29.0], [108.0, -29.0], 1.4);

/// A spot kept clear for a workstation in the great room, west of the desk
/// in front of the engraved-door wall, facing into the room (+z), in the
/// house's frame, x and z, m. The left sconce and the tall candle stand
/// light it.
pub const WORKSTATION: [f32; 2] = [-4.6, -22.6];

/// The reception chair in the great room, west of the entry walk a few
/// strides in from the door, in the house's frame, x and z, m, and the
/// point it faces: the doorway's middle, so whoever sits there greets
/// arrivals. Its desk stands before it. The owner's private placements
/// may seat a character here (`guests::seat`).
pub const RECEPTION: ([f32; 2], [f32; 2]) = ([-3.2, -16.2], [0.0, -12.4]);

/// The reception chair's seat: where a seated character's feet rest, x,
/// height, and z in the world, and its heading as the controller's yaw.
/// The pose sits the body on the seat; the feet are on the floor.
#[must_use]
pub fn reception() -> (Vec3, f32) {
    let (at, toward) = RECEPTION;
    let [x, z] = OWNERS_HOUSE.world(at);
    let [tx, tz] = OWNERS_HOUSE.world(toward);
    (
        Vec3::new(
            x,
            super::height(OWNERS_HOUSE.at[0], OWNERS_HOUSE.at[1]) + PODIUM,
            z,
        ),
        (tx - x).atan2(tz - z),
    )
}

/// The podium's top over the house's ground, m: the great room's marble
/// floor, where nothing lies on it.
pub const PODIUM: f32 = 1.6;

/// What gives a light in the house.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixture {
    /// A candelabrum's or a side table's candles.
    Candles,
    /// A bronze wall sconce's candle.
    Sconce,
    /// A floor lamp's amber shade.
    Lamp,
    /// The brazier's fire.
    Brazier,
    /// A lantern on the portico or beside the stair.
    Lantern,
    /// An uplight washing the facade.
    Uplight,
}

impl Fixture {
    /// Its linear color, candela, and range, m.
    #[must_use]
    pub const fn light(self) -> ([f32; 3], f32, f32) {
        match self {
            Self::Candles => ([1.0, 0.66, 0.34], 6_500.0, 5.0),
            Self::Sconce => ([1.0, 0.62, 0.3], 7_000.0, 6.0),
            Self::Lamp => ([1.0, 0.7, 0.42], 6_000.0, 6.0),
            Self::Brazier => ([1.0, 0.5, 0.2], 22_000.0, 9.0),
            Self::Lantern => ([1.0, 0.7, 0.4], 5_000.0, 8.0),
            Self::Uplight => ([1.0, 0.76, 0.5], 8_000.0, 7.0),
        }
    }

    /// Whether it lights the great room rather than the outside.
    #[must_use]
    pub const fn indoors(self) -> bool {
        !matches!(self, Self::Lantern | Self::Uplight)
    }

    /// Whether it burns, and so flickers.
    #[must_use]
    pub const fn flame(self) -> bool {
        !matches!(self, Self::Lamp | Self::Uplight)
    }
}

/// The house's lights, in its frame: x, height above its base, and z, m,
/// as `greco_house.footprint.json`'s `lights`.
pub const LIGHTS: [(Fixture, [f32; 3]); 20] = [
    (Fixture::Lantern, [-6.6, 4.92, -7.77]),
    (Fixture::Uplight, [-1.9, 2.5, -10.95]),
    (Fixture::Lantern, [-5.0, 2.02, 0.6]),
    (Fixture::Lantern, [-4.85, 2.42, -5.2]),
    (Fixture::Lantern, [6.6, 4.92, -7.77]),
    (Fixture::Uplight, [1.9, 2.5, -10.95]),
    (Fixture::Lantern, [5.0, 2.02, 0.6]),
    (Fixture::Lantern, [4.85, 2.42, -5.2]),
    (Fixture::Lamp, [4.6, 3.15, -23.6]),
    (Fixture::Candles, [0.55, 2.955, -21.85]),
    (Fixture::Sconce, [-1.85, 4.17, -24.88]),
    (Fixture::Sconce, [1.85, 4.17, -24.88]),
    (Fixture::Candles, [-6.3, 3.105, -24.3]),
    (Fixture::Brazier, [7.4, 2.95, -18.6]),
    (Fixture::Sconce, [1.95, 4.17, -12.26]),
    (Fixture::Sconce, [-1.95, 4.17, -12.26]),
    (Fixture::Candles, [-8.4, 3.105, -12.9]),
    (Fixture::Candles, [7.5, 3.105, -12.9]),
    (Fixture::Candles, [-8.7, 2.47, -15.7]),
    (Fixture::Lamp, [-8.8, 3.15, -21.7]),
];

/// The flames, in the house's frame, for their halos.
const FLAMES: [[f32; 3]; 19] = [
    [0.38, 2.941, -21.85],
    [0.55, 3.001, -21.85],
    [0.72, 2.941, -21.85],
    [-1.85, 4.156, -24.88],
    [1.85, 4.156, -24.88],
    [-6.5, 3.091, -24.3],
    [-6.3, 3.151, -24.3],
    [-6.1, 3.091, -24.3],
    [7.4, 2.68, -18.6],
    [1.95, 4.156, -12.26],
    [-1.95, 4.156, -12.26],
    [-8.6, 3.091, -12.9],
    [-8.4, 3.151, -12.9],
    [-8.2, 3.091, -12.9],
    [7.3, 3.091, -12.9],
    [7.5, 3.151, -12.9],
    [7.7, 3.091, -12.9],
    [-8.78, 2.446, -15.7],
    [-8.61, 2.376, -15.75],
];

/// Where the house's lights reach the player at all: its outside lights
/// within this distance of the house, m...
const OUTDOOR_REACH: f32 = 70.0;
/// ...and the great room's within this.
const INDOOR_REACH: f32 = 34.0;

/// The great room, in the house's frame: x, z, and height above the base
/// from, to, m. Its floor is the podium's top, 1.6 m up; the range starts
/// lower, so a walker the podium hasn't lifted yet counts as inside.
pub const ROOM: ([f32; 2], [f32; 2], [f32; 2]) = ([-9.6, 9.6], [-25.2, -12.0], [-1.0, 6.2]);

/// The great room's grade: a little darker, warmer, and more contrasty
/// than the afternoon outside, with cool shadows against warm highlights,
/// so the candles and lamps read as pools of light.
pub(super) const ROOM_GRADE: Grade = Grade {
    exposure: -1.0,
    balance: Vec3::new(1.04, 0.99, 0.92),
    saturation: 0.92,
    contrast: 1.2,
    shadows: Vec3::new(0.95, 0.97, 1.05),
    highlights: Vec3::new(1.05, 1.0, 0.93),
    ..Grade::STAGE
};

/// A point of the house's frame, x, height, and z, in the world.
fn world(local: [f32; 3]) -> Vec3 {
    let [x, z] = OWNERS_HOUSE.world([local[0], local[2]]);
    let [ax, az] = OWNERS_HOUSE.at;
    Vec3::new(x, super::height(ax, az) + local[1], z)
}

/// A world point in the house's frame, x, height, and z.
fn local(p: Vec3) -> [f32; 3] {
    let [ax, az] = OWNERS_HOUSE.at;
    let q = Quat::from_rotation_y(-OWNERS_HOUSE.yaw) * Vec3::new(p.x - ax, 0.0, p.z - az);
    [q.x, p.y - super::height(ax, az), q.z]
}

/// Whether `p` is inside the great room.
#[must_use]
pub fn in_room(p: Vec3) -> bool {
    let [x, y, z] = local(p);
    let ([x0, x1], [z0, z1], [y0, y1]) = ROOM;
    (x0..=x1).contains(&x) && (z0..=z1).contains(&z) && (y0..=y1).contains(&y)
}

/// The house's lamps that reach a player at `at` at `time`, s: those on
/// the player's side of the walls first (the great room's inside it, the
/// lanterns and uplights outside), then the nearest, at most
/// [`MAX_LAMPS`], the flames flickering. None when the player is far from
/// the house.
#[must_use]
pub fn lamps(at: Vec3, time: f32) -> Vec<Lamp> {
    let center = world([0.0, 0.0, -14.0]);
    let away = Vec3::new(at.x - center.x, 0.0, at.z - center.z).length();
    let inside = in_room(at);
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
                lamp.flickering(time, 40 + i as u32)
            } else {
                lamp
            };
            (
                (
                    fixture.indoors() != inside,
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

/// Lights Everglade's stage `neon` for a player at `at`: the house's lamps
/// in its free slots, and the great room's grade while the player is in
/// it. A stage far from the house is left as it was.
pub fn light(neon: &mut Neon, at: Vec3, time: f32) {
    let free = neon.lamps.iter().filter(|lamp| !lamp.lit()).count();
    let mut lamps = lamps(at, time).into_iter().take(free);
    for slot in neon.lamps.iter_mut().filter(|lamp| !lamp.lit()) {
        match lamps.next() {
            Some(lamp) => *slot = lamp,
            None => break,
        }
    }
    if in_room(at) {
        neon.grade = ROOM_GRADE;
        neon.vignette = neon.vignette.max(0.32);
        neon.bloom = neon.bloom.max(0.07);
    }
}

/// Where the house's flames burn, in the world, for their halos.
#[must_use]
pub fn flames() -> Vec<Vec3> {
    FLAMES.iter().copied().map(world).collect()
}

/// The great room's floor over the house's ground, m: the podium's top.
pub const FLOOR: f32 = 1.62;

/// The workshop agent's spots in the great room, in the house's frame:
/// where she stands, and the point she faces there. She stands at her
/// standing desk, the workstation ([`WORKSTATION`]), facing the room, works
/// at the console by the east wall while a command runs, and waits at the
/// lectern for an approval, all off the entry walkway. She never sits.
/// At the desk she stands the walker's clearance behind it; her typing
/// posture steps her up to it (`pose::DESK_STEP`).
pub const ALICE_DESK: ([f32; 2], [f32; 2]) = ([-4.6, -23.65], [-4.6, -20.0]);
/// Her place at the console by the east wall: the workbench.
pub const ALICE_WORKBENCH: ([f32; 2], [f32; 2]) = ([8.1, -21.6], [9.2, -21.6]);
/// Her place behind the lectern: the podium.
pub const ALICE_PODIUM: ([f32; 2], [f32; 2]) = ([5.0, -15.95], [5.0, -13.0]);
/// The middle of her screens, in the house's frame, and its height over
/// the floor, m: where she looks while she types, a little under a
/// standing figure's eyes.
pub const ALICE_SCREENS: ([f32; 2], f32) = ([-4.6, -22.4], 1.36);
/// The square she walks in, in the house's frame: its center and half
/// side, m. It holds the great room.
pub const ALICE_ROOM: ([f32; 2], f32) = ([0.0, -18.4], 9.5);

/// Which of her spots a station puts her at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliceSpot {
    Desk,
    Workbench,
    Podium,
}

impl AliceSpot {
    /// The spot for a studio station: commands and tests at the console,
    /// approvals at the lectern, and everything else at her workstation.
    #[must_use]
    pub fn of(station: coder_access::studio::Station) -> Self {
        use coder_access::studio::Station;
        match station {
            Station::Workbench | Station::ProvingGround => Self::Workbench,
            Station::Podium => Self::Podium,
            _ => Self::Desk,
        }
    }

    /// Where she stands and the point she faces, in the house's frame.
    #[must_use]
    pub fn local(self) -> ([f32; 2], [f32; 2]) {
        match self {
            Self::Desk => ALICE_DESK,
            Self::Workbench => ALICE_WORKBENCH,
            Self::Podium => ALICE_PODIUM,
        }
    }

    /// Where she stands, x and z, and her heading as the controller's yaw.
    #[must_use]
    pub fn world(self) -> ([f32; 2], f32) {
        let (at, toward) = self.local();
        let [x, z] = OWNERS_HOUSE.world(at);
        let [tx, tz] = OWNERS_HOUSE.world(toward);
        ([x, z], (tx - x).atan2(tz - z))
    }
}

/// The great room's floor height in the world, m.
#[must_use]
pub fn floor() -> f32 {
    super::super::height(OWNERS_HOUSE.at[0], OWNERS_HOUSE.at[1]) + FLOOR
}

/// Her screens' middle in the world, for her look while she types.
#[must_use]
pub fn alice_screens() -> glam::Vec3 {
    let ([x, z], up) = ALICE_SCREENS;
    let [x, z] = OWNERS_HOUSE.world([x, z]);
    glam::Vec3::new(x, floor() + up, z)
}

/// The square she walks in: the great room, on its floor.
#[must_use]
pub fn alice_area() -> verse_world::social::seats::Area {
    let (center, half) = ALICE_ROOM;
    verse_world::social::seats::Area {
        center: OWNERS_HOUSE.world(center),
        half,
        floor: Some(floor()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_house_stands_on_flat_ground_off_every_road_with_its_door_open() {
        let front = OWNERS_HOUSE.front();
        // The walk ends at the front step, and the house faces down it.
        assert!((front[0] - WALK.1[0]).abs() < 0.01 && (front[1] - WALK.1[1]).abs() < 0.01);
        let out = OWNERS_HOUSE.outward();
        assert!(out[0] < -0.99, "{out:?}");
        // Its front is inside the flat clearing, and its back stands where
        // the ground has risen less than the podium.
        let at = OWNERS_HOUSE.at;
        assert!(at[0].hypot(at[1]) < verse_world::social::everglade::CLEARING_RADIUS);
        for corner in [[-10.9, -26.1], [10.9, -26.1]] {
            let [x, z] = OWNERS_HOUSE.world(corner);
            assert!(super::super::height(x, z) < 1.0, "{x}, {z}");
        }
        // The doorway leads well inside, past the portico and the facade.
        let inside = OWNERS_HOUSE.world(GRECO_HOUSE.inside.unwrap());
        assert!(inside[0] > front[0] + 12.0);
    }

    #[test]
    fn the_entry_axis_stays_clear_of_furniture() {
        // From the door through the great room to its middle, a walk at
        // least 2.5 m wide crosses no block: no sofa or table in the way.
        let half = 1.25;
        let (door, middle) = (-12.0, -18.0);
        for &[x0, x1, z0, z1, _] in GRECO_HOUSE.blocks {
            let across = x1 > -half && x0 < half;
            let along = z1 > middle && z0 < door;
            assert!(
                !(across && along),
                "{:?} blocks the entry",
                [x0, x1, z0, z1]
            );
        }
    }

    #[test]
    fn the_reception_greets_arrivals_from_off_the_entry_walk() {
        let ([x, z], [tx, tz]) = RECEPTION;
        // Inside the great room, near the door, and west of the walk: the
        // chair and the desk keep a 2.5 m walk clear from the door.
        assert!(x.abs() < 9.55 - 0.4 && z < -12.0 - 0.4 && z > -18.0);
        let furniture: Vec<_> = GRECO_HOUSE
            .blocks
            .iter()
            .filter(|&&[x0, x1, z0, z1, _]| x0 <= x && x <= x1 + 1.5 && z0 <= z + 0.5 && z <= z1)
            .collect();
        assert_eq!(furniture.len(), 2, "the chair and the desk: {furniture:?}");
        for &&[_, x1, _, _, top] in &furniture {
            assert!(x1 < -1.25, "the reception crowds the entry walk");
            assert!(top < FLOOR + 1.0, "the room sees her over her desk");
        }
        // She faces the doorway: the door lies ahead of her, a little to
        // her left, never behind.
        let (feet, yaw) = reception();
        let [dx, dz] = OWNERS_HOUSE.world([tx, tz]);
        let ahead = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let to_door = Vec3::new(dx - feet.x, 0.0, dz - feet.z).normalize();
        assert!(ahead.dot(to_door) > 0.99);
        assert!((feet.y - (floor() - FLOOR + PODIUM)).abs() < 1e-4);
        assert!(in_room(feet + Vec3::Y * 0.5));
    }

    #[test]
    fn the_house_lights_only_near_it_nearest_first_within_the_lamp_limit() {
        // Far off in the town, nothing.
        assert!(lamps(Vec3::new(0.0, 0.0, -20.0), 0.0).is_empty());
        // On the street before it, the lanterns and uplights, not the room.
        let street = world([0.0, 0.0, 30.0]);
        let outside = lamps(street, 0.0);
        assert_eq!(outside.len(), 8, "{}", outside.len());
        // In the great room, every fixture, the room's own first.
        let inside = world([0.0, 1.8, -18.0]);
        assert!(in_room(inside) && !in_room(street));
        let all = lamps(inside, 3.0);
        assert_eq!(all.len(), LIGHTS.len());
        assert!(all.len() <= MAX_LAMPS);
        let room = LIGHTS.iter().filter(|(f, _)| f.indoors()).count();
        for lamp in &all[..room] {
            assert!(in_room(lamp.position), "{:?}", lamp.position);
        }
        assert!(all.iter().all(Lamp::lit));
        // The grade changes only in the room.
        let mut neon = Neon::plaza(0.0);
        let before = neon.grade;
        light(&mut neon, street, 0.0);
        assert_eq!(neon.grade, before);
        light(&mut neon, inside, 0.0);
        assert_eq!(neon.grade, ROOM_GRADE);
    }

    #[test]
    fn her_workstation_is_a_standing_desk() {
        let [x0, x1, z0, z1, top] = GRECO_HOUSE
            .blocks
            .iter()
            .copied()
            .find(|b| b[..4] == [-6.1, -3.1, -23.1, -22.1])
            .expect("the workstation's block");
        // She never sits: the desk stands at a standing worker's height.
        let height = top - FLOOR;
        assert!((0.95..=1.12).contains(&height), "desk {height} m high");
        assert!(x1 - x0 > 2.5 && z1 - z0 < 1.2);
    }

    #[test]
    fn the_workstation_spot_holds_only_the_workstation_and_is_lit() {
        let [wx, wz] = WORKSTATION;
        for &[x0, x1, z0, z1, _] in GRECO_HOUSE.blocks {
            let near = wx > x0 - 1.0 && wx < x1 + 1.0 && wz > z0 - 1.0 && wz < z1 + 1.0;
            let workstation = [x0, x1, z0, z1] == [-6.1, -3.1, -23.1, -22.1];
            assert!(
                !near || workstation,
                "{:?} crowds the workstation",
                [x0, x1, z0, z1]
            );
        }
        let lit = LIGHTS.iter().filter(|(f, at)| {
            let (_, _, range) = f.light();
            f.indoors() && (at[0] - wx).hypot(at[2] - wz) < range * 0.7
        });
        assert!(lit.count() >= 2);
    }

    #[test]
    fn nothing_else_stands_on_the_lot() {
        // Every other placement keeps off the house's footprint, so none
        // pokes through its floors or closes its doorway.
        let ([cx, cz], [hx, hz]) = (
            [OWNERS_HOUSE.at[0] + 13.05, OWNERS_HOUSE.at[1]],
            [13.05, 10.9],
        );
        for p in super::super::placements() {
            let [x, z] = p.at;
            let on = (x - cx).abs() < hx && (z - cz).abs() < hz;
            assert!(
                !on || p.model == GRECO_HOUSE.name,
                "{} at {x}, {z}",
                p.model
            );
        }
    }
    /// The house's own blocks, in its frame, around the great room's
    /// center, as the walker routes her.
    fn room_blocks() -> Vec<crate::controller::Footprint> {
        let ([cx, cz], _) = ALICE_ROOM;
        GRECO_HOUSE
            .blocks
            .iter()
            .map(|&[x0, x1, z0, z1, _]| crate::controller::Footprint {
                min: [x0 - cx, z0 - cz],
                max: [x1 - cx, z1 - cz],
            })
            .collect()
    }

    #[test]
    fn alice_works_inside_the_great_room_reachable_from_the_door() {
        use verse_world::social::nav;
        let ([cx, cz], half) = ALICE_ROOM;
        let blocks = room_blocks();
        // Just inside the front door, past the facade.
        let door = [0.0 - cx, -12.4 - cz];
        for spot in [AliceSpot::Desk, AliceSpot::Workbench, AliceSpot::Podium] {
            let ([x, z], toward) = spot.local();
            // Inside the walls, off the entry walkway, and facing into the
            // house's interior.
            assert!(
                x.abs() < 9.55 - 0.4 && z < -12.0 - 0.4 && z > -25.2 + 0.4,
                "{spot:?}"
            );
            assert!(
                !(x.abs() < 1.25 && z > -18.0),
                "{spot:?} stands on the walkway"
            );
            assert!(toward[0].is_finite() && toward[1].is_finite());
            let route = nav::plan(door, [x - cx, z - cz], &blocks, half)
                .unwrap_or_else(|e| panic!("{spot:?} is out of reach of the door: {e:?}"));
            assert!(!route.waypoints.is_empty());
            // Between her spots too, as she walks while she works.
            for other in [AliceSpot::Desk, AliceSpot::Workbench, AliceSpot::Podium] {
                let ([ox, oz], _) = other.local();
                nav::plan([x - cx, z - cz], [ox - cx, oz - cz], &blocks, half)
                    .unwrap_or_else(|e| panic!("{spot:?} to {other:?}: {e:?}"));
            }
        }
        // Her room's square holds every spot, inside the walker's bounds.
        let world = alice_area();
        assert_eq!(world.center, OWNERS_HOUSE.world(ALICE_ROOM.0));
        assert!((world.floor.unwrap() - floor()).abs() < 1e-6);
    }

    #[test]
    fn the_player_can_walk_up_and_talk_to_her_across_her_workstation() {
        use verse_world::social::nav;
        let ([cx, cz], half) = ALICE_ROOM;
        let blocks = room_blocks();
        let ([x, z], toward) = ALICE_DESK;
        let len = (toward[0] - x).hypot(toward[1] - z);
        // Where `--workshop-ask` and the captures stand the player.
        let walk_up = 2.2;
        let stand = [
            x + (toward[0] - x) / len * walk_up,
            z + (toward[1] - z) / len * walk_up,
        ];
        let reach = super::super::super::studio::TALK_REACH;
        assert!((stand[0] - x).hypot(stand[1] - z) <= reach);
        nav::plan(
            [0.0 - cx, -12.4 - cz],
            [stand[0] - cx, stand[1] - cz],
            &blocks,
            half,
        )
        .expect("the player walks from the door to her workstation");
        // She is seen from the door: nothing tall stands between them.
        for &[x0, x1, z0, z1, top] in GRECO_HOUSE.blocks {
            let across = x1 > -0.3 && x0 < 0.3;
            let along = z1 > -21.1 && z0 < -12.0;
            assert!(
                !(across && along && top > FLOOR + 1.0),
                "{:?}",
                [x0, x1, z0, z1]
            );
        }
    }
}
