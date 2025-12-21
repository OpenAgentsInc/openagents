//! Benchmark runner for autopilot task performance testing
//!
//! This module provides infrastructure for running standard benchmark tasks
//! to measure autopilot performance across versions and detect regressions.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A benchmark task with setup, execution, and validation
pub trait BenchmarkTask: Send + Sync {
    /// Unique benchmark ID (e.g., "B-001")
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Task category (file-ops, git, testing, etc.)
    fn category(&self) -> &str;

    /// Set up the benchmark environment (create files, repos, etc.)
    fn setup(&self, workspace: &Path) -> Result<()>;

    /// Return the prompt to give to the agent
    fn prompt(&self) -> &str;

    /// Validate the result after execution
    fn validate(&self, workspace: &Path) -> Result<ValidationResult>;

    /// Clean up the benchmark environment
    fn teardown(&self, workspace: &Path) -> Result<()>;
}

/// Result of benchmark validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the task was completed correctly
    pub success: bool,
    /// Detailed validation messages
    pub messages: Vec<String>,
    /// Custom metrics specific to this benchmark
    pub custom_metrics: HashMap<String, f64>,
}

/// Metrics collected during benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    /// Total execution time in milliseconds
    pub duration_ms: u64,
    /// Input tokens used
    pub tokens_in: u64,
    /// Output tokens used
    pub tokens_out: u64,
    /// Cached tokens used
    pub tokens_cached: u64,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Number of tool calls made
    pub tool_calls: u64,
    /// Number of tool errors encountered
    pub tool_errors: u64,
    /// Custom metrics from validation
    pub custom_metrics: HashMap<String, f64>,
}

/// Result of a single benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Benchmark ID
    pub benchmark_id: String,
    /// Software version
    pub version: String,
    /// When the benchmark was run
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether the benchmark passed validation
    pub success: bool,
    /// Validation messages
    pub messages: Vec<String>,
    /// Metrics collected
    pub metrics: BenchmarkMetrics,
}

/// Database for storing benchmark results
pub struct BenchmarkDatabase {
    conn: Connection,
}

impl BenchmarkDatabase {
    /// Open or create the benchmark database
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .context("Failed to open benchmark database")?;

        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS benchmark_runs (
                id INTEGER PRIMARY KEY,
                benchmark_id TEXT NOT NULL,
                version TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                success BOOLEAN NOT NULL,
                duration_ms INTEGER NOT NULL,
                tokens_in INTEGER NOT NULL,
                tokens_out INTEGER NOT NULL,
                tokens_cached INTEGER NOT NULL,
                cost_usd REAL NOT NULL,
                tool_calls INTEGER NOT NULL,
                tool_errors INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS benchmark_details (
                run_id INTEGER NOT NULL,
                metric_name TEXT NOT NULL,
                metric_value REAL NOT NULL,
                PRIMARY KEY (run_id, metric_name),
                FOREIGN KEY (run_id) REFERENCES benchmark_runs(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS benchmark_messages (
                run_id INTEGER NOT NULL,
                message TEXT NOT NULL,
                FOREIGN KEY (run_id) REFERENCES benchmark_runs(id)
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Store a benchmark result
    pub fn store_result(&mut self, result: &BenchmarkResult) -> Result<i64> {
        let tx = self.conn.transaction()?;

        // Insert main run record
        tx.execute(
            "INSERT INTO benchmark_runs (
                benchmark_id, version, timestamp, success,
                duration_ms, tokens_in, tokens_out, tokens_cached,
                cost_usd, tool_calls, tool_errors
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                result.benchmark_id,
                result.version,
                result.timestamp.to_rfc3339(),
                result.success,
                result.metrics.duration_ms,
                result.metrics.tokens_in,
                result.metrics.tokens_out,
                result.metrics.tokens_cached,
                result.metrics.cost_usd,
                result.metrics.tool_calls,
                result.metrics.tool_errors,
            ],
        )?;

        let run_id = tx.last_insert_rowid();

        // Insert custom metrics
        for (name, value) in &result.metrics.custom_metrics {
            tx.execute(
                "INSERT INTO benchmark_details (run_id, metric_name, metric_value) VALUES (?1, ?2, ?3)",
                params![run_id, name, value],
            )?;
        }

        // Insert validation messages
        for message in &result.messages {
            tx.execute(
                "INSERT INTO benchmark_messages (run_id, message) VALUES (?1, ?2)",
                params![run_id, message],
            )?;
        }

        tx.commit()?;
        Ok(run_id)
    }

    /// Get all results for a specific benchmark
    pub fn get_results(&self, benchmark_id: &str) -> Result<Vec<BenchmarkResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, benchmark_id, version, timestamp, success,
                    duration_ms, tokens_in, tokens_out, tokens_cached,
                    cost_usd, tool_calls, tool_errors
             FROM benchmark_runs
             WHERE benchmark_id = ?1
             ORDER BY timestamp DESC",
        )?;

        let results = stmt
            .query_map([benchmark_id], |row| {
                let run_id: i64 = row.get(0)?;
                let timestamp_str: String = row.get(3)?;
                let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok((
                    run_id,
                    BenchmarkResult {
                        benchmark_id: row.get(1)?,
                        version: row.get(2)?,
                        timestamp,
                        success: row.get(4)?,
                        messages: Vec::new(), // Loaded separately
                        metrics: BenchmarkMetrics {
                            duration_ms: row.get(5)?,
                            tokens_in: row.get(6)?,
                            tokens_out: row.get(7)?,
                            tokens_cached: row.get(8)?,
                            cost_usd: row.get(9)?,
                            tool_calls: row.get(10)?,
                            tool_errors: row.get(11)?,
                            custom_metrics: HashMap::new(), // Loaded separately
                        },
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Load messages and custom metrics for each result
        let mut full_results = Vec::new();
        for (run_id, mut result) in results {
            // Load messages
            let mut msg_stmt = self
                .conn
                .prepare("SELECT message FROM benchmark_messages WHERE run_id = ?1")?;
            let messages: Vec<String> = msg_stmt
                .query_map([run_id], |row| row.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            result.messages = messages;

            // Load custom metrics
            let mut metric_stmt = self.conn.prepare(
                "SELECT metric_name, metric_value FROM benchmark_details WHERE run_id = ?1",
            )?;
            let metrics: HashMap<String, f64> = metric_stmt
                .query_map([run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            result.metrics.custom_metrics = metrics;

            full_results.push(result);
        }

        Ok(full_results)
    }

    /// Get baseline results for a specific version
    pub fn get_baseline(&self, version: &str) -> Result<Vec<BenchmarkResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, benchmark_id, version, timestamp, success,
                    duration_ms, tokens_in, tokens_out, tokens_cached,
                    cost_usd, tool_calls, tool_errors
             FROM benchmark_runs
             WHERE version = ?1
             ORDER BY benchmark_id, timestamp DESC",
        )?;

        let results = stmt
            .query_map([version], |row| {
                let run_id: i64 = row.get(0)?;
                let timestamp_str: String = row.get(3)?;
                let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok((
                    run_id,
                    BenchmarkResult {
                        benchmark_id: row.get(1)?,
                        version: row.get(2)?,
                        timestamp,
                        success: row.get(4)?,
                        messages: Vec::new(),
                        metrics: BenchmarkMetrics {
                            duration_ms: row.get(5)?,
                            tokens_in: row.get(6)?,
                            tokens_out: row.get(7)?,
                            tokens_cached: row.get(8)?,
                            cost_usd: row.get(9)?,
                            tool_calls: row.get(10)?,
                            tool_errors: row.get(11)?,
                            custom_metrics: HashMap::new(),
                        },
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Load messages and custom metrics
        let mut full_results = Vec::new();
        for (run_id, mut result) in results {
            let mut msg_stmt = self
                .conn
                .prepare("SELECT message FROM benchmark_messages WHERE run_id = ?1")?;
            let messages: Vec<String> = msg_stmt
                .query_map([run_id], |row| row.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            result.messages = messages;

            let mut metric_stmt = self.conn.prepare(
                "SELECT metric_name, metric_value FROM benchmark_details WHERE run_id = ?1",
            )?;
            let metrics: HashMap<String, f64> = metric_stmt
                .query_map([run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            result.metrics.custom_metrics = metrics;

            full_results.push(result);
        }

        Ok(full_results)
    }
}

/// Runner for executing benchmark tasks
pub struct BenchmarkRunner {
    workspace_root: PathBuf,
    database: BenchmarkDatabase,
    version: String,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new(workspace_root: PathBuf, db_path: PathBuf, version: String) -> Result<Self> {
        let database = BenchmarkDatabase::open(db_path)?;
        Ok(Self {
            workspace_root,
            database,
            version,
        })
    }

    /// Run a single benchmark task
    pub fn run_benchmark(&mut self, task: &dyn BenchmarkTask) -> Result<BenchmarkResult> {
        let benchmark_id = task.id().to_string();
        println!("Running benchmark {}: {}", benchmark_id, task.name());

        // Create isolated workspace
        let workspace = self.workspace_root.join(&benchmark_id);
        if workspace.exists() {
            std::fs::remove_dir_all(&workspace)?;
        }
        std::fs::create_dir_all(&workspace)?;

        // Setup
        task.setup(&workspace)
            .context("Benchmark setup failed")?;

        // TODO: Execute autopilot with the task prompt
        // For now, this is a placeholder - actual execution would use claude-agent-sdk
        let start = Instant::now();

        // Placeholder metrics - will be populated by actual agent execution
        let metrics = BenchmarkMetrics {
            duration_ms: start.elapsed().as_millis() as u64,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            cost_usd: 0.0,
            tool_calls: 0,
            tool_errors: 0,
            custom_metrics: HashMap::new(),
        };

        // Validate
        let validation = task
            .validate(&workspace)
            .context("Benchmark validation failed")?;

        // Teardown
        task.teardown(&workspace)
            .context("Benchmark teardown failed")?;

        // Create result
        let result = BenchmarkResult {
            benchmark_id,
            version: self.version.clone(),
            timestamp: Utc::now(),
            success: validation.success,
            messages: validation.messages,
            metrics: BenchmarkMetrics {
                custom_metrics: validation.custom_metrics,
                ..metrics
            },
        };

        // Store result
        self.database.store_result(&result)?;

        Ok(result)
    }

    /// Run all benchmarks in a category
    pub fn run_category(&mut self, _category: &str, _tasks: &[&dyn BenchmarkTask]) -> Result<Vec<BenchmarkResult>> {
        // TODO: Implement category running
        todo!("Category running not yet implemented")
    }

    /// Compare results against a baseline version
    pub fn compare_to_baseline(
        &self,
        results: &[BenchmarkResult],
        baseline_version: &str,
    ) -> Result<ComparisonReport> {
        let baseline = self.database.get_baseline(baseline_version)?;

        let mut regressions = Vec::new();
        let mut improvements = Vec::new();

        for result in results {
            if let Some(base) = baseline
                .iter()
                .find(|b| b.benchmark_id == result.benchmark_id)
            {
                // Check for success regression
                if base.success && !result.success {
                    regressions.push(format!(
                        "{}: Now failing (was passing)",
                        result.benchmark_id
                    ));
                }

                // Check for performance regression (>10% slower)
                let duration_increase = (result.metrics.duration_ms as f64
                    - base.metrics.duration_ms as f64)
                    / base.metrics.duration_ms as f64;
                if duration_increase > 0.1 {
                    regressions.push(format!(
                        "{}: {:.1}% slower ({} ms vs {} ms)",
                        result.benchmark_id,
                        duration_increase * 100.0,
                        result.metrics.duration_ms,
                        base.metrics.duration_ms
                    ));
                }

                // Check for improvements
                if !base.success && result.success {
                    improvements.push(format!(
                        "{}: Now passing (was failing)",
                        result.benchmark_id
                    ));
                }
                if duration_increase < -0.1 {
                    improvements.push(format!(
                        "{}: {:.1}% faster ({} ms vs {} ms)",
                        result.benchmark_id,
                        duration_increase.abs() * 100.0,
                        result.metrics.duration_ms,
                        base.metrics.duration_ms
                    ));
                }
            }
        }

        Ok(ComparisonReport {
            baseline_version: baseline_version.to_string(),
            current_version: self.version.clone(),
            regressions,
            improvements,
        })
    }
}

/// Report comparing benchmark results to a baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub baseline_version: String,
    pub current_version: String,
    pub regressions: Vec<String>,
    pub improvements: Vec<String>,
}

impl ComparisonReport {
    /// Check if there are any regressions
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Print the report to stdout
    pub fn print(&self) {
        println!("\n=== Benchmark Comparison Report ===");
        println!("Baseline: {}", self.baseline_version);
        println!("Current:  {}", self.current_version);
        println!();

        if !self.regressions.is_empty() {
            println!("⚠️  Regressions ({}):", self.regressions.len());
            for regression in &self.regressions {
                println!("  - {}", regression);
            }
            println!();
        }

        if !self.improvements.is_empty() {
            println!("✅ Improvements ({}):", self.improvements.len());
            for improvement in &self.improvements {
                println!("  + {}", improvement);
            }
            println!();
        }

        if self.regressions.is_empty() && self.improvements.is_empty() {
            println!("No significant changes detected.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct MockBenchmark;

    impl BenchmarkTask for MockBenchmark {
        fn id(&self) -> &str {
            "B-TEST"
        }

        fn name(&self) -> &str {
            "Test Benchmark"
        }

        fn category(&self) -> &str {
            "test"
        }

        fn setup(&self, workspace: &Path) -> Result<()> {
            std::fs::write(workspace.join("test.txt"), "initial")?;
            Ok(())
        }

        fn prompt(&self) -> &str {
            "Change test.txt content to 'modified'"
        }

        fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
            let content = std::fs::read_to_string(workspace.join("test.txt"))?;
            Ok(ValidationResult {
                success: content == "modified",
                messages: vec!["Validated file content".to_string()],
                custom_metrics: HashMap::new(),
            })
        }

        fn teardown(&self, _workspace: &Path) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_benchmark_database() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let mut db = BenchmarkDatabase::open(&db_path)?;

        let result = BenchmarkResult {
            benchmark_id: "B-001".to_string(),
            version: "v0.1.0".to_string(),
            timestamp: Utc::now(),
            success: true,
            messages: vec!["Test passed".to_string()],
            metrics: BenchmarkMetrics {
                duration_ms: 1000,
                tokens_in: 100,
                tokens_out: 50,
                tokens_cached: 20,
                cost_usd: 0.01,
                tool_calls: 5,
                tool_errors: 0,
                custom_metrics: [("custom_metric".to_string(), 42.0)]
                    .iter()
                    .cloned()
                    .collect(),
            },
        };

        let run_id = db.store_result(&result)?;
        assert!(run_id > 0);

        let results = db.get_results("B-001")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].benchmark_id, "B-001");
        assert_eq!(results[0].version, "v0.1.0");
        assert_eq!(results[0].success, true);
        assert_eq!(results[0].messages.len(), 1);
        assert_eq!(results[0].metrics.duration_ms, 1000);
        assert_eq!(results[0].metrics.custom_metrics.get("custom_metric"), Some(&42.0));

        Ok(())
    }

    #[test]
    fn test_benchmark_runner() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workspace = temp_dir.path().join("workspace");
        let db_path = temp_dir.path().join("benchmarks.db");

        let mut runner = BenchmarkRunner::new(
            workspace.clone(),
            db_path,
            "v0.1.0".to_string(),
        )?;

        let task = MockBenchmark;
        let _result = runner.run_benchmark(&task)?;

        // Verify workspace was created and cleaned up
        assert!(workspace.join("B-TEST").exists());

        Ok(())
    }
}
