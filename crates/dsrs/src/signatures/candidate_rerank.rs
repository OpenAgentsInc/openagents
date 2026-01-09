//! Candidate Reranker Signature.
//!
//! Reranks retrieval candidates by relevance to the user's task.
//! Maps to swarm job `oa.retrieval_rerank.v1`.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A candidate for reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankCandidate {
    /// Unique identifier.
    pub id: String,

    /// Content to evaluate.
    pub content: String,

    /// File path (if applicable).
    pub path: Option<String>,

    /// Original retrieval score.
    pub original_score: f32,

    /// Additional metadata.
    pub metadata: std::collections::HashMap<String, String>,
}

/// A ranked candidate with score and rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCandidate {
    /// Candidate ID.
    pub id: String,

    /// New relevance score (0.0 to 1.0).
    pub score: f32,

    /// Explanation for the ranking.
    pub rationale: String,
}

/// Signature for reranking retrieval candidates.
///
/// # Inputs
/// - `user_task`: The user's task or question
/// - `candidates`: List of candidates to rerank
/// - `k`: Number of top results to return
///
/// # Outputs
/// - `topk`: Ranked list of candidates with scores
/// - `rationale`: Overall explanation of ranking strategy
#[derive(Debug, Clone)]
pub struct CandidateRerankSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for CandidateRerankSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert at evaluating code relevance. Given a user's task and a list
of code candidates, rerank them by relevance.

For each candidate, consider:
1. Direct relevance: Does it directly address the user's task?
2. Contextual relevance: Does it provide useful context or dependencies?
3. Code quality: Is the code well-structured and maintainable?
4. Specificity: Is it the right level of abstraction?

Output the top-k candidates with relevance scores (0.0-1.0) and brief rationales."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl CandidateRerankSignature {
    /// Create a new candidate rerank signature.
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

impl MetaSignature for CandidateRerankSignature {
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
            "user_task": {
                "type": "String",
                "desc": "The user's task or question",
                "__dsrs_field_type": "input"
            },
            "candidates": {
                "type": "Vec<RerankCandidate>",
                "desc": "List of candidates to rerank",
                "__dsrs_field_type": "input"
            },
            "k": {
                "type": "usize",
                "desc": "Number of top results to return",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "topk": {
                "type": "Vec<RankedCandidate>",
                "desc": "Ranked list of top-k candidates with scores",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Overall explanation of ranking strategy",
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
    fn test_candidate_rerank_signature() {
        let sig = CandidateRerankSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("user_task").is_some());
        assert!(inputs.get("candidates").is_some());
        assert!(inputs.get("k").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("topk").is_some());
        assert!(outputs.get("rationale").is_some());
    }

    #[test]
    fn test_rerank_candidate() {
        let candidate = RerankCandidate {
            id: "c1".to_string(),
            content: "fn main() {}".to_string(),
            path: Some("src/main.rs".to_string()),
            original_score: 0.8,
            metadata: std::collections::HashMap::new(),
        };

        assert_eq!(candidate.id, "c1");
        assert_eq!(candidate.original_score, 0.8);
    }
}
