//! DSPy Pipeline wrappers.
//!
//! This module provides ready-to-use pipeline wrappers for DSPy signatures.
//! Pipelines handle LM configuration, input/output serialization, and prediction.

pub mod retrieval;

// Re-export retrieval pipelines
pub use retrieval::{
    CandidateRerankInput, CandidateRerankPipeline, CandidateRerankResult, QueryComposerInput,
    QueryComposerPipeline, QueryComposerResult, RankedCandidate, RerankCandidate,
    RetrievalRouterInput, RetrievalRouterPipeline, RetrievalRouterResult,
};
