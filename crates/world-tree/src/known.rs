//! What one agent knows of the tree, as in the paper: the node IDs it has
//! perceived, by entering a place ([`Known::enter`]) or by seeing it
//! ([`Known::see`], which a zone's sight sweep fills). An agent plans only
//! over the subgraph it knows. Alice keeps hers in
//! `agents/NAME/known.json` ([`Known::save`], [`Known::load`]); townsfolk
//! keep theirs in memory.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Kind, Node, Tree};

/// The known subgraph's schema identifier.
pub const KNOWN_SCHEMA: &str = "openagents.verse-world-known.v1";

/// One agent's known nodes in one tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Known {
    pub schema: String,
    pub agent: String,
    /// The digest of the tree the IDs name.
    pub tree: String,
    pub nodes: BTreeSet<String>,
}

impl Known {
    /// Knowing only the zone's root.
    #[must_use]
    pub fn new(agent: &str, tree: &Tree) -> Self {
        Self {
            schema: KNOWN_SCHEMA.into(),
            agent: agent.into(),
            tree: tree.digest().into(),
            nodes: BTreeSet::from([tree.root().id.clone()]),
        }
    }

    /// Whether the agent knows node `id`.
    #[must_use]
    pub fn knows(&self, id: &str) -> bool {
        self.nodes.contains(id)
    }

    /// Learns node `id` and its ancestors. Returns how many it learned.
    pub fn see(&mut self, tree: &Tree, id: &str) -> usize {
        let Some(node) = tree.node(id) else { return 0 };
        let before = self.nodes.len();
        self.nodes.insert(node.id.clone());
        for up in tree.ancestors(id) {
            self.nodes.insert(up.id.clone());
        }
        self.nodes.len() - before
    }

    /// Enters node `id`: learns it, its ancestors, and what is directly in
    /// it. Entering a room shows its objects; entering a building shows
    /// its rooms and its own objects, but not what the rooms hold.
    /// Returns how many it learned.
    pub fn enter(&mut self, tree: &Tree, id: &str) -> usize {
        let before = self.nodes.len();
        if self.see(tree, id) == 0 && !self.knows(id) {
            return 0;
        }
        for child in tree.children(id) {
            self.nodes.insert(child.id.clone());
        }
        self.nodes.len() - before
    }

    /// Enters the room whose floor holds `[x, z]`, if any. Returns how
    /// many nodes it learned.
    pub fn enter_at(&mut self, tree: &Tree, at: [f32; 2]) -> usize {
        match tree.room_at(at) {
            Some(room) => {
                let id = room.id.clone();
                self.enter(tree, &id)
            }
            None => 0,
        }
    }

    /// The known children of node `id`, in tree order.
    #[must_use]
    pub fn children<'t>(&self, tree: &'t Tree, id: &str) -> Vec<&'t Node> {
        tree.children(id).filter(|n| self.knows(&n.id)).collect()
    }

    /// The known nodes, in tree order.
    #[must_use]
    pub fn subgraph<'t>(&self, tree: &'t Tree) -> Vec<&'t Node> {
        tree.nodes().iter().filter(|n| self.knows(&n.id)).collect()
    }

    /// The known nodes of `kind`.
    #[must_use]
    pub fn of_kind<'t>(&self, tree: &'t Tree, kind: Kind) -> Vec<&'t Node> {
        tree.of_kind(kind).filter(|n| self.knows(&n.id)).collect()
    }

    /// Moves to `tree`, a newer tree: keeps the IDs it still holds and
    /// drops the rest. Returns how many it dropped.
    pub fn rebase(&mut self, tree: &Tree) -> usize {
        let before = self.nodes.len();
        self.nodes.retain(|id| tree.node(id).is_some());
        self.nodes.insert(tree.root().id.clone());
        self.tree = tree.digest().into();
        before.saturating_sub(self.nodes.len())
    }

    /// Reads a known subgraph from `path`; a missing file is `None`.
    ///
    /// # Errors
    ///
    /// When the file can't be read or isn't a known subgraph.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let known: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if known.schema != KNOWN_SCHEMA {
            return Err(format!(
                "{}: the schema is {:?}, not {KNOWN_SCHEMA}",
                path.display(),
                known.schema
            ));
        }
        Ok(Some(known))
    }

    /// Writes the subgraph to `path` through a temporary file beside it,
    /// so a reader never sees half a file.
    ///
    /// # Errors
    ///
    /// When the file can't be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut text = serde_json::to_string_pretty(self).expect("plain data serializes");
        text.push('\n');
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, text).map_err(|e| format!("{}: {e}", partial.display()))?;
        std::fs::rename(&partial, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}
