//! A separately keyed deterministic checker evaluates actual coordinator facts.
use crate::Result;
use nostr::{
    automation, contracts,
    domain::{Event, RelaySigner, Tag},
    execution,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::Artifacts;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub controller: String,
    pub coordinator: String,
    pub scope: Value,
    pub producer: String,
    pub rule: Value,
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
pub struct EvaluationIntake {
    authority: String,
    principal: String,
    event: String,
    payload: Value,
    input: Value,
    admission: Value,
}
impl EvaluationIntake {
    pub fn authority(&self) -> &str {
        &self.authority
    }
    pub fn principal(&self) -> &str {
        &self.principal
    }
    pub fn event(&self) -> &str {
        &self.event
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub fn input(&self) -> &Value {
        &self.input
    }
    pub fn admission(&self) -> &Value {
        &self.admission
    }
}
pub type Reply = (Event, BTreeMap<String, Vec<u8>>);
pub trait EvaluationGate {
    fn with_fence(
        &self,
        proof: &EvaluationIntake,
        now: u64,
        call: &mut dyn FnMut() -> Result<Reply>,
    ) -> Result<Reply>;
}
pub struct Evaluator {
    store: Artifacts,
    secret: secp256k1::SecretKey,
    signer: RelaySigner,
}
impl Evaluator {
    pub fn open(root: PathBuf, secret: secp256k1::SecretKey) -> Result<Self> {
        Ok(Self {
            store: Artifacts::new(root)?,
            secret,
            signer: RelaySigner::from_secret_hex(&secret.display_secret().to_string())
                .map_err(|_| "evaluation key")?,
        })
    }
    pub fn initialize(root: PathBuf, secret: secp256k1::SecretKey, policy: Policy) -> Result<Self> {
        let host = Self::open(root, secret)?;
        let _guard = host.store.lock()?;
        if host.store.root().join("state.json").exists() {
            return Err("evaluation store already exists".into());
        }
        for key in [&policy.controller, &policy.coordinator, &policy.producer] {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "evaluation principal")?;
        }
        if BTreeSet::from([
            host.authority(),
            policy.controller.as_str(),
            policy.coordinator.as_str(),
            policy.producer.as_str(),
        ])
        .len()
            != 4
            || policy.until == 0
        {
            return Err("independent finite evaluation principals required".into());
        }
        contracts::parse_artifact(&policy.scope).map_err(|e| e.to_string())?;
        contracts::parse_artifact(&policy.rule).map_err(|e| e.to_string())?;
        host.store.save(
            &State {
                v: "openagents.automation-evaluator-host.v1".into(),
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
        let state: State = self.store.load(&self.signer)?;
        if state.v != "openagents.automation-evaluator-host.v1"
            || state.authority != self.authority()
        {
            return Err("evaluation store identity differs".into());
        }
        Ok(state)
    }
    pub fn policy(&self) -> Result<Policy> {
        let _guard = self.store.lock()?;
        Ok(self.state()?.policy)
    }
    fn bundle(&self, state: &State) -> Result<Value> {
        let config=self.store.put_json(&json!({"v":"openagents.automation-native-evaluation-config.v1","requires":[],"checker":self.authority(),"policy":state.policy}))?;
        let defs: Value =
            serde_json::from_slice(automation::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let schema = |v: Value| {
            self.store.put(
                &contracts::jcs(&v).map_err(|e| e.to_string())?,
                "application/schema+json",
                None,
            )
        };
        let input =
            schema(json!({"$defs":defs["$defs"],"$ref":"#/$defs/automation-evaluation-input"}))?;
        let output = schema(json!({"$defs":defs["$defs"],"$ref":"#/$defs/automation-evaluation"}))?;
        let id = format!(
            "{}:auto/{}",
            self.authority(),
            config["digest"]
                .as_str()
                .ok_or("config")?
                .trim_start_matches("sha256:")
        );
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Check a protected completion rule against actual finished work.","input":input,"output":output,"effects":{"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{"output_bytes":1048576},"support":{"bounds":{"output_bytes":"enforced","wall_ms":"enforced"},"cancellation":"before_dispatch","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"auto-evaluator","interface":"openagents.automation-native-evaluation.v1"}});
        nostr::cap::parse_definition(&card).map_err(|e| e.to_string())?;
        let target = json!({"id":id,"artifact":self.store.put(&contracts::jcs(&card).map_err(|e|e.to_string())?,"application/json",Some("openagents.cap.v1"))?});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":config["digest"].as_str().ok_or("config")?.trim_start_matches("sha256:"),"recipient":self.authority(),"policy":config,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.automation-native-evaluation-requirements.v1","requires":[],"config":config,"admission":"signed-controller-parent-call","source":"current-coordinator-child-ledger","spend_microunits":0}))?;
        Ok(
            json!({"target":target,"lock":lock,"context":context,"requirements":requirements,"config":config}),
        )
    }
    pub fn binding(&self) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.bundle(&self.state()?)
    }
    pub fn binding_files(&self) -> Result<BTreeMap<String, Vec<u8>>> {
        let _guard = self.store.lock()?;
        let bundle = self.bundle(&self.state()?)?;
        let card = contracts::parse_strict(&self.store.get(&bundle["target"]["artifact"])?)
            .map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        for r in [
            &bundle["target"]["artifact"],
            &bundle["lock"],
            &bundle["context"],
            &bundle["requirements"],
            &bundle["config"],
            &card["input"],
            &card["output"],
        ] {
            let b = self.store.get(r)?;
            files.insert(contracts::digest_bytes(&b), b);
        }
        Ok(files)
    }
    fn cached(
        &self,
        state: &State,
        key: &str,
        event: &Event,
        execute: &execution::Execute,
        now: u64,
    ) -> Result<Option<Reply>> {
        let Some(reply) = state.replies.get(key) else {
            return Ok(None);
        };
        let claim = &state.service.claims[key];
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err("idempotency_conflict: evaluation invocation changed".into());
        }
        let files = state.files[key]
            .iter()
            .map(|r| {
                let b = self.store.get(r)?;
                Ok((contracts::digest_bytes(&b), b))
            })
            .collect::<Result<_>>()?;
        Ok(Some((reply.clone(), files)))
    }
    pub fn run(
        &self,
        coordinator: &coordination_host::book::Coordinator,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
        gate: &dyn EvaluationGate,
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
            return Err("evaluation execute required".into());
        };
        let (policy, bundle) = {
            let _guard = self.store.lock()?;
            let state = self.state()?;
            let key = state
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            if let Some(reply) = self.cached(&state, &key, event, execute, now)? {
                return Ok(reply);
            }
            (state.policy.clone(), self.bundle(&state)?)
        };
        if opened.principal != policy.controller || coordinator.authority() != policy.coordinator {
            return Err("not_admitted: evaluation controller/coordinator differs".into());
        }
        for field in ["target", "lock", "context", "requirements"] {
            if execute.payload[field] != bundle[field] {
                return Err("identity_mismatch: evaluation pins differ".into());
            }
        }
        let input_ref = execute
            .input_artifact
            .as_ref()
            .ok_or("frozen evaluation input required")?;
        let bytes = files
            .get(&input_ref.digest)
            .ok_or("evaluation input bytes")?;
        contracts::check_artifact_bytes(input_ref, bytes).map_err(|e| e.to_string())?;
        let input = automation::parse(bytes).map_err(|e| e.to_string())?;
        if input["v"] != "openagents.automation-evaluation-input.v1" {
            return Err("evaluation input schema".into());
        }
        if files.len() > 256
            || files.values().map(Vec::len).sum::<usize>() > 1048576
            || files.iter().any(|(d, b)| contracts::digest_bytes(b) != *d)
        {
            return Err("evaluation closure byte/digest bound".into());
        }
        let mut admission = None;
        let mut outcome = None;
        for bytes in files.values() {
            let Ok(candidate) = serde_json::from_slice::<Event>(bytes) else {
                continue;
            };
            if candidate.pubkey != policy.controller {
                continue;
            }
            if let Ok((actor, body)) =
                coordinator.automation_evaluation_authorization(&candidate, now)
            {
                if actor == policy.controller
                    && body["purpose"] == "check"
                    && body["request"] == execute.request
                    && body["coordinator"] == policy.scope
                    && body["fingerprint"]
                        == contracts::digest_value(&input).map_err(|e| e.to_string())?
                {
                    admission = Some((candidate, body));
                    continue;
                }
            }
            // The owner/coordinator is the recipient of the original outcome;
            // the actual helper verifies its decrypted bytes and signature.
            if let Ok((actor, body)) = coordinator.automation_outcome_authorization(&candidate, now)
            {
                if actor == policy.controller
                    && contracts::digest_value(&body).map_err(|e| e.to_string())?
                        == input["outcome"]["digest"]
                {
                    outcome = Some(candidate);
                }
            }
        }
        let (admission, aux) =
            admission.ok_or("not_admitted: original controller evaluation allowance required")?;
        let outcome = outcome.ok_or("not_admitted: original controller outcome required")?;
        let proof = EvaluationIntake {
            authority: self.authority().into(),
            principal: opened.principal.clone(),
            event: event.id.clone(),
            payload: execute.payload.clone(),
            input: input.clone(),
            admission: aux.clone(),
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
                    &input,
                    &admission,
                    &aux,
                    &outcome,
                    files,
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
        input: &Value,
        admission: &Event,
        aux: &Value,
        outcome: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        if let Some(reply) = self.cached(&state, &key, event, execute, now)? {
            return Ok(reply);
        }
        if state.service.claims.contains_key(&key) {
            return Err("unknown: interrupted evaluation was not replayed".into());
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
            return Err("cannot_enforce: evaluation bounds differ".into());
        }
        let mut pins = files.clone();
        for field in ["target", "lock", "context", "requirements"] {
            let r = if field == "target" {
                &bundle[field]["artifact"]
            } else {
                &bundle[field]
            };
            let b = self.store.get(r)?;
            pins.insert(contracts::digest_bytes(&b), b);
        }
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))?;
        if state.service.claims.len() >= 4096
            || state.service.claims.values().any(|c| c.run == execute.run)
        {
            return Err("evaluation service identity bound".into());
        }
        let (claim, root) = match state
            .service
            .prepare(opened, now, &workspace_host::artifact::token())
            .map_err(|e| format!("{e:?}"))?
        {
            execution::Admission::Reserved { claim, root } => (claim, root),
            _ => return Err("unexpected evaluation retransmission".into()),
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
        let input_ref = &execute.payload["input"]["artifact"];
        let admitted=nostr::run::append_envelope(&mut records,self.authority(),"admitted",json!({"enforcement":[],"reservations":[],"deadlines":{"request":execute.deadline},"retention":execute.retain_until,"input":input_ref}),now).map_err(|e|e.to_string())?;
        let dispatched=nostr::run::append_envelope(&mut records,self.authority(),"dispatched",json!({"binding":execute.payload["target"],"attempt":execute.attempt,"input":input_ref,"context":execute.payload["context"],"effect":"native-coordinator-evaluation","generation":0}),now).map_err(|e|e.to_string())?;
        let mut refs = vec![
            root_ref,
            self.store.put_json(&admitted)?,
            self.store.put_json(&dispatched)?,
        ];
        state.files.insert(key.clone(), refs.clone());
        self.store.save(&state, &self.signer)?;
        let request = workspace_host::host::request(
            &self.secret,
            coordinator.authority(),
            json!({"v":"openagents.coordination-native-evaluation-material.v1","requires":[],"coordinator":policy.scope,"claim":aux["claim"],"request":execute.request,"input":input_ref,"rule":policy.rule,"admission":admission,"outcome":outcome}),
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let material_event = coordinator.automation_evaluation_material(
            &request,
            files,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let opened_material = nostr::private_artifact::open(&material_event, &self.secret)
            .map_err(|e| e.to_string())?;
        if opened_material.signer() != policy.coordinator {
            return Err("not_admitted: actual coordinator evaluation data required".into());
        }
        let material = contracts::parse_strict(
            opened_material
                .inline_bytes()
                .ok_or("inline native evaluation data")?,
        )
        .map_err(|e| e.to_string())?;
        if material["input"] != *input_ref {
            return Err("identity_mismatch: coordinator evaluated different input".into());
        }
        let (decision, evidence, reason) = judge(&material, input, policy)?;
        let evaluation = json!({"v":"openagents.automation-evaluation.v1","requires":[],"input":input_ref,"decision":decision,"evidence":evidence,"reason":reason});
        automation::validate(&evaluation).map_err(|e| e.to_string())?;
        let output = self.store.put_json(&evaluation)?;
        let statement = workspace_host::host::request(
            &self.secret,
            &policy.controller,
            evaluation,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        refs.extend([
            output.clone(),
            self.store
                .put_json(&serde_json::to_value(&statement).map_err(|e| e.to_string())?)?,
            self.store
                .put_json(&serde_json::to_value(&material_event).map_err(|e| e.to_string())?)?,
        ]);
        let resolved=nostr::run::append_envelope(&mut records,self.authority(),"resolved",json!({"outcome":"completed","dispatched":true,"output":output,"receipts":[],"usage":{"calls":1,"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now.saturating_add(started.elapsed().as_secs())).map_err(|e|e.to_string())?;
        let settled=nostr::run::append_envelope(&mut records,self.authority(),"settled",json!({"outcome":"completed","unknowns":[],"verification":"not_run","integration":"not_requested","results":[output],"reservations":[]}),now.saturating_add(started.elapsed().as_secs())).map_err(|e|e.to_string())?;
        let terminal = self.store.put_json(&settled)?;
        refs.extend([self.store.put_json(&resolved)?, terminal.clone()]);
        if refs
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?))
            .is_none_or(|n| n > 1048576)
            || started.elapsed().as_millis()
                > u128::from(execute.bounds.wall_ms.ok_or("wall bound")?)
            || now.saturating_add(started.elapsed().as_secs()) >= execute.deadline
        {
            return Err("unknown: evaluation exceeded its admitted time or bytes".into());
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
                    receipts: vec![terminal],
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
            created_at: now.saturating_add(started.elapsed().as_secs()),
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
        self.cached(&state, &key, event, execute, now)?
            .ok_or("retained evaluation reply".into())
    }
}
fn judge(
    material: &Value,
    input: &Value,
    policy: &Policy,
) -> Result<(&'static str, Vec<Value>, String)> {
    if material["v"] != "openagents.coordination-native-evaluation-data.v1"
        || material["plan"] != input["plan"]
        || material["frame"] != input["frame"]
        || material["acceptance"] != input["acceptance"]
        || material["rule"] != policy.rule
        || material["producer"] != policy.producer
    {
        return Err("identity_mismatch: evaluation data differs from protected input".into());
    }
    let rule = &material["rule_body"];
    let rule_bytes = contracts::jcs(rule).map_err(|e| e.to_string())?;
    contracts::check_artifact_bytes(
        &contracts::parse_artifact(&policy.rule).map_err(|e| e.to_string())?,
        &rule_bytes,
    )
    .map_err(|e| e.to_string())?;
    if rule["v"] != "openagents.automation-native-completion-rule.v1"
        || rule["check"] != "verified-occurrence-count"
        || rule["acceptance"] != input["acceptance"]
    {
        return Err("cannot_enforce: completion rule differs".into());
    }
    let required = rule["minimum_successes"]
        .as_u64()
        .filter(|n| (1..=16).contains(n))
        .ok_or("finite completion count")?;
    if material["unknowns"]
        .as_array()
        .is_none_or(|a| !a.is_empty())
    {
        return Ok((
            "unknown",
            vec![],
            "Earlier work still needs to be checked.".into(),
        ));
    }
    let children = material["children"]
        .as_array()
        .filter(|a| a.len() <= 16)
        .ok_or("native child evidence bound")?;
    let mut identities = BTreeSet::new();
    let mut evidence = vec![];
    let mut current = false;
    for child in children {
        if child["linked"] != true
            || child["plan"] != input["plan"]
            || child["frame"] != input["frame"]
            || child["acceptance"] != input["acceptance"]
            || child["target"] != material["target"]
            || child["producer"] != policy.producer
            || child["phase"] != "released"
            || child["unknown"] != false
            || child["outcome"] != "completed"
            || child["passed"] != true
        {
            return Ok((
                "blocked",
                evidence,
                "A result did not meet the required checks.".into(),
            ));
        }
        let id = child["occurrence"]["digest"]
            .as_str()
            .ok_or("occurrence digest")?;
        if !identities.insert(id.to_string()) {
            return Ok((
                "blocked",
                evidence,
                "The evidence repeats the same occurrence.".into(),
            ));
        }
        if child["occurrence"] == input["occurrence"] {
            current = true;
        }
        for field in ["claim", "result", "check", "terminal"] {
            contracts::parse_artifact(&child[field]).map_err(|e| e.to_string())?;
            evidence.push(child[field].clone());
        }
    }
    if !current {
        return Ok((
            "unknown",
            evidence,
            "The current result is missing its checks.".into(),
        ));
    }
    let count = identities.len() as u64;
    Ok((
        if count >= required {
            "complete"
        } else {
            "continue"
        },
        evidence,
        format!("{count} of {required} required results passed their checks."),
    ))
}
