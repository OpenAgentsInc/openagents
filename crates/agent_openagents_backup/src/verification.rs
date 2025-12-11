//! Verification Pipeline
//!
//! Runs typecheck, tests, and e2e verification commands.
//! Supports both host execution and sandbox execution.

use crate::error::{AgentError, AgentResult};
use crate::types::OrchestratorEvent;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_E2E_TIMEOUT_MS: u64 = 300_000;

/// Skip E2E labels
const SKIP_E2E_LABELS: &[&str] = &["skip-e2e", "no-e2e", "unit-only"];
/// E2E required labels
const E2E_LABELS: &[&str] = &["e2e", "golden-loop", "integration"];

/// Result of running a single verification command
#[derive(Debug, Clone)]
pub struct VerificationCommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub error_message: Option<String>,
}

/// Result of running verification commands
#[derive(Debug, Clone, Default)]
pub struct VerificationRunResult {
    pub passed: bool,
    pub results: Vec<VerificationCommandResult>,
    pub outputs: Vec<String>,
}

/// E2E run result
#[derive(Debug, Clone, Default)]
pub struct E2eRunResult {
    pub ran: bool,
    pub passed: bool,
    pub outputs: Vec<String>,
    pub reason: Option<String>,
}

/// Full verification pipeline result
#[derive(Debug, Clone, Default)]
pub struct VerificationPipelineResult {
    pub verification: VerificationRunResult,
    pub e2e: E2eRunResult,
}

/// Verification pipeline configuration
#[derive(Debug, Clone, Default)]
pub struct VerificationPipelineConfig {
    /// Working directory
    pub cwd: String,
    /// Typecheck commands (e.g., "cargo check", "bun run typecheck")
    pub typecheck_commands: Vec<String>,
    /// Test commands (e.g., "cargo test", "bun test")
    pub test_commands: Vec<String>,
    /// E2E test commands
    pub e2e_commands: Vec<String>,
    /// Task labels for determining E2E execution
    pub task_labels: Vec<String>,
    /// Timeout for verification commands in milliseconds
    pub timeout_ms: Option<u64>,
    /// Timeout for E2E commands in milliseconds
    pub e2e_timeout_ms: Option<u64>,
}

impl VerificationPipelineConfig {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            ..Default::default()
        }
    }

    pub fn with_typecheck_commands(mut self, commands: Vec<String>) -> Self {
        self.typecheck_commands = commands;
        self
    }

    pub fn with_test_commands(mut self, commands: Vec<String>) -> Self {
        self.test_commands = commands;
        self
    }

    pub fn with_e2e_commands(mut self, commands: Vec<String>) -> Self {
        self.e2e_commands = commands;
        self
    }

    pub fn with_task_labels(mut self, labels: Vec<String>) -> Self {
        self.task_labels = labels;
        self
    }
}

/// Combine stdout and stderr for output summary
fn summarize_output(stdout: &str, stderr: &str) -> String {
    format!("{}{}", stdout, stderr).trim().to_string()
}

/// Run a single command on the host
pub fn run_command(
    command: &str,
    cwd: &str,
    timeout_ms: u64,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> VerificationCommandResult {
    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::VerificationStart {
            command: command.to_string(),
        });
    }

    let start = Instant::now();

    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let duration = start.elapsed();
    let duration_ms = duration.as_millis() as u64;
    let timed_out = duration_ms > timeout_ms;

    let (exit_code, stdout, stderr, error_message) = match result {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            (exit_code, stdout, stderr, None)
        }
        Err(e) => (1, String::new(), String::new(), Some(e.to_string())),
    };

    let passed = exit_code == 0 && !timed_out;

    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::VerificationComplete {
            command: command.to_string(),
            passed,
            output: summarize_output(&stdout, &stderr),
        });
    }

    VerificationCommandResult {
        command: command.to_string(),
        exit_code,
        stdout,
        stderr,
        duration_ms,
        timed_out,
        error_message,
    }
}

/// Run verification commands on the host
pub fn run_verification_on_host(
    commands: &[String],
    cwd: &str,
    timeout_ms: Option<u64>,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> VerificationRunResult {
    if commands.is_empty() {
        return VerificationRunResult {
            passed: true,
            results: vec![],
            outputs: vec![],
        };
    }

    let timeout = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    let mut results = Vec::new();
    let mut outputs = Vec::new();

    for command in commands {
        let result = run_command(command, cwd, timeout, emit.as_mut());
        outputs.push(summarize_output(&result.stdout, &result.stderr));
        results.push(result);
    }

    let passed = results.iter().all(|r| r.exit_code == 0 && !r.timed_out);

    VerificationRunResult {
        passed,
        results,
        outputs,
    }
}

/// Build verification commands from typecheck and test commands
pub fn build_verification_commands(
    typecheck_commands: &[String],
    test_commands: &[String],
) -> Vec<String> {
    let mut commands = Vec::new();
    commands.extend(typecheck_commands.iter().cloned());
    commands.extend(test_commands.iter().cloned());
    commands
}

/// Check if E2E should run based on task labels
pub fn should_run_e2e(task_labels: &[String], e2e_commands_configured: bool) -> bool {
    let normalized_labels: Vec<String> = task_labels.iter().map(|l| l.to_lowercase()).collect();

    let has_skip_label = normalized_labels
        .iter()
        .any(|label| SKIP_E2E_LABELS.contains(&label.as_str()));

    if e2e_commands_configured {
        return !has_skip_label;
    }

    normalized_labels
        .iter()
        .any(|label| E2E_LABELS.contains(&label.as_str()))
}

/// Build E2E commands list
pub fn build_e2e_commands(commands: &[String]) -> Vec<String> {
    commands
        .iter()
        .filter(|cmd| !cmd.trim().is_empty())
        .cloned()
        .collect()
}

/// Run E2E tests on host
pub fn run_e2e_on_host(
    commands: &[String],
    cwd: &str,
    timeout_ms: Option<u64>,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> E2eRunResult {
    if commands.is_empty() {
        return E2eRunResult {
            ran: false,
            passed: true,
            outputs: vec![],
            reason: Some("No e2e commands".to_string()),
        };
    }

    let timeout = timeout_ms.unwrap_or(DEFAULT_E2E_TIMEOUT_MS);
    let mut outputs = Vec::new();
    let mut all_passed = true;

    for command in commands {
        if let Some(ref mut emit_fn) = emit {
            emit_fn(OrchestratorEvent::E2eStart {
                command: command.clone(),
            });
        }

        let result = run_command(command, cwd, timeout, None::<fn(OrchestratorEvent)>);
        let output = summarize_output(&result.stdout, &result.stderr);
        let passed = result.exit_code == 0;

        if let Some(ref mut emit_fn) = emit {
            emit_fn(OrchestratorEvent::E2eComplete {
                command: command.clone(),
                passed,
                output: output.clone(),
            });
        }

        outputs.push(output);
        if !passed {
            all_passed = false;
        }
    }

    E2eRunResult {
        ran: true,
        passed: all_passed,
        outputs,
        reason: None,
    }
}

/// Run the full verification pipeline
pub fn run_verification_pipeline(
    config: &VerificationPipelineConfig,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> VerificationPipelineResult {
    // Build and run verification commands
    let verification_commands =
        build_verification_commands(&config.typecheck_commands, &config.test_commands);

    let verification = run_verification_on_host(
        &verification_commands,
        &config.cwd,
        config.timeout_ms,
        emit.as_mut(),
    );

    // Check if we should run E2E
    let e2e_commands = build_e2e_commands(&config.e2e_commands);
    let e2e_configured = !e2e_commands.is_empty();
    let should_run = should_run_e2e(&config.task_labels, e2e_configured);

    let e2e = if should_run && e2e_configured {
        run_e2e_on_host(&e2e_commands, &config.cwd, config.e2e_timeout_ms, emit.as_mut())
    } else {
        let reason = if !e2e_configured {
            "No e2eCommands configured".to_string()
        } else {
            "Task has skip-e2e label".to_string()
        };

        if let Some(ref mut emit_fn) = emit {
            emit_fn(OrchestratorEvent::E2eSkipped {
                reason: reason.clone(),
            });
        }

        E2eRunResult {
            ran: false,
            passed: true,
            outputs: vec![],
            reason: Some(reason),
        }
    };

    VerificationPipelineResult { verification, e2e }
}

/// Async verification runner trait
#[async_trait::async_trait]
pub trait AsyncVerifier: Send + Sync {
    /// Run verification asynchronously
    async fn verify(&self, config: &VerificationPipelineConfig) -> AgentResult<VerificationPipelineResult>;
}

/// Default host-based verifier
pub struct HostVerifier;

#[async_trait::async_trait]
impl AsyncVerifier for HostVerifier {
    async fn verify(&self, config: &VerificationPipelineConfig) -> AgentResult<VerificationPipelineResult> {
        Ok(run_verification_pipeline(config, None::<fn(OrchestratorEvent)>))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_output() {
        assert_eq!(summarize_output("hello", "world"), "helloworld");
        assert_eq!(summarize_output("  hello  ", ""), "hello");
        assert_eq!(summarize_output("", "  world  "), "world");
    }

    #[test]
    fn test_build_verification_commands() {
        let typecheck = vec!["cargo check".to_string()];
        let tests = vec!["cargo test".to_string()];
        let commands = build_verification_commands(&typecheck, &tests);
        assert_eq!(commands, vec!["cargo check", "cargo test"]);
    }

    #[test]
    fn test_should_run_e2e_no_commands() {
        // No E2E commands, no labels
        assert!(!should_run_e2e(&[], false));

        // No E2E commands, has E2E label
        let labels = vec!["e2e".to_string()];
        assert!(should_run_e2e(&labels, false));
    }

    #[test]
    fn test_should_run_e2e_with_commands() {
        // Has commands, no skip label
        assert!(should_run_e2e(&[], true));

        // Has commands, has skip label
        let labels = vec!["skip-e2e".to_string()];
        assert!(!should_run_e2e(&labels, true));
    }

    #[test]
    fn test_build_e2e_commands() {
        let commands = vec![
            "npm run e2e".to_string(),
            "".to_string(),
            "  ".to_string(),
            "playwright test".to_string(),
        ];
        let result = build_e2e_commands(&commands);
        assert_eq!(result, vec!["npm run e2e", "playwright test"]);
    }

    #[test]
    fn test_verification_pipeline_config() {
        let config = VerificationPipelineConfig::new("/home/user/project")
            .with_typecheck_commands(vec!["cargo check".to_string()])
            .with_test_commands(vec!["cargo test".to_string()])
            .with_e2e_commands(vec!["cargo test --features e2e".to_string()]);

        assert_eq!(config.cwd, "/home/user/project");
        assert_eq!(config.typecheck_commands, vec!["cargo check"]);
        assert_eq!(config.test_commands, vec!["cargo test"]);
    }

    #[test]
    fn test_empty_verification() {
        let result = run_verification_on_host(&[], ".", None, None::<fn(OrchestratorEvent)>);
        assert!(result.passed);
        assert!(result.results.is_empty());
    }
}
