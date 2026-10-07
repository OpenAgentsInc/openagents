use std::{
    env,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::StreamExt;
use psionic_models::{
    CSM_AUDIO_CODEBOOK_LANES, CSM_CPU_EXECUTION_ENGINE, CSM_MIMI_CPU_EXECUTION_ENGINE,
    CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID, CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION,
    CSM_WATERMARK_OPERATOR_DOGFOOD, CsmAudioClip, CsmCapabilityRefusal, CsmContextWindowPolicy,
    CsmCpuGeneratedWindow, CsmCpuGenerationRequest, CsmCpuGenerator, CsmFrontendError,
    CsmGenerationWindow, CsmLlamaTextTokenizer, CsmMimiCodebookPrefix, CsmMimiDecoder,
    CsmModelArtifactDescriptor, CsmModelConfig, CsmPromptSegment, CsmPythonParityFixture,
    CsmRuntimeBackend, CsmSamplingStrategy, CsmVoiceProfileGovernanceManifest,
    csm_build_prompt_frame_plan, csm_default_config_candidates, csm_default_mimi_weight_candidates,
    csm_default_model_weight_candidates, csm_python_parity_fixture,
    csm_reference_audio_encoding_refusal, csm_voice_profile_governance_manifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::tokio_runtime_telemetry_axum::serve_with_runtime_telemetry;

pub const CSM_SPEECH_MODEL_ID: &str = "sesame/csm-1b";
pub const CSM_SPEECH_PRODUCT_ID: &str = "psionic.csm_speech";
pub const CSM_SPEECH_ROUTE_OPENAI: &str = "/v1/audio/speech";
pub const CSM_SPEECH_ROUTE_PSIONIC: &str = "/psionic/csm/speech";
pub const CSM_SPEECH_ROUTE_WORKER_METADATA: &str = "/psionic/csm/worker/metadata";
pub const CSM_SPEECH_RESPONSE_FORMAT_WAV: &str = "wav";
pub const CSM_SPEECH_DEFAULT_BACKEND: &str = "cpu";
pub const CSM_SPEECH_EXECUTION_MODE: &str = "native";
pub const CSM_SPEECH_DEFAULT_EXECUTION_ENGINE: &str = CSM_CPU_EXECUTION_ENGINE;
pub const CSM_SPEECH_DEFAULT_RESIDENCY_MODE: &str = "warm_cpu";
pub const CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION: &str = "psionic.csm.artifact_governance.v1";
pub const CSM_LICENSE_POSTURE: &str =
    "license_review_required_before_public_or_customer_use_operator_dogfood_only";
pub const CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE: &str =
    "blocked_without_artifact_license_voice_profile_watermark_and_runtime_image_governance";
pub const CSM_RUNTIME_IMAGE_REF_UNSET: &str = "not_configured_local_runtime";
pub const CSM_ROLLBACK_TARGET_UNSET: &str = "fallback_to_current_autopilot_tts_provider";
const CSM_SPEECH_DEFAULT_AUDIO_LENGTH_MS: u64 = 240;
const CSM_SPEECH_MAX_AUDIO_LENGTH_MS: u64 = 20_000;
const CSM_SPEECH_DEFAULT_TIMEOUT_MS: u64 = 10_000;
const CSM_SPEECH_MAX_TIMEOUT_MS: u64 = 30_000;
const CSM_SPEECH_STREAM_BOUNDARY: &str = "psionic-csm-stream";
const CSM_SPEECH_STREAM_WINDOW_FRAMES: usize = 2;

#[derive(Clone, Debug)]
pub struct CsmSpeechServerConfig {
    pub host: String,
    pub port: u16,
    pub model_id: String,
    pub runtime_enabled: bool,
    pub backend: String,
    pub startup_load_mode: String,
    pub cpu_fallback_on_accelerator_failure: bool,
    pub gpu_model: String,
    pub runtime_image_ref: String,
}

impl Default for CsmSpeechServerConfig {
    fn default() -> Self {
        Self {
            host: String::from("127.0.0.1"),
            port: 8081,
            model_id: String::from(CSM_SPEECH_MODEL_ID),
            runtime_enabled: true,
            backend: String::from(CSM_SPEECH_DEFAULT_BACKEND),
            startup_load_mode: String::from("sync"),
            cpu_fallback_on_accelerator_failure: false,
            gpu_model: String::from("not_configured"),
            runtime_image_ref: String::from(CSM_RUNTIME_IMAGE_REF_UNSET),
        }
    }
}

impl CsmSpeechServerConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(host) = env::var("PSIONIC_CSM_HOST") {
            config.host = host;
        }
        if let Ok(port) = env::var("PSIONIC_CSM_PORT")
            && let Ok(port) = port.parse::<u16>()
        {
            config.port = port;
        }
        if let Ok(model_id) = env::var("PSIONIC_CSM_MODEL_ID") {
            config.model_id = model_id;
        }
        if let Ok(runtime) = env::var("PSIONIC_CSM_RUNTIME") {
            config.runtime_enabled = !matches!(
                runtime.to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "disabled"
            );
        }
        if let Ok(backend) = env::var("PSIONIC_CSM_BACKEND") {
            config.backend = backend;
        }
        if let Ok(startup_load_mode) = env::var("PSIONIC_CSM_STARTUP_LOAD_MODE") {
            config.startup_load_mode = startup_load_mode;
        }
        if let Ok(fallback) = env::var("PSIONIC_CSM_CPU_FALLBACK_ON_ACCELERATOR_FAILURE") {
            config.cpu_fallback_on_accelerator_failure = matches!(
                fallback.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        if let Ok(gpu_model) = env::var("PSIONIC_CSM_GPU_MODEL") {
            config.gpu_model = gpu_model;
        }
        if let Ok(runtime_image_ref) = env::var("PSIONIC_CSM_RUNTIME_IMAGE_REF") {
            config.runtime_image_ref = runtime_image_ref;
        }
        config
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, CsmSpeechServerError> {
        let host = self.host.parse::<IpAddr>().map_err(|error| {
            CsmSpeechServerError::Config(format!("invalid CSM host `{}`: {error}", self.host))
        })?;
        Ok(SocketAddr::new(host, self.port))
    }
}

#[derive(Clone)]
pub struct CsmSpeechServer {
    state: Arc<CsmSpeechState>,
}

struct CsmSpeechState {
    model_id: String,
    fixture: CsmPythonParityFixture,
    descriptor: CsmModelArtifactDescriptor,
    governance: CsmVoiceProfileGovernanceManifest,
    runtime_image_ref: String,
    runtime: Mutex<CsmSpeechRuntimeSlot>,
}

impl CsmSpeechServer {
    pub fn from_config(config: CsmSpeechServerConfig) -> Result<Self, CsmSpeechServerError> {
        let fixture = csm_python_parity_fixture().map_err(|error| {
            CsmSpeechServerError::Config(format!("failed to load CSM parity fixture: {error}"))
        })?;
        let descriptor = CsmModelArtifactDescriptor::from_fixture(&fixture).map_err(|error| {
            CsmSpeechServerError::Config(format!("failed to build CSM descriptor: {error}"))
        })?;
        let governance = csm_voice_profile_governance_manifest().map_err(|error| {
            CsmSpeechServerError::Config(format!(
                "failed to load CSM voice-profile governance manifest: {error}"
            ))
        })?;
        governance
            .validate_against_descriptor(&descriptor)
            .map_err(|error| {
                CsmSpeechServerError::Config(format!(
                    "failed to validate CSM voice-profile governance manifest: {error}"
                ))
            })?;
        let background_load = matches!(
            config.startup_load_mode.to_ascii_lowercase().as_str(),
            "background" | "async" | "deferred"
        );
        let runtime = if background_load {
            CsmSpeechRuntimeSlot::loading(&config)
        } else {
            CsmSpeechRuntimeSlot::load(&config, &fixture, &descriptor)
        };
        let state = Arc::new(CsmSpeechState {
            model_id: config.model_id.clone(),
            fixture: fixture.clone(),
            descriptor: descriptor.clone(),
            governance,
            runtime_image_ref: config.runtime_image_ref.clone(),
            runtime: Mutex::new(runtime),
        });

        if background_load {
            let state_for_loader = Arc::clone(&state);
            std::thread::Builder::new()
                .name(String::from("psionic-csm-runtime-loader"))
                .spawn(move || {
                    eprintln!(
                        "psionic csm speech runtime loading started backend={} mode=background",
                        config.backend
                    );
                    let loaded =
                        CsmSpeechRuntimeSlot::load(&config, &state_for_loader.fixture, &state_for_loader.descriptor);
                    let state_label = loaded.status.state;
                    let backend_label = loaded.status.backend.clone();
                    let engine_label = loaded.status.execution_engine.clone();
                    match state_for_loader.runtime.lock() {
                        Ok(mut slot) => {
                            *slot = loaded;
                            eprintln!(
                                "psionic csm speech runtime loading finished state={} backend={} execution_engine={}",
                                state_label, backend_label, engine_label
                            );
                        }
                        Err(_) => {
                            eprintln!(
                                "psionic csm speech runtime loading finished but runtime lock is poisoned"
                            );
                        }
                    }
                })
                .map_err(|error| {
                    CsmSpeechServerError::Config(format!(
                        "failed to spawn CSM background runtime loader: {error}"
                    ))
                })?;
        }

        Ok(Self { state })
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(csm_health))
            .route("/v1/models", get(csm_models))
            .route(CSM_SPEECH_ROUTE_WORKER_METADATA, get(csm_worker_metadata))
            .route(CSM_SPEECH_ROUTE_OPENAI, post(csm_audio_speech))
            .route(CSM_SPEECH_ROUTE_PSIONIC, post(csm_audio_speech))
            .with_state(Arc::clone(&self.state))
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), CsmSpeechServerError> {
        serve_with_runtime_telemetry(listener, self.router())
            .await
            .map_err(CsmSpeechServerError::Io)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CsmSpeechServerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(String),
}

struct CsmSpeechRuntimeSlot {
    status: CsmRuntimeStatus,
    engine: Option<CsmResidentSpeechEngine>,
}

struct CsmResidentSpeechEngine {
    tokenizer: CsmLlamaTextTokenizer,
    generator: CsmCpuGenerator,
    mimi: CsmMimiDecoder,
}

#[derive(Clone, Debug, Serialize)]
struct CsmRuntimeStatus {
    requested_backend: String,
    residency: String,
    backend: String,
    execution_engine: String,
    state: &'static str,
    loaded_at_unix_ms: Option<u128>,
    load_latency_ms: Option<u128>,
    tokenizer_loaded: bool,
    csm_config_loaded: bool,
    csm_model_loaded: bool,
    mimi_loaded: bool,
    accelerated_backend: String,
    gpu_model: String,
    cpu_fallback_reason: Option<String>,
    refusal: Option<CsmSpeechRefusalPublication>,
}

impl CsmSpeechRuntimeSlot {
    fn loading(config: &CsmSpeechServerConfig) -> Self {
        let requested_backend = CsmRuntimeBackend::parse(&config.backend).ok();
        let backend_label = requested_backend
            .as_ref()
            .map(CsmRuntimeBackend::label)
            .unwrap_or(config.backend.as_str())
            .to_string();
        let execution_engine = requested_backend
            .as_ref()
            .map(CsmRuntimeBackend::execution_engine)
            .unwrap_or("unsupported_backend")
            .to_string();
        let residency = requested_backend
            .as_ref()
            .map(CsmRuntimeBackend::residency_mode)
            .unwrap_or("loading")
            .to_string();
        let accelerated_backend = requested_backend
            .as_ref()
            .map(CsmRuntimeBackend::accelerated_backend)
            .unwrap_or("unknown")
            .to_string();
        let gpu_model = if requested_backend
            .as_ref()
            .is_some_and(CsmRuntimeBackend::is_accelerated)
        {
            config.gpu_model.clone()
        } else {
            String::from("none")
        };
        Self {
            status: CsmRuntimeStatus {
                requested_backend: backend_label.clone(),
                residency,
                backend: backend_label,
                execution_engine,
                state: "loading",
                loaded_at_unix_ms: None,
                load_latency_ms: None,
                tokenizer_loaded: false,
                csm_config_loaded: false,
                csm_model_loaded: false,
                mimi_loaded: false,
                accelerated_backend,
                gpu_model,
                cpu_fallback_reason: None,
                refusal: None,
            },
            engine: None,
        }
    }

    fn load(
        config: &CsmSpeechServerConfig,
        fixture: &CsmPythonParityFixture,
        descriptor: &CsmModelArtifactDescriptor,
    ) -> Self {
        if !config.runtime_enabled {
            return Self::unavailable(
                config.backend.as_str(),
                "runtime_disabled",
                "CSM runtime loading is disabled by server configuration",
                vec!["enable_rust_csm_runtime"],
            );
        }
        let requested_backend = match CsmRuntimeBackend::parse(&config.backend) {
            Ok(backend) => backend,
            Err(_) => {
                return Self::unavailable(
                    config.backend.as_str(),
                    "unsupported_backend",
                    "CSM speech supports cpu, cuda, cuda:<ordinal>, metal, or metal:<ordinal> backends",
                    vec!["choose_supported_backend"],
                );
            }
        };
        let mut active_backend = requested_backend.clone();
        let mut cpu_fallback_reason = None;
        let requested_backend_label = requested_backend.label().to_string();

        if let Err(error) = requested_backend.device()
            && requested_backend.is_accelerated()
        {
            if config.cpu_fallback_on_accelerator_failure {
                cpu_fallback_reason = Some(format!(
                    "requested backend {} failed device initialization and explicit CPU fallback is enabled: {error}",
                    requested_backend.label()
                ));
                active_backend = CsmRuntimeBackend::Cpu;
            } else {
                eprintln!(
                    "psionic csm speech accelerated device initialization failed backend={} fallback=disabled error={error}",
                    requested_backend.label()
                );
                return Self::unavailable(
                    requested_backend.label(),
                    "accelerator_unavailable",
                    format!(
                        "requested CSM accelerator backend failed device initialization and CPU fallback is disabled: {error}"
                    ),
                    vec!["deploy_gpu_runtime_or_enable_explicit_cpu_fallback"],
                );
            }
        }

        if active_backend.is_accelerated() {
            // Reinitialize once here so startup fails before artifact hydration if the
            // accelerator backend is not usable inside the runtime image.
            if let Err(error) = active_backend.device() {
                eprintln!(
                    "psionic csm speech accelerated device recheck failed backend={} error={error}",
                    active_backend.label()
                );
                return Self::unavailable(
                    active_backend.label(),
                    "accelerator_unavailable",
                    format!(
                        "requested CSM accelerator backend failed device initialization: {error}"
                    ),
                    vec!["deploy_gpu_runtime_or_enable_explicit_cpu_fallback"],
                );
            }
        }

        if requested_backend.is_accelerated()
            && active_backend == CsmRuntimeBackend::Cpu
            && !config.cpu_fallback_on_accelerator_failure
        {
            return Self::unavailable(
                requested_backend.label(),
                "unsupported_backend",
                "CSM accelerator backend cannot silently fall back to CPU",
                vec!["enable_explicit_cpu_fallback_or_deploy_gpu_runtime"],
            );
        }

        let started = Instant::now();
        let loaded_at_unix_ms = current_unix_ms();

        let tokenizer = match CsmLlamaTextTokenizer::from_default_hf_cache(Some(
            &descriptor.digests.llama_tokenizer_digest,
        )) {
            Ok(tokenizer) => tokenizer,
            Err(_) => {
                return Self::unavailable(
                    active_backend.label(),
                    "llama_tokenizer_unavailable",
                    "matching Llama tokenizer artifact is unavailable in the local Hugging Face cache",
                    vec!["hydrate_gated_hf_artifacts"],
                );
            }
        };

        let Some(config_path) =
            csm_default_config_candidates(Some(&descriptor.digests.csm_config_digest))
                .into_iter()
                .next()
        else {
            return Self::unavailable(
                active_backend.label(),
                "csm_config_unavailable",
                "matching CSM config artifact is unavailable in the local Hugging Face cache",
                vec!["hydrate_gated_hf_artifacts"],
            );
        };
        let (model_config, config_digest) = match CsmModelConfig::from_json_file_with_digest(
            &config_path,
            Some(&descriptor.digests.csm_config_digest),
        ) {
            Ok(config) => config,
            Err(_) => {
                return Self::unavailable(
                    active_backend.label(),
                    "csm_config_invalid",
                    "matching CSM config artifact failed Rust digest or contract validation",
                    vec!["refresh_csm_artifact_cache"],
                );
            }
        };

        let Some(model_path) =
            csm_default_model_weight_candidates(Some(&descriptor.digests.csm_model_digest))
                .into_iter()
                .next()
        else {
            return Self::unavailable(
                active_backend.label(),
                "csm_model_unavailable",
                "matching CSM model artifact is unavailable in the local Hugging Face cache",
                vec!["hydrate_gated_hf_artifacts"],
            );
        };
        let Some(mimi_path) =
            csm_default_mimi_weight_candidates(Some(&fixture.model.mimi_weight_digest))
                .into_iter()
                .next()
        else {
            return Self::unavailable(
                active_backend.label(),
                "mimi_model_unavailable",
                "matching Mimi artifact is unavailable in the local Hugging Face cache",
                vec!["hydrate_gated_hf_artifacts"],
            );
        };
        let load_engine_for_backend =
            |backend: CsmRuntimeBackend| -> Result<CsmResidentSpeechEngine, CsmFrontendError> {
                let generator = CsmCpuGenerator::from_safetensors_file_with_backend(
                    &model_config,
                    config_digest.clone(),
                    &model_path,
                    Some(&descriptor.digests.csm_model_digest),
                    backend.clone(),
                )?;
                let mimi = CsmMimiDecoder::from_safetensors_file_with_backend(
                    &mimi_path,
                    Some(&fixture.model.mimi_weight_digest),
                    backend,
                )?;
                Ok(CsmResidentSpeechEngine {
                    tokenizer: tokenizer.clone(),
                    generator,
                    mimi,
                })
            };
        let engine = match load_engine_for_backend(active_backend.clone()) {
            Ok(engine) => engine,
            Err(error)
                if requested_backend.is_accelerated()
                    && active_backend.is_accelerated()
                    && config.cpu_fallback_on_accelerator_failure =>
            {
                eprintln!(
                    "psionic csm speech accelerated runtime load failed backend={} fallback=cpu error={error}",
                    requested_backend.label()
                );
                cpu_fallback_reason = Some(format!(
                    "requested backend {} failed model or Mimi load and explicit CPU fallback is enabled: {error}",
                    requested_backend.label()
                ));
                active_backend = CsmRuntimeBackend::Cpu;
                match load_engine_for_backend(active_backend.clone()) {
                    Ok(engine) => engine,
                    Err(error) => {
                        eprintln!(
                            "psionic csm speech CPU fallback runtime load failed after accelerator failure error={error}"
                        );
                        return Self::unavailable(
                            active_backend.label(),
                            "csm_cpu_fallback_load_failed",
                            "matching CSM or Mimi artifact failed Rust CPU fallback loading or digest validation",
                            vec!["refresh_csm_artifact_cache", "refresh_mimi_artifact_cache"],
                        );
                    }
                }
            }
            Err(error) => {
                eprintln!(
                    "psionic csm speech runtime load failed backend={} error={error}",
                    active_backend.label()
                );
                return Self::unavailable(
                    active_backend.label(),
                    "csm_runtime_load_failed",
                    "matching CSM or Mimi artifact failed Rust loading or digest validation",
                    vec!["refresh_csm_artifact_cache", "refresh_mimi_artifact_cache"],
                );
            }
        };

        let load_latency_ms = started.elapsed().as_millis();
        Self {
            status: CsmRuntimeStatus {
                requested_backend: requested_backend_label,
                residency: active_backend.residency_mode().to_string(),
                backend: active_backend.label().to_string(),
                execution_engine: active_backend.execution_engine().to_string(),
                state: "ready",
                loaded_at_unix_ms: Some(loaded_at_unix_ms),
                load_latency_ms: Some(load_latency_ms),
                tokenizer_loaded: true,
                csm_config_loaded: true,
                csm_model_loaded: true,
                mimi_loaded: true,
                accelerated_backend: active_backend.accelerated_backend().to_string(),
                gpu_model: if active_backend.is_accelerated() {
                    config.gpu_model.clone()
                } else {
                    String::from("none")
                },
                cpu_fallback_reason,
                refusal: None,
            },
            engine: Some(engine),
        }
    }

    fn unavailable(
        requested_backend: &str,
        code: &'static str,
        reason: impl Into<String>,
        pending_phases: Vec<&'static str>,
    ) -> Self {
        Self {
            status: CsmRuntimeStatus {
                requested_backend: requested_backend.to_string(),
                residency: "unavailable".to_string(),
                backend: CSM_SPEECH_DEFAULT_BACKEND.to_string(),
                execution_engine: CSM_SPEECH_DEFAULT_EXECUTION_ENGINE.to_string(),
                state: "unavailable",
                loaded_at_unix_ms: None,
                load_latency_ms: None,
                tokenizer_loaded: false,
                csm_config_loaded: false,
                csm_model_loaded: false,
                mimi_loaded: false,
                accelerated_backend: "unavailable_fail_closed".to_string(),
                gpu_model: "none".to_string(),
                cpu_fallback_reason: None,
                refusal: Some(CsmSpeechRefusalPublication {
                    code,
                    reason: reason.into(),
                    pending_phases,
                }),
            },
            engine: None,
        }
    }

    #[cfg(test)]
    fn is_ready(&self) -> bool {
        self.engine.is_some()
    }
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechHealthResponse {
    status: &'static str,
    model: String,
    capability: &'static str,
    served_backend: String,
    execution_mode: &'static str,
    execution_engine: String,
    supported_endpoints: Vec<&'static str>,
    supported_response_formats: Vec<&'static str>,
    voice_profiles: Vec<CsmVoiceProfilePublication>,
    artifact_digests: CsmArtifactDigestPublication,
    artifact_descriptor: CsmArtifactDescriptorPublication,
    governance: CsmArtifactGovernancePublication,
    runtime: CsmRuntimeStatus,
    worker: CsmWorkerMetadataPublication,
    codec_capabilities: CsmCodecCapabilityPublication,
    safety_capabilities: CsmSafetyCapabilityPublication,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_refusal: Option<CsmSpeechRefusalPublication>,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechModelsResponse {
    object: &'static str,
    data: Vec<CsmSpeechModelCard>,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechModelCard {
    id: String,
    object: &'static str,
    owned_by: &'static str,
    psionic_capability: &'static str,
    psionic_supported_endpoints: Vec<&'static str>,
    psionic_supported_response_formats: Vec<&'static str>,
    psionic_served_backend: String,
    psionic_execution_mode: &'static str,
    psionic_execution_engine: String,
    psionic_voice_profiles: Vec<CsmVoiceProfilePublication>,
    psionic_artifact_digests: CsmArtifactDigestPublication,
    psionic_artifact_descriptor: CsmArtifactDescriptorPublication,
    psionic_governance: CsmArtifactGovernancePublication,
    psionic_runtime: CsmRuntimeStatus,
    psionic_worker: CsmWorkerMetadataPublication,
    psionic_codec_capabilities: CsmCodecCapabilityPublication,
    psionic_safety_capabilities: CsmSafetyCapabilityPublication,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_execution_refusal: Option<CsmSpeechRefusalPublication>,
}

#[derive(Clone, Debug, Serialize)]
struct CsmVoiceProfilePublication {
    id: String,
    display_name: String,
    approval_status: String,
    runtime_admission: String,
    source_prompt_profile_id: String,
    speaker: u32,
    source: &'static str,
    prompt_transcript_sha256: String,
    prompt_audio_sha256: String,
    mimi_tokens_sha256: Option<String>,
    prompt_codebooks: Option<CsmPromptCodebookPublication>,
    consent_posture: String,
    allowed_product_surfaces: Vec<String>,
    disallowed_product_surfaces: Vec<String>,
    retention_policy: String,
    redaction_policy: String,
    watermarking: String,
    watermarking_refusal_code: String,
}

#[derive(Clone, Debug, Serialize)]
struct CsmPromptCodebookPublication {
    source: String,
    sample_rate_hz: u32,
    codebook_count: usize,
    frame_count: usize,
    prefix_frame_count: usize,
    prefix_codebook_count: usize,
    tokens_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct CsmArtifactDigestPublication {
    csm_config_digest: String,
    csm_model_digest: String,
    llama_tokenizer_digest: String,
    mimi_weight_digest: String,
}

#[derive(Clone, Debug, Serialize)]
struct CsmFrameContractPublication {
    frame_lanes: usize,
    audio_codebook_lanes: usize,
    text_lane_index: usize,
    max_seq_len: usize,
    generation_frame_ms: u64,
    sample_rate_hz: u32,
}

#[derive(Clone, Debug, Serialize)]
struct CsmArtifactDescriptorPublication {
    artifact_id: String,
    model_id: String,
    csm_repo: String,
    llama_tokenizer_repo: String,
    mimi_repo: String,
    mimi_weight: String,
    frame_contract: CsmFrameContractPublication,
}

#[derive(Clone, Debug, Serialize)]
struct CsmWorkerMetadataPublication {
    worker_id: &'static str,
    rpc_schema: &'static str,
    request_fields: Vec<&'static str>,
    response_metadata_fields: Vec<&'static str>,
    idempotency_key: &'static str,
    cancellation: &'static str,
    timeout_ms_default: u64,
    timeout_ms_max: u64,
    queue_depth: u64,
    in_flight_requests: u64,
    metrics: Vec<&'static str>,
    governance_fields: Vec<&'static str>,
    business_authority: &'static str,
}

#[derive(Clone, Debug, Serialize)]
struct CsmArtifactGovernancePublication {
    schema_version: &'static str,
    voice_profile_governance_schema_version: &'static str,
    artifact_id: String,
    artifact_hash: String,
    model_id: String,
    model_version: String,
    source_repositories: Vec<String>,
    license_posture: &'static str,
    runtime_image_ref: String,
    quantization: &'static str,
    tokenizer_dependency: String,
    audio_codec_dependency: String,
    allowed_voice_profile_ids: Vec<String>,
    disallowed_voice_use_cases: Vec<&'static str>,
    watermark_status: String,
    watermark_refusal_code: String,
    canary_promotion: &'static str,
    primary_promotion: &'static str,
    rollback_target: &'static str,
    missing_governance_blocks: Vec<&'static str>,
    autopilot_consumption: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct CsmCodecCapabilityPublication {
    mimi_decode: &'static str,
    mimi_decode_engine: &'static str,
    reference_audio_encode: &'static str,
    reference_audio_encode_refusal: CsmCapabilityRefusal,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSafetyCapabilityPublication {
    watermarking: &'static str,
    watermarking_refusal: CsmCapabilityRefusal,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechRefusalPublication {
    code: &'static str,
    reason: String,
    pending_phases: Vec<&'static str>,
}

fn voice_profiles(
    governance: &CsmVoiceProfileGovernanceManifest,
    descriptor: &CsmModelArtifactDescriptor,
) -> Vec<CsmVoiceProfilePublication> {
    governance
        .profiles
        .iter()
        .filter_map(|profile| {
            let source = descriptor
                .voice_profiles
                .iter()
                .find(|candidate| candidate.profile_id == profile.source_prompt_profile_id)?;
            Some(CsmVoiceProfilePublication {
                id: profile.profile_id.clone(),
                display_name: profile.display_name.clone(),
                approval_status: profile.approval_status.clone(),
                runtime_admission: profile.runtime_admission.clone(),
                source_prompt_profile_id: profile.source_prompt_profile_id.clone(),
                speaker: profile.speaker,
                source: "committed_voice_governance_manifest",
                prompt_transcript_sha256: profile.prompt_transcript_sha256.clone(),
                prompt_audio_sha256: profile.prompt_audio_sha256.clone(),
                mimi_tokens_sha256: profile.prompt_codebook_tokens_sha256.clone(),
                prompt_codebooks: source.prompt_codebooks.as_ref().map(|codebooks| {
                    CsmPromptCodebookPublication {
                        source: codebooks.source.clone(),
                        sample_rate_hz: codebooks.sample_rate_hz,
                        codebook_count: codebooks.codebook_count,
                        frame_count: codebooks.frame_count,
                        prefix_frame_count: codebooks.prefix_frame_count,
                        prefix_codebook_count: codebooks.prefix_codebook_count,
                        tokens_sha256: codebooks.tokens_sha256.clone(),
                    }
                }),
                consent_posture: profile.consent_posture.clone(),
                allowed_product_surfaces: profile.allowed_product_surfaces.clone(),
                disallowed_product_surfaces: profile.disallowed_product_surfaces.clone(),
                retention_policy: profile.retention_policy.clone(),
                redaction_policy: profile.redaction_policy.clone(),
                watermarking: profile.watermark_policy.status.clone(),
                watermarking_refusal_code: profile.watermark_policy.refusal_code.clone(),
            })
        })
        .collect()
}

fn artifact_digests(descriptor: &CsmModelArtifactDescriptor) -> CsmArtifactDigestPublication {
    CsmArtifactDigestPublication {
        csm_config_digest: descriptor.digests.csm_config_digest.clone(),
        csm_model_digest: descriptor.digests.csm_model_digest.clone(),
        llama_tokenizer_digest: descriptor.digests.llama_tokenizer_digest.clone(),
        mimi_weight_digest: descriptor.digests.mimi_weight_digest.clone(),
    }
}

fn csm_artifact_id(descriptor: &CsmModelArtifactDescriptor) -> String {
    format!(
        "{}@{}",
        descriptor.model_id, descriptor.digests.csm_model_digest
    )
}

fn artifact_descriptor(
    descriptor: &CsmModelArtifactDescriptor,
) -> CsmArtifactDescriptorPublication {
    CsmArtifactDescriptorPublication {
        artifact_id: csm_artifact_id(descriptor),
        model_id: descriptor.model_id.clone(),
        csm_repo: descriptor.csm_repo.clone(),
        llama_tokenizer_repo: descriptor.llama_tokenizer_repo.clone(),
        mimi_repo: descriptor.mimi_repo.clone(),
        mimi_weight: descriptor.mimi_weight.clone(),
        frame_contract: CsmFrameContractPublication {
            frame_lanes: descriptor.frame_contract.frame_lanes,
            audio_codebook_lanes: descriptor.frame_contract.audio_codebook_lanes,
            text_lane_index: descriptor.frame_contract.text_lane_index,
            max_seq_len: descriptor.frame_contract.max_seq_len,
            generation_frame_ms: descriptor.frame_contract.generation_frame_ms,
            sample_rate_hz: descriptor.frame_contract.sample_rate_hz,
        },
    }
}

fn governance_publication(state: &CsmSpeechState) -> CsmArtifactGovernancePublication {
    CsmArtifactGovernancePublication {
        schema_version: CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION,
        voice_profile_governance_schema_version: CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION,
        artifact_id: csm_artifact_id(&state.descriptor),
        artifact_hash: state.descriptor.digests.csm_model_digest.clone(),
        model_id: state.descriptor.model_id.clone(),
        model_version: "csm_1b_committed_fixture_descriptor_v1".to_string(),
        source_repositories: vec![
            state.descriptor.csm_repo.clone(),
            state.descriptor.llama_tokenizer_repo.clone(),
            state.descriptor.mimi_repo.clone(),
        ],
        license_posture: CSM_LICENSE_POSTURE,
        runtime_image_ref: state.runtime_image_ref.clone(),
        quantization: "none_full_precision_safetensors",
        tokenizer_dependency: format!(
            "{}@{}",
            state.descriptor.llama_tokenizer_repo, state.descriptor.digests.llama_tokenizer_digest
        ),
        audio_codec_dependency: format!(
            "{}:{}@{}",
            state.descriptor.mimi_repo,
            state.descriptor.mimi_weight,
            state.descriptor.digests.mimi_weight_digest
        ),
        allowed_voice_profile_ids: state
            .governance
            .profiles
            .iter()
            .map(|profile| profile.profile_id.clone())
            .collect(),
        disallowed_voice_use_cases: vec![
            "public_user_voice_clone",
            "customer_voice_clone",
            "contact_voice_clone",
            "arbitrary_reference_audio_upload",
        ],
        watermark_status: state.governance.watermark_policy.status.clone(),
        watermark_refusal_code: state.governance.watermark_policy.refusal_code.clone(),
        canary_promotion: CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE,
        primary_promotion: CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE,
        rollback_target: CSM_ROLLBACK_TARGET_UNSET,
        missing_governance_blocks: vec![
            "license_review",
            "private_watermark_or_equivalent_voice_safety_control",
            "runtime_image_ref",
            "autopilot_shadow_business_outcome_evidence",
        ],
        autopilot_consumption: vec![
            "artifact_id",
            "license_posture",
            "voice_profile_id",
            "watermark_status",
            "runtime_image_ref",
            "rollback_target",
        ],
    }
}

fn worker_metadata() -> CsmWorkerMetadataPublication {
    CsmWorkerMetadataPublication {
        worker_id: "psionic.csm_speech.worker.v1",
        rpc_schema: "psionic.csm.speech.worker.v1",
        request_fields: vec![
            "request_id",
            "input",
            "voice_profile_id",
            "artifact_id",
            "max_audio_length_ms",
            "timeout_ms",
            "cancellation_id",
            "stream",
        ],
        response_metadata_fields: vec![
            "request_id",
            "cancellation_id",
            "artifact_id",
            "voice_profile_id",
            "generated_frame_count",
            "first_audio_latency_ms",
            "full_generation_latency_ms",
            "output_duration_ms",
            "wav_pcm16_digest",
            "codebook_frames_sha256",
            "chunk_count",
        ],
        idempotency_key: "request_id",
        cancellation: "bounded_admission_only_current_generation_not_preemptible",
        timeout_ms_default: CSM_SPEECH_DEFAULT_TIMEOUT_MS,
        timeout_ms_max: CSM_SPEECH_MAX_TIMEOUT_MS,
        queue_depth: 0,
        in_flight_requests: 0,
        metrics: vec![
            "queue_depth",
            "in_flight_requests",
            "first_audio_latency_ms",
            "full_generation_latency_ms",
            "output_duration_ms",
            "failure_code",
            "runtime_state",
        ],
        governance_fields: vec![
            "artifact_id",
            "artifact_hash",
            "license_posture",
            "runtime_image_ref",
            "voice_profile_id",
            "watermark_status",
            "rollback_target",
        ],
        business_authority: "none_provider_output_is_evidence_not_instruction",
    }
}

fn codec_capabilities(runtime: &CsmRuntimeStatus) -> CsmCodecCapabilityPublication {
    let mimi_decode_engine = match runtime.backend.as_str() {
        "cuda" => "rust_moshi_mimi_cuda",
        "metal" => "rust_moshi_mimi_metal",
        _ => CSM_MIMI_CPU_EXECUTION_ENGINE,
    };
    CsmCodecCapabilityPublication {
        mimi_decode: "implemented",
        mimi_decode_engine,
        reference_audio_encode: "refused",
        reference_audio_encode_refusal: csm_reference_audio_encoding_refusal(),
    }
}

fn safety_capabilities() -> CsmSafetyCapabilityPublication {
    CsmSafetyCapabilityPublication {
        watermarking: CSM_WATERMARK_OPERATOR_DOGFOOD,
        watermarking_refusal: CsmCapabilityRefusal {
            code: "csm_watermarking_unavailable".to_string(),
            reason: "CSM speech watermarking is not implemented in the Rust serving path; output is admitted only for OpenAgents-operated Autopilot dogfood and remains unavailable for arbitrary public voice cloning".to_string(),
            required_phase: "private_watermark_or_equivalent_voice_safety_control_before_public_voice_clone".to_string(),
        },
    }
}

fn runtime_snapshot(state: &CsmSpeechState) -> CsmRuntimeStatus {
    match state.runtime.lock() {
        Ok(runtime) => runtime.status.clone(),
        Err(_) => {
            CsmSpeechRuntimeSlot::unavailable(
                CSM_SPEECH_DEFAULT_BACKEND,
                "runtime_lock_poisoned",
                "CSM runtime lock is poisoned and requests fail closed",
                vec!["restart_server"],
            )
            .status
        }
    }
}

async fn csm_health(State(state): State<Arc<CsmSpeechState>>) -> Json<CsmSpeechHealthResponse> {
    let runtime = runtime_snapshot(&state);
    Json(CsmSpeechHealthResponse {
        status: if runtime.state == "ready" {
            "ok"
        } else {
            "degraded"
        },
        model: state.model_id.clone(),
        capability: "speech_generation",
        served_backend: runtime.backend.clone(),
        execution_mode: CSM_SPEECH_EXECUTION_MODE,
        execution_engine: runtime.execution_engine.clone(),
        supported_endpoints: vec![CSM_SPEECH_ROUTE_OPENAI, CSM_SPEECH_ROUTE_PSIONIC],
        supported_response_formats: vec![CSM_SPEECH_RESPONSE_FORMAT_WAV],
        voice_profiles: voice_profiles(&state.governance, &state.descriptor),
        artifact_digests: artifact_digests(&state.descriptor),
        artifact_descriptor: artifact_descriptor(&state.descriptor),
        governance: governance_publication(&state),
        runtime: runtime.clone(),
        worker: worker_metadata(),
        codec_capabilities: codec_capabilities(&runtime),
        safety_capabilities: safety_capabilities(),
        execution_refusal: runtime.refusal,
    })
}

async fn csm_models(State(state): State<Arc<CsmSpeechState>>) -> Json<CsmSpeechModelsResponse> {
    let runtime = runtime_snapshot(&state);
    Json(CsmSpeechModelsResponse {
        object: "list",
        data: vec![CsmSpeechModelCard {
            id: state.model_id.clone(),
            object: "model",
            owned_by: "psionic",
            psionic_capability: "speech_generation",
            psionic_supported_endpoints: vec![CSM_SPEECH_ROUTE_OPENAI, CSM_SPEECH_ROUTE_PSIONIC],
            psionic_supported_response_formats: vec![CSM_SPEECH_RESPONSE_FORMAT_WAV],
            psionic_served_backend: runtime.backend.clone(),
            psionic_execution_mode: CSM_SPEECH_EXECUTION_MODE,
            psionic_execution_engine: runtime.execution_engine.clone(),
            psionic_voice_profiles: voice_profiles(&state.governance, &state.descriptor),
            psionic_artifact_digests: artifact_digests(&state.descriptor),
            psionic_artifact_descriptor: artifact_descriptor(&state.descriptor),
            psionic_governance: governance_publication(&state),
            psionic_runtime: runtime.clone(),
            psionic_worker: worker_metadata(),
            psionic_codec_capabilities: codec_capabilities(&runtime),
            psionic_safety_capabilities: safety_capabilities(),
            psionic_execution_refusal: runtime.refusal,
        }],
    })
}

async fn csm_worker_metadata(
    State(state): State<Arc<CsmSpeechState>>,
) -> Json<CsmSpeechHealthResponse> {
    csm_health(State(state)).await
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CsmSpeechRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_format: Option<String>,
    #[serde(default)]
    stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cancellation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_csm: Option<CsmGenerationParams>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct CsmGenerationParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    top_k: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_audio_length_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context_policy: Option<String>,
}

#[derive(Clone, Debug)]
struct ValidatedCsmSpeechRequest {
    model: String,
    artifact_id: String,
    request_id: String,
    input: String,
    voice_profile_id: String,
    source_prompt_profile_id: String,
    source_prompt_text: String,
    voice_approval_status: String,
    runtime_admission: String,
    consent_posture: String,
    watermarking: String,
    watermarking_refusal_code: String,
    speaker: u32,
    stream: bool,
    stream_format: CsmSpeechStreamFormat,
    cancellation_id: Option<String>,
    timeout_ms: u64,
    max_audio_length_ms: u64,
    sampling: CsmSamplingStrategy,
    context_policy: CsmServedContextPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CsmServedContextPolicy {
    None,
    PromptProfileOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CsmSpeechStreamFormat {
    MultipartMixed,
    JsonlBase64,
}

impl ValidatedCsmSpeechRequest {
    fn from_request(
        state: &CsmSpeechState,
        request: CsmSpeechRequest,
    ) -> Result<Self, CsmSpeechHttpError> {
        let model = request.model.unwrap_or_else(|| state.model_id.clone());
        if model != state.model_id {
            return Err(CsmSpeechHttpError::model_unavailable());
        }
        let artifact_id = csm_artifact_id(&state.descriptor);
        if let Some(requested_artifact_id) = request.artifact_id.as_deref()
            && requested_artifact_id != artifact_id
            && requested_artifact_id != state.descriptor.digests.csm_model_digest
        {
            return Err(CsmSpeechHttpError::invalid_request(
                "requested CSM artifact is not loaded by this worker",
                "artifact_unavailable",
            ));
        }
        let request_id = request
            .request_id
            .unwrap_or_else(|| format!("psionic_csm_req_{}", current_unix_ms()));
        if request_id.trim().is_empty() {
            return Err(CsmSpeechHttpError::invalid_request(
                "request_id must not be empty when provided",
                "empty_request_id",
            ));
        }
        if let Some(cancellation_id) = request.cancellation_id.as_deref()
            && cancellation_id.trim().is_empty()
        {
            return Err(CsmSpeechHttpError::invalid_request(
                "cancellation_id must not be empty when provided",
                "empty_cancellation_id",
            ));
        }
        let timeout_ms = request.timeout_ms.unwrap_or(CSM_SPEECH_DEFAULT_TIMEOUT_MS);
        if !(1..=CSM_SPEECH_MAX_TIMEOUT_MS).contains(&timeout_ms) {
            return Err(CsmSpeechHttpError::invalid_request(
                "timeout_ms must be between 1 and 30000",
                "invalid_timeout_ms",
            ));
        }
        if request.input.trim().is_empty() {
            return Err(CsmSpeechHttpError::invalid_request(
                "input text must not be empty",
                "empty_input",
            ));
        }
        let stream = request.stream;
        let stream_format = match request
            .stream_format
            .as_deref()
            .unwrap_or("multipart_mixed")
        {
            "multipart_mixed" => CsmSpeechStreamFormat::MultipartMixed,
            "jsonl_base64" => CsmSpeechStreamFormat::JsonlBase64,
            _ => {
                return Err(CsmSpeechHttpError::invalid_request(
                    "stream_format must be multipart_mixed or jsonl_base64",
                    "invalid_stream_format",
                ));
            }
        };
        let response_format = request
            .response_format
            .unwrap_or_else(|| String::from(CSM_SPEECH_RESPONSE_FORMAT_WAV));
        if response_format != CSM_SPEECH_RESPONSE_FORMAT_WAV {
            return Err(CsmSpeechHttpError::unsupported_format());
        }
        let voice_profile_id = request
            .voice_profile_id
            .or(request.voice)
            .unwrap_or_else(|| String::from(CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID));
        let Some(governed_profile) = state.governance.find_profile(&voice_profile_id) else {
            return Err(CsmSpeechHttpError::missing_voice_profile());
        };
        if !governed_profile.is_runtime_admitted() {
            return Err(CsmSpeechHttpError::unapproved_voice_profile());
        }
        let Some(prompt) = state.fixture.prompts.iter().find(|prompt| {
            prompt.profile_id == governed_profile.source_prompt_profile_id
                && prompt.speaker == governed_profile.speaker
        }) else {
            return Err(CsmSpeechHttpError::runtime_unavailable(
                "voice_profile_source_unavailable",
                "governed CSM voice profile references a missing source prompt",
            ));
        };
        let Some(source_descriptor) = state
            .descriptor
            .voice_profiles
            .iter()
            .find(|profile| profile.profile_id == governed_profile.source_prompt_profile_id)
        else {
            return Err(CsmSpeechHttpError::runtime_unavailable(
                "voice_profile_descriptor_unavailable",
                "governed CSM voice profile references a missing descriptor",
            ));
        };
        if source_descriptor.prompt_audio_sha256 != governed_profile.prompt_audio_sha256 {
            return Err(CsmSpeechHttpError::runtime_unavailable(
                "voice_profile_digest_mismatch",
                "governed CSM voice profile digest does not match descriptor",
            ));
        }
        let params = request.psionic_csm.unwrap_or_default();
        validate_params(&params)?;
        let max_audio_length_ms = params
            .max_audio_length_ms
            .unwrap_or(CSM_SPEECH_DEFAULT_AUDIO_LENGTH_MS);
        let context_policy = match params.context_policy.as_deref().unwrap_or("none") {
            "none" => CsmServedContextPolicy::None,
            "prompt_profile_only" => CsmServedContextPolicy::PromptProfileOnly,
            _ => unreachable!("context policy validated above"),
        };
        let sampling = csm_sampling_strategy(&params)?;
        Ok(Self {
            model,
            artifact_id,
            request_id,
            input: request.input,
            voice_profile_id,
            source_prompt_profile_id: governed_profile.source_prompt_profile_id.clone(),
            source_prompt_text: prompt.text.clone(),
            voice_approval_status: governed_profile.approval_status.clone(),
            runtime_admission: governed_profile.runtime_admission.clone(),
            consent_posture: governed_profile.consent_posture.clone(),
            watermarking: governed_profile.watermark_policy.status.clone(),
            watermarking_refusal_code: governed_profile.watermark_policy.refusal_code.clone(),
            speaker: prompt.speaker,
            stream,
            stream_format,
            cancellation_id: request.cancellation_id,
            timeout_ms,
            max_audio_length_ms,
            sampling,
            context_policy,
        })
    }
}

fn validate_params(params: &CsmGenerationParams) -> Result<(), CsmSpeechHttpError> {
    if let Some(temperature) = params.temperature
        && !(0.0..=2.0).contains(&temperature)
    {
        return Err(CsmSpeechHttpError::invalid_request(
            "temperature must be between 0 and 2",
            "invalid_temperature",
        ));
    }
    if let Some(top_k) = params.top_k
        && top_k == 0
    {
        return Err(CsmSpeechHttpError::invalid_request(
            "top_k must be greater than zero",
            "invalid_top_k",
        ));
    }
    if let Some(max_audio_length_ms) = params.max_audio_length_ms
        && !(80..=CSM_SPEECH_MAX_AUDIO_LENGTH_MS).contains(&max_audio_length_ms)
    {
        return Err(CsmSpeechHttpError::invalid_request(
            format!(
                "max_audio_length_ms must be between 80 and {CSM_SPEECH_MAX_AUDIO_LENGTH_MS} on the Rust CSM speech server"
            ),
            "invalid_max_audio_length_ms",
        ));
    }
    if let Some(context_policy) = params.context_policy.as_deref()
        && !matches!(context_policy, "prompt_profile_only" | "none")
    {
        return Err(CsmSpeechHttpError::invalid_request(
            "context_policy must be prompt_profile_only or none",
            "invalid_context_policy",
        ));
    }
    Ok(())
}

fn csm_sampling_strategy(
    params: &CsmGenerationParams,
) -> Result<CsmSamplingStrategy, CsmSpeechHttpError> {
    let top_k = params.top_k.unwrap_or(1);
    let temperature = f64::from(params.temperature.unwrap_or(0.0));
    if top_k <= 1 || temperature == 0.0 {
        return Ok(CsmSamplingStrategy::Greedy);
    }
    Ok(CsmSamplingStrategy::TopK {
        top_k,
        temperature,
        seed: 0,
    })
}

#[derive(Debug)]
struct CsmSpeechSynthesis {
    wav_bytes: Vec<u8>,
    codebook_frames_sha256: String,
    wav_pcm16_digest: String,
    backend: String,
    execution_engine: String,
    residency: String,
    accelerated_backend: String,
    gpu_model: String,
    cpu_fallback_reason: Option<String>,
    generated_frame_count: usize,
    prompt_frame_count: usize,
    hit_eos: bool,
    first_audio_latency_ms: u128,
    full_generation_latency_ms: u128,
    output_duration_ms: u64,
}

#[derive(Debug)]
struct CsmSpeechStreamChunk {
    index: usize,
    wav_bytes: Vec<u8>,
    duration_ms: u64,
    generated_frame_count: usize,
    final_chunk: bool,
    elapsed_ms: u128,
}

#[derive(Debug)]
struct CsmSpeechStreamingSummary {
    codebook_frames_sha256: String,
    wav_pcm16_digest: String,
    backend: String,
    execution_engine: String,
    residency: String,
    accelerated_backend: String,
    gpu_model: String,
    cpu_fallback_reason: Option<String>,
    generated_frame_count: usize,
    prompt_frame_count: usize,
    hit_eos: bool,
    first_audio_latency_ms: u128,
    full_generation_latency_ms: u128,
    output_duration_ms: u64,
    wav_bytes: usize,
    chunk_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechStreamTerminalMetadata {
    event: &'static str,
    model: String,
    artifact_id: String,
    governance_schema: &'static str,
    license_posture: &'static str,
    voice_profile_id: String,
    runtime_admission: String,
    watermarking: String,
    watermarking_refusal_code: String,
    backend: String,
    execution_engine: String,
    residency: String,
    accelerated_backend: String,
    gpu_model: String,
    cpu_fallback_reason: Option<String>,
    generated_frame_count: usize,
    prompt_frame_count: usize,
    hit_eos: bool,
    first_audio_latency_ms: u128,
    full_generation_latency_ms: u128,
    output_duration_ms: u64,
    wav_bytes: usize,
    chunk_count: usize,
    codebook_frames_sha256: String,
    wav_pcm16_digest: String,
}

impl CsmSpeechState {
    fn synthesize_blocking(
        &self,
        request: &ValidatedCsmSpeechRequest,
    ) -> Result<CsmSpeechSynthesis, CsmSpeechHttpError> {
        let started = Instant::now();
        let mut runtime = self.runtime.lock().map_err(|_| {
            CsmSpeechHttpError::runtime_unavailable(
                "runtime_lock_poisoned",
                "CSM runtime lock is poisoned and requests fail closed",
            )
        })?;
        let runtime_status = runtime.status.clone();
        let Some(engine) = runtime.engine.as_mut() else {
            let (code, reason) = runtime
                .status
                .refusal
                .as_ref()
                .map(|refusal| (refusal.code, refusal.reason.clone()))
                .unwrap_or((
                    "runtime_unavailable",
                    "CSM runtime is unavailable and requests fail closed".to_string(),
                ));
            return Err(CsmSpeechHttpError::runtime_unavailable(code, reason));
        };

        let target =
            CsmPromptSegment::encode_text_only(&engine.tokenizer, request.speaker, &request.input)
                .map_err(runtime_generation_error)?;
        let context = self.prompt_profile_context_segments(request, &engine.tokenizer)?;
        let plan = csm_build_prompt_frame_plan(
            &context,
            &target,
            CsmGenerationWindow::new(request.max_audio_length_ms),
            CsmContextWindowPolicy::Reject,
        )
        .map_err(runtime_generation_error)?;
        let report = engine
            .generator
            .generate_and_decode(
                &CsmCpuGenerationRequest {
                    prompt: plan,
                    sampling: request.sampling.clone(),
                },
                &mut engine.mimi,
            )
            .map_err(runtime_generation_error)?;
        let wav_bytes = report
            .decode
            .clip
            .to_wav_pcm16()
            .map_err(runtime_generation_error)?;
        let output_duration_ms = csm_output_duration_ms(
            report.decode.clip.samples.len(),
            report.decode.clip.sample_rate_hz,
        );
        let full_generation_latency_ms = started.elapsed().as_millis();

        Ok(CsmSpeechSynthesis {
            wav_bytes,
            codebook_frames_sha256: report.generation.frames_sha256,
            wav_pcm16_digest: report.decode.wav_pcm16_digest,
            backend: runtime_status.backend,
            execution_engine: runtime_status.execution_engine,
            residency: runtime_status.residency,
            accelerated_backend: runtime_status.accelerated_backend,
            gpu_model: runtime_status.gpu_model,
            cpu_fallback_reason: runtime_status.cpu_fallback_reason,
            generated_frame_count: report.generation.generated_frame_count,
            prompt_frame_count: report.generation.prompt_frame_count,
            hit_eos: report.generation.hit_eos,
            first_audio_latency_ms: full_generation_latency_ms,
            full_generation_latency_ms,
            output_duration_ms,
        })
    }

    fn synthesize_streaming_windows<F>(
        &self,
        request: &ValidatedCsmSpeechRequest,
        mut on_chunk: F,
    ) -> Result<CsmSpeechStreamingSummary, CsmSpeechHttpError>
    where
        F: FnMut(CsmSpeechStreamChunk) -> Result<(), CsmSpeechHttpError>,
    {
        let started = Instant::now();
        let mut runtime = self.runtime.lock().map_err(|_| {
            CsmSpeechHttpError::runtime_unavailable(
                "runtime_lock_poisoned",
                "CSM runtime lock is poisoned and requests fail closed",
            )
        })?;
        let runtime_status = runtime.status.clone();
        let Some(engine) = runtime.engine.as_mut() else {
            let (code, reason) = runtime
                .status
                .refusal
                .as_ref()
                .map(|refusal| (refusal.code, refusal.reason.clone()))
                .unwrap_or((
                    "runtime_unavailable",
                    "CSM runtime is unavailable and requests fail closed".to_string(),
                ));
            return Err(CsmSpeechHttpError::runtime_unavailable(code, reason));
        };

        let target =
            CsmPromptSegment::encode_text_only(&engine.tokenizer, request.speaker, &request.input)
                .map_err(runtime_generation_error)?;
        let context = self.prompt_profile_context_segments(request, &engine.tokenizer)?;
        let plan = csm_build_prompt_frame_plan(
            &context,
            &target,
            CsmGenerationWindow::new(request.max_audio_length_ms),
            CsmContextWindowPolicy::Reject,
        )
        .map_err(runtime_generation_error)?;

        let mut wav_hasher = Sha256::new();
        let mut first_audio_latency_ms = None;
        let mut output_duration_ms = 0_u64;
        let mut wav_bytes = 0_usize;
        let mut chunk_count = 0_usize;

        let generator = &mut engine.generator;
        let mimi = &mut engine.mimi;
        let report = generator
            .generate_codebook_frames_with_window_callback(
                &CsmCpuGenerationRequest {
                    prompt: plan,
                    sampling: request.sampling.clone(),
                },
                CSM_SPEECH_STREAM_WINDOW_FRAMES,
                |window: CsmCpuGeneratedWindow| {
                    let decode = mimi.decode_codebook_frames(&window.codebook_frames)?;
                    let elapsed_ms = started.elapsed().as_millis();
                    first_audio_latency_ms.get_or_insert(elapsed_ms);
                    let wav_chunks = split_csm_stream_clip_by_generated_frames(
                        &decode.clip,
                        window.codebook_frames.len(),
                    )?;
                    let subchunk_count = wav_chunks.len();
                    for (subchunk_index, clip) in wav_chunks.into_iter().enumerate() {
                        let wav_chunk = clip.to_wav_pcm16()?;
                        wav_hasher.update(&wav_chunk);
                        output_duration_ms = output_duration_ms.saturating_add(clip.duration_ms());
                        wav_bytes = wav_bytes.saturating_add(wav_chunk.len());
                        let index = chunk_count;
                        chunk_count = chunk_count.saturating_add(1);
                        let final_chunk =
                            window.final_window && subchunk_index + 1 == subchunk_count;
                        on_chunk(CsmSpeechStreamChunk {
                            index,
                            wav_bytes: wav_chunk,
                            duration_ms: clip.duration_ms(),
                            generated_frame_count: window
                                .start_frame_index
                                .saturating_add(subchunk_index + 1),
                            final_chunk,
                            elapsed_ms,
                        })
                        .map_err(|error| {
                            CsmFrontendError::CsmGeneration {
                                message: error.message,
                            }
                        })?;
                    }
                    Ok(())
                },
            )
            .map_err(runtime_generation_error)?;

        let full_generation_latency_ms = started.elapsed().as_millis();

        Ok(CsmSpeechStreamingSummary {
            codebook_frames_sha256: report.frames_sha256,
            wav_pcm16_digest: format!("sha256:{:x}", wav_hasher.finalize()),
            backend: runtime_status.backend,
            execution_engine: runtime_status.execution_engine,
            residency: runtime_status.residency,
            accelerated_backend: runtime_status.accelerated_backend,
            gpu_model: runtime_status.gpu_model,
            cpu_fallback_reason: runtime_status.cpu_fallback_reason,
            generated_frame_count: report.generated_frame_count,
            prompt_frame_count: report.prompt_frame_count,
            hit_eos: report.hit_eos,
            first_audio_latency_ms: first_audio_latency_ms.unwrap_or(full_generation_latency_ms),
            full_generation_latency_ms,
            output_duration_ms,
            wav_bytes,
            chunk_count,
        })
    }

    fn prompt_profile_context_segments(
        &self,
        request: &ValidatedCsmSpeechRequest,
        tokenizer: &CsmLlamaTextTokenizer,
    ) -> Result<Vec<CsmPromptSegment>, CsmSpeechHttpError> {
        match request.context_policy {
            CsmServedContextPolicy::None => Ok(Vec::new()),
            CsmServedContextPolicy::PromptProfileOnly => {
                let Some(prefix) = self
                    .fixture
                    .mimi_codebook_prefixes
                    .iter()
                    .find(|prefix| prefix.profile_id == request.source_prompt_profile_id)
                else {
                    return Err(CsmSpeechHttpError::runtime_unavailable(
                        "prompt_profile_context_unavailable",
                        "governed CSM voice profile does not have committed prompt codebooks",
                    ));
                };
                let frames = full_prompt_codebook_frames(prefix)?;
                let text_token_ids = tokenizer
                    .encode_segment_text(request.speaker, &request.source_prompt_text)
                    .map_err(runtime_generation_error)?;
                Ok(vec![CsmPromptSegment::with_audio_codebooks(
                    request.speaker,
                    request.source_prompt_text.clone(),
                    text_token_ids,
                    frames,
                )])
            }
        }
    }
}

fn full_prompt_codebook_frames(
    prefix: &CsmMimiCodebookPrefix,
) -> Result<Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>, CsmSpeechHttpError> {
    if prefix.codebook_count != CSM_AUDIO_CODEBOOK_LANES
        || prefix.prefix_codebook_count != CSM_AUDIO_CODEBOOK_LANES
        || prefix.prefix_frame_count != prefix.frame_count
        || prefix.prefix_by_codebook.len() != CSM_AUDIO_CODEBOOK_LANES
    {
        return Err(CsmSpeechHttpError::runtime_unavailable(
            "prompt_profile_context_unavailable",
            "prompt_profile_only requires full committed 32-codebook prompt context",
        ));
    }

    let mut frames = vec![[0_u32; CSM_AUDIO_CODEBOOK_LANES]; prefix.frame_count];
    for (codebook_index, row) in prefix.prefix_by_codebook.iter().enumerate() {
        if row.len() != prefix.frame_count {
            return Err(CsmSpeechHttpError::runtime_unavailable(
                "prompt_profile_context_unavailable",
                "prompt profile codebook row length does not match frame count",
            ));
        }
        for (frame_index, token) in row.iter().enumerate() {
            frames[frame_index][codebook_index] = *token;
        }
    }
    Ok(frames)
}

fn runtime_generation_error(error: CsmFrontendError) -> CsmSpeechHttpError {
    match error {
        CsmFrontendError::PromptContextOverflow { .. } => CsmSpeechHttpError::invalid_request(
            "CSM prompt exceeds the admitted context window",
            "context_overflow",
        ),
        CsmFrontendError::GenerationWindow { .. } => CsmSpeechHttpError::invalid_request(
            "CSM generation window is invalid",
            "invalid_generation_window",
        ),
        CsmFrontendError::UnsupportedSampling { .. } => CsmSpeechHttpError::invalid_request(
            "requested CSM sampling mode is unsupported",
            "unsupported_sampling",
        ),
        _ => CsmSpeechHttpError::runtime_unavailable(
            "csm_synthesis_failed",
            "CSM speech synthesis failed inside the Rust runtime",
        ),
    }
}

fn csm_output_duration_ms(sample_count: usize, sample_rate_hz: u32) -> u64 {
    if sample_rate_hz == 0 {
        return 0;
    }
    ((sample_count as u128 * 1_000) / u128::from(sample_rate_hz)) as u64
}

fn split_csm_stream_clip_by_generated_frames(
    clip: &CsmAudioClip,
    generated_frame_count: usize,
) -> Result<Vec<CsmAudioClip>, CsmFrontendError> {
    let channel_count = usize::from(clip.channels.max(1));
    let frame_count = generated_frame_count.max(1);
    let sample_frames = clip.frames();
    if sample_frames == 0 || frame_count <= 1 {
        return Ok(vec![clip.clone()]);
    }

    let mut chunks = Vec::with_capacity(frame_count);
    for index in 0..frame_count {
        let start_frame = index.saturating_mul(sample_frames) / frame_count;
        let end_frame = (index + 1).saturating_mul(sample_frames) / frame_count;
        if end_frame <= start_frame {
            continue;
        }
        let sample_start = start_frame.saturating_mul(channel_count);
        let sample_end = end_frame.saturating_mul(channel_count);
        let Some(samples) = clip.samples.get(sample_start..sample_end) else {
            return Err(CsmFrontendError::WavEncode {
                message: "stream subchunk sample range was invalid".to_string(),
            });
        };
        chunks.push(CsmAudioClip::new(
            clip.sample_rate_hz,
            clip.channels,
            samples.to_vec(),
        ));
    }

    if chunks.is_empty() {
        Ok(vec![clip.clone()])
    } else {
        Ok(chunks)
    }
}

fn current_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

async fn csm_audio_speech(
    State(state): State<Arc<CsmSpeechState>>,
    Json(request): Json<CsmSpeechRequest>,
) -> Result<Response, CsmSpeechHttpError> {
    let validated = ValidatedCsmSpeechRequest::from_request(&state, request)?;
    if validated.stream {
        return csm_generation_time_streaming_response(state, validated).await;
    }

    let state_for_runtime = Arc::clone(&state);
    let request_for_runtime = validated.clone();
    let synthesis = match tokio::task::spawn_blocking(move || {
        state_for_runtime.synthesize_blocking(&request_for_runtime)
    })
    .await
    {
        Ok(Ok(synthesis)) => synthesis,
        Ok(Err(error)) => {
            let mut response = error.into_response();
            insert_csm_execution_headers(response.headers_mut(), &state, &validated);
            return Ok(response);
        }
        Err(_) => {
            let mut response = CsmSpeechHttpError::runtime_unavailable(
                "runtime_join_failed",
                "CSM runtime blocking task failed before returning a speech response",
            )
            .into_response();
            insert_csm_execution_headers(response.headers_mut(), &state, &validated);
            return Ok(response);
        }
    };

    let mut response = csm_wav_response(synthesis)?;
    insert_csm_execution_headers(response.headers_mut(), &state, &validated);
    Ok(response)
}

async fn csm_generation_time_streaming_response(
    state: Arc<CsmSpeechState>,
    validated: ValidatedCsmSpeechRequest,
) -> Result<Response, CsmSpeechHttpError> {
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::convert::Infallible>>(16);
    let state_for_runtime = Arc::clone(&state);
    let request_for_runtime = validated.clone();
    let request_for_terminal = validated.clone();
    tokio::task::spawn_blocking(move || {
        let send_bytes = |tx: &mpsc::Sender<Result<Bytes, std::convert::Infallible>>,
                          bytes: Vec<u8>| {
            tx.blocking_send(Ok(Bytes::from(bytes))).is_ok()
        };

        if !send_bytes(
            &tx,
            csm_stream_start_part(request_for_runtime.stream_format),
        ) {
            return;
        }

        let synthesis =
            state_for_runtime.synthesize_streaming_windows(&request_for_runtime, |chunk| {
                let part = csm_stream_audio_part(request_for_runtime.stream_format, &chunk);
                if tx.blocking_send(Ok(Bytes::from(part))).is_err() {
                    return Err(CsmSpeechHttpError::runtime_unavailable(
                        "stream_client_closed",
                        "CSM streaming client closed before all generated chunks were delivered",
                    ));
                }
                Ok(())
            });

        match synthesis {
            Ok(summary) => {
                let terminal = CsmSpeechStreamTerminalMetadata {
                    event: "terminal",
                    model: request_for_terminal.model.clone(),
                    artifact_id: request_for_terminal.artifact_id.clone(),
                    governance_schema: CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION,
                    license_posture: CSM_LICENSE_POSTURE,
                    voice_profile_id: request_for_terminal.voice_profile_id.clone(),
                    runtime_admission: request_for_terminal.runtime_admission.clone(),
                    watermarking: request_for_terminal.watermarking.clone(),
                    watermarking_refusal_code: request_for_terminal
                        .watermarking_refusal_code
                        .clone(),
                    backend: summary.backend,
                    execution_engine: summary.execution_engine,
                    residency: summary.residency,
                    accelerated_backend: summary.accelerated_backend,
                    gpu_model: summary.gpu_model,
                    cpu_fallback_reason: summary.cpu_fallback_reason,
                    generated_frame_count: summary.generated_frame_count,
                    prompt_frame_count: summary.prompt_frame_count,
                    hit_eos: summary.hit_eos,
                    first_audio_latency_ms: summary.first_audio_latency_ms,
                    full_generation_latency_ms: summary.full_generation_latency_ms,
                    output_duration_ms: summary.output_duration_ms,
                    wav_bytes: summary.wav_bytes,
                    chunk_count: summary.chunk_count,
                    codebook_frames_sha256: summary.codebook_frames_sha256,
                    wav_pcm16_digest: summary.wav_pcm16_digest,
                };
                match csm_stream_terminal_part(request_for_terminal.stream_format, &terminal) {
                    Ok(part) => {
                        let _ = send_bytes(&tx, part);
                    }
                    Err(error) => {
                        let _ = send_bytes(
                            &tx,
                            csm_stream_error_part(request_for_terminal.stream_format, &error),
                        );
                    }
                }
            }
            Err(error) => {
                let _ = send_bytes(
                    &tx,
                    csm_stream_error_part(request_for_terminal.stream_format, &error),
                );
            }
        }
        let _ = send_bytes(
            &tx,
            csm_stream_close_part(request_for_terminal.stream_format),
        );
    });

    let stream = ReceiverStream::new(rx).map(|result| result);
    let content_type = match validated.stream_format {
        CsmSpeechStreamFormat::MultipartMixed => {
            format!("multipart/mixed; boundary={CSM_SPEECH_STREAM_BOUNDARY}")
        }
        CsmSpeechStreamFormat::JsonlBase64 => "application/x-ndjson".to_string(),
    };
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header("x-psionic-streaming-mode", "generation_time_windowed")
        .header(
            "x-psionic-stream-format",
            match validated.stream_format {
                CsmSpeechStreamFormat::MultipartMixed => "multipart_mixed",
                CsmSpeechStreamFormat::JsonlBase64 => "jsonl_base64",
            },
        )
        .header(
            "x-psionic-stream-window-frames",
            CSM_SPEECH_STREAM_WINDOW_FRAMES.to_string(),
        )
        .body(Body::from_stream(stream))
        .map_err(|_| {
            CsmSpeechHttpError::runtime_unavailable(
                "response_build_failed",
                "failed to build generation-time CSM stream response",
            )
        })?;
    insert_csm_execution_headers(response.headers_mut(), &state, &validated);
    Ok(response)
}

fn csm_wav_response(synthesis: CsmSpeechSynthesis) -> Result<Response, CsmSpeechHttpError> {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/wav")
        .body(Body::from(synthesis.wav_bytes.clone()))
        .map_err(|_| {
            CsmSpeechHttpError::runtime_unavailable(
                "response_build_failed",
                "failed to build CSM WAV response",
            )
        })?;
    insert_synthesis_headers(response.headers_mut(), &synthesis, 1);
    insert_csm_success_headers(response.headers_mut(), &synthesis);
    Ok(response)
}

#[cfg(test)]
fn csm_multipart_stream_body(
    chunks: Vec<&[u8]>,
    terminal: &CsmSpeechStreamTerminalMetadata,
) -> Result<Vec<u8>, CsmSpeechHttpError> {
    let mut body = Vec::new();
    body.extend_from_slice(&csm_multipart_stream_start_part());
    for (index, chunk) in chunks.iter().enumerate() {
        body.extend_from_slice(&csm_multipart_stream_audio_bytes(
            index, chunk, None, false, None, None,
        ));
    }
    body.extend_from_slice(&csm_multipart_stream_terminal_part(terminal)?);
    body.extend_from_slice(&csm_multipart_stream_close_part());
    Ok(body)
}

fn csm_multipart_stream_start_part() -> Vec<u8> {
    let mut part = Vec::new();
    part.extend_from_slice(format!("--{CSM_SPEECH_STREAM_BOUNDARY}\r\n").as_bytes());
    part.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
    part.extend_from_slice(
        br#"{"event":"start","encoding":"wav","chunk_transport":"multipart_mixed","streaming_mode":"generation_time_windowed"}"#,
    );
    part.extend_from_slice(b"\r\n");
    part
}

fn csm_stream_start_part(format: CsmSpeechStreamFormat) -> Vec<u8> {
    match format {
        CsmSpeechStreamFormat::MultipartMixed => csm_multipart_stream_start_part(),
        CsmSpeechStreamFormat::JsonlBase64 => {
            br#"{"event":"start","encoding":"wav","chunk_transport":"jsonl_base64","streaming_mode":"generation_time_windowed"}"#
                .iter()
                .copied()
                .chain(std::iter::once(b'\n'))
                .collect()
        }
    }
}

fn csm_multipart_stream_audio_part(chunk: &CsmSpeechStreamChunk) -> Vec<u8> {
    csm_multipart_stream_audio_bytes(
        chunk.index,
        &chunk.wav_bytes,
        Some(chunk.duration_ms),
        chunk.final_chunk,
        Some(chunk.generated_frame_count),
        Some(chunk.elapsed_ms),
    )
}

fn csm_stream_audio_part(format: CsmSpeechStreamFormat, chunk: &CsmSpeechStreamChunk) -> Vec<u8> {
    match format {
        CsmSpeechStreamFormat::MultipartMixed => csm_multipart_stream_audio_part(chunk),
        CsmSpeechStreamFormat::JsonlBase64 => {
            let payload = serde_json::json!({
                "event": "audio",
                "content_type": "audio/wav",
                "encoding": "wav",
                "chunk_index": chunk.index,
                "chunk_bytes": chunk.wav_bytes.len(),
                "duration_ms": chunk.duration_ms,
                "generated_frame_count": chunk.generated_frame_count,
                "elapsed_ms": chunk.elapsed_ms,
                "final_chunk": chunk.final_chunk,
                "data_base64": BASE64_STANDARD.encode(&chunk.wav_bytes),
            });
            serde_json::to_vec(&payload)
                .unwrap_or_else(|_| {
                    br#"{"event":"error","error":{"code":"json_encode_failed"}}"#.to_vec()
                })
                .into_iter()
                .chain(std::iter::once(b'\n'))
                .collect()
        }
    }
}

fn csm_multipart_stream_audio_bytes(
    index: usize,
    chunk: &[u8],
    duration_ms: Option<u64>,
    final_chunk: bool,
    generated_frame_count: Option<usize>,
    elapsed_ms: Option<u128>,
) -> Vec<u8> {
    let mut part = Vec::new();
    part.extend_from_slice(format!("--{CSM_SPEECH_STREAM_BOUNDARY}\r\n").as_bytes());
    part.extend_from_slice(b"Content-Type: audio/wav\r\n");
    part.extend_from_slice(format!("X-Psionic-Chunk-Index: {index}\r\n").as_bytes());
    part.extend_from_slice(format!("X-Psionic-Chunk-Bytes: {}\r\n", chunk.len()).as_bytes());
    part.extend_from_slice(format!("X-Psionic-Final-Chunk: {final_chunk}\r\n").as_bytes());
    if let Some(duration_ms) = duration_ms {
        part.extend_from_slice(
            format!("X-Psionic-Chunk-Duration-Ms: {duration_ms}\r\n").as_bytes(),
        );
    }
    if let Some(generated_frame_count) = generated_frame_count {
        part.extend_from_slice(
            format!("X-Psionic-Generated-Frame-Count: {generated_frame_count}\r\n").as_bytes(),
        );
    }
    if let Some(elapsed_ms) = elapsed_ms {
        part.extend_from_slice(format!("X-Psionic-Chunk-Elapsed-Ms: {elapsed_ms}\r\n").as_bytes());
    }
    part.extend_from_slice(b"\r\n");
    part.extend_from_slice(chunk);
    part.extend_from_slice(b"\r\n");
    part
}

fn csm_multipart_stream_terminal_part(
    terminal: &CsmSpeechStreamTerminalMetadata,
) -> Result<Vec<u8>, CsmSpeechHttpError> {
    let mut part = Vec::new();
    part.extend_from_slice(format!("--{CSM_SPEECH_STREAM_BOUNDARY}\r\n").as_bytes());
    part.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
    let terminal_json = serde_json::to_vec(terminal).map_err(|_| {
        CsmSpeechHttpError::runtime_unavailable(
            "terminal_metadata_failed",
            "failed to serialize CSM stream terminal metadata",
        )
    })?;
    part.extend_from_slice(&terminal_json);
    part.extend_from_slice(b"\r\n");
    Ok(part)
}

fn csm_stream_terminal_part(
    format: CsmSpeechStreamFormat,
    terminal: &CsmSpeechStreamTerminalMetadata,
) -> Result<Vec<u8>, CsmSpeechHttpError> {
    match format {
        CsmSpeechStreamFormat::MultipartMixed => csm_multipart_stream_terminal_part(terminal),
        CsmSpeechStreamFormat::JsonlBase64 => {
            let mut value = serde_json::to_value(terminal).map_err(|_| {
                CsmSpeechHttpError::runtime_unavailable(
                    "terminal_metadata_failed",
                    "failed to serialize CSM stream terminal metadata",
                )
            })?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "chunk_transport".to_string(),
                    serde_json::json!("jsonl_base64"),
                );
            }
            let mut line = serde_json::to_vec(&value).map_err(|_| {
                CsmSpeechHttpError::runtime_unavailable(
                    "terminal_metadata_failed",
                    "failed to serialize CSM stream terminal metadata",
                )
            })?;
            line.push(b'\n');
            Ok(line)
        }
    }
}

fn csm_multipart_stream_error_part(error: &CsmSpeechHttpError) -> Vec<u8> {
    let mut part = Vec::new();
    part.extend_from_slice(format!("--{CSM_SPEECH_STREAM_BOUNDARY}\r\n").as_bytes());
    part.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
    let payload = serde_json::json!({
        "event": "error",
        "error": {
            "message": error.message,
            "type": error.kind,
            "code": error.code,
            "served_backend": CSM_SPEECH_DEFAULT_BACKEND,
            "execution_mode": CSM_SPEECH_EXECUTION_MODE,
            "execution_engine": CSM_SPEECH_DEFAULT_EXECUTION_ENGINE
        }
    });
    let payload = serde_json::to_vec(&payload)
        .unwrap_or_else(|_| br#"{"event":"error","error":{"code":"stream_error"}}"#.to_vec());
    part.extend_from_slice(&payload);
    part.extend_from_slice(b"\r\n");
    part
}

fn csm_stream_error_part(format: CsmSpeechStreamFormat, error: &CsmSpeechHttpError) -> Vec<u8> {
    match format {
        CsmSpeechStreamFormat::MultipartMixed => csm_multipart_stream_error_part(error),
        CsmSpeechStreamFormat::JsonlBase64 => {
            let payload = serde_json::json!({
                "event": "error",
                "error": {
                    "message": error.message,
                    "type": error.kind,
                    "code": error.code,
                    "served_backend": CSM_SPEECH_DEFAULT_BACKEND,
                    "execution_mode": CSM_SPEECH_EXECUTION_MODE,
                    "execution_engine": CSM_SPEECH_DEFAULT_EXECUTION_ENGINE
                }
            });
            serde_json::to_vec(&payload)
                .unwrap_or_else(|_| {
                    br#"{"event":"error","error":{"code":"stream_error"}}"#.to_vec()
                })
                .into_iter()
                .chain(std::iter::once(b'\n'))
                .collect()
        }
    }
}

fn csm_multipart_stream_close_part() -> Vec<u8> {
    format!("--{CSM_SPEECH_STREAM_BOUNDARY}--\r\n").into_bytes()
}

fn csm_stream_close_part(format: CsmSpeechStreamFormat) -> Vec<u8> {
    match format {
        CsmSpeechStreamFormat::MultipartMixed => csm_multipart_stream_close_part(),
        CsmSpeechStreamFormat::JsonlBase64 => Vec::new(),
    }
}

fn insert_synthesis_headers(
    headers: &mut HeaderMap,
    synthesis: &CsmSpeechSynthesis,
    chunk_count: usize,
) {
    insert_header(
        headers,
        "x-psionic-csm-generated-frame-count",
        synthesis.generated_frame_count.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-prompt-frame-count",
        synthesis.prompt_frame_count.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-hit-eos",
        if synthesis.hit_eos { "true" } else { "false" },
    );
    insert_header(
        headers,
        "x-psionic-csm-frames-sha256",
        synthesis.codebook_frames_sha256.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-wav-pcm16-digest",
        synthesis.wav_pcm16_digest.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-first-audio-latency-ms",
        synthesis.first_audio_latency_ms.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-full-generation-latency-ms",
        synthesis.full_generation_latency_ms.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-output-duration-ms",
        synthesis.output_duration_ms.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-stream-chunk-count",
        chunk_count.to_string().as_str(),
    );
}

fn insert_csm_success_headers(headers: &mut HeaderMap, synthesis: &CsmSpeechSynthesis) {
    insert_header(headers, "x-psionic-residency-mode", &synthesis.residency);
    insert_header(
        headers,
        "x-psionic-accelerated-backend",
        &synthesis.accelerated_backend,
    );
    insert_header(headers, "x-psionic-generation-backend", &synthesis.backend);
    insert_header(
        headers,
        "x-psionic-generation-execution-engine",
        &synthesis.execution_engine,
    );
    insert_header(headers, "x-psionic-gpu-model", &synthesis.gpu_model);
    if let Some(reason) = synthesis.cpu_fallback_reason.as_deref() {
        insert_header(headers, "x-psionic-cpu-fallback-reason", reason);
    }
}

fn insert_csm_execution_headers(
    headers: &mut HeaderMap,
    state: &CsmSpeechState,
    request: &ValidatedCsmSpeechRequest,
) {
    let runtime = runtime_snapshot(state);
    insert_header(headers, "x-psionic-model-id", request.model.as_str());
    insert_header(headers, "x-psionic-request-id", request.request_id.as_str());
    if let Some(cancellation_id) = request.cancellation_id.as_deref() {
        insert_header(headers, "x-psionic-cancellation-id", cancellation_id);
    }
    insert_header(
        headers,
        "x-psionic-timeout-ms",
        request.timeout_ms.to_string().as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-artifact-id",
        request.artifact_id.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-governance-schema",
        CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION,
    );
    insert_header(
        headers,
        "x-psionic-csm-license-posture",
        CSM_LICENSE_POSTURE,
    );
    insert_header(
        headers,
        "x-psionic-csm-runtime-image-ref",
        state.runtime_image_ref.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-served-backend",
        runtime.backend.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-execution-mode",
        CSM_SPEECH_EXECUTION_MODE,
    );
    insert_header(
        headers,
        "x-psionic-execution-engine",
        runtime.execution_engine.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-requested-backend",
        runtime.requested_backend.as_str(),
    );
    insert_header(headers, "x-psionic-runtime-state", runtime.state);
    insert_header(
        headers,
        "x-psionic-csm-voice-profile-id",
        request.voice_profile_id.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-source-prompt-profile-id",
        request.source_prompt_profile_id.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-voice-approval-status",
        request.voice_approval_status.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-runtime-admission",
        request.runtime_admission.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-consent-posture",
        request.consent_posture.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-watermarking",
        request.watermarking.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-watermark-refusal-code",
        request.watermarking_refusal_code.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-csm-promotion-gate",
        CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE,
    );
    insert_header(
        headers,
        "x-psionic-csm-rollback-target",
        CSM_ROLLBACK_TARGET_UNSET,
    );
    insert_header(
        headers,
        "x-psionic-artifact-csm-config-digest",
        state.descriptor.digests.csm_config_digest.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-artifact-csm-model-digest",
        state.descriptor.digests.csm_model_digest.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-artifact-llama-tokenizer-digest",
        state.descriptor.digests.llama_tokenizer_digest.as_str(),
    );
    insert_header(
        headers,
        "x-psionic-artifact-mimi-weight-digest",
        state.descriptor.digests.mimi_weight_digest.as_str(),
    );
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(HeaderName::from_static(name), value);
    }
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechErrorEnvelope {
    error: CsmSpeechErrorBody,
}

#[derive(Clone, Debug, Serialize)]
struct CsmSpeechErrorBody {
    message: String,
    #[serde(rename = "type")]
    kind: &'static str,
    code: &'static str,
    served_backend: &'static str,
    execution_mode: &'static str,
    execution_engine: &'static str,
}

#[derive(Clone, Debug)]
struct CsmSpeechHttpError {
    status: StatusCode,
    message: String,
    kind: &'static str,
    code: &'static str,
}

impl CsmSpeechHttpError {
    fn invalid_request(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            kind: "invalid_request_error",
            code,
        }
    }

    fn unsupported_format() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "CSM speech currently supports response_format=wav only".to_string(),
            kind: "invalid_request_error",
            code: "unsupported_response_format",
        }
    }

    fn missing_voice_profile() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "requested CSM voice profile is not available".to_string(),
            kind: "invalid_request_error",
            code: "voice_profile_unavailable",
        }
    }

    fn unapproved_voice_profile() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "requested CSM voice profile is not approved for this runtime".to_string(),
            kind: "invalid_request_error",
            code: "voice_profile_unapproved",
        }
    }

    fn model_unavailable() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: "requested CSM model is not available on this Psionic speech server"
                .to_string(),
            kind: "not_found_error",
            code: "model_unavailable",
        }
    }

    fn runtime_unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            kind: "backend_unavailable",
            code,
        }
    }
}

impl IntoResponse for CsmSpeechHttpError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(CsmSpeechErrorEnvelope {
                error: CsmSpeechErrorBody {
                    message: self.message,
                    kind: self.kind,
                    code: self.code,
                    served_backend: CSM_SPEECH_DEFAULT_BACKEND,
                    execution_mode: CSM_SPEECH_EXECUTION_MODE,
                    execution_engine: CSM_SPEECH_DEFAULT_EXECUTION_ENGINE,
                },
            }),
        )
            .into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    fn disabled_runtime_config() -> CsmSpeechServerConfig {
        CsmSpeechServerConfig {
            runtime_enabled: false,
            ..CsmSpeechServerConfig::default()
        }
    }

    #[test]
    fn csm_generation_params_allow_current_autopilot_ack_window() {
        validate_params(&CsmGenerationParams {
            max_audio_length_ms: Some(15_000),
            ..CsmGenerationParams::default()
        })
        .expect("current Autopilot bounded answer window should be accepted");

        let error = validate_params(&CsmGenerationParams {
            max_audio_length_ms: Some(CSM_SPEECH_MAX_AUDIO_LENGTH_MS + 1),
            ..CsmGenerationParams::default()
        })
        .expect_err("overlarge CSM ack window should fail closed");
        assert_eq!(error.code, "invalid_max_audio_length_ms");
        assert!(error.message.contains("20000"));
    }

    #[tokio::test]
    async fn csm_speech_route_refuses_when_runtime_is_disabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSM_SPEECH_ROUTE_OPENAI)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request_id": "req_test_disabled_1",
                        "model": CSM_SPEECH_MODEL_ID,
                        "input": "hello from psionic",
                        "voice_profile_id": CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID,
                        "response_format": "wav",
                        "psionic_csm": {
                            "temperature": 0.1,
                            "top_k": 1,
                            "max_audio_length_ms": 250
                        }
                    }))?))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-psionic-execution-engine")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_SPEECH_DEFAULT_EXECUTION_ENGINE)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-request-id")
                .and_then(|value| value.to_str().ok()),
            Some("req_test_disabled_1")
        );
        assert!(
            response
                .headers()
                .get("x-psionic-csm-artifact-id")
                .is_some()
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-governance-schema")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-license-posture")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_LICENSE_POSTURE)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-runtime-image-ref")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_RUNTIME_IMAGE_REF_UNSET)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-voice-profile-id")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-source-prompt-profile-id")
                .and_then(|value| value.to_str().ok()),
            Some("conversational_b")
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-watermark-refusal-code")
                .and_then(|value| value.to_str().ok()),
            Some("csm_watermarking_unavailable")
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-promotion-gate")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE)
        );
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            payload["error"]["code"],
            serde_json::json!("runtime_disabled")
        );
        assert!(!String::from_utf8(body.to_vec())?.contains("/Users/"));
        Ok(())
    }

    #[tokio::test]
    async fn csm_worker_metadata_publishes_rpc_contract_and_metrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri(CSM_SPEECH_ROUTE_WORKER_METADATA)
                    .body(Body::empty())
                    .expect("worker metadata request"),
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            payload["worker"]["rpc_schema"],
            serde_json::json!("psionic.csm.speech.worker.v1")
        );
        assert!(
            payload["worker"]["request_fields"]
                .as_array()
                .is_some_and(|fields| fields.contains(&serde_json::json!("request_id"))
                    && fields.contains(&serde_json::json!("cancellation_id"))
                    && fields.contains(&serde_json::json!("timeout_ms"))
                    && fields.contains(&serde_json::json!("artifact_id")))
        );
        assert!(
            payload["worker"]["metrics"]
                .as_array()
                .is_some_and(
                    |metrics| metrics.contains(&serde_json::json!("queue_depth"))
                        && metrics.contains(&serde_json::json!("full_generation_latency_ms"))
                )
        );
        assert!(
            payload["worker"]["governance_fields"]
                .as_array()
                .is_some_and(
                    |fields| fields.contains(&serde_json::json!("license_posture"))
                        && fields.contains(&serde_json::json!("watermark_status"))
                        && fields.contains(&serde_json::json!("rollback_target"))
                )
        );
        assert_eq!(
            payload["worker"]["business_authority"],
            serde_json::json!("none_provider_output_is_evidence_not_instruction")
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_speech_route_validates_worker_request_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSM_SPEECH_ROUTE_PSIONIC)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request_id": "req_worker_controls",
                        "input": "hello",
                        "voice_profile_id": CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID,
                        "artifact_id": "wrong-artifact",
                        "timeout_ms": 100,
                        "cancellation_id": "cancel_worker_controls"
                    }))?))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            payload["error"]["code"],
            serde_json::json!("artifact_unavailable")
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_speech_route_refuses_missing_voice_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSM_SPEECH_ROUTE_PSIONIC)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "input": "hello",
                        "voice_profile_id": "missing"
                    }))?))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            payload["error"]["code"],
            serde_json::json!("voice_profile_unavailable")
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_speech_route_refuses_raw_source_prompt_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSM_SPEECH_ROUTE_PSIONIC)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "input": "hello",
                        "voice_profile_id": "conversational_b"
                    }))?))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            payload["error"]["code"],
            serde_json::json!("voice_profile_unavailable")
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_health_publishes_voice_profiles_artifacts_and_refusal_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(payload["status"], serde_json::json!("degraded"));
        assert_eq!(
            payload["execution_engine"],
            CSM_SPEECH_DEFAULT_EXECUTION_ENGINE
        );
        assert_eq!(
            payload["runtime"]["state"],
            serde_json::json!("unavailable")
        );
        assert_eq!(
            payload["execution_refusal"]["code"],
            serde_json::json!("runtime_disabled")
        );
        assert_eq!(
            payload["artifact_descriptor"]["frame_contract"]["frame_lanes"],
            serde_json::json!(33)
        );
        assert_eq!(
            payload["codec_capabilities"]["mimi_decode"],
            serde_json::json!("implemented")
        );
        assert_eq!(
            payload["codec_capabilities"]["reference_audio_encode_refusal"]["code"],
            serde_json::json!("rust_mimi_encode_not_implemented")
        );
        assert_eq!(
            payload["safety_capabilities"]["watermarking"],
            serde_json::json!(CSM_WATERMARK_OPERATOR_DOGFOOD)
        );
        assert_eq!(
            payload["safety_capabilities"]["watermarking_refusal"]["code"],
            serde_json::json!("csm_watermarking_unavailable")
        );
        assert_eq!(
            payload["governance"]["schema_version"],
            serde_json::json!(CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION)
        );
        assert_eq!(
            payload["governance"]["license_posture"],
            serde_json::json!(CSM_LICENSE_POSTURE)
        );
        assert_eq!(
            payload["governance"]["canary_promotion"],
            serde_json::json!(CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE)
        );
        assert_eq!(
            payload["governance"]["runtime_image_ref"],
            serde_json::json!(CSM_RUNTIME_IMAGE_REF_UNSET)
        );
        assert!(
            payload["governance"]["disallowed_voice_use_cases"]
                .as_array()
                .is_some_and(
                    |use_cases| use_cases.contains(&serde_json::json!("customer_voice_clone"))
                )
        );
        assert!(
            payload["voice_profiles"]
                .as_array()
                .is_some_and(|profiles| profiles.iter().any(|profile| {
                    profile["id"] == serde_json::json!(CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID)
                        && profile["source_prompt_profile_id"]
                            == serde_json::json!("conversational_b")
                        && profile["approval_status"]
                            == serde_json::json!("approved_openagents_operated_dogfood")
                        && profile["watermarking"]
                            == serde_json::json!(CSM_WATERMARK_OPERATOR_DOGFOOD)
                }))
        );
        assert_eq!(
            payload["voice_profiles"][0]["prompt_codebooks"]["codebook_count"],
            serde_json::json!(32)
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_models_publish_openai_and_psionic_speech_routes()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(disabled_runtime_config())?;
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .expect("models request"),
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        let endpoints = payload["data"][0]["psionic_supported_endpoints"]
            .as_array()
            .expect("endpoints");
        assert!(endpoints.contains(&serde_json::json!(CSM_SPEECH_ROUTE_OPENAI)));
        assert!(endpoints.contains(&serde_json::json!(CSM_SPEECH_ROUTE_PSIONIC)));
        assert_eq!(
            payload["data"][0]["psionic_voice_profiles"][0]["id"],
            serde_json::json!(CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID)
        );
        assert_eq!(
            payload["data"][0]["psionic_safety_capabilities"]["watermarking_refusal"]["code"],
            serde_json::json!("csm_watermarking_unavailable")
        );
        assert_eq!(
            payload["data"][0]["psionic_governance"]["primary_promotion"],
            serde_json::json!(CSM_PROMOTION_BLOCKED_WITHOUT_GOVERNANCE)
        );
        Ok(())
    }

    #[tokio::test]
    async fn csm_speech_route_serves_wav_when_runtime_artifacts_are_present()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = CsmSpeechServer::from_config(CsmSpeechServerConfig::default())?;
        if !server
            .state
            .runtime
            .lock()
            .expect("runtime lock")
            .is_ready()
        {
            eprintln!("skipping CSM serving test because gated runtime artifacts are unavailable");
            return Ok(());
        }

        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSM_SPEECH_ROUTE_OPENAI)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "model": CSM_SPEECH_MODEL_ID,
                    "input": "hello from psionic",
                    "voice_profile_id": CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID,
                        "response_format": "wav",
                        "psionic_csm": {
                            "max_audio_length_ms": 160,
                            "context_policy": "prompt_profile_only"
                        }
                    }))?))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("audio/wav")
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-residency-mode")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_SPEECH_DEFAULT_RESIDENCY_MODE)
        );
        assert_eq!(
            response
                .headers()
                .get("x-psionic-csm-watermarking")
                .and_then(|value| value.to_str().ok()),
            Some(CSM_WATERMARK_OPERATOR_DOGFOOD)
        );
        assert!(
            response
                .headers()
                .get("x-psionic-full-generation-latency-ms")
                .is_some()
        );
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        assert!(body.starts_with(b"RIFF"));
        assert!(body.len() > 44);
        Ok(())
    }

    #[test]
    fn csm_prompt_profile_only_has_full_default_female_context_codebooks()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = csm_python_parity_fixture()?;
        let prefix = fixture
            .mimi_codebook_prefixes
            .iter()
            .find(|prefix| prefix.profile_id == "conversational_b")
            .expect("default female source prompt codebooks");
        let frames = full_prompt_codebook_frames(prefix).expect("full prompt codebooks");

        assert_eq!(prefix.prefix_codebook_count, CSM_AUDIO_CODEBOOK_LANES);
        assert_eq!(prefix.prefix_frame_count, prefix.frame_count);
        assert_eq!(frames.len(), 375);
        assert_eq!(frames[0][0], 1049);
        assert_eq!(frames[0][31], 1902);
        assert_eq!(frames[374].len(), CSM_AUDIO_CODEBOOK_LANES);
        Ok(())
    }

    #[test]
    fn csm_multipart_stream_body_carries_binary_chunks_and_terminal_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let terminal = CsmSpeechStreamTerminalMetadata {
            event: "terminal",
            model: CSM_SPEECH_MODEL_ID.to_string(),
            artifact_id: format!("{CSM_SPEECH_MODEL_ID}@sha256:test"),
            governance_schema: CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION,
            license_posture: CSM_LICENSE_POSTURE,
            voice_profile_id: CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID.to_string(),
            runtime_admission: "admitted_openagents_operated_autopilot_production_dogfood"
                .to_string(),
            watermarking: CSM_WATERMARK_OPERATOR_DOGFOOD.to_string(),
            watermarking_refusal_code: "csm_watermarking_unavailable".to_string(),
            backend: CSM_SPEECH_DEFAULT_BACKEND.to_string(),
            execution_engine: CSM_SPEECH_DEFAULT_EXECUTION_ENGINE.to_string(),
            residency: CSM_SPEECH_DEFAULT_RESIDENCY_MODE.to_string(),
            accelerated_backend: "unavailable_fail_closed".to_string(),
            gpu_model: "none".to_string(),
            cpu_fallback_reason: None,
            generated_frame_count: 2,
            prompt_frame_count: 6,
            hit_eos: false,
            first_audio_latency_ms: 12,
            full_generation_latency_ms: 34,
            output_duration_ms: 160,
            wav_bytes: 8,
            chunk_count: 2,
            codebook_frames_sha256: "sha256:frames".to_string(),
            wav_pcm16_digest: "sha256:wav".to_string(),
        };
        let body =
            csm_multipart_stream_body(vec![b"RIFF".as_slice(), b"WAVE".as_slice()], &terminal)
                .expect("stream body");
        let text = String::from_utf8_lossy(&body);

        assert!(text.contains("Content-Type: audio/wav"));
        assert!(text.contains("X-Psionic-Chunk-Index: 0"));
        assert!(text.contains("\"event\":\"terminal\""));
        assert!(body.windows(4).any(|window| window == b"RIFF"));
        Ok(())
    }

    #[test]
    fn csm_jsonl_stream_parts_carry_base64_audio_and_terminal_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let chunk = CsmSpeechStreamChunk {
            index: 0,
            wav_bytes: b"RIFF".to_vec(),
            duration_ms: 160,
            generated_frame_count: 8,
            final_chunk: true,
            elapsed_ms: 42,
        };
        let audio_line = csm_stream_audio_part(CsmSpeechStreamFormat::JsonlBase64, &chunk);
        let audio_json = serde_json::from_slice::<serde_json::Value>(&audio_line)?;

        assert_eq!(audio_json["event"], "audio");
        assert_eq!(audio_json["chunk_transport"], serde_json::Value::Null);
        assert_eq!(audio_json["chunk_index"], 0);
        assert_eq!(audio_json["data_base64"], "UklGRg==");
        assert_eq!(audio_json["final_chunk"], true);

        let terminal = CsmSpeechStreamTerminalMetadata {
            event: "terminal",
            model: CSM_SPEECH_MODEL_ID.to_string(),
            artifact_id: format!("{CSM_SPEECH_MODEL_ID}@sha256:test"),
            governance_schema: CSM_ARTIFACT_GOVERNANCE_SCHEMA_VERSION,
            license_posture: CSM_LICENSE_POSTURE,
            voice_profile_id: CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID.to_string(),
            runtime_admission: "admitted_openagents_operated_autopilot_production_dogfood"
                .to_string(),
            watermarking: CSM_WATERMARK_OPERATOR_DOGFOOD.to_string(),
            watermarking_refusal_code: "csm_watermarking_unavailable".to_string(),
            backend: CSM_SPEECH_DEFAULT_BACKEND.to_string(),
            execution_engine: CSM_SPEECH_DEFAULT_EXECUTION_ENGINE.to_string(),
            residency: CSM_SPEECH_DEFAULT_RESIDENCY_MODE.to_string(),
            accelerated_backend: "unavailable_fail_closed".to_string(),
            gpu_model: "none".to_string(),
            cpu_fallback_reason: None,
            generated_frame_count: 8,
            prompt_frame_count: 6,
            hit_eos: false,
            first_audio_latency_ms: 42,
            full_generation_latency_ms: 84,
            output_duration_ms: 160,
            wav_bytes: 4,
            chunk_count: 1,
            codebook_frames_sha256: "sha256:frames".to_string(),
            wav_pcm16_digest: "sha256:wav".to_string(),
        };
        let terminal_line = csm_stream_terminal_part(CsmSpeechStreamFormat::JsonlBase64, &terminal)
            .expect("JSONL terminal metadata");
        let terminal_json = serde_json::from_slice::<serde_json::Value>(&terminal_line)?;

        assert_eq!(terminal_json["event"], "terminal");
        assert_eq!(terminal_json["chunk_transport"], "jsonl_base64");
        assert_eq!(terminal_json["chunk_count"], 1);
        Ok(())
    }
}
