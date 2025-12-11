//! Claude Code SDK-based HillClimber Runner
//!
//! This module provides an alternative HillClimber runner that uses the Claude Code SDK
//! (Claude Agent SDK) instead of the local Apple Foundation Model. It leverages Claude's
//! built-in tools (Read, Write, Edit, Bash) and optional skills for task solving.
//!
//! # Example
//!
//! ```rust,no_run
//! use hillclimber::{CCHillClimberRunner, CCRunnerOptions, HillClimberStore};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> hillclimber::Result<()> {
//!     let store = Arc::new(HillClimberStore::open(".openagents/openagents.db")?);
//!     let runner = CCHillClimberRunner::new(store);
//!
//!     let options = CCRunnerOptions::default()
//!         .model("claude-sonnet-4-5-20250929")
//!         .max_turns(30)
//!         .use_skills(true);
//!
//!     // Run a task...
//!     Ok(())
//! }
//! ```

use crate::error::{HillClimberError, Result};
use crate::orchestrator::HillClimberEmitter;
use crate::scoring::score_result;
use crate::store::HillClimberStore;
use crate::types::{
    HillClimberRun, HillClimberRunInput, TerminalBenchTask, generate_run_id,
};
use claude_agent_sdk::{
    query, QueryOptions, SdkMessage, SdkResultMessage, SettingSource,
};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// CC Runner Options
// ============================================================================

/// Options for the Claude Code SDK runner.
#[derive(Debug, Clone)]
pub struct CCRunnerOptions {
    /// Claude model to use (e.g., "claude-sonnet-4-5-20250929").
    pub model: String,
    /// Maximum conversation turns.
    pub max_turns: u32,
    /// Maximum budget in USD.
    pub max_budget_usd: Option<f64>,
    /// Whether to load skills from .claude/skills/.
    pub use_skills: bool,
    /// Whether to bypass permission prompts (dangerous, requires CLI config).
    pub skip_permissions: bool,
    /// Workspace directory for the task.
    pub workspace: PathBuf,
    /// Whether to generate tests before solving.
    pub generate_tests: bool,
    /// Verbose logging.
    pub verbose: bool,
    /// Additional system prompt hints (appended to default).
    pub hint: Option<String>,
}

impl Default for CCRunnerOptions {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5-20250929".to_string(),
            max_turns: 30,
            max_budget_usd: Some(5.0),
            use_skills: true,
            skip_permissions: false,
            workspace: PathBuf::from("."),
            generate_tests: true,
            verbose: false,
            hint: None,
        }
    }
}

impl CCRunnerOptions {
    /// Create new options with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set maximum turns.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set maximum budget in USD.
    pub fn max_budget_usd(mut self, budget: f64) -> Self {
        self.max_budget_usd = Some(budget);
        self
    }

    /// Set whether to use skills.
    pub fn use_skills(mut self, use_skills: bool) -> Self {
        self.use_skills = use_skills;
        self
    }

    /// Set whether to skip permission prompts.
    pub fn skip_permissions(mut self, skip: bool) -> Self {
        self.skip_permissions = skip;
        self
    }

    /// Set the workspace directory.
    pub fn workspace(mut self, workspace: impl Into<PathBuf>) -> Self {
        self.workspace = workspace.into();
        self
    }

    /// Set whether to generate tests first.
    pub fn generate_tests(mut self, generate: bool) -> Self {
        self.generate_tests = generate;
        self
    }

    /// Set verbose mode.
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set a hint to append to the system prompt.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

// ============================================================================
// CC Run Result
// ============================================================================

/// Result from a CC runner iteration.
#[derive(Debug, Clone)]
pub struct CCIterationResult {
    /// Turn number.
    pub turn: u32,
    /// Current progress (0.0-1.0).
    pub progress: f64,
    /// Tests passing.
    pub tests_passing: u32,
    /// Total tests.
    pub tests_total: u32,
    /// Current solution (if any).
    pub solution: Option<String>,
    /// Raw output from Claude.
    pub output: String,
}

// ============================================================================
// CC HillClimber Runner
// ============================================================================

/// Claude Code SDK-based HillClimber runner.
///
/// This runner uses the Claude Agent SDK to solve Terminal-Bench tasks.
/// Unlike the FM runner which manually orchestrates tools, the CC runner
/// lets Claude Code handle tool execution directly.
pub struct CCHillClimberRunner {
    store: Arc<HillClimberStore>,
}

impl CCHillClimberRunner {
    /// Create a new CC runner with a store.
    pub fn new(store: Arc<HillClimberStore>) -> Self {
        Self { store }
    }

    /// Build the task prompt for Claude.
    fn build_prompt(task: &TerminalBenchTask, options: &CCRunnerOptions) -> String {
        let mut prompt = String::new();

        // Task description
        prompt.push_str("# Task\n\n");
        prompt.push_str(&task.description);
        prompt.push_str("\n\n");

        // Verification method
        prompt.push_str("# Verification\n\n");
        if let Some(ref cmd) = task.verification.command {
            prompt.push_str(&format!("Run `{}` to verify your solution.\n", cmd));
        } else {
            prompt.push_str("Run `pytest -v` to verify your solution.\n");
        }
        prompt.push_str("Goal: All tests must pass.\n\n");

        // Process guidance (generic, no task-specific hints)
        prompt.push_str("# Process\n\n");
        prompt.push_str("1. Read the task requirements carefully\n");
        prompt.push_str("2. Examine any existing files in the workspace\n");
        prompt.push_str("3. Implement your solution\n");
        prompt.push_str("4. Run verification to check progress\n");
        prompt.push_str("5. Iterate until all tests pass\n\n");

        // Optional hint
        if let Some(ref hint) = options.hint {
            prompt.push_str("# Hint\n\n");
            prompt.push_str(hint);
            prompt.push_str("\n\n");
        }

        // Test generation (if enabled)
        if options.generate_tests {
            prompt.push_str("# Test Generation\n\n");
            prompt.push_str("Before implementing, consider generating your own tests ");
            prompt.push_str("to understand the requirements better. Use the testgen skill ");
            prompt.push_str("if available.\n\n");
        }

        prompt
    }

    /// Run a single task using Claude Code SDK.
    pub async fn run<E: HillClimberEmitter>(
        &self,
        task: &TerminalBenchTask,
        options: CCRunnerOptions,
        emitter: &E,
    ) -> Result<HillClimberRun> {
        let run_id = generate_run_id();
        let start_time = Instant::now();

        // Ensure we have a config for this task
        let config = self.store.ensure_default_config(&task.id)?;

        // Build prompt
        let prompt = Self::build_prompt(task, &options);

        // Build query options
        let mut query_options = QueryOptions::new()
            .model(&options.model)
            .cwd(&options.workspace)
            .max_turns(options.max_turns);

        if let Some(budget) = options.max_budget_usd {
            query_options = query_options.max_budget_usd(budget);
        }

        // Load skills if enabled
        if options.use_skills {
            query_options = query_options.setting_sources(vec![
                SettingSource::Project,
                SettingSource::User,
            ]);
        }

        // Skip permissions if requested (dangerous)
        if options.skip_permissions {
            query_options = query_options.dangerously_skip_permissions(true);
        }

        // Start the query
        let mut stream = match query(&prompt, query_options).await {
            Ok(s) => s,
            Err(e) => {
                emitter.on_error(&format!("Failed to start query: {}", e));
                return Err(HillClimberError::Configuration(format!(
                    "Claude Code SDK error: {}",
                    e
                )));
            }
        };

        // Track state
        let mut turn = 0u32;
        let mut best_progress = 0.0f64;
        let mut _output_buffer = String::new();
        let mut passed = false;
        let mut error_message: Option<String> = None;

        // Process stream
        emitter.on_turn_start(0, options.max_turns, "starting");

        while let Some(message) = stream.next().await {
            match message {
                Ok(sdk_msg) => {
                    match sdk_msg {
                        SdkMessage::Assistant(assistant_msg) => {
                            // Track turns based on assistant messages
                            turn += 1;
                            emitter.on_turn_start(turn, options.max_turns, "processing");

                            // Append to output buffer
                            if let Some(content) = assistant_msg.message.get("content") {
                                if let Some(text) = content.as_str() {
                                    _output_buffer.push_str(text);
                                    _output_buffer.push('\n');
                                }
                            }

                            if options.verbose {
                                tracing::debug!(turn, "Assistant message received");
                            }
                        }
                        SdkMessage::ToolProgress(progress_msg) => {
                            // Track tool execution progress
                            if options.verbose {
                                tracing::debug!(
                                    tool = %progress_msg.tool_name,
                                    elapsed_secs = progress_msg.elapsed_time_seconds,
                                    "Tool in progress"
                                );
                            }
                        }
                        SdkMessage::Result(result_msg) => {
                            // Query completed
                            match result_msg {
                                SdkResultMessage::Success(success) => {
                                    if options.verbose {
                                        tracing::info!(
                                            turns = success.num_turns,
                                            cost_usd = success.total_cost_usd,
                                            "Query completed successfully"
                                        );
                                    }
                                    turn = success.num_turns;
                                    // Check if result indicates success (no error)
                                    if !success.is_error {
                                        passed = true;
                                    }
                                }
                                SdkResultMessage::ErrorDuringExecution(err) => {
                                    error_message = Some("Execution error".to_string());
                                    turn = err.num_turns;
                                    emitter.on_error(error_message.as_ref().unwrap());
                                }
                                SdkResultMessage::ErrorMaxTurns(err) => {
                                    error_message = Some("Max turns exceeded".to_string());
                                    turn = err.num_turns;
                                    emitter.on_error(error_message.as_ref().unwrap());
                                }
                                SdkResultMessage::ErrorMaxBudget(err) => {
                                    error_message = Some("Max budget exceeded".to_string());
                                    turn = err.num_turns;
                                    emitter.on_error(error_message.as_ref().unwrap());
                                }
                                SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                                    error_message = Some("Max structured output retries".to_string());
                                    turn = err.num_turns;
                                    emitter.on_error(error_message.as_ref().unwrap());
                                }
                            }
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }

                    // Periodic heartbeat
                    let elapsed = start_time.elapsed().as_millis() as u64;
                    emitter.on_heartbeat(turn, options.max_turns, best_progress, best_progress, elapsed);
                }
                Err(e) => {
                    error_message = Some(format!("Stream error: {}", e));
                    emitter.on_error(error_message.as_ref().unwrap());
                    break;
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Emit completion
        emitter.on_run_complete(passed, best_progress);

        // Calculate score
        let score = score_result(passed, turn);

        // Build step summary
        let step_summary: Option<Vec<String>> = Some(vec![
            format!("backend: CC ({})", options.model),
            format!("turns: {}", turn),
            format!("passed: {}", passed),
        ]);

        // Create run record
        let run_input = HillClimberRunInput {
            run_id: run_id.clone(),
            task_id: task.id.clone(),
            config_id: config.id,
            passed,
            turns: turn,
            duration_ms,
            step_summary,
            error_message,
            meta_model: Some(options.model.clone()),
            proposed_change: None,
            change_accepted: false,
            score,
        };

        // Save run
        let run = self.store.save_run(&run_input)?;

        Ok(run)
    }

    /// Run multiple tasks in a loop.
    pub async fn run_loop<E: HillClimberEmitter>(
        &self,
        tasks: Vec<TerminalBenchTask>,
        max_runs: u32,
        options: CCRunnerOptions,
        emitter: &E,
    ) -> Result<Vec<HillClimberRun>> {
        let mut runs = Vec::new();
        let mut task_index = 0;

        for run_num in 0..max_runs {
            let task = &tasks[task_index % tasks.len()];

            if options.verbose {
                tracing::info!(
                    run = run_num + 1,
                    max_runs,
                    task_id = %task.id,
                    model = %options.model,
                    "Starting run"
                );
            }

            // Use task's source_path as workspace if available
            let mut run_options = options.clone();
            if let Some(ref path) = task.source_path {
                run_options.workspace = path.clone();
            }

            match self.run(task, run_options, emitter).await {
                Ok(run) => {
                    if options.verbose {
                        tracing::info!(
                            passed = run.passed,
                            score = run.score,
                            turns = run.turns,
                            "Run completed"
                        );
                    }
                    runs.push(run);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Run failed");
                }
            }

            task_index += 1;

            // Small delay between runs
            if run_num < max_runs - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        Ok(runs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cc_runner_options_default() {
        let options = CCRunnerOptions::default();
        assert_eq!(options.model, "claude-sonnet-4-5-20250929");
        assert_eq!(options.max_turns, 30);
        assert!(options.use_skills);
        assert!(!options.skip_permissions);
    }

    #[test]
    fn test_cc_runner_options_builder() {
        let options = CCRunnerOptions::new()
            .model("claude-opus-4-5-20251101")
            .max_turns(50)
            .max_budget_usd(10.0)
            .use_skills(false)
            .hint("Focus on edge cases");

        assert_eq!(options.model, "claude-opus-4-5-20251101");
        assert_eq!(options.max_turns, 50);
        assert_eq!(options.max_budget_usd, Some(10.0));
        assert!(!options.use_skills);
        assert_eq!(options.hint, Some("Focus on edge cases".to_string()));
    }

    #[test]
    fn test_build_prompt() {
        let task = TerminalBenchTask {
            id: "test-task".to_string(),
            description: "Write a hello world program".to_string(),
            source_path: None,
            verification: crate::types::VerificationConfig::default(),
        };

        let options = CCRunnerOptions::default();
        let prompt = CCHillClimberRunner::build_prompt(&task, &options);

        assert!(prompt.contains("Write a hello world program"));
        assert!(prompt.contains("pytest -v"));
        assert!(prompt.contains("All tests must pass"));
    }

    #[test]
    fn test_build_prompt_with_hint() {
        let task = TerminalBenchTask {
            id: "test-task".to_string(),
            description: "Solve this problem".to_string(),
            source_path: None,
            verification: crate::types::VerificationConfig::default(),
        };

        let options = CCRunnerOptions::default().hint("Try a recursive approach");
        let prompt = CCHillClimberRunner::build_prompt(&task, &options);

        assert!(prompt.contains("Hint"));
        assert!(prompt.contains("Try a recursive approach"));
    }
}
