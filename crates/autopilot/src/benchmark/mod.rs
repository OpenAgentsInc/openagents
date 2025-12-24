//! Benchmark runner for autopilot task performance testing

mod tasks;

pub use tasks::*;

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
    /// Actions per minute (tool calls + messages)
    pub apm: f64,
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

/// Baseline metrics for a benchmark across multiple runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMetrics {
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Average duration in milliseconds
    pub avg_duration_ms: f64,
    /// Average input tokens
    pub avg_tokens_in: i64,
    /// Average output tokens
    pub avg_tokens_out: i64,
    /// Average cost in USD
    pub avg_cost_usd: f64,
    /// Number of runs used to compute baseline
    pub sample_count: i64,
    /// When baseline was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
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
                tool_errors INTEGER NOT NULL,
                apm REAL NOT NULL DEFAULT 0.0
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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS benchmark_baselines (
                benchmark_id TEXT NOT NULL,
                version TEXT NOT NULL,
                success_rate REAL NOT NULL,
                avg_duration_ms REAL NOT NULL,
                avg_tokens_in INTEGER NOT NULL,
                avg_tokens_out INTEGER NOT NULL,
                avg_cost_usd REAL NOT NULL,
                sample_count INTEGER NOT NULL,
                updated_at DATETIME NOT NULL,
                PRIMARY KEY (benchmark_id, version)
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
                cost_usd, tool_calls, tool_errors, apm
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                result.metrics.apm,
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
                    cost_usd, tool_calls, tool_errors, apm
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
                            apm: row.get(12)?,
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
                    cost_usd, tool_calls, tool_errors, apm
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
                            apm: row.get(12)?,
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

    /// Get the latest result for each benchmark ID
    pub fn get_all_latest_results(&self) -> Result<Vec<BenchmarkResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, benchmark_id, version, timestamp, success,
                    duration_ms, tokens_in, tokens_out, tokens_cached,
                    cost_usd, tool_calls, tool_errors, apm
             FROM benchmark_runs
             WHERE id IN (
                 SELECT MAX(id) FROM benchmark_runs GROUP BY benchmark_id
             )
             ORDER BY benchmark_id",
        )?;

        let results = stmt
            .query_map([], |row| {
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
                            apm: row.get(12)?,
                            custom_metrics: HashMap::new(),
                        },
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

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

    /// Compute and store baseline metrics for a version
    pub fn update_baseline(&mut self, version: &str) -> Result<()> {
        let results = self.get_baseline(version)?;

        // Group results by benchmark_id
        let mut by_benchmark: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
        for result in &results {
            by_benchmark
                .entry(result.benchmark_id.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }

        let tx = self.conn.transaction()?;

        for (benchmark_id, runs) in by_benchmark {
            if runs.is_empty() {
                continue;
            }

            let sample_count = runs.len() as i64;
            let successes = runs.iter().filter(|r| r.success).count();
            let success_rate = successes as f64 / sample_count as f64;

            let avg_duration_ms = runs.iter().map(|r| r.metrics.duration_ms).sum::<u64>() as f64
                / sample_count as f64;
            let avg_tokens_in = runs.iter().map(|r| r.metrics.tokens_in).sum::<u64>() as i64
                / sample_count;
            let avg_tokens_out = runs.iter().map(|r| r.metrics.tokens_out).sum::<u64>() as i64
                / sample_count;
            let avg_cost_usd =
                runs.iter().map(|r| r.metrics.cost_usd).sum::<f64>() / sample_count as f64;

            tx.execute(
                "INSERT OR REPLACE INTO benchmark_baselines (
                    benchmark_id, version, success_rate, avg_duration_ms,
                    avg_tokens_in, avg_tokens_out, avg_cost_usd,
                    sample_count, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    benchmark_id,
                    version,
                    success_rate,
                    avg_duration_ms,
                    avg_tokens_in,
                    avg_tokens_out,
                    avg_cost_usd,
                    sample_count,
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Get baseline metrics for a specific benchmark and version
    pub fn get_baseline_metrics(
        &self,
        benchmark_id: &str,
        version: &str,
    ) -> Result<Option<BaselineMetrics>> {
        let mut stmt = self.conn.prepare(
            "SELECT success_rate, avg_duration_ms, avg_tokens_in, avg_tokens_out,
                    avg_cost_usd, sample_count, updated_at
             FROM benchmark_baselines
             WHERE benchmark_id = ?1 AND version = ?2",
        )?;

        let result = stmt.query_row(params![benchmark_id, version], |row| {
            let updated_at_str: String = row.get(6)?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(BaselineMetrics {
                success_rate: row.get(0)?,
                avg_duration_ms: row.get(1)?,
                avg_tokens_in: row.get(2)?,
                avg_tokens_out: row.get(3)?,
                avg_cost_usd: row.get(4)?,
                sample_count: row.get(5)?,
                updated_at,
            })
        });

        match result {
            Ok(metrics) => Ok(Some(metrics)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
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
    pub async fn run_benchmark(&mut self, task: &dyn BenchmarkTask) -> Result<BenchmarkResult> {
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

        // Execute autopilot with the task prompt
        let _start = Instant::now();
        let metrics = self.execute_benchmark_task(task, &workspace).await?;

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

    /// Execute a benchmark task by running an actual autopilot agent.
    ///
    /// This method spawns a real Claude agent using the SDK and captures its execution
    /// trajectory to extract benchmark metrics. The agent runs with:
    /// - Working directory set to the isolated benchmark workspace
    /// - Sonnet 4.5 model
    /// - Permissions dangerously skipped (benchmarks are trusted tasks)
    ///
    /// # Process
    ///
    /// 1. **Git Context**: Extracts current commit SHA and branch from workspace
    /// 2. **Trajectory Setup**: Creates a `TrajectoryCollector` to capture all agent activity
    /// 3. **Agent Execution**: Streams messages from Claude SDK's `query()` function
    /// 4. **Metric Extraction**: Extracts real metrics from the completed trajectory:
    ///    - Token counts from `trajectory.usage` (input, output, cached)
    ///    - Tool call counts by filtering `StepType::ToolCall` events
    ///    - Tool error counts by filtering failed `StepType::ToolResult` events
    ///    - Duration from wall-clock measurement
    ///    - Cost calculated using Sonnet pricing ($3/MTok input, $15/MTok output)
    ///
    /// # Returns
    ///
    /// `BenchmarkMetrics` containing:
    /// - `duration_ms`: Total execution time in milliseconds
    /// - `tokens_in`: Input tokens consumed (from API)
    /// - `tokens_out`: Output tokens generated (from API)
    /// - `tokens_cached`: Cached tokens used (from API)
    /// - `cost_usd`: Estimated cost in USD
    /// - `tool_calls`: Number of tool invocations
    /// - `tool_errors`: Number of failed tool calls
    ///
    /// # Example
    ///
    /// ```ignore
    /// let task = B001SimpleFileEdit;
    /// let metrics = runner.execute_benchmark_task(&task, &workspace).await?;
    /// assert!(metrics.tool_calls > 0); // Agent used tools
    /// assert!(metrics.tokens_in > 0);  // Consumed input tokens
    /// ```
    ///
    /// # Note
    ///
    /// This method consumes real API tokens and can take 10-180 seconds depending on
    /// task complexity. For testing without API calls, use the unit test mocks.
    async fn execute_benchmark_task(
        &self,
        task: &dyn BenchmarkTask,
        workspace: &Path,
    ) -> Result<BenchmarkMetrics> {
        use claude_agent_sdk::{QueryOptions, query};
        use futures::StreamExt;
        use crate::TrajectoryCollector;

        let start = Instant::now();

        // Get repo info for trajectory
        let repo_sha = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(workspace)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "unknown".to_string())
            .trim()
            .to_string();

        let branch = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(workspace)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        // Create trajectory collector
        let mut collector = TrajectoryCollector::new(
            task.prompt().to_string(),
            "sonnet".to_string(),
            workspace.display().to_string(),
            repo_sha,
            branch,
        );

        // Configure autopilot options
        let options = QueryOptions {
            cwd: Some(workspace.to_path_buf()),
            model: Some("sonnet".to_string()),
            allow_dangerously_skip_permissions: true,
            continue_session: false,
            ..Default::default()
        };

        // Execute the task prompt
        let mut stream = query(task.prompt(), options).await?;

        while let Some(msg) = stream.next().await {
            let msg = msg?;
            collector.process_message(&msg);
        }

        // Finalize the trajectory
        let trajectory = collector.finish();

        // Extract metrics from trajectory
        let duration_ms = start.elapsed().as_millis() as u64;
        let tokens_in = trajectory.usage.input_tokens;
        let tokens_out = trajectory.usage.output_tokens;
        let tokens_cached = trajectory.usage.cache_read_tokens + trajectory.usage.cache_creation_tokens;
        let tool_calls = trajectory.steps.iter()
            .filter(|s| matches!(s.step_type, crate::trajectory::StepType::ToolCall { .. }))
            .count() as u64;
        let tool_errors = trajectory.steps.iter()
            .filter(|s| matches!(s.step_type, crate::trajectory::StepType::ToolResult { success: false, .. }))
            .count() as u64;

        // Use the cost from trajectory if available, otherwise estimate
        let cost_usd = if trajectory.usage.cost_usd > 0.0 {
            trajectory.usage.cost_usd
        } else {
            // Rough estimate: $3/MTok input, $15/MTok output for Sonnet
            (tokens_in as f64 * 3.0 / 1_000_000.0)
                + (tokens_out as f64 * 15.0 / 1_000_000.0)
        };

        // Calculate APM (Actions Per Minute)
        let duration_minutes = duration_ms as f64 / 60000.0;
        let apm = if duration_minutes > 0.0 {
            tool_calls as f64 / duration_minutes
        } else {
            0.0
        };

        Ok(BenchmarkMetrics {
            duration_ms,
            tokens_in,
            tokens_out,
            tokens_cached,
            cost_usd,
            tool_calls,
            tool_errors,
            apm,
            custom_metrics: HashMap::new(),
        })
    }

    /// Run all benchmarks in a category
    pub async fn run_category(&mut self, category: &str, tasks: &[&dyn BenchmarkTask]) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for task in tasks {
            if task.category() == category {
                let result = self.run_benchmark(*task).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Compare results against a baseline version
    pub fn compare_to_baseline(
        &self,
        results: &[BenchmarkResult],
        baseline_version: &str,
    ) -> Result<ComparisonReport> {
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut comparisons = Vec::new();

        for result in results {
            // Try to get baseline metrics first (aggregated)
            if let Some(base_metrics) = self
                .database
                .get_baseline_metrics(&result.benchmark_id, baseline_version)?
            {
                let current_success_rate = if result.success { 1.0 } else { 0.0 };

                // Compare success rate
                if current_success_rate < base_metrics.success_rate {
                    regressions.push(format!(
                        "{}: Success rate dropped from {:.1}% to {:.1}%",
                        result.benchmark_id,
                        base_metrics.success_rate * 100.0,
                        current_success_rate * 100.0
                    ));
                } else if current_success_rate > base_metrics.success_rate {
                    improvements.push(format!(
                        "{}: Success rate improved from {:.1}% to {:.1}%",
                        result.benchmark_id,
                        base_metrics.success_rate * 100.0,
                        current_success_rate * 100.0
                    ));
                }

                // Compare performance (>10% threshold)
                let duration_change = (result.metrics.duration_ms as f64 - base_metrics.avg_duration_ms)
                    / base_metrics.avg_duration_ms;

                if duration_change > 0.1 {
                    regressions.push(format!(
                        "{}: {:.1}% slower ({} ms vs {:.0} ms avg)",
                        result.benchmark_id,
                        duration_change * 100.0,
                        result.metrics.duration_ms,
                        base_metrics.avg_duration_ms
                    ));
                } else if duration_change < -0.1 {
                    improvements.push(format!(
                        "{}: {:.1}% faster ({} ms vs {:.0} ms avg)",
                        result.benchmark_id,
                        duration_change.abs() * 100.0,
                        result.metrics.duration_ms,
                        base_metrics.avg_duration_ms
                    ));
                }

                // Compare token usage
                let tokens_total = result.metrics.tokens_in + result.metrics.tokens_out;
                let base_tokens_total = base_metrics.avg_tokens_in + base_metrics.avg_tokens_out;
                let token_change = (tokens_total as f64 - base_tokens_total as f64) / base_tokens_total as f64;

                if token_change > 0.2 {
                    // 20% threshold for token usage
                    regressions.push(format!(
                        "{}: {:.1}% more tokens ({} vs {:.0} avg)",
                        result.benchmark_id,
                        token_change * 100.0,
                        tokens_total,
                        base_tokens_total
                    ));
                }

                comparisons.push(BenchmarkComparison {
                    benchmark_id: result.benchmark_id.clone(),
                    current: result.clone(),
                    baseline: base_metrics.clone(),
                    duration_change_pct: duration_change * 100.0,
                    token_change_pct: token_change * 100.0,
                    success_rate_change: current_success_rate - base_metrics.success_rate,
                });
            }
        }

        Ok(ComparisonReport {
            baseline_version: baseline_version.to_string(),
            current_version: self.version.clone(),
            regressions,
            improvements,
            comparisons,
        })
    }
}

/// Detailed comparison for a single benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub benchmark_id: String,
    pub current: BenchmarkResult,
    pub baseline: BaselineMetrics,
    pub duration_change_pct: f64,
    pub token_change_pct: f64,
    pub success_rate_change: f64,
}

/// Report comparing benchmark results to a baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub baseline_version: String,
    pub current_version: String,
    pub regressions: Vec<String>,
    pub improvements: Vec<String>,
    pub comparisons: Vec<BenchmarkComparison>,
}

impl ComparisonReport {
    /// Check if there are any regressions
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Print the report to stdout
    pub fn print(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║          Benchmark Comparison Report                      ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        println!("  Baseline: {}", self.baseline_version);
        println!("  Current:  {}", self.current_version);
        println!();

        if !self.regressions.is_empty() {
            println!("⚠️  REGRESSIONS ({}):", self.regressions.len());
            for regression in &self.regressions {
                println!("  ❌ {}", regression);
            }
            println!();
        }

        if !self.improvements.is_empty() {
            println!("✅ IMPROVEMENTS ({}):", self.improvements.len());
            for improvement in &self.improvements {
                println!("  ✓ {}", improvement);
            }
            println!();
        }

        if self.regressions.is_empty() && self.improvements.is_empty() {
            println!("  No significant changes detected.");
        }

        // Print detailed comparison table
        if !self.comparisons.is_empty() {
            println!("\n┌────────────────────────────────────────────────────────────┐");
            println!("│ Detailed Comparison                                        │");
            println!("├─────────────┬──────────┬───────────┬───────────┬───────────┤");
            println!("│ Benchmark   │ Success  │ Duration  │ Tokens    │ Cost      │");
            println!("│ ID          │ Rate     │ Change    │ Change    │ Change    │");
            println!("├─────────────┼──────────┼───────────┼───────────┼───────────┤");

            for comp in &self.comparisons {
                let success_icon = if comp.success_rate_change >= 0.0 {
                    "✓"
                } else {
                    "✗"
                };
                let duration_icon = if comp.duration_change_pct <= 0.0 {
                    "↓"
                } else {
                    "↑"
                };
                let token_icon = if comp.token_change_pct <= 0.0 { "↓" } else { "↑" };

                println!(
                    "│ {:<11} │ {} {:>5.1}% │ {} {:>6.1}% │ {} {:>6.1}% │ ${:>7.4} │",
                    comp.benchmark_id,
                    success_icon,
                    comp.success_rate_change * 100.0,
                    duration_icon,
                    comp.duration_change_pct.abs(),
                    token_icon,
                    comp.token_change_pct.abs(),
                    comp.current.metrics.cost_usd
                );
            }

            println!("└─────────────┴──────────┴───────────┴───────────┴───────────┘");
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
                apm: 10.0,
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

    #[tokio::test]
    #[ignore] // Requires claude CLI executable
    async fn test_benchmark_runner() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workspace = temp_dir.path().join("workspace");
        let db_path = temp_dir.path().join("benchmarks.db");

        let mut runner = BenchmarkRunner::new(
            workspace.clone(),
            db_path,
            "v0.1.0".to_string(),
        )?;

        let task = MockBenchmark;
        let _result = runner.run_benchmark(&task).await?;

        // Verify workspace was created and cleaned up
        assert!(workspace.join("B-TEST").exists());

        Ok(())
    }
}
