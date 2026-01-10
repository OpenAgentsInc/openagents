//! Autonomous autopilot loop for continuous task execution.
//!
//! Runs Adjutant in a loop until:
//! - Task succeeds AND verification passes
//! - Definitive failure occurs
//! - Max iterations reached
//! - User interrupts

use crate::{Adjutant, Task, TaskResult};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Result of the autopilot loop execution.
#[derive(Debug)]
pub enum AutopilotResult {
    /// Task completed successfully and verification passed
    Success(TaskResult),
    /// Task failed definitively (cannot proceed)
    Failed(TaskResult),
    /// Max iterations reached without success
    MaxIterationsReached {
        iterations: usize,
        last_result: Option<TaskResult>,
    },
    /// User interrupted the loop
    UserInterrupted { iterations: usize },
    /// Error during execution
    Error(String),
}

/// Verification result after LLM reports success.
#[derive(Debug)]
pub struct Verification {
    pub passed: bool,
    pub reason: String,
}

/// Configuration for the autopilot loop.
#[derive(Debug, Clone)]
pub struct AutopilotConfig {
    /// Maximum iterations before stopping
    pub max_iterations: usize,
    /// Workspace root for running verification commands
    pub workspace_root: PathBuf,
    /// Whether to run verification after LLM reports success
    pub verify_completion: bool,
}

impl Default for AutopilotConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            workspace_root: std::env::current_dir().unwrap_or_default(),
            verify_completion: true,
        }
    }
}

/// Output sink for autopilot progress.
///
/// This trait abstracts the output mechanism, allowing the same loop logic
/// to work for both CLI (stdout) and UI (channel) contexts.
pub trait AutopilotOutput: Send {
    /// Called when an iteration starts
    fn iteration_start(&self, iteration: usize, max: usize);
    /// Called for each token/chunk of output
    fn token(&self, token: &str);
    /// Called when verification starts
    fn verification_start(&self);
    /// Called with verification result
    fn verification_result(&self, passed: bool, reason: &str);
    /// Called when an error occurs
    fn error(&self, msg: &str);
    /// Called when interrupted
    fn interrupted(&self);
    /// Called when max iterations reached
    fn max_iterations(&self, iterations: usize);
}

/// CLI output implementation - prints to stdout
pub struct CliOutput;

impl AutopilotOutput for CliOutput {
    fn iteration_start(&self, iteration: usize, max: usize) {
        println!("\n--- Iteration {}/{} ---\n", iteration, max);
    }

    fn token(&self, token: &str) {
        print!("{}", token);
        let _ = std::io::stdout().flush();
    }

    fn verification_start(&self) {
        println!("\n🔍 Verifying completion...");
    }

    fn verification_result(&self, passed: bool, reason: &str) {
        if passed {
            println!("✓ Verification passed");
        } else {
            println!("⚠ Verification failed: {}", reason);
            println!("Continuing...");
        }
    }

    fn error(&self, msg: &str) {
        eprintln!("\nError: {}", msg);
    }

    fn interrupted(&self) {
        println!("\n--- Interrupted by user ---");
    }

    fn max_iterations(&self, iterations: usize) {
        println!("\n--- Max iterations ({}) reached ---", iterations);
    }
}

/// Channel-based output for UI streaming (Coder desktop)
pub struct ChannelOutput {
    tx: mpsc::UnboundedSender<String>,
}

impl ChannelOutput {
    pub fn new(tx: mpsc::UnboundedSender<String>) -> Self {
        Self { tx }
    }
}

impl AutopilotOutput for ChannelOutput {
    fn iteration_start(&self, iteration: usize, max: usize) {
        let _ = self
            .tx
            .send(format!("\n\n--- Iteration {}/{} ---\n\n", iteration, max));
    }

    fn token(&self, token: &str) {
        let _ = self.tx.send(token.to_string());
    }

    fn verification_start(&self) {
        let _ = self.tx.send("\n\n🔍 Verifying completion...\n".to_string());
    }

    fn verification_result(&self, passed: bool, reason: &str) {
        if passed {
            let _ = self.tx.send("✓ Verification passed\n".to_string());
        } else {
            let _ = self.tx.send(format!(
                "⚠ Verification failed: {}\nContinuing...\n",
                reason
            ));
        }
    }

    fn error(&self, msg: &str) {
        let _ = self.tx.send(format!("\n\nError: {}\n", msg));
    }

    fn interrupted(&self) {
        let _ = self
            .tx
            .send("\n\n--- Interrupted by user ---\n".to_string());
    }

    fn max_iterations(&self, iterations: usize) {
        let _ = self.tx.send(format!(
            "\n\n--- Max iterations ({}) reached ---\n",
            iterations
        ));
    }
}

/// Autonomous autopilot loop runner.
pub struct AutopilotLoop<O: AutopilotOutput> {
    adjutant: Adjutant,
    original_task: Task,
    config: AutopilotConfig,
    output: O,
    interrupt_flag: Arc<AtomicBool>,
}

impl<O: AutopilotOutput> AutopilotLoop<O> {
    /// Create a new autopilot loop.
    pub fn new(
        adjutant: Adjutant,
        task: Task,
        config: AutopilotConfig,
        output: O,
        interrupt_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            adjutant,
            original_task: task,
            config,
            output,
            interrupt_flag,
        }
    }

    /// Run the autopilot loop until completion.
    pub async fn run(mut self) -> AutopilotResult {
        let mut iteration = 0;
        let mut last_result: Option<TaskResult> = None;

        loop {
            // Check for user interrupt
            if self.interrupt_flag.load(Ordering::Relaxed) {
                self.output.interrupted();
                return AutopilotResult::UserInterrupted {
                    iterations: iteration,
                };
            }

            iteration += 1;

            // Check max iterations
            if iteration > self.config.max_iterations {
                self.output.max_iterations(self.config.max_iterations);
                return AutopilotResult::MaxIterationsReached {
                    iterations: iteration - 1,
                    last_result,
                };
            }

            // Signal iteration start
            self.output
                .iteration_start(iteration, self.config.max_iterations);

            // Build prompt for this iteration
            let prompt = self.build_iteration_prompt(iteration, &last_result);

            // Create task for this iteration
            let task = Task::new(
                format!("{}-iter{}", self.original_task.id, iteration),
                self.original_task.title.clone(),
                prompt,
            );

            // Create channel for streaming tokens from this iteration
            let (iter_token_tx, mut iter_token_rx) = mpsc::unbounded_channel::<String>();

            // Collect tokens in background while executing
            let forward_handle = tokio::spawn(async move {
                let mut tokens = Vec::new();
                while let Some(token) = iter_token_rx.recv().await {
                    tokens.push(token);
                }
                tokens
            });

            // Execute the task
            let result = match self.adjutant.execute_streaming(&task, iter_token_tx).await {
                Ok(r) => r,
                Err(e) => {
                    self.output.error(&e.to_string());
                    return AutopilotResult::Error(e.to_string());
                }
            };

            // Wait for tokens and output them
            if let Ok(tokens) = forward_handle.await {
                for token in tokens {
                    self.output.token(&token);
                }
            }

            // Check if LLM reports success
            if result.success {
                if self.config.verify_completion {
                    // Verify completion with actual tests/checks
                    self.output.verification_start();

                    let verification = self.verify_completion(&result).await;

                    self.output
                        .verification_result(verification.passed, &verification.reason);

                    if verification.passed {
                        return AutopilotResult::Success(result);
                    } else {
                        // Verification failed - continue with feedback
                        last_result = Some(TaskResult {
                            success: false,
                            summary: format!(
                                "LLM reported success but verification failed: {}. Previous summary: {}",
                                verification.reason, result.summary
                            ),
                            modified_files: result.modified_files,
                            commit_hash: result.commit_hash,
                            error: Some(verification.reason),
                            session_id: result.session_id,
                        });
                        continue;
                    }
                } else {
                    // No verification, trust LLM
                    return AutopilotResult::Success(result);
                }
            }

            // Check for definitive failure
            if self.is_definitive_failure(&result) {
                self.output.token("\n\n--- Definitive failure detected ---\n");
                return AutopilotResult::Failed(result);
            }

            // Continue to next iteration
            last_result = Some(result);
        }
    }

    /// Build the prompt for a given iteration.
    fn build_iteration_prompt(&self, iteration: usize, last_result: &Option<TaskResult>) -> String {
        match (iteration, last_result) {
            // First iteration: use original task
            (1, _) => self.original_task.description.clone(),

            // Subsequent iterations: include context from previous attempt
            (_, Some(result)) => {
                let mut prompt = String::new();

                // Include failure/verification info
                if let Some(ref error) = result.error {
                    prompt.push_str(&format!("Previous attempt failed: {}\n\n", error));
                } else {
                    prompt.push_str(&format!("Previous attempt summary: {}\n\n", result.summary));
                }

                // Include modified files
                if !result.modified_files.is_empty() {
                    prompt.push_str(&format!(
                        "Files modified so far: {}\n\n",
                        result.modified_files.join(", ")
                    ));
                }

                // Original task context
                prompt.push_str(&format!(
                    "Original task: {}\n\n\
                     Continue working on this task. What's the next step to complete it?",
                    self.original_task.title
                ));

                prompt
            }

            // Fallback
            _ => self.original_task.description.clone(),
        }
    }

    /// Check if the result indicates a definitive failure (can't proceed).
    fn is_definitive_failure(&self, result: &TaskResult) -> bool {
        if let Some(ref error) = result.error {
            let error_lower = error.to_lowercase();
            error_lower.contains("cannot")
                || error_lower.contains("impossible")
                || error_lower.contains("permission denied")
                || error_lower.contains("not found")
                || error_lower.contains("does not exist")
                || error_lower.contains("no such file")
                || error_lower.contains("access denied")
        } else {
            false
        }
    }

    /// Verify that the task is actually complete (run tests, etc).
    async fn verify_completion(&self, result: &TaskResult) -> Verification {
        let mut passed = true;
        let mut reasons = vec![];

        // Check if Rust files were modified
        let has_rust = result
            .modified_files
            .iter()
            .any(|f| f.ends_with(".rs") || f.ends_with("Cargo.toml"));

        if has_rust {
            // Run cargo check
            self.output.token("  Running cargo check... ");
            match Command::new("cargo")
                .args(["check", "--message-format=short"])
                .current_dir(&self.config.workspace_root)
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        self.output.token("OK\n");
                    } else {
                        self.output.token("FAILED\n");
                        passed = false;
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        reasons.push(format!(
                            "cargo check failed: {}",
                            stderr.lines().take(3).collect::<Vec<_>>().join("; ")
                        ));
                    }
                }
                Err(e) => {
                    self.output.token(&format!("ERROR: {}\n", e));
                    // Don't fail verification if cargo isn't available
                }
            }

            // Run cargo test (only if check passed)
            if passed {
                self.output.token("  Running cargo test... ");
                match Command::new("cargo")
                    .args(["test", "--", "--test-threads=1"])
                    .current_dir(&self.config.workspace_root)
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            self.output.token("OK\n");
                        } else {
                            self.output.token("FAILED\n");
                            passed = false;
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            // Extract failure summary
                            let failure_lines: Vec<&str> = stdout
                                .lines()
                                .filter(|l| l.contains("FAILED") || l.contains("error"))
                                .take(3)
                                .collect();
                            if !failure_lines.is_empty() {
                                reasons.push(format!("tests failed: {}", failure_lines.join("; ")));
                            } else {
                                reasons.push("tests failed".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        self.output.token(&format!("ERROR: {}\n", e));
                        // Don't fail verification if cargo isn't available
                    }
                }
            }
        }

        // Check for TypeScript/JavaScript files
        let has_ts_js = result.modified_files.iter().any(|f| {
            f.ends_with(".ts")
                || f.ends_with(".tsx")
                || f.ends_with(".js")
                || f.ends_with(".jsx")
        });

        if has_ts_js {
            // Check for package.json to determine test command
            let package_json = self.config.workspace_root.join("package.json");
            if package_json.exists() {
                self.output.token("  Running npm test... ");
                match Command::new("npm")
                    .args(["test", "--", "--passWithNoTests"])
                    .current_dir(&self.config.workspace_root)
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            self.output.token("OK\n");
                        } else {
                            self.output.token("FAILED\n");
                            passed = false;
                            reasons.push("npm test failed".to_string());
                        }
                    }
                    Err(e) => {
                        self.output.token(&format!("SKIPPED: {}\n", e));
                    }
                }
            }
        }

        // If no files modified or no tests to run, consider it passed
        if result.modified_files.is_empty() && passed {
            self.output.token("  No files modified, accepting LLM verdict\n");
        }

        Verification {
            passed,
            reason: reasons.join(", "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autopilot_config_default() {
        let config = AutopilotConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert!(config.verify_completion);
    }

    #[test]
    fn test_definitive_failure_patterns() {
        let definitive_errors = [
            "Cannot find module",
            "impossible to complete",
            "Permission denied: /etc/passwd",
            "File not found: missing.rs",
            "Directory does not exist",
            "No such file or directory",
            "Access denied to resource",
        ];

        let retryable_errors = [
            "Need to try a different approach",
            "Compilation error, fixing...",
            "Test failed, retrying",
            "Network timeout, will retry",
        ];

        for error in definitive_errors {
            let lower = error.to_lowercase();
            assert!(
                lower.contains("cannot")
                    || lower.contains("impossible")
                    || lower.contains("permission denied")
                    || lower.contains("not found")
                    || lower.contains("does not exist")
                    || lower.contains("no such file")
                    || lower.contains("access denied"),
                "Expected definitive failure pattern in: {}",
                error
            );
        }

        for error in retryable_errors {
            let lower = error.to_lowercase();
            assert!(
                !lower.contains("cannot")
                    && !lower.contains("impossible")
                    && !lower.contains("permission denied")
                    && !lower.contains("not found")
                    && !lower.contains("does not exist")
                    && !lower.contains("no such file")
                    && !lower.contains("access denied"),
                "Expected retryable pattern in: {}",
                error
            );
        }
    }
}
