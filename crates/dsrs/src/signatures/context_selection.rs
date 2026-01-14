//! Context Selection Signature.
//!
//! Selects which context to include for the next turn.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for selecting the context slice for a coding agent turn.
///
/// # Inputs
/// - `session_summary`: Rolling summary of the session so far
/// - `recent_turns`: Recent dialogue/step history
/// - `file_history`: Recently touched files and notes
/// - `token_budget`: Token budget for context assembly
/// - `privacy_mode`: Privacy profile (local, redacted, swarm)
/// - `lane_constraints`: Lane/model constraints for this turn
///
/// # Outputs
/// - `context_plan`: JSON object describing what to include/exclude
/// - `reasoning`: Short explanation for the selection
/// - `confidence`: Confidence score (0.0-1.0)
#[derive(Debug, Clone)]
pub struct ContextSelectionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ContextSelectionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are selecting the context slice for the next coding agent turn.

Produce a JSON object for `context_plan` with this shape:
{
  "include_paths": ["path/one", "path/two"],
  "exclude_paths": ["path/three"],
  "include_summaries": ["summary_id_or_label"],
  "notes": "short rationale"
}

Rules:
- Prefer the smallest set of files that still support the next step.
- Include summaries when files are large or already summarized.
- Respect privacy mode and lane constraints.
- If unsure, include the most recently touched files."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ContextSelectionSignature {
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

impl MetaSignature for ContextSelectionSignature {
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
            "session_summary": {
                "type": "String",
                "desc": "Rolling summary of the session so far",
                "__dsrs_field_type": "input"
            },
            "recent_turns": {
                "type": "String",
                "desc": "Recent dialogue or step history",
                "__dsrs_field_type": "input"
            },
            "file_history": {
                "type": "String",
                "desc": "Recently touched files and notes",
                "__dsrs_field_type": "input"
            },
            "token_budget": {
                "type": "String",
                "desc": "Token budget for context assembly",
                "__dsrs_field_type": "input"
            },
            "privacy_mode": {
                "type": "String",
                "desc": "Privacy profile (local/redacted/swarm)",
                "__dsrs_field_type": "input"
            },
            "lane_constraints": {
                "type": "String",
                "desc": "Lane/model constraints for this turn",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "context_plan": {
                "type": "String",
                "desc": "JSON object describing what context to include",
                "__dsrs_field_type": "output"
            },
            "reasoning": {
                "type": "String",
                "desc": "Short explanation for the selection",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the context selection (0.0-1.0)",
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
    fn test_context_selection_signature() {
        let sig = ContextSelectionSignature::new();
        let inputs = sig.input_fields();
        let outputs = sig.output_fields();

        assert!(inputs.get("session_summary").is_some());
        assert!(inputs.get("token_budget").is_some());
        assert!(outputs.get("context_plan").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
