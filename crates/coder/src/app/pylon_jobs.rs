use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};

use crate::app::pylon_paths::pylon_data_dir;

const DEFAULT_JOB_LIMIT: u32 = 50;

#[derive(Clone, Debug)]
pub(crate) enum PylonJobsStatus {
    Idle,
    Refreshing,
    MissingDatabase,
    NoHomeDir,
    Error(String),
}

impl PylonJobsStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            PylonJobsStatus::Idle => "Idle",
            PylonJobsStatus::Refreshing => "Refreshing",
            PylonJobsStatus::MissingDatabase => "Database missing",
            PylonJobsStatus::NoHomeDir => "No home dir",
            PylonJobsStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            PylonJobsStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PylonJobRecord {
    pub(crate) id: String,
    pub(crate) kind: i64,
    pub(crate) status: String,
    pub(crate) price_msats: i64,
    pub(crate) customer_pubkey: String,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) completed_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct PylonJobsTotals {
    pub(crate) total_jobs: i64,
    pub(crate) total_price_msats: i64,
    pub(crate) completed_price_msats: i64,
    pub(crate) pending: i64,
    pub(crate) processing: i64,
    pub(crate) completed: i64,
    pub(crate) failed: i64,
    pub(crate) cancelled: i64,
}

impl PylonJobsTotals {
    fn empty() -> Self {
        Self {
            total_jobs: 0,
            total_price_msats: 0,
            completed_price_msats: 0,
            pending: 0,
            processing: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PylonJobsSnapshot {
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) db_exists: bool,
    pub(crate) totals: PylonJobsTotals,
    pub(crate) jobs: Vec<PylonJobRecord>,
}

impl PylonJobsSnapshot {
    fn empty(db_path: Option<PathBuf>, db_exists: bool) -> Self {
        Self {
            db_path,
            db_exists,
            totals: PylonJobsTotals::empty(),
            jobs: Vec::new(),
        }
    }
}

pub(crate) struct PylonJobsState {
    pub(crate) status: PylonJobsStatus,
    pub(crate) snapshot: PylonJobsSnapshot,
    pub(crate) last_refresh: Option<u64>,
}

impl PylonJobsState {
    pub(crate) fn new() -> Self {
        Self {
            status: PylonJobsStatus::Idle,
            snapshot: PylonJobsSnapshot::empty(None, false),
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = PylonJobsStatus::Refreshing;
        let (status, snapshot) = load_snapshot(DEFAULT_JOB_LIMIT);
        self.status = status;
        self.snapshot = snapshot;
        self.last_refresh = Some(now());
    }
}

impl Default for PylonJobsState {
    fn default() -> Self {
        Self::new()
    }
}

fn load_snapshot(limit: u32) -> (PylonJobsStatus, PylonJobsSnapshot) {
    let Some(pylon_dir) = pylon_data_dir() else {
        return (
            PylonJobsStatus::NoHomeDir,
            PylonJobsSnapshot::empty(None, false),
        );
    };
    let db_path = pylon_dir.join("pylon.db");
    if !db_path.exists() {
        return (
            PylonJobsStatus::MissingDatabase,
            PylonJobsSnapshot::empty(Some(db_path), false),
        );
    }

    let conn = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            return (
                PylonJobsStatus::Error(format!("DB open failed: {}", err)),
                PylonJobsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let totals = match load_totals(&conn) {
        Ok(totals) => totals,
        Err(err) => {
            return (
                PylonJobsStatus::Error(format!("Totals query failed: {}", err)),
                PylonJobsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let jobs = match load_recent_jobs(&conn, limit) {
        Ok(jobs) => jobs,
        Err(err) => {
            return (
                PylonJobsStatus::Error(format!("Jobs query failed: {}", err)),
                PylonJobsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    (
        PylonJobsStatus::Idle,
        PylonJobsSnapshot {
            db_path: Some(db_path),
            db_exists: true,
            totals,
            jobs,
        },
    )
}

fn load_totals(conn: &Connection) -> Result<PylonJobsTotals, rusqlite::Error> {
    let total_jobs: i64 = conn.query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))?;
    let total_price_msats: i64 = conn.query_row(
        "SELECT COALESCE(SUM(price_msats), 0) FROM jobs",
        [],
        |row| row.get(0),
    )?;
    let completed_price_msats: i64 = conn.query_row(
        "SELECT COALESCE(SUM(price_msats), 0) FROM jobs WHERE status = 'completed'",
        [],
        |row| row.get(0),
    )?;

    let mut totals = PylonJobsTotals {
        total_jobs,
        total_price_msats,
        completed_price_msats,
        ..PylonJobsTotals::empty()
    };

    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM jobs GROUP BY status")?;
    let rows = stmt.query_map([], |row| {
        let status: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((status, count))
    })?;

    for row in rows {
        let (status, count) = row?;
        match status.as_str() {
            "pending" => totals.pending = count,
            "processing" => totals.processing = count,
            "completed" => totals.completed = count,
            "failed" => totals.failed = count,
            "cancelled" => totals.cancelled = count,
            _ => {}
        }
    }

    Ok(totals)
}

fn load_recent_jobs(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<PylonJobRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, status, price_msats, customer_pubkey, error_message, created_at, completed_at
         FROM jobs
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        Ok(PylonJobRecord {
            id: row.get(0)?,
            kind: row.get(1)?,
            status: row.get(2)?,
            price_msats: row.get(3)?,
            customer_pubkey: row.get(4)?,
            error_message: row.get(5)?,
            created_at: row.get(6)?,
            completed_at: row.get(7)?,
        })
    })?;

    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row?);
    }
    Ok(jobs)
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
        assert_eq!(PylonJobsStatus::Idle.label(), "Idle");
        assert_eq!(PylonJobsStatus::MissingDatabase.label(), "Database missing");
    }
}
