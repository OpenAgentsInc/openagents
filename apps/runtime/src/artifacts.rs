use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::types::{RunEvent, RunStatus, RuntimeRun};

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid replay jsonl: {0}")]
    InvalidReplay(String),
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
#[serde(tag = "event")]
pub enum ReplayEvent {
    ReplayHeader {
        replay_version: u8,
        producer: String,
        created_at: String,
    },
    SessionStart {
        t: String,
        session_id: String,
        issue_number: Option<i64>,
        policy_bundle_id: String,
    },
    ToolCall {
        t: String,
        id: String,
        tool: String,
        params: Value,
        params_hash: String,
        step_id: String,
    },
    ToolResult {
        t: String,
        id: String,
        output_hash: String,
        exit_code: Option<i32>,
        step_utility: f32,
        latency_ms: u64,
    },
    Verification {
        t: String,
        commands: Vec<String>,
        exit_codes: Vec<i32>,
        verification_delta: i32,
    },
    Payment {
        t: String,
        rail: String,
        asset_id: String,
        amount_msats: u64,
        payment_proof: Value,
        job_hash: Option<String>,
        status: String,
    },
    RuntimeEvent {
        t: String,
        session_id: String,
        seq: u64,
        runtime_event_type: String,
        payload_hash: String,
        payload: Value,
        idempotency_key: Option<String>,
    },
    SessionEnd {
        t: String,
        status: String,
        confidence: f64,
        total_tool_calls: usize,
        total_latency_ms: u64,
    },
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

fn push_replay_line(lines: &mut Vec<String>, event: &ReplayEvent) -> Result<(), ArtifactError> {
    let value = serde_json::to_value(event)?;
    let canonical = canonical_json(&value)?;
    lines.push(canonical);
    Ok(())
}

pub fn build_replay_jsonl(run: &RuntimeRun) -> Result<String, ArtifactError> {
    let mut lines = Vec::new();

    let session_id = run.id.to_string();
    let policy_bundle_id = run
        .metadata
        .get("policy_bundle_id")
        .and_then(Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| "runtime.default".to_string());

    push_replay_line(
        &mut lines,
        &ReplayEvent::ReplayHeader {
            replay_version: 1,
            producer: "openagents-runtime".to_string(),
            created_at: run.created_at.to_rfc3339(),
        },
    )?;

    push_replay_line(
        &mut lines,
        &ReplayEvent::SessionStart {
            t: run.created_at.to_rfc3339(),
            session_id: session_id.clone(),
            issue_number: None,
            policy_bundle_id: policy_bundle_id.clone(),
        },
    )?;

    let mut total_tool_calls = 0usize;
    let mut total_latency_ms = 0u64;

    for event in &run.events {
        let t = event.recorded_at.to_rfc3339();

        match event.event_type.as_str() {
            "tool" => {
                let payload = match event.payload.as_object() {
                    Some(payload) => payload,
                    None => {
                        let payload_hash = sha256_prefixed(&canonical_json(&event.payload)?);
                        push_replay_line(
                            &mut lines,
                            &ReplayEvent::RuntimeEvent {
                                t,
                                session_id: session_id.clone(),
                                seq: event.seq,
                                runtime_event_type: event.event_type.clone(),
                                payload_hash,
                                payload: event.payload.clone(),
                                idempotency_key: event.idempotency_key.clone(),
                            },
                        )?;
                        continue;
                    }
                };

                let id = event
                    .idempotency_key
                    .clone()
                    .unwrap_or_else(|| format!("runtime.seq:{}", event.seq));

                let tool = payload
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let params = payload
                    .get("params")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Default::default()));
                let params_hash = payload
                    .get("params_hash")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        sha256_prefixed(&canonical_json(&params).unwrap_or_default())
                    });
                let output_hash = payload
                    .get("output_hash")
                    .and_then(Value::as_str)
                    .unwrap_or("sha256:unknown")
                    .to_string();
                let step_id = payload
                    .get("step_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| id.clone());
                let latency_ms = payload
                    .get("latency_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let exit_code = payload
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .and_then(|value| value.try_into().ok());
                let step_utility = payload
                    .get("step_utility")
                    .and_then(Value::as_f64)
                    .map(|value| value as f32)
                    .unwrap_or(0.0);

                total_tool_calls = total_tool_calls.saturating_add(1);
                total_latency_ms = total_latency_ms.saturating_add(latency_ms);

                push_replay_line(
                    &mut lines,
                    &ReplayEvent::ToolCall {
                        t: t.clone(),
                        id: id.clone(),
                        tool,
                        params,
                        params_hash,
                        step_id,
                    },
                )?;
                push_replay_line(
                    &mut lines,
                    &ReplayEvent::ToolResult {
                        t,
                        id,
                        output_hash,
                        exit_code,
                        step_utility,
                        latency_ms,
                    },
                )?;
            }
            "verification" => {
                let payload = match event.payload.as_object() {
                    Some(payload) => payload,
                    None => {
                        let payload_hash = sha256_prefixed(&canonical_json(&event.payload)?);
                        push_replay_line(
                            &mut lines,
                            &ReplayEvent::RuntimeEvent {
                                t,
                                session_id: session_id.clone(),
                                seq: event.seq,
                                runtime_event_type: event.event_type.clone(),
                                payload_hash,
                                payload: event.payload.clone(),
                                idempotency_key: event.idempotency_key.clone(),
                            },
                        )?;
                        continue;
                    }
                };

                let command = payload
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let exit_code: i32 = payload
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .and_then(|value| value.try_into().ok())
                    .unwrap_or(-1);
                let verification_delta: i32 = payload
                    .get("verification_delta")
                    .and_then(Value::as_i64)
                    .and_then(|value| value.try_into().ok())
                    .unwrap_or(0);

                push_replay_line(
                    &mut lines,
                    &ReplayEvent::Verification {
                        t,
                        commands: vec![command],
                        exit_codes: vec![exit_code],
                        verification_delta,
                    },
                )?;
            }
            "payment" => {
                let payload = match event.payload.as_object() {
                    Some(payload) => payload,
                    None => {
                        let payload_hash = sha256_prefixed(&canonical_json(&event.payload)?);
                        push_replay_line(
                            &mut lines,
                            &ReplayEvent::RuntimeEvent {
                                t,
                                session_id: session_id.clone(),
                                seq: event.seq,
                                runtime_event_type: event.event_type.clone(),
                                payload_hash,
                                payload: event.payload.clone(),
                                idempotency_key: event.idempotency_key.clone(),
                            },
                        )?;
                        continue;
                    }
                };

                let rail = payload
                    .get("rail")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let asset_id = payload
                    .get("asset_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let amount_msats = payload
                    .get("amount_msats")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let payment_proof = payload.get("payment_proof").cloned().unwrap_or(Value::Null);
                let job_hash = payload
                    .get("job_hash")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let status = payload
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("released")
                    .to_string();

                push_replay_line(
                    &mut lines,
                    &ReplayEvent::Payment {
                        t,
                        rail,
                        asset_id,
                        amount_msats,
                        payment_proof,
                        job_hash,
                        status,
                    },
                )?;
            }
            _ => {
                let payload_hash = sha256_prefixed(&canonical_json(&event.payload)?);
                push_replay_line(
                    &mut lines,
                    &ReplayEvent::RuntimeEvent {
                        t,
                        session_id: session_id.clone(),
                        seq: event.seq,
                        runtime_event_type: event.event_type.clone(),
                        payload_hash,
                        payload: event.payload.clone(),
                        idempotency_key: event.idempotency_key.clone(),
                    },
                )?;
            }
        }
    }

    let end_t = run
        .events
        .last()
        .map_or(run.updated_at.to_rfc3339(), |event| {
            event.recorded_at.to_rfc3339()
        });

    push_replay_line(
        &mut lines,
        &ReplayEvent::SessionEnd {
            t: end_t,
            status: replay_status(&run.status).to_string(),
            confidence: replay_confidence(&run.status),
            total_tool_calls,
            total_latency_ms,
        },
    )?;

    Ok(format!("{}\n", lines.join("\n")))
}

pub fn validate_replay_jsonl(replay: &str) -> Result<(), ArtifactError> {
    let parsed = replay
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line))
        .collect::<Result<Vec<_>, _>>()?;

    if parsed.is_empty() {
        return Err(ArtifactError::InvalidReplay("replay is empty".to_string()));
    }

    let first_event = parsed
        .first()
        .and_then(|value| value.get("event").and_then(Value::as_str))
        .unwrap_or_default();
    if first_event != "ReplayHeader" {
        return Err(ArtifactError::InvalidReplay(format!(
            "expected first event ReplayHeader, got {}",
            first_event
        )));
    }

    let replay_version = parsed
        .first()
        .and_then(|value| value.get("replay_version").and_then(Value::as_u64))
        .unwrap_or_default();
    if replay_version != 1 {
        return Err(ArtifactError::InvalidReplay(format!(
            "unsupported replay_version {}",
            replay_version
        )));
    }

    let second_event = parsed
        .get(1)
        .and_then(|value| value.get("event").and_then(Value::as_str))
        .unwrap_or_default();
    if second_event != "SessionStart" {
        return Err(ArtifactError::InvalidReplay(format!(
            "expected second event SessionStart, got {}",
            second_event
        )));
    }

    let last_event = parsed
        .last()
        .and_then(|value| value.get("event").and_then(Value::as_str))
        .unwrap_or_default();
    if last_event != "SessionEnd" {
        return Err(ArtifactError::InvalidReplay(format!(
            "expected last event SessionEnd, got {}",
            last_event
        )));
    }

    Ok(())
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
        super::validate_replay_jsonl(&replay)?;
        let lines = replay.lines().collect::<Vec<_>>();
        if lines.len() < 4 {
            return Err(anyhow!("replay should have header/start/events/end"));
        }
        if !lines
            .first()
            .map_or(false, |line| line.contains("\"event\":\"ReplayHeader\""))
        {
            return Err(anyhow!("missing replay header"));
        }
        if !lines
            .last()
            .map_or(false, |line| line.contains("\"event\":\"SessionEnd\""))
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
