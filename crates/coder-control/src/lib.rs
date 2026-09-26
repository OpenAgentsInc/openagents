//! Scoped Nostr clients share the existing task owner; they never become one.
//! Local setup establishes authority. Signed envelopes and relay delivery do not.

use nostr::contracts::{ArtifactRef, check_artifact_bytes, jcs, parse_artifact};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub mod client;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
mod store;
pub mod transport;
#[cfg(feature = "host")]
mod view;

#[cfg(feature = "host")]
pub use host::Host;
pub type Result<T> = std::result::Result<T, String>;

/// An explicit owner-installed mapping from opaque wire scope to one local task.
/// This file is host authority and is never accepted from a remote envelope.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub schema: String,
    pub owner: String,
    pub authority: String,
    pub task_directory: PathBuf,
    pub task_id: String,
    pub intent_digest: String,
    pub scope: Value,
    pub policy: Value,
    /// One pinned CAP target, lock, context, and requirements per supported role.
    pub operations: BTreeMap<String, Operation>,
    pub blobs: Blobs,
    pub max_pairing_seconds: u64,
    pub max_grant_seconds: u64,
    pub max_command_seconds: u64,
    pub retain_until: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub target: Value,
    pub lock: Value,
    pub context: Value,
    pub requirements: Value,
}

/// Exact bytes, supplied explicitly. No URL or filesystem locator is followed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Blobs(pub BTreeMap<String, Vec<u8>>);

impl Blobs {
    pub fn insert(&mut self, bytes: Vec<u8>, media_type: &str, schema: &str) -> Result<Value> {
        let reference = reference(&bytes, media_type, schema);
        let mut proposed = self.0.clone();
        proposed.insert(
            reference["digest"]
                .as_str()
                .ok_or("artifact identity")?
                .into(),
            bytes,
        );
        if proposed.len() > 4096
            || proposed.values().map(Vec::len).sum::<usize>() > 16 * 1024 * 1024
        {
            return Err("control artifact closure exceeds its bound".into());
        }
        self.0 = proposed;
        Ok(reference)
    }

    pub fn insert_json(&mut self, value: &Value, schema: &str) -> Result<Value> {
        self.insert(
            jcs(value).map_err(|e| e.to_string())?,
            "application/json",
            schema,
        )
    }

    pub fn bytes(&self, reference: &Value) -> Result<&[u8]> {
        let reference = parse_artifact(reference).map_err(|e| e.to_string())?;
        self.resolve(&reference)
    }

    pub fn resolve(&self, reference: &ArtifactRef) -> Result<&[u8]> {
        let bytes = self
            .0
            .get(&reference.digest)
            .ok_or("control artifact is unavailable")?;
        check_artifact_bytes(reference, bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    pub fn json(&self, reference: &Value) -> Result<Value> {
        nostr::contracts::parse_strict(self.bytes(reference)?).map_err(|e| e.to_string())
    }
}

pub fn reference(bytes: &[u8], media_type: &str, schema: &str) -> Value {
    json!({"digest":nostr::contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":media_type,"schema":schema})
}

/// A durable CJ answer and only the artifacts authorized for this recipient.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub feedback: Event,
    pub result: Event,
    pub artifacts: Vec<Event>,
}

#[cfg(all(test, feature = "host"))]
mod tests;
