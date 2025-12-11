//! TestGen Validator - Validates Claude followed the TestGen protocol
//!
//! This module validates that Claude followed the DESCRIBE → WRITE TESTS → ITERATE
//! workflow by checking for the presence of testgen_tests.py and running the tests.

use sandbox::{ContainerBackend, ContainerConfig, DockerBackend};
use std::fs;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

/// Result of TestGen validation
#[derive(Debug, Clone)]
pub struct TestGenValidation {
    /// Whether the DESCRIBE section was found in output
    pub describe_found: bool,
    /// Whether testgen_tests.py was created
    pub tests_created: bool,
    /// Whether testgen tests passed
    pub tests_passed: bool,
    /// Number of testgen tests that passed
    pub tests_passed_count: u32,
    /// Total number of testgen tests
    pub tests_total_count: u32,
    /// Number of iterations Claude performed
    pub iteration_count: u32,
    /// TestGen test output
    pub test_output: String,
    /// Error message if validation failed
    pub error: Option<String>,
}

impl Default for TestGenValidation {
    fn default() -> Self {
        Self {
            describe_found: false,
            tests_created: false,
            tests_passed: false,
            tests_passed_count: 0,
            tests_total_count: 0,
            iteration_count: 0,
            test_output: String::new(),
            error: None,
        }
    }
}

impl TestGenValidation {
    /// Check if TestGen protocol was followed (describe + tests created)
    pub fn protocol_followed(&self) -> bool {
        self.describe_found && self.tests_created
    }

    /// Check if ready for TB2 verification (testgen tests passed)
    pub fn ready_for_verification(&self) -> bool {
        self.tests_created && self.tests_passed
    }
}

/// Errors from TestGen validation
#[derive(Debug, Error)]
pub enum TestGenValidatorError {
    #[error("Docker not available")]
    DockerNotAvailable,

    #[error("Failed to run testgen tests: {0}")]
    RunFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Timeout running testgen tests")]
    Timeout,
}

/// TestGen Validator
pub struct TestGenValidator {
    backend: DockerBackend,
}

impl TestGenValidator {
    /// Create a new TestGen validator
    pub fn new() -> Self {
        Self {
            backend: DockerBackend::new(),
        }
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        self.backend.is_available().await
    }

    /// Check if testgen_tests.py exists in workspace
    pub fn tests_exist(workspace_dir: &Path) -> bool {
        workspace_dir.join("testgen_tests.py").exists()
    }

    /// Parse Claude output to check if DESCRIBE section is present
    pub fn parse_describe_section(claude_output: &str) -> bool {
        // Look for the task analysis markers
        claude_output.contains("### TASK ANALYSIS")
            || claude_output.contains("**Goal**:")
            || claude_output.contains("### ACCEPTANCE CRITERIA")
    }

    /// Count iterations from Claude output
    pub fn count_iterations(claude_output: &str) -> u32 {
        // Count how many times Claude ran pytest on testgen_tests.py
        let pytest_runs = claude_output
            .matches("pytest /app/testgen_tests.py")
            .count();
        let pytest_alt = claude_output
            .matches("pytest testgen_tests.py")
            .count();
        (pytest_runs + pytest_alt) as u32
    }

    /// Run testgen tests in container and return results
    pub async fn run_testgen_tests(
        &self,
        docker_image: &str,
        workspace_dir: &Path,
        timeout_secs: u64,
    ) -> Result<TestGenValidation, TestGenValidatorError> {
        if !self.is_available().await {
            return Err(TestGenValidatorError::DockerNotAvailable);
        }

        let mut validation = TestGenValidation::default();

        // Check if tests file exists
        if !Self::tests_exist(workspace_dir) {
            validation.error = Some("testgen_tests.py not found".to_string());
            return Ok(validation);
        }
        validation.tests_created = true;

        tracing::info!(
            target: "mechacoder::testgen",
            "Running TestGen tests"
        );

        // Build container config
        let config = ContainerConfig::new(docker_image, workspace_dir)
            .workdir("/app")
            .timeout(Duration::from_secs(timeout_secs));

        // Run pytest on testgen_tests.py
        let command = vec![
            "bash".to_string(),
            "-c".to_string(),
            // Install pytest if needed, then run
            "apt-get update -qq && apt-get install -y -qq python3 python3-pip > /dev/null 2>&1; \
             pip3 install -q pytest > /dev/null 2>&1; \
             pytest /app/testgen_tests.py -v 2>&1"
                .to_string(),
        ];

        let run_result = self
            .backend
            .run(&command, &config)
            .await
            .map_err(|e| TestGenValidatorError::RunFailed(e.to_string()))?;

        validation.test_output = run_result.combined_output();

        // Parse pytest output for pass/fail counts
        let (passed, failed, total) = Self::parse_pytest_output(&validation.test_output);
        validation.tests_passed_count = passed;
        validation.tests_total_count = total;
        validation.tests_passed = failed == 0 && total > 0;

        tracing::info!(
            target: "mechacoder::testgen",
            passed = validation.tests_passed,
            tests_passed = validation.tests_passed_count,
            tests_total = validation.tests_total_count,
            "TestGen tests complete"
        );

        Ok(validation)
    }

    /// Parse pytest output to extract pass/fail counts
    fn parse_pytest_output(output: &str) -> (u32, u32, u32) {
        // Look for summary line like "5 passed, 2 failed in 0.12s"
        // or "3 passed in 0.05s"
        let mut passed = 0u32;
        let mut failed = 0u32;

        for line in output.lines() {
            let line = line.to_lowercase();

            // Parse "X passed"
            if let Some(idx) = line.find(" passed") {
                if let Some(start) = line[..idx].rfind(|c: char| !c.is_ascii_digit()) {
                    if let Ok(n) = line[start + 1..idx].trim().parse::<u32>() {
                        passed = n;
                    }
                } else if let Ok(n) = line[..idx].trim().parse::<u32>() {
                    passed = n;
                }
            }

            // Parse "X failed"
            if let Some(idx) = line.find(" failed") {
                if let Some(start) = line[..idx].rfind(|c: char| !c.is_ascii_digit()) {
                    if let Ok(n) = line[start + 1..idx].trim().parse::<u32>() {
                        failed = n;
                    }
                } else if let Ok(n) = line[..idx].trim().parse::<u32>() {
                    failed = n;
                }
            }
        }

        let total = passed + failed;
        (passed, failed, total)
    }

    /// Validate Claude's full output and workspace
    pub async fn validate(
        &self,
        claude_output: &str,
        docker_image: &str,
        workspace_dir: &Path,
        timeout_secs: u64,
    ) -> Result<TestGenValidation, TestGenValidatorError> {
        let mut validation = TestGenValidation::default();

        // Check for DESCRIBE section
        validation.describe_found = Self::parse_describe_section(claude_output);

        // Count iterations
        validation.iteration_count = Self::count_iterations(claude_output);

        // Check if tests exist
        validation.tests_created = Self::tests_exist(workspace_dir);

        // If tests exist, run them
        if validation.tests_created {
            let test_result = self
                .run_testgen_tests(docker_image, workspace_dir, timeout_secs)
                .await?;

            validation.tests_passed = test_result.tests_passed;
            validation.tests_passed_count = test_result.tests_passed_count;
            validation.tests_total_count = test_result.tests_total_count;
            validation.test_output = test_result.test_output;
            validation.error = test_result.error;
        }

        Ok(validation)
    }
}

impl Default for TestGenValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_describe_section() {
        let with_describe = r#"
### TASK ANALYSIS
**Goal**: Write a regex
**Output**: /app/regex.txt
"#;
        assert!(TestGenValidator::parse_describe_section(with_describe));

        let without = "Just writing some code...";
        assert!(!TestGenValidator::parse_describe_section(without));
    }

    #[test]
    fn test_count_iterations() {
        let output = r#"
Running pytest /app/testgen_tests.py
...
Running pytest /app/testgen_tests.py again
...
Final pytest /app/testgen_tests.py
"#;
        assert_eq!(TestGenValidator::count_iterations(output), 3);
    }

    #[test]
    fn test_parse_pytest_output() {
        let output = "======================== 5 passed, 2 failed in 0.12s ========================";
        let (passed, failed, total) = TestGenValidator::parse_pytest_output(output);
        assert_eq!(passed, 5);
        assert_eq!(failed, 2);
        assert_eq!(total, 7);

        let output2 = "======================== 3 passed in 0.05s ========================";
        let (passed2, failed2, total2) = TestGenValidator::parse_pytest_output(output2);
        assert_eq!(passed2, 3);
        assert_eq!(failed2, 0);
        assert_eq!(total2, 3);
    }

    #[test]
    fn test_validation_default() {
        let v = TestGenValidation::default();
        assert!(!v.protocol_followed());
        assert!(!v.ready_for_verification());
    }

    #[test]
    fn test_protocol_followed() {
        let mut v = TestGenValidation::default();
        v.describe_found = true;
        v.tests_created = true;
        assert!(v.protocol_followed());

        v.tests_created = false;
        assert!(!v.protocol_followed());
    }
}
