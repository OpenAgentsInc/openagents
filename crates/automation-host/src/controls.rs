//! Owner controls use exact commands, compare-and-set, and the intake fence.
use crate::{Result, host::Automation};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
impl Automation {
    pub fn control(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now)?;
        if body["v"] != "openagents.automation-control.v1" {
            return Err("automation control required".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.current_invocation(&state, event)?;
        let id = Self::plan_key(&state, &body["plan"])?;
        let mut record = state.plans[&id].clone();
        if record.body["owner"] != actor || !state.policy.owners.contains(&actor) {
            return Err("not_admitted: current plan owner required".into());
        }
        let command = body["command"].as_str().ok_or("command")?.to_string();
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        if let Some(previous) = record.controls.get(&command) {
            if previous["fingerprint"] != fp {
                return Err("idempotency_conflict: automation control changed".into());
            }
            let result = previous["result"].clone();
            self.remember_invocation(&mut state, event, &result);
            self.save(&state)?;
            return Ok(result);
        }
        if record.controls.len() >= 1024 {
            return Err("automation control retention bound".into());
        }
        if body["action"] == "status" {
            let input = self.retain(&mut state, event, &body)?;
            let result = self.result(&mut state, input, &record, "accepted", now)?;
            record
                .controls
                .insert(command, json!({"fingerprint":fp,"result":result}));
            state.plans.insert(id, record);
            self.remember_invocation(&mut state, event, &result);
            self.save(&state)?;
            return Ok(result);
        }
        if body["expected_revision"] != record.state["revision"] {
            return Err("stale: automation state changed".into());
        }
        let status = record.state["status"].as_str().ok_or("status")?;
        let next = match body["action"].as_str() {
            Some("pause") if status == "active" => "paused",
            Some("resume") if status == "paused" => {
                if record.pending.is_some() {
                    return Err("unknown: reconcile earlier work before resuming".into());
                }
                if now >= record.body["expires_at"].as_u64().ok_or("expiry")? {
                    return Err("not_admitted: automation expired".into());
                }
                if record.body["trigger"]["kind"] == "source" {
                    self.validate_source_plan(&state, &record.body, &Default::default())?;
                }
                self.coordinator.automation_parent(
                    &record.body,
                    &state.policy.worker,
                    &state.policy.checker,
                    &Default::default(),
                    now,
                )?;
                if let Some(active) = &record.active {
                    let item = &record.occurrences[active];
                    if !item["claim"].is_null() {
                        let scope = self.coordinator.scope(&record.body["coordinator"])?;
                        let lease = scope["claims"]
                            .as_object()
                            .ok_or("claims")?
                            .values()
                            .find(|l| l["reference"] == item["claim"])
                            .ok_or("current child claim")?;
                        if lease["unknown"] == true
                            || lease["dispatched"] == true && lease["result"].is_null()
                        {
                            return Err("unknown: earlier worker requires reconciliation".into());
                        }
                    }
                }

                "active"
            }
            Some("cancel") if !matches!(status, "complete" | "cancelled") => "cancelled",
            _ => return Err("conflict: automation control does not apply to current state".into()),
        };
        // Consume elapsed paused slots before resuming; they never become a backlog.
        if status == "paused" {
            self.consume_paused(&mut state, &mut record, now)?;
        }
        let input = self.retain(&mut state, event, &body)?;
        self.transition(&mut state, &mut record, next, vec![input.clone()], now)?;
        let result = self.result(&mut state, input, &record, "accepted", now)?;
        record
            .controls
            .insert(command, json!({"fingerprint":fp,"result":result}));
        state.plans.insert(id, record);
        self.remember_invocation(&mut state, event, &result);
        self.save(&state)?;
        Ok(result)
    }
}
