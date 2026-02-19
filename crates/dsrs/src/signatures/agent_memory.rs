//! Agent Memory Signature.
//!
//! Detects redundant probes and suggests alternatives.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A previous query and its results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    /// The query that was executed.
    pub query: String,

    /// The retrieval lane used.
    pub lane: String,

    /// Number of results returned.
    pub result_count: usize,

    /// Whether the query was useful.
    pub was_useful: bool,

    /// Timestamp of the query.
    pub timestamp: u64,
}

/// Memory analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    /// Whether the proposed query is redundant.
    pub is_redundant: bool,

    /// Similarity score to existing queries (0.0-1.0).
    pub similarity_score: f32,

    /// The most similar previous query.
    pub similar_query: Option<String>,

    /// Why it's considered redundant (if applicable).
    pub redundancy_reason: Option<String>,
}

/// Signature for detecting redundant queries and suggesting alternatives.
///
/// # Inputs
/// - `proposed_query`: The query about to be executed
/// - `proposed_lane`: The lane about to be used
/// - `query_history`: List of previous queries
/// - `results_history`: Summary of what was found
///
/// # Outputs
/// - `analysis`: Analysis of potential redundancy
/// - `alternative_query`: Suggested alternative if redundant
/// - `should_proceed`: Whether to proceed with the query
/// - `rationale`: Explanation of the decision
#[derive(Debug, Clone)]
pub struct AgentMemorySignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for AgentMemorySignature {
    fn default() -> Self {
        Self {
            instruction:
                r#"You are an expert at optimizing agent exploration. Given a proposed query and
the history of previous queries, determine if it would be redundant.

A query is redundant if:
1. It's semantically equivalent to a previous query
2. Its results would be a subset of previous results
3. It searches the same content with different syntax
4. It's unlikely to find new information

Consider:
- Query similarity (exact match, synonym, subset)
- Lane differences (same query on different lanes may yield different results)
- Time sensitivity (old queries may be worth re-running)
- Result coverage (previous queries may have found everything)

If redundant, suggest an alternative that would find new information."#
                    .to_string(),
            demos: vec![],
        }
    }
}

impl AgentMemorySignature {
    /// Create a new agent memory signature.
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

impl MetaSignature for AgentMemorySignature {
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
            "proposed_query": {
                "type": "String",
                "desc": "The query about to be executed",
                "__dsrs_field_type": "input"
            },
            "proposed_lane": {
                "type": "String",
                "desc": "The retrieval lane about to be used",
                "__dsrs_field_type": "input"
            },
            "query_history": {
                "type": "Vec<QueryHistoryEntry>",
                "desc": "List of previous queries and their metadata",
                "__dsrs_field_type": "input"
            },
            "results_summary": {
                "type": "String",
                "desc": "Summary of what has been found so far",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "analysis": {
                "type": "MemoryAnalysis",
                "desc": "Analysis of potential redundancy",
                "__dsrs_field_type": "output"
            },
            "alternative_query": {
                "type": "String",
                "desc": "Suggested alternative query if redundant (optional)",
                "__dsrs_field_type": "output"
            },
            "should_proceed": {
                "type": "bool",
                "desc": "Whether to proceed with the original query",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of the decision",
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

/// Helper to check simple query redundancy.
pub fn is_simple_redundant(proposed: &str, history: &[QueryHistoryEntry]) -> bool {
    let proposed_lower = proposed.to_lowercase();

    for entry in history {
        let entry_lower = entry.query.to_lowercase();

        // Exact match
        if proposed_lower == entry_lower {
            return true;
        }

        // Substring match (proposed is subset of previous)
        if entry_lower.contains(&proposed_lower) {
            return true;
        }
    }

    false
}

/// Calculate simple text similarity (Jaccard on words).
pub fn simple_similarity(a: &str, b: &str) -> f32 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_memory_signature() {
        let sig = AgentMemorySignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("proposed_query").is_some());
        assert!(inputs.get("proposed_lane").is_some());
        assert!(inputs.get("query_history").is_some());
        assert!(inputs.get("results_summary").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("analysis").is_some());
        assert!(outputs.get("alternative_query").is_some());
        assert!(outputs.get("should_proceed").is_some());
        assert!(outputs.get("rationale").is_some());
    }

    #[test]
    fn test_simple_redundant() {
        let history = vec![
            QueryHistoryEntry {
                query: "fn main".to_string(),
                lane: "ripgrep".to_string(),
                result_count: 5,
                was_useful: true,
                timestamp: 0,
            },
            QueryHistoryEntry {
                query: "struct Config".to_string(),
                lane: "lsp".to_string(),
                result_count: 2,
                was_useful: true,
                timestamp: 0,
            },
        ];

        assert!(is_simple_redundant("fn main", &history));
        assert!(is_simple_redundant("FN MAIN", &history)); // Case insensitive
        assert!(!is_simple_redundant("fn helper", &history));
    }

    #[test]
    fn test_simple_similarity() {
        let sim = simple_similarity("hello world test", "hello world");
        assert!(sim > 0.5);

        let sim = simple_similarity("foo bar", "baz qux");
        assert!(sim < 0.1);

        let sim = simple_similarity("hello world", "hello world");
        assert!((sim - 1.0).abs() < 0.001);
    }
}
