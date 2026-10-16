//! Actual CJ work is fenced at intake, before input disclosure or effects.
use crate::{Result, book::Coordinator, operations::Limits};
use nostr::{
    contracts,
    domain::{Event, Tag},
    execution,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn prepare(&self, event: &Event, now: u64) -> Result<Event> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-dispatch.v1"),
        )?;
        crate::admin::closed(&body, &["v", "requires", "coordinator", "claim"])?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "claim", now)?;
        let lease_key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: current claim unavailable")?;
        let mut lease = scope.claims[&lease_key].clone();
        if actor != scope.owner && actor != lease.actor {
            return Err("not_admitted: only named worker or owner can dispatch".into());
        }
        if let Some(event) = lease.execution {
            return Ok(event);
        }
        self.current_lease(&scope, &lease, now)?;
        self.resources_current(&scope, &lease.body["resources"], now)?;
        let proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == lease.body["proposal"])
            .ok_or("proposal")?;
        let operation = self.operation(&proposal.body["target"])?;
        if operation["binding_contract"]["operation"] == "openagents.coord-native-container.v1" {
            return Err("container claims reserve children; they do not dispatch code".into());
        }
        let limits = Limits::parse(&proposal.body["bounds"], &scope.policy)?;
        if self.store.json(&proposal.body["context"])?["recipient"] != lease.actor {
            return Err("not_admitted: context recipient differs from worker".into());
        }
        let requirements=self.store.put_json(&json!({"v":"openagents.coordination-native-requirements.v1","requires":[],"coordinator":scope.reference,"proposal":proposal.reference,"claim":lease.reference,"effects":proposal.body["effects"],"bounds":proposal.body["bounds"]}))?;
        self.current_lease(
            &scope,
            &lease,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let request = workspace_host::artifact::token();
        let deadline = lease.body["expires_at"].as_u64().ok_or("claim expiry")?;
        let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":request,"target":proposal.body["target"],"lock":proposal.body["lock"],"input":{"artifact":proposal.body["input"]},"context":proposal.body["context"],"requirements":requirements,"bounds":{"wall_ms":limits.wall_ms,"output_bytes":limits.output_bytes,"jobs":1,"spend_microunits":0},"deadline":deadline,"retain_until":deadline+3600});
        let event = self.seal(
            &lease.actor,
            execution::REQUEST_KIND,
            vec![
                Tag::new(vec!["p".into(), lease.actor.clone()]),
                Tag::new(vec!["expiration".into(), deadline.to_string()]),
            ],
            &payload,
            now,
        )?;
        lease.input = Some(payload);
        lease.execution = Some(event.clone());
        scope.claims.insert(lease_key, lease);
        self.save_scope(&mut state, id, scope)?;
        Ok(event)
    }
    pub(crate) fn save_scope(
        &self,
        state: &mut crate::book::State,
        id: String,
        scope: crate::book::Scope,
    ) -> Result<()> {
        state.scopes.insert(id, scope);
        self.save(state)
    }
    pub(crate) fn seal(
        &self,
        recipient: &str,
        kind: u16,
        tags: Vec<Tag>,
        body: &Value,
        now: u64,
    ) -> Result<Event> {
        let peer = recipient
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "recipient key")?;
        execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(kind, tags, body)
        .map_err(|e| format!("{e:?}"))
    }
    pub(crate) fn intake(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(Value, BTreeMap<String, Vec<u8>>)> {
        let started = std::time::Instant::now();
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.coordination-native-intake.v1"))?;
        crate::admin::closed(&body, &["v", "requires", "request", "fingerprint"])?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = state
            .scopes
            .iter()
            .find(|(_, s)| {
                s.claims.values().any(|l| {
                    l.execution
                        .as_ref()
                        .is_some_and(|e| body["request"] == e.id)
                })
            })
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: worker request unavailable")?;
        let mut scope = state.scopes[&id].clone();
        let lease_key = scope
            .claims
            .iter()
            .find(|(_, l)| {
                l.execution
                    .as_ref()
                    .is_some_and(|e| body["request"] == e.id)
            })
            .map(|(id, _)| id.clone())
            .ok_or("claim")?;
        let mut lease = scope.claims[&lease_key].clone();
        self.current_lease(&scope, &lease, now)?;
        if actor != lease.actor || lease.dispatched {
            return Err("unknown: only an unstarted named worker intake can dispatch".into());
        }
        let payload = lease.input.clone().ok_or("execution payload")?;
        if body["fingerprint"] != contracts::digest_value(&payload).map_err(|e| e.to_string())? {
            return Err("idempotency_conflict: intake differs from exact CJ request".into());
        }
        self.resources_current(&scope, &lease.body["resources"], now)?;
        if lease
            .own_calls
            .checked_add(crate::claims::allocated(&scope, Some(&lease_key)))
            .is_none_or(|v| v >= lease.reserved_calls)
        {
            return Err("limit_exceeded: task allowance is spent or held by children".into());
        }
        let mut pins = BTreeMap::new();

        for reference in [
            &payload["target"]["artifact"],
            &payload["lock"],
            &payload["input"]["artifact"],
            &payload["context"],
            &payload["requirements"],
        ] {
            let bytes = self.store.get(reference)?;
            pins.insert(contracts::digest_bytes(&bytes), bytes);
        }
        self.current_lease(
            &scope,
            &lease,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        lease.own_calls += 1;
        lease.spent_calls += 1;
        lease.dispatched = true;
        lease.unknown = true;
        lease.phase = "running".into();
        let mut parent = lease.parent.clone();
        while let Some(key) = parent {
            let ancestor = scope.claims.get_mut(&key).ok_or("ancestor reservation")?;
            ancestor.spent_calls += 1;
            parent = ancestor.parent.clone();
        }
        scope.calls += 1;
        scope.revision += 1;
        scope.claims.insert(lease_key, lease);
        let input = self.retain(&mut state, event, &body)?;
        self.journal(&mut scope,"dispatched",Some(payload["request"].as_str().ok_or("request ID")?),json!({"binding":payload["target"],"attempt":1,"input":payload["input"],"context":payload["context"],"effect":"native-input-disclosure","generation":0}),now)?;
        let _ = input;
        self.save_scope(&mut state, id, scope)?;
        Ok((payload, pins))
    }
    pub fn record_result(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = state
            .scopes
            .iter()
            .find(|(_, s)| {
                s.claims.values().any(|l| {
                    l.execution.as_ref().is_some_and(|e| {
                        event
                            .tags
                            .iter()
                            .any(|t| t.0 == vec!["e".to_string(), e.id.clone()])
                    })
                })
            })
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: executed request unavailable")?;
        let mut scope = state.scopes[&id].clone();
        let key = scope
            .claims
            .iter()
            .find(|(_, l)| {
                l.execution.as_ref().is_some_and(|e| {
                    event
                        .tags
                        .iter()
                        .any(|t| t.0 == vec!["e".to_string(), e.id.clone()])
                })
            })
            .map(|(id, _)| id.clone())
            .ok_or("claim")?;
        let mut lease = scope.claims[&key].clone();
        let request = lease.execution.as_ref().ok_or("execution")?;
        let execute = lease.input.as_ref().ok_or("payload")?;
        let result = execution::bind_worker_event(
            event,
            &execution::Pending {
                worker: &lease.actor,
                customer: self.authority(),
                request: execute["request"].as_str().ok_or("request")?,
                attempt: 1,
                execute_event: &request.id,
            },
            &self.secret,
        )
        .map_err(|e| format!("{e:?}"))?;
        if event.kind != execution::RESULT_KIND {
            return Err("terminal CJ result required".into());
        }
        if result["run"] != execute["run"]
            || now > execute["retain_until"].as_u64().ok_or("retention")?
        {
            return Err("not_admitted: result run or retention differs".into());
        }
        if let Some(previous) = &lease.result {
            if previous != &result {
                return Err("conflict: worker result fork".into());
            }
            return Ok(previous.clone());
        }
        if !lease.dispatched
            || !matches!(
                result["outcome"].as_str(),
                Some("completed" | "failed" | "cancelled")
            )
            || result["spend"] != 0
        {
            return Err("unknown: unsupported or unaccounted worker outcome".into());
        }
        self.attachments(files)?;
        let mut root_found = false;
        for reference in result["artifacts"].as_array().ok_or("result artifacts")? {
            if reference["schema"] != nostr::run::RECORD_SCHEMA {
                continue;
            }
            let bytes = files
                .get(reference["digest"].as_str().ok_or("root digest")?)
                .ok_or("worker must supply its RUN root")?;
            contracts::check_artifact_bytes(
                &contracts::parse_artifact(reference).map_err(|e| e.to_string())?,
                bytes,
            )
            .map_err(|e| e.to_string())?;
            let body = contracts::parse_strict(bytes).map_err(|e| e.to_string())?;
            let record = nostr::run::parse_envelope(&body).map_err(|e| e.to_string())?;
            if root_found
                || record.seq != 0
                || record.controller != lease.actor
                || body["record"]["run"] != execute["run"]
                || body["record"]["data"]["request"] != execute["request"]
                || body["record"]["data"]["program"] != execute["target"]["artifact"]["digest"]
                || body["record"]["data"]["lock"] != execute["lock"]["digest"]
                || body["record"]["data"]["context"] != execute["context"]["digest"]
                || body["record"]["data"]["policy"] != execute["requirements"]["digest"]
                || result["record"]["digest"] != reference["digest"]
            {
                return Err(
                    "identity_mismatch: worker RUN root differs from admitted attempt".into(),
                );
            }
            root_found = true;
        }
        if !root_found {
            return Err("not_admitted: authenticated worker RUN root required".into());
        }
        if result["outcome"] == "completed" {
            let reference =
                contracts::parse_artifact(&result["output"]).map_err(|e| e.to_string())?;
            let bytes = files
                .get(&reference.digest)
                .ok_or("worker must supply exact output bytes")?;
            contracts::check_artifact_bytes(&reference, bytes).map_err(|e| e.to_string())?;
            let p = scope
                .proposals
                .values()
                .find(|p| p.reference == lease.body["proposal"])
                .ok_or("proposal")?;
            if bytes.len() as u64 > Limits::parse(&p.body["bounds"], &scope.policy)?.output_bytes {
                return Err("limit_exceeded: worker output".into());
            }
            self.schema_input(
                &self.operation(&p.body["target"])?["output"],
                &contracts::parse_strict(bytes).map_err(|e| e.to_string())?,
            )?;
        }
        lease.result = Some(result.clone());
        lease.unknown = false;
        lease.phase = "completed".into();
        scope.revision += 1;
        let task = scope
            .proposals
            .iter()
            .find(|(_, p)| p.reference == lease.body["proposal"])
            .map(|(id, _)| id.clone())
            .ok_or("task")?;
        if scope.proposals[&task].status != "cancelled" {
            scope.proposals.get_mut(&task).ok_or("task")?.status =
                if result["outcome"] == "completed" {
                    "completed"
                } else {
                    "failed"
                }
                .into();
        }
        let reference = self.retain(&mut state, event, &result)?;
        for entry in scope.backgrounds.values_mut() {
            if entry["active"] == key {
                entry["outputs"].as_array_mut().ok_or("background output history")?.push(json!({"proposal":lease.body["proposal"],"result":reference,"output":result["output"]}));
            }
        }
        self.journal(&mut scope,"resolved",Some(execute["request"].as_str().ok_or("request")?),json!({"outcome":result["outcome"],"dispatched":true,"output":result["output"],"receipts":[reference],"usage":{"calls":1,"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now)?;
        scope.claims.insert(key, lease);
        self.save_scope(&mut state, id, scope)?;
        Ok(result)
    }
}
