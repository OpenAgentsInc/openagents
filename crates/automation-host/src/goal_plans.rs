//! The owner-pinned rule and the independent native receiver are separate pins.
use crate::{
    Result,
    host::{Automation, State},
};
use nostr::contracts;
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Automation {
    pub fn mount_evaluator(mut self, evaluator: crate::evaluator::Evaluator) -> Result<Self> {
        let state = self.state()?;
        let policy = evaluator.policy()?;
        if state.policy.evaluator.as_ref() != Some(&evaluator.binding()?)
            || evaluator.authority() != state.policy.checker
            || policy.producer != state.policy.worker
            || policy.controller != self.authority()
            || policy.coordinator != self.coordinator.authority()
            || policy.scope != state.policy.coordinator
        {
            return Err("not_admitted: exact independently selected evaluator required".into());
        }
        self.evaluator = Some(evaluator);
        Ok(self)
    }
    pub(crate) fn validate_goal_plan(
        &self,
        state: &State,
        plan: &Value,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let evaluator = self
            .evaluator
            .as_ref()
            .ok_or("cannot_enforce: independent goal checker is not mounted")?;
        let bundle = evaluator.binding()?;
        let policy = evaluator.policy()?;
        if state.policy.evaluator.as_ref() != Some(&bundle)
            || plan["completion"].is_null()
            || plan["expires_at"].as_u64().ok_or("expiry")? > policy.until
        {
            return Err("not_admitted: evaluator policy does not cover plan".into());
        }
        for field in ["target", "lock", "context", "requirements"] {
            if plan["completion"][field] != bundle[field] {
                return Err("identity_mismatch: goal checker pins differ".into());
            }
        }
        let bounds = crate::plans::units(&plan["completion"]["bounds"])?;
        if bounds["calls"] != 1
            || bounds["wall_ms"] > 30000
            || bounds["input_bytes"] != 1048576
            || bounds["output_bytes"] != 1048576
        {
            return Err("cannot_enforce: native checker bounds differ".into());
        }
        let per = crate::plans::units(&plan["per_occurrence"])?;
        if per["input_bytes"] != 1048576 || per["output_bytes"] != 1048576 {
            return Err("cannot_enforce: goal-checking occurrence byte ceiling differs".into());
        }
        let frame = contracts::parse_strict(&self.coordinator.get_local_bytes(&plan["frame"])?)
            .map_err(|e| e.to_string())?;
        let rule = contracts::parse_strict(&self.coordinator.get_local_bytes(&policy.rule)?)
            .map_err(|e| e.to_string())?;
        if !frame["acceptance"]
            .as_array()
            .is_some_and(|a| a.contains(&policy.rule))
            || rule["v"] != "openagents.automation-native-completion-rule.v1"
            || rule["requires"] != json!([])
            || rule["check"] != "verified-occurrence-count"
            || rule["acceptance"] != plan["acceptance"]
            || rule.as_object().is_none_or(|o| o.len() != 5)
            || rule["minimum_successes"]
                .as_u64()
                .is_none_or(|n| !(1..=16).contains(&n))
        {
            return Err("not_admitted: owner-protected completion rule required".into());
        }
        let mut files = evaluator.binding_files()?;
        let bytes = contracts::jcs(&rule).map_err(|e| e.to_string())?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(&policy.rule).map_err(|e| e.to_string())?,
            &bytes,
        )
        .map_err(|e| e.to_string())?;
        files.insert(contracts::digest_bytes(&bytes), bytes);
        Ok(files)
    }
}
