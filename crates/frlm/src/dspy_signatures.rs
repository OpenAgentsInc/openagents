//! DSPy FRLM Signatures (Wave 12).
//!
//! FRLM-specific signatures for the flagship Federated Recursive Language Models.
//! These signatures enable learned, optimizable decomposition and aggregation.
//!
//! # Signatures
//!
//! - [`FRLMDecomposeSignature`] - Root decides what subcalls to spawn over which spans
//! - [`FRLMAggregateSignature`] - Reduce step: merge worker results into final answer

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{Value, json};

// ============================================================================
// StoppingRule Enum
// ============================================================================

/// Rule for when to stop recursive decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoppingRule {
    /// Process all spans exhaustively.
    Exhaustive,
    /// Stop when sufficient evidence is found.
    SufficientEvidence,
    /// Stop when budget is exhausted.
    BudgetExhausted,
    /// Stop when confidence exceeds threshold.
    ConfidenceThreshold,
}

impl std::fmt::Display for StoppingRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoppingRule::Exhaustive => write!(f, "EXHAUSTIVE"),
            StoppingRule::SufficientEvidence => write!(f, "SUFFICIENT_EVIDENCE"),
            StoppingRule::BudgetExhausted => write!(f, "BUDGET_EXHAUSTED"),
            StoppingRule::ConfidenceThreshold => write!(f, "CONFIDENCE_THRESHOLD"),
        }
    }
}

impl std::str::FromStr for StoppingRule {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().replace('-', "_").as_str() {
            "EXHAUSTIVE" | "ALL" | "COMPLETE" => Ok(StoppingRule::Exhaustive),
            "SUFFICIENT_EVIDENCE" | "ENOUGH" | "FOUND" => Ok(StoppingRule::SufficientEvidence),
            "BUDGET_EXHAUSTED" | "BUDGET" | "COST" => Ok(StoppingRule::BudgetExhausted),
            "CONFIDENCE_THRESHOLD" | "CONFIDENCE" | "THRESHOLD" => {
                Ok(StoppingRule::ConfidenceThreshold)
            }
            _ => Err(format!("Unknown stopping rule: {}", s)),
        }
    }
}

// ============================================================================
// SpanSelector Enum
// ============================================================================

/// Selector for which spans to process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanSelector {
    /// Select all spans.
    All,
    /// Select spans by content type.
    ByType(String),
    /// Select most relevant spans based on query.
    ByRelevance,
    /// Select spans by position range.
    ByPosition { start: usize, end: usize },
}

impl std::fmt::Display for SpanSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanSelector::All => write!(f, "ALL"),
            SpanSelector::ByType(t) => write!(f, "TYPE:{}", t),
            SpanSelector::ByRelevance => write!(f, "RELEVANCE"),
            SpanSelector::ByPosition { start, end } => write!(f, "POSITION:{}:{}", start, end),
        }
    }
}

impl std::str::FromStr for SpanSelector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        if upper == "ALL" || upper == "*" {
            Ok(SpanSelector::All)
        } else if upper == "RELEVANCE" || upper == "RELEVANT" {
            Ok(SpanSelector::ByRelevance)
        } else if upper.starts_with("TYPE:") {
            let t = s[5..].to_string();
            Ok(SpanSelector::ByType(t))
        } else if upper.starts_with("POSITION:") {
            let parts: Vec<&str> = s[9..].split(':').collect();
            if parts.len() == 2 {
                let start = parts[0]
                    .parse()
                    .map_err(|_| format!("Invalid start position: {}", parts[0]))?;
                let end = parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid end position: {}", parts[1]))?;
                Ok(SpanSelector::ByPosition { start, end })
            } else {
                Err(format!("Invalid position format: {}", s))
            }
        } else {
            Err(format!("Unknown span selector: {}", s))
        }
    }
}

// ============================================================================
// FRLMDecomposeSignature
// ============================================================================

/// FRLM Decompose Signature.
///
/// Root decides what subcalls to spawn over which spans.
/// This is the "map" phase of the FRLM map-reduce pattern.
///
/// # Inputs
/// - `query`: The user query to process
/// - `env_summary`: Summary of the environment/document
/// - `progress`: Current progress state (what's been done)
///
/// # Outputs
/// - `subqueries`: JSON array of [{span_selector, question, schema}]
/// - `stopping_rule`: When to stop recursing
#[derive(Debug, Clone)]
pub struct FRLMDecomposeSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for FRLMDecomposeSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an FRLM decomposition expert. Break down the query into parallel subqueries.

Given:
- A user query to answer
- A summary of the environment (documents, fragments available)
- Current progress (what's already been processed)

Generate subqueries that can be executed in parallel across distributed workers.

Decomposition rules:
1. Each subquery should target a specific span or set of spans
2. Subqueries should be independent (no dependencies between them)
3. Use span selectors to identify which fragments to query
4. Choose a stopping rule based on query type:
   - EXHAUSTIVE for comprehensive analysis
   - SUFFICIENT_EVIDENCE for fact-finding
   - BUDGET_EXHAUSTED for cost-sensitive queries
   - CONFIDENCE_THRESHOLD for iterative refinement

5. Include a schema for each subquery's expected output format
6. Consider budget and latency constraints

Output subqueries as JSON array with fields:
- span_selector: Which spans to process
- question: The specific question for that span
- schema: Expected output format (JSON schema)"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl FRLMDecomposeSignature {
    /// Create a new FRLM decompose signature.
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

impl MetaSignature for FRLMDecomposeSignature {
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
                "desc": "The user query to process",
                "__dsrs_field_type": "input"
            },
            "env_summary": {
                "type": "String",
                "desc": "Summary of the environment/document (available fragments, structure)",
                "__dsrs_field_type": "input"
            },
            "progress": {
                "type": "String",
                "desc": "Current progress state - what fragments have been processed, findings so far",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "subqueries": {
                "type": "String",
                "desc": "JSON array of subqueries: [{span_selector, question, schema}]",
                "__dsrs_field_type": "output"
            },
            "stopping_rule": {
                "type": "String",
                "desc": "When to stop recursing: EXHAUSTIVE, SUFFICIENT_EVIDENCE, BUDGET_EXHAUSTED, CONFIDENCE_THRESHOLD",
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

// ============================================================================
// FRLMAggregateSignature
// ============================================================================

/// FRLM Aggregate Signature.
///
/// Reduce step: merge worker results into final answer.
/// This is the "reduce" phase of the FRLM map-reduce pattern.
///
/// # Inputs
/// - `query`: The original user query
/// - `worker_results`: JSON array of worker outputs
///
/// # Outputs
/// - `answer`: The aggregated answer
/// - `citations`: SpanRefs or doc IDs for evidence
/// - `confidence`: Confidence score (0.0-1.0)
#[derive(Debug, Clone)]
pub struct FRLMAggregateSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for FRLMAggregateSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an FRLM aggregation expert. Merge distributed worker results into a final answer.

Given:
- The original user query
- Results from parallel worker executions (JSON array)

Synthesize a comprehensive answer that:
1. Integrates findings from all workers
2. Resolves any conflicts between worker outputs
3. Provides citations to source spans
4. Assigns a confidence score based on evidence quality

Aggregation rules:
1. Weight results by worker confidence scores
2. Prefer consensus findings over outliers
3. Include citations for all claims using SpanRef format
4. Flag any contradictions or gaps in evidence
5. Set confidence based on:
   - Coverage: How many relevant spans were processed?
   - Consensus: Do workers agree?
   - Evidence quality: Are citations specific?

Confidence scoring:
- 0.9+: Strong consensus, comprehensive coverage, specific citations
- 0.7-0.9: Good evidence with minor gaps
- 0.5-0.7: Mixed evidence or incomplete coverage
- <0.5: Insufficient evidence or major conflicts"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl FRLMAggregateSignature {
    /// Create a new FRLM aggregate signature.
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

impl MetaSignature for FRLMAggregateSignature {
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
                "desc": "The original user query",
                "__dsrs_field_type": "input"
            },
            "worker_results": {
                "type": "String",
                "desc": "JSON array of worker outputs with their findings, span IDs, and confidence scores",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "answer": {
                "type": "String",
                "desc": "The aggregated answer synthesized from worker results",
                "__dsrs_field_type": "output"
            },
            "citations": {
                "type": "String",
                "desc": "JSON array of SpanRefs or doc IDs providing evidence for the answer",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "String",
                "desc": "Confidence score from 0.0 to 1.0 based on evidence quality and coverage",
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // StoppingRule tests

    #[test]
    fn test_stopping_rule_display() {
        assert_eq!(StoppingRule::Exhaustive.to_string(), "EXHAUSTIVE");
        assert_eq!(
            StoppingRule::SufficientEvidence.to_string(),
            "SUFFICIENT_EVIDENCE"
        );
        assert_eq!(
            StoppingRule::BudgetExhausted.to_string(),
            "BUDGET_EXHAUSTED"
        );
        assert_eq!(
            StoppingRule::ConfidenceThreshold.to_string(),
            "CONFIDENCE_THRESHOLD"
        );
    }

    #[test]
    fn test_stopping_rule_from_str() {
        assert_eq!(
            "EXHAUSTIVE".parse::<StoppingRule>().unwrap(),
            StoppingRule::Exhaustive
        );
        assert_eq!(
            "ALL".parse::<StoppingRule>().unwrap(),
            StoppingRule::Exhaustive
        );
        assert_eq!(
            "sufficient_evidence".parse::<StoppingRule>().unwrap(),
            StoppingRule::SufficientEvidence
        );
        assert_eq!(
            "BUDGET".parse::<StoppingRule>().unwrap(),
            StoppingRule::BudgetExhausted
        );
        assert_eq!(
            "confidence-threshold".parse::<StoppingRule>().unwrap(),
            StoppingRule::ConfidenceThreshold
        );
        assert!("INVALID".parse::<StoppingRule>().is_err());
    }

    // SpanSelector tests

    #[test]
    fn test_span_selector_display() {
        assert_eq!(SpanSelector::All.to_string(), "ALL");
        assert_eq!(SpanSelector::ByRelevance.to_string(), "RELEVANCE");
        assert_eq!(
            SpanSelector::ByType("code".to_string()).to_string(),
            "TYPE:code"
        );
        assert_eq!(
            SpanSelector::ByPosition { start: 0, end: 10 }.to_string(),
            "POSITION:0:10"
        );
    }

    #[test]
    fn test_span_selector_from_str() {
        assert_eq!("ALL".parse::<SpanSelector>().unwrap(), SpanSelector::All);
        assert_eq!("*".parse::<SpanSelector>().unwrap(), SpanSelector::All);
        assert_eq!(
            "RELEVANCE".parse::<SpanSelector>().unwrap(),
            SpanSelector::ByRelevance
        );
        assert_eq!(
            "TYPE:markdown".parse::<SpanSelector>().unwrap(),
            SpanSelector::ByType("markdown".to_string())
        );
        assert_eq!(
            "POSITION:5:15".parse::<SpanSelector>().unwrap(),
            SpanSelector::ByPosition { start: 5, end: 15 }
        );
        assert!("INVALID".parse::<SpanSelector>().is_err());
    }

    // FRLMDecomposeSignature tests

    #[test]
    fn test_decompose_signature_default() {
        let sig = FRLMDecomposeSignature::new();
        assert!(sig.instruction().contains("decomposition"));
        assert!(sig.demos().is_empty());
    }

    #[test]
    fn test_decompose_signature_with_instruction() {
        let sig = FRLMDecomposeSignature::new().with_instruction("Custom instruction");
        assert_eq!(sig.instruction(), "Custom instruction");
    }

    #[test]
    fn test_decompose_input_fields() {
        let sig = FRLMDecomposeSignature::new();
        let inputs = sig.input_fields();
        assert!(inputs.get("query").is_some());
        assert!(inputs.get("env_summary").is_some());
        assert!(inputs.get("progress").is_some());
    }

    #[test]
    fn test_decompose_output_fields() {
        let sig = FRLMDecomposeSignature::new();
        let outputs = sig.output_fields();
        assert!(outputs.get("subqueries").is_some());
        assert!(outputs.get("stopping_rule").is_some());
    }

    // FRLMAggregateSignature tests

    #[test]
    fn test_aggregate_signature_default() {
        let sig = FRLMAggregateSignature::new();
        assert!(sig.instruction().contains("aggregation"));
        assert!(sig.demos().is_empty());
    }

    #[test]
    fn test_aggregate_signature_with_instruction() {
        let sig = FRLMAggregateSignature::new().with_instruction("Custom aggregation");
        assert_eq!(sig.instruction(), "Custom aggregation");
    }

    #[test]
    fn test_aggregate_input_fields() {
        let sig = FRLMAggregateSignature::new();
        let inputs = sig.input_fields();
        assert!(inputs.get("query").is_some());
        assert!(inputs.get("worker_results").is_some());
    }

    #[test]
    fn test_aggregate_output_fields() {
        let sig = FRLMAggregateSignature::new();
        let outputs = sig.output_fields();
        assert!(outputs.get("answer").is_some());
        assert!(outputs.get("citations").is_some());
        assert!(outputs.get("confidence").is_some());
    }

    // MetaSignature implementation tests

    #[test]
    fn test_set_demos_decompose() {
        let mut sig = FRLMDecomposeSignature::new();
        assert!(sig.demos().is_empty());

        let demo = Example::new(
            std::collections::HashMap::new(),
            vec!["query".to_string()],
            vec!["subqueries".to_string()],
        );
        sig.set_demos(vec![demo]).unwrap();
        assert_eq!(sig.demos().len(), 1);
    }

    #[test]
    fn test_set_demos_aggregate() {
        let mut sig = FRLMAggregateSignature::new();
        assert!(sig.demos().is_empty());

        let demo = Example::new(
            std::collections::HashMap::new(),
            vec!["query".to_string()],
            vec!["answer".to_string()],
        );
        sig.set_demos(vec![demo]).unwrap();
        assert_eq!(sig.demos().len(), 1);
    }
}
