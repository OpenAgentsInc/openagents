//! The `haunt.v1` document (`docs/verse/games/the-haunted-house.md`, "The
//! house document").
//!
//! Conventions: metres; +x east, +z north, y up; `yaw` in degrees, 0 facing
//! +z (north), 90 facing +x (east). A rectangle is `[x, z, width, depth]`
//! from its south-west corner. A prop's `at` and the arrival's `at` are
//! relative to their area's south-west corner, so moving an area moves its
//! contents. Assets, sounds, surfaces, presets and colours are named by id
//! from the library (`haunt-assets`), never by file path.

use serde::{Deserialize, Serialize};

/// The document kind.
pub const KIND: &str = "haunt.v1";
/// A door that leads out of the house rather than into another area.
pub const OUTSIDE: &str = "outside";

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

/// The whole house.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct House {
    pub kind: String,
    pub title: String,
    pub arrival: Arrival,
    pub exit: Exit,
    pub look: Look,
    pub audio: Audio,
    pub areas: Vec<Area>,
}

/// Where a player starts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arrival {
    pub area: String,
    /// Relative to the area's south-west corner, m.
    pub at: [f32; 2],
    /// Degrees; 180 faces south.
    pub yaw: f32,
    /// What plays behind the player about 2 s after arrival (a cue id).
    pub cue: String,
}

/// The way out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub area: String,
    pub door: String,
}

/// The view's global look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// How strongly the edges of the view darken to plum, 0 to 1.
    pub vignette: f32,
    /// Whether the player carries a lantern.
    pub lantern: bool,
    /// The paper grain's strength, 0 to 1.
    pub grain: f32,
    pub sky: String,
}

/// The five audio buses, each 0 to 1, and the music.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audio {
    pub master: f32,
    pub music: f32,
    pub ambience: f32,
    pub sfx: f32,
    #[serde(default = "one")]
    pub voice: f32,
    pub music_track: String,
}

/// What an area is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaKind {
    /// Outdoors, open to the sky.
    Yard,
    Hallway,
    Room,
}

impl AreaKind {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Yard => "yard",
            Self::Hallway => "hallway",
            Self::Room => "room",
        }
    }
}

/// One space of the house.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub id: String,
    pub kind: AreaKind,
    /// `[x, z, width, depth]` from the south-west corner, m.
    pub rect: [f32; 4],
    /// The ceiling's height, m; none outdoors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling: Option<f32>,
    pub surfaces: Surfaces,
    pub light: Light,
    pub sound: AreaSound,
    pub intent: Intent,
    #[serde(default)]
    pub doors: Vec<Door>,
    #[serde(default)]
    pub props: Vec<Prop>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
}

/// Surface ids from the library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surfaces {
    pub floor: String,
    pub walls: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<String>,
}

/// An area's light: a preset with optional overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub preset: String,
    /// How much darker than the preset, 0 to 1.
    #[serde(default)]
    pub dim: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<Ambient>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<PointLight>,
}

/// An ambient override.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ambient {
    /// A palette token.
    pub color: String,
    /// 0 to 1.
    pub level: f32,
}

/// A point light, relative to the area's south-west corner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointLight {
    pub at: [f32; 2],
    pub height: f32,
    /// A palette token.
    pub color: String,
    /// 0 to 2.
    pub intensity: f32,
    /// m.
    pub radius: f32,
    /// How much it flickers, 0 to 1.
    #[serde(default)]
    pub flicker: f32,
}

/// An area's sound.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaSound {
    /// An ambience bed id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed: Option<String>,
    /// The music's loudness here, 0 to 2.
    #[serde(default = "one")]
    pub music_gain: f32,
}

/// What an area is meant to make the player feel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub feeling: String,
    #[serde(default)]
    pub beat: String,
    /// On entering and at its peak, 0 to 5.
    pub intensity: [u8; 2],
}

/// A side of an area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wall {
    North,
    East,
    South,
    West,
}

impl Wall {
    pub const ALL: [Wall; 4] = [Wall::North, Wall::East, Wall::South, Wall::West];

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::East => "east",
            Self::South => "south",
            Self::West => "west",
        }
    }

    #[must_use]
    pub fn opposite(self) -> Wall {
        match self {
            Self::North => Self::South,
            Self::South => Self::North,
            Self::East => Self::West,
            Self::West => Self::East,
        }
    }

    /// The yaw of a player walking out through this wall.
    #[must_use]
    pub fn outward_yaw(self) -> f32 {
        match self {
            Self::North => 0.0,
            Self::East => 90.0,
            Self::South => 180.0,
            Self::West => 270.0,
        }
    }

    /// Whether the wall runs east to west (north and south walls).
    #[must_use]
    pub fn along_x(self) -> bool {
        matches!(self, Self::North | Self::South)
    }

    /// Reads `north`, `east`, `south` or `west`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Wall> {
        Wall::ALL.into_iter().find(|w| w.word() == word)
    }
}

/// A door in one of an area's walls. Both areas it joins list it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Door {
    pub id: String,
    /// The area on the other side, or `outside`.
    pub to: String,
    pub wall: Wall,
    /// From the wall's west end (north and south walls) or south end (east
    /// and west walls), m.
    pub offset: f32,
    pub width: f32,
}

/// A library asset placed in an area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub id: String,
    pub asset: String,
    /// Relative to the area's south-west corner, m.
    pub at: [f32; 2],
    /// Which way it faces, degrees.
    #[serde(default)]
    pub yaw: f32,
    /// Height above the floor, for something on a counter or a wall, m.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub y: f32,
    /// One of the asset's states; its first when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Why it is here, for the next reader.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The words on a sign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

/// When a trigger fires. Each fires once per visit to its area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum When {
    /// The player enters the area.
    Enter(bool),
    /// The player reaches the middle of the area.
    EnterMiddle(bool),
    /// The player comes within `within` metres of a prop.
    Near { prop: String, within: f32 },
    /// The player looks at a prop.
    Look { prop: String },
}

/// A timed change of an area's light.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightChange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<f32>,
    /// Seconds; none keeps it.
    #[serde(default, rename = "for", skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
}

/// A prop's new state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateChange {
    pub prop: String,
    pub to: String,
}

/// An effect, at a prop or over the area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectAt {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// One thing a trigger does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Plays an effect sound, from the trigger's prop or the area's middle.
    Sfx(String),
    Light(LightChange),
    State(StateChange),
    Effect(EffectAt),
}

/// A trigger: when, and what.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub when: When,
    #[serde(rename = "do")]
    pub actions: Vec<Action>,
    /// Fires on every visit (true) or only the first (false).
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub every_visit: bool,
}

impl Trigger {
    /// The prop a trigger watches, if any.
    #[must_use]
    pub fn prop(&self) -> Option<&str> {
        match &self.when {
            When::Near { prop, .. } | When::Look { prop } => Some(prop),
            _ => None,
        }
    }
}

impl House {
    /// Reads a document.
    ///
    /// # Errors
    /// It isn't JSON, or not a `haunt.v1` house; the message says why in
    /// one sentence.
    pub fn parse(text: &str) -> Result<House, String> {
        let house: House = serde_json::from_str(text)
            .map_err(|e| format!("The house document doesn't read: {e}."))?;
        if house.kind != KIND {
            return Err(format!(
                "The house document is {:?}, not {KIND}.",
                house.kind
            ));
        }
        Ok(house)
    }

    /// The document as pretty JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    #[must_use]
    pub fn area(&self, id: &str) -> Option<&Area> {
        self.areas.iter().find(|a| a.id == id)
    }

    pub fn area_mut(&mut self, id: &str) -> Option<&mut Area> {
        self.areas.iter_mut().find(|a| a.id == id)
    }

    /// The area and prop a prop id names.
    #[must_use]
    pub fn prop(&self, id: &str) -> Option<(&Area, &Prop)> {
        self.areas
            .iter()
            .find_map(|a| a.props.iter().find(|p| p.id == id).map(|p| (a, p)))
    }

    /// The arrival point in world coordinates (x, z).
    #[must_use]
    pub fn arrival_point(&self) -> Option<[f32; 2]> {
        let area = self.area(&self.arrival.area)?;
        Some(area.world(self.arrival.at))
    }
}

impl Area {
    #[must_use]
    pub fn x0(&self) -> f32 {
        self.rect[0]
    }
    #[must_use]
    pub fn z0(&self) -> f32 {
        self.rect[1]
    }
    #[must_use]
    pub fn x1(&self) -> f32 {
        self.rect[0] + self.rect[2]
    }
    #[must_use]
    pub fn z1(&self) -> f32 {
        self.rect[1] + self.rect[3]
    }
    #[must_use]
    pub fn width(&self) -> f32 {
        self.rect[2]
    }
    #[must_use]
    pub fn depth(&self) -> f32 {
        self.rect[3]
    }
    #[must_use]
    pub fn centre(&self) -> [f32; 2] {
        [
            self.rect[0] + self.rect[2] / 2.0,
            self.rect[1] + self.rect[3] / 2.0,
        ]
    }

    /// A point relative to the south-west corner, in world coordinates.
    #[must_use]
    pub fn world(&self, at: [f32; 2]) -> [f32; 2] {
        [self.rect[0] + at[0], self.rect[1] + at[1]]
    }

    /// Whether a world point (x, z) is inside, with a margin.
    #[must_use]
    pub fn contains(&self, p: [f32; 2], margin: f32) -> bool {
        p[0] >= self.x0() + margin
            && p[0] <= self.x1() - margin
            && p[1] >= self.z0() + margin
            && p[1] <= self.z1() - margin
    }

    /// A wall's line: its fixed coordinate, and where it starts and ends
    /// along the other axis.
    #[must_use]
    pub fn wall_line(&self, wall: Wall) -> (f32, f32, f32) {
        match wall {
            Wall::North => (self.z1(), self.x0(), self.x1()),
            Wall::South => (self.z0(), self.x0(), self.x1()),
            Wall::East => (self.x1(), self.z0(), self.z1()),
            Wall::West => (self.x0(), self.z0(), self.z1()),
        }
    }

    /// How long a wall is, m.
    #[must_use]
    pub fn wall_length(&self, wall: Wall) -> f32 {
        if wall.along_x() {
            self.width()
        } else {
            self.depth()
        }
    }

    /// A door's opening along its wall's axis, in world coordinates.
    #[must_use]
    pub fn door_span(&self, door: &Door) -> (f32, f32) {
        let (_, start, _) = self.wall_line(door.wall);
        (start + door.offset, start + door.offset + door.width)
    }

    /// A door's middle in world coordinates (x, z).
    #[must_use]
    pub fn door_centre(&self, door: &Door) -> [f32; 2] {
        let (fixed, _, _) = self.wall_line(door.wall);
        let (a, b) = self.door_span(door);
        let along = (a + b) / 2.0;
        if door.wall.along_x() {
            [along, fixed]
        } else {
            [fixed, along]
        }
    }

    /// A door's middle stepped `inset` metres into this area.
    #[must_use]
    pub fn door_inside(&self, door: &Door, inset: f32) -> [f32; 2] {
        let [x, z] = self.door_centre(door);
        match door.wall {
            Wall::North => [x, z - inset],
            Wall::South => [x, z + inset],
            Wall::East => [x - inset, z],
            Wall::West => [x + inset, z],
        }
    }
}

/// An id read as words: `long-hall` is "the long hall".
#[must_use]
pub fn name(id: &str) -> String {
    format!("the {}", id.replace(['-', '_'], " "))
}

/// Version 1: the MVP house (the yard, the entry hall, the parlor, the long
/// hall, the kitchen and the back hall).
pub const V1_JSON: &str = include_str!("../houses/v1.json");

/// Version 1, read.
///
/// # Panics
/// The checked-in house doesn't read; its tests catch that.
#[must_use]
pub fn v1() -> House {
    House::parse(V1_JSON).expect("houses/v1.json reads")
}
