use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use psionic_adapters::{
    AdapterArtifactIdentity, AdapterResidencyMode, AdapterServingBinding, LmHeadLoraAdapterArtifact,
};
use psionic_backend_cpu::{
    CpuBackend, decode_quantized_row_into, quantized_row_byte_len, quantized_row_dot,
};
use psionic_backend_cuda::{
    CudaAllocatorPoolTelemetry, CudaBackend, CudaBuffer, CudaDeviceMemoryInfo,
    CudaGemmTuningReport, CudaGemmTuningScope, CudaGraphExec, CudaHostBuffer,
    CudaQuantizedMatvecStats, CudaSubmission, ggml_q8_1_storage_bytes,
};
use psionic_backend_metal::{
    MetalBackend, MetalBuffer, MetalLogitsOutputMode, MetalQuantizedMatvecRequest,
    MetalTextGenerationRuntimePolicy,
};
use psionic_catalog::{BlobIntegrityPolicy, LocalBlobOpenOptions};
use psionic_core::QuantizationMode;
use psionic_models::{
    DecoderModelDescriptor, GgufBlobArtifact, GgufDecoderAdapterLoader, GgufDecoderFamily,
    GgufDecoderFamilyMetadata, GgufDecoderLayerKind, GgufMetadataValue, GgufRuntimeTokenizer,
    ModelLoadError, PagedTensorStorage, Qwen38MultimodalDecoderPlan,
    Qwen38MultimodalDecoderPlanReceipt, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::{
    BackendHealthTracker, CacheInvalidationTrigger, DeviceDiscovery, DeviceMemoryBudget,
    LoadedModelResidency, LocalRuntimeObservability, PrefixCacheIdentity, PrefixCacheMode,
    PrefixCacheRefusalReason, PrefixCacheState, SamplingPolicy,
};
use psionic_train::load_qwen38_lm_head_adapter_safetensors;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const QWEN38_CUDA_CONTEXT_LIMIT_TOKENS: usize = 4096;
const QWEN38_CUDA_PREFLIGHT_RUNTIME_RESERVE_BYTES: u64 = 1_834_614_784;
const QWEN38_METAL_CONTEXT_LIMIT_TOKENS: usize = 4096;

use crate::{
    ContinuousBatchGenerationResult, CudaGraphReplayMetrics, CudaGraphReplayMode, DecodeStrategy,
    GenerationEventStream, GenerationInput, GenerationMetrics, GenerationOptions,
    GenerationProvenance, GenerationRequest, GenerationResponse, GenerationStreamChunk,
    GenerationStreamEvent, GenerationStreamStatus, GenerationStreamTerminal,
    GenerationStreamingPolicy, GenerationTerminationDetail, LoadedModelView,
    LoadedModelsObservation, LocalRuntimeDiagnostic, ManagedTextGenerationRuntime,
    PsionRvllmCudaDeviceSurface, PsionRvllmFa3DecodeAttentionBackendKind,
    PsionRvllmFa3DecodeAttentionShape, Qwen35CudaAttentionBackendMetrics,
    Qwen35CudaDecodeOutputMetrics, Qwen35CudaDecodeOutputMode, ReferenceTextGenerationError,
    StreamingTextGenerationExecutor, TerminationReason, TextGenerationExecutor,
    current_time_millis, default_generation_streaming_policy,
    select_psion_rvllm_fa3_decode_attention_backend,
};

/// Stable schema for one opt-in Qwen3.8 MTP execution report.
pub const QWEN38_MTP_EXECUTION_REPORT_SCHEMA_VERSION: &str = "psionic.qwen38.mtp_execution.v1";

pub struct CudaGgufQwen35TextGenerationService {
    backend: CudaBackend,
    backend_selection: psionic_runtime::BackendSelection,
    model: Arc<CudaQwen35Model>,
    step_plan: Qwen35CudaStepPlan,
    shared_prefixes: Qwen35SharedPrefixStore,
    backend_health: BackendHealthTracker,
    residency: LoadedModelResidency,
    memory_plan: psionic_runtime::ModelMemoryPlan,
    residency_policy: psionic_runtime::ModelResidencyPolicy,
    last_multimodal_plan_receipt: Option<Qwen38MultimodalDecoderPlanReceipt>,
}

/// Machine-readable residency and fallback contract for one native Qwen CUDA service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen35CudaRuntimeContract {
    pub family: GgufDecoderFamily,
    pub model_id: String,
    pub artifact_digest: String,
    pub execution_plan_namespace: String,
    pub execution_plan_digest: String,
    pub graph_cache_namespace: String,
    pub graph_cache_identity: String,
    pub context_limit_tokens: usize,
    pub artifact_bytes: u64,
    pub device_capacity_bytes: Option<u64>,
    pub device_free_bytes_at_preflight: Option<u64>,
    pub preflight_required_device_bytes: u64,
    pub preflight_margin_bytes: Option<u64>,
    pub preflight_status: String,
    pub weight_device_bytes: u64,
    pub recurrent_state_bytes: u64,
    pub kv_cache_bytes: u64,
    pub scratch_device_bytes: u64,
    pub planned_device_bytes: u64,
    pub dense_f16_mirror_count: usize,
    pub quantization_modes: Vec<QuantizationMode>,
    pub raw_logits_materialization_observable: bool,
    pub host_fallback_enabled: bool,
}

/// Machine-readable residency and fallback contract for one native Qwen Metal service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen35MetalRuntimeContract {
    pub family: GgufDecoderFamily,
    pub model_id: String,
    pub artifact_digest: String,
    pub execution_plan_namespace: String,
    pub execution_plan_digest: String,
    pub context_limit_tokens: usize,
    pub artifact_bytes: u64,
    pub device_capacity_bytes: Option<u64>,
    pub available_execution_bytes: Option<u64>,
    pub weight_device_bytes: u64,
    pub recurrent_state_host_bytes: u64,
    pub kv_cache_host_bytes: u64,
    pub planned_device_visible_bytes: u64,
    pub admitted_layer_count: usize,
    pub resident_layer_count: usize,
    pub projection_count: usize,
    pub native_projection_count: usize,
    pub admitted_conversion_count: usize,
    pub quantization_modes: Vec<QuantizationMode>,
    pub host_stepped_state: bool,
    pub host_projection_fallback_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Qwen38MetalProjectionRow {
    name: String,
    mode: QuantizationMode,
    byte_length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Qwen38MetalPreflight {
    projection_count: usize,
    projected_device_weight_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Qwen38CudaPreflight {
    required_device_bytes: u64,
    free_device_bytes: u64,
    total_device_bytes: u64,
}

fn qwen38_cuda_preflight(
    artifact_bytes: u64,
    memory: CudaDeviceMemoryInfo,
) -> Result<Qwen38CudaPreflight, ModelLoadError> {
    let required_device_bytes =
        artifact_bytes.saturating_add(QWEN38_CUDA_PREFLIGHT_RUNTIME_RESERVE_BYTES);
    if required_device_bytes > memory.total_bytes {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "qwen38 cuda preflight requires {required_device_bytes} device bytes for the artifact and 4096-token runtime reserve, but the selected device exposes {total} total bytes",
                total = memory.total_bytes,
            ),
        });
    }
    if required_device_bytes > memory.free_bytes {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "qwen38 cuda preflight requires {required_device_bytes} free device bytes for the artifact and 4096-token runtime reserve, but cudaMemGetInfo reports {free} free of {total} total bytes",
                free = memory.free_bytes,
                total = memory.total_bytes,
            ),
        });
    }
    Ok(Qwen38CudaPreflight {
        required_device_bytes,
        free_device_bytes: memory.free_bytes,
        total_device_bytes: memory.total_bytes,
    })
}

fn qwen38_cuda_unsupported_quantization_modes(modes: &[QuantizationMode]) -> Vec<QuantizationMode> {
    modes
        .iter()
        .copied()
        .filter(|mode| {
            !matches!(mode, QuantizationMode::None) && !can_use_cuda_quantized_matvec(*mode)
        })
        .collect()
}

impl CudaGgufQwen35TextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let mut backend = CudaBackend::new();
        if !backend.quantized_kernels_available() {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(String::from(
                    "cuda quantized kernels are unavailable in this build",
                )),
            ));
        }
        let backend_selection = backend
            .backend_selection(&["input", "constant", "quantized_matmul", "rms_norm"])
            .map_err(|error| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                    error.to_string(),
                ))
            })?;
        let mut model = CudaQwen35Model::from_gguf_path(path, &mut backend)?;
        let step_plan = model.build_step_plan(&mut backend)?;
        let _ = model.autotune_cublaslt_plans(&mut backend);
        let context_limit_tokens = model.cuda_context_limit_tokens();
        let (recurrent_state_bytes, kv_cache_bytes) =
            model.planned_state_device_bytes(context_limit_tokens);
        let scratch_device_bytes = step_plan.device_residency_bytes();
        model.memory_plan.kv_cache_bytes = recurrent_state_bytes.saturating_add(kv_cache_bytes);
        model.memory_plan.graph_bytes = scratch_device_bytes;
        model.memory_plan.resident_device_bytes = model
            .weight_device_bytes
            .saturating_add(recurrent_state_bytes)
            .saturating_add(kv_cache_bytes)
            .saturating_add(scratch_device_bytes);
        let model = Arc::new(model);
        let mut backend_health = BackendHealthTracker::default();
        let now_millis = current_time_millis();
        backend_health.observe("cuda", backend.health(), now_millis);
        let residency =
            LoadedModelResidency::ready(now_millis, crate::DEFAULT_MODEL_KEEPALIVE_MILLIS);
        Ok(Self {
            memory_plan: model.memory_plan.clone(),
            residency_policy: psionic_runtime::ModelResidencyPolicy::default(),
            backend,
            backend_selection,
            model,
            step_plan,
            shared_prefixes: Qwen35SharedPrefixStore::default(),
            backend_health,
            residency,
            last_multimodal_plan_receipt: None,
        })
    }

    /// Returns the admitted decoder-input plan from the most recent native
    /// Qwen3.8 multimodal generation.
    #[must_use]
    pub fn last_qwen38_multimodal_plan_receipt(
        &self,
    ) -> Option<&Qwen38MultimodalDecoderPlanReceipt> {
        self.last_multimodal_plan_receipt.as_ref()
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model.descriptor
    }

    #[must_use]
    pub fn runtime_support(&self) -> crate::GgufDecoderRuntimeSupport {
        crate::GgufDecoderRuntimeSupport {
            family: self.model.family_metadata.family,
            supported_backends: vec![String::from("cuda")],
            unsupported_backends: vec![String::from("cpu"), String::from("metal")],
            unsupported_features: qwen35_unsupported_features(&self.model.family_metadata, true),
            quantization_modes: self.model.descriptor.weights.quantization_modes.clone(),
            adapter_runtime: crate::DecoderAdapterRuntimeSupport {
                support_level: String::from("unsupported"),
                import_formats: Vec::new(),
                residency_modes: Vec::new(),
                batching_mode: String::from("not_available"),
                unsupported_reasons: vec![String::from(
                    "LM-head LoRA serving is not implemented on the native qwen35 cuda runtime",
                )],
            },
        }
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        (model_id == self.model.descriptor.model.model_id)
            .then_some(self.model.plan_digest.as_str())
    }

    #[must_use]
    pub fn cuda_gemm_tuning_report(&self) -> Option<CudaGemmTuningReport> {
        self.backend.cuda_gemm_tuning_report()
    }

    /// Reports exact Psionic-owned CUDA allocation counts and byte high-water marks.
    #[must_use]
    pub fn cuda_allocator_pool_telemetry(&self) -> Option<CudaAllocatorPoolTelemetry> {
        self.backend.allocator_pool_telemetry()
    }

    #[must_use]
    pub fn cuda_runtime_contract(&self) -> Qwen35CudaRuntimeContract {
        let context_limit_tokens = self.model.cuda_context_limit_tokens();
        let (recurrent_state_bytes, kv_cache_bytes) =
            self.model.planned_state_device_bytes(context_limit_tokens);
        let scratch_device_bytes = self.step_plan.device_residency_bytes();
        let weight_device_bytes = self.model.weight_device_bytes;
        let artifact_bytes = self.model.memory_plan.weights_bytes;
        let device_capacity_bytes = self
            .model
            .qwen38_cuda_preflight
            .map(|row| row.total_device_bytes);
        let device_free_bytes_at_preflight = self
            .model
            .qwen38_cuda_preflight
            .map(|row| row.free_device_bytes);
        let preflight_required_device_bytes = self
            .model
            .qwen38_cuda_preflight
            .map(|row| row.required_device_bytes)
            .unwrap_or(0);
        Qwen35CudaRuntimeContract {
            family: self.model.family_metadata.family,
            model_id: self.model.descriptor.model.model_id.clone(),
            artifact_digest: self.model.descriptor.weights.digest.clone(),
            execution_plan_namespace: String::from(match self.model.family_metadata.family {
                GgufDecoderFamily::Qwen38 => "qwen38-native-cuda|v1",
                _ => "qwen35-native-cuda|v1",
            }),
            execution_plan_digest: self.model.plan_digest.clone(),
            graph_cache_namespace: String::from(match self.model.family_metadata.family {
                GgufDecoderFamily::Qwen38 => "qwen38-cuda-graph-cache|v1",
                _ => "qwen35-cuda-graph-cache|v1",
            }),
            graph_cache_identity: self.model.graph_cache_identity.clone(),
            context_limit_tokens,
            artifact_bytes,
            device_capacity_bytes,
            device_free_bytes_at_preflight,
            preflight_required_device_bytes,
            preflight_margin_bytes: device_free_bytes_at_preflight
                .map(|capacity| capacity.saturating_sub(preflight_required_device_bytes)),
            preflight_status: if self.model.qwen38_cuda_preflight.is_some() {
                String::from("admitted_before_weight_upload")
            } else {
                String::from("not_required_for_qwen35")
            },
            weight_device_bytes,
            recurrent_state_bytes,
            kv_cache_bytes,
            scratch_device_bytes,
            planned_device_bytes: weight_device_bytes
                .saturating_add(recurrent_state_bytes)
                .saturating_add(kv_cache_bytes)
                .saturating_add(scratch_device_bytes),
            dense_f16_mirror_count: self.model.dense_f16_mirror_count(),
            quantization_modes: self.model.descriptor.weights.quantization_modes.clone(),
            raw_logits_materialization_observable: true,
            host_fallback_enabled: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_raw_logits(
        &mut self,
        tokens: &TokenSequence,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let mut state = self
            .model
            .initial_state(&mut self.backend, self.model.cuda_context_limit_tokens())?;
        let options = GenerationOptions::sample(1);
        let mut logits = Vec::new();
        for token in tokens.as_slice() {
            logits = self
                .model
                .forward_token(
                    &mut self.backend,
                    &mut self.step_plan,
                    &mut state,
                    *token,
                    CudaStepOutputMode::FullLogits,
                    &options,
                    &[],
                )?
                .logits;
        }
        Ok(logits)
    }

    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        vec![self.loaded_model_view()]
    }

    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        LoadedModelsObservation::new(vec![self.loaded_model_view().summary])
    }

    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.backend_health
            .observe("cuda", self.backend.health(), current_time_millis());
        LocalRuntimeObservability {
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: crate::cache_invalidation_policy(),
            execution_profile: crate::default_text_generation_execution_profile(),
            queue_depth: 0,
            queue_capacity: Some(
                crate::default_text_generation_execution_profile()
                    .queue_policy
                    .max_queued_requests,
            ),
            active_sessions: 0,
            active_requests: self.residency.active_requests,
            memory_footprint: self.residency_snapshot(),
            backend_health: self.backend_health.snapshot(),
            recent_transitions: self.backend_health.recent_changes(),
        }
    }

    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency
            .refresh_keep_alive(keep_alive_millis, current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency.expire_now(current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        let responses = requests
            .iter()
            .map(|request| self.generate(request))
            .collect::<Vec<_>>();
        ContinuousBatchGenerationResult {
            responses,
            scheduler_metrics: psionic_runtime::GenerationSchedulerMetrics::default(),
        }
    }

    fn loaded_model_view(&self) -> LoadedModelView {
        let mut summary = crate::LoadedModelSummary::from_decoder_descriptor(
            self.model.descriptor.model.model_id.clone(),
            &self.model.descriptor,
        );
        summary.size_bytes = Some(self.memory_plan.weights_bytes);
        summary.size_vram_bytes = Some(self.memory_plan.resident_device_bytes);
        summary.backend = Some(String::from("cuda"));
        summary.fallback_state = crate::backend_selection_fallback_state(&self.backend_selection);
        LoadedModelView {
            summary,
            residency: self.residency.clone(),
            memory_plan: self.memory_plan.clone(),
            residency_policy: self.residency_policy.clone(),
            residency_snapshot: self.residency_snapshot(),
        }
    }

    fn residency_snapshot(&self) -> psionic_runtime::MemoryResidencySnapshot {
        psionic_runtime::MemoryResidencySnapshot::from_loaded_models(&[
            psionic_runtime::LoadedModelMemoryState {
                model_id: self.model.descriptor.model.model_id.clone(),
                plan: self.memory_plan.clone(),
                active_requests: self.residency.active_requests,
                last_used_at_millis: self.residency.last_used_at_millis,
            },
        ])
    }
}

impl TextGenerationExecutor for CudaGgufQwen35TextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        self.generate_with_qwen38_multimodal_plan(request, None)
    }
}

impl CudaGgufQwen35TextGenerationService {
    fn generate_with_qwen38_multimodal_plan(
        &mut self,
        request: &GenerationRequest,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        if request.product_id != crate::TEXT_GENERATION_PRODUCT_ID {
            return Err(ReferenceTextGenerationError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != self.model.descriptor {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.session_id.is_some() || request.reset_session {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native qwen35 cuda runtime does not implement session reuse yet",
                )),
            ));
        }
        if request.adapter_serving.is_some() {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: request
                    .adapter_serving
                    .as_ref()
                    .map(|binding| binding.binding_id.clone())
                    .unwrap_or_else(|| String::from("unknown")),
                reason: String::from(
                    "LM-head LoRA serving is not implemented on the native qwen35 cuda runtime",
                ),
            });
        }
        if multimodal_plan.is_some()
            && self.model.family_metadata.family != GgufDecoderFamily::Qwen38
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native multimodal decoder plans require a qwen38 artifact",
                )),
            ));
        }
        let generation_start = Instant::now();
        self.residency.begin_request(current_time_millis());
        let result = self.generate_inner(request, multimodal_plan);
        self.residency.finish_request(current_time_millis());
        self.last_multimodal_plan_receipt = result
            .as_ref()
            .ok()
            .and_then(|_| multimodal_plan.map(|plan| plan.receipt().clone()));
        result.map(|mut response| {
            response.metrics.total_duration_ns = Some(
                generation_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            response
        })
    }

    /// Runs one native Qwen3.8 CUDA generation with admitted vision
    /// embeddings and explicit multimodal rotary positions.
    pub fn generate_qwen38_multimodal(
        &mut self,
        request: &GenerationRequest,
        plan: &Qwen38MultimodalDecoderPlan,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_qwen38_multimodal_plan(request, Some(plan))
    }
}

impl StreamingTextGenerationExecutor for CudaGgufQwen35TextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, ReferenceTextGenerationError> {
        let response = self.generate(request)?;
        Ok(Box::new(CompletedQwen35Stream::new(response, "cuda")))
    }
}

impl ManagedTextGenerationRuntime for CudaGgufQwen35TextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        Self::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        Self::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::unload_model(self, model_id)
    }
}

impl CudaGgufQwen35TextGenerationService {
    fn generate_inner(
        &mut self,
        request: &GenerationRequest,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.step_plan.reset_graph_replay_state();
        let prompt_eval_start = Instant::now();
        let prompt_tokens = match multimodal_plan {
            Some(plan) => TokenSequence::new(
                plan.token_ids()
                    .iter()
                    .copied()
                    .map(TokenId)
                    .collect::<Vec<_>>(),
            ),
            None => match &request.prompt {
                GenerationInput::Text(text) => self.model.tokenizer.encode_with_defaults(text),
                GenerationInput::Tokens(tokens) => tokens.clone(),
            },
        };
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let context_limit_tokens = self.model.cuda_context_limit_tokens();
        let (prompt_tokens, context_window) = psionic_models::apply_context_window(
            &prompt_tokens,
            context_limit_tokens,
            0,
            request.options.max_output_tokens,
            request.options.context_overflow_policy,
            usize::from(
                prompt_tokens.as_slice().first().copied()
                    == Some(self.model.tokenizer.vocabulary().bos_id()),
            ),
        )?;
        if multimodal_plan.is_some_and(|plan| prompt_tokens.len() != plan.token_ids().len()) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 CUDA multimodal generation refuses context-window prompt truncation",
                )),
            ));
        }

        let output_mode = qwen35_cuda_output_mode(&request.options);
        let prefix_policy = crate::default_prefix_cache_policy();
        let compatibility =
            qwen35_prefix_compatibility_for_request(&self.model.descriptor, request);
        let prefix_lookup = if multimodal_plan.is_some() {
            let mut lookup = Qwen35SharedPrefixStore::empty_lookup(PrefixCacheState::Bypassed);
            lookup.refusal_reason = Some(PrefixCacheRefusalReason::RequestOptOut);
            lookup
        } else {
            self.shared_prefixes.controlled_lookup(
                &compatibility,
                &prompt_tokens,
                output_mode,
                request,
            )
        };
        let prefix_state = prefix_lookup.state;
        let prefix_cache_refusal_reason = prefix_lookup.refusal_reason;
        let prefix_cache_invalidation_trigger = prefix_lookup.invalidation_trigger;
        let mut prefix_tokens_reused = prefix_lookup.reused_tokens;
        let mut prefix_identity = prefix_lookup.identity;
        let cache_capacity_tokens = qwen35_cache_capacity_tokens(
            prompt_tokens.len(),
            request.options.max_output_tokens,
            context_limit_tokens,
        );
        let mut state = if let Some(entry) = prefix_lookup.entry.as_ref() {
            entry.state.deep_clone(&mut self.backend)?
        } else {
            self.model
                .initial_state(&mut self.backend, cache_capacity_tokens)?
        };
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut last_logits = Vec::new();
        let mut pending_selected_token = prefix_lookup
            .entry
            .as_ref()
            .and_then(|entry| entry.pending_selected_token);
        let mut last_candidates = prefix_lookup
            .entry
            .as_ref()
            .and_then(|entry| entry.last_candidates.clone());
        if let Some(entry) = prefix_lookup.entry.as_ref() {
            last_logits = entry.last_logits.clone();
        }
        let mut decode_output_metrics = Qwen35CudaDecodeOutputMetrics::default();
        let embedding_overrides = multimodal_plan
            .map(|plan| {
                plan.embedding_overrides()
                    .iter()
                    .map(|value| (value.token_index, value.embedding.as_slice()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();

        let prompt_suffix = &prompt_tokens.as_slice()[prefix_tokens_reused..];
        if matches!(
            output_mode,
            CudaStepOutputMode::ArgmaxOnly | CudaStepOutputMode::TopKCandidates(_)
        ) {
            if !prompt_suffix.is_empty() {
                let (last_prompt_token, prompt_prefix) = prompt_suffix
                    .split_last()
                    .expect("validated non-empty prompt suffix");
                for (suffix_index, token) in prompt_prefix.iter().enumerate() {
                    let token_index = prefix_tokens_reused.saturating_add(suffix_index);
                    let mrope_position = multimodal_plan
                        .map(|plan| plan.mrope_position_ids()[token_index])
                        .unwrap_or([state.position; 3]);
                    let step = self.model.forward_token_with_embedding_and_mrope(
                        &mut self.backend,
                        &mut self.step_plan,
                        &mut state,
                        *token,
                        embedding_overrides.get(&token_index).copied(),
                        mrope_position,
                        CudaStepOutputMode::NoOutput,
                        &request.options,
                        &[],
                    )?;
                    kernel_count = kernel_count.saturating_add(step.kernel_count);
                    bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                }
                let token_index = prefix_tokens_reused.saturating_add(prompt_prefix.len());
                let mrope_position = multimodal_plan
                    .map(|plan| plan.mrope_position_ids()[token_index])
                    .unwrap_or([state.position; 3]);
                let step = self.model.forward_token_with_embedding_and_mrope(
                    &mut self.backend,
                    &mut self.step_plan,
                    &mut state,
                    *last_prompt_token,
                    embedding_overrides.get(&token_index).copied(),
                    mrope_position,
                    output_mode,
                    &request.options,
                    &[],
                )?;
                pending_selected_token = step.selected_token;
                last_logits = step.logits;
                last_candidates = step.candidates;
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                accumulate_qwen35_decode_output_metrics(
                    &mut decode_output_metrics,
                    step.output_metrics.as_ref(),
                );
            }
        } else if !prompt_suffix.is_empty() {
            for (suffix_index, token) in prompt_suffix.iter().enumerate() {
                let token_index = prefix_tokens_reused.saturating_add(suffix_index);
                let mrope_position = multimodal_plan
                    .map(|plan| plan.mrope_position_ids()[token_index])
                    .unwrap_or([state.position; 3]);
                let step = self.model.forward_token_with_embedding_and_mrope(
                    &mut self.backend,
                    &mut self.step_plan,
                    &mut state,
                    *token,
                    embedding_overrides.get(&token_index).copied(),
                    mrope_position,
                    CudaStepOutputMode::FullLogits,
                    &request.options,
                    &[],
                )?;
                last_logits = step.logits;
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                accumulate_qwen35_decode_output_metrics(
                    &mut decode_output_metrics,
                    step.output_metrics.as_ref(),
                );
            }
        }

        if crate::prefix_recording_allowed(request)
            && prompt_suffix.is_empty()
            && prefix_state == PrefixCacheState::Hit
        {
            prefix_tokens_reused = prompt_tokens.len();
        }
        let mut recordable_prefix = (multimodal_plan.is_none()
            && crate::prefix_recording_allowed(request)
            && !prompt_tokens.is_empty())
        .then(|| -> Result<_, ReferenceTextGenerationError> {
            Ok((
                state.deep_clone(&mut self.backend)?,
                last_logits.clone(),
                pending_selected_token,
                last_candidates.clone(),
            ))
        })
        .transpose()?;

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let mut sampler = crate::GenerationSampler::new(&request.options)?;
        let structured_output_report = sampler.structured_output_report();
        let mut generated_tokens = Vec::new();
        let mut generated_text_terminated = None;
        let first_token_started = Instant::now();
        let mut first_token_emitted_at = None;
        let mut last_token_emitted_at = None;

        let (termination, termination_detail) = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break (
                    TerminationReason::MaxOutputTokens,
                    Some(GenerationTerminationDetail::max_output_tokens()),
                );
            }
            if prompt_tokens.len().saturating_add(generated_tokens.len())
                >= self.model.descriptor.config.max_context
            {
                break (
                    TerminationReason::ContextLimit,
                    Some(GenerationTerminationDetail::context_limit()),
                );
            }

            let next_token = if generated_tokens.is_empty() {
                if let Some(selected) = pending_selected_token.take() {
                    selected
                } else if matches!(output_mode, CudaStepOutputMode::ArgmaxOnly) {
                    let step = self.model.forward_generated_token(
                        &mut self.backend,
                        &mut self.step_plan,
                        &mut state,
                        *generated_tokens
                            .last()
                            .expect("generated token should exist"),
                        CudaStepOutputMode::ArgmaxOnly,
                        &request.options,
                        generated_tokens.as_slice(),
                        multimodal_plan,
                    )?;
                    kernel_count = kernel_count.saturating_add(step.kernel_count);
                    bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                    accumulate_qwen35_decode_output_metrics(
                        &mut decode_output_metrics,
                        step.output_metrics.as_ref(),
                    );
                    let selected_token = step.selected_token.ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                            String::from("qwen35 argmax decode did not return a selected token"),
                        ))
                    })?;
                    if request.options.structured_output.is_some() {
                        let structured_candidate_selection = sampler
                            .select_greedy_structured_token_from_candidates(
                                &self.model.tokenizer,
                                &[selected_token.as_u32()],
                                &[0.0_f32],
                                generated_tokens.as_slice(),
                            )?;
                        match structured_candidate_selection {
                            Some(crate::GenerationSelection::Token(token)) => token,
                            Some(crate::GenerationSelection::Terminate) => {
                                break (
                                    TerminationReason::EndOfSequence,
                                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                                );
                            }
                            None => {
                                let allowed_token_ids = sampler
                                    .structured_output_allowed_token_ids_for_generated_tokens(
                                        &self.model.tokenizer,
                                        generated_tokens.as_slice(),
                                    )?;
                                if allowed_token_ids.is_empty() {
                                    return Err(
                                        ReferenceTextGenerationError::StructuredOutputExhausted,
                                    );
                                }
                                let (allowed_logits, stats) = self
                                    .step_plan
                                    .gather_sparse_logits_from_current_output(
                                        &mut self.backend,
                                        allowed_token_ids.as_slice(),
                                        self.model.descriptor.config.vocab_size,
                                    )
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                let sparse_kernel_launches = stats.kernel_launches;
                                let sparse_readback_bytes = stats.device_to_host_bytes;
                                let sparse_bytes_moved = cuda_stats_bytes(stats);
                                kernel_count = kernel_count.saturating_add(sparse_kernel_launches);
                                bytes_moved = bytes_moved.saturating_add(sparse_bytes_moved);
                                accumulate_qwen35_decode_output_metrics(
                                    &mut decode_output_metrics,
                                    Some(&qwen35_decode_output_metrics(
                                        Qwen35CudaDecodeOutputMode::SparseLogits {
                                            token_count: allowed_token_ids.len(),
                                        },
                                        sparse_readback_bytes,
                                        false,
                                        None,
                                    )),
                                );
                                match sampler.select_next_token_from_exact_candidates(
                                    allowed_token_ids.as_slice(),
                                    allowed_logits.as_slice(),
                                    self.model.descriptor.config.vocab_size,
                                )? {
                                    crate::GenerationSelection::Token(token) => token,
                                    crate::GenerationSelection::Terminate => {
                                        break (
                                            TerminationReason::EndOfSequence,
                                            Some(
                                                GenerationTerminationDetail::end_of_sequence_token(
                                                ),
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        selected_token
                    }
                } else if let CudaStepOutputMode::TopKCandidates(top_k) = output_mode {
                    {
                        if !generated_tokens.is_empty() {
                            let step = self.model.forward_generated_token(
                                &mut self.backend,
                                &mut self.step_plan,
                                &mut state,
                                *generated_tokens
                                    .last()
                                    .expect("generated token should exist"),
                                CudaStepOutputMode::TopKCandidates(top_k),
                                &request.options,
                                generated_tokens.as_slice(),
                                multimodal_plan,
                            )?;
                            last_candidates = step.candidates;
                            kernel_count = kernel_count.saturating_add(step.kernel_count);
                            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                            accumulate_qwen35_decode_output_metrics(
                                &mut decode_output_metrics,
                                step.output_metrics.as_ref(),
                            );
                        }
                        let candidates = last_candidates.as_ref().ok_or_else(|| {
                            ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                                String::from(
                                    "qwen35 bounded candidate decode did not return candidates",
                                ),
                            ))
                        })?;
                        let structured_candidate_selection =
                            if request.options.structured_output.is_some()
                                && (matches!(
                                    request.options.decode_strategy,
                                    crate::DecodeStrategy::Greedy
                                ) || request.options.sampling_policy().effective_temperature()
                                    <= 1e-6
                                    || request.options.sampling_policy().effective_top_k()
                                        == Some(1))
                            {
                                sampler.select_greedy_structured_token_from_candidates(
                                    &self.model.tokenizer,
                                    candidates.indices(),
                                    candidates.values(),
                                    generated_tokens.as_slice(),
                                )?
                            } else {
                                None
                            };
                        match structured_candidate_selection {
                            Some(crate::GenerationSelection::Token(token)) => token,
                            Some(crate::GenerationSelection::Terminate) => {
                                break (
                                    TerminationReason::EndOfSequence,
                                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                                );
                            }
                            None if request.options.structured_output.is_some() => {
                                let allowed_token_ids = sampler
                                    .structured_output_allowed_token_ids_for_generated_tokens(
                                        &self.model.tokenizer,
                                        generated_tokens.as_slice(),
                                    )?;
                                if allowed_token_ids.is_empty() {
                                    return Err(
                                        ReferenceTextGenerationError::StructuredOutputExhausted,
                                    );
                                }
                                let (allowed_logits, stats) = self
                                    .step_plan
                                    .gather_sparse_logits_from_current_output(
                                        &mut self.backend,
                                        allowed_token_ids.as_slice(),
                                        self.model.descriptor.config.vocab_size,
                                    )
                                    .map_err(ReferenceTextGenerationError::Runtime)?;
                                let sparse_kernel_launches = stats.kernel_launches;
                                let sparse_readback_bytes = stats.device_to_host_bytes;
                                let sparse_bytes_moved = cuda_stats_bytes(stats);
                                kernel_count = kernel_count.saturating_add(sparse_kernel_launches);
                                bytes_moved = bytes_moved.saturating_add(sparse_bytes_moved);
                                accumulate_qwen35_decode_output_metrics(
                                    &mut decode_output_metrics,
                                    Some(&qwen35_decode_output_metrics(
                                        Qwen35CudaDecodeOutputMode::SparseLogits {
                                            token_count: allowed_token_ids.len(),
                                        },
                                        sparse_readback_bytes,
                                        false,
                                        None,
                                    )),
                                );
                                match sampler.select_next_token_from_exact_candidates(
                                    allowed_token_ids.as_slice(),
                                    allowed_logits.as_slice(),
                                    self.model.descriptor.config.vocab_size,
                                )? {
                                    crate::GenerationSelection::Token(token) => token,
                                    crate::GenerationSelection::Terminate => {
                                        break (
                                            TerminationReason::EndOfSequence,
                                            Some(
                                                GenerationTerminationDetail::end_of_sequence_token(
                                                ),
                                            ),
                                        );
                                    }
                                }
                            }
                            None => match sampler.select_next_token_from_presorted_candidates(
                                candidates.indices(),
                                candidates.values(),
                                self.model.descriptor.config.vocab_size,
                            )? {
                                crate::GenerationSelection::Token(token) => token,
                                crate::GenerationSelection::Terminate => {
                                    break (
                                        TerminationReason::EndOfSequence,
                                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                                    );
                                }
                            },
                        }
                    }
                } else {
                    match sampler.select_next_token(
                        &self.model.tokenizer,
                        &last_logits,
                        &crate::InMemoryKvCache::new(1, 1),
                        generated_tokens.as_slice(),
                    )? {
                        crate::GenerationSelection::Token(token) => token,
                        crate::GenerationSelection::Terminate => {
                            break (
                                TerminationReason::EndOfSequence,
                                Some(GenerationTerminationDetail::end_of_sequence_token()),
                            );
                        }
                    }
                }
            } else if matches!(output_mode, CudaStepOutputMode::ArgmaxOnly) {
                let step = self.model.forward_generated_token(
                    &mut self.backend,
                    &mut self.step_plan,
                    &mut state,
                    *generated_tokens
                        .last()
                        .expect("generated token should exist"),
                    CudaStepOutputMode::ArgmaxOnly,
                    &request.options,
                    generated_tokens.as_slice(),
                    multimodal_plan,
                )?;
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                accumulate_qwen35_decode_output_metrics(
                    &mut decode_output_metrics,
                    step.output_metrics.as_ref(),
                );
                let selected_token = step.selected_token.ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                        String::from("qwen35 argmax decode did not return a selected token"),
                    ))
                })?;
                if request.options.structured_output.is_some() {
                    let structured_candidate_selection = sampler
                        .select_greedy_structured_token_from_candidates(
                            &self.model.tokenizer,
                            &[selected_token.as_u32()],
                            &[0.0_f32],
                            generated_tokens.as_slice(),
                        )?;
                    match structured_candidate_selection {
                        Some(crate::GenerationSelection::Token(token)) => token,
                        Some(crate::GenerationSelection::Terminate) => {
                            break (
                                TerminationReason::EndOfSequence,
                                Some(GenerationTerminationDetail::end_of_sequence_token()),
                            );
                        }
                        None => {
                            let allowed_token_ids = sampler
                                .structured_output_allowed_token_ids_for_generated_tokens(
                                    &self.model.tokenizer,
                                    generated_tokens.as_slice(),
                                )?;
                            if allowed_token_ids.is_empty() {
                                return Err(
                                    ReferenceTextGenerationError::StructuredOutputExhausted,
                                );
                            }
                            let (allowed_logits, stats) = self
                                .step_plan
                                .gather_sparse_logits_from_current_output(
                                    &mut self.backend,
                                    allowed_token_ids.as_slice(),
                                    self.model.descriptor.config.vocab_size,
                                )
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            let sparse_kernel_launches = stats.kernel_launches;
                            let sparse_readback_bytes = stats.device_to_host_bytes;
                            let sparse_bytes_moved = cuda_stats_bytes(stats);
                            kernel_count = kernel_count.saturating_add(sparse_kernel_launches);
                            bytes_moved = bytes_moved.saturating_add(sparse_bytes_moved);
                            accumulate_qwen35_decode_output_metrics(
                                &mut decode_output_metrics,
                                Some(&qwen35_decode_output_metrics(
                                    Qwen35CudaDecodeOutputMode::SparseLogits {
                                        token_count: allowed_token_ids.len(),
                                    },
                                    sparse_readback_bytes,
                                    false,
                                    None,
                                )),
                            );
                            match sampler.select_next_token_from_exact_candidates(
                                allowed_token_ids.as_slice(),
                                allowed_logits.as_slice(),
                                self.model.descriptor.config.vocab_size,
                            )? {
                                crate::GenerationSelection::Token(token) => token,
                                crate::GenerationSelection::Terminate => {
                                    break (
                                        TerminationReason::EndOfSequence,
                                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                                    );
                                }
                            }
                        }
                    }
                } else {
                    selected_token
                }
            } else if let CudaStepOutputMode::TopKCandidates(top_k) = output_mode {
                {
                    if !generated_tokens.is_empty() {
                        let step = self.model.forward_generated_token(
                            &mut self.backend,
                            &mut self.step_plan,
                            &mut state,
                            *generated_tokens
                                .last()
                                .expect("generated token should exist"),
                            CudaStepOutputMode::TopKCandidates(top_k),
                            &request.options,
                            generated_tokens.as_slice(),
                            multimodal_plan,
                        )?;
                        last_candidates = step.candidates;
                        kernel_count = kernel_count.saturating_add(step.kernel_count);
                        bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                        accumulate_qwen35_decode_output_metrics(
                            &mut decode_output_metrics,
                            step.output_metrics.as_ref(),
                        );
                    }
                    let candidates = last_candidates.as_ref().ok_or_else(|| {
                        ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                            String::from(
                                "qwen35 bounded candidate decode did not return candidates",
                            ),
                        ))
                    })?;
                    let structured_candidate_selection =
                        if request.options.structured_output.is_some()
                            && (matches!(
                                request.options.decode_strategy,
                                crate::DecodeStrategy::Greedy
                            ) || request.options.sampling_policy().effective_temperature()
                                <= 1e-6
                                || request.options.sampling_policy().effective_top_k() == Some(1))
                        {
                            sampler.select_greedy_structured_token_from_candidates(
                                &self.model.tokenizer,
                                candidates.indices(),
                                candidates.values(),
                                generated_tokens.as_slice(),
                            )?
                        } else {
                            None
                        };
                    match structured_candidate_selection {
                        Some(crate::GenerationSelection::Token(token)) => token,
                        Some(crate::GenerationSelection::Terminate) => {
                            break (
                                TerminationReason::EndOfSequence,
                                Some(GenerationTerminationDetail::end_of_sequence_token()),
                            );
                        }
                        None if request.options.structured_output.is_some() => {
                            let allowed_token_ids = sampler
                                .structured_output_allowed_token_ids_for_generated_tokens(
                                    &self.model.tokenizer,
                                    generated_tokens.as_slice(),
                                )?;
                            if allowed_token_ids.is_empty() {
                                return Err(
                                    ReferenceTextGenerationError::StructuredOutputExhausted,
                                );
                            }
                            let (allowed_logits, stats) = self
                                .step_plan
                                .gather_sparse_logits_from_current_output(
                                    &mut self.backend,
                                    allowed_token_ids.as_slice(),
                                    self.model.descriptor.config.vocab_size,
                                )
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            let sparse_kernel_launches = stats.kernel_launches;
                            let sparse_readback_bytes = stats.device_to_host_bytes;
                            let sparse_bytes_moved = cuda_stats_bytes(stats);
                            kernel_count = kernel_count.saturating_add(sparse_kernel_launches);
                            bytes_moved = bytes_moved.saturating_add(sparse_bytes_moved);
                            accumulate_qwen35_decode_output_metrics(
                                &mut decode_output_metrics,
                                Some(&qwen35_decode_output_metrics(
                                    Qwen35CudaDecodeOutputMode::SparseLogits {
                                        token_count: allowed_token_ids.len(),
                                    },
                                    sparse_readback_bytes,
                                    false,
                                    None,
                                )),
                            );
                            match sampler.select_next_token_from_exact_candidates(
                                allowed_token_ids.as_slice(),
                                allowed_logits.as_slice(),
                                self.model.descriptor.config.vocab_size,
                            )? {
                                crate::GenerationSelection::Token(token) => token,
                                crate::GenerationSelection::Terminate => {
                                    break (
                                        TerminationReason::EndOfSequence,
                                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                                    );
                                }
                            }
                        }
                        None => match sampler.select_next_token_from_presorted_candidates(
                            candidates.indices(),
                            candidates.values(),
                            self.model.descriptor.config.vocab_size,
                        )? {
                            crate::GenerationSelection::Token(token) => token,
                            crate::GenerationSelection::Terminate => {
                                break (
                                    TerminationReason::EndOfSequence,
                                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                                );
                            }
                        },
                    }
                }
            } else {
                if !generated_tokens.is_empty() {
                    let step = self.model.forward_token(
                        &mut self.backend,
                        &mut self.step_plan,
                        &mut state,
                        *generated_tokens
                            .last()
                            .expect("generated token should exist"),
                        CudaStepOutputMode::FullLogits,
                        &request.options,
                        generated_tokens.as_slice(),
                    )?;
                    last_logits = step.logits;
                    kernel_count = kernel_count.saturating_add(step.kernel_count);
                    bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                    accumulate_qwen35_decode_output_metrics(
                        &mut decode_output_metrics,
                        step.output_metrics.as_ref(),
                    );
                }
                match sampler.select_next_token(
                    &self.model.tokenizer,
                    &last_logits,
                    &crate::InMemoryKvCache::new(1, 1),
                    generated_tokens.as_slice(),
                )? {
                    crate::GenerationSelection::Token(token) => token,
                    crate::GenerationSelection::Terminate => {
                        break (
                            TerminationReason::EndOfSequence,
                            Some(GenerationTerminationDetail::end_of_sequence_token()),
                        );
                    }
                }
            };

            if self.model.tokenizer.is_end_of_sequence(next_token) {
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                );
            }

            if first_token_emitted_at.is_none() {
                first_token_emitted_at = Some(first_token_started.elapsed());
            }
            last_token_emitted_at = Some(first_token_started.elapsed());
            generated_tokens.push(next_token);
            if let Some(stop_hit) = crate::truncate_generated_text_with_match(
                &self.model.tokenizer,
                &mut generated_tokens,
                &request.options.stop_sequences,
            ) {
                generated_text_terminated = Some(TerminationReason::EndOfSequence);
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::stop_sequence(
                        stop_hit.matched_stop_sequence,
                    )),
                );
            }
        };

        if let Some((
            record_state,
            record_last_logits,
            mut record_pending_selected_token,
            record_last_candidates,
        )) = recordable_prefix.take()
        {
            if record_pending_selected_token.is_none() {
                record_pending_selected_token = generated_tokens.first().copied();
            }
            let recorded_identity = self.shared_prefixes.record(
                &mut self.backend,
                compatibility,
                &prompt_tokens,
                output_mode,
                &record_state,
                record_last_logits.as_slice(),
                record_pending_selected_token,
                record_last_candidates.as_ref(),
            );
            if prefix_state != PrefixCacheState::Hit || prefix_identity.is_none() {
                prefix_identity = Some(recorded_identity);
            }
        }

        if !decode_output_metrics.is_zero() {
            decode_output_metrics.graph_cache_identity =
                Some(self.model.graph_cache_identity.clone());
        }
        let generated = TokenSequence::new(generated_tokens);
        let text = self.model.tokenizer.decode(generated.as_slice());
        let metrics = GenerationMetrics {
            total_duration_ns: None,
            load_duration_ns: Some(self.model.load_duration_ns),
            prompt_eval_count: Some(prompt_tokens.len()),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(generated.len()),
            eval_duration_ns: Some(
                first_token_started
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            time_to_first_token_ns: first_token_emitted_at
                .map(|duration| duration.as_nanos().try_into().unwrap_or(u64::MAX)),
            inter_token_latency_ns: average_inter_token_latency_ns(
                first_token_emitted_at,
                last_token_emitted_at,
                generated.len(),
            ),
            kv_cache: None,
            kv_residency: None,
            kv_cache_encoding: None,
            prefix_tokens_reused: Some(prefix_tokens_reused),
            termination_detail,
            gemma4_metal_decode: None,
            qwen35_cuda_decode: (!decode_output_metrics.is_zero()).then_some(decode_output_metrics),
            gpt_oss_perf: None,
        };
        let provenance = GenerationProvenance {
            served_artifact: crate::served_artifact_identity_for_decoder_backend(
                &self.model.descriptor,
                "cuda",
                &[],
            ),
            adapter_serving: None,
            served_revision: None,
            execution_plan_digest: self.model.plan_digest.clone(),
            cluster_execution: None,
            load_state: crate::GenerationLoadState::Warm,
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan: Some(self.memory_plan.clone()),
            residency_policy: Some(self.residency_policy.clone()),
            residency_snapshot: Some(self.residency_snapshot()),
            kv_cache_policy: None,
            kv_cache_encoding_policy: None,
            kv_ownership: None,
            prefix_cache_control: Some(request.prefix_cache_control.clone()),
            prefix_cache_state: Some(prefix_state),
            prefix_cache_refusal_reason,
            prefix_cache_policy: Some(prefix_policy),
            prefix_cache_identity: prefix_identity,
            compile_path: None,
            delivery_proof: Some(psionic_runtime::ExecutionDeliveryProof {
                execution_plan_digest: self.model.plan_digest.clone(),
                kernel_count,
                bytes_moved,
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
            cache_observations: crate::generation_cache_observations(
                &self.model.descriptor,
                None,
                crate::GenerationLoadState::Warm,
                None,
                false,
                &psionic_runtime::KvCacheState::default(),
                prefix_state,
                prefix_cache_invalidation_trigger,
            ),
            scheduler: None,
            structured_output: structured_output_report,
            psion_served_evidence: None,
            psion_served_output_claim_posture: None,
        };
        let structured_output_value = sampler.structured_output_value(text.as_str())?;
        let response = GenerationResponse::new(
            request,
            None,
            generated,
            text,
            prompt_tokens.len(),
            0,
            generated_text_terminated.unwrap_or(termination),
        )
        .with_metrics_and_provenance(metrics, provenance);
        Ok(if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        })
    }
}

/// Bounded configuration for the optional Qwen3.8 NextN draft head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen38MtpConfig {
    /// Maximum draft tokens verified in one target cycle.
    pub max_draft_tokens_per_cycle: usize,
}

impl Qwen38MtpConfig {
    /// Enables the current one-token-per-cycle correctness lane.
    #[must_use]
    pub const fn single_token() -> Self {
        Self {
            max_draft_tokens_per_cycle: 1,
        }
    }
}

/// Machine-readable accounting for one optional Qwen3.8 MTP generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38MtpExecutionReport {
    pub schema_version: String,
    pub backend: String,
    pub enabled: bool,
    pub max_draft_tokens_per_cycle: usize,
    pub draft_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub acceptance_rate: f64,
    pub mtp_forward_count: usize,
    pub mtp_alignment_forward_count: usize,
    pub target_forward_count: usize,
    pub target_replay_count: usize,
    pub rollback_count: usize,
    pub restored_state_parity: bool,
    pub mtp_weight_residency_bytes: u64,
    pub mtp_kv_cache_peak_bytes: u64,
    pub rollback_snapshot_peak_bytes: u64,
    pub added_peak_residency_bytes: u64,
    pub decode_duration_ns: u64,
    pub generated_token_count: usize,
    pub generated_tokens_per_second: f64,
    pub performance_claim: String,
}

#[derive(Clone, Debug, Default)]
struct Qwen38MtpExecutionLedger {
    draft_count: usize,
    accepted_count: usize,
    mtp_forward_count: usize,
    mtp_alignment_forward_count: usize,
    target_forward_count: usize,
    target_replay_count: usize,
    rollback_count: usize,
    restored_state_parity: bool,
    mtp_kv_cache_peak_bytes: u64,
    rollback_snapshot_peak_bytes: u64,
}

fn restore_qwen38_rejected_draft<S, O, E>(
    state: &mut S,
    snapshot: S,
    verified_prefix_state: &S,
    verified_prefix_output: &O,
    replay_prefix: impl FnOnce(&mut S) -> Result<O, E>,
) -> Result<(O, bool), E>
where
    S: PartialEq,
    O: PartialEq,
{
    *state = snapshot;
    let replayed = replay_prefix(state)?;
    let parity = state == verified_prefix_state && &replayed == verified_prefix_output;
    Ok((replayed, parity))
}

#[derive(Clone, Debug)]
pub struct CpuGgufQwen35TextGenerationService {
    backend: CpuBackend,
    model: Arc<CpuQwen35Model>,
    sessions: crate::InMemoryGenerationSessionStore,
    backend_health: BackendHealthTracker,
    residency: LoadedModelResidency,
    memory_plan: psionic_runtime::ModelMemoryPlan,
    residency_policy: psionic_runtime::ModelResidencyPolicy,
    mtp_config: Option<Qwen38MtpConfig>,
    adapters: BTreeMap<String, Qwen38CpuAdapterRuntime>,
    last_mtp_report: Option<Qwen38MtpExecutionReport>,
    last_multimodal_plan_receipt: Option<Qwen38MultimodalDecoderPlanReceipt>,
}

#[derive(Clone, Debug)]
struct Qwen38CpuAdapterRuntime {
    binding: AdapterServingBinding,
    adapter: Arc<LmHeadLoraAdapterArtifact>,
}

impl Qwen38CpuAdapterRuntime {
    fn apply_to_step(
        &self,
        step: &mut CpuQwen35ForwardStep,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.adapter
            .apply_to_logits(step.final_hidden.as_slice(), step.logits.as_mut_slice())
            .map_err(
                |error| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: self.binding.binding_id.clone(),
                    reason: error.to_string(),
                },
            )?;
        step.kernel_count = step.kernel_count.saturating_add(2);
        step.bytes_moved = step
            .bytes_moved
            .saturating_add(self.adapter.identity.parameter_count.saturating_mul(4));
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Qwen35CpuStateAllocationSummary {
    pub(crate) recurrent_layer_count: usize,
    pub(crate) full_attention_layer_count: usize,
    pub(crate) convolution_state_f32: usize,
    pub(crate) delta_state_f32: usize,
    pub(crate) kv_cache_capacity_entries: usize,
}

/// Phase associated with one native CPU recurrent-intermediate observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Qwen35CpuRecurrentTracePhase {
    /// A token supplied as part of the prompt prefill.
    Prefill,
    /// A supplied token evaluated after prefill with the retained recurrent state.
    Decode,
}

impl Qwen35CpuRecurrentTracePhase {
    /// Stable machine-facing phase label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prefill => "prefill",
            Self::Decode => "decode",
        }
    }
}

/// One named tensor captured from the first Qwen3.5-family recurrent CPU layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Qwen35CpuRecurrentTraceTensor {
    /// Prefill or token-at-a-time decode phase.
    pub phase: Qwen35CpuRecurrentTracePhase,
    /// Absolute token position in the retained recurrent state.
    pub position: usize,
    /// Token evaluated at this position.
    pub token_id: TokenId,
    /// llama.cpp-aligned recurrent boundary name.
    pub stage: String,
    /// Canonical GGML-order shape with the fastest-moving dimension first.
    pub shape: [usize; 4],
    /// F32 values in canonical GGML dimension order.
    pub values: Vec<f32>,
}

/// Native CPU trace for the first recurrent layer across prefill and decode.
#[derive(Clone, Debug, PartialEq)]
pub struct Qwen35CpuRecurrentTrace {
    /// Stable model identity bound to this trace.
    pub model_id: String,
    /// Runtime-plan digest bound to this trace.
    pub plan_digest: String,
    /// Traced recurrent layer index. The current contract is layer zero.
    pub layer_index: usize,
    /// Ordered tensor observations.
    pub tensors: Vec<Qwen35CpuRecurrentTraceTensor>,
}

struct Qwen35CpuTraceContext<'a> {
    phase: Qwen35CpuRecurrentTracePhase,
    position: usize,
    token_id: TokenId,
    tensors: &'a mut Vec<Qwen35CpuRecurrentTraceTensor>,
}

fn push_qwen35_cpu_trace_tensor(
    trace: &mut Option<Qwen35CpuTraceContext<'_>>,
    stage: &str,
    shape: [usize; 4],
    values: &[f32],
) {
    let Some(trace) = trace.as_mut() else {
        return;
    };
    trace.tensors.push(Qwen35CpuRecurrentTraceTensor {
        phase: trace.phase,
        position: trace.position,
        token_id: trace.token_id,
        stage: stage.to_string(),
        shape,
        values: values.to_vec(),
    });
}

#[derive(Clone, Copy, Debug)]
struct Qwen35CpuGenerationDeadline {
    started_at: Instant,
    timeout: Duration,
}

impl Qwen35CpuGenerationDeadline {
    fn new(timeout: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            timeout,
        }
    }

    fn check(self) -> Result<(), ReferenceTextGenerationError> {
        let elapsed = self.started_at.elapsed();
        if elapsed < self.timeout {
            return Ok(());
        }
        Err(ReferenceTextGenerationError::TimedOut {
            timeout_millis: duration_millis_saturating(self.timeout),
            elapsed_millis: duration_millis_saturating(elapsed),
        })
    }
}

fn duration_millis_saturating(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

impl CpuGgufQwen35TextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        Self::from_gguf_path_with_optional_qwen38_mtp(path, None)
    }

    /// Loads the appended Qwen3.8 NextN block for bounded CPU speculative decode.
    pub fn from_gguf_path_with_qwen38_mtp(
        path: impl AsRef<Path>,
        config: Qwen38MtpConfig,
    ) -> Result<Self, ReferenceTextGenerationError> {
        if config.max_draft_tokens_per_cycle != 1 {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(format!(
                    "qwen38 CPU MTP currently requires max_draft_tokens_per_cycle = 1, received {}",
                    config.max_draft_tokens_per_cycle,
                )),
            ));
        }
        Self::from_gguf_path_with_optional_qwen38_mtp(path, Some(config))
    }

    fn from_gguf_path_with_optional_qwen38_mtp(
        path: impl AsRef<Path>,
        mtp_config: Option<Qwen38MtpConfig>,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let backend = CpuBackend::new();
        let model = Arc::new(CpuQwen35Model::from_gguf_path(path, mtp_config.is_some())?);
        let now_millis = current_time_millis();
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe("cpu", backend.health(), now_millis);
        Ok(Self {
            backend,
            model: Arc::clone(&model),
            sessions: crate::InMemoryGenerationSessionStore::new(),
            backend_health,
            residency: LoadedModelResidency::ready(
                now_millis,
                crate::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            ),
            memory_plan: model.memory_plan.clone(),
            residency_policy: psionic_runtime::ModelResidencyPolicy::default(),
            mtp_config,
            adapters: BTreeMap::new(),
            last_mtp_report: None,
            last_multimodal_plan_receipt: None,
        })
    }

    /// Returns the MTP accounting from the most recent opt-in generation.
    #[must_use]
    pub fn last_qwen38_mtp_report(&self) -> Option<&Qwen38MtpExecutionReport> {
        self.last_mtp_report.as_ref()
    }

    /// Returns the admitted decoder-input plan from the most recent native
    /// Qwen3.8 multimodal generation.
    #[must_use]
    pub fn last_qwen38_multimodal_plan_receipt(
        &self,
    ) -> Option<&Qwen38MultimodalDecoderPlanReceipt> {
        self.last_multimodal_plan_receipt.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn test_raw_logits(
        &self,
        tokens: &TokenSequence,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let mut state = self
            .model
            .initial_state(self.model.descriptor.config.max_context);
        let mut logits = Vec::new();
        for token in tokens.as_slice() {
            logits = self.model.forward_token(&mut state, *token)?.logits;
        }
        Ok(logits)
    }

    #[cfg(test)]
    pub(crate) fn test_raw_logits_with_embedding_and_mrope(
        &self,
        tokens: &TokenSequence,
        embedding_overrides: &[(usize, Vec<f32>)],
        mrope_positions: &[[usize; 3]],
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let mut state = self
            .model
            .initial_state(self.model.descriptor.config.max_context);
        let mut logits = Vec::new();
        for (index, token) in tokens.as_slice().iter().enumerate() {
            let embedding_override = embedding_overrides
                .iter()
                .find(|(token_index, _)| *token_index == index)
                .map(|(_, embedding)| embedding.as_slice());
            let mrope_position = mrope_positions
                .get(index)
                .copied()
                .unwrap_or([state.position; 3]);
            logits = self
                .model
                .forward_token_with_embedding_and_mrope(
                    &mut state,
                    *token,
                    embedding_override,
                    mrope_position,
                )?
                .logits;
        }
        Ok(logits)
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model.descriptor
    }

    #[must_use]
    pub fn runtime_support(&self) -> crate::GgufDecoderRuntimeSupport {
        let mut unsupported_features =
            qwen35_unsupported_features(&self.model.family_metadata, false);
        if matches!(self.model.family_metadata.family, GgufDecoderFamily::Qwen38) {
            unsupported_features.retain(|feature| feature != "adapter_serving");
        }
        if self.model.mtp.is_some() {
            unsupported_features.retain(|feature| feature != "mtp_speculative_decoding_skipped");
            unsupported_features.push(String::from("mtp_non_greedy_decode"));
            unsupported_features.push(String::from("mtp_structured_output"));
        }
        crate::GgufDecoderRuntimeSupport {
            family: self.model.family_metadata.family,
            supported_backends: vec![String::from("cpu")],
            unsupported_backends: vec![String::from("cuda"), String::from("metal")],
            unsupported_features,
            quantization_modes: self.model.descriptor.weights.quantization_modes.clone(),
            adapter_runtime: if matches!(
                self.model.family_metadata.family,
                GgufDecoderFamily::Qwen38
            ) {
                crate::DecoderAdapterRuntimeSupport {
                    support_level: String::from("qwen38_lm_head_lora_cpu"),
                    import_formats: vec![String::from("safetensors")],
                    residency_modes: vec![String::from("hot_swap_overlay")],
                    batching_mode: String::from("one_explicit_binding_per_request"),
                    unsupported_reasons: vec![String::from(
                        "merged-resident and MTP-composed adapter execution are not implemented",
                    )],
                }
            } else {
                crate::DecoderAdapterRuntimeSupport {
                    support_level: String::from("unsupported"),
                    import_formats: Vec::new(),
                    residency_modes: Vec::new(),
                    batching_mode: String::from("not_available"),
                    unsupported_reasons: vec![String::from(
                        "LM-head LoRA serving is not implemented on the native qwen35 cpu runtime",
                    )],
                }
            },
        }
    }

    pub fn register_qwen38_lm_head_lora_adapter(
        &mut self,
        binding_id: impl Into<String>,
        path: impl AsRef<Path>,
        identity: AdapterArtifactIdentity,
        alpha: f32,
        residency_mode: AdapterResidencyMode,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        let binding_id = binding_id.into();
        if !matches!(self.model.family_metadata.family, GgufDecoderFamily::Qwen38) {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id,
                reason: String::from(
                    "Qwen3.8 LM-head LoRA adapters require a native qwen38 CPU decoder",
                ),
            });
        }
        if residency_mode != AdapterResidencyMode::HotSwapOverlay {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id,
                reason: String::from(
                    "native qwen38 CPU adapter serving currently supports hot-swap overlays only",
                ),
            });
        }
        let bytes = std::fs::read(path.as_ref()).map_err(|error| {
            ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding_id.clone(),
                reason: format!(
                    "failed to read Qwen3.8 adapter `{}`: {error}",
                    path.as_ref().display()
                ),
            }
        })?;
        let adapter = load_qwen38_lm_head_adapter_safetensors(&bytes, identity.clone(), alpha)
            .map_err(
                |error| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: binding_id.clone(),
                    reason: error.to_string(),
                },
            )?;
        if adapter.hidden_size != self.model.descriptor.config.hidden_size
            || adapter.vocab_size != self.model.descriptor.config.vocab_size
        {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id,
                reason: format!(
                    "Qwen3.8 adapter shape hidden={} vocab={} does not match native decoder hidden={} vocab={}",
                    adapter.hidden_size,
                    adapter.vocab_size,
                    self.model.descriptor.config.hidden_size,
                    self.model.descriptor.config.vocab_size,
                ),
            });
        }
        let binding = AdapterServingBinding::new(
            binding_id,
            identity.base_model_id.clone(),
            identity.base_model_revision.clone(),
            identity.base_served_artifact_digest.clone(),
            residency_mode,
            vec![identity],
        );
        self.adapters.insert(
            binding.served_adapter_digest.clone(),
            Qwen38CpuAdapterRuntime {
                binding: binding.clone(),
                adapter: Arc::new(adapter),
            },
        );
        Ok(binding)
    }

    pub fn detach_qwen38_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        self.adapters
            .remove(served_adapter_digest)
            .map(|runtime| runtime.binding)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: served_adapter_digest.to_string(),
                reason: String::from("Qwen3.8 adapter binding is not registered"),
            })
    }

    fn adapter_runtime_for_request(
        &self,
        request: &GenerationRequest,
    ) -> Result<Option<Qwen38CpuAdapterRuntime>, ReferenceTextGenerationError> {
        let Some(binding) = request.adapter_serving.as_ref() else {
            return Ok(None);
        };
        let runtime = self
            .adapters
            .get(binding.served_adapter_digest.as_str())
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding.binding_id.clone(),
                reason: String::from(
                    "Qwen3.8 adapter binding is not registered on this native CPU runtime",
                ),
            })?;
        if runtime.binding != *binding {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding.binding_id.clone(),
                reason: String::from(
                    "Qwen3.8 request binding does not exactly match the registered runtime binding",
                ),
            });
        }
        Ok(Some(runtime.clone()))
    }

    #[cfg(test)]
    pub(crate) fn state_allocation_summary(
        &self,
        cache_capacity_tokens: usize,
    ) -> Qwen35CpuStateAllocationSummary {
        let state = self.model.initial_state(cache_capacity_tokens);
        let mut summary = Qwen35CpuStateAllocationSummary {
            recurrent_layer_count: 0,
            full_attention_layer_count: 0,
            convolution_state_f32: 0,
            delta_state_f32: 0,
            kv_cache_capacity_entries: 0,
        };
        for layer in state.layers {
            match layer {
                CpuQwen35LayerState::Hybrid(state) => {
                    summary.recurrent_layer_count += 1;
                    summary.convolution_state_f32 += state.conv_state.len();
                    summary.delta_state_f32 += state.delta_state.len();
                }
                CpuQwen35LayerState::FullAttention(state) => {
                    summary.full_attention_layer_count += 1;
                    summary.kv_cache_capacity_entries += state.entries.capacity();
                }
            }
        }
        summary
    }

    /// Captures the final hidden state and logits for supplied text.
    ///
    /// This is a diagnostic comparator path and does not perform sampling.
    pub fn final_hidden_and_logits_for_text(
        &self,
        text: &str,
    ) -> Result<(Vec<f32>, Vec<f32>), ReferenceTextGenerationError> {
        let tokens = self.model.tokenizer.encode_with_defaults(text);
        self.final_hidden_and_logits_for_tokens(tokens.as_slice())
    }

    /// Captures the final hidden state and logits for supplied token ids.
    ///
    /// This is a diagnostic comparator path and does not perform sampling.
    pub fn final_hidden_and_logits_for_tokens(
        &self,
        tokens: &[TokenId],
    ) -> Result<(Vec<f32>, Vec<f32>), ReferenceTextGenerationError> {
        if tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let mut state = self.model.initial_state(qwen35_cache_capacity_tokens(
            tokens.len(),
            1,
            self.model.descriptor.config.max_context,
        ));
        let mut final_step = None;
        for token in tokens {
            final_step = Some(self.model.forward_token(&mut state, *token)?);
        }
        let step = final_step.ok_or(ReferenceTextGenerationError::EmptyPrompt)?;
        Ok((step.final_hidden, step.logits))
    }

    /// Captures layer-zero recurrent intermediates for supplied prefill and decode tokens.
    ///
    /// The decode tokens are inputs evaluated after the prompt with the same
    /// convolution and delta state. This is a diagnostic comparator path and
    /// does not perform sampling or execute later trunk layers.
    pub fn trace_first_recurrent_layer(
        &self,
        prompt_tokens: &TokenSequence,
        decode_tokens: &TokenSequence,
    ) -> Result<Qwen35CpuRecurrentTrace, ReferenceTextGenerationError> {
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let layer = self.model.layers.first().ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                "qwen35 cpu recurrent trace requires layer zero",
            )))
        })?;
        let CpuQwen35LayerKind::Hybrid(hybrid) = &layer.kind else {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen35 cpu recurrent trace requires recurrent layer zero",
                )),
            ));
        };
        let mut state = CpuQwen35HybridState::new(
            hybrid.qkv_gate_alpha_beta.rows_per_projection[0]
                .saturating_mul(hybrid.conv_kernel.saturating_sub(1)),
            hybrid
                .time_step_rank
                .saturating_mul(hybrid.state_size)
                .saturating_mul(hybrid.state_size),
        );
        let mut tensors = Vec::new();
        let mut position = 0usize;
        for (phase, tokens) in [
            (Qwen35CpuRecurrentTracePhase::Prefill, prompt_tokens),
            (Qwen35CpuRecurrentTracePhase::Decode, decode_tokens),
        ] {
            for token in tokens.as_slice() {
                if token.as_u32() as usize >= self.model.descriptor.config.vocab_size {
                    return Err(ReferenceTextGenerationError::InvalidToken {
                        token: token.as_u32(),
                        vocab_size: self.model.descriptor.config.vocab_size,
                    });
                }
                let hidden = self
                    .model
                    .token_embedding
                    .decode_row(token.as_u32() as usize)?;
                layer.forward_hybrid_attention(
                    &self.model.family_metadata,
                    hybrid,
                    hidden.as_slice(),
                    &mut state,
                    Some(Qwen35CpuTraceContext {
                        phase,
                        position,
                        token_id: *token,
                        tensors: &mut tensors,
                    }),
                )?;
                position = position.saturating_add(1);
            }
        }
        Ok(Qwen35CpuRecurrentTrace {
            model_id: self.model.descriptor.model.model_id.clone(),
            plan_digest: self.model.plan_digest.clone(),
            layer_index: 0,
            tensors,
        })
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        (model_id == self.model.descriptor.model.model_id)
            .then_some(self.model.plan_digest.as_str())
    }

    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        Ok(self.sessions.create(
            &self.model.descriptor,
            crate::served_artifact_identity_for_decoder_backend(&self.model.descriptor, "cpu", &[])
                .served_artifact_digest,
        ))
    }

    pub fn reset_session(
        &mut self,
        session_id: &crate::SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    pub fn close_session(
        &mut self,
        session_id: &crate::SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }

    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        vec![self.loaded_model_view()]
    }

    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        LoadedModelsObservation::new(vec![self.loaded_model_view().summary])
    }

    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.backend_health
            .observe("cpu", self.backend.health(), current_time_millis());
        LocalRuntimeObservability {
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: crate::cache_invalidation_policy(),
            execution_profile: crate::continuous_batch_text_generation_execution_profile(),
            queue_depth: 0,
            queue_capacity: Some(
                crate::continuous_batch_text_generation_execution_profile()
                    .queue_policy
                    .max_queued_requests,
            ),
            active_sessions: self.sessions.len(),
            active_requests: self.residency.active_requests,
            memory_footprint: self.residency_snapshot(),
            backend_health: self.backend_health.snapshot(),
            recent_transitions: self.backend_health.recent_changes(),
        }
    }

    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency
            .refresh_keep_alive(keep_alive_millis, current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency.expire_now(current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        let responses = requests
            .iter()
            .map(|request| self.generate(request))
            .collect::<Vec<_>>();
        ContinuousBatchGenerationResult {
            responses,
            scheduler_metrics: psionic_runtime::GenerationSchedulerMetrics::default(),
        }
    }

    fn loaded_model_view(&self) -> LoadedModelView {
        let mut summary = crate::LoadedModelSummary::from_decoder_descriptor(
            self.model.descriptor.model.model_id.clone(),
            &self.model.descriptor,
        );
        summary.size_bytes = Some(self.memory_plan.weights_bytes);
        summary.size_vram_bytes = Some(0);
        summary.backend = Some(String::from("cpu"));
        LoadedModelView {
            summary,
            residency: self.residency.clone(),
            memory_plan: self.memory_plan.clone(),
            residency_policy: self.residency_policy.clone(),
            residency_snapshot: self.residency_snapshot(),
        }
    }

    fn residency_snapshot(&self) -> psionic_runtime::MemoryResidencySnapshot {
        psionic_runtime::MemoryResidencySnapshot::from_loaded_models(&[
            psionic_runtime::LoadedModelMemoryState {
                model_id: self.model.descriptor.model.model_id.clone(),
                plan: self.memory_plan.clone(),
                active_requests: self.residency.active_requests,
                last_used_at_millis: self.residency.last_used_at_millis,
            },
        ])
    }

    fn generate_inner_with_qwen38_mtp(
        &mut self,
        request: &GenerationRequest,
        deadline: Option<Qwen35CpuGenerationDeadline>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        if request.options.decode_strategy != DecodeStrategy::Greedy {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 CPU MTP currently supports greedy decode only",
                )),
            ));
        }
        if request.options.structured_output.is_some() {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 CPU MTP does not support structured-output sampling",
                )),
            ));
        }
        let config = self.mtp_config.ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::UnsupportedStep(
                String::from("qwen38 CPU MTP generation requires an opt-in MTP configuration"),
            ))
        })?;
        let model = Arc::clone(&self.model);
        let mtp = model.mtp.as_ref().ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::UnsupportedStep(
                String::from("qwen38 CPU MTP generation requires loaded NextN weights"),
            ))
        })?;
        if let Some(deadline) = deadline {
            deadline.check()?;
        }

        let prompt_eval_start = Instant::now();
        let prompt_tokens = match &request.prompt {
            GenerationInput::Text(text) => model.tokenizer.encode_with_defaults(text),
            GenerationInput::Tokens(tokens) => tokens.clone(),
        };
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let (prompt_tokens, context_window) = psionic_models::apply_context_window(
            &prompt_tokens,
            model.descriptor.config.max_context,
            0,
            request.options.max_output_tokens,
            request.options.context_overflow_policy,
            usize::from(
                prompt_tokens.as_slice().first().copied()
                    == Some(model.tokenizer.vocabulary().bos_id()),
            ),
        )?;
        let cache_capacity_tokens = qwen35_cache_capacity_tokens(
            prompt_tokens.len(),
            request.options.max_output_tokens,
            model.descriptor.config.max_context,
        );
        let mut state = model.initial_state(cache_capacity_tokens);
        let mut mtp_state = mtp.initial_state(cache_capacity_tokens);
        let mut history = crate::InMemoryKvCache::new(model.descriptor.config.max_context, 0);
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut last_logits = Vec::new();
        let mut last_hidden = Vec::new();
        for token in prompt_tokens.as_slice() {
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            let step = model.forward_token(&mut state, *token)?;
            history.append(*token, Vec::new(), Vec::new())?;
            last_logits = step.logits;
            last_hidden = step.final_hidden;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let decode_start = Instant::now();
        let mut sampler = crate::GenerationSampler::new(&request.options)?;
        let structured_output_report = sampler.structured_output_report();
        let mut generated_tokens = Vec::new();
        let mut generated_text_terminated = None;
        let mut first_token_emitted_at = None;
        let mut last_token_emitted_at = None;
        let mut ledger = Qwen38MtpExecutionLedger {
            restored_state_parity: true,
            ..Qwen38MtpExecutionLedger::default()
        };

        let (termination, termination_detail) = 'decode: loop {
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            if generated_tokens.len() >= request.options.max_output_tokens {
                break (
                    TerminationReason::MaxOutputTokens,
                    Some(GenerationTerminationDetail::max_output_tokens()),
                );
            }
            if prompt_tokens.len().saturating_add(generated_tokens.len())
                >= model.descriptor.config.max_context
            {
                break (
                    TerminationReason::ContextLimit,
                    Some(GenerationTerminationDetail::context_limit()),
                );
            }

            let next_token = match sampler.select_next_token(
                &model.tokenizer,
                last_logits.as_slice(),
                &history,
                generated_tokens.as_slice(),
            )? {
                crate::GenerationSelection::Token(token) => token,
                crate::GenerationSelection::Terminate => {
                    break (
                        TerminationReason::EndOfSequence,
                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                    );
                }
            };
            if model.tokenizer.is_end_of_sequence(next_token) {
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                );
            }
            if first_token_emitted_at.is_none() {
                first_token_emitted_at = Some(decode_start.elapsed());
            }
            last_token_emitted_at = Some(decode_start.elapsed());
            generated_tokens.push(next_token);
            if let Some(stop_hit) = crate::truncate_generated_text_with_match(
                &model.tokenizer,
                &mut generated_tokens,
                &request.options.stop_sequences,
            ) {
                generated_text_terminated = Some(TerminationReason::EndOfSequence);
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::stop_sequence(
                        stop_hit.matched_stop_sequence,
                    )),
                );
            }
            history.append(next_token, Vec::new(), Vec::new())?;

            let draft_room = generated_tokens.len() < request.options.max_output_tokens
                && prompt_tokens.len().saturating_add(generated_tokens.len())
                    < model.descriptor.config.max_context;
            if !draft_room {
                let step = model.forward_token(&mut state, next_token)?;
                ledger.target_forward_count = ledger.target_forward_count.saturating_add(1);
                last_logits = step.logits;
                last_hidden = step.final_hidden;
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                continue;
            }

            let target_snapshot = state.clone();
            ledger.rollback_snapshot_peak_bytes = ledger
                .rollback_snapshot_peak_bytes
                .max(target_snapshot.allocated_bytes());
            let mtp_step = mtp.forward_draft(
                &model,
                &mut mtp_state,
                last_hidden.as_slice(),
                next_token,
                state.position,
            )?;
            ledger.draft_count = ledger.draft_count.saturating_add(1);
            ledger.mtp_forward_count = ledger.mtp_forward_count.saturating_add(1);
            ledger.mtp_kv_cache_peak_bytes = ledger
                .mtp_kv_cache_peak_bytes
                .max(mtp_state.allocated_bytes());
            kernel_count = kernel_count.saturating_add(mtp_step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(mtp_step.bytes_moved);
            let draft_token = match sampler.select_next_token(
                &model.tokenizer,
                mtp_step.logits.as_slice(),
                &history,
                generated_tokens.as_slice(),
            )? {
                crate::GenerationSelection::Token(token) => token,
                crate::GenerationSelection::Terminate => {
                    let step = model.forward_token(&mut state, next_token)?;
                    ledger.target_forward_count = ledger.target_forward_count.saturating_add(1);
                    last_logits = step.logits;
                    last_hidden = step.final_hidden;
                    kernel_count = kernel_count.saturating_add(step.kernel_count);
                    bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
                    continue;
                }
            };

            let verified_prefix_step = model.forward_token(&mut state, next_token)?;
            ledger.target_forward_count = ledger.target_forward_count.saturating_add(1);
            kernel_count = kernel_count.saturating_add(verified_prefix_step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(verified_prefix_step.bytes_moved);
            let verified_prefix_state = state.clone();
            let verifier_token = match sampler.select_next_token(
                &model.tokenizer,
                verified_prefix_step.logits.as_slice(),
                &history,
                generated_tokens.as_slice(),
            )? {
                crate::GenerationSelection::Token(token) => Some(token),
                crate::GenerationSelection::Terminate => None,
            };
            let speculative_step = model.forward_token(&mut state, draft_token)?;
            ledger.target_forward_count = ledger.target_forward_count.saturating_add(1);
            kernel_count = kernel_count.saturating_add(speculative_step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(speculative_step.bytes_moved);

            if verifier_token == Some(draft_token) {
                ledger.accepted_count = ledger.accepted_count.saturating_add(1);
                let alignment_step = mtp.forward_draft(
                    &model,
                    &mut mtp_state,
                    verified_prefix_step.final_hidden.as_slice(),
                    draft_token,
                    verified_prefix_state.position,
                )?;
                ledger.mtp_forward_count = ledger.mtp_forward_count.saturating_add(1);
                ledger.mtp_alignment_forward_count =
                    ledger.mtp_alignment_forward_count.saturating_add(1);
                ledger.mtp_kv_cache_peak_bytes = ledger
                    .mtp_kv_cache_peak_bytes
                    .max(mtp_state.allocated_bytes());
                kernel_count = kernel_count.saturating_add(alignment_step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(alignment_step.bytes_moved);
                last_logits = speculative_step.logits;
                last_hidden = speculative_step.final_hidden;
                if model.tokenizer.is_end_of_sequence(draft_token) {
                    break 'decode (
                        TerminationReason::EndOfSequence,
                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                    );
                }
                last_token_emitted_at = Some(decode_start.elapsed());
                generated_tokens.push(draft_token);
                history.append(draft_token, Vec::new(), Vec::new())?;
                if let Some(stop_hit) = crate::truncate_generated_text_with_match(
                    &model.tokenizer,
                    &mut generated_tokens,
                    &request.options.stop_sequences,
                ) {
                    generated_text_terminated = Some(TerminationReason::EndOfSequence);
                    break 'decode (
                        TerminationReason::EndOfSequence,
                        Some(GenerationTerminationDetail::stop_sequence(
                            stop_hit.matched_stop_sequence,
                        )),
                    );
                }
            } else {
                ledger.rollback_count = ledger.rollback_count.saturating_add(1);
                let (replay_step, restored_state_parity) = restore_qwen38_rejected_draft(
                    &mut state,
                    target_snapshot,
                    &verified_prefix_state,
                    &verified_prefix_step,
                    |state| model.forward_token(state, next_token),
                )?;
                ledger.target_forward_count = ledger.target_forward_count.saturating_add(1);
                ledger.target_replay_count = ledger.target_replay_count.saturating_add(1);
                ledger.restored_state_parity &= restored_state_parity;
                kernel_count = kernel_count.saturating_add(replay_step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(replay_step.bytes_moved);
                last_logits = replay_step.logits;
                last_hidden = replay_step.final_hidden;
            }
        };

        let decode_duration = decode_start.elapsed();
        let generated = TokenSequence::new(generated_tokens);
        let text = model.tokenizer.decode(generated.as_slice());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(
                prompt_eval_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(model.load_duration_ns),
            prompt_eval_count: Some(prompt_tokens.len()),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(generated.len()),
            eval_duration_ns: Some(decode_duration.as_nanos().try_into().unwrap_or(u64::MAX)),
            time_to_first_token_ns: first_token_emitted_at
                .map(|duration| duration.as_nanos().try_into().unwrap_or(u64::MAX)),
            inter_token_latency_ns: average_inter_token_latency_ns(
                first_token_emitted_at,
                last_token_emitted_at,
                generated.len(),
            ),
            kv_cache: None,
            kv_residency: None,
            kv_cache_encoding: None,
            prefix_tokens_reused: None,
            termination_detail,
            gemma4_metal_decode: None,
            qwen35_cuda_decode: None,
            gpt_oss_perf: None,
        };
        let provenance = GenerationProvenance {
            served_artifact: crate::served_artifact_identity_for_decoder_backend(
                &model.descriptor,
                "cpu",
                &[],
            ),
            adapter_serving: None,
            served_revision: None,
            execution_plan_digest: model.plan_digest.clone(),
            cluster_execution: None,
            load_state: crate::GenerationLoadState::Warm,
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan: Some(self.memory_plan.clone()),
            residency_policy: Some(self.residency_policy.clone()),
            residency_snapshot: Some(self.residency_snapshot()),
            kv_cache_policy: None,
            kv_cache_encoding_policy: None,
            kv_ownership: None,
            prefix_cache_control: Some(request.prefix_cache_control.clone()),
            prefix_cache_state: None,
            prefix_cache_refusal_reason: None,
            prefix_cache_policy: None,
            prefix_cache_identity: None,
            compile_path: None,
            delivery_proof: Some(psionic_runtime::ExecutionDeliveryProof {
                execution_plan_digest: model.plan_digest.clone(),
                kernel_count,
                bytes_moved,
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
            cache_observations: Vec::new(),
            scheduler: None,
            structured_output: structured_output_report,
            psion_served_evidence: None,
            psion_served_output_claim_posture: None,
        };
        let response = GenerationResponse::new(
            request,
            None,
            generated,
            text,
            prompt_tokens.len(),
            0,
            generated_text_terminated.unwrap_or(termination),
        )
        .with_metrics_and_provenance(metrics, provenance);
        let rejected_count = ledger.draft_count.saturating_sub(ledger.accepted_count);
        let acceptance_rate = if ledger.draft_count == 0 {
            0.0
        } else {
            ledger.accepted_count as f64 / ledger.draft_count as f64
        };
        let decode_duration_ns = decode_duration.as_nanos().try_into().unwrap_or(u64::MAX);
        let generated_tokens_per_second = if decode_duration.as_secs_f64() == 0.0 {
            0.0
        } else {
            response.output.tokens.len() as f64 / decode_duration.as_secs_f64()
        };
        let added_peak_residency_bytes = mtp
            .weight_residency_bytes
            .saturating_add(ledger.mtp_kv_cache_peak_bytes)
            .saturating_add(ledger.rollback_snapshot_peak_bytes);
        self.last_mtp_report = Some(Qwen38MtpExecutionReport {
            schema_version: String::from(QWEN38_MTP_EXECUTION_REPORT_SCHEMA_VERSION),
            backend: String::from("cpu"),
            enabled: true,
            max_draft_tokens_per_cycle: config.max_draft_tokens_per_cycle,
            draft_count: ledger.draft_count,
            accepted_count: ledger.accepted_count,
            rejected_count,
            acceptance_rate,
            mtp_forward_count: ledger.mtp_forward_count,
            mtp_alignment_forward_count: ledger.mtp_alignment_forward_count,
            target_forward_count: ledger.target_forward_count,
            target_replay_count: ledger.target_replay_count,
            rollback_count: ledger.rollback_count,
            restored_state_parity: ledger.restored_state_parity,
            mtp_weight_residency_bytes: mtp.weight_residency_bytes,
            mtp_kv_cache_peak_bytes: ledger.mtp_kv_cache_peak_bytes,
            rollback_snapshot_peak_bytes: ledger.rollback_snapshot_peak_bytes,
            added_peak_residency_bytes,
            decode_duration_ns,
            generated_token_count: response.output.tokens.len(),
            generated_tokens_per_second,
            performance_claim: String::from("correctness_only_no_acceleration_claim"),
        });
        Ok(response)
    }

    fn generate_inner(
        &mut self,
        request: &GenerationRequest,
        deadline: Option<Qwen35CpuGenerationDeadline>,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
        adapter_runtime: Option<&Qwen38CpuAdapterRuntime>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        if self.mtp_config.is_some() {
            if multimodal_plan.is_some() {
                return Err(ReferenceTextGenerationError::Runtime(
                    crate::RuntimeError::UnsupportedStep(String::from(
                        "qwen38 CPU multimodal generation does not support MTP speculative decoding",
                    )),
                ));
            }
            return self.generate_inner_with_qwen38_mtp(request, deadline);
        }
        self.last_mtp_report = None;
        if let Some(deadline) = deadline {
            deadline.check()?;
        }
        let prompt_eval_start = Instant::now();
        let prompt_tokens = match multimodal_plan {
            Some(plan) => TokenSequence::new(
                plan.token_ids()
                    .iter()
                    .copied()
                    .map(TokenId)
                    .collect::<Vec<_>>(),
            ),
            None => match &request.prompt {
                GenerationInput::Text(text) => self.model.tokenizer.encode_with_defaults(text),
                GenerationInput::Tokens(tokens) => tokens.clone(),
            },
        };
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let (prompt_tokens, context_window) = psionic_models::apply_context_window(
            &prompt_tokens,
            self.model.descriptor.config.max_context,
            0,
            request.options.max_output_tokens,
            request.options.context_overflow_policy,
            usize::from(
                prompt_tokens.as_slice().first().copied()
                    == Some(self.model.tokenizer.vocabulary().bos_id()),
            ),
        )?;
        if multimodal_plan.is_some_and(|plan| prompt_tokens.len() != plan.token_ids().len()) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 CPU multimodal generation refuses context-window prompt truncation",
                )),
            ));
        }

        let cache_capacity_tokens = qwen35_cache_capacity_tokens(
            prompt_tokens.len(),
            request.options.max_output_tokens,
            self.model.descriptor.config.max_context,
        );
        let mut state = self.model.initial_state(cache_capacity_tokens);
        let mut history = crate::InMemoryKvCache::new(self.model.descriptor.config.max_context, 0);
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut last_logits = Vec::new();
        let mut embedding_overrides = multimodal_plan
            .map(Qwen38MultimodalDecoderPlan::embedding_overrides)
            .unwrap_or_default()
            .iter()
            .peekable();
        for (token_index, token) in prompt_tokens.as_slice().iter().enumerate() {
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            let embedding_override = embedding_overrides
                .next_if(|value| value.token_index == token_index)
                .map(|value| value.embedding.as_slice());
            let mrope_position = multimodal_plan
                .map(|plan| plan.mrope_position_ids()[token_index])
                .unwrap_or([state.position; 3]);
            let mut step = self.model.forward_token_with_embedding_and_mrope(
                &mut state,
                *token,
                embedding_override,
                mrope_position,
            )?;
            if let Some(adapter_runtime) = adapter_runtime {
                adapter_runtime.apply_to_step(&mut step)?;
            }
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            history.append(*token, Vec::new(), Vec::new())?;
            last_logits = step.logits;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        }
        if embedding_overrides.next().is_some() {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 CPU multimodal decoder plan contains an out-of-range embedding override",
                )),
            ));
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let decode_start = Instant::now();
        let mut sampler = crate::GenerationSampler::new(&request.options)?;
        let structured_output_report = sampler.structured_output_report();
        let mut generated_tokens = Vec::new();
        let mut generated_text_terminated = None;
        let mut first_token_emitted_at = None;
        let mut last_token_emitted_at = None;

        let (termination, termination_detail) = loop {
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            if generated_tokens.len() >= request.options.max_output_tokens {
                break (
                    TerminationReason::MaxOutputTokens,
                    Some(GenerationTerminationDetail::max_output_tokens()),
                );
            }
            if prompt_tokens.len().saturating_add(generated_tokens.len())
                >= self.model.descriptor.config.max_context
            {
                break (
                    TerminationReason::ContextLimit,
                    Some(GenerationTerminationDetail::context_limit()),
                );
            }

            let next_token = match sampler.select_next_token(
                &self.model.tokenizer,
                last_logits.as_slice(),
                &history,
                generated_tokens.as_slice(),
            )? {
                crate::GenerationSelection::Token(token) => token,
                crate::GenerationSelection::Terminate => {
                    break (
                        TerminationReason::EndOfSequence,
                        Some(GenerationTerminationDetail::end_of_sequence_token()),
                    );
                }
            };

            if self.model.tokenizer.is_end_of_sequence(next_token) {
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                );
            }

            if first_token_emitted_at.is_none() {
                first_token_emitted_at = Some(decode_start.elapsed());
            }
            last_token_emitted_at = Some(decode_start.elapsed());
            generated_tokens.push(next_token);
            if let Some(stop_hit) = crate::truncate_generated_text_with_match(
                &self.model.tokenizer,
                &mut generated_tokens,
                &request.options.stop_sequences,
            ) {
                generated_text_terminated = Some(TerminationReason::EndOfSequence);
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::stop_sequence(
                        stop_hit.matched_stop_sequence,
                    )),
                );
            }

            let mrope_position = multimodal_plan
                .map(|plan| plan.generated_position(state.position))
                .transpose()
                .map_err(|error| {
                    ReferenceTextGenerationError::Runtime(crate::RuntimeError::UnsupportedStep(
                        error.to_string(),
                    ))
                })?
                .unwrap_or([state.position; 3]);
            let mut step = self.model.forward_token_with_embedding_and_mrope(
                &mut state,
                next_token,
                None,
                mrope_position,
            )?;
            if let Some(adapter_runtime) = adapter_runtime {
                adapter_runtime.apply_to_step(&mut step)?;
            }
            if let Some(deadline) = deadline {
                deadline.check()?;
            }
            history.append(next_token, Vec::new(), Vec::new())?;
            last_logits = step.logits;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        };

        let generated = TokenSequence::new(generated_tokens);
        let text = self.model.tokenizer.decode(generated.as_slice());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(
                prompt_eval_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(self.model.load_duration_ns),
            prompt_eval_count: Some(prompt_tokens.len()),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(generated.len()),
            eval_duration_ns: Some(
                decode_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            time_to_first_token_ns: first_token_emitted_at
                .map(|duration| duration.as_nanos().try_into().unwrap_or(u64::MAX)),
            inter_token_latency_ns: average_inter_token_latency_ns(
                first_token_emitted_at,
                last_token_emitted_at,
                generated.len(),
            ),
            kv_cache: None,
            kv_residency: None,
            kv_cache_encoding: None,
            prefix_tokens_reused: None,
            termination_detail,
            gemma4_metal_decode: None,
            qwen35_cuda_decode: None,
            gpt_oss_perf: None,
        };
        let provenance = GenerationProvenance {
            served_artifact: crate::served_artifact_identity_for_decoder_backend(
                &self.model.descriptor,
                "cpu",
                &[],
            ),
            adapter_serving: adapter_runtime.map(|runtime| runtime.binding.clone()),
            served_revision: None,
            execution_plan_digest: self.model.plan_digest.clone(),
            cluster_execution: None,
            load_state: crate::GenerationLoadState::Warm,
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan: Some(self.memory_plan.clone()),
            residency_policy: Some(self.residency_policy.clone()),
            residency_snapshot: Some(self.residency_snapshot()),
            kv_cache_policy: None,
            kv_cache_encoding_policy: None,
            kv_ownership: None,
            prefix_cache_control: Some(request.prefix_cache_control.clone()),
            prefix_cache_state: None,
            prefix_cache_refusal_reason: None,
            prefix_cache_policy: None,
            prefix_cache_identity: None,
            compile_path: None,
            delivery_proof: Some(psionic_runtime::ExecutionDeliveryProof {
                execution_plan_digest: self.model.plan_digest.clone(),
                kernel_count,
                bytes_moved,
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
            cache_observations: Vec::new(),
            scheduler: None,
            structured_output: structured_output_report,
            psion_served_evidence: None,
            psion_served_output_claim_posture: None,
        };
        let structured_output_value = sampler.structured_output_value(text.as_str())?;
        let response = GenerationResponse::new(
            request,
            None,
            generated,
            text,
            prompt_tokens.len(),
            0,
            generated_text_terminated.unwrap_or(termination),
        )
        .with_metrics_and_provenance(metrics, provenance);
        Ok(if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        })
    }

    fn generate_with_deadline(
        &mut self,
        request: &GenerationRequest,
        deadline: Option<Qwen35CpuGenerationDeadline>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_deadline_and_multimodal_plan(request, deadline, None)
    }

    fn generate_with_deadline_and_multimodal_plan(
        &mut self,
        request: &GenerationRequest,
        deadline: Option<Qwen35CpuGenerationDeadline>,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        if !crate::generation_product_supported(request) {
            return Err(ReferenceTextGenerationError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model.model.model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        let adapter_runtime = self.adapter_runtime_for_request(request)?;
        if adapter_runtime.is_some() && self.mtp_config.is_some() {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: request
                    .adapter_serving
                    .as_ref()
                    .map(|binding| binding.binding_id.clone())
                    .unwrap_or_else(|| String::from("unknown")),
                reason: String::from(
                    "native qwen38 CPU adapter serving does not compose with MTP speculative decoding",
                ),
            });
        }
        if request.session_id.is_some() || request.reset_session {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native qwen35 cpu runtime does not implement session reuse yet",
                )),
            ));
        }
        if multimodal_plan.is_some()
            && self.model.family_metadata.family != GgufDecoderFamily::Qwen38
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native multimodal decoder plans require a qwen38 artifact",
                )),
            ));
        }

        self.residency.begin_request(current_time_millis());
        let response =
            self.generate_inner(request, deadline, multimodal_plan, adapter_runtime.as_ref());
        self.residency.finish_request(current_time_millis());
        self.last_multimodal_plan_receipt = response
            .as_ref()
            .ok()
            .and_then(|_| multimodal_plan.map(|plan| plan.receipt().clone()));
        response
    }

    /// Runs one native Qwen3.8 generation with admitted vision embeddings and
    /// explicit multimodal rotary positions.
    pub fn generate_qwen38_multimodal(
        &mut self,
        request: &GenerationRequest,
        plan: &Qwen38MultimodalDecoderPlan,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_deadline_and_multimodal_plan(request, None, Some(plan))
    }

    /// Generates with a cooperative timeout checked at CPU token-step boundaries.
    ///
    /// Individual matrix operations are not preempted. A zero timeout expires
    /// before tokenization or model execution begins.
    pub fn generate_with_timeout(
        &mut self,
        request: &GenerationRequest,
        timeout: Duration,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_deadline(request, Some(Qwen35CpuGenerationDeadline::new(timeout)))
    }

    fn generate(
        &mut self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_deadline(request, None)
    }
}

impl TextGenerationExecutor for CpuGgufQwen35TextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        Self::generate(self, request)
    }
}

impl StreamingTextGenerationExecutor for CpuGgufQwen35TextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, ReferenceTextGenerationError> {
        let response = self.generate(request)?;
        Ok(Box::new(CompletedQwen35Stream::new(response, "cpu")))
    }
}

impl ManagedTextGenerationRuntime for CpuGgufQwen35TextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        Self::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        Self::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::unload_model(self, model_id)
    }
}

pub struct MetalGgufQwen35TextGenerationService {
    backend: MetalBackend,
    model: Arc<MetalQwen35Model>,
    sessions: crate::InMemoryGenerationSessionStore,
    backend_health: BackendHealthTracker,
    residency: LoadedModelResidency,
    memory_plan: psionic_runtime::ModelMemoryPlan,
    residency_policy: psionic_runtime::ModelResidencyPolicy,
    last_multimodal_plan_receipt: Option<Qwen38MultimodalDecoderPlanReceipt>,
}

impl MetalGgufQwen35TextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let mut backend = MetalBackend::new();
        let runtime = backend
            .configure_text_generation_runtime(MetalTextGenerationRuntimePolicy::gpt_oss_default())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if !runtime.admission.admitted {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(runtime.admission.refusal_reason.unwrap_or_else(
                    || String::from("metal token-generation runtime admission refused"),
                )),
            ));
        }
        let model = Arc::new(MetalQwen35Model::from_gguf_path(
            path,
            &mut backend,
            runtime.device_memory_budget,
        )?);
        let now_millis = current_time_millis();
        let mut backend_health = BackendHealthTracker::default();
        backend_health.observe("metal", backend.health(), now_millis);
        Ok(Self {
            backend,
            model: Arc::clone(&model),
            sessions: crate::InMemoryGenerationSessionStore::new(),
            backend_health,
            residency: LoadedModelResidency::ready(
                now_millis,
                crate::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            ),
            memory_plan: model.memory_plan.clone(),
            residency_policy: psionic_runtime::ModelResidencyPolicy::default(),
            last_multimodal_plan_receipt: None,
        })
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model.descriptor
    }

    /// Qwen3.8 multimodal generation.
    #[must_use]
    pub fn last_qwen38_multimodal_plan_receipt(
        &self,
    ) -> Option<&Qwen38MultimodalDecoderPlanReceipt> {
        self.last_multimodal_plan_receipt.as_ref()
    }

    #[must_use]
    pub fn runtime_support(&self) -> crate::GgufDecoderRuntimeSupport {
        crate::GgufDecoderRuntimeSupport {
            family: self.model.family_metadata.family,
            supported_backends: vec![String::from("metal")],
            unsupported_backends: vec![String::from("cpu"), String::from("cuda")],
            unsupported_features: qwen35_unsupported_features(&self.model.family_metadata, false),
            quantization_modes: self.model.descriptor.weights.quantization_modes.clone(),
            adapter_runtime: crate::DecoderAdapterRuntimeSupport {
                support_level: String::from("unsupported"),
                import_formats: Vec::new(),
                residency_modes: Vec::new(),
                batching_mode: String::from("not_available"),
                unsupported_reasons: vec![String::from(
                    "LM-head LoRA serving is not implemented on the native qwen35 metal runtime",
                )],
            },
        }
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        (model_id == self.model.descriptor.model.model_id)
            .then_some(self.model.plan_digest.as_str())
    }

    #[must_use]
    pub fn metal_runtime_contract(&self) -> Qwen35MetalRuntimeContract {
        let context_limit_tokens = self.model.metal_context_limit_tokens();
        let (recurrent_state_host_bytes, kv_cache_host_bytes) =
            self.model.planned_state_host_bytes(context_limit_tokens);
        let weight_device_bytes = self.model.weight_device_bytes();
        Qwen35MetalRuntimeContract {
            family: self.model.family_metadata.family,
            model_id: self.model.descriptor.model.model_id.clone(),
            artifact_digest: self.model.descriptor.weights.digest.clone(),
            execution_plan_namespace: String::from(match self.model.family_metadata.family {
                GgufDecoderFamily::Qwen38 => "qwen38-native-metal|v1",
                _ => "qwen35-native-metal|v1",
            }),
            execution_plan_digest: self.model.plan_digest.clone(),
            context_limit_tokens,
            artifact_bytes: self.model.memory_plan.weights_bytes,
            device_capacity_bytes: self.model.device_memory_budget.total_bytes,
            available_execution_bytes: self.model.device_memory_budget.available_execution_bytes,
            weight_device_bytes,
            recurrent_state_host_bytes,
            kv_cache_host_bytes,
            planned_device_visible_bytes: weight_device_bytes,
            admitted_layer_count: self.model.layers.len(),
            resident_layer_count: self.model.layers.len(),
            projection_count: self.model.projection_count(),
            native_projection_count: self.model.native_projection_count(),
            admitted_conversion_count: 0,
            quantization_modes: self.model.descriptor.weights.quantization_modes.clone(),
            host_stepped_state: true,
            host_projection_fallback_enabled: self.model.native_projection_count()
                != self.model.projection_count(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_raw_logits(
        &mut self,
        tokens: &TokenSequence,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let mut state = self
            .model
            .initial_state(self.model.metal_context_limit_tokens());
        let mut logits = Vec::new();
        for token in tokens.as_slice() {
            logits = self
                .model
                .forward_token(
                    &mut self.backend,
                    &mut state,
                    *token,
                    MetalStepOutputMode::FullLogits,
                )?
                .logits;
        }
        Ok(logits)
    }

    #[cfg(test)]
    pub(crate) fn test_raw_logits_with_embedding_and_mrope(
        &mut self,
        tokens: &TokenSequence,
        embedding_overrides: &[(usize, Vec<f32>)],
        mrope_positions: &[[usize; 3]],
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let mut state = self
            .model
            .initial_state(self.model.metal_context_limit_tokens());
        let mut logits = Vec::new();
        for (index, token) in tokens.as_slice().iter().enumerate() {
            let embedding_override = embedding_overrides
                .iter()
                .find(|(token_index, _)| *token_index == index)
                .map(|(_, embedding)| embedding.as_slice());
            let mrope_position = mrope_positions
                .get(index)
                .copied()
                .unwrap_or([state.position; 3]);
            logits = self
                .model
                .forward_token_with_embedding_and_mrope(
                    &mut self.backend,
                    &mut state,
                    *token,
                    embedding_override,
                    mrope_position,
                    MetalStepOutputMode::FullLogits,
                )?
                .logits;
        }
        Ok(logits)
    }

    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        Ok(self.sessions.create(
            &self.model.descriptor,
            crate::served_artifact_identity_for_decoder_backend(
                &self.model.descriptor,
                "metal",
                &[],
            )
            .served_artifact_digest,
        ))
    }

    pub fn reset_session(
        &mut self,
        session_id: &crate::SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    pub fn close_session(
        &mut self,
        session_id: &crate::SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }

    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        vec![self.loaded_model_view()]
    }

    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        LoadedModelsObservation::new(vec![self.loaded_model_view().summary])
    }

    #[must_use]
    pub fn observability(&mut self) -> LocalRuntimeObservability {
        self.backend_health
            .observe("metal", self.backend.health(), current_time_millis());
        LocalRuntimeObservability {
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: crate::cache_invalidation_policy(),
            execution_profile: crate::continuous_batch_text_generation_execution_profile(),
            queue_depth: 0,
            queue_capacity: Some(
                crate::continuous_batch_text_generation_execution_profile()
                    .queue_policy
                    .max_queued_requests,
            ),
            active_sessions: self.sessions.len(),
            active_requests: self.residency.active_requests,
            memory_footprint: self.residency_snapshot(),
            backend_health: self.backend_health.snapshot(),
            recent_transitions: self.backend_health.recent_changes(),
        }
    }

    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency
            .refresh_keep_alive(keep_alive_millis, current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        if model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                model_id.to_string(),
            ));
        }
        self.residency.expire_now(current_time_millis());
        Ok(self.loaded_model_view())
    }

    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        let responses = requests
            .iter()
            .map(|request| self.generate(request))
            .collect::<Vec<_>>();
        ContinuousBatchGenerationResult {
            responses,
            scheduler_metrics: psionic_runtime::GenerationSchedulerMetrics::default(),
        }
    }

    fn loaded_model_view(&self) -> LoadedModelView {
        let mut summary = crate::LoadedModelSummary::from_decoder_descriptor(
            self.model.descriptor.model.model_id.clone(),
            &self.model.descriptor,
        );
        summary.size_bytes = Some(self.memory_plan.weights_bytes);
        summary.size_vram_bytes = Some(self.memory_plan.resident_device_bytes);
        summary.backend = Some(String::from("metal"));
        LoadedModelView {
            summary,
            residency: self.residency.clone(),
            memory_plan: self.memory_plan.clone(),
            residency_policy: self.residency_policy.clone(),
            residency_snapshot: self.residency_snapshot(),
        }
    }

    fn residency_snapshot(&self) -> psionic_runtime::MemoryResidencySnapshot {
        psionic_runtime::MemoryResidencySnapshot::from_loaded_models(&[
            psionic_runtime::LoadedModelMemoryState {
                model_id: self.model.descriptor.model.model_id.clone(),
                plan: self.memory_plan.clone(),
                active_requests: self.residency.active_requests,
                last_used_at_millis: self.residency.last_used_at_millis,
            },
        ])
    }

    fn generate_inner(
        &mut self,
        request: &GenerationRequest,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        let prompt_eval_start = Instant::now();
        let prompt_tokens = match multimodal_plan {
            Some(plan) => TokenSequence::new(
                plan.token_ids()
                    .iter()
                    .copied()
                    .map(TokenId)
                    .collect::<Vec<_>>(),
            ),
            None => match &request.prompt {
                GenerationInput::Text(text) => self.model.tokenizer.encode_with_defaults(text),
                GenerationInput::Tokens(tokens) => tokens.clone(),
            },
        };
        if prompt_tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let (prompt_tokens, context_window) = psionic_models::apply_context_window(
            &prompt_tokens,
            self.model.metal_context_limit_tokens(),
            0,
            request.options.max_output_tokens,
            request.options.context_overflow_policy,
            usize::from(
                prompt_tokens.as_slice().first().copied()
                    == Some(self.model.tokenizer.vocabulary().bos_id()),
            ),
        )?;
        if multimodal_plan.is_some_and(|plan| prompt_tokens.len() != plan.token_ids().len()) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 Metal multimodal generation refuses context-window prompt truncation",
                )),
            ));
        }

        let cache_capacity_tokens = qwen35_cache_capacity_tokens(
            prompt_tokens.len(),
            request.options.max_output_tokens,
            self.model.metal_context_limit_tokens(),
        );
        let mut state = self.model.initial_state(cache_capacity_tokens);
        let mut history = crate::InMemoryKvCache::new(self.model.metal_context_limit_tokens(), 0);
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut last_logits = Vec::new();
        let output_mode = qwen35_metal_output_mode(&request.options);
        let mut pending_selected_token = None;
        let mut embedding_overrides = multimodal_plan
            .map(Qwen38MultimodalDecoderPlan::embedding_overrides)
            .unwrap_or_default()
            .iter()
            .peekable();
        if let Some((last_prompt_token, prompt_prefix)) = prompt_tokens.as_slice().split_last() {
            for (token_index, token) in prompt_prefix.iter().enumerate() {
                let embedding_override = embedding_overrides
                    .next_if(|value| value.token_index == token_index)
                    .map(|value| value.embedding.as_slice());
                let mrope_position = multimodal_plan
                    .map(|plan| plan.mrope_position_ids()[token_index])
                    .unwrap_or([state.position; 3]);
                let step = self.model.forward_token_with_embedding_and_mrope(
                    &mut self.backend,
                    &mut state,
                    *token,
                    embedding_override,
                    mrope_position,
                    MetalStepOutputMode::NoOutput,
                )?;
                history.append(*token, Vec::new(), Vec::new())?;
                kernel_count = kernel_count.saturating_add(step.kernel_count);
                bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            }
            let last_token_index = prompt_tokens.len().saturating_sub(1);
            let embedding_override = embedding_overrides
                .next_if(|value| value.token_index == last_token_index)
                .map(|value| value.embedding.as_slice());
            let mrope_position = multimodal_plan
                .map(|plan| plan.mrope_position_ids()[last_token_index])
                .unwrap_or([state.position; 3]);
            let step = self.model.forward_token_with_embedding_and_mrope(
                &mut self.backend,
                &mut state,
                *last_prompt_token,
                embedding_override,
                mrope_position,
                output_mode,
            )?;
            history.append(*last_prompt_token, Vec::new(), Vec::new())?;
            last_logits = step.logits;
            pending_selected_token = step.selected_token;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        }
        if embedding_overrides.next().is_some() {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "qwen38 Metal multimodal decoder plan contains an out-of-range embedding override",
                )),
            ));
        }

        let prompt_eval_duration_ns = prompt_eval_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        let decode_start = Instant::now();
        let mut sampler = crate::GenerationSampler::new(&request.options)?;
        let structured_output_report = sampler.structured_output_report();
        let mut generated_tokens = Vec::new();
        let mut generated_text_terminated = None;
        let mut first_token_emitted_at = None;
        let mut last_token_emitted_at = None;

        let (termination, termination_detail) = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break (
                    TerminationReason::MaxOutputTokens,
                    Some(GenerationTerminationDetail::max_output_tokens()),
                );
            }
            if prompt_tokens.len().saturating_add(generated_tokens.len())
                >= self.model.metal_context_limit_tokens()
            {
                break (
                    TerminationReason::ContextLimit,
                    Some(GenerationTerminationDetail::context_limit()),
                );
            }

            let next_token = if output_mode == MetalStepOutputMode::ArgmaxOnly {
                pending_selected_token.take().ok_or_else(|| {
                    ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(
                        String::from("qwen35 metal argmax decode did not return a selected token"),
                    ))
                })?
            } else {
                match sampler.select_next_token(
                    &self.model.tokenizer,
                    last_logits.as_slice(),
                    &history,
                    generated_tokens.as_slice(),
                )? {
                    crate::GenerationSelection::Token(token) => token,
                    crate::GenerationSelection::Terminate => {
                        break (
                            TerminationReason::EndOfSequence,
                            Some(GenerationTerminationDetail::end_of_sequence_token()),
                        );
                    }
                }
            };

            if self.model.tokenizer.is_end_of_sequence(next_token) {
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::end_of_sequence_token()),
                );
            }

            if first_token_emitted_at.is_none() {
                first_token_emitted_at = Some(decode_start.elapsed());
            }
            last_token_emitted_at = Some(decode_start.elapsed());
            generated_tokens.push(next_token);
            if let Some(stop_hit) = crate::truncate_generated_text_with_match(
                &self.model.tokenizer,
                &mut generated_tokens,
                &request.options.stop_sequences,
            ) {
                generated_text_terminated = Some(TerminationReason::EndOfSequence);
                break (
                    TerminationReason::EndOfSequence,
                    Some(GenerationTerminationDetail::stop_sequence(
                        stop_hit.matched_stop_sequence,
                    )),
                );
            }

            let mrope_position = multimodal_plan
                .map(|plan| plan.generated_position(state.position))
                .transpose()
                .map_err(|error| {
                    ReferenceTextGenerationError::Runtime(crate::RuntimeError::UnsupportedStep(
                        error.to_string(),
                    ))
                })?
                .unwrap_or([state.position; 3]);
            let step = self.model.forward_token_with_embedding_and_mrope(
                &mut self.backend,
                &mut state,
                next_token,
                None,
                mrope_position,
                output_mode,
            )?;
            history.append(next_token, Vec::new(), Vec::new())?;
            last_logits = step.logits;
            pending_selected_token = step.selected_token;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        };

        let generated = TokenSequence::new(generated_tokens);
        let text = self.model.tokenizer.decode(generated.as_slice());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(
                prompt_eval_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(self.model.load_duration_ns),
            prompt_eval_count: Some(prompt_tokens.len()),
            prompt_eval_duration_ns: Some(prompt_eval_duration_ns),
            context_window: Some(context_window),
            eval_count: Some(generated.len()),
            eval_duration_ns: Some(
                decode_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            time_to_first_token_ns: first_token_emitted_at
                .map(|duration| duration.as_nanos().try_into().unwrap_or(u64::MAX)),
            inter_token_latency_ns: average_inter_token_latency_ns(
                first_token_emitted_at,
                last_token_emitted_at,
                generated.len(),
            ),
            kv_cache: None,
            kv_residency: None,
            kv_cache_encoding: None,
            prefix_tokens_reused: None,
            termination_detail,
            gemma4_metal_decode: None,
            qwen35_cuda_decode: None,
            gpt_oss_perf: None,
        };
        let provenance = GenerationProvenance {
            served_artifact: crate::served_artifact_identity_for_decoder_backend(
                &self.model.descriptor,
                "metal",
                &[],
            ),
            adapter_serving: None,
            served_revision: None,
            execution_plan_digest: self.model.plan_digest.clone(),
            cluster_execution: None,
            load_state: crate::GenerationLoadState::Warm,
            isolation_policy: psionic_runtime::LocalServingIsolationPolicy::in_process_runtime(),
            streaming_policy: None,
            memory_plan: Some(self.memory_plan.clone()),
            residency_policy: Some(self.residency_policy.clone()),
            residency_snapshot: Some(self.residency_snapshot()),
            kv_cache_policy: None,
            kv_cache_encoding_policy: None,
            kv_ownership: None,
            prefix_cache_control: Some(request.prefix_cache_control.clone()),
            prefix_cache_state: None,
            prefix_cache_refusal_reason: None,
            prefix_cache_policy: None,
            prefix_cache_identity: None,
            compile_path: None,
            delivery_proof: Some(psionic_runtime::ExecutionDeliveryProof {
                execution_plan_digest: self.model.plan_digest.clone(),
                kernel_count,
                bytes_moved,
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
            cache_observations: Vec::new(),
            scheduler: None,
            structured_output: structured_output_report,
            psion_served_evidence: None,
            psion_served_output_claim_posture: None,
        };
        let structured_output_value = sampler.structured_output_value(text.as_str())?;
        let response = GenerationResponse::new(
            request,
            None,
            generated,
            text,
            prompt_tokens.len(),
            0,
            generated_text_terminated.unwrap_or(termination),
        )
        .with_metrics_and_provenance(metrics, provenance);
        Ok(if let Some(value) = structured_output_value {
            response.with_structured_output_value(value)
        } else {
            response
        })
    }

    fn generate(
        &mut self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_qwen38_multimodal_plan(request, None)
    }

    fn generate_with_qwen38_multimodal_plan(
        &mut self,
        request: &GenerationRequest,
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        if request.product_id != crate::TEXT_GENERATION_PRODUCT_ID {
            return Err(ReferenceTextGenerationError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model.model.model_id != self.model.descriptor.model.model_id {
            return Err(ReferenceTextGenerationError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.adapter_serving.is_some() {
            return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: request
                    .adapter_serving
                    .as_ref()
                    .map(|binding| binding.binding_id.clone())
                    .unwrap_or_else(|| String::from("unknown")),
                reason: String::from(
                    "LM-head LoRA serving is not implemented on the native qwen35 metal runtime",
                ),
            });
        }
        if request.session_id.is_some() || request.reset_session {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native qwen35 metal runtime does not implement session reuse yet",
                )),
            ));
        }
        if multimodal_plan.is_some()
            && self.model.family_metadata.family != GgufDecoderFamily::Qwen38
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(String::from(
                    "native multimodal decoder plans require a qwen38 artifact",
                )),
            ));
        }

        self.residency.begin_request(current_time_millis());
        let response = self.generate_inner(request, multimodal_plan);
        self.residency.finish_request(current_time_millis());
        self.last_multimodal_plan_receipt = response
            .as_ref()
            .ok()
            .and_then(|_| multimodal_plan.map(|plan| plan.receipt().clone()));
        response
    }

    /// Runs one native Qwen3.8 Metal generation with admitted vision
    /// embeddings and explicit multimodal rotary positions.
    pub fn generate_qwen38_multimodal(
        &mut self,
        request: &GenerationRequest,
        plan: &Qwen38MultimodalDecoderPlan,
    ) -> Result<GenerationResponse, ReferenceTextGenerationError> {
        self.generate_with_qwen38_multimodal_plan(request, Some(plan))
    }
}

impl TextGenerationExecutor for MetalGgufQwen35TextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        Self::generate(self, request)
    }
}

impl StreamingTextGenerationExecutor for MetalGgufQwen35TextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, ReferenceTextGenerationError> {
        let response = self.generate(request)?;
        Ok(Box::new(CompletedQwen35Stream::new(response, "metal")))
    }
}

impl ManagedTextGenerationRuntime for MetalGgufQwen35TextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        Self::loaded_models(self)
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        Self::observability(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Self::unload_model(self, model_id)
    }
}

#[derive(Clone, Debug)]
struct MetalQwen35Model {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GgufRuntimeTokenizer,
    token_embedding: HostMatrix,
    output_norm: Vec<f32>,
    output: MetalQuantizedMatrix,
    layers: Vec<MetalQwen35Layer>,
    plan_digest: String,
    load_duration_ns: u64,
    memory_plan: psionic_runtime::ModelMemoryPlan,
    device_memory_budget: DeviceMemoryBudget,
}

impl MetalQwen35Model {
    fn from_gguf_path(
        path: impl AsRef<Path>,
        backend: &mut MetalBackend,
        device_memory_budget: DeviceMemoryBudget,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let artifact = GgufBlobArtifact::open_path(&path, gguf_local_blob_open_options())?;
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if !matches!(
            adapter.family_metadata().family,
            GgufDecoderFamily::Qwen35 | GgufDecoderFamily::Qwen38
        ) {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }
        let qwen38_preflight =
            matches!(adapter.family_metadata().family, GgufDecoderFamily::Qwen38)
                .then(|| {
                    qwen38_metal_projection_rows(&artifact, adapter.tensor_layout())
                        .and_then(|rows| qwen38_metal_projection_preflight(rows.as_slice()))
                })
                .transpose()?;
        let tokenizer = GgufRuntimeTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build qwen35 tokenizer: {error}"),
            }
        })?;
        let token_embedding_name = adapter.tensor_layout().token_embedding.as_str();
        let token_embedding = HostMatrix::load(&artifact, token_embedding_name)?;
        let output = if let Some(name) = adapter.tensor_layout().output.as_ref() {
            MetalQuantizedMatrix::load(backend, &artifact, name.as_str())?
        } else {
            MetalQuantizedMatrix::from_host_matrix(
                backend,
                token_embedding_name,
                token_embedding.clone(),
            )?
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layout| {
                MetalQwen35Layer::load(backend, &artifact, layout, adapter.family_metadata())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output_norm =
            load_dense_vector(&artifact, adapter.tensor_layout().output_norm.as_str())?;
        let weights_bytes = std::fs::metadata(path.as_ref())
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let resident_device_bytes = output.device_residency_bytes().saturating_add(
            layers
                .iter()
                .map(MetalQwen35Layer::device_residency_bytes)
                .fold(0usize, usize::saturating_add),
        ) as u64;
        if let Some(preflight) = qwen38_preflight {
            if preflight.projection_count
                != layers
                    .iter()
                    .map(MetalQwen35Layer::projection_count)
                    .fold(1usize, usize::saturating_add)
                || preflight.projected_device_weight_bytes != resident_device_bytes
            {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "qwen38 metal residency mismatch after upload: preflight projections={} bytes={}, resident projections={} bytes={resident_device_bytes}",
                        preflight.projection_count,
                        preflight.projected_device_weight_bytes,
                        layers
                            .iter()
                            .map(MetalQwen35Layer::projection_count)
                            .fold(1usize, usize::saturating_add),
                    ),
                }
                .into());
            }
            if let Some(available) = device_memory_budget.available_execution_bytes {
                if resident_device_bytes > available {
                    return Err(ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: format!(
                            "qwen38 metal residency requires {resident_device_bytes} device-visible weight bytes, but the configured Metal execution budget exposes {available} bytes"
                        ),
                    }
                    .into());
                }
            }
        }
        Ok(Self {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            token_embedding,
            output_norm,
            output,
            layers,
            plan_digest: digest_qwen35_metal_plan(adapter.descriptor(), adapter.family_metadata()),
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
            memory_plan: psionic_runtime::ModelMemoryPlan::split_residency(
                weights_bytes,
                0,
                0,
                weights_bytes,
                resident_device_bytes,
            ),
            device_memory_budget,
        })
    }

    fn metal_context_limit_tokens(&self) -> usize {
        if matches!(self.family_metadata.family, GgufDecoderFamily::Qwen38) {
            self.descriptor
                .config
                .max_context
                .min(QWEN38_METAL_CONTEXT_LIMIT_TOKENS)
        } else {
            self.descriptor.config.max_context
        }
    }

    fn projection_count(&self) -> usize {
        self.layers
            .iter()
            .map(MetalQwen35Layer::projection_count)
            .fold(1usize, usize::saturating_add)
    }

    fn native_projection_count(&self) -> usize {
        self.layers
            .iter()
            .map(MetalQwen35Layer::native_projection_count)
            .fold(usize::from(self.output.is_native()), usize::saturating_add)
    }

    fn weight_device_bytes(&self) -> u64 {
        self.output
            .device_residency_bytes()
            .saturating_add(
                self.layers
                    .iter()
                    .map(MetalQwen35Layer::device_residency_bytes)
                    .fold(0usize, usize::saturating_add),
            )
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn planned_state_host_bytes(&self, cache_capacity_tokens: usize) -> (u64, u64) {
        let mut recurrent_bytes = 0usize;
        let mut kv_cache_bytes = 0usize;
        for layer in &self.layers {
            match &layer.kind {
                MetalQwen35LayerKind::Hybrid(hybrid) => {
                    recurrent_bytes = recurrent_bytes
                        .saturating_add(
                            hybrid.qkv_gate_alpha_beta.rows_per_projection[0]
                                .saturating_mul(hybrid.conv_kernel.saturating_sub(1))
                                .saturating_mul(std::mem::size_of::<f32>()),
                        )
                        .saturating_add(
                            hybrid
                                .time_step_rank
                                .saturating_mul(hybrid.state_size)
                                .saturating_mul(hybrid.state_size)
                                .saturating_mul(std::mem::size_of::<f32>()),
                        );
                }
                MetalQwen35LayerKind::FullAttention(full_attention) => {
                    kv_cache_bytes = kv_cache_bytes
                        .saturating_add(
                            cache_capacity_tokens
                                .saturating_mul(std::mem::size_of::<Qwen35FullAttentionEntry>()),
                        )
                        .saturating_add(
                            cache_capacity_tokens
                                .saturating_mul(full_attention.kv_width)
                                .saturating_mul(std::mem::size_of::<f32>())
                                .saturating_mul(2),
                        );
                }
            }
        }
        (
            recurrent_bytes.try_into().unwrap_or(u64::MAX),
            kv_cache_bytes.try_into().unwrap_or(u64::MAX),
        )
    }

    fn initial_state(&self, cache_capacity_tokens: usize) -> MetalQwen35State {
        MetalQwen35State {
            position: 0,
            layers: self
                .layers
                .iter()
                .map(|layer| layer.initial_state(cache_capacity_tokens))
                .collect(),
        }
    }

    #[cfg(test)]
    fn forward_token(
        &self,
        backend: &mut MetalBackend,
        state: &mut MetalQwen35State,
        token: TokenId,
        output_mode: MetalStepOutputMode,
    ) -> Result<MetalQwen35ForwardStep, ReferenceTextGenerationError> {
        self.forward_token_with_embedding_and_mrope(
            backend,
            state,
            token,
            None,
            [state.position; 3],
            output_mode,
        )
    }

    fn forward_token_with_embedding_and_mrope(
        &self,
        backend: &mut MetalBackend,
        state: &mut MetalQwen35State,
        token: TokenId,
        embedding_override: Option<&[f32]>,
        mrope_position: [usize; 3],
        output_mode: MetalStepOutputMode,
    ) -> Result<MetalQwen35ForwardStep, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.descriptor.config.vocab_size,
            });
        }
        if embedding_override
            .is_some_and(|embedding| embedding.len() != self.descriptor.config.hidden_size)
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(format!(
                    "qwen38 Metal decoder embedding override width must be {}, received {}",
                    self.descriptor.config.hidden_size,
                    embedding_override.map_or(0, <[f32]>::len),
                )),
            ));
        }
        let mut hidden = match embedding_override {
            Some(embedding) => embedding.to_vec(),
            None => self.token_embedding.decode_row(token.as_u32() as usize)?,
        };
        let mut kernel_count = 1usize;
        let mut bytes_moved = embedding_override.map_or_else(
            || self.token_embedding.host_residency_bytes() as u64,
            |embedding| embedding.len().saturating_mul(std::mem::size_of::<f32>()) as u64,
        );
        for (layer, layer_state) in self.layers.iter().zip(state.layers.iter_mut()) {
            let step = layer.forward(
                backend,
                &self.family_metadata,
                self.descriptor.config.hidden_size,
                self.descriptor.config.block.attention.head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                mrope_position,
                hidden,
                layer_state,
            )?;
            hidden = step.hidden;
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        }
        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        let (logits, selected_token, output_kernel_count, output_bytes_moved) = match output_mode {
            MetalStepOutputMode::NoOutput => (Vec::new(), None, 0usize, 0u64),
            MetalStepOutputMode::FullLogits => {
                let logits_step = self.output.matvec(backend, final_hidden.as_slice())?;
                (
                    logits_step.values,
                    None,
                    logits_step.kernel_count,
                    logits_step.bytes_moved,
                )
            }
            MetalStepOutputMode::ArgmaxOnly => {
                let selection = self.output.select_logits_output(
                    backend,
                    final_hidden.as_slice(),
                    MetalLogitsOutputMode::GreedyToken,
                )?;
                (
                    Vec::new(),
                    selection.selected_tokens.first().copied().map(TokenId),
                    1usize,
                    self.output.byte_length() as u64,
                )
            }
        };
        bytes_moved = bytes_moved.saturating_add(output_bytes_moved);
        kernel_count = kernel_count.saturating_add(output_kernel_count);
        state.position = state.position.saturating_add(1);
        Ok(MetalQwen35ForwardStep {
            logits,
            selected_token,
            kernel_count,
            bytes_moved,
        })
    }
}

#[derive(Clone, Debug)]
struct MetalQwen35ForwardStep {
    logits: Vec<f32>,
    selected_token: Option<TokenId>,
    kernel_count: usize,
    bytes_moved: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetalStepOutputMode {
    NoOutput,
    FullLogits,
    ArgmaxOnly,
}

#[derive(Clone, Debug)]
struct MetalQwen35State {
    position: usize,
    layers: Vec<MetalQwen35LayerState>,
}

#[derive(Clone, Debug)]
struct CpuQwen35Model {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GgufRuntimeTokenizer,
    token_embedding: HostMatrix,
    output_norm: Vec<f32>,
    output: HostMatrix,
    layers: Vec<CpuQwen35Layer>,
    mtp: Option<CpuQwen38MtpModel>,
    plan_digest: String,
    load_duration_ns: u64,
    memory_plan: psionic_runtime::ModelMemoryPlan,
}

impl CpuQwen35Model {
    fn from_gguf_path(
        path: impl AsRef<Path>,
        enable_qwen38_mtp: bool,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let artifact = GgufBlobArtifact::open_path(&path, gguf_local_blob_open_options())?;
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if !matches!(
            adapter.family_metadata().family,
            GgufDecoderFamily::Qwen35 | GgufDecoderFamily::Qwen38
        ) {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }
        let tokenizer = GgufRuntimeTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build qwen35 tokenizer: {error}"),
            }
        })?;
        let token_embedding_name = adapter.tensor_layout().token_embedding.as_str();
        let token_embedding = HostMatrix::load(&artifact, token_embedding_name)?;
        let output = if let Some(name) = adapter.tensor_layout().output.as_ref() {
            HostMatrix::load(&artifact, name.as_str())?
        } else {
            token_embedding.clone()
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layout| CpuQwen35Layer::load(&artifact, layout, adapter.family_metadata()))
            .collect::<Result<Vec<_>, _>>()?;
        let output_norm =
            load_dense_vector(&artifact, adapter.tensor_layout().output_norm.as_str())?;
        let mtp = if enable_qwen38_mtp {
            if adapter.family_metadata().family != GgufDecoderFamily::Qwen38 {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: String::from(
                        "optional Qwen3.8 MTP execution requires a qualified qwen38 artifact",
                    ),
                }
                .into());
            }
            let nextn_layers =
                family_fact_usize(adapter.family_metadata(), "qwen35.nextn_predict_layers")?;
            if nextn_layers != 1 {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "optional Qwen3.8 MTP execution requires exactly one NextN block, found {nextn_layers}",
                    ),
                }
                .into());
            }
            Some(CpuQwen38MtpModel::load(
                &artifact,
                adapter.tensor_layout().layers.len(),
                adapter.tensor_layout().layers.last().ok_or_else(|| {
                    ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: String::from("qwen38 MTP loading requires a trunk layer template"),
                    }
                })?,
                adapter.family_metadata(),
            )?)
        } else {
            None
        };
        let weights_bytes = std::fs::metadata(path.as_ref())
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        Ok(Self {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            token_embedding,
            output_norm,
            output,
            layers,
            mtp,
            plan_digest: if enable_qwen38_mtp {
                digest_qwen38_mtp_cpu_plan(adapter.descriptor(), adapter.family_metadata())
            } else {
                digest_qwen35_cpu_plan(adapter.descriptor(), adapter.family_metadata())
            },
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
            memory_plan: psionic_runtime::ModelMemoryPlan::host_only(weights_bytes, 0, 0),
        })
    }

    fn initial_state(&self, cache_capacity_tokens: usize) -> CpuQwen35State {
        CpuQwen35State {
            position: 0,
            layers: self
                .layers
                .iter()
                .map(|layer| layer.initial_state(cache_capacity_tokens))
                .collect(),
        }
    }

    fn forward_token(
        &self,
        state: &mut CpuQwen35State,
        token: TokenId,
    ) -> Result<CpuQwen35ForwardStep, ReferenceTextGenerationError> {
        self.forward_token_with_embedding_and_mrope(state, token, None, [state.position; 3])
    }

    fn forward_token_with_embedding_and_mrope(
        &self,
        state: &mut CpuQwen35State,
        token: TokenId,
        embedding_override: Option<&[f32]>,
        mrope_position: [usize; 3],
    ) -> Result<CpuQwen35ForwardStep, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.descriptor.config.vocab_size,
            });
        }
        if embedding_override
            .is_some_and(|embedding| embedding.len() != self.descriptor.config.hidden_size)
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(format!(
                    "qwen38 CPU decoder embedding override width must be {}, received {}",
                    self.descriptor.config.hidden_size,
                    embedding_override.map_or(0, <[f32]>::len),
                )),
            ));
        }
        let mut hidden = match embedding_override {
            Some(embedding) => embedding.to_vec(),
            None => self.token_embedding.decode_row(token.as_u32() as usize)?,
        };
        let mut kernel_count = 1usize;
        let mut bytes_moved = embedding_override.map_or_else(
            || self.token_embedding.host_residency_bytes() as u64,
            |embedding| embedding.len().saturating_mul(std::mem::size_of::<f32>()) as u64,
        );
        for (layer, layer_state) in self.layers.iter().zip(state.layers.iter_mut()) {
            hidden = layer.forward(
                &self.family_metadata,
                self.descriptor.config.hidden_size,
                self.descriptor.config.block.attention.head_count,
                self.descriptor.config.block.attention.head_dim,
                self.descriptor.config.block.attention.rotary_dim,
                mrope_position,
                hidden,
                layer_state,
            )?;
            kernel_count = kernel_count.saturating_add(1);
            bytes_moved = bytes_moved.saturating_add(layer.host_residency_bytes() as u64);
        }
        let final_hidden = rms_norm(
            hidden.as_slice(),
            self.output_norm.as_slice(),
            self.family_metadata.rms_norm_epsilon,
        );
        let logits = self
            .output
            .matvec(final_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        bytes_moved = bytes_moved.saturating_add(self.output.host_residency_bytes() as u64);
        state.position = state.position.saturating_add(1);
        Ok(CpuQwen35ForwardStep {
            logits,
            final_hidden,
            kernel_count: kernel_count.saturating_add(1),
            bytes_moved,
        })
    }
}

#[derive(Clone, Debug)]
struct CpuQwen38MtpModel {
    embedding_hidden_projection: HostMatrix,
    embedding_norm: Vec<f32>,
    target_hidden_norm: Vec<f32>,
    shared_head_norm: Vec<f32>,
    layer: CpuQwen35Layer,
    weight_residency_bytes: u64,
}

impl CpuQwen38MtpModel {
    fn load(
        artifact: &GgufBlobArtifact,
        layer_index: usize,
        trunk_template: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let prefix = format!("blk.{layer_index}");
        let mut layout = trunk_template.clone();
        layout.layer_index = layer_index;
        layout.layer_kind = GgufDecoderLayerKind::Qwen35FullAttention;
        layout.attention_norm = format!("{prefix}.attn_norm.weight");
        layout.attention_query_weight = Some(format!("{prefix}.attn_q.weight"));
        layout.attention_query_norm = Some(format!("{prefix}.attn_q_norm.weight"));
        layout.attention_key_weight = Some(format!("{prefix}.attn_k.weight"));
        layout.attention_key_norm = Some(format!("{prefix}.attn_k_norm.weight"));
        layout.attention_value_weight = Some(format!("{prefix}.attn_v.weight"));
        layout.attention_output_weight = Some(format!("{prefix}.attn_output.weight"));
        layout.attention_post_norm = Some(format!("{prefix}.post_attention_norm.weight"));
        layout.feed_forward_gate_weight = Some(format!("{prefix}.ffn_gate.weight"));
        layout.feed_forward_down_weight = Some(format!("{prefix}.ffn_down.weight"));
        layout.feed_forward_up_weight = Some(format!("{prefix}.ffn_up.weight"));

        let embedding_hidden_projection =
            HostMatrix::load(artifact, format!("{prefix}.nextn.eh_proj.weight").as_str())?;
        let embedding_norm =
            load_dense_vector(artifact, format!("{prefix}.nextn.enorm.weight").as_str())?;
        let target_hidden_norm =
            load_dense_vector(artifact, format!("{prefix}.nextn.hnorm.weight").as_str())?;
        let shared_head_norm = load_dense_vector(
            artifact,
            format!("{prefix}.nextn.shared_head_norm.weight").as_str(),
        )?;
        let layer = CpuQwen35Layer::load(artifact, &layout, metadata)?;
        let weight_residency_bytes = embedding_hidden_projection
            .host_residency_bytes()
            .saturating_add(vec_f32_bytes(embedding_norm.as_slice()))
            .saturating_add(vec_f32_bytes(target_hidden_norm.as_slice()))
            .saturating_add(vec_f32_bytes(shared_head_norm.as_slice()))
            .saturating_add(layer.host_residency_bytes())
            as u64;
        Ok(Self {
            embedding_hidden_projection,
            embedding_norm,
            target_hidden_norm,
            shared_head_norm,
            layer,
            weight_residency_bytes,
        })
    }

    fn initial_state(&self, cache_capacity_tokens: usize) -> CpuQwen38MtpState {
        let CpuQwen35LayerKind::FullAttention(attention) = &self.layer.kind else {
            unreachable!("qwen38 MTP loader always constructs a full-attention layer")
        };
        CpuQwen38MtpState {
            attention: CpuQwen35FullAttentionState::new(cache_capacity_tokens, attention.kv_width),
        }
    }

    fn forward_draft(
        &self,
        model: &CpuQwen35Model,
        state: &mut CpuQwen38MtpState,
        target_hidden: &[f32],
        next_token: TokenId,
        position: usize,
    ) -> Result<CpuQwen35ForwardStep, ReferenceTextGenerationError> {
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let token_embedding = model
            .token_embedding
            .decode_row(next_token.as_u32() as usize)?;
        let normalized_embedding = rms_norm(
            token_embedding.as_slice(),
            self.embedding_norm.as_slice(),
            epsilon,
        );
        let normalized_hidden =
            rms_norm(target_hidden, self.target_hidden_norm.as_slice(), epsilon);
        let mut concatenated = Vec::with_capacity(
            normalized_embedding
                .len()
                .saturating_add(normalized_hidden.len()),
        );
        concatenated.extend_from_slice(normalized_embedding.as_slice());
        concatenated.extend_from_slice(normalized_hidden.as_slice());
        let projected = self
            .embedding_hidden_projection
            .matvec(concatenated.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let CpuQwen35LayerKind::FullAttention(attention) = &self.layer.kind else {
            unreachable!("qwen38 MTP loader always constructs a full-attention layer")
        };
        let hidden = self.layer.forward_full_attention(
            &model.family_metadata,
            model.descriptor.config.hidden_size,
            model.descriptor.config.block.attention.head_count,
            model.descriptor.config.block.attention.head_dim,
            model.descriptor.config.block.attention.rotary_dim,
            attention,
            [position; 3],
            projected,
            &mut state.attention,
        )?;
        let final_hidden = rms_norm(hidden.as_slice(), self.shared_head_norm.as_slice(), epsilon);
        let logits = model
            .output
            .matvec(final_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let bytes_moved = self
            .weight_residency_bytes
            .saturating_add(model.token_embedding.host_residency_bytes() as u64)
            .saturating_add(model.output.host_residency_bytes() as u64);
        Ok(CpuQwen35ForwardStep {
            logits,
            final_hidden,
            kernel_count: 8,
            bytes_moved,
        })
    }
}

#[derive(Clone, Debug)]
struct CpuQwen38MtpState {
    attention: CpuQwen35FullAttentionState,
}

impl CpuQwen38MtpState {
    fn allocated_bytes(&self) -> u64 {
        self.attention.allocated_bytes()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CpuQwen35ForwardStep {
    logits: Vec<f32>,
    final_hidden: Vec<f32>,
    kernel_count: usize,
    bytes_moved: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct CpuQwen35State {
    position: usize,
    layers: Vec<CpuQwen35LayerState>,
}

impl CpuQwen35State {
    fn allocated_bytes(&self) -> u64 {
        let layer_storage = self
            .layers
            .capacity()
            .saturating_mul(std::mem::size_of::<CpuQwen35LayerState>());
        let state_storage = self
            .layers
            .iter()
            .map(|layer| match layer {
                CpuQwen35LayerState::Hybrid(state) => state.allocated_bytes(),
                CpuQwen35LayerState::FullAttention(state) => state.allocated_bytes(),
            })
            .fold(0u64, u64::saturating_add);
        (layer_storage as u64).saturating_add(state_storage)
    }
}

#[derive(Clone, Debug)]
struct CpuQwen35Layer {
    attention_norm: Vec<f32>,
    post_attention_norm: Vec<f32>,
    ffn_gate_up: HostProjectionGroup,
    ffn_down: HostMatrix,
    kind: CpuQwen35LayerKind,
}

impl CpuQwen35Layer {
    fn load(
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let attention_norm = load_dense_vector(artifact, layout.attention_norm.as_str())?;
        let post_attention_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_post_norm.as_deref(), "post_attention_norm")?,
        )?;
        let ffn_gate_up = HostProjectionGroup::load(
            artifact,
            &[
                required_tensor_name(layout.feed_forward_gate_weight.as_deref(), "ffn_gate")?,
                required_tensor_name(layout.feed_forward_up_weight.as_deref(), "ffn_up")?,
            ],
        )?;
        let kind = match layout.layer_kind {
            GgufDecoderLayerKind::Qwen35Hybrid => {
                CpuQwen35LayerKind::Hybrid(CpuQwen35HybridLayer::load(artifact, layout, metadata)?)
            }
            GgufDecoderLayerKind::Qwen35FullAttention => CpuQwen35LayerKind::FullAttention(
                CpuQwen35FullAttentionLayer::load(artifact, layout)?,
            ),
            other => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!("qwen35 cpu runtime does not support layer kind `{other:?}`"),
                });
            }
        };
        Ok(Self {
            attention_norm,
            post_attention_norm,
            ffn_gate_up,
            ffn_down: HostMatrix::load(
                artifact,
                required_tensor_name(layout.feed_forward_down_weight.as_deref(), "ffn_down")?,
            )?,
            kind,
        })
    }

    fn initial_state(&self, cache_capacity_tokens: usize) -> CpuQwen35LayerState {
        match &self.kind {
            CpuQwen35LayerKind::Hybrid(layer) => {
                CpuQwen35LayerState::Hybrid(CpuQwen35HybridState::new(
                    layer.qkv_gate_alpha_beta.rows_per_projection[0]
                        .saturating_mul(layer.conv_kernel.saturating_sub(1)),
                    layer
                        .time_step_rank
                        .saturating_mul(layer.state_size)
                        .saturating_mul(layer.state_size),
                ))
            }
            CpuQwen35LayerKind::FullAttention(layer) => CpuQwen35LayerState::FullAttention(
                CpuQwen35FullAttentionState::new(cache_capacity_tokens, layer.kv_width),
            ),
        }
    }

    fn host_residency_bytes(&self) -> usize {
        vec_f32_bytes(self.attention_norm.as_slice())
            .saturating_add(vec_f32_bytes(self.post_attention_norm.as_slice()))
            .saturating_add(self.ffn_gate_up.host_residency_bytes())
            .saturating_add(self.ffn_down.host_residency_bytes())
            .saturating_add(match &self.kind {
                CpuQwen35LayerKind::Hybrid(layer) => layer.host_residency_bytes(),
                CpuQwen35LayerKind::FullAttention(layer) => layer.host_residency_bytes(),
            })
    }

    fn forward(
        &self,
        family_metadata: &GgufDecoderFamilyMetadata,
        hidden_size: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        mrope_position: [usize; 3],
        input_hidden: Vec<f32>,
        state: &mut CpuQwen35LayerState,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        match (&self.kind, state) {
            (CpuQwen35LayerKind::Hybrid(hybrid), CpuQwen35LayerState::Hybrid(hybrid_state)) => self
                .forward_hybrid(
                    family_metadata,
                    hidden_size,
                    hybrid,
                    mrope_position[0],
                    input_hidden,
                    hybrid_state,
                ),
            (
                CpuQwen35LayerKind::FullAttention(full_attention),
                CpuQwen35LayerState::FullAttention(full_attention_state),
            ) => self.forward_full_attention(
                family_metadata,
                hidden_size,
                head_count,
                head_dim,
                rotary_dim,
                full_attention,
                mrope_position,
                input_hidden,
                full_attention_state,
            ),
            (CpuQwen35LayerKind::Hybrid(_), CpuQwen35LayerState::FullAttention(_))
            | (CpuQwen35LayerKind::FullAttention(_), CpuQwen35LayerState::Hybrid(_)) => Err(
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                    "qwen35 cpu layer/state kind mismatch",
                ))),
            ),
        }
    }

    fn forward_hybrid(
        &self,
        family_metadata: &GgufDecoderFamilyMetadata,
        _hidden_size: usize,
        hybrid: &CpuQwen35HybridLayer,
        _position: usize,
        input_hidden: Vec<f32>,
        state: &mut CpuQwen35HybridState,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let projected = self.forward_hybrid_attention(
            family_metadata,
            hybrid,
            input_hidden.as_slice(),
            state,
            None,
        )?;
        let epsilon = family_metadata.rms_norm_epsilon;
        let post_attention = add_vectors(projected.as_slice(), input_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention_norm = rms_norm(
            post_attention.as_slice(),
            self.post_attention_norm.as_slice(),
            epsilon,
        );
        let gate_up = self
            .ffn_gate_up
            .host_matvec(post_attention_norm.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let ffn = silu_glu(
            gate_up
                .slice(0)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up
                .slice(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
        );
        let ffn_down = self
            .ffn_down
            .matvec(ffn.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        add_vectors(post_attention.as_slice(), ffn_down.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)
    }

    fn forward_hybrid_attention(
        &self,
        family_metadata: &GgufDecoderFamilyMetadata,
        hybrid: &CpuQwen35HybridLayer,
        input_hidden: &[f32],
        state: &mut CpuQwen35HybridState,
        mut trace: Option<Qwen35CpuTraceContext<'_>>,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let tracing = trace.is_some();
        let epsilon = family_metadata.rms_norm_epsilon;
        let hidden_norm = rms_norm(input_hidden, self.attention_norm.as_slice(), epsilon);
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "attn_norm",
            [hidden_norm.len(), 1, 1, 1],
            hidden_norm.as_slice(),
        );
        let projected = hybrid
            .qkv_gate_alpha_beta
            .host_matvec(hidden_norm.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let qkv = projected
            .slice(0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let z = projected
            .slice(1)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let alpha = projected
            .slice(2)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let beta = projected
            .slice(3)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "linear_attn_qkv_mixed",
            [qkv.len(), 1, 1, 1],
            qkv,
        );
        let mut conv = vec![0.0_f32; qkv.len()];
        causal_depthwise_conv1d_step_in_place(
            qkv,
            state.conv_state.as_mut_slice(),
            &hybrid.ssm_conv1d,
            hybrid.conv_kernel,
            conv.as_mut_slice(),
        )?;
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "conv_output_raw",
            [conv.len(), 1, 1, 1],
            conv.as_slice(),
        );
        silu_forward_in_place(conv.as_mut_slice());
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "conv_output_silu",
            [conv.len(), 1, 1, 1],
            conv.as_slice(),
        );

        let mut alpha_softplus = if tracing {
            vec![0.0_f32; alpha.len()]
        } else {
            Vec::new()
        };
        let mut gate_preexp = vec![0.0_f32; alpha.len()];
        let mut decay = vec![0.0_f32; alpha.len()];
        let mut beta_sigmoid = vec![0.0_f32; beta.len()];
        for index in 0..alpha.len() {
            let softplus = softplus(alpha[index] + hybrid.ssm_dt[index]);
            if tracing {
                alpha_softplus[index] = softplus;
            }
            let gate = softplus * hybrid.ssm_a[index];
            gate_preexp[index] = gate;
            decay[index] = gate.exp();
            beta_sigmoid[index] = sigmoid(beta[index]);
        }
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "a_softplus",
            [alpha_softplus.len(), 1, 1, 1],
            alpha_softplus.as_slice(),
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "gate",
            [gate_preexp.len(), 1, 1, 1],
            gate_preexp.as_slice(),
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "beta_sigmoid",
            [1, beta_sigmoid.len(), 1, 1],
            beta_sigmoid.as_slice(),
        );

        let mut qkv_norm = vec![0.0_f32; v_offset.saturating_add(v_size)];
        qkv_norm[..q_size].copy_from_slice(&conv[..q_size]);
        qkv_norm[q_size..q_size + k_size].copy_from_slice(&conv[q_size..q_size + k_size]);
        qkv_norm[v_offset..v_offset + v_size].copy_from_slice(&conv[v_offset..v_offset + v_size]);

        let mut gated_delta = vec![0.0_f32; v_size];
        let mut norm_q = vec![0.0_f32; hybrid.state_size];
        let mut norm_k = vec![0.0_f32; hybrid.state_size];
        let mut kv_mem = vec![0.0_f32; hybrid.state_size];
        let mut delta = vec![0.0_f32; hybrid.state_size];
        let mut normalized_q = if tracing {
            vec![0.0_f32; q_size]
        } else {
            Vec::new()
        };
        let mut normalized_k = if tracing {
            vec![0.0_f32; k_size]
        } else {
            Vec::new()
        };
        let mut normalized_key_heads = if tracing {
            vec![false; hybrid.group_count]
        } else {
            Vec::new()
        };
        let repeat_factor = hybrid.time_step_rank / hybrid.group_count.max(1);
        for value_head_index in 0..hybrid.time_step_rank {
            let key_head_index = if hybrid.v_head_reordered {
                value_head_index % hybrid.group_count.max(1)
            } else if repeat_factor > 0 {
                value_head_index / repeat_factor
            } else {
                0
            };
            let q = &qkv_norm
                [key_head_index * hybrid.state_size..(key_head_index + 1) * hybrid.state_size];
            let k = &qkv_norm[q_size + key_head_index * hybrid.state_size
                ..q_size + (key_head_index + 1) * hybrid.state_size];
            let v = &qkv_norm[v_offset + value_head_index * hybrid.state_size
                ..v_offset + (value_head_index + 1) * hybrid.state_size];
            let state_slice = &mut state.delta_state[value_head_index
                .saturating_mul(hybrid.state_size)
                .saturating_mul(hybrid.state_size)
                ..(value_head_index + 1)
                    .saturating_mul(hybrid.state_size)
                    .saturating_mul(hybrid.state_size)];
            let output_slice = &mut gated_delta
                [value_head_index * hybrid.state_size..(value_head_index + 1) * hybrid.state_size];
            delta_net_autoregressive_step_in_place(
                q,
                k,
                v,
                decay[value_head_index],
                beta_sigmoid[value_head_index],
                state_slice,
                norm_q.as_mut_slice(),
                norm_k.as_mut_slice(),
                kv_mem.as_mut_slice(),
                delta.as_mut_slice(),
                output_slice,
            );
            if tracing && !normalized_key_heads[key_head_index] {
                let start = key_head_index.saturating_mul(hybrid.state_size);
                let end = start.saturating_add(hybrid.state_size);
                let q_unscale = (hybrid.state_size as f32).sqrt();
                for (destination, value) in normalized_q[start..end]
                    .iter_mut()
                    .zip(norm_q.iter().copied())
                {
                    *destination = value * q_unscale;
                }
                normalized_k[start..end].copy_from_slice(norm_k.as_slice());
                normalized_key_heads[key_head_index] = true;
            }
        }
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "q_conv_predelta",
            [hybrid.state_size, hybrid.group_count, 1, 1],
            normalized_q.as_slice(),
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "k_conv_predelta",
            [hybrid.state_size, hybrid.group_count, 1, 1],
            normalized_k.as_slice(),
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "v_conv_predelta",
            [hybrid.state_size, v_size / hybrid.state_size.max(1), 1, 1],
            &conv[v_offset..v_offset + v_size],
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "new_state",
            [
                hybrid.state_size,
                hybrid.state_size,
                hybrid.time_step_rank,
                1,
            ],
            state.delta_state.as_slice(),
        );
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "attn_output",
            [hybrid.state_size, v_size / hybrid.state_size.max(1), 1, 1],
            gated_delta.as_slice(),
        );

        let hybrid_norm = per_head_rms_norm(
            gated_delta.as_slice(),
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.ssm_norm.as_slice(),
            epsilon,
        );
        let activated = hybrid_norm
            .iter()
            .copied()
            .zip(z.iter().copied())
            .map(|(value, gate)| value * silu_scalar(gate))
            .collect::<Vec<_>>();
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "final_output",
            [activated.len(), 1, 1, 1],
            activated.as_slice(),
        );
        let projected = hybrid
            .ssm_out
            .matvec(activated.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        push_qwen35_cpu_trace_tensor(
            &mut trace,
            "linear_attn_out",
            [projected.len(), 1, 1, 1],
            projected.as_slice(),
        );
        Ok(projected)
    }

    fn forward_full_attention(
        &self,
        family_metadata: &GgufDecoderFamilyMetadata,
        _hidden_size: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        full_attention: &CpuQwen35FullAttentionLayer,
        mrope_position: [usize; 3],
        input_hidden: Vec<f32>,
        state: &mut CpuQwen35FullAttentionState,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        let epsilon = family_metadata.rms_norm_epsilon;
        let query_width = head_count.saturating_mul(head_dim);
        let kv_head_count = full_attention.kv_width / head_dim.max(1);
        let projected = full_attention
            .qkv
            .host_matvec(
                rms_norm(
                    input_hidden.as_slice(),
                    self.attention_norm.as_slice(),
                    epsilon,
                )
                .as_slice(),
            )
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let query_gate = projected
            .slice(0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let key = projected
            .slice(1)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let value = projected
            .slice(2)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if query_gate.len() != query_width.saturating_mul(2) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 cpu full-attention query/gate width mismatch: expected {}, actual {}",
                    query_width.saturating_mul(2),
                    query_gate.len(),
                )),
            ));
        }
        let mut query = vec![0.0_f32; query_width];
        let mut gate = vec![0.0_f32; query_width];
        for head_index in 0..head_count {
            let source_base = head_index.saturating_mul(head_dim).saturating_mul(2);
            let destination = head_index.saturating_mul(head_dim);
            query[destination..destination + head_dim]
                .copy_from_slice(&query_gate[source_base..source_base + head_dim]);
            gate[destination..destination + head_dim].copy_from_slice(
                &query_gate[source_base + head_dim..source_base + head_dim.saturating_mul(2)],
            );
        }
        query = per_head_rms_norm(
            query.as_slice(),
            head_count,
            head_dim,
            full_attention.query_norm.as_slice(),
            epsilon,
        );
        let mut key = per_head_rms_norm(
            key,
            kv_head_count,
            head_dim,
            full_attention.key_norm.as_slice(),
            epsilon,
        );
        let attention_scale = qwen35_attention_scale(family_metadata, head_dim);
        apply_rope_neox_mrope(
            query.as_mut_slice(),
            head_count,
            head_dim,
            rotary_dim,
            mrope_position,
            family_metadata,
        );
        apply_rope_neox_mrope(
            key.as_mut_slice(),
            kv_head_count,
            head_dim,
            rotary_dim,
            mrope_position,
            family_metadata,
        );
        let attention = attend_full_attention(
            query.as_slice(),
            key.as_slice(),
            value,
            state.entries.as_slice(),
            head_count,
            kv_head_count,
            head_dim,
            attention_scale,
            family_metadata.sliding_window,
        );
        state.entries.push(Qwen35FullAttentionEntry {
            key: key.clone(),
            value: value.to_vec(),
        });
        let gated = attention
            .iter()
            .copied()
            .zip(gate.iter().copied())
            .map(|(value, gate)| value * sigmoid(gate))
            .collect::<Vec<_>>();
        let projected = full_attention
            .output
            .matvec(gated.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention = add_vectors(projected.as_slice(), input_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention_norm = rms_norm(
            post_attention.as_slice(),
            self.post_attention_norm.as_slice(),
            epsilon,
        );
        let gate_up = self
            .ffn_gate_up
            .host_matvec(post_attention_norm.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let ffn = silu_glu(
            gate_up
                .slice(0)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up
                .slice(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
        );
        let ffn_down = self
            .ffn_down
            .matvec(ffn.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        add_vectors(post_attention.as_slice(), ffn_down.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)
    }
}

#[derive(Clone, Debug)]
enum CpuQwen35LayerKind {
    Hybrid(CpuQwen35HybridLayer),
    FullAttention(CpuQwen35FullAttentionLayer),
}

#[derive(Clone, Debug, PartialEq)]
enum CpuQwen35LayerState {
    Hybrid(CpuQwen35HybridState),
    FullAttention(CpuQwen35FullAttentionState),
}

#[derive(Clone, Debug)]
struct CpuQwen35HybridLayer {
    qkv_gate_alpha_beta: HostProjectionGroup,
    ssm_conv1d: DenseMatrix,
    ssm_a: Vec<f32>,
    ssm_dt: Vec<f32>,
    ssm_norm: Vec<f32>,
    ssm_out: HostMatrix,
    q_scale: Vec<f32>,
    k_scale: Vec<f32>,
    inner_size: usize,
    state_size: usize,
    group_count: usize,
    time_step_rank: usize,
    conv_kernel: usize,
    v_head_reordered: bool,
}

impl CpuQwen35HybridLayer {
    fn load(
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let ssm_conv1d = load_dense_matrix(
            artifact,
            required_tensor_name(layout.ssm_conv1d_weight.as_deref(), "ssm_conv1d")?,
        )?;
        let ssm_a = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_a.as_deref(), "ssm_a")?,
        )?;
        let ssm_dt = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_dt.as_deref(), "ssm_dt")?,
        )?;
        let ssm_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_norm_weight.as_deref(), "ssm_norm")?,
        )?;
        let state_size = family_fact_usize(metadata, "qwen35.ssm.state_size")?;
        let q_scale = vec![1.0_f32 / state_size as f32; state_size];
        let k_scale = vec![1.0_f32 / (state_size as f32).sqrt(); state_size];
        Ok(Self {
            qkv_gate_alpha_beta: HostProjectionGroup::load(
                artifact,
                &[
                    required_tensor_name(layout.attention_qkv_weight.as_deref(), "attn_qkv")?,
                    required_tensor_name(layout.attention_gate_weight.as_deref(), "attn_gate")?,
                    required_tensor_name(layout.ssm_alpha_weight.as_deref(), "ssm_alpha")?,
                    required_tensor_name(layout.ssm_beta_weight.as_deref(), "ssm_beta")?,
                ],
            )?,
            ssm_conv1d,
            ssm_a,
            ssm_dt,
            ssm_norm,
            ssm_out: HostMatrix::load(
                artifact,
                required_tensor_name(layout.ssm_out_weight.as_deref(), "ssm_out")?,
            )?,
            q_scale,
            k_scale,
            inner_size: family_fact_usize(metadata, "qwen35.ssm.inner_size")?,
            state_size,
            group_count: family_fact_usize(metadata, "qwen35.ssm.group_count")?,
            time_step_rank: family_fact_usize(metadata, "qwen35.ssm.time_step_rank")?,
            conv_kernel: family_fact_usize(metadata, "qwen35.ssm.conv_kernel")?,
            v_head_reordered: family_fact_bool_or_default(
                metadata,
                "qwen35.ssm.v_head_reordered",
                true,
            ),
        })
    }

    fn host_residency_bytes(&self) -> usize {
        self.qkv_gate_alpha_beta
            .host_residency_bytes()
            .saturating_add(self.ssm_conv1d.host_residency_bytes())
            .saturating_add(vec_f32_bytes(self.ssm_a.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_dt.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_norm.as_slice()))
            .saturating_add(self.ssm_out.host_residency_bytes())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CpuQwen35HybridState {
    conv_state: Vec<f32>,
    delta_state: Vec<f32>,
}

impl CpuQwen35HybridState {
    fn new(conv_len: usize, delta_len: usize) -> Self {
        Self {
            conv_state: vec![0.0; conv_len],
            delta_state: vec![0.0; delta_len],
        }
    }

    fn allocated_bytes(&self) -> u64 {
        self.conv_state
            .capacity()
            .saturating_add(self.delta_state.capacity())
            .saturating_mul(std::mem::size_of::<f32>()) as u64
    }
}

#[derive(Clone, Debug)]
struct CpuQwen35FullAttentionLayer {
    qkv: HostProjectionGroup,
    query_norm: Vec<f32>,
    key_norm: Vec<f32>,
    output: HostMatrix,
    kv_width: usize,
}

impl CpuQwen35FullAttentionLayer {
    fn load(
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        let query_name = required_tensor_name(layout.attention_query_weight.as_deref(), "attn_q")?;
        let key_name = required_tensor_name(layout.attention_key_weight.as_deref(), "attn_k")?;
        let value_name = required_tensor_name(layout.attention_value_weight.as_deref(), "attn_v")?;
        let qkv = HostProjectionGroup::load(artifact, &[query_name, key_name, value_name])?;
        let query_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_query_norm.as_deref(), "attn_q_norm")?,
        )?;
        let key_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_key_norm.as_deref(), "attn_k_norm")?,
        )?;
        Ok(Self {
            kv_width: qkv.rows_per_projection[1],
            qkv,
            query_norm,
            key_norm,
            output: HostMatrix::load(
                artifact,
                required_tensor_name(layout.attention_output_weight.as_deref(), "attn_output")?,
            )?,
        })
    }

    fn host_residency_bytes(&self) -> usize {
        self.qkv
            .host_residency_bytes()
            .saturating_add(vec_f32_bytes(self.query_norm.as_slice()))
            .saturating_add(vec_f32_bytes(self.key_norm.as_slice()))
            .saturating_add(self.output.host_residency_bytes())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CpuQwen35FullAttentionState {
    entries: Vec<Qwen35FullAttentionEntry>,
}

impl CpuQwen35FullAttentionState {
    fn new(cache_capacity_tokens: usize, _kv_width: usize) -> Self {
        Self {
            entries: Vec::with_capacity(cache_capacity_tokens),
        }
    }

    fn allocated_bytes(&self) -> u64 {
        let entry_storage =
            self.entries
                .capacity()
                .saturating_mul(std::mem::size_of::<Qwen35FullAttentionEntry>()) as u64;
        self.entries.iter().fold(entry_storage, |bytes, entry| {
            bytes
                .saturating_add(
                    entry
                        .key
                        .capacity()
                        .saturating_mul(std::mem::size_of::<f32>()) as u64,
                )
                .saturating_add(
                    entry
                        .value
                        .capacity()
                        .saturating_mul(std::mem::size_of::<f32>()) as u64,
                )
        })
    }
}

#[derive(Clone, Debug)]
struct MetalQwen35LayerStep {
    hidden: Vec<f32>,
    kernel_count: usize,
    bytes_moved: u64,
}

#[derive(Clone, Debug)]
struct MetalQwen35Layer {
    attention_norm: Vec<f32>,
    post_attention_norm: Vec<f32>,
    ffn_gate_up: MetalQuantizedProjectionGroup,
    ffn_down: MetalQuantizedMatrix,
    kind: MetalQwen35LayerKind,
}

impl MetalQwen35Layer {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let attention_norm = load_dense_vector(artifact, layout.attention_norm.as_str())?;
        let post_attention_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_post_norm.as_deref(), "post_attention_norm")?,
        )?;
        let ffn_gate_up = MetalQuantizedProjectionGroup::load(
            backend,
            artifact,
            &[
                required_tensor_name(layout.feed_forward_gate_weight.as_deref(), "ffn_gate")?,
                required_tensor_name(layout.feed_forward_up_weight.as_deref(), "ffn_up")?,
            ],
        )?;
        let kind = match layout.layer_kind {
            GgufDecoderLayerKind::Qwen35Hybrid => MetalQwen35LayerKind::Hybrid(
                MetalQwen35HybridLayer::load(backend, artifact, layout, metadata)?,
            ),
            GgufDecoderLayerKind::Qwen35FullAttention => MetalQwen35LayerKind::FullAttention(
                MetalQwen35FullAttentionLayer::load(backend, artifact, layout)?,
            ),
            other => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "qwen35 metal runtime does not support layer kind `{other:?}`"
                    ),
                });
            }
        };
        Ok(Self {
            attention_norm,
            post_attention_norm,
            ffn_gate_up,
            ffn_down: MetalQuantizedMatrix::load(
                backend,
                artifact,
                required_tensor_name(layout.feed_forward_down_weight.as_deref(), "ffn_down")?,
            )?,
            kind,
        })
    }

    fn initial_state(&self, cache_capacity_tokens: usize) -> MetalQwen35LayerState {
        match &self.kind {
            MetalQwen35LayerKind::Hybrid(layer) => {
                MetalQwen35LayerState::Hybrid(MetalQwen35HybridState::new(
                    layer.qkv_gate_alpha_beta.rows_per_projection[0]
                        .saturating_mul(layer.conv_kernel.saturating_sub(1)),
                    layer
                        .time_step_rank
                        .saturating_mul(layer.state_size)
                        .saturating_mul(layer.state_size),
                ))
            }
            MetalQwen35LayerKind::FullAttention(layer) => MetalQwen35LayerState::FullAttention(
                MetalQwen35FullAttentionState::new(cache_capacity_tokens, layer.kv_width),
            ),
        }
    }

    fn device_residency_bytes(&self) -> usize {
        self.ffn_gate_up
            .device_residency_bytes()
            .saturating_add(self.ffn_down.device_residency_bytes())
            .saturating_add(match &self.kind {
                MetalQwen35LayerKind::Hybrid(layer) => layer.device_residency_bytes(),
                MetalQwen35LayerKind::FullAttention(layer) => layer.device_residency_bytes(),
            })
    }

    fn projection_count(&self) -> usize {
        self.ffn_gate_up
            .parts
            .len()
            .saturating_add(1)
            .saturating_add(match &self.kind {
                MetalQwen35LayerKind::Hybrid(layer) => {
                    layer.qkv_gate_alpha_beta.parts.len().saturating_add(1)
                }
                MetalQwen35LayerKind::FullAttention(layer) => {
                    layer.qkv.parts.len().saturating_add(1)
                }
            })
    }

    fn native_projection_count(&self) -> usize {
        self.ffn_gate_up
            .parts
            .iter()
            .filter(|matrix| matrix.is_native())
            .count()
            .saturating_add(usize::from(self.ffn_down.is_native()))
            .saturating_add(match &self.kind {
                MetalQwen35LayerKind::Hybrid(layer) => layer
                    .qkv_gate_alpha_beta
                    .parts
                    .iter()
                    .filter(|matrix| matrix.is_native())
                    .count()
                    .saturating_add(usize::from(layer.ssm_out.is_native())),
                MetalQwen35LayerKind::FullAttention(layer) => layer
                    .qkv
                    .parts
                    .iter()
                    .filter(|matrix| matrix.is_native())
                    .count()
                    .saturating_add(usize::from(layer.output.is_native())),
            })
    }

    fn forward(
        &self,
        backend: &mut MetalBackend,
        family_metadata: &GgufDecoderFamilyMetadata,
        hidden_size: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        mrope_position: [usize; 3],
        input_hidden: Vec<f32>,
        state: &mut MetalQwen35LayerState,
    ) -> Result<MetalQwen35LayerStep, ReferenceTextGenerationError> {
        match (&self.kind, state) {
            (MetalQwen35LayerKind::Hybrid(hybrid), MetalQwen35LayerState::Hybrid(hybrid_state)) => {
                self.forward_hybrid(
                    backend,
                    family_metadata,
                    hidden_size,
                    hybrid,
                    mrope_position[0],
                    input_hidden,
                    hybrid_state,
                )
            }
            (
                MetalQwen35LayerKind::FullAttention(full_attention),
                MetalQwen35LayerState::FullAttention(full_attention_state),
            ) => self.forward_full_attention(
                backend,
                family_metadata,
                hidden_size,
                head_count,
                head_dim,
                rotary_dim,
                full_attention,
                mrope_position,
                input_hidden,
                full_attention_state,
            ),
            (MetalQwen35LayerKind::Hybrid(_), MetalQwen35LayerState::FullAttention(_))
            | (MetalQwen35LayerKind::FullAttention(_), MetalQwen35LayerState::Hybrid(_)) => Err(
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                    "qwen35 metal layer/state kind mismatch",
                ))),
            ),
        }
    }

    fn forward_hybrid(
        &self,
        backend: &mut MetalBackend,
        family_metadata: &GgufDecoderFamilyMetadata,
        _hidden_size: usize,
        hybrid: &MetalQwen35HybridLayer,
        _position: usize,
        input_hidden: Vec<f32>,
        state: &mut MetalQwen35HybridState,
    ) -> Result<MetalQwen35LayerStep, ReferenceTextGenerationError> {
        let epsilon = family_metadata.rms_norm_epsilon;
        let hidden_norm = rms_norm(
            input_hidden.as_slice(),
            self.attention_norm.as_slice(),
            epsilon,
        );
        let projected = hybrid
            .qkv_gate_alpha_beta
            .matvec(backend, hidden_norm.as_slice())?;
        let qkv = projected
            .outputs
            .slice(0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let z = projected
            .outputs
            .slice(1)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let alpha = projected
            .outputs
            .slice(2)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let beta = projected
            .outputs
            .slice(3)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);

        let mut conv = vec![0.0_f32; qkv.len()];
        causal_depthwise_conv1d_step_in_place(
            qkv,
            state.conv_state.as_mut_slice(),
            &hybrid.ssm_conv1d,
            hybrid.conv_kernel,
            conv.as_mut_slice(),
        )?;
        silu_forward_in_place(conv.as_mut_slice());

        let mut decay = vec![0.0_f32; alpha.len()];
        let mut beta_sigmoid = vec![0.0_f32; beta.len()];
        for index in 0..alpha.len() {
            let gate = softplus(alpha[index] + hybrid.ssm_dt[index]) * hybrid.ssm_a[index];
            decay[index] = gate.exp();
            beta_sigmoid[index] = sigmoid(beta[index]);
        }

        let mut qkv_norm = vec![0.0_f32; v_offset.saturating_add(v_size)];
        qkv_norm[..q_size].copy_from_slice(&conv[..q_size]);
        qkv_norm[q_size..q_size + k_size].copy_from_slice(&conv[q_size..q_size + k_size]);
        qkv_norm[v_offset..v_offset + v_size].copy_from_slice(&conv[v_offset..v_offset + v_size]);

        let mut gated_delta = vec![0.0_f32; v_size];
        let mut norm_q = vec![0.0_f32; hybrid.state_size];
        let mut norm_k = vec![0.0_f32; hybrid.state_size];
        let mut kv_mem = vec![0.0_f32; hybrid.state_size];
        let mut delta = vec![0.0_f32; hybrid.state_size];
        let repeat_factor = hybrid.time_step_rank / hybrid.group_count.max(1);
        for value_head_index in 0..hybrid.time_step_rank {
            let key_head_index = if hybrid.v_head_reordered {
                value_head_index % hybrid.group_count.max(1)
            } else if repeat_factor > 0 {
                value_head_index / repeat_factor
            } else {
                0
            };
            let q = &qkv_norm
                [key_head_index * hybrid.state_size..(key_head_index + 1) * hybrid.state_size];
            let k = &qkv_norm[q_size + key_head_index * hybrid.state_size
                ..q_size + (key_head_index + 1) * hybrid.state_size];
            let v = &qkv_norm[v_offset + value_head_index * hybrid.state_size
                ..v_offset + (value_head_index + 1) * hybrid.state_size];
            let state_slice = &mut state.delta_state[value_head_index
                .saturating_mul(hybrid.state_size)
                .saturating_mul(hybrid.state_size)
                ..(value_head_index + 1)
                    .saturating_mul(hybrid.state_size)
                    .saturating_mul(hybrid.state_size)];
            let output_slice = &mut gated_delta
                [value_head_index * hybrid.state_size..(value_head_index + 1) * hybrid.state_size];
            delta_net_autoregressive_step_in_place(
                q,
                k,
                v,
                decay[value_head_index],
                beta_sigmoid[value_head_index],
                state_slice,
                norm_q.as_mut_slice(),
                norm_k.as_mut_slice(),
                kv_mem.as_mut_slice(),
                delta.as_mut_slice(),
                output_slice,
            );
        }

        let hybrid_norm = per_head_rms_norm(
            gated_delta.as_slice(),
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.ssm_norm.as_slice(),
            epsilon,
        );
        let activated = hybrid_norm
            .iter()
            .copied()
            .zip(z.iter().copied())
            .map(|(value, gate)| value * silu_scalar(gate))
            .collect::<Vec<_>>();
        let projected_step = hybrid.ssm_out.matvec(backend, activated.as_slice())?;
        let post_attention = add_vectors(projected_step.values.as_slice(), input_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention_norm = rms_norm(
            post_attention.as_slice(),
            self.post_attention_norm.as_slice(),
            epsilon,
        );
        let gate_up = self
            .ffn_gate_up
            .matvec(backend, post_attention_norm.as_slice())?;
        let ffn = silu_glu(
            gate_up
                .outputs
                .slice(0)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up
                .outputs
                .slice(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
        );
        let ffn_down = self.ffn_down.matvec(backend, ffn.as_slice())?;
        let final_hidden = add_vectors(post_attention.as_slice(), ffn_down.values.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok(MetalQwen35LayerStep {
            hidden: final_hidden,
            kernel_count: projected
                .kernel_count
                .saturating_add(projected_step.kernel_count)
                .saturating_add(gate_up.kernel_count)
                .saturating_add(ffn_down.kernel_count),
            bytes_moved: projected
                .bytes_moved
                .saturating_add(projected_step.bytes_moved)
                .saturating_add(gate_up.bytes_moved)
                .saturating_add(ffn_down.bytes_moved),
        })
    }

    fn forward_full_attention(
        &self,
        backend: &mut MetalBackend,
        family_metadata: &GgufDecoderFamilyMetadata,
        _hidden_size: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        full_attention: &MetalQwen35FullAttentionLayer,
        mrope_position: [usize; 3],
        input_hidden: Vec<f32>,
        state: &mut MetalQwen35FullAttentionState,
    ) -> Result<MetalQwen35LayerStep, ReferenceTextGenerationError> {
        let epsilon = family_metadata.rms_norm_epsilon;
        let query_width = head_count.saturating_mul(head_dim);
        let kv_head_count = full_attention.kv_width / head_dim.max(1);
        let projected = full_attention.qkv.matvec(
            backend,
            rms_norm(
                input_hidden.as_slice(),
                self.attention_norm.as_slice(),
                epsilon,
            )
            .as_slice(),
        )?;
        let query_gate = projected
            .outputs
            .slice(0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let key = projected
            .outputs
            .slice(1)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let value = projected
            .outputs
            .slice(2)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if query_gate.len() != query_width.saturating_mul(2) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 metal full-attention query/gate width mismatch: expected {}, actual {}",
                    query_width.saturating_mul(2),
                    query_gate.len(),
                )),
            ));
        }
        let mut query = vec![0.0_f32; query_width];
        let mut gate = vec![0.0_f32; query_width];
        for head_index in 0..head_count {
            let source_base = head_index.saturating_mul(head_dim).saturating_mul(2);
            let destination = head_index.saturating_mul(head_dim);
            query[destination..destination + head_dim]
                .copy_from_slice(&query_gate[source_base..source_base + head_dim]);
            gate[destination..destination + head_dim].copy_from_slice(
                &query_gate[source_base + head_dim..source_base + head_dim.saturating_mul(2)],
            );
        }
        query = per_head_rms_norm(
            query.as_slice(),
            head_count,
            head_dim,
            full_attention.query_norm.as_slice(),
            epsilon,
        );
        let mut key = per_head_rms_norm(
            key,
            kv_head_count,
            head_dim,
            full_attention.key_norm.as_slice(),
            epsilon,
        );
        let attention_scale = qwen35_attention_scale(family_metadata, head_dim);
        apply_rope_neox_mrope(
            query.as_mut_slice(),
            head_count,
            head_dim,
            rotary_dim,
            mrope_position,
            family_metadata,
        );
        apply_rope_neox_mrope(
            key.as_mut_slice(),
            kv_head_count,
            head_dim,
            rotary_dim,
            mrope_position,
            family_metadata,
        );
        let attention = attend_full_attention(
            query.as_slice(),
            key.as_slice(),
            value,
            state.entries.as_slice(),
            head_count,
            kv_head_count,
            head_dim,
            attention_scale,
            family_metadata.sliding_window,
        );
        state.entries.push(Qwen35FullAttentionEntry {
            key: key.clone(),
            value: value.to_vec(),
        });
        let gated = attention
            .iter()
            .copied()
            .zip(gate.iter().copied())
            .map(|(value, gate)| value * sigmoid(gate))
            .collect::<Vec<_>>();
        let projected_step = full_attention.output.matvec(backend, gated.as_slice())?;
        let post_attention = add_vectors(projected_step.values.as_slice(), input_hidden.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention_norm = rms_norm(
            post_attention.as_slice(),
            self.post_attention_norm.as_slice(),
            epsilon,
        );
        let gate_up = self
            .ffn_gate_up
            .matvec(backend, post_attention_norm.as_slice())?;
        let ffn = silu_glu(
            gate_up
                .outputs
                .slice(0)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up
                .outputs
                .slice(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
        );
        let ffn_down = self.ffn_down.matvec(backend, ffn.as_slice())?;
        let final_hidden = add_vectors(post_attention.as_slice(), ffn_down.values.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok(MetalQwen35LayerStep {
            hidden: final_hidden,
            kernel_count: projected
                .kernel_count
                .saturating_add(projected_step.kernel_count)
                .saturating_add(gate_up.kernel_count)
                .saturating_add(ffn_down.kernel_count),
            bytes_moved: projected
                .bytes_moved
                .saturating_add(projected_step.bytes_moved)
                .saturating_add(gate_up.bytes_moved)
                .saturating_add(ffn_down.bytes_moved),
        })
    }
}

#[derive(Clone, Debug)]
enum MetalQwen35LayerKind {
    Hybrid(MetalQwen35HybridLayer),
    FullAttention(MetalQwen35FullAttentionLayer),
}

#[derive(Clone, Debug)]
enum MetalQwen35LayerState {
    Hybrid(MetalQwen35HybridState),
    FullAttention(MetalQwen35FullAttentionState),
}

type MetalQwen35HybridState = CpuQwen35HybridState;
type MetalQwen35FullAttentionState = CpuQwen35FullAttentionState;

#[derive(Clone, Debug)]
struct MetalQwen35HybridLayer {
    qkv_gate_alpha_beta: MetalQuantizedProjectionGroup,
    ssm_conv1d: DenseMatrix,
    ssm_a: Vec<f32>,
    ssm_dt: Vec<f32>,
    ssm_norm: Vec<f32>,
    ssm_out: MetalQuantizedMatrix,
    q_scale: Vec<f32>,
    k_scale: Vec<f32>,
    inner_size: usize,
    state_size: usize,
    group_count: usize,
    time_step_rank: usize,
    conv_kernel: usize,
    v_head_reordered: bool,
}

impl MetalQwen35HybridLayer {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let ssm_conv1d = load_dense_matrix(
            artifact,
            required_tensor_name(layout.ssm_conv1d_weight.as_deref(), "ssm_conv1d")?,
        )?;
        let ssm_a = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_a.as_deref(), "ssm_a")?,
        )?;
        let ssm_dt = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_dt.as_deref(), "ssm_dt")?,
        )?;
        let ssm_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_norm_weight.as_deref(), "ssm_norm")?,
        )?;
        let state_size = family_fact_usize(metadata, "qwen35.ssm.state_size")?;
        let q_scale = vec![1.0_f32 / state_size as f32; state_size];
        let k_scale = vec![1.0_f32 / (state_size as f32).sqrt(); state_size];
        Ok(Self {
            qkv_gate_alpha_beta: MetalQuantizedProjectionGroup::load(
                backend,
                artifact,
                &[
                    required_tensor_name(layout.attention_qkv_weight.as_deref(), "attn_qkv")?,
                    required_tensor_name(layout.attention_gate_weight.as_deref(), "attn_gate")?,
                    required_tensor_name(layout.ssm_alpha_weight.as_deref(), "ssm_alpha")?,
                    required_tensor_name(layout.ssm_beta_weight.as_deref(), "ssm_beta")?,
                ],
            )?,
            ssm_conv1d,
            ssm_a,
            ssm_dt,
            ssm_norm,
            ssm_out: MetalQuantizedMatrix::load(
                backend,
                artifact,
                required_tensor_name(layout.ssm_out_weight.as_deref(), "ssm_out")?,
            )?,
            q_scale,
            k_scale,
            inner_size: family_fact_usize(metadata, "qwen35.ssm.inner_size")?,
            state_size,
            group_count: family_fact_usize(metadata, "qwen35.ssm.group_count")?,
            time_step_rank: family_fact_usize(metadata, "qwen35.ssm.time_step_rank")?,
            conv_kernel: family_fact_usize(metadata, "qwen35.ssm.conv_kernel")?,
            v_head_reordered: family_fact_bool_or_default(
                metadata,
                "qwen35.ssm.v_head_reordered",
                true,
            ),
        })
    }

    fn device_residency_bytes(&self) -> usize {
        self.qkv_gate_alpha_beta
            .device_residency_bytes()
            .saturating_add(self.ssm_out.device_residency_bytes())
    }
}

#[derive(Clone, Debug)]
struct MetalQwen35FullAttentionLayer {
    qkv: MetalQuantizedProjectionGroup,
    query_norm: Vec<f32>,
    key_norm: Vec<f32>,
    output: MetalQuantizedMatrix,
    kv_width: usize,
}

impl MetalQwen35FullAttentionLayer {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        let query_name = required_tensor_name(layout.attention_query_weight.as_deref(), "attn_q")?;
        let key_name = required_tensor_name(layout.attention_key_weight.as_deref(), "attn_k")?;
        let value_name = required_tensor_name(layout.attention_value_weight.as_deref(), "attn_v")?;
        let qkv = MetalQuantizedProjectionGroup::load(
            backend,
            artifact,
            &[query_name, key_name, value_name],
        )?;
        let query_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_query_norm.as_deref(), "attn_q_norm")?,
        )?;
        let key_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_key_norm.as_deref(), "attn_k_norm")?,
        )?;
        Ok(Self {
            kv_width: qkv.rows_per_projection[1],
            qkv,
            query_norm,
            key_norm,
            output: MetalQuantizedMatrix::load(
                backend,
                artifact,
                required_tensor_name(layout.attention_output_weight.as_deref(), "attn_output")?,
            )?,
        })
    }

    fn device_residency_bytes(&self) -> usize {
        self.qkv
            .device_residency_bytes()
            .saturating_add(self.output.device_residency_bytes())
    }
}

#[derive(Clone, Debug)]
struct HostProjectionGroup {
    parts: Vec<HostMatrix>,
    rows_per_projection: Vec<usize>,
    columns: usize,
}

impl HostProjectionGroup {
    fn load(artifact: &GgufBlobArtifact, names: &[&str]) -> Result<Self, ModelLoadError> {
        let mut columns = None;
        let mut rows_per_projection = Vec::with_capacity(names.len());
        let mut parts = Vec::with_capacity(names.len());
        for name in names {
            let matrix = HostMatrix::load(artifact, name)?;
            if let Some(expected_columns) = columns {
                if matrix.columns() != expected_columns {
                    return Err(ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: format!(
                            "packed qwen35 cpu projection requires matching input width; `{name}` had {} but expected {}",
                            matrix.columns(),
                            expected_columns,
                        ),
                    });
                }
            } else {
                columns = Some(matrix.columns());
            }
            rows_per_projection.push(matrix.rows());
            parts.push(matrix);
        }
        Ok(Self {
            parts,
            rows_per_projection,
            columns: columns.unwrap_or(0),
        })
    }

    fn host_residency_bytes(&self) -> usize {
        self.parts
            .iter()
            .map(HostMatrix::host_residency_bytes)
            .sum()
    }

    fn host_matvec(&self, input: &[f32]) -> Result<ProjectionOutputs, crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
                "packed projection host matvec input width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let mut values = Vec::with_capacity(
            self.rows_per_projection
                .iter()
                .copied()
                .fold(0usize, usize::saturating_add),
        );
        for matrix in &self.parts {
            values.extend(matrix.matvec(input)?);
        }
        ProjectionOutputs::new(self.rows_per_projection.as_slice(), values)
    }
}

#[derive(Clone, Debug, Default)]
struct Qwen35SharedPrefixStore {
    entries: Vec<Qwen35SharedPrefixEntry>,
}

#[derive(Clone, Debug)]
struct Qwen35SharedPrefixEntry {
    compatibility: crate::SharedPrefixCompatibility,
    prompt_tokens: TokenSequence,
    output_mode: CudaStepOutputMode,
    state: Qwen35State,
    last_logits: Vec<f32>,
    pending_selected_token: Option<TokenId>,
    last_candidates: Option<Qwen35CudaTopKCandidates>,
}

#[derive(Clone, Debug)]
struct Qwen35PrefixLookupResult {
    state: PrefixCacheState,
    reused_tokens: usize,
    identity: Option<PrefixCacheIdentity>,
    entry: Option<Qwen35SharedPrefixEntry>,
    refusal_reason: Option<PrefixCacheRefusalReason>,
    invalidation_trigger: Option<CacheInvalidationTrigger>,
}

impl Qwen35SharedPrefixStore {
    fn empty_lookup(state: PrefixCacheState) -> Qwen35PrefixLookupResult {
        Qwen35PrefixLookupResult {
            state,
            reused_tokens: 0,
            identity: None,
            entry: None,
            refusal_reason: None,
            invalidation_trigger: None,
        }
    }

    fn boundary_refusal_reason(
        &self,
        compatibility: &crate::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> Option<PrefixCacheRefusalReason> {
        let mut saw_sampler_boundary = false;
        for entry in &self.entries {
            if !entry.compatibility.storage_identity_matches(compatibility)
                || crate::shared_prefix_len(
                    entry.prompt_tokens.as_slice(),
                    prompt_tokens.as_slice(),
                ) == 0
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
        compatibility: &crate::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
    ) -> bool {
        let retained = self.entries.len();
        self.entries.retain(|entry| {
            !(entry.compatibility.storage_identity_matches(compatibility)
                && crate::shared_prefix_len(
                    entry.prompt_tokens.as_slice(),
                    prompt_tokens.as_slice(),
                ) > 0)
        });
        self.entries.len() != retained
    }

    fn lookup(
        &self,
        compatibility: &crate::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        output_mode: CudaStepOutputMode,
    ) -> Qwen35PrefixLookupResult {
        let mut best: Option<&Qwen35SharedPrefixEntry> = None;
        for entry in &self.entries {
            if &entry.compatibility != compatibility || entry.output_mode != output_mode {
                continue;
            }
            if !prompt_tokens
                .as_slice()
                .starts_with(entry.prompt_tokens.as_slice())
            {
                continue;
            }
            match best {
                Some(current) if current.prompt_tokens.len() >= entry.prompt_tokens.len() => {}
                _ => best = Some(entry),
            }
        }
        if let Some(entry) = best {
            return Qwen35PrefixLookupResult {
                state: PrefixCacheState::Hit,
                reused_tokens: entry.prompt_tokens.len(),
                identity: Some(crate::prefix_identity(
                    compatibility,
                    entry.prompt_tokens.as_slice(),
                )),
                entry: Some(entry.clone()),
                refusal_reason: None,
                invalidation_trigger: None,
            };
        }
        if !self.entries.is_empty()
            && let Some(refusal_reason) = self.boundary_refusal_reason(compatibility, prompt_tokens)
        {
            let mut result = Self::empty_lookup(PrefixCacheState::Bypassed);
            result.refusal_reason = Some(refusal_reason);
            return result;
        }
        Self::empty_lookup(if self.entries.is_empty() {
            PrefixCacheState::None
        } else {
            PrefixCacheState::Miss
        })
    }

    fn controlled_lookup(
        &mut self,
        compatibility: &crate::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        output_mode: CudaStepOutputMode,
        request: &GenerationRequest,
    ) -> Qwen35PrefixLookupResult {
        match request.prefix_cache_control.mode {
            PrefixCacheMode::Auto => self.lookup(compatibility, prompt_tokens, output_mode),
            PrefixCacheMode::Bypass => {
                let mut result = Self::empty_lookup(PrefixCacheState::Bypassed);
                result.refusal_reason = Some(PrefixCacheRefusalReason::RequestOptOut);
                result
            }
            PrefixCacheMode::Invalidate => {
                let _ = self.invalidate(compatibility, prompt_tokens);
                let mut result = Self::empty_lookup(PrefixCacheState::Rebuilt);
                result.refusal_reason = Some(PrefixCacheRefusalReason::ForcedInvalidation);
                result.invalidation_trigger = Some(CacheInvalidationTrigger::ExplicitReset);
                result
            }
        }
    }

    fn record(
        &mut self,
        backend: &mut CudaBackend,
        compatibility: crate::SharedPrefixCompatibility,
        prompt_tokens: &TokenSequence,
        output_mode: CudaStepOutputMode,
        state: &Qwen35State,
        last_logits: &[f32],
        pending_selected_token: Option<TokenId>,
        last_candidates: Option<&Qwen35CudaTopKCandidates>,
    ) -> PrefixCacheIdentity {
        let identity = crate::prefix_identity(&compatibility, prompt_tokens.as_slice());
        let stored_state = state
            .deep_clone(backend)
            .expect("qwen35 prefix cache state clone should succeed");
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.compatibility == compatibility
                && entry.output_mode == output_mode
                && entry.prompt_tokens.as_slice() == prompt_tokens.as_slice()
        }) {
            existing.state = stored_state;
            existing.last_logits = last_logits.to_vec();
            existing.pending_selected_token = pending_selected_token;
            existing.last_candidates = last_candidates.cloned();
        } else {
            self.entries.push(Qwen35SharedPrefixEntry {
                compatibility,
                prompt_tokens: prompt_tokens.clone(),
                output_mode,
                state: stored_state,
                last_logits: last_logits.to_vec(),
                pending_selected_token,
                last_candidates: last_candidates.cloned(),
            });
        }
        identity
    }
}

fn qwen35_prefix_compatibility_for_request(
    descriptor: &DecoderModelDescriptor,
    request: &GenerationRequest,
) -> crate::SharedPrefixCompatibility {
    let served_artifact =
        crate::served_artifact_identity_for_decoder_backend(descriptor, "cuda", &[]);
    let policy = crate::default_prefix_cache_policy();
    crate::SharedPrefixCompatibility {
        served_artifact_digest: served_artifact.served_artifact_digest,
        model_id: descriptor.model.model_id.clone(),
        model_revision: descriptor.model.revision.clone(),
        weight_bundle_digest: descriptor.weights.digest.clone(),
        tokenizer_family: descriptor.tokenizer_family.clone(),
        tokenizer_digest: descriptor
            .artifact_identity
            .as_ref()
            .and_then(|value| value.tokenizer_digest.clone()),
        chat_template_digest: descriptor
            .artifact_identity
            .as_ref()
            .and_then(|value| value.chat_template_digest.clone()),
        generation_defaults_digest: descriptor
            .artifact_identity
            .as_ref()
            .map(|value| value.generation_defaults_digest.clone()),
        backend_compatibility: String::from("cuda"),
        tenant_id: crate::prefix_cache_tenant_id(request, &policy),
        sampler_digest: crate::prefix_cache_sampler_digest(request, &policy),
    }
}

fn deep_clone_cuda_buffer(
    backend: &mut CudaBackend,
    source: &CudaBuffer,
) -> Result<CudaBuffer, ReferenceTextGenerationError> {
    let clone = backend
        .byte_buffer(&vec![0_u8; source.byte_len()])
        .map_err(ReferenceTextGenerationError::Runtime)?;
    if source.byte_len() > 0 {
        let mut submission = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        submission
            .copy_buffer_region(source, 0, &clone, 0, source.byte_len())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        submission
            .commit(psionic_backend_cuda::CudaCommandWait::Completed)
            .map_err(ReferenceTextGenerationError::Runtime)?;
    }
    Ok(clone)
}

const QWEN35_CUDA_MAX_TOP_K: usize = 128;
const QWEN35_CUDA_PARTITIONED_TOP_K_THRESHOLD: usize = 40;
const QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_SMALL: usize = 48;
const QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_LARGE: usize = 56;
const QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_LARGE_THRESHOLD: usize = 96;

#[derive(Clone, Debug)]
struct Qwen35CudaTopKCandidates {
    top_k: usize,
    indices: [u32; QWEN35_CUDA_MAX_TOP_K],
    values: [f32; QWEN35_CUDA_MAX_TOP_K],
}

impl Qwen35CudaTopKCandidates {
    fn zeroed(top_k: usize) -> Result<Self, crate::RuntimeError> {
        if top_k == 0 || top_k > QWEN35_CUDA_MAX_TOP_K {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda top-k width must be in 1..={}, actual {}",
                QWEN35_CUDA_MAX_TOP_K, top_k
            )));
        }
        Ok(Self {
            top_k,
            indices: [0_u32; QWEN35_CUDA_MAX_TOP_K],
            values: [0.0_f32; QWEN35_CUDA_MAX_TOP_K],
        })
    }

    fn indices(&self) -> &[u32] {
        &self.indices[..self.top_k]
    }

    fn values(&self) -> &[f32] {
        &self.values[..self.top_k]
    }
}

fn qwen35_fast_greedy_path_enabled() -> bool {
    std::env::var_os("PSIONIC_QWEN35_DISABLE_FAST_GREEDY").is_none()
}

fn qwen35_fused_qkv_rms_norm_enabled() -> bool {
    std::env::var_os("PSIONIC_QWEN35_DISABLE_FUSED_QKV_RMS_NORM").is_none()
}

const QWEN35_CUDA_FA3_MAX_SPLITS: usize = crate::PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_SPLITS;
const QWEN35_CUBLASLT_MODEL_FAMILY: &str = "qwen35.native_cuda_decode";
const QWEN35_CUBLASLT_ROW_LADDER: [usize; 3] = [1, 8, 32];

#[derive(Clone)]
struct Qwen35CublasLtRepresentative {
    weight: CudaBuffer,
    inner: usize,
    cols: usize,
}

#[allow(clippy::too_many_arguments)]
fn encode_qwen35_full_attention_qkv_post_matvec(
    submission: &mut CudaSubmission,
    matvec_output_buffer: &CudaBuffer,
    head_count: usize,
    head_dim: usize,
    query_gate_rows: usize,
    key_rows: usize,
    kv_head_count: usize,
    query_norm_device: &CudaBuffer,
    key_norm_device: &CudaBuffer,
    epsilon: f32,
    q_buffer: &CudaBuffer,
    k_buffer: &CudaBuffer,
    qkv_norm_buffer: &CudaBuffer,
    gate_buffer: &CudaBuffer,
    query_width: usize,
    kv_width: usize,
) -> Result<(), ReferenceTextGenerationError> {
    if qwen35_fused_qkv_rms_norm_enabled() {
        submission.split_interleaved_query_gate_rms_norm_f32(
            matvec_output_buffer,
            head_count,
            head_dim,
            query_norm_device,
            epsilon,
            qkv_norm_buffer,
            gate_buffer,
        )?;
        submission.pack_qwen35_key_value_rms_norm_f32(
            matvec_output_buffer,
            query_gate_rows,
            query_gate_rows.saturating_add(key_rows),
            kv_head_count,
            head_dim,
            key_norm_device,
            epsilon,
            qkv_norm_buffer,
            query_width,
            query_width.saturating_add(kv_width),
        )?;
        return Ok(());
    }

    let query_bytes = query_width.saturating_mul(std::mem::size_of::<f32>());
    let key_bytes = kv_width.saturating_mul(std::mem::size_of::<f32>());
    submission.split_interleaved_query_gate_f32(
        matvec_output_buffer,
        head_count,
        head_dim,
        q_buffer,
        gate_buffer,
    )?;
    submission.rms_norm(q_buffer, query_norm_device, q_buffer, query_width, epsilon)?;
    submission.rms_norm_region(
        matvec_output_buffer,
        query_gate_rows,
        key_norm_device,
        k_buffer,
        0,
        key_rows,
        epsilon,
    )?;
    submission.copy_buffer_region(q_buffer, 0, qkv_norm_buffer, 0, query_bytes)?;
    submission.copy_buffer_region(k_buffer, 0, qkv_norm_buffer, query_bytes, key_bytes)?;
    submission.copy_buffer_region(
        matvec_output_buffer,
        query_gate_rows
            .saturating_add(key_rows)
            .saturating_mul(std::mem::size_of::<f32>()),
        qkv_norm_buffer,
        query_bytes.saturating_add(key_bytes),
        key_bytes,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_qwen35_hybrid_qkv_post_conv(
    submission: &mut CudaSubmission,
    conv_buffer: &CudaBuffer,
    group_count: usize,
    state_size: usize,
    v_offset: usize,
    v_size: usize,
    q_scale_device: &CudaBuffer,
    k_scale_device: &CudaBuffer,
    q_buffer: &CudaBuffer,
    k_buffer: &CudaBuffer,
    qkv_norm_buffer: &CudaBuffer,
) -> Result<(), ReferenceTextGenerationError> {
    let q_size = group_count.saturating_mul(state_size);
    let k_size = q_size;
    if qwen35_fused_qkv_rms_norm_enabled() {
        submission.pack_qwen35_hybrid_qkv_rms_norm_f32(
            conv_buffer,
            0,
            q_size,
            v_offset,
            group_count,
            state_size,
            v_size,
            q_scale_device,
            k_scale_device,
            1e-6,
            qkv_norm_buffer,
            0,
            q_size,
            v_offset,
        )?;
        return Ok(());
    }

    let q_bytes = q_size.saturating_mul(std::mem::size_of::<f32>());
    let k_bytes = k_size.saturating_mul(std::mem::size_of::<f32>());
    submission.rms_norm_region(conv_buffer, 0, q_scale_device, q_buffer, 0, q_size, 1e-6)?;
    submission.rms_norm_region(
        conv_buffer,
        q_size,
        k_scale_device,
        k_buffer,
        0,
        k_size,
        1e-6,
    )?;
    submission.copy_buffer_region(q_buffer, 0, qkv_norm_buffer, 0, q_bytes)?;
    submission.copy_buffer_region(k_buffer, 0, qkv_norm_buffer, q_bytes, k_bytes)?;
    submission.copy_buffer_region(
        conv_buffer,
        v_offset.saturating_mul(std::mem::size_of::<f32>()),
        qkv_norm_buffer,
        v_offset.saturating_mul(std::mem::size_of::<f32>()),
        v_size.saturating_mul(std::mem::size_of::<f32>()),
    )?;
    Ok(())
}

fn qwen35_cuda_output_mode(options: &GenerationOptions) -> CudaStepOutputMode {
    let policy = options.sampling_policy();
    let mirostat = policy.effective_mirostat();
    if options.structured_output.is_some()
        && !qwen35_sampling_penalties_active(options)
        && mirostat.is_none()
        && (matches!(options.decode_strategy, crate::DecodeStrategy::Greedy)
            || policy.effective_temperature() <= 1e-6
            || policy.effective_top_k() == Some(1))
    {
        return CudaStepOutputMode::TopKCandidates(QWEN35_CUDA_MAX_TOP_K);
    }
    if options.structured_output.is_none()
        && qwen35_fast_greedy_path_enabled()
        && !qwen35_sampling_penalties_active(options)
        && mirostat.is_none()
        && (matches!(options.decode_strategy, crate::DecodeStrategy::Greedy)
            || policy.effective_temperature() <= 1e-6
            || policy.effective_top_k() == Some(1))
    {
        return CudaStepOutputMode::ArgmaxOnly;
    }
    if options.structured_output.is_none()
        && matches!(options.decode_strategy, crate::DecodeStrategy::Sample)
        && policy.effective_temperature() > 1e-6
        && mirostat.is_none()
        && policy.effective_temperature() > 1e-6
        && let Some(top_k) = policy.effective_top_k()
        && top_k > 1
        && top_k <= QWEN35_CUDA_MAX_TOP_K
    {
        return CudaStepOutputMode::TopKCandidates(top_k);
    }
    CudaStepOutputMode::FullLogits
}

fn qwen35_metal_output_mode(options: &GenerationOptions) -> MetalStepOutputMode {
    let policy = options.sampling_policy();
    let mirostat = policy.effective_mirostat();
    if options.structured_output.is_none()
        && qwen35_fast_greedy_path_enabled()
        && !qwen35_sampling_penalties_active(options)
        && mirostat.is_none()
        && (matches!(options.decode_strategy, crate::DecodeStrategy::Greedy)
            || policy.effective_temperature() <= 1e-6
            || policy.effective_top_k() == Some(1))
    {
        return MetalStepOutputMode::ArgmaxOnly;
    }
    MetalStepOutputMode::FullLogits
}

fn qwen35_sampling_penalties_active(options: &GenerationOptions) -> bool {
    let policy = options.sampling_policy();
    (policy.effective_repeat_penalty() - 1.0).abs() > f32::EPSILON
        || policy.effective_presence_penalty().abs() > f32::EPSILON
        || policy.effective_frequency_penalty().abs() > f32::EPSILON
}

fn qwen35_sampling_penalty_counts(
    history: &[TokenId],
    vocab_size: usize,
    policy: &SamplingPolicy,
) -> BTreeMap<u32, usize> {
    let start = match policy.effective_repeat_last_n(history.len()) {
        Some(lookback) => history.len().saturating_sub(lookback),
        None => history.len(),
    };
    let mut counts = BTreeMap::new();
    for &token in &history[start..] {
        let token_id = token.as_u32();
        if token_id as usize >= vocab_size {
            continue;
        }
        *counts.entry(token_id).or_insert(0) += 1;
    }
    counts
}

fn qwen35_attention_device_surface(backend: &CudaBackend) -> PsionRvllmCudaDeviceSurface {
    let (architecture, compute_capability) = backend
        .selected_device()
        .and_then(|descriptor| descriptor.nvidia_metadata.as_ref())
        .map_or((None, None), |metadata| {
            (
                metadata.topology.architecture.clone(),
                metadata.topology.compute_capability.clone(),
            )
        });
    PsionRvllmCudaDeviceSurface::new(architecture, compute_capability)
}

fn register_qwen35_cublaslt_scope(
    backend: &CudaBackend,
    representatives: &mut BTreeMap<&'static str, Qwen35CublasLtRepresentative>,
    op_kind: &'static str,
    weight: &CudaBuffer,
    inner: usize,
    cols: usize,
) {
    let scope = CudaGemmTuningScope::new(QWEN35_CUBLASLT_MODEL_FAMILY, op_kind);
    backend.register_cublaslt_weight_scope(weight, scope);
    representatives
        .entry(op_kind)
        .or_insert_with(|| Qwen35CublasLtRepresentative {
            weight: weight.clone(),
            inner,
            cols,
        });
}

fn qwen35_decode_output_metrics(
    output_mode: Qwen35CudaDecodeOutputMode,
    readback_bytes: u64,
    raw_logits_materialized: bool,
    attention_backend: Option<Qwen35CudaAttentionBackendMetrics>,
) -> Qwen35CudaDecodeOutputMetrics {
    Qwen35CudaDecodeOutputMetrics {
        step_count: 1,
        output_modes: vec![output_mode],
        readback_bytes,
        raw_logits_materialized,
        graph_cache_identity: None,
        graph_replay: None,
        attention_backend,
    }
}

fn qwen35_cuda_graph_replay_metrics(
    output_mode: CudaGraphReplayMode,
    reused_graph_exec: bool,
    capture_latency_ns: u64,
    shape_drift: bool,
) -> CudaGraphReplayMetrics {
    CudaGraphReplayMetrics {
        step_count: 1,
        replay_hit_count: usize::from(reused_graph_exec),
        replay_miss_count: usize::from(!reused_graph_exec),
        capture_count: usize::from(!reused_graph_exec),
        shape_drift_count: usize::from(shape_drift),
        refusal_count: 0,
        capture_latency_ns,
        output_modes: vec![output_mode],
    }
}

fn attach_qwen35_graph_replay_metrics(
    output_metrics: &mut Option<Qwen35CudaDecodeOutputMetrics>,
    graph_replay: CudaGraphReplayMetrics,
) {
    if let Some(output_metrics) = output_metrics.as_mut() {
        output_metrics.graph_replay = Some(graph_replay);
    }
}

fn accumulate_qwen35_decode_output_metrics(
    total: &mut Qwen35CudaDecodeOutputMetrics,
    step_metrics: Option<&Qwen35CudaDecodeOutputMetrics>,
) {
    if let Some(step_metrics) = step_metrics {
        total.accumulate(step_metrics);
    }
}

fn can_use_q8_1_quantized_matvec(mode: QuantizationMode) -> bool {
    matches!(
        mode,
        QuantizationMode::GgmlQ8_0
            | QuantizationMode::GgmlQ3K
            | QuantizationMode::GgmlQ5K
            | QuantizationMode::GgmlQ4K
            | QuantizationMode::GgmlQ6K
            | QuantizationMode::GgmlIq3S
            | QuantizationMode::GgmlIq4Xs
            | QuantizationMode::GgmlMxfp4
    )
}

fn can_use_q8_1_argmax(mode: QuantizationMode) -> bool {
    matches!(
        mode,
        QuantizationMode::GgmlQ8_0
            | QuantizationMode::GgmlQ3K
            | QuantizationMode::GgmlQ5K
            | QuantizationMode::GgmlQ4K
            | QuantizationMode::GgmlQ6K
            | QuantizationMode::GgmlIq3S
            | QuantizationMode::GgmlIq4Xs
            | QuantizationMode::GgmlMxfp4
    )
}

fn can_use_cuda_quantized_matvec(mode: QuantizationMode) -> bool {
    matches!(
        mode,
        QuantizationMode::GgmlQ8_0
            | QuantizationMode::GgmlQ3K
            | QuantizationMode::GgmlQ5K
            | QuantizationMode::GgmlMxfp4
            | QuantizationMode::GgmlQ4K
            | QuantizationMode::GgmlQ6K
            | QuantizationMode::GgmlIq3S
            | QuantizationMode::GgmlIq4Xs
    )
}

fn qwen35_requires_dense_f16_mirror(mode: QuantizationMode) -> bool {
    !can_use_cuda_quantized_matvec(mode)
}

fn qwen35_unsupported_features(
    metadata: &GgufDecoderFamilyMetadata,
    include_multimodal_refusals: bool,
) -> Vec<String> {
    let mut features = Vec::new();
    if include_multimodal_refusals {
        features.push(String::from("multimodal_inputs"));
        features.push(String::from("video_inputs"));
    }
    features.push(String::from("adapter_serving"));
    features.push(String::from("session_reuse"));
    if family_fact_usize(metadata, "qwen35.nextn_predict_layers").unwrap_or(0) > 0 {
        features.push(String::from("mtp_speculative_decoding_skipped"));
    }
    if !include_multimodal_refusals && matches!(metadata.family, GgufDecoderFamily::Qwen38) {
        features.push(String::from("multimodal_inputs"));
        features.push(String::from("video_inputs"));
    }
    features
}

fn supports_native_metal_qwen35_projection(mode: QuantizationMode) -> bool {
    matches!(
        mode,
        QuantizationMode::GgmlQ4K
            | QuantizationMode::GgmlQ5K
            | QuantizationMode::GgmlQ6K
            | QuantizationMode::GgmlQ8_0
            | QuantizationMode::GgmlMxfp4
    )
}

fn qwen35_metal_projection_names(
    layout: &psionic_models::GgufDecoderTensorLayout,
) -> Result<Vec<String>, ModelLoadError> {
    let mut names = vec![
        layout
            .output
            .as_ref()
            .unwrap_or(&layout.token_embedding)
            .clone(),
    ];
    for layer in &layout.layers {
        names.extend([
            required_tensor_name(layer.feed_forward_gate_weight.as_deref(), "ffn_gate")?
                .to_string(),
            required_tensor_name(layer.feed_forward_up_weight.as_deref(), "ffn_up")?.to_string(),
            required_tensor_name(layer.feed_forward_down_weight.as_deref(), "ffn_down")?
                .to_string(),
        ]);
        match layer.layer_kind {
            GgufDecoderLayerKind::Qwen35Hybrid => names.extend([
                required_tensor_name(layer.attention_qkv_weight.as_deref(), "attn_qkv")?
                    .to_string(),
                required_tensor_name(layer.attention_gate_weight.as_deref(), "attn_gate")?
                    .to_string(),
                required_tensor_name(layer.ssm_alpha_weight.as_deref(), "ssm_alpha")?.to_string(),
                required_tensor_name(layer.ssm_beta_weight.as_deref(), "ssm_beta")?.to_string(),
                required_tensor_name(layer.ssm_out_weight.as_deref(), "ssm_out")?.to_string(),
            ]),
            GgufDecoderLayerKind::Qwen35FullAttention => names.extend([
                required_tensor_name(layer.attention_query_weight.as_deref(), "attn_q")?
                    .to_string(),
                required_tensor_name(layer.attention_key_weight.as_deref(), "attn_k")?.to_string(),
                required_tensor_name(layer.attention_value_weight.as_deref(), "attn_v")?
                    .to_string(),
                required_tensor_name(layer.attention_output_weight.as_deref(), "attn_output")?
                    .to_string(),
            ]),
            other => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "qwen metal projection admission does not support layer kind `{other:?}`"
                    ),
                });
            }
        }
    }
    Ok(names)
}

fn qwen38_metal_projection_rows(
    artifact: &GgufBlobArtifact,
    layout: &psionic_models::GgufDecoderTensorLayout,
) -> Result<Vec<Qwen38MetalProjectionRow>, ModelLoadError> {
    qwen35_metal_projection_names(layout)?
        .into_iter()
        .map(|name| {
            let storage = artifact.paged_tensor(name.as_str())?;
            Ok(Qwen38MetalProjectionRow {
                name,
                mode: storage.metadata().quantization,
                byte_length: storage.byte_length().try_into().unwrap_or(u64::MAX),
            })
        })
        .collect()
}

fn qwen38_metal_projection_preflight(
    rows: &[Qwen38MetalProjectionRow],
) -> Result<Qwen38MetalPreflight, ModelLoadError> {
    let unsupported = rows
        .iter()
        .filter(|row| !supports_native_metal_qwen35_projection(row.mode))
        .map(|row| format!("{}={:?}", row.name, row.mode))
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "qwen38 metal preflight refused required projections without a native kernel or admitted conversion: {}",
                unsupported.join(", ")
            ),
        });
    }
    Ok(Qwen38MetalPreflight {
        projection_count: rows.len(),
        projected_device_weight_bytes: rows
            .iter()
            .map(|row| row.byte_length)
            .fold(0u64, u64::saturating_add),
    })
}

fn qwen35_partitioned_top_k_block_override() -> Option<usize> {
    std::env::var("PSIONIC_QWEN35_PARTITIONED_TOP_K_BLOCKS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn qwen35_partitioned_top_k_threshold() -> usize {
    std::env::var("PSIONIC_QWEN35_PARTITIONED_TOP_K_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(QWEN35_CUDA_PARTITIONED_TOP_K_THRESHOLD)
}

fn qwen35_partitioned_top_k_block_count(top_k: usize, override_blocks: Option<usize>) -> usize {
    if let Some(blocks) = override_blocks {
        return blocks;
    }
    if top_k >= QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_LARGE_THRESHOLD {
        QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_LARGE
    } else {
        QWEN35_CUDA_PARTITIONED_TOP_K_BLOCKS_SMALL
    }
}

fn initial_cuda_argmax_pair_bytes() -> [u8; std::mem::size_of::<u64>()] {
    let ordered = !f32::NEG_INFINITY.to_bits();
    let packed = (u64::from(ordered) << 32) | u64::from(u32::MAX - i32::MAX as u32);
    packed.to_ne_bytes()
}

fn cuda_argmax_token_id(token: i32) -> Result<TokenId, ReferenceTextGenerationError> {
    u32::try_from(token).map(TokenId).map_err(|_| {
        ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(format!(
            "cuda argmax returned a negative token id {token}",
        )))
    })
}

fn cuda_argmax_token_from_packed_host_buffer(
    host_buffer: &CudaHostBuffer,
) -> Result<TokenId, ReferenceTextGenerationError> {
    let bytes = host_buffer.read_bytes().map_err(|error| {
        ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(format!(
            "cuda argmax returned an invalid packed host buffer: {error}",
        )))
    })?;
    let packed = u64::from_ne_bytes(bytes[..std::mem::size_of::<u64>()].try_into().map_err(
        |_| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                "cuda argmax returned invalid packed argmax bytes",
            )))
        },
    )?);
    cuda_argmax_token_id((u32::MAX - packed as u32) as i32)
}

fn cuda_top_k_candidates_from_indices(
    indices: &[u32],
) -> Result<Qwen35CudaTopKCandidates, crate::RuntimeError> {
    let mut candidates = Qwen35CudaTopKCandidates::zeroed(indices.len())?;
    candidates.indices[..indices.len()].copy_from_slice(indices);
    Ok(candidates)
}

fn cuda_top_k_candidates_from_index_host_buffer(
    indices_host_buffer: &CudaHostBuffer,
    top_k: usize,
) -> Result<Qwen35CudaTopKCandidates, crate::RuntimeError> {
    let expected_indices_bytes = top_k.saturating_mul(std::mem::size_of::<i32>());
    if indices_host_buffer.byte_len() < expected_indices_bytes {
        return Err(crate::RuntimeError::Backend(format!(
            "qwen35 cuda top-k index host buffer is too small: need {} bytes, have {}",
            expected_indices_bytes,
            indices_host_buffer.byte_len()
        )));
    }
    let mut index_bytes = [0_u8; QWEN35_CUDA_MAX_TOP_K * std::mem::size_of::<i32>()];
    indices_host_buffer
        .read_bytes_prefix_into(&mut index_bytes[..expected_indices_bytes])
        .map_err(|error| {
            crate::RuntimeError::Backend(format!(
                "failed to read qwen35 cuda top-k index host buffer: {error}",
            ))
        })?;
    let mut candidates = Qwen35CudaTopKCandidates::zeroed(top_k)?;
    for (slot, chunk) in candidates.indices[..top_k]
        .iter_mut()
        .zip(index_bytes[..expected_indices_bytes].chunks_exact(std::mem::size_of::<i32>()))
    {
        let index = i32::from_ne_bytes(chunk.try_into().map_err(|_| {
            crate::RuntimeError::Backend(String::from(
                "qwen35 cuda top-k returned invalid index bytes",
            ))
        })?);
        *slot = u32::try_from(index).map_err(|_| {
            crate::RuntimeError::Backend(format!(
                "qwen35 cuda top-k returned a negative token index {index}",
            ))
        })?;
    }
    Ok(candidates)
}

fn cuda_top_k_candidates_from_host_buffers(
    indices_host_buffer: &CudaHostBuffer,
    values_host_buffer: &CudaHostBuffer,
    top_k: usize,
) -> Result<Qwen35CudaTopKCandidates, crate::RuntimeError> {
    let expected_indices_bytes = top_k.saturating_mul(std::mem::size_of::<i32>());
    let expected_values_bytes = top_k.saturating_mul(std::mem::size_of::<f32>());
    if indices_host_buffer.byte_len() < expected_indices_bytes
        || values_host_buffer.byte_len() < expected_values_bytes
    {
        return Err(crate::RuntimeError::Backend(format!(
            "qwen35 cuda top-k host buffers are too small: need {} index bytes and {} value bytes, have {} and {}",
            expected_indices_bytes,
            expected_values_bytes,
            indices_host_buffer.byte_len(),
            values_host_buffer.byte_len()
        )));
    }

    let mut index_bytes = [0_u8; QWEN35_CUDA_MAX_TOP_K * std::mem::size_of::<i32>()];
    indices_host_buffer
        .read_bytes_prefix_into(&mut index_bytes[..expected_indices_bytes])
        .map_err(|error| {
            crate::RuntimeError::Backend(format!(
                "failed to read qwen35 cuda top-k index host buffer: {error}",
            ))
        })?;
    let mut value_bytes = [0_u8; QWEN35_CUDA_MAX_TOP_K * std::mem::size_of::<f32>()];
    values_host_buffer
        .read_bytes_prefix_into(&mut value_bytes[..expected_values_bytes])
        .map_err(|error| {
            crate::RuntimeError::Backend(format!(
                "failed to read qwen35 cuda top-k value host buffer: {error}",
            ))
        })?;

    let mut candidates = Qwen35CudaTopKCandidates::zeroed(top_k)?;
    for (slot, chunk) in candidates.indices[..top_k]
        .iter_mut()
        .zip(index_bytes[..expected_indices_bytes].chunks_exact(std::mem::size_of::<i32>()))
    {
        let index = i32::from_ne_bytes(chunk.try_into().map_err(|_| {
            crate::RuntimeError::Backend(String::from(
                "qwen35 cuda top-k returned invalid index bytes",
            ))
        })?);
        *slot = u32::try_from(index).map_err(|_| {
            crate::RuntimeError::Backend(format!(
                "qwen35 cuda top-k returned a negative token index {index}",
            ))
        })?;
    }

    for (slot, chunk) in candidates.values[..top_k]
        .iter_mut()
        .zip(value_bytes[..expected_values_bytes].chunks_exact(std::mem::size_of::<f32>()))
    {
        *slot = f32::from_ne_bytes(chunk.try_into().map_err(|_| {
            crate::RuntimeError::Backend(String::from(
                "qwen35 cuda top-k returned invalid value bytes",
            ))
        })?);
    }

    Ok(candidates)
}

fn cuda_f32_vec_from_host_buffer(
    host_buffer: &CudaHostBuffer,
    element_count: usize,
) -> Result<Vec<f32>, crate::RuntimeError> {
    let expected_bytes = element_count.saturating_mul(std::mem::size_of::<f32>());
    if host_buffer.byte_len() < expected_bytes {
        return Err(crate::RuntimeError::Backend(format!(
            "qwen35 cuda logits host buffer is too small: need {} bytes, have {}",
            expected_bytes,
            host_buffer.byte_len()
        )));
    }
    let bytes = host_buffer.read_bytes().map_err(|error| {
        crate::RuntimeError::Backend(format!(
            "failed to read qwen35 cuda logits host buffer: {error}",
        ))
    })?;
    let mut values = Vec::with_capacity(element_count);
    for chunk in bytes[..expected_bytes].chunks_exact(std::mem::size_of::<f32>()) {
        values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(values)
}

fn select_argmax(logits: &[f32]) -> Result<TokenId, ReferenceTextGenerationError> {
    let Some((index, _)) = logits
        .iter()
        .copied()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(&right.1))
    else {
        return Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(String::from(
                "argmax selection requires non-empty logits",
            )),
        ));
    };
    Ok(TokenId(index as u32))
}

fn average_inter_token_latency_ns(
    first: Option<std::time::Duration>,
    last: Option<std::time::Duration>,
    token_count: usize,
) -> Option<u64> {
    if token_count < 2 {
        return None;
    }
    let first = first?;
    let last = last?;
    last.checked_sub(first)
        .and_then(|delta| delta.as_nanos().checked_div((token_count - 1) as u128))
        .and_then(|average| average.try_into().ok())
}

fn qwen35_cache_capacity_tokens(
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

#[derive(Clone, Debug)]
struct CudaQwen35Model {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GgufRuntimeTokenizer,
    token_embedding: HostMatrix,
    token_embedding_f16: Option<CudaBuffer>,
    token_embedding_quantized: Option<CudaQuantizedMatrix>,
    output_norm: Vec<f32>,
    output_norm_device: CudaBuffer,
    output: CudaQuantizedMatrix,
    layers: Vec<Qwen35Layer>,
    plan_digest: String,
    graph_cache_identity: String,
    qwen38_cuda_preflight: Option<Qwen38CudaPreflight>,
    load_duration_ns: u64,
    weight_device_bytes: u64,
    memory_plan: psionic_runtime::ModelMemoryPlan,
}

impl CudaQwen35Model {
    fn from_gguf_path(
        path: impl AsRef<Path>,
        backend: &mut CudaBackend,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let artifact = GgufBlobArtifact::open_path(&path, gguf_local_blob_open_options())?;
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if !matches!(
            adapter.family_metadata().family,
            GgufDecoderFamily::Qwen35 | GgufDecoderFamily::Qwen38
        ) {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }
        let qwen38_cuda_preflight = if matches!(
            adapter.family_metadata().family,
            GgufDecoderFamily::Qwen38
        ) {
            let unsupported_quantization = qwen38_cuda_unsupported_quantization_modes(
                adapter.descriptor().weights.quantization_modes.as_slice(),
            );
            if !unsupported_quantization.is_empty() {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "qwen38 cuda preflight rejected unsupported quantization modes before weight upload: {unsupported_quantization:?}"
                    ),
                }
                .into());
            }
            let artifact_bytes = std::fs::metadata(path.as_ref())
                .map_err(|source| ModelLoadError::ArtifactRead {
                    path: path.as_ref().display().to_string(),
                    message: format!(
                        "failed qwen38 cuda preflight metadata inspection before weight upload: {source}"
                    ),
                })?
                .len();
            let memory = backend
                .device_memory_info()
                .map_err(ReferenceTextGenerationError::Runtime)?
                .ok_or_else(|| ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: String::from(
                        "qwen38 cuda preflight requires live device-memory information before weight upload",
                    ),
                })?;
            Some(qwen38_cuda_preflight(artifact_bytes, memory)?)
        } else {
            None
        };
        let tokenizer = GgufRuntimeTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build qwen35 tokenizer: {error}"),
            }
        })?;
        let token_embedding_name = adapter.tensor_layout().token_embedding.as_str();
        let token_embedding = HostMatrix::load(&artifact, token_embedding_name)?;
        let (token_embedding_f16, token_embedding_quantized) = match &token_embedding.kind {
            HostMatrixKind::Dense(_) => (
                try_build_cuda_host_matrix_row_major_f16_mirror(
                    backend,
                    token_embedding_name,
                    &token_embedding,
                )?,
                None,
            ),
            HostMatrixKind::Quantized(matrix) if can_use_cuda_quantized_matvec(matrix.mode) => (
                None,
                Some(load_cuda_quantized_matrix(
                    backend,
                    &artifact,
                    token_embedding_name,
                )?),
            ),
            HostMatrixKind::Quantized(matrix) => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "qwen35 cuda token embedding `{token_embedding_name}` uses unsupported quantization {:?}",
                        matrix.mode
                    ),
                }
                .into());
            }
        };
        let output = if let Some(name) = adapter.tensor_layout().output.as_ref() {
            load_cuda_quantized_matrix(backend, &artifact, name.as_str())?
        } else {
            load_cuda_quantized_matrix(
                backend,
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layout| Qwen35Layer::load(backend, &artifact, layout, adapter.family_metadata()))
            .collect::<Result<Vec<_>, _>>()?;
        let output_norm =
            load_dense_vector(&artifact, adapter.tensor_layout().output_norm.as_str())?;
        let output_norm_device =
            upload_f32_buffer(backend, output_norm.as_slice(), "qwen35_output_norm")?;
        let weights_bytes = std::fs::metadata(path.as_ref())
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let device_bytes = output
            .device_residency_bytes()
            .saturating_add(
                token_embedding_f16
                    .as_ref()
                    .map(CudaBuffer::byte_len)
                    .unwrap_or(0),
            )
            .saturating_add(
                token_embedding_quantized
                    .as_ref()
                    .map(CudaQuantizedMatrix::device_residency_bytes)
                    .unwrap_or(0),
            )
            .saturating_add(vec_f32_bytes(output_norm.as_slice()))
            .saturating_add(
                layers
                    .iter()
                    .map(Qwen35Layer::device_residency_bytes)
                    .sum::<usize>(),
            )
            .try_into()
            .unwrap_or(u64::MAX);
        let host_bytes = token_embedding
            .host_residency_bytes()
            .saturating_add(vec_f32_bytes(output_norm.as_slice()))
            .saturating_add(
                layers
                    .iter()
                    .map(Qwen35Layer::host_residency_bytes)
                    .sum::<usize>(),
            )
            .try_into()
            .unwrap_or(u64::MAX);
        let plan_digest = digest_qwen35_cuda_plan(adapter.descriptor(), adapter.family_metadata());
        let graph_cache_identity = digest_qwen35_cuda_graph_cache(
            adapter.descriptor(),
            adapter.family_metadata(),
            plan_digest.as_str(),
        );
        Ok(Self {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            token_embedding,
            token_embedding_f16,
            token_embedding_quantized,
            output_norm,
            output_norm_device,
            output,
            layers,
            plan_digest,
            graph_cache_identity,
            qwen38_cuda_preflight,
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
            weight_device_bytes: device_bytes,
            memory_plan: psionic_runtime::ModelMemoryPlan::split_residency(
                weights_bytes,
                0,
                0,
                host_bytes,
                device_bytes,
            ),
        })
    }

    fn initial_state(
        &self,
        backend: &mut CudaBackend,
        cache_capacity_tokens: usize,
    ) -> Result<Qwen35State, ReferenceTextGenerationError> {
        Ok(Qwen35State {
            position: 0,
            layers: self
                .layers
                .iter()
                .map(|layer| layer.initial_state(backend, cache_capacity_tokens))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn cuda_context_limit_tokens(&self) -> usize {
        if matches!(self.family_metadata.family, GgufDecoderFamily::Qwen38) {
            self.descriptor
                .config
                .max_context
                .min(QWEN38_CUDA_CONTEXT_LIMIT_TOKENS)
        } else {
            self.descriptor.config.max_context
        }
    }

    fn planned_state_device_bytes(&self, cache_capacity_tokens: usize) -> (u64, u64) {
        let mut recurrent_bytes = 0usize;
        let mut kv_cache_bytes = 0usize;
        for layer in &self.layers {
            match &layer.kind {
                Qwen35LayerKind::Hybrid(hybrid) => {
                    recurrent_bytes = recurrent_bytes
                        .saturating_add(
                            hybrid.qkv_gate_alpha_beta.rows_per_projection[0]
                                .saturating_mul(hybrid.conv_kernel.saturating_sub(1))
                                .saturating_mul(std::mem::size_of::<f32>()),
                        )
                        .saturating_add(
                            hybrid
                                .time_step_rank
                                .saturating_mul(hybrid.state_size)
                                .saturating_mul(hybrid.state_size)
                                .saturating_mul(std::mem::size_of::<f32>()),
                        );
                }
                Qwen35LayerKind::FullAttention(full_attention) => {
                    kv_cache_bytes = kv_cache_bytes.saturating_add(
                        cache_capacity_tokens
                            .saturating_mul(full_attention.kv_width)
                            .saturating_mul(std::mem::size_of::<u16>())
                            .saturating_mul(2),
                    );
                }
            }
        }
        (
            recurrent_bytes.try_into().unwrap_or(u64::MAX),
            kv_cache_bytes.try_into().unwrap_or(u64::MAX),
        )
    }

    fn dense_f16_mirror_count(&self) -> usize {
        let mut count = usize::from(self.token_embedding_f16.is_some())
            .saturating_add(usize::from(self.output.transposed_f16.is_some()));
        for layer in &self.layers {
            count = count
                .saturating_add(usize::from(layer.ffn_gate_up.transposed_f16.is_some()))
                .saturating_add(usize::from(layer.ffn_down.transposed_f16.is_some()))
                .saturating_add(match &layer.kind {
                    Qwen35LayerKind::Hybrid(hybrid) => {
                        usize::from(hybrid.qkv_gate_alpha_beta.transposed_f16.is_some())
                            .saturating_add(usize::from(hybrid.ssm_out.transposed_f16.is_some()))
                    }
                    Qwen35LayerKind::FullAttention(full_attention) => usize::from(
                        full_attention.qkv.transposed_f16.is_some(),
                    )
                    .saturating_add(usize::from(full_attention.output.transposed_f16.is_some())),
                });
        }
        count
    }

    fn build_step_plan(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<Qwen35CudaStepPlan, ReferenceTextGenerationError> {
        Qwen35CudaStepPlan::new(
            backend,
            self.descriptor.config.hidden_size,
            self.descriptor.config.block.attention.head_count,
            self.max_projection_input_columns(),
            self.max_projection_output_rows(),
            self.descriptor.config.vocab_size,
            self.descriptor.config.max_context,
        )
    }

    fn autotune_cublaslt_plans(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<(), ReferenceTextGenerationError> {
        let mut representatives = BTreeMap::new();
        if let Some(weight) = self.output.transposed_f16.as_ref() {
            register_qwen35_cublaslt_scope(
                backend,
                &mut representatives,
                "output_logits",
                weight,
                self.output.host.columns,
                self.output.host.rows,
            );
        }
        for layer in &self.layers {
            if let Some(weight) = layer.ffn_gate_up.transposed_f16.as_ref() {
                register_qwen35_cublaslt_scope(
                    backend,
                    &mut representatives,
                    "ffn_gate_up",
                    weight,
                    layer.ffn_gate_up.columns,
                    layer.ffn_gate_up.total_rows(),
                );
            }
            if let Some(weight) = layer.ffn_down.transposed_f16.as_ref() {
                register_qwen35_cublaslt_scope(
                    backend,
                    &mut representatives,
                    "ffn_down",
                    weight,
                    layer.ffn_down.host.columns,
                    layer.ffn_down.host.rows,
                );
            }
            match &layer.kind {
                Qwen35LayerKind::Hybrid(hybrid) => {
                    if let Some(weight) = hybrid.qkv_gate_alpha_beta.transposed_f16.as_ref() {
                        register_qwen35_cublaslt_scope(
                            backend,
                            &mut representatives,
                            "hybrid_qkv_gate_alpha_beta",
                            weight,
                            hybrid.qkv_gate_alpha_beta.columns,
                            hybrid.qkv_gate_alpha_beta.total_rows(),
                        );
                    }
                    if let Some(weight) = hybrid.ssm_out.transposed_f16.as_ref() {
                        register_qwen35_cublaslt_scope(
                            backend,
                            &mut representatives,
                            "hybrid_ssm_out",
                            weight,
                            hybrid.ssm_out.host.columns,
                            hybrid.ssm_out.host.rows,
                        );
                    }
                }
                Qwen35LayerKind::FullAttention(full_attention) => {
                    if let Some(weight) = full_attention.qkv.transposed_f16.as_ref() {
                        register_qwen35_cublaslt_scope(
                            backend,
                            &mut representatives,
                            "attention_qkv",
                            weight,
                            full_attention.qkv.columns,
                            full_attention.qkv.total_rows(),
                        );
                    }
                    if let Some(weight) = full_attention.output.transposed_f16.as_ref() {
                        register_qwen35_cublaslt_scope(
                            backend,
                            &mut representatives,
                            "attention_output",
                            weight,
                            full_attention.output.host.columns,
                            full_attention.output.host.rows,
                        );
                    }
                }
            }
        }
        for (op_kind, representative) in representatives {
            backend
                .autotune_cublaslt_f16_to_f32_weight_rows(
                    &representative.weight,
                    CudaGemmTuningScope::new(QWEN35_CUBLASLT_MODEL_FAMILY, op_kind),
                    representative.inner,
                    representative.cols,
                    &QWEN35_CUBLASLT_ROW_LADDER,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
        }
        Ok(())
    }

    fn max_projection_input_columns(&self) -> usize {
        self.layers
            .iter()
            .map(Qwen35Layer::max_matvec_input_columns)
            .fold(self.output.host.columns, usize::max)
    }

    fn max_projection_output_rows(&self) -> usize {
        self.layers
            .iter()
            .map(Qwen35Layer::max_matvec_output_rows)
            .max()
            .unwrap_or(self.descriptor.config.hidden_size)
    }

    fn full_attention_backend_selection(
        &self,
        backend: &CudaBackend,
        full_attention: &Qwen35FullAttentionLayer,
        state: &Qwen35FullAttentionState,
        use_graph_attention: bool,
    ) -> crate::PsionRvllmFa3DecodeAttentionSelection {
        let head_dim = self.descriptor.config.block.attention.head_dim;
        let kv_head_count = full_attention.kv_width / head_dim.max(1);
        select_psion_rvllm_fa3_decode_attention_backend(
            &qwen35_attention_device_surface(backend),
            PsionRvllmFa3DecodeAttentionShape {
                use_graph_attention,
                head_count: self.descriptor.config.block.attention.head_count,
                kv_head_count,
                head_dim,
                sliding_window: self.family_metadata.sliding_window.unwrap_or(0),
                past_tokens: state.len,
            },
        )
    }

    fn attention_backend_metrics(
        &self,
        backend: &CudaBackend,
        state: &Qwen35State,
        use_graph_attention: bool,
    ) -> Option<Qwen35CudaAttentionBackendMetrics> {
        let mut metrics = Qwen35CudaAttentionBackendMetrics::default();
        for (layer, layer_state) in self.layers.iter().zip(state.layers.iter()) {
            let (
                Qwen35LayerKind::FullAttention(full_attention),
                Qwen35LayerState::FullAttention(full_attention_state),
            ) = (&layer.kind, layer_state)
            else {
                continue;
            };
            metrics.push(
                self.full_attention_backend_selection(
                    backend,
                    full_attention,
                    full_attention_state,
                    use_graph_attention,
                )
                .execution(),
            );
        }
        (!metrics.is_zero()).then_some(metrics)
    }

    fn encode_token_embedding_lookup(
        &self,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        token: TokenId,
        position: usize,
    ) -> Result<u64, ReferenceTextGenerationError> {
        let decode_params_bytes = self.write_decode_params(plan, token, position)?;
        submission
            .copy_host_to_device(&plan.decode_params_host_buffer, &plan.decode_params_buffer)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if let Some(token_embedding) = self.token_embedding_quantized.as_ref() {
            submission
                .dequantize_row_to_f32(
                    &token_embedding.storage,
                    token_embedding.host.mode,
                    token_embedding.host.rows,
                    token_embedding.host.row_byte_len,
                    token_embedding.host.columns,
                    &plan.decode_params_buffer,
                    &plan.current_hidden_buffer,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
        } else if let Some(token_embedding_f16) = self.token_embedding_f16.as_ref() {
            submission
                .gather_f16_row_to_f32(
                    token_embedding_f16,
                    self.token_embedding.rows(),
                    self.token_embedding.columns(),
                    &plan.decode_params_buffer,
                    &plan.current_hidden_buffer,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
        } else {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(String::from(
                    "qwen35 cuda token embedding lookup has no admitted device storage",
                )),
            ));
        }
        Ok(decode_params_bytes)
    }

    fn write_decode_params(
        &self,
        plan: &mut Qwen35CudaStepPlan,
        token: TokenId,
        position: usize,
    ) -> Result<u64, ReferenceTextGenerationError> {
        let decode_params = [
            i32::try_from(position).map_err(|_| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(format!(
                    "qwen35 past-token count {position} exceeds i32 decode parameter limits",
                )))
            })?,
            i32::try_from(position).map_err(|_| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(format!(
                    "qwen35 decode position {position} exceeds i32 decode parameter limits",
                )))
            })?,
            i32::try_from(token.as_u32()).map_err(|_| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(format!(
                    "qwen35 token {} exceeds i32 decode parameter limits",
                    token.as_u32(),
                )))
            })?,
        ];
        plan.decode_params_host_buffer
            .write_i32(&decode_params)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok((decode_params.len() * std::mem::size_of::<i32>())
            .try_into()
            .unwrap_or(u64::MAX))
    }

    fn captured_initial_token(&self, layer_index: usize, token: TokenId) -> Option<TokenId> {
        (layer_index == 0).then_some(token).filter(|_| {
            self.token_embedding_f16.is_some() || self.token_embedding_quantized.is_some()
        })
    }

    fn encode_captured_decode_layers(
        &self,
        backend: &mut CudaBackend,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        state: &mut Qwen35State,
        token: TokenId,
        position: usize,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        for (layer_index, (layer, layer_state)) in
            self.layers.iter().zip(state.layers.iter_mut()).enumerate()
        {
            let initial_token = self.captured_initial_token(layer_index, token);
            match (&layer.kind, layer_state) {
                (Qwen35LayerKind::Hybrid(_), Qwen35LayerState::Hybrid(hybrid_state)) => {
                    layer.encode_hybrid_device_submission(
                        backend,
                        submission,
                        plan,
                        self,
                        hybrid_state,
                        position,
                        initial_token,
                        bytes_moved,
                    )?;
                }
                (
                    Qwen35LayerKind::FullAttention(full_attention),
                    Qwen35LayerState::FullAttention(full_attention_state),
                ) => {
                    layer.encode_full_attention_device_submission(
                        backend,
                        submission,
                        plan,
                        self,
                        full_attention,
                        full_attention_state,
                        position,
                        initial_token,
                        true,
                        bytes_moved,
                    )?;
                }
                _ => {
                    return Err(ReferenceTextGenerationError::Runtime(
                        crate::RuntimeError::Backend(String::from(
                            "qwen35 layer state kind mismatch",
                        )),
                    ));
                }
            }
        }
        Ok(())
    }

    fn forward_token(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        state: &mut Qwen35State,
        token: TokenId,
        output_mode: CudaStepOutputMode,
        request_options: &GenerationOptions,
        generated_history: &[TokenId],
    ) -> Result<Qwen35ForwardStep, ReferenceTextGenerationError> {
        let position = [state.position; 3];
        self.forward_token_with_embedding_and_mrope(
            backend,
            plan,
            state,
            token,
            None,
            position,
            output_mode,
            request_options,
            generated_history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn forward_generated_token(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        state: &mut Qwen35State,
        token: TokenId,
        output_mode: CudaStepOutputMode,
        request_options: &GenerationOptions,
        generated_history: &[TokenId],
        multimodal_plan: Option<&Qwen38MultimodalDecoderPlan>,
    ) -> Result<Qwen35ForwardStep, ReferenceTextGenerationError> {
        let position = multimodal_plan
            .map(|plan| plan.generated_position(state.position))
            .transpose()
            .map_err(|error| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::UnsupportedStep(
                    error.to_string(),
                ))
            })?
            .unwrap_or([state.position; 3]);
        self.forward_token_with_embedding_and_mrope(
            backend,
            plan,
            state,
            token,
            None,
            position,
            output_mode,
            request_options,
            generated_history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn forward_token_with_embedding_and_mrope(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        state: &mut Qwen35State,
        token: TokenId,
        embedding_override: Option<&[f32]>,
        mrope_position: [usize; 3],
        output_mode: CudaStepOutputMode,
        request_options: &GenerationOptions,
        generated_history: &[TokenId],
    ) -> Result<Qwen35ForwardStep, ReferenceTextGenerationError> {
        if token.as_u32() as usize >= self.descriptor.config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: self.descriptor.config.vocab_size,
            });
        }
        if let Some(embedding) = embedding_override
            && embedding.len() != self.descriptor.config.hidden_size
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::UnsupportedStep(format!(
                    "qwen38 CUDA vision embedding width mismatch: expected {}, actual {}",
                    self.descriptor.config.hidden_size,
                    embedding.len(),
                )),
            ));
        }
        let physical_position = state.position;
        if embedding_override.is_none()
            && mrope_position == [physical_position; 3]
            && std::env::var_os("PSIONIC_QWEN35_DEBUG_ATTENTION").is_none()
        {
            return self.forward_token_fused(
                backend,
                plan,
                state,
                token,
                output_mode,
                request_options,
                generated_history,
            );
        }
        let sampling_policy = request_options.sampling_policy();
        let attention_backend_metrics = self.attention_backend_metrics(backend, state, false);
        let mut bytes_moved = 0u64;
        let mut kernel_count = 0usize;
        let position = state.position;
        if let Some(embedding) = embedding_override {
            plan.current_hidden_buffer
                .write_f32_at_offset(0, embedding)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            bytes_moved =
                bytes_moved.saturating_add(vec_f32_bytes(embedding).try_into().unwrap_or(u64::MAX));
        } else if self.token_embedding_f16.is_none() && self.token_embedding_quantized.is_none() {
            let hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
            plan.current_hidden_buffer
                .write_f32_at_offset(0, hidden.as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            bytes_moved = bytes_moved.saturating_add(
                vec_f32_bytes(hidden.as_slice())
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            kernel_count = kernel_count.saturating_add(1);
        }
        for (layer_index, (layer, layer_state)) in
            self.layers.iter().zip(state.layers.iter_mut()).enumerate()
        {
            let initial_token = embedding_override
                .is_none()
                .then(|| self.captured_initial_token(layer_index, token))
                .flatten();
            match (&layer.kind, &mut *layer_state) {
                (Qwen35LayerKind::Hybrid(_), Qwen35LayerState::Hybrid(hybrid_state)) => {
                    layer.forward_hybrid_device(
                        backend,
                        plan,
                        self,
                        layer_index,
                        hybrid_state,
                        position,
                        initial_token,
                        &mut kernel_count,
                        &mut bytes_moved,
                    )?;
                }
                (
                    Qwen35LayerKind::FullAttention(full_attention),
                    Qwen35LayerState::FullAttention(full_attention_state),
                ) => {
                    layer.forward_full_attention_device(
                        backend,
                        plan,
                        self,
                        layer_index,
                        full_attention,
                        full_attention_state,
                        initial_token,
                        position,
                        mrope_position,
                        &mut kernel_count,
                        &mut bytes_moved,
                    )?;
                }
                _ => {
                    return Err(ReferenceTextGenerationError::Runtime(
                        crate::RuntimeError::Backend(String::from(
                            "qwen35 layer state kind mismatch",
                        )),
                    ));
                }
            }
            if std::env::var_os("PSIONIC_QWEN38_DEBUG_NONFINITE").is_some() {
                if let (Qwen35LayerKind::Hybrid(hybrid), Qwen35LayerState::Hybrid(hybrid_state)) =
                    (&layer.kind, &*layer_state)
                {
                    emit_qwen35_hybrid_intermediate_debug(
                        position,
                        layer_index,
                        hybrid,
                        hybrid_state,
                        plan,
                        self.descriptor.config.hidden_size,
                    )?;
                }
                emit_qwen35_hidden_debug(
                    position,
                    layer_index,
                    &layer.kind,
                    &plan.current_hidden_buffer,
                    self.descriptor.config.hidden_size,
                )?;
                let hidden = plan
                    .current_hidden_buffer
                    .read_f32_at_offset(0, self.descriptor.config.hidden_size)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                if let Some((hidden_index, value)) = hidden
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, value)| !value.is_finite())
                {
                    return Err(ReferenceTextGenerationError::Runtime(
                        crate::RuntimeError::Backend(format!(
                            "qwen38 CUDA hidden state became non-finite after layer {layer_index} at physical position {position}, MRoPE position {mrope_position:?}, hidden index {hidden_index}, value {value}"
                        )),
                    ));
                }
            }
        }

        let current_hidden_buffer = plan.current_hidden_buffer.clone();
        let output_norm_device = self.output_norm_device.clone();
        let (logits, selected_token, candidates, output_stats, output_metrics) = match output_mode {
            CudaStepOutputMode::NoOutput => {
                (Vec::new(), None, None, zero_cuda_matvec_stats(), None)
            }
            CudaStepOutputMode::FullLogits => {
                let (logits, stats) = plan
                    .run_output_logits_from_device(
                        backend,
                        &current_hidden_buffer,
                        &output_norm_device,
                        self.family_metadata.rms_norm_epsilon,
                        self.output.transposed_f16.as_ref(),
                        &self.output.storage,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                let readback_bytes = self
                    .output
                    .host
                    .rows
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX);
                (
                    logits,
                    None,
                    None,
                    stats,
                    Some(qwen35_decode_output_metrics(
                        Qwen35CudaDecodeOutputMode::RawLogits,
                        readback_bytes,
                        true,
                        attention_backend_metrics.clone(),
                    )),
                )
            }
            CudaStepOutputMode::ArgmaxOnly => {
                let (selected, stats) = plan
                    .run_output_argmax_from_device(
                        backend,
                        &current_hidden_buffer,
                        &output_norm_device,
                        self.family_metadata.rms_norm_epsilon,
                        self.output.transposed_f16.as_ref(),
                        &self.output.storage,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        request_options.structured_output.is_some(),
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                let readback_bytes = stats.device_to_host_bytes;
                (
                    Vec::new(),
                    Some(selected),
                    None,
                    stats,
                    Some(qwen35_decode_output_metrics(
                        Qwen35CudaDecodeOutputMode::ArgmaxOnly,
                        readback_bytes,
                        false,
                        attention_backend_metrics.clone(),
                    )),
                )
            }
            CudaStepOutputMode::TopKCandidates(top_k) => {
                let (candidates, stats) = if request_options.structured_output.is_some() {
                    let (indices, stats) = plan
                        .run_output_top_k_indices_from_device(
                            backend,
                            &current_hidden_buffer,
                            &output_norm_device,
                            self.family_metadata.rms_norm_epsilon,
                            self.output.transposed_f16.as_ref(),
                            &self.output.storage,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            top_k,
                            generated_history,
                            &sampling_policy,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    (
                        cuda_top_k_candidates_from_indices(indices.as_slice())
                            .map_err(ReferenceTextGenerationError::Runtime)?,
                        stats,
                    )
                } else {
                    plan.run_output_top_k_from_device(
                        backend,
                        &current_hidden_buffer,
                        &output_norm_device,
                        self.family_metadata.rms_norm_epsilon,
                        self.output.transposed_f16.as_ref(),
                        &self.output.storage,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        top_k,
                        generated_history,
                        &sampling_policy,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?
                };
                let readback_bytes = stats.device_to_host_bytes;
                (
                    Vec::new(),
                    None,
                    Some(candidates),
                    stats,
                    Some(qwen35_decode_output_metrics(
                        Qwen35CudaDecodeOutputMode::TopKCandidates { top_k },
                        readback_bytes,
                        false,
                        attention_backend_metrics.clone(),
                    )),
                )
            }
        };
        bytes_moved = bytes_moved.saturating_add(cuda_stats_bytes(output_stats));
        kernel_count = kernel_count.saturating_add(output_stats.kernel_launches);
        state.position = state.position.saturating_add(1);
        Ok(Qwen35ForwardStep {
            logits,
            selected_token,
            candidates,
            kernel_count,
            bytes_moved,
            output_metrics,
        })
    }

    fn forward_token_fused(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        state: &mut Qwen35State,
        token: TokenId,
        output_mode: CudaStepOutputMode,
        request_options: &GenerationOptions,
        generated_history: &[TokenId],
    ) -> Result<Qwen35ForwardStep, ReferenceTextGenerationError> {
        let mut bytes_moved = 0u64;
        let mut kernel_count = 0usize;
        let position = state.position;
        let sampling_policy = request_options.sampling_policy();
        let graph_attention_backend_metrics = self.attention_backend_metrics(backend, state, true);
        let legacy_attention_backend_metrics =
            self.attention_backend_metrics(backend, state, false);
        if self.token_embedding_f16.is_none() && self.token_embedding_quantized.is_none() {
            let hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
            plan.current_hidden_buffer
                .write_f32_at_offset(0, hidden.as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            bytes_moved = bytes_moved.saturating_add(
                vec_f32_bytes(hidden.as_slice())
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            kernel_count = kernel_count.saturating_add(1);
        }

        if output_mode == CudaStepOutputMode::NoOutput {
            let decode_params_bytes = self.write_decode_params(plan, token, position)?;
            bytes_moved = bytes_moved.saturating_add(decode_params_bytes);
            let no_output_graph_cache_identity = qwen35_decode_graph_cache_identity(state);
            let mut reused_graph_exec = false;
            let report = if plan.no_output_graph_cache_identity.as_ref()
                == Some(&no_output_graph_cache_identity)
            {
                if let Some(graph_exec) = plan.no_output_graph_exec.as_ref() {
                    reused_graph_exec = true;
                    graph_exec
                        .launch(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?
                } else {
                    let mut submission = backend
                        .begin_captured_submission()
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_host_to_device(
                            &plan.decode_params_host_buffer,
                            &plan.decode_params_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    self.encode_captured_decode_layers(
                        backend,
                        &mut submission,
                        plan,
                        state,
                        token,
                        position,
                        &mut bytes_moved,
                    )?;
                    let (report, graph_exec) = submission
                        .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    plan.no_output_graph_exec = Some(graph_exec);
                    plan.no_output_graph_cache_identity = Some(no_output_graph_cache_identity);
                    report
                }
            } else {
                plan.no_output_graph_exec = None;
                plan.no_output_graph_cache_identity = None;
                let mut submission = backend
                    .begin_captured_submission()
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                submission
                    .copy_host_to_device(
                        &plan.decode_params_host_buffer,
                        &plan.decode_params_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                self.encode_captured_decode_layers(
                    backend,
                    &mut submission,
                    plan,
                    state,
                    token,
                    position,
                    &mut bytes_moved,
                )?;
                let (report, graph_exec) = submission
                    .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                plan.no_output_graph_exec = Some(graph_exec);
                plan.no_output_graph_cache_identity = Some(no_output_graph_cache_identity);
                report
            };
            kernel_count = kernel_count.saturating_add(report.encoded_operations);
            if reused_graph_exec {
                for layer_state in &mut state.layers {
                    if let Qwen35LayerState::FullAttention(full_attention) = layer_state {
                        full_attention.len = full_attention.len.saturating_add(1);
                    }
                }
            }
            state.position = state.position.saturating_add(1);
            return Ok(Qwen35ForwardStep {
                logits: Vec::new(),
                selected_token: None,
                candidates: None,
                kernel_count,
                bytes_moved,
                output_metrics: None,
            });
        }

        let output_uses_q8_1_argmax =
            self.output.transposed_f16.is_none() && can_use_q8_1_argmax(self.output.host.mode);
        let output_uses_q8_1_matvec = self.output.transposed_f16.is_none()
            && can_use_q8_1_quantized_matvec(self.output.host.mode);
        let output_uses_f16_argmax = self.output.transposed_f16.is_some();

        if output_mode == CudaStepOutputMode::FullLogits {
            let decode_params_bytes = self.write_decode_params(plan, token, position)?;
            let logits_bytes = self
                .output
                .host
                .rows
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX);
            bytes_moved = bytes_moved
                .saturating_add(decode_params_bytes)
                .saturating_add(logits_bytes);
            let full_logits_graph_cache_identity = qwen35_decode_graph_cache_identity(state);
            let mut reused_graph_exec = false;
            let mut shape_drift = false;
            let mut capture_latency_ns = 0_u64;
            let report = if plan.full_logits_graph_cache_identity.as_ref()
                == Some(&full_logits_graph_cache_identity)
            {
                if let Some(graph_exec) = plan.full_logits_graph_exec.as_ref() {
                    reused_graph_exec = true;
                    graph_exec
                        .launch(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?
                } else {
                    let capture_started = Instant::now();
                    let mut submission = backend
                        .begin_captured_submission()
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_host_to_device(
                            &plan.decode_params_host_buffer,
                            &plan.decode_params_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    self.encode_captured_decode_layers(
                        backend,
                        &mut submission,
                        plan,
                        state,
                        token,
                        position,
                        &mut bytes_moved,
                    )?;
                    if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                        submission
                            .rms_norm(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .cast_f32_to_f16(
                                &plan.matvec_input_buffer,
                                &plan.vector_f16_buffer,
                                self.output.host.columns,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .matmul_f16_to_f32(
                                &plan.vector_f16_buffer,
                                transposed_f16,
                                &plan.logits_buffer,
                                1,
                                self.output.host.columns,
                                self.output.host.rows,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                    } else if output_uses_q8_1_matvec {
                        submission
                            .rms_norm_q8_1(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_q8_1_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.quantized_matvec_q8_1(
                            &self.output.storage,
                            0,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            &plan.matvec_input_q8_1_buffer,
                            None,
                            &plan.logits_buffer,
                        )?;
                    } else {
                        submission
                            .rms_norm(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.quantized_matvec(
                            &self.output.storage,
                            0,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            &plan.matvec_input_buffer,
                            &plan.logits_buffer,
                        )?;
                    }
                    submission
                        .copy_device_to_host(&plan.logits_buffer, &plan.logits_host_buffer)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    let (report, graph_exec) = submission
                        .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    capture_latency_ns = capture_started
                        .elapsed()
                        .as_nanos()
                        .try_into()
                        .unwrap_or(u64::MAX);
                    plan.full_logits_graph_exec = Some(graph_exec);
                    plan.full_logits_graph_cache_identity = Some(full_logits_graph_cache_identity);
                    report
                }
            } else {
                shape_drift = plan.full_logits_graph_exec.is_some()
                    || plan.full_logits_graph_cache_identity.is_some();
                plan.full_logits_graph_exec = None;
                plan.full_logits_graph_cache_identity = None;
                let capture_started = Instant::now();
                let mut submission = backend
                    .begin_captured_submission()
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                submission
                    .copy_host_to_device(
                        &plan.decode_params_host_buffer,
                        &plan.decode_params_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                self.encode_captured_decode_layers(
                    backend,
                    &mut submission,
                    plan,
                    state,
                    token,
                    position,
                    &mut bytes_moved,
                )?;
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission
                        .rms_norm(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .cast_f32_to_f16(
                            &plan.matvec_input_buffer,
                            &plan.vector_f16_buffer,
                            self.output.host.columns,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .matmul_f16_to_f32(
                            &plan.vector_f16_buffer,
                            transposed_f16,
                            &plan.logits_buffer,
                            1,
                            self.output.host.columns,
                            self.output.host.rows,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                } else if output_uses_q8_1_matvec {
                    submission
                        .rms_norm_q8_1(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_q8_1_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec_q8_1(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.logits_buffer,
                    )?;
                } else {
                    submission
                        .rms_norm(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        &plan.matvec_input_buffer,
                        &plan.logits_buffer,
                    )?;
                }
                submission
                    .copy_device_to_host(&plan.logits_buffer, &plan.logits_host_buffer)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                let (report, graph_exec) = submission
                    .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                capture_latency_ns = capture_started
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX);
                plan.full_logits_graph_exec = Some(graph_exec);
                plan.full_logits_graph_cache_identity = Some(full_logits_graph_cache_identity);
                report
            };
            kernel_count = kernel_count.saturating_add(report.encoded_operations);
            if reused_graph_exec {
                for layer_state in &mut state.layers {
                    if let Qwen35LayerState::FullAttention(full_attention) = layer_state {
                        full_attention.len = full_attention.len.saturating_add(1);
                    }
                }
            }
            state.position = state.position.saturating_add(1);
            let logits =
                cuda_f32_vec_from_host_buffer(&plan.logits_host_buffer, self.output.host.rows)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
            let mut step = Qwen35ForwardStep {
                logits,
                selected_token: None,
                candidates: None,
                kernel_count,
                bytes_moved,
                output_metrics: Some(qwen35_decode_output_metrics(
                    Qwen35CudaDecodeOutputMode::RawLogits,
                    logits_bytes,
                    true,
                    graph_attention_backend_metrics.clone(),
                )),
            };
            attach_qwen35_graph_replay_metrics(
                &mut step.output_metrics,
                qwen35_cuda_graph_replay_metrics(
                    CudaGraphReplayMode::RawLogits,
                    reused_graph_exec,
                    capture_latency_ns,
                    shape_drift,
                ),
            );
            return Ok(step);
        }

        if let CudaStepOutputMode::TopKCandidates(top_k) = output_mode {
            let decode_params_bytes = self.write_decode_params(plan, token, position)?;
            let top_k_bytes = top_k
                .saturating_mul(
                    std::mem::size_of::<u32>()
                        + if request_options.structured_output.is_some() {
                            0
                        } else {
                            std::mem::size_of::<f32>()
                        },
                )
                .try_into()
                .unwrap_or(u64::MAX);
            bytes_moved = bytes_moved
                .saturating_add(decode_params_bytes)
                .saturating_add(top_k_bytes);
            let top_k_graph_cache_identity = (top_k, qwen35_decode_graph_cache_identity(state));
            let mut reused_graph_exec = false;
            let mut shape_drift = false;
            let mut capture_latency_ns = 0_u64;
            let report = if plan.top_k_graph_cache_identity.as_ref()
                == Some(&top_k_graph_cache_identity)
            {
                if let Some(graph_exec) = plan.top_k_graph_exec.as_ref() {
                    reused_graph_exec = true;
                    graph_exec
                        .launch(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?
                } else {
                    let capture_started = Instant::now();
                    let mut submission = backend
                        .begin_captured_submission()
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_host_to_device(
                            &plan.decode_params_host_buffer,
                            &plan.decode_params_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    self.encode_captured_decode_layers(
                        backend,
                        &mut submission,
                        plan,
                        state,
                        token,
                        position,
                        &mut bytes_moved,
                    )?;
                    if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                        submission
                            .rms_norm(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .cast_f32_to_f16(
                                &plan.matvec_input_buffer,
                                &plan.vector_f16_buffer,
                                self.output.host.columns,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .matmul_f16_to_f32(
                                &plan.vector_f16_buffer,
                                transposed_f16,
                                &plan.logits_buffer,
                                1,
                                self.output.host.columns,
                                self.output.host.rows,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                    } else if output_uses_q8_1_matvec {
                        submission
                            .rms_norm_q8_1(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_q8_1_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.quantized_matvec_q8_1(
                            &self.output.storage,
                            0,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            &plan.matvec_input_q8_1_buffer,
                            None,
                            &plan.logits_buffer,
                        )?;
                    } else {
                        submission
                            .rms_norm(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.quantized_matvec(
                            &self.output.storage,
                            0,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            &plan.matvec_input_buffer,
                            &plan.logits_buffer,
                        )?;
                    }
                    plan.encode_top_k_from_logits(&mut submission, self.output.host.rows, top_k)?;
                    submission
                        .copy_device_to_host(
                            &plan.top_k_indices_buffer,
                            &plan.top_k_indices_host_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_device_to_host(
                            &plan.top_k_values_buffer,
                            &plan.top_k_values_host_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    let (report, graph_exec) = submission
                        .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    capture_latency_ns = capture_started
                        .elapsed()
                        .as_nanos()
                        .try_into()
                        .unwrap_or(u64::MAX);
                    plan.top_k_graph_exec = Some(graph_exec);
                    plan.top_k_graph_cache_identity = Some(top_k_graph_cache_identity);
                    report
                }
            } else {
                shape_drift =
                    plan.top_k_graph_exec.is_some() || plan.top_k_graph_cache_identity.is_some();
                plan.top_k_graph_exec = None;
                plan.top_k_graph_cache_identity = None;
                let capture_started = Instant::now();
                let mut submission = backend
                    .begin_captured_submission()
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                submission
                    .copy_host_to_device(
                        &plan.decode_params_host_buffer,
                        &plan.decode_params_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                self.encode_captured_decode_layers(
                    backend,
                    &mut submission,
                    plan,
                    state,
                    token,
                    position,
                    &mut bytes_moved,
                )?;
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission
                        .rms_norm(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .cast_f32_to_f16(
                            &plan.matvec_input_buffer,
                            &plan.vector_f16_buffer,
                            self.output.host.columns,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .matmul_f16_to_f32(
                            &plan.vector_f16_buffer,
                            transposed_f16,
                            &plan.logits_buffer,
                            1,
                            self.output.host.columns,
                            self.output.host.rows,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                } else if output_uses_q8_1_matvec {
                    submission
                        .rms_norm_q8_1(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_q8_1_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec_q8_1(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.logits_buffer,
                    )?;
                } else {
                    submission
                        .rms_norm(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        &plan.matvec_input_buffer,
                        &plan.logits_buffer,
                    )?;
                }
                plan.encode_top_k_from_logits(&mut submission, self.output.host.rows, top_k)?;
                submission
                    .copy_device_to_host(
                        &plan.top_k_indices_buffer,
                        &plan.top_k_indices_host_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                if request_options.structured_output.is_none() {
                    submission
                        .copy_device_to_host(
                            &plan.top_k_values_buffer,
                            &plan.top_k_values_host_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                }
                let (report, graph_exec) = submission
                    .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                capture_latency_ns = capture_started
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX);
                plan.top_k_graph_exec = Some(graph_exec);
                plan.top_k_graph_cache_identity = Some(top_k_graph_cache_identity);
                report
            };
            kernel_count = kernel_count.saturating_add(report.encoded_operations);
            if reused_graph_exec {
                for layer_state in &mut state.layers {
                    if let Qwen35LayerState::FullAttention(full_attention) = layer_state {
                        full_attention.len = full_attention.len.saturating_add(1);
                    }
                }
            }
            state.position = state.position.saturating_add(1);
            let candidates = if request_options.structured_output.is_some() {
                cuda_top_k_candidates_from_index_host_buffer(&plan.top_k_indices_host_buffer, top_k)
                    .map_err(ReferenceTextGenerationError::Runtime)?
            } else {
                cuda_top_k_candidates_from_host_buffers(
                    &plan.top_k_indices_host_buffer,
                    &plan.top_k_values_host_buffer,
                    top_k,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?
            };
            let mut step = Qwen35ForwardStep {
                logits: Vec::new(),
                selected_token: None,
                candidates: Some(candidates),
                kernel_count,
                bytes_moved,
                output_metrics: Some(qwen35_decode_output_metrics(
                    Qwen35CudaDecodeOutputMode::TopKCandidates { top_k },
                    top_k_bytes,
                    false,
                    graph_attention_backend_metrics.clone(),
                )),
            };
            attach_qwen35_graph_replay_metrics(
                &mut step.output_metrics,
                qwen35_cuda_graph_replay_metrics(
                    CudaGraphReplayMode::TopKCandidates { top_k },
                    reused_graph_exec,
                    capture_latency_ns,
                    shape_drift,
                ),
            );
            return Ok(step);
        }

        if output_mode == CudaStepOutputMode::ArgmaxOnly
            && (output_uses_q8_1_argmax || output_uses_f16_argmax)
        {
            let decode_params_bytes = self.write_decode_params(plan, token, position)?;
            plan.argmax_state_host_buffer
                .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            if output_uses_q8_1_argmax {
                let argmax_bytes = std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX);
                bytes_moved = bytes_moved
                    .saturating_add(decode_params_bytes)
                    .saturating_add(argmax_bytes)
                    .saturating_add(argmax_bytes);
            } else {
                bytes_moved = bytes_moved
                    .saturating_add(decode_params_bytes)
                    .saturating_add(std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX));
            }
            let decode_graph_cache_identity = qwen35_decode_graph_cache_identity(state);
            let mut reused_graph_exec = false;
            let mut shape_drift = false;
            let mut capture_latency_ns = 0_u64;
            let report = if plan.decode_graph_cache_identity.as_ref()
                == Some(&decode_graph_cache_identity)
            {
                if let Some(graph_exec) = plan.decode_graph_exec.as_ref() {
                    reused_graph_exec = true;
                    graph_exec
                        .launch(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?
                } else {
                    let capture_started = Instant::now();
                    let mut submission = backend
                        .begin_captured_submission()
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_host_to_device(
                            &plan.decode_params_host_buffer,
                            &plan.decode_params_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    self.encode_captured_decode_layers(
                        backend,
                        &mut submission,
                        plan,
                        state,
                        token,
                        position,
                        &mut bytes_moved,
                    )?;
                    if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                        submission
                            .rms_norm(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .cast_f32_to_f16(
                                &plan.matvec_input_buffer,
                                &plan.vector_f16_buffer,
                                self.output.host.columns,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .matmul_f16_to_f32(
                                &plan.vector_f16_buffer,
                                transposed_f16,
                                &plan.logits_buffer,
                                1,
                                self.output.host.columns,
                                self.output.host.rows,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .argmax_f32(
                                &plan.logits_buffer,
                                1,
                                self.output.host.rows,
                                &plan.next_token_buffer,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .copy_device_to_host(
                                &plan.next_token_buffer,
                                &plan.next_token_host_buffer,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                    } else {
                        submission
                            .rms_norm_q8_1(
                                &plan.current_hidden_buffer,
                                &self.output_norm_device,
                                &plan.matvec_input_q8_1_buffer,
                                self.output.host.columns,
                                self.family_metadata.rms_norm_epsilon,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission
                            .copy_host_to_device(
                                &plan.argmax_state_host_buffer,
                                &plan.argmax_state_buffer,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        submission.quantized_matvec_q8_1_argmax(
                            &self.output.storage,
                            0,
                            self.output.host.mode,
                            self.output.host.rows,
                            self.output.host.columns,
                            &plan.matvec_input_q8_1_buffer,
                            None,
                            &plan.argmax_state_buffer,
                        )?;
                        submission
                            .copy_device_to_host(
                                &plan.argmax_state_buffer,
                                &plan.argmax_state_host_buffer,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                    }
                    let (report, graph_exec) = submission
                        .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    capture_latency_ns = capture_started
                        .elapsed()
                        .as_nanos()
                        .try_into()
                        .unwrap_or(u64::MAX);
                    plan.decode_graph_exec = Some(graph_exec);
                    plan.decode_graph_cache_identity = Some(decode_graph_cache_identity);
                    report
                }
            } else {
                shape_drift =
                    plan.decode_graph_exec.is_some() || plan.decode_graph_cache_identity.is_some();
                plan.decode_graph_exec = None;
                plan.decode_graph_cache_identity = None;
                let capture_started = Instant::now();
                let mut submission = backend
                    .begin_captured_submission()
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                submission
                    .copy_host_to_device(
                        &plan.decode_params_host_buffer,
                        &plan.decode_params_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                self.encode_captured_decode_layers(
                    backend,
                    &mut submission,
                    plan,
                    state,
                    token,
                    position,
                    &mut bytes_moved,
                )?;
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission
                        .rms_norm(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .cast_f32_to_f16(
                            &plan.matvec_input_buffer,
                            &plan.vector_f16_buffer,
                            self.output.host.columns,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .matmul_f16_to_f32(
                            &plan.vector_f16_buffer,
                            transposed_f16,
                            &plan.logits_buffer,
                            1,
                            self.output.host.columns,
                            self.output.host.rows,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .argmax_f32(
                            &plan.logits_buffer,
                            1,
                            self.output.host.rows,
                            &plan.next_token_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_device_to_host(&plan.next_token_buffer, &plan.next_token_host_buffer)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                } else {
                    submission
                        .rms_norm_q8_1(
                            &plan.current_hidden_buffer,
                            &self.output_norm_device,
                            &plan.matvec_input_q8_1_buffer,
                            self.output.host.columns,
                            self.family_metadata.rms_norm_epsilon,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission
                        .copy_host_to_device(
                            &plan.argmax_state_host_buffer,
                            &plan.argmax_state_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec_q8_1_argmax(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        self.output.host.rows,
                        self.output.host.columns,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.argmax_state_buffer,
                    )?;
                    submission
                        .copy_device_to_host(
                            &plan.argmax_state_buffer,
                            &plan.argmax_state_host_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                }
                let (report, graph_exec) = submission
                    .commit_captured(psionic_backend_cuda::CudaCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                capture_latency_ns = capture_started
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX);
                plan.decode_graph_exec = Some(graph_exec);
                plan.decode_graph_cache_identity = Some(decode_graph_cache_identity);
                report
            };
            kernel_count = kernel_count.saturating_add(report.encoded_operations);
            if reused_graph_exec {
                for layer_state in &mut state.layers {
                    if let Qwen35LayerState::FullAttention(full_attention) = layer_state {
                        full_attention.len = full_attention.len.saturating_add(1);
                    }
                }
            }
            state.position = state.position.saturating_add(1);
            let selected_token = if output_uses_q8_1_argmax {
                cuda_argmax_token_from_packed_host_buffer(&plan.argmax_state_host_buffer)?
            } else {
                cuda_argmax_token_id(
                    plan.next_token_host_buffer
                        .read_i32()
                        .map_err(ReferenceTextGenerationError::Runtime)?,
                )?
            };
            let mut step = Qwen35ForwardStep {
                logits: Vec::new(),
                selected_token: Some(selected_token),
                candidates: None,
                kernel_count,
                bytes_moved,
                output_metrics: Some(qwen35_decode_output_metrics(
                    Qwen35CudaDecodeOutputMode::ArgmaxOnly,
                    if output_uses_q8_1_argmax {
                        std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX)
                    } else {
                        std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX)
                    },
                    false,
                    graph_attention_backend_metrics.clone(),
                )),
            };
            attach_qwen35_graph_replay_metrics(
                &mut step.output_metrics,
                qwen35_cuda_graph_replay_metrics(
                    CudaGraphReplayMode::ArgmaxOnly,
                    reused_graph_exec,
                    capture_latency_ns,
                    shape_drift,
                ),
            );
            return Ok(step);
        }

        if std::env::var_os("PSIONIC_QWEN35_DEBUG_FUSED_LAYERS").is_some() {
            for (layer_index, (layer, layer_state)) in
                self.layers.iter().zip(state.layers.iter_mut()).enumerate()
            {
                let initial_token = self.captured_initial_token(layer_index, token);
                let mut submission = backend
                    .begin_submission()
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                match (&layer.kind, &mut *layer_state) {
                    (Qwen35LayerKind::Hybrid(_), Qwen35LayerState::Hybrid(hybrid_state)) => {
                        layer.encode_hybrid_device_submission(
                            backend,
                            &mut submission,
                            plan,
                            self,
                            hybrid_state,
                            position,
                            initial_token,
                            &mut bytes_moved,
                        )?;
                    }
                    (
                        Qwen35LayerKind::FullAttention(full_attention),
                        Qwen35LayerState::FullAttention(full_attention_state),
                    ) => {
                        layer.encode_full_attention_device_submission(
                            backend,
                            &mut submission,
                            plan,
                            self,
                            full_attention,
                            full_attention_state,
                            position,
                            initial_token,
                            false,
                            &mut bytes_moved,
                        )?;
                    }
                    _ => {
                        return Err(ReferenceTextGenerationError::Runtime(
                            crate::RuntimeError::Backend(String::from(
                                "qwen35 layer state kind mismatch",
                            )),
                        ));
                    }
                }
                let report = submission
                    .commit(psionic_backend_cuda::CudaCommandWait::Completed)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                kernel_count = kernel_count.saturating_add(report.encoded_operations);
                if let (Qwen35LayerKind::Hybrid(hybrid), Qwen35LayerState::Hybrid(hybrid_state)) =
                    (&layer.kind, &*layer_state)
                {
                    emit_qwen35_hybrid_intermediate_debug(
                        position,
                        layer_index,
                        hybrid,
                        hybrid_state,
                        plan,
                        self.descriptor.config.hidden_size,
                    )?;
                }
                emit_qwen35_hidden_debug(
                    position,
                    layer_index,
                    &layer.kind,
                    &plan.current_hidden_buffer,
                    self.descriptor.config.hidden_size,
                )?;
            }
            let current_hidden_buffer = plan.current_hidden_buffer.clone();
            let output_norm_device = self.output_norm_device.clone();
            let (logits, selected_token, candidates, output_stats, output_metrics) =
                match output_mode {
                    CudaStepOutputMode::NoOutput => {
                        (Vec::new(), None, None, zero_cuda_matvec_stats(), None)
                    }
                    CudaStepOutputMode::FullLogits => {
                        let (logits, stats) = plan
                            .run_output_logits_from_device(
                                backend,
                                &current_hidden_buffer,
                                &output_norm_device,
                                self.family_metadata.rms_norm_epsilon,
                                self.output.transposed_f16.as_ref(),
                                &self.output.storage,
                                self.output.host.mode,
                                self.output.host.rows,
                                self.output.host.columns,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let readback_bytes = self
                            .output
                            .host
                            .rows
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX);
                        (
                            logits,
                            None,
                            None,
                            stats,
                            Some(qwen35_decode_output_metrics(
                                Qwen35CudaDecodeOutputMode::RawLogits,
                                readback_bytes,
                                true,
                                legacy_attention_backend_metrics.clone(),
                            )),
                        )
                    }
                    CudaStepOutputMode::ArgmaxOnly => {
                        let (selected, stats) = plan
                            .run_output_argmax_from_device(
                                backend,
                                &current_hidden_buffer,
                                &output_norm_device,
                                self.family_metadata.rms_norm_epsilon,
                                self.output.transposed_f16.as_ref(),
                                &self.output.storage,
                                self.output.host.mode,
                                self.output.host.rows,
                                self.output.host.columns,
                                request_options.structured_output.is_some(),
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?;
                        let readback_bytes = stats.device_to_host_bytes;
                        (
                            Vec::new(),
                            Some(selected),
                            None,
                            stats,
                            Some(qwen35_decode_output_metrics(
                                Qwen35CudaDecodeOutputMode::ArgmaxOnly,
                                readback_bytes,
                                false,
                                legacy_attention_backend_metrics.clone(),
                            )),
                        )
                    }
                    CudaStepOutputMode::TopKCandidates(top_k) => {
                        let (candidates, stats) = if request_options.structured_output.is_some() {
                            let (indices, stats) = plan
                                .run_output_top_k_indices_from_device(
                                    backend,
                                    &current_hidden_buffer,
                                    &output_norm_device,
                                    self.family_metadata.rms_norm_epsilon,
                                    self.output.transposed_f16.as_ref(),
                                    &self.output.storage,
                                    self.output.host.mode,
                                    self.output.host.rows,
                                    self.output.host.columns,
                                    top_k,
                                    generated_history,
                                    &sampling_policy,
                                )
                                .map_err(ReferenceTextGenerationError::Runtime)?;
                            (
                                cuda_top_k_candidates_from_indices(indices.as_slice())
                                    .map_err(ReferenceTextGenerationError::Runtime)?,
                                stats,
                            )
                        } else {
                            plan.run_output_top_k_from_device(
                                backend,
                                &current_hidden_buffer,
                                &output_norm_device,
                                self.family_metadata.rms_norm_epsilon,
                                self.output.transposed_f16.as_ref(),
                                &self.output.storage,
                                self.output.host.mode,
                                self.output.host.rows,
                                self.output.host.columns,
                                top_k,
                                generated_history,
                                &sampling_policy,
                            )
                            .map_err(ReferenceTextGenerationError::Runtime)?
                        };
                        let readback_bytes = stats.device_to_host_bytes;
                        (
                            Vec::new(),
                            None,
                            Some(candidates),
                            stats,
                            Some(qwen35_decode_output_metrics(
                                Qwen35CudaDecodeOutputMode::TopKCandidates { top_k },
                                readback_bytes,
                                false,
                                legacy_attention_backend_metrics.clone(),
                            )),
                        )
                    }
                };
            bytes_moved = bytes_moved.saturating_add(cuda_stats_bytes(output_stats));
            kernel_count = kernel_count.saturating_add(output_stats.kernel_launches);
            state.position = state.position.saturating_add(1);
            return Ok(Qwen35ForwardStep {
                logits,
                selected_token,
                candidates,
                kernel_count,
                bytes_moved,
                output_metrics,
            });
        }

        let mut submission = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        for (layer_index, (layer, layer_state)) in
            self.layers.iter().zip(state.layers.iter_mut()).enumerate()
        {
            let initial_token = self.captured_initial_token(layer_index, token);
            match (&layer.kind, layer_state) {
                (Qwen35LayerKind::Hybrid(_), Qwen35LayerState::Hybrid(hybrid_state)) => {
                    layer.encode_hybrid_device_submission(
                        backend,
                        &mut submission,
                        plan,
                        self,
                        hybrid_state,
                        position,
                        initial_token,
                        &mut bytes_moved,
                    )?;
                }
                (
                    Qwen35LayerKind::FullAttention(full_attention),
                    Qwen35LayerState::FullAttention(full_attention_state),
                ) => {
                    layer.encode_full_attention_device_submission(
                        backend,
                        &mut submission,
                        plan,
                        self,
                        full_attention,
                        full_attention_state,
                        position,
                        initial_token,
                        false,
                        &mut bytes_moved,
                    )?;
                }
                _ => {
                    return Err(ReferenceTextGenerationError::Runtime(
                        crate::RuntimeError::Backend(String::from(
                            "qwen35 layer state kind mismatch",
                        )),
                    ));
                }
            }
        }

        let output_rows = self.output.host.rows;
        let output_cols = self.output.host.columns;
        let output_mode_q8_1_projection = self.output.transposed_f16.is_none()
            && can_use_q8_1_quantized_matvec(self.output.host.mode);
        let output_mode_q8_1_argmax =
            output_mode_q8_1_projection && can_use_q8_1_argmax(self.output.host.mode);
        match output_mode {
            CudaStepOutputMode::NoOutput => {}
            CudaStepOutputMode::FullLogits => {
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.cast_f32_to_f16(
                        &plan.matvec_input_buffer,
                        &plan.vector_f16_buffer,
                        output_cols,
                    )?;
                    submission.matmul_f16_to_f32(
                        &plan.vector_f16_buffer,
                        transposed_f16,
                        &plan.logits_buffer,
                        1,
                        output_cols,
                        output_rows,
                    )?;
                } else if output_mode_q8_1_projection {
                    submission.rms_norm_q8_1(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_q8_1_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec_q8_1(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.logits_buffer,
                    )?;
                } else {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_buffer,
                        &plan.logits_buffer,
                    )?;
                }
                bytes_moved = bytes_moved.saturating_add(
                    output_rows
                        .saturating_mul(std::mem::size_of::<f32>())
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
            }
            CudaStepOutputMode::ArgmaxOnly => {
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.cast_f32_to_f16(
                        &plan.matvec_input_buffer,
                        &plan.vector_f16_buffer,
                        output_cols,
                    )?;
                    submission.matmul_f16_to_f32(
                        &plan.vector_f16_buffer,
                        transposed_f16,
                        &plan.logits_buffer,
                        1,
                        output_cols,
                        output_rows,
                    )?;
                    submission.argmax_f32(
                        &plan.logits_buffer,
                        1,
                        output_rows,
                        &plan.next_token_buffer,
                    )?;
                    submission
                        .copy_device_to_host(&plan.next_token_buffer, &plan.next_token_host_buffer)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    bytes_moved = bytes_moved
                        .saturating_add(std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX));
                } else if output_mode_q8_1_argmax {
                    plan.argmax_state_host_buffer
                        .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.rms_norm_q8_1(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_q8_1_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission
                        .copy_host_to_device(
                            &plan.argmax_state_host_buffer,
                            &plan.argmax_state_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    submission.quantized_matvec_q8_1_argmax(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.argmax_state_buffer,
                    )?;
                    submission
                        .copy_device_to_host(
                            &plan.argmax_state_buffer,
                            &plan.argmax_state_host_buffer,
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    let argmax_bytes = std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX);
                    bytes_moved = bytes_moved
                        .saturating_add(argmax_bytes)
                        .saturating_add(argmax_bytes);
                } else if output_mode_q8_1_projection {
                    submission.rms_norm_q8_1(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_q8_1_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec_q8_1(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.logits_buffer,
                    )?;
                    submission.argmax_f32(
                        &plan.logits_buffer,
                        1,
                        output_rows,
                        &plan.next_token_buffer,
                    )?;
                    submission
                        .copy_device_to_host(&plan.next_token_buffer, &plan.next_token_host_buffer)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    bytes_moved = bytes_moved
                        .saturating_add(std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX));
                } else {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_buffer,
                        &plan.logits_buffer,
                    )?;
                    submission.argmax_f32(
                        &plan.logits_buffer,
                        1,
                        output_rows,
                        &plan.next_token_buffer,
                    )?;
                    submission
                        .copy_device_to_host(&plan.next_token_buffer, &plan.next_token_host_buffer)
                        .map_err(ReferenceTextGenerationError::Runtime)?;
                    bytes_moved = bytes_moved
                        .saturating_add(std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX));
                }
            }
            CudaStepOutputMode::TopKCandidates(top_k) => {
                if let Some(transposed_f16) = self.output.transposed_f16.as_ref() {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.cast_f32_to_f16(
                        &plan.matvec_input_buffer,
                        &plan.vector_f16_buffer,
                        output_cols,
                    )?;
                    submission.matmul_f16_to_f32(
                        &plan.vector_f16_buffer,
                        transposed_f16,
                        &plan.logits_buffer,
                        1,
                        output_cols,
                        output_rows,
                    )?;
                } else if output_mode_q8_1_projection {
                    submission.rms_norm_q8_1(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_q8_1_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec_q8_1(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_q8_1_buffer,
                        None,
                        &plan.logits_buffer,
                    )?;
                } else {
                    submission.rms_norm(
                        &plan.current_hidden_buffer,
                        &self.output_norm_device,
                        &plan.matvec_input_buffer,
                        output_cols,
                        self.family_metadata.rms_norm_epsilon,
                    )?;
                    submission.quantized_matvec(
                        &self.output.storage,
                        0,
                        self.output.host.mode,
                        output_rows,
                        output_cols,
                        &plan.matvec_input_buffer,
                        &plan.logits_buffer,
                    )?;
                }
                plan.encode_top_k_from_logits(&mut submission, output_rows, top_k)?;
                submission
                    .copy_device_to_host(
                        &plan.top_k_indices_buffer,
                        &plan.top_k_indices_host_buffer,
                    )
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                submission
                    .copy_device_to_host(&plan.top_k_values_buffer, &plan.top_k_values_host_buffer)
                    .map_err(ReferenceTextGenerationError::Runtime)?;
                bytes_moved = bytes_moved.saturating_add(
                    top_k
                        .saturating_mul(std::mem::size_of::<u32>() + std::mem::size_of::<f32>())
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
            }
        }

        let report = submission
            .commit(psionic_backend_cuda::CudaCommandWait::Completed)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        kernel_count = kernel_count.saturating_add(report.encoded_operations);
        state.position = state.position.saturating_add(1);

        let logits = match output_mode {
            CudaStepOutputMode::FullLogits => plan
                .logits_buffer
                .read_f32_at_offset(0, output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            _ => Vec::new(),
        };
        let candidates = match output_mode {
            CudaStepOutputMode::TopKCandidates(top_k) => Some(
                cuda_top_k_candidates_from_host_buffers(
                    &plan.top_k_indices_host_buffer,
                    &plan.top_k_values_host_buffer,
                    top_k,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?,
            ),
            _ => None,
        };
        let selected_token = match output_mode {
            CudaStepOutputMode::ArgmaxOnly if output_mode_q8_1_argmax => Some(
                cuda_argmax_token_from_packed_host_buffer(&plan.argmax_state_host_buffer)?,
            ),
            CudaStepOutputMode::ArgmaxOnly => Some(cuda_argmax_token_id(
                plan.next_token_host_buffer
                    .read_i32()
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            )?),
            _ => None,
        };
        let output_metrics = match output_mode {
            CudaStepOutputMode::NoOutput => None,
            CudaStepOutputMode::FullLogits => Some(qwen35_decode_output_metrics(
                Qwen35CudaDecodeOutputMode::RawLogits,
                output_rows
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                true,
                legacy_attention_backend_metrics.clone(),
            )),
            CudaStepOutputMode::ArgmaxOnly => Some(qwen35_decode_output_metrics(
                Qwen35CudaDecodeOutputMode::ArgmaxOnly,
                if output_mode_q8_1_argmax {
                    std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX)
                } else {
                    std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX)
                },
                false,
                legacy_attention_backend_metrics.clone(),
            )),
            CudaStepOutputMode::TopKCandidates(top_k) => Some(qwen35_decode_output_metrics(
                Qwen35CudaDecodeOutputMode::TopKCandidates { top_k },
                top_k
                    .saturating_mul(
                        std::mem::size_of::<u32>()
                            + if request_options.structured_output.is_some() {
                                0
                            } else {
                                std::mem::size_of::<f32>()
                            },
                    )
                    .try_into()
                    .unwrap_or(u64::MAX),
                false,
                legacy_attention_backend_metrics.clone(),
            )),
        };
        Ok(Qwen35ForwardStep {
            logits,
            selected_token,
            candidates,
            kernel_count,
            bytes_moved,
            output_metrics,
        })
    }
}

#[derive(Clone, Debug)]
struct Qwen35Layer {
    attention_norm: Vec<f32>,
    attention_norm_device: CudaBuffer,
    post_attention_norm: Vec<f32>,
    post_attention_norm_device: CudaBuffer,
    ffn_gate_up: CudaQuantizedProjectionGroup,
    ffn_down: CudaQuantizedMatrix,
    kind: Qwen35LayerKind,
}

impl Qwen35Layer {
    fn encode_full_attention_qkv_native_submission(
        &self,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        full_attention: &Qwen35FullAttentionLayer,
        hidden_size: usize,
        epsilon: f32,
        head_count: usize,
        head_dim: usize,
        query_width: usize,
    ) -> Result<(), ReferenceTextGenerationError> {
        if !qwen35_fused_qkv_rms_norm_enabled() {
            return self.encode_full_attention_qkv_native_debug_submission(
                submission,
                plan,
                full_attention,
                hidden_size,
                epsilon,
                head_count,
                head_dim,
                query_width,
            );
        }
        let native_qkv = full_attention.native_qkv.as_ref().ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                "qwen35 full-attention native qkv path requested without native matrices",
            )))
        })?;
        let query_bytes = query_width.saturating_mul(std::mem::size_of::<f32>());
        let kv_bytes = full_attention
            .kv_width
            .saturating_mul(std::mem::size_of::<f32>());
        submission.rms_norm_q8_1(
            &plan.current_hidden_buffer,
            &self.attention_norm_device,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        submission.quantized_matvec_q8_1(
            &native_qkv.query_gate.storage,
            0,
            native_qkv.query_gate.host.mode,
            native_qkv.query_gate.host.rows,
            native_qkv.query_gate.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.matvec_output_buffer,
        )?;
        if qwen35_fused_qkv_rms_norm_enabled() {
            submission.split_interleaved_query_gate_rms_norm_f32(
                &plan.matvec_output_buffer,
                head_count,
                head_dim,
                &full_attention.query_norm_device,
                epsilon,
                &plan.qkv_norm_buffer,
                &plan.gate_buffer,
            )?;
        } else {
            submission.split_interleaved_query_gate_f32(
                &plan.matvec_output_buffer,
                head_count,
                head_dim,
                &plan.q_buffer,
                &plan.gate_buffer,
            )?;
            submission.rms_norm(
                &plan.q_buffer,
                &full_attention.query_norm_device,
                &plan.q_buffer,
                query_width,
                epsilon,
            )?;
            submission.copy_buffer_region(
                &plan.q_buffer,
                0,
                &plan.qkv_norm_buffer,
                0,
                query_bytes,
            )?;
        }
        submission.quantized_matvec_q8_1(
            &native_qkv.key.storage,
            0,
            native_qkv.key.host.mode,
            native_qkv.key.host.rows,
            native_qkv.key.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.k_buffer,
        )?;
        submission.rms_norm(
            &plan.k_buffer,
            &full_attention.key_norm_device,
            &plan.q_buffer,
            full_attention.kv_width,
            epsilon,
        )?;
        submission.copy_buffer_region(
            &plan.q_buffer,
            0,
            &plan.qkv_norm_buffer,
            query_bytes,
            kv_bytes,
        )?;
        submission.quantized_matvec_q8_1(
            &native_qkv.value.storage,
            0,
            native_qkv.value.host.mode,
            native_qkv.value.host.rows,
            native_qkv.value.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.k_buffer,
        )?;
        submission.copy_buffer_region(
            &plan.k_buffer,
            0,
            &plan.qkv_norm_buffer,
            query_bytes.saturating_add(kv_bytes),
            kv_bytes,
        )?;
        Ok(())
    }

    fn encode_full_attention_qkv_native_debug_submission(
        &self,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        full_attention: &Qwen35FullAttentionLayer,
        hidden_size: usize,
        epsilon: f32,
        head_count: usize,
        head_dim: usize,
        query_width: usize,
    ) -> Result<(), ReferenceTextGenerationError> {
        let native_qkv = full_attention.native_qkv.as_ref().ok_or_else(|| {
            ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(String::from(
                "qwen35 full-attention native debug qkv path requested without native matrices",
            )))
        })?;
        let query_bytes = query_width.saturating_mul(std::mem::size_of::<f32>());
        let kv_bytes = full_attention
            .kv_width
            .saturating_mul(std::mem::size_of::<f32>());
        submission.rms_norm_q8_1(
            &plan.current_hidden_buffer,
            &self.attention_norm_device,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        submission.quantized_matvec_q8_1(
            &native_qkv.query_gate.storage,
            0,
            native_qkv.query_gate.host.mode,
            native_qkv.query_gate.host.rows,
            native_qkv.query_gate.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.matvec_output_buffer,
        )?;
        submission.split_interleaved_query_gate_f32(
            &plan.matvec_output_buffer,
            head_count,
            head_dim,
            &plan.q_buffer,
            &plan.gate_buffer,
        )?;
        submission.rms_norm(
            &plan.q_buffer,
            &full_attention.query_norm_device,
            &plan.q_buffer,
            query_width,
            epsilon,
        )?;
        submission.quantized_matvec_q8_1(
            &native_qkv.key.storage,
            0,
            native_qkv.key.host.mode,
            native_qkv.key.host.rows,
            native_qkv.key.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.k_buffer,
        )?;
        submission.rms_norm(
            &plan.k_buffer,
            &full_attention.key_norm_device,
            &plan.k_buffer,
            full_attention.kv_width,
            epsilon,
        )?;
        submission.copy_buffer_region(&plan.q_buffer, 0, &plan.qkv_norm_buffer, 0, query_bytes)?;
        submission.copy_buffer_region(
            &plan.k_buffer,
            0,
            &plan.qkv_norm_buffer,
            query_bytes,
            kv_bytes,
        )?;
        submission.quantized_matvec_q8_1(
            &native_qkv.value.storage,
            0,
            native_qkv.value.host.mode,
            native_qkv.value.host.rows,
            native_qkv.value.host.columns,
            &plan.matvec_input_q8_1_buffer,
            None,
            &plan.matvec_output_buffer,
        )?;
        submission.copy_buffer_region(
            &plan.matvec_output_buffer,
            0,
            &plan.qkv_norm_buffer,
            query_bytes.saturating_add(kv_bytes),
            kv_bytes,
        )?;
        Ok(())
    }

    fn load(
        backend: &mut CudaBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let attention_norm = load_dense_vector(artifact, layout.attention_norm.as_str())?;
        let post_attention_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_post_norm.as_deref(), "post_attention_norm")?,
        )?;
        let ffn_gate_up = load_cuda_quantized_projection_group(
            backend,
            artifact,
            &[
                required_tensor_name(layout.feed_forward_gate_weight.as_deref(), "ffn_gate")?,
                required_tensor_name(layout.feed_forward_up_weight.as_deref(), "ffn_up")?,
            ],
            false,
        )?;
        let kind = match layout.layer_kind {
            GgufDecoderLayerKind::Qwen35Hybrid => Qwen35LayerKind::Hybrid(Qwen35HybridLayer::load(
                backend, artifact, layout, metadata,
            )?),
            GgufDecoderLayerKind::Qwen35FullAttention => Qwen35LayerKind::FullAttention(
                Qwen35FullAttentionLayer::load(backend, artifact, layout, metadata)?,
            ),
            other => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!("qwen35 cuda runtime does not support layer kind `{other:?}`"),
                });
            }
        };
        Ok(Self {
            attention_norm_device: upload_f32_buffer(
                backend,
                attention_norm.as_slice(),
                "qwen35_attention_norm",
            )?,
            attention_norm,
            post_attention_norm_device: upload_f32_buffer(
                backend,
                post_attention_norm.as_slice(),
                "qwen35_post_attention_norm",
            )?,
            post_attention_norm,
            ffn_gate_up,
            ffn_down: load_cuda_quantized_matrix(
                backend,
                artifact,
                required_tensor_name(layout.feed_forward_down_weight.as_deref(), "ffn_down")?,
            )?,
            kind,
        })
    }

    fn initial_state(
        &self,
        backend: &mut CudaBackend,
        cache_capacity_tokens: usize,
    ) -> Result<Qwen35LayerState, ReferenceTextGenerationError> {
        match &self.kind {
            Qwen35LayerKind::Hybrid(layer) => {
                Ok(Qwen35LayerState::Hybrid(layer.initial_state(backend)?))
            }
            Qwen35LayerKind::FullAttention(layer) => Ok(Qwen35LayerState::FullAttention(
                layer.initial_state(backend, cache_capacity_tokens)?,
            )),
        }
    }

    fn device_residency_bytes(&self) -> usize {
        let aux_bytes = vec_f32_bytes(self.attention_norm.as_slice())
            .saturating_add(vec_f32_bytes(self.post_attention_norm.as_slice()));
        self.ffn_gate_up
            .device_residency_bytes()
            .saturating_add(self.ffn_down.device_residency_bytes())
            .saturating_add(aux_bytes)
            .saturating_add(match &self.kind {
                Qwen35LayerKind::Hybrid(layer) => layer.device_residency_bytes(),
                Qwen35LayerKind::FullAttention(layer) => layer.device_residency_bytes(),
            })
    }

    fn host_residency_bytes(&self) -> usize {
        vec_f32_bytes(self.attention_norm.as_slice())
            .saturating_add(vec_f32_bytes(self.post_attention_norm.as_slice()))
            .saturating_add(match &self.kind {
                Qwen35LayerKind::Hybrid(layer) => layer.host_residency_bytes(),
                Qwen35LayerKind::FullAttention(layer) => layer.host_residency_bytes(),
            })
    }

    fn max_matvec_input_columns(&self) -> usize {
        usize::max(
            usize::max(self.ffn_gate_up.columns, self.ffn_down.host.columns),
            match &self.kind {
                Qwen35LayerKind::Hybrid(layer) => layer.max_matvec_input_columns(),
                Qwen35LayerKind::FullAttention(layer) => layer.max_matvec_input_columns(),
            },
        )
    }

    fn max_matvec_output_rows(&self) -> usize {
        usize::max(
            usize::max(self.ffn_gate_up.total_rows(), self.ffn_down.host.rows),
            match &self.kind {
                Qwen35LayerKind::Hybrid(layer) => layer.max_matvec_output_rows(),
                Qwen35LayerKind::FullAttention(layer) => layer.max_matvec_output_rows(),
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_full_attention_device_submission(
        &self,
        backend: &mut CudaBackend,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        model: &CudaQwen35Model,
        full_attention: &Qwen35FullAttentionLayer,
        state: &mut Qwen35FullAttentionState,
        position: usize,
        initial_token: Option<TokenId>,
        use_graph_attention: bool,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        let hidden_size = model.descriptor.config.hidden_size;
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let head_count = model.descriptor.config.block.attention.head_count;
        let head_dim = model.descriptor.config.block.attention.head_dim;
        let rotary_dim = model.descriptor.config.block.attention.rotary_dim;
        let query_width = head_count.saturating_mul(head_dim);
        let kv_head_count = full_attention.kv_width / head_dim.max(1);
        let query_gate_rows = full_attention.qkv.rows_per_projection[0];
        let key_rows = full_attention.qkv.rows_per_projection[1];
        let value_rows = full_attention.qkv.rows_per_projection[2];
        if query_gate_rows != query_width.saturating_mul(2) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention cuda path requires query/gate rows={} for query width {}, actual {}",
                    query_width.saturating_mul(2),
                    query_width,
                    query_gate_rows,
                )),
            ));
        }
        if key_rows != full_attention.kv_width || value_rows != full_attention.kv_width {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention cuda path requires key/value rows={} and {} to match kv width {}, actual key={} value={}",
                    full_attention.kv_width,
                    full_attention.kv_width,
                    full_attention.kv_width,
                    key_rows,
                    value_rows,
                )),
            ));
        }
        if full_attention.output.host.columns != query_width {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention output width mismatch: expected {}, actual {}",
                    query_width, full_attention.output.host.columns,
                )),
            ));
        }
        state
            .ensure_capacity(backend, state.len.saturating_add(1))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let (freq_scale, ext_factor, corr_dims, theta_scale) =
            qwen35_rope_runtime_parameters(rotary_dim, &model.family_metadata);
        if let Some(token) = initial_token {
            *bytes_moved = bytes_moved.saturating_add(
                model.encode_token_embedding_lookup(submission, plan, token, position)?,
            );
        }
        if full_attention.native_qkv.is_some() {
            self.encode_full_attention_qkv_native_submission(
                submission,
                plan,
                full_attention,
                hidden_size,
                epsilon,
                head_count,
                head_dim,
                query_width,
            )?;
        } else if let Some(transposed_f16) = full_attention.qkv.transposed_f16.as_ref() {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                full_attention.qkv.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                full_attention.qkv.columns,
                full_attention.qkv.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(full_attention.qkv.mode) {
            submission.rms_norm_q8_1(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                &full_attention.qkv.storage,
                0,
                full_attention.qkv.mode,
                full_attention.qkv.total_rows(),
                full_attention.qkv.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec(
                &full_attention.qkv.storage,
                0,
                full_attention.qkv.mode,
                full_attention.qkv.total_rows(),
                full_attention.qkv.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        if full_attention.native_qkv.is_none() {
            encode_qwen35_full_attention_qkv_post_matvec(
                submission,
                &plan.matvec_output_buffer,
                head_count,
                head_dim,
                query_gate_rows,
                key_rows,
                kv_head_count,
                &full_attention.query_norm_device,
                &full_attention.key_norm_device,
                epsilon,
                &plan.q_buffer,
                &plan.k_buffer,
                &plan.qkv_norm_buffer,
                &plan.gate_buffer,
                query_width,
                full_attention.kv_width,
            )?;
        }
        let attention_backend_selection = model.full_attention_backend_selection(
            backend,
            full_attention,
            state,
            use_graph_attention,
        );
        if use_graph_attention {
            match attention_backend_selection.executed_backend {
                PsionRvllmFa3DecodeAttentionBackendKind::Fa3SplitKvF16KvGraph => {
                    submission.attention_decode_rope_cache_f16_kv_graph_fa3(
                        &plan.qkv_norm_buffer,
                        0,
                        query_width,
                        query_width.saturating_add(full_attention.kv_width),
                        &state.key_cache,
                        &state.value_cache,
                        state.width,
                        0,
                        &plan.decode_params_buffer,
                        model.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        None,
                        &plan.attention_fa3_partial_output_buffer,
                        &plan.attention_fa3_partial_max_buffer,
                        &plan.attention_fa3_partial_sum_buffer,
                        &plan.gated_delta_buffer,
                    )?;
                }
                PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvGraphLegacy
                | PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvLegacy => {
                    submission.attention_decode_rope_cache_f16_kv_graph(
                        &plan.qkv_norm_buffer,
                        0,
                        query_width,
                        query_width.saturating_add(full_attention.kv_width),
                        &state.key_cache,
                        &state.value_cache,
                        state.width,
                        0,
                        &plan.decode_params_buffer,
                        model.family_metadata.sliding_window.unwrap_or(0),
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims,
                        theta_scale,
                        None,
                        &plan.gated_delta_buffer,
                    )?;
                }
            }
        } else {
            submission.attention_decode_rope_cache_f16_kv(
                &plan.qkv_norm_buffer,
                0,
                query_width,
                query_width.saturating_add(full_attention.kv_width),
                &state.key_cache,
                &state.value_cache,
                state.width,
                0,
                state.len,
                model.family_metadata.sliding_window.unwrap_or(0),
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                None,
                &plan.gated_delta_buffer,
            )?;
        }
        if let Some(transposed_f16) = full_attention.output.transposed_f16.as_ref() {
            submission.sigmoid_mul_f32(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                full_attention.output.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                full_attention.output.host.columns,
                full_attention.output.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(full_attention.output.host.mode) {
            submission.sigmoid_mul_q8_1(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &full_attention.output.storage,
                0,
                full_attention.output.host.mode,
                full_attention.output.host.rows,
                full_attention.output.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.sigmoid_mul_f32(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &full_attention.output.storage,
                0,
                full_attention.output.host.mode,
                full_attention.output.host.rows,
                full_attention.output.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_residual_rms_norm_q8_1(
            &plan.projected_buffer,
            &plan.current_hidden_buffer,
            None,
            &self.post_attention_norm_device,
            &plan.current_hidden_buffer,
            &plan.hidden_norm_buffer,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                self.ffn_gate_up.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                self.ffn_gate_up.columns,
                self.ffn_gate_up.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_gate_up.mode) {
            submission.quantized_matvec_q8_1(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        let gate_rows = self.ffn_gate_up.rows_per_projection[0];
        let up_rows = self.ffn_gate_up.rows_per_projection[1];
        if gate_rows != up_rows {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                    gate_rows, up_rows
                )),
            ));
        }
        if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                self.ffn_down.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                self.ffn_down.host.columns,
                self.ffn_down.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_down.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_f32_in_place(
            &plan.current_hidden_buffer,
            0,
            &plan.projected_buffer,
            hidden_size,
        )?;
        state.len = state.len.saturating_add(1);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_hybrid_device_submission(
        &self,
        _backend: &mut CudaBackend,
        submission: &mut CudaSubmission,
        plan: &mut Qwen35CudaStepPlan,
        model: &CudaQwen35Model,
        state: &mut Qwen35HybridState,
        position: usize,
        initial_token: Option<TokenId>,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        let Qwen35LayerKind::Hybrid(hybrid) = &self.kind else {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(String::from(
                    "qwen35 cuda hybrid path requires a hybrid layer",
                )),
            ));
        };
        let hidden_size = model.descriptor.config.hidden_size;
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let qkv_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[0];
        let z_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[1];
        let alpha_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[2];
        let beta_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[3];
        if alpha_rows != beta_rows
            || alpha_rows != hybrid.ssm_a.len()
            || alpha_rows != hybrid.ssm_dt.len()
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 hybrid cuda path requires aligned alpha/beta widths, got alpha={} beta={} ssm_a={} ssm_dt={}",
                    alpha_rows,
                    beta_rows,
                    hybrid.ssm_a.len(),
                    hybrid.ssm_dt.len()
                )),
            ));
        }
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);
        let z_offset = qkv_rows;
        let alpha_offset = z_offset.saturating_add(z_rows);
        let beta_offset = alpha_offset.saturating_add(alpha_rows);

        if let Some(token) = initial_token {
            *bytes_moved = bytes_moved.saturating_add(
                model.encode_token_embedding_lookup(submission, plan, token, position)?,
            );
        }
        if let Some(transposed_f16) = hybrid.qkv_gate_alpha_beta.transposed_f16.as_ref() {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                hybrid.qkv_gate_alpha_beta.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                hybrid.qkv_gate_alpha_beta.columns,
                hybrid.qkv_gate_alpha_beta.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(hybrid.qkv_gate_alpha_beta.mode) {
            submission.rms_norm_q8_1(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                &hybrid.qkv_gate_alpha_beta.storage,
                0,
                hybrid.qkv_gate_alpha_beta.mode,
                hybrid.qkv_gate_alpha_beta.total_rows(),
                hybrid.qkv_gate_alpha_beta.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec(
                &hybrid.qkv_gate_alpha_beta.storage,
                0,
                hybrid.qkv_gate_alpha_beta.mode,
                hybrid.qkv_gate_alpha_beta.total_rows(),
                hybrid.qkv_gate_alpha_beta.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        submission.depthwise_causal_conv1d_step_silu_f32(
            &plan.matvec_output_buffer,
            &state.conv_state,
            &hybrid.ssm_conv1d_device,
            qkv_rows,
            hybrid.conv_kernel,
            &plan.conv_buffer,
        )?;
        submission.qwen35_ssm_decay_beta_f32(
            &plan.matvec_output_buffer,
            alpha_offset,
            beta_offset,
            &hybrid.ssm_a_device,
            &hybrid.ssm_dt_device,
            alpha_rows,
            &plan.decay_buffer,
            &plan.beta_buffer,
        )?;
        encode_qwen35_hybrid_qkv_post_conv(
            submission,
            &plan.conv_buffer,
            hybrid.group_count,
            hybrid.state_size,
            v_offset,
            v_size,
            &hybrid.q_scale_device,
            &hybrid.k_scale_device,
            &plan.q_buffer,
            &plan.k_buffer,
            &plan.qkv_norm_buffer,
        )?;
        submission.gated_delta_step_f32(
            &plan.qkv_norm_buffer,
            0,
            q_size,
            v_offset,
            &plan.decay_buffer,
            &plan.beta_buffer,
            &state.delta_state,
            hybrid.group_count,
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.state_size,
            hybrid.v_head_reordered,
            &plan.gated_delta_buffer,
        )?;
        submission.rms_norm_region(
            &plan.gated_delta_buffer,
            0,
            &hybrid.ssm_norm_device,
            &plan.hybrid_norm_buffer,
            0,
            v_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = hybrid.ssm_out.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                hybrid.ssm_out.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                hybrid.ssm_out.host.columns,
                hybrid.ssm_out.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(hybrid.ssm_out.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &hybrid.ssm_out.storage,
                0,
                hybrid.ssm_out.host.mode,
                hybrid.ssm_out.host.rows,
                hybrid.ssm_out.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &hybrid.ssm_out.storage,
                0,
                hybrid.ssm_out.host.mode,
                hybrid.ssm_out.host.rows,
                hybrid.ssm_out.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_residual_rms_norm_q8_1(
            &plan.projected_buffer,
            &plan.current_hidden_buffer,
            None,
            &self.post_attention_norm_device,
            &plan.current_hidden_buffer,
            &plan.hidden_norm_buffer,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                self.ffn_gate_up.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                self.ffn_gate_up.columns,
                self.ffn_gate_up.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_gate_up.mode) {
            submission.quantized_matvec_q8_1(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        let gate_rows = self.ffn_gate_up.rows_per_projection[0];
        let up_rows = self.ffn_gate_up.rows_per_projection[1];
        if gate_rows != up_rows {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                    gate_rows, up_rows
                )),
            ));
        }
        if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                self.ffn_down.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                self.ffn_down.host.columns,
                self.ffn_down.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_down.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_f32_in_place(
            &plan.current_hidden_buffer,
            0,
            &plan.projected_buffer,
            hidden_size,
        )?;
        Ok(())
    }

    fn forward_host(
        &self,
        _backend: &mut CudaBackend,
        _plan: &mut Qwen35CudaStepPlan,
        _model: &CudaQwen35Model,
        _position: usize,
        _hidden: Vec<f32>,
        _state: &mut Qwen35LayerState,
        _kernel_count: &mut usize,
        _bytes_moved: &mut u64,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(String::from(
                "qwen35 host layer path is disabled; use the cuda-native device path",
            )),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn forward_full_attention_device(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        model: &CudaQwen35Model,
        layer_index: usize,
        full_attention: &Qwen35FullAttentionLayer,
        state: &mut Qwen35FullAttentionState,
        initial_token: Option<TokenId>,
        position: usize,
        mrope_position: [usize; 3],
        kernel_count: &mut usize,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        let hidden_size = model.descriptor.config.hidden_size;
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let head_count = model.descriptor.config.block.attention.head_count;
        let head_dim = model.descriptor.config.block.attention.head_dim;
        let rotary_dim = model.descriptor.config.block.attention.rotary_dim;
        let query_width = head_count.saturating_mul(head_dim);
        let kv_head_count = full_attention.kv_width / head_dim.max(1);
        let query_gate_rows = full_attention.qkv.rows_per_projection[0];
        let key_rows = full_attention.qkv.rows_per_projection[1];
        let value_rows = full_attention.qkv.rows_per_projection[2];
        if query_gate_rows != query_width.saturating_mul(2) {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention cuda path requires query/gate rows={} for query width {}, actual {}",
                    query_width.saturating_mul(2),
                    query_width,
                    query_gate_rows,
                )),
            ));
        }
        if key_rows != full_attention.kv_width || value_rows != full_attention.kv_width {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention cuda path requires key/value rows={} and {} to match kv width {}, actual key={} value={}",
                    full_attention.kv_width,
                    full_attention.kv_width,
                    full_attention.kv_width,
                    key_rows,
                    value_rows,
                )),
            ));
        }
        if full_attention.output.host.columns != query_width {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 full-attention output width mismatch: expected {}, actual {}",
                    query_width, full_attention.output.host.columns,
                )),
            ));
        }
        state
            .ensure_capacity(backend, state.len.saturating_add(1))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let (freq_scale, ext_factor, corr_dims, theta_scale) =
            qwen35_rope_runtime_parameters(rotary_dim, &model.family_metadata);
        let mrope_sections = qwen35_mrope_sections(&model.family_metadata);
        let mrope_interleaved = qwen35_mrope_interleaved(&model.family_metadata);
        let query_bytes = query_width.saturating_mul(std::mem::size_of::<f32>());
        let kv_bytes = full_attention
            .kv_width
            .saturating_mul(std::mem::size_of::<f32>());
        let debug_attention = std::env::var_os("PSIONIC_QWEN35_DEBUG_ATTENTION").is_some();

        if debug_attention {
            eprintln!(
                "qwen35_debug_layout position={} state_len={} query_width={} kv_width={} query_gate_rows={} key_rows={} value_rows={} q_buffer_bytes={} k_buffer_bytes={} gate_buffer_bytes={} qkv_norm_bytes={} hidden_norm_bytes={} matvec_output_bytes={} key_cache_bytes={} value_cache_bytes={}",
                position,
                state.len,
                query_width,
                full_attention.kv_width,
                query_gate_rows,
                key_rows,
                value_rows,
                plan.q_buffer.byte_len(),
                plan.k_buffer.byte_len(),
                plan.gate_buffer.byte_len(),
                plan.qkv_norm_buffer.byte_len(),
                plan.hidden_norm_buffer.byte_len(),
                plan.matvec_output_buffer.byte_len(),
                state.key_cache.byte_len(),
                state.value_cache.byte_len(),
            );
            let mut prep = backend
                .begin_submission()
                .map_err(ReferenceTextGenerationError::Runtime)?;
            if let Some(token) = initial_token {
                *bytes_moved = bytes_moved.saturating_add(
                    model.encode_token_embedding_lookup(&mut prep, plan, token, position)?,
                );
            }
            if full_attention.native_qkv.is_some() {
                self.encode_full_attention_qkv_native_debug_submission(
                    &mut prep,
                    plan,
                    full_attention,
                    hidden_size,
                    epsilon,
                    head_count,
                    head_dim,
                    query_width,
                )?;
            } else if let Some(transposed_f16) = full_attention.qkv.transposed_f16.as_ref() {
                prep.rms_norm(
                    &plan.current_hidden_buffer,
                    &self.attention_norm_device,
                    &plan.hidden_norm_buffer,
                    hidden_size,
                    epsilon,
                )?;
                prep.cast_f32_to_f16(
                    &plan.hidden_norm_buffer,
                    &plan.vector_f16_buffer,
                    full_attention.qkv.columns,
                )?;
                prep.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &plan.matvec_output_buffer,
                    1,
                    full_attention.qkv.columns,
                    full_attention.qkv.total_rows(),
                )?;
            } else {
                prep.rms_norm_q8_1(
                    &plan.current_hidden_buffer,
                    &self.attention_norm_device,
                    &plan.matvec_input_q8_1_buffer,
                    hidden_size,
                    epsilon,
                )?;
                prep.quantized_matvec_q8_1(
                    &full_attention.qkv.storage,
                    0,
                    full_attention.qkv.mode,
                    full_attention.qkv.total_rows(),
                    full_attention.qkv.columns,
                    &plan.matvec_input_q8_1_buffer,
                    None,
                    &plan.matvec_output_buffer,
                )?;
            }
            if full_attention.native_qkv.is_none() {
                prep.split_interleaved_query_gate_f32(
                    &plan.matvec_output_buffer,
                    head_count,
                    head_dim,
                    &plan.q_buffer,
                    &plan.gate_buffer,
                )?;
                prep.copy_buffer_region(
                    &plan.matvec_output_buffer,
                    query_gate_rows.saturating_mul(std::mem::size_of::<f32>()),
                    &plan.k_buffer,
                    0,
                    kv_bytes,
                )?;
                prep.rms_norm(
                    &plan.q_buffer,
                    &full_attention.query_norm_device,
                    &plan.q_buffer,
                    query_width,
                    epsilon,
                )?;
                prep.rms_norm(
                    &plan.k_buffer,
                    &full_attention.key_norm_device,
                    &plan.k_buffer,
                    full_attention.kv_width,
                    epsilon,
                )?;
                prep.copy_buffer_region(&plan.q_buffer, 0, &plan.qkv_norm_buffer, 0, query_bytes)?;
                prep.copy_buffer_region(
                    &plan.k_buffer,
                    0,
                    &plan.qkv_norm_buffer,
                    query_bytes,
                    kv_bytes,
                )?;
                prep.copy_buffer_region(
                    &plan.matvec_output_buffer,
                    query_gate_rows
                        .saturating_add(key_rows)
                        .saturating_mul(std::mem::size_of::<f32>()),
                    &plan.qkv_norm_buffer,
                    query_bytes.saturating_add(kv_bytes),
                    kv_bytes,
                )?;
            }
            let prep_report = prep.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            *kernel_count = kernel_count.saturating_add(prep_report.encoded_operations);

            let q_host = plan
                .q_buffer
                .read_f32_at_offset(0, query_width)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let gate_host = plan
                .gate_buffer
                .read_f32_at_offset(0, query_width)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let k_host = plan
                .k_buffer
                .read_f32_at_offset(0, full_attention.kv_width)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let v_host = plan
                .qkv_norm_buffer
                .read_f32_at_offset(
                    query_width.saturating_add(full_attention.kv_width),
                    full_attention.kv_width,
                )
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let cache_key_host = if state.len > 0 {
                f16_bytes_to_f32_vec(
                    state
                        .key_cache
                        .read_bytes_at_offset(
                            0,
                            state
                                .len
                                .saturating_mul(state.width)
                                .saturating_mul(state.element_bytes),
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?
                        .as_slice(),
                )
                .map_err(ReferenceTextGenerationError::Runtime)?
            } else {
                Vec::new()
            };
            let cache_value_host = if state.len > 0 {
                f16_bytes_to_f32_vec(
                    state
                        .value_cache
                        .read_bytes_at_offset(
                            0,
                            state
                                .len
                                .saturating_mul(state.width)
                                .saturating_mul(state.element_bytes),
                        )
                        .map_err(ReferenceTextGenerationError::Runtime)?
                        .as_slice(),
                )
                .map_err(ReferenceTextGenerationError::Runtime)?
            } else {
                Vec::new()
            };
            let mut q_rot = q_host.clone();
            let mut k_rot = k_host.clone();
            apply_rope_neox_mrope(
                q_rot.as_mut_slice(),
                head_count,
                head_dim,
                rotary_dim,
                mrope_position,
                &model.family_metadata,
            );
            apply_rope_neox_mrope(
                k_rot.as_mut_slice(),
                kv_head_count,
                head_dim,
                rotary_dim,
                mrope_position,
                &model.family_metadata,
            );
            let cache_entries = cache_key_host
                .chunks_exact(state.width)
                .zip(cache_value_host.chunks_exact(state.width))
                .map(|(key, value)| Qwen35FullAttentionEntry {
                    key: key.to_vec(),
                    value: value.to_vec(),
                })
                .collect::<Vec<_>>();
            let host_attention = attend_full_attention(
                q_rot.as_slice(),
                k_rot.as_slice(),
                v_host.as_slice(),
                cache_entries.as_slice(),
                head_count,
                kv_head_count,
                head_dim,
                qwen35_attention_scale(&model.family_metadata, head_dim),
                model.family_metadata.sliding_window,
            );
            let host_gated = host_attention
                .iter()
                .copied()
                .zip(gate_host.iter().copied())
                .map(|(value, gate)| value * sigmoid(gate))
                .collect::<Vec<_>>();
            let input_hidden = plan
                .current_hidden_buffer
                .read_f32_at_offset(0, hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let host_projected = full_attention
                .output
                .host_matvec(host_gated.as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let host_post_attention =
                add_vectors(host_projected.as_slice(), input_hidden.as_slice())
                    .map_err(ReferenceTextGenerationError::Runtime)?;
            let host_post_attention_norm = rms_norm(
                host_post_attention.as_slice(),
                self.post_attention_norm.as_slice(),
                epsilon,
            );
            let host_gate_up = self
                .ffn_gate_up
                .host_matvec(host_post_attention_norm.as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let host_ffn = silu_glu(
                host_gate_up
                    .slice(0)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
                host_gate_up
                    .slice(1)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
            );
            let host_ffn_down = self
                .ffn_down
                .host_matvec(host_ffn.as_slice())
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let host_final_hidden =
                add_vectors(host_post_attention.as_slice(), host_ffn_down.as_slice())
                    .map_err(ReferenceTextGenerationError::Runtime)?;

            let mut attention = backend
                .begin_submission()
                .map_err(ReferenceTextGenerationError::Runtime)?;
            if let Some(sections) = mrope_sections {
                attention.attention_decode_mrope_cache_f16_kv(
                    &plan.qkv_norm_buffer,
                    0,
                    query_width,
                    query_width.saturating_add(full_attention.kv_width),
                    &state.key_cache,
                    &state.value_cache,
                    state.width,
                    0,
                    state.len,
                    model.family_metadata.sliding_window.unwrap_or(0),
                    head_count,
                    kv_head_count,
                    head_dim,
                    rotary_dim,
                    mrope_position,
                    sections,
                    mrope_interleaved,
                    freq_scale,
                    ext_factor,
                    corr_dims,
                    theta_scale,
                    None,
                    &plan.gated_delta_buffer,
                )?;
            } else {
                attention.attention_decode_rope_cache_f16_kv(
                    &plan.qkv_norm_buffer,
                    0,
                    query_width,
                    query_width.saturating_add(full_attention.kv_width),
                    &state.key_cache,
                    &state.value_cache,
                    state.width,
                    0,
                    state.len,
                    model.family_metadata.sliding_window.unwrap_or(0),
                    head_count,
                    kv_head_count,
                    head_dim,
                    rotary_dim,
                    mrope_position[0],
                    freq_scale,
                    ext_factor,
                    corr_dims,
                    theta_scale,
                    None,
                    &plan.gated_delta_buffer,
                )?;
            }
            attention.sigmoid_mul_f32(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.gated_delta_buffer,
            )?;
            let attention_report =
                attention.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            *kernel_count = kernel_count.saturating_add(attention_report.encoded_operations);
            let device_gated = plan
                .gated_delta_buffer
                .read_f32_at_offset(0, query_width)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let max_diff = host_gated
                .iter()
                .copied()
                .zip(device_gated.iter().copied())
                .map(|(left, right)| (left - right).abs())
                .fold(0.0_f32, f32::max);
            eprintln!(
                "qwen35_debug position={} state_len={} max_gated_diff={max_diff:.6}",
                position, state.len
            );

            let mut tail = backend
                .begin_submission()
                .map_err(ReferenceTextGenerationError::Runtime)?;
            if let Some(transposed_f16) = full_attention.output.transposed_f16.as_ref() {
                tail.cast_f32_to_f16(
                    &plan.gated_delta_buffer,
                    &plan.vector_f16_buffer,
                    full_attention.output.host.columns,
                )?;
                tail.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &plan.projected_buffer,
                    1,
                    full_attention.output.host.columns,
                    full_attention.output.host.rows,
                )?;
            } else {
                tail.quantize_f32_to_q8_1(
                    &plan.gated_delta_buffer,
                    1,
                    query_width,
                    &plan.activated_q8_1_buffer,
                )?;
                tail.quantized_matvec_q8_1(
                    &full_attention.output.storage,
                    0,
                    full_attention.output.host.mode,
                    full_attention.output.host.rows,
                    full_attention.output.host.columns,
                    &plan.activated_q8_1_buffer,
                    None,
                    &plan.projected_buffer,
                )?;
            }
            tail.add_residual_rms_norm_q8_1(
                &plan.projected_buffer,
                &plan.current_hidden_buffer,
                None,
                &self.post_attention_norm_device,
                &plan.current_hidden_buffer,
                &plan.hidden_norm_buffer,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
                tail.cast_f32_to_f16(
                    &plan.hidden_norm_buffer,
                    &plan.vector_f16_buffer,
                    self.ffn_gate_up.columns,
                )?;
                tail.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &plan.matvec_output_buffer,
                    1,
                    self.ffn_gate_up.columns,
                    self.ffn_gate_up.total_rows(),
                )?;
            } else {
                tail.quantized_matvec_q8_1(
                    &self.ffn_gate_up.storage,
                    0,
                    self.ffn_gate_up.mode,
                    self.ffn_gate_up.total_rows(),
                    self.ffn_gate_up.columns,
                    &plan.matvec_input_q8_1_buffer,
                    None,
                    &plan.matvec_output_buffer,
                )?;
            }
            let gate_rows = self.ffn_gate_up.rows_per_projection[0];
            let up_rows = self.ffn_gate_up.rows_per_projection[1];
            if gate_rows != up_rows {
                return Err(ReferenceTextGenerationError::Runtime(
                    crate::RuntimeError::Backend(format!(
                        "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                        gate_rows, up_rows
                    )),
                ));
            }
            if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
                tail.silu_mul_f32(
                    &plan.matvec_output_buffer,
                    0,
                    &plan.matvec_output_buffer,
                    gate_rows,
                    gate_rows,
                    &plan.gated_delta_buffer,
                )?;
                tail.cast_f32_to_f16(
                    &plan.gated_delta_buffer,
                    &plan.vector_f16_buffer,
                    self.ffn_down.host.columns,
                )?;
                tail.matmul_f16_to_f32(
                    &plan.vector_f16_buffer,
                    transposed_f16,
                    &plan.projected_buffer,
                    1,
                    self.ffn_down.host.columns,
                    self.ffn_down.host.rows,
                )?;
            } else {
                tail.silu_mul_q8_1(
                    &plan.matvec_output_buffer,
                    0,
                    &plan.matvec_output_buffer,
                    gate_rows,
                    gate_rows,
                    &plan.activated_q8_1_buffer,
                )?;
                tail.quantized_matvec_q8_1(
                    &self.ffn_down.storage,
                    0,
                    self.ffn_down.host.mode,
                    self.ffn_down.host.rows,
                    self.ffn_down.host.columns,
                    &plan.activated_q8_1_buffer,
                    None,
                    &plan.projected_buffer,
                )?;
            }
            tail.add_f32_in_place(
                &plan.current_hidden_buffer,
                0,
                &plan.projected_buffer,
                hidden_size,
            )?;
            let tail_report = tail.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            *kernel_count = kernel_count.saturating_add(tail_report.encoded_operations);
            let device_final_hidden = plan
                .current_hidden_buffer
                .read_f32_at_offset(0, hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            state.len = state.len.saturating_add(1);
            eprintln!(
                "qwen35_debug layer={} position={} state_len={} max_gated_diff={max_diff:.6} final_hidden_diff={:.6}",
                layer_index,
                position,
                state.len.saturating_sub(1),
                max_abs_diff(device_final_hidden.as_slice(), host_final_hidden.as_slice())?,
            );
            return Ok(());
        }

        let mut submission = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if let Some(token) = initial_token {
            *bytes_moved = bytes_moved.saturating_add(model.encode_token_embedding_lookup(
                &mut submission,
                plan,
                token,
                position,
            )?);
        }
        if full_attention.native_qkv.is_some() {
            self.encode_full_attention_qkv_native_submission(
                &mut submission,
                plan,
                full_attention,
                hidden_size,
                epsilon,
                head_count,
                head_dim,
                query_width,
            )?;
        } else if can_use_q8_1_quantized_matvec(full_attention.qkv.mode) {
            submission.rms_norm_q8_1(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                &full_attention.qkv.storage,
                0,
                full_attention.qkv.mode,
                full_attention.qkv.total_rows(),
                full_attention.qkv.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec(
                &full_attention.qkv.storage,
                0,
                full_attention.qkv.mode,
                full_attention.qkv.total_rows(),
                full_attention.qkv.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        if full_attention.native_qkv.is_none() {
            encode_qwen35_full_attention_qkv_post_matvec(
                &mut submission,
                &plan.matvec_output_buffer,
                head_count,
                head_dim,
                query_gate_rows,
                key_rows,
                kv_head_count,
                &full_attention.query_norm_device,
                &full_attention.key_norm_device,
                epsilon,
                &plan.q_buffer,
                &plan.k_buffer,
                &plan.qkv_norm_buffer,
                &plan.gate_buffer,
                query_width,
                full_attention.kv_width,
            )?;
        }
        if let Some(sections) = mrope_sections {
            submission.attention_decode_mrope_cache_f16_kv(
                &plan.qkv_norm_buffer,
                0,
                query_width,
                query_width.saturating_add(full_attention.kv_width),
                &state.key_cache,
                &state.value_cache,
                state.width,
                0,
                state.len,
                model.family_metadata.sliding_window.unwrap_or(0),
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                mrope_position,
                sections,
                mrope_interleaved,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                None,
                &plan.gated_delta_buffer,
            )?;
        } else {
            submission.attention_decode_rope_cache_f16_kv(
                &plan.qkv_norm_buffer,
                0,
                query_width,
                query_width.saturating_add(full_attention.kv_width),
                &state.key_cache,
                &state.value_cache,
                state.width,
                0,
                state.len,
                model.family_metadata.sliding_window.unwrap_or(0),
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                mrope_position[0],
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                None,
                &plan.gated_delta_buffer,
            )?;
        }
        if can_use_q8_1_quantized_matvec(full_attention.output.host.mode) {
            submission.sigmoid_mul_q8_1(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &full_attention.output.storage,
                0,
                full_attention.output.host.mode,
                full_attention.output.host.rows,
                full_attention.output.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.sigmoid_mul_f32(
                &plan.gated_delta_buffer,
                0,
                &plan.gate_buffer,
                0,
                query_width,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &full_attention.output.storage,
                0,
                full_attention.output.host.mode,
                full_attention.output.host.rows,
                full_attention.output.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_residual_rms_norm_q8_1(
            &plan.projected_buffer,
            &plan.current_hidden_buffer,
            None,
            &self.post_attention_norm_device,
            &plan.current_hidden_buffer,
            &plan.hidden_norm_buffer,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                self.ffn_gate_up.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                self.ffn_gate_up.columns,
                self.ffn_gate_up.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_gate_up.mode) {
            submission.quantized_matvec_q8_1(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        let gate_rows = self.ffn_gate_up.rows_per_projection[0];
        let up_rows = self.ffn_gate_up.rows_per_projection[1];
        if gate_rows != up_rows {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                    gate_rows, up_rows
                )),
            ));
        }
        if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                self.ffn_down.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                self.ffn_down.host.columns,
                self.ffn_down.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_down.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_f32_in_place(
            &plan.current_hidden_buffer,
            0,
            &plan.projected_buffer,
            hidden_size,
        )?;
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        *kernel_count = kernel_count.saturating_add(report.encoded_operations);
        state.len = state.len.saturating_add(1);
        Ok(())
    }

    fn forward_hybrid_device(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        model: &CudaQwen35Model,
        layer_index: usize,
        state: &mut Qwen35HybridState,
        position: usize,
        initial_token: Option<TokenId>,
        kernel_count: &mut usize,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        let Qwen35LayerKind::Hybrid(hybrid) = &self.kind else {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(String::from(
                    "qwen35 cuda hybrid path requires a hybrid layer",
                )),
            ));
        };
        if qwen35_hybrid_compare_enabled(layer_index, position) {
            return self.forward_hybrid_device_debug_compare(
                backend,
                plan,
                model,
                layer_index,
                hybrid,
                state,
                position,
                initial_token,
                kernel_count,
                bytes_moved,
            );
        }
        let hidden_size = model.descriptor.config.hidden_size;
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let qkv_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[0];
        let z_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[1];
        let alpha_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[2];
        let beta_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[3];
        if alpha_rows != beta_rows
            || alpha_rows != hybrid.ssm_a.len()
            || alpha_rows != hybrid.ssm_dt.len()
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 hybrid cuda path requires aligned alpha/beta widths, got alpha={} beta={} ssm_a={} ssm_dt={}",
                    alpha_rows,
                    beta_rows,
                    hybrid.ssm_a.len(),
                    hybrid.ssm_dt.len()
                )),
            ));
        }
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);
        let z_offset = qkv_rows;
        let alpha_offset = z_offset.saturating_add(z_rows);
        let beta_offset = alpha_offset.saturating_add(alpha_rows);

        let mut submission = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if let Some(token) = initial_token {
            *bytes_moved = bytes_moved.saturating_add(model.encode_token_embedding_lookup(
                &mut submission,
                plan,
                token,
                position,
            )?);
        }
        if let Some(transposed_f16) = hybrid.qkv_gate_alpha_beta.transposed_f16.as_ref() {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                hybrid.qkv_gate_alpha_beta.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                hybrid.qkv_gate_alpha_beta.columns,
                hybrid.qkv_gate_alpha_beta.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(hybrid.qkv_gate_alpha_beta.mode) {
            submission.rms_norm_q8_1(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                &hybrid.qkv_gate_alpha_beta.storage,
                0,
                hybrid.qkv_gate_alpha_beta.mode,
                hybrid.qkv_gate_alpha_beta.total_rows(),
                hybrid.qkv_gate_alpha_beta.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            submission.quantized_matvec(
                &hybrid.qkv_gate_alpha_beta.storage,
                0,
                hybrid.qkv_gate_alpha_beta.mode,
                hybrid.qkv_gate_alpha_beta.total_rows(),
                hybrid.qkv_gate_alpha_beta.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        submission.depthwise_causal_conv1d_step_silu_f32(
            &plan.matvec_output_buffer,
            &state.conv_state,
            &hybrid.ssm_conv1d_device,
            qkv_rows,
            hybrid.conv_kernel,
            &plan.conv_buffer,
        )?;
        submission.qwen35_ssm_decay_beta_f32(
            &plan.matvec_output_buffer,
            alpha_offset,
            beta_offset,
            &hybrid.ssm_a_device,
            &hybrid.ssm_dt_device,
            alpha_rows,
            &plan.decay_buffer,
            &plan.beta_buffer,
        )?;
        encode_qwen35_hybrid_qkv_post_conv(
            &mut submission,
            &plan.conv_buffer,
            hybrid.group_count,
            hybrid.state_size,
            v_offset,
            v_size,
            &hybrid.q_scale_device,
            &hybrid.k_scale_device,
            &plan.q_buffer,
            &plan.k_buffer,
            &plan.qkv_norm_buffer,
        )?;
        submission.gated_delta_step_f32(
            &plan.qkv_norm_buffer,
            0,
            q_size,
            v_offset,
            &plan.decay_buffer,
            &plan.beta_buffer,
            &state.delta_state,
            hybrid.group_count,
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.state_size,
            hybrid.v_head_reordered,
            &plan.gated_delta_buffer,
        )?;
        submission.rms_norm_region(
            &plan.gated_delta_buffer,
            0,
            &hybrid.ssm_norm_device,
            &plan.hybrid_norm_buffer,
            0,
            v_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = hybrid.ssm_out.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                hybrid.ssm_out.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                hybrid.ssm_out.host.columns,
                hybrid.ssm_out.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(hybrid.ssm_out.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &hybrid.ssm_out.storage,
                0,
                hybrid.ssm_out.host.mode,
                hybrid.ssm_out.host.rows,
                hybrid.ssm_out.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &hybrid.ssm_out.storage,
                0,
                hybrid.ssm_out.host.mode,
                hybrid.ssm_out.host.rows,
                hybrid.ssm_out.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_residual_rms_norm_q8_1(
            &plan.projected_buffer,
            &plan.current_hidden_buffer,
            None,
            &self.post_attention_norm_device,
            &plan.current_hidden_buffer,
            &plan.hidden_norm_buffer,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
            submission.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                self.ffn_gate_up.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                self.ffn_gate_up.columns,
                self.ffn_gate_up.total_rows(),
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_gate_up.mode) {
            submission.quantized_matvec_q8_1(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.hidden_norm_buffer,
                &plan.matvec_output_buffer,
            )?;
        }
        let gate_rows = self.ffn_gate_up.rows_per_projection[0];
        let up_rows = self.ffn_gate_up.rows_per_projection[1];
        if gate_rows != up_rows {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                    gate_rows, up_rows
                )),
            ));
        }
        if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                self.ffn_down.host.columns,
            )?;
            submission.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                self.ffn_down.host.columns,
                self.ffn_down.host.rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(self.ffn_down.host.mode) {
            submission.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.activated_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        } else {
            submission.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            submission.quantized_matvec(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.gated_delta_buffer,
                &plan.projected_buffer,
            )?;
        }
        submission.add_f32_in_place(
            &plan.current_hidden_buffer,
            0,
            &plan.projected_buffer,
            hidden_size,
        )?;
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        *kernel_count = kernel_count.saturating_add(report.encoded_operations);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn forward_hybrid_device_debug_compare(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        model: &CudaQwen35Model,
        layer_index: usize,
        hybrid: &Qwen35HybridLayer,
        state: &mut Qwen35HybridState,
        position: usize,
        initial_token: Option<TokenId>,
        kernel_count: &mut usize,
        bytes_moved: &mut u64,
    ) -> Result<(), ReferenceTextGenerationError> {
        let hidden_size = model.descriptor.config.hidden_size;
        let epsilon = model.family_metadata.rms_norm_epsilon;
        let qkv_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[0];
        let z_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[1];
        let alpha_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[2];
        let beta_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[3];
        if alpha_rows != beta_rows
            || alpha_rows != hybrid.ssm_a.len()
            || alpha_rows != hybrid.ssm_dt.len()
        {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 hybrid cuda path requires aligned alpha/beta widths, got alpha={} beta={} ssm_a={} ssm_dt={}",
                    alpha_rows,
                    beta_rows,
                    hybrid.ssm_a.len(),
                    hybrid.ssm_dt.len()
                )),
            ));
        }
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);
        let z_offset = qkv_rows;
        let alpha_offset = z_offset.saturating_add(z_rows);
        let beta_offset = alpha_offset.saturating_add(alpha_rows);

        let input_hidden = if let Some(token) = initial_token {
            model.token_embedding.decode_row(token.as_u32() as usize)?
        } else {
            plan.current_hidden_buffer
                .read_f32_at_offset(0, hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?
        };
        let conv_state_before = state
            .conv_state
            .read_f32()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let delta_state_before = state
            .delta_state
            .read_f32()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let host = self.compute_qwen35_hybrid_host_debug(
            hybrid,
            input_hidden.as_slice(),
            conv_state_before.as_slice(),
            delta_state_before.as_slice(),
            epsilon,
        )?;

        let mut attention = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        if let Some(token) = initial_token {
            *bytes_moved = bytes_moved.saturating_add(model.encode_token_embedding_lookup(
                &mut attention,
                plan,
                token,
                position,
            )?);
        }
        if let Some(transposed_f16) = hybrid.qkv_gate_alpha_beta.transposed_f16.as_ref() {
            attention.rms_norm(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.hidden_norm_buffer,
                hidden_size,
                epsilon,
            )?;
            attention.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                hybrid.qkv_gate_alpha_beta.columns,
            )?;
            attention.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                hybrid.qkv_gate_alpha_beta.columns,
                hybrid.qkv_gate_alpha_beta.total_rows(),
            )?;
        } else {
            attention.rms_norm_q8_1(
                &plan.current_hidden_buffer,
                &self.attention_norm_device,
                &plan.matvec_input_q8_1_buffer,
                hidden_size,
                epsilon,
            )?;
            attention.quantized_matvec_q8_1(
                &hybrid.qkv_gate_alpha_beta.storage,
                0,
                hybrid.qkv_gate_alpha_beta.mode,
                hybrid.qkv_gate_alpha_beta.total_rows(),
                hybrid.qkv_gate_alpha_beta.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        }
        attention.depthwise_causal_conv1d_step_silu_f32(
            &plan.matvec_output_buffer,
            &state.conv_state,
            &hybrid.ssm_conv1d_device,
            qkv_rows,
            hybrid.conv_kernel,
            &plan.conv_buffer,
        )?;
        attention.qwen35_ssm_decay_beta_f32(
            &plan.matvec_output_buffer,
            alpha_offset,
            beta_offset,
            &hybrid.ssm_a_device,
            &hybrid.ssm_dt_device,
            alpha_rows,
            &plan.decay_buffer,
            &plan.beta_buffer,
        )?;
        encode_qwen35_hybrid_qkv_post_conv(
            &mut attention,
            &plan.conv_buffer,
            hybrid.group_count,
            hybrid.state_size,
            v_offset,
            v_size,
            &hybrid.q_scale_device,
            &hybrid.k_scale_device,
            &plan.q_buffer,
            &plan.k_buffer,
            &plan.qkv_norm_buffer,
        )?;
        attention.gated_delta_step_f32(
            &plan.qkv_norm_buffer,
            0,
            q_size,
            v_offset,
            &plan.decay_buffer,
            &plan.beta_buffer,
            &state.delta_state,
            hybrid.group_count,
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.state_size,
            hybrid.v_head_reordered,
            &plan.gated_delta_buffer,
        )?;
        attention.rms_norm_region(
            &plan.gated_delta_buffer,
            0,
            &hybrid.ssm_norm_device,
            &plan.hybrid_norm_buffer,
            0,
            v_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = hybrid.ssm_out.transposed_f16.as_ref() {
            attention.silu_mul_f32(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.gated_delta_buffer,
            )?;
            attention.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                hybrid.ssm_out.host.columns,
            )?;
            attention.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                hybrid.ssm_out.host.columns,
                hybrid.ssm_out.host.rows,
            )?;
        } else {
            attention.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                z_offset,
                &plan.hybrid_norm_buffer,
                0,
                v_size,
                &plan.activated_q8_1_buffer,
            )?;
            attention.quantized_matvec_q8_1(
                &hybrid.ssm_out.storage,
                0,
                hybrid.ssm_out.host.mode,
                hybrid.ssm_out.host.rows,
                hybrid.ssm_out.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        }
        let attention_report = attention
            .commit(psionic_backend_cuda::CudaCommandWait::Completed)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        *kernel_count = kernel_count.saturating_add(attention_report.encoded_operations);

        let device_conv = plan
            .conv_buffer
            .read_f32_at_offset(0, qkv_rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_decay = plan
            .decay_buffer
            .read_f32_at_offset(0, alpha_rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_beta = plan
            .beta_buffer
            .read_f32_at_offset(0, alpha_rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_qkv_norm = plan
            .qkv_norm_buffer
            .read_f32_at_offset(0, v_offset.saturating_add(v_size))
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_gated_delta = plan
            .gated_delta_buffer
            .read_f32_at_offset(0, v_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_hybrid_norm = plan
            .hybrid_norm_buffer
            .read_f32_at_offset(0, v_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_projected = plan
            .projected_buffer
            .read_f32_at_offset(0, hidden_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_conv_state = state
            .conv_state
            .read_f32()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let device_delta_state = state
            .delta_state
            .read_f32()
            .map_err(ReferenceTextGenerationError::Runtime)?;

        let mut tail = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        tail.add_residual_rms_norm_q8_1(
            &plan.projected_buffer,
            &plan.current_hidden_buffer,
            None,
            &self.post_attention_norm_device,
            &plan.current_hidden_buffer,
            &plan.hidden_norm_buffer,
            &plan.matvec_input_q8_1_buffer,
            hidden_size,
            epsilon,
        )?;
        if let Some(transposed_f16) = self.ffn_gate_up.transposed_f16.as_ref() {
            tail.cast_f32_to_f16(
                &plan.hidden_norm_buffer,
                &plan.vector_f16_buffer,
                self.ffn_gate_up.columns,
            )?;
            tail.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.matvec_output_buffer,
                1,
                self.ffn_gate_up.columns,
                self.ffn_gate_up.total_rows(),
            )?;
        } else {
            tail.quantized_matvec_q8_1(
                &self.ffn_gate_up.storage,
                0,
                self.ffn_gate_up.mode,
                self.ffn_gate_up.total_rows(),
                self.ffn_gate_up.columns,
                &plan.matvec_input_q8_1_buffer,
                None,
                &plan.matvec_output_buffer,
            )?;
        }
        let gate_rows = self.ffn_gate_up.rows_per_projection[0];
        let up_rows = self.ffn_gate_up.rows_per_projection[1];
        if gate_rows != up_rows {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "qwen35 dense ffn gate/up width mismatch: gate={} up={}",
                    gate_rows, up_rows
                )),
            ));
        }
        if let Some(transposed_f16) = self.ffn_down.transposed_f16.as_ref() {
            tail.silu_mul_f32(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.gated_delta_buffer,
            )?;
            tail.cast_f32_to_f16(
                &plan.gated_delta_buffer,
                &plan.vector_f16_buffer,
                self.ffn_down.host.columns,
            )?;
            tail.matmul_f16_to_f32(
                &plan.vector_f16_buffer,
                transposed_f16,
                &plan.projected_buffer,
                1,
                self.ffn_down.host.columns,
                self.ffn_down.host.rows,
            )?;
        } else {
            tail.silu_mul_q8_1(
                &plan.matvec_output_buffer,
                0,
                &plan.matvec_output_buffer,
                gate_rows,
                gate_rows,
                &plan.activated_q8_1_buffer,
            )?;
            tail.quantized_matvec_q8_1(
                &self.ffn_down.storage,
                0,
                self.ffn_down.host.mode,
                self.ffn_down.host.rows,
                self.ffn_down.host.columns,
                &plan.activated_q8_1_buffer,
                None,
                &plan.projected_buffer,
            )?;
        }
        tail.add_f32_in_place(
            &plan.current_hidden_buffer,
            0,
            &plan.projected_buffer,
            hidden_size,
        )?;
        let tail_report = tail
            .commit(psionic_backend_cuda::CudaCommandWait::Completed)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        *kernel_count = kernel_count.saturating_add(tail_report.encoded_operations);

        let device_final_hidden = plan
            .current_hidden_buffer
            .read_f32_at_offset(0, hidden_size)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        eprintln!(
            "qwen35_hybrid_compare position={} layer={} conv_diff={:.6} decay_diff={:.6} beta_diff={:.6} qkv_norm_diff={:.6} gated_diff={:.6} hybrid_norm_diff={:.6} projected_diff={:.6} conv_state_diff={:.6} delta_state_diff={:.6} final_hidden_diff={:.6}",
            position,
            layer_index,
            max_abs_diff(device_conv.as_slice(), host.conv.as_slice())?,
            max_abs_diff(device_decay.as_slice(), host.decay.as_slice())?,
            max_abs_diff(device_beta.as_slice(), host.beta.as_slice())?,
            max_abs_diff(device_qkv_norm.as_slice(), host.qkv_norm.as_slice())?,
            max_abs_diff(device_gated_delta.as_slice(), host.gated_delta.as_slice())?,
            max_abs_diff(device_hybrid_norm.as_slice(), host.hybrid_norm.as_slice())?,
            max_abs_diff(device_projected.as_slice(), host.projected.as_slice())?,
            max_abs_diff(
                device_conv_state.as_slice(),
                host.next_conv_state.as_slice()
            )?,
            max_abs_diff(
                device_delta_state.as_slice(),
                host.next_delta_state.as_slice()
            )?,
            max_abs_diff(device_final_hidden.as_slice(), host.final_hidden.as_slice())?,
        );
        Ok(())
    }

    fn compute_qwen35_hybrid_host_debug(
        &self,
        hybrid: &Qwen35HybridLayer,
        input_hidden: &[f32],
        conv_state: &[f32],
        delta_state: &[f32],
        epsilon: f32,
    ) -> Result<Qwen35HybridHostDebug, ReferenceTextGenerationError> {
        let hidden_norm = rms_norm(input_hidden, self.attention_norm.as_slice(), epsilon);
        let projected = hybrid
            .qkv_gate_alpha_beta
            .host_matvec(hidden_norm.as_slice())?;
        let qkv = projected
            .slice(0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let z = projected
            .slice(1)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let alpha = projected
            .slice(2)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let beta = projected
            .slice(3)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
        let k_size = q_size;
        let v_size = hybrid.inner_size;
        let v_offset = q_size.saturating_add(k_size);

        let mut next_conv_state = conv_state.to_vec();
        let mut conv = vec![0.0_f32; qkv.len()];
        causal_depthwise_conv1d_step_in_place(
            qkv,
            next_conv_state.as_mut_slice(),
            &hybrid.ssm_conv1d,
            hybrid.conv_kernel,
            conv.as_mut_slice(),
        )?;
        silu_forward_in_place(conv.as_mut_slice());

        let mut gate_preexp = vec![0.0_f32; alpha.len()];
        let mut decay = vec![0.0_f32; alpha.len()];
        let mut beta_sigmoid = vec![0.0_f32; beta.len()];
        for index in 0..alpha.len() {
            let gate = softplus(alpha[index] + hybrid.ssm_dt[index]) * hybrid.ssm_a[index];
            gate_preexp[index] = gate;
            decay[index] = gate.exp();
            beta_sigmoid[index] = sigmoid(beta[index]);
        }

        let mut qkv_norm = vec![0.0_f32; v_offset.saturating_add(v_size)];
        qkv_norm[..q_size].copy_from_slice(&conv[..q_size]);
        qkv_norm[q_size..q_size + k_size].copy_from_slice(&conv[q_size..q_size + k_size]);
        qkv_norm[v_offset..v_offset + v_size].copy_from_slice(&conv[v_offset..v_offset + v_size]);

        let mut next_delta_state = delta_state.to_vec();
        let mut gated_delta = vec![0.0_f32; v_size];
        let mut norm_q = vec![0.0_f32; hybrid.state_size];
        let mut norm_k = vec![0.0_f32; hybrid.state_size];
        let mut kv_mem = vec![0.0_f32; hybrid.state_size];
        let mut delta = vec![0.0_f32; hybrid.state_size];
        let repeat_factor = hybrid.time_step_rank / hybrid.group_count.max(1);
        for value_head_index in 0..hybrid.time_step_rank {
            let key_head_index = if hybrid.v_head_reordered {
                value_head_index % hybrid.group_count.max(1)
            } else if repeat_factor > 0 {
                value_head_index / repeat_factor
            } else {
                0
            };
            let q = &qkv_norm
                [key_head_index * hybrid.state_size..(key_head_index + 1) * hybrid.state_size];
            let k = &qkv_norm[q_size + key_head_index * hybrid.state_size
                ..q_size + (key_head_index + 1) * hybrid.state_size];
            let v = &qkv_norm[v_offset + value_head_index * hybrid.state_size
                ..v_offset + (value_head_index + 1) * hybrid.state_size];
            let state_slice = &mut next_delta_state[value_head_index
                .saturating_mul(hybrid.state_size)
                .saturating_mul(hybrid.state_size)
                ..(value_head_index + 1)
                    .saturating_mul(hybrid.state_size)
                    .saturating_mul(hybrid.state_size)];
            let output_slice = &mut gated_delta
                [value_head_index * hybrid.state_size..(value_head_index + 1) * hybrid.state_size];
            delta_net_autoregressive_step_in_place(
                q,
                k,
                v,
                decay[value_head_index],
                beta_sigmoid[value_head_index],
                state_slice,
                norm_q.as_mut_slice(),
                norm_k.as_mut_slice(),
                kv_mem.as_mut_slice(),
                delta.as_mut_slice(),
                output_slice,
            );
        }

        let hybrid_norm = per_head_rms_norm(
            gated_delta.as_slice(),
            hybrid.time_step_rank,
            hybrid.state_size,
            hybrid.ssm_norm.as_slice(),
            epsilon,
        );
        let activated = hybrid_norm
            .iter()
            .copied()
            .zip(z.iter().copied())
            .map(|(value, gate)| value * silu_scalar(gate))
            .collect::<Vec<_>>();
        let projected = hybrid
            .ssm_out
            .host_matvec(activated.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention = add_vectors(projected.as_slice(), input_hidden)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let post_attention_norm = rms_norm(
            post_attention.as_slice(),
            self.post_attention_norm.as_slice(),
            epsilon,
        );
        let gate_up = self
            .ffn_gate_up
            .host_matvec(post_attention_norm.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let ffn = silu_glu(
            gate_up
                .slice(0)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_up
                .slice(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
        );
        let ffn_down = self
            .ffn_down
            .host_matvec(ffn.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let final_hidden = add_vectors(post_attention.as_slice(), ffn_down.as_slice())
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok(Qwen35HybridHostDebug {
            conv,
            decay,
            beta: beta_sigmoid,
            qkv_norm,
            gated_delta,
            hybrid_norm,
            projected,
            next_conv_state,
            next_delta_state,
            final_hidden,
        })
    }
}

#[derive(Clone, Debug)]
enum Qwen35LayerKind {
    Hybrid(Qwen35HybridLayer),
    FullAttention(Qwen35FullAttentionLayer),
}

#[derive(Clone, Debug)]
struct Qwen35HybridLayer {
    qkv_gate_alpha_beta: CudaQuantizedProjectionGroup,
    ssm_conv1d: DenseMatrix,
    ssm_conv1d_device: CudaBuffer,
    ssm_a: Vec<f32>,
    ssm_a_device: CudaBuffer,
    ssm_dt: Vec<f32>,
    ssm_dt_device: CudaBuffer,
    ssm_norm: Vec<f32>,
    ssm_norm_device: CudaBuffer,
    q_scale_device: CudaBuffer,
    k_scale_device: CudaBuffer,
    ssm_out: CudaQuantizedMatrix,
    inner_size: usize,
    state_size: usize,
    group_count: usize,
    time_step_rank: usize,
    conv_kernel: usize,
    v_head_reordered: bool,
}

impl Qwen35HybridLayer {
    fn load(
        backend: &mut CudaBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let ssm_conv1d = load_dense_matrix(
            artifact,
            required_tensor_name(layout.ssm_conv1d_weight.as_deref(), "ssm_conv1d")?,
        )?;
        let ssm_a = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_a.as_deref(), "ssm_a")?,
        )?;
        let ssm_dt = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_dt.as_deref(), "ssm_dt")?,
        )?;
        let ssm_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.ssm_norm_weight.as_deref(), "ssm_norm")?,
        )?;
        let state_size = family_fact_usize(metadata, "qwen35.ssm.state_size")?;
        // Ollama applies L2 normalization to q/k, then scales q by
        // `1 / sqrt(head_v_dim)`. Our CUDA kernels use RMS-style normalization
        // with constant weights, so the weights must absorb the
        // `sqrt(state_size)` conversion from RMS to L2 space:
        // q = RMSNorm(x) * (1 / state_size), k = RMSNorm(x) * (1 / sqrt(state_size)).
        let q_scale = 1.0_f32 / state_size as f32;
        let k_scale = 1.0_f32 / (state_size as f32).sqrt();
        Ok(Self {
            qkv_gate_alpha_beta: load_cuda_quantized_projection_group(
                backend,
                artifact,
                &[
                    required_tensor_name(layout.attention_qkv_weight.as_deref(), "attn_qkv")?,
                    required_tensor_name(layout.attention_gate_weight.as_deref(), "attn_gate")?,
                    required_tensor_name(layout.ssm_alpha_weight.as_deref(), "ssm_alpha")?,
                    required_tensor_name(layout.ssm_beta_weight.as_deref(), "ssm_beta")?,
                ],
                false,
            )?,
            ssm_conv1d_device: upload_f32_buffer(
                backend,
                ssm_conv1d.values.as_slice(),
                "qwen35_ssm_conv1d",
            )?,
            ssm_conv1d,
            ssm_a_device: upload_f32_buffer(backend, ssm_a.as_slice(), "qwen35_ssm_a")?,
            ssm_a,
            ssm_dt_device: upload_f32_buffer(backend, ssm_dt.as_slice(), "qwen35_ssm_dt")?,
            ssm_dt,
            ssm_norm_device: upload_f32_buffer(backend, ssm_norm.as_slice(), "qwen35_ssm_norm")?,
            q_scale_device: upload_f32_buffer(
                backend,
                &vec![q_scale; state_size],
                "qwen35_ssm_q_scale",
            )?,
            k_scale_device: upload_f32_buffer(
                backend,
                &vec![k_scale; state_size],
                "qwen35_ssm_k_scale",
            )?,
            ssm_norm,
            ssm_out: load_cuda_quantized_matrix(
                backend,
                artifact,
                required_tensor_name(layout.ssm_out_weight.as_deref(), "ssm_out")?,
            )?,
            inner_size: family_fact_usize(metadata, "qwen35.ssm.inner_size")?,
            state_size,
            group_count: family_fact_usize(metadata, "qwen35.ssm.group_count")?,
            time_step_rank: family_fact_usize(metadata, "qwen35.ssm.time_step_rank")?,
            conv_kernel: family_fact_usize(metadata, "qwen35.ssm.conv_kernel")?,
            v_head_reordered: family_fact_bool_or_default(
                metadata,
                "qwen35.ssm.v_head_reordered",
                true,
            ),
        })
    }

    fn initial_state(
        &self,
        backend: &mut CudaBackend,
    ) -> Result<Qwen35HybridState, ReferenceTextGenerationError> {
        let conv_state = backend
            .f32_buffer(
                self.qkv_gate_alpha_beta.rows_per_projection[0]
                    .saturating_mul(self.conv_kernel.saturating_sub(1)),
            )
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let delta_state = backend
            .f32_buffer(
                self.time_step_rank
                    .saturating_mul(self.state_size)
                    .saturating_mul(self.state_size),
            )
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let mut submission = backend
            .begin_submission()
            .map_err(ReferenceTextGenerationError::Runtime)?;
        submission
            .fill_buffer(&conv_state, 0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        submission
            .fill_buffer(&delta_state, 0)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        submission
            .commit(psionic_backend_cuda::CudaCommandWait::Completed)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        Ok(Qwen35HybridState {
            conv_state,
            delta_state,
        })
    }

    fn device_residency_bytes(&self) -> usize {
        let aux_bytes = self
            .ssm_conv1d
            .host_residency_bytes()
            .saturating_add(vec_f32_bytes(self.ssm_a.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_dt.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_norm.as_slice()))
            .saturating_add(self.state_size.saturating_mul(std::mem::size_of::<f32>()))
            .saturating_add(self.state_size.saturating_mul(std::mem::size_of::<f32>()));
        self.qkv_gate_alpha_beta
            .device_residency_bytes()
            .saturating_add(self.ssm_out.device_residency_bytes())
            .saturating_add(aux_bytes)
    }

    fn host_residency_bytes(&self) -> usize {
        self.ssm_conv1d
            .host_residency_bytes()
            .saturating_add(vec_f32_bytes(self.ssm_a.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_dt.as_slice()))
            .saturating_add(vec_f32_bytes(self.ssm_norm.as_slice()))
    }

    fn max_matvec_input_columns(&self) -> usize {
        usize::max(self.qkv_gate_alpha_beta.columns, self.ssm_out.host.columns)
    }

    fn max_matvec_output_rows(&self) -> usize {
        usize::max(
            self.qkv_gate_alpha_beta.total_rows(),
            self.ssm_out.host.rows,
        )
    }

    fn forward(
        &self,
        _backend: &mut CudaBackend,
        _plan: &mut Qwen35CudaStepPlan,
        _model: &CudaQwen35Model,
        _position: usize,
        _hidden: &[f32],
        _state: &mut Qwen35HybridState,
        _kernel_count: &mut usize,
        _bytes_moved: &mut u64,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(String::from(
                "qwen35 hybrid host path is disabled; use forward_hybrid_device",
            )),
        ))
    }
}

#[derive(Clone, Debug)]
struct Qwen35HybridState {
    conv_state: CudaBuffer,
    delta_state: CudaBuffer,
}

#[derive(Clone, Debug)]
struct Qwen35FullAttentionLayer {
    qkv: CudaQuantizedProjectionGroup,
    native_qkv: Option<CudaQwen35FullAttentionQkv>,
    query_norm: Vec<f32>,
    query_norm_device: CudaBuffer,
    key_norm: Vec<f32>,
    key_norm_device: CudaBuffer,
    output: CudaQuantizedMatrix,
    kv_width: usize,
}

#[derive(Clone, Debug)]
struct CudaQwen35FullAttentionQkv {
    query_gate: CudaQuantizedMatrix,
    key: CudaQuantizedMatrix,
    value: CudaQuantizedMatrix,
}

impl Qwen35FullAttentionLayer {
    fn load(
        backend: &mut CudaBackend,
        artifact: &GgufBlobArtifact,
        layout: &psionic_models::GgufDecoderLayerTensorLayout,
        _metadata: &GgufDecoderFamilyMetadata,
    ) -> Result<Self, ModelLoadError> {
        let query_name = required_tensor_name(layout.attention_query_weight.as_deref(), "attn_q")?;
        let key_name = required_tensor_name(layout.attention_key_weight.as_deref(), "attn_k")?;
        let value_name = required_tensor_name(layout.attention_value_weight.as_deref(), "attn_v")?;
        let qkv = load_cuda_quantized_projection_group(
            backend,
            artifact,
            &[query_name, key_name, value_name],
            true,
        )?;
        let native_qkv = if qkv.native_parts.len() == 3 {
            Some(CudaQwen35FullAttentionQkv {
                query_gate: qkv.native_parts[0].clone(),
                key: qkv.native_parts[1].clone(),
                value: qkv.native_parts[2].clone(),
            })
        } else {
            None
        };
        let query_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_query_norm.as_deref(), "attn_q_norm")?,
        )?;
        let key_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.attention_key_norm.as_deref(), "attn_k_norm")?,
        )?;
        Ok(Self {
            kv_width: qkv.rows_per_projection[1],
            qkv,
            native_qkv,
            query_norm_device: upload_f32_buffer(
                backend,
                query_norm.as_slice(),
                "qwen35_attention_query_norm",
            )?,
            query_norm,
            key_norm_device: upload_f32_buffer(
                backend,
                key_norm.as_slice(),
                "qwen35_attention_key_norm",
            )?,
            key_norm,
            output: load_cuda_quantized_matrix(
                backend,
                artifact,
                required_tensor_name(layout.attention_output_weight.as_deref(), "attn_output")?,
            )?,
        })
    }

    fn initial_state(
        &self,
        backend: &mut CudaBackend,
        cache_capacity_tokens: usize,
    ) -> Result<Qwen35FullAttentionState, ReferenceTextGenerationError> {
        let cache_bytes = cache_capacity_tokens
            .saturating_mul(self.kv_width)
            .saturating_mul(std::mem::size_of::<u16>());
        Ok(Qwen35FullAttentionState {
            key_cache: backend
                .byte_buffer(&vec![0_u8; cache_bytes])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            value_cache: backend
                .byte_buffer(&vec![0_u8; cache_bytes])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            width: self.kv_width,
            element_bytes: std::mem::size_of::<u16>(),
            len: 0,
            capacity_tokens: cache_capacity_tokens,
        })
    }

    fn device_residency_bytes(&self) -> usize {
        self.qkv
            .device_residency_bytes()
            .saturating_add(vec_f32_bytes(self.query_norm.as_slice()))
            .saturating_add(vec_f32_bytes(self.key_norm.as_slice()))
            .saturating_add(self.output.device_residency_bytes())
    }

    fn host_residency_bytes(&self) -> usize {
        vec_f32_bytes(self.query_norm.as_slice())
            .saturating_add(vec_f32_bytes(self.key_norm.as_slice()))
    }

    fn max_matvec_input_columns(&self) -> usize {
        usize::max(self.qkv.columns, self.output.host.columns)
    }

    fn max_matvec_output_rows(&self) -> usize {
        usize::max(self.qkv.total_rows(), self.output.host.rows)
    }

    fn forward(
        &self,
        _backend: &mut CudaBackend,
        _plan: &mut Qwen35CudaStepPlan,
        _model: &CudaQwen35Model,
        _position: usize,
        _hidden: &[f32],
        _state: &mut Qwen35FullAttentionState,
        _kernel_count: &mut usize,
        _bytes_moved: &mut u64,
    ) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(String::from(
                "qwen35 full-attention host path is disabled; use forward_full_attention_device",
            )),
        ))
    }
}

#[derive(Clone, Debug)]
struct Qwen35FullAttentionState {
    key_cache: CudaBuffer,
    value_cache: CudaBuffer,
    width: usize,
    element_bytes: usize,
    len: usize,
    capacity_tokens: usize,
}

impl Qwen35FullAttentionState {
    fn ensure_capacity(
        &mut self,
        backend: &mut CudaBackend,
        required_tokens: usize,
    ) -> Result<(), crate::RuntimeError> {
        if required_tokens <= self.capacity_tokens {
            return Ok(());
        }
        let new_capacity = required_tokens
            .max(self.capacity_tokens.saturating_mul(2))
            .checked_next_power_of_two()
            .unwrap_or(required_tokens);
        let token_bytes = self.width.saturating_mul(self.element_bytes);
        let new_cache_bytes = new_capacity.saturating_mul(token_bytes);
        let new_key_cache = backend.byte_buffer(&vec![0_u8; new_cache_bytes])?;
        let new_value_cache = backend.byte_buffer(&vec![0_u8; new_cache_bytes])?;
        if self.len > 0 {
            let copy_bytes = self.len.saturating_mul(token_bytes);
            let mut submission = backend.begin_submission()?;
            submission.copy_buffer_region(&self.key_cache, 0, &new_key_cache, 0, copy_bytes)?;
            submission.copy_buffer_region(&self.value_cache, 0, &new_value_cache, 0, copy_bytes)?;
            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        }
        self.key_cache = new_key_cache;
        self.value_cache = new_value_cache;
        self.capacity_tokens = new_capacity;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Qwen35FullAttentionEntry {
    key: Vec<f32>,
    value: Vec<f32>,
}

#[derive(Clone, Debug)]
struct Qwen35State {
    position: usize,
    layers: Vec<Qwen35LayerState>,
}

#[derive(Clone, Debug)]
enum Qwen35LayerState {
    Hybrid(Qwen35HybridState),
    FullAttention(Qwen35FullAttentionState),
}

impl Qwen35State {
    fn deep_clone(&self, backend: &mut CudaBackend) -> Result<Self, ReferenceTextGenerationError> {
        Ok(Self {
            position: self.position,
            layers: self
                .layers
                .iter()
                .map(|layer| layer.deep_clone(backend))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl Qwen35LayerState {
    fn deep_clone(&self, backend: &mut CudaBackend) -> Result<Self, ReferenceTextGenerationError> {
        match self {
            Self::Hybrid(state) => Ok(Self::Hybrid(state.deep_clone(backend)?)),
            Self::FullAttention(state) => Ok(Self::FullAttention(state.deep_clone(backend)?)),
        }
    }
}

impl Qwen35HybridState {
    fn deep_clone(&self, backend: &mut CudaBackend) -> Result<Self, ReferenceTextGenerationError> {
        Ok(Self {
            conv_state: deep_clone_cuda_buffer(backend, &self.conv_state)?,
            delta_state: deep_clone_cuda_buffer(backend, &self.delta_state)?,
        })
    }
}

impl Qwen35FullAttentionState {
    fn deep_clone(&self, backend: &mut CudaBackend) -> Result<Self, ReferenceTextGenerationError> {
        Ok(Self {
            key_cache: deep_clone_cuda_buffer(backend, &self.key_cache)?,
            value_cache: deep_clone_cuda_buffer(backend, &self.value_cache)?,
            width: self.width,
            element_bytes: self.element_bytes,
            len: self.len,
            capacity_tokens: self.capacity_tokens,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CudaStepOutputMode {
    NoOutput,
    FullLogits,
    ArgmaxOnly,
    TopKCandidates(usize),
}

#[derive(Clone, Debug)]
struct Qwen35ForwardStep {
    logits: Vec<f32>,
    selected_token: Option<TokenId>,
    candidates: Option<Qwen35CudaTopKCandidates>,
    kernel_count: usize,
    bytes_moved: u64,
    output_metrics: Option<Qwen35CudaDecodeOutputMetrics>,
}

#[derive(Debug)]
struct Qwen35CudaStepPlan {
    matvec_input_buffer: CudaBuffer,
    matvec_input_q8_1_buffer: CudaBuffer,
    vector_f16_buffer: CudaBuffer,
    matvec_output_buffer: CudaBuffer,
    current_hidden_buffer: CudaBuffer,
    decode_params_host_buffer: CudaHostBuffer,
    decode_params_buffer: CudaBuffer,
    hidden_norm_buffer: CudaBuffer,
    gate_buffer: CudaBuffer,
    q_buffer: CudaBuffer,
    k_buffer: CudaBuffer,
    qkv_norm_buffer: CudaBuffer,
    conv_buffer: CudaBuffer,
    gated_delta_buffer: CudaBuffer,
    hybrid_norm_buffer: CudaBuffer,
    projected_buffer: CudaBuffer,
    activated_q8_1_buffer: CudaBuffer,
    decay_buffer: CudaBuffer,
    beta_buffer: CudaBuffer,
    attention_fa3_partial_output_buffer: CudaBuffer,
    attention_fa3_partial_max_buffer: CudaBuffer,
    attention_fa3_partial_sum_buffer: CudaBuffer,
    logits_buffer: CudaBuffer,
    logits_host_buffer: CudaHostBuffer,
    sparse_logits_buffer: CudaBuffer,
    sparse_logit_indices_buffer: CudaBuffer,
    top_k_indices_buffer: CudaBuffer,
    top_k_values_buffer: CudaBuffer,
    top_k_partial_indices_buffer: CudaBuffer,
    top_k_partial_values_buffer: CudaBuffer,
    penalty_token_ids_buffer: CudaBuffer,
    penalty_token_counts_buffer: CudaBuffer,
    penalty_token_ids_scratch: Vec<i32>,
    penalty_token_counts_scratch: Vec<i32>,
    sparse_logit_indices_scratch: Vec<i32>,
    next_token_host_buffer: CudaHostBuffer,
    next_token_buffer: CudaBuffer,
    argmax_state_host_buffer: CudaHostBuffer,
    argmax_state_buffer: CudaBuffer,
    top_k_indices_host_buffer: CudaHostBuffer,
    top_k_values_host_buffer: CudaHostBuffer,
    partitioned_top_k_block_override: Option<usize>,
    partitioned_top_k_threshold: usize,
    no_output_graph_exec: Option<CudaGraphExec>,
    no_output_graph_cache_identity: Option<Vec<(usize, usize)>>,
    decode_graph_exec: Option<CudaGraphExec>,
    decode_graph_cache_identity: Option<Vec<(usize, usize)>>,
    full_logits_graph_exec: Option<CudaGraphExec>,
    full_logits_graph_cache_identity: Option<Vec<(usize, usize)>>,
    top_k_graph_exec: Option<CudaGraphExec>,
    top_k_graph_cache_identity: Option<(usize, Vec<(usize, usize)>)>,
}

impl Qwen35CudaStepPlan {
    fn device_residency_bytes(&self) -> u64 {
        [
            &self.matvec_input_buffer,
            &self.matvec_input_q8_1_buffer,
            &self.vector_f16_buffer,
            &self.matvec_output_buffer,
            &self.current_hidden_buffer,
            &self.decode_params_buffer,
            &self.hidden_norm_buffer,
            &self.gate_buffer,
            &self.q_buffer,
            &self.k_buffer,
            &self.qkv_norm_buffer,
            &self.conv_buffer,
            &self.gated_delta_buffer,
            &self.hybrid_norm_buffer,
            &self.projected_buffer,
            &self.activated_q8_1_buffer,
            &self.decay_buffer,
            &self.beta_buffer,
            &self.attention_fa3_partial_output_buffer,
            &self.attention_fa3_partial_max_buffer,
            &self.attention_fa3_partial_sum_buffer,
            &self.logits_buffer,
            &self.sparse_logits_buffer,
            &self.sparse_logit_indices_buffer,
            &self.top_k_indices_buffer,
            &self.top_k_values_buffer,
            &self.top_k_partial_indices_buffer,
            &self.top_k_partial_values_buffer,
            &self.penalty_token_ids_buffer,
            &self.penalty_token_counts_buffer,
            &self.next_token_buffer,
            &self.argmax_state_buffer,
        ]
        .iter()
        .map(|buffer| u64::try_from(buffer.byte_len()).unwrap_or(u64::MAX))
        .fold(0u64, u64::saturating_add)
    }

    fn reset_graph_replay_state(&mut self) {
        self.no_output_graph_exec = None;
        self.no_output_graph_cache_identity = None;
        self.decode_graph_exec = None;
        self.decode_graph_cache_identity = None;
        self.full_logits_graph_exec = None;
        self.full_logits_graph_cache_identity = None;
        self.top_k_graph_exec = None;
        self.top_k_graph_cache_identity = None;
    }

    fn new(
        backend: &mut CudaBackend,
        hidden_size: usize,
        attention_head_count: usize,
        max_input_columns: usize,
        max_output_rows: usize,
        vocab_size: usize,
        max_penalty_token_count: usize,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let partitioned_top_k_block_override = qwen35_partitioned_top_k_block_override();
        let partitioned_top_k_threshold = qwen35_partitioned_top_k_threshold();
        let q8_1_bytes = ggml_q8_1_storage_bytes(1, max_input_columns)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let activated_q8_1_bytes = ggml_q8_1_storage_bytes(1, max_output_rows)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        let top_k_partial_len =
            QWEN35_CUDA_MAX_TOP_K.saturating_mul(qwen35_partitioned_top_k_block_count(
                QWEN35_CUDA_MAX_TOP_K,
                partitioned_top_k_block_override,
            ));
        let max_penalty_token_count = max_penalty_token_count.max(1);
        Ok(Self {
            matvec_input_buffer: backend
                .f32_buffer(max_input_columns)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            matvec_input_q8_1_buffer: backend
                .byte_buffer(&vec![0_u8; q8_1_bytes])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            vector_f16_buffer: backend
                .f16_buffer(max_input_columns)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            matvec_output_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            current_hidden_buffer: backend
                .f32_buffer(hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            decode_params_host_buffer: backend
                .host_buffer(3 * std::mem::size_of::<i32>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            decode_params_buffer: backend
                .i32_buffer(3)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            hidden_norm_buffer: backend
                .f32_buffer(hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gate_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            q_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            k_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            qkv_norm_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            conv_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            gated_delta_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            hybrid_norm_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            projected_buffer: backend
                .f32_buffer(hidden_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            activated_q8_1_buffer: backend
                .byte_buffer(&vec![0_u8; activated_q8_1_bytes])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            decay_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            beta_buffer: backend
                .f32_buffer(max_output_rows)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            attention_fa3_partial_output_buffer: backend
                .f32_buffer(max_output_rows.saturating_mul(QWEN35_CUDA_FA3_MAX_SPLITS))
                .map_err(ReferenceTextGenerationError::Runtime)?,
            attention_fa3_partial_max_buffer: backend
                .f32_buffer(attention_head_count.saturating_mul(QWEN35_CUDA_FA3_MAX_SPLITS))
                .map_err(ReferenceTextGenerationError::Runtime)?,
            attention_fa3_partial_sum_buffer: backend
                .f32_buffer(attention_head_count.saturating_mul(QWEN35_CUDA_FA3_MAX_SPLITS))
                .map_err(ReferenceTextGenerationError::Runtime)?,
            logits_buffer: backend
                .f32_buffer(vocab_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            logits_host_buffer: backend
                .host_buffer(vocab_size * std::mem::size_of::<f32>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            sparse_logits_buffer: backend
                .f32_buffer(vocab_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            sparse_logit_indices_buffer: backend
                .i32_buffer(vocab_size)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_indices_buffer: backend
                .i32_buffer(QWEN35_CUDA_MAX_TOP_K)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_values_buffer: backend
                .f32_buffer(QWEN35_CUDA_MAX_TOP_K)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_partial_indices_buffer: backend
                .i32_buffer(top_k_partial_len)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_partial_values_buffer: backend
                .f32_buffer(top_k_partial_len)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            penalty_token_ids_buffer: backend
                .i32_buffer(max_penalty_token_count)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            penalty_token_counts_buffer: backend
                .i32_buffer(max_penalty_token_count)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            penalty_token_ids_scratch: vec![0_i32; max_penalty_token_count],
            penalty_token_counts_scratch: vec![0_i32; max_penalty_token_count],
            sparse_logit_indices_scratch: vec![0_i32; vocab_size],
            next_token_host_buffer: backend
                .host_buffer(std::mem::size_of::<i32>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            next_token_buffer: backend
                .i32_buffer(1)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            argmax_state_host_buffer: backend
                .host_buffer(std::mem::size_of::<u64>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            argmax_state_buffer: backend
                .byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_indices_host_buffer: backend
                .host_buffer(QWEN35_CUDA_MAX_TOP_K * std::mem::size_of::<i32>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            top_k_values_host_buffer: backend
                .host_buffer(QWEN35_CUDA_MAX_TOP_K * std::mem::size_of::<f32>())
                .map_err(ReferenceTextGenerationError::Runtime)?,
            partitioned_top_k_block_override,
            partitioned_top_k_threshold,
            no_output_graph_exec: None,
            no_output_graph_cache_identity: None,
            decode_graph_exec: None,
            decode_graph_cache_identity: None,
            full_logits_graph_exec: None,
            full_logits_graph_cache_identity: None,
            top_k_graph_exec: None,
            top_k_graph_cache_identity: None,
        })
    }

    fn encode_top_k_from_logits(
        &self,
        submission: &mut CudaSubmission,
        logit_count: usize,
        top_k: usize,
    ) -> Result<(), crate::RuntimeError> {
        if top_k >= self.partitioned_top_k_threshold {
            let partitioned_top_k_blocks =
                qwen35_partitioned_top_k_block_count(top_k, self.partitioned_top_k_block_override);
            return submission.top_k_f32_one_row_partitioned(
                &self.logits_buffer,
                logit_count,
                top_k,
                partitioned_top_k_blocks,
                &self.top_k_partial_indices_buffer,
                &self.top_k_partial_values_buffer,
                &self.top_k_indices_buffer,
                &self.top_k_values_buffer,
            );
        }
        submission.top_k_f32(
            &self.logits_buffer,
            1,
            logit_count,
            top_k,
            &self.top_k_indices_buffer,
            &self.top_k_values_buffer,
        )
    }

    fn encode_sampling_penalties_from_history(
        &mut self,
        submission: &mut CudaSubmission,
        vocab_size: usize,
        history: &[TokenId],
        policy: &SamplingPolicy,
    ) -> Result<u64, crate::RuntimeError> {
        let repeat_penalty = policy.effective_repeat_penalty();
        let presence_penalty = policy.effective_presence_penalty();
        let frequency_penalty = policy.effective_frequency_penalty();
        if (repeat_penalty - 1.0).abs() <= f32::EPSILON
            && presence_penalty.abs() <= f32::EPSILON
            && frequency_penalty.abs() <= f32::EPSILON
        {
            return Ok(0);
        }

        let counts = qwen35_sampling_penalty_counts(history, vocab_size, policy);
        let active_token_count = counts.len();
        if active_token_count == 0 {
            return Ok(0);
        }
        if active_token_count > self.penalty_token_ids_scratch.len() {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda penalty history exceeds scratch capacity: active={} capacity={}",
                active_token_count,
                self.penalty_token_ids_scratch.len()
            )));
        }
        for (slot, (token_id, count)) in counts.into_iter().enumerate() {
            self.penalty_token_ids_scratch[slot] = i32::try_from(token_id).map_err(|_| {
                crate::RuntimeError::Backend(format!(
                    "qwen35 cuda penalty token id exceeds i32: {}",
                    token_id
                ))
            })?;
            self.penalty_token_counts_scratch[slot] = i32::try_from(count).map_err(|_| {
                crate::RuntimeError::Backend(format!(
                    "qwen35 cuda penalty token count exceeds i32: {}",
                    count
                ))
            })?;
        }
        self.penalty_token_ids_buffer
            .write_i32_at_offset(0, &self.penalty_token_ids_scratch[..active_token_count])?;
        self.penalty_token_counts_buffer
            .write_i32_at_offset(0, &self.penalty_token_counts_scratch[..active_token_count])?;
        submission.apply_sampling_penalties_f32_sparse(
            &self.logits_buffer,
            vocab_size,
            &self.penalty_token_ids_buffer,
            &self.penalty_token_counts_buffer,
            active_token_count,
            repeat_penalty,
            presence_penalty,
            frequency_penalty,
        )?;
        Ok(active_token_count
            .saturating_mul(std::mem::size_of::<i32>() * 2)
            .try_into()
            .unwrap_or(u64::MAX))
    }

    fn gather_sparse_logits_from_current_output(
        &mut self,
        backend: &mut CudaBackend,
        token_ids: &[u32],
        vocab_size: usize,
    ) -> Result<(Vec<f32>, CudaQuantizedMatvecStats), crate::RuntimeError> {
        if token_ids.is_empty() {
            return Ok((Vec::new(), zero_cuda_matvec_stats()));
        }
        if token_ids.len() > self.sparse_logit_indices_scratch.len() {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda sparse structured gather exceeds scratch capacity: requested={} capacity={}",
                token_ids.len(),
                self.sparse_logit_indices_scratch.len()
            )));
        }
        for (slot, &token_id) in token_ids.iter().enumerate() {
            if token_id as usize >= vocab_size {
                return Err(crate::RuntimeError::Backend(format!(
                    "qwen35 cuda sparse structured gather token id exceeds vocab: token_id={} vocab_size={}",
                    token_id, vocab_size
                )));
            }
            self.sparse_logit_indices_scratch[slot] = i32::try_from(token_id).map_err(|_| {
                crate::RuntimeError::Backend(format!(
                    "qwen35 cuda sparse structured gather token id exceeds i32: {}",
                    token_id
                ))
            })?;
        }
        self.sparse_logit_indices_buffer
            .write_i32_at_offset(0, &self.sparse_logit_indices_scratch[..token_ids.len()])?;
        let mut submission = backend.begin_submission()?;
        submission.gather_f32_by_indices(
            &self.logits_buffer,
            vocab_size,
            &self.sparse_logit_indices_buffer,
            token_ids.len(),
            &self.sparse_logits_buffer,
        )?;
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        let logits = self
            .sparse_logits_buffer
            .read_f32_at_offset(0, token_ids.len())?;
        Ok((
            logits,
            CudaQuantizedMatvecStats {
                host_to_device_bytes: token_ids
                    .len()
                    .saturating_mul(std::mem::size_of::<i32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                device_to_host_bytes: token_ids
                    .len()
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: report.encoded_operations,
            },
        ))
    }

    fn run_projection_matvec(
        &mut self,
        backend: &mut CudaBackend,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<CudaQuantizedMatvecStats, crate::RuntimeError> {
        if input.len() != cols {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda matvec input width mismatch: expected {cols}, actual {}",
                input.len()
            )));
        }
        self.matvec_input_buffer.write_f32_at_offset(0, input)?;
        let mut submission = backend.begin_submission()?;
        if can_use_q8_1_quantized_matvec(mode) {
            submission.quantize_f32_to_q8_1(
                &self.matvec_input_buffer,
                1,
                cols,
                &self.matvec_input_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                byte_offset,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.matvec_output_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                weights,
                byte_offset,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.matvec_output_buffer,
            )?;
        }
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        *output = self.matvec_output_buffer.read_f32_at_offset(0, rows)?;
        Ok(CudaQuantizedMatvecStats {
            host_to_device_bytes: input
                .len()
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX),
            device_to_host_bytes: rows
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX),
            submission_count: 1,
            sync_count: 1,
            kernel_launches: report.encoded_operations,
        })
    }

    fn run_output_logits(
        &mut self,
        backend: &mut CudaBackend,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<(Vec<f32>, CudaQuantizedMatvecStats), crate::RuntimeError> {
        if input.len() != cols {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda logits input width mismatch: expected {cols}, actual {}",
                input.len()
            )));
        }
        self.matvec_input_buffer.write_f32_at_offset(0, input)?;
        let mut submission = backend.begin_submission()?;
        if can_use_q8_1_quantized_matvec(mode) {
            submission.quantize_f32_to_q8_1(
                &self.matvec_input_buffer,
                1,
                cols,
                &self.matvec_input_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.logits_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.logits_buffer,
            )?;
        }
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        let logits = self.logits_buffer.read_f32_at_offset(0, rows)?;
        Ok((
            logits,
            CudaQuantizedMatvecStats {
                host_to_device_bytes: input
                    .len()
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                device_to_host_bytes: rows
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: report.encoded_operations,
            },
        ))
    }

    fn run_output_logits_from_device(
        &mut self,
        backend: &mut CudaBackend,
        input: &CudaBuffer,
        norm_weight: &CudaBuffer,
        epsilon: f32,
        transposed_f16: Option<&CudaBuffer>,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
    ) -> Result<(Vec<f32>, CudaQuantizedMatvecStats), crate::RuntimeError> {
        let mut submission = backend.begin_submission()?;
        if let Some(transposed_f16) = transposed_f16 {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.cast_f32_to_f16(&self.matvec_input_buffer, &self.vector_f16_buffer, cols)?;
            submission.matmul_f16_to_f32(
                &self.vector_f16_buffer,
                transposed_f16,
                &self.logits_buffer,
                1,
                cols,
                rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(mode) {
            submission.rms_norm_q8_1(
                input,
                norm_weight,
                &self.matvec_input_q8_1_buffer,
                cols,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.logits_buffer,
            )?;
        } else {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.quantized_matvec(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.logits_buffer,
            )?;
        }
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        let logits = self.logits_buffer.read_f32_at_offset(0, rows)?;
        Ok((
            logits,
            CudaQuantizedMatvecStats {
                host_to_device_bytes: 0,
                device_to_host_bytes: rows
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: report.encoded_operations,
            },
        ))
    }

    fn run_output_top_k_from_device(
        &mut self,
        backend: &mut CudaBackend,
        input: &CudaBuffer,
        norm_weight: &CudaBuffer,
        epsilon: f32,
        transposed_f16: Option<&CudaBuffer>,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        top_k: usize,
        history: &[TokenId],
        policy: &SamplingPolicy,
    ) -> Result<(Qwen35CudaTopKCandidates, CudaQuantizedMatvecStats), crate::RuntimeError> {
        if top_k == 0 || top_k > QWEN35_CUDA_MAX_TOP_K {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda top-k width must be in 1..={}, actual {}",
                QWEN35_CUDA_MAX_TOP_K, top_k
            )));
        }
        let mut submission = backend.begin_submission()?;
        if let Some(transposed_f16) = transposed_f16 {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.cast_f32_to_f16(&self.matvec_input_buffer, &self.vector_f16_buffer, cols)?;
            submission.matmul_f16_to_f32(
                &self.vector_f16_buffer,
                transposed_f16,
                &self.logits_buffer,
                1,
                cols,
                rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(mode) {
            submission.rms_norm_q8_1(
                input,
                norm_weight,
                &self.matvec_input_q8_1_buffer,
                cols,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.logits_buffer,
            )?;
        } else {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.quantized_matvec(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.logits_buffer,
            )?;
        }
        let host_to_device_bytes =
            self.encode_sampling_penalties_from_history(&mut submission, rows, history, policy)?;
        self.encode_top_k_from_logits(&mut submission, rows, top_k)?;
        submission
            .copy_device_to_host(&self.top_k_indices_buffer, &self.top_k_indices_host_buffer)?;
        submission
            .copy_device_to_host(&self.top_k_values_buffer, &self.top_k_values_host_buffer)?;
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        let result = cuda_top_k_candidates_from_host_buffers(
            &self.top_k_indices_host_buffer,
            &self.top_k_values_host_buffer,
            top_k,
        )?;
        Ok((
            result,
            CudaQuantizedMatvecStats {
                host_to_device_bytes,
                device_to_host_bytes: top_k
                    .saturating_mul(std::mem::size_of::<u32>() + std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: report.encoded_operations,
            },
        ))
    }

    fn run_output_top_k_indices_from_device(
        &mut self,
        backend: &mut CudaBackend,
        input: &CudaBuffer,
        norm_weight: &CudaBuffer,
        epsilon: f32,
        transposed_f16: Option<&CudaBuffer>,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        top_k: usize,
        history: &[TokenId],
        policy: &SamplingPolicy,
    ) -> Result<(Vec<u32>, CudaQuantizedMatvecStats), crate::RuntimeError> {
        if top_k == 0 || top_k > QWEN35_CUDA_MAX_TOP_K {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda top-k width must be in 1..={}, actual {}",
                QWEN35_CUDA_MAX_TOP_K, top_k
            )));
        }
        let mut submission = backend.begin_submission()?;
        if let Some(transposed_f16) = transposed_f16 {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.cast_f32_to_f16(&self.matvec_input_buffer, &self.vector_f16_buffer, cols)?;
            submission.matmul_f16_to_f32(
                &self.vector_f16_buffer,
                transposed_f16,
                &self.logits_buffer,
                1,
                cols,
                rows,
            )?;
        } else if can_use_q8_1_quantized_matvec(mode) {
            submission.rms_norm_q8_1(
                input,
                norm_weight,
                &self.matvec_input_q8_1_buffer,
                cols,
                epsilon,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.logits_buffer,
            )?;
        } else {
            submission.rms_norm(input, norm_weight, &self.matvec_input_buffer, cols, epsilon)?;
            submission.quantized_matvec(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.logits_buffer,
            )?;
        }
        let host_to_device_bytes =
            self.encode_sampling_penalties_from_history(&mut submission, rows, history, policy)?;
        self.encode_top_k_from_logits(&mut submission, rows, top_k)?;
        submission
            .copy_device_to_host(&self.top_k_indices_buffer, &self.top_k_indices_host_buffer)?;
        let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
        let indices =
            cuda_top_k_candidates_from_index_host_buffer(&self.top_k_indices_host_buffer, top_k)?
                .indices()
                .to_vec();
        Ok((
            indices,
            CudaQuantizedMatvecStats {
                host_to_device_bytes,
                device_to_host_bytes: top_k
                    .saturating_mul(std::mem::size_of::<u32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: report.encoded_operations,
            },
        ))
    }

    fn run_output_argmax(
        &mut self,
        backend: &mut CudaBackend,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<(TokenId, CudaQuantizedMatvecStats), crate::RuntimeError> {
        if input.len() != cols {
            return Err(crate::RuntimeError::Backend(format!(
                "qwen35 cuda argmax input width mismatch: expected {cols}, actual {}",
                input.len()
            )));
        }
        self.matvec_input_buffer.write_f32_at_offset(0, input)?;
        let mut submission = backend.begin_submission()?;
        let (selected, device_to_host_bytes, kernel_launches) = if can_use_q8_1_argmax(mode) {
            self.argmax_state_host_buffer
                .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())?;
            submission.quantize_f32_to_q8_1(
                &self.matvec_input_buffer,
                1,
                cols,
                &self.matvec_input_q8_1_buffer,
            )?;
            submission
                .copy_host_to_device(&self.argmax_state_host_buffer, &self.argmax_state_buffer)?;
            submission.quantized_matvec_q8_1_argmax(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.argmax_state_buffer,
            )?;
            submission
                .copy_device_to_host(&self.argmax_state_buffer, &self.argmax_state_host_buffer)?;
            let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            (
                cuda_argmax_token_from_packed_host_buffer(&self.argmax_state_host_buffer).map_err(
                    |error| match error {
                        ReferenceTextGenerationError::Runtime(runtime) => runtime,
                        other => crate::RuntimeError::Backend(other.to_string()),
                    },
                )?,
                std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX),
                report.encoded_operations,
            )
        } else if can_use_q8_1_quantized_matvec(mode) {
            submission.quantize_f32_to_q8_1(
                &self.matvec_input_buffer,
                1,
                cols,
                &self.matvec_input_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_q8_1_buffer,
                None,
                &self.logits_buffer,
            )?;
            submission.argmax_f32(&self.logits_buffer, 1, rows, &self.next_token_buffer)?;
            submission
                .copy_device_to_host(&self.next_token_buffer, &self.next_token_host_buffer)?;
            let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            (
                cuda_argmax_token_id(self.next_token_host_buffer.read_i32()?).map_err(|error| {
                    match error {
                        ReferenceTextGenerationError::Runtime(runtime) => runtime,
                        other => crate::RuntimeError::Backend(other.to_string()),
                    }
                })?,
                std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                report.encoded_operations,
            )
        } else {
            submission.quantized_matvec(
                weights,
                0,
                mode,
                rows,
                cols,
                &self.matvec_input_buffer,
                &self.logits_buffer,
            )?;
            submission.argmax_f32(&self.logits_buffer, 1, rows, &self.next_token_buffer)?;
            submission
                .copy_device_to_host(&self.next_token_buffer, &self.next_token_host_buffer)?;
            let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
            (
                cuda_argmax_token_id(self.next_token_host_buffer.read_i32()?).map_err(|error| {
                    match error {
                        ReferenceTextGenerationError::Runtime(runtime) => runtime,
                        other => crate::RuntimeError::Backend(other.to_string()),
                    }
                })?,
                std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                report.encoded_operations,
            )
        };
        Ok((
            selected,
            CudaQuantizedMatvecStats {
                host_to_device_bytes: input
                    .len()
                    .saturating_mul(std::mem::size_of::<f32>())
                    .saturating_add(std::mem::size_of::<u64>())
                    .try_into()
                    .unwrap_or(u64::MAX),
                device_to_host_bytes,
                submission_count: 1,
                sync_count: 1,
                kernel_launches,
            },
        ))
    }

    fn run_output_argmax_from_device(
        &mut self,
        backend: &mut CudaBackend,
        input: &CudaBuffer,
        norm_weight: &CudaBuffer,
        epsilon: f32,
        transposed_f16: Option<&CudaBuffer>,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        materialize_logits: bool,
    ) -> Result<(TokenId, CudaQuantizedMatvecStats), crate::RuntimeError> {
        let mut submission = backend.begin_submission()?;
        let (selected, host_to_device_bytes, device_to_host_bytes, kernel_launches) =
            if let Some(transposed_f16) = transposed_f16 {
                submission.rms_norm(
                    input,
                    norm_weight,
                    &self.matvec_input_buffer,
                    cols,
                    epsilon,
                )?;
                submission.cast_f32_to_f16(
                    &self.matvec_input_buffer,
                    &self.vector_f16_buffer,
                    cols,
                )?;
                submission.matmul_f16_to_f32(
                    &self.vector_f16_buffer,
                    transposed_f16,
                    &self.logits_buffer,
                    1,
                    cols,
                    rows,
                )?;
                submission.argmax_f32(&self.logits_buffer, 1, rows, &self.next_token_buffer)?;
                submission
                    .copy_device_to_host(&self.next_token_buffer, &self.next_token_host_buffer)?;
                let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                (
                    cuda_argmax_token_id(self.next_token_host_buffer.read_i32()?).map_err(
                        |error| match error {
                            ReferenceTextGenerationError::Runtime(runtime) => runtime,
                            other => crate::RuntimeError::Backend(other.to_string()),
                        },
                    )?,
                    0,
                    std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                    report.encoded_operations,
                )
            } else if can_use_q8_1_argmax(mode) && !materialize_logits {
                self.argmax_state_host_buffer
                    .write_bytes(initial_cuda_argmax_pair_bytes().as_slice())?;
                submission.rms_norm_q8_1(
                    input,
                    norm_weight,
                    &self.matvec_input_q8_1_buffer,
                    cols,
                    epsilon,
                )?;
                submission.copy_host_to_device(
                    &self.argmax_state_host_buffer,
                    &self.argmax_state_buffer,
                )?;
                submission.quantized_matvec_q8_1_argmax(
                    weights,
                    0,
                    mode,
                    rows,
                    cols,
                    &self.matvec_input_q8_1_buffer,
                    None,
                    &self.argmax_state_buffer,
                )?;
                submission.copy_device_to_host(
                    &self.argmax_state_buffer,
                    &self.argmax_state_host_buffer,
                )?;
                let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                (
                    cuda_argmax_token_from_packed_host_buffer(&self.argmax_state_host_buffer)
                        .map_err(|error| match error {
                            ReferenceTextGenerationError::Runtime(runtime) => runtime,
                            other => crate::RuntimeError::Backend(other.to_string()),
                        })?,
                    std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX),
                    std::mem::size_of::<u64>().try_into().unwrap_or(u64::MAX),
                    report.encoded_operations,
                )
            } else if can_use_q8_1_quantized_matvec(mode) {
                submission.rms_norm_q8_1(
                    input,
                    norm_weight,
                    &self.matvec_input_q8_1_buffer,
                    cols,
                    epsilon,
                )?;
                submission.quantized_matvec_q8_1(
                    weights,
                    0,
                    mode,
                    rows,
                    cols,
                    &self.matvec_input_q8_1_buffer,
                    None,
                    &self.logits_buffer,
                )?;
                submission.argmax_f32(&self.logits_buffer, 1, rows, &self.next_token_buffer)?;
                submission
                    .copy_device_to_host(&self.next_token_buffer, &self.next_token_host_buffer)?;
                let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                (
                    cuda_argmax_token_id(self.next_token_host_buffer.read_i32()?).map_err(
                        |error| match error {
                            ReferenceTextGenerationError::Runtime(runtime) => runtime,
                            other => crate::RuntimeError::Backend(other.to_string()),
                        },
                    )?,
                    0,
                    std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                    report.encoded_operations,
                )
            } else {
                submission.rms_norm(
                    input,
                    norm_weight,
                    &self.matvec_input_buffer,
                    cols,
                    epsilon,
                )?;
                submission.quantized_matvec(
                    weights,
                    0,
                    mode,
                    rows,
                    cols,
                    &self.matvec_input_buffer,
                    &self.logits_buffer,
                )?;
                submission.argmax_f32(&self.logits_buffer, 1, rows, &self.next_token_buffer)?;
                submission
                    .copy_device_to_host(&self.next_token_buffer, &self.next_token_host_buffer)?;
                let report = submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
                (
                    cuda_argmax_token_id(self.next_token_host_buffer.read_i32()?).map_err(
                        |error| match error {
                            ReferenceTextGenerationError::Runtime(runtime) => runtime,
                            other => crate::RuntimeError::Backend(other.to_string()),
                        },
                    )?,
                    0,
                    std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                    report.encoded_operations,
                )
            };
        Ok((
            selected,
            CudaQuantizedMatvecStats {
                host_to_device_bytes,
                device_to_host_bytes,
                submission_count: 1,
                sync_count: 1,
                kernel_launches,
            },
        ))
    }
}

#[derive(Clone, Debug)]
struct CompletedQwen35Stream {
    backend: &'static str,
    policy: GenerationStreamingPolicy,
    chunk: Option<GenerationStreamChunk>,
    terminal: Option<GenerationStreamTerminal>,
}

impl CompletedQwen35Stream {
    fn new(response: GenerationResponse, backend: &'static str) -> Self {
        let chunk = GenerationStreamChunk {
            request_id: response.request_id.clone(),
            model_id: response.model_id.clone(),
            session_id: response.session_id.clone(),
            output: response.output.clone(),
            cumulative_output_tokens: response.output.tokens.len(),
        };
        let terminal = GenerationStreamTerminal {
            status: GenerationStreamStatus::Succeeded,
            response,
            failure_reason: None,
            diagnostic: None,
        };
        Self {
            backend,
            policy: default_generation_streaming_policy(),
            chunk: Some(chunk),
            terminal: Some(terminal),
        }
    }
}

impl GenerationEventStream for CompletedQwen35Stream {
    fn policy(&self) -> &GenerationStreamingPolicy {
        &self.policy
    }

    fn next_event(&mut self) -> Option<GenerationStreamEvent> {
        if let Some(chunk) = self.chunk.take() {
            return Some(GenerationStreamEvent::Chunk(chunk));
        }
        self.terminal.take().map(GenerationStreamEvent::Terminal)
    }

    fn cancel(&mut self) -> Option<GenerationStreamTerminal> {
        self.chunk.take();
        self.terminal.take().map(|mut terminal| {
            terminal.status = GenerationStreamStatus::Cancelled;
            terminal.failure_reason = Some(String::from("stream cancelled by caller"));
            terminal.diagnostic = Some(
                LocalRuntimeDiagnostic::new(
                    psionic_runtime::LocalRuntimeErrorCode::Cancelled,
                    499,
                    "stream cancelled by caller",
                )
                .with_backend(self.backend),
            );
            terminal
        })
    }

    fn disconnect(&mut self) -> Option<GenerationStreamTerminal> {
        self.chunk.take();
        self.terminal.take().map(|mut terminal| {
            terminal.status = GenerationStreamStatus::Disconnected;
            terminal.failure_reason = Some(String::from("stream disconnected by caller"));
            terminal.diagnostic = Some(
                LocalRuntimeDiagnostic::new(
                    psionic_runtime::LocalRuntimeErrorCode::Disconnected,
                    499,
                    "stream disconnected by caller",
                )
                .with_backend(self.backend),
            );
            terminal
        })
    }
}

#[derive(Clone, Debug)]
struct HostMatrix {
    kind: HostMatrixKind,
}

#[derive(Clone, Debug)]
enum HostMatrixKind {
    Dense(DenseMatrix),
    Quantized(QuantizedMatrix),
}

impl HostMatrix {
    fn load(artifact: &GgufBlobArtifact, name: &str) -> Result<Self, ModelLoadError> {
        let storage = artifact.paged_tensor(name)?;
        if storage.metadata().quantized_layout.is_some() {
            let matrix = load_quantized_matrix(artifact, name)?;
            return Ok(Self {
                kind: HostMatrixKind::Quantized(matrix),
            });
        }
        let matrix = load_dense_matrix(artifact, name)?;
        Ok(Self {
            kind: HostMatrixKind::Dense(matrix),
        })
    }

    fn decode_row(&self, row_index: usize) -> Result<Vec<f32>, ReferenceTextGenerationError> {
        match &self.kind {
            HostMatrixKind::Dense(matrix) => matrix
                .decode_row(row_index)
                .map_err(ReferenceTextGenerationError::Runtime),
            HostMatrixKind::Quantized(matrix) => matrix
                .decode_row(row_index)
                .map_err(ReferenceTextGenerationError::Runtime),
        }
    }

    fn host_residency_bytes(&self) -> usize {
        match &self.kind {
            HostMatrixKind::Dense(matrix) => matrix.host_residency_bytes(),
            HostMatrixKind::Quantized(matrix) => matrix.byte_length(),
        }
    }

    fn rows(&self) -> usize {
        match &self.kind {
            HostMatrixKind::Dense(matrix) => matrix.rows,
            HostMatrixKind::Quantized(matrix) => matrix.rows,
        }
    }

    fn columns(&self) -> usize {
        match &self.kind {
            HostMatrixKind::Dense(matrix) => matrix.columns,
            HostMatrixKind::Quantized(matrix) => matrix.columns,
        }
    }

    fn matvec(&self, input: &[f32]) -> Result<Vec<f32>, crate::RuntimeError> {
        match &self.kind {
            HostMatrixKind::Dense(matrix) => matrix.matvec(input),
            HostMatrixKind::Quantized(matrix) => matrix.matvec(input),
        }
    }
}

#[derive(Clone, Debug)]
struct DenseMatrix {
    rows: usize,
    columns: usize,
    values: Vec<f32>,
}

impl DenseMatrix {
    fn decode_row(&self, row_index: usize) -> Result<Vec<f32>, crate::RuntimeError> {
        if row_index >= self.rows {
            return Err(crate::RuntimeError::Backend(format!(
                "dense row index {row_index} exceeds row count {}",
                self.rows
            )));
        }
        let start = row_index.saturating_mul(self.columns);
        let end = start.saturating_add(self.columns);
        Ok(self.values[start..end].to_vec())
    }

    fn host_residency_bytes(&self) -> usize {
        vec_f32_bytes(self.values.as_slice())
    }

    fn matvec(&self, input: &[f32]) -> Result<Vec<f32>, crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
                "dense matvec input width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        Ok(self
            .values
            .chunks_exact(self.columns)
            .map(|row| dot(row, input))
            .collect())
    }
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

    fn decode_row(&self, row_index: usize) -> Result<Vec<f32>, crate::RuntimeError> {
        if row_index >= self.rows {
            return Err(crate::RuntimeError::Backend(format!(
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

    fn matvec(&self, input: &[f32]) -> Result<Vec<f32>, crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
                "quantized matvec input width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let bytes = self.storage.bytes().map_err(model_load_runtime_error)?;
        let mut output = Vec::with_capacity(self.rows);
        for row_bytes in bytes.chunks_exact(self.row_byte_len) {
            output.push(quantized_row_dot(input, self.mode, row_bytes)?);
        }
        Ok(output)
    }
}

#[derive(Clone, Debug)]
struct MetalProjectionStep {
    values: Vec<f32>,
    kernel_count: usize,
    bytes_moved: u64,
}

#[derive(Clone, Debug)]
struct MetalProjectionOutputsStep {
    outputs: ProjectionOutputs,
    kernel_count: usize,
    bytes_moved: u64,
}

#[derive(Clone, Debug)]
struct MetalQuantizedMatrix {
    native_weights: Option<MetalBuffer>,
    mode: Option<QuantizationMode>,
    rows: usize,
    columns: usize,
    host_projection: HostMatrix,
}

impl MetalQuantizedMatrix {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        name: &str,
    ) -> Result<Self, ModelLoadError> {
        Self::from_host_matrix(backend, name, HostMatrix::load(artifact, name)?)
    }

    fn from_host_matrix(
        backend: &mut MetalBackend,
        name: &str,
        host_projection: HostMatrix,
    ) -> Result<Self, ModelLoadError> {
        let (native_weights, mode) = match &host_projection.kind {
            HostMatrixKind::Quantized(matrix)
                if supports_native_metal_qwen35_projection(matrix.mode) =>
            {
                let weights = backend
                    .quantized_buffer_from_slice(
                        matrix.storage.metadata().shape.clone(),
                        matrix.mode,
                        matrix.storage.bytes()?,
                        None,
                    )
                    .map_err(|error| ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: format!("failed to upload `{name}` to metal: {error}"),
                    })?;
                (Some(weights), Some(matrix.mode))
            }
            HostMatrixKind::Quantized(matrix) => (None, Some(matrix.mode)),
            HostMatrixKind::Dense(_) => (None, None),
        };
        let rows = host_projection.rows();
        let columns = host_projection.columns();
        Ok(Self {
            native_weights,
            mode,
            rows,
            columns,
            host_projection,
        })
    }

    fn is_native(&self) -> bool {
        self.native_weights.is_some()
    }

    fn device_residency_bytes(&self) -> usize {
        self.native_weights
            .as_ref()
            .map(MetalBuffer::byte_len)
            .unwrap_or(0)
    }

    fn byte_length(&self) -> usize {
        self.native_weights
            .as_ref()
            .map(MetalBuffer::byte_len)
            .unwrap_or_else(|| self.host_projection.host_residency_bytes())
    }

    fn matvec(
        &self,
        backend: &mut MetalBackend,
        input: &[f32],
    ) -> Result<MetalProjectionStep, ReferenceTextGenerationError> {
        if let (Some(weights), Some(mode)) = (&self.native_weights, self.mode) {
            let values = backend
                .quantized_matvec(weights, mode, self.rows, self.columns, input)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            return Ok(MetalProjectionStep {
                values,
                kernel_count: 1,
                bytes_moved: self.byte_length() as u64,
            });
        }
        Ok(MetalProjectionStep {
            values: self
                .host_projection
                .matvec(input)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            kernel_count: 0,
            bytes_moved: self.byte_length() as u64,
        })
    }

    fn select_logits_output(
        &self,
        backend: &mut MetalBackend,
        input: &[f32],
        output_mode: MetalLogitsOutputMode,
    ) -> Result<psionic_backend_metal::MetalLogitsSelectionResult, ReferenceTextGenerationError>
    {
        if let (Some(weights), Some(mode)) = (&self.native_weights, self.mode) {
            return backend
                .quantized_matvec_select_logits_output(
                    weights,
                    0,
                    mode,
                    self.rows,
                    self.columns,
                    input,
                    output_mode,
                )
                .map_err(ReferenceTextGenerationError::Runtime);
        }

        let logits = self
            .host_projection
            .matvec(input)
            .map_err(ReferenceTextGenerationError::Runtime)?;
        match output_mode {
            MetalLogitsOutputMode::RawLogits => {
                Ok(psionic_backend_metal::MetalLogitsSelectionResult {
                    selected_tokens: vec![select_argmax(logits.as_slice())?.as_u32()],
                    candidates: None,
                    logits: Some(logits),
                    metrics: psionic_backend_metal::MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: self
                            .rows
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                        raw_logits_materialized: true,
                    },
                })
            }
            MetalLogitsOutputMode::GreedyToken => {
                let token = select_argmax(logits.as_slice())?;
                Ok(psionic_backend_metal::MetalLogitsSelectionResult {
                    selected_tokens: vec![token.as_u32()],
                    candidates: None,
                    logits: None,
                    metrics: psionic_backend_metal::MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: std::mem::size_of::<u32>().try_into().unwrap_or(u64::MAX),
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::TopKCandidates(top_k) => {
                let mut scored = logits
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| (index as u32, value))
                    .collect::<Vec<_>>();
                scored.sort_by(|left, right| right.1.total_cmp(&left.1));
                scored.truncate(top_k);
                let selected_tokens = scored
                    .first()
                    .map(|(index, _)| vec![*index])
                    .unwrap_or_default();
                let indices = scored.iter().map(|(index, _)| *index).collect::<Vec<_>>();
                let values = scored
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>();
                Ok(psionic_backend_metal::MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: Some(psionic_backend_metal::MetalTopKResult {
                        row_count: 1,
                        top_k,
                        indices,
                        values,
                    }),
                    logits: None,
                    metrics: psionic_backend_metal::MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: top_k
                            .saturating_mul(std::mem::size_of::<u32>() + std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                        raw_logits_materialized: false,
                    },
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
struct MetalQuantizedProjectionGroup {
    parts: Vec<MetalQuantizedMatrix>,
    rows_per_projection: Vec<usize>,
    columns: usize,
}

impl MetalQuantizedProjectionGroup {
    fn load(
        backend: &mut MetalBackend,
        artifact: &GgufBlobArtifact,
        names: &[&str],
    ) -> Result<Self, ModelLoadError> {
        let mut columns = None;
        let mut rows_per_projection = Vec::with_capacity(names.len());
        let mut parts = Vec::with_capacity(names.len());
        for name in names {
            let matrix = MetalQuantizedMatrix::load(backend, artifact, name)?;
            if let Some(expected_columns) = columns {
                if matrix.columns != expected_columns {
                    return Err(ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: format!(
                            "packed qwen35 metal projection requires matching input width; `{name}` had {} but expected {}",
                            matrix.columns, expected_columns,
                        ),
                    });
                }
            } else {
                columns = Some(matrix.columns);
            }
            rows_per_projection.push(matrix.rows);
            parts.push(matrix);
        }
        Ok(Self {
            parts,
            rows_per_projection,
            columns: columns.unwrap_or(0),
        })
    }

    fn total_rows(&self) -> usize {
        self.rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add)
    }

    fn device_residency_bytes(&self) -> usize {
        self.parts
            .iter()
            .map(MetalQuantizedMatrix::device_residency_bytes)
            .fold(0usize, usize::saturating_add)
    }

    fn byte_length(&self) -> usize {
        self.parts
            .iter()
            .map(MetalQuantizedMatrix::byte_length)
            .fold(0usize, usize::saturating_add)
    }

    fn matvec(
        &self,
        backend: &mut MetalBackend,
        input: &[f32],
    ) -> Result<MetalProjectionOutputsStep, ReferenceTextGenerationError> {
        if input.len() != self.columns {
            return Err(ReferenceTextGenerationError::Runtime(
                crate::RuntimeError::Backend(format!(
                    "packed qwen35 metal projection width mismatch: expected {}, actual {}",
                    self.columns,
                    input.len()
                )),
            ));
        }
        if self.parts.iter().all(MetalQuantizedMatrix::is_native) {
            let requests = self
                .parts
                .iter()
                .map(|part| MetalQuantizedMatvecRequest {
                    weights: part
                        .native_weights
                        .as_ref()
                        .expect("native projection weights"),
                    byte_offset: 0,
                    mode: part.mode.expect("native projection mode"),
                    rows: part.rows,
                    columns: part.columns,
                })
                .collect::<Vec<_>>();
            let results = backend
                .quantized_matvec_batch(requests.as_slice(), input)
                .map_err(ReferenceTextGenerationError::Runtime)?;
            let mut values = Vec::with_capacity(self.total_rows());
            for result in results {
                values.extend(result.values);
            }
            return Ok(MetalProjectionOutputsStep {
                outputs: ProjectionOutputs::new(self.rows_per_projection.as_slice(), values)
                    .map_err(ReferenceTextGenerationError::Runtime)?,
                kernel_count: 1,
                bytes_moved: self.byte_length() as u64,
            });
        }

        let mut values = Vec::with_capacity(self.total_rows());
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        for part in &self.parts {
            let step = part.matvec(backend, input)?;
            values.extend(step.values);
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
        }
        Ok(MetalProjectionOutputsStep {
            outputs: ProjectionOutputs::new(self.rows_per_projection.as_slice(), values)
                .map_err(ReferenceTextGenerationError::Runtime)?,
            kernel_count,
            bytes_moved,
        })
    }
}

#[derive(Clone, Debug)]
struct CudaQuantizedMatrix {
    storage: CudaBuffer,
    host: QuantizedMatrix,
    transposed_f16: Option<CudaBuffer>,
}

impl CudaQuantizedMatrix {
    fn device_residency_bytes(&self) -> usize {
        self.storage.byte_len().saturating_add(
            self.transposed_f16
                .as_ref()
                .map(CudaBuffer::byte_len)
                .unwrap_or(0),
        )
    }

    fn matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<CudaQuantizedMatvecStats, crate::RuntimeError> {
        plan.run_projection_matvec(
            backend,
            &self.storage,
            0,
            self.host.mode,
            self.host.rows,
            self.host.columns,
            input,
            output,
        )
    }

    fn host_matvec(&self, input: &[f32]) -> Result<Vec<f32>, crate::RuntimeError> {
        self.host.matvec(input)
    }
}

#[derive(Clone, Debug)]
struct CudaQuantizedProjectionGroup {
    storage: CudaBuffer,
    host_parts: Vec<QuantizedMatrix>,
    native_parts: Vec<CudaQuantizedMatrix>,
    rows_per_projection: Vec<usize>,
    columns: usize,
    mode: QuantizationMode,
    transposed_f16: Option<CudaBuffer>,
}

#[derive(Clone, Debug)]
struct ProjectionOutputs {
    values: Vec<f32>,
    spans: Vec<(usize, usize)>,
}

#[derive(Clone, Debug)]
struct Qwen35HybridHostDebug {
    conv: Vec<f32>,
    decay: Vec<f32>,
    beta: Vec<f32>,
    qkv_norm: Vec<f32>,
    gated_delta: Vec<f32>,
    hybrid_norm: Vec<f32>,
    projected: Vec<f32>,
    next_conv_state: Vec<f32>,
    next_delta_state: Vec<f32>,
    final_hidden: Vec<f32>,
}

struct LoadedQuantizedProjectionPart {
    name: String,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: Vec<u8>,
}

impl ProjectionOutputs {
    fn slice(&self, index: usize) -> Result<&[f32], crate::RuntimeError> {
        let Some((start, end)) = self.spans.get(index).copied() else {
            return Err(crate::RuntimeError::Backend(format!(
                "projection output index {index} exceeds projection count {}",
                self.spans.len()
            )));
        };
        Ok(&self.values[start..end])
    }
}

impl CudaQuantizedProjectionGroup {
    fn total_rows(&self) -> usize {
        self.rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add)
    }

    fn device_residency_bytes(&self) -> usize {
        let storage_bytes = if self.native_parts.is_empty() {
            self.storage.byte_len()
        } else {
            self.native_parts
                .iter()
                .map(CudaQuantizedMatrix::device_residency_bytes)
                .sum()
        };
        storage_bytes.saturating_add(
            self.transposed_f16
                .as_ref()
                .map(CudaBuffer::byte_len)
                .unwrap_or(0),
        )
    }

    fn matvec_profiled(
        &self,
        backend: &mut CudaBackend,
        plan: &mut Qwen35CudaStepPlan,
        input: &[f32],
    ) -> Result<(ProjectionOutputs, CudaQuantizedMatvecStats), crate::RuntimeError> {
        let mut values = Vec::new();
        let stats = plan.run_projection_matvec(
            backend,
            &self.storage,
            0,
            self.mode,
            self.total_rows(),
            self.columns,
            input,
            &mut values,
        )?;
        Ok((
            ProjectionOutputs::new(self.rows_per_projection.as_slice(), values)?,
            stats,
        ))
    }

    fn host_matvec(&self, input: &[f32]) -> Result<ProjectionOutputs, crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
                "packed projection host matvec input width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        let mut values = Vec::with_capacity(self.total_rows());
        for matrix in &self.host_parts {
            values.extend(matrix.matvec(input)?);
        }
        ProjectionOutputs::new(self.rows_per_projection.as_slice(), values)
    }
}

impl ProjectionOutputs {
    fn new(rows_per_projection: &[usize], values: Vec<f32>) -> Result<Self, crate::RuntimeError> {
        let expected = rows_per_projection
            .iter()
            .copied()
            .fold(0usize, usize::saturating_add);
        if values.len() != expected {
            return Err(crate::RuntimeError::Backend(format!(
                "packed projection output mismatch: expected {expected} values, actual {}",
                values.len()
            )));
        }
        let mut spans = Vec::with_capacity(rows_per_projection.len());
        let mut offset = 0usize;
        for rows in rows_per_projection {
            let end = offset.saturating_add(*rows);
            spans.push((offset, end));
            offset = end;
        }
        Ok(Self { values, spans })
    }
}

fn load_quantized_matrix(
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<QuantizedMatrix, ModelLoadError> {
    let storage = artifact.paged_tensor(name)?;
    let metadata = storage.metadata().clone();
    let dims = metadata.shape.dims().to_vec();
    let [rows, columns] = dims.as_slice() else {
        return Err(ModelLoadError::InvalidTensorShape {
            name: metadata.name.clone(),
            expected: vec![0, 0],
            actual: dims,
        });
    };
    let layout =
        metadata
            .quantized_layout
            .ok_or_else(|| ModelLoadError::UnsupportedTensorDType {
                name: metadata.name.clone(),
                dtype: String::from("quantized"),
            })?;
    let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization: metadata.quantization,
            shape: metadata.shape.dims().to_vec(),
        }
    })?;
    Ok(QuantizedMatrix {
        storage,
        mode: metadata.quantization,
        rows: *rows,
        columns: *columns,
        row_byte_len,
    })
}

fn load_cuda_quantized_matrix(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<CudaQuantizedMatrix, ModelLoadError> {
    let host = load_quantized_matrix(artifact, name)?;
    let transposed_f16 = if qwen35_requires_dense_f16_mirror(host.mode) {
        Some(
            try_build_cuda_transposed_f16_mirror(
                backend,
                name,
                host.mode,
                host.rows,
                host.columns,
                host.row_byte_len,
                host.storage.bytes()?,
            )?
            .ok_or_else(|| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to build required qwen35 cuda f16 transpose mirror for `{name}`"
                ),
            })?,
        )
    } else {
        None
    };
    let storage = backend
        .byte_buffer(host.storage.bytes()?)
        .map_err(|error| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload `{name}` to cuda: {error}"),
        })?;
    Ok(CudaQuantizedMatrix {
        storage,
        host,
        transposed_f16,
    })
}

fn load_cuda_quantized_projection_group(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
    allow_mixed_native_parts: bool,
) -> Result<CudaQuantizedProjectionGroup, ModelLoadError> {
    let mut mode = None;
    let mut columns = None;
    let mut row_byte_len = None;
    let mut rows_per_projection = Vec::with_capacity(names.len());
    let mut host_parts = Vec::with_capacity(names.len());
    let mut projections = Vec::with_capacity(names.len());
    let mut mixed_quantization = false;
    for name in names {
        let projection = load_quantized_matrix(artifact, name)?;
        if let Some(expected_mode) = mode {
            if projection.mode != expected_mode {
                mixed_quantization = true;
            }
        } else {
            mode = Some(projection.mode);
        }
        if let Some(expected_columns) = columns {
            if projection.columns != expected_columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed qwen35 cuda projection requires matching input width; `{name}` had {} but expected {}",
                        projection.columns, expected_columns
                    ),
                });
            }
        } else {
            columns = Some(projection.columns);
        }
        if let Some(expected_row_byte_len) = row_byte_len {
            if projection.row_byte_len != expected_row_byte_len {
                mixed_quantization = true;
            }
        } else {
            row_byte_len = Some(projection.row_byte_len);
        }
        rows_per_projection.push(projection.rows);
        host_parts.push(projection.clone());
        projections.push(LoadedQuantizedProjectionPart {
            name: String::from(*name),
            mode: projection.mode,
            rows: projection.rows,
            columns: projection.columns,
            row_byte_len: projection.row_byte_len,
            bytes: projection.storage.bytes()?.to_vec(),
        });
    }
    let packed = pack_quantized_projection_bytes(
        projections
            .iter()
            .map(|projection| projection.bytes.as_slice())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let resolved_mode = mode.expect("projection group should resolve quantization mode");
    let resolved_columns = columns.expect("projection group should resolve columns");
    let _resolved_row_byte_len =
        row_byte_len.expect("projection group should resolve row byte length");
    let total_rows = rows_per_projection
        .iter()
        .copied()
        .fold(0usize, usize::saturating_add);
    let native_parts = if allow_mixed_native_parts
        && mixed_quantization
        && projections
            .iter()
            .all(|projection| can_use_cuda_quantized_matvec(projection.mode))
    {
        projections
            .iter()
            .zip(host_parts.iter().cloned())
            .map(|(projection, host)| {
                let storage = backend.byte_buffer(projection.bytes.as_slice()).map_err(
                    |error| ModelLoadError::ArtifactFormat {
                        format: String::from("gguf"),
                        message: format!(
                            "failed to upload native mixed qwen35 cuda projection `{}`: {error}",
                            projection.name
                        ),
                    },
                )?;
                Ok(CudaQuantizedMatrix {
                    storage,
                    host,
                    transposed_f16: None,
                })
            })
            .collect::<Result<Vec<_>, ModelLoadError>>()?
    } else {
        Vec::new()
    };
    let transposed_f16 = if native_parts.is_empty()
        && (mixed_quantization || qwen35_requires_dense_f16_mirror(resolved_mode))
    {
        Some(
            try_build_cuda_projection_group_transposed_f16_mirror(
                backend,
                names.join(", ").as_str(),
                projections.as_slice(),
                total_rows,
                resolved_columns,
            )?
            .ok_or_else(|| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to build required qwen35 cuda f16 transpose mirror for packed projection `{}`",
                    names.join(", ")
                ),
            })?,
        )
    } else {
        None
    };
    let storage = if let Some(first) = native_parts.first() {
        first.storage.clone()
    } else {
        backend
            .byte_buffer(packed.as_slice())
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to upload packed qwen35 cuda projection `{}`: {error}",
                    names.join(", ")
                ),
            })?
    };
    Ok(CudaQuantizedProjectionGroup {
        storage,
        host_parts,
        native_parts,
        rows_per_projection,
        columns: resolved_columns,
        mode: resolved_mode,
        transposed_f16,
    })
}

fn pack_quantized_projection_bytes(projections: &[&[u8]]) -> Vec<u8> {
    let total = projections
        .iter()
        .copied()
        .fold(0usize, |sum, bytes| sum.saturating_add(bytes.len()));
    let mut packed = Vec::with_capacity(total);
    for bytes in projections {
        packed.extend_from_slice(bytes);
    }
    packed
}

fn decode_quantized_projection_group_bytes_transposed_f16(
    projections: &[LoadedQuantizedProjectionPart],
    total_rows: usize,
    columns: usize,
    name: &str,
) -> Result<Vec<u8>, ModelLoadError> {
    let mut transposed = vec![
        0_u8;
        total_rows
            .saturating_mul(columns)
            .saturating_mul(std::mem::size_of::<u16>())
    ];
    let mut decoded_row = Vec::with_capacity(columns);
    let mut row_offset = 0usize;
    for projection in projections {
        if projection.columns != columns {
            return Err(ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "packed projection `{name}` width mismatch while building f16 transpose: expected {columns}, actual {} for `{}`",
                    projection.columns, projection.name
                ),
            });
        }
        let expected_bytes = projection.rows.saturating_mul(projection.row_byte_len);
        if projection.bytes.len() != expected_bytes {
            return Err(ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "packed projection `{name}` byte length mismatch while building f16 transpose for `{}`: expected {expected_bytes}, actual {}",
                    projection.name,
                    projection.bytes.len()
                ),
            });
        }
        for (row_index, row_bytes) in projection
            .bytes
            .chunks_exact(projection.row_byte_len)
            .enumerate()
        {
            decoded_row.clear();
            decode_quantized_row_into(projection.mode, row_bytes, &mut decoded_row).map_err(
                |error| ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "failed to decode quantized tensor `{}` while building packed projection f16 transpose `{name}`: {error}",
                        projection.name
                    ),
                },
            )?;
            if decoded_row.len() != columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed projection `{name}` decode width mismatch for `{}` while building f16 transpose: expected {columns}, actual {}",
                        projection.name,
                        decoded_row.len()
                    ),
                });
            }
            let packed_row_index = row_offset.saturating_add(row_index);
            for (column_index, value) in decoded_row.iter().copied().enumerate() {
                let offset = column_index
                    .saturating_mul(total_rows)
                    .saturating_add(packed_row_index)
                    .saturating_mul(std::mem::size_of::<u16>());
                transposed[offset..offset + std::mem::size_of::<u16>()]
                    .copy_from_slice(&f32_to_f16_bits(value).to_le_bytes());
            }
        }
        row_offset = row_offset.saturating_add(projection.rows);
    }
    if row_offset != total_rows {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!(
                "packed projection `{name}` row count mismatch while building f16 transpose: expected {total_rows}, actual {row_offset}"
            ),
        });
    }
    Ok(transposed)
}

fn try_build_cuda_projection_group_transposed_f16_mirror(
    backend: &mut CudaBackend,
    name: &str,
    projections: &[LoadedQuantizedProjectionPart],
    total_rows: usize,
    columns: usize,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let transposed = if projections.windows(2).all(|window| {
        window[0].mode == window[1].mode && window[0].row_byte_len == window[1].row_byte_len
    }) && !projections.is_empty()
    {
        try_build_cuda_transposed_f16_mirror(
            backend,
            name,
            projections[0].mode,
            total_rows,
            columns,
            projections[0].row_byte_len,
            pack_quantized_projection_bytes(
                projections
                    .iter()
                    .map(|projection| projection.bytes.as_slice())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .as_slice(),
        )?
        .map(Some)
        .unwrap_or(None)
    } else {
        let transposed = decode_quantized_projection_group_bytes_transposed_f16(
            projections,
            total_rows,
            columns,
            name,
        )?;
        match backend.byte_buffer(transposed.as_slice()) {
            Ok(buffer) => Some(buffer),
            Err(error) if error.to_string().contains("out of memory") => None,
            Err(error) => {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "failed to upload packed projection f16 transpose mirror for `{name}` to cuda: {error}"
                    ),
                });
            }
        }
    };
    Ok(transposed)
}

fn family_fact_usize(
    metadata: &GgufDecoderFamilyMetadata,
    key: &str,
) -> Result<usize, ModelLoadError> {
    metadata
        .family_facts
        .get(key)
        .and_then(GgufMetadataValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("missing required qwen35 family fact `{key}`"),
        })
}

fn family_fact_bool(
    metadata: &GgufDecoderFamilyMetadata,
    key: &str,
) -> Result<bool, ModelLoadError> {
    metadata
        .family_facts
        .get(key)
        .and_then(GgufMetadataValue::as_bool)
        .ok_or_else(|| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("missing required qwen35 family fact `{key}`"),
        })
}

fn family_fact_bool_or_default(
    metadata: &GgufDecoderFamilyMetadata,
    key: &str,
    default: bool,
) -> bool {
    metadata
        .family_facts
        .get(key)
        .and_then(GgufMetadataValue::as_bool)
        .unwrap_or(default)
}

fn family_fact_f32_or_default(
    metadata: &GgufDecoderFamilyMetadata,
    key: &str,
    default: f32,
) -> f32 {
    metadata
        .family_facts
        .get(key)
        .and_then(GgufMetadataValue::as_f32)
        .unwrap_or(default)
}

fn family_fact_usize_array(
    metadata: &GgufDecoderFamilyMetadata,
    keys: &[&str],
) -> Option<Vec<usize>> {
    keys.iter().find_map(|key| {
        metadata.family_facts.get(*key).and_then(|value| {
            value.as_array().map(|values| {
                values
                    .iter()
                    .filter_map(GgufMetadataValue::as_u64)
                    .filter_map(|value| usize::try_from(value).ok())
                    .collect::<Vec<_>>()
            })
        })
    })
}

fn qwen35_attention_scale(metadata: &GgufDecoderFamilyMetadata, head_dim: usize) -> f32 {
    family_fact_f32_or_default(
        metadata,
        "qwen35.attention.scale",
        (head_dim as f32).sqrt().recip(),
    )
}

fn qwen35_mrope_sections(metadata: &GgufDecoderFamilyMetadata) -> Option<[usize; 4]> {
    let values = family_fact_usize_array(
        metadata,
        &[
            "qwen35.mrope_sections",
            "qwen35.rope.mrope_section",
            "qwen35.rope.dimension_sections",
        ],
    )?;
    if values.is_empty() {
        return None;
    }
    let mut sections = [0_usize; 4];
    for (slot, value) in values.into_iter().take(4).enumerate() {
        sections[slot] = value;
    }
    Some(sections)
}

fn qwen35_mrope_interleaved(metadata: &GgufDecoderFamilyMetadata) -> bool {
    family_fact_bool_or_default(metadata, "qwen35.rope.mrope_interleaved", false)
}

fn gguf_local_blob_open_options() -> LocalBlobOpenOptions {
    LocalBlobOpenOptions::default().with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel)
}

fn emit_qwen35_hidden_debug(
    position: usize,
    layer_index: usize,
    kind: &Qwen35LayerKind,
    buffer: &CudaBuffer,
    element_count: usize,
) -> Result<(), ReferenceTextGenerationError> {
    let values = buffer
        .read_f32_at_offset(0, element_count)
        .map_err(ReferenceTextGenerationError::Runtime)?;
    let mut finite_count = 0usize;
    let mut nan_count = 0usize;
    let mut inf_count = 0usize;
    let mut max_abs = 0.0_f32;
    for value in values.iter().copied() {
        if value.is_nan() {
            nan_count = nan_count.saturating_add(1);
            continue;
        }
        if !value.is_finite() {
            inf_count = inf_count.saturating_add(1);
            continue;
        }
        finite_count = finite_count.saturating_add(1);
        max_abs = max_abs.max(value.abs());
    }
    let kind = match kind {
        Qwen35LayerKind::Hybrid(_) => "hybrid",
        Qwen35LayerKind::FullAttention(_) => "full_attention",
    };
    eprintln!(
        "qwen35_fused_debug position={} layer={} kind={} finite={} nan={} inf={} max_abs={:.6}",
        position, layer_index, kind, finite_count, nan_count, inf_count, max_abs
    );
    Ok(())
}

fn emit_qwen35_buffer_debug(
    position: usize,
    layer_index: usize,
    label: &str,
    buffer: &CudaBuffer,
    element_offset: usize,
    element_count: usize,
) -> Result<(), ReferenceTextGenerationError> {
    if element_count == 0 {
        return Ok(());
    }
    let values = buffer
        .read_f32_at_offset(element_offset, element_count)
        .map_err(ReferenceTextGenerationError::Runtime)?;
    let mut finite_count = 0usize;
    let mut nan_count = 0usize;
    let mut inf_count = 0usize;
    let mut max_abs = 0.0_f32;
    for value in values.iter().copied() {
        if value.is_nan() {
            nan_count = nan_count.saturating_add(1);
            continue;
        }
        if !value.is_finite() {
            inf_count = inf_count.saturating_add(1);
            continue;
        }
        finite_count = finite_count.saturating_add(1);
        max_abs = max_abs.max(value.abs());
    }
    eprintln!(
        "qwen35_hybrid_debug position={} layer={} buffer={} offset={} count={} finite={} nan={} inf={} max_abs={:.6}",
        position,
        layer_index,
        label,
        element_offset,
        element_count,
        finite_count,
        nan_count,
        inf_count,
        max_abs
    );
    Ok(())
}

fn emit_qwen35_hybrid_intermediate_debug(
    position: usize,
    layer_index: usize,
    hybrid: &Qwen35HybridLayer,
    hybrid_state: &Qwen35HybridState,
    plan: &Qwen35CudaStepPlan,
    hidden_size: usize,
) -> Result<(), ReferenceTextGenerationError> {
    let debug_layer = std::env::var("PSIONIC_QWEN35_DEBUG_HYBRID_LAYER")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    if debug_layer.is_some() && debug_layer != Some(layer_index) {
        return Ok(());
    }
    let debug_position_min = std::env::var("PSIONIC_QWEN35_DEBUG_HYBRID_POSITION_MIN")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if position < debug_position_min {
        return Ok(());
    }
    let qkv_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[0];
    let z_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[1];
    let alpha_rows = hybrid.qkv_gate_alpha_beta.rows_per_projection[2];
    let q_size = hybrid.group_count.saturating_mul(hybrid.state_size);
    let k_size = q_size;
    let v_size = hybrid.inner_size;
    let v_offset = q_size.saturating_add(k_size);
    let alpha_offset = qkv_rows.saturating_add(z_rows);
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "matvec_output_qkv",
        &plan.matvec_output_buffer,
        0,
        qkv_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "matvec_output_z",
        &plan.matvec_output_buffer,
        qkv_rows,
        z_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "matvec_output_alpha",
        &plan.matvec_output_buffer,
        alpha_offset,
        alpha_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "conv",
        &plan.conv_buffer,
        0,
        qkv_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "decay",
        &plan.decay_buffer,
        0,
        alpha_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "beta",
        &plan.beta_buffer,
        0,
        alpha_rows,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "qkv_norm_q",
        &plan.qkv_norm_buffer,
        0,
        q_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "qkv_norm_k",
        &plan.qkv_norm_buffer,
        q_size,
        k_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "qkv_norm_v",
        &plan.qkv_norm_buffer,
        v_offset,
        v_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "gated_delta",
        &plan.gated_delta_buffer,
        0,
        v_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "hybrid_norm",
        &plan.hybrid_norm_buffer,
        0,
        v_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "projected",
        &plan.projected_buffer,
        0,
        hidden_size,
    )?;
    emit_qwen35_buffer_debug(
        position,
        layer_index,
        "delta_state",
        &hybrid_state.delta_state,
        0,
        hybrid
            .time_step_rank
            .saturating_mul(hybrid.state_size)
            .saturating_mul(hybrid.state_size),
    )?;
    Ok(())
}

fn qwen35_hybrid_compare_enabled(layer_index: usize, position: usize) -> bool {
    if std::env::var_os("PSIONIC_QWEN35_DEBUG_HYBRID_COMPARE").is_none() {
        return false;
    }
    let debug_layer = std::env::var("PSIONIC_QWEN35_DEBUG_HYBRID_LAYER")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    if debug_layer.is_some() && debug_layer != Some(layer_index) {
        return false;
    }
    let debug_position_min = std::env::var("PSIONIC_QWEN35_DEBUG_HYBRID_POSITION_MIN")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    position >= debug_position_min
}

fn max_abs_diff(left: &[f32], right: &[f32]) -> Result<f32, ReferenceTextGenerationError> {
    if left.len() != right.len() {
        return Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(format!(
                "vector width mismatch during qwen35 diff: left={} right={}",
                left.len(),
                right.len()
            )),
        ));
    }
    Ok(left
        .iter()
        .copied()
        .zip(right.iter().copied())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f32, f32::max))
}

fn digest_qwen35_cuda_plan(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(descriptor.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.weights.digest.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.architecture.as_bytes());
    hasher.update(match metadata.family {
        GgufDecoderFamily::Qwen38 => b"|qwen38-native-cuda|v1".as_slice(),
        _ => b"|qwen35-native-cuda|v1".as_slice(),
    });
    hex::encode(hasher.finalize())
}

fn digest_qwen35_cuda_graph_cache(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
    plan_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(match metadata.family {
        GgufDecoderFamily::Qwen38 => b"qwen38-cuda-graph-cache|v1".as_slice(),
        _ => b"qwen35-cuda-graph-cache|v1".as_slice(),
    });
    hasher.update(b"|");
    hasher.update(descriptor.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.weights.digest.as_bytes());
    hasher.update(b"|");
    hasher.update(plan_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn digest_qwen35_cpu_plan(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(descriptor.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.weights.digest.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.architecture.as_bytes());
    hasher.update(b"|qwen35-native-cpu|v1");
    hex::encode(hasher.finalize())
}

fn digest_qwen38_mtp_cpu_plan(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(digest_qwen35_cpu_plan(descriptor, metadata).as_bytes());
    hasher.update(b"|qwen38-mtp-single-token-cpu|v1");
    hex::encode(hasher.finalize())
}

fn digest_qwen35_metal_plan(
    descriptor: &DecoderModelDescriptor,
    metadata: &GgufDecoderFamilyMetadata,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(descriptor.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(descriptor.weights.digest.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.architecture.as_bytes());
    hasher.update(match metadata.family {
        GgufDecoderFamily::Qwen38 => b"|qwen38-native-metal|v1".as_slice(),
        _ => b"|qwen35-native-metal|v1".as_slice(),
    });
    hex::encode(hasher.finalize())
}

fn required_tensor_name<'a>(name: Option<&'a str>, field: &str) -> Result<&'a str, ModelLoadError> {
    name.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!("missing required qwen35 tensor `{field}`"),
    })
}

fn load_dense_vector(artifact: &GgufBlobArtifact, name: &str) -> Result<Vec<f32>, ModelLoadError> {
    artifact
        .load_tensor(name)?
        .values()
        .map(|values| values.into_owned())
}

fn upload_f32_buffer(
    backend: &mut CudaBackend,
    values: &[f32],
    name: &str,
) -> Result<CudaBuffer, ModelLoadError> {
    let mut buffer =
        backend
            .f32_buffer(values.len())
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to allocate cuda f32 buffer for `{name}`: {error}"),
            })?;
    buffer
        .write_f32(values)
        .map_err(|error| ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload cuda f32 buffer for `{name}`: {error}"),
        })?;
    Ok(buffer)
}

fn try_build_cuda_host_matrix_row_major_f16_mirror(
    backend: &mut CudaBackend,
    name: &str,
    matrix: &HostMatrix,
) -> Result<Option<CudaBuffer>, ModelLoadError> {
    let dense = match &matrix.kind {
        HostMatrixKind::Dense(matrix) => dense_matrix_bytes_row_major_f16(matrix),
        HostMatrixKind::Quantized(matrix) => decode_quantized_matrix_bytes_row_major_f16(
            matrix.mode,
            matrix.rows,
            matrix.columns,
            matrix.row_byte_len,
            matrix.storage.bytes()?,
            name,
        )?,
    };
    match backend.byte_buffer(dense.as_slice()) {
        Ok(buffer) => Ok(Some(buffer)),
        Err(error) if error.to_string().contains("out of memory") => Ok(None),
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload row-major f16 mirror for `{name}` to cuda: {error}"),
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

fn try_build_cuda_transposed_f16_mirror(
    backend: &mut CudaBackend,
    name: &str,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
    bytes: &[u8],
) -> Result<Option<CudaBuffer>, ModelLoadError> {
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
        Err(error) if error.to_string().contains("out of memory") => Ok(None),
        Err(error) => Err(ModelLoadError::ArtifactFormat {
            format: String::from("gguf"),
            message: format!("failed to upload f16 transpose mirror for `{name}` to cuda: {error}"),
        }),
    }
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

fn dense_matrix_bytes_row_major_f16(matrix: &DenseMatrix) -> Vec<u8> {
    let mut dense = Vec::with_capacity(
        matrix
            .values
            .len()
            .saturating_mul(std::mem::size_of::<u16>()),
    );
    for value in &matrix.values {
        dense.extend_from_slice(&f32_to_f16_bits(*value).to_le_bytes());
    }
    dense
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

fn model_load_runtime_error(error: ModelLoadError) -> crate::RuntimeError {
    crate::RuntimeError::Backend(error.to_string())
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

fn cuda_stats_bytes(stats: CudaQuantizedMatvecStats) -> u64 {
    stats
        .host_to_device_bytes
        .saturating_add(stats.device_to_host_bytes)
}

fn qwen35_decode_graph_cache_identity(state: &Qwen35State) -> Vec<(usize, usize)> {
    state
        .layers
        .iter()
        .filter_map(|layer| match layer {
            Qwen35LayerState::Hybrid(_) => None,
            Qwen35LayerState::FullAttention(full_attention) => Some((
                full_attention.key_cache.allocation_identity(),
                full_attention.value_cache.allocation_identity(),
            )),
        })
        .collect()
}

fn vec_f32_bytes(values: &[f32]) -> usize {
    values.len().saturating_mul(std::mem::size_of::<f32>())
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
        let payload = ((mantissa >> 13) as u16) | 1;
        return sign | 0x7c00 | payload;
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
        let shift = (14 - half_exponent) as u32;
        let mut half_mantissa = (mantissa >> shift) as u16;
        let remainder_mask = (1_u32 << shift) - 1;
        let remainder = mantissa & remainder_mask;
        let halfway = 1_u32 << (shift - 1);
        if remainder > halfway || (remainder == halfway && (half_mantissa & 1) != 0) {
            half_mantissa = half_mantissa.wrapping_add(1);
        }
        return sign | half_mantissa;
    }

    let mut half = sign | (((half_exponent as u16) & 0x1f) << 10) | ((mantissa >> 13) as u16);
    let remainder = mantissa & 0x1fff;
    if remainder > 0x1000 || (remainder == 0x1000 && (half & 1) != 0) {
        half = half.wrapping_add(1);
    }
    half
}

fn qwen35_rope_runtime_parameters(
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

fn f16_bytes_to_f32_vec(bytes: &[u8]) -> Result<Vec<f32>, crate::RuntimeError> {
    if bytes.len() % std::mem::size_of::<u16>() != 0 {
        return Err(crate::RuntimeError::Backend(format!(
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

fn rms_norm(input: &[f32], weight: &[f32], epsilon: f32) -> Vec<f32> {
    let mean_square = input.iter().map(|value| value * value).sum::<f32>() / input.len() as f32;
    let scale = (mean_square + epsilon).sqrt().recip();
    input
        .iter()
        .zip(weight.iter())
        .map(|(value, weight)| value * scale * weight)
        .collect()
}

fn per_head_rms_norm(
    input: &[f32],
    head_count: usize,
    head_dim: usize,
    weight: &[f32],
    epsilon: f32,
) -> Vec<f32> {
    let mut normalized = vec![0.0_f32; input.len()];
    per_head_rms_norm_into(
        input,
        head_count,
        head_dim,
        weight,
        epsilon,
        &mut normalized,
    );
    normalized
}

fn per_head_rms_norm_into(
    input: &[f32],
    head_count: usize,
    head_dim: usize,
    weight: &[f32],
    epsilon: f32,
    output: &mut [f32],
) {
    for head_index in 0..head_count {
        let start = head_index.saturating_mul(head_dim);
        let end = start.saturating_add(head_dim);
        let input_head = &input[start..end];
        let output_head = &mut output[start..end];
        let mean_square =
            input_head.iter().map(|value| value * value).sum::<f32>() / head_dim as f32;
        let scale = (mean_square + epsilon).sqrt().recip();
        for ((out, value), weight) in output_head
            .iter_mut()
            .zip(input_head.iter().copied())
            .zip(weight.iter().copied())
        {
            *out = value * scale * weight;
        }
    }
}

fn add_vectors(left: &[f32], right: &[f32]) -> Result<Vec<f32>, crate::RuntimeError> {
    if left.len() != right.len() {
        return Err(crate::RuntimeError::Backend(format!(
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

fn silu_glu(gate: &[f32], up: &[f32]) -> Vec<f32> {
    gate.iter()
        .zip(up.iter())
        .map(|(gate, up)| silu_scalar(*gate) * *up)
        .collect()
}

fn silu_forward_in_place(values: &mut [f32]) {
    for value in values {
        *value = silu_scalar(*value);
    }
}

fn silu_scalar(value: f32) -> f32 {
    value / (1.0 + (-value).exp())
}

fn sigmoid(value: f32) -> f32 {
    1.0 / (1.0 + (-value).exp())
}

fn softplus(value: f32) -> f32 {
    if value > 20.0 {
        value
    } else {
        (1.0 + value.exp()).ln()
    }
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

fn l2_normalize_into(values: &[f32], epsilon: f32, output: &mut [f32]) {
    let norm = values
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt()
        .max(epsilon);
    for (out, value) in output.iter_mut().zip(values.iter().copied()) {
        *out = value / norm;
    }
}

fn delta_net_autoregressive_step_in_place(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    decay: f32,
    beta: f32,
    state: &mut [f32],
    norm_q: &mut [f32],
    norm_k: &mut [f32],
    kv_mem: &mut [f32],
    delta: &mut [f32],
    output: &mut [f32],
) {
    let head_dim = q.len();
    l2_normalize_into(q, 1e-6, &mut norm_q[..head_dim]);
    let scale = (head_dim as f32).sqrt().recip();
    for value in &mut norm_q[..head_dim] {
        *value *= scale;
    }
    l2_normalize_into(k, 1e-6, &mut norm_k[..head_dim]);

    for value in state.iter_mut() {
        *value *= decay;
    }
    for row in 0..head_dim {
        let row_slice = &state[row * head_dim..(row + 1) * head_dim];
        kv_mem[row] = dot(row_slice, &norm_k[..head_dim]);
    }
    for row in 0..head_dim {
        delta[row] = (v[row] - kv_mem[row]) * beta;
    }
    for row in 0..head_dim {
        let row_delta = delta[row];
        for column in 0..head_dim {
            state[row * head_dim + column] += row_delta * norm_k[column];
        }
    }
    for row in 0..head_dim {
        let row_slice = &state[row * head_dim..(row + 1) * head_dim];
        output[row] = dot(row_slice, &norm_q[..head_dim]);
    }
}

fn causal_depthwise_conv1d_step_in_place(
    input: &[f32],
    state: &mut [f32],
    weights: &DenseMatrix,
    kernel_size: usize,
    output: &mut [f32],
) -> Result<(), ReferenceTextGenerationError> {
    if weights.columns != kernel_size || weights.rows != input.len() {
        return Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(format!(
                "qwen35 conv1d shape mismatch: weights=[{}, {}] input={}",
                weights.rows,
                weights.columns,
                input.len()
            )),
        ));
    }
    let state_tokens = kernel_size.saturating_sub(1);
    if state.len() != input.len().saturating_mul(state_tokens) {
        return Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(format!(
                "qwen35 conv1d state length mismatch: expected {}, actual {}",
                input.len().saturating_mul(state_tokens),
                state.len()
            )),
        ));
    }
    if output.len() != input.len() {
        return Err(ReferenceTextGenerationError::Runtime(
            crate::RuntimeError::Backend(format!(
                "qwen35 conv1d output length mismatch: expected {}, actual {}",
                input.len(),
                output.len()
            )),
        ));
    }
    for row in 0..input.len() {
        let row_state = &state[row * state_tokens..(row + 1) * state_tokens];
        let row_weights = &weights.values[row * kernel_size..(row + 1) * kernel_size];
        output[row] =
            dot(row_state, &row_weights[..state_tokens]) + input[row] * row_weights[state_tokens];
    }
    if state_tokens > 0 {
        for row in 0..input.len() {
            let row_state = &mut state[row * state_tokens..(row + 1) * state_tokens];
            row_state.rotate_left(1);
            row_state[state_tokens - 1] = input[row];
        }
    }
    Ok(())
}

fn attend_full_attention(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    cache: &[Qwen35FullAttentionEntry],
    head_count: usize,
    kv_head_count: usize,
    head_dim: usize,
    attention_scale: f32,
    sliding_window: Option<usize>,
) -> Vec<f32> {
    let group_size = head_count / kv_head_count.max(1);
    let mut output = vec![0.0_f32; head_count.saturating_mul(head_dim)];
    let cache = if let Some(window) = sliding_window {
        let start = cache.len().saturating_sub(window);
        &cache[start..]
    } else {
        cache
    };
    for head_index in 0..head_count {
        let kv_head_index = (head_index / group_size.max(1)).min(kv_head_count.saturating_sub(1));
        let q = &query[head_index * head_dim..(head_index + 1) * head_dim];
        let current_key = &key[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];
        let current_value = &value[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];
        let mut logits = Vec::with_capacity(cache.len().saturating_add(1));
        for entry in cache {
            let cached_key = &entry.key[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];
            logits.push(dot(q, cached_key) * attention_scale);
        }
        logits.push(dot(q, current_key) * attention_scale);
        let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut weights = logits
            .iter()
            .map(|logit| (logit - max_logit).exp())
            .collect::<Vec<_>>();
        let denom = weights.iter().copied().sum::<f32>().max(f32::MIN_POSITIVE);
        for weight in &mut weights {
            *weight /= denom;
        }
        let destination = &mut output[head_index * head_dim..(head_index + 1) * head_dim];
        for (entry_index, entry) in cache.iter().enumerate() {
            let cached_value =
                &entry.value[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];
            axpy(destination, cached_value, weights[entry_index]);
        }
        axpy(destination, current_value, *weights.last().unwrap_or(&0.0));
    }
    output
}

fn apply_rope_neox_mrope(
    values: &mut [f32],
    head_count: usize,
    head_dim: usize,
    rotary_dim: usize,
    position: [usize; 3],
    metadata: &GgufDecoderFamilyMetadata,
) {
    let rotary_dim = rotary_dim.min(head_dim).max(2);
    let (freq_scale, ext_factor, corr_dims, theta_scale) =
        qwen35_rope_runtime_parameters(rotary_dim, metadata);
    let mrope_sections = qwen35_mrope_sections(metadata);
    let is_imrope = qwen35_mrope_interleaved(metadata);
    let position_ids = [
        position[0] as f32,
        position[1] as f32,
        position[2] as f32,
        0.0_f32,
    ];
    for head_index in 0..head_count {
        let head_base = head_index.saturating_mul(head_dim);
        for i0 in (0..rotary_dim).step_by(2) {
            let pair = i0 / 2;
            let index0 = head_base + pair;
            let index1 = head_base + pair + rotary_dim / 2;
            if index1 >= head_base + head_dim || index1 >= values.len() {
                continue;
            }
            let theta_base = if let Some(sections) = mrope_sections {
                qwen35_mrope_theta_base(pair, sections, is_imrope, position_ids)
                    * theta_scale.powf(pair as f32)
            } else {
                position[0] as f32 * theta_scale.powf(pair as f32)
            };
            let (cos_theta, sin_theta) =
                rope_yarn(theta_base, freq_scale, corr_dims, i0, ext_factor, 1.0);
            let x0 = values[index0];
            let x1 = values[index1];
            values[index0] = x0 * cos_theta - x1 * sin_theta;
            values[index1] = x0 * sin_theta + x1 * cos_theta;
        }
    }
}

fn qwen35_mrope_theta_base(
    pair: usize,
    sections: [usize; 4],
    is_imrope: bool,
    position_ids: [f32; 4],
) -> f32 {
    let section_dims = sections.iter().copied().sum::<usize>();
    if section_dims == 0 {
        return position_ids[0];
    }
    let sector = pair % section_dims;
    if is_imrope {
        if sector % 3 == 1 && sector < 3 * sections[1] {
            position_ids[1]
        } else if sector % 3 == 2 && sector < 3 * sections[2] {
            position_ids[2]
        } else if sector % 3 == 0 && sector < 3 * sections[0] {
            position_ids[0]
        } else {
            position_ids[3]
        }
    } else {
        let sec_w = sections[0].saturating_add(sections[1]);
        let sec_e = sec_w.saturating_add(sections[2]);
        if sector < sections[0] {
            position_ids[0]
        } else if sector < sec_w {
            position_ids[1]
        } else if sector < sec_e {
            position_ids[2]
        } else {
            position_ids[3]
        }
    }
}

fn rope_yarn_corr_dims(n_dims: usize, n_ctx_orig: usize, freq_base: f32) -> [f32; 2] {
    let corr_dim = |n_rot: f32| {
        n_dims as f32
            * ((n_ctx_orig as f32 / (n_rot * 2.0 * std::f32::consts::PI)).ln()
                / (2.0 * freq_base.ln()))
    };
    let start = corr_dim(32.0).floor().max(0.0);
    let end = corr_dim(1.0).ceil().min(n_dims.saturating_sub(1) as f32);
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
    use super::{
        QWEN38_CUDA_PREFLIGHT_RUNTIME_RESERVE_BYTES, Qwen38MetalProjectionRow,
        qwen35_mrope_theta_base, qwen38_cuda_preflight, qwen38_cuda_unsupported_quantization_modes,
        qwen38_metal_projection_preflight, restore_qwen38_rejected_draft,
    };
    use psionic_backend_cuda::CudaDeviceMemoryInfo;
    use psionic_core::QuantizationMode;

    #[test]
    fn qwen38_cuda_preflight_refuses_total_or_current_free_memory_shortfall() {
        let artifact_bytes = 13_441_059_904;
        let required = artifact_bytes + QWEN38_CUDA_PREFLIGHT_RUNTIME_RESERVE_BYTES;
        let total_error = qwen38_cuda_preflight(
            artifact_bytes,
            CudaDeviceMemoryInfo {
                free_bytes: required,
                total_bytes: required - 1,
            },
        )
        .expect_err("total-memory shortfall must be refused");
        assert!(total_error.to_string().contains("total bytes"));

        let free_error = qwen38_cuda_preflight(
            artifact_bytes,
            CudaDeviceMemoryInfo {
                free_bytes: required - 1,
                total_bytes: required + 1,
            },
        )
        .expect_err("live free-memory shortfall must be refused");
        assert!(free_error.to_string().contains("cudaMemGetInfo"));
    }

    #[test]
    fn qwen38_cuda_preflight_refuses_unsupported_quantization_modes() {
        assert!(
            qwen38_cuda_unsupported_quantization_modes(&[
                QuantizationMode::None,
                QuantizationMode::GgmlQ3K,
                QuantizationMode::GgmlQ5K,
                QuantizationMode::GgmlIq3S,
                QuantizationMode::GgmlIq4Xs,
            ])
            .is_empty()
        );
        assert_eq!(
            qwen38_cuda_unsupported_quantization_modes(&[
                QuantizationMode::GgmlQ3K,
                QuantizationMode::GgmlQ4_0,
                QuantizationMode::Int8Symmetric,
            ]),
            vec![QuantizationMode::GgmlQ4_0, QuantizationMode::Int8Symmetric,]
        );
    }

    #[test]
    fn qwen38_metal_preflight_admits_native_modes_and_refuses_host_projection_fallback() {
        let admitted = qwen38_metal_projection_preflight(&[
            Qwen38MetalProjectionRow {
                name: String::from("output.weight"),
                mode: QuantizationMode::GgmlQ6K,
                byte_length: 4096,
            },
            Qwen38MetalProjectionRow {
                name: String::from("blk.0.ffn_gate.weight"),
                mode: QuantizationMode::GgmlQ4K,
                byte_length: 2048,
            },
        ])
        .expect("native Metal quantization should be admitted");
        assert_eq!(admitted.projection_count, 2);
        assert_eq!(admitted.projected_device_weight_bytes, 6144);

        let error = qwen38_metal_projection_preflight(&[
            Qwen38MetalProjectionRow {
                name: String::from("blk.0.attn_q.weight"),
                mode: QuantizationMode::GgmlQ4_0,
                byte_length: 1024,
            },
            Qwen38MetalProjectionRow {
                name: String::from("blk.0.attn_k.weight"),
                mode: QuantizationMode::None,
                byte_length: 2048,
            },
        ])
        .expect_err("unsupported or dense host projections must be refused");
        let message = error.to_string();
        assert!(message.contains("blk.0.attn_q.weight=GgmlQ4_0"));
        assert!(message.contains("blk.0.attn_k.weight=None"));
        assert!(message.contains("without a native kernel or admitted conversion"));
    }

    #[test]
    fn qwen38_mtp_rollback_handles_accept_all_reject_all_and_partial_rejection() {
        fn run_script(drafts: &[u32], verifier_tokens: &[u32]) -> (Vec<u32>, usize, usize, bool) {
            let mut state = Vec::new();
            let mut accepted = 0usize;
            let mut rollbacks = 0usize;
            let mut restored = true;
            for (index, (&draft, &verifier)) in
                drafts.iter().zip(verifier_tokens.iter()).enumerate()
            {
                let accepted_prefix = 100 + index as u32;
                let snapshot = state.clone();
                state.push(accepted_prefix);
                let verified_prefix_state = state.clone();
                let verified_output = verifier;
                state.push(draft);
                if draft == verifier {
                    accepted += 1;
                    continue;
                }
                rollbacks += 1;
                let (_, parity) = restore_qwen38_rejected_draft(
                    &mut state,
                    snapshot,
                    &verified_prefix_state,
                    &verified_output,
                    |state| {
                        state.push(accepted_prefix);
                        Ok::<u32, ()>(verifier)
                    },
                )
                .expect("scripted replay");
                restored &= parity;
            }
            (state, accepted, rollbacks, restored)
        }

        let (accept_state, accepted, rollbacks, restored) = run_script(&[7, 8, 9], &[7, 8, 9]);
        assert_eq!((accepted, rollbacks, restored), (3, 0, true));
        assert_eq!(accept_state.len(), 6);

        let (reject_state, accepted, rollbacks, restored) = run_script(&[70, 80, 90], &[7, 8, 9]);
        assert_eq!((accepted, rollbacks, restored), (0, 3, true));
        assert_eq!(reject_state, vec![100, 101, 102]);

        let (partial_state, accepted, rollbacks, restored) = run_script(&[7, 80, 9], &[7, 8, 9]);
        assert_eq!((accepted, rollbacks, restored), (2, 1, true));
        assert_eq!(partial_state, vec![100, 7, 101, 102, 9]);
    }

    #[test]
    fn qwen38_interleaved_mrope_selects_temporal_height_and_width_positions() {
        let positions = [2.0, 3.0, 4.0, 0.0];
        let sections = [11, 11, 10, 0];
        assert_eq!(qwen35_mrope_theta_base(0, sections, true, positions), 2.0);
        assert_eq!(qwen35_mrope_theta_base(1, sections, true, positions), 3.0);
        assert_eq!(qwen35_mrope_theta_base(2, sections, true, positions), 4.0);
        assert_eq!(qwen35_mrope_theta_base(29, sections, true, positions), 4.0);
        assert_eq!(qwen35_mrope_theta_base(30, sections, true, positions), 2.0);
        assert_eq!(qwen35_mrope_theta_base(31, sections, true, positions), 3.0);
    }
}
