//! Model groups (#12400): a model too large for one machine runs as stage
//! Pylons, one per GPU, behind one head. The head publishes the group's one
//! routable beacon (NIP-PYLON v2, `group.role: head`): the exact model
//! artifact, one serial slot, the declared deadline and output limit, the
//! group's readiness, and its generation. Each stage publishes a beacon with
//! its layer range and no slots or services, so nothing routes to it.
//!
//! A request reaches a group only by asking for its model ([`Want`]), and
//! only while the head says the group is ready. Nothing here substitutes
//! another model for the one asked for.
//!
//! The group lifecycle (W10, #12397) reports readiness and generation
//! through [`GroupReadiness`]; [`head_beacon`] turns that report into the
//! head's beacon. [`field`] shows each group with its stages for the Pylon
//! Field.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use nostr::pylon::{
    self, BEACON_V2, Beacon, BeaconBook, Freshness, Group, GroupRole, Lane, Layers, Readiness,
    Service, Slots, Status,
};

/// The model a request asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// The exact artifact, `sha256:<64 hex>`: only a service whose model is
    /// `<name>@<artifact>` matches.
    Artifact(String),
    /// A model id: a service whose model is exactly this, or whose name
    /// before `@` is.
    Model(String),
}

impl Want {
    /// Whether `service` serves the model this asks for.
    #[must_use]
    pub fn matches(&self, service: &Service) -> bool {
        let (name, artifact) = match service.model.split_once('@') {
            Some((name, artifact)) => (name, Some(artifact)),
            None => (service.model.as_str(), None),
        };
        match self {
            Self::Artifact(want) => artifact == Some(want.as_str()),
            Self::Model(want) => service.model == *want || name == want,
        }
    }
}

/// Whether `beacon` can take a job for `want` on `lane` at `at`: fresh,
/// online, a free slot, not in `failing`, serving that model, and, for a
/// group head, ready. A stage never can.
#[must_use]
pub fn serves_want<'a>(
    beacon: &'a Beacon,
    at: u64,
    failing: &BTreeSet<String>,
    lane: Lane,
    want: &Want,
) -> Option<&'a Service> {
    if beacon.is_stage()
        || failing.contains(&beacon.address())
        || pylon::freshness(beacon, at) != Freshness::Fresh
        || beacon.status != Status::Online
        || beacon.slots.free == 0
        || beacon
            .group_head()
            .is_some_and(|g| g.readiness != Some(Readiness::Ready))
    {
        return None;
    }
    beacon
        .services
        .iter()
        .find(|s| s.lanes.contains(&lane) && want.matches(s))
}

/// What the group lifecycle reports: whether every stage is loaded and
/// checked, and the generation they agree on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupStatus {
    pub readiness: Readiness,
    pub generation: u64,
}

impl GroupStatus {
    /// A group that has not finished starting.
    #[must_use]
    pub fn starting() -> Self {
        Self {
            readiness: Readiness::Starting,
            generation: 0,
        }
    }
}

/// Where a head reads its group's readiness. W10's lifecycle implements
/// it, or feeds a [`SharedReadiness`].
pub trait GroupReadiness: Send + Sync {
    fn status(&self) -> GroupStatus;
}

impl GroupReadiness for GroupStatus {
    fn status(&self) -> GroupStatus {
        *self
    }
}

/// A readiness report one side sets and the beacon publisher reads.
#[derive(Debug, Clone)]
pub struct SharedReadiness(Arc<Mutex<GroupStatus>>);

impl Default for SharedReadiness {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(GroupStatus::starting())))
    }
}

impl SharedReadiness {
    /// Record the lifecycle's latest report.
    pub fn set(&self, status: GroupStatus) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
    }
}

impl GroupReadiness for SharedReadiness {
    fn status(&self) -> GroupStatus {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What a head declares about its group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSpec {
    /// The group's slug.
    pub id: String,
    /// The served model's name, before `@`.
    pub model: String,
    /// The exact artifact, `sha256:<64 hex>`.
    pub artifact: String,
    pub deadline_secs: u64,
    pub max_output_tokens: u32,
}

/// The head's v2 beacon from the provider's `base` beacon: its identity,
/// class, settlement, and timing, with one serial slot (free unless
/// `busy`), every service renamed to the group's exact model, and the
/// group's current readiness and generation.
#[must_use]
pub fn head_beacon(
    base: &Beacon,
    spec: &GroupSpec,
    readiness: &dyn GroupReadiness,
    busy: bool,
) -> Beacon {
    let status = readiness.status();
    let mut beacon = base.clone();
    beacon.v = BEACON_V2.into();
    beacon.slots = Slots {
        total: 1,
        free: u32::from(!busy),
    };
    for service in &mut beacon.services {
        service.model = format!("{}@{}", spec.model, spec.artifact);
    }
    beacon.group = Some(Group {
        id: spec.id.clone(),
        role: GroupRole::Head,
        generation: status.generation,
        artifact: spec.artifact.clone(),
        readiness: Some(status.readiness),
        deadline_secs: Some(spec.deadline_secs),
        max_output_tokens: Some(spec.max_output_tokens),
        layers: None,
    });
    beacon
}

/// A stage's v2 beacon from the stage Pylon's `base` beacon: no slots, no
/// services, and its layer range under the group's generation.
#[must_use]
pub fn stage_beacon(base: &Beacon, spec: &GroupSpec, generation: u64, layers: Layers) -> Beacon {
    let mut beacon = base.clone();
    beacon.v = BEACON_V2.into();
    beacon.slots = Slots { total: 0, free: 0 };
    beacon.services = Vec::new();
    beacon.group = Some(Group {
        id: spec.id.clone(),
        role: GroupRole::Stage,
        generation,
        artifact: spec.artifact.clone(),
        readiness: None,
        deadline_secs: None,
        max_output_tokens: None,
        layers: Some(layers),
    });
    beacon
}

/// One stage as the field shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageState {
    /// The world state of the stage's Pylon.
    pub pylon: pylon::PylonState,
    pub layers: Layers,
    pub generation: u64,
    /// Whether the stage is on the head's generation.
    pub current: bool,
}

/// One group as the field shows it: its head, when one is held, and its
/// stages in layer order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupState {
    /// `<head key or stage key>:<group id>`: groups are scoped to keys, so
    /// another provider can't add stages to a group it doesn't run.
    pub id: String,
    pub artifact: String,
    /// The head's world state; `None` while no head beacon is held.
    pub head: Option<pylon::PylonState>,
    /// The head's readiness, `None` without a head or when it is stale.
    pub readiness: Option<Readiness>,
    pub generation: Option<u64>,
    pub deadline_secs: Option<u64>,
    pub max_output_tokens: Option<u32>,
    pub stages: Vec<StageState>,
}

/// The groups in `book` at `at`, each head with its stages. A stage joins
/// a head of the same group id and artifact; heads and stages are matched
/// by NIP-OA owner when they have one, else by key, so a stranger's
/// beacons never join another provider's group.
#[must_use]
pub fn field(book: &BeaconBook, at: u64) -> Vec<GroupState> {
    let mut groups: BTreeMap<(String, String, String), GroupState> = BTreeMap::new();
    let scope = |event: &nostr::domain::Event, beacon: &Beacon| {
        pylon::beacon_owner(event)
            .ok()
            .flatten()
            .unwrap_or_else(|| beacon.provider.clone())
    };
    for (event, beacon) in book.iter() {
        let Some(group) = &beacon.group else { continue };
        let key = (
            scope(event, beacon),
            group.id.clone(),
            group.artifact.clone(),
        );
        let state = groups.entry(key.clone()).or_insert_with(|| GroupState {
            id: format!("{}:{}", key.0, key.1),
            artifact: group.artifact.clone(),
            head: None,
            readiness: None,
            generation: None,
            deadline_secs: None,
            max_output_tokens: None,
            stages: Vec::new(),
        });
        let projected = pylon::project(beacon, at, 0);
        match group.role {
            GroupRole::Head => {
                let fresh = pylon::freshness(beacon, at) == Freshness::Fresh;
                state.readiness = group.readiness.filter(|_| fresh);
                state.generation = Some(group.generation);
                state.deadline_secs = group.deadline_secs;
                state.max_output_tokens = group.max_output_tokens;
                state.head = Some(projected);
            }
            GroupRole::Stage => state.stages.push(StageState {
                pylon: projected,
                layers: group.layers.unwrap_or(Layers { from: 0, to: 0 }),
                generation: group.generation,
                current: false,
            }),
        }
    }
    groups
        .into_values()
        .map(|mut state| {
            for stage in &mut state.stages {
                stage.current = state.generation == Some(stage.generation);
            }
            state.stages.sort_by_key(|s| (s.layers.from, s.layers.to));
            state
        })
        .collect()
}

#[cfg(test)]
mod tests;
