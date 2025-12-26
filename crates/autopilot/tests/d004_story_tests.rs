//! Tests covering d-004 autopilot improvement stories.

use autopilot::alerts::{self, AlertSeverity, AlertType};
use autopilot::analyze::{aggregate_analyses, analyze_trajectory, TimePeriod};
use autopilot::auto_issues;
use autopilot::learning::LearningPipeline;
use autopilot::metrics::baseline::{BaselineCalculator, BaselineComparator, MetricDimension};
use autopilot::metrics::{
    Anomaly, AnomalySeverity, MetricsDb, SessionMetrics, SessionStatus, ToolCallMetrics,
};
use autopilot::reports::{format_report_markdown, generate_weekly_report};
use autopilot::trajectory::{StepType, Trajectory, TrajectoryResult};
use chrono::{Duration, Utc};
use issues::db as issues_db;
use issues::issue;
use rusqlite::Connection;
use serde_json::json;
use tempfile::tempdir;

fn make_session(
    id: &str,
    timestamp: chrono::DateTime<Utc>,
    issues_claimed: i32,
    issues_completed: i32,
    tool_calls: i32,
    tool_errors: i32,
    cost_usd: f64,
) -> SessionMetrics {
    SessionMetrics {
        id: id.to_string(),
        timestamp,
        model: "sonnet".to_string(),
        prompt: "test".to_string(),
        duration_seconds: 60.0,
        tokens_in: 1000,
        tokens_out: 500,
        tokens_cached: 0,
        cost_usd,
        issues_claimed,
        issues_completed,
        tool_calls,
        tool_errors,
        final_status: SessionStatus::Completed,
        messages: 12,
        apm: Some(18.0),
        source: "autopilot".to_string(),
        issue_numbers: None,
        directive_id: None,
    }
}

#[test]
fn test_metrics_summary_for_runs() {
    let db = MetricsDb::in_memory().expect("metrics db");
    let now = Utc::now();

    let session_a = make_session("session-a", now, 2, 1, 10, 2, 0.25);
    let session_b = make_session("session-b", now, 1, 1, 4, 0, 0.15);

    db.store_session(&session_a).expect("store session a");
    db.store_session(&session_b).expect("store session b");

    let summary = db.get_summary_stats().expect("summary stats");
    assert_eq!(summary.total_sessions, 2);
    assert_eq!(summary.total_issues_completed, 2);
    assert!((summary.total_cost_usd - 0.40).abs() < 1e-6);
    assert!((summary.completion_rate - (2.0 / 3.0)).abs() < 1e-6);
}

#[test]
fn test_baseline_comparison_flags_regressions() {
    let db = MetricsDb::in_memory().expect("metrics db");
    let now = Utc::now();

    let baseline_sessions = vec![
        make_session("baseline-1", now, 1, 1, 10, 1, 0.1),
        make_session("baseline-2", now, 1, 1, 10, 0, 0.1),
    ];

    let calculator = BaselineCalculator::new(&db);
    let baselines = calculator
        .calculate_baselines(&baseline_sessions)
        .expect("calculate baselines");
    for baseline in baselines.values() {
        db.store_baseline(baseline).expect("store baseline");
    }

    let current_sessions = vec![
        make_session("current-1", now, 1, 0, 10, 6, 0.2),
        make_session("current-2", now, 1, 0, 10, 5, 0.2),
    ];

    let comparator = BaselineComparator::new(&db);
    let regressions = comparator
        .detect_regressions(&current_sessions)
        .expect("detect regressions");

    assert!(
        regressions
            .iter()
            .any(|reg| reg.dimension == MetricDimension::ToolErrorRate),
        "expected tool error rate regression"
    );
}

#[test]
fn test_tool_error_and_success_patterns() {
    let mut trajectory = Trajectory::new(
        "Analyze tool errors".to_string(),
        "sonnet".to_string(),
        "/tmp".to_string(),
        "abc123".to_string(),
        None,
    );

    trajectory.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "tool-1".to_string(),
        input: json!({"file_path": "README.md"}),
    });
    trajectory.add_step(StepType::ToolResult {
        tool_id: "tool-1".to_string(),
        success: true,
        output: Some("ok".to_string()),
    });
    trajectory.add_step(StepType::ToolCall {
        tool: "Write".to_string(),
        tool_id: "tool-2".to_string(),
        input: json!({"file_path": "src/lib.rs"}),
    });
    trajectory.add_step(StepType::ToolResult {
        tool_id: "tool-2".to_string(),
        success: false,
        output: Some("permission denied".to_string()),
    });
    trajectory.result = Some(TrajectoryResult {
        success: false,
        duration_ms: 1200,
        num_turns: 2,
        result_text: None,
        errors: vec!["write failed".to_string()],
        issues_completed: 0,
        apm: Some(10.0),
    });

    let analysis = analyze_trajectory(&trajectory);
    assert_eq!(analysis.errors.failed_tool_calls, 1);
    assert_eq!(analysis.errors.errors_by_tool.get("Write"), Some(&1));
    assert_eq!(
        analysis
            .tool_usage
            .success_rate_by_tool
            .get("Read")
            .copied(),
        Some(1.0)
    );

    let mut success_traj = Trajectory::new(
        "Successful run".to_string(),
        "sonnet".to_string(),
        "/tmp".to_string(),
        "def456".to_string(),
        None,
    );
    success_traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "tool-3".to_string(),
        input: json!({"file_path": "Cargo.toml"}),
    });
    success_traj.add_step(StepType::ToolResult {
        tool_id: "tool-3".to_string(),
        success: true,
        output: Some("ok".to_string()),
    });
    success_traj.result = Some(TrajectoryResult {
        success: true,
        duration_ms: 800,
        num_turns: 1,
        result_text: Some("done".to_string()),
        errors: Vec::new(),
        issues_completed: 1,
        apm: Some(12.0),
    });

    let analysis_success = analyze_trajectory(&success_traj);
    let aggregate = aggregate_analyses(&[analysis, analysis_success]);
    assert!((aggregate.overall_success_rate - 0.5).abs() < 1e-6);
}

#[test]
fn test_anomaly_alerts_fire_on_thresholds() {
    let conn = Connection::open_in_memory().expect("alerts db");
    alerts::init_alerts_schema(&conn).expect("init schema");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS baselines (dimension TEXT PRIMARY KEY, mean REAL NOT NULL)",
        [],
    )
    .expect("init baselines table");
    alerts::add_alert_rule(
        &conn,
        "tool_error_rate",
        AlertType::Threshold,
        AlertSeverity::Critical,
        0.10,
        "Tool error rate exceeded",
    )
    .expect("add rule");

    let fired = alerts::evaluate_alerts(&conn, "session-1", "tool_error_rate", 0.25)
        .expect("evaluate alerts");
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].severity, AlertSeverity::Critical);
}

#[test]
fn test_auto_issue_creation_from_anomalies() {
    let db = MetricsDb::in_memory().expect("metrics db");
    let now = Utc::now();

    let session_a = make_session("session-1", now, 1, 0, 10, 2, 0.1);
    let session_b = make_session("session-2", now, 1, 0, 10, 2, 0.1);
    db.store_session(&session_a).expect("store session a");
    db.store_session(&session_b).expect("store session b");

    let anomaly_a = Anomaly {
        session_id: "session-1".to_string(),
        dimension: "tool_error_rate".to_string(),
        expected_value: 0.05,
        actual_value: 0.20,
        severity: AnomalySeverity::Error,
        investigated: false,
        issue_number: None,
    };
    let anomaly_b = Anomaly {
        session_id: "session-2".to_string(),
        dimension: "tool_error_rate".to_string(),
        expected_value: 0.05,
        actual_value: 0.18,
        severity: AnomalySeverity::Error,
        investigated: false,
        issue_number: None,
    };

    db.store_anomaly(&anomaly_a).expect("store anomaly a");
    db.store_anomaly(&anomaly_b).expect("store anomaly b");

    let patterns = auto_issues::detect_patterns(&db).expect("detect patterns");
    let issues = auto_issues::generate_issues(
        patterns
            .into_iter()
            .map(auto_issues::Pattern::Anomaly)
            .collect(),
    );

    let temp = tempdir().expect("tempdir");
    let issues_path = temp.path().join("issues.db");
    issues_db::init_db(&issues_path).expect("init issues db");

    let created = auto_issues::create_issues(&issues_path, &issues, &db)
        .expect("create issues");
    assert_eq!(created.len(), 1);

    let conn = issues_db::init_db(&issues_path).expect("open issues db");
    let issue = issue::get_issue_by_number(&conn, created[0])
        .expect("fetch issue")
        .expect("issue exists");
    assert!(issue.auto_created);
}

#[test]
fn test_learning_pipeline_generates_updates() {
    let db = MetricsDb::in_memory().expect("metrics db");
    let now = Utc::now();

    let session = make_session("learn-1", now, 1, 0, 10, 3, 0.2);
    db.store_session(&session).expect("store session");

    let tool_call = ToolCallMetrics {
        session_id: session.id.clone(),
        timestamp: now,
        tool_name: "Bash".to_string(),
        duration_ms: 12,
        success: false,
        error_type: Some("sqlite3 INSERT blocked".to_string()),
        tokens_in: 10,
        tokens_out: 5,
    };
    db.store_tool_call(&tool_call).expect("store tool call");

    let pipeline = LearningPipeline::new(&db);
    let report = pipeline
        .run(&[session.id.clone()])
        .expect("learning pipeline");

    assert!(!report.improvements.is_empty());
    assert!(!report.prompt_updates.is_empty());
    assert!(!report.hook_updates.is_empty());
}

#[test]
fn test_weekly_report_generation() {
    let db = MetricsDb::in_memory().expect("metrics db");
    let (this_start, _) = TimePeriod::ThisWeek.bounds();
    let (last_start, _) = TimePeriod::LastWeek.bounds();

    let this_week = make_session(
        "this-week",
        this_start + Duration::hours(2),
        1,
        1,
        4,
        0,
        0.05,
    );
    let last_week = make_session(
        "last-week",
        last_start + Duration::hours(2),
        1,
        1,
        3,
        0,
        0.04,
    );

    db.store_session(&this_week).expect("store this week");
    db.store_session(&last_week).expect("store last week");

    let report = generate_weekly_report(&db).expect("weekly report");
    assert_eq!(report.total_sessions, 1);
    assert_eq!(report.total_issues_completed, 1);

    let markdown = format_report_markdown(&report);
    assert!(markdown.contains("Weekly Autopilot Metrics Report"));
}
