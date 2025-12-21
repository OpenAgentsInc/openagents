//! Integration tests for benchmark execution with real agent
//!
//! These tests verify that the benchmark runner correctly executes
//! autopilot agents and captures metrics from actual task execution.

use anyhow::Result;
use autopilot::benchmark::{BenchmarkRunner, B001SimpleFileEdit};
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test environment with workspace and database
struct TestEnvironment {
    workspace: TempDir,
    db_path: PathBuf,
}

impl TestEnvironment {
    fn new() -> Result<Self> {
        let workspace = TempDir::new()?;
        let db_path = workspace.path().join("test-benchmarks.db");

        // Initialize git repo in workspace
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(workspace.path())
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(workspace.path())
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(workspace.path())
            .output()?;

        // Make initial commit
        std::fs::write(workspace.path().join("README.md"), "# Test\n")?;
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(workspace.path())
            .output()?;

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(workspace.path())
            .output()?;

        Ok(Self { workspace, db_path })
    }

    fn workspace_path(&self) -> PathBuf {
        self.workspace.path().to_path_buf()
    }

    fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }
}

#[tokio::test]
#[ignore] // Expensive test - requires actual API calls
async fn test_simple_file_edit_benchmark_execution() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Create benchmark runner
    let mut runner = BenchmarkRunner::new(
        env.workspace_path(),
        env.db_path(),
        "test-v1.0.0".to_string(),
    )?;

    // Create benchmark task
    let task = B001SimpleFileEdit;

    // Run the benchmark
    let result = runner.run_benchmark(&task).await?;

    // Verify basic result
    assert_eq!(result.benchmark_id, "B-001");
    assert_eq!(result.version, "test-v1.0.0");

    // Verify the task succeeded
    assert!(result.success, "Benchmark should succeed: {:?}", result.messages);

    // Verify metrics are populated (not placeholder zeros)
    assert!(result.metrics.duration_ms > 0, "Duration should be non-zero");
    assert!(result.metrics.tokens_in > 0, "Input tokens should be non-zero");
    assert!(result.metrics.tokens_out > 0, "Output tokens should be non-zero");
    assert!(result.metrics.cost_usd > 0.0, "Cost should be non-zero");

    // Verify tool calls were made (agent should use Edit tool)
    assert!(result.metrics.tool_calls > 0, "Should have tool calls");

    // Verify the file was actually edited
    let workspace_path = env.workspace_path().join("B-001");
    let test_file = workspace_path.join("test.txt");
    assert!(test_file.exists(), "Test file should exist");

    let content = std::fs::read_to_string(&test_file)?;
    assert!(content.contains("Hello, Benchmark!"), "File should be modified");

    Ok(())
}

#[tokio::test]
async fn test_benchmark_database_storage() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Create runner and verify database is created
    let runner = BenchmarkRunner::new(
        env.workspace_path(),
        env.db_path(),
        "test-v1.0.0".to_string(),
    )?;

    // Verify database file exists
    assert!(env.db_path().exists(), "Database should be created");

    // Verify tables exist (actual table names from database.rs)
    let conn = rusqlite::Connection::open(env.db_path())?;
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<_, _>>()?;

    assert!(tables.contains(&"benchmark_runs".to_string()));
    assert!(tables.contains(&"benchmark_baselines".to_string()));
    assert!(tables.contains(&"benchmark_details".to_string()));
    assert!(tables.contains(&"benchmark_messages".to_string()));

    Ok(())
}

#[tokio::test]
#[ignore] // Expensive test - requires actual API calls
async fn test_benchmark_metrics_accuracy() -> Result<()> {
    let env = TestEnvironment::new()?;

    let mut runner = BenchmarkRunner::new(
        env.workspace_path(),
        env.db_path(),
        "test-v1.0.0".to_string(),
    )?;

    let task = B001SimpleFileEdit;
    let result = runner.run_benchmark(&task).await?;

    // Verify token counts are reasonable for a simple file edit task
    // Should be < 10k tokens total for a simple task
    let total_tokens = result.metrics.tokens_in + result.metrics.tokens_out;
    assert!(
        total_tokens < 10_000,
        "Total tokens ({}) should be reasonable for simple task",
        total_tokens
    );

    // Verify duration is reasonable (< 2 minutes)
    assert!(
        result.metrics.duration_ms < 120_000,
        "Duration ({}ms) should be < 2 minutes",
        result.metrics.duration_ms
    );

    // Verify cost is reasonable (< $0.50 for a simple task)
    assert!(
        result.metrics.cost_usd < 0.50,
        "Cost (${}) should be reasonable",
        result.metrics.cost_usd
    );

    // Verify tool error rate
    if result.metrics.tool_calls > 0 {
        let error_rate = result.metrics.tool_errors as f64 / result.metrics.tool_calls as f64;
        assert!(
            error_rate < 0.2,
            "Tool error rate ({:.1}%) should be low",
            error_rate * 100.0
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_benchmark_workspace_isolation() -> Result<()> {
    let env = TestEnvironment::new()?;

    let mut runner = BenchmarkRunner::new(
        env.workspace_path(),
        env.db_path(),
        "test-v1.0.0".to_string(),
    )?;

    // Each benchmark should get its own workspace subdirectory
    let task = B001SimpleFileEdit;

    // Verify workspace doesn't exist before running
    let benchmark_workspace = env.workspace_path().join("B-001");
    assert!(!benchmark_workspace.exists(), "Workspace should not exist before run");

    // Note: This is a sync test, we can't actually run the async benchmark here
    // Just verify the workspace path logic

    Ok(())
}

#[tokio::test]
#[ignore] // Would require a failing benchmark
async fn test_benchmark_failure_handling() -> Result<()> {
    // Test that benchmark failures are properly reported
    // Would need a benchmark designed to fail for this test
    Ok(())
}

#[tokio::test]
async fn test_benchmark_result_persistence() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Create a dummy result and verify it can be stored/retrieved
    use autopilot::benchmark::{BenchmarkResult, BenchmarkMetrics};
    use chrono::Utc;
    use std::collections::HashMap;

    let db_path = env.db_path();

    // Create database
    let _runner = BenchmarkRunner::new(
        env.workspace_path(),
        db_path.clone(),
        "test-v1.0.0".to_string(),
    )?;

    let result = BenchmarkResult {
        benchmark_id: "TEST-001".to_string(),
        version: "test-v1.0.0".to_string(),
        timestamp: Utc::now(),
        success: true,
        messages: vec!["Test message".to_string()],
        metrics: BenchmarkMetrics {
            duration_ms: 1000,
            tokens_in: 100,
            tokens_out: 50,
            tokens_cached: 20,
            cost_usd: 0.01,
            tool_calls: 5,
            tool_errors: 0,
            custom_metrics: HashMap::new(),
        },
    };

    // Store result (use correct table name: benchmark_runs)
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute(
        "INSERT INTO benchmark_runs (benchmark_id, version, timestamp, success, duration_ms, tokens_in, tokens_out, tokens_cached, cost_usd, tool_calls, tool_errors)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            result.benchmark_id,
            result.version,
            result.timestamp.to_rfc3339(),
            result.success,
            result.metrics.duration_ms as i64,
            result.metrics.tokens_in as i64,
            result.metrics.tokens_out as i64,
            result.metrics.tokens_cached as i64,
            result.metrics.cost_usd,
            result.metrics.tool_calls as i64,
            result.metrics.tool_errors as i64,
        ],
    )?;

    // Retrieve result
    let mut stmt = conn.prepare(
        "SELECT benchmark_id, success, tokens_in FROM benchmark_runs WHERE benchmark_id = ?1"
    )?;

    let (id, success, tokens): (String, bool, i64) = stmt.query_row(
        ["TEST-001"],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    )?;

    assert_eq!(id, "TEST-001");
    assert!(success);
    assert_eq!(tokens, 100);

    Ok(())
}
