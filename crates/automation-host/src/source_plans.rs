//! Source bindings are explicit operator mounts, never caller-selected paths.
use crate::{
    Result,
    host::{Automation, State},
};
use nostr::contracts;
use serde_json::Value;
use std::collections::BTreeMap;
impl Automation {
    pub fn mount_source(mut self, source: crate::source::Source) -> Result<Self> {
        let state = self.state()?;
        let policy = source.policy()?;
        if state.policy.source.as_ref() != Some(&source.binding()?)
            || policy.controller != self.authority()
            || policy.coordinator != self.coordinator.authority()
            || policy.scope != state.policy.coordinator
            || [
                self.authority(),
                self.coordinator.authority(),
                state.policy.worker.as_str(),
                state.policy.checker.as_str(),
            ]
            .contains(&source.authority())
        {
            return Err("not_admitted: exact separate source binding required".into());
        }
        self.source = Some(source);
        Ok(self)
    }
    pub(crate) fn validate_source_plan(
        &self,
        state: &State,
        plan: &Value,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let source = self
            .source
            .as_ref()
            .ok_or("cannot_enforce: native source is not mounted")?;
        let bundle = source.binding()?;
        let policy = source.policy()?;
        let trigger = &plan["trigger"];
        for (field, pin) in [
            ("source", "target"),
            ("lock", "lock"),
            ("context", "context"),
            ("requirements", "requirements"),
            ("input", "config"),
        ] {
            if trigger[field] != bundle[pin] {
                return Err("identity_mismatch: source plan pins differ".into());
            }
        }
        if state.policy.source.as_ref() != Some(&bundle)
            || trigger["resource"] != policy.resource
            || plan["expires_at"].as_u64().ok_or("expiry")? > policy.until
        {
            return Err("not_admitted: source policy does not cover plan".into());
        }
        let per = crate::plans::units(&plan["per_occurrence"])?;
        if per["input_bytes"] != 1048576 || per["output_bytes"] != 1048576 {
            return Err("cannot_enforce: source occurrence byte ceilings must cover actual reader and worker responses".into());
        }
        let bounds = crate::plans::units(&trigger["bounds"])?;
        if bounds["calls"] != 1
            || bounds["wall_ms"] > 30000
            || bounds["output_bytes"] != 1048576
            || bounds["input_bytes"] != 1048576
        {
            return Err("cannot_enforce: native source bounds differ".into());
        }
        let permitted = source.binding_files()?;
        // A mount supplies only its own explicitly registered definition closure.
        for (digest, bytes) in files {
            if permitted
                .get(digest)
                .is_some_and(|expected| expected != bytes)
            {
                return Err("identity_mismatch: supplied source pin bytes differ".into());
            }
        }
        for (digest, bytes) in &permitted {
            if contracts::digest_bytes(bytes) != *digest {
                return Err("source pin digest differs".into());
            }
        }
        Ok(permitted)
    }
}
