use std::{
    cmp::Ordering,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use psionic_backend_cpu::{
    CpuBackend, decode_quantized_row_into, quantized_row_byte_len, quantized_row_dot,
};
use psionic_backend_cuda::{
    CudaBackend, CudaBuffer, CudaQuantizedMatvecStats, CudaSubmissionReport,
    TEXT_GENERATION_SUPPORTED_OPS as CUDA_TEXT_GENERATION_SUPPORTED_OPS, ggml_q8_1_storage_bytes,
};
use psionic_catalog::LocalBlobOpenOptions;
use psionic_models::{GgufBlobArtifact, GptOssTokenizer, PagedTensorStorage};
use psionic_runtime::{
    CacheAction, CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature,
    DeviceDiscovery, HealthStatus,
};
use sha2::{Digest, Sha256};

use super::{
    BackendHealthTracker, CompiledWordGenerationModel, DecodeStrategy, DecoderModelDescriptor,
    GenerationEventStream, GenerationModelHandle, GenerationOptions, GenerationResponse,
    GenerationStreamEvent, GenerationStreamStatus, GenerationStreamTerminal,
    GgufDecoderAdapterLoader, GgufDecoderFamily, GgufDecoderFamilyMetadata,
    GgufDecoderLayerTensorLayout, GptOssPerformanceMetrics, InMemoryGenerationModelRegistry,
    InMemoryGenerationSessionStore, LoadedModelRegistryError, LoadedModelView,
    LocalRuntimeObservability, ManagedTextGenerationRuntime, ModelLoadError, QuantizationMode,
    ReferenceTextGenerationError, SharedPrefixStore, TextGenerationExecutor, TokenId,
    TokenSequence, TokenizerBoundary, current_time_millis, default_prefix_cache_policy,
    generation_runtime_observability, prefix_compatibility, run_generation_request,
};
use thiserror::Error;

const GPT_OSS_OAI_SWIGLU_ALPHA: f32 = 1.702;
const GPT_OSS_OAI_SWIGLU_LIMIT: f32 = 7.0;
const GPT_OSS_YARN_BETA_FAST: f32 = 32.0;
const GPT_OSS_YARN_BETA_SLOW: f32 = 1.0;
const GPT_OSS_CPU_BACKEND: &str = "cpu";
const GPT_OSS_CUDA_BACKEND: &str = "cuda";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CudaStepOutputMode {
    FullLogits,
    DeviceArgmax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GptOssDecodeGraphNodeKind {
    AttnNorm,
    AttnQkv,
    AttnQRope,
    AttnKRope,
    AttnOut,
    FfnInp,
    AttnPostNorm,
    FfnMoeTopk,
    FfnMoeGateUp,
    FfnMoeDown,
    FfnMoeOut,
    ResultNorm,
    ResultOutput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GptOssDecodeGraphNode {
    kind: GptOssDecodeGraphNodeKind,
    name: &'static str,
}

impl GptOssDecodeGraphNode {
    const fn new(kind: GptOssDecodeGraphNodeKind, name: &'static str) -> Self {
        Self { kind, name }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GptOssDecodeGraph {
    layer_nodes: Vec<Vec<GptOssDecodeGraphNode>>,
    terminal_nodes: Vec<GptOssDecodeGraphNode>,
}

impl GptOssDecodeGraph {
    fn node_count(&self) -> usize {
        self.layer_nodes.iter().map(Vec::len).sum::<usize>() + self.terminal_nodes.len()
    }

    fn layer_node_count(&self) -> usize {
        self.layer_nodes.first().map_or(0, Vec::len)
    }

    fn signature_key(&self) -> String {
        let mut names = Vec::with_capacity(self.node_count());
        for layer in &self.layer_nodes {
            for node in layer {
                names.push(node.name);
            }
        }
        for node in &self.terminal_nodes {
            names.push(node.name);
        }
        names.join("|")
    }
}

fn duration_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

fn can_use_cuda_argmax_fast_path(options: &GenerationOptions) -> bool {
    options.decode_strategy == DecodeStrategy::Greedy
        && options.repeat_penalty.is_none()
        && options.presence_penalty.is_none()
        && options.frequency_penalty.is_none()
}

fn gpt_oss_layer_decode_graph_nodes() -> Vec<GptOssDecodeGraphNode> {
    vec![
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnNorm, "attn_norm"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnQkv, "attn_qkv"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnQRope, "attn_q_rope"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnKRope, "attn_k_rope"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnOut, "attn_out"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnInp, "ffn_inp"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnPostNorm, "attn_post_norm"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeTopk, "ffn_moe_topk"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeGateUp, "ffn_moe_gate_up"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeDown, "ffn_moe_down"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeOut, "ffn_moe_out"),
    ]
}

fn build_gpt_oss_decode_graph(layer_count: usize) -> GptOssDecodeGraph {
    GptOssDecodeGraph {
        layer_nodes: (0..layer_count)
            .map(|_| gpt_oss_layer_decode_graph_nodes())
            .collect(),
        terminal_nodes: vec![
            GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::ResultNorm, "result_norm"),
            GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::ResultOutput, "result_output"),
        ],
    }
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
pub enum CudaGptOssTextGenerationError {
    #[error("cuda backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        status: HealthStatus,
        message: String,
    },
    #[error(transparent)]
    Generation(#[from] ReferenceTextGenerationError),
}

pub struct CudaGgufGptOssTextGenerationService {
    backend: CudaBackend,
    backend_selection: psionic_runtime::BackendSelection,
    models: InMemoryGenerationModelRegistry<CudaGgufGptOssGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
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
        let mut compile_path = None;
        let mut kernel_count = 0usize;
        let mut bytes_moved = 0u64;
        let mut plan_cache_hits = 0usize;
        let mut plan_cache_misses = 0usize;
        let mut gpt_oss_perf: Option<GptOssPerformanceMetrics> = None;
        let mut decode_step_plan = None;
        let use_cuda_argmax_fast_path = can_use_cuda_argmax_fast_path(&request.options);
        let mut cache = if shared_prefix_eligible {
            let lookup = shared_prefixes.lookup(&compatibility, &prompt_tokens);
            prefix_state = lookup.state;
            prefix_tokens_reused = lookup.reused_tokens;
            prefix_identity = lookup.identity;
            prompt_logits = lookup.prompt_logits;
            last_logits = prompt_logits.last().cloned().unwrap_or_default();
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
        let mut cuda_cache = CudaKvCacheMirror::from_host_cache(
            backend,
            &cache,
            request.options.max_output_tokens.saturating_add(1),
        )?;
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
        let prompt_cache = cache.clone();

        let mut sampler = super::GenerationSampler::new(&request.options);
        let mut generated_tokens = Vec::new();
        let mut pending_token = sampler
            .select_next_token_from_history(&last_logits, &token_history)
            .ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?;
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
            kernel_count = kernel_count.saturating_add(step.kernel_count);
            bytes_moved = bytes_moved.saturating_add(step.bytes_moved);
            super::accumulate_optional_gpt_oss_perf(&mut gpt_oss_perf, step.perf.as_ref());
            if !step.key.is_empty() {
                cache.append(pending_token, step.key.clone(), step.value.clone())?;
            }
            token_history.push(pending_token.as_u32());

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
                pending_token = token;
            } else {
                last_logits = step.logits;
                pending_token = sampler
                    .select_next_token_from_history(&last_logits, &token_history)
                    .ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?;
            }
        };

        if shared_prefix_eligible {
            let recorded_identity = shared_prefixes.record(
                compatibility,
                &prompt_tokens,
                &prompt_logits,
                &prompt_cache,
            );
            if prefix_state != super::PrefixCacheState::Hit || prefix_identity.is_none() {
                prefix_identity = Some(recorded_identity);
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
            execution_plan_digest: delivery_plan_digest.clone(),
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

impl From<ModelLoadError> for CudaGptOssTextGenerationError {
    fn from(value: ModelLoadError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<LoadedModelRegistryError> for CudaGptOssTextGenerationError {
    fn from(value: LoadedModelRegistryError) -> Self {
        Self::Generation(ReferenceTextGenerationError::from(value))
    }
}

impl From<super::SessionStoreError> for CudaGptOssTextGenerationError {
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
        let artifact = GgufBlobArtifact::open_path(path, LocalBlobOpenOptions::default())?;
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
pub struct CudaGgufGptOssGenerationModel {
    inner: Arc<GptOssCudaModelInner>,
}

impl CudaGgufGptOssGenerationModel {
    pub fn from_gguf_path(
        path: impl AsRef<Path>,
        backend: &mut CudaBackend,
    ) -> Result<Self, CudaGptOssTextGenerationError> {
        let artifact = GgufBlobArtifact::open_path(path, LocalBlobOpenOptions::default())?;
        Self::from_blob_artifact(artifact, backend)
    }

    fn from_blob_artifact(
        artifact: GgufBlobArtifact,
        backend: &mut CudaBackend,
    ) -> Result<Self, CudaGptOssTextGenerationError> {
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
            load_cuda_quantized_matrix(backend, &artifact, output)?
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
            .map(|layer| GptOssCudaLayer::load(backend, &artifact, layer))
            .collect::<Result<Vec<_>, _>>()?;
        let output_norm =
            load_dense_vector(&artifact, adapter.tensor_layout().output_norm.as_str())?;
        let output_norm_device = upload_cuda_f32_buffer(
            backend,
            adapter.tensor_layout().output_norm.as_str(),
            output_norm.as_slice(),
        )?;
        let inner = GptOssCudaModelInner {
            descriptor: adapter.descriptor().clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            decode_graph: build_gpt_oss_decode_graph(adapter.tensor_layout().layers.len()),
            token_embedding: load_quantized_matrix(
                &artifact,
                adapter.tensor_layout().token_embedding.as_str(),
            )?,
            output_norm,
            output_norm_device,
            output,
            layers,
            plan_digest: digest_gpt_oss_plan(
                adapter.descriptor(),
                adapter.family_metadata(),
                GPT_OSS_CUDA_BACKEND,
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
    output_norm: Vec<f32>,
    output_norm_device: CudaBuffer,
    output: CudaQuantizedMatrix,
    layers: Vec<GptOssCudaLayer>,
    plan_digest: String,
    decode_step_plan: Mutex<Option<GptOssCudaStepPlan>>,
    load_duration_ns: u64,
}

#[derive(Clone, Debug)]
struct GptOssCudaStepPlanLayer {
    hidden_norm_buffer: CudaBuffer,
    qkv_buffer: CudaBuffer,
    attention_buffer: CudaBuffer,
    projected_buffer: CudaBuffer,
    ffn_norm_buffer: CudaBuffer,
    selected_ids_buffer: CudaBuffer,
    selected_weights_buffer: CudaBuffer,
    activated_buffer: CudaBuffer,
    activated_q8_1_buffer: CudaBuffer,
    moe_buffer: CudaBuffer,
}

#[derive(Clone, Debug)]
struct GptOssCudaStepPlan {
    digest: String,
    hidden_buffer: CudaBuffer,
    vector_q8_1_buffer: CudaBuffer,
    layers: Vec<GptOssCudaStepPlanLayer>,
    final_norm_buffer: CudaBuffer,
    logits_buffer: CudaBuffer,
    next_token_buffer: CudaBuffer,
}

impl GptOssCudaModelInner {
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
                selected_ids_buffer: backend.i32_buffer(selected_count)?,
                selected_weights_buffer: backend.f32_buffer(selected_count)?,
                activated_buffer: backend.f32_buffer(selected_count.saturating_mul(gate_rows))?,
                activated_q8_1_buffer: backend.byte_buffer(&vec![0_u8; activated_q8_1_bytes])?,
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
            vector_q8_1_buffer: backend.byte_buffer(&vec![0_u8; vector_q8_1_bytes])?,
            layers,
            final_norm_buffer: backend.f32_buffer(hidden_size)?,
            logits_buffer: backend.f32_buffer(self.output.rows)?,
            next_token_buffer: backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?,
        })
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
        let hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
        perf.stage_timings.token_embedding_ns = perf
            .stage_timings
            .token_embedding_ns
            .saturating_add(duration_ns(token_embedding_start));
        bytes_moved = bytes_moved.saturating_add(self.token_embedding.byte_length() as u64);
        let hidden_upload_bytes = hidden.len().saturating_mul(std::mem::size_of::<f32>());
        perf.cuda.host_to_device_bytes = perf
            .cuda
            .host_to_device_bytes
            .saturating_add(hidden_upload_bytes.try_into().unwrap_or(u64::MAX));
        plan.hidden_buffer.write_f32(hidden.as_slice())?;

        let cache_write_index = cuda_cache.len();
        cuda_cache.ensure_capacity(backend, cache_write_index.saturating_add(1))?;
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

        let mut submission = backend.begin_submission()?;
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
            let v_rows = layer.attention_qkv_weight.rows_per_projection[2];
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
            submission.rms_norm(
                current_hidden,
                &layer.attention_norm_device,
                &layer_plan.hidden_norm_buffer,
                hidden_size,
                self.family_metadata.rms_norm_epsilon,
            )?;
            if layer.attention_qkv_weight.mode == QuantizationMode::GgmlQ8_0 {
                submission.quantize_f32_to_q8_1(
                    &layer_plan.hidden_norm_buffer,
                    1,
                    hidden_size,
                    &plan.vector_q8_1_buffer,
                )?;
                submission.quantized_matvec_q8_1(
                    &layer.attention_qkv_weight.storage,
                    0,
                    layer.attention_qkv_weight.mode,
                    layer.attention_qkv_weight.total_rows(),
                    layer.attention_qkv_weight.columns,
                    &plan.vector_q8_1_buffer,
                    &layer_plan.qkv_buffer,
                )?;
            } else {
                submission.quantized_matvec(
                    &layer.attention_qkv_weight.storage,
                    0,
                    layer.attention_qkv_weight.mode,
                    layer.attention_qkv_weight.total_rows(),
                    layer.attention_qkv_weight.columns,
                    &layer_plan.hidden_norm_buffer,
                    &layer_plan.qkv_buffer,
                )?;
            }
            submission.add_f32_in_place(
                &layer_plan.qkv_buffer,
                0,
                &layer.attention_qkv_bias_device,
                layer.attention_qkv_weight.total_rows(),
            )?;
            submission.rope_neox_in_place(
                &layer_plan.qkv_buffer,
                0,
                head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
            )?;
            submission.rope_neox_in_place(
                &layer_plan.qkv_buffer,
                q_rows,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
            )?;
            submission.attention_decode(
                &layer_plan.qkv_buffer,
                0,
                &layer_plan.qkv_buffer,
                q_rows,
                &layer_plan.qkv_buffer,
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
                layer.attention_sinks_device.as_ref(),
                &layer_plan.attention_buffer,
            )?;
            submission.copy_buffer_region(
                &layer_plan.qkv_buffer,
                q_rows.saturating_mul(std::mem::size_of::<f32>()),
                &cuda_cache.key_buffer,
                cache_write_index
                    .saturating_mul(cuda_cache.width)
                    .saturating_add(layer_offset)
                    .saturating_mul(std::mem::size_of::<f32>()),
                k_rows.saturating_mul(std::mem::size_of::<f32>()),
            )?;
            submission.copy_buffer_region(
                &layer_plan.qkv_buffer,
                q_rows
                    .saturating_add(k_rows)
                    .saturating_mul(std::mem::size_of::<f32>()),
                &cuda_cache.value_buffer,
                cache_write_index
                    .saturating_mul(cuda_cache.width)
                    .saturating_add(layer_offset)
                    .saturating_mul(std::mem::size_of::<f32>()),
                v_rows.saturating_mul(std::mem::size_of::<f32>()),
            )?;
            if layer.attention_output_weight.mode == QuantizationMode::GgmlQ8_0 {
                submission.quantize_f32_to_q8_1(
                    &layer_plan.attention_buffer,
                    1,
                    layer.attention_output_weight.columns,
                    &plan.vector_q8_1_buffer,
                )?;
                submission.quantized_matvec_q8_1(
                    &layer.attention_output_weight.storage,
                    0,
                    layer.attention_output_weight.mode,
                    layer.attention_output_weight.rows,
                    layer.attention_output_weight.columns,
                    &plan.vector_q8_1_buffer,
                    &layer_plan.projected_buffer,
                )?;
            } else {
                submission.quantized_matvec(
                    &layer.attention_output_weight.storage,
                    0,
                    layer.attention_output_weight.mode,
                    layer.attention_output_weight.rows,
                    layer.attention_output_weight.columns,
                    &layer_plan.attention_buffer,
                    &layer_plan.projected_buffer,
                )?;
            }
            if let Some(bias) = layer.attention_output_bias_device.as_ref() {
                submission.add_f32_in_place(
                    &layer_plan.projected_buffer,
                    0,
                    bias,
                    layer.attention_output_weight.rows,
                )?;
            }
            submission.add_f32_in_place(
                &layer_plan.projected_buffer,
                0,
                current_hidden,
                hidden_size,
            )?;
            submission.rms_norm(
                &layer_plan.projected_buffer,
                &layer.feed_forward_norm_device,
                &layer_plan.ffn_norm_buffer,
                hidden_size,
                self.family_metadata.rms_norm_epsilon,
            )?;
            submission.router_topk_softmax(
                &layer.feed_forward_router_weight_device,
                layer.feed_forward_router_bias_device.as_ref(),
                &layer_plan.ffn_norm_buffer,
                layer.feed_forward_router_weight.rows,
                layer.feed_forward_router_weight.columns,
                selected_count,
                &layer_plan.selected_ids_buffer,
                &layer_plan.selected_weights_buffer,
            )?;
            if layer.feed_forward_gate_up_experts_weight.mode == QuantizationMode::GgmlQ8_0
                && layer.feed_forward_down_experts_weight.mode == QuantizationMode::GgmlQ8_0
            {
                submission.quantize_f32_to_q8_1(
                    &layer_plan.ffn_norm_buffer,
                    1,
                    layer.feed_forward_gate_up_experts_weight.columns,
                    &plan.vector_q8_1_buffer,
                )?;
                submission.moe_gate_up_swiglu_q8_1(
                    &layer.feed_forward_gate_up_experts_weight.storage,
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
                submission.quantize_f32_to_q8_1(
                    &layer_plan.activated_buffer,
                    selected_count,
                    layer.feed_forward_down_experts_weight.columns,
                    &layer_plan.activated_q8_1_buffer,
                )?;
                submission.moe_down_aggregate_q8_1(
                    &layer.feed_forward_down_experts_weight.storage,
                    layer.feed_forward_down_experts_weight.mode,
                    layer.feed_forward_down_experts_weight.row_byte_len,
                    layer.feed_forward_down_experts_weight.rows,
                    layer.feed_forward_down_experts_weight.columns,
                    &layer_plan.selected_ids_buffer,
                    &layer_plan.selected_weights_buffer,
                    selected_count,
                    &layer_plan.activated_q8_1_buffer,
                    layer.feed_forward_down_experts_bias_device.as_ref(),
                    &layer_plan.moe_buffer,
                )?;
            } else {
                submission.moe_gate_up_swiglu(
                    &layer.feed_forward_gate_up_experts_weight.storage,
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
                    &layer.feed_forward_down_experts_weight.storage,
                    layer.feed_forward_down_experts_weight.mode,
                    layer.feed_forward_down_experts_weight.row_byte_len,
                    layer.feed_forward_down_experts_weight.rows,
                    layer.feed_forward_down_experts_weight.columns,
                    &layer_plan.selected_ids_buffer,
                    &layer_plan.selected_weights_buffer,
                    selected_count,
                    &layer_plan.activated_buffer,
                    layer.feed_forward_down_experts_bias_device.as_ref(),
                    &layer_plan.moe_buffer,
                )?;
            }
            submission.add_f32_in_place(
                &layer_plan.moe_buffer,
                0,
                &layer_plan.projected_buffer,
                hidden_size,
            )?;
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

            bytes_moved = bytes_moved
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
        submission.rms_norm(
            final_hidden,
            &self.output_norm_device,
            &plan.final_norm_buffer,
            hidden_size,
            self.family_metadata.rms_norm_epsilon,
        )?;
        if self.output.mode == QuantizationMode::GgmlQ8_0 {
            submission.quantize_f32_to_q8_1(
                &plan.final_norm_buffer,
                1,
                self.output.columns,
                &plan.vector_q8_1_buffer,
            )?;
            submission.quantized_matvec_q8_1(
                &self.output.storage,
                0,
                self.output.mode,
                self.output.rows,
                self.output.columns,
                &plan.vector_q8_1_buffer,
                &plan.logits_buffer,
            )?;
        } else {
            submission.quantized_matvec(
                &self.output.storage,
                0,
                self.output.mode,
                self.output.rows,
                self.output.columns,
                &plan.final_norm_buffer,
                &plan.logits_buffer,
            )?;
        }
        if output_mode == CudaStepOutputMode::DeviceArgmax {
            submission.argmax_f32(
                &plan.logits_buffer,
                1,
                self.output.rows,
                &plan.next_token_buffer,
            )?;
        }
        let submission_report =
            submission.commit(psionic_backend_cuda::CudaCommandWait::Completed)?;
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
                let bytes = plan.next_token_buffer.read_bytes()?;
                let token = i32::from_ne_bytes(bytes[..4].try_into().map_err(|_| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(
                        String::from("cuda argmax returned an invalid token buffer"),
                    ))
                })?);
                let token = u32::try_from(token).map(TokenId).map_err(|_| {
                    ReferenceTextGenerationError::Runtime(super::RuntimeError::Backend(format!(
                        "cuda argmax returned a negative token id {token}",
                    )))
                })?;
                (
                    Vec::new(),
                    Some(token),
                    std::mem::size_of::<i32>().try_into().unwrap_or(u64::MAX),
                )
            }
        };
        cuda_cache.len = cache_write_index.saturating_add(1);
        accumulate_cuda_submission_report(&mut perf, &submission_report, 0, logits_readback_bytes);
        perf.stage_timings.logits_projection_ns = perf
            .stage_timings
            .logits_projection_ns
            .saturating_add(duration_ns(logits_start));
        bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
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

    fn forward_step(
        &self,
        backend: &mut CudaBackend,
        token: TokenId,
        position: usize,
        cache: &super::InMemoryKvCache,
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
            let (mut qkv_outputs, qkv_stats) = layer
                .attention_qkv_weight
                .matvec_profiled(backend, hidden_norm.as_slice())?;
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
            let attention_out_stats = layer.attention_output_weight.matvec_profiled(
                backend,
                attention.as_slice(),
                &mut attention_out,
            )?;
            accumulate_cuda_matvec_stats(&mut perf, &attention_out_stats);
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
            hidden = add_vectors(moe_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_qkv_weight.byte_length() as u64)
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

        let logits_projection_start = Instant::now();
        let mut logits = Vec::new();
        let logits_stats =
            self.output
                .matvec_profiled(backend, final_hidden.as_slice(), &mut logits)?;
        accumulate_cuda_matvec_stats(&mut perf, &logits_stats);
        bytes_moved = bytes_moved.saturating_add(self.output.byte_length() as u64);
        kernel_count = kernel_count.saturating_add(1);
        perf.stage_timings.logits_projection_ns = perf
            .stage_timings
            .logits_projection_ns
            .saturating_add(duration_ns(logits_projection_start));

        Ok(GptOssForwardStep {
            key: cache_key,
            value: cache_value,
            logits,
            selected_token: None,
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }
}

#[derive(Clone, Debug)]
struct GptOssCudaLayer {
    attention_norm: Vec<f32>,
    attention_norm_device: CudaBuffer,
    attention_qkv_weight: CudaQuantizedProjectionGroup,
    attention_qkv_bias_device: CudaBuffer,
    attention_query_bias: Vec<f32>,
    attention_key_bias: Vec<f32>,
    attention_value_bias: Vec<f32>,
    attention_output_weight: CudaQuantizedMatrix,
    attention_output_bias: Option<Vec<f32>>,
    attention_output_bias_device: Option<CudaBuffer>,
    attention_sinks: Option<Vec<f32>>,
    attention_sinks_device: Option<CudaBuffer>,
    feed_forward_norm: Vec<f32>,
    feed_forward_norm_device: CudaBuffer,
    feed_forward_router_weight: DenseMatrix,
    feed_forward_router_weight_device: CudaBuffer,
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
    ) -> Result<Self, ModelLoadError> {
        let attention_norm = load_dense_vector(artifact, layout.attention_norm.as_str())?;
        let attention_norm_device = upload_cuda_f32_buffer(
            backend,
            layout.attention_norm.as_str(),
            attention_norm.as_slice(),
        )?;
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
        let attention_qkv_bias_device =
            upload_cuda_f32_buffer(backend, "attention_qkv_bias", attention_qkv_bias.as_slice())?;
        let attention_output_bias = layout
            .attention_output_bias
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let attention_output_bias_device = attention_output_bias
            .as_ref()
            .map(|values| {
                upload_cuda_f32_buffer(backend, "attention_output_bias", values.as_slice())
            })
            .transpose()?;
        let attention_sinks = layout
            .attention_sinks_weight
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let attention_sinks_device = attention_sinks
            .as_ref()
            .map(|values| upload_cuda_f32_buffer(backend, "attention_sinks", values.as_slice()))
            .transpose()?;
        let feed_forward_norm = load_dense_vector(
            artifact,
            required_tensor_name(layout.feed_forward_norm.as_ref(), "feed_forward_norm")?,
        )?;
        let feed_forward_norm_device =
            upload_cuda_f32_buffer(backend, "feed_forward_norm", feed_forward_norm.as_slice())?;
        let feed_forward_router_weight = load_dense_matrix(
            artifact,
            required_tensor_name(
                layout.feed_forward_router_weight.as_ref(),
                "feed_forward_router_weight",
            )?,
        )?;
        let feed_forward_router_weight_device = upload_cuda_f32_buffer(
            backend,
            "feed_forward_router_weight",
            feed_forward_router_weight.values.as_slice(),
        )?;
        let feed_forward_router_bias = layout
            .feed_forward_router_bias
            .as_ref()
            .map(|name| load_dense_vector(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_router_bias_device = feed_forward_router_bias
            .as_ref()
            .map(|values| {
                upload_cuda_f32_buffer(backend, "feed_forward_router_bias", values.as_slice())
            })
            .transpose()?;
        let feed_forward_gate_experts_bias = layout
            .feed_forward_gate_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_gate_experts_bias_device = feed_forward_gate_experts_bias
            .as_ref()
            .map(|values| {
                upload_cuda_f32_buffer(backend, "feed_forward_gate_experts_bias", values.as_slice())
            })
            .transpose()?;
        let feed_forward_up_experts_bias = layout
            .feed_forward_up_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_up_experts_bias_device = feed_forward_up_experts_bias
            .as_ref()
            .map(|values| {
                upload_cuda_f32_buffer(backend, "feed_forward_up_experts_bias", values.as_slice())
            })
            .transpose()?;
        let feed_forward_down_experts_bias = layout
            .feed_forward_down_experts_bias
            .as_ref()
            .map(|name| load_dense_rank2_flat(artifact, name.as_str()))
            .transpose()?;
        let feed_forward_down_experts_bias_device = feed_forward_down_experts_bias
            .as_ref()
            .map(|values| {
                upload_cuda_f32_buffer(backend, "feed_forward_down_experts_bias", values.as_slice())
            })
            .transpose()?;
        Ok(Self {
            attention_norm,
            attention_norm_device,
            attention_qkv_weight: load_cuda_quantized_projection_group(
                backend,
                artifact,
                &[
                    layout.attention_query_weight.as_str(),
                    layout.attention_key_weight.as_str(),
                    layout.attention_value_weight.as_str(),
                ],
            )?,
            attention_qkv_bias_device,
            attention_query_bias,
            attention_key_bias,
            attention_value_bias,
            attention_output_weight: load_cuda_quantized_matrix(
                backend,
                artifact,
                layout.attention_output_weight.as_str(),
            )?,
            attention_output_bias,
            attention_output_bias_device,
            attention_sinks,
            attention_sinks_device,
            feed_forward_norm,
            feed_forward_norm_device,
            feed_forward_router_weight,
            feed_forward_router_weight_device,
            feed_forward_router_bias,
            feed_forward_router_bias_device,
            feed_forward_gate_up_experts_weight: load_cuda_quantized_expert_projection_group(
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
            feed_forward_gate_experts_bias,
            feed_forward_gate_experts_bias_device,
            feed_forward_up_experts_bias,
            feed_forward_up_experts_bias_device,
            feed_forward_down_experts_weight: load_cuda_quantized_expert_tensor(
                backend,
                artifact,
                required_tensor_name(
                    layout.feed_forward_down_experts_weight.as_ref(),
                    "feed_forward_down_experts_weight",
                )?,
            )?,
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
struct CudaQuantizedMatrix {
    storage: CudaBuffer,
    mode: QuantizationMode,
    rows: usize,
    columns: usize,
}

impl CudaQuantizedMatrix {
    fn byte_length(&self) -> usize {
        self.storage.byte_len()
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
        let result = backend.quantized_matvec_profiled(
            &self.storage,
            self.mode,
            self.rows,
            self.columns,
            input,
        )?;
        *output = result.values;
        Ok(result.stats)
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
    storage: CudaBuffer,
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
        self.storage.byte_len()
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
        let result = backend.quantized_matvec_profiled(
            &self.storage,
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
struct CudaQuantizedExpertTensor {
    storage: CudaBuffer,
    mode: QuantizationMode,
    outer: usize,
    rows: usize,
    columns: usize,
    row_byte_len: usize,
}

impl CudaQuantizedExpertTensor {
    fn byte_length(&self) -> usize {
        self.storage.byte_len()
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
        let byte_offset = expert_index
            .saturating_mul(self.rows)
            .saturating_mul(self.row_byte_len);
        let result = backend.quantized_matvec_with_offset_profiled(
            &self.storage,
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
    storage: CudaBuffer,
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
        self.storage.byte_len()
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
        let byte_offset = expert_index
            .saturating_mul(self.total_rows())
            .saturating_mul(self.row_byte_len);
        let result = backend.quantized_matvec_with_offset_profiled(
            &self.storage,
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
        let mut key_buffer = backend.f32_buffer(capacity_tokens.saturating_mul(cache.width()))?;
        let mut value_buffer = backend.f32_buffer(capacity_tokens.saturating_mul(cache.width()))?;
        if !cache.is_empty() {
            let mut keys = Vec::with_capacity(cache.len().saturating_mul(cache.width()));
            let mut values = Vec::with_capacity(cache.len().saturating_mul(cache.width()));
            for entry in cache.entries() {
                keys.extend_from_slice(entry.key.as_slice());
                values.extend_from_slice(entry.value.as_slice());
            }
            key_buffer.write_f32_at_offset(0, keys.as_slice())?;
            value_buffer.write_f32_at_offset(0, values.as_slice())?;
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
        let mut new_keys = backend.f32_buffer(new_capacity.saturating_mul(self.width))?;
        let mut new_values = backend.f32_buffer(new_capacity.saturating_mul(self.width))?;
        if self.len > 0 {
            let existing_keys = self
                .key_buffer
                .read_f32_at_offset(0, self.len.saturating_mul(self.width))?;
            let existing_values = self
                .value_buffer
                .read_f32_at_offset(0, self.len.saturating_mul(self.width))?;
            new_keys.write_f32_at_offset(0, existing_keys.as_slice())?;
            new_values.write_f32_at_offset(0, existing_values.as_slice())?;
        }
        self.key_buffer = new_keys;
        self.value_buffer = new_values;
        self.capacity_tokens = new_capacity;
        Ok(())
    }

    fn read_entry(&self, token_index: usize) -> Result<(Vec<f32>, Vec<f32>), super::RuntimeError> {
        if token_index >= self.len {
            return Err(super::RuntimeError::Backend(format!(
                "cuda kv cache entry read exceeds logical length: index={} len={}",
                token_index, self.len
            )));
        }
        let element_offset = token_index.saturating_mul(self.width);
        Ok((
            self.key_buffer
                .read_f32_at_offset(element_offset, self.width)?,
            self.value_buffer
                .read_f32_at_offset(element_offset, self.width)?,
        ))
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

fn load_cuda_quantized_matrix(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
) -> Result<CudaQuantizedMatrix, ModelLoadError> {
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
    quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
        ModelLoadError::InvalidQuantizedTensorShape {
            quantization,
            shape: dims.clone(),
        }
    })?;
    let bytes = storage.bytes()?;
    Ok(CudaQuantizedMatrix {
        storage: backend
            .byte_buffer(bytes)
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to upload `{name}` to cuda: {error}"),
            })?,
        mode: quantization,
        rows: *rows,
        columns: *columns,
    })
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

fn load_cuda_quantized_expert_tensor(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    name: &str,
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
    let bytes = storage.bytes()?;
    Ok(CudaQuantizedExpertTensor {
        storage: backend
            .byte_buffer(bytes)
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to upload `{name}` to cuda: {error}"),
            })?,
        mode: quantization,
        outer: *outer,
        rows: *rows,
        columns: *columns,
        row_byte_len,
    })
}

fn load_cuda_quantized_projection_group(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
) -> Result<CudaQuantizedProjectionGroup, ModelLoadError> {
    let mut mode = None;
    let mut columns = None;
    let mut rows_per_projection = Vec::with_capacity(names.len());
    let mut projection_bytes = Vec::with_capacity(names.len());
    for name in names {
        let storage = artifact.paged_tensor(name)?;
        let metadata = storage.metadata();
        let tensor_name = metadata.name.clone();
        let dims = metadata.shape.dims().to_vec();
        let quantization = metadata.quantization;
        let layout = metadata.quantized_layout;
        let [rows, projection_columns] = dims.as_slice() else {
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
                        "packed cuda projection group requires matching quantization, `{name}` had {quantization:?} but expected {expected_mode:?}"
                    ),
                });
            }
        } else {
            mode = Some(quantization);
        }
        if let Some(expected_columns) = columns {
            if expected_columns != *projection_columns {
                return Err(ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: format!(
                        "packed cuda projection group requires matching input width, `{name}` had {projection_columns} but expected {expected_columns}"
                    ),
                });
            }
        } else {
            columns = Some(*projection_columns);
        }
        rows_per_projection.push(*rows);
        projection_bytes.push(storage.bytes()?.to_vec());
    }
    let packed = pack_quantized_projection_bytes(
        projection_bytes
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>()
            .as_slice(),
    );
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
    let storage =
        backend
            .byte_buffer(packed.as_slice())
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to upload packed cuda projection group `{}`: {error}",
                    names.join(", ")
                ),
            })?;
    Ok(CudaQuantizedProjectionGroup {
        storage,
        mode,
        rows_per_projection,
        columns,
    })
}

fn load_cuda_quantized_expert_projection_group(
    backend: &mut CudaBackend,
    artifact: &GgufBlobArtifact,
    names: &[&str],
) -> Result<CudaQuantizedExpertProjectionGroup, ModelLoadError> {
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
        projection_bytes.push(storage.bytes()?.to_vec());
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
    let storage =
        backend
            .byte_buffer(packed.as_slice())
            .map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!(
                    "failed to upload packed cuda expert projection group `{}`: {error}",
                    names.join(", ")
                ),
            })?;
    Ok(CudaQuantizedExpertProjectionGroup {
        storage,
        mode,
        outer,
        rows_per_projection,
        columns,
        row_byte_len,
    })
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

fn cache_key_value_byte_len(width: usize) -> u64 {
    width
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<f32>())
        .try_into()
        .unwrap_or(u64::MAX)
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
    use super::{
        build_gpt_oss_decode_graph, digest_gpt_oss_cuda_step_plan,
        pack_quantized_expert_projection_bytes, pack_quantized_projection_bytes,
        split_projection_outputs,
    };

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
    fn gpt_oss_decode_graph_matches_llama_cpp_high_level_order() {
        let graph = build_gpt_oss_decode_graph(2);
        let layer_names = graph.layer_nodes[0]
            .iter()
            .map(|node| node.name)
            .collect::<Vec<_>>();
        assert_eq!(
            layer_names,
            vec![
                "attn_norm",
                "attn_qkv",
                "attn_q_rope",
                "attn_k_rope",
                "attn_out",
                "ffn_inp",
                "attn_post_norm",
                "ffn_moe_topk",
                "ffn_moe_gate_up",
                "ffn_moe_down",
                "ffn_moe_out",
            ]
        );
        let terminal_names = graph
            .terminal_nodes
            .iter()
            .map(|node| node.name)
            .collect::<Vec<_>>();
        assert_eq!(terminal_names, vec!["result_norm", "result_output"]);
        assert_eq!(graph.layer_node_count(), 11);
        assert_eq!(graph.node_count(), 24);
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
}
