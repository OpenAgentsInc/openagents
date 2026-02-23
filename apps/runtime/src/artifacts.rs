use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::types::{RunEvent, RunStatus, RuntimeRun};

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid tool receipt payload: {0}")]
    InvalidToolReceipt(String),
    #[error("invalid verification receipt payload: {0}")]
    InvalidVerificationReceipt(String),
    #[error("invalid payment receipt payload: {0}")]
    InvalidPaymentReceipt(String),
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
    pub metrics: ReceiptMetrics,
    pub tool_calls: Vec<ToolCallReceipt>,
    pub verification: Vec<VerificationReceipt>,
    pub payments: Vec<PaymentReceipt>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReceiptMetrics {
    /// Total tool calls recorded in this session.
    pub tool_calls: usize,
    /// Sum of tool latency (ms) across tool calls.
    pub tool_latency_ms_total: u64,
    /// Verification pass/fail if any verification records exist.
    pub verification_passed: Option<bool>,
    /// Total payment amount (msats) across payment records.
    pub payments_msats_total: u64,
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
pub struct PaymentReceipt {
    pub rail: String,
    pub asset_id: String,
    pub amount_msats: u64,
    pub payment_proof: Value,
    pub session_id: String,
    pub trajectory_hash: String,
    pub policy_bundle_id: String,
    pub job_hash: Option<String>,
    pub status: String,
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
    let session_id = run.id.to_string();
    let policy_bundle_id = run
        .metadata
        .get("policy_bundle_id")
        .and_then(Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| "runtime.default".to_string());
    let first_seq = run.events.first().map_or(0, |event| event.seq);
    let last_seq = run.events.last().map_or(0, |event| event.seq);
    let tool_calls = extract_tool_receipts(&run.events)?;
    let verification = extract_verification_receipts(&run.events)?;
    let payments = extract_payment_receipts(
        &run.events,
        &trajectory_hash,
        &policy_bundle_id,
        &session_id,
    )?;
    let metrics = ReceiptMetrics {
        tool_calls: tool_calls.len(),
        tool_latency_ms_total: tool_calls.iter().map(|call| call.latency_ms).sum(),
        verification_passed: if verification.is_empty() {
            None
        } else {
            Some(verification.iter().all(|entry| entry.exit_code == 0))
        },
        payments_msats_total: payments.iter().map(|payment| payment.amount_msats).sum(),
    };

    Ok(RuntimeReceipt {
        schema: "openagents.receipt.v1".to_string(),
        session_id,
        trajectory_hash,
        policy_bundle_id,
        created_at: run.created_at.to_rfc3339(),
        event_count: run.events.len(),
        first_seq,
        last_seq,
        metrics,
        tool_calls,
        verification,
        payments,
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

fn extract_tool_receipts(events: &[RunEvent]) -> Result<Vec<ToolCallReceipt>, ArtifactError> {
    let mut receipts = Vec::new();
    for event in events {
        if event.event_type != "tool" {
            continue;
        }
        let payload = event.payload.as_object().ok_or_else(|| {
            ArtifactError::InvalidToolReceipt("payload must be object".to_string())
        })?;
        let tool = payload
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::InvalidToolReceipt("missing tool".to_string()))?
            .to_string();
        let params_hash = payload
            .get("params_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::InvalidToolReceipt("missing params_hash".to_string()))?
            .to_string();
        let output_hash = payload
            .get("output_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::InvalidToolReceipt("missing output_hash".to_string()))?
            .to_string();
        let latency_ms = payload
            .get("latency_ms")
            .and_then(Value::as_u64)
            .ok_or_else(|| ArtifactError::InvalidToolReceipt("missing latency_ms".to_string()))?;
        let side_effects = payload
            .get("side_effects")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        receipts.push(ToolCallReceipt {
            tool,
            params_hash,
            output_hash,
            latency_ms,
            side_effects,
        });
    }
    Ok(receipts)
}

fn extract_verification_receipts(
    events: &[RunEvent],
) -> Result<Vec<VerificationReceipt>, ArtifactError> {
    let mut receipts = Vec::new();
    for event in events {
        if event.event_type != "verification" {
            continue;
        }
        let payload = event.payload.as_object().ok_or_else(|| {
            ArtifactError::InvalidVerificationReceipt("payload must be object".to_string())
        })?;
        let command = payload
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ArtifactError::InvalidVerificationReceipt("missing command".to_string())
            })?
            .to_string();
        let exit_code = payload
            .get("exit_code")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                ArtifactError::InvalidVerificationReceipt("missing exit_code".to_string())
            })?
            .try_into()
            .map_err(|_| {
                ArtifactError::InvalidVerificationReceipt("exit_code out of range".to_string())
            })?;
        let cwd = payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
        let duration_ms = payload.get("duration_ms").and_then(Value::as_u64);
        receipts.push(VerificationReceipt {
            command,
            exit_code,
            cwd,
            duration_ms,
        });
    }
    Ok(receipts)
}

fn extract_payment_receipts(
    events: &[RunEvent],
    trajectory_hash: &str,
    policy_bundle_id: &str,
    session_id: &str,
) -> Result<Vec<PaymentReceipt>, ArtifactError> {
    let mut receipts = Vec::new();
    for event in events {
        if event.event_type != "payment" {
            continue;
        }
        let payload = event.payload.as_object().ok_or_else(|| {
            ArtifactError::InvalidPaymentReceipt("payload must be object".to_string())
        })?;

        let rail = payload
            .get("rail")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::InvalidPaymentReceipt("missing rail".to_string()))?
            .to_string();
        let asset_id = payload
            .get("asset_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::InvalidPaymentReceipt("missing asset_id".to_string()))?
            .to_string();
        let amount_msats = payload
            .get("amount_msats")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ArtifactError::InvalidPaymentReceipt("missing amount_msats".to_string())
            })?;
        let payment_proof = payload.get("payment_proof").cloned().ok_or_else(|| {
            ArtifactError::InvalidPaymentReceipt("missing payment_proof".to_string())
        })?;
        let job_hash = payload
            .get("job_hash")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
        let status = payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("released")
            .to_string();

        receipts.push(PaymentReceipt {
            rail,
            asset_id,
            amount_msats,
            payment_proof,
            session_id: session_id.to_string(),
            trajectory_hash: trajectory_hash.to_string(),
            policy_bundle_id: policy_bundle_id.to_string(),
            job_hash,
            status,
        });
    }
    Ok(receipts)
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
