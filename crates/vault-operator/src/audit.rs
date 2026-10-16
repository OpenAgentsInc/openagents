//! The append-only key-use audit log. Each entry carries the digest of its
//! predecessor, is written and synced before the action it records, and
//! holds identities only: never content, plaintext digests, or key bytes.
use crate::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Write, path::PathBuf};

pub struct Audit {
    path: PathBuf,
    last: String,
    seq: u64,
    /// Fault: the next append fails, as a full disk or a removed file would.
    pub fail_next: bool,
}

fn digest(entry: &Value) -> Result<String> {
    let bytes = nostr::contracts::jcs(entry).map_err(|e| e.to_string())?;
    Ok(format!("sha256:{}", oa_vault::hex(&Sha256::digest(&bytes))))
}

impl Audit {
    /// Open the log and verify its whole chain first.
    pub fn open(path: PathBuf) -> Result<Self> {
        let (last, seq) = Self::verify_file(&path)?;
        Ok(Self {
            path,
            last,
            seq,
            fail_next: false,
        })
    }

    fn verify_file(path: &PathBuf) -> Result<(String, u64)> {
        let mut last = "sha256:genesis".to_string();
        let mut seq = 0;
        if let Ok(text) = std::fs::read_to_string(path) {
            for line in text.lines() {
                let row: Value =
                    serde_json::from_str(line).map_err(|_| "conflict: the audit log is damaged")?;
                let entry = &row["entry"];
                if entry["seq"] != seq
                    || entry["previous"] != last.as_str()
                    || row["digest"] != digest(entry)?.as_str()
                {
                    return Err("conflict: the audit chain is broken".into());
                }
                last = row["digest"].as_str().unwrap_or_default().into();
                seq += 1;
            }
        }
        Ok((last, seq))
    }

    /// Re-read and verify the chain on disk.
    pub fn verify(&self) -> Result<u64> {
        Self::verify_file(&self.path).map(|(_, seq)| seq)
    }

    /// Durably append one entry; the action it records happens only after.
    pub fn append(&mut self, kind: &str, fields: Value, at: u64) -> Result<String> {
        if std::mem::take(&mut self.fail_next) {
            return Err("unavailable: the audit log could not be written".into());
        }
        let mut entry = json!({"seq":self.seq,"previous":self.last,"at":at,"kind":kind});
        if let (Some(object), Some(extra)) = (entry.as_object_mut(), fields.as_object()) {
            for (k, v) in extra {
                object.insert(k.clone(), v.clone());
            }
        }
        let digest = digest(&entry)?;
        let line = serde_json::to_string(&json!({"entry":entry,"digest":digest}))
            .map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| "unavailable: the audit log could not be written")?;
        file.write_all(format!("{line}\n").as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|_| "unavailable: the audit log could not be written")?;
        self.last = digest.clone();
        self.seq += 1;
        Ok(digest)
    }

    /// Every entry, for review.
    pub fn entries(&self) -> Result<Vec<Value>> {
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        text.lines()
            .map(|l| {
                serde_json::from_str::<Value>(l)
                    .map(|r| r["entry"].clone())
                    .map_err(|e| e.to_string())
            })
            .collect()
    }
}
