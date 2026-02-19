//! Tool Call Signature.
//!
//! Chooses which tool to call and with what parameters.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for selecting a tool call and parameters.
///
/// # Inputs
/// - `step_id`: Current plan step ID
/// - `step_description`: Plan step description
/// - `step_intent`: Investigate/Modify/Verify/Synthesize
/// - `tool_schemas`: JSON array of available tool schemas
/// - `context_summary`: Relevant context for this step
/// - `execution_history`: Recent tool calls and outcomes
///
/// # Outputs
/// - `tool`: Name of the selected tool
/// - `params`: JSON object with tool parameters
/// - `expected_outcome`: What the tool call should achieve
/// - `progress_estimate`: Estimated progress (0.0-1.0)
/// - `needs_user_input`: Whether user input is required
/// - `user_question`: Question to ask the user (if needed)
/// - `confidence`: Confidence score (0.0-1.0)
#[derive(Debug, Clone)]
pub struct ToolCallSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ToolCallSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are selecting a tool call for a coding agent step.

Rules:
- Choose exactly one tool from tool_schemas.
- params must be valid JSON matching the tool schema.
- If user input is required, set needs_user_input=true and fill user_question.
- When needs_user_input=true, set tool="" and params="{}".
- progress_estimate should reflect progress toward completing the step."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ToolCallSignature {
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

impl MetaSignature for ToolCallSignature {
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
            "step_intent": {
                "type": "String",
                "desc": "Step intent (investigate/modify/verify/synthesize)",
                "__dsrs_field_type": "input"
            },
            "tool_schemas": {
                "type": "String",
                "desc": "JSON array of available tool schemas",
                "__dsrs_field_type": "input"
            },
            "context_summary": {
                "type": "String",
                "desc": "Relevant context for this step",
                "__dsrs_field_type": "input"
            },
            "execution_history": {
                "type": "String",
                "desc": "Recent tool calls and outcomes",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "tool": {
                "type": "String",
                "desc": "Name of the selected tool",
                "__dsrs_field_type": "output"
            },
            "params": {
                "type": "String",
                "desc": "JSON object with tool parameters",
                "__dsrs_field_type": "output"
            },
            "expected_outcome": {
                "type": "String",
                "desc": "What the tool call should achieve",
                "__dsrs_field_type": "output"
            },
            "progress_estimate": {
                "type": "f32",
                "desc": "Estimated progress toward completing the step (0.0-1.0)",
                "__dsrs_field_type": "output"
            },
            "needs_user_input": {
                "type": "bool",
                "desc": "Whether user input is required before proceeding",
                "__dsrs_field_type": "output"
            },
            "user_question": {
                "type": "String",
                "desc": "Question to ask the user if input is required",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the tool choice (0.0-1.0)",
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
    fn test_tool_call_signature() {
        let sig = ToolCallSignature::new();
        let inputs = sig.input_fields();
        let outputs = sig.output_fields();

        assert!(inputs.get("tool_schemas").is_some());
        assert!(outputs.get("tool").is_some());
        assert!(outputs.get("params").is_some());
        assert!(outputs.get("needs_user_input").is_some());
    }
}
