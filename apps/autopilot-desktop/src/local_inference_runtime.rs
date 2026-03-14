use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::state::job_inbox::JobExecutionParam;
use openagents_kernel_core::ids::sha256_prefixed_text;
use psionic_runtime::{
    BackendRuntimeResources, BatchExecutionPosture, CacheInvalidationTrigger, CompilePathEvidence,
    CompilePathTemperature, DeviceInventoryQualifiers, LocalRuntimeObservability, QueueDiscipline,
};
use psionic_serve::{
    CpuGgufGptOssTextGenerationService, CpuReferenceTextGenerationService,
    CudaGgufGptOssTextGenerationService, GenerationLoadState, GenerationOptions, GenerationRequest,
    ManagedTextGenerationRuntime, MetalGgufGptOssTextGenerationService, TextGenerationExecutor,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const DEFAULT_GPT_OSS_KEEPALIVE_MILLIS: u64 = 300_000;
const ENV_GPT_OSS_BACKEND: &str = "OPENAGENTS_GPT_OSS_BACKEND";
const ENV_GPT_OSS_MODEL_PATH: &str = "OPENAGENTS_GPT_OSS_MODEL_PATH";
const GPT_OSS_20B_FILENAME: &str = "gpt-oss-20b-mxfp4.gguf";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionMetrics {
    pub total_duration_ns: Option<u64>,
    pub load_duration_ns: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration_ns: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration_ns: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionProvenance {
    pub backend: String,
    pub requested_model: Option<String>,
    pub served_model: String,
    pub normalized_prompt_digest: String,
    pub normalized_options_json: String,
    pub normalized_options_digest: String,
    pub base_url: String,
    pub total_duration_ns: Option<u64>,
    pub load_duration_ns: Option<u64>,
    pub prompt_token_count: Option<u64>,
    pub generated_token_count: Option<u64>,
    pub warm_start: Option<bool>,
}

impl LocalInferenceExecutionProvenance {
    pub fn receipt_payload(&self) -> Value {
        json!({
            "backend": self.backend,
            "requested_model": self.requested_model,
            "served_model": self.served_model,
            "normalized_prompt_digest": self.normalized_prompt_digest,
            "normalized_options": self.normalized_options_value(),
            "normalized_options_digest": self.normalized_options_digest,
            "base_url": self.base_url,
            "metrics": {
                "total_duration_ns": self.total_duration_ns,
                "load_duration_ns": self.load_duration_ns,
                "prompt_token_count": self.prompt_token_count,
                "generated_token_count": self.generated_token_count,
            },
            "warm_start": self.warm_start,
        })
    }

    fn normalized_options_value(&self) -> Value {
        serde_json::from_str(self.normalized_options_json.as_str())
            .unwrap_or_else(|_| Value::String(self.normalized_options_json.clone()))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRuntimeExecutionPosture {
    #[default]
    Cold,
    Warming,
    Warm,
    CompileFailed,
    CacheInvalidated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRuntimeCacheInvalidationReason {
    ExplicitReset,
    BackendChange,
    ModelChange,
    ModelMetadataChange,
    DTypeChange,
    ShapeChange,
    ExecutionProfileChange,
    RuntimeBinaryUpgrade,
    BackendToolchainUpgrade,
    TokenizerDrift,
    ChatTemplateDrift,
    GenerationDefaultsDrift,
    QuantizationChange,
    PlanFormatUpgrade,
    KernelFormatUpgrade,
    PagedTensorFormatUpgrade,
    PrefixCacheFormatUpgrade,
    KvStateFormatUpgrade,
    ClusterRouteChange,
    ClusterTopologyChange,
}

impl LocalRuntimeCacheInvalidationReason {
    fn from_trigger(trigger: CacheInvalidationTrigger) -> Self {
        match trigger {
            CacheInvalidationTrigger::BinaryUpgrade => Self::RuntimeBinaryUpgrade,
            CacheInvalidationTrigger::BackendToolchainUpgrade => Self::BackendToolchainUpgrade,
            CacheInvalidationTrigger::ModelMetadataChange => Self::ModelMetadataChange,
            CacheInvalidationTrigger::TokenizerDrift => Self::TokenizerDrift,
            CacheInvalidationTrigger::ChatTemplateDrift => Self::ChatTemplateDrift,
            CacheInvalidationTrigger::GenerationDefaultsDrift => Self::GenerationDefaultsDrift,
            CacheInvalidationTrigger::QuantizationChange => Self::QuantizationChange,
            CacheInvalidationTrigger::PlanFormatUpgrade => Self::PlanFormatUpgrade,
            CacheInvalidationTrigger::KernelFormatUpgrade => Self::KernelFormatUpgrade,
            CacheInvalidationTrigger::PagedTensorFormatUpgrade => Self::PagedTensorFormatUpgrade,
            CacheInvalidationTrigger::PrefixCacheFormatUpgrade => Self::PrefixCacheFormatUpgrade,
            CacheInvalidationTrigger::KvStateFormatUpgrade => Self::KvStateFormatUpgrade,
            CacheInvalidationTrigger::ClusterRouteChange => Self::ClusterRouteChange,
            CacheInvalidationTrigger::ClusterTopologyChange => Self::ClusterTopologyChange,
            CacheInvalidationTrigger::ExplicitReset => Self::ExplicitReset,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalRuntimeCacheInvalidation {
    pub reason: LocalRuntimeCacheInvalidationReason,
    pub summary: String,
    pub observed_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalRuntimeCompileFailure {
    pub summary: String,
    pub observed_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalRuntimeDiagnostics {
    pub posture: LocalRuntimeExecutionPosture,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observability: Option<LocalRuntimeObservability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_resources: Option<BackendRuntimeResources>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_compile_path: Option<CompilePathEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_compile_path_observed_at_epoch_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_cold_compile_duration_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_warm_refresh_duration_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_cache_invalidation: Option<LocalRuntimeCacheInvalidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_compile_failure: Option<LocalRuntimeCompileFailure>,
}

impl LocalRuntimeDiagnostics {
    fn refresh_posture(&mut self, busy: bool) {
        self.posture = if busy {
            LocalRuntimeExecutionPosture::Warming
        } else if self.has_active_compile_failure() {
            LocalRuntimeExecutionPosture::CompileFailed
        } else if self.has_active_cache_invalidation() {
            LocalRuntimeExecutionPosture::CacheInvalidated
        } else if self
            .last_compile_path
            .as_ref()
            .is_some_and(|value| value.temperature == CompilePathTemperature::WarmReuse)
        {
            LocalRuntimeExecutionPosture::Warm
        } else {
            LocalRuntimeExecutionPosture::Cold
        };
    }

    fn has_active_cache_invalidation(&self) -> bool {
        match (
            self.last_cache_invalidation.as_ref(),
            self.last_compile_path_observed_at_epoch_ms,
        ) {
            (Some(invalidation), Some(compiled_at)) => {
                invalidation.observed_at_epoch_ms >= compiled_at
            }
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    fn has_active_compile_failure(&self) -> bool {
        match (
            self.last_compile_failure.as_ref(),
            self.last_compile_path_observed_at_epoch_ms,
        ) {
            (Some(failure), Some(compiled_at)) => failure.observed_at_epoch_ms >= compiled_at,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

pub fn local_runtime_execution_posture_label(
    posture: LocalRuntimeExecutionPosture,
) -> &'static str {
    match posture {
        LocalRuntimeExecutionPosture::Cold => "cold",
        LocalRuntimeExecutionPosture::Warming => "warming",
        LocalRuntimeExecutionPosture::Warm => "warm",
        LocalRuntimeExecutionPosture::CompileFailed => "compile_failed",
        LocalRuntimeExecutionPosture::CacheInvalidated => "cache_invalidated",
    }
}

pub fn local_runtime_device_inventory_label(device: &DeviceInventoryQualifiers) -> String {
    let topology = device.topology_key.as_deref().unwrap_or("-");
    let total_memory_bytes = device
        .total_memory_bytes
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let free_memory_bytes = device
        .free_memory_bytes
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    format!(
        "id={} topology={} performance_class={:?} memory_class={:?} total_bytes={} free_bytes={}",
        device.stable_device_id,
        topology,
        device.performance_class,
        device.memory_class,
        total_memory_bytes,
        free_memory_bytes
    )
}

pub fn local_runtime_cache_invalidation_reason_label(
    reason: LocalRuntimeCacheInvalidationReason,
) -> &'static str {
    match reason {
        LocalRuntimeCacheInvalidationReason::ExplicitReset => "explicit_reset",
        LocalRuntimeCacheInvalidationReason::BackendChange => "backend_change",
        LocalRuntimeCacheInvalidationReason::ModelChange => "model_change",
        LocalRuntimeCacheInvalidationReason::ModelMetadataChange => "model_metadata_change",
        LocalRuntimeCacheInvalidationReason::DTypeChange => "dtype_change",
        LocalRuntimeCacheInvalidationReason::ShapeChange => "shape_change",
        LocalRuntimeCacheInvalidationReason::ExecutionProfileChange => "execution_profile_change",
        LocalRuntimeCacheInvalidationReason::RuntimeBinaryUpgrade => "runtime_binary_upgrade",
        LocalRuntimeCacheInvalidationReason::BackendToolchainUpgrade => "backend_toolchain_upgrade",
        LocalRuntimeCacheInvalidationReason::TokenizerDrift => "tokenizer_drift",
        LocalRuntimeCacheInvalidationReason::ChatTemplateDrift => "chat_template_drift",
        LocalRuntimeCacheInvalidationReason::GenerationDefaultsDrift => "generation_defaults_drift",
        LocalRuntimeCacheInvalidationReason::QuantizationChange => "quantization_change",
        LocalRuntimeCacheInvalidationReason::PlanFormatUpgrade => "plan_format_upgrade",
        LocalRuntimeCacheInvalidationReason::KernelFormatUpgrade => "kernel_format_upgrade",
        LocalRuntimeCacheInvalidationReason::PagedTensorFormatUpgrade => {
            "paged_tensor_format_upgrade"
        }
        LocalRuntimeCacheInvalidationReason::PrefixCacheFormatUpgrade => {
            "prefix_cache_format_upgrade"
        }
        LocalRuntimeCacheInvalidationReason::KvStateFormatUpgrade => "kv_state_format_upgrade",
        LocalRuntimeCacheInvalidationReason::ClusterRouteChange => "cluster_route_change",
        LocalRuntimeCacheInvalidationReason::ClusterTopologyChange => "cluster_topology_change",
    }
}

pub fn compile_path_temperature_label(temperature: CompilePathTemperature) -> &'static str {
    match temperature {
        CompilePathTemperature::ColdCompile => "cold_compile",
        CompilePathTemperature::WarmReuse => "warm_reuse",
    }
}

pub fn local_runtime_scheduler_posture_label(
    diagnostics: &LocalRuntimeDiagnostics,
) -> Option<String> {
    let observability = diagnostics.observability.as_ref()?;
    let batch_posture = match observability.execution_profile.batch_posture {
        BatchExecutionPosture::SingleRequestOnly => "single_request_only",
        BatchExecutionPosture::CallerStaticBatch => "caller_static_batch",
        BatchExecutionPosture::SchedulerStaticBatch => "scheduler_static_batch",
        BatchExecutionPosture::ContinuousBatch => "continuous_batch",
    };
    let queue = match observability.execution_profile.queue_policy.discipline {
        QueueDiscipline::DirectCallerBackpressure => "direct_caller_backpressure",
        QueueDiscipline::Fifo => "fifo",
    };
    Some(format!(
        "{batch_posture}/{queue} active={} queued={}",
        observability
            .execution_profile
            .queue_policy
            .max_active_requests,
        observability
            .execution_profile
            .queue_policy
            .max_queued_requests
    ))
}

/// Backend-neutral local inference snapshot kept in the app seam.
#[derive(Clone, Debug, Default)]
pub struct LocalInferenceRuntimeSnapshot {
    pub base_url: String,
    pub reachable: bool,
    pub busy: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub configured_model_path: Option<String>,
    pub artifact_present: bool,
    pub backend_label: String,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub diagnostics: LocalRuntimeDiagnostics,
    pub refreshed_at: Option<Instant>,
}

impl LocalInferenceRuntimeSnapshot {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }
}

pub type LocalInferenceExecutionSnapshot = LocalInferenceRuntimeSnapshot;

/// App-owned local inference generation job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceGenerateJob {
    pub request_id: String,
    pub prompt: String,
    pub requested_model: Option<String>,
    pub params: Vec<JobExecutionParam>,
}

/// App-owned local inference runtime commands.
#[derive(Clone, Debug)]
pub enum LocalInferenceRuntimeCommand {
    Refresh,
    WarmConfiguredModel,
    UnloadConfiguredModel,
    Generate(LocalInferenceGenerateJob),
}

/// Local inference execution start notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionStarted {
    pub request_id: String,
    pub model: String,
}

/// Local inference execution completion notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionCompleted {
    pub request_id: String,
    pub model: String,
    pub output: String,
    pub metrics: LocalInferenceExecutionMetrics,
    pub provenance: LocalInferenceExecutionProvenance,
}

/// Local inference execution failure notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionFailed {
    pub request_id: String,
    pub error: String,
}

/// App-owned local inference runtime updates.
#[derive(Clone, Debug)]
pub enum LocalInferenceRuntimeUpdate {
    Snapshot(Box<LocalInferenceRuntimeSnapshot>),
    Started(LocalInferenceExecutionStarted),
    Completed(LocalInferenceExecutionCompleted),
    Failed(LocalInferenceExecutionFailed),
}

/// App-owned seam for the local inference runtime slot.
pub trait LocalInferenceRuntime {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String>;
    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate>;
}

pub fn default_local_inference_runtime() -> Result<Box<dyn LocalInferenceRuntime>, String> {
    GptOssRuntimeAdapter::new_auto()
        .map(|adapter| Box::new(adapter) as Box<dyn LocalInferenceRuntime>)
}

pub(crate) fn initial_local_inference_runtime_snapshot() -> LocalInferenceExecutionSnapshot {
    let model_path = configured_gpt_oss_model_path();
    configured_runtime_snapshot(model_path.as_path(), GptOssRuntimeBackend::from_env())
}

fn current_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn elapsed_ns(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn artifact_fingerprint(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or(0);
    Some(format!(
        "{}:{}:{}",
        path.display(),
        metadata.len(),
        modified
    ))
}

fn looks_like_compile_failure(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("compile")
        || normalized.contains("execution plan")
        || normalized.contains("decode-step plan")
        || normalized.contains("kernel cache")
}

/// In-process Psionic adapter for the app-owned runtime seam.
pub struct PsionicRuntimeAdapter {
    service: CpuReferenceTextGenerationService,
    snapshot: LocalInferenceRuntimeSnapshot,
    pending_updates: VecDeque<LocalInferenceRuntimeUpdate>,
}

impl PsionicRuntimeAdapter {
    pub fn new_reference() -> Result<Self, String> {
        let mut adapter = Self {
            service: CpuReferenceTextGenerationService::new().map_err(|error| {
                format!("failed to initialize Psionic reference runtime: {error}")
            })?,
            snapshot: LocalInferenceRuntimeSnapshot {
                base_url: String::from("in-process://psionic"),
                ..LocalInferenceRuntimeSnapshot::default()
            },
            pending_updates: VecDeque::new(),
        };
        adapter.refresh_snapshot(String::from("Psionic local runtime ready"));
        Ok(adapter)
    }

    fn configured_model_id(&self) -> String {
        self.service.model_descriptor().model.model_id.clone()
    }

    fn refresh_snapshot(&mut self, action: String) {
        let configured_model = self.configured_model_id();
        let loaded = self.service.loaded_models();
        let observability = self.service.observability();
        let loaded_models = loaded
            .models
            .into_iter()
            .map(|model| model.model)
            .collect::<Vec<_>>();
        self.snapshot.reachable = true;
        self.snapshot.configured_model = Some(configured_model.clone());
        self.snapshot.ready_model = Some(configured_model.clone());
        self.snapshot.available_models = vec![configured_model];
        self.snapshot.loaded_models = loaded_models;
        self.snapshot.last_action = Some(action);
        self.snapshot.last_error = None;
        self.snapshot.diagnostics.observability = Some(observability);
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
    }

    fn push_failure(&mut self, request_id: String, error: impl Into<String>) {
        let error = error.into();
        self.snapshot.last_request_id = Some(request_id.clone());
        self.snapshot.last_error = Some(error.clone());
        self.snapshot.last_action = Some(String::from("Psionic generation failed"));
        if looks_like_compile_failure(error.as_str()) {
            self.snapshot.diagnostics.last_compile_failure = Some(LocalRuntimeCompileFailure {
                summary: error.clone(),
                observed_at_epoch_ms: current_epoch_millis(),
            });
        }
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Failed(
                LocalInferenceExecutionFailed { request_id, error },
            ));
    }

    fn handle_generate(&mut self, job: LocalInferenceGenerateJob) {
        let normalized_prompt = normalize_prompt(job.prompt.as_str());
        let options_map = match build_generate_options(job.params.as_slice()) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let configured_model = self.configured_model_id();
        if let Some(requested_model) = job
            .requested_model
            .as_deref()
            .and_then(normalize_optional_text)
            && requested_model != configured_model
        {
            self.push_failure(
                job.request_id,
                format!(
                    "Psionic runtime only has '{}' configured, but '{}' was requested",
                    configured_model, requested_model
                ),
            );
            return;
        }

        let model = configured_model;
        self.snapshot.last_request_id = Some(job.request_id.clone());
        self.snapshot.last_error = None;
        self.snapshot.last_action =
            Some(format!("Psionic generation queued for {}", job.request_id));
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Started(
                LocalInferenceExecutionStarted {
                    request_id: job.request_id.clone(),
                    model: model.clone(),
                },
            ));

        let options = match psionic_generation_options(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let request = GenerationRequest::new_text(
            job.request_id.as_str(),
            self.service.model_descriptor().clone(),
            None,
            normalized_prompt.as_str(),
            options,
        );
        let response = match self.service.generate(&request) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(
                    job.request_id,
                    format!("Psionic local generation failed: {error}"),
                );
                return;
            }
        };
        let normalized_options_json = match canonical_options_json(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let normalized_options_digest = sha256_prefixed_text(normalized_options_json.as_str());
        let normalized_prompt_digest = sha256_prefixed_text(normalized_prompt.as_str());
        let provenance = LocalInferenceExecutionProvenance {
            backend: String::from("psionic"),
            requested_model: job.requested_model.clone(),
            served_model: response.model_id.clone(),
            normalized_prompt_digest,
            normalized_options_json,
            normalized_options_digest,
            base_url: self.snapshot.base_url.clone(),
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_token_count: u64::try_from(response.usage.input_tokens).ok(),
            generated_token_count: u64::try_from(response.usage.output_tokens).ok(),
            warm_start: response
                .provenance
                .as_ref()
                .and_then(|value| match value.load_state {
                    GenerationLoadState::Cold => Some(false),
                    GenerationLoadState::Warm => Some(true),
                }),
        };
        let metrics = LocalInferenceExecutionMetrics {
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_eval_count: response
                .metrics
                .prompt_eval_count
                .and_then(|value| u64::try_from(value).ok()),
            prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
            eval_count: response
                .metrics
                .eval_count
                .and_then(|value| u64::try_from(value).ok()),
            eval_duration_ns: response.metrics.eval_duration_ns,
        };
        if let Some(provenance) = response.provenance.as_ref() {
            if let Some(compile_path) = provenance.compile_path.clone() {
                self.snapshot.diagnostics.last_compile_path = Some(compile_path.clone());
                self.snapshot
                    .diagnostics
                    .last_compile_path_observed_at_epoch_ms = Some(current_epoch_millis());
                if compile_path.temperature == CompilePathTemperature::ColdCompile {
                    self.snapshot.diagnostics.last_cold_compile_duration_ns =
                        response.metrics.total_duration_ns;
                }
            }
        }
        self.snapshot.last_metrics = Some(metrics.clone());
        self.snapshot.last_action = Some(format!(
            "Psionic generation completed for {}",
            job.request_id
        ));
        self.snapshot.last_error = None;
        self.snapshot.refreshed_at = Some(Instant::now());
        self.refresh_snapshot(String::from(
            "Psionic local runtime refreshed after generation",
        ));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Completed(
                LocalInferenceExecutionCompleted {
                    request_id: job.request_id,
                    model,
                    output: response.output.text,
                    metrics,
                    provenance,
                },
            ));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GptOssRuntimeBackend {
    Auto,
    Cpu,
    Cuda,
    Metal,
}

impl GptOssRuntimeBackend {
    fn from_env() -> Self {
        match std::env::var(ENV_GPT_OSS_BACKEND).ok().as_deref() {
            Some("cpu") => Self::Cpu,
            Some("cuda") => Self::Cuda,
            Some("metal") => Self::Metal,
            _ => Self::Auto,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
        }
    }

    fn candidate_backends(self) -> Vec<Self> {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    vec![Self::Metal, Self::Cpu]
                } else if cfg!(target_os = "linux") {
                    vec![Self::Cuda, Self::Cpu]
                } else {
                    vec![Self::Cpu]
                }
            }
            backend => vec![backend],
        }
    }

    fn default_runtime_label(self) -> String {
        match self {
            Self::Auto => self
                .candidate_backends()
                .first()
                .copied()
                .unwrap_or(Self::Cpu)
                .label()
                .to_string(),
            backend => backend.label().to_string(),
        }
    }
}

enum GptOssRuntimeService {
    Cpu(CpuGgufGptOssTextGenerationService),
    Cuda(CudaGgufGptOssTextGenerationService),
    Metal(MetalGgufGptOssTextGenerationService),
}

impl GptOssRuntimeService {
    fn load(path: &Path, backend: GptOssRuntimeBackend) -> Result<Self, String> {
        match backend {
            GptOssRuntimeBackend::Cpu => CpuGgufGptOssTextGenerationService::from_gguf_path(path)
                .map(Self::Cpu)
                .map_err(|error| error.to_string()),
            GptOssRuntimeBackend::Cuda => CudaGgufGptOssTextGenerationService::from_gguf_path(path)
                .map(Self::Cuda)
                .map_err(|error| error.to_string()),
            GptOssRuntimeBackend::Metal => {
                MetalGgufGptOssTextGenerationService::from_gguf_path(path)
                    .map(Self::Metal)
                    .map_err(|error| error.to_string())
            }
            GptOssRuntimeBackend::Auto => Err(String::from(
                "auto backend must be resolved before service load",
            )),
        }
    }

    fn backend_label(&self) -> &'static str {
        match self {
            Self::Cpu(_) => "cpu",
            Self::Cuda(_) => "cuda",
            Self::Metal(_) => "metal",
        }
    }

    fn model_id(&self) -> &str {
        match self {
            Self::Cpu(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Cuda(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Metal(service) => service.model_descriptor().model.model_id.as_str(),
        }
    }

    fn model_descriptor(&self) -> &psionic_serve::DecoderModelDescriptor {
        match self {
            Self::Cpu(service) => service.model_descriptor(),
            Self::Cuda(service) => service.model_descriptor(),
            Self::Metal(service) => service.model_descriptor(),
        }
    }

    fn loaded_model_names(&mut self) -> Vec<String> {
        match self {
            Self::Cpu(service) => service
                .loaded_models()
                .models
                .into_iter()
                .map(|model| model.model)
                .collect(),
            Self::Cuda(service) => service
                .loaded_models()
                .models
                .into_iter()
                .map(|model| model.model)
                .collect(),
            Self::Metal(service) => service
                .loaded_models()
                .models
                .into_iter()
                .map(|model| model.model)
                .collect(),
        }
    }

    fn observability(&mut self) -> LocalRuntimeObservability {
        match self {
            Self::Cpu(service) => service.observability(),
            Self::Cuda(service) => service.observability(),
            Self::Metal(service) => service.observability(),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match self {
            Self::Cpu(service) => service.runtime_resources(),
            Self::Cuda(service) => service.runtime_resources(),
            Self::Metal(service) => service.runtime_resources(),
        }
    }

    fn selected_devices(&self) -> Vec<DeviceInventoryQualifiers> {
        match self {
            Self::Cpu(_) => Vec::new(),
            Self::Cuda(service) => service.backend_selection().selected_devices_inventory(),
            Self::Metal(service) => service.backend_selection().selected_devices_inventory(),
        }
    }

    fn warm_model(&mut self, keep_alive_millis: u64) -> Result<(), String> {
        let model_id = self.model_id().to_string();
        match self {
            Self::Cpu(service) => service
                .warm_model(model_id.as_str(), keep_alive_millis)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Self::Cuda(service) => service
                .warm_model(model_id.as_str(), keep_alive_millis)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Self::Metal(service) => service
                .warm_model(model_id.as_str(), keep_alive_millis)
                .map(|_| ())
                .map_err(|error| error.to_string()),
        }
    }

    fn unload_model(&mut self) -> Result<(), String> {
        let model_id = self.model_id().to_string();
        match self {
            Self::Cpu(service) => service
                .unload_model(model_id.as_str())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Self::Cuda(service) => service
                .unload_model(model_id.as_str())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Self::Metal(service) => service
                .unload_model(model_id.as_str())
                .map(|_| ())
                .map_err(|error| error.to_string()),
        }
    }

    fn generate(
        &mut self,
        request: &GenerationRequest,
    ) -> Result<psionic_serve::GenerationResponse, String> {
        match self {
            Self::Cpu(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Cuda(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Metal(service) => service.generate(request).map_err(|error| error.to_string()),
        }
    }
}

pub struct GptOssRuntimeAdapter {
    command_tx: Sender<LocalInferenceRuntimeCommand>,
    update_rx: Receiver<LocalInferenceRuntimeUpdate>,
}

impl GptOssRuntimeAdapter {
    pub fn new_auto() -> Result<Self, String> {
        Self::spawn(
            configured_gpt_oss_model_path(),
            GptOssRuntimeBackend::from_env(),
        )
    }

    fn spawn(model_path: PathBuf, backend: GptOssRuntimeBackend) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<LocalInferenceRuntimeCommand>();
        let (update_tx, update_rx) = mpsc::channel::<LocalInferenceRuntimeUpdate>();
        std::thread::Builder::new()
            .name(String::from("autopilot-gpt-oss"))
            .spawn(move || {
                let mut worker = GptOssRuntimeWorker::new(model_path, backend, update_tx);
                worker.push_snapshot();
                while let Ok(command) = command_rx.recv() {
                    worker.handle_command(command);
                }
            })
            .map_err(|error| format!("failed to spawn GPT-OSS local runtime worker: {error}"))?;
        Ok(Self {
            command_tx,
            update_rx,
        })
    }
}

impl LocalInferenceRuntime for GptOssRuntimeAdapter {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("failed to enqueue GPT-OSS local runtime command: {error}"))
    }

    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

struct GptOssRuntimeWorker {
    configured_backend: GptOssRuntimeBackend,
    configured_model_label: String,
    configured_artifact_fingerprint: Option<String>,
    model_path: PathBuf,
    service: Option<GptOssRuntimeService>,
    snapshot: LocalInferenceRuntimeSnapshot,
    update_tx: Sender<LocalInferenceRuntimeUpdate>,
}

impl GptOssRuntimeWorker {
    fn new(
        model_path: PathBuf,
        configured_backend: GptOssRuntimeBackend,
        update_tx: Sender<LocalInferenceRuntimeUpdate>,
    ) -> Self {
        let configured_model_label = configured_model_label(model_path.as_path());
        let snapshot = configured_runtime_snapshot(model_path.as_path(), configured_backend);
        Self {
            configured_backend,
            configured_model_label,
            configured_artifact_fingerprint: artifact_fingerprint(model_path.as_path()),
            model_path,
            service: None,
            snapshot,
            update_tx,
        }
    }

    fn handle_command(&mut self, command: LocalInferenceRuntimeCommand) {
        match command {
            LocalInferenceRuntimeCommand::Refresh => {
                self.reload_configuration();
                self.refresh_snapshot(String::from("GPT-OSS runtime refreshed"))
            }
            LocalInferenceRuntimeCommand::WarmConfiguredModel => self.handle_load_or_warm(),
            LocalInferenceRuntimeCommand::UnloadConfiguredModel => self.handle_unload(),
            LocalInferenceRuntimeCommand::Generate(job) => self.handle_generate(job),
        }
    }

    fn update_service_diagnostics(&mut self) {
        if let Some(service) = self.service.as_mut() {
            self.snapshot.diagnostics.observability = Some(service.observability());
            self.snapshot.diagnostics.runtime_resources = service.runtime_resources();
            self.snapshot.diagnostics.selected_devices = service.selected_devices();
        } else {
            self.snapshot.diagnostics.observability = None;
            self.snapshot.diagnostics.runtime_resources = None;
            self.snapshot.diagnostics.selected_devices.clear();
        }
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
    }

    fn record_cache_invalidation(
        &mut self,
        reason: LocalRuntimeCacheInvalidationReason,
        summary: impl Into<String>,
    ) {
        self.snapshot.diagnostics.last_cache_invalidation = Some(LocalRuntimeCacheInvalidation {
            reason,
            summary: summary.into(),
            observed_at_epoch_ms: current_epoch_millis(),
        });
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
    }

    fn record_compile_path(&mut self, compile_path: CompilePathEvidence, duration_ns: Option<u64>) {
        let observed_at_epoch_ms = current_epoch_millis();
        if compile_path.temperature == CompilePathTemperature::ColdCompile {
            self.snapshot.diagnostics.last_cold_compile_duration_ns = duration_ns;
        }
        self.snapshot.diagnostics.last_compile_path = Some(compile_path);
        self.snapshot
            .diagnostics
            .last_compile_path_observed_at_epoch_ms = Some(observed_at_epoch_ms);
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
    }

    fn maybe_record_compile_failure(&mut self, error: &str) {
        if !looks_like_compile_failure(error) {
            self.snapshot
                .diagnostics
                .refresh_posture(self.snapshot.busy);
            return;
        }
        self.snapshot.diagnostics.last_compile_failure = Some(LocalRuntimeCompileFailure {
            summary: error.to_string(),
            observed_at_epoch_ms: current_epoch_millis(),
        });
        self.snapshot
            .diagnostics
            .refresh_posture(self.snapshot.busy);
    }

    fn record_cache_invalidation_from_error(&mut self, error: &str) {
        if !looks_like_compile_failure(error) {
            return;
        }
        self.record_cache_invalidation(
            LocalRuntimeCacheInvalidationReason::ExecutionProfileChange,
            format!("runtime reported a compile-bearing failure: {error}"),
        );
    }

    fn record_cache_invalidation_from_provenance(
        &mut self,
        provenance: Option<&psionic_serve::GenerationProvenance>,
    ) {
        let Some(provenance) = provenance else {
            return;
        };
        if let Some(observation) = provenance
            .cache_observations
            .iter()
            .find(|observation| observation.trigger.is_some())
            && let Some(trigger) = observation.trigger
        {
            self.record_cache_invalidation(
                LocalRuntimeCacheInvalidationReason::from_trigger(trigger),
                observation.detail.clone(),
            );
        }
    }

    fn reload_configuration(&mut self) {
        let next_backend = GptOssRuntimeBackend::from_env();
        let next_model_path = configured_gpt_oss_model_path();
        let next_model_label = configured_model_label(next_model_path.as_path());
        let next_artifact_fingerprint = artifact_fingerprint(next_model_path.as_path());
        let backend_changed = next_backend != self.configured_backend;
        let model_changed =
            next_model_path != self.model_path || next_model_label != self.configured_model_label;
        let artifact_changed =
            !model_changed && next_artifact_fingerprint != self.configured_artifact_fingerprint;
        let previous_backend = self.configured_backend.default_runtime_label();
        let previous_model_path = self.model_path.display().to_string();

        self.configured_backend = next_backend;
        self.configured_model_label = next_model_label;
        self.model_path = next_model_path;
        self.configured_artifact_fingerprint = next_artifact_fingerprint;

        if !(backend_changed || model_changed || artifact_changed) {
            return;
        }

        self.service = None;
        self.snapshot.ready_model = None;
        self.snapshot.loaded_models.clear();
        self.snapshot.last_error = None;

        let (reason, summary) = if backend_changed {
            (
                LocalRuntimeCacheInvalidationReason::BackendChange,
                format!(
                    "local-runtime backend changed from {} to {} and invalidated cached execution state",
                    previous_backend,
                    self.configured_backend.default_runtime_label()
                ),
            )
        } else if model_changed {
            (
                LocalRuntimeCacheInvalidationReason::ModelChange,
                format!(
                    "local-runtime model changed from {} to {} and invalidated cached execution state",
                    previous_model_path,
                    self.model_path.display()
                ),
            )
        } else {
            (
                LocalRuntimeCacheInvalidationReason::ModelMetadataChange,
                format!(
                    "model artifact metadata changed for {} and invalidated cached execution state",
                    self.model_path.display()
                ),
            )
        };
        self.record_cache_invalidation(reason, summary);
    }

    fn refresh_snapshot(&mut self, action: String) {
        let artifact_present = self.model_path.is_file();
        self.snapshot.reachable = true;
        self.snapshot.artifact_present = artifact_present;
        self.snapshot.configured_model = Some(self.configured_model_label.clone());
        self.snapshot.configured_model_path = Some(self.model_path.display().to_string());
        self.snapshot.available_models = if artifact_present {
            vec![self.configured_model_label.clone()]
        } else {
            Vec::new()
        };
        if let Some(service) = self.service.as_mut() {
            self.snapshot.ready_model = Some(service.model_id().to_string());
            self.snapshot.loaded_models = service.loaded_model_names();
            self.snapshot.backend_label = service.backend_label().to_string();
            if !self
                .snapshot
                .available_models
                .iter()
                .any(|candidate| candidate == service.model_id())
            {
                self.snapshot
                    .available_models
                    .push(service.model_id().to_string());
            }
        } else {
            self.snapshot.ready_model = None;
            self.snapshot.loaded_models.clear();
            self.snapshot.backend_label = self.configured_backend.default_runtime_label();
        }
        self.snapshot.last_action = Some(action);
        if artifact_present {
            if self
                .snapshot
                .last_error
                .as_ref()
                .is_some_and(|error| error.contains("GPT-OSS 20B GGUF missing"))
            {
                self.snapshot.last_error = None;
            }
        } else {
            self.snapshot.last_error =
                Some(missing_gpt_oss_artifact_message(self.model_path.as_path()));
        }
        self.update_service_diagnostics();
        self.snapshot.refreshed_at = Some(Instant::now());
        self.push_snapshot();
    }

    fn handle_load_or_warm(&mut self) {
        self.reload_configuration();
        let started_at = Instant::now();
        self.snapshot.busy = true;
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some(format!(
            "Loading GPT-OSS 20B from {}",
            self.model_path.display()
        ));
        self.snapshot.diagnostics.refresh_posture(true);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.push_snapshot();

        if !self.model_path.is_file() {
            self.snapshot.busy = false;
            self.snapshot.last_action = Some(String::from("GPT-OSS 20B load blocked"));
            self.snapshot.last_error =
                Some(missing_gpt_oss_artifact_message(self.model_path.as_path()));
            self.snapshot.diagnostics.refresh_posture(false);
            self.snapshot.refreshed_at = Some(Instant::now());
            self.push_snapshot();
            return;
        }

        if self.service.is_none() {
            match self.load_service() {
                Ok(service) => {
                    self.service = Some(service);
                }
                Err(error) => {
                    self.snapshot.busy = false;
                    self.snapshot.last_action = Some(String::from("GPT-OSS 20B load failed"));
                    self.snapshot.last_error = Some(error);
                    if let Some(error) = self.snapshot.last_error.clone() {
                        self.maybe_record_compile_failure(error.as_str());
                        self.record_cache_invalidation_from_error(error.as_str());
                    }
                    self.snapshot.refreshed_at = Some(Instant::now());
                    self.push_snapshot();
                    return;
                }
            }
        }

        if let Some(service) = self.service.as_mut()
            && let Err(error) = service.warm_model(DEFAULT_GPT_OSS_KEEPALIVE_MILLIS)
        {
            self.snapshot.busy = false;
            self.snapshot.last_action = Some(String::from("GPT-OSS 20B warm failed"));
            self.snapshot.last_error = Some(error);
            if let Some(error) = self.snapshot.last_error.clone() {
                self.maybe_record_compile_failure(error.as_str());
                self.record_cache_invalidation_from_error(error.as_str());
            }
            self.snapshot.refreshed_at = Some(Instant::now());
            self.push_snapshot();
            return;
        }

        self.snapshot.busy = false;
        self.snapshot.diagnostics.last_warm_refresh_duration_ns = Some(elapsed_ns(started_at));
        let backend_label = self
            .service
            .as_ref()
            .map(GptOssRuntimeService::backend_label)
            .unwrap_or("unknown");
        self.refresh_snapshot(format!("GPT-OSS 20B loaded on {backend_label}"));
    }

    fn handle_unload(&mut self) {
        if let Some(service) = self.service.as_mut()
            && let Err(error) = service.unload_model()
        {
            self.snapshot.last_action = Some(String::from("GPT-OSS 20B unload failed"));
            self.snapshot.last_error = Some(error);
            if let Some(error) = self.snapshot.last_error.clone() {
                self.maybe_record_compile_failure(error.as_str());
            }
            self.snapshot.refreshed_at = Some(Instant::now());
            self.push_snapshot();
            return;
        }
        self.service = None;
        self.snapshot.busy = false;
        self.snapshot.last_error = None;
        self.record_cache_invalidation(
            LocalRuntimeCacheInvalidationReason::ExplicitReset,
            "operator unloaded the configured GPT-OSS runtime and invalidated warm execution state",
        );
        self.refresh_snapshot(String::from("GPT-OSS 20B unloaded"));
    }

    fn load_service(&self) -> Result<GptOssRuntimeService, String> {
        let mut failures = Vec::new();
        for backend in self.configured_backend.candidate_backends() {
            match GptOssRuntimeService::load(self.model_path.as_path(), backend) {
                Ok(service) => return Ok(service),
                Err(error) => failures.push(format!("{}: {error}", backend.label())),
            }
        }
        Err(format!(
            "failed to load GPT-OSS 20B from {} ({})",
            self.model_path.display(),
            failures.join("; ")
        ))
    }

    fn accepts_requested_model(&self, requested_model: &str) -> bool {
        if requested_model == self.configured_model_label {
            return true;
        }
        self.service
            .as_ref()
            .is_some_and(|service| requested_model == service.model_id())
    }

    fn push_failure(&mut self, request_id: String, error: impl Into<String>) {
        let error = error.into();
        self.snapshot.last_request_id = Some(request_id.clone());
        self.snapshot.last_error = Some(error.clone());
        self.snapshot.last_action = Some(String::from("GPT-OSS generation failed"));
        self.snapshot.busy = false;
        self.maybe_record_compile_failure(error.as_str());
        self.record_cache_invalidation_from_error(error.as_str());
        self.update_service_diagnostics();
        self.snapshot.refreshed_at = Some(Instant::now());
        self.push_snapshot();
        let _ = self.update_tx.send(LocalInferenceRuntimeUpdate::Failed(
            LocalInferenceExecutionFailed { request_id, error },
        ));
    }

    fn handle_generate(&mut self, job: LocalInferenceGenerateJob) {
        let normalized_prompt = normalize_prompt(job.prompt.as_str());
        let options_map = match build_generate_options(job.params.as_slice()) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };

        let Some(service) = self.service.as_ref() else {
            self.push_failure(
                job.request_id,
                String::from("GPT-OSS 20B is not loaded yet"),
            );
            return;
        };
        let loaded_model_id = service.model_id().to_string();
        let request_descriptor = service.model_descriptor().clone();

        if let Some(requested_model) = job
            .requested_model
            .as_deref()
            .and_then(normalize_optional_text)
            && !self.accepts_requested_model(requested_model.as_str())
        {
            self.push_failure(
                job.request_id,
                format!(
                    "GPT-OSS runtime only has '{}' loaded, but '{}' was requested",
                    loaded_model_id, requested_model
                ),
            );
            return;
        }

        let model = loaded_model_id;
        let started_at = Instant::now();
        self.snapshot.last_request_id = Some(job.request_id.clone());
        self.snapshot.last_error = None;
        self.snapshot.last_action =
            Some(format!("GPT-OSS generation queued for {}", job.request_id));
        self.snapshot.busy = true;
        self.snapshot.diagnostics.refresh_posture(true);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.push_snapshot();
        let _ = self.update_tx.send(LocalInferenceRuntimeUpdate::Started(
            LocalInferenceExecutionStarted {
                request_id: job.request_id.clone(),
                model: model.clone(),
            },
        ));

        let options = match psionic_generation_options(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let request = GenerationRequest::new_text(
            job.request_id.as_str(),
            request_descriptor,
            None,
            normalized_prompt.as_str(),
            options,
        );
        let Some(service) = self.service.as_mut() else {
            self.push_failure(
                job.request_id,
                String::from("GPT-OSS 20B is no longer loaded"),
            );
            return;
        };
        let response = match service.generate(&request) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(
                    job.request_id,
                    format!("GPT-OSS generation failed: {error}"),
                );
                return;
            }
        };
        let normalized_options_json = match canonical_options_json(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let normalized_options_digest = sha256_prefixed_text(normalized_options_json.as_str());
        let normalized_prompt_digest = sha256_prefixed_text(normalized_prompt.as_str());
        let provenance = LocalInferenceExecutionProvenance {
            backend: String::from("gpt_oss"),
            requested_model: job.requested_model.clone(),
            served_model: response.model_id.clone(),
            normalized_prompt_digest,
            normalized_options_json,
            normalized_options_digest,
            base_url: self.snapshot.base_url.clone(),
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_token_count: u64::try_from(response.usage.input_tokens).ok(),
            generated_token_count: u64::try_from(response.usage.output_tokens).ok(),
            warm_start: response
                .provenance
                .as_ref()
                .and_then(|value| match value.load_state {
                    GenerationLoadState::Cold => Some(false),
                    GenerationLoadState::Warm => Some(true),
                }),
        };
        let metrics = LocalInferenceExecutionMetrics {
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_eval_count: response
                .metrics
                .prompt_eval_count
                .and_then(|value| u64::try_from(value).ok()),
            prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
            eval_count: response
                .metrics
                .eval_count
                .and_then(|value| u64::try_from(value).ok()),
            eval_duration_ns: response.metrics.eval_duration_ns,
        };
        self.record_cache_invalidation_from_provenance(response.provenance.as_ref());
        if let Some(provenance) = response.provenance.as_ref()
            && let Some(compile_path) = provenance.compile_path.clone()
        {
            self.record_compile_path(
                compile_path,
                response
                    .metrics
                    .total_duration_ns
                    .or_else(|| Some(elapsed_ns(started_at))),
            );
        }
        self.snapshot.last_metrics = Some(metrics.clone());
        self.snapshot.last_action = Some(format!(
            "GPT-OSS generation completed for {}",
            job.request_id
        ));
        self.snapshot.last_error = None;
        self.snapshot.busy = false;
        self.refresh_snapshot(String::from("GPT-OSS runtime refreshed after generation"));
        let _ = self.update_tx.send(LocalInferenceRuntimeUpdate::Completed(
            LocalInferenceExecutionCompleted {
                request_id: job.request_id,
                model,
                output: response.output.text,
                metrics,
                provenance,
            },
        ));
    }

    fn push_snapshot(&self) {
        let _ = self
            .update_tx
            .send(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
    }
}

fn configured_runtime_snapshot(
    model_path: &Path,
    configured_backend: GptOssRuntimeBackend,
) -> LocalInferenceRuntimeSnapshot {
    let configured_model_label = configured_model_label(model_path);
    let artifact_present = model_path.is_file();
    let backend_label = configured_backend.default_runtime_label();
    let mut snapshot = LocalInferenceRuntimeSnapshot {
        base_url: format!("in-process://gpt_oss/{backend_label}"),
        reachable: true,
        busy: false,
        configured_model: Some(configured_model_label.clone()),
        ready_model: None,
        available_models: if artifact_present {
            vec![configured_model_label]
        } else {
            Vec::new()
        },
        loaded_models: Vec::new(),
        configured_model_path: Some(model_path.display().to_string()),
        artifact_present,
        backend_label,
        last_error: None,
        last_action: Some(String::from("Mission Control local model lane ready")),
        last_request_id: None,
        last_metrics: None,
        diagnostics: LocalRuntimeDiagnostics::default(),
        refreshed_at: Some(Instant::now()),
    };
    if !artifact_present {
        snapshot.last_error = Some(missing_gpt_oss_artifact_message(model_path));
        snapshot.last_action = Some(String::from("GPT-OSS 20B artifact missing"));
    }
    snapshot
}

fn configured_gpt_oss_model_path() -> PathBuf {
    std::env::var(ENV_GPT_OSS_MODEL_PATH)
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|home| {
                    home.join("models")
                        .join("gpt-oss")
                        .join(GPT_OSS_20B_FILENAME)
                })
                .unwrap_or_else(|| PathBuf::from("models/gpt-oss").join(GPT_OSS_20B_FILENAME))
        })
}

fn configured_model_label(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| GPT_OSS_20B_FILENAME.to_string())
}

fn missing_gpt_oss_artifact_message(path: &Path) -> String {
    format!(
        "GPT-OSS 20B GGUF missing at {}. Place the file there or set {}.",
        path.display(),
        ENV_GPT_OSS_MODEL_PATH
    )
}

impl LocalInferenceRuntime for PsionicRuntimeAdapter {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String> {
        match command {
            LocalInferenceRuntimeCommand::Refresh => {
                self.refresh_snapshot(String::from("Psionic local runtime refreshed"));
                Ok(())
            }
            LocalInferenceRuntimeCommand::WarmConfiguredModel => {
                let model_id = self.configured_model_id();
                self.service
                    .warm_model(model_id.as_str(), 300_000)
                    .map_err(|error| {
                        format!("failed to warm Psionic model '{model_id}': {error}")
                    })?;
                self.refresh_snapshot(format!("Psionic model '{}' warmed", model_id));
                Ok(())
            }
            LocalInferenceRuntimeCommand::UnloadConfiguredModel => {
                let model_id = self.configured_model_id();
                self.service
                    .unload_model(model_id.as_str())
                    .map_err(|error| {
                        format!("failed to unload Psionic model '{model_id}': {error}")
                    })?;
                self.refresh_snapshot(format!("Psionic model '{}' unloaded", model_id));
                Ok(())
            }
            LocalInferenceRuntimeCommand::Generate(job) => {
                self.handle_generate(job);
                Ok(())
            }
        }
    }

    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate> {
        self.pending_updates.drain(..).collect()
    }
}

fn psionic_generation_options(options: &Map<String, Value>) -> Result<GenerationOptions, String> {
    let max_output_tokens = options
        .get("num_predict")
        .and_then(Value::as_i64)
        .map(usize::try_from)
        .transpose()
        .map_err(|_| "num_predict must be non-negative".to_string())?
        .unwrap_or(64);
    let sampled = options.contains_key("temperature")
        || options.contains_key("top_k")
        || options.contains_key("top_p")
        || options.contains_key("seed");
    let mut generation_options = if sampled {
        GenerationOptions::sample(max_output_tokens)
    } else {
        GenerationOptions::greedy(max_output_tokens)
    };
    generation_options.temperature = options
        .get("temperature")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.top_k = options
        .get("top_k")
        .and_then(Value::as_i64)
        .map(usize::try_from)
        .transpose()
        .map_err(|_| "top_k must be non-negative".to_string())?;
    generation_options.top_p = options
        .get("top_p")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.presence_penalty = options
        .get("presence_penalty")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.frequency_penalty = options
        .get("frequency_penalty")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.seed = options
        .get("seed")
        .and_then(Value::as_i64)
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "seed must be non-negative".to_string())?;
    generation_options.stop_sequences = options
        .get("stop")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(generation_options)
}

pub fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn normalize_prompt(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

pub fn canonical_options_json(options: &Map<String, Value>) -> Result<String, String> {
    let canonical = options
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    serde_json::to_string(&canonical)
        .map_err(|error| format!("failed to encode normalized local inference options: {error}"))
}

pub fn build_generate_options(params: &[JobExecutionParam]) -> Result<Map<String, Value>, String> {
    let mut options = Map::<String, Value>::new();
    for param in params {
        let key = param.key.as_str();
        let value = param.value.as_str();
        match key {
            "max_tokens" => {
                options.insert("num_predict".to_string(), json!(parse_i64(key, value)?));
            }
            "temperature" => {
                options.insert("temperature".to_string(), json!(parse_f64(key, value)?));
            }
            "top_k" => {
                options.insert("top_k".to_string(), json!(parse_i64(key, value)?));
            }
            "top_p" => {
                options.insert("top_p".to_string(), json!(parse_f64(key, value)?));
            }
            "frequency_penalty" => {
                options.insert(
                    "frequency_penalty".to_string(),
                    json!(parse_f64(key, value)?),
                );
            }
            "presence_penalty" => {
                options.insert(
                    "presence_penalty".to_string(),
                    json!(parse_f64(key, value)?),
                );
            }
            "seed" => {
                options.insert("seed".to_string(), json!(parse_i64(key, value)?));
            }
            "stop" => {
                let Some(stop) = normalize_optional_text(value) else {
                    continue;
                };
                options.insert("stop".to_string(), json!([stop]));
            }
            _ => {}
        }
    }
    Ok(options)
}

fn parse_i64(key: &str, raw: &str) -> Result<i64, String> {
    raw.trim()
        .parse::<i64>()
        .map_err(|error| format!("invalid {} value '{}': {}", key, raw.trim(), error))
}

fn parse_f64(key: &str, raw: &str) -> Result<f64, String> {
    raw.trim()
        .parse::<f64>()
        .map_err(|error| format!("invalid {} value '{}': {}", key, raw.trim(), error))
}

#[cfg(test)]
mod tests {
    use super::{
        LocalInferenceGenerateJob, LocalInferenceRuntime, LocalInferenceRuntimeCommand,
        LocalInferenceRuntimeUpdate, PsionicRuntimeAdapter, default_local_inference_runtime,
    };
    use crate::state::job_inbox::JobExecutionParam;
    use psionic_runtime::{
        CacheAction, CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature,
    };
    use psionic_serve::ReferenceWordDecoder;
    use std::time::Duration;

    fn wait_for_snapshot(
        runtime: &mut dyn LocalInferenceRuntime,
        predicate: impl Fn(&super::LocalInferenceRuntimeSnapshot) -> bool,
        attempts: usize,
        sleep: Duration,
    ) -> Option<Box<super::LocalInferenceRuntimeSnapshot>> {
        for _ in 0..attempts {
            if let Some(snapshot) =
                runtime
                    .drain_updates()
                    .into_iter()
                    .find_map(|update| match update {
                        LocalInferenceRuntimeUpdate::Snapshot(value) if predicate(&value) => {
                            Some(value)
                        }
                        _ => None,
                    })
            {
                return Some(snapshot);
            }
            std::thread::sleep(sleep);
        }
        None
    }

    #[test]
    fn psionic_runtime_adapter_refreshes_and_generates() {
        let mut adapter = PsionicRuntimeAdapter::new_reference().expect("psionic adapter");
        adapter
            .enqueue(LocalInferenceRuntimeCommand::Refresh)
            .expect("refresh");
        let snapshot = adapter
            .drain_updates()
            .into_iter()
            .find_map(|update| match update {
                LocalInferenceRuntimeUpdate::Snapshot(value) => Some(value),
                _ => None,
            })
            .expect("snapshot");
        assert_eq!(
            snapshot.ready_model.as_deref(),
            Some(ReferenceWordDecoder::MODEL_ID)
        );

        adapter
            .enqueue(LocalInferenceRuntimeCommand::Generate(
                LocalInferenceGenerateJob {
                    request_id: "req-psionic-1".to_string(),
                    prompt: "hello".to_string(),
                    requested_model: Some(ReferenceWordDecoder::MODEL_ID.to_string()),
                    params: vec![JobExecutionParam {
                        key: "max_tokens".to_string(),
                        value: "2".to_string(),
                    }],
                },
            ))
            .expect("generate");
        let mut completed = None;
        for update in adapter.drain_updates() {
            if let LocalInferenceRuntimeUpdate::Completed(value) = update {
                completed = Some(value);
            }
        }
        let completed = completed.expect("completed update");
        assert_eq!(completed.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(completed.provenance.backend, "psionic");
        assert!(!completed.output.is_empty());
    }

    #[test]
    fn default_local_inference_runtime_tracks_gpt_oss_contract() {
        let mut runtime = default_local_inference_runtime().expect("default local runtime");
        runtime
            .enqueue(LocalInferenceRuntimeCommand::Refresh)
            .expect("refresh");
        let snapshot = wait_for_snapshot(runtime.as_mut(), |_| true, 20, Duration::from_millis(10))
            .expect("snapshot");
        assert!(
            snapshot.base_url.starts_with("in-process://gpt_oss/"),
            "unexpected base_url: {}",
            snapshot.base_url
        );
        assert_eq!(
            snapshot.configured_model.as_deref(),
            Some(super::GPT_OSS_20B_FILENAME)
        );
        assert_eq!(
            snapshot.configured_model_path.as_deref(),
            Some(
                super::configured_gpt_oss_model_path()
                    .display()
                    .to_string()
                    .as_str()
            )
        );
        assert!(!snapshot.busy);
    }

    #[test]
    fn configured_runtime_snapshot_tracks_backend_and_artifact_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_path = temp.path().join("gpt-oss-20b-mxfp4.gguf");
        std::fs::write(&model_path, b"gguf").expect("write gguf");

        let snapshot = super::configured_runtime_snapshot(
            model_path.as_path(),
            super::GptOssRuntimeBackend::Cuda,
        );
        assert_eq!(snapshot.backend_label, "cuda");
        assert!(snapshot.artifact_present);
        assert_eq!(
            snapshot.configured_model_path.as_deref(),
            Some(model_path.display().to_string().as_str())
        );
        assert_eq!(
            snapshot.configured_model.as_deref(),
            Some("gpt-oss-20b-mxfp4.gguf")
        );
        assert!(snapshot.last_error.is_none());
    }

    #[test]
    fn diagnostics_posture_tracks_compile_and_invalidation_state() {
        let mut diagnostics = super::LocalRuntimeDiagnostics::default();
        diagnostics.refresh_posture(false);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::Cold
        );

        diagnostics.last_compile_path = Some(CompilePathEvidence {
            temperature: CompilePathTemperature::WarmReuse,
            execution_plan_cache: CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Reuse,
                "reused cached plan",
            ),
            kernel_cache: CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Reuse,
                "reused cached kernels",
            ),
        });
        diagnostics.last_compile_path_observed_at_epoch_ms = Some(10);
        diagnostics.refresh_posture(false);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::Warm
        );

        diagnostics.last_cache_invalidation = Some(super::LocalRuntimeCacheInvalidation {
            reason: super::LocalRuntimeCacheInvalidationReason::BackendChange,
            summary: "backend changed".to_string(),
            observed_at_epoch_ms: 11,
        });
        diagnostics.refresh_posture(false);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::CacheInvalidated
        );

        diagnostics.last_compile_failure = Some(super::LocalRuntimeCompileFailure {
            summary: "compile failed".to_string(),
            observed_at_epoch_ms: 12,
        });
        diagnostics.refresh_posture(false);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::CompileFailed
        );

        diagnostics.refresh_posture(true);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::Warming
        );

        diagnostics.last_compile_path_observed_at_epoch_ms = Some(13);
        diagnostics.refresh_posture(false);
        assert_eq!(
            diagnostics.posture,
            super::LocalRuntimeExecutionPosture::Warm
        );
    }

    #[test]
    fn default_local_inference_runtime_can_load_real_gpt_oss_when_present() {
        let model_path = super::configured_gpt_oss_model_path();
        if !model_path.is_file() {
            return;
        }

        let mut runtime = default_local_inference_runtime().expect("default local runtime");
        runtime
            .enqueue(LocalInferenceRuntimeCommand::WarmConfiguredModel)
            .expect("queue warm");
        let snapshot = wait_for_snapshot(
            runtime.as_mut(),
            |snapshot| snapshot.is_ready() || snapshot.last_error.is_some(),
            240,
            Duration::from_millis(250),
        )
        .expect("warm snapshot");

        assert!(
            snapshot.is_ready(),
            "expected GPT-OSS runtime to load, got error: {:?}",
            snapshot.last_error
        );
    }
}
