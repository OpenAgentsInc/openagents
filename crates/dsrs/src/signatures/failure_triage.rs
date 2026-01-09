//! Failure Triage Signature.
//!
//! Interprets sandbox failures and recommends next actions.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Diagnosis of a sandbox failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDiagnosis {
    /// Root cause category.
    pub category: FailureCategory,

    /// Detailed explanation.
    pub explanation: String,

    /// Confidence in the diagnosis (0.0-1.0).
    pub confidence: f32,

    /// Related error patterns.
    pub patterns: Vec<String>,
}

/// Categories of sandbox failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureCategory {
    /// Out of memory.
    OutOfMemory,

    /// Timeout exceeded.
    Timeout,

    /// Compilation error.
    CompileError,

    /// Test failure.
    TestFailure,

    /// Missing dependency.
    MissingDependency,

    /// Permission denied.
    PermissionDenied,

    /// Network error.
    NetworkError,

    /// Configuration error.
    ConfigError,

    /// Unknown/other error.
    Unknown,
}

impl std::fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureCategory::OutOfMemory => write!(f, "out_of_memory"),
            FailureCategory::Timeout => write!(f, "timeout"),
            FailureCategory::CompileError => write!(f, "compile_error"),
            FailureCategory::TestFailure => write!(f, "test_failure"),
            FailureCategory::MissingDependency => write!(f, "missing_dependency"),
            FailureCategory::PermissionDenied => write!(f, "permission_denied"),
            FailureCategory::NetworkError => write!(f, "network_error"),
            FailureCategory::ConfigError => write!(f, "config_error"),
            FailureCategory::Unknown => write!(f, "unknown"),
        }
    }
}

/// Recommended action after failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageAction {
    /// Retry with larger resources.
    RetryLarger,

    /// Retry with longer timeout.
    RetryLonger,

    /// Fix code and retry.
    FixAndRetry,

    /// Install missing dependency.
    InstallDependency,

    /// Change configuration.
    UpdateConfig,

    /// Skip this command.
    Skip,

    /// Escalate to user.
    Escalate,

    /// Abort the task.
    Abort,
}

impl std::fmt::Display for TriageAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriageAction::RetryLarger => write!(f, "retry_larger"),
            TriageAction::RetryLonger => write!(f, "retry_longer"),
            TriageAction::FixAndRetry => write!(f, "fix_and_retry"),
            TriageAction::InstallDependency => write!(f, "install_dependency"),
            TriageAction::UpdateConfig => write!(f, "update_config"),
            TriageAction::Skip => write!(f, "skip"),
            TriageAction::Escalate => write!(f, "escalate"),
            TriageAction::Abort => write!(f, "abort"),
        }
    }
}

/// Signature for triaging sandbox failures.
///
/// # Inputs
/// - `command`: The command that failed
/// - `exit_code`: Exit code of the command
/// - `stderr_preview`: Preview of stderr output
/// - `stdout_preview`: Preview of stdout output
/// - `duration_ms`: How long the command ran
///
/// # Outputs
/// - `diagnosis`: Diagnosis of the failure
/// - `next_action`: Recommended next action
/// - `should_retry`: Whether to retry the command
/// - `fix_suggestion`: Suggested fix (if applicable)
#[derive(Debug, Clone)]
pub struct FailureTriageSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for FailureTriageSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert at diagnosing build and test failures. Given a failed command
and its output, determine the root cause and recommend the best next action.

Common failure patterns:
- OOM: "out of memory", "cannot allocate", signal 9/137
- Timeout: "timed out", signal 15, killed
- Compile: "error[E", "cannot find", "undefined reference"
- Test: "FAILED", "assertion failed", "panic"
- Dependency: "no such file", "package not found", "module not found"
- Network: "connection refused", "timeout", "DNS"
- Permission: "permission denied", "EACCES"

Consider:
1. Is this a transient or persistent error?
2. Can it be fixed by changing resources vs fixing code?
3. Is it safe to retry or will it waste budget?
4. Should the user be notified?

Output diagnosis, recommended action, and any fix suggestions."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl FailureTriageSignature {
    /// Create a new failure triage signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Add a demonstration example.
    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for FailureTriageSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "command": {
                "type": "String",
                "desc": "The command that failed",
                "__dsrs_field_type": "input"
            },
            "exit_code": {
                "type": "i32",
                "desc": "Exit code of the command",
                "__dsrs_field_type": "input"
            },
            "stderr_preview": {
                "type": "String",
                "desc": "Preview of stderr output (first/last 1000 chars)",
                "__dsrs_field_type": "input"
            },
            "stdout_preview": {
                "type": "String",
                "desc": "Preview of stdout output (first/last 1000 chars)",
                "__dsrs_field_type": "input"
            },
            "duration_ms": {
                "type": "u64",
                "desc": "How long the command ran in milliseconds",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "diagnosis": {
                "type": "FailureDiagnosis",
                "desc": "Diagnosis of the failure",
                "__dsrs_field_type": "output"
            },
            "next_action": {
                "type": "TriageAction",
                "desc": "Recommended next action",
                "__dsrs_field_type": "output"
            },
            "should_retry": {
                "type": "bool",
                "desc": "Whether to retry the command",
                "__dsrs_field_type": "output"
            },
            "fix_suggestion": {
                "type": "String",
                "desc": "Suggested fix if applicable (optional)",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_triage_signature() {
        let sig = FailureTriageSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("command").is_some());
        assert!(inputs.get("exit_code").is_some());
        assert!(inputs.get("stderr_preview").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("diagnosis").is_some());
        assert!(outputs.get("next_action").is_some());
        assert!(outputs.get("should_retry").is_some());
    }

    #[test]
    fn test_failure_category_display() {
        assert_eq!(FailureCategory::OutOfMemory.to_string(), "out_of_memory");
        assert_eq!(FailureCategory::CompileError.to_string(), "compile_error");
        assert_eq!(FailureCategory::TestFailure.to_string(), "test_failure");
    }

    #[test]
    fn test_triage_action_display() {
        assert_eq!(TriageAction::RetryLarger.to_string(), "retry_larger");
        assert_eq!(TriageAction::FixAndRetry.to_string(), "fix_and_retry");
        assert_eq!(TriageAction::Escalate.to_string(), "escalate");
    }
}
