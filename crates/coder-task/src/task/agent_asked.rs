//! The requests an agent admitted, kept on disk by request ID and the exact
//! digest of what was asked (#10955).
//!
//! A device that lost a reply, or a host that started again, sees the same
//! request ID once more. The same ID with the same request bytes answers as
//! the original admission and starts nothing; the same ID with different
//! bytes is a conflict. The ledger is written and synced before the request
//! joins the queue, so no admitted request can run twice for one ID.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "openagents.agent.asked.v1";
const FILE: &str = "asked.json";
/// The largest ledger file read back.
const MAX_BYTES: u64 = 256 * 1024;

/// What a ledger lookup found for one request ID.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Seen {
    /// The ID is new.
    New,
    /// The ID was admitted with these exact bytes.
    Same,
    /// The ID was admitted with other bytes.
    Changed,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: String,
    /// `(sha256 of the request ID, sha256 of the request)`, oldest first.
    asked: VecDeque<(String, String)>,
}

/// One agent's ledger, in her directory.
pub(crate) struct Asked {
    path: PathBuf,
    limit: usize,
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl Asked {
    pub(crate) fn new(dir: &Path, limit: usize) -> Self {
        Self {
            path: dir.join(FILE),
            limit,
        }
    }

    fn load(&self) -> Result<Ledger, String> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Ledger {
                    schema: SCHEMA.into(),
                    asked: VecDeque::new(),
                });
            }
            Err(e) => return Err(format!("cannot read {}: {e}", self.path.display())),
        };
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(file, MAX_BYTES + 1), &mut bytes)
            .map_err(|e| format!("cannot read {}: {e}", self.path.display()))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(format!("{} exceeds its bound", self.path.display()));
        }
        let ledger: Ledger = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{} is not a request ledger: {e}", self.path.display()))?;
        if ledger.schema != SCHEMA {
            return Err(format!(
                "{} is a ledger this host does not read",
                self.path.display()
            ));
        }
        Ok(ledger)
    }

    /// Whether `key` was admitted, and with which bytes.
    pub(crate) fn seen(&self, key: &str, digest: &str) -> Result<Seen, String> {
        let key = hex(key.as_bytes());
        Ok(
            match self.load()?.asked.iter().find(|(known, _)| *known == key) {
                None => Seen::New,
                Some((_, known)) if known == digest => Seen::Same,
                Some(_) => Seen::Changed,
            },
        )
    }

    /// Record `key` with `digest`, synced before the caller queues it.
    pub(crate) fn record(&self, key: &str, digest: &str) -> Result<(), String> {
        let mut ledger = self.load()?;
        ledger
            .asked
            .push_back((hex(key.as_bytes()), digest.to_owned()));
        while ledger.asked.len() > self.limit {
            ledger.asked.pop_front();
        }
        let body = serde_json::to_vec(&ledger).map_err(|e| e.to_string())?;
        let dir = self.path.parent().ok_or("the ledger has no directory")?;
        let temp = dir.join(format!(".{FILE}.tmp"));
        super::agent::write_private(&temp, &body)?;
        std::fs::rename(&temp, &self.path)
            .map_err(|e| format!("cannot write {}: {e}", self.path.display()))?;
        if let Ok(dir) = std::fs::File::open(dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

/// The digest of a request's exact admitted content.
pub(crate) fn digest(value: &serde_json::Value) -> String {
    hex(value.to_string().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_replay_and_changed_bytes_conflict_after_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let asked = Asked::new(dir.path(), 2);
        let one = digest(&serde_json::json!({"text":"one"}));
        let two = digest(&serde_json::json!({"text":"two"}));
        assert_eq!(asked.seen("k1", &one).unwrap(), Seen::New);
        asked.record("k1", &one).unwrap();
        let reopened = Asked::new(dir.path(), 2);
        assert_eq!(reopened.seen("k1", &one).unwrap(), Seen::Same);
        assert_eq!(reopened.seen("k1", &two).unwrap(), Seen::Changed);
        reopened.record("k2", &two).unwrap();
        reopened.record("k3", &two).unwrap();
        assert_eq!(reopened.seen("k1", &one).unwrap(), Seen::New, "bounded");
        std::fs::write(dir.path().join(FILE), b"{").unwrap();
        assert!(
            reopened.seen("k1", &one).is_err(),
            "an unreadable ledger fails closed"
        );
    }
}
