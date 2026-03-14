//! Served compute product contracts for Psionic.

#![allow(
    clippy::assigning_clones,
    clippy::large_enum_variant,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or
)]
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::useless_vec
    )
)]

mod conformance;
mod gguf;
mod gpt_oss;
mod openai_http;

use std::{
    collections::{BTreeMap, VecDeque},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub use conformance::*;
pub use gguf::*;
pub use gpt_oss::*;
pub use openai_http::*;
pub use psionic_adapters::*;
use psionic_backend_cpu::CpuBackend;
use psionic_backend_cuda::{
    CudaBackend, EMBEDDINGS_SUPPORTED_OPS as CUDA_EMBEDDINGS_SUPPORTED_OPS,
};
use psionic_backend_metal::{
    EMBEDDINGS_SUPPORTED_OPS, MetalBackend, TEXT_GENERATION_SUPPORTED_OPS,
};
use psionic_compiler::{CompileError, compile_graph};
pub use psionic_core::QuantizationMode;
use psionic_core::{DType, Device, Shape, TensorId};
use psionic_ir::{Graph, GraphBuilder, GraphError};
pub use psionic_models::{
    ActivationFunction, ArtifactWordDecoder, ByteProjectionEmbedder, ContextOverflowPolicy,
    ContextWindowAccounting, ContextWindowError, DecoderAttentionConfig, DecoderBlockConfig,
    DecoderConfig, DecoderFeedForwardConfig, DecoderFixtureWeights, DecoderModelDescriptor,
    DecoderWeightLoader, EmbeddingModelDescriptor, EmbeddingNormalization, EmbeddingWeights,
    FixtureDecoderLoader, FixtureWordTokenizer, GgufDecoderAdapter, GgufDecoderAdapterLoader,
    GgufDecoderFamily, GgufDecoderFamilyMetadata, GgufDecoderLayerTensorLayout,
    GgufDecoderTensorLayout, GgufEmbeddingAdapter, GgufEmbeddingAdapterLoader, GgufEmbeddingFamily,
    GgufEmbeddingFamilyMetadata, GgufEmbeddingLayerTensorLayout, GgufEmbeddingPooling,
    GgufEmbeddingTensorLayout, GgufPromptTemplateFamily, GgufPromptTemplateRenderer,
    GptOssHarmonyParsedOutput, ModelArtifactGovernance, ModelArtifactLicenseEntry,
    ModelArtifactLicenseFacts, ModelArtifactProvenance, ModelArtifactProvenanceKind,
    ModelDescriptor, ModelLoadError, PromptMessage, PromptMessageRole, PromptRenderError,
    ReferenceWordDecoder, RenderedPrompt, SmokeByteEmbedder, TokenId, TokenSequence,
    TokenVocabulary, TokenizerBoundary, WeightArtifactMetadata, WeightBundleMetadata, WeightFormat,
    WeightSource, WeightTensorMetadata, apply_context_window, digest_generation_defaults,
};
use psionic_runtime::{
    BackendHealthTracker, BackendSelection, BackendSelectionState, BackendToolchainIdentity,
    CacheAction, CacheInvalidationPolicy, CacheInvalidationTrigger, CacheKind, CacheObservation,
    ClusterExecutionContext, CompilePathEvidence, DeviceDiscovery, ExecutionCapabilityProfile,
    ExecutionDeliveryProof, GenerationSchedulerMetrics, GenerationSchedulerPolicy,
    GenerationSchedulerRequestReceipt, GenerationSchedulingClass, HealthStatus, KvCacheAccounting,
    KvCacheDeviceScope, KvCacheOwnerBinding, KvCacheOwnerClass, KvCacheOwnershipAccounting,
    KvCachePageLayout, KvCachePageSpan, KvCachePolicy, KvCacheSchedulerBinding, KvCacheSpillPolicy,
    KvCacheState, KvResidencyAccounting, KvResidencyMovement, KvResidencyMovementKind,
    KvResidencyRefusal, KvResidencyRefusalReason, KvResidencyTier, KvResidencyTierState,
    LoadedModelMemoryState, LoadedModelResidency, LocalRuntimeDiagnostic, LocalRuntimeErrorCode,
    LocalRuntimeObservability, LocalServingIsolationPolicy, MemoryResidencySnapshot,
    ModelAdmissionRefusal, ModelMemoryPlan, ModelResidencyPolicy, PrefillDecodeCapability,
    PrefillDecodeExecutionMode, PrefillDecodeHandoff, PrefixCacheControl, PrefixCacheIdentity,
    PrefixCacheMode, PrefixCacheRefusalReason, PrefixCacheReusePolicy, PrefixCacheState,
    QuantizationDispatchDecision, QuantizationDispatchRequest, QuantizationDispatchWorkload,
    RuntimeError, RuntimeTransitionEvent, RuntimeTransitionKind, RuntimeTransitionLog,
    SamplingPolicy, SamplingStrategy, ServedArtifactIdentity, ShardedModelManifest,
    ShardedModelManifestError, StructuredOutputError, StructuredOutputExecutionReport,
    StructuredOutputMatchStatus, StructuredOutputMatcher, StructuredOutputRequest,
    StructuredOutputValue, TokenSampler, default_cache_invalidation_policy, plan_model_admission,
    select_argmax_token,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "request and response types for served products";

/// Phase-0 embeddings product identifier.
pub const EMBEDDINGS_PRODUCT_ID: &str = "psionic.embeddings";

/// Phase-1 text-generation product identifier.
pub const TEXT_GENERATION_PRODUCT_ID: &str = "psionic.text_generation";

/// Adapter-backed text-generation product identifier.
pub const ADAPTER_TEXT_GENERATION_PRODUCT_ID: &str = "psionic.adapter_text_generation";

/// Default logical page width for reference-path paged KV state.
pub const DEFAULT_KV_PAGE_TOKENS: usize = 16;

/// Returns the default paged-KV policy for a decoder geometry.
#[must_use]
pub fn default_kv_cache_policy(max_context: usize, width: usize) -> KvCachePolicy {
    let bytes_per_token = width
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<f32>());
    KvCachePolicy {
        device_scope: KvCacheDeviceScope::SameDeviceOnly,
        spill_policy: KvCacheSpillPolicy::RefuseNewPages,
        page_layout: KvCachePageLayout::new(
            max_context,
            DEFAULT_KV_PAGE_TOKENS.min(max_context.max(1)),
            bytes_per_token,
        ),
    }
}

/// Returns the default paged-KV policy for a decoder descriptor.
#[must_use]
pub fn default_decoder_kv_cache_policy(model: &DecoderModelDescriptor) -> KvCachePolicy {
    default_kv_cache_policy(model.config.max_context, model.config.kv_width())
}

/// Returns the default paged-KV policy for a loaded generation model handle.
#[must_use]
pub fn default_generation_kv_cache_policy<M>(model: &M) -> KvCachePolicy
where
    M: GenerationModelHandle,
{
    default_kv_cache_policy(model.descriptor().config.max_context, model.cache_width())
}

/// Returns the default resident-memory plan for one decoder model.
#[must_use]
pub fn default_decoder_memory_plan(
    model: &DecoderModelDescriptor,
    weight_bytes: Option<u64>,
    resident_device_bytes: Option<u64>,
) -> ModelMemoryPlan {
    let kv_cache_bytes = default_decoder_kv_cache_policy(model)
        .page_layout
        .bytes_for_tokens(model.config.max_context);
    let weights_bytes = weight_bytes.unwrap_or(0);
    match resident_device_bytes {
        // Accelerated runtimes should override this split with a more exact
        // host/device breakdown when they know it; this keeps the current CPU
        // truth correct without double-counting device-resident models.
        Some(device_bytes) => ModelMemoryPlan::split_residency(
            weights_bytes,
            kv_cache_bytes,
            0,
            kv_cache_bytes,
            device_bytes,
        ),
        None => ModelMemoryPlan::host_only(weights_bytes, kv_cache_bytes, 0),
    }
}

fn default_generation_memory_plan<M>(
    model: &M,
    weight_bytes: Option<u64>,
    resident_device_bytes: Option<u64>,
) -> ModelMemoryPlan
where
    M: GenerationModelHandle,
{
    let kv_cache_bytes = default_generation_kv_cache_policy(model)
        .page_layout
        .bytes_for_tokens(model.descriptor().config.max_context);
    let weights_bytes = weight_bytes.unwrap_or(0);
    match resident_device_bytes {
        Some(device_bytes) => ModelMemoryPlan::split_residency(
            weights_bytes,
            kv_cache_bytes,
            0,
            kv_cache_bytes,
            device_bytes,
        ),
        None => ModelMemoryPlan::host_only(weights_bytes, kv_cache_bytes, 0),
    }
}

/// Slow-reader handling for the local streaming API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingBackpressurePolicy {
    /// Generation advances only when the caller pulls the next stream event.
    PullDriven,
}

/// Dropped-client handling for the local streaming API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingDisconnectPolicy {
    /// Abort generation and discard uncommitted output when the client disconnects.
    AbortGeneration,
}

/// Explicit cancellation handling for the local streaming API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingCancellationPolicy {
    /// Abort generation and discard uncommitted output after the current token step.
    AbortAfterCurrentToken,
}

/// Explicit streaming policy for in-process local generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationStreamingPolicy {
    /// Slow-reader / backpressure behavior.
    pub backpressure: StreamingBackpressurePolicy,
    /// Dropped-client behavior.
    pub disconnect: StreamingDisconnectPolicy,
    /// Explicit cancellation behavior.
    pub cancellation: StreamingCancellationPolicy,
}

/// Returns the default streaming policy for local Psionic serving.
#[must_use]
pub fn default_generation_streaming_policy() -> GenerationStreamingPolicy {
    GenerationStreamingPolicy {
        backpressure: StreamingBackpressurePolicy::PullDriven,
        disconnect: StreamingDisconnectPolicy::AbortGeneration,
        cancellation: StreamingCancellationPolicy::AbortAfterCurrentToken,
    }
}

/// Returns the default shared prompt-prefix reuse policy for local Psionic serving.
#[must_use]
pub fn default_prefix_cache_policy() -> PrefixCacheReusePolicy {
    PrefixCacheReusePolicy {
        shared_across_sessions: true,
        shared_across_users: false,
        shared_across_models: false,
        shared_across_backends: false,
        shared_across_sampler_settings: false,
    }
}

/// Returns the default execution profile for current local text generation.
#[must_use]
pub fn default_text_generation_execution_profile() -> ExecutionCapabilityProfile {
    ExecutionCapabilityProfile::single_request_latency_optimized()
        .with_prefill_decode_capability(PrefillDecodeCapability::colocated_split().with_detail(
            "local text generation separates prompt-prefill and decode inside one runtime-owned KV seam",
        ))
}

/// Returns the default scheduler policy for shared continuous-batching generation.
#[must_use]
pub fn default_generation_scheduler_policy() -> GenerationSchedulerPolicy {
    GenerationSchedulerPolicy::default()
}

/// Returns the execution profile for the first shared continuous-batching path.
#[must_use]
pub fn continuous_batch_text_generation_execution_profile() -> ExecutionCapabilityProfile {
    ExecutionCapabilityProfile::continuous_batch_throughput_optimized(
        &default_generation_scheduler_policy(),
    )
    .with_prefill_decode_capability(PrefillDecodeCapability::colocated_split().with_detail(
        "continuous batching keeps prompt-prefill and decode as distinct co-located scheduler phases",
    ))
}

fn session_kv_owner(model_id: &str, session_id: &SessionId) -> KvCacheOwnerBinding {
    KvCacheOwnerBinding::new(
        KvCacheOwnerClass::Session,
        session_id.as_str(),
        model_id.to_string(),
    )
    .with_session_id(session_id.as_str())
}

fn request_kv_owner(
    request: &GenerationRequest,
    batch_posture: psionic_runtime::BatchExecutionPosture,
    queue_depth_at_admission: Option<usize>,
) -> KvCacheOwnerBinding {
    let owner = KvCacheOwnerBinding::new(
        KvCacheOwnerClass::Request,
        request.request_id.clone(),
        request.model.model.model_id.clone(),
    )
    .with_scheduler(KvCacheSchedulerBinding {
        batch_posture,
        queue_depth_at_admission,
    });
    if let Some(session_id) = &request.session_id {
        owner.with_session_id(session_id.as_str())
    } else {
        owner
    }
}

fn shared_prefix_kv_owner(identity: &PrefixCacheIdentity) -> KvCacheOwnerBinding {
    KvCacheOwnerBinding::new(
        KvCacheOwnerClass::SharedPrefix,
        identity.prefix_digest.clone(),
        identity.model_id.clone(),
    )
}

fn generation_product_supported(request: &GenerationRequest) -> bool {
    request.product_id == TEXT_GENERATION_PRODUCT_ID
        || (request.product_id == ADAPTER_TEXT_GENERATION_PRODUCT_ID
            && request.adapter_serving.is_some())
}

fn effective_generation_served_artifact_digest(
    served_artifact: &ServedArtifactIdentity,
    adapter_serving: Option<&AdapterServingBinding>,
) -> String {
    adapter_serving
        .map(|binding| binding.served_adapter_digest.clone())
        .unwrap_or_else(|| served_artifact.served_artifact_digest.clone())
}

/// Returns the default execution profile for current local embeddings.
#[must_use]
pub fn default_embeddings_execution_profile() -> ExecutionCapabilityProfile {
    ExecutionCapabilityProfile::caller_static_batch_balanced()
}

/// Returns the current runtime-owned quantization dispatch recommendation for generation work.
#[must_use]
pub fn recommended_generation_quantization_dispatch(
    quantization: QuantizationMode,
    logical_tokens: usize,
    matrix_columns: usize,
    grouped_experts: bool,
) -> QuantizationDispatchDecision {
    let workload = if grouped_experts {
        QuantizationDispatchWorkload::GroupedExpert
    } else if logical_tokens <= 1 {
        QuantizationDispatchWorkload::LatencyCriticalDecode
    } else {
        QuantizationDispatchWorkload::BatchedPrefill
    };
    QuantizationDispatchDecision::advise(
        &QuantizationDispatchRequest::new(quantization, workload, logical_tokens, matrix_columns)
            .with_native_quantized_kernels(quantization != QuantizationMode::None)
            .with_grouped_dispatch(grouped_experts),
    )
}

/// Returns the current runtime-owned cache invalidation policy.
#[must_use]
pub fn cache_invalidation_policy() -> CacheInvalidationPolicy {
    default_cache_invalidation_policy()
}

fn default_generation_defaults_digest() -> String {
    digest_generation_defaults(false, false, &[])
}

fn backend_toolchain_identity(
    runtime_backend: &str,
    compiled_backend_features: &[String],
) -> BackendToolchainIdentity {
    let mut compiled_backend_features = compiled_backend_features.to_vec();
    compiled_backend_features.sort();
    compiled_backend_features.dedup();
    BackendToolchainIdentity::new(
        runtime_backend,
        format!("{runtime_backend}@{}", env!("CARGO_PKG_VERSION")),
        compiled_backend_features,
    )
}

fn served_artifact_identity_from_parts(
    model_id: &str,
    model_revision: &str,
    weights: &WeightBundleMetadata,
    runtime_backend: &str,
    compiled_backend_features: &[String],
    artifact_identity: Option<&psionic_models::ServedModelArtifactMetadata>,
) -> ServedArtifactIdentity {
    ServedArtifactIdentity::new(
        model_id,
        model_revision,
        weights.digest.clone(),
        artifact_identity.and_then(|value| value.model_blob_digest.clone()),
        artifact_identity.and_then(|value| value.tokenizer_digest.clone()),
        artifact_identity.and_then(|value| value.chat_template_digest.clone()),
        artifact_identity
            .map(|value| value.generation_defaults_digest.clone())
            .unwrap_or_else(default_generation_defaults_digest),
        weights.format.identity_label(),
        weights.quantization,
        backend_toolchain_identity(runtime_backend, compiled_backend_features),
    )
}

/// Returns the served-artifact identity for an embeddings descriptor and backend selection.
#[must_use]
pub fn served_artifact_identity_for_embedding_model(
    model: &EmbeddingModelDescriptor,
    backend_selection: &BackendSelection,
) -> ServedArtifactIdentity {
    served_artifact_identity_from_parts(
        model.model.model_id.as_str(),
        model.model.revision.as_str(),
        &model.weights,
        backend_selection.effective_backend.as_str(),
        &[],
        model.artifact_identity.as_ref(),
    )
}

/// Returns the served-artifact identity for a decoder descriptor and backend selection.
#[must_use]
pub fn served_artifact_identity_for_decoder_model(
    model: &DecoderModelDescriptor,
    backend_selection: &BackendSelection,
) -> ServedArtifactIdentity {
    served_artifact_identity_from_parts(
        model.model.model_id.as_str(),
        model.model.revision.as_str(),
        &model.weights,
        backend_selection.effective_backend.as_str(),
        &[],
        model.artifact_identity.as_ref(),
    )
}

fn served_artifact_identity_for_decoder_backend(
    model: &DecoderModelDescriptor,
    runtime_backend: &str,
    compiled_backend_features: &[String],
) -> ServedArtifactIdentity {
    served_artifact_identity_from_parts(
        model.model.model_id.as_str(),
        model.model.revision.as_str(),
        &model.weights,
        runtime_backend,
        compiled_backend_features,
        model.artifact_identity.as_ref(),
    )
}

/// Failure while loading one sharded-model manifest document for served products.
#[derive(Debug, Error)]
pub enum ShardedModelManifestLoadError {
    /// The manifest file could not be read.
    #[error("failed to read sharded model manifest `{path}`: {message}")]
    Read {
        /// Manifest path that failed.
        path: String,
        /// Underlying OS or I/O message.
        message: String,
    },
    /// The manifest file could not be decoded as JSON.
    #[error("failed to decode sharded model manifest `{path}`: {message}")]
    Decode {
        /// Manifest path that failed.
        path: String,
        /// Underlying JSON decode message.
        message: String,
    },
    /// The manifest decoded but failed structural validation.
    #[error("invalid sharded model manifest `{path}`: {error}")]
    Invalid {
        /// Manifest path that failed.
        path: String,
        /// Structural validation failure.
        error: ShardedModelManifestError,
    },
}

/// Loads and validates one sharded-model manifest from JSON on disk.
pub fn load_sharded_model_manifest_json(
    path: impl AsRef<std::path::Path>,
) -> Result<ShardedModelManifest, ShardedModelManifestLoadError> {
    let path = path.as_ref();
    let path_label = path.display().to_string();
    let contents =
        std::fs::read_to_string(path).map_err(|error| ShardedModelManifestLoadError::Read {
            path: path_label.clone(),
            message: error.to_string(),
        })?;
    let manifest = serde_json::from_str::<ShardedModelManifest>(&contents).map_err(|error| {
        ShardedModelManifestLoadError::Decode {
            path: path_label.clone(),
            message: error.to_string(),
        }
    })?;
    manifest
        .validate()
        .map_err(|error| ShardedModelManifestLoadError::Invalid {
            path: path_label,
            error,
        })?;
    Ok(manifest)
}

fn paged_tensor_cache_observation(weights: &WeightBundleMetadata) -> CacheObservation {
    if weights.is_artifact_backed() {
        CacheObservation::new(
            CacheKind::PagedTensorStorage,
            CacheAction::Restore,
            "artifact-backed tensor storage is reopened from local model bytes instead of trusted by display name",
        )
    } else {
        CacheObservation::new(
            CacheKind::PagedTensorStorage,
            CacheAction::Bypass,
            "fixture-backed weights do not use reusable paged tensor storage",
        )
    }
}

fn compile_path_cache_observations(
    compile_path: Option<&CompilePathEvidence>,
    fallback_load_state: Option<GenerationLoadState>,
) -> Vec<CacheObservation> {
    if let Some(compile_path) = compile_path {
        return vec![
            compile_path.execution_plan_cache.clone(),
            compile_path.kernel_cache.clone(),
        ];
    }

    let execution_plan_cache = match fallback_load_state {
        Some(GenerationLoadState::Cold) => CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Rebuild,
            "compile-path evidence was unavailable so the request fell back to model-load cold-path truth",
        ),
        Some(GenerationLoadState::Warm) => CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Reuse,
            "compile-path evidence was unavailable so the request fell back to model-load warm-path truth",
        ),
        None => CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Bypass,
            "request did not surface execution-plan compile-path evidence",
        ),
    };
    vec![
        execution_plan_cache,
        CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Bypass,
            "request did not surface explicit kernel-cache behavior",
        ),
    ]
}

fn build_delivery_proof(
    execution_plan_digest: String,
    kernel_count: usize,
    bytes_moved: u64,
    plan_cache_hits: usize,
    plan_cache_misses: usize,
    kv_growth: Option<psionic_runtime::KvCacheGrowth>,
    prefill_decode_handoff: Option<PrefillDecodeHandoff>,
    kv_residency: Option<KvResidencyAccounting>,
) -> Option<ExecutionDeliveryProof> {
    if kernel_count == 0
        && bytes_moved == 0
        && plan_cache_hits == 0
        && plan_cache_misses == 0
        && kv_growth.is_none()
        && prefill_decode_handoff.is_none()
        && kv_residency.is_none()
    {
        return None;
    }
    Some(ExecutionDeliveryProof {
        execution_plan_digest,
        kernel_count,
        bytes_moved,
        plan_cache_hits,
        plan_cache_misses,
        kv_growth,
        prefill_decode_handoff,
        kv_residency,
    })
}

fn elapsed_ns(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn average_inter_token_latency_ns(
    first_token_at: Option<Instant>,
    last_token_at: Option<Instant>,
    output_tokens: usize,
) -> Option<u64> {
    (output_tokens > 1)
        .then_some((first_token_at?, last_token_at?))
        .and_then(|(first_token_at, last_token_at)| {
            let interval_count = output_tokens.saturating_sub(1);
            let elapsed = last_token_at.duration_since(first_token_at).as_nanos();
            let average = elapsed / interval_count as u128;
            average.try_into().ok()
        })
}

fn local_prefill_decode_handoff(prefill_kv_state: &KvCacheState) -> Option<PrefillDecodeHandoff> {
    (prefill_kv_state.tokens > 0 || prefill_kv_state.pages > 0 || prefill_kv_state.bytes > 0)
        .then(|| PrefillDecodeHandoff::colocated(prefill_kv_state.pages, prefill_kv_state.bytes))
}

fn host_only_kv_residency(
    policy: &KvCachePolicy,
    host_state: KvCacheState,
) -> Option<KvResidencyAccounting> {
    (host_state.tokens > 0 || host_state.pages > 0 || host_state.bytes > 0).then(|| {
        KvResidencyAccounting::from_policy(policy).with_tier(
            KvResidencyTierState::resident(KvResidencyTier::Host, host_state)
                .with_detail("paged KV remained resident only in the host-owned cache"),
        )
    })
}

fn host_device_kv_residency(
    policy: &KvCachePolicy,
    host_state: KvCacheState,
    device_state: KvCacheState,
    prefetched_from_host: bool,
    write_back_growth: Option<psionic_runtime::KvCacheGrowth>,
) -> Option<KvResidencyAccounting> {
    let has_any_state = host_state.tokens > 0
        || host_state.pages > 0
        || host_state.bytes > 0
        || device_state.tokens > 0
        || device_state.pages > 0
        || device_state.bytes > 0;
    has_any_state.then(|| {
        let prefetched_pages = device_state.pages;
        let prefetched_bytes = device_state.bytes;
        let mut accounting = KvResidencyAccounting::from_policy(policy)
            .with_tier(
                KvResidencyTierState::resident(KvResidencyTier::Host, host_state).with_detail(
                    "host-owned KV mirror remained available for session or prefix continuity",
                ),
            )
            .with_tier(
                KvResidencyTierState::resident(KvResidencyTier::Device, device_state)
                    .with_detail("device-resident KV state backed active GPU decode"),
            );
        if prefetched_from_host && prefetched_bytes > 0 {
            accounting = accounting.with_movement(
                KvResidencyMovement::new(
                    KvResidencyMovementKind::Prefetch,
                    KvResidencyTier::Host,
                    KvResidencyTier::Device,
                    prefetched_pages,
                    prefetched_bytes,
                )
                .with_detail("host-resident KV state was prefetched into the active device tier"),
            );
        }
        if let Some(write_back_growth) = write_back_growth {
            if write_back_growth.tokens > 0
                || write_back_growth.pages > 0
                || write_back_growth.bytes > 0
            {
                accounting = accounting.with_movement(
                    KvResidencyMovement::new(
                        KvResidencyMovementKind::WriteBack,
                        KvResidencyTier::Device,
                        KvResidencyTier::Host,
                        write_back_growth.pages,
                        write_back_growth.bytes,
                    )
                    .with_detail(
                        "device-produced KV growth was written back into the host-owned cache",
                    ),
                );
            }
        }
        if matches!(policy.spill_policy, KvCacheSpillPolicy::SpillToHost)
            && !matches!(policy.device_scope, KvCacheDeviceScope::CrossDeviceExplicit)
        {
            accounting = accounting.with_refusal(KvResidencyRefusal::new(
                KvResidencyRefusalReason::SpillUnsupported,
                "spill-to-host requires explicit cross-device KV movement support",
            ));
        }
        accounting
    })
}

fn accumulate_generation_step_counters(
    step: &GenerationStepOutput,
    execution_plan_digest: &mut Option<String>,
    compile_path: &mut Option<CompilePathEvidence>,
    kernel_count: &mut usize,
    bytes_moved: &mut u64,
    plan_cache_hits: &mut usize,
    plan_cache_misses: &mut usize,
    gpt_oss_perf: &mut Option<GptOssPerformanceMetrics>,
) {
    if execution_plan_digest.is_none() {
        *execution_plan_digest = step.execution_plan_digest.clone();
    }
    if compile_path.is_none() {
        *compile_path = step.compile_path.clone();
    }
    *kernel_count = kernel_count.saturating_add(step.kernel_count);
    *bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
    *plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
    *plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
    if let Some(step_perf) = &step.gpt_oss_perf {
        gpt_oss_perf
            .get_or_insert_with(GptOssPerformanceMetrics::default)
            .accumulate(step_perf);
    }
}

fn accumulate_optional_gpt_oss_perf(
    gpt_oss_perf: &mut Option<GptOssPerformanceMetrics>,
    step_perf: Option<&GptOssPerformanceMetrics>,
) {
    if let Some(step_perf) = step_perf {
        gpt_oss_perf
            .get_or_insert_with(GptOssPerformanceMetrics::default)
            .accumulate(step_perf);
    }
}

fn prefix_cache_observation(
    prefix_state: PrefixCacheState,
    invalidation_trigger: Option<CacheInvalidationTrigger>,
) -> CacheObservation {
    let observation = match prefix_state {
        PrefixCacheState::None => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Bypass,
            "no compatible shared prefix entry existed for this prompt",
        ),
        PrefixCacheState::Hit => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Reuse,
            "compatible shared prefix state was reused",
        ),
        PrefixCacheState::Miss => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Rebuild,
            "shared prefix reuse missed and a fresh entry was recorded",
        ),
        PrefixCacheState::Bypassed => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Bypass,
            "shared prefix reuse was skipped because request-owned KV state already existed",
        ),
        PrefixCacheState::Rebuilt => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Rebuild,
            "stale shared prefix state was discarded and rebuilt",
        ),
    };
    if let Some(invalidation_trigger) = invalidation_trigger {
        observation.with_trigger(invalidation_trigger)
    } else {
        observation
    }
}

fn kv_state_observation(
    session_id: Option<&SessionId>,
    reset_session: bool,
    previous_kv_state: &KvCacheState,
) -> CacheObservation {
    if reset_session {
        return CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Invalidate,
            "existing session KV state was discarded before execution",
        )
        .with_trigger(CacheInvalidationTrigger::ExplicitReset);
    }
    if session_id.is_none() {
        return CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Bypass,
            "request did not target session-bound KV reuse",
        );
    }
    if previous_kv_state.tokens > 0 {
        CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Reuse,
            "compatible session KV state was reused",
        )
    } else {
        CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Bypass,
            "session existed but no retained KV state was available to reuse",
        )
    }
}

fn generation_cache_observations(
    model: &DecoderModelDescriptor,
    compile_path: Option<&CompilePathEvidence>,
    load_state: GenerationLoadState,
    session_id: Option<&SessionId>,
    reset_session: bool,
    previous_kv_state: &KvCacheState,
    prefix_state: PrefixCacheState,
    prefix_invalidation_trigger: Option<CacheInvalidationTrigger>,
) -> Vec<CacheObservation> {
    let mut observations = compile_path_cache_observations(compile_path, Some(load_state));
    observations.push(paged_tensor_cache_observation(&model.weights));
    observations.push(prefix_cache_observation(
        prefix_state,
        prefix_invalidation_trigger,
    ));
    observations.push(kv_state_observation(
        session_id,
        reset_session,
        previous_kv_state,
    ));
    observations
}

/// Returns the current cache observations surfaced for embeddings execution receipts.
#[must_use]
pub fn cache_observations_for_embedding_model(
    model: &EmbeddingModelDescriptor,
    compile_path: Option<&CompilePathEvidence>,
) -> Vec<CacheObservation> {
    let mut observations = compile_path_cache_observations(compile_path, None);
    observations.push(paged_tensor_cache_observation(&model.weights));
    observations
}

/// Embeddings request contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Stable client-provided request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Embeddings model descriptor.
    pub model: EmbeddingModelDescriptor,
    /// UTF-8 text inputs to embed.
    pub inputs: Vec<String>,
    /// Requested output dimensions when the caller wants a truncated vector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimensions: Option<usize>,
}

impl EmbeddingRequest {
    /// Creates an embeddings request for the default Psionic product.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        model: EmbeddingModelDescriptor,
        inputs: Vec<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            model,
            inputs,
            output_dimensions: None,
        }
    }

    /// Requests truncated output vectors when the model supports that behavior.
    #[must_use]
    pub fn with_output_dimensions(mut self, output_dimensions: usize) -> Self {
        self.output_dimensions = Some(output_dimensions);
        self
    }
}

/// Individual embeddings vector payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingVector {
    /// Input index in the request.
    pub index: usize,
    /// Embedding values.
    pub values: Vec<f32>,
}

/// Response metadata for embeddings execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingResponseMetadata {
    /// Stable output dimensionality.
    pub dimensions: usize,
    /// Number of returned vectors.
    pub vector_count: usize,
    /// Number of inputs carried by the request.
    pub input_count: usize,
    /// Model identifier used during execution.
    pub model_id: String,
    /// Model family used during execution.
    pub model_family: String,
    /// Model revision used during execution.
    pub model_revision: String,
    /// Normalization policy applied by the model.
    pub normalization: EmbeddingNormalization,
    /// Requested output dimensions when the caller asked for truncated vectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_output_dimensions: Option<usize>,
}

/// Explicit timing metrics for one embeddings call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EmbeddingMetrics {
    /// End-to-end embeddings duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Prompt-token count surfaced when the backend can expose it honestly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<usize>,
    /// Prompt-evaluation duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration_ns: Option<u64>,
}

/// Embeddings response contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Returned embeddings vectors.
    pub embeddings: Vec<EmbeddingVector>,
    /// Metadata describing the outputs.
    pub metadata: EmbeddingResponseMetadata,
    /// Explicit timing metrics for the request path, when known.
    #[serde(default, skip_serializing_if = "EmbeddingMetrics::is_empty")]
    pub metrics: EmbeddingMetrics,
    /// Explicit execution provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<EmbeddingProvenance>,
}

impl EmbeddingResponse {
    /// Creates an embeddings response from vectors and request metadata.
    #[must_use]
    pub fn new(request: &EmbeddingRequest, embeddings: Vec<EmbeddingVector>) -> Self {
        let requested_output_dimensions = canonical_embedding_output_dimensions(
            request.output_dimensions,
            request.model.dimensions,
        );
        let dimensions = embeddings
            .first()
            .map(|vector| vector.values.len())
            .unwrap_or_else(|| requested_output_dimensions.unwrap_or(request.model.dimensions));
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            metadata: EmbeddingResponseMetadata {
                dimensions,
                vector_count: embeddings.len(),
                input_count: request.inputs.len(),
                model_id: request.model.model.model_id.clone(),
                model_family: request.model.model.family.clone(),
                model_revision: request.model.model.revision.clone(),
                normalization: request.model.normalization,
                requested_output_dimensions,
            },
            embeddings,
            metrics: EmbeddingMetrics::default(),
            provenance: None,
        }
    }

    /// Attaches explicit metrics to an embeddings response.
    #[must_use]
    pub fn with_metrics(mut self, metrics: EmbeddingMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Attaches explicit metrics and provenance to an embeddings response.
    #[must_use]
    pub fn with_metrics_and_provenance(
        mut self,
        metrics: EmbeddingMetrics,
        provenance: EmbeddingProvenance,
    ) -> Self {
        self.metrics = metrics;
        self.provenance = Some(provenance);
        self
    }
}

/// Provenance fields attached to one embeddings response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingProvenance {
    /// Stable execution-plan digest for the active model graph.
    pub execution_plan_digest: String,
    /// Explicit clustered execution or scheduling context for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// Explicit warm/cold compile-path evidence for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Delivery-proof facts surfaced by the local runtime for this request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Explicit cache actions observed for the request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_observations: Vec<CacheObservation>,
}

impl EmbeddingProvenance {
    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        self.cluster_execution = Some(cluster_execution);
        self
    }
}

/// Minimal embeddings execution interface.
pub trait EmbeddingsExecutor {
    /// Error returned when embedding execution fails.
    type Error;

    /// Executes an embeddings request.
    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error>;
}

/// Stable generation session identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Creates a session identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Prompt input boundary for text generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GenerationInput {
    /// Prompt text that still needs tokenization.
    Text(String),
    /// Pre-tokenized prompt IDs.
    Tokens(TokenSequence),
}

/// Decode strategy for text generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStrategy {
    /// Greedy argmax decode.
    Greedy,
    /// Probability sampling with optional temperature, top-k, and top-p controls.
    Sample,
}

/// Generation options for the phase-1 reference path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// Maximum number of output tokens to emit.
    pub max_output_tokens: usize,
    /// Explicit posture when the prompt would exceed the available context window.
    #[serde(default)]
    pub context_overflow_policy: ContextOverflowPolicy,
    /// Decode strategy.
    pub decode_strategy: DecodeStrategy,
    /// Temperature override for stochastic sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-k sampling cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    /// Top-p / nucleus sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Repeat penalty applied to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Presence penalty applied once to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty scaled by prior token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Deterministic seed for stochastic decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Explicit stop sequences to truncate from generated text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
    /// Optional structured-output fallback contract enforced by Psionic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<StructuredOutputRequest>,
}

impl GenerationOptions {
    /// Creates greedy-decode options.
    #[must_use]
    pub fn greedy(max_output_tokens: usize) -> Self {
        Self {
            max_output_tokens,
            context_overflow_policy: ContextOverflowPolicy::Refuse,
            decode_strategy: DecodeStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
            stop_sequences: Vec::new(),
            structured_output: None,
        }
    }

    /// Creates stochastic sampling options with Ollama-aligned default posture.
    #[must_use]
    pub fn sample(max_output_tokens: usize) -> Self {
        Self {
            decode_strategy: DecodeStrategy::Sample,
            ..Self::greedy(max_output_tokens)
        }
    }

    fn sampling_policy(&self) -> SamplingPolicy {
        SamplingPolicy {
            strategy: match self.decode_strategy {
                DecodeStrategy::Greedy => SamplingStrategy::Greedy,
                DecodeStrategy::Sample => SamplingStrategy::Sample,
            },
            temperature: self.temperature,
            top_k: self.top_k,
            top_p: self.top_p,
            repeat_penalty: self.repeat_penalty,
            presence_penalty: self.presence_penalty,
            frequency_penalty: self.frequency_penalty,
            seed: self.seed,
        }
    }
}

/// Text-generation request contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Target decoder model descriptor.
    pub model: DecoderModelDescriptor,
    /// Optional session to reuse.
    pub session_id: Option<SessionId>,
    /// Prompt input boundary.
    pub prompt: GenerationInput,
    /// Generation options.
    pub options: GenerationOptions,
    /// Explicit adapter-serving binding when the request targets a hosted adapter product.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_serving: Option<AdapterServingBinding>,
    /// Request-level automatic prefix-cache control.
    #[serde(default, skip_serializing_if = "PrefixCacheControl::is_default")]
    pub prefix_cache_control: PrefixCacheControl,
    /// Whether to reset the session cache before generation.
    pub reset_session: bool,
}

impl GenerationRequest {
    /// Creates a text prompt request.
    #[must_use]
    pub fn new_text(
        request_id: impl Into<String>,
        model: DecoderModelDescriptor,
        session_id: Option<SessionId>,
        prompt: impl Into<String>,
        options: GenerationOptions,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            model,
            session_id,
            prompt: GenerationInput::Text(prompt.into()),
            options,
            adapter_serving: None,
            prefix_cache_control: PrefixCacheControl::default(),
            reset_session: false,
        }
    }

    /// Creates a pre-tokenized request.
    #[must_use]
    pub fn new_tokens(
        request_id: impl Into<String>,
        model: DecoderModelDescriptor,
        session_id: Option<SessionId>,
        prompt: TokenSequence,
        options: GenerationOptions,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            model,
            session_id,
            prompt: GenerationInput::Tokens(prompt),
            options,
            adapter_serving: None,
            prefix_cache_control: PrefixCacheControl::default(),
            reset_session: false,
        }
    }

    /// Returns a copy that requests a session reset before generation.
    #[must_use]
    pub fn with_reset_session(mut self, reset_session: bool) -> Self {
        self.reset_session = reset_session;
        self
    }

    /// Binds the request to an explicit adapter-serving product.
    #[must_use]
    pub fn with_adapter_serving(mut self, adapter_serving: AdapterServingBinding) -> Self {
        self.product_id = String::from(ADAPTER_TEXT_GENERATION_PRODUCT_ID);
        self.adapter_serving = Some(adapter_serving);
        self
    }

    /// Attaches request-level automatic prefix-cache control.
    #[must_use]
    pub fn with_prefix_cache_control(mut self, prefix_cache_control: PrefixCacheControl) -> Self {
        self.prefix_cache_control = prefix_cache_control;
        self
    }
}

/// Generated output payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationOutput {
    /// Output token IDs.
    pub tokens: TokenSequence,
    /// Output text rendering.
    pub text: String,
    /// Machine-readable structured output when the request used one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured: Option<StructuredOutputValue>,
    /// Structured GPT-OSS / Harmony output when parsing succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmony: Option<GptOssHarmonyParsedOutput>,
}

impl GenerationOutput {
    /// Attaches structured GPT-OSS / Harmony output while preserving raw text and token lanes.
    #[must_use]
    pub fn with_harmony(mut self, harmony: GptOssHarmonyParsedOutput) -> Self {
        self.harmony = Some(harmony);
        self
    }

    /// Attaches a machine-readable structured value while preserving raw text and token lanes.
    #[must_use]
    pub fn with_structured(mut self, structured: StructuredOutputValue) -> Self {
        self.structured = Some(structured);
        self
    }
}

/// Usage counters for generation execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationUsage {
    /// Number of prompt tokens consumed.
    pub input_tokens: usize,
    /// Number of output tokens produced.
    pub output_tokens: usize,
    /// Number of cached KV slots retained after execution.
    pub cache_tokens: usize,
}

/// Explicit timing and token metrics for one generation call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GenerationMetrics {
    /// End-to-end generation duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Prompt token count surfaced in the metrics lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<usize>,
    /// Prompt-evaluation duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration_ns: Option<u64>,
    /// Explicit prompt-budget accounting for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<ContextWindowAccounting>,
    /// Output token count surfaced in the metrics lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<usize>,
    /// Output-generation duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration_ns: Option<u64>,
    /// Time to first emitted token in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ns: Option<u64>,
    /// Average inter-token latency in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inter_token_latency_ns: Option<u64>,
    /// Explicit paged-KV accounting for the request, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache: Option<KvCacheAccounting>,
    /// Explicit hierarchical KV residency accounting for the request, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_residency: Option<KvResidencyAccounting>,
    /// Number of prompt-prefix tokens reused from the shared prefix cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_tokens_reused: Option<usize>,
    /// Psionic-owned GPT-OSS performance counters for the realized decode path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt_oss_perf: Option<GptOssPerformanceMetrics>,
}

/// Stage timings for one GPT-OSS request, accumulated across prefill and decode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssStageTimingMetrics {
    /// Token-embedding row decode time in nanoseconds.
    pub token_embedding_ns: u64,
    /// Whole forward-step wall time in nanoseconds.
    pub step_wall_ns: u64,
    /// Attention RMSNorm time in nanoseconds.
    pub attention_norm_ns: u64,
    /// Q/K/V projection time in nanoseconds.
    pub qkv_projection_ns: u64,
    /// RoPE application time in nanoseconds.
    pub rope_ns: u64,
    /// Attention score/value application time in nanoseconds.
    pub attention_ns: u64,
    /// Attention output projection time in nanoseconds.
    pub attention_output_projection_ns: u64,
    /// Feed-forward RMSNorm time in nanoseconds.
    pub feed_forward_norm_ns: u64,
    /// Router-logit time in nanoseconds.
    pub router_ns: u64,
    /// Expert projection time in nanoseconds.
    pub expert_projection_ns: u64,
    /// Expert activation time in nanoseconds.
    pub expert_activation_ns: u64,
    /// Expert aggregation time in nanoseconds.
    pub expert_aggregation_ns: u64,
    /// Final output RMSNorm time in nanoseconds.
    pub output_norm_ns: u64,
    /// Final logits projection time in nanoseconds.
    pub logits_projection_ns: u64,
    /// Stop-sequence checking time in nanoseconds.
    pub stop_check_ns: u64,
    /// Host-side sampling / token-selection time in nanoseconds.
    pub sampling_ns: u64,
}

impl GptOssStageTimingMetrics {
    fn accumulate(&mut self, other: &Self) {
        self.token_embedding_ns = self
            .token_embedding_ns
            .saturating_add(other.token_embedding_ns);
        self.step_wall_ns = self.step_wall_ns.saturating_add(other.step_wall_ns);
        self.attention_norm_ns = self
            .attention_norm_ns
            .saturating_add(other.attention_norm_ns);
        self.qkv_projection_ns = self
            .qkv_projection_ns
            .saturating_add(other.qkv_projection_ns);
        self.rope_ns = self.rope_ns.saturating_add(other.rope_ns);
        self.attention_ns = self.attention_ns.saturating_add(other.attention_ns);
        self.attention_output_projection_ns = self
            .attention_output_projection_ns
            .saturating_add(other.attention_output_projection_ns);
        self.feed_forward_norm_ns = self
            .feed_forward_norm_ns
            .saturating_add(other.feed_forward_norm_ns);
        self.router_ns = self.router_ns.saturating_add(other.router_ns);
        self.expert_projection_ns = self
            .expert_projection_ns
            .saturating_add(other.expert_projection_ns);
        self.expert_activation_ns = self
            .expert_activation_ns
            .saturating_add(other.expert_activation_ns);
        self.expert_aggregation_ns = self
            .expert_aggregation_ns
            .saturating_add(other.expert_aggregation_ns);
        self.output_norm_ns = self.output_norm_ns.saturating_add(other.output_norm_ns);
        self.logits_projection_ns = self
            .logits_projection_ns
            .saturating_add(other.logits_projection_ns);
        self.stop_check_ns = self.stop_check_ns.saturating_add(other.stop_check_ns);
        self.sampling_ns = self.sampling_ns.saturating_add(other.sampling_ns);
    }
}

/// CUDA-side counters surfaced by the GPT-OSS runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssCudaRuntimeMetrics {
    /// Bytes uploaded from host to device.
    pub host_to_device_bytes: u64,
    /// Bytes read back from device to host.
    pub device_to_host_bytes: u64,
    /// Number of CUDA submissions used by the request.
    pub submission_count: usize,
    /// Number of CUDA synchronizations used by the request.
    pub sync_count: usize,
    /// Number of quantized CUDA kernel launches used by the request.
    pub kernel_launches: usize,
    /// Whether any decode step used the ids-driven grouped expert path.
    #[serde(default, skip_serializing_if = "is_false")]
    pub grouped_expert_ids_path: bool,
    /// Number of hybrid selected4 cache hits across staged expert slots.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub hybrid_selected4_cache_hits: usize,
    /// Number of hybrid selected4 cache misses across staged expert slots.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub hybrid_selected4_cache_misses: usize,
    /// Bytes staged into hybrid selected4 CUDA caches on miss.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub hybrid_selected4_cache_staged_bytes: u64,
    /// Per-layer hybrid selected4 cache hits across staged expert slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_selected4_layer_cache_hits: Vec<usize>,
    /// Per-layer hybrid selected4 cache misses across staged expert slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_selected4_layer_cache_misses: Vec<usize>,
    /// Per-layer bytes staged into hybrid selected4 CUDA caches on miss.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_selected4_layer_cache_staged_bytes: Vec<u64>,
}

impl GptOssCudaRuntimeMetrics {
    fn accumulate(&mut self, other: &Self) {
        self.host_to_device_bytes = self
            .host_to_device_bytes
            .saturating_add(other.host_to_device_bytes);
        self.device_to_host_bytes = self
            .device_to_host_bytes
            .saturating_add(other.device_to_host_bytes);
        self.submission_count = self.submission_count.saturating_add(other.submission_count);
        self.sync_count = self.sync_count.saturating_add(other.sync_count);
        self.kernel_launches = self.kernel_launches.saturating_add(other.kernel_launches);
        self.grouped_expert_ids_path |= other.grouped_expert_ids_path;
        self.hybrid_selected4_cache_hits = self
            .hybrid_selected4_cache_hits
            .saturating_add(other.hybrid_selected4_cache_hits);
        self.hybrid_selected4_cache_misses = self
            .hybrid_selected4_cache_misses
            .saturating_add(other.hybrid_selected4_cache_misses);
        self.hybrid_selected4_cache_staged_bytes = self
            .hybrid_selected4_cache_staged_bytes
            .saturating_add(other.hybrid_selected4_cache_staged_bytes);
        accumulate_usize_vec(
            &mut self.hybrid_selected4_layer_cache_hits,
            other.hybrid_selected4_layer_cache_hits.as_slice(),
        );
        accumulate_usize_vec(
            &mut self.hybrid_selected4_layer_cache_misses,
            other.hybrid_selected4_layer_cache_misses.as_slice(),
        );
        accumulate_u64_vec(
            &mut self.hybrid_selected4_layer_cache_staged_bytes,
            other.hybrid_selected4_layer_cache_staged_bytes.as_slice(),
        );
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn accumulate_usize_vec(target: &mut Vec<usize>, other: &[usize]) {
    if target.len() < other.len() {
        target.resize(other.len(), 0);
    }
    for (index, value) in other.iter().copied().enumerate() {
        target[index] = target[index].saturating_add(value);
    }
}

fn accumulate_u64_vec(target: &mut Vec<u64>, other: &[u64]) {
    if target.len() < other.len() {
        target.resize(other.len(), 0);
    }
    for (index, value) in other.iter().copied().enumerate() {
        target[index] = target[index].saturating_add(value);
    }
}

/// Metal-side counters surfaced by the GPT-OSS runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssMetalRuntimeMetrics {
    /// Bytes written from host slices into Metal-owned buffers.
    pub host_to_device_bytes: u64,
    /// Bytes materialized back into host-owned vectors from Metal-owned buffers.
    pub device_to_host_bytes: u64,
    /// Number of Metal command submissions used by the request.
    pub submission_count: usize,
    /// Number of explicit Metal synchronizations used by the request.
    pub sync_count: usize,
    /// Number of quantized Metal kernel encodes used by the request.
    pub kernel_launches: usize,
    /// Whether any step used the ids-driven grouped expert projection path.
    #[serde(default, skip_serializing_if = "is_false")]
    pub grouped_expert_ids_path: bool,
}

impl GptOssMetalRuntimeMetrics {
    fn accumulate(&mut self, other: &Self) {
        self.host_to_device_bytes = self
            .host_to_device_bytes
            .saturating_add(other.host_to_device_bytes);
        self.device_to_host_bytes = self
            .device_to_host_bytes
            .saturating_add(other.device_to_host_bytes);
        self.submission_count = self.submission_count.saturating_add(other.submission_count);
        self.sync_count = self.sync_count.saturating_add(other.sync_count);
        self.kernel_launches = self.kernel_launches.saturating_add(other.kernel_launches);
        self.grouped_expert_ids_path |= other.grouped_expert_ids_path;
    }
}

/// Metal logits-output mode used by GPT-OSS decode steps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GptOssMetalLogitsOutputMode {
    /// The decode step returned only the greedy token id.
    GreedyToken,
    /// The decode step returned a bounded top-k candidate set.
    TopKCandidates { top_k: usize },
    /// The decode step materialized the dense raw logits vector.
    RawLogits,
}

/// Request-level decode-step logits-selection evidence for Metal GPT-OSS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssMetalDecodeLogitsMetrics {
    /// Number of decode steps that recorded logits-selection evidence.
    pub step_count: usize,
    /// Unique output modes observed across those decode steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_modes: Vec<GptOssMetalLogitsOutputMode>,
    /// Total bytes read back to the host for decode-step logits selection.
    pub readback_bytes: u64,
    /// Whether any decode step materialized dense raw logits on the host.
    pub raw_logits_materialized: bool,
}

impl GptOssMetalDecodeLogitsMetrics {
    fn accumulate(&mut self, other: &Self) {
        self.step_count = self.step_count.saturating_add(other.step_count);
        self.readback_bytes = self.readback_bytes.saturating_add(other.readback_bytes);
        self.raw_logits_materialized |= other.raw_logits_materialized;
        self.output_modes.extend(other.output_modes.iter().cloned());
        self.output_modes.sort();
        self.output_modes.dedup();
    }

    fn is_zero(&self) -> bool {
        self.step_count == 0
            && self.output_modes.is_empty()
            && self.readback_bytes == 0
            && !self.raw_logits_materialized
    }
}

/// Psionic-owned GPT-OSS performance summary attached to one request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssPerformanceMetrics {
    /// Number of step invocations accumulated into this summary.
    pub step_count: usize,
    /// Number of decoder layers traversed across those steps.
    pub layer_visit_count: usize,
    /// Number of explicit high-level decode-graph nodes for the realized GPT-OSS path.
    pub graph_node_count: usize,
    /// Number of per-layer graph nodes repeated across decoder layers.
    pub graph_layer_node_count: usize,
    /// Accumulated stage timings in nanoseconds.
    pub stage_timings: GptOssStageTimingMetrics,
    /// Accumulated CUDA transfer and synchronization counters.
    pub cuda: GptOssCudaRuntimeMetrics,
    /// Accumulated Metal transfer and synchronization counters.
    pub metal: GptOssMetalRuntimeMetrics,
    /// Decode-step logits-selection evidence for Metal GPT-OSS requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metal_decode_logits: Option<GptOssMetalDecodeLogitsMetrics>,
}

impl GptOssPerformanceMetrics {
    fn accumulate(&mut self, other: &Self) {
        self.step_count = self.step_count.saturating_add(other.step_count);
        self.layer_visit_count = self
            .layer_visit_count
            .saturating_add(other.layer_visit_count);
        self.graph_node_count = self.graph_node_count.max(other.graph_node_count);
        self.graph_layer_node_count = self
            .graph_layer_node_count
            .max(other.graph_layer_node_count);
        self.stage_timings.accumulate(&other.stage_timings);
        self.cuda.accumulate(&other.cuda);
        self.metal.accumulate(&other.metal);
        if let Some(other_metal_decode_logits) = &other.metal_decode_logits {
            self.metal_decode_logits
                .get_or_insert_with(GptOssMetalDecodeLogitsMetrics::default)
                .accumulate(other_metal_decode_logits);
            if self
                .metal_decode_logits
                .as_ref()
                .map_or(false, GptOssMetalDecodeLogitsMetrics::is_zero)
            {
                self.metal_decode_logits = None;
            }
        }
    }

    fn is_zero(&self) -> bool {
        self.step_count == 0
            && self.layer_visit_count == 0
            && self.graph_node_count == 0
            && self.graph_layer_node_count == 0
            && self.metal == GptOssMetalRuntimeMetrics::default()
            && self
                .metal_decode_logits
                .as_ref()
                .map_or(true, GptOssMetalDecodeLogitsMetrics::is_zero)
    }
}

/// Whether a request hit a cold or warm model path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationLoadState {
    /// The request consumed a freshly loaded model path.
    Cold,
    /// The request ran against an already-warm model.
    Warm,
}

/// Provenance fields attached to one generation response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationProvenance {
    /// Stable served-artifact identity for the active model/backend path.
    pub served_artifact: ServedArtifactIdentity,
    /// Explicit adapter-serving binding when the request targeted a hosted adapter product.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_serving: Option<AdapterServingBinding>,
    /// Stable execution-plan digest for the active model graph.
    pub execution_plan_digest: String,
    /// Explicit clustered execution or scheduling context for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// Whether the request took the warm or cold model path.
    pub load_state: GenerationLoadState,
    /// Explicit local-serving isolation policy for the active runtime.
    pub isolation_policy: LocalServingIsolationPolicy,
    /// Explicit streaming policy when the request used the local streaming API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_policy: Option<GenerationStreamingPolicy>,
    /// Explicit resident-memory plan for the active loaded model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_plan: Option<ModelMemoryPlan>,
    /// Active local-serving residency policy for the request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_policy: Option<ModelResidencyPolicy>,
    /// Aggregate resident-memory snapshot for the loaded-model set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_snapshot: Option<MemoryResidencySnapshot>,
    /// Explicit paged-KV policy for the request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
    /// Explicit request- or session-owned paged-KV accounting for the realized path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_ownership: Option<KvCacheOwnershipAccounting>,
    /// Request-level automatic prefix-cache control that governed the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_control: Option<PrefixCacheControl>,
    /// Observable shared prefix-cache state for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_state: Option<PrefixCacheState>,
    /// Explicit refusal reason when the runtime bypassed or invalidated unsafe reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_refusal_reason: Option<PrefixCacheRefusalReason>,
    /// Explicit shared prefix-cache reuse policy for the request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<PrefixCacheReusePolicy>,
    /// Shared prefix-cache identity when the request used or rebuilt a prefix entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_identity: Option<PrefixCacheIdentity>,
    /// Explicit warm/cold compile-path evidence for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Delivery-proof facts surfaced by the local runtime for this request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Explicit cache actions observed for the request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_observations: Vec<CacheObservation>,
    /// Explicit shared-scheduler receipt for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<GenerationSchedulerRequestReceipt>,
    /// Structured-output fallback report when the request used constrained generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<StructuredOutputExecutionReport>,
}

impl GenerationProvenance {
    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        self.cluster_execution = Some(cluster_execution);
        self
    }

    /// Attaches explicit adapter-serving posture.
    #[must_use]
    pub fn with_adapter_serving(mut self, adapter_serving: AdapterServingBinding) -> Self {
        self.adapter_serving = Some(adapter_serving);
        self
    }
}

/// Terminal reason for a generation response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    /// The model emitted an EOS token.
    EndOfSequence,
    /// The request hit the configured output token cap.
    MaxOutputTokens,
    /// The request hit the context limit.
    ContextLimit,
    /// The caller explicitly cancelled a streaming request.
    Cancelled,
    /// The caller disconnected after streaming started.
    Disconnected,
    /// The runtime failed after at least one chunk was emitted.
    Error,
}

/// One streamed output chunk from a generation request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationStreamChunk {
    /// Stable request identifier.
    pub request_id: String,
    /// Model identifier used for execution.
    pub model_id: String,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Newly emitted output tokens for this chunk.
    pub output: GenerationOutput,
    /// Number of output tokens emitted so far.
    pub cumulative_output_tokens: usize,
}

/// Terminal status for a streamed generation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStreamStatus {
    /// Generation completed successfully and committed its output.
    Succeeded,
    /// The caller explicitly cancelled the stream.
    Cancelled,
    /// The caller disconnected after streaming started.
    Disconnected,
    /// The runtime failed after streaming started.
    Failed,
}

/// Terminal event for a streamed generation request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationStreamTerminal {
    /// Terminal stream status.
    pub status: GenerationStreamStatus,
    /// Final or partial response snapshot at termination time.
    pub response: GenerationResponse,
    /// Explicit failure reason when the stream did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Structured diagnostic when the stream did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

/// Typed event emitted by the local streaming generation API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStreamEvent {
    /// Non-terminal output chunk.
    Chunk(GenerationStreamChunk),
    /// Terminal result for the stream.
    Terminal(GenerationStreamTerminal),
}

/// Text-generation response contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Model identifier used for execution.
    pub model_id: String,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Generated payload.
    pub output: GenerationOutput,
    /// Usage counters.
    pub usage: GenerationUsage,
    /// Explicit timing and token metrics.
    #[serde(default, skip_serializing_if = "GenerationMetrics::is_empty")]
    pub metrics: GenerationMetrics,
    /// Explicit execution provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<GenerationProvenance>,
    /// Terminal termination reason.
    pub termination: TerminationReason,
}

impl GenerationResponse {
    /// Creates a generation response.
    #[must_use]
    pub fn new(
        request: &GenerationRequest,
        session_id: Option<SessionId>,
        tokens: TokenSequence,
        text: impl Into<String>,
        input_tokens: usize,
        cache_tokens: usize,
        termination: TerminationReason,
    ) -> Self {
        let output_tokens = tokens.len();
        let usage = GenerationUsage {
            input_tokens,
            output_tokens,
            cache_tokens,
        };
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            model_id: request.model.model.model_id.clone(),
            session_id,
            output: GenerationOutput {
                tokens,
                text: text.into(),
                structured: None,
                harmony: None,
            },
            metrics: GenerationMetrics::from_usage(&usage),
            usage,
            provenance: None,
            termination,
        }
    }

    /// Attaches explicit metrics and provenance to a generation response.
    #[must_use]
    pub fn with_metrics_and_provenance(
        mut self,
        metrics: GenerationMetrics,
        provenance: GenerationProvenance,
    ) -> Self {
        self.metrics = metrics;
        self.provenance = Some(provenance);
        self
    }

    /// Attaches a machine-readable structured value to the response output.
    #[must_use]
    pub fn with_structured_output_value(mut self, structured: StructuredOutputValue) -> Self {
        self.output.structured = Some(structured);
        self
    }
}

impl GenerationMetrics {
    #[must_use]
    fn from_usage(usage: &GenerationUsage) -> Self {
        Self {
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: None,
            context_window: None,
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: None,
            time_to_first_token_ns: None,
            inter_token_latency_ns: None,
            kv_cache: None,
            kv_residency: None,
            prefix_tokens_reused: None,
            gpt_oss_perf: None,
        }
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.total_duration_ns.is_none()
            && self.load_duration_ns.is_none()
            && self.prompt_eval_count.is_none()
            && self.prompt_eval_duration_ns.is_none()
            && self.context_window.is_none()
            && self.eval_count.is_none()
            && self.eval_duration_ns.is_none()
            && self.time_to_first_token_ns.is_none()
            && self.inter_token_latency_ns.is_none()
            && self.kv_cache.is_none()
            && self.kv_residency.is_none()
            && self.prefix_tokens_reused.is_none()
            && self.gpt_oss_perf.is_none()
    }
}

impl EmbeddingMetrics {
    #[must_use]
    fn is_empty(&self) -> bool {
        self.total_duration_ns.is_none()
            && self.load_duration_ns.is_none()
            && self.prompt_eval_count.is_none()
            && self.prompt_eval_duration_ns.is_none()
    }
}

/// Minimal text-generation execution interface.
pub trait TextGenerationExecutor {
    /// Error returned when generation fails.
    type Error;

    /// Executes a text-generation request.
    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error>;
}

/// Library-first pull stream for local text generation.
pub trait GenerationEventStream {
    /// Returns the explicit policy governing the stream.
    fn policy(&self) -> &GenerationStreamingPolicy;

    /// Returns the next chunk or terminal event, pausing generation until pulled.
    fn next_event(&mut self) -> Option<GenerationStreamEvent>;

    /// Cancels the stream and returns the terminal event when cancellation starts a terminal path.
    fn cancel(&mut self) -> Option<GenerationStreamTerminal>;

    /// Signals that the client disconnected and returns the terminal event when disconnect starts a terminal path.
    fn disconnect(&mut self) -> Option<GenerationStreamTerminal>;
}

impl<T> GenerationEventStream for Box<T>
where
    T: GenerationEventStream + ?Sized,
{
    fn policy(&self) -> &GenerationStreamingPolicy {
        (**self).policy()
    }

    fn next_event(&mut self) -> Option<GenerationStreamEvent> {
        (**self).next_event()
    }

    fn cancel(&mut self) -> Option<GenerationStreamTerminal> {
        (**self).cancel()
    }

    fn disconnect(&mut self) -> Option<GenerationStreamTerminal> {
        (**self).disconnect()
    }
}

/// Minimal streaming text-generation execution interface.
pub trait StreamingTextGenerationExecutor: TextGenerationExecutor {
    /// Concrete stream returned by the executor.
    type Stream<'a>: GenerationEventStream
    where
        Self: 'a;

    /// Starts a pull-driven generation stream.
    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error>;
}

/// Library-first catalog surface for local installed-model inspection.
pub trait LocalModelCatalog {
    /// Returns the local installed-model observation.
    fn list_models(&self) -> ListModelsObservation;

    /// Returns the local model-inspection observation for one model name.
    fn show_model(&self, model: &str) -> ShowObservation;
}

/// Library-first generation surface that also exposes local model lifecycle.
pub trait ManagedTextGenerationRuntime:
    TextGenerationExecutor + StreamingTextGenerationExecutor
{
    /// Returns the current loaded-model observation.
    fn loaded_models(&mut self) -> LoadedModelsObservation;

    /// Returns current local-runtime observability for lifecycle/debug surfaces.
    fn observability(&mut self) -> LocalRuntimeObservability;

    /// Returns the explicit local-serving isolation policy for this runtime.
    fn isolation_policy(&self) -> LocalServingIsolationPolicy {
        LocalServingIsolationPolicy::in_process_runtime()
    }

    /// Refreshes or overrides keepalive for one already-loaded model.
    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error>;

    /// Unloads one currently loaded model.
    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error>;
}

/// Library-first aggregate runtime boundary over catalog, generation, and embeddings.
///
/// This wrapper is intentionally thin: it forwards to existing reusable Psionic
/// surfaces so downstream code can depend on one in-process library API
/// without reaching through multiple crates or speaking Ollama HTTP.
#[derive(Clone, Debug)]
pub struct PsionicLocalRuntime<C, G, E> {
    catalog: C,
    generation: G,
    embeddings: E,
}

impl<C, G, E> PsionicLocalRuntime<C, G, E> {
    /// Creates a new aggregate local runtime.
    #[must_use]
    pub fn new(catalog: C, generation: G, embeddings: E) -> Self {
        Self {
            catalog,
            generation,
            embeddings,
        }
    }

    /// Returns the underlying catalog surface.
    #[must_use]
    pub fn catalog(&self) -> &C {
        &self.catalog
    }

    /// Returns the underlying generation surface.
    #[must_use]
    pub fn generation(&self) -> &G {
        &self.generation
    }

    /// Returns the underlying embeddings surface.
    #[must_use]
    pub fn embeddings(&self) -> &E {
        &self.embeddings
    }

    /// Returns mutable access to the underlying generation surface.
    pub fn generation_mut(&mut self) -> &mut G {
        &mut self.generation
    }

    /// Returns mutable access to the underlying embeddings surface.
    pub fn embeddings_mut(&mut self) -> &mut E {
        &mut self.embeddings
    }

    /// Consumes the aggregate runtime and returns its component parts.
    #[must_use]
    pub fn into_parts(self) -> (C, G, E) {
        (self.catalog, self.generation, self.embeddings)
    }
}

impl<C, G, E> PsionicLocalRuntime<C, G, E>
where
    C: LocalModelCatalog,
    G: ManagedTextGenerationRuntime,
    E: EmbeddingsExecutor,
{
    /// Returns the installed-model observation through the catalog surface.
    #[must_use]
    pub fn list_models(&self) -> ListModelsObservation {
        self.catalog.list_models()
    }

    /// Returns model inspection through the catalog surface.
    #[must_use]
    pub fn show_model(&self, model: &str) -> ShowObservation {
        self.catalog.show_model(model)
    }

    /// Returns the currently loaded-model observation.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.generation.loaded_models()
    }

    /// Returns current in-process runtime observability.
    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.generation.observability()
    }

    /// Returns the explicit local-serving isolation policy for this runtime.
    #[must_use]
    pub fn isolation_policy(&self) -> LocalServingIsolationPolicy {
        self.generation.isolation_policy()
    }

    /// Refreshes keepalive for one loaded generation model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <G as TextGenerationExecutor>::Error> {
        self.generation.warm_model(model_id, keep_alive_millis)
    }

    /// Unloads one loaded generation model.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <G as TextGenerationExecutor>::Error> {
        self.generation.unload_model(model_id)
    }

    /// Executes a text-generation request through the managed generation surface.
    pub fn generate(
        &mut self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, <G as TextGenerationExecutor>::Error> {
        self.generation.generate(request)
    }

    /// Starts a pull-driven streaming generation request through the managed generation surface.
    pub fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<G::Stream<'a>, <G as TextGenerationExecutor>::Error> {
        self.generation.generate_stream(request)
    }

    /// Executes an embeddings request through the configured embeddings surface.
    pub fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, E::Error> {
        self.embeddings.embed(request)
    }
}

/// Trait for loaded generation model handles kept active by the serve layer.
pub trait GenerationModelHandle {
    /// Returns the active model descriptor.
    fn descriptor(&self) -> &DecoderModelDescriptor;

    /// Returns the actual per-token KV width owned by the loaded model path.
    fn cache_width(&self) -> usize {
        self.descriptor().config.kv_width()
    }
}

impl GenerationModelHandle for DecoderModelDescriptor {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self
    }
}

impl GenerationModelHandle for ReferenceWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }
}

trait WordDecoderExecutionModel: Clone {
    fn descriptor(&self) -> &DecoderModelDescriptor;
    fn tokenizer(&self) -> &dyn TokenizerBoundary;
    fn weights(&self) -> &DecoderFixtureWeights;
    fn encode_prompt_text(&self, text: &str) -> TokenSequence;

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        token == self.tokenizer().vocabulary().eos_id()
    }

    fn injected_stream_failure(&self, _position: usize) -> Option<ReferenceTextGenerationError> {
        None
    }
}

impl WordDecoderExecutionModel for ReferenceWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        self.tokenizer()
    }

    fn weights(&self) -> &DecoderFixtureWeights {
        self.weights()
    }

    fn encode_prompt_text(&self, text: &str) -> TokenSequence {
        self.tokenizer()
            .encode_with_special_tokens(text, true, false)
    }
}

impl WordDecoderExecutionModel for ArtifactWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        self.tokenizer()
    }

    fn weights(&self) -> &DecoderFixtureWeights {
        self.weights()
    }

    fn encode_prompt_text(&self, text: &str) -> TokenSequence {
        self.tokenizer()
            .encode_with_special_tokens(text, true, false)
    }
}

/// Default local keepalive used by the in-process serve services.
pub const DEFAULT_MODEL_KEEPALIVE_MILLIS: u64 = 300_000;

/// Loaded-model view exposed by the serve layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelView {
    /// Comparable loaded-model summary.
    pub summary: LoadedModelSummary,
    /// Explicit residency and keepalive truth.
    pub residency: LoadedModelResidency,
    /// Explicit resident-memory plan for the loaded model.
    pub memory_plan: ModelMemoryPlan,
    /// Active local-serving residency policy.
    pub residency_policy: ModelResidencyPolicy,
    /// Aggregate resident-memory snapshot after the latest registry mutation.
    pub residency_snapshot: MemoryResidencySnapshot,
}

#[derive(Clone, Debug)]
struct LoadedGenerationModel<M> {
    model: M,
    residency: LoadedModelResidency,
    has_served_request: bool,
    size_bytes: Option<u64>,
    size_vram_bytes: Option<u64>,
    memory_plan: ModelMemoryPlan,
    backend: Option<String>,
    fallback_state: Option<String>,
}

impl<M> LoadedGenerationModel<M>
where
    M: GenerationModelHandle,
{
    fn view(
        &self,
        residency_policy: &ModelResidencyPolicy,
        residency_snapshot: &MemoryResidencySnapshot,
    ) -> LoadedModelView {
        let descriptor = self.model.descriptor();
        let mut summary = LoadedModelSummary::from_decoder_descriptor(
            descriptor.model.model_id.clone(),
            descriptor,
        );
        summary.size_bytes = self.size_bytes;
        summary.size_vram_bytes = self.size_vram_bytes;
        summary.backend = self.backend.clone();
        summary.fallback_state = self.fallback_state.clone();
        LoadedModelView {
            summary,
            residency: self.residency.clone(),
            memory_plan: self.memory_plan.clone(),
            residency_policy: residency_policy.clone(),
            residency_snapshot: residency_snapshot.clone(),
        }
    }
}

/// Loaded-model registry failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoadedModelRegistryError {
    /// The requested model is not currently loaded.
    #[error("loaded model `{0}` was not found")]
    ModelNotLoaded(String),
    /// The configured residency policy refused the candidate model.
    #[error(transparent)]
    AdmissionRefused(#[from] ModelAdmissionRefusal),
}

/// In-memory registry of loaded generation models plus keepalive/lifecycle truth.
#[derive(Clone, Debug)]
pub struct InMemoryGenerationModelRegistry<M> {
    models: BTreeMap<String, LoadedGenerationModel<M>>,
    residency_policy: ModelResidencyPolicy,
    transitions: RuntimeTransitionLog,
}

impl<M> InMemoryGenerationModelRegistry<M>
where
    M: GenerationModelHandle,
{
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            models: BTreeMap::new(),
            residency_policy: ModelResidencyPolicy::default(),
            transitions: RuntimeTransitionLog::default(),
        }
    }

    /// Creates an empty registry with an explicit residency policy.
    #[must_use]
    pub fn with_residency_policy(residency_policy: ModelResidencyPolicy) -> Self {
        Self {
            models: BTreeMap::new(),
            residency_policy,
            transitions: RuntimeTransitionLog::default(),
        }
    }

    /// Loads or replaces a model handle by model ID using the default keepalive.
    pub fn load(&mut self, model: M) -> Result<Option<M>, LoadedModelRegistryError> {
        self.warm_with_metadata(model, 0, DEFAULT_MODEL_KEEPALIVE_MILLIS, None, None, None)
    }

    /// Loads or refreshes a model with explicit keepalive and runtime metadata.
    pub fn warm_with_metadata(
        &mut self,
        model: M,
        now_millis: u64,
        keep_alive_millis: u64,
        size_vram_bytes: Option<u64>,
        backend: Option<String>,
        fallback_state: Option<String>,
    ) -> Result<Option<M>, LoadedModelRegistryError> {
        let model_id = model.descriptor().model.model_id.clone();
        let size_bytes = weight_bundle_size_bytes(&model.descriptor().weights);
        let memory_plan = default_generation_memory_plan(&model, size_bytes, size_vram_bytes);
        let decision = plan_model_admission(
            &self.memory_states(),
            model_id.as_str(),
            &memory_plan,
            &self.residency_policy,
        )?;
        for evicted_model_id in decision.evicted_models {
            if self.models.remove(evicted_model_id.as_str()).is_some() {
                self.transitions.record(RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelUnloaded,
                    evicted_model_id,
                    now_millis,
                ));
            }
        }
        let previous = self
            .models
            .insert(
                model_id.clone(),
                LoadedGenerationModel {
                    model,
                    residency: LoadedModelResidency::ready(now_millis, keep_alive_millis),
                    has_served_request: false,
                    size_bytes,
                    size_vram_bytes,
                    memory_plan,
                    backend,
                    fallback_state,
                },
            )
            .map(|previous| previous.model);
        if previous.is_some() {
            self.transitions.record(RuntimeTransitionEvent::model(
                RuntimeTransitionKind::ModelUnloaded,
                model_id.clone(),
                now_millis,
            ));
        }
        self.transitions.record(RuntimeTransitionEvent::model(
            RuntimeTransitionKind::ModelLoadedCold,
            model_id,
            now_millis,
        ));
        Ok(previous)
    }

    /// Refreshes an already-loaded model's keepalive window.
    pub fn warm_loaded(
        &mut self,
        model_id: &str,
        now_millis: u64,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        self.models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?
            .residency
            .refresh_keep_alive(keep_alive_millis, now_millis);
        let snapshot = self.memory_snapshot();
        let entry = self
            .models
            .get(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        Ok(entry.view(&self.residency_policy, &snapshot))
    }

    /// Marks a request as actively using the model.
    pub fn begin_request(
        &mut self,
        model_id: &str,
        now_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        if !entry.has_served_request {
            self.transitions.record(RuntimeTransitionEvent::model(
                RuntimeTransitionKind::ModelBecameWarm,
                model_id,
                now_millis,
            ));
        }
        entry.has_served_request = true;
        entry.residency.begin_request(now_millis);
        let snapshot = self.memory_snapshot();
        let entry = self
            .models
            .get(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        Ok(entry.view(&self.residency_policy, &snapshot))
    }

    /// Marks a request as finished and refreshes idle expiry.
    pub fn finish_request(
        &mut self,
        model_id: &str,
        now_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        self.models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?
            .residency
            .finish_request(now_millis);
        let snapshot = self.memory_snapshot();
        let entry = self
            .models
            .get(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        Ok(entry.view(&self.residency_policy, &snapshot))
    }

    /// Unloads an active model and returns the final loaded-model view.
    pub fn unload_view(
        &mut self,
        model_id: &str,
        now_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .remove(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        self.transitions.record(RuntimeTransitionEvent::model(
            RuntimeTransitionKind::ModelUnloaded,
            model_id,
            now_millis,
        ));
        let snapshot = self.memory_snapshot();
        Ok(entry.view(&self.residency_policy, &snapshot))
    }

    /// Returns an active model by ID.
    #[must_use]
    pub fn active(&self, model_id: &str) -> Option<&M> {
        self.models.get(model_id).map(|entry| &entry.model)
    }

    /// Returns a mutable active model by ID.
    pub fn active_mut(&mut self, model_id: &str) -> Option<&mut M> {
        self.models.get_mut(model_id).map(|entry| &mut entry.model)
    }

    /// Unloads an active model.
    pub fn unload(&mut self, model_id: &str) -> Option<M> {
        self.models.remove(model_id).map(|entry| entry.model)
    }

    /// Unloads any idle models whose keepalive has expired.
    pub fn expire_idle(&mut self, now_millis: u64) -> Vec<M> {
        let expired = self
            .models
            .iter()
            .filter(|(_, entry)| entry.residency.is_expired(now_millis))
            .map(|(model_id, _)| model_id.clone())
            .collect::<Vec<_>>();
        let mut removed = Vec::with_capacity(expired.len());
        for model_id in expired {
            if let Some(entry) = self.models.remove(model_id.as_str()) {
                self.transitions.record(RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelUnloaded,
                    model_id.clone(),
                    now_millis,
                ));
                removed.push(entry.model);
            }
        }
        removed
    }

    /// Returns loaded-model views in stable `ps` order.
    #[must_use]
    pub fn loaded_model_views(&self) -> Vec<LoadedModelView> {
        let snapshot = self.memory_snapshot();
        let mut views = self
            .models
            .values()
            .map(|entry| entry.view(&self.residency_policy, &snapshot))
            .collect::<Vec<_>>();
        views.sort_by(|left, right| {
            right
                .residency
                .expires_at_millis
                .unwrap_or(u64::MAX)
                .cmp(&left.residency.expires_at_millis.unwrap_or(u64::MAX))
                .then_with(|| left.summary.model.cmp(&right.summary.model))
        });
        views
    }

    /// Returns the comparable loaded-model observation used by `ps` parity.
    #[must_use]
    pub fn loaded_models_observation(&self) -> LoadedModelsObservation {
        LoadedModelsObservation::new(
            self.loaded_model_views()
                .into_iter()
                .map(|view| view.summary)
                .collect(),
        )
    }

    #[must_use]
    fn load_state(&self, model_id: &str) -> Option<GenerationLoadState> {
        self.models.get(model_id).map(|entry| {
            if entry.has_served_request {
                GenerationLoadState::Warm
            } else {
                GenerationLoadState::Cold
            }
        })
    }

    /// Returns the active local-serving residency policy.
    #[must_use]
    pub fn residency_policy(&self) -> &ModelResidencyPolicy {
        &self.residency_policy
    }

    /// Returns the current resident-memory snapshot.
    #[must_use]
    pub fn memory_snapshot(&self) -> MemoryResidencySnapshot {
        MemoryResidencySnapshot::from_loaded_models(&self.memory_states())
    }

    /// Returns total active requests across loaded models.
    #[must_use]
    pub fn active_request_count(&self) -> usize {
        self.models
            .values()
            .map(|entry| entry.residency.active_requests)
            .sum()
    }

    /// Returns recent lifecycle transitions in chronological order.
    #[must_use]
    pub fn recent_transitions(&self) -> Vec<RuntimeTransitionEvent> {
        self.transitions.snapshot()
    }

    /// Returns the memory plan for one loaded model.
    #[must_use]
    pub fn memory_plan(&self, model_id: &str) -> Option<&ModelMemoryPlan> {
        self.models.get(model_id).map(|entry| &entry.memory_plan)
    }

    /// Returns the number of active models.
    #[must_use]
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    fn memory_states(&self) -> Vec<LoadedModelMemoryState> {
        self.models
            .iter()
            .map(|(model_id, entry)| LoadedModelMemoryState {
                model_id: model_id.clone(),
                plan: entry.memory_plan.clone(),
                active_requests: entry.residency.active_requests,
                last_used_at_millis: entry.residency.last_used_at_millis,
            })
            .collect()
    }
}

impl<M> Default for InMemoryGenerationModelRegistry<M>
where
    M: GenerationModelHandle,
{
    fn default() -> Self {
        Self::new()
    }
}

fn weight_bundle_size_bytes(weights: &WeightBundleMetadata) -> Option<u64> {
    (!weights.artifacts.is_empty()).then_some(
        weights
            .artifacts
            .iter()
            .map(|artifact| artifact.byte_length)
            .sum(),
    )
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn generation_runtime_observability<M>(
    models: &InMemoryGenerationModelRegistry<M>,
    sessions: &InMemoryGenerationSessionStore,
    backend_health: &BackendHealthTracker,
    execution_profile: ExecutionCapabilityProfile,
) -> LocalRuntimeObservability
where
    M: GenerationModelHandle,
{
    let mut recent_transitions = models.recent_transitions();
    recent_transitions.extend(backend_health.recent_changes());
    recent_transitions.sort_by(|left, right| {
        left.observed_at_millis
            .cmp(&right.observed_at_millis)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.model_id.cmp(&right.model_id))
            .then_with(|| left.backend.cmp(&right.backend))
            .then_with(|| left.previous_status.cmp(&right.previous_status))
            .then_with(|| left.status.cmp(&right.status))
            .then_with(|| left.message.cmp(&right.message))
    });

    LocalRuntimeObservability {
        isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
        cache_invalidation_policy: cache_invalidation_policy(),
        execution_profile: execution_profile.clone(),
        queue_depth: 0,
        queue_capacity: (execution_profile.queue_policy.max_queued_requests > 0)
            .then_some(execution_profile.queue_policy.max_queued_requests),
        active_sessions: sessions.len(),
        active_requests: models.active_request_count(),
        memory_footprint: models.memory_snapshot(),
        backend_health: backend_health.snapshot(),
        recent_transitions,
    }
}

impl LocalModelCatalog for LocalOllamaCatalogSubject {
    fn list_models(&self) -> ListModelsObservation {
        self.list_models_observation()
    }

    fn show_model(&self, model: &str) -> ShowObservation {
        self.show_model_observation(model)
    }
}

/// Single KV cache entry for one token position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KvCacheEntry {
    /// Token position inside the session cache.
    pub position: usize,
    /// Token ID associated with the slot.
    pub token: TokenId,
    /// Key vector.
    pub key: Vec<f32>,
    /// Value vector.
    pub value: Vec<f32>,
}

/// KV cache failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KvCacheError {
    /// A KV append would require more pages than the cache policy admits.
    #[error(
        "session KV cache refused growth at token {requested_tokens}: max_context={max_context} max_pages={max_pages} spill_policy={spill_policy:?}"
    )]
    PageBudgetExceeded {
        /// Token count the append would have produced.
        requested_tokens: usize,
        /// Maximum cache size in tokens.
        max_context: usize,
        /// Maximum admitted page count.
        max_pages: usize,
        /// Explicit spill/refusal posture.
        spill_policy: KvCacheSpillPolicy,
    },
    /// A KV append used vectors with the wrong width.
    #[error("kv width mismatch: expected={expected} key={actual_key} value={actual_value}")]
    WidthMismatch {
        /// Expected width.
        expected: usize,
        /// Actual key width.
        actual_key: usize,
        /// Actual value width.
        actual_value: usize,
    },
}

/// In-memory per-session KV cache with an explicit logical page layout.
#[derive(Clone, Debug, PartialEq)]
pub struct InMemoryKvCache {
    max_context: usize,
    width: usize,
    policy: KvCachePolicy,
    entries: Vec<KvCacheEntry>,
    owner: Option<KvCacheOwnerBinding>,
    pages: Vec<KvCacheLogicalPage>,
    next_page_index: usize,
    allocation_events: Vec<KvCachePageSpan>,
    reclaim_events: Vec<KvCachePageSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KvCacheLogicalPage {
    page_index: usize,
    start_position: usize,
    token_count: usize,
}

impl KvCacheLogicalPage {
    fn span(&self, layout: &KvCachePageLayout) -> KvCachePageSpan {
        KvCachePageSpan {
            page_index: self.page_index,
            start_token_position: self.start_position,
            token_count: self.token_count,
            bytes_used: layout.bytes_for_tokens(self.token_count),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KvCacheLedgerCheckpoint {
    previous_state: KvCacheState,
    allocation_cursor: usize,
    reclaim_cursor: usize,
}

impl InMemoryKvCache {
    /// Creates an empty in-memory KV cache.
    #[must_use]
    pub fn new(max_context: usize, width: usize) -> Self {
        Self::with_policy(
            max_context,
            width,
            default_kv_cache_policy(max_context, width),
        )
    }

    /// Creates an empty in-memory cache with an explicit paged-KV policy.
    #[must_use]
    pub fn with_policy(max_context: usize, width: usize, policy: KvCachePolicy) -> Self {
        Self {
            max_context,
            width,
            policy,
            entries: Vec::new(),
            owner: None,
            pages: Vec::new(),
            next_page_index: 0,
            allocation_events: Vec::new(),
            reclaim_events: Vec::new(),
        }
    }

    /// Returns the configured context limit.
    #[must_use]
    pub const fn max_context(&self) -> usize {
        self.max_context
    }

    /// Returns the configured KV vector width.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the explicit paged-KV policy for the cache.
    #[must_use]
    pub fn policy(&self) -> &KvCachePolicy {
        &self.policy
    }

    /// Returns the logical page layout for the cache.
    #[must_use]
    pub fn page_layout(&self) -> &KvCachePageLayout {
        &self.policy.page_layout
    }

    /// Binds the cache image to one explicit owner.
    pub fn bind_owner(&mut self, owner: KvCacheOwnerBinding) {
        self.owner = Some(owner);
    }

    /// Returns the bound cache owner, when present.
    #[must_use]
    pub fn owner(&self) -> Option<&KvCacheOwnerBinding> {
        self.owner.as_ref()
    }

    /// Returns the number of cached slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the cached entries in token order.
    #[must_use]
    pub fn entries(&self) -> &[KvCacheEntry] {
        &self.entries
    }

    /// Returns the current logical page spans owned by the cache.
    #[must_use]
    pub fn page_spans(&self) -> Vec<KvCachePageSpan> {
        self.pages
            .iter()
            .map(|page| page.span(self.page_layout()))
            .collect()
    }

    fn checkpoint(&self) -> KvCacheLedgerCheckpoint {
        self.checkpoint_with_state(self.state())
    }

    fn checkpoint_with_state(&self, previous_state: KvCacheState) -> KvCacheLedgerCheckpoint {
        KvCacheLedgerCheckpoint {
            previous_state,
            allocation_cursor: self.allocation_events.len(),
            reclaim_cursor: self.reclaim_events.len(),
        }
    }

    fn ownership_since(
        &self,
        checkpoint: &KvCacheLedgerCheckpoint,
    ) -> Option<KvCacheOwnershipAccounting> {
        self.ownership_since_with_current_state(checkpoint, self.state())
    }

    fn ownership_since_with_current_state(
        &self,
        checkpoint: &KvCacheLedgerCheckpoint,
        current_state: KvCacheState,
    ) -> Option<KvCacheOwnershipAccounting> {
        self.owner.clone().map(|owner| {
            KvCacheOwnershipAccounting::new(
                owner,
                checkpoint.previous_state.clone(),
                current_state,
                self.allocation_events[checkpoint.allocation_cursor..].to_vec(),
                self.reclaim_events[checkpoint.reclaim_cursor..].to_vec(),
            )
        })
    }

    /// Returns the current paged-KV snapshot for the cache.
    #[must_use]
    pub fn state(&self) -> KvCacheState {
        KvCacheState {
            tokens: self.len(),
            bytes: self
                .pages
                .iter()
                .map(|page| self.page_layout().bytes_for_tokens(page.token_count))
                .sum(),
            pages: self.pages.len(),
        }
    }

    /// Appends a token KV pair to the cache.
    pub fn append(
        &mut self,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<(), KvCacheError> {
        if key.len() != self.width || value.len() != self.width {
            return Err(KvCacheError::WidthMismatch {
                expected: self.width,
                actual_key: key.len(),
                actual_value: value.len(),
            });
        }
        self.ensure_capacity_for_append()?;
        self.allocate_tail_page_if_needed()?;
        self.entries.push(KvCacheEntry {
            position: self.entries.len(),
            token,
            key,
            value,
        });
        if let Some(page) = self.pages.last_mut() {
            page.token_count = page.token_count.saturating_add(1);
        }
        Ok(())
    }

    fn ensure_capacity_for_append(&mut self) -> Result<(), KvCacheError> {
        while self.entries.len() >= self.max_context
            || (self.tail_page_full() && self.pages.len() >= self.page_layout().max_pages)
        {
            match self.policy.spill_policy {
                KvCacheSpillPolicy::EvictOldestPages => self.evict_oldest_page()?,
                KvCacheSpillPolicy::RefuseNewPages | KvCacheSpillPolicy::SpillToHost => {
                    return Err(KvCacheError::PageBudgetExceeded {
                        requested_tokens: self.entries.len().saturating_add(1),
                        max_context: self.max_context,
                        max_pages: self.page_layout().max_pages,
                        spill_policy: self.policy.spill_policy,
                    });
                }
            }
        }
        Ok(())
    }

    fn allocate_tail_page_if_needed(&mut self) -> Result<(), KvCacheError> {
        if !self.pages.is_empty() && !self.tail_page_full() {
            return Ok(());
        }
        if self.pages.len() >= self.page_layout().max_pages {
            return Err(KvCacheError::PageBudgetExceeded {
                requested_tokens: self.entries.len().saturating_add(1),
                max_context: self.max_context,
                max_pages: self.page_layout().max_pages,
                spill_policy: self.policy.spill_policy,
            });
        }
        let page = KvCacheLogicalPage {
            page_index: self.next_page_index,
            start_position: self.entries.len(),
            token_count: 0,
        };
        self.next_page_index = self.next_page_index.saturating_add(1);
        self.allocation_events.push(page.span(self.page_layout()));
        self.pages.push(page);
        Ok(())
    }

    fn tail_page_full(&self) -> bool {
        self.pages
            .last()
            .is_some_and(|page| page.token_count >= self.page_layout().tokens_per_page)
    }

    fn evict_oldest_page(&mut self) -> Result<(), KvCacheError> {
        let Some(page) = self.pages.first().cloned() else {
            return Err(KvCacheError::PageBudgetExceeded {
                requested_tokens: self.entries.len().saturating_add(1),
                max_context: self.max_context,
                max_pages: self.page_layout().max_pages,
                spill_policy: self.policy.spill_policy,
            });
        };
        let removed_tokens = page.token_count;
        self.reclaim_events.push(page.span(self.page_layout()));
        self.pages.remove(0);
        self.entries.drain(..removed_tokens);
        for (position, entry) in self.entries.iter_mut().enumerate() {
            entry.position = position;
        }
        for page in &mut self.pages {
            page.start_position = page.start_position.saturating_sub(removed_tokens);
        }
        Ok(())
    }

    /// Truncates the cache to a target token count and records reclaimed pages.
    pub fn truncate(&mut self, token_count: usize) {
        if token_count >= self.entries.len() {
            return;
        }
        let page_layout = self.page_layout().clone();
        self.entries.truncate(token_count);
        while self
            .pages
            .last()
            .is_some_and(|page| page.start_position >= token_count)
        {
            if let Some(page) = self.pages.pop() {
                self.reclaim_events.push(page.span(&page_layout));
            }
        }
        if let Some(page) = self.pages.last_mut() {
            let page_end = page.start_position.saturating_add(page.token_count);
            if page_end > token_count {
                let retained_tokens = token_count.saturating_sub(page.start_position);
                let reclaimed_tokens = page.token_count.saturating_sub(retained_tokens);
                if retained_tokens == 0 {
                    let removed_page = self.pages.pop().expect("tail page exists");
                    self.reclaim_events.push(removed_page.span(&page_layout));
                } else if reclaimed_tokens > 0 {
                    let reclaimed_bytes = page_layout.bytes_for_tokens(reclaimed_tokens);
                    self.reclaim_events.push(KvCachePageSpan {
                        page_index: page.page_index,
                        start_token_position: token_count,
                        token_count: reclaimed_tokens,
                        bytes_used: reclaimed_bytes,
                    });
                    page.token_count = retained_tokens;
                }
            }
        }
    }

    /// Clears all cached slots.
    pub fn reset(&mut self) {
        self.truncate(0);
    }
}

/// Session metadata surfaced to higher layers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSession {
    /// Stable session identifier.
    pub session_id: SessionId,
    /// Stable served-artifact digest that owns the session KV state.
    pub served_artifact_digest: String,
    /// Bound model identifier.
    pub model_id: String,
    /// Bound model family.
    pub model_family: String,
    /// Bound model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest that owns the session KV state.
    pub weight_bundle_digest: String,
    /// Maximum context for the session cache.
    pub max_context: usize,
    /// KV vector width.
    pub kv_width: usize,
    /// Current cached token count.
    pub cached_tokens: usize,
    /// Explicit paged-KV policy for the session cache.
    pub kv_cache_policy: KvCachePolicy,
    /// Current paged-KV snapshot for the session cache.
    pub kv_cache: KvCacheState,
}

/// Session state stored in memory.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationSessionState {
    session: GenerationSession,
    cache: InMemoryKvCache,
    tokens: Vec<TokenId>,
}

impl GenerationSessionState {
    /// Returns session metadata.
    #[must_use]
    pub fn session(&self) -> &GenerationSession {
        &self.session
    }

    /// Returns the per-session KV cache.
    #[must_use]
    pub fn cache(&self) -> &InMemoryKvCache {
        &self.cache
    }

    /// Returns the tokens currently owned by the session.
    #[must_use]
    pub fn tokens(&self) -> &[TokenId] {
        &self.tokens
    }
}

/// Session store failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionStoreError {
    /// A session identifier was not found.
    #[error("generation session `{0}` was not found")]
    SessionNotFound(String),
    /// The caller attempted to use a session with the wrong model.
    #[error(
        "generation session `{session_id}` expects model `{expected_model}` revision `{expected_revision}` artifact `{expected_served_artifact_digest}` bundle `{expected_weight_bundle_digest}` but got model `{actual_model}` revision `{actual_revision}` artifact `{actual_served_artifact_digest}` bundle `{actual_weight_bundle_digest}`"
    )]
    ModelMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected served-artifact digest.
        expected_served_artifact_digest: String,
        /// Expected model identifier.
        expected_model: String,
        /// Expected model revision.
        expected_revision: String,
        /// Expected weight-bundle digest.
        expected_weight_bundle_digest: String,
        /// Actual model identifier.
        actual_model: String,
        /// Actual served-artifact digest.
        actual_served_artifact_digest: String,
        /// Actual model revision.
        actual_revision: String,
        /// Actual weight-bundle digest.
        actual_weight_bundle_digest: String,
    },
    /// The caller attempted to replace a session cache with incompatible
    /// geometry.
    #[error(
        "generation session `{session_id}` cache geometry mismatch: expected max_context={expected_max_context} width={expected_width}, actual max_context={actual_max_context} width={actual_width}"
    )]
    CacheGeometryMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected max context.
        expected_max_context: usize,
        /// Expected KV width.
        expected_width: usize,
        /// Actual max context.
        actual_max_context: usize,
        /// Actual KV width.
        actual_width: usize,
    },
    /// The session cache rejected an operation.
    #[error(transparent)]
    Cache(#[from] KvCacheError),
}

/// In-memory generation session store for the phase-1 reference path.
#[derive(Clone, Debug, Default)]
pub struct InMemoryGenerationSessionStore {
    next_session: u64,
    sessions: BTreeMap<SessionId, GenerationSessionState>,
}

impl InMemoryGenerationSessionStore {
    /// Creates an empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_session: 0,
            sessions: BTreeMap::new(),
        }
    }

    /// Creates a new session bound to a decoder model.
    pub fn create<M>(
        &mut self,
        model: &M,
        served_artifact_digest: impl Into<String>,
    ) -> GenerationSession
    where
        M: GenerationModelHandle,
    {
        self.next_session += 1;
        let session_id = SessionId::new(format!("sess-{:08}", self.next_session));
        let descriptor = model.descriptor();
        let cache_width = model.cache_width();
        let policy = default_generation_kv_cache_policy(model);
        let session = GenerationSession {
            session_id: session_id.clone(),
            served_artifact_digest: served_artifact_digest.into(),
            model_id: descriptor.model.model_id.clone(),
            model_family: descriptor.model.family.clone(),
            model_revision: descriptor.model.revision.clone(),
            weight_bundle_digest: descriptor.weights.digest.clone(),
            max_context: descriptor.config.max_context,
            kv_width: cache_width,
            cached_tokens: 0,
            kv_cache_policy: policy.clone(),
            kv_cache: KvCacheState::default(),
        };
        let mut cache =
            InMemoryKvCache::with_policy(descriptor.config.max_context, cache_width, policy);
        cache.bind_owner(session_kv_owner(
            descriptor.model.model_id.as_str(),
            &session_id,
        ));
        let state = GenerationSessionState {
            session: session.clone(),
            cache,
            tokens: Vec::new(),
        };
        self.sessions.insert(session_id, state);
        session
    }

    /// Returns session metadata by ID.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&GenerationSession> {
        self.sessions
            .get(session_id)
            .map(GenerationSessionState::session)
    }

    /// Returns the number of tracked generation sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Returns whether the session store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Returns session state by ID.
    pub fn state(
        &self,
        session_id: &SessionId,
    ) -> Result<&GenerationSessionState, SessionStoreError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))
    }

    /// Returns the session cache by ID.
    pub fn cache(&self, session_id: &SessionId) -> Result<&InMemoryKvCache, SessionStoreError> {
        Ok(self.state(session_id)?.cache())
    }

    /// Appends a KV slot to a session and returns updated metadata.
    pub fn append(
        &mut self,
        session_id: &SessionId,
        model: &DecoderModelDescriptor,
        served_artifact_digest: &str,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        validate_session_model(state, session_id, model, served_artifact_digest)?;

        state.cache.append(token, key, value)?;
        state.tokens.push(token);
        sync_session_cache_state(&mut state.session, &state.cache);
        Ok(state.session.clone())
    }

    /// Resets a session cache in place and returns updated metadata.
    pub fn reset(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        state.cache.reset();
        state.tokens.clear();
        sync_session_cache_state(&mut state.session, &state.cache);
        Ok(state.session.clone())
    }

    /// Closes a session and drops its KV cache.
    pub fn close(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, SessionStoreError> {
        self.sessions
            .remove(session_id)
            .map(|state| state.session)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))
    }

    /// Replaces a session cache wholesale and returns updated metadata.
    pub fn replace_cache(
        &mut self,
        session_id: &SessionId,
        model: &DecoderModelDescriptor,
        served_artifact_digest: &str,
        mut cache: InMemoryKvCache,
        tokens: TokenSequence,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        validate_session_model(state, session_id, model, served_artifact_digest)?;
        if state.cache.max_context() != cache.max_context() || state.cache.width() != cache.width()
        {
            return Err(SessionStoreError::CacheGeometryMismatch {
                session_id: session_id.as_str().to_string(),
                expected_max_context: state.cache.max_context(),
                expected_width: state.cache.width(),
                actual_max_context: cache.max_context(),
                actual_width: cache.width(),
            });
        }

        cache.bind_owner(session_kv_owner(model.model.model_id.as_str(), session_id));
        state.cache = cache;
        state.tokens = tokens.as_slice().to_vec();
        sync_session_cache_state(&mut state.session, &state.cache);
        Ok(state.session.clone())
    }
}

fn sync_session_cache_state(session: &mut GenerationSession, cache: &InMemoryKvCache) {
    session.cached_tokens = cache.len();
    session.kv_cache_policy = cache.policy().clone();
    session.kv_cache = cache.state();
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SharedPrefixCompatibility {
    served_artifact_digest: String,
    model_id: String,
    model_revision: String,
    weight_bundle_digest: String,
    tokenizer_family: String,
    tokenizer_digest: Option<String>,
    chat_template_digest: Option<String>,
    generation_defaults_digest: Option<String>,
    backend_compatibility: String,
    tenant_id: Option<String>,
    sampler_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct SharedPrefixEntry {
    compatibility: SharedPrefixCompatibility,
    prompt_tokens: TokenSequence,
    prompt_logits: Vec<Vec<f32>>,
    last_prompt_logits: Vec<f32>,
    greedy_prompt_token: Option<u32>,
    cache: InMemoryKvCache,
}

#[derive(Clone, Debug, Default)]
struct SharedPrefixStore {
    entries: Vec<SharedPrefixEntry>,
}

#[derive(Clone, Debug, PartialEq)]
struct PrefixLookupResult {
    state: PrefixCacheState,
    reused_tokens: usize,
    identity: Option<PrefixCacheIdentity>,
    cache: Option<InMemoryKvCache>,
    prompt_logits: Vec<Vec<f32>>,
    last_logits: Vec<f32>,
    refusal_reason: Option<PrefixCacheRefusalReason>,
    invalidation_trigger: Option<CacheInvalidationTrigger>,
}

#[derive(Clone, Debug, PartialEq)]
struct ExactPrefixLookupResult {
    identity: PrefixCacheIdentity,
    cache: InMemoryKvCache,
    last_logits: Vec<f32>,
    greedy_token: Option<u32>,
}

impl SharedPrefixCompatibility {
    fn storage_identity_matches(&self, other: &Self) -> bool {
        self.served_artifact_digest == other.served_artifact_digest
            && self.model_id == other.model_id
            && self.model_revision == other.model_revision
            && self.weight_bundle_digest == other.weight_bundle_digest
            && self.tokenizer_family == other.tokenizer_family
            && self.tokenizer_digest == other.tokenizer_digest
            && self.chat_template_digest == other.chat_template_digest
            && self.generation_defaults_digest == other.generation_defaults_digest
            && self.backend_compatibility == other.backend_compatibility
    }
}

impl SharedPrefixStore {
    fn empty_lookup(state: PrefixCacheState) -> PrefixLookupResult {
        PrefixLookupResult {
            state,
            reused_tokens: 0,
            identity: None,
            cache: None,
            prompt_logits: Vec::new(),
            last_logits: Vec::new(),
            refusal_reason: None,
            invalidation_trigger: None,
        }
    }

    fn boundary_refusal_reason(
        &self,
        compatibility: &SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<PrefixCacheRefusalReason> {
        let mut saw_sampler_boundary = false;
        for entry in &self.entries {
            if !entry.compatibility.storage_identity_matches(compatibility)
                || shared_prefix_len(entry.prompt_tokens.as_slice(), prompt_tokens.as_slice()) == 0
            {
                continue;
            }
            if entry.compatibility.tenant_id != compatibility.tenant_id {
                return Some(PrefixCacheRefusalReason::TenantBoundary);
            }
            if entry.compatibility.sampler_digest != compatibility.sampler_digest {
                saw_sampler_boundary = true;
            }
        }
        saw_sampler_boundary.then_some(PrefixCacheRefusalReason::SamplerBoundary)
    }

    fn invalidate(
        &mut self,
        compatibility: &SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> bool {
        let retained = self.entries.len();
        self.entries.retain(|entry| {
            !(entry.compatibility.storage_identity_matches(compatibility)
                && shared_prefix_len(entry.prompt_tokens.as_slice(), prompt_tokens.as_slice()) > 0)
        });
        self.entries.len() != retained
    }

    fn lookup_exact_prompt(
        &self,
        compatibility: &SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<ExactPrefixLookupResult> {
        self.entries
            .iter()
            .find(|entry| {
                &entry.compatibility == compatibility
                    && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
                    && entry.cache.len() >= prompt_tokens.len()
                    && !entry.last_prompt_logits.is_empty()
            })
            .map(|entry| {
                let mut cache = entry.cache.clone();
                cache.truncate(prompt_tokens.len());
                ExactPrefixLookupResult {
                    identity: prefix_identity(compatibility, prompt_tokens.as_slice()),
                    cache,
                    last_logits: entry.last_prompt_logits.clone(),
                    greedy_token: entry.greedy_prompt_token,
                }
            })
    }

    fn lookup(
        &mut self,
        compatibility: &SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> PrefixLookupResult {
        let compatible_indices: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (&entry.compatibility == compatibility).then_some(index))
            .collect();
        if compatible_indices.is_empty() {
            if let Some(refusal_reason) = self.boundary_refusal_reason(compatibility, prompt_tokens)
            {
                let mut result = Self::empty_lookup(PrefixCacheState::Bypassed);
                result.refusal_reason = Some(refusal_reason);
                return result;
            }
            return Self::empty_lookup(PrefixCacheState::None);
        }

        let mut best: Option<(usize, usize)> = None;
        let mut stale_prefix = false;
        for index in compatible_indices {
            let entry = &self.entries[index];
            let shared =
                shared_prefix_len(entry.prompt_tokens.as_slice(), prompt_tokens.as_slice());
            if shared == 0 {
                continue;
            }
            if entry.cache.len() < shared {
                stale_prefix = true;
                continue;
            }
            match best {
                Some((_, best_shared)) if best_shared >= shared => {}
                _ => best = Some((index, shared)),
            }
        }

        if let Some((index, shared)) = best {
            let entry = &self.entries[index];
            let mut cache = entry.cache.clone();
            cache.truncate(shared);
            let exact_prompt_hit = entry.prompt_tokens.as_slice() == prompt_tokens.as_slice();
            let can_return_prefix_logits = entry.prompt_logits.len() >= shared;
            return PrefixLookupResult {
                state: PrefixCacheState::Hit,
                reused_tokens: shared,
                identity: Some(prefix_identity(
                    compatibility,
                    &entry.prompt_tokens.as_slice()[..shared],
                )),
                cache: Some(cache),
                prompt_logits: if exact_prompt_hit {
                    Vec::new()
                } else if can_return_prefix_logits {
                    entry.prompt_logits[..shared].to_vec()
                } else {
                    Vec::new()
                },
                last_logits: if exact_prompt_hit {
                    entry.last_prompt_logits.clone()
                } else if can_return_prefix_logits {
                    entry
                        .prompt_logits
                        .get(shared.saturating_sub(1))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    Vec::new()
                },
                refusal_reason: None,
                invalidation_trigger: None,
            };
        }

        if stale_prefix {
            self.entries.retain(|entry| {
                !(&entry.compatibility == compatibility
                    && (entry.cache.len() < entry.prompt_tokens.len()
                        || entry.prompt_logits.len() < entry.prompt_tokens.len()))
            });
            let mut result = Self::empty_lookup(PrefixCacheState::Rebuilt);
            result.invalidation_trigger = Some(CacheInvalidationTrigger::PrefixCacheFormatUpgrade);
            return result;
        }

        Self::empty_lookup(PrefixCacheState::Miss)
    }

    fn record(
        &mut self,
        compatibility: SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        prompt_logits: &[Vec<f32>],
        cache: &InMemoryKvCache,
    ) -> PrefixCacheIdentity {
        let identity = prefix_identity(&compatibility, prompt_tokens.as_slice());
        let mut cache = cache.clone();
        cache.bind_owner(shared_prefix_kv_owner(&identity));
        let greedy_prompt_token = prompt_logits
            .last()
            .and_then(|logits| select_argmax_token(logits.as_slice()));
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.compatibility == compatibility
                && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
        }) {
            existing.prompt_logits = prompt_logits.to_vec();
            existing.last_prompt_logits = prompt_logits.last().cloned().unwrap_or_default();
            existing.greedy_prompt_token = greedy_prompt_token;
            existing.cache = cache;
        } else {
            self.entries.push(SharedPrefixEntry {
                compatibility,
                prompt_tokens: prompt_tokens.clone(),
                prompt_logits: prompt_logits.to_vec(),
                last_prompt_logits: prompt_logits.last().cloned().unwrap_or_default(),
                greedy_prompt_token,
                cache,
            });
        }
        identity
    }
}

fn shared_prefix_len(left: &[TokenId], right: &[TokenId]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn prefix_compatibility<M>(model: &M) -> SharedPrefixCompatibility
where
    M: CompiledWordGenerationModel,
{
    let served_artifact = served_artifact_identity_for_decoder_backend(
        model.descriptor(),
        model.backend_compatibility(),
        &[],
    );
    SharedPrefixCompatibility {
        served_artifact_digest: served_artifact.served_artifact_digest,
        model_id: model.descriptor().model.model_id.clone(),
        model_revision: model.descriptor().model.revision.clone(),
        weight_bundle_digest: model.descriptor().weights.digest.clone(),
        tokenizer_family: model.descriptor().tokenizer_family.clone(),
        tokenizer_digest: model
            .descriptor()
            .artifact_identity
            .as_ref()
            .and_then(|value| value.tokenizer_digest.clone()),
        chat_template_digest: model
            .descriptor()
            .artifact_identity
            .as_ref()
            .and_then(|value| value.chat_template_digest.clone()),
        generation_defaults_digest: model
            .descriptor()
            .artifact_identity
            .as_ref()
            .map(|value| value.generation_defaults_digest.clone()),
        backend_compatibility: model.backend_compatibility().to_string(),
        tenant_id: None,
        sampler_digest: None,
    }
}

fn prefix_cache_tenant_id(
    request: &GenerationRequest,
    policy: &PrefixCacheReusePolicy,
) -> Option<String> {
    if policy.shared_across_users {
        None
    } else {
        request.prefix_cache_control.tenant_id.clone().or_else(|| {
            request
                .session_id
                .as_ref()
                .map(|value| value.as_str().to_string())
        })
    }
}

fn prefix_cache_sampler_digest(
    request: &GenerationRequest,
    policy: &PrefixCacheReusePolicy,
) -> Option<String> {
    if policy.shared_across_sampler_settings {
        return None;
    }
    let sampling_policy = request.options.sampling_policy();
    let mut hasher = Sha256::new();
    hasher.update(b"prefix_cache_sampler|");
    hasher.update(
        if matches!(sampling_policy.strategy, SamplingStrategy::Greedy) {
            b"greedy".as_slice()
        } else {
            b"sample".as_slice()
        },
    );
    for value in [
        sampling_policy.temperature.map(f32::to_bits),
        sampling_policy.top_p.map(f32::to_bits),
        sampling_policy.repeat_penalty.map(f32::to_bits),
        sampling_policy.presence_penalty.map(f32::to_bits),
        sampling_policy.frequency_penalty.map(f32::to_bits),
    ] {
        hasher.update(value.unwrap_or_default().to_le_bytes());
    }
    hasher.update(
        sampling_policy
            .top_k
            .unwrap_or_default()
            .try_into()
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(sampling_policy.seed.unwrap_or_default().to_le_bytes());
    Some(hex::encode(hasher.finalize()))
}

fn prefix_compatibility_for_request<M>(
    model: &M,
    request: &GenerationRequest,
) -> SharedPrefixCompatibility
where
    M: CompiledWordGenerationModel,
{
    let mut compatibility = prefix_compatibility(model);
    if let Some(adapter_serving) = request.adapter_serving.as_ref() {
        compatibility.served_artifact_digest = adapter_serving.served_adapter_digest.clone();
    }
    let policy = default_prefix_cache_policy();
    compatibility.tenant_id = prefix_cache_tenant_id(request, &policy);
    compatibility.sampler_digest = prefix_cache_sampler_digest(request, &policy);
    compatibility
}

fn prefix_identity(
    compatibility: &SharedPrefixCompatibility,
    prompt_tokens: &[TokenId],
) -> PrefixCacheIdentity {
    let mut hasher = Sha256::new();
    for token in prompt_tokens {
        hasher.update(token.as_u32().to_le_bytes());
    }
    PrefixCacheIdentity {
        served_artifact_digest: compatibility.served_artifact_digest.clone(),
        model_id: compatibility.model_id.clone(),
        model_revision: compatibility.model_revision.clone(),
        weight_bundle_digest: compatibility.weight_bundle_digest.clone(),
        tokenizer_family: compatibility.tokenizer_family.clone(),
        tokenizer_digest: compatibility.tokenizer_digest.clone(),
        chat_template_digest: compatibility.chat_template_digest.clone(),
        generation_defaults_digest: compatibility.generation_defaults_digest.clone(),
        tenant_id: compatibility.tenant_id.clone(),
        sampler_digest: compatibility.sampler_digest.clone(),
        backend_compatibility: compatibility.backend_compatibility.clone(),
        prefix_digest: hex::encode(hasher.finalize()),
        prefix_tokens: prompt_tokens.len(),
    }
}

fn controlled_prefix_lookup(
    shared_prefixes: &mut SharedPrefixStore,
    compatibility: &SharedPrefixCompatibility,
    prompt_tokens: &TokenSequence,
    request: &GenerationRequest,
) -> PrefixLookupResult {
    match request.prefix_cache_control.mode {
        PrefixCacheMode::Auto => shared_prefixes.lookup(compatibility, prompt_tokens),
        PrefixCacheMode::Bypass => {
            let mut result = SharedPrefixStore::empty_lookup(PrefixCacheState::Bypassed);
            result.refusal_reason = Some(PrefixCacheRefusalReason::RequestOptOut);
            result
        }
        PrefixCacheMode::Invalidate => {
            let _ = shared_prefixes.invalidate(compatibility, prompt_tokens);
            let mut result = SharedPrefixStore::empty_lookup(PrefixCacheState::Rebuilt);
            result.refusal_reason = Some(PrefixCacheRefusalReason::ForcedInvalidation);
            result.invalidation_trigger = Some(CacheInvalidationTrigger::ExplicitReset);
            result
        }
    }
}

fn controlled_exact_prefix_lookup(
    shared_prefixes: &SharedPrefixStore,
    compatibility: &SharedPrefixCompatibility,
    prompt_tokens: &TokenSequence,
    request: &GenerationRequest,
) -> Option<ExactPrefixLookupResult> {
    (request.prefix_cache_control.mode == PrefixCacheMode::Auto)
        .then(|| shared_prefixes.lookup_exact_prompt(compatibility, prompt_tokens))
        .flatten()
}

fn prefix_recording_allowed(request: &GenerationRequest) -> bool {
    request.prefix_cache_control.mode != PrefixCacheMode::Bypass
}

fn validate_session_model(
    state: &GenerationSessionState,
    session_id: &SessionId,
    model: &DecoderModelDescriptor,
    served_artifact_digest: &str,
) -> Result<(), SessionStoreError> {
    if state.session.served_artifact_digest != served_artifact_digest
        || state.session.model_id != model.model.model_id
        || state.session.model_revision != model.model.revision
        || state.session.weight_bundle_digest != model.weights.digest
    {
        return Err(SessionStoreError::ModelMismatch {
            session_id: session_id.as_str().to_string(),
            expected_served_artifact_digest: state.session.served_artifact_digest.clone(),
            expected_model: state.session.model_id.clone(),
            expected_revision: state.session.model_revision.clone(),
            expected_weight_bundle_digest: state.session.weight_bundle_digest.clone(),
            actual_model: model.model.model_id.clone(),
            actual_served_artifact_digest: served_artifact_digest.to_string(),
            actual_revision: model.model.revision.clone(),
            actual_weight_bundle_digest: model.weights.digest.clone(),
        });
    }
    Ok(())
}

fn diagnostic_with_request_context(
    diagnostic: LocalRuntimeDiagnostic,
    product_id: &str,
    model_id: &str,
) -> LocalRuntimeDiagnostic {
    diagnostic
        .with_product_id(product_id.to_string())
        .with_model_id(model_id.to_string())
}

fn blob_error_diagnostic(error: &psionic_catalog::BlobError) -> LocalRuntimeDiagnostic {
    match error {
        psionic_catalog::BlobError::MissingFile { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::ArtifactMissing,
            404,
            error.to_string(),
        ),
        psionic_catalog::BlobError::Read { .. }
        | psionic_catalog::BlobError::MemoryMap { .. }
        | psionic_catalog::BlobError::InvalidPageSize { .. }
        | psionic_catalog::BlobError::InvalidDigestFormat { .. }
        | psionic_catalog::BlobError::DigestMismatch { .. }
        | psionic_catalog::BlobError::RangeOutOfBounds { .. }
        | psionic_catalog::BlobError::PageOutOfBounds { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::ArtifactInvalid,
            500,
            error.to_string(),
        ),
    }
}

fn model_load_error_diagnostic(error: &ModelLoadError) -> LocalRuntimeDiagnostic {
    match error {
        ModelLoadError::UnsupportedModel(_)
        | ModelLoadError::UnsupportedConfig(_)
        | ModelLoadError::UnsupportedGgufArchitecture { .. }
        | ModelLoadError::UnsupportedGgufEmbeddingArchitecture { .. } => {
            LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedModel,
                400,
                error.to_string(),
            )
        }
        ModelLoadError::UnsupportedGgufDecoderFamilyFeature { .. }
        | ModelLoadError::UnsupportedGgufEmbeddingFamilyFeature { .. }
        | ModelLoadError::UnsupportedOllamaAdapterPolicy { .. }
        | ModelLoadError::UnsupportedTokenizerModel { .. }
        | ModelLoadError::UnsupportedTensorDType { .. }
        | ModelLoadError::UnsupportedGgufTensorType { .. }
        | ModelLoadError::UnsupportedQuantizedTensorMode { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::UnsupportedCapability,
            400,
            error.to_string(),
        ),
        ModelLoadError::ArtifactRead { path, .. } => {
            if !std::path::Path::new(path).exists() {
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::ArtifactMissing,
                    404,
                    error.to_string(),
                )
            } else {
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::ArtifactInvalid,
                    500,
                    error.to_string(),
                )
            }
        }
        ModelLoadError::ArtifactWrite { .. } => {
            LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, error.to_string())
        }
        ModelLoadError::ArtifactFormat { .. }
        | ModelLoadError::MissingGgufMetadata { .. }
        | ModelLoadError::InvalidGgufMetadata { .. }
        | ModelLoadError::MissingTokenizerMetadata { .. }
        | ModelLoadError::InvalidTokenizerMetadata { .. }
        | ModelLoadError::MissingTensor(_)
        | ModelLoadError::MissingTensorScale(_)
        | ModelLoadError::InvalidTensorShape { .. }
        | ModelLoadError::InvalidTensorScaleShape { .. }
        | ModelLoadError::InvalidQuantizedTensorShape { .. }
        | ModelLoadError::InvalidQuantizedTensorByteLength { .. }
        | ModelLoadError::WeightDigestMismatch { .. }
        | ModelLoadError::WeightTensorMetadataMismatch { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::ArtifactInvalid,
            500,
            error.to_string(),
        ),
        ModelLoadError::Blob(blob) => blob_error_diagnostic(blob),
    }
}

fn context_window_diagnostic(error: &ContextWindowError) -> LocalRuntimeDiagnostic {
    LocalRuntimeDiagnostic::new(
        LocalRuntimeErrorCode::ContextOverflow,
        400,
        error.to_string(),
    )
}

fn loaded_model_registry_diagnostic(error: &LoadedModelRegistryError) -> LocalRuntimeDiagnostic {
    match error {
        LoadedModelRegistryError::ModelNotLoaded(model_id) => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::ModelNotLoaded,
            404,
            error.to_string(),
        )
        .with_model_id(model_id.clone()),
        LoadedModelRegistryError::AdmissionRefused(refusal) => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::AdmissionRefused,
            503,
            refusal.to_string(),
        )
        .with_model_id(refusal.requested_model_id.clone()),
    }
}

fn kv_cache_diagnostic(error: &KvCacheError) -> LocalRuntimeDiagnostic {
    match error {
        KvCacheError::PageBudgetExceeded { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::CacheExhausted,
            409,
            error.to_string(),
        ),
        KvCacheError::WidthMismatch { .. } => {
            LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, error.to_string())
        }
    }
}

fn session_store_diagnostic(error: &SessionStoreError) -> LocalRuntimeDiagnostic {
    match error {
        SessionStoreError::SessionNotFound(_) => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::SessionNotFound,
            404,
            error.to_string(),
        ),
        SessionStoreError::ModelMismatch { expected_model, .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::SessionMismatch,
            409,
            error.to_string(),
        )
        .with_model_id(expected_model.clone()),
        SessionStoreError::CacheGeometryMismatch { .. } => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::SessionMismatch,
            409,
            error.to_string(),
        ),
        SessionStoreError::Cache(cache) => kv_cache_diagnostic(cache),
    }
}

fn runtime_error_diagnostic(error: &RuntimeError) -> LocalRuntimeDiagnostic {
    match error {
        RuntimeError::UnsupportedStep(_) => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::UnsupportedCapability,
            503,
            error.to_string(),
        ),
        RuntimeError::Backend(_) => LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::BackendExecutionFailed,
            500,
            error.to_string(),
        ),
        RuntimeError::MissingInput(_) | RuntimeError::InvalidBuffer { .. } => {
            LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, error.to_string())
        }
    }
}

/// CPU reference text-generation error.
#[derive(Debug, Error)]
pub enum ReferenceTextGenerationError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The requested model is not loaded or does not match the loaded
    /// descriptor.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request targeted an adapter binding that this runtime cannot honor.
    #[error("unsupported adapter binding `{binding_id}`: {reason}")]
    UnsupportedAdapterBinding {
        /// Stable binding identifier or digest.
        binding_id: String,
        /// Plain-text refusal reason.
        reason: String,
    },
    /// The request carried no prompt tokens.
    #[error("generation request must contain at least one prompt token")]
    EmptyPrompt,
    /// A supplied token ID does not fit the active vocabulary.
    #[error("invalid token id {token} for vocabulary size {vocab_size}")]
    InvalidToken {
        /// Token value.
        token: u32,
        /// Active vocabulary size.
        vocab_size: usize,
    },
    /// A token step requested an out-of-range position.
    #[error("invalid position {position} for max context {max_context}")]
    InvalidPosition {
        /// Requested position.
        position: usize,
        /// Maximum supported context length.
        max_context: usize,
    },
    /// A context vector had the wrong width.
    #[error("invalid context width: expected {expected}, actual {actual}")]
    InvalidContextWidth {
        /// Expected width.
        expected: usize,
        /// Actual width.
        actual: usize,
    },
    /// The active cache geometry is incompatible with the reference model.
    #[error(
        "unsupported cache geometry: expected_kv_width={expected_kv_width} kv_width={kv_width}"
    )]
    UnsupportedCacheGeometry {
        /// Expected KV width for the active model.
        expected_kv_width: usize,
        /// Session KV width.
        kv_width: usize,
    },
    /// Loading or validating a model artifact failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Prompt context-window budgeting failed before execution.
    #[error(transparent)]
    ContextWindow(#[from] ContextWindowError),
    /// The compiler rejected the reference graph.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Session operations failed.
    #[error(transparent)]
    Session(#[from] SessionStoreError),
    /// Loaded-model registry operations failed.
    #[error(transparent)]
    LoadedModelRegistry(#[from] LoadedModelRegistryError),
    /// Cache operations failed.
    #[error(transparent)]
    Cache(#[from] KvCacheError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    /// Structured-output fallback request compilation failed.
    #[error(transparent)]
    StructuredOutput(#[from] StructuredOutputError),
    /// The constrained generation fallback could not find a valid continuation.
    #[error("structured output fallback could not find a valid continuation")]
    StructuredOutputExhausted,
    /// An expected graph output was missing.
    #[error("missing graph output `{0}`")]
    MissingOutput(&'static str),
}

impl ReferenceTextGenerationError {
    fn diagnostic_with_backend(&self, backend: &str) -> LocalRuntimeDiagnostic {
        let diagnostic = match self {
            Self::UnsupportedProduct(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedProduct,
                400,
                self.to_string(),
            ),
            Self::UnsupportedModel(model_id) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ModelNotFound,
                404,
                self.to_string(),
            )
            .with_model_id(model_id.clone()),
            Self::UnsupportedAdapterBinding { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedCapability,
                400,
                self.to_string(),
            ),
            Self::EmptyPrompt | Self::InvalidToken { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidRequest,
                400,
                self.to_string(),
            ),
            Self::InvalidPosition { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ContextOverflow,
                400,
                self.to_string(),
            ),
            Self::InvalidContextWidth { .. } => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
            }
            Self::UnsupportedCacheGeometry { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedCapability,
                503,
                self.to_string(),
            ),
            Self::StructuredOutput(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedCapability,
                400,
                self.to_string(),
            ),
            Self::StructuredOutputExhausted => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidOutput,
                422,
                self.to_string(),
            ),
            Self::Model(error) => model_load_error_diagnostic(error),
            Self::ContextWindow(error) => context_window_diagnostic(error),
            Self::Compile(_) | Self::Graph(_) | Self::MissingOutput(_) => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
            }
            Self::Session(error) => session_store_diagnostic(error),
            Self::LoadedModelRegistry(error) => loaded_model_registry_diagnostic(error),
            Self::Cache(error) => kv_cache_diagnostic(error),
            Self::Runtime(error) => runtime_error_diagnostic(error).with_backend(backend),
        };
        match self {
            Self::LoadedModelRegistry(LoadedModelRegistryError::AdmissionRefused(refusal)) => {
                diagnostic.with_model_id(refusal.requested_model_id.clone())
            }
            _ => diagnostic,
        }
    }

    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        self.diagnostic_with_backend("cpu")
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &GenerationRequest) -> LocalRuntimeDiagnostic {
        let diagnostic = self.diagnostic();
        let diagnostic = if diagnostic.model_id.is_none() {
            diagnostic.with_model_id(request.model.model.model_id.clone())
        } else {
            diagnostic
        };
        diagnostic.with_product_id(request.product_id.clone())
    }
}

/// Metal-backed text-generation execution error.
#[derive(Debug, Error)]
pub enum MetalTextGenerationError {
    /// Metal is not available for the requested product path on this machine.
    #[error("metal backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        /// Honest backend status.
        status: HealthStatus,
        /// Plain-text reason.
        message: String,
    },
    /// Request validation, model loading, session, or runtime execution failed.
    #[error(transparent)]
    Generation(#[from] ReferenceTextGenerationError),
}

impl MetalTextGenerationError {
    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        match self {
            Self::BackendUnavailable { status, .. } => LocalRuntimeDiagnostic::new(
                if *status == HealthStatus::Degraded {
                    LocalRuntimeErrorCode::BackendDegraded
                } else {
                    LocalRuntimeErrorCode::BackendUnavailable
                },
                503,
                self.to_string(),
            )
            .with_backend("metal")
            .with_backend_health(*status),
            Self::Generation(error) => error.diagnostic_with_backend("metal"),
        }
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &GenerationRequest) -> LocalRuntimeDiagnostic {
        diagnostic_with_request_context(
            self.diagnostic(),
            &request.product_id,
            &request.model.model.model_id,
        )
    }
}

impl From<ModelLoadError> for MetalTextGenerationError {
    fn from(value: ModelLoadError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<LoadedModelRegistryError> for MetalTextGenerationError {
    fn from(value: LoadedModelRegistryError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<SessionStoreError> for MetalTextGenerationError {
    fn from(value: SessionStoreError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

#[derive(Clone, Debug)]
struct GenerationStepOutput {
    key: Vec<f32>,
    value: Vec<f32>,
    logits: Vec<f32>,
    hidden: Option<Vec<f32>>,
    execution_plan_digest: Option<String>,
    compile_path: Option<CompilePathEvidence>,
    kernel_count: usize,
    bytes_moved: u64,
    plan_cache_hits: usize,
    plan_cache_misses: usize,
    gpt_oss_perf: Option<GptOssPerformanceMetrics>,
}

#[derive(Clone, Debug)]
struct EmbeddingStepOutput {
    values: Vec<f32>,
    execution_plan_digest: Option<String>,
    compile_path: Option<CompilePathEvidence>,
    kernel_count: usize,
    bytes_moved: u64,
    plan_cache_hits: usize,
    plan_cache_misses: usize,
}

trait CompiledWordGenerationModel: GenerationModelHandle + Clone {
    type Backend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary;
    fn encode_prompt_input(
        &self,
        input: &GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError>;
    fn is_end_of_sequence(&self, token: TokenId) -> bool;
    fn execute_step(
        &self,
        backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &InMemoryKvCache,
    ) -> Result<GenerationStepOutput, ReferenceTextGenerationError>;
    fn adjust_step_output(
        &self,
        step: &mut GenerationStepOutput,
        request: &GenerationRequest,
    ) -> Result<(), ReferenceTextGenerationError> {
        let _ = (step, request);
        Ok(())
    }
    fn plan_digest(&self) -> &str;
    fn load_duration_ns(&self) -> u64;
    fn backend_compatibility(&self) -> &'static str;
}

fn execute_generation_step_for_request<B, M>(
    loaded_model: &M,
    backend: &mut B,
    request: &GenerationRequest,
    token: TokenId,
    position: usize,
    cache: &InMemoryKvCache,
) -> Result<GenerationStepOutput, ReferenceTextGenerationError>
where
    M: CompiledWordGenerationModel<Backend = B>,
{
    let mut step = loaded_model.execute_step(backend, token, position, cache)?;
    loaded_model.adjust_step_output(&mut step, request)?;
    Ok(step)
}

/// Loaded CPU-backed generation model.
#[derive(Clone, Debug)]
struct CpuWordGenerationModel<M> {
    model: M,
    graph: Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
    plan_digest: String,
    load_duration_ns: u64,
}

impl<M> CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    /// Loads and compiles a decoder model.
    fn new(model: M) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let (
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
        ) = build_generation_graph(&model)?;
        let plan_digest = compile_graph(&graph)?.stable_digest();
        let load_duration_ns = load_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        Ok(Self {
            model,
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
            plan_digest,
            load_duration_ns,
        })
    }

    /// Returns the underlying generation model.
    #[must_use]
    fn model(&self) -> &M {
        &self.model
    }

    /// Returns the stable compiled-plan digest.
    #[must_use]
    fn plan_digest(&self) -> &str {
        self.plan_digest.as_str()
    }

    #[must_use]
    fn load_duration_ns(&self) -> u64 {
        self.load_duration_ns
    }
}

impl<M> GenerationModelHandle for CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.model.descriptor()
    }
}

impl<M> CompiledWordGenerationModel for CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    type Backend = CpuBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        self.model.tokenizer()
    }

    fn encode_prompt_input(
        &self,
        input: &GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            GenerationInput::Text(text) => self.model.encode_prompt_text(text),
            GenerationInput::Tokens(tokens) => tokens.clone(),
        })
    }

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.model.is_end_of_sequence(token)
    }

    fn execute_step(
        &self,
        backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &InMemoryKvCache,
    ) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
        validate_generation_step_request(&self.model, token, position)?;
        if let Some(error) = self.model.injected_stream_failure(position) {
            return Err(error);
        }
        let context = mean_cache_value(cache, self.model.descriptor().config.hidden_size);
        execute_cpu_generation_graph(
            backend,
            &self.graph,
            self.token_input_id,
            self.position_input_id,
            self.context_input_id,
            self.hidden_output_id,
            self.logits_output_id,
            &self.model.descriptor().config,
            token,
            position,
            context.as_slice(),
        )
    }

    fn plan_digest(&self) -> &str {
        self.plan_digest()
    }

    fn load_duration_ns(&self) -> u64 {
        self.load_duration_ns()
    }

    fn backend_compatibility(&self) -> &'static str {
        "cpu"
    }
}

/// Loaded Metal-backed generation model.
#[derive(Clone, Debug)]
struct MetalWordGenerationModel<M> {
    model: M,
    graph: Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
    plan_digest: String,
    load_duration_ns: u64,
}

impl<M> MetalWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    fn new(model: M, device: Device) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let (
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
        ) = build_generation_graph_for_device(device, &model)?;
        let plan_digest = compile_graph(&graph)?.stable_digest();
        let load_duration_ns = load_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        Ok(Self {
            model,
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
            plan_digest,
            load_duration_ns,
        })
    }

    fn plan_digest(&self) -> &str {
        self.plan_digest.as_str()
    }

    fn load_duration_ns(&self) -> u64 {
        self.load_duration_ns
    }
}

impl<M> GenerationModelHandle for MetalWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.model.descriptor()
    }
}

impl<M> CompiledWordGenerationModel for MetalWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    type Backend = MetalBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        self.model.tokenizer()
    }

    fn encode_prompt_input(
        &self,
        input: &GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            GenerationInput::Text(text) => self.model.encode_prompt_text(text),
            GenerationInput::Tokens(tokens) => tokens.clone(),
        })
    }

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.model.is_end_of_sequence(token)
    }

    fn execute_step(
        &self,
        backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &InMemoryKvCache,
    ) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
        validate_generation_step_request(&self.model, token, position)?;
        let context = mean_cache_value(cache, self.model.descriptor().config.hidden_size);
        execute_metal_generation_graph(
            backend,
            &self.graph,
            self.token_input_id,
            self.position_input_id,
            self.context_input_id,
            self.hidden_output_id,
            self.logits_output_id,
            &self.model.descriptor().config,
            token,
            position,
            context.as_slice(),
        )
    }

    fn plan_digest(&self) -> &str {
        self.plan_digest()
    }

    fn load_duration_ns(&self) -> u64 {
        self.load_duration_ns()
    }

    fn backend_compatibility(&self) -> &'static str {
        "metal"
    }
}

/// Reference-model alias for the phase-1 text-generation path.
type CpuReferenceGenerationModel = CpuWordGenerationModel<ReferenceWordDecoder>;

/// Artifact-backed model alias for the first model-backed text-generation path.
type CpuModelGenerationModel = CpuWordGenerationModel<ArtifactWordDecoder>;

/// Artifact-backed Metal model alias for the first accelerated text-generation path.
type MetalModelGenerationModel = MetalWordGenerationModel<ArtifactWordDecoder>;

/// CPU-backed deterministic text-generation reference service.
#[derive(Clone, Debug)]
pub struct CpuReferenceTextGenerationService {
    backend: CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuReferenceGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl CpuReferenceTextGenerationService {
    /// Creates a service with the default reference decoder loaded.
    pub fn new() -> Result<Self, ReferenceTextGenerationError> {
        let backend = CpuBackend::new();
        let model = ReferenceWordDecoder::new();
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            CpuReferenceGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe("cpu", backend.health(), current_time_millis());
        Ok(Self {
            backend,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    /// Loads or replaces a reference decoder model.
    pub fn load_model(
        &mut self,
        model: ReferenceWordDecoder,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.model_descriptor = model.descriptor().clone();
        self.models.warm_with_metadata(
            CpuReferenceGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        Ok(())
    }

    /// Returns the default reference model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    /// Returns the compiled plan digest for a loaded model.
    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CpuReferenceGenerationModel::plan_digest)
    }

    /// Refreshes keepalive for an already loaded model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    /// Returns the currently loaded models after applying idle expiry.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns runtime observability at a caller-provided time.
    #[must_use]
    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe("cpu", self.backend.health(), now_millis);
        generation_runtime_observability(
            &self.models,
            &self.sessions,
            &self.backend_health,
            continuous_batch_text_generation_execution_profile(),
        )
    }

    /// Returns runtime observability after applying idle expiry.
    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(current_time_millis())
    }

    /// Returns explicit loaded-model residency views at a caller-provided time.
    #[must_use]
    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    /// Returns explicit loaded-model residency views after applying idle expiry.
    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    /// Unloads a currently loaded model explicitly.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self.models.unload_view(model_id, current_time_millis())?)
    }

    /// Creates a reusable generation session for the provided model ID.
    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(
            model,
            served_artifact_identity_for_decoder_backend(model.descriptor(), "cpu", &[])
                .served_artifact_digest,
        ))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }

    /// Executes a shared continuous-batching run across compatible requests.
    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        run_continuous_batch_generation_requests(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            requests,
            default_generation_scheduler_policy(),
        )
    }
}

impl TextGenerationExecutor for CpuReferenceTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )
    }
}

impl StreamingTextGenerationExecutor for CpuReferenceTextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error> {
        Ok(Box::new(CpuGenerationStream::new(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )?))
    }
}

impl ManagedTextGenerationRuntime for CpuReferenceTextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        CpuReferenceTextGenerationService::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        CpuReferenceTextGenerationService::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuReferenceTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuReferenceTextGenerationService::unload_model(self, model_id)
    }
}

/// CPU-backed model-backed text-generation service.
#[derive(Clone, Debug)]
pub struct CpuModelTextGenerationService {
    backend: CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuModelGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl CpuModelTextGenerationService {
    /// Creates a service with the artifact-backed decoder loaded from a local safetensors file.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let backend = CpuBackend::new();
        let model = ArtifactWordDecoder::from_safetensors_artifact(path)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            CpuModelGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe("cpu", backend.health(), current_time_millis());
        Ok(Self {
            backend,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    /// Loads or replaces an artifact-backed decoder model.
    pub fn load_model(
        &mut self,
        model: ArtifactWordDecoder,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.model_descriptor = model.descriptor().clone();
        self.models.warm_with_metadata(
            CpuModelGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        Ok(())
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    /// Returns the compiled plan digest for the loaded model.
    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CpuModelGenerationModel::plan_digest)
    }

    /// Refreshes keepalive for an already loaded model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    /// Returns the currently loaded models after applying idle expiry.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns runtime observability at a caller-provided time.
    #[must_use]
    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe("cpu", self.backend.health(), now_millis);
        generation_runtime_observability(
            &self.models,
            &self.sessions,
            &self.backend_health,
            continuous_batch_text_generation_execution_profile(),
        )
    }

    /// Returns runtime observability after applying idle expiry.
    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(current_time_millis())
    }

    /// Returns explicit loaded-model residency views at a caller-provided time.
    #[must_use]
    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    /// Returns explicit loaded-model residency views after applying idle expiry.
    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    /// Unloads a currently loaded model explicitly.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self.models.unload_view(model_id, current_time_millis())?)
    }

    /// Creates a reusable generation session for the provided model ID.
    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(
            model,
            served_artifact_identity_for_decoder_backend(model.descriptor(), "cpu", &[])
                .served_artifact_digest,
        ))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }

    /// Executes a shared continuous-batching run across compatible requests.
    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        run_continuous_batch_generation_requests(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            requests,
            default_generation_scheduler_policy(),
        )
    }
}

impl TextGenerationExecutor for CpuModelTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )
    }
}

impl StreamingTextGenerationExecutor for CpuModelTextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error> {
        Ok(Box::new(CpuGenerationStream::new(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )?))
    }
}

impl ManagedTextGenerationRuntime for CpuModelTextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        CpuModelTextGenerationService::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        CpuModelTextGenerationService::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuModelTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuModelTextGenerationService::unload_model(self, model_id)
    }
}

/// Honest CPU product alias for model-backed text generation.
pub type CpuProductTextGenerationService = CpuModelTextGenerationService;

fn backend_selection_fallback_state(selection: &BackendSelection) -> Option<String> {
    match selection.selection_state {
        BackendSelectionState::Direct => None,
        BackendSelectionState::SameBackendDegraded => Some(String::from("same_backend_degraded")),
        BackendSelectionState::SameBackendSlowPath => Some(String::from("same_backend_slow_path")),
        BackendSelectionState::CrossBackendFallback => Some(String::from("cross_backend_fallback")),
        BackendSelectionState::Retried => Some(String::from("retried")),
        BackendSelectionState::Refused => Some(String::from("refused")),
    }
}

/// Metal-backed model-backed text-generation service for the supported dense product path.
pub struct MetalModelTextGenerationService {
    backend: MetalBackend,
    backend_selection: BackendSelection,
    models: InMemoryGenerationModelRegistry<MetalModelGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl MetalModelTextGenerationService {
    /// Loads the first model-backed text-generation family on Metal when the
    /// local machine exposes a genuinely supported Metal execution device.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, MetalTextGenerationError> {
        let backend = MetalBackend::new();
        let backend_selection = backend
            .backend_selection(TEXT_GENERATION_SUPPORTED_OPS)
            .map_err(|error| MetalTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let selected_device = backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.device.clone())
            .ok_or_else(|| MetalTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: String::from("metal backend selected no execution device"),
            })?;

        let model = ArtifactWordDecoder::from_safetensors_artifact(path)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            MetalModelGenerationModel::new(model, selected_device)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("metal")),
            backend_selection_fallback_state(&backend_selection),
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe("metal", backend.health(), current_time_millis());
        Ok(Self {
            backend,
            backend_selection,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    /// Loads or replaces an artifact-backed decoder model.
    pub fn load_model(
        &mut self,
        model: ArtifactWordDecoder,
    ) -> Result<(), MetalTextGenerationError> {
        let selected_device = self
            .backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.device.clone())
            .ok_or_else(|| MetalTextGenerationError::BackendUnavailable {
                status: self.backend.health().status,
                message: String::from("metal backend selected no execution device"),
            })?;
        self.model_descriptor = model.descriptor().clone();
        self.models.warm_with_metadata(
            MetalModelGenerationModel::new(model, selected_device)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("metal")),
            backend_selection_fallback_state(&self.backend_selection),
        )?;
        Ok(())
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    /// Returns truthful backend-selection data for the loaded Metal product.
    #[must_use]
    pub fn backend_selection(&self) -> &BackendSelection {
        &self.backend_selection
    }

    /// Returns the compiled plan digest for the loaded model.
    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(MetalModelGenerationModel::plan_digest)
    }

    /// Refreshes keepalive for an already loaded model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, MetalTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    /// Returns the currently loaded models after applying idle expiry.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns runtime observability at a caller-provided time.
    #[must_use]
    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe("metal", self.backend.health(), now_millis);
        generation_runtime_observability(
            &self.models,
            &self.sessions,
            &self.backend_health,
            default_text_generation_execution_profile(),
        )
    }

    /// Returns runtime observability after applying idle expiry.
    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(current_time_millis())
    }

    /// Returns explicit loaded-model residency views at a caller-provided time.
    #[must_use]
    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    /// Returns explicit loaded-model residency views after applying idle expiry.
    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    /// Unloads a currently loaded model explicitly.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, MetalTextGenerationError> {
        Ok(self.models.unload_view(model_id, current_time_millis())?)
    }

    /// Creates a reusable generation session for the provided model ID.
    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<GenerationSession, MetalTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        let compiled_backend_features = self
            .backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.feature_flags.clone())
            .unwrap_or_default();
        Ok(self.sessions.create(
            model,
            served_artifact_identity_for_decoder_backend(
                model.descriptor(),
                self.backend_selection.effective_backend.as_str(),
                &compiled_backend_features,
            )
            .served_artifact_digest,
        ))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, MetalTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, MetalTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }
}

impl TextGenerationExecutor for MetalModelTextGenerationService {
    type Error = MetalTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )
        .map_err(Into::into)
    }
}

/// Honest Metal product alias for model-backed text generation.
pub type MetalProductTextGenerationService = MetalModelTextGenerationService;

struct GenerationSampler {
    sampler: TokenSampler,
    structured_output: Option<StructuredOutputMatcher>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GenerationSelection {
    Token(TokenId),
    Terminate,
}

impl GenerationSampler {
    fn new(options: &GenerationOptions) -> Result<Self, ReferenceTextGenerationError> {
        Ok(Self {
            sampler: TokenSampler::new(&options.sampling_policy()),
            structured_output: options
                .structured_output
                .clone()
                .map(StructuredOutputMatcher::compile)
                .transpose()?,
        })
    }

    fn select_next_token(
        &mut self,
        tokenizer: &dyn TokenizerBoundary,
        logits: &[f32],
        cache: &InMemoryKvCache,
        generated_tokens: &[TokenId],
    ) -> Result<GenerationSelection, ReferenceTextGenerationError> {
        let history = cache
            .entries()
            .iter()
            .map(|entry| entry.token.as_u32())
            .collect::<Vec<_>>();
        self.select_next_token_from_history(tokenizer, logits, &history, generated_tokens)
    }

    fn select_next_token_from_history(
        &mut self,
        tokenizer: &dyn TokenizerBoundary,
        logits: &[f32],
        history: &[u32],
        generated_tokens: &[TokenId],
    ) -> Result<GenerationSelection, ReferenceTextGenerationError> {
        let Some(matcher) = self.structured_output.as_ref() else {
            return self
                .sampler
                .select_next_token(logits, history)
                .map(TokenId)
                .map_or(
                    Err(ReferenceTextGenerationError::MissingOutput("next_token")),
                    |token| Ok(GenerationSelection::Token(token)),
                );
        };

        let current_text = tokenizer.decode(generated_tokens);
        let current_match = matcher.classify(current_text.as_str());
        if matcher.prefers_completion_termination()
            && matches!(current_match.status, StructuredOutputMatchStatus::Complete)
            && !current_match.can_continue
        {
            return Ok(GenerationSelection::Terminate);
        }

        let mut masked_logits = logits.to_vec();
        for _ in 0..masked_logits.len() {
            let Some(candidate) = self.sampler.select_next_token(&masked_logits, history) else {
                return Err(ReferenceTextGenerationError::StructuredOutputExhausted);
            };
            let mut candidate_tokens = generated_tokens.to_vec();
            candidate_tokens.push(TokenId(candidate));
            let candidate_text = tokenizer.decode(candidate_tokens.as_slice());
            let matched = matcher.classify(candidate_text.as_str());
            if matched.is_allowed() {
                return Ok(GenerationSelection::Token(TokenId(candidate)));
            }
            let index = candidate as usize;
            if let Some(logit) = masked_logits.get_mut(index) {
                *logit = f32::NEG_INFINITY;
            }
        }
        Err(ReferenceTextGenerationError::StructuredOutputExhausted)
    }

    fn structured_output_report(&self) -> Option<StructuredOutputExecutionReport> {
        self.structured_output
            .as_ref()
            .map(StructuredOutputMatcher::execution_report)
    }

    fn structured_output_value(
        &self,
        text: &str,
    ) -> Result<Option<StructuredOutputValue>, ReferenceTextGenerationError> {
        let structured_output = self
            .structured_output
            .as_ref()
            .map(|matcher| matcher.materialize(text))
            .transpose()
            .map_err(ReferenceTextGenerationError::from)?;
        Ok(structured_output.flatten())
    }
}

/// Result of one shared continuous-batching generation run.
#[derive(Debug)]
pub struct ContinuousBatchGenerationResult {
    /// Responses in the same order as the supplied requests.
    pub responses: Vec<Result<GenerationResponse, ReferenceTextGenerationError>>,
    /// Aggregate scheduler metrics for the realized run.
    pub scheduler_metrics: GenerationSchedulerMetrics,
}

struct ScheduledGenerationState<M>
where
    M: CompiledWordGenerationModel,
{
    request: GenerationRequest,
    loaded_model: M,
    model_id: String,
    served_artifact: ServedArtifactIdentity,
    load_state: GenerationLoadState,
    generation_start: Instant,
    memory_plan: Option<ModelMemoryPlan>,
    residency_policy: Option<ModelResidencyPolicy>,
    residency_snapshot: Option<MemoryResidencySnapshot>,
    prompt_eval_started_at: Option<Instant>,
    prompt_eval_duration_ns: Option<u64>,
    prefill_handoff_state: Option<KvCacheState>,
    first_token_emitted_at: Option<Instant>,
    last_token_emitted_at: Option<Instant>,
    context_window: ContextWindowAccounting,
    previous_kv_state: KvCacheState,
    cache: InMemoryKvCache,
    request_kv_checkpoint: KvCacheLedgerCheckpoint,
    session_tokens: Vec<TokenId>,
    prompt_tokens: TokenSequence,
    prompt_logits: Vec<Vec<f32>>,
    prefill_cursor: usize,
    prefix_compatibility: SharedPrefixCompatibility,
    shared_prefix_eligible: bool,
    prefix_policy: PrefixCacheReusePolicy,
    prefix_state: PrefixCacheState,
    prefix_cache_refusal_reason: Option<PrefixCacheRefusalReason>,
    prefix_cache_invalidation_trigger: Option<CacheInvalidationTrigger>,
    prefix_tokens_reused: usize,
    prefix_identity: Option<PrefixCacheIdentity>,
    prompt_prefix_recorded: bool,
    execution_plan_digest: Option<String>,
    compile_path: Option<CompilePathEvidence>,
    kernel_count: usize,
    bytes_moved: u64,
    plan_cache_hits: usize,
    plan_cache_misses: usize,
    gpt_oss_perf: Option<GptOssPerformanceMetrics>,
    last_logits: Vec<f32>,
    sampler: GenerationSampler,
    generated_tokens: Vec<TokenId>,
    termination: Option<TerminationReason>,
}

impl<M> ScheduledGenerationState<M>
where
    M: CompiledWordGenerationModel,
{
    fn prepare(
        models: &mut InMemoryGenerationModelRegistry<M>,
        sessions: &mut InMemoryGenerationSessionStore,
        shared_prefixes: &mut SharedPrefixStore,
        request: &GenerationRequest,
    ) -> Result<Self, ReferenceTextGenerationError> {
        if !generation_product_supported(request) {
            return Err(ReferenceTextGenerationError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }

        let loaded_model = models
            .active(request.model.model.model_id.as_str())
            .ok_or_else(|| {
                ReferenceTextGenerationError::UnsupportedModel(request.model.model.model_id.clone())
            })?
            .clone();
        if loaded_model.descriptor() != &request.model {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }

        let model_id = request.model.model.model_id.clone();
        let load_state = models
            .load_state(model_id.as_str())
            .unwrap_or(GenerationLoadState::Warm);
        models.begin_request(model_id.as_str(), current_time_millis())?;
        let memory_plan = models.memory_plan(model_id.as_str()).cloned();
        let residency_policy = Some(models.residency_policy().clone());
        let residency_snapshot = Some(models.memory_snapshot());
        let served_artifact = served_artifact_identity_for_decoder_backend(
            loaded_model.descriptor(),
            loaded_model.backend_compatibility(),
            &[],
        );
        let effective_served_artifact_digest = effective_generation_served_artifact_digest(
            &served_artifact,
            request.adapter_serving.as_ref(),
        );

        let prepared = (|| -> Result<Self, ReferenceTextGenerationError> {
            let tokenizer = loaded_model.tokenizer();
            let prompt_tokens = loaded_model.encode_prompt_input(&request.prompt)?;
            if prompt_tokens.is_empty() {
                return Err(ReferenceTextGenerationError::EmptyPrompt);
            }

            let expected_kv_width = loaded_model.cache_width();
            let mut session_tokens = Vec::new();
            let prefix_compatibility = prefix_compatibility_for_request(&loaded_model, request);
            let prefix_policy = default_prefix_cache_policy();
            let mut prefix_state = PrefixCacheState::None;
            let mut prefix_cache_refusal_reason = None;
            let mut prefix_cache_invalidation_trigger = None;
            let mut prefix_tokens_reused = 0usize;
            let mut prefix_identity = None;
            let mut shared_prefix_eligible = false;
            let previous_kv_state = if let Some(session_id) = &request.session_id {
                if request.reset_session {
                    sessions.reset(session_id)?;
                }
                let state = sessions.state(session_id)?;
                validate_session_model(
                    state,
                    session_id,
                    loaded_model.descriptor(),
                    effective_served_artifact_digest.as_str(),
                )?;
                session_tokens = state.tokens().to_vec();
                if state.cache().is_empty() {
                    shared_prefix_eligible = true;
                } else {
                    prefix_state = PrefixCacheState::Bypassed;
                    prefix_cache_refusal_reason = Some(PrefixCacheRefusalReason::SessionBoundState);
                }
                state.cache().state()
            } else {
                shared_prefix_eligible = true;
                KvCacheState::default()
            };
            let preserve_prefix_tokens = usize::from(
                prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
            );
            let (prompt_tokens, context_window) = apply_context_window(
                &prompt_tokens,
                loaded_model.descriptor().config.max_context,
                previous_kv_state.tokens,
                request.options.max_output_tokens,
                request.options.context_overflow_policy,
                preserve_prefix_tokens,
            )?;
            let mut prompt_logits = Vec::new();
            let mut last_logits = Vec::new();
            let mut cache = if shared_prefix_eligible {
                let lookup = controlled_prefix_lookup(
                    shared_prefixes,
                    &prefix_compatibility,
                    &prompt_tokens,
                    request,
                );
                prefix_state = lookup.state;
                prefix_cache_refusal_reason = lookup.refusal_reason;
                prefix_cache_invalidation_trigger = lookup.invalidation_trigger;
                prefix_tokens_reused = lookup.reused_tokens;
                prefix_identity = lookup.identity;
                prompt_logits = lookup.prompt_logits;
                last_logits = if lookup.last_logits.is_empty() {
                    prompt_logits.last().cloned().unwrap_or_default()
                } else {
                    lookup.last_logits
                };
                lookup.cache.unwrap_or_else(|| {
                    InMemoryKvCache::new(
                        loaded_model.descriptor().config.max_context,
                        expected_kv_width,
                    )
                })
            } else if let Some(session_id) = &request.session_id {
                sessions.state(session_id)?.cache().clone()
            } else {
                InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            };
            if cache.width() != expected_kv_width {
                return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                    expected_kv_width,
                    kv_width: cache.width(),
                });
            }
            cache.bind_owner(request_kv_owner(
                request,
                psionic_runtime::BatchExecutionPosture::ContinuousBatch,
                None,
            ));
            let request_kv_checkpoint = cache.checkpoint();

            let prompt_prefix_recorded =
                !shared_prefix_eligible || prefix_tokens_reused == prompt_tokens.len();
            let prompt_eval_duration_ns =
                (prefix_tokens_reused == prompt_tokens.len()).then_some(0_u64);

            Ok(Self {
                request: request.clone(),
                loaded_model,
                model_id: model_id.clone(),
                served_artifact,
                load_state,
                generation_start: Instant::now(),
                memory_plan,
                residency_policy,
                residency_snapshot,
                prompt_eval_started_at: None,
                prompt_eval_duration_ns,
                prefill_handoff_state: (prefix_tokens_reused == prompt_tokens.len())
                    .then(|| cache.state()),
                first_token_emitted_at: None,
                last_token_emitted_at: None,
                context_window,
                previous_kv_state,
                cache,
                request_kv_checkpoint,
                session_tokens,
                prompt_tokens,
                prompt_logits,
                prefill_cursor: prefix_tokens_reused,
                prefix_compatibility,
                shared_prefix_eligible,
                prefix_policy,
                prefix_state,
                prefix_cache_refusal_reason,
                prefix_cache_invalidation_trigger,
                prefix_tokens_reused,
                prefix_identity,
                prompt_prefix_recorded,
                execution_plan_digest: None,
                compile_path: None,
                kernel_count: 0,
                bytes_moved: 0,
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                gpt_oss_perf: None,
                last_logits,
                sampler: GenerationSampler::new(&request.options)?,
                generated_tokens: Vec::new(),
                termination: None,
            })
        })();

        match prepared {
            Ok(state) => Ok(state),
            Err(error) => {
                let _ = models.finish_request(model_id.as_str(), current_time_millis());
                Err(error)
            }
        }
    }

    fn prefill_complete(&self) -> bool {
        self.prefill_cursor >= self.prompt_tokens.len()
    }

    fn is_finished(&self) -> bool {
        self.termination.is_some()
    }

    fn finish_request(&self, models: &mut InMemoryGenerationModelRegistry<M>) {
        let _ = models.finish_request(self.model_id.as_str(), current_time_millis());
    }

    fn bind_scheduler_owner(&mut self, queue_depth_at_admission: usize) {
        self.cache.bind_owner(request_kv_owner(
            &self.request,
            psionic_runtime::BatchExecutionPosture::ContinuousBatch,
            Some(queue_depth_at_admission),
        ));
    }

    fn step_prefill<B>(
        &mut self,
        backend: &mut B,
        shared_prefixes: &mut SharedPrefixStore,
        max_tokens: usize,
    ) -> Result<usize, ReferenceTextGenerationError>
    where
        M: CompiledWordGenerationModel<Backend = B>,
    {
        if self.prefill_complete() || max_tokens == 0 {
            return Ok(0);
        }
        let mut processed = 0usize;
        while processed < max_tokens && self.prefill_cursor < self.prompt_tokens.len() {
            if self.prompt_eval_started_at.is_none() {
                self.prompt_eval_started_at = Some(Instant::now());
            }
            let token = self.prompt_tokens.as_slice()[self.prefill_cursor];
            let step =
                self.loaded_model
                    .execute_step(backend, token, self.cache.len(), &self.cache)?;
            accumulate_generation_step_counters(
                &step,
                &mut self.execution_plan_digest,
                &mut self.compile_path,
                &mut self.kernel_count,
                &mut self.bytes_moved,
                &mut self.plan_cache_hits,
                &mut self.plan_cache_misses,
                &mut self.gpt_oss_perf,
            );
            self.cache.append(token, step.key, step.value)?;
            self.last_logits = step.logits;
            self.prompt_logits.push(self.last_logits.clone());
            self.prefill_cursor = self.prefill_cursor.saturating_add(1);
            processed = processed.saturating_add(1);
        }

        if self.prefill_complete() {
            if self.prompt_eval_duration_ns.is_none() {
                self.prompt_eval_duration_ns =
                    Some(self.prompt_eval_started_at.take().map_or(0, elapsed_ns));
            }
            if self.prefill_handoff_state.is_none() {
                self.prefill_handoff_state = Some(self.cache.state());
            }
            if self.shared_prefix_eligible && !self.prompt_prefix_recorded {
                let recorded_identity = shared_prefixes.record(
                    self.prefix_compatibility.clone(),
                    &self.prompt_tokens,
                    &self.prompt_logits,
                    &self.cache,
                );
                if self.prefix_state != PrefixCacheState::Hit || self.prefix_identity.is_none() {
                    self.prefix_identity = Some(recorded_identity);
                }
                self.prompt_prefix_recorded = true;
            }
        }

        Ok(processed)
    }

    fn step_decode<B>(
        &mut self,
        backend: &mut B,
        max_tokens: usize,
    ) -> Result<usize, ReferenceTextGenerationError>
    where
        M: CompiledWordGenerationModel<Backend = B>,
    {
        if !self.prefill_complete() || self.is_finished() || max_tokens == 0 {
            return Ok(0);
        }

        let mut decoded = 0usize;
        while decoded < max_tokens && self.termination.is_none() {
            if self.generated_tokens.len() >= self.request.options.max_output_tokens {
                self.termination = Some(TerminationReason::MaxOutputTokens);
                break;
            }
            if self.cache.len() >= self.cache.max_context() {
                self.termination = Some(TerminationReason::ContextLimit);
                break;
            }

            let next_token = match self.sampler.select_next_token(
                self.loaded_model.tokenizer(),
                &self.last_logits,
                &self.cache,
                self.generated_tokens.as_slice(),
            )? {
                GenerationSelection::Token(token) => token,
                GenerationSelection::Terminate => {
                    self.termination = Some(TerminationReason::EndOfSequence);
                    break;
                }
            };
            if self.loaded_model.is_end_of_sequence(next_token) {
                self.termination = Some(TerminationReason::EndOfSequence);
                break;
            }

            self.generated_tokens.push(next_token);
            let step = execute_generation_step_for_request(
                &self.loaded_model,
                backend,
                &self.request,
                next_token,
                self.cache.len(),
                &self.cache,
            )?;
            accumulate_generation_step_counters(
                &step,
                &mut self.execution_plan_digest,
                &mut self.compile_path,
                &mut self.kernel_count,
                &mut self.bytes_moved,
                &mut self.plan_cache_hits,
                &mut self.plan_cache_misses,
                &mut self.gpt_oss_perf,
            );
            self.cache.append(next_token, step.key, step.value)?;
            self.last_logits = step.logits;
            decoded = decoded.saturating_add(1);
            let emitted_at = Instant::now();
            if self.first_token_emitted_at.is_none() {
                self.first_token_emitted_at = Some(emitted_at);
            }
            self.last_token_emitted_at = Some(emitted_at);

            if truncate_generated_text(
                self.loaded_model.tokenizer(),
                &mut self.generated_tokens,
                &self.request.options.stop_sequences,
            )
            .is_some()
            {
                self.termination = Some(TerminationReason::EndOfSequence);
                break;
            }
        }

        Ok(decoded)
    }

    fn finalize(
        mut self,
        models: &mut InMemoryGenerationModelRegistry<M>,
        sessions: &mut InMemoryGenerationSessionStore,
        scheduler: Option<GenerationSchedulerRequestReceipt>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        let model_id = self.model_id.clone();
        let termination = self.termination.unwrap_or(TerminationReason::EndOfSequence);
        if let Some(session_id) = &self.request.session_id {
            self.session_tokens
                .extend_from_slice(self.prompt_tokens.as_slice());
            self.session_tokens
                .extend_from_slice(self.generated_tokens.as_slice());
            sessions.replace_cache(
                session_id,
                self.loaded_model.descriptor(),
                self.served_artifact.served_artifact_digest.as_str(),
                self.cache.clone(),
                TokenSequence::new(self.session_tokens),
            )?;
        }

        let generated = TokenSequence::new(self.generated_tokens);
        let text = self.loaded_model.tokenizer().decode(generated.as_slice());
        let usage = GenerationUsage {
            input_tokens: self.prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: self.cache.len(),
        };
        let total_duration_ns = elapsed_ns(self.generation_start);
        let time_to_first_token_ns = self
            .first_token_emitted_at
            .map(|first_token_at| first_token_at.duration_since(self.generation_start))
            .and_then(|duration| duration.as_nanos().try_into().ok());
        let inter_token_latency_ns = average_inter_token_latency_ns(
            self.first_token_emitted_at,
            self.last_token_emitted_at,
            usage.output_tokens,
        );
        let prefill_decode_handoff = self
            .prefill_handoff_state
            .as_ref()
            .and_then(local_prefill_decode_handoff);
        let kv_residency = host_only_kv_residency(self.cache.policy(), self.cache.state());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match self.load_state {
                GenerationLoadState::Cold => self.loaded_model.load_duration_ns(),
                GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(self.prompt_eval_duration_ns.unwrap_or(0)),
            context_window: Some(self.context_window),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(
                total_duration_ns.saturating_sub(self.prompt_eval_duration_ns.unwrap_or(0)),
            ),
            time_to_first_token_ns,
            inter_token_latency_ns,
            kv_cache: Some(KvCacheAccounting::from_states(
                &self.previous_kv_state,
                self.cache.state(),
            )),
            kv_residency: kv_residency.clone(),
            prefix_tokens_reused: Some(self.prefix_tokens_reused),
            gpt_oss_perf: self.gpt_oss_perf.filter(|perf| !perf.is_zero()),
        };
        let kv_ownership = self.cache.ownership_since(&self.request_kv_checkpoint);
        let delivery_plan_digest = self
            .execution_plan_digest
            .clone()
            .unwrap_or_else(|| self.loaded_model.plan_digest().to_string());
        let provenance = GenerationProvenance {
            served_artifact: self.served_artifact,
            adapter_serving: self.request.adapter_serving.clone(),
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            load_state: self.load_state,
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan: self.memory_plan,
            residency_policy: self.residency_policy,
            residency_snapshot: self.residency_snapshot,
            kv_cache_policy: Some(self.cache.policy().clone()),
            kv_ownership,
            prefix_cache_control: Some(self.request.prefix_cache_control.clone()),
            prefix_cache_state: Some(self.prefix_state),
            prefix_cache_refusal_reason: self.prefix_cache_refusal_reason,
            prefix_cache_policy: Some(self.prefix_policy.clone()),
            prefix_cache_identity: self.prefix_identity.clone(),
            compile_path: self.compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                self.kernel_count,
                self.bytes_moved,
                self.plan_cache_hits,
                self.plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
                prefill_decode_handoff.clone(),
                kv_residency,
            ),
            cache_observations: generation_cache_observations(
                self.loaded_model.descriptor(),
                self.compile_path.as_ref(),
                self.load_state,
                self.request.session_id.as_ref(),
                self.request.reset_session,
                &self.previous_kv_state,
                self.prefix_state,
                self.prefix_cache_invalidation_trigger,
            ),
            scheduler,
            structured_output: self.sampler.structured_output_report(),
        };
        let structured_output_value = self.sampler.structured_output_value(text.as_str())?;
        let response = GenerationResponse::new(
            &self.request,
            self.request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance);
        let response = if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        };
        let _ = models.finish_request(model_id.as_str(), current_time_millis());
        Ok(response)
    }
}

struct ActiveScheduledGenerationRequest<M>
where
    M: CompiledWordGenerationModel,
{
    index: usize,
    queue_depth_at_admission: usize,
    max_batch_size_observed: usize,
    prefill_tokens: usize,
    decode_tokens: usize,
    saw_prefill: bool,
    saw_decode: bool,
    state: ScheduledGenerationState<M>,
}

impl<M> ActiveScheduledGenerationRequest<M>
where
    M: CompiledWordGenerationModel,
{
    fn new(
        index: usize,
        queue_depth_at_admission: usize,
        mut state: ScheduledGenerationState<M>,
    ) -> Self {
        state.bind_scheduler_owner(queue_depth_at_admission);
        Self {
            index,
            queue_depth_at_admission,
            max_batch_size_observed: 1,
            prefill_tokens: 0,
            decode_tokens: 0,
            saw_prefill: false,
            saw_decode: false,
            state,
        }
    }

    fn scheduling_class(&self) -> GenerationSchedulingClass {
        match (self.saw_prefill, self.saw_decode) {
            (true, true) => GenerationSchedulingClass::MixedPrefillDecode,
            (true, false) => GenerationSchedulingClass::Prefill,
            (false, true) => GenerationSchedulingClass::Decode,
            (false, false) => GenerationSchedulingClass::FallbackSingleRequest,
        }
    }

    fn scheduler_receipt(
        &self,
        policy: &GenerationSchedulerPolicy,
    ) -> GenerationSchedulerRequestReceipt {
        let time_to_first_token_ns = self
            .state
            .first_token_emitted_at
            .map(|first_token_at| first_token_at.duration_since(self.state.generation_start))
            .and_then(|duration| duration.as_nanos().try_into().ok());
        let inter_token_latency_ns = average_inter_token_latency_ns(
            self.state.first_token_emitted_at,
            self.state.last_token_emitted_at,
            self.decode_tokens,
        );
        GenerationSchedulerRequestReceipt {
            policy: policy.clone(),
            batch_posture: psionic_runtime::BatchExecutionPosture::ContinuousBatch,
            queue_depth_at_admission: self.queue_depth_at_admission,
            max_batch_size_observed: self.max_batch_size_observed,
            scheduling_class: self.scheduling_class(),
            prefill_tokens: self.prefill_tokens,
            decode_tokens: self.decode_tokens,
            prefill_decode_mode: (self.prefill_tokens > 0 || self.decode_tokens > 0)
                .then_some(PrefillDecodeExecutionMode::DisaggregatedColocated),
            prefill_decode_handoff: self
                .state
                .prefill_handoff_state
                .as_ref()
                .and_then(local_prefill_decode_handoff),
            time_to_first_token_ns,
            inter_token_latency_ns,
            fallback_reason: None,
        }
    }
}

fn response_with_scheduler_receipt(
    mut response: GenerationResponse,
    receipt: GenerationSchedulerRequestReceipt,
) -> GenerationResponse {
    if let Some(provenance) = response.provenance.as_mut() {
        provenance.scheduler = Some(receipt);
    }
    response
}

fn generation_response_kv_ownership(
    response: &GenerationResponse,
) -> Option<&KvCacheOwnershipAccounting> {
    response
        .provenance
        .as_ref()
        .and_then(|provenance| provenance.kv_ownership.as_ref())
}

fn generation_response_prefill_decode_handoff(
    response: &GenerationResponse,
) -> Option<&PrefillDecodeHandoff> {
    response
        .provenance
        .as_ref()
        .and_then(|provenance| provenance.delivery_proof.as_ref())
        .and_then(|proof| proof.prefill_decode_handoff.as_ref())
}

fn accumulate_scheduler_timing_metrics(
    scheduler_metrics: &mut GenerationSchedulerMetrics,
    response: &GenerationResponse,
) {
    if let Some(time_to_first_token_ns) = response.metrics.time_to_first_token_ns {
        scheduler_metrics.total_time_to_first_token_ns = scheduler_metrics
            .total_time_to_first_token_ns
            .saturating_add(time_to_first_token_ns);
        scheduler_metrics.measured_time_to_first_token_requests = scheduler_metrics
            .measured_time_to_first_token_requests
            .saturating_add(1);
    }
    if let Some(inter_token_latency_ns) = response.metrics.inter_token_latency_ns {
        scheduler_metrics.total_inter_token_latency_ns = scheduler_metrics
            .total_inter_token_latency_ns
            .saturating_add(inter_token_latency_ns);
        scheduler_metrics.measured_inter_token_latency_requests = scheduler_metrics
            .measured_inter_token_latency_requests
            .saturating_add(1);
    }
}

fn update_scheduler_kv_peaks<M>(
    scheduler_metrics: &mut GenerationSchedulerMetrics,
    active: &VecDeque<ActiveScheduledGenerationRequest<M>>,
) where
    M: CompiledWordGenerationModel,
{
    let active_state = active
        .iter()
        .fold(KvCacheState::default(), |mut state, entry| {
            let cache_state = entry.state.cache.state();
            state.tokens = state.tokens.saturating_add(cache_state.tokens);
            state.bytes = state.bytes.saturating_add(cache_state.bytes);
            state.pages = state.pages.saturating_add(cache_state.pages);
            state
        });
    scheduler_metrics.peak_kv_pages_in_use = scheduler_metrics
        .peak_kv_pages_in_use
        .max(active_state.pages);
    scheduler_metrics.peak_kv_bytes_in_use = scheduler_metrics
        .peak_kv_bytes_in_use
        .max(active_state.bytes);
}

fn fallback_single_request_receipt(
    policy: &GenerationSchedulerPolicy,
    response: &GenerationResponse,
    fallback_reason: psionic_runtime::GenerationSchedulerFallbackReason,
) -> GenerationSchedulerRequestReceipt {
    GenerationSchedulerRequestReceipt {
        policy: policy.clone(),
        batch_posture: psionic_runtime::BatchExecutionPosture::SingleRequestOnly,
        queue_depth_at_admission: policy.max_queued_requests,
        max_batch_size_observed: 1,
        scheduling_class: GenerationSchedulingClass::FallbackSingleRequest,
        prefill_tokens: response
            .usage
            .input_tokens
            .saturating_sub(response.metrics.prefix_tokens_reused.unwrap_or(0)),
        decode_tokens: response.usage.output_tokens,
        prefill_decode_mode: generation_response_prefill_decode_handoff(response)
            .map(|handoff| handoff.mode)
            .or_else(|| {
                (response.usage.input_tokens > 0 || response.usage.output_tokens > 0)
                    .then_some(PrefillDecodeExecutionMode::DisaggregatedColocated)
            }),
        prefill_decode_handoff: generation_response_prefill_decode_handoff(response).cloned(),
        time_to_first_token_ns: response.metrics.time_to_first_token_ns,
        inter_token_latency_ns: response.metrics.inter_token_latency_ns,
        fallback_reason: Some(fallback_reason),
    }
}

fn run_continuous_batch_generation_requests<B, M>(
    backend: &mut B,
    models: &mut InMemoryGenerationModelRegistry<M>,
    sessions: &mut InMemoryGenerationSessionStore,
    shared_prefixes: &mut SharedPrefixStore,
    requests: Vec<GenerationRequest>,
    policy: GenerationSchedulerPolicy,
) -> ContinuousBatchGenerationResult
where
    M: CompiledWordGenerationModel<Backend = B>,
{
    let mut scheduler_metrics = GenerationSchedulerMetrics::for_policy(policy.clone());
    let mut responses = std::iter::repeat_with(|| None)
        .take(requests.len())
        .collect::<Vec<_>>();
    let total_capacity = policy.total_request_capacity();
    let mut waiting = VecDeque::new();

    for (index, request) in requests.into_iter().enumerate() {
        waiting.push_back((index, request));
    }

    let mut overflow = waiting.split_off(total_capacity.min(waiting.len()));
    while let Some((index, request)) = overflow.pop_front() {
        scheduler_metrics.record_fallback(
            psionic_runtime::GenerationSchedulerFallbackReason::QueueCapacityExceeded,
        );
        let result = run_generation_request(backend, models, sessions, shared_prefixes, &request)
            .map(|response| {
                let receipt = fallback_single_request_receipt(
                    &policy,
                    &response,
                    psionic_runtime::GenerationSchedulerFallbackReason::QueueCapacityExceeded,
                );
                response_with_scheduler_receipt(response, receipt)
            });
        if let Ok(response) = &result {
            accumulate_scheduler_timing_metrics(&mut scheduler_metrics, response);
        }
        responses[index] = Some(result);
    }

    let mut active = VecDeque::<ActiveScheduledGenerationRequest<M>>::new();
    while !waiting.is_empty() || !active.is_empty() {
        scheduler_metrics.max_queue_depth = scheduler_metrics.max_queue_depth.max(waiting.len());

        while active.len() < policy.max_active_requests && !waiting.is_empty() {
            let active_sessions = active
                .iter()
                .filter_map(|entry| entry.state.request.session_id.as_ref())
                .map(SessionId::as_str)
                .collect::<Vec<_>>();
            let next_index = waiting
                .iter()
                .position(|(_, request)| {
                    request
                        .session_id
                        .as_ref()
                        .map(|session_id| !active_sessions.contains(&session_id.as_str()))
                        .unwrap_or(true)
                })
                .unwrap_or(0);
            let Some((index, request)) = waiting.remove(next_index) else {
                break;
            };
            match ScheduledGenerationState::prepare(models, sessions, shared_prefixes, &request) {
                Ok(state) => {
                    scheduler_metrics.total_admitted_requests =
                        scheduler_metrics.total_admitted_requests.saturating_add(1);
                    active.push_back(ActiveScheduledGenerationRequest::new(
                        index,
                        waiting.len(),
                        state,
                    ));
                }
                Err(error) => {
                    responses[index] = Some(Err(error));
                }
            }
        }

        if active.is_empty() {
            continue;
        }
        update_scheduler_kv_peaks(&mut scheduler_metrics, &active);

        scheduler_metrics.total_cycles = scheduler_metrics.total_cycles.saturating_add(1);
        scheduler_metrics.max_batch_size = scheduler_metrics.max_batch_size.max(active.len());
        let active_batch_size = active.len();

        let mut decode_budget = policy.max_decode_tokens_per_tick;
        let mut cycle_decode_tokens = 0usize;
        while decode_budget > 0
            && active
                .iter()
                .any(|entry| entry.state.prefill_complete() && !entry.state.is_finished())
        {
            let active_len = active.len();
            let mut made_progress = false;
            for _ in 0..active_len {
                let Some(mut entry) = active.pop_front() else {
                    break;
                };
                entry.max_batch_size_observed =
                    entry.max_batch_size_observed.max(active_batch_size);
                if entry.state.prefill_complete() && !entry.state.is_finished() && decode_budget > 0
                {
                    entry.saw_decode = true;
                    match entry.state.step_decode(backend, 1) {
                        Ok(decoded) => {
                            if decoded > 0 {
                                entry.decode_tokens = entry.decode_tokens.saturating_add(decoded);
                                scheduler_metrics.total_decode_tokens = scheduler_metrics
                                    .total_decode_tokens
                                    .saturating_add(decoded);
                                cycle_decode_tokens = cycle_decode_tokens.saturating_add(decoded);
                                decode_budget = decode_budget.saturating_sub(decoded);
                                made_progress = true;
                            }
                        }
                        Err(error) => {
                            entry.state.finish_request(models);
                            responses[entry.index] = Some(Err(error));
                            continue;
                        }
                    }
                }
                if entry.state.is_finished() {
                    let receipt = entry.scheduler_receipt(&policy);
                    match entry.state.finalize(models, sessions, Some(receipt)) {
                        Ok(response) => {
                            if let Some(kv_ownership) = generation_response_kv_ownership(&response)
                            {
                                scheduler_metrics.total_kv_pages_allocated = scheduler_metrics
                                    .total_kv_pages_allocated
                                    .saturating_add(kv_ownership.allocated_pages.len());
                                scheduler_metrics.total_kv_pages_reclaimed = scheduler_metrics
                                    .total_kv_pages_reclaimed
                                    .saturating_add(kv_ownership.reclaimed_pages.len());
                                scheduler_metrics.total_kv_bytes_allocated =
                                    scheduler_metrics.total_kv_bytes_allocated.saturating_add(
                                        kv_ownership
                                            .allocated_pages
                                            .iter()
                                            .map(|page| page.bytes_used)
                                            .sum::<u64>(),
                                    );
                                scheduler_metrics.total_kv_bytes_reclaimed =
                                    scheduler_metrics.total_kv_bytes_reclaimed.saturating_add(
                                        kv_ownership
                                            .reclaimed_pages
                                            .iter()
                                            .map(|page| page.bytes_used)
                                            .sum::<u64>(),
                                    );
                            }
                            accumulate_scheduler_timing_metrics(&mut scheduler_metrics, &response);
                            scheduler_metrics.total_completed_requests =
                                scheduler_metrics.total_completed_requests.saturating_add(1);
                            responses[entry.index] = Some(Ok(response));
                        }
                        Err(error) => {
                            responses[entry.index] = Some(Err(error));
                        }
                    }
                    continue;
                }
                active.push_back(entry);
            }
            if !made_progress {
                break;
            }
        }

        let mut prefill_budget = policy.max_prefill_tokens_per_tick;
        let mut cycle_prefill_tokens = 0usize;
        while prefill_budget > 0
            && active
                .iter()
                .any(|entry| !entry.state.prefill_complete() && !entry.state.is_finished())
        {
            let active_len = active.len();
            let mut made_progress = false;
            for _ in 0..active_len {
                let Some(mut entry) = active.pop_front() else {
                    break;
                };
                entry.max_batch_size_observed =
                    entry.max_batch_size_observed.max(active_batch_size);
                if !entry.state.prefill_complete()
                    && !entry.state.is_finished()
                    && prefill_budget > 0
                {
                    match entry.state.step_prefill(backend, shared_prefixes, 1) {
                        Ok(prefilled) => {
                            if prefilled > 0 {
                                entry.saw_prefill = true;
                                entry.prefill_tokens =
                                    entry.prefill_tokens.saturating_add(prefilled);
                                scheduler_metrics.total_prefill_tokens = scheduler_metrics
                                    .total_prefill_tokens
                                    .saturating_add(prefilled);
                                cycle_prefill_tokens =
                                    cycle_prefill_tokens.saturating_add(prefilled);
                                prefill_budget = prefill_budget.saturating_sub(prefilled);
                                made_progress = true;
                            }
                        }
                        Err(error) => {
                            entry.state.finish_request(models);
                            responses[entry.index] = Some(Err(error));
                            continue;
                        }
                    }
                }
                active.push_back(entry);
            }
            if !made_progress {
                break;
            }
        }
        update_scheduler_kv_peaks(&mut scheduler_metrics, &active);

        let observed_class = if cycle_decode_tokens > 0 && cycle_prefill_tokens > 0 {
            Some(GenerationSchedulingClass::MixedPrefillDecode)
        } else if cycle_decode_tokens > 0 {
            Some(GenerationSchedulingClass::Decode)
        } else if cycle_prefill_tokens > 0 {
            Some(GenerationSchedulingClass::Prefill)
        } else {
            None
        };
        scheduler_metrics.last_scheduling_class =
            match (scheduler_metrics.last_scheduling_class, observed_class) {
                (Some(GenerationSchedulingClass::MixedPrefillDecode), _) => {
                    Some(GenerationSchedulingClass::MixedPrefillDecode)
                }
                (
                    Some(GenerationSchedulingClass::Prefill),
                    Some(GenerationSchedulingClass::Decode),
                )
                | (
                    Some(GenerationSchedulingClass::Decode),
                    Some(GenerationSchedulingClass::Prefill),
                )
                | (_, Some(GenerationSchedulingClass::MixedPrefillDecode)) => {
                    Some(GenerationSchedulingClass::MixedPrefillDecode)
                }
                (_, Some(observed_class)) => Some(observed_class),
                (current, None) => current,
            };
    }

    ContinuousBatchGenerationResult {
        responses: responses
            .into_iter()
            .map(|response| {
                response.unwrap_or_else(|| {
                    Err(ReferenceTextGenerationError::Runtime(
                        RuntimeError::Backend(String::from(
                            "continuous batch scheduler dropped a response",
                        )),
                    ))
                })
            })
            .collect(),
        scheduler_metrics,
    }
}

/// Pull-driven CPU generation stream for the local runtime API.
struct CpuGenerationStream<'a, M>
where
    M: WordDecoderExecutionModel,
{
    backend: &'a mut CpuBackend,
    models: &'a mut InMemoryGenerationModelRegistry<CpuWordGenerationModel<M>>,
    sessions: &'a mut InMemoryGenerationSessionStore,
    request: GenerationRequest,
    loaded_model: CpuWordGenerationModel<M>,
    model_id: String,
    served_artifact: ServedArtifactIdentity,
    load_state: GenerationLoadState,
    generation_start: Instant,
    streaming_policy: GenerationStreamingPolicy,
    memory_plan: Option<ModelMemoryPlan>,
    residency_policy: Option<ModelResidencyPolicy>,
    residency_snapshot: Option<MemoryResidencySnapshot>,
    prompt_eval_duration_ns: u64,
    prefill_handoff_state: KvCacheState,
    context_window: ContextWindowAccounting,
    previous_kv_state: KvCacheState,
    cache: InMemoryKvCache,
    request_kv_checkpoint: KvCacheLedgerCheckpoint,
    sampler: GenerationSampler,
    session_tokens: Vec<TokenId>,
    prompt_tokens: TokenSequence,
    prefix_policy: PrefixCacheReusePolicy,
    prefix_state: PrefixCacheState,
    prefix_cache_refusal_reason: Option<PrefixCacheRefusalReason>,
    prefix_cache_invalidation_trigger: Option<CacheInvalidationTrigger>,
    prefix_tokens_reused: usize,
    prefix_identity: Option<PrefixCacheIdentity>,
    execution_plan_digest: String,
    compile_path: Option<CompilePathEvidence>,
    kernel_count: usize,
    bytes_moved: u64,
    plan_cache_hits: usize,
    plan_cache_misses: usize,
    last_logits: Vec<f32>,
    generated_tokens: Vec<TokenId>,
    first_token_emitted_at: Option<Instant>,
    last_token_emitted_at: Option<Instant>,
    emitted_token_count: usize,
    emitted_text_bytes: usize,
    pending_terminal: Option<GenerationStreamTerminal>,
    request_finished: bool,
}

impl<'a, M> CpuGenerationStream<'a, M>
where
    M: WordDecoderExecutionModel,
{
    fn new(
        backend: &'a mut CpuBackend,
        models: &'a mut InMemoryGenerationModelRegistry<CpuWordGenerationModel<M>>,
        sessions: &'a mut InMemoryGenerationSessionStore,
        shared_prefixes: &'a mut SharedPrefixStore,
        request: &GenerationRequest,
    ) -> Result<Self, ReferenceTextGenerationError> {
        if !generation_product_supported(request) {
            return Err(ReferenceTextGenerationError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }

        let loaded_model = models
            .active(request.model.model.model_id.as_str())
            .ok_or_else(|| {
                ReferenceTextGenerationError::UnsupportedModel(request.model.model.model_id.clone())
            })?
            .clone();
        if loaded_model.descriptor() != &request.model {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }

        let model_id = request.model.model.model_id.clone();
        let load_state = models
            .load_state(model_id.as_str())
            .unwrap_or(GenerationLoadState::Warm);
        let request_start = current_time_millis();
        models.begin_request(model_id.as_str(), request_start)?;
        let streaming_policy = default_generation_streaming_policy();
        let memory_plan = models.memory_plan(model_id.as_str()).cloned();
        let residency_policy = Some(models.residency_policy().clone());
        let residency_snapshot = Some(models.memory_snapshot());
        let generation_start = Instant::now();
        let served_artifact = served_artifact_identity_for_decoder_backend(
            loaded_model.descriptor(),
            loaded_model.backend_compatibility(),
            &[],
        );
        let effective_served_artifact_digest = effective_generation_served_artifact_digest(
            &served_artifact,
            request.adapter_serving.as_ref(),
        );

        let prepared = (|| -> Result<_, ReferenceTextGenerationError> {
            let prompt_eval_start = Instant::now();
            let tokenizer = loaded_model.tokenizer();
            let prompt_tokens = loaded_model.encode_prompt_input(&request.prompt)?;
            if prompt_tokens.is_empty() {
                return Err(ReferenceTextGenerationError::EmptyPrompt);
            }

            let expected_kv_width = loaded_model.cache_width();
            let mut session_tokens = Vec::new();
            let compatibility = prefix_compatibility_for_request(&loaded_model, request);
            let prefix_policy = default_prefix_cache_policy();
            let mut prefix_state = PrefixCacheState::None;
            let mut prefix_cache_refusal_reason = None;
            let mut prefix_cache_invalidation_trigger = None;
            let mut prefix_tokens_reused = 0usize;
            let mut prefix_identity = None;
            let mut shared_prefix_eligible = false;
            let previous_kv_state = if let Some(session_id) = &request.session_id {
                if request.reset_session {
                    sessions.reset(session_id)?;
                }
                let state = sessions.state(session_id)?;
                validate_session_model(
                    state,
                    session_id,
                    loaded_model.descriptor(),
                    effective_served_artifact_digest.as_str(),
                )?;
                session_tokens = state.tokens().to_vec();
                if state.cache().is_empty() {
                    shared_prefix_eligible = true;
                } else {
                    prefix_state = PrefixCacheState::Bypassed;
                    prefix_cache_refusal_reason = Some(PrefixCacheRefusalReason::SessionBoundState);
                }
                state.cache().state()
            } else {
                shared_prefix_eligible = true;
                KvCacheState::default()
            };
            let preserve_prefix_tokens = usize::from(
                prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
            );
            let (prompt_tokens, context_window) = apply_context_window(
                &prompt_tokens,
                loaded_model.descriptor().config.max_context,
                previous_kv_state.tokens,
                request.options.max_output_tokens,
                request.options.context_overflow_policy,
                preserve_prefix_tokens,
            )?;
            let mut prompt_logits = Vec::new();
            let mut last_logits = Vec::new();
            let mut execution_plan_digest = None;
            let mut compile_path = None;
            let mut kernel_count = 0usize;
            let mut bytes_moved = 0u64;
            let mut plan_cache_hits = 0usize;
            let mut plan_cache_misses = 0usize;
            let mut cache = if shared_prefix_eligible {
                let lookup = controlled_prefix_lookup(
                    shared_prefixes,
                    &compatibility,
                    &prompt_tokens,
                    request,
                );
                prefix_state = lookup.state;
                prefix_cache_refusal_reason = lookup.refusal_reason;
                prefix_cache_invalidation_trigger = lookup.invalidation_trigger;
                prefix_tokens_reused = lookup.reused_tokens;
                prefix_identity = lookup.identity;
                prompt_logits = lookup.prompt_logits;
                last_logits = if lookup.last_logits.is_empty() {
                    prompt_logits.last().cloned().unwrap_or_default()
                } else {
                    lookup.last_logits
                };
                lookup.cache.unwrap_or_else(|| {
                    InMemoryKvCache::new(
                        loaded_model.descriptor().config.max_context,
                        expected_kv_width,
                    )
                })
            } else if let Some(session_id) = &request.session_id {
                sessions.state(session_id)?.cache().clone()
            } else {
                InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            };
            if cache.width() != expected_kv_width {
                return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                    expected_kv_width,
                    kv_width: cache.width(),
                });
            }
            cache.bind_owner(request_kv_owner(
                request,
                psionic_runtime::BatchExecutionPosture::SingleRequestOnly,
                None,
            ));
            let request_kv_checkpoint = cache.checkpoint();
            for token in &prompt_tokens.as_slice()[prefix_tokens_reused..] {
                let step = execute_generation_step_for_request(
                    &loaded_model,
                    backend,
                    request,
                    *token,
                    cache.len(),
                    &cache,
                )?;
                if execution_plan_digest.is_none() {
                    execution_plan_digest = step.execution_plan_digest.clone();
                }
                if compile_path.is_none() {
                    compile_path = step.compile_path.clone();
                }
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
                plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
                cache.append(*token, step.key, step.value)?;
                last_logits = step.logits;
                prompt_logits.push(last_logits.clone());
            }

            if shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len() {
                let recorded_identity =
                    shared_prefixes.record(compatibility, &prompt_tokens, &prompt_logits, &cache);
                if prefix_state != PrefixCacheState::Hit || prefix_identity.is_none() {
                    prefix_identity = Some(recorded_identity);
                }
            }

            let prompt_eval_duration_ns = elapsed_ns(prompt_eval_start);
            let prefill_handoff_state = cache.state();

            Ok((
                prompt_tokens,
                context_window,
                previous_kv_state,
                cache,
                request_kv_checkpoint,
                session_tokens,
                prefix_policy,
                prefix_state,
                prefix_cache_refusal_reason,
                prefix_cache_invalidation_trigger,
                prefix_tokens_reused,
                prefix_identity,
                prompt_eval_duration_ns,
                prefill_handoff_state,
                execution_plan_digest,
                compile_path,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                last_logits,
            ))
        })();

        match prepared {
            Ok((
                prompt_tokens,
                context_window,
                previous_kv_state,
                cache,
                request_kv_checkpoint,
                session_tokens,
                prefix_policy,
                prefix_state,
                prefix_cache_refusal_reason,
                prefix_cache_invalidation_trigger,
                prefix_tokens_reused,
                prefix_identity,
                prompt_eval_duration_ns,
                prefill_handoff_state,
                execution_plan_digest,
                compile_path,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                last_logits,
            )) => Ok(Self {
                execution_plan_digest: execution_plan_digest
                    .unwrap_or_else(|| loaded_model.plan_digest().to_string()),
                backend,
                models,
                sessions,
                request: request.clone(),
                loaded_model,
                model_id,
                served_artifact,
                load_state,
                generation_start,
                streaming_policy,
                memory_plan,
                residency_policy,
                residency_snapshot,
                prompt_eval_duration_ns,
                prefill_handoff_state,
                context_window,
                previous_kv_state,
                cache,
                request_kv_checkpoint,
                sampler: GenerationSampler::new(&request.options)?,
                session_tokens,
                prompt_tokens,
                prefix_policy,
                prefix_state,
                prefix_cache_refusal_reason,
                prefix_cache_invalidation_trigger,
                prefix_tokens_reused,
                prefix_identity,
                compile_path,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                last_logits,
                generated_tokens: Vec::new(),
                first_token_emitted_at: None,
                last_token_emitted_at: None,
                emitted_token_count: 0,
                emitted_text_bytes: 0,
                pending_terminal: None,
                request_finished: false,
            }),
            Err(error) => {
                let _ = models.finish_request(model_id.as_str(), current_time_millis());
                Err(error)
            }
        }
    }

    fn finish_request_once(&mut self) {
        if !self.request_finished {
            let _ = self
                .models
                .finish_request(self.model_id.as_str(), current_time_millis());
            self.request_finished = true;
        }
    }

    fn build_response(
        &self,
        output_tokens: &[TokenId],
        termination: TerminationReason,
        streaming_policy: bool,
    ) -> GenerationResponse {
        let generated = TokenSequence::new(output_tokens.to_vec());
        let usage = GenerationUsage {
            input_tokens: self.prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: self.cache.len(),
        };
        let total_duration_ns = elapsed_ns(self.generation_start);
        let time_to_first_token_ns = self
            .first_token_emitted_at
            .map(|first_token_at| first_token_at.duration_since(self.generation_start))
            .and_then(|duration| duration.as_nanos().try_into().ok());
        let inter_token_latency_ns = average_inter_token_latency_ns(
            self.first_token_emitted_at,
            self.last_token_emitted_at,
            usage.output_tokens,
        );
        let prefill_decode_handoff = local_prefill_decode_handoff(&self.prefill_handoff_state);
        let kv_residency = host_only_kv_residency(self.cache.policy(), self.cache.state());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match self.load_state {
                GenerationLoadState::Cold => self.loaded_model.load_duration_ns(),
                GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(self.prompt_eval_duration_ns),
            context_window: Some(self.context_window.clone()),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(total_duration_ns.saturating_sub(self.prompt_eval_duration_ns)),
            time_to_first_token_ns,
            inter_token_latency_ns,
            kv_cache: Some(KvCacheAccounting::from_states(
                &self.previous_kv_state,
                self.cache.state(),
            )),
            kv_residency: kv_residency.clone(),
            prefix_tokens_reused: Some(self.prefix_tokens_reused),
            gpt_oss_perf: None,
        };
        let provenance = GenerationProvenance {
            served_artifact: self.served_artifact.clone(),
            adapter_serving: self.request.adapter_serving.clone(),
            execution_plan_digest: self.execution_plan_digest.clone(),
            cluster_execution: None,
            load_state: self.load_state,
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: streaming_policy.then(|| self.streaming_policy.clone()),
            memory_plan: self.memory_plan.clone(),
            residency_policy: self.residency_policy.clone(),
            residency_snapshot: self.residency_snapshot.clone(),
            kv_cache_policy: Some(self.cache.policy().clone()),
            kv_ownership: self.cache.ownership_since(&self.request_kv_checkpoint),
            prefix_cache_control: Some(self.request.prefix_cache_control.clone()),
            prefix_cache_state: Some(self.prefix_state),
            prefix_cache_refusal_reason: self.prefix_cache_refusal_reason,
            prefix_cache_policy: Some(self.prefix_policy.clone()),
            prefix_cache_identity: self.prefix_identity.clone(),
            compile_path: self.compile_path.clone(),
            delivery_proof: build_delivery_proof(
                self.execution_plan_digest.clone(),
                self.kernel_count,
                self.bytes_moved,
                self.plan_cache_hits,
                self.plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
                prefill_decode_handoff,
                kv_residency,
            ),
            cache_observations: generation_cache_observations(
                self.loaded_model.descriptor(),
                self.compile_path.as_ref(),
                self.load_state,
                self.request.session_id.as_ref(),
                self.request.reset_session,
                &self.previous_kv_state,
                self.prefix_state,
                self.prefix_cache_invalidation_trigger,
            ),
            scheduler: None,
            structured_output: self.sampler.structured_output_report(),
        };
        let text = self.loaded_model.model().tokenizer().decode(output_tokens);
        let structured_output_value = self
            .sampler
            .structured_output_value(text.as_str())
            .ok()
            .flatten();
        let response = GenerationResponse::new(
            &self.request,
            self.request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance);
        if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        }
    }

    fn build_terminal(
        &mut self,
        status: GenerationStreamStatus,
        termination: TerminationReason,
        failure_reason: Option<String>,
        diagnostic: Option<LocalRuntimeDiagnostic>,
    ) -> GenerationStreamTerminal {
        if status == GenerationStreamStatus::Succeeded {
            if let Some(session_id) = &self.request.session_id {
                let mut committed_tokens = self.session_tokens.clone();
                committed_tokens.extend_from_slice(self.prompt_tokens.as_slice());
                committed_tokens.extend_from_slice(self.generated_tokens.as_slice());
                let _ = self.sessions.replace_cache(
                    session_id,
                    self.loaded_model.descriptor(),
                    self.served_artifact.served_artifact_digest.as_str(),
                    self.cache.clone(),
                    TokenSequence::new(committed_tokens),
                );
            }
        }
        let response = self.build_response(&self.generated_tokens, termination, true);
        self.finish_request_once();
        GenerationStreamTerminal {
            status,
            response,
            failure_reason,
            diagnostic,
        }
    }

    fn maybe_emit_chunk(&mut self, allow_full_flush: bool) -> Option<GenerationStreamChunk> {
        let tokenizer = self.loaded_model.model().tokenizer();
        let full_text = tokenizer.decode(self.generated_tokens.as_slice());
        let reserved_chars = if allow_full_flush {
            0
        } else {
            self.request
                .options
                .stop_sequences
                .iter()
                .filter(|stop| !stop.is_empty())
                .map(|stop| stop.chars().count())
                .max()
                .unwrap_or(0)
                .saturating_sub(1)
        };
        let safe_text = text_prefix_without_trailing_chars(full_text.as_str(), reserved_chars);
        let safe_token_count = token_count_for_decoded_prefix(
            self.loaded_model.model().tokenizer(),
            self.generated_tokens.as_slice(),
            safe_text,
        );
        if safe_token_count <= self.emitted_token_count {
            return None;
        }

        let delta_tokens =
            self.generated_tokens[self.emitted_token_count..safe_token_count].to_vec();
        let delta_text = safe_text[self.emitted_text_bytes..].to_string();
        self.emitted_token_count = safe_token_count;
        self.emitted_text_bytes = safe_text.len();
        Some(GenerationStreamChunk {
            request_id: self.request.request_id.clone(),
            model_id: self.request.model.model.model_id.clone(),
            session_id: self.request.session_id.clone(),
            output: GenerationOutput {
                tokens: TokenSequence::new(delta_tokens),
                text: delta_text,
                structured: None,
                harmony: None,
            },
            cumulative_output_tokens: self.emitted_token_count,
        })
    }

    fn emit_terminal_or_chunk(
        &mut self,
        status: GenerationStreamStatus,
        termination: TerminationReason,
        failure_reason: Option<String>,
        diagnostic: Option<LocalRuntimeDiagnostic>,
    ) -> GenerationStreamEvent {
        let terminal = self.build_terminal(status, termination, failure_reason, diagnostic);
        if let Some(chunk) = self.maybe_emit_chunk(true) {
            self.pending_terminal = Some(terminal);
            GenerationStreamEvent::Chunk(chunk)
        } else {
            GenerationStreamEvent::Terminal(terminal)
        }
    }
}

impl<M> GenerationEventStream for CpuGenerationStream<'_, M>
where
    M: WordDecoderExecutionModel,
{
    fn policy(&self) -> &GenerationStreamingPolicy {
        &self.streaming_policy
    }

    fn next_event(&mut self) -> Option<GenerationStreamEvent> {
        if let Some(terminal) = self.pending_terminal.take() {
            return Some(GenerationStreamEvent::Terminal(terminal));
        }
        if self.request_finished {
            return None;
        }

        loop {
            if self.generated_tokens.len() >= self.request.options.max_output_tokens {
                return Some(self.emit_terminal_or_chunk(
                    GenerationStreamStatus::Succeeded,
                    TerminationReason::MaxOutputTokens,
                    None,
                    None,
                ));
            }
            if self.cache.len() >= self.cache.max_context() {
                return Some(self.emit_terminal_or_chunk(
                    GenerationStreamStatus::Succeeded,
                    TerminationReason::ContextLimit,
                    None,
                    None,
                ));
            }

            let next_token = match self.sampler.select_next_token(
                self.loaded_model.model().tokenizer(),
                &self.last_logits,
                &self.cache,
                self.generated_tokens.as_slice(),
            ) {
                Ok(GenerationSelection::Token(token)) => token,
                Ok(GenerationSelection::Terminate) => {
                    return Some(self.emit_terminal_or_chunk(
                        GenerationStreamStatus::Succeeded,
                        TerminationReason::EndOfSequence,
                        None,
                        None,
                    ));
                }
                Err(error) => {
                    return Some(
                        self.emit_terminal_or_chunk(
                            GenerationStreamStatus::Failed,
                            TerminationReason::Error,
                            Some(error.to_string()),
                            Some(
                                error
                                    .diagnostic_for_request(&self.request)
                                    .with_backend("cpu"),
                            ),
                        ),
                    );
                }
            };
            if self.loaded_model.is_end_of_sequence(next_token) {
                return Some(self.emit_terminal_or_chunk(
                    GenerationStreamStatus::Succeeded,
                    TerminationReason::EndOfSequence,
                    None,
                    None,
                ));
            }

            self.generated_tokens.push(next_token);
            match execute_generation_step_for_request(
                &self.loaded_model,
                self.backend,
                &self.request,
                next_token,
                self.cache.len(),
                &self.cache,
            ) {
                Ok(step) => {
                    if self.compile_path.is_none() {
                        self.compile_path = step.compile_path.clone();
                    }
                    if let Some(digest) = step.execution_plan_digest.clone() {
                        self.execution_plan_digest = digest;
                    }
                    self.kernel_count = self.kernel_count.saturating_add(step.kernel_count);
                    self.bytes_moved = self.bytes_moved.saturating_add(step.bytes_moved);
                    self.plan_cache_hits =
                        self.plan_cache_hits.saturating_add(step.plan_cache_hits);
                    self.plan_cache_misses = self
                        .plan_cache_misses
                        .saturating_add(step.plan_cache_misses);
                    if let Err(error) = self.cache.append(next_token, step.key, step.value) {
                        return Some(self.emit_terminal_or_chunk(
                            GenerationStreamStatus::Failed,
                            TerminationReason::Error,
                            Some(error.to_string()),
                            Some(diagnostic_with_request_context(
                                kv_cache_diagnostic(&error).with_backend("cpu"),
                                &self.request.product_id,
                                &self.request.model.model.model_id,
                            )),
                        ));
                    }
                    self.last_logits = step.logits;
                    let emitted_at = Instant::now();
                    if self.first_token_emitted_at.is_none() {
                        self.first_token_emitted_at = Some(emitted_at);
                    }
                    self.last_token_emitted_at = Some(emitted_at);
                }
                Err(error) => {
                    return Some(self.emit_terminal_or_chunk(
                        GenerationStreamStatus::Failed,
                        TerminationReason::Error,
                        Some(error.to_string()),
                        Some(error.diagnostic_for_request(&self.request)),
                    ));
                }
            }

            if truncate_generated_text(
                self.loaded_model.model().tokenizer(),
                &mut self.generated_tokens,
                &self.request.options.stop_sequences,
            )
            .is_some()
            {
                return Some(self.emit_terminal_or_chunk(
                    GenerationStreamStatus::Succeeded,
                    TerminationReason::EndOfSequence,
                    None,
                    None,
                ));
            }

            if let Some(chunk) = self.maybe_emit_chunk(false) {
                return Some(GenerationStreamEvent::Chunk(chunk));
            }
        }
    }

    fn cancel(&mut self) -> Option<GenerationStreamTerminal> {
        if self.request_finished {
            return None;
        }
        let terminal = self.build_terminal(
            GenerationStreamStatus::Cancelled,
            TerminationReason::Cancelled,
            Some(String::from("stream cancelled by caller")),
            Some(diagnostic_with_request_context(
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::Cancelled,
                    499,
                    "stream cancelled by caller",
                )
                .with_backend("cpu"),
                &self.request.product_id,
                &self.request.model.model.model_id,
            )),
        );
        self.pending_terminal = None;
        Some(terminal)
    }

    fn disconnect(&mut self) -> Option<GenerationStreamTerminal> {
        if self.request_finished {
            return None;
        }
        let terminal = self.build_terminal(
            GenerationStreamStatus::Disconnected,
            TerminationReason::Disconnected,
            Some(String::from("stream disconnected by caller")),
            Some(diagnostic_with_request_context(
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::Disconnected,
                    499,
                    "stream disconnected by caller",
                )
                .with_backend("cpu"),
                &self.request.product_id,
                &self.request.model.model.model_id,
            )),
        );
        self.pending_terminal = None;
        Some(terminal)
    }
}

impl<M> Drop for CpuGenerationStream<'_, M>
where
    M: WordDecoderExecutionModel,
{
    fn drop(&mut self) {
        self.finish_request_once();
    }
}

fn run_generation_request<B, M>(
    backend: &mut B,
    models: &mut InMemoryGenerationModelRegistry<M>,
    sessions: &mut InMemoryGenerationSessionStore,
    shared_prefixes: &mut SharedPrefixStore,
    request: &GenerationRequest,
) -> Result<GenerationResponse, ReferenceTextGenerationError>
where
    M: CompiledWordGenerationModel<Backend = B>,
{
    if !generation_product_supported(request) {
        return Err(ReferenceTextGenerationError::UnsupportedProduct(
            request.product_id.clone(),
        ));
    }

    let loaded_model = models
        .active(request.model.model.model_id.as_str())
        .ok_or_else(|| {
            ReferenceTextGenerationError::UnsupportedModel(request.model.model.model_id.clone())
        })?
        .clone();
    if loaded_model.descriptor() != &request.model {
        return Err(ReferenceTextGenerationError::UnsupportedModel(
            request.model.model.model_id.clone(),
        ));
    }

    let model_id = request.model.model.model_id.as_str();
    let load_state = models
        .load_state(model_id)
        .unwrap_or(GenerationLoadState::Warm);
    let request_start = current_time_millis();
    let generation_start = Instant::now();
    models.begin_request(model_id, request_start)?;
    let memory_plan = models.memory_plan(model_id).cloned();
    let residency_policy = Some(models.residency_policy().clone());
    let residency_snapshot = Some(models.memory_snapshot());
    let served_artifact = served_artifact_identity_for_decoder_backend(
        loaded_model.descriptor(),
        loaded_model.backend_compatibility(),
        &[],
    );
    let effective_served_artifact_digest = effective_generation_served_artifact_digest(
        &served_artifact,
        request.adapter_serving.as_ref(),
    );

    let result = (|| -> Result<GenerationResponse, ReferenceTextGenerationError> {
        let prompt_eval_start = Instant::now();
        let tokenizer = loaded_model.tokenizer();
        let prompt_tokens = loaded_model.encode_prompt_input(&request.prompt)?;
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }

        let expected_kv_width = loaded_model.cache_width();
        let mut session_tokens = Vec::new();
        let compatibility = prefix_compatibility_for_request(&loaded_model, request);
        let prefix_policy = default_prefix_cache_policy();
        let mut prefix_state = PrefixCacheState::None;
        let mut prefix_cache_refusal_reason = None;
        let mut prefix_cache_invalidation_trigger = None;
        let mut prefix_tokens_reused = 0usize;
        let mut prefix_identity = None;
        let mut shared_prefix_eligible = false;
        let previous_kv_state = if let Some(session_id) = &request.session_id {
            if request.reset_session {
                sessions.reset(session_id)?;
            }
            let state = sessions.state(session_id)?;
            validate_session_model(
                state,
                session_id,
                loaded_model.descriptor(),
                effective_served_artifact_digest.as_str(),
            )?;
            session_tokens = state.tokens().to_vec();
            if state.cache().is_empty() {
                shared_prefix_eligible = true;
            } else {
                prefix_state = PrefixCacheState::Bypassed;
                prefix_cache_refusal_reason = Some(PrefixCacheRefusalReason::SessionBoundState);
            }
            state.cache().state()
        } else {
            shared_prefix_eligible = true;
            KvCacheState::default()
        };
        let preserve_prefix_tokens = usize::from(
            prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
        );
        let (prompt_tokens, context_window) = apply_context_window(
            &prompt_tokens,
            loaded_model.descriptor().config.max_context,
            previous_kv_state.tokens,
            request.options.max_output_tokens,
            request.options.context_overflow_policy,
            preserve_prefix_tokens,
        )?;
        let mut prompt_logits = Vec::new();
        let mut last_logits = Vec::new();
        let mut execution_plan_digest = None;
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        let mut gpt_oss_perf: Option<GptOssPerformanceMetrics> = None;
        let mut cache = if shared_prefix_eligible {
            let lookup =
                controlled_prefix_lookup(shared_prefixes, &compatibility, &prompt_tokens, request);
            prefix_state = lookup.state;
            prefix_cache_refusal_reason = lookup.refusal_reason;
            prefix_cache_invalidation_trigger = lookup.invalidation_trigger;
            prefix_tokens_reused = lookup.reused_tokens;
            prefix_identity = lookup.identity;
            prompt_logits = lookup.prompt_logits;
            last_logits = if lookup.last_logits.is_empty() {
                prompt_logits.last().cloned().unwrap_or_default()
            } else {
                lookup.last_logits
            };
            lookup.cache.unwrap_or_else(|| {
                InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            })
        } else if let Some(session_id) = &request.session_id {
            sessions.state(session_id)?.cache().clone()
        } else {
            InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        };
        if cache.width() != expected_kv_width {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width,
                kv_width: cache.width(),
            });
        }
        cache.bind_owner(request_kv_owner(
            request,
            psionic_runtime::BatchExecutionPosture::SingleRequestOnly,
            None,
        ));
        let request_kv_checkpoint = cache.checkpoint();
        for token in &prompt_tokens.as_slice()[prefix_tokens_reused..] {
            let step = execute_generation_step_for_request(
                &loaded_model,
                backend,
                request,
                *token,
                cache.len(),
                &cache,
            )?;
            accumulate_generation_step_counters(
                &step,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut kernel_count,
                &mut bytes_moved,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
                &mut gpt_oss_perf,
            );
            cache.append(*token, step.key, step.value)?;
            last_logits = step.logits;
            prompt_logits.push(last_logits.clone());
        }
        let prefill_handoff_state = cache.state();
        let prompt_cache = (shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len())
            .then(|| cache.clone());

        let mut sampler = GenerationSampler::new(&request.options)?;
        let structured_output_report = sampler.structured_output_report();
        let mut generated_tokens = Vec::new();
        let mut first_token_emitted_at = None;
        let mut last_token_emitted_at = None;
        let termination = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break TerminationReason::MaxOutputTokens;
            }
            if cache.len() >= cache.max_context() {
                break TerminationReason::ContextLimit;
            }

            let next_token = match sampler.select_next_token(
                loaded_model.tokenizer(),
                &last_logits,
                &cache,
                generated_tokens.as_slice(),
            )? {
                GenerationSelection::Token(token) => token,
                GenerationSelection::Terminate => break TerminationReason::EndOfSequence,
            };
            if loaded_model.is_end_of_sequence(next_token) {
                break TerminationReason::EndOfSequence;
            }

            generated_tokens.push(next_token);
            let step = execute_generation_step_for_request(
                &loaded_model,
                backend,
                request,
                next_token,
                cache.len(),
                &cache,
            )?;
            accumulate_generation_step_counters(
                &step,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut kernel_count,
                &mut bytes_moved,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
                &mut gpt_oss_perf,
            );
            cache.append(next_token, step.key, step.value)?;
            last_logits = step.logits;
            let emitted_at = Instant::now();
            if first_token_emitted_at.is_none() {
                first_token_emitted_at = Some(emitted_at);
            }
            last_token_emitted_at = Some(emitted_at);

            if truncate_generated_text(
                loaded_model.tokenizer(),
                &mut generated_tokens,
                &request.options.stop_sequences,
            )
            .is_some()
            {
                break TerminationReason::EndOfSequence;
            }
        };

        if shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len() {
            if let Some(prompt_cache) = prompt_cache.as_ref() {
                let recorded_identity = shared_prefixes.record(
                    compatibility,
                    &prompt_tokens,
                    &prompt_logits,
                    prompt_cache,
                );
                if prefix_state != PrefixCacheState::Hit || prefix_identity.is_none() {
                    prefix_identity = Some(recorded_identity);
                }
            }
        }

        let prompt_eval_duration_ns = elapsed_ns(prompt_eval_start);

        let generated = TokenSequence::new(generated_tokens);
        if let Some(session_id) = &request.session_id {
            session_tokens.extend_from_slice(prompt_tokens.as_slice());
            session_tokens.extend_from_slice(generated.as_slice());
            sessions.replace_cache(
                session_id,
                loaded_model.descriptor(),
                served_artifact.served_artifact_digest.as_str(),
                cache.clone(),
                TokenSequence::new(session_tokens),
            )?;
        }
        let text = loaded_model.tokenizer().decode(generated.as_slice());
        let usage = GenerationUsage {
            input_tokens: prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: cache.len(),
        };
        let kv_cache = KvCacheAccounting::from_states(&previous_kv_state, cache.state());
        let total_duration_ns = elapsed_ns(generation_start);
        let time_to_first_token_ns = first_token_emitted_at
            .map(|first_token_at| first_token_at.duration_since(generation_start))
            .and_then(|duration| duration.as_nanos().try_into().ok());
        let inter_token_latency_ns = average_inter_token_latency_ns(
            first_token_emitted_at,
            last_token_emitted_at,
            usage.output_tokens,
        );
        let prefill_decode_handoff = local_prefill_decode_handoff(&prefill_handoff_state);
        let kv_residency = host_only_kv_residency(cache.policy(), cache.state());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match load_state {
                GenerationLoadState::Cold => loaded_model.load_duration_ns(),
                GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(total_duration_ns.saturating_sub(prompt_eval_duration_ns)),
            time_to_first_token_ns,
            inter_token_latency_ns,
            kv_cache: Some(kv_cache),
            kv_residency: kv_residency.clone(),
            prefix_tokens_reused: Some(prefix_tokens_reused),
            gpt_oss_perf: gpt_oss_perf.filter(|perf| !perf.is_zero()),
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| loaded_model.plan_digest().to_string());
        let provenance = GenerationProvenance {
            served_artifact,
            adapter_serving: request.adapter_serving.clone(),
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            load_state,
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan,
            residency_policy,
            residency_snapshot,
            kv_cache_policy: Some(cache.policy().clone()),
            kv_ownership: cache.ownership_since(&request_kv_checkpoint),
            prefix_cache_control: Some(request.prefix_cache_control.clone()),
            prefix_cache_state: Some(prefix_state),
            prefix_cache_refusal_reason,
            prefix_cache_policy: Some(prefix_policy),
            prefix_cache_identity: prefix_identity,
            compile_path: compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
                prefill_decode_handoff,
                kv_residency,
            ),
            cache_observations: generation_cache_observations(
                loaded_model.descriptor(),
                compile_path.as_ref(),
                load_state,
                request.session_id.as_ref(),
                request.reset_session,
                &previous_kv_state,
                prefix_state,
                prefix_cache_invalidation_trigger,
            ),
            scheduler: None,
            structured_output: structured_output_report,
        };
        let structured_output_value = sampler.structured_output_value(text.as_str())?;
        let response = GenerationResponse::new(
            request,
            request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance);
        Ok(if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        })
    })();

    let _ = models.finish_request(model_id, current_time_millis());
    result
}

fn truncate_generated_text(
    tokenizer: &dyn TokenizerBoundary,
    generated_tokens: &mut Vec<TokenId>,
    stop_sequences: &[String],
) -> Option<String> {
    if stop_sequences.is_empty() {
        return None;
    }

    let text = tokenizer.decode(generated_tokens);
    let stop_index = stop_sequences
        .iter()
        .filter(|stop| !stop.is_empty())
        .filter_map(|stop| text.find(stop))
        .min()?;
    let truncated = text[..stop_index].trim_end().to_string();
    *generated_tokens = tokenizer.encode(truncated.as_str()).as_slice().to_vec();
    Some(truncated)
}

fn text_prefix_without_trailing_chars(text: &str, trailing_chars: usize) -> &str {
    if trailing_chars == 0 {
        return text;
    }
    let char_count = text.chars().count();
    if trailing_chars >= char_count {
        return "";
    }
    let keep_chars = char_count - trailing_chars;
    text.char_indices()
        .nth(keep_chars)
        .map_or(text, |(index, _)| &text[..index])
}

fn token_count_for_decoded_prefix(
    tokenizer: &dyn TokenizerBoundary,
    tokens: &[TokenId],
    prefix_text: &str,
) -> usize {
    let mut safe_count = 0;
    for count in 1..=tokens.len() {
        let decoded = tokenizer.decode(&tokens[..count]);
        if prefix_text.starts_with(decoded.as_str()) {
            safe_count = count;
            continue;
        }
        break;
    }
    safe_count
}

fn build_generation_graph<M>(
    model: &M,
) -> Result<(Graph, TensorId, TensorId, TensorId, TensorId, TensorId), GraphError>
where
    M: WordDecoderExecutionModel,
{
    build_generation_graph_for_device(Device::cpu(), model)
}

fn build_generation_graph_for_device<M>(
    device: Device,
    model: &M,
) -> Result<(Graph, TensorId, TensorId, TensorId, TensorId, TensorId), GraphError>
where
    M: WordDecoderExecutionModel,
{
    let descriptor = model.descriptor();
    let config = &descriptor.config;
    let weights = model.weights();

    let mut builder = GraphBuilder::new(device);
    let token_input = builder.input(
        "token_one_hot",
        Shape::new(vec![1, config.vocab_size]),
        DType::F32,
    );
    let position_input = builder.input(
        "position_one_hot",
        Shape::new(vec![1, config.max_context]),
        DType::F32,
    );
    let context_input = builder.input(
        "context",
        Shape::new(vec![1, config.hidden_size]),
        DType::F32,
    );
    let token_embedding = builder.constant_f32(
        Shape::new(vec![config.vocab_size, config.hidden_size]),
        weights.token_embedding().to_vec(),
    )?;
    let position_embedding = builder.constant_f32(
        Shape::new(vec![config.max_context, config.hidden_size]),
        weights.position_embedding().to_vec(),
    )?;
    let context_projection = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.hidden_size]),
        weights.context_projection().to_vec(),
    )?;
    let lm_head = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.vocab_size]),
        weights.lm_head().to_vec(),
    )?;
    let lm_bias = builder.constant_f32(
        Shape::new(vec![1, config.vocab_size]),
        weights.lm_bias().to_vec(),
    )?;

    let token_hidden = builder.matmul(&token_input, &token_embedding)?;
    let position_hidden = builder.matmul(&position_input, &position_embedding)?;
    let context_hidden = builder.matmul(&context_input, &context_projection)?;
    let hidden = builder.add(&token_hidden, &position_hidden)?;
    let hidden = builder.add(&hidden, &context_hidden)?;
    let logits = builder.matmul(&hidden, &lm_head)?;
    let logits = builder.add(&logits, &lm_bias)?;

    Ok((
        builder.finish(vec![hidden.clone(), logits.clone()]),
        token_input.id(),
        position_input.id(),
        context_input.id(),
        hidden.id(),
        logits.id(),
    ))
}

fn validate_generation_step_request<M>(
    model: &M,
    token: TokenId,
    position: usize,
) -> Result<(), ReferenceTextGenerationError>
where
    M: WordDecoderExecutionModel,
{
    let config = &model.descriptor().config;
    if token.as_u32() as usize >= config.vocab_size {
        return Err(ReferenceTextGenerationError::InvalidToken {
            token: token.as_u32(),
            vocab_size: config.vocab_size,
        });
    }
    if position >= config.max_context {
        return Err(ReferenceTextGenerationError::InvalidPosition {
            position,
            max_context: config.max_context,
        });
    }
    Ok(())
}

fn execute_cpu_generation_graph(
    backend: &mut CpuBackend,
    graph: &Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
    config: &DecoderConfig,
    token: TokenId,
    position: usize,
    context: &[f32],
) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        token_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.vocab_size]),
            one_hot(config.vocab_size, token.as_u32() as usize),
        )?,
    );
    runtime_inputs.insert(
        position_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.max_context]),
            one_hot(config.max_context, position),
        )?,
    );
    runtime_inputs.insert(
        context_input_id,
        backend.input_buffer(Shape::new(vec![1, config.hidden_size]), context.to_vec())?,
    );

    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let hidden = result
        .outputs
        .get(&hidden_output_id)
        .ok_or(ReferenceTextGenerationError::MissingOutput("hidden"))?
        .as_f32_slice()
        .ok_or(ReferenceTextGenerationError::MissingOutput("hidden_dense"))?
        .to_vec();
    let logits = result
        .outputs
        .get(&logits_output_id)
        .ok_or(ReferenceTextGenerationError::MissingOutput("logits"))?
        .as_f32_slice()
        .ok_or(ReferenceTextGenerationError::MissingOutput("logits_dense"))?
        .to_vec();
    Ok(GenerationStepOutput {
        key: hidden.clone(),
        value: hidden,
        logits,
        hidden: None,
        execution_plan_digest: result.metrics.execution_plan_digest.clone(),
        compile_path: result.metrics.compile_path.clone(),
        kernel_count: result.metrics.kernel_count,
        bytes_moved: result.metrics.bytes_moved,
        plan_cache_hits: result.metrics.plan_cache_hits,
        plan_cache_misses: result.metrics.plan_cache_misses,
        gpt_oss_perf: None,
    })
}

fn execute_metal_generation_graph(
    backend: &mut MetalBackend,
    graph: &Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
    config: &DecoderConfig,
    token: TokenId,
    position: usize,
    context: &[f32],
) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        token_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.vocab_size]),
            one_hot(config.vocab_size, token.as_u32() as usize),
        )?,
    );
    runtime_inputs.insert(
        position_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.max_context]),
            one_hot(config.max_context, position),
        )?,
    );
    runtime_inputs.insert(
        context_input_id,
        backend.input_buffer(Shape::new(vec![1, config.hidden_size]), context.to_vec())?,
    );

    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let hidden = result
        .outputs
        .get(&hidden_output_id)
        .ok_or(ReferenceTextGenerationError::MissingOutput("hidden"))?
        .read_f32()
        .map_err(ReferenceTextGenerationError::Runtime)?;
    let logits = result
        .outputs
        .get(&logits_output_id)
        .ok_or(ReferenceTextGenerationError::MissingOutput("logits"))?
        .read_f32()
        .map_err(ReferenceTextGenerationError::Runtime)?;
    Ok(GenerationStepOutput {
        key: hidden.clone(),
        value: hidden,
        logits,
        hidden: None,
        execution_plan_digest: result.metrics.execution_plan_digest.clone(),
        compile_path: result.metrics.compile_path.clone(),
        kernel_count: result.metrics.kernel_count,
        bytes_moved: result.metrics.bytes_moved,
        plan_cache_hits: result.metrics.plan_cache_hits,
        plan_cache_misses: result.metrics.plan_cache_misses,
        gpt_oss_perf: None,
    })
}

fn one_hot(width: usize, index: usize) -> Vec<f32> {
    let mut output = vec![0.0; width];
    output[index] = 1.0;
    output
}

fn mean_cache_value(cache: &InMemoryKvCache, width: usize) -> Vec<f32> {
    if cache.is_empty() {
        return vec![0.0; width];
    }

    let mut output = vec![0.0; width];
    for entry in cache.entries() {
        for (accumulator, value) in output.iter_mut().zip(entry.value.iter()) {
            *accumulator += *value;
        }
    }
    let scale = 1.0 / (cache.len() as f32);
    for value in &mut output {
        *value *= scale;
    }
    output
}

/// Smoke embeddings execution error.
#[derive(Debug, Error)]
pub enum SmokeEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The model produced an invalid output vector.
    #[error("invalid embedding output for input {index}: {message}")]
    InvalidOutput {
        /// Input index in the request batch.
        index: usize,
        /// Plain-text failure summary.
        message: String,
    },
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl SmokeEmbeddingsError {
    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        match self {
            Self::UnsupportedProduct(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedProduct,
                400,
                self.to_string(),
            )
            .with_backend("cpu"),
            Self::UnsupportedModel(model_id) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ModelNotFound,
                404,
                self.to_string(),
            )
            .with_model_id(model_id.clone())
            .with_backend("cpu"),
            Self::InvalidOutput { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidOutput,
                500,
                self.to_string(),
            )
            .with_backend("cpu"),
            Self::Graph(_) => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
                    .with_backend("cpu")
            }
            Self::Runtime(error) => runtime_error_diagnostic(error).with_backend("cpu"),
        }
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &EmbeddingRequest) -> LocalRuntimeDiagnostic {
        diagnostic_with_request_context(
            self.diagnostic(),
            &request.product_id,
            &request.model.model.model_id,
        )
    }
}

/// Model-backed embeddings execution error.
#[derive(Debug, Error)]
pub enum ModelEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The model produced an invalid output vector.
    #[error("invalid embedding output for input {index}: {message}")]
    InvalidOutput {
        /// Input index in the request batch.
        index: usize,
        /// Plain-text failure summary.
        message: String,
    },
    /// Loading or validating the model failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl ModelEmbeddingsError {
    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        match self {
            Self::UnsupportedProduct(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedProduct,
                400,
                self.to_string(),
            )
            .with_backend("cpu"),
            Self::UnsupportedModel(model_id) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ModelNotFound,
                404,
                self.to_string(),
            )
            .with_model_id(model_id.clone())
            .with_backend("cpu"),
            Self::InvalidOutput { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidOutput,
                500,
                self.to_string(),
            )
            .with_backend("cpu"),
            Self::Model(error) => model_load_error_diagnostic(error).with_backend("cpu"),
            Self::Graph(_) => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
                    .with_backend("cpu")
            }
            Self::Runtime(error) => runtime_error_diagnostic(error).with_backend("cpu"),
        }
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &EmbeddingRequest) -> LocalRuntimeDiagnostic {
        diagnostic_with_request_context(
            self.diagnostic(),
            &request.product_id,
            &request.model.model.model_id,
        )
    }
}

/// Metal-backed embeddings execution error.
#[derive(Debug, Error)]
pub enum MetalEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The model produced an invalid output vector.
    #[error("invalid embedding output for input {index}: {message}")]
    InvalidOutput {
        /// Input index in the request batch.
        index: usize,
        /// Plain-text failure summary.
        message: String,
    },
    /// Metal is not available for the requested product path on this machine.
    #[error("metal backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        /// Honest backend status.
        status: HealthStatus,
        /// Plain-text reason.
        message: String,
    },
    /// Loading or validating the model failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Metal runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl MetalEmbeddingsError {
    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        match self {
            Self::UnsupportedProduct(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedProduct,
                400,
                self.to_string(),
            )
            .with_backend("metal"),
            Self::UnsupportedModel(model_id) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ModelNotFound,
                404,
                self.to_string(),
            )
            .with_model_id(model_id.clone())
            .with_backend("metal"),
            Self::InvalidOutput { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidOutput,
                500,
                self.to_string(),
            )
            .with_backend("metal"),
            Self::BackendUnavailable { status, .. } => LocalRuntimeDiagnostic::new(
                if *status == HealthStatus::Degraded {
                    LocalRuntimeErrorCode::BackendDegraded
                } else {
                    LocalRuntimeErrorCode::BackendUnavailable
                },
                503,
                self.to_string(),
            )
            .with_backend("metal")
            .with_backend_health(*status),
            Self::Model(error) => model_load_error_diagnostic(error).with_backend("metal"),
            Self::Graph(_) => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
                    .with_backend("metal")
            }
            Self::Runtime(error) => runtime_error_diagnostic(error).with_backend("metal"),
        }
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &EmbeddingRequest) -> LocalRuntimeDiagnostic {
        diagnostic_with_request_context(
            self.diagnostic(),
            &request.product_id,
            &request.model.model.model_id,
        )
    }
}

/// CUDA-backed embeddings execution error.
#[derive(Debug, Error)]
pub enum CudaEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The model produced an invalid output vector.
    #[error("invalid embedding output for input {index}: {message}")]
    InvalidOutput {
        /// Input index in the request batch.
        index: usize,
        /// Plain-text failure summary.
        message: String,
    },
    /// CUDA is not available for the requested product path on this machine.
    #[error("cuda backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        /// Honest backend status.
        status: HealthStatus,
        /// Plain-text reason.
        message: String,
    },
    /// Loading or validating the model failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CUDA runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl CudaEmbeddingsError {
    /// Returns the backend-neutral diagnostic for the error.
    #[must_use]
    pub fn diagnostic(&self) -> LocalRuntimeDiagnostic {
        match self {
            Self::UnsupportedProduct(_) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::UnsupportedProduct,
                400,
                self.to_string(),
            )
            .with_backend("cuda"),
            Self::UnsupportedModel(model_id) => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::ModelNotFound,
                404,
                self.to_string(),
            )
            .with_model_id(model_id.clone())
            .with_backend("cuda"),
            Self::InvalidOutput { .. } => LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::InvalidOutput,
                500,
                self.to_string(),
            )
            .with_backend("cuda"),
            Self::BackendUnavailable { status, .. } => LocalRuntimeDiagnostic::new(
                if *status == HealthStatus::Degraded {
                    LocalRuntimeErrorCode::BackendDegraded
                } else {
                    LocalRuntimeErrorCode::BackendUnavailable
                },
                503,
                self.to_string(),
            )
            .with_backend("cuda")
            .with_backend_health(*status),
            Self::Model(error) => model_load_error_diagnostic(error).with_backend("cuda"),
            Self::Graph(_) => {
                LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, self.to_string())
                    .with_backend("cuda")
            }
            Self::Runtime(error) => runtime_error_diagnostic(error).with_backend("cuda"),
        }
    }

    /// Returns the diagnostic annotated with request context.
    #[must_use]
    pub fn diagnostic_for_request(&self, request: &EmbeddingRequest) -> LocalRuntimeDiagnostic {
        diagnostic_with_request_context(
            self.diagnostic(),
            &request.product_id,
            &request.model.model.model_id,
        )
    }
}

/// CPU-backed embeddings smoke service.
#[derive(Clone, Debug)]
pub struct SmokeEmbeddingsService {
    backend: CpuBackend,
    model: SmokeByteEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
    plan_digest: String,
}

impl SmokeEmbeddingsService {
    /// Creates a new smoke embeddings service.
    pub fn new() -> Result<Self, SmokeEmbeddingsError> {
        let model = SmokeByteEmbedder::new();
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            Device::cpu(),
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.projection(),
            model.bias(),
            input_shape.clone(),
        )?;
        let plan_digest = compile_graph(&graph)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?
            .stable_digest();
        Ok(Self {
            backend: CpuBackend::new(),
            model,
            graph,
            input_shape,
            input_id,
            output_id,
            plan_digest,
        })
    }

    /// Returns the smoke model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    fn embed_one(&mut self, input: &str) -> Result<EmbeddingStepOutput, SmokeEmbeddingsError> {
        execute_cpu_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )
        .map_err(SmokeEmbeddingsError::Runtime)
    }
}

impl EmbeddingsExecutor for SmokeEmbeddingsService {
    type Error = SmokeEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        let embed_start = Instant::now();
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(SmokeEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model.model.model_id != self.model.descriptor().model.model_id {
            return Err(SmokeEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Ok(EmbeddingResponse::new(request, Vec::new()).with_metrics(
                EmbeddingMetrics {
                    total_duration_ns: Some(0),
                    load_duration_ns: Some(0),
                    prompt_eval_count: None,
                    prompt_eval_duration_ns: None,
                },
            ));
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        let mut execution_plan_digest = None;
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        for (index, input) in request.inputs.iter().enumerate() {
            let step = self.embed_one(input)?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = step.execution_plan_digest.clone();
            }
            if compile_path.is_none() {
                compile_path = step.compile_path.clone();
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
            plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
            let values = finalize_embedding_values(
                step.values,
                request.model.normalization,
                request.output_dimensions,
            )
            .map_err(|message| SmokeEmbeddingsError::InvalidOutput { index, message })?;
            embeddings.push(EmbeddingVector { index, values });
        }

        let metrics = EmbeddingMetrics {
            total_duration_ns: Some(
                embed_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(0),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| self.plan_digest.clone());
        let provenance = EmbeddingProvenance {
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            compile_path: compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                None,
                None,
                None,
            ),
            cache_observations: cache_observations_for_embedding_model(
                self.model.descriptor(),
                compile_path.as_ref(),
            ),
        };
        Ok(EmbeddingResponse::new(request, embeddings)
            .with_metrics_and_provenance(metrics, provenance))
    }
}

/// CPU-backed model embeddings service for artifact-loaded embedding families.
#[derive(Clone, Debug)]
pub struct CpuModelEmbeddingsService {
    backend: CpuBackend,
    model: ByteProjectionEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
    plan_digest: String,
}

impl CpuModelEmbeddingsService {
    /// Loads the first model-backed embeddings family from a local safetensors artifact.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, ModelEmbeddingsError> {
        let model = ByteProjectionEmbedder::from_safetensors_artifact(path)?;
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            Device::cpu(),
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.weights().projection(),
            model.weights().bias(),
            input_shape.clone(),
        )?;
        let plan_digest = compile_graph(&graph)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?
            .stable_digest();
        Ok(Self {
            backend: CpuBackend::new(),
            model,
            graph,
            input_shape,
            input_id,
            output_id,
            plan_digest,
        })
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    fn embed_one(&mut self, input: &str) -> Result<EmbeddingStepOutput, ModelEmbeddingsError> {
        execute_cpu_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )
        .map_err(ModelEmbeddingsError::Runtime)
    }
}

impl EmbeddingsExecutor for CpuModelEmbeddingsService {
    type Error = ModelEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        let embed_start = Instant::now();
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(ModelEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != *self.model.descriptor() {
            return Err(ModelEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Ok(EmbeddingResponse::new(request, Vec::new()).with_metrics(
                EmbeddingMetrics {
                    total_duration_ns: Some(0),
                    load_duration_ns: Some(0),
                    prompt_eval_count: None,
                    prompt_eval_duration_ns: None,
                },
            ));
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        let mut execution_plan_digest = None;
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        for (index, input) in request.inputs.iter().enumerate() {
            let step = self.embed_one(input)?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = step.execution_plan_digest.clone();
            }
            if compile_path.is_none() {
                compile_path = step.compile_path.clone();
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
            plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
            let values = finalize_embedding_values(
                step.values,
                request.model.normalization,
                request.output_dimensions,
            )
            .map_err(|message| ModelEmbeddingsError::InvalidOutput { index, message })?;
            embeddings.push(EmbeddingVector { index, values });
        }

        let metrics = EmbeddingMetrics {
            total_duration_ns: Some(
                embed_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(0),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| self.plan_digest.clone());
        let provenance = EmbeddingProvenance {
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            compile_path: compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                None,
                None,
                None,
            ),
            cache_observations: cache_observations_for_embedding_model(
                self.model.descriptor(),
                compile_path.as_ref(),
            ),
        };
        Ok(EmbeddingResponse::new(request, embeddings)
            .with_metrics_and_provenance(metrics, provenance))
    }
}

/// Honest CPU product alias for model-backed embeddings.
pub type CpuProductEmbeddingsService = CpuModelEmbeddingsService;

/// Metal-backed embeddings service for the supported model-backed product path.
pub struct MetalModelEmbeddingsService {
    backend: MetalBackend,
    backend_selection: BackendSelection,
    model: ByteProjectionEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
    plan_digest: String,
}

impl MetalModelEmbeddingsService {
    /// Loads the first model-backed embeddings family on Metal when the local
    /// machine exposes a genuinely supported Metal execution device.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, MetalEmbeddingsError> {
        let backend = MetalBackend::new();
        let backend_selection = backend
            .backend_selection(EMBEDDINGS_SUPPORTED_OPS)
            .map_err(|error| MetalEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let selected_device = backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.device.clone())
            .ok_or_else(|| MetalEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: String::from("metal backend selected no execution device"),
            })?;

        let model = ByteProjectionEmbedder::from_safetensors_artifact(path)?;
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            selected_device,
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.weights().projection(),
            model.weights().bias(),
            input_shape.clone(),
        )?;
        let plan_digest = compile_graph(&graph)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?
            .stable_digest();
        Ok(Self {
            backend,
            backend_selection,
            model,
            graph,
            input_shape,
            input_id,
            output_id,
            plan_digest,
        })
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    /// Returns truthful backend-selection data for the loaded Metal product.
    #[must_use]
    pub fn backend_selection(&self) -> &BackendSelection {
        &self.backend_selection
    }

    fn embed_one(&mut self, input: &str) -> Result<EmbeddingStepOutput, MetalEmbeddingsError> {
        execute_metal_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )
        .map_err(MetalEmbeddingsError::Runtime)
    }
}

impl EmbeddingsExecutor for MetalModelEmbeddingsService {
    type Error = MetalEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        let embed_start = Instant::now();
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(MetalEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != *self.model.descriptor() {
            return Err(MetalEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Ok(EmbeddingResponse::new(request, Vec::new()).with_metrics(
                EmbeddingMetrics {
                    total_duration_ns: Some(0),
                    load_duration_ns: Some(0),
                    prompt_eval_count: None,
                    prompt_eval_duration_ns: None,
                },
            ));
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        let mut execution_plan_digest = None;
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        for (index, input) in request.inputs.iter().enumerate() {
            let step = self.embed_one(input)?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = step.execution_plan_digest.clone();
            }
            if compile_path.is_none() {
                compile_path = step.compile_path.clone();
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
            plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
            let values = finalize_embedding_values(
                step.values,
                request.model.normalization,
                request.output_dimensions,
            )
            .map_err(|message| MetalEmbeddingsError::InvalidOutput { index, message })?;
            embeddings.push(EmbeddingVector { index, values });
        }

        let metrics = EmbeddingMetrics {
            total_duration_ns: Some(
                embed_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(0),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| self.plan_digest.clone());
        let provenance = EmbeddingProvenance {
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            compile_path: compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                None,
                None,
                None,
            ),
            cache_observations: cache_observations_for_embedding_model(
                self.model.descriptor(),
                compile_path.as_ref(),
            ),
        };
        Ok(EmbeddingResponse::new(request, embeddings)
            .with_metrics_and_provenance(metrics, provenance))
    }
}

/// Honest Metal product alias for model-backed embeddings.
pub type MetalProductEmbeddingsService = MetalModelEmbeddingsService;

/// CUDA-backed embeddings service for the supported model-backed product path.
pub struct CudaModelEmbeddingsService {
    backend: CudaBackend,
    backend_selection: BackendSelection,
    model: ByteProjectionEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
    plan_digest: String,
}

impl CudaModelEmbeddingsService {
    /// Loads the first model-backed embeddings family on CUDA when the local
    /// machine exposes a genuinely supported CUDA execution device.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, CudaEmbeddingsError> {
        let backend = CudaBackend::new();
        let backend_selection = backend
            .backend_selection(CUDA_EMBEDDINGS_SUPPORTED_OPS)
            .map_err(|error| CudaEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let selected_device = backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.device.clone())
            .ok_or_else(|| CudaEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: String::from("cuda backend selected no execution device"),
            })?;

        let model = ByteProjectionEmbedder::from_safetensors_artifact(path)?;
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            selected_device,
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.weights().projection(),
            model.weights().bias(),
            input_shape.clone(),
        )?;
        let plan_digest = compile_graph(&graph)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?
            .stable_digest();
        Ok(Self {
            backend,
            backend_selection,
            model,
            graph,
            input_shape,
            input_id,
            output_id,
            plan_digest,
        })
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    /// Returns truthful backend-selection data for the loaded CUDA product.
    #[must_use]
    pub fn backend_selection(&self) -> &BackendSelection {
        &self.backend_selection
    }

    fn embed_one(&mut self, input: &str) -> Result<EmbeddingStepOutput, CudaEmbeddingsError> {
        execute_cuda_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )
        .map_err(CudaEmbeddingsError::Runtime)
    }
}

impl EmbeddingsExecutor for CudaModelEmbeddingsService {
    type Error = CudaEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        let embed_start = Instant::now();
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(CudaEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != *self.model.descriptor() {
            return Err(CudaEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Ok(EmbeddingResponse::new(request, Vec::new()).with_metrics(
                EmbeddingMetrics {
                    total_duration_ns: Some(0),
                    load_duration_ns: Some(0),
                    prompt_eval_count: None,
                    prompt_eval_duration_ns: None,
                },
            ));
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        let mut execution_plan_digest = None;
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        for (index, input) in request.inputs.iter().enumerate() {
            let step = self.embed_one(input)?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = step.execution_plan_digest.clone();
            }
            if compile_path.is_none() {
                compile_path = step.compile_path.clone();
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            plan_cache_hits = plan_cache_hits.saturating_add(step.plan_cache_hits);
            plan_cache_misses = plan_cache_misses.saturating_add(step.plan_cache_misses);
            let values = finalize_embedding_values(
                step.values,
                request.model.normalization,
                request.output_dimensions,
            )
            .map_err(|message| CudaEmbeddingsError::InvalidOutput { index, message })?;
            embeddings.push(EmbeddingVector { index, values });
        }

        let metrics = EmbeddingMetrics {
            total_duration_ns: Some(
                embed_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(0),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| self.plan_digest.clone());
        let provenance = EmbeddingProvenance {
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            compile_path: compile_path.clone(),
            delivery_proof: build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                None,
                None,
                None,
            ),
            cache_observations: cache_observations_for_embedding_model(
                self.model.descriptor(),
                compile_path.as_ref(),
            ),
        };
        Ok(EmbeddingResponse::new(request, embeddings)
            .with_metrics_and_provenance(metrics, provenance))
    }
}

/// Honest CUDA product alias for model-backed embeddings.
pub type CudaProductEmbeddingsService = CudaModelEmbeddingsService;

fn build_embedding_graph(
    device: Device,
    input_dimensions: usize,
    output_dimensions: usize,
    projection: &[f32],
    bias: &[f32],
    input_shape: Shape,
) -> Result<(Graph, TensorId, TensorId), GraphError> {
    let mut builder = GraphBuilder::new(device);
    let input = builder.input("features", input_shape, DType::F32);
    let weights = builder.constant_f32(
        Shape::new(vec![input_dimensions, output_dimensions]),
        projection.to_vec(),
    )?;
    let bias = builder.constant_f32(Shape::new(vec![1, output_dimensions]), bias.to_vec())?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    let output_id = shifted.id();
    let input_id = input.id();
    let graph = builder.finish(vec![shifted]);
    Ok((graph, input_id, output_id))
}

fn execute_cpu_embedding_graph(
    backend: &mut CpuBackend,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input_shape: &Shape,
    features: Vec<f32>,
) -> Result<EmbeddingStepOutput, RuntimeError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        input_id,
        backend.input_buffer(input_shape.clone(), features)?,
    );
    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let Some(output) = result.outputs.get(&output_id) else {
        return Err(RuntimeError::Backend(String::from(
            "missing embedding output",
        )));
    };
    Ok(EmbeddingStepOutput {
        values: output
            .as_f32_slice()
            .ok_or_else(|| {
                RuntimeError::Backend(String::from("embedding output must be dense f32"))
            })?
            .to_vec(),
        execution_plan_digest: result.metrics.execution_plan_digest.clone(),
        compile_path: result.metrics.compile_path.clone(),
        kernel_count: result.metrics.kernel_count,
        bytes_moved: result.metrics.bytes_moved,
        plan_cache_hits: result.metrics.plan_cache_hits,
        plan_cache_misses: result.metrics.plan_cache_misses,
    })
}

fn execute_metal_embedding_graph(
    backend: &mut MetalBackend,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input_shape: &Shape,
    features: Vec<f32>,
) -> Result<EmbeddingStepOutput, RuntimeError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        input_id,
        backend.input_buffer(input_shape.clone(), features)?,
    );
    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let Some(output) = result.outputs.get(&output_id) else {
        return Err(RuntimeError::Backend(String::from(
            "missing embedding output",
        )));
    };
    Ok(EmbeddingStepOutput {
        values: output.read_f32()?,
        execution_plan_digest: result.metrics.execution_plan_digest.clone(),
        compile_path: result.metrics.compile_path.clone(),
        kernel_count: result.metrics.kernel_count,
        bytes_moved: result.metrics.bytes_moved,
        plan_cache_hits: result.metrics.plan_cache_hits,
        plan_cache_misses: result.metrics.plan_cache_misses,
    })
}

fn execute_cuda_embedding_graph(
    backend: &mut CudaBackend,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input_shape: &Shape,
    features: Vec<f32>,
) -> Result<EmbeddingStepOutput, RuntimeError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        input_id,
        backend.input_buffer(input_shape.clone(), features)?,
    );
    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let Some(output) = result.outputs.get(&output_id) else {
        return Err(RuntimeError::Backend(String::from(
            "missing embedding output",
        )));
    };
    Ok(EmbeddingStepOutput {
        values: output.read_f32()?,
        execution_plan_digest: result.metrics.execution_plan_digest.clone(),
        compile_path: result.metrics.compile_path.clone(),
        kernel_count: result.metrics.kernel_count,
        bytes_moved: result.metrics.bytes_moved,
        plan_cache_hits: result.metrics.plan_cache_hits,
        plan_cache_misses: result.metrics.plan_cache_misses,
    })
}

fn canonical_embedding_output_dimensions(
    requested_output_dimensions: Option<usize>,
    model_dimensions: usize,
) -> Option<usize> {
    requested_output_dimensions
        .filter(|dimensions| *dimensions > 0 && *dimensions < model_dimensions)
}

fn finalize_embedding_values(
    mut values: Vec<f32>,
    normalization: EmbeddingNormalization,
    requested_output_dimensions: Option<usize>,
) -> Result<Vec<f32>, String> {
    values = normalize_embedding(values, normalization)?;
    if let Some(output_dimensions) =
        canonical_embedding_output_dimensions(requested_output_dimensions, values.len())
    {
        values.truncate(output_dimensions);
        values = normalize_embedding(values, normalization)?;
    }

    Ok(values)
}

fn normalize_embedding(
    mut values: Vec<f32>,
    normalization: EmbeddingNormalization,
) -> Result<Vec<f32>, String> {
    for value in &values {
        if !value.is_finite() {
            return Err(String::from("embedding contains NaN or Inf values"));
        }
    }

    if normalization != EmbeddingNormalization::UnitLength {
        return Ok(values);
    }

    let sum = values.iter().map(|value| value * value).sum::<f32>();
    let norm = sum.sqrt().max(1.0e-12);
    for value in &mut values {
        *value /= norm;
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use psionic_backend_cpu::CpuBackend;
    use psionic_core::{DType, Shape};
    use psionic_runtime::{
        AdmissionRefusalReason, BackendSelection, CacheInvalidationTrigger, CacheKind,
        ExecutionPartition, HealthStatus, KvCacheAccounting, KvCacheDeviceScope, KvCacheOwnerClass,
        KvCachePageLayout, KvCachePolicy, KvCacheSpillPolicy, KvCacheState,
        KvResidencyMovementKind, KvResidencyRefusalReason, KvResidencyTier, LoadedModelState,
        LocalRuntimeErrorCode, LocalServingIsolationPolicy, MemoryBudget, ModelResidencyPolicy,
        PrefixCacheControl, PrefixCacheMode, PrefixCacheRefusalReason, PrefixCacheState,
        QuantizationKernelStrategy, ResidencyPressureAction, RuntimeTransitionEvent,
        RuntimeTransitionKind, ShardedModelArtifactRef, ShardedModelLayoutKind,
        ShardedModelManifest,
    };
    use tempfile::tempdir;

    use super::{
        ADAPTER_TEXT_GENERATION_PRODUCT_ID, AdapterArtifactFormat, AdapterArtifactIdentity,
        AdapterArtifactKind, AdapterResidencyMode, AdapterServingBinding, AdapterTargetFamily,
        ContextOverflowPolicy, ContextWindowError, CpuGenerationStream,
        CpuReferenceTextGenerationService, CpuWordGenerationModel, DEFAULT_MODEL_KEEPALIVE_MILLIS,
        EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse, EmbeddingVector,
        EmbeddingsExecutor, FixtureWordTokenizer, GenerationEventStream, GenerationLoadState,
        GenerationModelHandle, GenerationOptions, GenerationRequest, GenerationResponse,
        GenerationStreamEvent, GenerationStreamStatus, InMemoryGenerationModelRegistry,
        InMemoryGenerationSessionStore, InMemoryKvCache, KvCacheError, ListModelsObservation,
        LoadedModelRegistryError, LocalModelCatalog, ModelDescriptor, ModelSummary,
        PsionicLocalRuntime, ReferenceTextGenerationError, ReferenceWordDecoder, SessionId,
        SharedPrefixCompatibility, SharedPrefixStore, ShowObservation, SmokeByteEmbedder,
        SmokeEmbeddingsService, StreamingTextGenerationExecutor, TerminationReason,
        TextGenerationExecutor, TokenId, WeightBundleMetadata, WeightFormat, WeightSource,
        WeightTensorMetadata, WordDecoderExecutionModel, current_time_millis,
        default_generation_streaming_policy, finalize_embedding_values,
        generation_product_supported, load_sharded_model_manifest_json, prefix_compatibility,
        prefix_compatibility_for_request, recommended_generation_quantization_dispatch,
        request_kv_owner, served_artifact_identity_for_decoder_model,
    };
    use crate::{DecoderBlockConfig, DecoderConfig, DecoderModelDescriptor};
    use psionic_models::{
        ActivationFunction, DecoderAttentionConfig, DecoderFeedForwardConfig,
        DecoderFixtureWeights, TokenSequence, TokenizerBoundary, assert_prompt_window_case,
        assert_rendered_prompt_case, golden_prompt_fixture, golden_prompt_fixtures,
        golden_tokenizer_fixture,
    };

    #[test]
    fn generation_quantization_dispatch_prefers_grouped_quantized_path() {
        let decision = recommended_generation_quantization_dispatch(
            super::QuantizationMode::GgmlQ4_0,
            8,
            4096,
            true,
        );

        assert_eq!(decision.strategy, QuantizationKernelStrategy::GroupedBlock);
    }

    #[test]
    fn embedding_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-1",
            sample_embedding_descriptor(),
            vec![String::from("hello world"), String::from("open agents")],
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let expected = r#"{
  "request_id": "req-1",
  "product_id": "psionic.embeddings",
  "model": {
    "model": {
      "model_id": "smoke-byte-embed-v0",
      "family": "smoke",
      "revision": "v0"
    },
    "dimensions": 8,
    "normalization": "None",
    "weights": {
      "format": "ProgrammaticFixture",
      "source": "Fixture",
      "quantization": "none",
      "digest": "30a2fd0264ef45e96101268ae97cfbdffb79540210c88ab834117bc0111c0b00",
      "tensors": [
        {
          "name": "bias",
          "shape": {
            "dims": [
              8
            ]
          },
          "dtype": "F32",
          "quantization": "none"
        },
        {
          "name": "projection",
          "shape": {
            "dims": [
              16,
              8
            ]
          },
          "dtype": "F32",
          "quantization": "none"
        }
      ],
      "artifacts": []
    },
    "artifact_identity": {
      "generation_defaults_digest": "6b25930e91686cee8bb5d4dae8dbed14f63c690c1c97ecb98552d8842e2d9395"
    }
  },
  "inputs": [
    "hello world",
    "open agents"
  ]
}"#;
        assert_eq!(encoded, expected);
        Ok(())
    }

    #[test]
    fn embedding_response_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-2",
            sample_embedding_descriptor(),
            vec![String::from("hi")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.0; 8],
            }],
        );

        let encoded = serde_json::to_string(&response)?;
        let decoded: EmbeddingResponse = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, response);
        assert_eq!(decoded.metadata.input_count, 1);
        assert_eq!(decoded.metadata.model_family, "smoke");
        assert_eq!(decoded.metadata.model_revision, "v0");
        assert_eq!(decoded.metadata.requested_output_dimensions, None);
        assert_eq!(decoded.metrics.total_duration_ns, None);
        assert_eq!(decoded.metrics.load_duration_ns, None);
        Ok(())
    }

    #[test]
    fn smoke_embeddings_service_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = SmokeEmbeddingsService::new()?;
        let request = EmbeddingRequest::new(
            "req-4",
            service.model_descriptor().clone(),
            vec![String::from("hello world")],
        );

        let first = service.embed(&request)?;
        let second = service.embed(&request)?;
        assert_eq!(first.embeddings, second.embeddings);
        assert_eq!(first.metadata, second.metadata);
        assert!(first.metrics.total_duration_ns.is_some());
        assert!(second.metrics.total_duration_ns.is_some());
        assert_eq!(first.metadata.dimensions, 8);
        Ok(())
    }

    #[test]
    fn embedding_output_rejects_non_finite_values() {
        let error = finalize_embedding_values(
            vec![f32::NAN, 1.0],
            EmbeddingNormalization::UnitLength,
            None,
        )
        .expect_err("non-finite embeddings should be rejected");
        assert_eq!(error, "embedding contains NaN or Inf values");
    }

    #[test]
    fn generation_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000001")),
            "hello",
            GenerationOptions::greedy(2),
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let encoded_again = serde_json::to_string_pretty(&request)?;
        assert_eq!(encoded, encoded_again);
        assert!(encoded.contains("\"product_id\": \"psionic.text_generation\""));
        assert!(encoded.contains("\"tokenizer_family\": \"fixture_wordpiece\""));
        assert!(encoded.contains("\"decode_strategy\": \"greedy\""));
        Ok(())
    }

    #[test]
    fn adapter_generation_product_requires_binding() {
        let mut request = GenerationRequest::new_text(
            "gen-adapter-guard",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-adapter-guard")),
            "hello",
            GenerationOptions::greedy(2),
        );

        request.product_id = String::from(ADAPTER_TEXT_GENERATION_PRODUCT_ID);
        assert!(
            !generation_product_supported(&request),
            "adapter product ids should not be accepted without a binding"
        );

        let request = request.with_adapter_serving(sample_adapter_serving_binding());
        assert!(
            generation_product_supported(&request),
            "adapter product ids should become valid once a binding is attached"
        );
    }

    #[test]
    fn generation_request_with_adapter_serving_switches_product_and_preserves_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let binding = sample_adapter_serving_binding();
        let request = GenerationRequest::new_text(
            "gen-adapter-request",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-adapter-request")),
            "hello",
            GenerationOptions::greedy(2),
        )
        .with_adapter_serving(binding.clone());

        assert_eq!(request.product_id, ADAPTER_TEXT_GENERATION_PRODUCT_ID);
        assert_eq!(request.adapter_serving, Some(binding.clone()));

        let encoded = serde_json::to_value(&request)?;
        assert_eq!(
            encoded["adapter_serving"]["served_adapter_digest"],
            serde_json::json!(binding.served_adapter_digest)
        );
        Ok(())
    }

    #[test]
    fn adapter_generation_prefix_compatibility_uses_binding_digest()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = CpuWordGenerationModel::new(ReferenceWordDecoder::new())?;
        let binding = sample_adapter_serving_binding();

        let baseline = prefix_compatibility(&model);
        let request = GenerationRequest::new_text(
            "adapter-prefix",
            model.descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(1),
        )
        .with_adapter_serving(binding.clone());
        let adapter = prefix_compatibility_for_request(&model, &request);

        assert_eq!(
            adapter.served_artifact_digest, binding.served_adapter_digest,
            "shared-prefix identity should key adapter-backed requests by the adapter binding digest"
        );
        assert_ne!(
            baseline.served_artifact_digest,
            adapter.served_artifact_digest
        );
        Ok(())
    }

    #[test]
    fn generation_response_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_tokens(
            "gen-2",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000002")),
            TokenSequence::new(vec![FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID,
            ]),
            "open agents",
            1,
            3,
            TerminationReason::EndOfSequence,
        );

        let encoded = serde_json::to_string(&response)?;
        let decoded: GenerationResponse = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, response);
        assert_eq!(decoded.usage.output_tokens, 2);
        assert_eq!(decoded.metrics.prompt_eval_count, Some(1));
        assert_eq!(decoded.metrics.prompt_eval_duration_ns, None);
        assert_eq!(decoded.metrics.eval_count, Some(2));
        assert_eq!(decoded.metrics.eval_duration_ns, None);
        assert_eq!(decoded.metrics.total_duration_ns, None);
        assert_eq!(decoded.metrics.load_duration_ns, None);
        assert_eq!(decoded.provenance, None);
        Ok(())
    }

    #[test]
    fn generation_sampling_options_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let options = GenerationOptions {
            max_output_tokens: 16,
            context_overflow_policy: ContextOverflowPolicy::TruncateOldest,
            decode_strategy: super::DecodeStrategy::Sample,
            temperature: Some(0.7),
            top_k: Some(32),
            top_p: Some(0.85),
            repeat_penalty: Some(1.2),
            presence_penalty: Some(0.4),
            frequency_penalty: Some(0.3),
            seed: Some(17),
            stop_sequences: vec![String::from("</end>"), String::from("STOP")],
            structured_output: None,
        };

        let encoded = serde_json::to_value(&options)?;
        assert_eq!(encoded["context_overflow_policy"], "truncate_oldest");
        assert_eq!(encoded["decode_strategy"], "sample");
        assert!(
            (encoded["temperature"].as_f64().expect("temperature") - 0.7).abs() < 1e-6,
            "temperature should round-trip as 0.7"
        );
        assert_eq!(encoded["top_k"], 32);
        assert!((encoded["top_p"].as_f64().expect("top_p") - 0.85).abs() < 1e-6);
        assert!((encoded["repeat_penalty"].as_f64().expect("repeat_penalty") - 1.2).abs() < 1e-6);
        assert!(
            (encoded["presence_penalty"]
                .as_f64()
                .expect("presence_penalty")
                - 0.4)
                .abs()
                < 1e-6
        );
        assert!(
            (encoded["frequency_penalty"]
                .as_f64()
                .expect("frequency_penalty")
                - 0.3)
                .abs()
                < 1e-6
        );
        assert_eq!(encoded["seed"], 17);
        assert_eq!(
            encoded["stop_sequences"],
            serde_json::json!(["</end>", "STOP"])
        );
        Ok(())
    }

    #[test]
    fn golden_fixture_corpus_is_visible_from_serve() {
        assert_eq!(golden_prompt_fixtures().len(), 4);
        let tokenizer = golden_tokenizer_fixture("qwen2").expect("qwen2 fixture");
        assert_eq!(tokenizer.family, "qwen2");
    }

    #[test]
    fn golden_prompt_render_cases_are_reusable_from_serve() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = golden_prompt_fixture("qwen2").expect("qwen2 fixture");
        let variant = fixture.template_variant("qwen2.default").expect("variant");
        let render_case = variant
            .render_case("qwen2.with_system_history")
            .expect("render case");

        assert_rendered_prompt_case(render_case, render_case.expected_rendered)?;
        Ok(())
    }

    #[test]
    fn golden_prompt_window_cases_are_reusable_from_serve() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = golden_prompt_fixture("command_r").expect("command-r fixture");
        let variant = fixture
            .template_variant("command_r.default")
            .expect("variant");
        let window_case = fixture
            .window_cases
            .iter()
            .find(|case| case.id == "command_r.system_history_over_small_window")
            .expect("window case");
        let render_case = variant
            .render_case(window_case.render_case_id)
            .expect("render case");

        assert_prompt_window_case(window_case, render_case.expected_rendered)?;
        Ok(())
    }

    #[derive(Clone, Debug)]
    struct TestCatalog {
        list_models: ListModelsObservation,
        show_model: ShowObservation,
    }

    impl LocalModelCatalog for TestCatalog {
        fn list_models(&self) -> ListModelsObservation {
            self.list_models.clone()
        }

        fn show_model(&self, _model: &str) -> ShowObservation {
            self.show_model.clone()
        }
    }

    #[test]
    fn psionic_local_runtime_forwards_catalog_lifecycle_generation_and_embeddings()
    -> Result<(), Box<dyn std::error::Error>> {
        let generation = CpuReferenceTextGenerationService::new()?;
        let embeddings = SmokeEmbeddingsService::new()?;
        let decoder_descriptor = generation.model_descriptor().clone();
        let embedding_descriptor = embeddings.model_descriptor().clone();
        let catalog = TestCatalog {
            list_models: ListModelsObservation::new(vec![
                ModelSummary::from_decoder_descriptor(
                    decoder_descriptor.model.model_id.clone(),
                    &decoder_descriptor,
                ),
                ModelSummary::from_embedding_descriptor(
                    embedding_descriptor.model.model_id.clone(),
                    &embedding_descriptor,
                ),
            ]),
            show_model: ShowObservation::from_decoder_descriptor(
                decoder_descriptor.model.model_id.clone(),
                &decoder_descriptor,
            ),
        };
        let mut runtime = PsionicLocalRuntime::new(catalog, generation, embeddings);

        let listed = runtime.list_models();
        assert_eq!(listed.models.len(), 2);
        assert_eq!(listed.models[0].name, decoder_descriptor.model.model_id);
        assert_eq!(listed.models[1].name, embedding_descriptor.model.model_id);

        let shown = runtime.show_model(ReferenceWordDecoder::MODEL_ID);
        assert_eq!(shown.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(shown.family.as_deref(), Some("fixture_decoder"));

        let loaded = runtime.loaded_models();
        assert_eq!(loaded.models.len(), 1);
        assert_eq!(loaded.models[0].model, ReferenceWordDecoder::MODEL_ID);
        let observability = runtime.observability();
        assert_eq!(
            observability.isolation_policy,
            LocalServingIsolationPolicy::in_process_runtime()
        );
        assert_eq!(observability.queue_depth, 0);
        assert_eq!(observability.active_sessions, 0);
        assert_eq!(observability.active_requests, 0);
        assert_eq!(observability.memory_footprint.loaded_models, 1);
        assert_eq!(observability.backend_health.len(), 1);
        assert_eq!(observability.backend_health[0].backend, "cpu");
        assert_eq!(observability.backend_health[0].status, HealthStatus::Ready);
        assert_eq!(
            runtime.isolation_policy(),
            LocalServingIsolationPolicy::in_process_runtime()
        );
        assert!(
            observability
                .recent_transitions
                .iter()
                .any(|event| event.kind == RuntimeTransitionKind::ModelLoadedCold)
        );

        let warmed = runtime.warm_model(ReferenceWordDecoder::MODEL_ID, 0)?;
        assert_eq!(warmed.summary.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(warmed.residency.keep_alive_millis, 0);

        let generation_request = GenerationRequest::new_text(
            "runtime-generate-1",
            decoder_descriptor,
            None,
            "hello",
            GenerationOptions::greedy(4),
        );
        let generation_response = runtime.generate(&generation_request)?;
        assert_eq!(generation_response.output.text, "open agents");
        let mut generation_stream = runtime.generate_stream(&generation_request)?;
        let mut streamed = String::new();
        let mut stream_status = None;
        while let Some(event) = generation_stream.next_event() {
            match event {
                GenerationStreamEvent::Chunk(chunk) => streamed.push_str(&chunk.output.text),
                GenerationStreamEvent::Terminal(terminal) => {
                    stream_status = Some(terminal.status);
                    break;
                }
            }
        }
        assert_eq!(streamed, "open agents");
        assert_eq!(stream_status, Some(GenerationStreamStatus::Succeeded));
        drop(generation_stream);

        let embedding_request = EmbeddingRequest::new(
            "runtime-embed-1",
            embedding_descriptor,
            vec![String::from("hello world")],
        );
        let embedding_response = runtime.embed(&embedding_request)?;
        assert_eq!(embedding_response.embeddings.len(), 1);
        assert_eq!(embedding_response.metadata.vector_count, 1);

        let unloaded = runtime.unload_model(ReferenceWordDecoder::MODEL_ID)?;
        assert_eq!(unloaded.summary.model, ReferenceWordDecoder::MODEL_ID);
        assert!(runtime.loaded_models().models.is_empty());
        Ok(())
    }

    #[test]
    fn generation_sessions_isolate_and_reset_kv_cache() -> Result<(), Box<dyn std::error::Error>> {
        let descriptor = sample_decoder_descriptor();
        let served_artifact_digest =
            super::served_artifact_identity_for_decoder_backend(&descriptor, "cpu", &[])
                .served_artifact_digest;
        let mut store = InMemoryGenerationSessionStore::new();
        let session_a = store.create(&descriptor, served_artifact_digest.clone());
        let session_b = store.create(&descriptor, served_artifact_digest.clone());

        assert_eq!(session_a.model_family, "fixture_decoder");
        assert_eq!(session_a.model_revision, "v0");
        assert_eq!(session_a.weight_bundle_digest, descriptor.weights.digest);
        assert_eq!(session_a.kv_cache.pages, 0);
        assert_eq!(
            session_a.kv_cache_policy.page_layout.max_context_tokens,
            descriptor.config.max_context
        );

        store.append(
            &session_a.session_id,
            &descriptor,
            served_artifact_digest.as_str(),
            FixtureWordTokenizer::HELLO_ID,
            vec![1.0; descriptor.config.kv_width()],
            vec![2.0; descriptor.config.kv_width()],
        )?;
        store.append(
            &session_b.session_id,
            &descriptor,
            served_artifact_digest.as_str(),
            FixtureWordTokenizer::RUSTY_ID,
            vec![3.0; descriptor.config.kv_width()],
            vec![4.0; descriptor.config.kv_width()],
        )?;

        assert_eq!(
            store.cache(&session_a.session_id)?.entries()[0].token,
            FixtureWordTokenizer::HELLO_ID
        );
        assert_eq!(
            store.cache(&session_b.session_id)?.entries()[0].token,
            FixtureWordTokenizer::RUSTY_ID
        );
        assert_eq!(
            store.state(&session_a.session_id)?.tokens(),
            &[FixtureWordTokenizer::HELLO_ID]
        );
        assert_eq!(
            store.state(&session_b.session_id)?.tokens(),
            &[FixtureWordTokenizer::RUSTY_ID]
        );
        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
        assert_eq!(
            store
                .state(&session_a.session_id)?
                .session()
                .kv_cache
                .tokens,
            1
        );
        assert_eq!(
            store.state(&session_a.session_id)?.session().kv_cache.pages,
            1
        );
        assert_eq!(
            store.state(&session_a.session_id)?.session().kv_cache.bytes,
            32
        );

        let reset = store.reset(&session_a.session_id)?;
        assert_eq!(reset.cached_tokens, 0);
        assert_eq!(reset.kv_cache.tokens, 0);
        assert_eq!(reset.kv_cache.pages, 0);
        assert!(store.cache(&session_a.session_id)?.is_empty());
        assert!(store.state(&session_a.session_id)?.tokens().is_empty());
        assert_eq!(store.cache(&session_b.session_id)?.len(), 1);

        let closed = store.close(&session_b.session_id)?;
        assert_eq!(closed.cached_tokens, 1);
        assert!(store.cache(&session_b.session_id).is_err());
        Ok(())
    }

    #[test]
    fn generation_sessions_reject_descriptor_drift_even_when_model_id_matches() {
        let descriptor = sample_decoder_descriptor();
        let served_artifact_digest =
            super::served_artifact_identity_for_decoder_backend(&descriptor, "cpu", &[])
                .served_artifact_digest;
        let mut drifted = descriptor.clone();
        drifted.weights.digest = String::from("different-weight-bundle");

        let mut store = InMemoryGenerationSessionStore::new();
        let session = store.create(&descriptor, served_artifact_digest.clone());
        let error = store
            .append(
                &session.session_id,
                &drifted,
                served_artifact_digest.as_str(),
                FixtureWordTokenizer::HELLO_ID,
                vec![1.0; descriptor.config.kv_width()],
                vec![2.0; descriptor.config.kv_width()],
            )
            .expect_err("drifted descriptor should fail");

        assert!(matches!(
            error,
            super::SessionStoreError::ModelMismatch {
                expected_model,
                actual_model,
                expected_weight_bundle_digest,
                actual_weight_bundle_digest,
                ..
            } if expected_model == descriptor.model.model_id
                && actual_model == descriptor.model.model_id
                && expected_weight_bundle_digest == descriptor.weights.digest
                && actual_weight_bundle_digest == "different-weight-bundle"
        ));
    }

    #[test]
    fn paged_kv_cache_tracks_growth_refill_and_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            page_layout: KvCachePageLayout::new(5, 2, 32),
        };
        let mut cache = InMemoryKvCache::with_policy(5, 4, policy.clone());
        assert_eq!(cache.state(), KvCacheState::default());

        cache.append(TokenId(1), vec![0.0; 4], vec![1.0; 4])?;
        let first = cache.state();
        assert_eq!(first.tokens, 1);
        assert_eq!(first.pages, 1);
        assert_eq!(first.bytes, 32);

        cache.append(TokenId(2), vec![0.0; 4], vec![1.0; 4])?;
        let refill = KvCacheAccounting::from_states(&first, cache.state());
        assert_eq!(refill.current.tokens, 2);
        assert_eq!(refill.current.pages, 1);
        assert_eq!(refill.growth.tokens, 1);
        assert_eq!(refill.growth.pages, 0);

        cache.append(TokenId(3), vec![0.0; 4], vec![1.0; 4])?;
        let growth = KvCacheAccounting::from_states(&first, cache.state());
        assert_eq!(growth.current.pages, 2);
        assert_eq!(growth.growth.pages, 1);

        cache.append(TokenId(4), vec![0.0; 4], vec![1.0; 4])?;
        cache.append(TokenId(5), vec![0.0; 4], vec![1.0; 4])?;
        let error = cache
            .append(TokenId(6), vec![0.0; 4], vec![1.0; 4])
            .expect_err("page-budget refusal");
        assert!(matches!(
            error,
            KvCacheError::PageBudgetExceeded {
                requested_tokens: 6,
                max_context: 5,
                max_pages: 3,
                spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            }
        ));

        cache.reset();
        assert_eq!(cache.state(), KvCacheState::default());
        Ok(())
    }

    #[test]
    fn paged_kv_cache_tracks_owner_bound_page_eviction_and_reclaim()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::EvictOldestPages,
            page_layout: KvCachePageLayout::new(4, 2, 32),
        };
        let mut cache = InMemoryKvCache::with_policy(4, 2, policy);
        cache.bind_owner(request_kv_owner(
            &GenerationRequest::new_text(
                "req-1",
                sample_named_decoder_descriptor("owner-bound"),
                None,
                "hello",
                GenerationOptions::greedy(1),
            ),
            psionic_runtime::BatchExecutionPosture::ContinuousBatch,
            Some(0),
        ));
        let checkpoint = cache.checkpoint();

        for token in 1..=5 {
            cache.append(TokenId(token), vec![0.0; 2], vec![1.0; 2])?;
        }
        let ownership = cache
            .ownership_since(&checkpoint)
            .expect("owner-bound accounting");
        assert_eq!(ownership.owner.owner_class, KvCacheOwnerClass::Request);
        assert_eq!(ownership.owner.owner_id, "req-1");
        assert_eq!(ownership.current.tokens, 3);
        assert_eq!(ownership.current.pages, 2);
        assert_eq!(ownership.allocated_pages.len(), 3);
        assert_eq!(ownership.reclaimed_pages.len(), 1);
        assert_eq!(ownership.reclaimed_pages[0].page_index, 0);

        let reclaim_checkpoint = cache.checkpoint();
        cache.truncate(1);
        let reclaim = cache
            .ownership_since(&reclaim_checkpoint)
            .expect("truncate accounting");
        assert_eq!(reclaim.current.tokens, 1);
        assert_eq!(reclaim.current.pages, 1);
        assert_eq!(reclaim.reclaimed_pages.len(), 2);
        assert!(
            reclaim
                .reclaimed_pages
                .iter()
                .any(|page| page.token_count == 1)
        );
        Ok(())
    }

    #[test]
    fn host_device_kv_residency_reports_prefetch_writeback_and_refusal() {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::SpillToHost,
            page_layout: KvCachePageLayout::new(16, 4, 32),
        };

        let accounting = super::host_device_kv_residency(
            &policy,
            KvCacheState::paged(&policy.page_layout, 8),
            KvCacheState::paged(&policy.page_layout, 8),
            true,
            Some(psionic_runtime::KvCacheGrowth {
                tokens: 4,
                pages: 1,
                bytes: 128,
            }),
        )
        .expect("host/device residency");

        assert!(accounting.has_tier(KvResidencyTier::Host));
        assert!(accounting.has_tier(KvResidencyTier::Device));
        assert_eq!(accounting.movements.len(), 2);
        assert_eq!(
            accounting.movements[0].kind,
            KvResidencyMovementKind::Prefetch
        );
        assert_eq!(
            accounting.movements[1].kind,
            KvResidencyMovementKind::WriteBack
        );
        assert_eq!(accounting.refusals.len(), 1);
        assert_eq!(
            accounting.refusals[0].reason,
            KvResidencyRefusalReason::SpillUnsupported
        );
    }

    #[test]
    fn shared_prefix_store_reports_hit_miss_and_rebuilt() -> Result<(), Box<dyn std::error::Error>>
    {
        let loaded_model = CpuWordGenerationModel::new(ReferenceWordDecoder::new())?;
        let compatibility = prefix_compatibility(&loaded_model);
        let tokenizer = loaded_model.model().tokenizer();
        let hello = tokenizer.encode_with_special_tokens("hello", true, false);
        let hello_world = tokenizer.encode_with_special_tokens("hello world", true, false);
        let rusty = TokenSequence::new(vec![FixtureWordTokenizer::RUSTY_ID]);
        let width = loaded_model.descriptor().config.hidden_size;
        let vocab_size = loaded_model.descriptor().config.vocab_size;

        let mut cache = InMemoryKvCache::new(loaded_model.descriptor().config.max_context, width);
        for token in hello_world.as_slice() {
            cache.append(*token, vec![0.0; width], vec![1.0; width])?;
        }
        let prompt_logits = hello_world
            .as_slice()
            .iter()
            .map(|token| {
                let mut logits = vec![-1.0_f32; vocab_size];
                logits[token.as_u32() as usize] = token.as_u32() as f32;
                logits
            })
            .collect::<Vec<_>>();

        let mut store = SharedPrefixStore::default();
        store.record(compatibility.clone(), &hello_world, &prompt_logits, &cache);
        assert_eq!(
            store.entries[0]
                .cache
                .owner()
                .expect("shared prefix owner")
                .owner_class,
            KvCacheOwnerClass::SharedPrefix
        );

        let hit = store.lookup(&compatibility, &hello);
        assert_eq!(hit.state, PrefixCacheState::Hit);
        assert_eq!(hit.reused_tokens, hello.len());
        assert_eq!(
            hit.identity.as_ref().map(|value| value.prefix_tokens),
            Some(hello.len())
        );
        assert_eq!(
            hit.cache.as_ref().map(InMemoryKvCache::len),
            Some(hello.len())
        );
        assert_eq!(hit.prompt_logits.len(), hello.len());
        assert_eq!(hit.last_logits, {
            let mut logits = vec![-1.0_f32; vocab_size];
            logits[hello.as_slice()[hello.len() - 1].as_u32() as usize] =
                hello.as_slice()[hello.len() - 1].as_u32() as f32;
            logits
        });

        let exact_hit = store.lookup(&compatibility, &hello_world);
        assert_eq!(exact_hit.state, PrefixCacheState::Hit);
        assert_eq!(exact_hit.reused_tokens, hello_world.len());
        assert!(exact_hit.prompt_logits.is_empty());
        assert_eq!(exact_hit.last_logits, {
            let mut logits = vec![-1.0_f32; vocab_size];
            logits[hello_world.as_slice()[hello_world.len() - 1].as_u32() as usize] =
                hello_world.as_slice()[hello_world.len() - 1].as_u32() as f32;
            logits
        });

        let exact_prompt = store
            .lookup_exact_prompt(&compatibility, &hello_world)
            .expect("exact prompt lookup should hit");
        assert_eq!(exact_prompt.cache.len(), hello_world.len());
        assert_eq!(exact_prompt.last_logits, {
            let mut logits = vec![-1.0_f32; vocab_size];
            logits[hello_world.as_slice()[hello_world.len() - 1].as_u32() as usize] =
                hello_world.as_slice()[hello_world.len() - 1].as_u32() as f32;
            logits
        });
        assert_eq!(
            exact_prompt.greedy_token,
            Some(hello_world.as_slice()[hello_world.len() - 1].as_u32())
        );

        let miss = store.lookup(&compatibility, &rusty);
        assert_eq!(miss.state, PrefixCacheState::Miss);
        assert_eq!(miss.reused_tokens, 0);

        store.entries[0]
            .cache
            .truncate(hello_world.len().saturating_sub(1));
        let rebuilt = store.lookup(&compatibility, &hello_world);
        assert_eq!(rebuilt.state, PrefixCacheState::Rebuilt);
        assert!(store.entries.is_empty());
        Ok(())
    }

    #[test]
    fn shared_prefix_store_preserves_exact_hit_for_exact_only_logit_receipts()
    -> Result<(), Box<dyn std::error::Error>> {
        let decoder = ReferenceWordDecoder::new();
        let compatibility = SharedPrefixCompatibility {
            served_artifact_digest: String::from("artifact"),
            model_id: String::from("reference-word-decoder"),
            model_revision: String::from("rev"),
            weight_bundle_digest: String::from("weights"),
            tokenizer_family: String::from("fixture-word"),
            tokenizer_digest: Some(String::from("tokenizer")),
            chat_template_digest: Some(String::from("chat-template")),
            generation_defaults_digest: Some(String::from("defaults")),
            backend_compatibility: String::from("cpu"),
            tenant_id: None,
            sampler_digest: None,
        };
        let hello_world = decoder.encode_prompt_text("hello world");
        let hello = decoder.encode_prompt_text("hello");
        let vocab_size = decoder.tokenizer().vocabulary().tokens().len();
        let width = decoder.descriptor().config.kv_width();
        let mut cache = InMemoryKvCache::new(decoder.descriptor().config.max_context, width);
        for token in hello_world.as_slice() {
            cache.append(*token, vec![0.0; width], vec![1.0; width])?;
        }
        let mut last_logits = vec![-1.0_f32; vocab_size];
        last_logits[hello_world.as_slice()[hello_world.len() - 1].as_u32() as usize] =
            hello_world.as_slice()[hello_world.len() - 1].as_u32() as f32;

        let mut store = SharedPrefixStore::default();
        store.record(
            compatibility.clone(),
            &hello_world,
            &[last_logits.clone()],
            &cache,
        );

        let partial = store.lookup(&compatibility, &hello);
        assert_eq!(partial.state, PrefixCacheState::Hit);
        assert_eq!(partial.reused_tokens, hello.len());
        assert_eq!(
            partial.cache.as_ref().map(InMemoryKvCache::len),
            Some(hello.len())
        );
        assert!(partial.prompt_logits.is_empty());
        assert!(partial.last_logits.is_empty());

        let exact = store
            .lookup_exact_prompt(&compatibility, &hello_world)
            .expect("exact-only receipt should still support exact hits");
        assert_eq!(exact.cache.len(), hello_world.len());
        assert_eq!(exact.last_logits, last_logits);
        Ok(())
    }

    #[test]
    fn shared_prefix_store_reports_tenant_and_sampler_boundary_refusals()
    -> Result<(), Box<dyn std::error::Error>> {
        let loaded_model = CpuWordGenerationModel::new(ReferenceWordDecoder::new())?;
        let mut compatibility = prefix_compatibility(&loaded_model);
        compatibility.tenant_id = Some(String::from("tenant-a"));
        compatibility.sampler_digest = Some(String::from("sampler-a"));

        let tokenizer = loaded_model.model().tokenizer();
        let hello_world = tokenizer.encode_with_special_tokens("hello world", true, false);
        let hello = tokenizer.encode_with_special_tokens("hello", true, false);
        let width = loaded_model.descriptor().config.hidden_size;
        let vocab_size = loaded_model.descriptor().config.vocab_size;

        let mut cache = InMemoryKvCache::new(loaded_model.descriptor().config.max_context, width);
        for token in hello_world.as_slice() {
            cache.append(*token, vec![0.0; width], vec![1.0; width])?;
        }
        let prompt_logits = hello_world
            .as_slice()
            .iter()
            .map(|token| {
                let mut logits = vec![-1.0_f32; vocab_size];
                logits[token.as_u32() as usize] = token.as_u32() as f32;
                logits
            })
            .collect::<Vec<_>>();

        let mut store = SharedPrefixStore::default();
        store.record(compatibility.clone(), &hello_world, &prompt_logits, &cache);

        let mut tenant_mismatch = compatibility.clone();
        tenant_mismatch.tenant_id = Some(String::from("tenant-b"));
        let tenant_result = store.lookup(&tenant_mismatch, &hello);
        assert_eq!(tenant_result.state, PrefixCacheState::Bypassed);
        assert_eq!(
            tenant_result.refusal_reason,
            Some(PrefixCacheRefusalReason::TenantBoundary)
        );

        let mut sampler_mismatch = compatibility;
        sampler_mismatch.sampler_digest = Some(String::from("sampler-b"));
        let sampler_result = store.lookup(&sampler_mismatch, &hello);
        assert_eq!(sampler_result.state, PrefixCacheState::Bypassed);
        assert_eq!(
            sampler_result.refusal_reason,
            Some(PrefixCacheRefusalReason::SamplerBoundary)
        );
        Ok(())
    }

    #[test]
    fn model_registry_tracks_active_generation_models() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let model = ReferenceWordDecoder::new();

        assert!(registry.load(model).expect("load model").is_none());
        assert_eq!(registry.len(), 1);
        assert!(registry.active(ReferenceWordDecoder::MODEL_ID).is_some());
        assert!(registry.unload(ReferenceWordDecoder::MODEL_ID).is_some());
        assert!(registry.is_empty());
    }

    #[derive(Clone, Debug)]
    struct TestGenerationHandle {
        descriptor: DecoderModelDescriptor,
    }

    impl GenerationModelHandle for TestGenerationHandle {
        fn descriptor(&self) -> &DecoderModelDescriptor {
            &self.descriptor
        }
    }

    #[derive(Clone, Debug)]
    struct FailingStreamWordDecoder {
        inner: ReferenceWordDecoder,
        fail_at_position: usize,
    }

    impl WordDecoderExecutionModel for FailingStreamWordDecoder {
        fn descriptor(&self) -> &DecoderModelDescriptor {
            self.inner.descriptor()
        }

        fn tokenizer(&self) -> &dyn TokenizerBoundary {
            self.inner.tokenizer()
        }

        fn weights(&self) -> &DecoderFixtureWeights {
            self.inner.weights()
        }

        fn encode_prompt_text(&self, text: &str) -> TokenSequence {
            self.inner
                .tokenizer()
                .encode_with_special_tokens(text, true, false)
        }

        fn injected_stream_failure(&self, position: usize) -> Option<ReferenceTextGenerationError> {
            (position >= self.fail_at_position).then_some(
                ReferenceTextGenerationError::MissingOutput("injected_stream_failure"),
            )
        }
    }

    #[test]
    fn model_registry_tracks_keepalive_order_and_idle_expiry() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let alpha = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("alpha"),
        };
        let beta = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("beta"),
        };

        assert!(
            registry
                .warm_with_metadata(
                    alpha,
                    1_000,
                    5_000,
                    Some(64),
                    Some(String::from("cpu")),
                    None
                )
                .expect("warm alpha")
                .is_none()
        );
        assert!(
            registry
                .warm_with_metadata(
                    beta,
                    2_000,
                    2_000,
                    Some(32),
                    Some(String::from("cpu")),
                    None
                )
                .expect("warm beta")
                .is_none()
        );

        let views = registry.loaded_model_views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].summary.model, "alpha");
        assert_eq!(views[0].residency.expires_at_millis, Some(6_000));
        assert_eq!(views[0].memory_plan.resident_device_bytes, 64);
        assert_eq!(views[1].summary.model, "beta");
        assert_eq!(views[1].residency.expires_at_millis, Some(4_000));
        assert_eq!(views[1].memory_plan.resident_device_bytes, 32);

        let warmed = registry
            .warm_loaded("beta", 3_000, 9_000)
            .expect("warm existing beta");
        assert_eq!(warmed.residency.keep_alive_millis, 9_000);
        assert_eq!(warmed.residency.expires_at_millis, Some(12_000));

        let request_start = registry
            .begin_request("beta", 3_500)
            .expect("begin request");
        assert_eq!(request_start.residency.state, LoadedModelState::Ready);
        assert_eq!(request_start.residency.active_requests, 1);
        assert_eq!(request_start.residency.expires_at_millis, None);

        let during_request = registry.loaded_model_views();
        assert_eq!(during_request[0].summary.model, "beta");

        let request_finish = registry
            .finish_request("beta", 4_000)
            .expect("finish request");
        assert_eq!(request_finish.residency.active_requests, 0);
        assert_eq!(request_finish.residency.expires_at_millis, Some(13_000));

        let expired = registry.expire_idle(6_001);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].descriptor().model.model_id, "alpha");

        let remaining = registry.loaded_model_views();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].summary.model, "beta");
    }

    #[test]
    fn model_registry_zero_keepalive_unloads_when_idle() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let gamma = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("gamma"),
        };

        assert!(
            registry
                .warm_with_metadata(gamma, 10_000, 5_000, None, Some(String::from("cpu")), None)
                .expect("warm gamma")
                .is_none()
        );
        let warmed = registry
            .warm_loaded("gamma", 10_100, 0)
            .expect("warm gamma with zero keepalive");
        assert_eq!(warmed.residency.expires_at_millis, Some(10_100));

        let expired = registry.expire_idle(10_100);
        assert_eq!(expired.len(), 1);
        assert!(registry.is_empty());
    }

    #[test]
    fn model_registry_records_lifecycle_transitions() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let alpha = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("alpha"),
        };

        registry
            .warm_with_metadata(alpha, 1_000, 5_000, None, Some(String::from("cpu")), None)
            .expect("warm alpha");
        registry.begin_request("alpha", 2_000).expect("begin alpha");
        registry
            .finish_request("alpha", 2_100)
            .expect("finish alpha");
        registry.unload_view("alpha", 2_100).expect("unload alpha");

        assert_eq!(
            registry.recent_transitions(),
            vec![
                RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelLoadedCold,
                    "alpha",
                    1_000,
                ),
                RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelBecameWarm,
                    "alpha",
                    2_000,
                ),
                RuntimeTransitionEvent::model(RuntimeTransitionKind::ModelUnloaded, "alpha", 2_100,),
            ]
        );
    }

    #[test]
    fn cpu_reference_observability_reports_sessions_memory_and_transitions()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let initial = service.observability_at(1_000);
        assert_eq!(initial.queue_depth, 0);
        assert_eq!(initial.active_sessions, 0);
        assert_eq!(initial.active_requests, 0);
        assert_eq!(initial.memory_footprint.loaded_models, 1);
        assert_eq!(initial.backend_health[0].backend, "cpu");
        assert_eq!(initial.backend_health[0].status, HealthStatus::Ready);
        assert_eq!(
            initial.recent_transitions[0].kind,
            RuntimeTransitionKind::ModelLoadedCold
        );

        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
        let with_session = service.observability_at(1_100);
        assert_eq!(with_session.active_sessions, 1);

        let request = GenerationRequest::new_text(
            "obs-gen-1",
            service.model_descriptor().clone(),
            Some(session.session_id),
            "hello",
            GenerationOptions::greedy(2),
        );
        let response = service.generate(&request)?;
        assert_eq!(response.output.text, "open agents");

        let after_generate = service.observability_at(1_200);
        assert_eq!(after_generate.active_requests, 0);
        assert!(
            after_generate
                .recent_transitions
                .iter()
                .any(|event| event.kind == RuntimeTransitionKind::ModelBecameWarm)
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_continuous_batch_scheduler_mixes_prefill_and_decode()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let descriptor = service.model_descriptor().clone();
        let long_prompt_len = descriptor
            .config
            .max_context
            .saturating_sub(2)
            .min(5)
            .max(1);
        let long_prompt_tokens = (0..long_prompt_len)
            .map(|index| match index % 4 {
                0 => FixtureWordTokenizer::HELLO_ID,
                1 => FixtureWordTokenizer::OPEN_ID,
                2 => FixtureWordTokenizer::AGENTS_ID,
                _ => FixtureWordTokenizer::WORLD_ID,
            })
            .collect::<Vec<_>>();
        let long_prompt = GenerationRequest::new_tokens(
            "batch-long",
            descriptor.clone(),
            None,
            TokenSequence::new(long_prompt_tokens),
            GenerationOptions::greedy(1),
        );
        let short_prompt = GenerationRequest::new_text(
            "batch-short",
            descriptor,
            None,
            "hello",
            GenerationOptions::greedy(2),
        );

        let result = service.generate_continuous_batch(vec![long_prompt, short_prompt]);
        assert_eq!(result.scheduler_metrics.max_batch_size, 2);
        assert_eq!(
            result.scheduler_metrics.last_scheduling_class,
            Some(psionic_runtime::GenerationSchedulingClass::MixedPrefillDecode)
        );
        assert!(result.scheduler_metrics.total_prefill_tokens >= 6);
        assert!(result.scheduler_metrics.total_decode_tokens >= 3);
        assert!(result.scheduler_metrics.peak_kv_pages_in_use > 0);
        assert!(result.scheduler_metrics.peak_kv_bytes_in_use > 0);
        assert!(result.scheduler_metrics.total_kv_pages_allocated >= 2);

        let responses = result
            .responses
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].usage.output_tokens, 1);
        assert_eq!(responses[1].output.text, "open agents");
        for response in responses {
            let receipt = response
                .provenance
                .as_ref()
                .and_then(|value| value.scheduler.as_ref())
                .expect("scheduler receipt");
            assert_eq!(
                receipt.batch_posture,
                psionic_runtime::BatchExecutionPosture::ContinuousBatch
            );
            assert!(matches!(
                receipt.scheduling_class,
                psionic_runtime::GenerationSchedulingClass::MixedPrefillDecode
                    | psionic_runtime::GenerationSchedulingClass::Decode
            ));
            assert!(receipt.max_batch_size_observed >= 1);
            let kv_ownership = response
                .provenance
                .as_ref()
                .and_then(|value| value.kv_ownership.as_ref())
                .expect("kv ownership");
            assert_eq!(kv_ownership.owner.owner_class, KvCacheOwnerClass::Request);
            assert_eq!(kv_ownership.owner.model_id, response.model_id);
            assert_eq!(
                kv_ownership
                    .owner
                    .scheduler
                    .as_ref()
                    .and_then(|value| value.queue_depth_at_admission),
                Some(receipt.queue_depth_at_admission)
            );
            assert!(kv_ownership.current.pages >= kv_ownership.previous.pages);
        }
        Ok(())
    }

    #[test]
    fn model_registry_refuses_candidate_when_host_budget_would_be_exceeded() {
        let policy = ModelResidencyPolicy {
            max_loaded_models: None,
            memory_budget: MemoryBudget {
                resident_host_bytes: Some(300),
                resident_device_bytes: None,
            },
            pressure_action: ResidencyPressureAction::RefuseNewModel,
        };
        let mut registry = InMemoryGenerationModelRegistry::with_residency_policy(policy);
        let alpha = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("alpha"),
        };
        let beta = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("beta"),
        };

        registry
            .warm_with_metadata(alpha, 1_000, 5_000, None, Some(String::from("cpu")), None)
            .expect("warm alpha");
        let error = registry
            .warm_with_metadata(beta, 2_000, 5_000, None, Some(String::from("cpu")), None)
            .expect_err("beta should be refused");

        assert!(matches!(
            error,
            LoadedModelRegistryError::AdmissionRefused(ref refusal)
                if refusal.reason == AdmissionRefusalReason::HostMemoryBudget
        ));
        assert_eq!(registry.len(), 1);
        assert!(registry.active("alpha").is_some());
        assert!(registry.active("beta").is_none());
    }

    #[test]
    fn model_registry_can_evict_oldest_idle_model_to_admit_new_candidate() {
        let policy = ModelResidencyPolicy {
            max_loaded_models: Some(1),
            memory_budget: MemoryBudget {
                resident_host_bytes: Some(300),
                resident_device_bytes: None,
            },
            pressure_action: ResidencyPressureAction::UnloadIdleOldestFirst,
        };
        let mut registry = InMemoryGenerationModelRegistry::with_residency_policy(policy);
        let alpha = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("alpha"),
        };
        let beta = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("beta"),
        };

        registry
            .warm_with_metadata(alpha, 1_000, 5_000, None, Some(String::from("cpu")), None)
            .expect("warm alpha");
        registry
            .warm_with_metadata(beta, 2_000, 5_000, None, Some(String::from("cpu")), None)
            .expect("beta should evict alpha to fit");

        let views = registry.loaded_model_views();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].summary.model, "beta");
        assert_eq!(views[0].residency_snapshot.loaded_models, 1);
        assert_eq!(views[0].residency_snapshot.resident_host_bytes, 192);
        assert!(registry.active("alpha").is_none());
        assert!(registry.active("beta").is_some());
    }

    #[test]
    fn cpu_reference_generation_stream_emits_chunks_then_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "stream-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );
        let mut stream = service.generate_stream(&request)?;
        assert_eq!(stream.policy(), &default_generation_streaming_policy());

        let mut chunk_text = String::new();
        let mut terminal = None;
        while let Some(event) = stream.next_event() {
            match event {
                GenerationStreamEvent::Chunk(chunk) => {
                    chunk_text.push_str(&chunk.output.text);
                }
                GenerationStreamEvent::Terminal(value) => {
                    terminal = Some(value);
                    break;
                }
            }
        }

        let terminal = terminal.expect("terminal event");
        assert_eq!(chunk_text, "open agents");
        assert_eq!(terminal.status, GenerationStreamStatus::Succeeded);
        assert_eq!(terminal.response.output.text, "open agents");
        assert_eq!(
            terminal
                .response
                .provenance
                .as_ref()
                .and_then(|value| value.streaming_policy.clone()),
            Some(default_generation_streaming_policy())
        );
        drop(stream);
        assert_eq!(service.loaded_model_views()[0].residency.active_requests, 0);
        Ok(())
    }

    #[test]
    fn cpu_reference_generation_stream_cancellation_discards_uncommitted_session_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
        let request = GenerationRequest::new_text(
            "stream-cancel-1",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "hello",
            GenerationOptions::greedy(4),
        );

        let mut stream = service.generate_stream(&request)?;
        let first = stream.next_event().expect("first stream event");
        assert!(matches!(first, GenerationStreamEvent::Chunk(_)));
        let terminal = stream.cancel().expect("cancel terminal");
        assert_eq!(terminal.status, GenerationStreamStatus::Cancelled);
        assert_eq!(terminal.response.termination, TerminationReason::Cancelled);

        drop(stream);

        let follow_up = GenerationRequest::new_text(
            "stream-cancel-2",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "rusty",
            GenerationOptions::greedy(1),
        );
        let follow_up_response = service.generate(&follow_up)?;
        assert_eq!(follow_up_response.output.text, "grad");
        assert_eq!(follow_up_response.usage.cache_tokens, 3);
        Ok(())
    }

    #[test]
    fn cpu_reference_generation_stream_disconnect_returns_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "stream-disconnect-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );

        let mut stream = service.generate_stream(&request)?;
        let _ = stream.next_event().expect("first event");
        let terminal = stream.disconnect().expect("disconnect terminal");
        assert_eq!(terminal.status, GenerationStreamStatus::Disconnected);
        assert_eq!(
            terminal.response.termination,
            TerminationReason::Disconnected
        );
        assert_eq!(
            terminal.failure_reason.as_deref(),
            Some("stream disconnected by caller")
        );
        Ok(())
    }

    #[test]
    fn generation_stream_returns_error_before_first_chunk_when_prompt_is_empty() {
        let mut service =
            CpuReferenceTextGenerationService::new().expect("reference service should build");
        let request = GenerationRequest::new_tokens(
            "stream-empty-1",
            service.model_descriptor().clone(),
            None,
            TokenSequence::new(Vec::new()),
            GenerationOptions::greedy(1),
        );

        let error = match service.generate_stream(&request) {
            Ok(_) => panic!("empty prompt should fail before streaming starts"),
            Err(error) => error,
        };
        assert!(matches!(error, ReferenceTextGenerationError::EmptyPrompt));
        let diagnostic = error.diagnostic_for_request(&request);
        assert_eq!(diagnostic.code, LocalRuntimeErrorCode::InvalidRequest);
        assert_eq!(diagnostic.status, 400);
        assert_eq!(
            diagnostic.product_id.as_deref(),
            Some("psionic.text_generation")
        );
    }

    #[test]
    fn generation_stream_reports_runtime_failure_after_stream_start()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CpuBackend::new();
        let mut models = InMemoryGenerationModelRegistry::new();
        let failing_model = FailingStreamWordDecoder {
            inner: ReferenceWordDecoder::new(),
            fail_at_position: 3,
        };
        models.warm_with_metadata(
            CpuWordGenerationModel::new(failing_model.clone())?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        let mut sessions = InMemoryGenerationSessionStore::new();
        let mut shared_prefixes = SharedPrefixStore::default();
        let request = GenerationRequest::new_text(
            "stream-fail-1",
            failing_model.descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );

        let mut stream = CpuGenerationStream::new(
            &mut backend,
            &mut models,
            &mut sessions,
            &mut shared_prefixes,
            &request,
        )?;
        let mut saw_chunk = false;
        let mut terminal = None;
        while let Some(event) = stream.next_event() {
            match event {
                GenerationStreamEvent::Chunk(_) => saw_chunk = true,
                GenerationStreamEvent::Terminal(value) => {
                    terminal = Some(value);
                    break;
                }
            }
        }

        let terminal = terminal.expect("failure terminal");
        assert!(saw_chunk);
        assert_eq!(terminal.status, GenerationStreamStatus::Failed);
        assert_eq!(terminal.response.termination, TerminationReason::Error);
        assert_eq!(
            terminal.failure_reason.as_deref(),
            Some("missing graph output `injected_stream_failure`")
        );
        assert_eq!(
            terminal.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::Internal)
        );
        assert_eq!(
            terminal
                .diagnostic
                .as_ref()
                .and_then(|value| value.product_id.as_deref()),
            Some("psionic.text_generation")
        );
        assert_eq!(
            terminal
                .diagnostic
                .as_ref()
                .and_then(|value| value.model_id.as_deref()),
            Some(ReferenceWordDecoder::MODEL_ID)
        );
        Ok(())
    }

    #[test]
    fn seeded_sampling_is_replayable() {
        let options = GenerationOptions {
            max_output_tokens: 4,
            context_overflow_policy: ContextOverflowPolicy::Refuse,
            decode_strategy: super::DecodeStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: Some(42),
            stop_sequences: Vec::new(),
            structured_output: None,
        };
        let cache = super::InMemoryKvCache::new(8, 1);
        let logits = vec![3.0, 2.9, 2.8];
        let tokenizer = FixtureWordTokenizer::new();
        let mut left = super::GenerationSampler::new(&options).expect("sampler");
        let mut right = super::GenerationSampler::new(&options).expect("sampler");

        let left_draws = (0..4)
            .map(|_| {
                match left
                    .select_next_token(&tokenizer, &logits, &cache, &[])
                    .expect("sample")
                {
                    super::GenerationSelection::Token(token) => token,
                    super::GenerationSelection::Terminate => panic!("unexpected terminate"),
                }
            })
            .collect::<Vec<_>>();
        let right_draws = (0..4)
            .map(|_| {
                match right
                    .select_next_token(&tokenizer, &logits, &cache, &[])
                    .expect("sample")
                {
                    super::GenerationSelection::Token(token) => token,
                    super::GenerationSelection::Terminate => panic!("unexpected terminate"),
                }
            })
            .collect::<Vec<_>>();

        assert_eq!(left_draws, right_draws);
    }

    #[test]
    fn penalties_shift_token_selection() -> Result<(), Box<dyn std::error::Error>> {
        let options = GenerationOptions {
            max_output_tokens: 4,
            context_overflow_policy: ContextOverflowPolicy::Refuse,
            decode_strategy: super::DecodeStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(2.0),
            presence_penalty: Some(0.5),
            frequency_penalty: Some(0.5),
            seed: None,
            stop_sequences: Vec::new(),
            structured_output: None,
        };
        let mut cache = super::InMemoryKvCache::new(8, 1);
        cache.append(TokenId(1), vec![0.0], vec![0.0])?;
        cache.append(TokenId(1), vec![0.0], vec![0.0])?;

        let tokenizer = FixtureWordTokenizer::new();
        let mut sampler = super::GenerationSampler::new(&options).expect("sampler");
        assert_eq!(
            sampler.select_next_token(&tokenizer, &[1.0, 3.0, 2.5], &cache, &[])?,
            super::GenerationSelection::Token(TokenId(2))
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_replays_seeded_sampling_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut left = CpuReferenceTextGenerationService::new()?;
        let mut right = CpuReferenceTextGenerationService::new()?;
        let options = GenerationOptions {
            max_output_tokens: 4,
            context_overflow_policy: ContextOverflowPolicy::Refuse,
            decode_strategy: super::DecodeStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: Some(1.1),
            presence_penalty: Some(0.2),
            frequency_penalty: Some(0.1),
            seed: Some(42),
            stop_sequences: Vec::new(),
            structured_output: None,
        };
        let left_request = GenerationRequest::new_text(
            "gen-ref-seeded-left",
            left.model_descriptor().clone(),
            None,
            "hello",
            options.clone(),
        );
        let right_request = GenerationRequest::new_text(
            "gen-ref-seeded-right",
            right.model_descriptor().clone(),
            None,
            "hello",
            options,
        );

        let left_response = left.generate(&left_request)?;
        let right_response = right.generate(&right_request)?;

        assert_eq!(left_response.output, right_response.output);
        assert_eq!(left_response.usage, right_response.usage);
        assert_eq!(left_response.termination, right_response.termination);
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );

        let first = service.generate(&request)?;
        let second = service.generate(&request)?;
        assert_eq!(first.output, second.output);
        assert_eq!(first.usage, second.usage);
        assert_eq!(first.termination, second.termination);
        assert_eq!(
            first
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            second
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str())
        );
        assert_eq!(
            first.output.tokens.as_slice(),
            &[
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID,
            ]
        );
        assert_eq!(first.output.text, "open agents");
        assert_eq!(first.termination, TerminationReason::EndOfSequence);
        assert_eq!(first.usage.input_tokens, 2);
        assert!(
            service
                .plan_digest(ReferenceWordDecoder::MODEL_ID)
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_reports_cold_then_warm_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-metrics-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );
        let expected_plan_digest = service
            .plan_digest(ReferenceWordDecoder::MODEL_ID)
            .expect("plan digest")
            .to_string();

        let first = service.generate(&request)?;
        assert_eq!(
            first.provenance.as_ref().map(|value| value.load_state),
            Some(GenerationLoadState::Cold)
        );
        assert_eq!(
            first
                .provenance
                .as_ref()
                .map(|value| value.isolation_policy.clone()),
            Some(LocalServingIsolationPolicy::in_process_runtime())
        );
        assert_eq!(
            first
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert_eq!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .is_some_and(|value| value.kernel_count > 0 && value.bytes_moved > 0)
        );
        assert_eq!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .and_then(|value| value.kv_growth.as_ref())
                .map(|value| value.pages),
            Some(1)
        );
        assert_eq!(
            first.metrics.prompt_eval_count,
            Some(first.usage.input_tokens)
        );
        assert_eq!(
            first
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.input_prompt_tokens),
            Some(first.usage.input_tokens)
        );
        assert_eq!(
            first
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.truncated_prompt_tokens),
            Some(0)
        );
        assert_eq!(first.metrics.eval_count, Some(first.usage.output_tokens));
        assert!(first.metrics.prompt_eval_duration_ns.is_some());
        assert!(first.metrics.eval_duration_ns.is_some());
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.tokens),
            Some(first.usage.cache_tokens)
        );
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.pages),
            Some(1)
        );
        assert!(
            first
                .metrics
                .kv_residency
                .as_ref()
                .is_some_and(|value| value.has_tier(KvResidencyTier::Host))
        );
        assert!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .and_then(|value| value.kv_residency.as_ref())
                .is_some_and(|value| value.has_tier(KvResidencyTier::Host))
        );
        assert!(first.metrics.total_duration_ns.is_some());
        assert!(first.metrics.load_duration_ns.is_some());
        assert!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.kv_cache_policy.as_ref())
                .is_some()
        );

        let second = service.generate(&GenerationRequest::new_text(
            "gen-ref-metrics-2",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        ))?;
        assert_eq!(
            second.provenance.as_ref().map(|value| value.load_state),
            Some(GenerationLoadState::Warm)
        );
        assert_eq!(
            second
                .provenance
                .as_ref()
                .map(|value| value.isolation_policy.clone()),
            Some(LocalServingIsolationPolicy::in_process_runtime())
        );
        assert_eq!(
            second
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert_eq!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .is_some_and(|value| value.kernel_count > 0 && value.plan_cache_hits > 0)
        );
        assert_eq!(
            second.metrics.prompt_eval_count,
            Some(second.usage.input_tokens)
        );
        assert_eq!(
            second
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.truncated_prompt_tokens),
            Some(0)
        );
        assert_eq!(second.metrics.eval_count, Some(second.usage.output_tokens));
        assert!(second.metrics.prompt_eval_duration_ns.is_some());
        assert!(second.metrics.eval_duration_ns.is_some());
        assert_eq!(
            second
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.tokens),
            Some(second.usage.cache_tokens)
        );
        assert!(
            second
                .metrics
                .kv_residency
                .as_ref()
                .is_some_and(|value| value.has_tier(KvResidencyTier::Host))
        );
        assert!(second.metrics.total_duration_ns.is_some());
        assert_eq!(second.metrics.load_duration_ns, Some(0));
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_reports_prefix_hits_and_bypasses()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let shared_tenant = PrefixCacheControl {
            mode: PrefixCacheMode::Auto,
            tenant_id: Some(String::from("tenant-a")),
        };

        let first = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-1",
                service.model_descriptor().clone(),
                None,
                "hello world",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(shared_tenant.clone()),
        )?;
        assert_eq!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::None)
        );
        assert_eq!(first.metrics.prefix_tokens_reused, Some(0));

        let second = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-2",
                service.model_descriptor().clone(),
                None,
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(shared_tenant.clone()),
        )?;
        assert_eq!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Hit)
        );
        assert_eq!(second.metrics.prefix_tokens_reused, Some(2));
        assert_eq!(second.output.text, "open agents");
        assert_eq!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_identity.as_ref())
                .map(|value| value.prefix_tokens),
            Some(2)
        );

        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
        let warmed_session = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-3",
                service.model_descriptor().clone(),
                Some(session.session_id.clone()),
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(shared_tenant.clone()),
        )?;
        assert_eq!(
            warmed_session
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Hit)
        );
        assert_eq!(warmed_session.metrics.prefix_tokens_reused, Some(2));

        let boundary_session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
        let tenant_boundary = service.generate(&GenerationRequest::new_text(
            "gen-ref-prefix-4",
            service.model_descriptor().clone(),
            Some(boundary_session.session_id),
            "hello",
            GenerationOptions::greedy(4),
        ))?;
        assert_eq!(
            tenant_boundary
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Bypassed)
        );
        assert_eq!(
            tenant_boundary
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_refusal_reason),
            Some(PrefixCacheRefusalReason::TenantBoundary)
        );
        assert_eq!(tenant_boundary.metrics.prefix_tokens_reused, Some(0));

        let bypassed = service.generate(&GenerationRequest::new_text(
            "gen-ref-prefix-5",
            service.model_descriptor().clone(),
            Some(session.session_id),
            "rusty",
            GenerationOptions::greedy(1),
        ))?;
        assert_eq!(
            bypassed
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Bypassed)
        );
        assert_eq!(
            bypassed
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_refusal_reason),
            Some(PrefixCacheRefusalReason::SessionBoundState)
        );
        assert_eq!(bypassed.metrics.prefix_tokens_reused, Some(0));
        assert_eq!(bypassed.output.text, "grad");
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_reports_prefix_control_refusals_and_invalidations()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let tenant = String::from("tenant-a");

        let seed = GenerationRequest::new_text(
            "gen-ref-prefix-control-1",
            service.model_descriptor().clone(),
            None,
            "hello world",
            GenerationOptions::greedy(4),
        )
        .with_prefix_cache_control(PrefixCacheControl {
            mode: PrefixCacheMode::Auto,
            tenant_id: Some(tenant.clone()),
        });
        let seeded = service.generate(&seed)?;
        assert_eq!(
            seeded
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::None)
        );

        let bypassed = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-control-2",
                service.model_descriptor().clone(),
                None,
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(PrefixCacheControl {
                mode: PrefixCacheMode::Bypass,
                tenant_id: Some(tenant.clone()),
            }),
        )?;
        assert_eq!(
            bypassed
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Bypassed)
        );
        assert_eq!(
            bypassed
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_refusal_reason),
            Some(PrefixCacheRefusalReason::RequestOptOut)
        );
        assert_eq!(bypassed.metrics.prefix_tokens_reused, Some(0));

        let hit = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-control-3",
                service.model_descriptor().clone(),
                None,
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(PrefixCacheControl {
                mode: PrefixCacheMode::Auto,
                tenant_id: Some(tenant.clone()),
            }),
        )?;
        assert_eq!(
            hit.provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Hit)
        );
        assert_eq!(hit.metrics.prefix_tokens_reused, Some(2));

        let invalidated = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-control-4",
                service.model_descriptor().clone(),
                None,
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(PrefixCacheControl {
                mode: PrefixCacheMode::Invalidate,
                tenant_id: Some(tenant.clone()),
            }),
        )?;
        assert_eq!(
            invalidated
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Rebuilt)
        );
        assert_eq!(
            invalidated
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_refusal_reason),
            Some(PrefixCacheRefusalReason::ForcedInvalidation)
        );
        assert!(
            invalidated
                .provenance
                .as_ref()
                .map(|value| value.cache_observations.iter().any(|observation| {
                    observation.kind == CacheKind::PrefixCache
                        && observation.trigger == Some(CacheInvalidationTrigger::ExplicitReset)
                }))
                .unwrap_or(false)
        );

        let rehit = service.generate(
            &GenerationRequest::new_text(
                "gen-ref-prefix-control-5",
                service.model_descriptor().clone(),
                None,
                "hello",
                GenerationOptions::greedy(4),
            )
            .with_prefix_cache_control(PrefixCacheControl {
                mode: PrefixCacheMode::Auto,
                tenant_id: Some(tenant),
            }),
        )?;
        assert_eq!(
            rehit
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Hit)
        );
        assert_eq!(rehit.metrics.prefix_tokens_reused, Some(2));
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_refuses_prompt_context_overflow() {
        let mut service = CpuReferenceTextGenerationService::new().expect("service");
        let request = GenerationRequest::new_text(
            "gen-ref-context-refuse",
            service.model_descriptor().clone(),
            None,
            "hello world",
            GenerationOptions::greedy(6),
        );

        let error = service
            .generate(&request)
            .expect_err("context overflow should refuse");
        assert!(matches!(
            error,
            ReferenceTextGenerationError::ContextWindow(
                ContextWindowError::CannotTruncateFurther {
                    max_context_tokens: 8,
                    existing_context_tokens: 0,
                    reserved_output_tokens: 6,
                    input_prompt_tokens: 3,
                    available_prompt_tokens: 2,
                    policy: ContextOverflowPolicy::Refuse,
                }
            )
        ));
    }

    #[test]
    fn cpu_reference_text_generation_can_truncate_oldest_prompt_tokens()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_tokens(
            "gen-ref-context-truncate",
            service.model_descriptor().clone(),
            None,
            TokenSequence::new(vec![
                FixtureWordTokenizer::HELLO_ID,
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID,
            ]),
            GenerationOptions {
                max_output_tokens: 6,
                context_overflow_policy: ContextOverflowPolicy::TruncateOldest,
                ..GenerationOptions::greedy(6)
            },
        );

        let response = service.generate(&request)?;
        assert_eq!(response.usage.input_tokens, 2);
        assert_eq!(
            response
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.input_prompt_tokens),
            Some(3)
        );
        assert_eq!(
            response
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.retained_prompt_tokens),
            Some(2)
        );
        assert_eq!(
            response
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.truncated_prompt_tokens),
            Some(1)
        );
        assert_eq!(
            response
                .metrics
                .context_window
                .as_ref()
                .map(|value| value.preserved_prefix_tokens),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_session_budget_counts_existing_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
        service.generate(&GenerationRequest::new_text(
            "gen-ref-context-session-prime",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "hello",
            GenerationOptions::greedy(1),
        ))?;

        let request = GenerationRequest::new_text(
            "gen-ref-context-session-overflow",
            service.model_descriptor().clone(),
            Some(session.session_id),
            "hello",
            GenerationOptions::greedy(5),
        );
        let error = service
            .generate(&request)
            .expect_err("session context overflow");

        assert!(matches!(
            error,
            ReferenceTextGenerationError::ContextWindow(
                ContextWindowError::CannotTruncateFurther {
                    max_context_tokens: 8,
                    existing_context_tokens: 3,
                    reserved_output_tokens: 5,
                    input_prompt_tokens: 2,
                    available_prompt_tokens: 0,
                    policy: ContextOverflowPolicy::Refuse,
                }
            )
        ));
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_reuses_and_resets_sessions()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;

        let first_request = GenerationRequest::new_text(
            "gen-ref-session-1",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "hello",
            GenerationOptions::greedy(4),
        );
        let first = service.generate(&first_request)?;
        assert_eq!(first.output.text, "open agents");
        assert_eq!(first.usage.cache_tokens, 4);
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.pages),
            Some(1)
        );

        let second_request = GenerationRequest::new_text(
            "gen-ref-session-2",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "rusty",
            GenerationOptions::greedy(1),
        );
        let second = service.generate(&second_request)?;
        assert_eq!(second.output.text, "grad");
        assert!(second.usage.cache_tokens > first.usage.cache_tokens);
        assert_eq!(
            second
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.tokens),
            Some(3)
        );

        let reset_request = GenerationRequest::new_text(
            "gen-ref-session-3",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "rusty",
            GenerationOptions::greedy(1),
        )
        .with_reset_session(true);
        let reset = service.generate(&reset_request)?;
        assert_eq!(reset.output.text, "grad");
        assert_eq!(reset.usage.cache_tokens, 3);
        assert_eq!(
            reset
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.tokens),
            Some(3)
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_truncates_stop_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-stop",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions {
                stop_sequences: vec![String::from("agents")],
                ..GenerationOptions::greedy(4)
            },
        );

        let response = service.generate(&request)?;
        assert_eq!(response.output.text, "open");
        assert_eq!(
            response.output.tokens.as_slice(),
            &[FixtureWordTokenizer::OPEN_ID]
        );
        assert_eq!(response.termination, TerminationReason::EndOfSequence);
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_updates_loaded_model_residency()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let warmed = service.warm_model(ReferenceWordDecoder::MODEL_ID, 0)?;
        assert_eq!(warmed.residency.keep_alive_millis, 0);

        let request = GenerationRequest::new_text(
            "gen-ref-lifecycle",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(2),
        );
        let response = service.generate(&request)?;
        assert_eq!(response.output.text, "open agents");
        assert!(service.loaded_model_views().is_empty());
        assert!(service.loaded_models().models.is_empty());
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_rejects_unknown_session() {
        let mut service = CpuReferenceTextGenerationService::new().expect("service");
        let request = GenerationRequest::new_text(
            "gen-ref-bad-session",
            service.model_descriptor().clone(),
            Some(SessionId::new("sess-missing")),
            "hello",
            GenerationOptions::greedy(2),
        );

        let error = service.generate(&request).expect_err("missing session");
        assert!(matches!(
            error,
            ReferenceTextGenerationError::Session(super::SessionStoreError::SessionNotFound(_))
        ));
    }

    fn sample_decoder_descriptor() -> DecoderModelDescriptor {
        sample_named_decoder_descriptor("fixture-word-decoder-v0")
    }

    fn sample_named_decoder_descriptor(model_id: &str) -> DecoderModelDescriptor {
        DecoderModelDescriptor::new(
            ModelDescriptor::new(model_id, "fixture_decoder", "v0"),
            DecoderConfig {
                hidden_size: 4,
                layer_count: 1,
                vocab_size: 4,
                max_context: 6,
                block: DecoderBlockConfig {
                    attention: DecoderAttentionConfig {
                        head_count: 2,
                        kv_head_count: 2,
                        head_dim: 2,
                        rotary_dim: 0,
                    },
                    feed_forward: DecoderFeedForwardConfig {
                        intermediate_size: 8,
                        activation: ActivationFunction::Identity,
                    },
                },
            },
            "fixture_wordpiece",
            WeightBundleMetadata {
                format: WeightFormat::ProgrammaticFixture,
                source: WeightSource::Fixture,
                quantization: psionic_core::QuantizationMode::None,
                quantization_modes: Vec::new(),
                digest: String::from("fixture-digest"),
                tensors: vec![WeightTensorMetadata::new(
                    "lm_head",
                    Shape::new(vec![4, 4]),
                    DType::F32,
                )],
                artifacts: Vec::new(),
            },
        )
    }

    fn sample_adapter_serving_binding() -> AdapterServingBinding {
        let model = sample_decoder_descriptor();
        let base_served_artifact = served_artifact_identity_for_decoder_model(
            &model,
            &BackendSelection::direct("cpu", None, vec![]),
        );
        let base_served_artifact_digest = base_served_artifact.served_artifact_digest;
        AdapterServingBinding::new(
            "fixture-word-decoder-qna",
            model.model.model_id.clone(),
            model.model.revision.clone(),
            base_served_artifact_digest.clone(),
            AdapterResidencyMode::HotSwapOverlay,
            vec![AdapterArtifactIdentity::new(
                "adapter-qna",
                "r1",
                AdapterArtifactKind::Lora,
                AdapterArtifactFormat::Safetensors,
                model.model.model_id,
                model.model.revision,
                base_served_artifact_digest,
                "adapter-digest-qna",
                psionic_core::QuantizationMode::GgmlQ8_0,
                AdapterTargetFamily::DecoderAttention,
                1_024_000,
            )],
        )
    }

    fn sample_embedding_descriptor() -> psionic_models::EmbeddingModelDescriptor {
        SmokeByteEmbedder::new().descriptor().clone()
    }

    #[test]
    fn sharded_model_manifest_loader_round_trips_valid_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = ShardedModelManifest::new(
            "layer-manifest",
            served_artifact_identity_for_decoder_model(
                &sample_decoder_descriptor(),
                &BackendSelection::direct("cpu", None, vec![]),
            ),
            ShardedModelLayoutKind::LayerSharded,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "decoder.layers0_20",
            "layer-digest-0",
            ExecutionPartition::LayerRange {
                start_layer: 0,
                end_layer: 20,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "decoder.layers20_40",
            "layer-digest-1",
            ExecutionPartition::LayerRange {
                start_layer: 20,
                end_layer: 40,
            },
        ));
        let tempdir = tempdir()?;
        let manifest_path = tempdir.path().join("layer-manifest.json");
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

        let loaded = load_sharded_model_manifest_json(&manifest_path)?;

        assert_eq!(loaded, manifest);
        assert_eq!(loaded.stable_digest(), manifest.stable_digest());
        Ok(())
    }
}
