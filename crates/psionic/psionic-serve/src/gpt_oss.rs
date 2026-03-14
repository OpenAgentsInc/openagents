#![allow(
    clippy::arc_with_non_send_sync,
    clippy::assigning_clones,
    clippy::collapsible_else_if,
    clippy::expect_used,
    clippy::manual_is_multiple_of,
    clippy::needless_lifetimes,
    clippy::too_many_arguments,
    clippy::unnecessary_lazy_evaluations,
    clippy::useless_conversion
)]
#![cfg_attr(
    test,
    allow(
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::print_stdout,
        clippy::useless_vec
    )
)]

use std::{
    cmp::Ordering,
    env,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use psionic_backend_cpu::{
    CpuBackend, decode_quantized_row_into, quantized_row_byte_len, quantized_row_dot,
};
use psionic_backend_cuda::{
    CudaBackend, CudaBuffer, CudaGraphExec, CudaHostBuffer, CudaQuantizedMatvecResult,
    CudaQuantizedMatvecStats, CudaSubmission, CudaSubmissionReport,
    TEXT_GENERATION_SUPPORTED_OPS as CUDA_TEXT_GENERATION_SUPPORTED_OPS, ggml_q8_1_storage_bytes,
};
use psionic_backend_metal::{
    MetalAttentionGraphReserve, MetalAttentionGraphRuntime, MetalBackend, MetalBuffer,
    MetalGraphReserveKind, MetalGroupedExpertMatvecResult, MetalKvCacheMirror,
    MetalLogitsOutputMode, MetalPromptResidencyMetrics, MetalSharedPrefixCompatibility,
    MetalSharedPrefixStore, MetalTopKResult,
    TEXT_GENERATION_SUPPORTED_OPS as METAL_TEXT_GENERATION_SUPPORTED_OPS,
};
use psionic_catalog::{BlobIntegrityPolicy, LocalBlobOpenOptions};
use psionic_core::Shape;
use psionic_models::{GgufBlobArtifact, GptOssTokenizer, PagedTensorStorage};
use psionic_runtime::{
    CacheAction, CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature,
    DeviceDiscovery, GptOssDecodeGraph, HealthStatus, KvCachePageLayout, KvCachePolicy,
    PrefixCacheIdentity, build_gpt_oss_decode_graph,
};
use sha2::{Digest, Sha256};

use super::{
    BackendHealthTracker, CompiledWordGenerationModel, DecodeStrategy, DecoderModelDescriptor,
    GenerationEventStream, GenerationModelHandle, GenerationOptions, GenerationResponse,
    GenerationStreamEvent, GenerationStreamStatus, GenerationStreamTerminal,
    GgufDecoderAdapterLoader, GgufDecoderFamily, GgufDecoderFamilyMetadata,
    GgufDecoderLayerTensorLayout, GptOssMetalDecodeLogitsMetrics, GptOssMetalLogitsOutputMode,
    GptOssPerformanceMetrics, InMemoryGenerationModelRegistry, InMemoryGenerationSessionStore,
    LoadedModelRegistryError, LoadedModelView, LocalRuntimeObservability,
    ManagedTextGenerationRuntime, ModelLoadError, QuantizationMode, ReferenceTextGenerationError,
    SharedPrefixStore, TextGenerationExecutor, TokenId, TokenSequence, TokenizerBoundary,
    current_time_millis, default_prefix_cache_policy, generation_runtime_observability,
    prefix_compatibility, run_generation_request,
};
use thiserror::Error;

const GPT_OSS_OAI_SWIGLU_ALPHA: f32 = 1.702;
const GPT_OSS_OAI_SWIGLU_LIMIT: f32 = 7.0;
const GPT_OSS_YARN_BETA_FAST: f32 = 32.0;
const GPT_OSS_YARN_BETA_SLOW: f32 = 1.0;
const GPT_OSS_CPU_BACKEND: &str = "cpu";
const GPT_OSS_CUDA_BACKEND: &str = "cuda";
const GPT_OSS_CUDA_HYBRID_MOE_BACKEND: &str = "cuda+host-moe";
const GPT_OSS_METAL_BACKEND: &str = "metal";

fn gpt_oss_local_blob_open_options() -> LocalBlobOpenOptions {
    LocalBlobOpenOptions::default().with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel)
}

fn decode_graph_fast_path_enabled() -> bool {
    env::var("PSIONIC_GPT_OSS_DISABLE_CUDA_GRAPHS")
        .map(|value| value != "1")
        .unwrap_or(true)
}

fn experimental_fused_selected4_moe_down_enabled() -> bool {
    env::var("PSIONIC_GPT_OSS_EXPERIMENTAL_FUSED_SELECTED4_MOE_DOWN")
        .map(|value| value == "1")
        .unwrap_or(false)
}

const HYBRID_SELECTED4_LAYER_CACHE_SLOTS: usize = 5;
const HYBRID_SELECTED4_LAYER_CACHE_REDUCED_SLOTS: usize = 4;
const HYBRID_SELECTED4_LAYER_CACHE_EXPANDED_SLOTS: usize = 6;
const HYBRID_SELECTED4_LAYER_CACHE_HOT_SLOTS: usize = 8;
const HYBRID_SELECTED4_LAYER_CACHE_MAX_SLOTS: usize = HYBRID_SELECTED4_LAYER_CACHE_HOT_SLOTS;
const HYBRID_SELECTED4_LAYER_CACHE_EXPANDED_TAIL_LAYERS: usize = 15;
const HYBRID_SELECTED4_LAYER_CACHE_PROFILED_EXPANDED_LAYERS_120B: &[usize] =
    &[10, 18, 21, 22, 26, 31, 33];
const HYBRID_SELECTED4_LAYER_CACHE_HOT_LAYERS_120B: &[usize] = &[23, 25, 28, 29];
const HYBRID_SELECTED4_LAYER_CACHE_REDUCED_LAYERS_120B: &[usize] = &[14, 15, 19, 20, 32];
const HYBRID_SELECTED4_LAYER_CACHE_TAIL_RESTORE_LAYERS_120B: &[usize] = &[35];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CudaStepOutputMode {
    FullLogits,
    DeviceArgmax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetalStepOutputMode {
    SkipLogits,
    Logits(MetalLogitsOutputMode),
}

fn duration_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

fn initial_cuda_argmax_pair_bytes() -> [u8; std::mem::size_of::<u64>()] {
    let packed = (u64::from(i32::MAX as u32) << 32) | u64::from(f32::NEG_INFINITY.to_bits());
    packed.to_ne_bytes()
}

fn cuda_argmax_token_id(token: i32) -> Result<TokenId, ReferenceTextGenerationError> {
    u32::try_from(token).map(TokenId).map_err(|_| {
        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
            "cuda argmax returned a negative token id {token}",
        )))
    })
}

fn cuda_argmax_token_from_packed_host_buffer(
    host_buffer: &CudaHostBuffer,
) -> Result<TokenId, ReferenceTextGenerationError> {
    let bytes = host_buffer.read_bytes().map_err(|error| {
        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
            "cuda argmax returned an invalid packed host buffer: {error}",
        )))
    })?;
    let packed = u64::from_ne_bytes(bytes[..std::mem::size_of::<u64>()].try_into().map_err(
        |_| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda argmax returned invalid packed argmax bytes",
            )))
        },
    )?);
    cuda_argmax_token_id((packed >> 32) as i32)
}

fn can_use_cuda_argmax_fast_path(options: &GenerationOptions) -> bool {
    options.decode_strategy == DecodeStrategy::Greedy
        && options.repeat_penalty.is_none()
        && options.presence_penalty.is_none()
        && options.frequency_penalty.is_none()
}

fn can_use_cached_prompt_argmax(options: &GenerationOptions) -> bool {
    can_use_cuda_argmax_fast_path(options)
}

fn has_sampling_penalties(options: &GenerationOptions) -> bool {
    options.repeat_penalty.is_some()
        || options.presence_penalty.is_some()
        || options.frequency_penalty.is_some()
}

fn hybrid_selected4_layer_cache_slots_for_model(layer_index: usize, layer_count: usize) -> usize {
    if layer_count == 36 {
        if HYBRID_SELECTED4_LAYER_CACHE_HOT_LAYERS_120B.contains(&layer_index) {
            HYBRID_SELECTED4_LAYER_CACHE_HOT_SLOTS
        } else if HYBRID_SELECTED4_LAYER_CACHE_TAIL_RESTORE_LAYERS_120B.contains(&layer_index) {
            HYBRID_SELECTED4_LAYER_CACHE_SLOTS
        } else if HYBRID_SELECTED4_LAYER_CACHE_PROFILED_EXPANDED_LAYERS_120B.contains(&layer_index)
        {
            HYBRID_SELECTED4_LAYER_CACHE_EXPANDED_SLOTS
        } else if HYBRID_SELECTED4_LAYER_CACHE_REDUCED_LAYERS_120B.contains(&layer_index) {
            HYBRID_SELECTED4_LAYER_CACHE_REDUCED_SLOTS
        } else {
            HYBRID_SELECTED4_LAYER_CACHE_SLOTS
        }
    } else if layer_index
        >= layer_count.saturating_sub(HYBRID_SELECTED4_LAYER_CACHE_EXPANDED_TAIL_LAYERS)
    {
        HYBRID_SELECTED4_LAYER_CACHE_EXPANDED_SLOTS
    } else {
        HYBRID_SELECTED4_LAYER_CACHE_SLOTS
    }
}

fn can_use_metal_greedy_logits_output(options: &GenerationOptions) -> bool {
    !has_sampling_penalties(options)
        && (options.decode_strategy == DecodeStrategy::Greedy
            || options.sampling_policy().effective_temperature() <= 1e-6)
}

fn metal_decode_logits_output_mode(options: &GenerationOptions) -> MetalLogitsOutputMode {
    if can_use_metal_greedy_logits_output(options) {
        return MetalLogitsOutputMode::GreedyToken;
    }
    if has_sampling_penalties(options) {
        return MetalLogitsOutputMode::RawLogits;
    }
    match options.sampling_policy().effective_top_k() {
        Some(1) => MetalLogitsOutputMode::GreedyToken,
        Some(top_k) if top_k > 1 => MetalLogitsOutputMode::TopKCandidates(top_k),
        _ => MetalLogitsOutputMode::RawLogits,
    }
}

fn can_use_q8_1_mmvq(mode: QuantizationMode) -> bool {
    matches!(
        mode,
        QuantizationMode::GgmlQ8_0 | QuantizationMode::GgmlMxfp4
    )
}

fn can_use_q8_1_norm_fusion(element_count: usize) -> bool {
    element_count % 32 == 0
}

fn can_use_q8_1_attention_output_fusion(
    attention_output_columns: usize,
    head_count: usize,
    head_dim: usize,
) -> bool {
    head_dim % 32 == 0
        && attention_output_columns == head_count.saturating_mul(head_dim)
        && attention_output_columns % 32 == 0
}

fn can_use_hybrid_cuda_hidden_residency_layer(layer: &GptOssCudaLayer, hidden_size: usize) -> bool {
    can_use_q8_1_norm_fusion(hidden_size)
        && layer.attention_norm_device.is_some()
        && layer.attention_qkv_bias_device.is_some()
        && layer.attention_qkv_weight.storage.is_some()
        && layer.attention_output_weight.storage.is_some()
        && layer.feed_forward_norm_device.is_some()
        && layer.feed_forward_router_weight_transposed_device.is_some()
        && layer.feed_forward_gate_up_experts_weight.host.is_some()
        && layer.feed_forward_down_experts_weight.host.is_some()
}

fn gpt_oss_metal_logits_output_mode(
    output_mode: MetalLogitsOutputMode,
) -> GptOssMetalLogitsOutputMode {
    match output_mode {
        MetalLogitsOutputMode::GreedyToken => GptOssMetalLogitsOutputMode::GreedyToken,
        MetalLogitsOutputMode::TopKCandidates(top_k) => {
            GptOssMetalLogitsOutputMode::TopKCandidates { top_k }
        }
        MetalLogitsOutputMode::RawLogits => GptOssMetalLogitsOutputMode::RawLogits,
    }
}

fn accumulate_metal_decode_logits_metrics(
    perf: &mut GptOssPerformanceMetrics,
    output_mode: MetalLogitsOutputMode,
    readback_bytes: u64,
    raw_logits_materialized: bool,
) {
    let metrics = perf
        .metal_decode_logits
        .get_or_insert_with(GptOssMetalDecodeLogitsMetrics::default);
    metrics.step_count = metrics.step_count.saturating_add(1);
    metrics.readback_bytes = metrics.readback_bytes.saturating_add(readback_bytes);
    metrics.raw_logits_materialized |= raw_logits_materialized;
    metrics
        .output_modes
        .push(gpt_oss_metal_logits_output_mode(output_mode));
    metrics.output_modes.sort();
    metrics.output_modes.dedup();
}

fn expand_metal_top_k_candidates_to_logits(
    vocab_size: usize,
    candidates: &MetalTopKResult,
) -> Result<Vec<f32>, ReferenceTextGenerationError> {
    if candidates.row_count != 1 {
        return Err(ReferenceTextGenerationError::Runtime(
            super::RuntimeError::Backend(format!(
                "metal logits top-k row count mismatch: expected 1, actual {}",
                candidates.row_count
            )),
        ));
    }
    if candidates.indices.len() != candidates.values.len() {
        return Err(ReferenceTextGenerationError::Runtime(
            super::RuntimeError::Backend(format!(
                "metal logits top-k shape mismatch: indices {}, values {}",
                candidates.indices.len(),
                candidates.values.len()
            )),
        ));
    }

    let mut logits = vec![f32::NEG_INFINITY; vocab_size];
    for (index, value) in candidates
        .indices
        .iter()
        .copied()
        .zip(candidates.values.iter().copied())
    {
        let token_index = usize::try_from(index).map_err(|_| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                "metal logits top-k token index conversion overflow: {index}",
            )))
        })?;
        if token_index >= vocab_size {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(format!(
                    "metal logits top-k token index out of bounds: index {token_index}, vocab {vocab_size}",
                )),
            ));
        }
        logits[token_index] = value;
    }
    Ok(logits)
}

fn accumulate_cuda_matvec_stats(
    perf: &mut GptOssPerformanceMetrics,
    stats: &CudaQuantizedMatvecStats,
) {
    perf.cuda.host_to_device_bytes = perf
        .cuda
        .host_to_device_bytes
        .saturating_add(stats.host_to_device_bytes);
    perf.cuda.device_to_host_bytes = perf
        .cuda
        .device_to_host_bytes
        .saturating_add(stats.device_to_host_bytes);
    perf.cuda.submission_count = perf
        .cuda
        .submission_count
        .saturating_add(stats.submission_count);
    perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(stats.sync_count);
    perf.cuda.kernel_launches = perf
        .cuda
        .kernel_launches
        .saturating_add(stats.kernel_launches);
}

fn accumulate_cuda_submission_report(
    perf: &mut GptOssPerformanceMetrics,
    report: &CudaSubmissionReport,
    host_to_device_bytes: u64,
    device_to_host_bytes: u64,
) {
    perf.cuda.host_to_device_bytes = perf
        .cuda
        .host_to_device_bytes
        .saturating_add(host_to_device_bytes);
    perf.cuda.device_to_host_bytes = perf
        .cuda
        .device_to_host_bytes
        .saturating_add(device_to_host_bytes);
    perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
    perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
    perf.cuda.kernel_launches = perf
        .cuda
        .kernel_launches
        .saturating_add(report.encoded_operations);
}

fn accumulate_metal_submission_report(
    perf: &mut GptOssPerformanceMetrics,
    report: &psionic_backend_metal::MetalSubmissionReport,
) {
    perf.metal.submission_count = perf.metal.submission_count.saturating_add(1);
    perf.metal.sync_count = perf
        .metal
        .sync_count
        .saturating_add(report.synchronized_buffers);
    perf.metal.kernel_launches = perf
        .metal
        .kernel_launches
        .saturating_add(report.encoded_operations);
}

fn accumulate_metal_host_to_device_bytes(perf: &mut GptOssPerformanceMetrics, byte_len: usize) {
    perf.metal.host_to_device_bytes = perf
        .metal
        .host_to_device_bytes
        .saturating_add(byte_len.try_into().unwrap_or(u64::MAX));
}

fn accumulate_metal_device_to_host_bytes(perf: &mut GptOssPerformanceMetrics, byte_len: usize) {
    perf.metal.device_to_host_bytes = perf
        .metal
        .device_to_host_bytes
        .saturating_add(byte_len.try_into().unwrap_or(u64::MAX));
}

/// CPU-backed real GGUF GPT-OSS text-generation service.
#[derive(Clone, Debug)]
pub struct CpuGgufGptOssTextGenerationService {
    backend: CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuGgufGptOssGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl CpuGgufGptOssTextGenerationService {
    /// Loads a real GGUF-backed GPT-OSS model from a local artifact path.
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let backend = CpuBackend::new();
        let model = CpuGgufGptOssGenerationModel::from_gguf_path(path)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            model,
            current_time_millis(),
            super::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from(GPT_OSS_CPU_BACKEND)),
            None,
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe(GPT_OSS_CPU_BACKEND, backend.health(), current_time_millis());
        Ok(Self {
            backend,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    /// Loads or replaces a GGUF-backed GPT-OSS model.
    pub fn load_model(
        &mut self,
        model: CpuGgufGptOssGenerationModel,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.model_descriptor = model.descriptor().clone();
        self.models.warm_with_metadata(
            model,
            current_time_millis(),
            super::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from(GPT_OSS_CPU_BACKEND)),
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
            .map(CpuGgufGptOssGenerationModel::plan_digest)
    }

    /// Refreshes keepalive for an already-loaded model.
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
    pub fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> super::LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns runtime observability at a caller-provided time.
    #[must_use]
    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe(GPT_OSS_CPU_BACKEND, self.backend.health(), now_millis);
        generation_runtime_observability(&self.models, &self.sessions, &self.backend_health)
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
    ) -> Result<super::GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(
            model,
            super::served_artifact_identity_for_decoder_backend(
                model.descriptor(),
                GPT_OSS_CPU_BACKEND,
                &[],
            )
            .served_artifact_digest,
        ))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &super::SessionId,
    ) -> Result<super::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &super::SessionId,
    ) -> Result<super::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }
}

impl TextGenerationExecutor for CpuGgufGptOssTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(
        &mut self,
        request: &super::GenerationRequest,
    ) -> Result<super::GenerationResponse, Self::Error> {
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )
    }
}

impl ManagedTextGenerationRuntime for CpuGgufGptOssTextGenerationService {
    fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        CpuGgufGptOssTextGenerationService::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        CpuGgufGptOssTextGenerationService::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuGgufGptOssTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        CpuGgufGptOssTextGenerationService::unload_model(self, model_id)
    }
}

impl super::StreamingTextGenerationExecutor for CpuGgufGptOssTextGenerationService {
    type Stream<'a> = Box<dyn super::GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &super::GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error> {
        let response = self.generate(request)?;
        Ok(Box::new(OneShotGenerationStream::new(response)))
    }
}

struct OneShotGenerationStream {
    streaming_policy: super::GenerationStreamingPolicy,
    terminal: Option<GenerationStreamTerminal>,
}

impl OneShotGenerationStream {
    fn new(response: GenerationResponse) -> Self {
        Self {
            streaming_policy: super::default_generation_streaming_policy(),
            terminal: Some(GenerationStreamTerminal {
                status: GenerationStreamStatus::Succeeded,
                response,
                failure_reason: None,
                diagnostic: None,
            }),
        }
    }
}

impl GenerationEventStream for OneShotGenerationStream {
    fn policy(&self) -> &super::GenerationStreamingPolicy {
        &self.streaming_policy
    }

    fn next_event(&mut self) -> Option<GenerationStreamEvent> {
        self.terminal.take().map(GenerationStreamEvent::Terminal)
    }

    fn cancel(&mut self) -> Option<GenerationStreamTerminal> {
        self.terminal.take()
    }

    fn disconnect(&mut self) -> Option<GenerationStreamTerminal> {
        self.terminal.take()
    }
}

#[derive(Debug, Error)]
pub enum MetalGptOssTextGenerationError {
    #[error("metal backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        status: HealthStatus,
        message: String,
    },
    #[error(transparent)]
    Generation(#[from] ReferenceTextGenerationError),
}

#[derive(Clone, Debug)]
struct MetalLayerPrefixLookup {
    state: super::PrefixCacheState,
    reused_tokens: usize,
    identity: Option<PrefixCacheIdentity>,
    caches: Option<Vec<MetalKvCacheMirror>>,
}

#[derive(Clone, Debug, Default)]
struct MetalLayerSharedPrefixStore {
    stores: Vec<MetalSharedPrefixStore>,
}

impl MetalLayerSharedPrefixStore {
    fn ensure_layer_count(&mut self, layer_count: usize) {
        if self.stores.len() < layer_count {
            self.stores
                .resize_with(layer_count, MetalSharedPrefixStore::default);
        }
    }

    fn lookup(
        &mut self,
        compatibility: &MetalSharedPrefixCompatibility,
        prompt_tokens: &[u32],
        layer_count: usize,
    ) -> MetalLayerPrefixLookup {
        self.ensure_layer_count(layer_count);
        if layer_count == 0 {
            return MetalLayerPrefixLookup {
                state: super::PrefixCacheState::None,
                reused_tokens: 0,
                identity: None,
                caches: Some(Vec::new()),
            };
        }

        let lookups = self
            .stores
            .iter_mut()
            .take(layer_count)
            .map(|store| store.lookup(compatibility, prompt_tokens))
            .collect::<Vec<_>>();
        if lookups
            .iter()
            .all(|lookup| lookup.state == super::PrefixCacheState::Hit)
        {
            let reused_tokens = lookups.first().map_or(0, |lookup| lookup.reused_tokens);
            let identity = lookups.iter().find_map(|lookup| lookup.identity.clone());
            let caches = lookups
                .into_iter()
                .map(|lookup| {
                    lookup.cache.ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "metal shared-prefix lookup reported a hit without a cache",
                            ),
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>();
            return match caches {
                Ok(caches) => MetalLayerPrefixLookup {
                    state: super::PrefixCacheState::Hit,
                    reused_tokens,
                    identity,
                    caches: Some(caches),
                },
                Err(_) => MetalLayerPrefixLookup {
                    state: super::PrefixCacheState::Rebuilt,
                    reused_tokens: 0,
                    identity: None,
                    caches: None,
                },
            };
        }

        let state = if lookups
            .iter()
            .any(|lookup| lookup.state == super::PrefixCacheState::Rebuilt)
        {
            super::PrefixCacheState::Rebuilt
        } else if lookups
            .iter()
            .any(|lookup| lookup.state == super::PrefixCacheState::Miss)
        {
            super::PrefixCacheState::Miss
        } else {
            super::PrefixCacheState::None
        };
        MetalLayerPrefixLookup {
            state,
            reused_tokens: 0,
            identity: None,
            caches: None,
        }
    }

    fn record(
        &mut self,
        compatibility: &MetalSharedPrefixCompatibility,
        prompt_tokens: &[u32],
        caches: &[MetalKvCacheMirror],
    ) {
        self.ensure_layer_count(caches.len());
        for (store, cache) in self.stores.iter_mut().zip(caches.iter()) {
            store.record(compatibility.clone(), prompt_tokens, cache);
        }
    }

    fn clear(&mut self) {
        for store in &mut self.stores {
            store.clear();
        }
    }
}

#[derive(Clone, Debug)]
struct MetalPromptPrefixExactLookup {
    identity: PrefixCacheIdentity,
    last_logits: Vec<f32>,
    greedy_token: Option<u32>,
}

#[derive(Clone, Debug)]
struct MetalPromptPrefixEntry {
    compatibility: super::SharedPrefixCompatibility,
    prompt_tokens: TokenSequence,
    last_prompt_logits: Vec<f32>,
    greedy_prompt_token: Option<u32>,
}

#[derive(Clone, Debug, Default)]
struct MetalPromptPrefixStore {
    entries: Vec<MetalPromptPrefixEntry>,
}

impl MetalPromptPrefixStore {
    fn lookup_exact_prompt(
        &self,
        compatibility: &super::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<MetalPromptPrefixExactLookup> {
        self.entries
            .iter()
            .find(|entry| {
                &entry.compatibility == compatibility
                    && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
                    && !entry.last_prompt_logits.is_empty()
            })
            .map(|entry| MetalPromptPrefixExactLookup {
                identity: super::prefix_identity(compatibility, prompt_tokens.as_slice()),
                last_logits: entry.last_prompt_logits.clone(),
                greedy_token: entry.greedy_prompt_token,
            })
    }

    fn record(
        &mut self,
        compatibility: super::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        last_prompt_logits: &[f32],
    ) -> PrefixCacheIdentity {
        let identity = super::prefix_identity(&compatibility, prompt_tokens.as_slice());
        let greedy_prompt_token = super::select_argmax_token(last_prompt_logits);
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.compatibility == compatibility
                && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
        }) {
            existing.last_prompt_logits = last_prompt_logits.to_vec();
            existing.greedy_prompt_token = greedy_prompt_token;
        } else {
            self.entries.push(MetalPromptPrefixEntry {
                compatibility,
                prompt_tokens: prompt_tokens.clone(),
                last_prompt_logits: last_prompt_logits.to_vec(),
                greedy_prompt_token,
            });
        }
        identity
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

pub struct MetalGgufGptOssTextGenerationService {
    backend: MetalBackend,
    backend_selection: psionic_runtime::BackendSelection,
    models: InMemoryGenerationModelRegistry<MetalGgufGptOssGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    metal_shared_prefixes: MetalLayerSharedPrefixStore,
    metal_prompt_prefixes: MetalPromptPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl MetalGgufGptOssTextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, MetalGptOssTextGenerationError> {
        let mut backend = MetalBackend::new();
        let runtime = backend
            .configure_text_generation_runtime(
                psionic_backend_metal::MetalTextGenerationRuntimePolicy::gpt_oss_default(),
            )
            .map_err(|error| MetalGptOssTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        if !runtime.admission.admitted {
            return Err(MetalGptOssTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: runtime.admission.refusal_reason.unwrap_or_else(|| {
                    String::from("metal token-generation runtime admission refused")
                }),
            });
        }
        let backend_selection = backend
            .backend_selection(METAL_TEXT_GENERATION_SUPPORTED_OPS)
            .map_err(|error| MetalGptOssTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let model = MetalGgufGptOssGenerationModel::from_gguf_path(path, &mut backend)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            model,
            current_time_millis(),
            super::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from(GPT_OSS_METAL_BACKEND)),
            super::backend_selection_fallback_state(&backend_selection),
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe(
            GPT_OSS_METAL_BACKEND,
            backend.health(),
            current_time_millis(),
        );
        Ok(Self {
            backend,
            backend_selection,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            metal_shared_prefixes: MetalLayerSharedPrefixStore::default(),
            metal_prompt_prefixes: MetalPromptPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    #[must_use]
    pub fn backend_selection(&self) -> &psionic_runtime::BackendSelection {
        &self.backend_selection
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(MetalGgufGptOssGenerationModel::plan_digest)
    }

    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<super::GenerationSession, MetalGptOssTextGenerationError> {
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
            super::served_artifact_identity_for_decoder_backend(
                model.descriptor(),
                self.backend_selection.effective_backend.as_str(),
                &compiled_backend_features,
            )
            .served_artifact_digest,
        ))
    }

    pub fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    pub fn loaded_models_at(&mut self, now_millis: u64) -> super::LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(current_time_millis())
    }

    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe(GPT_OSS_METAL_BACKEND, self.backend.health(), now_millis);
        generation_runtime_observability(&self.models, &self.sessions, &self.backend_health)
    }

    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, MetalGptOssTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, MetalGptOssTextGenerationError> {
        self.metal_shared_prefixes.clear();
        self.metal_prompt_prefixes.clear();
        Ok(self.models.unload_view(model_id, current_time_millis())?)
    }
}

impl TextGenerationExecutor for MetalGgufGptOssTextGenerationService {
    type Error = MetalGptOssTextGenerationError;

    fn generate(
        &mut self,
        request: &super::GenerationRequest,
    ) -> Result<super::GenerationResponse, Self::Error> {
        run_metal_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            &mut self.metal_shared_prefixes,
            &mut self.metal_prompt_prefixes,
            request,
        )
        .map_err(Into::into)
    }
}

impl ManagedTextGenerationRuntime for MetalGgufGptOssTextGenerationService {
    fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        MetalGgufGptOssTextGenerationService::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        MetalGgufGptOssTextGenerationService::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        MetalGgufGptOssTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        self.metal_shared_prefixes.clear();
        self.metal_prompt_prefixes.clear();
        MetalGgufGptOssTextGenerationService::unload_model(self, model_id)
    }
}

impl super::StreamingTextGenerationExecutor for MetalGgufGptOssTextGenerationService {
    type Stream<'a> = Box<dyn super::GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &super::GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error> {
        let response = self.generate(request)?;
        Ok(Box::new(OneShotGenerationStream::new(response)))
    }
}

#[derive(Debug, Error)]
pub enum CudaGptOssTextGenerationError {
    #[error("cuda backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        status: HealthStatus,
        message: String,
    },
    #[error(transparent)]
    Generation(#[from] ReferenceTextGenerationError),
}

#[derive(Clone, Debug)]
struct CudaSharedPrefixEntry {
    compatibility: super::SharedPrefixCompatibility,
    prompt_tokens: TokenSequence,
    cache: CudaKvCacheMirror,
}

#[derive(Clone, Debug, Default)]
struct CudaSharedPrefixStore {
    entries: Vec<CudaSharedPrefixEntry>,
}

impl CudaSharedPrefixStore {
    fn lookup_exact_prompt(
        &self,
        compatibility: &super::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<CudaKvCacheMirror> {
        self.entries
            .iter()
            .find(|entry| {
                &entry.compatibility == compatibility
                    && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
                    && entry.cache.len() >= prompt_tokens.len()
            })
            .map(|entry| entry.cache.truncated(prompt_tokens.len()))
    }

    fn lookup(
        &mut self,
        compatibility: &super::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<CudaKvCacheMirror> {
        let compatible_indices: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (&entry.compatibility == compatibility).then_some(index))
            .collect();
        if compatible_indices.is_empty() {
            return None;
        }

        let mut best: Option<(usize, usize)> = None;
        let mut stale_prefix = false;
        for index in compatible_indices {
            let entry = &self.entries[index];
            let shared =
                super::shared_prefix_len(entry.prompt_tokens.as_slice(), prompt_tokens.as_slice());
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
            return Some(self.entries[index].cache.truncated(shared));
        }

        if stale_prefix {
            self.entries.retain(|entry| {
                !(&entry.compatibility == compatibility
                    && entry.cache.len() < entry.prompt_tokens.len())
            });
        }
        None
    }

    fn record(
        &mut self,
        compatibility: super::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        cache: &CudaKvCacheMirror,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.compatibility == compatibility
                && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
        }) {
            existing.cache = cache.clone();
        } else {
            self.entries.push(CudaSharedPrefixEntry {
                compatibility,
                prompt_tokens: prompt_tokens.clone(),
                cache: cache.clone(),
            });
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

pub struct CudaGgufGptOssTextGenerationService {
    backend: CudaBackend,
    backend_selection: psionic_runtime::BackendSelection,
    models: InMemoryGenerationModelRegistry<CudaGgufGptOssGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    cuda_shared_prefixes: CudaSharedPrefixStore,
    backend_health: BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
}

impl CudaGgufGptOssTextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, CudaGptOssTextGenerationError> {
        let mut backend = CudaBackend::new();
        if !backend.quantized_kernels_available() {
            return Err(CudaGptOssTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                ),
            });
        }
        let backend_selection = backend
            .backend_selection(CUDA_TEXT_GENERATION_SUPPORTED_OPS)
            .map_err(|error| CudaGptOssTextGenerationError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let model = CudaGgufGptOssGenerationModel::from_gguf_path(path, &mut backend)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            model,
            current_time_millis(),
            super::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from(GPT_OSS_CUDA_BACKEND)),
            super::backend_selection_fallback_state(&backend_selection),
        )?;
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe(
            GPT_OSS_CUDA_BACKEND,
            backend.health(),
            current_time_millis(),
        );
        Ok(Self {
            backend,
            backend_selection,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            cuda_shared_prefixes: CudaSharedPrefixStore::default(),
            backend_health,
            model_descriptor,
        })
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    #[must_use]
    pub fn backend_selection(&self) -> &psionic_runtime::BackendSelection {
        &self.backend_selection
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CudaGgufGptOssGenerationModel::plan_digest)
    }

    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<super::GenerationSession, CudaGptOssTextGenerationError> {
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
            super::served_artifact_identity_for_decoder_backend(
                model.descriptor(),
                self.backend_selection.effective_backend.as_str(),
                &compiled_backend_features,
            )
            .served_artifact_digest,
        ))
    }

    pub fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    pub fn loaded_models_at(&mut self, now_millis: u64) -> super::LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(current_time_millis())
    }

    pub fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe(GPT_OSS_CUDA_BACKEND, self.backend.health(), now_millis);
        generation_runtime_observability(&self.models, &self.sessions, &self.backend_health)
    }
}

impl TextGenerationExecutor for CudaGgufGptOssTextGenerationService {
    type Error = CudaGptOssTextGenerationError;

    fn generate(
        &mut self,
        request: &super::GenerationRequest,
    ) -> Result<super::GenerationResponse, Self::Error> {
        run_cuda_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            &mut self.cuda_shared_prefixes,
            request,
        )
        .map_err(Into::into)
    }
}

fn ensure_cuda_decode_step_plan(
    loaded_model: &CudaGgufGptOssGenerationModel,
    backend: &mut CudaBackend,
    decode_step_plan: &mut Option<GptOssCudaStepPlan>,
    execution_plan_digest: &mut Option<String>,
    compile_path: &mut Option<CompilePathEvidence>,
    plan_cache_hits: &mut usize,
    plan_cache_misses: &mut usize,
) -> Result<(), ReferenceTextGenerationError> {
    if decode_step_plan.is_some() {
        return Ok(());
    }
    let (plan, plan_compile_path, cache_hit) =
        loaded_model.inner.acquire_decode_step_plan(backend)?;
    if execution_plan_digest.is_none() {
        *execution_plan_digest = Some(plan.digest.clone());
    }
    let _ = compile_path;
    let _ = plan_compile_path;
    if cache_hit {
        *plan_cache_hits = plan_cache_hits.saturating_add(1);
    } else {
        *plan_cache_misses = plan_cache_misses.saturating_add(1);
    }
    *decode_step_plan = Some(plan);
    Ok(())
}

fn ensure_metal_decode_step_plan(
    loaded_model: &MetalGgufGptOssGenerationModel,
    backend: &mut MetalBackend,
    decode_step_plan: &mut Option<GptOssMetalStepPlan>,
    execution_plan_digest: &mut Option<String>,
    compile_path: &mut Option<CompilePathEvidence>,
    plan_cache_hits: &mut usize,
    plan_cache_misses: &mut usize,
) -> Result<(), ReferenceTextGenerationError> {
    if decode_step_plan.is_some() {
        return Ok(());
    }
    let (plan, plan_compile_path, cache_hit) =
        loaded_model.inner.acquire_decode_step_plan(backend)?;
    if execution_plan_digest.is_none() {
        *execution_plan_digest = Some(plan.digest.clone());
    }
    if compile_path.is_none() {
        *compile_path = Some(plan_compile_path);
    }
    if cache_hit {
        *plan_cache_hits = plan_cache_hits.saturating_add(1);
    } else {
        *plan_cache_misses = plan_cache_misses.saturating_add(1);
    }
    *decode_step_plan = Some(plan);
    Ok(())
}

fn run_cuda_generation_request(
    backend: &mut CudaBackend,
    models: &mut InMemoryGenerationModelRegistry<CudaGgufGptOssGenerationModel>,
    sessions: &mut InMemoryGenerationSessionStore,
    shared_prefixes: &mut SharedPrefixStore,
    cuda_shared_prefixes: &mut CudaSharedPrefixStore,
    request: &super::GenerationRequest,
) -> Result<GenerationResponse, ReferenceTextGenerationError> {
    if request.product_id != super::TEXT_GENERATION_PRODUCT_ID {
        return Err(ReferenceTextGenerationError::UnsupportedProduct(
            request.product_id.clone(),
        ));
    }

    if models
        .active(request.model.model.model_id.as_str())
        .map(|model| !model.inner.supports_cuda_decode_plan())
        .unwrap_or(false)
    {
        let _ = cuda_shared_prefixes;
        return run_cuda_hybrid_generation_request(
            backend,
            models,
            sessions,
            shared_prefixes,
            request,
        );
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
        .unwrap_or(super::GenerationLoadState::Warm);
    let request_start = current_time_millis();
    let generation_start = Instant::now();
    models.begin_request(model_id, request_start)?;
    let memory_plan = models.memory_plan(model_id).cloned();
    let residency_policy = Some(models.residency_policy().clone());
    let residency_snapshot = Some(models.memory_snapshot());
    let served_artifact = super::served_artifact_identity_for_decoder_backend(
        loaded_model.descriptor(),
        loaded_model.backend_compatibility(),
        &[],
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
        let compatibility = prefix_compatibility(&loaded_model);
        let prefix_policy = default_prefix_cache_policy();
        let mut prefix_state = super::PrefixCacheState::None;
        let mut prefix_tokens_reused = 0usize;
        let mut prefix_identity = None;
        let mut shared_prefix_eligible = false;
        let previous_kv_state = if let Some(session_id) = &request.session_id {
            if request.reset_session {
                sessions.reset(session_id)?;
            }
            let state = sessions.state(session_id)?;
            super::validate_session_model(
                state,
                session_id,
                loaded_model.descriptor(),
                served_artifact.served_artifact_digest.as_str(),
            )?;
            session_tokens = state.tokens().to_vec();
            if state.cache().is_empty() {
                shared_prefix_eligible = true;
            } else {
                prefix_state = super::PrefixCacheState::Bypassed;
            }
            state.cache().state()
        } else {
            shared_prefix_eligible = true;
            psionic_runtime::KvCacheState::default()
        };
        let preserve_prefix_tokens = usize::from(
            prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
        );
        let (prompt_tokens, context_window) = super::apply_context_window(
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
        let mut decode_step_plan = None;
        let use_cuda_argmax_fast_path = can_use_cuda_argmax_fast_path(&request.options);
        let mut exact_prompt_token = None;
        let exact_prefix_hit = if shared_prefix_eligible && request.session_id.is_none() {
            shared_prefixes.lookup_exact_prompt(&compatibility, &prompt_tokens)
        } else {
            None
        };
        let exact_cuda_cache = if exact_prefix_hit.is_some() {
            cuda_shared_prefixes.lookup_exact_prompt(&compatibility, &prompt_tokens)
        } else {
            None
        };
        let exact_prompt_cache_hit = exact_prefix_hit.is_some() && exact_cuda_cache.is_some();
        let mut cache = if let Some(hit) = exact_prefix_hit.filter(|_| exact_prompt_cache_hit) {
            prefix_state = super::PrefixCacheState::Hit;
            prefix_tokens_reused = prompt_tokens.len();
            prefix_identity = Some(hit.identity);
            last_logits = hit.last_logits;
            exact_prompt_token = hit.greedy_token.map(TokenId);
            super::InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        } else if shared_prefix_eligible {
            let lookup = shared_prefixes.lookup(&compatibility, &prompt_tokens);
            prefix_state = lookup.state;
            prefix_tokens_reused = lookup.reused_tokens;
            prefix_identity = lookup.identity;
            prompt_logits = lookup.prompt_logits;
            last_logits = if lookup.last_logits.is_empty() {
                prompt_logits.last().cloned().unwrap_or_default()
            } else {
                lookup.last_logits
            };
            lookup.cache.unwrap_or_else(|| {
                super::InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            })
        } else if let Some(session_id) = &request.session_id {
            sessions.state(session_id)?.cache().clone()
        } else {
            super::InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        };
        let mut token_history = if exact_prompt_cache_hit {
            prompt_tokens
                .as_slice()
                .iter()
                .copied()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>()
        } else if request.session_id.is_none() && prefix_tokens_reused > 0 {
            prompt_tokens.as_slice()[..prefix_tokens_reused]
                .iter()
                .copied()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>()
        } else {
            cache
                .entries()
                .iter()
                .map(|entry| entry.token.as_u32())
                .collect::<Vec<_>>()
        };
        if cache.width() != expected_kv_width {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width,
                kv_width: cache.width(),
            });
        }
        let reserve_tokens = request.options.max_output_tokens.saturating_add(1);
        let mut cuda_cache = if shared_prefix_eligible && prefix_tokens_reused > 0 {
            if let Some(cache) = exact_cuda_cache {
                cache
            } else if let Some(cache) = cuda_shared_prefixes.lookup(&compatibility, &prompt_tokens)
            {
                cache.detached_with_reserve(
                    backend,
                    reserve_tokens,
                    loaded_model.descriptor().config.max_context,
                )?
            } else {
                CudaKvCacheMirror::from_host_cache(backend, &cache, reserve_tokens)?
            }
        } else {
            CudaKvCacheMirror::from_host_cache(backend, &cache, reserve_tokens)?
        };
        for token in &prompt_tokens.as_slice()[prefix_tokens_reused..] {
            ensure_cuda_decode_step_plan(
                &loaded_model,
                backend,
                &mut decode_step_plan,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
            )?;
            let step_start = Instant::now();
            let step = loaded_model.inner.forward_step_with_cuda_plan(
                backend,
                *token,
                cuda_cache.len(),
                &mut cuda_cache,
                decode_step_plan.as_mut().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("missing cuda gpt-oss decode-step plan"),
                    ))
                })?,
                CudaStepOutputMode::FullLogits,
                shared_prefix_eligible || request.session_id.is_some(),
            );
            let step = step?;
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.step_wall_ns = perf
                .stage_timings
                .step_wall_ns
                .saturating_add(duration_ns(step_start));
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            if !step.key.is_empty() {
                cache.append(*token, step.key.clone(), step.value.clone())?;
            }
            token_history.push(token.as_u32());
            last_logits = step.logits;
            prompt_logits.push(last_logits.clone());
        }
        let should_record_prompt_prefix =
            shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len();
        let prompt_cache = should_record_prompt_prefix.then(|| cache.clone());
        let prompt_cuda_cache = should_record_prompt_prefix.then(|| cuda_cache.clone());

        let mut sampler = super::GenerationSampler::new(&request.options)?;
        let mut generated_tokens = Vec::new();
        let mut pending_token = if exact_prompt_cache_hit
            && can_use_cached_prompt_argmax(&request.options)
        {
            exact_prompt_token.ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?
        } else {
            let sampling_start = Instant::now();
            let token = match sampler.select_next_token_from_history(
                loaded_model.tokenizer(),
                &last_logits,
                &token_history,
                generated_tokens.as_slice(),
            )? {
                super::GenerationSelection::Token(token) => token,
                super::GenerationSelection::Terminate => {
                    loaded_model.tokenizer().vocabulary().eos_id()
                }
            };
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.sampling_ns = perf
                .stage_timings
                .sampling_ns
                .saturating_add(duration_ns(sampling_start));
            token
        };
        let termination = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break super::TerminationReason::MaxOutputTokens;
            }
            if cuda_cache.len() >= cache.max_context() {
                break super::TerminationReason::ContextLimit;
            }
            if loaded_model.is_end_of_sequence(pending_token) {
                break super::TerminationReason::EndOfSequence;
            }

            generated_tokens.push(pending_token);
            ensure_cuda_decode_step_plan(
                &loaded_model,
                backend,
                &mut decode_step_plan,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
            )?;
            let step_start = Instant::now();
            let step = loaded_model.inner.forward_step_with_cuda_plan(
                backend,
                pending_token,
                cuda_cache.len(),
                &mut cuda_cache,
                decode_step_plan.as_mut().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("missing cuda gpt-oss decode-step plan"),
                    ))
                })?,
                if use_cuda_argmax_fast_path {
                    CudaStepOutputMode::DeviceArgmax
                } else {
                    CudaStepOutputMode::FullLogits
                },
                request.session_id.is_some(),
            );
            let step = step?;
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.step_wall_ns = perf
                .stage_timings
                .step_wall_ns
                .saturating_add(duration_ns(step_start));
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            if !step.key.is_empty() {
                cache.append(pending_token, step.key.clone(), step.value.clone())?;
            }
            token_history.push(pending_token.as_u32());

            let stop_check_start = Instant::now();
            let stop_hit = super::truncate_generated_text(
                loaded_model.tokenizer(),
                &mut generated_tokens,
                &request.options.stop_sequences,
            )
            .is_some();
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.stop_check_ns = perf
                .stage_timings
                .stop_check_ns
                .saturating_add(duration_ns(stop_check_start));
            if stop_hit {
                break super::TerminationReason::EndOfSequence;
            }

            if let Some(token) = step.selected_token {
                pending_token = token;
            } else {
                last_logits = step.logits;
                let sampling_start = Instant::now();
                pending_token = match sampler.select_next_token_from_history(
                    loaded_model.tokenizer(),
                    &last_logits,
                    &token_history,
                    generated_tokens.as_slice(),
                )? {
                    super::GenerationSelection::Token(token) => token,
                    super::GenerationSelection::Terminate => {
                        loaded_model.tokenizer().vocabulary().eos_id()
                    }
                };
                let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
                perf.stage_timings.sampling_ns = perf
                    .stage_timings
                    .sampling_ns
                    .saturating_add(duration_ns(sampling_start));
            }
        };

        if should_record_prompt_prefix {
            if let (Some(prompt_cache), Some(prompt_cuda_cache)) =
                (prompt_cache.as_ref(), prompt_cuda_cache.as_ref())
            {
                let recorded_identity = shared_prefixes.record(
                    compatibility.clone(),
                    &prompt_tokens,
                    &prompt_logits,
                    prompt_cache,
                );
                cuda_shared_prefixes.record(compatibility, &prompt_tokens, prompt_cuda_cache);
                if prefix_state != super::PrefixCacheState::Hit || prefix_identity.is_none() {
                    prefix_identity = Some(recorded_identity);
                }
            }
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);

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
        if let Some(plan) = decode_step_plan.take() {
            loaded_model.inner.release_decode_step_plan(plan);
        }
        let text = loaded_model.tokenizer().decode(generated.as_slice());
        let usage = super::GenerationUsage {
            input_tokens: prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: cuda_cache.len(),
        };
        let kv_cache = psionic_runtime::KvCacheAccounting::from_states(
            &previous_kv_state,
            psionic_runtime::KvCacheState::paged(cache.page_layout(), cuda_cache.len()),
        );
        let total_duration_ns = generation_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let metrics = super::GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match load_state {
                super::GenerationLoadState::Cold => loaded_model.load_duration_ns(),
                super::GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(total_duration_ns.saturating_sub(prompt_eval_duration_ns)),
            kv_cache: Some(kv_cache),
            prefix_tokens_reused: Some(prefix_tokens_reused),
            gpt_oss_perf: gpt_oss_perf.filter(|perf| !perf.is_zero()),
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| loaded_model.plan_digest().to_string());
        let provenance = super::GenerationProvenance {
            served_artifact,
            adapter_serving: None,
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            load_state,
            isolation_policy: super::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan,
            residency_policy,
            residency_snapshot,
            kv_cache_policy: Some(cache.policy().clone()),
            prefix_cache_state: Some(prefix_state),
            prefix_cache_policy: Some(prefix_policy),
            prefix_cache_identity: prefix_identity,
            compile_path: compile_path.clone(),
            delivery_proof: super::build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
            ),
            cache_observations: super::generation_cache_observations(
                loaded_model.descriptor(),
                compile_path.as_ref(),
                load_state,
                request.session_id.as_ref(),
                request.reset_session,
                &previous_kv_state,
                prefix_state,
            ),
            structured_output: sampler.structured_output_report(),
        };
        Ok(GenerationResponse::new(
            request,
            request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance))
    })();

    let _ = models.finish_request(model_id, current_time_millis());
    result
}

fn run_cuda_hybrid_generation_request(
    backend: &mut CudaBackend,
    models: &mut InMemoryGenerationModelRegistry<CudaGgufGptOssGenerationModel>,
    sessions: &mut InMemoryGenerationSessionStore,
    shared_prefixes: &mut SharedPrefixStore,
    request: &super::GenerationRequest,
) -> Result<GenerationResponse, ReferenceTextGenerationError> {
    if request.product_id != super::TEXT_GENERATION_PRODUCT_ID {
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
        .unwrap_or(super::GenerationLoadState::Warm);
    let request_start = current_time_millis();
    let generation_start = Instant::now();
    models.begin_request(model_id, request_start)?;
    let memory_plan = models.memory_plan(model_id).cloned();
    let residency_policy = Some(models.residency_policy().clone());
    let residency_snapshot = Some(models.memory_snapshot());
    let served_artifact = super::served_artifact_identity_for_decoder_backend(
        loaded_model.descriptor(),
        loaded_model.backend_compatibility(),
        &[],
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
        let compatibility = prefix_compatibility(&loaded_model);
        let prefix_policy = default_prefix_cache_policy();
        let mut prefix_state = super::PrefixCacheState::None;
        let mut prefix_tokens_reused = 0usize;
        let mut prefix_identity = None;
        let mut shared_prefix_eligible = false;
        let previous_kv_state = if let Some(session_id) = &request.session_id {
            if request.reset_session {
                sessions.reset(session_id)?;
            }
            let state = sessions.state(session_id)?;
            super::validate_session_model(
                state,
                session_id,
                loaded_model.descriptor(),
                served_artifact.served_artifact_digest.as_str(),
            )?;
            session_tokens = state.tokens().to_vec();
            if state.cache().is_empty() {
                shared_prefix_eligible = true;
            } else {
                prefix_state = super::PrefixCacheState::Bypassed;
            }
            state.cache().state()
        } else {
            shared_prefix_eligible = true;
            psionic_runtime::KvCacheState::default()
        };
        let preserve_prefix_tokens = usize::from(
            prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
        );
        let (prompt_tokens, context_window) = super::apply_context_window(
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
        let compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let plan_cache_hits = 0usize;
        let plan_cache_misses = 0usize;
        let mut gpt_oss_perf: Option<GptOssPerformanceMetrics> = None;
        let use_cuda_argmax_fast_path = can_use_cuda_argmax_fast_path(&request.options);
        let mut cache = if shared_prefix_eligible {
            let lookup = shared_prefixes.lookup(&compatibility, &prompt_tokens);
            prefix_state = lookup.state;
            prefix_tokens_reused = lookup.reused_tokens;
            prefix_identity = lookup.identity;
            prompt_logits = lookup.prompt_logits;
            last_logits = if lookup.last_logits.is_empty() {
                prompt_logits.last().cloned().unwrap_or_default()
            } else {
                lookup.last_logits
            };
            lookup.cache.unwrap_or_else(|| {
                super::InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            })
        } else if let Some(session_id) = &request.session_id {
            sessions.state(session_id)?.cache().clone()
        } else {
            super::InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        };
        let mut token_history = cache
            .entries()
            .iter()
            .map(|entry| entry.token.as_u32())
            .collect::<Vec<_>>();
        if cache.width() != expected_kv_width {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width,
                kv_width: cache.width(),
            });
        }
        let reserve_tokens = request.options.max_output_tokens.saturating_add(1);
        let mut cuda_cache = CudaKvCacheMirror::from_host_cache(backend, &cache, reserve_tokens)?;
        for token in &prompt_tokens.as_slice()[prefix_tokens_reused..] {
            let step_start = Instant::now();
            let step = loaded_model.inner.forward_step_with_output_mode(
                backend,
                *token,
                cache.len(),
                &cache,
                CudaStepOutputMode::FullLogits,
                Some(&mut cuda_cache),
                true,
            )?;
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.step_wall_ns = perf
                .stage_timings
                .step_wall_ns
                .saturating_add(duration_ns(step_start));
            if execution_plan_digest.is_none() {
                execution_plan_digest = Some(loaded_model.plan_digest().to_string());
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            cache.append(*token, step.key, step.value)?;
            token_history.push(token.as_u32());
            last_logits = step.logits;
            prompt_logits.push(last_logits.clone());
        }
        let should_record_prompt_prefix =
            shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len();
        let prompt_cache = should_record_prompt_prefix.then(|| cache.clone());

        let mut sampler = super::GenerationSampler::new(&request.options)?;
        let mut generated_tokens = Vec::new();
        let sampling_start = Instant::now();
        let mut pending_token = match sampler.select_next_token_from_history(
            loaded_model.tokenizer(),
            &last_logits,
            &token_history,
            generated_tokens.as_slice(),
        )? {
            super::GenerationSelection::Token(token) => token,
            super::GenerationSelection::Terminate => loaded_model.tokenizer().vocabulary().eos_id(),
        };
        let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
        perf.stage_timings.sampling_ns = perf
            .stage_timings
            .sampling_ns
            .saturating_add(duration_ns(sampling_start));
        let can_skip_host_kv_materialization = request.session_id.is_none()
            && use_cuda_argmax_fast_path
            && loaded_model
                .inner
                .supports_hybrid_cuda_device_argmax_fast_path();
        let mut generated_cache_tokens = cache.len();
        let termination = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break super::TerminationReason::MaxOutputTokens;
            }
            if generated_cache_tokens >= cache.max_context() {
                break super::TerminationReason::ContextLimit;
            }
            if loaded_model.is_end_of_sequence(pending_token) {
                break super::TerminationReason::EndOfSequence;
            }

            generated_tokens.push(pending_token);
            let step_start = Instant::now();
            let step = loaded_model.inner.forward_step_with_output_mode(
                backend,
                pending_token,
                generated_cache_tokens,
                &cache,
                if use_cuda_argmax_fast_path {
                    CudaStepOutputMode::DeviceArgmax
                } else {
                    CudaStepOutputMode::FullLogits
                },
                Some(&mut cuda_cache),
                !can_skip_host_kv_materialization,
            )?;
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.step_wall_ns = perf
                .stage_timings
                .step_wall_ns
                .saturating_add(duration_ns(step_start));
            if execution_plan_digest.is_none() {
                execution_plan_digest = Some(loaded_model.plan_digest().to_string());
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            if can_skip_host_kv_materialization {
                generated_cache_tokens = generated_cache_tokens.saturating_add(1);
            } else {
                cache.append(pending_token, step.key.clone(), step.value.clone())?;
                generated_cache_tokens = cache.len();
            }
            token_history.push(pending_token.as_u32());

            let stop_check_start = Instant::now();
            let stop_hit = super::truncate_generated_text(
                loaded_model.tokenizer(),
                &mut generated_tokens,
                &request.options.stop_sequences,
            )
            .is_some();
            let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
            perf.stage_timings.stop_check_ns = perf
                .stage_timings
                .stop_check_ns
                .saturating_add(duration_ns(stop_check_start));
            if stop_hit {
                break super::TerminationReason::EndOfSequence;
            }

            if let Some(token) = step.selected_token {
                pending_token = token;
            } else {
                last_logits = step.logits;
                let sampling_start = Instant::now();
                pending_token = match sampler.select_next_token_from_history(
                    loaded_model.tokenizer(),
                    &last_logits,
                    &token_history,
                    generated_tokens.as_slice(),
                )? {
                    super::GenerationSelection::Token(token) => token,
                    super::GenerationSelection::Terminate => {
                        loaded_model.tokenizer().vocabulary().eos_id()
                    }
                };
                let perf = gpt_oss_perf.get_or_insert_with(GptOssPerformanceMetrics::default);
                perf.stage_timings.sampling_ns = perf
                    .stage_timings
                    .sampling_ns
                    .saturating_add(duration_ns(sampling_start));
            }
        };

        if should_record_prompt_prefix {
            if let Some(prompt_cache) = prompt_cache.as_ref() {
                let recorded_identity = shared_prefixes.record(
                    compatibility,
                    &prompt_tokens,
                    &prompt_logits,
                    prompt_cache,
                );
                if prefix_state != super::PrefixCacheState::Hit || prefix_identity.is_none() {
                    prefix_identity = Some(recorded_identity);
                }
            }
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);

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
        let usage = super::GenerationUsage {
            input_tokens: prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: cache.len(),
        };
        let kv_cache =
            psionic_runtime::KvCacheAccounting::from_states(&previous_kv_state, cache.state());
        let total_duration_ns = generation_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let metrics = super::GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match load_state {
                super::GenerationLoadState::Cold => loaded_model.load_duration_ns(),
                super::GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(total_duration_ns.saturating_sub(prompt_eval_duration_ns)),
            kv_cache: Some(kv_cache),
            prefix_tokens_reused: Some(prefix_tokens_reused),
            gpt_oss_perf: gpt_oss_perf.filter(|perf| !perf.is_zero()),
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| loaded_model.plan_digest().to_string());
        let provenance = super::GenerationProvenance {
            served_artifact,
            adapter_serving: None,
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            load_state,
            isolation_policy: super::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan,
            residency_policy,
            residency_snapshot,
            kv_cache_policy: Some(cache.policy().clone()),
            prefix_cache_state: Some(prefix_state),
            prefix_cache_policy: Some(prefix_policy),
            prefix_cache_identity: prefix_identity,
            compile_path: compile_path.clone(),
            delivery_proof: super::build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
            ),
            cache_observations: super::generation_cache_observations(
                loaded_model.descriptor(),
                compile_path.as_ref(),
                load_state,
                request.session_id.as_ref(),
                request.reset_session,
                &previous_kv_state,
                prefix_state,
            ),
            structured_output: sampler.structured_output_report(),
        };
        Ok(GenerationResponse::new(
            request,
            request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance))
    })();

    let _ = models.finish_request(model_id, current_time_millis());
    result
}

fn run_metal_generation_request(
    backend: &mut MetalBackend,
    models: &mut InMemoryGenerationModelRegistry<MetalGgufGptOssGenerationModel>,
    sessions: &mut InMemoryGenerationSessionStore,
    shared_prefixes: &mut SharedPrefixStore,
    metal_shared_prefixes: &mut MetalLayerSharedPrefixStore,
    metal_prompt_prefixes: &mut MetalPromptPrefixStore,
    request: &super::GenerationRequest,
) -> Result<GenerationResponse, ReferenceTextGenerationError> {
    if request.product_id != super::TEXT_GENERATION_PRODUCT_ID {
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
        .unwrap_or(super::GenerationLoadState::Warm);
    let request_start = current_time_millis();
    let generation_start = Instant::now();
    models.begin_request(model_id, request_start)?;
    let memory_plan = models.memory_plan(model_id).cloned();
    let residency_policy = Some(models.residency_policy().clone());
    let residency_snapshot = Some(models.memory_snapshot());
    let served_artifact = super::served_artifact_identity_for_decoder_backend(
        loaded_model.descriptor(),
        loaded_model.backend_compatibility(),
        &[],
    );

    let result = (|| -> Result<GenerationResponse, ReferenceTextGenerationError> {
        let prompt_eval_start = Instant::now();
        let tokenizer = loaded_model.tokenizer();
        let prompt_tokens = loaded_model.encode_prompt_input(&request.prompt)?;
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }

        let expected_kv_width = loaded_model.cache_width();
        let layer_count = loaded_model.inner.layer_count();
        let layer_kv_width = loaded_model.inner.layer_kv_width();
        let mut session_tokens = Vec::new();
        let compatibility = prefix_compatibility(&loaded_model);
        let metal_compatibility = metal_prefix_compatibility(
            &compatibility,
            layer_kv_width,
            loaded_model.descriptor().config.max_context,
        );
        let prefix_policy = default_prefix_cache_policy();
        let mut prefix_state = super::PrefixCacheState::None;
        let mut prefix_tokens_reused = 0usize;
        let mut prefix_identity = None;
        let mut shared_prefix_eligible = false;
        let previous_kv_state = if let Some(session_id) = &request.session_id {
            if request.reset_session {
                sessions.reset(session_id)?;
            }
            let state = sessions.state(session_id)?;
            super::validate_session_model(
                state,
                session_id,
                loaded_model.descriptor(),
                served_artifact.served_artifact_digest.as_str(),
            )?;
            session_tokens = state.tokens().to_vec();
            if state.cache().is_empty() {
                shared_prefix_eligible = true;
            } else {
                prefix_state = super::PrefixCacheState::Bypassed;
            }
            state.cache().state()
        } else {
            shared_prefix_eligible = true;
            psionic_runtime::KvCacheState::default()
        };
        let preserve_prefix_tokens = usize::from(
            prompt_tokens.as_slice().first().copied() == Some(tokenizer.vocabulary().bos_id()),
        );
        let (prompt_tokens, context_window) = super::apply_context_window(
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
        let mut decode_step_plan = None;
        let mut exact_prompt_token = None;
        let prompt_token_ids = metal_prompt_token_ids(&prompt_tokens);
        let sessionless_metal_lookup = (shared_prefix_eligible && request.session_id.is_none())
            .then(|| {
                metal_shared_prefixes.lookup(&metal_compatibility, &prompt_token_ids, layer_count)
            });
        let sessionless_exact_prefix_hit = (shared_prefix_eligible && request.session_id.is_none())
            .then(|| metal_prompt_prefixes.lookup_exact_prompt(&compatibility, &prompt_tokens))
            .flatten();
        let exact_prompt_cache_hit = sessionless_exact_prefix_hit.is_some()
            && sessionless_metal_lookup.as_ref().is_some_and(|lookup| {
                lookup.state == super::PrefixCacheState::Hit
                    && lookup.reused_tokens == prompt_tokens.len()
                    && lookup.caches.is_some()
            });
        let cache = if shared_prefix_eligible && request.session_id.is_none() {
            if let Some(lookup) = sessionless_metal_lookup.as_ref() {
                prefix_state = lookup.state;
                prefix_tokens_reused = lookup.reused_tokens;
                prefix_identity = lookup.identity.clone();
            }
            if let Some(hit) = sessionless_exact_prefix_hit
                .as_ref()
                .filter(|_| exact_prompt_cache_hit)
            {
                prefix_identity = Some(hit.identity.clone());
                last_logits = hit.last_logits.clone();
                exact_prompt_token = hit.greedy_token.map(TokenId);
            }
            super::InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        } else if shared_prefix_eligible {
            let lookup = shared_prefixes.lookup(&compatibility, &prompt_tokens);
            prefix_state = lookup.state;
            prefix_tokens_reused = lookup.reused_tokens;
            prefix_identity = lookup.identity;
            prompt_logits = lookup.prompt_logits;
            last_logits = if lookup.last_logits.is_empty() {
                prompt_logits.last().cloned().unwrap_or_default()
            } else {
                lookup.last_logits
            };
            lookup.cache.unwrap_or_else(|| {
                super::InMemoryKvCache::new(
                    loaded_model.descriptor().config.max_context,
                    expected_kv_width,
                )
            })
        } else if let Some(session_id) = &request.session_id {
            sessions.state(session_id)?.cache().clone()
        } else {
            super::InMemoryKvCache::new(
                loaded_model.descriptor().config.max_context,
                expected_kv_width,
            )
        };
        let mut token_history = if exact_prompt_cache_hit {
            prompt_tokens
                .as_slice()
                .iter()
                .copied()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>()
        } else {
            cache
                .entries()
                .iter()
                .map(|entry| entry.token.as_u32())
                .collect::<Vec<_>>()
        };
        if cache.width() != expected_kv_width {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width,
                kv_width: cache.width(),
            });
        }

        let metal_lookup = if request.session_id.is_none() {
            sessionless_metal_lookup.clone()
        } else {
            (shared_prefix_eligible && prefix_tokens_reused > 0).then(|| {
                metal_shared_prefixes.lookup(&metal_compatibility, &prompt_token_ids, layer_count)
            })
        };
        let mut layer_caches = if let Some(lookup) = metal_lookup.as_ref() {
            if let Some(caches) = lookup.caches.as_ref() {
                caches.clone()
            } else {
                build_metal_layer_caches_from_host_cache(
                    backend,
                    &cache,
                    layer_count,
                    layer_kv_width,
                    request.options.max_output_tokens.saturating_add(1),
                )?
            }
        } else {
            build_metal_layer_caches_from_host_cache(
                backend,
                &cache,
                layer_count,
                layer_kv_width,
                request.options.max_output_tokens.saturating_add(1),
            )?
        };
        let metal_prefix_identity = prefix_identity.clone().or_else(|| {
            metal_lookup
                .as_ref()
                .and_then(|lookup| lookup.identity.clone())
        });
        let metal_prefix_metrics = {
            let current_state = metal_layer_cache_state(layer_caches.as_slice());
            let action = if let Some(lookup) = metal_lookup.as_ref() {
                if lookup.state == super::PrefixCacheState::Hit && lookup.caches.is_some() {
                    CacheAction::Reuse
                } else if prefix_tokens_reused > 0 {
                    CacheAction::Rebuild
                } else if lookup.state == super::PrefixCacheState::Rebuilt {
                    CacheAction::Invalidate
                } else {
                    CacheAction::Bypass
                }
            } else if !shared_prefix_eligible && !cache.is_empty() {
                CacheAction::Restore
            } else {
                CacheAction::Bypass
            };
            let residency_prefix_state = if shared_prefix_eligible {
                prefix_state
            } else {
                super::PrefixCacheState::Bypassed
            };
            MetalPromptResidencyMetrics::new(
                &psionic_runtime::KvCacheState::default(),
                current_state,
                residency_prefix_state,
                metal_prefix_identity,
                action,
            )
        };
        let mut attention_runtime = loaded_model
            .inner
            .reserve_decode_attention_runtime(backend)?;
        let decode_output_mode = metal_decode_logits_output_mode(&request.options);

        let prompt_suffix = &prompt_tokens.as_slice()[prefix_tokens_reused..];
        for (prompt_index, token) in prompt_suffix.iter().enumerate() {
            let is_last_prompt_token = prompt_index + 1 == prompt_suffix.len();
            ensure_metal_decode_step_plan(
                &loaded_model,
                backend,
                &mut decode_step_plan,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
            )?;
            let position = layer_caches
                .first()
                .map(MetalKvCacheMirror::len)
                .unwrap_or(cache.len());
            let step = loaded_model.inner.forward_step_with_device_attention_plan(
                backend,
                *token,
                position,
                layer_caches.as_mut_slice(),
                &mut attention_runtime,
                decode_step_plan.as_mut().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("missing metal gpt-oss decode-step plan"),
                    ))
                })?,
                if is_last_prompt_token {
                    MetalStepOutputMode::Logits(MetalLogitsOutputMode::RawLogits)
                } else {
                    MetalStepOutputMode::SkipLogits
                },
                false,
            )?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = Some(loaded_model.plan_digest().to_string());
            }
            if compile_path.is_none() {
                let runtime_compile_path = attention_runtime.metrics().compile_path.clone();
                match runtime_compile_path.temperature {
                    CompilePathTemperature::WarmReuse => {
                        plan_cache_hits = plan_cache_hits.saturating_add(1);
                    }
                    CompilePathTemperature::ColdCompile => {
                        plan_cache_misses = plan_cache_misses.saturating_add(1);
                    }
                }
                compile_path = Some(runtime_compile_path);
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            token_history.push(token.as_u32());
            if is_last_prompt_token {
                last_logits = step.logits;
                prompt_logits.push(last_logits.clone());
            }
        }
        let should_record_prompt_prefix =
            shared_prefix_eligible && prefix_tokens_reused != prompt_tokens.len();
        let prompt_layer_caches = should_record_prompt_prefix.then(|| layer_caches.clone());

        let mut sampler = super::GenerationSampler::new(&request.options)?;
        let mut generated_tokens = Vec::new();
        let mut pending_token =
            if exact_prompt_cache_hit && can_use_cached_prompt_argmax(&request.options) {
                Some(
                    exact_prompt_token
                        .ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?,
                )
            } else {
                Some(
                    match sampler.select_next_token_from_history(
                        loaded_model.tokenizer(),
                        &last_logits,
                        &token_history,
                        generated_tokens.as_slice(),
                    )? {
                        super::GenerationSelection::Token(token) => token,
                        super::GenerationSelection::Terminate => {
                            loaded_model.tokenizer().vocabulary().eos_id()
                        }
                    },
                )
            };
        let termination = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break super::TerminationReason::MaxOutputTokens;
            }
            let current_cache_tokens = layer_caches
                .first()
                .map(MetalKvCacheMirror::len)
                .unwrap_or(cache.len());
            if current_cache_tokens >= cache.max_context() {
                break super::TerminationReason::ContextLimit;
            }
            let next_token = if let Some(token) = pending_token.take() {
                token
            } else {
                match sampler.select_next_token_from_history(
                    loaded_model.tokenizer(),
                    &last_logits,
                    &token_history,
                    generated_tokens.as_slice(),
                )? {
                    super::GenerationSelection::Token(token) => token,
                    super::GenerationSelection::Terminate => {
                        loaded_model.tokenizer().vocabulary().eos_id()
                    }
                }
            };
            if loaded_model.is_end_of_sequence(next_token) {
                break super::TerminationReason::EndOfSequence;
            }

            generated_tokens.push(next_token);
            ensure_metal_decode_step_plan(
                &loaded_model,
                backend,
                &mut decode_step_plan,
                &mut execution_plan_digest,
                &mut compile_path,
                &mut plan_cache_hits,
                &mut plan_cache_misses,
            )?;
            let position = layer_caches
                .first()
                .map(MetalKvCacheMirror::len)
                .unwrap_or(cache.len());
            let step = loaded_model.inner.forward_step_with_device_attention_plan(
                backend,
                next_token,
                position,
                layer_caches.as_mut_slice(),
                &mut attention_runtime,
                decode_step_plan.as_mut().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("missing metal gpt-oss decode-step plan"),
                    ))
                })?,
                MetalStepOutputMode::Logits(decode_output_mode),
                true,
            )?;
            if execution_plan_digest.is_none() {
                execution_plan_digest = Some(loaded_model.plan_digest().to_string());
            }
            if compile_path.is_none() {
                let runtime_compile_path = attention_runtime.metrics().compile_path.clone();
                match runtime_compile_path.temperature {
                    CompilePathTemperature::WarmReuse => {
                        plan_cache_hits = plan_cache_hits.saturating_add(1);
                    }
                    CompilePathTemperature::ColdCompile => {
                        plan_cache_misses = plan_cache_misses.saturating_add(1);
                    }
                }
                compile_path = Some(runtime_compile_path);
            }
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            token_history.push(next_token.as_u32());

            if super::truncate_generated_text(
                loaded_model.tokenizer(),
                &mut generated_tokens,
                &request.options.stop_sequences,
            )
            .is_some()
            {
                break super::TerminationReason::EndOfSequence;
            }

            if let Some(token) = step.selected_token {
                pending_token = Some(token);
            } else {
                last_logits = step.logits;
                pending_token = Some(
                    match sampler.select_next_token_from_history(
                        loaded_model.tokenizer(),
                        &last_logits,
                        &token_history,
                        generated_tokens.as_slice(),
                    )? {
                        super::GenerationSelection::Token(token) => token,
                        super::GenerationSelection::Terminate => {
                            loaded_model.tokenizer().vocabulary().eos_id()
                        }
                    },
                );
            }
        };

        if should_record_prompt_prefix {
            if let Some(prompt_layer_caches) = prompt_layer_caches.as_ref() {
                let last_prompt_logits = prompt_logits
                    .last()
                    .cloned()
                    .unwrap_or_else(|| last_logits.clone());
                let recorded_identity = if request.session_id.is_none() {
                    metal_prompt_prefixes.record(
                        compatibility.clone(),
                        &prompt_tokens,
                        last_prompt_logits.as_slice(),
                    )
                } else {
                    let prompt_cache = build_host_cache_from_metal_layer_caches(
                        prompt_token_ids.as_slice(),
                        prompt_layer_caches,
                        cache.max_context(),
                        layer_kv_width,
                        cache.policy(),
                    )?;
                    shared_prefixes.record(
                        compatibility.clone(),
                        &prompt_tokens,
                        &prompt_logits,
                        &prompt_cache,
                    )
                };
                metal_shared_prefixes.record(
                    &metal_compatibility,
                    &prompt_token_ids,
                    prompt_layer_caches,
                );
                if prefix_state != super::PrefixCacheState::Hit || prefix_identity.is_none() {
                    prefix_identity = Some(recorded_identity);
                }
            }
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);

        if let Some(plan) = decode_step_plan.take() {
            loaded_model.inner.release_decode_step_plan(plan);
        }

        let generated = TokenSequence::new(generated_tokens);
        let final_cache = if request.session_id.is_some() {
            Some(build_host_cache_from_metal_layer_caches(
                token_history.as_slice(),
                layer_caches.as_slice(),
                cache.max_context(),
                layer_kv_width,
                cache.policy(),
            )?)
        } else {
            None
        };
        if let Some(session_id) = &request.session_id {
            session_tokens.extend_from_slice(prompt_tokens.as_slice());
            session_tokens.extend_from_slice(generated.as_slice());
            sessions.replace_cache(
                session_id,
                loaded_model.descriptor(),
                served_artifact.served_artifact_digest.as_str(),
                final_cache.clone().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("missing rebuilt host cache for metal session update"),
                    ))
                })?,
                TokenSequence::new(session_tokens),
            )?;
        }
        let text = loaded_model.tokenizer().decode(generated.as_slice());
        let current_cache_tokens = layer_caches
            .first()
            .map(MetalKvCacheMirror::len)
            .unwrap_or_else(|| final_cache.as_ref().map_or(0, super::InMemoryKvCache::len));
        let usage = super::GenerationUsage {
            input_tokens: prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: current_cache_tokens,
        };
        let kv_cache = psionic_runtime::KvCacheAccounting::from_states(
            &previous_kv_state,
            psionic_runtime::KvCacheState::paged(cache.page_layout(), current_cache_tokens),
        );
        let total_duration_ns = generation_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let metrics = super::GenerationMetrics {
            total_duration_ns: Some(total_duration_ns),
            load_duration_ns: Some(match load_state {
                super::GenerationLoadState::Cold => loaded_model.load_duration_ns(),
                super::GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(usage.output_tokens),
            eval_duration_ns: Some(total_duration_ns.saturating_sub(prompt_eval_duration_ns)),
            kv_cache: Some(kv_cache),
            prefix_tokens_reused: Some(prefix_tokens_reused),
            gpt_oss_perf: gpt_oss_perf.filter(|perf| !perf.is_zero()),
        };
        let delivery_plan_digest = execution_plan_digest
            .clone()
            .unwrap_or_else(|| loaded_model.plan_digest().to_string());
        let mut cache_observations = super::generation_cache_observations(
            loaded_model.descriptor(),
            compile_path.as_ref(),
            load_state,
            request.session_id.as_ref(),
            request.reset_session,
            &previous_kv_state,
            prefix_state,
        );
        extend_unique_cache_observations(
            &mut cache_observations,
            metal_prefix_metrics.observations.as_slice(),
        );
        let provenance = super::GenerationProvenance {
            served_artifact,
            adapter_serving: None,
            execution_plan_digest: delivery_plan_digest.clone(),
            cluster_execution: None,
            load_state,
            isolation_policy: super::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan,
            residency_policy,
            residency_snapshot,
            kv_cache_policy: Some(cache.policy().clone()),
            prefix_cache_state: Some(prefix_state),
            prefix_cache_policy: Some(prefix_policy),
            prefix_cache_identity: prefix_identity,
            compile_path: compile_path.clone(),
            delivery_proof: super::build_delivery_proof(
                delivery_plan_digest,
                kernel_count,
                bytes_moved,
                plan_cache_hits,
                plan_cache_misses,
                metrics.kv_cache.as_ref().map(|value| value.growth.clone()),
            ),
            cache_observations,
            structured_output: sampler.structured_output_report(),
        };
        Ok(GenerationResponse::new(
            request,
            request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance))
    })();

    let _ = models.finish_request(model_id, current_time_millis());
    result
}

fn metal_prefix_compatibility(
    compatibility: &super::SharedPrefixCompatibility,
    kv_width: usize,
    max_context_tokens: usize,
) -> MetalSharedPrefixCompatibility {
    MetalSharedPrefixCompatibility {
        served_artifact_digest: compatibility.served_artifact_digest.clone(),
        model_id: compatibility.model_id.clone(),
        model_revision: compatibility.model_revision.clone(),
        weight_bundle_digest: compatibility.weight_bundle_digest.clone(),
        tokenizer_family: compatibility.tokenizer_family.clone(),
        backend_compatibility: compatibility.backend_compatibility.clone(),
        kv_width,
        page_layout: KvCachePageLayout::new(max_context_tokens, 4, kv_width * 4 * 2),
    }
}

fn metal_prompt_token_ids(tokens: &TokenSequence) -> Vec<u32> {
    tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect()
}

fn build_metal_layer_caches_from_host_cache(
    backend: &mut MetalBackend,
    cache: &super::InMemoryKvCache,
    layer_count: usize,
    layer_kv_width: usize,
    reserve_tokens: usize,
) -> Result<Vec<MetalKvCacheMirror>, ReferenceTextGenerationError> {
    if cache.width() != layer_count.saturating_mul(layer_kv_width) {
        return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
            expected_kv_width: layer_count.saturating_mul(layer_kv_width),
            kv_width: cache.width(),
        });
    }

    let mut layer_keys =
        vec![Vec::with_capacity(cache.len().saturating_mul(layer_kv_width)); layer_count];
    let mut layer_values =
        vec![Vec::with_capacity(cache.len().saturating_mul(layer_kv_width)); layer_count];
    for entry in cache.entries() {
        for layer_index in 0..layer_count {
            let layer_offset = layer_index.saturating_mul(layer_kv_width);
            layer_keys[layer_index]
                .extend_from_slice(&entry.key[layer_offset..layer_offset + layer_kv_width]);
            layer_values[layer_index]
                .extend_from_slice(&entry.value[layer_offset..layer_offset + layer_kv_width]);
        }
    }

    layer_keys
        .into_iter()
        .zip(layer_values.into_iter())
        .map(|(keys, values)| {
            backend
                .kv_cache_mirror_from_host_rows(
                    layer_kv_width,
                    cache.max_context(),
                    cache.len(),
                    keys.as_slice(),
                    values.as_slice(),
                    reserve_tokens,
                )
                .map_err(ReferenceTextGenerationError::Runtime)
        })
        .collect()
}

fn build_host_cache_from_metal_layer_caches(
    tokens: &[u32],
    layer_caches: &[MetalKvCacheMirror],
    max_context: usize,
    layer_kv_width: usize,
    policy: &KvCachePolicy,
) -> Result<super::InMemoryKvCache, ReferenceTextGenerationError> {
    if layer_caches.is_empty() {
        if tokens.is_empty() {
            return Ok(super::InMemoryKvCache::with_policy(
                max_context,
                0,
                policy.clone(),
            ));
        }
        return Err(ReferenceTextGenerationError::Runtime(
            super::RuntimeError::Backend(String::from(
                "cannot rebuild host cache from an empty metal layer cache set",
            )),
        ));
    }
    let layer_count = layer_caches.len();
    let token_count = tokens.len();
    let width = layer_count.saturating_mul(layer_kv_width);
    let mut cache = super::InMemoryKvCache::with_policy(max_context, width, policy.clone());
    for (layer_index, layer_cache) in layer_caches.iter().enumerate() {
        if layer_cache.width() != layer_kv_width {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width: layer_kv_width,
                kv_width: layer_cache.width(),
            });
        }
        if layer_cache.len() != token_count {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(format!(
                    "metal layer cache {layer_index} token length mismatch while rebuilding host cache: expected {token_count}, actual {}",
                    layer_cache.len()
                )),
            ));
        }
    }
    for (token_index, token) in tokens.iter().copied().enumerate() {
        let mut key = vec![0.0; width];
        let mut value = vec![0.0; width];
        for (layer_index, layer_cache) in layer_caches.iter().enumerate() {
            let (layer_key, layer_value) = layer_cache
                .read_entry(token_index)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let layer_offset = layer_index.saturating_mul(layer_kv_width);
            key[layer_offset..layer_offset + layer_kv_width].copy_from_slice(layer_key.as_slice());
            value[layer_offset..layer_offset + layer_kv_width]
                .copy_from_slice(layer_value.as_slice());
        }
        cache.append(TokenId(token), key, value)?;
    }
    Ok(cache)
}

fn metal_layer_cache_state(layer_caches: &[MetalKvCacheMirror]) -> psionic_runtime::KvCacheState {
    layer_caches
        .first()
        .map(MetalKvCacheMirror::state)
        .unwrap_or_default()
}

fn extend_unique_cache_observations(
    cache_observations: &mut Vec<CacheObservation>,
    extra: &[CacheObservation],
) {
    for observation in extra {
        if !cache_observations.contains(observation) {
            cache_observations.push(observation.clone());
        }
    }
}

fn i32_slice_to_ne_bytes(values: &[i32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(std::mem::size_of::<i32>()));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn i32_ne_bytes_to_values(bytes: &[u8]) -> Result<Vec<i32>, super::RuntimeError> {
    if bytes.len() % std::mem::size_of::<i32>() != 0 {
        return Err(super::RuntimeError::Backend(format!(
            "i32 byte buffer length {} is not a multiple of {}",
            bytes.len(),
            std::mem::size_of::<i32>()
        )));
    }
    Ok(bytes
        .chunks_exact(std::mem::size_of::<i32>())
        .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("chunk length is exact")))
        .collect())
}

fn gather_selected_expert_bias_rows(
    values: &[f32],
    rows_per_expert: usize,
    selected: &[usize],
) -> Result<Vec<f32>, super::RuntimeError> {
    if rows_per_expert == 0 {
        return Ok(Vec::new());
    }
    if values.len() % rows_per_expert != 0 {
        return Err(super::RuntimeError::Backend(format!(
            "expert bias shape mismatch: {} values not divisible by rows_per_expert {}",
            values.len(),
            rows_per_expert
        )));
    }
    let expert_count = values.len() / rows_per_expert;
    let mut gathered = Vec::with_capacity(selected.len().saturating_mul(rows_per_expert));
    for expert_index in selected {
        if *expert_index >= expert_count {
            return Err(super::RuntimeError::Backend(format!(
                "expert bias index {} exceeds expert count {}",
                expert_index, expert_count
            )));
        }
        let start = expert_index.saturating_mul(rows_per_expert);
        let end = start.saturating_add(rows_per_expert);
        gathered.extend_from_slice(&values[start..end]);
    }
    Ok(gathered)
}

fn gather_selected_expert_bias_rows_into(
    values: &[f32],
    rows_per_expert: usize,
    selected: &[usize],
    output: &mut Vec<f32>,
) -> Result<(), super::RuntimeError> {
    if rows_per_expert == 0 {
        output.clear();
        return Ok(());
    }
    if values.len() % rows_per_expert != 0 {
        return Err(super::RuntimeError::Backend(format!(
            "expert bias shape mismatch: {} values not divisible by rows_per_expert {}",
            values.len(),
            rows_per_expert
        )));
    }
    let expert_count = values.len() / rows_per_expert;
    output.clear();
    output.reserve(selected.len().saturating_mul(rows_per_expert));
    for expert_index in selected {
        if *expert_index >= expert_count {
            return Err(super::RuntimeError::Backend(format!(
                "expert bias index {} exceeds expert count {}",
                expert_index, expert_count
            )));
        }
        let start = expert_index.saturating_mul(rows_per_expert);
        let end = start.saturating_add(rows_per_expert);
        output.extend_from_slice(&values[start..end]);
    }
    Ok(())
}

fn selected_expert_bias_rows<'a>(
    values: &'a [f32],
    rows_per_expert: usize,
    expert_index: usize,
) -> Result<&'a [f32], super::RuntimeError> {
    if rows_per_expert == 0 {
        return Ok(&[]);
    }
    if values.len() % rows_per_expert != 0 {
        return Err(super::RuntimeError::Backend(format!(
            "expert bias shape mismatch: {} values not divisible by rows_per_expert {}",
            values.len(),
            rows_per_expert
        )));
    }
    let expert_count = values.len() / rows_per_expert;
    if expert_index >= expert_count {
        return Err(super::RuntimeError::Backend(format!(
            "expert bias index {} exceeds expert count {}",
            expert_index, expert_count
        )));
    }
    let start = expert_index.saturating_mul(rows_per_expert);
    let end = start.saturating_add(rows_per_expert);
    Ok(&values[start..end])
}

impl From<ModelLoadError> for CudaGptOssTextGenerationError {
    fn from(value: ModelLoadError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<ModelLoadError> for MetalGptOssTextGenerationError {
    fn from(value: ModelLoadError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<LoadedModelRegistryError> for CudaGptOssTextGenerationError {
    fn from(value: LoadedModelRegistryError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<LoadedModelRegistryError> for MetalGptOssTextGenerationError {
    fn from(value: LoadedModelRegistryError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<super::SessionStoreError> for CudaGptOssTextGenerationError {
    fn from(value: super::SessionStoreError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<super::SessionStoreError> for MetalGptOssTextGenerationError {
    fn from(value: super::SessionStoreError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl ManagedTextGenerationRuntime for CudaGgufGptOssTextGenerationService {
    fn loaded_models(&mut self) -> super::LoadedModelsObservation {
        CudaGgufGptOssTextGenerationService::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        CudaGgufGptOssTextGenerationService::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, <Self as TextGenerationExecutor>::Error> {
        self.cuda_shared_prefixes.clear();
        Ok(self.models.unload_view(model_id, current_time_millis())?)
    }
}

impl super::StreamingTextGenerationExecutor for CudaGgufGptOssTextGenerationService {
    type Stream<'a> = Box<dyn super::GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &super::GenerationRequest,
    ) -> Result<Self::Stream<'a>, <Self as TextGenerationExecutor>::Error> {
        let response = self.generate(request)?;
        Ok(Box::new(OneShotGenerationStream::new(response)))
    }
}

#[derive(Clone, Debug)]
pub struct CpuGgufGptOssGenerationModel {
    inner: Arc<GptOssCpuModelInner>,
}

impl CpuGgufGptOssGenerationModel {
    /// Loads a CPU GPT-OSS execution model from a GGUF artifact path.
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let artifact = GgufBlobArtifact::open_path(path, gpt_oss_local_blob_open_options())?;
        Self::from_blob_artifact(artifact)
    }

    fn from_blob_artifact(
        artifact: GgufBlobArtifact,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if adapter.family_metadata().family != GgufDecoderFamily::GptOss {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }

        let tokenizer = GptOssTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build gpt-oss tokenizer: {error}"),
            }
        })?;
        let output = if let Some(output) = adapter.tensor_layout().output.as_ref() {
            load_quantized_matrix(&artifact, output)?
        } else {
            load_quantized_matrix(&artifact, adapter.tensor_layout().token_embedding.as_str())?
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layer| GptOssLayer::load(&artifact, layer))
            .collect::<Result<Vec<_>, _>>()?;
        let inner = GptOssCpuModelInner {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            token_embedding: load_quantized_matrix(
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?,
            output_norm: load_dense_vector(
                &artifact,
                adapter.tensor_layout().output_norm.as_str(),
            )?,
            output,
            layers,
            plan_digest: digest_gpt_oss_plan(
                adapter.descriptor(),
                adapter.family_metadata(),
                GPT_OSS_CPU_BACKEND,
            ),
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    #[must_use]
    pub fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }
}

impl GenerationModelHandle for CpuGgufGptOssGenerationModel {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.inner.descriptor
    }

    fn cache_width(&self) -> usize {
        self.inner.cache_width()
    }
}

impl CompiledWordGenerationModel for CpuGgufGptOssGenerationModel {
    type Backend = CpuBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        &self.inner.tokenizer
    }

    fn encode_prompt_input(
        &self,
        input: &super::GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            super::GenerationInput::Text(text) => self.inner.tokenizer.encode_with_defaults(text),
            super::GenerationInput::Tokens(tokens) => tokens.clone(),
        })
    }

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.inner.tokenizer.is_end_of_sequence(token)
    }

    fn execute_step(
        &self,
        _backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
    ) -> Result<super::GenerationStepOutput, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.inner.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.inner.descriptor.config.vocab_size,
            });
        }
        if position >= self.inner.descriptor.config.max_context {
            return Err(ReferenceTextGenerationError::InvalidPosition {
                position,
                max_context: self.inner.descriptor.config.max_context,
            });
        }
        if cache.width() != self.inner.cache_width() {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width: self.inner.cache_width(),
                kv_width: cache.width(),
            });
        }

        let step = self.inner.forward_step(token, position, cache)?;
        Ok(super::GenerationStepOutput {
            key: step.key,
            value: step.value,
            logits: step.logits,
            execution_plan_digest: Some(self.inner.plan_digest.clone()),
            compile_path: None,
            kernel_count: step.kernel_count,
            bytes_moved: step.bytes_moved,
            plan_cache_hits: 0,
            plan_cache_misses: 0,
            gpt_oss_perf: step.perf,
        })
    }

    fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }

    fn load_duration_ns(&self) -> u64 {
        self.inner.load_duration_ns
    }

    fn backend_compatibility(&self) -> &'static str {
        GPT_OSS_CPU_BACKEND
    }
}

#[derive(Clone, Debug)]
pub struct MetalGgufGptOssGenerationModel {
    inner: Arc<GptOssMetalModelInner>,
}

impl MetalGgufGptOssGenerationModel {
    pub fn from_gguf_path(
        path: impl AsRef<Path>,
        backend: &mut MetalBackend,
    ) -> Result<Self, MetalGptOssTextGenerationError> {
        let artifact = GgufBlobArtifact::open_path(path, gpt_oss_local_blob_open_options())?;
        Self::from_blob_artifact(artifact, backend)
    }

    fn from_blob_artifact(
        artifact: GgufBlobArtifact,
        backend: &mut MetalBackend,
    ) -> Result<Self, MetalGptOssTextGenerationError> {
        let load_start = Instant::now();
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if adapter.family_metadata().family != GgufDecoderFamily::GptOss {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }

        let tokenizer = GptOssTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build gpt-oss tokenizer: {error}"),
            }
        })?;
        let output = if let Some(output) = adapter.tensor_layout().output.as_ref() {
            load_metal_quantized_matrix(backend, &artifact, output)?
        } else {
            load_metal_quantized_matrix(
                backend,
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layer| GptOssMetalLayer::load(backend, &artifact, layer))
            .collect::<Result<Vec<_>, _>>()?;
        let inner = GptOssMetalModelInner {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            decode_graph: build_gpt_oss_decode_graph(adapter.tensor_layout().layers.len()),
            token_embedding: load_quantized_matrix(
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?,
            output_norm: load_dense_vector(
                &artifact,
                adapter.tensor_layout().output_norm.as_str(),
            )?,
            output,
            layers,
            plan_digest: digest_gpt_oss_plan(
                adapter.descriptor(),
                adapter.family_metadata(),
                GPT_OSS_METAL_BACKEND,
            ),
            decode_step_plan: Mutex::new(None),
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    #[must_use]
    pub fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }
}

impl GenerationModelHandle for MetalGgufGptOssGenerationModel {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.inner.descriptor
    }

    fn cache_width(&self) -> usize {
        self.inner.cache_width()
    }
}

impl CompiledWordGenerationModel for MetalGgufGptOssGenerationModel {
    type Backend = MetalBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        &self.inner.tokenizer
    }

    fn encode_prompt_input(
        &self,
        input: &super::GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            super::GenerationInput::Text(text) => self.inner.tokenizer.encode_with_defaults(text),
            super::GenerationInput::Tokens(tokens) => tokens.clone(),
        })
    }

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.inner.tokenizer.is_end_of_sequence(token)
    }

    fn execute_step(
        &self,
        backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
    ) -> Result<super::GenerationStepOutput, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.inner.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.inner.descriptor.config.vocab_size,
            });
        }
        if position >= self.inner.descriptor.config.max_context {
            return Err(ReferenceTextGenerationError::InvalidPosition {
                position,
                max_context: self.inner.descriptor.config.max_context,
            });
        }
        if cache.width() != self.inner.cache_width() {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width: self.inner.cache_width(),
                kv_width: cache.width(),
            });
        }

        let step = self.inner.forward_step(
            backend,
            token,
            position,
            cache,
            MetalLogitsOutputMode::RawLogits,
            false,
        )?;
        Ok(super::GenerationStepOutput {
            key: step.key,
            value: step.value,
            logits: step.logits,
            execution_plan_digest: Some(self.inner.plan_digest.clone()),
            compile_path: None,
            kernel_count: step.kernel_count,
            bytes_moved: step.bytes_moved,
            plan_cache_hits: 0,
            plan_cache_misses: 0,
            gpt_oss_perf: step.perf,
        })
    }

    fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }

    fn load_duration_ns(&self) -> u64 {
        self.inner.load_duration_ns
    }

    fn backend_compatibility(&self) -> &'static str {
        GPT_OSS_METAL_BACKEND
    }
}

#[derive(Clone, Debug)]
pub struct CudaGgufGptOssGenerationModel {
    inner: Arc<GptOssCudaModelInner>,
}

#[derive(Debug, Default)]
struct CudaF16MirrorState {
    disabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct CudaLoadPlacementPolicy {
    remaining_device_bytes: Option<u64>,
    host_backed_dense_tail: bool,
    host_backed_moe: bool,
}

impl CudaLoadPlacementPolicy {
    fn from_backend(backend: &CudaBackend) -> Self {
        let remaining_device_bytes = backend
            .runtime_resources()
            .and_then(|resources| resources.device_memory_budget)
            .and_then(|budget| budget.available_execution_bytes)
            .map(|bytes| bytes.saturating_mul(80) / 100);
        Self {
            remaining_device_bytes,
            host_backed_dense_tail: false,
            host_backed_moe: false,
        }
    }

    fn reserve_device_bytes(&mut self, bytes: usize) {
        if let Some(remaining) = &mut self.remaining_device_bytes {
            *remaining = remaining.saturating_sub(bytes as u64);
        }
    }

    fn should_keep_moe_on_host(&self, projected_device_bytes: usize) -> bool {
        self.host_backed_moe
            || self
                .remaining_device_bytes
                .map(|remaining| remaining < projected_device_bytes as u64)
                .unwrap_or(false)
    }

    fn should_keep_dense_on_host(&self, projected_device_bytes: usize) -> bool {
        self.host_backed_dense_tail
            || self
                .remaining_device_bytes
                .map(|remaining| remaining < projected_device_bytes as u64)
                .unwrap_or(false)
    }

    fn should_keep_aux_dense_on_host(&self, projected_device_bytes: usize) -> bool {
        self.remaining_device_bytes
            .map(|remaining| remaining < projected_device_bytes as u64)
            .unwrap_or(false)
    }

    fn force_host_backed_moe(&mut self) {
        self.host_backed_moe = true;
    }

    fn force_host_backed_dense_tail(&mut self) {
        self.host_backed_dense_tail = true;
    }
}

fn zero_cuda_matvec_stats() -> CudaQuantizedMatvecStats {
    CudaQuantizedMatvecStats {
        host_to_device_bytes: 0,
        device_to_host_bytes: 0,
        submission_count: 0,
        sync_count: 0,
        kernel_launches: 0,
    }
}

fn cuda_quantized_matvec_with_reused_buffers(
    backend: &mut CudaBackend,
    weights: &CudaBuffer,
    byte_offset: usize,
    mode: QuantizationMode,
    rows: usize,
    cols: usize,
    input: &[f32],
    input_host_buffer: &mut CudaHostBuffer,
    input_buffer: &CudaBuffer,
    output_buffer: &CudaBuffer,
) -> Result<CudaQuantizedMatvecResult, super::RuntimeError> {
    input_host_buffer.write_f32(input)?;
    let mut submission = backend.begin_submission()?;
    submission.copy_host_to_device(input_host_buffer, input_buffer)?;
    submission.quantized_matvec(
        weights,
        byte_offset,
        mode,
        rows,
        cols,
        input_buffer,
        output_buffer,
    )?;
    let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
    let values = output_buffer.read_f32_at_offset(0, rows)?;
    Ok(CudaQuantizedMatvecResult {
        values,
        stats: CudaQuantizedMatvecStats {
            host_to_device_bytes: (input.len())
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX),
            device_to_host_bytes: (rows)
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX),
            submission_count: 1,
            sync_count: 1,
            kernel_launches: report.encoded_operations,
        },
    })
}

impl CudaGgufGptOssGenerationModel {
    pub fn from_gguf_path(
        path: impl AsRef<Path>,
        backend: &mut CudaBackend,
    ) -> Result<Self, CudaGptOssTextGenerationError> {
        let artifact = GgufBlobArtifact::open_path(path, gpt_oss_local_blob_open_options())?;
        Self::from_blob_artifact(artifact, backend)
    }

    fn from_blob_artifact(
        artifact: GgufBlobArtifact,
        backend: &mut CudaBackend,
    ) -> Result<Self, CudaGptOssTextGenerationError> {
        let load_start = Instant::now();
        let mut placement = CudaLoadPlacementPolicy::from_backend(backend);
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if adapter.family_metadata().family != GgufDecoderFamily::GptOss {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }

        let tokenizer = GptOssTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build gpt-oss tokenizer: {error}"),
            }
        })?;
        let mut f16_mirror_state = CudaF16MirrorState::default();
        let output = if let Some(output) = adapter.tensor_layout().output.as_ref() {
            load_cuda_quantized_matrix(
                backend,
                &artifact,
                output,
                false,
                &mut f16_mirror_state,
                &mut placement,
            )?
        } else {
            load_cuda_quantized_matrix(
                backend,
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
                false,
                &mut f16_mirror_state,
                &mut placement,
            )?
        };
        placement.reserve_device_bytes(output.device_residency_bytes());
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layer| {
                GptOssCudaLayer::load(
                    backend,
                    &artifact,
                    layer,
                    &mut f16_mirror_state,
                    &mut placement,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let allow_decode_plan_only_cuda =
            !placement.host_backed_moe && !placement.host_backed_dense_tail;
        let output_norm =
            load_dense_vector(&artifact, adapter.tensor_layout().output_norm.as_str())?;
        let output_norm_device = if allow_decode_plan_only_cuda {
            let buffer = upload_cuda_f32_buffer(
                backend,
                adapter.tensor_layout().output_norm.as_str(),
                output_norm.as_slice(),
            )?;
            placement.reserve_device_bytes(buffer.byte_len());
            Some(buffer)
        } else {
            None
        };
        let token_embedding_name = adapter.tensor_layout().token_embedding.as_str();
        let token_embedding_storage = artifact.paged_tensor(token_embedding_name)?;
        let token_embedding_metadata = token_embedding_storage.metadata();
        let token_embedding_dims = token_embedding_metadata.shape.dims().to_vec();
        let [token_embedding_rows, token_embedding_columns] = token_embedding_dims.as_slice()
        else {
            return Err(ModelLoadError::InvalidTensorShape {
                name: token_embedding_metadata.name.clone(),
                expected: vec![0, 0],
                actual: token_embedding_dims,
            }
            .into());
        };
        let token_embedding_layout =
            token_embedding_metadata.quantized_layout.ok_or_else(|| {
                ModelLoadError::UnsupportedTensorDType {
                    name: token_embedding_metadata.name.clone(),
                    dtype: String::from("quantized"),
                }
            })?;
        let token_embedding_row_byte_len =
            quantized_row_byte_len(&token_embedding_metadata.shape, token_embedding_layout)
                .map_err(|_| ModelLoadError::InvalidQuantizedTensorShape {
                    quantization: token_embedding_metadata.quantization,
                    shape: token_embedding_metadata.shape.dims().to_vec(),
                })?;
        let token_embedding_f16 = if allow_decode_plan_only_cuda {
            let mirror = try_build_cuda_row_major_f16_mirror(
                backend,
                token_embedding_name,
                token_embedding_metadata.quantization,
                *token_embedding_rows,
                *token_embedding_columns,
                token_embedding_row_byte_len,
                token_embedding_storage.bytes()?,
            )?;
            if let Some(buffer) = mirror.as_ref() {
                placement.reserve_device_bytes(buffer.byte_len());
            }
            mirror
        } else {
            None
        };
        let token_embedding_device = if allow_decode_plan_only_cuda {
            let matrix = load_cuda_quantized_matrix(
                backend,
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
                false,
                &mut f16_mirror_state,
                &mut placement,
            )?;
            placement.reserve_device_bytes(matrix.device_residency_bytes());
            matrix
        } else {
            CudaQuantizedMatrix {
                storage: None,
                host: load_quantized_matrix(
                    &artifact,
                    adapter.tensor_layout().token_embedding.as_str(),
                )?,
                transposed_f16: None,
                mode: token_embedding_metadata.quantization,
                rows: *token_embedding_rows,
                columns: *token_embedding_columns,
                row_byte_len: token_embedding_row_byte_len,
            }
        };
        let plan_backend = if placement.host_backed_moe || placement.host_backed_dense_tail {
            GPT_OSS_CUDA_HYBRID_MOE_BACKEND
        } else {
            GPT_OSS_CUDA_BACKEND
        };
        let inner = GptOssCudaModelInner {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            decode_graph: build_gpt_oss_decode_graph(adapter.tensor_layout().layers.len()),
            token_embedding: load_quantized_matrix(
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?,
            token_embedding_device,
            token_embedding_f16,
            output_norm,
            output_norm_device,
            output,
            layers,
            plan_digest: digest_gpt_oss_plan(
                adapter.descriptor(),
                adapter.family_metadata(),
                plan_backend,
            ),
            decode_step_plan: Mutex::new(None),
            hybrid_selected4_plan: Mutex::new(None),
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    #[must_use]
    pub fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }
}

impl GenerationModelHandle for CudaGgufGptOssGenerationModel {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.inner.descriptor
    }

    fn cache_width(&self) -> usize {
        self.inner.cache_width()
    }
}

impl CompiledWordGenerationModel for CudaGgufGptOssGenerationModel {
    type Backend = CudaBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        &self.inner.tokenizer
    }

    fn encode_prompt_input(
        &self,
        input: &super::GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            super::GenerationInput::Text(text) => self.inner.tokenizer.encode_with_defaults(text),
            super::GenerationInput::Tokens(tokens) => tokens.clone(),
        })
    }

    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.inner.tokenizer.is_end_of_sequence(token)
    }

    fn execute_step(
        &self,
        backend: &mut Self::Backend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
    ) -> Result<super::GenerationStepOutput, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.inner.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.inner.descriptor.config.vocab_size,
            });
        }
        if position >= self.inner.descriptor.config.max_context {
            return Err(ReferenceTextGenerationError::InvalidPosition {
                position,
                max_context: self.inner.descriptor.config.max_context,
            });
        }
        if cache.width() != self.inner.cache_width() {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width: self.inner.cache_width(),
                kv_width: cache.width(),
            });
        }

        let step = self.inner.forward_step(backend, token, position, cache)?;
        Ok(super::GenerationStepOutput {
            key: step.key,
            value: step.value,
            logits: step.logits,
            execution_plan_digest: Some(self.inner.plan_digest.clone()),
            compile_path: None,
            kernel_count: step.kernel_count,
            bytes_moved: step.bytes_moved,
            plan_cache_hits: 0,
            plan_cache_misses: 0,
            gpt_oss_perf: step.perf,
        })
    }

    fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }

    fn load_duration_ns(&self) -> u64 {
        self.inner.load_duration_ns
    }

    fn backend_compatibility(&self) -> &'static str {
        GPT_OSS_CUDA_BACKEND
    }
}

#[derive(Debug)]
struct GptOssCudaModelInner {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GptOssTokenizer,
    decode_graph: GptOssDecodeGraph,
    token_embedding: QuantizedMatrix,
    token_embedding_device: CudaQuantizedMatrix,
    token_embedding_f16: Option<CudaBuffer>,
    output_norm: Vec<f32>,
    output_norm_device: Option<CudaBuffer>,
    output: CudaQuantizedMatrix,
    layers: Vec<GptOssCudaLayer>,
    plan_digest: String,
    decode_step_plan: Mutex<Option<GptOssCudaStepPlan>>,
    hybrid_selected4_plan: Mutex<Option<GptOssCudaHybridSelected4Plan>>,
    load_duration_ns: u64,
}

#[derive(Clone, Debug)]
struct GptOssCudaStepPlanLayer {
    hidden_norm_buffer: CudaBuffer,
    qkv_buffer: CudaBuffer,
    attention_buffer: CudaBuffer,
    projected_buffer: CudaBuffer,
    ffn_norm_buffer: CudaBuffer,
    router_logits_buffer: CudaBuffer,
    selected_ids_buffer: CudaBuffer,
    selected_weights_buffer: CudaBuffer,
    activated_buffer: CudaBuffer,
    activated_q8_1_buffer: CudaBuffer,
    moe_projected_buffer: CudaBuffer,
    moe_buffer: CudaBuffer,
}

#[derive(Debug)]
struct GptOssCudaStepPlan {
    digest: String,
    hidden_buffer: CudaBuffer,
    decode_params_host_buffer: CudaHostBuffer,
    decode_params_buffer: CudaBuffer,
    vector_q8_1_buffer: CudaBuffer,
    vector_f16_buffer: CudaBuffer,
    layers: Vec<GptOssCudaStepPlanLayer>,
    final_norm_buffer: CudaBuffer,
    logits_buffer: CudaBuffer,
    next_token_host_buffer: CudaHostBuffer,
    next_token_buffer: CudaBuffer,
    argmax_state_host_buffer: CudaHostBuffer,
    argmax_state_buffer: CudaBuffer,
    decode_graph_exec: Option<CudaGraphExec>,
    decode_graph_cache_identity: Option<(usize, usize)>,
}

#[derive(Debug)]
struct GptOssCudaHybridSelected4LayerCache {
    slot_count: usize,
    gate_up_weights_buffer: CudaBuffer,
    down_weights_buffer: CudaBuffer,
    gate_bias_buffer: Option<CudaBuffer>,
    up_bias_buffer: Option<CudaBuffer>,
    down_bias_buffer: Option<CudaBuffer>,
    cached_experts: Vec<Option<usize>>,
    slot_last_used: Vec<u64>,
    usage_clock: u64,
}

#[derive(Debug)]
struct GptOssCudaHybridSelected4Plan {
    hidden_input_host_buffer: CudaHostBuffer,
    hidden_input_buffer: CudaBuffer,
    hidden_input_q8_1_buffer: CudaBuffer,
    qkv_host_buffer: CudaHostBuffer,
    qkv_buffer: CudaBuffer,
    attention_input_host_buffer: CudaHostBuffer,
    attention_input_buffer: CudaBuffer,
    router_logits_buffer: CudaBuffer,
    selected_expert_ids_buffer: CudaBuffer,
    selected_ids_buffer: CudaBuffer,
    selected_weights_host_buffer: CudaHostBuffer,
    selected_weights_buffer: CudaBuffer,
    gate_up_weights_host_buffer: CudaHostBuffer,
    gate_up_weights_buffer: CudaBuffer,
    down_weights_host_buffer: CudaHostBuffer,
    down_weights_buffer: CudaBuffer,
    gate_bias_host_buffer: Option<CudaHostBuffer>,
    gate_bias_buffer: Option<CudaBuffer>,
    up_bias_host_buffer: Option<CudaHostBuffer>,
    up_bias_buffer: Option<CudaBuffer>,
    down_bias_host_buffer: Option<CudaHostBuffer>,
    down_bias_buffer: Option<CudaBuffer>,
    activated_q8_1_buffer: CudaBuffer,
    projected_buffer: CudaBuffer,
    output_buffer: CudaBuffer,
    logits_buffer: CudaBuffer,
    next_token_host_buffer: CudaHostBuffer,
    next_token_buffer: CudaBuffer,
    argmax_state_host_buffer: CudaHostBuffer,
    argmax_state_buffer: CudaBuffer,
    qkv_scratch: Vec<f32>,
    gate_up_weights_scratch: Vec<u8>,
    down_weights_scratch: Vec<u8>,
    gate_bias_scratch: Vec<f32>,
    up_bias_scratch: Vec<f32>,
    down_bias_scratch: Vec<f32>,
    selected_expert_ids_scratch: Vec<i32>,
    layer_caches: Vec<Option<GptOssCudaHybridSelected4LayerCache>>,
}

impl GptOssCudaModelInner {
    fn uses_host_backed_weights(&self) -> bool {
        !self.token_embedding_device.is_device_resident()
            || !self.output.is_device_resident()
            || self.layers.iter().any(|layer| {
                layer.attention_norm_device.is_none()
                    || layer.attention_qkv_bias_device.is_none()
                    || layer.feed_forward_norm_device.is_none()
                    || layer.feed_forward_router_weight_device.is_none()
                    || layer.feed_forward_router_weight_transposed_device.is_none()
                    || !layer.attention_qkv_weight.is_device_resident()
                    || !layer.attention_output_weight.is_device_resident()
                    || !layer
                        .feed_forward_gate_up_experts_weight
                        .is_device_resident()
                    || !layer.feed_forward_down_experts_weight.is_device_resident()
            })
    }

    fn supports_cuda_decode_plan(&self) -> bool {
        !self.uses_host_backed_weights()
    }

    fn supports_hybrid_cuda_device_argmax_fast_path(&self) -> bool {
        self.family_metadata.expert_used_count == Some(4)
            && self.layers.iter().any(|layer| {
                layer.feed_forward_gate_up_experts_weight.host.is_some()
                    && layer.feed_forward_down_experts_weight.host.is_some()
            })
            && self.layers.iter().all(|layer| {
                can_use_hybrid_cuda_hidden_residency_layer(
                    layer,
                    self.descriptor.config.hidden_size,
                )
            })
            && self.output.storage.is_some()
            && self.output_norm_device.is_some()
    }

    fn cache_width(&self) -> usize {
        self.descriptor
            .config
            .layer_count
            .saturating_mul(self.descriptor.config.kv_width())
    }

    fn graph_node_count(&self) -> usize {
        self.decode_graph.node_count()
    }

    fn graph_layer_node_count(&self) -> usize {
        self.decode_graph.layer_node_count()
    }

    fn acquire_decode_step_plan(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<(GptOssCudaStepPlan, CompilePathEvidence, bool), ReferenceTextGenerationError> {
        if !self.supports_cuda_decode_plan() {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(String::from(
                    "cuda gpt-oss decode-step plan requires fully device-resident expert tensors",
                )),
            ));
        }
        let cache_hit = self
            .decode_step_plan
            .lock()
            .map_err(|_| {
                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                    "cuda gpt-oss decode-step plan cache is poisoned",
                )))
            })?
            .take();
        if let Some(plan) = cache_hit {
            return Ok((plan, decode_step_plan_compile_path(true), true));
        }
        Ok((
            self.build_decode_step_plan(backend)?,
            decode_step_plan_compile_path(false),
            false,
        ))
    }

    fn release_decode_step_plan(&self, plan: GptOssCudaStepPlan) {
        if let Ok(mut cached_plan) = self.decode_step_plan.lock() {
            *cached_plan = Some(plan);
        }
    }

    fn acquire_hybrid_selected4_plan(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<Option<GptOssCudaHybridSelected4Plan>, ReferenceTextGenerationError> {
        if self.family_metadata.expert_used_count != Some(4) {
            return Ok(None);
        }
        let mut cached_plan = self.hybrid_selected4_plan.lock().map_err(|_| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda gpt-oss hybrid selected4 plan cache is poisoned",
            )))
        })?;
        if let Some(plan) = cached_plan.take() {
            return Ok(Some(plan));
        }
        self.build_hybrid_selected4_plan(backend).map(Some)
    }

    fn release_hybrid_selected4_plan(&self, plan: GptOssCudaHybridSelected4Plan) {
        if let Ok(mut cached_plan) = self.hybrid_selected4_plan.lock() {
            *cached_plan = Some(plan);
        }
    }

    fn build_hybrid_selected4_plan(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<GptOssCudaHybridSelected4Plan, ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let selected_count = self.family_metadata.expert_used_count.ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda gpt-oss hybrid selected4 plan requires expert_used_count metadata",
            )))
        })?;
        if selected_count != 4 {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(format!(
                    "cuda gpt-oss hybrid selected4 plan requires expert_used_count=4, actual {selected_count}",
                )),
            ));
        }

        let mut gate_up_bytes = 0usize;
        let mut down_bytes = 0usize;
        let mut qkv_values = 0usize;
        let mut attention_input_values = hidden_size;
        let mut router_rows = 0usize;
        let mut gate_bias_values = 0usize;
        let mut up_bias_values = 0usize;
        let mut down_bias_values = 0usize;
        let mut activated_q8_1_bytes = 0usize;
        let mut projected_values = 0usize;
        let mut has_host_backed_selected4 = false;

        for layer in &self.layers {
            let (Some(gate_up_host), Some(down_host)) = (
                layer.feed_forward_gate_up_experts_weight.host.as_ref(),
                layer.feed_forward_down_experts_weight.host.as_ref(),
            ) else {
                continue;
            };
            has_host_backed_selected4 = true;
            let gate_rows = gate_up_host.gate.rows;
            let up_rows = gate_up_host.up.rows;
            qkv_values = qkv_values.max(layer.attention_qkv_weight.total_rows());
            attention_input_values =
                attention_input_values.max(layer.attention_output_weight.columns);
            router_rows = router_rows.max(layer.feed_forward_router_weight.rows);
            gate_up_bytes = gate_up_bytes.max(
                selected_count
                    .saturating_mul(gate_rows.saturating_add(up_rows))
                    .saturating_mul(gate_up_host.gate.row_byte_len),
            );
            down_bytes = down_bytes.max(
                selected_count
                    .saturating_mul(down_host.rows)
                    .saturating_mul(down_host.row_byte_len),
            );
            activated_q8_1_bytes = activated_q8_1_bytes.max(
                ggml_q8_1_storage_bytes(selected_count, gate_rows)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            );
            projected_values = projected_values.max(selected_count.saturating_mul(down_host.rows));
            if layer.feed_forward_gate_experts_bias.is_some() {
                gate_bias_values = gate_bias_values.max(selected_count.saturating_mul(gate_rows));
            }
            if layer.feed_forward_up_experts_bias.is_some() {
                up_bias_values = up_bias_values.max(selected_count.saturating_mul(up_rows));
            }
            if layer.feed_forward_down_experts_bias.is_some() {
                down_bias_values =
                    down_bias_values.max(selected_count.saturating_mul(down_host.rows));
            }
        }

        if !has_host_backed_selected4 {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(String::from(
                    "cuda gpt-oss hybrid selected4 plan requested without host-backed selected4 layers",
                )),
            ));
        }

        let vector_q8_1_columns = hidden_size
            .max(attention_input_values)
            .max(self.output.columns);

        let hidden_input_host_buffer = backend
            .host_buffer(hidden_size.saturating_mul(std::mem::size_of::<f32>()))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let hidden_input_buffer = backend
            .f32_buffer(hidden_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let hidden_input_q8_1_buffer = backend
            .byte_buffer(&vec![
                0_u8;
                ggml_q8_1_storage_bytes(1, vector_q8_1_columns)
                    .map_err(ReferenceTextGenerationError::Runtime)?
            ])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let qkv_host_buffer = backend
            .host_buffer(qkv_values.saturating_mul(std::mem::size_of::<f32>()))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let qkv_buffer = backend
            .f32_buffer(qkv_values)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let attention_input_host_buffer = backend
            .host_buffer(attention_input_values.saturating_mul(std::mem::size_of::<f32>()))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let attention_input_buffer = backend
            .f32_buffer(attention_input_values)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let router_logits_buffer = backend
            .f32_buffer(router_rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let selected_expert_ids_buffer = backend
            .i32_buffer(selected_count)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let selected_ids = (0..selected_count)
            .map(|index| {
                i32::try_from(index).map_err(|_| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                        "selected expert index {index} exceeds i32 range",
                    )))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected_ids_buffer = backend
            .byte_buffer(i32_slice_to_ne_bytes(selected_ids.as_slice()).as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let selected_weights_host_buffer = backend
            .host_buffer(selected_count.saturating_mul(std::mem::size_of::<f32>()))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let selected_weights_buffer = backend
            .f32_buffer(selected_count)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let gate_up_weights_host_buffer = backend
            .host_buffer(gate_up_bytes)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let gate_up_weights_buffer = backend
            .byte_buffer(&vec![0_u8; gate_up_bytes])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let down_weights_host_buffer = backend
            .host_buffer(down_bytes)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let down_weights_buffer = backend
            .byte_buffer(&vec![0_u8; down_bytes])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let gate_bias_host_buffer = if gate_bias_values > 0 {
            Some(
                backend
                    .host_buffer(gate_bias_values.saturating_mul(std::mem::size_of::<f32>()))
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let gate_bias_buffer = if gate_bias_values > 0 {
            Some(
                backend
                    .f32_buffer(gate_bias_values)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let up_bias_host_buffer = if up_bias_values > 0 {
            Some(
                backend
                    .host_buffer(up_bias_values.saturating_mul(std::mem::size_of::<f32>()))
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let up_bias_buffer = if up_bias_values > 0 {
            Some(
                backend
                    .f32_buffer(up_bias_values)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let down_bias_host_buffer = if down_bias_values > 0 {
            Some(
                backend
                    .host_buffer(down_bias_values.saturating_mul(std::mem::size_of::<f32>()))
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let down_bias_buffer = if down_bias_values > 0 {
            Some(
                backend
                    .f32_buffer(down_bias_values)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )
        } else {
            None
        };
        let activated_q8_1_buffer = backend
            .byte_buffer(&vec![0_u8; activated_q8_1_bytes])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let projected_buffer = backend
            .f32_buffer(projected_values)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let output_buffer = backend
            .f32_buffer(hidden_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let logits_buffer = backend
            .f32_buffer(self.output.rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let next_token_host_buffer = backend
            .host_buffer(std::mem::size_of::<i32>())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let next_token_buffer = backend
            .byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let argmax_state_host_buffer = backend
            .host_buffer(std::mem::size_of::<u64>())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let argmax_state_buffer = backend
            .byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let mut layer_caches = Vec::with_capacity(self.layers.len());
        let mut layer_cache_oom = false;
        for (layer_index, layer) in self.layers.iter().enumerate() {
            let (Some(gate_up_host), Some(down_host)) = (
                layer.feed_forward_gate_up_experts_weight.host.as_ref(),
                layer.feed_forward_down_experts_weight.host.as_ref(),
            ) else {
                layer_caches.push(None);
                continue;
            };
            if layer_cache_oom {
                layer_caches.push(None);
                continue;
            }
            let layer_cache_slots =
                hybrid_selected4_layer_cache_slots_for_model(layer_index, self.layers.len());
            let gate_rows = gate_up_host.gate.rows;
            let up_rows = gate_up_host.up.rows;
            let gate_up_bytes = layer_cache_slots
                .saturating_mul(gate_rows.saturating_add(up_rows))
                .saturating_mul(gate_up_host.gate.row_byte_len);
            let down_bytes = layer_cache_slots
                .saturating_mul(down_host.rows)
                .saturating_mul(down_host.row_byte_len);
            let layer_cache: Result<GptOssCudaHybridSelected4LayerCache, super::RuntimeError> =
                (|| {
                    Ok(GptOssCudaHybridSelected4LayerCache {
                        slot_count: layer_cache_slots,
                        gate_up_weights_buffer: backend.byte_buffer(&vec![0_u8; gate_up_bytes])?,
                        down_weights_buffer: backend.byte_buffer(&vec![0_u8; down_bytes])?,
                        gate_bias_buffer: if layer.feed_forward_gate_experts_bias.is_some() {
                            Some(backend.f32_buffer(layer_cache_slots.saturating_mul(gate_rows))?)
                        } else {
                            None
                        },
                        up_bias_buffer: if layer.feed_forward_up_experts_bias.is_some() {
                            Some(backend.f32_buffer(layer_cache_slots.saturating_mul(up_rows))?)
                        } else {
                            None
                        },
                        down_bias_buffer: if layer.feed_forward_down_experts_bias.is_some() {
                            Some(
                                backend
                                    .f32_buffer(layer_cache_slots.saturating_mul(down_host.rows))?,
                            )
                        } else {
                            None
                        },
                        cached_experts: vec![None; layer_cache_slots],
                        slot_last_used: vec![0; layer_cache_slots],
                        usage_clock: 1,
                    })
                })();
            match layer_cache {
                Ok(layer_cache) => layer_caches.push(Some(layer_cache)),
                Err(error) => {
                    if error.to_string().contains("out of memory") {
                        layer_cache_oom = true;
                        layer_caches.push(None);
                    } else {
                        return Err(ReferenceTextGenerationError::Runtime(error));
                    }
                }
            }
        }

        Ok(GptOssCudaHybridSelected4Plan {
            hidden_input_host_buffer,
            hidden_input_buffer,
            hidden_input_q8_1_buffer,
            qkv_host_buffer,
            qkv_buffer,
            attention_input_host_buffer,
            attention_input_buffer,
            router_logits_buffer,
            selected_expert_ids_buffer,
            selected_ids_buffer,
            selected_weights_host_buffer,
            selected_weights_buffer,
            gate_up_weights_host_buffer,
            gate_up_weights_buffer,
            down_weights_host_buffer,
            down_weights_buffer,
            gate_bias_host_buffer,
            gate_bias_buffer,
            up_bias_host_buffer,
            up_bias_buffer,
            down_bias_host_buffer,
            down_bias_buffer,
            activated_q8_1_buffer,
            projected_buffer,
            output_buffer,
            logits_buffer,
            next_token_host_buffer,
            next_token_buffer,
            argmax_state_host_buffer,
            argmax_state_buffer,
            qkv_scratch: Vec::with_capacity(qkv_values),
            gate_up_weights_scratch: Vec::with_capacity(gate_up_bytes),
            down_weights_scratch: Vec::with_capacity(down_bytes),
            gate_bias_scratch: Vec::with_capacity(gate_bias_values),
            up_bias_scratch: Vec::with_capacity(up_bias_values),
            down_bias_scratch: Vec::with_capacity(down_bias_values),
            selected_expert_ids_scratch: Vec::with_capacity(selected_count),
            layer_caches,
        })
    }

    fn build_decode_step_plan(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<GptOssCudaStepPlan, ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let selected_count = self.family_metadata.expert_used_count.ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda gpt-oss path requires expert_used_count metadata",
            )))
        })?;
        let mut vector_q8_1_columns = hidden_size.max(self.output.columns);
        let mut layers = Vec::with_capacity(self.layers.len());
        for layer in &self.layers {
            let gate_rows = layer
                .feed_forward_gate_up_experts_weight
                .rows_per_projection[0];
            let up_rows = layer
                .feed_forward_gate_up_experts_weight
                .rows_per_projection[1];
            if gate_rows != up_rows {
                return Err(ReferenceTextGenerationError::Runtime(
                    super::RuntimeError::Backend(format!(
                        "cuda gpt-oss MoE path requires matching gate/up expert widths, got gate {} and up {}",
                        gate_rows, up_rows
                    )),
                ));
            }
            vector_q8_1_columns = vector_q8_1_columns
                .max(layer.attention_output_weight.columns)
                .max(layer.feed_forward_gate_up_experts_weight.columns);
            let activated_q8_1_bytes = ggml_q8_1_storage_bytes(selected_count, gate_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            layers.push(GptOssCudaStepPlanLayer {
                hidden_norm_buffer: backend.f32_buffer(hidden_size)?,
                qkv_buffer: backend.f32_buffer(layer.attention_qkv_weight.total_rows())?,
                attention_buffer: backend.f32_buffer(layer.attention_output_weight.columns)?,
                projected_buffer: backend.f32_buffer(layer.attention_output_weight.rows)?,
                ffn_norm_buffer: backend.f32_buffer(hidden_size)?,
                router_logits_buffer: backend.f32_buffer(layer.feed_forward_router_weight.rows)?,
                selected_ids_buffer: backend.i32_buffer(selected_count)?,
                selected_weights_buffer: backend.f32_buffer(selected_count)?,
                activated_buffer: backend.f32_buffer(selected_count.saturating_mul(gate_rows))?,
                activated_q8_1_buffer: backend.byte_buffer(&vec![0_u8; activated_q8_1_bytes])?,
                moe_projected_buffer: backend.f32_buffer(
                    selected_count.saturating_mul(layer.feed_forward_down_experts_weight.rows),
                )?,
                moe_buffer: backend.f32_buffer(hidden_size)?,
            });
        }
        let vector_q8_1_bytes = ggml_q8_1_storage_bytes(1, vector_q8_1_columns)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok(GptOssCudaStepPlan {
            digest: digest_gpt_oss_cuda_step_plan(
                self.plan_digest.as_str(),
                self.decode_graph.signature_key().as_str(),
            ),
            hidden_buffer: backend.f32_buffer(hidden_size)?,
            decode_params_host_buffer: backend.host_buffer(3 * std::mem::size_of::<i32>())?,
            decode_params_buffer: backend.i32_buffer(3)?,
            vector_q8_1_buffer: backend.byte_buffer(&vec![0_u8; vector_q8_1_bytes])?,
            vector_f16_buffer: backend.f16_buffer(vector_q8_1_columns)?,
            layers,
            final_norm_buffer: backend.f32_buffer(hidden_size)?,
            logits_buffer: backend.f32_buffer(self.output.rows)?,
            next_token_host_buffer: backend.host_buffer(std::mem::size_of::<i32>())?,
            next_token_buffer: backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?,
            argmax_state_host_buffer: backend.host_buffer(std::mem::size_of::<u64>())?,
            argmax_state_buffer: backend.byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])?,
            decode_graph_exec: None,
            decode_graph_cache_identity: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_cuda_forward_step_submission(
        &self,
        submission: &mut CudaSubmission,
        position: usize,
        cache_write_index: usize,
        cuda_cache: &CudaKvCacheMirror,
        plan: &mut GptOssCudaStepPlan,
        output_mode: CudaStepOutputMode,
        perf: &mut GptOssPerformanceMetrics,
        bytes_moved: &mut u64,
        use_graph_attention: bool,
    ) -> Result<(), ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let kv_width = self.descriptor.config.kv_width();
        let head_count = self.descriptor.config.block.attention.head_count;
        let kv_head_count = self.descriptor.config.block.attention.kv_head_count;
        let head_dim = self.descriptor.config.block.attention.head_dim;
        let rotary_dim = self.descriptor.config.block.attention.rotary_dim;
        let (freq_scale, ext_factor, corr_dims, theta_scale) =
            rope_runtime_parameters(rotary_dim, &self.family_metadata);
        let selected_count = self.family_metadata.expert_used_count.ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda gpt-oss path requires expert_used_count metadata",
            )))
        })?;

        submission
            .copy_host_to_device(&plan.decode_params_host_buffer, &plan.decode_params_buffer)?;
        let token_embedding_start = Instant::now();
        if let Some(token_embedding_f16) = self.token_embedding_f16.as_ref() {
            submission.gather_f16_row_to_f32(
                token_embedding_f16,
                self.token_embedding_device.rows,
                self.token_embedding_device.columns,
                &plan.decode_params_buffer,
                &plan.hidden_buffer,
            )?;
        } else {
            submission.dequantize_row_to_f32(
                self.token_embedding_device
                    .device_storage()
                    .map_err(ReferenceTextGenerationError::Runtime)?,
                self.token_embedding_device.mode,
                self.token_embedding_device.rows,
                self.token_embedding_device.row_byte_len,
                self.token_embedding_device.columns,
                &plan.decode_params_buffer,
                &plan.hidden_buffer,
            )?;
        }
        perf.stage_timings.token_embedding_ns = perf
            .stage_timings
            .token_embedding_ns
            .saturating_add(duration_ns(token_embedding_start));

        for (layer_index, layer) in self.layers.iter().enumerate() {
            let current_hidden = if layer_index == 0 {
                &plan.hidden_buffer
            } else {
                &plan.layers[layer_index.saturating_sub(1)].moe_buffer
            };
            let layer_plan = &plan.layers[layer_index];
            let layer_offset = layer_index.saturating_mul(kv_width);
            let q_rows = layer.attention_qkv_weight.rows_per_projection[0];
            let k_rows = layer.attention_qkv_weight.rows_per_projection[1];
            let gate_rows = layer
                .feed_forward_gate_up_experts_weight
                .rows_per_projection[0];
            let up_rows = layer
                .feed_forward_gate_up_experts_weight
                .rows_per_projection[1];
            if gate_rows != up_rows {
                return Err(ReferenceTextGenerationError::Runtime(
                    super::RuntimeError::Backend(format!(
                        "cuda gpt-oss MoE path requires matching gate/up expert widths, got gate {} and up {}",
                        gate_rows, up_rows
                    )),
                ));
            }

            let layer_start = Instant::now();
            let use_q8_1_attention_output_fusion =
                layer.attention_output_weight.transposed_f16.is_none()
                    && can_use_q8_1_mmvq(layer.attention_output_weight.mode)
                    && can_use_q8_1_attention_output_fusion(
                        layer.attention_output_weight.columns,
                        head_count,
                        head_dim,
                    );
            if let Some(transposed_f16) = layer.attention_qkv_weight.transposed_f16.as_ref() {
                submission.rms_norm(
                    current_hidden,
                    layer.attention_norm_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "cuda decode-step plan requires device attention norm buffer",
                            ),
                        ))
                    })?,
                    &layer_plan.hidden_norm_buffer,
                    hidden_size,
                    self.family_metadata.rms_norm_epsilon,
                )?;
                submission.cast_f32_to_f16(
                    &layer_plan.hidden_norm_buffer,
                    &plan.vector_f16_buffer,
                    hidden_size,
                )?;
                submission.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &layer_plan.qkv_buffer,
                    1,
                    layer.attention_qkv_weight.columns,
                    layer.attention_qkv_weight.total_rows(),
                )?;
                submission.add_f32_in_place(
                    &layer_plan.qkv_buffer,
                    0,
                    layer.attention_qkv_bias_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from("cuda decode-step plan requires device qkv bias buffer"),
                        ))
                    })?,
                    layer.attention_qkv_weight.total_rows(),
                )?;
            } else if can_use_q8_1_mmvq(layer.attention_qkv_weight.mode)
                && can_use_q8_1_norm_fusion(hidden_size)
            {
                submission.rms_norm_q8_1(
                    current_hidden,
                    layer.attention_norm_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "cuda decode-step plan requires device attention norm buffer",
                            ),
                        ))
                    })?,
                    &plan.vector_q8_1_buffer,
                    hidden_size,
                    self.family_metadata.rms_norm_epsilon,
                )?;
                submission.quantized_matvec_q8_1(
                    layer
                        .attention_qkv_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_qkv_weight.mode,
                    layer.attention_qkv_weight.total_rows(),
                    layer.attention_qkv_weight.columns,
                    &plan.vector_q8_1_buffer,
                    Some(layer.attention_qkv_bias_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from("cuda decode-step plan requires device qkv bias buffer"),
                        ))
                    })?),
                    &layer_plan.qkv_buffer,
                )?;
            } else if can_use_q8_1_mmvq(layer.attention_qkv_weight.mode) {
                submission.rms_norm(
                    current_hidden,
                    layer.attention_norm_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "cuda decode-step plan requires device attention norm buffer",
                            ),
                        ))
                    })?,
                    &layer_plan.hidden_norm_buffer,
                    hidden_size,
                    self.family_metadata.rms_norm_epsilon,
                )?;
                submission.quantize_f32_to_q8_1(
                    &layer_plan.hidden_norm_buffer,
                    1,
                    hidden_size,
                    &plan.vector_q8_1_buffer,
                )?;
                submission.quantized_matvec_q8_1(
                    layer
                        .attention_qkv_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_qkv_weight.mode,
                    layer.attention_qkv_weight.total_rows(),
                    layer.attention_qkv_weight.columns,
                    &plan.vector_q8_1_buffer,
                    Some(layer.attention_qkv_bias_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from("cuda decode-step plan requires device qkv bias buffer"),
                        ))
                    })?),
                    &layer_plan.qkv_buffer,
                )?;
            } else {
                submission.rms_norm(
                    current_hidden,
                    layer.attention_norm_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "cuda decode-step plan requires device attention norm buffer",
                            ),
                        ))
                    })?,
                    &layer_plan.hidden_norm_buffer,
                    hidden_size,
                    self.family_metadata.rms_norm_epsilon,
                )?;
                submission.quantized_matvec(
                    layer
                        .attention_qkv_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_qkv_weight.mode,
                    layer.attention_qkv_weight.total_rows(),
                    layer.attention_qkv_weight.columns,
                    &layer_plan.hidden_norm_buffer,
                    &layer_plan.qkv_buffer,
                )?;
                submission.add_f32_in_place(
                    &layer_plan.qkv_buffer,
                    0,
                    layer.attention_qkv_bias_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from("cuda decode-step plan requires device qkv bias buffer"),
                        ))
                    })?,
                    layer.attention_qkv_weight.total_rows(),
                )?;
            }
            if use_graph_attention {
                if use_q8_1_attention_output_fusion {
                    submission.attention_decode_rope_cache_f16_kv_graph_q8_1(
                        &layer_plan.qkv_buffer,
                        0,
                        q_rows,
                        q_rows.saturating_add(k_rows),
                        &cuda_cache.key_buffer,
                        &cuda_cache.value_buffer,
                        cuda_cache.width,
                        layer_offset,
                        &plan.decode_params_buffer,
                        self.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        layer.attention_sinks_device.as_ref(),
                        &plan.vector_q8_1_buffer,
                    )?;
                } else {
                    submission.attention_decode_rope_cache_f16_kv_graph(
                        &layer_plan.qkv_buffer,
                        0,
                        q_rows,
                        q_rows.saturating_add(k_rows),
                        &cuda_cache.key_buffer,
                        &cuda_cache.value_buffer,
                        cuda_cache.width,
                        layer_offset,
                        &plan.decode_params_buffer,
                        self.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        layer.attention_sinks_device.as_ref(),
                        &layer_plan.attention_buffer,
                    )?;
                }
            } else {
                if use_q8_1_attention_output_fusion {
                    submission.attention_decode_rope_cache_f16_kv_q8_1(
                        &layer_plan.qkv_buffer,
                        0,
                        q_rows,
                        q_rows.saturating_add(k_rows),
                        &cuda_cache.key_buffer,
                        &cuda_cache.value_buffer,
                        cuda_cache.width,
                        layer_offset,
                        cache_write_index,
                        self.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        layer.attention_sinks_device.as_ref(),
                        &plan.vector_q8_1_buffer,
                    )?;
                } else {
                    submission.attention_decode_rope_cache_f16_kv(
                        &layer_plan.qkv_buffer,
                        0,
                        q_rows,
                        q_rows.saturating_add(k_rows),
                        &cuda_cache.key_buffer,
                        &cuda_cache.value_buffer,
                        cuda_cache.width,
                        layer_offset,
                        cache_write_index,
                        self.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        layer.attention_sinks_device.as_ref(),
                        &layer_plan.attention_buffer,
                    )?;
                }
            }
            if let Some(transposed_f16) = layer.attention_output_weight.transposed_f16.as_ref() {
                submission.cast_f32_to_f16(
                    &layer_plan.attention_buffer,
                    &plan.vector_f16_buffer,
                    layer.attention_output_weight.columns,
                )?;
                submission.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &layer_plan.projected_buffer,
                    1,
                    layer.attention_output_weight.columns,
                    layer.attention_output_weight.rows,
                )?;
            } else if use_q8_1_attention_output_fusion {
                submission.quantized_matvec_q8_1(
                    layer
                        .attention_output_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_output_weight.mode,
                    layer.attention_output_weight.rows,
                    layer.attention_output_weight.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &layer_plan.projected_buffer,
                )?;
            } else if can_use_q8_1_mmvq(layer.attention_output_weight.mode) {
                submission.quantize_f32_to_q8_1(
                    &layer_plan.attention_buffer,
                    1,
                    layer.attention_output_weight.columns,
                    &plan.vector_q8_1_buffer,
                )?;
                submission.quantized_matvec_q8_1(
                    layer
                        .attention_output_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_output_weight.mode,
                    layer.attention_output_weight.rows,
                    layer.attention_output_weight.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &layer_plan.projected_buffer,
                )?;
            } else {
                submission.quantized_matvec(
                    layer
                        .attention_output_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    layer.attention_output_weight.mode,
                    layer.attention_output_weight.rows,
                    layer.attention_output_weight.columns,
                    &layer_plan.attention_buffer,
                    &layer_plan.projected_buffer,
                )?;
            }
            if can_use_q8_1_mmvq(layer.feed_forward_gate_up_experts_weight.mode)
                && can_use_q8_1_mmvq(layer.feed_forward_down_experts_weight.mode)
            {
                if false {
                    submission.add_residual_rms_norm_q8_1_router_topk(
                        &layer_plan.projected_buffer,
                        current_hidden,
                        layer.attention_output_bias_device.as_ref(),
                        layer
                            .feed_forward_norm_device
                            .as_ref()
                            .ok_or_else(|| {
                                ReferenceTextGenerationError::Runtime(
                                    super::RuntimeError::Backend(String::from(
                                        "cuda decode-step plan requires device feed-forward norm buffer",
                                    )),
                                )
                            })?,
                        &layer_plan.projected_buffer,
                        &layer_plan.ffn_norm_buffer,
                        &plan.vector_q8_1_buffer,
                        layer
                            .feed_forward_router_weight_device
                            .as_ref()
                            .ok_or_else(|| {
                                ReferenceTextGenerationError::Runtime(
                                    super::RuntimeError::Backend(String::from(
                                        "cuda decode-step plan requires device router weight buffer",
                                    )),
                                )
                            })?,
                        layer.feed_forward_router_bias_device.as_ref(),
                        layer.feed_forward_router_weight.rows,
                        selected_count,
                        &layer_plan.selected_ids_buffer,
                        &layer_plan.selected_weights_buffer,
                        hidden_size,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                } else {
                    submission.add_residual_rms_norm(
                        &layer_plan.projected_buffer,
                        current_hidden,
                        layer.attention_output_bias_device.as_ref(),
                        layer
                            .feed_forward_norm_device
                            .as_ref()
                            .ok_or_else(|| {
                                ReferenceTextGenerationError::Runtime(
                                    super::RuntimeError::Backend(String::from(
                                        "cuda decode-step plan requires device feed-forward norm buffer",
                                    )),
                                )
                            })?,
                        &layer_plan.projected_buffer,
                        &layer_plan.ffn_norm_buffer,
                        hidden_size,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.matmul(
                        &layer_plan.ffn_norm_buffer,
                        layer
                            .feed_forward_router_weight_transposed_device
                            .as_ref()
                            .ok_or_else(|| {
                                ReferenceTextGenerationError::Runtime(
                                    super::RuntimeError::Backend(String::from(
                                        "cuda decode-step plan requires device transposed router weight buffer",
                                    )),
                                )
                            })?,
                        &layer_plan.router_logits_buffer,
                        1,
                        layer.feed_forward_router_weight.columns,
                        layer.feed_forward_router_weight.rows,
                    )?;
                    if let Some(bias) = layer.feed_forward_router_bias_device.as_ref() {
                        submission.add_f32_in_place(
                            &layer_plan.router_logits_buffer,
                            0,
                            bias,
                            layer.feed_forward_router_weight.rows,
                        )?;
                    }
                    submission.router_topk_delayed_softmax(
                        &layer_plan.router_logits_buffer,
                        layer.feed_forward_router_weight.rows,
                        selected_count,
                        &layer_plan.selected_ids_buffer,
                        &layer_plan.selected_weights_buffer,
                    )?;
                }
                let use_selected4_quantized_gate_up = selected_count <= 4
                    && !experimental_fused_selected4_moe_down_enabled()
                    && layer.feed_forward_gate_up_experts_weight.columns % 32 == 0;
                let use_ids_driven_expert_matvec = use_selected4_quantized_gate_up;
                submission.quantize_f32_to_q8_1(
                    &layer_plan.ffn_norm_buffer,
                    1,
                    layer.feed_forward_gate_up_experts_weight.columns,
                    &plan.vector_q8_1_buffer,
                )?;
                if use_selected4_quantized_gate_up {
                    submission.expert_gate_up_swiglu_q8_1_ids(
                        layer
                            .feed_forward_gate_up_experts_weight
                            .device_storage()
                            .map_err(ReferenceTextGenerationError::Runtime)?,
                        layer.feed_forward_gate_up_experts_weight.mode,
                        layer.feed_forward_gate_up_experts_weight.row_byte_len,
                        layer.feed_forward_gate_up_experts_weight.total_rows(),
                        layer.feed_forward_gate_up_experts_weight.columns,
                        gate_rows,
                        up_rows,
                        &layer_plan.selected_ids_buffer,
                        selected_count,
                        &plan.vector_q8_1_buffer,
                        layer.feed_forward_gate_experts_bias_device.as_ref(),
                        layer.feed_forward_up_experts_bias_device.as_ref(),
                        &layer_plan.activated_q8_1_buffer,
                    )?;
                } else {
                    submission.moe_gate_up_swiglu_q8_1(
                        layer
                            .feed_forward_gate_up_experts_weight
                            .device_storage()
                            .map_err(ReferenceTextGenerationError::Runtime)?,
                        layer.feed_forward_gate_up_experts_weight.mode,
                        layer.feed_forward_gate_up_experts_weight.row_byte_len,
                        layer.feed_forward_gate_up_experts_weight.total_rows(),
                        layer.feed_forward_gate_up_experts_weight.columns,
                        gate_rows,
                        up_rows,
                        &layer_plan.selected_ids_buffer,
                        selected_count,
                        &plan.vector_q8_1_buffer,
                        layer.feed_forward_gate_experts_bias_device.as_ref(),
                        layer.feed_forward_up_experts_bias_device.as_ref(),
                        &layer_plan.activated_buffer,
                    )?;
                }
                if selected_count <= 4 && experimental_fused_selected4_moe_down_enabled() {
                    submission.moe_down_aggregate_q8_1_f32(
                        layer
                            .feed_forward_down_experts_weight
                            .device_storage()
                            .map_err(ReferenceTextGenerationError::Runtime)?,
                        layer.feed_forward_down_experts_weight.mode,
                        layer.feed_forward_down_experts_weight.row_byte_len,
                        layer.feed_forward_down_experts_weight.rows,
                        layer.feed_forward_down_experts_weight.columns,
                        &layer_plan.selected_ids_buffer,
                        &layer_plan.selected_weights_buffer,
                        selected_count,
                        &layer_plan.activated_buffer,
                        layer.feed_forward_down_experts_bias_device.as_ref(),
                        Some(&layer_plan.projected_buffer),
                        &layer_plan.moe_buffer,
                    )?;
                } else if use_selected4_quantized_gate_up {
                    if use_ids_driven_expert_matvec {
                        perf.cuda.grouped_expert_ids_path = true;
                        submission.expert_matvec_q8_1_ids(
                            layer
                                .feed_forward_down_experts_weight
                                .device_storage()
                                .map_err(ReferenceTextGenerationError::Runtime)?,
                            layer.feed_forward_down_experts_weight.mode,
                            layer.feed_forward_down_experts_weight.row_byte_len,
                            layer.feed_forward_down_experts_weight.rows,
                            layer.feed_forward_down_experts_weight.columns,
                            &layer_plan.selected_ids_buffer,
                            selected_count,
                            &layer_plan.activated_q8_1_buffer,
                            layer.feed_forward_down_experts_bias_device.as_ref(),
                            &layer_plan.moe_projected_buffer,
                        )?;
                        submission.accumulate_expert_outputs(
                            &layer_plan.moe_projected_buffer,
                            &layer_plan.selected_weights_buffer,
                            selected_count,
                            layer.feed_forward_down_experts_weight.rows,
                            Some(&layer_plan.projected_buffer),
                            &layer_plan.moe_buffer,
                        )?;
                    } else {
                        submission.moe_down_aggregate_q8_1(
                            layer
                                .feed_forward_down_experts_weight
                                .device_storage()
                                .map_err(ReferenceTextGenerationError::Runtime)?,
                            layer.feed_forward_down_experts_weight.mode,
                            layer.feed_forward_down_experts_weight.row_byte_len,
                            layer.feed_forward_down_experts_weight.rows,
                            layer.feed_forward_down_experts_weight.columns,
                            &layer_plan.selected_ids_buffer,
                            &layer_plan.selected_weights_buffer,
                            selected_count,
                            &layer_plan.activated_q8_1_buffer,
                            layer.feed_forward_down_experts_bias_device.as_ref(),
                            Some(&layer_plan.projected_buffer),
                            &layer_plan.moe_buffer,
                        )?;
                    }
                } else {
                    submission.quantize_f32_to_q8_1(
                        &layer_plan.activated_buffer,
                        selected_count,
                        layer.feed_forward_down_experts_weight.columns,
                        &layer_plan.activated_q8_1_buffer,
                    )?;
                    submission.moe_down_aggregate_q8_1(
                        layer
                            .feed_forward_down_experts_weight
                            .device_storage()
                            .map_err(ReferenceTextGenerationError::Runtime)?,
                        layer.feed_forward_down_experts_weight.mode,
                        layer.feed_forward_down_experts_weight.row_byte_len,
                        layer.feed_forward_down_experts_weight.rows,
                        layer.feed_forward_down_experts_weight.columns,
                        &layer_plan.selected_ids_buffer,
                        &layer_plan.selected_weights_buffer,
                        selected_count,
                        &layer_plan.activated_q8_1_buffer,
                        layer.feed_forward_down_experts_bias_device.as_ref(),
                        Some(&layer_plan.projected_buffer),
                        &layer_plan.moe_buffer,
                    )?;
                }
            } else {
                submission.add_residual_rms_norm(
                    &layer_plan.projected_buffer,
                    current_hidden,
                    layer.attention_output_bias_device.as_ref(),
                    layer.feed_forward_norm_device.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            String::from(
                                "cuda decode-step plan requires device feed-forward norm buffer",
                            ),
                        ))
                    })?,
                    &layer_plan.projected_buffer,
                    &layer_plan.ffn_norm_buffer,
                    hidden_size,
                    self.family_metadata.rms_norm_epsilon,
                )?;
                submission.matmul(
                    &layer_plan.ffn_norm_buffer,
                    layer
                        .feed_forward_router_weight_transposed_device
                        .as_ref()
                        .ok_or_else(|| {
                            ReferenceTextGenerationError::Runtime(
                                super::RuntimeError::Backend(String::from(
                                    "cuda decode-step plan requires device transposed router weight buffer",
                                )),
                            )
                        })?,
                    &layer_plan.router_logits_buffer,
                    1,
                    layer.feed_forward_router_weight.columns,
                    layer.feed_forward_router_weight.rows,
                )?;
                if let Some(bias) = layer.feed_forward_router_bias_device.as_ref() {
                    submission.add_f32_in_place(
                        &layer_plan.router_logits_buffer,
                        0,
                        bias,
                        layer.feed_forward_router_weight.rows,
                    )?;
                }
                submission.router_topk_delayed_softmax(
                    &layer_plan.router_logits_buffer,
                    layer.feed_forward_router_weight.rows,
                    selected_count,
                    &layer_plan.selected_ids_buffer,
                    &layer_plan.selected_weights_buffer,
                )?;
                submission.moe_gate_up_swiglu(
                    layer
                        .feed_forward_gate_up_experts_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    layer.feed_forward_gate_up_experts_weight.mode,
                    layer.feed_forward_gate_up_experts_weight.row_byte_len,
                    layer.feed_forward_gate_up_experts_weight.total_rows(),
                    layer.feed_forward_gate_up_experts_weight.columns,
                    gate_rows,
                    up_rows,
                    &layer_plan.selected_ids_buffer,
                    selected_count,
                    &layer_plan.ffn_norm_buffer,
                    layer.feed_forward_gate_experts_bias_device.as_ref(),
                    layer.feed_forward_up_experts_bias_device.as_ref(),
                    &layer_plan.activated_buffer,
                )?;
                submission.moe_down_aggregate(
                    layer
                        .feed_forward_down_experts_weight
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    layer.feed_forward_down_experts_weight.mode,
                    layer.feed_forward_down_experts_weight.row_byte_len,
                    layer.feed_forward_down_experts_weight.rows,
                    layer.feed_forward_down_experts_weight.columns,
                    &layer_plan.selected_ids_buffer,
                    &layer_plan.selected_weights_buffer,
                    selected_count,
                    &layer_plan.activated_buffer,
                    layer.feed_forward_down_experts_bias_device.as_ref(),
                    Some(&layer_plan.projected_buffer),
                    &layer_plan.moe_buffer,
                )?;
            }
            let layer_ns = duration_ns(layer_start);
            let stage_ns = layer_ns / 3;
            perf.stage_timings.feed_forward_norm_ns = perf
                .stage_timings
                .feed_forward_norm_ns
                .saturating_add(stage_ns);
            perf.stage_timings.router_ns = perf.stage_timings.router_ns.saturating_add(stage_ns);
            perf.stage_timings.expert_projection_ns = perf
                .stage_timings
                .expert_projection_ns
                .saturating_add(stage_ns);
            perf.stage_timings.qkv_projection_ns = perf
                .stage_timings
                .qkv_projection_ns
                .saturating_add(stage_ns);
            perf.stage_timings.attention_ns =
                perf.stage_timings.attention_ns.saturating_add(stage_ns);

            *bytes_moved = bytes_moved
                .saturating_add(layer.attention_qkv_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
        }

        let logits_start = Instant::now();
        let final_hidden = if self.layers.is_empty() {
            &plan.hidden_buffer
        } else {
            &plan.layers[self.layers.len().saturating_sub(1)].moe_buffer
        };
        let output_norm_device = self.output_norm_device.as_ref().ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "cuda decode-step plan requires device output norm buffer",
            )))
        })?;
        if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
            submission.rms_norm(
                final_hidden,
                output_norm_device,
                &plan.final_norm_buffer,
                hidden_size,
                self.family_metadata.rms_norm_epsilon,
            )?;
            submission.cast_f32_to_f16(
                &plan.final_norm_buffer,
                &plan.vector_f16_buffer,
                self.output.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.logits_buffer,
                1,
                self.output.columns,
                self.output.rows,
            )?;
        } else if can_use_q8_1_mmvq(self.output.mode)
            && can_use_q8_1_norm_fusion(self.output.columns)
        {
            submission.rms_norm_q8_1(
                final_hidden,
                output_norm_device,
                &plan.vector_q8_1_buffer,
                self.output.columns,
                self.family_metadata.rms_norm_epsilon,
            )?;
            if output_mode == CudaStepOutputMode::DeviceArgmax {
                submission.copy_host_to_device(
                    &plan.argmax_state_host_buffer,
                    &plan.argmax_state_buffer,
                )?;
                submission.quantized_matvec_q8_1_argmax(
                    self.output
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    self.output.mode,
                    self.output.rows,
                    self.output.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &plan.argmax_state_buffer,
                )?;
                submission.copy_device_to_host(
                    &plan.argmax_state_buffer,
                    &plan.argmax_state_host_buffer,
                )?;
            } else {
                submission.quantized_matvec_q8_1(
                    self.output
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    self.output.mode,
                    self.output.rows,
                    self.output.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &plan.logits_buffer,
                )?;
            }
        } else if can_use_q8_1_mmvq(self.output.mode) {
            submission.rms_norm(
                final_hidden,
                output_norm_device,
                &plan.final_norm_buffer,
                hidden_size,
                self.family_metadata.rms_norm_epsilon,
            )?;
            submission.quantize_f32_to_q8_1(
                &plan.final_norm_buffer,
                1,
                self.output.columns,
                &plan.vector_q8_1_buffer,
            )?;
            if output_mode == CudaStepOutputMode::DeviceArgmax {
                submission.copy_host_to_device(
                    &plan.argmax_state_host_buffer,
                    &plan.argmax_state_buffer,
                )?;
                submission.quantized_matvec_q8_1_argmax(
                    self.output
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    self.output.mode,
                    self.output.rows,
                    self.output.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &plan.argmax_state_buffer,
                )?;
                submission.copy_device_to_host(
                    &plan.argmax_state_buffer,
                    &plan.argmax_state_host_buffer,
                )?;
            } else {
                submission.quantized_matvec_q8_1(
                    self.output
                        .device_storage()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                    0,
                    self.output.mode,
                    self.output.rows,
                    self.output.columns,
                    &plan.vector_q8_1_buffer,
                    None,
                    &plan.logits_buffer,
                )?;
            }
        } else {
            submission.rms_norm(
                final_hidden,
                output_norm_device,
                &plan.final_norm_buffer,
                hidden_size,
                self.family_metadata.rms_norm_epsilon,
            )?;
            submission.quantized_matvec(
                self.output
                    .device_storage()
                    .map_err(ReferenceTextGenerationError::Runtime)?,
                0,
                self.output.mode,
                self.output.rows,
                self.output.columns,
                &plan.final_norm_buffer,
                &plan.logits_buffer,
            )?;
        }
        if output_mode == CudaStepOutputMode::DeviceArgmax
            && (self.output.transposed_f16.is_some() || !can_use_q8_1_mmvq(self.output.mode))
        {
            submission.argmax_f32(
                &plan.logits_buffer,
                1,
                self.output.rows,
                &plan.next_token_buffer,
            )?;
            submission
                .copy_device_to_host(&plan.next_token_buffer, &plan.next_token_host_buffer)?;
        }
        perf.stage_timings.logits_projection_ns = perf
            .stage_timings
            .logits_projection_ns
            .saturating_add(duration_ns(logits_start));
        *bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
        Ok(())
    }

    fn forward_step_with_cuda_plan(
        &self,
        backend: &mut CudaBackend,
        token: TokenId,
        position: usize,
        cuda_cache: &mut CudaKvCacheMirror,
        plan: &mut GptOssCudaStepPlan,
        output_mode: CudaStepOutputMode,
        materialize_host_kv: bool,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        let mut bytes_moved = 0_u64;
        let mut kernel_count = 0_usize;
        let mut perf = GptOssPerformanceMetrics {
            step_count: 1,
            layer_visit_count: self.layers.len(),
            graph_node_count: self.graph_node_count(),
            graph_layer_node_count: self.graph_layer_node_count(),
            ..GptOssPerformanceMetrics::default()
        };

        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

        let cache_write_index = cuda_cache.len();
        cuda_cache.ensure_capacity(backend, cache_write_index.saturating_add(1))?;
        let decode_params = [
            i32::try_from(cache_write_index).map_err(|_| {
                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                    "cache write index {cache_write_index} exceeds i32 decode parameter limits",
                )))
            })?,
            i32::try_from(position).map_err(|_| {
                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                    "position {position} exceeds i32 decode parameter limits",
                )))
            })?,
            i32::try_from(token.as_u32()).map_err(|_| {
                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                    "token {} exceeds i32 decode parameter limits",
                    token.as_u32(),
                )))
            })?,
        ];
        plan.decode_params_host_buffer.write_i32(&decode_params)?;
        perf.cuda.host_to_device_bytes = perf.cuda.host_to_device_bytes.saturating_add(
            (decode_params.len() * std::mem::size_of::<i32>())
                .try_into()
                .unwrap_or(u64::MAX),
        );
        if output_mode == CudaStepOutputMode::DeviceArgmax {
            plan.argmax_state_host_buffer
                .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())?;
            perf.cuda.host_to_device_bytes = perf
                .cuda
                .host_to_device_bytes
                .saturating_add(std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX));
        }

        let use_decode_graph_fast_path = decode_graph_fast_path_enabled()
            && output_mode == CudaStepOutputMode::DeviceArgmax
            && !materialize_host_kv;
        let decode_graph_cache_identity = Some((
            cuda_cache.key_buffer.allocation_identity(),
            cuda_cache.value_buffer.allocation_identity(),
        ));
        let submission_report = if use_decode_graph_fast_path {
            if plan.decode_graph_cache_identity == decode_graph_cache_identity {
                if let Some(graph_exec) = plan.decode_graph_exec.as_ref() {
                    graph_exec.launch(psionic_backend_cuda::CudaCommandWait::Completed)?
                } else {
                    let mut submission = backend.begin_captured_submission()?;
                    self.encode_cuda_forward_step_submission(
                        &mut submission,
                        position,
                        cache_write_index,
                        cuda_cache,
                        plan,
                        output_mode,
                        &mut perf,
                        &mut bytes_moved,
                        true,
                    )?;
                    let (report, graph_exec) = submission
                        .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)?;
                    plan.decode_graph_exec = Some(graph_exec);
                    plan.decode_graph_cache_identity = decode_graph_cache_identity;
                    report
                }
            } else {
                plan.decode_graph_exec = None;
                plan.decode_graph_cache_identity = None;
                let mut submission = backend.begin_captured_submission()?;
                self.encode_cuda_forward_step_submission(
                    &mut submission,
                    position,
                    cache_write_index,
                    cuda_cache,
                    plan,
                    output_mode,
                    &mut perf,
                    &mut bytes_moved,
                    true,
                )?;
                let (report, graph_exec) =
                    submission.commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)?;
                plan.decode_graph_exec = Some(graph_exec);
                plan.decode_graph_cache_identity = decode_graph_cache_identity;
                report
            }
        } else {
            let mut submission = backend.begin_submission()?;
            self.encode_cuda_forward_step_submission(
                &mut submission,
                position,
                cache_write_index,
                cuda_cache,
                plan,
                output_mode,
                &mut perf,
                &mut bytes_moved,
                false,
            )?;
            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?
        };
        let (logits_values, selected_token, logits_readback_bytes) = match output_mode {
            CudaStepOutputMode::FullLogits => (
                plan.logits_buffer.read_f32()?,
                None,
                self.output
                    .rows
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            CudaStepOutputMode::DeviceArgmax => {
                let token = if can_use_q8_1_mmvq(self.output.mode)
                    && self.output.transposed_f16.is_none()
                {
                    let bytes = plan
                        .argmax_state_host_buffer
                        .read_bytes()
                        .map_err(|error| {
                            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                format!(
                                    "cuda argmax returned an invalid packed host buffer: {error}"
                                ),
                            ))
                        })?;
                    let packed = u64::from_ne_bytes(
                        bytes[..std::mem::size_of::<u64>()]
                            .try_into()
                            .map_err(|_| {
                                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                    String::from(
                                        "cuda argmax returned invalid packed argmax bytes",
                                    ),
                                ))
                            })?,
                    );
                    (packed >> 32) as i32
                } else {
                    plan.next_token_host_buffer.read_i32().map_err(|error| {
                        ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                            format!("cuda argmax returned an invalid host token buffer: {error}",),
                        ))
                    })?
                };
                let token = u32::try_from(token).map(TokenId).map_err(|_| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                        "cuda argmax returned a negative token id {token}",
                    )))
                })?;
                (
                    Vec::new(),
                    Some(token),
                    if can_use_q8_1_mmvq(self.output.mode) {
                        std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX)
                    } else {
                        std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX)
                    },
                )
            }
        };
        cuda_cache.len = cache_write_index.saturating_add(1);
        accumulate_cuda_submission_report(&mut perf, &submission_report, 0, logits_readback_bytes);
        kernel_count = kernel_count.saturating_add(submission_report.encoded_operations);

        let (cache_key, cache_value) = if materialize_host_kv {
            let (key, value) = cuda_cache.read_entry(cache_write_index)?;
            let readback_bytes = cache_key_value_byte_len(cuda_cache.width);
            perf.cuda.device_to_host_bytes = perf
                .cuda
                .device_to_host_bytes
                .saturating_add(readback_bytes);
            (key, value)
        } else {
            (Vec::new(), Vec::new())
        };

        Ok(GptOssForwardStep {
            key: cache_key,
            value: cache_value,
            logits: logits_values,
            selected_token,
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }

    fn forward_step_with_output_mode(
        &self,
        backend: &mut CudaBackend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
        output_mode: CudaStepOutputMode,
        mut cuda_cache: Option<&mut CudaKvCacheMirror>,
        materialize_host_kv: bool,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        let mut hybrid_selected4_plan = if self.family_metadata.expert_used_count == Some(4)
            && self.layers.iter().any(|layer| {
                layer.feed_forward_gate_up_experts_weight.host.is_some()
                    && layer.feed_forward_down_experts_weight.host.is_some()
            }) {
            self.acquire_hybrid_selected4_plan(backend)?
        } else {
            None
        };
        let result = (|| -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
            let hidden_size = self.descriptor.config.hidden_size;
            let kv_width = self.descriptor.config.kv_width();
            let head_count = self.descriptor.config.block.attention.head_count;
            let kv_head_count = self.descriptor.config.block.attention.kv_head_count;
            let head_dim = self.descriptor.config.block.attention.head_dim;
            let rotary_dim = self.descriptor.config.block.attention.rotary_dim;
            let (freq_scale, ext_factor, corr_dims, theta_scale) =
                rope_runtime_parameters(rotary_dim, &self.family_metadata);
            let mut bytes_moved = 0_u64;
            let mut kernel_count = 0_usize;
            let mut perf = GptOssPerformanceMetrics {
                step_count: 1,
                layer_visit_count: self.layers.len(),
                graph_node_count: self.graph_node_count(),
                graph_layer_node_count: self.graph_layer_node_count(),
                ..GptOssPerformanceMetrics::default()
            };

            let token_embedding_start = Instant::now();
            let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
            perf.stage_timings.token_embedding_ns = perf
                .stage_timings
                .token_embedding_ns
                .saturating_add(duration_ns(token_embedding_start));
            bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

            let mut cache_key = vec![0.0; self.cache_width()];
            let mut cache_value = vec![0.0; self.cache_width()];
            let cache_write_index = if let Some(cuda_cache) = cuda_cache.as_deref_mut() {
                let index = cuda_cache.len();
                cuda_cache.ensure_capacity(backend, index.saturating_add(1))?;
                Some(index)
            } else {
                None
            };
            let mut hidden_device_ready = false;

            for (layer_index, layer) in self.layers.iter().enumerate() {
                let can_use_device_hidden_residency = output_mode
                    == CudaStepOutputMode::DeviceArgmax
                    && hybrid_selected4_plan.is_some()
                    && cuda_cache.is_some()
                    && cache_write_index.is_some()
                    && can_use_hybrid_cuda_hidden_residency_layer(layer, hidden_size);
                if !can_use_device_hidden_residency && hidden_device_ready {
                    let plan = hybrid_selected4_plan.as_ref().expect("checked above");
                    hidden = plan
                        .output_buffer
                        .read_f32_at_offset(0, hidden_size)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    perf.cuda.device_to_host_bytes = perf.cuda.device_to_host_bytes.saturating_add(
                        hidden_size
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                    );
                    hidden_device_ready = false;
                }
                if can_use_device_hidden_residency {
                    let plan = hybrid_selected4_plan.as_mut().expect("checked above");
                    let cuda_cache = cuda_cache.as_deref_mut().expect("checked above");
                    let cache_write_index = cache_write_index.expect("checked above");
                    let q_rows = layer.attention_qkv_weight.rows_per_projection[0];
                    let k_rows = layer.attention_qkv_weight.rows_per_projection[1];
                    let gate_rows = layer
                        .feed_forward_gate_up_experts_weight
                        .rows_per_projection[0];
                    let up_rows = layer
                        .feed_forward_gate_up_experts_weight
                        .rows_per_projection[1];
                    if gate_rows != up_rows {
                        return Err(ReferenceTextGenerationError::Runtime(
                            super::RuntimeError::Backend(format!(
                                "cuda gpt-oss hybrid MoE path requires matching gate/up expert widths, got gate {} and up {}",
                                gate_rows, up_rows
                            )),
                        ));
                    }
                    let cache_offset = layer_index.saturating_mul(kv_width);
                    let use_q8_1_attention_output_fusion =
                        can_use_q8_1_mmvq(layer.attention_output_weight.mode)
                            && can_use_q8_1_attention_output_fusion(
                                layer.attention_output_weight.columns,
                                head_count,
                                head_dim,
                            );
                    let layer_start = Instant::now();
                    let dense_start = Instant::now();
                    let mut dense_submission = backend.begin_submission()?;
                    if hidden_device_ready {
                        dense_submission
                            .copy_buffer(&plan.output_buffer, &plan.hidden_input_buffer)?;
                    } else {
                        plan.hidden_input_host_buffer
                            .write_f32(hidden.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        dense_submission.copy_host_to_device(
                            &plan.hidden_input_host_buffer,
                            &plan.hidden_input_buffer,
                        )?;
                    }
                    if can_use_q8_1_mmvq(layer.attention_qkv_weight.mode) {
                        dense_submission.rms_norm_q8_1(
                            &plan.hidden_input_buffer,
                            layer.attention_norm_device.as_ref().expect("checked above"),
                            &plan.hidden_input_q8_1_buffer,
                            hidden_size,
                            self.family_metadata.rms_norm_epsilon,
                        )?;
                        dense_submission.quantized_matvec_q8_1(
                            layer
                                .attention_qkv_weight
                                .storage
                                .as_ref()
                                .expect("checked above"),
                            0,
                            layer.attention_qkv_weight.mode,
                            layer.attention_qkv_weight.total_rows(),
                            layer.attention_qkv_weight.columns,
                            &plan.hidden_input_q8_1_buffer,
                            Some(
                                layer
                                    .attention_qkv_bias_device
                                    .as_ref()
                                    .expect("checked above"),
                            ),
                            &plan.qkv_buffer,
                        )?;
                    } else {
                        dense_submission.rms_norm(
                            &plan.hidden_input_buffer,
                            layer.attention_norm_device.as_ref().expect("checked above"),
                            &plan.attention_input_buffer,
                            hidden_size,
                            self.family_metadata.rms_norm_epsilon,
                        )?;
                        dense_submission.quantized_matvec(
                            layer
                                .attention_qkv_weight
                                .storage
                                .as_ref()
                                .expect("checked above"),
                            0,
                            layer.attention_qkv_weight.mode,
                            layer.attention_qkv_weight.total_rows(),
                            layer.attention_qkv_weight.columns,
                            &plan.attention_input_buffer,
                            &plan.qkv_buffer,
                        )?;
                        dense_submission.add_f32_in_place(
                            &plan.qkv_buffer,
                            0,
                            layer
                                .attention_qkv_bias_device
                                .as_ref()
                                .expect("checked above"),
                            layer.attention_qkv_weight.total_rows(),
                        )?;
                    }
                    if use_q8_1_attention_output_fusion {
                        dense_submission.attention_decode_rope_cache_f16_kv_q8_1(
                            &plan.qkv_buffer,
                            0,
                            q_rows,
                            q_rows.saturating_add(k_rows),
                            &cuda_cache.key_buffer,
                            &cuda_cache.value_buffer,
                            cuda_cache.width,
                            cache_offset,
                            cache_write_index,
                            self.family_metadata.sliding_window.unwrap_or(0),
                            head_count,
                            kv_head_count,
                            head_dim,
                            rotary_dim,
                            position,
                            freq_scale,
                            ext_factor,
                            corr_dims,
                            theta_scale,
                            layer.attention_sinks_device.as_ref(),
                            &plan.hidden_input_q8_1_buffer,
                        )?;
                        dense_submission.quantized_matvec_q8_1(
                            layer
                                .attention_output_weight
                                .storage
                                .as_ref()
                                .expect("checked above"),
                            0,
                            layer.attention_output_weight.mode,
                            layer.attention_output_weight.rows,
                            layer.attention_output_weight.columns,
                            &plan.hidden_input_q8_1_buffer,
                            None,
                            &plan.output_buffer,
                        )?;
                    } else {
                        dense_submission.attention_decode_rope_cache_f16_kv(
                            &plan.qkv_buffer,
                            0,
                            q_rows,
                            q_rows.saturating_add(k_rows),
                            &cuda_cache.key_buffer,
                            &cuda_cache.value_buffer,
                            cuda_cache.width,
                            cache_offset,
                            cache_write_index,
                            self.family_metadata.sliding_window.unwrap_or(0),
                            head_count,
                            kv_head_count,
                            head_dim,
                            rotary_dim,
                            position,
                            freq_scale,
                            ext_factor,
                            corr_dims,
                            theta_scale,
                            layer.attention_sinks_device.as_ref(),
                            &plan.attention_input_buffer,
                        )?;
                        if can_use_q8_1_mmvq(layer.attention_output_weight.mode) {
                            dense_submission.quantize_f32_to_q8_1(
                                &plan.attention_input_buffer,
                                1,
                                layer.attention_output_weight.columns,
                                &plan.hidden_input_q8_1_buffer,
                            )?;
                            dense_submission.quantized_matvec_q8_1(
                                layer
                                    .attention_output_weight
                                    .storage
                                    .as_ref()
                                    .expect("checked above"),
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                &plan.hidden_input_q8_1_buffer,
                                None,
                                &plan.output_buffer,
                            )?;
                        } else {
                            dense_submission.quantized_matvec(
                                layer
                                    .attention_output_weight
                                    .storage
                                    .as_ref()
                                    .expect("checked above"),
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                &plan.attention_input_buffer,
                                &plan.output_buffer,
                            )?;
                        }
                    }
                    dense_submission.add_residual_rms_norm_q8_1(
                        &plan.output_buffer,
                        &plan.hidden_input_buffer,
                        layer.attention_output_bias_device.as_ref(),
                        layer
                            .feed_forward_norm_device
                            .as_ref()
                            .expect("checked above"),
                        &plan.attention_input_buffer,
                        &plan.projected_buffer,
                        &plan.hidden_input_q8_1_buffer,
                        hidden_size,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    dense_submission.matmul(
                        &plan.projected_buffer,
                        layer
                            .feed_forward_router_weight_transposed_device
                            .as_ref()
                            .expect("checked above"),
                        &plan.router_logits_buffer,
                        1,
                        layer.feed_forward_router_weight.columns,
                        layer.feed_forward_router_weight.rows,
                    )?;
                    if let Some(bias) = layer.feed_forward_router_bias_device.as_ref() {
                        dense_submission.add_f32_in_place(
                            &plan.router_logits_buffer,
                            0,
                            bias,
                            layer.feed_forward_router_weight.rows,
                        )?;
                    }
                    dense_submission.router_topk_delayed_softmax(
                        &plan.router_logits_buffer,
                        layer.feed_forward_router_weight.rows,
                        4,
                        &plan.selected_expert_ids_buffer,
                        &plan.selected_weights_buffer,
                    )?;
                    let dense_report = dense_submission
                        .commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                    plan.selected_expert_ids_scratch = i32_ne_bytes_to_values(
                        plan.selected_expert_ids_buffer.read_bytes()?.as_slice(),
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                    let selected = plan
                        .selected_expert_ids_scratch
                        .iter()
                        .copied()
                        .map(|index| {
                            usize::try_from(index).map_err(|_| {
                                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                    format!("cuda router returned negative expert index {index}"),
                                ))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    perf.cuda.host_to_device_bytes =
                        perf.cuda
                            .host_to_device_bytes
                            .saturating_add(if hidden_device_ready {
                                0
                            } else {
                                hidden
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX)
                            });
                    perf.cuda.device_to_host_bytes = perf.cuda.device_to_host_bytes.saturating_add(
                        (selected.len().saturating_mul(std::mem::size_of::<i32>()))
                            .try_into()
                            .unwrap_or(u64::MAX),
                    );
                    perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                    perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                    perf.cuda.kernel_launches = perf
                        .cuda
                        .kernel_launches
                        .saturating_add(dense_report.encoded_operations);
                    let dense_stage_ns = duration_ns(dense_start);
                    let attn_stage_ns = dense_stage_ns / 2;
                    perf.stage_timings.attention_ns = perf
                        .stage_timings
                        .attention_ns
                        .saturating_add(attn_stage_ns);
                    perf.stage_timings.attention_output_projection_ns = perf
                        .stage_timings
                        .attention_output_projection_ns
                        .saturating_add(attn_stage_ns / 2);
                    perf.stage_timings.feed_forward_norm_ns = perf
                        .stage_timings
                        .feed_forward_norm_ns
                        .saturating_add(attn_stage_ns / 2);
                    perf.stage_timings.router_ns = perf
                        .stage_timings
                        .router_ns
                        .saturating_add(dense_stage_ns.saturating_sub(attn_stage_ns));

                    let selected_count = selected.len();
                    if selected_count == 4 {
                        let gate_up_host = layer
                            .feed_forward_gate_up_experts_weight
                            .host
                            .as_ref()
                            .expect("checked above");
                        let down_host = layer
                            .feed_forward_down_experts_weight
                            .host
                            .as_ref()
                            .expect("checked above");
                        let selected_key =
                            <[usize; 4]>::try_from(selected.as_slice()).map_err(|_| {
                                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                    String::from(
                                        "hybrid selected4 path received a non-4 expert set",
                                    ),
                                ))
                            })?;
                        let gate_stride = gate_up_host
                            .gate
                            .rows
                            .saturating_mul(gate_up_host.gate.row_byte_len);
                        let up_stride = gate_up_host
                            .up
                            .rows
                            .saturating_mul(gate_up_host.up.row_byte_len);
                        let gate_up_expert_stride = gate_stride.saturating_add(up_stride);
                        let down_expert_stride =
                            down_host.rows.saturating_mul(down_host.row_byte_len);
                        let mut staged_selected4_host_bytes = 0usize;
                        let mut selected_slot_ids = [0_i32, 1_i32, 2_i32, 3_i32];
                        let mut selected4_cache_hits = 0usize;
                        let mut selected4_cache_misses = 0usize;
                        let using_layer_cache = if let Some(layer_cache) = plan
                            .layer_caches
                            .get_mut(layer_index)
                            .and_then(Option::as_mut)
                        {
                            let mut reserved_slots =
                                [false; HYBRID_SELECTED4_LAYER_CACHE_MAX_SLOTS];
                            let slot_count = layer_cache.slot_count;
                            for (selected_index, expert_index) in
                                selected_key.iter().copied().enumerate()
                            {
                                let slot = layer_cache
                                    .cached_experts
                                    .iter()
                                    .enumerate()
                                    .find(|(slot, cached)| {
                                        *slot < slot_count
                                            && !reserved_slots[*slot]
                                            && **cached == Some(expert_index)
                                    })
                                    .map(|(slot, _)| slot)
                                    .unwrap_or_else(|| {
                                        layer_cache
                                            .cached_experts
                                            .iter()
                                            .enumerate()
                                            .filter(|(slot, _)| {
                                                *slot < slot_count && !reserved_slots[*slot]
                                            })
                                            .min_by_key(|(slot, cached)| {
                                                (
                                                    cached.is_some(),
                                                    layer_cache.slot_last_used[*slot],
                                                )
                                            })
                                            .map(|(slot, _)| slot)
                                            .unwrap_or(0)
                                    });
                                if layer_cache.cached_experts[slot] != Some(expert_index) {
                                    selected4_cache_misses =
                                        selected4_cache_misses.saturating_add(1);
                                    let (gate_bytes, up_bytes) =
                                        gate_up_host.expert_projection_bytes(expert_index)?;
                                    layer_cache
                                        .gate_up_weights_buffer
                                        .write_bytes_at_offset(
                                            slot.saturating_mul(gate_up_expert_stride),
                                            gate_bytes,
                                        )
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    layer_cache
                                        .gate_up_weights_buffer
                                        .write_bytes_at_offset(
                                            slot.saturating_mul(gate_up_expert_stride)
                                                .saturating_add(gate_stride),
                                            up_bytes,
                                        )
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    staged_selected4_host_bytes = staged_selected4_host_bytes
                                        .saturating_add(
                                            gate_bytes.len().saturating_add(up_bytes.len()),
                                        );
                                    let down_bytes = down_host.expert_bytes(expert_index)?;
                                    layer_cache
                                        .down_weights_buffer
                                        .write_bytes_at_offset(
                                            slot.saturating_mul(down_expert_stride),
                                            down_bytes,
                                        )
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    staged_selected4_host_bytes = staged_selected4_host_bytes
                                        .saturating_add(down_bytes.len());
                                    if let Some(values) =
                                        layer.feed_forward_gate_experts_bias.as_ref()
                                    {
                                        let bias = selected_expert_bias_rows(
                                            values.as_slice(),
                                            gate_rows,
                                            expert_index,
                                        )?;
                                        let device = layer_cache
                                            .gate_bias_buffer
                                            .as_mut()
                                            .ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 gate bias buffer",
                                                    )),
                                                )
                                            })?;
                                        device
                                            .write_f32_at_offset(
                                                slot.saturating_mul(gate_rows),
                                                bias,
                                            )
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                bias.len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    if let Some(values) =
                                        layer.feed_forward_up_experts_bias.as_ref()
                                    {
                                        let bias = selected_expert_bias_rows(
                                            values.as_slice(),
                                            up_rows,
                                            expert_index,
                                        )?;
                                        let device = layer_cache
                                            .up_bias_buffer
                                            .as_mut()
                                            .ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 up bias buffer",
                                                    )),
                                                )
                                            })?;
                                        device
                                            .write_f32_at_offset(slot.saturating_mul(up_rows), bias)
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                bias.len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    if let Some(values) =
                                        layer.feed_forward_down_experts_bias.as_ref()
                                    {
                                        let bias = selected_expert_bias_rows(
                                            values.as_slice(),
                                            layer.feed_forward_down_experts_weight.rows,
                                            expert_index,
                                        )?;
                                        let device = layer_cache
                                            .down_bias_buffer
                                            .as_mut()
                                            .ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 down bias buffer",
                                                    )),
                                                )
                                            })?;
                                        device
                                            .write_f32_at_offset(
                                                slot.saturating_mul(
                                                    layer.feed_forward_down_experts_weight.rows,
                                                ),
                                                bias,
                                            )
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                bias.len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    layer_cache.cached_experts[slot] = Some(expert_index);
                                } else {
                                    selected4_cache_hits = selected4_cache_hits.saturating_add(1);
                                }
                                reserved_slots[slot] = true;
                                layer_cache.slot_last_used[slot] = layer_cache.usage_clock;
                                layer_cache.usage_clock = layer_cache.usage_clock.saturating_add(1);
                                selected_slot_ids[selected_index] =
                                    i32::try_from(slot).map_err(|_| {
                                        ReferenceTextGenerationError::Runtime(
                                            super::RuntimeError::Backend(format!(
                                                "hybrid selected4 slot {slot} exceeds i32 range",
                                            )),
                                        )
                                    })?;
                            }
                            true
                        } else {
                            gate_up_host.selected_packed_bytes_into(
                                selected.as_slice(),
                                &mut plan.gate_up_weights_scratch,
                            )?;
                            down_host.selected_bytes_into(
                                selected.as_slice(),
                                &mut plan.down_weights_scratch,
                            )?;
                            plan.gate_up_weights_host_buffer
                                .write_bytes(plan.gate_up_weights_scratch.as_slice())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            plan.down_weights_host_buffer
                                .write_bytes(plan.down_weights_scratch.as_slice())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            staged_selected4_host_bytes = staged_selected4_host_bytes
                                .saturating_add(plan.gate_up_weights_scratch.len())
                                .saturating_add(plan.down_weights_scratch.len());
                            if let Some(values) = layer.feed_forward_gate_experts_bias.as_ref() {
                                gather_selected_expert_bias_rows_into(
                                    values.as_slice(),
                                    gate_rows,
                                    selected.as_slice(),
                                    &mut plan.gate_bias_scratch,
                                )?;
                                let host =
                                    plan.gate_bias_host_buffer.as_mut().ok_or_else(|| {
                                        ReferenceTextGenerationError::Runtime(
                                            super::RuntimeError::Backend(String::from(
                                                "missing hybrid selected4 gate bias host buffer",
                                            )),
                                        )
                                    })?;
                                host.write_f32(plan.gate_bias_scratch.as_slice())
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                staged_selected4_host_bytes = staged_selected4_host_bytes
                                    .saturating_add(
                                        plan.gate_bias_scratch
                                            .len()
                                            .saturating_mul(std::mem::size_of::<f32>()),
                                    );
                            }
                            if let Some(values) = layer.feed_forward_up_experts_bias.as_ref() {
                                gather_selected_expert_bias_rows_into(
                                    values.as_slice(),
                                    up_rows,
                                    selected.as_slice(),
                                    &mut plan.up_bias_scratch,
                                )?;
                                let host = plan.up_bias_host_buffer.as_mut().ok_or_else(|| {
                                    ReferenceTextGenerationError::Runtime(
                                        super::RuntimeError::Backend(String::from(
                                            "missing hybrid selected4 up bias host buffer",
                                        )),
                                    )
                                })?;
                                host.write_f32(plan.up_bias_scratch.as_slice())
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                staged_selected4_host_bytes = staged_selected4_host_bytes
                                    .saturating_add(
                                        plan.up_bias_scratch
                                            .len()
                                            .saturating_mul(std::mem::size_of::<f32>()),
                                    );
                            }
                            if let Some(values) = layer.feed_forward_down_experts_bias.as_ref() {
                                gather_selected_expert_bias_rows_into(
                                    values.as_slice(),
                                    layer.feed_forward_down_experts_weight.rows,
                                    selected.as_slice(),
                                    &mut plan.down_bias_scratch,
                                )?;
                                let host =
                                    plan.down_bias_host_buffer.as_mut().ok_or_else(|| {
                                        ReferenceTextGenerationError::Runtime(
                                            super::RuntimeError::Backend(String::from(
                                                "missing hybrid selected4 down bias host buffer",
                                            )),
                                        )
                                    })?;
                                host.write_f32(plan.down_bias_scratch.as_slice())
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                staged_selected4_host_bytes = staged_selected4_host_bytes
                                    .saturating_add(
                                        plan.down_bias_scratch
                                            .len()
                                            .saturating_mul(std::mem::size_of::<f32>()),
                                    );
                            }
                            false
                        };
                        let selected_id_bytes = i32_slice_to_ne_bytes(selected_slot_ids.as_slice());
                        plan.selected_ids_buffer
                            .write_bytes(selected_id_bytes.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let layer_cache =
                            plan.layer_caches.get(layer_index).and_then(Option::as_ref);
                        let gate_up_weights_device = layer_cache
                            .map(|cache| &cache.gate_up_weights_buffer)
                            .unwrap_or(&plan.gate_up_weights_buffer);
                        let down_weights_device = layer_cache
                            .map(|cache| &cache.down_weights_buffer)
                            .unwrap_or(&plan.down_weights_buffer);
                        let gate_bias_device = layer_cache
                            .and_then(|cache| cache.gate_bias_buffer.as_ref())
                            .or_else(|| plan.gate_bias_buffer.as_ref());
                        let up_bias_device = layer_cache
                            .and_then(|cache| cache.up_bias_buffer.as_ref())
                            .or_else(|| plan.up_bias_buffer.as_ref());
                        let down_bias_device = layer_cache
                            .and_then(|cache| cache.down_bias_buffer.as_ref())
                            .or_else(|| plan.down_bias_buffer.as_ref());
                        let expert_projection_start = Instant::now();
                        let mut expert_submission = backend.begin_submission()?;
                        if !using_layer_cache {
                            expert_submission.copy_host_to_device(
                                &plan.gate_up_weights_host_buffer,
                                gate_up_weights_device,
                            )?;
                            expert_submission.copy_host_to_device(
                                &plan.down_weights_host_buffer,
                                down_weights_device,
                            )?;
                            if let (Some(host), Some(device)) =
                                (plan.gate_bias_host_buffer.as_ref(), gate_bias_device)
                            {
                                expert_submission.copy_host_to_device(host, device)?;
                            }
                            if let (Some(host), Some(device)) =
                                (plan.up_bias_host_buffer.as_ref(), up_bias_device)
                            {
                                expert_submission.copy_host_to_device(host, device)?;
                            }
                            if let (Some(host), Some(device)) =
                                (plan.down_bias_host_buffer.as_ref(), down_bias_device)
                            {
                                expert_submission.copy_host_to_device(host, device)?;
                            }
                        }
                        expert_submission.moe_gate_up_swiglu_q8_1_selected4_quantized(
                            gate_up_weights_device,
                            layer.feed_forward_gate_up_experts_weight.mode,
                            layer.feed_forward_gate_up_experts_weight.row_byte_len,
                            layer.feed_forward_gate_up_experts_weight.total_rows(),
                            layer.feed_forward_gate_up_experts_weight.columns,
                            gate_rows,
                            up_rows,
                            &plan.selected_ids_buffer,
                            selected_count,
                            &plan.hidden_input_q8_1_buffer,
                            gate_bias_device,
                            up_bias_device,
                            &plan.activated_q8_1_buffer,
                        )?;
                        expert_submission.moe_down_project_q8_1_selected4(
                            down_weights_device,
                            layer.feed_forward_down_experts_weight.mode,
                            layer.feed_forward_down_experts_weight.row_byte_len,
                            layer.feed_forward_down_experts_weight.rows,
                            layer.feed_forward_down_experts_weight.columns,
                            &plan.selected_ids_buffer,
                            selected_count,
                            &plan.activated_q8_1_buffer,
                            down_bias_device,
                            &plan.projected_buffer,
                        )?;
                        expert_submission.accumulate_selected4(
                            &plan.projected_buffer,
                            &plan.selected_weights_buffer,
                            selected_count,
                            hidden_size,
                            None,
                            &plan.output_buffer,
                        )?;
                        expert_submission.add_f32_in_place(
                            &plan.output_buffer,
                            0,
                            &plan.attention_input_buffer,
                            hidden_size,
                        )?;
                        let expert_report = expert_submission
                            .commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                        perf.stage_timings.expert_projection_ns = perf
                            .stage_timings
                            .expert_projection_ns
                            .saturating_add(duration_ns(expert_projection_start));
                        perf.cuda.host_to_device_bytes =
                            perf.cuda.host_to_device_bytes.saturating_add(
                                (selected_id_bytes
                                    .len()
                                    .saturating_add(staged_selected4_host_bytes))
                                .try_into()
                                .unwrap_or(u64::MAX),
                            );
                        perf.cuda.hybrid_selected4_cache_hits = perf
                            .cuda
                            .hybrid_selected4_cache_hits
                            .saturating_add(selected4_cache_hits);
                        perf.cuda.hybrid_selected4_cache_misses = perf
                            .cuda
                            .hybrid_selected4_cache_misses
                            .saturating_add(selected4_cache_misses);
                        perf.cuda.hybrid_selected4_cache_staged_bytes = perf
                            .cuda
                            .hybrid_selected4_cache_staged_bytes
                            .saturating_add(
                                staged_selected4_host_bytes.try_into().unwrap_or(u64::MAX),
                            );
                        if perf.cuda.hybrid_selected4_layer_cache_hits.len() <= layer_index {
                            perf.cuda
                                .hybrid_selected4_layer_cache_hits
                                .resize(layer_index.saturating_add(1), 0);
                        }
                        perf.cuda.hybrid_selected4_layer_cache_hits[layer_index] =
                            perf.cuda.hybrid_selected4_layer_cache_hits[layer_index]
                                .saturating_add(selected4_cache_hits);
                        if perf.cuda.hybrid_selected4_layer_cache_misses.len() <= layer_index {
                            perf.cuda
                                .hybrid_selected4_layer_cache_misses
                                .resize(layer_index.saturating_add(1), 0);
                        }
                        perf.cuda.hybrid_selected4_layer_cache_misses[layer_index] =
                            perf.cuda.hybrid_selected4_layer_cache_misses[layer_index]
                                .saturating_add(selected4_cache_misses);
                        if perf.cuda.hybrid_selected4_layer_cache_staged_bytes.len() <= layer_index
                        {
                            perf.cuda
                                .hybrid_selected4_layer_cache_staged_bytes
                                .resize(layer_index.saturating_add(1), 0);
                        }
                        perf.cuda.hybrid_selected4_layer_cache_staged_bytes[layer_index] =
                            perf.cuda.hybrid_selected4_layer_cache_staged_bytes[layer_index]
                                .saturating_add(
                                    staged_selected4_host_bytes.try_into().unwrap_or(u64::MAX),
                                );
                        perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                        perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                        perf.cuda.kernel_launches = perf
                            .cuda
                            .kernel_launches
                            .saturating_add(expert_report.encoded_operations);
                        hidden_device_ready = true;
                        bytes_moved = bytes_moved
                            .saturating_add(layer.attention_qkv_weight.byte_length() as u64)
                            .saturating_add(layer.attention_output_weight.byte_length() as u64)
                            .saturating_add(
                                layer.feed_forward_gate_up_experts_weight.byte_length() as u64
                            )
                            .saturating_add(
                                layer.feed_forward_down_experts_weight.byte_length() as u64
                            );
                        kernel_count = kernel_count
                            .saturating_add(dense_report.encoded_operations)
                            .saturating_add(expert_report.encoded_operations);
                        let layer_ns = duration_ns(layer_start);
                        perf.stage_timings.step_wall_ns =
                            perf.stage_timings.step_wall_ns.saturating_add(layer_ns);
                        continue;
                    }
                }
                let residual = hidden.clone();
                let attention_norm_start = Instant::now();
                let hidden_norm = rms_norm(
                    hidden.as_slice(),
                    layer.attention_norm.as_slice(),
                    self.family_metadata.rms_norm_epsilon,
                );
                perf.stage_timings.attention_norm_ns = perf
                    .stage_timings
                    .attention_norm_ns
                    .saturating_add(duration_ns(attention_norm_start));

                let qkv_start = Instant::now();
                let (mut qkv_outputs, qkv_stats) = if let (Some(plan), Some(storage)) = (
                    hybrid_selected4_plan.as_mut(),
                    layer.attention_qkv_weight.storage.as_ref(),
                ) {
                    let result = cuda_quantized_matvec_with_reused_buffers(
                        backend,
                        storage,
                        0,
                        layer.attention_qkv_weight.mode,
                        layer.attention_qkv_weight.total_rows(),
                        layer.attention_qkv_weight.columns,
                        hidden_norm.as_slice(),
                        &mut plan.hidden_input_host_buffer,
                        &plan.hidden_input_buffer,
                        &plan.projected_buffer,
                    )?;
                    (
                        split_projection_outputs(
                            &layer.attention_qkv_weight.rows_per_projection,
                            result.values,
                        )?,
                        result.stats,
                    )
                } else {
                    layer
                        .attention_qkv_weight
                        .matvec_profiled(backend, hidden_norm.as_slice())?
                };
                accumulate_cuda_matvec_stats(&mut perf, &qkv_stats);
                let mut q = qkv_outputs.remove(0);
                add_bias_in_place(&mut q, layer.attention_query_bias.as_slice());

                let mut k = qkv_outputs.remove(0);
                add_bias_in_place(&mut k, layer.attention_key_bias.as_slice());

                let mut v = qkv_outputs.remove(0);
                add_bias_in_place(&mut v, layer.attention_value_bias.as_slice());
                perf.stage_timings.qkv_projection_ns = perf
                    .stage_timings
                    .qkv_projection_ns
                    .saturating_add(duration_ns(qkv_start));

                let cache_offset = layer_index.saturating_mul(kv_width);
                let mut cache_layer_key = k.clone();
                let rope_start = Instant::now();
                if cuda_cache.is_none() {
                    apply_rope_neox(
                        &mut q,
                        head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        &self.family_metadata,
                    );
                }
                apply_rope_neox(
                    &mut cache_layer_key,
                    kv_head_count,
                    head_dim,
                    rotary_dim,
                    position,
                    &self.family_metadata,
                );
                perf.stage_timings.rope_ns = perf
                    .stage_timings
                    .rope_ns
                    .saturating_add(duration_ns(rope_start));
                cache_key[cache_offset..cache_offset + kv_width]
                    .copy_from_slice(cache_layer_key.as_slice());
                cache_value[cache_offset..cache_offset + kv_width].copy_from_slice(v.as_slice());

                let use_fused_hybrid_ffn_prep = hybrid_selected4_plan.is_some()
                    && layer.attention_output_weight.storage.is_some()
                    && layer.feed_forward_router_weight_transposed_device.is_some()
                    && layer.feed_forward_norm_device.is_some()
                    && layer.feed_forward_gate_up_experts_weight.host.is_some()
                    && layer.feed_forward_down_experts_weight.host.is_some()
                    && self.family_metadata.expert_used_count == Some(4)
                    && can_use_q8_1_norm_fusion(hidden_size);
                let attention_output_start = Instant::now();
                let mut attention_out = Vec::new();
                let mut attention_output_device_ready = false;
                if let (Some(cuda_cache), Some(plan), Some(cache_write_index)) = (
                    cuda_cache.as_deref_mut(),
                    hybrid_selected4_plan.as_mut(),
                    cache_write_index,
                ) {
                    let q_rows = layer.attention_qkv_weight.rows_per_projection[0];
                    let k_rows = layer.attention_qkv_weight.rows_per_projection[1];
                    plan.qkv_scratch.clear();
                    plan.qkv_scratch.extend_from_slice(q.as_slice());
                    plan.qkv_scratch.extend_from_slice(k.as_slice());
                    plan.qkv_scratch.extend_from_slice(v.as_slice());
                    plan.qkv_host_buffer
                        .write_f32(plan.qkv_scratch.as_slice())
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    let mut submission = backend.begin_submission()?;
                    submission.copy_host_to_device(&plan.qkv_host_buffer, &plan.qkv_buffer)?;
                    submission.attention_decode_rope_cache_f16_kv(
                        &plan.qkv_buffer,
                        0,
                        q_rows,
                        q_rows.saturating_add(k_rows),
                        &cuda_cache.key_buffer,
                        &cuda_cache.value_buffer,
                        cuda_cache.width,
                        cache_offset,
                        cache_write_index,
                        self.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        layer.attention_sinks_device.as_ref(),
                        &plan.attention_input_buffer,
                    )?;
                    if let Some(storage) = layer.attention_output_weight.storage.as_ref() {
                        if can_use_q8_1_mmvq(layer.attention_output_weight.mode) {
                            submission.quantize_f32_to_q8_1(
                                &plan.attention_input_buffer,
                                1,
                                layer.attention_output_weight.columns,
                                &plan.hidden_input_q8_1_buffer,
                            )?;
                            submission.quantized_matvec_q8_1(
                                storage,
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                &plan.hidden_input_q8_1_buffer,
                                None,
                                &plan.output_buffer,
                            )?;
                        } else {
                            submission.quantized_matvec(
                                storage,
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                &plan.attention_input_buffer,
                                &plan.output_buffer,
                            )?;
                        }
                    }
                    let report =
                        submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                    perf.cuda.host_to_device_bytes = perf.cuda.host_to_device_bytes.saturating_add(
                        plan.qkv_scratch
                            .len()
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                    );
                    perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                    perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                    perf.cuda.kernel_launches = perf
                        .cuda
                        .kernel_launches
                        .saturating_add(report.encoded_operations);
                    if layer.attention_output_weight.storage.is_some() {
                        if use_fused_hybrid_ffn_prep {
                            attention_output_device_ready = true;
                        } else {
                            attention_out = plan
                                .output_buffer
                                .read_f32_at_offset(0, layer.attention_output_weight.rows)
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            perf.cuda.device_to_host_bytes =
                                perf.cuda.device_to_host_bytes.saturating_add(
                                    layer
                                        .attention_output_weight
                                        .rows
                                        .saturating_mul(std::mem::size_of::<f32>())
                                        .try_into()
                                        .unwrap_or(u64::MAX),
                                );
                        }
                    } else {
                        let attention = plan
                            .attention_input_buffer
                            .read_f32_at_offset(0, layer.attention_output_weight.columns)
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        perf.cuda.device_to_host_bytes =
                            perf.cuda.device_to_host_bytes.saturating_add(
                                layer
                                    .attention_output_weight
                                    .columns
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        let attention_out_stats = layer.attention_output_weight.matvec_profiled(
                            backend,
                            attention.as_slice(),
                            &mut attention_out,
                        )?;
                        accumulate_cuda_matvec_stats(&mut perf, &attention_out_stats);
                    }
                    perf.stage_timings.attention_ns = perf
                        .stage_timings
                        .attention_ns
                        .saturating_add(duration_ns(attention_output_start));
                } else {
                    let attention_start = Instant::now();
                    let attention = layer.attend(
                        layer_index,
                        q.as_slice(),
                        cache_layer_key.as_slice(),
                        v.as_slice(),
                        cache,
                        &self.descriptor,
                        self.family_metadata.sliding_window,
                    );
                    perf.stage_timings.attention_ns = perf
                        .stage_timings
                        .attention_ns
                        .saturating_add(duration_ns(attention_start));

                    let attention_out_stats = if let (Some(plan), Some(storage)) = (
                        hybrid_selected4_plan.as_mut(),
                        layer.attention_output_weight.storage.as_ref(),
                    ) {
                        if use_fused_hybrid_ffn_prep {
                            plan.attention_input_host_buffer
                                .write_f32(attention.as_slice())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            let mut submission = backend.begin_submission()?;
                            submission.copy_host_to_device(
                                &plan.attention_input_host_buffer,
                                &plan.attention_input_buffer,
                            )?;
                            submission.quantized_matvec(
                                storage,
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                &plan.attention_input_buffer,
                                &plan.output_buffer,
                            )?;
                            let report = submission
                                .commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                            attention_output_device_ready = true;
                            CudaQuantizedMatvecStats {
                                host_to_device_bytes: attention
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                                device_to_host_bytes: 0,
                                submission_count: 1,
                                sync_count: 1,
                                kernel_launches: report.encoded_operations,
                            }
                        } else {
                            let result = cuda_quantized_matvec_with_reused_buffers(
                                backend,
                                storage,
                                0,
                                layer.attention_output_weight.mode,
                                layer.attention_output_weight.rows,
                                layer.attention_output_weight.columns,
                                attention.as_slice(),
                                &mut plan.attention_input_host_buffer,
                                &plan.attention_input_buffer,
                                &plan.output_buffer,
                            )?;
                            attention_out = result.values;
                            result.stats
                        }
                    } else {
                        layer.attention_output_weight.matvec_profiled(
                            backend,
                            attention.as_slice(),
                            &mut attention_out,
                        )?
                    };
                    accumulate_cuda_matvec_stats(&mut perf, &attention_out_stats);
                }
                if !attention_output_device_ready {
                    if let Some(bias) = layer.attention_output_bias.as_ref() {
                        add_bias_in_place(&mut attention_out, bias.as_slice());
                    }
                }
                perf.stage_timings.attention_output_projection_ns = perf
                    .stage_timings
                    .attention_output_projection_ns
                    .saturating_add(duration_ns(attention_output_start));
                let mut ffn_residual = Vec::new();
                let ffn_input = if attention_output_device_ready {
                    Vec::new()
                } else {
                    hidden = add_vectors(attention_out.as_slice(), residual.as_slice())?;
                    ffn_residual = hidden.clone();
                    let feed_forward_norm_start = Instant::now();
                    let ffn_input = rms_norm(
                        hidden.as_slice(),
                        layer.feed_forward_norm.as_slice(),
                        self.family_metadata.rms_norm_epsilon,
                    );
                    perf.stage_timings.feed_forward_norm_ns = perf
                        .stage_timings
                        .feed_forward_norm_ns
                        .saturating_add(duration_ns(feed_forward_norm_start));
                    ffn_input
                };

                let router_start = Instant::now();
                let selected_count_target = self.family_metadata.expert_used_count.unwrap_or(0);
                let (selected, mut routing, routing_device_ready, hidden_input_q8_1_ready) =
                    if attention_output_device_ready {
                        let plan = hybrid_selected4_plan.as_mut().expect("checked above");
                        let router_transposed = layer
                            .feed_forward_router_weight_transposed_device
                            .as_ref()
                            .expect("checked above");
                        let feed_forward_norm = layer
                            .feed_forward_norm_device
                            .as_ref()
                            .expect("checked above");
                        plan.hidden_input_host_buffer
                            .write_f32(residual.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let mut submission = backend.begin_submission()?;
                        submission.copy_host_to_device(
                            &plan.hidden_input_host_buffer,
                            &plan.hidden_input_buffer,
                        )?;
                        submission.add_residual_rms_norm_q8_1(
                            &plan.output_buffer,
                            &plan.hidden_input_buffer,
                            layer.attention_output_bias_device.as_ref(),
                            feed_forward_norm,
                            &plan.output_buffer,
                            &plan.projected_buffer,
                            &plan.hidden_input_q8_1_buffer,
                            hidden_size,
                            self.family_metadata.rms_norm_epsilon,
                        )?;
                        submission.matmul(
                            &plan.projected_buffer,
                            router_transposed,
                            &plan.router_logits_buffer,
                            1,
                            layer.feed_forward_router_weight.columns,
                            layer.feed_forward_router_weight.rows,
                        )?;
                        if let Some(bias) = layer.feed_forward_router_bias_device.as_ref() {
                            submission.add_f32_in_place(
                                &plan.router_logits_buffer,
                                0,
                                bias,
                                layer.feed_forward_router_weight.rows,
                            )?;
                        }
                        submission.router_topk_delayed_softmax(
                            &plan.router_logits_buffer,
                            layer.feed_forward_router_weight.rows,
                            selected_count_target,
                            &plan.selected_expert_ids_buffer,
                            &plan.selected_weights_buffer,
                        )?;
                        let report =
                            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                        ffn_residual = plan
                            .output_buffer
                            .read_f32_at_offset(0, hidden_size)
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        plan.selected_expert_ids_scratch = i32_ne_bytes_to_values(
                            plan.selected_expert_ids_buffer.read_bytes()?.as_slice(),
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                        let selected = plan
                            .selected_expert_ids_scratch
                            .iter()
                            .copied()
                            .map(|index| {
                                usize::try_from(index).map_err(|_| {
                                    ReferenceTextGenerationError::Runtime(
                                        super::RuntimeError::Backend(format!(
                                            "cuda router returned negative expert index {index}"
                                        )),
                                    )
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        perf.cuda.host_to_device_bytes =
                            perf.cuda.host_to_device_bytes.saturating_add(
                                residual
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        perf.cuda.device_to_host_bytes =
                            perf.cuda.device_to_host_bytes.saturating_add(
                                selected_count_target
                                    .saturating_mul(std::mem::size_of::<i32>())
                                    .saturating_add(
                                        hidden_size.saturating_mul(std::mem::size_of::<f32>()),
                                    )
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                        perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                        perf.cuda.kernel_launches = perf
                            .cuda
                            .kernel_launches
                            .saturating_add(report.encoded_operations);
                        let fused_stage_ns = duration_ns(router_start);
                        let norm_stage_ns = fused_stage_ns / 2;
                        perf.stage_timings.feed_forward_norm_ns = perf
                            .stage_timings
                            .feed_forward_norm_ns
                            .saturating_add(norm_stage_ns);
                        perf.stage_timings.router_ns = perf
                            .stage_timings
                            .router_ns
                            .saturating_add(fused_stage_ns.saturating_sub(norm_stage_ns));
                        (selected, Vec::new(), true, true)
                    } else if let (Some(plan), Some(router_transposed)) = (
                        hybrid_selected4_plan.as_mut(),
                        layer.feed_forward_router_weight_transposed_device.as_ref(),
                    ) {
                        plan.hidden_input_host_buffer
                            .write_f32(ffn_input.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let mut submission = backend.begin_submission()?;
                        submission.copy_host_to_device(
                            &plan.hidden_input_host_buffer,
                            &plan.hidden_input_buffer,
                        )?;
                        submission.matmul(
                            &plan.hidden_input_buffer,
                            router_transposed,
                            &plan.router_logits_buffer,
                            1,
                            layer.feed_forward_router_weight.columns,
                            layer.feed_forward_router_weight.rows,
                        )?;
                        if let Some(bias) = layer.feed_forward_router_bias_device.as_ref() {
                            submission.add_f32_in_place(
                                &plan.router_logits_buffer,
                                0,
                                bias,
                                layer.feed_forward_router_weight.rows,
                            )?;
                        }
                        submission.router_topk_delayed_softmax(
                            &plan.router_logits_buffer,
                            layer.feed_forward_router_weight.rows,
                            selected_count_target,
                            &plan.selected_expert_ids_buffer,
                            &plan.selected_weights_buffer,
                        )?;
                        let report =
                            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                        plan.selected_expert_ids_scratch = i32_ne_bytes_to_values(
                            plan.selected_expert_ids_buffer.read_bytes()?.as_slice(),
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                        let selected =
                            plan.selected_expert_ids_scratch
                                .iter()
                                .copied()
                                .map(|index| {
                                    usize::try_from(index).map_err(|_| {
                                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                    format!("cuda router returned negative expert index {index}",),
                                ))
                            })
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                        perf.cuda.host_to_device_bytes =
                            perf.cuda.host_to_device_bytes.saturating_add(
                                ffn_input
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        perf.cuda.device_to_host_bytes =
                            perf.cuda.device_to_host_bytes.saturating_add(
                                selected_count_target
                                    .saturating_mul(std::mem::size_of::<i32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                        perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                        perf.cuda.kernel_launches = perf
                            .cuda
                            .kernel_launches
                            .saturating_add(report.encoded_operations);
                        (selected, Vec::new(), true, false)
                    } else {
                        let mut router_logits = Vec::new();
                        layer
                            .feed_forward_router_weight
                            .matvec(ffn_input.as_slice(), &mut router_logits)?;
                        if let Some(bias) = layer.feed_forward_router_bias.as_ref() {
                            add_bias_in_place(&mut router_logits, bias.as_slice());
                        }
                        let selected =
                            top_k_indices(router_logits.as_slice(), selected_count_target);
                        let routing =
                            softmax_selected(router_logits.as_slice(), selected.as_slice());
                        (selected, routing, false, false)
                    };
                if !attention_output_device_ready {
                    perf.stage_timings.router_ns = perf
                        .stage_timings
                        .router_ns
                        .saturating_add(duration_ns(router_start));
                }

                let mut moe_out = vec![0.0; hidden_size];
                let mut used_staged_selected4_cuda_path = false;
                if selected.len() <= 4 {
                    if let (Some(gate_up_host), Some(down_host)) = (
                        layer.feed_forward_gate_up_experts_weight.host.as_ref(),
                        layer.feed_forward_down_experts_weight.host.as_ref(),
                    ) {
                        let gate_rows = layer
                            .feed_forward_gate_up_experts_weight
                            .rows_per_projection[0];
                        let up_rows = layer
                            .feed_forward_gate_up_experts_weight
                            .rows_per_projection[1];
                        let selected_count = selected.len();
                        if selected_count == 4 {
                            if let Some(plan) = hybrid_selected4_plan.as_mut() {
                                let selected_key = <[usize; 4]>::try_from(selected.as_slice())
                                    .map_err(|_| {
                                        ReferenceTextGenerationError::Runtime(
                                            super::RuntimeError::Backend(String::from(
                                                "hybrid selected4 path received a non-4 expert set",
                                            )),
                                        )
                                    })?;
                                let gate_stride = gate_up_host
                                    .gate
                                    .rows
                                    .saturating_mul(gate_up_host.gate.row_byte_len);
                                let up_stride = gate_up_host
                                    .up
                                    .rows
                                    .saturating_mul(gate_up_host.up.row_byte_len);
                                let gate_up_expert_stride = gate_stride.saturating_add(up_stride);
                                let down_expert_stride =
                                    down_host.rows.saturating_mul(down_host.row_byte_len);
                                let mut staged_selected4_host_bytes = 0usize;
                                let mut selected_slot_ids = [0_i32, 1_i32, 2_i32, 3_i32];
                                let mut selected4_cache_hits = 0usize;
                                let mut selected4_cache_misses = 0usize;
                                if !routing_device_ready {
                                    plan.hidden_input_host_buffer
                                        .write_f32(ffn_input.as_slice())
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    plan.selected_weights_host_buffer
                                        .write_f32(routing.as_slice())
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                }
                                let using_layer_cache = if let Some(layer_cache) = plan
                                    .layer_caches
                                    .get_mut(layer_index)
                                    .and_then(Option::as_mut)
                                {
                                    let mut reserved_slots =
                                        [false; HYBRID_SELECTED4_LAYER_CACHE_MAX_SLOTS];
                                    let slot_count = layer_cache.slot_count;
                                    for (selected_index, expert_index) in
                                        selected_key.iter().copied().enumerate()
                                    {
                                        let slot = layer_cache
                                            .cached_experts
                                            .iter()
                                            .enumerate()
                                            .find(|(slot, cached)| {
                                                *slot < slot_count
                                                    && !reserved_slots[*slot]
                                                    && **cached == Some(expert_index)
                                            })
                                            .map(|(slot, _)| slot)
                                            .unwrap_or_else(|| {
                                                layer_cache
                                                    .cached_experts
                                                    .iter()
                                                    .enumerate()
                                                    .filter(|(slot, _)| {
                                                        *slot < slot_count && !reserved_slots[*slot]
                                                    })
                                                    .min_by_key(|(slot, cached)| {
                                                        (
                                                            cached.is_some(),
                                                            layer_cache.slot_last_used[*slot],
                                                        )
                                                    })
                                                    .map(|(slot, _)| slot)
                                                    .unwrap_or(0)
                                            });
                                        if layer_cache.cached_experts[slot] != Some(expert_index) {
                                            selected4_cache_misses =
                                                selected4_cache_misses.saturating_add(1);
                                            let (gate_bytes, up_bytes) = gate_up_host
                                                .expert_projection_bytes(expert_index)?;
                                            layer_cache
                                                .gate_up_weights_buffer
                                                .write_bytes_at_offset(
                                                    slot.saturating_mul(gate_up_expert_stride),
                                                    gate_bytes,
                                                )
                                                .map_err(ReferenceTextGenerationError::Runtime)?;
                                            layer_cache
                                                .gate_up_weights_buffer
                                                .write_bytes_at_offset(
                                                    slot.saturating_mul(gate_up_expert_stride)
                                                        .saturating_add(gate_stride),
                                                    up_bytes,
                                                )
                                                .map_err(ReferenceTextGenerationError::Runtime)?;
                                            staged_selected4_host_bytes =
                                                staged_selected4_host_bytes.saturating_add(
                                                    gate_bytes.len().saturating_add(up_bytes.len()),
                                                );
                                            let down_bytes =
                                                down_host.expert_bytes(expert_index)?;
                                            layer_cache
                                                .down_weights_buffer
                                                .write_bytes_at_offset(
                                                    slot.saturating_mul(down_expert_stride),
                                                    down_bytes,
                                                )
                                                .map_err(ReferenceTextGenerationError::Runtime)?;
                                            staged_selected4_host_bytes =
                                                staged_selected4_host_bytes
                                                    .saturating_add(down_bytes.len());
                                            if let Some(values) =
                                                layer.feed_forward_gate_experts_bias.as_ref()
                                            {
                                                let bias = selected_expert_bias_rows(
                                                    values.as_slice(),
                                                    gate_rows,
                                                    expert_index,
                                                )?;
                                                let device = layer_cache
                                                    .gate_bias_buffer
                                                    .as_mut()
                                                    .ok_or_else(|| {
                                                        ReferenceTextGenerationError::Runtime(
                                                            super::RuntimeError::Backend(
                                                                String::from(
                                                                    "missing hybrid selected4 gate bias buffer",
                                                                ),
                                                            ),
                                                        )
                                                    })?;
                                                device
                                                    .write_f32_at_offset(
                                                        slot.saturating_mul(gate_rows),
                                                        bias,
                                                    )
                                                    .map_err(
                                                        ReferenceTextGenerationError::Runtime,
                                                    )?;
                                                staged_selected4_host_bytes =
                                                    staged_selected4_host_bytes.saturating_add(
                                                        bias.len().saturating_mul(
                                                            std::mem::size_of::<f32>(),
                                                        ),
                                                    );
                                            }
                                            if let Some(values) =
                                                layer.feed_forward_up_experts_bias.as_ref()
                                            {
                                                let bias = selected_expert_bias_rows(
                                                    values.as_slice(),
                                                    up_rows,
                                                    expert_index,
                                                )?;
                                                let device = layer_cache
                                                    .up_bias_buffer
                                                    .as_mut()
                                                    .ok_or_else(|| {
                                                        ReferenceTextGenerationError::Runtime(
                                                            super::RuntimeError::Backend(
                                                                String::from(
                                                                    "missing hybrid selected4 up bias buffer",
                                                                ),
                                                            ),
                                                        )
                                                    })?;
                                                device
                                                    .write_f32_at_offset(
                                                        slot.saturating_mul(up_rows),
                                                        bias,
                                                    )
                                                    .map_err(
                                                        ReferenceTextGenerationError::Runtime,
                                                    )?;
                                                staged_selected4_host_bytes =
                                                    staged_selected4_host_bytes.saturating_add(
                                                        bias.len().saturating_mul(
                                                            std::mem::size_of::<f32>(),
                                                        ),
                                                    );
                                            }
                                            if let Some(values) =
                                                layer.feed_forward_down_experts_bias.as_ref()
                                            {
                                                let bias = selected_expert_bias_rows(
                                                    values.as_slice(),
                                                    layer.feed_forward_down_experts_weight.rows,
                                                    expert_index,
                                                )?;
                                                let device = layer_cache
                                                    .down_bias_buffer
                                                    .as_mut()
                                                    .ok_or_else(|| {
                                                        ReferenceTextGenerationError::Runtime(
                                                            super::RuntimeError::Backend(
                                                                String::from(
                                                                    "missing hybrid selected4 down bias buffer",
                                                                ),
                                                            ),
                                                        )
                                                    })?;
                                                device
                                                    .write_f32_at_offset(
                                                        slot.saturating_mul(
                                                            layer
                                                                .feed_forward_down_experts_weight
                                                                .rows,
                                                        ),
                                                        bias,
                                                    )
                                                    .map_err(
                                                        ReferenceTextGenerationError::Runtime,
                                                    )?;
                                                staged_selected4_host_bytes =
                                                    staged_selected4_host_bytes.saturating_add(
                                                        bias.len().saturating_mul(
                                                            std::mem::size_of::<f32>(),
                                                        ),
                                                    );
                                            }
                                            layer_cache.cached_experts[slot] = Some(expert_index);
                                        } else {
                                            selected4_cache_hits =
                                                selected4_cache_hits.saturating_add(1);
                                        }
                                        reserved_slots[slot] = true;
                                        layer_cache.slot_last_used[slot] = layer_cache.usage_clock;
                                        layer_cache.usage_clock =
                                            layer_cache.usage_clock.saturating_add(1);
                                        selected_slot_ids[selected_index] =
                                            i32::try_from(slot).map_err(|_| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(format!(
                                                        "hybrid selected4 slot {slot} exceeds i32 range",
                                                    )),
                                                )
                                            })?;
                                    }
                                    true
                                } else {
                                    gate_up_host.selected_packed_bytes_into(
                                        selected.as_slice(),
                                        &mut plan.gate_up_weights_scratch,
                                    )?;
                                    down_host.selected_bytes_into(
                                        selected.as_slice(),
                                        &mut plan.down_weights_scratch,
                                    )?;
                                    plan.gate_up_weights_host_buffer
                                        .write_bytes(plan.gate_up_weights_scratch.as_slice())
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    plan.down_weights_host_buffer
                                        .write_bytes(plan.down_weights_scratch.as_slice())
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                    staged_selected4_host_bytes = staged_selected4_host_bytes
                                        .saturating_add(plan.gate_up_weights_scratch.len())
                                        .saturating_add(plan.down_weights_scratch.len());

                                    if let Some(values) =
                                        layer.feed_forward_gate_experts_bias.as_ref()
                                    {
                                        gather_selected_expert_bias_rows_into(
                                            values.as_slice(),
                                            gate_rows,
                                            selected.as_slice(),
                                            &mut plan.gate_bias_scratch,
                                        )?;
                                        let host =
                                            plan.gate_bias_host_buffer.as_mut().ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 gate bias host buffer",
                                                    )),
                                                )
                                            })?;
                                        host.write_f32(plan.gate_bias_scratch.as_slice())
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                plan.gate_bias_scratch
                                                    .len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    if let Some(values) =
                                        layer.feed_forward_up_experts_bias.as_ref()
                                    {
                                        gather_selected_expert_bias_rows_into(
                                            values.as_slice(),
                                            up_rows,
                                            selected.as_slice(),
                                            &mut plan.up_bias_scratch,
                                        )?;
                                        let host =
                                            plan.up_bias_host_buffer.as_mut().ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 up bias host buffer",
                                                    )),
                                                )
                                            })?;
                                        host.write_f32(plan.up_bias_scratch.as_slice())
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                plan.up_bias_scratch
                                                    .len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    if let Some(values) =
                                        layer.feed_forward_down_experts_bias.as_ref()
                                    {
                                        gather_selected_expert_bias_rows_into(
                                            values.as_slice(),
                                            layer.feed_forward_down_experts_weight.rows,
                                            selected.as_slice(),
                                            &mut plan.down_bias_scratch,
                                        )?;
                                        let host =
                                            plan.down_bias_host_buffer.as_mut().ok_or_else(|| {
                                                ReferenceTextGenerationError::Runtime(
                                                    super::RuntimeError::Backend(String::from(
                                                        "missing hybrid selected4 down bias host buffer",
                                                    )),
                                                )
                                            })?;
                                        host.write_f32(plan.down_bias_scratch.as_slice())
                                            .map_err(ReferenceTextGenerationError::Runtime)?;
                                        staged_selected4_host_bytes = staged_selected4_host_bytes
                                            .saturating_add(
                                                plan.down_bias_scratch
                                                    .len()
                                                    .saturating_mul(std::mem::size_of::<f32>()),
                                            );
                                    }
                                    false
                                };
                                let selected_id_bytes =
                                    i32_slice_to_ne_bytes(selected_slot_ids.as_slice());
                                plan.selected_ids_buffer
                                    .write_bytes(selected_id_bytes.as_slice())
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                let layer_cache =
                                    plan.layer_caches.get(layer_index).and_then(Option::as_ref);
                                let gate_up_weights_device = layer_cache
                                    .map(|cache| &cache.gate_up_weights_buffer)
                                    .unwrap_or(&plan.gate_up_weights_buffer);
                                let down_weights_device = layer_cache
                                    .map(|cache| &cache.down_weights_buffer)
                                    .unwrap_or(&plan.down_weights_buffer);
                                let gate_bias_device = layer_cache
                                    .and_then(|cache| cache.gate_bias_buffer.as_ref())
                                    .or_else(|| plan.gate_bias_buffer.as_ref());
                                let up_bias_device = layer_cache
                                    .and_then(|cache| cache.up_bias_buffer.as_ref())
                                    .or_else(|| plan.up_bias_buffer.as_ref());
                                let down_bias_device = layer_cache
                                    .and_then(|cache| cache.down_bias_buffer.as_ref())
                                    .or_else(|| plan.down_bias_buffer.as_ref());

                                let expert_projection_start = Instant::now();
                                let mut submission = backend.begin_submission()?;
                                if !routing_device_ready {
                                    submission.copy_host_to_device(
                                        &plan.hidden_input_host_buffer,
                                        &plan.hidden_input_buffer,
                                    )?;
                                    submission.copy_host_to_device(
                                        &plan.selected_weights_host_buffer,
                                        &plan.selected_weights_buffer,
                                    )?;
                                }
                                if !using_layer_cache {
                                    submission.copy_host_to_device(
                                        &plan.gate_up_weights_host_buffer,
                                        gate_up_weights_device,
                                    )?;
                                    submission.copy_host_to_device(
                                        &plan.down_weights_host_buffer,
                                        down_weights_device,
                                    )?;
                                    if let (Some(host), Some(device)) =
                                        (plan.gate_bias_host_buffer.as_ref(), gate_bias_device)
                                    {
                                        submission.copy_host_to_device(host, device)?;
                                    }
                                    if let (Some(host), Some(device)) =
                                        (plan.up_bias_host_buffer.as_ref(), up_bias_device)
                                    {
                                        submission.copy_host_to_device(host, device)?;
                                    }
                                    if let (Some(host), Some(device)) =
                                        (plan.down_bias_host_buffer.as_ref(), down_bias_device)
                                    {
                                        submission.copy_host_to_device(host, device)?;
                                    }
                                }
                                if !hidden_input_q8_1_ready {
                                    submission.quantize_f32_to_q8_1(
                                        &plan.hidden_input_buffer,
                                        1,
                                        ffn_input.len(),
                                        &plan.hidden_input_q8_1_buffer,
                                    )?;
                                }
                                submission.moe_gate_up_swiglu_q8_1_selected4_quantized(
                                    gate_up_weights_device,
                                    layer.feed_forward_gate_up_experts_weight.mode,
                                    layer.feed_forward_gate_up_experts_weight.row_byte_len,
                                    layer.feed_forward_gate_up_experts_weight.total_rows(),
                                    layer.feed_forward_gate_up_experts_weight.columns,
                                    gate_rows,
                                    up_rows,
                                    &plan.selected_ids_buffer,
                                    selected_count,
                                    &plan.hidden_input_q8_1_buffer,
                                    gate_bias_device,
                                    up_bias_device,
                                    &plan.activated_q8_1_buffer,
                                )?;
                                submission.moe_down_project_q8_1_selected4(
                                    down_weights_device,
                                    layer.feed_forward_down_experts_weight.mode,
                                    layer.feed_forward_down_experts_weight.row_byte_len,
                                    layer.feed_forward_down_experts_weight.rows,
                                    layer.feed_forward_down_experts_weight.columns,
                                    &plan.selected_ids_buffer,
                                    selected_count,
                                    &plan.activated_q8_1_buffer,
                                    down_bias_device,
                                    &plan.projected_buffer,
                                )?;
                                submission.accumulate_selected4(
                                    &plan.projected_buffer,
                                    &plan.selected_weights_buffer,
                                    selected_count,
                                    hidden_size,
                                    None,
                                    &plan.output_buffer,
                                )?;
                                let report = submission
                                    .commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                                moe_out = plan.output_buffer.read_f32()?;
                                perf.stage_timings.expert_projection_ns = perf
                                    .stage_timings
                                    .expert_projection_ns
                                    .saturating_add(duration_ns(expert_projection_start));
                                perf.cuda.host_to_device_bytes =
                                    perf.cuda.host_to_device_bytes.saturating_add(
                                        ((if routing_device_ready {
                                            0
                                        } else {
                                            ffn_input
                                                .len()
                                                .saturating_mul(std::mem::size_of::<f32>())
                                        }) + if routing_device_ready {
                                            0
                                        } else {
                                            routing.len().saturating_mul(std::mem::size_of::<f32>())
                                        } + selected_id_bytes.len()
                                            + staged_selected4_host_bytes)
                                            as u64,
                                    );
                                perf.cuda.hybrid_selected4_cache_hits = perf
                                    .cuda
                                    .hybrid_selected4_cache_hits
                                    .saturating_add(selected4_cache_hits);
                                perf.cuda.hybrid_selected4_cache_misses = perf
                                    .cuda
                                    .hybrid_selected4_cache_misses
                                    .saturating_add(selected4_cache_misses);
                                perf.cuda.hybrid_selected4_cache_staged_bytes = perf
                                    .cuda
                                    .hybrid_selected4_cache_staged_bytes
                                    .saturating_add(
                                        staged_selected4_host_bytes.try_into().unwrap_or(u64::MAX),
                                    );
                                if perf.cuda.hybrid_selected4_layer_cache_hits.len() <= layer_index
                                {
                                    perf.cuda
                                        .hybrid_selected4_layer_cache_hits
                                        .resize(layer_index.saturating_add(1), 0);
                                }
                                perf.cuda.hybrid_selected4_layer_cache_hits[layer_index] =
                                    perf.cuda.hybrid_selected4_layer_cache_hits[layer_index]
                                        .saturating_add(selected4_cache_hits);
                                if perf.cuda.hybrid_selected4_layer_cache_misses.len()
                                    <= layer_index
                                {
                                    perf.cuda
                                        .hybrid_selected4_layer_cache_misses
                                        .resize(layer_index.saturating_add(1), 0);
                                }
                                perf.cuda.hybrid_selected4_layer_cache_misses[layer_index] =
                                    perf.cuda.hybrid_selected4_layer_cache_misses[layer_index]
                                        .saturating_add(selected4_cache_misses);
                                if perf.cuda.hybrid_selected4_layer_cache_staged_bytes.len()
                                    <= layer_index
                                {
                                    perf.cuda
                                        .hybrid_selected4_layer_cache_staged_bytes
                                        .resize(layer_index.saturating_add(1), 0);
                                }
                                perf.cuda.hybrid_selected4_layer_cache_staged_bytes[layer_index] =
                                    perf.cuda.hybrid_selected4_layer_cache_staged_bytes
                                        [layer_index]
                                        .saturating_add(
                                            staged_selected4_host_bytes
                                                .try_into()
                                                .unwrap_or(u64::MAX),
                                        );
                                perf.cuda.device_to_host_bytes =
                                    perf.cuda.device_to_host_bytes.saturating_add(
                                        hidden_size
                                            .saturating_mul(std::mem::size_of::<f32>())
                                            .try_into()
                                            .unwrap_or(u64::MAX),
                                    );
                                perf.cuda.submission_count =
                                    perf.cuda.submission_count.saturating_add(1);
                                perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                                perf.cuda.kernel_launches = perf
                                    .cuda
                                    .kernel_launches
                                    .saturating_add(report.encoded_operations);
                                used_staged_selected4_cuda_path = true;
                            }
                        }
                        if !used_staged_selected4_cuda_path {
                            if routing_device_ready && routing.is_empty() {
                                if let Some(plan) = hybrid_selected4_plan.as_ref() {
                                    routing = plan
                                        .selected_weights_buffer
                                        .read_f32_at_offset(0, selected_count)
                                        .map_err(ReferenceTextGenerationError::Runtime)?;
                                }
                            }
                            let selected_dense_ids = (0..selected_count)
                                .map(|index| {
                                    i32::try_from(index).map_err(|_| {
                                        ReferenceTextGenerationError::Runtime(
                                            super::RuntimeError::Backend(format!(
                                                "selected expert index {index} exceeds i32 range",
                                            )),
                                        )
                                    })
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let gate_up_weights =
                                gate_up_host.selected_packed_bytes(selected.as_slice())?;
                            let down_weights = down_host.selected_bytes(selected.as_slice())?;
                            let gate_bias = layer
                                .feed_forward_gate_experts_bias
                                .as_ref()
                                .map(|values| {
                                    gather_selected_expert_bias_rows(
                                        values.as_slice(),
                                        gate_rows,
                                        selected.as_slice(),
                                    )
                                })
                                .transpose()?;
                            let up_bias = layer
                                .feed_forward_up_experts_bias
                                .as_ref()
                                .map(|values| {
                                    gather_selected_expert_bias_rows(
                                        values.as_slice(),
                                        up_rows,
                                        selected.as_slice(),
                                    )
                                })
                                .transpose()?;
                            let down_bias = layer
                                .feed_forward_down_experts_bias
                                .as_ref()
                                .map(|values| {
                                    gather_selected_expert_bias_rows(
                                        values.as_slice(),
                                        layer.feed_forward_down_experts_weight.rows,
                                        selected.as_slice(),
                                    )
                                })
                                .transpose()?;
                            let input_buffer = backend.input_buffer(
                                Shape::new(vec![ffn_input.len()]),
                                ffn_input.clone(),
                            )?;
                            let input_q8_1 = backend.byte_buffer(&vec![
                                0_u8;
                                ggml_q8_1_storage_bytes(
                                    1,
                                    ffn_input.len()
                                )
                                .map_err(
                                    ReferenceTextGenerationError::Runtime
                                )?
                            ])?;
                            let selected_ids_buffer = backend.byte_buffer(
                                &i32_slice_to_ne_bytes(selected_dense_ids.as_slice()),
                            )?;
                            let selected_weights_buffer = backend
                                .input_buffer(Shape::new(vec![routing.len()]), routing.clone())?;
                            let gate_up_weights_buffer =
                                backend.byte_buffer(gate_up_weights.as_slice())?;
                            let down_weights_buffer =
                                backend.byte_buffer(down_weights.as_slice())?;
                            let gate_bias_buffer = gate_bias
                                .as_ref()
                                .map(|values| {
                                    backend.input_buffer(
                                        Shape::new(vec![values.len()]),
                                        values.clone(),
                                    )
                                })
                                .transpose()?;
                            let up_bias_buffer = up_bias
                                .as_ref()
                                .map(|values| {
                                    backend.input_buffer(
                                        Shape::new(vec![values.len()]),
                                        values.clone(),
                                    )
                                })
                                .transpose()?;
                            let down_bias_buffer = down_bias
                                .as_ref()
                                .map(|values| {
                                    backend.input_buffer(
                                        Shape::new(vec![values.len()]),
                                        values.clone(),
                                    )
                                })
                                .transpose()?;
                            let activated_q8_1 = backend.byte_buffer(&vec![
                            0_u8;
                            ggml_q8_1_storage_bytes(
                                selected_count,
                                gate_rows
                            )
                            .map_err(
                                ReferenceTextGenerationError::Runtime
                            )?
                        ])?;
                            let projected_buffer =
                                backend.f32_buffer(selected_count.saturating_mul(hidden_size))?;
                            let output_buffer = backend.f32_buffer(hidden_size)?;
                            let expert_projection_start = Instant::now();
                            let mut submission = backend.begin_submission()?;
                            submission.quantize_f32_to_q8_1(
                                &input_buffer,
                                1,
                                ffn_input.len(),
                                &input_q8_1,
                            )?;
                            submission.moe_gate_up_swiglu_q8_1_selected4_quantized(
                                &gate_up_weights_buffer,
                                layer.feed_forward_gate_up_experts_weight.mode,
                                layer.feed_forward_gate_up_experts_weight.row_byte_len,
                                layer.feed_forward_gate_up_experts_weight.total_rows(),
                                layer.feed_forward_gate_up_experts_weight.columns,
                                gate_rows,
                                up_rows,
                                &selected_ids_buffer,
                                selected_count,
                                &input_q8_1,
                                gate_bias_buffer.as_ref(),
                                up_bias_buffer.as_ref(),
                                &activated_q8_1,
                            )?;
                            submission.moe_down_project_q8_1_selected4(
                                &down_weights_buffer,
                                layer.feed_forward_down_experts_weight.mode,
                                layer.feed_forward_down_experts_weight.row_byte_len,
                                layer.feed_forward_down_experts_weight.rows,
                                layer.feed_forward_down_experts_weight.columns,
                                &selected_ids_buffer,
                                selected_count,
                                &activated_q8_1,
                                down_bias_buffer.as_ref(),
                                &projected_buffer,
                            )?;
                            submission.accumulate_selected4(
                                &projected_buffer,
                                &selected_weights_buffer,
                                selected_count,
                                hidden_size,
                                None,
                                &output_buffer,
                            )?;
                            let report = submission
                                .commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                            moe_out = output_buffer.read_f32()?;
                            perf.stage_timings.expert_projection_ns = perf
                                .stage_timings
                                .expert_projection_ns
                                .saturating_add(duration_ns(expert_projection_start));
                            perf.cuda.host_to_device_bytes =
                                perf.cuda.host_to_device_bytes.saturating_add(
                                    (ffn_input.len().saturating_mul(std::mem::size_of::<f32>())
                                        + routing.len().saturating_mul(std::mem::size_of::<f32>())
                                        + gate_up_weights.len()
                                        + down_weights.len()
                                        + selected_dense_ids
                                            .len()
                                            .saturating_mul(std::mem::size_of::<i32>())
                                        + gate_bias.as_ref().map_or(0, |values| {
                                            values.len().saturating_mul(std::mem::size_of::<f32>())
                                        })
                                        + up_bias.as_ref().map_or(0, |values| {
                                            values.len().saturating_mul(std::mem::size_of::<f32>())
                                        })
                                        + down_bias.as_ref().map_or(0, |values| {
                                            values.len().saturating_mul(std::mem::size_of::<f32>())
                                        })) as u64,
                                );
                            perf.cuda.device_to_host_bytes =
                                perf.cuda.device_to_host_bytes.saturating_add(
                                    hidden_size
                                        .saturating_mul(std::mem::size_of::<f32>())
                                        .try_into()
                                        .unwrap_or(u64::MAX),
                                );
                            perf.cuda.submission_count =
                                perf.cuda.submission_count.saturating_add(1);
                            perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                            perf.cuda.kernel_launches = perf
                                .cuda
                                .kernel_launches
                                .saturating_add(report.encoded_operations);
                            used_staged_selected4_cuda_path = true;
                        }
                    }
                }
                if !used_staged_selected4_cuda_path {
                    if routing_device_ready && routing.is_empty() {
                        if let Some(plan) = hybrid_selected4_plan.as_ref() {
                            routing = plan
                                .selected_weights_buffer
                                .read_f32_at_offset(0, selected.len())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                        }
                    }
                    for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                        let expert_projection_start = Instant::now();
                        let (mut gate_up_outputs, gate_up_stats) = layer
                            .feed_forward_gate_up_experts_weight
                            .expert_matvec_profiled(backend, expert_index, ffn_input.as_slice())?;
                        accumulate_cuda_matvec_stats(&mut perf, &gate_up_stats);
                        let mut gate = gate_up_outputs.remove(0);
                        if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                            add_expert_bias_in_place(
                                &mut gate,
                                bias.as_slice(),
                                expert_index,
                                layer
                                    .feed_forward_gate_up_experts_weight
                                    .rows_per_projection[0],
                            );
                        }

                        let mut up = gate_up_outputs.remove(0);
                        if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                            add_expert_bias_in_place(
                                &mut up,
                                bias.as_slice(),
                                expert_index,
                                layer
                                    .feed_forward_gate_up_experts_weight
                                    .rows_per_projection[1],
                            );
                        }
                        perf.stage_timings.expert_projection_ns = perf
                            .stage_timings
                            .expert_projection_ns
                            .saturating_add(duration_ns(expert_projection_start));

                        let expert_activation_start = Instant::now();
                        let activated = oai_swiglu(gate.as_slice(), up.as_slice());
                        perf.stage_timings.expert_activation_ns = perf
                            .stage_timings
                            .expert_activation_ns
                            .saturating_add(duration_ns(expert_activation_start));

                        let expert_down_start = Instant::now();
                        let mut expert = Vec::new();
                        let expert_stats = layer
                            .feed_forward_down_experts_weight
                            .expert_matvec_profiled(
                                backend,
                                expert_index,
                                activated.as_slice(),
                                &mut expert,
                            )?;
                        accumulate_cuda_matvec_stats(&mut perf, &expert_stats);
                        if let Some(bias) = layer.feed_forward_down_experts_bias.as_ref() {
                            add_expert_bias_in_place(
                                &mut expert,
                                bias.as_slice(),
                                expert_index,
                                layer.feed_forward_down_experts_weight.rows,
                            );
                        }
                        perf.stage_timings.expert_projection_ns = perf
                            .stage_timings
                            .expert_projection_ns
                            .saturating_add(duration_ns(expert_down_start));

                        let route = routing[selected_index];
                        let expert_aggregation_start = Instant::now();
                        for (dst, value) in moe_out.iter_mut().zip(expert.iter().copied()) {
                            *dst += value * route;
                        }
                        perf.stage_timings.expert_aggregation_ns = perf
                            .stage_timings
                            .expert_aggregation_ns
                            .saturating_add(duration_ns(expert_aggregation_start));
                    }
                }
                hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

                bytes_moved = bytes_moved
                    .saturating_add(layer.attention_qkv_weight.byte_length() as u64)
                    .saturating_add(layer.attention_output_weight.byte_length() as u64)
                    .saturating_add(layer.feed_forward_gate_up_experts_weight.byte_length() as u64)
                    .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
                kernel_count = kernel_count.saturating_add(5 + selected.len().saturating_mul(2));
            }

            if let (Some(cuda_cache), Some(cache_write_index)) =
                (cuda_cache.as_deref_mut(), cache_write_index)
            {
                cuda_cache.len = cache_write_index.saturating_add(1);
            }

            let logits_projection_start = Instant::now();
            let mut logits = Vec::new();
            let mut selected_token = None;
            if hidden_device_ready && materialize_host_kv {
                if let (Some(cuda_cache), Some(cache_write_index)) =
                    (cuda_cache.as_deref_mut(), cache_write_index)
                {
                    let (key, value) = cuda_cache.read_entry(cache_write_index)?;
                    perf.cuda.device_to_host_bytes = perf
                        .cuda
                        .device_to_host_bytes
                        .saturating_add(cache_key_value_byte_len(cuda_cache.width));
                    cache_key = key;
                    cache_value = value;
                }
            }
            if output_mode == CudaStepOutputMode::DeviceArgmax && hidden_device_ready {
                if let (Some(plan), Some(storage), Some(output_norm_device)) = (
                    hybrid_selected4_plan.as_mut(),
                    self.output.storage.as_ref(),
                    self.output_norm_device.as_ref(),
                ) {
                    let output_norm_start = Instant::now();
                    let mut submission = backend.begin_submission()?;
                    if can_use_q8_1_mmvq(self.output.mode)
                        && can_use_q8_1_norm_fusion(self.output.columns)
                    {
                        plan.argmax_state_host_buffer
                            .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.rms_norm_q8_1(
                            &plan.output_buffer,
                            output_norm_device,
                            &plan.hidden_input_q8_1_buffer,
                            self.output.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )?;
                        submission.copy_host_to_device(
                            &plan.argmax_state_host_buffer,
                            &plan.argmax_state_buffer,
                        )?;
                        submission.quantized_matvec_q8_1_argmax(
                            storage,
                            0,
                            self.output.mode,
                            self.output.rows,
                            self.output.columns,
                            &plan.hidden_input_q8_1_buffer,
                            None,
                            &plan.argmax_state_buffer,
                        )?;
                        submission.copy_device_to_host(
                            &plan.argmax_state_buffer,
                            &plan.argmax_state_host_buffer,
                        )?;
                    } else {
                        submission.rms_norm(
                            &plan.output_buffer,
                            output_norm_device,
                            &plan.hidden_input_buffer,
                            hidden_size,
                            self.family_metadata.rms_norm_epsilon,
                        )?;
                        if can_use_q8_1_mmvq(self.output.mode) {
                            plan.argmax_state_host_buffer
                                .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            submission.quantize_f32_to_q8_1(
                                &plan.hidden_input_buffer,
                                1,
                                self.output.columns,
                                &plan.hidden_input_q8_1_buffer,
                            )?;
                            submission.copy_host_to_device(
                                &plan.argmax_state_host_buffer,
                                &plan.argmax_state_buffer,
                            )?;
                            submission.quantized_matvec_q8_1_argmax(
                                storage,
                                0,
                                self.output.mode,
                                self.output.rows,
                                self.output.columns,
                                &plan.hidden_input_q8_1_buffer,
                                None,
                                &plan.argmax_state_buffer,
                            )?;
                            submission.copy_device_to_host(
                                &plan.argmax_state_buffer,
                                &plan.argmax_state_host_buffer,
                            )?;
                        } else {
                            submission.quantized_matvec(
                                storage,
                                0,
                                self.output.mode,
                                self.output.rows,
                                self.output.columns,
                                &plan.hidden_input_buffer,
                                &plan.logits_buffer,
                            )?;
                            submission.argmax_f32(
                                &plan.logits_buffer,
                                1,
                                self.output.rows,
                                &plan.next_token_buffer,
                            )?;
                            submission.copy_device_to_host(
                                &plan.next_token_buffer,
                                &plan.next_token_host_buffer,
                            )?;
                        }
                    }
                    let report =
                        submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                    perf.stage_timings.output_norm_ns = perf
                        .stage_timings
                        .output_norm_ns
                        .saturating_add(duration_ns(output_norm_start));
                    perf.cuda.device_to_host_bytes = perf.cuda.device_to_host_bytes.saturating_add(
                        if can_use_q8_1_mmvq(self.output.mode) {
                            std::mem::size_of::<u64>()
                        } else {
                            std::mem::size_of::<i32>()
                        }
                        .try_into()
                        .unwrap_or(u64::MAX),
                    );
                    if can_use_q8_1_mmvq(self.output.mode) {
                        perf.cuda.host_to_device_bytes =
                            perf.cuda.host_to_device_bytes.saturating_add(
                                std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX),
                            );
                        selected_token = Some(cuda_argmax_token_from_packed_host_buffer(
                            &plan.argmax_state_host_buffer,
                        )?);
                    } else {
                        let token = plan.next_token_host_buffer.read_i32().map_err(|error| {
                            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                format!(
                                    "cuda argmax returned an invalid host token buffer: {error}",
                                ),
                            ))
                        })?;
                        selected_token = Some(cuda_argmax_token_id(token)?);
                    }
                    perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                    perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                    perf.cuda.kernel_launches = perf
                        .cuda
                        .kernel_launches
                        .saturating_add(report.encoded_operations);
                    kernel_count = kernel_count.saturating_add(report.encoded_operations);
                } else {
                    let plan = hybrid_selected4_plan.as_ref().expect("checked above");
                    hidden = plan
                        .output_buffer
                        .read_f32_at_offset(0, hidden_size)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    perf.cuda.device_to_host_bytes = perf.cuda.device_to_host_bytes.saturating_add(
                        hidden_size
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                    );
                    hidden_device_ready = false;
                }
            }
            if !hidden_device_ready {
                let output_norm_start = Instant::now();
                let final_hidden = rms_norm(
                    hidden.as_slice(),
                    self.output_norm.as_slice(),
                    self.family_metadata.rms_norm_epsilon,
                );
                perf.stage_timings.output_norm_ns = perf
                    .stage_timings
                    .output_norm_ns
                    .saturating_add(duration_ns(output_norm_start));
                if output_mode == CudaStepOutputMode::DeviceArgmax {
                    if let (Some(plan), Some(storage)) =
                        (hybrid_selected4_plan.as_mut(), self.output.storage.as_ref())
                    {
                        plan.hidden_input_host_buffer
                            .write_f32(final_hidden.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let mut submission = backend.begin_submission()?;
                        submission.copy_host_to_device(
                            &plan.hidden_input_host_buffer,
                            &plan.hidden_input_buffer,
                        )?;
                        if can_use_q8_1_mmvq(self.output.mode) {
                            plan.argmax_state_host_buffer
                                .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            submission.quantize_f32_to_q8_1(
                                &plan.hidden_input_buffer,
                                1,
                                self.output.columns,
                                &plan.hidden_input_q8_1_buffer,
                            )?;
                            submission.copy_host_to_device(
                                &plan.argmax_state_host_buffer,
                                &plan.argmax_state_buffer,
                            )?;
                            submission.quantized_matvec_q8_1_argmax(
                                storage,
                                0,
                                self.output.mode,
                                self.output.rows,
                                self.output.columns,
                                &plan.hidden_input_q8_1_buffer,
                                None,
                                &plan.argmax_state_buffer,
                            )?;
                            submission.copy_device_to_host(
                                &plan.argmax_state_buffer,
                                &plan.argmax_state_host_buffer,
                            )?;
                        } else {
                            submission.quantized_matvec(
                                storage,
                                0,
                                self.output.mode,
                                self.output.rows,
                                self.output.columns,
                                &plan.hidden_input_buffer,
                                &plan.logits_buffer,
                            )?;
                            submission.argmax_f32(
                                &plan.logits_buffer,
                                1,
                                self.output.rows,
                                &plan.next_token_buffer,
                            )?;
                            submission.copy_device_to_host(
                                &plan.next_token_buffer,
                                &plan.next_token_host_buffer,
                            )?;
                        }
                        let report =
                            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                        perf.cuda.host_to_device_bytes =
                            perf.cuda.host_to_device_bytes.saturating_add(
                                final_hidden
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                                    .try_into()
                                    .unwrap_or(u64::MAX),
                            );
                        perf.cuda.device_to_host_bytes =
                            perf.cuda.device_to_host_bytes.saturating_add(
                                if can_use_q8_1_mmvq(self.output.mode) {
                                    std::mem::size_of::<u64>()
                                } else {
                                    std::mem::size_of::<i32>()
                                }
                                .try_into()
                                .unwrap_or(u64::MAX),
                            );
                        if can_use_q8_1_mmvq(self.output.mode) {
                            perf.cuda.host_to_device_bytes =
                                perf.cuda.host_to_device_bytes.saturating_add(
                                    std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX),
                                );
                            selected_token = Some(cuda_argmax_token_from_packed_host_buffer(
                                &plan.argmax_state_host_buffer,
                            )?);
                        } else {
                            let token = plan.next_token_host_buffer.read_i32().map_err(|error| {
                                ReferenceTextGenerationError::Runtime(
                                    super::RuntimeError::Backend(format!(
                                        "cuda argmax returned an invalid host token buffer: {error}",
                                    )),
                                )
                            })?;
                            selected_token = Some(cuda_argmax_token_id(token)?);
                        }
                        perf.cuda.submission_count = perf.cuda.submission_count.saturating_add(1);
                        perf.cuda.sync_count = perf.cuda.sync_count.saturating_add(1);
                        perf.cuda.kernel_launches = perf
                            .cuda
                            .kernel_launches
                            .saturating_add(report.encoded_operations);
                        kernel_count = kernel_count.saturating_add(report.encoded_operations);
                    } else {
                        let logits_stats = self.output.matvec_profiled(
                            backend,
                            final_hidden.as_slice(),
                            &mut logits,
                        )?;
                        accumulate_cuda_matvec_stats(&mut perf, &logits_stats);
                        selected_token = logits
                            .iter()
                            .copied()
                            .enumerate()
                            .max_by(|(_, left), (_, right)| left.total_cmp(right))
                            .and_then(|(index, _)| u32::try_from(index).ok().map(TokenId));
                        kernel_count = kernel_count.saturating_add(1);
                    }
                } else {
                    let logits_stats = if let (Some(plan), Some(storage)) =
                        (hybrid_selected4_plan.as_mut(), self.output.storage.as_ref())
                    {
                        let result = cuda_quantized_matvec_with_reused_buffers(
                            backend,
                            storage,
                            0,
                            self.output.mode,
                            self.output.rows,
                            self.output.columns,
                            final_hidden.as_slice(),
                            &mut plan.hidden_input_host_buffer,
                            &plan.hidden_input_buffer,
                            &plan.logits_buffer,
                        )?;
                        logits = result.values;
                        result.stats
                    } else {
                        self.output.matvec_profiled(
                            backend,
                            final_hidden.as_slice(),
                            &mut logits,
                        )?
                    };
                    accumulate_cuda_matvec_stats(&mut perf, &logits_stats);
                    kernel_count = kernel_count.saturating_add(1);
                }
            }
            bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
            perf.stage_timings.logits_projection_ns = perf
                .stage_timings
                .logits_projection_ns
                .saturating_add(duration_ns(logits_projection_start));

            Ok(GptOssForwardStep {
                key: cache_key,
                value: cache_value,
                logits,
                selected_token,
                kernel_count,
                bytes_moved,
                perf: Some(perf),
            })
        })();
        if let Some(plan) = hybrid_selected4_plan {
            self.release_hybrid_selected4_plan(plan);
        }
        result
    }

    fn forward_step(
        &self,
        backend: &mut CudaBackend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        self.forward_step_with_output_mode(
            backend,
            token,
            position,
            cache,
            CudaStepOutputMode::FullLogits,
            None,
            true,
        )
    }
}

#[derive(Clone, Debug)]
struct GptOssCudaLayer {
    attention_norm: Vec<f32>,
    attention_norm_device: Option<CudaBuffer>,
    attention_qkv_weight: CudaQuantizedProjectionGroup,
    attention_qkv_bias_device: Option<CudaBuffer>,
    attention_query_bias: Vec<f32>,
    attention_key_bias: Vec<f32>,
    attention_value_bias: Vec<f32>,
    attention_output_weight: CudaQuantizedMatrix,
    attention_output_bias: Option<Vec<f32>>,
    attention_output_bias_device: Option<CudaBuffer>,
    attention_sinks: Option<Vec<f32>>,
    attention_sinks_device: Option<CudaBuffer>,
    feed_forward_norm: Vec<f32>,
    feed_forward_norm_device: Option<CudaBuffer>,
    feed_forward_router_weight: DenseMatrix,
    feed_forward_router_weight_device: Option<CudaBuffer>,
    feed_forward_router_weight_transposed_device: Option<CudaBuffer>,
    feed_forward_router_bias: Option<Vec<f32>>,
    feed_forward_router_bias_device: Option<CudaBuffer>,
    feed_forward_gate_up_experts_weight: CudaQuantizedExpertProjectionGroup,
    feed_forward_gate_experts_bias: Option<Vec<f32>>,
    feed_forward_gate_experts_bias_device: Option<CudaBuffer>,
    feed_forward_up_experts_bias: Option<Vec<f32>>,
    feed_forward_up_experts_bias_device: Option<CudaBuffer>,
    feed_forward_down_experts_weight: CudaQuantizedExpertTensor,
    feed_forward_down_experts_bias: Option<Vec<f32>>,
    feed_forward_down_experts_bias_device: Option<CudaBuffer>,
}

impl GptOssCudaLayer {
    fn load(
        backend: &mut CudaBackend,
        artifact: &GgufBlobArtifact,
        layout: &GgufDecoderLayerTensorLayout,
        f16_mirror_state: &mut CudaF16MirrorState,
        placement: &mut CudaLoadPlacementPolicy,
    ) -> Result<Self, ModelLoadError> {
        let attention_norm = load_dense_vector(artifact, layout.attention_norm.as_str())?;
        let attention_norm_device = upload_optional_cuda_dense_buffer(
            backend,
            layout.attention_norm.as_str(),
            attention_norm.as_slice(),
            placement,
        )?;
        if let Some(buffer) = attention_norm_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let attention_query_bias = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_query_bias.as_ref(), "attention_query_bias")?,
        )?;
        let attention_key_bias = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_key_bias.as_ref(), "attention_key_bias")?,
        )?;
        let attention_value_bias = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_value_bias.as_ref(), "attention_value_bias")?,
        )?;
        let mut attention_qkv_bias = Vec::with_capacity(
            attention_query_bias
                .len()
                .saturating_add(attention_key_bias.len())
                .saturating_add(attention_value_bias.len()),
        );
        attention_qkv_bias.extend_from_slice(attention_query_bias.as_slice());
        attention_qkv_bias.extend_from_slice(attention_key_bias.as_slice());
        attention_qkv_bias.extend_from_slice(attention_value_bias.as_slice());
        let attention_qkv_bias_device = upload_optional_cuda_dense_buffer(
            backend,
            "attention_qkv_bias",
            attention_qkv_bias.as_slice(),
            placement,
        )?;
        if let Some(buffer) = attention_qkv_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let attention_output_bias = layout
            .attention_output_bias
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let attention_output_bias_device = if let Some(values) = attention_output_bias.as_ref() {
            upload_optional_cuda_dense_buffer(
                backend,
                "attention_output_bias",
                values.as_slice(),
                placement,
            )?
        } else {
            None
        };
        if let Some(buffer) = attention_output_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let attention_sinks = layout
            .attention_sinks_weight
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let attention_sinks_device = if let Some(values) = attention_sinks.as_ref() {
            upload_optional_cuda_dense_buffer(
                backend,
                "attention_sinks",
                values.as_slice(),
                placement,
            )?
        } else {
            None
        };
        if let Some(buffer) = attention_sinks_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.feed_forward_norm.as_ref(), "feed_forward_norm")?,
        )?;
        let feed_forward_norm_device = upload_optional_cuda_dense_buffer(
            backend,
            "feed_forward_norm",
            feed_forward_norm.as_slice(),
            placement,
        )?;
        if let Some(buffer) = feed_forward_norm_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_router_weight = load_dense_matrix(
            artifact,
            required_tensor_name(
                layout.feed_forward_router_weight.as_ref(),
                "feed_forward_router_weight",
            )?,
        )?;
        let hybrid_dense_mode = placement.host_backed_dense_tail || placement.host_backed_moe;
        let feed_forward_router_weight_device = if hybrid_dense_mode {
            None
        } else {
            upload_optional_cuda_aux_dense_buffer(
                backend,
                "feed_forward_router_weight",
                feed_forward_router_weight.values.as_slice(),
                placement,
            )?
        };
        if let Some(buffer) = feed_forward_router_weight_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_router_weight_transposed =
            transpose_dense_matrix_f32(&feed_forward_router_weight);
        let feed_forward_router_weight_transposed_device = upload_optional_cuda_aux_dense_buffer(
            backend,
            "feed_forward_router_weight_transposed",
            feed_forward_router_weight_transposed.as_slice(),
            placement,
        )?;
        if let Some(buffer) = feed_forward_router_weight_transposed_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_router_bias = layout
            .feed_forward_router_bias
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_router_bias_device =
            if let Some(values) = feed_forward_router_bias.as_ref() {
                upload_optional_cuda_aux_dense_buffer(
                    backend,
                    "feed_forward_router_bias",
                    values.as_slice(),
                    placement,
                )?
            } else {
                None
            };
        if let Some(buffer) = feed_forward_router_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_gate_experts_bias = layout
            .feed_forward_gate_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_gate_experts_bias_device = upload_optional_cuda_moe_bias_buffer(
            backend,
            "feed_forward_gate_experts_bias",
            feed_forward_gate_experts_bias.as_ref(),
            placement,
        )?;
        if let Some(buffer) = feed_forward_gate_experts_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_up_experts_bias = layout
            .feed_forward_up_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_up_experts_bias_device = upload_optional_cuda_moe_bias_buffer(
            backend,
            "feed_forward_up_experts_bias",
            feed_forward_up_experts_bias.as_ref(),
            placement,
        )?;
        if let Some(buffer) = feed_forward_up_experts_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let feed_forward_down_experts_bias = layout
            .feed_forward_down_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_down_experts_bias_device = upload_optional_cuda_moe_bias_buffer(
            backend,
            "feed_forward_down_experts_bias",
            feed_forward_down_experts_bias.as_ref(),
            placement,
        )?;
        if let Some(buffer) = feed_forward_down_experts_bias_device.as_ref() {
            placement.reserve_device_bytes(buffer.byte_len());
        }
        let attention_qkv_weight = load_cuda_quantized_projection_group(
            backend,
            artifact,
            &[
                layout.attention_query_weight.as_str(),
                layout.attention_key_weight.as_str(),
                layout.attention_value_weight.as_str(),
            ],
            false,
            f16_mirror_state,
            placement,
        )?;
        placement.reserve_device_bytes(attention_qkv_weight.device_residency_bytes());
        let attention_output_weight = load_cuda_quantized_matrix(
            backend,
            artifact,
            layout.attention_output_weight.as_str(),
            false,
            f16_mirror_state,
            placement,
        )?;
        placement.reserve_device_bytes(attention_output_weight.device_residency_bytes());
        let feed_forward_gate_up_experts_weight = load_cuda_quantized_expert_projection_group(
            backend,
            artifact,
            &[
                required_tensor_name(
                    layout.feed_forward_gate_experts_weight.as_ref(),
                    "feed_forward_gate_experts_weight",
                )?,
                required_tensor_name(
                    layout.feed_forward_up_experts_weight.as_ref(),
                    "feed_forward_up_experts_weight",
                )?,
            ],
            placement,
        )?;
        placement
            .reserve_device_bytes(feed_forward_gate_up_experts_weight.device_residency_bytes());
        let feed_forward_down_experts_weight = load_cuda_quantized_expert_tensor(
            backend,
            artifact,
            required_tensor_name(
                layout.feed_forward_down_experts_weight.as_ref(),
                "feed_forward_down_experts_weight",
            )?,
            placement,
        )?;
        placement.reserve_device_bytes(feed_forward_down_experts_weight.device_residency_bytes());
        Ok(Self {
            attention_norm,
            attention_norm_device,
            attention_qkv_weight,
            attention_qkv_bias_device,
            attention_query_bias,
            attention_key_bias,
            attention_value_bias,
            attention_output_weight,
            attention_output_bias,
            attention_output_bias_device,
            attention_sinks,
            attention_sinks_device,
            feed_forward_norm,
            feed_forward_norm_device,
            feed_forward_router_weight,
            feed_forward_router_weight_device,
            feed_forward_router_weight_transposed_device,
            feed_forward_router_bias,
            feed_forward_router_bias_device,
            feed_forward_gate_up_experts_weight,
            feed_forward_gate_experts_bias,
            feed_forward_gate_experts_bias_device,
            feed_forward_up_experts_bias,
            feed_forward_up_experts_bias_device,
            feed_forward_down_experts_weight,
            feed_forward_down_experts_bias,
            feed_forward_down_experts_bias_device,
        })
    }

    fn attend(
        &self,
        layer_index: usize,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        cache: &super::InMemoryKvCache,
        descriptor: &DecoderModelDescriptor,
        sliding_window: Option<usize>,
    ) -> Vec<f32> {
        attend_impl(
            self.attention_sinks.as_deref(),
            layer_index,
            query,
            key,
            value,
            cache,
            descriptor,
            sliding_window,
        )
    }
}

#[derive(Debug)]
struct GptOssMetalStepPlan {
    digest: String,
    hidden_norm_buffer: MetalBuffer,
    q_buffer: MetalBuffer,
    k_buffer: MetalBuffer,
    v_buffer: MetalBuffer,
    query_buffer: MetalBuffer,
    key_buffer: MetalBuffer,
    value_buffer: MetalBuffer,
    gate_up_output_buffer: MetalBuffer,
    expert_input_rows_buffer: MetalBuffer,
    expert_output_rows_buffer: MetalBuffer,
    final_hidden_buffer: MetalBuffer,
    logits_buffer: MetalBuffer,
    q_values: Vec<f32>,
    k_values: Vec<f32>,
    v_values: Vec<f32>,
    attention_values: Vec<f32>,
    gate_up_values: Vec<f32>,
    expert_input_values: Vec<f32>,
    expert_projected_values: Vec<f32>,
    cache_key: Vec<f32>,
    cache_value: Vec<f32>,
}

#[derive(Debug)]
struct GptOssMetalModelInner {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GptOssTokenizer,
    decode_graph: GptOssDecodeGraph,
    token_embedding: QuantizedMatrix,
    output_norm: Vec<f32>,
    output: MetalQuantizedMatrix,
    layers: Vec<GptOssMetalLayer>,
    plan_digest: String,
    decode_step_plan: Mutex<Option<GptOssMetalStepPlan>>,
    load_duration_ns: u64,
}

impl GptOssMetalModelInner {
    fn cache_width(&self) -> usize {
        self.descriptor
            .config
            .layer_count
            .saturating_mul(self.descriptor.config.kv_width())
    }

    fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn layer_kv_width(&self) -> usize {
        self.descriptor.config.kv_width()
    }

    fn graph_node_count(&self) -> usize {
        self.decode_graph.node_count()
    }

    fn graph_layer_node_count(&self) -> usize {
        self.decode_graph.layer_node_count()
    }

    fn acquire_decode_step_plan(
        &self,
        backend: &mut MetalBackend,
    ) -> Result<(GptOssMetalStepPlan, CompilePathEvidence, bool), ReferenceTextGenerationError>
    {
        let cache_hit = self
            .decode_step_plan
            .lock()
            .map_err(|_| {
                ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                    "metal gpt-oss decode-step plan cache is poisoned",
                )))
            })?
            .take();
        if let Some(plan) = cache_hit {
            return Ok((plan, metal_decode_step_plan_compile_path(true), true));
        }
        Ok((
            self.build_decode_step_plan(backend)?,
            metal_decode_step_plan_compile_path(false),
            false,
        ))
    }

    fn release_decode_step_plan(&self, plan: GptOssMetalStepPlan) {
        if let Ok(mut cached_plan) = self.decode_step_plan.lock() {
            *cached_plan = Some(plan);
        }
    }

    fn build_decode_step_plan(
        &self,
        backend: &mut MetalBackend,
    ) -> Result<GptOssMetalStepPlan, ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let head_count = self.descriptor.config.block.attention.head_count;
        let kv_head_count = self.descriptor.config.block.attention.kv_head_count;
        let head_dim = self.descriptor.config.block.attention.head_dim;
        let q_rows = head_count.saturating_mul(head_dim);
        let kv_rows = kv_head_count.saturating_mul(head_dim);
        let selected_count = self.family_metadata.expert_used_count.ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(String::from(
                "metal gpt-oss path requires expert_used_count metadata",
            )))
        })?;
        let max_gate_up_rows = self
            .layers
            .iter()
            .map(|layer| layer.feed_forward_gate_up_experts_weight.total_rows())
            .max()
            .unwrap_or(hidden_size);
        let max_down_columns = self
            .layers
            .iter()
            .map(|layer| layer.feed_forward_down_experts_weight.columns)
            .max()
            .unwrap_or(hidden_size);
        let max_down_rows = self
            .layers
            .iter()
            .map(|layer| layer.feed_forward_down_experts_weight.rows)
            .max()
            .unwrap_or(hidden_size);

        Ok(GptOssMetalStepPlan {
            digest: format!("{}:metal-decode-step", self.plan_digest),
            hidden_norm_buffer: backend
                .input_buffer(Shape::new(vec![hidden_size]), vec![0.0; hidden_size])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            q_buffer: backend
                .input_buffer(Shape::new(vec![q_rows]), vec![0.0; q_rows])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            k_buffer: backend
                .input_buffer(Shape::new(vec![kv_rows]), vec![0.0; kv_rows])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            v_buffer: backend
                .input_buffer(Shape::new(vec![kv_rows]), vec![0.0; kv_rows])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            query_buffer: backend
                .input_buffer(
                    Shape::new(vec![1, head_count, 1, head_dim]),
                    vec![0.0; q_rows],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            key_buffer: backend
                .input_buffer(
                    Shape::new(vec![1, kv_head_count, 1, head_dim]),
                    vec![0.0; kv_rows],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            value_buffer: backend
                .input_buffer(
                    Shape::new(vec![1, kv_head_count, 1, head_dim]),
                    vec![0.0; kv_rows],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up_output_buffer: backend
                .input_buffer(
                    Shape::new(vec![selected_count.saturating_mul(max_gate_up_rows)]),
                    vec![0.0; selected_count.saturating_mul(max_gate_up_rows)],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            expert_input_rows_buffer: backend
                .input_buffer(
                    Shape::new(vec![selected_count.saturating_mul(max_down_columns)]),
                    vec![0.0; selected_count.saturating_mul(max_down_columns)],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            expert_output_rows_buffer: backend
                .input_buffer(
                    Shape::new(vec![selected_count.saturating_mul(max_down_rows)]),
                    vec![0.0; selected_count.saturating_mul(max_down_rows)],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            final_hidden_buffer: backend
                .input_buffer(Shape::new(vec![hidden_size]), vec![0.0; hidden_size])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            logits_buffer: backend
                .input_buffer(
                    Shape::new(vec![self.output.host.rows]),
                    vec![0.0; self.output.host.rows],
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            q_values: vec![0.0; q_rows],
            k_values: vec![0.0; kv_rows],
            v_values: vec![0.0; kv_rows],
            attention_values: vec![0.0; q_rows],
            gate_up_values: vec![0.0; selected_count.saturating_mul(max_gate_up_rows)],
            expert_input_values: Vec::with_capacity(
                selected_count.saturating_mul(max_down_columns),
            ),
            expert_projected_values: vec![0.0; selected_count.saturating_mul(max_down_rows)],
            cache_key: vec![0.0; self.cache_width()],
            cache_value: vec![0.0; self.cache_width()],
        })
    }

    fn reserve_decode_attention_runtime(
        &self,
        backend: &mut MetalBackend,
    ) -> Result<MetalAttentionGraphRuntime, ReferenceTextGenerationError> {
        backend
            .reserve_attention_graph(MetalAttentionGraphReserve {
                kind: MetalGraphReserveKind::Decode,
                batch_size: 1,
                sequence_len: 1,
                query_head_count: self.descriptor.config.block.attention.head_count,
                kv_head_count: self.descriptor.config.block.attention.kv_head_count,
                head_dim: self.descriptor.config.block.attention.head_dim,
                max_context_tokens: self.descriptor.config.max_context,
                causal: true,
                interleaved: false,
                flash_attention: backend.supports_flash_attention(),
            })
            .map_err(ReferenceTextGenerationError::Runtime)
    }

    fn select_step_logits_output(
        &self,
        backend: &mut MetalBackend,
        final_hidden: &[f32],
        perf: &mut GptOssPerformanceMetrics,
        bytes_moved: &mut u64,
        kernel_count: &mut usize,
        output_mode: MetalLogitsOutputMode,
        record_decode_logits_metrics: bool,
    ) -> Result<(Vec<f32>, Option<TokenId>), ReferenceTextGenerationError> {
        let logits_projection_start = Instant::now();
        let selection = self
            .output
            .select_logits_output(backend, final_hidden, output_mode)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        perf.stage_timings.logits_projection_ns = perf
            .stage_timings
            .logits_projection_ns
            .saturating_add(duration_ns(logits_projection_start));
        if record_decode_logits_metrics {
            accumulate_metal_decode_logits_metrics(
                perf,
                selection.metrics.output_mode,
                selection.metrics.readback_bytes,
                selection.metrics.raw_logits_materialized,
            );
        }
        perf.metal.device_to_host_bytes = perf
            .metal
            .device_to_host_bytes
            .saturating_add(selection.metrics.readback_bytes);
        *bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
        *kernel_count = kernel_count.saturating_add(1);

        match output_mode {
            MetalLogitsOutputMode::GreedyToken => {
                let token = selection.selected_tokens.first().copied().ok_or_else(|| {
                    ReferenceTextGenerationError::MissingOutput("metal greedy token")
                })?;
                Ok((Vec::new(), Some(TokenId(token))))
            }
            MetalLogitsOutputMode::TopKCandidates(_) => {
                let candidates = selection.candidates.as_ref().ok_or_else(|| {
                    ReferenceTextGenerationError::MissingOutput("metal logits top_k candidates")
                })?;
                Ok((
                    expand_metal_top_k_candidates_to_logits(
                        self.descriptor.config.vocab_size,
                        candidates,
                    )?,
                    None,
                ))
            }
            MetalLogitsOutputMode::RawLogits => Ok((
                selection
                    .logits
                    .ok_or(ReferenceTextGenerationError::MissingOutput(
                        "metal raw logits",
                    ))?,
                None,
            )),
        }
    }

    fn forward_step(
        &self,
        backend: &mut MetalBackend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
        logits_output_mode: MetalLogitsOutputMode,
        record_decode_logits_metrics: bool,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let kv_width = self.descriptor.config.kv_width();
        let mut bytes_moved = 0_u64;
        let mut kernel_count = 0_usize;
        let mut perf = GptOssPerformanceMetrics {
            step_count: 1,
            layer_visit_count: self.layers.len(),
            graph_node_count: self.graph_node_count(),
            graph_layer_node_count: self.graph_layer_node_count(),
            ..GptOssPerformanceMetrics::default()
        };

        let token_embedding_start = Instant::now();
        let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
        perf.stage_timings.token_embedding_ns = perf
            .stage_timings
            .token_embedding_ns
            .saturating_add(duration_ns(token_embedding_start));
        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

        let mut cache_key = vec![0.0; self.cache_width()];
        let mut cache_value = vec![0.0; self.cache_width()];

        for (layer_index, layer) in self.layers.iter().enumerate() {
            let residual = hidden.clone();
            let attention_norm_start = Instant::now();
            let hidden_norm = rms_norm(
                hidden.as_slice(),
                layer.attention_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.attention_norm_ns = perf
                .stage_timings
                .attention_norm_ns
                .saturating_add(duration_ns(attention_norm_start));

            let qkv_start = Instant::now();
            let mut q = Vec::new();
            layer
                .attention_query_weight
                .matvec(backend, hidden_norm.as_slice(), &mut q)?;
            add_bias_in_place(&mut q, layer.attention_query_bias.as_slice());
            let mut k = Vec::new();
            layer
                .attention_key_weight
                .matvec(backend, hidden_norm.as_slice(), &mut k)?;
            add_bias_in_place(&mut k, layer.attention_key_bias.as_slice());
            let mut v = Vec::new();
            layer
                .attention_value_weight
                .matvec(backend, hidden_norm.as_slice(), &mut v)?;
            add_bias_in_place(&mut v, layer.attention_value_bias.as_slice());
            perf.stage_timings.qkv_projection_ns = perf
                .stage_timings
                .qkv_projection_ns
                .saturating_add(duration_ns(qkv_start));

            let rope_start = Instant::now();
            apply_rope_neox(
                &mut q,
                self.descriptor.config.block.attention.head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                position,
                &self.family_metadata,
            );
            apply_rope_neox(
                &mut k,
                self.descriptor.config.block.attention.kv_head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                position,
                &self.family_metadata,
            );
            perf.stage_timings.rope_ns = perf
                .stage_timings
                .rope_ns
                .saturating_add(duration_ns(rope_start));

            let cache_offset = layer_index.saturating_mul(kv_width);
            cache_key[cache_offset..cache_offset + kv_width].copy_from_slice(k.as_slice());
            cache_value[cache_offset..cache_offset + kv_width].copy_from_slice(v.as_slice());

            let attention_start = Instant::now();
            let attention = layer.attend(
                layer_index,
                q.as_slice(),
                k.as_slice(),
                v.as_slice(),
                cache,
                &self.descriptor,
                self.family_metadata.sliding_window,
            );
            perf.stage_timings.attention_ns = perf
                .stage_timings
                .attention_ns
                .saturating_add(duration_ns(attention_start));

            let attention_output_start = Instant::now();
            let mut attention_out = Vec::new();
            layer.attention_output_weight.matvec(
                backend,
                attention.as_slice(),
                &mut attention_out,
            )?;
            if let Some(bias) = layer.attention_output_bias.as_ref() {
                add_bias_in_place(&mut attention_out, bias.as_slice());
            }
            perf.stage_timings.attention_output_projection_ns = perf
                .stage_timings
                .attention_output_projection_ns
                .saturating_add(duration_ns(attention_output_start));
            hidden = add_vectors(attention_out.as_slice(), residual.as_slice())?;

            let ffn_residual = hidden.clone();
            let feed_forward_norm_start = Instant::now();
            let ffn_input = rms_norm(
                hidden.as_slice(),
                layer.feed_forward_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.feed_forward_norm_ns = perf
                .stage_timings
                .feed_forward_norm_ns
                .saturating_add(duration_ns(feed_forward_norm_start));

            let router_start = Instant::now();
            let mut router_logits = Vec::new();
            layer
                .feed_forward_router_weight
                .matvec(ffn_input.as_slice(), &mut router_logits)?;
            if let Some(bias) = layer.feed_forward_router_bias.as_ref() {
                add_bias_in_place(&mut router_logits, bias.as_slice());
            }
            let selected = top_k_indices(
                router_logits.as_slice(),
                self.family_metadata.expert_used_count.unwrap_or(0),
            );
            let routing = softmax_selected(router_logits.as_slice(), selected.as_slice());
            perf.stage_timings.router_ns = perf
                .stage_timings
                .router_ns
                .saturating_add(duration_ns(router_start));

            let mut moe_out = vec![0.0; hidden_size];
            if !selected.is_empty() {
                let expert_projection_start = Instant::now();
                let gate_up_outputs = layer.feed_forward_gate_up_experts_weight.selected_matvec(
                    backend,
                    selected.as_slice(),
                    ffn_input.as_slice(),
                )?;
                perf.stage_timings.expert_projection_ns = perf
                    .stage_timings
                    .expert_projection_ns
                    .saturating_add(duration_ns(expert_projection_start));

                for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                    let mut gate = gate_up_outputs[selected_index][0].clone();
                    if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut gate,
                            bias.as_slice(),
                            expert_index,
                            layer
                                .feed_forward_gate_up_experts_weight
                                .rows_per_projection[0],
                        );
                    }

                    let mut up = gate_up_outputs[selected_index][1].clone();
                    if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut up,
                            bias.as_slice(),
                            expert_index,
                            layer
                                .feed_forward_gate_up_experts_weight
                                .rows_per_projection[1],
                        );
                    }

                    let expert_activation_start = Instant::now();
                    let activated = oai_swiglu(gate.as_slice(), up.as_slice());
                    perf.stage_timings.expert_activation_ns = perf
                        .stage_timings
                        .expert_activation_ns
                        .saturating_add(duration_ns(expert_activation_start));

                    let expert_down_start = Instant::now();
                    let mut expert = Vec::new();
                    layer.feed_forward_down_experts_weight.expert_matvec(
                        backend,
                        expert_index,
                        activated.as_slice(),
                        &mut expert,
                    )?;
                    if let Some(bias) = layer.feed_forward_down_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut expert,
                            bias.as_slice(),
                            expert_index,
                            layer.feed_forward_down_experts_weight.rows,
                        );
                    }
                    perf.stage_timings.expert_projection_ns = perf
                        .stage_timings
                        .expert_projection_ns
                        .saturating_add(duration_ns(expert_down_start));

                    let route = routing[selected_index];
                    let expert_aggregation_start = Instant::now();
                    for (dst, value) in moe_out.iter_mut().zip(expert.iter().copied()) {
                        *dst += value * route;
                    }
                    perf.stage_timings.expert_aggregation_ns = perf
                        .stage_timings
                        .expert_aggregation_ns
                        .saturating_add(duration_ns(expert_aggregation_start));
                }
            }
            hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
            kernel_count = kernel_count.saturating_add(5 + selected.len().saturating_mul(2));
        }

        let output_norm_start = Instant::now();
        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        perf.stage_timings.output_norm_ns = perf
            .stage_timings
            .output_norm_ns
            .saturating_add(duration_ns(output_norm_start));

        let (logits, selected_token) = self.select_step_logits_output(
            backend,
            final_hidden.as_slice(),
            &mut perf,
            &mut bytes_moved,
            &mut kernel_count,
            logits_output_mode,
            record_decode_logits_metrics,
        )?;

        Ok(GptOssForwardStep {
            key: cache_key,
            value: cache_value,
            logits,
            selected_token,
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }

    #[allow(dead_code)]
    fn forward_step_with_device_attention(
        &self,
        backend: &mut MetalBackend,
        token: TokenId,
        position: usize,
        layer_caches: &mut [MetalKvCacheMirror],
        attention_runtime: &mut MetalAttentionGraphRuntime,
        logits_output_mode: MetalLogitsOutputMode,
        record_decode_logits_metrics: bool,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        if layer_caches.len() != self.layers.len() {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(format!(
                    "metal layer cache count mismatch: expected {}, actual {}",
                    self.layers.len(),
                    layer_caches.len()
                )),
            ));
        }

        let hidden_size = self.descriptor.config.hidden_size;
        let kv_width = self.descriptor.config.kv_width();
        let head_count = self.descriptor.config.block.attention.head_count;
        let kv_head_count = self.descriptor.config.block.attention.kv_head_count;
        let head_dim = self.descriptor.config.block.attention.head_dim;
        let rotary_dim = self.descriptor.config.block.attention.rotary_dim;
        let mut bytes_moved = 0_u64;
        let mut kernel_count = 0_usize;
        let mut perf = GptOssPerformanceMetrics {
            step_count: 1,
            layer_visit_count: self.layers.len(),
            graph_node_count: self.graph_node_count(),
            graph_layer_node_count: self.graph_layer_node_count(),
            ..GptOssPerformanceMetrics::default()
        };

        let token_embedding_start = Instant::now();
        let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
        perf.stage_timings.token_embedding_ns = perf
            .stage_timings
            .token_embedding_ns
            .saturating_add(duration_ns(token_embedding_start));
        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

        let (cos_values, sin_values) =
            metal_rope_cos_sin_values(position, head_dim, rotary_dim, &self.family_metadata);
        let cos = backend
            .input_buffer(Shape::new(vec![1, head_dim / 2]), cos_values)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let sin = backend
            .input_buffer(Shape::new(vec![1, head_dim / 2]), sin_values)
            .map_err(ReferenceTextGenerationError::Runtime)?;

        let mut cache_key = vec![0.0; self.cache_width()];
        let mut cache_value = vec![0.0; self.cache_width()];

        for (layer_index, (layer, layer_cache)) in
            self.layers.iter().zip(layer_caches.iter_mut()).enumerate()
        {
            if layer_cache.width() != kv_width {
                return Err(ReferenceTextGenerationError::Runtime(
                    super::RuntimeError::Backend(format!(
                        "metal layer cache width mismatch: expected {}, actual {}",
                        kv_width,
                        layer_cache.width()
                    )),
                ));
            }
            let residual = hidden.clone();
            let attention_norm_start = Instant::now();
            let hidden_norm = rms_norm(
                hidden.as_slice(),
                layer.attention_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.attention_norm_ns = perf
                .stage_timings
                .attention_norm_ns
                .saturating_add(duration_ns(attention_norm_start));

            let qkv_start = Instant::now();
            let mut q = Vec::new();
            layer
                .attention_query_weight
                .matvec(backend, hidden_norm.as_slice(), &mut q)?;
            add_bias_in_place(&mut q, layer.attention_query_bias.as_slice());
            let mut k = Vec::new();
            layer
                .attention_key_weight
                .matvec(backend, hidden_norm.as_slice(), &mut k)?;
            add_bias_in_place(&mut k, layer.attention_key_bias.as_slice());
            let mut v = Vec::new();
            layer
                .attention_value_weight
                .matvec(backend, hidden_norm.as_slice(), &mut v)?;
            add_bias_in_place(&mut v, layer.attention_value_bias.as_slice());
            perf.stage_timings.qkv_projection_ns = perf
                .stage_timings
                .qkv_projection_ns
                .saturating_add(duration_ns(qkv_start));

            let rope_start = Instant::now();
            let mut cache_layer_key = k.clone();
            apply_rope_neox(
                &mut cache_layer_key,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                &self.family_metadata,
            );
            perf.stage_timings.rope_ns = perf
                .stage_timings
                .rope_ns
                .saturating_add(duration_ns(rope_start));

            let cache_offset = layer_index.saturating_mul(kv_width);
            cache_key[cache_offset..cache_offset + kv_width]
                .copy_from_slice(cache_layer_key.as_slice());
            cache_value[cache_offset..cache_offset + kv_width].copy_from_slice(v.as_slice());

            let attention_start = Instant::now();
            let query = backend
                .input_buffer(Shape::new(vec![1, head_count, 1, head_dim]), q)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let key = backend
                .input_buffer(Shape::new(vec![1, kv_head_count, 1, head_dim]), k)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let value = backend
                .input_buffer(Shape::new(vec![1, kv_head_count, 1, head_dim]), v.clone())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let attention = backend
                .decode_attention_f32_reserved(
                    attention_runtime,
                    &query,
                    &key,
                    &value,
                    &cos,
                    &sin,
                    layer_cache,
                    1.0 / (head_dim as f32).sqrt(),
                    true,
                    false,
                    true,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let attention_values = attention
                .output
                .read_f32()
                .map_err(ReferenceTextGenerationError::Runtime)?;
            perf.stage_timings.attention_ns = perf
                .stage_timings
                .attention_ns
                .saturating_add(duration_ns(attention_start));

            let attention_output_start = Instant::now();
            let mut attention_out = Vec::new();
            layer.attention_output_weight.matvec(
                backend,
                attention_values.as_slice(),
                &mut attention_out,
            )?;
            if let Some(bias) = layer.attention_output_bias.as_ref() {
                add_bias_in_place(&mut attention_out, bias.as_slice());
            }
            perf.stage_timings.attention_output_projection_ns = perf
                .stage_timings
                .attention_output_projection_ns
                .saturating_add(duration_ns(attention_output_start));
            hidden = add_vectors(attention_out.as_slice(), residual.as_slice())?;

            let ffn_residual = hidden.clone();
            let feed_forward_norm_start = Instant::now();
            let ffn_input = rms_norm(
                hidden.as_slice(),
                layer.feed_forward_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.feed_forward_norm_ns = perf
                .stage_timings
                .feed_forward_norm_ns
                .saturating_add(duration_ns(feed_forward_norm_start));

            let router_start = Instant::now();
            let mut router_logits = Vec::new();
            layer
                .feed_forward_router_weight
                .matvec(ffn_input.as_slice(), &mut router_logits)?;
            if let Some(bias) = layer.feed_forward_router_bias.as_ref() {
                add_bias_in_place(&mut router_logits, bias.as_slice());
            }
            let selected = top_k_indices(
                router_logits.as_slice(),
                self.family_metadata.expert_used_count.unwrap_or(0),
            );
            let routing = softmax_selected(router_logits.as_slice(), selected.as_slice());
            perf.stage_timings.router_ns = perf
                .stage_timings
                .router_ns
                .saturating_add(duration_ns(router_start));

            let mut moe_out = vec![0.0; hidden_size];
            if !selected.is_empty() {
                let expert_projection_start = Instant::now();
                let gate_up_outputs = layer.feed_forward_gate_up_experts_weight.selected_matvec(
                    backend,
                    selected.as_slice(),
                    ffn_input.as_slice(),
                )?;
                perf.stage_timings.expert_projection_ns = perf
                    .stage_timings
                    .expert_projection_ns
                    .saturating_add(duration_ns(expert_projection_start));

                for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                    let mut gate = gate_up_outputs[selected_index][0].clone();
                    if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut gate,
                            bias.as_slice(),
                            expert_index,
                            layer
                                .feed_forward_gate_up_experts_weight
                                .rows_per_projection[0],
                        );
                    }

                    let mut up = gate_up_outputs[selected_index][1].clone();
                    if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut up,
                            bias.as_slice(),
                            expert_index,
                            layer
                                .feed_forward_gate_up_experts_weight
                                .rows_per_projection[1],
                        );
                    }

                    let expert_activation_start = Instant::now();
                    let activated = oai_swiglu(gate.as_slice(), up.as_slice());
                    perf.stage_timings.expert_activation_ns = perf
                        .stage_timings
                        .expert_activation_ns
                        .saturating_add(duration_ns(expert_activation_start));

                    let expert_down_start = Instant::now();
                    let mut expert = Vec::new();
                    layer.feed_forward_down_experts_weight.expert_matvec(
                        backend,
                        expert_index,
                        activated.as_slice(),
                        &mut expert,
                    )?;
                    if let Some(bias) = layer.feed_forward_down_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut expert,
                            bias.as_slice(),
                            expert_index,
                            layer.feed_forward_down_experts_weight.rows,
                        );
                    }
                    perf.stage_timings.expert_projection_ns = perf
                        .stage_timings
                        .expert_projection_ns
                        .saturating_add(duration_ns(expert_down_start));

                    let route = routing[selected_index];
                    let expert_aggregation_start = Instant::now();
                    for (dst, value) in moe_out.iter_mut().zip(expert.iter().copied()) {
                        *dst += value * route;
                    }
                    perf.stage_timings.expert_aggregation_ns = perf
                        .stage_timings
                        .expert_aggregation_ns
                        .saturating_add(duration_ns(expert_aggregation_start));
                }
            }
            hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
            kernel_count = kernel_count.saturating_add(5 + selected.len().saturating_mul(2));
        }

        let output_norm_start = Instant::now();
        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        perf.stage_timings.output_norm_ns = perf
            .stage_timings
            .output_norm_ns
            .saturating_add(duration_ns(output_norm_start));

        let (logits, selected_token) = self.select_step_logits_output(
            backend,
            final_hidden.as_slice(),
            &mut perf,
            &mut bytes_moved,
            &mut kernel_count,
            logits_output_mode,
            record_decode_logits_metrics,
        )?;

        Ok(GptOssForwardStep {
            key: cache_key,
            value: cache_value,
            logits,
            selected_token,
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }

    fn forward_step_with_device_attention_plan(
        &self,
        backend: &mut MetalBackend,
        token: TokenId,
        position: usize,
        layer_caches: &mut [MetalKvCacheMirror],
        attention_runtime: &mut MetalAttentionGraphRuntime,
        plan: &mut GptOssMetalStepPlan,
        output_mode: MetalStepOutputMode,
        record_decode_logits_metrics: bool,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        if layer_caches.len() != self.layers.len() {
            return Err(ReferenceTextGenerationError::Runtime(
                super::RuntimeError::Backend(format!(
                    "metal layer cache count mismatch: expected {}, actual {}",
                    self.layers.len(),
                    layer_caches.len()
                )),
            ));
        }

        let hidden_size = self.descriptor.config.hidden_size;
        let kv_width = self.descriptor.config.kv_width();
        let head_count = self.descriptor.config.block.attention.head_count;
        let kv_head_count = self.descriptor.config.block.attention.kv_head_count;
        let head_dim = self.descriptor.config.block.attention.head_dim;
        let rotary_dim = self.descriptor.config.block.attention.rotary_dim;
        let mut bytes_moved = 0_u64;
        let mut perf = GptOssPerformanceMetrics {
            step_count: 1,
            layer_visit_count: self.layers.len(),
            graph_node_count: self.graph_node_count(),
            graph_layer_node_count: self.graph_layer_node_count(),
            ..GptOssPerformanceMetrics::default()
        };

        let token_embedding_start = Instant::now();
        let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
        perf.stage_timings.token_embedding_ns = perf
            .stage_timings
            .token_embedding_ns
            .saturating_add(duration_ns(token_embedding_start));
        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

        let (cos_values, sin_values) =
            metal_rope_cos_sin_values(position, head_dim, rotary_dim, &self.family_metadata);
        plan.cache_key.fill(0.0);
        plan.cache_value.fill(0.0);

        let mut kernel_count = 0usize;

        let mut cos = backend
            .input_buffer(Shape::new(vec![1, head_dim / 2]), vec![0.0; head_dim / 2])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let mut sin = backend
            .input_buffer(Shape::new(vec![1, head_dim / 2]), vec![0.0; head_dim / 2])
            .map_err(ReferenceTextGenerationError::Runtime)?;
        write_metal_buffer_prefix(&mut cos, cos_values.as_slice(), &mut perf)?;
        write_metal_buffer_prefix(&mut sin, sin_values.as_slice(), &mut perf)?;

        for (layer_index, (layer, layer_cache)) in
            self.layers.iter().zip(layer_caches.iter_mut()).enumerate()
        {
            if layer_cache.width() != kv_width {
                return Err(ReferenceTextGenerationError::Runtime(
                    super::RuntimeError::Backend(format!(
                        "metal layer cache width mismatch: expected {}, actual {}",
                        kv_width,
                        layer_cache.width()
                    )),
                ));
            }

            let residual = hidden.clone();
            let attention_norm_start = Instant::now();
            let hidden_norm = rms_norm(
                hidden.as_slice(),
                layer.attention_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.attention_norm_ns = perf
                .stage_timings
                .attention_norm_ns
                .saturating_add(duration_ns(attention_norm_start));

            let qkv_start = Instant::now();
            write_metal_buffer_prefix(
                &mut plan.hidden_norm_buffer,
                hidden_norm.as_slice(),
                &mut perf,
            )?;
            let mut qkv_submission = backend.begin_submission("psionic.gpt_oss.qkv")?;
            backend.encode_quantized_matvec_submission(
                &mut qkv_submission,
                &layer.attention_query_weight.storage,
                0,
                layer.attention_query_weight.host.mode,
                layer.attention_query_weight.host.rows,
                layer.attention_query_weight.host.columns,
                &plan.hidden_norm_buffer,
                &plan.q_buffer,
            )?;
            backend.encode_quantized_matvec_submission(
                &mut qkv_submission,
                &layer.attention_key_weight.storage,
                0,
                layer.attention_key_weight.host.mode,
                layer.attention_key_weight.host.rows,
                layer.attention_key_weight.host.columns,
                &plan.hidden_norm_buffer,
                &plan.k_buffer,
            )?;
            backend.encode_quantized_matvec_submission(
                &mut qkv_submission,
                &layer.attention_value_weight.storage,
                0,
                layer.attention_value_weight.host.mode,
                layer.attention_value_weight.host.rows,
                layer.attention_value_weight.host.columns,
                &plan.hidden_norm_buffer,
                &plan.v_buffer,
            )?;
            qkv_submission.synchronize_buffer(&plan.q_buffer)?;
            qkv_submission.synchronize_buffer(&plan.k_buffer)?;
            qkv_submission.synchronize_buffer(&plan.v_buffer)?;
            let qkv_report = qkv_submission
                .commit(psionic_backend_metal::MetalCommandWait::Completed)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            accumulate_metal_submission_report(&mut perf, &qkv_report);
            kernel_count = kernel_count.saturating_add(qkv_report.encoded_operations);
            read_metal_buffer_prefix_into(
                &plan.q_buffer,
                layer.attention_query_weight.host.rows,
                &mut plan.q_values,
                &mut perf,
            )?;
            read_metal_buffer_prefix_into(
                &plan.k_buffer,
                layer.attention_key_weight.host.rows,
                &mut plan.k_values,
                &mut perf,
            )?;
            read_metal_buffer_prefix_into(
                &plan.v_buffer,
                layer.attention_value_weight.host.rows,
                &mut plan.v_values,
                &mut perf,
            )?;
            add_bias_in_place(&mut plan.q_values, layer.attention_query_bias.as_slice());
            add_bias_in_place(&mut plan.k_values, layer.attention_key_bias.as_slice());
            add_bias_in_place(&mut plan.v_values, layer.attention_value_bias.as_slice());
            perf.stage_timings.qkv_projection_ns = perf
                .stage_timings
                .qkv_projection_ns
                .saturating_add(duration_ns(qkv_start));

            let rope_start = Instant::now();
            let mut cache_layer_key = plan.k_values.clone();
            apply_rope_neox(
                &mut cache_layer_key,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                &self.family_metadata,
            );
            perf.stage_timings.rope_ns = perf
                .stage_timings
                .rope_ns
                .saturating_add(duration_ns(rope_start));

            let cache_offset = layer_index.saturating_mul(kv_width);
            plan.cache_key[cache_offset..cache_offset + kv_width]
                .copy_from_slice(cache_layer_key.as_slice());
            plan.cache_value[cache_offset..cache_offset + kv_width]
                .copy_from_slice(plan.v_values.as_slice());

            let attention_start = Instant::now();
            write_metal_buffer_prefix(&mut plan.query_buffer, plan.q_values.as_slice(), &mut perf)?;
            write_metal_buffer_prefix(&mut plan.key_buffer, plan.k_values.as_slice(), &mut perf)?;
            write_metal_buffer_prefix(&mut plan.value_buffer, plan.v_values.as_slice(), &mut perf)?;
            let attention = backend
                .decode_attention_f32_reserved(
                    attention_runtime,
                    &plan.query_buffer,
                    &plan.key_buffer,
                    &plan.value_buffer,
                    &cos,
                    &sin,
                    layer_cache,
                    1.0 / (head_dim as f32).sqrt(),
                    true,
                    false,
                    true,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
            read_metal_buffer_prefix_into(
                &attention.output,
                head_count.saturating_mul(head_dim),
                &mut plan.attention_values,
                &mut perf,
            )?;
            perf.stage_timings.attention_ns = perf
                .stage_timings
                .attention_ns
                .saturating_add(duration_ns(attention_start));

            let attention_output_start = Instant::now();
            write_metal_buffer_prefix(
                &mut plan.q_buffer,
                plan.attention_values.as_slice(),
                &mut perf,
            )?;
            let mut attention_output_submission =
                backend.begin_submission("psionic.gpt_oss.attention_output")?;
            backend.encode_quantized_matvec_submission(
                &mut attention_output_submission,
                &layer.attention_output_weight.storage,
                0,
                layer.attention_output_weight.host.mode,
                layer.attention_output_weight.host.rows,
                layer.attention_output_weight.host.columns,
                &plan.q_buffer,
                &plan.final_hidden_buffer,
            )?;
            attention_output_submission.synchronize_buffer(&plan.final_hidden_buffer)?;
            let attention_output_report = attention_output_submission
                .commit(psionic_backend_metal::MetalCommandWait::Completed)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            accumulate_metal_submission_report(&mut perf, &attention_output_report);
            kernel_count = kernel_count.saturating_add(attention_output_report.encoded_operations);
            read_metal_buffer_prefix_into(
                &plan.final_hidden_buffer,
                layer.attention_output_weight.host.rows,
                &mut plan.attention_values,
                &mut perf,
            )?;
            if let Some(bias) = layer.attention_output_bias.as_ref() {
                add_bias_in_place(&mut plan.attention_values, bias.as_slice());
            }
            perf.stage_timings.attention_output_projection_ns = perf
                .stage_timings
                .attention_output_projection_ns
                .saturating_add(duration_ns(attention_output_start));
            hidden = add_vectors(plan.attention_values.as_slice(), residual.as_slice())?;

            let ffn_residual = hidden.clone();
            let feed_forward_norm_start = Instant::now();
            let ffn_input = rms_norm(
                hidden.as_slice(),
                layer.feed_forward_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );
            perf.stage_timings.feed_forward_norm_ns = perf
                .stage_timings
                .feed_forward_norm_ns
                .saturating_add(duration_ns(feed_forward_norm_start));

            let router_start = Instant::now();
            let mut router_logits = Vec::new();
            layer
                .feed_forward_router_weight
                .matvec(ffn_input.as_slice(), &mut router_logits)?;
            if let Some(bias) = layer.feed_forward_router_bias.as_ref() {
                add_bias_in_place(&mut router_logits, bias.as_slice());
            }
            let selected = top_k_indices(
                router_logits.as_slice(),
                self.family_metadata.expert_used_count.unwrap_or(0),
            );
            let routing = softmax_selected(router_logits.as_slice(), selected.as_slice());
            perf.stage_timings.router_ns = perf
                .stage_timings
                .router_ns
                .saturating_add(duration_ns(router_start));

            let mut moe_out = vec![0.0; hidden_size];
            if !selected.is_empty() {
                let selected_ids = selected
                    .iter()
                    .copied()
                    .map(|index| {
                        i32::try_from(index).map_err(|_| {
                            ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                                format!(
                                    "expert index {index} exceeds i32 range for metal grouped dispatch",
                                ),
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let expert_projection_start = Instant::now();
                write_metal_buffer_prefix(
                    &mut plan.hidden_norm_buffer,
                    ffn_input.as_slice(),
                    &mut perf,
                )?;
                let mut gate_up_submission = backend.begin_submission("psionic.gpt_oss.gate_up")?;
                backend.encode_grouped_quantized_matvec_submission(
                    &mut gate_up_submission,
                    &layer.feed_forward_gate_up_experts_weight.storage,
                    layer.feed_forward_gate_up_experts_weight.mode,
                    layer.feed_forward_gate_up_experts_weight.row_byte_len,
                    layer.feed_forward_gate_up_experts_weight.total_rows(),
                    layer.feed_forward_gate_up_experts_weight.columns,
                    selected_ids.as_slice(),
                    &plan.hidden_norm_buffer,
                    &plan.gate_up_output_buffer,
                )?;
                gate_up_submission.synchronize_buffer(&plan.gate_up_output_buffer)?;
                let gate_up_report = gate_up_submission
                    .commit(psionic_backend_metal::MetalCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                accumulate_metal_submission_report(&mut perf, &gate_up_report);
                kernel_count = kernel_count.saturating_add(gate_up_report.encoded_operations);
                let per_selected_rows = layer.feed_forward_gate_up_experts_weight.total_rows();
                read_metal_buffer_prefix_into(
                    &plan.gate_up_output_buffer,
                    selected.len().saturating_mul(per_selected_rows),
                    &mut plan.gate_up_values,
                    &mut perf,
                )?;
                perf.stage_timings.expert_projection_ns = perf
                    .stage_timings
                    .expert_projection_ns
                    .saturating_add(duration_ns(expert_projection_start));

                let gate_rows = layer
                    .feed_forward_gate_up_experts_weight
                    .rows_per_projection[0];
                let up_rows = layer
                    .feed_forward_gate_up_experts_weight
                    .rows_per_projection[1];
                plan.expert_input_values.clear();
                for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                    let base = selected_index.saturating_mul(per_selected_rows);
                    let gate_end = base.saturating_add(gate_rows);
                    let up_end = gate_end.saturating_add(up_rows);

                    let mut gate = plan.gate_up_values[base..gate_end].to_vec();
                    if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            &mut gate,
                            bias.as_slice(),
                            expert_index,
                            gate_rows,
                        );
                    }
                    let mut up = plan.gate_up_values[gate_end..up_end].to_vec();
                    if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                        add_expert_bias_in_place(&mut up, bias.as_slice(), expert_index, up_rows);
                    }

                    let expert_activation_start = Instant::now();
                    let activated = oai_swiglu(gate.as_slice(), up.as_slice());
                    perf.stage_timings.expert_activation_ns = perf
                        .stage_timings
                        .expert_activation_ns
                        .saturating_add(duration_ns(expert_activation_start));
                    plan.expert_input_values
                        .extend_from_slice(activated.as_slice());
                }

                let expected_expert_inputs = selected
                    .len()
                    .saturating_mul(layer.feed_forward_down_experts_weight.columns);
                if plan.expert_input_values.len() != expected_expert_inputs {
                    return Err(ReferenceTextGenerationError::Runtime(
                        super::RuntimeError::Backend(format!(
                            "metal grouped expert inputs length mismatch: expected {expected_expert_inputs}, actual {}",
                            plan.expert_input_values.len()
                        )),
                    ));
                }

                let expert_down_start = Instant::now();
                write_metal_buffer_prefix(
                    &mut plan.expert_input_rows_buffer,
                    plan.expert_input_values.as_slice(),
                    &mut perf,
                )?;
                let mut expert_down_submission =
                    backend.begin_submission("psionic.gpt_oss.expert_down_ids")?;
                backend.encode_expert_matvec_f32_ids_submission(
                    &mut expert_down_submission,
                    &layer.feed_forward_down_experts_weight.storage,
                    layer.feed_forward_down_experts_weight.mode,
                    layer.feed_forward_down_experts_weight.row_byte_len,
                    layer.feed_forward_down_experts_weight.rows,
                    layer.feed_forward_down_experts_weight.columns,
                    selected_ids.as_slice(),
                    &plan.expert_input_rows_buffer,
                    &plan.expert_output_rows_buffer,
                )?;
                expert_down_submission.synchronize_buffer(&plan.expert_output_rows_buffer)?;
                let expert_down_report = expert_down_submission
                    .commit(psionic_backend_metal::MetalCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                perf.metal.grouped_expert_ids_path = true;
                accumulate_metal_submission_report(&mut perf, &expert_down_report);
                kernel_count = kernel_count.saturating_add(expert_down_report.encoded_operations);
                read_metal_buffer_prefix_into(
                    &plan.expert_output_rows_buffer,
                    selected
                        .len()
                        .saturating_mul(layer.feed_forward_down_experts_weight.rows),
                    &mut plan.expert_projected_values,
                    &mut perf,
                )?;
                perf.stage_timings.expert_projection_ns = perf
                    .stage_timings
                    .expert_projection_ns
                    .saturating_add(duration_ns(expert_down_start));

                for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                    let base =
                        selected_index.saturating_mul(layer.feed_forward_down_experts_weight.rows);
                    let end = base.saturating_add(layer.feed_forward_down_experts_weight.rows);
                    let expert_values = &mut plan.expert_projected_values[base..end];
                    if let Some(bias) = layer.feed_forward_down_experts_bias.as_ref() {
                        add_expert_bias_in_place(
                            expert_values,
                            bias.as_slice(),
                            expert_index,
                            layer.feed_forward_down_experts_weight.rows,
                        );
                    }

                    let route = routing[selected_index];
                    let expert_aggregation_start = Instant::now();
                    for (dst, value) in moe_out.iter_mut().zip(expert_values.iter().copied()) {
                        *dst += value * route;
                    }
                    perf.stage_timings.expert_aggregation_ns = perf
                        .stage_timings
                        .expert_aggregation_ns
                        .saturating_add(duration_ns(expert_aggregation_start));
                }
            }
            hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
        }

        let output_norm_start = Instant::now();
        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        perf.stage_timings.output_norm_ns = perf
            .stage_timings
            .output_norm_ns
            .saturating_add(duration_ns(output_norm_start));

        let mut logits = Vec::new();
        let mut selected_token = None;
        if let MetalStepOutputMode::Logits(logits_output_mode) = output_mode {
            let logits_projection_start = Instant::now();
            write_metal_buffer_prefix(
                &mut plan.final_hidden_buffer,
                final_hidden.as_slice(),
                &mut perf,
            )?;
            let mut logits_submission = backend.begin_submission("psionic.gpt_oss.logits")?;
            backend.encode_quantized_matvec_submission(
                &mut logits_submission,
                &self.output.storage,
                0,
                self.output.host.mode,
                self.output.host.rows,
                self.output.host.columns,
                &plan.final_hidden_buffer,
                &plan.logits_buffer,
            )?;
            logits_submission.synchronize_buffer(&plan.logits_buffer)?;
            let logits_report = logits_submission
                .commit(psionic_backend_metal::MetalCommandWait::Completed)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            accumulate_metal_submission_report(&mut perf, &logits_report);
            kernel_count = kernel_count.saturating_add(logits_report.encoded_operations);
            let selection = backend
                .select_logits_output_f32(
                    &plan.logits_buffer,
                    1,
                    self.output.host.rows,
                    logits_output_mode,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
            perf.stage_timings.logits_projection_ns = perf
                .stage_timings
                .logits_projection_ns
                .saturating_add(duration_ns(logits_projection_start));
            perf.metal.device_to_host_bytes = perf
                .metal
                .device_to_host_bytes
                .saturating_add(selection.metrics.readback_bytes);
            if record_decode_logits_metrics {
                accumulate_metal_decode_logits_metrics(
                    &mut perf,
                    selection.metrics.output_mode,
                    selection.metrics.readback_bytes,
                    selection.metrics.raw_logits_materialized,
                );
            }
            bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);

            match logits_output_mode {
                MetalLogitsOutputMode::GreedyToken => {
                    let token = selection.selected_tokens.first().copied().ok_or_else(|| {
                        ReferenceTextGenerationError::MissingOutput("metal greedy token")
                    })?;
                    selected_token = Some(TokenId(token));
                }
                MetalLogitsOutputMode::TopKCandidates(_) => {
                    let candidates = selection.candidates.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::MissingOutput("metal logits top_k candidates")
                    })?;
                    logits = expand_metal_top_k_candidates_to_logits(
                        self.descriptor.config.vocab_size,
                        candidates,
                    )?;
                }
                MetalLogitsOutputMode::RawLogits => {
                    logits =
                        selection
                            .logits
                            .ok_or(ReferenceTextGenerationError::MissingOutput(
                                "metal raw logits",
                            ))?;
                }
            }
        }

        Ok(GptOssForwardStep {
            key: plan.cache_key.clone(),
            value: plan.cache_value.clone(),
            logits,
            selected_token,
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }
}

#[derive(Clone, Debug)]
struct GptOssMetalLayer {
    attention_norm: Vec<f32>,
    attention_query_weight: MetalQuantizedMatrix,
    attention_query_bias: Vec<f32>,
    attention_key_weight: MetalQuantizedMatrix,
    attention_key_bias: Vec<f32>,
    attention_value_weight: MetalQuantizedMatrix,
    attention_value_bias: Vec<f32>,
    attention_output_weight: MetalQuantizedMatrix,
    attention_output_bias: Option<Vec<f32>>,
    attention_sinks: Option<Vec<f32>>,
    feed_forward_norm: Vec<f32>,
    feed_forward_router_weight: DenseMatrix,
    feed_forward_router_bias: Option<Vec<f32>>,
    feed_forward_gate_up_experts_weight: MetalQuantizedExpertProjectionGroup,
    feed_forward_gate_experts_bias: Option<Vec<f32>>,
    feed_forward_up_experts_bias: Option<Vec<f32>>,
    feed_forward_down_experts_weight: MetalQuantizedExpertTensor,
    feed_forward_down_experts_bias: Option<Vec<f32>>,
}

impl GptOssMetalLayer {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        layout: &GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        Ok(Self {
            attention_norm: load_dense_vector(artifact, layout.attention_norm.as_str())?,
            attention_query_weight: load_metal_quantized_matrix(
                backend,
                artifact,
                layout.attention_query_weight.as_str(),
            )?,
            attention_query_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_query_bias.as_ref(), "attention_query_bias")?,
            )?,
            attention_key_weight: load_metal_quantized_matrix(
                backend,
                artifact,
                layout.attention_key_weight.as_str(),
            )?,
            attention_key_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_key_bias.as_ref(), "attention_key_bias")?,
            )?,
            attention_value_weight: load_metal_quantized_matrix(
                backend,
                artifact,
                layout.attention_value_weight.as_str(),
            )?,
            attention_value_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_value_bias.as_ref(), "attention_value_bias")?,
            )?,
            attention_output_weight: load_metal_quantized_matrix(
                backend,
                artifact,
                layout.attention_output_weight.as_str(),
            )?,
            attention_output_bias: layout
                .attention_output_bias
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            attention_sinks: layout
                .attention_sinks_weight
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            feed_forward_norm: load_dense_vector(
                artifact,
                required_tensor_name(layout.feed_forward_norm.as_ref(), "feed_forward_norm")?,
            )?,
            feed_forward_router_weight: load_dense_matrix(
                artifact,
                required_tensor_name(
                    layout.feed_forward_router_weight.as_ref(),
                    "feed_forward_router_weight",
                )?,
            )?,
            feed_forward_router_bias: layout
                .feed_forward_router_bias
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            feed_forward_gate_up_experts_weight: load_metal_quantized_expert_projection_group(
                backend,
                artifact,
                &[
                    required_tensor_name(
                        layout.feed_forward_gate_experts_weight.as_ref(),
                        "feed_forward_gate_experts_weight",
                    )?,
                    required_tensor_name(
                        layout.feed_forward_up_experts_weight.as_ref(),
                        "feed_forward_up_experts_weight",
                    )?,
                ],
            )?,
            feed_forward_gate_experts_bias: layout
                .feed_forward_gate_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
            feed_forward_up_experts_bias: layout
                .feed_forward_up_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
            feed_forward_down_experts_weight: load_metal_quantized_expert_tensor(
                backend,
                artifact,
                required_tensor_name(
                    layout.feed_forward_down_experts_weight.as_ref(),
                    "feed_forward_down_experts_weight",
                )?,
            )?,
            feed_forward_down_experts_bias: layout
                .feed_forward_down_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
        })
    }

    fn attend(
        &self,
        layer_index: usize,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        cache: &super::InMemoryKvCache,
        descriptor: &DecoderModelDescriptor,
        sliding_window: Option<usize>,
    ) -> Vec<f32> {
        attend_impl(
            self.attention_sinks.as_deref(),
            layer_index,
            query,
            key,
            value,
            cache,
            descriptor,
            sliding_window,
        )
    }
}

#[derive(Clone, Debug)]
struct GptOssCpuModelInner {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GptOssTokenizer,
    token_embedding: QuantizedMatrix,
    output_norm: Vec<f32>,
    output: QuantizedMatrix,
    layers: Vec<GptOssLayer>,
    plan_digest: String,
    load_duration_ns: u64,
}

impl GptOssCpuModelInner {
    fn cache_width(&self) -> usize {
        self.descriptor
            .config
            .layer_count
            .saturating_mul(self.descriptor.config.kv_width())
    }

    fn forward_step(
        &self,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
    ) -> Result<GptOssForwardStep, ReferenceTextGenerationError> {
        let hidden_size = self.descriptor.config.hidden_size;
        let kv_width = self.descriptor.config.kv_width();
        let mut bytes_moved = 0_u64;
        let mut kernel_count = 0_usize;

        let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);

        let mut cache_key = vec![0.0; self.cache_width()];
        let mut cache_value = vec![0.0; self.cache_width()];

        for (layer_index, layer) in self.layers.iter().enumerate() {
            let residual = hidden.clone();
            let hidden_norm = rms_norm(
                hidden.as_slice(),
                layer.attention_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );

            let mut q = Vec::new();
            layer
                .attention_query_weight
                .matvec(hidden_norm.as_slice(), &mut q)?;
            add_bias_in_place(&mut q, layer.attention_query_bias.as_slice());

            let mut k = Vec::new();
            layer
                .attention_key_weight
                .matvec(hidden_norm.as_slice(), &mut k)?;
            add_bias_in_place(&mut k, layer.attention_key_bias.as_slice());

            let mut v = Vec::new();
            layer
                .attention_value_weight
                .matvec(hidden_norm.as_slice(), &mut v)?;
            add_bias_in_place(&mut v, layer.attention_value_bias.as_slice());

            apply_rope_neox(
                &mut q,
                self.descriptor.config.block.attention.head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                position,
                &self.family_metadata,
            );
            apply_rope_neox(
                &mut k,
                self.descriptor.config.block.attention.kv_head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                position,
                &self.family_metadata,
            );

            let cache_offset = layer_index.saturating_mul(kv_width);
            cache_key[cache_offset..cache_offset + kv_width].copy_from_slice(k.as_slice());
            cache_value[cache_offset..cache_offset + kv_width].copy_from_slice(v.as_slice());

            let attention = layer.attend(
                layer_index,
                q.as_slice(),
                k.as_slice(),
                v.as_slice(),
                cache,
                &self.descriptor,
                self.family_metadata.sliding_window,
            );

            let mut attention_out = Vec::new();
            layer
                .attention_output_weight
                .matvec(attention.as_slice(), &mut attention_out)?;
            if let Some(bias) = layer.attention_output_bias.as_ref() {
                add_bias_in_place(&mut attention_out, bias.as_slice());
            }
            hidden = add_vectors(attention_out.as_slice(), residual.as_slice())?;

            let ffn_residual = hidden.clone();
            let ffn_input = rms_norm(
                hidden.as_slice(),
                layer.feed_forward_norm.as_slice(),
                self.family_metadata.rms_norm_epsilon,
            );

            let mut router_logits = Vec::new();
            layer
                .feed_forward_router_weight
                .matvec(ffn_input.as_slice(), &mut router_logits)?;
            if let Some(bias) = layer.feed_forward_router_bias.as_ref() {
                add_bias_in_place(&mut router_logits, bias.as_slice());
            }
            let selected = top_k_indices(
                router_logits.as_slice(),
                self.family_metadata.expert_used_count.unwrap_or(0),
            );
            let routing = softmax_selected(router_logits.as_slice(), selected.as_slice());

            let mut moe_out = vec![0.0; hidden_size];
            for (selected_index, expert_index) in selected.iter().copied().enumerate() {
                let mut gate = Vec::new();
                layer.feed_forward_gate_experts_weight.expert_matvec(
                    expert_index,
                    ffn_input.as_slice(),
                    &mut gate,
                )?;
                if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                    add_expert_bias_in_place(
                        &mut gate,
                        bias.as_slice(),
                        expert_index,
                        layer.feed_forward_gate_experts_weight.rows,
                    );
                }

                let mut up = Vec::new();
                layer.feed_forward_up_experts_weight.expert_matvec(
                    expert_index,
                    ffn_input.as_slice(),
                    &mut up,
                )?;
                if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                    add_expert_bias_in_place(
                        &mut up,
                        bias.as_slice(),
                        expert_index,
                        layer.feed_forward_up_experts_weight.rows,
                    );
                }

                let activated = oai_swiglu(gate.as_slice(), up.as_slice());
                let mut expert = Vec::new();
                layer.feed_forward_down_experts_weight.expert_matvec(
                    expert_index,
                    activated.as_slice(),
                    &mut expert,
                )?;
                if let Some(bias) = layer.feed_forward_down_experts_bias.as_ref() {
                    add_expert_bias_in_place(
                        &mut expert,
                        bias.as_slice(),
                        expert_index,
                        layer.feed_forward_down_experts_weight.rows,
                    );
                }

                let route = routing[selected_index];
                for (dst, value) in moe_out.iter_mut().zip(expert.iter().copied()) {
                    *dst += value * route;
                }
            }
            hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
            kernel_count = kernel_count.saturating_add(7 + selected.len().saturating_mul(3));
        }

        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        let mut logits = Vec::new();
        self.output.matvec(final_hidden.as_slice(), &mut logits)?;
        bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
        kernel_count = kernel_count.saturating_add(1);

        Ok(GptOssForwardStep {
            key: cache_key,
            value: cache_value,
            logits,
            selected_token: None,
            kernel_count,
            bytes_moved,
            perf: None,
        })
    }
}

#[derive(Clone, Debug)]
struct GptOssLayer {
    attention_norm: Vec<f32>,
    attention_query_weight: QuantizedMatrix,
    attention_query_bias: Vec<f32>,
    attention_key_weight: QuantizedMatrix,
    attention_key_bias: Vec<f32>,
    attention_value_weight: QuantizedMatrix,
    attention_value_bias: Vec<f32>,
    attention_output_weight: QuantizedMatrix,
    attention_output_bias: Option<Vec<f32>>,
    attention_sinks: Option<Vec<f32>>,
    feed_forward_norm: Vec<f32>,
    feed_forward_router_weight: DenseMatrix,
    feed_forward_router_bias: Option<Vec<f32>>,
    feed_forward_gate_experts_weight: QuantizedExpertTensor,
    feed_forward_gate_experts_bias: Option<Vec<f32>>,
    feed_forward_up_experts_weight: QuantizedExpertTensor,
    feed_forward_up_experts_bias: Option<Vec<f32>>,
    feed_forward_down_experts_weight: QuantizedExpertTensor,
    feed_forward_down_experts_bias: Option<Vec<f32>>,
}

impl GptOssLayer {
    fn load(
        artifact: &GgufBlobArtifact,
        layout: &GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        Ok(Self {
            attention_norm: load_dense_vector(artifact, layout.attention_norm.as_str())?,
            attention_query_weight: load_quantized_matrix(
                artifact,
                layout.attention_query_weight.as_str(),
            )?,
            attention_query_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_query_bias.as_ref(), "attention_query_bias")?,
            )?,
            attention_key_weight: load_quantized_matrix(
                artifact,
                layout.attention_key_weight.as_str(),
            )?,
            attention_key_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_key_bias.as_ref(), "attention_key_bias")?,
            )?,
            attention_value_weight: load_quantized_matrix(
                artifact,
                layout.attention_value_weight.as_str(),
            )?,
            attention_value_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_value_bias.as_ref(), "attention_value_bias")?,
            )?,
            attention_output_weight: load_quantized_matrix(
                artifact,
                layout.attention_output_weight.as_str(),
            )?,
            attention_output_bias: layout
                .attention_output_bias
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            attention_sinks: layout
                .attention_sinks_weight
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            feed_forward_norm: load_dense_vector(
                artifact,
                required_tensor_name(layout.feed_forward_norm.as_ref(), "feed_forward_norm")?,
            )?,
            feed_forward_router_weight: load_dense_matrix(
                artifact,
                required_tensor_name(
                    layout.feed_forward_router_weight.as_ref(),
                    "feed_forward_router_weight",
                )?,
            )?,
            feed_forward_router_bias: layout
                .feed_forward_router_bias
                .as_ref()
                .map(|name| load_dense_vector(artifact, name.as_str()))
                .transpose()?,
            feed_forward_gate_experts_weight: load_quantized_expert_tensor(
                artifact,
                required_tensor_name(
                    layout.feed_forward_gate_experts_weight.as_ref(),
                    "feed_forward_gate_experts_weight",
                )?,
            )?,
            feed_forward_gate_experts_bias: layout
                .feed_forward_gate_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
            feed_forward_up_experts_weight: load_quantized_expert_tensor(
                artifact,
                required_tensor_name(
                    layout.feed_forward_up_experts_weight.as_ref(),
                    "feed_forward_up_experts_weight",
                )?,
            )?,
            feed_forward_up_experts_bias: layout
                .feed_forward_up_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
            feed_forward_down_experts_weight: load_quantized_expert_tensor(
                artifact,
                required_tensor_name(
                    layout.feed_forward_down_experts_weight.as_ref(),
                    "feed_forward_down_experts_weight",
                )?,
            )?,
            feed_forward_down_experts_bias: layout
                .feed_forward_down_experts_bias
                .as_ref()
                .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
                .transpose()?,
        })
    }

    fn attend(
        &self,
        layer_index: usize,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        cache: &super::InMemoryKvCache,
        descriptor: &DecoderModelDescriptor,
        sliding_window: Option<usize>,
    ) -> Vec<f32> {
        attend_impl(
            self.attention_sinks.as_deref(),
            layer_index,
            query,
            key,
            value,
            cache,
            descriptor,
            sliding_window,
        )
    }
}

fn attend_impl(
    attention_sinks: Option<&[f32]>,
    layer_index: usize,
    query: &[f32],
    key: &[f32],
    value: &[f32],
    cache: &super::InMemoryKvCache,
    descriptor: &DecoderModelDescriptor,
    sliding_window: Option<usize>,
) -> Vec<f32> {
    let head_count = descriptor.config.block.attention.head_count;
    let kv_head_count = descriptor.config.block.attention.kv_head_count;
    let head_dim = descriptor.config.block.attention.head_dim;
    let kv_width = descriptor.config.kv_width();
    let layer_offset = layer_index.saturating_mul(kv_width);
    let group_size = head_count / kv_head_count.max(1);
    let scale = 1.0 / (head_dim as f32).sqrt();

    let cached_entries = if layer_index % 2 == 0 {
        if let Some(window) = sliding_window {
            let keep = window.saturating_sub(1);
            let start = cache.len().saturating_sub(keep);
            &cache.entries()[start..]
        } else {
            cache.entries()
        }
    } else {
        cache.entries()
    };

    let mut output = vec![0.0; query.len()];
    for head_index in 0..head_count {
        let kv_head = if group_size == 0 {
            0
        } else {
            head_index / group_size
        }
        .min(kv_head_count.saturating_sub(1));
        let query_slice = &query[head_index * head_dim..(head_index + 1) * head_dim];
        let key_offset = kv_head * head_dim;
        let value_offset = kv_head * head_dim;

        let mut logits = Vec::with_capacity(cached_entries.len().saturating_add(1));
        for entry in cached_entries {
            let cached_key =
                &entry.key[layer_offset + key_offset..layer_offset + key_offset + head_dim];
            logits.push(dot(query_slice, cached_key) * scale);
        }
        logits.push(dot(query_slice, &key[key_offset..key_offset + head_dim]) * scale);

        let mut max_value = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if let Some(sinks) = attention_sinks {
            max_value = max_value.max(sinks[head_index]);
        }
        let mut weights = logits
            .iter()
            .copied()
            .map(|logit| (logit - max_value).exp())
            .collect::<Vec<_>>();
        let mut denom = weights.iter().copied().sum::<f32>();
        if let Some(sinks) = attention_sinks {
            denom += (sinks[head_index] - max_value).exp();
        }
        if denom != 0.0 {
            for weight in &mut weights {
                *weight /= denom;
            }
        }

        let destination = &mut output[head_index * head_dim..(head_index + 1) * head_dim];
        for (entry_index, entry) in cached_entries.iter().enumerate() {
            let cached_value =
                &entry.value[layer_offset + value_offset..layer_offset + value_offset + head_dim];
            axpy(destination, cached_value, weights[entry_index]);
        }
        axpy(
            destination,
            &value[value_offset..value_offset + head_dim],
            *weights.last().unwrap_or(&0.0),
        );
    }
    output
}

#[derive(Clone, Debug)]
struct DenseMatrix {
    rows: usize,
    columns: usize,
    values: Vec<f32>,
}

impl DenseMatrix {
    fn matvec(&self, input: &[f32], output: &mut Vec<f32>) -> Result<(), super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "dense matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        output.clear();
        output.resize(self.rows, 0.0);
        for (row_index, row) in self.values.chunks_exact(self.columns).enumerate() {
            output[row_index] = dot(row, input);
        }
        Ok(())
    }
}

fn transpose_dense_matrix_f32(matrix: &DenseMatrix) -> Vec<f32> {
    let mut transposed = vec![0.0_f32; matrix.rows.saturating_mul(matrix.columns)];
    for row_index in 0..matrix.rows {
        let row_start = row_index.saturating_mul(matrix.columns);
        let row = &matrix.values[row_start..row_start + matrix.columns];
        for (column_index, value) in row.iter().copied().enumerate() {
            transposed[column_index.saturating_mul(matrix.rows) + row_index] = value;
        }
    }
    transposed
}

#[derive(Clone, Debug)]
struct QuantizedMatrix {
    storage: PagedTensorStorage,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl QuantizedMatrix {
    fn byte_length(&self) -> usize {
        self.storage.byte_length()
    }

    fn decode_row(&self, row_index: usize) -> Result<Vec<f32>, super::RuntimeError> {
        if row_index >= self.rows {
            return Err(super::RuntimeError::Backend(format!(
                "quantized row index {row_index} exceeds row count {}",
                self.rows
            )));
        }
        let offset = row_index.saturating_mul(self.row_byte_len);
        let bytes = self
            .storage
            .read_range(offset, self.row_byte_len)
            .map_err(model_load_runtime_error)?;
        let mut output = Vec::new();
        decode_quantized_row_into(self.mode, bytes, &mut output)?;
        Ok(output)
    }

    fn matvec(&self, input: &[f32], output: &mut Vec<f32>) -> Result<(), super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "quantized matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        output.clear();
        output.resize(self.rows, 0.0);
        for row_index in 0..self.rows {
            let offset = row_index.saturating_mul(self.row_byte_len);
            let bytes = self
                .storage
                .read_range(offset, self.row_byte_len)
                .map_err(model_load_runtime_error)?;
            output[row_index] = quantized_row_dot(input, self.mode, bytes)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct MetalQuantizedMatrix {
    host: QuantizedMatrix,
    storage: MetalBuffer,
}

impl MetalQuantizedMatrix {
    fn byte_length(&self) -> usize {
        self.storage.byte_len()
    }

    fn matvec(
        &self,
        backend: &mut MetalBackend,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), super::RuntimeError> {
        *output = backend.quantized_matvec(
            &self.storage,
            self.host.mode,
            self.host.rows,
            self.host.columns,
            input,
        )?;
        Ok(())
    }

    fn select_logits_output(
        &self,
        backend: &mut MetalBackend,
        input: &[f32],
        output_mode: MetalLogitsOutputMode,
    ) -> Result<psionic_backend_metal::MetalLogitsSelectionResult, super::RuntimeError> {
        backend.quantized_matvec_select_logits_output(
            &self.storage,
            0,
            self.host.mode,
            self.host.rows,
            self.host.columns,
            input,
            output_mode,
        )
    }
}

#[derive(Clone, Debug)]
struct CudaQuantizedMatrix {
    storage: Option<CudaBuffer>,
    host: QuantizedMatrix,
    transposed_f16: Option<CudaBuffer>,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl CudaQuantizedMatrix {
    fn byte_length(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .unwrap_or_else(|| self.host.byte_length())
    }

    fn device_residency_bytes(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .unwrap_or(0)
            .saturating_add(
                self.transposed_f16
                    .as_ref()
                    .map(CudaBuffer::byte_len)
                    .unwrap_or(0),
            )
    }

    fn is_device_resident(&self) -> bool {
        self.storage.is_some()
    }

    fn device_storage(&self) -> Result<&CudaBuffer, super::RuntimeError> {
        self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda quantized matrix requires device storage for this execution path",
            ))
        })
    }

    fn matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<CudaQuantizedMatvecStats, super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "cuda quantized matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if self.storage.is_none() {
            self.host.matvec(input, output)?;
            return Ok(zero_cuda_matvec_stats());
        }
        let result = backend.quantized_matvec_profiled(
            self.device_storage()?,
            self.mode,
            self.rows,
            self.columns,
            input,
        )?;
        *output = result.values;
        Ok(result.stats)
    }
}

#[derive(Clone, Debug)]
struct QuantizedProjectionGroup {
    columns: usize,
    projections: Vec<QuantizedMatrix>,
}

impl QuantizedProjectionGroup {
    fn byte_length(&self) -> usize {
        self.projections
            .iter()
            .map(QuantizedMatrix::byte_length)
            .fold(0usize, usize::saturating_add)
    }

    fn matvec(&self, input: &[f32]) -> Result<Vec<Vec<f32>>, super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "packed matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let mut outputs = Vec::with_capacity(self.projections.len());
        for projection in &self.projections {
            let mut output = Vec::new();
            projection.matvec(input, &mut output)?;
            outputs.push(output);
        }
        Ok(outputs)
    }
}

#[derive(Clone, Debug)]
struct MetalQuantizedExpertTensor {
    storage: MetalBuffer,
    mode: QuantizationMode,
    outer: usize,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl MetalQuantizedExpertTensor {
    fn byte_length(&self) -> usize {
        self.storage.byte_len()
    }

    fn expert_matvec(
        &self,
        backend: &mut MetalBackend,
        expert_index: usize,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {expert_index} exceeds expert count {}",
                self.outer
            )));
        }
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "metal expert matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let byte_offset = expert_index
            .saturating_mul(self.rows)
            .saturating_mul(self.row_byte_len);
        *output = backend
            .quantized_matvec_with_offset(
                &self.storage,
                byte_offset,
                self.mode,
                self.rows,
                self.columns,
                input,
            )?
            .values;
        Ok(())
    }
}

fn split_projection_outputs(
    rows_per_projection: &[usize],
    values: Vec<f32>,
) -> Result<Vec<Vec<f32>>, super::RuntimeError> {
    let expected = rows_per_projection
        .iter()
        .copied()
        .fold(0usize, usize::saturating_add);
    if values.len() != expected {
        return Err(super::RuntimeError::Backend(format!(
            "packed cuda projection output mismatch: expected {expected} values, actual {}",
            values.len()
        )));
    }
    let mut offset = 0usize;
    let mut outputs = Vec::with_capacity(rows_per_projection.len());
    for rows in rows_per_projection {
        let end = offset.saturating_add(*rows);
        outputs.push(values[offset..end].to_vec());
        offset = end;
    }
    Ok(outputs)
}

#[derive(Clone, Debug)]
struct CudaQuantizedProjectionGroup {
    storage: Option<CudaBuffer>,
    host: Option<QuantizedProjectionGroup>,
    transposed_f16: Option<CudaBuffer>,
    mode: QuantizationMode,
    rows_per_projection: Vec<usize>,
    columns: usize,
}

impl CudaQuantizedProjectionGroup {
    fn total_rows(&self) -> usize {
        self.rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add)
    }

    fn byte_length(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .or_else(|| {
                self.host
                    .as_ref()
                    .map(QuantizedProjectionGroup::byte_length)
            })
            .unwrap_or(0)
    }

    fn device_residency_bytes(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .unwrap_or(0)
            .saturating_add(
                self.transposed_f16
                    .as_ref()
                    .map(CudaBuffer::byte_len)
                    .unwrap_or(0),
            )
    }

    fn is_device_resident(&self) -> bool {
        self.storage.is_some()
    }

    fn device_storage(&self) -> Result<&CudaBuffer, super::RuntimeError> {
        self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda packed projection requires device storage for this execution path",
            ))
        })
    }

    fn matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        input: &[f32],
    ) -> Result<(Vec<Vec<f32>>, CudaQuantizedMatvecStats), super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "cuda packed matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if let Some(host) = self.host.as_ref() {
            return Ok((host.matvec(input)?, zero_cuda_matvec_stats()));
        }
        let result = backend.quantized_matvec_profiled(
            self.device_storage()?,
            self.mode,
            self.total_rows(),
            self.columns,
            input,
        )?;
        Ok((
            split_projection_outputs(&self.rows_per_projection, result.values)?,
            result.stats,
        ))
    }
}

fn pack_quantized_projection_bytes(projections: &[&[u8]]) -> Vec<u8> {
    let total_bytes = projections
        .iter()
        .copied()
        .fold(0usize, |sum, bytes| sum.saturating_add(bytes.len()));
    let mut packed = Vec::with_capacity(total_bytes);
    for projection in projections {
        packed.extend_from_slice(projection);
    }
    packed
}

#[derive(Clone, Debug)]
struct QuantizedExpertTensor {
    storage: PagedTensorStorage,
    mode: QuantizationMode,
    outer: usize,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl QuantizedExpertTensor {
    fn byte_length(&self) -> usize {
        self.storage.byte_length()
    }

    fn expert_bytes(&self, expert_index: usize) -> Result<&[u8], super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {} exceeds expert count {}",
                expert_index, self.outer
            )));
        }
        let expert_stride = self.rows.saturating_mul(self.row_byte_len);
        let offset = expert_index.saturating_mul(expert_stride);
        self.storage
            .read_range(offset, expert_stride)
            .map_err(model_load_runtime_error)
    }

    fn selected_bytes_into(
        &self,
        selected: &[usize],
        packed: &mut Vec<u8>,
    ) -> Result<(), super::RuntimeError> {
        let expert_stride = self.rows.saturating_mul(self.row_byte_len);
        packed.clear();
        packed.reserve(selected.len().saturating_mul(expert_stride));
        for expert_index in selected {
            if *expert_index >= self.outer {
                return Err(super::RuntimeError::Backend(format!(
                    "expert index {} exceeds expert count {}",
                    expert_index, self.outer
                )));
            }
            let offset = expert_index.saturating_mul(expert_stride);
            let bytes = self
                .storage
                .read_range(offset, expert_stride)
                .map_err(model_load_runtime_error)?;
            packed.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn selected_bytes(&self, selected: &[usize]) -> Result<Vec<u8>, super::RuntimeError> {
        let mut packed = Vec::new();
        self.selected_bytes_into(selected, &mut packed)?;
        Ok(packed)
    }

    fn expert_matvec(
        &self,
        expert_index: usize,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {expert_index} exceeds expert count {}",
                self.outer
            )));
        }
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "expert matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        output.clear();
        output.resize(self.rows, 0.0);
        for row_index in 0..self.rows {
            let offset = (expert_index
                .saturating_mul(self.rows)
                .saturating_add(row_index))
            .saturating_mul(self.row_byte_len);
            let bytes = self
                .storage
                .read_range(offset, self.row_byte_len)
                .map_err(model_load_runtime_error)?;
            output[row_index] = quantized_row_dot(input, self.mode, bytes)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct QuantizedExpertProjectionGroup {
    outer: usize,
    columns: usize,
    gate: QuantizedExpertTensor,
    up: QuantizedExpertTensor,
}

impl QuantizedExpertProjectionGroup {
    fn byte_length(&self) -> usize {
        self.gate
            .byte_length()
            .saturating_add(self.up.byte_length())
    }

    fn expert_projection_bytes(
        &self,
        expert_index: usize,
    ) -> Result<(&[u8], &[u8]), super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {} exceeds expert count {}",
                expert_index, self.outer
            )));
        }
        Ok((
            self.gate.expert_bytes(expert_index)?,
            self.up.expert_bytes(expert_index)?,
        ))
    }

    fn selected_packed_bytes_into(
        &self,
        selected: &[usize],
        packed: &mut Vec<u8>,
    ) -> Result<(), super::RuntimeError> {
        let gate_stride = self.gate.rows.saturating_mul(self.gate.row_byte_len);
        let up_stride = self.up.rows.saturating_mul(self.up.row_byte_len);
        packed.clear();
        packed.reserve(
            selected
                .len()
                .saturating_mul(gate_stride.saturating_add(up_stride)),
        );
        for expert_index in selected {
            if *expert_index >= self.outer {
                return Err(super::RuntimeError::Backend(format!(
                    "expert index {} exceeds expert count {}",
                    expert_index, self.outer
                )));
            }
            let gate_offset = expert_index.saturating_mul(gate_stride);
            let gate_bytes = self
                .gate
                .storage
                .read_range(gate_offset, gate_stride)
                .map_err(model_load_runtime_error)?;
            packed.extend_from_slice(gate_bytes);
            let up_offset = expert_index.saturating_mul(up_stride);
            let up_bytes = self
                .up
                .storage
                .read_range(up_offset, up_stride)
                .map_err(model_load_runtime_error)?;
            packed.extend_from_slice(up_bytes);
        }
        Ok(())
    }

    fn selected_packed_bytes(&self, selected: &[usize]) -> Result<Vec<u8>, super::RuntimeError> {
        let mut packed = Vec::new();
        self.selected_packed_bytes_into(selected, &mut packed)?;
        Ok(packed)
    }

    fn expert_matvec(
        &self,
        expert_index: usize,
        input: &[f32],
    ) -> Result<Vec<Vec<f32>>, super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {expert_index} exceeds expert count {}",
                self.outer
            )));
        }
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "expert packed matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let mut gate = Vec::new();
        self.gate.expert_matvec(expert_index, input, &mut gate)?;
        let mut up = Vec::new();
        self.up.expert_matvec(expert_index, input, &mut up)?;
        Ok(vec![gate, up])
    }
}

#[derive(Clone, Debug)]
struct CudaQuantizedExpertTensor {
    storage: Option<CudaBuffer>,
    host: Option<QuantizedExpertTensor>,
    mode: QuantizationMode,
    outer: usize,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl CudaQuantizedExpertTensor {
    fn byte_length(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .or_else(|| self.host.as_ref().map(QuantizedExpertTensor::byte_length))
            .unwrap_or(0)
    }

    fn device_residency_bytes(&self) -> usize {
        self.storage.as_ref().map(CudaBuffer::byte_len).unwrap_or(0)
    }

    fn is_device_resident(&self) -> bool {
        self.storage.is_some()
    }

    fn device_storage(&self) -> Result<&CudaBuffer, super::RuntimeError> {
        self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda expert tensor requires device storage for this execution path",
            ))
        })
    }

    fn expert_matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        expert_index: usize,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<CudaQuantizedMatvecStats, super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {expert_index} exceeds expert count {}",
                self.outer
            )));
        }
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "cuda expert matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if let Some(host) = self.host.as_ref() {
            host.expert_matvec(expert_index, input, output)?;
            return Ok(zero_cuda_matvec_stats());
        }
        let storage = self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda expert tensor is missing both device and host storage",
            ))
        })?;
        let byte_offset = expert_index
            .saturating_mul(self.rows)
            .saturating_mul(self.row_byte_len);
        let result = backend.quantized_matvec_with_offset_profiled(
            storage,
            byte_offset,
            self.mode,
            self.rows,
            self.columns,
            input,
        )?;
        *output = result.values;
        Ok(result.stats)
    }
}

fn pack_quantized_expert_projection_bytes(
    row_byte_len: usize,
    outer: usize,
    rows_per_projection: &[usize],
    projections: &[&[u8]],
) -> Vec<u8> {
    let rows_per_expert = rows_per_projection
        .iter()
        .copied()
        .fold(0usize, usize::saturating_add);
    let total_bytes = outer
        .saturating_mul(rows_per_expert)
        .saturating_mul(row_byte_len);
    let mut packed = Vec::with_capacity(total_bytes);
    for expert_index in 0..outer {
        for (projection_rows, projection_bytes) in rows_per_projection
            .iter()
            .copied()
            .zip(projections.iter().copied())
        {
            let expert_stride = projection_rows.saturating_mul(row_byte_len);
            let start = expert_index.saturating_mul(expert_stride);
            let end = start.saturating_add(expert_stride);
            packed.extend_from_slice(&projection_bytes[start..end]);
        }
    }
    packed
}

#[derive(Clone, Debug)]
struct CudaQuantizedExpertProjectionGroup {
    storage: Option<CudaBuffer>,
    host: Option<QuantizedExpertProjectionGroup>,
    mode: QuantizationMode,
    outer: usize,
    rows_per_projection: Vec<usize>,
    columns: usize,
    row_byte_len: usize,
}

impl CudaQuantizedExpertProjectionGroup {
    fn total_rows(&self) -> usize {
        self.rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add)
    }

    fn byte_length(&self) -> usize {
        self.storage
            .as_ref()
            .map(CudaBuffer::byte_len)
            .or_else(|| {
                self.host
                    .as_ref()
                    .map(QuantizedExpertProjectionGroup::byte_length)
            })
            .unwrap_or(0)
    }

    fn device_residency_bytes(&self) -> usize {
        self.storage.as_ref().map(CudaBuffer::byte_len).unwrap_or(0)
    }

    fn is_device_resident(&self) -> bool {
        self.storage.is_some()
    }

    fn device_storage(&self) -> Result<&CudaBuffer, super::RuntimeError> {
        self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda expert projection group requires device storage for this execution path",
            ))
        })
    }

    fn expert_matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        expert_index: usize,
        input: &[f32],
    ) -> Result<(Vec<Vec<f32>>, CudaQuantizedMatvecStats), super::RuntimeError> {
        if expert_index >= self.outer {
            return Err(super::RuntimeError::Backend(format!(
                "expert index {expert_index} exceeds expert count {}",
                self.outer
            )));
        }
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "cuda expert packed matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if let Some(host) = self.host.as_ref() {
            return Ok((
                host.expert_matvec(expert_index, input)?,
                zero_cuda_matvec_stats(),
            ));
        }
        let storage = self.storage.as_ref().ok_or_else(|| {
            super::RuntimeError::Backend(String::from(
                "cuda expert projection group is missing both device and host storage",
            ))
        })?;
        let byte_offset = expert_index
            .saturating_mul(self.total_rows())
            .saturating_mul(self.row_byte_len);
        let result = backend.quantized_matvec_with_offset_profiled(
            storage,
            byte_offset,
            self.mode,
            self.total_rows(),
            self.columns,
            input,
        )?;
        Ok((
            split_projection_outputs(&self.rows_per_projection, result.values)?,
            result.stats,
        ))
    }
}

#[derive(Clone, Debug)]
struct MetalQuantizedExpertProjectionGroup {
    storage: MetalBuffer,
    mode: QuantizationMode,
    outer: usize,
    rows_per_projection: Vec<usize>,
    columns: usize,
    row_byte_len: usize,
}

impl MetalQuantizedExpertProjectionGroup {
    fn total_rows(&self) -> usize {
        self.rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add)
    }

    fn byte_length(&self) -> usize {
        self.storage.byte_len()
    }

    fn selected_matvec(
        &self,
        backend: &mut MetalBackend,
        selected: &[usize],
        input: &[f32],
    ) -> Result<Vec<Vec<Vec<f32>>>, super::RuntimeError> {
        if input.len() != self.columns {
            return Err(super::RuntimeError::Backend(format!(
                "metal expert packed matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if selected.is_empty() {
            return Ok(Vec::new());
        }
        let selected_ids = selected
            .iter()
            .copied()
            .map(|index| {
                if index >= self.outer {
                    return Err(super::RuntimeError::Backend(format!(
                        "expert index {index} exceeds expert count {}",
                        self.outer
                    )));
                }
                i32::try_from(index).map_err(|_| {
                    super::RuntimeError::Backend(format!(
                        "expert index {index} exceeds i32 range for metal grouped dispatch",
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let input_buffer = backend.input_buffer(Shape::new(vec![self.columns]), input.to_vec())?;
        let result: MetalGroupedExpertMatvecResult = backend.mul_mv_id(
            &self.storage,
            self.mode,
            self.row_byte_len,
            self.total_rows(),
            self.columns,
            selected_ids.as_slice(),
            &input_buffer,
        )?;
        let per_selected = result
            .values
            .chunks_exact(self.total_rows())
            .map(|values| split_projection_outputs(&self.rows_per_projection, values.to_vec()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(per_selected)
    }
}

#[derive(Clone, Debug)]
struct CudaKvCacheMirror {
    key_buffer: CudaBuffer,
    value_buffer: CudaBuffer,
    width: usize,
    len: usize,
    capacity_tokens: usize,
}

impl CudaKvCacheMirror {
    fn capacity_for_request(
        current_tokens: usize,
        reserve_tokens: usize,
        max_context: usize,
    ) -> usize {
        let requested = current_tokens
            .saturating_add(reserve_tokens)
            .max(64)
            .min(max_context.max(1));
        requested
            .checked_next_power_of_two()
            .unwrap_or(max_context.max(1))
            .min(max_context.max(1))
    }

    fn from_host_cache(
        backend: &mut CudaBackend,
        cache: &super::InMemoryKvCache,
        reserve_tokens: usize,
    ) -> Result<Self, super::RuntimeError> {
        let capacity_tokens =
            Self::capacity_for_request(cache.len(), reserve_tokens, cache.max_context());
        let mut key_buffer =
            backend.byte_buffer(&vec![
                0_u8;
                capacity_tokens
                    .saturating_mul(cache.width())
                    .saturating_mul(std::mem::size_of::<u16>())
            ])?;
        let mut value_buffer =
            backend.byte_buffer(&vec![
                0_u8;
                capacity_tokens
                    .saturating_mul(cache.width())
                    .saturating_mul(std::mem::size_of::<u16>())
            ])?;
        if !cache.is_empty() {
            let mut keys = Vec::with_capacity(cache.len().saturating_mul(cache.width()));
            let mut values = Vec::with_capacity(cache.len().saturating_mul(cache.width()));
            for entry in cache.entries() {
                keys.extend_from_slice(entry.key.as_slice());
                values.extend_from_slice(entry.value.as_slice());
            }
            key_buffer
                .write_bytes_at_offset(0, f32_slice_to_f16_bytes(keys.as_slice()).as_slice())?;
            value_buffer
                .write_bytes_at_offset(0, f32_slice_to_f16_bytes(values.as_slice()).as_slice())?;
        }
        Ok(Self {
            key_buffer,
            value_buffer,
            width: cache.width(),
            len: cache.len(),
            capacity_tokens,
        })
    }

    fn ensure_capacity(
        &mut self,
        backend: &mut CudaBackend,
        required_tokens: usize,
    ) -> Result<(), super::RuntimeError> {
        if required_tokens <= self.capacity_tokens {
            return Ok(());
        }
        let new_capacity = required_tokens
            .max(self.capacity_tokens.saturating_mul(2))
            .checked_next_power_of_two()
            .unwrap_or(required_tokens);
        let mut new_keys =
            backend.byte_buffer(&vec![
                0_u8;
                new_capacity
                    .saturating_mul(self.width)
                    .saturating_mul(std::mem::size_of::<u16>())
            ])?;
        let mut new_values =
            backend.byte_buffer(&vec![
                0_u8;
                new_capacity
                    .saturating_mul(self.width)
                    .saturating_mul(std::mem::size_of::<u16>())
            ])?;
        if self.len > 0 {
            let existing_keys = self.key_buffer.read_bytes_at_offset(
                0,
                self.len
                    .saturating_mul(self.width)
                    .saturating_mul(std::mem::size_of::<u16>()),
            )?;
            let existing_values = self.value_buffer.read_bytes_at_offset(
                0,
                self.len
                    .saturating_mul(self.width)
                    .saturating_mul(std::mem::size_of::<u16>()),
            )?;
            new_keys.write_bytes_at_offset(0, existing_keys.as_slice())?;
            new_values.write_bytes_at_offset(0, existing_values.as_slice())?;
        }
        self.key_buffer = new_keys;
        self.value_buffer = new_values;
        self.capacity_tokens = new_capacity;
        Ok(())
    }

    fn detached_with_reserve(
        &self,
        backend: &mut CudaBackend,
        reserve_tokens: usize,
        max_context: usize,
    ) -> Result<Self, super::RuntimeError> {
        let capacity_tokens = Self::capacity_for_request(self.len, reserve_tokens, max_context);
        let key_bytes = capacity_tokens
            .saturating_mul(self.width)
            .saturating_mul(std::mem::size_of::<u16>());
        let value_bytes = key_bytes;
        let key_buffer = backend.byte_buffer(&vec![0_u8; key_bytes])?;
        let value_buffer = backend.byte_buffer(&vec![0_u8; value_bytes])?;
        if self.len > 0 {
            let copy_bytes = self
                .len
                .saturating_mul(self.width)
                .saturating_mul(std::mem::size_of::<u16>());
            let mut submission = backend.begin_submission()?;
            submission.copy_buffer_region(&self.key_buffer, 0, &key_buffer, 0, copy_bytes)?;
            submission.copy_buffer_region(&self.value_buffer, 0, &value_buffer, 0, copy_bytes)?;
            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        }
        Ok(Self {
            key_buffer,
            value_buffer,
            width: self.width,
            len: self.len,
            capacity_tokens,
        })
    }

    fn read_entry(&self, token_index: usize) -> Result<(Vec<f32>, Vec<f32>), super::RuntimeError> {
        if token_index >= self.len {
            return Err(super::RuntimeError::Backend(format!(
                "cuda kv cache entry read exceeds logical length: index={} len={}",
                token_index, self.len
            )));
        }
        let byte_offset = token_index
            .saturating_mul(self.width)
            .saturating_mul(std::mem::size_of::<u16>());
        let byte_len = self.width.saturating_mul(std::mem::size_of::<u16>());
        Ok((
            f16_bytes_to_f32_vec(
                self.key_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
            )?,
            f16_bytes_to_f32_vec(
                self.value_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
            )?,
        ))
    }

    fn truncated(&self, len: usize) -> Self {
        let mut truncated = self.clone();
        truncated.len = len.min(self.len);
        truncated
    }

    fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Debug)]
struct GptOssForwardStep {
    key: Vec<f32>,
    value: Vec<f32>,
    logits: Vec<f32>,
    selected_token: Option<TokenId>,
    kernel_count: usize,
    bytes_moved: u64,
    perf: Option<GptOssPerformanceMetrics>,
}

fn load_quantized_matrix(
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<QuantizedMatrix, ModelLoadError> {
    let storage = artifact.paged_tensor(name)?;
    let metadata = storage.metadata();
    let tensor_name = metadata.name.clone();
    let dims = metadata.shape.dims().to_vec();
    let quantization = metadata.quantization;
    let layout = metadata.quantized_layout;
    let [rows, columns] = dims.as_slice() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: tensor_name,
            expected: vec![0, 0],
            actual: dims,
        });
    };
    let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
        name: tensor_name,
        dtype: String::from("quantized"),
    })?;
    let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization,
            shape: dims.clone(),
        }
    })?;
    Ok(QuantizedMatrix {
        storage,
        mode: quantization,
        rows: *rows,
        columns: *columns,
        row_byte_len,
    })
}

fn load_metal_quantized_matrix(
    backend: &mut MetalBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<MetalQuantizedMatrix, ModelLoadError> {
    let host = load_quantized_matrix(artifact, name)?;
    let keepalive: Arc<PagedTensorStorage> = Arc::new(host.storage.clone());
    let bytes_owner = Arc::clone(&keepalive);
    let bytes = bytes_owner.bytes()?;
    let keepalive: Arc<dyn std::any::Any> = keepalive;
    let storage = backend
        .quantized_buffer_from_slice(
            host.storage.metadata().shape.clone(),
            host.mode,
            bytes,
            Some(keepalive),
        )
        .map_err(|error| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload `{name}` to metal: {error}"),
        })?;
    Ok(MetalQuantizedMatrix { host, storage })
}

fn load_cuda_quantized_matrix(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
    build_f16_mirror: bool,
    f16_mirror_state: &mut CudaF16MirrorState,
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<CudaQuantizedMatrix, ModelLoadError> {
    let host = load_quantized_matrix(artifact, name)?;
    let mode = host.mode;
    let rows = host.rows;
    let columns = host.columns;
    let row_byte_len = host.row_byte_len;
    let projected_device_bytes = host.byte_length();
    if placement.should_keep_dense_on_host(projected_device_bytes) {
        placement.force_host_backed_dense_tail();
        return Ok(CudaQuantizedMatrix {
            storage: None,
            host,
            transposed_f16: None,
            mode,
            rows,
            columns,
            row_byte_len,
        });
    }
    let storage = artifact.paged_tensor(name)?;
    let bytes = storage.bytes()?;
    let transposed_f16 = try_build_cuda_transposed_f16_mirror(
        backend,
        name,
        mode,
        rows,
        columns,
        row_byte_len,
        bytes,
        build_f16_mirror,
        f16_mirror_state,
    )?;
    match backend.byte_buffer(bytes) {
        Ok(buffer) => Ok(CudaQuantizedMatrix {
            storage: Some(buffer),
            host,
            transposed_f16,
            mode,
            rows,
            columns,
            row_byte_len,
        }),
        Err(error) if error.to_string().contains("out of memory") => {
            placement.force_host_backed_dense_tail();
            Ok(CudaQuantizedMatrix {
                storage: None,
                host,
                transposed_f16: None,
                mode,
                rows,
                columns,
                row_byte_len,
            })
        }
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload `{name}` to cuda: {error}"),
        }),
    }
}

fn decode_quantized_matrix_bytes_transposed_f16(
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
    name: &str,
) -> Result<Vec<u8>, ModelLoadError> {
    let expected_bytes = rows.saturating_mul(row_byte_len);
    if bytes.len() != expected_bytes {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "quantized tensor `{name}` byte length mismatch while building f16 transpose: expected {expected_bytes}, actual {}",
                bytes.len()
            ),
        });
    }
    let mut transposed = vec![
        0_u8;
        rows.saturating_mul(columns)
            .saturating_mul(std::mem::size_of::<u16>())
    ];
    let mut decoded_row = Vec::with_capacity(columns);
    for (row_index, row_bytes) in bytes.chunks_exact(row_byte_len).enumerate() {
        decoded_row.clear();
        decode_quantized_row_into(mode, row_bytes, &mut decoded_row).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to decode quantized tensor `{name}` while building f16 transpose: {error}"
                ),
            }
        })?;
        if decoded_row.len() != columns {
            return Err(ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "quantized tensor `{name}` decode width mismatch while building f16 transpose: expected {columns}, actual {}",
                    decoded_row.len()
                ),
            });
        }
        for (column_index, value) in decoded_row.iter().copied().enumerate() {
            let offset = column_index
                .saturating_mul(rows)
                .saturating_add(row_index)
                .saturating_mul(std::mem::size_of::<u16>());
            transposed[offset..offset + std::mem::size_of::<u16>()]
                .copy_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
    }
    Ok(transposed)
}

fn decode_quantized_matrix_bytes_row_major_f16(
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
    name: &str,
) -> Result<Vec<u8>, ModelLoadError> {
    let expected_bytes = rows.saturating_mul(row_byte_len);
    if bytes.len() != expected_bytes {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "quantized tensor `{name}` byte length mismatch while building row-major f16 mirror: expected {expected_bytes}, actual {}",
                bytes.len()
            ),
        });
    }
    let mut dense = vec![
        0_u8;
        rows.saturating_mul(columns)
            .saturating_mul(std::mem::size_of::<u16>())
    ];
    let mut decoded_row = Vec::with_capacity(columns);
    for (row_index, row_bytes) in bytes.chunks_exact(row_byte_len).enumerate() {
        decoded_row.clear();
        decode_quantized_row_into(mode, row_bytes, &mut decoded_row).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to decode quantized tensor `{name}` while building row-major f16 mirror: {error}"
                ),
            }
        })?;
        if decoded_row.len() != columns {
            return Err(ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "quantized tensor `{name}` decode width mismatch while building row-major f16 mirror: expected {columns}, actual {}",
                    decoded_row.len()
                ),
            });
        }
        for (column_index, value) in decoded_row.iter().copied().enumerate() {
            let offset = row_index
                .saturating_mul(columns)
                .saturating_add(column_index)
                .saturating_mul(std::mem::size_of::<u16>());
            dense[offset..offset + std::mem::size_of::<u16>()]
                .copy_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
    }
    Ok(dense)
}

#[cfg(test)]
fn decode_quantized_matrix_bytes_transposed_f32(
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
    name: &str,
) -> Result<Vec<f32>, ModelLoadError> {
    let expected_bytes = rows.saturating_mul(row_byte_len);
    if bytes.len() != expected_bytes {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "quantized tensor `{name}` byte length mismatch while building dense transpose: expected {expected_bytes}, actual {}",
                bytes.len()
            ),
        });
    }
    let mut transposed = vec![0.0_f32; rows.saturating_mul(columns)];
    let mut decoded_row = Vec::with_capacity(columns);
    for (row_index, row_bytes) in bytes.chunks_exact(row_byte_len).enumerate() {
        decoded_row.clear();
        decode_quantized_row_into(mode, row_bytes, &mut decoded_row).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to decode quantized tensor `{name}` while building dense transpose: {error}"
                ),
            }
        })?;
        if decoded_row.len() != columns {
            return Err(ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "quantized tensor `{name}` decode width mismatch while building dense transpose: expected {columns}, actual {}",
                    decoded_row.len()
                ),
            });
        }
        for (column_index, value) in decoded_row.iter().copied().enumerate() {
            transposed[column_index.saturating_mul(rows).saturating_add(row_index)] = value;
        }
    }
    Ok(transposed)
}

fn try_build_cuda_transposed_f16_mirror(
    backend: &mut CudaBackend,
    name: &str,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
    build_f16_mirror: bool,
    f16_mirror_state: &mut CudaF16MirrorState,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    if !build_f16_mirror || f16_mirror_state.disabled || mode != QuantizationMode::GgmlQ8_0 {
        return Ok(None);
    }
    let transposed = decode_quantized_matrix_bytes_transposed_f16(
        mode,
        rows,
        columns,
        row_byte_len,
        bytes,
        name,
    )?;
    match backend.byte_buffer(transposed.as_slice()) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(error) if error.to_string().contains("out of memory") => {
            f16_mirror_state.disabled = true;
            Ok(None)
        }
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload f16 transpose mirror for `{name}` to cuda: {error}"),
        }),
    }
}

fn try_build_cuda_row_major_f16_mirror(
    backend: &mut CudaBackend,
    name: &str,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let dense = decode_quantized_matrix_bytes_row_major_f16(
        mode,
        rows,
        columns,
        row_byte_len,
        bytes,
        name,
    )?;
    match backend.byte_buffer(dense.as_slice()) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(error) if error.to_string().contains("out of memory") => Ok(None),
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload row-major f16 mirror for `{name}` to cuda: {error}"),
        }),
    }
}

fn load_quantized_expert_tensor(
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<QuantizedExpertTensor, ModelLoadError> {
    let storage = artifact.paged_tensor(name)?;
    let metadata = storage.metadata();
    let tensor_name = metadata.name.clone();
    let dims = metadata.shape.dims().to_vec();
    let quantization = metadata.quantization;
    let layout = metadata.quantized_layout;
    let [outer, rows, columns] = dims.as_slice() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: tensor_name,
            expected: vec![0, 0, 0],
            actual: dims,
        });
    };
    let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
        name: tensor_name,
        dtype: String::from("quantized"),
    })?;
    let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization,
            shape: dims.clone(),
        }
    })?;
    Ok(QuantizedExpertTensor {
        storage,
        mode: quantization,
        outer: *outer,
        rows: *rows,
        columns: *columns,
        row_byte_len,
    })
}

fn load_quantized_expert_projection_group(
    artifact: &GgufBlobArtifact,
    names: &[&str],
) -> Result<QuantizedExpertProjectionGroup, ModelLoadError> {
    if names.len() != 2 {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "host expert projection group requires exactly 2 tensors, actual {} for `{}`",
                names.len(),
                names.join(", ")
            ),
        });
    }
    let gate = load_quantized_expert_tensor(artifact, names[0])?;
    let up = load_quantized_expert_tensor(artifact, names[1])?;
    if gate.mode != up.mode {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "host expert projection group requires matching quantization, `{}` had {:?} but `{}` had {:?}",
                names[0], gate.mode, names[1], up.mode
            ),
        });
    }
    if gate.outer != up.outer {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "host expert projection group requires matching expert count, `{}` had {} but `{}` had {}",
                names[0], gate.outer, names[1], up.outer
            ),
        });
    }
    if gate.columns != up.columns {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "host expert projection group requires matching input width, `{}` had {} but `{}` had {}",
                names[0], gate.columns, names[1], up.columns
            ),
        });
    }
    Ok(QuantizedExpertProjectionGroup {
        outer: gate.outer,
        columns: gate.columns,
        gate,
        up,
    })
}

fn load_metal_quantized_expert_tensor(
    backend: &mut MetalBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<MetalQuantizedExpertTensor, ModelLoadError> {
    let storage = artifact.paged_tensor(name)?;
    let metadata = storage.metadata();
    let tensor_name = metadata.name.clone();
    let dims = metadata.shape.dims().to_vec();
    let quantization = metadata.quantization;
    let layout = metadata.quantized_layout;
    let [outer, rows, columns] = dims.as_slice() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: tensor_name,
            expected: vec![0, 0, 0],
            actual: dims,
        });
    };
    let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
        name: tensor_name,
        dtype: String::from("quantized"),
    })?;
    let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization,
            shape: dims.clone(),
        }
    })?;
    let keepalive: Arc<PagedTensorStorage> = Arc::new(storage.clone());
    let bytes_owner = Arc::clone(&keepalive);
    let bytes = bytes_owner.bytes()?;
    let keepalive: Arc<dyn std::any::Any> = keepalive;
    Ok(MetalQuantizedExpertTensor {
        storage: backend
            .quantized_buffer_from_slice(
                metadata.shape.clone(),
                quantization,
                bytes,
                Some(keepalive),
            )
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to upload `{name}` to metal: {error}"),
            })?,
        mode: quantization,
        outer: *outer,
        rows: *rows,
        columns: *columns,
        row_byte_len,
    })
}

fn load_cuda_quantized_expert_tensor(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<CudaQuantizedExpertTensor, ModelLoadError> {
    let storage = artifact.paged_tensor(name)?;
    let metadata = storage.metadata();
    let tensor_name = metadata.name.clone();
    let dims = metadata.shape.dims().to_vec();
    let quantization = metadata.quantization;
    let layout = metadata.quantized_layout;
    let [outer, rows, columns] = dims.as_slice() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: tensor_name,
            expected: vec![0, 0, 0],
            actual: dims,
        });
    };
    let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
        name: tensor_name,
        dtype: String::from("quantized"),
    })?;
    let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization,
            shape: dims.clone(),
        }
    })?;
    let projected_device_bytes = outer.saturating_mul(*rows).saturating_mul(row_byte_len);
    if placement.should_keep_moe_on_host(projected_device_bytes) {
        placement.force_host_backed_moe();
        return Ok(CudaQuantizedExpertTensor {
            storage: None,
            host: Some(QuantizedExpertTensor {
                storage,
                mode: quantization,
                outer: *outer,
                rows: *rows,
                columns: *columns,
                row_byte_len,
            }),
            mode: quantization,
            outer: *outer,
            rows: *rows,
            columns: *columns,
            row_byte_len,
        });
    }
    let bytes = storage.bytes()?;
    match backend.byte_buffer(bytes) {
        Ok(buffer) => Ok(CudaQuantizedExpertTensor {
            storage: Some(buffer),
            host: None,
            mode: quantization,
            outer: *outer,
            rows: *rows,
            columns: *columns,
            row_byte_len,
        }),
        Err(error) if error.to_string().contains("out of memory") => {
            placement.force_host_backed_moe();
            Ok(CudaQuantizedExpertTensor {
                storage: None,
                host: Some(QuantizedExpertTensor {
                    storage,
                    mode: quantization,
                    outer: *outer,
                    rows: *rows,
                    columns: *columns,
                    row_byte_len,
                }),
                mode: quantization,
                outer: *outer,
                rows: *rows,
                columns: *columns,
                row_byte_len,
            })
        }
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload `{name}` to cuda: {error}"),
        }),
    }
}

fn load_cuda_quantized_projection_group(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
    build_f16_mirror: bool,
    f16_mirror_state: &mut CudaF16MirrorState,
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<CudaQuantizedProjectionGroup, ModelLoadError> {
    let mut mode = None;
    let mut columns = None;
    let mut row_byte_len = None;
    let mut rows_per_projection = Vec::with_capacity(names.len());
    let mut host_projections = Vec::with_capacity(names.len());
    for name in names {
        let projection = load_quantized_matrix(artifact, name)?;
        if let Some(expected_mode) = mode {
            if expected_mode != projection.mode {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda projection group requires matching quantization, `{name}` had {:?} but expected {expected_mode:?}",
                        projection.mode
                    ),
                });
            }
        } else {
            mode = Some(projection.mode);
        }
        if let Some(expected_columns) = columns {
            if expected_columns != projection.columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda projection group requires matching input width, `{name}` had {} but expected {expected_columns}",
                        projection.columns
                    ),
                });
            }
        } else {
            columns = Some(projection.columns);
        }
        if let Some(expected_row_byte_len) = row_byte_len {
            if expected_row_byte_len != projection.row_byte_len {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda projection group requires matching row layout, `{name}` had row byte length {} but expected {expected_row_byte_len}",
                        projection.row_byte_len
                    ),
                });
            }
        } else {
            row_byte_len = Some(projection.row_byte_len);
        }
        rows_per_projection.push(projection.rows);
        host_projections.push(projection);
    }
    let mode = mode.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda projection group requires at least one tensor name, actual 0 for `{}`",
            names.join(", ")
        ),
    })?;
    let columns = columns.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda projection group did not resolve an input width for `{}`",
            names.join(", ")
        ),
    })?;
    let row_byte_len = row_byte_len.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda projection group did not resolve a row layout for `{}`",
            names.join(", ")
        ),
    })?;
    let projected_device_bytes = host_projections
        .iter()
        .map(QuantizedMatrix::byte_length)
        .fold(0usize, usize::saturating_add);
    let host_group = QuantizedProjectionGroup {
        columns,
        projections: host_projections.clone(),
    };
    if placement.should_keep_dense_on_host(projected_device_bytes) {
        placement.force_host_backed_dense_tail();
        return Ok(CudaQuantizedProjectionGroup {
            storage: None,
            host: Some(host_group),
            transposed_f16: None,
            mode,
            rows_per_projection,
            columns,
        });
    }
    let mut projection_bytes = Vec::with_capacity(names.len());
    for projection in &host_projections {
        projection_bytes.push(projection.storage.bytes()?.to_vec());
    }
    let packed = pack_quantized_projection_bytes(
        projection_bytes
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    match backend.byte_buffer(packed.as_slice()) {
        Ok(storage) => Ok(CudaQuantizedProjectionGroup {
            transposed_f16: try_build_cuda_transposed_f16_mirror(
                backend,
                names.join(", ").as_str(),
                mode,
                rows_per_projection
                    .iter()
                    .copied()
                    .fold(0usize, usize::saturating_add),
                columns,
                row_byte_len,
                packed.as_slice(),
                build_f16_mirror,
                f16_mirror_state,
            )?,
            storage: Some(storage),
            host: None,
            mode,
            rows_per_projection,
            columns,
        }),
        Err(error) if error.to_string().contains("out of memory") => {
            placement.force_host_backed_dense_tail();
            Ok(CudaQuantizedProjectionGroup {
                storage: None,
                host: Some(host_group),
                transposed_f16: None,
                mode,
                rows_per_projection,
                columns,
            })
        }
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "failed to upload packed cuda projection group `{}`: {error}",
                names.join(", ")
            ),
        }),
    }
}

fn load_metal_quantized_expert_projection_group(
    backend: &mut MetalBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
) -> Result<MetalQuantizedExpertProjectionGroup, ModelLoadError> {
    let mut mode = None;
    let mut outer = None;
    let mut columns = None;
    let mut row_byte_len = None;
    let mut rows_per_projection = Vec::with_capacity(names.len());
    let mut projection_bytes = Vec::with_capacity(names.len());
    for name in names {
        let storage = artifact.paged_tensor(name)?;
        let metadata = storage.metadata();
        let tensor_name = metadata.name.clone();
        let dims = metadata.shape.dims().to_vec();
        let quantization = metadata.quantization;
        let layout = metadata.quantized_layout;
        let [projection_outer, rows, projection_columns] = dims.as_slice() else {
            return Err(ModelLoadError::InvalidTensorShape {
                name: tensor_name,
                expected: vec![0, 0, 0],
                actual: dims,
            });
        };
        let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
            name: tensor_name,
            dtype: String::from("quantized"),
        })?;
        let projection_row_byte_len =
            quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
                ModelLoadError::InvalidQuantizedTensorShape {
                    quantization,
                    shape: dims.clone(),
                }
            })?;
        if let Some(expected_mode) = mode {
            if expected_mode != quantization {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed metal expert projection group requires matching quantization, `{name}` had {quantization:?} but expected {expected_mode:?}"
                    ),
                });
            }
        } else {
            mode = Some(quantization);
        }
        if let Some(expected_outer) = outer {
            if expected_outer != *projection_outer {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed metal expert projection group requires matching expert count, `{name}` had {projection_outer} but expected {expected_outer}"
                    ),
                });
            }
        } else {
            outer = Some(*projection_outer);
        }
        if let Some(expected_columns) = columns {
            if expected_columns != *projection_columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed metal expert projection group requires matching input width, `{name}` had {projection_columns} but expected {expected_columns}"
                    ),
                });
            }
        } else {
            columns = Some(*projection_columns);
        }
        if let Some(expected_row_byte_len) = row_byte_len {
            if expected_row_byte_len != projection_row_byte_len {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed metal expert projection group requires matching row layout, `{name}` had row byte length {projection_row_byte_len} but expected {expected_row_byte_len}"
                    ),
                });
            }
        } else {
            row_byte_len = Some(projection_row_byte_len);
        }
        rows_per_projection.push(*rows);
        projection_bytes.push(storage.bytes()?.to_vec());
    }
    let row_byte_len = row_byte_len.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed metal expert projection group did not resolve a row layout for `{}`",
            names.join(", ")
        ),
    })?;
    let outer = outer.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed metal expert projection group did not resolve an expert count for `{}`",
            names.join(", ")
        ),
    })?;
    let columns = columns.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed metal expert projection group did not resolve an input width for `{}`",
            names.join(", ")
        ),
    })?;
    let mode = mode.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed metal expert projection group requires at least one tensor name, actual 0 for `{}`",
            names.join(", ")
        ),
    })?;
    let packed = pack_quantized_expert_projection_bytes(
        row_byte_len,
        outer,
        rows_per_projection.as_slice(),
        projection_bytes
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let total_rows = rows_per_projection
        .iter()
        .copied()
        .fold(0usize, usize::saturating_add);
    let packed = Arc::new(packed);
    let bytes_owner = Arc::clone(&packed);
    let bytes = bytes_owner.as_slice();
    let keepalive: Arc<dyn std::any::Any> = packed;
    let storage = backend
        .quantized_buffer_from_slice(
            Shape::new(vec![outer, total_rows, columns]),
            mode,
            bytes,
            Some(keepalive),
        )
        .map_err(|error| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "failed to upload packed metal expert projection group `{}`: {error}",
                names.join(", ")
            ),
        })?;
    Ok(MetalQuantizedExpertProjectionGroup {
        storage,
        mode,
        outer,
        rows_per_projection,
        columns,
        row_byte_len,
    })
}

fn load_cuda_quantized_expert_projection_group(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<CudaQuantizedExpertProjectionGroup, ModelLoadError> {
    let mut mode = None;
    let mut outer = None;
    let mut columns = None;
    let mut row_byte_len = None;
    let mut rows_per_projection = Vec::with_capacity(names.len());
    for name in names {
        let storage = artifact.paged_tensor(name)?;
        let metadata = storage.metadata();
        let tensor_name = metadata.name.clone();
        let dims = metadata.shape.dims().to_vec();
        let quantization = metadata.quantization;
        let layout = metadata.quantized_layout;
        let [projection_outer, rows, projection_columns] = dims.as_slice() else {
            return Err(ModelLoadError::InvalidTensorShape {
                name: tensor_name,
                expected: vec![0, 0, 0],
                actual: dims,
            });
        };
        let layout = layout.ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
            name: tensor_name,
            dtype: String::from("quantized"),
        })?;
        let projection_row_byte_len =
            quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
                ModelLoadError::InvalidQuantizedTensorShape {
                    quantization,
                    shape: dims.clone(),
                }
            })?;
        if let Some(expected_mode) = mode {
            if expected_mode != quantization {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda expert projection group requires matching quantization, `{name}` had {quantization:?} but expected {expected_mode:?}"
                    ),
                });
            }
        } else {
            mode = Some(quantization);
        }
        if let Some(expected_outer) = outer {
            if expected_outer != *projection_outer {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda expert projection group requires matching expert count, `{name}` had {projection_outer} but expected {expected_outer}"
                    ),
                });
            }
        } else {
            outer = Some(*projection_outer);
        }
        if let Some(expected_columns) = columns {
            if expected_columns != *projection_columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda expert projection group requires matching input width, `{name}` had {projection_columns} but expected {expected_columns}"
                    ),
                });
            }
        } else {
            columns = Some(*projection_columns);
        }
        if let Some(expected_row_byte_len) = row_byte_len {
            if expected_row_byte_len != projection_row_byte_len {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda expert projection group requires matching row layout, `{name}` had row byte length {projection_row_byte_len} but expected {expected_row_byte_len}"
                    ),
                });
            }
        } else {
            row_byte_len = Some(projection_row_byte_len);
        }
        rows_per_projection.push(*rows);
    }
    let row_byte_len = row_byte_len.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda expert projection group did not resolve a row layout for `{}`",
            names.join(", ")
        ),
    })?;
    let outer = outer.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda expert projection group did not resolve an expert count for `{}`",
            names.join(", ")
        ),
    })?;
    let columns = columns.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda expert projection group did not resolve an input width for `{}`",
            names.join(", ")
        ),
    })?;
    let mode = mode.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!(
            "packed cuda expert projection group requires at least one tensor name, actual 0 for `{}`",
            names.join(", ")
        ),
    })?;
    let projected_device_bytes = outer
        .saturating_mul(
            rows_per_projection
                .iter()
                .copied()
                .fold(0usize, usize::saturating_add),
        )
        .saturating_mul(row_byte_len);
    if placement.should_keep_moe_on_host(projected_device_bytes) {
        placement.force_host_backed_moe();
        return Ok(CudaQuantizedExpertProjectionGroup {
            storage: None,
            host: Some(load_quantized_expert_projection_group(artifact, names)?),
            mode,
            outer,
            rows_per_projection,
            columns,
            row_byte_len,
        });
    }
    let mut projection_bytes = Vec::with_capacity(names.len());
    for name in names {
        projection_bytes.push(artifact.paged_tensor(name)?.bytes()?.to_vec());
    }
    let packed = pack_quantized_expert_projection_bytes(
        row_byte_len,
        outer,
        rows_per_projection.as_slice(),
        projection_bytes
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    match backend.byte_buffer(packed.as_slice()) {
        Ok(buffer) => Ok(CudaQuantizedExpertProjectionGroup {
            storage: Some(buffer),
            host: None,
            mode,
            outer,
            rows_per_projection,
            columns,
            row_byte_len,
        }),
        Err(error) if error.to_string().contains("out of memory") => {
            placement.force_host_backed_moe();
            Ok(CudaQuantizedExpertProjectionGroup {
                storage: None,
                host: Some(load_quantized_expert_projection_group(artifact, names)?),
                mode,
                outer,
                rows_per_projection,
                columns,
                row_byte_len,
            })
        }
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "failed to upload packed cuda expert projection group `{}`: {error}",
                names.join(", ")
            ),
        }),
    }
}

fn load_dense_vector(artifact: &GgufBlobArtifact, name: &str) -> Result<Vec<f32>, ModelLoadError> {
    let tensor = artifact.load_tensor(name)?;
    tensor.values().map(|values| values.into_owned())
}

fn upload_cuda_f32_buffer(
    backend: &mut CudaBackend,
    name: &str,
    values: &[f32],
) -> Result<CudaBuffer, ModelLoadError> {
    let mut buffer =
        backend
            .f32_buffer(values.len())
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to allocate cuda buffer for `{name}`: {error}"),
            })?;
    buffer
        .write_f32(values)
        .map_err(|error| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload `{name}` to cuda: {error}"),
        })?;
    Ok(buffer)
}

fn upload_optional_cuda_moe_bias_buffer(
    backend: &mut CudaBackend,
    name: &str,
    values: Option<&Vec<f32>>,
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let Some(values) = values else {
        return Ok(None);
    };
    let projected_device_bytes = values.len().saturating_mul(std::mem::size_of::<f32>());
    if placement.should_keep_moe_on_host(projected_device_bytes) {
        placement.force_host_backed_moe();
        return Ok(None);
    }
    match upload_cuda_f32_buffer(backend, name, values.as_slice()) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(ModelLoadError::ArtifactFormat { message, .. })
            if message.contains("out of memory") =>
        {
            placement.force_host_backed_moe();
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn upload_optional_cuda_dense_buffer(
    backend: &mut CudaBackend,
    name: &str,
    values: &[f32],
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let projected_device_bytes = values.len().saturating_mul(std::mem::size_of::<f32>());
    if placement.should_keep_dense_on_host(projected_device_bytes) {
        placement.force_host_backed_dense_tail();
        return Ok(None);
    }
    match upload_cuda_f32_buffer(backend, name, values) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(ModelLoadError::ArtifactFormat { message, .. })
            if message.contains("out of memory") =>
        {
            placement.force_host_backed_dense_tail();
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn upload_optional_cuda_aux_dense_buffer(
    backend: &mut CudaBackend,
    name: &str,
    values: &[f32],
    placement: &mut CudaLoadPlacementPolicy,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let projected_device_bytes = values.len().saturating_mul(std::mem::size_of::<f32>());
    if placement.should_keep_aux_dense_on_host(projected_device_bytes) {
        return Ok(None);
    }
    match upload_cuda_f32_buffer(backend, name, values) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(ModelLoadError::ArtifactFormat { message, .. })
            if message.contains("out of memory") =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn rope_runtime_parameters(
    rotary_dim: usize,
    metadata: &GgufDecoderFamilyMetadata,
) -> (f32, f32, [f32; 2], f32) {
    let freq_scale = metadata
        .rope_scaling_factor
        .filter(|value| *value > 0.0)
        .map_or(1.0, |value| 1.0 / value);
    let ext_factor = metadata
        .rope_scaling_factor
        .zip(metadata.rope_original_context_length)
        .filter(|(factor, original)| *factor > 1.0 && *original > 0)
        .map_or(0.0, |_| 1.0);
    let corr_dims = metadata
        .rope_original_context_length
        .map(|original| rope_yarn_corr_dims(rotary_dim, original, metadata.rope_theta))
        .unwrap_or([0.0, rotary_dim as f32 - 1.0]);
    let theta_scale = metadata.rope_theta.powf(-2.0 / rotary_dim as f32);
    (freq_scale, ext_factor, corr_dims, theta_scale)
}

fn load_dense_matrix(
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<DenseMatrix, ModelLoadError> {
    let tensor = artifact.load_tensor(name)?;
    let [rows, columns] = tensor.metadata().shape.dims() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: tensor.metadata().name.clone(),
            expected: vec![0, 0],
            actual: tensor.metadata().shape.dims().to_vec(),
        });
    };
    Ok(DenseMatrix {
        rows: *rows,
        columns: *columns,
        values: tensor.values()?.into_owned(),
    })
}

fn load_dense_rank2_flat(
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<Vec<f32>, ModelLoadError> {
    artifact
        .load_tensor(name)?
        .values()
        .map(|values| values.into_owned())
}

fn required_tensor_name<'a>(
    name: Option<&'a String>,
    field: &str,
) -> Result<&'a str, ModelLoadError> {
    name.map(String::as_str)
        .ok_or_else(|| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("missing required gpt-oss tensor layout field `{field}`"),
        })
}

fn model_load_runtime_error(error: ModelLoadError) -> super::RuntimeError {
    super::RuntimeError::Backend(error.to_string())
}

fn decode_step_plan_compile_path(plan_cache_hit: bool) -> CompilePathEvidence {
    CompilePathEvidence {
        temperature: if plan_cache_hit {
            CompilePathTemperature::WarmReuse
        } else {
            CompilePathTemperature::ColdCompile
        },
        execution_plan_cache: if plan_cache_hit {
            CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Reuse,
                "reused a cached gpt-oss cuda decode-step plan",
            )
        } else {
            CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Rebuild,
                "built a new gpt-oss cuda decode-step plan",
            )
        },
        kernel_cache: CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Reuse,
            "reused the configured cuda kernel cache",
        ),
    }
}

fn metal_decode_step_plan_compile_path(plan_cache_hit: bool) -> CompilePathEvidence {
    CompilePathEvidence {
        temperature: if plan_cache_hit {
            CompilePathTemperature::WarmReuse
        } else {
            CompilePathTemperature::ColdCompile
        },
        execution_plan_cache: if plan_cache_hit {
            CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Reuse,
                "reused a cached gpt-oss metal decode-step plan",
            )
        } else {
            CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Rebuild,
                "built a new gpt-oss metal decode-step plan",
            )
        },
        kernel_cache: CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Reuse,
            "reused the configured metal kernel cache",
        ),
    }
}

fn cache_key_value_byte_len(width: usize) -> u64 {
    width
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<u16>())
        .try_into()
        .unwrap_or(u64::MAX)
}

fn write_metal_buffer_prefix(
    buffer: &mut MetalBuffer,
    values: &[f32],
    perf: &mut GptOssPerformanceMetrics,
) -> Result<(), ReferenceTextGenerationError> {
    buffer
        .write_f32_prefix(values)
        .map_err(ReferenceTextGenerationError::Runtime)?;
    accumulate_metal_host_to_device_bytes(
        perf,
        values.len().saturating_mul(std::mem::size_of::<f32>()),
    );
    Ok(())
}

fn read_metal_buffer_prefix_into(
    buffer: &MetalBuffer,
    element_count: usize,
    values: &mut Vec<f32>,
    perf: &mut GptOssPerformanceMetrics,
) -> Result<(), ReferenceTextGenerationError> {
    buffer
        .read_f32_prefix_into(element_count, values)
        .map_err(ReferenceTextGenerationError::Runtime)?;
    accumulate_metal_device_to_host_bytes(
        perf,
        element_count.saturating_mul(std::mem::size_of::<f32>()),
    );
    Ok(())
}

fn f32_slice_to_f16_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(std::mem::size_of::<u16>()));
    for value in values {
        bytes.extend_from_slice(&f32_to_f16_bits(*value).to_le_bytes());
    }
    bytes
}

fn f16_bytes_to_f32_vec(bytes: &[u8]) -> Result<Vec<f32>, super::RuntimeError> {
    if bytes.len() % std::mem::size_of::<u16>() != 0 {
        return Err(super::RuntimeError::Backend(format!(
            "f16 byte buffer length must be divisible by 2, actual {}",
            bytes.len()
        )));
    }
    let mut values = Vec::with_capacity(bytes.len() / std::mem::size_of::<u16>());
    for chunk in bytes.chunks_exact(std::mem::size_of::<u16>()) {
        values.push(f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])));
    }
    Ok(values)
}

fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;

    if exponent == 0xff {
        if mantissa == 0 {
            return sign | 0x7c00;
        }
        return sign | 0x7c00 | ((mantissa >> 13) as u16) | 1;
    }

    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }

    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x0080_0000;
        let shift = u32::try_from(14 - half_exponent).unwrap_or(u32::MAX);
        let round_bit = 1_u32 << shift.saturating_sub(1);
        let round_mask = round_bit.saturating_sub(1);
        let mut half_mantissa = (mantissa >> shift) as u16;
        let round_bits = mantissa & (round_bit | round_mask);
        if round_bits > round_bit || (round_bits == round_bit && (half_mantissa & 1) != 0) {
            half_mantissa = half_mantissa.saturating_add(1);
        }
        return sign | half_mantissa;
    }

    let mut half_bits = sign | ((half_exponent as u16) << 10) | ((mantissa >> 13) as u16);
    let round_bits = mantissa & 0x1fff;
    if round_bits > 0x1000 || (round_bits == 0x1000 && (half_bits & 1) != 0) {
        half_bits = half_bits.saturating_add(1);
    }
    half_bits
}

fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits & 0x8000)) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = bits & 0x03ff;

    let value = if exponent == 0 {
        if mantissa == 0 {
            sign
        } else {
            let mut normalized = u32::from(mantissa);
            let mut shift = 0_u32;
            while (normalized & 0x0400) == 0 {
                normalized <<= 1;
                shift = shift.saturating_add(1);
            }
            normalized &= 0x03ff;
            sign | ((113_u32.saturating_sub(shift)) << 23) | (normalized << 13)
        }
    } else if exponent == 0x1f {
        sign | 0x7f80_0000 | (u32::from(mantissa) << 13)
    } else {
        sign | ((u32::from(exponent) + 112) << 23) | (u32::from(mantissa) << 13)
    };
    f32::from_bits(value)
}

fn digest_gpt_oss_cuda_step_plan(model_plan_digest: &str, graph_signature: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model_plan_digest.as_bytes());
    hasher.update(b"|cuda-decode-step|v1");
    hasher.update(b"|");
    hasher.update(graph_signature.as_bytes());
    hex::encode(hasher.finalize())
}

fn digest_gpt_oss_plan(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
    backend: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(descriptor.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.weights.digest.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.architecture.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.rope_theta.to_bits().to_be_bytes());
    hasher.update(b"|");
    hasher.update(backend.as_bytes());
    hex::encode(hasher.finalize())
}

fn rms_norm(input: &[f32], weight: &[f32], epsilon: f32) -> Vec<f32> {
    let mean_square = input.iter().map(|value| value * value).sum::<f32>() / input.len() as f32;
    let scale = (mean_square + epsilon).sqrt().recip();
    input
        .iter()
        .zip(weight.iter())
        .map(|(value, weight)| value * scale * weight)
        .collect()
}

fn add_vectors(left: &[f32], right: &[f32]) -> Result<Vec<f32>, super::RuntimeError> {
    if left.len() != right.len() {
        return Err(super::RuntimeError::Backend(format!(
            "vector width mismatch: left={} right={}",
            left.len(),
            right.len()
        )));
    }
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(left, right)| left + right)
        .collect())
}

fn add_bias_in_place(values: &mut [f32], bias: &[f32]) {
    for (value, bias) in values.iter_mut().zip(bias.iter().copied()) {
        *value += bias;
    }
}

fn add_expert_bias_in_place(values: &mut [f32], bias: &[f32], expert_index: usize, width: usize) {
    let start = expert_index.saturating_mul(width);
    let end = start.saturating_add(width).min(bias.len());
    add_bias_in_place(values, &bias[start..end]);
}

fn top_k_indices(values: &[f32], k: usize) -> Vec<usize> {
    let mut indices = (0..values.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        values[*right]
            .partial_cmp(&values[*left])
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.cmp(right))
    });
    indices.truncate(k.min(indices.len()));
    indices
}

fn softmax_selected(values: &[f32], indices: &[usize]) -> Vec<f32> {
    let selected = indices
        .iter()
        .copied()
        .map(|index| values[index])
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Vec::new();
    }
    let max_value = selected.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut weights = selected
        .iter()
        .copied()
        .map(|value| (value - max_value).exp())
        .collect::<Vec<_>>();
    let denom = weights.iter().copied().sum::<f32>();
    if denom != 0.0 {
        for weight in &mut weights {
            *weight /= denom;
        }
    }
    weights
}

fn oai_swiglu(gate: &[f32], up: &[f32]) -> Vec<f32> {
    gate.iter()
        .zip(up.iter())
        .map(|(gate, up)| {
            let x = gate.min(GPT_OSS_OAI_SWIGLU_LIMIT);
            let y = up.clamp(-GPT_OSS_OAI_SWIGLU_LIMIT, GPT_OSS_OAI_SWIGLU_LIMIT);
            let out_glu = x / (1.0 + (GPT_OSS_OAI_SWIGLU_ALPHA * -x).exp());
            out_glu * (y + 1.0)
        })
        .collect()
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn axpy(destination: &mut [f32], source: &[f32], alpha: f32) {
    for (destination, source) in destination.iter_mut().zip(source.iter().copied()) {
        *destination += source * alpha;
    }
}

fn apply_rope_neox(
    values: &mut [f32],
    head_count: usize,
    head_dim: usize,
    rotary_dim: usize,
    position: usize,
    metadata: &GgufDecoderFamilyMetadata,
) {
    let rotary_dim = rotary_dim.min(head_dim).max(2);
    let freq_scale = metadata
        .rope_scaling_factor
        .filter(|value| *value > 0.0)
        .map_or(1.0, |value| 1.0 / value);
    let ext_factor = metadata
        .rope_scaling_factor
        .zip(metadata.rope_original_context_length)
        .filter(|(factor, original)| *factor > 1.0 && *original > 0)
        .map_or(0.0, |_| 1.0);
    let corr_dims = metadata
        .rope_original_context_length
        .map(|original| rope_yarn_corr_dims(rotary_dim, original, metadata.rope_theta))
        .unwrap_or([0.0, rotary_dim as f32 - 1.0]);
    let theta_scale = metadata.rope_theta.powf(-2.0 / rotary_dim as f32);
    for head_index in 0..head_count {
        let head_base = head_index.saturating_mul(head_dim);
        for i0 in (0..rotary_dim).step_by(2) {
            let pair = i0 / 2;
            let index0 = head_base + pair;
            let index1 = head_base + pair + rotary_dim / 2;
            if index1 >= head_base + head_dim || index1 >= values.len() {
                continue;
            }
            let theta_base = position as f32 * theta_scale.powf(pair as f32);
            let (cos_theta, sin_theta) =
                rope_yarn(theta_base, freq_scale, corr_dims, i0, ext_factor, 1.0);
            let x0 = values[index0];
            let x1 = values[index1];
            values[index0] = x0 * cos_theta - x1 * sin_theta;
            values[index1] = x0 * sin_theta + x1 * cos_theta;
        }
    }
}

fn metal_rope_cos_sin_values(
    position: usize,
    head_dim: usize,
    rotary_dim: usize,
    metadata: &GgufDecoderFamilyMetadata,
) -> (Vec<f32>, Vec<f32>) {
    let rotary_dim = rotary_dim.min(head_dim).max(2);
    let half_dim = head_dim / 2;
    let freq_scale = metadata
        .rope_scaling_factor
        .filter(|value| *value > 0.0)
        .map_or(1.0, |value| 1.0 / value);
    let ext_factor = metadata
        .rope_scaling_factor
        .zip(metadata.rope_original_context_length)
        .filter(|(factor, original)| *factor > 1.0 && *original > 0)
        .map_or(0.0, |_| 1.0);
    let corr_dims = metadata
        .rope_original_context_length
        .map(|original| rope_yarn_corr_dims(rotary_dim, original, metadata.rope_theta))
        .unwrap_or([0.0, rotary_dim as f32 - 1.0]);
    let theta_scale = metadata.rope_theta.powf(-2.0 / rotary_dim as f32);
    let mut cos = vec![1.0; half_dim];
    let mut sin = vec![0.0; half_dim];
    for pair in 0..(rotary_dim / 2) {
        let i0 = pair * 2;
        let theta_base = position as f32 * theta_scale.powf(pair as f32);
        let (cos_theta, sin_theta) =
            rope_yarn(theta_base, freq_scale, corr_dims, i0, ext_factor, 1.0);
        cos[pair] = cos_theta;
        sin[pair] = sin_theta;
    }
    (cos, sin)
}

fn rope_yarn_corr_dims(n_dims: usize, n_ctx_orig: usize, freq_base: f32) -> [f32; 2] {
    let corr_dim = |n_rot: f32| {
        n_dims as f32
            * ((n_ctx_orig as f32 / (n_rot * 2.0 * std::f32::consts::PI)).ln()
                / (2.0 * freq_base.ln()))
    };
    let start = corr_dim(GPT_OSS_YARN_BETA_FAST).floor().max(0.0);
    let end = corr_dim(GPT_OSS_YARN_BETA_SLOW)
        .ceil()
        .min(n_dims.saturating_sub(1) as f32);
    [start, end]
}

fn rope_yarn(
    theta_extrap: f32,
    freq_scale: f32,
    corr_dims: [f32; 2],
    i0: usize,
    ext_factor: f32,
    mscale: f32,
) -> (f32, f32) {
    let theta_interp = freq_scale * theta_extrap;
    let mut theta = theta_interp;
    let mut mscale = mscale;
    if ext_factor != 0.0 {
        let ramp_mix = rope_yarn_ramp(corr_dims[0], corr_dims[1], i0) * ext_factor;
        theta = theta_interp * (1.0 - ramp_mix) + theta_extrap * ramp_mix;
        mscale *= 1.0 + 0.1 * (1.0 / freq_scale).ln();
    }
    (theta.cos() * mscale, theta.sin() * mscale)
}

fn rope_yarn_ramp(low: f32, high: f32, i0: usize) -> f32 {
    let y = ((i0 / 2) as f32 - low) / (high - low).max(0.001);
    1.0 - y.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::CpuGgufGptOssTextGenerationService;
    use super::{
        MetalGgufGptOssTextGenerationService, build_gpt_oss_decode_graph, can_use_q8_1_mmvq,
        decode_quantized_matrix_bytes_transposed_f16, decode_quantized_matrix_bytes_transposed_f32,
        decode_quantized_row_into, digest_gpt_oss_cuda_step_plan, f16_bits_to_f32,
        f32_slice_to_f16_bytes, f32_to_f16_bits, pack_quantized_expert_projection_bytes,
        pack_quantized_projection_bytes, split_projection_outputs,
    };
    use crate::QuantizationMode;
    #[cfg(target_os = "macos")]
    use crate::{
        GenerationOptions, GenerationRequest, GptOssMetalDecodeLogitsMetrics,
        GptOssMetalLogitsOutputMode, GptOssPerformanceMetrics, TextGenerationExecutor, TokenId,
        TokenSequence,
    };
    use psionic_backend_cpu::quantized_row_dot;
    use psionic_backend_cuda::{CudaBackend, CudaCommandStatus, CudaCommandWait};
    use psionic_models::{GgufMetadataValue, GgufTensorType};
    use psionic_runtime::{
        CacheAction, CacheKind, CompilePathTemperature, DeviceDiscovery, HealthStatus,
        PrefixCacheState,
    };
    use std::{fs, path::Path, path::PathBuf, time::Instant};
    #[cfg(target_os = "macos")]
    use tempfile::tempdir;

    #[cfg(target_os = "macos")]
    fn real_gpt_oss_gguf_path() -> Option<PathBuf> {
        ["/Users/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf"]
            .into_iter()
            .map(PathBuf::from)
            .find(|candidate| candidate.exists())
    }

    #[cfg(target_os = "macos")]
    fn run_real_metal_text_request(
        metal: &mut MetalGgufGptOssTextGenerationService,
        request_id: &str,
        prompt: &str,
        max_output_tokens: usize,
    ) -> Result<(crate::GenerationResponse, f64), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            request_id,
            metal.model_descriptor().clone(),
            None,
            prompt,
            GenerationOptions::greedy(max_output_tokens),
        );
        let started = Instant::now();
        let response = metal.generate(&request)?;
        Ok((response, started.elapsed().as_secs_f64()))
    }

    #[test]
    fn packed_projection_bytes_preserve_projection_order() {
        let packed = pack_quantized_projection_bytes(&[&[1, 2], &[3], &[4, 5, 6]]);
        assert_eq!(packed, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn packed_expert_projection_bytes_interleave_each_expert_group() {
        let packed =
            pack_quantized_expert_projection_bytes(1, 2, &[2, 1], &[&[10, 11, 20, 21], &[30, 40]]);
        assert_eq!(packed, vec![10, 11, 30, 20, 21, 40]);
    }

    #[test]
    fn split_projection_outputs_respects_segment_rows() {
        let outputs = split_projection_outputs(&[2, 1, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .expect("packed outputs split");
        assert_eq!(
            outputs,
            vec![vec![1.0, 2.0], vec![3.0], vec![4.0, 5.0, 6.0]]
        );
    }

    #[test]
    fn cuda_step_plan_digest_includes_decode_graph_signature() {
        let short_graph = build_gpt_oss_decode_graph(1);
        let long_graph = build_gpt_oss_decode_graph(2);
        let short_digest =
            digest_gpt_oss_cuda_step_plan("model-digest", short_graph.signature_key().as_str());
        let long_digest =
            digest_gpt_oss_cuda_step_plan("model-digest", long_graph.signature_key().as_str());
        assert_ne!(short_digest, long_digest);
    }

    #[test]
    fn q8_1_mmvq_helper_accepts_gpt_oss_quantization_modes() {
        assert!(can_use_q8_1_mmvq(QuantizationMode::GgmlQ8_0));
        assert!(can_use_q8_1_mmvq(QuantizationMode::GgmlMxfp4));
    }

    #[test]
    fn quantized_matrix_transpose_decode_preserves_row_values() {
        let row = std::iter::once(128_u8)
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect::<Vec<_>>();
        let bytes = row
            .iter()
            .copied()
            .chain(row.iter().copied())
            .collect::<Vec<_>>();
        let transposed = decode_quantized_matrix_bytes_transposed_f32(
            QuantizationMode::GgmlMxfp4,
            2,
            32,
            17,
            bytes.as_slice(),
            "test",
        )
        .expect("decode transpose");

        let mut decoded_row = Vec::new();
        decode_quantized_row_into(
            QuantizationMode::GgmlMxfp4,
            row.as_slice(),
            &mut decoded_row,
        )
        .expect("decode row");
        for (column_index, expected) in decoded_row.iter().copied().enumerate() {
            assert_eq!(transposed[column_index * 2], expected);
            assert_eq!(transposed[column_index * 2 + 1], expected);
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_gpt_oss_service_reports_backend_unavailable_off_platform() {
        match MetalGgufGptOssTextGenerationService::from_gguf_path("missing.gguf") {
            Err(super::MetalGptOssTextGenerationError::BackendUnavailable { .. }) => {}
            Err(error) => panic!("expected backend unavailable error, got {error:?}"),
            Ok(_) => panic!("off-platform metal service should refuse"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_matches_cpu_reference_on_synthetic_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny-gpt-oss.gguf");
        write_test_gpt_oss_gguf(&path)?;

        let mut cpu = CpuGgufGptOssTextGenerationService::from_gguf_path(&path)?;
        let mut metal = MetalGgufGptOssTextGenerationService::from_gguf_path(&path)?;
        assert_eq!(metal.backend_selection().requested_backend, "metal");
        assert_eq!(metal.backend_selection().effective_backend, "metal");

        let prompt_tokens = TokenSequence::new(vec![TokenId(2)]);
        let cpu_request = GenerationRequest::new_tokens(
            "tiny-gpt-oss-cpu",
            cpu.model_descriptor().clone(),
            None,
            prompt_tokens.clone(),
            GenerationOptions::greedy(1),
        );
        let metal_request = GenerationRequest::new_tokens(
            "tiny-gpt-oss-metal",
            metal.model_descriptor().clone(),
            None,
            prompt_tokens,
            GenerationOptions::greedy(1),
        );

        let cpu_response = cpu.generate(&cpu_request)?;
        let metal_response = metal.generate(&metal_request)?;
        assert_eq!(metal_response.output.tokens, cpu_response.output.tokens);
        assert_eq!(metal_response.output.text, cpu_response.output.text);
        assert_eq!(metal_response.termination, cpu_response.termination);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_reuses_device_prefix_and_reports_graph_metrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny-gpt-oss-device-kv.gguf");
        write_test_gpt_oss_gguf(&path)?;

        let mut metal = MetalGgufGptOssTextGenerationService::from_gguf_path(&path)?;
        let prompt_tokens = TokenSequence::new(vec![TokenId(2)]);
        let request = GenerationRequest::new_tokens(
            "tiny-gpt-oss-metal-device-kv",
            metal.model_descriptor().clone(),
            None,
            prompt_tokens,
            GenerationOptions::greedy(1),
        );

        let first = metal.generate(&request)?;
        let first_perf = first
            .metrics
            .gpt_oss_perf
            .as_ref()
            .ok_or("missing first gpt-oss perf")?;
        assert!(first_perf.graph_node_count > 0);
        assert!(first_perf.graph_layer_node_count > 0);
        assert!(matches!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.compile_path.as_ref())
                .map(|value| value.temperature),
            Some(CompilePathTemperature::WarmReuse | CompilePathTemperature::ColdCompile)
        ));
        assert!(
            first
                .provenance
                .as_ref()
                .map(|value| {
                    value.cache_observations.iter().any(|observation| {
                        observation.kind == CacheKind::KvState
                            && observation.detail.contains("device-resident kv state")
                    })
                })
                .unwrap_or(false)
        );

        let second = metal.generate(&request)?;
        assert_eq!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            Some(PrefixCacheState::Hit)
        );
        assert_eq!(second.metrics.prefix_tokens_reused, Some(1));
        let second_perf = second
            .metrics
            .gpt_oss_perf
            .as_ref()
            .ok_or("missing second gpt-oss perf")?;
        assert!(second_perf.graph_node_count > 0);
        assert!(second_perf.graph_layer_node_count > 0);
        assert_eq!(
            second
                .provenance
                .as_ref()
                .and_then(|value| value.compile_path.as_ref())
                .map(|value| value.temperature),
            Some(CompilePathTemperature::WarmReuse)
        );
        assert!(
            second
                .provenance
                .as_ref()
                .map(|value| {
                    value.cache_observations.iter().any(|observation| {
                        observation.kind == CacheKind::KvState
                            && observation.action == CacheAction::Reuse
                            && observation.detail == "device-resident kv state was reused"
                    })
                })
                .unwrap_or(false)
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn metal_perf_for_options(
        request_id: &str,
        options: GenerationOptions,
    ) -> Result<GptOssPerformanceMetrics, Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join(format!("{request_id}.gguf"));
        write_test_gpt_oss_gguf(&path)?;

        let mut metal = MetalGgufGptOssTextGenerationService::from_gguf_path(&path)?;
        let request = GenerationRequest::new_tokens(
            request_id,
            metal.model_descriptor().clone(),
            None,
            TokenSequence::new(vec![TokenId(2)]),
            options,
        );
        let response = metal.generate(&request)?;
        response
            .metrics
            .gpt_oss_perf
            .ok_or_else(|| std::io::Error::other("missing metal gpt-oss perf metrics").into())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_batches_decode_kernels_into_fewer_submissions()
    -> Result<(), Box<dyn std::error::Error>> {
        let perf =
            metal_perf_for_options("tiny-gpt-oss-metal-step-plan", GenerationOptions::greedy(1))?;

        assert_eq!(perf.step_count, 2);
        assert_eq!(perf.layer_visit_count, 2);
        assert_eq!(perf.metal.kernel_launches, 14);
        assert_eq!(perf.metal.submission_count, 10);
        assert!(perf.metal.submission_count < perf.metal.kernel_launches);
        assert!(perf.metal.grouped_expert_ids_path);
        assert!(perf.metal.host_to_device_bytes > 0);
        assert!(perf.metal.device_to_host_bytes > 0);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn metal_decode_logits_metrics_for_options(
        request_id: &str,
        options: GenerationOptions,
    ) -> Result<GptOssMetalDecodeLogitsMetrics, Box<dyn std::error::Error>> {
        metal_perf_for_options(request_id, options)?
            .metal_decode_logits
            .ok_or_else(|| std::io::Error::other("missing metal decode logits metrics").into())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_reports_greedy_decode_logits_mode()
    -> Result<(), Box<dyn std::error::Error>> {
        let metrics = metal_decode_logits_metrics_for_options(
            "tiny-gpt-oss-metal-greedy-logits",
            GenerationOptions::greedy(1),
        )?;

        assert_eq!(metrics.step_count, 1);
        assert_eq!(
            metrics.output_modes,
            vec![GptOssMetalLogitsOutputMode::GreedyToken]
        );
        assert_eq!(metrics.readback_bytes, std::mem::size_of::<u32>() as u64);
        assert!(!metrics.raw_logits_materialized);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_reports_bounded_top_k_decode_logits_mode()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut options = GenerationOptions::sample(1);
        options.top_k = Some(2);
        options.seed = Some(7);
        let metrics =
            metal_decode_logits_metrics_for_options("tiny-gpt-oss-metal-top-k-logits", options)?;

        assert_eq!(metrics.step_count, 1);
        assert_eq!(
            metrics.output_modes,
            vec![GptOssMetalLogitsOutputMode::TopKCandidates { top_k: 2 }]
        );
        assert_eq!(
            metrics.readback_bytes,
            (2 * std::mem::size_of::<u32>() + 2 * std::mem::size_of::<f32>()) as u64
        );
        assert!(!metrics.raw_logits_materialized);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_gpt_oss_service_reports_raw_decode_logits_mode_when_required()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut options = GenerationOptions::greedy(1);
        options.repeat_penalty = Some(1.1);
        let metrics =
            metal_decode_logits_metrics_for_options("tiny-gpt-oss-metal-raw-logits", options)?;

        assert_eq!(metrics.step_count, 1);
        assert_eq!(
            metrics.output_modes,
            vec![GptOssMetalLogitsOutputMode::RawLogits]
        );
        assert!(metrics.readback_bytes >= std::mem::size_of::<f32>() as u64);
        assert!(metrics.raw_logits_materialized);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "loads the local GPT-OSS GGUF and prints a direct native Metal receipt"]
    fn metal_real_gpt_oss_direct_short_text_receipt() -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = real_gpt_oss_gguf_path() else {
            return Ok(());
        };

        let mut metal = MetalGgufGptOssTextGenerationService::from_gguf_path(&path)?;
        let (prompt_only, prompt_only_wall) = run_real_metal_text_request(
            &mut metal,
            "real-metal-short-text-prompt-only",
            "Hello",
            0,
        )?;
        let (prompt_plus_one, prompt_plus_one_wall) =
            run_real_metal_text_request(&mut metal, "real-metal-short-text-plus-one", "Hello", 1)?;
        let prompt_ns = prompt_only
            .metrics
            .prompt_eval_duration_ns
            .unwrap_or_default();
        let prompt_tps = if prompt_ns == 0 {
            0.0
        } else {
            prompt_only.usage.input_tokens as f64 / (prompt_ns as f64 / 1_000_000_000.0)
        };
        let prompt_perf = prompt_only.metrics.gpt_oss_perf.as_ref();
        let plus_one_perf = prompt_plus_one.metrics.gpt_oss_perf.as_ref();

        println!(
            "real metal short-text receipt: prompt_only_tokens={} prompt_only_wall_s={prompt_only_wall:.3} prompt_only_prompt_tps={prompt_tps:.3} exact_hit_plus_one_output_tokens={} exact_hit_plus_one_wall_s={prompt_plus_one_wall:.3} exact_hit_prefix_tokens_reused={:?} plus_one_termination={:?} plus_one_output={:?}",
            prompt_only.usage.input_tokens,
            prompt_plus_one.usage.output_tokens,
            prompt_plus_one.metrics.prefix_tokens_reused,
            prompt_plus_one.termination,
            prompt_plus_one.output.text,
        );
        if let Some(perf) = prompt_perf {
            println!(
                "real metal prompt-only perf: step_count={} step_wall_s={:.3} attention_s={:.3} expert_projection_s={:.3} submissions={} kernels={} h2d_mb={:.3} d2h_mb={:.3}",
                perf.step_count,
                perf.stage_timings.step_wall_ns as f64 / 1_000_000_000.0,
                perf.stage_timings.attention_ns as f64 / 1_000_000_000.0,
                perf.stage_timings.expert_projection_ns as f64 / 1_000_000_000.0,
                perf.metal.submission_count,
                perf.metal.kernel_launches,
                perf.metal.host_to_device_bytes as f64 / (1024.0 * 1024.0),
                perf.metal.device_to_host_bytes as f64 / (1024.0 * 1024.0),
            );
        }
        if let Some(perf) = plus_one_perf {
            println!(
                "real metal plus-one perf: step_count={} step_wall_s={:.3} attention_s={:.3} expert_projection_s={:.3} submissions={} kernels={} h2d_mb={:.3} d2h_mb={:.3}",
                perf.step_count,
                perf.stage_timings.step_wall_ns as f64 / 1_000_000_000.0,
                perf.stage_timings.attention_ns as f64 / 1_000_000_000.0,
                perf.stage_timings.expert_projection_ns as f64 / 1_000_000_000.0,
                perf.metal.submission_count,
                perf.metal.kernel_launches,
                perf.metal.host_to_device_bytes as f64 / (1024.0 * 1024.0),
                perf.metal.device_to_host_bytes as f64 / (1024.0 * 1024.0),
            );
        }
        Ok(())
    }

    #[derive(Clone, Debug)]
    struct TestGgufTensor {
        name: String,
        shape: Vec<usize>,
        tensor_type: GgufTensorType,
        bytes: Vec<u8>,
    }

    impl TestGgufTensor {
        fn new(
            name: impl Into<String>,
            shape: Vec<usize>,
            tensor_type: GgufTensorType,
            bytes: Vec<u8>,
        ) -> Self {
            Self {
                name: name.into(),
                shape,
                tensor_type,
                bytes,
            }
        }
    }

    fn write_test_gpt_oss_gguf(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(path, build_test_gpt_oss_gguf()?)?;
        Ok(())
    }

    fn build_test_gpt_oss_gguf() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        build_test_gguf(gpt_oss_metadata().as_slice(), gpt_oss_tensors().as_slice())
    }

    fn build_test_gguf(
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let alignment = metadata
            .iter()
            .find(|(key, _)| key == "general.alignment")
            .and_then(|(_, value)| match value {
                GgufMetadataValue::U64(value) => Some(*value as usize),
                GgufMetadataValue::U32(value) => Some(*value as usize),
                _ => None,
            })
            .unwrap_or(32)
            .max(1);

        let mut bytes = Vec::new();
        bytes.extend(b"GGUF");
        push_u32(&mut bytes, 3);
        push_u64(&mut bytes, u64::try_from(tensors.len())?);
        push_u64(&mut bytes, u64::try_from(metadata.len())?);

        for (key, value) in metadata {
            push_gguf_string(&mut bytes, key)?;
            push_u32(&mut bytes, gguf_metadata_value_type(value));
            push_gguf_value(&mut bytes, value)?;
        }

        let mut next_offset = 0usize;
        let mut tensor_offsets = Vec::with_capacity(tensors.len());
        for tensor in tensors {
            tensor_offsets.push(next_offset);
            next_offset = align_usize(next_offset + tensor.bytes.len(), alignment);
        }

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            push_gguf_string(&mut bytes, tensor.name.as_str())?;
            push_u32(&mut bytes, u32::try_from(tensor.shape.len())?);
            for dimension in tensor.shape.iter().rev() {
                push_u64(&mut bytes, u64::try_from(*dimension)?);
            }
            push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
            push_u64(&mut bytes, u64::try_from(*offset)?);
        }

        let tensor_data_offset = align_usize(bytes.len(), alignment);
        bytes.resize(tensor_data_offset, 0);

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            let start = tensor_data_offset + offset;
            if bytes.len() < start {
                bytes.resize(start, 0);
            }
            bytes.extend_from_slice(tensor.bytes.as_slice());
            bytes.resize(align_usize(bytes.len(), alignment), 0);
        }

        Ok(bytes)
    }

    fn gpt_oss_metadata() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("gpt-oss")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(String::from("tiny psionic gpt-oss")),
            ),
            (
                String::from("general.alignment"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.context_length"),
                GgufMetadataValue::U32(128),
            ),
            (
                String::from("gpt-oss.embedding_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.feed_forward_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.expert_feed_forward_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.block_count"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("gpt-oss.attention.head_count"),
                GgufMetadataValue::U32(4),
            ),
            (
                String::from("gpt-oss.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("gpt-oss.attention.key_length"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.attention.value_length"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                String::from("gpt-oss.rope.dimension_count"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
            (
                String::from("gpt-oss.rope.scaling.factor"),
                GgufMetadataValue::F32(32.0),
            ),
            (
                String::from("gpt-oss.rope.scaling.original_context_length"),
                GgufMetadataValue::U32(4096),
            ),
            (
                String::from("gpt-oss.expert_count"),
                GgufMetadataValue::U32(3),
            ),
            (
                String::from("gpt-oss.expert_used_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("gpt-4o")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<|start|>")),
                    GgufMetadataValue::String(String::from("<|end|>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                    GgufMetadataValue::String(String::from("gpt-oss")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("hello world")),
                    GgufMetadataValue::String(String::from("psionic gpt-oss")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.unknown_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.padding_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn gpt_oss_tensors() -> Vec<TestGgufTensor> {
        let expert_blocks = 3 * 32;
        vec![
            quantized_q8_0_tensor("token_embd.weight", vec![6, 32]),
            dense_f32_tensor("output_norm.weight", vec![32]),
            quantized_q8_0_tensor("output.weight", vec![6, 32]),
            dense_f32_tensor("blk.0.attn_norm.weight", vec![32]),
            quantized_q8_0_tensor("blk.0.attn_q.weight", vec![64, 32]),
            dense_f32_tensor("blk.0.attn_q.bias", vec![64]),
            quantized_q8_0_tensor("blk.0.attn_k.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_k.bias", vec![16]),
            quantized_q8_0_tensor("blk.0.attn_v.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_v.bias", vec![16]),
            quantized_q8_0_tensor("blk.0.attn_output.weight", vec![32, 64]),
            dense_f32_tensor("blk.0.attn_output.bias", vec![32]),
            dense_f32_tensor("blk.0.post_attention_norm.weight", vec![32]),
            dense_f32_tensor("blk.0.attn_sinks.weight", vec![16]),
            dense_f32_tensor("blk.0.ffn_gate_inp.weight", vec![3, 32]),
            dense_f32_tensor("blk.0.ffn_gate_inp.bias", vec![3]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_gate_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_gate_exps.bias", vec![3, 32]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_up_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_up_exps.bias", vec![3, 32]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_down_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_down_exps.bias", vec![3, 32]),
        ]
    }

    fn dense_f32_tensor(name: &str, shape: Vec<usize>) -> TestGgufTensor {
        let elements = shape.iter().product::<usize>();
        TestGgufTensor::new(
            name,
            shape,
            GgufTensorType::F32,
            encode_f32_bytes(&vec![0.0; elements]),
        )
    }

    fn quantized_q8_0_tensor(name: &str, shape: Vec<usize>) -> TestGgufTensor {
        let rows = shape
            .iter()
            .take(shape.len().saturating_sub(1))
            .product::<usize>();
        TestGgufTensor::new(name, shape, GgufTensorType::Q8_0, repeated_q8_0_bytes(rows))
    }

    fn quantized_mxfp4_tensor(name: &str, shape: Vec<usize>, bytes: Vec<u8>) -> TestGgufTensor {
        TestGgufTensor::new(name, shape, GgufTensorType::MXFP4, bytes)
    }

    fn repeated_q8_0_bytes(row_count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(row_count * 34);
        for _ in 0..row_count {
            bytes.extend([0x00, 0x3c]);
            bytes.extend([0_u8; 32]);
        }
        bytes
    }

    fn repeated_mxfp4_bytes(block_count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(block_count * 17);
        for _ in 0..block_count {
            bytes.push(128_u8);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
        }
        bytes
    }

    fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
    }

    fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
        match value {
            GgufMetadataValue::U8(_) => 0,
            GgufMetadataValue::I8(_) => 1,
            GgufMetadataValue::U16(_) => 2,
            GgufMetadataValue::I16(_) => 3,
            GgufMetadataValue::U32(_) => 4,
            GgufMetadataValue::I32(_) => 5,
            GgufMetadataValue::F32(_) => 6,
            GgufMetadataValue::Bool(_) => 7,
            GgufMetadataValue::String(_) => 8,
            GgufMetadataValue::Array(_) => 9,
            GgufMetadataValue::U64(_) => 10,
            GgufMetadataValue::I64(_) => 11,
            GgufMetadataValue::F64(_) => 12,
        }
    }

    fn gguf_tensor_type_code(tensor_type: GgufTensorType) -> u32 {
        match tensor_type {
            GgufTensorType::F32 => 0,
            GgufTensorType::Q8_0 => 8,
            GgufTensorType::MXFP4 => 39,
            other => panic!("unsupported synthetic gguf tensor type: {other:?}"),
        }
    }

    fn push_gguf_string(
        bytes: &mut Vec<u8>,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        push_u64(bytes, u64::try_from(value.len())?);
        bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_gguf_value(
        bytes: &mut Vec<u8>,
        value: &GgufMetadataValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            GgufMetadataValue::U8(value) => bytes.push(*value),
            GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
            GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
            GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
            GgufMetadataValue::Array(values) => {
                let value_type = values.first().map_or(4, gguf_metadata_value_type);
                push_u32(bytes, value_type);
                push_u64(bytes, u64::try_from(values.len())?);
                for value in values {
                    push_gguf_value(bytes, value)?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn f32_to_f16_bits_preserves_half_subnormals() {
        let value = 1.0e-6_f32;
        let bits = f32_to_f16_bits(value);
        assert_ne!(bits, 0);
        let roundtrip = f16_bits_to_f32(bits);
        assert!(roundtrip > 0.0);
        assert!(roundtrip < 6.1035156e-5);
    }

    #[test]
    fn q8_0_transposed_f16_mirror_matches_quantized_projection_for_subnormal_scales_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let row_a = sample_q8_0_row(1.0e-6, 1);
        let row_b = sample_q8_0_row(2.0e-6, -1);
        let weights = row_a
            .iter()
            .copied()
            .chain(row_b.iter().copied())
            .collect::<Vec<_>>();
        let input = (0..32)
            .map(|index| index as f32 / 16.0 - 1.0)
            .collect::<Vec<_>>();
        let input_f16 = input
            .iter()
            .copied()
            .map(|value| f16_bits_to_f32(f32_to_f16_bits(value)))
            .collect::<Vec<_>>();
        let expected = [
            quantized_row_dot(&input_f16, QuantizationMode::GgmlQ8_0, row_a.as_slice())?,
            quantized_row_dot(&input_f16, QuantizationMode::GgmlQ8_0, row_b.as_slice())?,
        ];
        let transposed = decode_quantized_matrix_bytes_transposed_f16(
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            34,
            weights.as_slice(),
            "test",
        )?;

        let mut left = backend.f16_buffer(32)?;
        left.write_bytes(f32_slice_to_f16_bytes(input.as_slice()).as_slice())?;
        let right = backend.byte_buffer(transposed.as_slice())?;
        let output = backend.f32_buffer(2)?;

        let mut submission = backend.begin_submission()?;
        submission.matmul_f16_to_f32(&left, &right, &output, 1, 32, 2)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);

        let actual = output.read_f32()?;
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            assert!(
                (actual - expected).abs() <= 1.0e-7,
                "expected {expected}, actual {actual}",
            );
        }
        Ok(())
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }

    fn align_usize(value: usize, alignment: usize) -> usize {
        if alignment == 0 {
            return value;
        }
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + alignment - remainder
        }
    }

    fn sample_q8_0_row(scale: f32, multiplier: i8) -> Vec<u8> {
        std::iter::once(f32_to_f16_bits(scale).to_le_bytes()[0])
            .chain(std::iter::once(f32_to_f16_bits(scale).to_le_bytes()[1]))
            .chain((1_i8..=32).map(|value| value.saturating_mul(multiplier).to_le_bytes()[0]))
            .collect()
    }
}
