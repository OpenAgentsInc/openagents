use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use openagents_kernel_core::authority::{
    AcceptComputeOutcomeRequest, CreateComputeEvaluationRunRequest,
    CreateComputeTrainingRunRequest, FinalizeComputeEvaluationRunRequest,
    FinalizeComputeTrainingRunRequest, HttpKernelAuthorityClient, KernelAuthority,
    RegisterComputeBenchmarkPackageRequest, RegisterComputeCheckpointFamilyPolicyRequest,
    RegisterComputeEnvironmentPackageRequest, RegisterComputeTrainingPolicyRequest,
    RegisterComputeValidatorPolicyRequest,
};
use openagents_kernel_core::compute::{
    COMPUTE_APPLE_BENCHMARK_PACKAGE_METADATA_ABI_VERSION,
    COMPUTE_APPLE_TRAINING_POLICY_METADATA_ABI_VERSION,
    COMPUTE_APPLE_TRAINING_RUN_METADATA_ABI_VERSION, ComputeAcceptedOutcome,
    ComputeAppleAdapterSampleKind, ComputeAppleBenchmarkPackageMetadata,
    ComputeAppleRuntimeValidationPosture, ComputeAppleTrainingPolicyMetadata,
    ComputeAppleTrainingRunMetadata, ComputeBenchmarkPackage, ComputeCheckpointBinding,
    ComputeCheckpointFamilyPolicy, ComputeEnvironmentArtifactExpectation,
    ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
    ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
    ComputeEvaluationArtifact, ComputeEvaluationMetric, ComputeEvaluationRun,
    ComputeEvaluationRunStatus, ComputeEvaluationSample, ComputeEvaluationSampleStatus,
    ComputeEvaluationSummary, ComputeProofPosture, ComputeRegistryStatus, ComputeTrainingPolicy,
    ComputeTrainingRun, ComputeTrainingRunStatus, ComputeTrainingSummary, ComputeValidatorPolicy,
};
use openagents_kernel_core::compute_benchmarks::ComputeBenchmarkAdapterKind;
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::{PolicyContext, ReceiptHints, TraceContext};
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterLoadRequest, AppleFmBridgeClient,
    AppleFmGenerationOptions, AppleFmGenerationSchema, AppleFmHealthResponse, AppleFmSamplingMode,
    AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
    AppleFmSessionStructuredGenerationRequest, DEFAULT_APPLE_FM_MODEL_ID,
};
use psionic_data::{
    APPLE_ADAPTER_DEFAULT_INSTRUCTION, AppleAdapterDatasetContract, AppleAdapterMessageRole,
    AppleAdapterRuntimeCompatibilityProfile, AppleAdapterSampleKind,
    AppleAdapterSampleTokenCapture, DatasetKey, DatasetPackingMode, DatasetPackingPolicy,
    OverlongSequencePosture,
};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentPackageRefs,
    AppleAdapterEnvironmentRuntimeRequirements, AppleAdapterEnvironmentSpec,
    EnvironmentArtifactExpectation, EnvironmentDatasetBinding, EnvironmentDifficultyMetadata,
    EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
    EnvironmentRubricScoreKind, EnvironmentToolContract, EnvironmentToolInterface,
};
use psionic_eval::{
    AppleAdapterEvalHarness, AppleAdapterObservedSampleOutput, AppleAdapterRuntimeDriftReport,
    AppleAdapterRuntimeSmokeReceipt, AppleAdapterRuntimeSmokeRequest, EvalArtifact, EvalMetric,
    EvalRunState,
};
use psionic_train::{
    APPLE_LIVE_REFERENCE_BASE_MODEL_SIGNATURE, APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
    APPLE_LIVE_REFERENCE_LORA_RANK, AppleAdapterActivationCheckpointPolicy,
    AppleAdapterExecutionConfig, AppleAdapterExperimentManifest, AppleAdapterPrecisionPolicy,
    AppleAdapterReferenceModel, AppleAdapterSftProgressEvent, AppleAdapterSftRunOutcome,
    AppleAdapterSftRunRequest, AppleAdapterTrainingExecutionBackend, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
    apple_live_reference_trainable_targets, run_apple_adapter_sft_export_with_progress,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::apple_repo_lookup_tools::{AppleRepoLookupRecorder, build_repo_lookup_tools};

const APPLE_TRAINING_SCHEMA_VERSION: u16 = 1;
const APPLE_TRAINING_STATE_FILENAME: &str = "apple-adapter-training.json";
const APPLE_TRAINING_ROOT_DIR: &str = "apple-adapter-training";
const APPLE_TRAINING_LOG_LIMIT: usize = 64;
const APPLE_TRAINING_EVENT_LIMIT: usize = 128;
const APPLE_TRAINING_PACKAGE_FORMAT_VERSION: &str = "openagents.apple-fmadapter.v1";
const APPLE_TRAINING_OWNER_ID: &str = "openagents.autopilot_desktop";
const APPLE_TRAINING_CHECKPOINT_FAMILY: &str = "apple_adapter";
const APPLE_TRAINING_CHECKPOINT_POLICY_VERSION: &str = "2026.03.15";
const APPLE_TRAINING_RUNTIME_VALIDATION_POSTURE: ComputeAppleRuntimeValidationPosture =
    ComputeAppleRuntimeValidationPosture::HeldOutAndRuntimeSmoke;
const APPLE_TRAINING_PRODUCT_ID: &str = "psionic.training.apple_adapter.sft";
const APPLE_TRAINING_RUNTIME_SMOKE_PROMPT: &str =
    "Explain what a mutex does in one short sentence.";
const APPLE_PSIONIC_TRAINING_BACKEND_ID: &str = "psionic.apple_adapter.reference_sft.v1";
const APPLE_PSIONIC_EXPORT_BACKEND_ID: &str = "psionic.apple_runtime_asset.native.v1";
const APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER: usize = 30;
const APPLE_LIVE_REFERENCE_SEGMENT0_END_LAYER_EXCLUSIVE: usize = 35;
const APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER: usize = 16;
const APPLE_LIVE_REFERENCE_SEGMENT1_END_LAYER_EXCLUSIVE: usize = 21;

static APPLE_TRAINING_CONTROLLER: OnceLock<Mutex<AppleAdapterTrainingController>> = OnceLock::new();

fn default_apple_fm_base_url() -> String {
    "http://127.0.0.1:11435".to_string()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterOperatorStageState {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl AppleAdapterOperatorStageState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Interrupted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterOperatorProgressPhase {
    Launch,
    Training,
    Evaluation,
    Export,
    RuntimeSmoke,
    Acceptance,
}

impl AppleAdapterOperatorProgressPhase {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::Training => "training",
            Self::Evaluation => "evaluation",
            Self::Export => "export",
            Self::RuntimeSmoke => "runtime_smoke",
            Self::Acceptance => "acceptance",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterOperatorProgressEventKind {
    TrainingStarted,
    EpochStarted,
    StepCompleted,
    LossObserved,
    CheckpointWritten,
    HeldOutEvalStarted,
    HeldOutSampleCompleted,
    ExportStarted,
    ExportCompleted,
    RuntimeSmokeStarted,
    RuntimeSmokeCompleted,
    AcceptanceStarted,
    AcceptanceCompleted,
    Heartbeat,
}

impl AppleAdapterOperatorProgressEventKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TrainingStarted => "training_started",
            Self::EpochStarted => "epoch_started",
            Self::StepCompleted => "step_completed",
            Self::LossObserved => "loss_observed",
            Self::CheckpointWritten => "checkpoint_written",
            Self::HeldOutEvalStarted => "held_out_eval_started",
            Self::HeldOutSampleCompleted => "held_out_sample_completed",
            Self::ExportStarted => "export_started",
            Self::ExportCompleted => "export_completed",
            Self::RuntimeSmokeStarted => "runtime_smoke_started",
            Self::RuntimeSmokeCompleted => "runtime_smoke_completed",
            Self::AcceptanceStarted => "acceptance_started",
            Self::AcceptanceCompleted => "acceptance_completed",
            Self::Heartbeat => "heartbeat",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorProgressSnapshot {
    pub current_phase: Option<AppleAdapterOperatorProgressPhase>,
    pub run_started_at_epoch_ms: Option<u64>,
    pub phase_started_at_epoch_ms: Option<u64>,
    pub last_heartbeat_at_epoch_ms: Option<u64>,
    pub run_elapsed_ms: Option<u64>,
    pub phase_elapsed_ms: Option<u64>,
    pub eta_ms: Option<u64>,
    pub current_epoch: Option<u64>,
    pub expected_epochs: Option<u64>,
    pub completed_steps: Option<u64>,
    pub expected_steps: Option<u64>,
    pub latest_loss: Option<f64>,
    pub completed_eval_samples: Option<u64>,
    pub expected_eval_samples: Option<u64>,
    pub last_checkpoint_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorProgressEvent {
    pub sequence: u64,
    pub event_id: String,
    pub occurred_at_epoch_ms: u64,
    pub phase: AppleAdapterOperatorProgressPhase,
    pub kind: AppleAdapterOperatorProgressEventKind,
    pub detail: String,
    pub epoch_index: Option<u64>,
    pub expected_epochs: Option<u64>,
    pub step_index: Option<u64>,
    pub expected_steps: Option<u64>,
    pub eval_sample_id: Option<String>,
    pub eval_sample_index: Option<u64>,
    pub expected_eval_samples: Option<u64>,
    pub loss: Option<f64>,
    pub eta_ms: Option<u64>,
    pub checkpoint_path: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorLocalSummary {
    pub completed_steps: u64,
    pub expected_steps: u64,
    pub average_loss: Option<f64>,
    pub processed_token_count: Option<u64>,
    pub training_backend: String,
    pub export_backend: String,
    pub training_wall_clock_ms: Option<u64>,
    pub export_wall_clock_ms: Option<u64>,
    pub training_max_resident_set_size_bytes: Option<u64>,
    pub training_peak_memory_footprint_bytes: Option<u64>,
    pub export_max_resident_set_size_bytes: Option<u64>,
    pub export_peak_memory_footprint_bytes: Option<u64>,
    pub checkpoint_size_bytes: Option<u64>,
    pub runtime_asset_size_bytes: Option<u64>,
    pub held_out_pass_rate_bps: Option<u32>,
    pub held_out_average_score_bps: Option<u32>,
    pub runtime_smoke_passed: Option<bool>,
    pub runtime_smoke_digest: Option<String>,
    pub package_digest: Option<String>,
    pub adapter_identifier: Option<String>,
    pub base_model_signature: String,
    pub tokenizer_digest: String,
    pub prompt_shaping_digest: String,
    pub runtime_model_id: String,
    pub runtime_use_case: String,
    pub runtime_guardrails: String,
    pub locale: Option<String>,
    pub default_instruction: Option<String>,
    pub bridge_version: Option<String>,
    pub bridge_platform: Option<String>,
    pub package_format_version: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorAuthorityRefs {
    pub core_environment_ref: Option<String>,
    pub benchmark_environment_ref: Option<String>,
    pub benchmark_package_ref: Option<String>,
    pub validator_policy_ref: Option<String>,
    pub training_policy_ref: Option<String>,
    pub training_run_id: Option<String>,
    pub held_out_eval_run_id: Option<String>,
    pub runtime_validation_eval_run_id: Option<String>,
    pub accepted_outcome_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorRunStatus {
    pub run_id: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub train_dataset_path: String,
    pub held_out_dataset_path: String,
    #[serde(default = "default_apple_fm_base_url")]
    pub apple_fm_base_url: String,
    pub package_name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub launch_state: AppleAdapterOperatorStageState,
    pub export_state: AppleAdapterOperatorStageState,
    pub evaluation_state: AppleAdapterOperatorStageState,
    pub acceptance_state: AppleAdapterOperatorStageState,
    pub run_directory: String,
    pub staged_package_path: Option<String>,
    pub exported_package_path: Option<String>,
    pub training_checkpoint_path: Option<String>,
    pub runtime_asset_package_path: Option<String>,
    pub launched_at_epoch_ms: Option<u64>,
    pub evaluated_at_epoch_ms: Option<u64>,
    pub exported_at_epoch_ms: Option<u64>,
    pub accepted_at_epoch_ms: Option<u64>,
    pub local_summary: Option<AppleAdapterOperatorLocalSummary>,
    pub authority_refs: AppleAdapterOperatorAuthorityRefs,
    pub held_out_eval: Option<EvalRunState>,
    pub runtime_smoke_receipt: Option<AppleAdapterRuntimeSmokeReceipt>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub progress: AppleAdapterOperatorProgressSnapshot,
    #[serde(default)]
    pub recent_events: Vec<AppleAdapterOperatorProgressEvent>,
    #[serde(default)]
    pub next_event_sequence: u64,
    pub log_lines: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorStatus {
    pub schema_version: u16,
    pub storage_path: String,
    pub updated_at_epoch_ms: u64,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub runs: Vec<AppleAdapterOperatorRunStatus>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorLaunchRequest {
    pub train_dataset_path: String,
    pub held_out_dataset_path: String,
    pub package_name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    pub apple_fm_base_url: String,
    #[serde(default)]
    pub expected_base_model_signature: Option<String>,
    #[serde(default)]
    pub experiment_manifest_path: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedAppleAdapterTrainingState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    last_action: Option<String>,
    last_error: Option<String>,
    runs: Vec<AppleAdapterOperatorRunStatus>,
}

struct AppleAdapterTrainingController {
    storage_path: PathBuf,
    state: PersistedAppleAdapterTrainingState,
}

#[derive(Clone, Debug)]
struct AppleAdapterOperatorProgressUpdate {
    phase: AppleAdapterOperatorProgressPhase,
    kind: AppleAdapterOperatorProgressEventKind,
    detail: String,
    epoch_index: Option<u64>,
    expected_epochs: Option<u64>,
    step_index: Option<u64>,
    expected_steps: Option<u64>,
    eval_sample_id: Option<String>,
    eval_sample_index: Option<u64>,
    expected_eval_samples: Option<u64>,
    loss: Option<f64>,
    eta_ms: Option<u64>,
    checkpoint_path: Option<String>,
}

impl AppleAdapterOperatorProgressUpdate {
    fn new(
        phase: AppleAdapterOperatorProgressPhase,
        kind: AppleAdapterOperatorProgressEventKind,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            phase,
            kind,
            detail: detail.into(),
            epoch_index: None,
            expected_epochs: None,
            step_index: None,
            expected_steps: None,
            eval_sample_id: None,
            eval_sample_index: None,
            expected_eval_samples: None,
            loss: None,
            eta_ms: None,
            checkpoint_path: None,
        }
    }

    fn with_epochs(mut self, epoch_index: u64, expected_epochs: u64) -> Self {
        self.epoch_index = Some(epoch_index);
        self.expected_epochs = Some(expected_epochs);
        self
    }

    fn with_steps(mut self, step_index: u64, expected_steps: u64) -> Self {
        self.step_index = Some(step_index);
        self.expected_steps = Some(expected_steps);
        self
    }

    fn with_eval_sample(
        mut self,
        eval_sample_id: impl Into<String>,
        eval_sample_index: u64,
        expected_eval_samples: u64,
    ) -> Self {
        self.eval_sample_id = Some(eval_sample_id.into());
        self.eval_sample_index = Some(eval_sample_index);
        self.expected_eval_samples = Some(expected_eval_samples);
        self
    }

    fn with_loss(mut self, loss: f64) -> Self {
        self.loss = Some(loss);
        self
    }

    fn with_eta_ms(mut self, eta_ms: u64) -> Self {
        self.eta_ms = Some(eta_ms);
        self
    }

    fn with_checkpoint_path(mut self, checkpoint_path: impl Into<String>) -> Self {
        self.checkpoint_path = Some(checkpoint_path.into());
        self
    }
}

impl AppleAdapterTrainingController {
    fn load(storage_path: PathBuf) -> Self {
        let mut state = fs::read(storage_path.as_path())
            .ok()
            .and_then(|raw| serde_json::from_slice::<PersistedAppleAdapterTrainingState>(&raw).ok())
            .unwrap_or_else(|| PersistedAppleAdapterTrainingState {
                schema_version: APPLE_TRAINING_SCHEMA_VERSION,
                updated_at_epoch_ms: current_epoch_ms(),
                last_action: None,
                last_error: None,
                runs: Vec::new(),
            });
        normalize_nonterminal_runs(&mut state.runs);
        Self {
            storage_path,
            state,
        }
    }

    fn persist(&mut self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create Apple adapter training state dir: {error}")
            })?;
        }
        self.state.schema_version = APPLE_TRAINING_SCHEMA_VERSION;
        self.state.updated_at_epoch_ms = current_epoch_ms();
        let raw = serde_json::to_vec_pretty(&self.state)
            .map_err(|error| format!("Failed to encode Apple adapter training state: {error}"))?;
        fs::write(self.storage_path.as_path(), raw)
            .map_err(|error| format!("Failed to write Apple adapter training state: {error}"))
    }

    fn status(&self) -> AppleAdapterOperatorStatus {
        AppleAdapterOperatorStatus {
            schema_version: self.state.schema_version,
            storage_path: self.storage_path.display().to_string(),
            updated_at_epoch_ms: self.state.updated_at_epoch_ms,
            last_action: self.state.last_action.clone(),
            last_error: self.state.last_error.clone(),
            runs: self.state.runs.clone(),
        }
    }

    fn run_mut(&mut self, run_id: &str) -> Result<&mut AppleAdapterOperatorRunStatus, String> {
        self.state
            .runs
            .iter_mut()
            .find(|run| run.run_id == run_id)
            .ok_or_else(|| format!("Unknown Apple adapter operator run `{run_id}`"))
    }

    fn create_run(
        &mut self,
        request: &AppleAdapterOperatorLaunchRequest,
    ) -> Result<AppleAdapterOperatorRunStatus, String> {
        let run_id = build_run_id(request.package_name.as_str());
        if self.state.runs.iter().any(|run| run.run_id == run_id) {
            return Err(format!(
                "Apple adapter operator run `{run_id}` already exists; change the package name or retry later"
            ));
        }
        let run_directory = apple_training_root_dir().join(run_id.as_str());
        let now = current_epoch_ms();
        let run = AppleAdapterOperatorRunStatus {
            run_id: run_id.clone(),
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            train_dataset_path: request.train_dataset_path.clone(),
            held_out_dataset_path: request.held_out_dataset_path.clone(),
            apple_fm_base_url: request.apple_fm_base_url.clone(),
            package_name: request.package_name.clone(),
            author: request.author.clone(),
            description: request.description.clone(),
            license: request.license.clone(),
            launch_state: AppleAdapterOperatorStageState::Running,
            export_state: AppleAdapterOperatorStageState::Pending,
            evaluation_state: AppleAdapterOperatorStageState::Running,
            acceptance_state: AppleAdapterOperatorStageState::Pending,
            run_directory: run_directory.display().to_string(),
            staged_package_path: None,
            exported_package_path: None,
            training_checkpoint_path: None,
            runtime_asset_package_path: None,
            launched_at_epoch_ms: Some(now),
            evaluated_at_epoch_ms: None,
            exported_at_epoch_ms: None,
            accepted_at_epoch_ms: None,
            local_summary: None,
            authority_refs: AppleAdapterOperatorAuthorityRefs::default(),
            held_out_eval: None,
            runtime_smoke_receipt: None,
            last_action: Some("Created Apple adapter operator run".to_string()),
            last_error: None,
            progress: AppleAdapterOperatorProgressSnapshot {
                run_started_at_epoch_ms: Some(now),
                current_phase: Some(AppleAdapterOperatorProgressPhase::Launch),
                phase_started_at_epoch_ms: Some(now),
                ..AppleAdapterOperatorProgressSnapshot::default()
            },
            recent_events: Vec::new(),
            next_event_sequence: 0,
            log_lines: vec![format!("{} launch: created operator run {}", now, run_id)],
        };
        self.state.last_action = Some(format!("Created Apple adapter operator run {}", run_id));
        self.state.last_error = None;
        self.state.runs.insert(0, run.clone());
        self.persist()?;
        Ok(run)
    }

    fn append_log_line(run: &mut AppleAdapterOperatorRunStatus, line: String) {
        run.log_lines.push(line);
        if run.log_lines.len() > APPLE_TRAINING_LOG_LIMIT {
            let trim = run.log_lines.len() - APPLE_TRAINING_LOG_LIMIT;
            run.log_lines.drain(0..trim);
        }
    }

    fn push_log(
        &mut self,
        run_id: &str,
        line: impl Into<String>,
        last_action: impl Into<String>,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        let line = format!("{} {}", now, line.into());
        let action = last_action.into();
        let run = self.run_mut(run_id)?;
        run.updated_at_epoch_ms = now;
        run.last_action = Some(action.clone());
        Self::append_log_line(run, line);
        self.state.last_action = Some(action);
        self.state.last_error = None;
        self.persist()
    }

    fn push_progress_event(
        &mut self,
        run_id: &str,
        update: AppleAdapterOperatorProgressUpdate,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        let run = self.run_mut(run_id)?;
        run.updated_at_epoch_ms = now;
        let phase_changed = run.progress.current_phase != Some(update.phase);
        if phase_changed {
            run.progress.current_phase = Some(update.phase);
            run.progress.phase_started_at_epoch_ms = Some(now);
        }
        let run_started_at = run
            .progress
            .run_started_at_epoch_ms
            .or(run.launched_at_epoch_ms)
            .unwrap_or(now);
        run.progress.run_started_at_epoch_ms = Some(run_started_at);
        run.progress.last_heartbeat_at_epoch_ms = Some(now);
        run.progress.run_elapsed_ms = Some(now.saturating_sub(run_started_at));
        let phase_started_at = run.progress.phase_started_at_epoch_ms.unwrap_or(now);
        run.progress.phase_elapsed_ms = Some(now.saturating_sub(phase_started_at));
        if let Some(epoch_index) = update.epoch_index {
            run.progress.current_epoch = Some(epoch_index);
        }
        if let Some(expected_epochs) = update.expected_epochs {
            run.progress.expected_epochs = Some(expected_epochs);
        }
        if let Some(step_index) = update.step_index {
            run.progress.completed_steps = Some(step_index);
        }
        if let Some(expected_steps) = update.expected_steps {
            run.progress.expected_steps = Some(expected_steps);
        }
        if let Some(loss) = update.loss {
            run.progress.latest_loss = Some(loss);
        }
        if let Some(eval_sample_index) = update.eval_sample_index {
            run.progress.completed_eval_samples = Some(eval_sample_index);
        }
        if let Some(expected_eval_samples) = update.expected_eval_samples {
            run.progress.expected_eval_samples = Some(expected_eval_samples);
        }
        if let Some(eta_ms) = update.eta_ms {
            run.progress.eta_ms = Some(eta_ms);
        }
        if let Some(checkpoint_path) = update.checkpoint_path.clone() {
            run.progress.last_checkpoint_path = Some(checkpoint_path);
        }

        run.next_event_sequence = run.next_event_sequence.saturating_add(1);
        let sequence = run.next_event_sequence;
        let detail = update.detail;
        let event = AppleAdapterOperatorProgressEvent {
            sequence,
            event_id: format!("{run_id}-telemetry-{sequence:06}"),
            occurred_at_epoch_ms: now,
            phase: update.phase,
            kind: update.kind,
            detail: detail.clone(),
            epoch_index: update.epoch_index,
            expected_epochs: update.expected_epochs,
            step_index: update.step_index,
            expected_steps: update.expected_steps,
            eval_sample_id: update.eval_sample_id,
            eval_sample_index: update.eval_sample_index,
            expected_eval_samples: update.expected_eval_samples,
            loss: update.loss,
            eta_ms: update.eta_ms,
            checkpoint_path: update.checkpoint_path,
        };
        run.recent_events.push(event);
        if run.recent_events.len() > APPLE_TRAINING_EVENT_LIMIT {
            let trim = run.recent_events.len() - APPLE_TRAINING_EVENT_LIMIT;
            run.recent_events.drain(0..trim);
        }
        run.last_action = Some(detail.clone());
        Self::append_log_line(
            run,
            format!(
                "{now} telemetry:{}:{} {}",
                update.phase.label(),
                update.kind.label(),
                detail
            ),
        );
        self.state.last_action = Some(detail);
        self.state.last_error = None;
        self.persist()
    }

    fn set_failure(&mut self, run_id: &str, stage: AppleAdapterFailureStage, error: &str) {
        let now = current_epoch_ms();
        if let Ok(run) = self.run_mut(run_id) {
            run.updated_at_epoch_ms = now;
            run.last_error = Some(error.to_string());
            run.last_action = Some(stage.action_label().to_string());
            run.progress.last_heartbeat_at_epoch_ms = Some(now);
            run.progress.run_elapsed_ms = run
                .progress
                .run_started_at_epoch_ms
                .map(|started_at| now.saturating_sub(started_at));
            run.progress.phase_elapsed_ms = run
                .progress
                .phase_started_at_epoch_ms
                .map(|started_at| now.saturating_sub(started_at));
            Self::append_log_line(run, format!("{} error: {}", now, error));
            match stage {
                AppleAdapterFailureStage::Launch => {
                    run.launch_state = AppleAdapterOperatorStageState::Failed;
                    run.evaluation_state = AppleAdapterOperatorStageState::Failed;
                }
                AppleAdapterFailureStage::Export => {
                    run.export_state = AppleAdapterOperatorStageState::Failed;
                }
                AppleAdapterFailureStage::Accept => {
                    run.acceptance_state = AppleAdapterOperatorStageState::Failed;
                }
            }
        }
        self.state.last_error = Some(error.to_string());
        self.state.last_action = Some(stage.action_label().to_string());
        let _ = self.persist();
    }
}

#[derive(Clone, Copy)]
enum AppleAdapterFailureStage {
    Launch,
    Export,
    Accept,
}

impl AppleAdapterFailureStage {
    const fn action_label(self) -> &'static str {
        match self {
            Self::Launch => "Apple adapter training launch failed",
            Self::Export => "Apple adapter training export failed",
            Self::Accept => "Apple adapter training acceptance failed",
        }
    }
}

pub(crate) fn operator_status() -> Result<AppleAdapterOperatorStatus, String> {
    with_controller(|controller| Ok(controller.status()))
}

pub(crate) fn launch_run_async(
    request: AppleAdapterOperatorLaunchRequest,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    validate_launch_request(&request)?;
    let run = with_controller(|controller| controller.create_run(&request))?;
    let run_id = run.run_id.clone();
    let thread_request = request.clone();
    std::thread::Builder::new()
        .name(format!("apple-adapter-launch-{run_id}"))
        .spawn({
            let run_id = run_id.clone();
            move || {
                if let Err(error) = execute_launch_pipeline(run_id.as_str(), &thread_request) {
                    let _ = with_controller(|controller| {
                        controller.set_failure(
                            run_id.as_str(),
                            AppleAdapterFailureStage::Launch,
                            error.as_str(),
                        );
                        Ok(())
                    });
                }
            }
        })
        .map_err(|error| format!("Failed to spawn Apple adapter launch thread: {error}"))?;
    Ok(run)
}

pub(crate) fn launch_run(
    request: AppleAdapterOperatorLaunchRequest,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    validate_launch_request(&request)?;
    let run = with_controller(|controller| controller.create_run(&request))?;
    let run_id = run.run_id.clone();
    if let Err(error) = execute_launch_pipeline(run_id.as_str(), &request) {
        with_controller(|controller| {
            controller.set_failure(
                run_id.as_str(),
                AppleAdapterFailureStage::Launch,
                error.as_str(),
            );
            Ok(())
        })?;
        return Err(error);
    }
    with_controller(|controller| {
        controller
            .state
            .runs
            .iter()
            .find(|existing| existing.run_id == run_id)
            .cloned()
            .ok_or_else(|| format!("Apple adapter operator run `{run_id}` disappeared"))
    })
}

pub(crate) fn export_run(
    run_id: &str,
    export_path: &Path,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    let result = export_run_impl(run_id, export_path);
    if let Err(error) = result.as_ref() {
        with_controller(|controller| {
            controller.set_failure(run_id, AppleAdapterFailureStage::Export, error.as_str());
            Ok(())
        })?;
    }
    result
}

pub(crate) fn accept_run(
    run_id: &str,
    authority_client: &HttpKernelAuthorityClient,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    let result = accept_run_impl(run_id, authority_client);
    if let Err(error) = result.as_ref() {
        with_controller(|controller| {
            controller.set_failure(run_id, AppleAdapterFailureStage::Accept, error.as_str());
            Ok(())
        })?;
    }
    result
}

fn validate_launch_request(request: &AppleAdapterOperatorLaunchRequest) -> Result<(), String> {
    if request.train_dataset_path.trim().is_empty() {
        return Err("Apple adapter training requires `train_dataset_path`".to_string());
    }
    if request.held_out_dataset_path.trim().is_empty() {
        return Err("Apple adapter training requires `held_out_dataset_path`".to_string());
    }
    if request.package_name.trim().is_empty() {
        return Err("Apple adapter training requires `package_name`".to_string());
    }
    if request.apple_fm_base_url.trim().is_empty() {
        return Err("Apple adapter training requires an Apple FM bridge base URL".to_string());
    }
    if let Some(path) = request
        .experiment_manifest_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let manifest_path = Path::new(path);
        if !manifest_path.is_file() {
            return Err(format!(
                "Apple adapter training `experiment_manifest_path` does not exist: {}",
                manifest_path.display()
            ));
        }
    }
    if let Some(signature) = request
        .expected_base_model_signature
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let is_valid_hex = signature.len() == 40
            && signature
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
        if !is_valid_hex {
            return Err(
                "Apple adapter training `expected_base_model_signature` must be a 40-character lowercase hex digest"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn push_progress_event(
    run_id: &str,
    update: AppleAdapterOperatorProgressUpdate,
) -> Result<(), String> {
    with_controller(|controller| controller.push_progress_event(run_id, update))
}

fn training_remaining_eta_ms(
    completed_steps: u64,
    expected_steps: u64,
    step_duration_ms: u64,
) -> u64 {
    expected_steps
        .saturating_sub(completed_steps)
        .saturating_mul(step_duration_ms)
}

fn held_out_eval_eta_ms(
    started_at: Instant,
    completed_samples: usize,
    total_samples: usize,
) -> Option<u64> {
    if completed_samples == 0 || completed_samples >= total_samples {
        return Some(0);
    }
    let elapsed_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let average_sample_ms = elapsed_ms / completed_samples as u64;
    Some(average_sample_ms.saturating_mul(total_samples.saturating_sub(completed_samples) as u64))
}

fn execute_launch_pipeline(
    run_id: &str,
    request: &AppleAdapterOperatorLaunchRequest,
) -> Result<(), String> {
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Launch,
            AppleAdapterOperatorProgressEventKind::Heartbeat,
            "Reading Apple adapter train and held-out datasets",
        ),
    )?;
    with_controller(|controller| {
        controller.push_log(
            run_id,
            "launch: reading train and held-out datasets",
            "Imported Apple adapter datasets",
        )
    })?;
    let run_directory = apple_training_root_dir().join(run_id);
    fs::create_dir_all(run_directory.as_path())
        .map_err(|error| format!("Failed to create run directory: {error}"))?;
    let staging_root = run_directory.join("staged");
    fs::create_dir_all(staging_root.as_path())
        .map_err(|error| format!("Failed to create staging directory: {error}"))?;

    let mut runtime_profile =
        derive_runtime_compatibility_profile(request.apple_fm_base_url.as_str())
            .map_err(|error| error.to_string())?;
    let expected_base_model_signature = request
        .expected_base_model_signature
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(APPLE_LIVE_REFERENCE_BASE_MODEL_SIGNATURE);
    runtime_profile =
        runtime_profile.with_base_model_signature(expected_base_model_signature.to_string());
    with_controller(|controller| {
        controller.push_log(
            run_id,
            format!(
                "launch: derived Apple runtime lineage model={} use_case={} guardrails={} base_signature={}",
                runtime_profile.model_id,
                runtime_profile.use_case,
                runtime_profile.guardrails,
                runtime_profile.base_model_signature(),
            ),
            "Derived Apple runtime compatibility lineage",
        )
    })?;
    if request
        .expected_base_model_signature
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        with_controller(|controller| {
            controller.push_log(
                run_id,
                format!(
                    "launch: pinned Apple compatibility signature {} from operator contract",
                    runtime_profile.base_model_signature()
                ),
                "Pinned Apple runtime compatibility signature",
            )
        })?;
    }

    let train_dataset = load_dataset(
        Path::new(request.train_dataset_path.as_str()),
        &runtime_profile,
    )
    .map_err(|error| error.to_string())?;
    let held_out_dataset = load_dataset(
        Path::new(request.held_out_dataset_path.as_str()),
        &runtime_profile,
    )
    .map_err(|error| error.to_string())?;
    if held_out_dataset.samples.is_empty() {
        return Err("Held-out Apple adapter dataset must contain at least one sample".to_string());
    }
    let runtime_profile = runtime_profile_with_dataset_defaults(&runtime_profile, &train_dataset);
    let experiment_manifest = load_apple_experiment_manifest(request)
        .map_err(|error| format!("Failed to load Apple experiment manifest: {error}"))?;

    with_controller(|controller| {
        controller.push_log(
            run_id,
            format!(
                "launch: imported {} train samples and {} held-out samples",
                train_dataset.samples.len(),
                held_out_dataset.samples.len()
            ),
            "Prepared Apple adapter datasets",
        )
    })?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Launch,
            AppleAdapterOperatorProgressEventKind::Heartbeat,
            format!(
                "Prepared Apple adapter datasets train_samples={} held_out_samples={}",
                train_dataset.samples.len(),
                held_out_dataset.samples.len()
            ),
        ),
    )?;

    let environment = build_environment_bundle(run_id, &train_dataset, &held_out_dataset)
        .map_err(|error| error.to_string())?;
    let captures = train_dataset
        .derive_token_captures()
        .map_err(|error| error.to_string())?;
    let packing_policy = build_execution_packing_policy(captures.as_slice());
    with_controller(|controller| {
        controller.push_log(
            run_id,
            format!(
                "launch: derived packing window={} tokens batch_budget={} tokens from frozen corpus",
                packing_policy.max_row_tokens, packing_policy.max_batch_tokens
            ),
            "Derived Apple adapter packing policy",
        )
    })?;
    if let Some(manifest) = experiment_manifest.as_ref() {
        with_controller(|controller| {
            controller.push_log(
                run_id,
                format!(
                    "launch: loaded experiment manifest {} target_id={} max_steps={} symbolic_targets={}",
                    request
                        .experiment_manifest_path
                        .as_deref()
                        .unwrap_or_default(),
                    manifest.target_id,
                    manifest.max_steps,
                    manifest.lora_targets.join(",")
                ),
                "Loaded Apple experiment manifest",
            )
        })?;
        if manifest.input_width != APPLE_LIVE_REFERENCE_FEATURE_WIDTH
            || manifest.output_width != APPLE_LIVE_REFERENCE_FEATURE_WIDTH
            || manifest.lora_rank != APPLE_LIVE_REFERENCE_LORA_RANK
        {
            with_controller(|controller| {
                controller.push_log(
                    run_id,
                    format!(
                        "launch: manifest requested feature_width={}x{} lora_rank={} but the live Apple runtime-export lane remains pinned to {}x{} rank {} for export compatibility; applying symbolic target families plus max_steps only",
                        manifest.input_width,
                        manifest.output_width,
                        manifest.lora_rank,
                        APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
                        APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
                        APPLE_LIVE_REFERENCE_LORA_RANK,
                    ),
                    "Pinned live Apple export-compatible model geometry",
                )
            })?;
        }
        let skipped_targets = live_backend_skipped_manifest_targets(manifest);
        if !skipped_targets.is_empty() {
            with_controller(|controller| {
                controller.push_log(
                    run_id,
                    format!(
                        "launch: current live Apple exportable backend cannot represent manifest targets [{}] because those runtime tensors need geometry outside the current 2048x2048 reference lane; skipping them for this run",
                        skipped_targets.join(",")
                    ),
                    "Skipped unsupported manifest targets for live Apple export lane",
                )
            })?;
        }
    }
    let execution_config = build_psionic_execution_config(
        run_id,
        &runtime_profile,
        &packing_policy,
        experiment_manifest.as_ref(),
    )?;
    let sft_request = build_psionic_sft_request(run_id, request);
    with_controller(|controller| {
        let learning_rate = execution_config
            .model
            .targets
            .first()
            .map(|target| target.optimizer.learning_rate)
            .unwrap_or(0.0);
        controller.push_log(
            run_id,
            format!(
                "launch: executing Rust-native Psionic Apple training across {} live targets effective_max_steps={} learning_rate={:.4}",
                execution_config.model.targets.len(),
                execution_config.budget.max_steps,
                learning_rate,
            ),
            "Running Rust-native Apple adapter training",
        )
    })?;
    let training_started = Instant::now();
    let backend = AppleAdapterTrainingExecutionBackend::new(
        execution_config,
        &train_dataset,
        captures.as_slice(),
        &environment,
    )
    .map_err(|error| format!("Failed to build Psionic Apple training backend: {error}"))?;
    let mut progress_error = None;
    let step_duration_ms = sft_request.step_duration_ms;
    let sft_outcome = run_apple_adapter_sft_export_with_progress(
        &backend,
        &train_dataset,
        &environment,
        &sft_request,
        |event| {
            if progress_error.is_some() {
                return;
            }
            let result = match event {
                AppleAdapterSftProgressEvent::TrainingStarted {
                    expected_steps,
                    expected_epochs,
                } => push_progress_event(
                    run_id,
                    AppleAdapterOperatorProgressUpdate::new(
                        AppleAdapterOperatorProgressPhase::Training,
                        AppleAdapterOperatorProgressEventKind::TrainingStarted,
                        format!(
                            "Started Rust-native Apple adapter training steps={expected_steps} epochs={expected_epochs}"
                        ),
                    )
                    .with_epochs(1, *expected_epochs)
                    .with_steps(0, *expected_steps)
                    .with_eta_ms(expected_steps.saturating_mul(step_duration_ms)),
                ),
                AppleAdapterSftProgressEvent::EpochStarted {
                    epoch_index,
                    expected_epochs,
                    expected_steps,
                } => push_progress_event(
                    run_id,
                    AppleAdapterOperatorProgressUpdate::new(
                        AppleAdapterOperatorProgressPhase::Training,
                        AppleAdapterOperatorProgressEventKind::EpochStarted,
                        format!(
                            "Started Apple adapter epoch {epoch_index}/{expected_epochs}"
                        ),
                    )
                    .with_epochs(*epoch_index, *expected_epochs)
                    .with_steps(0, *expected_steps)
                    .with_eta_ms(expected_steps.saturating_mul(step_duration_ms)),
                ),
                AppleAdapterSftProgressEvent::StepCompleted {
                    receipt,
                    expected_steps,
                    expected_epochs,
                    epoch_index,
                } => {
                    let eta_ms = training_remaining_eta_ms(
                        receipt.schedule.global_step,
                        *expected_steps,
                        step_duration_ms,
                    );
                    push_progress_event(
                        run_id,
                        AppleAdapterOperatorProgressUpdate::new(
                            AppleAdapterOperatorProgressPhase::Training,
                            AppleAdapterOperatorProgressEventKind::LossObserved,
                            format!(
                                "Observed Apple adapter loss {:.6} at step {}/{}",
                                receipt.loss,
                                receipt.schedule.global_step,
                                expected_steps
                            ),
                        )
                        .with_epochs(*epoch_index, *expected_epochs)
                        .with_steps(receipt.schedule.global_step, *expected_steps)
                        .with_loss(f64::from(receipt.loss))
                        .with_eta_ms(eta_ms),
                    )
                    .and_then(|_| {
                        push_progress_event(
                            run_id,
                            AppleAdapterOperatorProgressUpdate::new(
                                AppleAdapterOperatorProgressPhase::Training,
                                AppleAdapterOperatorProgressEventKind::StepCompleted,
                                format!(
                                    "Completed Apple adapter step {}/{} epoch {}/{}",
                                    receipt.schedule.global_step,
                                    expected_steps,
                                    epoch_index,
                                    expected_epochs
                                ),
                            )
                            .with_epochs(*epoch_index, *expected_epochs)
                            .with_steps(receipt.schedule.global_step, *expected_steps)
                            .with_loss(f64::from(receipt.loss))
                            .with_eta_ms(eta_ms),
                        )
                    })
                    .and_then(|_| {
                        push_progress_event(
                            run_id,
                            AppleAdapterOperatorProgressUpdate::new(
                                AppleAdapterOperatorProgressPhase::Training,
                                AppleAdapterOperatorProgressEventKind::Heartbeat,
                                format!(
                                    "Apple adapter training heartbeat step={}/{} eta_ms={}",
                                    receipt.schedule.global_step, expected_steps, eta_ms
                                ),
                            )
                            .with_epochs(*epoch_index, *expected_epochs)
                            .with_steps(receipt.schedule.global_step, *expected_steps)
                            .with_loss(f64::from(receipt.loss))
                            .with_eta_ms(eta_ms),
                        )
                    })
                }
            };
            if let Err(error) = result {
                progress_error = Some(error);
            }
        },
    )
    .map_err(|error| format!("Psionic Apple training failed: {error}"))?;
    if let Some(error) = progress_error {
        return Err(error);
    }
    let training_wall_clock_ms = elapsed_ms(training_started);
    let training_artifacts =
        write_psionic_training_artifacts(run_directory.as_path(), &sft_outcome)?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Training,
            AppleAdapterOperatorProgressEventKind::CheckpointWritten,
            format!(
                "Wrote Apple adapter checkpoint {}",
                training_artifacts.final_checkpoint_path.display()
            ),
        )
        .with_steps(
            sft_outcome.summary.run_summary.completed_steps,
            sft_outcome.summary.run_summary.budget.max_steps,
        )
        .with_checkpoint_path(
            training_artifacts
                .final_checkpoint_path
                .display()
                .to_string(),
        )
        .with_eta_ms(0),
    )?;
    with_controller(|controller| {
        controller.push_log(
            run_id,
            format!(
                "launch: Psionic checkpoint {} ({} bytes, {} ms)",
                training_artifacts.final_checkpoint_path.display(),
                training_artifacts.checkpoint_size_bytes,
                training_wall_clock_ms
            ),
            "Completed Rust-native Apple training",
        )
    })?;
    let staged_package_path =
        staging_root.join(package_directory_name(request.package_name.as_str()));
    if staged_package_path.exists() {
        fs::remove_dir_all(staged_package_path.as_path()).map_err(|error| {
            format!(
                "Failed to clear stale staged package {}: {error}",
                staged_package_path.display()
            )
        })?;
    }
    let export_started = Instant::now();
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Export,
            AppleAdapterOperatorProgressEventKind::ExportStarted,
            format!(
                "Starting staged Apple adapter export to {}",
                staged_package_path.display()
            ),
        ),
    )?;
    sft_outcome
        .adapter_package
        .write_to_directory(staged_package_path.as_path())
        .map_err(|error| format!("Failed to write staged Apple adapter package: {error}"))?;
    let export_wall_clock_ms = elapsed_ms(export_started);
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Export,
            AppleAdapterOperatorProgressEventKind::ExportCompleted,
            format!(
                "Completed staged Apple adapter export {}",
                staged_package_path.display()
            ),
        )
        .with_eta_ms(0),
    )?;
    with_controller(|controller| {
        controller.push_log(
            run_id,
            format!(
                "launch: Psionic exported {} (package digest={}, {} ms)",
                staged_package_path.display(),
                sft_outcome.adapter_package.package_digest,
                export_wall_clock_ms
            ),
            "Completed Rust-native Apple runtime asset export",
        )
    })?;

    with_controller(|controller| {
        let run = controller.run_mut(run_id)?;
        run.staged_package_path = Some(staged_package_path.display().to_string());
        run.training_checkpoint_path = Some(
            training_artifacts
                .final_checkpoint_path
                .display()
                .to_string(),
        );
        run.runtime_asset_package_path = Some(staged_package_path.display().to_string());
        run.launch_state = AppleAdapterOperatorStageState::Completed;
        run.last_error = None;
        run.updated_at_epoch_ms = current_epoch_ms();
        run.last_action = Some("Exported staged Apple adapter package".to_string());
        controller.state.last_action = Some(format!(
            "Staged Apple adapter package for operator run {}",
            run_id
        ));
        controller.persist()
    })?;

    with_controller(|controller| {
        controller.push_log(
            run_id,
            "evaluation: running held-out eval and runtime smoke",
            "Evaluating staged Apple adapter package",
        )
    })?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Evaluation,
            AppleAdapterOperatorProgressEventKind::HeldOutEvalStarted,
            format!(
                "Started held-out Apple adapter eval samples={}",
                held_out_dataset.samples.len()
            ),
        ),
    )?;
    let held_out_eval = run_local_held_out_eval(
        request.apple_fm_base_url.as_str(),
        staged_package_path.as_path(),
        &environment,
        &held_out_dataset,
        run_id,
    )
    .map_err(|error| format!("Held-out Apple adapter eval failed: {error}"))?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::RuntimeSmoke,
            AppleAdapterOperatorProgressEventKind::RuntimeSmokeStarted,
            "Started Apple adapter runtime smoke",
        ),
    )?;
    let runtime_smoke = run_local_runtime_smoke(
        request.apple_fm_base_url.as_str(),
        staged_package_path.as_path(),
        &environment,
        &runtime_profile,
    )
    .map_err(|error| format!("Apple adapter runtime smoke failed: {error}"))?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::RuntimeSmoke,
            AppleAdapterOperatorProgressEventKind::RuntimeSmokeCompleted,
            format!(
                "Completed Apple adapter runtime smoke passed={} digest={}",
                runtime_smoke.passed, runtime_smoke.smoke_digest
            ),
        )
        .with_eta_ms(0),
    )?;

    let local_summary = build_psionic_local_summary(
        &sft_outcome,
        &training_artifacts,
        captures.as_slice(),
        training_wall_clock_ms,
        export_wall_clock_ms,
        &held_out_eval,
        &runtime_smoke,
        &runtime_profile,
    );
    with_controller(|controller| {
        let run = controller.run_mut(run_id)?;
        run.evaluation_state = AppleAdapterOperatorStageState::Completed;
        run.evaluated_at_epoch_ms = Some(current_epoch_ms());
        run.local_summary = Some(local_summary);
        run.held_out_eval = Some(held_out_eval);
        run.runtime_smoke_receipt = Some(runtime_smoke);
        run.last_error = None;
        run.updated_at_epoch_ms = current_epoch_ms();
        run.last_action = Some("Completed held-out eval and runtime smoke".to_string());
        controller.state.last_action = Some(format!(
            "Completed Apple adapter eval for operator run {}",
            run_id
        ));
        controller.persist()
    })?;
    with_controller(|controller| {
        controller.push_log(
            run_id,
            "launch: completed local train/eval staging",
            "Completed repo-native Apple adapter launch",
        )
    })?;
    Ok(())
}

fn export_run_impl(
    run_id: &str,
    export_path: &Path,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    validate_runtime_compatible_package_path(export_path)?;
    let staged_package_path = with_controller(|controller| {
        let run = controller.run_mut(run_id)?;
        let staged_package_path = run.staged_package_path.clone().ok_or_else(|| {
            format!("Run `{run_id}` does not have a staged Apple adapter package")
        })?;
        run.export_state = AppleAdapterOperatorStageState::Running;
        run.last_error = None;
        run.updated_at_epoch_ms = current_epoch_ms();
        controller.state.last_action = Some(format!(
            "Exporting staged Apple adapter package for {}",
            run_id
        ));
        controller.persist()?;
        Ok(PathBuf::from(staged_package_path))
    })?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Export,
            AppleAdapterOperatorProgressEventKind::ExportStarted,
            format!(
                "Started exported Apple adapter package copy to {}",
                export_path.display()
            ),
        ),
    )?;

    if export_path.exists() {
        let previous_export = with_controller(|controller| {
            Ok(controller
                .run_mut(run_id)?
                .exported_package_path
                .clone()
                .map(PathBuf::from))
        })?;
        if previous_export.as_deref() != Some(export_path) {
            return Err(format!(
                "Export target {} already exists",
                export_path.display()
            ));
        }
    } else if let Some(parent) = export_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create export parent directory {}: {error}",
                parent.display()
            )
        })?;
    }

    copy_directory(staged_package_path.as_path(), export_path)
        .map_err(|error| error.to_string())?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Export,
            AppleAdapterOperatorProgressEventKind::ExportCompleted,
            format!(
                "Completed exported Apple adapter package copy to {}",
                export_path.display()
            ),
        )
        .with_eta_ms(0),
    )?;
    with_controller(|controller| {
        let now = current_epoch_ms();
        {
            let run = controller.run_mut(run_id)?;
            run.export_state = AppleAdapterOperatorStageState::Completed;
            run.exported_at_epoch_ms = Some(now);
            run.exported_package_path = Some(export_path.display().to_string());
            run.updated_at_epoch_ms = now;
            run.last_action = Some("Exported Apple adapter package".to_string());
            run.last_error = None;
            run.log_lines
                .push(format!("{now} export: wrote {}", export_path.display()));
            if run.log_lines.len() > APPLE_TRAINING_LOG_LIMIT {
                let trim = run.log_lines.len() - APPLE_TRAINING_LOG_LIMIT;
                run.log_lines.drain(0..trim);
            }
        }
        controller.state.last_action =
            Some(format!("Exported Apple adapter package for {}", run_id));
        controller.persist()?;
        controller
            .state
            .runs
            .iter()
            .find(|run| run.run_id == run_id)
            .cloned()
            .ok_or_else(|| format!("Apple adapter operator run `{run_id}` disappeared"))
    })
}

fn accept_run_impl(
    run_id: &str,
    authority_client: &HttpKernelAuthorityClient,
) -> Result<AppleAdapterOperatorRunStatus, String> {
    let run = with_controller(|controller| {
        let run = controller.run_mut(run_id)?.clone();
        if run.export_state != AppleAdapterOperatorStageState::Completed {
            return Err(format!(
                "Run `{run_id}` must be exported before it can be accepted into kernel truth"
            ));
        }
        if run.evaluation_state != AppleAdapterOperatorStageState::Completed {
            return Err(format!(
                "Run `{run_id}` must complete local evaluation before acceptance"
            ));
        }
        Ok(run)
    })?;
    let accepted_outcome_id = run
        .authority_refs
        .accepted_outcome_id
        .clone()
        .unwrap_or_else(|| accepted_outcome_id_for_run(run_id));
    let already_accepted = crate::kernel_control::run_kernel_call(async {
        let outcomes = authority_client
            .list_compute_accepted_outcomes(None, None)
            .await?;
        Ok(outcomes
            .into_iter()
            .any(|outcome| outcome.outcome_id == accepted_outcome_id))
    })?;
    if already_accepted {
        return with_controller(|controller| {
            let now = current_epoch_ms();
            {
                let run = controller.run_mut(run_id)?;
                run.acceptance_state = AppleAdapterOperatorStageState::Completed;
                run.accepted_at_epoch_ms.get_or_insert(now);
                if run.authority_refs.accepted_outcome_id.is_none() {
                    run.authority_refs.accepted_outcome_id = Some(accepted_outcome_id.clone());
                }
                run.updated_at_epoch_ms = now;
            }
            controller.state.last_action = Some(format!(
                "Apple adapter operator run {} was already accepted",
                run_id
            ));
            controller.persist()?;
            controller
                .state
                .runs
                .iter()
                .find(|run| run.run_id == run_id)
                .cloned()
                .ok_or_else(|| format!("Apple adapter operator run `{run_id}` disappeared"))
        });
    }

    with_controller(|controller| {
        let run = controller.run_mut(run_id)?;
        run.acceptance_state = AppleAdapterOperatorStageState::Running;
        run.updated_at_epoch_ms = current_epoch_ms();
        run.last_error = None;
        run.last_action = Some("Publishing Apple adapter training to kernel authority".to_string());
        controller.state.last_action = Some(format!(
            "Publishing Apple adapter operator run {} to kernel authority",
            run_id
        ));
        controller.persist()
    })?;
    push_progress_event(
        run_id,
        AppleAdapterOperatorProgressUpdate::new(
            AppleAdapterOperatorProgressPhase::Acceptance,
            AppleAdapterOperatorProgressEventKind::AcceptanceStarted,
            "Started Apple adapter acceptance publication",
        ),
    )?;

    let exported_package_path = run
        .exported_package_path
        .clone()
        .ok_or_else(|| format!("Run `{run_id}` is missing an exported package path"))?;
    let local_summary = run
        .local_summary
        .clone()
        .ok_or_else(|| format!("Run `{run_id}` is missing a local training summary"))?;
    let held_out_eval = run
        .held_out_eval
        .clone()
        .ok_or_else(|| format!("Run `{run_id}` is missing held-out eval results"))?;
    let runtime_smoke = run
        .runtime_smoke_receipt
        .clone()
        .ok_or_else(|| format!("Run `{run_id}` is missing runtime smoke receipt"))?;
    let kernel_refs = kernel_refs_for_run(run_id);
    let runtime_profile = runtime_profile_from_summary(&local_summary);
    let train_dataset = load_dataset(Path::new(run.train_dataset_path.as_str()), &runtime_profile)
        .map_err(|error| format!("Failed to reload train dataset for `{run_id}`: {error}"))?;
    let held_out_dataset = load_dataset(
        Path::new(run.held_out_dataset_path.as_str()),
        &runtime_profile,
    )
    .map_err(|error| format!("Failed to reload held-out dataset for `{run_id}`: {error}"))?;
    let environment = build_environment_bundle(run_id, &train_dataset, &held_out_dataset)
        .map_err(|error| format!("Failed to rebuild Apple environment for `{run_id}`: {error}"))?;
    let runtime_drift = run_local_runtime_drift_check(
        run.apple_fm_base_url.as_str(),
        Path::new(exported_package_path.as_str()),
        &environment,
        &runtime_profile,
        &runtime_smoke,
    )
    .map_err(|error| {
        format!("Failed to rerun Apple runtime drift check for `{run_id}`: {error}")
    })?;
    if runtime_drift.drifted {
        let reason_codes = runtime_drift
            .reason_codes
            .iter()
            .map(|code| format!("{code:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let detail = runtime_drift
            .detail
            .as_deref()
            .unwrap_or("runtime drift detected after the earlier smoke receipt");
        return Err(format!(
            "Run `{run_id}` failed Apple runtime drift revalidation before acceptance: [{reason_codes}] {detail}"
        ));
    }

    ensure_registry(authority_client, &run, &kernel_refs)?;
    let created_at_ms = i64::try_from(run.created_at_epoch_ms).unwrap_or(i64::MAX);
    let launched_at_ms = i64::try_from(run.launched_at_epoch_ms.unwrap_or(run.created_at_epoch_ms))
        .unwrap_or(i64::MAX);
    let evaluated_at_ms =
        i64::try_from(run.evaluated_at_epoch_ms.unwrap_or(run.created_at_epoch_ms))
            .unwrap_or(i64::MAX);
    let accepted_at_ms = i64::try_from(current_epoch_ms()).unwrap_or(i64::MAX);

    ensure_eval_run(
        authority_client,
        build_held_out_eval_requests(
            &run,
            &kernel_refs,
            &held_out_eval,
            created_at_ms,
            evaluated_at_ms,
        )?,
    )?;
    ensure_eval_run(
        authority_client,
        build_runtime_eval_requests(&run, &kernel_refs, &runtime_smoke, evaluated_at_ms)?,
    )?;
    ensure_training_run(
        authority_client,
        build_training_run_requests(
            &run,
            &kernel_refs,
            &local_summary,
            exported_package_path.as_str(),
            created_at_ms,
            launched_at_ms,
            evaluated_at_ms,
        )?,
    )?;
    ensure_training_outcome(
        authority_client,
        build_accept_outcome_request(&run, &kernel_refs, accepted_at_ms)?,
    )?;

    with_controller(|controller| {
        let now = current_epoch_ms();
        {
            let run = controller.run_mut(run_id)?;
            run.acceptance_state = AppleAdapterOperatorStageState::Completed;
            run.accepted_at_epoch_ms = Some(now);
            run.authority_refs = AppleAdapterOperatorAuthorityRefs {
                core_environment_ref: Some(kernel_refs.core_environment_ref.clone()),
                benchmark_environment_ref: Some(kernel_refs.benchmark_environment_ref.clone()),
                benchmark_package_ref: Some(kernel_refs.benchmark_package_ref.clone()),
                validator_policy_ref: Some(kernel_refs.validator_policy_ref.clone()),
                training_policy_ref: Some(kernel_refs.training_policy_ref.clone()),
                training_run_id: Some(kernel_refs.training_run_id.clone()),
                held_out_eval_run_id: Some(kernel_refs.held_out_eval_run_id.clone()),
                runtime_validation_eval_run_id: Some(kernel_refs.runtime_eval_run_id.clone()),
                accepted_outcome_id: Some(kernel_refs.accepted_outcome_id.clone()),
            };
            run.updated_at_epoch_ms = now;
            run.last_error = None;
            run.last_action = Some("Accepted Apple adapter training outcome".to_string());
            run.log_lines.push(format!(
                "{now} accept: published training_run={} outcome={}",
                kernel_refs.training_run_id, kernel_refs.accepted_outcome_id
            ));
            if run.log_lines.len() > APPLE_TRAINING_LOG_LIMIT {
                let trim = run.log_lines.len() - APPLE_TRAINING_LOG_LIMIT;
                run.log_lines.drain(0..trim);
            }
        }
        controller.state.last_action =
            Some(format!("Accepted Apple adapter operator run {}", run_id));
        controller.persist()?;
        controller
            .state
            .runs
            .iter()
            .find(|run| run.run_id == run_id)
            .cloned()
            .ok_or_else(|| format!("Apple adapter operator run `{run_id}` disappeared"))
    })
    .and_then(|run| {
        push_progress_event(
            run_id,
            AppleAdapterOperatorProgressUpdate::new(
                AppleAdapterOperatorProgressPhase::Acceptance,
                AppleAdapterOperatorProgressEventKind::AcceptanceCompleted,
                format!(
                    "Completed Apple adapter acceptance outcome={}",
                    run.authority_refs
                        .accepted_outcome_id
                        .as_deref()
                        .unwrap_or("-")
                ),
            )
            .with_eta_ms(0),
        )?;
        Ok(run)
    })
}

fn ensure_registry(
    authority_client: &HttpKernelAuthorityClient,
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
) -> Result<(), String> {
    let summary = run.local_summary.as_ref().ok_or_else(|| {
        format!(
            "Run `{}` is missing Apple adapter lineage summary",
            run.run_id
        )
    })?;
    let created_at_ms = i64::try_from(run.created_at_epoch_ms).unwrap_or(i64::MAX);
    let core_package = build_compute_environment_package(run, refs, true, created_at_ms)?;
    let benchmark_package_env = build_compute_environment_package(run, refs, false, created_at_ms)?;
    let validator_policy = build_validator_policy(refs, created_at_ms);
    let checkpoint_policy = build_checkpoint_policy(created_at_ms);
    let benchmark_package = build_compute_benchmark_package(run, refs, summary, created_at_ms)?;
    let training_policy = build_compute_training_policy(refs, summary, created_at_ms);

    crate::kernel_control::run_kernel_call(authority_client.register_compute_environment_package(
        RegisterComputeEnvironmentPackageRequest {
            idempotency_key: format!("{}.register_env.core", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            package: core_package,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    ))?;
    crate::kernel_control::run_kernel_call(authority_client.register_compute_environment_package(
        RegisterComputeEnvironmentPackageRequest {
            idempotency_key: format!("{}.register_env.benchmark", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            package: benchmark_package_env,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    ))?;
    crate::kernel_control::run_kernel_call(authority_client.register_compute_validator_policy(
        RegisterComputeValidatorPolicyRequest {
            idempotency_key: format!("{}.register_validator", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            policy_record: validator_policy,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    ))?;
    crate::kernel_control::run_kernel_call(
        authority_client.register_compute_checkpoint_family_policy(
            RegisterComputeCheckpointFamilyPolicyRequest {
                idempotency_key: "apple_adapter.checkpoint_policy".to_string(),
                trace: TraceContext::default(),
                policy: PolicyContext::default(),
                policy_record: checkpoint_policy,
                evidence: Vec::new(),
                hints: ReceiptHints::default(),
            },
        ),
    )?;
    crate::kernel_control::run_kernel_call(authority_client.register_compute_benchmark_package(
        RegisterComputeBenchmarkPackageRequest {
            idempotency_key: format!("{}.register_benchmark", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            benchmark_package,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    ))?;
    crate::kernel_control::run_kernel_call(authority_client.register_compute_training_policy(
        RegisterComputeTrainingPolicyRequest {
            idempotency_key: format!("{}.register_training_policy", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            training_policy,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    ))?;
    Ok(())
}

struct EvalLifecycleRequests {
    create: CreateComputeEvaluationRunRequest,
    finalize: FinalizeComputeEvaluationRunRequest,
    samples: Vec<ComputeEvaluationSample>,
}

struct TrainingLifecycleRequests {
    create: CreateComputeTrainingRunRequest,
    finalize: FinalizeComputeTrainingRunRequest,
}

fn ensure_eval_run(
    authority_client: &HttpKernelAuthorityClient,
    lifecycle: EvalLifecycleRequests,
) -> Result<(), String> {
    let existing = crate::kernel_control::run_kernel_call(async {
        let runs = authority_client
            .list_compute_evaluation_runs(None, None, None)
            .await?;
        Ok(runs
            .into_iter()
            .find(|run| run.eval_run_id == lifecycle.create.eval_run.eval_run_id))
    })?;
    if let Some(existing) = existing {
        if existing.status == ComputeEvaluationRunStatus::Finalized {
            return Ok(());
        }
    } else {
        crate::kernel_control::run_kernel_call(
            authority_client.create_compute_evaluation_run(lifecycle.create),
        )?;
    }
    if !lifecycle.samples.is_empty() {
        crate::kernel_control::run_kernel_call(
            authority_client.append_compute_evaluation_samples(
                openagents_kernel_core::authority::AppendComputeEvaluationSamplesRequest {
                    idempotency_key: lifecycle
                        .finalize
                        .idempotency_key
                        .replace("finalize", "samples"),
                    trace: lifecycle.finalize.trace.clone(),
                    policy: lifecycle.finalize.policy.clone(),
                    eval_run_id: lifecycle.finalize.eval_run_id.clone(),
                    samples: lifecycle.samples,
                    evidence: Vec::new(),
                    hints: lifecycle.finalize.hints.clone(),
                },
            ),
        )?;
    }
    crate::kernel_control::run_kernel_call(
        authority_client.finalize_compute_evaluation_run(lifecycle.finalize),
    )?;
    Ok(())
}

fn ensure_training_run(
    authority_client: &HttpKernelAuthorityClient,
    lifecycle: TrainingLifecycleRequests,
) -> Result<(), String> {
    let existing = crate::kernel_control::run_kernel_call(async {
        let runs = authority_client
            .list_compute_training_runs(None, None, None)
            .await?;
        Ok(runs
            .into_iter()
            .find(|run| run.training_run_id == lifecycle.create.training_run.training_run_id))
    })?;
    if let Some(existing) = existing {
        if existing.status == ComputeTrainingRunStatus::Accepted {
            return Ok(());
        }
    } else {
        crate::kernel_control::run_kernel_call(
            authority_client.create_compute_training_run(lifecycle.create),
        )?;
    }
    crate::kernel_control::run_kernel_call(
        authority_client.finalize_compute_training_run(lifecycle.finalize),
    )?;
    Ok(())
}

fn ensure_training_outcome(
    authority_client: &HttpKernelAuthorityClient,
    request: AcceptComputeOutcomeRequest,
) -> Result<(), String> {
    let existing = crate::kernel_control::run_kernel_call(async {
        let outcomes = authority_client
            .list_compute_accepted_outcomes(None, None)
            .await?;
        Ok(outcomes
            .into_iter()
            .find(|outcome| outcome.outcome_id == request.outcome.outcome_id))
    })?;
    if existing.is_some() {
        return Ok(());
    }
    crate::kernel_control::run_kernel_call(authority_client.accept_compute_outcome(request))?;
    Ok(())
}

fn build_held_out_eval_requests(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    held_out_eval: &EvalRunState,
    created_at_ms: i64,
    finalized_at_ms: i64,
) -> Result<EvalLifecycleRequests, String> {
    let environment_binding = ComputeEnvironmentBinding {
        environment_ref: refs.core_environment_ref.clone(),
        environment_version: Some(refs.version.clone()),
        dataset_ref: Some(dataset_ref_for_path(
            run.held_out_dataset_path.as_str(),
            "held_out",
        )),
        rubric_ref: Some(format!(
            "rubric://apple_adapter/operator/{}/held_out",
            run.run_id
        )),
        evaluator_policy_ref: Some(format!(
            "policy://eval/apple_adapter/operator/{}/held_out",
            run.run_id
        )),
    };
    let samples = held_out_eval
        .samples
        .iter()
        .map(compute_eval_sample_from_eval_sample)
        .collect::<Vec<_>>();
    let summary = compute_eval_summary_from_eval_summary(
        held_out_eval
            .summary
            .as_ref()
            .ok_or_else(|| format!("Held-out eval for `{}` is missing a summary", run.run_id))?,
    );
    Ok(EvalLifecycleRequests {
        create: CreateComputeEvaluationRunRequest {
            idempotency_key: format!("{}.held_out.create", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: refs.held_out_eval_run_id.clone(),
                environment_binding,
                product_id: Some(APPLE_TRAINING_PRODUCT_ID.to_string()),
                capacity_lot_id: None,
                instrument_id: None,
                delivery_proof_id: None,
                model_ref: Some("model://apple-foundation-model".to_string()),
                source_ref: Some(format!(
                    "artifact://apple_adapter/{}/held_out_eval",
                    run.run_id
                )),
                created_at_ms,
                expected_sample_count: Some(samples.len() as u64),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: Some(created_at_ms),
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: json!({
                    "mode": "offline_held_out",
                    "benchmark_package_refs": [refs.benchmark_package_ref.clone()],
                    "operator_run_id": run.run_id,
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
        finalize: FinalizeComputeEvaluationRunRequest {
            idempotency_key: format!("{}.held_out.finalize", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: refs.held_out_eval_run_id.clone(),
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms,
            artifacts: compute_eval_artifacts(held_out_eval.run_artifacts.as_slice()),
            metadata: json!({
                "mode": "offline_held_out",
                "benchmark_adapter_kind": ComputeBenchmarkAdapterKind::AppleAdapterEvalV1.label(),
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
        .with_summary(summary),
        samples,
    })
}

fn build_runtime_eval_requests(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    smoke: &AppleAdapterRuntimeSmokeReceipt,
    finalized_at_ms: i64,
) -> Result<EvalLifecycleRequests, String> {
    let artifact = ComputeEvaluationArtifact {
        artifact_kind: "apple_runtime_smoke_receipt".to_string(),
        artifact_ref: format!("artifact://apple_adapter/{}/runtime_smoke", run.run_id),
        digest: Some(smoke.smoke_digest.clone()),
        metadata: json!({
            "adapter_id": smoke.adapter_id,
            "package_digest": smoke.package_digest,
            "runtime_state": &smoke.runtime_state,
        }),
    };
    let sample = ComputeEvaluationSample {
        eval_run_id: refs.runtime_eval_run_id.clone(),
        sample_id: format!("{}.runtime_smoke", run.run_id),
        ordinal: Some(1),
        status: if smoke.passed {
            ComputeEvaluationSampleStatus::Passed
        } else {
            ComputeEvaluationSampleStatus::Failed
        },
        input_ref: Some(format!(
            "artifact://apple_adapter/{}/runtime_smoke/input",
            run.run_id
        )),
        output_ref: Some(format!(
            "artifact://apple_adapter/{}/runtime_smoke/output",
            run.run_id
        )),
        expected_output_ref: None,
        score_bps: Some(if smoke.passed { 10_000 } else { 0 }),
        metrics: smoke
            .metrics
            .iter()
            .map(compute_eval_metric_from_eval_metric)
            .collect(),
        artifacts: vec![artifact.clone()],
        error_reason: None,
        recorded_at_ms: finalized_at_ms,
        metadata: json!({ "bridge_base": "desktop_control" }),
    };
    let summary = ComputeEvaluationSummary {
        total_samples: 1,
        scored_samples: 1,
        passed_samples: if smoke.passed { 1 } else { 0 },
        failed_samples: if smoke.passed { 0 } else { 1 },
        errored_samples: 0,
        average_score_bps: Some(if smoke.passed { 10_000 } else { 0 }),
        pass_rate_bps: Some(if smoke.passed { 10_000 } else { 0 }),
        aggregate_metrics: smoke
            .metrics
            .iter()
            .map(compute_eval_metric_from_eval_metric)
            .collect(),
        artifacts: vec![artifact],
    };
    Ok(EvalLifecycleRequests {
        create: CreateComputeEvaluationRunRequest {
            idempotency_key: format!("{}.runtime.create", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: refs.runtime_eval_run_id.clone(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: refs.core_environment_ref.clone(),
                    environment_version: Some(refs.version.clone()),
                    dataset_ref: None,
                    rubric_ref: Some(format!(
                        "rubric://apple_adapter/operator/{}/runtime_smoke",
                        run.run_id
                    )),
                    evaluator_policy_ref: Some(format!(
                        "policy://eval/apple_adapter/operator/{}/runtime_smoke",
                        run.run_id
                    )),
                },
                product_id: Some(APPLE_TRAINING_PRODUCT_ID.to_string()),
                capacity_lot_id: None,
                instrument_id: None,
                delivery_proof_id: None,
                model_ref: Some("model://apple-foundation-model".to_string()),
                source_ref: Some(format!(
                    "artifact://apple_adapter/{}/runtime_smoke",
                    run.run_id
                )),
                created_at_ms: finalized_at_ms,
                expected_sample_count: Some(1),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: Some(finalized_at_ms),
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: json!({
                    "mode": "runtime_smoke",
                    "benchmark_package_refs": [refs.benchmark_package_ref.clone()],
                    "operator_run_id": run.run_id,
                    "runtime_state": &smoke.runtime_state,
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
        finalize: FinalizeComputeEvaluationRunRequest {
            idempotency_key: format!("{}.runtime.finalize", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: refs.runtime_eval_run_id.clone(),
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms,
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "runtime_smoke_receipt".to_string(),
                artifact_ref: format!("artifact://apple_adapter/{}/runtime_smoke", run.run_id),
                digest: Some(smoke.smoke_digest.clone()),
                metadata: json!({
                    "package_digest": smoke.package_digest,
                    "adapter_id": smoke.adapter_id,
                    "runtime_state": &smoke.runtime_state,
                }),
            }],
            metadata: json!({ "mode": "runtime_smoke" }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
        .with_summary(summary),
        samples: vec![sample],
    })
}

fn build_training_run_requests(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    summary: &AppleAdapterOperatorLocalSummary,
    exported_package_path: &str,
    created_at_ms: i64,
    launched_at_ms: i64,
    finalized_at_ms: i64,
) -> Result<TrainingLifecycleRequests, String> {
    let checkpoint_ref_root = format!("checkpoint://apple_adapter/operator/{}", run.run_id);
    let final_checkpoint_ref = format!("{checkpoint_ref_root}/final");
    let promotion_checkpoint_ref = format!("{checkpoint_ref_root}/promotion");
    let training_summary = ComputeTrainingSummary {
        completed_step_count: Some(summary.completed_steps),
        processed_token_count: summary.processed_token_count,
        average_loss: summary.average_loss,
        best_eval_score_bps: summary
            .held_out_average_score_bps
            .or(summary.held_out_pass_rate_bps),
        accepted_checkpoint_ref: Some(promotion_checkpoint_ref.clone()),
        aggregate_metrics: summary
            .held_out_average_score_bps
            .map(|score| {
                vec![ComputeEvaluationMetric {
                    metric_id: "apple_adapter.text_match".to_string(),
                    metric_value: f64::from(score) / 10_000.0,
                    unit: Some("fraction".to_string()),
                    metadata: json!({ "benchmark_package_ref": refs.benchmark_package_ref }),
                }]
            })
            .unwrap_or_default(),
        artifacts: vec![ComputeEvaluationArtifact {
            artifact_kind: "training_manifest".to_string(),
            artifact_ref: format!("artifact://apple_adapter/{}/training_manifest", run.run_id),
            digest: summary.package_digest.clone(),
            metadata: json!({
                "exported_package_path": exported_package_path,
            }),
        }],
    };
    let create_training_run = ComputeTrainingRun {
        training_run_id: refs.training_run_id.clone(),
        training_policy_ref: refs.training_policy_ref.clone(),
        environment_binding: ComputeEnvironmentBinding {
            environment_ref: refs.core_environment_ref.clone(),
            environment_version: Some(refs.version.clone()),
            dataset_ref: Some(dataset_ref_for_path(
                run.train_dataset_path.as_str(),
                "train",
            )),
            rubric_ref: Some(format!(
                "rubric://apple_adapter/operator/{}/held_out",
                run.run_id
            )),
            evaluator_policy_ref: Some(format!(
                "policy://eval/apple_adapter/operator/{}/held_out",
                run.run_id
            )),
        },
        checkpoint_binding: ComputeCheckpointBinding {
            checkpoint_family: APPLE_TRAINING_CHECKPOINT_FAMILY.to_string(),
            latest_checkpoint_ref: Some(format!("{checkpoint_ref_root}/base")),
            recovery_posture: Some("warm-resume".to_string()),
        },
        validator_policy_ref: refs.validator_policy_ref.clone(),
        benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
        product_id: Some(APPLE_TRAINING_PRODUCT_ID.to_string()),
        capacity_lot_id: None,
        instrument_id: None,
        delivery_proof_id: None,
        model_ref: Some("model://apple-foundation-model".to_string()),
        source_ref: Some(format!(
            "artifact://apple_adapter/{}/training_input",
            run.run_id
        )),
        rollout_verification_eval_run_ids: vec![
            refs.held_out_eval_run_id.clone(),
            refs.runtime_eval_run_id.clone(),
        ],
        created_at_ms,
        started_at_ms: Some(launched_at_ms),
        finalized_at_ms: None,
        expected_step_count: Some(summary.expected_steps),
        completed_step_count: Some(summary.completed_steps),
        status: ComputeTrainingRunStatus::Running,
        final_checkpoint_ref: None,
        promotion_checkpoint_ref: None,
        summary: None,
        metadata: json!({
            "apple_adapter": apple_training_run_metadata_value(
                summary,
                refs,
                None,
                None,
                None,
            ),
            "operator_run_id": run.run_id,
            "exported_package_path": exported_package_path,
        }),
    };
    Ok(TrainingLifecycleRequests {
        create: CreateComputeTrainingRunRequest {
            idempotency_key: format!("{}.training.create", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            training_run: create_training_run,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
        finalize: FinalizeComputeTrainingRunRequest {
            idempotency_key: format!("{}.training.finalize", run.run_id),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            training_run_id: refs.training_run_id.clone(),
            status: ComputeTrainingRunStatus::Accepted,
            finalized_at_ms,
            final_checkpoint_ref: Some(final_checkpoint_ref),
            promotion_checkpoint_ref: Some(promotion_checkpoint_ref),
            summary: Some(training_summary),
            metadata: json!({
                "apple_adapter": apple_training_run_metadata_value(
                    summary,
                    refs,
                    summary.package_digest.clone(),
                    Some(refs.held_out_eval_run_id.clone()),
                    Some(refs.runtime_eval_run_id.clone()),
                ),
                "operator_run_id": run.run_id,
                "exported_package_path": exported_package_path,
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        },
    })
}

fn build_accept_outcome_request(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    accepted_at_ms: i64,
) -> Result<AcceptComputeOutcomeRequest, String> {
    let summary = run
        .local_summary
        .as_ref()
        .ok_or_else(|| format!("Run `{}` is missing a local summary", run.run_id))?;
    Ok(AcceptComputeOutcomeRequest {
        idempotency_key: format!("{}.training.accept", run.run_id),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        outcome: ComputeAcceptedOutcome::from_training_run(
            refs.accepted_outcome_id.clone(),
            accepted_at_ms,
            &ComputeTrainingRun {
                training_run_id: refs.training_run_id.clone(),
                training_policy_ref: refs.training_policy_ref.clone(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: refs.core_environment_ref.clone(),
                    environment_version: Some(refs.version.clone()),
                    dataset_ref: Some(dataset_ref_for_path(
                        run.train_dataset_path.as_str(),
                        "train",
                    )),
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                checkpoint_binding: ComputeCheckpointBinding {
                    checkpoint_family: APPLE_TRAINING_CHECKPOINT_FAMILY.to_string(),
                    latest_checkpoint_ref: Some(format!(
                        "checkpoint://apple_adapter/operator/{}/promotion",
                        run.run_id
                    )),
                    recovery_posture: Some("warm-resume".to_string()),
                },
                validator_policy_ref: refs.validator_policy_ref.clone(),
                benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
                product_id: Some(APPLE_TRAINING_PRODUCT_ID.to_string()),
                capacity_lot_id: None,
                instrument_id: None,
                delivery_proof_id: None,
                model_ref: Some("model://apple-foundation-model".to_string()),
                source_ref: Some(format!(
                    "artifact://apple_adapter/{}/training_input",
                    run.run_id
                )),
                rollout_verification_eval_run_ids: vec![
                    refs.held_out_eval_run_id.clone(),
                    refs.runtime_eval_run_id.clone(),
                ],
                created_at_ms: accepted_at_ms,
                started_at_ms: Some(accepted_at_ms),
                finalized_at_ms: Some(accepted_at_ms),
                expected_step_count: run
                    .local_summary
                    .as_ref()
                    .map(|summary| summary.expected_steps),
                completed_step_count: run
                    .local_summary
                    .as_ref()
                    .map(|summary| summary.completed_steps),
                status: ComputeTrainingRunStatus::Accepted,
                final_checkpoint_ref: Some(format!(
                    "checkpoint://apple_adapter/operator/{}/final",
                    run.run_id
                )),
                promotion_checkpoint_ref: Some(format!(
                    "checkpoint://apple_adapter/operator/{}/promotion",
                    run.run_id
                )),
                summary: Some(compute_training_summary_from_local(run)?),
                metadata: json!({
                    "apple_adapter": apple_training_run_metadata_value(
                        summary,
                        refs,
                        summary.package_digest.clone(),
                        Some(refs.held_out_eval_run_id.clone()),
                        Some(refs.runtime_eval_run_id.clone()),
                    ),
                }),
            },
            json!({ "operator_run_id": run.run_id }),
        ),
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    })
}

fn compute_training_summary_from_local(
    run: &AppleAdapterOperatorRunStatus,
) -> Result<ComputeTrainingSummary, String> {
    let summary = run
        .local_summary
        .as_ref()
        .ok_or_else(|| format!("Run `{}` is missing a local summary", run.run_id))?;
    Ok(ComputeTrainingSummary {
        completed_step_count: Some(summary.completed_steps),
        processed_token_count: summary.processed_token_count,
        average_loss: summary.average_loss,
        best_eval_score_bps: summary
            .held_out_average_score_bps
            .or(summary.held_out_pass_rate_bps),
        accepted_checkpoint_ref: Some(format!(
            "checkpoint://apple_adapter/operator/{}/promotion",
            run.run_id
        )),
        aggregate_metrics: summary
            .held_out_average_score_bps
            .map(|score| {
                vec![ComputeEvaluationMetric {
                    metric_id: "apple_adapter.text_match".to_string(),
                    metric_value: f64::from(score) / 10_000.0,
                    unit: Some("fraction".to_string()),
                    metadata: json!({}),
                }]
            })
            .unwrap_or_default(),
        artifacts: vec![ComputeEvaluationArtifact {
            artifact_kind: "training_manifest".to_string(),
            artifact_ref: format!("artifact://apple_adapter/{}/training_manifest", run.run_id),
            digest: summary.package_digest.clone(),
            metadata: json!({}),
        }],
    })
}

fn build_compute_environment_package(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    core: bool,
    created_at_ms: i64,
) -> Result<ComputeEnvironmentPackage, String> {
    let dataset_ref = if core {
        dataset_ref_for_path(run.train_dataset_path.as_str(), "train")
    } else {
        dataset_ref_for_path(run.held_out_dataset_path.as_str(), "held_out")
    };
    Ok(ComputeEnvironmentPackage {
        environment_ref: if core {
            refs.core_environment_ref.clone()
        } else {
            refs.benchmark_environment_ref.clone()
        },
        version: refs.version.clone(),
        family: "apple_adapter".to_string(),
        display_name: if core {
            format!("Apple Adapter Operator Core {}", run.package_name)
        } else {
            format!("Apple Adapter Operator Benchmark {}", run.package_name)
        },
        owner_id: APPLE_TRAINING_OWNER_ID.to_string(),
        created_at_ms,
        updated_at_ms: created_at_ms,
        status: ComputeEnvironmentPackageStatus::Active,
        description: Some(if core {
            "Repo-native Apple adapter operator core environment".to_string()
        } else {
            "Repo-native Apple adapter operator benchmark environment".to_string()
        }),
        package_digest: Some(sha256_prefixed_text(
            format!(
                "{}|{}|{}",
                run.run_id,
                if core { "core" } else { "benchmark" },
                dataset_ref
            )
            .as_str(),
        )),
        dataset_bindings: vec![ComputeEnvironmentDatasetBinding {
            dataset_ref,
            split_ref: Some(if core {
                "train".to_string()
            } else {
                "held_out".to_string()
            }),
            mount_path: Some(if core {
                "/datasets/apple/train".to_string()
            } else {
                "/datasets/apple/held_out".to_string()
            }),
            integrity_ref: None,
            access_policy_ref: None,
            required: true,
            metadata: Value::Null,
        }],
        harness: Some(ComputeEnvironmentHarness {
            harness_ref: if core {
                format!("harness://apple_adapter/operator/{}/session", run.run_id)
            } else {
                format!("harness://apple_adapter/operator/{}/benchmark", run.run_id)
            },
            runtime_family: "multi_turn_dialog".to_string(),
            entrypoint: Some(if core {
                "apple_adapter::session".to_string()
            } else {
                "apple_adapter::benchmark".to_string()
            }),
            args: Vec::new(),
            sandbox_profile_ref: Some(if core {
                "sandbox.profile.apple_adapter.core".to_string()
            } else {
                "sandbox.profile.apple_adapter.benchmark".to_string()
            }),
            evaluator_policy_ref: Some(if core {
                format!(
                    "policy://eval/apple_adapter/operator/{}/held_out",
                    run.run_id
                )
            } else {
                format!(
                    "policy://eval/apple_adapter/operator/{}/benchmark",
                    run.run_id
                )
            }),
            time_budget_ms: Some(30_000),
            metadata: Value::Null,
        }),
        rubric_bindings: vec![ComputeEnvironmentRubricBinding {
            rubric_ref: if core {
                format!("rubric://apple_adapter/operator/{}/held_out", run.run_id)
            } else {
                format!("rubric://apple_adapter/operator/{}/benchmark", run.run_id)
            },
            score_type: Some("scalar".to_string()),
            pass_threshold_bps: Some(0),
            metadata: Value::Null,
        }],
        expected_artifacts: vec![ComputeEnvironmentArtifactExpectation {
            artifact_kind: "training_manifest".to_string(),
            artifact_ref: Some(format!(
                "artifact://apple_adapter/{}/training_manifest",
                run.run_id
            )),
            required: true,
            verification_policy_ref: Some(format!(
                "verify://apple_adapter/operator/{}/training_manifest",
                run.run_id
            )),
            metadata: Value::Null,
        }],
        policy_refs: vec![
            refs.training_policy_ref.clone(),
            refs.validator_policy_ref.clone(),
        ],
        metadata: json!({
            "operator_run_id": run.run_id,
            "surface": if core { "core" } else { "benchmark" },
        }),
    })
}

fn build_validator_policy(
    refs: &AppleAdapterKernelRefs,
    created_at_ms: i64,
) -> ComputeValidatorPolicy {
    ComputeValidatorPolicy {
        policy_ref: refs.validator_policy_ref.clone(),
        version: refs.version.clone(),
        owner_id: APPLE_TRAINING_OWNER_ID.to_string(),
        created_at_ms,
        updated_at_ms: created_at_ms,
        status: ComputeRegistryStatus::Active,
        validator_pool_ref: format!("validator_pool://apple_adapter/operator/{}", refs.run_slug),
        minimum_validator_count: Some(1),
        challenge_window_ms: Some(60_000),
        required_proof_posture: Some(ComputeProofPosture::ChallengeEligible),
        benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
        metadata: json!({ "operator_run_slug": refs.run_slug }),
    }
}

fn build_checkpoint_policy(created_at_ms: i64) -> ComputeCheckpointFamilyPolicy {
    ComputeCheckpointFamilyPolicy {
        checkpoint_family: APPLE_TRAINING_CHECKPOINT_FAMILY.to_string(),
        version: APPLE_TRAINING_CHECKPOINT_POLICY_VERSION.to_string(),
        owner_id: APPLE_TRAINING_OWNER_ID.to_string(),
        created_at_ms,
        updated_at_ms: created_at_ms,
        status: ComputeRegistryStatus::Active,
        description: Some("Repo-native Apple adapter operator checkpoint family".to_string()),
        source_family: Some("apple_foundation_models".to_string()),
        default_recovery_posture: Some("warm-resume".to_string()),
        allowed_environment_refs: Vec::new(),
        validator_policy_ref: None,
        retention_policy_ref: None,
        metadata: json!({ "surface": "operator" }),
    }
}

fn apple_benchmark_metadata_value(
    summary: &AppleAdapterOperatorLocalSummary,
    refs: &AppleAdapterKernelRefs,
    sample_kinds: Vec<ComputeAppleAdapterSampleKind>,
) -> Value {
    let mut value = serde_json::to_value(ComputeAppleBenchmarkPackageMetadata {
        abi_version: COMPUTE_APPLE_BENCHMARK_PACKAGE_METADATA_ABI_VERSION.to_string(),
        base_model_signature: summary.base_model_signature.clone(),
        tokenizer_digest: summary.tokenizer_digest.clone(),
        package_format_version: summary.package_format_version.clone(),
        environment_ref: refs.benchmark_environment_ref.clone(),
        core_environment_ref: refs.core_environment_ref.clone(),
        environment_group_ref: format!("group.apple_adapter.operator.{}", refs.run_slug),
        validator_policy_ref: refs.validator_policy_ref.clone(),
        runtime_validation_posture: APPLE_TRAINING_RUNTIME_VALIDATION_POSTURE,
        sample_kinds,
    })
    .unwrap_or(Value::Null);
    extend_apple_lineage_metadata(&mut value, summary);
    value
}

fn apple_training_policy_metadata_value(
    summary: &AppleAdapterOperatorLocalSummary,
    refs: &AppleAdapterKernelRefs,
) -> Value {
    let mut value = serde_json::to_value(ComputeAppleTrainingPolicyMetadata {
        abi_version: COMPUTE_APPLE_TRAINING_POLICY_METADATA_ABI_VERSION.to_string(),
        base_model_signature: summary.base_model_signature.clone(),
        tokenizer_digest: summary.tokenizer_digest.clone(),
        package_format_version: summary.package_format_version.clone(),
        environment_ref: refs.core_environment_ref.clone(),
        benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
        validator_policy_ref: refs.validator_policy_ref.clone(),
        draft_model_present: false,
        runtime_validation_posture: APPLE_TRAINING_RUNTIME_VALIDATION_POSTURE,
    })
    .unwrap_or(Value::Null);
    extend_apple_lineage_metadata(&mut value, summary);
    value
}

fn apple_training_run_metadata_value(
    summary: &AppleAdapterOperatorLocalSummary,
    refs: &AppleAdapterKernelRefs,
    package_digest: Option<String>,
    held_out_eval_run_id: Option<String>,
    runtime_validation_eval_run_id: Option<String>,
) -> Value {
    let mut value = serde_json::to_value(ComputeAppleTrainingRunMetadata {
        abi_version: COMPUTE_APPLE_TRAINING_RUN_METADATA_ABI_VERSION.to_string(),
        base_model_signature: summary.base_model_signature.clone(),
        tokenizer_digest: summary.tokenizer_digest.clone(),
        package_format_version: summary.package_format_version.clone(),
        environment_ref: refs.core_environment_ref.clone(),
        benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
        validator_policy_ref: refs.validator_policy_ref.clone(),
        draft_model_present: false,
        runtime_validation_posture: APPLE_TRAINING_RUNTIME_VALIDATION_POSTURE,
        package_digest,
        held_out_eval_run_id,
        runtime_validation_eval_run_id,
    })
    .unwrap_or(Value::Null);
    extend_apple_lineage_metadata(&mut value, summary);
    value
}

fn extend_apple_lineage_metadata(value: &mut Value, summary: &AppleAdapterOperatorLocalSummary) {
    let Value::Object(object) = value else {
        return;
    };
    object.insert(
        "prompt_shaping_digest".to_string(),
        Value::String(summary.prompt_shaping_digest.clone()),
    );
    object.insert(
        "runtime_model_id".to_string(),
        Value::String(summary.runtime_model_id.clone()),
    );
    object.insert(
        "runtime_use_case".to_string(),
        Value::String(summary.runtime_use_case.clone()),
    );
    object.insert(
        "runtime_guardrails".to_string(),
        Value::String(summary.runtime_guardrails.clone()),
    );
    if let Some(locale) = &summary.locale {
        object.insert("locale".to_string(), Value::String(locale.clone()));
    }
    if let Some(default_instruction) = &summary.default_instruction {
        object.insert(
            "default_instruction".to_string(),
            Value::String(default_instruction.clone()),
        );
    }
    if let Some(bridge_version) = &summary.bridge_version {
        object.insert(
            "bridge_version".to_string(),
            Value::String(bridge_version.clone()),
        );
    }
    if let Some(bridge_platform) = &summary.bridge_platform {
        object.insert(
            "bridge_platform".to_string(),
            Value::String(bridge_platform.clone()),
        );
    }
}

fn build_compute_benchmark_package(
    run: &AppleAdapterOperatorRunStatus,
    refs: &AppleAdapterKernelRefs,
    summary: &AppleAdapterOperatorLocalSummary,
    created_at_ms: i64,
) -> Result<ComputeBenchmarkPackage, String> {
    let sample_kinds = benchmark_sample_kinds(run)?;
    let mut required_metric_ids = vec!["apple_adapter.text_match".to_string()];
    if sample_kinds
        .iter()
        .any(|kind| *kind == ComputeAppleAdapterSampleKind::GuidedGenerationWithSchema)
    {
        required_metric_ids.push("apple_adapter.structured_output_match".to_string());
    }
    if sample_kinds
        .iter()
        .any(|kind| *kind == ComputeAppleAdapterSampleKind::ToolCalling)
    {
        required_metric_ids.push("apple_adapter.tool_call_coverage".to_string());
    }
    Ok(ComputeBenchmarkPackage {
        benchmark_package_ref: refs.benchmark_package_ref.clone(),
        version: refs.version.clone(),
        family: format!("apple_adapter.operator.{}", refs.run_slug),
        display_name: format!("Apple Adapter Operator Benchmark {}", refs.run_slug),
        owner_id: APPLE_TRAINING_OWNER_ID.to_string(),
        created_at_ms,
        updated_at_ms: created_at_ms,
        status: ComputeRegistryStatus::Active,
        environment_ref: refs.benchmark_environment_ref.clone(),
        environment_version: Some(refs.version.clone()),
        benchmark_suite_ref: Some(format!(
            "benchmark://apple_adapter/operator/{}/suite",
            refs.run_slug
        )),
        adapter_kind: Some(
            ComputeBenchmarkAdapterKind::AppleAdapterEvalV1
                .label()
                .to_string(),
        ),
        evaluator_policy_ref: Some(format!(
            "policy://eval/apple_adapter/operator/{}/benchmark",
            refs.run_slug
        )),
        pass_threshold_bps: Some(0),
        required_metric_ids,
        artifact_refs: vec![format!(
            "artifact://apple_adapter/{}/benchmark_manifest",
            refs.run_slug
        )],
        metadata: json!({
            "apple_adapter": apple_benchmark_metadata_value(summary, refs, sample_kinds)
        }),
    })
}

fn build_compute_training_policy(
    refs: &AppleAdapterKernelRefs,
    summary: &AppleAdapterOperatorLocalSummary,
    created_at_ms: i64,
) -> ComputeTrainingPolicy {
    ComputeTrainingPolicy {
        training_policy_ref: refs.training_policy_ref.clone(),
        version: refs.version.clone(),
        owner_id: APPLE_TRAINING_OWNER_ID.to_string(),
        created_at_ms,
        updated_at_ms: created_at_ms,
        status: ComputeRegistryStatus::Active,
        environment_refs: vec![refs.core_environment_ref.clone()],
        checkpoint_family: APPLE_TRAINING_CHECKPOINT_FAMILY.to_string(),
        validator_policy_ref: refs.validator_policy_ref.clone(),
        benchmark_package_refs: vec![refs.benchmark_package_ref.clone()],
        stage_policy_refs: vec![format!(
            "policy://training/apple_adapter/operator/{}/sft",
            refs.run_slug
        )],
        metadata: json!({
            "apple_adapter": apple_training_policy_metadata_value(summary, refs)
        }),
    }
}

#[derive(Clone)]
struct AppleAdapterKernelRefs {
    run_slug: String,
    version: String,
    core_environment_ref: String,
    benchmark_environment_ref: String,
    benchmark_package_ref: String,
    validator_policy_ref: String,
    training_policy_ref: String,
    training_run_id: String,
    held_out_eval_run_id: String,
    runtime_eval_run_id: String,
    accepted_outcome_id: String,
}

fn kernel_refs_for_run(run_id: &str) -> AppleAdapterKernelRefs {
    let slug = slugify(run_id);
    let version = "2026.03.15".to_string();
    AppleAdapterKernelRefs {
        run_slug: slug.clone(),
        version,
        core_environment_ref: format!("env.openagents.apple_adapter.operator.{slug}.core"),
        benchmark_environment_ref: format!(
            "env.openagents.apple_adapter.operator.{slug}.benchmark"
        ),
        benchmark_package_ref: format!("benchmark://apple_adapter/operator/{slug}"),
        validator_policy_ref: format!("policy://validator/apple_adapter/operator/{slug}"),
        training_policy_ref: format!("policy://training/apple_adapter/operator/{slug}"),
        training_run_id: format!("train.apple_adapter.operator.{slug}"),
        held_out_eval_run_id: format!("eval.apple_adapter.operator.{slug}.held_out"),
        runtime_eval_run_id: format!("eval.apple_adapter.operator.{slug}.runtime"),
        accepted_outcome_id: format!("accepted.training.apple_adapter.operator.{slug}"),
    }
}

fn benchmark_ref_for_run(run_id: &str) -> String {
    kernel_refs_for_run(run_id).benchmark_package_ref
}

fn validator_policy_ref_for_run(run_id: &str) -> String {
    kernel_refs_for_run(run_id).validator_policy_ref
}

fn accepted_outcome_id_for_run(run_id: &str) -> String {
    kernel_refs_for_run(run_id).accepted_outcome_id
}

fn benchmark_sample_kinds(
    run: &AppleAdapterOperatorRunStatus,
) -> Result<Vec<ComputeAppleAdapterSampleKind>, String> {
    let summary = run.local_summary.as_ref().ok_or_else(|| {
        format!(
            "Run `{}` is missing Apple adapter lineage summary",
            run.run_id
        )
    })?;
    let held_out_path = PathBuf::from(run.held_out_dataset_path.as_str());
    let held_out_dataset = load_dataset(
        held_out_path.as_path(),
        &runtime_profile_from_summary(summary),
    )
    .map_err(|error| error.to_string())?;
    let mut kinds = Vec::new();
    for sample in &held_out_dataset.samples {
        let kind = match sample.sample_kind {
            AppleAdapterSampleKind::SupervisedFineTune => {
                ComputeAppleAdapterSampleKind::SupervisedFineTune
            }
            AppleAdapterSampleKind::SchemaFreeGuidedGeneration => {
                ComputeAppleAdapterSampleKind::SchemaFreeGuidedGeneration
            }
            AppleAdapterSampleKind::GuidedGenerationWithSchema => {
                ComputeAppleAdapterSampleKind::GuidedGenerationWithSchema
            }
            AppleAdapterSampleKind::ToolCalling => ComputeAppleAdapterSampleKind::ToolCalling,
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    Ok(kinds)
}

pub(crate) fn build_environment_bundle(
    run_id: &str,
    train_dataset: &AppleAdapterDatasetContract,
    held_out_dataset: &AppleAdapterDatasetContract,
) -> Result<AppleAdapterEnvironmentBundle> {
    Ok(AppleAdapterEnvironmentSpec {
        version: "2026.03.15".to_string(),
        display_name: format!("Apple Adapter Operator {}", run_id),
        core_environment_ref: kernel_refs_for_run(run_id).core_environment_ref,
        benchmark_environment_ref: kernel_refs_for_run(run_id).benchmark_environment_ref,
        train_dataset: EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                dataset_ref_for_path("operator://train", "train"),
                "2026.03.15",
            ),
            split: Some("train".to_string()),
            mount_path: "/datasets/apple/train".to_string(),
            required: true,
        },
        held_out_eval_dataset: EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                dataset_ref_for_path("operator://held_out", "held_out"),
                "2026.03.15",
            ),
            split: Some("held_out".to_string()),
            mount_path: "/datasets/apple/held_out".to_string(),
            required: true,
        },
        benchmark_dataset: Some(EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                dataset_ref_for_path("operator://held_out", "held_out"),
                "2026.03.15",
            ),
            split: Some("held_out".to_string()),
            mount_path: "/datasets/apple/benchmark".to_string(),
            required: true,
        }),
        package_refs: AppleAdapterEnvironmentPackageRefs {
            group_ref: format!("group.apple_adapter.operator.{}", slugify(run_id)),
            core_pin_alias: format!("apple_adapter_operator_{}_core", slugify(run_id)),
            benchmark_pin_alias: format!("apple_adapter_operator_{}_benchmark", slugify(run_id)),
            core_member_ref: format!("apple_adapter_operator_{}_core_member", slugify(run_id)),
            benchmark_member_ref: format!(
                "apple_adapter_operator_{}_benchmark_member",
                slugify(run_id)
            ),
            session_profile_ref: format!("session://apple_adapter/operator/{}", slugify(run_id)),
            runtime_profile_ref: "runtime://apple/fm".to_string(),
            tool_bundle_ref: format!("tools://apple_adapter/operator/{}", slugify(run_id)),
            rubric_binding_ref: format!("rubric://apple_adapter/operator/{}", slugify(run_id)),
            structured_output_profile_ref: Some(format!(
                "structured://apple_adapter/operator/{}",
                slugify(run_id)
            )),
            benchmark_profile_ref: format!(
                "benchmark://apple_adapter/operator/{}/default",
                slugify(run_id)
            ),
            benchmark_runtime_profile_ref: format!(
                "runtime://apple_adapter/operator/{}/benchmark",
                slugify(run_id)
            ),
        },
        runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements {
            foundation_bridge_ref: "bridge://apple-foundation-models".to_string(),
            model_id: "apple-foundation-model".to_string(),
            platform_requirement: "macos26_apple_silicon".to_string(),
            adapter_inventory_required: true,
            session_attach_required: true,
            structured_output_supported: held_out_dataset
                .samples
                .iter()
                .any(|sample| sample.response_format.is_some()),
            tool_calling_supported: held_out_dataset
                .samples
                .iter()
                .any(|sample| !sample.tools.is_empty()),
            max_context_tokens: 4096,
            max_session_turns: 4,
            time_budget_ms: 30_000,
        },
        tools: train_dataset
            .samples
            .iter()
            .flat_map(|sample| sample.tools.iter())
            .map(|tool| EnvironmentToolContract {
                tool_name: tool.function.name.clone(),
                interface: EnvironmentToolInterface::NativeFunction,
                description: tool.function.description.clone().unwrap_or_default(),
                args_schema: tool.function.arguments.clone(),
                result_schema: None,
            })
            .collect(),
        rubric_hooks: vec![EnvironmentRubricHook {
            rubric_ref: format!(
                "rubric://apple_adapter/operator/{}/held_out",
                slugify(run_id)
            ),
            hook_name: "score_output".to_string(),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(0),
        }],
        expected_artifacts: vec![EnvironmentArtifactExpectation {
            artifact_kind: "training_manifest".to_string(),
            required: true,
            verification_policy_ref: Some(format!(
                "verify://apple_adapter/operator/{}/training_manifest",
                slugify(run_id)
            )),
        }],
        core_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Training,
            policy_ref: format!(
                "policy://training/apple_adapter/operator/{}/sft",
                slugify(run_id)
            ),
            required: true,
        }],
        benchmark_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Benchmark,
            policy_ref: format!(
                "policy://benchmark/apple_adapter/operator/{}/default",
                slugify(run_id)
            ),
            required: true,
        }],
        difficulty: Some(EnvironmentDifficultyMetadata {
            difficulty_tier: "operator".to_string(),
            min_agent_level: Some(1),
            tags: vec!["apple_adapter".to_string(), "repo_native".to_string()],
        }),
    }
    .build_bundle()?)
}

fn build_execution_packing_policy(
    captures: &[AppleAdapterSampleTokenCapture],
) -> DatasetPackingPolicy {
    let max_sample_tokens = captures
        .iter()
        .map(AppleAdapterSampleTokenCapture::total_tokens)
        .max()
        .unwrap_or(96)
        .max(96);
    let max_row_tokens = round_up_token_budget(max_sample_tokens, 8);
    let max_batch_tokens = round_up_token_budget(max_row_tokens.saturating_mul(2), 8);
    DatasetPackingPolicy::new(
        DatasetPackingMode::PackIntoContextWindow,
        max_row_tokens,
        max_batch_tokens.max(max_row_tokens),
        2,
    )
    .with_pad_to_multiple_of(8)
    .with_overlong_sequence_posture(OverlongSequencePosture::Refuse)
}

struct PsionicAppleTrainingArtifacts {
    final_checkpoint_path: PathBuf,
    checkpoint_size_bytes: u64,
}

fn load_apple_experiment_manifest(
    request: &AppleAdapterOperatorLaunchRequest,
) -> Result<Option<AppleAdapterExperimentManifest>, anyhow::Error> {
    let Some(path) = request
        .experiment_manifest_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read experiment manifest {}", path))?;
    let manifest = serde_json::from_str::<AppleAdapterExperimentManifest>(raw.as_str())
        .with_context(|| format!("failed to decode experiment manifest {}", path))?;
    manifest.validate()?;
    Ok(Some(manifest))
}

fn build_psionic_execution_config(
    run_id: &str,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
    packing_policy: &DatasetPackingPolicy,
    experiment_manifest: Option<&AppleAdapterExperimentManifest>,
) -> Result<AppleAdapterExecutionConfig, String> {
    let optimizer = if experiment_manifest.is_some() {
        TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8).with_gradient_clip_norm(1.0)
    } else {
        TrainingOptimizerConfig::adamw(0.01, 0.9, 0.99, 1e-8).with_gradient_clip_norm(1.0)
    };
    let target_policy = TrainingOptimizerResidencyPolicy::host_only();
    let targets = if let Some(manifest) = experiment_manifest {
        manifest_trainable_targets(manifest, &optimizer, target_policy)?
    } else {
        apple_live_reference_trainable_targets(optimizer.clone(), target_policy)
    };
    let max_steps = experiment_manifest.map_or(4, |manifest| manifest.max_steps);
    Ok(AppleAdapterExecutionConfig {
        run_id: run_id.to_string(),
        checkpoint_family: format!("{APPLE_TRAINING_CHECKPOINT_FAMILY}.psionic"),
        budget: TrainingLoopBudget {
            max_steps,
            steps_per_window: 1,
            windows_per_cadence: 1,
        },
        packing_policy: packing_policy.clone(),
        precision_policy: AppleAdapterPrecisionPolicy::F32Reference,
        activation_checkpoint_policy: AppleAdapterActivationCheckpointPolicy::Disabled,
        model: AppleAdapterReferenceModel {
            base_model_signature: runtime_profile.base_model_signature(),
            tokenizer_digest: runtime_profile
                .dataset_metadata()
                .tokenizer
                .tokenizer_digest,
            prompt_shaping_digest: runtime_profile.prompt_shaping_digest(),
            input_width: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
            output_width: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
            targets,
        },
    })
}

fn manifest_trainable_targets(
    manifest: &AppleAdapterExperimentManifest,
    optimizer: &TrainingOptimizerConfig,
    optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
) -> Result<Vec<psionic_train::AppleAdapterTrainableTarget>, String> {
    let mut targets = Vec::new();
    for symbolic_target in &manifest.lora_targets {
        let mut expanded = match symbolic_target.as_str() {
            "decoder.attn.q_proj" => {
                q_projection_trainable_targets(optimizer, optimizer_residency_policy)
            }
            "decoder.ffn.up_proj" => Vec::new(),
            other => {
                return Err(format!(
                    "Apple experiment manifest target `{other}` is not supported by the live runtime-exportable operator lane"
                ));
            }
        };
        targets.append(&mut expanded);
    }
    targets.sort_by(|lhs, rhs| lhs.target_id.cmp(&rhs.target_id));
    targets.dedup_by(|lhs, rhs| lhs.target_id == rhs.target_id);
    if targets.is_empty() {
        return Err(
            "Apple experiment manifest did not expand to any runtime-exportable targets"
                .to_string(),
        );
    }
    Ok(targets)
}

fn live_backend_skipped_manifest_targets(manifest: &AppleAdapterExperimentManifest) -> Vec<String> {
    manifest
        .lora_targets
        .iter()
        .filter(|target| target.as_str() == "decoder.ffn.up_proj")
        .cloned()
        .collect()
}

fn q_projection_trainable_targets(
    optimizer: &TrainingOptimizerConfig,
    optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
) -> Vec<psionic_train::AppleAdapterTrainableTarget> {
    let mut targets = Vec::new();
    for layer in
        APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER..APPLE_LIVE_REFERENCE_SEGMENT0_END_LAYER_EXCLUSIVE
    {
        targets.push(psionic_train::AppleAdapterTrainableTarget {
            target_id: format!(
                "layers.segment_0.layer_{layer}.attention.qkv_transform.adapters.base_adapter.lora_0"
            ),
            lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
            lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
            optimizer: optimizer.clone(),
            optimizer_residency_policy,
        });
    }
    for layer in
        APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER..APPLE_LIVE_REFERENCE_SEGMENT1_END_LAYER_EXCLUSIVE
    {
        targets.push(psionic_train::AppleAdapterTrainableTarget {
            target_id: format!(
                "layers.segment_1.layer_{layer}.attention.q_transform.adapters.base_adapter"
            ),
            lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
            lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
            optimizer: optimizer.clone(),
            optimizer_residency_policy,
        });
    }
    targets
}

fn build_psionic_sft_request(
    run_id: &str,
    request: &AppleAdapterOperatorLaunchRequest,
) -> AppleAdapterSftRunRequest {
    let slug = slugify(run_id);
    AppleAdapterSftRunRequest {
        dataset_ref: dataset_ref_for_path(request.train_dataset_path.as_str(), "train"),
        benchmark_refs: vec![format!(
            "benchmark://openagents/apple_adapter/{slug}/held_out"
        )],
        validator_policy_ref: format!("validator://openagents/apple_adapter/{slug}/held_out"),
        package_name: request.package_name.clone(),
        author: request.author.clone(),
        description: request.description.clone(),
        license: request.license.clone(),
        started_at_ms: current_epoch_ms(),
        step_duration_ms: 250,
    }
}

fn write_psionic_training_artifacts(
    run_directory: &Path,
    outcome: &AppleAdapterSftRunOutcome,
) -> Result<PsionicAppleTrainingArtifacts, String> {
    let root = run_directory.join("psionic");
    let checkpoints_dir = root.join("checkpoints");
    fs::create_dir_all(checkpoints_dir.as_path()).map_err(|error| {
        format!(
            "Failed to create Psionic checkpoint directory {}: {error}",
            checkpoints_dir.display()
        )
    })?;
    let initial_path = checkpoints_dir.join("adapter-initial.safetensors");
    let final_path = checkpoints_dir.join("adapter-final.safetensors");
    let (initial_bytes, _) = outcome
        .initial_bundle
        .export_safetensors()
        .map_err(|error| format!("Failed to export initial Psionic checkpoint: {error}"))?;
    let (final_bytes, _) = outcome
        .final_bundle
        .export_safetensors()
        .map_err(|error| format!("Failed to export final Psionic checkpoint: {error}"))?;
    fs::write(initial_path.as_path(), initial_bytes).map_err(|error| {
        format!(
            "Failed to write initial Psionic checkpoint {}: {error}",
            initial_path.display()
        )
    })?;
    fs::write(final_path.as_path(), final_bytes.as_slice()).map_err(|error| {
        format!(
            "Failed to write final Psionic checkpoint {}: {error}",
            final_path.display()
        )
    })?;

    let summary_path = root.join("training-summary.json");
    let summary_bytes = serde_json::to_vec_pretty(&outcome.summary)
        .map_err(|error| format!("Failed to encode Psionic training summary: {error}"))?;
    fs::write(summary_path.as_path(), summary_bytes).map_err(|error| {
        format!(
            "Failed to write Psionic training summary {}: {error}",
            summary_path.display()
        )
    })?;

    let receipts_path = root.join("step-receipts.json");
    let receipt_bytes = serde_json::to_vec_pretty(&outcome.step_receipts)
        .map_err(|error| format!("Failed to encode Psionic step receipts: {error}"))?;
    fs::write(receipts_path.as_path(), receipt_bytes).map_err(|error| {
        format!(
            "Failed to write Psionic step receipts {}: {error}",
            receipts_path.display()
        )
    })?;

    let gradients_path = root.join("gradient-records.json");
    let gradient_bytes = serde_json::to_vec_pretty(&outcome.gradient_records)
        .map_err(|error| format!("Failed to encode Psionic gradient records: {error}"))?;
    fs::write(gradients_path.as_path(), gradient_bytes).map_err(|error| {
        format!(
            "Failed to write Psionic gradient records {}: {error}",
            gradients_path.display()
        )
    })?;

    Ok(PsionicAppleTrainingArtifacts {
        final_checkpoint_path: final_path,
        checkpoint_size_bytes: match u64::try_from(final_bytes.len()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        },
    })
}

fn build_psionic_local_summary(
    outcome: &AppleAdapterSftRunOutcome,
    artifacts: &PsionicAppleTrainingArtifacts,
    captures: &[AppleAdapterSampleTokenCapture],
    training_wall_clock_ms: u64,
    export_wall_clock_ms: u64,
    held_out_eval: &EvalRunState,
    runtime_smoke: &AppleAdapterRuntimeSmokeReceipt,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
) -> AppleAdapterOperatorLocalSummary {
    let processed_token_count = Some(
        captures
            .iter()
            .map(|capture| u64::from(capture.total_tokens()))
            .sum::<u64>(),
    );
    let average_loss = (!outcome.step_receipts.is_empty()).then(|| {
        outcome
            .step_receipts
            .iter()
            .map(|receipt| f64::from(receipt.loss))
            .sum::<f64>()
            / outcome.step_receipts.len() as f64
    });
    let runtime_asset_size_bytes = outcome
        .adapter_package
        .inventory
        .iter()
        .find(|entry| entry.relative_path == "adapter_weights.bin")
        .map(|entry| entry.byte_length);
    AppleAdapterOperatorLocalSummary {
        completed_steps: outcome.summary.run_summary.completed_steps,
        expected_steps: outcome.summary.run_summary.budget.max_steps,
        average_loss,
        processed_token_count,
        training_backend: APPLE_PSIONIC_TRAINING_BACKEND_ID.to_string(),
        export_backend: APPLE_PSIONIC_EXPORT_BACKEND_ID.to_string(),
        training_wall_clock_ms: Some(training_wall_clock_ms),
        export_wall_clock_ms: Some(export_wall_clock_ms),
        training_max_resident_set_size_bytes: None,
        training_peak_memory_footprint_bytes: None,
        export_max_resident_set_size_bytes: None,
        export_peak_memory_footprint_bytes: None,
        checkpoint_size_bytes: Some(artifacts.checkpoint_size_bytes),
        runtime_asset_size_bytes,
        held_out_pass_rate_bps: held_out_eval
            .summary
            .as_ref()
            .map(|summary| summary.pass_rate_bps),
        held_out_average_score_bps: held_out_eval
            .summary
            .as_ref()
            .and_then(|summary| summary.average_score_bps),
        runtime_smoke_passed: Some(runtime_smoke.passed),
        runtime_smoke_digest: Some(runtime_smoke.smoke_digest.clone()),
        package_digest: Some(outcome.adapter_package.package_digest.clone()),
        adapter_identifier: Some(outcome.adapter_package.metadata.adapter_identifier.clone()),
        base_model_signature: outcome
            .adapter_package
            .metadata
            .base_model_signature
            .clone(),
        tokenizer_digest: runtime_profile
            .dataset_metadata()
            .tokenizer
            .tokenizer_digest,
        prompt_shaping_digest: runtime_profile.prompt_shaping_digest(),
        runtime_model_id: runtime_profile.model_id.clone(),
        runtime_use_case: runtime_profile.use_case.clone(),
        runtime_guardrails: runtime_profile.guardrails.clone(),
        locale: runtime_profile.locale.clone(),
        default_instruction: runtime_profile.default_instruction.clone(),
        bridge_version: runtime_profile.bridge_version.clone(),
        bridge_platform: runtime_profile.bridge_platform.clone(),
        package_format_version: APPLE_TRAINING_PACKAGE_FORMAT_VERSION.to_string(),
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    match u64::try_from(started.elapsed().as_millis()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

fn round_up_token_budget(value: u32, multiple: u32) -> u32 {
    if multiple <= 1 {
        return value.max(1);
    }
    let remainder = value % multiple;
    if remainder == 0 {
        value.max(multiple)
    } else {
        value.saturating_add(multiple - remainder)
    }
}

fn safe_adapter_package_name(package_name: &str) -> String {
    let mut safe = slugify(package_name).replace('-', "_");
    safe.retain(|character| character.is_ascii_alphanumeric() || character == '_');
    if safe.is_empty() {
        String::from("openagents_adapter")
    } else {
        safe
    }
}

fn validate_runtime_compatible_package_path(path: &Path) -> Result<(), String> {
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return Ok(());
    };
    if !file_name.ends_with(".fmadapter") {
        return Ok(());
    }
    let stem = file_name.trim_end_matches(".fmadapter");
    let safe = safe_adapter_package_name(stem);
    if stem == safe {
        Ok(())
    } else {
        Err(format!(
            "Apple Foundation Models runtime requires an underscore-safe `.fmadapter` directory name; use `{safe}.fmadapter` instead of `{file_name}`"
        ))
    }
}

fn derive_runtime_compatibility_profile(
    apple_fm_base_url: &str,
) -> Result<AppleAdapterRuntimeCompatibilityProfile> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("Failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let health = client.health().with_context(|| {
        format!("Failed to fetch Apple FM bridge health from {apple_fm_base_url}")
    })?;
    if !health.model_available {
        let detail = health
            .availability_message
            .clone()
            .unwrap_or_else(|| "Apple Foundation Models runtime is not ready".to_string());
        bail!("{detail}");
    }
    Ok(runtime_profile_from_health(&health))
}

fn runtime_profile_from_health(
    health: &AppleFmHealthResponse,
) -> AppleAdapterRuntimeCompatibilityProfile {
    let mut profile = AppleAdapterRuntimeCompatibilityProfile::new(
        DEFAULT_APPLE_FM_MODEL_ID,
        health.default_use_case.label(),
        health.default_guardrails.label(),
    );
    if let Some(locale) = operator_locale_tag() {
        profile = profile.with_locale(locale);
    }
    if let Some(version) = health
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        profile = profile.with_bridge_version(version.to_string());
    }
    if let Some(platform) = health
        .platform
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        profile = profile.with_bridge_platform(platform.to_string());
    }
    profile
}

pub(crate) fn runtime_profile_from_summary(
    summary: &AppleAdapterOperatorLocalSummary,
) -> AppleAdapterRuntimeCompatibilityProfile {
    let mut profile = AppleAdapterRuntimeCompatibilityProfile::new(
        summary.runtime_model_id.clone(),
        summary.runtime_use_case.clone(),
        summary.runtime_guardrails.clone(),
    );
    if let Some(locale) = &summary.locale {
        profile = profile.with_locale(locale.clone());
    }
    if !summary.base_model_signature.trim().is_empty() {
        profile = profile.with_base_model_signature(summary.base_model_signature.clone());
    }
    if let Some(default_instruction) = &summary.default_instruction {
        profile = profile.with_default_instruction(default_instruction.clone());
    }
    if let Some(bridge_version) = &summary.bridge_version {
        profile = profile.with_bridge_version(bridge_version.clone());
    }
    if let Some(bridge_platform) = &summary.bridge_platform {
        profile = profile.with_bridge_platform(bridge_platform.clone());
    }
    profile
}

pub(crate) fn runtime_profile_with_dataset_defaults(
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
    dataset: &AppleAdapterDatasetContract,
) -> AppleAdapterRuntimeCompatibilityProfile {
    let mut profile = runtime_profile.clone();
    if profile.default_instruction.is_none() {
        if let Some(default_instruction) = &dataset.metadata.default_instruction {
            profile = profile.with_default_instruction(default_instruction.clone());
        }
    }
    profile
}

fn operator_locale_tag() -> Option<String> {
    [
        std::env::var("LC_ALL").ok(),
        std::env::var("LC_MESSAGES").ok(),
        std::env::var("LANG").ok(),
    ]
    .into_iter()
    .flatten()
    .map(|value| normalize_locale_tag(value.as_str()))
    .find(|value| !value.is_empty())
}

fn normalize_locale_tag(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let without_encoding = trimmed.split('.').next().unwrap_or(trimmed);
    let without_modifier = without_encoding
        .split('@')
        .next()
        .unwrap_or(without_encoding);
    without_modifier.replace('_', "-")
}

fn run_local_held_out_eval(
    apple_fm_base_url: &str,
    package_path: &Path,
    environment: &AppleAdapterEnvironmentBundle,
    dataset: &AppleAdapterDatasetContract,
    run_id: &str,
) -> Result<EvalRunState> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("Failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let adapter = client
        .load_adapter(&AppleFmAdapterLoadRequest {
            package_path: package_path.display().to_string(),
            requested_adapter_id: Some(format!("operator-{}", slugify(run_id))),
        })
        .context("Failed to load staged Apple adapter into Apple FM bridge")?;
    let mut observed_outputs = Vec::with_capacity(dataset.samples.len());
    let eval_started = Instant::now();
    for (index, sample) in dataset.samples.iter().enumerate() {
        let instructions = sample
            .messages
            .iter()
            .find(|message| message.role == AppleAdapterMessageRole::System)
            .map(|message| message.content.clone());
        let prompt = sample
            .messages
            .iter()
            .find(|message| message.role == AppleAdapterMessageRole::User)
            .map(|message| message.content.clone())
            .ok_or_else(|| {
                anyhow!(
                    "Apple adapter held-out sample `{}` is missing a user prompt",
                    sample.sample_id
                )
            })?;
        let tool_recorder = AppleRepoLookupRecorder::default();
        let tools = build_repo_lookup_tools(sample.tools.as_slice(), tool_recorder.clone())?;
        let session = client
            .create_session_with_tools(
                &AppleFmSessionCreateRequest {
                    instructions,
                    model: None,
                    tools: Vec::new(),
                    adapter: None,
                    tool_callback: None,
                    transcript_json: None,
                    transcript: None,
                },
                tools,
            )
            .context("Failed to create Apple FM eval session")?;
        let _attached_session = client
            .attach_session_adapter(
                session.id.as_str(),
                &AppleFmAdapterAttachRequest {
                    adapter: adapter.adapter.clone(),
                },
            )
            .context("Failed to attach staged Apple adapter to eval session")?;
        let observed_output = if let Some(response_format) = sample.response_format.as_ref() {
            match client.respond_structured_in_session(
                session.id.as_str(),
                &AppleFmSessionStructuredGenerationRequest {
                    prompt,
                    schema: AppleFmGenerationSchema::with_title_hint(
                        response_format.json_schema.schema.clone(),
                        Some(response_format.json_schema.name.as_str()),
                    )?,
                    options: apple_eval_generation_options(),
                    adapter: None,
                },
            ) {
                Ok(response) => AppleAdapterObservedSampleOutput::from_text(
                    sample.sample_id.clone(),
                    response.content.to_json_string().unwrap_or_default(),
                )
                .with_structured_output(response.content.content),
                Err(error) => runtime_error_observed_output(
                    sample.sample_id.as_str(),
                    error.to_string(),
                    true,
                ),
            }
        } else {
            match client.respond_in_session(
                session.id.as_str(),
                &AppleFmSessionRespondRequest {
                    prompt,
                    options: apple_eval_generation_options(),
                    adapter: None,
                },
            ) {
                Ok(response) => AppleAdapterObservedSampleOutput::from_text(
                    sample.sample_id.clone(),
                    response.output,
                ),
                Err(error) => runtime_error_observed_output(
                    sample.sample_id.as_str(),
                    error.to_string(),
                    false,
                ),
            }
        };
        let observed_output = tool_recorder.attach_to_output(observed_output)?;
        let _ = client.delete_session(session.id.as_str());
        observed_outputs.push(observed_output);
        let completed_samples = index.saturating_add(1);
        let total_samples = dataset.samples.len();
        let eta_ms = held_out_eval_eta_ms(eval_started, completed_samples, total_samples)
            .unwrap_or_default();
        push_progress_event(
            run_id,
            AppleAdapterOperatorProgressUpdate::new(
                AppleAdapterOperatorProgressPhase::Evaluation,
                AppleAdapterOperatorProgressEventKind::HeldOutSampleCompleted,
                format!(
                    "Completed held-out eval sample {}/{} `{}`",
                    completed_samples, total_samples, sample.sample_id
                ),
            )
            .with_eval_sample(
                sample.sample_id.as_str(),
                completed_samples as u64,
                total_samples as u64,
            )
            .with_eta_ms(eta_ms),
        )
        .map_err(anyhow::Error::msg)?;
        push_progress_event(
            run_id,
            AppleAdapterOperatorProgressUpdate::new(
                AppleAdapterOperatorProgressPhase::Evaluation,
                AppleAdapterOperatorProgressEventKind::Heartbeat,
                format!(
                    "Held-out eval heartbeat sample={}/{} eta_ms={}",
                    completed_samples, total_samples, eta_ms
                ),
            )
            .with_eval_sample(
                sample.sample_id.as_str(),
                completed_samples as u64,
                total_samples as u64,
            )
            .with_eta_ms(eta_ms),
        )
        .map_err(anyhow::Error::msg)?;
    }
    let _ = client.unload_adapter(adapter.adapter.adapter_id.as_str());
    AppleAdapterEvalHarness::new(environment.clone())?
        .run_held_out_eval(
            format!("local-held-out-{}", slugify(run_id)),
            dataset,
            observed_outputs,
            current_epoch_ms(),
            current_epoch_ms() + 10,
        )
        .map_err(Into::into)
}

fn runtime_error_observed_output(
    sample_id: &str,
    error: String,
    structured: bool,
) -> AppleAdapterObservedSampleOutput {
    let mut observed = AppleAdapterObservedSampleOutput::from_text(
        sample_id.to_string(),
        format!("runtime_error:{error}"),
    );
    observed
        .metadata
        .insert(String::from("runtime_error"), Value::String(error));
    observed.metadata.insert(
        String::from("apple_adapter.runtime_failures"),
        json!([{
            "failure_code": "bridge_request_failed",
            "detail": observed
                .metadata
                .get("runtime_error")
                .and_then(Value::as_str)
                .unwrap_or_default()
        }]),
    );
    if structured {
        observed = observed.with_structured_output(Value::Null);
    }
    observed
}

pub(crate) fn apple_eval_generation_options() -> Option<AppleFmGenerationOptions> {
    AppleFmGenerationOptions::new(Some(AppleFmSamplingMode::greedy()), None, Some(96)).ok()
}

fn run_local_runtime_smoke(
    apple_fm_base_url: &str,
    package_path: &Path,
    environment: &AppleAdapterEnvironmentBundle,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
) -> Result<AppleAdapterRuntimeSmokeReceipt> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("Failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let harness = AppleAdapterEvalHarness::new(environment.clone())?;
    let request = build_runtime_smoke_request(package_path, runtime_profile);
    harness
        .run_runtime_smoke(&client, &request)
        .map_err(Into::into)
}

fn run_local_runtime_drift_check(
    apple_fm_base_url: &str,
    package_path: &Path,
    environment: &AppleAdapterEnvironmentBundle,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
    previous_receipt: &AppleAdapterRuntimeSmokeReceipt,
) -> Result<AppleAdapterRuntimeDriftReport> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("Failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let harness = AppleAdapterEvalHarness::new(environment.clone())?;
    let request = build_runtime_smoke_request(package_path, runtime_profile);
    Ok(harness.detect_runtime_drift(&client, &request, previous_receipt))
}

fn build_runtime_smoke_request(
    package_path: &Path,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
) -> AppleAdapterRuntimeSmokeRequest {
    AppleAdapterRuntimeSmokeRequest {
        package_path: package_path.display().to_string(),
        requested_adapter_id: Some("operator-runtime-smoke".to_string()),
        instructions: Some(
            "You are running the OpenAgents Apple adapter runtime smoke test.".to_string(),
        ),
        text_prompt: APPLE_TRAINING_RUNTIME_SMOKE_PROMPT.to_string(),
        expected_text_substring: None,
        expected_base_model_signature: Some(runtime_profile.base_model_signature()),
        expected_tokenizer_digest: Some(
            runtime_profile
                .dataset_metadata()
                .tokenizer
                .tokenizer_digest
                .clone(),
        ),
        expected_prompt_shaping_digest: Some(runtime_profile.prompt_shaping_digest()),
        structured_prompt: None,
        structured_schema: None,
        expected_structured_output: None,
        options: apple_eval_generation_options(),
    }
}

fn compute_eval_sample_from_eval_sample(
    sample: &psionic_eval::EvalSampleRecord,
) -> ComputeEvaluationSample {
    ComputeEvaluationSample {
        eval_run_id: String::new(),
        sample_id: sample.sample_id.clone(),
        ordinal: sample.ordinal,
        status: match sample.status {
            psionic_eval::EvalSampleStatus::Passed => ComputeEvaluationSampleStatus::Passed,
            psionic_eval::EvalSampleStatus::Failed => ComputeEvaluationSampleStatus::Failed,
            psionic_eval::EvalSampleStatus::Errored => ComputeEvaluationSampleStatus::Errored,
        },
        input_ref: sample.input_ref.clone(),
        output_ref: sample.output_ref.clone(),
        expected_output_ref: sample.expected_output_ref.clone(),
        score_bps: sample.score_bps,
        metrics: sample
            .metrics
            .iter()
            .map(compute_eval_metric_from_eval_metric)
            .collect(),
        artifacts: compute_eval_artifacts(sample.artifacts.as_slice()),
        error_reason: sample.error_reason.clone(),
        recorded_at_ms: i64::try_from(current_epoch_ms()).unwrap_or(i64::MAX),
        metadata: if sample.metadata.is_empty() {
            Value::Null
        } else {
            serde_json::to_value(&sample.metadata).unwrap_or(Value::Null)
        },
    }
}

fn compute_eval_summary_from_eval_summary(
    summary: &psionic_eval::EvalSummary,
) -> ComputeEvaluationSummary {
    ComputeEvaluationSummary {
        total_samples: summary.total_samples,
        scored_samples: summary.scored_samples,
        passed_samples: summary.passed_samples,
        failed_samples: summary.failed_samples,
        errored_samples: summary.errored_samples,
        average_score_bps: summary.average_score_bps,
        pass_rate_bps: Some(summary.pass_rate_bps),
        aggregate_metrics: summary
            .aggregate_metrics
            .iter()
            .map(compute_eval_metric_from_eval_metric)
            .collect(),
        artifacts: compute_eval_artifacts(summary.artifacts.as_slice()),
    }
}

fn compute_eval_metric_from_eval_metric(metric: &EvalMetric) -> ComputeEvaluationMetric {
    ComputeEvaluationMetric {
        metric_id: metric.metric_id.clone(),
        metric_value: metric.metric_value,
        unit: metric.unit.clone(),
        metadata: metric.metadata.clone(),
    }
}

fn compute_eval_artifacts(artifacts: &[EvalArtifact]) -> Vec<ComputeEvaluationArtifact> {
    artifacts
        .iter()
        .map(|artifact| ComputeEvaluationArtifact {
            artifact_kind: artifact.artifact_kind.clone(),
            artifact_ref: artifact.artifact_ref.clone(),
            digest: Some(artifact.artifact_digest.clone()),
            metadata: Value::Null,
        })
        .collect()
}

pub(crate) fn load_dataset(
    path: &Path,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
) -> Result<AppleAdapterDatasetContract> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Apple adapter dataset {}", path.display()))?;
    let mut dataset = AppleAdapterDatasetContract::from_jsonl_str(
        raw.as_str(),
        runtime_profile.dataset_metadata(),
    )
    .with_context(|| format!("Failed to import Apple adapter dataset {}", path.display()))?;
    if dataset.samples.iter().any(|sample| {
        sample
            .messages
            .first()
            .map(|message| message.role != AppleAdapterMessageRole::System)
            .unwrap_or(true)
    }) {
        dataset.metadata = runtime_profile
            .clone()
            .with_default_instruction(APPLE_ADAPTER_DEFAULT_INSTRUCTION)
            .dataset_metadata();
        dataset.validate()?;
    }
    Ok(dataset)
}

fn dataset_ref_for_path(path: &str, split: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(OsStr::to_str)
        .map(slugify)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "dataset".to_string());
    format!("dataset://openagents/apple_adapter/{stem}/{split}")
}

fn package_directory_name(package_name: &str) -> String {
    format!("{}.fmadapter", safe_adapter_package_name(package_name))
}

fn build_run_id(package_name: &str) -> String {
    let slug = slugify(package_name);
    format!("{}-{}", slug, current_epoch_ms())
}

fn normalize_nonterminal_runs(runs: &mut [AppleAdapterOperatorRunStatus]) {
    for run in runs {
        let mut interrupted = false;
        if run.launch_state == AppleAdapterOperatorStageState::Running {
            run.launch_state = AppleAdapterOperatorStageState::Interrupted;
            interrupted = true;
        }
        if run.evaluation_state == AppleAdapterOperatorStageState::Running {
            run.evaluation_state = AppleAdapterOperatorStageState::Interrupted;
            interrupted = true;
        }
        if run.export_state == AppleAdapterOperatorStageState::Running {
            run.export_state = AppleAdapterOperatorStageState::Interrupted;
            interrupted = true;
        }
        if run.acceptance_state == AppleAdapterOperatorStageState::Running {
            run.acceptance_state = AppleAdapterOperatorStageState::Interrupted;
            interrupted = true;
        }
        if interrupted {
            run.last_error = Some(
                "Autopilot restarted before the Apple adapter operator flow reached a terminal stage"
                    .to_string(),
            );
            run.last_action = Some("Recovered interrupted Apple adapter operator run".to_string());
        }
    }
}

fn copy_directory(source: &Path, target: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!("Source {} is not a directory", source.display());
    }
    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("Failed to replace export target {}", target.display()))?;
    }
    fs::create_dir_all(target)
        .with_context(|| format!("Failed to create export target {}", target.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("Failed to read directory {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(source_path.as_path(), target_path.as_path())?;
        } else {
            fs::copy(source_path.as_path(), target_path.as_path()).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            previous_dash = false;
            ch.to_ascii_lowercase()
        } else if previous_dash {
            continue;
        } else {
            previous_dash = true;
            '-'
        };
        output.push(normalized);
    }
    output.trim_matches('-').to_string()
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn apple_training_root_dir() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(APPLE_TRAINING_ROOT_DIR)
}

fn apple_training_state_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(APPLE_TRAINING_STATE_FILENAME)
}

fn with_controller<T>(
    f: impl FnOnce(&mut AppleAdapterTrainingController) -> Result<T, String>,
) -> Result<T, String> {
    let controller = APPLE_TRAINING_CONTROLLER.get_or_init(|| {
        Mutex::new(AppleAdapterTrainingController::load(
            apple_training_state_path(),
        ))
    });
    let mut controller = controller
        .lock()
        .map_err(|_| "Apple adapter training controller lock poisoned".to_string())?;
    f(&mut controller)
}

trait FinalizeComputeEvaluationRunRequestExt {
    fn with_summary(self, summary: ComputeEvaluationSummary) -> Self;
}

impl FinalizeComputeEvaluationRunRequestExt for FinalizeComputeEvaluationRunRequest {
    fn with_summary(mut self, summary: ComputeEvaluationSummary) -> Self {
        self.metadata = match self.metadata {
            Value::Object(mut object) => {
                object.insert(
                    "summary".to_string(),
                    serde_json::to_value(&summary).unwrap_or(Value::Null),
                );
                Value::Object(object)
            }
            _ => json!({ "summary": summary }),
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APPLE_LIVE_REFERENCE_SEGMENT0_END_LAYER_EXCLUSIVE,
        APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER,
        APPLE_LIVE_REFERENCE_SEGMENT1_END_LAYER_EXCLUSIVE,
        APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER, APPLE_PSIONIC_EXPORT_BACKEND_ID,
        APPLE_PSIONIC_TRAINING_BACKEND_ID, build_psionic_execution_config,
        q_projection_trainable_targets,
    };
    use psionic_data::{
        AppleAdapterRuntimeCompatibilityProfile, DatasetKey, DatasetPackingMode,
        DatasetPackingPolicy,
    };
    use psionic_train::{
        APPLE_ADAPTER_EXPERIMENT_MANIFEST_ABI_VERSION, APPLE_LIVE_REFERENCE_LORA_RANK,
        AppleAdapterExperimentManifest, AppleAdapterUsefulAdapterAcceptanceGate,
        TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
    };

    #[test]
    fn apple_operator_authoritative_backends_are_rust_only() {
        for backend in [
            APPLE_PSIONIC_TRAINING_BACKEND_ID,
            APPLE_PSIONIC_EXPORT_BACKEND_ID,
        ] {
            assert!(
                !backend.contains("toolkit"),
                "authoritative live backend should not reference toolkit: {backend}"
            );
            assert!(
                !backend.contains("python"),
                "authoritative live backend should not reference python: {backend}"
            );
        }
    }

    #[test]
    fn symbolic_q_projection_targets_expand_to_exportable_runtime_targets() {
        let optimizer =
            TrainingOptimizerConfig::adamw(0.01, 0.9, 0.99, 1e-8).with_gradient_clip_norm(1.0);
        let targets = q_projection_trainable_targets(
            &optimizer,
            TrainingOptimizerResidencyPolicy::host_only(),
        );
        assert_eq!(
            targets.len(),
            (APPLE_LIVE_REFERENCE_SEGMENT0_END_LAYER_EXCLUSIVE
                - APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER)
                + (APPLE_LIVE_REFERENCE_SEGMENT1_END_LAYER_EXCLUSIVE
                    - APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER)
        );
        assert!(targets.iter().all(|target| {
            target.lora_rank == APPLE_LIVE_REFERENCE_LORA_RANK
                && (target
                    .target_id
                    .contains("qkv_transform.adapters.base_adapter.lora_0")
                    || target
                        .target_id
                        .contains("attention.q_transform.adapters.base_adapter"))
        }));
    }

    #[test]
    fn manifest_backed_psionic_execution_config_respects_manifest_step_budget() {
        let runtime_profile = AppleAdapterRuntimeCompatibilityProfile::new(
            "apple-foundation-model",
            "general",
            "default",
        );
        let packing_policy =
            DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 256, 512, 1);
        let manifest = AppleAdapterExperimentManifest {
            abi_version: APPLE_ADAPTER_EXPERIMENT_MANIFEST_ABI_VERSION.to_string(),
            experiment_id: "apple_adapter.test.max_steps".to_string(),
            target_id: "apple_adapter.test".to_string(),
            dataset: DatasetKey::new("dataset://openagents/apple_adapter/test", "2026.03.16"),
            train_split_digest: "sha256:train".to_string(),
            held_out_split_digest: "sha256:held".to_string(),
            benchmark_split_digest: "sha256:bench".to_string(),
            corpus_manifest_digest: "sha256:corpus".to_string(),
            base_model_signature: "9799725ff8e851184037110b422d891ad3b92ec1".to_string(),
            tokenizer_digest: "sha256:tokenizer".to_string(),
            prompt_shaping_digest: "sha256:prompt".to_string(),
            environment_ref: "env.openagents.apple.test".to_string(),
            benchmark_ref: "benchmark://openagents/apple_adapter/test".to_string(),
            fidelity_plan_id: "openagents.apple.token_sequence_reference.v1".to_string(),
            input_width: 48,
            output_width: 24,
            lora_targets: vec!["decoder.attn.q_proj".to_string()],
            lora_rank: 4,
            max_steps: 8,
            useful_adapter_gate: AppleAdapterUsefulAdapterAcceptanceGate {
                runtime_smoke_required: true,
                standard_benchmark_policy:
                    psionic_eval::AppleAdapterBaseVsAdapterAcceptancePolicy {
                        minimum_adapter_score_bps: 0,
                        minimum_adapter_pass_rate_bps: 0,
                        minimum_score_delta_bps: 0,
                        minimum_pass_rate_delta_bps: 0,
                        minimum_improved_case_count: 0,
                    },
                overfit_non_zero_policy: psionic_eval::AppleAdapterBaseVsAdapterAcceptancePolicy {
                    minimum_adapter_score_bps: 1,
                    minimum_adapter_pass_rate_bps: 1,
                    minimum_score_delta_bps: 1,
                    minimum_pass_rate_delta_bps: 1,
                    minimum_improved_case_count: 1,
                },
            },
        };

        let config = build_psionic_execution_config(
            "test-run",
            &runtime_profile,
            &packing_policy,
            Some(&manifest),
        )
        .expect("manifest-backed config should build");

        assert_eq!(config.budget.max_steps, 8);
    }
}
