//! Immutable plan admission and nonrenewable aggregate allowance.
use crate::{
    Result,
    host::{Automation, Record},
};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
use standing_jobs::ledger::{Ledger, Units};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn units(bounds: &Value) -> Result<Units> {
    nostr::coordination::bounds_valid(bounds).map_err(|e| e.to_string())?;
    let mut result = Units::new();
    for body in bounds.as_array().ok_or("bounds")? {
        let bound = contracts::parse_bound(body).map_err(|e| e.to_string())?;
        let name = body["bound"].as_str().ok_or("bound name")?;
        let mechanism = match name {
            "calls" => "native-ledger",
            "wall_ms" => "native-deadline",
            "input_bytes" | "output_bytes" => "native-bytes",
            _ => return Err("cannot_enforce: unsupported aggregate dimension".into()),
        };
        if bound.assurance != contracts::Assurance::Host
            || bound.mechanism != mechanism
            || bound.minimum.is_some_and(|v| v > bound.ceiling)
        {
            return Err("cannot_enforce: aggregate mechanism differs".into());
        }
        result.insert(name.into(), bound.ceiling);
    }
    if ["calls", "wall_ms", "input_bytes", "output_bytes"]
        .iter()
        .any(|n| result.get(*n).copied().unwrap_or(0) == 0)
    {
        return Err(
            "cannot_enforce: explicit calls, time, input, and output allowances required".into(),
        );
    }
    Ok(result)
}
impl Automation {
    pub fn policy(&self) -> Result<Value> {
        let _guard = self.store.lock()?;
        Ok(self.state()?.policy_ref)
    }
    pub fn admit(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now)?;
        if body["v"] != "openagents.automation-plan.v1" {
            return Err("automation plan required".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        if body["owner"] != actor
            || body["controller"] != self.authority()
            || !state.policy.owners.contains(&actor)
        {
            return Err("not_admitted: independently selected plan owner required".into());
        }
        let id = body["automation"]
            .as_str()
            .ok_or("automation ID")?
            .to_string();
        if let Some(record) = state.plans.get(&id).cloned() {
            if record.body != body {
                return Err("idempotency_conflict: immutable plan changed".into());
            }
            let result = self.result(
                &mut state,
                record.reference.clone(),
                &record,
                "duplicate",
                now,
            )?;
            self.save(&state)?;
            return Ok(result);
        }
        if state.plans.len() as u64 >= state.policy.max_plans
            || now >= state.policy.until
            || now >= body["expires_at"].as_u64().ok_or("expiry")?
            || body["expires_at"].as_u64().ok_or("expiry")? > state.policy.until
            || body["expires_at"].as_u64().unwrap() - body["starts_at"].as_u64().unwrap()
                > 90 * 86400
            || body["retain_until"].as_u64().ok_or("retention")?
                > state.policy.until.saturating_add(7 * 86400)
            || body["max_occurrences"].as_u64().unwrap() > 1024
            || body["max_observations"].as_u64().unwrap() > 2048
            || body["policy"] != state.policy_ref
            || body["coordinator"] != state.policy.coordinator
        {
            return Err("not_admitted: plan exceeds native owner policy".into());
        }
        if state
            .plans
            .values()
            .any(|p| p.body["run"] == body["run"] || p.body["reservation"] == body["reservation"])
        {
            return Err("conflict: scheduler identity or parent reservation already bound".into());
        }
        if !matches!(body["trigger"]["kind"].as_str(), Some("once" | "interval"))
            || !body["completion"].is_null()
        {
            return Err(
                "cannot_enforce: source and continuation adapters must be explicitly mounted"
                    .into(),
            );
        }
        let (parent, closure) = self.coordinator.automation_parent(
            &body,
            &state.policy.worker,
            &state.policy.checker,
            files,
            now,
        )?;
        let aggregate = units(&body["aggregate"])?;
        let per = units(&body["per_occurrence"])?;
        if per["calls"] < 3
            || aggregate["calls"] > parent["calls"].as_u64().ok_or("parent calls")?
            || per
                .iter()
                .any(|(name, n)| aggregate.get(name).is_none_or(|cap| n > cap))
        {
            return Err(
                "not_admitted: preparation, worker, and independent check need parent allowance"
                    .into(),
            );
        }
        let coor_policy: coordination_host::book::Policy =
            serde_json::from_value(parent["policy"].clone()).map_err(|_| "coordinator policy")?;
        coordination_host::NativeLimits::parse(&body["per_occurrence"], &coor_policy)?;
        for (digest, bytes) in closure {
            if contracts::digest_bytes(&bytes) != digest {
                return Err("closure digest differs".into());
            }
            self.store.put(&bytes, "application/octet-stream", None)?;
        }
        let requirements = self.store.json(&body["requirements"])?;
        if requirements
            != json!({"v":"openagents.automation-native-requirements.v1","requires":[],"spend_microunits":0})
        {
            return Err("cannot_enforce: unsupported native automation requirements".into());
        }
        if self.store.json(&body["acceptance"])?
            != json!({"v":"openagents.coordination-native-check-policy.v1","requires":[],"check":"exact-admitted-output"})
        {
            return Err("cannot_enforce: independent native output check required".into());
        }
        let reference = self.retain(&mut state, event, &body)?;
        let mut record = Record {
            reference: reference.clone(),
            body,
            original: event.clone(),
            state: Value::Null,
            history: vec![],
            journal: vec![],
            occurrences: BTreeMap::new(),
            outcomes: BTreeMap::new(),
            polls: BTreeMap::new(),
            evaluations: BTreeMap::new(),
            controls: BTreeMap::new(),
            skips: vec![],
            next_slot: 0,
            accounting: Ledger::new(aggregate)?,
            active: None,
            charged_requests: BTreeMap::new(),
            source_revisions: BTreeSet::new(),
            pending: None,
        };
        self.transition(
            &mut state,
            &mut record,
            "active",
            vec![reference.clone()],
            now,
        )?;
        let result = self.result(&mut state, reference, &record, "accepted", now)?;
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result)
    }
}
