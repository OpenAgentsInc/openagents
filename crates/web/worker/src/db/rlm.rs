//! RLM run database operations for D1.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

/// RLM run record stored in D1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmRunRecord {
    pub id: String,
    pub user_id: String,
    pub query: String,
    pub status: String,
    pub fragment_count: i64,
    pub budget_sats: i64,
    pub total_cost_sats: i64,
    pub total_duration_ms: i64,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// Trace event record stored in D1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmTraceEventRecord {
    pub run_id: String,
    pub seq: i64,
    pub event_type: String,
    pub timestamp_ms: i64,
    pub event_json: String,
}

/// List recent runs for a user.
pub async fn list_runs(
    db: &D1Database,
    user_id: &str,
    limit: u32,
    before: Option<i64>,
) -> Result<Vec<RlmRunRecord>> {
    let (sql, bindings): (&str, Vec<JsValue>) = if let Some(before_ts) = before {
        (
            "SELECT id, user_id, query, status, fragment_count, budget_sats, total_cost_sats,
                    total_duration_ms, output, error_message, created_at, completed_at
             FROM rlm_runs
             WHERE user_id = ? AND created_at < ?
             ORDER BY created_at DESC
             LIMIT ?",
            vec![user_id.into(), before_ts.into(), (limit as i64).into()],
        )
    } else {
        (
            "SELECT id, user_id, query, status, fragment_count, budget_sats, total_cost_sats,
                    total_duration_ms, output, error_message, created_at, completed_at
             FROM rlm_runs
             WHERE user_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
            vec![user_id.into(), (limit as i64).into()],
        )
    };

    db.prepare(sql)
        .bind(&bindings)?
        .all()
        .await
        .and_then(|result| result.results::<RlmRunRecord>())
}

/// Get a run by ID for a user.
pub async fn get_run(
    db: &D1Database,
    user_id: &str,
    run_id: &str,
) -> Result<Option<RlmRunRecord>> {
    db.prepare(
        "SELECT id, user_id, query, status, fragment_count, budget_sats, total_cost_sats,
                total_duration_ms, output, error_message, created_at, completed_at
         FROM rlm_runs
         WHERE user_id = ? AND id = ?",
    )
    .bind(&[user_id.into(), run_id.into()])?
    .first::<RlmRunRecord>(None)
    .await
}

/// Upsert a run for a user.
pub async fn upsert_run(
    db: &D1Database,
    run: &RlmRunRecord,
) -> Result<()> {
    db.prepare(
        "INSERT INTO rlm_runs (
            id, user_id, query, status, fragment_count, budget_sats, total_cost_sats,
            total_duration_ms, output, error_message, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            user_id = excluded.user_id,
            query = excluded.query,
            status = excluded.status,
            fragment_count = excluded.fragment_count,
            budget_sats = excluded.budget_sats,
            total_cost_sats = excluded.total_cost_sats,
            total_duration_ms = excluded.total_duration_ms,
            output = excluded.output,
            error_message = excluded.error_message,
            completed_at = excluded.completed_at",
    )
    .bind(&[
        run.id.clone().into(),
        run.user_id.clone().into(),
        run.query.clone().into(),
        run.status.clone().into(),
        run.fragment_count.into(),
        run.budget_sats.into(),
        run.total_cost_sats.into(),
        run.total_duration_ms.into(),
        run.output.clone().map(|v| v.into()).unwrap_or(JsValue::NULL),
        run.error_message.clone().map(|v| v.into()).unwrap_or(JsValue::NULL),
        run.created_at.into(),
        run.completed_at.map(|v| v.into()).unwrap_or(JsValue::NULL),
    ])?
    .run()
    .await?;

    Ok(())
}

/// Replace trace events for a run.
pub async fn replace_trace_events(
    db: &D1Database,
    run_id: &str,
    events: &[RlmTraceEventRecord],
) -> Result<()> {
    db.prepare("DELETE FROM rlm_trace_events WHERE run_id = ?")
        .bind(&[run_id.into()])?
        .run()
        .await?;

    for event in events {
        db.prepare(
            "INSERT INTO rlm_trace_events (
                run_id, seq, event_type, timestamp_ms, event_json
             ) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&[
            event.run_id.clone().into(),
            event.seq.into(),
            event.event_type.clone().into(),
            event.timestamp_ms.into(),
            event.event_json.clone().into(),
        ])?
        .run()
        .await?;
    }

    Ok(())
}

/// List trace events for a run.
pub async fn list_trace_events(
    db: &D1Database,
    run_id: &str,
) -> Result<Vec<RlmTraceEventRecord>> {
    db.prepare(
        "SELECT run_id, seq, event_type, timestamp_ms, event_json
         FROM rlm_trace_events
         WHERE run_id = ?
         ORDER BY seq ASC",
    )
    .bind(&[run_id.into()])?
    .all()
    .await
    .and_then(|result| result.results::<RlmTraceEventRecord>())
}
