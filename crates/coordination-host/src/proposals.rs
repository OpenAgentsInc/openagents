//! Task identity and frozen input admission precede reservations or dispatch.
use crate::{
    Result,
    book::{Coordinator, Proposal, Request, Scope},
    operations::Limits,
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn propose(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now, Some("openagents.task-proposal.v1"))?;
        coordination::validate(&body).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.current_invocation(&state, event)?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let grant = self.check(&scope, &actor, "propose", now)?;
        let target = body["target"]["id"].as_str().ok_or("target ID")?;
        if !grant.all && !grant.targets.contains(target) {
            return Err("not_admitted: operation outside participant grant".into());
        }
        if scope.targets.get(target) != Some(&body["target"]) {
            return Err("not_admitted: exact registered operation required".into());
        }
        let task = body["task"].as_str().ok_or("task ID")?.to_string();
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!("{actor}:proposal:{task}");
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fp {
                return Err("idempotency_conflict: changed proposal".into());
            }
            return Ok(previous.result.clone());
        }
        if scope.proposals.contains_key(&task)
            || scope.proposals.len() >= 4096
            || scope.requests.len() >= 4096
        {
            return Err("conflict: task identity already retained or limit reached".into());
        }
        self.check_frame(&scope, &body)?;
        let operation = self.operation(&body["target"])?;
        self.effects_subset(&operation["effects"], &body["effects"])?;
        self.effects_subset(&body["effects"], &scope.policy.effects)?;
        let limits = Limits::parse(&body["bounds"], &scope.policy)?;
        let mut depth = 0;
        let mut parent = body["parent"].as_str();
        while let Some(id) = parent {
            depth += 1;
            if depth > limits.depth {
                return Err("limit_exceeded: parent task depth".into());
            }
            let p = scope
                .proposals
                .get(id)
                .ok_or("not_admitted: absent parent task")?;
            if p.body["effects"]["delegates"] != true {
                return Err("not_admitted: parent operation has no child authority".into());
            }
            self.effects_subset(&body["effects"], &p.body["effects"])?;
            let parent_limits = Limits::parse(&p.body["bounds"], &scope.policy)?;
            if limits.calls > parent_limits.calls
                || limits.wall_ms > parent_limits.wall_ms
                || limits.input_bytes > parent_limits.input_bytes
                || limits.output_bytes > parent_limits.output_bytes
                || limits.concurrency > parent_limits.concurrency
                || limits.depth > parent_limits.depth
            {
                return Err("not_admitted: child bounds exceed parent ceiling".into());
            }
            parent = p.body["parent"].as_str();
        }
        for dependency in body["dependencies"].as_array().ok_or("dependencies")? {
            if !scope
                .proposals
                .contains_key(dependency.as_str().ok_or("dependency ID")?)
            {
                return Err("not_admitted: absent dependency owner".into());
            }
        }
        self.attachments(files)?;
        let reference = self.store.put_json(&body)?;
        let mut supplied = files.clone();
        supplied.insert(
            reference["digest"]
                .as_str()
                .ok_or("proposal digest")?
                .into(),
            contracts::jcs(&body).map_err(|e| e.to_string())?,
        );
        let closure = self.closure(&scope, &actor, &[reference.clone()], &supplied)?;
        let input = self.store.json(&body["input"])?;
        if self.store.get(&body["input"])?.len() as u64 > limits.input_bytes {
            return Err("limit_exceeded: task input".into());
        }
        self.schema_input(&operation["input"], &input)?;
        let lock =
            contracts::parse_lock(&self.store.json(&body["lock"])?).map_err(|e| e.to_string())?;
        if self.store.json(&body["lock"])?["root"] != body["target"] || lock.entries.len() != 1 {
            return Err(
                "unsupported_feature: native operation needs its exact single-component lock"
                    .into(),
            );
        }
        let context = contracts::parse_context(&self.store.json(&body["context"])?)
            .map_err(|e| e.to_string())?;
        if self.store.json(&body["context"])?["task"] != self.store.json(&body["frame"])?["task"]
            || self.store.json(&body["context"])?["coverage"] != "complete"
            || !context.entries.is_empty()
            || !context.omissions.is_empty()
        {
            return Err(
                "unsupported_feature: native operation requires a complete explicit-input context"
                    .into(),
            );
        }
        if self.store.json(&body["acceptance"])?
            != json!({"v":"openagents.coordination-native-check-policy.v1","requires":[],"check":"exact-admitted-output"})
        {
            return Err("unsupported_feature: independent native output check required".into());
        }
        let effects = contracts::parse_effects(&body["effects"]).map_err(|e| e.to_string())?;
        if body["reuse"] == "exact"
            && (!effects.writes.is_empty()
                || effects.process
                || effects.delegates
                || effects.spend
                || !effects.network.is_empty())
        {
            return Err("not_admitted: effectful work cannot request automatic reuse".into());
        }
        let scope_name = scope.descriptor["scope"].as_str().ok_or("scope ID")?;
        let reuse_key = coordination::reuse_key(scope_name, &body).map_err(|e| e.to_string())?;
        let reuse = if body["reuse"] == "exact" {
            scope
                .proposals
                .iter()
                .find(|(_, p)| {
                    p.author == actor
                        && p.status == "completed"
                        && p.body["reuse"] == "exact"
                        && self.reusable(&scope, p, now)
                        && coordination::reuse_key(scope_name, &p.body).ok().as_ref()
                            == Some(&reuse_key)
                })
                .map(|(id, _)| id.clone())
        } else {
            None
        };
        self.retain(&mut state, event, &body)?;
        scope.proposals.insert(
            task.clone(),
            Proposal {
                reference: reference.clone(),
                body,
                author: actor.clone(),
                fingerprint: fp.clone(),
                closure,
                status: if reuse.is_some() {
                    "reused"
                } else {
                    "proposed"
                }
                .into(),
                claim: None,
                executions: vec![],
                findings: vec![],
                reuse: reuse.clone(),
            },
        );
        scope.revision += 1;
        let record=self.journal(&mut scope,"observed",Some(&task),json!({"evidence":[reference],"sources":[],"complete":true,"provenance":{"operation":"task_proposal","reuse":reuse}}),now)?;
        let result = self.result(
            &mut state,
            &scope,
            &reference,
            &record,
            if reuse.is_some() {
                "duplicate"
            } else {
                "accepted"
            },
            reference.clone(),
            None,
            &actor,
            now,
        )?;
        scope.requests.insert(
            key,
            Request {
                fingerprint: fp,
                result: result.clone(),
            },
        );
        state.scopes.insert(id, scope);
        self.save(&state)?;
        Ok(result)
    }
    pub(crate) fn check_frame(&self, scope: &Scope, proposal: &Value) -> Result<()> {
        let frame = self.store.json(&proposal["frame"])?;
        let current = scope
            .frames
            .get(frame["task"].as_str().ok_or("frame task")?)
            .ok_or("not_admitted: frame was not independently admitted")?;
        if current["reference"] != proposal["frame"]
            || !frame["acceptance"]
                .as_array()
                .is_some_and(|a| a.contains(&proposal["acceptance"]))
        {
            return Err("stale: task frame or acceptance changed".into());
        }
        Ok(())
    }
    fn reusable(&self, scope: &Scope, p: &Proposal, now: u64) -> bool {
        let Some(lease) = scope
            .claims
            .values()
            .find(|l| l.body["proposal"] == p.reference)
        else {
            return false;
        };
        if lease.unknown
            || self.check_frame(scope, &p.body).is_err()
            || self
                .resources_current(scope, &lease.body["resources"], now)
                .is_err()
        {
            return false;
        }
        let Some(result) = lease
            .result
            .as_ref()
            .filter(|r| r["outcome"] == "completed")
        else {
            return false;
        };
        let Some(report) = lease.check.as_ref().and_then(|r| self.store.json(r).ok()) else {
            return false;
        };
        report["passed"] == true
            && report["input"] == p.body["input"]
            && report["output"] == result["output"]
            && report["checker"].as_str().is_some_and(|key| {
                scope.policy.checkers.contains(key)
                    && self.check(scope, key, "finding", now).is_ok()
            })
            && self.store.get(&result["output"]).is_ok()
    }
}
