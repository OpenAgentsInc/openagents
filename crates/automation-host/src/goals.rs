//! A valid independent decision is required before another continuation.
use crate::{
    Result,
    evaluator::{EvaluationGate, EvaluationIntake, Reply},
    host::{Automation, Record, State},
};
use nostr::{
    automation, contracts,
    domain::{Event, Tag},
    execution,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
impl Automation {
    pub(crate) fn prepare_evaluation(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<Event> {
        let active = record
            .active
            .clone()
            .ok_or("active evaluation occurrence")?;
        let occurrence = record.occurrences[&active].clone();
        let outcome = record
            .outcomes
            .get(&active)
            .ok_or("settled occurrence outcome")?["reference"]
            .clone();
        if record.pending.is_some() {
            return Err("unknown: an earlier operation requires reconciliation".into());
        }
        let evaluator = self
            .evaluator
            .as_ref()
            .ok_or("independent checker not mounted")?;
        let bundle = evaluator.binding()?;
        let bounds = crate::plans::units(&record.body["completion"]["bounds"])?;
        let input = json!({"v":"openagents.automation-evaluation-input.v1","requires":[],"plan":record.reference,"occurrence":occurrence["reference"],"outcome":outcome,"frame":record.body["frame"],"acceptance":record.body["acceptance"]});
        automation::validate(&input).map_err(|e| e.to_string())?;
        let input_ref = self.store.put_json(&input)?;
        let (parent, _) = self.coordinator.automation_parent(
            &record.body,
            &state.policy.worker,
            &state.policy.checker,
            &BTreeMap::new(),
            now,
        )?;
        let request = workspace_host::artifact::token();
        let run = workspace_host::artifact::token();
        let admission = workspace_host::host::request(
            &self.secret,
            self.coordinator.authority(),
            json!({"v":"openagents.coordination-native-auxiliary.v1","requires":[],"coordinator":record.body["coordinator"],"claim":parent["claim"],"command":workspace_host::artifact::token(),"purpose":"check","request":request,"fingerprint":contracts::digest_value(&input).map_err(|e|e.to_string())?}),
            now,
        )?;
        let deadline = now
            .checked_add(bounds["wall_ms"].div_ceil(1000))
            .ok_or("evaluation deadline")?
            .min(record.body["expires_at"].as_u64().ok_or("expiry")?)
            .min(
                occurrence["body"]["deadline"]
                    .as_u64()
                    .ok_or("occurrence deadline")?,
            );
        if deadline <= now {
            return Err("stale: no evaluation time remains".into());
        }
        let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":run,"target":bundle["target"],"lock":bundle["lock"],"input":{"artifact":input_ref},"context":bundle["context"],"requirements":bundle["requirements"],"bounds":{"wall_ms":bounds["wall_ms"].min((deadline-now).saturating_mul(1000)),"output_bytes":1048576,"jobs":1,"spend_microunits":0},"deadline":deadline,"retain_until":record.body["retain_until"]});
        let peer = evaluator
            .authority()
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "checker key")?;
        let event = execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            execution::REQUEST_KIND,
            vec![
                Tag::new(vec!["p".into(), evaluator.authority().into()]),
                Tag::new(vec!["expiration".into(), deadline.to_string()]),
            ],
            &payload,
        )
        .map_err(|e| format!("{e:?}"))?;
        record.pending = Some(
            json!({"phase":"evaluate","event":event,"payload":payload,"input":input_ref,"admission":admission}),
        );
        Ok(event)
    }
    pub(crate) fn evaluation_files(
        &self,
        state: &State,
        record: &Record,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let pending = record.pending.as_ref().ok_or("pending evaluation")?;
        let mut files = BTreeMap::new();
        let bytes = self.store.get(&pending["input"])?;
        files.insert(contracts::digest_bytes(&bytes), bytes);
        let admission = contracts::jcs(&pending["admission"]).map_err(|e| e.to_string())?;
        files.insert(contracts::digest_bytes(&admission), admission);
        let active = record.active.as_ref().ok_or("active")?;
        let outcome = &record.outcomes[active]["reference"];
        let original = state
            .originals
            .get(outcome["digest"].as_str().ok_or("outcome")?)
            .and_then(|events| events.iter().find(|e| e.pubkey == self.authority()))
            .ok_or("original controller outcome declaration")?;
        let bytes = contracts::jcs(&serde_json::to_value(original).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        files.insert(contracts::digest_bytes(&bytes), bytes);
        Ok(files)
    }
    /// Execute only the already pinned independent checker; no target work runs here.
    pub fn evaluate(&self, reference: &Value, now: u64) -> Result<Value> {
        let result = self.evaluate_inner(reference, now, false);
        if let Err(error) = &result {
            self.block_unknown(reference, now, error)?;
        }
        result
    }
    pub(crate) fn evaluate_known(&self, reference: &Value, now: u64) -> Result<Value> {
        self.evaluate_inner(reference, now, true)
    }
    fn evaluate_inner(&self, reference: &Value, now: u64, known_only: bool) -> Result<Value> {
        let started = std::time::Instant::now();
        let (request, files) = {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            let id = Self::plan_key(&state, reference)?;
            let mut record = state.plans[&id].clone();
            let now = now.saturating_add(started.elapsed().as_secs());
            let Some(active) = record.active.as_ref() else {
                return record
                    .evaluations
                    .values()
                    .last()
                    .map(|e| e["reference"].clone())
                    .ok_or("no occurrence awaiting a check".into());
            };
            if (!known_only && record.state["status"] != "active")
                || record.occurrences[active]["phase"] != "evaluate"
            {
                return Err(
                    "not_admitted: active settled occurrence required for evaluation".into(),
                );
            }
            let request = if let Some(pending) = &record.pending {
                if pending["phase"] != "evaluate" {
                    return Err("unknown: another operation is pending".into());
                }
                serde_json::from_value(pending["event"].clone())
                    .map_err(|_| "evaluation request")?
            } else {
                if known_only {
                    return Err("unknown: no saved checker invocation".into());
                }
                let event = self.prepare_evaluation(&mut state, &mut record, now)?;
                self.transition(&mut state, &mut record, "active", vec![], now)?;
                state.plans.insert(id, record.clone());
                self.save(&state)?;
                event
            };
            (request, self.evaluation_files(&state, &record)?)
        };
        let reply = self
            .evaluator
            .as_ref()
            .ok_or("goal checker not mounted")?
            .run(
                &self.coordinator,
                &request,
                &files,
                now.saturating_add(started.elapsed().as_secs()),
                if known_only {
                    &crate::recovery::SavedOnly
                } else {
                    self
                },
            );
        let (response, files) = reply?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        let now = now.saturating_add(started.elapsed().as_secs());
        let pending = record.pending.as_ref().ok_or("pending evaluation")?.clone();
        let result = execution::bind_worker_event(
            &response,
            &execution::Pending {
                worker: &state.policy.checker,
                customer: self.authority(),
                request: pending["payload"]["request"].as_str().ok_or("request")?,
                attempt: 1,
                execute_event: request.id.as_str(),
            },
            &self.secret,
        )
        .map_err(|e| format!("{e:?}"))?;
        if response.kind != execution::RESULT_KIND
            || result["outcome"] != "completed"
            || result["run"] != pending["payload"]["run"]
            || result["spend"] != 0
            || files.len() > 256
            || files.values().map(Vec::len).sum::<usize>() > 1048576
        {
            return Err("unknown: complete bounded checker result required".into());
        }
        for (d, b) in &files {
            if contracts::digest_bytes(b) != *d {
                return Err("identity_mismatch: checker bytes differ".into());
            }
        }
        nostr::run::terminal_from_result(&result, &files, &state.policy.checker)
            .map_err(|e| e.to_string())?;
        let output_ref = &result["output"];
        let output = files
            .get(output_ref["digest"].as_str().ok_or("output")?)
            .ok_or("evaluation bytes")?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(output_ref).map_err(|e| e.to_string())?,
            output,
        )
        .map_err(|e| e.to_string())?;
        let evaluation = automation::parse(output).map_err(|e| e.to_string())?;
        if evaluation["v"] != "openagents.automation-evaluation.v1"
            || evaluation["input"] != pending["input"]
        {
            return Err("identity_mismatch: checker evaluated different input".into());
        }
        let original = files
            .values()
            .filter_map(|b| serde_json::from_slice::<Event>(b).ok())
            .find(|event| {
                nostr::private_artifact::open(event, &self.secret).is_ok_and(|opened| {
                    opened.signer() == state.policy.checker
                        && opened.inline_bytes() == Some(output.as_slice())
                })
            })
            .ok_or("not_admitted: original independent evaluation declaration required")?;
        let active = record.active.clone().ok_or("evaluated occurrence")?;
        let ordinal = record.occurrences[&active]["body"]["ordinal"]
            .as_u64()
            .ok_or("ordinal")?;
        let mut known = BTreeSet::new();
        for value in record.occurrences.values() {
            known.insert(
                value["reference"]["digest"]
                    .as_str()
                    .ok_or("occurrence")?
                    .to_string(),
            );
        }
        // Every counted claim must belong to this host's actual admitted occurrences.
        let scope = self.coordinator.scope(&record.body["coordinator"])?;
        for evidence in evaluation["evidence"]
            .as_array()
            .ok_or("evaluation evidence")?
        {
            if evidence["schema"] == "openagents.claim.v1" {
                let claim = scope["claims"]
                    .as_object()
                    .ok_or("claims")?
                    .values()
                    .find(|l| l["reference"] == *evidence)
                    .ok_or("actual evaluated claim")?;
                let proposal = scope["proposals"]
                    .as_object()
                    .ok_or("proposals")?
                    .values()
                    .find(|p| p["reference"] == claim["body"]["proposal"])
                    .ok_or("actual evaluated proposal")?;
                let wrapper = contracts::parse_strict(
                    &self
                        .coordinator
                        .get_local_bytes(&proposal["body"]["input"])?,
                )
                .map_err(|e| e.to_string())?;
                if wrapper["plan"] != record.reference
                    || !known.contains(
                        wrapper["occurrence"]["digest"]
                            .as_str()
                            .ok_or("counted occurrence")?,
                    )
                {
                    return Err(
                        "identity_mismatch: checker counted work outside admitted occurrences"
                            .into(),
                    );
                }
            }
        }
        for bytes in files.values() {
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let policy_valid = self.check_evaluation_policy(&record, &evaluation, &scope)?;
        let reference = self.retain(&mut state, &original, &evaluation)?;
        record.evaluations.insert(format!("{ordinal:020}"),json!({"reference":reference,"body":evaluation,"at":now,"occurrence":record.occurrences[&active]["reference"],"accepted":policy_valid}));
        record.pending = None;
        let current = record.state["status"].as_str().ok_or("status")?.to_string();
        let decision = evaluation["decision"].as_str().ok_or("decision")?;
        let fresh = self
            .coordinator
            .automation_parent(
                &record.body,
                &state.policy.worker,
                &state.policy.checker,
                &BTreeMap::new(),
                now,
            )
            .is_ok();
        let status = if current != "active" {
            current.as_str()
        } else if !fresh || !policy_valid || matches!(decision, "unknown" | "blocked") {
            "blocked"
        } else if decision == "complete" {
            "complete"
        } else if record.body["trigger"]["kind"] == "once" {
            "exhausted"
        } else {
            "active"
        };
        if matches!(decision, "continue" | "complete")
            && policy_valid
            && (fresh || matches!(current.as_str(), "cancelled" | "expired" | "exhausted"))
        {
            record.active = None;
            record.occurrences.get_mut(&active).ok_or("occurrence")?["phase"] = json!("settled");
        }
        self.transition(
            &mut state,
            &mut record,
            status,
            vec![reference.clone()],
            now,
        )?;
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(reference)
    }
    pub(crate) fn check_evaluation_policy(
        &self,
        record: &Record,
        evaluation: &Value,
        scope: &Value,
    ) -> Result<bool> {
        if matches!(evaluation["decision"].as_str(), Some("unknown" | "blocked")) {
            return Ok(true);
        }
        let claims = scope["claims"].as_object().ok_or("claims")?;
        let parent = claims
            .iter()
            .find(|(_, l)| l["body"]["reservation"] == record.body["reservation"])
            .map(|(id, _)| id.as_str())
            .ok_or("parent")?;
        for (id, claim) in claims {
            if id == parent {
                continue;
            }
            let mut ancestor = claim["parent"].as_str();
            let mut belongs = false;
            for _ in 0..64 {
                let Some(key) = ancestor else {
                    break;
                };
                if key == parent {
                    belongs = true;
                    break;
                }
                ancestor = claims.get(key).ok_or("ancestor")?["parent"].as_str();
            }
            if belongs
                && (claim["phase"] != "released"
                    || claim["unknown"] == true
                    || claim["dispatched"] == true && claim["result"].is_null())
            {
                return Ok(false);
            }
        }
        let mut expected = BTreeSet::new();
        for value in record.outcomes.values() {
            let outcome = self.store.json(&value["reference"])?;
            let claim = claims
                .values()
                .find(|l| l["reference"] == outcome["claim"])
                .ok_or("settled claim")?;
            if claim["phase"] != "released"
                || claim["unknown"] == true
                || claim["result"]["outcome"] != "completed"
                || claim["check"].is_null()
            {
                return Ok(false);
            }
            let check =
                contracts::parse_strict(&self.coordinator.get_local_bytes(&claim["check"])?)
                    .map_err(|e| e.to_string())?;
            if check["passed"] != true
                || check["checker"] != self.evaluator.as_ref().ok_or("evaluator")?.authority()
            {
                return Ok(false);
            }
            expected.insert(
                outcome["claim"]["digest"]
                    .as_str()
                    .ok_or("claim digest")?
                    .to_string(),
            );
        }
        let mut cited = BTreeSet::new();
        for r in evaluation["evidence"].as_array().ok_or("evidence")? {
            if r["schema"] == "openagents.claim.v1"
                && !cited.insert(r["digest"].as_str().ok_or("claim")?.to_string())
            {
                return Ok(false);
            }
        }
        if cited != expected {
            return Ok(false);
        }
        let rule = self.evaluator.as_ref().ok_or("evaluator")?.policy()?.rule;
        let rule = contracts::parse_strict(&self.coordinator.get_local_bytes(&rule)?)
            .map_err(|e| e.to_string())?;
        let required = rule["minimum_successes"]
            .as_u64()
            .ok_or("completion count")?;
        Ok(evaluation["decision"]
            == if expected.len() as u64 >= required {
                "complete"
            } else {
                "continue"
            })
    }
    pub(crate) fn tick_continuation(&self, reference: &Value, now: u64) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        if matches!(
            record.state["status"].as_str(),
            Some("complete" | "cancelled" | "exhausted" | "expired")
        ) {
            return Ok(record.state);
        }
        if now >= record.body["expires_at"].as_u64().ok_or("expiry")? {
            self.transition(&mut state, &mut record, "expired", vec![], now)?;
        } else {
            if record.state["status"] != "active"
                || record.active.is_some()
                || record.pending.is_some()
            {
                return Ok(record.state);
            }
            let prior = record.evaluations.values().last();
            if record.occurrences.len() > 0
                && prior
                    .is_none_or(|e| e["body"]["decision"] != "continue" || e["accepted"] != true)
            {
                return Err("not_admitted: an independent continue decision is required".into());
            }
            let due = if let Some(previous) = prior {
                previous["at"]
                    .as_u64()
                    .ok_or("evaluation time")?
                    .checked_add(
                        record.body["trigger"]["minimum_interval_seconds"]
                            .as_u64()
                            .ok_or("minimum interval")?,
                    )
                    .ok_or("continuation time")?
            } else {
                record.body["starts_at"].as_u64().ok_or("start")?
            };
            if now < due {
                return Ok(record.state);
            }
            if record.occurrences.len() as u64
                >= record.body["max_occurrences"]
                    .as_u64()
                    .ok_or("occurrence cap")?
            {
                self.transition(&mut state, &mut record, "exhausted", vec![], now)?;
            } else {
                self.coordinator.automation_parent(
                    &record.body,
                    &state.policy.worker,
                    &state.policy.checker,
                    &BTreeMap::new(),
                    now,
                )?;
                let occurrence = workspace_host::artifact::token();
                let units = crate::plans::units(&record.body["per_occurrence"])?;
                if record
                    .accounting
                    .reserve(&occurrence, units.clone())
                    .is_err()
                {
                    self.transition(&mut state, &mut record, "exhausted", vec![], now)?;
                } else {
                    let predecessor = record
                        .outcomes
                        .values()
                        .max_by_key(|v| v["ordinal"].as_u64())
                        .map(|v| v["reference"].clone());
                    let body = json!({"v":"openagents.automation-occurrence.v1","requires":[],"plan":record.reference,"occurrence":occurrence,"ordinal":record.occurrences.len()+1,"key":{"kind":"continuation","predecessor":predecessor},"due_at":due,"deadline":now.checked_add(units["wall_ms"].div_ceil(1000)).ok_or("deadline")?.min(record.body["expires_at"].as_u64().ok_or("expiry")?),"observation":null,"prior":predecessor,"request":workspace_host::artifact::token(),"run":workspace_host::artifact::token(),"task":workspace_host::artifact::token()});
                    automation::validate(&body).map_err(|e| e.to_string())?;
                    let reference = self.declare(
                        &mut state,
                        &body,
                        record.body["owner"].as_str().ok_or("owner")?,
                        now,
                    )?;
                    let wrapper = json!({"v":"openagents.automation-input.v1","requires":[],"plan":record.reference,"occurrence":reference,"frame":record.body["frame"],"payload":record.body["payload"],"context":record.body["context"],"prior":predecessor});
                    let input = self.store.put_json(&wrapper)?;
                    record.occurrences.insert(occurrence.clone(),json!({"reference":reference,"body":body,"input":input,"phase":"prepare","started_at":now,"proposal":null,"claim":null,"execute":null,"result":null,"check":null}));
                    record.active = Some(occurrence);
                    self.transition(&mut state, &mut record, "active", vec![reference], now)?;
                }
            }
        }
        self.propagate_terminal(&mut state, &mut record, now)?;
        let result = record.state.clone();
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result)
    }
}
impl EvaluationGate for Automation {
    fn with_fence(
        &self,
        proof: &EvaluationIntake,
        now: u64,
        call: &mut dyn FnMut() -> Result<Reply>,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        let record = state
            .plans
            .values()
            .find(|r| {
                r.pending
                    .as_ref()
                    .is_some_and(|p| p["phase"] == "evaluate" && p["event"]["id"] == proof.event())
            })
            .ok_or("not_admitted: exact pending evaluation required")?;
        let pending = record.pending.as_ref().ok_or("pending")?;
        if record.state["status"] != "active"
            || now >= record.body["expires_at"].as_u64().ok_or("expiry")?
            || proof.authority() != state.policy.checker
            || proof.principal() != self.authority()
            || *proof.payload() != pending["payload"]
            || proof.input()["plan"] != record.reference
            || proof.admission()["request"] != pending["payload"]["request"]
        {
            return Err("not_admitted: current evaluation intake fence differs".into());
        }
        self.coordinator.automation_parent(
            &record.body,
            &state.policy.worker,
            &state.policy.checker,
            &BTreeMap::new(),
            now,
        )?;
        call()
    }
}
