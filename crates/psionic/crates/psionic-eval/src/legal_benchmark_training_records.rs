//! Export legal benchmark runs into canonical fine-tuning records.

use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    LegalBenchmarkHiddenCriterionPolicy, LegalBenchmarkTrainingDeliverableRef,
    LegalBenchmarkTrainingEvidenceRef, LegalBenchmarkTrainingExample,
    LegalBenchmarkTrainingExampleKind, LegalBenchmarkTrainingRecord,
    LegalBenchmarkTrainingRecordBundle, LegalBenchmarkTrainingRecordError,
    LegalBenchmarkTrainingSplit, LegalBenchmarkTrainingToolInvocation,
    LegalBenchmarkTrainingVisibility,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    BenchmarkTaskSpec, CoverageMode, CoverageSnapshot, CriterionFailureClass, CriterionVerdict,
    RunRecord, ScoreReport, ToolCallRecord, ToolPolicy, run_record_digest, score_report_digest,
    stable_json_digest, transcript_digest,
};

/// Split policy for legal benchmark training-record export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkTrainingRecordSplitPolicy {
    /// Put every exported record in the retained smoke split.
    RetainedSmoke,
    /// Deterministically partition by task id into train, development, and holdout.
    DeterministicTrainDevHoldout,
}

/// Input bundle for legal benchmark training-record export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingRecordExportInput {
    /// Output bundle id.
    pub bundle_id: String,
    /// Benchmark suite id.
    pub suite_id: String,
    /// Source campaign/report id.
    pub source_report_id: String,
    /// Task specs used by the runs.
    pub task_specs: Vec<BenchmarkTaskSpec>,
    /// Run records to export.
    pub run_records: Vec<RunRecord>,
    /// Score reports paired to run records.
    pub score_reports: Vec<ScoreReport>,
    /// Split policy.
    pub split_policy: LegalBenchmarkTrainingRecordSplitPolicy,
}

/// Errors raised while exporting legal benchmark training records.
#[derive(Debug, Error)]
pub enum LegalBenchmarkTrainingRecordExportError {
    /// Missing task for a run/score.
    #[error("missing task spec `{task_id}`")]
    MissingTask {
        /// Missing task id.
        task_id: String,
    },
    /// Missing run for a score report.
    #[error("missing run record `{run_id}`")]
    MissingRun {
        /// Missing run id.
        run_id: String,
    },
    /// Digest or serialization failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Canonical record validation failure.
    #[error(transparent)]
    Record(#[from] LegalBenchmarkTrainingRecordError),
}

/// Exports existing legal benchmark task/run/score records into canonical
/// fine-tuning records.
pub fn export_legal_benchmark_training_records(
    input: &LegalBenchmarkTrainingRecordExportInput,
) -> Result<LegalBenchmarkTrainingRecordBundle, LegalBenchmarkTrainingRecordExportError> {
    let task_by_id = input
        .task_specs
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    let run_by_id = input
        .run_records
        .iter()
        .map(|run| (run.run_id.as_str(), run))
        .collect::<BTreeMap<_, _>>();

    let mut score_reports = input.score_reports.iter().collect::<Vec<_>>();
    score_reports.sort_by(|left, right| {
        left.task_id
            .cmp(&right.task_id)
            .then_with(|| left.run_id.cmp(&right.run_id))
            .then_with(|| left.score_report_id.cmp(&right.score_report_id))
    });

    let mut records = Vec::new();
    for score_report in score_reports {
        let run_record = run_by_id.get(score_report.run_id.as_str()).ok_or_else(|| {
            LegalBenchmarkTrainingRecordExportError::MissingRun {
                run_id: score_report.run_id.clone(),
            }
        })?;
        let task_spec = task_by_id
            .get(score_report.task_id.as_str())
            .ok_or_else(|| LegalBenchmarkTrainingRecordExportError::MissingTask {
                task_id: score_report.task_id.clone(),
            })?;
        records.push(export_one_record(
            input,
            task_spec,
            run_record,
            score_report,
        )?);
    }

    let source_report_digest =
        stable_json_digest("psionic.legal_benchmark.training_record_source.v1", input)?;
    LegalBenchmarkTrainingRecordBundle::new(
        input.bundle_id.clone(),
        input.suite_id.clone(),
        input.source_report_id.clone(),
        source_report_digest,
        records,
        "legal benchmark training records preserve source evidence and judge-only scoring provenance; they are dataset artifacts, not score-lift claims",
    )
    .map_err(Into::into)
}

fn export_one_record(
    input: &LegalBenchmarkTrainingRecordExportInput,
    task_spec: &BenchmarkTaskSpec,
    run_record: &RunRecord,
    score_report: &ScoreReport,
) -> Result<LegalBenchmarkTrainingRecord, LegalBenchmarkTrainingRecordExportError> {
    let coverage_snapshot = score_report
        .coverage_snapshot
        .as_ref()
        .or(run_record.coverage_snapshot.as_ref());
    let hidden_criterion_policy = hidden_criterion_policy(coverage_snapshot);
    let model_visible_allowed = hidden_criterion_policy.allows_model_visible_examples();
    let split = split_for_task(input.split_policy, task_spec.task_id.as_str());
    let score_digest = score_report_digest(score_report)?;
    let coverage_snapshot_ref = coverage_snapshot_digest(coverage_snapshot)?;
    let examples = training_examples(
        task_spec,
        run_record,
        score_report,
        coverage_snapshot,
        model_visible_allowed,
    )?;
    LegalBenchmarkTrainingRecord::new(
        format!(
            "legal.training.{}.{}",
            sanitize_id(task_spec.task_id.as_str()),
            sanitize_id(run_record.run_id.as_str())
        ),
        input.suite_id.clone(),
        task_spec.task_id.clone(),
        task_spec.task_version.clone(),
        task_spec.practice_area.clone(),
        task_spec.work_type.clone(),
        run_record.input_artifact_manifest_hash.clone(),
        tool_policy_digest(&task_spec.tool_policy)?,
        tool_invocations(&run_record.tool_calls),
        evidence_refs(coverage_snapshot),
        deliverable_refs(task_spec),
        coverage_snapshot_ref,
        score_report.score_report_id.clone(),
        score_digest,
        failure_family_labels(score_report),
        judge_provenance(score_report),
        hidden_criterion_policy,
        split,
        examples,
        record_metadata(run_record, score_report)?,
    )
    .map_err(Into::into)
}

fn training_examples(
    task_spec: &BenchmarkTaskSpec,
    run_record: &RunRecord,
    score_report: &ScoreReport,
    coverage_snapshot: Option<&CoverageSnapshot>,
    model_visible_allowed: bool,
) -> Result<Vec<LegalBenchmarkTrainingExample>, serde_json::Error> {
    let visibility = if model_visible_allowed {
        LegalBenchmarkTrainingVisibility::ModelVisible
    } else {
        LegalBenchmarkTrainingVisibility::ExcludedFromTraining
    };
    let excluded_reason = (!model_visible_allowed).then(|| {
        String::from("source run included hidden criterion material in model-visible content")
    });

    let mut examples = Vec::new();
    examples.push(LegalBenchmarkTrainingExample {
        example_id: format!("{}.directive", task_spec.task_id),
        example_kind: LegalBenchmarkTrainingExampleKind::TaskDirective,
        visibility,
        input_text: task_spec.instructions.clone(),
        target_text: None,
        source_refs: task_spec
            .source_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        excluded_reason: excluded_reason.clone(),
    });

    if let Some(final_answer) = final_assistant_text(run_record) {
        examples.push(LegalBenchmarkTrainingExample {
            example_id: format!("{}.final_answer", run_record.run_id),
            example_kind: LegalBenchmarkTrainingExampleKind::EvidenceBackedDraft,
            visibility,
            input_text: String::from("Produce the final legal benchmark deliverable."),
            target_text: Some(final_answer),
            source_refs: coverage_snapshot
                .map(|snapshot| {
                    snapshot
                        .evidence_refs
                        .iter()
                        .map(|evidence| evidence.evidence_id.clone())
                        .collect()
                })
                .unwrap_or_default(),
            excluded_reason: excluded_reason.clone(),
        });
    }

    if !run_record.tool_calls.is_empty() {
        examples.push(LegalBenchmarkTrainingExample {
            example_id: format!("{}.tool_trace", run_record.run_id),
            example_kind: LegalBenchmarkTrainingExampleKind::ToolTrace,
            visibility,
            input_text: serde_json::to_string(&run_record.tool_calls)?,
            target_text: None,
            source_refs: Vec::new(),
            excluded_reason,
        });
    }

    examples.push(LegalBenchmarkTrainingExample {
        example_id: format!("{}.judge", score_report.score_report_id),
        example_kind: LegalBenchmarkTrainingExampleKind::JudgeRationale,
        visibility: LegalBenchmarkTrainingVisibility::JudgeOnly,
        input_text: judge_summary(score_report),
        target_text: None,
        source_refs: score_report
            .criterion_results
            .iter()
            .flat_map(|criterion| criterion.evidence_refs.clone())
            .collect(),
        excluded_reason: None,
    });

    Ok(examples)
}

fn final_assistant_text(run_record: &RunRecord) -> Option<String> {
    run_record
        .transcript
        .iter()
        .rev()
        .find(|event| event.event_kind == crate::TranscriptEventKind::Assistant)
        .and_then(|event| event.content.clone())
        .filter(|content| !content.trim().is_empty())
}

fn tool_invocations(tool_calls: &[ToolCallRecord]) -> Vec<LegalBenchmarkTrainingToolInvocation> {
    let mut invocations = tool_calls
        .iter()
        .map(|tool_call| LegalBenchmarkTrainingToolInvocation {
            tool_call_id: tool_call.tool_call_id.clone(),
            tool_name: tool_call.tool_name.clone(),
            call_event_index: tool_call.call_event_index,
            result_event_index: tool_call.result_event_index,
            input: tool_call.input.clone(),
            output: tool_call.output.clone(),
            error_kind: tool_call.error_kind.clone(),
        })
        .collect::<Vec<_>>();
    invocations.sort_by(|left, right| {
        left.call_event_index
            .cmp(&right.call_event_index)
            .then_with(|| left.tool_call_id.cmp(&right.tool_call_id))
    });
    invocations
}

fn evidence_refs(
    coverage_snapshot: Option<&CoverageSnapshot>,
) -> Vec<LegalBenchmarkTrainingEvidenceRef> {
    coverage_snapshot
        .map(|snapshot| {
            snapshot
                .evidence_refs
                .iter()
                .map(|evidence| LegalBenchmarkTrainingEvidenceRef {
                    evidence_id: evidence.evidence_id.clone(),
                    source_ref: evidence.source_ref.clone(),
                    locator: evidence.locator.clone(),
                    span_hash: evidence.span_hash.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn deliverable_refs(task_spec: &BenchmarkTaskSpec) -> Vec<LegalBenchmarkTrainingDeliverableRef> {
    task_spec
        .deliverables
        .iter()
        .map(|deliverable| LegalBenchmarkTrainingDeliverableRef {
            deliverable_id: deliverable.deliverable_id.clone(),
            relative_path: deliverable.required_path.clone(),
            required: deliverable.required,
        })
        .collect()
}

fn hidden_criterion_policy(
    coverage_snapshot: Option<&CoverageSnapshot>,
) -> LegalBenchmarkHiddenCriterionPolicy {
    match coverage_snapshot {
        Some(snapshot) if snapshot.hidden_criteria_visible => {
            LegalBenchmarkHiddenCriterionPolicy::HiddenCriteriaVisible
        }
        Some(snapshot)
            if snapshot.mode == CoverageMode::HillClimb
                && snapshot
                    .derived_checklist_items
                    .iter()
                    .any(|item| item.agent_visible) =>
        {
            LegalBenchmarkHiddenCriterionPolicy::DerivedChecklistModelVisible
        }
        Some(_) => LegalBenchmarkHiddenCriterionPolicy::JudgeOnlyExcluded,
        None => LegalBenchmarkHiddenCriterionPolicy::NotPresent,
    }
}

fn split_for_task(
    split_policy: LegalBenchmarkTrainingRecordSplitPolicy,
    task_id: &str,
) -> LegalBenchmarkTrainingSplit {
    match split_policy {
        LegalBenchmarkTrainingRecordSplitPolicy::RetainedSmoke => {
            LegalBenchmarkTrainingSplit::RetainedSmoke
        }
        LegalBenchmarkTrainingRecordSplitPolicy::DeterministicTrainDevHoldout => {
            match stable_bucket(task_id) % 10 {
                0 => LegalBenchmarkTrainingSplit::Holdout,
                1 => LegalBenchmarkTrainingSplit::Development,
                _ => LegalBenchmarkTrainingSplit::Train,
            }
        }
    }
}

fn failure_family_labels(score_report: &ScoreReport) -> Vec<String> {
    let mut labels = score_report
        .failure_comparisons
        .iter()
        .map(|comparison| failure_class_label(comparison.failure_class).to_string())
        .collect::<BTreeSet<_>>();
    if score_report.all_pass {
        labels.insert(String::from("passed"));
    }
    if labels.is_empty() {
        labels.insert(String::from("unclassified"));
    }
    labels.into_iter().collect()
}

fn judge_provenance(score_report: &ScoreReport) -> Vec<String> {
    score_report
        .criterion_results
        .iter()
        .map(|criterion| {
            format!(
                "{}@{}:{}",
                criterion.judge_model, criterion.judge_prompt_hash, criterion.raw_response_hash
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn judge_summary(score_report: &ScoreReport) -> String {
    let mut rows = Vec::new();
    for criterion in &score_report.criterion_results {
        rows.push(format!(
            "{}:{}:{}",
            criterion.criterion_id,
            criterion_verdict_label(criterion.verdict),
            criterion.reasoning
        ));
    }
    if rows.is_empty() {
        return format!(
            "score_report={} all_pass={} criterion_pass_rate_bps={}",
            score_report.score_report_id,
            score_report.all_pass,
            score_report.criterion_pass_rate_bps
        );
    }
    rows.join("\n")
}

fn record_metadata(
    run_record: &RunRecord,
    score_report: &ScoreReport,
) -> Result<BTreeMap<String, Value>, serde_json::Error> {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        String::from("run_record_digest"),
        Value::String(run_record_digest(run_record)?),
    );
    metadata.insert(
        String::from("transcript_digest"),
        Value::String(transcript_digest(&run_record.transcript)?),
    );
    metadata.insert(
        String::from("terminal_state"),
        serde_json::to_value(run_record.terminal_state)?,
    );
    metadata.insert(
        String::from("criterion_pass_rate_bps"),
        Value::Number(score_report.criterion_pass_rate_bps.into()),
    );
    Ok(metadata)
}

fn coverage_snapshot_digest(
    coverage_snapshot: Option<&CoverageSnapshot>,
) -> Result<Option<String>, serde_json::Error> {
    coverage_snapshot
        .map(|snapshot| {
            stable_json_digest("psionic.legal_benchmark.coverage_snapshot.v1", snapshot)
        })
        .transpose()
}

fn tool_policy_digest(tool_policy: &ToolPolicy) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.tool_policy.v1", tool_policy)
}

fn stable_bucket(value: &str) -> u64 {
    value.as_bytes().iter().fold(0_u64, |acc, byte| {
        acc.wrapping_mul(131).wrapping_add(u64::from(*byte))
    })
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn failure_class_label(failure_class: CriterionFailureClass) -> &'static str {
    match failure_class {
        CriterionFailureClass::CoverageGap => "coverage_gap",
        CriterionFailureClass::ExtractionGap => "extraction_gap",
        CriterionFailureClass::DraftingGap => "drafting_gap",
        CriterionFailureClass::ReasoningGap => "reasoning_gap",
        CriterionFailureClass::Passed => "passed",
    }
}

fn criterion_verdict_label(verdict: CriterionVerdict) -> &'static str {
    match verdict {
        CriterionVerdict::Pass => "pass",
        CriterionVerdict::Fail => "fail",
        CriterionVerdict::Ambiguous => "ambiguous",
        CriterionVerdict::NotEvaluated => "not_evaluated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, BenchmarkTaskSpec, CoverageSnapshot, DataClassification, DeliverableKind,
        DeliverableSpec, DocumentCoverageEntry, EvidenceCoverageEntry, JudgeMode, JudgePolicy,
        RunMetrics, RunTerminalState, SourceArtifact, ToolPolicy, TranscriptEvent,
        TranscriptEventKind,
    };

    fn task_spec() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("harvey.task.001"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            title: String::from("Review renewal clause"),
            instructions: String::from("Review the agreement and summarize renewal obligations."),
            work_type: String::from("memo"),
            tags: vec![String::from("smoke")],
            source_artifacts: vec![SourceArtifact {
                artifact_id: String::from("source.contract"),
                artifact_kind: ArtifactKind::SourceDocument,
                relative_path: String::from("contract.pdf"),
                original_filename: String::from("contract.pdf"),
                media_type: String::from("application/pdf"),
                byte_size: 10,
                sha256: String::from("source-digest"),
                data_classification: DataClassification::BenchmarkConfidential,
                provenance: None,
            }],
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Renewal memo"),
                required: true,
            }],
            criteria: Vec::new(),
            judge_policy: JudgePolicy {
                mode: JudgeMode::Deterministic,
                provider: String::from("local"),
                model: String::from("judge.local"),
                prompt_template_id: String::from("judge.template"),
                prompt_template_hash: String::from("judge-prompt"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("pdf_search")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 4,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: BTreeMap::new(),
        }
    }

    fn coverage(hidden_criteria_visible: bool) -> CoverageSnapshot {
        CoverageSnapshot {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            mode: CoverageMode::Integrity,
            hidden_criteria_visible,
            derived_checklist_items: Vec::new(),
            documents: vec![DocumentCoverageEntry {
                artifact_id: String::from("source.contract"),
                relative_path: String::from("contract.pdf"),
                discovered: true,
                read: true,
                used_extracted_text: true,
            }],
            facts: Vec::new(),
            evidence_refs: vec![EvidenceCoverageEntry {
                evidence_id: String::from("ev.renewal"),
                source_ref: String::from("source.contract"),
                locator: Some(String::from("p.2")),
                span_hash: String::from("span-digest"),
            }],
            deliverable_sections: Vec::new(),
            validations: Vec::new(),
            self_checks: Vec::new(),
        }
    }

    fn run_record(hidden_criteria_visible: bool) -> RunRecord {
        RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run.001"),
            task_id: String::from("harvey.task.001"),
            task_version: String::from("v1"),
            input_artifact_manifest_hash: String::from("input-manifest"),
            run_config_hash: String::from("run-config"),
            output_artifact_manifest_hash: String::from("output-manifest"),
            terminal_state: RunTerminalState::Submitted,
            transcript: vec![TranscriptEvent {
                event_index: 0,
                event_kind: TranscriptEventKind::Assistant,
                role: Some(String::from("assistant")),
                content: Some(String::from("The renewal term is one year, citing p.2.")),
                payload: None,
                timestamp_ms: 1,
            }],
            tool_calls: Vec::new(),
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 10,
                output_tokens: 12,
                wall_time_ms: 50,
                estimated_cost_micro_usd: 1,
            },
            extraction_receipt_refs: vec![String::from("extract.001")],
            coverage_snapshot: Some(coverage(hidden_criteria_visible)),
            metadata: BTreeMap::new(),
        }
    }

    fn score_report(hidden_criteria_visible: bool) -> ScoreReport {
        ScoreReport {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            score_report_id: String::from("score.001"),
            run_id: String::from("run.001"),
            task_id: String::from("harvey.task.001"),
            task_version: String::from("v1"),
            run_record_hash: String::from("run-hash"),
            output_artifact_manifest_hash: String::from("output-manifest"),
            all_pass: true,
            criterion_pass_rate_bps: 10_000,
            criterion_results: Vec::new(),
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 10,
                output_tokens: 12,
                wall_time_ms: 50,
                estimated_cost_micro_usd: 1,
            },
            document_coverage_bps: 10_000,
            failure_diagnostics: Vec::new(),
            extraction_receipt_refs: vec![String::from("extract.001")],
            coverage_snapshot: Some(coverage(hidden_criteria_visible)),
            failure_comparisons: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn export_retained_smoke_records_is_deterministic() {
        let input = LegalBenchmarkTrainingRecordExportInput {
            bundle_id: String::from("bundle.harvey.retained_smoke.v1"),
            suite_id: String::from("harvey_labs"),
            source_report_id: String::from("report.retained_smoke"),
            task_specs: vec![task_spec()],
            run_records: vec![run_record(false)],
            score_reports: vec![score_report(false)],
            split_policy: LegalBenchmarkTrainingRecordSplitPolicy::RetainedSmoke,
        };
        let first = export_legal_benchmark_training_records(&input).expect("first export");
        let second = export_legal_benchmark_training_records(&input).expect("second export");
        assert_eq!(first.bundle_digest, second.bundle_digest);
        assert_eq!(
            first.records[0].split,
            LegalBenchmarkTrainingSplit::RetainedSmoke
        );
        assert!(first.records[0]
            .examples
            .iter()
            .any(|example| example.visibility == LegalBenchmarkTrainingVisibility::ModelVisible));
    }

    #[test]
    fn export_excludes_model_visible_examples_when_hidden_criteria_visible() {
        let input = LegalBenchmarkTrainingRecordExportInput {
            bundle_id: String::from("bundle.harvey.hidden.v1"),
            suite_id: String::from("harvey_labs"),
            source_report_id: String::from("report.hidden"),
            task_specs: vec![task_spec()],
            run_records: vec![run_record(true)],
            score_reports: vec![score_report(true)],
            split_policy: LegalBenchmarkTrainingRecordSplitPolicy::RetainedSmoke,
        };
        let bundle = export_legal_benchmark_training_records(&input).expect("export");
        assert_eq!(
            bundle.records[0].hidden_criterion_policy,
            LegalBenchmarkHiddenCriterionPolicy::HiddenCriteriaVisible
        );
        assert!(bundle.records[0]
            .examples
            .iter()
            .all(|example| example.visibility != LegalBenchmarkTrainingVisibility::ModelVisible));
    }
}
