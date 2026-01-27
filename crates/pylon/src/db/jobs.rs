//! Job persistence for provider mode

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::PylonDb;

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(JobStatus::Pending),
            "processing" => Some(JobStatus::Processing),
            "completed" => Some(JobStatus::Completed),
            "failed" => Some(JobStatus::Failed),
            "cancelled" => Some(JobStatus::Cancelled),
            _ => None,
        }
    }
}

/// A job record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub kind: u16,
    pub customer_pubkey: String,
    pub status: JobStatus,
    pub price_msats: u64,
    pub input_hash: Option<String>,
    pub output_hash: Option<String>,
    pub error_message: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub created_at: u64,
}

impl PylonDb {
    /// Create a new job
    pub fn create_job(&self, job: &Job) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO jobs (id, kind, customer_pubkey, status, price_msats, input_hash, started_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                job.id,
                job.kind,
                job.customer_pubkey,
                job.status.as_str(),
                job.price_msats as i64,
                job.input_hash,
                job.started_at as i64,
                job.created_at as i64,
            ],
        )?;
        Ok(())
    }

    /// Get a job by ID
    pub fn get_job(&self, id: &str) -> anyhow::Result<Option<Job>> {
        let job = self
            .conn()
            .query_row(
                "SELECT id, kind, customer_pubkey, status, price_msats, input_hash, output_hash,
                        error_message, started_at, completed_at, created_at
                 FROM jobs WHERE id = ?",
                [id],
                |row| {
                    Ok(Job {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        customer_pubkey: row.get(2)?,
                        status: JobStatus::from_str(&row.get::<_, String>(3)?)
                            .unwrap_or(JobStatus::Pending),
                        price_msats: row.get::<_, i64>(4)? as u64,
                        input_hash: row.get(5)?,
                        output_hash: row.get(6)?,
                        error_message: row.get(7)?,
                        started_at: row.get::<_, i64>(8)? as u64,
                        completed_at: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
                        created_at: row.get::<_, i64>(10)? as u64,
                    })
                },
            )
            .optional()?;

        Ok(job)
    }

    /// Update job status
    pub fn update_job_status(&self, id: &str, status: JobStatus) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = if matches!(
            status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            Some(now)
        } else {
            None
        };

        self.conn().execute(
            "UPDATE jobs SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![status.as_str(), completed_at, id],
        )?;
        Ok(())
    }

    /// Mark job as completed with output
    pub fn complete_job(
        &self,
        id: &str,
        output_hash: Option<&str>,
        price_msats: u64,
    ) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "UPDATE jobs SET status = 'completed', output_hash = ?1, price_msats = ?2, completed_at = ?3 WHERE id = ?4",
            params![output_hash, price_msats as i64, now, id],
        )?;
        Ok(())
    }

    /// Mark job as failed with error
    pub fn fail_job(&self, id: &str, error: &str) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "UPDATE jobs SET status = 'failed', error_message = ?1, completed_at = ?2 WHERE id = ?3",
            params![error, now, id],
        )?;
        Ok(())
    }

    /// List jobs by status
    pub fn list_jobs_by_status(&self, status: JobStatus, limit: usize) -> anyhow::Result<Vec<Job>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, customer_pubkey, status, price_msats, input_hash, output_hash,
                    error_message, started_at, completed_at, created_at
             FROM jobs WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let jobs = stmt
            .query_map(params![status.as_str(), limit as i64], |row| {
                Ok(Job {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    customer_pubkey: row.get(2)?,
                    status: JobStatus::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(JobStatus::Pending),
                    price_msats: row.get::<_, i64>(4)? as u64,
                    input_hash: row.get(5)?,
                    output_hash: row.get(6)?,
                    error_message: row.get(7)?,
                    started_at: row.get::<_, i64>(8)? as u64,
                    completed_at: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
                    created_at: row.get::<_, i64>(10)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get total job count by status
    pub fn count_jobs_by_status(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<JobStatus, u64>> {
        use std::collections::HashMap;

        let mut counts = HashMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT status, COUNT(*) FROM jobs GROUP BY status")?;

        let rows = stmt.query_map([], |row| {
            let status_str: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((status_str, count))
        })?;

        for row in rows {
            let (status_str, count) = row?;
            if let Ok(status) = JobStatus::from_str(&status_str) {
                counts.insert(status, count as u64);
            }
        }

        Ok(counts)
    }

    // Invoice methods

    /// Record a new invoice for a job
    pub fn record_invoice(
        &self,
        job_id: &str,
        bolt11: &str,
        amount_msats: u64,
    ) -> anyhow::Result<()> {
        let id = format!("inv_{}", &job_id[..16.min(job_id.len())]);

        self.conn().execute(
            "INSERT INTO invoices (id, job_id, bolt11, amount_msats, status)
             VALUES (?1, ?2, ?3, ?4, 'pending')",
            params![id, job_id, bolt11, amount_msats as i64],
        )?;

        Ok(())
    }

    /// Mark an invoice as paid
    pub fn mark_invoice_paid(&self, job_id: &str, amount_msats: u64) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "UPDATE invoices SET status = 'paid', paid_amount_msats = ?1, paid_at = ?2 WHERE job_id = ?3",
            params![amount_msats as i64, now, job_id],
        )?;

        Ok(())
    }

    /// Get pending invoice count
    pub fn count_pending_invoices(&self) -> anyhow::Result<u64> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM invoices WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?;

        Ok(count as u64)
    }

    /// Mark expired invoices (older than given seconds)
    pub fn expire_old_invoices(&self, max_age_secs: u64) -> anyhow::Result<u64> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - max_age_secs as i64;

        let count = self.conn().execute(
            "UPDATE invoices SET status = 'expired' WHERE status = 'pending' AND created_at < ?1",
            params![cutoff],
        )?;

        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn test_create_and_get_job() {
        let db = PylonDb::open_in_memory().unwrap();

        let job = Job {
            id: "test-job-1".to_string(),
            kind: 5100,
            customer_pubkey: "abc123".to_string(),
            status: JobStatus::Pending,
            price_msats: 1000,
            input_hash: Some("input-hash".to_string()),
            output_hash: None,
            error_message: None,
            started_at: now(),
            completed_at: None,
            created_at: now(),
        };

        db.create_job(&job).unwrap();

        let retrieved = db.get_job("test-job-1").unwrap().unwrap();
        assert_eq!(retrieved.id, "test-job-1");
        assert_eq!(retrieved.kind, 5100);
        assert_eq!(retrieved.status, JobStatus::Pending);
    }

    #[test]
    fn test_complete_job() {
        let db = PylonDb::open_in_memory().unwrap();

        let job = Job {
            id: "test-job-2".to_string(),
            kind: 5100,
            customer_pubkey: "abc123".to_string(),
            status: JobStatus::Processing,
            price_msats: 0,
            input_hash: None,
            output_hash: None,
            error_message: None,
            started_at: now(),
            completed_at: None,
            created_at: now(),
        };

        db.create_job(&job).unwrap();
        db.complete_job("test-job-2", Some("output-hash"), 5000)
            .unwrap();

        let retrieved = db.get_job("test-job-2").unwrap().unwrap();
        assert_eq!(retrieved.status, JobStatus::Completed);
        assert_eq!(retrieved.price_msats, 5000);
        assert!(retrieved.completed_at.is_some());
    }
}
