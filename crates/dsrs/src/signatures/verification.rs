//! Verification Signature.
//!
//! Verifies that code changes are correct and complete.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{json, Value};

/// Signature for verifying code changes are correct and complete.
///
/// # Inputs
/// - `original_request`: User's original task description
/// - `changes_made`: Summary of all edits made
/// - `test_output`: Build/test results (if available)
///
/// # Outputs
/// - `verification_status`: PASS/PARTIAL/FAIL
/// - `missing_requirements`: What's not done yet
/// - `issues_found`: Problems detected in the changes
/// - `suggested_fixes`: How to fix any issues
/// - `confidence`: Confidence in verification (0.0-1.0)
#[derive(Debug, Clone)]
pub struct VerificationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for VerificationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert code reviewer verifying that changes meet requirements.

Analyze the original request, the changes made, and any test output to determine:

1. Verification Status:
   - PASS: All requirements met, no issues found
   - PARTIAL: Some requirements met, others pending
   - FAIL: Critical issues or requirements not addressed

2. Missing Requirements: List any requirements from the original request that
   are not addressed by the changes. Be specific about what's missing.

3. Issues Found: Identify any problems with the changes:
   - Logic errors
   - Style inconsistencies
   - Missing error handling
   - Potential bugs
   - Test failures

4. Suggested Fixes: For each issue, suggest a specific fix.

Be thorough but fair in your assessment."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl VerificationSignature {
    /// Create a new verification signature.
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

impl MetaSignature for VerificationSignature {
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
            "original_request": {
                "type": "String",
                "desc": "User's original task description",
                "__dsrs_field_type": "input"
            },
            "changes_made": {
                "type": "String",
                "desc": "Summary of all code changes made",
                "__dsrs_field_type": "input"
            },
            "test_output": {
                "type": "String",
                "desc": "Build and test results (if available)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "verification_status": {
                "type": "String",
                "desc": "Overall status: PASS, PARTIAL, or FAIL",
                "__dsrs_field_type": "output"
            },
            "missing_requirements": {
                "type": "String",
                "desc": "Requirements from original request not yet addressed",
                "__dsrs_field_type": "output"
            },
            "issues_found": {
                "type": "String",
                "desc": "Problems detected in the changes",
                "__dsrs_field_type": "output"
            },
            "suggested_fixes": {
                "type": "String",
                "desc": "Suggested fixes for any issues found",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in verification assessment (0.0-1.0)",
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
    fn test_verification_signature() {
        let sig = VerificationSignature::new();

        assert!(!sig.instruction().is_empty());

        let inputs = sig.input_fields();
        assert!(inputs.get("original_request").is_some());
        assert!(inputs.get("changes_made").is_some());
        assert!(inputs.get("test_output").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("verification_status").is_some());
        assert!(outputs.get("missing_requirements").is_some());
        assert!(outputs.get("issues_found").is_some());
        assert!(outputs.get("suggested_fixes").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
