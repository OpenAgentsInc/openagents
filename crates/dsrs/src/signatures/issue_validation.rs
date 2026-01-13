//! Issue Validation Signature.
//!
//! Validates whether an issue is still accurate before starting work.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for validating whether an issue is still accurate and worth working on.
///
/// # Inputs
/// - `issue_title`: The issue's title
/// - `issue_description`: The issue's full description
/// - `blocked_reason`: Why the issue was marked blocked (if any)
/// - `recent_commits`: Git log of recent commits (last 20)
/// - `changed_files`: Files modified in recent commits
///
/// # Outputs
/// - `is_valid`: Whether the issue is still valid to work on
/// - `validation_status`: VALID, ALREADY_ADDRESSED, STALE, or NEEDS_UPDATE
/// - `reason`: Explanation of the validation result
/// - `confidence`: Confidence in the assessment (0.0-1.0)
#[derive(Debug, Clone)]
pub struct IssueValidationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for IssueValidationSignature {
    fn default() -> Self {
        Self {
            instruction:
                r#"You are validating whether a GitHub issue is still accurate and worth working on.

Given the issue details and recent repository activity, determine:

1. Is Valid: Can the agent safely proceed to work on this issue?
   - true: Issue is still relevant and not yet addressed
   - false: Issue appears stale, already addressed, or needs updates

2. Validation Status:
   - VALID: Issue accurately describes current state, safe to proceed
   - ALREADY_ADDRESSED: Recent commits appear to have solved this issue
   - STALE: Issue describes a state that no longer exists (outdated)
   - NEEDS_UPDATE: Issue exists but description needs revision first

3. Reason: Clear explanation for the validation result. If invalid, explain
   what changed or which commits addressed it.

4. Confidence: How confident you are in this assessment (0.0-1.0)

Be thorough in checking:
- Do any recent commit messages mention this issue number or similar work?
- Do the changed files relate to what the issue is asking for?
- Does the blocked_reason (if any) still apply?
- Has the described problem/missing feature been addressed?"#
                    .to_string(),
            demos: vec![],
        }
    }
}

impl IssueValidationSignature {
    /// Create a new issue validation signature.
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

impl MetaSignature for IssueValidationSignature {
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
            "issue_title": {
                "type": "String",
                "desc": "The issue's title",
                "__dsrs_field_type": "input"
            },
            "issue_description": {
                "type": "String",
                "desc": "The issue's full description",
                "__dsrs_field_type": "input"
            },
            "blocked_reason": {
                "type": "String",
                "desc": "Why the issue was marked blocked (if any)",
                "__dsrs_field_type": "input"
            },
            "recent_commits": {
                "type": "String",
                "desc": "Git log of recent commits (last 20)",
                "__dsrs_field_type": "input"
            },
            "changed_files": {
                "type": "String",
                "desc": "Files modified in recent commits",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "is_valid": {
                "type": "bool",
                "desc": "Whether the issue is still valid to work on",
                "__dsrs_field_type": "output"
            },
            "validation_status": {
                "type": "String",
                "desc": "Status: VALID, ALREADY_ADDRESSED, STALE, or NEEDS_UPDATE",
                "__dsrs_field_type": "output"
            },
            "reason": {
                "type": "String",
                "desc": "Explanation of the validation result",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the assessment (0.0-1.0)",
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
    fn test_issue_validation_signature() {
        let sig = IssueValidationSignature::new();

        assert!(!sig.instruction().is_empty());

        let inputs = sig.input_fields();
        assert!(inputs.get("issue_title").is_some());
        assert!(inputs.get("issue_description").is_some());
        assert!(inputs.get("blocked_reason").is_some());
        assert!(inputs.get("recent_commits").is_some());
        assert!(inputs.get("changed_files").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("is_valid").is_some());
        assert!(outputs.get("validation_status").is_some());
        assert!(outputs.get("reason").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
