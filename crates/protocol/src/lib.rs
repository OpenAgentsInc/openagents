//! Protocol layer for typed job schemas with deterministic hashing.
//!
//! This crate provides the foundation for the OpenAgents swarm protocol:
//!
//! - **Typed job schemas**: Every job type has a well-defined request/response structure
//! - **Deterministic hashing**: Canonical JSON serialization ensures identical inputs
//!   produce identical hashes across all implementations
//! - **Verification modes**: Jobs can be objective (deterministic) or subjective
//!   (requiring judgment)
//! - **Provenance tracking**: Full audit trail of model, parameters, and hashes
//!
//! # Job Types
//!
//! | Job Type | Description | Verification |
//! |----------|-------------|--------------|
//! | `oa.code_chunk_analysis.v1` | Analyze code chunks | Subjective + Judge |
//! | `oa.retrieval_rerank.v1` | Rerank retrieval candidates | Subjective + Majority |
//! | `oa.sandbox_run.v1` | Run commands in sandbox | Objective |
//!
//! # Example
//!
//! ```
//! use protocol::jobs::{ChunkAnalysisRequest, chunk_analysis::CodeChunk, JobEnvelope, JobRequest};
//! use protocol::hash::canonical_hash;
//!
//! // Create a typed job request
//! let request = ChunkAnalysisRequest {
//!     task: "Understand this code".into(),
//!     chunk: CodeChunk::new("src/lib.rs", 1, 50, "fn main() { }"),
//!     ..Default::default()
//! };
//!
//! // Compute deterministic hash
//! let hash = request.compute_hash().unwrap();
//! assert_eq!(hash.len(), 64); // SHA-256 hex
//!
//! // Wrap in envelope for transport
//! let envelope = JobEnvelope::from_request(request);
//! assert_eq!(envelope.job_type, "oa.code_chunk_analysis.v1");
//! ```

pub mod hash;
pub mod jobs;
pub mod provenance;
pub mod verification;
pub mod version;

// Re-export commonly used types at crate root
pub use hash::{HashError, Hashable, canonical_hash, canonical_json};
pub use jobs::{
    ChunkAnalysisRequest, ChunkAnalysisResponse, JobEnvelope, JobRequest, JobResponse,
    RerankRequest, RerankResponse, SandboxRunRequest, SandboxRunResponse,
};
pub use provenance::{Provenance, SamplingParams, TokenCounts};
pub use verification::{AdjudicationStrategy, Verification, VerificationMode};
pub use version::{SchemaVersion, VersionError};
