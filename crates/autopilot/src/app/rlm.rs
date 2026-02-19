use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

const DEFAULT_RUN_LIMIT: u32 = 50;
const DEFAULT_TRACE_LIMIT: u32 = 200;

#[derive(Clone, Debug)]
pub(crate) enum RlmStatus {
    Idle,
    Refreshing,
    MissingDatabase,
    NoHomeDir,
    Error(String),
}

impl RlmStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            RlmStatus::Idle => "Idle",
            RlmStatus::Refreshing => "Refreshing",
            RlmStatus::MissingDatabase => "Database missing",
            RlmStatus::NoHomeDir => "No home dir",
            RlmStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            RlmStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RlmRunSummary {
    pub(crate) id: String,
    pub(crate) query: String,
    pub(crate) status: String,
    pub(crate) fragment_count: i64,
    pub(crate) budget_sats: i64,
    pub(crate) total_cost_sats: i64,
    pub(crate) total_duration_ms: i64,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) completed_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct RlmSnapshot {
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) db_exists: bool,
    pub(crate) runs: Vec<RlmRunSummary>,
}

impl RlmSnapshot {
    fn empty(db_path: Option<PathBuf>, db_exists: bool) -> Self {
        Self {
            db_path,
            db_exists,
            runs: Vec::new(),
        }
    }
}

pub(crate) struct RlmState {
    pub(crate) status: RlmStatus,
    pub(crate) snapshot: RlmSnapshot,
    pub(crate) last_refresh: Option<u64>,
}

impl RlmState {
    pub(crate) fn new() -> Self {
        Self {
            status: RlmStatus::Idle,
            snapshot: RlmSnapshot::empty(None, false),
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = RlmStatus::Refreshing;
        let (status, snapshot) = load_snapshot(DEFAULT_RUN_LIMIT);
        self.status = status;
        self.snapshot = snapshot;
        self.last_refresh = Some(now());
    }
}

impl Default for RlmState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RlmTraceStatus {
    Idle,
    Refreshing,
    MissingDatabase,
    NoHomeDir,
    MissingRun,
    Error(String),
}

impl RlmTraceStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            RlmTraceStatus::Idle => "Idle",
            RlmTraceStatus::Refreshing => "Refreshing",
            RlmTraceStatus::MissingDatabase => "Database missing",
            RlmTraceStatus::NoHomeDir => "No home dir",
            RlmTraceStatus::MissingRun => "No runs",
            RlmTraceStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            RlmTraceStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RlmTraceEvent {
    pub(crate) seq: i64,
    pub(crate) event_type: String,
    pub(crate) timestamp_ms: i64,
    pub(crate) event_json: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RlmTraceSnapshot {
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) db_exists: bool,
    pub(crate) run: Option<RlmRunSummary>,
    pub(crate) events: Vec<RlmTraceEvent>,
}

impl RlmTraceSnapshot {
    fn empty(db_path: Option<PathBuf>, db_exists: bool) -> Self {
        Self {
            db_path,
            db_exists,
            run: None,
            events: Vec::new(),
        }
    }
}

pub(crate) struct RlmTraceState {
    pub(crate) status: RlmTraceStatus,
    pub(crate) snapshot: RlmTraceSnapshot,
    pub(crate) last_refresh: Option<u64>,
    pub(crate) selected_run_id: Option<String>,
}

impl RlmTraceState {
    pub(crate) fn new() -> Self {
        Self {
            status: RlmTraceStatus::Idle,
            snapshot: RlmTraceSnapshot::empty(None, false),
            last_refresh: None,
            selected_run_id: None,
        }
    }

    pub(crate) fn refresh(&mut self, run_id: Option<String>) {
        self.status = RlmTraceStatus::Refreshing;
        let (status, snapshot, selected_run_id) = load_trace_snapshot(DEFAULT_TRACE_LIMIT, run_id);
        self.status = status;
        self.snapshot = snapshot;
        self.selected_run_id = selected_run_id;
        self.last_refresh = Some(now());
    }

    pub(crate) fn refresh_selected(&mut self) {
        let run_id = self.selected_run_id.clone();
        self.refresh(run_id);
    }
}

impl Default for RlmTraceState {
    fn default() -> Self {
        Self::new()
    }
}

fn load_snapshot(limit: u32) -> (RlmStatus, RlmSnapshot) {
    let Some(home) = dirs::home_dir() else {
        return (RlmStatus::NoHomeDir, RlmSnapshot::empty(None, false));
    };
    let db_path = home.join(".openagents").join("pylon").join("rlm.db");
    if !db_path.exists() {
        return (
            RlmStatus::MissingDatabase,
            RlmSnapshot::empty(Some(db_path), false),
        );
    }

    let conn = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            return (
                RlmStatus::Error(format!("DB open failed: {}", err)),
                RlmSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, query, status, fragment_count, budget_sats, total_cost_sats,
                total_duration_ms, error_message, created_at, completed_at
         FROM runs
         ORDER BY created_at DESC
         LIMIT ?1",
    ) {
        Ok(stmt) => stmt,
        Err(err) => {
            return (
                RlmStatus::Error(format!("Query prepare failed: {}", err)),
                RlmSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let rows = match stmt.query_map(params![limit], |row| {
        Ok(RlmRunSummary {
            id: row.get(0)?,
            query: row.get(1)?,
            status: row.get(2)?,
            fragment_count: row.get(3)?,
            budget_sats: row.get(4)?,
            total_cost_sats: row.get(5)?,
            total_duration_ms: row.get(6)?,
            error_message: row.get(7)?,
            created_at: row.get(8)?,
            completed_at: row.get(9)?,
        })
    }) {
        Ok(rows) => rows,
        Err(err) => {
            return (
                RlmStatus::Error(format!("Query failed: {}", err)),
                RlmSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let mut runs = Vec::new();
    for row in rows {
        match row {
            Ok(run) => runs.push(run),
            Err(err) => {
                return (
                    RlmStatus::Error(format!("Row parse failed: {}", err)),
                    RlmSnapshot::empty(Some(db_path), true),
                );
            }
        }
    }

    (
        RlmStatus::Idle,
        RlmSnapshot {
            db_path: Some(db_path),
            db_exists: true,
            runs,
        },
    )
}

fn load_trace_snapshot(
    limit: u32,
    run_id: Option<String>,
) -> (RlmTraceStatus, RlmTraceSnapshot, Option<String>) {
    let Some(home) = dirs::home_dir() else {
        return (
            RlmTraceStatus::NoHomeDir,
            RlmTraceSnapshot::empty(None, false),
            run_id,
        );
    };
    let db_path = home.join(".openagents").join("pylon").join("rlm.db");
    if !db_path.exists() {
        return (
            RlmTraceStatus::MissingDatabase,
            RlmTraceSnapshot::empty(Some(db_path), false),
            run_id,
        );
    }

    let conn = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            return (
                RlmTraceStatus::Error(format!("DB open failed: {}", err)),
                RlmTraceSnapshot::empty(Some(db_path), true),
                run_id,
            );
        }
    };

    let selected_run_id = match run_id {
        Some(id) if !id.trim().is_empty() => Some(id),
        _ => match fetch_latest_run_id(&conn) {
            Ok(id) => id,
            Err(err) => {
                return (
                    RlmTraceStatus::Error(format!("Run lookup failed: {}", err)),
                    RlmTraceSnapshot::empty(Some(db_path), true),
                    None,
                );
            }
        },
    };

    let Some(run_id) = selected_run_id.clone() else {
        return (
            RlmTraceStatus::MissingRun,
            RlmTraceSnapshot::empty(Some(db_path), true),
            None,
        );
    };

    let run = match load_run_summary(&conn, &run_id) {
        Ok(run) => run,
        Err(err) => {
            return (
                RlmTraceStatus::Error(format!("Run query failed: {}", err)),
                RlmTraceSnapshot::empty(Some(db_path), true),
                Some(run_id),
            );
        }
    };

    let Some(run) = run else {
        return (
            RlmTraceStatus::MissingRun,
            RlmTraceSnapshot::empty(Some(db_path), true),
            Some(run_id),
        );
    };

    let events = match load_trace_events(&conn, &run.id, limit) {
        Ok(events) => events,
        Err(err) => {
            return (
                RlmTraceStatus::Error(format!("Trace query failed: {}", err)),
                RlmTraceSnapshot::empty(Some(db_path), true),
                Some(run_id),
            );
        }
    };

    (
        RlmTraceStatus::Idle,
        RlmTraceSnapshot {
            db_path: Some(db_path),
            db_exists: true,
            run: Some(run),
            events,
        },
        Some(run_id),
    )
}

fn fetch_latest_run_id(conn: &Connection) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT id FROM runs ORDER BY created_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
}

fn load_run_summary(
    conn: &Connection,
    run_id: &str,
) -> Result<Option<RlmRunSummary>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, query, status, fragment_count, budget_sats, total_cost_sats,
                total_duration_ms, error_message, created_at, completed_at
         FROM runs
         WHERE id = ?1",
        params![run_id],
        |row| {
            Ok(RlmRunSummary {
                id: row.get(0)?,
                query: row.get(1)?,
                status: row.get(2)?,
                fragment_count: row.get(3)?,
                budget_sats: row.get(4)?,
                total_cost_sats: row.get(5)?,
                total_duration_ms: row.get(6)?,
                error_message: row.get(7)?,
                created_at: row.get(8)?,
                completed_at: row.get(9)?,
            })
        },
    )
    .optional()
}

fn load_trace_events(
    conn: &Connection,
    run_id: &str,
    limit: u32,
) -> Result<Vec<RlmTraceEvent>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT seq, event_type, timestamp_ms, event_json
         FROM trace_events
         WHERE run_id = ?1
         ORDER BY seq DESC
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![run_id, limit], |row| {
        Ok(RlmTraceEvent {
            seq: row.get(0)?,
            event_type: row.get(1)?,
            timestamp_ms: row.get(2)?,
            event_json: row.get(3)?,
        })
    })?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    events.reverse();
    Ok(events)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_labels() {
        assert_eq!(RlmStatus::Idle.label(), "Idle");
        assert_eq!(RlmStatus::MissingDatabase.label(), "Database missing");
    }
}
