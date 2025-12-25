//! Snapshot tests for autopilot dashboard UI rendering
//!
//! These tests capture HTML output from dashboard rendering functions
//! to catch unintended UI regressions when refactoring.

use crate::dashboard::{SummaryStats, dashboard_page, sessions_table, summary_card};
use crate::metrics::{SessionMetrics, SessionStatus};
use chrono::Utc;
use insta::assert_snapshot;

/// Create test session with specific state
fn create_test_session(
    id: &str,
    status: SessionStatus,
    issues_completed: i32,
    tool_errors: i32,
) -> SessionMetrics {
    // Use a fixed timestamp for deterministic snapshots
    let fixed_timestamp = chrono::DateTime::parse_from_rfc3339("2025-12-22T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    SessionMetrics {
        id: id.to_string(),
        timestamp: fixed_timestamp,
        model: "claude-sonnet-4-5".to_string(),
        prompt: "Test task".to_string(),
        duration_seconds: 300.0,
        tokens_in: 10000,
        tokens_out: 5000,
        tokens_cached: 2000,
        cost_usd: 1.50,
        issues_claimed: issues_completed + 1,
        issues_completed,
        tool_calls: 25,
        tool_errors,
        final_status: status,
        messages: 10,
        apm: Some(20.0), // (10 messages + 25 tool_calls) / 5 minutes = 7.0 APM
        source: "autopilot".to_string(),
        issue_numbers: None,
        directive_id: None,
    }
}

/// Create test summary stats
fn create_test_stats(total_sessions: i64, total_issues: i64, cost: f64) -> SummaryStats {
    SummaryStats {
        total_sessions,
        total_issues_completed: total_issues,
        total_cost_usd: cost,
        avg_duration_seconds: 250.0,
        avg_tokens_per_session: 15000.0,
        completion_rate: 0.85,
    }
}

#[test]
fn test_dashboard_empty_state() {
    let sessions = vec![];
    let stats = SummaryStats::default();

    let html = dashboard_page(&sessions, &stats);

    assert_snapshot!(html);
}

#[test]
fn test_dashboard_with_successful_session() {
    let sessions = vec![create_test_session(
        "session-001",
        SessionStatus::Completed,
        3,
        0,
    )];
    let stats = create_test_stats(1, 3, 1.50);

    let html = dashboard_page(&sessions, &stats);

    assert_snapshot!(html);
}

#[test]
fn test_dashboard_with_error_session() {
    let sessions = vec![create_test_session(
        "session-002",
        SessionStatus::Crashed,
        0,
        5,
    )];
    let stats = create_test_stats(1, 0, 0.75);

    let html = dashboard_page(&sessions, &stats);

    assert_snapshot!(html);
}

#[test]
fn test_dashboard_with_timeout_session() {
    let sessions = vec![create_test_session(
        "session-003",
        SessionStatus::MaxTurns,
        1,
        2,
    )];
    let stats = create_test_stats(1, 1, 2.00);

    let html = dashboard_page(&sessions, &stats);

    assert_snapshot!(html);
}

#[test]
fn test_dashboard_with_mixed_sessions() {
    let sessions = vec![
        create_test_session("session-001", SessionStatus::Completed, 5, 0),
        create_test_session("session-002", SessionStatus::Crashed, 0, 3),
        create_test_session("session-003", SessionStatus::MaxTurns, 2, 1),
        create_test_session("session-004", SessionStatus::Completed, 4, 0),
    ];
    let stats = create_test_stats(4, 11, 6.50);

    let html = dashboard_page(&sessions, &stats);

    assert_snapshot!(html);
}

#[test]
fn test_summary_card_zero_stats() {
    let stats = SummaryStats::default();

    let markup = summary_card(&stats);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_summary_card_with_data() {
    let stats = create_test_stats(42, 180, 125.75);

    let markup = summary_card(&stats);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_summary_card_high_completion_rate() {
    let stats = SummaryStats {
        total_sessions: 100,
        total_issues_completed: 450,
        total_cost_usd: 500.00,
        avg_duration_seconds: 180.0,
        avg_tokens_per_session: 20000.0,
        completion_rate: 0.95,
    };

    let markup = summary_card(&stats);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_sessions_table_empty() {
    let sessions = vec![];

    let markup = sessions_table(&sessions);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_sessions_table_single_session() {
    let sessions = vec![create_test_session(
        "session-001",
        SessionStatus::Completed,
        3,
        0,
    )];

    let markup = sessions_table(&sessions);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_sessions_table_multiple_statuses() {
    let sessions = vec![
        create_test_session("session-001", SessionStatus::Completed, 5, 0),
        create_test_session("session-002", SessionStatus::Crashed, 0, 8),
        create_test_session("session-003", SessionStatus::MaxTurns, 2, 3),
    ];

    let markup = sessions_table(&sessions);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_sessions_table_high_error_rate() {
    let sessions = vec![
        create_test_session("session-001", SessionStatus::Completed, 3, 15), // High errors even on success
    ];

    let markup = sessions_table(&sessions);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_sessions_table_zero_completion() {
    let sessions = vec![
        create_test_session("session-001", SessionStatus::Crashed, 0, 10),
        create_test_session("session-002", SessionStatus::MaxTurns, 0, 5),
    ];

    let markup = sessions_table(&sessions);
    let html = markup.into_string();

    assert_snapshot!(html);
}

#[test]
fn test_dashboard_max_sessions() {
    // Test with maximum typical display (50 sessions)
    let sessions: Vec<_> = (1..=50)
        .map(|i| {
            let status = match i % 3 {
                0 => SessionStatus::Completed,
                1 => SessionStatus::Crashed,
                _ => SessionStatus::MaxTurns,
            };
            create_test_session(
                &format!("session-{:03}", i),
                status,
                (i % 5) as i32,
                (i % 7) as i32,
            )
        })
        .collect();

    let stats = create_test_stats(50, 150, 75.00);

    let html = dashboard_page(&sessions, &stats);

    // Just verify it renders without error and has expected structure
    assert!(html.contains("Autopilot Metrics Dashboard"));
    assert!(html.contains("session-001"));
    assert!(html.contains("session-050"));
}
