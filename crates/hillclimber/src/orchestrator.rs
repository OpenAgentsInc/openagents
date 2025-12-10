//! HillClimber MAP Orchestrator Module
//!
//! Implements the Modular Agentic Planner (MAP) main loop.
//! Coordinates the FM actor, monitor, and evaluator to solve tasks.
//!
//! Part of the MAP (Modular Agentic Planner) architecture.

use crate::decomposer::{decompose_task, get_current_subtask, is_subtask_complete};
use crate::error::{HillClimberError, Result};
use crate::evaluator::parse_pytest_output;
use crate::monitor::{create_action_signature, monitor_action};
use crate::prompt::{build_fm_context, build_user_prompt, parse_fm_response, sanitize_for_fm, SYSTEM_PROMPT};
use crate::types::{
    ActionContext, ActionResult, EvaluatorResult, ExecutionState, FMAction, MAPOrchestratorOptions,
    MAPOrchestratorResult, StepDecision, SubtaskState, TerminalBenchTask, VerificationConfig,
};
use fm_bridge::FMClient as FMBridgeClient;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use testgen::{
    generator::{IterationConfig, NoopEmitter as TestGenNoopEmitter},
    EnvironmentInfo, TestGenContext, TestGenerator,
};

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
// ============================================================================

// ============================================================================
// Emitter Trait
// ============================================================================

/// Trait for emitting progress events during orchestration.
pub trait HillClimberEmitter: Send + Sync {
    /// Called at the start of each turn.
    fn on_turn_start(&self, turn: u32, max_turns: u32, subtask_name: &str);

    /// Called when verification completes.
    fn on_verify_complete(&self, passing: u32, total: u32, progress: f64);

    /// Called periodically with progress info.
    fn on_heartbeat(&self, turn: u32, max_turns: u32, progress: f64, best: f64, elapsed_ms: u64);

    /// Called when the run completes.
    fn on_run_complete(&self, passed: bool, progress: f64);

    /// Called on error.
    fn on_error(&self, error: &str);
}

/// No-op emitter for when progress tracking isn't needed.
pub struct NoopEmitter;

impl HillClimberEmitter for NoopEmitter {
    fn on_turn_start(&self, _turn: u32, _max_turns: u32, _subtask_name: &str) {}
    fn on_verify_complete(&self, _passing: u32, _total: u32, _progress: f64) {}
    fn on_heartbeat(&self, _turn: u32, _max_turns: u32, _progress: f64, _best: f64, _elapsed_ms: u64) {}
    fn on_run_complete(&self, _passed: bool, _progress: f64) {}
    fn on_error(&self, _error: &str) {}
}

// ============================================================================
// FM Client Trait
// ============================================================================

/// Trait for making FM (Foundation Model) calls.
///
/// This allows dependency injection for testing and different backends.
#[async_trait::async_trait]
pub trait FMClient: Send + Sync {
    /// Generate a response from the FM.
    async fn generate(&self, system: &str, user: &str) -> Result<String>;
}

// ============================================================================
// Tool Executor Trait
// ============================================================================

/// Trait for executing tool actions.
///
/// This allows dependency injection for testing and sandboxed execution.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a read_file action.
    async fn read_file(&self, path: &str) -> Result<ActionResult>;

    /// Execute a write_file action.
    async fn write_file(&self, path: &str, content: &str) -> Result<ActionResult>;

    /// Execute a run_command action.
    async fn run_command(&self, command: &str) -> Result<ActionResult>;

    /// Execute verify_progress (run tests).
    async fn verify_progress(&self, verification: &VerificationConfig) -> Result<EvaluatorResult>;
}

// ============================================================================
// Workspace Tool Executor
// ============================================================================

/// Default tool executor that operates on a local workspace.
pub struct WorkspaceExecutor {
    pub workspace: PathBuf,
    pub verification: VerificationConfig,
}

impl WorkspaceExecutor {
    /// Create a new workspace executor.
    pub fn new(workspace: PathBuf, verification: VerificationConfig) -> Self {
        Self {
            workspace,
            verification,
        }
    }

    /// Resolve a path relative to the workspace.
    fn resolve_path(&self, path: &str) -> PathBuf {
        if path.starts_with('/') {
            // Handle /app/ prefix common in Terminal-Bench
            if path.starts_with("/app/") {
                self.workspace.join(&path[5..])
            } else {
                PathBuf::from(path)
            }
        } else {
            self.workspace.join(path)
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for WorkspaceExecutor {
    async fn read_file(&self, path: &str) -> Result<ActionResult> {
        let full_path = self.resolve_path(path);
        match tokio::fs::read_to_string(&full_path).await {
            Ok(content) => Ok(ActionResult {
                success: true,
                output: content,
                modified_file: None,
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                output: format!("Error reading {}: {}", path, e),
                modified_file: None,
            }),
        }
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<ActionResult> {
        let full_path = self.resolve_path(path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return Ok(ActionResult {
                    success: false,
                    output: format!("Error creating directories: {}", e),
                    modified_file: None,
                });
            }
        }

        match tokio::fs::write(&full_path, content).await {
            Ok(_) => Ok(ActionResult {
                success: true,
                output: format!("Wrote {} bytes to {}", content.len(), path),
                modified_file: Some(path.to_string()),
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                output: format!("Error writing {}: {}", path, e),
                modified_file: None,
            }),
        }
    }

    async fn run_command(&self, command: &str) -> Result<ActionResult> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.workspace)
            .output()
            .await
            .map_err(|e| HillClimberError::Io(e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        Ok(ActionResult {
            success: output.status.success(),
            output: combined.trim().to_string(),
            modified_file: None,
        })
    }

    async fn verify_progress(&self, verification: &VerificationConfig) -> Result<EvaluatorResult> {
        // Run pytest locally
        let start = Instant::now();
        let cmd = verification
            .command
            .clone()
            .unwrap_or_else(|| "pytest -v".to_string());

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&self.workspace)
            .output()
            .await
            .map_err(|e| HillClimberError::Io(e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        let parse_result = parse_pytest_output(&combined);
        let progress = if parse_result.total > 0 {
            parse_result.passed as f64 / parse_result.total as f64
        } else {
            0.0
        };

        Ok(EvaluatorResult {
            passed: output.status.success() && parse_result.failed == 0,
            progress,
            tests_total: parse_result.total,
            tests_passing: parse_result.passed,
            failures: parse_result.failures,
            suggestion: None,
            raw_output: combined,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// ============================================================================
// MAP Orchestrator
// ============================================================================

/// MAP Orchestrator - coordinates the main execution loop.
pub struct MAPOrchestrator<F: FMClient, T: ToolExecutor, E: HillClimberEmitter> {
    fm_client: F,
    tool_executor: T,
    emitter: E,
    options: MAPOrchestratorOptions,
}

impl<F: FMClient, T: ToolExecutor, E: HillClimberEmitter> MAPOrchestrator<F, T, E> {
    /// Create a new MAP orchestrator.
    pub fn new(fm_client: F, tool_executor: T, emitter: E, options: MAPOrchestratorOptions) -> Self {
        Self {
            fm_client,
            tool_executor,
            emitter,
            options,
        }
    }

    /// Run the MAP loop for a task.
    ///
    /// # Arguments
    ///
    /// * `task` - The Terminal-Bench task to solve
    ///
    /// # Returns
    ///
    /// Result with orchestration outcome
    pub async fn run(&self, task: &TerminalBenchTask) -> Result<MAPOrchestratorResult> {
        let start_time = Instant::now();

        // Step 1: Decompose task into subtasks
        let decomposition = decompose_task(task);

        // Step 2: Initialize execution state
        let mut state = ExecutionState::new(&decomposition);

        // Step 3: Load initial file contents
        let mut file_contents: HashMap<String, String> = HashMap::new();
        for path in &decomposition.files_to_read {
            if let Ok(result) = self.tool_executor.read_file(path).await {
                if result.success {
                    file_contents.insert(path.clone(), result.output);
                }
            }
        }

        // Step 3.5: Generate comprehensive tests (if enabled)
        if self.options.generate_tests {
            if self.options.verbose {
                tracing::info!("Generating tests for task {}...", task.id);
            }

            // Create FM client for testgen (uses fm_bridge directly)
            let testgen_client = FMBridgeClient::new();

            // Use reduced config to fit within FM context window
            let testgen_config = IterationConfig {
                min_tests_per_category: 1,
                target_tests_per_category: 2,
                max_rounds_per_category: 2,
                max_total_rounds: 4,
                max_total_tokens: 15000, // Reduced from 100000 for small FM context
                max_total_time_ms: 120000,
                ..Default::default()
            };
            let generator = TestGenerator::with_config(testgen_client, testgen_config);

            // Sanitize task description to avoid FM safety filter
            let sanitized_description = sanitize_for_fm(&task.description);

            // Set up minimal environment
            let environment = EnvironmentInfo::default();

            // Generate tests
            match generator
                .generate_iteratively(
                    &sanitized_description,
                    &task.id,
                    &environment,
                    TestGenContext::Benchmark, // All 5 categories
                    &TestGenNoopEmitter,
                )
                .await
            {
                Ok(result) => {
                    if self.options.verbose {
                        tracing::info!("Generated {} tests", result.tests.len());
                    }

                    if !result.tests.is_empty() {
                        // Convert to pytest format
                        // Pass task.description (not sanitized) for file path extraction
                        let pytest_content = crate::testgen_writer::format_as_pytest(
                            &result.tests,
                            &task.id,
                            Some(&task.description),
                        );

                        // Write to workspace
                        let test_file = "test_generated.py";
                        match self.tool_executor.write_file(test_file, &pytest_content).await {
                            Ok(_) => {
                                if self.options.verbose {
                                    tracing::info!(
                                        "Wrote {} tests to {}",
                                        result.tests.len(),
                                        test_file
                                    );
                                }
                            }
                            Err(e) => {
                                if self.options.verbose {
                                    tracing::warn!("Warning: Failed to write tests: {}", e);
                                }
                                // Continue anyway - tests are enhancement, not requirement
                            }
                        }
                    }
                }
                Err(e) => {
                    if self.options.verbose {
                        tracing::warn!("Warning: Test generation failed: {}", e);
                    }
                    // Continue anyway - tests are enhancement, not requirement
                }
            }
        }

        // Step 4: Main loop
        while state.total_turns < self.options.max_turns {
            state.total_turns += 1;
            state.subtask_turns += 1;

            // Get current subtask
            let completed_ids: Vec<u32> = state
                .subtask_status
                .iter()
                .filter(|s| s.status == SubtaskState::Completed)
                .map(|s| s.subtask_id)
                .collect();

            let current_subtask = match get_current_subtask(&decomposition, &completed_ids) {
                Some(s) => s,
                None => {
                    // All subtasks complete
                    break;
                }
            };

            self.emitter.on_turn_start(
                state.total_turns,
                self.options.max_turns,
                &current_subtask.name,
            );

            // Build context and prompt
            let context = build_fm_context(task, &decomposition, &state, file_contents.clone());
            let user_prompt = build_user_prompt(&context, state.total_turns, self.options.max_turns);

            // Get action from FM
            let response = self
                .fm_client
                .generate(SYSTEM_PROMPT, &user_prompt)
                .await?;

            let action = match parse_fm_response(&response) {
                Ok(a) => a,
                Err(e) => {
                    self.emitter.on_error(&format!("Failed to parse FM response: {}", e));
                    continue;
                }
            };

            // Monitor validates action
            let action_context = ActionContext {
                tool_name: action.tool_name.clone(),
                args: action.tool_args.clone(),
                workspace: self.options.workspace.clone(),
                task_id: task.id.clone(),
                modified_files: state.modified_files.clone(),
                turn_number: state.total_turns,
                previous_actions: state.previous_actions.clone(),
            };

            let decision = monitor_action(&action_context);

            if !decision.allowed {
                let reason = decision.reason.clone().unwrap_or_default();
                self.emitter.on_error(&format!("Action rejected: {}", reason));
                // Add rejection to previous actions so FM can adjust
                state.previous_actions.push(format!(
                    "REJECTED:{} - {}",
                    action.tool_name,
                    reason
                ));
                continue;
            }

            if let Some(warning) = &decision.warning {
                state.monitor_warning = Some(warning.clone());
            }

            // Execute action
            let result = self.execute_action(&action, &task.verification).await?;

            // Track action and result
            let signature = create_action_signature(&action.tool_name, &action.tool_args);
            state.previous_actions.push(signature);

            if let Some(modified_file) = &result.modified_file {
                if !state.modified_files.contains(modified_file) {
                    state.modified_files.push(modified_file.clone());
                }
                // Update file contents cache
                if let Ok(read_result) = self.tool_executor.read_file(modified_file).await {
                    if read_result.success {
                        file_contents.insert(modified_file.clone(), read_result.output);
                    }
                }
            }

            // Handle verification results
            if action.tool_name == "verify_progress" {
                if let Some(eval) = self.parse_verification_result(&result) {
                    let progress = eval.progress;
                    let passed = eval.passed;

                    self.emitter
                        .on_verify_complete(eval.tests_passing, eval.tests_total, progress);

                    // Update state
                    if progress > state.best_progress {
                        state.best_progress = progress;
                        state.turns_since_improvement = 0;
                    } else {
                        state.turns_since_improvement += 1;
                    }

                    state.last_evaluation = Some(eval);

                    // Check for completion
                    if passed {
                        state.output = "All tests pass!".to_string();
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        self.emitter.on_run_complete(true, 1.0);

                        return Ok(MAPOrchestratorResult {
                            passed: true,
                            turns: state.total_turns,
                            duration_ms,
                            progress: 1.0,
                            output: state.output,
                            error: None,
                            subtask_status: state.subtask_status,
                            evaluation: state.last_evaluation,
                        });
                    }

                    // Decide next step
                    let decision = self.decide_step(&state, current_subtask);
                    match decision {
                        StepDecision::Advance => {
                            // Move to next subtask
                            state.current_subtask += 1;
                            state.subtask_turns = 0;
                            if state.current_subtask < state.subtask_status.len() {
                                state.subtask_status[state.current_subtask - 1].status =
                                    SubtaskState::Completed;
                                state.subtask_status[state.current_subtask].status =
                                    SubtaskState::InProgress;
                            }
                        }
                        StepDecision::NoProgress => {
                            // Record no progress warning
                            state.monitor_warning =
                                Some("No progress for several turns".to_string());
                        }
                        _ => {}
                    }
                }
            }

            // Emit heartbeat
            self.emitter.on_heartbeat(
                state.total_turns,
                self.options.max_turns,
                state.last_evaluation.as_ref().map(|e| e.progress).unwrap_or(0.0),
                state.best_progress,
                start_time.elapsed().as_millis() as u64,
            );
        }

        // Loop ended - determine final result
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let final_progress = state.last_evaluation.as_ref().map(|e| e.progress).unwrap_or(0.0);
        let passed = state.last_evaluation.as_ref().map(|e| e.passed).unwrap_or(false);

        self.emitter.on_run_complete(passed, final_progress);

        Ok(MAPOrchestratorResult {
            passed,
            turns: state.total_turns,
            duration_ms,
            progress: final_progress,
            output: state.output,
            error: if !passed && state.total_turns >= self.options.max_turns {
                Some("Max turns reached".to_string())
            } else {
                None
            },
            subtask_status: state.subtask_status,
            evaluation: state.last_evaluation,
        })
    }

    /// Execute an action.
    async fn execute_action(
        &self,
        action: &FMAction,
        verification: &VerificationConfig,
    ) -> Result<ActionResult> {
        match action.tool_name.as_str() {
            "read_file" => {
                let path = action
                    .tool_args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.tool_executor.read_file(path).await
            }
            "write_file" => {
                let path = action
                    .tool_args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = action
                    .tool_args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.tool_executor.write_file(path, content).await
            }
            "run_command" => {
                let command = action
                    .tool_args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.tool_executor.run_command(command).await
            }
            "verify_progress" => {
                let eval = self.tool_executor.verify_progress(verification).await?;
                Ok(ActionResult {
                    success: eval.passed,
                    output: crate::evaluator::format_for_prompt(&eval),
                    modified_file: None,
                })
            }
            _ => Ok(ActionResult {
                success: false,
                output: format!("Unknown tool: {}", action.tool_name),
                modified_file: None,
            }),
        }
    }

    /// Parse verification result from action output.
    fn parse_verification_result(&self, result: &ActionResult) -> Option<EvaluatorResult> {
        // Parse the pytest output from the result
        let parse_result = parse_pytest_output(&result.output);
        let progress = if parse_result.total > 0 {
            parse_result.passed as f64 / parse_result.total as f64
        } else {
            0.0
        };

        Some(EvaluatorResult {
            passed: result.success && parse_result.failed == 0,
            progress,
            tests_total: parse_result.total,
            tests_passing: parse_result.passed,
            failures: parse_result.failures,
            suggestion: None,
            raw_output: result.output.clone(),
            duration_ms: 0,
        })
    }

    /// Decide what to do next based on current state.
    fn decide_step(&self, state: &ExecutionState, current_subtask: &crate::types::Subtask) -> StepDecision {
        // Check if all tests pass
        if let Some(eval) = &state.last_evaluation {
            if eval.passed {
                return StepDecision::Complete;
            }
        }

        // Check if subtask checkpoint is met
        let artifacts: Vec<String> = state.modified_files.clone();
        let progress = state.last_evaluation.as_ref().map(|e| e.progress).unwrap_or(0.0);

        if is_subtask_complete(current_subtask, progress, &artifacts) {
            return StepDecision::Advance;
        }

        // Check for no progress
        if state.turns_since_improvement >= 5 {
            return StepDecision::NoProgress;
        }

        // Check if subtask max turns exceeded
        if state.subtask_turns >= current_subtask.max_turns {
            return StepDecision::Advance;
        }

        StepDecision::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Mock FM Client for testing
    struct MockFMClient {
        responses: Vec<String>,
        call_count: AtomicU32,
    }

    impl MockFMClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses,
                call_count: AtomicU32::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl FMClient for MockFMClient {
        async fn generate(&self, _system: &str, _user: &str) -> Result<String> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst) as usize;
            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                Ok(self.responses.last().cloned().unwrap_or_default())
            }
        }
    }

    // Mock Tool Executor for testing
    struct MockToolExecutor {
        pass_on_verify: bool,
    }

    #[async_trait::async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn read_file(&self, path: &str) -> Result<ActionResult> {
            Ok(ActionResult {
                success: true,
                output: format!("Mock content of {}", path),
                modified_file: None,
            })
        }

        async fn write_file(&self, path: &str, content: &str) -> Result<ActionResult> {
            Ok(ActionResult {
                success: true,
                output: format!("Wrote {} bytes to {}", content.len(), path),
                modified_file: Some(path.to_string()),
            })
        }

        async fn run_command(&self, command: &str) -> Result<ActionResult> {
            Ok(ActionResult {
                success: true,
                output: format!("Executed: {}", command),
                modified_file: None,
            })
        }

        async fn verify_progress(&self, _verification: &VerificationConfig) -> Result<EvaluatorResult> {
            Ok(EvaluatorResult {
                passed: self.pass_on_verify,
                progress: if self.pass_on_verify { 1.0 } else { 0.5 },
                tests_total: 10,
                tests_passing: if self.pass_on_verify { 10 } else { 5 },
                failures: vec![],
                suggestion: None,
                raw_output: "Mock test output".to_string(),
                duration_ms: 100,
            })
        }
    }

    #[tokio::test]
    async fn test_orchestrator_quick_success() {
        let fm_client = MockFMClient::new(vec![
            r#"{"tool_name": "write_file", "tool_args": {"path": "/app/solution.txt", "content": "test"}}"#.to_string(),
            r#"{"tool_name": "verify_progress", "tool_args": {}}"#.to_string(),
        ]);

        let tool_executor = MockToolExecutor { pass_on_verify: true };
        let emitter = NoopEmitter;
        let options = MAPOrchestratorOptions {
            workspace: PathBuf::from("/tmp/test"),
            max_turns: 10,
            ..Default::default()
        };

        let orchestrator = MAPOrchestrator::new(fm_client, tool_executor, emitter, options);

        let task = TerminalBenchTask {
            id: "test".to_string(),
            description: "Write test to /app/solution.txt".to_string(),
            source_path: None,
            verification: VerificationConfig::default(),
        };

        let result = orchestrator.run(&task).await.unwrap();
        assert!(result.passed);
        assert!(result.turns <= 3);
    }

    #[tokio::test]
    async fn test_orchestrator_max_turns() {
        let fm_client = MockFMClient::new(vec![
            r#"{"tool_name": "write_file", "tool_args": {"path": "/app/test.txt", "content": "x"}}"#.to_string(),
        ]);

        let tool_executor = MockToolExecutor { pass_on_verify: false };
        let emitter = NoopEmitter;
        let options = MAPOrchestratorOptions {
            workspace: PathBuf::from("/tmp/test"),
            max_turns: 3,
            ..Default::default()
        };

        let orchestrator = MAPOrchestrator::new(fm_client, tool_executor, emitter, options);

        let task = TerminalBenchTask {
            id: "test".to_string(),
            description: "Test task".to_string(),
            source_path: None,
            verification: VerificationConfig::default(),
        };

        let result = orchestrator.run(&task).await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.turns, 3);
        assert!(result.error.is_some());
    }
}
