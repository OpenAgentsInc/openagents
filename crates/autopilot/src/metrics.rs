//! Metrics collection and storage for autopilot self-improvement
//!
//! This module implements the foundational infrastructure for continual constant
//! improvement of autopilot (directive d-004). It provides:
//!
//! - Session-level metrics tracking (tokens, costs, completion rates)
//! - Per-tool-call metrics (duration, success/failure, error types)
//! - SQLite-based persistent storage
//! - Anomaly detection and baseline tracking
//!
//! The metrics enable data-driven optimization of autopilot performance across
//! 50+ dimensions defined in docs/autopilot/IMPROVEMENT-DIMENSIONS.md.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Session-level metrics captured for each autopilot run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    /// Unique session identifier
    pub id: String,
    /// Session start timestamp
    pub timestamp: DateTime<Utc>,
    /// Model used (e.g., "sonnet", "opus", "haiku")
    pub model: String,
    /// Initial prompt/task description
    pub prompt: String,
    /// Total duration in seconds
    pub duration_seconds: f64,
    /// Input tokens consumed
    pub tokens_in: i64,
    /// Output tokens generated
    pub tokens_out: i64,
    /// Cached tokens (prompt caching)
    pub tokens_cached: i64,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Number of issues claimed during session
    pub issues_claimed: i32,
    /// Number of issues successfully completed
    pub issues_completed: i32,
    /// Total number of tool calls
    pub tool_calls: i32,
    /// Number of failed tool calls
    pub tool_errors: i32,
    /// Final session status
    pub final_status: SessionStatus,
}

/// Possible session termination states
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session completed successfully
    Completed,
    /// Session crashed or encountered fatal error
    Crashed,
    /// Session stopped due to budget exhaustion
    BudgetExhausted,
    /// Session stopped due to max turns reached
    MaxTurns,
    /// Session still running
    Running,
}

impl SessionStatus {
    fn to_string(&self) -> &'static str {
        match self {
            SessionStatus::Completed => "completed",
            SessionStatus::Crashed => "crashed",
            SessionStatus::BudgetExhausted => "budget_exhausted",
            SessionStatus::MaxTurns => "max_turns",
            SessionStatus::Running => "running",
        }
    }

    #[allow(dead_code)]
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "completed" => Ok(SessionStatus::Completed),
            "crashed" => Ok(SessionStatus::Crashed),
            "budget_exhausted" => Ok(SessionStatus::BudgetExhausted),
            "max_turns" => Ok(SessionStatus::MaxTurns),
            "running" => Ok(SessionStatus::Running),
            _ => Err(anyhow::anyhow!("Invalid session status: {}", s)),
        }
    }
}

/// Per-tool-call metrics for detailed analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallMetrics {
    /// Parent session ID
    pub session_id: String,
    /// Tool call timestamp
    pub timestamp: DateTime<Utc>,
    /// Tool name (e.g., "Read", "Write", "Bash")
    pub tool_name: String,
    /// Duration in milliseconds
    pub duration_ms: i64,
    /// Whether the tool call succeeded
    pub success: bool,
    /// Error type if failed (None if success)
    pub error_type: Option<String>,
    /// Input tokens for this call
    pub tokens_in: i64,
    /// Output tokens for this call
    pub tokens_out: i64,
}

/// Detected anomaly in metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Parent session ID
    pub session_id: String,
    /// Metric dimension that's anomalous
    pub dimension: String,
    /// Expected value (baseline)
    pub expected_value: f64,
    /// Actual observed value
    pub actual_value: f64,
    /// Severity level
    pub severity: AnomalySeverity,
    /// Whether this has been investigated
    pub investigated: bool,
    /// Issue number created for this anomaly (if any)
    pub issue_number: Option<i32>,
}

/// Severity of detected anomaly
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Warning,
    Error,
    Critical,
}

impl AnomalySeverity {
    fn to_string(&self) -> &'static str {
        match self {
            AnomalySeverity::Warning => "warning",
            AnomalySeverity::Error => "error",
            AnomalySeverity::Critical => "critical",
        }
    }

    #[allow(dead_code)]
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "warning" => Ok(AnomalySeverity::Warning),
            "error" => Ok(AnomalySeverity::Error),
            "critical" => Ok(AnomalySeverity::Critical),
            _ => Err(anyhow::anyhow!("Invalid anomaly severity: {}", s)),
        }
    }
}

/// Baseline statistics for a metric dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Metric dimension name
    pub dimension: String,
    /// Mean value
    pub mean: f64,
    /// Standard deviation
    pub stddev: f64,
    /// 50th percentile (median)
    pub p50: f64,
    /// 90th percentile
    pub p90: f64,
    /// 99th percentile
    pub p99: f64,
    /// Number of samples used to calculate baseline
    pub sample_count: i32,
    /// When baseline was last updated
    pub updated_at: DateTime<Utc>,
}

/// Metrics database for persistent storage
pub struct MetricsDb {
    conn: Connection,
}

impl MetricsDb {
    /// Open or create metrics database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .context("Failed to open metrics database")?;

        let db = Self { conn };
        db.init_schema()?;

        Ok(db)
    }

    /// Create an in-memory database (for testing)
    #[allow(dead_code)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to create in-memory database")?;

        let db = Self { conn };
        db.init_schema()?;

        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                model TEXT NOT NULL,
                prompt TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                tokens_in INTEGER NOT NULL,
                tokens_out INTEGER NOT NULL,
                tokens_cached INTEGER NOT NULL,
                cost_usd REAL NOT NULL,
                issues_claimed INTEGER NOT NULL,
                issues_completed INTEGER NOT NULL,
                tool_calls INTEGER NOT NULL,
                tool_errors INTEGER NOT NULL,
                final_status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tool_calls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                success INTEGER NOT NULL,
                error_type TEXT,
                tokens_in INTEGER NOT NULL,
                tokens_out INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE TABLE IF NOT EXISTS anomalies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                dimension TEXT NOT NULL,
                expected_value REAL NOT NULL,
                actual_value REAL NOT NULL,
                severity TEXT NOT NULL,
                investigated INTEGER NOT NULL DEFAULT 0,
                issue_number INTEGER,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE TABLE IF NOT EXISTS baselines (
                dimension TEXT PRIMARY KEY,
                mean REAL NOT NULL,
                stddev REAL NOT NULL,
                p50 REAL NOT NULL,
                p90 REAL NOT NULL,
                p99 REAL NOT NULL,
                sample_count INTEGER NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_timestamp ON sessions(timestamp);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_session_id ON tool_calls(session_id);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_tool_name ON tool_calls(tool_name);
            CREATE INDEX IF NOT EXISTS idx_anomalies_session_id ON anomalies(session_id);
            CREATE INDEX IF NOT EXISTS idx_anomalies_severity ON anomalies(severity);
            "#,
        )
        .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Store session metrics
    pub fn store_session(&self, metrics: &SessionMetrics) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO sessions
            (id, timestamp, model, prompt, duration_seconds, tokens_in, tokens_out,
             tokens_cached, cost_usd, issues_claimed, issues_completed, tool_calls,
             tool_errors, final_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                metrics.id,
                metrics.timestamp.to_rfc3339(),
                metrics.model,
                metrics.prompt,
                metrics.duration_seconds,
                metrics.tokens_in,
                metrics.tokens_out,
                metrics.tokens_cached,
                metrics.cost_usd,
                metrics.issues_claimed,
                metrics.issues_completed,
                metrics.tool_calls,
                metrics.tool_errors,
                metrics.final_status.to_string(),
            ],
        )
        .context("Failed to store session metrics")?;

        Ok(())
    }

    /// Store tool call metrics
    pub fn store_tool_call(&self, metrics: &ToolCallMetrics) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO tool_calls
            (session_id, timestamp, tool_name, duration_ms, success, error_type,
             tokens_in, tokens_out)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                metrics.session_id,
                metrics.timestamp.to_rfc3339(),
                metrics.tool_name,
                metrics.duration_ms,
                metrics.success as i32,
                metrics.error_type,
                metrics.tokens_in,
                metrics.tokens_out,
            ],
        )
        .context("Failed to store tool call metrics")?;

        Ok(())
    }

    /// Store anomaly
    pub fn store_anomaly(&self, anomaly: &Anomaly) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO anomalies
            (session_id, dimension, expected_value, actual_value, severity,
             investigated, issue_number)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                anomaly.session_id,
                anomaly.dimension,
                anomaly.expected_value,
                anomaly.actual_value,
                anomaly.severity.to_string(),
                anomaly.investigated as i32,
                anomaly.issue_number,
            ],
        )
        .context("Failed to store anomaly")?;

        Ok(())
    }

    /// Store baseline
    pub fn store_baseline(&self, baseline: &Baseline) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO baselines
            (dimension, mean, stddev, p50, p90, p99, sample_count, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                baseline.dimension,
                baseline.mean,
                baseline.stddev,
                baseline.p50,
                baseline.p90,
                baseline.p99,
                baseline.sample_count,
                baseline.updated_at.to_rfc3339(),
            ],
        )
        .context("Failed to store baseline")?;

        Ok(())
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, model, prompt, duration_seconds, tokens_in,
                   tokens_out, tokens_cached, cost_usd, issues_claimed,
                   issues_completed, tool_calls, tool_errors, final_status
            FROM sessions WHERE id = ?1
            "#,
        )?;

        let result = stmt.query_row(params![session_id], |row| {
            Ok(SessionMetrics {
                id: row.get(0)?,
                timestamp: row.get::<_, String>(1)?.parse().unwrap(),
                model: row.get(2)?,
                prompt: row.get(3)?,
                duration_seconds: row.get(4)?,
                tokens_in: row.get(5)?,
                tokens_out: row.get(6)?,
                tokens_cached: row.get(7)?,
                cost_usd: row.get(8)?,
                issues_claimed: row.get(9)?,
                issues_completed: row.get(10)?,
                tool_calls: row.get(11)?,
                tool_errors: row.get(12)?,
                final_status: SessionStatus::from_str(&row.get::<_, String>(13)?).unwrap(),
            })
        });

        match result {
            Ok(metrics) => Ok(Some(metrics)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all sessions
    pub fn get_all_sessions(&self) -> Result<Vec<SessionMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, model, prompt, duration_seconds, tokens_in,
                   tokens_out, tokens_cached, cost_usd, issues_claimed,
                   issues_completed, tool_calls, tool_errors, final_status
            FROM sessions
            ORDER BY timestamp DESC
            "#,
        )?;

        let sessions = stmt
            .query_map([], |row| {
                Ok(SessionMetrics {
                    id: row.get(0)?,
                    timestamp: row.get::<_, String>(1)?.parse().unwrap(),
                    model: row.get(2)?,
                    prompt: row.get(3)?,
                    duration_seconds: row.get(4)?,
                    tokens_in: row.get(5)?,
                    tokens_out: row.get(6)?,
                    tokens_cached: row.get(7)?,
                    cost_usd: row.get(8)?,
                    issues_claimed: row.get(9)?,
                    issues_completed: row.get(10)?,
                    tool_calls: row.get(11)?,
                    tool_errors: row.get(12)?,
                    final_status: SessionStatus::from_str(&row.get::<_, String>(13)?).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Get tool calls for a session
    pub fn get_tool_calls(&self, session_id: &str) -> Result<Vec<ToolCallMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, timestamp, tool_name, duration_ms, success,
                   error_type, tokens_in, tokens_out
            FROM tool_calls
            WHERE session_id = ?1
            ORDER BY timestamp
            "#,
        )?;

        let tool_calls = stmt
            .query_map(params![session_id], |row| {
                Ok(ToolCallMetrics {
                    session_id: row.get(0)?,
                    timestamp: row.get::<_, String>(1)?.parse().unwrap(),
                    tool_name: row.get(2)?,
                    duration_ms: row.get(3)?,
                    success: row.get::<_, i32>(4)? != 0,
                    error_type: row.get(5)?,
                    tokens_in: row.get(6)?,
                    tokens_out: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tool_calls)
    }

    /// Get database path
    pub fn path(&self) -> PathBuf {
        match self.conn.path() {
            Some(p) => PathBuf::from(p),
            None => PathBuf::from(":memory:"),
        }
    }
}

/// Get default metrics database path
pub fn default_db_path() -> PathBuf {
    PathBuf::from("autopilot-metrics.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_database() {
        let db = MetricsDb::in_memory().unwrap();
        // In-memory databases return empty string or ":memory:"
        let path = db.path();
        assert!(path == PathBuf::from("") || path == PathBuf::from(":memory:"));
    }

    #[test]
    fn test_store_and_retrieve_session() {
        let db = MetricsDb::in_memory().unwrap();

        let metrics = SessionMetrics {
            id: "test-session-1".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test prompt".to_string(),
            duration_seconds: 120.5,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 1,
            tool_calls: 10,
            tool_errors: 1,
            final_status: SessionStatus::Completed,
        };

        db.store_session(&metrics).unwrap();

        let retrieved = db.get_session("test-session-1").unwrap().unwrap();
        assert_eq!(retrieved.id, metrics.id);
        assert_eq!(retrieved.model, metrics.model);
        assert_eq!(retrieved.tool_calls, metrics.tool_calls);
        assert_eq!(retrieved.final_status, SessionStatus::Completed);
    }

    #[test]
    fn test_store_and_retrieve_tool_calls() {
        let db = MetricsDb::in_memory().unwrap();

        // First create a session (required for foreign key)
        let session = SessionMetrics {
            id: "test-session-2".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test prompt".to_string(),
            duration_seconds: 120.5,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 1,
            tool_calls: 10,
            tool_errors: 1,
            final_status: SessionStatus::Completed,
        };
        db.store_session(&session).unwrap();

        let tool_call = ToolCallMetrics {
            session_id: "test-session-2".to_string(),
            timestamp: Utc::now(),
            tool_name: "Read".to_string(),
            duration_ms: 50,
            success: true,
            error_type: None,
            tokens_in: 100,
            tokens_out: 50,
        };

        db.store_tool_call(&tool_call).unwrap();

        let retrieved = db.get_tool_calls("test-session-2").unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].tool_name, "Read");
        assert!(retrieved[0].success);
    }

    #[test]
    fn test_store_anomaly() {
        let db = MetricsDb::in_memory().unwrap();

        // First create a session (required for foreign key)
        let session = SessionMetrics {
            id: "test-session-3".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test prompt".to_string(),
            duration_seconds: 120.5,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 2,
            issues_completed: 1,
            tool_calls: 10,
            tool_errors: 1,
            final_status: SessionStatus::Completed,
        };
        db.store_session(&session).unwrap();

        let anomaly = Anomaly {
            session_id: "test-session-3".to_string(),
            dimension: "tool_error_rate".to_string(),
            expected_value: 0.05,
            actual_value: 0.15,
            severity: AnomalySeverity::Warning,
            investigated: false,
            issue_number: None,
        };

        db.store_anomaly(&anomaly).unwrap();
    }

    #[test]
    fn test_store_baseline() {
        let db = MetricsDb::in_memory().unwrap();

        let baseline = Baseline {
            dimension: "tokens_per_task".to_string(),
            mean: 5000.0,
            stddev: 1000.0,
            p50: 4500.0,
            p90: 6500.0,
            p99: 8000.0,
            sample_count: 100,
            updated_at: Utc::now(),
        };

        db.store_baseline(&baseline).unwrap();
    }
}
