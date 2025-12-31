//! Job database for persistent storage of marketplace compute jobs

use crate::compute::consumer::{JobInfo, JobState};
use rusqlite::{Connection, Result as SqliteResult, params};
use std::path::PathBuf;

/// Job database for persistent storage
pub struct JobDatabase {
    conn: Connection,
}

impl JobDatabase {
    /// Create a new job database
    pub fn new(db_path: Option<PathBuf>) -> SqliteResult<Self> {
        let path = db_path.unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".openagents")
                .join("marketplace.db")
        });

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn new_in_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> SqliteResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                request TEXT NOT NULL,
                result TEXT,
                error TEXT,
                provider TEXT,
                payment_amount INTEGER,
                payment_bolt11 TEXT,
                submitted_at INTEGER NOT NULL,
                completed_at INTEGER,
                local_attempted INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Create index on state for filtering
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state)",
            [],
        )?;

        // Create index on submitted_at for sorting
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_submitted_at ON jobs(submitted_at DESC)",
            [],
        )?;

        Ok(())
    }

    /// Save a job to the database
    pub fn save_job(&self, job: &JobInfo) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO jobs (
                job_id, state, request, result, error, provider,
                payment_amount, payment_bolt11, submitted_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.job_id,
                state_to_string(job.state),
                job.request,
                job.result,
                job.error,
                job.provider,
                job.payment_amount,
                job.payment_bolt11,
                job.submitted_at as i64,
                job.completed_at.map(|t| t as i64),
            ],
        )?;
        Ok(())
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: &str) -> SqliteResult<Option<JobInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT job_id, state, request, result, error, provider,
                    payment_amount, payment_bolt11, submitted_at, completed_at
             FROM jobs WHERE job_id = ?1",
        )?;

        let mut rows = stmt.query(params![job_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row_to_job_info(row)?))
        } else {
            Ok(None)
        }
    }

    /// Get all jobs, optionally filtered by state
    pub fn get_jobs(
        &self,
        state_filter: Option<JobState>,
        limit: Option<usize>,
    ) -> SqliteResult<Vec<JobInfo>> {
        let query = if let Some(state) = state_filter {
            format!(
                "SELECT job_id, state, request, result, error, provider,
                        payment_amount, payment_bolt11, submitted_at, completed_at
                 FROM jobs WHERE state = '{}' ORDER BY submitted_at DESC {}",
                state_to_string(state),
                limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default()
            )
        } else {
            format!(
                "SELECT job_id, state, request, result, error, provider,
                        payment_amount, payment_bolt11, submitted_at, completed_at
                 FROM jobs ORDER BY submitted_at DESC {}",
                limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default()
            )
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], row_to_job_info)?;

        let mut jobs = Vec::new();
        for job_result in rows {
            jobs.push(job_result?);
        }

        Ok(jobs)
    }

    /// Delete old jobs (older than days)
    pub fn delete_old_jobs(&self, days: u64) -> SqliteResult<usize> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (days * 24 * 3600);

        self.conn.execute(
            "DELETE FROM jobs WHERE submitted_at < ?1",
            params![cutoff as i64],
        )
    }

    /// Get job count by state
    pub fn count_jobs_by_state(&self) -> SqliteResult<Vec<(JobState, usize)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT state, COUNT(*) FROM jobs GROUP BY state")?;

        let rows = stmt.query_map([], |row| {
            let state_str: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((string_to_state(&state_str), count as usize))
        })?;

        let mut counts = Vec::new();
        for row_result in rows {
            counts.push(row_result?);
        }

        Ok(counts)
    }
}

/// Convert JobState to string for storage
fn state_to_string(state: JobState) -> &'static str {
    match state {
        JobState::Pending => "pending",
        JobState::PaymentRequired => "payment_required",
        JobState::Processing => "processing",
        JobState::Completed => "completed",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

/// Convert string to JobState
fn string_to_state(s: &str) -> JobState {
    match s {
        "pending" => JobState::Pending,
        "payment_required" => JobState::PaymentRequired,
        "processing" => JobState::Processing,
        "completed" => JobState::Completed,
        "failed" => JobState::Failed,
        "cancelled" => JobState::Cancelled,
        _ => JobState::Pending, // Default fallback
    }
}

/// Convert database row to JobInfo
fn row_to_job_info(row: &rusqlite::Row) -> SqliteResult<JobInfo> {
    Ok(JobInfo {
        job_id: row.get(0)?,
        state: string_to_state(&row.get::<_, String>(1)?),
        request: row.get(2)?,
        result: row.get(3)?,
        error: row.get(4)?,
        provider: row.get(5)?,
        payment_amount: row.get(6)?,
        payment_bolt11: row.get(7)?,
        submitted_at: row.get::<_, i64>(8)? as u64,
        completed_at: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_database() {
        let db = JobDatabase::new_in_memory().unwrap();
        assert!(db.conn.is_autocommit());
    }

    #[test]
    fn test_save_and_get_job() {
        let db = JobDatabase::new_in_memory().unwrap();

        let job = JobInfo::new("job123", "test request");

        db.save_job(&job).unwrap();

        let loaded = db.get_job("job123").unwrap().unwrap();
        assert_eq!(loaded.job_id, "job123");
        assert_eq!(loaded.state, JobState::Pending);
        assert_eq!(loaded.request, "test request");
    }

    #[test]
    fn test_update_job() {
        let db = JobDatabase::new_in_memory().unwrap();

        let mut job = JobInfo::new("job456", "test");
        db.save_job(&job).unwrap();

        // Update job - manually set to completed
        job.state = JobState::Completed;
        job.result = Some("result data".to_string());
        job.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        db.save_job(&job).unwrap();

        let loaded = db.get_job("job456").unwrap().unwrap();
        assert_eq!(loaded.state, JobState::Completed);
        assert_eq!(loaded.result, Some("result data".to_string()));
        assert!(loaded.completed_at.is_some());
    }

    #[test]
    fn test_get_jobs_with_filter() {
        let db = JobDatabase::new_in_memory().unwrap();

        // Create jobs with different states
        let mut job1 = JobInfo::new("job1", "req1");
        job1.state = JobState::Completed;
        job1.result = Some("result1".to_string());
        db.save_job(&job1).unwrap();

        let job2 = JobInfo::new("job2", "req2");
        db.save_job(&job2).unwrap();

        let mut job3 = JobInfo::new("job3", "req3");
        job3.mark_failed("error");
        db.save_job(&job3).unwrap();

        // Get all jobs
        let all_jobs = db.get_jobs(None, None).unwrap();
        assert_eq!(all_jobs.len(), 3);

        // Get only completed jobs
        let completed = db.get_jobs(Some(JobState::Completed), None).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].job_id, "job1");

        // Get only pending jobs
        let pending = db.get_jobs(Some(JobState::Pending), None).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].job_id, "job2");
    }

    #[test]
    fn test_get_jobs_with_limit() {
        let db = JobDatabase::new_in_memory().unwrap();

        // Create multiple jobs
        for i in 1..=10 {
            let job = JobInfo::new(format!("job{}", i), "test");
            db.save_job(&job).unwrap();
        }

        // Get with limit
        let limited = db.get_jobs(None, Some(5)).unwrap();
        assert_eq!(limited.len(), 5);
    }

    #[test]
    fn test_delete_old_jobs() {
        let db = JobDatabase::new_in_memory().unwrap();

        // Create an old job (simulate by setting old timestamp)
        let mut old_job = JobInfo::new("old_job", "test");
        old_job.submitted_at = 0; // Very old timestamp
        db.save_job(&old_job).unwrap();

        // Create a recent job
        let new_job = JobInfo::new("new_job", "test");
        db.save_job(&new_job).unwrap();

        // Delete jobs older than 30 days
        let deleted = db.delete_old_jobs(30).unwrap();
        assert_eq!(deleted, 1);

        // Verify only new job remains
        let jobs = db.get_jobs(None, None).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "new_job");
    }

    #[test]
    fn test_count_jobs_by_state() {
        let db = JobDatabase::new_in_memory().unwrap();

        // Create jobs with different states
        let mut job1 = JobInfo::new("job1", "req1");
        job1.state = JobState::Completed;
        job1.result = Some("result".to_string());
        db.save_job(&job1).unwrap();

        let mut job2 = JobInfo::new("job2", "req2");
        job2.state = JobState::Completed;
        job2.result = Some("result".to_string());
        db.save_job(&job2).unwrap();

        let job3 = JobInfo::new("job3", "req3");
        db.save_job(&job3).unwrap();

        let mut job4 = JobInfo::new("job4", "req4");
        job4.mark_failed("error");
        db.save_job(&job4).unwrap();

        let counts = db.count_jobs_by_state().unwrap();
        assert_eq!(counts.len(), 3); // 3 different states

        // Find specific counts
        for (state, count) in counts {
            match state {
                JobState::Completed => assert_eq!(count, 2),
                JobState::Pending => assert_eq!(count, 1),
                JobState::Failed => assert_eq!(count, 1),
                _ => panic!("Unexpected state"),
            }
        }
    }

    #[test]
    fn test_nonexistent_job() {
        let db = JobDatabase::new_in_memory().unwrap();
        let result = db.get_job("nonexistent").unwrap();
        assert!(result.is_none());
    }
}
