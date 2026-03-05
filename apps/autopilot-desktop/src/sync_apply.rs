use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SYNC_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncApplyPolicy {
    pub stale_clamp_window: u64,
}

impl Default for SyncApplyPolicy {
    fn default() -> Self {
        Self {
            stale_clamp_window: 10_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamApplyDecision {
    Applied {
        stream_id: String,
        seq: u64,
        previous_seq: u64,
    },
    Duplicate {
        stream_id: String,
        seq: u64,
        checkpoint_seq: u64,
    },
    OutOfOrder {
        stream_id: String,
        expected_seq: u64,
        received_seq: u64,
        checkpoint_seq: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncApplyEngine {
    checkpoint_path: PathBuf,
    policy: SyncApplyPolicy,
    checkpoints: BTreeMap<String, u64>,
}

impl SyncApplyEngine {
    #[must_use]
    pub fn default_checkpoint_path() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".openagents")
            .join("autopilot-sync-checkpoints-v1.json")
    }

    pub fn load_or_new_default() -> Result<Self, String> {
        Self::load_or_new(Self::default_checkpoint_path(), SyncApplyPolicy::default())
    }

    pub fn load_or_new(path: PathBuf, policy: SyncApplyPolicy) -> Result<Self, String> {
        let mut engine = Self {
            checkpoint_path: path,
            policy,
            checkpoints: BTreeMap::new(),
        };
        let Some(document) = load_checkpoint_document(engine.checkpoint_path.as_path())? else {
            return Ok(engine);
        };

        if document.schema_version != SYNC_CHECKPOINT_SCHEMA_VERSION {
            return Ok(engine);
        }

        for row in document.streams {
            if row.seq == 0 {
                continue;
            }
            let stream_id = row.stream_id.trim();
            if stream_id.is_empty() {
                continue;
            }
            engine.checkpoints.insert(stream_id.to_string(), row.seq);
        }
        Ok(engine)
    }

    #[must_use]
    pub fn checkpoint_for(&self, stream_id: &str) -> Option<u64> {
        self.checkpoints.get(stream_id).copied()
    }

    #[must_use]
    pub fn max_checkpoint_seq(&self) -> u64 {
        self.checkpoints.values().copied().max().unwrap_or(0)
    }

    #[must_use]
    pub fn resume_cursor_for_stream(&self, stream_id: &str, remote_head_seq: Option<u64>) -> u64 {
        let checkpoint = self.checkpoint_for(stream_id).unwrap_or(0);
        let Some(head) = remote_head_seq else {
            return checkpoint;
        };
        let min_allowed = head.saturating_sub(self.policy.stale_clamp_window);
        checkpoint.max(min_allowed)
    }

    pub fn apply_seq(&mut self, stream_id: &str, seq: u64) -> Result<StreamApplyDecision, String> {
        if seq == 0 {
            return Err("sync apply seq must be > 0".to_string());
        }
        let stream_id = stream_id.trim();
        if stream_id.is_empty() {
            return Err("sync apply stream_id must not be empty".to_string());
        }

        let checkpoint = self.checkpoints.get(stream_id).copied().unwrap_or(0);
        if seq <= checkpoint {
            return Ok(StreamApplyDecision::Duplicate {
                stream_id: stream_id.to_string(),
                seq,
                checkpoint_seq: checkpoint,
            });
        }

        let expected = checkpoint.saturating_add(1);
        if seq != expected {
            return Ok(StreamApplyDecision::OutOfOrder {
                stream_id: stream_id.to_string(),
                expected_seq: expected,
                received_seq: seq,
                checkpoint_seq: checkpoint,
            });
        }

        self.checkpoints.insert(stream_id.to_string(), seq);
        self.persist()?;
        Ok(StreamApplyDecision::Applied {
            stream_id: stream_id.to_string(),
            seq,
            previous_seq: checkpoint,
        })
    }

    pub fn rewind_stream(&mut self, stream_id: &str, seq: u64) -> Result<(), String> {
        let stream_id = stream_id.trim();
        if stream_id.is_empty() {
            return Err("sync rewind stream_id must not be empty".to_string());
        }
        self.checkpoints.insert(stream_id.to_string(), seq);
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .checkpoint_path
            .parent()
            .ok_or_else(|| "checkpoint path has no parent directory".to_string())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create checkpoint directory {}: {error}",
                parent.display()
            )
        })?;

        let document = StreamCheckpointDocument {
            schema_version: SYNC_CHECKPOINT_SCHEMA_VERSION,
            streams: self
                .checkpoints
                .iter()
                .map(|(stream_id, seq)| StreamCheckpointRow {
                    stream_id: stream_id.clone(),
                    seq: *seq,
                })
                .collect(),
        };
        let payload = serde_json::to_string_pretty(&document)
            .map_err(|error| format!("failed to serialize sync checkpoint document: {error}"))?;
        let temp_path = self.checkpoint_path.with_extension("json.tmp");
        fs::write(temp_path.as_path(), payload).map_err(|error| {
            format!(
                "failed to write checkpoint temp file {}: {error}",
                temp_path.display()
            )
        })?;
        fs::rename(temp_path.as_path(), self.checkpoint_path.as_path()).map_err(|error| {
            format!(
                "failed to persist checkpoint file {}: {error}",
                self.checkpoint_path.display()
            )
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct StreamCheckpointDocument {
    schema_version: u32,
    streams: Vec<StreamCheckpointRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct StreamCheckpointRow {
    stream_id: String,
    seq: u64,
}

fn load_checkpoint_document(path: &Path) -> Result<Option<StreamCheckpointDocument>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read checkpoint file {}: {error}", path.display()))?;
    let document =
        serde_json::from_str::<StreamCheckpointDocument>(raw.as_str()).map_err(|error| {
            format!(
                "failed to parse checkpoint file {}: {error}",
                path.display()
            )
        })?;
    Ok(Some(document))
}

#[cfg(test)]
mod tests {
    use super::{StreamApplyDecision, SyncApplyEngine, SyncApplyPolicy};

    fn unique_temp_checkpoint_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!("openagents-sync-checkpoint-{name}-{nanos}.json"))
    }

    #[test]
    fn apply_seq_accepts_in_order_and_drops_duplicates() {
        let path = unique_temp_checkpoint_path("duplicates");
        let mut engine = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("engine should initialize");

        let first = engine
            .apply_seq("runtime.command", 1)
            .expect("first seq should apply");
        assert!(matches!(first, StreamApplyDecision::Applied { .. }));

        let duplicate = engine
            .apply_seq("runtime.command", 1)
            .expect("duplicate seq should evaluate");
        assert!(matches!(
            duplicate,
            StreamApplyDecision::Duplicate {
                checkpoint_seq: 1,
                ..
            }
        ));
    }

    #[test]
    fn out_of_order_seq_requests_rebootstrap() {
        let path = unique_temp_checkpoint_path("out-of-order");
        let mut engine = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("engine should initialize");
        let _ = engine
            .apply_seq("runtime.command", 1)
            .expect("seed seq should apply");

        let decision = engine
            .apply_seq("runtime.command", 3)
            .expect("out-of-order seq should evaluate");
        assert!(matches!(
            decision,
            StreamApplyDecision::OutOfOrder {
                expected_seq: 2,
                received_seq: 3,
                checkpoint_seq: 1,
                ..
            }
        ));
        assert_eq!(engine.checkpoint_for("runtime.command"), Some(1));
    }

    #[test]
    fn checkpoints_persist_and_reload_across_restart() {
        let path = unique_temp_checkpoint_path("persist");
        let mut writer = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("writer should initialize");
        let _ = writer
            .apply_seq("runtime.command", 1)
            .expect("first seq should apply");
        let _ = writer
            .apply_seq("runtime.command", 2)
            .expect("second seq should apply");
        let _ = writer
            .apply_seq("codex.command", 1)
            .expect("codex seq should apply");

        let reader = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("reader should initialize");
        assert_eq!(reader.checkpoint_for("runtime.command"), Some(2));
        assert_eq!(reader.checkpoint_for("codex.command"), Some(1));
        assert_eq!(reader.max_checkpoint_seq(), 2);
    }

    #[test]
    fn resume_cursor_clamps_stale_checkpoint_to_recent_window() {
        let path = unique_temp_checkpoint_path("stale-clamp");
        let mut engine = SyncApplyEngine::load_or_new(
            path.clone(),
            SyncApplyPolicy {
                stale_clamp_window: 5,
            },
        )
        .expect("engine should initialize");
        let _ = engine
            .apply_seq("runtime.command", 1)
            .expect("first seq should apply");

        let clamped = engine.resume_cursor_for_stream("runtime.command", Some(20));
        assert_eq!(clamped, 15);
    }

    #[test]
    fn rewind_stream_persists_recovery_checkpoint() {
        let path = unique_temp_checkpoint_path("rewind");
        let mut engine = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("engine should initialize");
        let _ = engine
            .apply_seq("runtime.command", 1)
            .expect("first seq should apply");
        let _ = engine
            .apply_seq("runtime.command", 2)
            .expect("second seq should apply");
        engine
            .rewind_stream("runtime.command", 1)
            .expect("rewind should persist");

        let restored = SyncApplyEngine::load_or_new(path.clone(), SyncApplyPolicy::default())
            .expect("restored engine should initialize");
        assert_eq!(restored.checkpoint_for("runtime.command"), Some(1));
    }
}
