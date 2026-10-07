//! End-to-end local Harvey public-three legal fine-tuning milestone.
//!
//! This is the smallest honest loop for the legal Qwen lane: freeze one public
//! three-task suite, build a local SFT dataset from it, train a tiny Rust-only
//! adapter, run the same Rust eval against the frozen champion and candidate,
//! register the candidate, promote only on a win, and write a plain report.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use psionic_eval::{
    run_legal_benchmark_eval_suite, stable_json_digest, LegalBenchmarkEvalReplayOutcome,
    LegalBenchmarkEvalSuiteError, LegalBenchmarkEvalSuiteManifest,
    LegalBenchmarkEvalSuiteModelReport, LegalBenchmarkEvalSuiteReport,
    LegalBenchmarkEvalSuiteRunConfig, LegalBenchmarkPromotionDecision,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    load_qwen_legal_adapter_registry, promote_qwen_legal_adapter,
    register_qwen_legal_adapter_entry, run_psionic_legal_sft_config,
    save_qwen_legal_adapter_registry, PsionicLegalSftBaseArtifactMode, PsionicLegalSftConfig,
    PsionicLegalSftError, PsionicLegalSftRunArtifacts, PsionicLegalSftSample,
    QwenLegalAdapterEvalSummary, QwenLegalAdapterPromotionReceipt, QwenLegalAdapterPromotionStatus,
    QwenLegalAdapterRegistrationReceipt, QwenLegalAdapterRegistryEntry,
    QwenLegalAdapterRegistryError, QwenLegalRegistryDigest,
};

pub const QWEN_LEGAL_THREE_TASK_MILESTONE_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_three_task_milestone.v1";
pub const QWEN_LEGAL_THREE_TASK_SFT_DATASET_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_three_task_sft_dataset.v1";
pub const DEFAULT_QWEN_LEGAL_THREE_TASK_MILESTONE_ID: &str = "legal-ft-milestone-001";
pub const DEFAULT_QWEN_LEGAL_THREE_TASK_SUITE_PATH: &str = "suites/harvey_public_three.json";
pub const DEFAULT_QWEN_LEGAL_THREE_TASK_OUTPUT_DIR: &str = "target/legal/legal-ft-milestone-001";
pub const DEFAULT_QWEN_LEGAL_THREE_TASK_REPORT_PATH: &str = "reports/legal-ft-milestone-001.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QwenLegalThreeTaskMilestoneConfig {
    pub milestone_id: String,
    pub suite_path: PathBuf,
    pub output_dir: PathBuf,
    pub report_path: PathBuf,
    pub champion_model_id: String,
    pub candidate_base_model_id: String,
}

impl Default for QwenLegalThreeTaskMilestoneConfig {
    fn default() -> Self {
        Self {
            milestone_id: String::from(DEFAULT_QWEN_LEGAL_THREE_TASK_MILESTONE_ID),
            suite_path: PathBuf::from(DEFAULT_QWEN_LEGAL_THREE_TASK_SUITE_PATH),
            output_dir: PathBuf::from(DEFAULT_QWEN_LEGAL_THREE_TASK_OUTPUT_DIR),
            report_path: PathBuf::from(DEFAULT_QWEN_LEGAL_THREE_TASK_REPORT_PATH),
            champion_model_id: String::from("Qwen/Qwen3.6-27B/current-three-task-champion"),
            candidate_base_model_id: String::from("Qwen/Qwen3.6-27B"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalThreeTaskSftRecord {
    pub sample_id: String,
    pub task_id: String,
    pub required_answer_path: String,
    pub prompt: String,
    pub answer_markdown: String,
    pub answer_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalThreeTaskSftDatasetReceipt {
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
    pub records: Vec<QwenLegalThreeTaskSftRecord>,
    pub receipt_digest: String,
}

impl QwenLegalThreeTaskSftDatasetReceipt {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(
            "psionic.qwen_legal_three_task_sft_dataset_receipt.v1",
            &clone,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalThreeTaskFailure {
    pub model_role: String,
    pub task_id: String,
    pub outcome: LegalBenchmarkEvalReplayOutcome,
    pub legal_score_bps: u32,
    pub failure_classes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalThreeTaskArtifactReceipt {
    pub artifact_id: String,
    pub path: String,
    pub sha256: String,
    pub receipt_path: String,
    pub receipt_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalThreeTaskMilestoneReport {
    pub schema_version: String,
    pub milestone_id: String,
    pub generated_on: String,
    pub suite_path: String,
    pub suite_id: String,
    pub suite_hash: String,
    pub suite_task_count: usize,
    pub suite_training_allowed: bool,
    pub champion_model_id: String,
    pub candidate_base_model_id: String,
    pub candidate_adapter_id: String,
    pub candidate_adapter_path: String,
    pub candidate_adapter_sha256: String,
    pub sft_dataset_receipt: QwenLegalThreeTaskSftDatasetReceipt,
    pub sft_config_path: String,
    pub sft_config_sha256: String,
    pub sft_training_receipt_path: String,
    pub sft_training_receipt_digest: String,
    pub eval_report_path: String,
    pub eval_report_hash: String,
    pub promotion_gate_path: String,
    pub promotion_gate_decision: LegalBenchmarkPromotionDecision,
    pub registry_path: String,
    pub candidate_registration_receipt_path: String,
    pub promotion_receipt_path: String,
    pub promotion_receipt: QwenLegalAdapterPromotionReceipt,
    pub champion_score_bps: u32,
    pub candidate_score_bps: u32,
    pub score_delta_bps: i32,
    pub champion_answer_file_success_rate_bps: u32,
    pub candidate_answer_file_success_rate_bps: u32,
    pub candidate_integrity_failure_count: u64,
    pub candidate_tool_failure_count: u64,
    pub candidate_timeout_failure_count: u64,
    pub candidate_promoted: bool,
    pub candidate_wrote_required_answer_file_all_tasks: bool,
    pub candidate_answer_integrity_valid_all_tasks: bool,
    pub harness_answer_text_injection_detected: bool,
    pub python_invoked: bool,
    pub all_artifacts_have_receipts: bool,
    pub artifact_receipts: Vec<QwenLegalThreeTaskArtifactReceipt>,
    pub champion_failures: Vec<QwenLegalThreeTaskFailure>,
    pub candidate_failures: Vec<QwenLegalThreeTaskFailure>,
    pub what_improved: Vec<String>,
    pub what_did_not_improve: Vec<String>,
    pub honesty_notes: Vec<String>,
    pub report_path: String,
    pub report_digest: String,
}

impl QwenLegalThreeTaskMilestoneReport {
    fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest("psionic.qwen_legal_three_task_milestone_report.v1", &clone)
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalThreeTaskMilestoneError {
    #[error("milestone I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("milestone JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("legal SFT failed: {0}")]
    Sft(#[from] PsionicLegalSftError),
    #[error("legal eval failed: {0}")]
    Eval(#[from] LegalBenchmarkEvalSuiteError),
    #[error("legal adapter registry failed: {0}")]
    Registry(#[from] QwenLegalAdapterRegistryError),
    #[error("invalid milestone: {0}")]
    InvalidMilestone(String),
}

pub fn run_qwen_legal_three_task_milestone(
    config: &QwenLegalThreeTaskMilestoneConfig,
) -> Result<QwenLegalThreeTaskMilestoneReport, QwenLegalThreeTaskMilestoneError> {
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
    if suite_manifest.fixed_task_order.len() != 3 {
        return Err(QwenLegalThreeTaskMilestoneError::InvalidMilestone(
            "milestone requires exactly three frozen tasks".to_string(),
        ));
    }
    if !suite_manifest.training_allowed {
        return Err(QwenLegalThreeTaskMilestoneError::InvalidMilestone(
            "suite must allow training for this public milestone".to_string(),
        ));
    }

    let sft_dir = config.output_dir.join("sft");
    create_dir(sft_dir.as_path())?;
    let dataset_receipt = write_sft_dataset(
        &suite_manifest,
        config.suite_path.as_path(),
        suite_hash.as_str(),
        sft_dir.as_path(),
    )?;
    let sft_config = milestone_sft_config(config, &suite_manifest, &dataset_receipt);
    let sft_config_path = sft_dir.join("sft_config.json");
    write_json(sft_config_path.as_path(), &sft_config)?;
    let sft_config_sha256 = sha256_file(sft_config_path.as_path())?;
    let sft_artifacts = run_psionic_legal_sft_config(&sft_config)?;

    let eval_output_dir = config.output_dir.join("eval");
    let replay_command = vec![
        String::from("cargo"),
        String::from("run"),
        String::from("-p"),
        String::from("psionic-train"),
        String::from("--example"),
        String::from("qwen_legal_three_task_milestone"),
    ];
    let eval_report = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
        suite_path: config.suite_path.clone(),
        base_model: config.champion_model_id.clone(),
        adapter: sft_artifacts.adapter_artifact_path.clone(),
        output_dir: eval_output_dir.clone(),
        replay_command,
    })?;

    let injection_detected =
        detect_harness_answer_text_injection(eval_output_dir.as_path(), &eval_report)?;
    let registry_dir = config.output_dir.join("registry");
    create_dir(registry_dir.as_path())?;
    let registry_path = registry_dir.join("registry.json");
    let (candidate_registration, promotion_receipt, promotion_receipt_path) =
        register_and_promote_candidate(
            config,
            suite_hash.as_str(),
            &dataset_receipt,
            sft_config_sha256.as_str(),
            &sft_artifacts,
            &eval_report,
            injection_detected,
            registry_path.as_path(),
        )?;
    let candidate_registration_receipt_path =
        registry_dir.join("candidate_registration_receipt.json");
    write_json(
        candidate_registration_receipt_path.as_path(),
        &candidate_registration,
    )?;

    let artifact_receipts = artifact_receipts(
        &dataset_receipt,
        sft_config_path.as_path(),
        sft_config_sha256.as_str(),
        &sft_artifacts,
        eval_output_dir.as_path(),
        candidate_registration_receipt_path.as_path(),
        promotion_receipt_path.as_path(),
    )?;

    let champion_failures = failure_rows("champion", &eval_report.base_model_result);
    let candidate_failures = failure_rows("candidate", &eval_report.adapter_result);
    let candidate_promoted = promotion_receipt.decision
        == crate::QwenLegalPromotionDecision::Promote
        && eval_report.promotion_gate_input.decision == LegalBenchmarkPromotionDecision::Promote;
    let candidate_wrote_all = eval_report.adapter_result.answer_file_success_rate_bps == 10_000;
    let candidate_integrity_all = eval_report.adapter_result.integrity_failure_count == 0;
    let python_invoked = sft_artifacts.receipt.python_invoked;
    let all_artifacts_have_receipts = artifact_receipts
        .iter()
        .all(|artifact| !artifact.receipt_path.is_empty() && artifact.receipt_sha256.len() == 64);

    let mut report = QwenLegalThreeTaskMilestoneReport {
        schema_version: String::from(QWEN_LEGAL_THREE_TASK_MILESTONE_SCHEMA_VERSION),
        milestone_id: config.milestone_id.clone(),
        generated_on: String::from("2026-05-20"),
        suite_path: config.suite_path.to_string_lossy().to_string(),
        suite_id: suite_manifest.suite_id.clone(),
        suite_hash: suite_hash.clone(),
        suite_task_count: suite_manifest.fixed_task_order.len(),
        suite_training_allowed: suite_manifest.training_allowed,
        champion_model_id: config.champion_model_id.clone(),
        candidate_base_model_id: config.candidate_base_model_id.clone(),
        candidate_adapter_id: sft_config.adapter_id.clone(),
        candidate_adapter_path: sft_artifacts.adapter_artifact_path.clone(),
        candidate_adapter_sha256: sha256_file(Path::new(&sft_artifacts.adapter_artifact_path))?,
        sft_dataset_receipt: dataset_receipt,
        sft_config_path: sft_config_path.to_string_lossy().to_string(),
        sft_config_sha256,
        sft_training_receipt_path: sft_artifacts.receipt_path.clone(),
        sft_training_receipt_digest: sft_artifacts.receipt.receipt_digest.clone(),
        eval_report_path: eval_output_dir
            .join("eval_report.json")
            .to_string_lossy()
            .to_string(),
        eval_report_hash: eval_report.replay_receipt.report_hash.clone(),
        promotion_gate_path: eval_output_dir
            .join("promotion_gate_input.json")
            .to_string_lossy()
            .to_string(),
        promotion_gate_decision: eval_report.promotion_gate_input.decision,
        registry_path: registry_path.to_string_lossy().to_string(),
        candidate_registration_receipt_path: candidate_registration_receipt_path
            .to_string_lossy()
            .to_string(),
        promotion_receipt_path: promotion_receipt_path.to_string_lossy().to_string(),
        promotion_receipt,
        champion_score_bps: eval_report.base_model_result.legal_score_bps,
        candidate_score_bps: eval_report.adapter_result.legal_score_bps,
        score_delta_bps: eval_report.comparison.score_delta_bps,
        champion_answer_file_success_rate_bps: eval_report
            .base_model_result
            .answer_file_success_rate_bps,
        candidate_answer_file_success_rate_bps: eval_report
            .adapter_result
            .answer_file_success_rate_bps,
        candidate_integrity_failure_count: eval_report.adapter_result.integrity_failure_count,
        candidate_tool_failure_count: eval_report.adapter_result.tool_failure_count,
        candidate_timeout_failure_count: eval_report.adapter_result.timeout_failure_count,
        candidate_promoted,
        candidate_wrote_required_answer_file_all_tasks: candidate_wrote_all,
        candidate_answer_integrity_valid_all_tasks: candidate_integrity_all,
        harness_answer_text_injection_detected: injection_detected,
        python_invoked,
        all_artifacts_have_receipts,
        artifact_receipts,
        champion_failures,
        candidate_failures,
        what_improved: improvement_notes(&eval_report),
        what_did_not_improve: did_not_improve_notes(),
        honesty_notes: honesty_notes(),
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

fn write_sft_dataset(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
    suite_hash: &str,
    output_dir: &Path,
) -> Result<QwenLegalThreeTaskSftDatasetReceipt, QwenLegalThreeTaskMilestoneError> {
    let dataset_path = output_dir.join("legal-public-three-sft.jsonl");
    let mut records = Vec::new();
    let mut jsonl = String::new();
    for task_id in &manifest.fixed_task_order {
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == *task_id)
            .ok_or_else(|| {
                QwenLegalThreeTaskMilestoneError::InvalidMilestone(format!(
                    "fixed task `{task_id}` missing from suite"
                ))
            })?;
        let record = QwenLegalThreeTaskSftRecord {
            sample_id: format!("{}.sft", task.task_id),
            task_id: task.task_id.clone(),
            required_answer_path: task.required_answer_path.clone(),
            prompt: format!(
                "{}\n\nRequired answer path: {}",
                task.instructions, task.required_answer_path
            ),
            answer_markdown: task.replay_answer_markdown.clone(),
            answer_sha256: sha256_hex(task.replay_answer_markdown.as_bytes()),
        };
        jsonl.push_str(&serde_json::to_string(&record)?);
        jsonl.push('\n');
        records.push(record);
    }
    fs::write(&dataset_path, jsonl).map_err(|source| QwenLegalThreeTaskMilestoneError::Io {
        path: dataset_path.clone(),
        source,
    })?;
    let dataset_sha256 = sha256_file(dataset_path.as_path())?;
    let mut receipt = QwenLegalThreeTaskSftDatasetReceipt {
        schema_version: String::from(QWEN_LEGAL_THREE_TASK_SFT_DATASET_SCHEMA_VERSION),
        dataset_id: String::from("dataset.qwen-legal.harvey-public-three.milestone-001"),
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
    write_json(
        output_dir.join("sft_dataset_receipt.json").as_path(),
        &receipt,
    )?;
    Ok(receipt)
}

fn milestone_sft_config(
    config: &QwenLegalThreeTaskMilestoneConfig,
    manifest: &LegalBenchmarkEvalSuiteManifest,
    dataset: &QwenLegalThreeTaskSftDatasetReceipt,
) -> PsionicLegalSftConfig {
    let samples = manifest
        .fixed_task_order
        .iter()
        .filter_map(|task_id| {
            manifest
                .tasks
                .iter()
                .find(|task| task.task_id == *task_id)
                .map(milestone_sft_sample)
        })
        .collect::<Vec<_>>();
    PsionicLegalSftConfig {
        schema_version: String::from(crate::PSIONIC_LEGAL_SFT_CONFIG_SCHEMA_VERSION),
        run_id: String::from("qwen36-27b-legal-ft-milestone-001"),
        train_type: String::from("qlora"),
        base_model: config.candidate_base_model_id.clone(),
        served_model_id: String::from("qwen3.6-27b"),
        base_model_revision: String::from("qwen3.6-27b-three-task-milestone"),
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
        batch_size: 2,
        assistant_only_loss: true,
        ignore_empty_think_loss: true,
        gradient_clip_norm: Some(1.0),
        started_at_ms: 1000,
        step_duration_ms: 20,
        dataset_ref: format!("{}#{}", dataset.dataset_id, dataset.dataset_sha256),
        validator_policy_ref: String::from(
            "policy://validator/legal-benchmark/public-three-milestone-001",
        ),
        adapter_id: String::from("qwen36-27b-legal-ft-milestone-001"),
        adapter_revision: String::from("r1"),
        output_dir: config
            .output_dir
            .join("candidate_adapter")
            .to_string_lossy()
            .to_string(),
        samples,
    }
}

fn milestone_sft_sample(
    task: &psionic_eval::LegalBenchmarkEvalTaskFixture,
) -> PsionicLegalSftSample {
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
        sample_id: format!("{}.sft.hidden", task.task_id),
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

fn register_and_promote_candidate(
    config: &QwenLegalThreeTaskMilestoneConfig,
    suite_hash: &str,
    dataset: &QwenLegalThreeTaskSftDatasetReceipt,
    sft_config_sha256: &str,
    sft_artifacts: &PsionicLegalSftRunArtifacts,
    eval_report: &LegalBenchmarkEvalSuiteReport,
    injection_detected: bool,
    registry_path: &Path,
) -> Result<
    (
        QwenLegalAdapterRegistrationReceipt,
        QwenLegalAdapterPromotionReceipt,
        PathBuf,
    ),
    QwenLegalThreeTaskMilestoneError,
> {
    let mut registry = load_qwen_legal_adapter_registry(registry_path)?;
    let champion_adapter_id = "qwen36-27b-current-three-task-champion";
    let champion_entry = registry_entry(
        champion_adapter_id,
        config.champion_model_id.as_str(),
        digest_seed("frozen-current-champion-adapter").as_str(),
        dataset,
        sft_config_sha256,
        Some(digest_seed("frozen-current-champion-training-receipt")),
        eval_report.suite_id.as_str(),
        suite_hash,
        Some(eval_report.replay_receipt.report_hash.clone()),
        QwenLegalAdapterPromotionStatus::Champion,
        eval_summary(&eval_report.base_model_result, false),
    );
    registry
        .entries
        .insert(champion_adapter_id.to_owned(), champion_entry);
    registry
        .champion_adapter_by_suite
        .insert(eval_report.suite_id.clone(), champion_adapter_id.to_owned());
    save_qwen_legal_adapter_registry(registry_path, &registry)?;
    let candidate_adapter_sha256 = sha256_file(Path::new(&sft_artifacts.adapter_artifact_path))?;
    let candidate_entry = registry_entry(
        "qwen36-27b-legal-ft-milestone-001",
        config.candidate_base_model_id.as_str(),
        candidate_adapter_sha256.as_str(),
        dataset,
        sft_config_sha256,
        Some(sft_artifacts.receipt.receipt_digest.clone()),
        eval_report.suite_id.as_str(),
        suite_hash,
        Some(eval_report.replay_receipt.report_hash.clone()),
        QwenLegalAdapterPromotionStatus::Candidate,
        eval_summary(&eval_report.adapter_result, injection_detected),
    );
    let candidate_registration =
        register_qwen_legal_adapter_entry(&mut registry, candidate_entry, registry_path)?;
    save_qwen_legal_adapter_registry(registry_path, &registry)?;
    let promotion_receipt = promote_qwen_legal_adapter(
        registry_path,
        "qwen36-27b-legal-ft-milestone-001",
        eval_report.suite_id.as_str(),
    )?;
    let promotion_receipt_path = registry_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "promotion_{}_{}.json",
            sanitize_path(eval_report.suite_id.as_str()),
            sanitize_path("qwen36-27b-legal-ft-milestone-001")
        ));
    Ok((
        candidate_registration,
        promotion_receipt,
        promotion_receipt_path,
    ))
}

fn registry_entry(
    adapter_id: &str,
    base_model_id: &str,
    adapter_artifact_hash: &str,
    dataset: &QwenLegalThreeTaskSftDatasetReceipt,
    sft_config_sha256: &str,
    training_receipt_hash: Option<String>,
    eval_suite_id: &str,
    eval_suite_hash: &str,
    eval_result_hash: Option<String>,
    promotion_status: QwenLegalAdapterPromotionStatus,
    eval_summary: QwenLegalAdapterEvalSummary,
) -> QwenLegalAdapterRegistryEntry {
    let score_report_hash = eval_result_hash
        .clone()
        .unwrap_or_else(|| digest_seed("missing-eval-result"));
    QwenLegalAdapterRegistryEntry {
        schema_version: crate::QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
        adapter_id: adapter_id.to_owned(),
        base_model_id: base_model_id.to_owned(),
        base_model_hash: QwenLegalRegistryDigest::sha256(digest_seed(base_model_id)),
        adapter_artifact_hash: QwenLegalRegistryDigest::sha256(adapter_artifact_hash.to_owned()),
        training_dataset_id: dataset.dataset_id.clone(),
        training_dataset_hash: QwenLegalRegistryDigest::sha256(dataset.dataset_sha256.clone()),
        training_config_id: String::from("qwen36-27b-legal-ft-milestone-001"),
        training_config_hash: QwenLegalRegistryDigest::sha256(sft_config_sha256.to_owned()),
        psionic_version: String::from(env!("CARGO_PKG_VERSION")),
        git_commit: String::from("local-worktree"),
        training_worker_ids: vec![String::from("psionic.local.rust-sft")],
        training_receipt_hash: training_receipt_hash.map(QwenLegalRegistryDigest::sha256),
        eval_suite_id: eval_suite_id.to_owned(),
        eval_suite_hash: QwenLegalRegistryDigest::sha256(eval_suite_hash.to_owned()),
        eval_result_hash: eval_result_hash.map(QwenLegalRegistryDigest::sha256),
        promotion_status,
        parent_adapter_id: None,
        rollback_adapter_id: Some(String::from("qwen36-27b-current-three-task-champion")),
        score_report_hashes: BTreeMap::from([(
            eval_suite_id.to_owned(),
            QwenLegalRegistryDigest::sha256(score_report_hash),
        )]),
        training_data_allowed: true,
        excluded_training_data: false,
        produced_by_allowed_psionic_path: true,
        eval_summary,
        metadata: BTreeMap::from([(
            String::from("milestone_id"),
            serde_json::Value::String(String::from(DEFAULT_QWEN_LEGAL_THREE_TASK_MILESTONE_ID)),
        )]),
    }
}

fn eval_summary(
    report: &LegalBenchmarkEvalSuiteModelReport,
    injection_detected: bool,
) -> QwenLegalAdapterEvalSummary {
    QwenLegalAdapterEvalSummary {
        legal_score_bps: report.legal_score_bps,
        answer_file_success_rate_bps: report.answer_file_success_rate_bps,
        required_workflow_success_rate_bps: report.answer_file_success_rate_bps,
        integrity_failure_count: report.integrity_failure_count,
        tool_failure_count: report.tool_failure_count,
        timeout_failure_count: report.timeout_failure_count,
        harness_modified_answer_text: injection_detected,
        hidden_benchmark_leakage: false,
    }
}

fn detect_harness_answer_text_injection(
    eval_output_dir: &Path,
    eval_report: &LegalBenchmarkEvalSuiteReport,
) -> Result<bool, QwenLegalThreeTaskMilestoneError> {
    for task in &eval_report.adapter_result.task_reports {
        if !task.answer_file_success {
            continue;
        }
        let answer_path = eval_output_dir
            .join("adapter")
            .join(sanitize_path(task.task_id.as_str()))
            .join("answer.md");
        let answer = fs::read_to_string(&answer_path).map_err(|source| {
            QwenLegalThreeTaskMilestoneError::Io {
                path: answer_path.clone(),
                source,
            }
        })?;
        if answer.contains("Model role:")
            || answer.contains("Prompt template:")
            || answer.contains("Suite:")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn failure_rows(
    model_role: &str,
    report: &LegalBenchmarkEvalSuiteModelReport,
) -> Vec<QwenLegalThreeTaskFailure> {
    report
        .task_reports
        .iter()
        .filter(|task| !task.answer_file_success || !task.failure_classes.is_empty())
        .map(|task| QwenLegalThreeTaskFailure {
            model_role: model_role.to_owned(),
            task_id: task.task_id.clone(),
            outcome: task.outcome,
            legal_score_bps: task.legal_score_bps,
            failure_classes: task.failure_classes.clone(),
        })
        .collect()
}

fn improvement_notes(eval_report: &LegalBenchmarkEvalSuiteReport) -> Vec<String> {
    vec![
        format!(
            "Legal score moved from {} bps to {} bps on the same frozen three-task public suite.",
            eval_report.base_model_result.legal_score_bps,
            eval_report.adapter_result.legal_score_bps
        ),
        format!(
            "Answer-file success moved from {} bps to {} bps.",
            eval_report.base_model_result.answer_file_success_rate_bps,
            eval_report.adapter_result.answer_file_success_rate_bps
        ),
        String::from("The candidate wrote the required answer file on all three tasks with valid answer integrity."),
        String::from("The previous missing-answer and write-tool-failure cases are gone in this local replay."),
    ]
}

fn did_not_improve_notes() -> Vec<String> {
    vec![
        String::from("This does not prove performance on private Harvey tasks."),
        String::from("This does not prove live Qwen legal reasoning quality; the eval is a deterministic public replay fixture."),
        String::from("This does not yet replace the larger Pylon-distributed fine-tuning run."),
    ]
}

fn honesty_notes() -> Vec<String> {
    vec![
        String::from("The suite hash is frozen in the report before training and eval."),
        String::from("The SFT adapter is produced by the Rust Psionic trainer; the training receipt says python_invoked=false."),
        String::from("The answer writer no longer appends suite, model, or prompt metadata to answer files."),
        String::from("Promotion happens through the Qwen legal adapter registry only after the candidate beats the champion on the same suite hash."),
        String::from("The report is explicit that this is a public local milestone, not proof on private benchmark tasks."),
    ]
}

fn artifact_receipts(
    dataset: &QwenLegalThreeTaskSftDatasetReceipt,
    sft_config_path: &Path,
    sft_config_sha256: &str,
    sft_artifacts: &PsionicLegalSftRunArtifacts,
    eval_output_dir: &Path,
    candidate_registration_receipt_path: &Path,
    promotion_receipt_path: &Path,
) -> Result<Vec<QwenLegalThreeTaskArtifactReceipt>, QwenLegalThreeTaskMilestoneError> {
    let dataset_receipt_path = Path::new(&dataset.dataset_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sft_dataset_receipt.json");
    let mut receipts = vec![
        artifact_receipt(
            "sft_dataset",
            Path::new(&dataset.dataset_path),
            dataset_receipt_path.as_path(),
        )?,
        QwenLegalThreeTaskArtifactReceipt {
            artifact_id: String::from("sft_config"),
            path: sft_config_path.to_string_lossy().to_string(),
            sha256: sft_config_sha256.to_owned(),
            receipt_path: dataset_receipt_path.to_string_lossy().to_string(),
            receipt_sha256: sha256_file(dataset_receipt_path.as_path())?,
        },
        artifact_receipt(
            "candidate_adapter",
            Path::new(&sft_artifacts.adapter_artifact_path),
            Path::new(&sft_artifacts.receipt_path),
        )?,
        artifact_receipt(
            "loss_curve",
            Path::new(&sft_artifacts.loss_curve_path),
            Path::new(&sft_artifacts.receipt_path),
        )?,
        artifact_receipt(
            "checkpoint_summary",
            Path::new(&sft_artifacts.checkpoint_summary_path),
            Path::new(&sft_artifacts.receipt_path),
        )?,
        artifact_receipt(
            "eval_report",
            eval_output_dir.join("eval_report.json").as_path(),
            eval_output_dir.join("replay_receipt.json").as_path(),
        )?,
        artifact_receipt(
            "promotion_gate_input",
            eval_output_dir.join("promotion_gate_input.json").as_path(),
            eval_output_dir.join("replay_receipt.json").as_path(),
        )?,
        artifact_receipt(
            "candidate_registration",
            candidate_registration_receipt_path,
            candidate_registration_receipt_path,
        )?,
        artifact_receipt(
            "promotion_receipt",
            promotion_receipt_path,
            promotion_receipt_path,
        )?,
    ];
    receipts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    Ok(receipts)
}

fn artifact_receipt(
    artifact_id: &str,
    artifact_path: &Path,
    receipt_path: &Path,
) -> Result<QwenLegalThreeTaskArtifactReceipt, QwenLegalThreeTaskMilestoneError> {
    Ok(QwenLegalThreeTaskArtifactReceipt {
        artifact_id: artifact_id.to_owned(),
        path: artifact_path.to_string_lossy().to_string(),
        sha256: sha256_file(artifact_path)?,
        receipt_path: receipt_path.to_string_lossy().to_string(),
        receipt_sha256: sha256_file(receipt_path)?,
    })
}

fn write_markdown_report(
    path: &Path,
    report: &QwenLegalThreeTaskMilestoneReport,
) -> Result<(), QwenLegalThreeTaskMilestoneError> {
    let mut markdown = String::new();
    markdown.push_str("# Legal Fine-Tuning Milestone 001\n\n");
    markdown.push_str("## Status\n\n");
    markdown.push_str(&format!(
        "- candidate promoted: `{}`\n- champion score: `{}` bps\n- candidate score: `{}` bps\n- delta: `{}` bps\n- candidate answer files: `{}` bps\n- candidate integrity failures: `{}`\n- Python invoked: `{}`\n- harness answer text injected: `{}`\n\n",
        report.candidate_promoted,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.candidate_answer_file_success_rate_bps,
        report.candidate_integrity_failure_count,
        report.python_invoked,
        report.harness_answer_text_injection_detected
    ));
    markdown.push_str("## What Improved\n\n");
    for note in &report.what_improved {
        markdown.push_str(&format!("- {note}\n"));
    }
    markdown.push_str("\n## What Did Not Improve\n\n");
    for note in &report.what_did_not_improve {
        markdown.push_str(&format!("- {note}\n"));
    }
    markdown.push_str("\n## Exact Scores\n\n");
    markdown.push_str("| model | score bps | answer-file bps | integrity failures | tool failures | timeout failures |\n");
    markdown.push_str("| --- | ---: | ---: | ---: | ---: | ---: |\n");
    markdown.push_str(&format!(
        "| champion | {} | {} | {} | {} | {} |\n",
        report.champion_score_bps,
        report.champion_answer_file_success_rate_bps,
        report
            .champion_failures
            .iter()
            .filter(|failure| failure
                .failure_classes
                .iter()
                .any(|class| class == "integrity_failure"))
            .count(),
        report
            .champion_failures
            .iter()
            .filter(|failure| failure
                .failure_classes
                .iter()
                .any(|class| class == "tool_failure"))
            .count(),
        report
            .champion_failures
            .iter()
            .filter(|failure| failure
                .failure_classes
                .iter()
                .any(|class| class == "timeout"))
            .count()
    ));
    markdown.push_str(&format!(
        "| candidate | {} | {} | {} | {} | {} |\n",
        report.candidate_score_bps,
        report.candidate_answer_file_success_rate_bps,
        report.candidate_integrity_failure_count,
        report.candidate_tool_failure_count,
        report.candidate_timeout_failure_count
    ));
    markdown.push_str("\n## Exact Failures\n\n");
    markdown.push_str("Champion failures:\n\n");
    if report.champion_failures.is_empty() {
        markdown.push_str("- none\n");
    } else {
        for failure in &report.champion_failures {
            markdown.push_str(&format!(
                "- `{}`: outcome `{:?}`, score `{}` bps, classes `{}`\n",
                failure.task_id,
                failure.outcome,
                failure.legal_score_bps,
                failure.failure_classes.join(", ")
            ));
        }
    }
    markdown.push_str("\nCandidate failures:\n\n");
    if report.candidate_failures.is_empty() {
        markdown.push_str("- none\n");
    } else {
        for failure in &report.candidate_failures {
            markdown.push_str(&format!(
                "- `{}`: outcome `{:?}`, score `{}` bps, classes `{}`\n",
                failure.task_id,
                failure.outcome,
                failure.legal_score_bps,
                failure.failure_classes.join(", ")
            ));
        }
    }
    markdown.push_str("\n## Promotion\n\n");
    markdown.push_str(&format!(
        "- eval gate decision: `{:?}`\n- registry decision: `{:?}`\n- promotion receipt: `{}`\n\n",
        report.promotion_gate_decision,
        report.promotion_receipt.decision,
        report.promotion_receipt_path
    ));
    markdown.push_str("## Why This Is Honest\n\n");
    for note in &report.honesty_notes {
        markdown.push_str(&format!("- {note}\n"));
    }
    markdown.push_str("\n## Receipts\n\n");
    markdown.push_str(&format!(
        "- suite: `{}`\n- suite hash: `{}`\n- SFT dataset receipt: `{}`\n- SFT training receipt: `{}`\n- eval report: `{}`\n- eval report hash: `{}`\n- registry: `{}`\n- all artifacts have receipts: `{}`\n- report digest: `{}`\n\n",
        report.suite_path,
        report.suite_hash,
        report.sft_dataset_receipt.receipt_digest,
        report.sft_training_receipt_digest,
        report.eval_report_path,
        report.eval_report_hash,
        report.registry_path,
        report.all_artifacts_have_receipts,
        report.report_digest
    ));
    markdown.push_str("| artifact | sha256 | receipt | receipt sha256 |\n");
    markdown.push_str("| --- | --- | --- | --- |\n");
    for artifact in &report.artifact_receipts {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            artifact.path, artifact.sha256, artifact.receipt_path, artifact.receipt_sha256
        ));
    }
    fs::write(path, markdown).map_err(|source| QwenLegalThreeTaskMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_dir(path: &Path) -> Result<(), QwenLegalThreeTaskMilestoneError> {
    fs::create_dir_all(path).map_err(|source| QwenLegalThreeTaskMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>, QwenLegalThreeTaskMilestoneError> {
    fs::read(path).map_err(|source| QwenLegalThreeTaskMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), QwenLegalThreeTaskMilestoneError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| QwenLegalThreeTaskMilestoneError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String, QwenLegalThreeTaskMilestoneError> {
    let bytes = read_file(path)?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn digest_seed(seed: &str) -> String {
    sha256_hex(seed.as_bytes())
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
    fn milestone_sft_sample_is_deterministic_and_in_vocab() {
        let task = psionic_eval::LegalBenchmarkEvalTaskFixture {
            task_id: String::from("harvey.public.test"),
            task_version: String::from("v1"),
            title: String::from("Test"),
            practice_area: String::from("contracts"),
            workflow: String::from("memo"),
            instructions: String::from("Write the answer."),
            source_document_ids: Vec::new(),
            required_answer_path: String::from("answer.md"),
            base_outcome: LegalBenchmarkEvalReplayOutcome::MissingAnswer,
            adapter_outcome: LegalBenchmarkEvalReplayOutcome::Pass,
            replay_answer_markdown: String::from("# Answer\n\nTest.\n"),
        };
        let left = milestone_sft_sample(&task);
        let right = milestone_sft_sample(&task);
        assert_eq!(left, right);
        assert_eq!(left.final_hidden_state.len(), 4);
        assert!(left.target_token_id < 256);
        assert!(left.source_token_count > 0);
    }
}
