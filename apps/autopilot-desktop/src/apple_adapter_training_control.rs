use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use openagents_kernel_core::authority::{
    AcceptComputeOutcomeRequest, CreateComputeEvaluationRunRequest,
    CreateComputeTrainingRunRequest, FinalizeComputeEvaluationRunRequest,
    FinalizeComputeTrainingRunRequest, HttpKernelAuthorityClient, KernelAuthority,
    RegisterComputeBenchmarkPackageRequest, RegisterComputeCheckpointFamilyPolicyRequest,
    RegisterComputeEnvironmentPackageRequest, RegisterComputeTrainingPolicyRequest,
    RegisterComputeValidatorPolicyRequest,
};
use openagents_kernel_core::compute::{
    ComputeAcceptedOutcome, ComputeAppleAdapterSampleKind, ComputeAppleBenchmarkPackageMetadata,
    ComputeAppleRuntimeValidationPosture, ComputeAppleTrainingPolicyMetadata,
    ComputeAppleTrainingRunMetadata, ComputeBenchmarkPackage, ComputeCheckpointBinding,
    ComputeCheckpointFamilyPolicy, ComputeEnvironmentArtifactExpectation,
    ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
    ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
    ComputeEvaluationArtifact, ComputeEvaluationMetric, ComputeEvaluationRun,
    ComputeEvaluationRunStatus, ComputeEvaluationSample, ComputeEvaluationSampleStatus,
    ComputeEvaluationSummary, ComputeProofPosture, ComputeRegistryStatus, ComputeTrainingPolicy,
    ComputeTrainingRun, ComputeTrainingRunStatus, ComputeTrainingSummary, ComputeValidatorPolicy,
    COMPUTE_APPLE_BENCHMARK_PACKAGE_METADATA_ABI_VERSION,
    COMPUTE_APPLE_TRAINING_POLICY_METADATA_ABI_VERSION,
    COMPUTE_APPLE_TRAINING_RUN_METADATA_ABI_VERSION,
};
use openagents_kernel_core::compute_benchmarks::ComputeBenchmarkAdapterKind;
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::{PolicyContext, ReceiptHints, TraceContext};
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterLoadRequest, AppleFmBridgeClient,
    AppleFmGeneratedContent, AppleFmGenerationSchema, AppleFmHealthResponse,
    AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
    AppleFmSessionStructuredGenerationRequest, AppleFmTool, AppleFmToolCallError,
    AppleFmToolDefinition, DEFAULT_APPLE_FM_MODEL_ID,
};
use psionic_data::{
    AppleAdapterDatasetContract, AppleAdapterMessageRole, AppleAdapterRuntimeCompatibilityProfile,
    AppleAdapterSampleKind, DatasetKey, DatasetPackingMode, DatasetPackingPolicy,
    OverlongSequencePosture, APPLE_ADAPTER_DEFAULT_INSTRUCTION,
};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentPackageRefs,
    AppleAdapterEnvironmentRuntimeRequirements, AppleAdapterEnvironmentSpec,
    EnvironmentArtifactExpectation, EnvironmentDatasetBinding, EnvironmentDifficultyMetadata,
    EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
    EnvironmentRubricScoreKind, EnvironmentToolContract, EnvironmentToolInterface,
};
use psionic_eval::{
    AppleAdapterEvalHarness, AppleAdapterObservedSampleOutput, AppleAdapterObservedToolCall,
    AppleAdapterRuntimeSmokeReceipt, AppleAdapterRuntimeSmokeRequest, EvalArtifact, EvalMetric,
    EvalRunState,
};
use psionic_train::{
    run_apple_adapter_sft_export, AppleAdapterActivationCheckpointPolicy,
    AppleAdapterExecutionConfig, AppleAdapterPrecisionPolicy, AppleAdapterReferenceModel,
    AppleAdapterSftRunOutcome, AppleAdapterSftRunRequest, AppleAdapterTrainableTarget,
    AppleAdapterTrainingExecutionBackend, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const APPLE_TRAINING_SCHEMA_VERSION: u16 = 1;
const APPLE_TRAINING_STATE_FILENAME: &str = "apple-adapter-training.json";
const APPLE_TRAINING_ROOT_DIR: &str = "apple-adapter-training";
const APPLE_TRAINING_LOG_LIMIT: usize = 64;
const APPLE_TRAINING_PACKAGE_FORMAT_VERSION: &str = "openagents.apple-fmadapter.v1";
const APPLE_TRAINING_OWNER_ID: &str = "openagents.autopilot_desktop";
const APPLE_TRAINING_CHECKPOINT_FAMILY: &str = "apple_adapter";
const APPLE_TRAINING_CHECKPOINT_POLICY_VERSION: &str = "2026.03.15";
const APPLE_TRAINING_RUNTIME_VALIDATION_POSTURE: ComputeAppleRuntimeValidationPosture =
    ComputeAppleRuntimeValidationPosture::HeldOutAndRuntimeSmoke;
const APPLE_TRAINING_PRODUCT_ID: &str = "psionic.training.apple_adapter.sft";
const APPLE_TRAINING_RUNTIME_SMOKE_PROMPT: &str =
    "Explain what a mutex does in one short sentence.";

static APPLE_TRAINING_CONTROLLER: OnceLock<Mutex<AppleAdapterTrainingController>> = OnceLock::new();

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterOperatorLocalSummary {
    pub completed_steps: u64,
    pub expected_steps: u64,
    pub average_loss: Option<f64>,
    pub processed_token_count: Option<u64>,
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
            log_lines: vec![format!("{} launch: created operator run {}", now, run_id)],
        };
        self.state.last_action = Some(format!("Created Apple adapter operator run {}", run_id));
        self.state.last_error = None;
        self.state.runs.insert(0, run.clone());
        self.persist()?;
        Ok(run)
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
        run.log_lines.push(line);
        if run.log_lines.len() > APPLE_TRAINING_LOG_LIMIT {
            let trim = run.log_lines.len() - APPLE_TRAINING_LOG_LIMIT;
            run.log_lines.drain(0..trim);
        }
        self.state.last_action = Some(action);
        self.state.last_error = None;
        self.persist()
    }

    fn set_failure(&mut self, run_id: &str, stage: AppleAdapterFailureStage, error: &str) {
        let now = current_epoch_ms();
        if let Ok(run) = self.run_mut(run_id) {
            run.updated_at_epoch_ms = now;
            run.last_error = Some(error.to_string());
            run.last_action = Some(stage.action_label().to_string());
            run.log_lines.push(format!("{} error: {}", now, error));
            if run.log_lines.len() > APPLE_TRAINING_LOG_LIMIT {
                let trim = run.log_lines.len() - APPLE_TRAINING_LOG_LIMIT;
                run.log_lines.drain(0..trim);
            }
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
    Ok(())
}

fn execute_launch_pipeline(
    run_id: &str,
    request: &AppleAdapterOperatorLaunchRequest,
) -> Result<(), String> {
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

    let runtime_profile = derive_runtime_compatibility_profile(request.apple_fm_base_url.as_str())
        .map_err(|error| error.to_string())?;
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

    let environment = build_environment_bundle(run_id, &train_dataset, &held_out_dataset)
        .map_err(|error| error.to_string())?;
    let captures = train_dataset
        .derive_token_captures()
        .map_err(|error| error.to_string())?;
    let config = build_execution_config(
        run_id,
        &train_dataset,
        runtime_profile.base_model_signature(),
    );
    let backend = AppleAdapterTrainingExecutionBackend::new(
        config,
        &train_dataset,
        captures.as_slice(),
        &environment,
    )
    .map_err(|error| format!("Failed to build Apple adapter backend: {error}"))?;
    let started_at_ms = current_epoch_ms();
    let sft_request = AppleAdapterSftRunRequest {
        dataset_ref: dataset_ref_for_path(request.train_dataset_path.as_str(), "train"),
        benchmark_refs: vec![benchmark_ref_for_run(run_id)],
        validator_policy_ref: validator_policy_ref_for_run(run_id),
        package_name: request.package_name.clone(),
        author: request.author.clone(),
        description: request.description.clone(),
        license: request.license.clone(),
        started_at_ms,
        step_duration_ms: 40,
    };

    with_controller(|controller| {
        controller.push_log(
            run_id,
            "launch: executing repo-native Apple adapter SFT export",
            "Running repo-native Apple adapter training",
        )
    })?;
    let outcome =
        run_apple_adapter_sft_export(&backend, &train_dataset, &environment, &sft_request)
            .map_err(|error| format!("Apple adapter SFT export failed: {error}"))?;
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
    outcome
        .write_package_to_directory(staged_package_path.as_path())
        .map_err(|error| format!("Failed to write staged Apple adapter package: {error}"))?;

    with_controller(|controller| {
        let run = controller.run_mut(run_id)?;
        run.staged_package_path = Some(staged_package_path.display().to_string());
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
    let held_out_eval = run_local_held_out_eval(
        request.apple_fm_base_url.as_str(),
        staged_package_path.as_path(),
        &environment,
        &held_out_dataset,
        run_id,
    )
    .map_err(|error| format!("Held-out Apple adapter eval failed: {error}"))?;
    let runtime_smoke = run_local_runtime_smoke(
        request.apple_fm_base_url.as_str(),
        staged_package_path.as_path(),
        &environment,
        &runtime_profile,
    )
    .map_err(|error| format!("Apple adapter runtime smoke failed: {error}"))?;

    let local_summary =
        build_local_summary(&outcome, &held_out_eval, &runtime_smoke, &runtime_profile);
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

fn build_environment_bundle(
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

fn build_execution_config(
    run_id: &str,
    dataset: &AppleAdapterDatasetContract,
    base_model_signature: String,
) -> AppleAdapterExecutionConfig {
    AppleAdapterExecutionConfig {
        run_id: format!("apple-train-{}", slugify(run_id)),
        checkpoint_family: APPLE_TRAINING_CHECKPOINT_FAMILY.to_string(),
        budget: TrainingLoopBudget::new(dataset.samples.len().max(1) as u64, 1, 1)
            .expect("valid fixed budget"),
        packing_policy: DatasetPackingPolicy::new(
            DatasetPackingMode::PackIntoContextWindow,
            96,
            192,
            2,
        )
        .with_pad_to_multiple_of(8)
        .with_overlong_sequence_posture(OverlongSequencePosture::Refuse),
        precision_policy: AppleAdapterPrecisionPolicy::F32Reference,
        activation_checkpoint_policy: AppleAdapterActivationCheckpointPolicy::Disabled,
        model: AppleAdapterReferenceModel {
            base_model_signature,
            tokenizer_digest: dataset.metadata.tokenizer.tokenizer_digest.clone(),
            prompt_shaping_digest: dataset.metadata.prompt_shaping_digest.clone(),
            input_width: 48,
            output_width: 24,
            targets: vec![
                AppleAdapterTrainableTarget {
                    target_id: "decoder.attn.q_proj".to_string(),
                    lora_rank: 4,
                    lora_alpha: 8.0,
                    optimizer: TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8)
                        .with_gradient_clip_norm(1.0),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
                AppleAdapterTrainableTarget {
                    target_id: "decoder.ffn.up_proj".to_string(),
                    lora_rank: 4,
                    lora_alpha: 8.0,
                    optimizer: TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8)
                        .with_gradient_clip_norm(1.0),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            ],
        },
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

fn runtime_profile_from_summary(
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

fn runtime_profile_with_dataset_defaults(
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
    for sample in &dataset.samples {
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
        let tool_recorder =
            std::sync::Arc::new(Mutex::new(Vec::<AppleAdapterObservedToolCall>::new()));
        let tools = sample
            .tools
            .iter()
            .map(|tool| {
                Ok(std::sync::Arc::new(RecordingTool {
                    definition: AppleFmToolDefinition::new(
                        tool.function.name.clone(),
                        tool.function.description.clone(),
                        AppleFmGenerationSchema::new(tool.function.arguments.clone())?,
                    ),
                    recorder: tool_recorder.clone(),
                }) as std::sync::Arc<dyn AppleFmTool>)
            })
            .collect::<Result<Vec<_>>>()?;
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
            let response = client
                .respond_structured_in_session(
                    session.id.as_str(),
                    &AppleFmSessionStructuredGenerationRequest {
                        prompt,
                        schema: AppleFmGenerationSchema::new(
                            response_format.json_schema.schema.clone(),
                        )?,
                        options: None,
                        adapter: None,
                    },
                )
                .context("Failed to run Apple FM structured held-out sample")?;
            AppleAdapterObservedSampleOutput::from_text(
                sample.sample_id.clone(),
                response.content.to_json_string().unwrap_or_default(),
            )
            .with_structured_output(response.content.content)
        } else {
            let response = client
                .respond_in_session(
                    session.id.as_str(),
                    &AppleFmSessionRespondRequest {
                        prompt,
                        options: None,
                        adapter: None,
                    },
                )
                .context("Failed to run Apple FM held-out text sample")?;
            AppleAdapterObservedSampleOutput::from_text(sample.sample_id.clone(), response.output)
        };
        let observed_tool_calls = tool_recorder
            .lock()
            .map_err(|_| anyhow!("Apple adapter eval tool recorder lock poisoned"))?
            .clone();
        let observed_output = if observed_tool_calls.is_empty() {
            observed_output
        } else {
            observed_output.with_tool_calls(observed_tool_calls)
        };
        let _ = client.delete_session(session.id.as_str());
        observed_outputs.push(observed_output);
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
    harness
        .run_runtime_smoke(
            &client,
            &AppleAdapterRuntimeSmokeRequest {
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
                options: None,
            },
        )
        .map_err(Into::into)
}

fn build_local_summary(
    outcome: &AppleAdapterSftRunOutcome,
    held_out_eval: &EvalRunState,
    runtime_smoke: &AppleAdapterRuntimeSmokeReceipt,
    runtime_profile: &AppleAdapterRuntimeCompatibilityProfile,
) -> AppleAdapterOperatorLocalSummary {
    let processed_token_count = Some(
        outcome
            .gradient_records
            .iter()
            .map(|record| u64::from(record.training_batch.sample_count))
            .sum::<u64>(),
    );
    let average_loss = if outcome.step_receipts.is_empty() {
        None
    } else {
        Some(
            outcome
                .step_receipts
                .iter()
                .map(|receipt| f64::from(receipt.loss))
                .sum::<f64>()
                / outcome.step_receipts.len() as f64,
        )
    };
    AppleAdapterOperatorLocalSummary {
        completed_steps: outcome.summary.run_summary.completed_steps,
        expected_steps: outcome.summary.run_summary.budget.max_steps,
        average_loss,
        processed_token_count,
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
        package_digest: Some(outcome.summary.package_digest.clone()),
        adapter_identifier: Some(outcome.summary.adapter_identifier.clone()),
        base_model_signature: outcome.summary.base_model_signature.clone(),
        tokenizer_digest: outcome
            .final_bundle
            .tokenizer
            .digest
            .tokenizer_digest
            .clone(),
        prompt_shaping_digest: outcome
            .final_bundle
            .tokenizer
            .digest
            .template_digest
            .clone()
            .unwrap_or_else(|| runtime_profile.prompt_shaping_digest()),
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

fn load_dataset(
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
    let trimmed = package_name.trim();
    if trimmed.ends_with(".fmadapter") {
        trimmed.to_string()
    } else {
        format!("{trimmed}.fmadapter")
    }
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

#[derive(Clone)]
struct RecordingTool {
    definition: AppleFmToolDefinition,
    recorder: std::sync::Arc<Mutex<Vec<AppleAdapterObservedToolCall>>>,
}

impl AppleFmTool for RecordingTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let argument_payload = arguments.content.clone();
        let mut recorder = self.recorder.lock().map_err(|_| {
            AppleFmToolCallError::new(self.definition.name.clone(), "tool recorder lock poisoned")
        })?;
        recorder.push(AppleAdapterObservedToolCall {
            tool_name: self.definition.name.clone(),
            succeeded: true,
            arguments: Some(argument_payload.clone()),
        });
        serde_json::to_string(&json!({
            "tool_name": self.definition.name,
            "arguments": argument_payload,
            "ok": true,
        }))
        .map_err(|error| AppleFmToolCallError::new(self.definition.name.clone(), error.to_string()))
    }
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
