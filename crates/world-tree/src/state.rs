//! Object states (`docs/verse/generative-agents.md`, item 3): what the
//! tree's objects are like now, derived from [`Conditions`] the zone reads
//! off its clock and its studio snapshot. Lamps are lit or dark, doors
//! open or closed, an exclusive object is busy or free, and the Task Wall
//! counts its columns.

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
/// exclusive object, and Task Wall.
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
