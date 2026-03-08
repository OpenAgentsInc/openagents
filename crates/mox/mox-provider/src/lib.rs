//! OpenAgents provider-facing types for Mox.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use mox_runtime::{
    AmdDeviceMetadata, AmdRecoveryProfile, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo,
    BackendSelection, HealthStatus, KvCacheAccounting, KvCachePolicy, LocalRuntimeDiagnostic,
    LocalRuntimeObservability, MemoryResidencySnapshot, ModelMemoryPlan, ModelResidencyPolicy,
    PrefixCacheIdentity, PrefixCacheReusePolicy, PrefixCacheState,
};
use mox_serve::{
    DecoderModelDescriptor, EMBEDDINGS_PRODUCT_ID, EmbeddingModelDescriptor,
    EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse, GenerationInput,
    GenerationLoadState, GenerationRequest, GenerationResponse, GenerationStreamStatus,
    GenerationStreamTerminal, GenerationStreamingPolicy, QuantizationMode, SessionId,
    TEXT_GENERATION_PRODUCT_ID, TerminationReason, WeightArtifactMetadata, WeightBundleMetadata,
    WeightFormat, WeightSource, default_decoder_kv_cache_policy, default_prefix_cache_policy,
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

/// Provider-facing wrapper for live local-runtime observability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuntimeObservabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Current local-runtime observability snapshot.
    pub observability: LocalRuntimeObservability,
}

impl LocalRuntimeObservabilityEnvelope {
    /// Creates a provider-facing wrapper from a reusable runtime snapshot.
    #[must_use]
    pub fn new(observability: LocalRuntimeObservability) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            observability,
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
    /// Normalization policy applied to returned vectors.
    pub normalization: EmbeddingNormalization,
    /// Whether output order matches input order.
    pub preserves_input_order: bool,
    /// Whether an empty input batch returns an empty successful response.
    pub empty_batch_returns_empty: bool,
    /// Whether callers may request truncated output dimensions.
    pub supports_output_dimensions: bool,
    /// Whether callers may request overflow truncation on long embedding inputs.
    pub supports_input_truncation: bool,
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
        normalization: EmbeddingNormalization,
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
            normalization,
            preserves_input_order: true,
            empty_batch_returns_empty: true,
            supports_output_dimensions: true,
            supports_input_truncation: false,
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
            model.normalization,
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
    /// Execution was cancelled by the caller.
    Cancelled,
    /// Execution aborted because the client disconnected mid-stream.
    Disconnected,
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
    /// Number of request inputs.
    pub input_count: usize,
    /// Number of returned vectors.
    pub output_vector_count: usize,
    /// Normalization policy applied to returned vectors.
    pub normalization: EmbeddingNormalization,
    /// Requested output dimensions when the caller asked for truncated vectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_output_dimensions: Option<usize>,
    /// End-to-end embeddings duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal status.
    pub status: ReceiptStatus,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
    /// Structured local-runtime diagnostic for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
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
            input_count: response.metadata.input_count,
            output_vector_count: response.metadata.vector_count,
            normalization: response.metadata.normalization,
            requested_output_dimensions: response.metadata.requested_output_dimensions,
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
            diagnostic: None,
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
            output_dimensions: request
                .output_dimensions
                .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions)
                .unwrap_or(request.model.dimensions),
            input_count: request.inputs.len(),
            output_vector_count: 0,
            normalization: request.model.normalization,
            requested_output_dimensions: request
                .output_dimensions
                .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions),
            total_duration_ns: None,
            load_duration_ns: None,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason: Some(failure_reason.into()),
            diagnostic: None,
        }
    }

    /// Attaches a structured diagnostic, preserving the plain-text failure reason.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(diagnostic.message.clone());
        }
        self.diagnostic = Some(diagnostic);
        self
    }
}

/// KV-cache mode exposed to provider capability consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheMode {
    /// In-memory per-session KV cache.
    InMemory,
    /// Explicit paged KV cache.
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
    /// Explicit resident-memory plan for the loaded model.
    pub memory_plan: ModelMemoryPlan,
    /// Active local-serving residency policy for the served model set.
    pub residency_policy: ModelResidencyPolicy,
    /// Explicit streaming policy for the local runtime API.
    pub streaming_policy: GenerationStreamingPolicy,
    /// Advertised KV cache posture.
    pub kv_cache_mode: KvCacheMode,
    /// Explicit paged-KV policy when the served path uses paged KV state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
    /// Explicit shared prompt-prefix reuse policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<PrefixCacheReusePolicy>,
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
        memory_plan: ModelMemoryPlan,
        residency_policy: ModelResidencyPolicy,
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
            memory_plan,
            residency_policy,
            streaming_policy: mox_serve::default_generation_streaming_policy(),
            kv_cache_policy: (kv_cache_mode == KvCacheMode::Paged)
                .then(|| default_decoder_kv_cache_policy(model)),
            prefix_cache_policy: Some(default_prefix_cache_policy()),
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
    /// Explicit resident-memory plan for the loaded model, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_plan: Option<ModelMemoryPlan>,
    /// Active local-serving residency policy, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_policy: Option<ModelResidencyPolicy>,
    /// Aggregate resident-memory snapshot for the loaded-model set, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_snapshot: Option<MemoryResidencySnapshot>,
    /// Streaming policy for the local runtime API, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_policy: Option<GenerationStreamingPolicy>,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Prompt token count.
    pub input_tokens: usize,
    /// Output token count.
    pub output_tokens: usize,
    /// Cached token count after execution.
    pub cache_tokens: usize,
    /// End-to-end generation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Prompt-evaluation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration_ns: Option<u64>,
    /// Output-generation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration_ns: Option<u64>,
    /// Whether the request took a warm or cold model path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_state: Option<GenerationLoadState>,
    /// Explicit paged-KV policy for the request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
    /// Explicit paged-KV accounting for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache: Option<KvCacheAccounting>,
    /// Shared prefix-cache state for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_state: Option<PrefixCacheState>,
    /// Shared prefix-cache reuse policy for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<PrefixCacheReusePolicy>,
    /// Shared prefix-cache identity for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_identity: Option<PrefixCacheIdentity>,
    /// Number of prompt-prefix tokens reused from the shared prefix cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_tokens_reused: Option<usize>,
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
    /// Structured local-runtime diagnostic for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
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
        let execution_plan_digest = response
            .provenance
            .as_ref()
            .map(|value| value.execution_plan_digest.clone())
            .unwrap_or_else(|| execution_plan_digest.into());
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: request_digest.into(),
            execution_plan_digest: Some(execution_plan_digest),
            model_id: response.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            memory_plan: response
                .provenance
                .as_ref()
                .and_then(|value| value.memory_plan.clone()),
            residency_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.residency_policy.clone()),
            residency_snapshot: response
                .provenance
                .as_ref()
                .and_then(|value| value.residency_snapshot.clone()),
            streaming_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.streaming_policy.clone()),
            session_id: response.session_id.clone(),
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cache_tokens: response.usage.cache_tokens,
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
            eval_duration_ns: response.metrics.eval_duration_ns,
            load_state: response.provenance.as_ref().map(|value| value.load_state),
            kv_cache_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.kv_cache_policy.clone()),
            kv_cache: response.metrics.kv_cache.clone(),
            prefix_cache_state: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            prefix_cache_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_policy.clone()),
            prefix_cache_identity: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_identity.clone()),
            prefix_tokens_reused: response.metrics.prefix_tokens_reused,
            termination: Some(response.termination),
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
            diagnostic: None,
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
            memory_plan: None,
            residency_policy: None,
            residency_snapshot: None,
            streaming_policy: None,
            session_id: request.session_id.clone(),
            input_tokens,
            output_tokens: 0,
            cache_tokens: 0,
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_duration_ns: None,
            eval_duration_ns: None,
            load_state: None,
            kv_cache_policy: None,
            kv_cache: None,
            prefix_cache_state: None,
            prefix_cache_policy: None,
            prefix_cache_identity: None,
            prefix_tokens_reused: None,
            termination: None,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason: Some(failure_reason.into()),
            diagnostic: None,
        }
    }

    /// Attaches a structured diagnostic, preserving the plain-text failure reason.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(diagnostic.message.clone());
        }
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Creates a receipt from a terminal streaming event, preserving partial output when present.
    #[must_use]
    pub fn from_stream_terminal(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        terminal: &GenerationStreamTerminal,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        let mut receipt = Self::succeeded(
            backend_selection,
            request,
            &terminal.response,
            digest_generation_request(request),
            execution_plan_digest,
            started_at_unix_ms,
            ended_at_unix_ms,
        );
        match terminal.status {
            GenerationStreamStatus::Succeeded => receipt,
            GenerationStreamStatus::Cancelled => {
                receipt.status = ReceiptStatus::Cancelled;
                receipt.failure_reason = terminal.failure_reason.clone();
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
            GenerationStreamStatus::Disconnected => {
                receipt.status = ReceiptStatus::Disconnected;
                receipt.failure_reason = terminal.failure_reason.clone();
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
            GenerationStreamStatus::Failed => {
                receipt.status = ReceiptStatus::Failed;
                receipt.failure_reason = terminal.failure_reason.clone();
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
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
    hasher.update(
        request
            .output_dimensions
            .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions)
            .map_or_else(String::new, |dimensions| dimensions.to_string())
            .as_bytes(),
    );
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
    if let Some(temperature) = request.options.temperature {
        hasher.update(format!("temperature={temperature}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(top_k) = request.options.top_k {
        hasher.update(format!("top_k={top_k}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(top_p) = request.options.top_p {
        hasher.update(format!("top_p={top_p}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(repeat_penalty) = request.options.repeat_penalty {
        hasher.update(format!("repeat_penalty={repeat_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(presence_penalty) = request.options.presence_penalty {
        hasher.update(format!("presence_penalty={presence_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(frequency_penalty) = request.options.frequency_penalty {
        hasher.update(format!("frequency_penalty={frequency_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(seed) = request.options.seed {
        hasher.update(format!("seed={seed}").as_bytes());
    }
    hasher.update(b"|");
    for stop_sequence in &request.options.stop_sequences {
        hasher.update(stop_sequence.as_bytes());
        hasher.update(b"\x1f");
    }
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
    use mox_core::{
        BackendExtensionKind, DType, Device, DeviceKind,
        QuantizationMode as RuntimeQuantizationMode,
    };
    use mox_runtime::{
        AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, AmdDeviceMetadata,
        AmdDriverBinding, AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile,
        AmdRuntimeMode, AmdTopologyInfo, BackendExtensionSupport, BackendRuntimeResources,
        BackendSelection, DeviceDescriptor, DeviceMemoryBudget, HealthStatus, KernelCachePolicy,
        KernelCacheReport, KernelCacheState, KvCacheAccounting, LocalRuntimeDiagnostic,
        LocalRuntimeErrorCode, LocalRuntimeObservability, MemoryResidencySnapshot,
        ModelResidencyPolicy, PrefixCacheIdentity, PrefixCacheState, QuantizationExecution,
        QuantizationLoadPath, QuantizationSupport, RuntimeTransitionEvent, RuntimeTransitionKind,
    };
    use mox_serve::{
        EmbeddingMetrics, EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse,
        EmbeddingVector, GenerationLoadState, GenerationMetrics, GenerationOptions,
        GenerationProvenance, GenerationRequest, GenerationResponse, GenerationStreamStatus,
        GenerationStreamTerminal, ReferenceWordDecoder, SessionId, SmokeByteEmbedder,
        TerminationReason, TokenSequence, default_decoder_kv_cache_policy,
        default_decoder_memory_plan, default_generation_streaming_policy,
        default_prefix_cache_policy,
    };
    use serde_json::json;

    use super::{
        BatchPosture, CapabilityEnvelope, ExecutionReceipt, KvCacheMode,
        LocalRuntimeObservabilityEnvelope, ProviderReadiness, ReceiptStatus,
        TextGenerationCapabilityEnvelope, TextGenerationReceipt, WeightBundleEvidence,
        digest_embedding_request, digest_generation_request,
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
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "policy": {
                        "unavailable": "refuse",
                        "degraded": "allow_same_backend"
                    },
                    "selection_state": "direct",
                    "fallback_reason": null,
                    "degraded_reason": null
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
                "normalization": "None",
                "preserves_input_order": true,
                "empty_batch_returns_empty": true,
                "supports_output_dimensions": true,
                "supports_input_truncation": false,
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
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
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
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "policy": {
                        "unavailable": "refuse",
                        "degraded": "allow_same_backend"
                    },
                    "selection_state": "direct",
                    "fallback_reason": null,
                    "degraded_reason": null
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
                "memory_plan": {
                    "weights_bytes": 0,
                    "kv_cache_bytes": 640,
                    "graph_bytes": 0,
                    "resident_host_bytes": 640,
                    "resident_device_bytes": 0
                },
                "residency_policy": {
                    "max_loaded_models": null,
                    "memory_budget": {
                        "resident_host_bytes": null,
                        "resident_device_bytes": null
                    },
                    "pressure_action": "refuse_new_model"
                },
                "streaming_policy": {
                    "backpressure": "pull_driven",
                    "disconnect": "abort_generation",
                    "cancellation": "abort_after_current_token"
                },
                "kv_cache_mode": "paged",
                "kv_cache_policy": {
                    "device_scope": "same_device_only",
                    "spill_policy": "refuse_new_pages",
                    "page_layout": {
                        "max_context_tokens": 8,
                        "tokens_per_page": 8,
                        "bytes_per_token": 80,
                        "page_bytes": 640,
                        "max_pages": 1
                    }
                },
                "prefix_cache_policy": {
                    "shared_across_sessions": true,
                    "shared_across_users": false,
                    "shared_across_models": false,
                    "shared_across_backends": false
                },
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
    fn capability_envelope_preserves_backend_runtime_resources()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection()
                .with_runtime_resources(Some(sample_backend_runtime_resources()))
                .with_backend_extensions(vec![
                    BackendExtensionSupport::reference(BackendExtensionKind::RmsNorm),
                    BackendExtensionSupport::reference(BackendExtensionKind::RotaryEmbedding),
                ]),
            &model,
            ProviderReadiness::ready("cpu backend ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["allocator_pool"]["policy"]["max_cached_bytes"],
            json!(8 * 1024 * 1024)
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["kernel_cache"]["policy"]["enabled"],
            json!(false)
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["device_memory_budget"]["allocator_pool_budget_bytes"],
            json!(8 * 1024 * 1024)
        );
        assert_eq!(
            encoded["backend_selection"]["backend_extensions"],
            json!([
                {
                    "kind": "rms_norm",
                    "execution": "reference"
                },
                {
                    "kind": "rotary_embedding",
                    "execution": "reference"
                }
            ])
        );
        Ok(())
    }

    #[test]
    fn local_runtime_observability_envelope_serializes_stably()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = LocalRuntimeObservabilityEnvelope::new(LocalRuntimeObservability {
            queue_depth: 0,
            queue_capacity: None,
            active_sessions: 2,
            active_requests: 1,
            memory_footprint: MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 640,
                resident_device_bytes: 0,
            },
            backend_health: vec![mox_runtime::BackendHealthObservation {
                backend: String::from("cpu"),
                status: HealthStatus::Ready,
                message: String::from("cpu backend ready"),
                observed_at_millis: 10,
                changed_at_millis: 10,
            }],
            recent_transitions: vec![
                RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelLoadedCold,
                    "fixture-word-decoder-v0",
                    5,
                ),
                RuntimeTransitionEvent {
                    kind: RuntimeTransitionKind::ModelBecameWarm,
                    model_id: Some(String::from("fixture-word-decoder-v0")),
                    backend: None,
                    previous_status: None,
                    status: None,
                    message: None,
                    observed_at_millis: 9,
                },
            ],
        });

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "mox",
                "observability": {
                    "queue_depth": 0,
                    "active_sessions": 2,
                    "active_requests": 1,
                    "memory_footprint": {
                        "loaded_models": 1,
                        "resident_host_bytes": 640,
                        "resident_device_bytes": 0
                    },
                    "backend_health": [{
                        "backend": "cpu",
                        "status": "Ready",
                        "message": "cpu backend ready",
                        "observed_at_millis": 10,
                        "changed_at_millis": 10
                    }],
                    "recent_transitions": [
                        {
                            "kind": "model_loaded_cold",
                            "model_id": "fixture-word-decoder-v0",
                            "observed_at_millis": 5
                        },
                        {
                            "kind": "model_became_warm",
                            "model_id": "fixture-word-decoder-v0",
                            "observed_at_millis": 9
                        }
                    ]
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
        assert_eq!(
            encoded["backend_selection"]["policy"],
            json!({
                "unavailable": "fallback_to_compatible_backend",
                "degraded": "allow_same_backend"
            })
        );
        assert_eq!(
            encoded["backend_selection"]["selection_state"],
            json!("cross_backend_fallback")
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
    fn cuda_capability_reports_backend_identity_without_amd_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            cuda_backend_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Offline,
                message: String::from(
                    "cuda backend architecture is present but NVIDIA discovery and execution are not landed yet",
                ),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cuda"));
        assert_eq!(
            encoded["backend_selection"]["requested_backend"],
            json!("cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["effective_backend"],
            json!("cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["selected_device"]["device"]["kind"],
            json!("Cuda")
        );
        assert_eq!(encoded["amd"], serde_json::Value::Null);
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
        )
        .with_metrics(EmbeddingMetrics {
            total_duration_ns: Some(50),
            load_duration_ns: Some(5),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        });
        let receipt = ExecutionReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            10,
            20,
        );

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        assert_eq!(receipt.total_duration_ns, Some(50));
        assert_eq!(receipt.load_duration_ns, Some(5));
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: ExecutionReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.runtime_backend, "cpu");
        assert_eq!(decoded.backend_selection.requested_backend, "cpu");
        assert_eq!(decoded.output_vector_count, 1);
        assert_eq!(decoded.input_count, 1);
        assert_eq!(decoded.normalization, EmbeddingNormalization::None);
        assert_eq!(decoded.requested_output_dimensions, None);
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
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                kv_cache: Some(KvCacheAccounting {
                    current: mox_runtime::KvCacheState {
                        tokens: 2,
                        bytes: 64,
                        pages: 1,
                    },
                    growth: mox_runtime::KvCacheGrowth {
                        tokens: 2,
                        bytes: 64,
                        pages: 1,
                    },
                }),
                prefix_tokens_reused: Some(1),
            },
            GenerationProvenance {
                execution_plan_digest: String::from("plan-digest-from-response"),
                load_state: GenerationLoadState::Cold,
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: Some(MemoryResidencySnapshot {
                    loaded_models: 1,
                    resident_host_bytes: 640,
                    resident_device_bytes: 0,
                }),
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                prefix_cache_state: Some(PrefixCacheState::Hit),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: Some(PrefixCacheIdentity {
                    model_id: request.model.model.model_id.clone(),
                    model_revision: request.model.model.revision.clone(),
                    weight_bundle_digest: request.model.weights.digest.clone(),
                    tokenizer_family: request.model.tokenizer_family.clone(),
                    tokenizer_digest: Some(String::from("tokenizer-digest")),
                    chat_template_digest: None,
                    generation_defaults_digest: None,
                    backend_compatibility: String::from("cpu"),
                    prefix_digest: String::from("prefix-digest"),
                    prefix_tokens: 1,
                }),
            },
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
        assert_eq!(
            receipt.execution_plan_digest.as_deref(),
            Some("plan-digest-from-response")
        );
        assert_eq!(receipt.total_duration_ns, Some(75));
        assert_eq!(receipt.load_duration_ns, Some(25));
        assert_eq!(receipt.prompt_eval_duration_ns, Some(15));
        assert_eq!(receipt.eval_duration_ns, Some(60));
        assert_eq!(receipt.load_state, Some(GenerationLoadState::Cold));
        assert_eq!(
            receipt.memory_plan,
            Some(default_decoder_memory_plan(&request.model, None, None))
        );
        assert_eq!(
            receipt.residency_policy,
            Some(ModelResidencyPolicy::default())
        );
        assert_eq!(
            receipt.residency_snapshot,
            Some(MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 640,
                resident_device_bytes: 0,
            })
        );
        assert_eq!(receipt.streaming_policy, None);
        assert_eq!(
            receipt.kv_cache_policy,
            Some(default_decoder_kv_cache_policy(&request.model))
        );
        assert_eq!(
            receipt.kv_cache.as_ref().map(|value| value.current.pages),
            Some(1)
        );
        assert_eq!(receipt.prefix_cache_state, Some(PrefixCacheState::Hit));
        assert_eq!(
            receipt.prefix_cache_policy,
            Some(default_prefix_cache_policy())
        );
        assert_eq!(receipt.prefix_tokens_reused, Some(1));
        assert_eq!(
            receipt
                .prefix_cache_identity
                .as_ref()
                .map(|value| value.prefix_digest.as_str()),
            Some("prefix-digest")
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
    fn streaming_terminal_receipt_preserves_partial_cancellation_output() {
        let request = GenerationRequest::new_text(
            "gen-stream-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-stream-1")),
            "hello",
            GenerationOptions::greedy(4),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![mox_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            TerminationReason::Cancelled,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(10),
                load_duration_ns: Some(2),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(4),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(6),
                kv_cache: None,
                prefix_tokens_reused: Some(0),
            },
            GenerationProvenance {
                execution_plan_digest: String::from("stream-plan"),
                load_state: GenerationLoadState::Cold,
                streaming_policy: Some(default_generation_streaming_policy()),
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: Some(MemoryResidencySnapshot {
                    loaded_models: 1,
                    resident_host_bytes: 640,
                    resident_device_bytes: 0,
                }),
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
            },
        );
        let terminal = GenerationStreamTerminal {
            status: GenerationStreamStatus::Cancelled,
            response,
            failure_reason: Some(String::from("stream cancelled by caller")),
            diagnostic: Some(
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::Cancelled,
                    499,
                    "stream cancelled by caller",
                )
                .with_product_id(request.product_id.clone())
                .with_model_id(request.model.model.model_id.clone())
                .with_backend("cpu"),
            ),
        };

        let receipt = TextGenerationReceipt::from_stream_terminal(
            cpu_backend_selection(),
            &request,
            &terminal,
            "stream-plan",
            5,
            12,
        );

        assert_eq!(receipt.status, ReceiptStatus::Cancelled);
        assert_eq!(receipt.output_tokens, 1);
        assert_eq!(
            receipt.streaming_policy,
            Some(default_generation_streaming_policy())
        );
        assert_eq!(
            receipt.failure_reason.as_deref(),
            Some("stream cancelled by caller")
        );
        assert_eq!(
            receipt.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::Cancelled)
        );
    }

    #[test]
    fn failed_receipt_carries_reason() -> Result<(), Box<dyn std::error::Error>> {
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
        )
        .with_diagnostic(
            LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::BackendUnavailable,
                503,
                "backend offline",
            )
            .with_product_id(request.product_id.clone())
            .with_model_id(request.model.model.model_id.clone())
            .with_backend("metal"),
        );
        assert_eq!(receipt.status, ReceiptStatus::Failed);
        assert_eq!(receipt.input_count, 1);
        assert_eq!(receipt.normalization, EmbeddingNormalization::None);
        assert_eq!(receipt.requested_output_dimensions, None);
        assert_eq!(receipt.failure_reason.as_deref(), Some("backend offline"));
        assert_eq!(
            receipt.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::BackendUnavailable)
        );
        assert_eq!(receipt.runtime_backend, "cpu");
        assert_eq!(receipt.backend_selection.requested_backend, "metal");
        assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
        let encoded = serde_json::to_string(&receipt)?;
        let decoded = serde_json::from_str::<ExecutionReceipt>(&encoded)?;
        assert_eq!(
            decoded.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::BackendUnavailable)
        );
        Ok(())
    }

    #[test]
    fn amd_receipt_carries_execution_context() -> Result<(), Box<dyn std::error::Error>> {
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
        let Some(amd) = receipt.amd else {
            return Err("amd context missing".into());
        };
        assert_eq!(amd.mode, AmdRuntimeMode::Userspace);
        assert!(amd.risk.requires_explicit_opt_in);
        assert_eq!(amd.recovery.driver_binding, AmdDriverBinding::KernelAmdgpu);
        assert_eq!(receipt.input_count, 1);
        assert_eq!(receipt.normalization, EmbeddingNormalization::None);
        Ok(())
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
    fn embedding_request_digest_changes_when_output_dimensions_change() {
        let request = EmbeddingRequest::new(
            "req-embed-digest-1",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let truncated = request.clone().with_output_dimensions(4);

        assert_ne!(
            digest_embedding_request(&request),
            digest_embedding_request(&truncated)
        );
    }

    #[test]
    fn generation_request_digest_changes_when_options_change() {
        let request = GenerationRequest::new_tokens(
            "gen-7",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000007")),
            TokenSequence::new(vec![mox_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::sample(2),
        );
        let baseline = digest_generation_request(&request);

        let mut with_seed = request.clone();
        with_seed.options.seed = Some(17);
        assert_ne!(digest_generation_request(&with_seed), baseline);

        let mut with_temperature = request.clone();
        with_temperature.options.temperature = Some(0.7);
        assert_ne!(digest_generation_request(&with_temperature), baseline);

        let mut with_stop = request;
        with_stop.options.stop_sequences = vec![String::from("</end>")];
        assert_ne!(digest_generation_request(&with_stop), baseline);
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

    fn sample_backend_runtime_resources() -> BackendRuntimeResources {
        BackendRuntimeResources {
            allocator_pool: AllocatorPoolReport {
                policy: AllocatorPoolPolicy::exact_tensor_spec(64, 8 * 1024 * 1024),
                state: AllocatorPoolState {
                    cached_buffers: 3,
                    cached_bytes: 4096,
                },
            },
            kernel_cache: KernelCacheReport {
                policy: KernelCachePolicy::disabled(),
                state: KernelCacheState::default(),
            },
            device_memory_budget: Some(DeviceMemoryBudget::new(
                Some(16 * 1024 * 1024 * 1024),
                8 * 1024 * 1024,
                0,
            )),
        }
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
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ4_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ4_1,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ8_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
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

    fn cuda_backend_selection() -> BackendSelection {
        BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
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

    fn sample_cuda_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("NVIDIA CUDA Test Device")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
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
