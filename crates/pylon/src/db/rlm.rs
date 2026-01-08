//! SQLite storage for RLM runs and trace events.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Record of an RLM run stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmRunRecord {
    pub id: String,
    pub query: String,
    pub status: String,
    pub fragment_count: i64,
    pub budget_sats: i64,
    pub total_cost_sats: i64,
    pub total_duration_ms: i64,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// SQLite store for RLM runs.
pub struct RlmStore {
    conn: Connection,
}

impl RlmStore {
    /// Open or create an RLM store at the given path.
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Run the RLM schema migration.
    fn migrate(&self) -> anyhow::Result<()> {
        self.conn
            .execute_batch(include_str!("migrations/004_rlm.sql"))?;
        Ok(())
    }

    /// Insert a new running RLM run.
    pub fn insert_run(
        &self,
        run_id: &str,
        query: &str,
        fragment_count: usize,
        budget_sats: u64,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO runs (id, query, status, fragment_count, budget_sats)
             VALUES (?1, ?2, 'running', ?3, ?4)",
            params![
                run_id,
                query,
                fragment_count as i64,
                budget_sats as i64
            ],
        )?;
        Ok(())
    }

    /// Mark a run as completed.
    pub fn mark_completed(
        &self,
        run_id: &str,
        output: &str,
        total_cost_sats: u64,
        total_duration_ms: u64,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE runs
             SET status = 'completed',
                 output = ?1,
                 total_cost_sats = ?2,
                 total_duration_ms = ?3,
                 completed_at = unixepoch()
             WHERE id = ?4",
            params![
                output,
                total_cost_sats as i64,
                total_duration_ms as i64,
                run_id
            ],
        )?;
        Ok(())
    }

    /// Mark a run as failed.
    pub fn mark_failed(&self, run_id: &str, error_message: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE runs
             SET status = 'failed',
                 error_message = ?1,
                 completed_at = unixepoch()
             WHERE id = ?2",
            params![error_message, run_id],
        )?;
        Ok(())
    }

    /// List recent runs, newest first.
    pub fn list_runs(&self, limit: u32) -> anyhow::Result<Vec<RlmRunRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, query, status, fragment_count, budget_sats, total_cost_sats,
                    total_duration_ms, created_at, completed_at
             FROM runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(RlmRunRecord {
                id: row.get(0)?,
                query: row.get(1)?,
                status: row.get(2)?,
                fragment_count: row.get(3)?,
                budget_sats: row.get(4)?,
                total_cost_sats: row.get(5)?,
                total_duration_ms: row.get(6)?,
                created_at: row.get(7)?,
                completed_at: row.get(8)?,
            })
        })?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }
}
