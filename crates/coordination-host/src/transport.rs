//! One durable CJ service owns controller invocation identities across scopes.
use crate::{Result, book::Coordinator};
use nostr::{
    contracts,
    domain::{Event, RelaySigner, Tag},
    execution,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub(crate) fn controller_binding(&self, scope: &str, policy: &Value) -> Result<Value> {
        let schema = self.store.put(
            nostr::coordination::SCHEMA_BYTES,
            "application/schema+json",
            None,
        )?;
        let id = format!("{}:coord/{scope}", self.authority());
        let definition = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Admit and coordinate tasks within this owner-selected scope.","input":schema,"output":schema,"effects":{"reads":["coordination"],"writes":["coordination"],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"idempotent","evidence":[]},"binding_contract":{"operation":"openagents.coordination-controller.v1","interface":"openagents.coordination-controller.v1"}});
        let artifact = self.store.put(
            &contracts::jcs(&definition).map_err(|e| e.to_string())?,
            "application/json",
            Some("openagents.cap.v1"),
        )?;
        let target = json!({"id":id,"artifact":artifact});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":scope,"recipient":self.authority(),"policy":policy,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.coordination-controller-requirements.v1","requires":[],"scope":scope,"policy":policy,"operation":"coordination-only"}))?;
        Ok(json!({"target":target,"lock":lock,"context":context,"requirements":requirements}))
    }
    pub(crate) fn authenticated_cj(
        &self,
        event: &Event,
        now: u64,
        schema: Option<&str>,
    ) -> Result<(String, Value)> {
        let opened = execution::open_request(
            event,
            self.authority(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("unsupported controller control".into());
        };
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let admitted = state
            .service
            .claims
            .get(&key)
            .ok_or("not_admitted: use the durable controller CJ service")?;
        if admitted.execute_event != event.id
            || admitted.fingerprint != execute.fingerprint
            || !admitted.dispatch_intent
        {
            return Err("not_admitted: exact durably accepted CJ input required".into());
        }
        self.check_controller_request(&state, &opened, execute, now)?;
        if schema.is_some_and(|s| execute.input["v"] != s) {
            return Err("unsupported controller input schema".into());
        }
        Ok((opened.principal, execute.input.clone()))
    }
    fn check_controller_request(
        &self,
        state: &crate::book::State,
        opened: &execution::Opened,
        execute: &execution::Execute,
        now: u64,
    ) -> Result<()> {
        let scope = state
            .scopes
            .values()
            .find(|s| s.transport["target"] == execute.payload["target"])
            .ok_or("not_admitted: unregistered controller operation")?;
        if execute.input_artifact.is_some()
            || execute.payload["lock"] != scope.transport["lock"]
            || execute.payload["context"] != scope.transport["context"]
            || execute.payload["requirements"] != scope.transport["requirements"]
            || execute.deadline > scope.policy.until
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute.bounds.wall_ms.is_none_or(|v| v == 0 || v > 30_000)
            || execute
                .bounds
                .output_bytes
                .is_none_or(|v| v == 0 || v > 1_048_576)
        {
            return Err("cannot_enforce: controller pins or bounds differ".into());
        }
        let body = &execute.input;
        let belongs = match body["v"].as_str() {
            Some("openagents.task-proposal.v1" | "openagents.coordination-control.v1") => {
                body["coordinator"] == scope.reference
            }
            Some("openagents.claim-request.v1" | "openagents.finding.v1") => scope
                .proposals
                .values()
                .any(|p| p.reference == body["proposal"]),
            _ => false,
        };
        if !belongs {
            return Err(
                "not_admitted: controller input names another scope or unsupported operation"
                    .into(),
            );
        }
        self.check(
            scope,
            &opened.principal,
            if body["action"] == "status" {
                "read"
            } else {
                match body["v"].as_str() {
                    Some("openagents.task-proposal.v1") => "propose",
                    Some("openagents.claim-request.v1") => "claim",
                    Some("openagents.finding.v1") => "finding",
                    _ => "control",
                }
            },
            now,
        )?;
        let mut pins = BTreeMap::new();
        for field in ["target", "lock", "context", "requirements"] {
            let reference = if field == "target" {
                &execute.payload[field]["artifact"]
            } else {
                &execute.payload[field]
            };
            let bytes = self.store.get(reference)?;
            pins.insert(contracts::digest_bytes(&bytes), bytes);
        }
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))
    }
    /// Handle an authenticated remote operation, retaining one exact CJ result.
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
            return Err("unsupported controller control".into());
        };
        let key;
        {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            key = state
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            if let Some(previous) = state.replies.get(&key) {
                let claim = &state.service.claims[&key];
                if claim.fingerprint != execute.fingerprint
                    || claim.execute_event != event.id
                    || now > claim.retain_until
                {
                    return Err(
                        "idempotency_conflict: changed or expired controller request".into(),
                    );
                }
                let mut files = BTreeMap::new();
                for reference in state
                    .reply_artifacts
                    .get(&key)
                    .ok_or("retained CJ response artifacts")?
                {
                    let bytes = self.store.get(reference)?;
                    files.insert(contracts::digest_bytes(&bytes), bytes);
                }
                return Ok((previous.clone(), files));
            }
            if state.service.claims.contains_key(&key) {
                return Err(
                    "unknown: interrupted controller invocation requires reconciliation; no replay"
                        .into(),
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
            self.check_controller_request(&state, &opened, execute, now)?;
            if state.service.claims.len() >= 4096
                || state.service.claims.values().any(|c| c.run == execute.run)
                || state
                    .scopes
                    .values()
                    .any(|s| s.descriptor["run"] == execute.run)
            {
                return Err("conflict: retained RUN identity or service limit".into());
            }
            let (claim, root) = match state
                .service
                .prepare(&opened, now, &workspace_host::artifact::token())
                .map_err(|e| format!("{e:?}"))?
            {
                execution::Admission::Reserved { claim, root } => (claim, root),
                _ => return Err("unexpected controller retransmission".into()),
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
            Some("openagents.task-proposal.v1") => self.propose(event, files, now),
            Some("openagents.claim-request.v1") => self.claim(event, now),
            Some("openagents.coordination-control.v1") => self.control(event, now),
            Some("openagents.finding.v1") => self.finding(event, files, now),
            _ => Err("unsupported_feature: controller operation".into()),
        };
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        if state.replies.contains_key(&key) {
            drop(_guard);
            return self.serve(event, files, now);
        }
        let value = match output {
            Ok(value) => value,
            Err(message) => {
                let id = state
                    .scopes
                    .iter()
                    .find(|(_, s)| s.transport["target"] == execute.payload["target"])
                    .map(|(id, _)| id.clone())
                    .ok_or("controller scope")?;
                let mut scope = state.scopes[&id].clone();
                let prefix = message.split(':').next().unwrap_or("not_admitted");
                let reason = if matches!(
                    prefix,
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
                        | "conflict"
                ) {
                    prefix
                } else {
                    "not_admitted"
                };
                let input = self.retain(&mut state, event, &execute.input)?;
                let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":true,"provenance":{"operation":"controller_refusal","reason":reason}}),now)?;
                let value = self.result(
                    &mut state,
                    &scope,
                    &input,
                    &record,
                    if matches!(reason, "conflict" | "stale" | "idempotency_conflict") {
                        "conflict"
                    } else {
                        "refused"
                    },
                    Value::Null,
                    Some(reason),
                    &opened.principal,
                    now,
                )?;
                state.scopes.insert(id, scope);
                value
            }
        };
        let reference = self.store.put_json(&value)?;
        let artifacts = state.reply_artifacts.get_mut(&key).ok_or("CJ root")?;
        artifacts.push(reference.clone());
        for field in ["value", "record"] {
            if contracts::parse_artifact(&value[field]).is_ok()
                && !artifacts.contains(&value[field])
            {
                artifacts.push(value[field].clone());
            }
        }
        let output = Some(reference);
        let outcome = "completed";
        let result = state
            .service
            .observe(
                &key,
                execution::Report {
                    outcome,
                    dispatched: true,
                    output,
                    artifacts: state.reply_artifacts[&key].clone(),
                    receipts: vec![],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let response = self.seal(
            &opened.principal,
            execution::RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), event.id.clone()]),
                Tag::new(vec!["p".into(), opened.principal.clone()]),
            ],
            &result,
            now,
        )?;
        state.replies.insert(key.clone(), response.clone());
        self.save(&state)?;
        let mut closure = BTreeMap::new();
        for reference in &state.reply_artifacts[&key] {
            let bytes = self.store.get(reference)?;
            closure.insert(contracts::digest_bytes(&bytes), bytes);
        }
        Ok((response, closure))
    }
}
/// A client must receive this exact transport bundle from its admitted owner.
pub fn invoke(
    secret: &secp256k1::SecretKey,
    controller: &str,
    transport: &Value,
    input: Value,
    request: String,
    now: u64,
) -> Result<Event> {
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| "client key")?;
    let peer = controller
        .parse::<secp256k1::XOnlyPublicKey>()
        .map_err(|_| "controller key")?;
    let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":workspace_host::artifact::token(),"target":transport["target"],"lock":transport["lock"],"input":input,"context":transport["context"],"requirements":transport["requirements"],"bounds":{"wall_ms":30000,"output_bytes":1048576,"jobs":1,"spend_microunits":0},"deadline":now+30,"retain_until":now+3600});
    execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(secret, &peer),
        nonce: secp256k1::rand::random(),
        created_at: now,
    }
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), controller.into()]),
            Tag::new(vec!["expiration".into(), (now + 30).to_string()]),
        ],
        &payload,
    )
    .map_err(|e| format!("{e:?}"))
}
impl Coordinator {
    pub(crate) fn current_invocation(
        &self,
        state: &crate::book::State,
        event: &Event,
    ) -> Result<()> {
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
            return Err("controller execute required".into());
        };
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = state
            .service
            .claims
            .get(&key)
            .ok_or("controller acceptance required")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || !claim.dispatch_intent
            || claim.result.is_some()
        {
            return Err("revoked: controller invocation was reconciled or replaced".into());
        }
        Ok(())
    }
    /// Local-owner reconciliation never dispatches a controller operation again.
    /// Acquiring this store's exclusive lock fences its sole mutation path.
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
            return Err("controller execute required".into());
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
            .ok_or("no retained controller admission")?
            .clone();
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err("idempotency_conflict: recovery differs from admitted request".into());
        }
        if state.replies.contains_key(&key) {
            drop(_guard);
            return self.serve(event, &BTreeMap::new(), now);
        }
        let id = state
            .scopes
            .iter()
            .find(|(_, s)| s.transport["target"] == execute.payload["target"])
            .map(|(id, _)| id.clone())
            .ok_or("admitted controller scope")?;
        let mut scope = state.scopes[&id].clone();
        let fingerprint = contracts::digest_value(&execute.input).map_err(|e| e.to_string())?;
        let domain_key = match execute.input["v"].as_str() {
            Some("openagents.task-proposal.v1") => format!(
                "{}:proposal:{}",
                opened.principal,
                execute.input["task"].as_str().ok_or("task ID")?
            ),
            Some("openagents.claim-request.v1") => {
                format!("{}:claim:{fingerprint}", opened.principal)
            }
            Some("openagents.coordination-control.v1") => {
                format!("{}:control:{fingerprint}", opened.principal)
            }
            Some("openagents.finding.v1") => format!("{}:finding:{fingerprint}", opened.principal),
            _ => return Err("unsupported controller recovery".into()),
        };
        let known = scope
            .requests
            .get(&domain_key)
            .filter(|r| r.fingerprint == fingerprint)
            .map(|r| r.result.clone());
        let dispatched = known.is_some();
        let value = if let Some(value) = known {
            value
        } else {
            let input = self.retain(&mut state, event, &execute.input)?;
            let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":true,"provenance":{"operation":"fenced-controller-absence","request":execute.request}}),now)?;
            self.result(
                &mut state,
                &scope,
                &input,
                &record,
                "refused",
                Value::Null,
                Some("unavailable"),
                &opened.principal,
                now,
            )?
        };
        let reference = self.store.put_json(&value)?;
        let artifacts = state
            .reply_artifacts
            .get_mut(&key)
            .ok_or("controller RUN root")?;
        artifacts.push(reference.clone());
        for field in ["value", "record"] {
            if contracts::parse_artifact(&value[field]).is_ok()
                && !artifacts.contains(&value[field])
            {
                artifacts.push(value[field].clone());
            }
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
                    outcome: "completed",
                    dispatched,
                    output: Some(reference),
                    artifacts: state.reply_artifacts[&key].clone(),
                    receipts: vec![],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let reply = self.seal(
            &opened.principal,
            execution::RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), event.id.clone()]),
                Tag::new(vec!["p".into(), opened.principal.clone()]),
            ],
            &result,
            now,
        )?;
        state.replies.insert(key.clone(), reply.clone());
        self.save_scope(&mut state, id, scope)?;
        let mut files = BTreeMap::new();
        for r in &state.reply_artifacts[&key] {
            let bytes = self.store.get(r)?;
            files.insert(contracts::digest_bytes(&bytes), bytes);
        }
        Ok((reply, files))
    }
}
