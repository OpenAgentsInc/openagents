//! Baseline comparison system for Terminal-Bench.
//!
//! Tracks pass rate deltas, detects regressions, and reports improvements.
//! Uses SQLite for persistent storage.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Types
// ============================================================================

/// Task result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pass,
    Fail,
    Timeout,
    Error,
    Skip,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pass => "pass",
            TaskStatus::Fail => "fail",
            TaskStatus::Timeout => "timeout",
            TaskStatus::Error => "error",
            TaskStatus::Skip => "skip",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pass" => Some(TaskStatus::Pass),
            "fail" => Some(TaskStatus::Fail),
            "timeout" => Some(TaskStatus::Timeout),
            "error" => Some(TaskStatus::Error),
            "skip" => Some(TaskStatus::Skip),
            _ => None,
        }
    }
}

/// A baseline record - snapshot of a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRecord {
    pub id: i64,
    pub model: String,
    pub suite_name: String,
    pub suite_version: String,
    pub timestamp: DateTime<Utc>,
    pub pass_rate: f64,
    pub passed: u32,
    pub total: u32,
    pub task_results: HashMap<String, TaskStatus>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub notes: Option<String>,
}

/// Input for creating a baseline (without auto-generated fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineInput {
    pub model: String,
    pub suite_name: String,
    pub suite_version: String,
    pub pass_rate: f64,
    pub passed: u32,
    pub total: u32,
    pub task_results: HashMap<String, TaskStatus>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub notes: Option<String>,
}

/// Per-task comparison delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDelta {
    pub task_id: String,
    pub baseline: Option<TaskStatus>,
    pub current: TaskStatus,
    pub changed: bool,
    pub improved: bool,
    pub regressed: bool,
}

/// Alert severity for regressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// Regression alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAlert {
    pub severity: AlertSeverity,
    pub message: String,
    pub affected_tasks: Vec<String>,
}

/// Comparison verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Improved,
    Regressed,
    Unchanged,
    Mixed,
}

impl Verdict {
    pub fn emoji(&self) -> &'static str {
        match self {
            Verdict::Improved => "✅",
            Verdict::Regressed => "❌",
            Verdict::Unchanged => "➖",
            Verdict::Mixed => "⚠️",
        }
    }
}

/// Full baseline comparison result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    pub compared_at: DateTime<Utc>,
    pub baseline_id: i64,
    pub baseline_timestamp: DateTime<Utc>,
    pub baseline_pass_rate: f64,
    pub current_pass_rate: f64,
    pub current_passed: u32,
    pub current_total: u32,
    pub pass_rate_delta: f64,
    pub pass_rate_delta_percent: f64,
    pub verdict: Verdict,
    pub task_deltas: Vec<TaskDelta>,
    pub improved_tasks: Vec<String>,
    pub regressed_tasks: Vec<String>,
    pub regression_alert: Option<RegressionAlert>,
}

// ============================================================================
// Benchmark Results (input format)
// ============================================================================

/// Individual task result from a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
}

/// Summary statistics for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub pass_rate: f64,
    pub passed: u32,
    pub total: u32,
}

/// Complete benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub model: String,
    pub suite_name: String,
    pub suite_version: String,
    pub timestamp: DateTime<Utc>,
    pub summary: BenchmarkSummary,
    pub results: Vec<TaskResult>,
}

// ============================================================================
// Store
// ============================================================================

/// SQLite-backed baseline store.
pub struct BaselineStore {
    conn: Connection,
}

impl BaselineStore {
    /// Create or open a baseline store.
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory store (for testing).
    pub fn in_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> SqliteResult<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS baselines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL,
                suite_name TEXT NOT NULL,
                suite_version TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                pass_rate REAL NOT NULL,
                passed INTEGER NOT NULL,
                total INTEGER NOT NULL,
                task_results TEXT NOT NULL,
                git_commit TEXT,
                git_branch TEXT,
                notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_baselines_model ON baselines(model);
            CREATE INDEX IF NOT EXISTS idx_baselines_suite ON baselines(suite_name);
            CREATE INDEX IF NOT EXISTS idx_baselines_timestamp ON baselines(timestamp);
            "#,
        )
    }

    /// Save a new baseline.
    pub fn save(&self, input: &BaselineInput) -> SqliteResult<BaselineRecord> {
        let task_results_json = serde_json::to_string(&input.task_results)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let timestamp = Utc::now();

        self.conn.execute(
            r#"
            INSERT INTO baselines (model, suite_name, suite_version, timestamp, pass_rate, passed, total, task_results, git_commit, git_branch, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                input.model,
                input.suite_name,
                input.suite_version,
                timestamp.to_rfc3339(),
                input.pass_rate,
                input.passed,
                input.total,
                task_results_json,
                input.git_commit,
                input.git_branch,
                input.notes,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(BaselineRecord {
            id,
            model: input.model.clone(),
            suite_name: input.suite_name.clone(),
            suite_version: input.suite_version.clone(),
            timestamp,
            pass_rate: input.pass_rate,
            passed: input.passed,
            total: input.total,
            task_results: input.task_results.clone(),
            git_commit: input.git_commit.clone(),
            git_branch: input.git_branch.clone(),
            notes: input.notes.clone(),
        })
    }

    /// Get the most recent baseline for a model/suite.
    pub fn get_baseline(&self, model: &str, suite_name: Option<&str>) -> SqliteResult<Option<BaselineRecord>> {
        let query = match suite_name {
            Some(_) => {
                r#"
                SELECT id, model, suite_name, suite_version, timestamp, pass_rate, passed, total, task_results, git_commit, git_branch, notes
                FROM baselines
                WHERE model = ?1 AND suite_name = ?2
                ORDER BY timestamp DESC
                LIMIT 1
                "#
            }
            None => {
                r#"
                SELECT id, model, suite_name, suite_version, timestamp, pass_rate, passed, total, task_results, git_commit, git_branch, notes
                FROM baselines
                WHERE model = ?1
                ORDER BY timestamp DESC
                LIMIT 1
                "#
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let result = if let Some(suite) = suite_name {
            stmt.query_row(params![model, suite], row_to_baseline)
        } else {
            stmt.query_row(params![model], row_to_baseline)
        };

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get baseline history for trend analysis.
    pub fn get_history(
        &self,
        model: Option<&str>,
        suite_name: Option<&str>,
        limit: Option<u32>,
    ) -> SqliteResult<Vec<BaselineRecord>> {
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(m) = model {
            conditions.push(format!("model = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(m.to_string()));
        }
        if let Some(s) = suite_name {
            conditions.push(format!("suite_name = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(s.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();

        let query = format!(
            r#"
            SELECT id, model, suite_name, suite_version, timestamp, pass_rate, passed, total, task_results, git_commit, git_branch, notes
            FROM baselines
            {}
            ORDER BY timestamp ASC
            {}
            "#,
            where_clause, limit_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_refs.as_slice(), row_to_baseline)?;
        rows.collect()
    }

    /// Clear all baselines.
    pub fn clear(&self) -> SqliteResult<()> {
        self.conn.execute("DELETE FROM baselines", [])?;
        Ok(())
    }
}

fn row_to_baseline(row: &rusqlite::Row) -> SqliteResult<BaselineRecord> {
    let task_results_json: String = row.get(8)?;
    let task_results: HashMap<String, TaskStatus> = serde_json::from_str(&task_results_json)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e)))?;

    let timestamp_str: String = row.get(4)?;
    let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(BaselineRecord {
        id: row.get(0)?,
        model: row.get(1)?,
        suite_name: row.get(2)?,
        suite_version: row.get(3)?,
        timestamp,
        pass_rate: row.get(5)?,
        passed: row.get(6)?,
        total: row.get(7)?,
        task_results,
        git_commit: row.get(9)?,
        git_branch: row.get(10)?,
        notes: row.get(11)?,
    })
}

// ============================================================================
// Comparison Functions
// ============================================================================

/// Compare benchmark results against a baseline.
pub fn compare_with_baseline(results: &BenchmarkResults, baseline: &BaselineRecord) -> BaselineComparison {
    let current_pass_rate = results.summary.pass_rate;
    let baseline_pass_rate = baseline.pass_rate;

    let pass_rate_delta = current_pass_rate - baseline_pass_rate;
    let pass_rate_delta_percent = if baseline_pass_rate > 0.0 {
        (pass_rate_delta / baseline_pass_rate) * 100.0
    } else if current_pass_rate > 0.0 {
        100.0
    } else {
        0.0
    };

    // Build task-level comparison
    let mut task_deltas = Vec::new();
    let mut improved_tasks = Vec::new();
    let mut regressed_tasks = Vec::new();

    for result in &results.results {
        let baseline_status = baseline.task_results.get(&result.task_id).copied();
        let current_status = result.status;

        let changed = baseline_status.map(|b| b != current_status).unwrap_or(false);
        let was_pass = baseline_status == Some(TaskStatus::Pass);
        let is_pass = current_status == TaskStatus::Pass;

        let improved = changed && !was_pass && is_pass;
        let regressed = changed && was_pass && !is_pass;

        if improved {
            improved_tasks.push(result.task_id.clone());
        }
        if regressed {
            regressed_tasks.push(result.task_id.clone());
        }

        task_deltas.push(TaskDelta {
            task_id: result.task_id.clone(),
            baseline: baseline_status,
            current: current_status,
            changed,
            improved,
            regressed,
        });
    }

    // Determine verdict
    let verdict = match (improved_tasks.is_empty(), regressed_tasks.is_empty()) {
        (false, true) => Verdict::Improved,
        (true, false) => Verdict::Regressed,
        (false, false) => Verdict::Mixed,
        (true, true) => Verdict::Unchanged,
    };

    // Generate regression alert if needed
    let regression_alert = if !regressed_tasks.is_empty() {
        let severity = if regressed_tasks.len() >= 3 || pass_rate_delta <= -0.1 {
            AlertSeverity::Critical
        } else {
            AlertSeverity::Warning
        };

        let message = if severity == AlertSeverity::Critical {
            format!(
                "Critical regression: {} tasks regressed ({:.1}% pass rate drop)",
                regressed_tasks.len(),
                pass_rate_delta * 100.0
            )
        } else {
            format!("Regression detected: {} task(s) regressed", regressed_tasks.len())
        };

        Some(RegressionAlert {
            severity,
            message,
            affected_tasks: regressed_tasks.clone(),
        })
    } else {
        None
    };

    BaselineComparison {
        compared_at: Utc::now(),
        baseline_id: baseline.id,
        baseline_timestamp: baseline.timestamp,
        baseline_pass_rate,
        current_pass_rate,
        current_passed: results.summary.passed,
        current_total: results.summary.total,
        pass_rate_delta,
        pass_rate_delta_percent,
        verdict,
        task_deltas,
        improved_tasks,
        regressed_tasks,
        regression_alert,
    }
}

/// Create a baseline from benchmark results.
pub fn create_baseline(results: &BenchmarkResults, git_commit: Option<&str>, git_branch: Option<&str>, notes: Option<&str>) -> BaselineInput {
    let task_results: HashMap<String, TaskStatus> = results
        .results
        .iter()
        .map(|r| (r.task_id.clone(), r.status))
        .collect();

    BaselineInput {
        model: results.model.clone(),
        suite_name: results.suite_name.clone(),
        suite_version: results.suite_version.clone(),
        pass_rate: results.summary.pass_rate,
        passed: results.summary.passed,
        total: results.summary.total,
        task_results,
        git_commit: git_commit.map(String::from),
        git_branch: git_branch.map(String::from),
        notes: notes.map(String::from),
    }
}

// ============================================================================
// Markdown Formatting
// ============================================================================

/// Format baseline comparison as markdown.
pub fn format_comparison_markdown(comparison: &BaselineComparison) -> String {
    let mut lines = Vec::new();

    // Header with verdict
    lines.push(format!(
        "# Baseline Comparison: {} {}",
        comparison.verdict.emoji(),
        format!("{:?}", comparison.verdict).to_uppercase()
    ));
    lines.push(String::new());
    lines.push(format!("Generated: {}", comparison.compared_at.to_rfc3339()));
    lines.push(String::new());

    // Regression alert
    if let Some(alert) = &comparison.regression_alert {
        let emoji = if alert.severity == AlertSeverity::Critical { "🚨" } else { "⚠️" };
        lines.push(format!("## {} {:?} Alert", emoji, alert.severity));
        lines.push(String::new());
        lines.push(alert.message.clone());
        lines.push(String::new());
        if !alert.affected_tasks.is_empty() {
            lines.push("**Affected tasks:**".to_string());
            for task in &alert.affected_tasks {
                lines.push(format!("- {}", task));
            }
            lines.push(String::new());
        }
    }

    // Summary
    lines.push("## Summary".to_string());
    lines.push(String::new());
    lines.push("| Metric | Baseline | Current | Delta |".to_string());
    lines.push("|--------|----------|---------|-------|".to_string());
    lines.push(format!(
        "| Pass Rate | {:.1}% | {:.1}% | {}{:.1}% |",
        comparison.baseline_pass_rate * 100.0,
        comparison.current_pass_rate * 100.0,
        if comparison.pass_rate_delta >= 0.0 { "+" } else { "" },
        comparison.pass_rate_delta * 100.0
    ));
    lines.push(format!(
        "| Tasks Passed | - | {}/{} | - |",
        comparison.current_passed, comparison.current_total
    ));
    lines.push(String::new());

    // Improved tasks
    if !comparison.improved_tasks.is_empty() {
        lines.push("## ✅ Improved Tasks".to_string());
        lines.push(String::new());
        for task in &comparison.improved_tasks {
            if let Some(delta) = comparison.task_deltas.iter().find(|d| &d.task_id == task) {
                lines.push(format!(
                    "- **{}**: {} → {}",
                    task,
                    delta.baseline.map(|s| s.as_str()).unwrap_or("N/A"),
                    delta.current.as_str()
                ));
            }
        }
        lines.push(String::new());
    }

    // Regressed tasks
    if !comparison.regressed_tasks.is_empty() {
        lines.push("## ❌ Regressed Tasks".to_string());
        lines.push(String::new());
        for task in &comparison.regressed_tasks {
            if let Some(delta) = comparison.task_deltas.iter().find(|d| &d.task_id == task) {
                lines.push(format!(
                    "- **{}**: {} → {}",
                    task,
                    delta.baseline.map(|s| s.as_str()).unwrap_or("N/A"),
                    delta.current.as_str()
                ));
            }
        }
        lines.push(String::new());
    }

    // All task changes table
    let changed_tasks: Vec<_> = comparison.task_deltas.iter().filter(|d| d.changed).collect();
    if !changed_tasks.is_empty() {
        lines.push("## All Task Changes".to_string());
        lines.push(String::new());
        lines.push("| Task | Baseline | Current | Status |".to_string());
        lines.push("|------|----------|---------|--------|".to_string());
        for delta in changed_tasks {
            let status = if delta.improved {
                "✅ Improved"
            } else if delta.regressed {
                "❌ Regressed"
            } else {
                "➖ Changed"
            };
            lines.push(format!(
                "| {} | {} | {} | {} |",
                delta.task_id,
                delta.baseline.map(|s| s.as_str()).unwrap_or("N/A"),
                delta.current.as_str(),
                status
            ));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Format pass rate trend as markdown.
pub fn format_trend_markdown(baselines: &[BaselineRecord]) -> String {
    if baselines.is_empty() {
        return "# Pass Rate Trend\n\nNo baseline data available.".to_string();
    }

    let mut lines = Vec::new();
    lines.push("# Pass Rate Trend".to_string());
    lines.push(String::new());
    lines.push(format!("Model: {}", baselines[0].model));
    lines.push(format!("Suite: {}", baselines[0].suite_name));
    lines.push(format!("Data points: {}", baselines.len()));
    lines.push(String::new());

    // Calculate trend
    let first = baselines[0].pass_rate;
    let last = baselines[baselines.len() - 1].pass_rate;
    let trend = last - first;
    let trend_emoji = if trend > 0.0 { "📈" } else if trend < 0.0 { "📉" } else { "➖" };

    lines.push(format!(
        "## Overall Trend: {} {}{:.1}%",
        trend_emoji,
        if trend >= 0.0 { "+" } else { "" },
        trend * 100.0
    ));
    lines.push(String::new());

    // Table
    lines.push("| Date | Pass Rate | Passed | Total | Change |".to_string());
    lines.push("|------|-----------|--------|-------|--------|".to_string());

    let mut prev_rate = 0.0;
    for (i, baseline) in baselines.iter().enumerate() {
        let change = if i == 0 {
            "-".to_string()
        } else {
            let delta = baseline.pass_rate - prev_rate;
            format!("{}{:.1}%", if delta >= 0.0 { "+" } else { "" }, delta * 100.0)
        };
        let date = baseline.timestamp.format("%Y-%m-%d").to_string();
        lines.push(format!(
            "| {} | {:.1}% | {} | {} | {} |",
            date,
            baseline.pass_rate * 100.0,
            baseline.passed,
            baseline.total,
            change
        ));
        prev_rate = baseline.pass_rate;
    }
    lines.push(String::new());

    lines.join("\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results() -> BenchmarkResults {
        BenchmarkResults {
            model: "fm".to_string(),
            suite_name: "regex-log".to_string(),
            suite_version: "1.0".to_string(),
            timestamp: Utc::now(),
            summary: BenchmarkSummary {
                pass_rate: 0.75,
                passed: 3,
                total: 4,
            },
            results: vec![
                TaskResult { task_id: "task-1".to_string(), status: TaskStatus::Pass },
                TaskResult { task_id: "task-2".to_string(), status: TaskStatus::Pass },
                TaskResult { task_id: "task-3".to_string(), status: TaskStatus::Pass },
                TaskResult { task_id: "task-4".to_string(), status: TaskStatus::Fail },
            ],
        }
    }

    #[test]
    fn test_create_baseline() {
        let results = sample_results();
        let input = create_baseline(&results, Some("abc123"), Some("main"), None);

        assert_eq!(input.model, "fm");
        assert_eq!(input.suite_name, "regex-log");
        assert_eq!(input.pass_rate, 0.75);
        assert_eq!(input.passed, 3);
        assert_eq!(input.total, 4);
        assert_eq!(input.task_results.len(), 4);
        assert_eq!(input.git_commit, Some("abc123".to_string()));
    }

    #[test]
    fn test_store_save_and_get() {
        let store = BaselineStore::in_memory().unwrap();
        let results = sample_results();
        let input = create_baseline(&results, None, None, None);

        let saved = store.save(&input).unwrap();
        assert!(saved.id > 0);

        let retrieved = store.get_baseline("fm", Some("regex-log")).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.model, "fm");
        assert_eq!(retrieved.pass_rate, 0.75);
    }

    #[test]
    fn test_comparison_improved() {
        let store = BaselineStore::in_memory().unwrap();

        // Create baseline with 50% pass rate
        let mut baseline_results = sample_results();
        baseline_results.summary.pass_rate = 0.5;
        baseline_results.summary.passed = 2;
        baseline_results.results[2].status = TaskStatus::Fail; // task-3 was failing
        let input = create_baseline(&baseline_results, None, None, None);
        let baseline = store.save(&input).unwrap();

        // Current results: 75% (task-3 now passes)
        let current = sample_results();
        let comparison = compare_with_baseline(&current, &baseline);

        assert_eq!(comparison.verdict, Verdict::Improved);
        assert!(comparison.improved_tasks.contains(&"task-3".to_string()));
        assert!(comparison.regressed_tasks.is_empty());
        assert!(comparison.pass_rate_delta > 0.0);
    }

    #[test]
    fn test_comparison_regressed() {
        let store = BaselineStore::in_memory().unwrap();

        // Create baseline with 100% pass rate
        let mut baseline_results = sample_results();
        baseline_results.summary.pass_rate = 1.0;
        baseline_results.summary.passed = 4;
        baseline_results.results[3].status = TaskStatus::Pass; // task-4 was passing
        let input = create_baseline(&baseline_results, None, None, None);
        let baseline = store.save(&input).unwrap();

        // Current results: 75% (task-4 now fails)
        let current = sample_results();
        let comparison = compare_with_baseline(&current, &baseline);

        assert_eq!(comparison.verdict, Verdict::Regressed);
        assert!(comparison.regressed_tasks.contains(&"task-4".to_string()));
        assert!(comparison.improved_tasks.is_empty());
        assert!(comparison.regression_alert.is_some());
    }

    #[test]
    fn test_format_comparison_markdown() {
        let store = BaselineStore::in_memory().unwrap();
        let results = sample_results();
        let input = create_baseline(&results, None, None, None);
        let baseline = store.save(&input).unwrap();

        let comparison = compare_with_baseline(&results, &baseline);
        let markdown = format_comparison_markdown(&comparison);

        assert!(markdown.contains("Baseline Comparison"));
        assert!(markdown.contains("Summary"));
        assert!(markdown.contains("Pass Rate"));
    }

    #[test]
    fn test_get_history() {
        let store = BaselineStore::in_memory().unwrap();

        // Save multiple baselines
        for i in 0..5 {
            let mut results = sample_results();
            results.summary.pass_rate = 0.5 + (i as f64 * 0.1);
            let input = create_baseline(&results, None, None, None);
            store.save(&input).unwrap();
        }

        let history = store.get_history(Some("fm"), None, Some(3)).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_format_trend_markdown() {
        let store = BaselineStore::in_memory().unwrap();

        for i in 0..3 {
            let mut results = sample_results();
            results.summary.pass_rate = 0.5 + (i as f64 * 0.1);
            results.summary.passed = 2 + i;
            let input = create_baseline(&results, None, None, None);
            store.save(&input).unwrap();
        }

        let history = store.get_history(Some("fm"), None, None).unwrap();
        let markdown = format_trend_markdown(&history);

        assert!(markdown.contains("Pass Rate Trend"));
        assert!(markdown.contains("Overall Trend"));
        assert!(markdown.contains("fm"));
    }
}
