use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_legal_reward_trace_from_run_dir, run_record_digest, score_report_digest,
    CoverageSnapshot, CriterionFailureClass, LegalRewardTraceError, LegalVerifierRewardTrace,
    Metadata, RunRecord, RunTerminalState, ScoreReport, TranscriptEventKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN_LEGAL_RL_ROLLOUT_BATCH_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_rollout_batch.v1";
pub const QWEN_LEGAL_RL_ROLLOUT_RECORD_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_rollout_record.v1";
pub const QWEN_LEGAL_RL_REWARD_COMPONENT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_reward_components.v1";
pub const QWEN_LEGAL_RL_DPO_PAIR_SCHEMA_VERSION: &str = "psionic.qwen_legal_rl_dpo_pair.v1";
pub const QWEN_LEGAL_RL_GRPO_SEED_SCHEMA_VERSION: &str = "psionic.qwen_legal_rl_grpo_seed.v1";
pub const QWEN_LEGAL_RL_TASK_IMPROVEMENT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_task_improvement.v1";
pub const QWEN_LEGAL_RL_FAILURE_IMPROVEMENT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_failure_improvement.v1";

const DEFAULT_BATCH_ID: &str = "qwen-legal-rl-rollouts-001";
const DEFAULT_RUNS_ROOT: &str = "fixtures/qwen_legal/real_finetune/harvey_no_cheat_suite_plain_text_shim_after_lora_2026_05_20_025";
const DEFAULT_OUTPUT_DIR: &str = "target/legal/qwen_rl_rollouts/batch-001";
const DEFAULT_SEED: &str = "qwen-legal-rl-rollout-seed-001";
const DEFAULT_BASE_MODEL_ID: &str = "Qwen/Qwen3.5-0.8B";
const DEFAULT_MODEL_ARTIFACT_DIGEST: &str =
    "sha256:qwen35-08b-mlx-lora-local-run-artifacts-2026-05-20";
const DEFAULT_SCORE_THRESHOLD_BPS: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QwenLegalRlRolloutBatchConfig {
    pub batch_id: String,
    pub runs_root: PathBuf,
    pub output_dir: PathBuf,
    pub seed: String,
    pub base_model_id: String,
    pub model_artifact_digest: String,
    pub adapter_artifact_digest: Option<String>,
    pub served_route_id: Option<String>,
    pub accepted_score_threshold_bps: u32,
}

impl Default for QwenLegalRlRolloutBatchConfig {
    fn default() -> Self {
        Self {
            batch_id: String::from(DEFAULT_BATCH_ID),
            runs_root: PathBuf::from(DEFAULT_RUNS_ROOT),
            output_dir: PathBuf::from(DEFAULT_OUTPUT_DIR),
            seed: String::from(DEFAULT_SEED),
            base_model_id: String::from(DEFAULT_BASE_MODEL_ID),
            model_artifact_digest: String::from(DEFAULT_MODEL_ARTIFACT_DIGEST),
            adapter_artifact_digest: None,
            served_route_id: None,
            accepted_score_threshold_bps: DEFAULT_SCORE_THRESHOLD_BPS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalRlRolloutClass {
    Accepted,
    Rejected,
    Quarantined,
    AdversarialHoldout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalRlRolloutSourceKind {
    ServedQwenRoute,
    LocalQwenInferencePath,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlRewardComponents {
    pub schema_version: String,
    pub file_discipline: f32,
    pub document_coverage: f32,
    pub citation_evidence: f32,
    pub legal_reasoning: f32,
    pub spreadsheet_reasoning: f32,
    pub missing_facts: f32,
    pub pre_submit_self_check: f32,
    pub no_runner_output_mutation: f32,
    pub total_reward: f32,
    pub component_digest: String,
}

impl QwenLegalRlRewardComponents {
    pub fn stable_digest(&self) -> Result<String, QwenLegalRlRolloutBatchError> {
        let mut clone = self.clone();
        clone.component_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_rl_reward_components|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlArtifactRef {
    pub artifact_kind: String,
    pub source_path: String,
    pub preserved_path: String,
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlBlueprintScaffoldRef {
    pub declared_input_present: bool,
    pub declared_input_digest: Option<String>,
    pub source: String,
    pub runner_may_add_answer_text: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlRolloutRecord {
    pub schema_version: String,
    pub rollout_id: String,
    pub rollout_class: QwenLegalRlRolloutClass,
    pub source_kind: QwenLegalRlRolloutSourceKind,
    pub source_run_dir: String,
    pub preserved_run_dir: String,
    pub run_id: String,
    pub task_id: String,
    pub task_version: String,
    pub base_model_id: String,
    pub observed_model_id: String,
    pub served_route_id: String,
    pub model_artifact_digest: String,
    pub adapter_artifact_digest: Option<String>,
    pub raw_response_hashes: Vec<String>,
    pub run_record_hash: String,
    pub score_report_hash: Option<String>,
    pub reward_trace_hash: String,
    pub enhanced_reward_hash: String,
    pub legal_content_score_bps: u32,
    pub all_pass: bool,
    pub total_reward: f32,
    pub enhanced_reward: QwenLegalRlRewardComponents,
    pub blueprint_scaffold: QwenLegalRlBlueprintScaffoldRef,
    pub failure_labels: Vec<String>,
    pub training_eligible: bool,
    pub negative_training_eligible: bool,
    pub runner_output_mutation_allowed: bool,
    pub runner_added_answer_text_detected: bool,
    pub transcript_event_count: usize,
    pub tool_call_count: usize,
    pub artifact_refs: Vec<QwenLegalRlArtifactRef>,
    pub rollout_digest: String,
}

impl QwenLegalRlRolloutRecord {
    pub fn stable_digest(&self) -> Result<String, QwenLegalRlRolloutBatchError> {
        let mut clone = self.clone();
        clone.rollout_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_rl_rollout_record|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlDpoPair {
    pub schema_version: String,
    pub pair_id: String,
    pub task_id: String,
    pub chosen_rollout_id: String,
    pub rejected_rollout_id: String,
    pub chosen_reward: f32,
    pub rejected_reward: f32,
    pub reward_delta: f32,
    pub trainable: bool,
    pub reason: String,
    pub pair_digest: String,
}

impl QwenLegalRlDpoPair {
    pub fn stable_digest(&self) -> Result<String, QwenLegalRlRolloutBatchError> {
        let mut clone = self.clone();
        clone.pair_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_rl_dpo_pair|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlGrpoSeed {
    pub schema_version: String,
    pub seed_id: String,
    pub task_id: String,
    pub rollout_ids: Vec<String>,
    pub rewards: Vec<f32>,
    pub trainable_rollout_classes: Vec<QwenLegalRlRolloutClass>,
    pub deterministic_group_digest: String,
}

impl QwenLegalRlGrpoSeed {
    pub fn stable_digest(&self) -> Result<String, QwenLegalRlRolloutBatchError> {
        let mut clone = self.clone();
        clone.deterministic_group_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_rl_grpo_seed|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlTaskImprovementRow {
    pub schema_version: String,
    pub task_id: String,
    pub rollout_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub quarantined_count: usize,
    pub adversarial_holdout_count: usize,
    pub negative_training_count: usize,
    pub best_rollout_id: String,
    pub best_rollout_class: QwenLegalRlRolloutClass,
    pub best_score_bps: u32,
    pub best_reward: f32,
    pub worst_rollout_id: String,
    pub worst_rollout_class: QwenLegalRlRolloutClass,
    pub worst_score_bps: u32,
    pub worst_reward: f32,
    pub score_delta_bps: i64,
    pub reward_delta: f32,
    pub failure_labels_observed: Vec<String>,
    pub preference_training_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlFailureImprovementRow {
    pub schema_version: String,
    pub failure_label: String,
    pub affected_rollout_count: usize,
    pub affected_task_count: usize,
    pub negative_training_count: usize,
    pub quarantined_count: usize,
    pub worst_rollout_id: String,
    pub worst_task_id: String,
    pub worst_score_bps: u32,
    pub worst_reward: f32,
    pub best_counterexample_rollout_id: Option<String>,
    pub best_counterexample_task_id: Option<String>,
    pub best_counterexample_score_bps: Option<u32>,
    pub best_counterexample_reward: Option<f32>,
    pub score_gap_bps: Option<i64>,
    pub reward_gap: Option<f32>,
    pub training_action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlRolloutClassCounts {
    pub accepted: usize,
    pub rejected: usize,
    pub quarantined: usize,
    pub adversarial_holdout: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRlRolloutBatchReport {
    pub schema_version: String,
    pub batch_id: String,
    pub seed: String,
    pub runs_root: String,
    pub output_dir: String,
    pub source_kind: QwenLegalRlRolloutSourceKind,
    pub base_model_id: String,
    pub model_artifact_digest: String,
    pub adapter_artifact_digest: Option<String>,
    pub accepted_score_threshold_bps: u32,
    pub rollout_record_path: String,
    pub reward_trace_path: String,
    pub enhanced_reward_path: String,
    pub dpo_pair_path: String,
    pub grpo_seed_path: String,
    pub task_improvement_path: String,
    pub failure_improvement_path: String,
    pub preserved_artifacts_dir: String,
    pub rollout_count: usize,
    pub class_counts: QwenLegalRlRolloutClassCounts,
    pub dpo_pair_count: usize,
    pub grpo_seed_count: usize,
    pub task_improvement_count: usize,
    pub failure_improvement_count: usize,
    pub bad_completion_count: usize,
    pub quarantined_rollout_ids: Vec<String>,
    pub deterministic_rollout_ids: Vec<String>,
    pub runner_added_answer_text_count: usize,
    pub blueprint_scaffold_count: usize,
    pub hidden_or_private_performance_claim: bool,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl QwenLegalRlRolloutBatchReport {
    pub fn stable_digest(&self) -> Result<String, QwenLegalRlRolloutBatchError> {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_rl_rollout_batch|", &clone)
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalRlRolloutBatchError {
    #[error("invalid Qwen legal RL rollout batch config: {0}")]
    InvalidConfig(String),
    #[error("Qwen legal RL rollout batch I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Qwen legal RL rollout batch JSON failed at `{path}`: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Qwen legal RL reward trace failed: {0}")]
    RewardTrace(#[from] LegalRewardTraceError),
    #[error("Qwen legal RL serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("no Qwen legal run artifacts found under `{0}`")]
    NoRuns(PathBuf),
}

pub fn run_qwen_legal_rl_rollout_cli(
    args: &[String],
) -> Result<QwenLegalRlRolloutBatchReport, QwenLegalRlRolloutBatchError> {
    let config = parse_rollout_batch_args(args)?;
    run_qwen_legal_rl_rollout_batch(&config)
}

pub fn run_qwen_legal_rl_rollout_batch(
    config: &QwenLegalRlRolloutBatchConfig,
) -> Result<QwenLegalRlRolloutBatchReport, QwenLegalRlRolloutBatchError> {
    validate_config(config)?;
    fs::create_dir_all(&config.output_dir).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: config.output_dir.clone(),
        source,
    })?;

    let mut discovered = Vec::new();
    discover_candidate_dirs(config.runs_root.as_path(), &mut discovered)?;
    discovered.sort();
    if discovered.is_empty() {
        return Err(QwenLegalRlRolloutBatchError::NoRuns(
            config.runs_root.clone(),
        ));
    }

    let mut prepared = discovered
        .iter()
        .map(|source| prepare_rollout(config, source))
        .collect::<Result<Vec<_>, _>>()?;
    let best_by_task = best_score_by_task(prepared.as_slice());
    for prepared_rollout in &mut prepared {
        prepared_rollout.class = classify_rollout(prepared_rollout, &best_by_task, config);
    }
    prepared.sort_by(|left, right| {
        left.run_record
            .task_id
            .cmp(&right.run_record.task_id)
            .then_with(|| left.rollout_id.cmp(&right.rollout_id))
    });

    let mut records = prepared
        .iter()
        .map(|prepared_rollout| rollout_record(config, prepared_rollout))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|left, right| {
        left.task_id
            .cmp(&right.task_id)
            .then_with(|| left.rollout_id.cmp(&right.rollout_id))
    });

    let dpo_pairs = build_dpo_pairs(records.as_slice())?;
    let grpo_seeds = build_grpo_seeds(records.as_slice())?;
    let task_improvements = build_task_improvement_rows(records.as_slice());
    let failure_improvements = build_failure_improvement_rows(records.as_slice());
    let reward_traces = prepared
        .iter()
        .map(|prepared_rollout| prepared_rollout.reward_trace.clone())
        .collect::<Vec<_>>();
    let enhanced_rewards = records
        .iter()
        .map(|record| record.enhanced_reward.clone())
        .collect::<Vec<_>>();

    let rollout_record_path = config.output_dir.join("rollout_records.jsonl");
    let reward_trace_path = config.output_dir.join("reward_traces.jsonl");
    let enhanced_reward_path = config.output_dir.join("enhanced_rewards.jsonl");
    let dpo_pair_path = config.output_dir.join("dpo_pairs.jsonl");
    let grpo_seed_path = config.output_dir.join("grpo_seeds.jsonl");
    let task_improvement_path = config.output_dir.join("task_improvements.json");
    let failure_improvement_path = config.output_dir.join("failure_improvements.json");
    write_jsonl(rollout_record_path.as_path(), records.as_slice())?;
    write_jsonl(reward_trace_path.as_path(), reward_traces.as_slice())?;
    write_jsonl(enhanced_reward_path.as_path(), enhanced_rewards.as_slice())?;
    write_jsonl(dpo_pair_path.as_path(), dpo_pairs.as_slice())?;
    write_jsonl(grpo_seed_path.as_path(), grpo_seeds.as_slice())?;
    write_json(task_improvement_path.as_path(), &task_improvements)?;
    write_json(failure_improvement_path.as_path(), &failure_improvements)?;

    let class_counts = class_counts(records.as_slice());
    let mut report = QwenLegalRlRolloutBatchReport {
        schema_version: String::from(QWEN_LEGAL_RL_ROLLOUT_BATCH_SCHEMA_VERSION),
        batch_id: config.batch_id.clone(),
        seed: config.seed.clone(),
        runs_root: config.runs_root.display().to_string(),
        output_dir: config.output_dir.display().to_string(),
        source_kind: QwenLegalRlRolloutSourceKind::LocalQwenInferencePath,
        base_model_id: config.base_model_id.clone(),
        model_artifact_digest: config.model_artifact_digest.clone(),
        adapter_artifact_digest: config.adapter_artifact_digest.clone(),
        accepted_score_threshold_bps: config.accepted_score_threshold_bps,
        rollout_record_path: rollout_record_path.display().to_string(),
        reward_trace_path: reward_trace_path.display().to_string(),
        enhanced_reward_path: enhanced_reward_path.display().to_string(),
        dpo_pair_path: dpo_pair_path.display().to_string(),
        grpo_seed_path: grpo_seed_path.display().to_string(),
        task_improvement_path: task_improvement_path.display().to_string(),
        failure_improvement_path: failure_improvement_path.display().to_string(),
        preserved_artifacts_dir: preserved_root(config).display().to_string(),
        rollout_count: records.len(),
        class_counts,
        dpo_pair_count: dpo_pairs.len(),
        grpo_seed_count: grpo_seeds.len(),
        task_improvement_count: task_improvements.len(),
        failure_improvement_count: failure_improvements.len(),
        bad_completion_count: records
            .iter()
            .filter(|record| record.negative_training_eligible)
            .count(),
        quarantined_rollout_ids: records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Quarantined)
            .map(|record| record.rollout_id.clone())
            .collect(),
        deterministic_rollout_ids: records
            .iter()
            .map(|record| record.rollout_id.clone())
            .collect(),
        runner_added_answer_text_count: records
            .iter()
            .filter(|record| record.runner_added_answer_text_detected)
            .count(),
        blueprint_scaffold_count: records
            .iter()
            .filter(|record| record.blueprint_scaffold.declared_input_present)
            .count(),
        hidden_or_private_performance_claim: false,
        claim_boundary: String::from(
            "This batch turns preserved local Qwen legal run artifacts into RL/DPO/GRPO data. It is not a hidden or retained Harvey score claim. The runner records and scores model-written outputs, but it does not add answer text.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest()?;
    write_json(
        config.output_dir.join("batch_report.json").as_path(),
        &report,
    )?;

    Ok(report)
}

#[derive(Clone, Debug)]
struct PreparedRollout {
    source_dir: PathBuf,
    preserved_dir: PathBuf,
    rollout_id: String,
    class: QwenLegalRlRolloutClass,
    run_record: RunRecord,
    score_report: Option<ScoreReport>,
    reward_trace: LegalVerifierRewardTrace,
    artifact_refs: Vec<QwenLegalRlArtifactRef>,
}

fn parse_rollout_batch_args(
    args: &[String],
) -> Result<QwenLegalRlRolloutBatchConfig, QwenLegalRlRolloutBatchError> {
    let mut config = QwenLegalRlRolloutBatchConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--runs-root" => {
                index += 1;
                config.runs_root = PathBuf::from(required_arg(args, index, "--runs-root")?);
            }
            "--out" | "--output-dir" => {
                index += 1;
                config.output_dir = PathBuf::from(required_arg(args, index, "--out")?);
            }
            "--batch-id" => {
                index += 1;
                config.batch_id = required_arg(args, index, "--batch-id")?.to_string();
            }
            "--seed" => {
                index += 1;
                config.seed = required_arg(args, index, "--seed")?.to_string();
            }
            "--base-model" => {
                index += 1;
                config.base_model_id = required_arg(args, index, "--base-model")?.to_string();
            }
            "--model-artifact-digest" => {
                index += 1;
                config.model_artifact_digest =
                    required_arg(args, index, "--model-artifact-digest")?.to_string();
            }
            "--adapter-artifact-digest" => {
                index += 1;
                config.adapter_artifact_digest =
                    Some(required_arg(args, index, "--adapter-artifact-digest")?.to_string());
            }
            "--served-route-id" => {
                index += 1;
                config.served_route_id =
                    Some(required_arg(args, index, "--served-route-id")?.to_string());
            }
            "--accepted-score-threshold-bps" => {
                index += 1;
                let value = required_arg(args, index, "--accepted-score-threshold-bps")?;
                config.accepted_score_threshold_bps = value.parse::<u32>().map_err(|source| {
                    QwenLegalRlRolloutBatchError::InvalidConfig(format!(
                        "--accepted-score-threshold-bps must be an integer: {source}"
                    ))
                })?;
            }
            "--help" | "-h" => {
                return Err(QwenLegalRlRolloutBatchError::InvalidConfig(String::from(
                    "usage: psionic-train qwen-legal-rl-rollouts [--runs-root <path>] [--out <dir>] [--batch-id <id>] [--seed <seed>]",
                )));
            }
            other => {
                return Err(QwenLegalRlRolloutBatchError::InvalidConfig(format!(
                    "unknown argument `{other}`"
                )));
            }
        }
        index += 1;
    }
    Ok(config)
}

fn required_arg<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, QwenLegalRlRolloutBatchError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| QwenLegalRlRolloutBatchError::InvalidConfig(format!("{flag} needs a value")))
}

fn validate_config(
    config: &QwenLegalRlRolloutBatchConfig,
) -> Result<(), QwenLegalRlRolloutBatchError> {
    require_nonempty(config.batch_id.as_str(), "batch_id")?;
    require_nonempty(config.seed.as_str(), "seed")?;
    require_nonempty(config.base_model_id.as_str(), "base_model_id")?;
    require_nonempty(
        config.model_artifact_digest.as_str(),
        "model_artifact_digest",
    )?;
    if !config.runs_root.is_dir() {
        return Err(QwenLegalRlRolloutBatchError::InvalidConfig(format!(
            "runs_root `{}` is not a directory",
            config.runs_root.display()
        )));
    }
    Ok(())
}

fn require_nonempty(value: &str, field: &str) -> Result<(), QwenLegalRlRolloutBatchError> {
    if value.trim().is_empty() {
        return Err(QwenLegalRlRolloutBatchError::InvalidConfig(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn discover_candidate_dirs(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), QwenLegalRlRolloutBatchError> {
    if dir.join("run").join("run_record.json").is_file() || dir.join("run_record.json").is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| QwenLegalRlRolloutBatchError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            discover_candidate_dirs(path.as_path(), out)?;
        }
    }
    Ok(())
}

fn prepare_rollout(
    config: &QwenLegalRlRolloutBatchConfig,
    source_dir: &Path,
) -> Result<PreparedRollout, QwenLegalRlRolloutBatchError> {
    let run_artifact_dir = if source_dir.join("run").join("run_record.json").is_file() {
        source_dir.join("run")
    } else {
        source_dir.to_path_buf()
    };
    let run_record_path = run_artifact_dir.join("run_record.json");
    let score_report_path = score_report_path(source_dir, run_artifact_dir.as_path());
    let run_record = read_json::<RunRecord>(run_record_path.as_path())?;
    let score_report = read_optional_json::<ScoreReport>(score_report_path.as_ref())?;
    let run_hash = run_record_digest(&run_record)?;
    let rollout_id = deterministic_rollout_id(config, &run_record, run_hash.as_str());
    let preserved_dir = preserved_root(config).join(&rollout_id);
    let staged_reward_dir = config
        .output_dir
        .join("reward_stage")
        .join(rollout_id.as_str());
    fs::create_dir_all(&preserved_dir).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: preserved_dir.clone(),
        source,
    })?;
    fs::create_dir_all(&staged_reward_dir).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: staged_reward_dir.clone(),
        source,
    })?;

    let artifact_refs = preserve_artifacts(
        source_dir,
        run_artifact_dir.as_path(),
        preserved_dir.as_path(),
        staged_reward_dir.as_path(),
    )?;
    let reward_trace = build_legal_reward_trace_from_run_dir(staged_reward_dir.as_path())?;

    Ok(PreparedRollout {
        source_dir: source_dir.to_path_buf(),
        preserved_dir,
        rollout_id,
        class: QwenLegalRlRolloutClass::Rejected,
        run_record,
        score_report,
        reward_trace,
        artifact_refs,
    })
}

fn score_report_path(source_dir: &Path, run_artifact_dir: &Path) -> Option<PathBuf> {
    for candidate in [
        source_dir.join("score_report.json"),
        run_artifact_dir.join("score_report.json"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn preserve_artifacts(
    source_dir: &Path,
    run_artifact_dir: &Path,
    preserved_dir: &Path,
    staged_reward_dir: &Path,
) -> Result<Vec<QwenLegalRlArtifactRef>, QwenLegalRlRolloutBatchError> {
    let candidates = [
        ("run_record", run_artifact_dir.join("run_record.json"), true),
        (
            "transcript",
            run_artifact_dir.join("transcript.jsonl"),
            false,
        ),
        (
            "run_receipt",
            run_artifact_dir.join("run_receipt.json"),
            false,
        ),
        (
            "output_manifest",
            first_existing_path(
                run_artifact_dir,
                &["output_manifest.json", "output_artifact_manifest.json"],
            ),
            false,
        ),
        (
            "tool_receipts",
            run_artifact_dir.join("tool_receipts.json"),
            false,
        ),
        (
            "score_report",
            score_report_path(source_dir, run_artifact_dir)
                .unwrap_or_else(|| source_dir.join("score_report.json")),
            false,
        ),
    ];
    let mut refs = Vec::new();
    for (kind, source_path, required) in candidates {
        if !source_path.is_file() {
            if required {
                return Err(QwenLegalRlRolloutBatchError::InvalidConfig(format!(
                    "required artifact `{}` is missing",
                    source_path.display()
                )));
            }
            continue;
        }
        let preserved_path = preserved_dir.join(file_name_for_kind(kind, source_path.as_path()));
        copy_file(source_path.as_path(), preserved_path.as_path())?;
        stage_reward_artifact(kind, source_path.as_path(), staged_reward_dir)?;
        let metadata = fs::metadata(source_path.as_path()).map_err(|source| {
            QwenLegalRlRolloutBatchError::Io {
                path: source_path.clone(),
                source,
            }
        })?;
        refs.push(QwenLegalRlArtifactRef {
            artifact_kind: kind.to_string(),
            source_path: source_path.display().to_string(),
            preserved_path: preserved_path.display().to_string(),
            sha256: sha256_file(source_path.as_path())?,
            byte_len: metadata.len(),
        });
    }
    refs.sort_by(|left, right| left.artifact_kind.cmp(&right.artifact_kind));
    Ok(refs)
}

fn first_existing_path(dir: &Path, names: &[&str]) -> PathBuf {
    for name in names {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    dir.join(names[0])
}

fn file_name_for_kind(kind: &str, source_path: &Path) -> String {
    match kind {
        "output_manifest" => String::from("output_manifest.json"),
        _ => source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(kind)
            .to_string(),
    }
}

fn stage_reward_artifact(
    kind: &str,
    source_path: &Path,
    staged_reward_dir: &Path,
) -> Result<(), QwenLegalRlRolloutBatchError> {
    let target_name = match kind {
        "run_record" => Some("run_record.json"),
        "score_report" => Some("score_report.json"),
        "run_receipt" => Some("run_receipt.json"),
        "output_manifest" => Some("output_manifest.json"),
        _ => None,
    };
    if let Some(target_name) = target_name {
        copy_file(source_path, staged_reward_dir.join(target_name).as_path())?;
    }
    Ok(())
}

fn copy_file(source: &Path, dest: &Path) -> Result<(), QwenLegalRlRolloutBatchError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalRlRolloutBatchError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::copy(source, dest).map_err(|source_error| QwenLegalRlRolloutBatchError::Io {
        path: dest.to_path_buf(),
        source: source_error,
    })?;
    Ok(())
}

fn best_score_by_task(prepared: &[PreparedRollout]) -> BTreeMap<String, u32> {
    let mut best = BTreeMap::<String, u32>::new();
    for rollout in prepared {
        let score = rollout
            .score_report
            .as_ref()
            .map(|report| report.criterion_pass_rate_bps)
            .unwrap_or(rollout.reward_trace.legal_content_score_bps);
        best.entry(rollout.run_record.task_id.clone())
            .and_modify(|current| *current = (*current).max(score))
            .or_insert(score);
    }
    best
}

fn classify_rollout(
    rollout: &PreparedRollout,
    best_by_task: &BTreeMap<String, u32>,
    config: &QwenLegalRlRolloutBatchConfig,
) -> QwenLegalRlRolloutClass {
    if rollout.reward_trace.fatal_excluded
        || runner_output_mutation_allowed(&rollout.run_record, rollout.score_report.as_ref())
        || runner_added_answer_text_detected(&rollout.run_record)
    {
        return QwenLegalRlRolloutClass::Quarantined;
    }
    if is_adversarial_holdout(
        &rollout.source_dir,
        &rollout.run_record,
        rollout.score_report.as_ref(),
    ) {
        return QwenLegalRlRolloutClass::AdversarialHoldout;
    }
    let score = rollout
        .score_report
        .as_ref()
        .map(|report| report.criterion_pass_rate_bps)
        .unwrap_or(rollout.reward_trace.legal_content_score_bps);
    let best = best_by_task
        .get(rollout.run_record.task_id.as_str())
        .copied()
        .unwrap_or(score);
    if score >= config.accepted_score_threshold_bps && score == best {
        QwenLegalRlRolloutClass::Accepted
    } else {
        QwenLegalRlRolloutClass::Rejected
    }
}

fn is_adversarial_holdout(
    source_dir: &Path,
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
) -> bool {
    let source = source_dir.display().to_string().to_ascii_lowercase();
    source.contains("adversarial")
        || source.contains("holdout")
        || metadata_flag(&run_record.metadata, "adversarial_holdout")
        || metadata_flag(&run_record.metadata, "holdout")
        || score_report.is_some_and(|report| {
            metadata_flag(&report.metadata, "adversarial_holdout")
                || metadata_flag(&report.metadata, "holdout")
        })
}

fn rollout_record(
    config: &QwenLegalRlRolloutBatchConfig,
    rollout: &PreparedRollout,
) -> Result<QwenLegalRlRolloutRecord, QwenLegalRlRolloutBatchError> {
    let score_report_hash = rollout
        .score_report
        .as_ref()
        .map(score_report_digest)
        .transpose()?;
    let model_id = observed_model_id(&rollout.run_record, rollout.score_report.as_ref())
        .unwrap_or_else(|| config.base_model_id.clone());
    let route_id = config
        .served_route_id
        .clone()
        .or_else(|| observed_route_id(&rollout.run_record, rollout.score_report.as_ref()))
        .unwrap_or_else(|| String::from("route.qwen.local.inference_artifacts"));
    let runner_output_mutation_allowed =
        runner_output_mutation_allowed(&rollout.run_record, rollout.score_report.as_ref());
    let runner_added_answer_text_detected = runner_added_answer_text_detected(&rollout.run_record);
    let enhanced_reward = enhanced_reward_components(
        &rollout.reward_trace,
        &rollout.run_record,
        rollout.score_report.as_ref(),
        !runner_output_mutation_allowed && !runner_added_answer_text_detected,
    )?;
    let failure_labels = failure_labels(
        &rollout.reward_trace,
        &rollout.run_record,
        rollout.score_report.as_ref(),
        runner_output_mutation_allowed,
        runner_added_answer_text_detected,
    );
    let training_eligible = matches!(
        rollout.class,
        QwenLegalRlRolloutClass::Accepted | QwenLegalRlRolloutClass::Rejected
    ) && !runner_output_mutation_allowed
        && !runner_added_answer_text_detected
        && !rollout.reward_trace.fatal_excluded;
    let negative_training_eligible =
        rollout.class == QwenLegalRlRolloutClass::Rejected && training_eligible;
    let blueprint_scaffold =
        blueprint_scaffold_ref(&rollout.run_record, rollout.score_report.as_ref());
    let mut record = QwenLegalRlRolloutRecord {
        schema_version: String::from(QWEN_LEGAL_RL_ROLLOUT_RECORD_SCHEMA_VERSION),
        rollout_id: rollout.rollout_id.clone(),
        rollout_class: rollout.class,
        source_kind: QwenLegalRlRolloutSourceKind::LocalQwenInferencePath,
        source_run_dir: rollout.source_dir.display().to_string(),
        preserved_run_dir: rollout.preserved_dir.display().to_string(),
        run_id: rollout.run_record.run_id.clone(),
        task_id: rollout.run_record.task_id.clone(),
        task_version: rollout.run_record.task_version.clone(),
        base_model_id: config.base_model_id.clone(),
        observed_model_id: model_id,
        served_route_id: route_id,
        model_artifact_digest: config.model_artifact_digest.clone(),
        adapter_artifact_digest: config.adapter_artifact_digest.clone(),
        raw_response_hashes: raw_response_hashes(&rollout.run_record),
        run_record_hash: run_record_digest(&rollout.run_record)?,
        score_report_hash,
        reward_trace_hash: rollout.reward_trace.trace_digest.clone(),
        enhanced_reward_hash: enhanced_reward.component_digest.clone(),
        legal_content_score_bps: rollout
            .score_report
            .as_ref()
            .map(|report| report.criterion_pass_rate_bps)
            .unwrap_or(rollout.reward_trace.legal_content_score_bps),
        all_pass: rollout
            .score_report
            .as_ref()
            .map(|report| report.all_pass)
            .unwrap_or(false),
        total_reward: rollout.reward_trace.total_reward,
        enhanced_reward,
        blueprint_scaffold,
        failure_labels,
        training_eligible,
        negative_training_eligible,
        runner_output_mutation_allowed,
        runner_added_answer_text_detected,
        transcript_event_count: rollout.run_record.transcript.len(),
        tool_call_count: rollout.run_record.tool_calls.len(),
        artifact_refs: rollout.artifact_refs.clone(),
        rollout_digest: String::new(),
    };
    record.rollout_digest = record.stable_digest()?;
    Ok(record)
}

fn enhanced_reward_components(
    trace: &LegalVerifierRewardTrace,
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
    no_runner_output_mutation: bool,
) -> Result<QwenLegalRlRewardComponents, QwenLegalRlRolloutBatchError> {
    let workflow = &trace.components;
    let file_discipline = mean([
        workflow.wrote_required_file,
        workflow.correct_path,
        workflow.non_empty_answer,
        workflow.answer_length_ok,
        workflow.submitted_ok,
        workflow.integrity_valid,
    ]);
    let coverage = score_report
        .map(|report| report.document_coverage_bps as f32 / 10_000.0)
        .filter(|value| *value > 0.0)
        .unwrap_or(workflow.source_usage_ok);
    let citation_evidence = citation_evidence_reward(run_record, score_report);
    let legal_reasoning = score_report
        .map(|report| report.criterion_pass_rate_bps as f32 / 10_000.0)
        .unwrap_or(trace.legal_content_reward);
    let spreadsheet_reasoning = spreadsheet_reasoning_reward(run_record, score_report);
    let missing_facts = missing_facts_reward(score_report);
    let pre_submit_self_check = pre_submit_self_check_reward(run_record, score_report);
    let no_runner_output_mutation = bool_reward(no_runner_output_mutation);
    let total_reward = file_discipline * 2.0
        + coverage
        + citation_evidence
        + legal_reasoning * 3.0
        + spreadsheet_reasoning
        + missing_facts
        + pre_submit_self_check
        + no_runner_output_mutation * 3.0;
    let mut components = QwenLegalRlRewardComponents {
        schema_version: String::from(QWEN_LEGAL_RL_REWARD_COMPONENT_SCHEMA_VERSION),
        file_discipline,
        document_coverage: coverage,
        citation_evidence,
        legal_reasoning,
        spreadsheet_reasoning,
        missing_facts,
        pre_submit_self_check,
        no_runner_output_mutation,
        total_reward,
        component_digest: String::new(),
    };
    components.component_digest = components.stable_digest()?;
    Ok(components)
}

fn citation_evidence_reward(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> f32 {
    if coverage_snapshot(run_record, score_report)
        .is_some_and(|snapshot| !snapshot.evidence_refs.is_empty())
    {
        return 1.0;
    }
    if score_report.is_some_and(|report| {
        report
            .criterion_results
            .iter()
            .any(|result| !result.evidence_refs.is_empty())
    }) {
        return 1.0;
    }
    bool_reward(
        model_text(run_record)
            .to_ascii_lowercase()
            .contains("source"),
    )
}

fn spreadsheet_reasoning_reward(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> f32 {
    let task_text = format!(
        "{} {} {}",
        run_record.task_id,
        model_text(run_record),
        score_report
            .map(|report| serde_json::to_string(report).unwrap_or_default())
            .unwrap_or_default()
    )
    .to_ascii_lowercase();
    let requires_spreadsheet = task_text.contains(".xlsx")
        || task_text.contains("spreadsheet")
        || task_text.contains("schedule")
        || task_text.contains("calculation");
    if !requires_spreadsheet {
        return 1.0;
    }
    bool_reward(
        task_text.contains("xlsx")
            || task_text.contains("spreadsheet")
            || task_text.contains("calculation")
            || task_text.contains("table"),
    )
}

fn missing_facts_reward(score_report: Option<&ScoreReport>) -> f32 {
    let Some(score_report) = score_report else {
        return 0.5;
    };
    let mut missing_count = 0usize;
    for diagnostic in &score_report.failure_diagnostics {
        let lower = diagnostic.to_ascii_lowercase();
        if lower.contains("missing") || lower.contains("not find") || lower.contains("gap") {
            missing_count += 1;
        }
    }
    for comparison in &score_report.failure_comparisons {
        if matches!(
            comparison.failure_class,
            CriterionFailureClass::CoverageGap | CriterionFailureClass::ExtractionGap
        ) {
            missing_count += 1;
        }
    }
    1.0 - (missing_count.min(10) as f32 / 10.0)
}

fn pre_submit_self_check_reward(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> f32 {
    if run_record
        .tool_calls
        .iter()
        .any(|call| call.tool_name == "validate_deliverables" && call.error_kind.is_none())
    {
        return 1.0;
    }
    if coverage_snapshot(run_record, score_report)
        .is_some_and(|snapshot| !snapshot.self_checks.is_empty())
    {
        return 1.0;
    }
    bool_reward(
        model_text(run_record)
            .to_ascii_lowercase()
            .contains("self-check"),
    )
}

fn coverage_snapshot<'a>(
    run_record: &'a RunRecord,
    score_report: Option<&'a ScoreReport>,
) -> Option<&'a CoverageSnapshot> {
    score_report
        .and_then(|report| report.coverage_snapshot.as_ref())
        .or(run_record.coverage_snapshot.as_ref())
}

fn failure_labels(
    trace: &LegalVerifierRewardTrace,
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
    runner_output_mutation_allowed: bool,
    runner_added_answer_text_detected: bool,
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for reason in &trace.exclusion_reasons {
        labels.insert(reason.clone());
    }
    for reason in &trace.evidence.integrity_invalid_reasons {
        labels.insert(format!("integrity:{reason}"));
    }
    for criterion_id in &trace.evidence.failed_criterion_ids {
        labels.insert(format!("failed_criterion:{criterion_id}"));
    }
    if run_record.terminal_state != RunTerminalState::Submitted {
        labels.insert(format!("terminal_state:{:?}", run_record.terminal_state));
    }
    if runner_output_mutation_allowed {
        labels.insert(String::from("runner_output_mutation_allowed"));
    }
    if runner_added_answer_text_detected {
        labels.insert(String::from("runner_added_answer_text_detected"));
    }
    if let Some(score_report) = score_report {
        for comparison in &score_report.failure_comparisons {
            if comparison.failure_class != CriterionFailureClass::Passed {
                labels.insert(format!("failure_class:{:?}", comparison.failure_class));
            }
        }
        for diagnostic in &score_report.failure_diagnostics {
            let lower = diagnostic.to_ascii_lowercase();
            if lower.contains("missing") {
                labels.insert(String::from("diagnostic:missing_facts"));
            } else if lower.contains("not find") {
                labels.insert(String::from("diagnostic:marker_not_found"));
            }
        }
    }
    if labels.is_empty() {
        labels.insert(String::from("none"));
    }
    labels.into_iter().collect()
}

fn build_dpo_pairs(
    records: &[QwenLegalRlRolloutRecord],
) -> Result<Vec<QwenLegalRlDpoPair>, QwenLegalRlRolloutBatchError> {
    let mut by_task: BTreeMap<String, Vec<&QwenLegalRlRolloutRecord>> = BTreeMap::new();
    for record in records {
        by_task
            .entry(record.task_id.clone())
            .or_default()
            .push(record);
    }
    let mut pairs = Vec::new();
    for (task_id, task_records) in by_task {
        let chosen = task_records
            .iter()
            .copied()
            .filter(|record| record.training_eligible)
            .max_by(|left, right| {
                left.legal_content_score_bps
                    .cmp(&right.legal_content_score_bps)
                    .then_with(|| {
                        left.enhanced_reward
                            .total_reward
                            .total_cmp(&right.enhanced_reward.total_reward)
                    })
            });
        let Some(chosen) = chosen else {
            continue;
        };
        for rejected in task_records
            .iter()
            .copied()
            .filter(|record| record.rollout_id != chosen.rollout_id && record.training_eligible)
        {
            if rejected.legal_content_score_bps > chosen.legal_content_score_bps {
                continue;
            }
            let score_delta = (chosen.legal_content_score_bps as i64
                - rejected.legal_content_score_bps as i64) as f32
                / 10_000.0;
            let enhanced_delta =
                chosen.enhanced_reward.total_reward - rejected.enhanced_reward.total_reward;
            let reward_delta = score_delta.max(0.0) + enhanced_delta.max(0.0);
            let trainable = score_delta > 0.0 && rejected.negative_training_eligible;
            let mut pair = QwenLegalRlDpoPair {
                schema_version: String::from(QWEN_LEGAL_RL_DPO_PAIR_SCHEMA_VERSION),
                pair_id: format!(
                    "dpo.{}",
                    short_sha256(
                        format!("{}|{}|{}", task_id, chosen.rollout_id, rejected.rollout_id)
                            .as_bytes()
                    )
                ),
                task_id: task_id.clone(),
                chosen_rollout_id: chosen.rollout_id.clone(),
                rejected_rollout_id: rejected.rollout_id.clone(),
                chosen_reward: chosen.enhanced_reward.total_reward,
                rejected_reward: rejected.enhanced_reward.total_reward,
                reward_delta,
                trainable,
                reason: if trainable {
                    String::from(
                        "same task; chosen has equal or better legal reward and rejected is preserved as negative training data",
                    )
                } else {
                    String::from("same task pair retained for audit but not trainable")
                },
                pair_digest: String::new(),
            };
            pair.pair_digest = pair.stable_digest()?;
            pairs.push(pair);
        }
    }
    pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    Ok(pairs)
}

fn build_grpo_seeds(
    records: &[QwenLegalRlRolloutRecord],
) -> Result<Vec<QwenLegalRlGrpoSeed>, QwenLegalRlRolloutBatchError> {
    let mut by_task: BTreeMap<String, Vec<&QwenLegalRlRolloutRecord>> = BTreeMap::new();
    for record in records
        .iter()
        .filter(|record| record.training_eligible || record.negative_training_eligible)
    {
        by_task
            .entry(record.task_id.clone())
            .or_default()
            .push(record);
    }
    let mut seeds = Vec::new();
    for (task_id, mut task_records) in by_task {
        task_records.sort_by(|left, right| left.rollout_id.cmp(&right.rollout_id));
        if task_records.len() < 2 {
            continue;
        }
        let rollout_ids = task_records
            .iter()
            .map(|record| record.rollout_id.clone())
            .collect::<Vec<_>>();
        let rewards = task_records
            .iter()
            .map(|record| record.enhanced_reward.total_reward)
            .collect::<Vec<_>>();
        let trainable_rollout_classes = task_records
            .iter()
            .map(|record| record.rollout_class)
            .collect::<Vec<_>>();
        let mut seed = QwenLegalRlGrpoSeed {
            schema_version: String::from(QWEN_LEGAL_RL_GRPO_SEED_SCHEMA_VERSION),
            seed_id: format!(
                "grpo.{}",
                short_sha256(format!("{}|{}", task_id, rollout_ids.join("|")).as_bytes())
            ),
            task_id,
            rollout_ids,
            rewards,
            trainable_rollout_classes,
            deterministic_group_digest: String::new(),
        };
        seed.deterministic_group_digest = seed.stable_digest()?;
        seeds.push(seed);
    }
    seeds.sort_by(|left, right| left.seed_id.cmp(&right.seed_id));
    Ok(seeds)
}

fn build_task_improvement_rows(
    records: &[QwenLegalRlRolloutRecord],
) -> Vec<QwenLegalRlTaskImprovementRow> {
    let mut by_task: BTreeMap<String, Vec<&QwenLegalRlRolloutRecord>> = BTreeMap::new();
    for record in records {
        by_task
            .entry(record.task_id.clone())
            .or_default()
            .push(record);
    }

    let mut rows = Vec::new();
    for (task_id, task_records) in by_task {
        if task_records.is_empty() {
            continue;
        }
        let mut best_ranked = task_records.clone();
        sort_records_best_first(best_ranked.as_mut_slice());
        let best = best_ranked[0];

        let mut worst_ranked = task_records.clone();
        sort_records_worst_first(worst_ranked.as_mut_slice());
        let worst = worst_ranked[0];

        let failure_labels_observed = task_records
            .iter()
            .flat_map(|record| record.failure_labels.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let accepted_count = task_records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Accepted)
            .count();
        let rejected_count = task_records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Rejected)
            .count();
        let quarantined_count = task_records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Quarantined)
            .count();
        let adversarial_holdout_count = task_records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::AdversarialHoldout)
            .count();
        let negative_training_count = task_records
            .iter()
            .filter(|record| record.negative_training_eligible)
            .count();
        let preference_training_status =
            if task_records.iter().any(|record| record.training_eligible)
                && task_records
                    .iter()
                    .any(|record| record.negative_training_eligible)
            {
                String::from("chosen_rejected_pair_available")
            } else if negative_training_count > 0 {
                String::from("negative_only_replay")
            } else {
                String::from("positive_only_audit")
            };

        rows.push(QwenLegalRlTaskImprovementRow {
            schema_version: String::from(QWEN_LEGAL_RL_TASK_IMPROVEMENT_SCHEMA_VERSION),
            task_id,
            rollout_count: task_records.len(),
            accepted_count,
            rejected_count,
            quarantined_count,
            adversarial_holdout_count,
            negative_training_count,
            best_rollout_id: best.rollout_id.clone(),
            best_rollout_class: best.rollout_class,
            best_score_bps: best.legal_content_score_bps,
            best_reward: best.enhanced_reward.total_reward,
            worst_rollout_id: worst.rollout_id.clone(),
            worst_rollout_class: worst.rollout_class,
            worst_score_bps: worst.legal_content_score_bps,
            worst_reward: worst.enhanced_reward.total_reward,
            score_delta_bps: best.legal_content_score_bps as i64
                - worst.legal_content_score_bps as i64,
            reward_delta: best.enhanced_reward.total_reward - worst.enhanced_reward.total_reward,
            failure_labels_observed,
            preference_training_status,
        });
    }
    rows
}

fn build_failure_improvement_rows(
    records: &[QwenLegalRlRolloutRecord],
) -> Vec<QwenLegalRlFailureImprovementRow> {
    let mut by_label: BTreeMap<String, Vec<&QwenLegalRlRolloutRecord>> = BTreeMap::new();
    for record in records {
        for label in &record.failure_labels {
            if label == "none" {
                continue;
            }
            by_label.entry(label.clone()).or_default().push(record);
        }
    }

    let mut rows = Vec::new();
    for (failure_label, affected_records) in by_label {
        if affected_records.is_empty() {
            continue;
        }
        let mut worst_ranked = affected_records.clone();
        sort_records_worst_first(worst_ranked.as_mut_slice());
        let worst = worst_ranked[0];

        let affected_task_ids = affected_records
            .iter()
            .map(|record| record.task_id.clone())
            .collect::<BTreeSet<_>>();
        let mut counterexamples = records
            .iter()
            .filter(|record| {
                affected_task_ids.contains(record.task_id.as_str())
                    && !record
                        .failure_labels
                        .iter()
                        .any(|label| label == &failure_label)
            })
            .collect::<Vec<_>>();
        sort_records_best_first(counterexamples.as_mut_slice());
        let best_counterexample = counterexamples.first().copied();

        let negative_training_count = affected_records
            .iter()
            .filter(|record| record.negative_training_eligible)
            .count();
        let quarantined_count = affected_records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Quarantined)
            .count();
        let training_action = if negative_training_count > 0 && best_counterexample.is_some() {
            String::from("dpo_negative_pair_or_grpo_contrast")
        } else if quarantined_count > 0 {
            String::from("quarantine_replay_required")
        } else {
            String::from("audit_only_until_counterexample")
        };

        rows.push(QwenLegalRlFailureImprovementRow {
            schema_version: String::from(QWEN_LEGAL_RL_FAILURE_IMPROVEMENT_SCHEMA_VERSION),
            failure_label,
            affected_rollout_count: affected_records.len(),
            affected_task_count: affected_task_ids.len(),
            negative_training_count,
            quarantined_count,
            worst_rollout_id: worst.rollout_id.clone(),
            worst_task_id: worst.task_id.clone(),
            worst_score_bps: worst.legal_content_score_bps,
            worst_reward: worst.enhanced_reward.total_reward,
            best_counterexample_rollout_id: best_counterexample
                .map(|record| record.rollout_id.clone()),
            best_counterexample_task_id: best_counterexample.map(|record| record.task_id.clone()),
            best_counterexample_score_bps: best_counterexample
                .map(|record| record.legal_content_score_bps),
            best_counterexample_reward: best_counterexample
                .map(|record| record.enhanced_reward.total_reward),
            score_gap_bps: best_counterexample.map(|record| {
                record.legal_content_score_bps as i64 - worst.legal_content_score_bps as i64
            }),
            reward_gap: best_counterexample.map(|record| {
                record.enhanced_reward.total_reward - worst.enhanced_reward.total_reward
            }),
            training_action,
        });
    }
    rows
}

fn sort_records_best_first(records: &mut [&QwenLegalRlRolloutRecord]) {
    records.sort_by(|left, right| {
        right
            .legal_content_score_bps
            .cmp(&left.legal_content_score_bps)
            .then_with(|| {
                right
                    .enhanced_reward
                    .total_reward
                    .total_cmp(&left.enhanced_reward.total_reward)
            })
            .then_with(|| left.rollout_id.cmp(&right.rollout_id))
    });
}

fn sort_records_worst_first(records: &mut [&QwenLegalRlRolloutRecord]) {
    records.sort_by(|left, right| {
        left.legal_content_score_bps
            .cmp(&right.legal_content_score_bps)
            .then_with(|| {
                left.enhanced_reward
                    .total_reward
                    .total_cmp(&right.enhanced_reward.total_reward)
            })
            .then_with(|| left.rollout_id.cmp(&right.rollout_id))
    });
}

fn class_counts(records: &[QwenLegalRlRolloutRecord]) -> QwenLegalRlRolloutClassCounts {
    QwenLegalRlRolloutClassCounts {
        accepted: records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Accepted)
            .count(),
        rejected: records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Rejected)
            .count(),
        quarantined: records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::Quarantined)
            .count(),
        adversarial_holdout: records
            .iter()
            .filter(|record| record.rollout_class == QwenLegalRlRolloutClass::AdversarialHoldout)
            .count(),
    }
}

fn deterministic_rollout_id(
    config: &QwenLegalRlRolloutBatchConfig,
    run_record: &RunRecord,
    run_record_hash: &str,
) -> String {
    format!(
        "qwen.rl.{}",
        short_sha256(
            format!(
                "{}|{}|{}|{}|{}|{}",
                config.seed,
                config.batch_id,
                config.model_artifact_digest,
                run_record.task_id,
                run_record.run_id,
                run_record_hash
            )
            .as_bytes()
        )
    )
}

fn raw_response_hashes(run_record: &RunRecord) -> Vec<String> {
    let mut hashes = BTreeSet::new();
    for event in &run_record.transcript {
        if event.event_kind != TranscriptEventKind::Assistant {
            continue;
        }
        if let Some(payload) = &event.payload {
            collect_string_field(payload, "raw_response_hash", &mut hashes);
        }
    }
    hashes.into_iter().collect()
}

fn observed_model_id(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> Option<String> {
    metadata_string(&run_record.metadata, "route_model_id")
        .or_else(|| metadata_string(&run_record.metadata, "model_id"))
        .or_else(|| transcript_payload_string(run_record, "model_id"))
        .or_else(|| transcript_payload_string(run_record, "route_model_id"))
        .or_else(|| score_report.and_then(|report| metadata_string(&report.metadata, "model_id")))
}

fn observed_route_id(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> Option<String> {
    metadata_string(&run_record.metadata, "route_id")
        .or_else(|| transcript_payload_string(run_record, "route_id"))
        .or_else(|| score_report.and_then(|report| metadata_string(&report.metadata, "route_id")))
}

fn transcript_payload_string(run_record: &RunRecord, key: &str) -> Option<String> {
    for event in &run_record.transcript {
        if let Some(payload) = &event.payload {
            if let Some(value) = find_string_field(payload, key) {
                return Some(value);
            }
        }
    }
    None
}

fn find_string_field(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(value) = map.get(key).and_then(Value::as_str) {
                return Some(value.to_string());
            }
            map.values().find_map(|value| find_string_field(value, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_string_field(value, key)),
        _ => None,
    }
}

fn collect_string_field(value: &Value, key: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(value) = map.get(key).and_then(Value::as_str) {
                out.insert(value.to_string());
            }
            for nested in map.values() {
                collect_string_field(nested, key, out);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_string_field(nested, key, out);
            }
        }
        _ => {}
    }
}

fn blueprint_scaffold_ref(
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
) -> QwenLegalRlBlueprintScaffoldRef {
    let system_text = run_record
        .transcript
        .iter()
        .filter(|event| event.event_kind == TranscriptEventKind::System)
        .filter_map(|event| event.content.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    let declared_input_present = system_text.contains("Blueprint work-product scaffold")
        || metadata_string(&run_record.metadata, "mode").as_deref() == Some("blueprint_scaffold")
        || score_report
            .and_then(|report| metadata_string(&report.metadata, "mode"))
            .as_deref()
            == Some("blueprint_scaffold");
    let declared_input_digest = declared_input_present
        .then(|| sha256_hex(system_text.as_bytes()))
        .filter(|digest| digest != &sha256_hex(b""));
    QwenLegalRlBlueprintScaffoldRef {
        declared_input_present,
        declared_input_digest,
        source: if declared_input_present {
            String::from("declared_system_prompt_or_score_metadata")
        } else {
            String::from("none")
        },
        runner_may_add_answer_text: false,
    }
}

fn runner_output_mutation_allowed(
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
) -> bool {
    metadata_flag(&run_record.metadata, "runner_content_mutation_allowed")
        || transcript_payload_bool(run_record, "runner_content_mutation_allowed")
        || score_report.is_some_and(|report| {
            metadata_flag(&report.metadata, "runner_content_mutation_allowed")
        })
}

fn runner_added_answer_text_detected(run_record: &RunRecord) -> bool {
    if metadata_value_contains(
        &run_record.metadata,
        &["runner-added answer text", "runner_added_answer_text"],
    ) {
        return true;
    }
    run_record.transcript.iter().any(|event| {
        event.event_kind == TranscriptEventKind::Runner
            && event.content.as_ref().is_some_and(|content| {
                let lower = content.to_ascii_lowercase();
                lower.contains("runner-added answer text")
                    || lower.contains("runner added answer text")
                    || lower.contains("harness-injected")
            })
    })
}

fn metadata_flag(metadata: &Metadata, key: &str) -> bool {
    metadata.get(key).and_then(Value::as_bool) == Some(true)
}

fn metadata_string(metadata: &Metadata, key: &str) -> Option<String> {
    metadata.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn metadata_value_contains(metadata: &Metadata, needles: &[&str]) -> bool {
    metadata
        .values()
        .any(|value| value_contains(value, needles))
}

fn value_contains(value: &Value, needles: &[&str]) -> bool {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            needles.iter().any(|needle| lower.contains(needle))
        }
        Value::Array(values) => values.iter().any(|value| value_contains(value, needles)),
        Value::Object(map) => map.values().any(|value| value_contains(value, needles)),
        _ => false,
    }
}

fn transcript_payload_bool(run_record: &RunRecord, key: &str) -> bool {
    run_record
        .transcript
        .iter()
        .filter_map(|event| event.payload.as_ref())
        .any(|payload| find_bool_field(payload, key))
}

fn find_bool_field(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.get(key).and_then(Value::as_bool) == Some(true)
                || map.values().any(|value| find_bool_field(value, key))
        }
        Value::Array(values) => values.iter().any(|value| find_bool_field(value, key)),
        _ => false,
    }
}

fn model_text(run_record: &RunRecord) -> String {
    run_record
        .transcript
        .iter()
        .filter(|event| {
            matches!(
                event.event_kind,
                TranscriptEventKind::Assistant | TranscriptEventKind::ToolCall
            )
        })
        .filter_map(|event| {
            event
                .content
                .clone()
                .or_else(|| event.payload.as_ref().map(Value::to_string))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn preserved_root(config: &QwenLegalRlRolloutBatchConfig) -> PathBuf {
    config.output_dir.join("preserved_rollouts")
}

fn read_json<T>(path: &Path) -> Result<T, QwenLegalRlRolloutBatchError>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|source| QwenLegalRlRolloutBatchError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional_json<T>(path: Option<&PathBuf>) -> Result<Option<T>, QwenLegalRlRolloutBatchError>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    read_json(path.as_path()).map(Some)
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), QwenLegalRlRolloutBatchError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalRlRolloutBatchError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_jsonl<T>(path: &Path, values: &[T]) -> Result<(), QwenLegalRlRolloutBatchError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalRlRolloutBatchError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut out = String::new();
    for value in values {
        out.push_str(serde_json::to_string(value)?.as_str());
        out.push('\n');
    }
    fs::write(path, out).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn stable_json_digest<T>(prefix: &[u8], value: &T) -> Result<String, QwenLegalRlRolloutBatchError>
where
    T: Serialize,
{
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value)?);
    Ok(hex::encode(hasher.finalize()))
}

fn sha256_file(path: &Path) -> Result<String, QwenLegalRlRolloutBatchError> {
    let bytes = fs::read(path).map_err(|source| QwenLegalRlRolloutBatchError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    sha256_hex(bytes)[..16].to_string()
}

fn bool_reward(value: bool) -> f32 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn mean<const N: usize>(values: [f32; N]) -> f32 {
    values.iter().sum::<f32>() / N.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn qwen_legal_rl_rollout_batch_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let first_dir = tempfile::tempdir()?;
        let second_dir = tempfile::tempdir()?;
        let first_config = QwenLegalRlRolloutBatchConfig {
            runs_root: fixture_runs_root(),
            output_dir: first_dir.path().join("batch"),
            ..QwenLegalRlRolloutBatchConfig::default()
        };
        let second_config = QwenLegalRlRolloutBatchConfig {
            runs_root: fixture_runs_root(),
            output_dir: second_dir.path().join("batch"),
            ..QwenLegalRlRolloutBatchConfig::default()
        };

        let first = run_qwen_legal_rl_rollout_batch(&first_config)?;
        let second = run_qwen_legal_rl_rollout_batch(&second_config)?;

        assert_eq!(
            first.deterministic_rollout_ids,
            second.deterministic_rollout_ids
        );
        assert_eq!(first.rollout_count, 6);
        assert!(first.class_counts.accepted >= 3);
        assert!(first.class_counts.rejected >= 1);
        assert_eq!(first.runner_added_answer_text_count, 0);
        assert_eq!(first.hidden_or_private_performance_claim, false);
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_rollout_batch_preserves_negative_examples(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let config = QwenLegalRlRolloutBatchConfig {
            runs_root: fixture_runs_root(),
            output_dir: temp.path().join("batch"),
            ..QwenLegalRlRolloutBatchConfig::default()
        };

        let report = run_qwen_legal_rl_rollout_batch(&config)?;
        let pairs = fs::read_to_string(&report.dpo_pair_path)?;
        let records = fs::read_to_string(&report.rollout_record_path)?;
        let task_improvements = read_json::<Vec<QwenLegalRlTaskImprovementRow>>(Path::new(
            &report.task_improvement_path,
        ))?;
        let failure_improvements = read_json::<Vec<QwenLegalRlFailureImprovementRow>>(Path::new(
            &report.failure_improvement_path,
        ))?;

        assert!(report.bad_completion_count >= 1);
        assert!(report.dpo_pair_count >= 1);
        assert_eq!(report.task_improvement_count, task_improvements.len());
        assert_eq!(report.failure_improvement_count, failure_improvements.len());
        assert!(report.task_improvement_count >= 1);
        assert!(report.failure_improvement_count >= 1);
        assert!(pairs.contains("\"trainable\":true"));
        assert!(records.contains("\"negative_training_eligible\":true"));
        assert!(task_improvements.iter().any(|row| row.score_delta_bps > 0));
        assert!(failure_improvements.iter().any(
            |row| row.negative_training_count > 0 && row.score_gap_bps.unwrap_or_default() > 0
        ));
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_rollout_batch_quarantines_runner_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source = fixture_runs_root()
            .join("model_only")
            .join("harvey_funds_asset_management_analyze_mfn_waterfall");
        let mutated = temp.path().join("mutated");
        copy_dir_for_test(source.as_path(), mutated.as_path())?;
        let run_record_path = mutated.join("run").join("run_record.json");
        let mut run_record = read_json::<RunRecord>(run_record_path.as_path())?;
        run_record
            .metadata
            .insert(String::from("runner_content_mutation_allowed"), json!(true));
        write_json(run_record_path.as_path(), &run_record)?;
        let score_report_path = mutated.join("score_report.json");
        let mut score_report = read_json::<ScoreReport>(score_report_path.as_path())?;
        score_report
            .metadata
            .insert(String::from("runner_content_mutation_allowed"), json!(true));
        write_json(score_report_path.as_path(), &score_report)?;

        let config = QwenLegalRlRolloutBatchConfig {
            runs_root: temp.path().to_path_buf(),
            output_dir: temp.path().join("batch"),
            ..QwenLegalRlRolloutBatchConfig::default()
        };
        let report = run_qwen_legal_rl_rollout_batch(&config)?;
        let records = fs::read_to_string(&report.rollout_record_path)?;

        assert_eq!(report.class_counts.quarantined, 1);
        assert!(records.contains("\"rollout_class\":\"quarantined\""));
        assert!(records.contains("runner_output_mutation_allowed"));
        Ok(())
    }

    fn copy_dir_for_test(source: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_for_test(source_path.as_path(), dest_path.as_path())?;
            } else {
                fs::copy(source_path, dest_path)?;
            }
        }
        Ok(())
    }

    fn fixture_runs_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(DEFAULT_RUNS_ROOT)
    }
}
