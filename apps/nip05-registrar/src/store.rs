use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::error::RegistrarError;
use crate::validation::{is_valid_hex_pubkey, validate_handle};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NostrJson {
    /// Map of handle -> 64-char lowercase hex pubkey, per NIP-05.
    pub names: BTreeMap<String, String>,
    /// Optional NIP-05 relays map keyed by hex pubkey.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub relays: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct Store {
    /// Single-writer assumption: this store owns the file. The registrar
    /// process is the only writer. Concurrent reads are fine; concurrent
    /// writers will race on rename and corrupt the snapshot. Run one
    /// registrar process per data file.
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
                let parsed = serde_json::from_slice::<NostrJson>(&bytes).map_err(|err| {
                    tracing::error!(error = %err, "failed to parse nostr.json");
                    RegistrarError::Internal
                })?;
                validate_loaded(&parsed, &reserved)?;
                parsed
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

    pub fn has_handle(&self, name: &str) -> Result<bool, RegistrarError> {
        Ok(self.read_guard()?.names.contains_key(name))
    }

    /// Returns true if the pubkey is already mapped to a *different*
    /// handle than `excluding_name`. Lets the challenge issuer give
    /// caller-friendly errors before any signing happens.
    pub fn has_pubkey_other_than(
        &self,
        pubkey_hex: &str,
        excluding_name: &str,
    ) -> Result<bool, RegistrarError> {
        let guard = self.read_guard()?;
        Ok(guard.names.iter().any(|(name, existing)| {
            existing.eq_ignore_ascii_case(pubkey_hex) && name != excluding_name
        }))
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
        guard.names.insert(name.to_string(), pubkey_hex.to_string());
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
        // If no remaining handle points at this pubkey, drop the relays
        // entry too. Otherwise the relays map slowly accumulates dead
        // entries indistinguishable from active ones.
        let still_referenced = guard
            .names
            .values()
            .any(|existing| existing.eq_ignore_ascii_case(&prev_value));
        let removed_relays = if !still_referenced {
            guard.relays.remove(&prev_value)
        } else {
            None
        };

        if let Err(err) = atomic_write_json(&self.path, &guard) {
            // Roll back both map mutations on failure to keep memory and
            // disk consistent.
            guard.names.insert(name.to_string(), prev_value.clone());
            if let Some(relays) = removed_relays {
                guard.relays.insert(prev_value, relays);
            }
            return Err(err);
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn validate_loaded(state: &NostrJson, reserved: &BTreeSet<String>) -> Result<(), RegistrarError> {
    let mut seen_pubkeys: BTreeSet<String> = BTreeSet::new();
    for (name, pubkey) in &state.names {
        if validate_handle(name).is_err() {
            tracing::error!(handle = %name, "startup validation: malformed handle in nostr.json");
            return Err(RegistrarError::Internal);
        }
        if reserved.contains(name) {
            tracing::error!(handle = %name, "startup validation: reserved handle present in nostr.json");
            return Err(RegistrarError::Internal);
        }
        let lowered = pubkey.to_ascii_lowercase();
        if !is_valid_hex_pubkey(&lowered) {
            tracing::error!(
                handle = %name,
                pubkey = %pubkey,
                "startup validation: invalid x-only secp256k1 pubkey in nostr.json"
            );
            return Err(RegistrarError::Internal);
        }
        if !seen_pubkeys.insert(lowered) {
            tracing::error!(
                handle = %name,
                pubkey = %pubkey,
                "startup validation: duplicate pubkey across handles in nostr.json"
            );
            return Err(RegistrarError::Internal);
        }
    }
    for (pubkey, relays) in &state.relays {
        if !is_valid_hex_pubkey(&pubkey.to_ascii_lowercase()) {
            tracing::error!(
                pubkey = %pubkey,
                "startup validation: invalid pubkey key in relays map"
            );
            return Err(RegistrarError::Internal);
        }
        for relay in relays {
            // Reject obviously broken relays. We only sanity-check
            // protocol/host shape, not reachability.
            if !(relay.starts_with("ws://") || relay.starts_with("wss://"))
                || relay.len() < 7
                || relay.contains(char::is_whitespace)
            {
                tracing::error!(
                    relay = %relay,
                    "startup validation: malformed relay url in nostr.json"
                );
                return Err(RegistrarError::Internal);
            }
        }
    }
    Ok(())
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
    // Unique temp filename so concurrent calls don't clobber each other's
    // tmp file. Single-writer is the contract; this just keeps a brief
    // race during startup or test harnesses from corrupting state.
    let mut rng_buf = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut rng_buf);
    let tmp_path = parent.join(format!(".{}.tmp.{}", file_name, hex::encode(rng_buf)));

    let mut open_opts = OpenOptions::new();
    open_opts.write(true).create_new(true).truncate(true);
    #[cfg(unix)]
    {
        open_opts.mode(0o600);
    }
    let mut file = open_opts.open(&tmp_path).map_err(|err| {
        tracing::error!(error = %err, path = %tmp_path.display(), "failed to open tmp nostr.json");
        RegistrarError::Internal
    })?;
    if let Err(err) = file.write_all(&serialized) {
        tracing::error!(error = %err, "failed to write tmp nostr.json");
        let _ = std::fs::remove_file(&tmp_path);
        return Err(RegistrarError::Internal);
    }
    if let Err(err) = file.flush() {
        tracing::error!(error = %err, "failed to flush tmp nostr.json");
        let _ = std::fs::remove_file(&tmp_path);
        return Err(RegistrarError::Internal);
    }
    if let Err(err) = file.sync_all() {
        tracing::error!(error = %err, "failed to fsync tmp nostr.json");
        let _ = std::fs::remove_file(&tmp_path);
        return Err(RegistrarError::Internal);
    }
    drop(file);

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        tracing::error!(error = %err, "failed to rename tmp nostr.json");
        let _ = std::fs::remove_file(&tmp_path);
        return Err(RegistrarError::Internal);
    }

    // fsync the parent directory so the rename is durable across crashes.
    // No-op or unsupported on some platforms (e.g. older Windows); skip
    // gracefully if the open fails.
    #[cfg(unix)]
    if let Ok(dir) = std::fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}
