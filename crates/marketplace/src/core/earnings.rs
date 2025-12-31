//! Real-time earnings tracking with minute-level buckets
//!
//! Provides infrastructure for tracking marketplace earnings in real-time using
//! minute-level revenue buckets. Enables dashboard visualizations and export for
//! accounting purposes.
//!
//! # Revenue Bucket System
//!
//! Earnings are tracked in minute-level buckets to enable:
//! - Real-time dashboard updates
//! - Time-series analysis and visualization
//! - Efficient aggregation for different time periods (day/week/month)
//! - Export for accounting and tax purposes
//!
//! Each bucket stores:
//! - Gross revenue (total payment received)
//! - Split amounts (creator/compute/platform/referrer)
//! - Revenue source type (compute/skill/data/trajectory)
//! - Associated item ID for drill-down analysis

use super::revenue::{RevenueSplit, RevenueSplitConfig};
use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Revenue bucket for minute-level earnings tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueBucket {
    /// Unique bucket ID
    pub id: String,
    /// Bucket timestamp (unix seconds, rounded to minute)
    pub bucket_minute: u64,
    /// Revenue type (compute, skill, data, trajectory)
    pub revenue_type: RevenueType,
    /// Associated item ID (job_id, skill_id, etc.)
    pub item_id: String,
    /// Gross revenue in satoshis
    pub gross_sats: u64,
    /// Creator's share in satoshis
    pub creator_sats: u64,
    /// Compute provider's share in satoshis
    pub compute_sats: u64,
    /// Platform's share in satoshis
    pub platform_sats: u64,
    /// Referrer's share in satoshis (0 if no referrer)
    pub referrer_sats: Option<u64>,
    /// Revenue split version used
    pub split_version: u32,
    /// Created timestamp (unix seconds)
    pub created_at: u64,
}

/// Revenue type for marketplace earnings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevenueType {
    /// Compute job revenue
    Compute,
    /// Skill license revenue
    Skill,
    /// Data access revenue
    Data,
    /// Trajectory contribution revenue
    Trajectory,
}

impl RevenueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RevenueType::Compute => "compute",
            RevenueType::Skill => "skill",
            RevenueType::Data => "data",
            RevenueType::Trajectory => "trajectory",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "compute" => Some(RevenueType::Compute),
            "skill" => Some(RevenueType::Skill),
            "data" => Some(RevenueType::Data),
            "trajectory" => Some(RevenueType::Trajectory),
            _ => None,
        }
    }
}

/// Earnings statistics for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsStats {
    /// Total gross earnings
    pub total_gross_sats: u64,
    /// Earnings by type
    pub by_type: EarningsByType,
    /// Earnings over time periods
    pub by_period: EarningsByPeriod,
}

/// Earnings broken down by revenue type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsByType {
    pub compute_sats: u64,
    pub skill_sats: u64,
    pub data_sats: u64,
    pub trajectory_sats: u64,
}

/// Earnings broken down by time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsByPeriod {
    pub last_hour_sats: u64,
    pub last_day_sats: u64,
    pub last_week_sats: u64,
    pub last_month_sats: u64,
}

/// Earnings tracker for real-time revenue recording
pub struct EarningsTracker {
    split_config: RevenueSplitConfig,
}

impl EarningsTracker {
    /// Create a new earnings tracker
    pub fn new(split_config: RevenueSplitConfig) -> Self {
        Self { split_config }
    }

    /// Create with default revenue split configuration
    pub fn with_defaults() -> Self {
        Self::new(RevenueSplitConfig::default())
    }

    /// Record earnings for a completed transaction
    ///
    /// # Arguments
    /// * `conn` - Database connection
    /// * `revenue_type` - Type of revenue (compute/skill/data/trajectory)
    /// * `item_id` - Associated item ID
    /// * `gross_sats` - Gross revenue amount in satoshis
    /// * `has_referrer` - Whether there was a referrer to pay
    ///
    /// # Returns
    /// The created revenue bucket
    pub fn record_earnings(
        &self,
        conn: &Connection,
        revenue_type: RevenueType,
        item_id: &str,
        gross_sats: u64,
        has_referrer: bool,
    ) -> Result<RevenueBucket> {
        // Calculate revenue split
        let split = RevenueSplit::calculate(gross_sats, &self.split_config, has_referrer);

        // Get current timestamp and round to minute
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let bucket_minute = (now / 60) * 60; // Round down to minute

        // Create bucket record
        let bucket = RevenueBucket {
            id: uuid::Uuid::new_v4().to_string(),
            bucket_minute,
            revenue_type,
            item_id: item_id.to_string(),
            gross_sats: split.gross_sats,
            creator_sats: split.creator_sats,
            compute_sats: split.compute_sats,
            platform_sats: split.platform_sats,
            referrer_sats: if has_referrer {
                Some(split.referrer_sats)
            } else {
                None
            },
            split_version: 1, // Version 1 uses default RevenueSplitConfig
            created_at: now,
        };

        // Insert into database
        conn.execute(
            r#"
            INSERT INTO revenue_buckets (
                id, bucket_minute, type, item_id, gross_sats,
                creator_sats, compute_sats, platform_sats, referrer_sats,
                split_version, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(bucket_minute, type, item_id) DO UPDATE SET
                gross_sats = gross_sats + excluded.gross_sats,
                creator_sats = creator_sats + excluded.creator_sats,
                compute_sats = compute_sats + excluded.compute_sats,
                platform_sats = platform_sats + excluded.platform_sats,
                referrer_sats = COALESCE(referrer_sats, 0) + COALESCE(excluded.referrer_sats, 0)
            "#,
            params![
                bucket.id,
                bucket.bucket_minute,
                bucket.revenue_type.as_str(),
                bucket.item_id,
                bucket.gross_sats,
                bucket.creator_sats,
                bucket.compute_sats,
                bucket.platform_sats,
                bucket.referrer_sats,
                bucket.split_version,
                bucket.created_at,
            ],
        )?;

        Ok(bucket)
    }

    /// Get earnings statistics for dashboard
    ///
    /// # Arguments
    /// * `conn` - Database connection
    ///
    /// # Returns
    /// Aggregated earnings statistics
    pub fn get_earnings_stats(&self, conn: &Connection) -> Result<EarningsStats> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Get total earnings
        let total_gross_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets",
            [],
            |row| row.get(0),
        )?;

        // Get earnings by type
        let compute_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE type = 'compute'",
            [],
            |row| row.get(0),
        )?;

        let skill_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE type = 'skill'",
            [],
            |row| row.get(0),
        )?;

        let data_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE type = 'data'",
            [],
            |row| row.get(0),
        )?;

        let trajectory_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE type = 'trajectory'",
            [],
            |row| row.get(0),
        )?;

        // Get earnings by time period
        let one_hour_ago = now - 3600;
        let one_day_ago = now - 86400;
        let one_week_ago = now - 604800;
        let one_month_ago = now - 2592000;

        let last_hour_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE bucket_minute >= ?1",
            params![one_hour_ago],
            |row| row.get(0),
        )?;

        let last_day_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE bucket_minute >= ?1",
            params![one_day_ago],
            |row| row.get(0),
        )?;

        let last_week_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE bucket_minute >= ?1",
            params![one_week_ago],
            |row| row.get(0),
        )?;

        let last_month_sats: u64 = conn.query_row(
            "SELECT COALESCE(SUM(gross_sats), 0) FROM revenue_buckets WHERE bucket_minute >= ?1",
            params![one_month_ago],
            |row| row.get(0),
        )?;

        Ok(EarningsStats {
            total_gross_sats,
            by_type: EarningsByType {
                compute_sats,
                skill_sats,
                data_sats,
                trajectory_sats,
            },
            by_period: EarningsByPeriod {
                last_hour_sats,
                last_day_sats,
                last_week_sats,
                last_month_sats,
            },
        })
    }

    /// Get recent revenue buckets for history display
    ///
    /// # Arguments
    /// * `conn` - Database connection
    /// * `limit` - Maximum number of buckets to return
    /// * `revenue_type` - Optional filter by revenue type
    ///
    /// # Returns
    /// List of recent revenue buckets
    pub fn get_recent_buckets(
        &self,
        conn: &Connection,
        limit: usize,
        revenue_type: Option<RevenueType>,
    ) -> Result<Vec<RevenueBucket>> {
        let query = if let Some(rev_type) = revenue_type {
            format!(
                "SELECT id, bucket_minute, type, item_id, gross_sats, creator_sats, \
                 compute_sats, platform_sats, referrer_sats, split_version, created_at \
                 FROM revenue_buckets WHERE type = '{}' \
                 ORDER BY bucket_minute DESC LIMIT ?1",
                rev_type.as_str()
            )
        } else {
            "SELECT id, bucket_minute, type, item_id, gross_sats, creator_sats, \
             compute_sats, platform_sats, referrer_sats, split_version, created_at \
             FROM revenue_buckets ORDER BY bucket_minute DESC LIMIT ?1"
                .to_string()
        };

        let mut stmt = conn.prepare(&query)?;
        let buckets = stmt
            .query_map(params![limit], |row| {
                Ok(RevenueBucket {
                    id: row.get(0)?,
                    bucket_minute: row.get(1)?,
                    revenue_type: RevenueType::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(RevenueType::Compute),
                    item_id: row.get(3)?,
                    gross_sats: row.get(4)?,
                    creator_sats: row.get(5)?,
                    compute_sats: row.get(6)?,
                    platform_sats: row.get(7)?,
                    referrer_sats: row.get(8)?,
                    split_version: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(buckets)
    }

    /// Export earnings data for accounting
    ///
    /// # Arguments
    /// * `conn` - Database connection
    /// * `from_timestamp` - Start of date range (unix seconds)
    /// * `to_timestamp` - End of date range (unix seconds)
    ///
    /// # Returns
    /// List of revenue buckets in the specified range
    pub fn export_earnings(
        &self,
        conn: &Connection,
        from_timestamp: u64,
        to_timestamp: u64,
    ) -> Result<Vec<RevenueBucket>> {
        let mut stmt = conn.prepare(
            "SELECT id, bucket_minute, type, item_id, gross_sats, creator_sats, \
             compute_sats, platform_sats, referrer_sats, split_version, created_at \
             FROM revenue_buckets \
             WHERE bucket_minute >= ?1 AND bucket_minute <= ?2 \
             ORDER BY bucket_minute ASC",
        )?;

        let buckets = stmt
            .query_map(params![from_timestamp, to_timestamp], |row| {
                Ok(RevenueBucket {
                    id: row.get(0)?,
                    bucket_minute: row.get(1)?,
                    revenue_type: RevenueType::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(RevenueType::Compute),
                    item_id: row.get(3)?,
                    gross_sats: row.get(4)?,
                    creator_sats: row.get(5)?,
                    compute_sats: row.get(6)?,
                    platform_sats: row.get(7)?,
                    referrer_sats: row.get(8)?,
                    split_version: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(buckets)
    }

    /// Export earnings as CSV for accounting
    ///
    /// # Arguments
    /// * `conn` - Database connection
    /// * `from_timestamp` - Start of date range (unix seconds)
    /// * `to_timestamp` - End of date range (unix seconds)
    ///
    /// # Returns
    /// CSV-formatted string with earnings data
    pub fn export_as_csv(
        &self,
        conn: &Connection,
        from_timestamp: u64,
        to_timestamp: u64,
    ) -> Result<String> {
        let buckets = self.export_earnings(conn, from_timestamp, to_timestamp)?;

        let mut csv = String::from(
            "bucket_time,revenue_type,item_id,gross_sats,creator_sats,compute_sats,platform_sats,referrer_sats\n",
        );

        for bucket in buckets {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                bucket.bucket_minute,
                bucket.revenue_type.as_str(),
                bucket.item_id,
                bucket.gross_sats,
                bucket.creator_sats,
                bucket.compute_sats,
                bucket.platform_sats,
                bucket.referrer_sats.unwrap_or(0),
            ));
        }

        Ok(csv)
    }

    /// Export earnings as JSON for accounting
    ///
    /// # Arguments
    /// * `conn` - Database connection
    /// * `from_timestamp` - Start of date range (unix seconds)
    /// * `to_timestamp` - End of date range (unix seconds)
    ///
    /// # Returns
    /// JSON-formatted string with earnings data
    pub fn export_as_json(
        &self,
        conn: &Connection,
        from_timestamp: u64,
        to_timestamp: u64,
    ) -> Result<String> {
        let buckets = self.export_earnings(conn, from_timestamp, to_timestamp)?;
        let json = serde_json::to_string_pretty(&buckets)?;
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;

    #[test]
    fn test_record_earnings_single() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        let bucket = tracker
            .record_earnings(&conn, RevenueType::Compute, "job-123", 100_000, false)
            .unwrap();

        assert_eq!(bucket.gross_sats, 100_000);
        assert_eq!(bucket.revenue_type, RevenueType::Compute);
        assert_eq!(bucket.item_id, "job-123");
        assert!(bucket.creator_sats > 0);
        assert!(bucket.compute_sats > 0);
        assert!(bucket.platform_sats > 0);
    }

    #[test]
    fn test_record_earnings_with_referrer() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        let bucket = tracker
            .record_earnings(&conn, RevenueType::Skill, "skill-456", 50_000, true)
            .unwrap();

        assert_eq!(bucket.gross_sats, 50_000);
        assert!(bucket.referrer_sats.is_some());
        assert!(bucket.referrer_sats.unwrap() > 0);
    }

    #[test]
    fn test_earnings_stats() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        // Record multiple earnings
        tracker
            .record_earnings(&conn, RevenueType::Compute, "job-1", 100_000, false)
            .unwrap();
        tracker
            .record_earnings(&conn, RevenueType::Skill, "skill-1", 50_000, false)
            .unwrap();
        tracker
            .record_earnings(&conn, RevenueType::Data, "data-1", 75_000, false)
            .unwrap();

        let stats = tracker.get_earnings_stats(&conn).unwrap();

        assert_eq!(stats.total_gross_sats, 225_000);
        assert_eq!(stats.by_type.compute_sats, 100_000);
        assert_eq!(stats.by_type.skill_sats, 50_000);
        assert_eq!(stats.by_type.data_sats, 75_000);
        assert_eq!(stats.by_period.last_hour_sats, 225_000);
    }

    #[test]
    fn test_get_recent_buckets() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        // Record earnings
        tracker
            .record_earnings(&conn, RevenueType::Compute, "job-1", 100_000, false)
            .unwrap();
        tracker
            .record_earnings(&conn, RevenueType::Skill, "skill-1", 50_000, false)
            .unwrap();

        let buckets = tracker.get_recent_buckets(&conn, 10, None).unwrap();
        assert_eq!(buckets.len(), 2);

        // Filter by type
        let compute_buckets = tracker
            .get_recent_buckets(&conn, 10, Some(RevenueType::Compute))
            .unwrap();
        assert_eq!(compute_buckets.len(), 1);
        assert_eq!(compute_buckets[0].revenue_type, RevenueType::Compute);
    }

    #[test]
    fn test_export_earnings_csv() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        tracker
            .record_earnings(&conn, RevenueType::Compute, "job-1", 100_000, false)
            .unwrap();

        let csv = tracker.export_as_csv(&conn, 0, i64::MAX as u64).unwrap();
        assert!(csv.contains("bucket_time,revenue_type"));
        assert!(csv.contains("compute,job-1,100000"));
    }

    #[test]
    fn test_export_earnings_json() {
        let conn = init_memory_db().unwrap();
        let tracker = EarningsTracker::with_defaults();

        tracker
            .record_earnings(&conn, RevenueType::Skill, "skill-1", 50_000, false)
            .unwrap();

        let json = tracker.export_as_json(&conn, 0, i64::MAX as u64).unwrap();
        assert!(json.contains("\"revenue_type\""));
        assert!(json.contains("\"skill\""));
        // to_string_pretty adds space after colon
        assert!(json.contains("\"gross_sats\": 50000"));
    }

    #[test]
    fn test_revenue_type_conversions() {
        assert_eq!(RevenueType::Compute.as_str(), "compute");
        assert_eq!(RevenueType::Skill.as_str(), "skill");
        assert_eq!(RevenueType::Data.as_str(), "data");
        assert_eq!(RevenueType::Trajectory.as_str(), "trajectory");

        assert_eq!(RevenueType::from_str("compute"), Some(RevenueType::Compute));
        assert_eq!(RevenueType::from_str("skill"), Some(RevenueType::Skill));
        assert_eq!(RevenueType::from_str("data"), Some(RevenueType::Data));
        assert_eq!(
            RevenueType::from_str("trajectory"),
            Some(RevenueType::Trajectory)
        );
        assert_eq!(RevenueType::from_str("invalid"), None);
    }
}
