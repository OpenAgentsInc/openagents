//! HillClimber Evaluator Module
//!
//! Real-time progress scoring during task execution.
//! Runs verification, parses test output, returns structured feedback.
//!
//! Supports Docker-based verification via bollard and local pytest fallback.

use crate::error::{HillClimberError, Result};
use crate::types::{EvaluatorResult, FailureDetail, TerminalBenchTask};
use bollard::container::{Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions, WaitContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures::StreamExt;
use regex::Regex;
use std::path::Path;
use std::time::Instant;
use tokio::process::Command;

// ============================================================================
// Docker Image
// ============================================================================

const TB2_IMAGE: &str = "terminal-bench-2:latest";

// ============================================================================
// Pytest Output Parsing
// ============================================================================

/// Parse pytest output to extract test results.
pub fn parse_pytest_output(output: &str) -> ParseResult {
    let mut failures: Vec<FailureDetail> = Vec::new();

    // Parse pytest summary line - handles both orders:
    // "1 passed, 2 failed in 0.05s" OR "2 failed, 1 passed in 0.05s"
    let passed_re = Regex::new(r"(\d+)\s+passed").unwrap();
    let failed_re = Regex::new(r"(\d+)\s+failed").unwrap();
    // Also parse our own summary format: "Verification: FAILED (10/21 tests)"
    let summary_format_re = Regex::new(r"Verification:\s*\w+\s*\((\d+)/(\d+)\s*tests?\)").unwrap();

    let (mut passed, mut failed) = (0u32, 0u32);

    // First try our summary format (takes precedence since it's already parsed)
    if let Some(caps) = summary_format_re.captures(output) {
        passed = caps[1].parse().unwrap_or(0);
        let total: u32 = caps[2].parse().unwrap_or(0);
        failed = total.saturating_sub(passed);
    } else {
        // Parse pytest format: passed and failed independently (order doesn't matter)
        if let Some(caps) = passed_re.captures(output) {
            passed = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = failed_re.captures(output) {
            failed = caps[1].parse().unwrap_or(0);
        }
    }

    // Parse individual test failures from short summary
    // Pattern: FAILED tests/test_*.py::test_name
    let short_failure_re = Regex::new(r"FAILED\s+(\S+)::(\w+)").unwrap();

    // Pattern: tests/test_*.py::test_name FAILED - message (verbose output)
    let verbose_failure_re = Regex::new(r"(\S+)::(\w+)\s+FAILED\s*[-–]\s*(.+?)(?:\n|$)").unwrap();

    // First try to parse verbose output with messages
    for caps in verbose_failure_re.captures_iter(output) {
        failures.push(FailureDetail {
            test_name: caps[2].to_string(),
            line_number: None,
            expected: None,
            actual: None,
            message: caps[3].trim().to_string(),
        });
    }

    // If no failures found, try short summary format
    if failures.is_empty() {
        for caps in short_failure_re.captures_iter(output) {
            failures.push(FailureDetail {
                test_name: caps[2].to_string(),
                line_number: None,
                expected: None,
                actual: None,
                message: "Test failed".to_string(),
            });
        }
    }

    // Also parse assertion details from verbose output
    // Pattern: AssertionError: Expected [...], but got [...]
    let assert_re = Regex::new(r"Expected\s+(\[.+?\]),\s+but\s+got\s+(\[.+?\])").unwrap();

    for caps in assert_re.captures_iter(output) {
        if let Some(last) = failures.last_mut() {
            if last.expected.is_none() {
                last.expected = Some(caps[1].to_string());
                last.actual = Some(caps[2].to_string());
            }
        }
    }

    let total = passed + failed;

    ParseResult {
        total,
        passed,
        failed,
        failures,
    }
}

/// Parse generic test output (exit code based).
pub fn parse_generic_output(output: &str, exit_code: i32) -> ParseResult {
    if exit_code == 0 {
        return ParseResult {
            total: 1,
            passed: 1,
            failed: 0,
            failures: vec![],
        };
    }

    // Try to extract error information from output
    let mut failures: Vec<FailureDetail> = Vec::new();

    // Look for common error patterns
    let error_patterns = [
        r"error:\s*(.+)",
        r"failed:\s*(.+)",
        r"assertion.*?:\s*(.+)",
    ];

    for pattern in error_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for caps in re.captures_iter(output) {
                failures.push(FailureDetail {
                    test_name: "verification".to_string(),
                    line_number: None,
                    expected: None,
                    actual: None,
                    message: caps[0].to_string(),
                });
            }
        }
    }

    // If no failures parsed, create a generic one
    if failures.is_empty() {
        let truncated = if output.len() > 500 {
            format!("{}...", &output[..500])
        } else {
            output.to_string()
        };
        failures.push(FailureDetail {
            test_name: "verification".to_string(),
            line_number: None,
            expected: None,
            actual: None,
            message: truncated,
        });
    }

    ParseResult {
        total: 1,
        passed: 0,
        failed: 1,
        failures,
    }
}

/// Parsed test results.
pub struct ParseResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub failures: Vec<FailureDetail>,
}

// ============================================================================
// Local Pytest Runner
// ============================================================================

/// Run pytest locally (fallback when Docker unavailable).
pub async fn run_local_pytest(workspace: &Path) -> Result<EvaluatorResult> {
    let start = Instant::now();

    let tests_dir = workspace.join("tests");
    if !tests_dir.exists() {
        return Err(HillClimberError::Workspace(format!(
            "Tests directory not found: {}",
            tests_dir.display()
        )));
    }

    let output = Command::new("python3")
        .args(["-m", "pytest", "tests/", "-v", "--tb=short"])
        .current_dir(workspace)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .await
        .map_err(|e| HillClimberError::Workspace(format!("Failed to run pytest: {}", e)))?;

    let duration_ms = start.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_output = format!("{}\n{}", stdout, stderr);

    let parse_result = parse_pytest_output(&raw_output);
    let passed = output.status.success() && parse_result.failed == 0;
    let progress = if parse_result.total > 0 {
        parse_result.passed as f64 / parse_result.total as f64
    } else {
        0.0
    };

    Ok(EvaluatorResult {
        passed,
        progress,
        tests_total: parse_result.total,
        tests_passing: parse_result.passed,
        failures: parse_result.failures,
        suggestion: None,
        raw_output,
        duration_ms,
    })
}

// ============================================================================
// Docker Verification
// ============================================================================

/// Check if Docker is available and daemon is running.
pub async fn is_docker_available() -> bool {
    // Check environment override
    if std::env::var("OPENAGENTS_DOCKER_AVAILABLE").as_deref() == Ok("0") {
        return false;
    }
    if std::env::var("OPENAGENTS_DOCKER_AVAILABLE").as_deref() == Ok("1") {
        return true;
    }

    match Docker::connect_with_local_defaults() {
        Ok(docker) => docker.ping().await.is_ok(),
        Err(_) => false,
    }
}

/// Run verification using Docker.
pub async fn run_docker_verification(
    task: &TerminalBenchTask,
    workspace: &Path,
    timeout_secs: u64,
) -> Result<EvaluatorResult> {
    let start = Instant::now();

    let docker = Docker::connect_with_local_defaults()
        .map_err(|e| HillClimberError::Docker(e))?;

    // Ensure image exists (pull if needed)
    let _ = docker
        .create_image(
            Some(CreateImageOptions {
                from_image: TB2_IMAGE,
                ..Default::default()
            }),
            None,
            None,
        )
        .collect::<Vec<_>>()
        .await;

    // Create container name
    let container_name = format!("hc-{}-{}", task.id, uuid::Uuid::new_v4().to_string()[..8].to_string());

    // Get absolute workspace path
    let workspace_abs = workspace
        .canonicalize()
        .map_err(|e| HillClimberError::Workspace(format!("Invalid workspace path: {}", e)))?;

    // Create container config
    let config = Config {
        image: Some(TB2_IMAGE.to_string()),
        cmd: Some(vec![
            "pytest".to_string(),
            "tests/".to_string(),
            "-v".to_string(),
            "--tb=short".to_string(),
        ]),
        working_dir: Some("/app".to_string()),
        host_config: Some(bollard::models::HostConfig {
            binds: Some(vec![format!(
                "{}:/app:rw",
                workspace_abs.display()
            )]),
            auto_remove: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Create container
    let container = docker
        .create_container(
            Some(CreateContainerOptions {
                name: &container_name,
                platform: None,
            }),
            config,
        )
        .await
        .map_err(|e| HillClimberError::Docker(e))?;

    // Start container
    docker
        .start_container::<String>(&container.id, None)
        .await
        .map_err(|e| HillClimberError::Docker(e))?;

    // Wait for container with timeout
    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        docker.wait_container(&container.id, Some(WaitContainerOptions { condition: "not-running" })).next(),
    )
    .await;

    let exit_code = match wait_result {
        Ok(Some(Ok(result))) => result.status_code,
        Ok(Some(Err(e))) => {
            // Try to remove container on error
            let _ = docker.remove_container(&container.id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            return Err(HillClimberError::Docker(e));
        }
        Ok(None) => {
            let _ = docker.remove_container(&container.id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            return Err(HillClimberError::Timeout("Docker container exited unexpectedly".to_string()));
        }
        Err(_) => {
            // Timeout - kill container
            let _ = docker.remove_container(&container.id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            return Err(HillClimberError::Timeout(format!(
                "Docker verification timed out after {}s",
                timeout_secs
            )));
        }
    };

    // Collect logs
    let logs_options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        follow: false,
        ..Default::default()
    };

    let logs: Vec<_> = docker
        .logs(&container.id, Some(logs_options))
        .collect()
        .await;

    let raw_output: String = logs
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|log| match log {
            LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                String::from_utf8_lossy(&message).to_string()
            }
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("");

    let duration_ms = start.elapsed().as_millis() as u64;

    // Parse output
    let parse_result = if raw_output.contains("pytest") || raw_output.contains("PASSED") || raw_output.contains("FAILED") {
        parse_pytest_output(&raw_output)
    } else {
        parse_generic_output(&raw_output, exit_code as i32)
    };

    let passed = exit_code == 0;
    let progress = if parse_result.total > 0 {
        parse_result.passed as f64 / parse_result.total as f64
    } else if passed {
        1.0
    } else {
        0.0
    };

    Ok(EvaluatorResult {
        passed,
        progress,
        tests_total: parse_result.total,
        tests_passing: parse_result.passed,
        failures: parse_result.failures,
        suggestion: None,
        raw_output,
        duration_ms,
    })
}

// ============================================================================
// Main Evaluator Functions
// ============================================================================

/// Evaluate progress on a task by running verification.
///
/// Tries Docker first, falls back to local pytest if unavailable.
pub async fn evaluate_progress(
    task: &TerminalBenchTask,
    workspace: &Path,
) -> Result<EvaluatorResult> {
    // Check if Docker is available
    if is_docker_available().await {
        match run_docker_verification(task, workspace, 120).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::warn!("Docker verification failed: {}, falling back to local", e);
            }
        }
    }

    // Fall back to local pytest
    run_local_pytest(workspace).await
}

/// Quick evaluation that doesn't parse detailed failures.
/// Use for frequent checks during execution.
pub async fn quick_evaluate(
    task: &TerminalBenchTask,
    workspace: &Path,
) -> Result<QuickResult> {
    let cmd = task
        .verification
        .command
        .as_deref()
        .or(task.verification.script.as_deref())
        .unwrap_or("exit 1");

    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(workspace)
        .output()
        .await
        .map_err(|e| HillClimberError::Workspace(format!("Quick evaluate failed: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Quick parse for pass count
    let passed_re = Regex::new(r"(\d+)\s+passed").unwrap();
    let failed_re = Regex::new(r"(\d+)\s+failed").unwrap();

    let passed = passed_re
        .captures(&stdout)
        .and_then(|c| c[1].parse::<u32>().ok())
        .unwrap_or(0);
    let failed = failed_re
        .captures(&stdout)
        .and_then(|c| c[1].parse::<u32>().ok())
        .unwrap_or(0);
    let total = passed + failed;
    let total = if total == 0 { 1 } else { total };

    let progress = if output.status.success() {
        1.0
    } else {
        passed as f64 / total as f64
    };

    Ok(QuickResult {
        passed: output.status.success(),
        progress,
        message: if output.status.success() {
            "All tests passing".to_string()
        } else {
            format!("{}/{} tests passing", passed, total)
        },
    })
}

/// Result from quick evaluation.
pub struct QuickResult {
    pub passed: bool,
    pub progress: f64,
    pub message: String,
}

// ============================================================================
// Prompt Formatting
// ============================================================================

/// Format evaluation result for injection into FM prompt.
pub fn format_for_prompt(result: &EvaluatorResult) -> String {
    let mut lines = Vec::new();

    if result.passed {
        lines.push(format!(
            "Verification: PASSED ({}/{} tests)",
            result.tests_passing, result.tests_total
        ));
    } else {
        lines.push(format!(
            "Verification: FAILED ({}/{} tests)",
            result.tests_passing, result.tests_total
        ));

        // Add first 3 failures
        for failure in result.failures.iter().take(3) {
            if failure.expected.is_some() && failure.actual.is_some() {
                lines.push(format!(
                    "  - {}: expected {}, got {}",
                    failure.test_name,
                    failure.expected.as_ref().unwrap(),
                    failure.actual.as_ref().unwrap()
                ));
            } else {
                let msg_truncated = if failure.message.len() > 100 {
                    format!("{}...", &failure.message[..100])
                } else {
                    failure.message.clone()
                };
                lines.push(format!("  - {}: {}", failure.test_name, msg_truncated));
            }
        }

        if result.failures.len() > 3 {
            lines.push(format!(
                "  ... and {} more failures",
                result.failures.len() - 3
            ));
        }

        if let Some(ref suggestion) = result.suggestion {
            lines.push(format!("Suggestion: {}", suggestion));
        }
    }

    lines.join("\n")
}

// ============================================================================
// Suggestion Generator
// ============================================================================

/// Generate a suggestion based on failures.
/// Note: This is generic - no task-specific suggestions.
pub fn generate_suggestion(failures: &[FailureDetail]) -> Option<String> {
    if failures.is_empty() {
        return None;
    }

    let first_failure = &failures[0];

    // If we have expected/actual, analyze the difference
    if let (Some(expected), Some(actual)) = (&first_failure.expected, &first_failure.actual) {
        // Try to parse as arrays and compare
        if expected.starts_with('[') && actual.starts_with('[') {
            // Count items roughly
            let expected_count = expected.matches(',').count() + 1;
            let actual_count = actual.matches(',').count() + 1;

            if expected_count > actual_count {
                return Some(format!(
                    "Missing {} matches. Check if constraints are too restrictive.",
                    expected_count - actual_count
                ));
            } else if actual_count > expected_count {
                return Some(format!(
                    "{} false positives. Check boundary conditions.",
                    actual_count - expected_count
                ));
            }
        }
    }

    // Generic suggestion
    let msg_truncated = if first_failure.message.len() > 100 {
        format!("{}...", &first_failure.message[..100])
    } else {
        first_failure.message.clone()
    };
    Some(format!("Fix: {}", msg_truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pytest_output_mixed() {
        let output = r#"
============================= test session starts ==============================
collected 10 items

tests/test_solution.py::test_basic PASSED
tests/test_solution.py::test_edge_case FAILED - AssertionError: Expected ['a', 'b'], but got ['a']

=========================== short test summary info ============================
FAILED tests/test_solution.py::test_edge_case
========================= 1 passed, 1 failed in 0.05s ==========================
"#;

        let result = parse_pytest_output(output);
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.total, 2);
        assert_eq!(result.failures.len(), 1);
    }

    #[test]
    fn test_parse_pytest_output_all_passed() {
        let output = "========================= 5 passed in 0.10s ==========================";
        let result = parse_pytest_output(output);
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 0);
        assert_eq!(result.total, 5);
    }

    #[test]
    fn test_format_for_prompt() {
        let result = EvaluatorResult {
            passed: false,
            progress: 0.5,
            tests_total: 10,
            tests_passing: 5,
            failures: vec![FailureDetail {
                test_name: "test_basic".to_string(),
                line_number: None,
                expected: Some("['a', 'b']".to_string()),
                actual: Some("['a']".to_string()),
                message: "AssertionError".to_string(),
            }],
            suggestion: Some("Check edge cases".to_string()),
            raw_output: String::new(),
            duration_ms: 100,
        };

        let prompt = format_for_prompt(&result);
        assert!(prompt.contains("FAILED"));
        assert!(prompt.contains("5/10"));
        assert!(prompt.contains("test_basic"));
        assert!(prompt.contains("Suggestion:"));
    }
}
