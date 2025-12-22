//! Alert configuration and notification system for autopilot metric regressions
//!
//! This module provides configurable alerting for autopilot metrics that exceed thresholds
//! or deviate significantly from established baselines.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Warning,
    Error,
    Critical,
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertSeverity::Warning => "warning",
            AlertSeverity::Error => "error",
            AlertSeverity::Critical => "critical",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "warning" => Some(AlertSeverity::Warning),
            "error" => Some(AlertSeverity::Error),
            "critical" => Some(AlertSeverity::Critical),
            _ => None,
        }
    }
}

/// Type of alert condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertType {
    /// Alert when metric exceeds absolute threshold
    Threshold,
    /// Alert when metric deviates from baseline by N standard deviations
    Regression,
    /// Alert when metric shows consistent trend over N consecutive runs
    RateOfChange,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::Threshold => "threshold",
            AlertType::Regression => "regression",
            AlertType::RateOfChange => "rate_of_change",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "threshold" => Some(AlertType::Threshold),
            "regression" => Some(AlertType::Regression),
            "rate_of_change" => Some(AlertType::RateOfChange),
            _ => None,
        }
    }
}

/// Alert configuration rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: i64,
    pub metric_name: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    /// Threshold value (interpretation depends on alert_type)
    pub threshold: f64,
    /// Description of what this alert detects
    pub description: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

/// Fired alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiredAlert {
    pub id: i64,
    pub rule_id: i64,
    pub session_id: String,
    pub metric_name: String,
    pub current_value: f64,
    pub baseline_value: Option<f64>,
    pub delta: f64,
    pub severity: AlertSeverity,
    pub message: String,
    pub fired_at: DateTime<Utc>,
}

/// Initialize alerts table in the database
pub fn init_alerts_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS alert_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_name TEXT NOT NULL,
            alert_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            threshold REAL NOT NULL,
            description TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            created_at DATETIME NOT NULL
        )
        "#,
        [],
    )?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS fired_alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id INTEGER NOT NULL,
            session_id TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            current_value REAL NOT NULL,
            baseline_value REAL,
            delta REAL NOT NULL,
            severity TEXT NOT NULL,
            message TEXT NOT NULL,
            fired_at DATETIME NOT NULL,
            FOREIGN KEY (rule_id) REFERENCES alert_rules(id)
        )
        "#,
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_fired_alerts_session ON fired_alerts(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_fired_alerts_metric ON fired_alerts(metric_name)",
        [],
    )?;

    Ok(())
}

/// Add default alert rules if none exist
pub fn add_default_alerts(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM alert_rules",
        [],
        |row| row.get(0),
    )?;

    if count == 0 {
        let now = Utc::now().to_rfc3339();

        // Tool error rate >10% (critical)
        conn.execute(
            "INSERT INTO alert_rules (metric_name, alert_type, severity, threshold, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "tool_error_rate",
                "threshold",
                "critical",
                0.10,
                "Tool error rate exceeds 10%",
                now
            ],
        )?;

        // Task completion rate <80% (warning)
        conn.execute(
            "INSERT INTO alert_rules (metric_name, alert_type, severity, threshold, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "task_completion_rate",
                "threshold",
                "warning",
                0.80,
                "Task completion rate below 80%",
                now
            ],
        )?;

        // Tokens per task increase >20% from baseline (warning)
        conn.execute(
            "INSERT INTO alert_rules (metric_name, alert_type, severity, threshold, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "tokens_per_task",
                "regression",
                "warning",
                0.20,
                "Token usage increased >20% from baseline",
                now
            ],
        )?;
    }

    Ok(())
}

/// Add a new alert rule
pub fn add_alert_rule(
    conn: &Connection,
    metric_name: &str,
    alert_type: AlertType,
    severity: AlertSeverity,
    threshold: f64,
    description: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO alert_rules (metric_name, alert_type, severity, threshold, description, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            metric_name,
            alert_type.as_str(),
            severity.as_str(),
            threshold,
            description,
            Utc::now().to_rfc3339()
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Remove an alert rule
pub fn remove_alert_rule(conn: &Connection, rule_id: i64) -> Result<()> {
    conn.execute("DELETE FROM alert_rules WHERE id = ?1", params![rule_id])?;
    Ok(())
}

/// List all alert rules
pub fn list_alert_rules(conn: &Connection) -> Result<Vec<AlertRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, metric_name, alert_type, severity, threshold, description, enabled, created_at
         FROM alert_rules
         ORDER BY id",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(AlertRule {
            id: row.get(0)?,
            metric_name: row.get(1)?,
            alert_type: AlertType::from_str(&row.get::<_, String>(2)?)
                .unwrap_or(AlertType::Threshold),
            severity: AlertSeverity::from_str(&row.get::<_, String>(3)?)
                .unwrap_or(AlertSeverity::Warning),
            threshold: row.get(4)?,
            description: row.get(5)?,
            enabled: row.get(6)?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;

    let mut rules = Vec::new();
    for row in rows {
        rules.push(row?);
    }

    Ok(rules)
}

/// Get baseline value for a metric
fn get_baseline(conn: &Connection, metric_name: &str) -> Result<Option<f64>> {
    let result: Option<f64> = conn
        .query_row(
            "SELECT mean FROM baselines WHERE dimension = ?1",
            params![metric_name],
            |row| row.get(0),
        )
        .optional()?;

    Ok(result)
}

/// Evaluate alerts for a given metric value
pub fn evaluate_alerts(
    conn: &Connection,
    session_id: &str,
    metric_name: &str,
    current_value: f64,
) -> Result<Vec<FiredAlert>> {
    let mut alerts = Vec::new();

    // Get enabled rules for this metric
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, severity, threshold, description
         FROM alert_rules
         WHERE metric_name = ?1 AND enabled = 1",
    )?;

    let rules = stmt.query_map(params![metric_name], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    for rule in rules {
        let (rule_id, alert_type_str, severity_str, threshold, description) = rule?;
        let alert_type = AlertType::from_str(&alert_type_str).unwrap_or(AlertType::Threshold);
        let severity = AlertSeverity::from_str(&severity_str).unwrap_or(AlertSeverity::Warning);

        let should_alert = match alert_type {
            AlertType::Threshold => {
                // For error rates: alert if value > threshold
                // For completion rates: alert if value < threshold (inverted)
                if metric_name.contains("error") || metric_name.contains("failure") {
                    current_value > threshold
                } else if metric_name.contains("completion") || metric_name.contains("success") {
                    current_value < threshold
                } else {
                    current_value > threshold
                }
            }
            AlertType::Regression => {
                // Alert if deviation from baseline exceeds threshold percentage
                if let Some(baseline) = get_baseline(conn, metric_name)? {
                    let delta = (current_value - baseline).abs() / baseline;
                    delta > threshold
                } else {
                    false // No baseline, can't evaluate regression
                }
            }
            AlertType::RateOfChange => {
                // TODO: Implement trend detection (requires historical data)
                false
            }
        };

        if should_alert {
            let baseline = get_baseline(conn, metric_name)?;
            let delta = if let Some(base) = baseline {
                current_value - base
            } else {
                0.0
            };

            let message = format!(
                "{}: {} = {:.2} (threshold: {:.2}{})",
                description,
                metric_name,
                current_value,
                threshold,
                if let Some(base) = baseline {
                    format!(", baseline: {:.2}, delta: {:+.2}", base, delta)
                } else {
                    String::new()
                }
            );

            // Store fired alert
            conn.execute(
                "INSERT INTO fired_alerts (rule_id, session_id, metric_name, current_value, baseline_value, delta, severity, message, fired_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    rule_id,
                    session_id,
                    metric_name,
                    current_value,
                    baseline,
                    delta,
                    severity.as_str(),
                    message,
                    Utc::now().to_rfc3339()
                ],
            )?;

            let alert_id = conn.last_insert_rowid();

            alerts.push(FiredAlert {
                id: alert_id,
                rule_id,
                session_id: session_id.to_string(),
                metric_name: metric_name.to_string(),
                current_value,
                baseline_value: baseline,
                delta,
                severity,
                message,
                fired_at: Utc::now(),
            });
        }
    }

    Ok(alerts)
}

/// Get alert history, optionally filtered by session or metric
pub fn get_alert_history(
    conn: &Connection,
    session_id: Option<&str>,
    metric_name: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<FiredAlert>> {
    let mut query = "SELECT id, rule_id, session_id, metric_name, current_value, baseline_value, delta, severity, message, fired_at
                     FROM fired_alerts
                     WHERE 1=1".to_string();

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(sid) = session_id {
        query.push_str(" AND session_id = ?");
        params.push(Box::new(sid.to_string()));
    }

    if let Some(metric) = metric_name {
        query.push_str(" AND metric_name = ?");
        params.push(Box::new(metric.to_string()));
    }

    query.push_str(" ORDER BY fired_at DESC");

    if let Some(lim) = limit {
        query.push_str(&format!(" LIMIT {}", lim));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(FiredAlert {
            id: row.get(0)?,
            rule_id: row.get(1)?,
            session_id: row.get(2)?,
            metric_name: row.get(3)?,
            current_value: row.get(4)?,
            baseline_value: row.get(5)?,
            delta: row.get(6)?,
            severity: AlertSeverity::from_str(&row.get::<_, String>(7)?)
                .unwrap_or(AlertSeverity::Warning),
            message: row.get(8)?,
            fired_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        alerts.push(row?);
    }

    Ok(alerts)
}

/// Log alert to stdout with color-coded severity
pub fn log_alert_to_stdout(alert: &FiredAlert) {
    let color = match alert.severity {
        AlertSeverity::Warning => "\x1b[33m", // Yellow
        AlertSeverity::Error => "\x1b[31m",   // Red
        AlertSeverity::Critical => "\x1b[1;31m", // Bold red
    };
    let reset = "\x1b[0m";

    eprintln!(
        "{}[{}] {}: {}{}",
        color,
        alert.severity.as_str().to_uppercase(),
        alert.metric_name,
        alert.message,
        reset
    );
}

/// Append alert to alerts.log file
pub fn log_alert_to_file(alert: &FiredAlert, log_path: &Path) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .context("Failed to open alerts.log")?;

    writeln!(
        file,
        "[{}] [{}] {}: {} (session: {})",
        alert.fired_at.format("%Y-%m-%d %H:%M:%S"),
        alert.severity.as_str().to_uppercase(),
        alert.metric_name,
        alert.message,
        alert.session_id
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_alerts_schema(&conn).unwrap();

        // Create baselines table for regression tests
        conn.execute(
            "CREATE TABLE IF NOT EXISTS baselines (dimension TEXT PRIMARY KEY, mean REAL)",
            [],
        ).unwrap();

        conn
    }

    #[test]
    fn test_add_and_list_rules() {
        let conn = setup_test_db();

        let rule_id = add_alert_rule(
            &conn,
            "tool_error_rate",
            AlertType::Threshold,
            AlertSeverity::Critical,
            0.10,
            "Test alert",
        )
        .unwrap();

        assert!(rule_id > 0);

        let rules = list_alert_rules(&conn).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].metric_name, "tool_error_rate");
        assert_eq!(rules[0].threshold, 0.10);
    }

    #[test]
    fn test_threshold_alert() {
        let conn = setup_test_db();

        add_alert_rule(
            &conn,
            "tool_error_rate",
            AlertType::Threshold,
            AlertSeverity::Critical,
            0.10,
            "High error rate",
        )
        .unwrap();

        // Should not alert (below threshold)
        let alerts = evaluate_alerts(&conn, "test-session", "tool_error_rate", 0.05).unwrap();
        assert_eq!(alerts.len(), 0);

        // Should alert (above threshold)
        let alerts = evaluate_alerts(&conn, "test-session", "tool_error_rate", 0.15).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
    }

    #[test]
    fn test_regression_alert() {
        let conn = setup_test_db();

        // Set baseline
        conn.execute(
            "INSERT INTO baselines (dimension, mean) VALUES (?1, ?2)",
            params!["tokens_per_task", 1000.0],
        )
        .unwrap();

        add_alert_rule(
            &conn,
            "tokens_per_task",
            AlertType::Regression,
            AlertSeverity::Warning,
            0.20, // 20% deviation
            "Token usage regression",
        )
        .unwrap();

        // Should not alert (within 20%)
        let alerts = evaluate_alerts(&conn, "test-session", "tokens_per_task", 1100.0).unwrap();
        assert_eq!(alerts.len(), 0);

        // Should alert (>20% increase)
        let alerts = evaluate_alerts(&conn, "test-session", "tokens_per_task", 1300.0).unwrap();
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].delta > 0.0);
    }

    #[test]
    fn test_alert_history() {
        let conn = setup_test_db();

        add_alert_rule(
            &conn,
            "tool_error_rate",
            AlertType::Threshold,
            AlertSeverity::Critical,
            0.10,
            "Test",
        )
        .unwrap();

        // Fire some alerts
        evaluate_alerts(&conn, "session-1", "tool_error_rate", 0.15).unwrap();
        evaluate_alerts(&conn, "session-2", "tool_error_rate", 0.20).unwrap();

        // Get all alerts
        let history = get_alert_history(&conn, None, None, None).unwrap();
        assert_eq!(history.len(), 2);

        // Filter by session
        let history = get_alert_history(&conn, Some("session-1"), None, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].session_id, "session-1");
    }

    #[test]
    fn test_remove_alert_rule() {
        let conn = setup_test_db();

        let rule_id = add_alert_rule(
            &conn,
            "test_metric",
            AlertType::Threshold,
            AlertSeverity::Warning,
            1.0,
            "Test",
        )
        .unwrap();

        remove_alert_rule(&conn, rule_id).unwrap();

        let rules = list_alert_rules(&conn).unwrap();
        assert_eq!(rules.len(), 0);
    }
}
