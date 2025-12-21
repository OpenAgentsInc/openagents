//! Tests for autopilot metrics dashboard API endpoints

use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus};
use chrono::Utc;

#[test]
fn test_sessions_api_data_filtering() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_filter.db");

    let store = MetricsDb::open(&db_path).unwrap();

    // Create test data with different statuses
    for i in 0..5 {
        let metrics = SessionMetrics {
            id: format!("session-{}", i),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: format!("Test prompt {}", i),
            duration_seconds: 100.0 + (i as f64 * 10.0),
            tokens_in: 1000 + (i as i64 * 100),
            tokens_out: 500 + (i as i64 * 50),
            tokens_cached: 200,
            cost_usd: 0.05 + (i as f64 * 0.01),
            issues_claimed: 2,
            issues_completed: if i % 2 == 0 { 2 } else { 1 },
            tool_calls: 10 + i,
            tool_errors: if i % 3 == 0 { 1 } else { 0 },
            final_status: if i == 4 {
                SessionStatus::Crashed
            } else {
                SessionStatus::Completed
            },
        };
        store.store_session(&metrics).unwrap();
    }

    // Test filtering by status
    let all_sessions = store.get_all_sessions().unwrap();
    assert_eq!(all_sessions.len(), 5);

    let crashed_sessions: Vec<_> = all_sessions
        .iter()
        .filter(|s| matches!(s.final_status, SessionStatus::Crashed))
        .collect();
    assert_eq!(crashed_sessions.len(), 1);

    let completed_sessions: Vec<_> = all_sessions
        .iter()
        .filter(|s| matches!(s.final_status, SessionStatus::Completed))
        .collect();
    assert_eq!(completed_sessions.len(), 4);
}

#[test]
fn test_sessions_api_sorting() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_sort.db");

    let store = MetricsDb::open(&db_path).unwrap();

    // Create sessions with different durations
    for i in 0..5 {
        let metrics = SessionMetrics {
            id: format!("session-{}", i),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: format!("Test {}", i),
            duration_seconds: 100.0 + (i as f64 * 10.0),
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05 + (i as f64 * 0.01),
            issues_claimed: 2,
            issues_completed: 2,
            tool_calls: 10,
            tool_errors: 0,
            final_status: SessionStatus::Completed,
        };
        store.store_session(&metrics).unwrap();
    }

    // Test sorting by duration
    let mut sessions = store.get_all_sessions().unwrap();
    sessions.sort_by(|a, b| a.duration_seconds.partial_cmp(&b.duration_seconds).unwrap());

    assert_eq!(sessions[0].duration_seconds, 100.0);
    assert_eq!(sessions[4].duration_seconds, 140.0);

    // Test sorting by cost
    sessions.sort_by(|a, b| a.cost_usd.partial_cmp(&b.cost_usd).unwrap());

    assert_eq!(sessions[0].cost_usd, 0.05);
    assert_eq!(sessions[4].cost_usd, 0.09);
}

#[test]
fn test_session_detail_data() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_detail.db");

    let store = MetricsDb::open(&db_path).unwrap();

    let metrics = SessionMetrics {
        id: "test-session-123".to_string(),
        timestamp: Utc::now(),
        model: "sonnet".to_string(),
        prompt: "Test prompt".to_string(),
        duration_seconds: 150.0,
        tokens_in: 2000,
        tokens_out: 1000,
        tokens_cached: 500,
        cost_usd: 0.10,
        issues_claimed: 3,
        issues_completed: 2,
        tool_calls: 25,
        tool_errors: 2,
        final_status: SessionStatus::Completed,
    };
    store.store_session(&metrics).unwrap();

    // Retrieve session
    let session = store.get_session("test-session-123").unwrap().unwrap();

    assert_eq!(session.id, "test-session-123");
    assert_eq!(session.tool_calls, 25);
    assert_eq!(session.tool_errors, 2);
    assert_eq!(session.issues_claimed, 3);
    assert_eq!(session.issues_completed, 2);

    // Get tool calls and anomalies (should be empty for this test)
    let tool_calls = store.get_tool_calls("test-session-123").unwrap();
    let anomalies = store.get_anomalies("test-session-123").unwrap();

    assert!(tool_calls.is_empty());
    assert!(anomalies.is_empty());
}

#[test]
fn test_metrics_summary_stats() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_stats.db");

    let store = MetricsDb::open(&db_path).unwrap();

    // Add multiple sessions
    for i in 0..10 {
        let metrics = SessionMetrics {
            id: format!("session-{}", i),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: format!("Test {}", i),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 2,
            tool_calls: 10,
            tool_errors: 0,
            final_status: SessionStatus::Completed,
        };
        store.store_session(&metrics).unwrap();
    }

    let stats = store.get_summary_stats().unwrap();

    assert_eq!(stats.total_sessions, 10);
    assert_eq!(stats.total_issues_completed, 20);
    assert_eq!(stats.total_cost_usd, 0.50);
    assert_eq!(stats.avg_duration_seconds, 100.0);
    assert!((stats.completion_rate - 1.0).abs() < 0.01); // Should be 100%
}

#[test]
fn test_trends_calculation_error_rate() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_trends.db");

    let store = MetricsDb::open(&db_path).unwrap();

    // Add sessions with varying error rates
    for i in 0..5 {
        let metrics = SessionMetrics {
            id: format!("session-{}", i),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: format!("Test {}", i),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 2,
            tool_calls: 20,
            tool_errors: i as i32,
            final_status: SessionStatus::Completed,
        };
        store.store_session(&metrics).unwrap();
    }

    let sessions = store.get_all_sessions().unwrap();

    // Calculate total error rate
    let total_calls: i32 = sessions.iter().map(|s| s.tool_calls).sum();
    let total_errors: i32 = sessions.iter().map(|s| s.tool_errors).sum();

    assert_eq!(total_calls, 100); // 5 sessions * 20 calls
    assert_eq!(total_errors, 10); // 0 + 1 + 2 + 3 + 4

    let error_rate = (total_errors as f64 / total_calls as f64) * 100.0;
    assert_eq!(error_rate, 10.0);
}

#[test]
fn test_pagination_logic() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_pagination.db");

    let store = MetricsDb::open(&db_path).unwrap();

    // Create 20 sessions
    for i in 0..20 {
        let metrics = SessionMetrics {
            id: format!("session-{:02}", i),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: format!("Test {}", i),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 2,
            tool_calls: 10,
            tool_errors: 0,
            final_status: SessionStatus::Completed,
        };
        store.store_session(&metrics).unwrap();
    }

    let all_sessions = store.get_all_sessions().unwrap();
    assert_eq!(all_sessions.len(), 20);

    // Test pagination: offset=0, limit=10
    let page1: Vec<_> = all_sessions.iter().skip(0).take(10).collect();
    assert_eq!(page1.len(), 10);

    // Test pagination: offset=10, limit=10
    let page2: Vec<_> = all_sessions.iter().skip(10).take(10).collect();
    assert_eq!(page2.len(), 10);

    // Test pagination: offset=15, limit=10 (should only get 5)
    let page3: Vec<_> = all_sessions.iter().skip(15).take(10).collect();
    assert_eq!(page3.len(), 5);
}
