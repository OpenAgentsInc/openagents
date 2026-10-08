//! Object states (`docs/verse/generative-agents.md`, item 3): what the
//! tree's objects are like now, derived from [`Conditions`] the zone reads
//! off its clock and its studio snapshot. Lamps are lit or dark, doors
//! open or closed, an exclusive object is busy or free, and the Task Wall
//! counts its columns. A pylon and the Wellspring carry the `pylon` and
//! `wellspring` states of NIP-PYLON's world projection
//! (`nips/openagents/NIP-PYLON.md`), which the zone derives from its
//! compute source; with no source, they have no state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Object, Tree};

/// One Task Wall column and how many tasks it holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub count: u32,
}

/// A pylon's status (NIP-PYLON's `status`): `unknown` for a stale or
/// missing source, never `online`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PylonStatus {
    Online,
    /// Finishing admitted jobs and taking no new ones.
    Draining,
    Offline,
    Unknown,
}

impl PylonStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Draining => "draining",
            Self::Offline => "offline",
            Self::Unknown => "unknown",
        }
    }
}

/// A pylon's hardware family (NIP-PYLON's `class.family`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    UnifiedMemory,
    Gpu,
    Cpu,
}

impl Family {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnifiedMemory => "unified-memory",
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
        }
    }
}

/// A pylon's size band within its family (NIP-PYLON's `class.tier`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Small,
    Medium,
    Large,
    Xl,
}

impl Tier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Xl => "xl",
        }
    }
}

/// One object's state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum State {
    Lamp {
        lit: bool,
    },
    Door {
        open: bool,
    },
    /// An exclusive object: busy while someone uses it.
    Workstation {
        busy: bool,
        /// Who uses it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
    },
    TaskWall {
        columns: Vec<Column>,
    },
    /// A pylon, from its source's newest fresh sample.
    Pylon {
        /// The pylon's address: a `30200` address once beacons exist, or
        /// `local:<slug>` for this machine's own source.
        pylon: String,
        status: PylonStatus,
        family: Family,
        tier: Tier,
        /// Slots in use: `total − free`.
        busy: u32,
        total: u32,
        /// Jobs served, from receipts.
        jobs: u64,
        /// Sats earned, in millisatoshis, per network; empty while nothing
        /// pays.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        paid_msat: BTreeMap<String, u64>,
        /// Seconds online, when the source says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uptime: Option<u64>,
    },
    /// The Wellspring, the pool's totals.
    Wellspring {
        /// The pool: a `30201` address, or `local` for this machine's.
        pool: String,
        online: u32,
        busy: u32,
        total: u32,
        /// Accepted jobs in the newest slice.
        rate: u32,
        /// Only when the client recomputed a pool aggregate.
        verified: bool,
    },
}

impl State {
    /// A short phrase for a dump or a prompt, such as `lit` or `busy (ada)`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Lamp { lit } => if *lit { "lit" } else { "dark" }.into(),
            Self::Door { open } => if *open { "open" } else { "closed" }.into(),
            Self::Workstation { busy: false, .. } => "free".into(),
            Self::Workstation { busy: true, by } => match by {
                Some(who) => format!("busy ({who})"),
                None => "busy".into(),
            },
            Self::TaskWall { columns } if columns.is_empty() => "no tasks known".into(),
            Self::Pylon {
                status,
                busy,
                total,
                jobs,
                ..
            } => format!(
                "{}, {busy} of {total} slots busy, {jobs} jobs",
                status.as_str()
            ),
            Self::Wellspring {
                online,
                busy,
                total,
                verified,
                ..
            } => format!(
                "{online} pylons online, {busy} of {total} slots busy{}",
                if *verified { "" } else { ", unverified" }
            ),
            Self::TaskWall { columns } => columns
                .iter()
                .map(|c| format!("{} {}", c.name.to_lowercase(), c.count))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

/// What the zone knows now, in the tree's terms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conditions {
    /// Whether the town's lamps burn (`time_of_day::Light::lamps_lit`).
    pub lamps_lit: bool,
    /// Who uses each exclusive object, by node ID.
    pub occupants: BTreeMap<String, String>,
    /// The Task Wall's columns in order, with their counts.
    pub task_columns: Vec<Column>,
    /// Each pylon's and the Wellspring's state by node ID, from the
    /// zone's compute source. A node with none has no state.
    pub compute: BTreeMap<String, State>,
}

/// Every stateful object's state, by node ID.
pub type States = BTreeMap<String, State>;

/// The NIP-MV entity ID that carries node `node`'s state: `obj-` and the
/// first 20 hex digits of the ID's SHA-256, since an entity ID is at most
/// 64 bytes of `[a-z0-9_-]` and a node ID has slashes.
#[must_use]
pub fn entity_id(node: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(node.as_bytes());
    let hex: String = digest.iter().take(10).map(|b| format!("{b:02x}")).collect();
    format!("obj-{hex}")
}

/// The state of each object in `tree` under `now`: every lamp, door,
/// exclusive object, and Task Wall, and each pylon and the Wellspring that
/// `now` has a state for.
#[must_use]
pub fn derive(tree: &Tree, now: &Conditions) -> States {
    let mut out = States::new();
    for node in tree.nodes() {
        let state = match node.object {
            Some(Object::Lamp) => State::Lamp { lit: now.lamps_lit },
            Some(Object::Door) => State::Door {
                open: node.open.unwrap_or(false),
            },
            Some(Object::TaskWall) => State::TaskWall {
                columns: now.task_columns.clone(),
            },
            Some(Object::Pylon | Object::Wellspring) => match now.compute.get(&node.id) {
                Some(state) => state.clone(),
                None => continue,
            },
            Some(_) if node.exclusive => {
                let by = now.occupants.get(&node.id).cloned();
                State::Workstation {
                    busy: by.is_some(),
                    by,
                }
            }
            _ => continue,
        };
        out.insert(node.id.clone(), state);
    }
    out
}
