//! Earnings tracking for compute provider

use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Tracks earnings from compute jobs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EarningsTracker {
    /// Total sats earned today
    pub today_sats: u64,
    /// Total sats earned this week
    pub week_sats: u64,
    /// Total sats earned all time
    pub all_time_sats: u64,
    /// Number of jobs completed today
    pub jobs_today: u32,
    /// Number of jobs completed this week
    pub jobs_week: u32,
    /// Number of jobs completed all time
    pub jobs_all_time: u32,
    /// When the "today" period started
    pub day_start: Option<DateTime<Utc>>,
    /// When the "week" period started
    pub week_start: Option<DateTime<Utc>>,
}

impl EarningsTracker {
    /// Create a new earnings tracker
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            today_sats: 0,
            week_sats: 0,
            all_time_sats: 0,
            jobs_today: 0,
            jobs_week: 0,
            jobs_all_time: 0,
            day_start: Some(start_of_day(now)),
            week_start: Some(start_of_week(now)),
        }
    }

    /// Record a payment received for a job
    pub fn record_payment(&mut self, amount_msats: u64) {
        let amount_sats = amount_msats / 1000;
        self.maybe_roll_over_periods();

        self.today_sats += amount_sats;
        self.week_sats += amount_sats;
        self.all_time_sats += amount_sats;
        self.jobs_today += 1;
        self.jobs_week += 1;
        self.jobs_all_time += 1;
    }

    /// Check and roll over day/week periods if needed
    fn maybe_roll_over_periods(&mut self) {
        let now = Utc::now();
        let today_start = start_of_day(now);
        let week_start = start_of_week(now);

        // Roll over day
        if self.day_start.is_none() || self.day_start.unwrap() < today_start {
            self.today_sats = 0;
            self.jobs_today = 0;
            self.day_start = Some(today_start);
        }

        // Roll over week
        if self.week_start.is_none() || self.week_start.unwrap() < week_start {
            self.week_sats = 0;
            self.jobs_week = 0;
            self.week_start = Some(week_start);
        }
    }

    /// Get average sats per job (all time)
    pub fn avg_sats_per_job(&self) -> u64 {
        if self.jobs_all_time == 0 {
            0
        } else {
            self.all_time_sats / self.jobs_all_time as u64
        }
    }

    /// Format sats for display (with commas)
    pub fn format_sats(sats: u64) -> String {
        let s = sats.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.insert(0, ',');
            }
            result.insert(0, c);
        }
        result
    }

    /// Get today's earnings formatted
    pub fn today_display(&self) -> String {
        Self::format_sats(self.today_sats)
    }

    /// Get week's earnings formatted
    pub fn week_display(&self) -> String {
        Self::format_sats(self.week_sats)
    }

    /// Get all-time earnings formatted
    pub fn all_time_display(&self) -> String {
        Self::format_sats(self.all_time_sats)
    }
}

/// Get the start of the current day (UTC midnight)
fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// Get the start of the current week (Monday UTC midnight)
fn start_of_week(dt: DateTime<Utc>) -> DateTime<Utc> {
    let days_since_monday = dt.weekday().num_days_from_monday();
    let monday = dt - Duration::days(days_since_monday as i64);
    start_of_day(monday)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_payment() {
        let mut tracker = EarningsTracker::new();

        tracker.record_payment(100_000); // 100 sats
        assert_eq!(tracker.today_sats, 100);
        assert_eq!(tracker.jobs_today, 1);
        assert_eq!(tracker.all_time_sats, 100);

        tracker.record_payment(50_000); // 50 sats
        assert_eq!(tracker.today_sats, 150);
        assert_eq!(tracker.jobs_today, 2);
        assert_eq!(tracker.all_time_sats, 150);
    }

    #[test]
    fn test_format_sats() {
        assert_eq!(EarningsTracker::format_sats(0), "0");
        assert_eq!(EarningsTracker::format_sats(100), "100");
        assert_eq!(EarningsTracker::format_sats(1_000), "1,000");
        assert_eq!(EarningsTracker::format_sats(1_000_000), "1,000,000");
    }

    #[test]
    fn test_avg_sats_per_job() {
        let mut tracker = EarningsTracker::new();
        assert_eq!(tracker.avg_sats_per_job(), 0);

        tracker.record_payment(100_000);
        tracker.record_payment(200_000);
        assert_eq!(tracker.avg_sats_per_job(), 150);
    }
}
