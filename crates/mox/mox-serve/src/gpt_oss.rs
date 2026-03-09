use std::{cmp::Ordering, path::Path, sync::Arc, time::Instant};

use mox_backend_cpu::{
    CpuBackend, decode_quantized_row_into, quantized_row_byte_len, quantized_row_dot,
};
use mox_backend_cuda::{
    CudaBackend, CudaBuffer, CudaQuantizedMatvecStats,
    TEXT_GENERATION_SUPPORTED_OPS as CUDA_TEXT_GENERATION_SUPPORTED_OPS,
};
use mox_catalog::LocalBlobOpenOptions;
use mox_models::{GgufBlobArtifact, GptOssTokenizer, PagedTensorStorage};
use mox_runtime::{DeviceDiscovery, HealthStatus};
use sha2::{Digest, Sha256};

use super::{
    BackendHealthTracker, CompiledWordGenerationModel, DecoderModelDescriptor,
    GenerationEventStream, GenerationModelHandle, GenerationResponse, GenerationStreamEvent,
    GenerationStreamStatus, GenerationStreamTerminal, GgufDecoderAdapterLoader,
    GgufDecoderFamily, GgufDecoderFamilyMetadata, GgufDecoderLayerTensorLayout,
    GptOssPerformanceMetrics, InMemoryGenerationModelRegistry,
    InMemoryGenerationSessionStore, LoadedModelRegistryError, LoadedModelView,
    LocalRuntimeObservability, ManagedTextGenerationRuntime, ModelLoadError, QuantizationMode,
    ReferenceTextGenerationError, SharedPrefixStore, TextGenerationExecutor, TokenId,
    TokenSequence, TokenizerBoundary, current_time_millis, generation_runtime_observability,
    run_generation_request,
};
use thiserror::Error;

const GPT_OSS_OAI_SWIGLU_ALPHA: f32 = 1.702;
const GPT_OSS_OAI_SWIGLU_LIMIT: f32 = 7.0;
const GPT_OSS_YARN_BETA_FAST: f32 = 32.0;
const GPT_OSS_YARN_BETA_SLOW: f32 = 1.0;
const GPT_OSS_CPU_BACKEND: &str = "cpu";
const GPT_OSS_CUDA_BACKEND: &str = "cuda";

fn duration_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
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
    backend_selection: mox_runtime::BackendSelection,
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
    pub fn backend_selection(&self) -> &mox_runtime::BackendSelection {
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
        let inner = GptOssCudaModelInner {
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
                GPT_OSS_CUDA_BACKEND,
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

#[derive(Clone, Debug)]
struct GptOssCudaModelInner {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GptOssTokenizer,
    token_embedding: QuantizedMatrix,
    output_norm: Vec<f32>,
    output: CudaQuantizedMatrix,
    layers: Vec<GptOssCudaLayer>,
    plan_digest: String,
    load_duration_ns: u64,
}

impl GptOssCudaModelInner {
    fn cache_width(&self) -> usize {
        self.descriptor
            .config
            .layer_count
            .saturating_mul(self.descriptor.config.kv_width())
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
            let q_stats = layer.attention_query_weight.matvec_profiled(
                backend,
                hidden_norm.as_slice(),
                &mut q,
            )?;
            accumulate_cuda_matvec_stats(&mut perf, &q_stats);
            add_bias_in_place(&mut q, layer.attention_query_bias.as_slice());

            let mut k = Vec::new();
            let k_stats = layer.attention_key_weight.matvec_profiled(
                backend,
                hidden_norm.as_slice(),
                &mut k,
            )?;
            accumulate_cuda_matvec_stats(&mut perf, &k_stats);
            add_bias_in_place(&mut k, layer.attention_key_bias.as_slice());

            let mut v = Vec::new();
            let v_stats = layer.attention_value_weight.matvec_profiled(
                backend,
                hidden_norm.as_slice(),
                &mut v,
            )?;
            accumulate_cuda_matvec_stats(&mut perf, &v_stats);
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
                let mut gate = Vec::new();
                let gate_stats = layer.feed_forward_gate_experts_weight.expert_matvec_profiled(
                    backend,
                    expert_index,
                    ffn_input.as_slice(),
                    &mut gate,
                )?;
                accumulate_cuda_matvec_stats(&mut perf, &gate_stats);
                if let Some(bias) = layer.feed_forward_gate_experts_bias.as_ref() {
                    add_expert_bias_in_place(
                        &mut gate,
                        bias.as_slice(),
                        expert_index,
                        layer.feed_forward_gate_experts_weight.rows,
                    );
                }

                let mut up = Vec::new();
                let up_stats = layer.feed_forward_up_experts_weight.expert_matvec_profiled(
                    backend,
                    expert_index,
                    ffn_input.as_slice(),
                    &mut up,
                )?;
                accumulate_cuda_matvec_stats(&mut perf, &up_stats);
                if let Some(bias) = layer.feed_forward_up_experts_bias.as_ref() {
                    add_expert_bias_in_place(
                        &mut up,
                        bias.as_slice(),
                        expert_index,
                        layer.feed_forward_up_experts_weight.rows,
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
                let expert_stats = layer.feed_forward_down_experts_weight.expert_matvec_profiled(
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
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_up_experts_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_experts_weight.byte_length() as u64);
            kernel_count = kernel_count.saturating_add(7 + selected.len().saturating_mul(3));
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
        let logits_stats = self
            .output
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
            kernel_count,
            bytes_moved,
            perf: Some(perf),
        })
    }
}

#[derive(Clone, Debug)]
struct GptOssCudaLayer {
    attention_norm: Vec<f32>,
    attention_query_weight: CudaQuantizedMatrix,
    attention_query_bias: Vec<f32>,
    attention_key_weight: CudaQuantizedMatrix,
    attention_key_bias: Vec<f32>,
    attention_value_weight: CudaQuantizedMatrix,
    attention_value_bias: Vec<f32>,
    attention_output_weight: CudaQuantizedMatrix,
    attention_output_bias: Option<Vec<f32>>,
    attention_sinks: Option<Vec<f32>>,
    feed_forward_norm: Vec<f32>,
    feed_forward_router_weight: DenseMatrix,
    feed_forward_router_bias: Option<Vec<f32>>,
    feed_forward_gate_experts_weight: CudaQuantizedExpertTensor,
    feed_forward_gate_experts_bias: Option<Vec<f32>>,
    feed_forward_up_experts_weight: CudaQuantizedExpertTensor,
    feed_forward_up_experts_bias: Option<Vec<f32>>,
    feed_forward_down_experts_weight: CudaQuantizedExpertTensor,
    feed_forward_down_experts_bias: Option<Vec<f32>>,
}

impl GptOssCudaLayer {
    fn load(
        backend: &mut CudaBackend,
        artifact: &GgufBlobArtifact,
        layout: &GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        Ok(Self {
            attention_norm: load_dense_vector(artifact, layout.attention_norm.as_str())?,
            attention_query_weight: load_cuda_quantized_matrix(
                backend,
                artifact,
                layout.attention_query_weight.as_str(),
            )?,
            attention_query_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_query_bias.as_ref(), "attention_query_bias")?,
            )?,
            attention_key_weight: load_cuda_quantized_matrix(
                backend,
                artifact,
                layout.attention_key_weight.as_str(),
            )?,
            attention_key_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_key_bias.as_ref(), "attention_key_bias")?,
            )?,
            attention_value_weight: load_cuda_quantized_matrix(
                backend,
                artifact,
                layout.attention_value_weight.as_str(),
            )?,
            attention_value_bias: load_dense_vector(
                artifact,
                required_tensor_name(layout.attention_value_bias.as_ref(), "attention_value_bias")?,
            )?,
            attention_output_weight: load_cuda_quantized_matrix(
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
            feed_forward_gate_experts_weight: load_cuda_quantized_expert_tensor(
                backend,
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
            feed_forward_up_experts_weight: load_cuda_quantized_expert_tensor(
                backend,
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
            feed_forward_down_experts_weight: load_cuda_quantized_expert_tensor(
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

#[derive(Clone, Debug)]
struct GptOssForwardStep {
    key: Vec<f32>,
    value: Vec<f32>,
    logits: Vec<f32>,
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

fn load_dense_vector(artifact: &GgufBlobArtifact, name: &str) -> Result<Vec<f32>, ModelLoadError> {
    let tensor = artifact.load_tensor(name)?;
    tensor.values().map(|values| values.into_owned())
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
