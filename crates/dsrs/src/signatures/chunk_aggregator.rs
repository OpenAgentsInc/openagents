//! Chunk Analysis Aggregator Signature.
//!
//! Aggregates chunk analysis results into actionable next steps.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A finding from chunk analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkFinding {
    /// The chunk that was analyzed.
    pub chunk_path: String,

    /// Line range.
    pub start_line: usize,
    pub end_line: usize,

    /// Type of finding (summary, bug, symbol, etc.).
    pub finding_type: String,

    /// The finding content.
    pub content: String,

    /// Confidence score (0.0-1.0).
    pub confidence: f32,

    /// Related symbols or identifiers.
    pub related_symbols: Vec<String>,
}

/// A recommended next action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    /// Action type (explore, fix, test, etc.).
    pub action_type: String,

    /// Description of what to do.
    pub description: String,

    /// Target files or symbols.
    pub targets: Vec<String>,

    /// Priority (1 = highest).
    pub priority: u32,

    /// Estimated effort (low, medium, high).
    pub effort: String,
}

/// Signature for aggregating chunk analyses into next steps.
///
/// # Inputs
/// - `findings`: List of findings from chunk analyses
/// - `user_task`: The user's original task
/// - `budget_remaining`: Remaining budget in millisatoshis
///
/// # Outputs
/// - `summary`: High-level summary of all findings
/// - `next_actions`: Recommended next actions
/// - `confidence`: Overall confidence in the analysis
/// - `key_insights`: Most important discoveries
#[derive(Debug, Clone)]
pub struct ChunkAnalysisToActionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ChunkAnalysisToActionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert at synthesizing code analysis results. Given findings from
multiple chunk analyses, aggregate them into actionable next steps.

Your job is to:
1. Identify patterns and connections across findings
2. Prioritize the most impactful actions
3. Suggest concrete next steps
4. Highlight key insights that address the user's task

Consider:
- Which findings are most relevant to the user's task?
- Are there any conflicting or duplicate findings?
- What is the most efficient path to solving the problem?
- What uncertainties remain that need more exploration?

Output a cohesive summary and prioritized action list."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ChunkAnalysisToActionSignature {
    /// Create a new chunk analysis aggregator signature.
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

impl MetaSignature for ChunkAnalysisToActionSignature {
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
            "findings": {
                "type": "Vec<ChunkFinding>",
                "desc": "List of findings from chunk analyses",
                "__dsrs_field_type": "input"
            },
            "user_task": {
                "type": "String",
                "desc": "The user's original task",
                "__dsrs_field_type": "input"
            },
            "budget_remaining": {
                "type": "u64",
                "desc": "Remaining budget in millisatoshis",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "summary": {
                "type": "String",
                "desc": "High-level summary of all findings",
                "__dsrs_field_type": "output"
            },
            "next_actions": {
                "type": "Vec<NextAction>",
                "desc": "Recommended next actions in priority order",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Overall confidence in the analysis (0.0-1.0)",
                "__dsrs_field_type": "output"
            },
            "key_insights": {
                "type": "Vec<String>",
                "desc": "Most important discoveries",
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
    fn test_chunk_aggregator_signature() {
        let sig = ChunkAnalysisToActionSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("findings").is_some());
        assert!(inputs.get("user_task").is_some());
        assert!(inputs.get("budget_remaining").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("summary").is_some());
        assert!(outputs.get("next_actions").is_some());
        assert!(outputs.get("confidence").is_some());
        assert!(outputs.get("key_insights").is_some());
    }

    #[test]
    fn test_chunk_finding() {
        let finding = ChunkFinding {
            chunk_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            finding_type: "bug".to_string(),
            content: "Potential null pointer".to_string(),
            confidence: 0.8,
            related_symbols: vec!["process_data".to_string()],
        };

        assert_eq!(finding.finding_type, "bug");
        assert_eq!(finding.confidence, 0.8);
    }

    #[test]
    fn test_next_action() {
        let action = NextAction {
            action_type: "fix".to_string(),
            description: "Add null check".to_string(),
            targets: vec!["src/main.rs:15".to_string()],
            priority: 1,
            effort: "low".to_string(),
        };

        assert_eq!(action.priority, 1);
        assert_eq!(action.effort, "low");
    }
}
