//! Query Composer Signature.
//!
//! Transforms user goals and failure logs into targeted search queries.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for composing retrieval queries from goals and context.
///
/// # Inputs
/// - `goal`: The user's task or question
/// - `failure_log`: Previous failed attempts and their reasons
/// - `previous_queries`: Queries already tried
///
/// # Outputs
/// - `queries`: List of new search queries to try
/// - `lanes`: Suggested retrieval lanes for each query
/// - `rationale`: Explanation of query strategy
#[derive(Debug, Clone)]
pub struct QueryComposerSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for QueryComposerSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert at composing search queries for code exploration.
Given a user's goal and any previous failed attempts, generate targeted search queries
that will help locate relevant code.

Consider:
1. Break complex goals into specific, searchable terms
2. Use different query strategies (function names, error messages, types, etc.)
3. Account for what queries have already been tried
4. Suggest appropriate retrieval lanes (ripgrep for text, lsp for symbols, semantic for concepts)

Output a list of queries with their recommended lanes."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl QueryComposerSignature {
    /// Create a new query composer signature.
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

impl MetaSignature for QueryComposerSignature {
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
            "goal": {
                "type": "String",
                "desc": "The user's task or question to solve",
                "__dsrs_field_type": "input"
            },
            "failure_log": {
                "type": "String",
                "desc": "Previous failed attempts and their reasons (optional)",
                "__dsrs_field_type": "input"
            },
            "previous_queries": {
                "type": "Vec<String>",
                "desc": "Queries already tried (optional)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "queries": {
                "type": "Vec<String>",
                "desc": "List of new search queries to try",
                "__dsrs_field_type": "output"
            },
            "lanes": {
                "type": "Vec<String>",
                "desc": "Suggested retrieval lane for each query (ripgrep, lsp, semantic, git)",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of the query strategy",
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
    fn test_query_composer_signature() {
        let sig = QueryComposerSignature::new();

        assert!(!sig.instruction().is_empty());

        let inputs = sig.input_fields();
        assert!(inputs.get("goal").is_some());
        assert!(inputs.get("failure_log").is_some());
        assert!(inputs.get("previous_queries").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("queries").is_some());
        assert!(outputs.get("lanes").is_some());
        assert!(outputs.get("rationale").is_some());
    }

    #[test]
    fn test_custom_instruction() {
        let sig = QueryComposerSignature::new()
            .with_instruction("Custom instruction for query composition");

        assert!(sig.instruction().contains("Custom instruction"));
    }
}
