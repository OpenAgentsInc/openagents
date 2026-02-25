use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

const CHECKPOINT_SCHEMA_VERSION: u32 = 1;
const CHECKPOINT_FILE_NAME: &str = "runtime-sync-checkpoints.v1.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeCursorSource {
    LocalCheckpoint,
    RemoteLatest,
    ClampedToRemoteHead,
    FallbackZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeCursorDecision {
    pub cursor: u64,
    pub source: ResumeCursorSource,
}

#[must_use]
pub fn resolve_resume_cursor(
    local: Option<u64>,
    remote_latest: Option<u64>,
) -> ResumeCursorDecision {
    match (local, remote_latest) {
        (Some(local_cursor), Some(remote_head)) if local_cursor <= remote_head => {
            ResumeCursorDecision {
                cursor: local_cursor,
                source: ResumeCursorSource::LocalCheckpoint,
            }
        }
        (Some(local_cursor), Some(remote_head)) if local_cursor > remote_head => {
            ResumeCursorDecision {
                cursor: remote_head,
                source: ResumeCursorSource::ClampedToRemoteHead,
            }
        }
        (Some(local_cursor), None) => ResumeCursorDecision {
            cursor: local_cursor,
            source: ResumeCursorSource::LocalCheckpoint,
        },
        (None, Some(remote_head)) => ResumeCursorDecision {
            cursor: remote_head,
            source: ResumeCursorSource::RemoteLatest,
        },
        (None, None) => ResumeCursorDecision {
            cursor: 0,
            source: ResumeCursorSource::FallbackZero,
        },
        (Some(_), Some(_)) => ResumeCursorDecision {
            cursor: 0,
            source: ResumeCursorSource::FallbackZero,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSyncCheckpointEntry {
    pub worker_id: String,
    pub stream_id: String,
    pub watermark: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeSyncCheckpointDocument {
    version: u32,
    entries: Vec<RuntimeSyncCheckpointEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSyncCheckpointStore {
    path: PathBuf,
    entries: HashMap<(String, String), RuntimeSyncCheckpointEntry>,
}

impl RuntimeSyncCheckpointStore {
    pub fn load_default() -> Self {
        let path = default_checkpoint_path();
        Self::load(path)
    }

    pub fn load(path: PathBuf) -> Self {
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => {
                return Self {
                    path,
                    entries: HashMap::new(),
                };
            }
        };
        let parsed = serde_json::from_str::<RuntimeSyncCheckpointDocument>(raw.as_str());
        let mut entries = HashMap::new();
        if let Ok(document) = parsed
            && document.version == CHECKPOINT_SCHEMA_VERSION
        {
            for entry in document.entries {
                entries.insert((entry.worker_id.clone(), entry.stream_id.clone()), entry);
            }
        }
        Self { path, entries }
    }

    #[must_use]
    pub fn watermark(&self, worker_id: &str, stream_id: &str) -> Option<u64> {
        self.entries
            .get(&(worker_id.to_string(), stream_id.to_string()))
            .map(|entry| entry.watermark)
    }

    pub fn upsert(
        &mut self,
        worker_id: &str,
        stream_id: &str,
        watermark: u64,
    ) -> Result<(), String> {
        let key = (worker_id.to_string(), stream_id.to_string());
        let existing = self
            .entries
            .get(&key)
            .map(|entry| entry.watermark)
            .unwrap_or(0);
        let next_watermark = existing.max(watermark);
        self.entries.insert(
            key,
            RuntimeSyncCheckpointEntry {
                worker_id: worker_id.to_string(),
                stream_id: stream_id.to_string(),
                watermark: next_watermark,
                updated_at: Utc::now().to_rfc3339(),
            },
        );
        self.flush()
    }

    pub fn rewind(
        &mut self,
        worker_id: &str,
        stream_id: &str,
        watermark: u64,
    ) -> Result<(), String> {
        self.entries.insert(
            (worker_id.to_string(), stream_id.to_string()),
            RuntimeSyncCheckpointEntry {
                worker_id: worker_id.to_string(),
                stream_id: stream_id.to_string(),
                watermark,
                updated_at: Utc::now().to_rfc3339(),
            },
        );
        self.flush()
    }

    fn flush(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("checkpoint mkdir failed: {error}"))?;
        }
        let mut entries = self.entries.values().cloned().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.worker_id
                .cmp(&right.worker_id)
                .then(left.stream_id.cmp(&right.stream_id))
        });
        let encoded = serde_json::to_string_pretty(&RuntimeSyncCheckpointDocument {
            version: CHECKPOINT_SCHEMA_VERSION,
            entries,
        })
        .map_err(|error| format!("checkpoint encode failed: {error}"))?;
        fs::write(&self.path, encoded).map_err(|error| format!("checkpoint write failed: {error}"))
    }
}

fn default_checkpoint_path() -> PathBuf {
    if let Some(mut data_dir) = dirs::data_local_dir() {
        data_dir.push("openagents");
        data_dir.push(CHECKPOINT_FILE_NAME);
        return data_dir;
    }

    if let Some(mut home_dir) = dirs::home_dir() {
        home_dir.push(".openagents");
        home_dir.push(CHECKPOINT_FILE_NAME);
        return home_dir;
    }

    PathBuf::from(CHECKPOINT_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::{
        ResumeCursorSource, RuntimeSyncCheckpointDocument, RuntimeSyncCheckpointEntry,
        RuntimeSyncCheckpointStore, resolve_resume_cursor,
    };

    #[test]
    fn checkpoint_store_persists_and_recovers_watermark() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("runtime-sync-checkpoints.v1.json");
        let mut store = RuntimeSyncCheckpointStore::load(path.clone());
        store
            .upsert("worker-1", "runtime.codex.worker.events.worker-1", 41)
            .expect("checkpoint write");
        store
            .upsert("worker-1", "runtime.codex.worker.events.worker-1", 39)
            .expect("checkpoint write monotonic");
        assert_eq!(
            store.watermark("worker-1", "runtime.codex.worker.events.worker-1"),
            Some(41)
        );

        let recovered = RuntimeSyncCheckpointStore::load(path);
        assert_eq!(
            recovered.watermark("worker-1", "runtime.codex.worker.events.worker-1"),
            Some(41)
        );
    }

    #[test]
    fn checkpoint_store_recovers_as_empty_on_corrupt_payload() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("runtime-sync-checkpoints.v1.json");
        std::fs::write(&path, "not json").expect("write corrupt file");
        let recovered = RuntimeSyncCheckpointStore::load(path);
        assert!(
            recovered
                .watermark("worker-x", "runtime.codex.worker.events.worker-x")
                .is_none()
        );
    }

    #[test]
    fn resume_cursor_resolution_handles_local_remote_and_stale_cases() {
        let local = resolve_resume_cursor(Some(12), Some(50));
        assert_eq!(local.cursor, 12);
        assert_eq!(local.source, ResumeCursorSource::LocalCheckpoint);

        let remote = resolve_resume_cursor(None, Some(50));
        assert_eq!(remote.cursor, 50);
        assert_eq!(remote.source, ResumeCursorSource::RemoteLatest);

        let stale = resolve_resume_cursor(Some(90), Some(50));
        assert_eq!(stale.cursor, 50);
        assert_eq!(stale.source, ResumeCursorSource::ClampedToRemoteHead);

        let fallback = resolve_resume_cursor(None, None);
        assert_eq!(fallback.cursor, 0);
        assert_eq!(fallback.source, ResumeCursorSource::FallbackZero);
    }

    #[test]
    fn checkpoint_store_rewind_supports_rebootstrap() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("runtime-sync-checkpoints.v1.json");
        let mut store = RuntimeSyncCheckpointStore::load(path);
        store
            .upsert("worker-2", "runtime.codex.worker.events.worker-2", 33)
            .expect("checkpoint write");
        store
            .rewind("worker-2", "runtime.codex.worker.events.worker-2", 7)
            .expect("checkpoint rewind");
        assert_eq!(
            store.watermark("worker-2", "runtime.codex.worker.events.worker-2"),
            Some(7)
        );
    }

    #[test]
    fn checkpoint_document_contract_shape_roundtrips() {
        let document = RuntimeSyncCheckpointDocument {
            version: 1,
            entries: vec![RuntimeSyncCheckpointEntry {
                worker_id: "worker-a".to_string(),
                stream_id: "runtime.codex.worker.events.worker-a".to_string(),
                watermark: 77,
                updated_at: "2026-02-25T00:00:00Z".to_string(),
            }],
        };
        let encoded = serde_json::to_string(&document).expect("encode doc");
        let decoded =
            serde_json::from_str::<RuntimeSyncCheckpointDocument>(&encoded).expect("decode doc");
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].watermark, 77);
    }
}
