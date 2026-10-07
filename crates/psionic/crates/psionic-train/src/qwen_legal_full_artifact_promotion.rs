//! Full-artifact promotion gates for Qwen legal fine-tuning candidates.
//!
//! This module is intentionally separate from the older small-adapter registry:
//! it admits whole Qwen checkpoint/adapter candidates from Pylon jobs only when
//! the promotion packet carries lineage, eval, worker, aggregate, and settlement
//! evidence.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN_LEGAL_FULL_ARTIFACT_PROMOTION_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_full_artifact_promotion.v1";
pub const QWEN_LEGAL_FULL_ARTIFACT_REGISTRY_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_full_artifact_registry.v1";
pub const QWEN_LEGAL_AUTOPILOT4_PROMOTION_FEED_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_autopilot4_promotion_summary_feed.v1";

const DEFAULT_OUTPUT_DIR: &str = "target/legal/qwen_promotion_gate/full-artifact-001";
const DEFAULT_REPORT_ID: &str = "qwen.legal.full_artifact_promotion.001";
const DEFAULT_REGISTRY_ID: &str = "qwen.legal.full_artifact_registry.local";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QwenLegalFullArtifactPromotionConfig {
    pub output_dir: PathBuf,
    pub report_id: String,
    pub registry_id: String,
}

impl Default for QwenLegalFullArtifactPromotionConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from(DEFAULT_OUTPUT_DIR),
            report_id: String::from(DEFAULT_REPORT_ID),
            registry_id: String::from(DEFAULT_REGISTRY_ID),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalFullArtifactCandidateKind {
    Base,
    Sft,
    Dpo,
    Grpo,
    PriorChampion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalFullArtifactPromotionStage {
    LocalSmoke,
    PublicTrainingSliceEval,
    PublicHeldOutEval,
    PrivateEval,
    AdversarialHoldout,
    ServingCanary,
}

impl QwenLegalFullArtifactPromotionStage {
    pub fn required_stages() -> [Self; 6] {
        [
            Self::LocalSmoke,
            Self::PublicTrainingSliceEval,
            Self::PublicHeldOutEval,
            Self::PrivateEval,
            Self::AdversarialHoldout,
            Self::ServingCanary,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalFullArtifactEvalVisibility {
    LocalSmoke,
    PublicTrainingSlice,
    PublicHeldOut,
    Private,
    AdversarialHoldout,
    ServingCanary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalFullArtifactHardFailureClass {
    Leakage,
    AnswerInjection,
    MissingArtifact,
    PrivateEvalTrainContamination,
    InvalidWorkerReceipt,
    UnpaidAcceptedWork,
    ServingMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalFullArtifactPromotionDecision {
    Promote,
    Hold,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactRef {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub digest: String,
    pub uri: String,
    pub byte_len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactSettlementEvidence {
    pub settlement_state: String,
    pub accepted_work_count: u32,
    pub paid_accepted_work_count: u32,
    pub bitcoin_txids: Vec<String>,
    pub invoice_receipt_hashes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactLineage {
    pub corpus_manifest_hash: String,
    pub base_model_hash: String,
    pub model_checkpoint_hash: String,
    pub adapter_hash: Option<String>,
    pub tokenizer_hash: String,
    pub prompt_template_hash: String,
    pub training_config_hash: String,
    pub run_receipt_hashes: Vec<String>,
    pub worker_receipt_hashes: Vec<String>,
    pub aggregate_receipt_hash: String,
    pub settlement: QwenLegalFullArtifactSettlementEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactEvalReport {
    pub stage: QwenLegalFullArtifactPromotionStage,
    pub visibility: QwenLegalFullArtifactEvalVisibility,
    pub suite_id: String,
    pub suite_hash: String,
    pub report_hash: String,
    pub score_bps: u32,
    pub answer_file_success_rate_bps: u32,
    pub required_workflow_success_rate_bps: u32,
    pub integrity_failure_count: u64,
    pub tool_failure_count: u64,
    pub timeout_failure_count: u64,
    pub leakage_detected: bool,
    pub answer_injection_detected: bool,
    pub private_eval_train_contamination_detected: bool,
    pub serving_mismatch_detected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactCandidate {
    pub candidate_id: String,
    pub candidate_kind: QwenLegalFullArtifactCandidateKind,
    pub base_model_id: String,
    pub adapter_id: Option<String>,
    pub serving_route_id: Option<String>,
    pub prior_champion_ref: Option<String>,
    pub artifacts: Vec<QwenLegalFullArtifactRef>,
    pub lineage: QwenLegalFullArtifactLineage,
    pub eval_reports: Vec<QwenLegalFullArtifactEvalReport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactComparisonRow {
    pub baseline_candidate_id: String,
    pub baseline_kind: QwenLegalFullArtifactCandidateKind,
    pub candidate_id: String,
    pub candidate_kind: QwenLegalFullArtifactCandidateKind,
    pub public_training_slice_delta_bps: i32,
    pub public_held_out_delta_bps: i32,
    pub private_eval_delta_bps: Option<i32>,
    pub adversarial_holdout_delta_bps: i32,
    pub serving_canary_delta_bps: i32,
    pub candidate_beats_baseline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactAutopilot4StageSummary {
    pub stage: QwenLegalFullArtifactPromotionStage,
    pub visibility: QwenLegalFullArtifactEvalVisibility,
    pub status: String,
    pub score_bps: Option<u32>,
    pub private_score_redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactAutopilot4SummaryFeed {
    pub schema_version: String,
    pub feed_id: String,
    pub source_report_id: String,
    pub candidate_id: String,
    pub decision: QwenLegalFullArtifactPromotionDecision,
    pub hard_failure_classes: Vec<QwenLegalFullArtifactHardFailureClass>,
    pub public_status_line: String,
    pub stage_summaries: Vec<QwenLegalFullArtifactAutopilot4StageSummary>,
    pub comparison_candidate_ids: Vec<String>,
    pub private_eval_present: bool,
    pub private_score_exported: bool,
    pub private_task_content_exported: bool,
    pub hidden_or_retained_score_claim: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactPromotionReport {
    pub schema_version: String,
    pub report_id: String,
    pub candidate_id: String,
    pub decision: QwenLegalFullArtifactPromotionDecision,
    pub hard_failure_classes: Vec<QwenLegalFullArtifactHardFailureClass>,
    pub reasons: Vec<String>,
    pub stages_present: Vec<QwenLegalFullArtifactPromotionStage>,
    pub missing_stages: Vec<QwenLegalFullArtifactPromotionStage>,
    pub comparison_kinds_present: Vec<QwenLegalFullArtifactCandidateKind>,
    pub required_comparison_kinds_present: bool,
    pub comparisons: Vec<QwenLegalFullArtifactComparisonRow>,
    pub public_private_boundary: String,
    pub training_lineage_complete: bool,
    pub settlement_complete: bool,
    pub serving_ready: bool,
    pub private_task_content_exported: bool,
    pub autopilot4_summary_feed: QwenLegalFullArtifactAutopilot4SummaryFeed,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalFullArtifactPromotionRegistry {
    pub schema_version: String,
    pub registry_id: String,
    pub candidates: BTreeMap<String, QwenLegalFullArtifactCandidate>,
    pub champion_candidate_id: Option<String>,
    pub reports: Vec<QwenLegalFullArtifactPromotionReport>,
    pub registry_digest: String,
}

impl QwenLegalFullArtifactPromotionRegistry {
    pub fn stable_digest(&self) -> Result<String, QwenLegalFullArtifactPromotionError> {
        let mut clone = self.clone();
        clone.registry_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_full_artifact_registry|", &clone)
    }
}

impl QwenLegalFullArtifactPromotionReport {
    pub fn stable_digest(&self) -> Result<String, QwenLegalFullArtifactPromotionError> {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(
            b"psionic_qwen_legal_full_artifact_promotion_report|",
            &clone,
        )
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalFullArtifactPromotionError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bad config: {0}")]
    BadConfig(String),
}

pub fn run_qwen_legal_full_artifact_promotion_cli(
    args: &[String],
) -> Result<QwenLegalFullArtifactPromotionReport, QwenLegalFullArtifactPromotionError> {
    let mut config = QwenLegalFullArtifactPromotionConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" | "--output-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(QwenLegalFullArtifactPromotionError::BadConfig(
                        "missing value for --out".to_owned(),
                    ));
                };
                config.output_dir = PathBuf::from(value);
            }
            "--report-id" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(QwenLegalFullArtifactPromotionError::BadConfig(
                        "missing value for --report-id".to_owned(),
                    ));
                };
                config.report_id = value.clone();
            }
            "--registry-id" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(QwenLegalFullArtifactPromotionError::BadConfig(
                        "missing value for --registry-id".to_owned(),
                    ));
                };
                config.registry_id = value.clone();
            }
            "--help" | "-h" => {
                return Err(QwenLegalFullArtifactPromotionError::BadConfig(
                    "usage: qwen-legal-artifact-promotion [--out <dir>] [--report-id <id>] [--registry-id <id>]".to_owned(),
                ));
            }
            other => {
                return Err(QwenLegalFullArtifactPromotionError::BadConfig(format!(
                    "unsupported qwen legal artifact promotion arg `{other}`"
                )));
            }
        }
        index += 1;
    }
    write_qwen_legal_full_artifact_promotion_rehearsal(&config)
}

pub fn write_qwen_legal_full_artifact_promotion_rehearsal(
    config: &QwenLegalFullArtifactPromotionConfig,
) -> Result<QwenLegalFullArtifactPromotionReport, QwenLegalFullArtifactPromotionError> {
    fs::create_dir_all(&config.output_dir).map_err(|source| {
        QwenLegalFullArtifactPromotionError::Io {
            path: config.output_dir.clone(),
            source,
        }
    })?;

    let baselines = rehearsal_baselines();
    let candidate = rehearsal_candidate();
    let report =
        evaluate_qwen_legal_full_artifact_promotion(&config.report_id, &candidate, &baselines)?;

    let mut registry = QwenLegalFullArtifactPromotionRegistry {
        schema_version: String::from(QWEN_LEGAL_FULL_ARTIFACT_REGISTRY_SCHEMA_VERSION),
        registry_id: config.registry_id.clone(),
        candidates: BTreeMap::new(),
        champion_candidate_id: (report.decision == QwenLegalFullArtifactPromotionDecision::Promote)
            .then(|| candidate.candidate_id.clone()),
        reports: vec![report.clone()],
        registry_digest: String::new(),
    };
    registry
        .candidates
        .insert(candidate.candidate_id.clone(), candidate);
    for baseline in baselines {
        registry
            .candidates
            .insert(baseline.candidate_id.clone(), baseline);
    }
    registry.registry_digest = registry.stable_digest()?;

    write_pretty_json(
        &config
            .output_dir
            .join("qwen_legal_full_artifact_promotion_report.json"),
        &report,
    )?;
    write_pretty_json(
        &config
            .output_dir
            .join("qwen_legal_full_artifact_promotion_registry.json"),
        &registry,
    )?;
    write_pretty_json(
        &config
            .output_dir
            .join("autopilot4_qwen_legal_promotion_summary_feed.json"),
        &report.autopilot4_summary_feed,
    )?;

    Ok(report)
}

pub fn evaluate_qwen_legal_full_artifact_promotion(
    report_id: &str,
    candidate: &QwenLegalFullArtifactCandidate,
    baselines: &[QwenLegalFullArtifactCandidate],
) -> Result<QwenLegalFullArtifactPromotionReport, QwenLegalFullArtifactPromotionError> {
    let mut hard_failures = collect_hard_failures(candidate);
    let missing_stages = missing_stages(candidate);
    if !missing_stages.is_empty() {
        hard_failures.insert(QwenLegalFullArtifactHardFailureClass::MissingArtifact);
    }

    let comparison_kinds_present = comparison_kinds_present(candidate, baselines);
    let required_comparison_kinds_present =
        required_comparison_kinds_present(&comparison_kinds_present);
    if !required_comparison_kinds_present {
        hard_failures.insert(QwenLegalFullArtifactHardFailureClass::MissingArtifact);
    }

    let comparisons = baselines
        .iter()
        .map(|baseline| comparison_row(candidate, baseline))
        .collect::<Vec<_>>();
    let candidate_beats_all = !comparisons.is_empty()
        && comparisons
            .iter()
            .all(|comparison| comparison.candidate_beats_baseline);
    let training_lineage_complete = training_lineage_complete(candidate);
    let settlement_complete = settlement_complete(&candidate.lineage.settlement);
    let serving_ready = hard_failures
        .iter()
        .all(|failure| *failure != QwenLegalFullArtifactHardFailureClass::ServingMismatch)
        && stage_score(
            candidate,
            QwenLegalFullArtifactPromotionStage::ServingCanary,
        )
        .is_some();

    let decision = if !hard_failures.is_empty() {
        QwenLegalFullArtifactPromotionDecision::Reject
    } else if candidate_beats_all {
        QwenLegalFullArtifactPromotionDecision::Promote
    } else {
        QwenLegalFullArtifactPromotionDecision::Hold
    };
    let hard_failure_classes = hard_failures.iter().copied().collect::<Vec<_>>();
    let reasons = promotion_reasons(
        decision,
        &hard_failure_classes,
        candidate_beats_all,
        required_comparison_kinds_present,
        &missing_stages,
    );
    let stages_present = stages_present(candidate);
    let autopilot4_summary_feed = autopilot4_summary_feed(
        report_id,
        candidate,
        decision,
        &hard_failure_classes,
        &stages_present,
        baselines,
    );
    let mut report = QwenLegalFullArtifactPromotionReport {
        schema_version: String::from(QWEN_LEGAL_FULL_ARTIFACT_PROMOTION_SCHEMA_VERSION),
        report_id: report_id.to_owned(),
        candidate_id: candidate.candidate_id.clone(),
        decision,
        hard_failure_classes,
        reasons,
        stages_present,
        missing_stages,
        comparison_kinds_present,
        required_comparison_kinds_present,
        comparisons,
        public_private_boundary: String::from(
            "Public scores are safe to show as aggregate local/public evidence. Private eval evidence is used only as a promotion gate here; private task content and private task-level answers are not exported to Autopilot4.",
        ),
        training_lineage_complete,
        settlement_complete,
        serving_ready,
        private_task_content_exported: false,
        autopilot4_summary_feed,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest()?;
    Ok(report)
}

fn collect_hard_failures(
    candidate: &QwenLegalFullArtifactCandidate,
) -> BTreeSet<QwenLegalFullArtifactHardFailureClass> {
    let mut failures = BTreeSet::new();
    if !training_lineage_complete(candidate) {
        failures.insert(QwenLegalFullArtifactHardFailureClass::MissingArtifact);
    }
    if invalid_worker_receipt(candidate) {
        failures.insert(QwenLegalFullArtifactHardFailureClass::InvalidWorkerReceipt);
    }
    if !settlement_complete(&candidate.lineage.settlement) {
        failures.insert(QwenLegalFullArtifactHardFailureClass::UnpaidAcceptedWork);
    }
    for eval in &candidate.eval_reports {
        if !digest_is_complete(eval.suite_hash.as_str())
            || !digest_is_complete(eval.report_hash.as_str())
            || eval.score_bps > 10_000
            || eval.answer_file_success_rate_bps > 10_000
            || eval.required_workflow_success_rate_bps > 10_000
        {
            failures.insert(QwenLegalFullArtifactHardFailureClass::MissingArtifact);
        }
        if eval.leakage_detected {
            failures.insert(QwenLegalFullArtifactHardFailureClass::Leakage);
        }
        if eval.answer_injection_detected || eval.integrity_failure_count > 0 {
            failures.insert(QwenLegalFullArtifactHardFailureClass::AnswerInjection);
        }
        if eval.private_eval_train_contamination_detected {
            failures.insert(QwenLegalFullArtifactHardFailureClass::PrivateEvalTrainContamination);
        }
        if eval.serving_mismatch_detected {
            failures.insert(QwenLegalFullArtifactHardFailureClass::ServingMismatch);
        }
    }
    failures
}

fn training_lineage_complete(candidate: &QwenLegalFullArtifactCandidate) -> bool {
    if candidate.artifacts.is_empty() {
        return false;
    }
    if candidate.artifacts.iter().any(|artifact| {
        artifact.artifact_id.trim().is_empty()
            || artifact.artifact_kind.trim().is_empty()
            || artifact.uri.trim().is_empty()
            || artifact.byte_len == 0
            || !digest_is_complete(artifact.digest.as_str())
    }) {
        return false;
    }
    let lineage = &candidate.lineage;
    let required_hashes = [
        lineage.corpus_manifest_hash.as_str(),
        lineage.base_model_hash.as_str(),
        lineage.model_checkpoint_hash.as_str(),
        lineage.tokenizer_hash.as_str(),
        lineage.prompt_template_hash.as_str(),
        lineage.training_config_hash.as_str(),
        lineage.aggregate_receipt_hash.as_str(),
    ];
    if required_hashes.iter().any(|hash| !digest_is_complete(hash)) {
        return false;
    }
    if candidate.candidate_kind != QwenLegalFullArtifactCandidateKind::Base
        && !lineage
            .adapter_hash
            .as_deref()
            .is_some_and(digest_is_complete)
    {
        return false;
    }
    if lineage.run_receipt_hashes.is_empty()
        || lineage
            .run_receipt_hashes
            .iter()
            .any(|hash| !digest_is_complete(hash.as_str()))
    {
        return false;
    }
    !lineage.worker_receipt_hashes.is_empty()
}

fn invalid_worker_receipt(candidate: &QwenLegalFullArtifactCandidate) -> bool {
    candidate.lineage.worker_receipt_hashes.is_empty()
        || candidate
            .lineage
            .worker_receipt_hashes
            .iter()
            .any(|hash| !digest_is_complete(hash.as_str()))
}

fn settlement_complete(settlement: &QwenLegalFullArtifactSettlementEvidence) -> bool {
    settlement.settlement_state == "settled"
        && settlement.accepted_work_count > 0
        && settlement.accepted_work_count == settlement.paid_accepted_work_count
        && !settlement.bitcoin_txids.is_empty()
        && !settlement.invoice_receipt_hashes.is_empty()
        && settlement
            .invoice_receipt_hashes
            .iter()
            .all(|hash| digest_is_complete(hash.as_str()))
}

fn missing_stages(
    candidate: &QwenLegalFullArtifactCandidate,
) -> Vec<QwenLegalFullArtifactPromotionStage> {
    let present = candidate
        .eval_reports
        .iter()
        .map(|report| report.stage)
        .collect::<BTreeSet<_>>();
    QwenLegalFullArtifactPromotionStage::required_stages()
        .into_iter()
        .filter(|stage| !present.contains(stage))
        .collect()
}

fn stages_present(
    candidate: &QwenLegalFullArtifactCandidate,
) -> Vec<QwenLegalFullArtifactPromotionStage> {
    candidate
        .eval_reports
        .iter()
        .map(|report| report.stage)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn comparison_kinds_present(
    candidate: &QwenLegalFullArtifactCandidate,
    baselines: &[QwenLegalFullArtifactCandidate],
) -> Vec<QwenLegalFullArtifactCandidateKind> {
    let mut kinds = baselines
        .iter()
        .map(|baseline| baseline.candidate_kind)
        .collect::<BTreeSet<_>>();
    kinds.insert(candidate.candidate_kind);
    kinds.into_iter().collect()
}

fn required_comparison_kinds_present(kinds: &[QwenLegalFullArtifactCandidateKind]) -> bool {
    let kinds = kinds.iter().copied().collect::<BTreeSet<_>>();
    [
        QwenLegalFullArtifactCandidateKind::Base,
        QwenLegalFullArtifactCandidateKind::Sft,
        QwenLegalFullArtifactCandidateKind::Dpo,
        QwenLegalFullArtifactCandidateKind::Grpo,
        QwenLegalFullArtifactCandidateKind::PriorChampion,
    ]
    .into_iter()
    .all(|kind| kinds.contains(&kind))
}

fn comparison_row(
    candidate: &QwenLegalFullArtifactCandidate,
    baseline: &QwenLegalFullArtifactCandidate,
) -> QwenLegalFullArtifactComparisonRow {
    let public_training_slice_delta_bps = score_delta(
        candidate,
        baseline,
        QwenLegalFullArtifactPromotionStage::PublicTrainingSliceEval,
    );
    let public_held_out_delta_bps = score_delta(
        candidate,
        baseline,
        QwenLegalFullArtifactPromotionStage::PublicHeldOutEval,
    );
    let private_eval_delta_bps = optional_score_delta(
        candidate,
        baseline,
        QwenLegalFullArtifactPromotionStage::PrivateEval,
    );
    let adversarial_holdout_delta_bps = score_delta(
        candidate,
        baseline,
        QwenLegalFullArtifactPromotionStage::AdversarialHoldout,
    );
    let serving_canary_delta_bps = score_delta(
        candidate,
        baseline,
        QwenLegalFullArtifactPromotionStage::ServingCanary,
    );
    let candidate_beats_baseline = public_training_slice_delta_bps > 0
        && public_held_out_delta_bps > 0
        && private_eval_delta_bps.is_some_and(|delta| delta > 0)
        && adversarial_holdout_delta_bps > 0
        && serving_canary_delta_bps > 0;
    QwenLegalFullArtifactComparisonRow {
        baseline_candidate_id: baseline.candidate_id.clone(),
        baseline_kind: baseline.candidate_kind,
        candidate_id: candidate.candidate_id.clone(),
        candidate_kind: candidate.candidate_kind,
        public_training_slice_delta_bps,
        public_held_out_delta_bps,
        private_eval_delta_bps,
        adversarial_holdout_delta_bps,
        serving_canary_delta_bps,
        candidate_beats_baseline,
    }
}

fn score_delta(
    candidate: &QwenLegalFullArtifactCandidate,
    baseline: &QwenLegalFullArtifactCandidate,
    stage: QwenLegalFullArtifactPromotionStage,
) -> i32 {
    optional_score_delta(candidate, baseline, stage).unwrap_or(0)
}

fn optional_score_delta(
    candidate: &QwenLegalFullArtifactCandidate,
    baseline: &QwenLegalFullArtifactCandidate,
    stage: QwenLegalFullArtifactPromotionStage,
) -> Option<i32> {
    Some(
        i32::try_from(stage_score(candidate, stage)?).ok()?
            - i32::try_from(stage_score(baseline, stage)?).ok()?,
    )
}

fn stage_score(
    candidate: &QwenLegalFullArtifactCandidate,
    stage: QwenLegalFullArtifactPromotionStage,
) -> Option<u32> {
    candidate
        .eval_reports
        .iter()
        .find(|report| report.stage == stage)
        .map(|report| report.score_bps)
}

fn promotion_reasons(
    decision: QwenLegalFullArtifactPromotionDecision,
    hard_failure_classes: &[QwenLegalFullArtifactHardFailureClass],
    candidate_beats_all: bool,
    required_comparison_kinds_present: bool,
    missing_stages: &[QwenLegalFullArtifactPromotionStage],
) -> Vec<String> {
    if !hard_failure_classes.is_empty() {
        let mut reasons = hard_failure_classes
            .iter()
            .map(|failure| format!("hard failure: {failure:?}"))
            .collect::<Vec<_>>();
        if !missing_stages.is_empty() {
            reasons.push(format!("missing promotion stages: {missing_stages:?}"));
        }
        if !required_comparison_kinds_present {
            reasons.push(String::from(
                "missing required comparison candidates: base, SFT, DPO, GRPO, and prior champion",
            ));
        }
        return reasons;
    }
    match decision {
        QwenLegalFullArtifactPromotionDecision::Promote => vec![String::from(
            "candidate has complete lineage, settlement evidence, all promotion stages, all comparison classes, and beats every baseline",
        )],
        QwenLegalFullArtifactPromotionDecision::Hold if !candidate_beats_all => vec![String::from(
            "candidate passed hard gates but did not beat every required comparison candidate",
        )],
        QwenLegalFullArtifactPromotionDecision::Hold => vec![String::from(
            "candidate passed hard gates but remains held for operator review",
        )],
        QwenLegalFullArtifactPromotionDecision::Reject => {
            vec![String::from("candidate rejected by promotion policy")]
        }
    }
}

fn autopilot4_summary_feed(
    report_id: &str,
    candidate: &QwenLegalFullArtifactCandidate,
    decision: QwenLegalFullArtifactPromotionDecision,
    hard_failure_classes: &[QwenLegalFullArtifactHardFailureClass],
    stages_present: &[QwenLegalFullArtifactPromotionStage],
    baselines: &[QwenLegalFullArtifactCandidate],
) -> QwenLegalFullArtifactAutopilot4SummaryFeed {
    let stage_summaries = candidate
        .eval_reports
        .iter()
        .map(|report| {
            let private = report.visibility == QwenLegalFullArtifactEvalVisibility::Private;
            QwenLegalFullArtifactAutopilot4StageSummary {
                stage: report.stage,
                visibility: report.visibility,
                status: if hard_failure_classes.is_empty() {
                    String::from("passed")
                } else {
                    String::from("blocked")
                },
                score_bps: (!private).then_some(report.score_bps),
                private_score_redacted: private,
            }
        })
        .collect::<Vec<_>>();
    QwenLegalFullArtifactAutopilot4SummaryFeed {
        schema_version: String::from(QWEN_LEGAL_AUTOPILOT4_PROMOTION_FEED_SCHEMA_VERSION),
        feed_id: format!("autopilot4.qwen_legal.promotion_summary.{report_id}"),
        source_report_id: report_id.to_owned(),
        candidate_id: candidate.candidate_id.clone(),
        decision,
        hard_failure_classes: hard_failure_classes.to_vec(),
        public_status_line: match decision {
            QwenLegalFullArtifactPromotionDecision::Promote => String::from(
                "Promotable: complete public evidence, private gate present, no private task content exported.",
            ),
            QwenLegalFullArtifactPromotionDecision::Hold => {
                String::from("Held: complete enough to review, but not promoted.")
            }
            QwenLegalFullArtifactPromotionDecision::Reject => {
                String::from("Blocked: hard promotion gate failed.")
            }
        },
        stage_summaries,
        comparison_candidate_ids: baselines
            .iter()
            .map(|baseline| baseline.candidate_id.clone())
            .collect(),
        private_eval_present: stages_present
            .contains(&QwenLegalFullArtifactPromotionStage::PrivateEval),
        private_score_exported: false,
        private_task_content_exported: false,
        hidden_or_retained_score_claim: false,
    }
}

fn rehearsal_candidate() -> QwenLegalFullArtifactCandidate {
    candidate(
        "qwen36-legal-grpo-001",
        QwenLegalFullArtifactCandidateKind::Grpo,
        Some("adapter.qwen36.legal.grpo.001"),
        Some("qwen36-legal-prior-champion-000"),
        [8_700, 9_100, 8_800, 8_600, 8_400, 8_700],
    )
}

fn rehearsal_baselines() -> Vec<QwenLegalFullArtifactCandidate> {
    vec![
        candidate(
            "qwen36-base-000",
            QwenLegalFullArtifactCandidateKind::Base,
            None,
            None,
            [5_200, 5_000, 4_700, 4_600, 4_400, 4_900],
        ),
        candidate(
            "qwen36-legal-sft-000",
            QwenLegalFullArtifactCandidateKind::Sft,
            Some("adapter.qwen36.legal.sft.000"),
            None,
            [7_100, 7_200, 6_900, 6_800, 6_600, 6_900],
        ),
        candidate(
            "qwen36-legal-dpo-000",
            QwenLegalFullArtifactCandidateKind::Dpo,
            Some("adapter.qwen36.legal.dpo.000"),
            None,
            [7_600, 7_900, 7_500, 7_300, 7_100, 7_500],
        ),
        candidate(
            "qwen36-legal-prior-champion-000",
            QwenLegalFullArtifactCandidateKind::PriorChampion,
            Some("adapter.qwen36.legal.champion.000"),
            None,
            [8_100, 8_300, 8_000, 7_900, 7_700, 8_000],
        ),
    ]
}

fn candidate(
    candidate_id: &str,
    kind: QwenLegalFullArtifactCandidateKind,
    adapter_id: Option<&str>,
    prior_champion_ref: Option<&str>,
    scores: [u32; 6],
) -> QwenLegalFullArtifactCandidate {
    let adapter_hash = adapter_id.map(|id| digest(format!("{id}.adapter")));
    QwenLegalFullArtifactCandidate {
        candidate_id: candidate_id.to_owned(),
        candidate_kind: kind,
        base_model_id: String::from("Qwen/Qwen3.6-35B-A3B"),
        adapter_id: adapter_id.map(str::to_owned),
        serving_route_id: Some(format!("psionic://qwen-legal/{candidate_id}")),
        prior_champion_ref: prior_champion_ref.map(str::to_owned),
        artifacts: artifact_refs(candidate_id, adapter_hash.as_deref()),
        lineage: QwenLegalFullArtifactLineage {
            corpus_manifest_hash: digest("corpus.harvey.legal.locked"),
            base_model_hash: digest("model.qwen36.35b.a3b"),
            model_checkpoint_hash: digest(format!("{candidate_id}.checkpoint")),
            adapter_hash,
            tokenizer_hash: digest("tokenizer.qwen36"),
            prompt_template_hash: digest("prompt.autopilot.blueprint.legal.v1"),
            training_config_hash: digest(format!("{candidate_id}.training.config")),
            run_receipt_hashes: vec![digest(format!("{candidate_id}.run.receipt"))],
            worker_receipt_hashes: vec![
                digest(format!("{candidate_id}.worker.0")),
                digest(format!("{candidate_id}.worker.1")),
            ],
            aggregate_receipt_hash: digest(format!("{candidate_id}.aggregate")),
            settlement: QwenLegalFullArtifactSettlementEvidence {
                settlement_state: String::from("settled"),
                accepted_work_count: 2,
                paid_accepted_work_count: 2,
                bitcoin_txids: vec![
                    format!("btc-tx-{candidate_id}-0"),
                    format!("btc-tx-{candidate_id}-1"),
                ],
                invoice_receipt_hashes: vec![
                    digest(format!("{candidate_id}.invoice.0")),
                    digest(format!("{candidate_id}.invoice.1")),
                ],
            },
        },
        eval_reports: eval_reports(candidate_id, scores),
    }
}

fn artifact_refs(candidate_id: &str, adapter_hash: Option<&str>) -> Vec<QwenLegalFullArtifactRef> {
    let mut artifacts = vec![
        artifact_ref(candidate_id, "corpus_manifest", "json", "corpus"),
        artifact_ref(candidate_id, "base_model_manifest", "json", "base"),
        artifact_ref(
            candidate_id,
            "model_checkpoint",
            "safetensors",
            "checkpoint",
        ),
        artifact_ref(candidate_id, "tokenizer", "json", "tokenizer"),
    ];
    if let Some(adapter_hash) = adapter_hash {
        artifacts.push(QwenLegalFullArtifactRef {
            artifact_id: format!("{candidate_id}.adapter"),
            artifact_kind: String::from("adapter_safetensors"),
            digest: adapter_hash.to_owned(),
            uri: format!("artifact://qwen-legal/{candidate_id}/adapter.safetensors"),
            byte_len: 4096,
        });
    }
    artifacts
}

fn artifact_ref(
    candidate_id: &str,
    artifact_kind: &str,
    extension: &str,
    seed: &str,
) -> QwenLegalFullArtifactRef {
    QwenLegalFullArtifactRef {
        artifact_id: format!("{candidate_id}.{artifact_kind}"),
        artifact_kind: artifact_kind.to_owned(),
        digest: digest(format!("{candidate_id}.{seed}")),
        uri: format!("artifact://qwen-legal/{candidate_id}/{artifact_kind}.{extension}"),
        byte_len: 1024,
    }
}

fn eval_reports(candidate_id: &str, scores: [u32; 6]) -> Vec<QwenLegalFullArtifactEvalReport> {
    [
        (
            QwenLegalFullArtifactPromotionStage::LocalSmoke,
            QwenLegalFullArtifactEvalVisibility::LocalSmoke,
            "local_smoke",
            scores[0],
        ),
        (
            QwenLegalFullArtifactPromotionStage::PublicTrainingSliceEval,
            QwenLegalFullArtifactEvalVisibility::PublicTrainingSlice,
            "harvey_public_training_slice",
            scores[1],
        ),
        (
            QwenLegalFullArtifactPromotionStage::PublicHeldOutEval,
            QwenLegalFullArtifactEvalVisibility::PublicHeldOut,
            "harvey_public_heldout",
            scores[2],
        ),
        (
            QwenLegalFullArtifactPromotionStage::PrivateEval,
            QwenLegalFullArtifactEvalVisibility::Private,
            "harvey_private_gate",
            scores[3],
        ),
        (
            QwenLegalFullArtifactPromotionStage::AdversarialHoldout,
            QwenLegalFullArtifactEvalVisibility::AdversarialHoldout,
            "harvey_adversarial_holdout",
            scores[4],
        ),
        (
            QwenLegalFullArtifactPromotionStage::ServingCanary,
            QwenLegalFullArtifactEvalVisibility::ServingCanary,
            "psionic_serving_canary",
            scores[5],
        ),
    ]
    .into_iter()
    .map(
        |(stage, visibility, suite_id, score_bps)| QwenLegalFullArtifactEvalReport {
            stage,
            visibility,
            suite_id: suite_id.to_owned(),
            suite_hash: digest(format!("{suite_id}.suite")),
            report_hash: digest(format!("{candidate_id}.{suite_id}.report")),
            score_bps,
            answer_file_success_rate_bps: 10_000,
            required_workflow_success_rate_bps: 10_000,
            integrity_failure_count: 0,
            tool_failure_count: 0,
            timeout_failure_count: 0,
            leakage_detected: false,
            answer_injection_detected: false,
            private_eval_train_contamination_detected: false,
            serving_mismatch_detected: false,
        },
    )
    .collect()
}

fn write_pretty_json<T>(path: &Path, value: &T) -> Result<(), QwenLegalFullArtifactPromotionError>
where
    T: Serialize,
{
    fs::write(path, serde_json::to_vec_pretty(value)?).map_err(|source| {
        QwenLegalFullArtifactPromotionError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn digest(seed: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_ref());
    hex::encode(hasher.finalize())
}

fn digest_is_complete(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn stable_json_digest<T>(
    namespace: &[u8],
    value: &T,
) -> Result<String, QwenLegalFullArtifactPromotionError>
where
    T: Serialize,
{
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    hasher.update(serde_json::to_vec(value)?);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_candidate() -> QwenLegalFullArtifactCandidate {
        rehearsal_candidate()
    }

    fn assert_refuses_with(
        mutate: impl FnOnce(&mut QwenLegalFullArtifactCandidate),
        failure: QwenLegalFullArtifactHardFailureClass,
    ) {
        let mut candidate = valid_candidate();
        mutate(&mut candidate);
        let report = evaluate_qwen_legal_full_artifact_promotion(
            "test.report",
            &candidate,
            &rehearsal_baselines(),
        )
        .expect("report");
        assert_eq!(
            report.decision,
            QwenLegalFullArtifactPromotionDecision::Reject
        );
        assert!(
            report.hard_failure_classes.contains(&failure),
            "expected {failure:?}, got {:?}",
            report.hard_failure_classes
        );
    }

    #[test]
    fn promotes_complete_grpo_candidate_against_required_comparisons() {
        let report = evaluate_qwen_legal_full_artifact_promotion(
            "test.report",
            &valid_candidate(),
            &rehearsal_baselines(),
        )
        .expect("report");
        assert_eq!(
            report.decision,
            QwenLegalFullArtifactPromotionDecision::Promote
        );
        assert!(report.required_comparison_kinds_present);
        assert_eq!(report.comparisons.len(), 4);
        assert!(
            report
                .comparisons
                .iter()
                .all(|row| row.candidate_beats_baseline)
        );
        assert!(!report.private_task_content_exported);
        assert!(report.autopilot4_summary_feed.private_eval_present);
        assert!(!report.autopilot4_summary_feed.private_score_exported);
        assert!(!report.autopilot4_summary_feed.private_task_content_exported);
        assert!(digest_is_complete(report.report_digest.as_str()));
    }

    #[test]
    fn refuses_missing_artifact() {
        assert_refuses_with(
            |candidate| candidate.artifacts.clear(),
            QwenLegalFullArtifactHardFailureClass::MissingArtifact,
        );
    }

    #[test]
    fn refuses_leakage() {
        assert_refuses_with(
            |candidate| candidate.eval_reports[0].leakage_detected = true,
            QwenLegalFullArtifactHardFailureClass::Leakage,
        );
    }

    #[test]
    fn refuses_answer_injection() {
        assert_refuses_with(
            |candidate| candidate.eval_reports[0].answer_injection_detected = true,
            QwenLegalFullArtifactHardFailureClass::AnswerInjection,
        );
    }

    #[test]
    fn refuses_private_eval_train_contamination() {
        assert_refuses_with(
            |candidate| {
                candidate.eval_reports[3].private_eval_train_contamination_detected = true;
            },
            QwenLegalFullArtifactHardFailureClass::PrivateEvalTrainContamination,
        );
    }

    #[test]
    fn refuses_invalid_worker_receipt() {
        assert_refuses_with(
            |candidate| candidate.lineage.worker_receipt_hashes[0] = String::from("bad"),
            QwenLegalFullArtifactHardFailureClass::InvalidWorkerReceipt,
        );
    }

    #[test]
    fn refuses_unpaid_accepted_work() {
        assert_refuses_with(
            |candidate| candidate.lineage.settlement.paid_accepted_work_count = 1,
            QwenLegalFullArtifactHardFailureClass::UnpaidAcceptedWork,
        );
    }

    #[test]
    fn refuses_serving_mismatch() {
        assert_refuses_with(
            |candidate| candidate.eval_reports[5].serving_mismatch_detected = true,
            QwenLegalFullArtifactHardFailureClass::ServingMismatch,
        );
    }

    #[test]
    fn writes_registry_report_and_public_summary_feed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = write_qwen_legal_full_artifact_promotion_rehearsal(
            &QwenLegalFullArtifactPromotionConfig {
                output_dir: temp.path().to_path_buf(),
                ..QwenLegalFullArtifactPromotionConfig::default()
            },
        )
        .expect("write rehearsal");
        assert_eq!(
            report.decision,
            QwenLegalFullArtifactPromotionDecision::Promote
        );
        assert!(
            temp.path()
                .join("qwen_legal_full_artifact_promotion_registry.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("autopilot4_qwen_legal_promotion_summary_feed.json")
                .exists()
        );
    }
}
