//! One retained scheduler state and RUN history per independently admitted plan.
use crate::Result;
use nostr::domain::{Event, RelaySigner};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::Artifacts;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub owners: BTreeSet<String>,
    pub coordinator: Value,
    pub worker: String,
    pub checker: String,
    pub max_plans: u64,
    pub until: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub reference: Value,
    pub body: Value,
    pub original: Event,
    pub state: Value,
    pub history: Vec<Value>,
    pub journal: Vec<Value>,
    pub occurrences: BTreeMap<String, Value>,
    pub outcomes: BTreeMap<String, Value>,
    pub polls: BTreeMap<String, Value>,
    pub evaluations: BTreeMap<String, Value>,
    pub controls: BTreeMap<String, Value>,
    pub skips: Vec<Value>,
    pub next_slot: u64,
    pub accounting: standing_jobs::ledger::Ledger,
    pub active: Option<String>,
    pub charged_requests: BTreeMap<String, Value>,
    pub source_revisions: BTreeSet<String>,
    pub pending: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub v: String,
    pub authority: String,
    pub policy: Policy,
    pub policy_ref: Value,
    pub plans: BTreeMap<String, Record>,
    pub originals: BTreeMap<String, Vec<Event>>,
    pub service: nostr::execution::Service,
    pub replies: BTreeMap<String, Event>,
    pub reply_artifacts: BTreeMap<String, Vec<Value>>,
}
pub struct Automation {
    pub(crate) store: Artifacts,
    pub(crate) secret: SecretKey,
    pub(crate) signer: RelaySigner,
    pub(crate) coordinator: coordination_host::book::Coordinator,
}
impl Automation {
    pub fn initialize(
        root: PathBuf,
        secret: SecretKey,
        policy: Policy,
        coordinator: coordination_host::book::Coordinator,
    ) -> Result<Self> {
        let host = Self::open(root, secret, coordinator)?;
        let _guard = host.store.lock()?;
        if host.store.root().join("state.json").exists() {
            return Err("automation store already exists".into());
        }
        if policy.max_plans == 0
            || policy.max_plans > 64
            || policy.owners.is_empty()
            || policy.owners.len() > 64
        {
            return Err("finite automation owner policy required".into());
        }
        for key in policy
            .owners
            .iter()
            .chain([&policy.worker, &policy.checker])
        {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "automation principal")?;
        }
        if BTreeSet::from([
            policy.worker.as_str(),
            policy.checker.as_str(),
            host.coordinator.authority(),
            host.authority(),
        ])
        .len()
            != 4
        {
            return Err("independent checker and separate coordinator identity required".into());
        }
        host.coordinator.scope(&policy.coordinator)?;
        let policy_ref=host.store.put_json(&serde_json::json!({"v":"openagents.automation-native-policy.v1","requires":[],"controller":host.authority(),"policy":policy}))?;
        host.save(&State {
            v: "openagents.automation-host.v1".into(),
            authority: host.authority().into(),
            policy,
            policy_ref,
            plans: BTreeMap::new(),
            originals: BTreeMap::new(),
            service: nostr::execution::Service::new(host.authority(), 1, 9_007_199_254_740_991),
            replies: BTreeMap::new(),
            reply_artifacts: BTreeMap::new(),
        })?;
        drop(_guard);
        Ok(host)
    }
    pub fn open(
        root: PathBuf,
        secret: SecretKey,
        coordinator: coordination_host::book::Coordinator,
    ) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|_| "automation key")?;
        Ok(Self {
            store: Artifacts::new(root)?,
            secret,
            signer,
            coordinator,
        })
    }
    pub fn authority(&self) -> &str {
        self.signer.pubkey()
    }
    pub(crate) fn state(&self) -> Result<State> {
        let state: State = self.store.load(&self.signer)?;
        if state.v != "openagents.automation-host.v1" || state.authority != self.authority() {
            return Err("automation store identity differs".into());
        }
        Ok(state)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }
    pub fn put_local_json(&self, body: &Value) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.state()?;
        self.store.put_json(body)
    }
}
