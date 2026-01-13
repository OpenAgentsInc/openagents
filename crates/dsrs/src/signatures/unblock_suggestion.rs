//! Unblock Suggestion Signature.
//!
//! Analyzes blocked issues to recommend which one to unblock first.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Signature for suggesting which blocked issue to unblock first.
///
/// # Inputs
/// - `blocked_issues`: JSON array of blocked issue summaries with blocked reasons
/// - `workspace_context`: Current workspace state (active directive, project info)
/// - `recent_commits`: Recent git commit history for context
///
/// # Outputs
/// - `selected_issue_number`: Issue number to unblock first
/// - `unblock_rationale`: Why this issue should be unblocked first
/// - `unblock_strategy`: How to unblock this issue
/// - `estimated_effort`: Effort required (low/medium/high)
/// - `cascade_potential`: Whether unblocking this helps other issues
#[derive(Debug, Clone)]
pub struct UnblockSuggestionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for UnblockSuggestionSignature {
    fn default() -> Self {
        let instruction =
            r#"You are an expert at analyzing blocked work items and finding paths forward.
Given a list of blocked issues with their blocking reasons, determine which issue to unblock first.

Consider these factors:
1. Effort to unblock (prefer lower effort)
2. Value once unblocked (prefer higher impact issues)
3. Cascade potential (unblocking one may unblock others)
4. Alignment with recent work (leverage existing context)
5. Clarity of unblock path (some blockers are clearer to resolve)

Analyze each blocked issue's reason and determine:
- Which issue has the most achievable unblock path
- What concrete steps would unblock it
- Whether unblocking it would help other issues

OUTPUT FORMAT (use this exact JSON structure):
selected_issue_number: 123
unblock_rationale: "Clear explanation of why this issue should be unblocked first"
unblock_strategy: "Specific steps to unblock: 1) Do X, 2) Do Y, 3) Then Z"
estimated_effort: "low"
cascade_potential: "Unblocking this would also help issues #456 and #789"

Be specific and actionable. Focus on the most practical path forward."#
                .to_string();

        // Create demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "blocked_issues".to_string(),
            json!([
                {
                    "number": 6,
                    "title": "Refactor main.rs into modules",
                    "blocked_reason": "File is 2400 lines, requires extensive analysis",
                    "priority": "low"
                },
                {
                    "number": 21,
                    "title": "Add README for crate",
                    "blocked_reason": "Crate has no source code yet",
                    "priority": "medium"
                },
                {
                    "number": 35,
                    "title": "Document API endpoints",
                    "blocked_reason": "API design not finalized",
                    "priority": "high"
                }
            ]),
        );
        demo_data.insert(
            "workspace_context".to_string(),
            json!("Rust CLI tool for autonomous coding. Recent work focused on UI and bootloader."),
        );
        demo_data.insert(
            "recent_commits".to_string(),
            json!("abc123 Add bootloader graph\ndef456 Implement issue suggestions\nghi789 Fix staleness filter"),
        );
        demo_data.insert("selected_issue_number".to_string(), json!(6));
        demo_data.insert(
            "unblock_rationale".to_string(),
            json!("The 2400-line file can be analyzed incrementally. Breaking it into smaller modules will improve maintainability and enable future refactoring tasks."),
        );
        demo_data.insert(
            "unblock_strategy".to_string(),
            json!("1) Read and map the current module structure, 2) Identify logical groupings (commands, UI, config), 3) Extract one module at a time with tests, 4) Update imports incrementally"),
        );
        demo_data.insert("estimated_effort".to_string(), json!("medium"));
        demo_data.insert(
            "cascade_potential".to_string(),
            json!("Completing this refactor would improve code organization and make issues #21 and #35 easier to document"),
        );

        let demo = Example {
            data: demo_data,
            input_keys: vec![
                "blocked_issues".to_string(),
                "workspace_context".to_string(),
                "recent_commits".to_string(),
            ],
            output_keys: vec![
                "selected_issue_number".to_string(),
                "unblock_rationale".to_string(),
                "unblock_strategy".to_string(),
                "estimated_effort".to_string(),
                "cascade_potential".to_string(),
            ],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl UnblockSuggestionSignature {
    /// Create a new unblock suggestion signature.
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

impl MetaSignature for UnblockSuggestionSignature {
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
            "blocked_issues": {
                "type": "String",
                "desc": "JSON array of blocked issue summaries with number, title, blocked_reason, priority",
                "__dsrs_field_type": "input"
            },
            "workspace_context": {
                "type": "String",
                "desc": "Current workspace state: project description, active directive, recent work patterns",
                "__dsrs_field_type": "input"
            },
            "recent_commits": {
                "type": "String",
                "desc": "Recent git commit history showing recent work focus",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "selected_issue_number": {
                "type": "u32",
                "desc": "Issue number of the recommended issue to unblock first",
                "__dsrs_field_type": "output"
            },
            "unblock_rationale": {
                "type": "String",
                "desc": "Explanation of why this issue should be unblocked first",
                "__dsrs_field_type": "output"
            },
            "unblock_strategy": {
                "type": "String",
                "desc": "Concrete steps to unblock this issue",
                "__dsrs_field_type": "output"
            },
            "estimated_effort": {
                "type": "String",
                "desc": "Estimated effort to unblock: low, medium, or high",
                "__dsrs_field_type": "output"
            },
            "cascade_potential": {
                "type": "String",
                "desc": "Description of how unblocking this issue helps other blocked issues",
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
    fn test_unblock_suggestion_signature() {
        let sig = UnblockSuggestionSignature::new();

        assert!(!sig.instruction().is_empty());
        assert!(sig.instruction().contains("blocked"));

        let inputs = sig.input_fields();
        assert!(inputs.get("blocked_issues").is_some());
        assert!(inputs.get("workspace_context").is_some());
        assert!(inputs.get("recent_commits").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("selected_issue_number").is_some());
        assert!(outputs.get("unblock_rationale").is_some());
        assert!(outputs.get("unblock_strategy").is_some());
        assert!(outputs.get("estimated_effort").is_some());
        assert!(outputs.get("cascade_potential").is_some());
    }

    #[test]
    fn test_has_demo() {
        let sig = UnblockSuggestionSignature::new();
        let demos = sig.demos();
        assert!(!demos.is_empty());

        let demo = &demos[0];
        assert!(demo.input_keys.contains(&"blocked_issues".to_string()));
        assert!(
            demo.output_keys
                .contains(&"selected_issue_number".to_string())
        );
    }
}
