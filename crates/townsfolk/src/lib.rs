//! Verse's townsfolk (`docs/verse/generative-agents.md`, item 5, phase E1):
//! villagers as data, with routines that are a pure function of the town
//! clock and a seed.
//!
//! - A villager is one definition, `openagents.verse-npc.v1` ([`Npc`]): an
//!   ID, a name, a character card that labels it a character, a look, a
//!   home and a workplace in the world tree, a routine table of
//!   (time, node, activity) rows, and a few fixed lines.
//! - The town is one roster, `openagents.verse-town.v1` ([`Town`]): the
//!   seed every device jitters routines with, the [`Budgets`], and the
//!   admitted definitions by ID and digest. A client loads only admitted
//!   definitions whose digest matches ([`Roster::load`]).
//! - [`validate`] checks a definition against a [`world_tree::Tree`] and
//!   reports typed [`Problem`]s that name the field. Routes come from a
//!   [`validate::Router`] the zone supplies, and text passes a
//!   [`validate::Screen`] such as the secret screen.
//! - [`routine`] places a villager at a town time: standing at a node, or
//!   walking between two with its progress; [`routine::gatherings`] says
//!   who stands together, which rumors (phase E2) travel by.
//! - [`sim`] renders a day as text, and [`files`] keeps the checked-in
//!   directory: proposals anyone may stage, and the roster only the owner
//!   changes ([`files::admit`]).
//!
//! The crate depends on serde, SHA-256, the town clock, and the world tree
//! only, so the web build, the phones, and the command line read it alike.

pub mod files;
pub mod routine;
pub mod sim;
pub mod validate;

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use world_tree::Affordance;

pub use routine::{Placement, Roster, Villager};

/// A villager definition's schema.
pub const NPC_SCHEMA: &str = "openagents.verse-npc.v1";
/// The town roster's schema.
pub const TOWN_SCHEMA: &str = "openagents.verse-town.v1";
/// A staged proposal's schema ([`files::Proposal`]).
pub const PROPOSAL_SCHEMA: &str = "openagents.verse-town-proposal.v1";

/// The longest villager ID, bytes of `[a-z0-9-]`.
pub const MAX_ID: usize = 40;
/// The longest name, characters.
pub const MAX_NAME: usize = 24;
/// The longest character role, such as `baker`, characters.
pub const MAX_ROLE: usize = 40;
/// The longest character summary, characters.
pub const MAX_ABOUT: usize = 280;
/// The longest fixed line, characters.
pub const MAX_LINE: usize = 160;

/// The town's budgets. The roster sets them, and none may pass
/// [`Budgets::CEILING`], which only a code change raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    /// Admitted villagers.
    pub villagers: u32,
    /// Routine rows per villager.
    pub routine_rows: u32,
    /// Fixed lines per villager.
    pub lines: u32,
    /// Rumors traveling at once (phase E2).
    pub rumors_in_flight: u32,
    /// Model-written villager replies per player per town day (phase E2).
    pub replies_per_player_per_day: u32,
}

impl Budgets {
    /// The hard ceilings.
    pub const CEILING: Self = Self {
        villagers: 40,
        routine_rows: 24,
        lines: 12,
        rumors_in_flight: 32,
        replies_per_player_per_day: 50,
    };

    /// Each budget by name with its value and ceiling.
    #[must_use]
    pub fn named(&self) -> [(&'static str, u32, u32); 5] {
        let c = Self::CEILING;
        [
            ("villagers", self.villagers, c.villagers),
            ("routine_rows", self.routine_rows, c.routine_rows),
            ("lines", self.lines, c.lines),
            (
                "rumors_in_flight",
                self.rumors_in_flight,
                c.rumors_in_flight,
            ),
            (
                "replies_per_player_per_day",
                self.replies_per_player_per_day,
                c.replies_per_player_per_day,
            ),
        ]
    }
}

impl Default for Budgets {
    fn default() -> Self {
        Self::CEILING
    }
}

/// What a villager does in a routine row: a closed vocabulary, each fitting
/// the world tree's affordances ([`Activity::fits`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activity {
    Sleep,
    Eat,
    Drink,
    Bake,
    Smith,
    Craft,
    Tend,
    Work,
    Sell,
    Shop,
    Read,
    Pray,
    RingBell,
    Gather,
    Rest,
}

impl Activity {
    pub const ALL: [Self; 15] = [
        Self::Sleep,
        Self::Eat,
        Self::Drink,
        Self::Bake,
        Self::Smith,
        Self::Craft,
        Self::Tend,
        Self::Work,
        Self::Sell,
        Self::Shop,
        Self::Read,
        Self::Pray,
        Self::RingBell,
        Self::Gather,
        Self::Rest,
    ];

    /// The wire name, such as `ring-bell`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Eat => "eat",
            Self::Drink => "drink",
            Self::Bake => "bake",
            Self::Smith => "smith",
            Self::Craft => "craft",
            Self::Tend => "tend",
            Self::Work => "work",
            Self::Sell => "sell",
            Self::Shop => "shop",
            Self::Read => "read",
            Self::Pray => "pray",
            Self::RingBell => "ring-bell",
            Self::Gather => "gather",
            Self::Rest => "rest",
        }
    }

    /// The word a nameplate shows, such as `ringing the bell`.
    #[must_use]
    pub const fn doing(self) -> &'static str {
        match self {
            Self::Sleep => "sleeping",
            Self::Eat => "eating",
            Self::Drink => "drinking",
            Self::Bake => "baking",
            Self::Smith => "at the forge",
            Self::Craft => "crafting",
            Self::Tend => "tending",
            Self::Work => "working",
            Self::Sell => "selling",
            Self::Shop => "shopping",
            Self::Read => "reading",
            Self::Pray => "praying",
            Self::RingBell => "ringing the bell",
            Self::Gather => "chatting",
            Self::Rest => "resting",
        }
    }

    /// The affordances a node may offer for this activity; a row's node
    /// must offer one.
    #[must_use]
    pub const fn fits(self) -> &'static [Affordance] {
        use Affordance as A;
        match self {
            Self::Sleep => &[A::Sleep],
            Self::Eat => &[A::Eat],
            Self::Drink => &[A::Drink],
            Self::Bake => &[A::Work, A::BuyBread],
            Self::Smith | Self::Craft | Self::Tend | Self::Work => &[A::Work],
            Self::Sell => &[A::Sell, A::Shop, A::BuyBread],
            Self::Shop => &[A::Shop, A::BuyBread],
            Self::Read => &[A::Read],
            Self::Pray | Self::RingBell => &[A::Pray],
            Self::Gather => &[A::Gather, A::Rest, A::Eat, A::Drink],
            Self::Rest => &[A::Rest, A::Gather],
        }
    }

    /// The activity named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The affordances a workplace offers one of.
pub const WORK_AFFORDANCES: [Affordance; 7] = [
    Affordance::Work,
    Affordance::Sell,
    Affordance::Shop,
    Affordance::BuyBread,
    Affordance::Pray,
    Affordance::Read,
    Affordance::Teach,
];

/// A villager's character card: it labels the villager a character, never
/// a person or a working agent, and carries what a reply (phase E2) is
/// written from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    /// Always `true`: townsfolk are labeled characters.
    pub character: bool,
    /// What the villager does in town, such as `baker`.
    pub role: String,
    /// A short description in the third person.
    pub about: String,
}

/// How a villager looks: the outfit's color, each channel 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    pub tint: [f32; 3],
}

impl Default for Look {
    fn default() -> Self {
        Self {
            tint: [0.55, 0.45, 0.35],
        }
    }
}

/// One routine row: from `at`, the villager goes to `node` and does
/// `activity` there until the next row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Town time, `HH:MM` on a 24-hour clock.
    pub at: String,
    /// A world-tree node ID: a building, a room, or an object.
    pub node: String,
    pub activity: Activity,
}

impl Row {
    /// Seconds into the town day, when `at` is `HH:MM`.
    #[must_use]
    pub fn second(&self) -> Option<u32> {
        parse_time(&self.at)
    }
}

/// `HH:MM` as seconds into the day.
#[must_use]
pub fn parse_time(text: &str) -> Option<u32> {
    let (h, m) = text.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 3_600 + m * 60)
}

/// Seconds into the day as `HH:MM`.
#[must_use]
pub fn clock_text(second: f64) -> String {
    let s = second.rem_euclid(86_400.0) as u32;
    format!("{:02}:{:02}", s / 3_600, (s / 60) % 60)
}

/// A villager definition, `openagents.verse-npc.v1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Npc {
    pub schema: String,
    /// Stable ID, `[a-z0-9-]`, which also names its file.
    pub id: String,
    /// The name its nameplate shows.
    pub name: String,
    pub card: Card,
    #[serde(default)]
    pub look: Look,
    /// Where it sleeps: a node offering `sleep`.
    pub home: String,
    /// Where it works: a node offering one of [`WORK_AFFORDANCES`].
    pub workplace: String,
    /// The day, sorted by time, starting at `00:00`.
    pub routine: Vec<Row>,
    /// Fixed lines it may say: any time, or at a quest step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<Line>,
    /// Mixed into the town's seed for this villager's delays and standing
    /// offsets; zero when absent.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub seed: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// The longest quest step a line is keyed by, bytes.
pub const MAX_STEP: usize = 64;

/// A fixed line: plain text said any time, or an object with the quest
/// step it is said at, such as `{"step": "apprentice-road/2", "text": ...}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Line {
    Any(String),
    Step { step: String, text: String },
}

impl Line {
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Any(text) | Self::Step { text, .. } => text,
        }
    }

    /// The quest step it is said at, if it is keyed by one.
    #[must_use]
    pub fn step(&self) -> Option<&str> {
        match self {
            Self::Any(_) => None,
            Self::Step { step, .. } => Some(step),
        }
    }
}

impl From<&str> for Line {
    fn from(text: &str) -> Self {
        Self::Any(text.to_owned())
    }
}

impl From<String> for Line {
    fn from(text: String) -> Self {
        Self::Any(text)
    }
}

impl Npc {
    /// Reads a definition.
    ///
    /// # Errors
    ///
    /// When the text isn't a definition: unknown fields, a missing field,
    /// or an activity outside the vocabulary.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// `sha256:` and the hex SHA-256 of the definition as compact JSON, in
    /// field order: what the roster admits.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_of(self)
    }

    /// The definition as pretty JSON ending in a newline.
    #[must_use]
    pub fn to_json(&self) -> String {
        pretty(self)
    }
}

/// One admitted definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admitted {
    pub id: String,
    pub digest: String,
}

/// The town roster, `openagents.verse-town.v1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Town {
    pub schema: String,
    /// The world tree's zone, such as `everglade`.
    pub zone: String,
    /// What every device jitters routines with.
    pub seed: u64,
    pub budgets: Budgets,
    /// The admitted definitions, by ID.
    pub admitted: Vec<Admitted>,
}

impl Town {
    /// An empty roster for `zone`.
    #[must_use]
    pub fn new(zone: &str, seed: u64) -> Self {
        Self {
            schema: TOWN_SCHEMA.into(),
            zone: zone.into(),
            seed,
            budgets: Budgets::default(),
            admitted: Vec::new(),
        }
    }

    /// Reads a roster.
    ///
    /// # Errors
    ///
    /// When the text isn't a roster.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        pretty(self)
    }

    /// The admitted entry for `id`.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&Admitted> {
        self.admitted.iter().find(|a| a.id == id)
    }

    /// Checks the schema, the budgets against their ceilings, and the
    /// admitted count against its budget.
    #[must_use]
    pub fn problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        if self.schema != TOWN_SCHEMA {
            out.push(Problem::new(
                "schema",
                Code::Schema,
                format!("is {:?}, not {TOWN_SCHEMA}", self.schema),
            ));
        }
        for (name, value, ceiling) in self.budgets.named() {
            if value > ceiling {
                out.push(Problem::new(
                    format!("budgets.{name}"),
                    Code::Budget,
                    format!("is {value}, over the ceiling of {ceiling}"),
                ));
            }
        }
        let count = self.admitted.len();
        if count > self.budgets.villagers as usize {
            out.push(Problem::new(
                "admitted",
                Code::Budget,
                format!(
                    "holds {count} villagers, over the budget of {}",
                    self.budgets.villagers
                ),
            ));
        }
        for (i, a) in self.admitted.iter().enumerate() {
            if self.admitted[..i].iter().any(|b| b.id == a.id) {
                out.push(Problem::new(
                    format!("admitted[{i}].id"),
                    Code::Duplicate,
                    format!("{} is admitted twice", a.id),
                ));
            }
        }
        out
    }
}

/// What kind of problem a [`Problem`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// The schema field is wrong.
    Schema,
    /// An ID isn't `[a-z0-9-]` of the right length, or doesn't match its
    /// file.
    Id,
    /// Text is empty, too long, or outside the allowed characters.
    Text,
    /// The card doesn't label the villager a character.
    Character,
    /// A color channel isn't 0 to 1.
    Tint,
    /// A node isn't in the world tree, or is a district or the zone.
    Node,
    /// A node doesn't offer an affordance the field needs.
    Affordance,
    /// A row's time isn't `HH:MM`, the rows aren't sorted, or the day
    /// doesn't start at `00:00`.
    Time,
    /// The routine never sleeps at home or never works at the workplace.
    Coverage,
    /// A walk doesn't finish before the next row starts.
    Walk,
    /// A leg has no route around the zone's blockers.
    Route,
    /// Two villagers book one exclusive object at once.
    Exclusive,
    /// Text holds something the secret screen refuses.
    Secret,
    /// Over a budget or a ceiling.
    Budget,
    /// The digest isn't the admitted one.
    Digest,
    /// The definition isn't admitted.
    NotAdmitted,
    /// The same ID twice.
    Duplicate,
    /// An admitted definition has no file.
    Missing,
}

impl Code {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::Id => "id",
            Self::Text => "text",
            Self::Character => "character",
            Self::Tint => "tint",
            Self::Node => "node",
            Self::Affordance => "affordance",
            Self::Time => "time",
            Self::Coverage => "coverage",
            Self::Walk => "walk",
            Self::Route => "route",
            Self::Exclusive => "exclusive",
            Self::Secret => "secret",
            Self::Budget => "budget",
            Self::Digest => "digest",
            Self::NotAdmitted => "not-admitted",
            Self::Duplicate => "duplicate",
            Self::Missing => "missing",
        }
    }
}

/// One thing wrong with a definition or the roster: the field, as a path
/// such as `routine[3].node`, its [`Code`], and a message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub field: String,
    pub code: Code,
    pub message: String,
}

impl Problem {
    #[must_use]
    pub fn new(field: impl Into<String>, code: Code, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code,
            message: message.into(),
        }
    }

    /// The same problem under a villager's ID, such as
    /// `mira-baker: routine[3].node`.
    #[must_use]
    pub fn of(mut self, id: &str) -> Self {
        self.field = format!("{id}: {}", self.field);
        self
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({})",
            self.field,
            self.message,
            self.code.as_str()
        )
    }
}

/// Whether `id` is 1 to [`MAX_ID`] bytes of `[a-z0-9-]`, not starting or
/// ending with `-`.
#[must_use]
pub fn good_id(id: &str) -> bool {
    (1..=MAX_ID).contains(&id.len())
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn digest_of<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("plain data serializes");
    let hash = Sha256::digest(&bytes);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

fn pretty<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("plain data serializes");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests;
