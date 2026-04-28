use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::error::RegistrarError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NostrJson {
    /// Map of handle -> 64-char lowercase hex pubkey, per NIP-05.
    pub names: BTreeMap<String, String>,
    /// Optional NIP-05 relays map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub relays: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    reserved: BTreeSet<String>,
    state: RwLock<NostrJson>,
}

impl Store {
    pub fn load(path: PathBuf, reserved: BTreeSet<String>) -> Result<Self, RegistrarError> {
        let initial = if path.exists() {
            let bytes = std::fs::read(&path).map_err(|err| {
                tracing::error!(error = %err, path = %path.display(), "failed to read nostr.json");
                RegistrarError::Internal
            })?;
            if bytes.is_empty() {
                NostrJson::default()
            } else {
                serde_json::from_slice::<NostrJson>(&bytes).map_err(|err| {
                    tracing::error!(error = %err, "failed to parse nostr.json");
                    RegistrarError::Internal
                })?
            }
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    tracing::error!(error = %err, "failed to create nostr.json parent dir");
                    RegistrarError::Internal
                })?;
            }
            let empty = NostrJson::default();
            atomic_write_json(&path, &empty)?;
            empty
        };

        Ok(Self {
            path,
            reserved,
            state: RwLock::new(initial),
        })
    }

    fn read_guard(&self) -> Result<RwLockReadGuard<'_, NostrJson>, RegistrarError> {
        self.state.read().map_err(|err| {
            tracing::error!(error = %err, "store rwlock read poisoned");
            RegistrarError::Internal
        })
    }

    fn write_guard(&self) -> Result<RwLockWriteGuard<'_, NostrJson>, RegistrarError> {
        self.state.write().map_err(|err| {
            tracing::error!(error = %err, "store rwlock write poisoned");
            RegistrarError::Internal
        })
    }

    pub fn snapshot(&self) -> Result<NostrJson, RegistrarError> {
        Ok(self.read_guard()?.clone())
    }

    pub fn is_reserved(&self, name: &str) -> bool {
        self.reserved.contains(name)
    }

    pub fn claim(&self, name: &str, pubkey_hex: &str) -> Result<(), RegistrarError> {
        if self.is_reserved(name) {
            return Err(RegistrarError::ReservedHandle);
        }
        let mut guard = self.write_guard()?;
        if guard.names.contains_key(name) {
            return Err(RegistrarError::HandleTaken);
        }
        if guard
            .names
            .values()
            .any(|existing| existing.eq_ignore_ascii_case(pubkey_hex))
        {
            return Err(RegistrarError::PubkeyTaken);
        }
        guard
            .names
            .insert(name.to_string(), pubkey_hex.to_string());
        if let Err(err) = atomic_write_json(&self.path, &guard) {
            // Roll back the in-memory insert so disk and memory match.
            guard.names.remove(name);
            return Err(err);
        }
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), RegistrarError> {
        let mut guard = self.write_guard()?;
        let Some(prev_value) = guard.names.remove(name) else {
            return Err(RegistrarError::NotFound);
        };
        if let Err(err) = atomic_write_json(&self.path, &guard) {
            // Roll back the in-memory removal so disk and memory match.
            guard.names.insert(name.to_string(), prev_value);
            return Err(err);
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn atomic_write_json(path: &Path, value: &NostrJson) -> Result<(), RegistrarError> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|err| {
        tracing::error!(error = %err, "failed to serialize nostr.json");
        RegistrarError::Internal
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nostr.json");
    let tmp_path = parent.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp_path, &serialized).map_err(|err| {
        tracing::error!(error = %err, path = %tmp_path.display(), "failed to write tmp nostr.json");
        RegistrarError::Internal
    })?;
    std::fs::rename(&tmp_path, path).map_err(|err| {
        tracing::error!(error = %err, "failed to rename tmp nostr.json");
        let _ = std::fs::remove_file(&tmp_path);
        RegistrarError::Internal
    })?;
    Ok(())
}
