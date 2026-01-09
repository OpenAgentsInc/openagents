//! Retrieval Policy Pipelines.
//!
//! Pipeline wrappers for retrieval-related DSPy signatures:
//! - QueryComposerPipeline: Compose search queries from goals
//! - RetrievalRouterPipeline: Route queries to appropriate lanes
//! - CandidateRerankPipeline: Rerank retrieval candidates

use crate::data::example::Example;
use crate::signatures::{
    CandidateRerankSignature, QueryComposerSignature, RetrievalRouterSignature,
};
use crate::{Predict, Predictor, LM, GLOBAL_SETTINGS};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Query Composer Pipeline
// ============================================================================

/// Input for query composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryComposerInput {
    /// The user's task or question.
    pub goal: String,
    /// Previous failed attempts and their reasons.
    pub failure_log: String,
    /// Queries already tried.
    pub previous_queries: Vec<String>,
}

/// Result from query composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryComposerResult {
    /// List of new search queries to try.
    pub queries: Vec<String>,
    /// Suggested retrieval lane for each query.
    pub lanes: Vec<String>,
    /// Explanation of the query strategy.
    pub rationale: String,
    /// Confidence in these queries (0.0-1.0).
    pub confidence: f32,
}

impl Default for QueryComposerResult {
    fn default() -> Self {
        Self {
            queries: vec![],
            lanes: vec![],
            rationale: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered query composition pipeline.
pub struct QueryComposerPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for QueryComposerPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryComposerPipeline {
    /// Create a new query composer pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &crate::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get vec of strings from prediction value.
    fn get_string_vec(prediction: &crate::Prediction, key: &str) -> Vec<String> {
        let val = prediction.get(key, None);
        if let Some(arr) = val.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else if let Some(s) = val.as_str() {
            // Try to parse as JSON array
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &crate::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Compose search queries from a goal.
    pub async fn compose(&self, input: &QueryComposerInput) -> Result<QueryComposerResult> {
        // Check if we have an LM available
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for query composition"));
        }

        let signature = QueryComposerSignature::new();
        let predictor = Predict::new(signature);

        let previous_queries_json = serde_json::to_string(&input.previous_queries)?;

        let mut data = HashMap::new();
        data.insert("goal".to_string(), json!(input.goal));
        data.insert("failure_log".to_string(), json!(input.failure_log));
        data.insert("previous_queries".to_string(), json!(previous_queries_json));

        let example = Example {
            data,
            input_keys: vec!["goal".to_string(), "failure_log".to_string(), "previous_queries".to_string()],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        Ok(QueryComposerResult {
            queries: Self::get_string_vec(&prediction, "queries"),
            lanes: Self::get_string_vec(&prediction, "lanes"),
            rationale: Self::get_string(&prediction, "rationale"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Retrieval Router Pipeline
// ============================================================================

/// Input for retrieval routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalRouterInput {
    /// The search query to route.
    pub query: String,
    /// List of available retrieval lanes.
    pub available_lanes: Vec<String>,
    /// Remaining budget in millisatoshis.
    pub budget_remaining: u64,
}

/// Result from retrieval routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalRouterResult {
    /// The recommended retrieval lane.
    pub lane: String,
    /// Number of results to fetch.
    pub k: usize,
    /// Explanation of the routing decision.
    pub rationale: String,
    /// Confidence in this routing (0.0-1.0).
    pub confidence: f32,
}

impl Default for RetrievalRouterResult {
    fn default() -> Self {
        Self {
            lane: "ripgrep".to_string(),
            k: 10,
            rationale: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered retrieval routing pipeline.
pub struct RetrievalRouterPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for RetrievalRouterPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl RetrievalRouterPipeline {
    /// Create a new retrieval router pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &crate::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get usize from prediction value.
    fn get_usize(prediction: &crate::Prediction, key: &str) -> usize {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_u64() {
            n as usize
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(10)
        } else {
            10
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &crate::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Route a query to the appropriate retrieval lane.
    pub async fn route(&self, input: &RetrievalRouterInput) -> Result<RetrievalRouterResult> {
        // Check if we have an LM available
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for retrieval routing"));
        }

        let signature = RetrievalRouterSignature::new();
        let predictor = Predict::new(signature);

        let available_lanes_json = serde_json::to_string(&input.available_lanes)?;

        let mut data = HashMap::new();
        data.insert("query".to_string(), json!(input.query));
        data.insert("available_lanes".to_string(), json!(available_lanes_json));
        data.insert("budget_remaining".to_string(), json!(input.budget_remaining.to_string()));

        let example = Example {
            data,
            input_keys: vec!["query".to_string(), "available_lanes".to_string(), "budget_remaining".to_string()],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        Ok(RetrievalRouterResult {
            lane: Self::get_string(&prediction, "lane"),
            k: Self::get_usize(&prediction, "k"),
            rationale: Self::get_string(&prediction, "rationale"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Candidate Rerank Pipeline
// ============================================================================

/// A candidate for reranking (re-exported from signature).
pub use crate::signatures::candidate_rerank::{RankedCandidate, RerankCandidate};

/// Input for candidate reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRerankInput {
    /// The user's task or question.
    pub user_task: String,
    /// List of candidates to rerank.
    pub candidates: Vec<RerankCandidate>,
    /// Number of top results to return.
    pub k: usize,
}

/// Result from candidate reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRerankResult {
    /// Ranked list of top-k candidates with scores.
    pub topk: Vec<RankedCandidate>,
    /// Overall explanation of ranking strategy.
    pub rationale: String,
    /// Confidence in this ranking (0.0-1.0).
    pub confidence: f32,
}

impl Default for CandidateRerankResult {
    fn default() -> Self {
        Self {
            topk: vec![],
            rationale: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered candidate reranking pipeline.
pub struct CandidateRerankPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for CandidateRerankPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl CandidateRerankPipeline {
    /// Create a new candidate rerank pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &crate::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &crate::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Helper to parse ranked candidates from prediction.
    fn get_ranked_candidates(prediction: &crate::Prediction, key: &str) -> Vec<RankedCandidate> {
        let val = prediction.get(key, None);
        if let Some(arr) = val.as_array() {
            arr.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        } else if let Some(s) = val.as_str() {
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        }
    }

    /// Rerank candidates by relevance to the user's task.
    pub async fn rerank(&self, input: &CandidateRerankInput) -> Result<CandidateRerankResult> {
        // Check if we have an LM available
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for candidate reranking"));
        }

        let signature = CandidateRerankSignature::new();
        let predictor = Predict::new(signature);

        let candidates_json = serde_json::to_string(&input.candidates)?;

        let mut data = HashMap::new();
        data.insert("user_task".to_string(), json!(input.user_task));
        data.insert("candidates".to_string(), json!(candidates_json));
        data.insert("k".to_string(), json!(input.k.to_string()));

        let example = Example {
            data,
            input_keys: vec!["user_task".to_string(), "candidates".to_string(), "k".to_string()],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        Ok(CandidateRerankResult {
            topk: Self::get_ranked_candidates(&prediction, "topk"),
            rationale: Self::get_string(&prediction, "rationale"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_composer_input_serialization() {
        let input = QueryComposerInput {
            goal: "Find the auth handler".to_string(),
            failure_log: "".to_string(),
            previous_queries: vec!["auth".to_string()],
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: QueryComposerInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.goal, input.goal);
    }

    #[test]
    fn test_query_composer_result_default() {
        let result = QueryComposerResult::default();
        assert!(result.queries.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_retrieval_router_input_serialization() {
        let input = RetrievalRouterInput {
            query: "handleAuth".to_string(),
            available_lanes: vec!["ripgrep".to_string(), "lsp".to_string()],
            budget_remaining: 10000,
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: RetrievalRouterInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query, input.query);
    }

    #[test]
    fn test_retrieval_router_result_default() {
        let result = RetrievalRouterResult::default();
        assert_eq!(result.lane, "ripgrep");
        assert_eq!(result.k, 10);
    }

    #[test]
    fn test_candidate_rerank_input_serialization() {
        let input = CandidateRerankInput {
            user_task: "Fix the login bug".to_string(),
            candidates: vec![RerankCandidate {
                id: "c1".to_string(),
                content: "fn login() {}".to_string(),
                path: Some("src/auth.rs".to_string()),
                original_score: 0.9,
                metadata: std::collections::HashMap::new(),
            }],
            k: 5,
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: CandidateRerankInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_task, input.user_task);
    }

    #[test]
    fn test_candidate_rerank_result_default() {
        let result = CandidateRerankResult::default();
        assert!(result.topk.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_query_composer_pipeline_creation() {
        let pipeline = QueryComposerPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_retrieval_router_pipeline_creation() {
        let pipeline = RetrievalRouterPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_candidate_rerank_pipeline_creation() {
        let pipeline = CandidateRerankPipeline::new();
        assert!(pipeline.lm.is_none());
    }
}
