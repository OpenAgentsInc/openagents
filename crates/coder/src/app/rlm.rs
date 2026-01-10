use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};

const DEFAULT_RUN_LIMIT: u32 = 50;

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
