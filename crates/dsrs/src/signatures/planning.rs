//! Planning Signature.
//!
//! Emits a PlanIR with stable step IDs.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for producing a structured plan (PlanIR).
///
/// # Inputs
/// - `task_description`: User request or issue description
/// - `repo_context`: Repository summary or manifest
/// - `file_tree`: High-level file structure
/// - `context_summary`: Context selected for this turn
/// - `constraints`: Budget, privacy, and lane constraints
///
/// # Outputs
/// - `analysis`: High-level analysis of the task
/// - `steps`: JSON array of plan steps (PlanIR.steps)
/// - `verification_strategy`: JSON object for verification
/// - `complexity`: Low/Medium/High/VeryHigh
/// - `confidence`: Planner confidence (0.0-1.0)
#[derive(Debug, Clone)]
pub struct PlanningSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for PlanningSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are a planning agent for a coding task.

Output a structured plan with stable step IDs.

Rules:
- steps must be a JSON array of objects with fields:
  id, description, intent, target_files, depends_on, max_iterations
- intent must be one of: investigate, modify, verify, synthesize
- verification_strategy must be a JSON object with:
  commands (array), success_criteria (string), max_retries (number)
- complexity must be one of: low, medium, high, veryhigh
- Keep step IDs stable and sequential (step-1, step-2, ...)"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl PlanningSignature {
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

impl MetaSignature for PlanningSignature {
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
            "task_description": {
                "type": "String",
                "desc": "User request or issue description",
                "__dsrs_field_type": "input"
            },
            "repo_context": {
                "type": "String",
                "desc": "Repository summary or manifest",
                "__dsrs_field_type": "input"
            },
            "file_tree": {
                "type": "String",
                "desc": "High-level file structure",
                "__dsrs_field_type": "input"
            },
            "context_summary": {
                "type": "String",
                "desc": "Context selected for this turn",
                "__dsrs_field_type": "input"
            },
            "constraints": {
                "type": "String",
                "desc": "Budget, privacy, and lane constraints",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "analysis": {
                "type": "String",
                "desc": "High-level analysis of the task",
                "__dsrs_field_type": "output"
            },
            "steps": {
                "type": "String",
                "desc": "JSON array of plan steps (PlanIR.steps)",
                "__dsrs_field_type": "output"
            },
            "verification_strategy": {
                "type": "String",
                "desc": "JSON object describing verification commands and criteria",
                "__dsrs_field_type": "output"
            },
            "complexity": {
                "type": "String",
                "desc": "Overall complexity: low/medium/high/veryhigh",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Planner confidence (0.0-1.0)",
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
    fn test_planning_signature() {
        let sig = PlanningSignature::new();
        let inputs = sig.input_fields();
        let outputs = sig.output_fields();

        assert!(inputs.get("task_description").is_some());
        assert!(outputs.get("steps").is_some());
        assert!(outputs.get("verification_strategy").is_some());
        assert!(outputs.get("complexity").is_some());
    }
}
