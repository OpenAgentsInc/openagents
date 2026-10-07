//! Deterministic local replay harness for legal benchmark eval suites.
//!
//! This module is deliberately small and receipt-heavy. It does not claim to
//! run a real model by itself. It replays declared base and adapter outputs
//! through the Rust legal benchmark scorer so that later training and
//! promotion code can compare the same suite in the same order with the same
//! prompt, scorer, source hashes, and inference settings.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    artifact_from_file, artifact_manifest_digest, build_input_artifact_manifest,
    build_output_artifact_manifest, evaluate_legal_benchmark_run,
    generate_legal_benchmark_static_report, stable_json_digest, ArtifactKind, BenchmarkTaskSpec,
    CriterionKind, CriterionSpec, DataClassification, DeliverableKind, DeliverableSpec, JudgeMode,
    JudgePolicy, LegalBenchmarkEvaluationInput, LegalBenchmarkEvaluationResult,
    LegalBenchmarkReportInput, Metadata, MockLegalBenchmarkJudge, RunMetrics, RunRecord,
    RunTerminalState, ScoreReport, SourceArtifact, ToolCallRecord, ToolPolicy, TranscriptEvent,
    TranscriptEventKind,
};

pub const LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkEvalMode {
    Smoke,
    PublicHarveyThreeTask,
    PublicHarveyExpanded,
    InternalSynthetic,
    HiddenAuditOnly,
}

impl LegalBenchmarkEvalMode {
    fn allows_training(self) -> bool {
        !matches!(self, Self::HiddenAuditOnly)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalInferenceSettings {
    pub temperature: f32,
    pub top_p: f32,
    pub max_output_tokens: u32,
    pub seed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalSourceDocument {
    pub document_id: String,
    pub relative_path: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkEvalReplayOutcome {
    Pass,
    MissingAnswer,
    ToolFailure,
    Timeout,
}

impl LegalBenchmarkEvalReplayOutcome {
    fn failure_class(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::MissingAnswer => "missing_answer",
            Self::ToolFailure => "tool_failure",
            Self::Timeout => "timeout",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalTaskFixture {
    pub task_id: String,
    pub task_version: String,
    pub title: String,
    pub practice_area: String,
    pub workflow: String,
    pub instructions: String,
    pub source_document_ids: Vec<String>,
    pub required_answer_path: String,
    pub base_outcome: LegalBenchmarkEvalReplayOutcome,
    pub adapter_outcome: LegalBenchmarkEvalReplayOutcome,
    pub replay_answer_markdown: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalSuiteManifest {
    pub schema_version: u16,
    pub suite_id: String,
    pub mode: LegalBenchmarkEvalMode,
    pub fixed_task_order: Vec<String>,
    pub prompt_template_id: String,
    pub prompt_template_hash: String,
    pub scorer_version: String,
    pub inference_settings: LegalBenchmarkEvalInferenceSettings,
    pub source_documents: Vec<LegalBenchmarkEvalSourceDocument>,
    pub tasks: Vec<LegalBenchmarkEvalTaskFixture>,
    pub training_allowed: bool,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalModelBinding {
    pub role: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_sha256: Option<String>,
    pub artifact_materialized: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalTaskReport {
    pub task_id: String,
    pub model_role: String,
    pub run_id: String,
    pub task_type: String,
    pub outcome: LegalBenchmarkEvalReplayOutcome,
    pub submitted: bool,
    pub correct_path: bool,
    pub answer_file_success: bool,
    pub legal_score_bps: u32,
    pub integrity_valid: bool,
    pub tool_failure: bool,
    pub timeout_failure: bool,
    pub failure_classes: Vec<String>,
    pub score_report_hash: String,
    pub run_record_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalTaskTypeScore {
    pub task_type: String,
    pub task_count: u64,
    pub mean_legal_score_bps: u32,
    pub median_legal_score_bps: u32,
    pub answer_file_write_rate_bps: u32,
    pub correct_path_rate_bps: u32,
    pub submit_rate_bps: u32,
    pub integrity_valid_rate_bps: u32,
    pub failure_class_counts: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalSuiteModelReport {
    pub model_binding: LegalBenchmarkEvalModelBinding,
    pub answer_file_write_rate_bps: u32,
    pub answer_file_success_rate_bps: u32,
    pub correct_path_rate_bps: u32,
    pub submit_rate_bps: u32,
    pub integrity_valid_rate_bps: u32,
    pub mean_legal_score_bps: u32,
    pub median_legal_score_bps: u32,
    pub legal_score_bps: u32,
    pub integrity_failure_count: u64,
    pub tool_failure_count: u64,
    pub timeout_failure_count: u64,
    pub failure_class_counts: BTreeMap<String, u64>,
    pub score_by_task_type: BTreeMap<String, LegalBenchmarkEvalTaskTypeScore>,
    pub score_confidence_notes: Vec<String>,
    pub task_reports: Vec<LegalBenchmarkEvalTaskReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalComparison {
    pub champion_legal_score_bps: u32,
    pub candidate_legal_score_bps: u32,
    pub score_delta_bps: i32,
    pub regression_vs_champion_bps: i32,
    pub answer_file_success_delta_bps: i32,
    pub integrity_failure_delta: i64,
    pub tool_failure_delta: i64,
    pub timeout_failure_delta: i64,
    pub regression_detected: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalPromotionGateInput {
    pub schema_version: u16,
    pub suite_id: String,
    pub suite_hash: String,
    pub baseline_model_id: String,
    pub candidate_model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_adapter_id: Option<String>,
    pub baseline_legal_score_bps: u32,
    pub candidate_legal_score_bps: u32,
    pub score_delta_bps: i32,
    pub candidate_answer_file_success_rate_bps: u32,
    pub candidate_integrity_failure_count: u64,
    pub candidate_tool_failure_count: u64,
    pub candidate_timeout_failure_count: u64,
    pub regression_detected: bool,
    pub decision: LegalBenchmarkPromotionDecision,
    pub replay_command: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkPromotionDecision {
    Promote,
    Hold,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalReplayReceipt {
    pub schema_version: u16,
    pub suite_path: String,
    pub suite_hash: String,
    pub output_dir: String,
    pub report_hash: String,
    pub replay_command: Vec<String>,
    pub generated_artifacts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalSuiteReport {
    pub schema_version: u16,
    pub suite_id: String,
    pub suite_hash: String,
    pub mode: LegalBenchmarkEvalMode,
    pub fixed_task_order: Vec<String>,
    pub prompt_template_hash: String,
    pub scorer_version: String,
    pub source_document_hashes: BTreeMap<String, String>,
    pub base_model_result: LegalBenchmarkEvalSuiteModelReport,
    pub adapter_result: LegalBenchmarkEvalSuiteModelReport,
    pub comparison: LegalBenchmarkEvalComparison,
    pub promotion_gate_input: LegalBenchmarkEvalPromotionGateInput,
    pub replay_receipt: LegalBenchmarkEvalReplayReceipt,
    pub report_boundary: String,
}

#[derive(Clone, Debug)]
pub struct LegalBenchmarkEvalSuiteRunConfig {
    pub suite_path: PathBuf,
    pub base_model: String,
    pub adapter: String,
    pub output_dir: PathBuf,
    pub replay_command: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvalSuiteCatalogEntry {
    pub suite_level: String,
    pub suite_id: String,
    pub path: String,
    pub task_count: usize,
    pub materialized: bool,
    pub promotion_target: bool,
    pub notes: String,
}

pub fn legal_benchmark_public_suite_catalog() -> Vec<LegalBenchmarkEvalSuiteCatalogEntry> {
    vec![
        suite_catalog_entry(
            "harvey_public_001_single",
            "suites/harvey_public_001_single.json",
            1,
            true,
            true,
            "single-task smoke gate",
        ),
        suite_catalog_entry(
            "harvey_public_003_workflow",
            "suites/harvey_public_003_workflow.json",
            3,
            true,
            true,
            "frozen three-task workflow gate",
        ),
        suite_catalog_entry(
            "harvey_public_010_mixed",
            "suites/harvey_public_010_mixed.json",
            10,
            true,
            true,
            "mixed public-style regression suite",
        ),
        suite_catalog_entry(
            "harvey_public_025_regression",
            "suites/harvey_public_025_regression.json",
            25,
            false,
            false,
            "reserved until enough public tasks are materialized",
        ),
        suite_catalog_entry(
            "harvey_public_050_candidate_gate",
            "suites/harvey_public_050_candidate_gate.json",
            50,
            false,
            false,
            "reserved until enough public tasks are materialized",
        ),
    ]
}

pub fn resolve_legal_benchmark_eval_suite_ref(suite_ref: &str) -> PathBuf {
    let path = PathBuf::from(suite_ref);
    if path.is_absolute() || path.exists() || path.extension().is_some() {
        return path;
    }
    legal_benchmark_public_suite_catalog()
        .into_iter()
        .find(|entry| entry.suite_level == suite_ref || entry.suite_id == suite_ref)
        .map(|entry| PathBuf::from(entry.path))
        .unwrap_or_else(|| PathBuf::from(suite_ref))
}

pub fn default_legal_benchmark_eval_output_dir(suite_ref: &str) -> PathBuf {
    PathBuf::from("target/legal").join(format!("{}_eval_smoke", sanitize_path_component(suite_ref)))
}

fn suite_catalog_entry(
    suite_level: &str,
    path: &str,
    task_count: usize,
    materialized: bool,
    promotion_target: bool,
    notes: &str,
) -> LegalBenchmarkEvalSuiteCatalogEntry {
    LegalBenchmarkEvalSuiteCatalogEntry {
        suite_level: String::from(suite_level),
        suite_id: String::from(suite_level),
        path: String::from(path),
        task_count,
        materialized,
        promotion_target,
        notes: String::from(notes),
    }
}

#[derive(Debug, Error)]
pub enum LegalBenchmarkEvalSuiteError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("artifact error: {0}")]
    Artifact(#[from] crate::ArtifactManifestError),
    #[error("evaluation error: {0}")]
    Evaluation(#[from] crate::LegalBenchmarkEvaluationError),
    #[error("invalid eval suite manifest: {0}")]
    InvalidManifest(String),
}

pub fn run_legal_benchmark_eval_suite(
    config: &LegalBenchmarkEvalSuiteRunConfig,
) -> Result<LegalBenchmarkEvalSuiteReport, LegalBenchmarkEvalSuiteError> {
    let suite_bytes = read_file(config.suite_path.as_path())?;
    let manifest = serde_json::from_slice::<LegalBenchmarkEvalSuiteManifest>(&suite_bytes)?;
    validate_suite_manifest(&manifest, config.suite_path.as_path())?;
    let suite_hash =
        stable_json_digest("psionic.legal_benchmark.eval_suite_manifest.v1", &manifest)?;
    fs::create_dir_all(&config.output_dir).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: config.output_dir.clone(),
        source,
    })?;

    let base_binding = LegalBenchmarkEvalModelBinding {
        role: String::from("base"),
        model_id: config.base_model.clone(),
        adapter_id: None,
        artifact_path: None,
        artifact_sha256: None,
        artifact_materialized: false,
    };
    let adapter_binding = adapter_binding(config.adapter.as_str())?;
    let base_result = run_model_eval(
        &manifest,
        config.suite_path.as_path(),
        &suite_hash,
        &base_binding,
        &config.output_dir.join("base"),
        false,
    )?;
    let adapter_result = run_model_eval(
        &manifest,
        config.suite_path.as_path(),
        &suite_hash,
        &adapter_binding,
        &config.output_dir.join("adapter"),
        true,
    )?;
    let comparison = compare_model_reports(&base_result, &adapter_result);
    let promotion_gate_input = promotion_gate_input(
        &manifest,
        suite_hash.as_str(),
        &base_result,
        &adapter_result,
        &comparison,
        &config.replay_command,
    );
    let source_document_hashes = manifest
        .source_documents
        .iter()
        .map(|document| (document.document_id.clone(), document.sha256.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut report = LegalBenchmarkEvalSuiteReport {
        schema_version: LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION,
        suite_id: manifest.suite_id.clone(),
        suite_hash: suite_hash.clone(),
        mode: manifest.mode,
        fixed_task_order: manifest.fixed_task_order.clone(),
        prompt_template_hash: manifest.prompt_template_hash.clone(),
        scorer_version: manifest.scorer_version.clone(),
        source_document_hashes,
        base_model_result: base_result,
        adapter_result,
        comparison,
        promotion_gate_input,
        replay_receipt: LegalBenchmarkEvalReplayReceipt {
            schema_version: LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION,
            suite_path: config.suite_path.to_string_lossy().to_string(),
            suite_hash: suite_hash.clone(),
            output_dir: config.output_dir.to_string_lossy().to_string(),
            report_hash: String::new(),
            replay_command: config.replay_command.clone(),
            generated_artifacts: Vec::new(),
        },
        report_boundary: String::from(
            "This is a deterministic local replay report. It compares declared base and adapter outputs through the Rust scorer; it is not proof of performance on private benchmark tasks.",
        ),
    };
    let report_hash = stable_json_digest("psionic.legal_benchmark.eval_suite_report.v1", &report)?;
    let generated_artifacts = write_eval_suite_report(config.output_dir.as_path(), &report)?;
    report.replay_receipt.report_hash = report_hash;
    report.replay_receipt.generated_artifacts = generated_artifacts;
    write_file_pretty(
        config.output_dir.join("replay_receipt.json").as_path(),
        &report.replay_receipt,
    )?;
    write_file_pretty(
        config.output_dir.join("eval_report.json").as_path(),
        &report,
    )?;
    Ok(report)
}

fn validate_suite_manifest(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
) -> Result<(), LegalBenchmarkEvalSuiteError> {
    if manifest.schema_version != LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION {
        return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
            "schema_version must be {LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION}"
        )));
    }
    if manifest.suite_id.is_empty() {
        return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(String::from(
            "suite_id must not be empty",
        )));
    }
    if manifest.fixed_task_order.len() != manifest.tasks.len() {
        return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(String::from(
            "fixed_task_order must include every task exactly once",
        )));
    }
    let mut task_ids = std::collections::BTreeSet::new();
    for task in &manifest.tasks {
        if !task_ids.insert(task.task_id.as_str()) {
            return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                "duplicate task id {}",
                task.task_id
            )));
        }
    }
    let mut ordered_task_ids = std::collections::BTreeSet::new();
    for task_id in &manifest.fixed_task_order {
        if !ordered_task_ids.insert(task_id.as_str()) {
            return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                "fixed_task_order repeats task {task_id}"
            )));
        }
        if !task_ids.contains(task_id.as_str()) {
            return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                "fixed_task_order contains unknown task {task_id}"
            )));
        }
    }
    if !manifest.mode.allows_training() && manifest.training_allowed {
        return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(String::from(
            "hidden audit suites cannot be marked training_allowed",
        )));
    }
    for document in &manifest.source_documents {
        let path = resolve_suite_relative_path(suite_path, document.relative_path.as_str());
        let bytes = read_file(path.as_path())?;
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != document.sha256 {
            return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                "source document {} hash mismatch",
                document.document_id
            )));
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != document.byte_size {
            return Err(LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                "source document {} byte_size mismatch",
                document.document_id
            )));
        }
    }
    Ok(())
}

fn run_model_eval(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
    suite_hash: &str,
    binding: &LegalBenchmarkEvalModelBinding,
    output_dir: &Path,
    use_adapter_outcomes: bool,
) -> Result<LegalBenchmarkEvalSuiteModelReport, LegalBenchmarkEvalSuiteError> {
    fs::create_dir_all(output_dir).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let mut task_reports = Vec::new();
    let mut score_reports = Vec::new();
    let mut run_records = Vec::new();
    for task_id in &manifest.fixed_task_order {
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == *task_id)
            .ok_or_else(|| {
                LegalBenchmarkEvalSuiteError::InvalidManifest(format!("missing task {task_id}"))
            })?;
        let task_spec = build_task_spec(manifest, suite_path, task)?;
        let outcome = if use_adapter_outcomes {
            task.adapter_outcome
        } else {
            task.base_outcome
        };
        let task_output_dir = output_dir.join(sanitize_path_component(task.task_id.as_str()));
        let evaluation = replay_task(
            manifest,
            suite_hash,
            binding,
            &task_spec,
            task,
            outcome,
            &task_output_dir,
        )?;
        let score_report_hash = evaluation.score_report_hash.clone();
        let run_record_hash = evaluation.score_report.run_record_hash.clone();
        let task_report = task_report_from_score(
            binding,
            task,
            outcome,
            &evaluation.score_report,
            score_report_hash,
            run_record_hash,
        );
        write_file_pretty(
            task_output_dir.join("score_report.json").as_path(),
            &evaluation.score_report,
        )?;
        task_reports.push(task_report);
        score_reports.push(evaluation.score_report);
        run_records.push(read_json_file::<RunRecord>(
            task_output_dir.join("run_record.json").as_path(),
        )?);
    }
    let static_report = generate_legal_benchmark_static_report(&LegalBenchmarkReportInput {
        report_id: format!("{}.{}", manifest.suite_id, binding.role),
        score_reports,
        run_records,
        output_manifests: Vec::new(),
    })?;
    fs::write(output_dir.join("report.md"), static_report.markdown).map_err(|source| {
        LegalBenchmarkEvalSuiteError::Io {
            path: output_dir.join("report.md"),
            source,
        }
    })?;
    fs::write(
        output_dir.join("autopilot_report.json"),
        serde_json::to_vec_pretty(&static_report.autopilot_export)?,
    )
    .map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: output_dir.join("autopilot_report.json"),
        source,
    })?;
    Ok(model_report(binding.clone(), task_reports))
}

fn replay_task(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_hash: &str,
    binding: &LegalBenchmarkEvalModelBinding,
    task_spec: &BenchmarkTaskSpec,
    task: &LegalBenchmarkEvalTaskFixture,
    outcome: LegalBenchmarkEvalReplayOutcome,
    output_dir: &Path,
) -> Result<LegalBenchmarkEvaluationResult, LegalBenchmarkEvalSuiteError> {
    fs::create_dir_all(output_dir).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let answer_content = answer_content(manifest, binding, task);
    let mut tool_calls = Vec::new();
    let mut artifacts = Vec::new();
    if outcome == LegalBenchmarkEvalReplayOutcome::Pass {
        let answer_path = output_dir.join(&task.required_answer_path);
        if let Some(parent) = answer_path.parent() {
            fs::create_dir_all(parent).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&answer_path, answer_content.as_bytes()).map_err(|source| {
            LegalBenchmarkEvalSuiteError::Io {
                path: answer_path.clone(),
                source,
            }
        })?;
        artifacts.push(artifact_from_file(
            format!("artifact.output.{}.answer", task.task_id),
            ArtifactKind::GeneratedDeliverable,
            output_dir,
            &answer_path,
            DataClassification::BenchmarkConfidential,
            Some(format!("deterministic_replay:{}", binding.role)),
        )?);
        tool_calls.push(write_tool_call(
            format!("call.write.{}.{}", binding.role, task.task_id),
            task.required_answer_path.as_str(),
            answer_content.as_str(),
            None,
        ));
    } else if outcome == LegalBenchmarkEvalReplayOutcome::ToolFailure {
        tool_calls.push(write_tool_call(
            format!("call.write.{}.{}", binding.role, task.task_id),
            task.required_answer_path.as_str(),
            answer_content.as_str(),
            Some("write_tool_failed"),
        ));
    }
    let run_id_value = run_id(binding, task.task_id.as_str());
    let output_manifest = build_output_artifact_manifest(
        task.task_id.clone(),
        task.task_version.clone(),
        run_id_value.as_str(),
        artifacts,
    );
    let mut run_record = run_record(
        manifest,
        suite_hash,
        binding,
        run_id_value.as_str(),
        task_spec,
        &output_manifest,
        outcome,
        tool_calls,
    )?;
    run_record.output_artifact_manifest_hash = artifact_manifest_digest(&output_manifest)?;
    write_file_pretty(output_dir.join("run_record.json").as_path(), &run_record)?;
    write_file_pretty(
        output_dir.join("output_manifest.json").as_path(),
        &output_manifest,
    )?;
    let mut judge = MockLegalBenchmarkJudge::pass();
    evaluate_legal_benchmark_run(
        &LegalBenchmarkEvaluationInput {
            task_spec: task_spec.clone(),
            run_record,
            output_artifact_manifest: output_manifest,
            output_root: output_dir.to_path_buf(),
        },
        &mut judge,
    )
    .map_err(LegalBenchmarkEvalSuiteError::from)
}

fn build_task_spec(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_path: &Path,
    task: &LegalBenchmarkEvalTaskFixture,
) -> Result<BenchmarkTaskSpec, LegalBenchmarkEvalSuiteError> {
    let mut source_artifacts = Vec::new();
    for source_document_id in &task.source_document_ids {
        let document = manifest
            .source_documents
            .iter()
            .find(|document| document.document_id == *source_document_id)
            .ok_or_else(|| {
                LegalBenchmarkEvalSuiteError::InvalidManifest(format!(
                    "task {} references unknown source document {}",
                    task.task_id, source_document_id
                ))
            })?;
        source_artifacts.push(source_artifact_from_document(suite_path, document)?);
    }
    Ok(BenchmarkTaskSpec {
        schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
        task_id: task.task_id.clone(),
        task_version: task.task_version.clone(),
        domain: String::from("legal"),
        practice_area: task.practice_area.clone(),
        workflow: task.workflow.clone(),
        title: task.title.clone(),
        instructions: task.instructions.clone(),
        work_type: task.workflow.clone(),
        tags: vec![
            manifest.suite_id.clone(),
            String::from("harvey_public"),
            String::from("deterministic_replay"),
        ],
        source_artifacts,
        deliverables: vec![DeliverableSpec {
            deliverable_id: String::from("answer"),
            deliverable_kind: DeliverableKind::Markdown,
            required_path: task.required_answer_path.clone(),
            description: String::from("Required legal work product answer."),
            required: true,
        }],
        criteria: vec![
            CriterionSpec {
                criterion_id: format!("{}.answer_file", task.task_id),
                criterion_kind: CriterionKind::DeliverableValidation,
                description: String::from("The required answer file exists and is readable."),
                weight_bps: Some(5000),
                deliverable_ids: vec![String::from("answer")],
                source_artifact_ids: Vec::new(),
            },
            CriterionSpec {
                criterion_id: format!("{}.legal_work_product", task.task_id),
                criterion_kind: CriterionKind::LegalReasoning,
                description: String::from(
                    "The answer gives a concise legal work product grounded in the source documents.",
                ),
                weight_bps: Some(5000),
                deliverable_ids: vec![String::from("answer")],
                source_artifact_ids: task.source_document_ids.clone(),
            },
        ],
        judge_policy: JudgePolicy {
            mode: JudgeMode::Deterministic,
            provider: String::from("rust_local_replay"),
            model: manifest.scorer_version.clone(),
            prompt_template_id: manifest.prompt_template_id.clone(),
            prompt_template_hash: manifest.prompt_template_hash.clone(),
            all_pass_required: true,
            sample_count: 1,
        },
        tool_policy: ToolPolicy {
            allowed_tools: vec![String::from("write"), String::from("read")],
            network_allowed: false,
            source_artifacts_read_only: true,
            max_turns: 8,
            max_wall_time_seconds: 120,
        },
        source_compatibility: None,
        metadata: Metadata::new(),
    })
}

fn run_record(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_hash: &str,
    binding: &LegalBenchmarkEvalModelBinding,
    run_id_value: &str,
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &crate::ArtifactManifest,
    outcome: LegalBenchmarkEvalReplayOutcome,
    tool_calls: Vec<ToolCallRecord>,
) -> Result<RunRecord, LegalBenchmarkEvalSuiteError> {
    let input_manifest = build_input_artifact_manifest(task_spec);
    let input_artifact_manifest_hash = artifact_manifest_digest(&input_manifest)?;
    let run_config = json!({
        "suite_id": manifest.suite_id,
        "suite_hash": suite_hash,
        "model_binding": binding,
        "inference_settings": manifest.inference_settings,
        "prompt_template_hash": manifest.prompt_template_hash,
        "scorer_version": manifest.scorer_version,
    });
    let run_config_hash = stable_json_digest(
        "psionic.legal_benchmark.eval_suite.run_config.v1",
        &run_config,
    )?;
    let terminal_state = match outcome {
        LegalBenchmarkEvalReplayOutcome::Pass => RunTerminalState::Submitted,
        LegalBenchmarkEvalReplayOutcome::MissingAnswer => RunTerminalState::NoToolCalls,
        LegalBenchmarkEvalReplayOutcome::ToolFailure => RunTerminalState::SandboxFailure,
        LegalBenchmarkEvalReplayOutcome::Timeout => RunTerminalState::MaxTurns,
    };
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("eval_suite_id"),
        Value::String(manifest.suite_id.clone()),
    );
    metadata.insert(
        String::from("eval_suite_hash"),
        Value::String(suite_hash.to_owned()),
    );
    metadata.insert(
        String::from("deterministic_replay_outcome"),
        json!(outcome.failure_class()),
    );
    metadata.insert(String::from("not_real_model_inference"), Value::Bool(true));
    Ok(RunRecord {
        schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
        run_id: run_id_value.to_owned(),
        task_id: task_spec.task_id.clone(),
        task_version: task_spec.task_version.clone(),
        input_artifact_manifest_hash,
        run_config_hash,
        output_artifact_manifest_hash: artifact_manifest_digest(output_manifest)?,
        terminal_state,
        transcript: transcript(binding, task_spec, outcome),
        metrics: RunMetrics {
            model_turns: 1,
            tool_call_count: u32::try_from(tool_calls.len()).unwrap_or(u32::MAX),
            input_tokens: 320,
            output_tokens: if outcome == LegalBenchmarkEvalReplayOutcome::Pass {
                180
            } else {
                0
            },
            wall_time_ms: if outcome == LegalBenchmarkEvalReplayOutcome::Timeout {
                120_000
            } else {
                150
            },
            estimated_cost_micro_usd: 0,
        },
        tool_calls,
        extraction_receipt_refs: Vec::new(),
        coverage_snapshot: None,
        metadata,
    })
}

fn transcript(
    binding: &LegalBenchmarkEvalModelBinding,
    task_spec: &BenchmarkTaskSpec,
    outcome: LegalBenchmarkEvalReplayOutcome,
) -> Vec<TranscriptEvent> {
    vec![
        TranscriptEvent {
            event_index: 0,
            event_kind: TranscriptEventKind::System,
            role: Some(String::from("system")),
            content: Some(String::from("deterministic legal benchmark replay")),
            payload: None,
            timestamp_ms: 0,
        },
        TranscriptEvent {
            event_index: 1,
            event_kind: TranscriptEventKind::User,
            role: Some(String::from("user")),
            content: Some(task_spec.instructions.clone()),
            payload: None,
            timestamp_ms: 1,
        },
        TranscriptEvent {
            event_index: 2,
            event_kind: TranscriptEventKind::Assistant,
            role: Some(String::from("assistant")),
            content: Some(format!(
                "{} replay outcome: {}",
                binding.role,
                outcome.failure_class()
            )),
            payload: None,
            timestamp_ms: 2,
        },
    ]
}

fn task_report_from_score(
    binding: &LegalBenchmarkEvalModelBinding,
    task: &LegalBenchmarkEvalTaskFixture,
    outcome: LegalBenchmarkEvalReplayOutcome,
    score_report: &ScoreReport,
    score_report_hash: String,
    run_record_hash: String,
) -> LegalBenchmarkEvalTaskReport {
    let integrity_valid = score_report
        .metadata
        .get("answer_integrity")
        .and_then(|value| value.get("valid"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let submitted = outcome == LegalBenchmarkEvalReplayOutcome::Pass;
    let correct_path = submitted;
    let answer_file_success = score_report.all_pass && integrity_valid;
    let mut failure_classes = Vec::new();
    if !answer_file_success {
        failure_classes.push(String::from(outcome.failure_class()));
    }
    if !integrity_valid {
        failure_classes.push(String::from("integrity_failure"));
    }
    if outcome == LegalBenchmarkEvalReplayOutcome::ToolFailure {
        failure_classes.push(String::from("tool_failure"));
    }
    if outcome == LegalBenchmarkEvalReplayOutcome::Timeout {
        failure_classes.push(String::from("timeout"));
    }
    failure_classes.sort();
    failure_classes.dedup();
    LegalBenchmarkEvalTaskReport {
        task_id: task.task_id.clone(),
        model_role: binding.role.clone(),
        run_id: score_report.run_id.clone(),
        task_type: task_type(task),
        outcome,
        submitted,
        correct_path,
        answer_file_success,
        legal_score_bps: score_report.criterion_pass_rate_bps,
        integrity_valid,
        tool_failure: outcome == LegalBenchmarkEvalReplayOutcome::ToolFailure,
        timeout_failure: outcome == LegalBenchmarkEvalReplayOutcome::Timeout,
        failure_classes,
        score_report_hash,
        run_record_hash,
    }
}

fn model_report(
    binding: LegalBenchmarkEvalModelBinding,
    task_reports: Vec<LegalBenchmarkEvalTaskReport>,
) -> LegalBenchmarkEvalSuiteModelReport {
    let task_count = u64::try_from(task_reports.len()).unwrap_or(0);
    let answer_success_count = task_reports
        .iter()
        .filter(|task| task.answer_file_success)
        .count();
    let answer_write_count = task_reports.iter().filter(|task| task.correct_path).count();
    let submitted_count = task_reports.iter().filter(|task| task.submitted).count();
    let integrity_valid_count = task_reports
        .iter()
        .filter(|task| task.integrity_valid)
        .count();
    let scores = task_reports
        .iter()
        .map(|task| task.legal_score_bps)
        .collect::<Vec<_>>();
    let score_sum = task_reports
        .iter()
        .map(|task| u64::from(task.legal_score_bps))
        .sum::<u64>();
    let mut failure_class_counts = BTreeMap::new();
    for task in &task_reports {
        for class in &task.failure_classes {
            *failure_class_counts.entry(class.clone()).or_insert(0) += 1;
        }
    }
    let mean_legal_score_bps = average_bps(score_sum, task_count);
    LegalBenchmarkEvalSuiteModelReport {
        model_binding: binding,
        answer_file_write_rate_bps: ratio_bps(
            u64::try_from(answer_write_count).unwrap_or(0),
            task_count,
        ),
        answer_file_success_rate_bps: ratio_bps(
            u64::try_from(answer_success_count).unwrap_or(0),
            task_count,
        ),
        correct_path_rate_bps: ratio_bps(
            u64::try_from(answer_write_count).unwrap_or(0),
            task_count,
        ),
        submit_rate_bps: ratio_bps(u64::try_from(submitted_count).unwrap_or(0), task_count),
        integrity_valid_rate_bps: ratio_bps(
            u64::try_from(integrity_valid_count).unwrap_or(0),
            task_count,
        ),
        mean_legal_score_bps,
        median_legal_score_bps: median_bps(scores.as_slice()),
        legal_score_bps: mean_legal_score_bps,
        integrity_failure_count: u64::try_from(
            task_reports
                .iter()
                .filter(|task| !task.integrity_valid)
                .count(),
        )
        .unwrap_or(u64::MAX),
        tool_failure_count: u64::try_from(
            task_reports.iter().filter(|task| task.tool_failure).count(),
        )
        .unwrap_or(u64::MAX),
        timeout_failure_count: u64::try_from(
            task_reports
                .iter()
                .filter(|task| task.timeout_failure)
                .count(),
        )
        .unwrap_or(u64::MAX),
        failure_class_counts,
        score_by_task_type: score_by_task_type(task_reports.as_slice()),
        score_confidence_notes: score_confidence_notes(task_reports.as_slice()),
        task_reports,
    }
}

fn compare_model_reports(
    base: &LegalBenchmarkEvalSuiteModelReport,
    adapter: &LegalBenchmarkEvalSuiteModelReport,
) -> LegalBenchmarkEvalComparison {
    let score_delta_bps = i32::try_from(adapter.legal_score_bps).unwrap_or(i32::MAX)
        - i32::try_from(base.legal_score_bps).unwrap_or(i32::MAX);
    let answer_file_success_delta_bps = i32::try_from(adapter.answer_file_success_rate_bps)
        .unwrap_or(i32::MAX)
        - i32::try_from(base.answer_file_success_rate_bps).unwrap_or(i32::MAX);
    let integrity_failure_delta = i64::try_from(adapter.integrity_failure_count)
        .unwrap_or(i64::MAX)
        - i64::try_from(base.integrity_failure_count).unwrap_or(i64::MAX);
    let tool_failure_delta = i64::try_from(adapter.tool_failure_count).unwrap_or(i64::MAX)
        - i64::try_from(base.tool_failure_count).unwrap_or(i64::MAX);
    let timeout_failure_delta = i64::try_from(adapter.timeout_failure_count).unwrap_or(i64::MAX)
        - i64::try_from(base.timeout_failure_count).unwrap_or(i64::MAX);
    let regression_detected = score_delta_bps < 0
        || answer_file_success_delta_bps < 0
        || integrity_failure_delta > 0
        || tool_failure_delta > 0
        || timeout_failure_delta > 0;
    LegalBenchmarkEvalComparison {
        champion_legal_score_bps: base.legal_score_bps,
        candidate_legal_score_bps: adapter.legal_score_bps,
        score_delta_bps,
        regression_vs_champion_bps: score_delta_bps,
        answer_file_success_delta_bps,
        integrity_failure_delta,
        tool_failure_delta,
        timeout_failure_delta,
        regression_detected,
    }
}

fn score_by_task_type(
    task_reports: &[LegalBenchmarkEvalTaskReport],
) -> BTreeMap<String, LegalBenchmarkEvalTaskTypeScore> {
    let mut grouped = BTreeMap::<String, Vec<&LegalBenchmarkEvalTaskReport>>::new();
    for task in task_reports {
        grouped
            .entry(task.task_type.clone())
            .or_default()
            .push(task);
    }
    grouped
        .into_iter()
        .map(|(task_type, tasks)| {
            let task_count = u64::try_from(tasks.len()).unwrap_or(0);
            let scores = tasks
                .iter()
                .map(|task| task.legal_score_bps)
                .collect::<Vec<_>>();
            let score_sum = scores.iter().map(|score| u64::from(*score)).sum::<u64>();
            let answer_write_count = tasks.iter().filter(|task| task.correct_path).count();
            let submitted_count = tasks.iter().filter(|task| task.submitted).count();
            let integrity_valid_count = tasks.iter().filter(|task| task.integrity_valid).count();
            let mut failure_class_counts = BTreeMap::new();
            for task in &tasks {
                for class in &task.failure_classes {
                    *failure_class_counts.entry(class.clone()).or_insert(0) += 1;
                }
            }
            (
                task_type.clone(),
                LegalBenchmarkEvalTaskTypeScore {
                    task_type,
                    task_count,
                    mean_legal_score_bps: average_bps(score_sum, task_count),
                    median_legal_score_bps: median_bps(scores.as_slice()),
                    answer_file_write_rate_bps: ratio_bps(
                        u64::try_from(answer_write_count).unwrap_or(0),
                        task_count,
                    ),
                    correct_path_rate_bps: ratio_bps(
                        u64::try_from(answer_write_count).unwrap_or(0),
                        task_count,
                    ),
                    submit_rate_bps: ratio_bps(
                        u64::try_from(submitted_count).unwrap_or(0),
                        task_count,
                    ),
                    integrity_valid_rate_bps: ratio_bps(
                        u64::try_from(integrity_valid_count).unwrap_or(0),
                        task_count,
                    ),
                    failure_class_counts,
                },
            )
        })
        .collect()
}

fn score_confidence_notes(task_reports: &[LegalBenchmarkEvalTaskReport]) -> Vec<String> {
    let mut notes = vec![String::from(
        "Deterministic replay fixture: scores measure harness behavior for declared outcomes, not live model legal quality or performance on private Harvey tasks.",
    )];
    if task_reports.len() < 10 {
        notes.push(format!(
            "Small suite: {} task(s), so use this only as a smoke/workflow gate.",
            task_reports.len()
        ));
    } else {
        notes.push(format!(
            "Mixed suite: {} task(s) across {} task type(s); use as a local regression signal before retained evaluation.",
            task_reports.len(),
            task_reports
                .iter()
                .map(|task| task.task_type.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        ));
    }
    notes
}

fn promotion_gate_input(
    manifest: &LegalBenchmarkEvalSuiteManifest,
    suite_hash: &str,
    base: &LegalBenchmarkEvalSuiteModelReport,
    adapter: &LegalBenchmarkEvalSuiteModelReport,
    comparison: &LegalBenchmarkEvalComparison,
    replay_command: &[String],
) -> LegalBenchmarkEvalPromotionGateInput {
    let decision = if comparison.regression_detected {
        LegalBenchmarkPromotionDecision::Reject
    } else if adapter.integrity_failure_count == 0
        && adapter.tool_failure_count == 0
        && adapter.timeout_failure_count == 0
        && adapter.legal_score_bps > base.legal_score_bps
    {
        LegalBenchmarkPromotionDecision::Promote
    } else {
        LegalBenchmarkPromotionDecision::Hold
    };
    LegalBenchmarkEvalPromotionGateInput {
        schema_version: LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION,
        suite_id: manifest.suite_id.clone(),
        suite_hash: suite_hash.to_owned(),
        baseline_model_id: base.model_binding.model_id.clone(),
        candidate_model_id: adapter.model_binding.model_id.clone(),
        candidate_adapter_id: adapter.model_binding.adapter_id.clone(),
        baseline_legal_score_bps: base.legal_score_bps,
        candidate_legal_score_bps: adapter.legal_score_bps,
        score_delta_bps: comparison.score_delta_bps,
        candidate_answer_file_success_rate_bps: adapter.answer_file_success_rate_bps,
        candidate_integrity_failure_count: adapter.integrity_failure_count,
        candidate_tool_failure_count: adapter.tool_failure_count,
        candidate_timeout_failure_count: adapter.timeout_failure_count,
        regression_detected: comparison.regression_detected,
        decision,
        replay_command: replay_command.to_vec(),
    }
}

fn write_eval_suite_report(
    output_dir: &Path,
    report: &LegalBenchmarkEvalSuiteReport,
) -> Result<Vec<String>, LegalBenchmarkEvalSuiteError> {
    let promotion_path = output_dir.join("promotion_gate_input.json");
    write_file_pretty(promotion_path.as_path(), &report.promotion_gate_input)?;
    Ok(vec![
        String::from("eval_report.json"),
        String::from("promotion_gate_input.json"),
        String::from("replay_receipt.json"),
        String::from("base/report.md"),
        String::from("base/autopilot_report.json"),
        String::from("adapter/report.md"),
        String::from("adapter/autopilot_report.json"),
    ])
}

fn source_artifact_from_document(
    suite_path: &Path,
    document: &LegalBenchmarkEvalSourceDocument,
) -> Result<SourceArtifact, LegalBenchmarkEvalSuiteError> {
    let path = resolve_suite_relative_path(suite_path, document.relative_path.as_str());
    let root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut artifact = artifact_from_file(
        document.document_id.clone(),
        ArtifactKind::SourceDocument,
        root.as_path(),
        path.as_path(),
        DataClassification::PublicReference,
        Some(String::from("eval_suite_manifest")),
    )?;
    artifact.relative_path = document.relative_path.clone();
    artifact.media_type = document.media_type.clone();
    artifact.byte_size = document.byte_size;
    artifact.sha256 = document.sha256.clone();
    Ok(artifact)
}

fn adapter_binding(
    adapter: &str,
) -> Result<LegalBenchmarkEvalModelBinding, LegalBenchmarkEvalSuiteError> {
    let path = PathBuf::from(adapter);
    if path.is_file() {
        let bytes = read_file(path.as_path())?;
        Ok(LegalBenchmarkEvalModelBinding {
            role: String::from("adapter"),
            model_id: String::from("qwen3.6-legal-adapter"),
            adapter_id: Some(adapter.to_owned()),
            artifact_path: Some(adapter.to_owned()),
            artifact_sha256: Some(sha256_hex(&bytes)),
            artifact_materialized: true,
        })
    } else {
        Ok(LegalBenchmarkEvalModelBinding {
            role: String::from("adapter"),
            model_id: String::from("qwen3.6-legal-adapter"),
            adapter_id: Some(adapter.to_owned()),
            artifact_path: Some(adapter.to_owned()),
            artifact_sha256: Some(stable_json_digest(
                "psionic.legal_benchmark.eval_suite.adapter_id.v1",
                &adapter,
            )?),
            artifact_materialized: false,
        })
    }
}

fn write_tool_call(
    tool_call_id: String,
    relative_path: &str,
    content: &str,
    error_kind: Option<&str>,
) -> ToolCallRecord {
    let after_hash = sha256_hex(content.as_bytes());
    ToolCallRecord {
        tool_call_id,
        tool_name: String::from("write"),
        call_event_index: 3,
        result_event_index: Some(4),
        input: json!({
            "tool": "write",
            "input": {
                "root": "output",
                "relative_path": relative_path,
                "content": content,
                "overwrite": true
            }
        }),
        output: if error_kind.is_some() {
            None
        } else {
            Some(json!({
                "tool": "write",
                "output": {
                    "relative_path": relative_path,
                    "bytes_written": content.len(),
                    "after_hash": after_hash
                }
            }))
        },
        error_kind: error_kind.map(str::to_owned),
        elapsed_ms: 1,
    }
}

fn answer_content(
    _manifest: &LegalBenchmarkEvalSuiteManifest,
    _binding: &LegalBenchmarkEvalModelBinding,
    task: &LegalBenchmarkEvalTaskFixture,
) -> String {
    task.replay_answer_markdown.clone()
}

fn run_id(binding: &LegalBenchmarkEvalModelBinding, task_id: &str) -> String {
    format!("run.{}.{}", binding.role, sanitize_path_component(task_id))
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

fn task_type(task: &LegalBenchmarkEvalTaskFixture) -> String {
    format!("{}/{}", task.practice_area, task.workflow)
}

fn resolve_suite_relative_path(suite_path: &Path, relative_path: &str) -> PathBuf {
    let path = PathBuf::from(relative_path);
    if path.is_absolute() || path.exists() {
        path
    } else {
        suite_path
            .parent()
            .map(|parent| parent.join(relative_path))
            .unwrap_or(path)
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, LegalBenchmarkEvalSuiteError> {
    fs::read(path).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json_file<T>(path: &Path) -> Result<T, LegalBenchmarkEvalSuiteError>
where
    T: for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_slice(&read_file(path)?)?)
}

fn write_file_pretty<T>(path: &Path, value: &T) -> Result<(), LegalBenchmarkEvalSuiteError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| LegalBenchmarkEvalSuiteError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn ratio_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from(numerator.saturating_mul(10_000) / denominator).unwrap_or(u32::MAX)
}

fn average_bps(sum: u64, count: u64) -> u32 {
    if count == 0 {
        return 0;
    }
    u32::try_from(sum / count).unwrap_or(u32::MAX)
}

fn median_bps(scores: &[u32]) -> u32 {
    if scores.is_empty() {
        return 0;
    }
    let mut sorted = scores.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        u32::try_from((u64::from(sorted[middle - 1]) + u64::from(sorted[middle])) / 2)
            .unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_suite() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_dir = temp.path().join("fixtures");
        fs::create_dir_all(&source_dir).expect("source dir");
        let source_path = source_dir.join("lease.txt");
        let source = "Lease says notice must be written within ten days.\n";
        fs::write(&source_path, source).expect("source write");
        let hash = sha256_hex(source.as_bytes());
        let manifest = LegalBenchmarkEvalSuiteManifest {
            schema_version: LEGAL_BENCHMARK_EVAL_SUITE_SCHEMA_VERSION,
            suite_id: String::from("test.public_three"),
            mode: LegalBenchmarkEvalMode::PublicHarveyThreeTask,
            fixed_task_order: vec![String::from("task.one"), String::from("task.two")],
            prompt_template_id: String::from("legal.autopilot.v1"),
            prompt_template_hash: String::from("prompt-hash"),
            scorer_version: String::from("rust-local-scorer-v1"),
            inference_settings: LegalBenchmarkEvalInferenceSettings {
                temperature: 0.0,
                top_p: 1.0,
                max_output_tokens: 1024,
                seed: 7,
            },
            source_documents: vec![LegalBenchmarkEvalSourceDocument {
                document_id: String::from("lease"),
                relative_path: source_path.to_string_lossy().to_string(),
                media_type: String::from("text/plain"),
                byte_size: u64::try_from(source.len()).expect("source length"),
                sha256: hash,
            }],
            tasks: vec![
                task("task.one", LegalBenchmarkEvalReplayOutcome::MissingAnswer),
                task("task.two", LegalBenchmarkEvalReplayOutcome::Pass),
            ],
            training_allowed: true,
            metadata: Metadata::new(),
        };
        let suite_path = temp.path().join("suite.json");
        fs::write(
            &suite_path,
            serde_json::to_vec_pretty(&manifest).expect("manifest json"),
        )
        .expect("manifest write");
        (temp, suite_path)
    }

    fn task(
        task_id: &str,
        base_outcome: LegalBenchmarkEvalReplayOutcome,
    ) -> LegalBenchmarkEvalTaskFixture {
        LegalBenchmarkEvalTaskFixture {
            task_id: String::from(task_id),
            task_version: String::from("v1"),
            title: String::from("Task"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            instructions: String::from("Write the answer."),
            source_document_ids: vec![String::from("lease")],
            required_answer_path: String::from("answer.md"),
            base_outcome,
            adapter_outcome: LegalBenchmarkEvalReplayOutcome::Pass,
            replay_answer_markdown: String::from(
                "# Answer\n\nThe lease requires written notice within ten days.\n",
            ),
        }
    }

    #[test]
    fn eval_suite_compares_base_and_adapter() {
        let (_temp, suite_path) = write_temp_suite();
        let output = tempfile::tempdir().expect("output");
        let report = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
            suite_path,
            base_model: String::from("qwen3.6-27b"),
            adapter: String::from("adapter-smoke"),
            output_dir: output.path().join("run"),
            replay_command: vec![String::from("legal_benchmark_eval_suite")],
        })
        .expect("eval suite");

        assert_eq!(report.fixed_task_order, vec!["task.one", "task.two"]);
        assert!(report.adapter_result.legal_score_bps > report.base_model_result.legal_score_bps);
        assert_eq!(report.adapter_result.integrity_failure_count, 0);
        assert_eq!(
            report.promotion_gate_input.decision,
            LegalBenchmarkPromotionDecision::Promote
        );
        assert!(output.path().join("run/eval_report.json").is_file());
        assert!(output.path().join("run/replay_receipt.json").is_file());
        let answer = fs::read_to_string(output.path().join("run/adapter/task.one/answer.md"))
            .expect("adapter answer");
        assert_eq!(
            answer,
            "# Answer\n\nThe lease requires written notice within ten days.\n"
        );
        assert!(!answer.contains("Model role:"));
        assert!(!answer.contains("Prompt template:"));
    }

    #[test]
    fn hidden_suite_cannot_be_marked_training_allowed() {
        let (_temp, suite_path) = write_temp_suite();
        let mut manifest = serde_json::from_slice::<LegalBenchmarkEvalSuiteManifest>(
            &fs::read(&suite_path).expect("read suite"),
        )
        .expect("json");
        manifest.mode = LegalBenchmarkEvalMode::HiddenAuditOnly;
        manifest.training_allowed = true;
        fs::write(
            &suite_path,
            serde_json::to_vec_pretty(&manifest).expect("json"),
        )
        .expect("write suite");
        let output = tempfile::tempdir().expect("output");
        let error = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
            suite_path,
            base_model: String::from("qwen3.6-27b"),
            adapter: String::from("adapter-smoke"),
            output_dir: output.path().join("run"),
            replay_command: Vec::new(),
        })
        .expect_err("hidden suite should fail");

        assert!(error.to_string().contains("hidden audit"));
    }
}
