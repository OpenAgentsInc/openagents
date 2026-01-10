use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};

use crate::app::pylon_paths::pylon_data_dir;

const DEFAULT_EARNINGS_LIMIT: u32 = 40;

#[derive(Clone, Debug)]
pub(crate) enum PylonEarningsStatus {
    Idle,
    Refreshing,
    MissingDatabase,
    NoHomeDir,
    Error(String),
}

impl PylonEarningsStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            PylonEarningsStatus::Idle => "Idle",
            PylonEarningsStatus::Refreshing => "Refreshing",
            PylonEarningsStatus::MissingDatabase => "Database missing",
            PylonEarningsStatus::NoHomeDir => "No home dir",
            PylonEarningsStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            PylonEarningsStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PylonEarningRecord {
    pub(crate) job_id: Option<String>,
    pub(crate) amount_msats: i64,
    pub(crate) source: String,
    pub(crate) payment_hash: Option<String>,
    pub(crate) earned_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct PylonEarningSourceTotal {
    pub(crate) source: String,
    pub(crate) amount_msats: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct PylonEarningsTotals {
    pub(crate) total_msats: i64,
    pub(crate) total_entries: i64,
    pub(crate) job_count: i64,
    pub(crate) by_source: Vec<PylonEarningSourceTotal>,
}

impl PylonEarningsTotals {
    fn empty() -> Self {
        Self {
            total_msats: 0,
            total_entries: 0,
            job_count: 0,
            by_source: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PylonEarningsSnapshot {
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) db_exists: bool,
    pub(crate) totals: PylonEarningsTotals,
    pub(crate) earnings: Vec<PylonEarningRecord>,
}

impl PylonEarningsSnapshot {
    fn empty(db_path: Option<PathBuf>, db_exists: bool) -> Self {
        Self {
            db_path,
            db_exists,
            totals: PylonEarningsTotals::empty(),
            earnings: Vec::new(),
        }
    }
}

pub(crate) struct PylonEarningsState {
    pub(crate) status: PylonEarningsStatus,
    pub(crate) snapshot: PylonEarningsSnapshot,
    pub(crate) last_refresh: Option<u64>,
}

impl PylonEarningsState {
    pub(crate) fn new() -> Self {
        Self {
            status: PylonEarningsStatus::Idle,
            snapshot: PylonEarningsSnapshot::empty(None, false),
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = PylonEarningsStatus::Refreshing;
        let (status, snapshot) = load_snapshot(DEFAULT_EARNINGS_LIMIT);
        self.status = status;
        self.snapshot = snapshot;
        self.last_refresh = Some(now());
    }
}

impl Default for PylonEarningsState {
    fn default() -> Self {
        Self::new()
    }
}

fn load_snapshot(limit: u32) -> (PylonEarningsStatus, PylonEarningsSnapshot) {
    let Some(pylon_dir) = pylon_data_dir() else {
        return (
            PylonEarningsStatus::NoHomeDir,
            PylonEarningsSnapshot::empty(None, false),
        );
    };
    let db_path = pylon_dir.join("pylon.db");
    if !db_path.exists() {
        return (
            PylonEarningsStatus::MissingDatabase,
            PylonEarningsSnapshot::empty(Some(db_path), false),
        );
    }

    let conn = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            return (
                PylonEarningsStatus::Error(format!("DB open failed: {}", err)),
                PylonEarningsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let totals = match load_totals(&conn) {
        Ok(totals) => totals,
        Err(err) => {
            return (
                PylonEarningsStatus::Error(format!("Totals query failed: {}", err)),
                PylonEarningsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let earnings = match load_recent_earnings(&conn, limit) {
        Ok(earnings) => earnings,
        Err(err) => {
            return (
                PylonEarningsStatus::Error(format!("Earnings query failed: {}", err)),
                PylonEarningsSnapshot::empty(Some(db_path), true),
            );
        }
    };

    (
        PylonEarningsStatus::Idle,
        PylonEarningsSnapshot {
            db_path: Some(db_path),
            db_exists: true,
            totals,
            earnings,
        },
    )
}

fn load_totals(conn: &Connection) -> Result<PylonEarningsTotals, rusqlite::Error> {
    let total_msats: i64 =
        conn.query_row("SELECT COALESCE(SUM(amount_msats), 0) FROM earnings", [], |row| {
            row.get(0)
        })?;
    let total_entries: i64 = conn.query_row("SELECT COUNT(*) FROM earnings", [], |row| {
        row.get(0)
    })?;
    let job_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT job_id) FROM earnings WHERE job_id IS NOT NULL",
        [],
        |row| row.get(0),
    )?;

    let mut stmt = conn.prepare("SELECT source, SUM(amount_msats) FROM earnings GROUP BY source")?;
    let rows = stmt.query_map([], |row| {
        let source: String = row.get(0)?;
        let amount: i64 = row.get(1)?;
        Ok(PylonEarningSourceTotal {
            source,
            amount_msats: amount,
        })
    })?;

    let mut by_source = Vec::new();
    for row in rows {
        by_source.push(row?);
    }
    by_source.sort_by(|a, b| a.source.cmp(&b.source));

    Ok(PylonEarningsTotals {
        total_msats,
        total_entries,
        job_count,
        by_source,
    })
}

fn load_recent_earnings(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<PylonEarningRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT job_id, amount_msats, source, payment_hash, earned_at
         FROM earnings
         ORDER BY earned_at DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        Ok(PylonEarningRecord {
            job_id: row.get(0)?,
            amount_msats: row.get(1)?,
            source: row.get(2)?,
            payment_hash: row.get(3)?,
            earned_at: row.get(4)?,
        })
    })?;

    let mut earnings = Vec::new();
    for row in rows {
        earnings.push(row?);
    }
    Ok(earnings)
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
        assert_eq!(PylonEarningsStatus::Idle.label(), "Idle");
        assert_eq!(PylonEarningsStatus::MissingDatabase.label(), "Database missing");
    }
}
