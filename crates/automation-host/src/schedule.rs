//! UTC slot selection is durable before any cross-host operation.
use crate::{
    Result,
    host::{Automation, Record, State},
    plans::units,
};
use nostr::{automation, contracts};
use serde_json::{Value, json};
impl Automation {
    pub(crate) fn due(record: &Record, slot: u64) -> Result<u64> {
        let t = &record.body["trigger"];
        if t["kind"] == "once" {
            if slot != 0 {
                return Err("once slot already consumed".into());
            }
            return t["at"].as_u64().ok_or("once time".into());
        }
        automation::due_at(t, slot).map_err(|e| e.to_string())
    }
    fn latest(record: &Record, now: u64) -> Option<u64> {
        let trigger = &record.body["trigger"];
        if trigger["kind"] == "once" {
            return (now >= trigger["at"].as_u64()?).then_some(0);
        }
        standing_jobs::Interval {
            first: trigger["first_at"].as_u64()?,
            every: trigger["every_seconds"].as_u64()?,
            expires: record.body["expires_at"].as_u64()?,
        }
        .latest(now)
        .map(|(slot, _)| slot)
    }
    pub(crate) fn skip(
        &self,
        state: &mut State,
        record: &mut Record,
        last: u64,
        reason: &str,
        now: u64,
    ) -> Result<()> {
        if last < record.next_slot {
            return Ok(());
        }
        let body = json!({"v":"openagents.automation-skipped.v1","requires":[],"plan":record.reference,"first_slot":record.next_slot,"last_slot":last,"first_due_at":Self::due(record,record.next_slot)?,"last_due_at":Self::due(record,last)?,"reason":reason});
        automation::validate(&body).map_err(|e| e.to_string())?;
        let reference = self.declare(
            state,
            &body,
            record.body["owner"].as_str().ok_or("owner")?,
            now,
        )?;
        record.skips.push(reference);
        record.next_slot = last.checked_add(1).ok_or("slot overflow")?;
        Ok(())
    }
    pub(crate) fn consume_paused(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<()> {
        if let Some(latest) = Self::latest(record, now) {
            self.skip(state, record, latest, "paused", now)?;
        }
        Ok(())
    }
    /// Admit at most one exact occurrence. A queued or uncertain one remains active.
    pub fn tick(&self, reference: &Value, now: u64) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        if record.body["trigger"]["kind"] == "source" {
            drop(_guard);
            return self.tick_source(reference, now);
        }
        if record.body["trigger"]["kind"] == "continuation" {
            drop(_guard);
            return self.tick_continuation(reference, now);
        }
        let status = record.state["status"]
            .as_str()
            .ok_or("plan status")?
            .to_string();
        if matches!(status.as_str(), "complete" | "cancelled" | "exhausted") {
            return Ok(record.state);
        }
        let Some(latest) = Self::latest(&record, now) else {
            return Ok(record.state);
        };
        if latest < record.next_slot && now < record.body["expires_at"].as_u64().ok_or("expiry")? {
            return Ok(record.state);
        }
        if now >= record.body["expires_at"].as_u64().ok_or("expiry")? {
            self.skip(&mut state, &mut record, latest, "expired", now)?;
            self.transition(&mut state, &mut record, "expired", vec![], now)?;
        } else if status != "active" {
            if status == "paused" {
                self.skip(&mut state, &mut record, latest, "paused", now)?;
                self.transition(&mut state, &mut record, &status, vec![], now)?;
            } else {
                return Ok(record.state);
            }
        } else if record.active.is_some() {
            let skip_latest = record.body["trigger"]["missed"] == "skip";
            if skip_latest {
                self.skip(&mut state, &mut record, latest, "occupied", now)?;
            } else if latest > record.next_slot {
                self.skip(&mut state, &mut record, latest - 1, "occupied", now)?;
            }
            self.transition(&mut state, &mut record, "active", vec![], now)?;
        } else {
            self.coordinator.automation_parent(
                &record.body,
                &state.policy.worker,
                &state.policy.checker,
                &Default::default(),
                now,
            )?;
            if record.occurrences.len() as u64
                >= record.body["max_occurrences"]
                    .as_u64()
                    .ok_or("occurrence limit")?
            {
                self.skip(&mut state, &mut record, latest, "bound", now)?;
                self.transition(&mut state, &mut record, "exhausted", vec![], now)?;
            } else {
                let due = Self::due(&record, latest)?;
                if record.body["trigger"]["kind"] == "interval"
                    && record.body["trigger"]["missed"] == "skip"
                    && now > due
                {
                    self.skip(&mut state, &mut record, latest, "missed", now)?;
                    self.transition(&mut state, &mut record, "active", vec![], now)?;
                } else {
                    if latest > record.next_slot {
                        self.skip(&mut state, &mut record, latest - 1, "missed", now)?;
                    }
                    let occurrence = workspace_host::artifact::token();
                    let request = workspace_host::artifact::token();
                    let run = workspace_host::artifact::token();
                    let allowances = units(&record.body["per_occurrence"])?;
                    if let Err(reason) = record.accounting.reserve(&occurrence, allowances.clone())
                    {
                        self.transition(&mut state, &mut record, "exhausted", vec![], now)?;
                        state.plans.insert(id, record);
                        self.save(&state)?;
                        return Err(reason);
                    }
                    let deadline = now
                        .checked_add(allowances["wall_ms"].div_ceil(1000))
                        .ok_or("deadline overflow")?
                        .min(record.body["expires_at"].as_u64().ok_or("expiry")?);
                    let body = json!({"v":"openagents.automation-occurrence.v1","requires":[],"plan":record.reference,"occurrence":occurrence,"ordinal":record.occurrences.len()+1,"key":if record.body["trigger"]["kind"]=="once"{json!({"kind":"once"})}else{json!({"kind":"interval","slot":latest})},"due_at":due,"deadline":deadline,"observation":null,"prior":record.outcomes.values().max_by_key(|v|v["ordinal"].as_u64()).map(|v|v["reference"].clone()),"request":request,"run":run,"task":workspace_host::artifact::token()});
                    automation::validate(&body).map_err(|e| e.to_string())?;
                    let reference = self.declare(
                        &mut state,
                        &body,
                        record.body["owner"].as_str().ok_or("owner")?,
                        now,
                    )?;
                    let wrapper = json!({"v":"openagents.automation-input.v1","requires":[],"plan":record.reference,"occurrence":reference,"frame":record.body["frame"],"payload":record.body["payload"],"context":record.body["context"],"prior":body["prior"]});
                    automation::validate(&wrapper).map_err(|e| e.to_string())?;
                    let wrapper_ref = self.store.put_json(&wrapper)?;
                    let input_bytes =
                        contracts::jcs(&wrapper).map_err(|e| e.to_string())?.len() as u64;
                    if input_bytes > allowances["input_bytes"]
                        || input_bytes > allowances["output_bytes"]
                    {
                        return Err("limit_exceeded: typed automation wrapper bytes".into());
                    }
                    record.occurrences.insert(occurrence.clone(),json!({"reference":reference,"body":body,"input":wrapper_ref,"phase":"prepare","started_at":now,"proposal":null,"claim":null,"execute":null,"result":null,"check":null}));
                    record.active = Some(occurrence);
                    record.next_slot = latest + 1;
                    self.transition(&mut state, &mut record, "active", vec![reference], now)?;
                }
            }
        }
        let result = record.state.clone();
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result)
    }
}
