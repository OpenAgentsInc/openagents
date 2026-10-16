//! A single durable CJ identity book serves this controller's plan operations.
use crate::{
    Result,
    host::{Automation, State},
};
use nostr::{
    contracts,
    domain::{Event, Tag},
    execution,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Automation {
    pub(crate) fn bundle(&self, state: &State) -> Result<Value> {
        let schemas: Value =
            serde_json::from_slice(nostr::automation::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let schema = |value: Value| {
            self.store.put(
                &contracts::jcs(&value).map_err(|e| e.to_string())?,
                "application/schema+json",
                None,
            )
        };
        let input = schema(
            json!({"$defs":schemas["$defs"],"oneOf":[{"$ref":"#/$defs/automation-plan"},{"$ref":"#/$defs/automation-control"}]}),
        )?;
        let output = schema(json!({"$defs":schemas["$defs"],"$ref":"#/$defs/automation-result"}))?;
        let id = format!("{}:auto/controller", self.authority());
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Manage owner-selected finite scheduled tasks.","input":input,"output":output,"effects":{"reads":["automation"],"writes":["automation"],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{"output_bytes":65536},"support":{"bounds":{"output_bytes":"enforced"},"cancellation":"before_dispatch","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"auto-controller","interface":"openagents.automation-controller.v1"}});
        nostr::cap::parse_definition(&card).map_err(|e| e.to_string())?;
        let target = json!({"id":id,"artifact":self.store.put(&contracts::jcs(&card).map_err(|e|e.to_string())?,"application/json",Some("openagents.cap.v1"))?});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let task = state.policy_ref["digest"]
            .as_str()
            .ok_or("policy digest")?
            .trim_start_matches("sha256:");
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":self.authority(),"policy":state.policy_ref,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.automation-controller-requirements.v1","requires":[],"policy":state.policy_ref,"operation":"plan-management-only"}))?;
        Ok(json!({"target":target,"lock":lock,"context":context,"requirements":requirements}))
    }
    /// Local owner distribution of an immutable admitted endpoint bundle.
    pub fn transport(&self) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.bundle(&self.state()?)
    }
    fn check_request(
        &self,
        state: &State,
        opened: &execution::Opened,
        execute: &execution::Execute,
        now: u64,
    ) -> Result<()> {
        let bundle = self.bundle(state)?;
        for field in ["target", "lock", "context", "requirements"] {
            if execute.payload[field] != bundle[field] {
                return Err("identity_mismatch: AUTO endpoint pins differ".into());
            }
        }
        if execute.input_artifact.is_some()
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute.bounds.wall_ms.is_none_or(|v| v == 0 || v > 30000)
            || execute
                .bounds
                .output_bytes
                .is_none_or(|v| !(65536..=1048576).contains(&v))
            || execute.deadline > state.policy.until
            || now >= state.policy.until
            || !state.policy.owners.contains(&opened.principal)
        {
            return Err("cannot_enforce: AUTO endpoint bounds or current owner differ".into());
        }
        nostr::automation::validate(&execute.input).map_err(|e| e.to_string())?;
        let owner = match execute.input["v"].as_str() {
            Some("openagents.automation-plan.v1") => {
                execute.input["owner"].as_str().ok_or("owner")?
            }
            Some("openagents.automation-control.v1") => {
                let id = Self::plan_key(state, &execute.input["plan"])?;
                let plan = &state.plans[&id];
                let required = 65536u64
                    .checked_add(
                        plan.state["accounting"]["size"]
                            .as_u64()
                            .ok_or("accounting size")?,
                    )
                    .ok_or("response bytes")?;
                if execute.bounds.output_bytes.is_none_or(|n| n < required) {
                    return Err(
                        "cannot_enforce: output allowance cannot hold current accounting evidence"
                            .into(),
                    );
                }
                plan.body["owner"].as_str().ok_or("owner")?
            }
            _ => return Err("unsupported_feature: AUTO endpoint input".into()),
        };
        if owner != opened.principal {
            return Err("not_admitted: signed plan owner required".into());
        }
        let mut pins = BTreeMap::new();
        for field in ["target", "lock", "context", "requirements"] {
            let reference = if field == "target" {
                &bundle[field]["artifact"]
            } else {
                &bundle[field]
            };
            let bytes = self.store.get(reference)?;
            pins.insert(contracts::digest_bytes(&bytes), bytes);
        }
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))
    }
    pub(crate) fn authenticated_cj(&self, event: &Event, now: u64) -> Result<(String, Value)> {
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("AUTO execute required".into());
        };
        let _guard = self.store.lock()?;
        let state = self.state()?;
        self.current_invocation(&state, event)?;
        self.check_request(&state, &opened, execute, now)?;
        Ok((opened.principal, execute.input.clone()))
    }
    pub(crate) fn current_invocation(&self, state: &State, event: &Event) -> Result<()> {
        if event.kind != execution::REQUEST_KIND {
            return Ok(());
        }
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("AUTO execute required".into());
        };
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = state
            .service
            .claims
            .get(&key)
            .ok_or("not_admitted: use the durable AUTO service")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || !claim.dispatch_intent
            || claim.result.is_some()
        {
            return Err("revoked: AUTO invocation was reconciled or replaced".into());
        }
        Ok(())
    }
    pub(crate) fn remember_invocation(&self, state: &mut State, event: &Event, value: &Value) {
        if event.kind == execution::REQUEST_KIND {
            state.invocations.insert(event.id.clone(), value.clone());
        }
    }
    fn reply_files(&self, state: &State, key: &str) -> Result<BTreeMap<String, Vec<u8>>> {
        state
            .reply_artifacts
            .get(key)
            .ok_or("CJ reply artifacts")?
            .iter()
            .map(|r| {
                let bytes = self.store.get(r)?;
                Ok((contracts::digest_bytes(&bytes), bytes))
            })
            .collect()
    }
    fn finish_invocation(
        &self,
        state: &mut State,
        event: &Event,
        opened: &execution::Opened,
        execute: &execution::Execute,
        value: Value,
        dispatched: bool,
        now: u64,
    ) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let output = self.store.put_json(&value)?;
        let mut artifacts = state.reply_artifacts.get(&key).ok_or("CJ root")?.clone();
        artifacts.truncate(1);
        artifacts.push(output.clone());
        if contracts::parse_artifact(&value["state"]).is_ok() {
            artifacts.push(value["state"].clone());
            let body = self.store.json(&value["state"])?;
            artifacts.push(body["record"].clone());
            artifacts.push(body["accounting"].clone());
        }
        let total = artifacts
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?));
        let exceeded = total.is_none_or(|n| n > execute.bounds.output_bytes.unwrap_or(0));
        if exceeded {
            artifacts.truncate(1);
        }
        let root = self.store.get(&artifacts[0])?;
        state
            .service
            .acknowledge(&key, &root)
            .map_err(|e| format!("{e:?}"))?;
        let result = state
            .service
            .observe(
                &key,
                execution::Report {
                    outcome: if exceeded { "failed" } else { "completed" },
                    dispatched,
                    output: (!exceeded).then_some(output),
                    artifacts: artifacts.clone(),
                    receipts: vec![],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: exceeded.then(|| "limit_exceeded".into()),
                    message: exceeded.then(|| "The response exceeded its byte limit.".into()),
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let peer = opened
            .principal
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "caller key")?;
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
                Tag::new(vec!["p".into(), opened.principal.clone()]),
            ],
            &result,
        )
        .map_err(|e| format!("{e:?}"))?;
        state.reply_artifacts.insert(key.clone(), artifacts);
        state.replies.insert(key.clone(), response.clone());
        self.save(state)?;
        Ok((response, self.reply_files(state, &key)?))
    }
    pub fn serve(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("AUTO execute required".into());
        };
        let key;
        {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            key = state
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            if let Some(reply) = state.replies.get(&key) {
                let claim = &state.service.claims[&key];
                if claim.fingerprint != execute.fingerprint
                    || claim.execute_event != event.id
                    || now > claim.retain_until
                {
                    return Err("idempotency_conflict: changed or expired AUTO invocation".into());
                }
                return Ok((reply.clone(), self.reply_files(&state, &key)?));
            }
            if state.service.claims.contains_key(&key) {
                return Err(
                    "unknown: interrupted AUTO invocation requires recovery; no replay".into(),
                );
            }
            execution::open_request(
                event,
                self.authority(),
                &self.secret,
                now,
                execution::Window::DEFAULT,
            )
            .map_err(|e| format!("{e:?}"))?;
            self.check_request(&state, &opened, execute, now)?;
            if state.service.claims.len() >= 4096
                || state.service.claims.values().any(|c| c.run == execute.run)
                || state.plans.values().any(|p| p.body["run"] == execute.run)
                || execute.input["run"] == execute.run
            {
                return Err("conflict: retained RUN identity or service bound".into());
            }
            let (claim, root) = match state
                .service
                .prepare(&opened, now, &workspace_host::artifact::token())
                .map_err(|e| format!("{e:?}"))?
            {
                execution::Admission::Reserved { claim, root } => (claim, root),
                _ => return Err("unexpected AUTO retransmission".into()),
            };
            let root_ref =
                self.store
                    .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
            state.reply_artifacts.insert(key.clone(), vec![root_ref]);
            self.save(&state)?;
            state
                .service
                .acknowledge(&claim.key, &root)
                .map_err(|e| format!("{e:?}"))?;
            state
                .service
                .intend(&claim.key)
                .map_err(|e| format!("{e:?}"))?;
            self.save(&state)?;
        }
        let output = match execute.input["v"].as_str() {
            Some("openagents.automation-plan.v1") => self.admit(event, files, now),
            Some("openagents.automation-control.v1") => self.control(event, now),
            _ => Err("unsupported_feature: AUTO operation".into()),
        };
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        if let Some(reply) = state.replies.get(&key) {
            return Ok((reply.clone(), self.reply_files(&state, &key)?));
        }
        let value = match output {
            Ok(value) => value,
            Err(message) => {
                let prefix = message.split(':').next().unwrap_or("not_admitted");
                let reason = match prefix {
                    "malformed"
                    | "unsupported_version"
                    | "unsupported_feature"
                    | "not_admitted"
                    | "unavailable"
                    | "content_unavailable"
                    | "identity_mismatch"
                    | "incompatible"
                    | "revoked"
                    | "stale"
                    | "cannot_enforce"
                    | "limit_exceeded"
                    | "idempotency_conflict"
                    | "conflict" => prefix,
                    _ => "not_admitted",
                };
                let input = self.retain(&mut state, event, &execute.input)?;
                let body = json!({"v":"openagents.automation-result.v1","requires":[],"input":input,"status":if matches!(reason,"conflict"|"stale"|"idempotency_conflict"){"conflict"}else{"refused"},"state":null,"reason":reason});
                self.declare(&mut state, &body, &opened.principal, now)?;
                body
            }
        };
        self.finish_invocation(&mut state, event, &opened, execute, value, true, now)
    }
    /// The exclusive state lock proves either an exact retained result or absence.
    /// Settling the CJ claim fences a late original handler before domain mutation.
    pub fn recover_controller(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("AUTO execute required".into());
        };
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = state
            .service
            .claims
            .get(&key)
            .ok_or("no retained AUTO invocation")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err("idempotency_conflict: recovery input differs".into());
        }
        if let Some(reply) = state.replies.get(&key) {
            return Ok((reply.clone(), self.reply_files(&state, &key)?));
        }
        let known = state.invocations.get(&event.id).cloned();
        let dispatched = known.is_some();
        let value = if let Some(value) = known {
            value
        } else {
            let input = self.retain(&mut state, event, &execute.input)?;
            let body = json!({"v":"openagents.automation-result.v1","requires":[],"input":input,"status":"refused","state":null,"reason":"unavailable"});
            self.declare(&mut state, &body, &opened.principal, now)?;
            body
        };
        self.finish_invocation(&mut state, event, &opened, execute, value, dispatched, now)
    }
}
