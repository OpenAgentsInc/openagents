//! Qwen3.6-27B legal fine-tuning target-path milestone.
//!
//! This runner is intentionally explicit about the current boundary. It loads
//! the Qwen3.6-27B smoke target artifacts, runs the Rust legal eval, builds
//! public training data, trains SFT, runs the Rust DPO and GRPO smoke stages,
//! evaluates the candidate ladder, promotes only the winning candidate, and
//! writes a report. It does not claim full 27B weight training.

use std::fs;
use std::path::{Path, PathBuf};

use psionic_eval::{
    run_legal_benchmark_eval_suite, stable_json_digest, LegalBenchmarkEvalMode,
    LegalBenchmarkEvalSuiteError, LegalBenchmarkEvalSuiteManifest,
    LegalBenchmarkEvalSuiteModelReport, LegalBenchmarkEvalSuiteRunConfig,
};
use psionic_models::{
    run_qwen36_legal_prompt_smoke, write_qwen36_27b_smoke_safetensors,
    Qwen36LegalPromptSmokeReport, Qwen36TargetPathError, QWEN36_27B_MODEL_ID,
    QWEN36_27B_SMOKE_CONFIG_PATH, QWEN36_27B_SMOKE_SHARD_PATH, QWEN36_27B_SMOKE_TOKENIZER_PATH,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    default_qwen36_legal_dpo_smoke_config, default_qwen36_legal_grpo_smoke_config,
    run_psionic_legal_dpo_config, run_psionic_legal_grpo_config, run_psionic_legal_sft_config,
    PsionicLegalDpoError, PsionicLegalDpoRunArtifacts, PsionicLegalGrpoError,
    PsionicLegalGrpoRunArtifacts, PsionicLegalSftBaseArtifactMode, PsionicLegalSftConfig,
    PsionicLegalSftError, PsionicLegalSftRunArtifacts, PsionicLegalSftSample,
};

pub const QWEN36_27B_LEGAL_FT_MILESTONE_SCHEMA_VERSION: &str =
    "psionic.qwen36_27b_legal_ft_milestone.v1";
pub const QWEN36_27B_LEGAL_FT_DATASET_SCHEMA_VERSION: &str =
    "psionic.qwen36_27b_legal_ft_dataset.v1";
pub const QWEN36_27B_LEGAL_FT_PROMOTION_SCHEMA_VERSION: &str =
    "psionic.qwen36_27b_legal_ft_promotion.v1";
pub const DEFAULT_QWEN36_27B_LEGAL_FT_RUN_ID: &str = "qwen36-27b-legal-ft-001";
pub const DEFAULT_QWEN36_27B_LEGAL_FT_SUITE_PATH: &str = "suites/harvey_public_three.json";
pub const DEFAULT_QWEN36_27B_LEGAL_FT_OUTPUT_DIR: &str = "target/legal/qwen36-27b-legal-ft-001";
pub const DEFAULT_QWEN36_27B_LEGAL_FT_REPORT_PATH: &str = "reports/qwen36-27b-legal-ft-001.md";
const BASE_CANDIDATE_ID: &str = "qwen36_27b_base";
const SFT_CANDIDATE_ID: &str = "qwen36_27b_sft_round_001";
const DPO_CANDIDATE_ID: &str = "qwen36_27b_sft_dpo_round_001";
const GRPO_CANDIDATE_ID: &str = "qwen36_27b_sft_grpo_round_001";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Qwen36LegalFtMilestoneConfig {
    pub run_id: String,
    pub suite_path: PathBuf,
    pub output_dir: PathBuf,
    pub report_path: PathBuf,
    pub prompt_path: PathBuf,
    pub model_config_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub shard_path: PathBuf,
    pub champion_model_id: String,
}

impl Default for Qwen36LegalFtMilestoneConfig {
    fn default() -> Self {
        Self {
            run_id: String::from(DEFAULT_QWEN36_27B_LEGAL_FT_RUN_ID),
            suite_path: PathBuf::from(DEFAULT_QWEN36_27B_LEGAL_FT_SUITE_PATH),
            output_dir: PathBuf::from(DEFAULT_QWEN36_27B_LEGAL_FT_OUTPUT_DIR),
            report_path: PathBuf::from(DEFAULT_QWEN36_27B_LEGAL_FT_REPORT_PATH),
            prompt_path: PathBuf::from("fixtures/legal/smoke.prompt"),
            model_config_path: PathBuf::from(QWEN36_27B_SMOKE_CONFIG_PATH),
            tokenizer_path: PathBuf::from(QWEN36_27B_SMOKE_TOKENIZER_PATH),
            shard_path: PathBuf::from(QWEN36_27B_SMOKE_SHARD_PATH),
            champion_model_id: String::from("Qwen/Qwen3.6-27B/current-three-task-champion"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtArtifactHash {
    pub role: String,
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtSftRecord {
    pub sample_id: String,
    pub task_id: String,
    pub prompt: String,
    pub answer_markdown: String,
    pub answer_sha256: String,
    pub source_token_count: u32,
    pub target_token_id: u32,
    pub hidden_state: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtDatasetReceipt {
    pub schema_version: String,
    pub dataset_id: String,
    pub dataset_path: String,
    pub dataset_sha256: String,
    pub suite_path: String,
    pub suite_id: String,
    pub suite_hash: String,
    pub training_allowed: bool,
    pub record_count: usize,
    pub task_ids: Vec<String>,
    pub records: Vec<Qwen36LegalFtSftRecord>,
    pub receipt_digest: String,
}

impl Qwen36LegalFtDatasetReceipt {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest("psionic.qwen36_27b_legal_ft_dataset_receipt.v1", &clone)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36LegalFtCandidateStage {
    Base,
    Sft,
    Dpo,
    Grpo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtCandidateReport {
    pub candidate_id: String,
    pub stage: Qwen36LegalFtCandidateStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_artifact_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_artifact_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_receipt_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_receipt_digest: Option<String>,
    pub eval_output_dir: String,
    pub eval_report_hash: String,
    pub legal_score_bps: u32,
    pub answer_file_success_rate_bps: u32,
    pub integrity_failure_count: u64,
    pub tool_failure_count: u64,
    pub timeout_failure_count: u64,
    pub python_invoked: bool,
    pub hard_failure_count: u64,
    pub candidate_notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36LegalFtPromotionDecision {
    Promote,
    Hold,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtPromotionReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub champion_candidate_id: String,
    pub promoted_candidate_id: String,
    pub champion_score_bps: u32,
    pub promoted_score_bps: u32,
    pub score_delta_bps: i32,
    pub decision: Qwen36LegalFtPromotionDecision,
    pub reasons: Vec<String>,
    pub receipt_digest: String,
}

impl Qwen36LegalFtPromotionReceipt {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest("psionic.qwen36_27b_legal_ft_promotion_receipt.v1", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalFtMilestoneReport {
    pub schema_version: String,
    pub run_id: String,
    pub generated_on: String,
    pub model_id: String,
    pub suite_path: String,
    pub suite_id: String,
    pub suite_hash: String,
    pub suite_training_allowed: bool,
    pub hidden_benchmark_training: bool,
    pub base_artifacts: Vec<Qwen36LegalFtArtifactHash>,
    pub target_load_report_path: String,
    pub target_load_report_sha256: String,
    pub target_load_claim_boundary: String,
    pub model_load_verified: bool,
    pub base_eval_report_hash: String,
    pub dataset_receipt: Qwen36LegalFtDatasetReceipt,
    pub sft_config_path: String,
    pub sft_config_sha256: String,
    pub candidate_ladder: Vec<Qwen36LegalFtCandidateReport>,
    pub promoted_candidate_id: String,
    pub promotion_receipt_path: String,
    pub promotion_receipt: Qwen36LegalFtPromotionReceipt,
    pub all_receipts_present: bool,
    pub no_python_invoked: bool,
    pub claim_boundary: String,
    pub report_path: String,
    pub report_digest: String,
}

impl Qwen36LegalFtMilestoneReport {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest("psionic.qwen36_27b_legal_ft_milestone_report.v1", &clone)
    }
}

#[derive(Debug, Error)]
pub enum Qwen36LegalFtMilestoneError {
    #[error("Qwen3.6-27B legal FT I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Qwen3.6-27B legal FT JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Qwen3.6 target load failed: {0}")]
    Target(#[from] Qwen36TargetPathError),
    #[error("legal eval failed: {0}")]
    Eval(#[from] LegalBenchmarkEvalSuiteError),
    #[error("legal SFT failed: {0}")]
    Sft(#[from] PsionicLegalSftError),
    #[error("legal DPO failed: {0}")]
    Dpo(#[from] PsionicLegalDpoError),
    #[error("legal GRPO failed: {0}")]
    Grpo(#[from] PsionicLegalGrpoError),
    #[error("invalid Qwen3.6-27B legal FT milestone: {0}")]
    InvalidMilestone(String),
}

pub fn run_qwen36_27b_legal_ft_milestone(
    config: &Qwen36LegalFtMilestoneConfig,
) -> Result<Qwen36LegalFtMilestoneReport, Qwen36LegalFtMilestoneError> {
    create_dir(config.output_dir.as_path())?;
    if let Some(parent) = config.report_path.parent() {
        create_dir(parent)?;
    }
    if !config.shard_path.exists() {
        if let Some(parent) = config.shard_path.parent() {
            create_dir(parent)?;
        }
        write_qwen36_27b_smoke_safetensors(config.shard_path.as_path())?;
    }

    let target_report = run_qwen36_legal_prompt_smoke(
        QWEN36_27B_MODEL_ID,
        config.prompt_path.as_path(),
        config.model_config_path.as_path(),
        config.tokenizer_path.as_path(),
        &[config.shard_path.clone()],
    )?;
    let target_report_path = config.output_dir.join("qwen36_27b_target_load_report.json");
    write_json(target_report_path.as_path(), &target_report)?;
    let target_load_report_sha256 = sha256_file(target_report_path.as_path())?;
    let base_artifacts = base_artifacts(config, &target_report)?;
    let model_load_verified = target_report.model_id == QWEN36_27B_MODEL_ID
        && target_report.served_model_id == "qwen3.6-27b"
        && !target_report.loaded_shards.is_empty()
        && base_artifacts
            .iter()
            .all(|artifact| artifact.sha256.len() == 64);

    let suite_bytes = read_file(config.suite_path.as_path())?;
    let suite_manifest: LegalBenchmarkEvalSuiteManifest = serde_json::from_slice(&suite_bytes)?;
    let suite_hash = stable_json_digest(
        "psionic.legal_benchmark.eval_suite_manifest.v1",
        &suite_manifest,
    )?;
    if !suite_manifest.training_allowed
        || suite_manifest.mode == LegalBenchmarkEvalMode::HiddenAuditOnly
    {
        return Err(Qwen36LegalFtMilestoneError::InvalidMilestone(
            "the Qwen3.6-27B milestone only trains on public training-allowed suite data"
                .to_string(),
        ));
    }

    let base_eval = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
        suite_path: config.suite_path.clone(),
        base_model: String::from(QWEN36_27B_MODEL_ID),
        adapter: String::from("qwen36_27b_base_no_adapter"),
        output_dir: config.output_dir.join("eval").join("base"),
        replay_command: vec![
            String::from("cargo"),
            String::from("run"),
            String::from("-p"),
            String::from("psionic-train"),
            String::from("--example"),
            String::from("qwen36_27b_legal_ft_milestone"),
        ],
    })?;

    let dataset_dir = config.output_dir.join("dataset");
    create_dir(dataset_dir.as_path())?;
    let dataset = write_sft_dataset(
        &suite_manifest,
        config.suite_path.as_path(),
        suite_hash.as_str(),
        dataset_dir.as_path(),
    )?;
    let sft_config = sft_config(config, &dataset);
    let sft_config_path = config.output_dir.join("sft_config.json");
    write_json(sft_config_path.as_path(), &sft_config)?;
    let sft_config_sha256 = sha256_file(sft_config_path.as_path())?;
    let sft_artifacts = run_psionic_legal_sft_config(&sft_config)?;

    let dpo_artifacts = run_dpo_stage(config, &sft_artifacts)?;
    let grpo_artifacts = run_grpo_stage(config, &sft_artifacts)?;

    let mut candidates = Vec::new();
    candidates.push(base_candidate_report(
        config,
        &base_eval.base_model_result,
        &base_eval.replay_receipt.report_hash,
    ));
    candidates.push(candidate_report_from_eval(
        SFT_CANDIDATE_ID,
        Qwen36LegalFtCandidateStage::Sft,
        config,
        &sft_artifacts.adapter_artifact_path,
        &sft_artifacts.receipt_path,
        sft_artifacts.receipt.receipt_digest.as_str(),
        sft_artifacts.receipt.python_invoked,
        "SFT adapter trained from the public Harvey three-task SFT dataset.",
    )?);
    candidates.push(candidate_report_from_eval(
        DPO_CANDIDATE_ID,
        Qwen36LegalFtCandidateStage::Dpo,
        config,
        &dpo_artifacts.adapter_artifact_path,
        &dpo_artifacts.receipt_path,
        dpo_artifacts.receipt.receipt_digest.as_str(),
        dpo_artifacts.receipt.python_invoked,
        "DPO adapter trained from public legal preference pairs using the SFT adapter as parent.",
    )?);
    candidates.push(candidate_report_from_eval(
        GRPO_CANDIDATE_ID,
        Qwen36LegalFtCandidateStage::Grpo,
        config,
        &grpo_artifacts.adapter_artifact_path,
        &grpo_artifacts.receipt_path,
        grpo_artifacts.receipt.receipt_digest.as_str(),
        grpo_artifacts.receipt.python_invoked,
        "GRPO adapter trained from deterministic local reward groups using the SFT adapter as parent.",
    )?);

    let promotion = promotion_receipt(config.run_id.as_str(), candidates.as_slice())?;
    let promotion_receipt_path = config.output_dir.join("promotion_receipt.json");
    write_json(promotion_receipt_path.as_path(), &promotion)?;
    let all_receipts_present = receipt_paths_present(candidates.as_slice())
        && promotion_receipt_path.is_file()
        && target_report_path.is_file()
        && Path::new(&dataset.dataset_path).is_file();
    let no_python_invoked = candidates.iter().all(|candidate| !candidate.python_invoked);
    let mut report = Qwen36LegalFtMilestoneReport {
        schema_version: String::from(QWEN36_27B_LEGAL_FT_MILESTONE_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        generated_on: String::from("2026-05-20"),
        model_id: String::from(QWEN36_27B_MODEL_ID),
        suite_path: config.suite_path.to_string_lossy().to_string(),
        suite_id: suite_manifest.suite_id.clone(),
        suite_hash,
        suite_training_allowed: suite_manifest.training_allowed,
        hidden_benchmark_training: false,
        base_artifacts,
        target_load_report_path: target_report_path.to_string_lossy().to_string(),
        target_load_report_sha256,
        target_load_claim_boundary: target_report.claim_boundary,
        model_load_verified,
        base_eval_report_hash: base_eval.replay_receipt.report_hash,
        dataset_receipt: dataset,
        sft_config_path: sft_config_path.to_string_lossy().to_string(),
        sft_config_sha256,
        candidate_ladder: candidates,
        promoted_candidate_id: promotion.promoted_candidate_id.clone(),
        promotion_receipt_path: promotion_receipt_path.to_string_lossy().to_string(),
        promotion_receipt: promotion,
        all_receipts_present,
        no_python_invoked,
        claim_boundary: String::from(
            "This is a Qwen3.6-27B target-path legal fine-tuning milestone over public training-allowed Harvey fixtures. It loads the Qwen3.6-27B smoke target artifacts, runs Rust SFT, DPO, and GRPO adapter updates, evaluates the candidate ladder, and records receipts. It does not load the full 27B checkpoint, measure private Harvey tasks, or prove production leaderboard standing.",
        ),
        report_path: config.report_path.to_string_lossy().to_string(),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest()?;
    write_json(
        config.output_dir.join("milestone_report.json").as_path(),
        &report,
    )?;
    write_markdown_report(config.report_path.as_path(), &report)?;
    Ok(report)
}

fn base_artifacts(
    config: &Qwen36LegalFtMilestoneConfig,
    target_report: &Qwen36LegalPromptSmokeReport,
) -> Result<Vec<Qwen36LegalFtArtifactHash>, Qwen36LegalFtMilestoneError> {
    let mut artifacts = vec![
        artifact_hash("model_config", config.model_config_path.as_path())?,
        artifact_hash("tokenizer", config.tokenizer_path.as_path())?,
    ];
    for shard in &target_report.loaded_shards {
        let path = PathBuf::from(&shard.path);
        artifacts.push(artifact_hash("safetensors_shard", path.as_path())?);
    }
    Ok(artifacts)
}

fn artifact_hash(
    role: &str,
    path: &Path,
) -> Result<Qwen36LegalFtArtifactHash, Qwen36LegalFtMilestoneError> {
    let bytes = read_file(path)?;
    Ok(Qwen36LegalFtArtifactHash {
        role: role.to_owned(),
        path: path.to_string_lossy().to_string(),
        sha256: sha256_hex(bytes.as_slice()),
        byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

fn write_sft_dataset(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
    suite_hash: &str,
    output_dir: &Path,
) -> Result<Qwen36LegalFtDatasetReceipt, Qwen36LegalFtMilestoneError> {
    let dataset_path = output_dir.join("qwen36-27b-legal-ft-001-sft.jsonl");
    let mut records = Vec::new();
    let mut jsonl = String::new();
    for task_id in &manifest.fixed_task_order {
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == *task_id)
            .ok_or_else(|| {
                Qwen36LegalFtMilestoneError::InvalidMilestone(format!(
                    "fixed task `{task_id}` missing from suite"
                ))
            })?;
        let sample = sft_sample(task);
        let record = Qwen36LegalFtSftRecord {
            sample_id: sample.sample_id.clone(),
            task_id: task.task_id.clone(),
            prompt: task.instructions.clone(),
            answer_markdown: task.replay_answer_markdown.clone(),
            answer_sha256: sha256_hex(task.replay_answer_markdown.as_bytes()),
            source_token_count: sample.source_token_count,
            target_token_id: sample.target_token_id,
            hidden_state: sample.final_hidden_state,
        };
        jsonl.push_str(&serde_json::to_string(&record)?);
        jsonl.push('\n');
        records.push(record);
    }
    fs::write(&dataset_path, jsonl).map_err(|source| Qwen36LegalFtMilestoneError::Io {
        path: dataset_path.clone(),
        source,
    })?;
    let dataset_sha256 = sha256_file(dataset_path.as_path())?;
    let mut receipt = Qwen36LegalFtDatasetReceipt {
        schema_version: String::from(QWEN36_27B_LEGAL_FT_DATASET_SCHEMA_VERSION),
        dataset_id: String::from("dataset.qwen36-27b.harvey-public-three.ft-001"),
        dataset_path: dataset_path.to_string_lossy().to_string(),
        dataset_sha256,
        suite_path: suite_path.to_string_lossy().to_string(),
        suite_id: manifest.suite_id.clone(),
        suite_hash: suite_hash.to_owned(),
        training_allowed: manifest.training_allowed,
        record_count: records.len(),
        task_ids: manifest.fixed_task_order.clone(),
        records,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest()?;
    write_json(output_dir.join("dataset_receipt.json").as_path(), &receipt)?;
    Ok(receipt)
}

fn sft_config(
    config: &Qwen36LegalFtMilestoneConfig,
    dataset: &Qwen36LegalFtDatasetReceipt,
) -> PsionicLegalSftConfig {
    PsionicLegalSftConfig {
        schema_version: String::from(crate::PSIONIC_LEGAL_SFT_CONFIG_SCHEMA_VERSION),
        run_id: String::from("qwen36-27b-legal-ft-001-sft"),
        train_type: String::from("qlora"),
        base_model: String::from(QWEN36_27B_MODEL_ID),
        served_model_id: String::from("qwen3.6-27b"),
        base_model_revision: String::from("qwen3.6-27b-ft-001-smoke-revision"),
        base_artifact_mode: PsionicLegalSftBaseArtifactMode::SyntheticHiddenStateSmoke,
        base_served_artifact_digest: format!(
            "sha256:{}",
            sha256_hex(QWEN36_27B_MODEL_ID.as_bytes())
        ),
        base_safetensors_paths: Vec::new(),
        model_config_path: Some(config.model_config_path.to_string_lossy().to_string()),
        tokenizer_path: Some(config.tokenizer_path.to_string_lossy().to_string()),
        tokenizer_digest: String::from("sha256:qwen36-27b-tokenizer-smoke"),
        prompt_template_digest: String::from("sha256:qwen36-chat-template-v1-smoke"),
        hidden_size: 4,
        vocab_size: 256,
        adapter_target_id: String::from("lm_head"),
        target_modules: vec![String::from("all-linear")],
        moe_safety: None,
        lora_rank: 16,
        lora_alpha: 32.0,
        lora_dropout: 0.0,
        learning_rate: 0.12,
        epochs: 1,
        max_seq_len: 8192,
        gradient_accumulation_steps: 8,
        max_steps: 8,
        batch_size: 2,
        assistant_only_loss: true,
        ignore_empty_think_loss: true,
        gradient_clip_norm: Some(1.0),
        started_at_ms: 10_000,
        step_duration_ms: 20,
        dataset_ref: format!("{}#{}", dataset.dataset_id, dataset.dataset_sha256),
        validator_policy_ref: String::from(
            "policy://validator/legal-benchmark/qwen36-27b-ft-001-sft",
        ),
        adapter_id: String::from(SFT_CANDIDATE_ID),
        adapter_revision: String::from("r1"),
        output_dir: config.output_dir.join("sft").to_string_lossy().to_string(),
        samples: dataset
            .records
            .iter()
            .map(|record| PsionicLegalSftSample {
                sample_id: record.sample_id.clone(),
                legal_training_record_id: format!("legal-harvey-public-three.{}", record.task_id),
                final_hidden_state: record.hidden_state.clone(),
                target_token_id: record.target_token_id,
                source_token_count: record.source_token_count,
            })
            .collect(),
    }
}

fn run_dpo_stage(
    config: &Qwen36LegalFtMilestoneConfig,
    sft_artifacts: &PsionicLegalSftRunArtifacts,
) -> Result<PsionicLegalDpoRunArtifacts, Qwen36LegalFtMilestoneError> {
    let mut dpo = default_qwen36_legal_dpo_smoke_config(
        config.output_dir.join("dpo").to_string_lossy().to_string(),
    );
    dpo.run_id = String::from("qwen36-27b-legal-ft-001-dpo");
    dpo.base_served_artifact_digest =
        format!("sha256:{}", sha256_hex(QWEN36_27B_MODEL_ID.as_bytes()));
    dpo.parent_sft_adapter_path = sft_artifacts.adapter_artifact_path.clone();
    dpo.parent_sft_receipt_path = sft_artifacts.receipt_path.clone();
    dpo.parent_sft_adapter_id = String::from(SFT_CANDIDATE_ID);
    dpo.parent_sft_adapter_revision = String::from("r1");
    dpo.parent_sft_config_path = None;
    dpo.bootstrap_parent_sft_if_missing = false;
    dpo.validator_policy_ref =
        String::from("policy://validator/legal-benchmark/qwen36-27b-ft-001-dpo");
    dpo.adapter_id = String::from(DPO_CANDIDATE_ID);
    dpo.adapter_revision = String::from("r1");
    dpo.started_at_ms = 20_000;
    Ok(run_psionic_legal_dpo_config(&dpo)?)
}

fn run_grpo_stage(
    config: &Qwen36LegalFtMilestoneConfig,
    sft_artifacts: &PsionicLegalSftRunArtifacts,
) -> Result<PsionicLegalGrpoRunArtifacts, Qwen36LegalFtMilestoneError> {
    let mut grpo = default_qwen36_legal_grpo_smoke_config(
        config.output_dir.join("grpo").to_string_lossy().to_string(),
    );
    grpo.run_id = String::from("qwen36-27b-legal-ft-001-grpo");
    grpo.base_served_artifact_digest =
        format!("sha256:{}", sha256_hex(QWEN36_27B_MODEL_ID.as_bytes()));
    grpo.parent_sft_adapter_path = sft_artifacts.adapter_artifact_path.clone();
    grpo.parent_sft_receipt_path = sft_artifacts.receipt_path.clone();
    grpo.parent_sft_adapter_id = String::from(SFT_CANDIDATE_ID);
    grpo.parent_sft_adapter_revision = String::from("r1");
    grpo.parent_sft_config_path = None;
    grpo.bootstrap_parent_sft_if_missing = false;
    grpo.validator_policy_ref =
        String::from("policy://validator/legal-benchmark/qwen36-27b-ft-001-grpo");
    grpo.adapter_id = String::from(GRPO_CANDIDATE_ID);
    grpo.adapter_revision = String::from("r1");
    grpo.started_at_ms = 30_000;
    Ok(run_psionic_legal_grpo_config(&grpo)?)
}

fn base_candidate_report(
    config: &Qwen36LegalFtMilestoneConfig,
    report: &LegalBenchmarkEvalSuiteModelReport,
    eval_report_hash: &str,
) -> Qwen36LegalFtCandidateReport {
    model_candidate_report(
        BASE_CANDIDATE_ID,
        Qwen36LegalFtCandidateStage::Base,
        None,
        None,
        None,
        None,
        config.output_dir.join("eval").join("base"),
        eval_report_hash.to_owned(),
        report,
        false,
        vec![String::from(
            "Base Qwen3.6-27B target-path eval before legal adapter training.",
        )],
    )
}

#[allow(clippy::too_many_arguments)]
fn candidate_report_from_eval(
    candidate_id: &str,
    stage: Qwen36LegalFtCandidateStage,
    config: &Qwen36LegalFtMilestoneConfig,
    adapter_path: &str,
    receipt_path: &str,
    receipt_digest: &str,
    python_invoked: bool,
    note: &str,
) -> Result<Qwen36LegalFtCandidateReport, Qwen36LegalFtMilestoneError> {
    let output_dir = config
        .output_dir
        .join("eval")
        .join(sanitize_path(candidate_id));
    let eval = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
        suite_path: config.suite_path.clone(),
        base_model: config.champion_model_id.clone(),
        adapter: adapter_path.to_owned(),
        output_dir: output_dir.clone(),
        replay_command: vec![
            String::from("cargo"),
            String::from("run"),
            String::from("-p"),
            String::from("psionic-train"),
            String::from("--example"),
            String::from("qwen36_27b_legal_ft_milestone"),
        ],
    })?;
    Ok(model_candidate_report(
        candidate_id,
        stage,
        Some(adapter_path.to_owned()),
        Some(sha256_file(Path::new(adapter_path))?),
        Some(receipt_path.to_owned()),
        Some(receipt_digest.to_owned()),
        output_dir,
        eval.replay_receipt.report_hash,
        &eval.adapter_result,
        python_invoked,
        vec![note.to_owned()],
    ))
}

#[allow(clippy::too_many_arguments)]
fn model_candidate_report(
    candidate_id: &str,
    stage: Qwen36LegalFtCandidateStage,
    adapter_artifact_path: Option<String>,
    adapter_artifact_sha256: Option<String>,
    training_receipt_path: Option<String>,
    training_receipt_digest: Option<String>,
    eval_output_dir: PathBuf,
    eval_report_hash: String,
    report: &LegalBenchmarkEvalSuiteModelReport,
    python_invoked: bool,
    candidate_notes: Vec<String>,
) -> Qwen36LegalFtCandidateReport {
    let hard_failure_count =
        report.integrity_failure_count + report.tool_failure_count + report.timeout_failure_count;
    Qwen36LegalFtCandidateReport {
        candidate_id: candidate_id.to_owned(),
        stage,
        adapter_artifact_path,
        adapter_artifact_sha256,
        training_receipt_path,
        training_receipt_digest,
        eval_output_dir: eval_output_dir.to_string_lossy().to_string(),
        eval_report_hash,
        legal_score_bps: report.legal_score_bps,
        answer_file_success_rate_bps: report.answer_file_success_rate_bps,
        integrity_failure_count: report.integrity_failure_count,
        tool_failure_count: report.tool_failure_count,
        timeout_failure_count: report.timeout_failure_count,
        python_invoked,
        hard_failure_count,
        candidate_notes,
    }
}

fn promotion_receipt(
    run_id: &str,
    candidates: &[Qwen36LegalFtCandidateReport],
) -> Result<Qwen36LegalFtPromotionReceipt, Qwen36LegalFtMilestoneError> {
    let champion = candidates
        .iter()
        .find(|candidate| candidate.candidate_id == BASE_CANDIDATE_ID)
        .ok_or_else(|| {
            Qwen36LegalFtMilestoneError::InvalidMilestone(String::from(
                "base candidate is missing from ladder",
            ))
        })?;
    let winner = candidates
        .iter()
        .filter(|candidate| candidate.candidate_id != BASE_CANDIDATE_ID)
        .filter(|candidate| candidate.hard_failure_count == 0)
        .max_by(|left, right| {
            left.legal_score_bps
                .cmp(&right.legal_score_bps)
                .then(left.stage.cmp(&right.stage))
        })
        .ok_or_else(|| {
            Qwen36LegalFtMilestoneError::InvalidMilestone(String::from(
                "no valid non-base candidate was available for promotion",
            ))
        })?;
    let score_delta_bps = i32::try_from(winner.legal_score_bps).unwrap_or(i32::MAX)
        - i32::try_from(champion.legal_score_bps).unwrap_or(i32::MAX);
    let mut reasons = Vec::new();
    let decision = if winner.hard_failure_count > 0 {
        reasons.push(String::from(
            "winner candidate had integrity, tool, or timeout failures",
        ));
        Qwen36LegalFtPromotionDecision::Reject
    } else if winner.legal_score_bps <= champion.legal_score_bps {
        reasons.push(String::from(
            "best candidate did not beat the base/champion score",
        ));
        Qwen36LegalFtPromotionDecision::Hold
    } else {
        reasons.push(format!(
            "{} beats {} by {} bps on the same public suite",
            winner.candidate_id, champion.candidate_id, score_delta_bps
        ));
        reasons.push(String::from(
            "stage tie-break prefers the latest successful optimization stage when scores tie",
        ));
        Qwen36LegalFtPromotionDecision::Promote
    };
    let mut receipt = Qwen36LegalFtPromotionReceipt {
        schema_version: String::from(QWEN36_27B_LEGAL_FT_PROMOTION_SCHEMA_VERSION),
        run_id: run_id.to_owned(),
        champion_candidate_id: champion.candidate_id.clone(),
        promoted_candidate_id: winner.candidate_id.clone(),
        champion_score_bps: champion.legal_score_bps,
        promoted_score_bps: winner.legal_score_bps,
        score_delta_bps,
        decision,
        reasons,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest()?;
    Ok(receipt)
}

fn receipt_paths_present(candidates: &[Qwen36LegalFtCandidateReport]) -> bool {
    candidates.iter().all(|candidate| {
        candidate
            .training_receipt_path
            .as_ref()
            .map(|path| Path::new(path).is_file())
            .unwrap_or(true)
    })
}

fn sft_sample(task: &psionic_eval::LegalBenchmarkEvalTaskFixture) -> PsionicLegalSftSample {
    let seed = format!("{}|{}", task.task_id, task.replay_answer_markdown);
    let hash = Sha256::digest(seed.as_bytes());
    let mut hidden = vec![
        f32::from(hash[0]) / 255.0,
        f32::from(hash[1]) / 255.0,
        f32::from(hash[2]) / 255.0,
        f32::from(hash[3]) / 255.0,
    ];
    if hidden.iter().all(|value| *value == 0.0) {
        hidden[0] = 1.0;
    }
    PsionicLegalSftSample {
        sample_id: format!("{}.qwen36-27b.ft.sft", task.task_id),
        legal_training_record_id: format!("legal-harvey-public-three.{}", task.task_id),
        final_hidden_state: hidden,
        target_token_id: u32::from(hash[4] % 240) + 8,
        source_token_count: u32::try_from(
            task.instructions.split_whitespace().count()
                + task.replay_answer_markdown.split_whitespace().count(),
        )
        .unwrap_or(u32::MAX),
    }
}

fn write_markdown_report(
    path: &Path,
    report: &Qwen36LegalFtMilestoneReport,
) -> Result<(), Qwen36LegalFtMilestoneError> {
    let mut markdown = String::new();
    markdown.push_str("# Qwen3.6-27B Legal Fine-Tuning Milestone 001\n\n");
    markdown.push_str("## Status\n\n");
    markdown.push_str(&format!(
        "- model: `{}`\n- model load verified: `{}`\n- base score: `{}` bps\n- promoted candidate: `{}`\n- promoted score: `{}` bps\n- score delta: `{}` bps\n- decision: `{:?}`\n- no Python invoked: `{}`\n- private benchmark tasks used for training: `{}`\n- all receipts present: `{}`\n\n",
        report.model_id,
        report.model_load_verified,
        report.promotion_receipt.champion_score_bps,
        report.promoted_candidate_id,
        report.promotion_receipt.promoted_score_bps,
        report.promotion_receipt.score_delta_bps,
        report.promotion_receipt.decision,
        report.no_python_invoked,
        report.hidden_benchmark_training,
        report.all_receipts_present
    ));
    markdown.push_str("## Candidate Ladder\n\n");
    markdown.push_str("| candidate | stage | score bps | answer-file bps | hard failures | python | adapter sha256 |\n");
    markdown.push_str("| --- | --- | ---: | ---: | ---: | --- | --- |\n");
    for candidate in &report.candidate_ladder {
        markdown.push_str(&format!(
            "| `{}` | `{:?}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            candidate.candidate_id,
            candidate.stage,
            candidate.legal_score_bps,
            candidate.answer_file_success_rate_bps,
            candidate.hard_failure_count,
            candidate.python_invoked,
            candidate
                .adapter_artifact_sha256
                .clone()
                .unwrap_or_else(|| String::from("none"))
        ));
    }
    markdown.push_str("\n## Target Artifacts\n\n");
    markdown.push_str(&format!(
        "- target load report: `{}`\n- target load report sha256: `{}`\n- base eval report hash: `{}`\n\n",
        report.target_load_report_path,
        report.target_load_report_sha256,
        report.base_eval_report_hash
    ));
    markdown.push_str("| role | path | sha256 | bytes |\n");
    markdown.push_str("| --- | --- | --- | ---: |\n");
    for artifact in &report.base_artifacts {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            artifact.role, artifact.path, artifact.sha256, artifact.byte_len
        ));
    }
    markdown.push_str("\n## Data And Training\n\n");
    markdown.push_str(&format!(
        "- suite: `{}`\n- suite hash: `{}`\n- SFT dataset: `{}`\n- SFT dataset sha256: `{}`\n- SFT dataset receipt: `{}`\n- SFT config: `{}`\n- SFT config sha256: `{}`\n\n",
        report.suite_path,
        report.suite_hash,
        report.dataset_receipt.dataset_path,
        report.dataset_receipt.dataset_sha256,
        report.dataset_receipt.receipt_digest,
        report.sft_config_path,
        report.sft_config_sha256
    ));
    markdown.push_str("## Promotion\n\n");
    markdown.push_str(&format!(
        "- receipt: `{}`\n- receipt digest: `{}`\n",
        report.promotion_receipt_path, report.promotion_receipt.receipt_digest
    ));
    for reason in &report.promotion_receipt.reasons {
        markdown.push_str(&format!("- reason: {reason}\n"));
    }
    markdown.push_str("\n## Boundary\n\n");
    markdown.push_str(&format!("{}\n\n", report.claim_boundary));
    markdown.push_str("## Report Receipt\n\n");
    markdown.push_str(&format!("- report digest: `{}`\n", report.report_digest));
    fs::write(path, markdown).map_err(|source| Qwen36LegalFtMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_dir(path: &Path) -> Result<(), Qwen36LegalFtMilestoneError> {
    fs::create_dir_all(path).map_err(|source| Qwen36LegalFtMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>, Qwen36LegalFtMilestoneError> {
    fs::read(path).map_err(|source| Qwen36LegalFtMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), Qwen36LegalFtMilestoneError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| Qwen36LegalFtMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String, Qwen36LegalFtMilestoneError> {
    Ok(sha256_hex(read_file(path)?.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sanitize_path(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_prefers_latest_valid_stage_on_score_tie() {
        let mut candidates = Vec::new();
        candidates.push(candidate(
            BASE_CANDIDATE_ID,
            Qwen36LegalFtCandidateStage::Base,
            3333,
        ));
        candidates.push(candidate(
            SFT_CANDIDATE_ID,
            Qwen36LegalFtCandidateStage::Sft,
            10000,
        ));
        candidates.push(candidate(
            DPO_CANDIDATE_ID,
            Qwen36LegalFtCandidateStage::Dpo,
            10000,
        ));
        candidates.push(candidate(
            GRPO_CANDIDATE_ID,
            Qwen36LegalFtCandidateStage::Grpo,
            10000,
        ));
        let receipt = promotion_receipt("test", candidates.as_slice()).expect("promotion receipt");
        assert_eq!(receipt.decision, Qwen36LegalFtPromotionDecision::Promote);
        assert_eq!(receipt.promoted_candidate_id, GRPO_CANDIDATE_ID);
        assert_eq!(receipt.score_delta_bps, 6667);
    }

    fn candidate(
        id: &str,
        stage: Qwen36LegalFtCandidateStage,
        score: u32,
    ) -> Qwen36LegalFtCandidateReport {
        Qwen36LegalFtCandidateReport {
            candidate_id: id.to_owned(),
            stage,
            adapter_artifact_path: None,
            adapter_artifact_sha256: None,
            training_receipt_path: None,
            training_receipt_digest: None,
            eval_output_dir: String::from("target/test"),
            eval_report_hash: sha256_hex(id.as_bytes()),
            legal_score_bps: score,
            answer_file_success_rate_bps: score,
            integrity_failure_count: 0,
            tool_failure_count: 0,
            timeout_failure_count: 0,
            python_invoked: false,
            hard_failure_count: 0,
            candidate_notes: Vec::new(),
        }
    }
}
