//! Tool Result Signature.
//!
//! Interprets tool output and produces step-level learning signals.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for interpreting tool results and producing a learning signal.
///
/// # Inputs
/// - `step_id`: Current plan step ID
/// - `step_description`: Plan step description
/// - `expected_outcome`: Expected outcome from the tool call
/// - `tool_name`: Tool that was executed
/// - `tool_params`: JSON parameters used for the tool call
/// - `tool_output`: Tool output (stdout/content)
/// - `tool_error`: Tool error (stderr or error message)
///
/// # Outputs
/// - `success`: yes/partial/no
/// - `extracted_facts`: JSON array of facts learned
/// - `should_continue`: Whether to continue this step
/// - `step_utility`: Utility score (-1.0 to +1.0)
/// - `confidence`: Confidence score (0.0-1.0)
#[derive(Debug, Clone)]
pub struct ToolResultSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ToolResultSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are interpreting a tool result for a coding agent step.

Rules:
- success must be one of: yes, partial, no
- extracted_facts must be a JSON array of short strings
- step_utility must be between -1.0 and +1.0
- should_continue=true if more tool calls are needed for this step"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ToolResultSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for ToolResultSignature {
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
            "step_id": {
                "type": "String",
                "desc": "Current plan step ID",
                "__dsrs_field_type": "input"
            },
            "step_description": {
                "type": "String",
                "desc": "Plan step description",
                "__dsrs_field_type": "input"
            },
            "expected_outcome": {
                "type": "String",
                "desc": "Expected outcome from the tool call",
                "__dsrs_field_type": "input"
            },
            "tool_name": {
                "type": "String",
                "desc": "Tool that was executed",
                "__dsrs_field_type": "input"
            },
            "tool_params": {
                "type": "String",
                "desc": "JSON parameters used for the tool call",
                "__dsrs_field_type": "input"
            },
            "tool_output": {
                "type": "String",
                "desc": "Tool output (stdout/content)",
                "__dsrs_field_type": "input"
            },
            "tool_error": {
                "type": "String",
                "desc": "Tool error (stderr or error message)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "success": {
                "type": "String",
                "desc": "yes/partial/no",
                "__dsrs_field_type": "output"
            },
            "extracted_facts": {
                "type": "String",
                "desc": "JSON array of facts learned",
                "__dsrs_field_type": "output"
            },
            "should_continue": {
                "type": "bool",
                "desc": "Whether to continue this step",
                "__dsrs_field_type": "output"
            },
            "step_utility": {
                "type": "f32",
                "desc": "Utility score (-1.0 to +1.0)",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the judgment (0.0-1.0)",
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
    fn test_tool_result_signature() {
        let sig = ToolResultSignature::new();
        let inputs = sig.input_fields();
        let outputs = sig.output_fields();

        assert!(inputs.get("tool_output").is_some());
        assert!(outputs.get("step_utility").is_some());
        assert!(outputs.get("should_continue").is_some());
    }
}
