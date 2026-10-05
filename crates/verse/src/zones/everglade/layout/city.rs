//! The city around the first town, after the rest of the illustrated map
//! (`docs/verse/everglade-map.svg`): Main Street's far blocks, the Fountain
//! Plaza and the Market Hall, Stoop Lane's townhouses, the Lantern Quarter's
//! pubs and halls, Brownstone Row, the Knowledge District's college and
//! Observatory Hill, the Foundry's and the Creative District's workshops and
//! ateliers, Fernhollow, the orchard and the beekeeper's hut, and the rest of
//! Walden Woods.
//!
//! Every building is the workshop's kit on its 2 m grid, one to three
//! stories of wall pieces under round-tile roofs, generated from the
//! [`BUILDINGS`] table. Its walls block walking as one footprint per wall
//! run ([`blocks`]) rather than one per piece, so the town's blockers stay
//! within navigation's bound.

use super::{
    Collision, DOOR_HALF, EAST, NORTH, Piece, Placement, SOUTH, WALL_TOP, WEST, dress, height,
    noise, prop, roof_at, tree, wall, wall_lantern,
};
use crate::controller::Footprint;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The wall a building's door is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    South,
    North,
    West,
    East,
}

impl Side {
    const ALL: [Self; 4] = [Self::South, Self::North, Self::West, Self::East];

    /// The wall's outward heading, as the controller's yaw.
    fn outward(self) -> f32 {
        match self {
            Self::South => SOUTH,
            Self::North => NORTH,
            Self::West => WEST,
            Self::East => EAST,
        }
    }

    /// The wall's outward normal, x and z.
    fn normal(self) -> [f32; 2] {
        match self {
            Self::South => [0.0, -1.0],
            Self::North => [0.0, 1.0],
            Self::West => [-1.0, 0.0],
            Self::East => [1.0, 0.0],
        }
    }
}

/// How a building's walls look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Plaster with flat windows and some timber framing.
    Plaster,
    /// Stone-based walls: the brownstones and the college.
    Stone,
    /// Timber framing: cabins, barns, and workshops.
    Timber,
}

/// One closed building of the city.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub name: &'static str,
    /// Center and half extents, m. The depth is always 10 m, so the 8 x 10
    /// roofs fit, and the width a multiple of 8 m.
    pub rect: ([f32; 2], [f32; 2]),
    /// The wall its door is in.
    pub door: Side,
    /// Where its walk meets the street, m: z for a door in the south or
    /// north wall, x for one in the west or east wall.
    pub street: f32,
    pub stories: u8,
    pub style: Style,
}

const fn building(
    name: &'static str,
    center: [f32; 2],
    width: f32,
    door: Side,
    street: f32,
    stories: u8,
    style: Style,
) -> Building {
    Building {
        name,
        rect: (center, [width / 2.0, 5.0]),
        door,
        street,
        stories,
        style,
    }
}

use Side::{East as E, North as N, South as S, West as W};
use Style::{Plaster, Stone, Timber};

/// Every building of the city, by district.
pub const BUILDINGS: [Building; 57] = [
    // Main Street's far blocks, on its north side.
    building("corner shop", [-88.0, 55.0], 8.0, S, 46.0, 2, Plaster),
    building("chandler", [-72.0, 55.0], 8.0, S, 46.0, 1, Timber),
    building("tailor", [-56.0, 55.0], 8.0, S, 46.0, 2, Plaster),
    building("tea house", [-40.0, 55.0], 8.0, S, 46.0, 1, Plaster),
    building("print shop", [40.0, 55.0], 8.0, S, 46.0, 2, Plaster),
    building("cheesemonger", [56.0, 55.0], 8.0, S, 46.0, 1, Timber),
    building("music shop", [72.0, 55.0], 8.0, S, 46.0, 2, Stone),
    building("hardware store", [88.0, 55.0], 8.0, S, 46.0, 1, Plaster),
    // The Market Hall on the Fountain Plaza, and the plaza's two cafés.
    building("market hall", [0.0, 90.0], 24.0, S, 80.0, 2, Stone),
    building("plaza cafe west", [-20.0, 71.0], 8.0, E, -1.0, 1, Timber),
    building("plaza cafe east", [20.0, 71.0], 8.0, W, 1.0, 2, Plaster),
    // Stoop Lane's townhouses, both sides of its western lane.
    building("townhouse 1", [-52.0, 35.0], 8.0, W, -60.0, 2, Plaster),
    building("townhouse 2", [-52.0, 21.0], 8.0, W, -60.0, 2, Stone),
    building("townhouse 3", [-52.0, 7.0], 8.0, W, -60.0, 1, Plaster),
    building("townhouse 4", [-68.0, 35.0], 8.0, E, -60.0, 1, Timber),
    building("townhouse 5", [-68.0, 21.0], 8.0, E, -60.0, 2, Plaster),
    building("townhouse 6", [-68.0, 7.0], 8.0, E, -60.0, 2, Stone),
    // The Lantern Quarter: pubs and meeting halls on Hearth Road and
    // Lantern Road, and the Music Hall.
    building("meeting hall", [-86.0, 0.0], 16.0, S, -8.0, 2, Plaster),
    building("the lantern", [-112.0, 0.0], 8.0, S, -8.0, 1, Timber),
    building("music hall", [-80.0, -16.0], 16.0, N, -8.0, 2, Stone),
    building("the fiddle", [-112.0, -16.0], 8.0, N, -8.0, 1, Timber),
    building("guild hall", [-88.0, -29.0], 8.0, W, -100.0, 1, Plaster),
    building("the hearth", [-112.0, -29.0], 8.0, E, -100.0, 2, Timber),
    building("choir house", [-88.0, 21.0], 8.0, W, -100.0, 2, Timber),
    building(
        "the lamplighter",
        [-112.0, 21.0],
        8.0,
        E,
        -100.0,
        1,
        Plaster,
    ),
    building("the snug", [-112.0, 35.0], 8.0, E, -100.0, 1, Stone),
    // Brownstone Row: two-story brownstones on its north side, and lower
    // homes across the street.
    building("brownstone 1", [-14.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("brownstone 2", [-24.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("brownstone 3", [-44.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("brownstone 4", [-54.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("brownstone 5", [-64.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("brownstone 6", [-74.0, -71.0], 8.0, S, -78.0, 2, Stone),
    building("row house 1", [-14.0, -87.0], 8.0, N, -78.0, 1, Stone),
    building("row house 2", [-24.0, -87.0], 8.0, N, -78.0, 1, Plaster),
    building("row house 3", [-44.0, -87.0], 8.0, N, -78.0, 1, Stone),
    building("row house 4", [-54.0, -87.0], 8.0, N, -78.0, 1, Plaster),
    building("row house 5", [-64.0, -87.0], 8.0, N, -78.0, 1, Stone),
    building("row house 6", [-74.0, -87.0], 8.0, N, -78.0, 1, Plaster),
    // Walden Woods' far cabins.
    building("prototype shed", [-88.0, -92.0], 8.0, N, -78.0, 1, Timber),
    building("quiet cabin", [-116.0, -45.0], 8.0, E, -100.0, 1, Timber),
    // The Knowledge District: the college along Library Way, the
    // observatory on its hill, and the sketch cabin in the long meadow.
    building("college hall", [54.0, -20.0], 16.0, S, -29.0, 2, Stone),
    building("lecture hall", [80.0, -20.0], 16.0, S, -29.0, 2, Plaster),
    building("archive", [100.0, -20.0], 8.0, S, -29.0, 1, Stone),
    building("seminar house", [54.0, -38.0], 8.0, N, -29.0, 1, Plaster),
    building("map room", [76.0, -38.0], 8.0, N, -29.0, 2, Stone),
    building("scriptorium", [96.0, -38.0], 8.0, N, -29.0, 1, Timber),
    building("observatory", [88.0, -69.0], 8.0, W, 64.0, 3, Stone),
    building("sketch cabin", [44.0, -90.0], 8.0, N, -78.0, 1, Timber),
    // The Foundry along Foundry Road.
    building("workshop", [76.0, 9.0], 8.0, S, 0.0, 1, Timber),
    building("fab hall", [96.0, 9.0], 16.0, S, 0.0, 2, Timber),
    building("server barn annex", [76.0, -9.0], 8.0, N, 0.0, 1, Timber),
    building("forge", [96.0, -9.0], 16.0, N, 0.0, 2, Stone),
    // The Creative District along Studio Road.
    building("studio", [76.0, 38.0], 8.0, S, 29.0, 2, Plaster),
    building("atelier", [92.0, 38.0], 8.0, S, 29.0, 1, Timber),
    building("pottery", [76.0, 20.0], 8.0, N, 29.0, 1, Plaster),
    building("atelier hall", [96.0, 20.0], 16.0, N, 29.0, 2, Plaster),
    // Fernhollow's lookout hut.
    building("lookout hut", [72.0, 77.0], 8.0, W, 64.0, 2, Timber),
];

/// The beekeeper's hut by the orchard.
const BEEKEEPER: Building = building("beekeeper's hut", [-70.0, 77.0], 8.0, W, -80.0, 1, Timber);

/// Every building, the table's and the beekeeper's hut.
fn all() -> impl Iterator<Item = &'static Building> {
    BUILDINGS.iter().chain(std::iter::once(&BEEKEEPER))
}

/// The city's streets: each a segment and its half width, m.
pub const STREETS: [([f32; 2], [f32; 2], f32); 19] = [
    // Main Street's far blocks.
    ([-100.0, 46.0], [-34.0, 46.0], 1.6),
    ([44.0, 46.0], [104.0, 46.0], 1.6),
    // Market Way, from Main Street across the Fountain Plaza.
    ([0.0, 46.0], [0.0, 80.0], 1.4),
    // Hearth Road west, between Stoop Lane's homes into the Lantern Quarter.
    ([-118.0, -8.0], [-34.0, -8.0], 1.3),
    // Lantern Road, from Main Street to Brownstone Row.
    ([-100.0, -78.0], [-100.0, 46.0], 1.4),
    // Stoop Lane's western lane, and Stoop Lane south to Brownstone Row.
    ([-60.0, -8.0], [-60.0, 46.0], 1.2),
    ([-34.0, -78.0], [-34.0, -40.0], 1.4),
    // Brownstone Row.
    ([-100.0, -78.0], [20.0, -78.0], 1.4),
    // Library Way east to the college, and south to the long meadow.
    ([39.0, -29.0], [104.0, -29.0], 1.4),
    ([20.0, -78.0], [20.0, -29.0], 1.2),
    ([20.0, -78.0], [64.0, -78.0], 1.2),
    // Foundry Road: north and south through the east of town, and east
    // past the workshops, joined to the first town's road north of the
    // Server Barn.
    ([64.0, -78.0], [64.0, 46.0], 1.4),
    ([40.0, 0.0], [40.0, 10.0], 1.2),
    ([40.0, 10.0], [64.0, 10.0], 1.2),
    ([64.0, 0.0], [112.0, 0.0], 1.3),
    // Studio Road east, through the Creative District.
    ([64.0, 29.0], [112.0, 29.0], 1.3),
    // The paths to Fernhollow and to the orchard.
    ([64.0, 46.0], [64.0, 77.0], 1.1),
    ([-80.0, 46.0], [-80.0, 77.0], 1.1),
    // Around the Thinking Pond.
    ([-100.0, -56.0], [-107.0, -56.0], 1.0),
];

/// The Fountain Plaza's paving: center and half extents, m.
pub const PLAZA: ([f32; 2], [f32; 2]) = ([0.0, 72.0], [10.0, 7.0]);
/// The second community garden, by Brownstone Row.
pub const GARDEN: ([f32; 2], [f32; 2]) = ([-66.0, -54.0], [6.0, 5.0]);
/// The orchard's rows north of Main Street.
pub const ORCHARD: ([f32; 2], [f32; 2]) = ([-100.0, 76.0], [12.0, 9.0]);

/// The footprints the city reserves: its buildings, the plaza, the garden,
/// and the orchard.
pub fn reserved() -> impl Iterator<Item = ([f32; 2], [f32; 2])> {
    all().map(|b| b.rect).chain([PLAZA, GARDEN, ORCHARD])
}

/// The centers of a building's doorways on its wall line: a double door in
/// the middle of a wide south or north wall, else one door.
fn door_centers(b: &Building) -> Vec<[f32; 2]> {
    let ([cx, cz], [hx, hz]) = b.rect;
    match b.door {
        S | N => {
            let z = if b.door == S { cz - hz } else { cz + hz };
            if hx >= 8.0 {
                vec![[cx - 1.0, z], [cx + 1.0, z]]
            } else {
                vec![[cx - 1.0, z]]
            }
        }
        W | E => {
            let x = if b.door == W { cx - hx } else { cx + hx };
            vec![[x, cz]]
        }
    }
}

/// Each building's doorway: a point outside on its walk and one inside.
pub fn doors() -> Vec<(&'static str, [f32; 2], [f32; 2])> {
    all()
        .map(|b| {
            let d = door_centers(b)[0];
            let n = b.door.normal();
            (
                b.name,
                [d[0] + 1.5 * n[0], d[1] + 1.5 * n[1]],
                [d[0] - 2.0 * n[0], d[1] - 2.0 * n[1]],
            )
        })
        .collect()
}

/// The streets and each building's walk from its street to its door.
pub fn roads() -> Vec<([f32; 2], [f32; 2], f32)> {
    let mut out = STREETS.to_vec();
    for b in all() {
        let doors = door_centers(b);
        // A double door's walk meets the pier between its leaves.
        let along = doors.iter().map(|d| [d[0], d[1]]).fold([0.0; 2], |s, d| {
            [
                s[0] + d[0] / doors.len() as f32,
                s[1] + d[1] / doors.len() as f32,
            ]
        });
        let n = b.door.normal();
        let end = [along[0] + 0.7 * n[0], along[1] + 0.7 * n[1]];
        let start = match b.door {
            S | N => [end[0], b.street],
            W | E => [b.street, end[1]],
        };
        if (start[0] - end[0]).abs() + (start[1] - end[1]).abs() > 0.5 {
            out.push((start, end, 1.0));
        }
    }
    out
}

/// Each building's walls as one footprint per run between doorways, with
/// the walls' top, m.
#[must_use]
pub fn blocks() -> Vec<(Footprint, f32)> {
    let mut out = Vec::new();
    for b in all() {
        let ([cx, cz], [hx, hz]) = b.rect;
        let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
        let top = height(cx, cz) + WALL_TOP * f32::from(b.stories);
        for side in Side::ALL {
            // Along the wall from end to end, past the corner posts, and
            // across from 0.1 m outside its line to 0.32 m inside.
            let (lo, hi) = match side {
                S | N => (west - 0.12, east + 0.12),
                W | E => (south - 0.12, north + 0.12),
            };
            let gaps: Vec<f32> = if side == b.door {
                door_centers(b)
                    .iter()
                    .map(|d| if matches!(side, S | N) { d[0] } else { d[1] })
                    .collect()
            } else {
                Vec::new()
            };
            let mut runs = Vec::new();
            let mut from = lo;
            for g in gaps {
                runs.push((from, g - DOOR_HALF));
                from = g + DOOR_HALF;
            }
            runs.push((from, hi));
            for (a, z) in runs.into_iter().filter(|(a, z)| z > a) {
                let footprint = match side {
                    S => Footprint {
                        min: [a, south - 0.1],
                        max: [z, south + 0.32],
                    },
                    N => Footprint {
                        min: [a, north - 0.32],
                        max: [z, north + 0.1],
                    },
                    W => Footprint {
                        min: [west - 0.1, a],
                        max: [west + 0.32, z],
                    },
                    E => Footprint {
                        min: [east - 0.32, a],
                        max: [east + 0.1, z],
                    },
                };
                out.push((footprint, top));
            }
        }
    }
    out
}

/// The wall pieces of one side of one story, from west to east or from
/// south to north.
fn pieces(b: &Building, side: Side, story: u8, salt: u32) -> Vec<Piece> {
    use Piece::{Base, Door, Flat, Plain, Round, Timber as Grid};
    let ([cx, cz], [hx, hz]) = b.rect;
    let count = match side {
        S | N => hx as usize,
        W | E => hz as usize,
    };
    let doors: Vec<usize> = if story == 0 && side == b.door {
        door_centers(b)
            .iter()
            .map(|d| match side {
                S | N => ((d[0] - (cx - hx) - 1.0) / 2.0).round() as usize,
                W | E => ((d[1] - (cz - hz) - 1.0) / 2.0).round() as usize,
            })
            .collect()
    } else {
        Vec::new()
    };
    let front = side == b.door;
    (0..count)
        .map(|i| {
            if doors.contains(&i) {
                return Door;
            }
            let r = noise(i as u32 + 16 * story as u32, salt + side as u32 * 7);
            let end = i == 0 || i + 1 == count;
            // Upper stories and back walls are mostly plain plaster, the
            // kit's cheapest piece; the fronts carry the detail.
            if (story > 0 || !front) && r > 0.6 {
                return if b.style == Style::Stone { Base } else { Plain };
            }
            match b.style {
                Plaster if end => Plain,
                Plaster if front && story == 0 && r < 0.4 => Round,
                Plaster if r < 0.55 => Flat,
                Plaster if r < 0.8 => Grid,
                Plaster => Plain,
                Stone if end || (story == 0 && r < 0.3) => Base,
                Stone if r < 0.75 => Flat,
                Stone => Plain,
                Timber if end => Grid,
                Timber if r < 0.4 => Flat,
                Timber if r < 0.8 => Grid,
                Timber => Plain,
            }
        })
        .collect()
}

/// One building: its stories of wall pieces, corner posts, a floor, its
/// roofs, and sometimes a chimney.
fn raise(out: &mut Vec<Placement>, b: &Building, salt: u32) {
    let ([cx, cz], [hx, hz]) = b.rect;
    let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
    let mut pieces_out = Vec::new();
    for story in 0..b.stories {
        let lift = WALL_TOP * f32::from(story);
        for side in Side::ALL {
            for (i, piece) in pieces(b, side, story, salt).into_iter().enumerate() {
                let step = 1.0 + 2.0 * i as f32;
                let center = match side {
                    S => [west + step, south],
                    N => [west + step, north],
                    W => [west, south + step],
                    E => [east, south + step],
                };
                pieces_out.clear();
                wall(&mut pieces_out, piece, center, side.outward());
                // The walls block as runs (`blocks`), not piece by piece.
                out.extend(pieces_out.iter().map(|p| {
                    Placement {
                        collision: Collision::None,
                        ..*p
                    }
                    .lift(lift)
                }));
            }
        }
        for corner in [[west, south], [east, south], [west, north], [east, north]] {
            out.push(
                Placement::new("village/Corner_Exterior_Wood", corner, 0.0, Collision::None)
                    .lift(lift),
            );
        }
    }
    let floor = if b.style == Style::Timber {
        "village/Floor_WoodDark"
    } else {
        "village/Floor_Brick"
    };
    for i in 0..(hx as i32) {
        for j in 0..(hz as i32) {
            let at = [west + 1.0 + 2.0 * i as f32, south + 1.0 + 2.0 * j as f32];
            out.push(Placement::new(floor, at, 0.0, Collision::None).lift(0.02));
        }
    }
    let eaves = WALL_TOP * f32::from(b.stories);
    roof_at(out, b.rect, eaves);
    if noise(salt, 90) < 0.3 {
        let side = if noise(salt, 91) < 0.5 { -1.0 } else { 1.0 };
        out.push(
            Placement::new(
                "village/Prop_Chimney",
                [cx + side * (hx - 2.0), cz + 2.5],
                0.0,
                Collision::None,
            )
            .lift(eaves + 1.78),
        );
    }
}

/// Every placement of the city.
pub fn build(out: &mut Vec<Placement>) {
    for (k, b) in all().enumerate() {
        raise(out, b, 500 + k as u32);
    }
    market(out);
    stoops(out);
    lanterns(out);
    brownstones(out);
    knowledge(out);
    workshops(out);
    fernhollow(out);
    orchard(out);
    woods(out);
}

/// The Fountain Plaza's paving, fountain, and stalls, and the Market
/// Hall's long tables.
fn market(out: &mut Vec<Placement>) {
    let ([px, pz], [phx, phz]) = PLAZA;
    for i in 0..(phx as i32) {
        for j in 0..(phz as i32) {
            let at = [
                px - phx + 1.0 + 2.0 * i as f32,
                pz - phz + 1.0 + 2.0 * j as f32,
            ];
            out.push(Placement::new("village/Floor_Brick", at, 0.0, Collision::None).lift(0.02));
        }
    }
    // Stalls on the plaza's east side, benches facing the fountain.
    for z in [66.0_f32, 78.0] {
        prop(out, "props/Table_Large", [6.0, z], 0.0);
        prop(out, "village/Prop_Crate", [8.4, z + 0.2], 0.4);
        out.push(Placement::new("props/Scroll_1", [5.6, z], 0.6, Collision::None).lift(0.81));
    }
    prop(out, "props/Bench", [-6.0, 67.0], 0.0);
    // The Market Hall's tables, inside its doors.
    let ([mx, mz], _) = BUILDINGS[8].rect;
    for x in [mx - 7.0, mx + 7.0] {
        prop(out, "props/Table_Large", [x, mz], 0.0);
        prop(out, "village/Prop_Crate", [x + 2.4, mz + 2.0], 0.3);
    }
    // Lanterns on the hall's front and street trees on Main Street.
    wall_lantern(out, [mx - 4.0, mz - 5.0], SOUTH, 0.0);
    wall_lantern(out, [mx + 4.0, mz - 5.0], SOUTH, 0.0);
    for (i, x) in [-80.0_f32, -48.0, 48.0, 80.0].into_iter().enumerate() {
        tree(
            out,
            "nature/CommonTree_5",
            [x, 42.0],
            noise(i as u32, 92) * TAU,
            0.7,
        );
    }
}

/// Stoop Lane's little gardens: a bush and flowers at each townhouse door.
fn stoops(out: &mut Vec<Placement>) {
    for (k, b) in BUILDINGS[11..17].iter().enumerate() {
        let d = door_centers(b)[0];
        let n = b.door.normal();
        let flowers = if k % 2 == 0 {
            "nature/Bush_Common_Flowers"
        } else {
            "nature/Bush_Common"
        };
        out.push(Placement::new(
            flowers,
            [d[0] + 1.3 * n[0], d[1] - 3.2],
            k as f32,
            Collision::Core(0.5),
        ));
        dress(
            out,
            "nature/Flower_3_Group",
            [d[0] + 1.2 * n[0], d[1] + 3.2],
            k as f32 * 1.3,
            0.8,
        );
    }
}

/// Lanterns by the Lantern Quarter's halls and pubs on Hearth Road, and the
/// Music Hall's stage.
fn lanterns(out: &mut Vec<Placement>) {
    for b in &BUILDINGS[17..21] {
        let doors = door_centers(b);
        // Beside a single door, or clear of both leaves of a double one.
        let (d, along) = match doors.as_slice() {
            [a, b] => ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], 2.8),
            _ => (doors[0], 1.6),
        };
        wall_lantern(out, d, b.door.outward(), along);
    }
    let ([mx, mz], _) = BUILDINGS[19].rect;
    for i in 0..4 {
        let at = [mx - 3.0 + 2.0 * i as f32, mz - 3.0];
        out.push(Placement::new("village/Floor_WoodDark", at, 0.0, Collision::None).lift(0.05));
    }
    prop(out, "props/BookStand", [mx, mz - 3.4], 0.0);
    for x in [mx - 5.0, mx - 3.0, mx + 3.0, mx + 5.0] {
        prop(out, "props/Stool", [x, mz + 2.0], PI);
    }
    for b in [BUILDINGS[18], BUILDINGS[20]] {
        let ([cx, cz], _) = b.rect;
        prop(out, "props/Table_Large", [cx + 1.0, cz], FRAC_PI_2);
    }
}

/// Brownstone Row's stoops and the community garden behind it.
fn brownstones(out: &mut Vec<Placement>) {
    for (k, b) in BUILDINGS[26..32].iter().enumerate() {
        let d = door_centers(b)[0];
        out.push(
            Placement::new(
                "village/Stairs_Exterior_Straight",
                [d[0] + 2.0, d[1] - 1.1],
                SOUTH,
                Collision::None,
            )
            .scale(0.5),
        );
        if k % 2 == 0 {
            dress(
                out,
                "nature/Flower_4_Group",
                [d[0] + 3.4, d[1] - 1.0],
                k as f32,
                0.7,
            );
        }
    }
    super::garden_plot(out, GARDEN, 60);
}

/// The college's furniture and the sculptures of the Sculpture Walk.
fn knowledge(out: &mut Vec<Placement>) {
    let ([cx, cz], _) = BUILDINGS[40].rect;
    for x in [cx - 6.0, cx + 6.0] {
        prop(out, "props/Bookcase_2", [x, cz + 4.4], PI);
    }
    prop(out, "props/Table_Large", [cx + 3.0, cz + 1.0], 0.0);
    // The observatory's lookout: a vine on its tower and a bench below.
    let ([ox, oz], [ohx, _]) = BUILDINGS[46].rect;
    prop(out, "props/Bench", [ox - ohx - 1.4, oz + 3.2], WEST);
    // The long meadow's flowers.
    for (i, at) in [
        [28.0, -66.0],
        [36.0, -88.0],
        [52.0, -70.0],
        [12.0, -90.0],
        [30.0, -100.0],
        [56.0, -100.0],
    ]
    .into_iter()
    .enumerate()
    {
        let model = if i % 2 == 0 {
            "nature/Flower_4_Group"
        } else {
            "nature/Flower_3_Group"
        };
        dress(out, model, at, noise(i as u32, 93) * TAU, 1.0);
    }
}

/// The Foundry's and the Creative District's yards: metal stock, a wagon,
/// workbenches, and the Sculpture Walk's stones on brick plinths.
fn workshops(out: &mut Vec<Placement>) {
    prop(out, "props/Crate_Metal", [106.0, 3.0], 0.2);
    prop(out, "village/Prop_Crate", [107.4, 3.4], 0.5);
    prop(out, "village/Prop_Wagon", [110.0, -6.0], 0.0);
    prop(out, "props/Workbench", [80.0, 2.6], 0.0);
    prop(out, "props/Anvil", [72.0, -2.6], 0.4);
    prop(out, "props/Workbench", [80.0, 26.4], PI);
    for (i, z) in [14.0_f32, 20.0, 26.0, 32.0].into_iter().enumerate() {
        let at = [58.0, z];
        out.push(Placement::new("village/Floor_Brick", at, 0.0, Collision::None).lift(0.03));
        let model = ["nature/Rock_Medium_3", "nature/Rock_Medium_1"][i % 2];
        out.push(
            Placement::new(model, at, noise(i as u32, 94) * TAU, Collision::Bounds)
                .scale(0.45)
                .lift(0.03),
        );
    }
}

/// Fernhollow: pines around the Fern Pond, ferns, and mushrooms.
fn fernhollow(out: &mut Vec<Placement>) {
    for (i, at) in [[100.0, 82.0], [84.0, 88.0], [104.0, 66.0], [80.0, 64.0]]
        .into_iter()
        .enumerate()
    {
        let model = ["nature/Pine_1", "nature/Pine_2"][i % 2];
        tree(out, model, at, noise(i as u32, 95) * TAU, 1.0);
    }
    for (i, at) in [[86.0, 72.0], [94.0, 81.0], [96.0, 71.0], [82.0, 80.0]]
        .into_iter()
        .enumerate()
    {
        dress(out, "nature/Fern_1", at, noise(i as u32, 96) * TAU, 0.4);
    }
    dress(out, "nature/Mushroom_Common", [88.0, 84.0], 0.6, 1.3);
}

/// The orchard's rows of young fruit trees and the beekeeper's hives.
fn orchard(out: &mut Vec<Placement>) {
    let ([ox, oz], [ohx, ohz]) = ORCHARD;
    for i in 0..4 {
        for j in 0..3 {
            let at = [
                ox - ohx + 3.0 + 6.0 * i as f32,
                oz - ohz + 3.0 + 6.0 * j as f32,
            ];
            let model = if (i + j) % 2 == 0 {
                "nature/CommonTree_5"
            } else {
                "nature/CommonTree_3"
            };
            tree(out, model, at, noise(i * 3 + j, 97) * TAU, 0.6);
        }
    }
    let ([bx, bz], [_, bhz]) = BEEKEEPER.rect;
    for k in 0..4 {
        prop(
            out,
            "village/Prop_Crate",
            [bx - 1.5 + 2.0 * k as f32, bz + bhz + 1.6],
            0.2 * k as f32,
        );
    }
    dress(out, "nature/Flower_4_Group", [bx + 6.0, bz + 6.0], 0.3, 1.0);
}

/// Walden Woods: trees at hashed points of open ground south and west of
/// the Lantern Quarter, and the Thinking Pond's bench.
fn woods(out: &mut Vec<Placement>) {
    prop(out, "props/Bench", [-107.4, -58.0], WEST);
    let models = ["nature/Pine_1", "nature/CommonTree_4", "nature/Pine_2"];
    let mut placed = 0;
    for n in 0..400_u32 {
        if placed == 20 {
            break;
        }
        let x = -132.0 + 52.0 * noise(n, 98);
        let z = -112.0 + 76.0 * noise(n, 99);
        if x.hypot(z) > 150.0 || !super::open_ground(x, z) {
            continue;
        }
        tree(
            out,
            models[placed % models.len()],
            [x, z],
            noise(n, 100) * TAU,
            0.9 + 0.3 * noise(n, 101),
        );
        placed += 1;
    }
}
