use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use psionic_adapters::{
    AdapterArtifactIdentity, AdapterResidencyMode, AdapterServingBinding, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact,
};
use psionic_backend_cpu::{decode_quantized_row_into, quantized_row_byte_len, quantized_row_dot};
use psionic_catalog::{BlobIntegrityPolicy, LocalBlobOpenOptions};
use psionic_core::QuantizationMode;
use psionic_models::{
    DecoderModelDescriptor, GgufBlobArtifact, GgufDecoderAdapterLoader, GgufDecoderFamily,
    GgufDecoderFamilyMetadata, GgufDecoderLayerTensorLayout, GgufRuntimeTokenizer, ModelLoadError,
    PagedTensorStorage, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::DeviceDiscovery;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ContinuousBatchGenerationResult, CpuGgufGptOssTextGenerationService, GenerationEventStream,
    GenerationInput, GenerationModelHandle, GenerationRequest, GenerationResponse,
    GenerationStepOutput, GenerationStreamChunk, GenerationStreamEvent, GenerationStreamStatus,
    GenerationStreamTerminal, GenerationStreamingPolicy, InMemoryGenerationModelRegistry,
    InMemoryGenerationSessionStore, LoadedModelView, LoadedModelsObservation,
    LocalRuntimeObservability, ManagedTextGenerationRuntime, ReferenceTextGenerationError,
    SessionId, SharedPrefixStore, StreamingTextGenerationExecutor, TextGenerationExecutor,
    continuous_batch_text_generation_execution_profile, default_generation_scheduler_policy,
    default_generation_streaming_policy,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderAdapterRuntimeSupport {
    pub support_level: String,
    pub import_formats: Vec<String>,
    pub residency_modes: Vec<String>,
    pub batching_mode: String,
    pub unsupported_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufDecoderRuntimeSupport {
    pub family: GgufDecoderFamily,
    pub supported_backends: Vec<String>,
    pub unsupported_backends: Vec<String>,
    pub unsupported_features: Vec<String>,
    pub quantization_modes: Vec<QuantizationMode>,
    pub adapter_runtime: DecoderAdapterRuntimeSupport,
}

#[derive(Clone, Debug)]
pub struct CpuGgufTextGenerationService {
    inner: CpuGgufServiceKind,
}

#[derive(Clone, Debug)]
enum CpuGgufServiceKind {
    GptOss(CpuGgufGptOssTextGenerationService),
    Dense(CpuDenseGgufTextGenerationService),
}

#[derive(Clone, Debug)]
struct DenseAdapterRuntime {
    binding: AdapterServingBinding,
    adapter: LmHeadLoraAdapterArtifact,
    merged_delta: Option<DenseMatrix>,
}

impl DenseAdapterRuntime {
    fn new(
        binding: AdapterServingBinding,
        adapter: LmHeadLoraAdapterArtifact,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let merged_delta = matches!(binding.residency_mode, AdapterResidencyMode::MergedResident)
            .then(|| DenseMatrix {
                rows: adapter.vocab_size,
                columns: adapter.hidden_size,
                values: adapter.merged_output_delta(),
            });
        Ok(Self {
            binding,
            adapter,
            merged_delta,
        })
    }

    fn apply_to_logits(
        &self,
        hidden: &[f32],
        logits: &mut [f32],
    ) -> Result<(), ReferenceTextGenerationError> {
        if let Some(merged_delta) = self.merged_delta.as_ref() {
            merged_delta.matvec_add(hidden, logits)?;
            return Ok(());
        }
        self.adapter
            .apply_to_logits(hidden, logits)
            .map_err(
                |error| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: self.binding.binding_id.clone(),
                    reason: error.to_string(),
                },
            )
    }
}

#[derive(Clone, Debug, Default)]
struct DenseAdapterRuntimeStore {
    runtimes: BTreeMap<String, DenseAdapterRuntime>,
}

impl DenseAdapterRuntimeStore {
    fn insert(&mut self, runtime: DenseAdapterRuntime) {
        self.runtimes
            .insert(runtime.binding.served_adapter_digest.clone(), runtime);
    }

    fn remove(&mut self, served_adapter_digest: &str) -> Option<DenseAdapterRuntime> {
        self.runtimes.remove(served_adapter_digest)
    }

    fn get(
        &self,
        binding: &AdapterServingBinding,
    ) -> Result<&DenseAdapterRuntime, ReferenceTextGenerationError> {
        self.runtimes
            .get(&binding.served_adapter_digest)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding.binding_id.clone(),
                reason: format!(
                    "adapter binding `{}` is not registered on this CPU GGUF runtime",
                    binding.served_adapter_digest
                ),
            })
    }
}

impl CpuGgufTextGenerationService {
    pub fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let family = GgufDecoderAdapterLoader
            .load_path(path.as_ref())?
            .family_metadata()
            .family;
        let inner = match family {
            GgufDecoderFamily::GptOss => CpuGgufServiceKind::GptOss(
                CpuGgufGptOssTextGenerationService::from_gguf_path(path)?,
            ),
            GgufDecoderFamily::Llama | GgufDecoderFamily::Qwen | GgufDecoderFamily::Mistral => {
                CpuGgufServiceKind::Dense(CpuDenseGgufTextGenerationService::from_gguf_path(path)?)
            }
        };
        Ok(Self { inner })
    }

    pub fn load_model_from_gguf_path(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), ReferenceTextGenerationError> {
        *self = Self::from_gguf_path(path)?;
        Ok(())
    }

    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        match &self.inner {
            CpuGgufServiceKind::GptOss(service) => service.model_descriptor(),
            CpuGgufServiceKind::Dense(service) => service.model_descriptor(),
        }
    }

    #[must_use]
    pub fn runtime_support(&self) -> GgufDecoderRuntimeSupport {
        match &self.inner {
            CpuGgufServiceKind::GptOss(service) => runtime_support_for_descriptor(
                service.model_descriptor(),
                GgufDecoderFamily::GptOss,
                vec![
                    String::from("cpu"),
                    String::from("cuda"),
                    String::from("metal"),
                ],
                Vec::new(),
                unsupported_adapter_runtime_support(
                    "LM-head LoRA serving is currently implemented only on dense CPU GGUF families",
                ),
            ),
            CpuGgufServiceKind::Dense(service) => service.runtime_support(),
        }
    }

    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        match &self.inner {
            CpuGgufServiceKind::GptOss(service) => service.plan_digest(model_id),
            CpuGgufServiceKind::Dense(service) => service.plan_digest(model_id),
        }
    }

    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.create_session(model_id),
            CpuGgufServiceKind::Dense(service) => service.create_session(model_id),
        }
    }

    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.reset_session(session_id),
            CpuGgufServiceKind::Dense(service) => service.reset_session(session_id),
        }
    }

    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.close_session(session_id),
            CpuGgufServiceKind::Dense(service) => service.close_session(session_id),
        }
    }

    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.loaded_model_views(),
            CpuGgufServiceKind::Dense(service) => service.loaded_model_views(),
        }
    }

    pub fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.generate_continuous_batch(requests),
            CpuGgufServiceKind::Dense(service) => service.generate_continuous_batch(requests),
        }
    }

    pub fn register_lm_head_lora_adapter(
        &mut self,
        binding_id: impl Into<String>,
        path: impl AsRef<Path>,
        identity: AdapterArtifactIdentity,
        alpha: f32,
        residency_mode: AdapterResidencyMode,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(_) => {
                Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: binding_id.into(),
                    reason: String::from(
                        "LM-head LoRA serving is currently supported only on dense CPU GGUF families",
                    ),
                })
            }
            CpuGgufServiceKind::Dense(service) => service.register_lm_head_lora_adapter(
                binding_id,
                path,
                identity,
                alpha,
                residency_mode,
            ),
        }
    }

    pub fn detach_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(_) => {
                Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: served_adapter_digest.to_string(),
                    reason: String::from(
                        "LM-head LoRA serving is currently supported only on dense CPU GGUF families",
                    ),
                })
            }
            CpuGgufServiceKind::Dense(service) => {
                service.detach_adapter_binding(served_adapter_digest)
            }
        }
    }

    pub fn merge_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(_) => {
                Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: served_adapter_digest.to_string(),
                    reason: String::from(
                        "LM-head LoRA serving is currently supported only on dense CPU GGUF families",
                    ),
                })
            }
            CpuGgufServiceKind::Dense(service) => {
                service.merge_adapter_binding(served_adapter_digest)
            }
        }
    }

    pub fn unmerge_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(_) => {
                Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: served_adapter_digest.to_string(),
                    reason: String::from(
                        "LM-head LoRA serving is currently supported only on dense CPU GGUF families",
                    ),
                })
            }
            CpuGgufServiceKind::Dense(service) => {
                service.unmerge_adapter_binding(served_adapter_digest)
            }
        }
    }
}

impl TextGenerationExecutor for CpuGgufTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.generate(request),
            CpuGgufServiceKind::Dense(service) => service.generate(request),
        }
    }
}

impl StreamingTextGenerationExecutor for CpuGgufTextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.generate_stream(request),
            CpuGgufServiceKind::Dense(service) => service.generate_stream(request),
        }
    }
}

impl ManagedTextGenerationRuntime for CpuGgufTextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.loaded_models(),
            CpuGgufServiceKind::Dense(service) => service.loaded_models(),
        }
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.observability(),
            CpuGgufServiceKind::Dense(service) => service.observability(),
        }
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.warm_model(model_id, keep_alive_millis),
            CpuGgufServiceKind::Dense(service) => service.warm_model(model_id, keep_alive_millis),
        }
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        match &mut self.inner {
            CpuGgufServiceKind::GptOss(service) => service.unload_model(model_id),
            CpuGgufServiceKind::Dense(service) => service.unload_model(model_id),
        }
    }
}

#[derive(Clone, Debug)]
struct CpuDenseGgufTextGenerationService {
    backend: super::CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuDenseGgufGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    shared_prefixes: SharedPrefixStore,
    backend_health: super::BackendHealthTracker,
    model_descriptor: DecoderModelDescriptor,
    runtime_support: GgufDecoderRuntimeSupport,
    adapters: Arc<Mutex<DenseAdapterRuntimeStore>>,
}

impl CpuDenseGgufTextGenerationService {
    fn from_gguf_path(path: impl AsRef<Path>) -> Result<Self, ReferenceTextGenerationError> {
        let backend = super::CpuBackend::new();
        let adapters = Arc::new(Mutex::new(DenseAdapterRuntimeStore::default()));
        let model = CpuDenseGgufGenerationModel::from_gguf_path(path, Arc::clone(&adapters))?;
        let model_descriptor = model.descriptor().clone();
        let runtime_support = model.runtime_support();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.warm_with_metadata(
            model,
            super::current_time_millis(),
            super::DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        )?;
        let mut backend_health = super::BackendHealthTracker::default();
        backend_health.observe("cpu", backend.health(), super::current_time_millis());
        Ok(Self {
            backend,
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            shared_prefixes: SharedPrefixStore::default(),
            backend_health,
            model_descriptor,
            runtime_support,
            adapters,
        })
    }

    #[must_use]
    fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    #[must_use]
    fn runtime_support(&self) -> GgufDecoderRuntimeSupport {
        self.runtime_support.clone()
    }

    fn register_lm_head_lora_adapter(
        &mut self,
        binding_id: impl Into<String>,
        path: impl AsRef<Path>,
        identity: AdapterArtifactIdentity,
        alpha: f32,
        residency_mode: AdapterResidencyMode,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        let binding_id = binding_id.into();
        let served_artifact =
            crate::served_artifact_identity_for_decoder_backend(&self.model_descriptor, "cpu", &[]);
        validate_adapter_identity(
            &self.model_descriptor,
            self.runtime_support.family,
            served_artifact.served_artifact_digest.as_str(),
            &identity,
        )?;
        let adapter =
            LmHeadLoraAdapterArtifact::from_safetensors_path(path, identity.clone(), alpha)
                .map_err(
                    |error| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                        binding_id: binding_id.clone(),
                        reason: error.to_string(),
                    },
                )?;
        validate_lm_head_lora_adapter(&self.model_descriptor, &adapter)?;
        let binding = AdapterServingBinding::new(
            binding_id,
            self.model_descriptor.model.model_id.clone(),
            self.model_descriptor.model.revision.clone(),
            served_artifact.served_artifact_digest,
            residency_mode,
            vec![identity],
        );
        self.adapters
            .lock()
            .map_err(
                |_| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: binding.binding_id.clone(),
                    reason: String::from("adapter registry is poisoned"),
                },
            )?
            .insert(DenseAdapterRuntime::new(binding.clone(), adapter)?);
        Ok(binding)
    }

    fn detach_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        self.adapters
            .lock()
            .map_err(
                |_| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                    binding_id: served_adapter_digest.to_string(),
                    reason: String::from("adapter registry is poisoned"),
                },
            )?
            .remove(served_adapter_digest)
            .map(|runtime| runtime.binding)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: served_adapter_digest.to_string(),
                reason: String::from("adapter binding is not registered"),
            })
    }

    fn merge_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        self.rebind_adapter_residency(served_adapter_digest, AdapterResidencyMode::MergedResident)
    }

    fn unmerge_adapter_binding(
        &mut self,
        served_adapter_digest: &str,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        self.rebind_adapter_residency(served_adapter_digest, AdapterResidencyMode::HotSwapOverlay)
    }

    fn rebind_adapter_residency(
        &mut self,
        served_adapter_digest: &str,
        residency_mode: AdapterResidencyMode,
    ) -> Result<AdapterServingBinding, ReferenceTextGenerationError> {
        let mut adapters = self.adapters.lock().map_err(|_| {
            ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: served_adapter_digest.to_string(),
                reason: String::from("adapter registry is poisoned"),
            }
        })?;
        let runtime = adapters.remove(served_adapter_digest).ok_or_else(|| {
            ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: served_adapter_digest.to_string(),
                reason: String::from("adapter binding is not registered"),
            }
        })?;
        let binding = AdapterServingBinding::new(
            runtime.binding.binding_id.clone(),
            runtime.binding.base_model_id.clone(),
            runtime.binding.base_model_revision.clone(),
            runtime.binding.base_served_artifact_digest.clone(),
            residency_mode,
            runtime.binding.adapters.clone(),
        );
        adapters.insert(DenseAdapterRuntime::new(binding.clone(), runtime.adapter)?);
        Ok(binding)
    }

    #[must_use]
    fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CpuDenseGgufGenerationModel::plan_digest)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, super::current_time_millis(), keep_alive_millis)?)
    }

    #[must_use]
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(super::current_time_millis())
    }

    #[must_use]
    fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    #[must_use]
    fn observability(&mut self) -> LocalRuntimeObservability {
        self.observability_at(super::current_time_millis())
    }

    #[must_use]
    fn observability_at(&mut self, now_millis: u64) -> LocalRuntimeObservability {
        self.models.expire_idle(now_millis);
        self.backend_health
            .observe("cpu", self.backend.health(), now_millis);
        super::generation_runtime_observability(
            &self.models,
            &self.sessions,
            &self.backend_health,
            continuous_batch_text_generation_execution_profile(),
        )
    }

    #[must_use]
    fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(super::current_time_millis())
    }

    #[must_use]
    fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .unload_view(model_id, super::current_time_millis())?)
    }

    fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(
            model,
            super::served_artifact_identity_for_decoder_backend(model.descriptor(), "cpu", &[])
                .served_artifact_digest,
        ))
    }

    fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<crate::GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }

    fn generate_continuous_batch(
        &mut self,
        requests: Vec<GenerationRequest>,
    ) -> ContinuousBatchGenerationResult {
        super::run_continuous_batch_generation_requests(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            requests,
            default_generation_scheduler_policy(),
        )
    }
}

impl TextGenerationExecutor for CpuDenseGgufTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        super::run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            &mut self.shared_prefixes,
            request,
        )
    }
}

impl StreamingTextGenerationExecutor for CpuDenseGgufTextGenerationService {
    type Stream<'a> = Box<dyn GenerationEventStream + 'a>;

    fn generate_stream<'a>(
        &'a mut self,
        request: &GenerationRequest,
    ) -> Result<Self::Stream<'a>, ReferenceTextGenerationError> {
        let response = self.generate(request)?;
        Ok(Box::new(CompletedGenerationStream::new(response)))
    }
}

impl ManagedTextGenerationRuntime for CpuDenseGgufTextGenerationService {
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
struct CpuDenseGgufGenerationModel {
    inner: Arc<DenseGgufModelInner>,
    adapters: Arc<Mutex<DenseAdapterRuntimeStore>>,
}

impl CpuDenseGgufGenerationModel {
    fn from_gguf_path(
        path: impl AsRef<Path>,
        adapters: Arc<Mutex<DenseAdapterRuntimeStore>>,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let artifact = GgufBlobArtifact::open_path(path, gguf_local_blob_open_options())?;
        Self::from_blob_artifact(artifact, adapters)
    }

    fn from_blob_artifact(
        artifact: GgufBlobArtifact,
        adapters: Arc<Mutex<DenseAdapterRuntimeStore>>,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let adapter = GgufDecoderAdapterLoader.load_blob_artifact(&artifact)?;
        if matches!(adapter.family_metadata().family, GgufDecoderFamily::GptOss) {
            return Err(ModelLoadError::UnsupportedModel(
                adapter.descriptor().model.model_id.clone(),
            )
            .into());
        }
        let tokenizer = GgufRuntimeTokenizer::from_gguf(adapter.tokenizer()).map_err(|error| {
            ModelLoadError::ArtifactFormat {
                format: String::from("gguf"),
                message: format!("failed to build runtime tokenizer: {error}"),
            }
        })?;
        let token_embedding =
            ProjectionMatrix::load(&artifact, adapter.tensor_layout().token_embedding.as_str())?;
        let output = if let Some(name) = adapter.tensor_layout().output.as_ref() {
            ProjectionMatrix::load(&artifact, name)?
        } else {
            token_embedding.clone()
        };
        let layers = adapter
            .tensor_layout()
            .layers
            .iter()
            .map(|layout| DenseGgufLayer::load(&artifact, layout))
            .collect::<Result<Vec<_>, _>>()?;
        let descriptor = adapter.descriptor().clone();
        let inner = DenseGgufModelInner {
            descriptor: descriptor.clone(),
            family_metadata: adapter.family_metadata().clone(),
            tokenizer,
            token_embedding,
            output_norm: load_dense_vector(
                &artifact,
                adapter.tensor_layout().output_norm.as_str(),
            )?,
            output,
            layers,
            plan_digest: digest_dense_gguf_plan(&descriptor, adapter.family_metadata()),
            load_duration_ns: load_start
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(u64::MAX),
        };
        Ok(Self {
            inner: Arc::new(inner),
            adapters,
        })
    }

    #[must_use]
    fn plan_digest(&self) -> &str {
        self.inner.plan_digest.as_str()
    }

    #[must_use]
    fn runtime_support(&self) -> GgufDecoderRuntimeSupport {
        runtime_support_for_descriptor(
            &self.inner.descriptor,
            self.inner.family_metadata.family,
            vec![String::from("cpu")],
            vec![String::from("cuda"), String::from("metal")],
            dense_adapter_runtime_support(),
        )
    }
}

impl crate::GenerationModelHandle for CpuDenseGgufGenerationModel {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.inner.descriptor
    }
}

impl super::CompiledWordGenerationModel for CpuDenseGgufGenerationModel {
    type Backend = super::CpuBackend;

    fn tokenizer(&self) -> &dyn TokenizerBoundary {
        &self.inner.tokenizer
    }

    fn encode_prompt_input(
        &self,
        input: &GenerationInput,
    ) -> Result<TokenSequence, ReferenceTextGenerationError> {
        Ok(match input {
            GenerationInput::Text(text) => self.inner.tokenizer.encode_with_defaults(text),
            GenerationInput::Tokens(tokens) => tokens.clone(),
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
        cache: &crate::InMemoryKvCache,
    ) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
        let config = &self.inner.descriptor.config;
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
        if cache.width() != self.inner.cache_width() {
            return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
                expected_kv_width: self.inner.cache_width(),
                kv_width: cache.width(),
            });
        }
        let step = self.inner.forward_step(token, position, cache)?;
        Ok(GenerationStepOutput {
            key: step.key,
            value: step.value,
            logits: step.logits,
            hidden: Some(step.final_hidden),
            execution_plan_digest: Some(self.inner.plan_digest.clone()),
            compile_path: None,
            kernel_count: step.kernel_count,
            bytes_moved: step.bytes_moved,
            plan_cache_hits: 0,
            plan_cache_misses: 0,
            gpt_oss_perf: None,
        })
    }

    fn plan_digest(&self) -> &str {
        self.plan_digest()
    }

    fn load_duration_ns(&self) -> u64 {
        self.inner.load_duration_ns
    }

    fn backend_compatibility(&self) -> &'static str {
        "cpu"
    }

    fn adjust_step_output(
        &self,
        step: &mut GenerationStepOutput,
        request: &GenerationRequest,
    ) -> Result<(), ReferenceTextGenerationError> {
        let Some(binding) = request.adapter_serving.as_ref() else {
            return Ok(());
        };
        let hidden = step.hidden.as_ref().ok_or_else(|| {
            ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding.binding_id.clone(),
                reason: String::from(
                    "the active dense GGUF step does not expose the final hidden state needed for LM-head LoRA serving",
                ),
            }
        })?;
        let adapters = self.adapters.lock().map_err(|_| {
            ReferenceTextGenerationError::UnsupportedAdapterBinding {
                binding_id: binding.binding_id.clone(),
                reason: String::from("adapter registry is poisoned"),
            }
        })?;
        let runtime = adapters.get(binding)?;
        runtime.apply_to_logits(hidden.as_slice(), step.logits.as_mut_slice())
    }
}

#[derive(Clone, Debug)]
struct DenseGgufModelInner {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GgufRuntimeTokenizer,
    token_embedding: ProjectionMatrix,
    output_norm: Vec<f32>,
    output: ProjectionMatrix,
    layers: Vec<DenseGgufLayer>,
    plan_digest: String,
    load_duration_ns: u64,
}

impl DenseGgufModelInner {
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
        cache: &crate::InMemoryKvCache,
    ) -> Result<DenseGgufForwardStep, ReferenceTextGenerationError> {
        let kv_width = self.descriptor.config.kv_width();
        let mut bytes_moved = self.token_embedding.byte_length() as u64;
        let mut kernel_count = 1usize;
        let mut hidden = self.token_embedding.decode_row(token.as_u32() as usize)?;
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
            if let Some(bias) = layer.attention_query_bias.as_ref() {
                add_bias_in_place(&mut q, bias.as_slice());
            }

            let mut k = Vec::new();
            layer
                .attention_key_weight
                .matvec(hidden_norm.as_slice(), &mut k)?;
            if let Some(bias) = layer.attention_key_bias.as_ref() {
                add_bias_in_place(&mut k, bias.as_slice());
            }

            let mut v = Vec::new();
            layer
                .attention_value_weight
                .matvec(hidden_norm.as_slice(), &mut v)?;
            if let Some(bias) = layer.attention_value_bias.as_ref() {
                add_bias_in_place(&mut v, bias.as_slice());
            }

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

            let attention = attend_impl(
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
            let mut gate = Vec::new();
            layer
                .feed_forward_gate_weight
                .matvec(ffn_input.as_slice(), &mut gate)?;
            let mut up = Vec::new();
            layer
                .feed_forward_up_weight
                .matvec(ffn_input.as_slice(), &mut up)?;
            let activated = silu_glu(gate.as_slice(), up.as_slice());
            let mut ffn_out = Vec::new();
            layer
                .feed_forward_down_weight
                .matvec(activated.as_slice(), &mut ffn_out)?;
            hidden = add_vectors(ffn_out.as_slice(), ffn_residual.as_slice())?;

            bytes_moved = bytes_moved
                .saturating_add(layer.attention_query_weight.byte_length() as u64)
                .saturating_add(layer.attention_key_weight.byte_length() as u64)
                .saturating_add(layer.attention_value_weight.byte_length() as u64)
                .saturating_add(layer.attention_output_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_gate_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_up_weight.byte_length() as u64)
                .saturating_add(layer.feed_forward_down_weight.byte_length() as u64);
            kernel_count = kernel_count.saturating_add(7);
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

        Ok(DenseGgufForwardStep {
            key: cache_key,
            value: cache_value,
            logits,
            final_hidden,
            kernel_count,
            bytes_moved,
        })
    }
}

#[derive(Clone, Debug)]
struct DenseGgufLayer {
    attention_norm: Vec<f32>,
    attention_query_weight: ProjectionMatrix,
    attention_query_bias: Option<Vec<f32>>,
    attention_key_weight: ProjectionMatrix,
    attention_key_bias: Option<Vec<f32>>,
    attention_value_weight: ProjectionMatrix,
    attention_value_bias: Option<Vec<f32>>,
    attention_output_weight: ProjectionMatrix,
    attention_output_bias: Option<Vec<f32>>,
    feed_forward_norm: Vec<f32>,
    feed_forward_gate_weight: ProjectionMatrix,
    feed_forward_up_weight: ProjectionMatrix,
    feed_forward_down_weight: ProjectionMatrix,
}

impl DenseGgufLayer {
    fn load(
        artifact: &GgufBlobArtifact,
        layout: &GgufDecoderLayerTensorLayout,
    ) -> Result<Self, ModelLoadError> {
        Ok(Self {
            attention_norm: load_dense_vector(artifact, layout.attention_norm.as_str())?,
            attention_query_weight: ProjectionMatrix::load(
                artifact,
                layout.attention_query_weight.as_str(),
            )?,
            attention_query_bias: load_optional_dense_vector(
                artifact,
                layout.attention_query_bias.as_deref(),
            )?,
            attention_key_weight: ProjectionMatrix::load(
                artifact,
                layout.attention_key_weight.as_str(),
            )?,
            attention_key_bias: load_optional_dense_vector(
                artifact,
                layout.attention_key_bias.as_deref(),
            )?,
            attention_value_weight: ProjectionMatrix::load(
                artifact,
                layout.attention_value_weight.as_str(),
            )?,
            attention_value_bias: load_optional_dense_vector(
                artifact,
                layout.attention_value_bias.as_deref(),
            )?,
            attention_output_weight: ProjectionMatrix::load(
                artifact,
                layout.attention_output_weight.as_str(),
            )?,
            attention_output_bias: load_optional_dense_vector(
                artifact,
                layout.attention_output_bias.as_deref(),
            )?,
            feed_forward_norm: load_dense_vector(
                artifact,
                required_tensor_name(layout.feed_forward_norm.as_deref(), "feed_forward_norm")?,
            )?,
            feed_forward_gate_weight: ProjectionMatrix::load(
                artifact,
                required_tensor_name(
                    layout.feed_forward_gate_weight.as_deref(),
                    "feed_forward_gate_weight",
                )?,
            )?,
            feed_forward_up_weight: ProjectionMatrix::load(
                artifact,
                required_tensor_name(
                    layout.feed_forward_up_weight.as_deref(),
                    "feed_forward_up_weight",
                )?,
            )?,
            feed_forward_down_weight: ProjectionMatrix::load(
                artifact,
                required_tensor_name(
                    layout.feed_forward_down_weight.as_deref(),
                    "feed_forward_down_weight",
                )?,
            )?,
        })
    }
}

#[derive(Clone, Debug)]
enum ProjectionMatrix {
    Dense(DenseMatrix),
    Quantized(QuantizedMatrix),
}

impl ProjectionMatrix {
    fn load(artifact: &GgufBlobArtifact, name: &str) -> Result<Self, ModelLoadError> {
        let storage = artifact.paged_tensor(name)?;
        let metadata = storage.metadata();
        if let Some(layout) = metadata.quantized_layout {
            let dims = metadata.shape.dims().to_vec();
            let tensor_name = metadata.name.clone();
            let quantization = metadata.quantization;
            let [rows, columns] = dims.as_slice() else {
                return Err(ModelLoadError::InvalidTensorShape {
                    name: tensor_name,
                    expected: vec![0, 0],
                    actual: dims,
                });
            };
            let row_byte_len = quantized_row_byte_len(&metadata.shape, layout).map_err(|_| {
                ModelLoadError::InvalidQuantizedTensorShape {
                    quantization,
                    shape: metadata.shape.dims().to_vec(),
                }
            })?;
            return Ok(Self::Quantized(QuantizedMatrix {
                storage,
                mode: quantization,
                rows: *rows,
                columns: *columns,
                row_byte_len,
            }));
        }

        let tensor = artifact.load_tensor(name)?;
        let [rows, columns] = tensor.metadata().shape.dims() else {
            return Err(ModelLoadError::InvalidTensorShape {
                name: tensor.metadata().name.clone(),
                expected: vec![0, 0],
                actual: tensor.metadata().shape.dims().to_vec(),
            });
        };
        Ok(Self::Dense(DenseMatrix {
            rows: *rows,
            columns: *columns,
            values: tensor.values()?.into_owned(),
        }))
    }

    fn byte_length(&self) -> usize {
        match self {
            Self::Dense(matrix) => matrix
                .values
                .len()
                .saturating_mul(std::mem::size_of::<f32>()),
            Self::Quantized(matrix) => matrix.byte_length(),
        }
    }

    fn decode_row(&self, row_index: usize) -> Result<Vec<f32>, crate::RuntimeError> {
        match self {
            Self::Dense(matrix) => matrix.decode_row(row_index),
            Self::Quantized(matrix) => matrix.decode_row(row_index),
        }
    }

    fn matvec(&self, input: &[f32], output: &mut Vec<f32>) -> Result<(), crate::RuntimeError> {
        match self {
            Self::Dense(matrix) => matrix.matvec(input, output),
            Self::Quantized(matrix) => matrix.matvec(input, output),
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

    fn matvec(&self, input: &[f32], output: &mut Vec<f32>) -> Result<(), crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
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

    fn matvec_add(&self, input: &[f32], output: &mut [f32]) -> Result<(), crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
                "dense matvec width mismatch: expected {}, actual {}",
                self.columns,
                input.len()
            )));
        }
        if output.len() != self.rows {
            return Err(crate::RuntimeError::Backend(format!(
                "dense output row mismatch: expected {}, actual {}",
                self.rows,
                output.len()
            )));
        }
        for (row_index, row) in self.values.chunks_exact(self.columns).enumerate() {
            output[row_index] += dot(row, input);
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

    fn matvec(&self, input: &[f32], output: &mut Vec<f32>) -> Result<(), crate::RuntimeError> {
        if input.len() != self.columns {
            return Err(crate::RuntimeError::Backend(format!(
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
struct DenseGgufForwardStep {
    key: Vec<f32>,
    value: Vec<f32>,
    logits: Vec<f32>,
    final_hidden: Vec<f32>,
    kernel_count: usize,
    bytes_moved: u64,
}

fn gguf_local_blob_open_options() -> LocalBlobOpenOptions {
    LocalBlobOpenOptions::default().with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel)
}

fn runtime_support_for_descriptor(
    descriptor: &DecoderModelDescriptor,
    family: GgufDecoderFamily,
    supported_backends: Vec<String>,
    unsupported_backends: Vec<String>,
    adapter_runtime: DecoderAdapterRuntimeSupport,
) -> GgufDecoderRuntimeSupport {
    GgufDecoderRuntimeSupport {
        family,
        supported_backends,
        unsupported_backends,
        unsupported_features: Vec::new(),
        quantization_modes: descriptor.weights.quantization_modes.clone(),
        adapter_runtime,
    }
}

fn unsupported_adapter_runtime_support(reason: impl Into<String>) -> DecoderAdapterRuntimeSupport {
    DecoderAdapterRuntimeSupport {
        support_level: String::from("unsupported"),
        import_formats: Vec::new(),
        residency_modes: Vec::new(),
        batching_mode: String::from("not_available"),
        unsupported_reasons: vec![reason.into()],
    }
}

fn dense_adapter_runtime_support() -> DecoderAdapterRuntimeSupport {
    DecoderAdapterRuntimeSupport {
        support_level: String::from("lm_head_lora_cpu"),
        import_formats: vec![String::from("safetensors")],
        residency_modes: vec![
            String::from("hot_swap_overlay"),
            String::from("merged_resident"),
        ],
        batching_mode: String::from("mixed_adapter_bindings_per_request"),
        unsupported_reasons: Vec::new(),
    }
}

fn validate_adapter_identity(
    descriptor: &DecoderModelDescriptor,
    family: GgufDecoderFamily,
    served_artifact_digest: &str,
    identity: &AdapterArtifactIdentity,
) -> Result<(), ReferenceTextGenerationError> {
    if !matches!(
        family,
        GgufDecoderFamily::Llama | GgufDecoderFamily::Qwen | GgufDecoderFamily::Mistral
    ) {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: identity.adapter_id.clone(),
            reason: format!("decoder family `{family:?}` does not support LM-head LoRA serving"),
        });
    }
    if identity.base_model_id != descriptor.model.model_id {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: identity.adapter_id.clone(),
            reason: format!(
                "adapter targets base model `{}`, but the loaded model is `{}`",
                identity.base_model_id, descriptor.model.model_id
            ),
        });
    }
    if identity.base_model_revision != descriptor.model.revision {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: identity.adapter_id.clone(),
            reason: format!(
                "adapter targets base revision `{}`, but the loaded model is `{}`",
                identity.base_model_revision, descriptor.model.revision
            ),
        });
    }
    if identity.base_served_artifact_digest != served_artifact_digest {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: identity.adapter_id.clone(),
            reason: format!(
                "adapter targets served artifact `{}`, but the loaded model is `{served_artifact_digest}`",
                identity.base_served_artifact_digest
            ),
        });
    }
    if identity.target_family != AdapterTargetFamily::DecoderComposite {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: identity.adapter_id.clone(),
            reason: format!(
                "adapter target family `{:?}` is unsupported; only `decoder_composite` LM-head LoRA bindings are implemented",
                identity.target_family
            ),
        });
    }
    Ok(())
}

fn validate_lm_head_lora_adapter(
    descriptor: &DecoderModelDescriptor,
    adapter: &LmHeadLoraAdapterArtifact,
) -> Result<(), ReferenceTextGenerationError> {
    if adapter.hidden_size != descriptor.config.hidden_size {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: adapter.identity.adapter_id.clone(),
            reason: format!(
                "adapter hidden width {} does not match model hidden width {}",
                adapter.hidden_size, descriptor.config.hidden_size
            ),
        });
    }
    if adapter.vocab_size != descriptor.config.vocab_size {
        return Err(ReferenceTextGenerationError::UnsupportedAdapterBinding {
            binding_id: adapter.identity.adapter_id.clone(),
            reason: format!(
                "adapter vocab width {} does not match model vocab width {}",
                adapter.vocab_size, descriptor.config.vocab_size
            ),
        });
    }
    Ok(())
}

fn digest_dense_gguf_plan(
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
    hasher.update(descriptor.model.family.as_bytes());
    hasher.update(b"|");
    hasher.update(metadata.architecture.as_bytes());
    hasher.update(b"|dense-gguf-cpu|v1");
    hex::encode(hasher.finalize())
}

fn load_dense_vector(artifact: &GgufBlobArtifact, name: &str) -> Result<Vec<f32>, ModelLoadError> {
    artifact
        .load_tensor(name)?
        .values()
        .map(|values| values.into_owned())
}

fn load_optional_dense_vector(
    artifact: &GgufBlobArtifact,
    name: Option<&str>,
) -> Result<Option<Vec<f32>>, ModelLoadError> {
    name.map(|name| load_dense_vector(artifact, name))
        .transpose()
}

fn required_tensor_name<'a>(name: Option<&'a str>, field: &str) -> Result<&'a str, ModelLoadError> {
    name.ok_or_else(|| ModelLoadError::ArtifactFormat {
        format: String::from("gguf"),
        message: format!("missing required dense gguf tensor layout field `{field}`"),
    })
}

fn model_load_runtime_error(error: ModelLoadError) -> crate::RuntimeError {
    crate::RuntimeError::Backend(error.to_string())
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

fn add_bias_in_place(values: &mut [f32], bias: &[f32]) {
    for (value, bias) in values.iter_mut().zip(bias.iter().copied()) {
        *value += bias;
    }
}

fn silu_glu(gate: &[f32], up: &[f32]) -> Vec<f32> {
    gate.iter()
        .zip(up.iter())
        .map(|(gate, up)| {
            let activated = *gate / (1.0 + (-*gate).exp());
            activated * *up
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

fn attend_impl(
    layer_index: usize,
    query: &[f32],
    key: &[f32],
    value: &[f32],
    cache: &crate::InMemoryKvCache,
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
        cache.entries().to_vec()
    } else {
        cache.entries().iter().rev().cloned().collect()
    };
    let cached_entries = if let Some(window) = sliding_window {
        let retained = window.saturating_sub(1);
        cached_entries
            .into_iter()
            .rev()
            .take(retained)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
    } else {
        cached_entries
    };

    let mut output = vec![0.0; head_count.saturating_mul(head_dim)];
    for head_index in 0..head_count {
        let kv_head_index = head_index / group_size.max(1);
        let q = &query[head_index * head_dim..(head_index + 1) * head_dim];
        let local_key = &key[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];
        let local_value = &value[kv_head_index * head_dim..(kv_head_index + 1) * head_dim];

        let mut weights = Vec::with_capacity(cached_entries.len().saturating_add(1));
        for entry in &cached_entries {
            let start = layer_offset + kv_head_index * head_dim;
            let end = start + head_dim;
            weights.push(dot(q, &entry.key[start..end]) * scale);
        }
        weights.push(dot(q, local_key) * scale);

        let max_weight = weights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        for weight in &mut weights {
            *weight = (*weight - max_weight).exp();
        }
        let denom = weights.iter().copied().sum::<f32>().max(f32::EPSILON);
        for weight in &mut weights {
            *weight /= denom;
        }

        let output_slice = &mut output[head_index * head_dim..(head_index + 1) * head_dim];
        for (entry, weight) in cached_entries.iter().zip(weights.iter().copied()) {
            let start = layer_offset + kv_head_index * head_dim;
            let end = start + head_dim;
            axpy(output_slice, &entry.value[start..end], weight);
        }
        axpy(output_slice, local_value, *weights.last().unwrap_or(&0.0));
    }
    output
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

#[derive(Clone, Debug)]
struct CompletedGenerationStream {
    policy: GenerationStreamingPolicy,
    chunk: Option<GenerationStreamChunk>,
    terminal: Option<GenerationStreamTerminal>,
}

impl CompletedGenerationStream {
    fn new(response: GenerationResponse) -> Self {
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
            policy: default_generation_streaming_policy(),
            chunk: Some(chunk),
            terminal: Some(terminal),
        }
    }
}

impl GenerationEventStream for CompletedGenerationStream {
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
        self.terminal.take()
    }

    fn disconnect(&mut self) -> Option<GenerationStreamTerminal> {
        self.cancel()
    }
}

#[cfg(test)]
mod tests {
    use super::{CpuGgufServiceKind, CpuGgufTextGenerationService};
    use crate::{
        CompiledWordGenerationModel, GenerationModelHandle, GenerationOptions, GenerationRequest,
        GenerationStreamEvent, InMemoryKvCache, ReferenceTextGenerationError,
        StreamingTextGenerationExecutor, TextGenerationExecutor,
    };
    use psionic_adapters::{
        AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterResidencyMode,
        AdapterTargetFamily,
    };
    use psionic_core::QuantizationMode;
    use psionic_models::{GgufDecoderFamily, GgufMetadataValue, GgufTensorType};
    use safetensors::{Dtype as SafeTensorsDType, serialize_to_file, tensor::TensorView};
    use std::{fs, path::Path};
    use tempfile::tempdir;

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

    #[test]
    fn cpu_gguf_service_executes_llama_family() -> Result<(), Box<dyn std::error::Error>> {
        run_dense_family_case(
            "llama",
            GgufDecoderFamily::Llama,
            dense_llama_metadata("tiny psionic llama"),
            false,
            3,
            4,
            "hello",
        )
    }

    #[test]
    fn cpu_gguf_service_executes_qwen_family() -> Result<(), Box<dyn std::error::Error>> {
        run_dense_family_case(
            "qwen2",
            GgufDecoderFamily::Qwen,
            dense_qwen_metadata("tiny psionic qwen"),
            true,
            2,
            3,
            "hello",
        )
    }

    #[test]
    fn cpu_gguf_service_executes_mistral_family() -> Result<(), Box<dyn std::error::Error>> {
        run_dense_family_case(
            "mistral",
            GgufDecoderFamily::Mistral,
            dense_mistral_metadata("tiny psionic mistral"),
            false,
            3,
            4,
            "hello",
        )
    }

    #[test]
    fn cpu_gguf_service_streams_generic_family_output() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("llama_stream.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny psionic llama stream").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let mut service = CpuGgufTextGenerationService::from_gguf_path(&path)?;
        let request = GenerationRequest::new_text(
            String::from("gguf-llama-stream"),
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(1),
        );
        let mut stream = service.generate_stream(&request)?;

        let Some(GenerationStreamEvent::Chunk(chunk)) = stream.next_event() else {
            panic!("expected streamed chunk");
        };
        assert_eq!(chunk.output.text, "world");
        assert_eq!(chunk.cumulative_output_tokens, 1);

        let Some(GenerationStreamEvent::Terminal(terminal)) = stream.next_event() else {
            panic!("expected terminal stream event");
        };
        assert_eq!(terminal.response.output.text, "world");
        assert!(stream.next_event().is_none());
        Ok(())
    }

    #[test]
    fn cpu_gguf_service_serves_lm_head_lora_overlay_and_merge_modes()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("llama_lora.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny psionic llama lora").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let mut service = CpuGgufTextGenerationService::from_gguf_path(&path)?;
        let descriptor = service.model_descriptor().clone();
        let hidden = final_hidden_for_prompt(&mut service, "hello")?;
        let adapter_path = temp.path().join("lm_head_lora.safetensors");
        write_prompt_specific_lora_adapter(&adapter_path, hidden.as_slice(), 5, 32.0)?;
        let binding = service.register_lm_head_lora_adapter(
            "adapter-psionic",
            &adapter_path,
            sample_lora_identity(&descriptor, AdapterTargetFamily::DecoderComposite),
            1.0,
            AdapterResidencyMode::HotSwapOverlay,
        )?;
        let support = service.runtime_support();
        assert_eq!(support.adapter_runtime.support_level, "lm_head_lora_cpu");
        assert_eq!(
            support.adapter_runtime.import_formats,
            vec![String::from("safetensors")]
        );

        let baseline = service.generate(&GenerationRequest::new_text(
            String::from("llama-baseline"),
            descriptor.clone(),
            None,
            "hello",
            GenerationOptions::greedy(1),
        ))?;
        assert_eq!(baseline.output.text, "world");

        let overlay_request = GenerationRequest::new_text(
            String::from("llama-overlay"),
            descriptor.clone(),
            None,
            "hello",
            GenerationOptions::greedy(1),
        )
        .with_adapter_serving(binding.clone());
        let overlay = service.generate(&overlay_request)?;
        assert_eq!(overlay.output.text, "psionic");
        assert_eq!(
            overlay
                .provenance
                .as_ref()
                .and_then(|value| value.adapter_serving.clone()),
            Some(binding.clone())
        );

        let merged_binding =
            service.merge_adapter_binding(binding.served_adapter_digest.as_str())?;
        let merged = service.generate(
            &GenerationRequest::new_text(
                String::from("llama-merged"),
                descriptor.clone(),
                None,
                "hello",
                GenerationOptions::greedy(1),
            )
            .with_adapter_serving(merged_binding.clone()),
        )?;
        assert_eq!(merged.output.text, "psionic");
        assert_eq!(
            merged_binding.residency_mode,
            AdapterResidencyMode::MergedResident
        );

        let unmerged_binding =
            service.unmerge_adapter_binding(merged_binding.served_adapter_digest.as_str())?;
        let unmerged = service.generate(
            &GenerationRequest::new_text(
                String::from("llama-unmerged"),
                descriptor.clone(),
                None,
                "hello",
                GenerationOptions::greedy(1),
            )
            .with_adapter_serving(unmerged_binding.clone()),
        )?;
        assert_eq!(unmerged.output.text, "psionic");
        assert_eq!(
            unmerged_binding.residency_mode,
            AdapterResidencyMode::HotSwapOverlay
        );

        let detached =
            service.detach_adapter_binding(unmerged_binding.served_adapter_digest.as_str())?;
        assert_eq!(
            detached.served_adapter_digest,
            unmerged_binding.served_adapter_digest
        );
        let error = service
            .generate(
                &GenerationRequest::new_text(
                    String::from("llama-detached"),
                    descriptor,
                    None,
                    "hello",
                    GenerationOptions::greedy(1),
                )
                .with_adapter_serving(unmerged_binding),
            )
            .expect_err("detached adapter binding should be refused");
        assert!(matches!(
            error,
            ReferenceTextGenerationError::UnsupportedAdapterBinding { .. }
        ));
        Ok(())
    }

    #[test]
    fn cpu_gguf_service_refuses_incompatible_lm_head_lora_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("llama_lora_refusal.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny psionic llama refusal").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let mut service = CpuGgufTextGenerationService::from_gguf_path(&path)?;
        let descriptor = service.model_descriptor().clone();
        let hidden = final_hidden_for_prompt(&mut service, "hello")?;
        let adapter_path = temp.path().join("lm_head_lora_bad.safetensors");
        write_prompt_specific_lora_adapter(&adapter_path, hidden.as_slice(), 5, 32.0)?;
        let error = service
            .register_lm_head_lora_adapter(
                "adapter-bad",
                &adapter_path,
                sample_lora_identity(&descriptor, AdapterTargetFamily::DecoderAttention),
                1.0,
                AdapterResidencyMode::HotSwapOverlay,
            )
            .expect_err("unsupported target family should be refused");
        assert!(matches!(
            error,
            ReferenceTextGenerationError::UnsupportedAdapterBinding { .. }
        ));
        assert!(
            error.to_string().contains("decoder_composite"),
            "refusal should describe the supported adapter target"
        );
        Ok(())
    }

    fn run_dense_family_case(
        family_label: &str,
        expected_family: GgufDecoderFamily,
        metadata: Vec<(String, GgufMetadataValue)>,
        include_qkv_bias: bool,
        hello_token_index: usize,
        world_token_index: usize,
        prompt: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join(format!("{family_label}.gguf"));
        write_test_gguf(
            &path,
            metadata.as_slice(),
            dense_decoder_tensors(include_qkv_bias, hello_token_index, world_token_index)
                .as_slice(),
        )?;

        let mut service = CpuGgufTextGenerationService::from_gguf_path(&path)?;
        let descriptor = service.model_descriptor().clone();
        let request = GenerationRequest::new_text(
            format!("gguf-{family_label}"),
            descriptor.clone(),
            None,
            prompt,
            GenerationOptions::greedy(1),
        );

        let response = service.generate(&request)?;
        let support = service.runtime_support();
        let loaded = service.loaded_model_views();
        let expected_family_label = family_label_for(expected_family);

        assert_eq!(descriptor.model.family, expected_family_label);
        assert_eq!(response.output.text, "world");
        assert_eq!(response.output.tokens.as_slice().len(), 1);
        assert_eq!(
            response
                .provenance
                .as_ref()
                .map(|value| value.served_artifact.quantization_family),
            Some(QuantizationMode::None)
        );
        assert_eq!(support.family, expected_family);
        assert_eq!(support.supported_backends, vec![String::from("cpu")]);
        assert_eq!(
            support.unsupported_backends,
            vec![String::from("cuda"), String::from("metal")]
        );
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].summary.family.as_deref(),
            Some(expected_family_label)
        );
        Ok(())
    }

    fn family_label_for(family: GgufDecoderFamily) -> &'static str {
        match family {
            GgufDecoderFamily::Llama => "llama",
            GgufDecoderFamily::Qwen => "qwen",
            GgufDecoderFamily::Mistral => "mistral",
            GgufDecoderFamily::GptOss => "gpt_oss",
        }
    }

    fn final_hidden_for_prompt(
        service: &mut CpuGgufTextGenerationService,
        prompt: &str,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let CpuGgufServiceKind::Dense(dense) = &mut service.inner else {
            panic!("expected dense GGUF runtime");
        };
        let model_id = dense.model_descriptor.model.model_id.clone();
        let loaded = dense
            .models
            .active(model_id.as_str())
            .expect("dense model should be active")
            .clone();
        let prompt_tokens =
            loaded.encode_prompt_input(&crate::GenerationInput::Text(prompt.into()))?;
        let mut cache =
            InMemoryKvCache::new(loaded.descriptor().config.max_context, loaded.cache_width());
        let mut final_hidden = None;
        for token in prompt_tokens.as_slice() {
            let step = loaded.execute_step(&mut dense.backend, *token, cache.len(), &cache)?;
            cache.append(*token, step.key, step.value)?;
            final_hidden = step.hidden;
        }
        Ok(final_hidden.expect("prompt evaluation should produce hidden state"))
    }

    fn sample_lora_identity(
        descriptor: &psionic_models::DecoderModelDescriptor,
        target_family: AdapterTargetFamily,
    ) -> AdapterArtifactIdentity {
        AdapterArtifactIdentity::new(
            "adapter-psionic",
            "r1",
            AdapterArtifactKind::Lora,
            AdapterArtifactFormat::Safetensors,
            descriptor.model.model_id.clone(),
            descriptor.model.revision.clone(),
            crate::served_artifact_identity_for_decoder_backend(descriptor, "cpu", &[])
                .served_artifact_digest,
            "adapter-artifact-digest",
            QuantizationMode::None,
            target_family,
            10,
        )
    }

    fn write_prompt_specific_lora_adapter(
        path: &Path,
        hidden: &[f32],
        target_token_index: usize,
        strength: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let norm = hidden.iter().map(|value| value * value).sum::<f32>();
        let lora_a: Vec<f32> = hidden.iter().map(|value| value / norm.max(1e-6)).collect();
        let mut lora_b = vec![0.0_f32; 6];
        lora_b[target_token_index] = strength;
        let lora_a_bytes = encode_f32_bytes(lora_a.as_slice());
        let lora_b_bytes = encode_f32_bytes(lora_b.as_slice());
        let mut tensors = std::collections::BTreeMap::new();
        tensors.insert(
            "lm_head.lora_A.weight".to_string(),
            TensorView::new(SafeTensorsDType::F32, vec![1, hidden.len()], &lora_a_bytes)?,
        );
        tensors.insert(
            "lm_head.lora_B.weight".to_string(),
            TensorView::new(SafeTensorsDType::F32, vec![6, 1], &lora_b_bytes)?,
        );
        serialize_to_file(tensors, None, path)?;
        Ok(())
    }

    fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn dense_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        metadata.extend(sentencepiece_tokenizer_metadata_entries());
        metadata
    }

    fn dense_mistral_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("mistral", name);
        metadata.push((
            String::from("mistral.attention.sliding_window"),
            GgufMetadataValue::U32(16),
        ));
        metadata.extend(sentencepiece_tokenizer_metadata_entries());
        metadata
    }

    fn dense_qwen_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("qwen2", name);
        metadata.extend(qwen_tokenizer_metadata_entries());
        metadata
    }

    fn dense_family_header(architecture: &str, name: &str) -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(architecture.to_string()),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                format!("{architecture}.context_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                format!("{architecture}.embedding_length"),
                GgufMetadataValue::U32(4),
            ),
            (
                format!("{architecture}.feed_forward_length"),
                GgufMetadataValue::U32(8),
            ),
            (
                format!("{architecture}.block_count"),
                GgufMetadataValue::U32(1),
            ),
            (
                format!("{architecture}.attention.head_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                format!("{architecture}.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                format!("{architecture}.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                format!("{architecture}.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
        ]
    }

    fn sentencepiece_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("llama")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<unk>")),
                    GgufMetadataValue::String(String::from("<s>")),
                    GgufMetadataValue::String(String::from("</s>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("tokenizer.ggml.unknown_token_id"),
                GgufMetadataValue::U32(0),
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

    fn qwen_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<|bos|>")),
                    GgufMetadataValue::String(String::from("<|eos|>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                    GgufMetadataValue::String(String::from("agent")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![GgufMetadataValue::String(String::from(
                    "hello world",
                ))]),
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
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn dense_decoder_tensors(
        include_qkv_bias: bool,
        hello_token_index: usize,
        world_token_index: usize,
    ) -> Vec<TestGgufTensor> {
        let mut tensors = vec![
            dense_tensor(
                "token_embd.weight",
                vec![6, 4],
                token_embedding_values(hello_token_index),
            ),
            dense_tensor("output_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
            dense_tensor(
                "output.weight",
                vec![6, 4],
                output_values(world_token_index),
            ),
            dense_tensor("blk.0.attn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
            dense_tensor("blk.0.attn_q.weight", vec![4, 4], vec![0.0; 16]),
            dense_tensor("blk.0.attn_k.weight", vec![2, 4], vec![0.0; 8]),
            dense_tensor("blk.0.attn_v.weight", vec![2, 4], vec![0.0; 8]),
            dense_tensor("blk.0.attn_output.weight", vec![4, 4], vec![0.0; 16]),
            dense_tensor("blk.0.ffn_gate.weight", vec![8, 4], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_down.weight", vec![4, 8], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_up.weight", vec![8, 4], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
        ];
        if include_qkv_bias {
            tensors.push(dense_tensor("blk.0.attn_q.bias", vec![4], vec![0.0; 4]));
            tensors.push(dense_tensor("blk.0.attn_k.bias", vec![2], vec![0.0; 2]));
            tensors.push(dense_tensor("blk.0.attn_v.bias", vec![2], vec![0.0; 2]));
        }
        tensors
    }

    fn token_embedding_values(hello_token_index: usize) -> Vec<f32> {
        let mut values = vec![0.0; 6 * 4];
        values[hello_token_index.saturating_mul(4)] = 2.0;
        values
    }

    fn output_values(world_token_index: usize) -> Vec<f32> {
        let mut values = vec![0.0; 6 * 4];
        values[world_token_index.saturating_mul(4)] = 1.0;
        values
    }

    fn dense_tensor(name: &str, shape: Vec<usize>, values: Vec<f32>) -> TestGgufTensor {
        TestGgufTensor::new(
            name,
            shape,
            GgufTensorType::F32,
            encode_f32_bytes(values.as_slice()),
        )
    }

    fn write_test_gguf(
        path: &Path,
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(path, build_test_gguf(metadata, tensors)?)?;
        Ok(())
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

    fn align_usize(value: usize, alignment: usize) -> usize {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + alignment - remainder
        }
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

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }
}
