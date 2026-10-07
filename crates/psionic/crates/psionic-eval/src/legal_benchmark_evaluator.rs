//! Criterion-scoped evaluator and judge adapter for legal benchmark runs.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    ArtifactManifest, ArtifactManifestError, BenchmarkIntegrityError, BenchmarkIntegrityGuard,
    BenchmarkTaskSpec, CriterionResult, CriterionSpec, CriterionVerdict, DeliverableKind,
    DeliverableSpec, JudgePolicy, LegalBenchmarkAnswerIntegrityReport, Metadata, RunRecord,
    ScoreReport, SourceArtifact, artifact_from_file, artifact_manifest_digest,
    classify_criterion_failures, document_coverage_bps_from_snapshot, fallback_coverage_snapshot,
    run_record_digest, score_report_digest, stable_json_digest,
};

pub const LEGAL_BENCHMARK_EVALUATOR_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvaluationInput {
    pub task_spec: BenchmarkTaskSpec,
    pub run_record: RunRecord,
    pub output_artifact_manifest: ArtifactManifest,
    pub output_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkPrecheck {
    pub precheck_id: String,
    pub passed: bool,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkJudgeRequest {
    pub schema_version: u16,
    pub criterion: CriterionSpec,
    pub prompt_template_id: String,
    pub prompt_template_hash: String,
    pub judge_model: String,
    pub output_text: String,
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkJudgeResponse {
    pub verdict: CriterionVerdict,
    pub reasoning: String,
    pub confidence_bps: Option<u16>,
    pub raw_response: Value,
    pub latency_ms: u64,
    pub cost_micro_usd: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvaluationResult {
    pub score_report: ScoreReport,
    pub prechecks: Vec<LegalBenchmarkPrecheck>,
    pub score_report_hash: String,
}

#[derive(Debug, Error)]
pub enum LegalBenchmarkEvaluationError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("artifact manifest error: {0}")]
    ArtifactManifest(#[from] ArtifactManifestError),
    #[error("benchmark integrity error: {0}")]
    Integrity(#[from] BenchmarkIntegrityError),
    #[error("judge error: {0}")]
    Judge(String),
}

pub trait LegalBenchmarkJudgeAdapter {
    fn judge(
        &mut self,
        request: &LegalBenchmarkJudgeRequest,
    ) -> Result<LegalBenchmarkJudgeResponse, LegalBenchmarkEvaluationError>;
}

#[derive(Clone, Debug)]
pub struct MockLegalBenchmarkJudge {
    verdict: CriterionVerdict,
    reasoning: String,
    confidence_bps: Option<u16>,
}

impl MockLegalBenchmarkJudge {
    pub fn pass() -> Self {
        Self {
            verdict: CriterionVerdict::Pass,
            reasoning: String::from("mock judge passed the criterion"),
            confidence_bps: Some(10_000),
        }
    }

    pub fn fail(reasoning: impl Into<String>) -> Self {
        Self {
            verdict: CriterionVerdict::Fail,
            reasoning: reasoning.into(),
            confidence_bps: Some(10_000),
        }
    }
}

impl LegalBenchmarkJudgeAdapter for MockLegalBenchmarkJudge {
    fn judge(
        &mut self,
        request: &LegalBenchmarkJudgeRequest,
    ) -> Result<LegalBenchmarkJudgeResponse, LegalBenchmarkEvaluationError> {
        Ok(LegalBenchmarkJudgeResponse {
            verdict: self.verdict,
            reasoning: self.reasoning.clone(),
            confidence_bps: self.confidence_bps,
            raw_response: json!({
                "mock": true,
                "criterion_id": request.criterion.criterion_id,
                "verdict": self.verdict,
                "reasoning": self.reasoning,
            }),
            latency_ms: 1,
            cost_micro_usd: 0,
        })
    }
}

pub fn evaluate_legal_benchmark_run<J>(
    input: &LegalBenchmarkEvaluationInput,
    judge: &mut J,
) -> Result<LegalBenchmarkEvaluationResult, LegalBenchmarkEvaluationError>
where
    J: LegalBenchmarkJudgeAdapter,
{
    let integrity_guard = BenchmarkIntegrityGuard::before_scoring(
        &input.task_spec,
        &input.output_artifact_manifest,
        &input.output_root,
        &input.run_record.tool_calls,
    )?;
    let initial_answer_integrity = integrity_guard.finalize_after_scoring()?;
    let mut prechecks = run_prechecks(input)?;
    prechecks.push(answer_integrity_precheck(&initial_answer_integrity));
    let mut failure_diagnostics = prechecks
        .iter()
        .filter(|precheck| !precheck.passed)
        .map(|precheck| format!("{}: {}", precheck.precheck_id, precheck.detail))
        .collect::<Vec<_>>();
    let mut criterion_results = Vec::new();
    let prompt_template = default_judge_prompt_template(&input.task_spec.judge_policy);
    let prompt_template_hash = stable_json_digest(
        "psionic.legal_benchmark.judge_prompt_template.v1",
        &prompt_template,
    )?;
    let mut total_judge_cost = 0_u64;
    let mut total_judge_latency = 0_u64;

    for criterion in &input.task_spec.criteria {
        if let Some(result) =
            deterministic_criterion_result(criterion, input, &failure_diagnostics)?
        {
            criterion_results.push(result);
            continue;
        }
        let output_text = criterion_output_text(criterion, input)?;
        let judge_request = LegalBenchmarkJudgeRequest {
            schema_version: LEGAL_BENCHMARK_EVALUATOR_SCHEMA_VERSION,
            criterion: criterion.clone(),
            prompt_template_id: input.task_spec.judge_policy.prompt_template_id.clone(),
            prompt_template_hash: prompt_template_hash.clone(),
            judge_model: input.task_spec.judge_policy.model.clone(),
            output_text,
            evidence_refs: criterion_evidence_refs(criterion, input),
            metadata: Metadata::new(),
        };
        let judge_response = judge.judge(&judge_request)?;
        total_judge_cost = total_judge_cost.saturating_add(judge_response.cost_micro_usd);
        total_judge_latency = total_judge_latency.saturating_add(judge_response.latency_ms);
        criterion_results.push(criterion_result_from_judge(
            criterion,
            &judge_request,
            judge_response,
        )?);
    }

    let answer_integrity = integrity_guard.finalize_after_scoring()?;
    if !answer_integrity.valid {
        failure_diagnostics.push(format!(
            "answer_integrity: {}",
            answer_integrity.invalid_reasons.join("; ")
        ));
    }

    let passed_count = criterion_results
        .iter()
        .filter(|result| result.passed)
        .count();
    let raw_criterion_pass_rate_bps = if criterion_results.is_empty() {
        0
    } else {
        u32::try_from((passed_count * 10_000) / criterion_results.len()).unwrap_or(0)
    };
    let criterion_pass_rate_bps = if answer_integrity.valid {
        raw_criterion_pass_rate_bps
    } else {
        0
    };
    let all_pass = criterion_results
        .iter()
        .all(|result| result.verdict == CriterionVerdict::Pass)
        && answer_integrity.valid;
    let mut metrics = input.run_record.metrics.clone();
    metrics.estimated_cost_micro_usd = metrics
        .estimated_cost_micro_usd
        .saturating_add(total_judge_cost);
    metrics.wall_time_ms = metrics.wall_time_ms.saturating_add(total_judge_latency);
    let output_manifest_hash = artifact_manifest_digest(&input.output_artifact_manifest)?;
    let run_record_hash = run_record_digest(&input.run_record)?;
    let coverage_snapshot = input
        .run_record
        .coverage_snapshot
        .clone()
        .unwrap_or_else(|| {
            fallback_coverage_snapshot(&input.task_spec, &input.output_artifact_manifest)
        });
    let failure_comparisons =
        classify_criterion_failures(&input.task_spec, &criterion_results, &coverage_snapshot);
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("precheck_count"),
        json!(u64::try_from(prechecks.len()).unwrap_or(u64::MAX)),
    );
    metadata.insert(
        String::from("judge_prompt_template_hash"),
        Value::String(prompt_template_hash),
    );
    metadata.insert(
        String::from("answer_integrity"),
        serde_json::to_value(&answer_integrity)?,
    );
    let score_report = ScoreReport {
        schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
        score_report_id: format!(
            "score.{}.{}",
            input.run_record.run_id, input.task_spec.judge_policy.prompt_template_id
        ),
        run_id: input.run_record.run_id.clone(),
        task_id: input.task_spec.task_id.clone(),
        task_version: input.task_spec.task_version.clone(),
        run_record_hash,
        output_artifact_manifest_hash: output_manifest_hash,
        all_pass,
        criterion_pass_rate_bps,
        criterion_results,
        metrics,
        document_coverage_bps: document_coverage_bps_from_snapshot(
            &input.task_spec,
            &coverage_snapshot,
        ),
        failure_diagnostics,
        extraction_receipt_refs: input.run_record.extraction_receipt_refs.clone(),
        coverage_snapshot: Some(coverage_snapshot),
        failure_comparisons,
        metadata,
    };
    let score_report_hash = score_report_digest(&score_report)?;
    Ok(LegalBenchmarkEvaluationResult {
        score_report,
        prechecks,
        score_report_hash,
    })
}

fn run_prechecks(
    input: &LegalBenchmarkEvaluationInput,
) -> Result<Vec<LegalBenchmarkPrecheck>, LegalBenchmarkEvaluationError> {
    let mut prechecks = Vec::new();
    let expected_manifest_hash = artifact_manifest_digest(&input.output_artifact_manifest)?;
    prechecks.push(LegalBenchmarkPrecheck {
        precheck_id: String::from("output_manifest_hash"),
        passed: expected_manifest_hash == input.run_record.output_artifact_manifest_hash,
        detail: String::from("output manifest hash matches run record"),
        evidence_refs: vec![input.output_artifact_manifest.manifest_id.clone()],
    });
    for deliverable in &input.task_spec.deliverables {
        prechecks.push(check_deliverable(input, deliverable)?);
    }
    for artifact in &input.output_artifact_manifest.artifacts {
        prechecks.push(check_manifest_artifact(input, artifact)?);
    }
    Ok(prechecks)
}

fn answer_integrity_precheck(
    report: &LegalBenchmarkAnswerIntegrityReport,
) -> LegalBenchmarkPrecheck {
    LegalBenchmarkPrecheck {
        precheck_id: String::from("answer_integrity"),
        passed: report.valid,
        detail: if report.valid {
            String::from("answer file integrity is valid")
        } else {
            format!(
                "answer file integrity invalid: {}",
                report.invalid_reasons.join("; ")
            )
        },
        evidence_refs: report
            .answer_files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect(),
    }
}

fn check_deliverable(
    input: &LegalBenchmarkEvaluationInput,
    deliverable: &DeliverableSpec,
) -> Result<LegalBenchmarkPrecheck, LegalBenchmarkEvaluationError> {
    let path = input.output_root.join(&deliverable.required_path);
    let exists = path.is_file();
    let readable = exists && fs::read(&path).is_ok();
    let type_matches = exists && deliverable_type_matches(deliverable, &path);
    let passed = if deliverable.required {
        exists && readable && type_matches
    } else {
        !exists || (readable && type_matches)
    };
    Ok(LegalBenchmarkPrecheck {
        precheck_id: format!("deliverable.{}", deliverable.deliverable_id),
        passed,
        detail: format!(
            "exists={exists}; readable={readable}; type_matches={type_matches}; path={}",
            deliverable.required_path
        ),
        evidence_refs: vec![deliverable.required_path.clone()],
    })
}

fn check_manifest_artifact(
    input: &LegalBenchmarkEvaluationInput,
    artifact: &SourceArtifact,
) -> Result<LegalBenchmarkPrecheck, LegalBenchmarkEvaluationError> {
    let path = input.output_root.join(&artifact.relative_path);
    if !path.is_file() {
        return Ok(LegalBenchmarkPrecheck {
            precheck_id: format!("artifact.{}", artifact.artifact_id),
            passed: false,
            detail: format!("manifest artifact missing at {}", artifact.relative_path),
            evidence_refs: vec![artifact.artifact_id.clone()],
        });
    }
    let rebuilt = artifact_from_file(
        artifact.artifact_id.clone(),
        artifact.artifact_kind,
        &input.output_root,
        &path,
        artifact.data_classification,
        artifact.provenance.clone(),
    )?;
    let passed = rebuilt.sha256 == artifact.sha256
        && rebuilt.byte_size == artifact.byte_size
        && rebuilt.media_type == artifact.media_type;
    Ok(LegalBenchmarkPrecheck {
        precheck_id: format!("artifact.{}", artifact.artifact_id),
        passed,
        detail: format!("manifest artifact verifies at {}", artifact.relative_path),
        evidence_refs: vec![artifact.artifact_id.clone()],
    })
}

fn deterministic_criterion_result(
    criterion: &CriterionSpec,
    input: &LegalBenchmarkEvaluationInput,
    failure_diagnostics: &[String],
) -> Result<Option<CriterionResult>, LegalBenchmarkEvaluationError> {
    if criterion.deliverable_ids.is_empty() || failure_diagnostics.is_empty() {
        return Ok(None);
    }
    let deliverable_ids = criterion
        .deliverable_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let failed_deliverable = input
        .task_spec
        .deliverables
        .iter()
        .filter(|deliverable| deliverable_ids.contains(&deliverable.deliverable_id))
        .any(|deliverable| {
            failure_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains(&deliverable.deliverable_id))
        });
    if !failed_deliverable {
        return Ok(None);
    }
    Ok(Some(CriterionResult {
        criterion_id: criterion.criterion_id.clone(),
        passed: false,
        verdict: CriterionVerdict::Fail,
        reasoning: String::from("required deliverable failed deterministic precheck"),
        evidence_refs: criterion.deliverable_ids.clone(),
        judge_model: String::from("deterministic_precheck"),
        judge_prompt_hash: String::from("deterministic_precheck"),
        raw_response_hash: stable_json_digest(
            "psionic.legal_benchmark.deterministic_criterion.v1",
            criterion,
        )?,
        confidence_bps: Some(10_000),
        judge_latency_ms: Some(0),
        judge_cost_micro_usd: Some(0),
    }))
}

fn criterion_result_from_judge(
    criterion: &CriterionSpec,
    request: &LegalBenchmarkJudgeRequest,
    response: LegalBenchmarkJudgeResponse,
) -> Result<CriterionResult, LegalBenchmarkEvaluationError> {
    let raw_response_hash = stable_json_digest(
        "psionic.legal_benchmark.judge_raw_response.v1",
        &response.raw_response,
    )?;
    Ok(CriterionResult {
        criterion_id: criterion.criterion_id.clone(),
        passed: response.verdict == CriterionVerdict::Pass,
        verdict: response.verdict,
        reasoning: response.reasoning,
        evidence_refs: request.evidence_refs.clone(),
        judge_model: request.judge_model.clone(),
        judge_prompt_hash: request.prompt_template_hash.clone(),
        raw_response_hash,
        confidence_bps: response.confidence_bps,
        judge_latency_ms: Some(response.latency_ms),
        judge_cost_micro_usd: Some(response.cost_micro_usd),
    })
}

fn criterion_output_text(
    criterion: &CriterionSpec,
    input: &LegalBenchmarkEvaluationInput,
) -> Result<String, LegalBenchmarkEvaluationError> {
    let mut text = String::new();
    let deliverable_ids = criterion
        .deliverable_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for deliverable in &input.task_spec.deliverables {
        if !deliverable_ids.is_empty() && !deliverable_ids.contains(&deliverable.deliverable_id) {
            continue;
        }
        let path = input.output_root.join(&deliverable.required_path);
        if path.is_file() {
            let content =
                fs::read_to_string(&path).map_err(|source| LegalBenchmarkEvaluationError::Io {
                    path: path.clone(),
                    source,
                })?;
            text.push_str(&format!(
                "\n\n# Deliverable {}\n{}\n",
                deliverable.deliverable_id, content
            ));
        }
    }
    Ok(text)
}

fn criterion_evidence_refs(
    criterion: &CriterionSpec,
    input: &LegalBenchmarkEvaluationInput,
) -> Vec<String> {
    let mut refs = criterion.source_artifact_ids.clone();
    refs.extend(criterion.deliverable_ids.iter().cloned());
    if refs.is_empty() {
        refs.extend(
            input
                .output_artifact_manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone()),
        );
    }
    refs
}

fn deliverable_type_matches(deliverable: &DeliverableSpec, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match deliverable.deliverable_kind {
        DeliverableKind::Text => matches!(extension.as_str(), "txt" | "text"),
        DeliverableKind::Markdown => matches!(extension.as_str(), "md" | "markdown"),
        DeliverableKind::Docx => extension == "docx",
        DeliverableKind::Xlsx => extension == "xlsx",
        DeliverableKind::Pdf => extension == "pdf",
        DeliverableKind::Json => extension == "json",
        DeliverableKind::Directory => path.is_dir(),
        DeliverableKind::Other => true,
    }
}

fn default_judge_prompt_template(policy: &JudgePolicy) -> String {
    format!(
        "Judge each legal benchmark criterion using all-pass semantics. template={} mode={:?} samples={}",
        policy.prompt_template_id, policy.mode, policy.sample_count
    )
}

#[cfg(test)]
fn now_ms() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, ArtifactManifestRole, CriterionKind, DataClassification, DeliverableKind,
        JudgeMode, RunMetrics, RunTerminalState, ToolCallRecord, ToolPolicy, TranscriptEvent,
        TranscriptEventKind, build_output_artifact_manifest,
    };
    use sha2::{Digest, Sha256};

    fn task_spec() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.eval.mock"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            title: String::from("Evaluate memo"),
            instructions: String::from("Produce a memo."),
            work_type: String::from("review"),
            tags: vec![String::from("eval")],
            source_artifacts: Vec::new(),
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Memo"),
                required: true,
            }],
            criteria: vec![
                CriterionSpec {
                    criterion_id: String::from("criterion.memo.exists"),
                    criterion_kind: CriterionKind::DeliverableValidation,
                    description: String::from("The memo exists."),
                    weight_bps: Some(5000),
                    deliverable_ids: vec![String::from("memo")],
                    source_artifact_ids: Vec::new(),
                },
                CriterionSpec {
                    criterion_id: String::from("criterion.reasoning"),
                    criterion_kind: CriterionKind::LegalReasoning,
                    description: String::from("The memo includes legal reasoning."),
                    weight_bps: Some(5000),
                    deliverable_ids: vec![String::from("memo")],
                    source_artifact_ids: Vec::new(),
                },
            ],
            judge_policy: JudgePolicy {
                mode: JudgeMode::Llm,
                provider: String::from("mock"),
                model: String::from("mock-judge"),
                prompt_template_id: String::from("judge.legal.v1"),
                prompt_template_hash: String::from("fixture-template-hash"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("write")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 4,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: Metadata::new(),
        }
    }

    fn run_record(task: &BenchmarkTaskSpec, output_manifest: &ArtifactManifest) -> RunRecord {
        RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run.eval.mock"),
            task_id: task.task_id.clone(),
            task_version: task.task_version.clone(),
            input_artifact_manifest_hash: String::from("input-hash"),
            run_config_hash: String::from("config-hash"),
            output_artifact_manifest_hash: artifact_manifest_digest(output_manifest)
                .expect("manifest hash"),
            terminal_state: RunTerminalState::Submitted,
            transcript: vec![TranscriptEvent {
                event_index: 0,
                event_kind: TranscriptEventKind::Assistant,
                role: Some(String::from("assistant")),
                content: Some(String::from("submitted")),
                payload: None,
                timestamp_ms: now_ms(),
            }],
            tool_calls: Vec::new(),
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 10,
                output_tokens: 5,
                wall_time_ms: 100,
                estimated_cost_micro_usd: 12,
            },
            extraction_receipt_refs: vec![String::from("extract.1")],
            coverage_snapshot: None,
            metadata: Metadata::new(),
        }
    }

    fn write_tool_call(relative_path: &str, content: &str) -> ToolCallRecord {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let after_hash = hex::encode(hasher.finalize());
        ToolCallRecord {
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

    #[test]
    fn evaluator_scores_completed_run_with_mock_judge() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let memo_content = "# Memo\n\nLegal reasoning.\n";
        fs::write(output_root.join("memo.md"), memo_content).expect("memo");
        let task = task_spec();
        let artifact = artifact_from_file(
            "artifact.output.0",
            ArtifactKind::GeneratedDeliverable,
            &output_root,
            output_root.join("memo.md"),
            DataClassification::BenchmarkConfidential,
            Some(String::from("test")),
        )
        .expect("artifact");
        let output_manifest = build_output_artifact_manifest(
            task.task_id.clone(),
            task.task_version.clone(),
            "run.eval.mock",
            vec![artifact],
        );
        assert_eq!(output_manifest.manifest_role, ArtifactManifestRole::Output);
        let mut run_record = run_record(&task, &output_manifest);
        run_record.tool_calls = vec![write_tool_call("memo.md", memo_content)];
        run_record.metrics.tool_call_count = 1;
        let input = LegalBenchmarkEvaluationInput {
            task_spec: task,
            run_record,
            output_artifact_manifest: output_manifest,
            output_root,
        };
        let mut judge = MockLegalBenchmarkJudge::pass();
        let result = evaluate_legal_benchmark_run(&input, &mut judge).expect("evaluation");

        assert!(result.score_report.all_pass);
        assert_eq!(result.score_report.criterion_pass_rate_bps, 10_000);
        assert_eq!(result.score_report.document_coverage_bps, 10_000);
        assert_eq!(
            result.score_report.extraction_receipt_refs,
            vec!["extract.1"]
        );
        assert!(result.score_report.failure_diagnostics.is_empty());
        assert_eq!(result.score_report.criterion_results.len(), 2);
        assert!(result.score_report_hash.len() == 64);
    }

    #[test]
    fn missing_deliverable_fails_deterministic_precheck() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let task = task_spec();
        let output_manifest = build_output_artifact_manifest(
            task.task_id.clone(),
            task.task_version.clone(),
            "run.eval.missing",
            Vec::new(),
        );
        let run_record = run_record(&task, &output_manifest);
        let input = LegalBenchmarkEvaluationInput {
            task_spec: task,
            run_record,
            output_artifact_manifest: output_manifest,
            output_root,
        };
        let mut judge = MockLegalBenchmarkJudge::pass();
        let result = evaluate_legal_benchmark_run(&input, &mut judge).expect("evaluation");

        assert!(!result.score_report.all_pass);
        assert!(!result.score_report.failure_diagnostics.is_empty());
        assert_eq!(
            result.score_report.criterion_results[0].verdict,
            CriterionVerdict::Fail
        );
    }
}
