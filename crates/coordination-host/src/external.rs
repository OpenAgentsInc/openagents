//! Operator-owned pins for native operations executed by a separate host.
//! Registration is inert; the owning runtime must implement fenced intake.
use crate::{Result, book::Coordinator};
use nostr::contracts;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalOperation {
    pub definition: Value,
    pub worker: String,
    pub revoked: bool,
}

impl Coordinator {
    /// Host-key administration. Neither a task proposal nor a participant
    /// grant can register an executor or change its implementation pins.
    pub fn register_executor(
        &self,
        definition: &Value,
        worker: &str,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Value> {
        worker
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "worker key")?;
        let parsed = contracts::parse_definition(definition).map_err(|e| e.to_string())?;
        if worker == self.authority()
            || parsed.artifact.schema.as_deref() != Some("openagents.cap.v1")
            || !parsed.id.starts_with(&format!("{worker}:"))
            || parsed
                .id
                .starts_with(&format!("{}:coord/", self.authority()))
        {
            return Err(
                "not_admitted: executor definition must belong to the named external worker".into(),
            );
        }
        if files.len() > 256 || files.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576 {
            return Err("limit_exceeded: executor artifacts".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.attachments(files)?;
        let body = contracts::parse_strict(&self.store.get(&definition["artifact"])?)
            .map_err(|e| e.to_string())?;
        let cap = nostr::cap::parse_definition(&body).map_err(|e| e.to_string())?;
        if cap.id != parsed.id || cap.profile != nostr::cap::Profile::Native || cap.effects.spend {
            return Err(
                "unsupported_feature: executor needs a native zero-spend definition".into(),
            );
        }
        for name in ["input", "output"] {
            let schema = contracts::parse_schema_ref(&body[name]).map_err(|e| e.to_string())?;
            let bytes = self.store.get(&body[name])?;
            contracts::prepare_closure(&BTreeMap::from([(schema.0.digest, bytes)]))
                .map_err(|e| e.to_string())?;
        }
        let key = parsed.artifact.digest;
        if let Some(previous) = state.executors.get(&key) {
            if previous.definition != *definition || previous.worker != worker || previous.revoked {
                return Err("not_admitted: registered executor differs or was revoked".into());
            }
            return Ok(definition.clone());
        }
        if state.executors.len() >= 256 {
            return Err("limit_exceeded: registered executors".into());
        }
        state.executors.insert(
            key,
            ExternalOperation {
                definition: definition.clone(),
                worker: worker.into(),
                revoked: false,
            },
        );
        self.save(&state)?;
        Ok(definition.clone())
    }

    /// Revoke new claims and intake without asserting that earlier work stopped.
    /// Revocation of this immutable binding is permanent.
    pub fn revoke_executor(&self, definition: &Value) -> Result<()> {
        let parsed = contracts::parse_definition(definition).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let registered = state
            .executors
            .get_mut(&parsed.artifact.digest)
            .ok_or("not_admitted: executor unavailable")?;
        if registered.definition != *definition {
            return Err("identity_mismatch: executor definition differs".into());
        }
        registered.revoked = true;
        self.save(&state)
    }

    pub(crate) fn external_operation(&self, definition: &Value) -> Result<ExternalOperation> {
        let parsed = contracts::parse_definition(definition).map_err(|e| e.to_string())?;
        let state = self.state()?;
        let registered = state
            .executors
            .get(&parsed.artifact.digest)
            .ok_or("not_admitted: foreign operation has no registered executor")?;
        if registered.definition != *definition || registered.revoked {
            return Err("not_admitted: executor binding differs or was revoked".into());
        }
        Ok(registered.clone())
    }

    pub(crate) fn check_executor(&self, definition: &Value, worker: &str) -> Result<()> {
        let parsed = contracts::parse_definition(definition).map_err(|e| e.to_string())?;
        if parsed
            .id
            .starts_with(&format!("{}:coord/", self.authority()))
        {
            return Ok(());
        }
        if self.external_operation(definition)?.worker != worker {
            return Err("not_admitted: claim names another executor".into());
        }
        Ok(())
    }
}
