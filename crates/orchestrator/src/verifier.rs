//! Task verification - validates that tasks meet acceptance criteria

use crate::OrchestratorResult;
use serde::{Deserialize, Serialize};

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed
    pub passed: bool,
    /// Overall score (0.0 - 1.0)
    pub score: f32,
    /// Individual check results
    pub checks: Vec<CheckResult>,
    /// Summary message
    pub summary: String,
    /// Suggestions for improvement
    pub suggestions: Vec<String>,
}

/// Result of a single verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Check name
    pub name: String,
    /// Whether check passed
    pub passed: bool,
    /// Score (0.0 - 1.0)
    pub score: f32,
    /// Details about the check
    pub details: String,
}

/// Task verifier
pub struct Verifier {
    /// Strictness threshold (0.0 - 1.0)
    strictness: f32,
    /// Custom checks
    custom_checks: Vec<Box<dyn VerificationCheck>>,
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new(0.7)
    }
}

impl Verifier {
    /// Create a new verifier with given strictness
    pub fn new(strictness: f32) -> Self {
        Self {
            strictness: strictness.clamp(0.0, 1.0),
            custom_checks: Vec::new(),
        }
    }

    /// Add a custom verification check
    pub fn add_check(&mut self, check: Box<dyn VerificationCheck>) {
        self.custom_checks.push(check);
    }

    /// Verify a task completion
    pub async fn verify(
        &self,
        task: &tasks::Task,
        context: &VerificationContext,
    ) -> OrchestratorResult<VerificationResult> {
        let mut checks = Vec::new();

        // Run built-in checks
        checks.push(self.check_files_exist(context).await?);
        checks.push(self.check_tests_pass(context).await?);
        checks.push(self.check_no_syntax_errors(context).await?);
        checks.push(self.check_acceptance_criteria(task, context).await?);

        // Run custom checks
        for custom_check in &self.custom_checks {
            checks.push(custom_check.run(task, context).await?);
        }

        // Calculate overall score
        let total_score: f32 = checks.iter().map(|c| c.score).sum();
        let avg_score = total_score / checks.len() as f32;
        let all_passed = checks.iter().all(|c| c.passed);

        // Determine if verification passed based on strictness
        let passed = all_passed || avg_score >= self.strictness;

        // Generate summary
        let summary = if passed {
            format!(
                "Verification passed with score {:.2}",
                avg_score
            )
        } else {
            format!(
                "Verification failed with score {:.2} (threshold: {:.2})",
                avg_score, self.strictness
            )
        };

        // Collect suggestions from failed checks
        let suggestions: Vec<String> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| format!("{}: {}", c.name, c.details))
            .collect();

        Ok(VerificationResult {
            passed,
            score: avg_score,
            checks,
            summary,
            suggestions,
        })
    }

    async fn check_files_exist(
        &self,
        context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult> {
        let missing: Vec<_> = context
            .expected_files
            .iter()
            .filter(|f| !std::path::Path::new(f).exists())
            .cloned()
            .collect();

        let passed = missing.is_empty();
        let score = if passed {
            1.0
        } else {
            1.0 - (missing.len() as f32 / context.expected_files.len() as f32)
        };

        Ok(CheckResult {
            name: "files_exist".to_string(),
            passed,
            score,
            details: if passed {
                "All expected files exist".to_string()
            } else {
                format!("Missing files: {}", missing.join(", "))
            },
        })
    }

    async fn check_tests_pass(
        &self,
        context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult> {
        if context.test_command.is_none() {
            return Ok(CheckResult {
                name: "tests_pass".to_string(),
                passed: true,
                score: 1.0,
                details: "No test command specified".to_string(),
            });
        }

        let test_cmd = context.test_command.as_ref().unwrap();
        let result = tools::BashTool::execute_with_timeout(test_cmd, 120_000)?;

        let passed = result.success;
        let score = if passed { 1.0 } else { 0.0 };

        Ok(CheckResult {
            name: "tests_pass".to_string(),
            passed,
            score,
            details: if passed {
                "All tests passed".to_string()
            } else {
                format!("Tests failed:\n{}", result.output.chars().take(500).collect::<String>())
            },
        })
    }

    async fn check_no_syntax_errors(
        &self,
        context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult> {
        if context.lint_command.is_none() {
            return Ok(CheckResult {
                name: "no_syntax_errors".to_string(),
                passed: true,
                score: 1.0,
                details: "No lint command specified".to_string(),
            });
        }

        let lint_cmd = context.lint_command.as_ref().unwrap();
        let result = tools::BashTool::execute_with_timeout(lint_cmd, 60_000)?;

        let passed = result.success;
        let score = if passed { 1.0 } else { 0.0 };

        Ok(CheckResult {
            name: "no_syntax_errors".to_string(),
            passed,
            score,
            details: if passed {
                "No syntax errors found".to_string()
            } else {
                format!("Lint errors:\n{}", result.output.chars().take(500).collect::<String>())
            },
        })
    }

    async fn check_acceptance_criteria(
        &self,
        task: &tasks::Task,
        _context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult> {
        // If no acceptance criteria, pass by default
        let criteria = match &task.acceptance_criteria {
            Some(c) if !c.is_empty() => c,
            _ => {
                return Ok(CheckResult {
                    name: "acceptance_criteria".to_string(),
                    passed: true,
                    score: 1.0,
                    details: "No acceptance criteria specified".to_string(),
                });
            }
        };

        // For now, just check that acceptance criteria exist
        // In a real implementation, you'd use LLM to verify each criterion
        Ok(CheckResult {
            name: "acceptance_criteria".to_string(),
            passed: true,
            score: 0.8, // Partial score since we can't fully verify
            details: format!("Acceptance criteria present: {}", criteria.chars().take(100).collect::<String>()),
        })
    }
}

/// Context for verification
#[derive(Debug, Clone, Default)]
pub struct VerificationContext {
    /// Files that should exist after task completion
    pub expected_files: Vec<String>,
    /// Command to run tests
    pub test_command: Option<String>,
    /// Command to run linter
    pub lint_command: Option<String>,
    /// Command to run build
    pub build_command: Option<String>,
    /// Custom context data
    pub data: serde_json::Value,
}

impl VerificationContext {
    /// Create a new verification context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set expected files
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.expected_files = files;
        self
    }

    /// Set test command
    pub fn with_test_command(mut self, cmd: impl Into<String>) -> Self {
        self.test_command = Some(cmd.into());
        self
    }

    /// Set lint command
    pub fn with_lint_command(mut self, cmd: impl Into<String>) -> Self {
        self.lint_command = Some(cmd.into());
        self
    }

    /// Set build command
    pub fn with_build_command(mut self, cmd: impl Into<String>) -> Self {
        self.build_command = Some(cmd.into());
        self
    }
}

/// Trait for custom verification checks
#[async_trait::async_trait]
pub trait VerificationCheck: Send + Sync {
    /// Run the verification check
    async fn run(
        &self,
        task: &tasks::Task,
        context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult>;

    /// Get the check name
    fn name(&self) -> &str;
}

/// A simple custom check that runs a command
pub struct CommandCheck {
    name: String,
    command: String,
    description: String,
}

impl CommandCheck {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            description: description.into(),
        }
    }
}

#[async_trait::async_trait]
impl VerificationCheck for CommandCheck {
    async fn run(
        &self,
        _task: &tasks::Task,
        _context: &VerificationContext,
    ) -> OrchestratorResult<CheckResult> {
        let result = tools::BashTool::execute_with_timeout(&self.command, 60_000)?;

        Ok(CheckResult {
            name: self.name.clone(),
            passed: result.success,
            score: if result.success { 1.0 } else { 0.0 },
            details: if result.success {
                self.description.clone()
            } else {
                format!("{}: {}", self.description, result.output.chars().take(200).collect::<String>())
            },
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_creation() {
        let verifier = Verifier::new(0.7);
        assert_eq!(verifier.strictness, 0.7);
    }

    #[test]
    fn test_verification_context() {
        let context = VerificationContext::new()
            .with_test_command("cargo test")
            .with_files(vec!["src/lib.rs".to_string()]);

        assert_eq!(context.test_command, Some("cargo test".to_string()));
        assert_eq!(context.expected_files.len(), 1);
    }

    #[tokio::test]
    async fn test_files_exist_check() {
        let verifier = Verifier::new(0.7);
        let context = VerificationContext::new().with_files(vec!["/tmp".to_string()]);

        let result = verifier.check_files_exist(&context).await.unwrap();
        assert!(result.passed);
        assert_eq!(result.score, 1.0);
    }
}
