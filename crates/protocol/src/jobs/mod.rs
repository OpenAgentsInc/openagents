//! Job request and response types for the protocol.
//!
//! Every job in the swarm has:
//! - A **job type** (e.g., `oa.code_chunk_analysis.v1`)
//! - A **schema version** (semver, e.g., `1.0.0`)
//! - A **request** with typed input and verification settings
//! - A **response** with typed output and provenance
//!
//! The [`JobRequest`] and [`JobResponse`] traits define the common interface.

pub mod chunk_analysis;
pub mod embeddings;
pub mod rerank;
pub mod sandbox;

pub use chunk_analysis::{ChunkAnalysisRequest, ChunkAnalysisResponse};
pub use embeddings::{EmbeddingsRequest, EmbeddingsResponse};
pub use rerank::{RerankRequest, RerankResponse};
pub use sandbox::{SandboxRunRequest, SandboxRunResponse};

use crate::hash::{HashError, canonical_hash};
use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Trait for job request types.
///
/// Every job request has a static job type, schema version, and verification settings.
/// The request can compute its canonical hash for deterministic identification.
pub trait JobRequest: Serialize + DeserializeOwned + Clone {
    /// The job type identifier (e.g., `oa.code_chunk_analysis.v1`).
    const JOB_TYPE: &'static str;

    /// The schema version for this job type.
    const SCHEMA_VERSION: SchemaVersion;

    /// Get the verification settings for this request.
    fn verification(&self) -> &Verification;

    /// Compute the canonical SHA-256 hash of this request.
    fn compute_hash(&self) -> Result<String, HashError> {
        canonical_hash(self)
    }
}

/// Trait for job response types.
///
/// Every job response has an associated request type and provenance information.
pub trait JobResponse: Serialize + DeserializeOwned + Clone {
    /// The associated request type.
    type Request: JobRequest;

    /// Get the provenance information for this response.
    fn provenance(&self) -> &Provenance;
}

/// A job envelope that wraps any job request or response.
///
/// This provides a uniform structure for serializing jobs with metadata.
///
/// # Example
///
/// ```
/// use protocol::jobs::{JobEnvelope, ChunkAnalysisRequest};
///
/// let request = ChunkAnalysisRequest::default();
/// let envelope = JobEnvelope::from_request(request);
///
/// assert_eq!(envelope.job_type, "oa.code_chunk_analysis.v1");
/// assert!(envelope.job_hash.is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope<T> {
    /// The job type identifier.
    pub job_type: String,

    /// The schema version.
    pub schema_version: String,

    /// The computed hash of the payload (for requests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_hash: Option<String>,

    /// The actual request or response payload.
    pub payload: T,
}

impl<T: JobRequest> JobEnvelope<T> {
    /// Create an envelope from a job request.
    ///
    /// This computes the job hash automatically.
    pub fn from_request(request: T) -> Self {
        let job_hash = request.compute_hash().ok();
        Self {
            job_type: T::JOB_TYPE.to_string(),
            schema_version: T::SCHEMA_VERSION.to_string(),
            job_hash,
            payload: request,
        }
    }
}

impl<T: JobResponse> JobEnvelope<T> {
    /// Create an envelope from a job response.
    pub fn from_response(response: T) -> Self {
        Self {
            job_type: T::Request::JOB_TYPE.to_string(),
            schema_version: T::Request::SCHEMA_VERSION.to_string(),
            job_hash: None,
            payload: response,
        }
    }
}

/// Registry of known job types.
///
/// This provides a way to look up job type metadata.
#[derive(Debug, Clone)]
pub struct JobTypeInfo {
    /// The job type identifier.
    pub job_type: &'static str,
    /// The current schema version.
    pub schema_version: SchemaVersion,
    /// Default verification mode.
    pub default_verification: Verification,
    /// Human-readable description.
    pub description: &'static str,
}

/// Get information about all registered job types.
pub fn registered_job_types() -> Vec<JobTypeInfo> {
    vec![
        JobTypeInfo {
            job_type: ChunkAnalysisRequest::JOB_TYPE,
            schema_version: ChunkAnalysisRequest::SCHEMA_VERSION,
            default_verification: Verification::subjective_with_judge(2),
            description: "Analyze a code chunk for summaries, symbols, and faults",
        },
        JobTypeInfo {
            job_type: EmbeddingsRequest::JOB_TYPE,
            schema_version: EmbeddingsRequest::SCHEMA_VERSION,
            default_verification: Verification::subjective_with_majority(1),
            description: "Generate text embeddings for semantic search",
        },
        JobTypeInfo {
            job_type: RerankRequest::JOB_TYPE,
            schema_version: RerankRequest::SCHEMA_VERSION,
            default_verification: Verification::subjective_with_majority(2),
            description: "Rerank retrieval candidates by relevance",
        },
        JobTypeInfo {
            job_type: SandboxRunRequest::JOB_TYPE,
            schema_version: SandboxRunRequest::SCHEMA_VERSION,
            default_verification: Verification::objective(),
            description: "Run commands in a sandboxed environment",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_envelope_from_request() {
        let request = ChunkAnalysisRequest::default();
        let envelope = JobEnvelope::from_request(request);

        assert_eq!(envelope.job_type, "oa.code_chunk_analysis.v1");
        assert_eq!(envelope.schema_version, "1.0.0");
        assert!(envelope.job_hash.is_some());
    }

    #[test]
    fn test_registered_job_types() {
        let types = registered_job_types();
        assert_eq!(types.len(), 4);

        let chunk_type = types.iter().find(|t| t.job_type.contains("chunk")).unwrap();
        assert_eq!(chunk_type.default_verification.redundancy, 2);

        let embed_type = types.iter().find(|t| t.job_type.contains("embed")).unwrap();
        assert_eq!(embed_type.default_verification.redundancy, 1);
    }

    #[test]
    fn test_envelope_serde() {
        let request = ChunkAnalysisRequest::default();
        let envelope = JobEnvelope::from_request(request);

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: JobEnvelope<ChunkAnalysisRequest> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.job_type, envelope.job_type);
        assert_eq!(parsed.job_hash, envelope.job_hash);
    }
}
