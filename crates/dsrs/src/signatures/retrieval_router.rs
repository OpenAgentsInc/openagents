//! Retrieval Router Signature.
//!
//! Decides which retrieval lane to use and how many results to fetch.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{json, Value};

/// Signature for routing retrieval queries to appropriate lanes.
///
/// # Inputs
/// - `query`: The search query
/// - `available_lanes`: List of available retrieval lanes
/// - `budget_remaining`: Remaining budget in millisatoshis
///
/// # Outputs
/// - `lane`: The recommended retrieval lane
/// - `k`: Number of results to fetch
/// - `rationale`: Explanation of the routing decision
#[derive(Debug, Clone)]
pub struct RetrievalRouterSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for RetrievalRouterSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are a retrieval routing expert. Given a query and available retrieval lanes,
decide which lane to use and how many results to fetch.

Available lanes and their characteristics:
- ripgrep: Fast text/regex search. Best for exact strings, error messages, identifiers.
- lsp: Symbol-aware search. Best for function/class/type definitions and references.
- semantic: Vector similarity search. Best for conceptual queries, natural language.
- git: Git history search. Best for finding who changed what, recent modifications.

Consider:
1. Query type: Is it an exact string, symbol name, or conceptual?
2. Budget: Semantic search may cost more than local search.
3. Specificity: How many results are likely useful?

Output the best lane, optimal k value, and your reasoning."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl RetrievalRouterSignature {
    /// Create a new retrieval router signature.
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

impl MetaSignature for RetrievalRouterSignature {
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
            "query": {
                "type": "String",
                "desc": "The search query to route",
                "__dsrs_field_type": "input"
            },
            "available_lanes": {
                "type": "Vec<String>",
                "desc": "List of available retrieval lanes",
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
            "lane": {
                "type": "String",
                "desc": "The recommended retrieval lane",
                "__dsrs_field_type": "output"
            },
            "k": {
                "type": "usize",
                "desc": "Number of results to fetch",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of the routing decision",
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
    fn test_retrieval_router_signature() {
        let sig = RetrievalRouterSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("query").is_some());
        assert!(inputs.get("available_lanes").is_some());
        assert!(inputs.get("budget_remaining").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("lane").is_some());
        assert!(outputs.get("k").is_some());
        assert!(outputs.get("rationale").is_some());
    }
}
