//! TB2 Verifier - Runs tests and parses results
//!
//! This module handles running the Terminal-Bench 2 verification tests
//! and parsing the results from reward.txt and ctrf.json.

use sandbox::{ContainerBackend, ContainerConfig, DockerBackend};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::Duration;
use terminalbench::TB2Task;
use thiserror::Error;

/// Verification result
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether all tests passed
    pub passed: bool,
    /// Number of tests that passed
    pub tests_passed: u32,
    /// Total number of tests
    pub tests_total: u32,
    /// Reward value (1.0 = pass, 0.0 = fail)
    pub reward: f64,
    /// Test output
    pub output: String,
    /// Error message if verification failed to run
    pub error: Option<String>,
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self {
            passed: false,
            tests_passed: 0,
            tests_total: 0,
            reward: 0.0,
            output: String::new(),
            error: None,
        }
    }
}

/// CTRF (Common Test Results Format) report
#[derive(Debug, Clone, Deserialize)]
pub struct CtrfReport {
    pub results: CtrfResults,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CtrfResults {
    pub tool: CtrfTool,
    pub summary: CtrfSummary,
    #[serde(default)]
    pub tests: Vec<CtrfTest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CtrfTool {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CtrfSummary {
    pub tests: u32,
    pub passed: u32,
    pub failed: u32,
    pub pending: u32,
    pub skipped: u32,
    pub other: u32,
    #[serde(default)]
    pub start: Option<u64>,
    #[serde(default)]
    pub stop: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CtrfTest {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub trace: Option<String>,
}

/// Errors from verification
#[derive(Debug, Error)]
pub enum VerifierError {
    #[error("Docker not available")]
    DockerNotAvailable,

    #[error("Verification failed to run: {0}")]
    RunFailed(String),

    #[error("Failed to read reward.txt: {0}")]
    RewardReadError(String),

    #[error("Failed to parse ctrf.json: {0}")]
    CtrfParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Timeout")]
    Timeout,
}

/// TB2 Verifier
pub struct TB2Verifier {
    backend: DockerBackend,
}

impl TB2Verifier {
    /// Create a new verifier
    pub fn new() -> Self {
        Self {
            backend: DockerBackend::new(),
        }
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        self.backend.is_available().await
    }

    /// Run verification tests in container
    pub async fn run_tests(
        &self,
        task: &TB2Task,
        workspace_dir: &Path,
        logs_dir: &Path,
    ) -> Result<VerificationResult, VerifierError> {
        if !self.is_available().await {
            return Err(VerifierError::DockerNotAvailable);
        }

        // Ensure verifier logs directory exists
        let verifier_logs = logs_dir.join("verifier");
        fs::create_dir_all(&verifier_logs)?;

        tracing::info!(
            target: "mechacoder::verifier",
            task_id = %task.id,
            "Running verification tests"
        );

        // Build container config
        let timeout_sec = task.verifier_timeout_sec() as u64;
        let config = ContainerConfig::new(task.docker_image(), workspace_dir)
            .workdir("/app")
            .memory_limit(task.memory_limit())
            .cpu_limit(task.cpu_limit() as f32)
            .timeout(Duration::from_secs(timeout_sec))
            // Mount logs
            .volume_mount(format!("{}:/logs", logs_dir.display()))
            // Mount tests
            .volume_mount(format!("{}:/tests:ro", task.tests_dir.display()));

        // Run test.sh
        let command = vec![
            "bash".to_string(),
            "-c".to_string(),
            "bash /tests/test.sh 2>&1".to_string(),
        ];

        let run_result = self.backend.run(&command, &config).await.map_err(|e| {
            VerifierError::RunFailed(e.to_string())
        })?;

        tracing::debug!(
            target: "mechacoder::verifier",
            exit_code = run_result.exit_code,
            "Test script completed"
        );

        // Parse results
        let mut result = VerificationResult {
            output: run_result.combined_output(),
            ..Default::default()
        };

        // Try to parse reward.txt
        match self.parse_reward(&verifier_logs) {
            Ok(reward) => {
                result.reward = reward;
                result.passed = reward >= 1.0;
            }
            Err(e) => {
                tracing::warn!(
                    target: "mechacoder::verifier",
                    error = %e,
                    "Failed to parse reward.txt"
                );
                result.error = Some(format!("Failed to parse reward: {}", e));
            }
        }

        // Try to parse ctrf.json for detailed results
        match self.parse_ctrf(&verifier_logs) {
            Ok(ctrf) => {
                result.tests_passed = ctrf.results.summary.passed;
                result.tests_total = ctrf.results.summary.tests;
            }
            Err(e) => {
                tracing::debug!(
                    target: "mechacoder::verifier",
                    error = %e,
                    "Failed to parse ctrf.json (may not exist)"
                );
            }
        }

        tracing::info!(
            target: "mechacoder::verifier",
            passed = result.passed,
            reward = result.reward,
            tests_passed = result.tests_passed,
            tests_total = result.tests_total,
            "Verification complete"
        );

        Ok(result)
    }

    /// Parse reward.txt (contains "1" or "0")
    fn parse_reward(&self, verifier_logs: &Path) -> Result<f64, VerifierError> {
        let reward_path = verifier_logs.join("reward.txt");

        let content = fs::read_to_string(&reward_path).map_err(|e| {
            VerifierError::RewardReadError(format!(
                "Failed to read {}: {}",
                reward_path.display(),
                e
            ))
        })?;

        let reward: f64 = content.trim().parse().map_err(|_| {
            VerifierError::RewardReadError(format!(
                "Invalid reward value: '{}'",
                content.trim()
            ))
        })?;

        Ok(reward)
    }

    /// Parse CTRF JSON for test details
    fn parse_ctrf(&self, verifier_logs: &Path) -> Result<CtrfReport, VerifierError> {
        let ctrf_path = verifier_logs.join("ctrf.json");

        let content = fs::read_to_string(&ctrf_path).map_err(|e| {
            VerifierError::CtrfParseError(format!(
                "Failed to read {}: {}",
                ctrf_path.display(),
                e
            ))
        })?;

        let report: CtrfReport = serde_json::from_str(&content).map_err(|e| {
            VerifierError::CtrfParseError(format!("Failed to parse CTRF JSON: {}", e))
        })?;

        Ok(report)
    }
}

impl Default for TB2Verifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctrf_summary_deserialize() {
        let json = r#"{
            "results": {
                "tool": {"name": "pytest", "version": "8.4.1"},
                "summary": {
                    "tests": 5,
                    "passed": 4,
                    "failed": 1,
                    "pending": 0,
                    "skipped": 0,
                    "other": 0
                },
                "tests": []
            }
        }"#;

        let report: CtrfReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.results.summary.tests, 5);
        assert_eq!(report.results.summary.passed, 4);
        assert_eq!(report.results.summary.failed, 1);
    }

    #[test]
    fn test_verification_result_default() {
        let result = VerificationResult::default();
        assert!(!result.passed);
        assert_eq!(result.reward, 0.0);
        assert_eq!(result.tests_passed, 0);
    }
}
