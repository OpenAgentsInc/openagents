//! One durable authority serializes scope changes and participant claims.
use crate::Result;
use nostr::{
    contracts, coordination,
    domain::{Event, RelaySigner},
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::{self, Artifacts};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub recipients: BTreeSet<String>,
    pub checkers: BTreeSet<String>,
    pub calls: u64,
    pub concurrency: u64,
    pub max_claim_seconds: u64,
    pub max_depth: u64,
    pub until: u64,
    pub effects: Value,
    pub parent: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub actor: String,
    pub roles: BTreeSet<String>,
    pub resources: BTreeSet<String>,
    pub targets: BTreeSet<String>,
    pub all: bool,
    pub until: u64,
    pub epoch: u64,
    pub revoked: bool,
    pub reference: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: String,
    pub domain: String,
    pub source: String,
    pub reference: Value,
    pub binding: Value,
    pub owner: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proposal {
    pub reference: Value,
    pub body: Value,
    pub author: String,
    pub fingerprint: String,
    pub closure: BTreeSet<String>,
    pub status: String,
    pub claim: Option<Value>,
    pub executions: Vec<String>,
    pub findings: Vec<Value>,
    pub reuse: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lease {
    pub reference: Value,
    pub body: Value,
    pub actor: String,
    pub domains: BTreeMap<String, String>,
    pub phase: String,
    pub dispatched: bool,
    pub result: Option<Value>,
    pub execution: Option<Event>,
    pub input: Option<Value>,
    pub parent: Option<String>,
    pub reserved_calls: u64,
    pub spent_calls: u64,
    pub unknown: bool,
    pub own_calls: u64,
    pub check: Option<Value>,
    pub check_admission: Option<Value>,
    pub integration: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub fingerprint: String,
    pub result: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scope {
    pub descriptor: Value,
    pub reference: Value,
    pub owner: String,
    pub policy: Policy,
    pub policy_ref: Value,
    pub namespace: Value,
    pub transport: Value,
    pub revision: u64,
    pub token: u64,
    pub frozen: bool,
    pub revoked: bool,
    pub grants: BTreeMap<String, Grant>,
    pub resources: BTreeMap<String, Resource>,
    pub targets: BTreeMap<String, Value>,
    pub proposals: BTreeMap<String, Proposal>,
    pub claims: BTreeMap<String, Lease>,
    pub requests: BTreeMap<String, Request>,
    pub allowance: Value,
    pub calls: u64,
    pub held_calls: u64,
    pub journal: Vec<Value>,
    pub backgrounds: BTreeMap<String, Value>,
    pub frames: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub v: String,
    pub authority: String,
    pub scopes: BTreeMap<String, Scope>,
    pub originals: BTreeMap<String, Vec<Event>>,
    pub service: nostr::execution::Service,
    pub replies: BTreeMap<String, Event>,
    pub reply_artifacts: BTreeMap<String, Vec<Value>>,
}
pub struct Coordinator {
    pub(crate) store: Artifacts,
    pub(crate) secret: SecretKey,
    pub(crate) signer: RelaySigner,
    pub(crate) sources: BTreeMap<String, workspace_host::host::Host>,
}
pub const ROLES: &[&str] = &[
    "read",
    "propose",
    "claim",
    "control",
    "finding",
    "integrate",
    "background",
];
impl Coordinator {
    pub fn initialize(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let host = Self::open(root, secret)?;
        let _guard = host.store.lock()?;
        if host.store.root().join("state.json").exists() {
            return Err("coordinator already exists".into());
        }
        host.save(&State {
            v: "openagents.coordination-host.v1".into(),
            authority: host.signer.pubkey().into(),
            scopes: BTreeMap::new(),
            originals: BTreeMap::new(),
            service: nostr::execution::Service::new(host.authority(), 16, 9_007_199_254_740_991),
            replies: BTreeMap::new(),
            reply_artifacts: BTreeMap::new(),
        })?;
        drop(_guard);
        Ok(host)
    }
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|_| "coordinator key")?;
        Ok(Self {
            store: Artifacts::new(root)?,
            secret,
            signer,
            sources: BTreeMap::new(),
        })
    }
    pub fn authority(&self) -> &str {
        self.signer.pubkey()
    }
    pub(crate) fn state(&self) -> Result<State> {
        let state: State = self.store.load(&self.signer)?;
        if state.v != "openagents.coordination-host.v1" || state.authority != self.authority() {
            return Err("coordinator identity or version differs".into());
        }
        Ok(state)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }
    pub(crate) fn authenticate(
        &self,
        event: &Event,
        now: u64,
        schema: Option<&str>,
    ) -> Result<(String, Value)> {
        if event.kind == nostr::execution::REQUEST_KIND {
            return self.authenticated_cj(event, now, schema);
        }
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.authority()
            || opened.body().issued_at > now.saturating_add(300)
            || now > opened.body().retain_until
            || schema.is_some_and(|schema| opened.artifact().schema.as_deref() != Some(schema))
        {
            return Err("not_admitted: private coordinator request unavailable".into());
        }
        let body = contracts::parse_strict(
            opened
                .inline_bytes()
                .ok_or("inline coordination request required")?,
        )
        .map_err(|e| e.to_string())?;
        if body["v"] != opened.artifact().schema.as_deref().unwrap_or("") {
            return Err("coordination envelope schema differs".into());
        }
        Ok((opened.signer().into(), body))
    }
    pub(crate) fn retain(&self, state: &mut State, event: &Event, body: &Value) -> Result<Value> {
        let reference = self.store.put_json(body)?;
        let originals = state
            .originals
            .entry(
                reference["digest"]
                    .as_str()
                    .ok_or("artifact digest")?
                    .into(),
            )
            .or_default();
        if !originals.iter().any(|original| original.id == event.id) {
            if originals.len() >= 64 {
                return Err("original declaration audience limit exceeded".into());
            }
            originals.push(event.clone());
        }
        Ok(reference)
    }
    pub(crate) fn declare(
        &self,
        state: &mut State,
        body: &Value,
        recipient: &str,
        now: u64,
    ) -> Result<Value> {
        let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
        self.retain(state, &event, body)
    }
    pub(crate) fn check<'a>(
        &self,
        scope: &'a Scope,
        actor: &str,
        role: &str,
        now: u64,
    ) -> Result<&'a Grant> {
        if role == "read" && actor == scope.owner {
            return scope
                .grants
                .get(actor)
                .ok_or_else(|| "owner read record unavailable".into());
        }
        if scope.revoked || scope.frozen || now >= scope.policy.until {
            return Err("revoked: coordination scope unavailable".into());
        }
        scope
            .grants
            .get(actor)
            .filter(|grant| !grant.revoked && now < grant.until && grant.roles.contains(role))
            .ok_or_else(|| "not_admitted: coordination rights unavailable".into())
    }
    pub(crate) fn grant(
        &self,
        actor: &str,
        roles: BTreeSet<String>,
        resources: BTreeSet<String>,
        targets: BTreeSet<String>,
        all: bool,
        until: u64,
        epoch: u64,
    ) -> Result<Grant> {
        actor
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "participant key")?;
        if roles.iter().any(|role| !ROLES.contains(&role.as_str()))
            || resources.len() > 256
            || targets.len() > 64
        {
            return Err("unsupported coordinator grant".into());
        }
        let reference=self.store.put_json(&json!({"v":"openagents.coordination-native-grant.v1","requires":[],"authority":self.authority(),"actor":actor,"roles":roles,"resources":resources,"targets":targets,"all":all,"until":until,"epoch":epoch}))?;
        Ok(Grant {
            actor: actor.into(),
            roles,
            resources,
            targets,
            all,
            until,
            epoch,
            revoked: false,
            reference,
        })
    }
    pub fn create_scope(
        &self,
        policy: Policy,
        resources: Vec<Resource>,
        targets: Vec<Value>,
        now: u64,
    ) -> Result<Value> {
        if policy.calls == 0
            || policy.calls > 4096
            || policy.concurrency == 0
            || policy.concurrency > 64
            || policy.max_claim_seconds == 0
            || policy.max_claim_seconds > 300
            || policy.max_depth == 0
            || policy.max_depth > 64
            || policy.until <= now
            || policy.until > now + 90 * 86_400
            || policy.parent.is_some()
        {
            return Err("unsupported coordinator policy or independent parent reservation".into());
        }
        if policy.recipients.len() > 64 || !policy.recipients.contains(self.authority()) {
            return Err(
                "the native policy must explicitly include the owner and at most 64 recipients"
                    .into(),
            );
        }
        if policy.checkers.is_empty()
            || policy.checkers.len() > 16
            || !policy.checkers.is_subset(&policy.recipients)
        {
            return Err(
                "native policy needs independently selected checker keys within its recipient set"
                    .into(),
            );
        }
        for recipient in &policy.recipients {
            recipient
                .parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "policy recipient key")?;
        }
        contracts::parse_effects(&policy.effects).map_err(|e| e.to_string())?;
        if resources.len() > 256 || targets.is_empty() || targets.len() > 64 {
            return Err("scope namespace or operation limit exceeded".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        if state.scopes.len() >= 16 {
            return Err("coordination scope limit exceeded".into());
        }
        let scope_id = artifact::token();
        let policy_ref=self.store.put_json(&json!({"v":"openagents.coordination-native-policy.v1","requires":[],"owner":self.authority(),"policy":policy}))?;
        let mut namespace = BTreeMap::new();
        for resource in resources {
            let actual = self.resource(
                resource.id.clone(),
                resource.source.clone(),
                resource.reference.clone(),
                now,
            )?;
            if actual.domain != resource.domain || actual.binding != resource.binding {
                return Err("not_admitted: resource must match mounted source owner".into());
            }
            if resource.id.is_empty()
                || resource.id.len() > 1024
                || resource.domain.is_empty()
                || resource.domain.len() > 1024
                || resource.owner != self.authority()
                || namespace.insert(resource.id.clone(), resource).is_some()
            {
                return Err("unsupported or duplicate native resource identity".into());
            }
        }
        let namespace_ref=self.store.put_json(&json!({"v":"openagents.coordination-native-resources.v1","requires":[],"authority":self.authority(),"resources":namespace}))?;
        let mut operations = BTreeMap::new();
        for target in targets {
            let operation = self.operation(&target)?;
            self.effects_subset(&operation["effects"], &policy.effects)?;
            contracts::parse_definition(&target).map_err(|e| e.to_string())?;
            if operations
                .insert(target["id"].as_str().ok_or("operation ID")?.into(), target)
                .is_some()
            {
                return Err("duplicate coordinator operation".into());
            }
        }
        let allowance=self.store.put_json(&json!({"v":"openagents.reservation.v1","requires":[],"id":artifact::token(),"parent":null,"currency":"USD","ceiling_microunits":0,"spent_microunits":0,"unknown":false}))?;
        let transport = self.controller_binding(&scope_id, &policy_ref)?;
        let descriptor = json!({"v":"openagents.coordinator.v1","requires":[],"scope":scope_id,"controller":self.authority(),"epoch":0,"run":artifact::token(),"capability":transport["target"],"policy":policy_ref,"resources":namespace_ref});
        coordination::validate(&descriptor).map_err(|e| e.to_string())?;
        let reference = self.declare(&mut state, &descriptor, self.authority(), now)?;
        let grant = self.grant(
            self.authority(),
            ROLES.iter().map(|role| role.to_string()).collect(),
            BTreeSet::new(),
            BTreeSet::new(),
            true,
            policy.until,
            0,
        )?;
        state.scopes.insert(
            scope_id,
            Scope {
                descriptor,
                reference: reference.clone(),
                owner: self.authority().into(),
                policy,
                policy_ref,
                namespace: namespace_ref,
                transport,
                revision: 0,
                token: 0,
                frozen: false,
                revoked: false,
                grants: BTreeMap::from([(self.authority().into(), grant)]),
                resources: namespace,
                targets: operations,
                proposals: BTreeMap::new(),
                claims: BTreeMap::new(),
                requests: BTreeMap::new(),
                allowance,
                calls: 0,
                held_calls: 0,
                journal: Vec::new(),
                backgrounds: BTreeMap::new(),
                frames: BTreeMap::new(),
            },
        );
        self.save(&state)?;
        Ok(reference)
    }
}
