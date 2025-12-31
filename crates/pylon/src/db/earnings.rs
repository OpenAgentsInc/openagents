//! Earnings tracking for provider mode

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::PylonDb;

/// Earning source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EarningSource {
    Job,
    Tip,
    Other,
}

impl EarningSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            EarningSource::Job => "job",
            EarningSource::Tip => "tip",
            EarningSource::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "job" => Some(EarningSource::Job),
            "tip" => Some(EarningSource::Tip),
            "other" => Some(EarningSource::Other),
            _ => None,
        }
    }
}

/// An earning record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Earning {
    pub id: String,
    pub job_id: Option<String>,
    pub amount_msats: u64,
    pub source: EarningSource,
    pub payment_hash: Option<String>,
    pub preimage: Option<String>,
    pub earned_at: u64,
}

/// Earnings summary statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EarningsSummary {
    pub total_msats: u64,
    pub total_sats: u64,
    pub job_count: u64,
    pub by_source: HashMap<String, u64>,
}

impl PylonDb {
    /// Record an earning
    pub fn record_earning(&self, earning: &Earning) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO earnings (id, job_id, amount_msats, source, payment_hash, preimage, earned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                earning.id,
                earning.job_id,
                earning.amount_msats as i64,
                earning.source.as_str(),
                earning.payment_hash,
                earning.preimage,
                earning.earned_at as i64,
            ],
        )?;
        Ok(())
    }

    /// Record an earning from a completed job
    pub fn record_job_earning(
        &self,
        job_id: &str,
        amount_msats: u64,
        payment_hash: Option<&str>,
        preimage: Option<&str>,
    ) -> anyhow::Result<String> {
        let id = format!("earn-{}", uuid::Uuid::new_v4());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let earning = Earning {
            id: id.clone(),
            job_id: Some(job_id.to_string()),
            amount_msats,
            source: EarningSource::Job,
            payment_hash: payment_hash.map(|s| s.to_string()),
            preimage: preimage.map(|s| s.to_string()),
            earned_at: now,
        };

        self.record_earning(&earning)?;
        Ok(id)
    }

    /// Get total earnings
    pub fn get_total_earnings(&self) -> anyhow::Result<u64> {
        let total: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(amount_msats), 0) FROM earnings",
            [],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// Get earnings summary
    pub fn get_earnings_summary(&self) -> anyhow::Result<EarningsSummary> {
        let total_msats: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(amount_msats), 0) FROM earnings",
            [],
            |row| row.get(0),
        )?;

        let job_count: i64 = self.conn().query_row(
            "SELECT COUNT(DISTINCT job_id) FROM earnings WHERE job_id IS NOT NULL",
            [],
            |row| row.get(0),
        )?;

        // Get breakdown by source
        let mut by_source = HashMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT source, SUM(amount_msats) FROM earnings GROUP BY source")?;

        let rows = stmt.query_map([], |row| {
            let source: String = row.get(0)?;
            let amount: i64 = row.get(1)?;
            Ok((source, amount))
        })?;

        for row in rows {
            let (source, amount) = row?;
            by_source.insert(source, amount as u64);
        }

        Ok(EarningsSummary {
            total_msats: total_msats as u64,
            total_sats: total_msats as u64 / 1000,
            job_count: job_count as u64,
            by_source,
        })
    }

    /// Get earnings within a time range
    pub fn get_earnings_in_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> anyhow::Result<Vec<Earning>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, job_id, amount_msats, source, payment_hash, preimage, earned_at
             FROM earnings
             WHERE earned_at >= ?1 AND earned_at <= ?2
             ORDER BY earned_at DESC",
        )?;

        let earnings = stmt
            .query_map(params![start_time as i64, end_time as i64], |row| {
                Ok(Earning {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    amount_msats: row.get::<_, i64>(2)? as u64,
                    source: EarningSource::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(EarningSource::Other),
                    payment_hash: row.get(4)?,
                    preimage: row.get(5)?,
                    earned_at: row.get::<_, i64>(6)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(earnings)
    }

    /// Get today's earnings
    pub fn get_today_earnings(&self) -> anyhow::Result<u64> {
        // Start of today (UTC)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_of_day = now - (now % 86400);

        let total: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(amount_msats), 0) FROM earnings WHERE earned_at >= ?",
            [start_of_day as i64],
            |row| row.get(0),
        )?;

        Ok(total as u64)
    }

    /// Get last N earnings
    pub fn get_recent_earnings(&self, limit: usize) -> anyhow::Result<Vec<Earning>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, job_id, amount_msats, source, payment_hash, preimage, earned_at
             FROM earnings
             ORDER BY earned_at DESC
             LIMIT ?",
        )?;

        let earnings = stmt
            .query_map([limit as i64], |row| {
                Ok(Earning {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    amount_msats: row.get::<_, i64>(2)? as u64,
                    source: EarningSource::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(EarningSource::Other),
                    payment_hash: row.get(4)?,
                    preimage: row.get(5)?,
                    earned_at: row.get::<_, i64>(6)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(earnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::jobs::{Job, JobStatus};

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn create_test_job(db: &PylonDb, id: &str) {
        let job = Job {
            id: id.to_string(),
            kind: 5100,
            customer_pubkey: "abc123".to_string(),
            status: JobStatus::Completed,
            price_msats: 1000,
            input_hash: None,
            output_hash: None,
            error_message: None,
            started_at: now(),
            completed_at: Some(now()),
            created_at: now(),
        };
        db.create_job(&job).unwrap();
    }

    #[test]
    fn test_record_earning() {
        let db = PylonDb::open_in_memory().unwrap();

        // Create job first (for foreign key)
        create_test_job(&db, "job-1");

        let earning = Earning {
            id: "earn-1".to_string(),
            job_id: Some("job-1".to_string()),
            amount_msats: 5000,
            source: EarningSource::Job,
            payment_hash: Some("hash123".to_string()),
            preimage: None,
            earned_at: now(),
        };

        db.record_earning(&earning).unwrap();

        let total = db.get_total_earnings().unwrap();
        assert_eq!(total, 5000);
    }

    #[test]
    fn test_record_earning_without_job() {
        let db = PylonDb::open_in_memory().unwrap();

        // Test earning without job_id (tips, etc.)
        let earning = Earning {
            id: "earn-tip-1".to_string(),
            job_id: None,
            amount_msats: 1000,
            source: EarningSource::Tip,
            payment_hash: None,
            preimage: None,
            earned_at: now(),
        };

        db.record_earning(&earning).unwrap();

        let total = db.get_total_earnings().unwrap();
        assert_eq!(total, 1000);
    }

    #[test]
    fn test_earnings_summary() {
        let db = PylonDb::open_in_memory().unwrap();

        // Create jobs first (for foreign key)
        for i in 0..3 {
            create_test_job(&db, &format!("job-{}", i));
        }

        // Record multiple earnings
        for i in 0..3 {
            let earning = Earning {
                id: format!("earn-{}", i),
                job_id: Some(format!("job-{}", i)),
                amount_msats: 1000 * (i as u64 + 1),
                source: EarningSource::Job,
                payment_hash: None,
                preimage: None,
                earned_at: now(),
            };
            db.record_earning(&earning).unwrap();
        }

        let summary = db.get_earnings_summary().unwrap();
        assert_eq!(summary.total_msats, 6000); // 1000 + 2000 + 3000
        assert_eq!(summary.total_sats, 6);
        assert_eq!(summary.job_count, 3);
    }
}
