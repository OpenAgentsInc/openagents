//! The Airbnb: hand-made room modules on a grid of cells, joined by
//! doorways, shuffled from a seed (`docs/verse/games/the-ice-cream-man.md`,
//! The Airbnb).
//!
//! Every module keeps a plus-shaped lane clear through its middle (|x| or
//! |z| under [`LANE`] in room space), so each doorway, key spot and use
//! spot is reachable in a straight line from the room's centre. Furniture
//! sits in the four corners.

use std::collections::VecDeque;

use crate::geom::{Block, V2, first_hit, v};
use crate::rng::Rng;

/// A cell's side, in metres.
pub const CELL: f32 = 10.0;
/// Walls' thickness and height.
pub const WALL: f32 = 0.3;
pub const WALL_HEIGHT: f32 = 3.2;
/// A doorway's width.
pub const DOOR: f32 = 2.2;
/// Half the width of each room's clear plus.
pub const LANE: f32 = 1.5;
/// The original grid: 4 by 4 rooms. A run's house is
/// `layout.house_size` rooms along each side *tunable*.
pub const COLS: usize = 4;
pub const ROWS: usize = 4;
/// The biggest house, in rooms along a side.
pub const MAX_SIZE: usize = 10;
/// How many keys a run hides.
pub const KEYS: usize = 3;
/// Buttons in the spear puzzle (`House::buttons`).
pub const BUTTONS: usize = 3;

/// The room modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Module {
    Bedroom,
    Kitchen,
    LivingRoom,
    Bathroom,
    Laundry,
    Hallway,
    Stairwell,
    FreezerRoom,
    /// The Backrooms: a wide empty yellow room with wall stubs in its
    /// corners (only in a house built as the Backrooms).
    Backroom,
}

impl Module {
    /// The Airbnb's own modules (not [`Module::Backroom`]).
    pub const ALL: [Self; 8] = [
        Self::Bedroom,
        Self::Kitchen,
        Self::LivingRoom,
        Self::Bathroom,
        Self::Laundry,
        Self::Hallway,
        Self::Stairwell,
        Self::FreezerRoom,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bedroom => "Bedroom",
            Self::Kitchen => "Kitchen",
            Self::LivingRoom => "Living room",
            Self::Bathroom => "Bathroom",
            Self::Laundry => "Laundry",
            Self::Hallway => "Hallway",
            Self::Stairwell => "Stairwell",
            Self::FreezerRoom => "The freezer room",
            Self::Backroom => "The Backrooms",
        }
    }

    #[must_use]
    pub fn def(self) -> &'static ModuleDef {
        match self {
            Self::Bedroom => &BEDROOM,
            Self::Kitchen => &KITCHEN,
            Self::LivingRoom => &LIVING_ROOM,
            Self::Bathroom => &BATHROOM,
            Self::Laundry => &LAUNDRY,
            Self::Hallway => &HALLWAY,
            Self::Stairwell => &STAIRWELL,
            Self::FreezerRoom => &FREEZER_ROOM,
            Self::Backroom => &BACKROOM,
        }
    }
}

/// The wall colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flavor {
    Strawberry,
    Mint,
    Vanilla,
    Chocolate,
}

impl Flavor {
    pub const ALL: [Self; 4] = [Self::Strawberry, Self::Mint, Self::Vanilla, Self::Chocolate];
}

/// What a piece of furniture is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropKind {
    Bed,
    Closet,
    Dresser,
    Counter,
    Fridge,
    Table,
    Sofa,
    Tv,
    Tub,
    Toilet,
    Sink,
    Washer,
    Shelf,
    Stairs,
    Freezer,
    Pillar,
    Plant,
    /// A stub of wall, Backrooms style.
    Partition,
}

/// A piece of furniture in room space (-5 to 5 on each axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub kind: PropKind,
    pub min: V2,
    pub max: V2,
    pub height: f32,
    /// Where the player stands to use it (closets and fridges).
    pub use_at: Option<V2>,
}

const fn prop(kind: PropKind, x0: f32, z0: f32, x1: f32, z1: f32, height: f32) -> Prop {
    Prop {
        kind,
        min: v(x0, z0),
        max: v(x1, z1),
        height,
        use_at: None,
    }
}

const fn usable(kind: PropKind, x0: f32, z0: f32, x1: f32, z1: f32, h: f32, at: V2) -> Prop {
    Prop {
        kind,
        min: v(x0, z0),
        max: v(x1, z1),
        height: h,
        use_at: Some(at),
    }
}

/// A room module: its furniture and where a key can lie.
#[derive(Debug)]
pub struct ModuleDef {
    pub props: &'static [Prop],
    pub key_slots: &'static [V2],
    /// Where an extra closet fits, clear of the lanes, the furniture and
    /// the key spots, when a house asks for more hiding spots.
    pub spare_closet: Option<Prop>,
}

use PropKind as K;

const IN: f32 = 4.85;

static BEDROOM: ModuleDef = ModuleDef {
    props: &[
        prop(K::Bed, 2.0, -IN, IN, -2.0, 0.6),
        usable(K::Closet, -IN, -IN, -3.4, -2.4, 2.4, v(-2.6, -3.6)),
        prop(K::Dresser, -IN, 2.6, -2.4, IN, 1.0),
        prop(K::Plant, 3.6, 3.6, 4.4, 4.4, 1.2),
    ],
    key_slots: &[v(3.6, 0.0), v(0.0, 3.6), v(-3.4, 0.0), v(3.0, -1.6)],
    spare_closet: Some(usable(K::Closet, 2.0, 3.6, 3.4, IN, 2.4, v(2.7, 2.9))),
};

static KITCHEN: ModuleDef = ModuleDef {
    props: &[
        prop(K::Counter, 1.8, -IN, IN, -4.0, 1.0),
        usable(K::Fridge, -IN, -IN, -3.6, -3.6, 2.0, v(-2.9, -2.9)),
        prop(K::Table, 2.2, 2.2, 4.0, 4.0, 0.8),
        prop(K::Counter, -IN, 2.0, -4.0, IN, 1.0),
    ],
    key_slots: &[v(0.0, -3.8), v(3.2, -2.8), v(-3.6, 0.0), v(0.0, 3.6)],
    spare_closet: Some(usable(K::Closet, -3.9, 3.6, -2.4, IN, 2.4, v(-3.1, 2.9))),
};

static LIVING_ROOM: ModuleDef = ModuleDef {
    props: &[
        prop(K::Sofa, 2.0, -4.5, 4.6, -3.3, 0.8),
        prop(K::Table, 2.6, -2.6, 3.8, -1.9, 0.5),
        usable(K::Closet, -IN, -IN, -3.4, -2.6, 2.4, v(-2.7, -3.7)),
        prop(K::Tv, 3.8, 2.0, IN, 4.5, 1.4),
        prop(K::Plant, -4.4, 3.6, -3.6, 4.4, 1.2),
    ],
    key_slots: &[v(0.0, -3.6), v(2.6, 3.0), v(-3.6, 0.0), v(3.6, 0.0)],
    spare_closet: None,
};

static BATHROOM: ModuleDef = ModuleDef {
    props: &[
        prop(K::Tub, 2.0, -IN, IN, -3.2, 0.7),
        prop(K::Toilet, 3.8, 3.4, 4.6, 4.6, 0.8),
        prop(K::Sink, -IN, 3.0, -4.0, 4.4, 1.0),
        usable(K::Closet, -IN, -4.6, -3.6, -2.4, 2.2, v(-2.9, -3.5)),
    ],
    key_slots: &[v(3.2, -2.4), v(0.0, 3.6), v(-3.6, 0.0), v(2.4, 2.6)],
    spare_closet: Some(usable(K::Closet, 1.8, 3.6, 3.4, IN, 2.2, v(2.6, 2.9))),
};

static LAUNDRY: ModuleDef = ModuleDef {
    props: &[
        prop(K::Washer, 3.7, -IN, IN, -3.7, 1.0),
        prop(K::Washer, 2.4, -IN, 3.6, -3.7, 1.0),
        usable(K::Fridge, 3.6, 3.6, IN, IN, 2.0, v(2.9, 2.9)),
        usable(K::Closet, -IN, 2.4, -3.4, IN, 2.4, v(-2.7, 3.6)),
        prop(K::Shelf, -IN, -IN, -2.0, -4.2, 1.8),
    ],
    key_slots: &[v(3.0, -2.6), v(0.0, -3.6), v(3.6, 0.0), v(-3.0, -2.8)],
    spare_closet: None,
};

static HALLWAY: ModuleDef = ModuleDef {
    props: &[
        prop(K::Pillar, -IN, -IN, -1.7, -1.7, WALL_HEIGHT),
        prop(K::Pillar, 1.7, -IN, IN, -1.7, WALL_HEIGHT),
        prop(K::Pillar, -IN, 1.7, -1.7, IN, WALL_HEIGHT),
        prop(K::Pillar, 1.7, 1.7, IN, IN, WALL_HEIGHT),
    ],
    key_slots: &[v(0.0, -3.8), v(3.8, 0.0), v(0.0, 3.8), v(-3.8, 0.0)],
    spare_closet: None,
};

static STAIRWELL: ModuleDef = ModuleDef {
    props: &[
        prop(K::Stairs, 1.8, -IN, IN, -1.8, 3.0),
        usable(K::Closet, -IN, 2.0, -3.0, IN, 2.4, v(-2.3, 3.4)),
        prop(K::Plant, 3.6, 3.6, 4.4, 4.4, 1.2),
    ],
    key_slots: &[v(-3.0, -3.0), v(0.0, 3.6), v(3.6, 0.0), v(2.4, 2.6)],
    spare_closet: None,
};

static FREEZER_ROOM: ModuleDef = ModuleDef {
    props: &[
        prop(K::Freezer, 2.0, -4.6, 4.6, -3.0, 1.0),
        prop(K::Freezer, -4.6, 3.0, -2.0, 4.6, 1.0),
        usable(K::Fridge, -IN, -IN, -3.6, -3.6, 2.4, v(-2.9, -2.9)),
        usable(K::Fridge, 3.6, 3.6, IN, IN, 2.4, v(2.9, 2.9)),
    ],
    key_slots: &[v(3.0, -2.2), v(-3.0, 2.2), v(0.0, -3.8), v(3.8, 0.0)],
    spare_closet: None,
};

/// The Backrooms: four L-shaped wall stubs in the corners, the plus clear.
static BACKROOM: ModuleDef = ModuleDef {
    props: &[
        prop(K::Partition, -3.6, -3.6, -1.9, -3.3, WALL_HEIGHT),
        prop(K::Partition, -3.6, -3.3, -3.3, -1.9, WALL_HEIGHT),
        prop(K::Partition, 1.9, 3.3, 3.6, 3.6, WALL_HEIGHT),
        prop(K::Partition, 3.3, 1.9, 3.6, 3.3, WALL_HEIGHT),
        prop(K::Partition, 2.4, -IN, 2.7, -2.6, WALL_HEIGHT),
        prop(K::Partition, -IN, 2.4, -2.6, 2.7, WALL_HEIGHT),
    ],
    key_slots: &[v(0.0, -3.8), v(3.8, 0.0), v(0.0, 3.8), v(-3.8, 0.0)],
    spare_closet: None,
};

/// A grid cell: column and row.
pub type Cell = (usize, usize);

/// The four sides of a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    North,
    East,
    South,
    West,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// The step to the neighbour on this side (z grows southward).
    #[must_use]
    pub fn step(self) -> (isize, isize) {
        match self {
            Self::North => (0, -1),
            Self::East => (1, 0),
            Self::South => (0, 1),
            Self::West => (-1, 0),
        }
    }

    #[must_use]
    pub fn outward(self) -> V2 {
        let (x, z) = self.step();
        v(x as f32, z as f32)
    }
}

/// One room of the house.
#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub cell: Cell,
    pub module: Module,
    pub flavor: Flavor,
    /// Doorways, by side.
    pub doors: [bool; 4],
}

/// What a fridge holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pickup {
    /// More hot fudge.
    Fudge,
    /// One more fart cloud.
    Fart,
}

/// A usable spot in the house: a closet to hide in or a fridge to open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    /// World position where the player stands.
    pub at: V2,
    /// The prop's own box, in world space.
    pub block: Block,
    pub room: usize,
}

/// The front door: in an outer wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrontDoor {
    pub room: usize,
    pub side: Side,
    /// The middle of the doorway in the wall.
    pub at: V2,
    /// Where the player stands to unlock it.
    pub stand: V2,
}

/// What shape of house to build: its size, its doorways, and where the
/// keys go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Rooms along each side.
    pub size: usize,
    /// Doorways beyond the ones that join every room.
    pub loops: usize,
    pub backrooms: bool,
    /// Keys at the far ends (else in any rooms).
    pub spread: bool,
    /// The last key is locked in the cone cabinet.
    pub puzzle_key: bool,
}

impl Shape {
    /// The house a run with `tuning` plays.
    #[must_use]
    pub fn of(tuning: &crate::tuning::Tuning) -> Self {
        Self {
            size: (tuning.house_size as usize).clamp(3, MAX_SIZE),
            loops: tuning.loops as usize,
            backrooms: tuning.backrooms == 1,
            spread: tuning.key_spread == 1,
            puzzle_key: tuning.puzzle_key == 1,
        }
    }
}

/// A whole house from a seed.
#[derive(Clone, Debug, PartialEq)]
pub struct House {
    pub seed: u64,
    /// Rooms across (x) and down (z).
    pub cols: usize,
    pub rows: usize,
    /// Rooms in row-major order.
    pub rooms: Vec<Room>,
    /// Walls and furniture in world space.
    pub blocks: Vec<Block>,
    pub closets: Vec<Spot>,
    pub fridges: Vec<(Spot, Pickup)>,
    pub keys: Vec<V2>,
    pub spawn: V2,
    pub spawn_room: usize,
    pub man_start: V2,
    pub front: FrontDoor,
    /// Furniture added beyond the rooms' modules (spare closets), by room,
    /// in room space.
    pub extras: Vec<(usize, Prop)>,
    /// The spear puzzle's three sprinkle buttons, pressed in this order,
    /// each in its own room (used when the earned weapons are in the game).
    pub buttons: Vec<V2>,
    /// The cone cabinet the buttons open, with the waffle cone spear in it.
    pub cabinet: V2,
    /// Built as the Backrooms.
    pub backrooms: bool,
    /// The key locked in the cone cabinet until the puzzle is solved
    /// (`keys[i]` lies at [`House::cabinet`]).
    pub cabinet_key: Option<usize>,
}

/// The centre of a cell in world space.
#[must_use]
pub fn centre(cell: Cell) -> V2 {
    v(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}

/// The cell a point is in, clamped to the biggest grid
/// ([`House::cell_of`] clamps to a house's own).
#[must_use]
pub fn cell_of(p: V2) -> Cell {
    let col = (p.x / CELL).floor().clamp(0.0, (MAX_SIZE - 1) as f32) as usize;
    let row = (p.z / CELL).floor().clamp(0.0, (MAX_SIZE - 1) as f32) as usize;
    (col, row)
}

fn neighbour_in(cell: Cell, side: Side, cols: usize, rows: usize) -> Option<Cell> {
    let (dx, dz) = side.step();
    let col = cell.0.checked_add_signed(dx)?;
    let row = cell.1.checked_add_signed(dz)?;
    (col < cols && row < rows).then_some((col, row))
}

fn side_index(side: Side) -> usize {
    match side {
        Side::North => 0,
        Side::East => 1,
        Side::South => 2,
        Side::West => 3,
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::North => Side::South,
        Side::East => Side::West,
        Side::South => Side::North,
        Side::West => Side::East,
    }
}

impl House {
    /// The house a run with `tuning` plays: its size, loops, the
    /// Backrooms or not, its keys, and its extra closets.
    #[must_use]
    pub fn of(seed: u64, tuning: &crate::tuning::Tuning) -> Self {
        Self::with_closets(
            Self::generate(seed, Shape::of(tuning)),
            tuning.extra_closets as usize,
        )
    }

    /// The cell a point is in, clamped to this house.
    #[must_use]
    pub fn cell_of(&self, p: V2) -> Cell {
        let col = (p.x / CELL).floor().clamp(0.0, (self.cols - 1) as f32) as usize;
        let row = (p.z / CELL).floor().clamp(0.0, (self.rows - 1) as f32) as usize;
        (col, row)
    }

    /// A cell's room, in [`House::rooms`].
    #[must_use]
    pub fn index(&self, cell: Cell) -> usize {
        cell.1 * self.cols + cell.0
    }

    /// The room a point is in.
    #[must_use]
    pub fn room_of(&self, p: V2) -> usize {
        self.index(self.cell_of(p))
    }

    fn neighbour(&self, cell: Cell, side: Side) -> Option<Cell> {
        neighbour_in(cell, side, self.cols, self.rows)
    }

    /// The house for `seed` with `extra` more closets to hide in (up to
    /// one in each room whose module has room for one): the same house
    /// otherwise.
    #[must_use]
    pub fn tuned(seed: u64, extra: usize) -> Self {
        Self::with_closets(Self::new(seed), extra)
    }

    fn with_closets(mut house: Self, extra: usize) -> Self {
        let seed = house.seed;
        if extra == 0 {
            return house;
        }
        let mut rooms: Vec<usize> = (0..house.rooms.len())
            .filter(|i| house.rooms[*i].module.def().spare_closet.is_some())
            .collect();
        // Its own stream, so the rest of the house stays as it was.
        let mut rng = Rng::new(seed ^ 0x5eed_c105_e7c1_05e7);
        rng.shuffle(&mut rooms);
        for room in rooms.into_iter().take(extra) {
            let Some(p) = house.rooms[room].module.def().spare_closet else {
                continue;
            };
            let c = centre(house.rooms[room].cell);
            let block = Block {
                min: c + p.min,
                max: c + p.max,
                height: p.height,
            };
            house.blocks.push(block);
            if let Some(at) = p.use_at {
                house.closets.push(Spot {
                    at: c + at,
                    block,
                    room,
                });
            }
            house.extras.push((room, p));
        }
        house
    }

    /// A room's furniture in room space: its module's and any extras.
    #[must_use]
    pub fn props(&self, room: usize) -> Vec<Prop> {
        let mut props = self.rooms[room].module.def().props.to_vec();
        props.extend(
            self.extras
                .iter()
                .filter(|(r, _)| *r == room)
                .map(|(_, p)| *p),
        );
        props
    }

    /// Builds the Airbnb for `seed`, as the original game did (the game
    /// itself plays [`House::of`] its tuning).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::build(seed, crate::tuning::AIRBNB_LOOPS as usize, false)
    }

    /// Builds the 4 by 4 house for `seed` with `loops` extra doorways, as
    /// the Backrooms or not.
    #[must_use]
    pub fn build(seed: u64, loops: usize, backrooms: bool) -> Self {
        Self::generate(
            seed,
            Shape {
                size: COLS,
                loops,
                backrooms,
                spread: false,
                puzzle_key: false,
            },
        )
    }

    /// Builds the house for `seed` in `shape`.
    #[must_use]
    pub fn generate(seed: u64, shape: Shape) -> Self {
        let Shape {
            size,
            loops,
            backrooms,
            ..
        } = shape;
        let (cols, rows) = (size, size);
        let mut rng = Rng::new(seed);
        let count = cols * rows;
        let corners = [(0, 0), (cols - 1, 0), (0, rows - 1), (cols - 1, rows - 1)];
        let spawn_cell = corners[rng.below(4)];
        let spawn_room = spawn_cell.1 * cols + spawn_cell.0;

        let mut kinds: Vec<Module> = if backrooms {
            // The Backrooms: a few of the Airbnb's rooms (closets and
            // fridges to use) among empty yellow ones.
            // A bigger house has more of them, about as many per room.
            let own = [
                Module::Bedroom,
                Module::Kitchen,
                Module::Bathroom,
                Module::FreezerRoom,
                Module::LivingRoom,
                Module::Laundry,
            ];
            let mut kinds = Vec::new();
            for _ in 0..(count / 16).max(1) {
                kinds.extend(own);
            }
            kinds.resize(count - 1, Module::Backroom);
            kinds
        } else {
            // Modules: every kind once, then repeats, hallways most (the
            // house keeps going).
            let mut kinds: Vec<Module> = Module::ALL.to_vec();
            while kinds.len() < count - 1 {
                let pick = rng.below(Module::ALL.len() + 3);
                kinds.push(Module::ALL.get(pick).copied().unwrap_or(Module::Hallway));
            }
            kinds
        };
        rng.shuffle(&mut kinds);
        let mut kinds = kinds.into_iter();
        let mut rooms: Vec<Room> = (0..count)
            .map(|i| {
                let cell = (i % cols, i / cols);
                let module = if i == spawn_room {
                    Module::Bedroom
                } else {
                    kinds.next().unwrap_or(Module::Hallway)
                };
                Room {
                    cell,
                    module,
                    flavor: Flavor::ALL[0],
                    doors: [false; 4],
                }
            })
            .collect();
        for room in &mut rooms {
            room.flavor = Flavor::ALL[rng.below(4)];
        }

        let index = |cell: Cell| cell.1 * cols + cell.0;
        let neighbour = |cell: Cell, side: Side| neighbour_in(cell, side, cols, rows);

        // Doorways: a random tree from the bedroom, then a few loops.
        let mut seen = vec![false; count];
        let mut stack = vec![spawn_cell];
        seen[spawn_room] = true;
        while let Some(&cell) = stack.last() {
            let open: Vec<(Side, Cell)> = Side::ALL
                .iter()
                .filter_map(|side| neighbour(cell, *side).map(|n| (*side, n)))
                .filter(|(_, n)| !seen[index(*n)])
                .collect();
            if open.is_empty() {
                stack.pop();
                continue;
            }
            let (side, next) = open[rng.below(open.len())];
            rooms[index(cell)].doors[side_index(side)] = true;
            rooms[index(next)].doors[side_index(opposite(side))] = true;
            seen[index(next)] = true;
            stack.push(next);
        }
        let mut added = 0;
        let mut tries = 0;
        while added < loops && tries < 100.max(loops * 25) {
            tries += 1;
            let cell = rooms[rng.below(count)].cell;
            let side = Side::ALL[rng.below(4)];
            if let Some(next) = neighbour(cell, side)
                && !rooms[index(cell)].doors[side_index(side)]
            {
                rooms[index(cell)].doors[side_index(side)] = true;
                rooms[index(next)].doors[side_index(opposite(side))] = true;
                added += 1;
            }
        }

        let mut house = Self {
            seed,
            cols,
            rows,
            rooms,
            blocks: Vec::new(),
            closets: Vec::new(),
            fridges: Vec::new(),
            keys: Vec::new(),
            spawn: centre(spawn_cell) + v(0.0, 1.0),
            spawn_room,
            man_start: V2::ZERO,
            front: FrontDoor {
                room: 0,
                side: Side::North,
                at: V2::ZERO,
                stand: V2::ZERO,
            },
            extras: Vec::new(),
            buttons: Vec::new(),
            cabinet: V2::ZERO,
            backrooms,
            cabinet_key: None,
        };
        let distance = house.distances(spawn_cell);

        // The front door: the outer room farthest from the bedroom.
        let mut front_room = spawn_room;
        for (i, room) in house.rooms.iter().enumerate() {
            let outer = Side::ALL
                .iter()
                .any(|side| house.neighbour(room.cell, *side).is_none());
            if outer && distance[i] > distance[front_room] {
                front_room = i;
            }
        }
        let outer: Vec<Side> = Side::ALL
            .iter()
            .copied()
            .filter(|side| {
                house
                    .neighbour(house.rooms[front_room].cell, *side)
                    .is_none()
            })
            .collect();
        let side = outer[rng.below(outer.len())];
        let mid = centre(house.rooms[front_room].cell) + side.outward() * (CELL / 2.0);
        house.front = FrontDoor {
            room: front_room,
            side,
            at: mid,
            stand: mid - side.outward() * 1.1,
        };

        // Keys: three other rooms two or more doorways from the bedroom
        // (one fewer on the floor when the last is in the cone cabinet).
        let floor_keys = if shape.puzzle_key { KEYS - 1 } else { KEYS };
        let mut candidates: Vec<usize> = (0..count)
            .filter(|i| *i != spawn_room && *i != front_room && distance[*i] >= 2)
            .collect();
        if candidates.len() < KEYS {
            candidates = (0..count)
                .filter(|i| *i != spawn_room && *i != front_room)
                .collect();
        }
        rng.shuffle(&mut candidates);
        let key_rooms: Vec<usize> = if shape.spread {
            // The far ends: each key in the room farthest (in doorways)
            // from the bedroom, the front door and the keys before it.
            let mut anchors = vec![
                distance.clone(),
                house.distances(house.rooms[front_room].cell),
            ];
            let mut picked = Vec::new();
            for _ in 0..floor_keys {
                let best = candidates
                    .iter()
                    .copied()
                    .filter(|r| !picked.contains(r))
                    .max_by_key(|r| anchors.iter().map(|d| d[*r]).min().unwrap_or(0));
                if let Some(room) = best {
                    picked.push(room);
                    anchors.push(house.distances(house.rooms[room].cell));
                }
            }
            picked
        } else {
            candidates.iter().copied().take(floor_keys).collect()
        };
        for room in key_rooms {
            let slots = house.rooms[room].module.def().key_slots;
            let slot = slots[rng.below(slots.len())];
            house.keys.push(centre(house.rooms[room].cell) + slot);
        }

        // The Ice Cream Man starts three or more rooms away.
        let far: Vec<usize> = (0..count).filter(|i| distance[*i] >= 3).collect();
        let man_room = if far.is_empty() {
            (0..count).max_by_key(|i| distance[*i]).unwrap_or(0)
        } else {
            far[rng.below(far.len())]
        };
        house.man_start = centre(house.rooms[man_room].cell);

        house.build_blocks(&mut rng);
        house.place_puzzle(front_room, shape.spread);
        if shape.puzzle_key {
            house.cabinet_key = Some(house.keys.len());
            house.keys.push(house.cabinet);
        }
        house
    }

    /// The spear puzzle: three buttons and the cone cabinet, each in its
    /// own room away from the bedroom and the front door, on a key spot
    /// the room's key doesn't use. Its own stream, so the rest of the
    /// house is the same with or without it.
    ///
    /// With `spread`, the puzzle's rooms keep away from the key rooms too,
    /// and the cabinet goes to the room farthest from the bedroom and
    /// the front door of those picked.
    fn place_puzzle(&mut self, front_room: usize, spread: bool) {
        let mut rng = Rng::new(self.seed ^ 0x5b0a_7c0d_e5ea_3001);
        let key_rooms: Vec<usize> = self.keys.iter().map(|k| self.room_of(*k)).collect();
        let mut rooms: Vec<usize> = (0..self.rooms.len())
            .filter(|i| *i != self.spawn_room && *i != front_room)
            .filter(|i| !spread || !key_rooms.contains(i))
            .collect();
        rng.shuffle(&mut rooms);
        if spread {
            let from_bed = self.distances(self.rooms[self.spawn_room].cell);
            let from_door = self.distances(self.rooms[front_room].cell);
            // Each in the room farthest from the bedroom, the front door,
            // the keys and the puzzle's rooms before it.
            let mut anchors = vec![from_bed.clone(), from_door.clone()];
            anchors.extend(
                key_rooms
                    .iter()
                    .map(|r| self.distances(self.rooms[*r].cell)),
            );
            let mut picked = Vec::new();
            for _ in 0..=BUTTONS {
                let best = rooms
                    .iter()
                    .copied()
                    .filter(|r| !picked.contains(r))
                    .max_by_key(|r| anchors.iter().map(|d| d[*r]).min().unwrap_or(0));
                if let Some(room) = best {
                    picked.push(room);
                    anchors.push(self.distances(self.rooms[room].cell));
                }
            }
            rooms = picked;
            // The cabinet last: the farthest of them.
            if let Some(far) =
                (0..rooms.len()).max_by_key(|i| from_bed[rooms[*i]].min(from_door[rooms[*i]]))
            {
                let room = rooms.remove(far);
                rooms.push(room);
            }
        }
        let mut spots = Vec::new();
        for room in rooms.into_iter().take(BUTTONS + 1) {
            let c = centre(self.rooms[room].cell);
            let slots: Vec<V2> = self.rooms[room]
                .module
                .def()
                .key_slots
                .iter()
                .map(|s| c + *s)
                .filter(|s| self.keys.iter().all(|k| k.distance(*s) > 1.0))
                .collect();
            spots.push(slots[rng.below(slots.len())]);
        }
        self.cabinet = spots.pop().unwrap_or(self.spawn);
        self.buttons = spots;
    }

    /// Walls (with gaps for doorways) and furniture, and the closets and
    /// fridges with what each fridge holds.
    fn build_blocks(&mut self, rng: &mut Rng) {
        let half = WALL / 2.0;
        let gap = DOOR / 2.0;
        let wall = |min: V2, max: V2| Block {
            min,
            max,
            height: WALL_HEIGHT,
        };
        // Each room draws its north and west walls; the last row and column
        // also their south and east.
        for room in &self.rooms {
            let (col, row) = room.cell;
            let x0 = col as f32 * CELL;
            let z0 = row as f32 * CELL;
            let mut sides = vec![Side::North, Side::West];
            if col == self.cols - 1 {
                sides.push(Side::East);
            }
            if row == self.rows - 1 {
                sides.push(Side::South);
            }
            for side in sides {
                let open = room.doors[side_index(side)];
                let horizontal = matches!(side, Side::North | Side::South);
                let line = match side {
                    Side::North => z0,
                    Side::South => z0 + CELL,
                    Side::West => x0,
                    Side::East => x0 + CELL,
                };
                let (start, end) = if horizontal {
                    (x0 - half, x0 + CELL + half)
                } else {
                    (z0 - half, z0 + CELL + half)
                };
                let mid = (start + end) / 2.0;
                let pieces: Vec<(f32, f32)> = if open {
                    vec![(start, mid - gap), (mid + gap, end)]
                } else {
                    vec![(start, end)]
                };
                for (a, b) in pieces {
                    self.blocks.push(if horizontal {
                        wall(v(a, line - half), v(b, line + half))
                    } else {
                        wall(v(line - half, a), v(line + half, b))
                    });
                }
            }
        }
        for (i, room) in self.rooms.iter().enumerate() {
            let c = centre(room.cell);
            for p in room.module.def().props {
                let block = Block {
                    min: c + p.min,
                    max: c + p.max,
                    height: p.height,
                };
                self.blocks.push(block);
                if let Some(at) = p.use_at {
                    let spot = Spot {
                        at: c + at,
                        block,
                        room: i,
                    };
                    match p.kind {
                        PropKind::Closet => self.closets.push(spot),
                        PropKind::Fridge => {
                            let pickup = if rng.below(3) == 0 {
                                Pickup::Fart
                            } else {
                                Pickup::Fudge
                            };
                            self.fridges.push((spot, pickup));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Steps through doorways from `from` to every room.
    #[must_use]
    pub fn distances(&self, from: Cell) -> Vec<usize> {
        let mut distance = vec![usize::MAX; self.rooms.len()];
        let mut queue = VecDeque::from([from]);
        distance[self.index(from)] = 0;
        while let Some(cell) = queue.pop_front() {
            for side in Side::ALL {
                if !self.rooms[self.index(cell)].doors[side_index(side)] {
                    continue;
                }
                if let Some(next) = self.neighbour(cell, side)
                    && distance[self.index(next)] == usize::MAX
                {
                    distance[self.index(next)] = distance[self.index(cell)] + 1;
                    queue.push_back(next);
                }
            }
        }
        distance
    }

    /// The rooms on the way from one cell to another, both included.
    #[must_use]
    pub fn cell_path(&self, from: Cell, to: Cell) -> Vec<Cell> {
        let distance = self.distances(to);
        let mut path = vec![from];
        let mut at = from;
        while at != to {
            let here = distance[self.index(at)];
            if here == usize::MAX {
                break;
            }
            let next = Side::ALL
                .iter()
                .filter(|side| self.rooms[self.index(at)].doors[side_index(**side)])
                .filter_map(|side| self.neighbour(at, *side))
                .find(|n| distance[self.index(*n)] + 1 == here);
            match next {
                Some(next) => {
                    path.push(next);
                    at = next;
                }
                None => break,
            }
        }
        path
    }

    /// The middle of the doorway between two neighbouring cells.
    #[must_use]
    pub fn doorway(a: Cell, b: Cell) -> V2 {
        (centre(a) + centre(b)) * 0.5
    }

    /// Rooms that open onto this one.
    #[must_use]
    pub fn neighbours(&self, cell: Cell) -> Vec<Cell> {
        Side::ALL
            .iter()
            .filter(|side| self.rooms[self.index(cell)].doors[side_index(**side)])
            .filter_map(|side| self.neighbour(cell, *side))
            .collect()
    }

    /// Waypoints for a body of `radius` from `from` to `to`: through the
    /// doorways, by way of a room's centre when furniture is in the way.
    #[must_use]
    pub fn route(&self, from: V2, to: V2, radius: f32) -> Vec<V2> {
        let cells = self.cell_path(self.cell_of(from), self.cell_of(to));
        let mut points = Vec::new();
        let mut last = from;
        let push = |points: &mut Vec<V2>, last: &mut V2, next: V2, room: Cell| {
            if first_hit(&self.blocks, *last, next, radius, 0.0).is_some() {
                let c = centre(room);
                if c.distance(*last) > 0.2 {
                    points.push(c);
                    *last = c;
                }
            }
            points.push(next);
            *last = next;
        };
        for pair in cells.windows(2) {
            push(
                &mut points,
                &mut last,
                Self::doorway(pair[0], pair[1]),
                pair[0],
            );
        }
        push(&mut points, &mut last, to, self.cell_of(to));
        points
    }

    /// Whether `p` is open floor for a body of `radius`.
    #[must_use]
    pub fn open(&self, p: V2, radius: f32) -> bool {
        self.blocks.iter().all(|b| !b.grown(radius).contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: f32 = 0.35;

    #[test]
    fn every_module_keeps_its_lanes_and_spots_reachable() {
        for module in Module::ALL.into_iter().chain([Module::Backroom]) {
            let def = module.def();
            // A spare closet, when a house asks for one, keeps them too.
            let props: Vec<Prop> = def.props.iter().copied().chain(def.spare_closet).collect();
            let blocks: Vec<Block> = props
                .iter()
                .map(|p| Block {
                    min: p.min,
                    max: p.max,
                    height: p.height,
                })
                .collect();
            let mut spots: Vec<V2> = def.key_slots.to_vec();
            spots.extend(props.iter().filter_map(|p| p.use_at));
            spots.extend([v(0.0, -4.8), v(4.8, 0.0), v(0.0, 4.8), v(-4.8, 0.0)]);
            for spot in spots {
                assert!(
                    first_hit(&blocks, V2::ZERO, spot, BODY, 0.0).is_none(),
                    "{module:?}: {spot:?} is blocked from the centre"
                );
            }
            for p in &props {
                assert!(p.min.x >= -IN && p.max.x <= IN && p.min.z >= -IN && p.max.z <= IN);
                if let Some(at) = p.use_at {
                    let block = Block {
                        min: p.min,
                        max: p.max,
                        height: p.height,
                    };
                    assert!(block.closest(at).distance(at) < 1.2, "{module:?} {at:?}");
                }
            }
            assert!(def.key_slots.len() >= 3, "{module:?}");
        }
    }

    #[test]
    fn a_house_has_twelve_or_more_rooms_from_six_or_more_modules() {
        for seed in 0..50 {
            let house = House::new(seed);
            assert!(house.rooms.len() >= 12);
            let mut kinds: Vec<Module> = house.rooms.iter().map(|r| r.module).collect();
            kinds.sort_by_key(|m| *m as usize);
            kinds.dedup();
            assert!(kinds.len() >= 6, "seed {seed}: {kinds:?}");
            // Backrooms-style: some module repeats.
            assert!(kinds.len() < house.rooms.len());
            assert_eq!(house.rooms[house.spawn_room].module, Module::Bedroom);
        }
    }

    #[test]
    fn every_room_is_reachable_and_the_keys_are_in_three_rooms() {
        for seed in 0..50 {
            let house = House::new(seed);
            let from = house.rooms[house.spawn_room].cell;
            assert!(house.distances(from).iter().all(|d| *d != usize::MAX));
            assert_eq!(house.keys.len(), KEYS);
            let mut rooms: Vec<Cell> = house.keys.iter().map(|k| cell_of(*k)).collect();
            rooms.dedup();
            rooms.sort_unstable();
            rooms.dedup();
            assert_eq!(rooms.len(), KEYS, "seed {seed}");
            for key in &house.keys {
                assert!(house.open(*key, BODY), "seed {seed}: key {key:?}");
                assert_ne!(cell_of(*key), from);
            }
            assert!(house.open(house.spawn, BODY));
            assert!(house.open(house.front.stand, BODY));
            assert!(house.open(house.man_start, BODY));
            let steps = house.distances(from)[house.room_of(house.man_start)];
            assert!(steps >= 3, "seed {seed}: he starts {steps} rooms away");
        }
    }

    #[test]
    fn the_front_door_is_in_an_outer_wall_far_from_the_bedroom() {
        for seed in 0..50 {
            let house = House::new(seed);
            let cell = house.rooms[house.front.room].cell;
            assert!(house.neighbour(cell, house.front.side).is_none());
            let d = house.distances(house.rooms[house.spawn_room].cell);
            assert!(d[house.front.room] >= 2, "seed {seed}");
            // The door sits in a solid wall.
            assert!(
                house
                    .blocks
                    .iter()
                    .any(|b| b.height >= WALL_HEIGHT && b.contains(house.front.at))
            );
        }
    }

    #[test]
    fn the_same_seed_builds_the_same_house_and_others_differ() {
        assert_eq!(House::new(42), House::new(42));
        let differ = (1..10).filter(|s| House::new(*s) != House::new(42)).count();
        assert_eq!(differ, 9);
    }

    #[test]
    fn routes_reach_every_key_through_open_floor() {
        for seed in 0..30 {
            let house = House::new(seed);
            for target in house.keys.iter().chain([&house.front.stand]) {
                let route = house.route(house.spawn, *target, BODY);
                let mut last = house.spawn;
                for point in &route {
                    assert!(
                        first_hit(&house.blocks, last, *point, BODY - 0.05, 0.0).is_none(),
                        "seed {seed}: {last:?} to {point:?}"
                    );
                    last = *point;
                }
                assert_eq!(route.last(), Some(target));
            }
        }
    }

    #[test]
    fn fridges_and_closets_are_in_the_house() {
        let mut fridges = 0;
        let mut closets = 0;
        for seed in 0..20 {
            let house = House::new(seed);
            fridges += house.fridges.len();
            closets += house.closets.len();
            assert!(!house.closets.is_empty(), "the bedroom has a closet");
            for (spot, _) in &house.fridges {
                assert!(house.open(spot.at, BODY));
            }
        }
        assert!(fridges >= 20 && closets >= 20);
    }

    #[test]
    fn extra_closets_add_hiding_spots_and_keep_the_house() {
        for seed in 0..30 {
            let plain = House::new(seed);
            let house = House::tuned(seed, 2);
            assert_eq!(house.closets.len(), plain.closets.len() + 2, "seed {seed}");
            assert_eq!(house.extras.len(), 2);
            assert_eq!(house.keys, plain.keys);
            assert_eq!(house.front, plain.front);
            assert_eq!(House::tuned(seed, 0), plain);
            for spot in &house.closets {
                assert!(house.open(spot.at, BODY), "seed {seed}: {spot:?}");
            }
            for target in house.keys.iter().chain([&house.front.stand]) {
                let route = house.route(house.spawn, *target, BODY);
                let mut last = house.spawn;
                for point in &route {
                    assert!(
                        first_hit(&house.blocks, last, *point, BODY - 0.05, 0.0).is_none(),
                        "seed {seed}: {last:?} to {point:?}"
                    );
                    last = *point;
                }
            }
            let (room, _) = house.extras[0];
            assert_eq!(
                house.props(room).len(),
                house.rooms[room].module.def().props.len() + 1
            );
        }
        // Every house has room for the most a document may ask for.
        for seed in 0..100 {
            let house = House::tuned(seed, crate::tuning::MAX_EXTRA_CLOSETS as usize);
            assert_eq!(
                house.extras.len(),
                crate::tuning::MAX_EXTRA_CLOSETS as usize,
                "seed {seed}"
            );
        }
    }

    #[test]
    fn a_backrooms_house_is_mostly_empty_yellow_rooms_and_still_works() {
        for seed in 0..50 {
            let house = House::build(seed, 10, true);
            let backrooms = house
                .rooms
                .iter()
                .filter(|r| r.module == Module::Backroom)
                .count();
            assert!(backrooms >= 8, "seed {seed}: {backrooms}");
            assert_eq!(house.rooms[house.spawn_room].module, Module::Bedroom);
            let from = house.rooms[house.spawn_room].cell;
            assert!(house.distances(from).iter().all(|d| *d != usize::MAX));
            assert_eq!(house.keys.len(), KEYS);
            let doors = |h: &House| {
                h.rooms
                    .iter()
                    .map(|r| r.doors.iter().filter(|d| **d).count())
                    .sum::<usize>()
            };
            assert!(
                doors(&house) > doors(&House::new(seed)),
                "seed {seed}: more doorways"
            );
            for target in house.keys.iter().chain([&house.front.stand]) {
                let route = house.route(house.spawn, *target, BODY);
                let mut last = house.spawn;
                for point in &route {
                    assert!(
                        first_hit(&house.blocks, last, *point, BODY - 0.05, 0.0).is_none(),
                        "seed {seed}: {last:?} to {point:?}"
                    );
                    last = *point;
                }
            }
            // Room for the most spare closets a document may ask for.
            let mut tuning = crate::tuning::Tuning::default();
            tuning.backrooms = 1;
            tuning.extra_closets = crate::tuning::MAX_EXTRA_CLOSETS;
            let closets = House::of(seed, &tuning);
            assert_eq!(
                closets.extras.len(),
                crate::tuning::MAX_EXTRA_CLOSETS as usize
            );
        }
        // Off, the house is the same as ever.
        assert_eq!(
            House::build(5, crate::tuning::AIRBNB_LOOPS as usize, false),
            House::new(5)
        );
    }

    #[test]
    fn a_bigger_house_spreads_its_keys_to_the_far_ends() {
        let tuning = crate::tuning::Tuning::default();
        for seed in 0..30 {
            let house = House::of(seed, &tuning);
            assert_eq!((house.cols, house.rows), (7, 7));
            assert_eq!(house.rooms.len(), 49);
            let from = house.rooms[house.spawn_room].cell;
            let steps = house.distances(from);
            assert!(steps.iter().all(|d| *d != usize::MAX), "seed {seed}");
            assert_eq!(house.keys.len(), KEYS);
            // The last key is in the cone cabinet.
            assert_eq!(house.cabinet_key, Some(KEYS - 1));
            assert_eq!(house.keys[KEYS - 1], house.cabinet);
            let mut rooms: Vec<usize> = house.keys.iter().map(|k| house.room_of(*k)).collect();
            rooms.extend(house.buttons.iter().map(|b| house.room_of(*b)));
            rooms.sort_unstable();
            rooms.dedup();
            assert_eq!(
                rooms.len(),
                KEYS + BUTTONS,
                "seed {seed}: every stop in its own room"
            );
            for key in &house.keys {
                assert!(house.open(*key, BODY), "seed {seed}: key {key:?}");
                assert!(
                    steps[house.room_of(*key)] >= 3,
                    "seed {seed}: a key near the bedroom"
                );
            }
            assert!(house.open(house.front.stand, BODY));
            assert!(house.open(house.man_start, BODY));
            for target in house
                .keys
                .iter()
                .chain(&house.buttons)
                .chain([&house.front.stand])
            {
                let route = house.route(house.spawn, *target, BODY);
                let mut last = house.spawn;
                for point in &route {
                    assert!(
                        first_hit(&house.blocks, last, *point, BODY - 0.05, 0.0).is_none(),
                        "seed {seed}: {last:?} to {point:?}"
                    );
                    last = *point;
                }
                assert_eq!(route.last(), Some(target));
            }
            // A house of any size points to its own rooms.
            let far = v(1000.0, 1000.0);
            assert_eq!(house.room_of(far), house.rooms.len() - 1);
        }
        // Spread keys are farther than shuffled ones, over many houses.
        let shape = |spread| Shape {
            size: 7,
            loops: 7,
            backrooms: true,
            spread,
            puzzle_key: false,
        };
        let total = |spread| {
            (0..30)
                .map(|seed| {
                    let house = House::generate(seed, shape(spread));
                    let steps = house.distances(house.rooms[house.spawn_room].cell);
                    house
                        .keys
                        .iter()
                        .map(|k| steps[house.room_of(*k)])
                        .sum::<usize>()
                })
                .sum::<usize>()
        };
        assert!(
            total(true) > total(false) + 30,
            "{} {}",
            total(true),
            total(false)
        );
    }

    #[test]
    fn the_spear_puzzle_sits_in_four_rooms_on_open_floor() {
        for seed in 0..50 {
            for house in [House::new(seed), House::build(seed, 10, true)] {
                assert_eq!(house.buttons.len(), BUTTONS);
                let mut rooms: Vec<Cell> = house
                    .buttons
                    .iter()
                    .chain([&house.cabinet])
                    .map(|p| cell_of(*p))
                    .collect();
                rooms.sort_unstable();
                rooms.dedup();
                assert_eq!(rooms.len(), BUTTONS + 1, "seed {seed}");
                for spot in house.buttons.iter().chain([&house.cabinet]) {
                    assert!(house.open(*spot, BODY), "seed {seed}: {spot:?}");
                    assert_ne!(cell_of(*spot), house.rooms[house.spawn_room].cell);
                    assert!(house.keys.iter().all(|k| k.distance(*spot) > 1.0));
                }
            }
        }
    }
}
