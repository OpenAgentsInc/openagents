//! Failed-run capture for legal benchmark training and audit.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ArtifactManifest, ArtifactManifestRole, BenchmarkTaskSpec, LegalAttemptedFileWrite,
    LegalBadRunExample, LegalBenchmarkAnswerIntegrityReport, LegalBenchmarkRunReceipt,
    LegalBenchmarkVisibility, LegalCapturedAnswerFile, LegalContentDigest, LegalFailureClass,
    LegalIntegrityReceipt, LegalRequiredFileStatus, LegalSchemaError, LegalToolCall, RunRecord,
    RunTerminalState, ScoreReport, ToolCallRecord, TranscriptEventKind,
    build_output_artifact_manifest, run_record_digest,
};

#[derive(Clone, Debug)]
pub struct FailedTrajectoryCaptureInput<'a> {
    pub benchmark_visibility: LegalBenchmarkVisibility,
    pub allow_private_training: bool,
    pub hidden_answer_labels_present: bool,
    pub scorer_secret_present: bool,
    pub task_spec: &'a BenchmarkTaskSpec,
    pub run_record: &'a RunRecord,
    pub output_manifest: &'a ArtifactManifest,
    pub score_report: Option<&'a ScoreReport>,
    pub answer_integrity: Option<&'a LegalBenchmarkAnswerIntegrityReport>,
    pub output_root: &'a Path,
    pub raw_malformed_text: Option<String>,
    pub suggested_correction: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FailedTrajectoryRunDirInspection {
    pub bad_run: LegalBadRunExample,
    pub source_paths: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Error)]
pub enum FailedTrajectoryCaptureError {
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
    #[error("missing required run-dir file {0}")]
    MissingRunDirFile(PathBuf),
    #[error("invalid output-relative path `{0}`")]
    InvalidRelativePath(String),
    #[error("failed to digest run record: {0}")]
    Digest(#[from] serde_json::Error),
    #[error("schema error: {0}")]
    Schema(#[from] LegalSchemaError),
}

pub fn capture_failed_trajectory(
    input: FailedTrajectoryCaptureInput<'_>,
) -> Result<LegalBadRunExample, FailedTrajectoryCaptureError> {
    let required_file_paths = input
        .task_spec
        .deliverables
        .iter()
        .filter(|deliverable| deliverable.required)
        .map(|deliverable| deliverable.required_path.clone())
        .collect::<Vec<_>>();
    let required_file_set = required_file_paths.iter().cloned().collect::<BTreeSet<_>>();
    let attempted_file_writes = attempted_output_writes(&input.run_record.tool_calls)?;
    let required_files = required_file_paths
        .iter()
        .map(|relative_path| required_file_status(input.output_root, relative_path))
        .collect::<Result<Vec<_>, _>>()?;
    let answer_files = captured_answer_files(
        input.output_root,
        &required_file_set,
        input.output_manifest,
        &attempted_file_writes,
    )?;
    let full_prompt = transcript_prompt(input.run_record);
    let full_model_response = transcript_model_response(input.run_record);
    let action_sequence = transcript_action_sequence(input.run_record);
    let stop_reason = latest_stop_reason(input.run_record)
        .unwrap_or_else(|| format!("{:?}", input.run_record.terminal_state));
    let scorer_feedback = input
        .score_report
        .map(score_feedback)
        .filter(|value| !value.is_empty());
    let answer_integrity = input
        .answer_integrity
        .cloned()
        .unwrap_or_else(LegalBenchmarkAnswerIntegrityReport::default);
    let integrity = LegalIntegrityReceipt::from_answer_integrity(&answer_integrity);
    let failure_class = classify_failure(
        input.run_record,
        input.score_report,
        &answer_integrity,
        &required_files,
        &attempted_file_writes,
        input.raw_malformed_text.as_deref(),
    );
    let (training_eligible, training_eligibility_reasons) = training_eligibility(
        input.benchmark_visibility,
        input.allow_private_training,
        input.hidden_answer_labels_present,
        input.scorer_secret_present,
        failure_class,
        &full_prompt,
        &full_model_response,
        &required_files,
    );

    Ok(LegalBadRunExample {
        schema_version: crate::LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
        example_id: format!("bad_run.{}", input.run_record.run_id),
        run_receipt_hash: LegalContentDigest::sha256(run_record_digest(input.run_record)?),
        full_prompt,
        full_model_response,
        tool_call_transcript: input
            .run_record
            .tool_calls
            .iter()
            .map(LegalToolCall::from_tool_call_record)
            .collect::<Result<Vec<_>, _>>()?,
        attempted_file_writes,
        required_file_paths,
        required_files,
        answer_files,
        action_sequence,
        stop_reason,
        score_bps: input
            .score_report
            .map(|score| score.criterion_pass_rate_bps),
        scorer_feedback,
        integrity,
        failure_class,
        suggested_correction: input.suggested_correction,
        training_eligible,
        sft_eligible: false,
        training_eligibility_reasons,
        raw_malformed_text: input.raw_malformed_text,
    })
}

pub fn inspect_failed_trajectory_run_dir(
    run_dir: impl AsRef<Path>,
) -> Result<FailedTrajectoryRunDirInspection, FailedTrajectoryCaptureError> {
    let run_dir = run_dir.as_ref();
    let run_record_path = run_dir.join("run_record.json");
    let task_spec_path = run_dir.join("task_spec.json");
    let output_manifest_path = run_dir.join("output_artifact_manifest.json");
    let score_report_path = run_dir.join("score_report.json");
    let run_receipt_path = run_dir.join("run_receipt.json");
    let malformed_path = run_dir.join("malformed_tool_call.txt");

    let run_record = read_required_json::<RunRecord>(&run_record_path)?;
    let task_spec = read_required_json::<BenchmarkTaskSpec>(&task_spec_path)?;
    let output_manifest = read_optional_json::<ArtifactManifest>(&output_manifest_path)?
        .unwrap_or_else(|| {
            let manifest = build_output_artifact_manifest(
                run_record.task_id.clone(),
                run_record.task_version.clone(),
                run_record.run_id.clone(),
                Vec::new(),
            );
            debug_assert_eq!(manifest.manifest_role, ArtifactManifestRole::Output);
            manifest
        });
    let score_report = read_optional_json::<ScoreReport>(&score_report_path)?;
    let answer_integrity = read_optional_json::<LegalBenchmarkRunReceipt>(&run_receipt_path)?
        .map(|receipt| receipt.answer_integrity);
    let raw_malformed_text = if malformed_path.exists() {
        Some(read_to_string(&malformed_path)?)
    } else {
        None
    };
    let output_root = run_dir.join("output");
    let bad_run = capture_failed_trajectory(FailedTrajectoryCaptureInput {
        benchmark_visibility: LegalBenchmarkVisibility::Public,
        allow_private_training: false,
        hidden_answer_labels_present: false,
        scorer_secret_present: false,
        task_spec: &task_spec,
        run_record: &run_record,
        output_manifest: &output_manifest,
        score_report: score_report.as_ref(),
        answer_integrity: answer_integrity.as_ref(),
        output_root: &output_root,
        raw_malformed_text,
        suggested_correction: None,
    })?;
    let source_paths = BTreeMap::from([
        (String::from("run_record"), run_record_path),
        (String::from("task_spec"), task_spec_path),
        (String::from("output_manifest"), output_manifest_path),
        (String::from("score_report"), score_report_path),
        (String::from("run_receipt"), run_receipt_path),
    ]);
    Ok(FailedTrajectoryRunDirInspection {
        bad_run,
        source_paths,
    })
}

fn attempted_output_writes(
    tool_calls: &[ToolCallRecord],
) -> Result<Vec<LegalAttemptedFileWrite>, FailedTrajectoryCaptureError> {
    let mut writes = Vec::new();
    for call in tool_calls {
        if !matches!(call.tool_name.as_str(), "write" | "edit") {
            continue;
        }
        let Some(input) = tool_payload(&call.input) else {
            continue;
        };
        if input.get("root").and_then(Value::as_str) != Some("output") {
            continue;
        }
        let Some(relative_path) = input.get("relative_path").and_then(Value::as_str) else {
            continue;
        };
        validate_relative_path(relative_path)?;
        let content = input.get("content").and_then(Value::as_str);
        let output = call.output.as_ref().and_then(tool_output_payload);
        writes.push(LegalAttemptedFileWrite {
            relative_path: relative_path.to_string(),
            tool_call_id: call.tool_call_id.clone(),
            tool_name: call.tool_name.clone(),
            content_hash: content
                .map(|value| LegalContentDigest::sha256(sha256_hex(value.as_bytes()))),
            byte_len: content
                .map(|value| u64::try_from(value.len()).unwrap_or(u64::MAX))
                .or_else(|| {
                    output
                        .and_then(|value| value.get("bytes_written"))
                        .and_then(Value::as_u64)
                }),
        });
    }
    Ok(writes)
}

fn required_file_status(
    output_root: &Path,
    relative_path: &str,
) -> Result<LegalRequiredFileStatus, FailedTrajectoryCaptureError> {
    validate_relative_path(relative_path)?;
    let path = output_root.join(relative_path);
    if !path.exists() {
        return Ok(LegalRequiredFileStatus {
            relative_path: relative_path.to_string(),
            existed: false,
            content_hash: None,
            byte_len: None,
        });
    }
    let bytes = fs::read(&path).map_err(|source| FailedTrajectoryCaptureError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(LegalRequiredFileStatus {
        relative_path: relative_path.to_string(),
        existed: true,
        content_hash: Some(LegalContentDigest::sha256(sha256_hex(&bytes))),
        byte_len: Some(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
    })
}

fn captured_answer_files(
    output_root: &Path,
    required_file_set: &BTreeSet<String>,
    output_manifest: &ArtifactManifest,
    attempted_file_writes: &[LegalAttemptedFileWrite],
) -> Result<Vec<LegalCapturedAnswerFile>, FailedTrajectoryCaptureError> {
    let mut paths = required_file_set.clone();
    for artifact in &output_manifest.artifacts {
        paths.insert(artifact.relative_path.clone());
    }
    for write in attempted_file_writes {
        paths.insert(write.relative_path.clone());
    }
    let mut answer_files = Vec::new();
    for relative_path in paths {
        validate_relative_path(relative_path.as_str())?;
        let path = output_root.join(relative_path.as_str());
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|source| FailedTrajectoryCaptureError::Io {
            path: path.clone(),
            source,
        })?;
        answer_files.push(LegalCapturedAnswerFile {
            relative_path,
            content: String::from_utf8_lossy(&bytes).into_owned(),
            content_hash: LegalContentDigest::sha256(sha256_hex(&bytes)),
            byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(answer_files)
}

fn classify_failure(
    run_record: &RunRecord,
    score_report: Option<&ScoreReport>,
    answer_integrity: &LegalBenchmarkAnswerIntegrityReport,
    required_files: &[LegalRequiredFileStatus],
    attempted_file_writes: &[LegalAttemptedFileWrite],
    raw_malformed_text: Option<&str>,
) -> LegalFailureClass {
    if let Some(raw) = raw_malformed_text {
        if serde_json::from_str::<Value>(raw).is_err() {
            return LegalFailureClass::InvalidJson;
        }
        return LegalFailureClass::ToolCallMalformed;
    }
    if answer_integrity.invalid_reasons.iter().any(|reason| {
        reason.contains("changed_during_scoring")
            || reason.contains("no_model_write")
            || reason.contains("actual_hash_does_not_match")
    }) {
        return LegalFailureClass::HarnessIntegrityFailure;
    }
    if matches!(
        run_record.terminal_state,
        RunTerminalState::MaxTurns | RunTerminalState::MaxTokens
    ) {
        return LegalFailureClass::Timeout;
    }
    if latest_stop_reason(run_record)
        .as_deref()
        .is_some_and(|reason| reason.contains("refusal") || reason.contains("safety"))
    {
        return LegalFailureClass::ModelRefusal;
    }
    let missing_required = required_files.iter().any(|file| !file.existed);
    if missing_required && attempted_file_writes.is_empty() {
        return LegalFailureClass::DidNotWriteRequiredFile;
    }
    if missing_required && !attempted_file_writes.is_empty() {
        return LegalFailureClass::WroteWrongPath;
    }
    if required_files.iter().any(|file| file.byte_len == Some(0)) {
        return LegalFailureClass::WroteEmptyFile;
    }
    if required_files
        .iter()
        .filter_map(|file| file.byte_len)
        .any(|len| len > 200_000)
    {
        return LegalFailureClass::WroteTooLong;
    }
    if required_files
        .iter()
        .filter_map(|file| file.byte_len)
        .any(|len| len > 0 && len < 40)
    {
        return LegalFailureClass::WroteTooShort;
    }
    if let Some(score_report) = score_report {
        let feedback = score_feedback(score_report).to_lowercase();
        if feedback.contains("scorer unavailable") || feedback.contains("judge unavailable") {
            return LegalFailureClass::ScorerUnavailable;
        }
        if feedback.contains("source") || feedback.contains("citation missing") {
            return LegalFailureClass::FailedToUseSources;
        }
        if feedback.contains("hallucinated") || feedback.contains("fake citation") {
            return LegalFailureClass::HallucinatedCitations;
        }
    }
    if matches!(run_record.terminal_state, RunTerminalState::NoToolCalls) {
        return LegalFailureClass::DidNotSubmit;
    }
    LegalFailureClass::Other
}

fn training_eligibility(
    visibility: LegalBenchmarkVisibility,
    allow_private_training: bool,
    hidden_answer_labels_present: bool,
    scorer_secret_present: bool,
    failure_class: LegalFailureClass,
    full_prompt: &str,
    full_model_response: &str,
    required_files: &[LegalRequiredFileStatus],
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    match visibility {
        LegalBenchmarkVisibility::Public
        | LegalBenchmarkVisibility::Synthetic
        | LegalBenchmarkVisibility::Internal => {}
        LegalBenchmarkVisibility::Private if allow_private_training => {}
        LegalBenchmarkVisibility::Private => {
            reasons.push(String::from(
                "private benchmark failure is audit-only by default",
            ));
        }
        LegalBenchmarkVisibility::Hidden => {
            reasons.push(String::from("hidden benchmark failure is audit-only"));
        }
    }
    if hidden_answer_labels_present {
        reasons.push(String::from("hidden answer labels are present"));
    }
    if scorer_secret_present {
        reasons.push(String::from("scorer secret is present"));
    }
    if matches!(
        failure_class,
        LegalFailureClass::HarnessIntegrityFailure | LegalFailureClass::ScorerUnavailable
    ) {
        reasons.push(String::from(
            "failure did not come from usable model behavior",
        ));
    }
    if full_prompt.trim().is_empty() || full_model_response.trim().is_empty() {
        reasons.push(String::from("prompt or model response is incomplete"));
    }
    if required_files.is_empty() {
        reasons.push(String::from("required file list is missing"));
    }
    (reasons.is_empty(), reasons)
}

fn transcript_prompt(run_record: &RunRecord) -> String {
    run_record
        .transcript
        .iter()
        .filter(|event| {
            matches!(
                event.event_kind,
                TranscriptEventKind::System | TranscriptEventKind::User
            )
        })
        .filter_map(|event| event.content.as_deref())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn transcript_model_response(run_record: &RunRecord) -> String {
    run_record
        .transcript
        .iter()
        .filter(|event| matches!(event.event_kind, TranscriptEventKind::Assistant))
        .filter_map(|event| {
            event
                .content
                .clone()
                .or_else(|| event.payload.as_ref().map(Value::to_string))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn transcript_action_sequence(run_record: &RunRecord) -> Vec<String> {
    run_record
        .transcript
        .iter()
        .map(|event| {
            let kind = format!("{:?}", event.event_kind);
            let tool = event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("tool_name"))
                .and_then(Value::as_str)
                .map(|tool_name| format!(":{tool_name}"))
                .unwrap_or_default();
            format!("{}:{kind}{tool}", event.event_index)
        })
        .collect()
}

fn latest_stop_reason(run_record: &RunRecord) -> Option<String> {
    run_record
        .transcript
        .iter()
        .rev()
        .filter(|event| matches!(event.event_kind, TranscriptEventKind::Assistant))
        .find_map(|event| {
            event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("stop_reason"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn score_feedback(score_report: &ScoreReport) -> String {
    let mut parts = score_report.failure_diagnostics.clone();
    parts.extend(
        score_report
            .criterion_results
            .iter()
            .filter(|criterion| !criterion.passed)
            .map(|criterion| criterion.reasoning.clone()),
    );
    parts.join("\n")
}

fn tool_payload(value: &Value) -> Option<&Value> {
    value.get("input").unwrap_or(value).as_object()?;
    Some(value.get("input").unwrap_or(value))
}

fn tool_output_payload(value: &Value) -> Option<&Value> {
    value.get("output").unwrap_or(value).as_object()?;
    Some(value.get("output").unwrap_or(value))
}

fn validate_relative_path(relative_path: &str) -> Result<(), FailedTrajectoryCaptureError> {
    let path = Path::new(relative_path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err(FailedTrajectoryCaptureError::InvalidRelativePath(
            relative_path.to_string(),
        ));
    }
    Ok(())
}

fn read_required_json<T>(path: &Path) -> Result<T, FailedTrajectoryCaptureError>
where
    T: serde::de::DeserializeOwned,
{
    if !path.exists() {
        return Err(FailedTrajectoryCaptureError::MissingRunDirFile(
            path.to_path_buf(),
        ));
    }
    read_optional_json(path).map(|value| value.expect("exists checked above"))
}

fn read_optional_json<T>(path: &Path) -> Result<Option<T>, FailedTrajectoryCaptureError>
where
    T: serde::de::DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|source| FailedTrajectoryCaptureError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| FailedTrajectoryCaptureError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn read_to_string(path: &Path) -> Result<String, FailedTrajectoryCaptureError> {
    fs::read_to_string(path).map_err(|source| FailedTrajectoryCaptureError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CriterionKind, DeliverableKind, DeliverableSpec, JudgeMode, JudgePolicy, Metadata,
        RunMetrics, SourceCompatibility, ToolPolicy, TranscriptEvent, artifact_from_file,
    };
    use std::fs;

    fn task_spec() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.failed.capture"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            title: String::from("Capture failed run"),
            instructions: String::from("Write memo.md."),
            work_type: String::from("review"),
            tags: vec![String::from("failed_trajectory_capture")],
            source_artifacts: Vec::new(),
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Memo"),
                required: true,
            }],
            criteria: vec![crate::CriterionSpec {
                criterion_id: String::from("criterion.memo"),
                criterion_kind: CriterionKind::DeliverableValidation,
                description: String::from("memo exists"),
                weight_bps: Some(10_000),
                deliverable_ids: vec![String::from("memo")],
                source_artifact_ids: Vec::new(),
            }],
            judge_policy: JudgePolicy {
                mode: JudgeMode::Deterministic,
                provider: String::from("mock"),
                model: String::from("mock"),
                prompt_template_id: String::from("judge"),
                prompt_template_hash: String::from("hash"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("write")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 2,
                max_wall_time_seconds: 60,
            },
            source_compatibility: Some(SourceCompatibility {
                upstream_suite: String::from("harvey_labs"),
                upstream_commit: String::from("fixture"),
                upstream_task_path: String::from("tasks/mock"),
                upstream_fields: Metadata::new(),
            }),
            metadata: Metadata::new(),
        }
    }

    fn run_record(tool_calls: Vec<ToolCallRecord>, terminal_state: RunTerminalState) -> RunRecord {
        RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run.failed.capture"),
            task_id: String::from("legal.failed.capture"),
            task_version: String::from("v1"),
            input_artifact_manifest_hash: String::from(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            run_config_hash: String::from(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            output_artifact_manifest_hash: String::from(
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            ),
            terminal_state,
            transcript: vec![
                TranscriptEvent {
                    event_index: 0,
                    event_kind: TranscriptEventKind::System,
                    role: Some(String::from("system")),
                    content: Some(String::from("Use the legal benchmark tools.")),
                    payload: None,
                    timestamp_ms: 1,
                },
                TranscriptEvent {
                    event_index: 1,
                    event_kind: TranscriptEventKind::User,
                    role: Some(String::from("user")),
                    content: Some(String::from("Write memo.md.")),
                    payload: None,
                    timestamp_ms: 1,
                },
                TranscriptEvent {
                    event_index: 2,
                    event_kind: TranscriptEventKind::Assistant,
                    role: Some(String::from("assistant")),
                    content: Some(String::from("I will write the memo.")),
                    payload: Some(serde_json::json!({"stop_reason": "stop"})),
                    timestamp_ms: 2,
                },
            ],
            tool_calls,
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 10,
                output_tokens: 5,
                wall_time_ms: 100,
                estimated_cost_micro_usd: 1,
            },
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            metadata: Metadata::new(),
        }
    }

    fn write_call(relative_path: &str, content: &str) -> ToolCallRecord {
        ToolCallRecord {
            tool_call_id: format!("call.write.{}", relative_path.replace('/', "_")),
            tool_name: String::from("write"),
            call_event_index: 3,
            result_event_index: Some(4),
            input: serde_json::json!({
                "tool": "write",
                "input": {
                    "root": "output",
                    "relative_path": relative_path,
                    "content": content,
                    "overwrite": true
                }
            }),
            output: Some(serde_json::json!({
                "tool": "write",
                "output": {
                    "relative_path": relative_path,
                    "bytes_written": content.len(),
                    "after_hash": sha256_hex(content.as_bytes())
                }
            })),
            error_kind: None,
            elapsed_ms: 1,
        }
    }

    fn empty_manifest() -> ArtifactManifest {
        build_output_artifact_manifest(
            "legal.failed.capture",
            "v1",
            "run.failed.capture",
            Vec::new(),
        )
    }

    #[test]
    fn failed_trajectory_capture_missing_required_file_creates_bad_example() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let run_record = run_record(Vec::new(), RunTerminalState::NoToolCalls);
        let bad_run = capture_failed_trajectory(FailedTrajectoryCaptureInput {
            benchmark_visibility: LegalBenchmarkVisibility::Public,
            allow_private_training: false,
            hidden_answer_labels_present: false,
            scorer_secret_present: false,
            task_spec: &task_spec(),
            run_record: &run_record,
            output_manifest: &empty_manifest(),
            score_report: None,
            answer_integrity: None,
            output_root: &output_root,
            raw_malformed_text: None,
            suggested_correction: Some(String::from("Write memo.md before submitting.")),
        })
        .expect("bad run");

        assert_eq!(
            bad_run.failure_class,
            LegalFailureClass::DidNotWriteRequiredFile
        );
        assert_eq!(bad_run.required_files[0].relative_path, "memo.md");
        assert!(!bad_run.required_files[0].existed);
        assert!(!bad_run.sft_eligible);
        assert!(bad_run.training_eligible);
    }

    #[test]
    fn failed_trajectory_capture_wrong_path_captures_wrong_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let wrong_content = "# Wrong\n\nWrote the wrong file.\n";
        fs::write(output_root.join("wrong.md"), wrong_content).expect("wrong file");
        let artifact = artifact_from_file(
            "artifact.wrong",
            crate::ArtifactKind::GeneratedDeliverable,
            &output_root,
            output_root.join("wrong.md"),
            crate::DataClassification::BenchmarkConfidential,
            Some(String::from("model")),
        )
        .expect("artifact");
        let manifest = build_output_artifact_manifest(
            "legal.failed.capture",
            "v1",
            "run.failed.capture",
            vec![artifact],
        );
        let run_record = run_record(
            vec![write_call("wrong.md", wrong_content)],
            RunTerminalState::Submitted,
        );
        let bad_run = capture_failed_trajectory(FailedTrajectoryCaptureInput {
            benchmark_visibility: LegalBenchmarkVisibility::Public,
            allow_private_training: false,
            hidden_answer_labels_present: false,
            scorer_secret_present: false,
            task_spec: &task_spec(),
            run_record: &run_record,
            output_manifest: &manifest,
            score_report: None,
            answer_integrity: None,
            output_root: &output_root,
            raw_malformed_text: None,
            suggested_correction: None,
        })
        .expect("bad run");

        assert_eq!(bad_run.failure_class, LegalFailureClass::WroteWrongPath);
        assert_eq!(bad_run.attempted_file_writes[0].relative_path, "wrong.md");
        assert_eq!(bad_run.answer_files[0].relative_path, "wrong.md");
        assert_eq!(bad_run.answer_files[0].content, wrong_content);
    }

    #[test]
    fn failed_trajectory_capture_malformed_tool_call_captures_raw_text() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let run_record = run_record(Vec::new(), RunTerminalState::PolicyFailure);
        let bad_run = capture_failed_trajectory(FailedTrajectoryCaptureInput {
            benchmark_visibility: LegalBenchmarkVisibility::Public,
            allow_private_training: false,
            hidden_answer_labels_present: false,
            scorer_secret_present: false,
            task_spec: &task_spec(),
            run_record: &run_record,
            output_manifest: &empty_manifest(),
            score_report: None,
            answer_integrity: None,
            output_root: &output_root,
            raw_malformed_text: Some(String::from("{\"tool\":\"write\", bad")),
            suggested_correction: None,
        })
        .expect("bad run");

        assert_eq!(bad_run.failure_class, LegalFailureClass::InvalidJson);
        assert_eq!(
            bad_run.raw_malformed_text.as_deref(),
            Some("{\"tool\":\"write\", bad")
        );
    }

    #[test]
    fn failed_trajectory_capture_hidden_runs_are_audit_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let run_record = run_record(Vec::new(), RunTerminalState::NoToolCalls);
        let bad_run = capture_failed_trajectory(FailedTrajectoryCaptureInput {
            benchmark_visibility: LegalBenchmarkVisibility::Hidden,
            allow_private_training: false,
            hidden_answer_labels_present: true,
            scorer_secret_present: false,
            task_spec: &task_spec(),
            run_record: &run_record,
            output_manifest: &empty_manifest(),
            score_report: None,
            answer_integrity: None,
            output_root: &output_root,
            raw_malformed_text: None,
            suggested_correction: None,
        })
        .expect("bad run");

        assert!(!bad_run.training_eligible);
        assert!(
            bad_run
                .training_eligibility_reasons
                .iter()
                .any(|reason| reason.contains("hidden"))
        );
    }
}
