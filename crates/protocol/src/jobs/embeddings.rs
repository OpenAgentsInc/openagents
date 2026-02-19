//! Embeddings job type (`oa.embeddings.v1`).
//!
//! This job type is used for generating text embeddings via the swarm.
//! It takes a batch of texts and returns embedding vectors.

use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

use super::{JobRequest, JobResponse};

/// Request for text embeddings.
///
/// # Example
///
/// ```
/// use protocol::jobs::EmbeddingsRequest;
///
/// let request = EmbeddingsRequest {
///     texts: vec!["Hello world".into(), "How are you?".into()],
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingsRequest {
    /// Texts to embed.
    pub texts: Vec<String>,

    /// Optional: preferred embedding model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_hint: Option<String>,

    /// Optional: desired embedding dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,

    /// Verification settings.
    #[serde(default = "default_verification")]
    pub verification: Verification,
}

fn default_verification() -> Verification {
    // Embeddings are deterministic for the same model, so use minimal redundancy
    // Use majority with 1 provider (effectively no redundancy needed)
    Verification::subjective_with_majority(1)
}

impl Default for EmbeddingsRequest {
    fn default() -> Self {
        Self {
            texts: Vec::new(),
            model_hint: None,
            dimensions: None,
            verification: default_verification(),
        }
    }
}

impl EmbeddingsRequest {
    /// Create a new embeddings request for a single text.
    pub fn single(text: impl Into<String>) -> Self {
        Self {
            texts: vec![text.into()],
            ..Default::default()
        }
    }

    /// Create a new embeddings request for multiple texts.
    pub fn batch(texts: Vec<String>) -> Self {
        Self {
            texts,
            ..Default::default()
        }
    }

    /// Set the preferred model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_hint = Some(model.into());
        self
    }

    /// Set the desired dimensions.
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

impl JobRequest for EmbeddingsRequest {
    const JOB_TYPE: &'static str = "oa.embeddings.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}

/// Response containing embedding vectors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingsResponse {
    /// Embedding vectors (one per input text).
    pub embeddings: Vec<Vec<f32>>,

    /// Model used for embedding.
    pub model_id: String,

    /// Dimensions of the embedding vectors.
    pub dimensions: usize,

    /// Provenance information.
    pub provenance: Provenance,
}

impl EmbeddingsResponse {
    /// Create a new embeddings response.
    pub fn new(embeddings: Vec<Vec<f32>>, model_id: impl Into<String>) -> Self {
        let dimensions = embeddings.first().map(|e| e.len()).unwrap_or(0);
        let model_str: String = model_id.into();
        Self {
            embeddings,
            model_id: model_str.clone(),
            dimensions,
            provenance: Provenance::new(model_str),
        }
    }

    /// Set provenance.
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }
}

impl JobResponse for EmbeddingsResponse {
    type Request = EmbeddingsRequest;

    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobEnvelope;

    #[test]
    fn test_embeddings_request_hash() {
        let request = EmbeddingsRequest::batch(vec!["Hello".into(), "World".into()]);

        let hash1 = request.compute_hash().unwrap();
        let hash2 = request.compute_hash().unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_job_type_constant() {
        assert_eq!(EmbeddingsRequest::JOB_TYPE, "oa.embeddings.v1");
        assert_eq!(
            EmbeddingsRequest::SCHEMA_VERSION,
            SchemaVersion::new(1, 0, 0)
        );
    }

    #[test]
    fn test_single_text() {
        let request = EmbeddingsRequest::single("Hello world");
        assert_eq!(request.texts.len(), 1);
        assert_eq!(request.texts[0], "Hello world");
    }

    #[test]
    fn test_with_model() {
        let request = EmbeddingsRequest::single("test").with_model("nomic-embed-text");
        assert_eq!(request.model_hint, Some("nomic-embed-text".into()));
    }

    #[test]
    fn test_with_dimensions() {
        let request = EmbeddingsRequest::single("test").with_dimensions(768);
        assert_eq!(request.dimensions, Some(768));
    }

    #[test]
    fn test_request_serde() {
        let request = EmbeddingsRequest {
            texts: vec!["text1".into(), "text2".into()],
            model_hint: Some("text-embedding-3-small".into()),
            dimensions: Some(1536),
            verification: Verification::subjective_with_majority(1),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: EmbeddingsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_response_serde() {
        let response = EmbeddingsResponse::new(
            vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
            "nomic-embed-text",
        );

        let json = serde_json::to_string(&response).unwrap();
        let parsed: EmbeddingsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.embeddings, parsed.embeddings);
        assert_eq!(response.model_id, parsed.model_id);
        assert_eq!(response.dimensions, 3);
    }

    #[test]
    fn test_envelope_integration() {
        let request = EmbeddingsRequest::single("test");
        let envelope = JobEnvelope::from_request(request);
        assert_eq!(envelope.job_type, "oa.embeddings.v1");
        assert!(envelope.job_hash.is_some());
    }

    #[test]
    fn test_default_verification() {
        let request = EmbeddingsRequest::default();
        assert_eq!(request.verification.redundancy, 1);
    }
}
