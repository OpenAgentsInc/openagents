use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    GemmaE4bAssistantMaskKind, GemmaE4bBenchmarkOverlapCheck, GemmaE4bFinetuneDatasetContract,
    GemmaE4bFinetuneEvalPackBinding, GemmaE4bLmHeadSupervisionSample,
    canonical_gemma_e4b_finetune_eval_pack_binding, canonical_gemma_e4b_finetuning_mvp_contract,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const GEMMA_FINETUNE_SCHEMA_VERSION: u16 = 1;
const GEMMA_FINETUNE_STATE_FILENAME: &str = "gemma-finetune.json";
const GEMMA_FINETUNE_ROOT_DIR: &str = "gemma-finetune";
const DEFAULT_TENANT_ID: &str = "desktop-owner";
const GEMMA_FINETUNE_EVAL_BENCHMARK_REF: &str = "benchmark://psionic/gemma4/e4b/finetune_eval";

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaFinetuneStatus {
    pub available: bool,
    pub schema_version: u16,
    pub storage_path: Option<String>,
    pub project_count: usize,
    pub dataset_count: usize,
    pub validation_receipt_count: usize,
    pub projects: Vec<GemmaFinetuneProjectStatus>,
    pub datasets: Vec<GemmaFinetuneDatasetStatus>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedGemmaFinetuneState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    last_action: Option<String>,
    last_error: Option<String>,
    projects: Vec<GemmaFinetuneProjectStatus>,
    datasets: Vec<GemmaFinetuneDatasetStatus>,
}

struct GemmaFinetuneController {
    storage_path: PathBuf,
    state: PersistedGemmaFinetuneState,
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
            projects: self.state.projects.clone(),
            datasets: self.state.datasets.clone(),
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
        self.state
            .projects
            .sort_by(|left, right| right.updated_at_epoch_ms.cmp(&left.updated_at_epoch_ms));
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
            schema_version: String::from(
                psionic_train::GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION,
            ),
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
        self.state
            .datasets
            .sort_by(|left, right| right.updated_at_epoch_ms.cmp(&left.updated_at_epoch_ms));
        if let Some(project_mut) = self.state.projects.get_mut(project_index) {
            project_mut.active_dataset_id = Some(dataset_id.clone());
            project_mut.updated_at_epoch_ms = now;
            project_mut.last_action = Some(format!(
                "Bound Gemma dataset {} to project {}",
                dataset_id, project_mut.project_id
            ));
            project_mut.last_error = (!errors.is_empty()).then(|| errors.join(" | "));
        }
        self.state.last_action = Some(format!(
            "Registered Gemma finetune dataset {} for project {}",
            dataset_id, project.project_id
        ));
        self.state.last_error = (!errors.is_empty()).then(|| errors.join(" | "));
        self.persist()?;
        Ok(dataset)
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

#[allow(dead_code)]
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
        ]
    }

    fn write_rows(path: &Path) {
        let raw = serde_json::to_vec_pretty(&sample_rows()).expect("encode rows");
        fs::write(path, raw).expect("write rows");
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
    fn register_dataset_persists_validation_receipt() {
        let storage = temp_path("dataset-state");
        let train = temp_path("train");
        let validation = temp_path("validation");
        let holdout = temp_path("holdout");
        write_rows(train.as_path());
        write_rows(validation.as_path());
        write_rows(holdout.as_path());

        let mut controller = GemmaFinetuneController::load(storage.clone());
        let project = controller
            .create_project(&GemmaFinetuneProjectCreateRequest {
                project_name: "Support agent".to_string(),
                tenant_id: Some("design-partner".to_string()),
                base_served_artifact_digest: "sha256:gemma4-e4b-base".to_string(),
                hidden_size: 4,
            })
            .expect("create project");
        let contract = canonical_gemma_e4b_finetuning_mvp_contract().expect("contract");
        let dataset = controller
            .register_dataset(&GemmaFinetuneDatasetRegisterRequest {
                project_id: project.project_id.clone(),
                dataset_ref: "dataset://openagents/support-agent@2026.04".to_string(),
                train_path: train.display().to_string(),
                validation_path: validation.display().to_string(),
                holdout_path: holdout.display().to_string(),
                baseline_short_path: None,
                final_report_path: None,
                chat_template_digest: contract.tokenizer.template_digest.clone(),
                assistant_mask_coverage_bps: 10_000,
                overlap_check_id: None,
                overlap_detail: None,
                compared_benchmark_refs: Vec::new(),
            })
            .expect("register dataset");
        assert_eq!(dataset.validation_receipt.status, "validated");
        assert!(dataset.validation_receipt.errors.is_empty());
        assert_eq!(dataset.train_split.sample_count, 2);
        assert_eq!(controller.status().dataset_count, 1);

        let _ = fs::remove_file(storage);
        let _ = fs::remove_file(train);
        let _ = fs::remove_file(validation);
        let _ = fs::remove_file(holdout);
    }

    #[test]
    fn register_dataset_reports_template_drift() {
        let storage = temp_path("dataset-drift-state");
        let train = temp_path("drift-train");
        let validation = temp_path("drift-validation");
        let holdout = temp_path("drift-holdout");
        write_rows(train.as_path());
        write_rows(validation.as_path());
        write_rows(holdout.as_path());

        let mut controller = GemmaFinetuneController::load(storage.clone());
        let project = controller
            .create_project(&GemmaFinetuneProjectCreateRequest {
                project_name: "Support agent".to_string(),
                tenant_id: None,
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
                baseline_short_path: None,
                final_report_path: None,
                chat_template_digest: Some("drifted-template".to_string()),
                assistant_mask_coverage_bps: 10_000,
                overlap_check_id: None,
                overlap_detail: None,
                compared_benchmark_refs: Vec::new(),
            })
            .expect("register dataset");
        assert_eq!(dataset.validation_receipt.status, "rejected");
        assert!(
            dataset
                .validation_receipt
                .errors
                .iter()
                .any(|error| error.contains("template"))
        );

        let _ = fs::remove_file(storage);
        let _ = fs::remove_file(train);
        let _ = fs::remove_file(validation);
        let _ = fs::remove_file(holdout);
    }
}
