//! Unit tests for EarningsTracker domain module

use chrono::{Duration, Utc};
use compute::domain::EarningsTracker;

// =========================================================================
// EarningsTracker initialization
// =========================================================================

#[test]
fn test_new_earnings_tracker() {
    let tracker = EarningsTracker::new();

    assert_eq!(tracker.today_sats, 0);
    assert_eq!(tracker.week_sats, 0);
    assert_eq!(tracker.all_time_sats, 0);
    assert_eq!(tracker.jobs_today, 0);
    assert_eq!(tracker.jobs_week, 0);
    assert_eq!(tracker.jobs_all_time, 0);
    assert!(tracker.day_start.is_some());
    assert!(tracker.week_start.is_some());
}

#[test]
fn test_default_earnings_tracker() {
    let tracker = EarningsTracker::default();

    assert_eq!(tracker.today_sats, 0);
    assert_eq!(tracker.all_time_sats, 0);
}

// =========================================================================
// Payment recording
// =========================================================================

#[test]
fn test_record_single_payment() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000); // 100 sats

    assert_eq!(tracker.today_sats, 100);
    assert_eq!(tracker.week_sats, 100);
    assert_eq!(tracker.all_time_sats, 100);
    assert_eq!(tracker.jobs_today, 1);
    assert_eq!(tracker.jobs_week, 1);
    assert_eq!(tracker.jobs_all_time, 1);
}

#[test]
fn test_record_multiple_payments() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000); // 100 sats
    tracker.record_payment(50_000); // 50 sats
    tracker.record_payment(25_000); // 25 sats

    assert_eq!(tracker.today_sats, 175);
    assert_eq!(tracker.week_sats, 175);
    assert_eq!(tracker.all_time_sats, 175);
    assert_eq!(tracker.jobs_today, 3);
    assert_eq!(tracker.jobs_week, 3);
    assert_eq!(tracker.jobs_all_time, 3);
}

#[test]
fn test_record_zero_payment() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(0);

    assert_eq!(tracker.today_sats, 0);
    assert_eq!(tracker.jobs_today, 1); // Job still counts
}

#[test]
fn test_record_large_payment() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(1_000_000_000); // 1 million sats

    assert_eq!(tracker.today_sats, 1_000_000);
    assert_eq!(tracker.all_time_sats, 1_000_000);
}

#[test]
fn test_msats_to_sats_conversion() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(1_500); // 1.5 sats (1 sat after truncation)

    assert_eq!(tracker.today_sats, 1);
}

#[test]
fn test_fractional_sats_truncated() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(999); // < 1 sat

    assert_eq!(tracker.today_sats, 0); // Truncated to 0
    assert_eq!(tracker.jobs_today, 1); // But job still counts
}

// =========================================================================
// Average calculation
// =========================================================================

#[test]
fn test_avg_sats_per_job_zero_jobs() {
    let tracker = EarningsTracker::new();
    assert_eq!(tracker.avg_sats_per_job(), 0);
}

#[test]
fn test_avg_sats_per_job_single() {
    let mut tracker = EarningsTracker::new();
    tracker.record_payment(100_000);

    assert_eq!(tracker.avg_sats_per_job(), 100);
}

#[test]
fn test_avg_sats_per_job_multiple() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000); // 100 sats
    tracker.record_payment(200_000); // 200 sats
    tracker.record_payment(300_000); // 300 sats

    assert_eq!(tracker.avg_sats_per_job(), 200);
}

#[test]
fn test_avg_sats_per_job_truncation() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000); // 100 sats
    tracker.record_payment(150_000); // 150 sats

    // Average is 125, but integer division
    assert_eq!(tracker.avg_sats_per_job(), 125);
}

// =========================================================================
// Formatting tests
// =========================================================================

#[test]
fn test_format_sats_zero() {
    assert_eq!(EarningsTracker::format_sats(0), "0");
}

#[test]
fn test_format_sats_small() {
    assert_eq!(EarningsTracker::format_sats(1), "1");
    assert_eq!(EarningsTracker::format_sats(99), "99");
    assert_eq!(EarningsTracker::format_sats(999), "999");
}

#[test]
fn test_format_sats_thousands() {
    assert_eq!(EarningsTracker::format_sats(1_000), "1,000");
    assert_eq!(EarningsTracker::format_sats(9_999), "9,999");
    assert_eq!(EarningsTracker::format_sats(12_345), "12,345");
}

#[test]
fn test_format_sats_millions() {
    assert_eq!(EarningsTracker::format_sats(1_000_000), "1,000,000");
    assert_eq!(EarningsTracker::format_sats(1_234_567), "1,234,567");
}

#[test]
fn test_format_sats_billions() {
    assert_eq!(EarningsTracker::format_sats(1_000_000_000), "1,000,000,000");
    assert_eq!(
        EarningsTracker::format_sats(21_000_000_000_000),
        "21,000,000,000,000"
    );
}

#[test]
fn test_today_display() {
    let mut tracker = EarningsTracker::new();
    tracker.record_payment(1_234_000); // 1,234 sats

    assert_eq!(tracker.today_display(), "1,234");
}

#[test]
fn test_week_display() {
    let mut tracker = EarningsTracker::new();
    tracker.record_payment(5_678_000); // 5,678 sats

    assert_eq!(tracker.week_display(), "5,678");
}

#[test]
fn test_all_time_display() {
    let mut tracker = EarningsTracker::new();
    tracker.record_payment(9_999_000); // 9,999 sats

    assert_eq!(tracker.all_time_display(), "9,999");
}

// =========================================================================
// Period rollover tests
// =========================================================================

#[test]
fn test_day_rollover_resets_today() {
    let mut tracker = EarningsTracker::new();

    // Set day_start to yesterday
    tracker.day_start = Some(Utc::now() - Duration::days(1));
    tracker.today_sats = 500;
    tracker.jobs_today = 5;

    // Record new payment (should trigger rollover)
    tracker.record_payment(100_000);

    assert_eq!(tracker.today_sats, 100); // Reset + new
    assert_eq!(tracker.jobs_today, 1); // Reset + new
    assert_eq!(tracker.all_time_sats, 100); // Only new payment (rollover doesn't preserve old today)
}

#[test]
fn test_week_rollover_resets_week() {
    let mut tracker = EarningsTracker::new();

    // Set week_start to last week
    tracker.week_start = Some(Utc::now() - Duration::days(7));
    tracker.week_sats = 1000;
    tracker.jobs_week = 10;

    // Record new payment (should trigger rollover)
    tracker.record_payment(200_000);

    assert_eq!(tracker.week_sats, 200); // Reset + new
    assert_eq!(tracker.jobs_week, 1); // Reset + new
}

#[test]
fn test_both_rollovers_simultaneously() {
    let mut tracker = EarningsTracker::new();

    // Set both to old dates
    tracker.day_start = Some(Utc::now() - Duration::days(8));
    tracker.week_start = Some(Utc::now() - Duration::days(8));
    tracker.today_sats = 100;
    tracker.week_sats = 500;
    tracker.jobs_today = 1;
    tracker.jobs_week = 5;

    tracker.record_payment(300_000);

    assert_eq!(tracker.today_sats, 300);
    assert_eq!(tracker.week_sats, 300);
    assert_eq!(tracker.jobs_today, 1);
    assert_eq!(tracker.jobs_week, 1);
}

#[test]
fn test_no_rollover_same_day() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000);
    tracker.record_payment(200_000);

    // Should accumulate, not reset
    assert_eq!(tracker.today_sats, 300);
    assert_eq!(tracker.jobs_today, 2);
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn test_maximum_u64_payment() {
    let mut tracker = EarningsTracker::new();

    // This would overflow if not careful, but our conversion should handle it
    tracker.record_payment(u64::MAX);

    // u64::MAX / 1000 = 18,446,744,073,709,551
    assert!(tracker.all_time_sats > 0);
}

#[test]
fn test_many_small_payments() {
    let mut tracker = EarningsTracker::new();

    for _ in 0..1000 {
        tracker.record_payment(1_000); // 1 sat each
    }

    assert_eq!(tracker.today_sats, 1000);
    assert_eq!(tracker.jobs_today, 1000);
}

#[test]
fn test_serialization() {
    let tracker = EarningsTracker::new();
    let serialized = serde_json::to_string(&tracker).unwrap();
    assert!(serialized.contains("today_sats"));
    assert!(serialized.contains("all_time_sats"));
}

#[test]
fn test_deserialization() {
    let mut tracker = EarningsTracker::new();
    tracker.record_payment(100_000);

    let serialized = serde_json::to_string(&tracker).unwrap();
    let deserialized: EarningsTracker = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.today_sats, tracker.today_sats);
    assert_eq!(deserialized.all_time_sats, tracker.all_time_sats);
    assert_eq!(deserialized.jobs_today, tracker.jobs_today);
}

#[test]
fn test_clone() {
    let mut tracker1 = EarningsTracker::new();
    tracker1.record_payment(100_000);

    let tracker2 = tracker1.clone();

    assert_eq!(tracker2.today_sats, tracker1.today_sats);
    assert_eq!(tracker2.all_time_sats, tracker1.all_time_sats);
}

// =========================================================================
// Time window consistency
// =========================================================================

#[test]
fn test_week_includes_today() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000);

    // Week should always include today
    assert_eq!(tracker.week_sats, tracker.today_sats);
}

#[test]
fn test_all_time_includes_week() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000);

    // All-time should always include week
    assert!(tracker.all_time_sats >= tracker.week_sats);
}

#[test]
fn test_all_time_includes_today() {
    let mut tracker = EarningsTracker::new();

    tracker.record_payment(100_000);

    // All-time should always include today
    assert!(tracker.all_time_sats >= tracker.today_sats);
}

// =========================================================================
// Realistic scenarios
// =========================================================================

#[test]
fn test_typical_day_scenario() {
    let mut tracker = EarningsTracker::new();

    // Simulate a day of work
    tracker.record_payment(5_000); // 5 sats
    tracker.record_payment(10_000); // 10 sats
    tracker.record_payment(7_500); // 7.5 sats (truncated to 7)
    tracker.record_payment(15_000); // 15 sats
    tracker.record_payment(20_000); // 20 sats

    assert_eq!(tracker.jobs_today, 5);
    assert_eq!(tracker.today_sats, 57); // 5+10+7+15+20
    assert_eq!(tracker.avg_sats_per_job(), 11); // 57/5 = 11.4 truncated
}

#[test]
fn test_zero_earnings_day() {
    let tracker = EarningsTracker::new();

    assert_eq!(tracker.today_display(), "0");
    assert_eq!(tracker.week_display(), "0");
    assert_eq!(tracker.all_time_display(), "0");
}

#[test]
fn test_high_volume_day() {
    let mut tracker = EarningsTracker::new();

    // Simulate 100 jobs
    for _ in 0..100 {
        tracker.record_payment(10_000); // 10 sats each
    }

    assert_eq!(tracker.jobs_today, 100);
    assert_eq!(tracker.today_sats, 1_000);
    assert_eq!(tracker.today_display(), "1,000");
}
