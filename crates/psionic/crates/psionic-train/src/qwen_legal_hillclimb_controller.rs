//! Qwen legal hillclimb experiment controller and public progress feed.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN_LEGAL_HILLCLIMB_PLAN_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_hillclimb_experiment_plan.v1";
pub const QWEN_LEGAL_HILLCLIMB_RUN_RECORD_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_hillclimb_run_record.v1";
pub const QWEN_LEGAL_HILLCLIMB_REGISTRY_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_hillclimb_registry.v1";
pub const QWEN_LEGAL_HILLCLIMB_PROGRESS_FEED_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_hillclimb_progress_feed.v1";
pub const QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION: &str = "psionic.qwen_legal_model_ladder.v1";
pub const QWEN_LEGAL_ACCEPTANCE_TARGETS_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_acceptance_targets.v1";

const DEFAULT_OUTPUT_DIR: &str = "target/legal/qwen_hillclimb";
const DEFAULT_REGISTRY_PATH: &str =
    "target/legal/qwen_hillclimb/qwen_legal_hillclimb_registry.json";
const DEFAULT_FEED_PATH: &str =
    "target/legal/qwen_hillclimb/autopilot4_qwen_legal_hillclimb_progress_feed.json";
const DEFAULT_REGISTRY_ID: &str = "qwen.legal.hillclimb.registry.local";
const DEFAULT_FEED_ID: &str = "autopilot4.qwen_legal.hillclimb.progress.local";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QwenLegalHillclimbControllerConfig {
    pub plan_path: PathBuf,
    pub registry_path: PathBuf,
    pub feed_path: PathBuf,
    pub output_dir: PathBuf,
    pub selected_rung: Option<String>,
}

impl QwenLegalHillclimbControllerConfig {
    pub fn new(plan_path: impl Into<PathBuf>) -> Self {
        Self {
            plan_path: plan_path.into(),
            registry_path: PathBuf::from(DEFAULT_REGISTRY_PATH),
            feed_path: PathBuf::from(DEFAULT_FEED_PATH),
            output_dir: PathBuf::from(DEFAULT_OUTPUT_DIR),
            selected_rung: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalModelLadderRung {
    pub schema_version: String,
    pub rung_name: String,
    pub model_id: String,
    pub adapter_target: String,
    pub why_it_exists: String,
    pub proves: String,
    pub does_not_prove: String,
    pub expected_memory: String,
    pub quantization_mode: String,
    pub pylon_count: u32,
    pub training_method: QwenLegalHillclimbTrainingMethod,
    pub acceptance_target: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalAcceptanceTarget {
    pub schema_version: String,
    pub split_name: String,
    pub baseline_score_bps: u32,
    pub first_credible_score_bps: u32,
    pub strong_model_threshold_bps: u32,
    pub near_perfect_threshold_bps: u32,
    pub max_critical_regression_bps: u32,
    pub plain_language_definition: String,
    pub stop_condition: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbTrainingMethod {
    Sft,
    Dpo,
    Grpo,
    HybridRl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbPromotionRule {
    PublicHeldoutNoRegression,
    RetainedSliceNoRegression,
    OperatorReviewOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbRunKind {
    Baseline,
    Candidate,
    RegressionCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbPromotionDecision {
    Promote,
    Hold,
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbScoreClaimLevel {
    PlumbingProof,
    PublicFixtureWin,
    HoldoutImprovement,
    StrongLegalModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbReportMode {
    TrainablePublicFixture,
    PublicHoldout,
    ModelOnly,
    BlueprintAssisted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbStopDecision {
    Hold,
    ContinueTraining,
    Rollback,
    Promote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalHillclimbRegistryRole {
    Champion,
    LatestCandidate,
    BestCandidate,
    Candidate,
    RejectedCandidate,
    Baseline,
    RegressionCheck,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbDeclaredRun {
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    pub score_bps: u32,
    pub data_split: String,
    pub worker_set: Vec<String>,
    pub payment_status: String,
    pub replay_command: String,
    pub report_path: String,
    pub trace_retained: bool,
    pub runner_added_answer_text_count: u32,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub train_only_improvement: bool,
    #[serde(default)]
    pub broad_benchmark_claim: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbRegressionCheck {
    pub check_id: String,
    pub evaluator_split: String,
    pub task_type: String,
    pub failure_category: String,
    pub critical: bool,
    pub baseline_score_bps: u32,
    pub candidate_score_bps: u32,
    pub max_allowed_regression_bps: u32,
    pub replay_command: String,
    pub report_path: String,
    pub trace_retained: bool,
    pub runner_added_answer_text_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbAcceptanceReportRef {
    pub report_mode: QwenLegalHillclimbReportMode,
    pub data_split: String,
    pub report_path: String,
    pub replay_command: String,
    pub trace_retained: bool,
    pub runner_added_answer_text_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbExperimentPlan {
    pub schema_version: String,
    pub experiment_id: String,
    pub model_ladder_rung: String,
    pub model_id: String,
    pub adapter_target: String,
    pub corpus_split: String,
    pub pylon_count: u32,
    pub training_method: QwenLegalHillclimbTrainingMethod,
    pub evaluator_split: String,
    pub promotion_rule: QwenLegalHillclimbPromotionRule,
    pub score_claim_level: QwenLegalHillclimbScoreClaimLevel,
    pub min_candidate_delta_bps: u32,
    pub max_regression_bps: u32,
    pub baseline: QwenLegalHillclimbDeclaredRun,
    pub candidate: QwenLegalHillclimbDeclaredRun,
    pub regression_checks: Vec<QwenLegalHillclimbRegressionCheck>,
    pub acceptance_reports: Vec<QwenLegalHillclimbAcceptanceReportRef>,
    pub plan_digest: String,
}

impl QwenLegalHillclimbExperimentPlan {
    pub fn stable_digest(&self) -> Result<String, QwenLegalHillclimbControllerError> {
        let mut clone = self.clone();
        clone.plan_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_hillclimb_experiment_plan|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbRunRecord {
    pub schema_version: String,
    pub run_id: String,
    pub experiment_id: String,
    pub run_kind: QwenLegalHillclimbRunKind,
    pub registry_role: QwenLegalHillclimbRegistryRole,
    pub model_ladder_rung: String,
    pub model_id: String,
    pub adapter_target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    pub corpus_split: String,
    pub evaluator_split: String,
    pub training_method: QwenLegalHillclimbTrainingMethod,
    pub pylon_count: u32,
    pub score_claim_level: QwenLegalHillclimbScoreClaimLevel,
    pub score_bps: u32,
    pub delta_vs_baseline_bps: i32,
    pub data_split: String,
    pub worker_set: Vec<String>,
    pub payment_status: String,
    pub replay_command: String,
    pub report_path: String,
    pub trace_retained: bool,
    pub runner_added_answer_text_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression_task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression_failure_category: Option<String>,
    pub critical_regression: bool,
    pub failed: bool,
    pub train_only_improvement: bool,
    pub presented_as_broad_benchmark_gain: bool,
    pub guardrail_passed: bool,
    pub promotion_decision: QwenLegalHillclimbPromotionDecision,
    pub stop_decision: QwenLegalHillclimbStopDecision,
    pub refusal_reasons: Vec<String>,
    pub record_index: u64,
    pub record_digest: String,
}

impl QwenLegalHillclimbRunRecord {
    pub fn stable_digest(&self) -> Result<String, QwenLegalHillclimbControllerError> {
        let mut clone = self.clone();
        clone.record_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_hillclimb_run_record|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbRegistry {
    pub schema_version: String,
    pub registry_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub champion_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_candidate_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_candidate_run_id: Option<String>,
    pub rejected_candidate_run_ids: Vec<String>,
    pub records: Vec<QwenLegalHillclimbRunRecord>,
    pub registry_digest: String,
}

impl QwenLegalHillclimbRegistry {
    pub fn empty() -> Self {
        Self {
            schema_version: String::from(QWEN_LEGAL_HILLCLIMB_REGISTRY_SCHEMA_VERSION),
            registry_id: String::from(DEFAULT_REGISTRY_ID),
            champion_run_id: None,
            latest_candidate_run_id: None,
            best_candidate_run_id: None,
            rejected_candidate_run_ids: Vec::new(),
            records: Vec::new(),
            registry_digest: String::new(),
        }
    }

    pub fn stable_digest(&self) -> Result<String, QwenLegalHillclimbControllerError> {
        let mut clone = self.clone();
        clone.registry_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_hillclimb_registry|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbFeedRunSummary {
    pub run_id: String,
    pub run_kind: QwenLegalHillclimbRunKind,
    pub registry_role: QwenLegalHillclimbRegistryRole,
    pub model_ladder_rung: String,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    pub score_claim_level: QwenLegalHillclimbScoreClaimLevel,
    pub score_bps: u32,
    pub delta_vs_baseline_bps: i32,
    pub payment_status: String,
    pub promotion_decision: QwenLegalHillclimbPromotionDecision,
    pub stop_decision: QwenLegalHillclimbStopDecision,
    pub refusal_reasons: Vec<String>,
    pub replay_command: String,
    pub report_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbScorePoint {
    pub record_index: u64,
    pub run_id: String,
    pub run_kind: QwenLegalHillclimbRunKind,
    pub model_ladder_rung: String,
    pub score_claim_level: QwenLegalHillclimbScoreClaimLevel,
    pub score_bps: u32,
    pub delta_vs_baseline_bps: i32,
    pub evaluator_split: String,
    pub report_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbProgressFeed {
    pub schema_version: String,
    pub feed_id: String,
    pub registry_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub champion_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_candidate_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_candidate_run_id: Option<String>,
    pub rejected_candidate_run_ids: Vec<String>,
    pub public_status_line: String,
    pub recent_runs: Vec<QwenLegalHillclimbFeedRunSummary>,
    pub score_history: Vec<QwenLegalHillclimbScorePoint>,
    pub hidden_or_retained_score_claim: bool,
    pub train_only_gain_exported_as_broad_benchmark: bool,
    pub feed_digest: String,
}

impl QwenLegalHillclimbProgressFeed {
    pub fn stable_digest(&self) -> Result<String, QwenLegalHillclimbControllerError> {
        let mut clone = self.clone();
        clone.feed_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_hillclimb_progress_feed|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalHillclimbControllerOutput {
    pub plan_path: String,
    pub run_records_path: String,
    pub registry_path: String,
    pub feed_path: String,
    pub candidate_run_id: String,
    pub model_ladder_rung: String,
    pub promotion_decision: QwenLegalHillclimbPromotionDecision,
    pub stop_decision: QwenLegalHillclimbStopDecision,
    pub refusal_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub champion_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_candidate_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_candidate_run_id: Option<String>,
    pub output_digest: String,
}

impl QwenLegalHillclimbControllerOutput {
    pub fn stable_digest(&self) -> Result<String, QwenLegalHillclimbControllerError> {
        let mut clone = self.clone();
        clone.output_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_hillclimb_controller_output|", &clone)
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalHillclimbControllerError {
    #[error("invalid Qwen legal hillclimb plan: {detail}")]
    InvalidPlan { detail: String },
    #[error("Qwen legal hillclimb I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen legal hillclimb JSON failed at `{path}`: {message}")]
    Json { path: String, message: String },
    #[error("Qwen legal hillclimb serialization failed: {message}")]
    Serialization { message: String },
}

pub fn qwen_legal_model_ladder() -> Vec<QwenLegalModelLadderRung> {
    vec![
        QwenLegalModelLadderRung {
            schema_version: String::from(QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION),
            rung_name: String::from("smoke-qwen35-08b"),
            model_id: String::from("Qwen/Qwen3.5-0.8B"),
            adapter_target: String::from("qwen35_08b_legal_lora"),
            why_it_exists: String::from("fast local correctness and command-surface tests"),
            proves: String::from("dataset, receipt, replay, scoring, and feed plumbing works"),
            does_not_prove: String::from("strong legal reasoning or retained benchmark quality"),
            expected_memory: String::from("2-6 GB local RAM or VRAM"),
            quantization_mode: String::from("q4/q8 serving or tiny LoRA smoke"),
            pylon_count: 1,
            training_method: QwenLegalHillclimbTrainingMethod::Sft,
            acceptance_target: String::from(
                "all schema, receipt, replay, and public-three regression checks pass",
            ),
        },
        QwenLegalModelLadderRung {
            schema_version: String::from(QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION),
            rung_name: String::from("plumbing-qwen35-4b"),
            model_id: String::from("Qwen/Qwen3.5-4B"),
            adapter_target: String::from("qwen35_legal_lora"),
            why_it_exists: String::from("small model for cheap multi-step plumbing beyond smoke"),
            proves: String::from("SFT, DPO, GRPO, Pylon, settlement, and promotion paths compose"),
            does_not_prove: String::from("production legal quality or strong-model behavior"),
            expected_memory: String::from("8-16 GB local RAM or VRAM"),
            quantization_mode: String::from("q4/q8 serving, LoRA training smoke"),
            pylon_count: 2,
            training_method: QwenLegalHillclimbTrainingMethod::Grpo,
            acceptance_target: String::from(
                "beats local smoke baseline without public-three regression",
            ),
        },
        QwenLegalModelLadderRung {
            schema_version: String::from(QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION),
            rung_name: String::from("dense-qwen36-27b"),
            model_id: String::from("Qwen/Qwen3.6-27B"),
            adapter_target: String::from("qwen36_legal_lora"),
            why_it_exists: String::from(
                "first serious dense legal target; checkpoint is local and avoids MoE router training",
            ),
            proves: String::from(
                "dense full-weight-adjacent legal adapter training can improve heldout legal tasks",
            ),
            does_not_prove: String::from("MoE router safety or very-large-model economics"),
            expected_memory: String::from("64-96 GB unified memory or accelerator memory"),
            quantization_mode: String::from("bf16/fp16 adapter training, q4/q8 serving checks"),
            pylon_count: 4,
            training_method: QwenLegalHillclimbTrainingMethod::Grpo,
            acceptance_target: String::from(
                "meets the configured hillclimb target and has zero public-three regression",
            ),
        },
        QwenLegalModelLadderRung {
            schema_version: String::from(QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION),
            rung_name: String::from("moe-qwen36-35b-a3b"),
            model_id: String::from("Qwen/Qwen3.6-35B-A3B"),
            adapter_target: String::from("qwen36_moe_legal_lora"),
            why_it_exists: String::from(
                "later sparse target after dense 27B training and MoE-safe serving are stable",
            ),
            proves: String::from(
                "active-expert sparse training and serving can preserve legal gains",
            ),
            does_not_prove: String::from("very-large distributed training reliability"),
            expected_memory: String::from("80-128 GB aggregate memory with MoE-aware serving"),
            quantization_mode: String::from("MoE-safe adapter training, q4/q8 serving rehearsal"),
            pylon_count: 6,
            training_method: QwenLegalHillclimbTrainingMethod::Grpo,
            acceptance_target: String::from(
                "beats dense 27B champion with no router, serving, or public regression failures",
            ),
        },
        QwenLegalModelLadderRung {
            schema_version: String::from(QWEN_LEGAL_MODEL_LADDER_SCHEMA_VERSION),
            rung_name: String::from("large-serving-eval-only"),
            model_id: String::from("Qwen large legal serving/eval target"),
            adapter_target: String::from("qwen_large_legal_eval"),
            why_it_exists: String::from(
                "serving and evaluation rehearsal after distributed training gates are reliable",
            ),
            proves: String::from(
                "promotion, rollback, and evaluation paths can handle large models",
            ),
            does_not_prove: String::from("that Psionic can train the very large model yet"),
            expected_memory: String::from(
                "160 GB+ aggregate memory, distributed serving preferred",
            ),
            quantization_mode: String::from(
                "serving quantization only until training gates mature",
            ),
            pylon_count: 8,
            training_method: QwenLegalHillclimbTrainingMethod::HybridRl,
            acceptance_target: String::from(
                "serving/eval parity first; training only after distributed payment and promotion gates are reliable",
            ),
        },
    ]
}

pub fn qwen_legal_model_ladder_rung(name: &str) -> Option<QwenLegalModelLadderRung> {
    qwen_legal_model_ladder()
        .into_iter()
        .find(|rung| rung.rung_name == name)
}

pub fn qwen_legal_acceptance_targets() -> Vec<QwenLegalAcceptanceTarget> {
    vec![
        QwenLegalAcceptanceTarget {
            schema_version: String::from(QWEN_LEGAL_ACCEPTANCE_TARGETS_SCHEMA_VERSION),
            split_name: String::from("trainable_public_fixture"),
            baseline_score_bps: 10_000,
            first_credible_score_bps: 10_000,
            strong_model_threshold_bps: 10_000,
            near_perfect_threshold_bps: 10_000,
            max_critical_regression_bps: 0,
            plain_language_definition: String::from(
                "plumbing proof only; this split can show the runner and trainer work but cannot support a strong-model claim",
            ),
            stop_condition: String::from(
                "continue training unless this split fails, leaks, or adds runner-written answer text",
            ),
        },
        QwenLegalAcceptanceTarget {
            schema_version: String::from(QWEN_LEGAL_ACCEPTANCE_TARGETS_SCHEMA_VERSION),
            split_name: String::from("public_heldout"),
            baseline_score_bps: 5_260,
            first_credible_score_bps: 5_310,
            strong_model_threshold_bps: 7_000,
            near_perfect_threshold_bps: 9_500,
            max_critical_regression_bps: 0,
            plain_language_definition: String::from(
                "holdout improvement; this is the first split that can support a serious legal-quality claim",
            ),
            stop_condition: String::from(
                "promote only above the declared threshold with zero critical regressions; continue training below threshold; roll back on critical regression",
            ),
        },
        QwenLegalAcceptanceTarget {
            schema_version: String::from(QWEN_LEGAL_ACCEPTANCE_TARGETS_SCHEMA_VERSION),
            split_name: String::from("model_only"),
            baseline_score_bps: 5_260,
            first_credible_score_bps: 5_310,
            strong_model_threshold_bps: 6_500,
            near_perfect_threshold_bps: 9_000,
            max_critical_regression_bps: 0,
            plain_language_definition: String::from(
                "model-only run; measures the adapter without Blueprint scaffold help",
            ),
            stop_condition: String::from(
                "continue training until model-only improvement clears the first credible bar and does not regress critical task/failure categories",
            ),
        },
        QwenLegalAcceptanceTarget {
            schema_version: String::from(QWEN_LEGAL_ACCEPTANCE_TARGETS_SCHEMA_VERSION),
            split_name: String::from("blueprint_assisted"),
            baseline_score_bps: 5_260,
            first_credible_score_bps: 5_310,
            strong_model_threshold_bps: 7_500,
            near_perfect_threshold_bps: 9_500,
            max_critical_regression_bps: 0,
            plain_language_definition: String::from(
                "Blueprint-assisted run; measures the model inside the intended legal-workflow scaffold",
            ),
            stop_condition: String::from(
                "promote only when holdout/model-only evidence is also clean; otherwise keep it as workflow evidence",
            ),
        },
    ]
}

pub fn qwen_legal_acceptance_target(split_name: &str) -> Option<QwenLegalAcceptanceTarget> {
    let normalized = normalize_split_name(split_name);
    qwen_legal_acceptance_targets()
        .into_iter()
        .find(|target| target.split_name == normalized)
}

pub fn run_qwen_legal_hillclimb_cli(
    args: &[String],
) -> Result<QwenLegalHillclimbControllerOutput, QwenLegalHillclimbControllerError> {
    let mut config: Option<QwenLegalHillclimbControllerConfig> = None;
    let mut registry_path: Option<PathBuf> = None;
    let mut feed_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut selected_rung: Option<String> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid_plan_error("missing value for --plan".to_owned()))?;
                config = Some(QwenLegalHillclimbControllerConfig::new(value));
                index += 2;
            }
            "--registry" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid_plan_error("missing value for --registry".to_owned()))?;
                registry_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--feed" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid_plan_error("missing value for --feed".to_owned()))?;
                feed_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--out" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid_plan_error("missing value for --out".to_owned()))?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--rung" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid_plan_error("missing value for --rung".to_owned()))?;
                selected_rung = Some(value.clone());
                index += 2;
            }
            "--help" | "-h" => {
                return Err(invalid_plan_error(
                    "usage: psionic-train qwen-legal-hillclimb --plan <plan.json> [--rung <rung-name>] [--registry <registry.json>] [--feed <feed.json>] [--out <dir>]".to_owned(),
                ));
            }
            other => {
                return Err(invalid_plan_error(format!(
                    "unsupported qwen legal hillclimb arg `{other}`"
                )));
            }
        }
    }

    let mut config = config.ok_or_else(|| {
        invalid_plan_error(
            "missing --plan <plan.json> for qwen legal hillclimb controller".to_owned(),
        )
    })?;
    if let Some(path) = registry_path {
        config.registry_path = path;
    }
    if let Some(path) = feed_path {
        config.feed_path = path;
    }
    if let Some(path) = output_dir {
        config.output_dir = path;
    }
    config.selected_rung = selected_rung;

    run_qwen_legal_hillclimb_controller(&config)
}

pub fn run_qwen_legal_hillclimb_controller(
    config: &QwenLegalHillclimbControllerConfig,
) -> Result<QwenLegalHillclimbControllerOutput, QwenLegalHillclimbControllerError> {
    let mut plan: QwenLegalHillclimbExperimentPlan = read_json(config.plan_path.as_path())?;
    if let Some(rung_name) = config.selected_rung.as_deref() {
        apply_model_ladder_rung(&mut plan, rung_name)?;
    }
    validate_and_finalize_plan(&mut plan)?;
    fs::create_dir_all(config.output_dir.as_path()).map_err(|error| {
        QwenLegalHillclimbControllerError::Io {
            path: config.output_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let mut registry = load_hillclimb_registry(config.registry_path.as_path())?;
    let starting_index = u64::try_from(registry.records.len()).unwrap_or(u64::MAX);
    let new_records = build_records_for_plan(&plan, starting_index)?;
    let candidate_record = new_records
        .iter()
        .find(|record| record.run_kind == QwenLegalHillclimbRunKind::Candidate)
        .ok_or_else(|| invalid_plan_error("controller did not build a candidate record"))?
        .clone();

    registry.records.extend(new_records.clone());
    refresh_registry_roles(&mut registry)?;

    let feed = build_progress_feed(&registry)?;
    let run_records_path = config
        .output_dir
        .join(format!("{}_run_records.json", plan.experiment_id));
    write_json(run_records_path.as_path(), &new_records)?;
    write_json(config.registry_path.as_path(), &registry)?;
    write_json(config.feed_path.as_path(), &feed)?;

    let mut output = QwenLegalHillclimbControllerOutput {
        plan_path: config.plan_path.display().to_string(),
        run_records_path: run_records_path.display().to_string(),
        registry_path: config.registry_path.display().to_string(),
        feed_path: config.feed_path.display().to_string(),
        candidate_run_id: candidate_record.run_id,
        model_ladder_rung: plan.model_ladder_rung,
        promotion_decision: candidate_record.promotion_decision,
        stop_decision: candidate_record.stop_decision,
        refusal_reasons: candidate_record.refusal_reasons,
        champion_run_id: registry.champion_run_id,
        latest_candidate_run_id: registry.latest_candidate_run_id,
        best_candidate_run_id: registry.best_candidate_run_id,
        output_digest: String::new(),
    };
    output.output_digest = output.stable_digest()?;
    Ok(output)
}

fn build_records_for_plan(
    plan: &QwenLegalHillclimbExperimentPlan,
    starting_index: u64,
) -> Result<Vec<QwenLegalHillclimbRunRecord>, QwenLegalHillclimbControllerError> {
    let baseline_score = plan.baseline.score_bps;
    let mut records = Vec::with_capacity(2 + plan.regression_checks.len());
    records.push(finalize_record(QwenLegalHillclimbRunRecord {
        schema_version: String::from(QWEN_LEGAL_HILLCLIMB_RUN_RECORD_SCHEMA_VERSION),
        run_id: plan.baseline.run_id.clone(),
        experiment_id: plan.experiment_id.clone(),
        run_kind: QwenLegalHillclimbRunKind::Baseline,
        registry_role: QwenLegalHillclimbRegistryRole::Baseline,
        model_ladder_rung: plan.model_ladder_rung.clone(),
        model_id: plan.model_id.clone(),
        adapter_target: plan.adapter_target.clone(),
        adapter_id: plan.baseline.adapter_id.clone(),
        corpus_split: plan.corpus_split.clone(),
        evaluator_split: plan.evaluator_split.clone(),
        training_method: plan.training_method,
        pylon_count: plan.pylon_count,
        score_claim_level: QwenLegalHillclimbScoreClaimLevel::PlumbingProof,
        score_bps: plan.baseline.score_bps,
        delta_vs_baseline_bps: 0,
        data_split: plan.baseline.data_split.clone(),
        worker_set: plan.baseline.worker_set.clone(),
        payment_status: plan.baseline.payment_status.clone(),
        replay_command: plan.baseline.replay_command.clone(),
        report_path: plan.baseline.report_path.clone(),
        trace_retained: plan.baseline.trace_retained,
        runner_added_answer_text_count: plan.baseline.runner_added_answer_text_count,
        regression_task_type: None,
        regression_failure_category: None,
        critical_regression: false,
        failed: plan.baseline.failed,
        train_only_improvement: plan.baseline.train_only_improvement,
        presented_as_broad_benchmark_gain: plan.baseline.broad_benchmark_claim,
        guardrail_passed: !plan.baseline.failed,
        promotion_decision: QwenLegalHillclimbPromotionDecision::Hold,
        stop_decision: QwenLegalHillclimbStopDecision::Hold,
        refusal_reasons: if plan.baseline.failed {
            vec![String::from("baseline run failed")]
        } else {
            Vec::new()
        },
        record_index: starting_index,
        record_digest: String::new(),
    })?);

    let candidate_refusals = candidate_refusal_reasons(plan);
    let candidate_stop_decision = candidate_stop_decision(plan, candidate_refusals.as_slice());
    let candidate_delta = score_delta(plan.candidate.score_bps, baseline_score);
    records.push(finalize_record(QwenLegalHillclimbRunRecord {
        schema_version: String::from(QWEN_LEGAL_HILLCLIMB_RUN_RECORD_SCHEMA_VERSION),
        run_id: plan.candidate.run_id.clone(),
        experiment_id: plan.experiment_id.clone(),
        run_kind: QwenLegalHillclimbRunKind::Candidate,
        registry_role: if candidate_refusals.is_empty() {
            QwenLegalHillclimbRegistryRole::LatestCandidate
        } else {
            QwenLegalHillclimbRegistryRole::RejectedCandidate
        },
        model_ladder_rung: plan.model_ladder_rung.clone(),
        model_id: plan.model_id.clone(),
        adapter_target: plan.adapter_target.clone(),
        adapter_id: plan.candidate.adapter_id.clone(),
        corpus_split: plan.corpus_split.clone(),
        evaluator_split: plan.evaluator_split.clone(),
        training_method: plan.training_method,
        pylon_count: plan.pylon_count,
        score_claim_level: plan.score_claim_level,
        score_bps: plan.candidate.score_bps,
        delta_vs_baseline_bps: candidate_delta,
        data_split: plan.candidate.data_split.clone(),
        worker_set: plan.candidate.worker_set.clone(),
        payment_status: plan.candidate.payment_status.clone(),
        replay_command: plan.candidate.replay_command.clone(),
        report_path: plan.candidate.report_path.clone(),
        trace_retained: plan.candidate.trace_retained,
        runner_added_answer_text_count: plan.candidate.runner_added_answer_text_count,
        regression_task_type: None,
        regression_failure_category: None,
        critical_regression: false,
        failed: plan.candidate.failed,
        train_only_improvement: plan.candidate.train_only_improvement,
        presented_as_broad_benchmark_gain: plan.candidate.broad_benchmark_claim,
        guardrail_passed: candidate_refusals.is_empty(),
        promotion_decision: if candidate_refusals.is_empty() {
            QwenLegalHillclimbPromotionDecision::Promote
        } else {
            QwenLegalHillclimbPromotionDecision::Reject
        },
        stop_decision: candidate_stop_decision,
        refusal_reasons: candidate_refusals,
        record_index: starting_index.saturating_add(1),
        record_digest: String::new(),
    })?);

    for (offset, check) in plan.regression_checks.iter().enumerate() {
        let regression_delta = score_delta(check.candidate_score_bps, check.baseline_score_bps);
        let regression_bps = check
            .baseline_score_bps
            .saturating_sub(check.candidate_score_bps);
        let max_allowed = plan
            .max_regression_bps
            .min(check.max_allowed_regression_bps);
        let critical_breach = check.critical && regression_bps > max_allowed;
        let refusal_reasons = if regression_bps > max_allowed {
            vec![format!(
                "regression check `{}` lost {regression_bps} bps; max allowed is {max_allowed} bps",
                check.check_id
            )]
        } else {
            Vec::new()
        };
        records.push(finalize_record(QwenLegalHillclimbRunRecord {
            schema_version: String::from(QWEN_LEGAL_HILLCLIMB_RUN_RECORD_SCHEMA_VERSION),
            run_id: check.check_id.clone(),
            experiment_id: plan.experiment_id.clone(),
            run_kind: QwenLegalHillclimbRunKind::RegressionCheck,
            registry_role: QwenLegalHillclimbRegistryRole::RegressionCheck,
            model_ladder_rung: plan.model_ladder_rung.clone(),
            model_id: plan.model_id.clone(),
            adapter_target: plan.adapter_target.clone(),
            adapter_id: plan.candidate.adapter_id.clone(),
            corpus_split: plan.corpus_split.clone(),
            evaluator_split: check.evaluator_split.clone(),
            training_method: plan.training_method,
            pylon_count: plan.pylon_count,
            score_claim_level: plan.score_claim_level,
            score_bps: check.candidate_score_bps,
            delta_vs_baseline_bps: regression_delta,
            data_split: check.evaluator_split.clone(),
            worker_set: plan.candidate.worker_set.clone(),
            payment_status: plan.candidate.payment_status.clone(),
            replay_command: check.replay_command.clone(),
            report_path: check.report_path.clone(),
            trace_retained: check.trace_retained,
            runner_added_answer_text_count: check.runner_added_answer_text_count,
            regression_task_type: Some(check.task_type.clone()),
            regression_failure_category: Some(check.failure_category.clone()),
            critical_regression: check.critical,
            failed: false,
            train_only_improvement: false,
            presented_as_broad_benchmark_gain: false,
            guardrail_passed: refusal_reasons.is_empty(),
            promotion_decision: if refusal_reasons.is_empty() {
                QwenLegalHillclimbPromotionDecision::Hold
            } else {
                QwenLegalHillclimbPromotionDecision::Reject
            },
            stop_decision: if critical_breach {
                QwenLegalHillclimbStopDecision::Rollback
            } else {
                QwenLegalHillclimbStopDecision::Hold
            },
            refusal_reasons,
            record_index: starting_index.saturating_add(2 + offset as u64),
            record_digest: String::new(),
        })?);
    }

    Ok(records)
}

fn candidate_refusal_reasons(plan: &QwenLegalHillclimbExperimentPlan) -> Vec<String> {
    let mut reasons = Vec::new();
    let candidate = &plan.candidate;
    if candidate.failed {
        reasons.push(String::from("candidate run failed"));
    }
    if candidate.train_only_improvement && candidate.broad_benchmark_claim {
        reasons.push(String::from(
            "train-only improvement cannot be exported as a broad benchmark gain",
        ));
    }
    if evaluator_is_training_split(plan.evaluator_split.as_str()) && candidate.broad_benchmark_claim
    {
        reasons.push(String::from(
            "training-split evaluator cannot support a broad benchmark gain claim",
        ));
    }
    if !payment_status_allows_promotion(candidate.payment_status.as_str()) {
        reasons.push(format!(
            "candidate payment status `{}` is not settled or operator-deferred",
            candidate.payment_status
        ));
    }
    if !candidate.trace_retained {
        reasons.push(String::from("candidate scored trace was not retained"));
    }
    if candidate.runner_added_answer_text_count > 0 {
        reasons.push(format!(
            "candidate has {} runner-added answer text events",
            candidate.runner_added_answer_text_count
        ));
    }
    if candidate.report_path.trim().is_empty() {
        reasons.push(String::from(
            "candidate score claim lacks a replayable report path",
        ));
    }
    let delta = score_delta(candidate.score_bps, plan.baseline.score_bps);
    if delta < i32::try_from(plan.min_candidate_delta_bps).unwrap_or(i32::MAX) {
        reasons.push(format!(
            "candidate delta {delta} bps is below required {} bps",
            plan.min_candidate_delta_bps
        ));
    }
    match plan.score_claim_level {
        QwenLegalHillclimbScoreClaimLevel::StrongLegalModel => {
            if !split_is_holdout(plan.evaluator_split.as_str())
                || !split_is_holdout(candidate.data_split.as_str())
            {
                reasons.push(String::from(
                    "strong legal model claim requires a holdout evaluator and candidate data split",
                ));
            }
            if let Some(target) = qwen_legal_acceptance_target(plan.evaluator_split.as_str()) {
                if candidate.score_bps < target.strong_model_threshold_bps {
                    reasons.push(format!(
                        "strong legal model claim score {} bps is below {} bps threshold for `{}`",
                        candidate.score_bps, target.strong_model_threshold_bps, target.split_name
                    ));
                }
            } else {
                reasons.push(format!(
                    "no acceptance target is defined for evaluator split `{}`",
                    plan.evaluator_split
                ));
            }
        }
        QwenLegalHillclimbScoreClaimLevel::HoldoutImprovement => {
            if !split_is_holdout(plan.evaluator_split.as_str())
                || !split_is_holdout(candidate.data_split.as_str())
            {
                reasons.push(String::from(
                    "holdout improvement claim requires a holdout evaluator and candidate data split",
                ));
            }
            if let Some(target) = qwen_legal_acceptance_target(plan.evaluator_split.as_str()) {
                if candidate.score_bps < target.first_credible_score_bps {
                    reasons.push(format!(
                        "holdout improvement score {} bps is below first credible target {} bps for `{}`",
                        candidate.score_bps,
                        target.first_credible_score_bps,
                        target.split_name
                    ));
                }
            }
        }
        QwenLegalHillclimbScoreClaimLevel::PublicFixtureWin => {
            if !normalize_split_name(plan.evaluator_split.as_str()).contains("fixture") {
                reasons.push(String::from(
                    "public fixture win claim must be reported on the trainable public fixture split",
                ));
            }
        }
        QwenLegalHillclimbScoreClaimLevel::PlumbingProof => {}
    }
    for check in &plan.regression_checks {
        let regression_bps = check
            .baseline_score_bps
            .saturating_sub(check.candidate_score_bps);
        let max_allowed = plan
            .max_regression_bps
            .min(check.max_allowed_regression_bps);
        if regression_bps > max_allowed {
            reasons.push(format!(
                "regression check `{}` ({}/{}) lost {regression_bps} bps; max allowed is {max_allowed} bps",
                check.check_id, check.task_type, check.failure_category
            ));
        }
    }
    reasons
}

fn candidate_stop_decision(
    plan: &QwenLegalHillclimbExperimentPlan,
    reasons: &[String],
) -> QwenLegalHillclimbStopDecision {
    if reasons.is_empty() {
        return QwenLegalHillclimbStopDecision::Promote;
    }
    if plan.candidate.failed
        || plan.candidate.runner_added_answer_text_count > 0
        || has_critical_regression_breach(plan)
    {
        return QwenLegalHillclimbStopDecision::Rollback;
    }
    let delta = score_delta(plan.candidate.score_bps, plan.baseline.score_bps);
    if delta < i32::try_from(plan.min_candidate_delta_bps).unwrap_or(i32::MAX)
        || reasons
            .iter()
            .any(|reason| reason.contains("below first credible target"))
    {
        return QwenLegalHillclimbStopDecision::ContinueTraining;
    }
    QwenLegalHillclimbStopDecision::Hold
}

fn has_critical_regression_breach(plan: &QwenLegalHillclimbExperimentPlan) -> bool {
    plan.regression_checks.iter().any(|check| {
        let regression_bps = check
            .baseline_score_bps
            .saturating_sub(check.candidate_score_bps);
        let max_allowed = plan
            .max_regression_bps
            .min(check.max_allowed_regression_bps);
        check.critical && regression_bps > max_allowed
    })
}

fn refresh_registry_roles(
    registry: &mut QwenLegalHillclimbRegistry,
) -> Result<(), QwenLegalHillclimbControllerError> {
    let mut rejected = BTreeSet::new();
    let mut best_candidate: Option<(String, u32, u64)> = None;
    let mut latest_candidate: Option<(String, u64)> = None;
    let mut champion: Option<(String, u64)> = None;

    for record in &registry.records {
        if record.run_kind == QwenLegalHillclimbRunKind::Candidate {
            latest_candidate = Some((record.run_id.clone(), record.record_index));
            if record.promotion_decision == QwenLegalHillclimbPromotionDecision::Reject {
                rejected.insert(record.run_id.clone());
            }
            let replace_best = best_candidate.as_ref().is_none_or(|(_, score, index)| {
                record.score_bps > *score
                    || (record.score_bps == *score && record.record_index > *index)
            });
            if record.guardrail_passed
                && record.promotion_decision != QwenLegalHillclimbPromotionDecision::Reject
                && replace_best
            {
                best_candidate =
                    Some((record.run_id.clone(), record.score_bps, record.record_index));
            }
            if record.promotion_decision == QwenLegalHillclimbPromotionDecision::Promote {
                champion = Some((record.run_id.clone(), record.record_index));
            }
        }
    }

    if champion.is_none() {
        champion = registry
            .records
            .iter()
            .filter(|record| record.run_kind == QwenLegalHillclimbRunKind::Baseline)
            .max_by_key(|record| record.record_index)
            .map(|record| (record.run_id.clone(), record.record_index));
    }

    registry.champion_run_id = champion.map(|(run_id, _)| run_id);
    registry.latest_candidate_run_id = latest_candidate.map(|(run_id, _)| run_id);
    registry.best_candidate_run_id = best_candidate.map(|(run_id, _, _)| run_id);
    registry.rejected_candidate_run_ids = rejected.into_iter().collect();
    for record in &mut registry.records {
        record.registry_role = registry_role_for_record(
            record,
            registry.champion_run_id.as_deref(),
            registry.latest_candidate_run_id.as_deref(),
            registry.best_candidate_run_id.as_deref(),
        );
        record.record_digest = String::new();
        record.record_digest = record.stable_digest()?;
    }
    registry.registry_digest = String::new();
    registry.registry_digest = registry.stable_digest()?;
    Ok(())
}

fn registry_role_for_record(
    record: &QwenLegalHillclimbRunRecord,
    champion_run_id: Option<&str>,
    latest_candidate_run_id: Option<&str>,
    best_candidate_run_id: Option<&str>,
) -> QwenLegalHillclimbRegistryRole {
    if champion_run_id == Some(record.run_id.as_str())
        && record.promotion_decision == QwenLegalHillclimbPromotionDecision::Promote
    {
        QwenLegalHillclimbRegistryRole::Champion
    } else if record.promotion_decision == QwenLegalHillclimbPromotionDecision::Reject
        && record.run_kind == QwenLegalHillclimbRunKind::Candidate
    {
        QwenLegalHillclimbRegistryRole::RejectedCandidate
    } else if latest_candidate_run_id == Some(record.run_id.as_str()) {
        QwenLegalHillclimbRegistryRole::LatestCandidate
    } else if best_candidate_run_id == Some(record.run_id.as_str()) {
        QwenLegalHillclimbRegistryRole::BestCandidate
    } else {
        match record.run_kind {
            QwenLegalHillclimbRunKind::Baseline => QwenLegalHillclimbRegistryRole::Baseline,
            QwenLegalHillclimbRunKind::Candidate => QwenLegalHillclimbRegistryRole::Candidate,
            QwenLegalHillclimbRunKind::RegressionCheck => {
                QwenLegalHillclimbRegistryRole::RegressionCheck
            }
        }
    }
}

fn build_progress_feed(
    registry: &QwenLegalHillclimbRegistry,
) -> Result<QwenLegalHillclimbProgressFeed, QwenLegalHillclimbControllerError> {
    let mut recent_records = registry.records.clone();
    recent_records.sort_by_key(|record| record.record_index);
    recent_records.reverse();
    recent_records.truncate(10);
    recent_records.reverse();

    let mut score_history = registry
        .records
        .iter()
        .map(|record| QwenLegalHillclimbScorePoint {
            record_index: record.record_index,
            run_id: record.run_id.clone(),
            run_kind: record.run_kind,
            model_ladder_rung: record.model_ladder_rung.clone(),
            score_claim_level: record.score_claim_level,
            score_bps: record.score_bps,
            delta_vs_baseline_bps: record.delta_vs_baseline_bps,
            evaluator_split: record.evaluator_split.clone(),
            report_path: record.report_path.clone(),
        })
        .collect::<Vec<_>>();
    score_history.sort_by_key(|point| point.record_index);

    let train_only_gain_exported_as_broad_benchmark = registry.records.iter().any(|record| {
        record.train_only_improvement
            && record.presented_as_broad_benchmark_gain
            && record.promotion_decision == QwenLegalHillclimbPromotionDecision::Promote
    });
    let public_status_line = public_status_line(registry);
    let mut feed = QwenLegalHillclimbProgressFeed {
        schema_version: String::from(QWEN_LEGAL_HILLCLIMB_PROGRESS_FEED_SCHEMA_VERSION),
        feed_id: String::from(DEFAULT_FEED_ID),
        registry_id: registry.registry_id.clone(),
        champion_run_id: registry.champion_run_id.clone(),
        latest_candidate_run_id: registry.latest_candidate_run_id.clone(),
        best_candidate_run_id: registry.best_candidate_run_id.clone(),
        rejected_candidate_run_ids: registry.rejected_candidate_run_ids.clone(),
        public_status_line,
        recent_runs: recent_records
            .iter()
            .map(|record| QwenLegalHillclimbFeedRunSummary {
                run_id: record.run_id.clone(),
                run_kind: record.run_kind,
                registry_role: record.registry_role,
                model_ladder_rung: record.model_ladder_rung.clone(),
                model_id: record.model_id.clone(),
                adapter_id: record.adapter_id.clone(),
                score_claim_level: record.score_claim_level,
                score_bps: record.score_bps,
                delta_vs_baseline_bps: record.delta_vs_baseline_bps,
                payment_status: record.payment_status.clone(),
                promotion_decision: record.promotion_decision,
                stop_decision: record.stop_decision,
                refusal_reasons: record.refusal_reasons.clone(),
                replay_command: record.replay_command.clone(),
                report_path: record.report_path.clone(),
            })
            .collect(),
        score_history,
        hidden_or_retained_score_claim: false,
        train_only_gain_exported_as_broad_benchmark,
        feed_digest: String::new(),
    };
    feed.feed_digest = feed.stable_digest()?;
    Ok(feed)
}

fn public_status_line(registry: &QwenLegalHillclimbRegistry) -> String {
    match (
        registry.champion_run_id.as_deref(),
        registry.latest_candidate_run_id.as_deref(),
    ) {
        (Some(champion), Some(latest)) if champion == latest => {
            format!("latest candidate `{latest}` is the current promoted Qwen legal champion")
        }
        (Some(champion), Some(latest)) => {
            format!("latest candidate `{latest}` recorded; current champion is `{champion}`")
        }
        (Some(champion), None) => {
            format!("current Qwen legal champion is `{champion}`")
        }
        _ => String::from("no Qwen legal hillclimb runs recorded yet"),
    }
}

fn validate_and_finalize_plan(
    plan: &mut QwenLegalHillclimbExperimentPlan,
) -> Result<(), QwenLegalHillclimbControllerError> {
    if plan.schema_version != QWEN_LEGAL_HILLCLIMB_PLAN_SCHEMA_VERSION {
        return Err(invalid_plan_error("plan schema_version drifted"));
    }
    require_nonempty(plan.experiment_id.as_str(), "experiment_id")?;
    require_nonempty(plan.model_ladder_rung.as_str(), "model_ladder_rung")?;
    require_nonempty(plan.model_id.as_str(), "model_id")?;
    require_nonempty(plan.adapter_target.as_str(), "adapter_target")?;
    require_nonempty(plan.corpus_split.as_str(), "corpus_split")?;
    require_nonempty(plan.evaluator_split.as_str(), "evaluator_split")?;
    if plan.pylon_count == 0 {
        return Err(invalid_plan_error("pylon_count must be greater than zero"));
    }
    validate_model_ladder_selection(plan)?;
    validate_declared_run(&plan.baseline, "baseline")?;
    validate_declared_run(&plan.candidate, "candidate")?;
    validate_acceptance_reports(plan.acceptance_reports.as_slice())?;
    for check in &plan.regression_checks {
        require_nonempty(check.check_id.as_str(), "regression check_id")?;
        require_nonempty(check.evaluator_split.as_str(), "regression evaluator_split")?;
        require_nonempty(check.task_type.as_str(), "regression task_type")?;
        require_nonempty(
            check.failure_category.as_str(),
            "regression failure_category",
        )?;
        require_nonempty(check.replay_command.as_str(), "regression replay_command")?;
        require_nonempty(check.report_path.as_str(), "regression report_path")?;
        if !check.trace_retained {
            return Err(invalid_plan_error(format!(
                "regression check `{}` must retain its scored trace",
                check.check_id
            )));
        }
        if check.runner_added_answer_text_count > 0 {
            return Err(invalid_plan_error(format!(
                "regression check `{}` has runner-added answer text",
                check.check_id
            )));
        }
        validate_score(check.baseline_score_bps, "regression baseline_score_bps")?;
        validate_score(check.candidate_score_bps, "regression candidate_score_bps")?;
    }
    let expected = plan.stable_digest()?;
    if plan.plan_digest.is_empty() {
        plan.plan_digest = expected;
    } else if plan.plan_digest != expected {
        return Err(invalid_plan_error("plan digest drifted"));
    }
    Ok(())
}

fn apply_model_ladder_rung(
    plan: &mut QwenLegalHillclimbExperimentPlan,
    rung_name: &str,
) -> Result<(), QwenLegalHillclimbControllerError> {
    let rung = qwen_legal_model_ladder_rung(rung_name).ok_or_else(|| {
        invalid_plan_error(format!(
            "unknown Qwen legal model ladder rung `{rung_name}`"
        ))
    })?;
    plan.model_ladder_rung = rung.rung_name;
    plan.model_id = rung.model_id;
    plan.adapter_target = rung.adapter_target;
    plan.pylon_count = rung.pylon_count;
    plan.training_method = rung.training_method;
    plan.plan_digest.clear();
    Ok(())
}

fn validate_model_ladder_selection(
    plan: &QwenLegalHillclimbExperimentPlan,
) -> Result<(), QwenLegalHillclimbControllerError> {
    let rung = qwen_legal_model_ladder_rung(plan.model_ladder_rung.as_str()).ok_or_else(|| {
        invalid_plan_error(format!(
            "unknown Qwen legal model ladder rung `{}`",
            plan.model_ladder_rung
        ))
    })?;
    if plan.model_id != rung.model_id {
        return Err(invalid_plan_error(format!(
            "model_id `{}` does not match ladder rung `{}` model `{}`",
            plan.model_id, rung.rung_name, rung.model_id
        )));
    }
    if plan.adapter_target != rung.adapter_target {
        return Err(invalid_plan_error(format!(
            "adapter_target `{}` does not match ladder rung `{}` target `{}`",
            plan.adapter_target, rung.rung_name, rung.adapter_target
        )));
    }
    if plan.pylon_count != rung.pylon_count {
        return Err(invalid_plan_error(format!(
            "pylon_count `{}` does not match ladder rung `{}` count `{}`",
            plan.pylon_count, rung.rung_name, rung.pylon_count
        )));
    }
    if plan.training_method != rung.training_method {
        return Err(invalid_plan_error(format!(
            "training_method does not match ladder rung `{}`",
            rung.rung_name
        )));
    }
    Ok(())
}

fn validate_declared_run(
    run: &QwenLegalHillclimbDeclaredRun,
    label: &str,
) -> Result<(), QwenLegalHillclimbControllerError> {
    require_nonempty(run.run_id.as_str(), format!("{label} run_id").as_str())?;
    require_nonempty(
        run.data_split.as_str(),
        format!("{label} data_split").as_str(),
    )?;
    require_nonempty(
        run.payment_status.as_str(),
        format!("{label} payment_status").as_str(),
    )?;
    require_nonempty(
        run.replay_command.as_str(),
        format!("{label} replay_command").as_str(),
    )?;
    require_nonempty(
        run.report_path.as_str(),
        format!("{label} report_path").as_str(),
    )?;
    if !run.trace_retained {
        return Err(invalid_plan_error(format!(
            "{label} scored trace must be retained"
        )));
    }
    if run.runner_added_answer_text_count > 0 {
        return Err(invalid_plan_error(format!(
            "{label} has runner-added answer text"
        )));
    }
    validate_score(run.score_bps, format!("{label} score_bps").as_str())?;
    if run.worker_set.is_empty() {
        return Err(invalid_plan_error(format!(
            "{label} worker_set must include at least one worker id"
        )));
    }
    Ok(())
}

fn validate_acceptance_reports(
    reports: &[QwenLegalHillclimbAcceptanceReportRef],
) -> Result<(), QwenLegalHillclimbControllerError> {
    let mut modes = BTreeSet::new();
    for report in reports {
        require_nonempty(report.data_split.as_str(), "acceptance report data_split")?;
        require_nonempty(report.report_path.as_str(), "acceptance report report_path")?;
        require_nonempty(
            report.replay_command.as_str(),
            "acceptance report replay_command",
        )?;
        if !report.trace_retained {
            return Err(invalid_plan_error(format!(
                "acceptance report {:?} must retain its scored trace",
                report.report_mode
            )));
        }
        if report.runner_added_answer_text_count > 0 {
            return Err(invalid_plan_error(format!(
                "acceptance report {:?} has runner-added answer text",
                report.report_mode
            )));
        }
        modes.insert(report.report_mode);
    }
    for required in [
        QwenLegalHillclimbReportMode::TrainablePublicFixture,
        QwenLegalHillclimbReportMode::PublicHoldout,
        QwenLegalHillclimbReportMode::ModelOnly,
        QwenLegalHillclimbReportMode::BlueprintAssisted,
    ] {
        if !modes.contains(&required) {
            return Err(invalid_plan_error(format!(
                "acceptance reports must include {:?}",
                required
            )));
        }
    }
    Ok(())
}

fn validate_score(score_bps: u32, field: &str) -> Result<(), QwenLegalHillclimbControllerError> {
    if score_bps > 10_000 {
        return Err(invalid_plan_error(format!(
            "{field} must be at most 10000 basis points"
        )));
    }
    Ok(())
}

fn finalize_record(
    mut record: QwenLegalHillclimbRunRecord,
) -> Result<QwenLegalHillclimbRunRecord, QwenLegalHillclimbControllerError> {
    record.record_digest = record.stable_digest()?;
    Ok(record)
}

fn load_hillclimb_registry(
    path: &Path,
) -> Result<QwenLegalHillclimbRegistry, QwenLegalHillclimbControllerError> {
    if !path.exists() {
        return Ok(QwenLegalHillclimbRegistry::empty());
    }
    let registry: QwenLegalHillclimbRegistry = read_json(path)?;
    if registry.schema_version != QWEN_LEGAL_HILLCLIMB_REGISTRY_SCHEMA_VERSION {
        return Err(invalid_plan_error("registry schema_version drifted"));
    }
    if !registry.registry_digest.is_empty()
        && registry.registry_digest != registry.stable_digest()?
    {
        return Err(invalid_plan_error("registry digest drifted"));
    }
    Ok(registry)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, QwenLegalHillclimbControllerError> {
    let bytes = fs::read(path).map_err(|error| QwenLegalHillclimbControllerError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| {
        QwenLegalHillclimbControllerError::Json {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    })
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), QwenLegalHillclimbControllerError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalHillclimbControllerError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        QwenLegalHillclimbControllerError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| QwenLegalHillclimbControllerError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn score_delta(candidate_score_bps: u32, baseline_score_bps: u32) -> i32 {
    i32::try_from(candidate_score_bps).unwrap_or(i32::MAX)
        - i32::try_from(baseline_score_bps).unwrap_or(i32::MAX)
}

fn evaluator_is_training_split(evaluator_split: &str) -> bool {
    let lower = evaluator_split.to_ascii_lowercase();
    lower.contains("train") || lower.contains("training")
}

fn split_is_holdout(split: &str) -> bool {
    let normalized = normalize_split_name(split);
    normalized.contains("holdout") || normalized.contains("heldout")
}

fn normalize_split_name(split: &str) -> String {
    split
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace("public_holdout", "public_heldout")
}

fn payment_status_allows_promotion(status: &str) -> bool {
    matches!(
        status,
        "settled" | "deferred" | "deferred_by_operator" | "operator_deferred"
    )
}

fn require_nonempty(value: &str, field: &str) -> Result<(), QwenLegalHillclimbControllerError> {
    if value.trim().is_empty() {
        return Err(invalid_plan_error(format!("{field} must be present")));
    }
    Ok(())
}

fn stable_json_digest<T: Serialize>(
    prefix: &[u8],
    value: &T,
) -> Result<String, QwenLegalHillclimbControllerError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        QwenLegalHillclimbControllerError::Serialization {
            message: error.to_string(),
        }
    })?;
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn invalid_plan_error(detail: impl Into<String>) -> QwenLegalHillclimbControllerError {
    QwenLegalHillclimbControllerError::InvalidPlan {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plan(
        dir: &Path,
        plan: &mut QwenLegalHillclimbExperimentPlan,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        validate_and_finalize_plan(plan)?;
        let path = dir.join(format!("{}.json", plan.experiment_id));
        write_json(path.as_path(), plan)?;
        Ok(path)
    }

    fn controller_config(dir: &Path, plan_path: PathBuf) -> QwenLegalHillclimbControllerConfig {
        QwenLegalHillclimbControllerConfig {
            plan_path,
            registry_path: dir.join("registry.json"),
            feed_path: dir.join("feed.json"),
            output_dir: dir.join("out"),
            selected_rung: None,
        }
    }

    fn canonical_plan(experiment_id: &str) -> QwenLegalHillclimbExperimentPlan {
        QwenLegalHillclimbExperimentPlan {
            schema_version: String::from(QWEN_LEGAL_HILLCLIMB_PLAN_SCHEMA_VERSION),
            experiment_id: String::from(experiment_id),
            model_ladder_rung: String::from("dense-qwen36-27b"),
            model_id: String::from("Qwen/Qwen3.6-27B"),
            adapter_target: String::from("qwen36_legal_lora"),
            corpus_split: String::from("harvey_public_training_slice_v1"),
            pylon_count: 4,
            training_method: QwenLegalHillclimbTrainingMethod::Grpo,
            evaluator_split: String::from("public_heldout"),
            promotion_rule: QwenLegalHillclimbPromotionRule::PublicHeldoutNoRegression,
            score_claim_level: QwenLegalHillclimbScoreClaimLevel::HoldoutImprovement,
            min_candidate_delta_bps: 50,
            max_regression_bps: 25,
            baseline: QwenLegalHillclimbDeclaredRun {
                run_id: format!("{experiment_id}.baseline"),
                adapter_id: Some(String::from("qwen36-legal-prior-champion")),
                score_bps: 5_260,
                data_split: String::from("public_heldout"),
                worker_set: vec![String::from("pylon.legal.baseline.01")],
                payment_status: String::from("settled"),
                replay_command: String::from("psionic-train qwen-legal-hillclimb baseline"),
                report_path: String::from("target/legal/qwen_hillclimb/baseline_report.json"),
                trace_retained: true,
                runner_added_answer_text_count: 0,
                failed: false,
                train_only_improvement: false,
                broad_benchmark_claim: false,
            },
            candidate: QwenLegalHillclimbDeclaredRun {
                run_id: format!("{experiment_id}.candidate"),
                adapter_id: Some(format!("{experiment_id}.adapter")),
                score_bps: 5_380,
                data_split: String::from("public_heldout"),
                worker_set: vec![
                    String::from("pylon.legal.candidate.01"),
                    String::from("pylon.legal.candidate.02"),
                ],
                payment_status: String::from("settled"),
                replay_command: String::from("psionic-train qwen-legal-hillclimb candidate"),
                report_path: String::from("target/legal/qwen_hillclimb/candidate_report.json"),
                trace_retained: true,
                runner_added_answer_text_count: 0,
                failed: false,
                train_only_improvement: false,
                broad_benchmark_claim: true,
            },
            regression_checks: vec![QwenLegalHillclimbRegressionCheck {
                check_id: format!("{experiment_id}.regression.public_three"),
                evaluator_split: String::from("public_three_regression"),
                task_type: String::from("legal_work_product"),
                failure_category: String::from("public_three_regression"),
                critical: true,
                baseline_score_bps: 10_000,
                candidate_score_bps: 10_000,
                max_allowed_regression_bps: 0,
                replay_command: String::from(
                    "psionic-train qwen-legal-hillclimb regression public-three",
                ),
                report_path: String::from(
                    "target/legal/qwen_hillclimb/public_three_regression_report.json",
                ),
                trace_retained: true,
                runner_added_answer_text_count: 0,
            }],
            acceptance_reports: vec![
                QwenLegalHillclimbAcceptanceReportRef {
                    report_mode: QwenLegalHillclimbReportMode::TrainablePublicFixture,
                    data_split: String::from("trainable_public_fixture"),
                    report_path: String::from(
                        "target/legal/qwen_hillclimb/trainable_public_fixture_report.json",
                    ),
                    replay_command: String::from(
                        "psionic-train qwen-legal-hillclimb report trainable-public-fixture",
                    ),
                    trace_retained: true,
                    runner_added_answer_text_count: 0,
                },
                QwenLegalHillclimbAcceptanceReportRef {
                    report_mode: QwenLegalHillclimbReportMode::PublicHoldout,
                    data_split: String::from("public_heldout"),
                    report_path: String::from(
                        "target/legal/qwen_hillclimb/public_holdout_report.json",
                    ),
                    replay_command: String::from(
                        "psionic-train qwen-legal-hillclimb report public-holdout",
                    ),
                    trace_retained: true,
                    runner_added_answer_text_count: 0,
                },
                QwenLegalHillclimbAcceptanceReportRef {
                    report_mode: QwenLegalHillclimbReportMode::ModelOnly,
                    data_split: String::from("model_only"),
                    report_path: String::from("target/legal/qwen_hillclimb/model_only_report.json"),
                    replay_command: String::from(
                        "psionic-train qwen-legal-hillclimb report model-only",
                    ),
                    trace_retained: true,
                    runner_added_answer_text_count: 0,
                },
                QwenLegalHillclimbAcceptanceReportRef {
                    report_mode: QwenLegalHillclimbReportMode::BlueprintAssisted,
                    data_split: String::from("blueprint_assisted"),
                    report_path: String::from(
                        "target/legal/qwen_hillclimb/blueprint_assisted_report.json",
                    ),
                    replay_command: String::from(
                        "psionic-train qwen-legal-hillclimb report blueprint-assisted",
                    ),
                    trace_retained: true,
                    runner_added_answer_text_count: 0,
                },
            ],
            plan_digest: String::new(),
        }
    }

    #[test]
    fn hillclimb_score_comparison_promotes_candidate() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.promote");
        let plan_path = write_plan(temp.path(), &mut plan)?;
        let output =
            run_qwen_legal_hillclimb_controller(&controller_config(temp.path(), plan_path))?;

        assert_eq!(
            output.promotion_decision,
            QwenLegalHillclimbPromotionDecision::Promote
        );
        assert_eq!(
            output.champion_run_id.as_deref(),
            Some("hillclimb.promote.candidate")
        );
        let registry: QwenLegalHillclimbRegistry =
            read_json(temp.path().join("registry.json").as_path())?;
        assert_eq!(
            registry.latest_candidate_run_id.as_deref(),
            Some("hillclimb.promote.candidate")
        );
        assert_eq!(
            registry.best_candidate_run_id.as_deref(),
            Some("hillclimb.promote.candidate")
        );
        Ok(())
    }

    #[test]
    fn hillclimb_refuses_train_only_broad_gain() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.refuse");
        plan.candidate.score_bps = 9_000;
        plan.candidate.train_only_improvement = true;
        plan.candidate.broad_benchmark_claim = true;
        plan.plan_digest.clear();
        let plan_path = write_plan(temp.path(), &mut plan)?;
        let output =
            run_qwen_legal_hillclimb_controller(&controller_config(temp.path(), plan_path))?;

        assert_eq!(
            output.promotion_decision,
            QwenLegalHillclimbPromotionDecision::Reject
        );
        assert!(
            output
                .refusal_reasons
                .iter()
                .any(|reason| reason.contains("train-only improvement"))
        );
        let registry: QwenLegalHillclimbRegistry =
            read_json(temp.path().join("registry.json").as_path())?;
        assert_eq!(
            registry.rejected_candidate_run_ids,
            vec![String::from("hillclimb.refuse.candidate")]
        );
        Ok(())
    }

    #[test]
    fn hillclimb_refuses_strong_claim_without_holdout_improvement()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.strong.refuse");
        plan.score_claim_level = QwenLegalHillclimbScoreClaimLevel::StrongLegalModel;
        plan.evaluator_split = String::from("trainable_public_fixture");
        plan.candidate.data_split = String::from("trainable_public_fixture");
        plan.candidate.score_bps = 10_000;
        plan.plan_digest.clear();
        let plan_path = write_plan(temp.path(), &mut plan)?;
        let output =
            run_qwen_legal_hillclimb_controller(&controller_config(temp.path(), plan_path))?;

        assert_eq!(
            output.promotion_decision,
            QwenLegalHillclimbPromotionDecision::Reject
        );
        assert_eq!(output.stop_decision, QwenLegalHillclimbStopDecision::Hold);
        assert!(
            output
                .refusal_reasons
                .iter()
                .any(|reason| reason.contains("requires a holdout evaluator"))
        );
        Ok(())
    }

    #[test]
    fn hillclimb_rolls_back_on_critical_regression() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.critical.regression");
        plan.candidate.score_bps = 9_500;
        plan.regression_checks[0].candidate_score_bps = 9_900;
        plan.plan_digest.clear();
        let plan_path = write_plan(temp.path(), &mut plan)?;
        let output =
            run_qwen_legal_hillclimb_controller(&controller_config(temp.path(), plan_path))?;

        assert_eq!(
            output.promotion_decision,
            QwenLegalHillclimbPromotionDecision::Reject
        );
        assert_eq!(
            output.stop_decision,
            QwenLegalHillclimbStopDecision::Rollback
        );
        assert!(
            output
                .refusal_reasons
                .iter()
                .any(|reason| reason.contains("legal_work_product/public_three_regression"))
        );
        Ok(())
    }

    #[test]
    fn hillclimb_registry_appends_runs() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut first = canonical_plan("hillclimb.append.first");
        let first_path = write_plan(temp.path(), &mut first)?;
        let config = controller_config(temp.path(), first_path);
        run_qwen_legal_hillclimb_controller(&config)?;

        let mut second = canonical_plan("hillclimb.append.second");
        second.candidate.score_bps = 5_410;
        second.plan_digest.clear();
        let second_path = write_plan(temp.path(), &mut second)?;
        let second_config = QwenLegalHillclimbControllerConfig {
            plan_path: second_path,
            ..config
        };
        run_qwen_legal_hillclimb_controller(&second_config)?;

        let registry: QwenLegalHillclimbRegistry =
            read_json(temp.path().join("registry.json").as_path())?;
        assert_eq!(registry.records.len(), 6);
        assert_eq!(
            registry.champion_run_id.as_deref(),
            Some("hillclimb.append.second.candidate")
        );
        assert_eq!(registry.registry_digest, registry.stable_digest()?);
        Ok(())
    }

    #[test]
    fn hillclimb_export_feed_has_recent_runs_and_history() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.feed");
        let plan_path = write_plan(temp.path(), &mut plan)?;
        run_qwen_legal_hillclimb_controller(&controller_config(temp.path(), plan_path))?;

        let feed: QwenLegalHillclimbProgressFeed =
            read_json(temp.path().join("feed.json").as_path())?;
        assert_eq!(
            feed.schema_version,
            QWEN_LEGAL_HILLCLIMB_PROGRESS_FEED_SCHEMA_VERSION
        );
        assert_eq!(feed.recent_runs.len(), 3);
        assert_eq!(feed.score_history.len(), 3);
        assert_eq!(
            feed.latest_candidate_run_id.as_deref(),
            Some("hillclimb.feed.candidate")
        );
        assert!(!feed.hidden_or_retained_score_claim);
        assert!(!feed.train_only_gain_exported_as_broad_benchmark);
        assert_eq!(feed.feed_digest, feed.stable_digest()?);
        Ok(())
    }

    #[test]
    fn hillclimb_controller_selects_ladder_rung_by_name() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut plan = canonical_plan("hillclimb.rung");
        plan.model_ladder_rung = String::from("smoke-qwen35-08b");
        plan.model_id = String::from("Qwen/Qwen3.5-0.8B");
        plan.adapter_target = String::from("qwen35_08b_legal_lora");
        plan.pylon_count = 1;
        plan.training_method = QwenLegalHillclimbTrainingMethod::Sft;
        plan.plan_digest.clear();
        let plan_path = write_plan(temp.path(), &mut plan)?;
        let mut config = controller_config(temp.path(), plan_path);
        config.selected_rung = Some(String::from("dense-qwen36-27b"));

        run_qwen_legal_hillclimb_controller(&config)?;

        let registry: QwenLegalHillclimbRegistry =
            read_json(temp.path().join("registry.json").as_path())?;
        let candidate = registry
            .records
            .iter()
            .find(|record| record.run_kind == QwenLegalHillclimbRunKind::Candidate)
            .expect("candidate record");
        assert_eq!(candidate.model_ladder_rung, "dense-qwen36-27b");
        assert_eq!(candidate.model_id, "Qwen/Qwen3.6-27B");
        assert_eq!(candidate.pylon_count, 4);
        Ok(())
    }
}
