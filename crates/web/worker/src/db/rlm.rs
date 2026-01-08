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

/// RLM experiment record stored in D1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmExperimentRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// RLM experiment summary with run count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmExperimentSummary {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub run_count: i64,
}

/// RLM experiment run link payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmExperimentRunLink {
    pub run_id: String,
    pub label: Option<String>,
}

/// RLM experiment run record with joined run metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmExperimentRunRecord {
    pub experiment_id: String,
    pub run_id: String,
    pub label: Option<String>,
    pub added_at: i64,
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

/// List experiments for a user with run counts.
pub async fn list_experiments(
    db: &D1Database,
    user_id: &str,
    limit: u32,
    before: Option<i64>,
) -> Result<Vec<RlmExperimentSummary>> {
    let (sql, bindings): (&str, Vec<JsValue>) = if let Some(before_ts) = before {
        (
            "SELECT e.id, e.user_id, e.name, e.description, e.created_at, e.updated_at,
                    COUNT(er.run_id) as run_count
             FROM rlm_experiments e
             LEFT JOIN rlm_experiment_runs er ON e.id = er.experiment_id
             WHERE e.user_id = ? AND e.updated_at < ?
             GROUP BY e.id
             ORDER BY e.updated_at DESC
             LIMIT ?",
            vec![user_id.into(), before_ts.into(), (limit as i64).into()],
        )
    } else {
        (
            "SELECT e.id, e.user_id, e.name, e.description, e.created_at, e.updated_at,
                    COUNT(er.run_id) as run_count
             FROM rlm_experiments e
             LEFT JOIN rlm_experiment_runs er ON e.id = er.experiment_id
             WHERE e.user_id = ?
             GROUP BY e.id
             ORDER BY e.updated_at DESC
             LIMIT ?",
            vec![user_id.into(), (limit as i64).into()],
        )
    };

    db.prepare(sql)
        .bind(&bindings)?
        .all()
        .await
        .and_then(|result| result.results::<RlmExperimentSummary>())
}

/// Get a single experiment by ID.
pub async fn get_experiment(
    db: &D1Database,
    user_id: &str,
    experiment_id: &str,
) -> Result<Option<RlmExperimentRecord>> {
    db.prepare(
        "SELECT id, user_id, name, description, created_at, updated_at
         FROM rlm_experiments
         WHERE user_id = ? AND id = ?",
    )
    .bind(&[user_id.into(), experiment_id.into()])?
    .first::<RlmExperimentRecord>(None)
    .await
}

/// Create a new experiment.
pub async fn create_experiment(db: &D1Database, experiment: &RlmExperimentRecord) -> Result<()> {
    db.prepare(
        "INSERT INTO rlm_experiments (id, user_id, name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        experiment.id.clone().into(),
        experiment.user_id.clone().into(),
        experiment.name.clone().into(),
        experiment.description.clone().map(|v| v.into()).unwrap_or(JsValue::NULL),
        experiment.created_at.into(),
        experiment.updated_at.into(),
    ])?
    .run()
    .await?;
    Ok(())
}

/// Update experiment timestamp.
pub async fn touch_experiment(db: &D1Database, experiment_id: &str, updated_at: i64) -> Result<()> {
    db.prepare("UPDATE rlm_experiments SET updated_at = ? WHERE id = ?")
        .bind(&[updated_at.into(), experiment_id.into()])?
        .run()
        .await?;
    Ok(())
}

/// Add runs to an experiment.
pub async fn add_runs_to_experiment(
    db: &D1Database,
    experiment_id: &str,
    run_links: &[RlmExperimentRunLink],
) -> Result<()> {
    for link in run_links {
        db.prepare(
            "INSERT INTO rlm_experiment_runs (experiment_id, run_id, label)
             VALUES (?, ?, ?)
             ON CONFLICT(experiment_id, run_id) DO UPDATE SET
                label = excluded.label",
        )
        .bind(&[
            experiment_id.into(),
            link.run_id.clone().into(),
            link.label.clone().map(|v| v.into()).unwrap_or(JsValue::NULL),
        ])?
        .run()
        .await?;
    }
    Ok(())
}

/// Remove a run from an experiment.
pub async fn remove_run_from_experiment(
    db: &D1Database,
    experiment_id: &str,
    run_id: &str,
) -> Result<()> {
    db.prepare("DELETE FROM rlm_experiment_runs WHERE experiment_id = ? AND run_id = ?")
        .bind(&[experiment_id.into(), run_id.into()])?
        .run()
        .await?;
    Ok(())
}

/// List runs attached to an experiment.
pub async fn list_experiment_runs(
    db: &D1Database,
    user_id: &str,
    experiment_id: &str,
) -> Result<Vec<RlmExperimentRunRecord>> {
    db.prepare(
        "SELECT er.experiment_id, er.run_id, er.label, er.created_at as added_at,
                r.query, r.status, r.fragment_count, r.budget_sats, r.total_cost_sats,
                r.total_duration_ms, r.output, r.error_message, r.created_at, r.completed_at
         FROM rlm_experiment_runs er
         JOIN rlm_runs r ON r.id = er.run_id
         WHERE er.experiment_id = ? AND r.user_id = ?
         ORDER BY r.created_at DESC",
    )
    .bind(&[experiment_id.into(), user_id.into()])?
    .all()
    .await
    .and_then(|result| result.results::<RlmExperimentRunRecord>())
}
