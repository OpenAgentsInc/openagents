//! Verifier reward traces for legal benchmark RL training.
//!
//! Reward traces are built from run receipts, score reports, output manifests,
//! and answer integrity reports. They intentionally avoid judge-private or
//! hidden benchmark details.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_answer_integrity_report, build_output_artifact_manifest, run_record_digest,
    score_report_digest, stable_json_digest, ArtifactManifest, BenchmarkIntegrityError,
    BenchmarkTaskSpec, LegalBenchmarkAnswerIntegrityReport, Metadata, RunActor, RunRecord,
    RunTerminalState, ScoreReport,
};

/// Schema version for each verifier reward trace line.
pub const LEGAL_REWARD_TRACE_SCHEMA_VERSION: &str = "psionic.legal_benchmark.reward_trace.v1";
/// Schema version for the reward trace dataset manifest.
pub const LEGAL_REWARD_TRACE_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.reward_trace_manifest.v1";

const DEFAULT_DATASET_ID: &str = "legal-reward-v1";
const ANSWER_LENGTH_MAX_BYTES: u64 = 100_000;

/// Reward components used by GRPO-style legal benchmark training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalReward {
    pub wrote_required_file: f32,
    pub correct_path: f32,
    pub non_empty_answer: f32,
    pub answer_length_ok: f32,
    pub source_usage_ok: f32,
    pub submitted_ok: f32,
    pub integrity_valid: f32,
    pub public_score_delta: f32,
    pub no_hidden_leakage: f32,
}

/// Initial reward weights for legal workflow traces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRewardWeights {
    pub wrote_required_file: f32,
    pub correct_path: f32,
    pub non_empty_answer: f32,
    pub answer_length_ok: f32,
    pub source_usage_ok: f32,
    pub submitted_ok: f32,
    pub integrity_valid: f32,
    pub public_score_delta: f32,
}

impl Default for LegalRewardWeights {
    fn default() -> Self {
        Self {
            wrote_required_file: 3.0,
            correct_path: 2.0,
            non_empty_answer: 1.0,
            answer_length_ok: 1.0,
            source_usage_ok: 1.0,
            submitted_ok: 1.0,
            integrity_valid: 3.0,
            public_score_delta: 1.0,
        }
    }
}

/// Evidence kept beside a reward trace for replay and audit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRewardTraceEvidence {
    pub run_dir: String,
    pub terminal_state: RunTerminalState,
    pub required_answer_paths: Vec<String>,
    pub observed_answer_paths: Vec<String>,
    pub model_written_paths: Vec<String>,
    pub integrity_invalid_reasons: Vec<String>,
    pub score_failure_diagnostics: Vec<String>,
    pub failed_criterion_ids: Vec<String>,
    pub answer_integrity_report_hash: String,
    pub source_usage_basis: String,
}

/// One deterministic verifier reward trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalVerifierRewardTrace {
    pub schema_version: String,
    pub trace_id: String,
    pub run_id: String,
    pub task_id: String,
    pub task_version: String,
    pub run_record_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_report_hash: Option<String>,
    pub components: LegalReward,
    pub weights: LegalRewardWeights,
    pub workflow_reward: f32,
    pub legal_content_score_bps: u32,
    pub legal_content_reward: f32,
    pub total_reward: f32,
    pub fatal_excluded: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusion_reasons: Vec<String>,
    pub evidence: LegalRewardTraceEvidence,
    pub trace_digest: String,
}

impl LegalVerifierRewardTrace {
    /// Computes the stable trace digest with the digest field cleared.
    pub fn stable_digest(&self) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        clone.trace_digest.clear();
        stable_json_digest("psionic.legal_benchmark.reward_trace.v1", &clone)
    }
}

/// Dataset manifest emitted next to the reward JSONL file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRewardTraceManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub output_jsonl: String,
    pub total_count: usize,
    pub included_count: usize,
    pub excluded_count: usize,
    pub excluded_reasons: BTreeMap<String, usize>,
    pub trace_hashes: Vec<String>,
    pub dataset_hash: String,
}

/// Builder config for reward trace datasets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegalRewardTraceBuilderConfig {
    pub runs_root: PathBuf,
    pub out_jsonl: PathBuf,
    pub manifest_json: PathBuf,
    pub dataset_id: String,
}

impl LegalRewardTraceBuilderConfig {
    /// Creates a config using the default manifest path beside the output file.
    #[must_use]
    pub fn new(runs_root: PathBuf, out_jsonl: PathBuf) -> Self {
        Self {
            runs_root,
            manifest_json: default_manifest_path(out_jsonl.as_path()),
            out_jsonl,
            dataset_id: String::from(DEFAULT_DATASET_ID),
        }
    }
}

/// Result returned by the reward trace builder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRewardTraceBuildResult {
    pub traces: Vec<LegalVerifierRewardTrace>,
    pub manifest: LegalRewardTraceManifest,
}

/// Errors raised while building legal reward traces.
#[derive(Debug, Error)]
pub enum LegalRewardTraceError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("reward trace digest failed: {0}")]
    Digest(#[from] serde_json::Error),
    #[error("answer integrity failed: {0}")]
    Integrity(#[from] BenchmarkIntegrityError),
    #[error("no run_record.json files found under {0}")]
    NoRuns(PathBuf),
}

/// Builds reward traces from all run directories under `runs_root`.
pub fn build_legal_benchmark_reward_traces(
    config: &LegalRewardTraceBuilderConfig,
) -> Result<LegalRewardTraceBuildResult, LegalRewardTraceError> {
    let mut run_dirs = Vec::new();
    discover_run_dirs(config.runs_root.as_path(), &mut run_dirs)?;
    run_dirs.sort();
    if run_dirs.is_empty() {
        return Err(LegalRewardTraceError::NoRuns(config.runs_root.clone()));
    }

    let mut traces = run_dirs
        .iter()
        .map(|run_dir| build_legal_reward_trace_from_run_dir(run_dir))
        .collect::<Result<Vec<_>, _>>()?;
    traces.sort_by(|left, right| left.run_id.cmp(&right.run_id));

    let jsonl = render_jsonl(traces.as_slice())?;
    if let Some(parent) = config.out_jsonl.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalRewardTraceError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(config.out_jsonl.as_path(), jsonl.as_bytes()).map_err(|source| {
        LegalRewardTraceError::Io {
            path: config.out_jsonl.clone(),
            source,
        }
    })?;

    let manifest = reward_manifest(config, traces.as_slice(), jsonl.as_bytes());
    if let Some(parent) = config.manifest_json.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalRewardTraceError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(
        config.manifest_json.as_path(),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .map_err(|source| LegalRewardTraceError::Io {
        path: config.manifest_json.clone(),
        source,
    })?;

    Ok(LegalRewardTraceBuildResult { traces, manifest })
}

/// Builds one reward trace from a run directory.
pub fn build_legal_reward_trace_from_run_dir(
    run_dir: impl AsRef<Path>,
) -> Result<LegalVerifierRewardTrace, LegalRewardTraceError> {
    let run_dir = run_dir.as_ref();
    let run_record_path = run_dir.join("run_record.json");
    let run_record = read_required_json::<RunRecord>(run_record_path.as_path())?;
    let score_report = read_optional_json::<ScoreReport>(run_dir.join("score_report.json"))?;
    let task_spec = read_optional_json::<BenchmarkTaskSpec>(run_dir.join("task_spec.json"))?;
    let output_manifest = read_output_manifest(run_dir, &run_record)?;
    let output_root = output_root(run_dir);
    let answer_integrity = answer_integrity_report(
        run_dir,
        task_spec.as_ref(),
        &run_record,
        &output_manifest,
        output_root.as_path(),
        score_report.as_ref(),
    )?;
    let weights = LegalRewardWeights::default();
    let required_paths = required_paths(task_spec.as_ref(), &answer_integrity);
    let observed_paths = observed_paths(&answer_integrity, &output_manifest);
    let model_written_paths = model_written_paths(&answer_integrity);
    let hidden_leakage = hidden_leakage_detected(&run_record, score_report.as_ref());
    let harness_assisted = harness_assisted_output(&answer_integrity, &run_record);
    let components = reward_components(
        &run_record,
        score_report.as_ref(),
        &answer_integrity,
        &required_paths,
        hidden_leakage,
        task_spec.as_ref(),
    );
    let workflow_reward = workflow_reward(&components, &weights);
    let legal_content_score_bps = score_report
        .as_ref()
        .map(|report| report.criterion_pass_rate_bps)
        .unwrap_or(0);
    let legal_content_reward = legal_content_score_bps as f32 / 10_000.0;
    let mut exclusion_reasons = Vec::new();
    if hidden_leakage {
        exclusion_reasons.push(String::from("hidden_or_private_scoring_leakage"));
    }
    if harness_assisted {
        exclusion_reasons.push(String::from("harness_assisted_output"));
    }
    let fatal_excluded = !exclusion_reasons.is_empty();
    let total_reward = if fatal_excluded {
        0.0
    } else {
        workflow_reward + components.public_score_delta * weights.public_score_delta
    };
    let score_report_hash = score_report.as_ref().map(score_report_digest).transpose()?;
    let evidence = LegalRewardTraceEvidence {
        run_dir: run_dir.display().to_string(),
        terminal_state: run_record.terminal_state,
        required_answer_paths: required_paths,
        observed_answer_paths: observed_paths,
        model_written_paths,
        integrity_invalid_reasons: answer_integrity.invalid_reasons.clone(),
        score_failure_diagnostics: score_report
            .as_ref()
            .map(|report| report.failure_diagnostics.clone())
            .unwrap_or_default(),
        failed_criterion_ids: score_report
            .as_ref()
            .map(failed_criterion_ids)
            .unwrap_or_default(),
        answer_integrity_report_hash: answer_integrity.report_hash.clone(),
        source_usage_basis: source_usage_basis(&run_record, task_spec.as_ref()),
    };
    let mut trace = LegalVerifierRewardTrace {
        schema_version: String::from(LEGAL_REWARD_TRACE_SCHEMA_VERSION),
        trace_id: format!("reward.{}", run_record.run_id),
        run_id: run_record.run_id.clone(),
        task_id: run_record.task_id.clone(),
        task_version: run_record.task_version.clone(),
        run_record_hash: run_record_digest(&run_record)?,
        score_report_hash,
        components,
        weights,
        workflow_reward,
        legal_content_score_bps,
        legal_content_reward,
        total_reward,
        fatal_excluded,
        exclusion_reasons,
        evidence,
        trace_digest: String::new(),
    };
    trace.trace_digest = trace.stable_digest()?;
    Ok(trace)
}

fn reward_components(
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
    required_paths: &[String],
    hidden_leakage: bool,
    task_spec: Option<&BenchmarkTaskSpec>,
) -> LegalReward {
    let required_answer_files = answer_integrity
        .answer_files
        .iter()
        .filter(|file| file.required_by_task)
        .collect::<Vec<_>>();
    let required_files_exist = if required_answer_files.is_empty() {
        !required_paths.is_empty()
            && required_paths.iter().all(|path| {
                answer_integrity
                    .answer_files
                    .iter()
                    .any(|file| file.relative_path == *path && file.exists && file.required_by_task)
            })
    } else {
        required_answer_files.iter().all(|file| file.exists)
    };
    let non_empty_answer = required_answer_files
        .iter()
        .filter(|file| file.exists)
        .all(|file| file.byte_size.unwrap_or(0) > 0);
    let answer_length_ok = required_answer_files
        .iter()
        .filter(|file| file.exists)
        .all(|file| {
            let size = file.byte_size.unwrap_or(0);
            size > 0 && size <= ANSWER_LENGTH_MAX_BYTES
        });
    let public_score_delta = public_score_delta(score_report);
    LegalReward {
        wrote_required_file: bool_reward(required_files_exist),
        correct_path: bool_reward(
            required_files_exist && paths_match(required_paths, answer_integrity),
        ),
        non_empty_answer: bool_reward(required_files_exist && non_empty_answer),
        answer_length_ok: bool_reward(required_files_exist && answer_length_ok),
        source_usage_ok: bool_reward(source_usage_ok(run_record, task_spec)),
        submitted_ok: bool_reward(run_record.terminal_state == RunTerminalState::Submitted),
        integrity_valid: bool_reward(answer_integrity.valid),
        public_score_delta,
        no_hidden_leakage: bool_reward(!hidden_leakage),
    }
}

fn workflow_reward(components: &LegalReward, weights: &LegalRewardWeights) -> f32 {
    components.wrote_required_file * weights.wrote_required_file
        + components.correct_path * weights.correct_path
        + components.non_empty_answer * weights.non_empty_answer
        + components.answer_length_ok * weights.answer_length_ok
        + components.source_usage_ok * weights.source_usage_ok
        + components.submitted_ok * weights.submitted_ok
        + components.integrity_valid * weights.integrity_valid
}

fn answer_integrity_report(
    run_dir: &Path,
    task_spec: Option<&BenchmarkTaskSpec>,
    run_record: &RunRecord,
    output_manifest: &ArtifactManifest,
    output_root: &Path,
    score_report: Option<&ScoreReport>,
) -> Result<LegalBenchmarkAnswerIntegrityReport, LegalRewardTraceError> {
    if let Some(report) = run_receipt_answer_integrity(run_dir.join("run_receipt.json").as_path())?
    {
        return Ok(report);
    }
    if let Some(report) = score_report.and_then(score_answer_integrity) {
        return Ok(report);
    }
    if let Some(task_spec) = task_spec {
        return Ok(build_answer_integrity_report(
            task_spec,
            output_manifest,
            output_root,
            &run_record.tool_calls,
        )?);
    }
    Ok(LegalBenchmarkAnswerIntegrityReport::default())
}

fn run_receipt_answer_integrity(
    path: &Path,
) -> Result<Option<LegalBenchmarkAnswerIntegrityReport>, LegalRewardTraceError> {
    if !path.is_file() {
        return Ok(None);
    }
    let value = read_required_json::<Value>(path)?;
    let Some(answer_integrity) = value.get("answer_integrity") else {
        return Ok(None);
    };
    serde_json::from_value(answer_integrity.clone())
        .map(Some)
        .map_err(|source| LegalRewardTraceError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn score_answer_integrity(
    score_report: &ScoreReport,
) -> Option<LegalBenchmarkAnswerIntegrityReport> {
    score_report
        .metadata
        .get("answer_integrity")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn read_output_manifest(
    run_dir: &Path,
    run_record: &RunRecord,
) -> Result<ArtifactManifest, LegalRewardTraceError> {
    for file_name in ["output_manifest.json", "output_artifact_manifest.json"] {
        let path = run_dir.join(file_name);
        if path.is_file() {
            return read_required_json::<ArtifactManifest>(path.as_path());
        }
    }
    Ok(build_output_artifact_manifest(
        run_record.task_id.clone(),
        run_record.task_version.clone(),
        run_record.run_id.clone(),
        Vec::new(),
    ))
}

fn required_paths(
    task_spec: Option<&BenchmarkTaskSpec>,
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    if let Some(task_spec) = task_spec {
        for deliverable in task_spec
            .deliverables
            .iter()
            .filter(|deliverable| deliverable.required)
        {
            paths.insert(deliverable.required_path.clone());
        }
    }
    for file in answer_integrity
        .answer_files
        .iter()
        .filter(|file| file.required_by_task)
    {
        paths.insert(file.relative_path.clone());
    }
    paths.into_iter().collect()
}

fn observed_paths(
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
    output_manifest: &ArtifactManifest,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for file in answer_integrity
        .answer_files
        .iter()
        .filter(|file| file.exists)
    {
        paths.insert(file.relative_path.clone());
    }
    for artifact in &output_manifest.artifacts {
        paths.insert(artifact.relative_path.clone());
    }
    paths.into_iter().collect()
}

fn model_written_paths(answer_integrity: &LegalBenchmarkAnswerIntegrityReport) -> Vec<String> {
    answer_integrity
        .answer_files
        .iter()
        .filter(|file| file.writer_tool_call_id.is_some())
        .map(|file| file.relative_path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn paths_match(
    required_paths: &[String],
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
) -> bool {
    if required_paths.is_empty() {
        return false;
    }
    required_paths.iter().all(|required_path| {
        answer_integrity.answer_files.iter().any(|file| {
            file.required_by_task && file.relative_path == *required_path && file.exists
        })
    })
}

fn source_usage_ok(run_record: &RunRecord, task_spec: Option<&BenchmarkTaskSpec>) -> bool {
    if task_spec
        .map(|task| task.source_artifacts.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    if run_record
        .coverage_snapshot
        .as_ref()
        .is_some_and(|snapshot| {
            snapshot.documents.iter().any(|document| document.read)
                || !snapshot.evidence_refs.is_empty()
        })
    {
        return true;
    }
    if run_record
        .tool_calls
        .iter()
        .any(is_source_reading_tool_call)
    {
        return true;
    }
    false
}

fn source_usage_basis(run_record: &RunRecord, task_spec: Option<&BenchmarkTaskSpec>) -> String {
    if task_spec
        .map(|task| task.source_artifacts.is_empty())
        .unwrap_or(false)
    {
        return String::from("task_has_no_source_artifacts");
    }
    if run_record
        .coverage_snapshot
        .as_ref()
        .is_some_and(|snapshot| {
            snapshot.documents.iter().any(|document| document.read)
                || !snapshot.evidence_refs.is_empty()
        })
    {
        return String::from("run_record_coverage_snapshot");
    }
    if run_record
        .tool_calls
        .iter()
        .any(is_source_reading_tool_call)
    {
        return String::from("document_root_tool_call");
    }
    String::from("no_source_usage_evidence")
}

fn is_source_reading_tool_call(call: &crate::ToolCallRecord) -> bool {
    if call.error_kind.is_some() {
        return false;
    }
    if !matches!(
        call.tool_name.as_str(),
        "read"
            | "grep"
            | "glob"
            | "inventory"
            | "email_summary"
            | "spreadsheet_summary"
            | "pdf_search"
            | "evidence_table"
    ) {
        return false;
    }
    tool_payload(&call.input)
        .and_then(|input| input.get("root"))
        .and_then(Value::as_str)
        == Some("documents")
}

fn tool_payload(value: &Value) -> Option<&Value> {
    value.get("input").unwrap_or(value).as_object()?;
    Some(value.get("input").unwrap_or(value))
}

fn public_score_delta(score_report: Option<&ScoreReport>) -> f32 {
    let Some(score_report) = score_report else {
        return 0.0;
    };
    metadata_i64(&score_report.metadata, "public_score_delta_bps")
        .or_else(|| metadata_i64(&score_report.metadata, "score_delta_bps"))
        .map(|delta| delta as f32 / 10_000.0)
        .unwrap_or(score_report.criterion_pass_rate_bps as f32 / 10_000.0)
}

fn hidden_leakage_detected(run_record: &RunRecord, score_report: Option<&ScoreReport>) -> bool {
    run_record
        .coverage_snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.hidden_criteria_visible)
        || metadata_hidden_leakage(&run_record.metadata)
        || score_report.is_some_and(|report| {
            report
                .coverage_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.hidden_criteria_visible)
                || metadata_hidden_leakage(&report.metadata)
        })
}

fn metadata_hidden_leakage(metadata: &Metadata) -> bool {
    metadata.iter().any(|(key, value)| {
        (key == "hidden_criteria_visible" && value.as_bool() == Some(true))
            || (key == "benchmark_visibility"
                && value
                    .as_str()
                    .is_some_and(|visibility| matches!(visibility, "hidden" | "private")))
            || value_contains_leak_marker(value)
    })
}

fn value_contains_leak_marker(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let text = text.to_ascii_lowercase();
            [
                "hidden benchmark answer",
                "hidden scoring label",
                "scorer-only target",
                "hidden criterion material",
            ]
            .iter()
            .any(|marker| text.contains(marker))
        }
        Value::Array(values) => values.iter().any(value_contains_leak_marker),
        Value::Object(map) => map.values().any(value_contains_leak_marker),
        _ => false,
    }
}

fn harness_assisted_output(
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
    run_record: &RunRecord,
) -> bool {
    answer_integrity.answer_files.iter().any(|file| {
        file.exists
            && (matches!(file.creation_actor, RunActor::Harness | RunActor::Scorer)
                || matches!(
                    file.last_modifying_actor,
                    RunActor::Harness | RunActor::Scorer
                )
                || file
                    .failure_reasons
                    .iter()
                    .any(|reason| harness_assist_reason(reason)))
    }) || answer_integrity
        .invalid_reasons
        .iter()
        .any(|reason| harness_assist_reason(reason))
        || metadata_contains_harness_marker(&run_record.metadata)
}

fn harness_assist_reason(reason: &str) -> bool {
    [
        "answer_file_has_no_model_write",
        "answer_file_changed_during_scoring",
        "actual_hash_does_not_match_model_write_after_hash",
        "harness-injected",
    ]
    .iter()
    .any(|marker| reason.contains(marker))
}

fn metadata_contains_harness_marker(metadata: &Metadata) -> bool {
    metadata.values().any(|value| match value {
        Value::String(text) => text.contains("harness-injected"),
        Value::Array(values) => values
            .iter()
            .any(|nested| metadata_contains_harness_marker_value(nested)),
        Value::Object(map) => map.values().any(metadata_contains_harness_marker_value),
        _ => false,
    })
}

fn metadata_contains_harness_marker_value(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("harness-injected"),
        Value::Array(values) => values.iter().any(metadata_contains_harness_marker_value),
        Value::Object(map) => map.values().any(metadata_contains_harness_marker_value),
        _ => false,
    }
}

fn failed_criterion_ids(score_report: &ScoreReport) -> Vec<String> {
    score_report
        .criterion_results
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.criterion_id.clone())
        .collect()
}

fn metadata_i64(metadata: &Metadata, key: &str) -> Option<i64> {
    metadata.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
    })
}

fn bool_reward(value: bool) -> f32 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn discover_run_dirs(dir: &Path, run_dirs: &mut Vec<PathBuf>) -> Result<(), LegalRewardTraceError> {
    if dir.join("run_record.json").is_file() {
        run_dirs.push(dir.to_path_buf());
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|source| LegalRewardTraceError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| LegalRewardTraceError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            discover_run_dirs(path.as_path(), run_dirs)?;
        }
    }
    Ok(())
}

fn output_root(run_dir: &Path) -> PathBuf {
    let nested_output = run_dir.join("output");
    if nested_output.is_dir() {
        nested_output
    } else {
        run_dir.to_path_buf()
    }
}

fn render_jsonl(traces: &[LegalVerifierRewardTrace]) -> Result<String, serde_json::Error> {
    let mut out = String::new();
    for trace in traces {
        out.push_str(serde_json::to_string(trace)?.as_str());
        out.push('\n');
    }
    Ok(out)
}

fn reward_manifest(
    config: &LegalRewardTraceBuilderConfig,
    traces: &[LegalVerifierRewardTrace],
    jsonl: &[u8],
) -> LegalRewardTraceManifest {
    let mut excluded_reasons = BTreeMap::new();
    for trace in traces {
        for reason in &trace.exclusion_reasons {
            *excluded_reasons.entry(reason.clone()).or_insert(0) += 1;
        }
    }
    LegalRewardTraceManifest {
        schema_version: String::from(LEGAL_REWARD_TRACE_MANIFEST_SCHEMA_VERSION),
        dataset_id: config.dataset_id.clone(),
        output_jsonl: config.out_jsonl.display().to_string(),
        total_count: traces.len(),
        included_count: traces.iter().filter(|trace| !trace.fatal_excluded).count(),
        excluded_count: traces.iter().filter(|trace| trace.fatal_excluded).count(),
        excluded_reasons,
        trace_hashes: traces
            .iter()
            .map(|trace| trace.trace_digest.clone())
            .collect(),
        dataset_hash: sha256_hex(jsonl),
    }
}

fn default_manifest_path(out_jsonl: &Path) -> PathBuf {
    let mut path = out_jsonl.to_path_buf();
    path.set_extension("manifest.json");
    path
}

fn read_required_json<T>(path: &Path) -> Result<T, LegalRewardTraceError>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|source| LegalRewardTraceError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|source| LegalRewardTraceError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional_json<T>(path: impl AsRef<Path>) -> Result<Option<T>, LegalRewardTraceError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    if !path.is_file() {
        return Ok(None);
    }
    read_required_json(path).map(Some)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifact_from_file, artifact_manifest_digest, ArtifactKind, CriterionResult,
        CriterionVerdict, DataClassification, RunMetrics, TranscriptEvent, TranscriptEventKind,
    };
    use serde_json::json;

    #[test]
    fn legal_reward_penalizes_missing_required_file() -> Result<(), Box<dyn std::error::Error>> {
        let run_dir = PathBuf::from("../../fixtures/legal_benchmark/failed_run_missing_file");
        let trace = build_legal_reward_trace_from_run_dir(run_dir)?;

        assert_eq!(trace.components.wrote_required_file, 0.0);
        assert_eq!(trace.components.correct_path, 0.0);
        assert_eq!(trace.components.non_empty_answer, 0.0);
        assert_eq!(trace.components.submitted_ok, 0.0);
        assert!(!trace.fatal_excluded);
        assert!(trace.total_reward < 4.0);
        assert_eq!(trace.trace_digest, trace.stable_digest()?);
        Ok(())
    }

    #[test]
    fn legal_reward_invalidates_harness_assisted_output() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let run_dir = temp.path();
        fs::write(run_dir.join("memo.md"), "# Memo\n\nHarness wrote this.\n")?;
        let task = task_spec();
        let artifact = artifact_from_file(
            "artifact.output.memo",
            ArtifactKind::GeneratedDeliverable,
            run_dir,
            run_dir.join("memo.md"),
            DataClassification::BenchmarkConfidential,
            Some(String::from("harness")),
        )?;
        let output_manifest = crate::build_output_artifact_manifest(
            "legal.reward.fixture",
            "v1",
            "run.harness",
            vec![artifact],
        );
        let run = run_record(&task, &output_manifest, RunTerminalState::Submitted);
        fs::write(
            run_dir.join("task_spec.json"),
            serde_json::to_vec_pretty(&task)?,
        )?;
        fs::write(
            run_dir.join("output_manifest.json"),
            serde_json::to_vec_pretty(&output_manifest)?,
        )?;
        fs::write(
            run_dir.join("run_record.json"),
            serde_json::to_vec_pretty(&run)?,
        )?;

        let trace = build_legal_reward_trace_from_run_dir(run_dir)?;

        assert!(trace.fatal_excluded);
        assert_eq!(trace.total_reward, 0.0);
        assert!(trace
            .exclusion_reasons
            .iter()
            .any(|reason| reason == "harness_assisted_output"));
        assert_eq!(trace.components.integrity_valid, 0.0);
        Ok(())
    }

    #[test]
    fn legal_reward_trace_replays_deterministically() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let run_dir = temp.path().join("run.good");
        fs::create_dir_all(&run_dir)?;
        let task = task_spec();
        let content = "# Memo\n\nModel wrote this answer.\n";
        fs::write(run_dir.join("memo.md"), content)?;
        let artifact = artifact_from_file(
            "artifact.output.memo",
            ArtifactKind::GeneratedDeliverable,
            &run_dir,
            run_dir.join("memo.md"),
            DataClassification::BenchmarkConfidential,
            Some(String::from("model")),
        )?;
        let output_manifest = crate::build_output_artifact_manifest(
            "legal.reward.fixture",
            "v1",
            "run.good",
            vec![artifact],
        );
        let mut run = run_record(&task, &output_manifest, RunTerminalState::Submitted);
        run.tool_calls = vec![write_tool_call("memo.md", content)];
        let score = score_report(&run, &output_manifest)?;
        fs::write(
            run_dir.join("task_spec.json"),
            serde_json::to_vec_pretty(&task)?,
        )?;
        fs::write(
            run_dir.join("output_manifest.json"),
            serde_json::to_vec_pretty(&output_manifest)?,
        )?;
        fs::write(
            run_dir.join("run_record.json"),
            serde_json::to_vec_pretty(&run)?,
        )?;
        fs::write(
            run_dir.join("score_report.json"),
            serde_json::to_vec_pretty(&score)?,
        )?;

        let out = temp.path().join("legal-reward-v1.jsonl");
        let config = LegalRewardTraceBuilderConfig::new(temp.path().to_path_buf(), out.clone());
        let first = build_legal_benchmark_reward_traces(&config)?;
        let first_jsonl = fs::read_to_string(&out)?;
        let second = build_legal_benchmark_reward_traces(&config)?;
        let second_jsonl = fs::read_to_string(&out)?;

        assert_eq!(first.manifest.dataset_hash, second.manifest.dataset_hash);
        assert_eq!(first_jsonl, second_jsonl);
        assert_eq!(first.traces[0].trace_digest, second.traces[0].trace_digest);
        assert_eq!(first.traces[0].components.wrote_required_file, 1.0);
        assert_eq!(first.traces[0].components.integrity_valid, 1.0);
        Ok(())
    }

    fn task_spec() -> BenchmarkTaskSpec {
        serde_json::from_value(json!({
            "schema_version": 1,
            "task_id": "legal.reward.fixture",
            "task_version": "v1",
            "domain": "legal",
            "practice_area": "contracts",
            "workflow": "review",
            "title": "Reward fixture",
            "instructions": "Write memo.md.",
            "work_type": "review",
            "tags": ["reward_trace"],
            "source_artifacts": [],
            "deliverables": [{
                "deliverable_id": "memo",
                "deliverable_kind": "markdown",
                "required_path": "memo.md",
                "description": "Memo",
                "required": true
            }],
            "criteria": [{
                "criterion_id": "criterion.memo",
                "criterion_kind": "deliverable_validation",
                "description": "The memo exists.",
                "weight_bps": 10000,
                "deliverable_ids": ["memo"]
            }],
            "judge_policy": {
                "mode": "deterministic",
                "provider": "mock",
                "model": "mock",
                "prompt_template_id": "judge",
                "prompt_template_hash": "hash",
                "all_pass_required": true,
                "sample_count": 1
            },
            "tool_policy": {
                "allowed_tools": ["write"],
                "network_allowed": false,
                "source_artifacts_read_only": true,
                "max_turns": 2,
                "max_wall_time_seconds": 60
            }
        }))
        .expect("valid task fixture")
    }

    fn run_record(
        task: &BenchmarkTaskSpec,
        output_manifest: &ArtifactManifest,
        terminal_state: RunTerminalState,
    ) -> RunRecord {
        RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run.reward.fixture"),
            task_id: task.task_id.clone(),
            task_version: task.task_version.clone(),
            input_artifact_manifest_hash: String::from(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            run_config_hash: String::from(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            output_artifact_manifest_hash: artifact_manifest_digest(output_manifest)
                .expect("manifest digest"),
            terminal_state,
            transcript: vec![TranscriptEvent {
                event_index: 0,
                event_kind: TranscriptEventKind::Assistant,
                role: Some(String::from("assistant")),
                content: Some(String::from("done")),
                payload: None,
                timestamp_ms: 0,
            }],
            tool_calls: Vec::new(),
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 10,
                output_tokens: 10,
                wall_time_ms: 10,
                estimated_cost_micro_usd: 0,
            },
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            metadata: Metadata::new(),
        }
    }

    fn score_report(
        run: &RunRecord,
        output_manifest: &ArtifactManifest,
    ) -> Result<ScoreReport, Box<dyn std::error::Error>> {
        Ok(ScoreReport {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            score_report_id: String::from("score.reward.fixture"),
            run_id: run.run_id.clone(),
            task_id: run.task_id.clone(),
            task_version: run.task_version.clone(),
            run_record_hash: run_record_digest(run)?,
            output_artifact_manifest_hash: artifact_manifest_digest(output_manifest)?,
            all_pass: true,
            criterion_pass_rate_bps: 10_000,
            criterion_results: vec![CriterionResult {
                criterion_id: String::from("criterion.memo"),
                passed: true,
                verdict: CriterionVerdict::Pass,
                reasoning: String::from("passed"),
                evidence_refs: vec![String::from("memo")],
                judge_model: String::from("deterministic"),
                judge_prompt_hash: String::from("hash"),
                raw_response_hash: String::from("raw"),
                confidence_bps: Some(10_000),
                judge_latency_ms: Some(0),
                judge_cost_micro_usd: Some(0),
            }],
            metrics: run.metrics.clone(),
            document_coverage_bps: 10_000,
            failure_diagnostics: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            failure_comparisons: Vec::new(),
            metadata: Metadata::new(),
        })
    }

    fn write_tool_call(relative_path: &str, content: &str) -> crate::ToolCallRecord {
        let after_hash = sha256_hex(content.as_bytes());
        crate::ToolCallRecord {
            tool_call_id: String::from("call.write.memo"),
            tool_name: String::from("write"),
            call_event_index: 1,
            result_event_index: Some(2),
            input: json!({
                "tool": "write",
                "input": {
                    "root": "output",
                    "relative_path": relative_path,
                    "content": content,
                    "overwrite": true
                }
            }),
            output: Some(json!({
                "tool": "write",
                "output": {
                    "relative_path": relative_path,
                    "bytes_written": content.len(),
                    "after_hash": after_hash
                }
            })),
            error_kind: None,
            elapsed_ms: 1,
        }
    }
}
