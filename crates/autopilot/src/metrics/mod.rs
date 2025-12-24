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
//!
//! # Examples
//!
//! ## Storing session metrics
//!
//! ```ignore
//! use autopilot::metrics::{MetricsStore, SessionMetrics, SessionStatus};
//! use chrono::Utc;
//!
//! # fn example() -> anyhow::Result<()> {
//! let store = MetricsStore::new("autopilot-metrics.db")?;
//!
//! let metrics = SessionMetrics {
//!     id: "session-123".to_string(),
//!     timestamp: Utc::now(),
//!     model: "sonnet".to_string(),
//!     prompt: "Fix clippy warnings".to_string(),
//!     duration_seconds: 45.2,
//!     tokens_in: 5000,
//!     tokens_out: 2000,
//!     tokens_cached: 1000,
//!     cost_usd: 0.035,
//!     issues_claimed: 1,
//!     issues_completed: 1,
//!     tool_calls: 12,
//!     tool_errors: 0,
//!     final_status: SessionStatus::Completed,
//! };
//!
//! store.store_session(&metrics)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Querying metrics for analysis
//!
//! ```ignore
//! use autopilot::metrics::MetricsStore;
//!
//! # fn example() -> anyhow::Result<()> {
//! let store = MetricsStore::new("autopilot-metrics.db")?;
//!
//! // Get all sessions from the past week
//! let sessions = store.get_sessions_since(chrono::Duration::days(7))?;
//!
//! // Calculate average token efficiency
//! let avg_tokens_per_issue: f64 = sessions.iter()
//!     .filter(|s| s.issues_completed > 0)
//!     .map(|s| (s.tokens_in + s.tokens_out) as f64 / s.issues_completed as f64)
//!     .sum::<f64>() / sessions.len() as f64;
//!
//! println!("Average tokens per completed issue: {:.0}", avg_tokens_per_issue);
//! # Ok(())
//! # }
//! ```

pub mod baseline;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
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
    /// Number of messages (user + assistant)
    pub messages: i32,
    /// Actions Per Minute (APM) - calculated from (messages + tool_calls) / duration_minutes
    pub apm: Option<f64>,
    /// Source of the session data (autopilot, claude_code, or combined)
    pub source: String,
    /// Issue numbers claimed/completed during this session (comma-separated)
    pub issue_numbers: Option<String>,
    /// Directive ID this session was working on
    pub directive_id: Option<String>,
}

impl SessionMetrics {
    /// Calculate and set APM for this session
    ///
    /// APM = (messages + tool_calls) / duration_minutes
    pub fn calculate_apm(&mut self) {
        use crate::apm::calculate_apm;

        let duration_minutes = self.duration_seconds / 60.0;
        self.apm = calculate_apm(
            self.messages as u32,
            self.tool_calls as u32,
            duration_minutes,
        );
    }
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

/// Error recovery attempt tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecovery {
    /// Parent session ID
    pub session_id: String,
    /// Tool call ID that failed (if available)
    pub tool_call_id: Option<i64>,
    /// Recovery attempt timestamp
    pub timestamp: DateTime<Utc>,
    /// Type of error (EISDIR, ENOENT, etc.)
    pub error_type: String,
    /// Original error message
    pub original_error: String,
    /// Whether recovery was attempted
    pub recovery_attempted: bool,
    /// Recovery action taken
    pub recovery_action: Option<String>,
    /// Whether recovery succeeded
    pub recovery_succeeded: bool,
    /// Number of retries
    pub retry_count: i32,
    /// Final result after recovery
    pub final_result: Option<String>,
}

/// Aggregate metrics for a specific issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAggregateMetrics {
    pub issue_number: i32,
    pub sessions_count: i32,
    pub total_duration_seconds: f64,
    pub avg_duration_seconds: f64,
    pub total_tokens: i64,
    pub avg_tokens: f64,
    pub total_cost_usd: f64,
    pub avg_cost_usd: f64,
    pub tool_calls: i32,
    pub tool_errors: i32,
    pub error_rate: f64,
}

/// Aggregate metrics for a specific directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveAggregateMetrics {
    pub directive_id: String,
    pub sessions_count: i32,
    pub issues_completed: i32,
    pub total_duration_seconds: f64,
    pub avg_duration_seconds: f64,
    pub total_tokens: i64,
    pub avg_tokens: f64,
    pub total_cost_usd: f64,
    pub avg_cost_usd: f64,
    pub tool_calls: i32,
    pub tool_errors: i32,
    pub error_rate: f64,
}

/// Velocity snapshot tracking improvement rate over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocitySnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Time period analyzed (e.g., "week", "month")
    pub period: String,
    /// Overall velocity score (-1.0 to 1.0, higher is better)
    pub velocity_score: f64,
    /// Number of improving metrics
    pub improving_metrics: i32,
    /// Number of degrading metrics
    pub degrading_metrics: i32,
    /// Number of stable metrics
    pub stable_metrics: i32,
    /// Number of issues completed in this period
    pub issues_completed: i32,
    /// Key metrics with their trends (JSON)
    pub key_metrics: Vec<MetricVelocity>,
}

/// Individual metric velocity tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricVelocity {
    /// Metric name
    pub dimension: String,
    /// Percent change from previous period
    pub percent_change: f64,
    /// Trend direction
    pub direction: String,
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

/// Personal best record for a metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalBest {
    /// Metric name (e.g., "apm", "velocity_score")
    pub metric: String,
    /// Best value achieved
    pub value: f64,
    /// Session ID where this was achieved
    pub session_id: Option<String>,
    /// Project name (None for global)
    pub project: Option<String>,
    /// When this personal best was achieved
    pub timestamp: DateTime<Utc>,
    /// Optional context (e.g., "3 issues completed")
    pub context: Option<String>,
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
        // Enable foreign keys
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // Create schema_version table if it doesn't exist
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
            [],
        )?;

        // Get current schema version
        let version: i32 = self.conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap_or(0);

        // Run migrations if needed
        if version < 1 {
            self.migrate_v1()?;
        }
        if version < 2 {
            self.migrate_v2()?;
        }
        if version < 3 {
            self.migrate_v3()?;
        }

        if version < 4 {
            self.migrate_v4()?;
        }

        Ok(())
    }

    fn migrate_v1(&self) -> Result<()> {
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
                final_status TEXT NOT NULL,
                apm REAL,
                source TEXT DEFAULT 'autopilot',
                messages INTEGER NOT NULL DEFAULT 0
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
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
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
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
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

            CREATE TABLE IF NOT EXISTS apm_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                source TEXT NOT NULL,
                window TEXT NOT NULL,
                apm REAL NOT NULL,
                actions INTEGER NOT NULL,
                duration_minutes REAL NOT NULL,
                messages INTEGER NOT NULL,
                tool_calls INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS apm_baselines (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                source TEXT NOT NULL,
                median_apm REAL NOT NULL,
                min_apm REAL NOT NULL,
                max_apm REAL NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                sample_size INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS velocity_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                period TEXT NOT NULL,
                velocity_score REAL NOT NULL,
                improving_metrics INTEGER NOT NULL,
                degrading_metrics INTEGER NOT NULL,
                stable_metrics INTEGER NOT NULL,
                issues_completed INTEGER NOT NULL DEFAULT 0,
                key_metrics_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS personal_bests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric TEXT NOT NULL,
                value REAL NOT NULL,
                session_id TEXT,
                project TEXT,
                timestamp TEXT NOT NULL,
                context TEXT,
                UNIQUE(metric, project)
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_timestamp ON sessions(timestamp);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_session_id ON tool_calls(session_id);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_tool_name ON tool_calls(tool_name);
            CREATE INDEX IF NOT EXISTS idx_anomalies_session_id ON anomalies(session_id);
            CREATE INDEX IF NOT EXISTS idx_anomalies_severity ON anomalies(severity);
            CREATE INDEX IF NOT EXISTS idx_apm_snapshots_source_window ON apm_snapshots(source, window);
            CREATE INDEX IF NOT EXISTS idx_apm_snapshots_timestamp ON apm_snapshots(timestamp);
            CREATE INDEX IF NOT EXISTS idx_velocity_snapshots_timestamp ON velocity_snapshots(timestamp);
            CREATE INDEX IF NOT EXISTS idx_velocity_snapshots_period ON velocity_snapshots(period);
            "#,
        )
        .context("Failed to initialize database schema")?;

        // Initialize alert schema
        crate::alerts::init_alerts_schema(&self.conn)?;
        crate::alerts::add_default_alerts(&self.conn)?;

        // Migrate existing sessions table to add APM fields
        // This is safe because we're adding nullable columns with defaults
        let columns: Vec<String> = self.conn.prepare("PRAGMA table_info(sessions)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;

        if !columns.contains(&"apm".to_string()) {
            self.conn.execute("ALTER TABLE sessions ADD COLUMN apm REAL", [])?;
        }
        if !columns.contains(&"source".to_string()) {
            self.conn.execute("ALTER TABLE sessions ADD COLUMN source TEXT DEFAULT 'autopilot'", [])?;
        }
        if !columns.contains(&"messages".to_string()) {
            self.conn.execute("ALTER TABLE sessions ADD COLUMN messages INTEGER NOT NULL DEFAULT 0", [])?;
        }
        if !columns.contains(&"issue_numbers".to_string()) {
            self.conn.execute("ALTER TABLE sessions ADD COLUMN issue_numbers TEXT", [])?;
        }
        if !columns.contains(&"directive_id".to_string()) {
            self.conn.execute("ALTER TABLE sessions ADD COLUMN directive_id TEXT", [])?;
        }

        // Add indexes for issue and directive filtering
        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sessions_directive ON sessions(directive_id);
            "#
        )?;

        // Set schema version
        self.set_schema_version(1)?;

        Ok(())
    }

    fn migrate_v2(&self) -> Result<()> {
        // Recreate tool_calls table with ON DELETE CASCADE
        self.conn.execute_batch(
            r#"
            -- Recreate tool_calls with CASCADE
            CREATE TABLE tool_calls_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                success INTEGER NOT NULL,
                error_type TEXT,
                tokens_in INTEGER NOT NULL,
                tokens_out INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            INSERT INTO tool_calls_new SELECT * FROM tool_calls;
            DROP TABLE tool_calls;
            ALTER TABLE tool_calls_new RENAME TO tool_calls;
            CREATE INDEX idx_tool_calls_session_id ON tool_calls(session_id);
            CREATE INDEX idx_tool_calls_tool_name ON tool_calls(tool_name);

            -- Recreate anomalies with CASCADE
            CREATE TABLE anomalies_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                dimension TEXT NOT NULL,
                expected_value REAL NOT NULL,
                actual_value REAL NOT NULL,
                severity TEXT NOT NULL,
                investigated INTEGER NOT NULL DEFAULT 0,
                issue_number INTEGER,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            INSERT INTO anomalies_new SELECT * FROM anomalies;
            DROP TABLE anomalies;
            ALTER TABLE anomalies_new RENAME TO anomalies;
            CREATE INDEX idx_anomalies_session_id ON anomalies(session_id);
            CREATE INDEX idx_anomalies_severity ON anomalies(severity);
            "#
        )?;

        self.set_schema_version(2)?;
        Ok(())
    }

    fn migrate_v3(&self) -> Result<()> {
        // Add proposed_improvements table for storing automated refinement proposals
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS proposed_improvements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                improvement_type TEXT NOT NULL,
                description TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                proposed_fix TEXT NOT NULL,
                severity INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                file_path TEXT,
                section TEXT,
                current_text TEXT,
                new_text TEXT,
                rationale TEXT,
                created_at TEXT NOT NULL,
                reviewed_at TEXT,
                applied_at TEXT,
                reviewer_notes TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_proposed_improvements_status ON proposed_improvements(status);
            CREATE INDEX IF NOT EXISTS idx_proposed_improvements_created ON proposed_improvements(created_at);
            CREATE INDEX IF NOT EXISTS idx_proposed_improvements_severity ON proposed_improvements(severity);
            "#
        )?;

        self.set_schema_version(3)?;
        Ok(())
    }

    fn migrate_v4(&self) -> Result<()> {
        // Add error_recoveries table for tracking automatic error recovery
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS error_recoveries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                tool_call_id INTEGER,
                timestamp TEXT NOT NULL,
                error_type TEXT NOT NULL,
                original_error TEXT NOT NULL,
                recovery_attempted INTEGER NOT NULL DEFAULT 1,
                recovery_action TEXT,
                recovery_succeeded INTEGER NOT NULL DEFAULT 0,
                retry_count INTEGER NOT NULL DEFAULT 0,
                final_result TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
                FOREIGN KEY (tool_call_id) REFERENCES tool_calls(id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_error_recoveries_session ON error_recoveries(session_id);
            CREATE INDEX IF NOT EXISTS idx_error_recoveries_type ON error_recoveries(error_type);
            CREATE INDEX IF NOT EXISTS idx_error_recoveries_success ON error_recoveries(recovery_succeeded);
            CREATE INDEX IF NOT EXISTS idx_error_recoveries_timestamp ON error_recoveries(timestamp);
            "#
        )?;

        self.set_schema_version(4)?;
        Ok(())
    }

    fn set_schema_version(&self, version: i32) -> Result<()> {
        self.conn.execute("DELETE FROM schema_version", [])?;
        self.conn.execute("INSERT INTO schema_version (version) VALUES (?)", [version])?;
        Ok(())
    }

    /// Store session metrics
    pub fn store_session(&self, metrics: &SessionMetrics) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO sessions
            (id, timestamp, model, prompt, duration_seconds, tokens_in, tokens_out,
             tokens_cached, cost_usd, issues_claimed, issues_completed, tool_calls,
             tool_errors, final_status, messages, apm, source, issue_numbers, directive_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
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
                metrics.messages,
                metrics.apm,
                metrics.source,
                metrics.issue_numbers,
                metrics.directive_id,
            ],
        )
        .context("Failed to store session metrics")?;

        // Broadcast update to dashboard WebSocket clients (if dashboard is running)
        crate::dashboard::broadcast_metrics_update("session_updated", Some(metrics.id.clone()));

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

        // Broadcast tool call update for real-time monitoring
        crate::dashboard::broadcast_metrics_update("tool_call", Some(metrics.session_id.clone()));

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

        // Broadcast anomaly detection for real-time alerts
        crate::dashboard::broadcast_metrics_update("anomaly_detected", Some(anomaly.session_id.clone()));

        Ok(())
    }

    /// Store error recovery attempt
    pub fn store_error_recovery(&self, recovery: &ErrorRecovery) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO error_recoveries
            (session_id, tool_call_id, timestamp, error_type, original_error,
             recovery_attempted, recovery_action, recovery_succeeded, retry_count, final_result)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                recovery.session_id,
                recovery.tool_call_id,
                recovery.timestamp.to_rfc3339(),
                recovery.error_type,
                recovery.original_error,
                recovery.recovery_attempted as i32,
                recovery.recovery_action,
                recovery.recovery_succeeded as i32,
                recovery.retry_count,
                recovery.final_result,
            ],
        )
        .context("Failed to store error recovery")?;

        Ok(())
    }

    /// Get error recoveries for a specific session
    pub fn get_error_recoveries(&self, session_id: &str) -> Result<Vec<ErrorRecovery>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, tool_call_id, timestamp, error_type, original_error,
                   recovery_attempted, recovery_action, recovery_succeeded, retry_count, final_result
            FROM error_recoveries
            WHERE session_id = ?1
            ORDER BY timestamp DESC
            "#,
        )?;

        let recoveries = stmt
            .query_map(params![session_id], |row| {
                let timestamp_str = row.get::<_, String>(2)?;
                let timestamp = timestamp_str.parse()
                    .unwrap_or_else(|_| Utc::now());

                Ok(ErrorRecovery {
                    session_id: row.get(0)?,
                    tool_call_id: row.get(1)?,
                    timestamp,
                    error_type: row.get(3)?,
                    original_error: row.get(4)?,
                    recovery_attempted: row.get::<_, i32>(5)? != 0,
                    recovery_action: row.get(6)?,
                    recovery_succeeded: row.get::<_, i32>(7)? != 0,
                    retry_count: row.get(8)?,
                    final_result: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(recoveries)
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
                let severity_str = row.get::<_, String>(4)?;
                let severity = AnomalySeverity::from_str(&severity_str)
                    .unwrap_or_else(|e| {
                        eprintln!("Invalid anomaly severity '{}': {}. Using 'warning' as fallback.", severity_str, e);
                        AnomalySeverity::Warning
                    });
                Ok(Anomaly {
                    session_id: row.get(0)?,
                    dimension: row.get(1)?,
                    expected_value: row.get(2)?,
                    actual_value: row.get(3)?,
                    severity,
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
            let timestamp_str = row.get::<_, String>(1)?;
            let timestamp = timestamp_str.parse()
                .unwrap_or_else(|e| {
                    eprintln!("Invalid timestamp '{}': {}. Using current time.", timestamp_str, e);
                    Utc::now()
                });

            let status_str = row.get::<_, String>(13)?;
            let final_status = SessionStatus::from_str(&status_str)
                .unwrap_or_else(|e| {
                    eprintln!("Invalid session status '{}': {}. Using 'crashed' as fallback.", status_str, e);
                    SessionStatus::Crashed
                });

            Ok(SessionMetrics {
                id: row.get(0)?,
                timestamp,
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
                final_status,
                messages: row.get(14).unwrap_or(0),
                apm: row.get(15).ok(),
                source: row.get(16).unwrap_or_else(|_| "autopilot".to_string()),
                issue_numbers: row.get(17).ok(),
                directive_id: row.get(18).ok(),
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
                    messages: row.get(14).unwrap_or(0),
                    apm: row.get(15).ok(),
                    source: row.get(16).unwrap_or_else(|_| "autopilot".to_string()),
                issue_numbers: row.get(17).ok(),
                directive_id: row.get(18).ok(),
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

    /// Get recent sessions (most recent first)
    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, model, prompt, duration_seconds,
                   tokens_in, tokens_out, tokens_cached, cost_usd,
                   issues_claimed, issues_completed, tool_calls,
                   tool_errors, final_status
            FROM sessions
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )?;

        let sessions = stmt
            .query_map(params![limit as i64], |row| {
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
                    final_status: match row.get::<_, String>(13)?.as_str() {
                        "completed" => SessionStatus::Completed,
                        "crashed" => SessionStatus::Crashed,
                        "budget_exhausted" => SessionStatus::BudgetExhausted,
                        "max_turns" => SessionStatus::MaxTurns,
                        "running" => SessionStatus::Running,
                        _ => SessionStatus::Crashed,
                    },
                    messages: row.get(14).unwrap_or(0),
                    apm: row.get(15).ok(),
                    source: row.get(16).unwrap_or_else(|_| "autopilot".to_string()),
                issue_numbers: row.get(17).ok(),
                directive_id: row.get(18).ok(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Get summary statistics for dashboard
    pub fn get_summary_stats(&self) -> Result<crate::dashboard::SummaryStats> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COUNT(*) as total_sessions,
                COALESCE(SUM(issues_completed), 0) as total_issues,
                COALESCE(SUM(cost_usd), 0) as total_cost,
                COALESCE(AVG(duration_seconds), 0) as avg_duration,
                COALESCE(AVG(tokens_in + tokens_out), 0) as avg_tokens,
                COALESCE(
                    CAST(SUM(issues_completed) AS REAL) / NULLIF(SUM(issues_claimed), 0),
                    0
                ) as completion_rate
            FROM sessions
            "#,
        )?;

        let stats = stmt.query_row([], |row| {
            Ok(crate::dashboard::SummaryStats {
                total_sessions: row.get(0)?,
                total_issues_completed: row.get(1)?,
                total_cost_usd: row.get(2)?,
                avg_duration_seconds: row.get(3)?,
                avg_tokens_per_session: row.get(4)?,
                completion_rate: row.get(5)?,
            })
        })?;

        Ok(stats)
    }

    /// Get uninvestigated anomalies from the database
    pub fn get_uninvestigated_anomalies(&self) -> Result<Vec<(String, String, f64, f64, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, dimension, expected_value, actual_value, severity
            FROM anomalies
            WHERE investigated = 0 AND issue_number IS NULL
            ORDER BY severity DESC
            "#,
        )?;

        let anomalies = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(anomalies)
    }

    /// Mark anomalies as investigated with issue number
    pub fn mark_anomalies_investigated(
        &self,
        session_ids: &[(String, String)],
        issue_number: i32,
    ) -> Result<()> {
        for (session_id, dimension) in session_ids {
            self.conn.execute(
                r#"
                UPDATE anomalies
                SET investigated = 1, issue_number = ?1
                WHERE session_id = ?2 AND dimension = ?3
                "#,
                rusqlite::params![issue_number, session_id, dimension],
            )?;
        }

        Ok(())
    }

    /// Get sessions for a specific issue number
    pub fn get_sessions_for_issue(&self, issue_number: i32) -> Result<Vec<SessionMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, model, prompt, duration_seconds, tokens_in,
                   tokens_out, tokens_cached, cost_usd, issues_claimed,
                   issues_completed, tool_calls, tool_errors, final_status,
                   messages, apm, source, issue_numbers, directive_id
            FROM sessions
            WHERE issue_numbers LIKE ?1
            ORDER BY timestamp DESC
            "#,
        )?;

        let search_pattern = format!("%{}%", issue_number);
        let sessions = stmt
            .query_map(params![search_pattern], |row| {
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
                    messages: row.get(14).unwrap_or(0),
                    apm: row.get(15).ok(),
                    source: row.get(16).unwrap_or_else(|_| "autopilot".to_string()),
                    issue_numbers: row.get(17).ok(),
                    directive_id: row.get(18).ok(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Get sessions for a specific directive
    pub fn get_sessions_for_directive(&self, directive_id: &str) -> Result<Vec<SessionMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, model, prompt, duration_seconds, tokens_in,
                   tokens_out, tokens_cached, cost_usd, issues_claimed,
                   issues_completed, tool_calls, tool_errors, final_status,
                   messages, apm, source, issue_numbers, directive_id
            FROM sessions
            WHERE directive_id = ?1
            ORDER BY timestamp DESC
            "#,
        )?;

        let sessions = stmt
            .query_map(params![directive_id], |row| {
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
                    messages: row.get(14).unwrap_or(0),
                    apm: row.get(15).ok(),
                    source: row.get(16).unwrap_or_else(|_| "autopilot".to_string()),
                    issue_numbers: row.get(17).ok(),
                    directive_id: row.get(18).ok(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Calculate aggregate metrics for a specific issue
    pub fn get_issue_aggregate_metrics(&self, issue_number: i32) -> Result<Option<IssueAggregateMetrics>> {
        let sessions = self.get_sessions_for_issue(issue_number)?;

        if sessions.is_empty() {
            return Ok(None);
        }

        let total_duration: f64 = sessions.iter().map(|s| s.duration_seconds).sum();
        let total_tokens: i64 = sessions.iter().map(|s| s.tokens_in + s.tokens_out).sum();
        let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();
        let total_tool_calls: i32 = sessions.iter().map(|s| s.tool_calls).sum();
        let total_tool_errors: i32 = sessions.iter().map(|s| s.tool_errors).sum();
        let sessions_count = sessions.len() as i32;

        Ok(Some(IssueAggregateMetrics {
            issue_number,
            sessions_count,
            total_duration_seconds: total_duration,
            avg_duration_seconds: total_duration / sessions_count as f64,
            total_tokens,
            avg_tokens: total_tokens as f64 / sessions_count as f64,
            total_cost_usd: total_cost,
            avg_cost_usd: total_cost / sessions_count as f64,
            tool_calls: total_tool_calls,
            tool_errors: total_tool_errors,
            error_rate: if total_tool_calls > 0 {
                (total_tool_errors as f64) / (total_tool_calls as f64)
            } else {
                0.0
            },
        }))
    }

    /// Calculate aggregate metrics for a specific directive
    pub fn get_directive_aggregate_metrics(&self, directive_id: &str) -> Result<Option<DirectiveAggregateMetrics>> {
        let sessions = self.get_sessions_for_directive(directive_id)?;

        if sessions.is_empty() {
            return Ok(None);
        }

        let total_duration: f64 = sessions.iter().map(|s| s.duration_seconds).sum();
        let total_tokens: i64 = sessions.iter().map(|s| s.tokens_in + s.tokens_out).sum();
        let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();
        let total_issues_completed: i32 = sessions.iter().map(|s| s.issues_completed).sum();
        let total_tool_calls: i32 = sessions.iter().map(|s| s.tool_calls).sum();
        let total_tool_errors: i32 = sessions.iter().map(|s| s.tool_errors).sum();
        let sessions_count = sessions.len() as i32;

        Ok(Some(DirectiveAggregateMetrics {
            directive_id: directive_id.to_string(),
            sessions_count,
            issues_completed: total_issues_completed,
            total_duration_seconds: total_duration,
            avg_duration_seconds: total_duration / sessions_count as f64,
            total_tokens,
            avg_tokens: total_tokens as f64 / sessions_count as f64,
            total_cost_usd: total_cost,
            avg_cost_usd: total_cost / sessions_count as f64,
            tool_calls: total_tool_calls,
            tool_errors: total_tool_errors,
            error_rate: if total_tool_calls > 0 {
                (total_tool_errors as f64) / (total_tool_calls as f64)
            } else {
                0.0
            },
        }))
    }

    /// Get a reference to the database connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get aggregate metrics for a specific issue
    pub fn get_issue_metrics(&self, issue_number: i32) -> Result<Option<IssueAggregateMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COUNT(*) as sessions_count,
                SUM(duration_seconds) as total_duration,
                AVG(duration_seconds) as avg_duration,
                SUM(tokens_in + tokens_out) as total_tokens,
                AVG(tokens_in + tokens_out) as avg_tokens,
                SUM(cost_usd) as total_cost,
                AVG(cost_usd) as avg_cost,
                SUM(tool_calls) as tool_calls,
                SUM(tool_errors) as tool_errors
            FROM sessions
            WHERE issue_numbers LIKE '%' || ?1 || '%'
            "#,
        )?;

        let result = stmt.query_row([issue_number], |row| {
            let sessions_count: i32 = row.get(0)?;
            if sessions_count == 0 {
                return Ok(None);
            }

            let tool_calls: i32 = row.get(7)?;
            let tool_errors: i32 = row.get(8)?;
            let error_rate = if tool_calls > 0 {
                (tool_errors as f64 / tool_calls as f64) * 100.0
            } else {
                0.0
            };

            Ok(Some(IssueAggregateMetrics {
                issue_number,
                sessions_count,
                total_duration_seconds: row.get(1)?,
                avg_duration_seconds: row.get(2)?,
                total_tokens: row.get(3)?,
                avg_tokens: row.get(4)?,
                total_cost_usd: row.get(5)?,
                avg_cost_usd: row.get(6)?,
                tool_calls,
                tool_errors,
                error_rate,
            }))
        })?;

        Ok(result)
    }

    /// Get aggregate metrics for a specific directive
    pub fn get_directive_metrics(&self, directive_id: &str) -> Result<Option<DirectiveAggregateMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COUNT(*) as sessions_count,
                SUM(issues_completed) as issues_completed,
                SUM(duration_seconds) as total_duration,
                AVG(duration_seconds) as avg_duration,
                SUM(tokens_in + tokens_out) as total_tokens,
                AVG(tokens_in + tokens_out) as avg_tokens,
                SUM(cost_usd) as total_cost,
                AVG(cost_usd) as avg_cost,
                SUM(tool_calls) as tool_calls,
                SUM(tool_errors) as tool_errors
            FROM sessions
            WHERE directive_id = ?1
            "#,
        )?;

        let result = stmt.query_row([directive_id], |row| {
            let sessions_count: i32 = row.get(0)?;
            if sessions_count == 0 {
                return Ok(None);
            }

            let tool_calls: i32 = row.get(8)?;
            let tool_errors: i32 = row.get(9)?;
            let error_rate = if tool_calls > 0 {
                (tool_errors as f64 / tool_calls as f64) * 100.0
            } else {
                0.0
            };

            Ok(Some(DirectiveAggregateMetrics {
                directive_id: directive_id.to_string(),
                sessions_count,
                issues_completed: row.get(1)?,
                total_duration_seconds: row.get(2)?,
                avg_duration_seconds: row.get(3)?,
                total_tokens: row.get(4)?,
                avg_tokens: row.get(5)?,
                total_cost_usd: row.get(6)?,
                avg_cost_usd: row.get(7)?,
                tool_calls,
                tool_errors,
                error_rate,
            }))
        })?;

        Ok(result)
    }

    /// Get all issue metrics
    pub fn get_all_issue_metrics(&self) -> Result<Vec<IssueAggregateMetrics>> {
        // This is a simplified implementation - it gets unique issue numbers
        // and then calls get_issue_metrics for each
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT issue_numbers
            FROM sessions
            WHERE issue_numbers IS NOT NULL
            "#,
        )?;

        let issue_numbers_raw: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut metrics = Vec::new();
        for issue_str in issue_numbers_raw {
            // Parse comma-separated issue numbers
            for num_str in issue_str.split(',') {
                if let Ok(num) = num_str.trim().parse::<i32>() {
                    if let Some(metric) = self.get_issue_metrics(num)? {
                        metrics.push(metric);
                    }
                }
            }
        }

        // Deduplicate by issue_number
        metrics.sort_by_key(|m| m.issue_number);
        metrics.dedup_by_key(|m| m.issue_number);

        Ok(metrics)
    }

    /// Get all directive metrics
    pub fn get_all_directive_metrics(&self) -> Result<Vec<DirectiveAggregateMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT directive_id
            FROM sessions
            WHERE directive_id IS NOT NULL
            "#,
        )?;

        let directive_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut metrics = Vec::new();
        for id in directive_ids {
            if let Some(metric) = self.get_directive_metrics(&id)? {
                metrics.push(metric);
            }
        }

        Ok(metrics)
    }

    /// Store velocity snapshot
    pub fn store_velocity_snapshot(&self, snapshot: &VelocitySnapshot) -> Result<()> {
        let key_metrics_json = serde_json::to_string(&snapshot.key_metrics)?;

        self.conn.execute(
            r#"
            INSERT INTO velocity_snapshots
            (timestamp, period, velocity_score, improving_metrics, degrading_metrics,
             stable_metrics, issues_completed, key_metrics_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                snapshot.timestamp.to_rfc3339(),
                snapshot.period,
                snapshot.velocity_score,
                snapshot.improving_metrics,
                snapshot.degrading_metrics,
                snapshot.stable_metrics,
                snapshot.issues_completed,
                key_metrics_json
            ],
        )?;

        Ok(())
    }

    /// Get recent velocity snapshots
    pub fn get_velocity_snapshots(&self, limit: usize) -> Result<Vec<VelocitySnapshot>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT timestamp, period, velocity_score, improving_metrics,
                   degrading_metrics, stable_metrics, issues_completed, key_metrics_json
            FROM velocity_snapshots
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )?;

        let snapshots = stmt
            .query_map([limit], |row| {
                let key_metrics_json: String = row.get(7)?;
                let key_metrics: Vec<MetricVelocity> =
                    serde_json::from_str(&key_metrics_json).unwrap_or_default();

                Ok(VelocitySnapshot {
                    timestamp: row.get::<_, String>(0)?.parse().unwrap(),
                    period: row.get(1)?,
                    velocity_score: row.get(2)?,
                    improving_metrics: row.get(3)?,
                    degrading_metrics: row.get(4)?,
                    stable_metrics: row.get(5)?,
                    issues_completed: row.get(6)?,
                    key_metrics,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    /// Update personal best for a metric
    pub fn update_personal_best(
        &self,
        metric: &str,
        value: f64,
        session_id: Option<&str>,
        project: Option<&str>,
        context: Option<&str>,
    ) -> Result<bool> {
        let timestamp = Utc::now().to_rfc3339();

        // Check if this beats the current personal best
        let current_best: Option<f64> = self.conn.query_row(
            "SELECT value FROM personal_bests WHERE metric = ? AND project IS ?",
            params![metric, project],
            |row| row.get(0),
        ).optional()?;

        let is_new_best = current_best.map_or(true, |best| value > best);

        if is_new_best {
            self.conn.execute(
                r#"
                INSERT OR REPLACE INTO personal_bests
                (metric, value, session_id, project, timestamp, context)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![metric, value, session_id, project, timestamp, context],
            )?;
        }

        Ok(is_new_best)
    }

    /// Get personal best for a metric
    pub fn get_personal_best(
        &self,
        metric: &str,
        project: Option<&str>,
    ) -> Result<Option<PersonalBest>> {
        self.conn.query_row(
            "SELECT metric, value, session_id, project, timestamp, context FROM personal_bests WHERE metric = ? AND project IS ?",
            params![metric, project],
            |row| {
                Ok(PersonalBest {
                    metric: row.get(0)?,
                    value: row.get(1)?,
                    session_id: row.get(2)?,
                    project: row.get(3)?,
                    timestamp: row.get::<_, String>(4)?.parse().unwrap(),
                    context: row.get(5)?,
                })
            },
        ).optional()
        .map_err(|e| anyhow::Error::new(e))
    }

    /// Get all personal bests
    pub fn get_all_personal_bests(&self) -> Result<Vec<PersonalBest>> {
        let mut stmt = self.conn.prepare(
            "SELECT metric, value, session_id, project, timestamp, context FROM personal_bests ORDER BY timestamp DESC"
        )?;

        let bests = stmt.query_map([], |row| {
            Ok(PersonalBest {
                metric: row.get(0)?,
                value: row.get(1)?,
                session_id: row.get(2)?,
                project: row.get(3)?,
                timestamp: row.get::<_, String>(4)?.parse().unwrap(),
                context: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(bests)
    }
}

impl Drop for MetricsDb {
    fn drop(&mut self) {
        // Ensure any pending transactions are handled
        // Connection::close() is called automatically by rusqlite's Drop impl,
        // but we can add explicit cleanup here if needed in the future

        // Note: rusqlite's Connection already has a Drop impl that:
        // - Rolls back any uncommitted transaction
        // - Closes the database connection
        // This explicit Drop serves as documentation and allows future cleanup
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
                            output.as_ref().map(|o| {
                                // Try to extract error type from output
                                if o.contains("EISDIR") {
                                    "EISDIR".to_string()
                                } else if o.contains("ENOENT") {
                                    "ENOENT".to_string()
                                } else if o.contains("Exit code") {
                                    "NonZeroExit".to_string()
                                } else {
                                    "Unknown".to_string()
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

    // Count messages (user + assistant messages)
    let messages_count = traj
        .steps
        .iter()
        .filter(|s| matches!(&s.step_type, StepType::User { .. } | StepType::Assistant { .. }))
        .count() as i32;

    let mut session_metrics = SessionMetrics {
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
        messages: messages_count,
        apm: None,
                issue_numbers: None,
                directive_id: None,
        source: "autopilot".to_string(),
    };

    // Calculate APM
    session_metrics.calculate_apm();

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

/// Backfill APM data for existing sessions in the database
///
/// Calculates APM for all sessions that don't have it set yet and updates them.
pub fn backfill_apm_for_sessions(db_path: &Path) -> Result<usize> {
    use crate::apm::calculate_apm;

    let db = MetricsDb::open(db_path)?;
    let conn = &db.conn;

    // Get all sessions without APM
    let mut stmt = conn.prepare(
        "SELECT id, duration_seconds, tool_calls, messages FROM sessions WHERE apm IS NULL"
    )?;

    let sessions: Vec<(String, f64, u32, u32)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut updated = 0;
    for (id, duration_seconds, tool_calls, messages) in sessions {
        let duration_minutes = duration_seconds / 60.0;

        if let Some(apm) = calculate_apm(messages, tool_calls, duration_minutes) {
            conn.execute(
                "UPDATE sessions SET apm = ?1 WHERE id = ?2",
                rusqlite::params![apm, id],
            )?;
            updated += 1;
        }
    }

    Ok(updated)
}

/// Backfill metrics from existing trajectory logs in docs/logs/
///
/// Scans the logs directory for .jsonl files (preferred) and .rlog files (fallback),
/// extracts metrics from each trajectory, and imports them into the metrics database.
/// This provides historical baseline data for trend analysis and regression detection.
///
/// Returns a tuple of (files_processed, records_created, errors_encountered)
pub fn backfill_metrics_from_logs(logs_dir: &Path, db_path: &Path) -> Result<(usize, usize, usize)> {
    use walkdir::WalkDir;

    let db = MetricsDb::open(db_path)?;
    let mut files_processed = 0;
    let mut records_created = 0;
    let mut errors = 0;

    // Walk through all date directories (YYYYMMDD)
    for entry in WalkDir::new(logs_dir)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip if not a .jsonl file
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }

        // Check if this session already exists in the database
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            // Extract session_id from filename (before first dash)
            // Example: 133825-call-issue-ready-now-to.jsonl -> 133825
            let session_id_prefix = filename.split('-').next().unwrap_or("");

            // Check if we already have a session with this timestamp
            let exists = db.conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE id LIKE ?1",
                params![format!("%{}%", session_id_prefix)],
                |row| row.get::<_, i64>(0)
            ).unwrap_or(0) > 0;

            if exists {
                // Skip this file - already imported
                continue;
            }
        }

        // Try to extract metrics from the .jsonl file
        match extract_metrics_from_jsonl_file(path) {
            Ok((session_metrics, tool_call_metrics)) => {
                // Store session metrics
                if let Err(e) = db.store_session(&session_metrics) {
                    eprintln!("Error storing session metrics from {:?}: {}", path, e);
                    errors += 1;
                    continue;
                }

                // Store tool call metrics
                for tcm in tool_call_metrics {
                    if let Err(e) = db.store_tool_call(&tcm) {
                        eprintln!("Error storing tool call metric: {}", e);
                        errors += 1;
                    }
                }

                files_processed += 1;
                records_created += 1;
            }
            Err(e) => {
                eprintln!("Error extracting metrics from {:?}: {}", path, e);
                errors += 1;
            }
        }
    }

    Ok((files_processed, records_created, errors))
}

/// Extract metrics from a .jsonl file (Claude Code trajectory format)
///
/// Parses the JSONL file and constructs a Trajectory object from the SDK messages.
fn extract_metrics_from_jsonl_file<P: AsRef<Path>>(
    path: P,
) -> Result<(SessionMetrics, Vec<ToolCallMetrics>)> {
    use std::io::{BufRead, BufReader};
    use std::fs::File;

    let file = File::open(path.as_ref())
        .with_context(|| format!("Failed to open file: {:?}", path.as_ref()))?;
    let reader = BufReader::new(file);

    let mut session_id = String::new();
    let mut model = String::from("unknown");
    let mut prompt = String::new();
    let mut started_at = chrono::Utc::now();
    let mut ended_at: Option<DateTime<Utc>> = None;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut cache_read_tokens = 0u64;
    let mut messages = 0;
    let mut tool_calls = 0;

    // Parse the JSONL file line by line
    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result.with_context(|| format!("Failed to read line {}", line_num))?;

        let msg: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON on line {}", line_num))?;

        // Extract session_id from system init message
        if msg["type"] == "system" && msg["message"]["subtype"] == "init" {
            if let Some(sid) = msg["message"]["session_id"].as_str() {
                session_id = sid.to_string();
            }
            if let Some(m) = msg["message"]["model"].as_str() {
                model = m.to_string();
            }
            if let Some(ts) = msg["timestamp"].as_str() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                    started_at = dt.with_timezone(&chrono::Utc);
                }
            }
        }

        // Extract prompt from first user message
        if prompt.is_empty() && msg["type"] == "user" {
            if let Some(content_arr) = msg["message"]["content"].as_array() {
                for item in content_arr {
                    if item["type"] == "text" {
                        if let Some(text) = item["text"].as_str() {
                            prompt = text.to_string();
                            break;
                        }
                    }
                }
            }
        }

        // Count messages and extract token usage
        if msg["type"] == "assistant" {
            messages += 1;

            if let Some(usage) = msg["message"]["usage"].as_object() {
                total_input_tokens += usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                total_output_tokens += usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                cache_read_tokens += usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            }

            // Count tool calls
            if let Some(content_arr) = msg["message"]["content"].as_array() {
                for item in content_arr {
                    if item["type"] == "tool_use" {
                        tool_calls += 1;
                    }
                }
            }

            // Update timestamp
            if let Some(ts) = msg["timestamp"].as_str() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                    ended_at = Some(dt.with_timezone(&chrono::Utc));
                }
            }
        }
    }

    // Calculate duration
    let duration_seconds = if let Some(end) = ended_at {
        (end - started_at).num_milliseconds() as f64 / 1000.0
    } else {
        0.0
    };

    // Create session metrics
    let mut session_metrics = SessionMetrics {
        id: if session_id.is_empty() {
            // Fallback: use filename as session ID
            path.as_ref()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            session_id
        },
        timestamp: started_at,
        model,
        prompt: if prompt.len() > 500 {
            format!("{}...", &prompt[..500])
        } else {
            prompt
        },
        duration_seconds,
        tokens_in: total_input_tokens as i64,
        tokens_out: total_output_tokens as i64,
        tokens_cached: cache_read_tokens as i64,
        cost_usd: calculate_cost(total_input_tokens, total_output_tokens, cache_read_tokens),
        issues_claimed: 0, // Not available from JSONL
        issues_completed: 0, // Not available from JSONL
        tool_calls: tool_calls as i32,
        tool_errors: 0, // Would need to parse tool results
        final_status: SessionStatus::Completed,
        messages: messages as i32,
        apm: None,
        source: "backfill".to_string(),
        issue_numbers: None,
        directive_id: None,
    };

    session_metrics.calculate_apm();

    // For now, return empty tool call metrics (could be enhanced later)
    Ok((session_metrics, vec![]))
}

/// Calculate approximate cost based on token usage
/// Using rough estimates for Sonnet 4.5 pricing
fn calculate_cost(input_tokens: u64, output_tokens: u64, cache_tokens: u64) -> f64 {
    const INPUT_PRICE_PER_M: f64 = 3.0;
    const OUTPUT_PRICE_PER_M: f64 = 15.0;
    const CACHE_PRICE_PER_M: f64 = 0.3;

    let input_cost = (input_tokens as f64 / 1_000_000.0) * INPUT_PRICE_PER_M;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_M;
    let cache_cost = (cache_tokens as f64 / 1_000_000.0) * CACHE_PRICE_PER_M;

    input_cost + output_cost + cache_cost
}

/// Store an APM snapshot
pub fn store_apm_snapshot(
    db_path: &Path,
    snapshot: &crate::apm::APMSnapshot,
) -> Result<()> {
    let db = MetricsDb::open(db_path)?;

    db.conn.execute(
        r#"
        INSERT INTO apm_snapshots (timestamp, source, window, apm, actions, duration_minutes, messages, tool_calls)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        rusqlite::params![
            snapshot.timestamp.to_rfc3339(),
            snapshot.source.as_str(),
            snapshot.window.as_str(),
            snapshot.apm,
            snapshot.actions,
            snapshot.duration_minutes,
            snapshot.messages,
            snapshot.tool_calls,
        ],
    )?;

    Ok(())
}

/// Store or update an APM baseline
pub fn store_apm_baseline(
    db_path: &Path,
    baseline: &crate::apm::APMBaseline,
) -> Result<()> {
    let db = MetricsDb::open(db_path)?;

    db.conn.execute(
        r#"
        INSERT OR REPLACE INTO apm_baselines
        (id, name, source, median_apm, min_apm, max_apm, created_at, updated_at, sample_size)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        rusqlite::params![
            baseline.id,
            baseline.name,
            baseline.source.as_str(),
            baseline.median_apm,
            baseline.min_apm,
            baseline.max_apm,
            baseline.created_at.to_rfc3339(),
            baseline.updated_at.to_rfc3339(),
            baseline.sample_size,
        ],
    )?;

    Ok(())
}

/// Get an APM baseline by ID
pub fn get_apm_baseline(
    db_path: &Path,
    baseline_id: &str,
) -> Result<Option<crate::apm::APMBaseline>> {
    let db = MetricsDb::open(db_path)?;

    let mut stmt = db.conn.prepare(
        r#"
        SELECT id, name, source, median_apm, min_apm, max_apm, created_at, updated_at, sample_size
        FROM apm_baselines
        WHERE id = ?1
        "#,
    )?;

    let baseline = stmt.query_row(params![baseline_id], |row| {
        let source_str: String = row.get(2)?;
        let source = match source_str.as_str() {
            "autopilot" => crate::apm::APMSource::Autopilot,
            "claude_code" => crate::apm::APMSource::ClaudeCode,
            _ => crate::apm::APMSource::Combined,
        };

        let created_at: String = row.get(6)?;
        let updated_at: String = row.get(7)?;

        Ok(crate::apm::APMBaseline {
            id: row.get(0)?,
            name: row.get(1)?,
            source,
            median_apm: row.get(3)?,
            min_apm: row.get(4)?,
            max_apm: row.get(5)?,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at)
                .unwrap()
                .with_timezone(&Utc),
            sample_size: row.get(8)?,
        })
    });

    match baseline {
        Ok(b) => Ok(Some(b)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List all APM baselines
pub fn list_apm_baselines(db_path: &Path) -> Result<Vec<crate::apm::APMBaseline>> {
    let db = MetricsDb::open(db_path)?;

    let mut stmt = db.conn.prepare(
        r#"
        SELECT id, name, source, median_apm, min_apm, max_apm, created_at, updated_at, sample_size
        FROM apm_baselines
        ORDER BY source, median_apm DESC
        "#,
    )?;

    let baselines = stmt
        .query_map([], |row| {
            let source_str: String = row.get(2)?;
            let source = match source_str.as_str() {
                "autopilot" => crate::apm::APMSource::Autopilot,
                "claude_code" => crate::apm::APMSource::ClaudeCode,
                _ => crate::apm::APMSource::Combined,
            };

            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            Ok(crate::apm::APMBaseline {
                id: row.get(0)?,
                name: row.get(1)?,
                source,
                median_apm: row.get(3)?,
                min_apm: row.get(4)?,
                max_apm: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .unwrap()
                    .with_timezone(&Utc),
                sample_size: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(baselines)
}

/// Delete an APM baseline
pub fn delete_apm_baseline(db_path: &Path, baseline_id: &str) -> Result<()> {
    let db = MetricsDb::open(db_path)?;

    db.conn.execute(
        "DELETE FROM apm_baselines WHERE id = ?1",
        params![baseline_id],
    )?;

    Ok(())
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
                apm: Some(20.0),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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
            messages: 10,
            apm: None,
                issue_numbers: None,
                directive_id: None,
            source: "autopilot".to_string(),
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

#[cfg(test)]
mod metrics_cascade_tests {
    use crate::metrics::MetricsDb;
    use tempfile::tempdir;

    #[test]
    fn test_cascade_delete_tool_calls() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_metrics.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Insert a session
        db.conn.execute(
            "INSERT INTO sessions (id, timestamp, model, prompt, duration_seconds, tokens_in, tokens_out, tokens_cached, cost_usd, issues_claimed, issues_completed, tool_calls, tool_errors, final_status, messages) 
             VALUES ('test-session', '2024-01-01T00:00:00Z', 'claude-3', 'test', 1.0, 100, 50, 0, 0.01, 1, 0, 2, 0, 'success', 10)",
            [],
        ).unwrap();

        // Insert tool calls
        db.conn.execute(
            "INSERT INTO tool_calls (session_id, timestamp, tool_name, duration_ms, success, tokens_in, tokens_out)
             VALUES ('test-session', '2024-01-01T00:00:00Z', 'test_tool', 100, 1, 10, 5)",
            [],
        ).unwrap();

        // Verify tool_call exists
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM tool_calls WHERE session_id = 'test-session'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);

        // Delete session
        db.conn.execute("DELETE FROM sessions WHERE id = 'test-session'", []).unwrap();

        // Verify tool_call was CASCADE deleted
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM tool_calls WHERE session_id = 'test-session'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cascade_delete_anomalies() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_metrics.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Insert a session
        db.conn.execute(
            "INSERT INTO sessions (id, timestamp, model, prompt, duration_seconds, tokens_in, tokens_out, tokens_cached, cost_usd, issues_claimed, issues_completed, tool_calls, tool_errors, final_status, messages) 
             VALUES ('test-session', '2024-01-01T00:00:00Z', 'claude-3', 'test', 1.0, 100, 50, 0, 0.01, 1, 0, 2, 0, 'success', 10)",
            [],
        ).unwrap();

        // Insert anomaly
        db.conn.execute(
            "INSERT INTO anomalies (session_id, dimension, expected_value, actual_value, severity)
             VALUES ('test-session', 'duration', 5.0, 15.0, 'medium')",
            [],
        ).unwrap();

        // Verify anomaly exists
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM anomalies WHERE session_id = 'test-session'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);

        // Delete session
        db.conn.execute("DELETE FROM sessions WHERE id = 'test-session'", []).unwrap();

        // Verify anomaly was CASCADE deleted
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM anomalies WHERE session_id = 'test-session'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }
}
