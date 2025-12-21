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

    /// Get anomalies for a specific session
    pub fn get_anomalies(&self, session_id: &str) -> Result<Vec<Anomaly>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, dimension, expected_value, actual_value, severity,
                   investigated, issue_number
            FROM anomalies
            WHERE session_id = ?1
            ORDER BY severity DESC
            "#,
        )?;

        let anomalies = stmt
            .query_map(params![session_id], |row| {
                Ok(Anomaly {
                    session_id: row.get(0)?,
                    dimension: row.get(1)?,
                    expected_value: row.get(2)?,
                    actual_value: row.get(3)?,
                    severity: AnomalySeverity::from_str(&row.get::<_, String>(4)?).unwrap(),
                    investigated: row.get::<_, i32>(5)? != 0,
                    issue_number: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(anomalies)
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

    /// Get baseline for a metric dimension
    pub fn get_baseline(&self, dimension: &str) -> Result<Option<Baseline>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT dimension, mean, stddev, p50, p90, p99, sample_count, updated_at
            FROM baselines WHERE dimension = ?1
            "#,
        )?;

        let result = stmt.query_row(params![dimension], |row| {
            Ok(Baseline {
                dimension: row.get(0)?,
                mean: row.get(1)?,
                stddev: row.get(2)?,
                p50: row.get(3)?,
                p90: row.get(4)?,
                p99: row.get(5)?,
                sample_count: row.get(6)?,
                updated_at: row.get::<_, String>(7)?.parse().unwrap(),
            })
        });

        match result {
            Ok(baseline) => Ok(Some(baseline)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Calculate baseline statistics for a metric dimension from historical sessions
    pub fn calculate_baseline(&self, dimension: &str, min_samples: usize) -> Result<Option<Baseline>> {
        use statrs::statistics::{Data, Distribution, OrderStatistics};

        // Get historical values for this dimension
        let values: Vec<f64> = match dimension {
            "tool_error_rate" => {
                let mut stmt = self.conn.prepare(
                    "SELECT tool_calls, tool_errors FROM sessions WHERE tool_calls > 0"
                )?;
                stmt.query_map([], |row| {
                    let calls: i32 = row.get(0)?;
                    let errors: i32 = row.get(1)?;
                    Ok((errors as f64) / (calls as f64))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            "tokens_per_issue" => {
                let mut stmt = self.conn.prepare(
                    "SELECT tokens_in, tokens_out, issues_completed FROM sessions WHERE issues_completed > 0"
                )?;
                stmt.query_map([], |row| {
                    let tokens_in: i64 = row.get(0)?;
                    let tokens_out: i64 = row.get(1)?;
                    let issues: i32 = row.get(2)?;
                    Ok(((tokens_in + tokens_out) as f64) / (issues as f64))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            "duration_per_issue" => {
                let mut stmt = self.conn.prepare(
                    "SELECT duration_seconds, issues_completed FROM sessions WHERE issues_completed > 0"
                )?;
                stmt.query_map([], |row| {
                    let duration: f64 = row.get(0)?;
                    let issues: i32 = row.get(1)?;
                    Ok(duration / (issues as f64))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            "cost_per_issue" => {
                let mut stmt = self.conn.prepare(
                    "SELECT cost_usd, issues_completed FROM sessions WHERE issues_completed > 0"
                )?;
                stmt.query_map([], |row| {
                    let cost: f64 = row.get(0)?;
                    let issues: i32 = row.get(1)?;
                    Ok(cost / (issues as f64))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            "session_duration" => {
                let mut stmt = self.conn.prepare("SELECT duration_seconds FROM sessions")?;
                stmt.query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?
            }
            "completion_rate" => {
                let mut stmt = self.conn.prepare(
                    "SELECT issues_claimed, issues_completed FROM sessions WHERE issues_claimed > 0"
                )?;
                stmt.query_map([], |row| {
                    let claimed: i32 = row.get(0)?;
                    let completed: i32 = row.get(1)?;
                    Ok((completed as f64) / (claimed as f64))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            _ => return Ok(None),
        };

        if values.len() < min_samples {
            return Ok(None);
        }

        let mut data = Data::new(values);
        let mean = data.mean().unwrap_or(0.0);
        let stddev = data.std_dev().unwrap_or(0.0);
        let p50 = data.median();
        let p90 = data.percentile(90);
        let p99 = data.percentile(99);

        Ok(Some(Baseline {
            dimension: dimension.to_string(),
            mean,
            stddev,
            p50,
            p90,
            p99,
            sample_count: data.len() as i32,
            updated_at: Utc::now(),
        }))
    }

    /// Update all baselines from recent session data
    pub fn update_baselines(&self, min_samples: usize) -> Result<Vec<String>> {
        let dimensions = vec![
            "tool_error_rate",
            "tokens_per_issue",
            "duration_per_issue",
            "cost_per_issue",
            "session_duration",
            "completion_rate",
        ];

        let mut updated = Vec::new();

        for dimension in dimensions {
            if let Some(baseline) = self.calculate_baseline(dimension, min_samples)? {
                self.store_baseline(&baseline)?;
                updated.push(dimension.to_string());
            }
        }

        Ok(updated)
    }

    /// Detect anomalies in session metrics by comparing against baselines
    pub fn detect_anomalies(&self, session: &SessionMetrics) -> Result<Vec<Anomaly>> {
        let mut anomalies = Vec::new();

        // Tool error rate
        if session.tool_calls > 0 {
            let error_rate = (session.tool_errors as f64) / (session.tool_calls as f64);

            // Rule-based thresholds
            if error_rate > 0.20 {
                anomalies.push(Anomaly {
                    session_id: session.id.clone(),
                    dimension: "tool_error_rate".to_string(),
                    expected_value: 0.10,
                    actual_value: error_rate,
                    severity: AnomalySeverity::Error,
                    investigated: false,
                    issue_number: None,
                });
            } else if error_rate > 0.10 {
                anomalies.push(Anomaly {
                    session_id: session.id.clone(),
                    dimension: "tool_error_rate".to_string(),
                    expected_value: 0.05,
                    actual_value: error_rate,
                    severity: AnomalySeverity::Warning,
                    investigated: false,
                    issue_number: None,
                });
            }

            // Baseline comparison (if available)
            if let Some(baseline) = self.get_baseline("tool_error_rate")? {
                if baseline.stddev > 0.0 {
                    let z_score = (error_rate - baseline.mean).abs() / baseline.stddev;
                    if z_score > 3.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "tool_error_rate_zscore".to_string(),
                            expected_value: baseline.mean,
                            actual_value: error_rate,
                            severity: AnomalySeverity::Critical,
                            investigated: false,
                            issue_number: None,
                        });
                    } else if z_score > 2.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "tool_error_rate_zscore".to_string(),
                            expected_value: baseline.mean,
                            actual_value: error_rate,
                            severity: AnomalySeverity::Warning,
                            investigated: false,
                            issue_number: None,
                        });
                    }
                }
            }
        }

        // Tokens per issue
        if session.issues_completed > 0 {
            let total_tokens = session.tokens_in + session.tokens_out;
            let tokens_per_issue = (total_tokens as f64) / (session.issues_completed as f64);

            if let Some(baseline) = self.get_baseline("tokens_per_issue")? {
                if baseline.stddev > 0.0 {
                    let z_score = (tokens_per_issue - baseline.mean).abs() / baseline.stddev;
                    if z_score > 3.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "tokens_per_issue".to_string(),
                            expected_value: baseline.mean,
                            actual_value: tokens_per_issue,
                            severity: AnomalySeverity::Critical,
                            investigated: false,
                            issue_number: None,
                        });
                    } else if z_score > 2.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "tokens_per_issue".to_string(),
                            expected_value: baseline.mean,
                            actual_value: tokens_per_issue,
                            severity: AnomalySeverity::Warning,
                            investigated: false,
                            issue_number: None,
                        });
                    }
                }
            }
        }

        // Cost per issue
        if session.issues_completed > 0 {
            let cost_per_issue = session.cost_usd / (session.issues_completed as f64);

            if let Some(baseline) = self.get_baseline("cost_per_issue")? {
                if baseline.stddev > 0.0 {
                    let z_score = (cost_per_issue - baseline.mean).abs() / baseline.stddev;
                    if z_score > 3.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "cost_per_issue".to_string(),
                            expected_value: baseline.mean,
                            actual_value: cost_per_issue,
                            severity: AnomalySeverity::Error,
                            investigated: false,
                            issue_number: None,
                        });
                    } else if z_score > 2.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "cost_per_issue".to_string(),
                            expected_value: baseline.mean,
                            actual_value: cost_per_issue,
                            severity: AnomalySeverity::Warning,
                            investigated: false,
                            issue_number: None,
                        });
                    }
                }
            }
        }

        // Session duration
        if let Some(baseline) = self.get_baseline("session_duration")? {
            if baseline.stddev > 0.0 {
                let z_score = (session.duration_seconds - baseline.mean).abs() / baseline.stddev;
                if z_score > 3.0 {
                    anomalies.push(Anomaly {
                        session_id: session.id.clone(),
                        dimension: "session_duration".to_string(),
                        expected_value: baseline.mean,
                        actual_value: session.duration_seconds,
                        severity: AnomalySeverity::Warning,
                        investigated: false,
                        issue_number: None,
                    });
                }
            }
        }

        // Completion rate
        if session.issues_claimed > 0 {
            let completion_rate = (session.issues_completed as f64) / (session.issues_claimed as f64);

            if let Some(baseline) = self.get_baseline("completion_rate")? {
                if baseline.stddev > 0.0 {
                    let z_score = (completion_rate - baseline.mean).abs() / baseline.stddev;
                    // Low completion rate is more concerning
                    if completion_rate < baseline.mean - 2.0 * baseline.stddev {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "completion_rate".to_string(),
                            expected_value: baseline.mean,
                            actual_value: completion_rate,
                            severity: AnomalySeverity::Error,
                            investigated: false,
                            issue_number: None,
                        });
                    } else if z_score > 2.0 {
                        anomalies.push(Anomaly {
                            session_id: session.id.clone(),
                            dimension: "completion_rate".to_string(),
                            expected_value: baseline.mean,
                            actual_value: completion_rate,
                            severity: AnomalySeverity::Warning,
                            investigated: false,
                            issue_number: None,
                        });
                    }
                }
            }
        }

        Ok(anomalies)
    }
}

/// Get default metrics database path
pub fn default_db_path() -> PathBuf {
    PathBuf::from("autopilot-metrics.db")
}

/// Extract metrics from a Trajectory
pub fn extract_metrics_from_trajectory(
    traj: &crate::trajectory::Trajectory,
) -> Result<(SessionMetrics, Vec<ToolCallMetrics>)> {
    use crate::trajectory::StepType;
    use std::collections::HashMap;

    // Calculate session-level metrics
    let duration_seconds = if let Some(ended_at) = traj.ended_at {
        (ended_at - traj.started_at).num_milliseconds() as f64 / 1000.0
    } else {
        // Session still running or incomplete
        0.0
    };

    // Count tool calls and errors
    let mut tool_calls_count = 0;
    let mut tool_errors_count = 0;
    let mut tool_call_map: HashMap<String, (String, DateTime<Utc>)> = HashMap::new();
    let mut tool_call_metrics = Vec::new();

    for step in &traj.steps {
        match &step.step_type {
            StepType::ToolCall { tool, tool_id, .. } => {
                tool_calls_count += 1;
                tool_call_map.insert(
                    tool_id.clone(),
                    (tool.clone(), step.timestamp),
                );
            }
            StepType::ToolResult {
                tool_id,
                success,
                ..
            } => {
                if !success {
                    tool_errors_count += 1;
                }

                // Create ToolCallMetrics if we have the matching ToolCall
                if let Some((tool_name, call_timestamp)) = tool_call_map.get(tool_id) {
                    let duration_ms = (step.timestamp - *call_timestamp).num_milliseconds();

                    let error_type = if !success {
                        // Extract error type from output if available
                        if let StepType::ToolResult { output, .. } = &step.step_type {
                            output.as_ref().and_then(|o| {
                                // Try to extract error type from output
                                if o.contains("EISDIR") {
                                    Some("EISDIR".to_string())
                                } else if o.contains("ENOENT") {
                                    Some("ENOENT".to_string())
                                } else if o.contains("Exit code") {
                                    Some("NonZeroExit".to_string())
                                } else {
                                    Some("Unknown".to_string())
                                }
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    tool_call_metrics.push(ToolCallMetrics {
                        session_id: traj.session_id.clone(),
                        timestamp: *call_timestamp,
                        tool_name: tool_name.clone(),
                        duration_ms,
                        success: *success,
                        error_type,
                        tokens_in: step.tokens_in.unwrap_or(0) as i64,
                        tokens_out: step.tokens_out.unwrap_or(0) as i64,
                    });
                }
            }
            _ => {}
        }
    }

    // Determine final status
    let final_status = if let Some(result) = &traj.result {
        if result.success {
            SessionStatus::Completed
        } else if result.errors.iter().any(|e| e.contains("budget")) {
            SessionStatus::BudgetExhausted
        } else if result.errors.iter().any(|e| e.contains("max turns") || e.contains("MaxTurns")) {
            SessionStatus::MaxTurns
        } else {
            SessionStatus::Crashed
        }
    } else {
        SessionStatus::Running
    };

    // Count issues completed (from result or by scanning steps)
    let issues_completed = if let Some(result) = &traj.result {
        result.issues_completed as i32
    } else {
        // Count issue_complete tool calls
        traj.steps
            .iter()
            .filter(|s| matches!(&s.step_type, StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_complete"))
            .count() as i32
    };

    // Count issues claimed
    let issues_claimed = traj
        .steps
        .iter()
        .filter(|s| matches!(&s.step_type, StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_claim"))
        .count() as i32;

    let session_metrics = SessionMetrics {
        id: traj.session_id.clone(),
        timestamp: traj.started_at,
        model: traj.model.clone(),
        prompt: traj.prompt.clone(),
        duration_seconds,
        tokens_in: traj.usage.input_tokens as i64,
        tokens_out: traj.usage.output_tokens as i64,
        tokens_cached: traj.usage.cache_read_tokens as i64,
        cost_usd: traj.usage.cost_usd,
        issues_claimed,
        issues_completed,
        tool_calls: tool_calls_count,
        tool_errors: tool_errors_count,
        final_status,
    };

    Ok((session_metrics, tool_call_metrics))
}

/// Extract metrics from a JSON trajectory file
pub fn extract_metrics_from_json_file<P: AsRef<Path>>(
    path: P,
) -> Result<(SessionMetrics, Vec<ToolCallMetrics>)> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read trajectory file: {:?}", path.as_ref()))?;

    let traj: crate::trajectory::Trajectory = serde_json::from_str(&content)
        .context("Failed to parse trajectory JSON")?;

    extract_metrics_from_trajectory(&traj)
}

/// Extract metrics from an rlog file by parsing it into a Trajectory first
pub fn extract_metrics_from_rlog_file<P: AsRef<Path>>(
    path: P,
) -> Result<(SessionMetrics, Vec<ToolCallMetrics>)> {
    // For now, we'll look for the corresponding .json file
    // In the future, we could implement direct .rlog parsing
    let json_path = path.as_ref().with_extension("json");

    if json_path.exists() {
        extract_metrics_from_json_file(json_path)
    } else {
        Err(anyhow::anyhow!(
            "No corresponding .json file found for .rlog file: {:?}",
            path.as_ref()
        ))
    }
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

    #[test]
    fn test_extract_metrics_from_trajectory() {
        use crate::trajectory::{Step, StepType, TokenUsage, Trajectory, TrajectoryResult};

        // Create a sample trajectory
        let mut traj = Trajectory {
            session_id: "test-123".to_string(),
            prompt: "Test task".to_string(),
            model: "sonnet".to_string(),
            cwd: "/test".to_string(),
            repo_sha: "abc123".to_string(),
            branch: Some("main".to_string()),
            started_at: Utc::now(),
            ended_at: Some(Utc::now() + chrono::Duration::seconds(120)),
            steps: vec![],
            result: Some(TrajectoryResult {
                success: true,
                duration_ms: 120000,
                num_turns: 5,
                result_text: Some("Done".to_string()),
                errors: vec![],
                issues_completed: 1,
            }),
            usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 200,
                cache_creation_tokens: 100,
                cost_usd: 0.05,
            },
        };

        // Add some steps
        let tool_call_time = Utc::now();
        traj.steps.push(Step {
            step_id: 1,
            timestamp: tool_call_time,
            step_type: StepType::ToolCall {
                tool: "Read".to_string(),
                tool_id: "tool-1".to_string(),
                input: serde_json::json!({"file_path": "test.txt"}),
            },
            tokens_in: Some(10),
            tokens_out: Some(5),
            tokens_cached: None,
        });

        traj.steps.push(Step {
            step_id: 2,
            timestamp: tool_call_time + chrono::Duration::milliseconds(50),
            step_type: StepType::ToolResult {
                tool_id: "tool-1".to_string(),
                success: true,
                output: Some("file contents".to_string()),
            },
            tokens_in: None,
            tokens_out: None,
            tokens_cached: None,
        });

        // Add a failed tool call
        traj.steps.push(Step {
            step_id: 3,
            timestamp: tool_call_time + chrono::Duration::milliseconds(100),
            step_type: StepType::ToolCall {
                tool: "Read".to_string(),
                tool_id: "tool-2".to_string(),
                input: serde_json::json!({"file_path": "missing.txt"}),
            },
            tokens_in: Some(10),
            tokens_out: Some(5),
            tokens_cached: None,
        });

        traj.steps.push(Step {
            step_id: 4,
            timestamp: tool_call_time + chrono::Duration::milliseconds(120),
            step_type: StepType::ToolResult {
                tool_id: "tool-2".to_string(),
                success: false,
                output: Some("Error: ENOENT: no such file".to_string()),
            },
            tokens_in: None,
            tokens_out: None,
            tokens_cached: None,
        });

        // Extract metrics
        let (session, tool_calls) = extract_metrics_from_trajectory(&traj).unwrap();

        // Verify session metrics
        assert_eq!(session.id, "test-123");
        assert_eq!(session.model, "sonnet");
        assert_eq!(session.tokens_in, 1000);
        assert_eq!(session.tokens_out, 500);
        assert_eq!(session.tool_calls, 2);
        assert_eq!(session.tool_errors, 1);
        assert_eq!(session.final_status, SessionStatus::Completed);

        // Verify tool call metrics
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].tool_name, "Read");
        assert!(tool_calls[0].success);
        assert_eq!(tool_calls[0].duration_ms, 50);

        assert_eq!(tool_calls[1].tool_name, "Read");
        assert!(!tool_calls[1].success);
        assert_eq!(tool_calls[1].error_type, Some("ENOENT".to_string()));
    }

    #[test]
    fn test_extract_metrics_from_json_file() {
        // Test with actual trajectory file if it exists
        let test_file = "docs/logs/20251220/020941-start-working.json";
        if std::path::Path::new(test_file).exists() {
            let result = extract_metrics_from_json_file(test_file);
            assert!(result.is_ok(), "Failed to extract metrics: {:?}", result.err());

            let (session, tool_calls) = result.unwrap();
            // Verify we got valid metrics
            assert!(!session.id.is_empty());
            assert!(!session.model.is_empty());
            assert!(session.tokens_in > 0);
            assert!(!tool_calls.is_empty());
        }
    }

    #[test]
    fn test_calculate_baseline() {
        let db = MetricsDb::in_memory().unwrap();

        // Create several sessions with varying tool error rates
        for i in 0..10 {
            let session = SessionMetrics {
                id: format!("session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0 + i as f64 * 10.0,
                tokens_in: 1000,
                tokens_out: 500,
                tokens_cached: 0,
                cost_usd: 0.05,
                issues_claimed: 1,
                issues_completed: 1,
                tool_calls: 100,
                tool_errors: i, // 0% to 9% error rate
                final_status: SessionStatus::Completed,
            };
            db.store_session(&session).unwrap();
        }

        // Calculate baseline with min 5 samples
        let baseline = db.calculate_baseline("tool_error_rate", 5).unwrap();
        assert!(baseline.is_some());

        let baseline = baseline.unwrap();
        assert_eq!(baseline.dimension, "tool_error_rate");
        assert!(baseline.mean > 0.0);
        assert!(baseline.stddev > 0.0);
        assert_eq!(baseline.sample_count, 10);
    }

    #[test]
    fn test_update_baselines() {
        let db = MetricsDb::in_memory().unwrap();

        // Create sessions with completed issues
        for i in 0..10 {
            let session = SessionMetrics {
                id: format!("session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0 + i as f64 * 10.0,
                tokens_in: 1000 * (i + 1) as i64,
                tokens_out: 500 * (i + 1) as i64,
                tokens_cached: 0,
                cost_usd: 0.05 * (i + 1) as f64,
                issues_claimed: 2,
                issues_completed: 1,
                tool_calls: 100,
                tool_errors: i,
                final_status: SessionStatus::Completed,
            };
            db.store_session(&session).unwrap();
        }

        // Update baselines with min 5 samples
        let updated = db.update_baselines(5).unwrap();
        assert!(updated.len() > 0);

        // Verify baselines were stored
        let baseline = db.get_baseline("tool_error_rate").unwrap();
        assert!(baseline.is_some());
    }

    #[test]
    fn test_detect_anomalies_rule_based() {
        let db = MetricsDb::in_memory().unwrap();

        // Session with high error rate (should trigger warning)
        let session_warning = SessionMetrics {
            id: "session-warning".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test".to_string(),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 0,
            cost_usd: 0.05,
            issues_claimed: 1,
            issues_completed: 1,
            tool_calls: 100,
            tool_errors: 15, // 15% error rate - should trigger warning
            final_status: SessionStatus::Completed,
        };

        let anomalies = db.detect_anomalies(&session_warning).unwrap();
        assert!(anomalies.len() > 0);
        assert!(anomalies.iter().any(|a| a.dimension == "tool_error_rate"));

        // Session with very high error rate (should trigger error)
        let session_error = SessionMetrics {
            id: "session-error".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test".to_string(),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 0,
            cost_usd: 0.05,
            issues_claimed: 1,
            issues_completed: 1,
            tool_calls: 100,
            tool_errors: 25, // 25% error rate - should trigger error
            final_status: SessionStatus::Completed,
        };

        let anomalies = db.detect_anomalies(&session_error).unwrap();
        assert!(anomalies.len() > 0);
        let error_anomaly = anomalies
            .iter()
            .find(|a| a.dimension == "tool_error_rate")
            .unwrap();
        assert_eq!(error_anomaly.severity, AnomalySeverity::Error);
    }

    #[test]
    fn test_detect_anomalies_with_baseline() {
        let db = MetricsDb::in_memory().unwrap();

        // Create baseline sessions with varying token usage
        for i in 0..20 {
            let session = SessionMetrics {
                id: format!("baseline-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0,
                tokens_in: 10000 + (i * 100) as i64, // Add variance: 10000-11900
                tokens_out: 5000 + (i * 50) as i64,  // Add variance: 5000-5950
                tokens_cached: 0,
                cost_usd: 0.05,
                issues_claimed: 1,
                issues_completed: 1,
                tool_calls: 100,
                tool_errors: 2, // Consistent 2% error rate
                final_status: SessionStatus::Completed,
            };
            db.store_session(&session).unwrap();
        }

        // Update baselines
        db.update_baselines(10).unwrap();

        // Now test with an anomalous session (much higher tokens per issue)
        let anomalous_session = SessionMetrics {
            id: "anomalous".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test".to_string(),
            duration_seconds: 100.0,
            tokens_in: 100000, // Much more tokens than baseline
            tokens_out: 50000,
            tokens_cached: 0,
            cost_usd: 0.50,
            issues_claimed: 1,
            issues_completed: 1,
            tool_calls: 100,
            tool_errors: 2,
            final_status: SessionStatus::Completed,
        };

        let anomalies = db.detect_anomalies(&anomalous_session).unwrap();
        assert!(anomalies.len() > 0);

        // Should detect anomaly in tokens_per_issue
        let tokens_anomaly = anomalies
            .iter()
            .find(|a| a.dimension == "tokens_per_issue");
        assert!(tokens_anomaly.is_some());
    }

    #[test]
    fn test_store_and_retrieve_baseline() {
        let db = MetricsDb::in_memory().unwrap();

        let baseline = Baseline {
            dimension: "test_metric".to_string(),
            mean: 100.0,
            stddev: 10.0,
            p50: 95.0,
            p90: 120.0,
            p99: 150.0,
            sample_count: 100,
            updated_at: Utc::now(),
        };

        db.store_baseline(&baseline).unwrap();

        let retrieved = db.get_baseline("test_metric").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.dimension, "test_metric");
        assert_eq!(retrieved.mean, 100.0);
        assert_eq!(retrieved.sample_count, 100);
    }
}
