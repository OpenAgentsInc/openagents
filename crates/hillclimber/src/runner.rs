//! HillClimber Runner Module
//!
//! High-level runner that integrates all components:
//! - TestGen for comprehensive test generation
//! - MAP Orchestrator for task execution
//! - Store for persistence
//! - Scoring for result evaluation
//!
//! This module provides the main entry point for running HillClimber.

use crate::error::{HillClimberError, Result};
use crate::orchestrator::{FMClient, MAPOrchestrator, NoopEmitter, WorkspaceExecutor};
use crate::scoring::score_result;
use crate::store::HillClimberStore;
use crate::types::{
    HillClimberRun, HillClimberRunInput, MAPOrchestratorOptions,
    TerminalBenchTask, VerificationConfig, generate_run_id,
};
use fm_bridge::FMClient as FMBridgeClient;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// FM Client Adapter
// ============================================================================

/// Adapter to use fm-bridge's FMClient with our FMClient trait.
pub struct FMBridgeAdapter {
    client: FMBridgeClient,
}

impl FMBridgeAdapter {
    /// Create a new FM bridge adapter.
    pub fn new() -> Self {
        Self {
            client: FMBridgeClient::new(),
        }
    }
}

impl Default for FMBridgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FMClient for FMBridgeAdapter {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        // Combine system and user prompts
        let full_prompt = format!("{}\n\n{}", system, user);
        let response = self.client
            .complete(full_prompt, None)
            .await
            .map_err(HillClimberError::FmBridge)?;

        // Extract content from first choice
        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(content)
    }
}

// ============================================================================
// Run Options
// ============================================================================

/// Options for a single HillClimber run.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Task to run
    pub task: TerminalBenchTask,
    /// Config to use (if None, uses current or default)
    pub config_id: Option<i64>,
    /// Workspace directory
    pub workspace: PathBuf,
    /// Maximum turns
    pub max_turns: u32,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Whether to generate tests first
    pub generate_tests: bool,
    /// Whether to use parallel sampling
    pub use_sampling: bool,
    /// Verbose output
    pub verbose: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            task: TerminalBenchTask {
                id: "unknown".to_string(),
                description: "".to_string(),
                source_path: None,
                verification: VerificationConfig::default(),
            },
            config_id: None,
            workspace: PathBuf::from("."),
            max_turns: 30,
            timeout_secs: 600,
            generate_tests: true,
            use_sampling: true,
            verbose: false,
        }
    }
}

// ============================================================================
// Runner
// ============================================================================

/// HillClimber Runner - orchestrates a single run.
pub struct HillClimberRunner {
    store: std::sync::Arc<HillClimberStore>,
}

impl HillClimberRunner {
    /// Create a new runner.
    pub fn new(store: std::sync::Arc<HillClimberStore>) -> Self {
        Self { store }
    }

    /// Create a new runner with a store (takes ownership).
    pub fn with_store(store: HillClimberStore) -> Self {
        Self {
            store: std::sync::Arc::new(store),
        }
    }

    /// Run a single task.
    pub async fn run(&self, options: RunOptions) -> Result<HillClimberRun> {
        let run_id = generate_run_id();

        // Ensure we have a config for this task
        let config = self.store.ensure_default_config(&options.task.id)?;

        // Set up orchestrator
        let orchestrator_options = MAPOrchestratorOptions {
            workspace: options.workspace.clone(),
            timeout_secs: options.timeout_secs,
            max_turns: options.max_turns,
            task_description: options.task.description.clone(),
            verbose: options.verbose,
            use_sampling: options.use_sampling,
        };

        let tool_executor = WorkspaceExecutor::new(
            options.workspace.clone(),
            options.task.verification.clone(),
        );

        let emitter = NoopEmitter;

        let orchestrator = MAPOrchestrator::new(
            FMBridgeAdapter::new(),
            tool_executor,
            emitter,
            orchestrator_options,
        );

        // Run the task
        let start_time = std::time::Instant::now();
        let result = orchestrator.run(&options.task).await?;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Calculate score
        let score = score_result(result.passed, result.turns);

        // Create run record
        let run_input = HillClimberRunInput {
            run_id: run_id.clone(),
            task_id: options.task.id.clone(),
            config_id: config.id,
            passed: result.passed,
            turns: result.turns,
            duration_ms,
            step_summary: Some(
                result
                    .subtask_status
                    .iter()
                    .map(|s| format!("{}: {:?}", s.name, s.status))
                    .collect(),
            ),
            error_message: result.error,
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score,
        };

        // Save run (this also updates best internally)
        let run = self.store.save_run(&run_input)?;

        Ok(run)
    }

    /// Run multiple tasks in round-robin.
    pub async fn run_loop(
        &self,
        tasks: Vec<TerminalBenchTask>,
        max_runs: u32,
        sleep_ms: u64,
        verbose: bool,
        max_turns: u32,
    ) -> Result<Vec<HillClimberRun>> {
        let mut runs = Vec::new();
        let mut task_index = 0;

        for run_num in 0..max_runs {
            let task = &tasks[task_index % tasks.len()];

            if verbose {
                println!("Run {}/{}: Task {}", run_num + 1, max_runs, task.id);
            }

            // Use task's source_path as workspace if available
            let workspace = task.source_path.clone().unwrap_or_else(|| PathBuf::from("."));

            let options = RunOptions {
                task: task.clone(),
                workspace,
                verbose,
                max_turns,
                ..Default::default()
            };

            match self.run(options).await {
                Ok(run) => {
                    if verbose {
                        println!(
                            "  Result: {} (score: {}, turns: {})",
                            if run.passed { "PASS" } else { "FAIL" },
                            run.score,
                            run.turns
                        );
                    }
                    runs.push(run);
                }
                Err(e) => {
                    if verbose {
                        println!("  Error: {}", e);
                    }
                }
            }

            task_index += 1;

            // Sleep between runs
            if sleep_ms > 0 && run_num < max_runs - 1 {
                sleep(Duration::from_millis(sleep_ms)).await;
            }
        }

        Ok(runs)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a task from a Terminal-Bench task file.
pub fn load_task(path: &PathBuf) -> Result<TerminalBenchTask> {
    let content = std::fs::read_to_string(path)
        .map_err(HillClimberError::Io)?;

    serde_json::from_str(&content)
        .map_err(HillClimberError::Serialization)
}

/// Create a task from ID and description.
pub fn create_task(id: &str, description: &str) -> TerminalBenchTask {
    TerminalBenchTask {
        id: id.to_string(),
        description: description.to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_task() {
        let task = create_task("test-task", "Write a hello world program");
        assert_eq!(task.id, "test-task");
        assert!(task.description.contains("hello world"));
    }

    #[test]
    fn test_run_options_default() {
        let options = RunOptions::default();
        assert_eq!(options.max_turns, 30);
        assert_eq!(options.timeout_secs, 600);
        assert!(options.generate_tests);
    }
}
