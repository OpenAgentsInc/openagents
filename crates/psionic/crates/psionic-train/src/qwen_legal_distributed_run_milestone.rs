//! First two-worker local Pylon/Psionic legal fine-tuning milestone.
//!
//! This runner keeps the loop honest and small: it uses only the public
//! three-task Harvey suite, shards the SFT data across two local Pylon worker
//! identities, trains Rust LoRA adapters for each shard, verifies signed worker
//! receipts, settles the local payment decisions, merges the adapters with the
//! existing Rust merge path, evaluates the merged adapter, and writes a plain
//! report.

use std::fs;
use std::path::{Path, PathBuf};

use psionic_eval::{
    stable_json_digest, LegalBenchmarkEvalMode, LegalBenchmarkEvalReplayOutcome,
    LegalBenchmarkEvalSuiteManifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    pylon_bitcoin_payout_target_for_job, run_psionic_legal_sft_config,
    run_qwen_legal_lora_merge_manifest, run_qwen_legal_pylon_worker_job,
    settle_qwen_legal_pylon_training_job_spec, verify_qwen_legal_pylon_worker_receipt_path,
    PsionicLegalSftBaseArtifactMode, PsionicLegalSftConfig, PsionicLegalSftError,
    PsionicLegalSftRunArtifacts, PsionicLegalSftSample, PylonLocalWorkerRunOptions,
    PylonTrainingArtifactRef, PylonTrainingExpectedOutputArtifact,
    PylonTrainingHardwareRequirements, PylonTrainingJobKind, PylonTrainingJobSpec,
    PylonTrainingPaymentBudget, PylonTrainingPaymentDecisionReceipt, PylonTrainingPaymentStatus,
    PylonTrainingReceiptRequirements, PylonTrainingShardAssignment, PylonTrainingWorkerJobStatus,
    PylonTrainingWorkerReceipt, PylonTrainingWorkerReceiptVerification,
    QwenLegalLoraMergeBaseModel, QwenLegalLoraMergeError, QwenLegalLoraMergeManifest,
    QwenLegalLoraMergeMode, QwenLegalLoraMergeOutput, QwenLegalLoraMergePromotionDecision,
    QwenLegalLoraMergeValidation, QwenLegalLoraWorkerAdapterInput, QwenLegalPylonTrainingJobError,
    QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION, QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION,
};

pub const QWEN_LEGAL_DISTRIBUTED_RUN_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_distributed_run_milestone.v1";
pub const QWEN_LEGAL_DISTRIBUTED_DATASET_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_distributed_sft_dataset.v1";
pub const QWEN_LEGAL_DISTRIBUTED_SHARD_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_distributed_sft_shard.v1";
pub const DEFAULT_QWEN_LEGAL_DISTRIBUTED_RUN_ID: &str = "legal-ft-distributed-run-001";
pub const DEFAULT_QWEN_LEGAL_DISTRIBUTED_SUITE_PATH: &str = "suites/harvey_public_three.json";
pub const DEFAULT_QWEN_LEGAL_DISTRIBUTED_OUTPUT_DIR: &str =
    "target/legal/legal-ft-distributed-run-001";
pub const DEFAULT_QWEN_LEGAL_DISTRIBUTED_REPORT_PATH: &str =
    "reports/legal-ft-distributed-run-001.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QwenLegalDistributedRunMilestoneConfig {
    pub run_id: String,
    pub suite_path: PathBuf,
    pub output_dir: PathBuf,
    pub report_path: PathBuf,
    pub champion_model_id: String,
    pub candidate_base_model_id: String,
}

impl Default for QwenLegalDistributedRunMilestoneConfig {
    fn default() -> Self {
        Self {
            run_id: String::from(DEFAULT_QWEN_LEGAL_DISTRIBUTED_RUN_ID),
            suite_path: PathBuf::from(DEFAULT_QWEN_LEGAL_DISTRIBUTED_SUITE_PATH),
            output_dir: PathBuf::from(DEFAULT_QWEN_LEGAL_DISTRIBUTED_OUTPUT_DIR),
            report_path: PathBuf::from(DEFAULT_QWEN_LEGAL_DISTRIBUTED_REPORT_PATH),
            champion_model_id: String::from("Qwen/Qwen3.6-27B/current-three-task-champion"),
            candidate_base_model_id: String::from("Qwen/Qwen3.6-27B"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalDistributedSftRecord {
    pub sample_id: String,
    pub task_id: String,
    pub required_answer_path: String,
    pub prompt: String,
    pub answer_markdown: String,
    pub answer_sha256: String,
    pub shard_index: u32,
    pub source_token_count: u32,
    pub target_token_id: u32,
    pub hidden_state: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalDistributedDatasetReceipt {
    pub schema_version: String,
    pub dataset_id: String,
    pub dataset_path: String,
    pub dataset_sha256: String,
    pub suite_path: String,
    pub suite_id: String,
    pub suite_hash: String,
    pub suite_mode: LegalBenchmarkEvalMode,
    pub training_allowed: bool,
    pub record_count: usize,
    pub shard_count: usize,
    pub task_ids: Vec<String>,
    pub records: Vec<QwenLegalDistributedSftRecord>,
    pub receipt_digest: String,
}

impl QwenLegalDistributedDatasetReceipt {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(
            "psionic.qwen_legal_distributed_sft_dataset_receipt.v1",
            &clone,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalDistributedShardManifest {
    pub schema_version: String,
    pub shard_id: String,
    pub shard_index: u32,
    pub shard_count: u32,
    pub dataset_id: String,
    pub dataset_sha256: String,
    pub worker_id: String,
    pub task_ids: Vec<String>,
    pub sample_ids: Vec<String>,
    pub source_token_count: u64,
    pub manifest_digest: String,
}

impl QwenLegalDistributedShardManifest {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.manifest_digest.clear();
        stable_json_digest(
            "psionic.qwen_legal_distributed_sft_shard_manifest.v1",
            &clone,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalDistributedShardReport {
    pub shard_id: String,
    pub shard_index: u32,
    pub worker_id: String,
    pub task_ids: Vec<String>,
    pub sample_ids: Vec<String>,
    pub source_token_count: u64,
    pub shard_manifest_path: String,
    pub shard_manifest_sha256: String,
    pub sft_config_path: String,
    pub sft_config_sha256: String,
    pub adapter_artifact_path: String,
    pub adapter_artifact_sha256: String,
    pub training_receipt_path: String,
    pub training_receipt_digest: String,
    pub python_invoked: bool,
    pub worker_job_path: String,
    pub worker_job_digest: String,
    pub worker_receipt_path: String,
    pub worker_receipt_digest: String,
    pub worker_signature_valid: bool,
    pub worker_output_hash_verified: bool,
    pub worker_status: PylonTrainingWorkerJobStatus,
    pub payment_decision_path: String,
    pub payment_decision_digest: String,
    pub payment_status: PylonTrainingPaymentStatus,
    pub payment_proof: Option<String>,
    pub agreed_price_microusd: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalDistributedRunReport {
    pub schema_version: String,
    pub run_id: String,
    pub generated_on: String,
    pub suite_path: String,
    pub suite_id: String,
    pub suite_hash: String,
    pub suite_training_allowed: bool,
    pub hidden_benchmark_training: bool,
    pub dataset_receipt: QwenLegalDistributedDatasetReceipt,
    pub worker_count: usize,
    pub shard_count: usize,
    pub shards: Vec<QwenLegalDistributedShardReport>,
    pub all_workers_signed_receipts: bool,
    pub all_worker_outputs_hash_verified: bool,
    pub all_worker_payments_payable: bool,
    pub payable_total_microusd: u64,
    pub merge_manifest_path: String,
    pub merge_manifest_sha256: String,
    pub merge_receipt_path: String,
    pub merge_receipt_hash: String,
    pub merged_adapter_path: String,
    pub merged_adapter_sha256: String,
    pub eval_output_dir: String,
    pub eval_report_hash: String,
    pub champion_score_bps: u32,
    pub candidate_score_bps: u32,
    pub score_delta_bps: i32,
    pub candidate_answer_file_success_rate_bps: u32,
    pub candidate_integrity_failure_count: u64,
    pub candidate_tool_failure_count: u64,
    pub candidate_timeout_failure_count: u64,
    pub promotion_decision: QwenLegalLoraMergePromotionDecision,
    pub promotion_reasons: Vec<String>,
    pub no_python_in_worker_path: bool,
    pub all_artifacts_have_receipts: bool,
    pub claim_boundary: String,
    pub report_path: String,
    pub report_digest: String,
}

impl QwenLegalDistributedRunReport {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest("psionic.qwen_legal_distributed_run_report.v1", &clone)
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalDistributedRunMilestoneError {
    #[error("distributed legal run I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("distributed legal run JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("legal SFT failed: {0}")]
    Sft(#[from] PsionicLegalSftError),
    #[error("Pylon worker job failed: {0}")]
    Pylon(#[from] QwenLegalPylonTrainingJobError),
    #[error("LoRA merge failed: {0}")]
    Merge(#[from] QwenLegalLoraMergeError),
    #[error("invalid distributed legal run: {0}")]
    InvalidRun(String),
}

pub fn run_qwen_legal_distributed_run_milestone(
    config: &QwenLegalDistributedRunMilestoneConfig,
) -> Result<QwenLegalDistributedRunReport, QwenLegalDistributedRunMilestoneError> {
    create_dir(config.output_dir.as_path())?;
    if let Some(parent) = config.report_path.parent() {
        create_dir(parent)?;
    }

    let suite_bytes = read_file(config.suite_path.as_path())?;
    let suite_manifest: LegalBenchmarkEvalSuiteManifest = serde_json::from_slice(&suite_bytes)?;
    let suite_hash = stable_json_digest(
        "psionic.legal_benchmark.eval_suite_manifest.v1",
        &suite_manifest,
    )?;
    if !suite_manifest.training_allowed {
        return Err(QwenLegalDistributedRunMilestoneError::InvalidRun(
            "the distributed milestone only trains on public training-allowed suites".to_string(),
        ));
    }
    if suite_manifest.mode == LegalBenchmarkEvalMode::HiddenAuditOnly {
        return Err(QwenLegalDistributedRunMilestoneError::InvalidRun(
            "hidden benchmark suites cannot be used as distributed training data".to_string(),
        ));
    }
    if suite_manifest.fixed_task_order.len() < 2 {
        return Err(QwenLegalDistributedRunMilestoneError::InvalidRun(
            "at least two tasks are required to exercise two worker shards".to_string(),
        ));
    }

    let dataset_dir = config.output_dir.join("dataset");
    create_dir(dataset_dir.as_path())?;
    let dataset = write_distributed_dataset(
        &suite_manifest,
        config.suite_path.as_path(),
        suite_hash.as_str(),
        dataset_dir.as_path(),
        2,
    )?;

    let shard_plans = build_shard_plans(&dataset)?;
    let mut shard_reports = Vec::new();
    for shard in &shard_plans {
        shard_reports.push(run_worker_shard(config, &dataset, shard)?);
    }

    let merge_dir = config.output_dir.join("merge");
    create_dir(merge_dir.as_path())?;
    let merge_manifest_path = merge_dir.join("merge_manifest.json");
    let champion_score_bps = expected_suite_score_bps(&suite_manifest, false);
    let merge_manifest = merge_manifest(
        config,
        shard_reports.as_slice(),
        merge_dir.as_path(),
        champion_score_bps,
    );
    write_json(merge_manifest_path.as_path(), &merge_manifest)?;
    let merge_manifest_sha256 = sha256_file(merge_manifest_path.as_path())?;
    let merge_receipt = run_qwen_legal_lora_merge_manifest(merge_manifest_path.as_path())?;
    let merge_receipt_path =
        PathBuf::from(&merge_receipt.output_adapter_path).with_extension("merge-receipt.json");

    let validation = merge_receipt.validation.as_ref().ok_or_else(|| {
        QwenLegalDistributedRunMilestoneError::InvalidRun(
            "merge receipt did not include validation".to_string(),
        )
    })?;
    let promotion_gate = merge_receipt.promotion_gate.as_ref().ok_or_else(|| {
        QwenLegalDistributedRunMilestoneError::InvalidRun(
            "merge receipt did not include promotion gate".to_string(),
        )
    })?;

    let all_workers_signed_receipts = shard_reports
        .iter()
        .all(|shard| shard.worker_signature_valid && shard.worker_receipt_digest.len() == 64);
    let all_worker_outputs_hash_verified = shard_reports.iter().all(|shard| {
        shard.worker_output_hash_verified
            && shard.worker_status == PylonTrainingWorkerJobStatus::Succeeded
    });
    let all_worker_payments_payable = shard_reports
        .iter()
        .all(|shard| shard.payment_status == PylonTrainingPaymentStatus::Payable);
    let payable_total_microusd = shard_reports
        .iter()
        .filter(|shard| shard.payment_status == PylonTrainingPaymentStatus::Payable)
        .map(|shard| shard.agreed_price_microusd)
        .sum();
    let no_python_in_worker_path = shard_reports.iter().all(|shard| !shard.python_invoked);
    let all_artifacts_have_receipts = all_workers_signed_receipts
        && all_worker_outputs_hash_verified
        && !merge_receipt.receipt_hash.is_empty()
        && !validation.report_hash.is_empty();

    let mut report = QwenLegalDistributedRunReport {
        schema_version: String::from(QWEN_LEGAL_DISTRIBUTED_RUN_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        generated_on: String::from("2026-05-20"),
        suite_path: config.suite_path.to_string_lossy().to_string(),
        suite_id: suite_manifest.suite_id.clone(),
        suite_hash,
        suite_training_allowed: suite_manifest.training_allowed,
        hidden_benchmark_training: false,
        dataset_receipt: dataset,
        worker_count: shard_reports.len(),
        shard_count: shard_reports.len(),
        shards: shard_reports,
        all_workers_signed_receipts,
        all_worker_outputs_hash_verified,
        all_worker_payments_payable,
        payable_total_microusd,
        merge_manifest_path: merge_manifest_path.to_string_lossy().to_string(),
        merge_manifest_sha256,
        merge_receipt_path: merge_receipt_path.to_string_lossy().to_string(),
        merge_receipt_hash: merge_receipt.receipt_hash.clone(),
        merged_adapter_path: merge_receipt.output_adapter_path.clone(),
        merged_adapter_sha256: merge_receipt.output_adapter_sha256.clone(),
        eval_output_dir: validation.output_dir.clone(),
        eval_report_hash: validation.report_hash.clone(),
        champion_score_bps: promotion_gate.champion_score_bps,
        candidate_score_bps: promotion_gate.candidate_score_bps,
        score_delta_bps: promotion_gate.score_delta_bps,
        candidate_answer_file_success_rate_bps: validation.answer_file_success_rate_bps,
        candidate_integrity_failure_count: validation.integrity_failure_count,
        candidate_tool_failure_count: validation.tool_failure_count,
        candidate_timeout_failure_count: validation.timeout_failure_count,
        promotion_decision: promotion_gate.decision,
        promotion_reasons: promotion_gate.reasons.clone(),
        no_python_in_worker_path,
        all_artifacts_have_receipts,
        claim_boundary: String::from(
            "This is a local two-worker Pylon/Psionic legal fine-tuning milestone over the public training-allowed Harvey three-task fixture. It proves Rust sharding, local worker SFT, signed receipts, payment decisions, adapter merge, and replay eval. It does not prove performance on private Harvey benchmark tasks or remote tailnet worker execution.",
        ),
        report_path: config.report_path.to_string_lossy().to_string(),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest()?;
    write_json(
        config
            .output_dir
            .join("distributed_run_report.json")
            .as_path(),
        &report,
    )?;
    write_markdown_report(config.report_path.as_path(), &report)?;
    Ok(report)
}

#[derive(Clone, Debug)]
struct ShardPlan {
    worker_id: String,
    shard_id: String,
    shard_index: u32,
    shard_count: u32,
    records: Vec<QwenLegalDistributedSftRecord>,
}

fn write_distributed_dataset(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
    suite_hash: &str,
    output_dir: &Path,
    shard_count: u32,
) -> Result<QwenLegalDistributedDatasetReceipt, QwenLegalDistributedRunMilestoneError> {
    let dataset_path = output_dir.join("legal-public-three-distributed-sft.jsonl");
    let mut records = Vec::new();
    let mut jsonl = String::new();
    for (index, task_id) in manifest.fixed_task_order.iter().enumerate() {
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == *task_id)
            .ok_or_else(|| {
                QwenLegalDistributedRunMilestoneError::InvalidRun(format!(
                    "fixed task `{task_id}` missing from suite"
                ))
            })?;
        let sample = sft_sample(task);
        let shard_index = u32::try_from(index).unwrap_or_default() % shard_count;
        let record = QwenLegalDistributedSftRecord {
            sample_id: sample.sample_id.clone(),
            task_id: task.task_id.clone(),
            required_answer_path: task.required_answer_path.clone(),
            prompt: format!(
                "{}\n\nRequired answer path: {}",
                task.instructions, task.required_answer_path
            ),
            answer_markdown: task.replay_answer_markdown.clone(),
            answer_sha256: sha256_hex(task.replay_answer_markdown.as_bytes()),
            shard_index,
            source_token_count: sample.source_token_count,
            target_token_id: sample.target_token_id,
            hidden_state: sample.final_hidden_state,
        };
        jsonl.push_str(&serde_json::to_string(&record)?);
        jsonl.push('\n');
        records.push(record);
    }
    fs::write(&dataset_path, jsonl).map_err(|source| {
        QwenLegalDistributedRunMilestoneError::Io {
            path: dataset_path.clone(),
            source,
        }
    })?;
    let dataset_sha256 = sha256_file(dataset_path.as_path())?;
    let mut receipt = QwenLegalDistributedDatasetReceipt {
        schema_version: String::from(QWEN_LEGAL_DISTRIBUTED_DATASET_SCHEMA_VERSION),
        dataset_id: String::from("dataset.qwen-legal.harvey-public-three.distributed-001"),
        dataset_path: dataset_path.to_string_lossy().to_string(),
        dataset_sha256,
        suite_path: suite_path.to_string_lossy().to_string(),
        suite_id: manifest.suite_id.clone(),
        suite_hash: suite_hash.to_owned(),
        suite_mode: manifest.mode,
        training_allowed: manifest.training_allowed,
        record_count: records.len(),
        shard_count: usize::try_from(shard_count).unwrap_or_default(),
        task_ids: manifest.fixed_task_order.clone(),
        records,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest()?;
    write_json(
        output_dir
            .join("distributed_dataset_receipt.json")
            .as_path(),
        &receipt,
    )?;
    Ok(receipt)
}

fn build_shard_plans(
    dataset: &QwenLegalDistributedDatasetReceipt,
) -> Result<Vec<ShardPlan>, QwenLegalDistributedRunMilestoneError> {
    let workers = ["pylon.local.harvey-legal.01", "pylon.local.harvey-legal.02"];
    let mut plans = Vec::new();
    for (index, worker_id) in workers.iter().enumerate() {
        let shard_index = u32::try_from(index).unwrap_or_default();
        let records = dataset
            .records
            .iter()
            .filter(|record| record.shard_index == shard_index)
            .cloned()
            .collect::<Vec<_>>();
        if records.is_empty() {
            return Err(QwenLegalDistributedRunMilestoneError::InvalidRun(format!(
                "worker shard {shard_index} has no records"
            )));
        }
        plans.push(ShardPlan {
            worker_id: String::from(*worker_id),
            shard_id: format!("shard.{}.{}", dataset.dataset_id, index + 1),
            shard_index,
            shard_count: 2,
            records,
        });
    }
    Ok(plans)
}

fn run_worker_shard(
    config: &QwenLegalDistributedRunMilestoneConfig,
    dataset: &QwenLegalDistributedDatasetReceipt,
    shard: &ShardPlan,
) -> Result<QwenLegalDistributedShardReport, QwenLegalDistributedRunMilestoneError> {
    let shard_dir = config
        .output_dir
        .join("workers")
        .join(sanitize_path(shard.worker_id.as_str()));
    create_dir(shard_dir.as_path())?;
    let shard_manifest = shard_manifest(dataset, shard)?;
    let shard_manifest_path = shard_dir.join("shard_manifest.json");
    write_json(shard_manifest_path.as_path(), &shard_manifest)?;
    let shard_manifest_sha256 = sha256_file(shard_manifest_path.as_path())?;

    let sft_config = shard_sft_config(config, dataset, shard, shard_dir.as_path());
    let sft_config_path = shard_dir.join("sft_config.json");
    write_json(sft_config_path.as_path(), &sft_config)?;
    let sft_config_sha256 = sha256_file(sft_config_path.as_path())?;
    let sft_artifacts = run_psionic_legal_sft_config(&sft_config)?;
    let adapter_sha256 = sha256_file(Path::new(&sft_artifacts.adapter_artifact_path))?;

    let worker_job = pylon_worker_job(
        config,
        dataset,
        shard,
        shard_dir.as_path(),
        &sft_artifacts,
        adapter_sha256.as_str(),
        sft_config_path.as_path(),
        sft_config_sha256.as_str(),
        shard_manifest_path.as_path(),
        shard_manifest_sha256.as_str(),
    )?;
    let worker_job_path = shard_dir.join("pylon_job.json");
    write_json(worker_job_path.as_path(), &worker_job)?;
    let worker_job_digest = worker_job.stable_digest();
    let worker_receipt = run_qwen_legal_pylon_worker_job(
        &worker_job,
        &PylonLocalWorkerRunOptions {
            worker_id: shard.worker_id.clone(),
            started_at_ms: 20_000 + u64::from(shard.shard_index) * 1_000,
            emit_outputs: false,
        },
    )?;
    let verification = verify_qwen_legal_pylon_worker_receipt_path(&worker_job.receipt_path)?;
    let payment = settle_qwen_legal_pylon_training_job_spec(&worker_job)?;
    Ok(shard_report(
        shard,
        shard_manifest_path,
        shard_manifest_sha256,
        sft_config_path,
        sft_config_sha256,
        sft_artifacts,
        adapter_sha256,
        worker_job_path,
        worker_job_digest,
        worker_receipt,
        verification,
        payment,
    ))
}

fn shard_manifest(
    dataset: &QwenLegalDistributedDatasetReceipt,
    shard: &ShardPlan,
) -> Result<QwenLegalDistributedShardManifest, QwenLegalDistributedRunMilestoneError> {
    let mut manifest = QwenLegalDistributedShardManifest {
        schema_version: String::from(QWEN_LEGAL_DISTRIBUTED_SHARD_SCHEMA_VERSION),
        shard_id: shard.shard_id.clone(),
        shard_index: shard.shard_index,
        shard_count: shard.shard_count,
        dataset_id: dataset.dataset_id.clone(),
        dataset_sha256: dataset.dataset_sha256.clone(),
        worker_id: shard.worker_id.clone(),
        task_ids: shard
            .records
            .iter()
            .map(|record| record.task_id.clone())
            .collect(),
        sample_ids: shard
            .records
            .iter()
            .map(|record| record.sample_id.clone())
            .collect(),
        source_token_count: shard_source_token_count(shard),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = manifest.stable_digest()?;
    Ok(manifest)
}

fn shard_sft_config(
    config: &QwenLegalDistributedRunMilestoneConfig,
    dataset: &QwenLegalDistributedDatasetReceipt,
    shard: &ShardPlan,
    shard_dir: &Path,
) -> PsionicLegalSftConfig {
    PsionicLegalSftConfig {
        schema_version: String::from(crate::PSIONIC_LEGAL_SFT_CONFIG_SCHEMA_VERSION),
        run_id: format!("{}-shard-{}", config.run_id, shard.shard_index + 1),
        train_type: String::from("qlora"),
        base_model: config.candidate_base_model_id.clone(),
        served_model_id: String::from("qwen3.6-27b"),
        base_model_revision: String::from("qwen3.6-27b-distributed-legal-smoke"),
        base_artifact_mode: PsionicLegalSftBaseArtifactMode::SyntheticHiddenStateSmoke,
        base_served_artifact_digest: format!(
            "sha256:{}",
            sha256_hex(config.candidate_base_model_id.as_bytes())
        ),
        base_safetensors_paths: Vec::new(),
        model_config_path: Some(String::from("fixtures/qwen36_27b_smoke/config.json")),
        tokenizer_path: Some(String::from("fixtures/qwen36_27b_smoke/tokenizer.json")),
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
        batch_size: 1,
        assistant_only_loss: true,
        ignore_empty_think_loss: true,
        gradient_clip_norm: Some(1.0),
        started_at_ms: 30_000 + u64::from(shard.shard_index) * 1_000,
        step_duration_ms: 20,
        dataset_ref: format!("{}#{}", dataset.dataset_id, dataset.dataset_sha256),
        validator_policy_ref: String::from(
            "policy://validator/legal-benchmark/public-three-distributed-001",
        ),
        adapter_id: format!(
            "qwen36-27b-legal-distributed-run-001-shard-{}",
            shard.shard_index + 1
        ),
        adapter_revision: String::from("r1"),
        output_dir: shard_dir.join("sft").to_string_lossy().to_string(),
        samples: shard
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

#[allow(clippy::too_many_arguments)]
fn pylon_worker_job(
    config: &QwenLegalDistributedRunMilestoneConfig,
    dataset: &QwenLegalDistributedDatasetReceipt,
    shard: &ShardPlan,
    shard_dir: &Path,
    sft_artifacts: &PsionicLegalSftRunArtifacts,
    _adapter_sha256: &str,
    sft_config_path: &Path,
    sft_config_sha256: &str,
    shard_manifest_path: &Path,
    shard_manifest_sha256: &str,
) -> Result<PylonTrainingJobSpec, QwenLegalDistributedRunMilestoneError> {
    let job_id = format!("job.{}.sft-shard-{}", config.run_id, shard.shard_index + 1);
    Ok(PylonTrainingJobSpec {
        schema_version: String::from(QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION),
        job_id: job_id.clone(),
        parent_run_id: config.run_id.clone(),
        job_kind: PylonTrainingJobKind::SftTrainShard,
        model_id: config.candidate_base_model_id.clone(),
        model_hash: sha256_hex(config.candidate_base_model_id.as_bytes()),
        adapter_id: None,
        adapter_hash: None,
        dataset_manifest_hash: dataset.dataset_sha256.clone(),
        shard_assignment: PylonTrainingShardAssignment {
            assignment_id: format!("assignment.{}.{}", config.run_id, shard.shard_index + 1),
            shard_id: shard.shard_id.clone(),
            shard_index: shard.shard_index,
            shard_count: shard.shard_count,
            start_index: Some(u64::from(shard.shard_index)),
            end_index: Some(u64::from(shard.shard_index) + 1),
        },
        training_config_hash: sft_config_sha256.to_owned(),
        expected_input_artifacts: vec![
            PylonTrainingArtifactRef {
                artifact_id: String::from("harvey-public-three-suite"),
                artifact_type: String::from("eval_suite"),
                path: config.suite_path.to_string_lossy().to_string(),
                sha256: sha256_file(config.suite_path.as_path())?,
            },
            PylonTrainingArtifactRef {
                artifact_id: String::from("distributed-sft-dataset"),
                artifact_type: String::from("sft_dataset"),
                path: dataset.dataset_path.clone(),
                sha256: dataset.dataset_sha256.clone(),
            },
            PylonTrainingArtifactRef {
                artifact_id: format!("distributed-sft-shard-{}", shard.shard_index + 1),
                artifact_type: String::from("sft_shard_manifest"),
                path: shard_manifest_path.to_string_lossy().to_string(),
                sha256: shard_manifest_sha256.to_owned(),
            },
            PylonTrainingArtifactRef {
                artifact_id: format!("distributed-sft-config-{}", shard.shard_index + 1),
                artifact_type: String::from("training_config"),
                path: sft_config_path.to_string_lossy().to_string(),
                sha256: sft_config_sha256.to_owned(),
            },
        ],
        expected_output_artifacts: vec![PylonTrainingExpectedOutputArtifact {
            artifact_id: format!("artifact.{job_id}.adapter"),
            artifact_type: String::from("legal_sft_adapter"),
            path: sft_artifacts.adapter_artifact_path.clone(),
            required: true,
        }],
        max_runtime_ms: 120_000,
        hardware_requirements: PylonTrainingHardwareRequirements {
            min_memory_bytes: 512 * 1024 * 1024,
            require_accelerator: false,
            accepted_backend_labels: vec![String::from("local_protocol_smoke")],
        },
        payment_budget: PylonTrainingPaymentBudget {
            budget_id: String::from("budget.qwen-legal.distributed-run-001"),
            agreed_price_microusd: 5_000,
            max_cost_microusd: 5_000,
            currency: String::from("USD"),
            payment_account_ref: String::from("ledger://local-smoke/qwen-legal-distributed"),
            bitcoin_payout: pylon_bitcoin_payout_target_for_job(job_id.as_str(), 50_000),
            pay_failed_but_valid_eval_attempts: false,
        },
        receipt_requirements: PylonTrainingReceiptRequirements {
            require_signature: true,
            require_logs_hash: true,
            require_metrics: true,
            required_output_artifact_types: vec![String::from("legal_sft_adapter")],
        },
        output_dir: shard_dir.join("pylon").to_string_lossy().to_string(),
        receipt_path: shard_dir
            .join("pylon")
            .join(format!("{job_id}.receipt.json"))
            .to_string_lossy()
            .to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn shard_report(
    shard: &ShardPlan,
    shard_manifest_path: PathBuf,
    shard_manifest_sha256: String,
    sft_config_path: PathBuf,
    sft_config_sha256: String,
    sft_artifacts: PsionicLegalSftRunArtifacts,
    adapter_artifact_sha256: String,
    worker_job_path: PathBuf,
    worker_job_digest: String,
    worker_receipt: PylonTrainingWorkerReceipt,
    verification: PylonTrainingWorkerReceiptVerification,
    payment: PylonTrainingPaymentDecisionReceipt,
) -> QwenLegalDistributedShardReport {
    QwenLegalDistributedShardReport {
        shard_id: shard.shard_id.clone(),
        shard_index: shard.shard_index,
        worker_id: shard.worker_id.clone(),
        task_ids: shard
            .records
            .iter()
            .map(|record| record.task_id.clone())
            .collect(),
        sample_ids: shard
            .records
            .iter()
            .map(|record| record.sample_id.clone())
            .collect(),
        source_token_count: shard_source_token_count(shard),
        shard_manifest_path: shard_manifest_path.to_string_lossy().to_string(),
        shard_manifest_sha256,
        sft_config_path: sft_config_path.to_string_lossy().to_string(),
        sft_config_sha256,
        adapter_artifact_path: sft_artifacts.adapter_artifact_path.clone(),
        adapter_artifact_sha256,
        training_receipt_path: sft_artifacts.receipt_path.clone(),
        training_receipt_digest: sft_artifacts.receipt.receipt_digest.clone(),
        python_invoked: sft_artifacts.receipt.python_invoked,
        worker_job_path: worker_job_path.to_string_lossy().to_string(),
        worker_job_digest,
        worker_receipt_path: verification.receipt_path,
        worker_receipt_digest: worker_receipt.receipt_digest,
        worker_signature_valid: verification.signature_valid,
        worker_output_hash_verified: verification.output_files_rechecked,
        worker_status: verification.status,
        payment_decision_path: payment.decision_path.clone(),
        payment_decision_digest: payment.decision_digest.clone(),
        payment_status: payment.payment_status,
        payment_proof: payment.payment_proof.clone(),
        agreed_price_microusd: payment.agreed_price_microusd,
    }
}

fn merge_manifest(
    config: &QwenLegalDistributedRunMilestoneConfig,
    shards: &[QwenLegalDistributedShardReport],
    merge_dir: &Path,
    champion_score_bps: u32,
) -> QwenLegalLoraMergeManifest {
    let parent_adapter_sha256 =
        sha256_hex(format!("{}:parent-adapter-zero", config.run_id).as_bytes());
    QwenLegalLoraMergeManifest {
        schema_version: String::from(QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION),
        merge_id: config.run_id.clone(),
        mode: QwenLegalLoraMergeMode::DeltaAveraging,
        parent_adapter_sha256: parent_adapter_sha256.clone(),
        base_model: QwenLegalLoraMergeBaseModel {
            base_model_id: config.candidate_base_model_id.clone(),
            base_model_revision: String::from("qwen3.6-27b-distributed-legal-smoke"),
            base_served_artifact_digest: format!(
                "sha256:{}",
                sha256_hex(config.candidate_base_model_id.as_bytes())
            ),
        },
        output_adapter: QwenLegalLoraMergeOutput {
            adapter_id: String::from("qwen36-27b-legal-distributed-run-001"),
            adapter_revision: String::from("r1-merged"),
            path: merge_dir
                .join("qwen36-27b-legal-distributed-run-001.safetensors")
                .to_string_lossy()
                .to_string(),
            expected_sha256: None,
        },
        worker_adapters: shards
            .iter()
            .map(|shard| QwenLegalLoraWorkerAdapterInput {
                worker_id: shard.worker_id.clone(),
                adapter_id: format!(
                    "qwen36-27b-legal-distributed-run-001-shard-{}",
                    shard.shard_index + 1
                ),
                adapter_revision: String::from("r1"),
                path: shard.adapter_artifact_path.clone(),
                sha256: shard.adapter_artifact_sha256.clone(),
                dataset_shard_hash: shard.shard_manifest_sha256.clone(),
                token_count: shard.source_token_count,
                parent_adapter_sha256: parent_adapter_sha256.clone(),
                compatibility: None,
                validator_replay: None,
            })
            .collect(),
        compatibility: None,
        validation: Some(QwenLegalLoraMergeValidation {
            suite_path: config.suite_path.to_string_lossy().to_string(),
            base_model: config.champion_model_id.clone(),
            output_dir: config.output_dir.join("eval").to_string_lossy().to_string(),
            champion_adapter_id: String::from("qwen36-27b-current-three-task-champion"),
            champion_score_bps,
        }),
    }
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
        sample_id: format!("{}.distributed.sft", task.task_id),
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

fn expected_suite_score_bps(manifest: &LegalBenchmarkEvalSuiteManifest, adapter: bool) -> u32 {
    let task_count = manifest.tasks.len() as u64;
    if task_count == 0 {
        return 0;
    }
    let pass_count = manifest
        .tasks
        .iter()
        .filter(|task| {
            if adapter {
                task.adapter_outcome == LegalBenchmarkEvalReplayOutcome::Pass
            } else {
                task.base_outcome == LegalBenchmarkEvalReplayOutcome::Pass
            }
        })
        .count() as u64;
    u32::try_from((pass_count * 10_000) / task_count).unwrap_or(u32::MAX)
}

fn shard_source_token_count(shard: &ShardPlan) -> u64 {
    shard
        .records
        .iter()
        .map(|record| u64::from(record.source_token_count))
        .sum()
}

fn write_markdown_report(
    path: &Path,
    report: &QwenLegalDistributedRunReport,
) -> Result<(), QwenLegalDistributedRunMilestoneError> {
    let mut markdown = String::new();
    markdown.push_str("# Legal Distributed Fine-Tuning Run 001\n\n");
    markdown.push_str("## Status\n\n");
    markdown.push_str(&format!(
        "- workers: `{}`\n- all worker receipts signed: `{}`\n- all worker outputs hash verified: `{}`\n- all worker payments payable: `{}`\n- champion score: `{}` bps\n- candidate score: `{}` bps\n- delta: `{}` bps\n- promotion decision: `{:?}`\n- no Python in worker path: `{}`\n- private benchmark tasks used for training: `{}`\n\n",
        report.worker_count,
        report.all_workers_signed_receipts,
        report.all_worker_outputs_hash_verified,
        report.all_worker_payments_payable,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.promotion_decision,
        report.no_python_in_worker_path,
        report.hidden_benchmark_training
    ));
    markdown.push_str("## What Ran\n\n");
    markdown.push_str("This run trained two local Pylon worker shards with the Rust Psionic legal SFT trainer. Each worker produced a LoRA adapter, a Psionic training receipt, a signed Pylon worker receipt, and a local payment decision. The aggregator merged the two adapters and evaluated the merged adapter on the same public three-task Harvey suite.\n\n");
    markdown.push_str("## Worker Table\n\n");
    markdown.push_str(
        "| worker | shard | tasks | adapter sha256 | signed | output verified | payment |\n",
    );
    markdown.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for shard in &report.shards {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{:?}` |\n",
            shard.worker_id,
            shard.shard_id,
            shard.task_ids.join(", "),
            shard.adapter_artifact_sha256,
            shard.worker_signature_valid,
            shard.worker_output_hash_verified,
            shard.payment_status
        ));
    }
    markdown.push_str("\n## Shard Table\n\n");
    markdown
        .push_str("| shard | samples | tokens | manifest | training receipt | worker receipt |\n");
    markdown.push_str("| --- | --- | ---: | --- | --- | --- |\n");
    for shard in &report.shards {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            shard.shard_id,
            shard.sample_ids.join(", "),
            shard.source_token_count,
            shard.shard_manifest_sha256,
            shard.training_receipt_digest,
            shard.worker_receipt_digest
        ));
    }
    markdown.push_str("\n## Adapter Merge\n\n");
    markdown.push_str(&format!(
        "- merge manifest: `{}`\n- merge manifest sha256: `{}`\n- merge receipt: `{}`\n- merge receipt hash: `{}`\n- merged adapter: `{}`\n- merged adapter sha256: `{}`\n\n",
        report.merge_manifest_path,
        report.merge_manifest_sha256,
        report.merge_receipt_path,
        report.merge_receipt_hash,
        report.merged_adapter_path,
        report.merged_adapter_sha256
    ));
    markdown.push_str("## Eval Result\n\n");
    markdown.push_str(&format!(
        "- eval output dir: `{}`\n- eval report hash: `{}`\n- candidate answer-file success: `{}` bps\n- candidate integrity failures: `{}`\n- candidate tool failures: `{}`\n- candidate timeouts: `{}`\n\n",
        report.eval_output_dir,
        report.eval_report_hash,
        report.candidate_answer_file_success_rate_bps,
        report.candidate_integrity_failure_count,
        report.candidate_tool_failure_count,
        report.candidate_timeout_failure_count
    ));
    markdown.push_str("## Promotion Decision\n\n");
    markdown.push_str(&format!(
        "- decision: `{:?}`\n- champion score: `{}` bps\n- candidate score: `{}` bps\n- score delta: `{}` bps\n",
        report.promotion_decision,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps
    ));
    for reason in &report.promotion_reasons {
        markdown.push_str(&format!("- reason: {reason}\n"));
    }
    markdown.push_str("\n## Payment And Budget Receipts\n\n");
    markdown.push_str(&format!(
        "- payable total: `{}` micro-USD\n- all worker payments payable: `{}`\n\n",
        report.payable_total_microusd, report.all_worker_payments_payable
    ));
    markdown.push_str("| worker | decision | digest | proof | amount micro-USD |\n");
    markdown.push_str("| --- | --- | --- | --- | ---: |\n");
    for shard in &report.shards {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            shard.worker_id,
            shard.payment_decision_path,
            shard.payment_decision_digest,
            shard
                .payment_proof
                .clone()
                .unwrap_or_else(|| String::from("none")),
            shard.agreed_price_microusd
        ));
    }
    markdown.push_str("\n## Boundary\n\n");
    markdown.push_str(&format!("{}\n\n", report.claim_boundary));
    markdown.push_str("## Report Receipt\n\n");
    markdown.push_str(&format!(
        "- report digest: `{}`\n- all artifacts have receipts: `{}`\n",
        report.report_digest, report.all_artifacts_have_receipts
    ));
    fs::write(path, markdown).map_err(|source| QwenLegalDistributedRunMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_dir(path: &Path) -> Result<(), QwenLegalDistributedRunMilestoneError> {
    fs::create_dir_all(path).map_err(|source| QwenLegalDistributedRunMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>, QwenLegalDistributedRunMilestoneError> {
    fs::read(path).map_err(|source| QwenLegalDistributedRunMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), QwenLegalDistributedRunMilestoneError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| QwenLegalDistributedRunMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String, QwenLegalDistributedRunMilestoneError> {
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
    fn public_records_split_across_two_worker_shards() {
        let task_a = psionic_eval::LegalBenchmarkEvalTaskFixture {
            task_id: String::from("a"),
            task_version: String::from("v1"),
            title: String::from("A"),
            practice_area: String::from("contracts"),
            workflow: String::from("memo"),
            instructions: String::from("Write a short answer."),
            source_document_ids: Vec::new(),
            required_answer_path: String::from("answer.md"),
            base_outcome: LegalBenchmarkEvalReplayOutcome::MissingAnswer,
            adapter_outcome: LegalBenchmarkEvalReplayOutcome::Pass,
            replay_answer_markdown: String::from("# Answer\n\nA.\n"),
        };
        let task_b = psionic_eval::LegalBenchmarkEvalTaskFixture {
            task_id: String::from("b"),
            replay_answer_markdown: String::from("# Answer\n\nB.\n"),
            ..task_a.clone()
        };
        let task_c = psionic_eval::LegalBenchmarkEvalTaskFixture {
            task_id: String::from("c"),
            replay_answer_markdown: String::from("# Answer\n\nC.\n"),
            ..task_a.clone()
        };
        let records = vec![task_a, task_b, task_c]
            .iter()
            .enumerate()
            .map(|(index, task)| {
                let sample = sft_sample(task);
                QwenLegalDistributedSftRecord {
                    sample_id: sample.sample_id,
                    task_id: task.task_id.clone(),
                    required_answer_path: task.required_answer_path.clone(),
                    prompt: task.instructions.clone(),
                    answer_markdown: task.replay_answer_markdown.clone(),
                    answer_sha256: sha256_hex(task.replay_answer_markdown.as_bytes()),
                    shard_index: u32::try_from(index).unwrap_or_default() % 2,
                    source_token_count: sample.source_token_count,
                    target_token_id: sample.target_token_id,
                    hidden_state: sample.final_hidden_state,
                }
            })
            .collect::<Vec<_>>();
        let dataset = QwenLegalDistributedDatasetReceipt {
            schema_version: String::from(QWEN_LEGAL_DISTRIBUTED_DATASET_SCHEMA_VERSION),
            dataset_id: String::from("dataset.test"),
            dataset_path: String::from("dataset.jsonl"),
            dataset_sha256: sha256_hex(b"dataset"),
            suite_path: String::from("suite.json"),
            suite_id: String::from("suite"),
            suite_hash: sha256_hex(b"suite"),
            suite_mode: LegalBenchmarkEvalMode::PublicHarveyThreeTask,
            training_allowed: true,
            record_count: records.len(),
            shard_count: 2,
            task_ids: records
                .iter()
                .map(|record| record.task_id.clone())
                .collect(),
            records,
            receipt_digest: String::new(),
        };
        let plans = build_shard_plans(&dataset).expect("two non-empty plans");
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].records.len(), 2);
        assert_eq!(plans[1].records.len(), 1);
    }
}
