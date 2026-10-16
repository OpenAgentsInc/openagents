//! Remote custody and guardian quorum admission for sovereign agents.
//!
//! The custodian holds an authority-signed custody policy and an optional
//! guardian policy per agent. Custody operations bind the agent, exact
//! profile and policy revisions, requester, controller generation, purpose,
//! complete input, nonce, and deadline. Listed operations additionally need
//! an action-level guardian quorum that is consumed atomically before the
//! signer is called. Guardians are approvers, never custodians.
use crate::{
    Result,
    signer::{Call, Nip46},
};
use nostr::{
    contracts::{self, digest_bytes},
    domain::{Event, RelaySigner},
    sovereign,
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

/// Independently configured host facts a request cannot nominate.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Agent pubkey to its admitted policy authority.
    pub authorities: BTreeMap<String, String>,
    /// Agent pubkey to its admitted SOV profile ArtifactRef.
    pub profiles: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    agent: String,
    action: Value,
    policy: Value,
    generation: u64,
    operation: Value,
    input: Value,
    requests: BTreeMap<String, Value>,
    decisions: BTreeMap<String, (String, Value)>,
    denied: bool,
    consumed: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    fingerprint: String,
    requester: String,
    purpose: String,
    status: String,
    receipt: Value,
    reason: Option<String>,
    plaintext: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    config: Config,
    generations: BTreeMap<String, u64>,
    guardian: BTreeMap<String, Value>,
    custody: BTreeMap<String, Value>,
    revoked: BTreeSet<String>,
    pending: BTreeMap<String, Pending>,
    operations: BTreeMap<String, Operation>,
    signers: BTreeMap<String, BTreeSet<String>>,
}

/// A signed private request, its exact artifacts, and further declarations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub declarations: Vec<Event>,
}

pub struct Custodian {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
    nip46: RefCell<BTreeMap<String, Nip46>>,
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

impl Custodian {
    pub fn initialize(root: PathBuf, secret: SecretKey, config: Config) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: custodian state already exists".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let generations = config.authorities.keys().map(|a| (a.clone(), 1)).collect();
        store.save(
            &State {
                config,
                generations,
                ..State::default()
            },
            &signer,
        )
    }
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let custodian = Self {
            store,
            signer,
            secret,
            nip46: RefCell::new(BTreeMap::new()),
        };
        custodian.state()?;
        Ok(custodian)
    }
    /// Mount the NIP-46 connection for an agent's remote signer.
    pub fn mount(&self, agent: &str, connection: Nip46) -> Result<()> {
        let _lock = self.store.lock()?;
        let state = self.state()?;
        if connection.expected_agent() != agent || !state.config.authorities.contains_key(agent) {
            return Err(refuse(
                "identity_mismatch",
                "connection names another agent",
            ));
        }
        if let Some(reference) = state.custody.get(agent) {
            let policy = self.json(reference)?;
            if policy["signer"] != connection.remote()
                || policy["transport"] != connection.transport_pubkey()
            {
                return Err(refuse(
                    "identity_mismatch",
                    "connection differs from the admitted custody policy",
                ));
            }
        }
        self.nip46.borrow_mut().insert(agent.into(), connection);
        Ok(())
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn store(&self) -> &Artifacts {
        &self.store
    }
    fn state(&self) -> Result<State> {
        self.store.load(&self.signer)
    }
    fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }

    fn sign(&self, state: &mut State, body: &Value, recipient: &str, now: u64) -> Result<Value> {
        sovereign::validate(body).map_err(contract)?;
        let reference = self.store.put_json(body)?;
        let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
        self.store.put(
            &serde_json::to_vec(&event).map_err(|e| e.to_string())?,
            "application/json",
            None,
        )?;
        state
            .signers
            .entry(digest_of(&reference).into())
            .or_default()
            .insert(self.pubkey().into());
        Ok(reference)
    }

    fn authenticate(&self, state: &mut State, event: &Event) -> Result<(String, Value, Value)> {
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let bytes = opened
            .inline_bytes()
            .ok_or("malformed: requests carry their artifact inline")?
            .to_vec();
        let body = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
        let schema = body["v"].as_str().unwrap_or_default().to_string();
        let reference = self.store.put(
            &bytes,
            "application/json",
            Some(schema.as_str()).filter(|s| !s.is_empty()),
        )?;
        state
            .signers
            .entry(digest_of(&reference).into())
            .or_default()
            .insert(opened.signer().into());
        Ok((opened.signer().into(), body, reference))
    }

    fn bytes(&self, reference: &Value) -> Result<Vec<u8>> {
        self.store
            .get(reference)
            .map_err(|_| refuse("content_unavailable", "referenced bytes are not retained"))
    }
    fn json(&self, reference: &Value) -> Result<Value> {
        contracts::parse_strict_bounded(&self.bytes(reference)?, 1_048_576).map_err(contract)
    }

    /// Admit one signed operation. Decrypted plaintext is returned only in
    /// this local reply to its admitted requester; it is never an artifact.
    pub fn submit(&self, bundle: &Bundle, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        for declaration in &bundle.declarations {
            self.authenticate(&mut state, declaration)?;
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event)?;
        let outcome = if body["v"] == "openagents.action.v1" {
            self.request_approvals(&mut state, &signer, &body, &input, now)
        } else {
            match sovereign::validate(&body) {
                Err(e) => Err(format!("{}: {}", e.code.as_str(), e)),
                Ok(()) => match body["v"].as_str() {
                    Some(
                        "openagents.sovereign-guardian-policy.v1"
                        | "openagents.sovereign-custody-policy.v1",
                    ) => self.admit_policy(&mut state, &signer, &body, &input),
                    Some("openagents.approval-decision.v1") => {
                        self.decide(&mut state, &signer, &body, &input, now)
                    }
                    Some("openagents.sovereign-custody-request.v1") => {
                        self.custody(&mut state, &signer, &body, &input, now)
                    }
                    _ => Err(refuse("unsupported_feature", "operation")),
                },
            }
        };
        let value = match outcome {
            Ok(value) => value,
            Err(error) => {
                let mut fresh = self.state()?;
                fresh.signers.extend(state.signers);
                state = fresh;
                json!({"status":if matches!(code(&error),"conflict"|"idempotency_conflict"){"conflict"}else{"refused"},"reason":code(&error)})
            }
        };
        self.save(&state)?;
        Ok(value)
    }

    fn admit_policy(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        let authority = state
            .config
            .authorities
            .get(&agent)
            .ok_or_else(|| refuse("not_admitted", "unknown agent"))?;
        if body["authority"] != authority.as_str() || signer != authority {
            return Err(refuse(
                "not_admitted",
                "the policy is not signed by the agent's authority",
            ));
        }
        let custody = body["v"] == "openagents.sovereign-custody-policy.v1";
        if custody && body["mechanism"] != "nip46" {
            return Err(refuse(
                "cannot_enforce",
                "only NIP-46 remote custody is supported here",
            ));
        }
        let lineage = if custody {
            &state.custody
        } else {
            &state.guardian
        };
        match (lineage.get(&agent), body["previous"].is_null()) {
            (None, true) => {}
            (Some(current), false) if same(current, &body["previous"]) => {
                let current = self.json(current)?;
                if current["revision"].as_u64().map(|r| r + 1) != body["revision"].as_u64() {
                    return Err(refuse("conflict", "policy revision must advance by one"));
                }
            }
            _ => return Err(refuse("conflict", "policy predecessor")),
        }
        if custody {
            if let Some(connection) = self.nip46.borrow().get(&agent)
                && (connection.remote() != body["signer"].as_str().unwrap_or_default()
                    || connection.transport_pubkey()
                        != body["transport"].as_str().unwrap_or_default())
            {
                return Err(refuse(
                    "identity_mismatch",
                    "policy names another signer or transport",
                ));
            }
            state.custody.insert(agent.clone(), input.clone());
        } else {
            state.guardian.insert(agent.clone(), input.clone());
            // A policy change invalidates every pending quorum for the agent.
            state
                .pending
                .retain(|_, p| p.agent != agent || p.consumed.is_some());
        }
        Ok(json!({"status":"accepted","value":input}))
    }

    /// Issue one approval request per guardian for an exact action.
    fn request_approvals(
        &self,
        state: &mut State,
        signer: &str,
        action: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        for field in [
            "task_frame",
            "snapshot",
            "operation",
            "binding",
            "input",
            "context",
            "effects",
            "bounds",
            "recipient",
            "preconditions",
            "nonce",
        ] {
            if action.get(field).is_none() {
                return Err(refuse("malformed", "POL action field"));
            }
        }
        let preconditions = self.json(&action["preconditions"])?;
        sovereign::validate(&preconditions).map_err(contract)?;
        if preconditions["v"] != "openagents.sovereign-action-preconditions.v1" {
            return Err(refuse("unsupported_version", "action preconditions"));
        }
        let policy = self.json(&preconditions["guardian_policy"])?;
        let agent = policy["agent"].as_str().unwrap_or_default().to_string();
        let current = state
            .guardian
            .get(&agent)
            .ok_or_else(|| refuse("not_admitted", "no guardian policy"))?;
        if !same(current, &preconditions["guardian_policy"]) {
            return Err(refuse(
                "stale",
                "the action binds a superseded guardian policy",
            ));
        }
        if !state
            .config
            .profiles
            .get(&agent)
            .is_some_and(|p| same(p, &preconditions["profile"]))
        {
            return Err(refuse(
                "identity_mismatch",
                "the action binds another profile",
            ));
        }
        let generation = state.generations.get(&agent).copied().unwrap_or(1);
        if preconditions["generation"] != generation {
            return Err(refuse("stale", "controller generation"));
        }
        if !same(&preconditions["input"], &action["input"]) {
            return Err(refuse(
                "identity_mismatch",
                "preconditions bind another input",
            ));
        }
        if !policy["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|o| *o == action["operation"])
        {
            return Err(refuse(
                "not_admitted",
                "the operation is not gated by this policy",
            ));
        }
        let custody = state
            .custody
            .get(&agent)
            .map(|c| self.json(c))
            .transpose()?;
        if !custody.is_some_and(|c| {
            c["callers"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| c == signer)
        }) {
            return Err(refuse(
                "not_admitted",
                "requester is not an admitted caller",
            ));
        }
        if now >= policy["expires_at"].as_u64().unwrap_or(0)
            || now < policy["starts_at"].as_u64().unwrap_or(u64::MAX)
        {
            return Err(refuse("stale", "guardian policy is not current"));
        }
        let key = digest_of(input).to_string();
        if let Some(pending) = state.pending.get(&key) {
            return Ok(json!({"status":"duplicate","requests":pending.requests}));
        }
        let mut requests = BTreeMap::new();
        for member in policy["members"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let request = json!({"v":"openagents.approval-request.v1","requires":[],"request":token(),"action":input,
                "requester":signer,"approver":member,"expires_at":policy["expires_at"].as_u64().unwrap_or(now).min(now + 3600),
                "reviews":[]});
            requests.insert(member.to_string(), self.sign(state, &request, member, now)?);
        }
        state.pending.insert(
            key,
            Pending {
                agent,
                action: input.clone(),
                policy: preconditions["guardian_policy"].clone(),
                generation,
                operation: action["operation"].clone(),
                input: action["input"].clone(),
                requests: requests.clone(),
                decisions: BTreeMap::new(),
                denied: false,
                consumed: None,
            },
        );
        Ok(json!({"status":"accepted","requests":requests}))
    }

    fn decide(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let pending = state
            .pending
            .get_mut(digest_of(&body["action"]))
            .ok_or_else(|| refuse("not_admitted", "no pending approval for that action"))?;
        if body["approver"] != signer {
            return Err(refuse(
                "identity_mismatch",
                "decisions are signed by their approver",
            ));
        }
        let request = pending
            .requests
            .get(signer)
            .ok_or_else(|| refuse("not_admitted", "not a guardian for this action"))?;
        if !same(request, &body["request"]) {
            // A decision for another action's request cannot count here.
            return Err(refuse(
                "identity_mismatch",
                "decision names another request",
            ));
        }
        let request_body = self.json(request)?;
        let expires = body["expires_at"].as_u64().unwrap_or(0);
        if expires > request_body["expires_at"].as_u64().unwrap_or(0) || expires <= now {
            return Err(refuse("stale", "decision expiry"));
        }
        if pending.consumed.is_some() {
            return Err(refuse("conflict", "the quorum was already consumed"));
        }
        let decision = body["decision"].as_str().unwrap_or_default().to_string();
        match pending.decisions.get(signer) {
            Some((prior, reference)) if same(reference, input) => {
                return Ok(json!({"status":"duplicate","decision":prior}));
            }
            Some((prior, _)) if *prior != decision => {
                // A conflicting decision by one guardian blocks admission.
                pending.denied = true;
            }
            _ => {}
        }
        if decision == "deny" {
            pending.denied = true;
        }
        pending
            .decisions
            .insert(signer.into(), (decision, input.clone()));
        Ok(json!({"status":"accepted","denied":pending.denied}))
    }

    /// Count distinct, unexpired member approvals; any denial blocks.
    fn quorum(
        &self,
        state: &mut State,
        action: &Value,
        agent: &str,
        operation: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let current = state
            .guardian
            .get(agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "no guardian policy"))?;
        let policy = self.json(&current)?;
        let generation = state.generations.get(agent).copied().unwrap_or(1);
        let pending = state
            .pending
            .get(digest_of(action))
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "no approvals for that action"))?;
        if let Some(admission) = &pending.consumed {
            return Err(refuse(
                "conflict",
                &format!("quorum already consumed by {}", digest_of(admission)),
            ));
        }
        if !same(&pending.policy, &current) || pending.generation != generation {
            return Err(refuse(
                "stale",
                "policy or controller changed; the quorum is void",
            ));
        }
        if pending.operation != *operation || !same(&pending.input, input) {
            return Err(refuse(
                "identity_mismatch",
                "the request is not the approved action",
            ));
        }
        if pending.denied {
            return Err(refuse("not_admitted", "a guardian denied the action"));
        }
        let members: BTreeSet<&str> = policy["members"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut approvals = Vec::new();
        for (approver, (decision, reference)) in &pending.decisions {
            let body = self.json(reference)?;
            if decision == "approve"
                && members.contains(approver.as_str())
                && body["expires_at"].as_u64().is_some_and(|e| e > now)
            {
                approvals.push(reference.clone());
            }
        }
        if (approvals.len() as u64) < policy["threshold"].as_u64().unwrap_or(u64::MAX) {
            return Err(refuse("not_admitted", "the guardian threshold is not met"));
        }
        let admission = json!({"v":"openagents.sovereign-quorum-admission.v1","requires":[],"action":action,
            "policy":current,"approvals":approvals,"generation":generation,"consumed_at":now});
        let reference = self.sign(state, &admission, agent, now)?;
        state
            .pending
            .get_mut(digest_of(action))
            .ok_or("pending")?
            .consumed = Some(reference.clone());
        Ok(reference)
    }

    fn custody(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        let key = format!(
            "{agent}:{signer}:{}",
            body["nonce"].as_str().unwrap_or_default()
        );
        if let Some(op) = state.operations.get(&key).cloned() {
            if op.fingerprint != digest_of(input) {
                return Err(refuse(
                    "idempotency_conflict",
                    "the nonce was used for another request",
                ));
            }
            // Exact retry: the retained outcome, never another signer call.
            let current_policy = state
                .custody
                .get(&agent)
                .filter(|p| same(p, &body["policy"]))
                .map(|p| self.json(p))
                .transpose()?;
            let same_guardians = match state.guardian.get(&agent) {
                Some(policy) => same(policy, &body["guardian"]),
                None => body["guardian"].is_null(),
            };
            let disclose = op.purpose == "nip44_decrypt"
                && !state.revoked.contains(signer)
                && state
                    .config
                    .profiles
                    .get(&agent)
                    .is_some_and(|p| same(p, &body["profile"]))
                && body["generation"] == state.generations.get(&agent).copied().unwrap_or(1)
                && body["deadline"]
                    .as_u64()
                    .is_some_and(|deadline| now < deadline)
                && same_guardians
                && current_policy.as_ref().is_some_and(|policy| {
                    policy["starts_at"]
                        .as_u64()
                        .is_some_and(|start| start <= now)
                        && policy["expires_at"].as_u64().is_some_and(|end| now < end)
                        && policy["callers"]
                            .as_array()
                            .is_some_and(|callers| callers.iter().any(|v| v == signer))
                        && policy["purposes"]
                            .as_array()
                            .is_some_and(|purposes| purposes.iter().any(|v| v == "nip44_decrypt"))
                });
            return Ok(
                json!({"status":op.status,"reason":op.reason,"receipt":op.receipt,
                "plaintext":if disclose { op.plaintext.clone() } else { None }}),
            );
        }
        if body["requester"] != signer {
            return Err(refuse("identity_mismatch", "requester is the signer"));
        }
        let policy_ref = state
            .custody
            .get(&agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "no custody policy"))?;
        if !same(&policy_ref, &body["policy"]) {
            return Err(refuse("stale", "custody policy revision"));
        }
        let policy = self.json(&policy_ref)?;
        if state.revoked.contains(signer)
            || !policy["callers"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| c == signer)
        {
            return Err(refuse(
                "not_admitted",
                "requester is not an admitted caller",
            ));
        }
        if !state
            .config
            .profiles
            .get(&agent)
            .is_some_and(|p| same(p, &body["profile"]))
        {
            return Err(refuse("identity_mismatch", "profile"));
        }
        if body["generation"] != state.generations.get(&agent).copied().unwrap_or(1) {
            return Err(refuse("stale", "controller generation"));
        }
        let purpose = body["purpose"].as_str().unwrap_or_default().to_string();
        if !policy["purposes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| *p == purpose)
        {
            return Err(refuse("not_admitted", "purpose is not admitted"));
        }
        if body["deadline"].as_u64().unwrap_or(0) <= now
            || now < policy["starts_at"].as_u64().unwrap_or(u64::MAX)
            || now >= policy["expires_at"].as_u64().unwrap_or(0)
        {
            return Err(refuse("stale", "deadline or policy expiry"));
        }
        let operand = self.json(&body["input"])?;
        if !self.nip46.borrow().contains_key(&agent) {
            return Err(refuse("unavailable", "no signer is mounted for this agent"));
        }
        // Guardian gate for listed operations: consume the quorum first.
        let gated = state
            .guardian
            .get(&agent)
            .map(|g| self.json(g))
            .transpose()?
            .is_some_and(|g| {
                g["operations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|o| *o == body["operation"])
            });
        let quorum = if gated {
            if body["guardian"].is_null()
                || !state
                    .guardian
                    .get(&agent)
                    .is_some_and(|g| same(g, &body["guardian"]))
            {
                return Err(refuse("stale", "guardian policy revision"));
            }
            let action = body["action"].clone();
            if action.is_null() {
                return Err(refuse(
                    "not_admitted",
                    "this operation needs a guardian-approved action",
                ));
            }
            Some(self.quorum(
                state,
                &action,
                &agent,
                &body["operation"],
                &body["input"],
                now,
            )?)
        } else {
            None
        };
        let (method, params) = match purpose.as_str() {
            "sign" => {
                if operand["pubkey"] != agent.as_str() {
                    return Err(refuse(
                        "identity_mismatch",
                        "the unsigned event names another key",
                    ));
                }
                (
                    "sign_event",
                    vec![serde_json::to_string(&operand).map_err(|e| e.to_string())?],
                )
            }
            "nip44_encrypt" | "nip44_decrypt" => {
                if purpose == "nip44_decrypt" && operand["recipient"] != signer {
                    return Err(refuse(
                        "not_admitted",
                        "plaintext goes only to its admitted recipient",
                    ));
                }
                let text = operand["text"]
                    .as_str()
                    .ok_or("malformed: cipher text")?
                    .to_string();
                let peer = operand["peer"]
                    .as_str()
                    .ok_or("malformed: peer")?
                    .to_string();
                (purpose.as_str(), vec![peer, text])
            }
            _ => return Err(refuse("unsupported_feature", "purpose")),
        };
        // Persist dispatch intent (and quorum consumption) before the call.
        state.operations.insert(
            key.clone(),
            Operation {
                fingerprint: digest_of(input).into(),
                requester: signer.into(),
                purpose: purpose.clone(),
                status: "unknown".into(),
                receipt: Value::Null,
                reason: None,
                plaintext: None,
            },
        );
        let mut snapshot = self.state()?;
        snapshot
            .operations
            .insert(key.clone(), state.operations[&key].clone());
        snapshot.pending = state.pending.clone();
        snapshot.signers.extend(state.signers.clone());
        self.save(&snapshot)?;
        let call = {
            let mut connections = self.nip46.borrow_mut();
            let connection = connections
                .get_mut(&agent)
                .ok_or_else(|| refuse("unavailable", "no signer mounted"))?;
            if method == "sign_event" {
                connection.sign_event(&params[0], now)
            } else {
                connection.nip44(method, &params[0], &params[1], now)
            }
        };
        let (status, result, reason, plaintext) = match call {
            Call::Result(value) => match purpose.as_str() {
                "sign" => {
                    let event: Event =
                        serde_json::from_str(&value).map_err(|_| "malformed: signed event")?;
                    let matches = event.validate_crypto().is_ok()
                        && event.pubkey == agent
                        && event.kind as u64 == operand["kind"].as_u64().unwrap_or(u64::MAX)
                        && event.created_at == operand["created_at"].as_u64().unwrap_or(0)
                        && event.content == operand["content"].as_str().unwrap_or_default()
                        && serde_json::to_value(&event.tags).ok() == Some(operand["tags"].clone());
                    if matches {
                        (
                            "completed",
                            Some(self.store.put(value.as_bytes(), "application/json", None)?),
                            None,
                            None,
                        )
                    } else {
                        ("refused", None, Some("identity_mismatch"), None)
                    }
                }
                "nip44_encrypt" => (
                    "completed",
                    Some(self.store.put(value.as_bytes(), "text/plain", None)?),
                    None,
                    None,
                ),
                _ => ("completed", None, None, Some(value)),
            },
            Call::Refused(reason) if reason == "unavailable" => {
                ("refused", None, Some("unavailable"), None)
            }
            Call::Refused(_) => ("refused", None, Some("not_admitted"), None),
            Call::Unknown => ("unknown", None, None, None),
        };
        let receipt = json!({"v":"openagents.sovereign-custody-receipt.v1","requires":[],"request":input,
            "status":status,"result":result,"quorum":quorum,"evidence":[],"reason":reason});
        let receipt = self.sign(state, &receipt, signer, now)?;
        state.operations.insert(
            key,
            Operation {
                fingerprint: digest_of(input).into(),
                requester: signer.into(),
                purpose,
                status: status.into(),
                receipt: receipt.clone(),
                reason: reason.map(str::to_string),
                plaintext: plaintext.clone(),
            },
        );
        Ok(json!({"status":status,"reason":reason,"receipt":receipt,"plaintext":plaintext}))
    }

    /// Revoke a caller. Retained results may still be named, but decrypted
    /// plaintext is not disclosed again.
    pub fn revoke(&self, caller: &str) -> Result<()> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        state.revoked.insert(caller.into());
        self.save(&state)
    }

    /// A controller change: pending quorums for the agent become void.
    pub fn advance_generation(&self, agent: &str) -> Result<u64> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let next = state.generations.get(agent).copied().unwrap_or(1) + 1;
        state.generations.insert(agent.into(), next);
        self.save(&state)?;
        Ok(next)
    }
}

/// Build a signed private request envelope for a custodian.
pub fn envelope(secret: &SecretKey, custodian: &str, body: &Value, now: u64) -> Result<Event> {
    if body["v"] != "openagents.action.v1" {
        sovereign::validate(body).map_err(contract)?;
    }
    workspace_host::host::request(secret, custodian, body.clone(), now)
}
