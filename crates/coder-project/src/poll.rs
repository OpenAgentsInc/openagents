//! The project board: one fetch of the item list, pinned.
//!
//! A [`Board`] is a claim about what the project said at one fetch — no
//! more. Each item pins the fields a dispatch decision reads: the issue
//! number, the title, the status field value, a version marker — the
//! item's own `updatedAt`, or its content digest when the fetch carried
//! no timestamp — and the declared dependency edges. Pinning the version
//! is what makes *reconcile before dispatch* checkable instead of
//! vibes: [`Board::changes`] names what a later fetch moved, so a stale
//! board is a diff a coordinator can read rather than a surprise a run
//! discovers.
//!
//! The module never fetches. The caller hands [`Board::from_snapshot`] a
//! parsed document — an object carrying `items`, or the bare item list —
//! and a snapshot missing a field pinning needs is an incomplete
//! observation, reported rather than guessed. A dependency edge pointing
//! at an item the snapshot does not contain is incomplete dependency
//! observation: it blocks the dependent, because missing data refuses,
//! it does not optimistically dispatch.
//!
//! Project membership and issue text are inputs to admission, never
//! execution authority. Every item carries
//! `authority: Authority::NoAuthority`: a task enters the admitted set
//! only through the coordinator's own checks — the prepared task map,
//! the reservation book, the permit. An exclusion is a number an
//! operator supplied, never a word an issue wrote about itself.
//!
//! # Determinism
//!
//! The board reads no clock and keeps no unordered state: the same
//! snapshot parses to the same board, the same board answers the same
//! ready list, the same refill, and the same change list, every time.
//! Items answer in the order the project held them.
//!
//! # The supplied document
//!
//! ```json
//! {
//!   "repository": "owner/repo",
//!   "exclusions": [9476],
//!   "items": [
//!     {
//!       "number": 9514,
//!       "title": "Project polling and refill",
//!       "status": "Ready",
//!       "updatedAt": "2026-09-21T00:00:00Z",
//!       "state": "open",
//!       "blockedBy": [{"number": 9504, "state": "closed"}],
//!       "blocking": [{"number": 9600}]
//!     }
//!   ]
//! }
//! ```
//!
//! Fields also resolve through a `content` object, the shape `gh project
//! item-list` and the GraphQL item nodes use, and a dependency
//! connection carrying `nodes`. `repository` and `exclusions` are
//! optional; a dependency that names a repository the board cannot
//! confirm as the scoped one is outside the observation and blocks.
//! An item whose declared type says it is not an issue is skipped with a
//! reason, not carried as work.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The most items one snapshot carries. A project bigger than the bound
/// is an observation the board refuses rather than truncates.
const ITEMS_MAX: usize = 512;

/// What project membership grants an item: nothing.
///
/// The marker is set by the board, never read from the snapshot, so an
/// issue that writes "approved" in its own title changes nothing. The
/// admitted set is the coordinator's decision, built from its prepared
/// task map, its capacity book, and its permits — the board is one of
/// its inputs, not its source of authority.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Authority {
    /// The item is tracker context only; admission is decided elsewhere.
    #[default]
    NoAuthority,
}

/// An issue's open or closed state, when the snapshot said it.
///
/// `Closed` is dependency evidence — the item is done as the tracker
/// reports — and never work: a closed item is not a dispatch candidate,
/// however ready its edges look. An absent state is unknown, not open.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssueState {
    /// The issue is open.
    Open,
    /// The issue is closed — done as the tracker reports it.
    Closed,
}

/// What one dependency edge pins about the blocker's state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeState {
    /// The blocker was open at the fetch.
    Open,
    /// The blocker was closed at the fetch — the edge is satisfied as
    /// the project reported it.
    Closed,
    /// The fetch did not pin the blocker's state — unknown completeness,
    /// never a satisfied dependency.
    Unknown,
}

/// One declared dependency edge: this item waits on `number`.
///
/// `repository` is the `owner/repo` the edge named, when it named one;
/// an edge into a repository the board cannot confirm as the scoped one
/// is outside the observation and blocks the dependent.
/// `state` is the blocker's pinned state as the snapshot reported it.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Edge {
    /// The blocking issue's number.
    pub number: u64,
    /// The repository the edge named; absent means the scoped one.
    #[serde(default)]
    pub repository: Option<String>,
    /// The blocker's pinned state.
    pub state: EdgeState,
}

/// The item's version pin: what *edited mid-flight* diffs against.
///
/// `updated_at` is the item's own `updatedAt`; `content_digest` is the
/// fetched body digest. A snapshot must carry at least one — a pin the
/// fetch cannot name is an incomplete observation — and a later
/// snapshot's differing marker is a [`Change::Version`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Version {
    /// The item's `updatedAt` marker, or empty when the fetch carried none.
    pub updated_at: String,
    /// The item's content digest, or empty when the fetch carried none.
    pub content_digest: String,
}

/// One project item, pinned: who it is, what the board saw, and nothing
/// it did not earn.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Item {
    /// The issue number within the scoped repository.
    pub number: u64,
    /// The issue title as fetched — context, not instructions.
    pub title: String,
    /// The project status field value, exactly as the board reported it.
    /// A readiness label is for people scanning the board; dispatch reads
    /// the dependency edges and the coordinator's own finished set.
    pub status: String,
    /// The item's version pin.
    pub version: Version,
    /// The issue's open or closed state, when the snapshot carried it.
    #[serde(default)]
    pub state: Option<IssueState>,
    /// The declared blockers, deduplicated and ordered. `blocks`
    /// relations the snapshot carries merge in here: whichever direction
    /// the document named an edge from, the dependent's record holds it.
    pub blocked_by: BTreeSet<Edge>,
    /// Membership and text grant nothing; the marker is the board's,
    /// not the tracker's.
    pub authority: Authority,
}

/// A project item the board records but does not carry as an issue —
/// a pull request, a draft — named so the record of what the project
/// held is complete.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Skipped {
    /// The item's identity in the document, or its position.
    pub id: String,
    /// Why it is not an issue this board carries.
    pub reason: String,
}

/// Why an item cannot dispatch, when its own fields are not the reason.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Blockage {
    /// The edge points at an item the snapshot does not contain —
    /// incomplete dependency observation. Missing data refuses.
    MissingDependency {
        /// The blocker number the observation cannot see.
        dependency: u64,
    },
    /// The edge pins no blocker state and the blocker item's own record
    /// does not close it — unknown completeness, never satisfaction.
    UnpinnedDependency {
        /// The blocker whose state the snapshot did not pin.
        dependency: u64,
    },
    /// The blocker is open as the snapshot reports it.
    OpenDependency {
        /// The blocker still open.
        dependency: u64,
    },
}

/// An item that became dispatchable in a refill round.
///
/// The admission is the board's claim — *the project makes this item
/// ready and a slot is free* — not a grant to run. The coordinator still
/// applies its own prepared task map, capacity, and permit checks before
/// anything executes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Admission {
    /// The issue number entering the dispatchable set.
    pub issue: u64,
    /// The item's title at the pin.
    pub title: String,
    /// The item's status field value at the pin.
    pub status: String,
    /// The item's version pin — the dispatch pins what it saw.
    pub version: Version,
    /// The declared blockers this admission counted as satisfied, in
    /// number order, so the record says what the decision rested on.
    pub satisfied: Vec<u64>,
}

/// One way a later snapshot disagrees with the pinned board.
///
/// A coordinator reconciles this list before it dispatches: an empty
/// list is the only green light, and every entry names the issue and
/// the field that moved.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Change {
    /// The item appeared after the board was pinned.
    Added {
        /// The new item's issue number.
        issue: u64,
    },
    /// The pinned item is gone from the project.
    Removed {
        /// The missing item's issue number.
        issue: u64,
    },
    /// The status field value moved.
    Status {
        /// The item's issue number.
        issue: u64,
        /// The pinned value.
        from: String,
        /// The value the later snapshot carries.
        to: String,
    },
    /// The version pin drifted — the item was edited mid-flight.
    Version {
        /// The item's issue number.
        issue: u64,
        /// The pinned markers.
        from: Version,
        /// The markers the later snapshot carries.
        to: Version,
    },
    /// The item's declared blockers changed.
    Edges {
        /// The dependent item's issue number.
        issue: u64,
        /// Blockers the later snapshot added.
        added: Vec<Edge>,
        /// Blockers the pinned board carried and the later snapshot dropped.
        removed: Vec<Edge>,
    },
}

/// The pinned board: every item the fetch claimed, in the project's order.
#[derive(Debug)]
pub struct Board {
    items: BTreeMap<u64, Item>,
    order: Vec<u64>,
    skipped: Vec<Skipped>,
    exclusions: BTreeSet<u64>,
    repository: Option<String>,
}

impl Board {
    /// Parse a supplied item-list snapshot into a pinned board.
    ///
    /// The caller fetches; this is the deterministic half. The document
    /// is an object carrying `items` — also resolved through a GraphQL
    /// `data.organization.projectV2.items` answer — or the bare item
    /// list itself. Each item must carry its number, title, status field
    /// value, and a version pin (`updatedAt`, or a content digest when
    /// the fetch carried no timestamp); anything less is an incomplete
    /// observation and the whole parse refuses, because a board that
    /// cannot pin what it read is worse than no board.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the document is not a board this
    /// host reads: no item list, an item past the bound, an item missing
    /// a pinning field, an issue number pinned twice, an item that
    /// depends on itself, or a dependency that names no issue.
    pub fn from_snapshot(value: &Value) -> Result<Self, String> {
        let nodes = item_nodes(value)?;
        if nodes.len() > ITEMS_MAX {
            return Err(format!(
                "the item list holds {} items against a bound of {ITEMS_MAX}",
                nodes.len()
            ));
        }
        let repository = snapshot_repository(value);
        let exclusions = snapshot_exclusions(value)?;
        let mut board = Self {
            items: BTreeMap::new(),
            order: Vec::new(),
            skipped: Vec::new(),
            exclusions,
            repository,
        };
        for (position, node) in nodes.iter().enumerate() {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("item {position}"));
            if let Some(kind) = field(node, &["type", "__typename"]).and_then(Value::as_str)
                && !kind.eq_ignore_ascii_case("issue")
            {
                board.skipped.push(Skipped {
                    id,
                    reason: format!("a {kind} project item, not an issue"),
                });
                continue;
            }
            let item = parse_item(node, &id)?;
            let number = item.number;
            if board.items.insert(number, item).is_some() {
                return Err(format!("the snapshot pins issue #{number} twice"));
            }
            board.order.push(number);
        }
        board.merge_blocking(nodes)?;
        Ok(board)
    }

    /// The item one issue number pins, when the board carries it.
    #[must_use]
    pub fn item(&self, issue: u64) -> Option<&Item> {
        self.items.get(&issue)
    }

    /// Every item, in the order the project held them.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.order.iter().map(|number| &self.items[number])
    }

    /// The non-issue items the board recorded rather than carried.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// The exclusions the snapshot document carried — operator-supplied
    /// numbers, honored alongside the set [`Board::ready`] is handed.
    #[must_use]
    pub fn exclusions(&self) -> &BTreeSet<u64> {
        &self.exclusions
    }

    /// The board's digest over everything it pinned. Two boards that
    /// digest the same saw the same project; a different digest is a
    /// claim [`Board::changes`] can enumerate.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!({
            "items": self.items().collect::<Vec<_>>(),
            "skipped": self.skipped,
            "exclusions": self.exclusions,
            "repository": self.repository,
        }))
    }

    /// The items whose declared blockers are all satisfied as the
    /// project itself reports, minus the supplied exclusions.
    ///
    /// `exclusions` is the operator's list — a #9476-style set honored
    /// by number, never read from item text — unioned with any
    /// `exclusions` the snapshot document carried; an exclusion is never
    /// dispatchable however it arrived. A blocker is satisfied only by
    /// positive evidence: the edge pins it closed, or the blocker item's
    /// own record is closed. An edge to an item the snapshot does not
    /// contain is incomplete dependency observation and blocks; an edge
    /// whose blocker state was never pinned blocks the same way.
    /// Answers in project order — the same board, the same list.
    #[must_use]
    pub fn ready(&self, exclusions: &BTreeSet<u64>) -> Vec<u64> {
        let excluded: BTreeSet<u64> = self.exclusions.union(exclusions).copied().collect();
        let finished = BTreeSet::new();
        self.items()
            .filter(|item| {
                !excluded.contains(&item.number)
                    && item.state != Some(IssueState::Closed)
                    && self.blockage(item, &finished).is_none()
            })
            .map(|item| item.number)
            .collect()
    }

    /// The items a freed capacity admits, completion-driven.
    ///
    /// `finished` is the coordinator's newly finished issue numbers —
    /// its own evidence that a blocker is done, counted even while the
    /// tracker still shows the issue open — and `free_slots` is how many
    /// executor places stand open. The answer is the first `free_slots`
    /// dispatchable items in project order: the next ready item enters
    /// when a slot frees, never waiting for a whole batch. A finished or
    /// excluded item never enters; a closed item is evidence, not work;
    /// an item blocked by missing or unpinned dependency observation
    /// waits, because the board refuses what it cannot see.
    #[must_use]
    pub fn refill(&self, free_slots: u32, finished: &BTreeSet<u64>) -> Vec<Admission> {
        let mut admissions = Vec::new();
        for item in self.items() {
            if admissions.len() >= free_slots as usize {
                break;
            }
            if finished.contains(&item.number)
                || self.exclusions.contains(&item.number)
                || item.state == Some(IssueState::Closed)
            {
                continue;
            }
            if self.blockage(item, finished).is_none() {
                admissions.push(Admission {
                    issue: item.number,
                    title: item.title.clone(),
                    status: item.status.clone(),
                    version: item.version.clone(),
                    satisfied: item.blocked_by.iter().map(|edge| edge.number).collect(),
                });
            }
        }
        admissions
    }

    /// What a later board moved against this pinned one.
    ///
    /// `after` is the board a later fetch built. The answer names every
    /// status move, every version drift — an item edited mid-flight —
    /// every added or removed item, and every dependency edge that
    /// changed, so the coordinator reconciles the project before it
    /// dispatches rather than dispatching a stale board. An empty answer
    /// is the only green light. Entries answer in issue-number order:
    /// the same pair of boards yields the same list.
    #[must_use]
    pub fn changes(&self, after: &Board) -> Vec<Change> {
        let mut changes = Vec::new();
        for (number, before) in &self.items {
            let Some(now) = after.items.get(number) else {
                changes.push(Change::Removed { issue: *number });
                continue;
            };
            if before.status != now.status {
                changes.push(Change::Status {
                    issue: *number,
                    from: before.status.clone(),
                    to: now.status.clone(),
                });
            }
            if before.version != now.version {
                changes.push(Change::Version {
                    issue: *number,
                    from: before.version.clone(),
                    to: now.version.clone(),
                });
            }
            if before.blocked_by != now.blocked_by {
                changes.push(Change::Edges {
                    issue: *number,
                    added: now
                        .blocked_by
                        .difference(&before.blocked_by)
                        .cloned()
                        .collect(),
                    removed: before
                        .blocked_by
                        .difference(&now.blocked_by)
                        .cloned()
                        .collect(),
                });
            }
        }
        for number in &after.order {
            if !self.items.contains_key(number) {
                changes.push(Change::Added { issue: *number });
            }
        }
        changes
    }

    /// Why an item cannot dispatch on dependency grounds, or `None`
    /// when every declared blocker is satisfied.
    ///
    /// `finished` is the same coordinator evidence [`Board::refill`]
    /// counts. The reason names the first blocker that fails, in edge
    /// order — a missing observation before an unpinned state before an
    /// open blocker only because edge order put it first.
    #[must_use]
    pub fn blocked_reason(&self, issue: u64, finished: &BTreeSet<u64>) -> Option<Blockage> {
        self.items
            .get(&issue)
            .and_then(|item| self.blockage(item, finished))
    }

    /// The first unsatisfied edge on the item, in edge order.
    fn blockage(&self, item: &Item, finished: &BTreeSet<u64>) -> Option<Blockage> {
        item.blocked_by
            .iter()
            .find_map(|edge| self.edge_blockage(edge, finished))
    }

    /// Whether one declared edge is satisfied — and why not, when it is not.
    ///
    /// An edge the board cannot resolve into the pinned items is
    /// missing observation: a blocker in another repository the board
    /// does not carry, or a number no item claims. Of the rest, only
    /// positive done evidence satisfies — the coordinator's `finished`,
    /// the blocker item's own closed state, or the edge's pinned close.
    fn edge_blockage(&self, edge: &Edge, finished: &BTreeSet<u64>) -> Option<Blockage> {
        if let Some(repository) = &edge.repository
            && self.repository.as_deref() != Some(repository.as_str())
        {
            return Some(Blockage::MissingDependency {
                dependency: edge.number,
            });
        }
        let Some(blocker) = self.items.get(&edge.number) else {
            return Some(Blockage::MissingDependency {
                dependency: edge.number,
            });
        };
        if finished.contains(&edge.number)
            || blocker.state == Some(IssueState::Closed)
            || edge.state == EdgeState::Closed
        {
            return None;
        }
        if blocker.state == Some(IssueState::Open) || edge.state == EdgeState::Open {
            return Some(Blockage::OpenDependency {
                dependency: edge.number,
            });
        }
        Some(Blockage::UnpinnedDependency {
            dependency: edge.number,
        })
    }

    /// Fold `blocking` relations into the dependents' `blocked_by`.
    ///
    /// Whichever direction the document named an edge from, the
    /// dependent's record holds it: an item carrying `blocking: [B]`
    /// lands on B's record as a blocker with the *blocker's* pinned
    /// state, since the edge's own `state` described the dependent. An
    /// edge the dependent already carries is not duplicated.
    fn merge_blocking(&mut self, nodes: &[Value]) -> Result<(), String> {
        let mut declared: Vec<(u64, u64)> = Vec::new();
        for node in nodes {
            let Some(number) = field(node, &["number"]).and_then(Value::as_u64) else {
                continue;
            };
            for edge in parse_edges(node, &["blocking", "blocks"], "blocking")? {
                declared.push((number, edge.number));
            }
        }
        for (blocker, dependent) in declared {
            let already_declared = self
                .items
                .get(&dependent)
                .is_some_and(|item| item.blocked_by.iter().any(|edge| edge.number == blocker));
            if already_declared {
                continue;
            }
            let state = self.items.get(&blocker).and_then(|item| item.state).map_or(
                EdgeState::Unknown,
                |state| match state {
                    IssueState::Open => EdgeState::Open,
                    IssueState::Closed => EdgeState::Closed,
                },
            );
            if let Some(item) = self.items.get_mut(&dependent) {
                item.blocked_by.insert(Edge {
                    number: blocker,
                    repository: None,
                    state,
                });
            }
        }
        Ok(())
    }
}

/// The item list a supplied document carries: `items` as an array or a
/// `nodes` connection, or a GraphQL answer's
/// `data.organization.projectV2.items.nodes`. A bare array is the item
/// list itself.
fn item_nodes(value: &Value) -> Result<&[Value], String> {
    if let Some(items) = value.as_array() {
        return Ok(items);
    }
    let document = value
        .as_object()
        .ok_or_else(|| "a snapshot is an object carrying items".to_string())?;
    if let Some(items) = document.get("items") {
        if let Some(list) = items.as_array() {
            return Ok(list);
        }
        if let Some(nodes) = items.get("nodes").and_then(Value::as_array) {
            return Ok(nodes);
        }
        return Err("the snapshot's items are not a list".to_string());
    }
    if let Some(nodes) = document
        .get("data")
        .and_then(|data| data.get("organization"))
        .and_then(|organization| organization.get("projectV2"))
        .and_then(|project| project.get("items"))
        .and_then(|items| items.get("nodes"))
        .and_then(Value::as_array)
    {
        return Ok(nodes);
    }
    Err("the snapshot carries no item list".to_string())
}

/// The scoped repository the document names, as `owner/repo` — from a
/// `repository` string or a `{nameWithOwner}` object.
fn snapshot_repository(value: &Value) -> Option<String> {
    let repository = value.get("repository")?;
    repository
        .as_str()
        .or_else(|| repository.get("nameWithOwner").and_then(Value::as_str))
        .map(str::to_string)
}

/// The operator exclusions the document carries — numbers, never text.
fn snapshot_exclusions(value: &Value) -> Result<BTreeSet<u64>, String> {
    let mut exclusions = BTreeSet::new();
    let Some(list) = value.get("exclusions") else {
        return Ok(exclusions);
    };
    for entry in list
        .as_array()
        .ok_or_else(|| "the snapshot's exclusions are not a list".to_string())?
    {
        let number = entry
            .as_u64()
            .or_else(|| entry.get("number").and_then(Value::as_u64))
            .ok_or_else(|| "an exclusion is an issue number".to_string())?;
        if number == 0 {
            return Err("an exclusion names issue #0".to_string());
        }
        exclusions.insert(number);
    }
    Ok(exclusions)
}

/// A field on an item, resolved through a `content` object when the
/// item does not carry it directly — the shape `gh project item-list`
/// and GraphQL item nodes use.
fn field<'a>(item: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(found) = item.get(*key).filter(|found| !found.is_null()) {
            return Some(found);
        }
    }
    let content = item.get("content").filter(|content| content.is_object())?;
    for key in keys {
        if let Some(found) = content.get(*key).filter(|found| !found.is_null()) {
            return Some(found);
        }
    }
    None
}

/// One item, strict: a missing pinning field is an incomplete
/// observation, reported rather than defaulted.
fn parse_item(node: &Value, id: &str) -> Result<Item, String> {
    let number = field(node, &["number"])
        .and_then(Value::as_u64)
        .filter(|number| *number > 0)
        .ok_or_else(|| {
            format!("project item {id} carries no issue number — an incomplete observation, not a guessable one")
        })?;
    let title = field(node, &["title"])
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .ok_or_else(|| format!("issue #{number} carries no title — an incomplete observation"))?
        .to_string();
    let status = status(node)
        .filter(|status| !status.trim().is_empty())
        .ok_or_else(|| {
            format!("issue #{number} carries no status field value — an incomplete observation")
        })?;
    let version = Version {
        updated_at: field(node, &["updatedAt", "updated_at", "updated"])
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        content_digest: field(
            node,
            &[
                "bodyDigest",
                "body_digest",
                "contentDigest",
                "content_digest",
            ],
        )
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string(),
    };
    if version.updated_at.is_empty() && version.content_digest.is_empty() {
        return Err(format!(
            "issue #{number} carries neither an updated-at nor a content digest — an incomplete observation cannot pin a version"
        ));
    }
    let state = match field(node, &["state"]).and_then(Value::as_str) {
        Some(state) if state.eq_ignore_ascii_case("open") => Some(IssueState::Open),
        Some(state) if state.eq_ignore_ascii_case("closed") => Some(IssueState::Closed),
        _ => None,
    };
    let blocked_by = parse_edges(node, &["blockedBy", "blocked_by"], "blocked-by")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if blocked_by.iter().any(|edge| edge.number == number) {
        return Err(format!("issue #{number} depends on itself"));
    }
    Ok(Item {
        number,
        title,
        status,
        version,
        state,
        blocked_by,
        authority: Authority::NoAuthority,
    })
}

/// The status field value: a `status` string, a `{name}` single-select,
/// or a `fieldValueByName` answer's `name`.
fn status(node: &Value) -> Option<String> {
    let value = field(node, &["status", "fieldValueByName"])?;
    value
        .as_str()
        .or_else(|| value.get("name").and_then(Value::as_str))
        .map(str::to_string)
}

/// The dependency edges one relation carries: an array of numbers or
/// `{number, state, repository}` objects, or a connection's `nodes`. A
/// dependency that cannot name its issue is a refused snapshot, not a
/// dropped edge.
fn parse_edges(node: &Value, keys: &[&str], relation: &str) -> Result<Vec<Edge>, String> {
    let Some(value) = field(node, keys) else {
        return Ok(Vec::new());
    };
    let nodes = if let Some(list) = value.as_array() {
        list
    } else if let Some(nodes) = value.get("nodes").and_then(Value::as_array) {
        nodes
    } else {
        return Err(format!("a {relation} relation is not a list"));
    };
    let mut edges = Vec::new();
    for dependency in nodes {
        let dependency = dependency.get("node").unwrap_or(dependency);
        let number = if let Some(number) = dependency.as_u64() {
            number
        } else {
            dependency
                .get("number")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("a {relation} dependency carries no issue number"))?
        };
        if number == 0 {
            return Err(format!("a {relation} dependency names issue #0"));
        }
        let state = match dependency.get("state").and_then(Value::as_str) {
            Some(state) if state.eq_ignore_ascii_case("closed") => EdgeState::Closed,
            Some(state) if state.eq_ignore_ascii_case("open") => EdgeState::Open,
            _ => EdgeState::Unknown,
        };
        let repository = dependency
            .get("repository")
            .and_then(|repository| {
                repository
                    .as_str()
                    .or_else(|| repository.get("nameWithOwner").and_then(Value::as_str))
            })
            .or_else(|| dependency.get("repo").and_then(Value::as_str))
            .map(str::to_string);
        edges.push(Edge {
            number,
            repository,
            state,
        });
    }
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = r#"{
        "repository": "Example/public",
        "exclusions": [9476],
        "items": [
            {"number": 9476, "title": "Separately owned", "status": "In progress",
             "updatedAt": "2026-09-20T00:00:00Z", "state": "open"},
            {"number": 9504, "title": "Authority", "status": "Done",
             "updatedAt": "2026-09-20T01:00:00Z", "state": "closed"},
            {"number": 9507, "title": "Intake", "status": "Ready",
             "updatedAt": "2026-09-20T02:00:00Z", "state": "open",
             "blockedBy": [{"number": 9504, "state": "closed"}]},
            {"number": 9514, "title": "Polling and refill", "status": "Ready",
             "updatedAt": "2026-09-20T03:00:00Z", "state": "open",
             "blockedBy": [{"number": 9507, "state": "open"}]},
            {"number": 9520, "title": "Blocked by the unseen", "status": "Ready",
             "updatedAt": "2026-09-20T04:00:00Z", "state": "open",
             "blockedBy": [{"number": 9600, "state": "closed"}]}
        ]
    }"#;

    fn board() -> Board {
        Board::from_snapshot(&serde_json::from_str(SNAPSHOT).unwrap()).unwrap()
    }

    #[test]
    fn ready_respects_satisfied_dependencies_and_exclusions() {
        let board = board();
        assert_eq!(board.ready(&BTreeSet::new()), [9507]);
        assert!(
            !board.ready(&BTreeSet::new()).contains(&9476),
            "the document's exclusions hold without being supplied twice"
        );
        assert!(board.ready(&BTreeSet::from([9507])).is_empty());
        assert_eq!(
            board.blocked_reason(9514, &BTreeSet::new()),
            Some(Blockage::OpenDependency { dependency: 9507 }),
            "9514 waits on open 9507"
        );
    }

    #[test]
    fn refill_admits_on_completion_without_waiting_for_batch_mates() {
        let board = board();
        let finished = BTreeSet::from([9507]);
        let admissions = board.refill(1, &finished);
        assert_eq!(
            admissions.len(),
            1,
            "one free slot admits the next ready item"
        );
        assert_eq!(admissions[0].issue, 9514);
        assert_eq!(admissions[0].satisfied, [9507]);
        assert!(
            board.refill(3, &finished).iter().all(|a| a.issue != 9507),
            "a finished issue never re-enters"
        );
        assert!(
            board.refill(3, &finished).iter().all(|a| a.issue != 9476),
            "an excluded issue never enters"
        );
        assert!(
            board.refill(0, &finished).is_empty(),
            "no free slot admits nothing"
        );
    }

    #[test]
    fn a_dependency_on_an_item_the_snapshot_does_not_contain_blocks() {
        let board = board();
        assert!(!board.ready(&BTreeSet::new()).contains(&9520));
        assert_eq!(
            board.blocked_reason(9520, &BTreeSet::from([9600])),
            Some(Blockage::MissingDependency { dependency: 9600 }),
            "an edge the observation cannot see refuses even finished evidence"
        );
        assert!(
            board
                .refill(6, &BTreeSet::from([9600]))
                .iter()
                .all(|a| a.issue != 9520),
            "missing data refuses; it does not optimistically dispatch"
        );
        let cross_repository = serde_json::json!({"items": [{
            "number": 1, "title": "Dependent", "status": "Ready",
            "updatedAt": "v1",
            "blockedBy": [{"number": 2, "state": "closed", "repository": "Other/elsewhere"}]
        }]});
        let board = Board::from_snapshot(&cross_repository).unwrap();
        assert_eq!(
            board.blocked_reason(1, &BTreeSet::new()),
            Some(Blockage::MissingDependency { dependency: 2 }),
            "a dependency outside the observed repository blocks"
        );
    }

    #[test]
    fn missing_pinning_fields_are_incomplete_observations() {
        for mutation in [
            serde_json::json!({"items": [{"title": "No number", "status": "Ready", "updatedAt": "v"}]}),
            serde_json::json!({"items": [{"number": 1, "status": "Ready", "updatedAt": "v"}]}),
            serde_json::json!({"items": [{"number": 1, "title": "No status", "updatedAt": "v"}]}),
            serde_json::json!({"items": [{"number": 1, "title": "No pin", "status": "Ready"}]}),
        ] {
            let error = Board::from_snapshot(&mutation).unwrap_err();
            assert!(error.contains("observation"), "{error}");
        }
        let content_digest_pin = serde_json::json!({"items": [
            {"number": 1, "title": "Digest-pinned", "status": "Ready", "bodyDigest": "abc123"}
        ]});
        let board = Board::from_snapshot(&content_digest_pin).unwrap();
        assert_eq!(board.item(1).unwrap().version.content_digest, "abc123");
    }

    #[test]
    fn changes_name_status_version_membership_and_edge_drift() {
        let before = board();
        let after_document = serde_json::json!({
            "repository": "Example/public",
            "exclusions": [9476],
            "items": [
                {"number": 9476, "title": "Separately owned", "status": "Done",
                 "updatedAt": "2026-09-21T00:00:00Z", "state": "open"},
                {"number": 9504, "title": "Authority", "status": "Done",
                 "updatedAt": "2026-09-20T01:00:00Z", "state": "closed"},
                {"number": 9507, "title": "Intake", "status": "In progress",
                 "updatedAt": "2026-09-20T02:00:00Z", "state": "open",
                 "blockedBy": [{"number": 9504, "state": "closed"}]},
                {"number": 9514, "title": "Polling and refill", "status": "Ready",
                 "updatedAt": "2026-09-21T03:00:00Z", "state": "open",
                 "blockedBy": [{"number": 9507, "state": "open"}, {"number": 9504, "state": "closed"}]},
                {"number": 9601, "title": "Appeared later", "status": "Ready", "updatedAt": "v9"}
            ]
        });
        let after = Board::from_snapshot(&after_document).unwrap();
        let changes = before.changes(&after);
        assert!(
            changes.contains(&Change::Removed { issue: 9520 }),
            "{changes:?}"
        );
        assert!(
            changes.contains(&Change::Added { issue: 9601 }),
            "{changes:?}"
        );
        assert!(
            changes.contains(&Change::Status {
                issue: 9507,
                from: "Ready".into(),
                to: "In progress".into()
            }),
            "{changes:?}"
        );
        assert!(
            changes.contains(&Change::Status {
                issue: 9476,
                from: "In progress".into(),
                to: "Done".into()
            }),
            "{changes:?}"
        );
        assert!(
            changes
                .iter()
                .any(|change| matches!(change, Change::Version { issue: 9514, .. })),
            "an edited item is version drift: {changes:?}"
        );
        assert!(
            changes.iter().any(|change| matches!(
                change,
                Change::Edges { issue: 9514, added, .. } if added.iter().any(|e| e.number == 9504)
            )),
            "a new declared blocker is an edge change: {changes:?}"
        );
        assert!(
            before.changes(&before).is_empty(),
            "a board never diffs itself"
        );
    }

    #[test]
    fn the_same_snapshot_builds_the_same_board() {
        let first = board();
        let second = Board::from_snapshot(&serde_json::from_str(SNAPSHOT).unwrap()).unwrap();
        assert_eq!(first.digest(), second.digest());
        let exclusions = BTreeSet::from([9476]);
        assert_eq!(first.ready(&exclusions), second.ready(&exclusions));
        let finished = BTreeSet::from([9507]);
        assert_eq!(first.refill(2, &finished), second.refill(2, &finished));
        assert_eq!(first.changes(&second), Vec::<Change>::new());
    }

    #[test]
    fn membership_and_text_grant_no_authority() {
        let board = board();
        assert!(
            board
                .items()
                .all(|item| item.authority == Authority::NoAuthority),
            "every item is tracker context, never execution authority"
        );
    }

    #[test]
    fn graphql_nodes_and_blocking_relations_parse() {
        let document = serde_json::json!({
            "data": {"organization": {"projectV2": {"items": {"nodes": [
                {"id": "PVTI_a", "fieldValueByName": {"name": "Ready"},
                 "content": {"__typename": "Issue", "number": 10, "title": "Blocker",
                             "state": "open", "updatedAt": "v1",
                             "blocking": {"nodes": [{"number": 11, "state": "open"}]}}},
                {"id": "PVTI_b", "fieldValueByName": {"name": "Ready"},
                 "content": {"__typename": "Issue", "number": 11, "title": "Dependent",
                             "state": "open", "updatedAt": "v2"}},
                {"id": "PVTI_c",
                 "content": {"__typename": "PullRequest", "number": 12, "title": "A pull request"}}
            ]}}}}
        });
        let board = Board::from_snapshot(&document).unwrap();
        assert_eq!(
            board.skipped().len(),
            1,
            "the pull request is recorded, not carried"
        );
        assert!(board.ready(&BTreeSet::new()).contains(&10));
        assert_eq!(
            board.blocked_reason(11, &BTreeSet::new()),
            Some(Blockage::OpenDependency { dependency: 10 }),
            "a blocks relation lands on the dependent's record"
        );
        assert!(
            board
                .refill(2, &BTreeSet::from([10]))
                .iter()
                .any(|a| a.issue == 11),
            "finishing the blocker refills the dependent"
        );
    }
}
