//! Stored responses (`store: true`; `docs/inference/gateway.md`, section 3,
//! P2).
//!
//! A [`Record`] is one finished response and the context it answered (the
//! input items, earlier turns of a `previous_response_id` chain included),
//! enough to continue from it. Records are:
//!
//! - **Opt-in.** Only a request that sends `store: true` is kept; the
//!   default is `false`. A tenant marked zero-retention is never stored
//!   (the session layer refuses `store: true` for it).
//! - **Owner-scoped.** A record belongs to the tenant that made it. Every
//!   read and delete names the owner; another tenant's id reads as missing.
//! - **Sealed.** Each record is sealed with AES-256-GCM ([`Sealer`]),
//!   bound to its owner and id, before it touches memory or disk. A
//!   directory listing shows a hash of the owner and the response id,
//!   never text.
//! - **Retained for a fixed time.** Each record expires
//!   [`Config::retention_days`] after it was made (30 by default); an
//!   expired record reads as missing and [`ResponseStore::sweep`] removes
//!   it. `DELETE /v1/responses/{id}` removes one at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::item::Item;
use crate::response::Response;
use crate::seal::Sealer;

/// How long records are kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Days a stored response is kept before it expires.
    #[serde(default = "default_retention_days")]
    pub retention_days: u64,
    /// The environment variable holding the sealing key (base64 of 32
    /// bytes), or naming its file as `<var>_FILE`. Without either, the
    /// gateway makes a key file in its state directory.
    #[serde(default = "default_key_env")]
    pub key_env: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            key_env: default_key_env(),
        }
    }
}

fn default_retention_days() -> u64 {
    30
}

fn default_key_env() -> String {
    "INFERENCE_STORE_KEY".to_owned()
}

impl Config {
    /// The retention in milliseconds.
    #[must_use]
    pub fn retention_ms(&self) -> u64 {
        self.retention_days.saturating_mul(86_400_000)
    }
}

/// One stored response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The response id (`resp_` and 32 hex characters).
    pub id: String,
    /// The tenant that made it.
    pub owner: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    /// The input the response answered, earlier turns first.
    pub context: Vec<Item>,
    /// The finished response.
    pub response: Response,
}

impl Record {
    /// The items a continuation starts from: the context, then this
    /// response's output (the spec's `previous_response.input` then
    /// `previous_response.output`).
    #[must_use]
    pub fn continued(&self) -> Vec<Item> {
        let mut items = self.context.clone();
        items.extend(self.response.output.iter().cloned());
        items
    }
}

/// Where records are kept.
pub trait ResponseStore: Send + Sync {
    /// Keeps `record` (sealed), replacing one with the same owner and id.
    ///
    /// # Errors
    ///
    /// A sentence when it could not be kept.
    fn put(&self, record: &Record) -> Result<(), String>;

    /// The owner's record `id`, unless it is missing or expired at
    /// `now_ms`.
    fn get(&self, owner: &str, id: &str, now_ms: u64) -> Option<Record>;

    /// Removes the owner's record `id`. Whether there was one.
    fn delete(&self, owner: &str, id: &str) -> bool;

    /// Removes every record expired at `now_ms`. How many went.
    fn sweep(&self, now_ms: u64) -> usize;
}

/// What a record is sealed to: its purpose, owner, and id.
fn aad(owner: &str, id: &str) -> Vec<u8> {
    format!("openagents:stored-response\0{owner}\0{id}").into_bytes()
}

/// Whether `id` is one of ours (`resp_` and 32 lowercase hex characters),
/// so no other text becomes a path.
#[must_use]
pub fn is_response_id(id: &str) -> bool {
    id.strip_prefix("resp_")
        .is_some_and(|hex| hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn seal(sealer: &Sealer, record: &Record) -> Result<String, String> {
    let plain =
        serde_json::to_vec(record).map_err(|_| "the response could not be encoded".to_owned())?;
    sealer.seal(&aad(&record.owner, &record.id), &plain)
}

fn unseal(sealer: &Sealer, owner: &str, id: &str, blob: &str) -> Option<Record> {
    let plain = sealer.open(&aad(owner, id), blob)?;
    let record: Record = serde_json::from_slice(&plain).ok()?;
    (record.owner == owner && record.id == id).then_some(record)
}

/// Records in memory, sealed: for tests and single-process use.
pub struct MemoryStore {
    sealer: Sealer,
    records: Mutex<HashMap<(String, String), (u64, String)>>,
}

impl MemoryStore {
    #[must_use]
    pub fn new(sealer: Sealer) -> Self {
        Self {
            sealer,
            records: Mutex::new(HashMap::new()),
        }
    }

    /// The sealed blobs held, for tests that check nothing is kept in the
    /// clear.
    #[must_use]
    pub fn sealed(&self) -> Vec<String> {
        self.records
            .lock()
            .map(|records| records.values().map(|(_, blob)| blob.clone()).collect())
            .unwrap_or_default()
    }
}

impl ResponseStore for MemoryStore {
    fn put(&self, record: &Record) -> Result<(), String> {
        let blob = seal(&self.sealer, record)?;
        let mut records = self
            .records
            .lock()
            .map_err(|_| "the store is unavailable".to_owned())?;
        records.insert(
            (record.owner.clone(), record.id.clone()),
            (record.expires_at_ms, blob),
        );
        Ok(())
    }

    fn get(&self, owner: &str, id: &str, now_ms: u64) -> Option<Record> {
        let records = self.records.lock().ok()?;
        let (expires, blob) = records.get(&(owner.to_owned(), id.to_owned()))?;
        if *expires <= now_ms {
            return None;
        }
        unseal(&self.sealer, owner, id, blob)
    }

    fn delete(&self, owner: &str, id: &str) -> bool {
        self.records
            .lock()
            .map(|mut records| records.remove(&(owner.to_owned(), id.to_owned())).is_some())
            .unwrap_or(false)
    }

    fn sweep(&self, now_ms: u64) -> usize {
        self.records
            .lock()
            .map(|mut records| {
                let before = records.len();
                records.retain(|_, (expires, _)| *expires > now_ms);
                before - records.len()
            })
            .unwrap_or(0)
    }
}

/// Records on disk under one directory: `<dir>/<sha256(owner)>/<id>.json`,
/// each file `{"expires_at_ms": ..., "sealed": "oa1...."}`.
pub struct DirStore {
    sealer: Sealer,
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    expires_at_ms: u64,
    sealed: String,
}

impl DirStore {
    /// A store in `dir`, made if missing.
    ///
    /// # Errors
    ///
    /// A sentence when the directory cannot be made.
    pub fn open(dir: &Path, sealer: Sealer) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|_| "the stored responses folder can't be made".to_owned())?;
        Ok(Self {
            sealer,
            dir: dir.to_path_buf(),
        })
    }

    fn owner_dir(&self, owner: &str) -> PathBuf {
        let digest = ring::digest::digest(&ring::digest::SHA256, owner.as_bytes());
        let hex: String = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir.join(hex)
    }

    fn path(&self, owner: &str, id: &str) -> Option<PathBuf> {
        is_response_id(id).then(|| self.owner_dir(owner).join(format!("{id}.json")))
    }
}

impl ResponseStore for DirStore {
    fn put(&self, record: &Record) -> Result<(), String> {
        let path = self
            .path(&record.owner, &record.id)
            .ok_or_else(|| "not a response id".to_owned())?;
        let sealed = seal(&self.sealer, record)?;
        let body = serde_json::to_vec(&OnDisk {
            expires_at_ms: record.expires_at_ms,
            sealed,
        })
        .map_err(|_| "the response could not be encoded".to_owned())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| "the stored responses folder can't be made".to_owned())?;
        }
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, body).map_err(|_| "the response can't be written".to_owned())?;
        std::fs::rename(&partial, &path).map_err(|_| "the response can't be written".to_owned())
    }

    fn get(&self, owner: &str, id: &str, now_ms: u64) -> Option<Record> {
        let path = self.path(owner, id)?;
        let text = std::fs::read(&path).ok()?;
        let on_disk: OnDisk = serde_json::from_slice(&text).ok()?;
        if on_disk.expires_at_ms <= now_ms {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        unseal(&self.sealer, owner, id, &on_disk.sealed)
    }

    fn delete(&self, owner: &str, id: &str) -> bool {
        self.path(owner, id)
            .is_some_and(|path| std::fs::remove_file(path).is_ok())
    }

    fn sweep(&self, now_ms: u64) -> usize {
        let mut removed = 0;
        let Ok(owners) = std::fs::read_dir(&self.dir) else {
            return 0;
        };
        for owner in owners.flatten() {
            let Ok(files) = std::fs::read_dir(owner.path()) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let expired = std::fs::read(&path)
                    .ok()
                    .and_then(|text| serde_json::from_slice::<OnDisk>(&text).ok())
                    .is_none_or(|on_disk| on_disk.expires_at_ms <= now_ms);
                if expired && std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{Message, Role};
    use crate::request::CreateResponse;
    use crate::seal::random_id;

    fn record(owner: &str, expires_at_ms: u64) -> Record {
        let id = random_id("resp_");
        let mut response = Response::from_request(id.clone(), 1, "m", &CreateResponse::default());
        response.output = vec![Item::Message(Message::text(Role::Assistant, "the answer"))];
        Record {
            id,
            owner: owner.to_owned(),
            created_at_ms: 1,
            expires_at_ms,
            context: vec![Item::Message(Message::text(
                Role::User,
                "the secret question",
            ))],
            response,
        }
    }

    fn exercise(store: &dyn ResponseStore) {
        let kept = record("acme", 1_000);
        store.put(&kept).unwrap();
        assert_eq!(store.get("acme", &kept.id, 10).unwrap(), kept);
        assert_eq!(
            store.get("acme", &kept.id, 10).unwrap().continued().len(),
            2
        );
        // Another owner reads it as missing.
        assert!(store.get("other", &kept.id, 10).is_none());
        assert!(!store.delete("other", &kept.id));
        // Expired reads as missing.
        assert!(store.get("acme", &kept.id, 1_000).is_none());
        // Delete removes it at once.
        let gone = record("acme", 1_000);
        store.put(&gone).unwrap();
        assert!(store.delete("acme", &gone.id));
        assert!(store.get("acme", &gone.id, 10).is_none());
        assert!(!store.delete("acme", &gone.id));
        // Sweep removes what has expired.
        let late = record("acme", 5_000);
        store.put(&late).unwrap();
        store.put(&kept).unwrap();
        assert_eq!(store.sweep(2_000), 1);
        assert!(store.get("acme", &late.id, 2_000).is_some());
    }

    #[test]
    fn memory_store_is_sealed_owner_scoped_and_expires() {
        let store = MemoryStore::new(Sealer::generate().unwrap().0);
        exercise(&store);
        for blob in store.sealed() {
            assert!(!blob.contains("secret"));
        }
    }

    #[test]
    fn dir_store_is_sealed_owner_scoped_and_expires() {
        let dir = std::env::temp_dir().join(random_id("store-test-"));
        let store = DirStore::open(&dir, Sealer::generate().unwrap().0).unwrap();
        exercise(&store);
        // Nothing on disk reads in the clear, and paths carry no owner.
        let mut files = 0;
        for owner in std::fs::read_dir(&dir).unwrap().flatten() {
            assert!(!owner.file_name().to_string_lossy().contains("acme"));
            for file in std::fs::read_dir(owner.path()).unwrap().flatten() {
                files += 1;
                let text = std::fs::read_to_string(file.path()).unwrap();
                assert!(!text.contains("secret") && !text.contains("answer"));
            }
        }
        assert_eq!(files, 1);
        // Ids that are not ours never become paths.
        assert!(store.get("acme", "../../etc/passwd", 0).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
