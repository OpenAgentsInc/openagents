//! The lifecycle coordinator and shared fencing authority for sovereign
//! agents. It binds `(agent, activation ID)` to exact bytes forever, holds one
//! exclusive lifecycle claim per agent, accepts checkpoints only from the
//! current controller generation, and completes a transfer only after the
//! authority signed it, the target accepted it, and every old dispatcher
//! acknowledged fencing.
use crate::Result;
use nostr::{
    contracts::{self, digest_bytes},
    domain::{Event, RelaySigner},
    sovereign_lifecycle as life,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::Artifacts;

/// Independently configured facts a request cannot nominate.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Agent pubkey to its governing authority.
    pub authorities: BTreeMap<String, String>,
    /// Agent pubkey to its admitted profile ArtifactRef.
    pub profiles: BTreeMap<String, Value>,
    /// Runtime hosts admitted as controllers.
    pub hosts: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transfer {
    reference: Value,
    body: Value,
    acceptance: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activation {
    id: String,
    agent: String,
    reference: Value,
    controller: String,
    generation: u64,
    dispatchers: Vec<String>,
    status: String,
    reservation: Value,
    expires_at: u64,
    checkpoints: Vec<Value>,
    fencing: BTreeMap<String, (String, Value)>,
    unknown: Vec<Value>,
    late: Vec<Value>,
    transfer: Option<Transfer>,
    successor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    fingerprint: String,
    result: Value,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    config: Config,
    activations: BTreeMap<String, Activation>,
    bindings: BTreeMap<String, Binding>,
    claims: BTreeMap<String, String>,
    signers: BTreeMap<String, BTreeSet<String>>,
}

/// A signed private request, its exact artifacts, and further declarations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

pub struct Coordinator {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
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

impl Coordinator {
    pub fn initialize(root: PathBuf, secret: SecretKey, config: Config) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: coordinator state already exists".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        store.save(
            &State {
                config,
                ..State::default()
            },
            &signer,
        )
    }
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let coordinator = Self {
            store,
            signer,
            secret,
        };
        coordinator.state()?;
        Ok(coordinator)
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
        life::validate(body).map_err(contract)?;
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

    #[allow(clippy::too_many_arguments)]
    fn result(
        &self,
        state: &mut State,
        recipient: &str,
        input: &Value,
        status: &str,
        activation: Option<&Activation>,
        value: Option<Value>,
        reason: Option<&str>,
        now: u64,
    ) -> Result<Value> {
        let body = json!({"v":"openagents.sovereign-lifecycle-result.v1","requires":[],"request":input,
            "status":status,"activation":activation.map(|a| a.reference.clone()),
            "state":activation.map_or("none", |a| a.status.as_str()),"value":value,"reason":reason});
        self.sign(state, &body, recipient, now)?;
        Ok(body)
    }

    /// Admit one signed lifecycle operation.
    pub fn submit(&self, bundle: &Bundle, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event)?;
        let outcome = match life::validate(&body) {
            Err(e) => Err(format!("{}: {}", e.code.as_str(), e)),
            Ok(()) => match body["v"].as_str() {
                Some("openagents.sovereign-activation-request.v1") => {
                    self.activate(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-checkpoint.v1") => {
                    self.checkpoint(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-transfer.v1") => {
                    self.transfer(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-transfer-acceptance.v1") => {
                    self.accept(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-fencing.v1") => {
                    self.fence(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-lifecycle-control.v1") => {
                    self.control(&mut state, &signer, &body, &input, now)
                }
                _ => Err(refuse("unsupported_feature", "operation")),
            },
        };
        let result = match outcome {
            Ok(result) => result,
            Err(error) => {
                let mut fresh = self.state()?;
                fresh.signers.extend(state.signers);
                state = fresh;
                let status = if matches!(code(&error), "conflict" | "idempotency_conflict") {
                    "conflict"
                } else {
                    "refused"
                };
                self.result(
                    &mut state,
                    &signer,
                    &input,
                    status,
                    None,
                    None,
                    Some(code(&error)),
                    now,
                )?
            }
        };
        self.save(&state)?;
        Ok(result)
    }

    fn authority(state: &State, agent: &str, signer: &str) -> Result<()> {
        if state.config.authorities.get(agent).map(String::as_str) != Some(signer) {
            return Err(refuse(
                "not_admitted",
                "only the agent's governing authority may do this",
            ));
        }
        Ok(())
    }

    /// Validate a target placement: exact ENV records whose executor is the
    /// named controller, and a profile disclosure that admits that host.
    fn placement(
        &self,
        state: &State,
        agent: &str,
        controller: &str,
        environment: &Value,
    ) -> Result<()> {
        if !state.config.hosts.contains(controller) {
            return Err(refuse(
                "not_admitted",
                "the controller is not an admitted runtime host",
            ));
        }
        nostr::environment::verify(
            &environment["lease"],
            &self.bytes(&environment["lease"])?,
            "openagents.environment-lease.v1",
        )
        .map_err(|e| refuse("incompatible", &format!("environment lease: {e}")))?;
        let materialization = nostr::environment::verify(
            &environment["materialization"],
            &self.bytes(&environment["materialization"])?,
            "openagents.environment-materialization.v1",
        )
        .map_err(|e| refuse("incompatible", &format!("materialization: {e}")))?;
        if !same(&materialization["lease"], &environment["lease"]) {
            return Err(refuse(
                "identity_mismatch",
                "materialization names another lease",
            ));
        }
        if !materialization["participants"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["role"] == "executor" && p["principal"] == controller)
        {
            return Err(refuse(
                "incompatible",
                "the materialized runtime is not the controller's",
            ));
        }
        let profile = self.json(
            state
                .config
                .profiles
                .get(agent)
                .ok_or_else(|| refuse("not_admitted", "unknown agent"))?,
        )?;
        let disclosure = self.json(&profile["disclosure"])?;
        if disclosure["v"] != "openagents.sovereign-state-disclosure.v1"
            || !disclosure["recipients"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|r| r == controller)
        {
            return Err(refuse(
                "not_admitted",
                "private state may not be disclosed to that host",
            ));
        }
        Ok(())
    }

    /// A complete, retained checkpoint chain ending at `latest`.
    fn chain(&self, activation: &Activation, latest: &Value) -> Result<()> {
        let last = activation
            .checkpoints
            .last()
            .ok_or_else(|| refuse("unavailable", "no checkpoint"))?;
        if !same(last, latest) {
            return Err(refuse("stale", "not the latest checkpoint"));
        }
        let mut previous: Option<Value> = None;
        for (revision, reference) in activation.checkpoints.iter().enumerate() {
            let body = self.json(reference)?;
            if body["revision"] != revision as u64
                || body["previous"] != previous.clone().unwrap_or(Value::Null)
            {
                return Err(refuse("conflict", "the checkpoint chain is incomplete"));
            }
            for field in ["record", "state"] {
                self.bytes(&body[field])?;
            }
            previous = Some(reference.clone());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn admit(
        &self,
        state: &mut State,
        request: &Value,
        request_ref: &Value,
        generation: u64,
        checkpoint: Value,
        unknown: Vec<Value>,
        now: u64,
    ) -> Result<String> {
        self.check_request(state, request, now)?;
        let id = request["activation"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let agent = request["agent"].as_str().unwrap_or_default().to_string();
        let body = json!({"v":"openagents.sovereign-activation.v1","requires":[],"activation":id,
            "profile":request["profile"],"plan":request["plan"],"controller":request["controller"],
            "generation":generation,"claim":request["claim"],"reservation":request["reservation"],
            "environment":request["environment"],"checkpoint":checkpoint,"admitted_at":now});
        let controller = request["controller"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let reference = self.sign(state, &body, &controller, now)?;
        let _ = request_ref;
        state.activations.insert(
            id.clone(),
            Activation {
                id: id.clone(),
                agent: agent.clone(),
                reference,
                controller,
                generation,
                dispatchers: request["dispatchers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                status: "active".into(),
                reservation: request["reservation"].clone(),
                expires_at: request["expires_at"].as_u64().unwrap_or(0),
                checkpoints: Vec::new(),
                fencing: BTreeMap::new(),
                unknown,
                late: Vec::new(),
                transfer: None,
                successor: None,
            },
        );
        state.claims.insert(agent, id.clone());
        Ok(id)
    }

    fn check_request(&self, state: &State, request: &Value, now: u64) -> Result<()> {
        let agent = request["agent"].as_str().unwrap_or_default();
        if state
            .activations
            .contains_key(request["activation"].as_str().unwrap_or_default())
        {
            return Err(refuse(
                "conflict",
                "activation ID already belongs to an admitted lifecycle",
            ));
        }
        if !state
            .config
            .profiles
            .get(agent)
            .is_some_and(|p| same(p, &request["profile"]))
        {
            return Err(refuse(
                "identity_mismatch",
                "profile is not the admitted one",
            ));
        }
        if request["expires_at"].as_u64().unwrap_or(0) <= now {
            return Err(refuse("stale", "activation request expired"));
        }
        let controller = request["controller"].as_str().unwrap_or_default();
        self.placement(state, agent, controller, &request["environment"])?;
        let lease = self.json(&request["environment"]["lease"])?;
        let materialization = self.json(&request["environment"]["materialization"])?;
        let lease_end = lease["not_after"].as_u64().unwrap_or(0);
        if lease["created_at"].as_u64().unwrap_or(u64::MAX) > now
            || lease_end <= now
            || request["expires_at"].as_u64().unwrap_or(u64::MAX) > lease_end
            || materialization["observed_at"].as_u64().unwrap_or(u64::MAX) > now
        {
            return Err(refuse(
                "stale",
                "the environment lease must cover this activation's lifetime",
            ));
        }
        contracts::parse_reservation(&self.json(&request["reservation"])?).map_err(contract)?;
        self.bytes(&request["plan"])?;
        self.bytes(&request["claim"])?;
        Ok(())
    }

    fn activate(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        Self::authority(state, &agent, signer)?;
        let key = format!(
            "{agent}:{}",
            body["activation"].as_str().unwrap_or_default()
        );
        // The binding outlives release and recovery: an old ID never reopens.
        if let Some(binding) = state.bindings.get(&key) {
            if binding.fingerprint == digest_of(input) {
                let mut result = binding.result.clone();
                result["status"] = json!("duplicate");
                return Ok(result);
            }
            return Err(refuse(
                "idempotency_conflict",
                "activation ID already bound to other bytes",
            ));
        }
        if let Some(holder) = state.claims.get(&agent)
            && state
                .activations
                .get(holder)
                .is_some_and(|a| matches!(a.status.as_str(), "active" | "fencing"))
        {
            return Err(refuse(
                "conflict",
                "another lifecycle holds this agent's claim",
            ));
        }
        self.check_request(state, body, now)?;
        let (generation, checkpoint, unknown) = if body["checkpoint"].is_null() {
            (1, Value::Null, Vec::new())
        } else {
            // Recovery into a new activation from a predecessor of this agent.
            let checkpoint = self.json(&body["checkpoint"])?;
            let predecessor = state
                .activations
                .values()
                .find(|a| a.agent == agent && same(&a.reference, &checkpoint["activation"]))
                .cloned()
                .ok_or_else(|| {
                    refuse(
                        "not_admitted",
                        "checkpoint is not from this agent's lifecycle",
                    )
                })?;
            match predecessor.status.as_str() {
                "revoked" => {
                    return Err(refuse("revoked", "a revoked lifecycle cannot be recovered"));
                }
                "stopped" | "expired" => {}
                _ => return Err(refuse("conflict", "the predecessor is not stopped")),
            }
            if !predecessor.unknown.is_empty() {
                return Err(refuse(
                    "conflict",
                    "unknown effects must be reconciled before recovery",
                ));
            }
            self.chain(&predecessor, &body["checkpoint"])?;
            (
                predecessor.generation + 1,
                body["checkpoint"].clone(),
                Vec::new(),
            )
        };
        let id = self.admit(state, body, input, generation, checkpoint, unknown, now)?;
        let activation = state.activations.get(&id).cloned().ok_or("activation")?;
        let result = self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&activation),
            None,
            None,
            now,
        )?;
        state.bindings.insert(
            key,
            Binding {
                fingerprint: digest_of(input).into(),
                result: result.clone(),
            },
        );
        Ok(result)
    }

    fn checkpoint(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let id = state
            .activations
            .values()
            .find(|a| same(&a.reference, &body["activation"]))
            .map(|a| a.id.clone())
            .ok_or_else(|| refuse("not_admitted", "unknown activation"))?;
        let mut activation = state.activations.remove(&id).ok_or("activation")?;
        if signer != activation.controller
            || body["controller"] != signer
            || body["generation"] != activation.generation
            || activation.status != "active"
        {
            // A late result from a fenced or stale controller is retained as
            // evidence only; it never extends the lifecycle.
            if signer == activation.controller {
                let late = json!({"v":"openagents.sovereign-late-result.v1","requires":[],"activation":activation.reference,
                    "generation":body["generation"],"controller":signer,"record":input,"received_at":now});
                let reference = self.sign(state, &late, &activation.controller.clone(), now)?;
                activation.late.push(reference);
            }
            state.activations.insert(id, activation.clone());
            return self.result(
                state,
                signer,
                input,
                "refused",
                Some(&activation),
                None,
                Some("stale"),
                now,
            );
        }
        let outcome = (|| -> Result<()> {
            if body["revision"] != activation.checkpoints.len() as u64
                || body["previous"]
                    != activation
                        .checkpoints
                        .last()
                        .cloned()
                        .unwrap_or(Value::Null)
            {
                return Err(refuse("conflict", "checkpoint lineage"));
            }
            let record =
                nostr::run::parse_envelope(&self.json(&body["record"])?).map_err(contract)?;
            if record.run != activation.id
                || record.controller != activation.controller
                || record.generation != activation.generation
            {
                return Err(refuse(
                    "identity_mismatch",
                    "the record is not this activation's scheduler cut",
                ));
            }
            let obligations: Vec<&str> = body["obligations"]
                .as_array()
                .into_iter()
                .flatten()
                .map(digest_of)
                .collect();
            if record.data.get("state").and_then(Value::as_str) != Some(digest_of(&body["state"]))
                || record
                    .data
                    .get("obligations")
                    .and_then(Value::as_array)
                    .map(|o| o.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    != Some(obligations)
            {
                return Err(refuse(
                    "identity_mismatch",
                    "the record does not bind this state and obligation set",
                ));
            }
            let state_bytes = self.bytes(&body["state"])?;
            let profile = self.json(
                state
                    .config
                    .profiles
                    .get(&activation.agent)
                    .ok_or("profile")?,
            )?;
            if let Ok(schema) = self.bytes(&profile["state_schema"]) {
                let digest = digest_bytes(&schema);
                let closure =
                    contracts::prepare_closure(&BTreeMap::from([(digest.clone(), schema)]))
                        .map_err(contract)?;
                let value =
                    contracts::parse_strict_bounded(&state_bytes, 1_048_576).map_err(contract)?;
                contracts::validate_instance(&closure, &digest, &value).map_err(|_| {
                    refuse(
                        "malformed",
                        "state does not conform to the profile's state schema",
                    )
                })?;
            }
            for obligation in body["obligations"].as_array().into_iter().flatten() {
                self.bytes(obligation)?;
            }
            activation.checkpoints.push(input.clone());
            Ok(())
        })();
        state.activations.insert(id, activation.clone());
        outcome?;
        self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&activation),
            None,
            None,
            now,
        )
    }

    fn transfer(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        // Finding a host, or that host's own signature, grants nothing.
        Self::authority(state, &agent, signer)?;
        let id = state
            .claims
            .get(&agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "no lifecycle claim"))?;
        let mut from = state.activations.get(&id).cloned().ok_or("activation")?;
        if !same(&from.reference, &body["from"]) {
            return Err(refuse(
                "stale",
                "the transfer names a lifecycle that does not hold the claim",
            ));
        }
        if from.status != "active" || from.transfer.is_some() {
            return Err(refuse("conflict", "a transfer is already in progress"));
        }
        if body["to"]["agent"] != agent.as_str() || body["to"]["checkpoint"] != body["checkpoint"] {
            return Err(refuse(
                "identity_mismatch",
                "the new activation must start from this checkpoint",
            ));
        }
        if body["to"]["controller"] == from.controller.as_str() {
            return Err(refuse("incompatible", "a transfer moves to another host"));
        }
        if !same(&body["to"]["reservation"], &from.reservation) {
            return Err(refuse(
                "conflict",
                "child reservations are preserved across transfer",
            ));
        }
        let key = format!(
            "{agent}:{}",
            body["to"]["activation"].as_str().unwrap_or_default()
        );
        if state.bindings.contains_key(&key) {
            return Err(refuse("conflict", "the new activation ID is already bound"));
        }
        self.chain(&from, &body["checkpoint"])?;
        self.check_request(state, &body["to"], now)?;
        // Stop new dispatch now; completion waits for fencing and acceptance.
        from.status = "fencing".into();
        from.transfer = Some(Transfer {
            reference: input.clone(),
            body: body.clone(),
            acceptance: None,
        });
        state.activations.insert(id, from.clone());
        self.result(
            state,
            signer,
            input,
            "pending",
            Some(&from),
            None,
            None,
            now,
        )
    }

    fn pending_transfer(state: &State, transfer: &Value) -> Result<String> {
        state
            .activations
            .values()
            .find(|a| {
                a.transfer
                    .as_ref()
                    .is_some_and(|t| same(&t.reference, transfer))
            })
            .map(|a| a.id.clone())
            .ok_or_else(|| refuse("not_admitted", "no such pending transfer"))
    }

    fn accept(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let id = Self::pending_transfer(state, &body["transfer"])?;
        let mut from = state.activations.remove(&id).ok_or("activation")?;
        let outcome = (|| -> Result<()> {
            let transfer = from.transfer.as_mut().ok_or("transfer")?;
            if signer != transfer.body["to"]["controller"] || body["controller"] != signer {
                return Err(refuse("not_admitted", "only the target controller accepts"));
            }
            if body["environment"] != transfer.body["to"]["environment"] {
                return Err(refuse(
                    "identity_mismatch",
                    "acceptance names another placement",
                ));
            }
            transfer.acceptance = Some(input.clone());
            Ok(())
        })();
        state.activations.insert(id.clone(), from);
        outcome?;
        self.complete(state, &id, input, signer, now)
    }

    fn fence(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let id = state
            .activations
            .values()
            .find(|a| same(&a.reference, &body["activation"]))
            .map(|a| a.id.clone())
            .ok_or_else(|| refuse("not_admitted", "unknown activation"))?;
        let mut activation = state.activations.remove(&id).ok_or("activation")?;
        let outcome = (|| -> Result<()> {
            if body["dispatcher"] != signer || !activation.dispatchers.iter().any(|d| d == signer) {
                return Err(refuse("not_admitted", "not a participating dispatcher"));
            }
            if activation.status != "fencing" || body["generation"] != activation.generation {
                return Err(refuse("stale", "nothing to fence at that generation"));
            }
            for unknown in body["unknown"].as_array().into_iter().flatten() {
                if !activation.unknown.contains(unknown) {
                    activation.unknown.push(unknown.clone());
                }
            }
            activation.fencing.insert(
                signer.into(),
                (
                    body["status"].as_str().unwrap_or_default().into(),
                    input.clone(),
                ),
            );
            Ok(())
        })();
        state.activations.insert(id.clone(), activation);
        outcome?;
        self.complete(state, &id, input, signer, now)
    }

    /// Finish a transfer once the authority signed it, the target accepted,
    /// and every old dispatcher acknowledged fencing.
    fn complete(
        &self,
        state: &mut State,
        id: &str,
        input: &Value,
        signer: &str,
        now: u64,
    ) -> Result<Value> {
        let from = state.activations.get(id).cloned().ok_or("activation")?;
        let transfer = from.transfer.clone().ok_or("transfer")?;
        let fenced = from
            .dispatchers
            .iter()
            .all(|d| from.fencing.get(d).is_some_and(|(s, _)| s == "fenced"));
        if !fenced || transfer.acceptance.is_none() {
            return self.result(
                state,
                signer,
                input,
                "pending",
                Some(&from),
                None,
                None,
                now,
            );
        }
        // A target may expire while its controller or dispatchers are offline.
        // Retain valid fencing evidence even when final admission refuses.
        if let Err(error) = self.check_request(state, &transfer.body["to"], now) {
            let status = if code(&error) == "conflict" {
                "conflict"
            } else {
                "refused"
            };
            return self.result(
                state,
                signer,
                input,
                status,
                Some(&from),
                None,
                Some(code(&error)),
                now,
            );
        }
        let generation = from.generation + 1;
        let new_id = self.admit(
            state,
            &transfer.body["to"],
            &transfer.reference,
            generation,
            transfer.body["checkpoint"].clone(),
            from.unknown.clone(),
            now,
        )?;
        let key = format!("{}:{new_id}", from.agent);
        let mut old = state.activations.remove(id).ok_or("activation")?;
        old.status = "transferred".into();
        old.successor = Some(new_id.clone());
        state.activations.insert(id.into(), old);
        let new = state
            .activations
            .get(&new_id)
            .cloned()
            .ok_or("activation")?;
        let result = self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&new),
            None,
            None,
            now,
        )?;
        state.bindings.insert(
            key,
            Binding {
                fingerprint: digest_bytes(&contracts::jcs(&transfer.body["to"]).map_err(contract)?),
                result: result.clone(),
            },
        );
        Ok(result)
    }

    fn control(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        Self::authority(state, &agent, signer)?;
        let id = state
            .activations
            .values()
            .find(|a| a.agent == agent && same(&a.reference, &body["activation"]))
            .map(|a| a.id.clone())
            .ok_or_else(|| refuse("not_admitted", "unknown activation"))?;
        let mut activation = state.activations.remove(&id).ok_or("activation")?;
        let outcome = (|| -> Result<()> {
            match body["action"].as_str() {
                Some("stop")
                    if matches!(activation.status.as_str(), "active" | "fencing")
                        && activation.transfer.is_none() =>
                {
                    activation.status = "stopped".into();
                }
                Some("revoke") if !matches!(activation.status.as_str(), "transferred") => {
                    activation.status = "revoked".into();
                    activation.transfer = None;
                }
                Some("rollback")
                    if activation.status == "fencing" && activation.successor.is_none() =>
                {
                    // Return to the same host under a fresh generation; the
                    // fenced generation stays fenced.
                    activation.status = "active".into();
                    activation.transfer = None;
                    activation.fencing.clear();
                    activation.generation += 1;
                }
                _ => return Err(refuse("conflict", "lifecycle transition is not allowed")),
            }
            Ok(())
        })();
        state.activations.insert(id.clone(), activation.clone());
        outcome?;
        if matches!(activation.status.as_str(), "stopped" | "revoked")
            && state.claims.get(&agent) == Some(&id)
        {
            state.claims.remove(&agent);
        }
        self.result(
            state,
            signer,
            input,
            "accepted",
            Some(&activation),
            None,
            None,
            now,
        )
    }

    /// The dispatcher check: only the current controller and generation of
    /// an active lifecycle that holds the claim may dispatch.
    pub fn check_dispatch(
        &self,
        agent: &str,
        activation: &str,
        controller: &str,
        generation: u64,
        now: u64,
    ) -> Result<()> {
        let state = self.state()?;
        let current = state
            .claims
            .get(agent)
            .ok_or_else(|| refuse("not_admitted", "no lifecycle claim"))?;
        let a = state
            .activations
            .get(activation)
            .ok_or_else(|| refuse("not_admitted", "unknown activation"))?;
        if current != activation
            || a.agent != agent
            || a.status != "active"
            || a.controller != controller
            || a.generation != generation
            || a.expires_at <= now
        {
            return Err(refuse(
                "stale",
                "dispatch is fenced for that controller generation",
            ));
        }
        Ok(())
    }

    /// Expire lifecycles past their deadline and release their claims.
    pub fn expire(&self, now: u64) -> Result<Vec<String>> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut expired = Vec::new();
        for activation in state.activations.values_mut() {
            if activation.status == "active" && activation.expires_at <= now {
                activation.status = "expired".into();
                expired.push((activation.agent.clone(), activation.id.clone()));
            }
        }
        for (agent, id) in &expired {
            if state.claims.get(agent) == Some(id) {
                state.claims.remove(agent);
            }
        }
        self.save(&state)?;
        Ok(expired.into_iter().map(|(_, id)| id).collect())
    }

    pub fn status(&self, activation: &str) -> Result<Value> {
        let state = self.state()?;
        let a = state
            .activations
            .get(activation)
            .ok_or_else(|| refuse("not_admitted", "unknown activation"))?;
        Ok(
            json!({"activation":a.reference,"status":a.status,"controller":a.controller,"generation":a.generation,
            "checkpoints":a.checkpoints,"unknown":a.unknown,"late":a.late,"successor":a.successor,
            "claim":state.claims.get(&a.agent)}),
        )
    }
}

/// Build a signed private request envelope for a coordinator.
pub fn envelope(secret: &SecretKey, coordinator: &str, body: &Value, now: u64) -> Result<Event> {
    life::validate(body).map_err(contract)?;
    workspace_host::host::request(secret, coordinator, body.clone(), now)
}
