//! Task Understanding Signature.
//!
//! Parses user intent, extracts requirements, and identifies task scope.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Signature for understanding user tasks and extracting requirements.
///
/// # Inputs
/// - `user_request`: Raw user task description
/// - `repo_context`: Brief repo overview (from manifest or README)
///
/// # Outputs
/// - `task_type`: FEATURE/BUGFIX/REFACTOR/DOCS/TEST
/// - `requirements`: JSON array of specific requirements
/// - `scope_estimate`: SMALL/MEDIUM/LARGE
/// - `clarifying_questions`: Questions if request is ambiguous
/// - `confidence`: Confidence score (0.0-1.0)
#[derive(Debug, Clone)]
pub struct TaskUnderstandingSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for TaskUnderstandingSignature {
    fn default() -> Self {
        let instruction = r#"You are an expert at understanding software development tasks.
Given a user's request and repository context, analyze and extract:

1. Task Type: Classify as FEATURE (new functionality), BUGFIX (fixing broken behavior),
   REFACTOR (restructuring code), DOCS (documentation), or TEST (adding tests)

2. Requirements: Break down the request into specific, actionable requirements.

3. Scope: Estimate as SMALL (< 50 lines, 1-2 files), MEDIUM (50-200 lines, 2-5 files),
   or LARGE (> 200 lines, 5+ files)

4. Questions: If the request is ambiguous, list clarifying questions.
   Leave empty if the request is clear.

OUTPUT FORMAT (use these exact structures):
- task_type: "FEATURE" (or BUGFIX, REFACTOR, DOCS, TEST)
- requirements: ["Add X functionality", "Modify Y to support Z"]
- scope_estimate: "SMALL" (or MEDIUM, LARGE)
- clarifying_questions: ["What should happen when...?"] or []
- confidence: 0.85 (decimal between 0 and 1)

Be precise. Use actual values, not placeholders."#
            .to_string();

        // Create demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "user_request".to_string(),
            json!("Add a --version flag that prints the version from Cargo.toml"),
        );
        demo_data.insert(
            "repo_context".to_string(),
            json!("Rust CLI tool using clap for argument parsing"),
        );
        demo_data.insert("task_type".to_string(), json!("FEATURE"));
        demo_data.insert(
            "requirements".to_string(),
            json!([
                "Add --version flag to clap Args struct",
                "Use env!(\"CARGO_PKG_VERSION\") to get version",
                "Print version and exit when flag is passed"
            ]),
        );
        demo_data.insert("scope_estimate".to_string(), json!("SMALL"));
        demo_data.insert("clarifying_questions".to_string(), json!([]));
        demo_data.insert("confidence".to_string(), json!(0.95));

        let demo = Example {
            data: demo_data,
            input_keys: vec!["user_request".to_string(), "repo_context".to_string()],
            output_keys: vec![
                "task_type".to_string(),
                "requirements".to_string(),
                "scope_estimate".to_string(),
                "clarifying_questions".to_string(),
                "confidence".to_string(),
            ],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl TaskUnderstandingSignature {
    /// Create a new task understanding signature.
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

impl MetaSignature for TaskUnderstandingSignature {
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
            "user_request": {
                "type": "String",
                "desc": "Raw user task description",
                "__dsrs_field_type": "input"
            },
            "repo_context": {
                "type": "String",
                "desc": "Brief repository overview (from manifest or README)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "task_type": {
                "type": "String",
                "desc": "Task classification: FEATURE, BUGFIX, REFACTOR, DOCS, or TEST",
                "__dsrs_field_type": "output"
            },
            "requirements": {
                "type": "String",
                "desc": "JSON array of specific requirements extracted from the request",
                "__dsrs_field_type": "output"
            },
            "scope_estimate": {
                "type": "String",
                "desc": "Estimated scope: SMALL, MEDIUM, or LARGE",
                "__dsrs_field_type": "output"
            },
            "clarifying_questions": {
                "type": "String",
                "desc": "Questions to ask if the request is ambiguous (empty if clear)",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the analysis (0.0-1.0)",
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
    fn test_task_understanding_signature() {
        let sig = TaskUnderstandingSignature::new();

        assert!(!sig.instruction().is_empty());

        let inputs = sig.input_fields();
        assert!(inputs.get("user_request").is_some());
        assert!(inputs.get("repo_context").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("task_type").is_some());
        assert!(outputs.get("requirements").is_some());
        assert!(outputs.get("scope_estimate").is_some());
        assert!(outputs.get("clarifying_questions").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
