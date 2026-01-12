//! DSPy Pipeline wrappers.
//!
//! This module provides ready-to-use pipeline wrappers for DSPy signatures.
//! Pipelines handle LM configuration, input/output serialization, and prediction.

pub mod code_change;
pub mod retrieval;

// Re-export retrieval pipelines
pub use retrieval::{
    CandidateRerankInput, CandidateRerankPipeline, CandidateRerankResult, QueryComposerInput,
    QueryComposerPipeline, QueryComposerResult, RankedCandidate, RerankCandidate,
    RetrievalRouterInput, RetrievalRouterPipeline, RetrievalRouterResult,
};

// Re-export code change pipeline
pub use code_change::{
    CodeChangeInput, CodeChangePipeline, CodeChangeResult, CodeEdit, CodeExplorationResult, Scope,
    TaskType, TaskUnderstandingResult, VerificationResult, VerificationStatus,
};
