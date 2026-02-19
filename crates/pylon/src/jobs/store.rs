//! SQLite storage for submitted jobs

use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A job record stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    /// Event ID (primary key)
    pub id: String,
    /// Job kind (5050 for text generation, etc.)
    pub kind: u16,
    /// Input prompt
    pub prompt: String,
    /// Relay URL where job was submitted
    pub relay: String,
    /// Target provider pubkey (optional)
    pub provider: Option<String>,
    /// Bid in millisats
    pub bid_msats: Option<u64>,
    /// Job status: pending, processing, completed, failed
    pub status: String,
    /// Result content (if completed)
    pub result: Option<String>,
    /// Payment invoice (if required)
    pub bolt11: Option<String>,
    /// Payment amount in millisats
    pub amount_msats: Option<u64>,
    /// Unix timestamp when created
    pub created_at: i64,
    /// Unix timestamp when completed
    pub completed_at: Option<i64>,
}

impl JobRecord {
    /// Create a new pending job record
    pub fn new(id: String, kind: u16, prompt: String, relay: String) -> Self {
        Self {
            id,
            kind,
            prompt,
            relay,
            provider: None,
            bid_msats: None,
            status: "pending".to_string(),
            result: None,
            bolt11: None,
            amount_msats: None,
            created_at: chrono::Utc::now().timestamp(),
            completed_at: None,
        }
    }

    /// Set the target provider
    pub fn with_provider(mut self, provider: String) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the bid amount
    pub fn with_bid(mut self, bid_msats: u64) -> Self {
        self.bid_msats = Some(bid_msats);
        self
    }
}

/// SQLite store for job records
pub struct JobStore {
    conn: Connection,
}

impl JobStore {
    /// Open or create a job store at the given path
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                kind INTEGER NOT NULL,
                prompt TEXT NOT NULL,
                relay TEXT NOT NULL,
                provider TEXT,
                bid_msats INTEGER,
                status TEXT NOT NULL DEFAULT 'pending',
                result TEXT,
                bolt11 TEXT,
                amount_msats INTEGER,
                created_at INTEGER NOT NULL,
                completed_at INTEGER
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Insert a new job
    pub fn insert(&self, job: &JobRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO jobs (id, kind, prompt, relay, provider, bid_msats, status, result, bolt11, amount_msats, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                job.id,
                job.kind,
                job.prompt,
                job.relay,
                job.provider,
                job.bid_msats,
                job.status,
                job.result,
                job.bolt11,
                job.amount_msats,
                job.created_at,
                job.completed_at,
            ],
        )?;
        Ok(())
    }

    /// Get a job by ID
    pub fn get(&self, id: &str) -> Result<Option<JobRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, prompt, relay, provider, bid_msats, status, result, bolt11, amount_msats, created_at, completed_at
             FROM jobs WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(JobRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                prompt: row.get(2)?,
                relay: row.get(3)?,
                provider: row.get(4)?,
                bid_msats: row.get(5)?,
                status: row.get(6)?,
                result: row.get(7)?,
                bolt11: row.get(8)?,
                amount_msats: row.get(9)?,
                created_at: row.get(10)?,
                completed_at: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update job status
    pub fn update_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE jobs SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    /// Update job with result
    pub fn update_result(
        &self,
        id: &str,
        result: &str,
        bolt11: Option<&str>,
        amount_msats: Option<u64>,
    ) -> Result<()> {
        let completed_at = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE jobs SET status = 'completed', result = ?1, bolt11 = ?2, amount_msats = ?3, completed_at = ?4 WHERE id = ?5",
            params![result, bolt11, amount_msats, completed_at, id],
        )?;
        Ok(())
    }

    /// Mark job as failed
    pub fn mark_failed(&self, id: &str, error: &str) -> Result<()> {
        let completed_at = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE jobs SET status = 'failed', result = ?1, completed_at = ?2 WHERE id = ?3",
            params![error, completed_at, id],
        )?;
        Ok(())
    }

    /// List recent jobs
    pub fn list(&self, limit: u32) -> Result<Vec<JobRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, prompt, relay, provider, bid_msats, status, result, bolt11, amount_msats, created_at, completed_at
             FROM jobs ORDER BY created_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                prompt: row.get(2)?,
                relay: row.get(3)?,
                provider: row.get(4)?,
                bid_msats: row.get(5)?,
                status: row.get(6)?,
                result: row.get(7)?,
                bolt11: row.get(8)?,
                amount_msats: row.get(9)?,
                created_at: row.get(10)?,
                completed_at: row.get(11)?,
            })
        })?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_job_store() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("jobs.db");
        let store = JobStore::new(&db_path).unwrap();

        // Insert a job
        let job = JobRecord::new(
            "test123".to_string(),
            5050,
            "What is 2+2?".to_string(),
            "wss://nexus.openagents.com".to_string(),
        )
        .with_bid(1000);

        store.insert(&job).unwrap();

        // Get the job
        let retrieved = store.get("test123").unwrap().unwrap();
        assert_eq!(retrieved.id, "test123");
        assert_eq!(retrieved.prompt, "What is 2+2?");
        assert_eq!(retrieved.status, "pending");

        // Update with result
        store
            .update_result("test123", "4", Some("lnbc..."), Some(1000))
            .unwrap();

        let updated = store.get("test123").unwrap().unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.result.unwrap(), "4");

        // List jobs
        let jobs = store.list(10).unwrap();
        assert_eq!(jobs.len(), 1);
    }
}
