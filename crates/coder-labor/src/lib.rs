//! Recoverable, free-only agent labor with separate host execution authority.
//!
//! Encrypted Nostr declarations establish attribution. An agreement does not
//! authorize a command, execution does not accept a delivery, and acceptance
//! does not perform a payment.

use nostr::contracts::{
    ArtifactRef, ContractError, RefusalCode, check_artifact_bytes, jcs, parse_artifact,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub mod admission;
pub mod book;
pub mod execution;
pub mod records;
pub mod store;
pub mod transport;

pub type Result<T> = std::result::Result<T, String>;

/// Exact canonical artifact identity; no locator is followed by this host.
pub fn reference(value: &Value, schema: &str) -> Result<Value> {
    let bytes = jcs(value).map_err(|e| e.to_string())?;
    Ok(
        json!({"digest":nostr::contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema}),
    )
}

pub fn artifact_value(value: &ArtifactRef) -> Value {
    json!({"digest":value.digest,"size":value.size,"media_type":value.media_type,"schema":value.schema})
}

/// A bounded, inert closure of exact public or privately authorized bytes.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Blobs(pub BTreeMap<String, Value>);
impl Blobs {
    pub fn insert(&mut self, value: Value, schema: &str) -> Result<Value> {
        let r = reference(&value, schema)?;
        let mut proposed = self.0.clone();
        proposed.insert(r["digest"].as_str().ok_or("artifact digest")?.into(), value);
        if proposed.len() > 256
            || serde_json::to_vec(&proposed)
                .map_err(|e| e.to_string())?
                .len()
                > 8 * 1024 * 1024
        {
            return Err("labor closure exceeds its retention bound".into());
        }
        self.0 = proposed;
        Ok(r)
    }
    pub fn get(&self, r: &Value) -> Result<&Value> {
        let reference = parse_artifact(r).map_err(|e| e.to_string())?;
        self.resolve(&reference)
    }
    pub fn resolve(&self, r: &ArtifactRef) -> Result<&Value> {
        let value = self
            .0
            .get(&r.digest)
            .ok_or("labor artifact is unavailable")?;
        check_artifact_bytes(r, &jcs(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(value)
    }
}

pub(crate) fn contract(error: impl ToString) -> ContractError {
    ContractError::new(RefusalCode::NotAdmitted, error.to_string())
}

pub(crate) fn exact(value: &Value, schema: &str, fields: &[&str]) -> Result<()> {
    let map = value.as_object().ok_or("labor body must be an object")?;
    if value["v"] != schema || value["requires"] != json!([]) {
        return Err("unsupported labor version or required feature".into());
    }
    if fields.iter().any(|key| !map.contains_key(*key))
        || map
            .keys()
            .any(|key| key != "v" && key != "requires" && !fields.contains(&key.as_str()))
    {
        return Err("labor body has missing or unsupported fields".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
