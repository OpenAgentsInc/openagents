//! Verse's world tree, `openagents.verse-world-tree.v1`
//! (`docs/verse/generative-agents.md`, item 3): a zone's places as the
//! paper's containment tree, so an agent names a place, never coordinates.
//!
//! ```text
//! zone
//!   district
//!     building
//!       room
//!         object
//! ```
//!
//! Each [`Node`] has a stable, human-readable ID (its parent's ID, a `/`,
//! and a slug of its name), a name, a [`Kind`], a standing point that
//! navigation can reach, and the [`Affordance`]s it offers. An object that
//! one agent uses at a time is `exclusive`. The [`Tree`] carries a SHA-256
//! digest of its content, so a plan or a memory can name the exact tree it
//! was made against.
//!
//! This crate holds the data types, the query API, object states
//! ([`state`]), per-agent known subgraphs ([`known`]), and the place choice
//! an agent grounds an action with ([`choose`]). It depends on serde and
//! SHA-256 only, so `coder` and the web build read it without a renderer.
//! Everglade's tree is generated from its layout tables in
//! `verse-zone-everglade` (`zones::everglade::world_tree`) and checked in
//! here as `data/everglade.json` ([`everglade`]); a test in that crate fails
//! when the snapshot is stale and names the command that regenerates it.

pub mod choose;
pub mod known;
pub mod state;
pub mod text;

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use choose::{Ask, Choose, Descent, descend, options};
pub use known::Known;
pub use state::{Column, Conditions, Family, PylonStatus, State, States, Tier};

/// The tree's schema identifier.
pub const SCHEMA: &str = "openagents.verse-world-tree.v1";

/// The longest slug a node ID's segment may have.
pub const MAX_SLUG: usize = 48;

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The tree's root: a whole zone, such as Everglade.
    Zone,
    District,
    /// A building, or a walled place such as the workshop and its yard.
    Building,
    Room,
    Object,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zone => "zone",
            Self::District => "district",
            Self::Building => "building",
            Self::Room => "room",
            Self::Object => "object",
        }
    }

    /// Whether a node of this kind may stand under one of `parent`.
    #[must_use]
    pub const fn fits_under(self, parent: Self) -> bool {
        matches!(
            (parent, self),
            (Self::Zone, Self::District)
                | (Self::District, Self::Building)
                | (Self::Building, Self::Room | Self::Object)
                | (Self::Room, Self::Object)
        )
    }
}

/// What kind of object an object node is, which decides its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Object {
    /// A doorway: open or closed.
    Door,
    /// A light fixture: lit or dark.
    Lamp,
    /// A standing desk or workstation one agent works at.
    Workstation,
    /// A console where commands run.
    Console,
    /// A lectern or a speaker's place.
    Lectern,
    /// A board to read.
    Board,
    /// The studio's Task Wall, with a count per column.
    TaskWall,
    /// An Agent Studio station, where seats gather for an activity.
    Station,
    /// Benches or a spot where people gather.
    Seats,
    /// A compute pylon's site in the Pylon Field: one machine that serves
    /// work (`nips/openagents/NIP-PYLON.md`, World projection).
    Pylon,
    /// The Wellspring, the pooled capacity the pylons feed.
    Wellspring,
}

impl Object {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Door => "door",
            Self::Lamp => "lamp",
            Self::Workstation => "workstation",
            Self::Console => "console",
            Self::Lectern => "lectern",
            Self::Board => "board",
            Self::TaskWall => "task-wall",
            Self::Station => "station",
            Self::Seats => "seats",
            Self::Pylon => "pylon",
            Self::Wellspring => "wellspring",
        }
    }
}

/// What an agent can do at a node: a small closed vocabulary, so a plan
/// and a routine name the same activities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Affordance {
    Sleep,
    Eat,
    Drink,
    BuyBread,
    Shop,
    Work,
    Read,
    Pray,
    Gather,
    Rest,
    RunCommands,
    Review,
    Approve,
    Sell,
    Teach,
    Practice,
    Plan,
}

impl Affordance {
    pub const ALL: [Self; 17] = [
        Self::Sleep,
        Self::Eat,
        Self::Drink,
        Self::BuyBread,
        Self::Shop,
        Self::Work,
        Self::Read,
        Self::Pray,
        Self::Gather,
        Self::Rest,
        Self::RunCommands,
        Self::Review,
        Self::Approve,
        Self::Sell,
        Self::Teach,
        Self::Practice,
        Self::Plan,
    ];

    /// The affordance's wire name, such as `buy-bread`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Eat => "eat",
            Self::Drink => "drink",
            Self::BuyBread => "buy-bread",
            Self::Shop => "shop",
            Self::Work => "work",
            Self::Read => "read",
            Self::Pray => "pray",
            Self::Gather => "gather",
            Self::Rest => "rest",
            Self::RunCommands => "run-commands",
            Self::Review => "review",
            Self::Approve => "approve",
            Self::Sell => "sell",
            Self::Teach => "teach",
            Self::Practice => "practice",
            Self::Plan => "plan",
        }
    }

    /// The affordance named `name`, its wire name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

impl fmt::Display for Affordance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One place in the tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Stable ID: the parent's ID, a `/`, and this node's slug; the root's
    /// is the zone's name.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// What an object is; only objects have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<Object>,
    /// The district's slug, for a district and everything under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    /// Where an agent stands to be at this node, x and z, m: a point
    /// navigation reaches from the zone's approach.
    pub stand: [f32; 2],
    /// The heading to face there, as the controller's yaw, when it
    /// matters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    /// A room's floor, as an axis-aligned box: min and max, x and z, m.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<[[f32; 2]; 2]>,
    /// A building's or a room's doorway: a point outside it and a point
    /// inside, x and z, m. A route into it crosses straight between them,
    /// since a doorway can be narrower than navigation's grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<[[f32; 2]; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affordances: Vec<Affordance>,
    /// One agent uses it at a time.
    #[serde(default, skip_serializing_if = "is_false")]
    pub exclusive: bool,
    /// A door's fixed state: whether it stands open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<bool>,
    /// Where the node comes from in the layout, such as `door:bakery`,
    /// `station:desks`, or `light:estate:3`.
    pub source: String,
}

impl Node {
    /// The last segment of the ID.
    #[must_use]
    pub fn slug(&self) -> &str {
        self.id.rsplit_once('/').map_or(&self.id, |(_, s)| s)
    }

    /// The parent's ID; `None` for the root.
    #[must_use]
    pub fn parent_id(&self) -> Option<&str> {
        self.id.rsplit_once('/').map(|(p, _)| p)
    }

    /// Whether it offers `affordance`.
    #[must_use]
    pub fn offers(&self, affordance: Affordance) -> bool {
        self.affordances.contains(&affordance)
    }

    /// Whether `[x, z]` lies on a room's floor.
    #[must_use]
    pub fn contains(&self, at: [f32; 2]) -> bool {
        self.area.is_some_and(|[min, max]| {
            (min[0]..=max[0]).contains(&at[0]) && (min[1]..=max[1]).contains(&at[1])
        })
    }
}

/// Why a list of nodes is not a tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeError {
    Empty,
    /// The schema isn't [`SCHEMA`].
    Schema(String),
    /// The first node isn't the zone's root.
    Root(String),
    Duplicate(String),
    /// A segment of the ID isn't 1 to [`MAX_SLUG`] bytes of `[a-z0-9-]`.
    BadId(String),
    /// The parent isn't an earlier node.
    Orphan(String),
    /// The node's kind can't stand under its parent's.
    Nesting(String),
    /// A field is missing, out of place, or not finite.
    Field(String, &'static str),
    /// The recorded digest isn't the content's.
    Digest {
        recorded: String,
        content: String,
    },
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the tree has no nodes"),
            Self::Schema(s) => write!(f, "the schema is {s:?}, not {SCHEMA}"),
            Self::Root(id) => write!(f, "{id} isn't a zone root"),
            Self::Duplicate(id) => write!(f, "{id} appears twice"),
            Self::BadId(id) => write!(f, "{id} isn't a node ID"),
            Self::Orphan(id) => write!(f, "{id}'s parent isn't an earlier node"),
            Self::Nesting(id) => write!(f, "{id}'s kind can't stand under its parent's"),
            Self::Field(id, why) => write!(f, "{id}: {why}"),
            Self::Digest { recorded, content } => {
                write!(
                    f,
                    "the recorded digest {recorded} isn't the content's {content}"
                )
            }
        }
    }
}

impl std::error::Error for TreeError {}

/// A node ID's slug for `name`: lowercase ASCII letters and digits, with
/// a `-` for each run of anything else; apostrophes vanish.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars().filter(|c| *c != '\'' && *c != '\u{2019}') {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn good_segment(s: &str) -> bool {
    (1..=MAX_SLUG).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The digested part of a tree.
#[derive(Serialize)]
struct Body<'a> {
    schema: &'a str,
    zone: &'a str,
    nodes: &'a [Node],
}

/// A tree as it is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    schema: String,
    zone: String,
    digest: String,
    nodes: Vec<Node>,
}

/// A zone's world tree: its nodes in order, each after its parent, and
/// their digest.
#[derive(Clone, Debug)]
pub struct Tree {
    zone: String,
    digest: String,
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    children: Vec<Vec<usize>>,
}

impl PartialEq for Tree {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest && self.nodes == other.nodes
    }
}

impl Serialize for Tree {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Written {
            schema: SCHEMA.into(),
            zone: self.zone.clone(),
            digest: self.digest.clone(),
            nodes: self.nodes.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Tree {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = Written::deserialize(deserializer)?;
        if written.schema != SCHEMA {
            return Err(serde::de::Error::custom(TreeError::Schema(written.schema)));
        }
        let tree = Self::new(&written.zone, written.nodes).map_err(serde::de::Error::custom)?;
        if tree.digest != written.digest {
            return Err(serde::de::Error::custom(TreeError::Digest {
                recorded: written.digest,
                content: tree.digest,
            }));
        }
        Ok(tree)
    }
}

impl Tree {
    /// Checks `nodes` and digests them.
    ///
    /// # Errors
    ///
    /// When the nodes aren't a tree rooted at `zone`: see [`TreeError`].
    pub fn new(zone: &str, nodes: Vec<Node>) -> Result<Self, TreeError> {
        let root = nodes.first().ok_or(TreeError::Empty)?;
        if root.id != zone || root.kind != Kind::Zone || !good_segment(zone) {
            return Err(TreeError::Root(root.id.clone()));
        }
        let mut index: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
        let mut children = vec![Vec::new(); nodes.len()];
        for (i, node) in nodes.iter().enumerate() {
            let id = &node.id;
            if !id.split('/').all(good_segment) {
                return Err(TreeError::BadId(id.clone()));
            }
            if i > 0 {
                let parent = node
                    .parent_id()
                    .and_then(|p| index.get(p).copied())
                    .ok_or_else(|| TreeError::Orphan(id.clone()))?;
                if !node.kind.fits_under(nodes[parent].kind) {
                    return Err(TreeError::Nesting(id.clone()));
                }
                children[parent].push(i);
            }
            check_fields(node)?;
            if index.insert(id.clone(), i).is_some() {
                return Err(TreeError::Duplicate(id.clone()));
            }
        }
        let body = Body {
            schema: SCHEMA,
            zone,
            nodes: &nodes,
        };
        let bytes = serde_json::to_vec(&body).expect("plain data serializes");
        let digest = format!("sha256:{}", hex(&Sha256::digest(&bytes)));
        Ok(Self {
            zone: zone.to_owned(),
            digest,
            nodes,
            index,
            children,
        })
    }

    /// Reads a tree from its JSON and checks its digest.
    ///
    /// # Errors
    ///
    /// When the text isn't a tree or its digest isn't its content's.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// The tree as pretty JSON, one field a line, ending in a newline: the
    /// checked-in snapshot's form.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("plain data serializes");
        text.push('\n');
        text
    }

    #[must_use]
    pub fn zone(&self) -> &str {
        &self.zone
    }

    /// `sha256:` and the hex SHA-256 of the schema, the zone, and the
    /// nodes as compact JSON.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Every node, each after its parent.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    #[must_use]
    pub fn root(&self) -> &Node {
        &self.nodes[0]
    }

    /// The node with ID `id`.
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.index.get(id).map(|&i| &self.nodes[i])
    }

    /// Where an agent stands for node `id`, and the heading to face.
    #[must_use]
    pub fn stand(&self, id: &str) -> Option<([f32; 2], Option<f32>)> {
        self.node(id).map(|n| (n.stand, n.facing))
    }

    /// The node's children, in order.
    pub fn children(&self, id: &str) -> impl Iterator<Item = &Node> {
        self.index
            .get(id)
            .map_or(&[][..], |&i| &self.children[i][..])
            .iter()
            .map(|&c| &self.nodes[c])
    }

    #[must_use]
    pub fn parent(&self, id: &str) -> Option<&Node> {
        self.node(id)?.parent_id().and_then(|p| self.node(p))
    }

    /// The node's ancestors, nearest first, ending at the root.
    #[must_use]
    pub fn ancestors(&self, id: &str) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut at = self.parent(id);
        while let Some(node) = at {
            out.push(node);
            at = self.parent(&node.id);
        }
        out
    }

    /// Every node depth first, each followed by everything under it, as an
    /// outline reads.
    #[must_use]
    pub fn walk(&self) -> Vec<&Node> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![0];
        while let Some(i) = stack.pop() {
            out.push(&self.nodes[i]);
            stack.extend(self.children[i].iter().rev());
        }
        out
    }

    /// Everything under the node, in tree order.
    #[must_use]
    pub fn descendants(&self, id: &str) -> Vec<&Node> {
        let prefix = format!("{id}/");
        self.nodes
            .iter()
            .filter(|n| n.id.starts_with(&prefix))
            .collect()
    }

    /// The nodes of `kind`.
    pub fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.kind == kind)
    }

    /// The objects of `object` kind.
    pub fn objects(&self, object: Object) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.object == Some(object))
    }

    /// The nodes in district `district`, by its slug, the district first.
    pub fn in_district<'a>(&'a self, district: &'a str) -> impl Iterator<Item = &'a Node> {
        self.nodes
            .iter()
            .filter(move |n| n.district.as_deref() == Some(district))
    }

    /// The nodes that offer `affordance`.
    pub fn with_affordance(&self, affordance: Affordance) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.offers(affordance))
    }

    /// The node made from layout source `source`, such as `door:bakery`.
    #[must_use]
    pub fn by_source(&self, source: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.source == source)
    }

    /// The nearest node of `kind` at or above `id`.
    #[must_use]
    pub fn enclosing(&self, id: &str, kind: Kind) -> Option<&Node> {
        let node = self.node(id)?;
        if node.kind == kind {
            return Some(node);
        }
        self.ancestors(id).into_iter().find(|n| n.kind == kind)
    }

    /// The smallest room whose floor holds `[x, z]`.
    #[must_use]
    pub fn room_at(&self, at: [f32; 2]) -> Option<&Node> {
        self.of_kind(Kind::Room)
            .filter(|r| r.contains(at))
            .min_by(|a, b| area(a).total_cmp(&area(b)))
    }
}

fn area(room: &Node) -> f32 {
    room.area.map_or(f32::INFINITY, |[min, max]| {
        (max[0] - min[0]) * (max[1] - min[1])
    })
}

fn check_fields(node: &Node) -> Result<(), TreeError> {
    let bad = |why| Err(TreeError::Field(node.id.clone(), why));
    if node.name.trim().is_empty() {
        return bad("no name");
    }
    if node.source.is_empty() {
        return bad("no source");
    }
    if !node.stand.iter().all(|v| v.is_finite()) || node.facing.is_some_and(|f| !f.is_finite()) {
        return bad("a standing point or heading isn't finite");
    }
    if (node.kind == Kind::Object) != node.object.is_some() {
        return bad("only an object, and every object, names its object kind");
    }
    if node.open.is_some() != (node.object == Some(Object::Door)) {
        return bad("only a door, and every door, says whether it is open");
    }
    if let Some([min, max]) = node.area
        && (node.kind != Kind::Room
            || !min.iter().chain(&max).all(|v| v.is_finite())
            || min[0] > max[0]
            || min[1] > max[1])
    {
        return bad("only a room has a floor, as a finite box");
    }
    if let Some(entry) = node.entry
        && (!matches!(node.kind, Kind::Building | Kind::Room)
            || !entry.iter().flatten().all(|v| v.is_finite()))
    {
        return bad("only a building or a room has a doorway, at finite points");
    }
    if node.kind == Kind::Zone || node.district.is_some() {
        return Ok(());
    }
    bad("a node under a district names it")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Everglade's tree, generated from its layout tables and checked in as
/// `data/everglade.json`.
///
/// # Panics
///
/// When the checked-in snapshot doesn't parse, which its test refuses.
#[must_use]
pub fn everglade() -> &'static Tree {
    static TREE: LazyLock<Tree> = LazyLock::new(|| {
        Tree::parse(include_str!("../data/everglade.json")).expect("the Everglade tree parses")
    });
    &TREE
}

#[cfg(test)]
mod tests;
