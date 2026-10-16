//! Cleanup fences exact identities and uses original outcomes, never replay.
use crate::{
    Result,
    host::{Automation, Record, State},
};
use nostr::{automation, contracts, domain::Event};
use serde_json::{Value, json};
impl Automation {
    pub(crate) fn propagate_terminal(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<()> {
        if matches!(
            record.state["status"].as_str(),
            Some("cancelled" | "expired" | "exhausted")
        ) {
            let id = record.body["automation"]
                .as_str()
                .ok_or("plan id")?
                .to_string();
            state.plans.insert(id, record.clone());
            self.save(state)?;
            if let Err(error) = self.cancel_children(state, record, now) {
                if let Some(active) = &record.active {
                    record.occurrences.get_mut(active).ok_or("occurrence")?["cleanup_error"] =
                        json!(error);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn block_unknown(&self, reference: &Value, now: u64, error: &str) -> Result<()> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        if record.state["status"] == "active"
            && record
                .pending
                .as_ref()
                .is_some_and(|p| p["phase"] == "evaluate")
        {
            let detail=self.store.put_json(&json!({"v":"openagents.automation-native-unknown.v1","requires":[],"plan":record.reference,"reason":error}))?;
            self.transition(&mut state, &mut record, "blocked", vec![detail], now)?;
            state.plans.insert(id, record);
            self.save(&state)?;
        }
        Ok(())
    }
    pub(crate) fn occurrence_proposal(&self, record: &Record, occurrence: &Value) -> Result<Value> {
        let mut bounds = record.body["per_occurrence"].clone();
        for b in bounds.as_array_mut().ok_or("bounds")? {
            if b["bound"] == "calls" {
                b["ceiling"] = json!(2);
            }
        }
        Ok(
            json!({"v":"openagents.task-proposal.v1","requires":[],"task":occurrence["body"]["task"],"coordinator":record.body["coordinator"],"frame":record.body["frame"],"parent":record.body["task"],"target":record.body["target"],"lock":record.body["lock"],"input":occurrence["input"],"context":record.body["context"],"dependencies":[],"effects":record.body["effects"],"bounds":bounds,"acceptance":record.body["acceptance"],"reuse":"never"}),
        )
    }
    pub(crate) fn cancel_children(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<Option<Value>> {
        let active = record.active.clone().or_else(|| {
            if matches!(
                record.state["status"].as_str(),
                Some("cancelled" | "expired" | "exhausted")
            ) {
                record
                    .occurrences
                    .iter()
                    .max_by_key(|(_, o)| o["body"]["ordinal"].as_u64())
                    .map(|(id, _)| id.clone())
            } else {
                None
            }
        });
        let Some(active) = active else {
            return Ok(None);
        };
        let occurrence = record.occurrences[&active].clone();
        let terminal = matches!(
            record.state["status"].as_str(),
            Some("cancelled" | "expired" | "exhausted")
        );
        let proposal = self.occurrence_proposal(record, &occurrence)?;
        let event = self.coordinator.automation_cleanup(
            &record.body,
            &occurrence["body"],
            &proposal,
            terminal,
            now,
        )?;
        let opened =
            nostr::private_artifact::open(&event, &self.secret).map_err(|e| e.to_string())?;
        if opened.signer() != self.coordinator.authority() {
            return Err("cleanup authority differs".into());
        }
        let proof = contracts::parse_strict(opened.inline_bytes().ok_or("inline cleanup proof")?)
            .map_err(|e| e.to_string())?;
        if proof["occurrence"] != occurrence["reference"] || proof["plan"] != record.reference {
            return Err("cleanup identity differs".into());
        }
        let proof_ref = self.retain(state, &event, &proof)?;
        record.occurrences.get_mut(&active).ok_or("occurrence")?["cleanup"] = proof_ref;
        Ok(Some(proof))
    }
    /// Local owner recovery. It fences queued work and reads saved replies only.
    /// An absent receiver reply never proves that a dispatched operation did not run.
    pub fn reconcile(
        &self,
        reference: &Value,
        worker: Option<&coordination_host::worker::Worker>,
        now: u64,
    ) -> Result<Value> {
        // Do not take a worker lock while holding AUTO: a queued worker may be
        // waiting for AUTO's intake fence while it holds its own lock.
        let execute = {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            let id = Self::plan_key(&state, reference)?;
            let mut record = state.plans[&id].clone();
            if record.state["status"] == "active" {
                self.transition(&mut state, &mut record, "paused", vec![], now)?;
                state.plans.insert(id.clone(), record);
                self.save(&state)?;
            }
            state.plans[&id]
                .active
                .as_ref()
                .and_then(|k| state.plans[&id].occurrences.get(k))
                .and_then(|o| serde_json::from_value::<Event>(o["execute"].clone()).ok())
        };
        if let (Some(worker), Some(execute)) = (worker, execute) {
            if let Some((result, files)) = worker.retained_result(&execute, now)? {
                self.record_result(reference, &result, &files, now)?;
            }
        }
        let pending = {
            let _guard = self.store.lock()?;
            let state = self.state()?;
            state.plans[&Self::plan_key(&state, reference)?]
                .pending
                .clone()
        };
        if let Some(pending) = pending {
            if pending["phase"] == "evaluate" {
                let _ = self.evaluate_known(reference, now);
            }
            if pending["phase"] == "source" {
                if let Some(source) = &self.source {
                    let event: Event = serde_json::from_value(pending["event"].clone())
                        .map_err(|_| "source request")?;
                    if let Ok((response, files)) =
                        source.run(&self.coordinator, &event, now, &SavedOnly)
                    {
                        let _guard = self.store.lock()?;
                        let mut state = self.state()?;
                        let id = Self::plan_key(&state, reference)?;
                        let mut record = state.plans[&id].clone();
                        let observation =
                            self.accept_poll(&mut state, &mut record, &response, &files, now)?;
                        let status = record.state["status"].as_str().ok_or("status")?.to_string();
                        self.transition(&mut state, &mut record, &status, vec![observation], now)?;
                        state.plans.insert(id, record);
                        self.save(&state)?;
                    }
                }
            }
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        let status = record.state["status"].as_str().ok_or("status")?.to_string();
        let status = if !matches!(
            status.as_str(),
            "cancelled" | "complete" | "exhausted" | "expired"
        ) && now >= record.body["expires_at"].as_u64().ok_or("expiry")?
        {
            "expired"
        } else {
            status.as_str()
        };
        // Persist the scheduling fence before crossing the coordinator boundary.
        let next = if status == "active" { "paused" } else { status };
        self.transition(&mut state, &mut record, next, vec![], now)?;
        state.plans.insert(id.clone(), record.clone());
        self.save(&state)?;
        let Some(proof) = self.cancel_children(&mut state, &mut record, now)? else {
            return Ok(record.state);
        };
        let Some(active) = record.active.clone() else {
            state.plans.insert(id, record.clone());
            self.save(&state)?;
            return Ok(record.state);
        };
        let mut evidence = vec![record.occurrences[&active]["cleanup"].clone()];
        // Pending source/checker effects retain their original identities. Their
        // absence cannot be inferred from an unrelated worker's cleanup proof.
        if proof["known"] == true
            && record
                .pending
                .as_ref()
                .is_none_or(|p| !matches!(p["phase"].as_str(), Some("source" | "evaluate")))
        {
            if let Some(outcome) = record.outcomes.get(&active) {
                evidence.push(outcome["reference"].clone());
                record.active = None;
                record.occurrences.get_mut(&active).ok_or("occurrence")?["phase"] =
                    json!("settled");
                record.pending = None;
            } else if proof["dispatched"] == false
                || record.occurrences[&active]["result"].is_object()
            {
                let occurrence = record.occurrences[&active].clone();
                let terminal = if proof["dispatched"] == true {
                    occurrence["terminal"].clone()
                } else {
                    proof["record"].clone()
                };
                contracts::parse_artifact(&terminal).map_err(|e| e.to_string())?;
                if proof["dispatched"] == false {
                    let bytes = self.coordinator.get_local_bytes(&terminal)?;
                    self.store
                        .put(&bytes, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
                }
                let reserved = record.accounting.requests[&active].reserved.clone();
                record.accounting.settle(&active, Some(reserved))?;
                let usage=self.store.put_json(&json!({"v":"openagents.automation-native-settlement.v1","requires":[],"occurrence":occurrence["reference"],"ledger":record.accounting,"cleanup":evidence[0]}))?;
                let outcome = json!({"v":"openagents.automation-outcome.v1","requires":[],"occurrence":occurrence["reference"],"proposal":proof["proposal"],"claim":proof["claim"],"result":if proof["dispatched"]==true{occurrence["result"].clone()}else{Value::Null},"settled":terminal,"evidence":evidence,"usage":[usage]});
                automation::validate(&outcome).map_err(|e| e.to_string())?;
                let outcome_ref = self.declare(
                    &mut state,
                    &outcome,
                    record.body["owner"].as_str().ok_or("owner")?,
                    now,
                )?;
                // Retain the exact proposed bytes, even when admission was refused.
                self.store
                    .put_json(&self.occurrence_proposal(&record, &occurrence)?)?;
                record.occurrences.get_mut(&active).ok_or("occurrence")?["proposal"] =
                    proof["proposal"].clone();
                record.occurrences.get_mut(&active).ok_or("occurrence")?["claim"] =
                    proof["claim"].clone();
                record.occurrences.get_mut(&active).ok_or("occurrence")?["phase"] =
                    json!("settled");
                record.outcomes.insert(active.clone(),json!({"reference":outcome_ref,"ordinal":occurrence["body"]["ordinal"],"cancelled":true}));
                evidence = vec![outcome_ref];
                record.active = None;
                record.pending = None;
            }
        }
        self.transition(&mut state, &mut record, next, evidence, now)?;
        let result = record.state.clone();
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result)
    }
}

/// Saved receiver replies return before this gate; it never admits a new call.
pub(crate) struct SavedOnly;
impl crate::source::SourceGate for SavedOnly {
    fn with_fence(
        &self,
        _: &crate::source::SourceIntake,
        _: u64,
        _: &mut dyn FnMut() -> Result<crate::source::Reply>,
    ) -> Result<crate::source::Reply> {
        Err("unknown: recovery cannot start a source call".into())
    }
}
impl crate::evaluator::EvaluationGate for SavedOnly {
    fn with_fence(
        &self,
        _: &crate::evaluator::EvaluationIntake,
        _: u64,
        _: &mut dyn FnMut() -> Result<crate::evaluator::Reply>,
    ) -> Result<crate::evaluator::Reply> {
        Err("unknown: recovery cannot start a checker call".into())
    }
}
