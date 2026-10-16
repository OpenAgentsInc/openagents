//! One fixed native source and one CJ identity book per source principal.
use crate::Result;
use nostr::{
    contracts,
    domain::{Event, RelaySigner, Tag},
    execution,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use workspace_host::artifact::Artifacts;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub controller: String,
    pub coordinator: String,
    pub scope: Value,
    pub resource: String,
    pub until: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    v: String,
    authority: String,
    policy: Policy,
    service: execution::Service,
    replies: BTreeMap<String, Event>,
    files: BTreeMap<String, Vec<Value>>,
}
/// Only this native source constructs a proof, after verifying both signatures.
pub struct SourceIntake {
    authority: String,
    principal: String,
    event: String,
    fingerprint: String,
    payload: Value,
    admission: Value,
}
impl SourceIntake {
    pub fn authority(&self) -> &str {
        &self.authority
    }
    pub fn principal(&self) -> &str {
        &self.principal
    }
    pub fn event(&self) -> &str {
        &self.event
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub fn admission(&self) -> &Value {
        &self.admission
    }
}
pub type Reply = (Event, BTreeMap<String, Vec<u8>>);
/// The controller holds its pause/cancel fence throughout the actual source call.
pub trait SourceGate {
    fn with_fence(
        &self,
        proof: &SourceIntake,
        now: u64,
        call: &mut dyn FnMut() -> Result<Reply>,
    ) -> Result<Reply>;
}
pub struct Source {
    store: Artifacts,
    secret: secp256k1::SecretKey,
    signer: RelaySigner,
}
impl Source {
    pub fn open(root: PathBuf, secret: secp256k1::SecretKey) -> Result<Self> {
        Ok(Self {
            store: Artifacts::new(root)?,
            secret,
            signer: RelaySigner::from_secret_hex(&secret.display_secret().to_string())
                .map_err(|_| "source key")?,
        })
    }
    pub fn initialize(root: PathBuf, secret: secp256k1::SecretKey, policy: Policy) -> Result<Self> {
        let host = Self::open(root, secret)?;
        let _guard = host.store.lock()?;
        if host.store.root().join("state.json").exists() {
            return Err("source store already exists".into());
        }
        for key in [&policy.controller, &policy.coordinator] {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "source principal")?;
        }
        if [policy.controller.as_str(), policy.coordinator.as_str()].contains(&host.authority())
            || policy.controller == policy.coordinator
            || policy.resource.is_empty()
            || policy.resource.len() > 1024
            || policy.until == 0
        {
            return Err("separate finite source binding required".into());
        }
        contracts::parse_artifact(&policy.scope).map_err(|e| e.to_string())?;
        host.store.save(
            &State {
                v: "openagents.automation-source-host.v1".into(),
                authority: host.authority().into(),
                service: execution::Service::new(
                    host.authority(),
                    1,
                    policy.until.saturating_add(86400),
                ),
                policy,
                replies: BTreeMap::new(),
                files: BTreeMap::new(),
            },
            &host.signer,
        )?;
        drop(_guard);
        Ok(host)
    }
    pub fn authority(&self) -> &str {
        self.signer.pubkey()
    }
    fn state(&self) -> Result<State> {
        let s: State = self.store.load(&self.signer)?;
        if s.v != "openagents.automation-source-host.v1" || s.authority != self.authority() {
            return Err("source store identity differs".into());
        }
        Ok(s)
    }
    fn bundle(&self, state: &State) -> Result<Value> {
        let config=self.store.put_json(&json!({"v":"openagents.automation-native-source-config.v1","requires":[],"source":self.authority(),"policy":state.policy}))?;
        let defs: Value =
            serde_json::from_slice(nostr::coordination::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let schema = |value: Value| {
            self.store.put(
                &contracts::jcs(&value).map_err(|e| e.to_string())?,
                "application/schema+json",
                None,
            )
        };
        let input = schema(
            json!({"$defs":{"event":defs["$defs"]["event"]},"type":"object","additionalProperties":false,"required":["v","requires","config","admission"],"properties":{"v":{"const":"openagents.automation-native-source-call.v1"},"requires":{"type":"array","maxItems":0},"config":{"const":config},"admission":{"$ref":"#/$defs/event"}}}),
        )?;
        let auto: Value =
            serde_json::from_slice(nostr::automation::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let output =
            schema(json!({"$defs":auto["$defs"],"$ref":"#/$defs/automation-observation"}))?;
        let id = format!(
            "{}:auto/{}",
            self.authority(),
            config["digest"]
                .as_str()
                .ok_or("config digest")?
                .trim_start_matches("sha256:")
        );
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Observe one explicitly granted workspace source.","input":input,"output":output,"effects":{"reads":["artifact",state.policy.resource],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{"output_bytes":1048576},"support":{"bounds":{"output_bytes":"enforced","wall_ms":"enforced"},"cancellation":"before_dispatch","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"auto-source","interface":"openagents.automation-native-source.v1"}});
        nostr::cap::parse_definition(&card).map_err(|e| e.to_string())?;
        let target = json!({"id":id,"artifact":self.store.put(&contracts::jcs(&card).map_err(|e|e.to_string())?,"application/json",Some("openagents.cap.v1"))?});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":config["digest"].as_str().ok_or("config")?.trim_start_matches("sha256:"),"recipient":self.authority(),"policy":config,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.automation-native-source-requirements.v1","requires":[],"config":config,"content_bytes":524288,"spend_microunits":0}))?;
        Ok(
            json!({"target":target,"lock":lock,"context":context,"requirements":requirements,"config":config}),
        )
    }
    pub fn policy(&self) -> Result<Policy> {
        let _guard = self.store.lock()?;
        Ok(self.state()?.policy)
    }
    pub fn binding(&self) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.bundle(&self.state()?)
    }
    pub fn binding_files(&self) -> Result<BTreeMap<String, Vec<u8>>> {
        let _guard = self.store.lock()?;
        let bundle = self.bundle(&self.state()?)?;
        let mut result = BTreeMap::new();
        let card = contracts::parse_strict(&self.store.get(&bundle["target"]["artifact"])?)
            .map_err(|e| e.to_string())?;
        for r in [
            &bundle["target"]["artifact"],
            &bundle["lock"],
            &bundle["context"],
            &bundle["requirements"],
            &bundle["config"],
            &card["input"],
            &card["output"],
        ] {
            let bytes = self.store.get(r)?;
            result.insert(contracts::digest_bytes(&bytes), bytes);
        }
        Ok(result)
    }
    pub fn run(
        &self,
        coordinator: &coordination_host::book::Coordinator,
        event: &Event,
        now: u64,
        gate: &dyn SourceGate,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("source execute required".into());
        };
        let (policy, bundle) = {
            let _guard = self.store.lock()?;
            let state = self.state()?;
            let key = state
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            if let Some(reply) = state.replies.get(&key) {
                let claim = &state.service.claims[&key];
                if claim.execute_event != event.id
                    || claim.fingerprint != execute.fingerprint
                    || now > claim.retain_until
                {
                    return Err("idempotency_conflict: source invocation changed".into());
                }
                return Ok((
                    reply.clone(),
                    state.files[&key]
                        .iter()
                        .map(|r| {
                            let b = self.store.get(r)?;
                            Ok((contracts::digest_bytes(&b), b))
                        })
                        .collect::<Result<_>>()?,
                ));
            }
            (state.policy.clone(), self.bundle(&state)?)
        };
        if opened.principal != policy.controller || coordinator.authority() != policy.coordinator {
            return Err("not_admitted: source controller/coordinator differs".into());
        }
        for field in ["target", "lock", "context", "requirements"] {
            if execute.payload[field] != bundle[field] {
                return Err("identity_mismatch: source pins differ".into());
            }
        }
        if execute.input_artifact.is_some()
            || execute.input["v"] != "openagents.automation-native-source-call.v1"
            || execute.input["requires"] != json!([])
            || execute.input["config"] != bundle["config"]
            || execute.input.as_object().is_none_or(|o| o.len() != 4)
        {
            return Err("cannot_enforce: closed native source input required".into());
        }
        let admission: Event = serde_json::from_value(execute.input["admission"].clone())
            .map_err(|_| "original source admission")?;
        let (actor, body) = coordinator.automation_source_authorization(&admission, now)?;
        if actor != policy.controller
            || body["executor"] != self.authority()
            || body["resource"] != policy.resource
            || body["coordinator"] != policy.scope
            || body["request"] != execute.request
            || body["source"] != coordinator.automation_source_operation(&policy.resource)?
        {
            return Err(
                "identity_mismatch: source admission differs from original controller call".into(),
            );
        }
        let proof = SourceIntake {
            authority: self.authority().into(),
            principal: opened.principal.clone(),
            event: event.id.clone(),
            fingerprint: execute.fingerprint.clone(),
            payload: execute.payload.clone(),
            admission: body,
        };
        gate.with_fence(
            &proof,
            now.saturating_add(started.elapsed().as_secs()),
            &mut || {
                self.execute(
                    coordinator,
                    event,
                    &opened,
                    execute,
                    &policy,
                    &bundle,
                    &admission,
                    now.saturating_add(started.elapsed().as_secs()),
                )
            },
        )
    }

    fn execute(
        &self,
        coordinator: &coordination_host::book::Coordinator,
        event: &Event,
        opened: &execution::Opened,
        execute: &execution::Execute,
        policy: &Policy,
        bundle: &Value,
        admission: &Event,
        now: u64,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        if let Some(response) = state.replies.get(&key) {
            let claim = &state.service.claims[&key];
            if claim.execute_event != event.id
                || claim.fingerprint != execute.fingerprint
                || now > claim.retain_until
            {
                return Err("idempotency_conflict: source invocation changed".into());
            }
            return Ok((
                response.clone(),
                state.files[&key]
                    .iter()
                    .map(|r| {
                        let b = self.store.get(r)?;
                        Ok((contracts::digest_bytes(&b), b))
                    })
                    .collect::<Result<_>>()?,
            ));
        }
        if state.service.claims.contains_key(&key) {
            return Err("unknown: interrupted source execution was not replayed".into());
        }
        execution::open_request(
            event,
            self.authority(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        if now >= policy.until
            || execute.deadline > policy.until
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute.bounds.wall_ms.is_none_or(|n| n == 0 || n > 30000)
            || execute.bounds.output_bytes != Some(1048576)
        {
            return Err("cannot_enforce: source bounds differ".into());
        }
        let mut pins = self.binding_files_unlocked(bundle)?;
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))?;
        if state.service.claims.len() >= 4096
            || state.service.claims.values().any(|c| c.run == execute.run)
        {
            return Err("source service identity bound".into());
        }
        let (claim, root) = match state
            .service
            .prepare(opened, now, &workspace_host::artifact::token())
            .map_err(|e| format!("{e:?}"))?
        {
            execution::Admission::Reserved { claim, root } => (claim, root),
            _ => return Err("unexpected source retransmission".into()),
        };
        let root_ref =
            self.store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
        state.files.insert(key.clone(), vec![root_ref.clone()]);
        self.store.save(&state, &self.signer)?;
        state
            .service
            .acknowledge(&key, &root)
            .map_err(|e| format!("{e:?}"))?;
        state.service.intend(&key).map_err(|e| format!("{e:?}"))?;
        let mut records = vec![contracts::parse_strict(&root).map_err(|e| e.to_string())?];
        let input_ref = self.store.put_json(&execute.input)?;
        let admitted=nostr::run::append_envelope(&mut records,self.authority(),"admitted",json!({"enforcement":[],"reservations":[],"deadlines":{"request":execute.deadline},"retention":execute.retain_until,"input":input_ref}),now).map_err(|e|e.to_string())?;
        let dispatched=nostr::run::append_envelope(&mut records,self.authority(),"dispatched",json!({"binding":execute.payload["target"],"attempt":execute.attempt,"input":input_ref,"context":execute.payload["context"],"effect":"native-workspace-read","generation":0}),now).map_err(|e|e.to_string())?;
        let preflight_refs = vec![
            root_ref.clone(),
            self.store.put_json(&admitted)?,
            self.store.put_json(&dispatched)?,
        ];
        state.files.insert(key.clone(), preflight_refs.clone());
        self.store.save(&state, &self.signer)?;
        let (original, files) = coordinator
            .automation_observe(admission, now.saturating_add(started.elapsed().as_secs()))?;
        let mut observation = files
            .values()
            .filter_map(|b| contracts::parse_strict(b).ok())
            .find(|v| v["v"] == "openagents.automation-observation.v1")
            .ok_or("native source observation unavailable")?;
        if observation["available"] == true {
            let snapshot = contracts::parse_strict(
                files
                    .get(
                        observation["evidence"]["digest"]
                            .as_str()
                            .ok_or("snapshot")?,
                    )
                    .ok_or("snapshot bytes")?,
            )
            .map_err(|e| e.to_string())?;
            if snapshot["content"]["size"]
                .as_u64()
                .is_none_or(|n| n > 524288)
            {
                return Err("limit_exceeded: native source content exceeds its ceiling".into());
            }
        }
        observation["source"] = bundle["target"].clone();
        nostr::automation::validate(&observation).map_err(|e| e.to_string())?;
        let statement = workspace_host::host::request(
            &self.secret,
            &policy.controller,
            observation.clone(),
            now,
        )?;
        let output = self.store.put_json(&observation)?;
        let original_ref = self
            .store
            .put_json(&serde_json::to_value(&original).map_err(|e| e.to_string())?)?;
        let statement_ref = self
            .store
            .put_json(&serde_json::to_value(&statement).map_err(|e| e.to_string())?)?;
        let resolved=nostr::run::append_envelope(&mut records,self.authority(),"resolved",json!({"outcome":"completed","dispatched":true,"output":output,"receipts":[],"usage":{"calls":1,"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now.saturating_add(started.elapsed().as_secs())).map_err(|e|e.to_string())?;
        let settled=nostr::run::append_envelope(&mut records,self.authority(),"settled",json!({"outcome":"completed","unknowns":[],"verification":"not_run","integration":"not_requested","results":[output],"reservations":[]}),now.saturating_add(started.elapsed().as_secs())).map_err(|e|e.to_string())?;
        let terminal_ref = self.store.put_json(&settled)?;
        let mut refs = preflight_refs;
        refs.extend([
            output.clone(),
            original_ref,
            statement_ref,
            self.store.put_json(&resolved)?,
            terminal_ref.clone(),
        ]);
        for bytes in files.values() {
            let r = self.store.put(bytes, "application/octet-stream", None)?;
            if !refs.iter().any(|p| p["digest"] == r["digest"]) {
                refs.push(r);
            }
        }
        if refs
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?))
            .is_none_or(|n| n > 1048576)
            || started.elapsed().as_millis()
                > u128::from(execute.bounds.wall_ms.ok_or("wall bound")?)
            || now.saturating_add(started.elapsed().as_secs()) >= execute.deadline
        {
            return Err(
                "unknown: source response exceeded its admitted deadline or byte ceiling".into(),
            );
        }
        let result = state
            .service
            .observe(
                &claim.key,
                execution::Report {
                    outcome: "completed",
                    dispatched: true,
                    output: Some(output),
                    artifacts: refs.clone(),
                    receipts: vec![terminal_ref],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let peer = policy
            .controller
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "controller")?;
        let response = execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            execution::RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), event.id.clone()]),
                Tag::new(vec!["p".into(), policy.controller.clone()]),
            ],
            &result,
        )
        .map_err(|e| format!("{e:?}"))?;
        state.files.insert(key.clone(), refs);
        state.replies.insert(key.clone(), response.clone());
        self.store.save(&state, &self.signer)?;
        pins.clear();
        for r in &state.files[&key] {
            let b = self.store.get(r)?;
            pins.insert(contracts::digest_bytes(&b), b);
        }
        Ok((response, pins))
    }
    fn binding_files_unlocked(&self, bundle: &Value) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut pins = BTreeMap::new();
        for field in ["target", "lock", "context", "requirements"] {
            let r = if field == "target" {
                &bundle[field]["artifact"]
            } else {
                &bundle[field]
            };
            let b = self.store.get(r)?;
            pins.insert(contracts::digest_bytes(&b), b);
        }
        Ok(pins)
    }
}
