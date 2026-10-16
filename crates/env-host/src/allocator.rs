//! The NIP-ENV allocator. It admits requests, reserves worst-case allowance
//! before any provider effect, signs leases/materializations/states, holds
//! the single execution slot, and retains cleanup obligations until they are
//! confirmed. Nothing here grants workspace, credential, or wallet access.
use crate::{
    Result,
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use nostr::{
    contracts::{self, digest_bytes, jcs},
    domain::{Event, RelaySigner},
    environment as env,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::{Artifacts, token};

/// At most this many leases are retained. Limits stop new admissions; they
/// never discard history.
pub const MAX_LEASES: usize = 64;
mod bootstrap;

/// Host policy. Owners, controllers, and readers are independently
/// authorized here; a request cannot name itself into these sets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub owners: BTreeSet<String>,
    pub controllers: BTreeSet<String>,
    pub readers: BTreeSet<String>,
    /// Host logical provider account.
    pub account: String,
    pub max_ttl_seconds: u64,
    pub max_lifetime_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    fingerprint: String,
    result: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attachment {
    id: String,
    grant: Value,
    request: Value,
    status: String,
    fingerprint: String,
    expires_at: u64,
    participants: BTreeSet<String>,
    admissions: BTreeMap<String, (String, Value)>,
    fencing: BTreeMap<String, (String, Value)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lease {
    id: String,
    owner: String,
    run: String,
    mode: String,
    request: Value,
    request_ref: Value,
    lease_ref: Value,
    reservation_ref: Value,
    reservation_id: String,
    parent_reservation: String,
    currency: String,
    reserved: u64,
    ttl: u64,
    idle: u64,
    not_after: u64,
    allocation: String,
    realized: Option<Realized>,
    resource_ref: Option<Value>,
    revision: u64,
    state_ref: Option<Value>,
    status: String,
    expires_at: u64,
    last_activity_at: u64,
    materialization: Option<Value>,
    attachment: Option<Attachment>,
    history: Vec<Value>,
    entries: Vec<Value>,
    cleanup: Option<Value>,
    cleanup_status: Option<String>,
    #[serde(default)]
    cleanup_intent: Option<Vec<Value>>,
    #[serde(default)]
    bootstrap: Option<Value>,
    run_records: Vec<Value>,
    generation: u64,
    hold_released: bool,
}

impl Lease {
    fn effective_expiry(&self) -> u64 {
        self.expires_at
            .min(self.last_activity_at.saturating_add(self.idle))
            .min(self.not_after)
    }
    fn executor(&self, store: &Artifacts) -> Result<String> {
        let materialization =
            store.json(self.materialization.as_ref().ok_or("no materialization")?)?;
        materialization["participants"]
            .as_array()
            .and_then(|p| p.iter().find(|p| p["role"] == "executor"))
            .and_then(|p| p["principal"].as_str())
            .map(str::to_string)
            .ok_or_else(|| "malformed: executor".into())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub(crate) policy: Policy,
    #[serde(default)]
    pub(crate) transport: Option<crate::transport::Transport>,
    holds: BTreeMap<String, u64>,
    #[serde(default)]
    known: BTreeMap<String, BTreeSet<String>>,
    priors: BTreeMap<String, Prior>,
    signers: BTreeMap<String, BTreeSet<String>>,
    declarations: BTreeMap<String, Value>,
    leases: BTreeMap<String, Lease>,
}

/// A signed request with the exact artifacts it references and any further
/// signed declarations (for example the caller's execute declaration).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub declarations: Vec<Event>,
}

/// What the host operator supplies to materialize a lease.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializeSpec {
    pub participants: Vec<Value>,
    pub enforcement: Value,
    #[serde(default)]
    pub evidence: Vec<Value>,
    #[serde(default)]
    pub bootstrap: Option<Event>,
}

pub struct Allocator {
    pub(crate) store: Artifacts,
    pub(crate) signer: RelaySigner,
    pub(crate) secret: SecretKey,
    provider: RefCell<Box<dyn Provider>>,
    binding: String,
    claims: Option<Box<dyn crate::claims::ClaimAuthority>>,
}

fn refuse(code: &str, detail: &str) -> String {
    format!("{code}: {detail}")
}

/// The shared refusal code at the front of an error, or `unavailable`.
pub fn code(error: &str) -> &'static str {
    const CODES: &[&str] = &[
        "malformed",
        "unsupported_version",
        "unsupported_feature",
        "not_admitted",
        "unavailable",
        "content_unavailable",
        "identity_mismatch",
        "incompatible",
        "revoked",
        "stale",
        "cannot_enforce",
        "limit_exceeded",
        "idempotency_conflict",
        "conflict",
    ];
    let head = error.split(':').next().unwrap_or_default().trim();
    CODES
        .iter()
        .find(|c| **c == head)
        .copied()
        .unwrap_or("unavailable")
}

fn contract(e: contracts::ContractError) -> String {
    format!("{}: {}", e.code.as_str(), e)
}

fn digest_of(reference: &Value) -> &str {
    reference["digest"].as_str().unwrap_or_default()
}

fn same(a: &Value, b: &Value) -> bool {
    !digest_of(a).is_empty() && digest_of(a) == digest_of(b)
}

impl Allocator {
    pub fn initialize(root: PathBuf, secret: SecretKey, policy: Policy) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: allocator state already exists".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        store.save(
            &State {
                policy,
                ..State::default()
            },
            &signer,
        )
    }
    pub fn open(root: PathBuf, secret: SecretKey, provider: Box<dyn Provider>) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let allocator = Self {
            store,
            signer,
            secret,
            binding: String::new(),
            claims: None,
            provider: RefCell::new(provider),
        };
        allocator.state()?;
        let binding = format!(
            "{}:{}",
            allocator.pubkey(),
            allocator.provider.borrow().binding()
        );
        Ok(Self {
            binding,
            ..allocator
        })
    }
    pub fn with_claims(mut self, claims: Box<dyn crate::claims::ClaimAuthority>) -> Self {
        self.claims = Some(claims);
        self
    }
    fn check_claim(
        &self,
        attach: &Value,
        declaration: &Value,
        plaintext: &Value,
        frame: &Value,
        now: u64,
    ) -> Result<()> {
        let reservation = self.body(&attach["reservation"], "openagents.reservation.v1")?;
        self.claims
            .as_ref()
            .ok_or("not_admitted: a current coordinator authority is required")?
            .check(
                &attach["claim"],
                attach["controller"].as_str().ok_or("controller")?,
                attach["worker"].as_str().ok_or("worker")?,
                declaration,
                plaintext,
                &reservation,
                frame,
                now,
            )
    }
    pub(crate) fn allocation_run_used(&self, state: &State, run: &str) -> bool {
        state.leases.values().any(|l| l.run == run)
    }
    pub(crate) fn provider_binding(&self) -> String {
        self.provider.borrow().binding()
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn store(&self) -> &Artifacts {
        &self.store
    }
    pub(crate) fn state(&self) -> Result<State> {
        self.store.load(&self.signer)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }

    /// Sign one allocator artifact: retain exact bytes and a private `3188`
    /// declaration addressed to `recipient`.
    fn sign(&self, state: &mut State, body: &Value, recipient: &str, now: u64) -> Result<Value> {
        env::validate(body).map_err(contract)?;
        let reference = self.store.put_json(body)?;
        self.remember_input(state, recipient, &reference)?;
        let digest = digest_of(&reference).to_string();
        if !state.declarations.contains_key(&digest) {
            let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
            let event_ref = self.store.put(
                &serde_json::to_vec(&event).map_err(|e| e.to_string())?,
                "application/json",
                None,
            )?;
            state.declarations.insert(digest.clone(), event_ref);
            state
                .signers
                .entry(digest)
                .or_default()
                .insert(self.pubkey().into());
        }
        Ok(reference)
    }

    fn remember_input(&self, state: &mut State, principal: &str, reference: &Value) -> Result<()> {
        let known = state.known.entry(principal.into()).or_default();
        if known.len() >= 8192 && !known.contains(digest_of(reference)) {
            return Err(refuse("limit_exceeded", "authorized artifact retention"));
        }
        known.insert(digest_of(reference).to_string());
        Ok(())
    }
    fn check_request_closure(
        &self,
        state: &State,
        owner: &str,
        request: &Value,
        definition: &Value,
    ) -> Result<()> {
        let mut shared = BTreeSet::new();
        if !definition["bootstrap"].is_null() {
            for key in ["target", "lock", "context", "requirements"] {
                let r = if key == "target" {
                    &definition["bootstrap"][key]["artifact"]
                } else {
                    &definition["bootstrap"][key]
                };
                shared.insert(digest_of(r).to_string());
            }
        }
        if !state
            .known
            .get(owner)
            .is_some_and(|k| k.contains(digest_of(&definition["requirements"])))
        {
            return Err(refuse(
                "not_admitted",
                "environment requirements were not supplied or shared",
            ));
        }
        let requirements = self.body(
            &definition["requirements"],
            "openagents.environment-requirements.v1",
        )?;
        let recipients = requirements["recipients"].as_array().ok_or("recipients")?;
        let mut queue = ["definition", "frame", "policy", "reservation", "resource"]
            .iter()
            .filter_map(|k| request.get(*k).filter(|r| !r.is_null()).cloned())
            .map(|r| (r, 0usize))
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut total = 0usize;
        while let Some((reference, depth)) = queue.pop() {
            let parsed = contracts::parse_artifact(&reference).map_err(contract)?;
            if !seen.insert(parsed.digest.clone()) || shared.contains(&parsed.digest) {
                continue;
            }
            if depth > 32 || seen.len() > 256 {
                return Err(refuse("limit_exceeded", "environment artifact graph"));
            }
            if !state
                .known
                .get(owner)
                .is_some_and(|k| k.contains(&parsed.digest))
            {
                return Err(refuse(
                    "not_admitted",
                    "referenced bytes were not supplied or shared with this owner",
                ));
            }
            let bytes = self.bytes(&reference)?;
            total = total
                .checked_add(bytes.len())
                .ok_or("artifact size overflow")?;
            if total > 8 * 1048576 {
                return Err(refuse("limit_exceeded", "environment artifact bytes"));
            }
            if matches!(
                parsed.media_type.as_str(),
                "application/json" | "application/schema+json"
            ) {
                let body = contracts::parse_strict(&bytes).map_err(contract)?;
                if body["v"] == "openagents.evidence.v1" {
                    let evidence = contracts::parse_evidence(&body).map_err(contract)?;
                    if !contracts::may_read(&evidence, self.pubkey())
                        || recipients
                            .iter()
                            .filter_map(Value::as_str)
                            .any(|p| !contracts::may_read(&evidence, p))
                    {
                        return Err(refuse(
                            "not_admitted",
                            "environment evidence excludes a required recipient",
                        ));
                    }
                }
                let mut stack = vec![&body];
                let mut nodes = 0usize;
                while let Some(value) = stack.pop() {
                    nodes += 1;
                    if nodes > 8192 {
                        return Err(refuse("limit_exceeded", "environment artifact structure"));
                    }
                    if contracts::parse_artifact(value).is_ok() {
                        queue.push((value.clone(), depth + 1));
                        continue;
                    }
                    match value {
                        Value::Array(a) => stack.extend(a),
                        Value::Object(o) => stack.extend(
                            o.iter()
                                .filter(|(k, _)| k.as_str() != "meta")
                                .map(|(_, v)| v),
                        ),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn cj_principal(&self, state: &State, principal: &str, input: &Value) -> bool {
        state.policy.owners.contains(principal)
            || state.policy.controllers.contains(principal)
            || state.policy.readers.contains(principal)
            || input["participant"] == principal
                && state.leases.values().any(|l| {
                    l.attachment.as_ref().is_some_and(|a| {
                        a.participants.contains(principal)
                            && (a.grant == input["attachment"]
                                || input["v"] == "openagents.environment-participant-admission.v1")
                    })
                })
    }
    pub(crate) fn note_cj_input(
        &self,
        state: &mut State,
        event: &Event,
        body: &Value,
        principal: &str,
    ) -> Result<Value> {
        let reference = self.store.put_json(body)?;
        let digest = digest_of(&reference).to_string();
        state
            .signers
            .entry(digest.clone())
            .or_default()
            .insert(principal.to_string());
        let event_ref = self.store.put(
            &serde_json::to_vec(event).map_err(|e| e.to_string())?,
            "application/json",
            None,
        )?;
        state
            .declarations
            .entry(format!("{digest}:{principal}"))
            .or_insert(event_ref);
        Ok(reference)
    }
    pub(crate) fn interrupted_cj(
        &self,
        state: &mut State,
        principal: &str,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let reference = self.store.put_json(input)?;
        let lease = state
            .leases
            .values()
            .find(|l| l.request_ref == reference && l.owner == principal)
            .cloned();
        // A missing transport reply says nothing about provider effects. The
        // original lease and reservation, if present, keep their obligations.
        self.result(
            state,
            principal,
            &reference,
            "unknown",
            lease.as_ref(),
            None,
            Some("unavailable"),
            now,
        )
    }
    /// Authenticate a private declaration and retain its exact bytes.
    fn authenticate(
        &self,
        state: &mut State,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value, Value)> {
        if event.kind == nostr::execution::REQUEST_KIND {
            return self.authenticate_cj(state, event, now);
        }
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.pubkey()
            || opened.body().issued_at > now.saturating_add(300)
            || now > opened.body().retain_until
        {
            return Err(refuse(
                "not_admitted",
                "private declaration time or recipient",
            ));
        }
        let bytes = opened
            .inline_bytes()
            .ok_or("malformed: declarations carry their artifact inline")?
            .to_vec();
        let body = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
        let schema = body["v"].as_str().unwrap_or_default().to_string();
        let reference = self.store.put(
            &bytes,
            "application/json",
            Some(schema.as_str()).filter(|s| !s.is_empty()),
        )?;
        let digest = digest_of(&reference).to_string();
        // The same bytes may be signed by several principals; each request
        // is attributed to its own verified signer.
        state
            .signers
            .entry(digest.clone())
            .or_default()
            .insert(opened.signer().into());
        let event_ref = self.store.put(
            &serde_json::to_vec(event).map_err(|e| e.to_string())?,
            "application/json",
            None,
        )?;
        state
            .declarations
            .entry(format!("{digest}:{}", opened.signer()))
            .or_insert(event_ref);
        Ok((opened.signer().into(), body, reference))
    }

    fn bytes(&self, reference: &Value) -> Result<Vec<u8>> {
        self.store
            .get(reference)
            .map_err(|_| refuse("content_unavailable", "referenced bytes are not retained"))
    }
    fn body(&self, reference: &Value, schema: &str) -> Result<Value> {
        let bytes = self.bytes(reference)?;
        if schema.starts_with("openagents.environment-") {
            env::verify(reference, &bytes, schema).map_err(contract)
        } else {
            let value = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
            if value["v"] != schema {
                return Err(refuse("identity_mismatch", schema));
            }
            Ok(value)
        }
    }
    fn signed_by(&self, state: &State, reference: &Value, signer: &str) -> Result<()> {
        if !state
            .signers
            .get(digest_of(reference))
            .is_some_and(|signers| signers.contains(signer))
        {
            return Err(refuse(
                "identity_mismatch",
                "artifact has no retained signature from its expected signer",
            ));
        }
        Ok(())
    }

    /// The typed RUN data each record type requires, with the allocator's
    /// own detail under `environment`.
    fn run_data(&self, lease: &Lease, kind: &str, detail: Value) -> Value {
        let request = digest_of(&lease.request_ref);
        let mut data = match kind {
            "created" => json!({"owner":self.pubkey(),"request":request,"base":null,
                "program":self.binding.clone(),"lock":digest_of(&lease.request["definition"]),
                "context":digest_of(&lease.request["frame"]),"policy":digest_of(&lease.request["policy"]),
                "parent":null,"recipients":[lease.owner]}),
            "admitted" => {
                json!({"enforcement":lease.request["bounds"],"reservations":[lease.reservation_id],
                "deadlines":{"not_after":lease.not_after},"retention":{"retain_until":lease.request["retain_until"]},
                "input":request})
            }
            "dispatched" => json!({"binding":self.binding.clone(),"attempt":1,"input":request,
                "context":null,"effect":"environment","generation":lease.generation}),
            "observed" => {
                json!({"evidence":[],"sources":[],"complete":true,"provenance":"allocator"})
            }
            "resolved" => {
                json!({"outcome":"completed","dispatched":true,"output":null,"receipts":[],
                "usage":{"entries":lease.entries.len()},"verification":"not_run","integration":"not_requested"})
            }
            "cancel_requested" => {
                json!({"requester":self.pubkey(),"scope":"lease","reason":"drain"})
            }
            "unknown" => json!({"attempt":1,"reservation":lease.reservation_id,"evidence":[]}),
            "reconciled" => {
                json!({"previous":null,"evidence":[],"outcome":"reconciled","accounting":null})
            }
            _ => json!({}),
        };
        data["environment"] = detail;
        data
    }

    fn run_record(&self, lease: &mut Lease, kind: &str, detail: Value, now: u64) -> Result<Value> {
        let data = self.run_data(lease, kind, detail);
        let previous = lease.run_records.last().map(|b| b["digest"].clone());
        let record = json!({"run":lease.run,"seq":lease.run_records.len(),"previous":previous,
            "controller":self.pubkey(),"generation":0,"type":kind,
            "subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(contract)?;
        let body =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for body in lease.run_records.iter().chain(std::iter::once(&body)) {
            let record = nostr::run::parse_envelope(body).map_err(contract)?;
            if nostr::run::ingest(&mut journal, record, self.pubkey()).map_err(contract)?
                != nostr::run::Ingest::Applied
            {
                return Err("conflict: allocation RUN did not extend its exact history".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        lease.run_records.push(body);
        Ok(reference)
    }

    fn accounting(&self, state: &mut State, lease: &Lease, now: u64) -> Result<Value> {
        let body = json!({"v":"openagents.environment-accounting.v1","requires":[],
            "reservation":lease.reservation_ref,"currency":lease.currency,"reserved":lease.reserved,
            "entries":lease.entries});
        self.sign(state, &body, &lease.owner, now)
    }

    fn charge(lease: &mut Lease, phase: &str, cost: Cost, receipt: Option<Value>) {
        let (kind, amount) = match cost {
            Cost::Measured(n) => ("measured", json!(n)),
            Cost::Declared(n) => ("declared", json!(n)),
            Cost::Estimated(n) => ("estimated", json!(n)),
            Cost::Unknown => ("unknown", Value::Null),
        };
        let entry = json!({"phase":phase,"kind":kind,"amount":amount,"receipt":receipt});
        // A retried identical receipt is not counted twice.
        if !lease.entries.contains(&entry) {
            lease.entries.push(entry);
        }
    }

    /// Write the next signed state, derived from one exact RUN transition.
    fn transition(
        &self,
        state: &mut State,
        lease: &mut Lease,
        status: &str,
        kind: &str,
        data: Value,
        now: u64,
    ) -> Result<Value> {
        let record = self.run_record(lease, kind, data, now)?;
        let accounting = self.accounting(state, lease, now)?;
        let revision = if lease.state_ref.is_none() {
            0
        } else {
            lease.revision + 1
        };
        lease.status = status.into();
        let body = json!({"v":"openagents.environment-state.v1","requires":[],"lease":lease.lease_ref,
            "revision":revision,"previous":lease.state_ref,"status":status,"expires_at":lease.expires_at,
            "last_activity_at":lease.last_activity_at,"materialization":lease.materialization,
            "attachment":lease.attachment.as_ref().map(|a| a.grant.clone()),"accounting":accounting,
            "cleanup":lease.cleanup,"record":record});
        let reference = self.sign(state, &body, &lease.owner.clone(), now)?;
        lease.revision = revision;
        lease.state_ref = Some(reference.clone());
        lease.history.push(reference.clone());
        Ok(reference)
    }

    #[allow(clippy::too_many_arguments)]
    fn result(
        &self,
        state: &mut State,
        recipient: &str,
        input: &Value,
        status: &str,
        lease: Option<&Lease>,
        value: Option<Value>,
        reason: Option<&str>,
        now: u64,
    ) -> Result<Value> {
        let body = json!({"v":"openagents.environment-result.v1","requires":[],"input":input,
            "status":status,"lease":lease.map(|l| l.lease_ref.clone()),
            "state":lease.and_then(|l| l.state_ref.clone()),"value":value,"reason":reason});
        self.sign(state, &body, recipient, now)?;
        Ok(body)
    }

    fn release_hold(state: &mut State, lease: &mut Lease) {
        if lease.hold_released || lease.entries.iter().any(|e| e["kind"] == "unknown") {
            // Unknown cost is not zero; the hold stays until reconciled.
            return;
        }
        if let Some(held) = state.holds.get_mut(&lease.parent_reservation) {
            *held = held.saturating_sub(lease.reserved);
        }
        lease.hold_released = true;
    }

    /// Admit one signed CJ-carried operation.
    pub fn submit(&self, bundle: &Bundle, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        if bundle.artifacts.len() > 256
            || bundle.declarations.len() > 64
            || bundle.artifacts.values().map(Vec::len).sum::<usize>() > 8 * 1048576
        {
            return Err(refuse("limit_exceeded", "operation input closure"));
        }
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        for declaration in &bundle.declarations {
            self.authenticate(&mut state, declaration, now)?;
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event, now)?;
        if !self.cj_principal(&state, &signer, &body) {
            return Err(refuse(
                "not_admitted",
                "environment sender is not authorized",
            ));
        }
        self.remember_input(&mut state, &signer, &input)?;
        for (digest, bytes) in &bundle.artifacts {
            let reference =
                workspace_host::artifact::reference(bytes, "application/octet-stream", None);
            if reference["digest"] != *digest {
                return Err(refuse("identity_mismatch", "artifact bytes"));
            }
            self.remember_input(&mut state, &signer, &reference)?;
        }
        if let Err(e) = env::validate(&body) {
            let reason = e.code.as_str();
            let result = self.result(
                &mut state,
                &signer,
                &input,
                "refused",
                None,
                None,
                Some(reason),
                now,
            )?;
            self.remember_cj(&mut state, &bundle.event, &result);
            self.save(&state)?;
            return Ok(result);
        }
        let outcome = match body["v"].as_str() {
            Some("openagents.environment-request.v1") => {
                self.request(&mut state, &signer, &body, &input, now)
            }
            Some("openagents.environment-control.v1") => {
                self.control(&mut state, &signer, &body, &input, now)
            }
            Some("openagents.environment-attach.v1") => {
                self.attach(&mut state, &signer, &body, &input, now)
            }
            Some("openagents.environment-participant-admission.v1") => {
                self.admission(&mut state, &signer, &body, &input, now)
            }
            Some("openagents.environment-attachment-control.v1") => {
                self.attachment_control(&mut state, &signer, &body, &input, now)
            }
            Some("openagents.environment-fencing.v1") => {
                self.fencing(&mut state, &signer, &body, &input, now)
            }
            _ => Err(refuse("unsupported_feature", "operation")),
        };
        let result = match outcome {
            Ok(result) => result,
            Err(error) => {
                // Refusals change no lease; reload to drop partial edits but
                // keep the authenticated provenance and the refusal result.
                let mut fresh = self.state()?;
                fresh.signers.extend(state.signers);
                fresh.declarations.extend(state.declarations);
                fresh.known.extend(state.known);
                state = fresh;
                let status = if code(&error) == "conflict" || code(&error) == "idempotency_conflict"
                {
                    "conflict"
                } else {
                    "refused"
                };
                let lease = self
                    .lease_for(&state, &body)
                    .filter(|l| {
                        l.owner == signer
                            || state.policy.controllers.contains(&signer)
                            || state.policy.readers.contains(&signer)
                            || l.attachment
                                .as_ref()
                                .is_some_and(|a| a.participants.contains(&signer))
                    })
                    .cloned();
                self.result(
                    &mut state,
                    &signer,
                    &input,
                    status,
                    lease.as_ref(),
                    None,
                    Some(code(&error)),
                    now,
                )?
            }
        };
        self.remember_cj(&mut state, &bundle.event, &result);
        self.save(&state)?;
        Ok(result)
    }

    fn lease_for<'a>(&self, state: &'a State, body: &Value) -> Option<&'a Lease> {
        let lease = &body["lease"];
        state
            .leases
            .values()
            .find(|l| same(&l.lease_ref, lease))
            .or_else(|| {
                body["attachment"].as_object()?;
                state.leases.values().find(|l| {
                    l.attachment
                        .as_ref()
                        .is_some_and(|a| same(&a.grant, &body["attachment"]))
                })
            })
    }

    fn take_lease(&self, state: &mut State, reference: &Value) -> Result<Lease> {
        let id = state
            .leases
            .values()
            .find(|l| same(&l.lease_ref, reference))
            .map(|l| l.id.clone())
            .ok_or_else(|| refuse("not_admitted", "unknown lease"))?;
        state.leases.remove(&id).ok_or_else(|| "lease".into())
    }

    fn prior(state: &State, key: &str, fingerprint: &str) -> Result<Option<Value>> {
        match state.priors.get(key) {
            Some(prior) if prior.fingerprint == fingerprint => Ok(Some(prior.result.clone())),
            Some(_) => Err(refuse(
                "idempotency_conflict",
                "the same identity arrived with different bytes",
            )),
            None => Ok(None),
        }
    }

    fn request(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let key = format!(
            "request:{signer}:{}",
            body["request"].as_str().unwrap_or_default()
        );
        let fingerprint = digest_of(input).to_string();
        if let Some(result) = Self::prior(state, &key, &fingerprint)? {
            return Ok(result);
        }
        if body["owner"] != signer || !state.policy.owners.contains(signer) {
            return Err(refuse(
                "not_admitted",
                "owner is not authorized by this host",
            ));
        }
        if body["allocator"] != self.pubkey() {
            return Err(refuse("not_admitted", "request names another allocator"));
        }
        if state.leases.values().any(|l| body["run"] == l.run.as_str())
            || state
                .transport
                .as_ref()
                .is_some_and(|t| t.service.claims.values().any(|c| body["run"] == c.run))
        {
            return Err(refuse("conflict", "allocation run is already used"));
        }
        if state.leases.len() >= MAX_LEASES {
            return Err(refuse("limit_exceeded", "lease retention limit"));
        }
        if !state
            .known
            .get(signer)
            .is_some_and(|k| k.contains(digest_of(&body["definition"])))
        {
            return Err(refuse(
                "not_admitted",
                "environment definition was not supplied or shared",
            ));
        }
        let definition = self.body(&body["definition"], "openagents.environment-definition.v1")?;
        if !definition["bootstrap"].is_null() {
            self.validate_bootstrap_plan(state, &definition, body)?;
        }
        self.check_request_closure(state, signer, body, &definition)?;
        self.provider.borrow().admit_request(body, now)?;
        if body["effects"]["spend"] == true {
            return Err(refuse(
                "unsupported_feature",
                "a lease is not a wallet authorization",
            ));
        }
        let parent = contracts::parse_reservation(
            &self.body(&body["reservation"], "openagents.reservation.v1")?,
        )
        .map_err(contract)?;
        let not_after = env::number(body, "not_after").map_err(contract)?;
        let ttl = env::number(body, "ttl_seconds").map_err(contract)?;
        if not_after <= now {
            return Err(refuse("stale", "request deadline has passed"));
        }
        if ttl > state.policy.max_ttl_seconds || not_after - now > state.policy.max_lifetime_seconds
        {
            return Err(refuse("limit_exceeded", "lease exceeds host policy"));
        }
        let worst = env::ceiling(&body["bounds"], "spend_microunits")
            .ok_or_else(|| refuse("cannot_enforce", "spend ceiling"))?;
        let currency = body["bounds"]
            .as_array()
            .and_then(|b| b.iter().find(|b| b["bound"] == "spend_microunits"))
            .and_then(|b| b["currency"].as_str())
            .unwrap_or_default()
            .to_string();
        let held = state.holds.get(&parent.id).copied().unwrap_or(0);
        let mut effective = parent.clone();
        effective.spent_microunits = parent.spent_microunits.map(|s| s.saturating_add(held));
        let child = contracts::Reservation {
            id: token(),
            parent: Some(parent.id.clone()),
            currency: currency.clone(),
            ceiling_microunits: Some(worst),
            spent_microunits: Some(0),
            unknown: false,
        };
        contracts::admit_child(&effective, &child).map_err(contract)?;
        let owner = signer.to_string();
        let realized = if body["mode"] == "adopt" {
            let resource = self.body(&body["resource"], "openagents.environment-resource.v1")?;
            if resource["owner"] != signer
                || resource["allocator"] != self.pubkey()
                || resource["provider"]["id"] != self.binding.as_str()
                || resource["account"] != state.policy.account.as_str()
            {
                return Err(refuse("identity_mismatch", "resource descriptor binding"));
            }
            let ownership = self.bytes(&resource["ownership"])?;
            Some(self.provider.borrow_mut().adopt(
                &state.policy.account,
                resource["resource"].as_str().unwrap_or_default(),
                resource["generation"].as_str().unwrap_or_default(),
                &ownership,
            )?)
        } else {
            None
        };
        *state.holds.entry(parent.id.clone()).or_default() += worst;
        let reservation = json!({"v":"openagents.reservation.v1","requires":[],"id":child.id,
            "parent":parent.id,"currency":currency,"ceiling_microunits":worst,"spent_microunits":0,"unknown":false});
        let reservation_ref = self.store.put_json(&reservation)?;
        let retain_until = env::number(body, "retain_until").map_err(contract)?;
        let id = token();
        let lease_body = json!({"v":"openagents.environment-lease.v1","requires":[],"lease":id,
            "request":input,"allocator":self.pubkey(),"created_at":now,"not_after":not_after,
            "retain_until":retain_until,"reservation":reservation_ref});
        let lease_ref = self.sign(state, &lease_body, &owner, now)?;
        let mut lease = Lease {
            id: id.clone(),
            owner: owner.clone(),
            run: body["run"].as_str().unwrap_or_default().into(),
            mode: body["mode"].as_str().unwrap_or_default().into(),
            request: body.clone(),
            request_ref: input.clone(),
            lease_ref,
            reservation_ref,
            reservation_id: child.id,
            parent_reservation: parent.id,
            currency,
            reserved: worst,
            ttl,
            idle: env::number(body, "idle_seconds").map_err(contract)?,
            not_after,
            allocation: "intended".into(),
            realized: None,
            resource_ref: None,
            revision: 0,
            state_ref: None,
            status: "provisioning".into(),
            expires_at: (now + ttl).min(not_after),
            last_activity_at: now,
            materialization: None,
            attachment: None,
            history: Vec::new(),
            entries: Vec::new(),
            cleanup: None,
            cleanup_status: None,
            cleanup_intent: None,
            bootstrap: None,
            run_records: Vec::new(),
            generation: 0,
            hold_released: false,
        };
        self.run_record(
            &mut lease,
            "created",
            json!({"request":digest_of(input)}),
            now,
        )?;
        self.run_record(&mut lease, "admitted", json!({"reserved":worst}), now)?;
        if let Some(realized) = realized {
            lease.allocation = "created".into();
            self.bind_resource(state, &mut lease, realized, now)?;
            self.transition(
                state,
                &mut lease,
                "provisioning",
                "observed",
                json!({"adopted":true}),
                now,
            )?;
        } else {
            // Durable dispatch intent precedes the provider effect, so a crash
            // here leaves an allocation to reconcile, never a blind retry.
            self.transition(
                state,
                &mut lease,
                "provisioning",
                "dispatched",
                json!({"provider":self.binding.clone(),"key":body["request"]}),
                now,
            )?;
            state.leases.insert(id.clone(), lease.clone());
            self.save(state)?;
            state.leases.remove(&id);
            self.apply_create(state, &mut lease, now)?;
        }
        let status = match lease.status.as_str() {
            "unknown" => "unknown",
            "failed" => "refused",
            _ => "accepted",
        };
        let reason = (status == "refused").then_some("unavailable");
        let result = self.result(
            state,
            &owner,
            input,
            status,
            Some(&lease),
            None,
            reason,
            now,
        )?;
        state.leases.insert(id, lease);
        state.priors.insert(
            key,
            Prior {
                fingerprint,
                result: result.clone(),
            },
        );
        Ok(result)
    }

    fn bind_resource(
        &self,
        state: &mut State,
        lease: &mut Lease,
        realized: Realized,
        now: u64,
    ) -> Result<()> {
        let ownership = self
            .store
            .put(&realized.ownership, "application/json", None)?;
        self.remember_input(state, &lease.owner, &ownership)?;
        let binding = self.binding.clone();
        let provider =
            json!({"id":binding,"artifact":self.store.put(binding.as_bytes(),"text/plain",None)?});
        let descriptor = json!({"v":"openagents.environment-resource.v1","requires":[],
            "owner":lease.owner,"allocator":self.pubkey(),"provider":provider,"account":realized.account,
            "resource":realized.resource,"generation":realized.generation,"ownership":ownership});
        lease.resource_ref = Some(self.sign(state, &descriptor, &lease.owner.clone(), now)?);
        lease.realized = Some(realized);
        Ok(())
    }

    fn apply_create(&self, state: &mut State, lease: &mut Lease, now: u64) -> Result<()> {
        let key = lease.request["request"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let outcome = self
            .provider
            .borrow_mut()
            .create(&state.policy.account, &key);
        match outcome {
            Create::Created(realized, cost) => {
                lease.allocation = "created".into();
                Self::charge(lease, "allocate", cost, None);
                self.bind_resource(state, lease, realized, now)?;
                self.transition(
                    state,
                    lease,
                    "provisioning",
                    "observed",
                    json!({"created":true}),
                    now,
                )?;
            }
            Create::Refused(reason) => {
                lease.allocation = "refused".into();
                Self::charge(lease, "allocate", Cost::Measured(0), None);
                Self::release_hold(state, lease);
                self.transition(
                    state,
                    lease,
                    "failed",
                    "resolved",
                    json!({"refused":reason}),
                    now,
                )?;
            }
            Create::Unknown(reason) => {
                lease.allocation = "unknown".into();
                Self::charge(lease, "allocate", Cost::Unknown, None);
                self.transition(
                    state,
                    lease,
                    "unknown",
                    "unknown",
                    json!({"reason":reason}),
                    now,
                )?;
            }
        }
        Ok(())
    }

    /// Reconcile an allocation whose create response was lost, or retry an
    /// unresolved cleanup by exact identity. It never creates a second resource.
    pub fn reconcile(&self, lease_id: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut lease = state
            .leases
            .remove(lease_id)
            .ok_or_else(|| refuse("not_admitted", "unknown lease"))?;
        if matches!(lease.allocation.as_str(), "intended" | "unknown") {
            let key = lease.request["request"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let found = self
                .provider
                .borrow_mut()
                .lookup(&state.policy.account, &key);
            match found {
                Lookup::Found(realized) => {
                    lease.allocation = "created".into();
                    // Lookup supplies no cost receipt. Keep any unknown charge.
                    Self::charge(&mut lease, "allocate", Cost::Unknown, None);
                    self.bind_resource(&mut state, &mut lease, realized, now)?;
                    self.transition(
                        &mut state,
                        &mut lease,
                        "provisioning",
                        "reconciled",
                        json!({"found":true}),
                        now,
                    )?;
                }
                Lookup::Absent => {
                    lease.allocation = "failed".into();
                    // Lookup supplies no cost receipt. Keep any unknown charge.
                    Self::charge(&mut lease, "allocate", Cost::Unknown, None);
                    Self::release_hold(&mut state, &mut lease);
                    self.transition(
                        &mut state,
                        &mut lease,
                        "failed",
                        "reconciled",
                        json!({"found":false}),
                        now,
                    )?;
                }
                Lookup::Unknown(_) => {}
            }
        } else if matches!(
            lease.cleanup_status.as_deref(),
            Some("unknown" | "failed" | "pending")
        ) {
            state.leases.insert(lease_id.into(), lease);
            self.save(&state)?;
            drop(_lock);
            return self.cleanup(lease_id, now);
        }
        let out = json!({"lease":lease.lease_ref,"state":lease.state_ref,"status":lease.status});
        state.leases.insert(lease_id.into(), lease);
        self.save(&state)?;
        Ok(out)
    }

    /// Place the definition's exact closure, validate every participant and
    /// the enforcement plan, and sign a fresh materialization generation.
    pub fn materialize(&self, lease_id: &str, spec: &MaterializeSpec, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut lease = state
            .leases
            .remove(lease_id)
            .ok_or_else(|| refuse("not_admitted", "unknown lease"))?;
        let outcome = self.materialize_lease(&mut state, &mut lease, spec, now);
        if let Err(error) = &outcome {
            // Failed readiness is retained evidence, not silence.
            if lease.state_ref.is_some() && lease.allocation == "created" {
                let mut fresh = self.state()?;
                let mut retained = fresh.leases.remove(lease_id).ok_or("lease")?;
                self.run_record(
                    &mut retained,
                    "observed",
                    json!({"materialization_refused":code(error)}),
                    now,
                )?;
                fresh.leases.insert(lease_id.into(), retained);
                self.save(&fresh)?;
            }
            return Err(error.clone());
        }
        state.leases.insert(lease_id.into(), lease);
        self.save(&state)?;
        outcome
    }

    fn materialize_lease(
        &self,
        state: &mut State,
        lease: &mut Lease,
        spec: &MaterializeSpec,
        now: u64,
    ) -> Result<Value> {
        if lease.allocation != "created"
            || !matches!(lease.status.as_str(), "provisioning" | "ready")
        {
            return Err(refuse("unavailable", "lease is not materializable"));
        }
        if lease.attachment.is_some() {
            return Err(refuse("conflict", "an attachment holds the lease"));
        }
        if lease.effective_expiry() <= now {
            return Err(refuse("stale", "lease has expired"));
        }
        let definition = self.body(
            &lease.request["definition"],
            "openagents.environment-definition.v1",
        )?;
        let requirements = self.body(
            &definition["requirements"],
            "openagents.environment-requirements.v1",
        )?;
        let mut wanted = Vec::new();
        for component in definition["components"].as_array().into_iter().flatten() {
            wanted.push((
                component["id"].as_str().unwrap_or_default().to_string(),
                self.bytes(&component["artifact"])?,
            ));
        }
        if !definition["workspace"].is_null() {
            wanted.push(("workspace".into(), self.bytes(&definition["workspace"])?));
        }
        let realized = lease.realized.clone().ok_or("realized resource")?;
        let observed: BTreeMap<String, String> = if lease.bootstrap.is_some() {
            self.provider.borrow_mut().observe_components(
                &realized,
                &wanted.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            )?
        } else {
            self.provider.borrow_mut().materialize(&realized, &wanted)?
        }
        .into_iter()
        .collect();
        for (id, bytes) in &wanted {
            if observed.get(id) != Some(&digest_bytes(bytes)) {
                return Err(refuse(
                    "incompatible",
                    "a required component is missing or substituted",
                ));
            }
        }
        let recipients: BTreeSet<&str> = requirements["recipients"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for participant in &spec.participants {
            let principal = participant["principal"].as_str().unwrap_or_default();
            if !recipients.contains(principal) {
                return Err(refuse(
                    "not_admitted",
                    "participant is not an admitted recipient",
                ));
            }
            self.bytes(&participant["assurance"])?;
            self.bytes(&participant["configuration"])?;
            if participant["role"] == "executor"
                && (participant["runtime"] != definition["runtime"]
                    || participant["configuration"] != definition["configuration"])
            {
                {
                    return Err(refuse(
                        "incompatible",
                        "executor runtime or configuration differs",
                    ));
                }
            }
            if participant["role"] != "tool" && participant["model"] != definition["model"] {
                return Err(refuse("incompatible", "participant model differs"));
            }
            if !participant["model"].is_null() {
                let model = self.body(&participant["model"], "openagents.environment-model.v1")?;
                if model["identity"] == "opaque" && requirements["opaque_models"] != true {
                    return Err(refuse(
                        "incompatible",
                        "requirements need stronger model evidence than an opaque identity",
                    ));
                }
            }
        }
        let enforcement_body = json!({"v":"openagents.environment-enforcement.v1","requires":[],
            "plan":spec.enforcement});
        env::validate(&enforcement_body).map_err(contract)?;
        let plan: Vec<contracts::BoundAssignment> = spec
            .enforcement
            .as_array()
            .into_iter()
            .flatten()
            .map(contracts::parse_bound)
            .collect::<std::result::Result<_, _>>()
            .map_err(contract)?;
        for name in requirements["bounds"].as_array().into_iter().flatten() {
            if !spec
                .enforcement
                .as_array()
                .into_iter()
                .flatten()
                .any(|b| b["bound"] == *name)
            {
                return Err(refuse(
                    "cannot_enforce",
                    "a required bound has no mechanism",
                ));
            }
        }
        for (assignment, raw) in plan
            .iter()
            .zip(spec.enforcement.as_array().into_iter().flatten())
        {
            if requirements["assurance"] == "host"
                && assignment.assurance != contracts::Assurance::Host
            {
                return Err(refuse(
                    "cannot_enforce",
                    "requirements need host enforcement",
                ));
            }
            if let Some(limit) = env::ceiling(
                &lease.request["bounds"],
                raw["bound"].as_str().unwrap_or_default(),
            ) && assignment.ceiling > limit
            {
                return Err(refuse("limit_exceeded", "enforcement widens the request"));
            }
        }
        let enforcement = self.sign(state, &enforcement_body, &lease.owner.clone(), now)?;
        let mut components = definition["components"].clone();
        if let Some(list) = components.as_array_mut() {
            list.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        }
        let bootstrap = if definition["bootstrap"].is_null() {
            Value::Null
        } else {
            self.run_bootstrap(
                state,
                lease,
                spec.bootstrap
                    .as_ref()
                    .ok_or("not_admitted: original bootstrap execute required")?,
                now,
            )?
        };
        let body = json!({"v":"openagents.environment-materialization.v1","requires":[],
            "lease":lease.lease_ref,"definition":lease.request["definition"],
            "resource":lease.resource_ref,"generation":token(),"components":components,
            "runtime":definition["runtime"],"configuration":definition["configuration"],
            "model":definition["model"],"workspace":definition["workspace"],
            "participants":spec.participants,"bootstrap":bootstrap,"enforcement":enforcement,
            "evidence":spec.evidence,"observed_at":now});
        let reference = self.sign(state, &body, &lease.owner.clone(), now)?;
        lease.materialization = Some(reference.clone());
        self.transition(
            state,
            lease,
            "ready",
            "observed",
            json!({"materialization":digest_of(&reference)}),
            now,
        )?;
        Ok(json!({"lease":lease.lease_ref,"materialization":reference,"state":lease.state_ref}))
    }

    fn control(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let key = format!(
            "command:{signer}:{}",
            body["command"].as_str().unwrap_or_default()
        );
        let fingerprint = digest_of(input).to_string();
        if let Some(result) = Self::prior(state, &key, &fingerprint)? {
            return Ok(result);
        }
        let mut lease = self.take_lease(state, &body["lease"])?;
        let outcome = self.control_lease(state, &mut lease, signer, body, input, now);
        let id = lease.id.clone();
        state.leases.insert(id.clone(), lease);
        let result = outcome?;
        if body["action"] != "status" {
            state.priors.insert(
                key,
                Prior {
                    fingerprint,
                    result: result.clone(),
                },
            );
        }
        Ok(result)
    }

    fn control_lease(
        &self,
        state: &mut State,
        lease: &mut Lease,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let control = signer == lease.owner || state.policy.controllers.contains(signer);
        let action = body["action"].as_str().unwrap_or_default();
        if !control && !(action == "status" && state.policy.readers.contains(signer)) {
            return Err(refuse("not_admitted", "lease control"));
        }
        if body["expected_revision"] != lease.revision {
            return Err(refuse("conflict", "stale lease revision"));
        }
        match action {
            "status" => {}
            "renew" => {
                if lease.effective_expiry() <= now {
                    return Err(refuse("stale", "an expired lease cannot be renewed"));
                }
                if !matches!(lease.status.as_str(), "ready" | "reserved" | "attached") {
                    return Err(refuse("unavailable", "lease is not renewable"));
                }
                let until = body["requested_until"].as_u64().unwrap_or_default();
                if until <= now || until > lease.not_after.min(now + lease.ttl) {
                    return Err(refuse("limit_exceeded", "renewal exceeds the lease"));
                }
                if lease.hold_released {
                    return Err(refuse("cannot_enforce", "reservation is no longer held"));
                }
                lease.expires_at = until;
                lease.last_activity_at = now;
                let status = lease.status.clone();
                self.transition(
                    state,
                    lease,
                    &status,
                    "observed",
                    json!({"renewed":until}),
                    now,
                )?;
            }
            "release" => {
                if lease.status == "released" {
                    return Err(refuse("conflict", "lease already released"));
                }
                if let Some(attachment) = lease.attachment.as_mut() {
                    attachment.status = "draining".into();
                    self.transition(
                        state,
                        lease,
                        "draining",
                        "cancel_requested",
                        json!({"release":true}),
                        now,
                    )?;
                } else {
                    self.cleanup_lease(state, lease, now)?;
                }
            }
            _ => return Err(refuse("unsupported_feature", "control action")),
        }
        let owner = lease.owner.clone();
        let status = if lease.status == "unknown" {
            "unknown"
        } else {
            "accepted"
        };
        self.result(
            state,
            if control { &owner } else { signer },
            input,
            status,
            Some(lease),
            None,
            None,
            now,
        )
    }

    fn attach(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let declaration = self.body(&body["execute"], "openagents.environment-execute.v1")?;
        let caller = declaration["caller"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        self.signed_by(state, &body["execute"], &caller)?;
        let plaintext = self.body(&declaration["body"], nostr::execution::SCHEMA)?;
        let key = format!(
            "attach:{}:{caller}:{}:{}",
            digest_of(&body["lease"]),
            plaintext["request"].as_str().unwrap_or_default(),
            plaintext["attempt"]
        );
        let fingerprint = digest_of(input).to_string();
        if let Some(result) = Self::prior(state, &key, &fingerprint)? {
            // An exact retry returns the original grant even after changes.
            return Ok(result);
        }
        let mut lease = self.take_lease(state, &body["lease"])?;
        let outcome = self.attach_lease(
            state,
            &mut lease,
            signer,
            body,
            &declaration,
            &plaintext,
            input,
            now,
        );
        state.leases.insert(lease.id.clone(), lease);
        let result = outcome?;
        state.priors.insert(
            key,
            Prior {
                fingerprint,
                result: result.clone(),
            },
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn attach_lease(
        &self,
        state: &mut State,
        lease: &mut Lease,
        signer: &str,
        body: &Value,
        declaration: &Value,
        plaintext: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        if body["controller"] != signer
            || signer != lease.owner && !state.policy.controllers.contains(signer)
        {
            return Err(refuse("not_admitted", "attachment controller"));
        }
        let generation = env::number(body, "generation").map_err(contract)?;
        if generation < lease.generation {
            return Err(refuse("stale", "controller generation is fenced"));
        }
        if lease.status != "ready" || lease.attachment.is_some() {
            return Err(refuse("conflict", "the execution slot is not free"));
        }
        if lease.effective_expiry() <= now {
            return Err(refuse("stale", "lease has expired"));
        }
        if body["expected_revision"] != lease.revision {
            return Err(refuse("conflict", "stale lease revision"));
        }
        if !lease
            .materialization
            .as_ref()
            .is_some_and(|m| same(m, &body["materialization"]))
        {
            return Err(refuse(
                "identity_mismatch",
                "materialization is not current",
            ));
        }
        if body["worker"] != lease.executor(&self.store)?.as_str() {
            return Err(refuse(
                "identity_mismatch",
                "worker is not the primary executor",
            ));
        }
        if declaration["event"]["pubkey"] != declaration["caller"] {
            return Err(refuse(
                "identity_mismatch",
                "execute caller differs from its event signer",
            ));
        }
        if !plaintext["requires"]
            .as_array()
            .is_some_and(|r| r.iter().any(|f| f == env::BINDING_FEATURE))
        {
            return Err(refuse(
                "unsupported_feature",
                "execute lacks the environment binding",
            ));
        }
        if body["input_digest"]
            != contracts::digest_value(&plaintext["input"])
                .map_err(contract)?
                .as_str()
        {
            return Err(refuse("identity_mismatch", "input digest"));
        }
        self.bytes(&body["claim"])?;
        self.check_claim(body, &declaration, plaintext, &lease.request["frame"], now)?;
        let reservation = contracts::parse_reservation(
            &self.body(&body["reservation"], "openagents.reservation.v1")?,
        )
        .map_err(contract)?;
        let resource_child = reservation.id == lease.reservation_id
            || reservation.parent.as_deref() == Some(lease.reservation_id.as_str());
        // A preallocated resource and its later invocation have separate
        // authorities. The live COORD check above verifies the invocation's
        // actual parent. Only explicit zero-dollar holds can be independent;
        // neither immutable reservation is reparented or given spending rights.
        let independent_no_spend = lease.currency == "USD"
            && lease.reserved == 0
            && reservation.ceiling_microunits == Some(0)
            && !reservation.unknown;
        if !resource_child && !independent_no_spend
            || reservation.currency != lease.currency
            || reservation
                .ceiling_microunits
                .is_none_or(|c| c > lease.reserved)
        {
            return Err(refuse(
                "limit_exceeded",
                "attachment reservation amplifies the lease",
            ));
        }
        let deadline = plaintext["deadline"].as_u64().unwrap_or_default();
        if deadline > lease.effective_expiry() {
            return Err(refuse(
                "cannot_enforce",
                "the execute deadline exceeds the current lease",
            ));
        }
        let expires_at = lease.effective_expiry().min(deadline);
        if expires_at <= now {
            return Err(refuse("stale", "execution deadline has passed"));
        }
        let materialization = self
            .store
            .json(lease.materialization.as_ref().ok_or("m")?)?;
        let participants: BTreeSet<String> = materialization["participants"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["principal"].as_str().map(str::to_string))
            .collect();
        let id = token();
        let grant = json!({"v":"openagents.environment-attachment.v1","requires":[],"attachment":id,
            "request":input,"lease":lease.lease_ref,"materialization":body["materialization"],
            "execute":body["execute"],"claim":body["claim"],"reservation":body["reservation"],
            "expires_at":expires_at});
        let grant_ref = self.sign(state, &grant, signer, now)?;
        lease.generation = generation;
        lease.attachment = Some(Attachment {
            id,
            grant: grant_ref.clone(),
            request: body.clone(),
            status: "reserved".into(),
            fingerprint: contracts::digest_value(plaintext).map_err(contract)?,
            expires_at,
            participants,
            admissions: BTreeMap::new(),
            fencing: BTreeMap::new(),
        });
        self.transition(
            state,
            lease,
            "reserved",
            "observed",
            json!({"attachment":digest_of(&grant_ref)}),
            now,
        )?;
        self.result(
            state,
            signer,
            input,
            "accepted",
            Some(lease),
            Some(grant_ref),
            None,
            now,
        )
    }

    fn current_attachment<'a>(
        &self,
        state: &'a mut State,
        attachment: &Value,
    ) -> Result<&'a mut Lease> {
        state
            .leases
            .values_mut()
            .find(|l| {
                l.attachment
                    .as_ref()
                    .is_some_and(|a| same(&a.grant, attachment))
            })
            .ok_or_else(|| refuse("stale", "attachment is not current"))
    }

    fn admission(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        if body["participant"] != signer {
            return Err(refuse("identity_mismatch", "admission signer"));
        }
        let projection = self.body(
            &body["input"],
            "openagents.environment-participant-input.v1",
        )?;
        if projection["participant"] != signer {
            return Err(refuse("identity_mismatch", "projection participant"));
        }
        let lease_id = {
            let lease = self.current_attachment(state, &projection["attachment"])?;
            lease.id.clone()
        };
        let mut lease = state.leases.remove(&lease_id).ok_or("lease")?;
        let outcome = (|| -> Result<Value> {
            let attachment = lease.attachment.as_mut().ok_or("attachment")?;
            if attachment.status != "reserved" {
                return Err(refuse("stale", "attachment no longer admits participants"));
            }
            if !attachment.participants.contains(signer) {
                return Err(refuse(
                    "not_admitted",
                    "participant is not in the materialization",
                ));
            }
            if !lease
                .materialization
                .as_ref()
                .is_some_and(|m| same(m, &body["materialization"]))
            {
                return Err(refuse(
                    "identity_mismatch",
                    "materialization is not current",
                ));
            }
            let status = body["status"].as_str().unwrap_or_default().to_string();
            if status == "admitted" {
                if body["root_input_digest"] != attachment.request["input_digest"]
                    || body["projection_digest"] != projection["projection"]["digest"]
                {
                    return Err(refuse("identity_mismatch", "admitted input identity"));
                }
                self.bytes(&body["enforcement"])?;
            }
            match attachment.admissions.get(signer) {
                Some((_, prior)) if same(prior, input) => {}
                Some(_) => return Err(refuse("conflict", "participant already answered")),
                None => {
                    attachment
                        .admissions
                        .insert(signer.into(), (status, input.clone()));
                }
            }
            Ok(input.clone())
        })();
        let value = match outcome {
            Ok(value) => value,
            Err(e) => {
                state.leases.insert(lease_id, lease);
                return Err(e);
            }
        };
        let result = self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&lease),
            Some(value),
            None,
            now,
        );
        state.leases.insert(lease_id, lease);
        result
    }

    fn attachment_control(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let key = format!(
            "command:{signer}:{}",
            body["command"].as_str().unwrap_or_default()
        );
        let fingerprint = digest_of(input).to_string();
        if let Some(result) = Self::prior(state, &key, &fingerprint)? {
            return Ok(result);
        }
        let lease_id = self
            .current_attachment(state, &body["attachment"])?
            .id
            .clone();
        let mut lease = state.leases.remove(&lease_id).ok_or("lease")?;
        let outcome = (|| -> Result<Value> {
            if signer != lease.owner && !state.policy.controllers.contains(signer) {
                return Err(refuse("not_admitted", "attachment control"));
            }
            if body["expected_revision"] != lease.revision {
                return Err(refuse("conflict", "stale lease revision"));
            }
            let expiry = lease.effective_expiry();
            let attachment = lease.attachment.as_mut().ok_or("attachment")?;
            match body["action"].as_str() {
                Some("activate") => {
                    if attachment.status != "reserved" {
                        return Err(refuse("conflict", "attachment is not reserved"));
                    }
                    if attachment.participants.iter().any(|p| {
                        attachment
                            .admissions
                            .get(p)
                            .is_none_or(|(s, _)| s != "admitted")
                    }) {
                        return Err(refuse("not_admitted", "every participant must admit first"));
                    }
                    if expiry <= now || attachment.expires_at <= now {
                        return Err(refuse("stale", "lease or grant has expired"));
                    }
                    attachment.status = "active".into();
                    lease.last_activity_at = now;
                    self.transition(
                        state,
                        &mut lease,
                        "attached",
                        "dispatched",
                        json!({"activated":true}),
                        now,
                    )?;
                }
                Some("drain") => {
                    if !matches!(attachment.status.as_str(), "reserved" | "active") {
                        return Err(refuse("conflict", "attachment is not draining-capable"));
                    }
                    attachment.status = "draining".into();
                    self.transition(
                        state,
                        &mut lease,
                        "draining",
                        "cancel_requested",
                        json!({"drain":true}),
                        now,
                    )?;
                }
                _ => return Err(refuse("unsupported_feature", "attachment action")),
            }
            let owner = lease.owner.clone();
            self.result(
                state,
                &owner,
                input,
                "accepted",
                Some(&lease),
                None,
                None,
                now,
            )
        })();
        state.leases.insert(lease_id, lease);
        let result = outcome?;
        state.priors.insert(
            key,
            Prior {
                fingerprint,
                result: result.clone(),
            },
        );
        Ok(result)
    }

    fn fencing(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        if body["participant"] != signer {
            return Err(refuse("identity_mismatch", "fencing signer"));
        }
        let lease_id = self
            .current_attachment(state, &body["attachment"])?
            .id
            .clone();
        let mut lease = state.leases.remove(&lease_id).ok_or("lease")?;
        let outcome = (|| -> Result<()> {
            let attachment = lease.attachment.as_mut().ok_or("attachment")?;
            if attachment.status != "draining" {
                return Err(refuse("conflict", "fence only a draining attachment"));
            }
            if !attachment.participants.contains(signer) {
                return Err(refuse("not_admitted", "fencing participant"));
            }
            attachment.fencing.insert(
                signer.into(),
                (
                    body["status"].as_str().unwrap_or_default().into(),
                    input.clone(),
                ),
            );
            let complete = attachment.participants.iter().all(|p| {
                attachment
                    .fencing
                    .get(p)
                    .is_some_and(|(s, _)| s == "fenced")
            });
            if complete {
                // Only reconciled, fenced attachments release the slot.
                lease.attachment = None;
                let status = if lease.effective_expiry() > now {
                    "ready"
                } else {
                    "draining"
                };
                self.transition(
                    state,
                    &mut lease,
                    status,
                    "reconciled",
                    json!({"fenced":true}),
                    now,
                )?;
            } else if attachment.fencing.values().any(|(s, _)| s == "unreachable") {
                self.transition(
                    state,
                    &mut lease,
                    "draining",
                    "unknown",
                    json!({"unreachable":true}),
                    now,
                )?;
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            state.leases.insert(lease_id, lease);
            return Err(e);
        }
        let result = self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&lease),
            None,
            None,
            now,
        );
        state.leases.insert(lease_id, lease);
        result
    }

    /// The dispatcher check: only a currently active exact attachment within
    /// its lease and grant expiry permits a dispatch.
    pub fn check_dispatch(&self, attachment: &str, fingerprint: &str, now: u64) -> Result<()> {
        let started = std::time::Instant::now();
        let state = self.state()?;
        let lease = state
            .leases
            .values()
            .find(|l| l.attachment.as_ref().is_some_and(|a| a.id == attachment))
            .ok_or_else(|| refuse("not_admitted", "no current attachment"))?;
        let current = lease.attachment.as_ref().ok_or("attachment")?;
        if lease.status != "attached" || current.status != "active" {
            return Err(refuse("not_admitted", "attachment is not active"));
        }
        if current.fingerprint != fingerprint {
            return Err(refuse("identity_mismatch", "execute fingerprint"));
        }
        if lease.effective_expiry() <= now || current.expires_at <= now {
            return Err(refuse("stale", "lease or grant has expired"));
        }
        self.provider.borrow_mut().check_current(
            lease
                .realized
                .as_ref()
                .ok_or("unavailable: original environment resource is not observed")?,
        )?;
        let now = now.saturating_add(started.elapsed().as_secs());
        if lease.effective_expiry() <= now || current.expires_at <= now {
            return Err(refuse("stale", "lease expired during resource observation"));
        }
        let declaration = self.body(
            &current.request["execute"],
            "openagents.environment-execute.v1",
        )?;
        let plaintext = self.body(&declaration["body"], nostr::execution::SCHEMA)?;
        self.check_claim(
            &current.request,
            &declaration,
            &plaintext,
            &lease.request["frame"],
            now,
        )?;
        Ok(())
    }

    /// Hold the allocator's current attachment fence through actual worker intake.
    pub fn with_worker_intake(
        &self,
        proof: &coordination_host::worker::VerifiedIntake,
        now: u64,
        dispatch: &mut dyn FnMut() -> Result<coordination_host::worker::Intake>,
    ) -> Result<coordination_host::worker::Intake> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        let state = self.state()?;
        let lease = state
            .leases
            .values()
            .find(|l| {
                l.attachment
                    .as_ref()
                    .is_some_and(|a| a.fingerprint == proof.fingerprint())
            })
            .ok_or("not_admitted: exact active environment attachment required")?;
        let attachment = lease.attachment.as_ref().ok_or("attachment")?;
        let declaration = self.body(
            &attachment.request["execute"],
            "openagents.environment-execute.v1",
        )?;
        if attachment.request["claim"] != *proof.claim()
            || attachment.request["worker"] != proof.actor()
            || declaration["caller"] != proof.authority()
            || declaration["event"]["id"] != proof.request()
        {
            return Err("identity_mismatch: environment worker intake differs".into());
        }
        self.check_dispatch(&attachment.id, proof.fingerprint(), now)?;
        dispatch()
    }
    /// Move every lease past its effective expiry to draining. Expiry never
    /// releases a resource or proves that old processes stopped.
    pub fn expire(&self, now: u64) -> Result<Vec<String>> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let ids: Vec<String> = state
            .leases
            .values()
            .filter(|l| {
                matches!(
                    l.status.as_str(),
                    "provisioning" | "ready" | "reserved" | "attached"
                ) && l.effective_expiry() <= now
            })
            .map(|l| l.id.clone())
            .collect();
        for id in &ids {
            let mut lease = state.leases.remove(id).ok_or("lease")?;
            if let Some(attachment) = lease.attachment.as_mut() {
                attachment.status = "draining".into();
            }
            self.transition(
                &mut state,
                &mut lease,
                "draining",
                "cancel_requested",
                json!({"expired":true}),
                now,
            )?;
            state.leases.insert(id.clone(), lease);
        }
        self.save(&state)?;
        Ok(ids)
    }

    /// Retain outputs outside the resource, then destroy a created resource or
    /// detach an adopted one by exact identity.
    pub fn cleanup(&self, lease_id: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut lease = state
            .leases
            .remove(lease_id)
            .ok_or_else(|| refuse("not_admitted", "unknown lease"))?;
        let outcome = self.cleanup_lease(&mut state, &mut lease, now);
        state.leases.insert(lease_id.into(), lease.clone());
        outcome?;
        self.save(&state)?;
        Ok(
            json!({"lease":lease.lease_ref,"state":lease.state_ref,"status":lease.status,
            "cleanup":lease.cleanup,"cleanup_status":lease.cleanup_status}),
        )
    }

    fn cleanup_lease(&self, state: &mut State, lease: &mut Lease, now: u64) -> Result<()> {
        if lease.status == "released" {
            return Err(refuse("conflict", "lease already released"));
        }
        let Some(realized) = lease.realized.clone() else {
            if lease.allocation == "failed" || lease.allocation == "refused" {
                return Err(refuse("conflict", "no resource exists to clean up"));
            }
            return Err(refuse(
                "unavailable",
                "allocation is unresolved; reconcile first",
            ));
        };
        let resource = lease.resource_ref.clone().ok_or("resource")?;
        let action = if lease.mode == "adopt" {
            "detach"
        } else {
            "destroy"
        };
        let mut fencing = Vec::new();
        let reason;
        let mut status = "pending";
        let mut retained = vec![lease.request_ref.clone(), lease.lease_ref.clone()];
        retained.extend(lease.materialization.clone());
        let mut provider_ref = Value::Null;
        if let Some(attachment) = &lease.attachment {
            fencing.extend(attachment.fencing.values().map(|(_, r)| r.clone()));
            reason = "participating dispatchers are not all fenced".into();
        } else {
            let reconciling = lease.cleanup_intent.is_some();
            if let Some(saved) = &lease.cleanup_intent {
                retained = saved.clone();
            }
            let collected = if reconciling {
                Ok(Vec::new())
            } else {
                self.provider.borrow_mut().collect(&realized)
            };
            match collected {
                Err(e) => {
                    status = "failed";
                    reason = format!("retention failed; nothing was deleted: {}", code(&e));
                }
                Ok(outputs) => {
                    let mut ok = true;
                    for (name, bytes) in outputs {
                        match self.store.put(&bytes, "application/octet-stream", None) {
                            Ok(reference) => {
                                retained.push(reference);
                                let _ = name;
                            }
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if !ok {
                        status = "failed";
                        reason = "retention failed; nothing was deleted".into();
                    } else {
                        let teardown = if reconciling {
                            self.provider
                                .borrow_mut()
                                .reconcile_teardown(&realized, action)
                        } else {
                            lease.cleanup_intent = Some(retained.clone());
                            lease.cleanup_status = Some("pending".into());
                            Self::charge(lease, "cleanup", Cost::Unknown, None);
                            self.transition(
                                state,
                                lease,
                                "draining",
                                "dispatched",
                                json!({"phase":"cleanup","action":action,"resource":resource}),
                                now,
                            )?;
                            state.leases.insert(lease.id.clone(), lease.clone());
                            self.save(state)?;
                            state.leases.remove(&lease.id);
                            if action == "destroy" {
                                self.provider.borrow_mut().destroy(&realized)
                            } else {
                                self.provider.borrow_mut().detach(&realized)
                            }
                        };
                        match teardown {
                            Teardown::Confirmed(receipt, cost) => {
                                let reference =
                                    self.store.put(&receipt, "application/json", None)?;
                                if !reconciling && !matches!(cost, Cost::Unknown) {
                                    lease.entries.retain(|e| {
                                        !(e["phase"] == "cleanup" && e["kind"] == "unknown")
                                    });
                                }
                                Self::charge(lease, "cleanup", cost, Some(reference.clone()));
                                provider_ref = reference;
                                status = "confirmed";
                                reason = format!("{action} confirmed");
                            }
                            Teardown::Failed(e) => {
                                status = "failed";
                                reason = format!("{action} failed: {}", code(&e));
                            }
                            Teardown::Unknown(e) => {
                                Self::charge(lease, "cleanup", Cost::Unknown, None);
                                status = "unknown";
                                reason = format!(
                                    "{action} outcome unknown; charges may continue: {}",
                                    e.chars().take(200).collect::<String>()
                                );
                            }
                        }
                    }
                }
            }
        }
        let accounting = self.accounting(state, lease, now)?;
        let body = json!({"v":"openagents.environment-cleanup.v1","requires":[],"lease":lease.lease_ref,
            "resource":resource,"attachment":lease.attachment.as_ref().map(|a| a.grant.clone()),
            "action":action,"status":status,"fencing":fencing,"retained":retained,"provider":provider_ref,
            "accounting":accounting,"reason":reason});
        lease.cleanup = Some(self.sign(state, &body, &lease.owner.clone(), now)?);
        lease.cleanup_status = Some(status.into());
        let next = match status {
            "confirmed" => {
                Self::release_hold(state, lease);
                "released"
            }
            "unknown" => "unknown",
            "failed" => "failed",
            _ => "draining",
        };
        let kind = if status == "confirmed" {
            "resolved"
        } else {
            "observed"
        };
        self.transition(state, lease, next, kind, json!({"cleanup":status}), now)?;
        Ok(())
    }

    /// Current lease view for the owner or an admitted reader.
    pub fn status(&self, lease_id: &str) -> Result<Value> {
        let state = self.state()?;
        let lease = state
            .leases
            .get(lease_id)
            .ok_or_else(|| refuse("not_admitted", "unknown lease"))?;
        Ok(
            json!({"lease":lease.lease_ref,"state":lease.state_ref,"status":lease.status,
            "revision":lease.revision,"allocation":lease.allocation,"expires_at":lease.effective_expiry(),
            "attachment":lease.attachment.as_ref().map(|a| json!({"id":a.id,"grant":a.grant,"status":a.status})),
            "cleanup":lease.cleanup,"cleanup_status":lease.cleanup_status,
            "held":state.holds.get(&lease.parent_reservation),"history":lease.history}),
        )
    }

    /// Lease IDs in admission order of their identifiers.
    pub fn leases(&self) -> Result<Vec<String>> {
        Ok(self.state()?.leases.keys().cloned().collect())
    }

    /// The retained signed declaration for an artifact, if any.
    pub fn declaration(&self, reference: &Value) -> Result<Event> {
        let state = self.state()?;
        let event_ref = state
            .declarations
            .get(digest_of(reference))
            .ok_or_else(|| refuse("content_unavailable", "no retained declaration"))?;
        serde_json::from_slice(&self.store.get(event_ref)?).map_err(|e| e.to_string())
    }
}

/// Build a signed private request envelope for an allocator.
pub fn envelope(secret: &SecretKey, allocator: &str, body: &Value, now: u64) -> Result<Event> {
    env::validate(body).map_err(contract)?;
    workspace_host::host::request(secret, allocator, body.clone(), now)
}

/// Exact JCS reference for a body.
pub fn reference(body: &Value) -> Result<Value> {
    let bytes = jcs(body).map_err(|e| e.to_string())?;
    Ok(workspace_host::artifact::reference(
        &bytes,
        "application/json",
        body["v"].as_str(),
    ))
}

impl Allocator {
    pub(crate) fn runtime_use(
        &self,
        request: &crate::runtime::RuntimeUse<'_>,
        now: u64,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let origin_now = now;
        let started = std::time::Instant::now();
        let _g = self.store.lock()?;
        let state = self.state()?;
        let lease = state
            .leases
            .values()
            .find(|l| {
                l.attachment
                    .as_ref()
                    .is_some_and(|a| a.id == request.attachment)
            })
            .ok_or("not_admitted: actual runtime attachment is not held")?;
        let attachment = lease.attachment.as_ref().ok_or("attachment")?;
        if attachment.grant != *request.grant
            || attachment.request["controller"] != request.controller
            || attachment.request["worker"] != request.executor
            || lease.lease_ref != *request.lease
            || lease.materialization.as_ref() != Some(request.materialization)
        {
            return Err(
                "identity_mismatch: runtime admission differs from the allocator's actual grant"
                    .into(),
            );
        }
        let materialization = self.body(
            request.materialization,
            "openagents.environment-materialization.v1",
        )?;
        if materialization["runtime"]["artifact"] != *request.engine {
            return Err(
                "identity_mismatch: admitted engine differs from the realized runtime".into(),
            );
        }
        let now = now.saturating_add(started.elapsed().as_secs());
        self.check_dispatch(request.attachment, request.fingerprint, now)?;
        let declaration = self.body(
            &attachment.request["execute"],
            "openagents.environment-execute.v1",
        )?;
        let plaintext = self.body(&declaration["body"], nostr::execution::SCHEMA)?;
        let reservation = self.body(
            &attachment.request["reservation"],
            "openagents.reservation.v1",
        )?;
        let now = origin_now.saturating_add(started.elapsed().as_secs());
        self.claims
            .as_ref()
            .ok_or("not_admitted: live runtime claim authority required")?
            .with_current(
                &attachment.request["claim"],
                request.controller,
                request.executor,
                &declaration,
                &plaintext,
                &reservation,
                &lease.request["frame"],
                now,
                &mut || {
                    let current = origin_now.saturating_add(started.elapsed().as_secs());
                    if lease.effective_expiry() <= current || attachment.expires_at <= current {
                        return Err("stale: runtime attachment expired before disclosure".into());
                    }
                    effect()
                },
            )
    }
}

impl Allocator {
    pub(crate) fn runtime_continuation(
        &self,
        continuation: &crate::runtime::RuntimeContinuation<'_>,
        now: u64,
        effect: &mut dyn FnMut(&crate::runtime::ContinuationFence<'_>, u64) -> Result<Value>,
    ) -> Result<Value> {
        let request = &continuation.runtime;
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let lease = state
            .leases
            .values()
            .find(|l| {
                l.attachment
                    .as_ref()
                    .is_some_and(|a| a.id == request.attachment)
            })
            .ok_or("not_admitted: no current runtime attachment")?;
        let attachment = lease.attachment.as_ref().ok_or("attachment")?;
        if lease.status != "attached"
            || attachment.status != "active"
            || attachment.grant != *request.grant
            || attachment.fingerprint != request.fingerprint
            || attachment.request["controller"] != request.controller
            || attachment.request["worker"] != request.executor
            || lease.lease_ref != *request.lease
            || lease.materialization.as_ref() != Some(request.materialization)
        {
            return Err("not_admitted: original active runtime attachment differs".into());
        }
        let materialization = self.body(
            request.materialization,
            "openagents.environment-materialization.v1",
        )?;
        let declaration = self.body(
            &attachment.request["execute"],
            "openagents.environment-execute.v1",
        )?;
        let plaintext = self.body(&declaration["body"], nostr::execution::SCHEMA)?;
        if materialization["runtime"]["artifact"] != *request.engine
            || declaration["event"]["id"] != continuation.execute_event
            || plaintext["target"] != *continuation.target
        {
            return Err("identity_mismatch: original runtime execution differs".into());
        }
        let deadline = lease
            .effective_expiry()
            .min(attachment.expires_at)
            .min(plaintext["deadline"].as_u64().ok_or("execute deadline")?);
        if now.saturating_add(started.elapsed().as_secs()) >= deadline {
            return Err("stale: runtime continuation expired".into());
        }
        self.provider.borrow_mut().check_current(
            lease
                .realized
                .as_ref()
                .ok_or("unavailable: original environment resource is not observed")?,
        )?;
        let reservation = self.body(
            &attachment.request["reservation"],
            "openagents.reservation.v1",
        )?;
        self.claims
            .as_ref()
            .ok_or("not_admitted: live runtime claim authority required")?
            .with_continuation(
                &attachment.request["claim"],
                request.controller,
                request.executor,
                &declaration,
                &plaintext,
                &reservation,
                &lease.request["frame"],
                now.saturating_add(started.elapsed().as_secs()),
                &mut |current| {
                    let current = current.max(now.saturating_add(started.elapsed().as_secs()));
                    if current >= deadline {
                        return Err("stale: runtime continuation expired".into());
                    }
                    let fence = crate::runtime::ContinuationFence {
                        runtime: crate::runtime::RuntimeFence {
                            request: *request,
                            deadline,
                            admitted_at: current,
                            started: std::time::Instant::now(),
                        },
                    };
                    effect(&fence, current)
                },
            )
    }

    pub(crate) fn runtime_execution(
        &self,
        request: &crate::runtime::RuntimeDispatch<'_>,
        now: u64,
        effect: &mut dyn FnMut(
            &crate::runtime::RuntimeFence<'_>,
            &coordination_host::worker::Intake,
            u64,
        ) -> Result<Value>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let lease = state
            .leases
            .values()
            .find(|l| {
                l.attachment
                    .as_ref()
                    .is_some_and(|a| a.id == request.attachment)
            })
            .ok_or("not_admitted: actual runtime attachment is not held")?;
        let attachment = lease.attachment.as_ref().ok_or("attachment")?;
        if attachment.grant != *request.grant
            || attachment.request["controller"] != request.controller
            || attachment.request["worker"] != request.executor
        {
            return Err("identity_mismatch: runtime dispatch differs from the saved grant".into());
        }
        let materialization_ref = lease.materialization.as_ref().ok_or("materialization")?;
        let materialization = self.body(
            materialization_ref,
            "openagents.environment-materialization.v1",
        )?;
        if materialization["runtime"]["artifact"] != *request.engine {
            return Err(
                "identity_mismatch: realized runtime differs from the admitted engine".into(),
            );
        }
        self.check_dispatch(
            request.attachment,
            request.fingerprint,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let declaration = self.body(
            &attachment.request["execute"],
            "openagents.environment-execute.v1",
        )?;
        let plaintext = self.body(&declaration["body"], nostr::execution::SCHEMA)?;
        if plaintext["target"] != *request.target {
            return Err(
                "identity_mismatch: runtime target differs from the attached execution".into(),
            );
        }
        let reservation = self.body(
            &attachment.request["reservation"],
            "openagents.reservation.v1",
        )?;
        let deadline = lease
            .effective_expiry()
            .min(attachment.expires_at)
            .min(plaintext["deadline"].as_u64().ok_or("execute deadline")?);
        let current = now.saturating_add(started.elapsed().as_secs());
        self.claims
            .as_ref()
            .ok_or("not_admitted: live runtime claim authority required")?
            .with_intake(
                &attachment.request["claim"],
                request.controller,
                request.executor,
                &declaration,
                &plaintext,
                &reservation,
                &lease.request["frame"],
                request.intake,
                current,
                &mut |admitted, current| {
                    let current = current.max(now.saturating_add(started.elapsed().as_secs()));
                    if current >= deadline {
                        return Err(
                            "stale: runtime attachment expired before input disclosure".into()
                        );
                    }
                    let fence = crate::runtime::RuntimeFence {
                        request: crate::runtime::RuntimeUse {
                            attachment: request.attachment,
                            fingerprint: request.fingerprint,
                            grant: request.grant,
                            controller: request.controller,
                            executor: request.executor,
                            lease: &lease.lease_ref,
                            materialization: materialization_ref,
                            engine: request.engine,
                        },
                        deadline,
                        admitted_at: current,
                        started: std::time::Instant::now(),
                    };
                    effect(&fence, admitted, current)
                },
            )
    }
}
