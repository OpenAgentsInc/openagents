//! Transcript sources: rows an application publishes in its own process for
//! the adapter's transcript layout to read, so the rows never cross the view
//! contract.
//!
//! An application that renders a `Transcript` calls [`detach`] on its view
//! before validating it. Each transcript's rows move into a named source, and
//! the node keeps only its label, its earlier control, and the source's name.
//! The view stays small however long the conversation grows, and the adapter
//! neither decodes nor re-encodes rows: its layout update names the source
//! (`Update::source`), and Rust reads the rows here and lays out the ones
//! whose content changed.
//!
//! A published row is validated as a one-node view, so each row keeps the
//! view's node, depth, and text bounds; only the transcript's total escapes
//! them. A source holds at most [`super::MAX_ROWS`] rows, and the process
//! holds at most [`MAX_SOURCES`] sources.

use super::{EarlierRow, LayoutError, MAX_ROWS, content_hash, without_intents};
use crate::view::{Element, Node, View};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

/// The most sources one process holds.
pub const MAX_SOURCES: usize = 64;

/// One published row: its node and a hash of its content.
#[derive(Debug)]
pub(crate) struct SourceRow {
    pub(crate) node: Arc<Node<()>>,
    pub(crate) content: u64,
}

/// One publication of a source: its rows, oldest first, and its earlier
/// control. Snapshots are immutable; a layout reads the newest.
#[derive(Debug)]
pub struct Snapshot {
    pub(crate) rows: Vec<SourceRow>,
    pub(crate) earlier: Option<EarlierRow>,
    /// Unique per publication in this process.
    pub(crate) generation: u64,
}

impl Snapshot {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The row keys, oldest first.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.rows.iter().map(|row| row.node.key.as_str())
    }

    /// The rows, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &Node<()>> {
        self.rows.iter().map(|row| &*row.node)
    }
}

#[derive(Default)]
struct Registry {
    sources: HashMap<String, Arc<Snapshot>>,
    generation: u64,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Mutex::default);

fn registry() -> MutexGuard<'static, Registry> {
    // A panic while holding the lock leaves whole snapshots behind, never a
    // half-written one, so a poisoned lock is still consistent.
    REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Publishes `rows` and `earlier` as the source `name`, replacing what it
/// held. A row equal to the one the source already held under its key keeps
/// its hash and is not validated again, so republishing a long transcript
/// after a streamed token costs a comparison per row.
pub fn publish(
    name: &str,
    rows: Vec<Node<()>>,
    earlier: Option<EarlierRow>,
) -> Result<(), LayoutError> {
    if !crate::valid_id(name) {
        return Err(LayoutError::Source);
    }
    if rows.len() > MAX_ROWS {
        return Err(LayoutError::Limit);
    }
    let previous = registry().sources.get(name).cloned();
    let known: HashMap<&str, &SourceRow> = previous
        .as_deref()
        .map(|snapshot| {
            snapshot
                .rows
                .iter()
                .map(|row| (row.node.key.as_str(), row))
                .collect()
        })
        .unwrap_or_default();
    let mut seen = HashSet::with_capacity(rows.len());
    let mut published = Vec::with_capacity(rows.len());
    for row in rows {
        if !seen.insert(row.key.clone()) {
            return Err(LayoutError::DuplicateRow(row.key));
        }
        match known.get(row.key.as_str()) {
            Some(old) if *old.node == row => published.push(SourceRow {
                node: old.node.clone(),
                content: old.content,
            }),
            _ => {
                let checked = View::new("layout", 1, row)
                    .validate()
                    .map_err(LayoutError::Row)?;
                let node = checked.view().root.clone();
                let content = content_hash(&node);
                published.push(SourceRow {
                    node: Arc::new(node),
                    content,
                });
            }
        }
    }
    let mut registry = registry();
    if !registry.sources.contains_key(name) && registry.sources.len() >= MAX_SOURCES {
        return Err(LayoutError::Limit);
    }
    registry.generation += 1;
    let snapshot = Snapshot {
        rows: published,
        earlier,
        generation: registry.generation,
    };
    registry.sources.insert(name.to_owned(), Arc::new(snapshot));
    Ok(())
}

/// The newest publication of `name`.
pub fn get(name: &str) -> Option<Arc<Snapshot>> {
    registry().sources.get(name).cloned()
}

/// Removes the source `name`. A layout that already read it keeps its rows.
pub fn retire(name: &str) {
    registry().sources.remove(name);
}

/// Moves the rows of every transcript in `root` into a source named
/// `{scope}:{key}` and leaves the node with only its label, earlier control,
/// and source name. Sources under `scope` that `root` no longer shows are
/// retired. Returns how many transcripts were detached.
///
/// `scope` names the application surface, such as its view instance; two
/// surfaces must not share one.
pub fn detach<I>(root: &mut Node<I>, scope: &str) -> Result<usize, LayoutError> {
    let prefix = format!("{scope}:");
    let mut names = HashSet::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        match &mut node.element {
            Element::Transcript {
                children,
                earlier,
                source,
                ..
            } => {
                if source.is_some() {
                    continue;
                }
                let name = format!("{prefix}{}", node.key);
                let rows = children.iter().map(without_intents).collect();
                publish(&name, rows, earlier.as_ref().map(EarlierRow::from))?;
                children.clear();
                *source = Some(name.clone());
                names.insert(name);
            }
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Message { children, .. }
            | Element::Dialog { children, .. }
            | Element::Choice { children, .. }
            | Element::Tool { children, .. } => pending.extend(children.iter_mut()),
            _ => {}
        }
    }
    registry()
        .sources
        .retain(|name, _| !name.starts_with(&prefix) || names.contains(name));
    Ok(names.len())
}
