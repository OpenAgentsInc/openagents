//! OpenAgents provider-facing types for Rustygrad.

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

use rustygrad_runtime::HealthStatus;
use rustygrad_serve::{
    EmbeddingModelDescriptor, EmbeddingRequest, EmbeddingResponse, EMBEDDINGS_PRODUCT_ID,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "provider integration, capabilities, and receipts";

/// Provider-facing backend family identifier.
pub const BACKEND_FAMILY: &str = "rustygrad";

/// Capability envelope for a provider-advertised embeddings product.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu`.
    pub runtime_backend: String,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Stable output dimensions.
    pub dimensions: usize,
    /// Current readiness status.
    pub readiness: ProviderReadiness,
}

impl CapabilityEnvelope {
    /// Creates a capability envelope for an embeddings model.
    #[must_use]
    pub fn embeddings(
        runtime_backend: impl Into<String>,
        model_id: impl Into<String>,
        model_family: impl Into<String>,
        dimensions: usize,
        readiness: ProviderReadiness,
    ) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            runtime_backend: runtime_backend.into(),
            model_id: model_id.into(),
            model_family: model_family.into(),
            dimensions,
            readiness,
        }
    }

    /// Creates a capability envelope directly from an embeddings model descriptor.
    #[must_use]
    pub fn from_embedding_model(
        runtime_backend: impl Into<String>,
        model: &EmbeddingModelDescriptor,
        readiness: ProviderReadiness,
    ) -> Self {
        Self::embeddings(
            runtime_backend,
            model.model.model_id.clone(),
            model.model.family.clone(),
            model.dimensions,
            readiness,
        )
    }
}

/// Provider readiness contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReadiness {
    /// Current status.
    pub status: HealthStatus,
    /// Plain-text explanation.
    pub message: String,
}

impl ProviderReadiness {
    /// Creates a ready state.
    #[must_use]
    pub fn ready(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Ready,
            message: message.into(),
        }
    }
}

/// Terminal receipt status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    /// Execution completed successfully.
    Succeeded,
    /// Execution failed.
    Failed,
}

/// Execution receipt for an embeddings job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Product identifier.
    pub product_id: String,
    /// Backend family.
    pub backend_family: String,
    /// Runtime backend.
    pub runtime_backend: String,
    /// Request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Model identifier.
    pub model_id: String,
    /// Output dimensions.
    pub output_dimensions: usize,
    /// Number of returned vectors.
    pub output_vector_count: usize,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal status.
    pub status: ReceiptStatus,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
}

impl ExecutionReceipt {
    /// Creates a success receipt from request/response contracts.
    #[must_use]
    pub fn succeeded(
        runtime_backend: impl Into<String>,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        request_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: runtime_backend.into(),
            request_id: request.request_id.clone(),
            request_digest: request_digest.into(),
            model_id: response.metadata.model_id.clone(),
            output_dimensions: response.metadata.dimensions,
            output_vector_count: response.metadata.vector_count,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
        }
    }

    /// Creates a success receipt and computes the request digest internally.
    #[must_use]
    pub fn succeeded_for_response(
        runtime_backend: impl Into<String>,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self::succeeded(
            runtime_backend,
            request,
            response,
            digest_embedding_request(request),
            started_at_unix_ms,
            ended_at_unix_ms,
        )
    }

    /// Creates a failure receipt for a request that could not be executed.
    #[must_use]
    pub fn failed_for_request(
        runtime_backend: impl Into<String>,
        request: &EmbeddingRequest,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        failure_reason: impl Into<String>,
    ) -> Self {
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: runtime_backend.into(),
            request_id: request.request_id.clone(),
            request_digest: digest_embedding_request(request),
            model_id: request.model.model.model_id.clone(),
            output_dimensions: request.model.dimensions,
            output_vector_count: 0,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason: Some(failure_reason.into()),
        }
    }
}

/// Computes a deterministic digest for an embeddings request.
#[must_use]
pub fn digest_embedding_request(request: &EmbeddingRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.product_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.family.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.dimensions.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", request.model.normalization).as_bytes());
    for input in &request.inputs {
        hasher.update(b"|");
        hasher.update(input.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Provider-side adapter interface for the embeddings smoke path.
pub trait EmbeddingsProviderAdapter {
    /// Returns the advertised capability envelope.
    fn capability(&self) -> CapabilityEnvelope;

    /// Returns the provider readiness state.
    fn readiness(&self) -> ProviderReadiness;
}

#[cfg(test)]
mod tests {
    use rustygrad_runtime::HealthStatus;
    use rustygrad_serve::{
        EmbeddingModelDescriptor, EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse,
        EmbeddingVector, ModelDescriptor,
    };

    use super::{
        digest_embedding_request, CapabilityEnvelope, ExecutionReceipt, ProviderReadiness,
        ReceiptStatus,
    };

    #[test]
    fn capability_envelope_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let model = EmbeddingModelDescriptor::new(
            ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
            8,
            EmbeddingNormalization::None,
        );
        let envelope = CapabilityEnvelope::from_embedding_model(
            "cpu",
            &model,
            ProviderReadiness::ready("cpu backend ready"),
        );

        let encoded = serde_json::to_string_pretty(&envelope)?;
        let expected = r#"{
  "backend_family": "rustygrad",
  "product_id": "rustygrad.embeddings",
  "runtime_backend": "cpu",
  "model_id": "smoke-byte-embed-v0",
  "model_family": "smoke",
  "dimensions": 8,
  "readiness": {
    "status": "Ready",
    "message": "cpu backend ready"
  }
}"#;
        assert_eq!(encoded, expected);
        Ok(())
    }

    #[test]
    fn execution_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-3",
            EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                4,
                EmbeddingNormalization::UnitLength,
            ),
            vec![String::from("hello")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.1, 0.2, 0.3, 0.4],
            }],
        );
        let receipt = ExecutionReceipt::succeeded_for_response("cpu", &request, &response, 10, 20);

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: ExecutionReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.output_vector_count, 1);
        assert_eq!(decoded.failure_reason, None);
        Ok(())
    }

    #[test]
    fn failed_receipt_carries_reason() {
        let request = EmbeddingRequest::new(
            "req-4",
            EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                8,
                EmbeddingNormalization::None,
            ),
            vec![String::from("hello")],
        );

        let receipt =
            ExecutionReceipt::failed_for_request("cpu", &request, 5, 6, "backend offline");
        assert_eq!(receipt.status, ReceiptStatus::Failed);
        assert_eq!(receipt.failure_reason.as_deref(), Some("backend offline"));
    }

    #[test]
    fn request_digest_is_deterministic() {
        let request = EmbeddingRequest::new(
            "req-5",
            EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                8,
                EmbeddingNormalization::None,
            ),
            vec![String::from("same input")],
        );

        let first = digest_embedding_request(&request);
        let second = digest_embedding_request(&request);
        assert_eq!(first, second);
    }

    #[test]
    fn readiness_helper_sets_ready_status() {
        let readiness = ProviderReadiness::ready("ok");
        assert_eq!(readiness.status, HealthStatus::Ready);
        assert_eq!(readiness.message, "ok");
    }
}
