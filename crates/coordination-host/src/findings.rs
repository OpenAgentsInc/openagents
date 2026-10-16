//! Original worker findings remain history; their declarations do not integrate.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn finding(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now, Some("openagents.finding.v1"))?;
        coordination::validate(&body).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.current_invocation(&state, event)?;
        let id = state
            .scopes
            .iter()
            .find(|(_, s)| {
                s.proposals
                    .values()
                    .any(|p| p.reference == body["proposal"])
            })
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: proposal unavailable")?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "finding", now)?;
        if body["producer"] != actor
            || body["verification"] != "not_run"
            || body["integration"] != "not_requested"
        {
            return Err(
                "not_admitted: original producer may submit unverified, unapplied findings".into(),
            );
        }
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!("{actor}:finding:{fp}");
        if let Some(previous) = scope.requests.get(&key) {
            return Ok(previous.result.clone());
        }
        let task = scope
            .proposals
            .iter()
            .find(|(_, p)| p.reference == body["proposal"])
            .map(|(id, _)| id.clone())
            .ok_or("task")?;
        let proposal = scope.proposals[&task].clone();
        let lease = scope
            .claims
            .values()
            .find(|l| l.body["proposal"] == proposal.reference && l.actor == actor)
            .ok_or("not_admitted: producer has no admitted attempt")?;
        let result = lease
            .result
            .as_ref()
            .filter(|r| r["outcome"] == "completed")
            .ok_or("not_admitted: original completed CJ result required")?;
        let result_ref = workspace_host::artifact::json_ref(result)?;
        let frame = self.store.json(&proposal.body["frame"])?;
        if body["content"] != result["output"]
            || body["frame"] != proposal.body["frame"]
            || body["context"] != proposal.body["context"]
            || body["snapshot"] != frame["snapshot"]
            || !body["receipts"]
                .as_array()
                .ok_or("finding receipts")?
                .contains(&result_ref)
        {
            return Err("identity_mismatch: finding differs from admitted worker result".into());
        }
        if proposal.findings.len() >= 64 || scope.requests.len() >= 4096 {
            return Err("limit_exceeded: finding retention".into());
        }
        self.attachments(files)?;
        self.closure(
            &scope,
            &actor,
            &body["evidence"]
                .as_array()
                .ok_or("finding evidence")?
                .clone(),
            files,
        )?;
        let stale = self.check_frame(&scope, &proposal.body).is_err()
            || self
                .resources_current(&scope, &lease.body["resources"], now)
                .is_err()
            || now >= body["valid_until"].as_u64().ok_or("finding expiry")?;
        let reference = self.retain(&mut state, event, &body)?;
        let verification = if !stale
            && lease
                .check
                .as_ref()
                .is_some_and(|r| self.store.json(r).ok().is_some_and(|r| r["passed"] == true))
        {
            "passed"
        } else {
            "not_run"
        };
        scope.proposals.get_mut(&task).ok_or("task")?.findings.push(json!({"finding":reference,"stale":stale,"verification":verification,"integration":"not_requested"}));
        scope.revision += 1;
        let record=self.journal(&mut scope,"observed",None,json!({"evidence":[reference],"sources":[],"complete":true,"provenance":{"operation":"original_worker_finding","producer":actor,"stale":stale}}),now)?;
        let answer = self.result(
            &mut state,
            &scope,
            &reference,
            &record,
            "accepted",
            reference.clone(),
            None,
            &actor,
            now,
        )?;
        scope.requests.insert(
            key,
            Request {
                fingerprint: fp,
                result: answer.clone(),
            },
        );
        self.save_scope(&mut state, id, scope)?;
        Ok(answer)
    }
}
