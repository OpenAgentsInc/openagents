//! The city around the first town, after the rest of the illustrated map
//! (`docs/verse/everglade-map.svg`): Main Street's far blocks, the Fountain
//! Plaza and the Market Hall, Stoop Lane's townhouses, the Lantern Quarter's
//! pubs and halls, Brownstone Row, the Knowledge District's college and
//! Observatory Hill, the Foundry's and the Creative District's workshops and
//! ateliers, Fernhollow, the orchard and the beekeeper's hut, and the rest of
//! Walden Woods.
//!
//! Most buildings are the workshop's kit on its 2 m grid, one to three
//! stories of wall pieces under round-tile roofs, generated from the
//! [`BUILDINGS`] table. Their walls block walking as one footprint per wall
//! run ([`blocks`]) rather than one per piece, so the town's blockers stay
//! within navigation's bound. Some of the table's places hold a whole
//! generated model instead ([`STAND_INS`], `docs/verse/blender-pipeline.md`):
//! the townhouses, the corner shops, the market hall, the tavern, the row
//! houses, the L-shaped houses, the cottage with its tower, and the
//! observatory, which block by their own boxes (`generated`).

use super::generated::{
    BAKERY, BOARDWALK_CAFE, BOATHOUSE, BROWNSTONE, CLOCK_TOWER, CORNER_SHOP, COTTAGE_THATCH,
    COTTAGE_TOWER, FARMHOUSE, FOUNTAIN, GAMBREL_BARN, GAMBREL_HOUSE, GAZEBO, GREENHOUSE,
    GUILD_HALL, HIP_HOUSE, Instance, L_HOUSE, LANTERN_INN, LOG_CABIN, LOOKOUT, MARKET_HALL,
    MEETING_HALL, MUSIC_HALL, Model, OBSERVATORY, ROW_TOWNHOUSE, SHOP_HOUSE, SMITHY, STONE_COTTAGE,
    TAVERN, TIMBER_HOUSE, TOWNHOUSE_BALCONY, TOWNHOUSE_JETTIED, WINDMILL,
};
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
pub const BUILDINGS: [Building; 77] = [
    // Main Street's far blocks, on its north side.
    building("corner shop", [-88.0, 55.0], 8.0, S, 46.0, 2, Plaster),
    building("bakehouse", [-72.0, 55.0], 8.0, S, 46.0, 1, Timber),
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
    // Fernhollow's lookout tower.
    building("lookout hut", [72.0, 77.0], 8.0, W, 64.0, 2, Timber),
    // The second round, after the map: the clock tower on Library Way, the
    // farm at the long meadow's edge on its lane south of Brownstone Row,
    // the woodcutter's cottage in Walden Woods, and the cabin by the Fern
    // Pond. Each holds a generated model (`STAND_INS`).
    building("clock tower", [24.0, -21.0], 8.0, S, -29.0, 3, Stone),
    building("windmill", [30.0, -103.0], 8.0, N, -96.0, 3, Plaster),
    building("farmhouse", [14.0, -109.0], 8.0, N, -96.0, 1, Timber),
    building(
        "woodcutter's cottage",
        [-106.0, -74.0],
        8.0,
        E,
        -100.0,
        1,
        Timber,
    ),
    building("fern cabin", [88.0, 93.0], 8.0, S, 86.0, 1, Timber),
    // The third round: two hipped houses on Well Square, south of Hearth
    // Road between the Lantern Quarter and Stoop Lane, and the barn east
    // of the farm's paddock.
    building("well house west", [-75.0, -28.0], 8.0, E, -66.0, 2, Plaster),
    building(
        "well house south",
        [-60.0, -42.0],
        10.0,
        N,
        -33.0,
        2,
        Plaster,
    ),
    building("barn", [58.0, -105.0], 10.0, N, -96.0, 1, Timber),
    // The sixth round: Market Row, a lane behind Main Street's far blocks
    // on each side of the Fountain Plaza, with houses and shops on both
    // sides, as the map's dense blocks north of Main Street. Each holds a
    // lighter generated house (`STAND_INS`).
    building(
        "market row east 1",
        [32.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row east 2",
        [42.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row east 3",
        [52.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row east 4",
        [32.0, 83.0],
        8.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row east 5",
        [42.0, 83.0],
        8.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row east 6",
        [52.0, 83.0],
        8.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 1",
        [-32.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 2",
        [-42.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 3",
        [-52.0, 66.0],
        8.0,
        N,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 4",
        [-32.0, 83.0],
        8.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 5",
        [-42.0, 83.0],
        10.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
    building(
        "market row west 6",
        [-54.0, 83.0],
        8.0,
        S,
        MARKET_ROW,
        2,
        Plaster,
    ),
];

/// Market Row's z, m: the lane behind Main Street's far blocks.
pub const MARKET_ROW: f32 = 74.5;

/// The beekeeper's hut by the orchard.
const BEEKEEPER: Building = building("beekeeper's hut", [-70.0, 77.0], 8.0, W, -80.0, 1, Timber);

/// Every building, the table's and the beekeeper's hut.
fn all() -> impl Iterator<Item = &'static Building> {
    BUILDINGS.iter().chain(std::iter::once(&BEEKEEPER))
}

/// One generated model standing in a building's place.
#[derive(Clone, Copy, Debug)]
pub struct StandIn {
    /// The model's own name, unique among doors and fronts.
    pub name: &'static str,
    /// The [`BUILDINGS`] place it stands in.
    pub building: &'static str,
    pub model: &'static Model,
    /// How far along the door's wall from its center, m: east for a south
    /// or north wall, north for a west or east wall.
    pub along: f32,
    /// How far its front wall stands behind the place's wall line, m.
    pub setback: f32,
    pub scale: f32,
}

const fn stand_in(
    name: &'static str,
    building: &'static str,
    model: &'static Model,
    along: f32,
) -> StandIn {
    StandIn {
        name,
        building,
        model,
        along,
        setback: 0.0,
        scale: 1.0,
    }
}

/// The generated models that replace kit buildings, mixed among the kit
/// ones so each street varies: townhouses and corner shops on Main Street
/// and Stoop Lane, the open market hall on the Fountain Plaza, the tavern
/// in the Lantern Quarter, terraces of row houses on Brownstone Row, the
/// observatory on its hill, and the cottage with its tower in Walden Woods.
pub const STAND_INS: [StandIn; 74] = [
    stand_in("corner shop", "corner shop", &CORNER_SHOP, 0.0),
    stand_in("bakehouse", "bakehouse", &BAKERY, 0.0),
    stand_in("tailor", "tailor", &TOWNHOUSE_BALCONY, 0.0),
    stand_in("tea house", "tea house", &TOWNHOUSE_JETTIED, 0.0),
    stand_in("print shop", "print shop", &CORNER_SHOP, 0.0),
    stand_in("music shop", "music shop", &TOWNHOUSE_JETTIED, 0.0),
    stand_in("market hall", "market hall", &MARKET_HALL, 0.0),
    stand_in("townhouse 1", "townhouse 1", &TOWNHOUSE_JETTIED, 0.0),
    stand_in("townhouse 3", "townhouse 3", &TOWNHOUSE_BALCONY, 0.0),
    stand_in("townhouse 4", "townhouse 4", &TOWNHOUSE_BALCONY, 0.0),
    stand_in("townhouse 6", "townhouse 6", &TOWNHOUSE_JETTIED, 0.0),
    // The Lantern Quarter: the round Music Hall, the meeting hall back from
    // Hearth Road behind its porch, the tavern on its corner, and the guild
    // hall, a little south so its turret clears the Music Hall.
    stand_in("music hall", "music hall", &MUSIC_HALL, 0.0),
    StandIn {
        setback: 2.0,
        ..stand_in("meeting hall", "meeting hall", &MEETING_HALL, 0.0)
    },
    stand_in("the lantern", "the lantern", &TAVERN, 0.0),
    stand_in("guild hall", "guild hall", &GUILD_HALL, -2.0),
    stand_in("the snug", "the snug", &L_HOUSE, 0.0),
    stand_in("brownstone 3 west", "brownstone 3", &ROW_TOWNHOUSE, -2.1),
    stand_in("brownstone 3 east", "brownstone 3", &ROW_TOWNHOUSE, 2.1),
    stand_in("brownstone 4 west", "brownstone 4", &ROW_TOWNHOUSE, -2.1),
    stand_in("brownstone 4 east", "brownstone 4", &ROW_TOWNHOUSE, 2.1),
    stand_in("seminar house", "seminar house", &L_HOUSE, 0.0),
    // The drum tower stands at the middle of the hill's flat top.
    StandIn {
        setback: 4.0,
        scale: 1.6,
        ..stand_in("observatory", "observatory", &OBSERVATORY, 0.0)
    },
    stand_in("quiet cabin", "quiet cabin", &COTTAGE_TOWER, 0.0),
    stand_in("studio", "studio", &TOWNHOUSE_BALCONY, 0.0),
    // The Boardwalk Cafés face each other across Studio Road, their decks
    // to the street.
    StandIn {
        setback: 0.6,
        ..stand_in("boardwalk cafe east", "atelier", &BOARDWALK_CAFE, 0.0)
    },
    StandIn {
        setback: 0.6,
        ..stand_in("boardwalk cafe west", "pottery", &BOARDWALK_CAFE, 0.0)
    },
    stand_in("smithy", "forge", &SMITHY, -2.0),
    stand_in("sketch cabin", "sketch cabin", &LOG_CABIN, 0.0),
    stand_in("lookout", "lookout hut", &LOOKOUT, 0.0),
    stand_in("clock tower", "clock tower", &CLOCK_TOWER, 0.0),
    stand_in("windmill", "windmill", &WINDMILL, 0.0),
    stand_in("farmhouse", "farmhouse", &FARMHOUSE, 0.0),
    stand_in(
        "woodcutter's cottage",
        "woodcutter's cottage",
        &COTTAGE_THATCH,
        0.0,
    ),
    stand_in("fern cabin", "fern cabin", &LOG_CABIN, 0.0),
    stand_in("well house west", "well house west", &HIP_HOUSE, 0.0),
    stand_in("well house south", "well house south", &HIP_HOUSE, 0.0),
    stand_in("barn", "barn", &GAMBREL_BARN, 0.0),
    // The sixth round's lighter houses (`scripts/blender/town_houses.py`)
    // in most of the kit-built houses' places: shops on Main Street and the
    // Fountain Plaza, inns in the Lantern Quarter, brownstones with stoops
    // and stone cottages on Brownstone Row, and gambrel, stone, and
    // timber-framed houses through Stoop Lane, the Knowledge District, and
    // the Foundry. Each costs a quarter of a kit-built house's triangles.
    stand_in("hardware store", "hardware store", &SHOP_HOUSE, 0.0),
    stand_in("cheesemonger", "cheesemonger", &SHOP_HOUSE, 0.0),
    // South along its wall, so its walk crosses the plaza clear of the
    // fountain.
    stand_in("plaza cafe west", "plaza cafe west", &TIMBER_HOUSE, -2.3),
    stand_in("plaza cafe east", "plaza cafe east", &SHOP_HOUSE, 0.0),
    stand_in("townhouse 2", "townhouse 2", &GAMBREL_HOUSE, 0.0),
    stand_in("townhouse 5", "townhouse 5", &TIMBER_HOUSE, 0.0),
    stand_in("the fiddle", "the fiddle", &LANTERN_INN, 0.0),
    stand_in("the hearth", "the hearth", &TIMBER_HOUSE, 0.0),
    stand_in("choir house", "choir house", &GAMBREL_HOUSE, 0.0),
    stand_in("the lamplighter", "the lamplighter", &LANTERN_INN, 0.0),
    brownstone("brownstone 1"),
    brownstone("brownstone 2"),
    brownstone("brownstone 5"),
    brownstone("brownstone 6"),
    brownstone("row house 1"),
    stand_in("row house 2", "row house 2", &STONE_COTTAGE, 0.0),
    stand_in("row house 3", "row house 3", &GAMBREL_HOUSE, 0.0),
    brownstone("row house 4"),
    stand_in("row house 5", "row house 5", &STONE_COTTAGE, 0.0),
    stand_in("row house 6", "row house 6", &GAMBREL_HOUSE, 0.0),
    stand_in("archive", "archive", &STONE_COTTAGE, 0.0),
    stand_in("map room", "map room", &TIMBER_HOUSE, 0.0),
    stand_in("scriptorium", "scriptorium", &GAMBREL_HOUSE, 0.0),
    stand_in("workshop", "workshop", &TIMBER_HOUSE, 0.0),
    stand_in(
        "server barn annex",
        "server barn annex",
        &STONE_COTTAGE,
        0.0,
    ),
    row("market row east 1", &SHOP_HOUSE),
    row("market row east 2", &GAMBREL_HOUSE),
    row("market row east 3", &TIMBER_HOUSE),
    row("market row east 4", &STONE_COTTAGE),
    row("market row east 5", &SHOP_HOUSE),
    row("market row east 6", &GAMBREL_HOUSE),
    row("market row west 1", &TIMBER_HOUSE),
    row("market row west 2", &SHOP_HOUSE),
    row("market row west 3", &STONE_COTTAGE),
    row("market row west 4", &GAMBREL_HOUSE),
    row("market row west 5", &LANTERN_INN),
    row("market row west 6", &TIMBER_HOUSE),
];

/// A Market Row house in its own place.
const fn row(building: &'static str, model: &'static Model) -> StandIn {
    stand_in(building, building, model, 0.0)
}

/// A brownstone in `building`'s place, set back so its stoop stands on
/// its own ground in front of the wall line.
const fn brownstone(building: &'static str) -> StandIn {
    StandIn {
        setback: 2.6,
        ..stand_in(building, building, &BROWNSTONE, 0.0)
    }
}

/// Generated models that stand on open ground rather than in a building's
/// place: the boathouse on Lantern Pond's north bank, its arch to the
/// water; the glasshouse in the community garden behind Brownstone Row;
/// and the gazebo on the commons' east lawn.
pub const GROUNDS: [Instance; 3] = [
    Instance {
        scale: 0.8,
        ..Instance::new("boathouse", &BOATHOUSE, [4.5, 34.2], PI)
    },
    Instance::new("glasshouse", &GREENHOUSE, [-52.0, -58.0], PI),
    Instance::new("gazebo", &GAZEBO, [30.0, 26.0], -FRAC_PI_2),
];

/// The Fountain Plaza's fountain, on the plaza's west half clear of
/// Market Way.
pub const PLAZA_FOUNTAIN: Instance = Instance::new("fountain", &FOUNTAIN, [-6.0, 75.5], 0.0);

/// Whether a generated model stands in `b`'s place.
fn replaced(b: &Building) -> bool {
    STAND_INS.iter().any(|s| s.building == b.name)
}

/// The building in `name`'s place.
fn named(name: &str) -> &'static Building {
    all()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no building named {name}"))
}

/// A stand-in placed at its building's door wall, facing out of it.
fn place(s: &StandIn) -> Instance {
    let b = named(s.building);
    let ([cx, cz], [hx, hz]) = b.rect;
    let n = b.door.normal();
    let (wall, along) = match b.door {
        S => ([cx, cz - hz], [1.0, 0.0]),
        N => ([cx, cz + hz], [1.0, 0.0]),
        W => ([cx - hx, cz], [0.0, 1.0]),
        E => ([cx + hx, cz], [0.0, 1.0]),
    };
    Instance {
        name: s.name,
        model: s.model,
        at: [
            wall[0] - n[0] * s.setback + along[0] * s.along,
            wall[1] - n[1] * s.setback + along[1] * s.along,
        ],
        yaw: b.door.outward(),
        scale: s.scale,
    }
}

/// The front door of every building on Main Street's far blocks and on
/// Stoop Lane's western lane, kit-built or generated: its point on the
/// front wall's line and its outward normal.
pub(super) fn street_fronts() -> Vec<([f32; 2], [f32; 2])> {
    let on_street = |b: &Building| b.street == 46.0 || b.street == -60.0;
    let kit = all()
        .filter(|b| on_street(b) && !replaced(b))
        .map(|b| (door_centers(b)[0], b.door.normal()));
    let generated = STAND_INS
        .iter()
        .filter(|s| on_street(named(s.building)))
        .map(|s| {
            let i = place(s);
            (i.world([i.model.front[0], 0.0]), i.outward())
        });
    kit.chain(generated).collect()
}

/// The footprints of the city's kit-built houses, which take paint
/// (`super::paint`).
pub(super) fn kit_rects() -> impl Iterator<Item = ([f32; 2], [f32; 2])> {
    all().filter(|b| !replaced(b)).map(|b| b.rect)
}

/// The city's generated models: the stand-ins, the plaza's fountain, and
/// the models on open ground ([`GROUNDS`]).
#[must_use]
pub fn instances() -> Vec<Instance> {
    STAND_INS
        .iter()
        .map(place)
        .chain(std::iter::once(PLAZA_FOUNTAIN))
        .chain(GROUNDS)
        .collect()
}

/// The ground each of [`GROUNDS`] covers: the box around its blocks.
fn grounds_rects() -> impl Iterator<Item = ([f32; 2], [f32; 2])> {
    GROUNDS.iter().map(|i| {
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for (f, _) in i.blocks() {
            min = [min[0].min(f.min[0]), min[1].min(f.min[1])];
            max = [max[0].max(f.max[0]), max[1].max(f.max[1])];
        }
        (
            [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
            [(max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0],
        )
    })
}

/// The city's streets: each a segment and its half width, m.
pub const STREETS: [([f32; 2], [f32; 2], f32); 27] = [
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
    // The farm lane south from Brownstone Row to the windmill and the
    // farmhouse, and the path from Fernhollow's lookout to the fern cabin.
    ([20.0, -78.0], [20.0, -96.0], 1.0),
    ([8.0, -96.0], [34.0, -96.0], 1.0),
    ([64.0, 77.0], [66.0, 82.0], 0.9),
    ([66.0, 82.0], [87.0, 86.0], 0.9),
    // Stoop Lane's western lane, on south across Hearth Road to Well
    // Square.
    ([-60.0, -23.0], [-60.0, -8.0], 1.2),
    // Market Row, behind Main Street's far blocks: east to the path to
    // Fernhollow, and west to a lane from Main Street between the tailor
    // and the bakehouse.
    ([26.0, MARKET_ROW], [64.0, MARKET_ROW], 1.2),
    ([-26.0, MARKET_ROW], [-64.0, MARKET_ROW], 1.2),
    ([-64.0, 46.0], [-64.0, MARKET_ROW], 1.2),
];

/// The Fountain Plaza's paving: center and half extents, m.
pub const PLAZA: ([f32; 2], [f32; 2]) = ([0.0, 72.0], [10.0, 7.0]);
/// The second community garden, by Brownstone Row.
pub const GARDEN: ([f32; 2], [f32; 2]) = ([-66.0, -54.0], [6.0, 5.0]);
/// The orchard's rows north of Main Street.
pub const ORCHARD: ([f32; 2], [f32; 2]) = ([-100.0, 76.0], [12.0, 9.0]);

/// The footprints the city reserves: its buildings, the models on open
/// ground, the plaza, the garden, and the orchard.
pub fn reserved() -> impl Iterator<Item = ([f32; 2], [f32; 2])> {
    all()
        .map(|b| b.rect)
        .chain(grounds_rects())
        .chain([PLAZA, GARDEN, ORCHARD, super::WELL_SQUARE])
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
    let open = instances().into_iter().filter_map(|i| {
        let inside = i.model.inside?;
        Some((i.name, i.front(), i.world(inside)))
    });
    all()
        .filter(|b| !replaced(b))
        .map(|b| {
            let d = door_centers(b)[0];
            let n = b.door.normal();
            (
                b.name,
                [d[0] + 1.5 * n[0], d[1] + 1.5 * n[1]],
                [d[0] - 2.0 * n[0], d[1] - 2.0 * n[1]],
            )
        })
        .chain(open)
        .collect()
}

/// The streets and each building's walk from its street to its door.
pub fn roads() -> Vec<([f32; 2], [f32; 2], f32)> {
    let mut out = STREETS.to_vec();
    // Each stand-in's walk from its building's street to its front step.
    for s in &STAND_INS {
        let b = named(s.building);
        let end = place(s).front();
        let start = match b.door {
            S | N => [end[0], b.street],
            W | E => [b.street, end[1]],
        };
        if (start[0] - end[0]).abs() + (start[1] - end[1]).abs() > 0.5 {
            out.push((start, end, 1.0));
        }
    }
    for b in all().filter(|b| !replaced(b)) {
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

/// Each kit building's walls as one footprint per run between doorways,
/// and every generated model's boxes (`super::generated`), the first
/// town's and the city's, with their tops, m: what navigation plans
/// around.
#[must_use]
pub fn blocks() -> Vec<(Footprint, f32)> {
    let mut out: Vec<(Footprint, f32)> = super::generated()
        .iter()
        .flat_map(Instance::blocks)
        .collect();
    out.extend(kit_blocks());
    out
}

/// Each kit building's walls as one footprint per run between doorways,
/// with their tops, m. A generated model collides by its own triangles
/// instead (`demolition::carve`).
#[must_use]
pub fn kit_blocks() -> Vec<(Footprint, f32)> {
    let mut out = Vec::new();
    for b in all().filter(|b| !replaced(b)) {
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
        if !replaced(b) {
            raise(out, b, 500 + k as u32);
        }
    }
    out.extend(instances().iter().map(Instance::placement));
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

/// The Fountain Plaza's stalls, benches, and flower boxes, and the market
/// hall's long tables under its arcade. The plaza's cobbles are the
/// ground's (`super::PAVED_SQUARES`).
fn market(out: &mut Vec<Placement>) {
    // Stalls on the plaza's east side, facing the fountain across it, with
    // their stock beside them.
    for (k, z) in [66.5_f32, 77.0].into_iter().enumerate() {
        prop(out, super::STALL, [6.5, z], WEST);
        prop(out, "generated/barrel", [7.9, z + 1.6], 0.3 * k as f32);
    }
    prop(out, "village/Prop_Crate", [8.0, 69.0], 0.4);
    prop(out, "generated/hand_cart", [-8.5, 67.0], 0.5);
    prop(out, "props/Bench", [-6.0, 69.6], 0.0);
    prop(out, "props/Bench", [-6.0, 80.8], PI);
    // Flower boxes at the fountain's four sides.
    let [fx, fz] = PLAZA_FOUNTAIN.at;
    for (dx, dz, yaw) in [(-3.6, 0.0, FRAC_PI_2), (3.6, 0.0, FRAC_PI_2)] {
        prop(out, "generated/flower_box", [fx + dx, fz + dz], yaw);
    }
    // The market hall's stock under its arcade, clear of its open bay: a
    // hand cart of sacks, barrels, and crates.
    let hall = place(
        STAND_INS
            .iter()
            .find(|s| s.building == "market hall")
            .unwrap(),
    );
    prop(out, "generated/hand_cart", hall.world([-3.0, -4.2]), 1.2);
    for (k, local) in [[3.0, -4.6], [4.0, -3.6], [-4.6, -6.4]]
        .into_iter()
        .enumerate()
    {
        prop(out, "generated/barrel", hall.world(local), 0.9 * k as f32);
    }
    prop(out, "village/Prop_Crate", hall.world([3.6, -6.0]), 0.3);
    for (i, x) in [-80.0_f32, -48.0, 48.0, 80.0].into_iter().enumerate() {
        tree(
            out,
            super::foliage::PARK_TREES[i % 2],
            [x, 42.0],
            noise(i as u32, 92) * TAU,
            0.7,
        );
    }
}

/// The door of building `b`, kit or generated: a point on its walk just
/// outside, and its outward normal.
fn front_of(b: &Building) -> ([f32; 2], [f32; 2]) {
    if let Some(s) = STAND_INS.iter().find(|s| s.building == b.name) {
        let i = place(s);
        return (i.front(), i.outward());
    }
    let d = door_centers(b)[0];
    let n = b.door.normal();
    ([d[0] + 0.9 * n[0], d[1] + 0.9 * n[1]], n)
}

/// Stoop Lane's little gardens: a bush and flowers beside each
/// townhouse's door, and a flower box under its window.
fn stoops(out: &mut Vec<Placement>) {
    for (k, b) in BUILDINGS[11..17].iter().enumerate() {
        let (d, n) = front_of(b);
        let flowers = if k % 2 == 0 {
            "nature/Bush_Common_Flowers"
        } else {
            "nature/Bush_Common"
        };
        out.push(Placement::new(
            flowers,
            [d[0] + 0.4 * n[0], d[1] - 3.2],
            k as f32,
            Collision::Core(0.5),
        ));
        dress(
            out,
            "nature/Flower_3_Group",
            [d[0] + 0.3 * n[0], d[1] + 3.2],
            k as f32 * 1.3,
            0.8,
        );
    }
}

/// Lanterns by the Lantern Quarter's halls and pubs on Hearth Road, and the
/// pubs' tables.
fn lanterns(out: &mut Vec<Placement>) {
    for b in BUILDINGS[17..21].iter().filter(|b| !replaced(b)) {
        let doors = door_centers(b);
        // Beside a single door, or clear of both leaves of a double one.
        let (d, along) = match doors.as_slice() {
            [a, b] => ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], 2.8),
            _ => (doors[0], 1.6),
        };
        wall_lantern(out, d, b.door.outward(), along);
    }
    for b in [BUILDINGS[18], BUILDINGS[20]]
        .iter()
        .filter(|b| !replaced(b))
    {
        let ([cx, cz], _) = b.rect;
        prop(out, "props/Table_Large", [cx + 1.0, cz], FRAC_PI_2);
    }
    // Barrels by the tavern's door, and tables out front.
    let tavern = place(
        STAND_INS
            .iter()
            .find(|s| s.building == "the lantern")
            .unwrap(),
    );
    for (k, local) in [[-4.8, 1.0], [-4.0, 1.3], [4.6, 1.1]]
        .into_iter()
        .enumerate()
    {
        prop(out, "generated/barrel", tavern.world(local), 0.7 * k as f32);
    }
}

/// Flowers in Brownstone Row's areaways, inside the brownstones' rails.
fn brownstones(out: &mut Vec<Placement>) {
    for (k, s) in STAND_INS
        .iter()
        .filter(|s| std::ptr::eq(s.model, &BROWNSTONE))
        .enumerate()
    {
        if k % 2 == 0 {
            dress(
                out,
                "nature/Flower_4_Group",
                place(s).world([-1.4, 1.0]),
                k as f32,
                0.6,
            );
        }
    }
}

/// The college's furniture, the observatory's bench, and the long meadow's
/// flowers.
fn knowledge(out: &mut Vec<Placement>) {
    let ([cx, cz], _) = BUILDINGS[40].rect;
    for x in [cx - 6.0, cx + 6.0] {
        prop(out, "props/Bookcase_2", [x, cz + 4.4], PI);
    }
    prop(out, "props/Table_Large", [cx + 3.0, cz + 1.0], 0.0);
    // A bench on the hill's top, looking west over the town.
    let ([ox, oz], _) = BUILDINGS[46].rect;
    prop(out, "props/Bench", [ox - 5.6, oz + 4.6], WEST);
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
    // The Sculpture Walk: a statue, a bronze ring on its cairn, a sundial,
    // and a second statue on brick pavers, facing the walk.
    for (i, z) in [14.0_f32, 20.0, 26.0, 32.0].into_iter().enumerate() {
        let at = [58.0, z];
        out.push(Placement::new("village/Floor_Brick", at, 0.0, Collision::None).lift(0.03));
        let model = [
            "generated/statue",
            "generated/sculpture",
            "generated/sundial",
            "generated/statue",
        ][i];
        out.push(
            Placement::new(model, at, WEST + 0.3 * (i as f32 - 1.5), Collision::Bounds).lift(0.03),
        );
    }
}

/// Fernhollow: pines around the Fern Pond, ferns, and mushrooms.
fn fernhollow(out: &mut Vec<Placement>) {
    for (i, at) in [[100.0, 82.0], [78.0, 90.0], [104.0, 66.0], [80.0, 64.0]]
        .into_iter()
        .enumerate()
    {
        let scale = [1.0, 1.15][i % 2];
        tree(
            out,
            super::foliage::FIR.0,
            at,
            noise(i as u32, 95) * TAU,
            scale,
        );
    }
    for (i, at) in [[86.0, 72.0], [94.0, 81.0], [96.0, 71.0], [82.0, 80.0]]
        .into_iter()
        .enumerate()
    {
        dress(out, "nature/Fern_1", at, noise(i as u32, 96) * TAU, 0.4);
    }
    dress(out, "nature/Mushroom_Common", [88.0, 84.0], 0.6, 1.3);
}

/// The orchard's rows of fruit trees and the beekeeper's hives.
fn orchard(out: &mut Vec<Placement>) {
    let ([ox, oz], [ohx, ohz]) = ORCHARD;
    for i in 0..4 {
        for j in 0..3 {
            // Rows a little uneven, as an old orchard's are.
            let at = [
                ox - ohx + 3.0 + 6.0 * i as f32 + noise(i * 3 + j, 96) - 0.5,
                oz - ohz + 3.0 + 6.0 * j as f32 + noise(i * 3 + j, 98) - 0.5,
            ];
            let scale = 0.9 + 0.3 * noise(i * 3 + j, 99);
            tree(
                out,
                "generated/fruit_tree",
                at,
                noise(i * 3 + j, 97) * TAU,
                scale,
            );
        }
    }
    let ([bx, bz], [_, bhz]) = BEEKEEPER.rect;
    for k in 0..2 {
        prop(
            out,
            "generated/beehives",
            [bx - 1.0 + 3.4 * k as f32, bz + bhz + 1.8],
            0.1 * k as f32,
        );
    }
    dress(out, "nature/Flower_4_Group", [bx + 6.0, bz + 6.0], 0.3, 1.0);
}

/// Walden Woods: trees at hashed points of open ground south and west of
/// the Lantern Quarter, and the Thinking Pond's bench.
fn woods(out: &mut Vec<Placement>) {
    prop(out, "props/Bench", [-107.4, -58.0], WEST);
    // Dark spruces and pale birches among the kit's pines: Walden's own
    // trees, cheaper than the kit's, so the woods stand thicker.
    let models = [
        super::foliage::FIR.0,
        "generated/spruce_low",
        "generated/birch_low",
        super::foliage::PARK_TREES[0],
        "generated/spruce_low",
        "generated/birch_low",
        super::foliage::FIR.0,
        "generated/spruce_low",
    ];
    let mut placed = 0;
    for n in 0..600_u32 {
        if placed == 34 {
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
