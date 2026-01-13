//! Issue Suggestion Signature.
//!
//! Analyzes available issues and context to recommend top issues to work on.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Signature for suggesting which issues to work on next.
///
/// # Inputs
/// - `available_issues`: JSON array of available issue summaries (non-stale, non-blocked)
/// - `workspace_context`: Current workspace state (active directive, project info)
/// - `recent_work`: JSON array of recently completed/worked issue numbers
/// - `user_preferences`: Optional user preferences for issue types and priorities
///
/// # Outputs
/// - `suggestions`: JSON array of top 3 suggested issues with rationale
/// - `confidence`: Overall confidence in suggestions (0.0-1.0)
#[derive(Debug, Clone)]
pub struct IssueSuggestionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for IssueSuggestionSignature {
    fn default() -> Self {
        let instruction = r#"You are an expert at prioritizing software development work.
Given a list of available issues and workspace context, recommend the top 3 issues to work on.

Consider these factors in priority order:
1. Priority level (urgent > high > medium > low)
2. Issue type relevance (bugs often need faster attention than features)
3. Recency (newer issues may have better context)
4. Blocking relationships (unblock dependencies first)
5. Alignment with current work (related issues are efficient to batch)
6. User's recent activity patterns

For each suggestion, provide:
- Issue number and title
- Rationale explaining why this issue is a good choice now
- Estimated complexity (low/medium/high)

OUTPUT FORMAT (use this exact JSON structure):
suggestions: [
  {"number": 123, "title": "Issue title here", "rationale": "Why this is a good choice", "complexity": "low"},
  {"number": 456, "title": "Second issue", "rationale": "Reason for recommendation", "complexity": "medium"},
  {"number": 789, "title": "Third issue", "rationale": "Why include this one", "complexity": "high"}
]
confidence: 0.85

Be precise. Return exactly 3 suggestions if 3+ issues are available. Use actual issue numbers and titles from the input."#
            .to_string();

        // Create demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "available_issues".to_string(),
            json!([
                {"number": 15, "title": "Fix timeout in API calls", "priority": "high", "issue_type": "bug"},
                {"number": 22, "title": "Add dark mode support", "priority": "medium", "issue_type": "feature"},
                {"number": 8, "title": "Refactor auth module", "priority": "low", "issue_type": "task"}
            ]),
        );
        demo_data.insert(
            "workspace_context".to_string(),
            json!("Rust CLI tool with recent work on UI components"),
        );
        demo_data.insert("recent_work".to_string(), json!([10, 12, 14]));
        demo_data.insert("user_preferences".to_string(), json!({}));
        demo_data.insert(
            "suggestions".to_string(),
            json!([
                {"number": 15, "title": "Fix timeout in API calls", "rationale": "High priority bug affecting users", "complexity": "medium"},
                {"number": 22, "title": "Add dark mode support", "rationale": "Aligns with recent UI work", "complexity": "medium"},
                {"number": 8, "title": "Refactor auth module", "rationale": "Good for batching with other maintenance", "complexity": "high"}
            ]),
        );
        demo_data.insert("confidence".to_string(), json!(0.9));

        let demo = Example {
            data: demo_data,
            input_keys: vec![
                "available_issues".to_string(),
                "workspace_context".to_string(),
                "recent_work".to_string(),
                "user_preferences".to_string(),
            ],
            output_keys: vec!["suggestions".to_string(), "confidence".to_string()],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl IssueSuggestionSignature {
    /// Create a new issue suggestion signature.
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

impl MetaSignature for IssueSuggestionSignature {
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
            "available_issues": {
                "type": "String",
                "desc": "JSON array of available issue summaries (non-stale, non-blocked)",
                "__dsrs_field_type": "input"
            },
            "workspace_context": {
                "type": "String",
                "desc": "Current workspace state: active directive, project info, recent commits",
                "__dsrs_field_type": "input"
            },
            "recent_work": {
                "type": "String",
                "desc": "JSON array of recently completed/worked issue numbers",
                "__dsrs_field_type": "input"
            },
            "user_preferences": {
                "type": "String",
                "desc": "User preferences for issue types and priorities (optional JSON)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "suggestions": {
                "type": "String",
                "desc": "JSON array of top 3 suggested issues with rationale and complexity",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in these suggestions (0.0-1.0)",
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
    fn test_issue_suggestion_signature() {
        let sig = IssueSuggestionSignature::new();

        assert!(!sig.instruction().is_empty());
        assert!(sig.instruction().contains("prioritizing"));

        let inputs = sig.input_fields();
        assert!(inputs.get("available_issues").is_some());
        assert!(inputs.get("workspace_context").is_some());
        assert!(inputs.get("recent_work").is_some());
        assert!(inputs.get("user_preferences").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("suggestions").is_some());
        assert!(outputs.get("confidence").is_some());
    }

    #[test]
    fn test_has_demo() {
        let sig = IssueSuggestionSignature::new();
        let demos = sig.demos();
        assert!(!demos.is_empty());

        let demo = &demos[0];
        assert!(demo.input_keys.contains(&"available_issues".to_string()));
        assert!(demo.output_keys.contains(&"suggestions".to_string()));
    }
}
