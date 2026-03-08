//! OpenAgents provider-facing types for Mox.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use mox_runtime::{
    AmdDeviceMetadata, AmdRecoveryProfile, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo,
    BackendSelection, HealthStatus,
};
use mox_serve::{
    DecoderModelDescriptor, EMBEDDINGS_PRODUCT_ID, EmbeddingModelDescriptor, EmbeddingRequest,
    EmbeddingResponse, GenerationInput, GenerationRequest, GenerationResponse, QuantizationMode,
    SessionId, TEXT_GENERATION_PRODUCT_ID, TerminationReason, WeightArtifactMetadata,
    WeightBundleMetadata, WeightFormat, WeightSource,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "provider integration, capabilities, and receipts";

/// Provider-facing backend family identifier.
pub const BACKEND_FAMILY: &str = "mox";

/// Stable provider-facing summary of the weight bundle backing a served model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightBundleEvidence {
    /// Weight artifact format.
    pub format: WeightFormat,
    /// Weight source authority.
    pub source: WeightSource,
    /// Weight quantization posture.
    pub quantization: QuantizationMode,
    /// Stable bundle digest.
    pub digest: String,
    /// External artifacts that backed the bundle, if any.
    pub artifacts: Vec<WeightArtifactMetadata>,
}

impl WeightBundleEvidence {
    /// Creates weight-bundle evidence from stable model metadata.
    #[must_use]
    pub fn from_metadata(metadata: &WeightBundleMetadata) -> Self {
        Self {
            format: metadata.format,
            source: metadata.source,
            quantization: metadata.quantization,
            digest: metadata.digest.clone(),
            artifacts: metadata.artifacts.clone(),
        }
    }
}

/// AMD-specific provider truth derived from reusable runtime/backend state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdCapabilityContext {
    /// Active AMD mode.
    pub mode: AmdRuntimeMode,
    /// Stable AMD topology snapshot.
    pub topology: AmdTopologyInfo,
    /// Risk posture for the AMD mode.
    pub risk: AmdRiskProfile,
    /// Recovery posture for the AMD mode.
    pub recovery: AmdRecoveryProfile,
}

impl AmdCapabilityContext {
    /// Derives AMD capability context from a runtime backend selection.
    #[must_use]
    pub fn from_backend_selection(backend_selection: &BackendSelection) -> Option<Self> {
        backend_selection
            .selected_device
            .as_ref()
            .and_then(|device| device.amd_metadata.as_ref())
            .map(Self::from_metadata)
    }

    /// Derives AMD capability context directly from runtime device metadata.
    #[must_use]
    pub fn from_metadata(metadata: &AmdDeviceMetadata) -> Self {
        Self {
            mode: metadata.mode,
            topology: metadata.topology.clone(),
            risk: metadata.risk.clone(),
            recovery: metadata.recovery.clone(),
        }
    }
}

/// Capability envelope for a provider-advertised embeddings product.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu`.
    pub runtime_backend: String,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// AMD-specific capability context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity for the loaded model.
    pub weight_bundle: WeightBundleEvidence,
    /// Stable output dimensions.
    pub dimensions: usize,
    /// Current readiness status.
    pub readiness: ProviderReadiness,
}

impl CapabilityEnvelope {
    /// Creates a capability envelope for an embeddings model.
    #[must_use]
    pub fn embeddings(
        backend_selection: BackendSelection,
        model_id: impl Into<String>,
        model_family: impl Into<String>,
        model_revision: impl Into<String>,
        weight_bundle: WeightBundleEvidence,
        dimensions: usize,
        readiness: ProviderReadiness,
    ) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            model_id: model_id.into(),
            model_family: model_family.into(),
            model_revision: model_revision.into(),
            weight_bundle,
            dimensions,
            readiness,
        }
    }

    /// Creates a capability envelope directly from an embeddings model descriptor.
    #[must_use]
    pub fn from_embedding_model(
        backend_selection: BackendSelection,
        model: &EmbeddingModelDescriptor,
        readiness: ProviderReadiness,
    ) -> Self {
        Self::embeddings(
            backend_selection,
            model.model.model_id.clone(),
            model.model.family.clone(),
            model.model.revision.clone(),
            WeightBundleEvidence::from_metadata(&model.weights),
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
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// AMD-specific execution context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// Request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity used during execution.
    pub weight_bundle: WeightBundleEvidence,
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
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        request_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: request_digest.into(),
            model_id: response.metadata.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
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
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self::succeeded(
            backend_selection,
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
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        failure_reason: impl Into<String>,
    ) -> Self {
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: digest_embedding_request(request),
            model_id: request.model.model.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            output_dimensions: request.model.dimensions,
            output_vector_count: 0,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason: Some(failure_reason.into()),
        }
    }
}

/// KV-cache mode exposed to provider capability consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheMode {
    /// In-memory per-session KV cache.
    InMemory,
    /// Future paged KV cache.
    Paged,
    /// Future tiered/offloaded KV cache.
    Tiered,
}

/// Batch posture exposed to provider capability consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchPosture {
    /// Single-request reference path only.
    SingleRequestOnly,
    /// Future static batching.
    StaticBatch,
    /// Future continuous batching.
    Continuous,
}

/// Capability envelope for a text-generation provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGenerationCapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu`.
    pub runtime_backend: String,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// AMD-specific capability context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity for the loaded model.
    pub weight_bundle: WeightBundleEvidence,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Advertised KV cache posture.
    pub kv_cache_mode: KvCacheMode,
    /// Advertised batching posture.
    pub batch_posture: BatchPosture,
    /// Current readiness state.
    pub readiness: ProviderReadiness,
}

impl TextGenerationCapabilityEnvelope {
    /// Creates a capability envelope from a decoder model descriptor.
    #[must_use]
    pub fn from_decoder_model(
        backend_selection: BackendSelection,
        model: &DecoderModelDescriptor,
        kv_cache_mode: KvCacheMode,
        batch_posture: BatchPosture,
        readiness: ProviderReadiness,
    ) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            model_id: model.model.model_id.clone(),
            model_family: model.model.family.clone(),
            model_revision: model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&model.weights),
            max_context: model.config.max_context,
            kv_cache_mode,
            batch_posture,
            readiness,
        }
    }
}

/// Execution receipt for a text-generation job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGenerationReceipt {
    /// Product identifier.
    pub product_id: String,
    /// Backend family.
    pub backend_family: String,
    /// Runtime backend.
    pub runtime_backend: String,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// AMD-specific execution context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// Request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Optional execution-plan digest.
    pub execution_plan_digest: Option<String>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity used during execution.
    pub weight_bundle: WeightBundleEvidence,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Prompt token count.
    pub input_tokens: usize,
    /// Output token count.
    pub output_tokens: usize,
    /// Cached token count after execution.
    pub cache_tokens: usize,
    /// Terminal termination reason when execution succeeded.
    pub termination: Option<TerminationReason>,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal status.
    pub status: ReceiptStatus,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
}

impl TextGenerationReceipt {
    /// Creates a success receipt from request/response contracts.
    #[must_use]
    pub fn succeeded(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        response: &GenerationResponse,
        request_digest: impl Into<String>,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: request_digest.into(),
            execution_plan_digest: Some(execution_plan_digest.into()),
            model_id: response.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            session_id: response.session_id.clone(),
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cache_tokens: response.usage.cache_tokens,
            termination: Some(response.termination),
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
        }
    }

    /// Creates a success receipt and computes the request digest internally.
    #[must_use]
    pub fn succeeded_for_response(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        response: &GenerationResponse,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self::succeeded(
            backend_selection,
            request,
            response,
            digest_generation_request(request),
            execution_plan_digest,
            started_at_unix_ms,
            ended_at_unix_ms,
        )
    }

    /// Creates a failure receipt for a request that could not be executed.
    #[must_use]
    pub fn failed_for_request(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        execution_plan_digest: Option<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        failure_reason: impl Into<String>,
    ) -> Self {
        let input_tokens = match &request.prompt {
            GenerationInput::Text(text) => text.split_whitespace().count(),
            GenerationInput::Tokens(tokens) => tokens.len(),
        };

        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: digest_generation_request(request),
            execution_plan_digest,
            model_id: request.model.model.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            session_id: request.session_id.clone(),
            input_tokens,
            output_tokens: 0,
            cache_tokens: 0,
            termination: None,
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
    hasher.update(b"|");
    digest_weight_bundle(&mut hasher, &request.model.weights);
    for input in &request.inputs {
        hasher.update(b"|");
        hasher.update(input.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Computes a deterministic digest for a generation request.
#[must_use]
pub fn digest_generation_request(request: &GenerationRequest) -> String {
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
    hasher.update(request.model.config.hidden_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.layer_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.vocab_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.max_context.to_string().as_bytes());
    hasher.update(b"|");
    digest_weight_bundle(&mut hasher, &request.model.weights);
    hasher.update(b"|");
    if let Some(session_id) = &request.session_id {
        hasher.update(session_id.as_str().as_bytes());
    }
    hasher.update(b"|");
    digest_generation_input(&mut hasher, &request.prompt);
    hasher.update(b"|");
    hasher.update(request.options.max_output_tokens.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", request.options.decode_strategy).as_bytes());
    hasher.update(b"|");
    hasher.update(if request.reset_session { b"1" } else { b"0" });
    hex::encode(hasher.finalize())
}

/// Provider-side adapter interface for the embeddings smoke path.
pub trait EmbeddingsProviderAdapter {
    /// Returns the advertised capability envelope.
    fn capability(&self) -> CapabilityEnvelope;

    /// Returns the provider readiness state.
    fn readiness(&self) -> ProviderReadiness;
}

/// Provider-side adapter interface for text generation.
pub trait TextGenerationProviderAdapter {
    /// Returns the advertised text-generation capability envelope.
    fn text_generation_capability(&self) -> TextGenerationCapabilityEnvelope;

    /// Returns the provider readiness state.
    fn readiness(&self) -> ProviderReadiness;
}

fn digest_generation_input(hasher: &mut Sha256, input: &GenerationInput) {
    match input {
        GenerationInput::Text(text) => {
            hasher.update(b"text|");
            hasher.update(text.as_bytes());
        }
        GenerationInput::Tokens(tokens) => {
            hasher.update(b"tokens|");
            for token in tokens.as_slice() {
                hasher.update(token.as_u32().to_string().as_bytes());
                hasher.update(b",");
            }
        }
    }
}

fn digest_weight_bundle(hasher: &mut Sha256, weight_bundle: &WeightBundleMetadata) {
    hasher.update(format!("{:?}", weight_bundle.format).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", weight_bundle.source).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", weight_bundle.quantization).as_bytes());
    hasher.update(b"|");
    hasher.update(weight_bundle.digest.as_bytes());
    for artifact in &weight_bundle.artifacts {
        hasher.update(b"|");
        hasher.update(artifact.name.as_bytes());
        hasher.update(b":");
        hasher.update(artifact.byte_length.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(artifact.sha256.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use mox_core::{DType, Device, DeviceKind, QuantizationMode as RuntimeQuantizationMode};
    use mox_runtime::{
        AmdDeviceMetadata, AmdDriverBinding, AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel,
        AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, BackendSelection, DeviceDescriptor,
        HealthStatus, QuantizationExecution, QuantizationLoadPath, QuantizationSupport,
    };
    use mox_serve::{
        EmbeddingRequest, EmbeddingResponse, EmbeddingVector, GenerationOptions, GenerationRequest,
        GenerationResponse, ReferenceWordDecoder, SessionId, SmokeByteEmbedder, TokenSequence,
    };
    use serde_json::json;

    use super::{
        BatchPosture, CapabilityEnvelope, ExecutionReceipt, KvCacheMode, ProviderReadiness,
        ReceiptStatus, TextGenerationCapabilityEnvelope, TextGenerationReceipt,
        WeightBundleEvidence, digest_embedding_request, digest_generation_request,
    };

    #[test]
    fn capability_envelope_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("cpu backend ready"),
        );

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "mox",
                "product_id": "mox.embeddings",
                "runtime_backend": "cpu",
                "backend_selection": {
                    "requested_backend": "cpu",
                    "effective_backend": "cpu",
                    "selected_device": {
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "fallback_reason": null
                },
                "model_id": "smoke-byte-embed-v0",
                "model_family": "smoke",
                "model_revision": "v0",
                "weight_bundle": {
                    "format": "ProgrammaticFixture",
                    "source": "Fixture",
                    "quantization": "none",
                    "digest": "30a2fd0264ef45e96101268ae97cfbdffb79540210c88ab834117bc0111c0b00",
                    "artifacts": []
                },
                "dimensions": 8,
                "readiness": {
                    "status": "Ready",
                    "message": "cpu backend ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn text_generation_capability_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cpu_backend_selection(),
            &model,
            KvCacheMode::InMemory,
            BatchPosture::SingleRequestOnly,
            ProviderReadiness::ready("cpu backend ready"),
        );

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "mox",
                "product_id": "mox.text_generation",
                "runtime_backend": "cpu",
                "backend_selection": {
                    "requested_backend": "cpu",
                    "effective_backend": "cpu",
                    "selected_device": {
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "fallback_reason": null
                },
                "model_id": "fixture-word-decoder-v0",
                "model_family": "fixture_decoder",
                "model_revision": "v0",
                "weight_bundle": {
                    "format": "ProgrammaticFixture",
                    "source": "Fixture",
                    "quantization": "none",
                    "digest": "7daf98e44b6eee34df8d97f24419709f23b19010cdb49c9b18b771936ced352b",
                    "artifacts": []
                },
                "max_context": 8,
                "kv_cache_mode": "in_memory",
                "batch_posture": "single_request_only",
                "readiness": {
                    "status": "Ready",
                    "message": "cpu backend ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn fallback_capability_reports_requested_metal_but_effective_cpu()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            metal_fallback_selection(),
            &model,
            ProviderReadiness::ready("cpu fallback ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cpu"));
        assert_eq!(
            encoded["backend_selection"]["requested_backend"],
            json!("metal")
        );
        assert_eq!(
            encoded["backend_selection"]["effective_backend"],
            json!("cpu")
        );
        assert_eq!(
            encoded["backend_selection"]["fallback_reason"],
            json!("metal backend unavailable: no supported Metal device")
        );
        Ok(())
    }

    #[test]
    fn amd_kfd_capability_reports_mode_topology_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_kfd_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Ready,
                message: String::from("amd_kfd ready on 1 AMD device"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("amd_kfd"));
        assert_eq!(encoded["amd"]["mode"], json!("kfd"));
        assert_eq!(encoded["amd"]["topology"]["architecture"], json!("gfx1100"));
        assert_eq!(
            encoded["amd"]["risk"]["requires_explicit_opt_in"],
            json!(false)
        );
        assert_eq!(
            encoded["amd"]["recovery"]["expected_actions"],
            json!(["kernel_driver_reset", "reboot_host"])
        );
        Ok(())
    }

    #[test]
    fn amd_userspace_capability_reports_disabled_risk_posture()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_userspace_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Offline,
                message: String::from("amd_userspace disabled pending explicit opt-in"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("amd_userspace"));
        assert_eq!(encoded["amd"]["mode"], json!("userspace"));
        assert_eq!(
            encoded["amd"]["risk"]["requires_explicit_opt_in"],
            json!(true)
        );
        assert_eq!(
            encoded["amd"]["risk"]["may_unbind_kernel_driver"],
            json!(true)
        );
        assert_eq!(
            encoded["amd"]["recovery"]["driver_binding"],
            json!("kernel_amdgpu")
        );
        Ok(())
    }

    #[test]
    fn execution_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-3",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.1, 0.2, 0.3, 0.4],
                // Receipt tests do not care about matching model dimensions here.
            }],
        );
        let receipt = ExecutionReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            10,
            20,
        );

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: ExecutionReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.runtime_backend, "cpu");
        assert_eq!(decoded.backend_selection.requested_backend, "cpu");
        assert_eq!(decoded.output_vector_count, 1);
        assert_eq!(decoded.failure_reason, None);
        assert_eq!(decoded.model_family, "smoke");
        assert_eq!(decoded.model_revision, "v0");
        assert_eq!(
            decoded.weight_bundle,
            WeightBundleEvidence::from_metadata(&request.model.weights)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-3",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000003")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![mox_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            mox_serve::TerminationReason::EndOfSequence,
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "plan-digest-1",
            100,
            120,
        );

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        assert_eq!(receipt.output_tokens, 1);
        assert_eq!(
            receipt.termination,
            Some(mox_serve::TerminationReason::EndOfSequence)
        );
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: TextGenerationReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.runtime_backend, "cpu");
        assert_eq!(decoded.backend_selection.requested_backend, "cpu");
        assert_eq!(decoded.model_family, "fixture_decoder");
        assert_eq!(decoded.model_revision, "v0");
        assert_eq!(
            decoded.weight_bundle,
            WeightBundleEvidence::from_metadata(&request.model.weights)
        );
        Ok(())
    }

    #[test]
    fn failed_receipt_carries_reason() {
        let request = EmbeddingRequest::new(
            "req-4",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );

        let receipt = ExecutionReceipt::failed_for_request(
            metal_fallback_selection(),
            &request,
            5,
            6,
            "backend offline",
        );
        assert_eq!(receipt.status, ReceiptStatus::Failed);
        assert_eq!(receipt.failure_reason.as_deref(), Some("backend offline"));
        assert_eq!(receipt.runtime_backend, "cpu");
        assert_eq!(receipt.backend_selection.requested_backend, "metal");
        assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    }

    #[test]
    fn amd_receipt_carries_execution_context() {
        let request = EmbeddingRequest::new(
            "req-amd-1",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );

        let receipt = ExecutionReceipt::failed_for_request(
            amd_userspace_selection(),
            &request,
            10,
            11,
            "backend disabled",
        );
        let amd = receipt.amd.expect("amd context");
        assert_eq!(amd.mode, AmdRuntimeMode::Userspace);
        assert!(amd.risk.requires_explicit_opt_in);
        assert_eq!(amd.recovery.driver_binding, AmdDriverBinding::KernelAmdgpu);
    }

    #[test]
    fn request_digests_are_deterministic() {
        let embedding_request = EmbeddingRequest::new(
            "req-5",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let generation_request = GenerationRequest::new_tokens(
            "gen-5",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000005")),
            TokenSequence::new(vec![mox_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );

        assert_eq!(
            digest_embedding_request(&embedding_request),
            digest_embedding_request(&embedding_request)
        );
        assert_eq!(
            digest_generation_request(&generation_request),
            digest_generation_request(&generation_request)
        );
    }

    #[test]
    fn request_digests_change_when_weight_identity_changes() {
        let mut embedding_request = EmbeddingRequest::new(
            "req-6",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let mut generation_request = GenerationRequest::new_tokens(
            "gen-6",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000006")),
            TokenSequence::new(vec![mox_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );

        let embedding_digest = digest_embedding_request(&embedding_request);
        let generation_digest = digest_generation_request(&generation_request);

        embedding_request.model.weights.digest = String::from("different-embedding-bundle");
        generation_request.model.weights.quantization = mox_serve::QuantizationMode::Int8Symmetric;

        assert_ne!(
            digest_embedding_request(&embedding_request),
            embedding_digest
        );
        assert_ne!(
            digest_generation_request(&generation_request),
            generation_digest
        );
    }

    #[test]
    fn readiness_helper_sets_ready_status() {
        let readiness = ProviderReadiness::ready("ok");
        assert_eq!(readiness.status, HealthStatus::Ready);
        assert_eq!(readiness.message, "ok");
    }

    fn sample_decoder_descriptor() -> mox_serve::DecoderModelDescriptor {
        ReferenceWordDecoder::new().descriptor().clone()
    }

    fn sample_embedding_descriptor() -> mox_serve::EmbeddingModelDescriptor {
        SmokeByteEmbedder::new().descriptor().clone()
    }

    fn cpu_backend_selection() -> BackendSelection {
        BackendSelection::direct(
            "cpu",
            Some(sample_cpu_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("matmul"),
                String::from("add"),
            ],
        )
    }

    fn metal_fallback_selection() -> BackendSelection {
        BackendSelection::fallback(
            "metal",
            "cpu",
            Some(sample_cpu_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("matmul"),
                String::from("add"),
            ],
            "metal backend unavailable: no supported Metal device",
        )
    }

    fn sample_cpu_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cpu"),
            device: Device::cpu(),
            device_name: Some(String::from("host cpu")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::Int8Symmetric,
                    load_path: QuantizationLoadPath::DequantizedF32,
                    execution: QuantizationExecution::DequantizeToF32,
                },
            ],
            memory_capacity_bytes: None,
            unified_memory: Some(true),
            feature_flags: vec![String::from("host_memory")],
            amd_metadata: None,
        }
    }

    fn amd_kfd_selection() -> BackendSelection {
        BackendSelection::direct(
            "amd_kfd",
            Some(sample_amd_kfd_device()),
            vec![String::from("probe_only")],
        )
    }

    fn amd_userspace_selection() -> BackendSelection {
        BackendSelection::direct(
            "amd_userspace",
            Some(sample_amd_userspace_device()),
            vec![String::from("probe_only")],
        )
    }

    fn sample_amd_kfd_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("amd_kfd"),
            device: Device::new(DeviceKind::AmdKfd, 0, Some(String::from("amd_kfd:0"))),
            device_name: Some(String::from("AMD Radeon KFD Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("kfd_device_node")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Kfd,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Standard,
                    requires_explicit_opt_in: false,
                    may_unbind_kernel_driver: false,
                    warnings: Vec::new(),
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::KernelAmdgpu,
                    expected_actions: vec![
                        AmdRecoveryAction::KernelDriverReset,
                        AmdRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    fn sample_amd_userspace_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("amd_userspace"),
            device: Device::new(
                DeviceKind::AmdUserspace,
                0,
                Some(String::from("amd_userspace:0")),
            ),
            device_name: Some(String::from("AMD Radeon Userspace Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("userspace_opt_in_disabled")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Userspace,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Elevated,
                    requires_explicit_opt_in: true,
                    may_unbind_kernel_driver: true,
                    warnings: vec![String::from(
                        "userspace mode may require unloading or rebinding amdgpu",
                    )],
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::KernelAmdgpu,
                    expected_actions: vec![
                        AmdRecoveryAction::ProcessRestart,
                        AmdRecoveryAction::RebindKernelDriver,
                        AmdRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }
}
