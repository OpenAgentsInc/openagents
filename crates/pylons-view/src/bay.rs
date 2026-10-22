//! The Pylon Bay: the one place on the Grid where its pylons stand (#12423).
//!
//! The Bay lies east of the plaza, left of the walk from the spawn to the
//! Gym, and faces the plaza. Its front holds four sections, one for each
//! kind of machine: Mac, GPU, CPU, and secure. Behind them, each model
//! group has a lane of its own: the head first, then its parts in layer
//! order, joined by a pipe on the floor.
//!
//! [`Bay::sync`] keeps every pylon on the spot it took when it arrived, so
//! the Bay doesn't reshuffle as pylons come and go. A spot is free again
//! once its pylon leaves.
//!
//! Both browser modules read this module: the Grid layer
//! (`crates/pylons-web`) for the places its labels stand on, and Verse
//! (`crates/everglade-web`) for the machines it draws. The layer hands the
//! Bay to Verse as a flat list of numbers ([`encode`] and [`decode`]).

pub mod art;
pub mod machine;

use std::collections::{BTreeMap, BTreeSet};

use crate::Pylon;

/// The Bay's corner on the floor, `[x, z]`, m: where its first section
/// starts. Bay coordinates run `u` along its front (world +z, away from the
/// spawn) and `v` into its depth (world +x, away from the plaza).
pub const ORIGIN: [f64; 2] = [17.0, -2.0];

/// The world point at Bay coordinates (`u`, `v`) on the floor.
#[must_use]
pub fn world(u: f64, v: f64) -> [f64; 3] {
    [ORIGIN[0] + v, 0.0, ORIGIN[1] + u]
}

/// Spots in one section: two across and three deep.
pub const SECTION_SLOTS: usize = 6;
/// How far apart the sections start, along the front, m.
pub const SECTION_PITCH: f64 = 7.0;
/// One section's width along the front, m.
pub const SECTION_WIDTH: f64 = 5.5;
/// The two columns of a section, from its start, m.
const COLUMNS: [f64; 2] = [1.25, 4.25];
/// How far apart a section's rows are, m.
const ROW_PITCH: f64 = 2.8;

/// Lanes for model groups: two across and four deep.
pub const LANES: usize = 8;
/// Where the first row of lanes is, into the Bay, m.
pub const LANE_TOP: f64 = 10.5;
/// How far apart lane rows are, m.
pub const LANE_ROW_PITCH: f64 = 4.0;
/// How far apart the two lane columns start, m.
pub const LANE_PITCH: f64 = 14.0;
/// A lane's length, m.
pub const LANE_LENGTH: f64 = 11.0;
/// The head's place in its lane, m.
pub const HEAD_AT: f64 = 1.0;
/// The first part's place in its lane, m.
pub const FIRST_PART_AT: f64 = 3.3;
/// The most room between two parts, m.
const PART_PITCH: f64 = 2.3;
/// The most parts a lane draws apart; more share the lane's length.
pub const MOST_PARTS: u32 = 8;

/// The Bay's outline in Bay coordinates: `u` from, `u` to, `v` from, `v`
/// to.
pub const PAD: [f64; 4] = [-1.5, 28.5, -2.2, 24.3];
/// Where the Bay's sign stands, into the Bay, m: behind its last lanes,
/// raised over them as the Gym's name is over the Gym.
pub const SIGN_V: f64 = 25.0;

/// The kind of machine, and so the section a pylon stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Mac,
    Gpu,
    Cpu,
    Secure,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Mac, Kind::Gpu, Kind::Cpu, Kind::Secure];

    /// The section's name, painted on the floor in front of it.
    #[must_use]
    pub fn sign(self) -> &'static str {
        match self {
            Kind::Mac => "MAC",
            Kind::Gpu => "GPU",
            Kind::Cpu => "CPU",
            Kind::Secure => "SECURE",
        }
    }

    /// A pylon's kind. A beacon doesn't say yet whether its machine is
    /// sealed, so a label that names sealed hardware (Intel TDX, AMD
    /// SEV-SNP, a confidential or attested machine) counts as secure.
    #[must_use]
    pub fn of(pylon: &Pylon) -> Kind {
        let name = pylon.name.to_ascii_lowercase();
        if [
            "tdx",
            "sev-snp",
            "confidential",
            "sealed",
            "enclave",
            "attested",
        ]
        .iter()
        .any(|word| name.contains(word))
        {
            return Kind::Secure;
        }
        match pylon.family.as_str() {
            "gpu" => Kind::Gpu,
            "cpu" => Kind::Cpu,
            _ => Kind::Mac,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What Verse draws for a pylon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A slim tower with a crystal cap: a unified-memory machine.
    Mac,
    /// A rack of blades with fans on top.
    Gpu,
    /// A squat box with a chip on its front and fins on top.
    Cpu,
    /// A box inside a locked cage.
    Secure,
    /// A model group's head: the router with a halo.
    Head,
    /// A model group's part: a slim rack.
    Part,
}

impl Shape {
    pub const ALL: [Shape; 6] = [
        Shape::Mac,
        Shape::Gpu,
        Shape::Cpu,
        Shape::Secure,
        Shape::Head,
        Shape::Part,
    ];

    #[must_use]
    pub fn from_index(index: usize) -> Option<Shape> {
        Self::ALL.get(index).copied()
    }

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The top of the machine, m: where its beam starts.
    #[must_use]
    pub fn top(self) -> f64 {
        match self {
            Shape::Mac => 2.35,
            Shape::Gpu => 1.7,
            Shape::Cpu => 1.05,
            Shape::Secure => 1.75,
            Shape::Head => 1.8,
            Shape::Part => 1.45,
        }
    }

    fn of(pylon: &Pylon) -> Shape {
        match pylon.group.as_ref().map(|g| g.role.as_str()) {
            Some("head") => Shape::Head,
            // A part on a Mac's GPU stands as a Mac (#12420).
            Some(_)
                if pylon.family == "unified-memory"
                    || pylon
                        .group
                        .as_ref()
                        .and_then(|g| g.device.as_deref())
                        .map(machine::Device::of)
                        == Some(machine::Device::Metal) =>
            {
                Shape::Mac
            }
            Some(_) => Shape::Part,
            None => match Kind::of(pylon) {
                Kind::Mac => Shape::Mac,
                Kind::Gpu => Shape::Gpu,
                Kind::Cpu => Shape::Cpu,
                Kind::Secure => Shape::Secure,
            },
        }
    }
}

/// How busy a machine is, which sets its beam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Online with nothing running: a short, faint beam.
    Idle,
    /// Some of its slots busy.
    Some,
    /// Every slot busy: the tallest, brightest beam.
    Full,
    /// Winding down: a broken beam.
    Draining,
}

impl Load {
    pub const ALL: [Load; 4] = [Load::Idle, Load::Some, Load::Full, Load::Draining];

    #[must_use]
    pub fn from_index(index: usize) -> Option<Load> {
        Self::ALL.get(index).copied()
    }

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The beam's height over the machine's top, m.
    #[must_use]
    pub fn beam(self) -> f64 {
        match self {
            Load::Idle | Load::Draining => 1.2,
            Load::Some => 2.0,
            Load::Full => 2.8,
        }
    }

    /// Whether the machine is working.
    #[must_use]
    pub fn busy(self) -> bool {
        matches!(self, Load::Some | Load::Full)
    }

    fn of(busy: u32, total: u32, draining: bool) -> Load {
        if draining {
            Load::Draining
        } else if busy == 0 {
            Load::Idle
        } else if busy < total {
            Load::Some
        } else {
            Load::Full
        }
    }
}

/// One pylon's place in the Bay.
#[derive(Debug, Clone, PartialEq)]
pub struct Stand {
    pub id: String,
    /// Its foot on the floor.
    pub at: [f64; 3],
    pub shape: Shape,
    pub load: Load,
}

/// The pipe joining a model group's head to its parts.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipe {
    /// The head's foot: where the pipe starts.
    pub at: [f64; 3],
    pub parts: u32,
    /// The head is answering: a step runs through the pipe.
    pub lit: bool,
}

/// The Bay this moment: every pylon that has a spot, and each group's
/// pipe. A pylon with no spot (its section or every lane is full) has no
/// stand; the list still names it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub stands: Vec<Stand>,
    pub pipes: Vec<Pipe>,
}

impl Layout {
    /// The stand of the pylon `id`.
    #[must_use]
    pub fn stand(&self, id: &str) -> Option<&Stand> {
        self.stands.iter().find(|stand| stand.id == id)
    }
}

/// Who holds which spot. Spots are kept until their pylon (or, for a
/// lane, every pylon of its group) leaves.
#[derive(Debug, Clone, Default)]
pub struct Bay {
    sections: [Vec<Option<String>>; 4],
    lanes: Vec<Option<String>>,
}

/// The spot `slot` of section `kind`, in Bay coordinates.
#[must_use]
pub fn section_spot(kind: Kind, slot: usize) -> Option<(f64, f64)> {
    (slot < SECTION_SLOTS).then(|| {
        (
            kind.index() as f64 * SECTION_PITCH + COLUMNS[slot % 2],
            (slot / 2) as f64 * ROW_PITCH,
        )
    })
}

/// Lane `lane`'s start, in Bay coordinates.
#[must_use]
pub fn lane_start(lane: usize) -> Option<(f64, f64)> {
    (lane < LANES).then(|| {
        (
            (lane % 2) as f64 * LANE_PITCH,
            LANE_TOP + (lane / 2) as f64 * LANE_ROW_PITCH,
        )
    })
}

/// How far along its lane a group member stands, m: the head first, then
/// part 1 to part `parts`.
#[must_use]
pub fn member_at(part: Option<u32>, parts: u32) -> f64 {
    let Some(part) = part else {
        return HEAD_AT;
    };
    let parts = parts.clamp(1, MOST_PARTS);
    let part = part.clamp(1, parts);
    let room = LANE_LENGTH - 0.8 - FIRST_PART_AT;
    let pitch = if parts > 1 {
        (room / f64::from(parts - 1)).min(PART_PITCH)
    } else {
        0.0
    };
    FIRST_PART_AT + f64::from(part - 1) * pitch
}

fn take(spots: &mut [Option<String>], id: &str) -> Option<usize> {
    if let Some(held) = spots.iter().position(|s| s.as_deref() == Some(id)) {
        return Some(held);
    }
    let free = spots.iter().position(Option::is_none)?;
    spots[free] = Some(id.to_owned());
    Some(free)
}

impl Bay {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sections: std::array::from_fn(|_| vec![None; SECTION_SLOTS]),
            lanes: vec![None; LANES],
        }
    }

    /// Places `pylons`, the snapshot's whole list: pylons already here keep
    /// their spots, pylons gone free theirs, and new ones take the first
    /// free spot in their section, or a free lane for a new group, in the
    /// order of their IDs.
    pub fn sync(&mut self, pylons: &[Pylon]) -> Layout {
        if self.lanes.len() != LANES {
            *self = Self::new();
        }
        let mut ordinary: [BTreeSet<&str>; 4] = Default::default();
        let mut groups: BTreeSet<&str> = BTreeSet::new();
        for pylon in pylons {
            match &pylon.group {
                Some(group) => {
                    groups.insert(group.id.as_str());
                }
                None => {
                    ordinary[Kind::of(pylon).index()].insert(pylon.id.as_str());
                }
            }
        }
        for (section, present) in self.sections.iter_mut().zip(&ordinary) {
            for spot in section.iter_mut() {
                if spot.as_deref().is_some_and(|id| !present.contains(id)) {
                    *spot = None;
                }
            }
            for id in present {
                take(section, id);
            }
        }
        for spot in &mut self.lanes {
            if spot.as_deref().is_some_and(|id| !groups.contains(id)) {
                *spot = None;
            }
        }
        for id in &groups {
            take(&mut self.lanes, id);
        }

        // Each group's pipe: as many parts as its head says, or as its
        // parts say before the head arrives; lit while the head answers.
        let mut pipes: BTreeMap<usize, Pipe> = BTreeMap::new();
        let mut stands = Vec::with_capacity(pylons.len());
        let lane_of = |id: &str| self.lanes.iter().position(|l| l.as_deref() == Some(id));
        let heads: BTreeMap<&str, &Pylon> = pylons
            .iter()
            .filter_map(|p| {
                let g = p.group.as_ref()?;
                (g.role == "head").then_some((g.id.as_str(), p))
            })
            .collect();
        for pylon in pylons {
            let draining = pylon.status == "draining";
            let (at, load) = match &pylon.group {
                None => {
                    let kind = Kind::of(pylon);
                    let Some((u, v)) = self.sections[kind.index()]
                        .iter()
                        .position(|s| s.as_deref() == Some(pylon.id.as_str()))
                        .and_then(|slot| section_spot(kind, slot))
                    else {
                        continue;
                    };
                    (world(u, v), Load::of(pylon.busy, pylon.total, draining))
                }
                Some(group) => {
                    let Some(lane) = lane_of(&group.id) else {
                        continue;
                    };
                    let Some((u0, v)) = lane_start(lane) else {
                        continue;
                    };
                    let head = heads.get(group.id.as_str());
                    let parts = head
                        .and_then(|h| h.group.as_ref())
                        .map(|g| g.parts)
                        .filter(|parts| *parts > 0)
                        .unwrap_or(group.parts.max(group.part.unwrap_or(0)));
                    let answering = head.is_some_and(|h| h.busy > 0 && h.status == "online");
                    let pipe = pipes.entry(lane).or_insert_with(|| Pipe {
                        at: world(u0 + HEAD_AT, v),
                        parts: 0,
                        lit: false,
                    });
                    pipe.parts = pipe.parts.max(parts.min(MOST_PARTS));
                    pipe.lit |= answering;
                    let part = if group.role == "head" {
                        None
                    } else {
                        Some(group.part.unwrap_or(1))
                    };
                    let load = match part {
                        None => Load::of(pylon.busy, pylon.total, draining),
                        // A part has no slots of its own: it works while
                        // its head answers.
                        Some(_) if draining => Load::Draining,
                        Some(_) if answering => Load::Some,
                        Some(_) => Load::Idle,
                    };
                    (world(u0 + member_at(part, parts), v), load)
                }
            };
            stands.push(Stand {
                id: pylon.id.clone(),
                at,
                shape: Shape::of(pylon),
                load,
            });
        }
        Layout {
            stands,
            pipes: pipes.into_values().filter(|p| p.parts > 0).collect(),
        }
    }
}

/// How much a pylon's label says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// No label: far away, or a group's part away from its head.
    Hidden,
    /// Its name only.
    Name,
    /// Name, machine, parts and layers, how busy, and models.
    Full,
}

/// Within this distance from the camera a pylon shows its name, m.
pub const NAME_REACH: f64 = 48.0;
/// Within this distance it shows everything, m.
pub const FULL_REACH: f64 = 16.0;
/// A group's part shows its label only this near, m: from further, the
/// group's one label is its head's.
pub const PART_REACH: f64 = 9.0;

/// How much the label of a `shape` pylon `depth` meters from the camera
/// says. A pointer over it, or its open card, shows everything.
#[must_use]
pub fn tier(shape: Shape, depth: f64, focused: bool) -> Tier {
    if focused {
        return Tier::Full;
    }
    if shape == Shape::Part {
        return if depth <= PART_REACH {
            Tier::Full
        } else {
            Tier::Hidden
        };
    }
    if depth <= FULL_REACH {
        Tier::Full
    } else if depth <= NAME_REACH {
        Tier::Name
    } else {
        Tier::Hidden
    }
}

/// A label's words: a title, and the lines under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub title: String,
    pub lines: Vec<String>,
}

/// What a pylon's label says in full. A part says its place in its group
/// ("Part 2 of 4", "Layers 10–20 · GPU"); a head says its group and its
/// model; any other pylon says its machine, how busy it is, and its models.
#[must_use]
pub fn tag(pylon: &Pylon) -> Tag {
    let models: Vec<&str> = {
        let chat: Vec<&str> = pylon
            .services
            .iter()
            .filter(|s| s.kinds.iter().any(|k| k == "Chat"))
            .map(|s| s.model.split('@').next().unwrap_or(&s.model))
            .collect();
        if chat.is_empty() {
            pylon
                .services
                .iter()
                .map(|s| s.model.split('@').next().unwrap_or(&s.model))
                .take(1)
                .collect()
        } else {
            chat.into_iter().take(2).collect()
        }
    };
    let status = (pylon.status == "draining").then_some(pylon.status_words());
    let machine = match Kind::of(pylon) {
        Kind::Secure => format!("Secure {}", pylon.class_words()),
        _ => pylon.class_words().to_owned(),
    };
    let mut lines = Vec::new();
    match &pylon.group {
        Some(group) if group.role != "head" => {
            let title = match group.part {
                Some(part) => format!("Part {part} of {}", group.parts),
                None => "Model group part".to_owned(),
            };
            let mut line = match group.layers {
                Some([from, to]) => format!("Layers {from}–{to} · {machine}"),
                None => machine,
            };
            if let Some(status) = status {
                line = format!("{status} · {line}");
            }
            lines.push(line);
            return Tag { title, lines };
        }
        Some(group) => {
            let mut line = format!(
                "Model group · {} parts · {}",
                group.parts,
                pylon.slots_words()
            );
            if let Some(status) = status {
                line = format!("{status} · {line}");
            }
            lines.push(line);
        }
        None => {
            let mut line = format!("{machine} · {}", pylon.slots_words());
            if let Some(status) = status {
                line = format!("{status} · {line}");
            }
            lines.push(line);
        }
    }
    if !models.is_empty() {
        lines.push(models.join(", "));
    }
    Tag {
        title: pylon.name.clone(),
        lines,
    }
}

/// The page global the layer hands Verse the machines' labels in, as JSON
/// ([`Label`]s), whenever one changes (#12459). Verse draws them on its
/// canvas, so no HTML label stands over the world.
pub const LABELS_GLOBAL: &str = "pylonsLabels";

/// One machine's label as the layer hands it to Verse: where it stands in
/// the world, how much it says, and its words.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Label {
    /// The point over the machine's top the label stands on, m.
    pub at: [f64; 3],
    /// [`Tier::Full`] when `true`, else [`Tier::Name`].
    pub full: bool,
    pub title: String,
    pub lines: Vec<String>,
    /// Its slots, drawn as boxes in a full label; none for a group's part.
    pub busy: u32,
    pub total: u32,
    /// Under the pointer or with its card open.
    pub focused: bool,
}

impl Label {
    /// The label of `pylon` standing at `at`, saying as much as `tier`
    /// allows; `None` at [`Tier::Hidden`].
    #[must_use]
    pub fn of(pylon: &Pylon, at: [f64; 3], tier: Tier, focused: bool) -> Option<Self> {
        if tier == Tier::Hidden {
            return None;
        }
        let tag = tag(pylon);
        let full = tier == Tier::Full;
        let part = pylon.group.as_ref().is_some_and(|g| g.role == "part");
        Some(Self {
            at,
            full,
            title: tag.title,
            lines: if full { tag.lines } else { Vec::new() },
            busy: pylon.busy,
            total: if full && !part {
                pylon.total.min(16)
            } else {
                0
            },
            focused,
        })
    }
}

/// The handoff's version: the first number [`encode`] writes.
pub const WIRE: f64 = 1.0;
/// Numbers per stand and per pipe in the handoff.
const STAND_WIDTH: usize = 6;
const PIPE_WIDTH: usize = 4;

/// A stand as Verse draws it this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub x: f64,
    pub z: f64,
    pub shape: Shape,
    pub load: Load,
    /// When its last job receipt arrived, page milliseconds
    /// (`performance.now()`), or a negative number for none.
    pub pulse_at: f64,
    /// A pointer is over it or its card is open.
    pub focused: bool,
}

/// A pipe as Verse draws it this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnPipe {
    pub x: f64,
    pub z: f64,
    pub parts: u32,
    pub lit: bool,
}

/// The Bay as numbers for Verse: `[WIRE, stands, pipes]`, then each
/// stand's `[x, z, shape, load, pulse_at, focused]`, then each pipe's
/// `[x, z, parts, lit]`.
#[must_use]
pub fn encode(stands: &[Drawn], pipes: &[DrawnPipe]) -> Vec<f64> {
    let mut out = Vec::with_capacity(3 + stands.len() * STAND_WIDTH + pipes.len() * PIPE_WIDTH);
    out.extend([WIRE, stands.len() as f64, pipes.len() as f64]);
    for s in stands {
        out.extend([
            s.x,
            s.z,
            s.shape.index() as f64,
            s.load.index() as f64,
            s.pulse_at,
            f64::from(u8::from(s.focused)),
        ]);
    }
    for p in pipes {
        out.extend([p.x, p.z, f64::from(p.parts), f64::from(u8::from(p.lit))]);
    }
    out
}

/// Reads [`encode`]'s numbers back; `None` for any other list.
#[must_use]
pub fn decode(numbers: &[f64]) -> Option<(Vec<Drawn>, Vec<DrawnPipe>)> {
    let (&[wire, stands, pipes], rest) = numbers.split_first_chunk::<3>()?;
    if wire != WIRE || !(0.0..=1024.0).contains(&stands) || !(0.0..=1024.0).contains(&pipes) {
        return None;
    }
    let (stands, pipes) = (stands as usize, pipes as usize);
    if rest.len() != stands * STAND_WIDTH + pipes * PIPE_WIDTH || rest.iter().any(|v| v.is_nan()) {
        return None;
    }
    let (stand_numbers, pipe_numbers) = rest.split_at(stands * STAND_WIDTH);
    let drawn = stand_numbers
        .chunks_exact(STAND_WIDTH)
        .map(|n| {
            Some(Drawn {
                x: n[0],
                z: n[1],
                shape: Shape::from_index(n[2] as usize)?,
                load: Load::from_index(n[3] as usize)?,
                pulse_at: n[4],
                focused: n[5] > 0.5,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let pipes = pipe_numbers
        .chunks_exact(PIPE_WIDTH)
        .map(|n| DrawnPipe {
            x: n[0],
            z: n[1],
            parts: (n[2].max(0.0) as u32).min(MOST_PARTS),
            lit: n[3] > 0.5,
        })
        .collect();
    Some((drawn, pipes))
}

#[cfg(test)]
mod tests;
