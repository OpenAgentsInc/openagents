use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::types::{RunEvent, RunStatus, RuntimeRun};

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeReceipt {
    pub schema: String,
    pub session_id: String,
    pub trajectory_hash: String,
    pub policy_bundle_id: String,
    pub created_at: String,
    pub event_count: usize,
    pub first_seq: u64,
    pub last_seq: u64,
    pub tool_calls: Vec<ToolCallReceipt>,
    pub verification: Vec<VerificationReceipt>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolCallReceipt {
    pub tool: String,
    pub params_hash: String,
    pub output_hash: String,
    pub latency_ms: u64,
    pub side_effects: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerificationReceipt {
    pub command: String,
    pub exit_code: i32,
    pub cwd: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplayHeader {
    #[serde(rename = "type")]
    pub event_type: String,
    pub replay_version: u8,
    pub producer: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionStart {
    #[serde(rename = "type")]
    pub event_type: String,
    pub ts: String,
    pub session_id: String,
    pub policy_bundle_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeReplayEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub ts: String,
    pub session_id: String,
    pub seq: u64,
    pub runtime_event_type: String,
    pub payload_hash: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionEnd {
    #[serde(rename = "type")]
    pub event_type: String,
    pub ts: String,
    pub session_id: String,
    pub status: String,
    pub confidence: f64,
    pub total_tool_calls: usize,
}

pub fn build_receipt(run: &RuntimeRun) -> Result<RuntimeReceipt, ArtifactError> {
    let trajectory_hash = trajectory_hash(&run.events)?;
    let policy_bundle_id = run
        .metadata
        .get("policy_bundle_id")
        .and_then(Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| "runtime.default".to_string());
    let first_seq = run.events.first().map_or(0, |event| event.seq);
    let last_seq = run.events.last().map_or(0, |event| event.seq);

    Ok(RuntimeReceipt {
        schema: "openagents.receipt.v1".to_string(),
        session_id: run.id.to_string(),
        trajectory_hash,
        policy_bundle_id,
        created_at: run.created_at.to_rfc3339(),
        event_count: run.events.len(),
        first_seq,
        last_seq,
        tool_calls: Vec::new(),
        verification: Vec::new(),
    })
}

pub fn build_replay_jsonl(run: &RuntimeRun) -> Result<String, ArtifactError> {
    let mut lines = Vec::new();

    let header = ReplayHeader {
        event_type: "ReplayHeader".to_string(),
        replay_version: 1,
        producer: "openagents-runtime".to_string(),
        created_at: run.created_at.to_rfc3339(),
    };
    lines.push(serde_json::to_string(&header)?);

    let session_start = SessionStart {
        event_type: "SessionStart".to_string(),
        ts: run.created_at.to_rfc3339(),
        session_id: run.id.to_string(),
        policy_bundle_id: run
            .metadata
            .get("policy_bundle_id")
            .and_then(Value::as_str)
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or_else(|| "runtime.default".to_string()),
    };
    lines.push(serde_json::to_string(&session_start)?);

    for event in &run.events {
        let payload_hash = sha256_prefixed(&canonical_json(&event.payload)?);
        let replay_event = RuntimeReplayEvent {
            event_type: "RuntimeEvent".to_string(),
            ts: event.recorded_at.to_rfc3339(),
            session_id: run.id.to_string(),
            seq: event.seq,
            runtime_event_type: event.event_type.clone(),
            payload_hash,
            payload: event.payload.clone(),
        };
        lines.push(serde_json::to_string(&replay_event)?);
    }

    let session_end = SessionEnd {
        event_type: "SessionEnd".to_string(),
        ts: run
            .events
            .last()
            .map_or(run.updated_at.to_rfc3339(), |event| {
                event.recorded_at.to_rfc3339()
            }),
        session_id: run.id.to_string(),
        status: replay_status(&run.status).to_string(),
        confidence: replay_confidence(&run.status),
        total_tool_calls: 0,
    };
    lines.push(serde_json::to_string(&session_end)?);

    Ok(format!("{}\n", lines.join("\n")))
}

pub fn trajectory_hash(events: &[RunEvent]) -> Result<String, ArtifactError> {
    let canonical_events = events
        .iter()
        .map(|event| {
            serde_json::json!({
                "seq": event.seq,
                "event_type": event.event_type,
                "payload": event.payload,
                "idempotency_key": event.idempotency_key,
                "recorded_at": event.recorded_at.to_rfc3339(),
            })
        })
        .collect::<Vec<_>>();
    let canonical = canonical_json(&Value::Array(canonical_events))?;
    Ok(sha256_prefixed(&canonical))
}

fn replay_status(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Succeeded => "success",
        RunStatus::Failed => "failure",
        RunStatus::Canceled => "cancelled",
        _ => "success",
    }
}

fn replay_confidence(status: &RunStatus) -> f64 {
    match status {
        RunStatus::Succeeded => 1.0,
        RunStatus::Failed => 0.0,
        RunStatus::Canceled => 0.0,
        _ => 0.5,
    }
}

fn canonical_json(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(&sort_json_value(value))
}

fn sort_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), sort_json_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.iter().map(sort_json_value).collect()),
        _ => value.clone(),
    }
}

fn sha256_prefixed(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use serde_json::json;

    use super::{build_receipt, build_replay_jsonl, trajectory_hash};
    use crate::types::{RunEvent, RunStatus, RuntimeRun};

    fn sample_run() -> RuntimeRun {
        let now = Utc::now();
        RuntimeRun {
            id: uuid::Uuid::now_v7(),
            worker_id: Some("desktop:artifact-worker".to_string()),
            status: RunStatus::Succeeded,
            metadata: json!({"policy_bundle_id": "policy.v1"}),
            events: vec![
                RunEvent {
                    seq: 1,
                    event_type: "run.started".to_string(),
                    payload: json!({"source": "runtime"}),
                    idempotency_key: None,
                    recorded_at: now,
                },
                RunEvent {
                    seq: 2,
                    event_type: "run.finished".to_string(),
                    payload: json!({"status": "succeeded"}),
                    idempotency_key: Some("finish-key".to_string()),
                    recorded_at: now,
                },
            ],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn receipt_generation_is_deterministic() -> Result<()> {
        let run = sample_run();
        let first = serde_json::to_string(&build_receipt(&run)?)?;
        let second = serde_json::to_string(&build_receipt(&run)?)?;
        if first != second {
            return Err(anyhow!("receipt generation should be deterministic"));
        }
        Ok(())
    }

    #[test]
    fn replay_generation_has_required_sections() -> Result<()> {
        let run = sample_run();
        let replay = build_replay_jsonl(&run)?;
        let lines = replay.lines().collect::<Vec<_>>();
        if lines.len() < 4 {
            return Err(anyhow!("replay should have header/start/events/end"));
        }
        if !lines
            .first()
            .map_or(false, |line| line.contains("\"type\":\"ReplayHeader\""))
        {
            return Err(anyhow!("missing replay header"));
        }
        if !lines
            .last()
            .map_or(false, |line| line.contains("\"type\":\"SessionEnd\""))
        {
            return Err(anyhow!("missing session end"));
        }
        Ok(())
    }

    #[test]
    fn trajectory_hash_changes_when_events_change() -> Result<()> {
        let run = sample_run();
        let first_hash = trajectory_hash(&run.events)?;
        let mut changed = run.clone();
        changed.events[1].payload = json!({"status": "failed"});
        let second_hash = trajectory_hash(&changed.events)?;
        if first_hash == second_hash {
            return Err(anyhow!("trajectory hash should change with event payload"));
        }
        Ok(())
    }
}
