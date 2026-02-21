use std::{
    collections::{BTreeMap, HashMap},
    fs::{File, OpenOptions, create_dir_all},
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::Arc,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::types::RunEvent;

#[derive(Clone)]
pub struct DurableEventLog {
    inner: Arc<Mutex<DurableEventLogState>>,
}

#[derive(Debug, Error)]
pub enum EventLogError {
    #[error("event log I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("event log serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("run sequence overflow for run {0}")]
    SequenceOverflow(Uuid),
    #[error(
        "run sequence conflict for run {run_id}: expected previous seq {expected_previous_seq}, actual previous seq {actual_previous_seq}"
    )]
    SequenceConflict {
        run_id: Uuid,
        expected_previous_seq: u64,
        actual_previous_seq: u64,
    },
}

#[derive(Clone, Debug)]
pub struct EventLogAppendRequest {
    pub run_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub idempotency_key: Option<String>,
    pub expected_previous_seq: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct EventLogAppendOutcome {
    pub event: RunEvent,
    pub idempotent_replay: bool,
}

#[derive(Debug)]
struct DurableEventLogState {
    sink: EventLogSink,
    latest_seq: HashMap<Uuid, u64>,
    events_by_run: HashMap<Uuid, BTreeMap<u64, RunEvent>>,
    idempotency_index: HashMap<(Uuid, String), RunEvent>,
}

#[derive(Debug)]
enum EventLogSink {
    Durable { file: File },
    Memory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedRunEvent {
    run_id: Uuid,
    seq: u64,
    event_type: String,
    payload: serde_json::Value,
    idempotency_key: Option<String>,
    recorded_at: chrono::DateTime<Utc>,
}

impl DurableEventLog {
    #[must_use]
    pub fn new_memory() -> Self {
        let state = DurableEventLogState {
            sink: EventLogSink::Memory,
            latest_seq: HashMap::new(),
            events_by_run: HashMap::new(),
            idempotency_index: HashMap::new(),
        };
        Self {
            inner: Arc::new(Mutex::new(state)),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, EventLogError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }

        let append_file = OpenOptions::new().append(true).create(true).open(path)?;
        let read_file = OpenOptions::new().read(true).open(path)?;
        let mut state = DurableEventLogState {
            sink: EventLogSink::Durable { file: append_file },
            latest_seq: HashMap::new(),
            events_by_run: HashMap::new(),
            idempotency_index: HashMap::new(),
        };

        let reader = BufReader::new(read_file);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let persisted: PersistedRunEvent = serde_json::from_str(&line)?;
            let event = RunEvent {
                seq: persisted.seq,
                event_type: persisted.event_type,
                payload: persisted.payload,
                idempotency_key: persisted.idempotency_key.clone(),
                recorded_at: persisted.recorded_at,
            };
            state.record_event(persisted.run_id, event);
        }

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
        })
    }

    pub fn open_default() -> Result<Self, EventLogError> {
        let configured = std::env::var("RUNTIME_EVENT_LOG_PATH")
            .unwrap_or_else(|_| ".runtime-data/runtime-events.jsonl".to_string());
        Self::open(configured)
    }

    pub async fn append(
        &self,
        request: EventLogAppendRequest,
    ) -> Result<EventLogAppendOutcome, EventLogError> {
        let mut state = self.inner.lock().await;

        let normalized_key = request.idempotency_key.as_deref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        if let Some(key) = normalized_key.clone()
            && let Some(existing) = state.idempotency_index.get(&(request.run_id, key))
        {
            return Ok(EventLogAppendOutcome {
                event: existing.clone(),
                idempotent_replay: true,
            });
        }

        let current_latest = state.latest_seq.get(&request.run_id).copied().unwrap_or(0);
        if let Some(expected_previous_seq) = request.expected_previous_seq
            && expected_previous_seq != current_latest
        {
            return Err(EventLogError::SequenceConflict {
                run_id: request.run_id,
                expected_previous_seq,
                actual_previous_seq: current_latest,
            });
        }

        let next_seq = current_latest
            .checked_add(1)
            .ok_or(EventLogError::SequenceOverflow(request.run_id))?;
        let event = RunEvent {
            seq: next_seq,
            event_type: request.event_type,
            payload: request.payload,
            idempotency_key: normalized_key.clone(),
            recorded_at: Utc::now(),
        };

        if let EventLogSink::Durable { file } = &mut state.sink {
            let persisted = PersistedRunEvent {
                run_id: request.run_id,
                seq: event.seq,
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                idempotency_key: event.idempotency_key.clone(),
                recorded_at: event.recorded_at,
            };
            let encoded = serde_json::to_string(&persisted)?;
            file.write_all(encoded.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_data()?;
        }

        state.record_event(request.run_id, event.clone());

        Ok(EventLogAppendOutcome {
            event,
            idempotent_replay: false,
        })
    }

    pub async fn events_for_run(&self, run_id: Uuid) -> Vec<RunEvent> {
        let state = self.inner.lock().await;
        state
            .events_by_run
            .get(&run_id)
            .map_or_else(Vec::new, |entries| entries.values().cloned().collect())
    }
}

impl DurableEventLogState {
    fn record_event(&mut self, run_id: Uuid, event: RunEvent) {
        self.latest_seq.insert(run_id, event.seq);
        if let Some(key) = event.idempotency_key.clone() {
            self.idempotency_index.insert((run_id, key), event.clone());
        }
        self.events_by_run
            .entry(run_id)
            .or_default()
            .insert(event.seq, event);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::{Result, anyhow};
    use serde_json::json;
    use tempfile::tempdir;

    use super::{DurableEventLog, EventLogAppendRequest, EventLogError};

    #[tokio::test]
    async fn duplicate_idempotency_key_returns_existing_event() -> Result<()> {
        let dir = tempdir()?;
        let log = Arc::new(DurableEventLog::open(dir.path().join("events.jsonl"))?);
        let run_id = uuid::Uuid::now_v7();

        let mut tasks = Vec::new();
        for _ in 0..16 {
            let log = Arc::clone(&log);
            tasks.push(tokio::spawn(async move {
                log.append(EventLogAppendRequest {
                    run_id,
                    event_type: "run.step.completed".to_string(),
                    payload: json!({"step": 1}),
                    idempotency_key: Some("dup-key".to_string()),
                    expected_previous_seq: Some(0),
                })
                .await
            }));
        }

        let mut seqs = Vec::new();
        let mut replay_count = 0_u64;
        for task in tasks {
            let outcome = task.await.map_err(|err| anyhow!(err.to_string()))??;
            seqs.push(outcome.event.seq);
            if outcome.idempotent_replay {
                replay_count = replay_count.saturating_add(1);
            }
        }
        if seqs.iter().any(|seq| *seq != 1) {
            return Err(anyhow!("duplicate idempotency writes should return seq=1"));
        }
        if replay_count == 0 {
            return Err(anyhow!("expected idempotent replay outcomes"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn concurrent_unique_writes_remain_contiguous() -> Result<()> {
        let dir = tempdir()?;
        let log = Arc::new(DurableEventLog::open(dir.path().join("events.jsonl"))?);
        let run_id = uuid::Uuid::now_v7();
        let mut tasks = Vec::new();

        for step in 0..32 {
            let log = Arc::clone(&log);
            tasks.push(tokio::spawn(async move {
                log.append(EventLogAppendRequest {
                    run_id,
                    event_type: "run.step.completed".to_string(),
                    payload: json!({"step": step}),
                    idempotency_key: Some(format!("step-{step}")),
                    expected_previous_seq: None,
                })
                .await
            }));
        }

        for task in tasks {
            let _ = task.await.map_err(|err| anyhow!(err.to_string()))??;
        }

        let events = log.events_for_run(run_id).await;
        if events.len() != 32 {
            return Err(anyhow!("expected 32 events, got {}", events.len()));
        }
        let last_seq = events.last().map(|event| event.seq).unwrap_or(0);
        if last_seq != 32 {
            return Err(anyhow!("expected last seq 32, got {last_seq}"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn sequence_conflict_is_reported() -> Result<()> {
        let dir = tempdir()?;
        let log = DurableEventLog::open(dir.path().join("events.jsonl"))?;
        let run_id = uuid::Uuid::now_v7();

        let _ = log
            .append(EventLogAppendRequest {
                run_id,
                event_type: "run.started".to_string(),
                payload: json!({}),
                idempotency_key: None,
                expected_previous_seq: Some(0),
            })
            .await?;

        let conflict = log
            .append(EventLogAppendRequest {
                run_id,
                event_type: "run.step.completed".to_string(),
                payload: json!({"step": 1}),
                idempotency_key: None,
                expected_previous_seq: Some(0),
            })
            .await;
        if !matches!(
            conflict,
            Err(EventLogError::SequenceConflict {
                expected_previous_seq: 0,
                actual_previous_seq: 1,
                ..
            })
        ) {
            return Err(anyhow!("expected sequence conflict"));
        }

        Ok(())
    }
}
