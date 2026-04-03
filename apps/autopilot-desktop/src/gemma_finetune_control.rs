use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    GEMMA_E4B_BASELINE_SWEEP_REQUEST_SCHEMA_VERSION,
    GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION,
    GEMMA_E4B_FINETUNE_EVAL_RECEIPT_SCHEMA_VERSION, GEMMA_E4B_OPERATOR_REVIEW_SCHEMA_VERSION,
    GemmaE4bAssistantMaskKind, GemmaE4bBaselineCandidate, GemmaE4bBaselineSweepOutcome,
    GemmaE4bBaselineSweepRequest, GemmaE4bBenchmarkOverlapCheck,
    GemmaE4bCheckpointPromotionDecision, GemmaE4bCudaAdapterCheckpoint,
    GemmaE4bCudaAdapterExportRequest, GemmaE4bCudaAdapterExportedArtifact,
    GemmaE4bCudaAdapterSftConfig, GemmaE4bCudaAdapterSftTrainer, GemmaE4bCudaAdapterTargetSet,
    GemmaE4bFinetuneDatasetContract, GemmaE4bFinetuneEvalPackBinding, GemmaE4bFinetuneEvalReceipt,
    GemmaE4bFinetuneEvalSubjectKind, GemmaE4bLmHeadSupervisionSample,
    GemmaE4bOperatorPromotionReview, GemmaE4bOperatorReviewCaseVerdict,
    GemmaE4bOperatorReviewState, GemmaE4bPromotedCheckpointVibePacket,
    GemmaE4bPromotionDecisionState, GemmaE4bReviewVerdictStatus, GemmaE4bServedBaseModelBinding,
    TrainingLoopBudget, TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
    canonical_gemma_e4b_cuda_adapter_target_set, canonical_gemma_e4b_finetune_eval_pack_binding,
    canonical_gemma_e4b_finetuning_mvp_contract,
    canonical_gemma_e4b_promoted_checkpoint_vibe_packet, decide_gemma_e4b_checkpoint_promotion,
    run_gemma_e4b_baseline_sweep,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const GEMMA_FINETUNE_SCHEMA_VERSION: u16 = 2;
const GEMMA_FINETUNE_STATE_FILENAME: &str = "gemma-finetune.json";
const GEMMA_FINETUNE_ROOT_DIR: &str = "gemma-finetune";
const DEFAULT_TENANT_ID: &str = "desktop-owner";
const GEMMA_FINETUNE_EVAL_BENCHMARK_REF: &str = "benchmark://psionic/gemma4/e4b/finetune_eval";
const GEMMA_FINETUNE_VALIDATOR_POLICY_REF: &str = "policy://validator/gemma4/e4b-text-sft";
const GEMMA_FINETUNE_DEFAULT_STEP_DURATION_MS: u64 = 25;
const GEMMA_FINETUNE_DEFAULT_MAX_STEPS: u64 = 4;
const GEMMA_FINETUNE_DEFAULT_BATCH_SIZE: usize = 2;
const GEMMA_FINETUNE_DEFAULT_LEARNING_RATE: f32 = 0.08;
const GEMMA_FINETUNE_DEFAULT_WEIGHT_DECAY: f32 = 0.01;
const GEMMA_FINETUNE_DEFAULT_BETA1: f32 = 0.9;
const GEMMA_FINETUNE_DEFAULT_BETA2: f32 = 0.99;
const GEMMA_FINETUNE_DEFAULT_EPSILON: f32 = 1e-8;
const GEMMA_FINETUNE_DEFAULT_GRADIENT_CLIP_NORM: f32 = 1.0;
const GEMMA_FINETUNE_EVENT_LIMIT: usize = 128;
const GEMMA_FINETUNE_CHECKPOINT_LIMIT: usize = 16;
const GEMMA_FINETUNE_ARTIFACT_LIMIT: usize = 8;

static GEMMA_FINETUNE_CONTROLLER: OnceLock<Mutex<GemmaFinetuneController>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GemmaFinetuneProjectCreateRequest {
    pub project_name: String,
    pub tenant_id: Option<String>,
    pub base_served_artifact_digest: String,
    pub hidden_size: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GemmaFinetuneDatasetRegisterRequest {
    pub project_id: String,
    pub dataset_ref: String,
    pub train_path: String,
    pub validation_path: String,
    pub holdout_path: String,
    pub baseline_short_path: Option<String>,
    pub final_report_path: Option<String>,
    pub chat_template_digest: Option<String>,
    pub assistant_mask_coverage_bps: u32,
    pub overlap_check_id: Option<String>,
    pub overlap_detail: Option<String>,
    pub compared_benchmark_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GemmaFinetuneJobCreateRequest {
    pub project_id: String,
    pub dataset_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GemmaFinetuneCheckpointPromotionRequest {
    pub job_id: String,
    pub checkpoint_id: Option<String>,
    pub reviewer_id: String,
    pub review_state: Option<String>,
    pub failed_case_ids: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaFinetuneJobState {
    #[default]
    Pending,
    Running,
    Completed,
    CancelRequested,
    Cancelled,
    Failed,
}

impl GemmaFinetuneJobState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaFinetuneJobPhase {
    #[default]
    Queued,
    BaselineSweep,
    Training,
    AwaitingPromotion,
    Promoted,
    Cancelled,
    Failed,
}

impl GemmaFinetuneJobPhase {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::BaselineSweep => "baseline_sweep",
            Self::Training => "training",
            Self::AwaitingPromotion => "awaiting_promotion",
            Self::Promoted => "promoted",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaFinetuneJobEventKind {
    Created,
    Started,
    BaselineSweepCompleted,
    StepCompleted,
    CheckpointWritten,
    ArtifactExported,
    EvalReceiptRecorded,
    CancelRequested,
    Cancelled,
    PromotionRecorded,
    Promoted,
    Completed,
    Failed,
}

impl GemmaFinetuneJobEventKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Started => "started",
            Self::BaselineSweepCompleted => "baseline_sweep_completed",
            Self::StepCompleted => "step_completed",
            Self::CheckpointWritten => "checkpoint_written",
            Self::ArtifactExported => "artifact_exported",
            Self::EvalReceiptRecorded => "eval_receipt_recorded",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
            Self::PromotionRecorded => "promotion_recorded",
            Self::Promoted => "promoted",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneLaneBindingStatus {
    pub model_id: String,
    pub model_family: String,
    pub training_family_id: String,
    pub checkpoint_family: String,
    pub base_model_revision: String,
    pub adapter_family: String,
    pub eval_pack_storage_key: String,
    pub tokenizer_contract_digest: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneProjectStatus {
    pub project_id: String,
    pub tenant_id: String,
    pub project_name: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub base_served_artifact_digest: String,
    pub hidden_size: usize,
    pub active_dataset_id: Option<String>,
    pub lane_binding: GemmaFinetuneLaneBindingStatus,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneSplitFileStatus {
    pub split_id: String,
    pub split_ref: String,
    pub file_path: String,
    pub file_digest: String,
    pub sample_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneValidationReceiptStatus {
    pub receipt_id: String,
    pub validated_at_epoch_ms: u64,
    pub status: String,
    pub tokenizer_contract_digest: String,
    pub tokenizer_compatible: bool,
    pub template_compatible: bool,
    pub assistant_mask_coverage_bps: u32,
    pub compared_benchmark_refs: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub dataset_contract: GemmaE4bFinetuneDatasetContract,
    pub eval_pack_binding: GemmaE4bFinetuneEvalPackBinding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneDatasetStatus {
    pub dataset_id: String,
    pub project_id: String,
    pub dataset_ref: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub train_split: GemmaFinetuneSplitFileStatus,
    pub held_out_validation_split: GemmaFinetuneSplitFileStatus,
    pub final_report_split: GemmaFinetuneSplitFileStatus,
    pub baseline_short_split: GemmaFinetuneSplitFileStatus,
    pub validation_receipt: GemmaFinetuneValidationReceiptStatus,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaFinetuneTrainingPlanStatus {
    pub run_id: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub validator_policy_ref: String,
    pub target_set_id: String,
    pub batch_size: usize,
    pub max_steps: u64,
    pub step_duration_ms: u64,
    pub learning_rate: f32,
    pub weight_decay: f32,
    pub beta1: f32,
    pub beta2: f32,
    pub epsilon: f32,
    pub gradient_clip_norm: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneEventStatus {
    pub event_id: String,
    pub seq: u64,
    pub occurred_at_epoch_ms: u64,
    pub kind: GemmaFinetuneJobEventKind,
    pub phase: GemmaFinetuneJobPhase,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaFinetuneCheckpointStatus {
    pub checkpoint_id: String,
    pub checkpoint_digest: String,
    pub checkpoint_path: String,
    pub saved_at_epoch_ms: u64,
    pub completed_steps: u64,
    pub max_steps: u64,
    pub mean_loss: Option<f32>,
    pub eval_receipt: Option<GemmaE4bFinetuneEvalReceipt>,
    pub promotion_decision: Option<GemmaE4bCheckpointPromotionDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneArtifactStatus {
    pub artifact_id: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub artifact_path: String,
    pub adapter_artifact_digest: String,
    pub adapter_identity_digest: String,
    pub created_at_epoch_ms: u64,
    pub promoted_model_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetunePromotionStatus {
    pub checkpoint_id: String,
    pub vibe_packet: GemmaE4bPromotedCheckpointVibePacket,
    pub operator_review: GemmaE4bOperatorPromotionReview,
    pub decision: GemmaE4bCheckpointPromotionDecision,
    pub promoted_model_ref: Option<String>,
    pub promoted_at_epoch_ms: Option<u64>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetunePromotedModelStatus {
    pub model_ref: String,
    pub project_id: String,
    pub dataset_id: String,
    pub job_id: String,
    pub checkpoint_id: String,
    pub artifact_id: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub adapter_artifact_digest: String,
    pub benchmark_package_storage_key: String,
    pub promoted_at_epoch_ms: u64,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaFinetuneJobStatus {
    pub job_id: String,
    pub project_id: String,
    pub dataset_id: String,
    pub dataset_ref: String,
    pub tenant_id: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub started_at_epoch_ms: Option<u64>,
    pub finished_at_epoch_ms: Option<u64>,
    pub state: GemmaFinetuneJobState,
    pub phase: GemmaFinetuneJobPhase,
    pub cancel_requested: bool,
    pub plan: GemmaFinetuneTrainingPlanStatus,
    pub selected_candidate_id: Option<String>,
    pub baseline_sweep: Option<GemmaE4bBaselineSweepOutcome>,
    pub untuned_base_eval: Option<GemmaE4bFinetuneEvalReceipt>,
    pub checkpoint_candidate_eval: Option<GemmaE4bFinetuneEvalReceipt>,
    pub completed_steps: u64,
    pub final_train_mean_loss: Option<f32>,
    pub checkpoints: Vec<GemmaFinetuneCheckpointStatus>,
    pub artifacts: Vec<GemmaFinetuneArtifactStatus>,
    pub events: Vec<GemmaFinetuneEventStatus>,
    pub promotion: Option<GemmaFinetunePromotionStatus>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GemmaFinetuneStatus {
    pub available: bool,
    pub schema_version: u16,
    pub storage_path: Option<String>,
    pub project_count: usize,
    pub dataset_count: usize,
    pub validation_receipt_count: usize,
    pub job_count: usize,
    pub promoted_model_count: usize,
    pub projects: Vec<GemmaFinetuneProjectStatus>,
    pub datasets: Vec<GemmaFinetuneDatasetStatus>,
    pub jobs: Vec<GemmaFinetuneJobStatus>,
    pub promoted_models: Vec<GemmaFinetunePromotedModelStatus>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedGemmaFinetuneState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    last_action: Option<String>,
    last_error: Option<String>,
    #[serde(default)]
    projects: Vec<GemmaFinetuneProjectStatus>,
    #[serde(default)]
    datasets: Vec<GemmaFinetuneDatasetStatus>,
    #[serde(default)]
    jobs: Vec<GemmaFinetuneJobStatus>,
    #[serde(default)]
    promoted_models: Vec<GemmaFinetunePromotedModelStatus>,
}

struct GemmaFinetuneController {
    storage_path: PathBuf,
    state: PersistedGemmaFinetuneState,
}

#[derive(Clone)]
struct GemmaFinetuneJobRuntimeContext {
    project: GemmaFinetuneProjectStatus,
    dataset: GemmaFinetuneDatasetStatus,
    job: GemmaFinetuneJobStatus,
}

impl GemmaFinetuneController {
    fn load(storage_path: PathBuf) -> Self {
        let state = fs::read(storage_path.as_path())
            .ok()
            .and_then(|raw| serde_json::from_slice::<PersistedGemmaFinetuneState>(&raw).ok())
            .unwrap_or_else(|| PersistedGemmaFinetuneState {
                schema_version: GEMMA_FINETUNE_SCHEMA_VERSION,
                updated_at_epoch_ms: current_epoch_ms(),
                last_action: None,
                last_error: None,
                projects: Vec::new(),
                datasets: Vec::new(),
                jobs: Vec::new(),
                promoted_models: Vec::new(),
            });
        Self {
            storage_path,
            state,
        }
    }

    fn persist(&mut self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create Gemma finetune state dir: {error}"))?;
        }
        self.state.schema_version = GEMMA_FINETUNE_SCHEMA_VERSION;
        self.state.updated_at_epoch_ms = current_epoch_ms();
        let raw = serde_json::to_vec_pretty(&self.state)
            .map_err(|error| format!("Failed to encode Gemma finetune state: {error}"))?;
        fs::write(self.storage_path.as_path(), raw)
            .map_err(|error| format!("Failed to write Gemma finetune state: {error}"))
    }

    fn status(&self) -> GemmaFinetuneStatus {
        GemmaFinetuneStatus {
            available: true,
            schema_version: self.state.schema_version,
            storage_path: Some(self.storage_path.display().to_string()),
            project_count: self.state.projects.len(),
            dataset_count: self.state.datasets.len(),
            validation_receipt_count: self.state.datasets.len(),
            job_count: self.state.jobs.len(),
            promoted_model_count: self.state.promoted_models.len(),
            projects: self.state.projects.clone(),
            datasets: self.state.datasets.clone(),
            jobs: self.state.jobs.clone(),
            promoted_models: self.state.promoted_models.clone(),
            last_action: self.state.last_action.clone(),
            last_error: self.state.last_error.clone(),
        }
    }

    fn create_project(
        &mut self,
        request: &GemmaFinetuneProjectCreateRequest,
    ) -> Result<GemmaFinetuneProjectStatus, String> {
        if request.project_name.trim().is_empty() {
            return Err("Gemma finetune project name must be present".to_string());
        }
        if request.base_served_artifact_digest.trim().is_empty() {
            return Err(
                "Gemma finetune project must bind a non-empty served base artifact digest"
                    .to_string(),
            );
        }
        if request.hidden_size == 0 {
            return Err("Gemma finetune project hidden_size must be greater than zero".to_string());
        }
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()
            .map_err(|error| format!("Failed to load bounded Gemma contract: {error}"))?;
        let eval_pack = canonical_gemma_e4b_finetune_eval_pack_binding()
            .map_err(|error| format!("Failed to load Gemma eval-pack binding: {error}"))?;
        let tenant_id = normalized_tenant_id(request.tenant_id.as_deref());
        let now = current_epoch_ms();
        let project_name = request.project_name.trim().to_string();
        let project_id = format!("{}-{}", slugify(project_name.as_str()), now);
        if self
            .state
            .projects
            .iter()
            .any(|project| project.project_id == project_id)
        {
            return Err(format!(
                "Gemma finetune project `{project_id}` already exists"
            ));
        }
        let project = GemmaFinetuneProjectStatus {
            project_id: project_id.clone(),
            tenant_id,
            project_name,
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            base_served_artifact_digest: request.base_served_artifact_digest.clone(),
            hidden_size: request.hidden_size,
            active_dataset_id: None,
            lane_binding: GemmaFinetuneLaneBindingStatus {
                model_id: contract.model_id.clone(),
                model_family: contract.model_family.clone(),
                training_family_id: contract.training_family_id.clone(),
                checkpoint_family: contract.checkpoint_family.clone(),
                base_model_revision: contract.base_model_revision.clone(),
                adapter_family: contract.adapter_target.adapter_family.clone(),
                eval_pack_storage_key: eval_pack.benchmark_package_storage_key.clone(),
                tokenizer_contract_digest: contract.tokenizer_contract_digest.clone(),
            },
            last_action: Some("Created Gemma finetune project".to_string()),
            last_error: None,
        };
        self.state.projects.push(project.clone());
        self.sort_state();
        self.state.last_action = Some(format!(
            "Created Gemma finetune project {}",
            project.project_id
        ));
        self.state.last_error = None;
        self.persist()?;
        Ok(project)
    }

    fn register_dataset(
        &mut self,
        request: &GemmaFinetuneDatasetRegisterRequest,
    ) -> Result<GemmaFinetuneDatasetStatus, String> {
        let project_index = self
            .state
            .projects
            .iter()
            .position(|project| project.project_id == request.project_id)
            .ok_or_else(|| format!("Unknown Gemma finetune project `{}`", request.project_id))?;
        if request.dataset_ref.trim().is_empty() {
            return Err("Gemma finetune dataset_ref must be present".to_string());
        }
        let now = current_epoch_ms();
        let dataset_slug = slugify(request.dataset_ref.as_str());
        let dataset_id = format!("{dataset_slug}-{now}");
        let project = self
            .state
            .projects
            .get(project_index)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Gemma finetune project `{}` disappeared",
                    request.project_id
                )
            })?;
        let train_path = PathBuf::from(request.train_path.as_str());
        let validation_path = PathBuf::from(request.validation_path.as_str());
        let holdout_path = PathBuf::from(request.holdout_path.as_str());
        let baseline_short_path = request
            .baseline_short_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| validation_path.clone());
        let final_report_path = request
            .final_report_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| holdout_path.clone());
        let train_samples = load_hidden_state_samples(train_path.as_path())?;
        let validation_samples = load_hidden_state_samples(validation_path.as_path())?;
        let holdout_samples = load_hidden_state_samples(holdout_path.as_path())?;
        let baseline_short_samples = if baseline_short_path == validation_path {
            validation_samples.clone()
        } else {
            load_hidden_state_samples(baseline_short_path.as_path())?
        };
        let final_report_samples = if final_report_path == holdout_path {
            holdout_samples.clone()
        } else {
            load_hidden_state_samples(final_report_path.as_path())?
        };
        if train_samples.is_empty() {
            return Err("Gemma finetune train split must contain at least one sample".to_string());
        }
        if validation_samples.is_empty() {
            return Err(
                "Gemma finetune validation split must contain at least one sample".to_string(),
            );
        }
        if holdout_samples.is_empty() {
            return Err(
                "Gemma finetune holdout split must contain at least one sample".to_string(),
            );
        }
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()
            .map_err(|error| format!("Failed to load bounded Gemma contract: {error}"))?;
        let eval_pack_binding = canonical_gemma_e4b_finetune_eval_pack_binding()
            .map_err(|error| format!("Failed to load Gemma eval-pack binding: {error}"))?;
        let canonical_template_digest = contract
            .tokenizer
            .template_digest
            .clone()
            .unwrap_or_default();
        let chat_template_digest = request
            .chat_template_digest
            .clone()
            .unwrap_or_else(|| canonical_template_digest.clone());
        let compared_benchmark_refs = if request.compared_benchmark_refs.is_empty() {
            vec![String::from(GEMMA_FINETUNE_EVAL_BENCHMARK_REF)]
        } else {
            request.compared_benchmark_refs.clone()
        };
        let mut dataset_contract = GemmaE4bFinetuneDatasetContract {
            schema_version: String::from(GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION),
            dataset_ref: request.dataset_ref.clone(),
            train_split_ref: format!("split://openagents/gemma4/e4b/{dataset_id}/train"),
            held_out_validation_split_ref: format!(
                "split://openagents/gemma4/e4b/{dataset_id}/held_out_validation"
            ),
            final_report_split_ref: format!(
                "split://openagents/gemma4/e4b/{dataset_id}/final_report"
            ),
            baseline_short_split_ref: format!(
                "split://openagents/gemma4/e4b/{dataset_id}/baseline_short"
            ),
            chat_template_digest: chat_template_digest.clone(),
            assistant_mask_kind: GemmaE4bAssistantMaskKind::AssistantResponsesOnly,
            assistant_mask_coverage_bps: request.assistant_mask_coverage_bps,
            benchmark_overlap_check: GemmaE4bBenchmarkOverlapCheck {
                check_id: request
                    .overlap_check_id
                    .clone()
                    .unwrap_or_else(|| format!("{dataset_id}-overlap-check")),
                compared_benchmark_refs: compared_benchmark_refs.clone(),
                exact_overlap_refs: Vec::new(),
                near_duplicate_overlap_refs: Vec::new(),
                passed: true,
                detail: request
                    .overlap_detail
                    .clone()
                    .unwrap_or_else(|| "bounded dataset cleared overlap review".to_string()),
            },
            dataset_digest: String::new(),
        };
        dataset_contract.dataset_digest = dataset_contract.stable_digest();
        let mut warnings = Vec::new();
        if baseline_short_path == validation_path {
            warnings.push(
                "baseline_short path defaulted to the validation file for the bounded MVP lane"
                    .to_string(),
            );
        }
        if final_report_path == holdout_path {
            warnings.push(
                "final_report path defaulted to the holdout file for the bounded MVP lane"
                    .to_string(),
            );
        }
        let mut errors = Vec::new();
        if chat_template_digest != canonical_template_digest {
            errors.push(
                "dataset chat template digest drifted from the bounded Gemma prompt fixture"
                    .to_string(),
            );
        }
        if let Err(error) = dataset_contract.validate() {
            errors.push(error.to_string());
        }
        let validation_receipt = GemmaFinetuneValidationReceiptStatus {
            receipt_id: format!("{dataset_id}.validation"),
            validated_at_epoch_ms: now,
            status: if errors.is_empty() {
                "validated".to_string()
            } else {
                "rejected".to_string()
            },
            tokenizer_contract_digest: contract.tokenizer_contract_digest.clone(),
            tokenizer_compatible: true,
            template_compatible: chat_template_digest == canonical_template_digest,
            assistant_mask_coverage_bps: request.assistant_mask_coverage_bps,
            compared_benchmark_refs,
            warnings,
            errors: errors.clone(),
            dataset_contract,
            eval_pack_binding,
        };
        let dataset = GemmaFinetuneDatasetStatus {
            dataset_id: dataset_id.clone(),
            project_id: project.project_id.clone(),
            dataset_ref: request.dataset_ref.clone(),
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            train_split: split_status(
                "train",
                validation_receipt.dataset_contract.train_split_ref.as_str(),
                train_path.as_path(),
                train_samples.len(),
            )?,
            held_out_validation_split: split_status(
                "held_out_validation",
                validation_receipt
                    .dataset_contract
                    .held_out_validation_split_ref
                    .as_str(),
                validation_path.as_path(),
                validation_samples.len(),
            )?,
            final_report_split: split_status(
                "final_report",
                validation_receipt
                    .dataset_contract
                    .final_report_split_ref
                    .as_str(),
                final_report_path.as_path(),
                final_report_samples.len(),
            )?,
            baseline_short_split: split_status(
                "baseline_short",
                validation_receipt
                    .dataset_contract
                    .baseline_short_split_ref
                    .as_str(),
                baseline_short_path.as_path(),
                baseline_short_samples.len(),
            )?,
            validation_receipt,
            last_action: Some("Registered Gemma finetune dataset".to_string()),
            last_error: (!errors.is_empty()).then(|| errors.join(" | ")),
        };
        self.state.datasets.retain(|existing| {
            !(existing.project_id == dataset.project_id
                && existing.dataset_id == dataset.dataset_id)
        });
        self.state.datasets.push(dataset.clone());
        if let Some(project_mut) = self.state.projects.get_mut(project_index) {
            project_mut.active_dataset_id = Some(dataset_id.clone());
            project_mut.updated_at_epoch_ms = now;
            project_mut.last_action = Some(format!(
                "Bound Gemma dataset {} to project {}",
                dataset_id, project_mut.project_id
            ));
            project_mut.last_error = (!errors.is_empty()).then(|| errors.join(" | "));
        }
        self.sort_state();
        self.state.last_action = Some(format!(
            "Registered Gemma finetune dataset {} for project {}",
            dataset_id, project.project_id
        ));
        self.state.last_error = (!errors.is_empty()).then(|| errors.join(" | "));
        self.persist()?;
        Ok(dataset)
    }

    fn create_job(
        &mut self,
        request: &GemmaFinetuneJobCreateRequest,
    ) -> Result<GemmaFinetuneJobStatus, String> {
        let project = self
            .state
            .projects
            .iter()
            .find(|project| project.project_id == request.project_id)
            .cloned()
            .ok_or_else(|| format!("Unknown Gemma finetune project `{}`", request.project_id))?;
        let dataset = resolve_job_dataset(
            &self.state.datasets,
            &project,
            request.dataset_id.as_deref(),
        )?;
        if dataset.validation_receipt.status != "validated"
            || !dataset.validation_receipt.errors.is_empty()
        {
            return Err(format!(
                "Gemma finetune dataset `{}` is not admitted into the bounded lane",
                dataset.dataset_id
            ));
        }
        if self.state.jobs.iter().any(|job| {
            job.project_id == project.project_id
                && !job.state.is_terminal()
                && job.dataset_id == dataset.dataset_id
        }) {
            return Err(format!(
                "Gemma finetune dataset `{}` already has an active job",
                dataset.dataset_id
            ));
        }
        let now = current_epoch_ms();
        let job_id = format!("{}-{}", slugify(project.project_name.as_str()), now);
        let plan = default_training_plan(job_id.as_str(), &project)?;
        let mut job = GemmaFinetuneJobStatus {
            job_id: job_id.clone(),
            project_id: project.project_id.clone(),
            dataset_id: dataset.dataset_id.clone(),
            dataset_ref: dataset.dataset_ref.clone(),
            tenant_id: project.tenant_id.clone(),
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            started_at_epoch_ms: None,
            finished_at_epoch_ms: None,
            state: GemmaFinetuneJobState::Pending,
            phase: GemmaFinetuneJobPhase::Queued,
            cancel_requested: false,
            plan,
            selected_candidate_id: None,
            baseline_sweep: None,
            untuned_base_eval: None,
            checkpoint_candidate_eval: None,
            completed_steps: 0,
            final_train_mean_loss: None,
            checkpoints: Vec::new(),
            artifacts: Vec::new(),
            events: Vec::new(),
            promotion: None,
            last_action: Some("Queued Gemma finetune job".to_string()),
            last_error: None,
        };
        let phase = job.phase;
        append_job_event(
            &mut job,
            GemmaFinetuneJobEventKind::Created,
            phase,
            format!(
                "Queued bounded Gemma finetune job for dataset {}",
                dataset.dataset_id
            ),
            now,
        );
        self.state.jobs.push(job.clone());
        if let Some(project_mut) = self
            .state
            .projects
            .iter_mut()
            .find(|item| item.project_id == project.project_id)
        {
            project_mut.updated_at_epoch_ms = now;
            project_mut.last_action = Some(format!("Queued Gemma finetune job {}", job_id));
            project_mut.last_error = None;
        }
        self.sort_state();
        self.state.last_action = Some(format!("Queued Gemma finetune job {}", job_id));
        self.state.last_error = None;
        self.persist()?;
        Ok(job)
    }

    fn job(&self, job_id: &str) -> Result<GemmaFinetuneJobStatus, String> {
        self.state
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown Gemma finetune job `{job_id}`"))
    }

    fn cancel_job(&mut self, job_id: &str) -> Result<GemmaFinetuneJobStatus, String> {
        let now = current_epoch_ms();
        let result = self.mutate_job(job_id, |job| {
            if job.state.is_terminal() {
                return Ok(job.clone());
            }
            job.cancel_requested = true;
            job.updated_at_epoch_ms = now;
            job.last_action = Some("Cancellation requested".to_string());
            job.last_error = None;
            if job.state == GemmaFinetuneJobState::Pending {
                job.state = GemmaFinetuneJobState::Cancelled;
                job.phase = GemmaFinetuneJobPhase::Cancelled;
                job.finished_at_epoch_ms = Some(now);
                let phase = job.phase;
                append_job_event(
                    job,
                    GemmaFinetuneJobEventKind::Cancelled,
                    phase,
                    "Cancelled before the worker started".to_string(),
                    now,
                );
            } else {
                job.state = GemmaFinetuneJobState::CancelRequested;
                let phase = job.phase;
                append_job_event(
                    job,
                    GemmaFinetuneJobEventKind::CancelRequested,
                    phase,
                    "Worker will stop at the next bounded cancellation check".to_string(),
                    now,
                );
            }
            Ok(job.clone())
        })?;
        self.state.last_action = Some(format!("Updated Gemma finetune job {}", job_id));
        self.state.last_error = None;
        self.persist()?;
        Ok(result)
    }

    fn load_job_runtime_context(
        &self,
        job_id: &str,
    ) -> Result<GemmaFinetuneJobRuntimeContext, String> {
        let job = self.job(job_id)?;
        let project = self
            .state
            .projects
            .iter()
            .find(|project| project.project_id == job.project_id)
            .cloned()
            .ok_or_else(|| format!("Gemma finetune project `{}` disappeared", job.project_id))?;
        let dataset = self
            .state
            .datasets
            .iter()
            .find(|dataset| dataset.dataset_id == job.dataset_id)
            .cloned()
            .ok_or_else(|| format!("Gemma finetune dataset `{}` disappeared", job.dataset_id))?;
        Ok(GemmaFinetuneJobRuntimeContext {
            project,
            dataset,
            job,
        })
    }

    fn job_cancel_requested(&self, job_id: &str) -> Result<bool, String> {
        Ok(self.job(job_id)?.cancel_requested)
    }

    fn mark_job_started(&mut self, job_id: &str, detail: &str) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.state = GemmaFinetuneJobState::Running;
            job.phase = GemmaFinetuneJobPhase::BaselineSweep;
            job.started_at_epoch_ms = Some(now);
            job.updated_at_epoch_ms = now;
            job.last_action = Some(detail.to_string());
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::Started,
                phase,
                detail.to_string(),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Started Gemma finetune job {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn record_baseline_sweep(
        &mut self,
        job_id: &str,
        outcome: GemmaE4bBaselineSweepOutcome,
        selected_candidate_id: String,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.phase = GemmaFinetuneJobPhase::Training;
            job.updated_at_epoch_ms = now;
            job.selected_candidate_id = Some(selected_candidate_id.clone());
            job.baseline_sweep = Some(outcome.clone());
            job.last_action = Some("Completed bounded baseline sweep".to_string());
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::BaselineSweepCompleted,
                phase,
                format!(
                    "Recommended short-run candidate `{}` from {} candidate rows",
                    selected_candidate_id,
                    outcome.candidate_results.len()
                ),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Recorded Gemma finetune baseline sweep {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn record_eval_receipt(
        &mut self,
        job_id: &str,
        subject_label: &str,
        receipt: GemmaE4bFinetuneEvalReceipt,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            match receipt.subject_kind {
                GemmaE4bFinetuneEvalSubjectKind::UntunedBase => {
                    job.untuned_base_eval = Some(receipt.clone());
                }
                GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate => {
                    job.checkpoint_candidate_eval = Some(receipt.clone());
                }
            }
            job.updated_at_epoch_ms = now;
            job.last_action = Some(format!("Recorded {subject_label} eval receipt"));
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::EvalReceiptRecorded,
                phase,
                format!(
                    "Recorded {} receipt {} at {} bps",
                    subject_label, receipt.receipt_id, receipt.held_out_score_bps
                ),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Recorded Gemma finetune eval receipt {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn record_training_step(
        &mut self,
        job_id: &str,
        completed_steps: u64,
        mean_loss: Option<f32>,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.completed_steps = completed_steps;
            job.final_train_mean_loss = mean_loss;
            job.updated_at_epoch_ms = now;
            job.last_action = Some(format!("Completed Gemma train step {}", completed_steps));
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::StepCompleted,
                phase,
                format!(
                    "Completed step {}/{} with mean_loss={}",
                    completed_steps,
                    job.plan.max_steps,
                    mean_loss
                        .map(|value| format!("{value:.4}"))
                        .unwrap_or_else(|| "-".to_string())
                ),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Recorded Gemma finetune step {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn record_checkpoint(
        &mut self,
        job_id: &str,
        checkpoint: GemmaFinetuneCheckpointStatus,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.updated_at_epoch_ms = now;
            job.last_action = Some(format!("Saved checkpoint {}", checkpoint.checkpoint_id));
            job.last_error = None;
            job.checkpoints
                .retain(|item| item.checkpoint_id != checkpoint.checkpoint_id);
            job.checkpoints.push(checkpoint.clone());
            job.checkpoints
                .sort_by(|left, right| right.saved_at_epoch_ms.cmp(&left.saved_at_epoch_ms));
            if job.checkpoints.len() > GEMMA_FINETUNE_CHECKPOINT_LIMIT {
                job.checkpoints.truncate(GEMMA_FINETUNE_CHECKPOINT_LIMIT);
            }
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::CheckpointWritten,
                phase,
                format!(
                    "Saved checkpoint {} at step {}",
                    checkpoint.checkpoint_id, checkpoint.completed_steps
                ),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Saved Gemma finetune checkpoint {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn record_artifact(
        &mut self,
        job_id: &str,
        artifact: GemmaFinetuneArtifactStatus,
    ) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.updated_at_epoch_ms = now;
            job.last_action = Some(format!("Exported artifact {}", artifact.artifact_id));
            job.last_error = None;
            job.artifacts
                .retain(|item| item.artifact_id != artifact.artifact_id);
            job.artifacts.push(artifact.clone());
            job.artifacts
                .sort_by(|left, right| right.created_at_epoch_ms.cmp(&left.created_at_epoch_ms));
            if job.artifacts.len() > GEMMA_FINETUNE_ARTIFACT_LIMIT {
                job.artifacts.truncate(GEMMA_FINETUNE_ARTIFACT_LIMIT);
            }
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::ArtifactExported,
                phase,
                format!(
                    "Exported adapter artifact {} ({})",
                    artifact.artifact_id, artifact.adapter_artifact_digest
                ),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Exported Gemma finetune artifact {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn finalize_job_completed(&mut self, job_id: &str, detail: &str) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.state = GemmaFinetuneJobState::Completed;
            job.phase = GemmaFinetuneJobPhase::AwaitingPromotion;
            job.finished_at_epoch_ms = Some(now);
            job.updated_at_epoch_ms = now;
            job.last_action = Some(detail.to_string());
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::Completed,
                phase,
                detail.to_string(),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Completed Gemma finetune job {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn finalize_job_cancelled(&mut self, job_id: &str, detail: &str) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.state = GemmaFinetuneJobState::Cancelled;
            job.phase = GemmaFinetuneJobPhase::Cancelled;
            job.finished_at_epoch_ms = Some(now);
            job.updated_at_epoch_ms = now;
            job.last_action = Some(detail.to_string());
            job.last_error = None;
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::Cancelled,
                phase,
                detail.to_string(),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Cancelled Gemma finetune job {}", job_id));
        self.state.last_error = None;
        self.persist()
    }

    fn mark_job_failed(&mut self, job_id: &str, error: &str) -> Result<(), String> {
        let now = current_epoch_ms();
        self.mutate_job(job_id, |job| {
            job.state = GemmaFinetuneJobState::Failed;
            job.phase = GemmaFinetuneJobPhase::Failed;
            job.finished_at_epoch_ms = Some(now);
            job.updated_at_epoch_ms = now;
            job.last_action = Some("Gemma finetune job failed".to_string());
            job.last_error = Some(error.to_string());
            let phase = job.phase;
            append_job_event(
                job,
                GemmaFinetuneJobEventKind::Failed,
                phase,
                error.to_string(),
                now,
            );
            Ok(())
        })?;
        self.state.last_action = Some(format!("Gemma finetune job {} failed", job_id));
        self.state.last_error = Some(error.to_string());
        self.persist()
    }

    fn promote_checkpoint(
        &mut self,
        request: &GemmaFinetuneCheckpointPromotionRequest,
    ) -> Result<GemmaFinetunePromotionStatus, String> {
        if request.reviewer_id.trim().is_empty() {
            return Err("Gemma finetune promotion requires a reviewer_id".to_string());
        }
        let job_index = self
            .state
            .jobs
            .iter()
            .position(|job| job.job_id == request.job_id)
            .ok_or_else(|| format!("Unknown Gemma finetune job `{}`", request.job_id))?;
        let job_snapshot = self
            .state
            .jobs
            .get(job_index)
            .cloned()
            .ok_or_else(|| format!("Gemma finetune job `{}` disappeared", request.job_id))?;
        if job_snapshot.state != GemmaFinetuneJobState::Completed {
            return Err(format!(
                "Gemma finetune job `{}` is not ready for promotion",
                request.job_id
            ));
        }
        let checkpoint_status = request
            .checkpoint_id
            .as_deref()
            .and_then(|checkpoint_id| {
                job_snapshot
                    .checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.checkpoint_id == checkpoint_id)
            })
            .or_else(|| job_snapshot.checkpoints.first())
            .cloned()
            .ok_or_else(|| format!("Gemma finetune job `{}` has no checkpoints", request.job_id))?;
        let checkpoint = read_checkpoint(checkpoint_status.checkpoint_path.as_str())?;
        let dataset = self
            .state
            .datasets
            .iter()
            .find(|dataset| dataset.dataset_id == job_snapshot.dataset_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Gemma finetune dataset `{}` disappeared",
                    job_snapshot.dataset_id
                )
            })?;
        let untuned_base_eval = job_snapshot.untuned_base_eval.clone().ok_or_else(|| {
            format!(
                "Gemma finetune job `{}` is missing untuned base eval",
                request.job_id
            )
        })?;
        let candidate_eval = checkpoint_status
            .eval_receipt
            .clone()
            .or_else(|| job_snapshot.checkpoint_candidate_eval.clone())
            .ok_or_else(|| {
                format!(
                    "Gemma finetune job `{}` is missing checkpoint eval",
                    request.job_id
                )
            })?;
        let vibe_packet = canonical_gemma_e4b_promoted_checkpoint_vibe_packet(&checkpoint)
            .map_err(|error| format!("Failed to build promoted-checkpoint vibe packet: {error}"))?;
        let review_state = parse_review_state(request.review_state.as_deref())?;
        let failed_case_ids = request
            .failed_case_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let case_verdicts = vibe_packet
            .cases
            .iter()
            .map(|case| GemmaE4bOperatorReviewCaseVerdict {
                case_id: case.case_id.clone(),
                status: if failed_case_ids.iter().any(|failed| failed == &case.case_id) {
                    GemmaE4bReviewVerdictStatus::Failed
                } else {
                    GemmaE4bReviewVerdictStatus::Passed
                },
                detail: format!("reviewed {}", case.case_id),
            })
            .collect::<Vec<_>>();
        let mut operator_review = GemmaE4bOperatorPromotionReview {
            schema_version: String::from(GEMMA_E4B_OPERATOR_REVIEW_SCHEMA_VERSION),
            review_id: format!("{}.promotion-review", checkpoint.checkpoint_id),
            packet_id: vibe_packet.packet_id.clone(),
            reviewer_id: request.reviewer_id.trim().to_string(),
            state: review_state,
            case_verdicts,
            summary: request.summary.clone().unwrap_or_else(|| {
                "Bounded OpenAgents operator review over the Gemma finetune checkpoint".to_string()
            }),
            review_digest: String::new(),
        };
        operator_review.review_digest = operator_review.stable_digest();
        let decision = decide_gemma_e4b_checkpoint_promotion(
            &checkpoint,
            &dataset.validation_receipt.dataset_contract,
            &dataset.validation_receipt.eval_pack_binding,
            &untuned_base_eval,
            &candidate_eval,
            &vibe_packet,
            &operator_review,
        )
        .map_err(|error| format!("Failed to score Gemma promotion decision: {error}"))?;
        let now = current_epoch_ms();
        let mut promoted_model_ref = None;
        let mut promoted_at_epoch_ms = None;
        if decision.decision_state == GemmaE4bPromotionDecisionState::Promote {
            let artifact = job_snapshot.artifacts.first().cloned().ok_or_else(|| {
                format!(
                    "Gemma finetune job `{}` has no exported artifact",
                    request.job_id
                )
            })?;
            let model_ref = format!(
                "model://openagents/gemma4/e4b/{}/{}",
                job_snapshot.project_id, artifact.adapter_revision
            );
            let promoted_model = GemmaFinetunePromotedModelStatus {
                model_ref: model_ref.clone(),
                project_id: job_snapshot.project_id.clone(),
                dataset_id: job_snapshot.dataset_id.clone(),
                job_id: job_snapshot.job_id.clone(),
                checkpoint_id: checkpoint_status.checkpoint_id.clone(),
                artifact_id: artifact.artifact_id.clone(),
                adapter_id: artifact.adapter_id.clone(),
                adapter_revision: artifact.adapter_revision.clone(),
                adapter_artifact_digest: artifact.adapter_artifact_digest.clone(),
                benchmark_package_storage_key: decision.benchmark_package_storage_key.clone(),
                promoted_at_epoch_ms: now,
                last_action: Some("Promoted Gemma finetune checkpoint".to_string()),
                last_error: None,
            };
            self.state
                .promoted_models
                .retain(|item| item.model_ref != promoted_model.model_ref);
            self.state.promoted_models.push(promoted_model);
            self.state
                .promoted_models
                .sort_by(|left, right| right.promoted_at_epoch_ms.cmp(&left.promoted_at_epoch_ms));
            promoted_model_ref = Some(model_ref);
            promoted_at_epoch_ms = Some(now);
        }
        let promotion = GemmaFinetunePromotionStatus {
            checkpoint_id: checkpoint_status.checkpoint_id.clone(),
            vibe_packet,
            operator_review,
            decision: decision.clone(),
            promoted_model_ref: promoted_model_ref.clone(),
            promoted_at_epoch_ms,
            last_action: Some("Scored Gemma checkpoint promotion".to_string()),
            last_error: None,
        };
        {
            let job =
                self.state.jobs.get_mut(job_index).ok_or_else(|| {
                    format!("Gemma finetune job `{}` disappeared", request.job_id)
                })?;
            job.updated_at_epoch_ms = now;
            job.promotion = Some(promotion.clone());
            job.last_action = Some(match decision.decision_state {
                GemmaE4bPromotionDecisionState::Promote => {
                    "Promoted Gemma finetune checkpoint".to_string()
                }
                GemmaE4bPromotionDecisionState::HoldForReview => {
                    "Held Gemma finetune checkpoint for review".to_string()
                }
                GemmaE4bPromotionDecisionState::Reject => {
                    "Rejected Gemma finetune checkpoint".to_string()
                }
            });
            job.last_error = None;
            if decision.decision_state == GemmaE4bPromotionDecisionState::Promote {
                job.phase = GemmaFinetuneJobPhase::Promoted;
            }
            if let Some(checkpoint_mut) = job
                .checkpoints
                .iter_mut()
                .find(|item| item.checkpoint_id == checkpoint_status.checkpoint_id)
            {
                checkpoint_mut.promotion_decision = Some(decision.clone());
            }
            if let Some(model_ref) = promoted_model_ref.clone() {
                if let Some(artifact_mut) = job.artifacts.first_mut() {
                    artifact_mut.promoted_model_ref = Some(model_ref);
                }
                let phase = job.phase;
                append_job_event(
                    job,
                    GemmaFinetuneJobEventKind::Promoted,
                    phase,
                    format!(
                        "Promoted checkpoint {} to {}",
                        checkpoint_status.checkpoint_id,
                        promoted_model_ref.as_deref().unwrap_or("-")
                    ),
                    now,
                );
            } else {
                let phase = job.phase;
                append_job_event(
                    job,
                    GemmaFinetuneJobEventKind::PromotionRecorded,
                    phase,
                    format!(
                        "Scored checkpoint {} as {}",
                        checkpoint_status.checkpoint_id,
                        promotion_decision_state_label(&promotion.decision)
                    ),
                    now,
                );
            }
        }
        self.sort_state();
        self.state.last_action = Some(format!(
            "Scored Gemma finetune promotion {}",
            request.job_id
        ));
        self.state.last_error = None;
        self.persist()?;
        Ok(promotion)
    }

    fn mutate_job<T>(
        &mut self,
        job_id: &str,
        f: impl FnOnce(&mut GemmaFinetuneJobStatus) -> Result<T, String>,
    ) -> Result<T, String> {
        let index = self
            .state
            .jobs
            .iter()
            .position(|job| job.job_id == job_id)
            .ok_or_else(|| format!("Unknown Gemma finetune job `{job_id}`"))?;
        let result = {
            let job = self
                .state
                .jobs
                .get_mut(index)
                .ok_or_else(|| format!("Unknown Gemma finetune job `{job_id}`"))?;
            f(job)?
        };
        self.sort_state();
        Ok(result)
    }

    fn sort_state(&mut self) {
        self.state
            .projects
            .sort_by(|left, right| right.updated_at_epoch_ms.cmp(&left.updated_at_epoch_ms));
        self.state
            .datasets
            .sort_by(|left, right| right.updated_at_epoch_ms.cmp(&left.updated_at_epoch_ms));
        self.state
            .jobs
            .sort_by(|left, right| right.updated_at_epoch_ms.cmp(&left.updated_at_epoch_ms));
    }
}

pub(crate) fn status() -> Result<GemmaFinetuneStatus, String> {
    with_controller(|controller| Ok(controller.status()))
}

pub(crate) fn create_project(
    request: GemmaFinetuneProjectCreateRequest,
) -> Result<GemmaFinetuneProjectStatus, String> {
    with_controller(|controller| controller.create_project(&request))
}

pub(crate) fn register_dataset(
    request: GemmaFinetuneDatasetRegisterRequest,
) -> Result<GemmaFinetuneDatasetStatus, String> {
    with_controller(|controller| controller.register_dataset(&request))
}

pub(crate) fn create_job(
    request: GemmaFinetuneJobCreateRequest,
) -> Result<GemmaFinetuneJobStatus, String> {
    let job = with_controller(|controller| controller.create_job(&request))?;
    let job_id = job.job_id.clone();
    spawn_job_worker(job_id.as_str())?;
    Ok(job)
}

pub(crate) fn get_job(job_id: &str) -> Result<GemmaFinetuneJobStatus, String> {
    with_controller(|controller| controller.job(job_id))
}

pub(crate) fn cancel_job(job_id: &str) -> Result<GemmaFinetuneJobStatus, String> {
    with_controller(|controller| controller.cancel_job(job_id))
}

pub(crate) fn promote_checkpoint(
    request: GemmaFinetuneCheckpointPromotionRequest,
) -> Result<GemmaFinetunePromotionStatus, String> {
    with_controller(|controller| controller.promote_checkpoint(&request))
}

fn spawn_job_worker(job_id: &str) -> Result<(), String> {
    let worker_job_id = job_id.to_string();
    std::thread::Builder::new()
        .name(format!("gemma-finetune-{worker_job_id}"))
        .spawn(move || {
            if let Err(error) = run_job_worker(worker_job_id.as_str()) {
                let _ = with_controller(|controller| {
                    controller.mark_job_failed(worker_job_id.as_str(), error.as_str())
                });
            }
        })
        .map(|_| ())
        .map_err(|error| {
            let _ = with_controller(|controller| {
                controller.mark_job_failed(
                    job_id,
                    format!("Failed to start Gemma job worker: {error}").as_str(),
                )
            });
            format!("Failed to start Gemma finetune worker: {error}")
        })
}

fn run_job_worker(job_id: &str) -> Result<(), String> {
    let context = with_controller(|controller| controller.load_job_runtime_context(job_id))?;
    if with_controller(|controller| controller.job_cancel_requested(job_id))? {
        with_controller(|controller| {
            controller.finalize_job_cancelled(job_id, "Cancelled before bounded execution started")
        })?;
        return Ok(());
    }
    with_controller(|controller| {
        controller.mark_job_started(
            job_id,
            "Started bounded Gemma finetune worker and entered the baseline sweep",
        )
    })?;

    let train_samples =
        load_hidden_state_samples(Path::new(context.dataset.train_split.file_path.as_str()))?;
    let held_out_samples = load_hidden_state_samples(Path::new(
        context.dataset.held_out_validation_split.file_path.as_str(),
    ))?;
    let project = context.project;
    let dataset = context.dataset;
    let plan = context.job.plan;
    let target_set = canonical_gemma_e4b_cuda_adapter_target_set();
    let base_binding = project_base_binding(&project)?;
    let baseline_request = baseline_sweep_request(job_id, &dataset.validation_receipt, &plan)?;
    let baseline_outcome = run_gemma_e4b_baseline_sweep(
        base_binding.clone(),
        target_set.clone(),
        train_samples.clone(),
        held_out_samples.clone(),
        &baseline_request,
    )
    .map_err(|error| format!("Gemma baseline sweep failed: {error}"))?;
    let selected_candidate_id = baseline_outcome.recommended_candidate_id.clone();
    with_controller(|controller| {
        controller.record_baseline_sweep(
            job_id,
            baseline_outcome.clone(),
            selected_candidate_id.clone(),
        )
    })?;
    if with_controller(|controller| controller.job_cancel_requested(job_id))? {
        with_controller(|controller| {
            controller.finalize_job_cancelled(job_id, "Cancelled after the baseline sweep")
        })?;
        return Ok(());
    }

    let untuned_base_loss = held_out_loss_for_untuned_base(
        job_id,
        &plan,
        &base_binding,
        &target_set,
        held_out_samples.clone(),
    )?;
    let untuned_base_eval = build_proxy_eval_receipt(
        GemmaE4bFinetuneEvalSubjectKind::UntunedBase,
        project.lane_binding.model_id.as_str(),
        project.base_served_artifact_digest.as_str(),
        &dataset.validation_receipt,
        untuned_base_loss,
        "Proxy held-out receipt derived from one untuned-base loss pass plus bounded contract checks because the public Gemma scorer is not exported into OpenAgents.",
    );
    with_controller(|controller| {
        controller.record_eval_receipt(job_id, "untuned_base", untuned_base_eval.clone())
    })?;

    let trainer = GemmaE4bCudaAdapterSftTrainer::new(
        training_config_from_plan(job_id, &plan)?,
        target_set.clone(),
        base_binding.clone(),
        train_samples.clone(),
    )
    .map_err(|error| format!("Failed to initialize Gemma trainer: {error}"))?;
    let mut run = trainer
        .initialize_run()
        .map_err(|error| format!("Failed to initialize Gemma run: {error}"))?;
    for _ in 0..plan.max_steps {
        if with_controller(|controller| controller.job_cancel_requested(job_id))? {
            with_controller(|controller| {
                controller.finalize_job_cancelled(
                    job_id,
                    "Cancelled while bounded Gemma training was in progress",
                )
            })?;
            return Ok(());
        }
        let progress = trainer
            .advance_run(&mut run, Some(1), current_epoch_ms(), plan.step_duration_ms)
            .map_err(|error| format!("Failed to advance Gemma run: {error}"))?;
        let completed_steps = run.summary().completed_steps;
        let mean_loss = progress
            .gradient_records
            .last()
            .map(|record| record.mean_loss);
        with_controller(|controller| {
            controller.record_training_step(job_id, completed_steps, mean_loss)
        })?;
        if completed_steps % 2 == 0 || completed_steps == plan.max_steps {
            let checkpoint_id = if completed_steps == plan.max_steps {
                format!("{}-{}-final", plan.adapter_id, plan.adapter_revision)
            } else {
                format!(
                    "{}-{}-step-{}",
                    plan.adapter_id, plan.adapter_revision, completed_steps
                )
            };
            let checkpoint = trainer
                .save_checkpoint(checkpoint_id.clone(), &run, current_epoch_ms())
                .map_err(|error| format!("Failed to save checkpoint `{checkpoint_id}`: {error}"))?;
            let checkpoint_path = write_checkpoint(job_id, &checkpoint)?;
            with_controller(|controller| {
                controller.record_checkpoint(
                    job_id,
                    GemmaFinetuneCheckpointStatus {
                        checkpoint_id: checkpoint.checkpoint_id.clone(),
                        checkpoint_digest: checkpoint.checkpoint_digest.clone(),
                        checkpoint_path,
                        saved_at_epoch_ms: checkpoint.saved_at_ms,
                        completed_steps,
                        max_steps: plan.max_steps,
                        mean_loss,
                        eval_receipt: None,
                        promotion_decision: None,
                    },
                )
            })?;
        }
    }

    let export_request = GemmaE4bCudaAdapterExportRequest {
        dataset_ref: dataset.dataset_ref.clone(),
        validator_policy_ref: plan.validator_policy_ref.clone(),
        adapter_id: plan.adapter_id.clone(),
        adapter_revision: plan.adapter_revision.clone(),
    };
    let exported_artifact = trainer
        .export_run_artifact(&run, &export_request)
        .map_err(|error| format!("Failed to export Gemma artifact: {error}"))?;
    let artifact_path = write_artifact(job_id, &plan, &exported_artifact)?;
    let artifact_status = GemmaFinetuneArtifactStatus {
        artifact_id: format!("{}-{}", plan.adapter_id, plan.adapter_revision),
        adapter_id: plan.adapter_id.clone(),
        adapter_revision: plan.adapter_revision.clone(),
        artifact_path,
        adapter_artifact_digest: exported_artifact.adapter_artifact_digest.clone(),
        adapter_identity_digest: exported_artifact.adapter_identity_digest.clone(),
        created_at_epoch_ms: current_epoch_ms(),
        promoted_model_ref: None,
    };
    with_controller(|controller| controller.record_artifact(job_id, artifact_status.clone()))?;

    let candidate_loss = held_out_loss_for_candidate(
        job_id,
        &plan,
        &base_binding,
        &target_set,
        held_out_samples,
        &exported_artifact,
    )?;
    let candidate_checkpoint = with_controller(|controller| controller.job(job_id))?
        .checkpoints
        .first()
        .cloned()
        .ok_or_else(|| {
            format!("Gemma finetune job `{job_id}` did not retain a final checkpoint")
        })?;
    let candidate_eval = build_proxy_eval_receipt(
        GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate,
        candidate_checkpoint.checkpoint_id.as_str(),
        candidate_checkpoint.checkpoint_digest.as_str(),
        &dataset.validation_receipt,
        candidate_loss,
        "Proxy held-out receipt derived from the bounded adapter lane plus contract checks because the public Gemma scorer is not exported into OpenAgents.",
    );
    with_controller(|controller| {
        controller.record_eval_receipt(job_id, "checkpoint_candidate", candidate_eval.clone())
    })?;
    with_controller(|controller| {
        controller.record_checkpoint(
            job_id,
            GemmaFinetuneCheckpointStatus {
                checkpoint_id: candidate_checkpoint.checkpoint_id.clone(),
                checkpoint_digest: candidate_checkpoint.checkpoint_digest.clone(),
                checkpoint_path: candidate_checkpoint.checkpoint_path.clone(),
                saved_at_epoch_ms: candidate_checkpoint.saved_at_epoch_ms,
                completed_steps: candidate_checkpoint.completed_steps,
                max_steps: candidate_checkpoint.max_steps,
                mean_loss: candidate_checkpoint.mean_loss,
                eval_receipt: Some(candidate_eval),
                promotion_decision: candidate_checkpoint.promotion_decision.clone(),
            },
        )
    })?;
    with_controller(|controller| {
        controller.finalize_job_completed(
            job_id,
            "Completed bounded Gemma finetune job and left the checkpoint awaiting promotion",
        )
    })?;
    Ok(())
}

fn with_controller<T>(
    f: impl FnOnce(&mut GemmaFinetuneController) -> Result<T, String>,
) -> Result<T, String> {
    let controller = GEMMA_FINETUNE_CONTROLLER
        .get_or_init(|| Mutex::new(GemmaFinetuneController::load(gemma_finetune_state_path())));
    let mut controller = controller
        .lock()
        .map_err(|_| "Gemma finetune controller lock poisoned".to_string())?;
    f(&mut controller)
}

fn gemma_finetune_state_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(GEMMA_FINETUNE_STATE_FILENAME)
}

fn gemma_finetune_root_dir() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(GEMMA_FINETUNE_ROOT_DIR)
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn normalized_tenant_id(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_TENANT_ID)
        .to_string()
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn split_status(
    split_id: &str,
    split_ref: &str,
    path: &Path,
    sample_count: usize,
) -> Result<GemmaFinetuneSplitFileStatus, String> {
    Ok(GemmaFinetuneSplitFileStatus {
        split_id: split_id.to_string(),
        split_ref: split_ref.to_string(),
        file_path: path.display().to_string(),
        file_digest: file_digest(path)?,
        sample_count,
    })
}

fn file_digest(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read dataset file {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn load_hidden_state_samples(path: &Path) -> Result<Vec<GemmaE4bLmHeadSupervisionSample>, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "Failed to read Gemma dataset file {}: {error}",
            path.display()
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<GemmaE4bLmHeadSupervisionSample>>(trimmed).map_err(
            |error| {
                format!(
                    "Failed to decode Gemma dataset JSON array {}: {error}",
                    path.display()
                )
            },
        );
    }
    trimmed
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<GemmaE4bLmHeadSupervisionSample>(line).map_err(|error| {
                format!(
                    "Failed to decode Gemma dataset JSONL row {} in {}: {error}",
                    index + 1,
                    path.display()
                )
            })
        })
        .collect()
}

fn resolve_job_dataset(
    datasets: &[GemmaFinetuneDatasetStatus],
    project: &GemmaFinetuneProjectStatus,
    dataset_id: Option<&str>,
) -> Result<GemmaFinetuneDatasetStatus, String> {
    if let Some(dataset_id) = dataset_id {
        return datasets
            .iter()
            .find(|dataset| {
                dataset.dataset_id == dataset_id && dataset.project_id == project.project_id
            })
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Unknown Gemma finetune dataset `{dataset_id}` for project `{}`",
                    project.project_id
                )
            });
    }
    let active_dataset_id = project.active_dataset_id.as_deref().ok_or_else(|| {
        format!(
            "Gemma finetune project `{}` has no active dataset",
            project.project_id
        )
    })?;
    datasets
        .iter()
        .find(|dataset| dataset.dataset_id == active_dataset_id)
        .cloned()
        .ok_or_else(|| format!("Gemma finetune dataset `{active_dataset_id}` disappeared"))
}

fn default_training_plan(
    job_id: &str,
    project: &GemmaFinetuneProjectStatus,
) -> Result<GemmaFinetuneTrainingPlanStatus, String> {
    let target_set = canonical_gemma_e4b_cuda_adapter_target_set();
    let adapter_slug = slugify(project.project_name.as_str());
    let adapter_revision = format!("r{}", current_epoch_ms());
    Ok(GemmaFinetuneTrainingPlanStatus {
        run_id: format!("gemma-finetune-{job_id}"),
        adapter_id: adapter_slug,
        adapter_revision,
        validator_policy_ref: GEMMA_FINETUNE_VALIDATOR_POLICY_REF.to_string(),
        target_set_id: target_set.target_set_id,
        batch_size: GEMMA_FINETUNE_DEFAULT_BATCH_SIZE,
        max_steps: GEMMA_FINETUNE_DEFAULT_MAX_STEPS,
        step_duration_ms: GEMMA_FINETUNE_DEFAULT_STEP_DURATION_MS,
        learning_rate: GEMMA_FINETUNE_DEFAULT_LEARNING_RATE,
        weight_decay: GEMMA_FINETUNE_DEFAULT_WEIGHT_DECAY,
        beta1: GEMMA_FINETUNE_DEFAULT_BETA1,
        beta2: GEMMA_FINETUNE_DEFAULT_BETA2,
        epsilon: GEMMA_FINETUNE_DEFAULT_EPSILON,
        gradient_clip_norm: Some(GEMMA_FINETUNE_DEFAULT_GRADIENT_CLIP_NORM),
    })
}

fn training_config_from_plan(
    job_id: &str,
    plan: &GemmaFinetuneTrainingPlanStatus,
) -> Result<GemmaE4bCudaAdapterSftConfig, String> {
    Ok(GemmaE4bCudaAdapterSftConfig {
        run_id: format!("{}-train", job_id),
        budget: TrainingLoopBudget::new(plan.max_steps, 1, 1)
            .map_err(|error| format!("Failed to build Gemma training budget: {error}"))?,
        batch_size: plan.batch_size,
        optimizer: TrainingOptimizerConfig::adamw(
            plan.learning_rate,
            plan.beta1,
            plan.beta2,
            plan.epsilon,
        )
        .with_weight_decay(plan.weight_decay)
        .with_gradient_clip_norm(plan.gradient_clip_norm.unwrap_or(1.0)),
        optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
    })
}

fn baseline_sweep_request(
    job_id: &str,
    receipt: &GemmaFinetuneValidationReceiptStatus,
    plan: &GemmaFinetuneTrainingPlanStatus,
) -> Result<GemmaE4bBaselineSweepRequest, String> {
    Ok(GemmaE4bBaselineSweepRequest {
        schema_version: String::from(GEMMA_E4B_BASELINE_SWEEP_REQUEST_SCHEMA_VERSION),
        dataset_contract: receipt.dataset_contract.clone(),
        eval_pack_binding: receipt.eval_pack_binding.clone(),
        validator_policy_ref: plan.validator_policy_ref.clone(),
        adapter_id_prefix: format!("{}-baseline", plan.adapter_id),
        adapter_revision_prefix: format!("{}-cand", plan.adapter_revision),
        candidates: vec![
            GemmaE4bBaselineCandidate {
                candidate_id: String::from("conservative"),
                config: GemmaE4bCudaAdapterSftConfig {
                    run_id: format!("{job_id}-baseline-conservative"),
                    budget: TrainingLoopBudget::new(plan.max_steps, 1, 1)
                        .map_err(|error| format!("Failed to build baseline budget: {error}"))?,
                    batch_size: plan.batch_size,
                    optimizer: TrainingOptimizerConfig::adamw(0.02, 0.9, 0.99, plan.epsilon)
                        .with_weight_decay(plan.weight_decay)
                        .with_gradient_clip_norm(plan.gradient_clip_norm.unwrap_or(1.0)),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
                max_steps: 2,
                detail: String::from("Lower-learning-rate short sweep"),
            },
            GemmaE4bBaselineCandidate {
                candidate_id: String::from("balanced"),
                config: GemmaE4bCudaAdapterSftConfig {
                    run_id: format!("{job_id}-baseline-balanced"),
                    budget: TrainingLoopBudget::new(plan.max_steps, 1, 1)
                        .map_err(|error| format!("Failed to build baseline budget: {error}"))?,
                    batch_size: plan.batch_size,
                    optimizer: TrainingOptimizerConfig::adamw(
                        plan.learning_rate,
                        plan.beta1,
                        plan.beta2,
                        plan.epsilon,
                    )
                    .with_weight_decay(plan.weight_decay)
                    .with_gradient_clip_norm(plan.gradient_clip_norm.unwrap_or(1.0)),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
                max_steps: 2,
                detail: String::from("Default bounded short sweep"),
            },
        ],
        started_at_ms: current_epoch_ms(),
        step_duration_ms: plan.step_duration_ms,
    })
}

fn project_base_binding(
    project: &GemmaFinetuneProjectStatus,
) -> Result<GemmaE4bServedBaseModelBinding, String> {
    let contract = canonical_gemma_e4b_finetuning_mvp_contract()
        .map_err(|error| format!("Failed to load bounded Gemma contract: {error}"))?;
    Ok(GemmaE4bServedBaseModelBinding {
        model_id: contract.model_id,
        base_model_revision: contract.base_model_revision,
        base_served_artifact_digest: project.base_served_artifact_digest.clone(),
        tokenizer: contract.tokenizer,
        hidden_size: project.hidden_size,
    })
}

fn held_out_loss_for_untuned_base(
    job_id: &str,
    plan: &GemmaFinetuneTrainingPlanStatus,
    base_binding: &GemmaE4bServedBaseModelBinding,
    target_set: &GemmaE4bCudaAdapterTargetSet,
    held_out_samples: Vec<GemmaE4bLmHeadSupervisionSample>,
) -> Result<f32, String> {
    let trainer = GemmaE4bCudaAdapterSftTrainer::new(
        GemmaE4bCudaAdapterSftConfig {
            run_id: format!("{job_id}-untuned-base-eval"),
            budget: TrainingLoopBudget::new(1, 1, 1)
                .map_err(|error| format!("Failed to build untuned-base eval budget: {error}"))?,
            batch_size: plan.batch_size,
            optimizer: TrainingOptimizerConfig::adamw(
                plan.learning_rate,
                plan.beta1,
                plan.beta2,
                plan.epsilon,
            )
            .with_weight_decay(plan.weight_decay)
            .with_gradient_clip_norm(plan.gradient_clip_norm.unwrap_or(1.0)),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
        },
        target_set.clone(),
        base_binding.clone(),
        held_out_samples,
    )
    .map_err(|error| format!("Failed to initialize untuned-base eval lane: {error}"))?;
    let mut run = trainer
        .initialize_run()
        .map_err(|error| format!("Failed to initialize untuned-base eval run: {error}"))?;
    trainer
        .advance_run(&mut run, Some(1), current_epoch_ms(), plan.step_duration_ms)
        .map_err(|error| format!("Failed to score untuned-base held-out loss: {error}"))?
        .gradient_records
        .first()
        .map(|record| record.mean_loss)
        .ok_or_else(|| "Untuned-base eval produced no held-out loss".to_string())
}

fn held_out_loss_for_candidate(
    job_id: &str,
    plan: &GemmaFinetuneTrainingPlanStatus,
    base_binding: &GemmaE4bServedBaseModelBinding,
    target_set: &GemmaE4bCudaAdapterTargetSet,
    held_out_samples: Vec<GemmaE4bLmHeadSupervisionSample>,
    artifact: &GemmaE4bCudaAdapterExportedArtifact,
) -> Result<f32, String> {
    let trainer = GemmaE4bCudaAdapterSftTrainer::new(
        GemmaE4bCudaAdapterSftConfig {
            run_id: format!("{job_id}-candidate-eval"),
            budget: TrainingLoopBudget::new(1, 1, 1)
                .map_err(|error| format!("Failed to build candidate eval budget: {error}"))?,
            batch_size: plan.batch_size,
            optimizer: TrainingOptimizerConfig::adamw(
                plan.learning_rate,
                plan.beta1,
                plan.beta2,
                plan.epsilon,
            )
            .with_weight_decay(plan.weight_decay)
            .with_gradient_clip_norm(plan.gradient_clip_norm.unwrap_or(1.0)),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
        },
        target_set.clone(),
        base_binding.clone(),
        held_out_samples,
    )
    .map_err(|error| format!("Failed to initialize candidate eval lane: {error}"))?;
    let adapter = artifact
        .load_lm_head_lora_artifact()
        .map_err(|error| format!("Failed to reload exported Gemma adapter: {error}"))?;
    let mut run = trainer
        .initialize_run_from_loaded_adapter(&adapter)
        .map_err(|error| format!("Failed to initialize candidate eval run: {error}"))?;
    trainer
        .advance_run(&mut run, Some(1), current_epoch_ms(), plan.step_duration_ms)
        .map_err(|error| format!("Failed to score candidate held-out loss: {error}"))?
        .gradient_records
        .first()
        .map(|record| record.mean_loss)
        .ok_or_else(|| "Candidate eval produced no held-out loss".to_string())
}

fn build_proxy_eval_receipt(
    subject_kind: GemmaE4bFinetuneEvalSubjectKind,
    subject_id: &str,
    subject_digest: &str,
    validation_receipt: &GemmaFinetuneValidationReceiptStatus,
    held_out_loss: f32,
    detail: &str,
) -> GemmaE4bFinetuneEvalReceipt {
    let held_out_score_bps = loss_to_bps(held_out_loss);
    let held_out_pass_rate_bps = loss_to_bps(held_out_loss);
    let mut receipt = GemmaE4bFinetuneEvalReceipt {
        schema_version: String::from(GEMMA_E4B_FINETUNE_EVAL_RECEIPT_SCHEMA_VERSION),
        receipt_id: format!(
            "{}.eval",
            slugify(subject_id).chars().take(64).collect::<String>()
        ),
        subject_kind,
        subject_id: subject_id.to_string(),
        subject_digest: subject_digest.to_string(),
        benchmark_package_storage_key: validation_receipt
            .eval_pack_binding
            .benchmark_package_storage_key
            .clone(),
        benchmark_package_digest: validation_receipt
            .eval_pack_binding
            .benchmark_package_digest
            .clone(),
        held_out_validation_split_ref: validation_receipt
            .dataset_contract
            .held_out_validation_split_ref
            .clone(),
        final_report_split_ref: validation_receipt
            .dataset_contract
            .final_report_split_ref
            .clone(),
        held_out_pass_rate_bps,
        held_out_score_bps,
        chat_template_passed: validation_receipt.template_compatible,
        assistant_mask_passed: validation_receipt.assistant_mask_coverage_bps == 10_000,
        tool_call_format_passed: validation_receipt.template_compatible,
        formatting_passed: validation_receipt.template_compatible,
        steerability_passed: held_out_loss.is_finite(),
        detail: detail.to_string(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn loss_to_bps(loss: f32) -> u32 {
    let clamped = if loss.is_finite() {
        (10_000.0 / (1.0 + loss.max(0.0))).round()
    } else {
        1.0
    };
    clamped.clamp(1.0, 10_000.0) as u32
}

fn parse_review_state(raw: Option<&str>) -> Result<GemmaE4bOperatorReviewState, String> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(GemmaE4bOperatorReviewState::Pending),
        Some("pending") => Ok(GemmaE4bOperatorReviewState::Pending),
        Some("approved") => Ok(GemmaE4bOperatorReviewState::Approved),
        Some("rejected") => Ok(GemmaE4bOperatorReviewState::Rejected),
        Some(other) => Err(format!(
            "Unsupported Gemma promotion review_state `{other}`; use pending, approved, or rejected"
        )),
    }
}

fn append_job_event(
    job: &mut GemmaFinetuneJobStatus,
    kind: GemmaFinetuneJobEventKind,
    phase: GemmaFinetuneJobPhase,
    detail: String,
    occurred_at_epoch_ms: u64,
) {
    let seq = job.events.first().map(|event| event.seq + 1).unwrap_or(1);
    job.events.insert(
        0,
        GemmaFinetuneEventStatus {
            event_id: format!("{}-event-{}", job.job_id, seq),
            seq,
            occurred_at_epoch_ms,
            kind,
            phase,
            detail,
        },
    );
    if job.events.len() > GEMMA_FINETUNE_EVENT_LIMIT {
        job.events.truncate(GEMMA_FINETUNE_EVENT_LIMIT);
    }
}

fn write_checkpoint(
    job_id: &str,
    checkpoint: &GemmaE4bCudaAdapterCheckpoint,
) -> Result<String, String> {
    let path = gemma_finetune_root_dir()
        .join("jobs")
        .join(job_id)
        .join("checkpoints")
        .join(format!("{}.json", checkpoint.checkpoint_id));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create Gemma checkpoint dir {}: {error}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_vec_pretty(checkpoint)
        .map_err(|error| format!("Failed to encode Gemma checkpoint: {error}"))?;
    fs::write(path.as_path(), raw).map_err(|error| {
        format!(
            "Failed to write Gemma checkpoint {}: {error}",
            path.display()
        )
    })?;
    Ok(path.display().to_string())
}

fn read_checkpoint(path: &str) -> Result<GemmaE4bCudaAdapterCheckpoint, String> {
    let raw = fs::read(path)
        .map_err(|error| format!("Failed to read Gemma checkpoint {}: {error}", path))?;
    serde_json::from_slice::<GemmaE4bCudaAdapterCheckpoint>(&raw)
        .map_err(|error| format!("Failed to decode Gemma checkpoint {}: {error}", path))
}

fn write_artifact(
    job_id: &str,
    plan: &GemmaFinetuneTrainingPlanStatus,
    artifact: &GemmaE4bCudaAdapterExportedArtifact,
) -> Result<String, String> {
    let path = gemma_finetune_root_dir()
        .join("jobs")
        .join(job_id)
        .join("artifacts")
        .join(format!(
            "{}-{}.safetensors",
            plan.adapter_id, plan.adapter_revision
        ));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create Gemma artifact dir {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path.as_path(), artifact.adapter_bytes.as_slice())
        .map_err(|error| format!("Failed to write Gemma artifact {}: {error}", path.display()))?;
    Ok(path.display().to_string())
}

fn promotion_decision_state_label(decision: &GemmaE4bCheckpointPromotionDecision) -> &'static str {
    match decision.decision_state {
        GemmaE4bPromotionDecisionState::Promote => "promote",
        GemmaE4bPromotionDecisionState::HoldForReview => "hold_for_review",
        GemmaE4bPromotionDecisionState::Reject => "reject",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openagents-gemma-finetune-{}-{}-{}.json",
            name,
            std::process::id(),
            current_epoch_ms()
        ))
    }

    fn sample_rows() -> Vec<GemmaE4bLmHeadSupervisionSample> {
        vec![
            GemmaE4bLmHeadSupervisionSample::new("a", vec![1.0, 0.0, 0.0, 0.0], 48, 11),
            GemmaE4bLmHeadSupervisionSample::new("b", vec![0.0, 1.0, 0.0, 0.0], 106, 12),
            GemmaE4bLmHeadSupervisionSample::new("c", vec![0.0, 0.0, 1.0, 0.0], 50, 10),
            GemmaE4bLmHeadSupervisionSample::new("d", vec![0.0, 0.0, 0.0, 1.0], 1, 9),
        ]
    }

    fn write_rows(path: &Path) {
        let raw = serde_json::to_vec_pretty(&sample_rows()).expect("encode rows");
        fs::write(path, raw).expect("write rows");
    }

    fn prepare_project_and_dataset(
        controller: &mut GemmaFinetuneController,
        train: &Path,
        validation: &Path,
        holdout: &Path,
    ) -> (GemmaFinetuneProjectStatus, GemmaFinetuneDatasetStatus) {
        let project = controller
            .create_project(&GemmaFinetuneProjectCreateRequest {
                project_name: "Support agent".to_string(),
                tenant_id: Some("design-partner".to_string()),
                base_served_artifact_digest: "sha256:gemma4-e4b-base".to_string(),
                hidden_size: 4,
            })
            .expect("create project");
        let dataset = controller
            .register_dataset(&GemmaFinetuneDatasetRegisterRequest {
                project_id: project.project_id.clone(),
                dataset_ref: "dataset://openagents/support-agent@2026.04".to_string(),
                train_path: train.display().to_string(),
                validation_path: validation.display().to_string(),
                holdout_path: holdout.display().to_string(),
                baseline_short_path: Some(validation.display().to_string()),
                final_report_path: Some(holdout.display().to_string()),
                chat_template_digest: canonical_gemma_e4b_finetuning_mvp_contract()
                    .expect("contract")
                    .tokenizer
                    .template_digest,
                assistant_mask_coverage_bps: 10_000,
                overlap_check_id: None,
                overlap_detail: None,
                compared_benchmark_refs: Vec::new(),
            })
            .expect("register dataset");
        (project, dataset)
    }

    #[test]
    fn create_project_binds_bounded_gemma_lane() {
        let storage = temp_path("project-state");
        let mut controller = GemmaFinetuneController::load(storage.clone());
        let project = controller
            .create_project(&GemmaFinetuneProjectCreateRequest {
                project_name: "Support agent".to_string(),
                tenant_id: None,
                base_served_artifact_digest: "sha256:gemma4-e4b-base".to_string(),
                hidden_size: 4,
            })
            .expect("create project");
        assert_eq!(project.tenant_id, DEFAULT_TENANT_ID);
        assert_eq!(project.lane_binding.model_id, "gemma4:e4b");
        assert!(
            project
                .lane_binding
                .eval_pack_storage_key
                .contains("benchmark://psionic/gemma4/e4b/finetune_eval")
        );
        assert!(storage.exists());
        let _ = fs::remove_file(storage);
    }

    #[test]
    fn create_job_queues_status_after_dataset_validation() {
        let storage = temp_path("job-state");
        let train = temp_path("job-train");
        let validation = temp_path("job-validation");
        let holdout = temp_path("job-holdout");
        write_rows(train.as_path());
        write_rows(validation.as_path());
        write_rows(holdout.as_path());

        let mut controller = GemmaFinetuneController::load(storage.clone());
        let (project, dataset) = prepare_project_and_dataset(
            &mut controller,
            train.as_path(),
            validation.as_path(),
            holdout.as_path(),
        );
        let job = controller
            .create_job(&GemmaFinetuneJobCreateRequest {
                project_id: project.project_id.clone(),
                dataset_id: Some(dataset.dataset_id.clone()),
            })
            .expect("create job");
        assert_eq!(job.state, GemmaFinetuneJobState::Pending);
        assert_eq!(job.phase, GemmaFinetuneJobPhase::Queued);
        assert_eq!(job.dataset_id, dataset.dataset_id);
        assert!(controller.status().job_count >= 1);

        let _ = fs::remove_file(storage);
        let _ = fs::remove_file(train);
        let _ = fs::remove_file(validation);
        let _ = fs::remove_file(holdout);
    }

    #[test]
    fn promote_checkpoint_records_model_ref_when_decision_is_promote()
    -> Result<(), Box<dyn std::error::Error>> {
        let storage = temp_path("promotion-state");
        let train = temp_path("promotion-train");
        let validation = temp_path("promotion-validation");
        let holdout = temp_path("promotion-holdout");
        write_rows(train.as_path());
        write_rows(validation.as_path());
        write_rows(holdout.as_path());

        let mut controller = GemmaFinetuneController::load(storage.clone());
        let (project, dataset) = prepare_project_and_dataset(
            &mut controller,
            train.as_path(),
            validation.as_path(),
            holdout.as_path(),
        );
        let job = controller
            .create_job(&GemmaFinetuneJobCreateRequest {
                project_id: project.project_id.clone(),
                dataset_id: Some(dataset.dataset_id.clone()),
            })
            .expect("create job");
        let plan = default_training_plan(job.job_id.as_str(), &project)?;
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            training_config_from_plan(job.job_id.as_str(), &plan)?,
            canonical_gemma_e4b_cuda_adapter_target_set(),
            project_base_binding(&project)?,
            sample_rows(),
        )?;
        let mut run = trainer.initialize_run()?;
        let _ = trainer.advance_run(&mut run, Some(1), 1_000, plan.step_duration_ms)?;
        let checkpoint =
            trainer.save_checkpoint("support-agent-r1-final", &run, current_epoch_ms())?;
        let checkpoint_path =
            write_checkpoint(job.job_id.as_str(), &checkpoint).expect("checkpoint");
        let eval_receipt = build_proxy_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate,
            checkpoint.checkpoint_id.as_str(),
            checkpoint.checkpoint_digest.as_str(),
            &dataset.validation_receipt,
            0.5,
            "candidate",
        );
        let base_eval = build_proxy_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::UntunedBase,
            "gemma4:e4b",
            project.base_served_artifact_digest.as_str(),
            &dataset.validation_receipt,
            1.5,
            "base",
        );
        controller
            .mutate_job(job.job_id.as_str(), |job| {
                job.state = GemmaFinetuneJobState::Completed;
                job.phase = GemmaFinetuneJobPhase::AwaitingPromotion;
                job.untuned_base_eval = Some(base_eval.clone());
                job.checkpoint_candidate_eval = Some(eval_receipt.clone());
                job.checkpoints.push(GemmaFinetuneCheckpointStatus {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                    checkpoint_digest: checkpoint.checkpoint_digest.clone(),
                    checkpoint_path,
                    saved_at_epoch_ms: checkpoint.saved_at_ms,
                    completed_steps: 1,
                    max_steps: 1,
                    mean_loss: Some(0.5),
                    eval_receipt: Some(eval_receipt.clone()),
                    promotion_decision: None,
                });
                job.artifacts.push(GemmaFinetuneArtifactStatus {
                    artifact_id: "support-agent-r1".to_string(),
                    adapter_id: "support-agent".to_string(),
                    adapter_revision: "r1".to_string(),
                    artifact_path: "/tmp/support-agent-r1.safetensors".to_string(),
                    adapter_artifact_digest: "adapter-digest".to_string(),
                    adapter_identity_digest: "identity-digest".to_string(),
                    created_at_epoch_ms: current_epoch_ms(),
                    promoted_model_ref: None,
                });
                Ok(())
            })
            .expect("seed completed job");
        let promotion = controller
            .promote_checkpoint(&GemmaFinetuneCheckpointPromotionRequest {
                job_id: job.job_id,
                checkpoint_id: Some("support-agent-r1-final".to_string()),
                reviewer_id: "operator-1".to_string(),
                review_state: Some("approved".to_string()),
                failed_case_ids: Vec::new(),
                summary: Some("approve".to_string()),
            })
            .expect("promote checkpoint");
        assert_eq!(
            promotion.decision.decision_state,
            GemmaE4bPromotionDecisionState::Promote
        );
        assert!(promotion.promoted_model_ref.is_some());

        let _ = fs::remove_file(storage);
        let _ = fs::remove_file(train);
        let _ = fs::remove_file(validation);
        let _ = fs::remove_file(holdout);
        Ok(())
    }
}
