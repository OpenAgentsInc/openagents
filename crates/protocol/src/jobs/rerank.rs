//! Retrieval reranking job type (`oa.retrieval_rerank.v1`).
//!
//! This job type is used for reranking retrieval candidates by relevance
//! to a user's task. It takes a list of candidates and returns the top-k
//! ranked by a model.

use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

use super::{JobRequest, JobResponse};

/// A candidate document/chunk for reranking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankCandidate {
    /// Unique identifier for the candidate.
    pub id: String,

    /// The content to rank.
    pub content: String,

    /// Optional: file path for code candidates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Optional: original retrieval score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_score: Option<f32>,

    /// Optional: additional metadata.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

impl RerankCandidate {
    /// Create a new rerank candidate.
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            path: None,
            original_score: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set the file path.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set the original retrieval score.
    pub fn with_score(mut self, score: f32) -> Self {
        self.original_score = Some(score);
        self
    }
}

/// Request for retrieval reranking.
///
/// # Example
///
/// ```
/// use protocol::jobs::{RerankRequest, rerank::RerankCandidate};
///
/// let request = RerankRequest {
///     user_task: "Find authentication code".into(),
///     candidates: vec![
///         RerankCandidate::new("1", "def login(): ...").with_path("auth.py"),
///         RerankCandidate::new("2", "def logout(): ...").with_path("auth.py"),
///     ],
///     k: 5,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankRequest {
    /// The user's task/query for ranking relevance.
    pub user_task: String,

    /// Candidates to rerank.
    pub candidates: Vec<RerankCandidate>,

    /// Number of top candidates to return.
    #[serde(default = "default_k")]
    pub k: usize,

    /// Optional: rubric for ranking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_rubric: Option<String>,

    /// Verification settings.
    #[serde(default = "default_verification")]
    pub verification: Verification,
}

fn default_k() -> usize {
    10
}

fn default_verification() -> Verification {
    Verification::subjective_with_majority(2)
}

impl Default for RerankRequest {
    fn default() -> Self {
        Self {
            user_task: String::new(),
            candidates: Vec::new(),
            k: default_k(),
            ranking_rubric: None,
            verification: default_verification(),
        }
    }
}

impl JobRequest for RerankRequest {
    const JOB_TYPE: &'static str = "oa.retrieval_rerank.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}

/// A ranked candidate in the response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedCandidate {
    /// Rank position (1-indexed).
    pub rank: usize,

    /// Candidate ID (matches input).
    pub id: String,

    /// Relevance score (0.0 to 1.0).
    pub score: f32,

    /// Explanation for the ranking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// Response from retrieval reranking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankResponse {
    /// Top-k ranked candidates.
    pub topk: Vec<RankedCandidate>,

    /// Provenance information.
    pub provenance: Provenance,
}

impl JobResponse for RerankResponse {
    type Request = RerankRequest;

    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobEnvelope;

    #[test]
    fn test_rerank_request_hash() {
        let request = RerankRequest {
            user_task: "Find auth code".into(),
            candidates: vec![
                RerankCandidate::new("1", "content1"),
                RerankCandidate::new("2", "content2"),
            ],
            k: 5,
            ..Default::default()
        };

        let hash1 = request.compute_hash().unwrap();
        let hash2 = request.compute_hash().unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_job_type_constant() {
        assert_eq!(RerankRequest::JOB_TYPE, "oa.retrieval_rerank.v1");
        assert_eq!(RerankRequest::SCHEMA_VERSION, SchemaVersion::new(1, 0, 0));
    }

    #[test]
    fn test_default_verification_is_majority() {
        let request = RerankRequest::default();
        assert_eq!(
            request.verification.adjudication,
            crate::verification::AdjudicationStrategy::MajorityVote
        );
    }

    #[test]
    fn test_request_serde() {
        let request = RerankRequest {
            user_task: "Find relevant code".into(),
            candidates: vec![
                RerankCandidate::new("c1", "code1").with_path("file1.rs").with_score(0.8),
                RerankCandidate::new("c2", "code2").with_path("file2.rs"),
            ],
            k: 3,
            ranking_rubric: Some("Prefer direct matches".into()),
            verification: Verification::subjective_with_majority(3),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: RerankRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_response_serde() {
        let response = RerankResponse {
            topk: vec![
                RankedCandidate {
                    rank: 1,
                    id: "c1".into(),
                    score: 0.95,
                    why: Some("Exact match for auth".into()),
                },
                RankedCandidate {
                    rank: 2,
                    id: "c2".into(),
                    score: 0.7,
                    why: None,
                },
            ],
            provenance: Provenance::new("gpt-4"),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: RerankResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, parsed);
    }

    #[test]
    fn test_envelope_integration() {
        let request = RerankRequest {
            user_task: "Test".into(),
            candidates: vec![RerankCandidate::new("1", "test")],
            ..Default::default()
        };

        let envelope = JobEnvelope::from_request(request);
        assert_eq!(envelope.job_type, "oa.retrieval_rerank.v1");
    }
}
