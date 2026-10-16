//! Durable cross-host steps use exact retained inputs; uncertain work is not rerun.
use crate::{Result, host::Automation};
use nostr::{automation, contracts, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Automation {
    fn coor_request(&self, body: Value, now: u64) -> Result<Event> {
        workspace_host::host::request(&self.secret, self.coordinator.authority(), body, now)
    }
    pub(crate) fn input_files(&self, root: &Value) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut files = BTreeMap::new();
        let mut queue = vec![(root.clone(), 0)];
        while let Some((r, depth)) = queue.pop() {
            let parsed = contracts::parse_artifact(&r).map_err(|e| e.to_string())?;
            if files.contains_key(&parsed.digest) {
                continue;
            }
            if matches!(
                parsed.schema.as_deref(),
                Some(
                    "openagents.automation-native-policy.v1"
                        | "openagents.coordination-native-policy.v1"
                        | "openagents.coordinator.v1"
                        | "openagents.workspace-binding.v1"
                )
            ) {
                continue;
            }
            if depth > 32 || files.len() >= 256 {
                return Err("automation input closure bound".into());
            }
            let bytes = self
                .store
                .get(&r)
                .map_err(|e| format!("input {}: {e}", parsed.digest))?;
            if matches!(
                parsed.media_type.as_str(),
                "application/json" | "application/schema+json"
            ) {
                let b = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
                let mut stack = vec![&b];
                while let Some(v) = stack.pop() {
                    if contracts::parse_artifact(v).is_ok() {
                        queue.push((v.clone(), depth + 1));
                        continue;
                    }
                    match v {
                        Value::Object(fields) => stack.extend(
                            fields
                                .iter()
                                .filter(|(k, _)| k.as_str() != "meta")
                                .map(|(_, v)| v),
                        ),
                        Value::Array(items) => stack.extend(items),
                        _ => {}
                    }
                }
            }
            files.insert(parsed.digest, bytes);
        }
        Ok(files)
    }
    /// Advance local admission steps. Returns the exact encrypted worker request
    /// after preparation; this method never runs the worker itself.
    pub fn advance(&self, reference: &Value, now: u64) -> Result<Option<Event>> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        if record.state["status"] != "active"
            || now >= record.body["expires_at"].as_u64().ok_or("expiry")?
        {
            return Err("not_admitted: automation is not active".into());
        }
        let Some(active) = record.active.clone() else {
            return Ok(None);
        };
        for _ in 0..4 {
            let occurrence = record.occurrences[&active].clone();
            if occurrence["phase"] == "worker" {
                return serde_json::from_value(occurrence["execute"].clone())
                    .map(Some)
                    .map_err(|_| "worker request".into());
            }
            if now >= occurrence["body"]["deadline"].as_u64().ok_or("deadline")? {
                return Err(
                    "unknown: occurrence deadline passed; exact outcome requires recovery".into(),
                );
            }
            let (parent, _) = self.coordinator.automation_parent(
                &record.body,
                &state.policy.worker,
                &state.policy.checker,
                &BTreeMap::new(),
                now,
            )?;
            let phase = occurrence["phase"].as_str().ok_or("occurrence phase")?;
            let input = if let Some(pending) = &record.pending {
                if pending["phase"] != phase {
                    return Err("unknown: pending cross-host phase differs".into());
                }
                serde_json::from_value::<Event>(pending["event"].clone())
                    .map_err(|_| "pending event")?
            } else {
                let body = match phase {
                    "prepare" => {
                        json!({"v":"openagents.coordination-native-auxiliary.v1","requires":[],"coordinator":record.body["coordinator"],"claim":parent["claim"],"command":workspace_host::artifact::token(),"purpose":"prepare","request":occurrence["body"]["request"],"fingerprint":contracts::digest_value(&occurrence["body"]).map_err(|e|e.to_string())?})
                    }
                    "propose" => self.occurrence_proposal(&record, &occurrence)?,
                    "claim" => {
                        json!({"v":"openagents.claim-request.v1","requires":[],"proposal":occurrence["proposal"],"worker":state.policy.worker,"resources":if record.body["trigger"]["kind"]=="source"{let observation=self.store.json(&occurrence["body"]["observation"])?;json!([{"id":record.body["trigger"]["resource"],"mode":"read","snapshot":observation["evidence"]}])}else{json!([])},"expected_revision":self.coordinator.scope(&record.body["coordinator"])?["revision"],"requested_until":occurrence["body"]["deadline"],"reservation":record.body["reservation"]})
                    }
                    "dispatch" => {
                        json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":record.body["coordinator"],"claim":occurrence["claim"],"request":occurrence["body"]["request"],"run":occurrence["body"]["run"]})
                    }
                    _ => return Err("unknown: unsupported occurrence phase".into()),
                };
                let event = self.coor_request(body, now)?;
                record.pending = Some(json!({"phase":phase,"event":event}));
                state.plans.insert(id.clone(), record.clone());
                self.save(&state)?;
                event
            };
            let (next, value) = match phase {
                "prepare" => ("propose", self.coordinator.charge_auxiliary(&input, now)?),
                "propose" => (
                    "claim",
                    self.coordinator.propose(
                        &input,
                        &self.input_files(&occurrence["input"])?,
                        now,
                    )?["value"]
                        .clone(),
                ),
                "claim" => (
                    "dispatch",
                    self.coordinator.claim(&input, now)?["value"].clone(),
                ),
                "dispatch" => (
                    "worker",
                    serde_json::to_value(self.coordinator.prepare(&input, now)?)
                        .map_err(|e| e.to_string())?,
                ),
                _ => return Err("occurrence phase".into()),
            };
            if matches!(phase, "propose" | "claim") {
                let bytes = self.coordinator.get_local_bytes(&value)?;
                self.store
                    .put(&bytes, "application/json", value["schema"].as_str())?;
                if phase == "claim" {
                    let claim = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
                    let reservation = &claim["reservation"];
                    let bytes = self.coordinator.get_local_bytes(reservation)?;
                    self.store
                        .put(&bytes, "application/json", reservation["schema"].as_str())?;
                }
            }
            let item = record
                .occurrences
                .get_mut(&active)
                .ok_or("active occurrence")?;
            item["phase"] = json!(next);
            item[match phase {
                "prepare" => "preparation",
                "propose" => "proposal",
                "claim" => "claim",
                _ => "execute",
            }] = value;
            record.pending = None;
            self.transition(&mut state, &mut record, "active", vec![], now)?;
            state.plans.insert(id.clone(), record.clone());
            self.save(&state)?;
        }
        serde_json::from_value(record.occurrences[&active]["execute"].clone())
            .map(Some)
            .map_err(|_| "worker request".into())
    }
    /// Record a terminal worker result through the actual coordinator verifier.
    /// No caller-provided outcome or usage summary is accepted as evidence.
    pub fn record_result(
        &self,
        reference: &Value,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        let active = record
            .occurrences
            .iter()
            .find(|(_, o)| {
                o["execute"]["id"].as_str().is_some_and(|id| {
                    event
                        .tags
                        .iter()
                        .any(|t| t.0 == vec!["e".to_string(), id.to_string()])
                })
            })
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: result has no admitted occurrence")?;
        let occurrence = record.occurrences[&active].clone();
        let execute: Event =
            serde_json::from_value(occurrence["execute"].clone()).map_err(|_| "worker request")?;
        if !event
            .tags
            .iter()
            .any(|t| t.0 == vec!["e".to_string(), execute.id.clone()])
        {
            return Err("identity_mismatch: result is not the current occurrence".into());
        }
        let result = self.coordinator.record_result(event, files, now)?;
        if result["request"] != occurrence["body"]["request"]
            || result["run"] != occurrence["body"]["run"]
        {
            return Err("identity_mismatch: occurrence result differs".into());
        }
        if occurrence["result"].is_object() {
            if self.store.json(&occurrence["result"])? != result {
                return Err("conflict: occurrence result fork".into());
            }
            return Ok(occurrence["result"].clone());
        }
        for (digest, bytes) in files {
            if contracts::digest_bytes(bytes) != *digest {
                return Err("result digest differs".into());
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let terminal = nostr::run::terminal_from_result(&result, files, &state.policy.worker)
            .map_err(|e| e.to_string())?;
        let result_ref = self.store.put_json(&result)?;
        // Keep the original signed event encrypted to its actual coordinator.
        self.store
            .put_json(&serde_json::to_value(event).map_err(|e| e.to_string())?)?;
        let item = record
            .occurrences
            .get_mut(&active)
            .ok_or("active occurrence")?;
        item["result"] = result_ref.clone();
        item["terminal"] = terminal;
        item["phase"] = json!("check");
        let status = record.state["status"].as_str().ok_or("status")?.to_string();
        self.transition(
            &mut state,
            &mut record,
            &status,
            vec![result_ref.clone()],
            now,
        )?;
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result_ref)
    }
    /// Run the independently configured native checker against actual retained bytes.
    pub fn check(
        &self,
        reference: &Value,
        checker: &coordination_host::checks::Checker,
        now: u64,
    ) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        let Some(active) = record.active.clone() else {
            return record
                .outcomes
                .values()
                .max_by_key(|v| v["ordinal"].as_u64())
                .map(|v| v["reference"].clone())
                .ok_or("no result to check".into());
        };
        if let Some(outcome) = record.outcomes.get(&active) {
            return Ok(outcome["reference"].clone());
        }
        let occurrence = record.occurrences[&active].clone();
        if occurrence["phase"] != "check" {
            return Err("not_admitted: active independently checkable result required".into());
        }
        if checker.authority() != state.policy.checker {
            return Err("not_admitted: independently selected checker differs".into());
        }
        // The checker's actual signed response and current COORD grant are verified
        // before this controller records a known terminal outcome.
        if now >= record.body["expires_at"].as_u64().ok_or("expiry")?
            && !occurrence["check"].is_object()
        {
            return Err(
                "not_admitted: plan expired before a new check; reconcile the known result".into(),
            );
        }
        let worker_result = self.store.json(&occurrence["result"])?;
        let successful = worker_result["outcome"] == "completed";
        let (check_ref, report) = if !successful {
            (Value::Null, Value::Null)
        } else if occurrence["check"].is_object() {
            (
                occurrence["check"].clone(),
                occurrence["check_event"].clone(),
            )
        } else {
            let report = checker.check(
                &self.coordinator,
                &record.body["coordinator"],
                &occurrence["claim"],
                now,
            )?;
            if report.pubkey != state.policy.checker {
                return Err("not_admitted: independently selected checker differs".into());
            }
            let check_ref = self.coordinator.record_check(&report, now)?;
            let check_body =
                contracts::parse_strict(&self.coordinator.get_local_bytes(&check_ref)?)
                    .map_err(|e| e.to_string())?;
            self.store.put_json(&check_body)?;
            let item = record
                .occurrences
                .get_mut(&active)
                .ok_or("active occurrence")?;
            item["check"] = check_ref.clone();
            item["check_event"] = serde_json::to_value(&report).map_err(|e| e.to_string())?;
            state.plans.insert(id.clone(), record.clone());
            self.save(&state)?;
            (
                check_ref,
                serde_json::to_value(report).map_err(|e| e.to_string())?,
            )
        };
        let passed = successful && self.store.json(&check_ref)?["passed"] == true;
        let evidence = if check_ref.is_object() {
            vec![check_ref.clone()]
        } else {
            vec![]
        };
        let request = if let Some(pending) = &record.pending {
            if pending["phase"] != "release" {
                return Err("unknown: reconcile the current pending operation".into());
            }
            serde_json::from_value::<Event>(pending["event"].clone())
                .map_err(|_| "retained release event")?
        } else {
            let request=self.coor_request(json!({"v":"openagents.coordination-control.v1","requires":[],"coordinator":record.body["coordinator"],"proposal":occurrence["proposal"],"claim":occurrence["claim"],"action":"release","expected_revision":self.coordinator.scope(&record.body["coordinator"])?["revision"],"requested_until":null,"evidence":evidence}),now)?;
            record.pending = Some(json!({"phase":"release","event":request}));
            state.plans.insert(id.clone(), record.clone());
            self.save(&state)?;
            request
        };
        self.coordinator.control(&request, now)?;
        record.pending = None;
        let reserved = record
            .accounting
            .requests
            .get(&active)
            .ok_or("occurrence allowance")?
            .reserved
            .clone();
        // Charge the full admitted finite ceilings. This conservative native profile
        // does not infer unused capacity from incomplete timing or byte telemetry.
        record.accounting.settle(&active, Some(reserved))?;
        let settlement=self.store.put_json(&json!({"v":"openagents.automation-native-settlement.v1","requires":[],"occurrence":occurrence["reference"],"ledger":record.accounting,"checker_event":report,"check":check_ref}))?;
        let outcome = json!({"v":"openagents.automation-outcome.v1","requires":[],"occurrence":occurrence["reference"],"proposal":occurrence["proposal"],"claim":occurrence["claim"],"result":occurrence["result"],"settled":occurrence["terminal"],"evidence":evidence,"usage":[settlement]});
        automation::validate(&outcome).map_err(|e| e.to_string())?;
        let outcome_ref = self.declare(
            &mut state,
            &outcome,
            record.body["owner"].as_str().ok_or("owner")?,
            now,
        )?;
        record.outcomes.insert(
            active.clone(),
            json!({"reference":outcome_ref,"ordinal":occurrence["body"]["ordinal"]}),
        );
        let current = record.state["status"].as_str().ok_or("status")?.to_string();
        let evaluation_pending = passed
            && !record.body["completion"].is_null()
            && !matches!(current.as_str(), "cancelled" | "expired" | "exhausted");
        record.occurrences.get_mut(&active).ok_or("occurrence")?["phase"] =
            json!(if evaluation_pending {
                "evaluate"
            } else {
                "settled"
            });
        if !evaluation_pending {
            record.active = None;
        }
        let status = if current != "active" {
            current.as_str()
        } else if !passed {
            "paused"
        } else if record.body["trigger"]["kind"] == "once" && !evaluation_pending {
            "complete"
        } else {
            "active"
        };
        self.transition(
            &mut state,
            &mut record,
            status,
            vec![outcome_ref.clone()],
            now,
        )?;
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(outcome_ref)
    }
}
impl coordination_host::worker::DispatchGate for Automation {
    fn intake(
        &self,
        proof: &coordination_host::worker::VerifiedIntake,
        now: u64,
        dispatch: &mut dyn FnMut() -> Result<coordination_host::worker::Intake>,
    ) -> Result<coordination_host::worker::Intake> {
        let origin_now = now;
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        if proof.authority() != self.coordinator.authority() || proof.actor() != state.policy.worker
        {
            return Err("not_admitted: AUTO worker/coordinator differs".into());
        }
        let (id, mut record) = state
            .plans
            .iter()
            .find(|(_, r)| {
                r.active
                    .as_ref()
                    .and_then(|id| r.occurrences.get(id))
                    .is_some_and(|o| o["execute"]["id"] == proof.request())
            })
            .map(|(id, r)| (id.clone(), r.clone()))
            .ok_or("not_admitted: exact active AUTO occurrence required")?;
        let item = &record.occurrences[record.active.as_ref().ok_or("active")?];
        if record.state["status"] != "active"
            || item["phase"] != "worker"
            || now >= item["body"]["deadline"].as_u64().ok_or("deadline")?
            || proof.claim() != &item["claim"]
            || proof.scope() != &record.body["coordinator"]
            || proof.payload()["input"]["artifact"] != item["input"]
            || proof.payload()["request"] != item["body"]["request"]
            || proof.payload()["run"] != item["body"]["run"]
        {
            return Err("not_admitted: current AUTO occurrence fence differs".into());
        }
        if record.body["trigger"]["kind"] == "source" {
            self.revalidate_source(&mut state, &mut record, now)?;
            state.plans.insert(id, record.clone());
            self.save(&state)?;
        }
        self.coordinator.automation_parent(
            &record.body,
            &state.policy.worker,
            &state.policy.checker,
            &BTreeMap::new(),
            origin_now.saturating_add(started.elapsed().as_secs()),
        )?;
        dispatch()
    }
}
