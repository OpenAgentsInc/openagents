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

use serde::{Deserialize, Serialize};

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
    if beacon.slots.free == 0 {
        return None;
    }
    serves_want_when_free(beacon, at, failing, lane, want)
}

/// As [`serves_want`], whether or not a slot is free now: what a busy
/// group would serve once its turn ends.
#[must_use]
pub fn serves_want_when_free<'a>(
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

    /// Each stage by name, and whether it is loaded and checked at the
    /// current generation. Empty when the source doesn't know its stages.
    fn stages(&self) -> Vec<(String, bool)> {
        Vec::new()
    }

    /// Why the group stopped for good, once it has. A stopped group
    /// reports `Recovering` forever, so its publisher stops publishing.
    fn stopped(&self) -> Option<String> {
        None
    }

    /// The request sizes and prompt modes every stage serves, once every
    /// stage has announced them (#12408).
    fn limits(&self) -> Option<crate::stage_group::StageLimits> {
        None
    }

    /// Each joined stage by name, with the device, profile, and bundle it
    /// announced (#12418). Empty when the source doesn't know them.
    fn stage_devices(&self) -> Vec<(String, StageDevice)> {
        Vec::new()
    }
}

/// The key a stage beacon's inert `meta` carries its device under.
pub const STAGE_DEVICE: &str = "stage_device";

/// What a stage announced about where it runs (#12418), as its beacon's
/// `meta` carries it, so the Grid can show a group that mixes device types.
/// Nothing routes on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageDevice {
    /// Such as `cuda-sm_120`, `cuda-sm_89`, or `cpu-x86_64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// The device's name, such as `NVIDIA GeForce RTX 4080`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where a GPU stage keeps its routed experts: `device` or `host`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experts: Option<String>,
    /// The numerical profile it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Its verified bundle hash, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
}

/// `beacon` with `device` in its `meta` under [`STAGE_DEVICE`], keeping any
/// other `meta`; without one, with that key removed.
pub fn set_stage_device(beacon: &mut Beacon, device: Option<&StageDevice>) {
    let mut meta = match beacon.meta.take() {
        Some(serde_json::Value::Object(map)) => map,
        Some(other) => {
            beacon.meta = Some(other);
            return;
        }
        None => serde_json::Map::new(),
    };
    match device.and_then(|d| serde_json::to_value(d).ok()) {
        Some(value) => {
            meta.insert(STAGE_DEVICE.into(), value);
        }
        None => {
            meta.remove(STAGE_DEVICE);
        }
    }
    beacon.meta = (!meta.is_empty()).then_some(serde_json::Value::Object(meta));
}

/// The device a stage beacon carries, if any.
#[must_use]
pub fn stage_device(beacon: &Beacon) -> Option<StageDevice> {
    serde_json::from_value(beacon.meta.as_ref()?.get(STAGE_DEVICE)?.clone()).ok()
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

/// The key a head beacon's inert `meta` carries its answer profile under.
pub const ANSWER_PROFILE: &str = "answer_profile";
/// How many recent answers the profile summarizes.
pub const PROFILE_WINDOW: usize = 32;

/// What a group head measured of its own recent answers (W14, #12401):
/// how long the first token takes, how fast the answer then comes, and how
/// long answers run. The head beacon carries it in its inert `meta` as
/// `answer_profile`, beside the deadline and output limit it declares, so a
/// buyer can see how an answer fits that deadline. Nothing routes on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerProfile {
    /// The answers it summarizes, the most recent up to [`PROFILE_WINDOW`].
    pub answers: usize,
    /// The median time from admission to the first token, ms.
    pub first_token_ms: u64,
    /// The median decode rate after the first token, tokens per second.
    pub tokens_per_s: f64,
    /// The median and the longest answer, tokens.
    pub median_tokens: u64,
    pub max_tokens: u64,
    /// The longest time from admission to the last token, ms.
    pub max_answer_ms: u64,
}

/// One answer as the head timed it.
#[derive(Debug, Clone, Copy)]
struct Answered {
    first_token_ms: u64,
    tokens_per_s: f64,
    tokens: u64,
    answer_ms: u64,
}

/// The head's recent answers, which [`AnswerProfile`] summarizes.
#[derive(Debug, Default)]
pub struct AnswerBook {
    answers: Mutex<std::collections::VecDeque<Answered>>,
}

impl AnswerBook {
    /// Record one complete answer.
    pub fn record(&self, first_token_ms: u64, tokens_per_s: f64, tokens: u64, answer_ms: u64) {
        let mut answers = self.answers.lock().unwrap_or_else(|e| e.into_inner());
        if answers.len() == PROFILE_WINDOW {
            answers.pop_front();
        }
        answers.push_back(Answered {
            first_token_ms,
            tokens_per_s,
            tokens,
            answer_ms,
        });
    }

    /// The profile of the recorded answers; `None` before the first.
    #[must_use]
    pub fn profile(&self) -> Option<AnswerProfile> {
        let answers = self.answers.lock().unwrap_or_else(|e| e.into_inner());
        if answers.is_empty() {
            return None;
        }
        fn median<T: Copy + PartialOrd>(mut values: Vec<T>) -> T {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            values[values.len() / 2]
        }
        let rate = median(answers.iter().map(|a| a.tokens_per_s).collect());
        Some(AnswerProfile {
            answers: answers.len(),
            first_token_ms: median(answers.iter().map(|a| a.first_token_ms).collect()),
            tokens_per_s: (rate * 100.0).round() / 100.0,
            median_tokens: median(answers.iter().map(|a| a.tokens).collect()),
            max_tokens: answers.iter().map(|a| a.tokens).max().unwrap_or(0),
            max_answer_ms: answers.iter().map(|a| a.answer_ms).max().unwrap_or(0),
        })
    }
}

/// `beacon` with `profile` in its `meta` under [`ANSWER_PROFILE`], keeping
/// any other `meta` it has; without a profile, with that key removed.
pub fn set_answer_profile(beacon: &mut Beacon, profile: Option<&AnswerProfile>) {
    let mut meta = match beacon.meta.take() {
        Some(serde_json::Value::Object(map)) => map,
        Some(other) => {
            beacon.meta = Some(other);
            return;
        }
        None => serde_json::Map::new(),
    };
    match profile.and_then(|p| serde_json::to_value(p).ok()) {
        Some(value) => {
            meta.insert(ANSWER_PROFILE.into(), value);
        }
        None => {
            meta.remove(ANSWER_PROFILE);
        }
    }
    beacon.meta = (!meta.is_empty()).then_some(serde_json::Value::Object(meta));
}

/// The key a head beacon's inert `meta` carries its request limits under.
pub const REQUEST_LIMITS: &str = "request_limits";

/// `beacon` with the group's checked request `limits` in its `meta` under
/// [`REQUEST_LIMITS`], keeping any other `meta`; without limits, with that
/// key removed.
pub fn set_request_limits(beacon: &mut Beacon, limits: Option<&crate::stage_group::StageLimits>) {
    let mut meta = match beacon.meta.take() {
        Some(serde_json::Value::Object(map)) => map,
        Some(other) => {
            beacon.meta = Some(other);
            return;
        }
        None => serde_json::Map::new(),
    };
    match limits.and_then(|l| serde_json::to_value(l).ok()) {
        Some(value) => {
            meta.insert(REQUEST_LIMITS.into(), value);
        }
        None => {
            meta.remove(REQUEST_LIMITS);
        }
    }
    beacon.meta = (!meta.is_empty()).then_some(serde_json::Value::Object(meta));
}

/// The request limits a head beacon carries, if any.
#[must_use]
pub fn request_limits(beacon: &Beacon) -> Option<crate::stage_group::StageLimits> {
    serde_json::from_value(beacon.meta.as_ref()?.get(REQUEST_LIMITS)?.clone()).ok()
}

/// The answer profile a head beacon carries, if any.
#[must_use]
pub fn answer_profile(beacon: &Beacon) -> Option<AnswerProfile> {
    serde_json::from_value(beacon.meta.as_ref()?.get(ANSWER_PROFILE)?.clone()).ok()
}

/// A stage's v2 beacon from the stage Pylon's `base` beacon: no slots, no
/// services, and its layer range under the group's generation.
#[must_use]
pub fn stage_beacon(base: &Beacon, spec: &GroupSpec, generation: u64, layers: Layers) -> Beacon {
    let mut beacon = base.clone();
    beacon.v = BEACON_V2.into();
    // A stage takes no jobs, so it has no answer profile of its own; it
    // inherits no other stage's device either.
    set_answer_profile(&mut beacon, None);
    set_request_limits(&mut beacon, None);
    set_stage_device(&mut beacon, None);
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
    /// The device it announced, when its beacon carries one (#12418).
    pub device: Option<StageDevice>,
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
                device: stage_device(beacon),
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
