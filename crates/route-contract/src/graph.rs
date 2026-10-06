//! Bounded execution graphs and funded rework (#10703).
//!
//! The flat dispatch plan ([`crate::route::DispatchPlan`]) starts N
//! independent runs. A [`Graph`] adds typed dependencies: each node is one
//! run in its existing execution owner, with its own pinned input and its
//! own admission snapshot (by digest), and may bind artifacts of the nodes
//! it needs. [`GraphRun`] is a projection over the owner's dispositions,
//! not another scheduler: it answers which nodes may dispatch now and
//! records what the owner reports.
//!
//! - Concurrency is bounded by the graph's own `max_parallel`, which the
//!   operator's admission sets; a node dispatches at most once per attempt.
//! - A node is ready only when every node it needs completed with an
//!   independently verified check and the artifacts it binds are
//!   available. A failure, a failed check, a cancellation, an unknown
//!   dispatch, or an unavailable artifact blocks its dependents, each with
//!   its own attributable cause.
//! - Child completion grants nothing: the graph's publication and spending
//!   rights are the ones its admission named, and no node outcome changes
//!   them.
//! - Cancelling the parent stops every node not yet dispatched and returns
//!   the dispatched tasks to cancel through the task owner; those stay
//!   in flight until the owner acknowledges.
//! - Rework starts a new attempt of a failed node only under remaining
//!   authority: an owned-host graph within its attempt bound, a funded
//!   graph within its reservation's remaining sats, or a new explicit
//!   offer. An owned-host graph needs no compute reservation; a paid node
//!   in an owned graph is ineligible.
//! - The summary is a pure read; it starts nothing.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::digest::Digest;
use crate::lifecycle::CheckLabel;

/// A graph's schema.
pub const GRAPH_SCHEMA: &str = "openagents.route.execution-graph.v1";
/// The most nodes one graph holds.
pub const NODES_MAX: usize = 16;
/// The most nodes that run at once.
pub const PARALLEL_MAX: u32 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    /// [`GRAPH_SCHEMA`].
    pub schema: String,
    pub id: String,
    pub nodes: Vec<Node>,
    /// The operator's concurrency bound.
    pub max_parallel: u32,
    /// Attempts each node may make, rework included.
    pub max_attempts: u32,
    pub funding: Funding,
    /// The graph's admission may publish; no node outcome changes this.
    pub publication: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    /// The nodes whose verified results this node needs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    /// The pinned input.
    pub input: Digest,
    /// The node's own admission snapshot.
    pub admission: Digest,
    /// Artifacts of needed nodes this node reads, by node and path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binds: Vec<ArtifactBinding>,
    /// The most this node may charge, in sats; `None` for owned work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sats: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBinding {
    pub node: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Funding {
    /// Work on the person's own hosts: no compute reservation.
    Owned,
    /// A retail reservation held before dispatch, with its bound.
    Reserved { reservation: String, max_sats: u64 },
}

/// Why a graph is not admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    Schema,
    Empty,
    TooMany,
    Parallel,
    Attempts,
    DuplicateNode(String),
    UnknownNeed {
        node: String,
        need: String,
    },
    /// A binding reads a node the binder does not need.
    UnboundArtifact {
        node: String,
        from: String,
    },
    Cycle,
    /// A node charges in an owned graph, or more than the reservation.
    Ineligible(String),
}

impl Graph {
    /// Checks the graph's bounds and shape.
    ///
    /// # Errors
    ///
    /// The first [`Invalid`] found.
    pub fn validate(&self) -> Result<(), Invalid> {
        if self.schema != GRAPH_SCHEMA {
            return Err(Invalid::Schema);
        }
        if self.nodes.is_empty() {
            return Err(Invalid::Empty);
        }
        if self.nodes.len() > NODES_MAX {
            return Err(Invalid::TooMany);
        }
        if self.max_parallel == 0 || self.max_parallel > PARALLEL_MAX {
            return Err(Invalid::Parallel);
        }
        if self.max_attempts == 0 {
            return Err(Invalid::Attempts);
        }
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            if !ids.insert(node.id.as_str()) {
                return Err(Invalid::DuplicateNode(node.id.clone()));
            }
        }
        let mut total: u64 = 0;
        for node in &self.nodes {
            for need in &node.needs {
                if !ids.contains(need.as_str()) || need == &node.id {
                    return Err(Invalid::UnknownNeed {
                        node: node.id.clone(),
                        need: need.clone(),
                    });
                }
            }
            for bind in &node.binds {
                if !node.needs.contains(&bind.node) {
                    return Err(Invalid::UnboundArtifact {
                        node: node.id.clone(),
                        from: bind.node.clone(),
                    });
                }
            }
            match (&self.funding, node.max_sats) {
                (Funding::Owned, Some(_)) => return Err(Invalid::Ineligible(node.id.clone())),
                (_, Some(sats)) => total = total.saturating_add(sats),
                _ => {}
            }
        }
        if let Funding::Reserved { max_sats, .. } = &self.funding
            && total > *max_sats
        {
            return Err(Invalid::Ineligible("the reservation".into()));
        }
        // Kahn's algorithm: every node must be ordered.
        let mut indegree: BTreeMap<&str, usize> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.needs.len()))
            .collect();
        let mut ready: Vec<&str> = indegree
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut ordered = 0;
        while let Some(id) = ready.pop() {
            ordered += 1;
            for node in self
                .nodes
                .iter()
                .filter(|n| n.needs.iter().any(|x| x == id))
            {
                let count = indegree.get_mut(node.id.as_str()).expect("known node");
                *count -= 1;
                if *count == 0 {
                    ready.push(node.id.as_str());
                }
            }
        }
        if ordered != self.nodes.len() {
            return Err(Invalid::Cycle);
        }
        Ok(())
    }

    fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

/// One node's state, as the task owner's dispositions project it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeState {
    Waiting,
    Dispatched {
        task: String,
    },
    /// Finished, with the check's label and the artifacts retained.
    Completed {
        task: String,
        check: CheckLabel,
        artifacts: Vec<String>,
    },
    Failed {
        task: String,
        check_failed: bool,
    },
    Cancelled {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<String>,
    },
    /// Whether the dispatch happened is unknown: reconcile this task.
    Unknown {
        task: String,
    },
    /// Never dispatched, because a needed node did not verify or an
    /// artifact it binds is unavailable.
    Blocked {
        by: String,
        cause: String,
    },
}

impl NodeState {
    fn settled(&self) -> bool {
        !matches!(
            self,
            NodeState::Waiting | NodeState::Dispatched { .. } | NodeState::Unknown { .. }
        )
    }
}

/// Why a move is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    UnknownNode,
    /// Not ready: a need is not verified, the parallel bound is reached,
    /// or the graph was cancelled.
    NotReady,
    /// The node already dispatched this attempt.
    AlreadyDispatched,
    /// Rework of a node that did not fail.
    NotFailed,
    /// No remaining attempts, or no remaining funds and no new offer.
    NoAuthority,
}

/// What the owner reports for a dispatched node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
    Completed {
        check: CheckLabel,
        artifacts: Vec<String>,
    },
    Failed,
    Cancelled,
    Unknown,
}

/// How a rework is authorized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authority {
    /// Within the graph's own remaining attempts and funds.
    Remaining { sats: u64 },
    /// A new explicit offer the person confirmed, by digest.
    Offer(Digest),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRun {
    pub graph: Graph,
    pub states: BTreeMap<String, NodeState>,
    pub attempts: BTreeMap<String, u32>,
    /// Sats committed to dispatched paid attempts.
    pub spent_sats: u64,
    pub cancelled: bool,
    /// Rework offers confirmed, by node.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub offers: BTreeMap<String, Vec<Digest>>,
}

impl GraphRun {
    /// Starts tracking `graph`.
    ///
    /// # Errors
    ///
    /// An invalid graph.
    pub fn new(graph: Graph) -> Result<Self, Invalid> {
        graph.validate()?;
        let states = graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), NodeState::Waiting))
            .collect();
        Ok(Self {
            graph,
            states,
            attempts: BTreeMap::new(),
            spent_sats: 0,
            cancelled: false,
            offers: BTreeMap::new(),
        })
    }

    fn in_flight(&self) -> usize {
        self.states
            .values()
            .filter(|state| matches!(state, NodeState::Dispatched { .. }))
            .count()
    }

    fn verified(&self, id: &str) -> bool {
        matches!(
            self.states.get(id),
            Some(NodeState::Completed {
                check: CheckLabel::Verified,
                ..
            })
        )
    }

    /// The nodes that may dispatch now, within the parallel bound.
    #[must_use]
    pub fn ready(&self) -> Vec<String> {
        if self.cancelled {
            return Vec::new();
        }
        let room = (self.graph.max_parallel as usize).saturating_sub(self.in_flight());
        self.graph
            .nodes
            .iter()
            .filter(|node| matches!(self.states.get(&node.id), Some(NodeState::Waiting)))
            .filter(|node| node.needs.iter().all(|need| self.verified(need)))
            .filter(|node| {
                node.binds
                    .iter()
                    .all(|bind| match self.states.get(&bind.node) {
                        Some(NodeState::Completed { artifacts, .. }) => {
                            artifacts.contains(&bind.path)
                        }
                        _ => false,
                    })
            })
            .map(|node| node.id.clone())
            .take(room)
            .collect()
    }

    /// The owner accepted `task` for `node`.
    ///
    /// # Errors
    ///
    /// A node that is not ready or already dispatched.
    pub fn dispatched(&mut self, node: &str, task: &str) -> Result<(), Refused> {
        match self.states.get(node) {
            None => return Err(Refused::UnknownNode),
            Some(NodeState::Waiting) => {}
            Some(_) => return Err(Refused::AlreadyDispatched),
        }
        if !self.ready().iter().any(|id| id == node) {
            return Err(Refused::NotReady);
        }
        // A node reworked under a new offer is funded by that offer, not
        // the graph's reservation.
        let sats = if self.offers.contains_key(node) {
            0
        } else {
            self.graph.node(node).and_then(|n| n.max_sats).unwrap_or(0)
        };
        self.spent_sats = self.spent_sats.saturating_add(sats);
        *self.attempts.entry(node.to_owned()).or_default() += 1;
        self.states.insert(
            node.to_owned(),
            NodeState::Dispatched {
                task: task.to_owned(),
            },
        );
        Ok(())
    }

    /// The owner's report for `node`'s task. Blocks dependents on any
    /// outcome other than a verified completion with its artifacts.
    pub fn report(&mut self, node: &str, report: Report) {
        let task = match self.states.get(node) {
            Some(NodeState::Dispatched { task } | NodeState::Unknown { task }) => task.clone(),
            _ => return,
        };
        let state = match report {
            Report::Completed { check, artifacts } => {
                if check == CheckLabel::CheckFailed {
                    NodeState::Failed {
                        task,
                        check_failed: true,
                    }
                } else {
                    NodeState::Completed {
                        task,
                        check,
                        artifacts,
                    }
                }
            }
            Report::Failed => NodeState::Failed {
                task,
                check_failed: false,
            },
            Report::Cancelled => NodeState::Cancelled { task: Some(task) },
            Report::Unknown => NodeState::Unknown { task },
        };
        self.states.insert(node.to_owned(), state);
        self.propagate();
    }

    /// Blocks every waiting node whose need cannot verify or whose bound
    /// artifact is missing, transitively.
    fn propagate(&mut self) {
        loop {
            let mut changed = false;
            for node in &self.graph.nodes {
                if !matches!(self.states.get(&node.id), Some(NodeState::Waiting)) {
                    continue;
                }
                let mut block = None;
                for need in &node.needs {
                    let cause = match self.states.get(need) {
                        Some(NodeState::Failed {
                            check_failed: true, ..
                        }) => Some("its check failed"),
                        Some(NodeState::Failed { .. }) => Some("it failed"),
                        Some(NodeState::Cancelled { .. }) => Some("it was cancelled"),
                        Some(NodeState::Unknown { .. }) => Some("its dispatch is unknown"),
                        Some(NodeState::Blocked { .. }) => Some("it was blocked"),
                        Some(NodeState::Completed { check, .. })
                            if *check != CheckLabel::Verified =>
                        {
                            Some("it finished without a verified check")
                        }
                        _ => None,
                    };
                    if let Some(cause) = cause {
                        block = Some((need.clone(), cause.to_owned()));
                        break;
                    }
                }
                if block.is_none() {
                    for bind in &node.binds {
                        if let Some(NodeState::Completed { artifacts, .. }) =
                            self.states.get(&bind.node)
                            && !artifacts.contains(&bind.path)
                        {
                            block = Some((
                                bind.node.clone(),
                                format!("artifact {} is unavailable", bind.path),
                            ));
                            break;
                        }
                    }
                }
                if let Some((by, cause)) = block {
                    self.states
                        .insert(node.id.clone(), NodeState::Blocked { by, cause });
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Cancels the graph: every waiting node is cancelled, and the tasks
    /// already dispatched are returned for the task owner to cancel; they
    /// stay dispatched until the owner reports.
    pub fn cancel(&mut self) -> Vec<String> {
        self.cancelled = true;
        let mut tasks = Vec::new();
        for state in self.states.values_mut() {
            match state {
                NodeState::Waiting => *state = NodeState::Cancelled { task: None },
                NodeState::Dispatched { task } | NodeState::Unknown { task } => {
                    tasks.push(task.clone());
                }
                _ => {}
            }
        }
        tasks
    }

    /// A new attempt of a failed node, under `authority`. Its blocked
    /// dependents wait again.
    ///
    /// # Errors
    ///
    /// [`Refused::NotFailed`] for a node that did not fail, and
    /// [`Refused::NoAuthority`] without remaining attempts or funds.
    pub fn rework(&mut self, node: &str, authority: &Authority) -> Result<(), Refused> {
        if self.cancelled {
            return Err(Refused::NoAuthority);
        }
        match self.states.get(node) {
            None => return Err(Refused::UnknownNode),
            Some(NodeState::Failed { .. }) => {}
            Some(_) => return Err(Refused::NotFailed),
        }
        let used = self.attempts.get(node).copied().unwrap_or(0);
        match authority {
            Authority::Remaining { sats } => {
                if used >= self.graph.max_attempts {
                    return Err(Refused::NoAuthority);
                }
                let remaining = match &self.graph.funding {
                    Funding::Owned => {
                        if *sats > 0 {
                            return Err(Refused::NoAuthority);
                        }
                        0
                    }
                    Funding::Reserved { max_sats, .. } => max_sats.saturating_sub(self.spent_sats),
                };
                if *sats > remaining {
                    return Err(Refused::NoAuthority);
                }
            }
            Authority::Offer(digest) => {
                self.offers
                    .entry(node.to_owned())
                    .or_default()
                    .push(digest.clone());
                // A confirmed offer is its own new authority and budget.
                self.attempts.insert(node.to_owned(), 0);
            }
        }
        self.states.insert(node.to_owned(), NodeState::Waiting);
        for state in self.states.values_mut() {
            if matches!(state, NodeState::Blocked { by, .. } if by == node) {
                *state = NodeState::Waiting;
            }
        }
        self.propagate();
        Ok(())
    }

    /// Whether the graph may publish: only what its admission named, never
    /// more because nodes completed.
    #[must_use]
    pub fn may_publish(&self) -> bool {
        self.graph.publication && self.states.values().all(|state| self.verified_state(state))
    }

    fn verified_state(&self, state: &NodeState) -> bool {
        matches!(
            state,
            NodeState::Completed {
                check: CheckLabel::Verified,
                ..
            }
        )
    }

    /// Whether every node reached a state nothing moves on its own.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.states.values().all(NodeState::settled)
    }

    /// Each node's outcome, attributable by node. Reading starts nothing.
    #[must_use]
    pub fn summary(&self) -> Vec<(String, NodeState)> {
        self.graph
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    self.states
                        .get(&node.id)
                        .cloned()
                        .unwrap_or(NodeState::Waiting),
                )
            })
            .collect()
    }
}
