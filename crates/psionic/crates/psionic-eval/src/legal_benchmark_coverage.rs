//! Criterion-adjacent coverage tracking for legal benchmark runs.
//!
//! This module tracks what the agent discovered, read, extracted, drafted, and
//! validated without giving the model hidden rubric criteria in integrity mode.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    AgentVisibleChecklist, ArtifactManifest, BenchmarkTaskSpec, CoverageMode, CoverageSnapshot,
    CriterionCoverageFailure, CriterionFailureClass, CriterionResult, CriterionVerdict,
    DeliverableSectionCoverage, DerivedChecklistItem, DocumentCoverageEntry, EvidenceCoverageEntry,
    FactCoverageEntry, LegalBenchmarkPathRoot, Metadata, RunConfig, SelfCheckCoverageEntry,
    SourceArtifact, ToolCallRecord, TranscriptEvent, ValidationCoverageEntry, stable_json_digest,
};

pub const LEGAL_BENCHMARK_COVERAGE_SCHEMA_VERSION: u16 = 1;

/// Returns the configured coverage mode. Integrity mode is the default.
#[must_use]
pub fn coverage_mode_from_metadata(metadata: &Metadata) -> CoverageMode {
    metadata
        .get("coverage_mode")
        .and_then(Value::as_str)
        .map(|mode| match mode {
            "hill_climb" | "training" => CoverageMode::HillClimb,
            _ => CoverageMode::Integrity,
        })
        .unwrap_or(CoverageMode::Integrity)
}

/// Returns the configured run coverage mode.
#[must_use]
pub fn coverage_mode_from_run_config(run_config: &RunConfig) -> CoverageMode {
    coverage_mode_from_metadata(&run_config.metadata)
}

/// Builds the model-visible checklist under the configured policy.
#[must_use]
pub fn agent_visible_checklist(run_config: &RunConfig) -> AgentVisibleChecklist {
    let mode = coverage_mode_from_run_config(run_config);
    let items = match mode {
        CoverageMode::Integrity => Vec::new(),
        CoverageMode::HillClimb => derived_checklist_items(&run_config.metadata)
            .into_iter()
            .filter(|item| item.agent_visible)
            .collect(),
    };
    AgentVisibleChecklist {
        mode,
        items,
        hidden_criteria_visible: false,
    }
}

/// Builds a coverage snapshot from run-time tool and transcript state.
pub fn build_coverage_snapshot(
    task_spec: &BenchmarkTaskSpec,
    run_config: &RunConfig,
    tool_calls: &[ToolCallRecord],
    transcript: &[TranscriptEvent],
    output_manifest: &ArtifactManifest,
) -> Result<CoverageSnapshot, serde_json::Error> {
    let checklist = agent_visible_checklist(run_config);
    let mut documents = seed_document_entries(&task_spec.source_artifacts);
    let mut facts = Vec::new();
    let mut evidence_refs = Vec::new();
    let mut deliverable_sections =
        output_deliverable_sections(task_spec, output_manifest.artifacts.as_slice());
    let mut validations = build_deliverable_validations(task_spec, output_manifest);

    for call in tool_calls {
        apply_tool_call_coverage(
            task_spec,
            call,
            &mut documents,
            &mut facts,
            &mut evidence_refs,
            &mut deliverable_sections,
            &mut validations,
        )?;
    }

    let self_checks = transcript
        .iter()
        .filter_map(self_check_from_transcript_event)
        .collect::<Vec<_>>();

    Ok(CoverageSnapshot {
        schema_version: LEGAL_BENCHMARK_COVERAGE_SCHEMA_VERSION,
        mode: checklist.mode,
        hidden_criteria_visible: model_visible_text_contains_hidden_criteria(task_spec, transcript),
        derived_checklist_items: checklist.items,
        documents: documents.into_values().collect(),
        facts,
        evidence_refs,
        deliverable_sections,
        validations,
        self_checks,
    })
}

/// Builds a conservative coverage snapshot for imported or historical runs that
/// predate run-time coverage capture.
#[must_use]
pub fn fallback_coverage_snapshot(
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &ArtifactManifest,
) -> CoverageSnapshot {
    CoverageSnapshot {
        schema_version: LEGAL_BENCHMARK_COVERAGE_SCHEMA_VERSION,
        mode: CoverageMode::Integrity,
        hidden_criteria_visible: false,
        derived_checklist_items: Vec::new(),
        documents: seed_document_entries(&task_spec.source_artifacts)
            .into_values()
            .collect(),
        facts: Vec::new(),
        evidence_refs: Vec::new(),
        deliverable_sections: output_deliverable_sections(
            task_spec,
            output_manifest.artifacts.as_slice(),
        ),
        validations: build_deliverable_validations(task_spec, output_manifest),
        self_checks: Vec::new(),
    }
}

/// Computes document read coverage from a snapshot.
#[must_use]
pub fn document_coverage_bps_from_snapshot(
    task_spec: &BenchmarkTaskSpec,
    snapshot: &CoverageSnapshot,
) -> u32 {
    if task_spec.source_artifacts.is_empty() {
        return 10_000;
    }
    let read = snapshot
        .documents
        .iter()
        .filter(|entry| entry.read)
        .filter(|entry| {
            task_spec.source_artifacts.iter().any(|artifact| {
                artifact.artifact_id == entry.artifact_id
                    || artifact.relative_path == entry.relative_path
            })
        })
        .count();
    u32::try_from((read * 10_000) / task_spec.source_artifacts.len())
        .unwrap_or(0)
        .min(10_000)
}

/// Compares missed criteria against coverage state after judging.
#[must_use]
pub fn classify_criterion_failures(
    task_spec: &BenchmarkTaskSpec,
    criterion_results: &[CriterionResult],
    coverage: &CoverageSnapshot,
) -> Vec<CriterionCoverageFailure> {
    criterion_results
        .iter()
        .filter(|result| result.verdict != CriterionVerdict::Pass)
        .filter_map(|result| {
            let criterion = task_spec
                .criteria
                .iter()
                .find(|criterion| criterion.criterion_id == result.criterion_id)?;
            let read_sources = read_source_refs(coverage);
            let missing_source_artifact_ids = criterion
                .source_artifact_ids
                .iter()
                .filter(|source| !read_sources.contains(source.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let drafted_deliverables = drafted_deliverable_ids(coverage);
            let missing_deliverable_ids = criterion
                .deliverable_ids
                .iter()
                .filter(|deliverable| !drafted_deliverables.contains(deliverable.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let expected_source_refs =
                expected_source_refs(task_spec, criterion.source_artifact_ids.as_slice());
            let evidence_refs = criterion_evidence_refs(expected_source_refs.as_slice(), coverage);
            let validation_failed = coverage.validations.iter().any(|validation| {
                !validation.passed
                    && criterion
                        .deliverable_ids
                        .iter()
                        .any(|deliverable| validation.target_ref == *deliverable)
            });
            let failure_class = if !missing_source_artifact_ids.is_empty() {
                CriterionFailureClass::CoverageGap
            } else if !criterion.source_artifact_ids.is_empty() && evidence_refs.is_empty() {
                CriterionFailureClass::ExtractionGap
            } else if !missing_deliverable_ids.is_empty() || validation_failed {
                CriterionFailureClass::DraftingGap
            } else {
                CriterionFailureClass::ReasoningGap
            };
            Some(CriterionCoverageFailure {
                criterion_id: result.criterion_id.clone(),
                failure_class,
                missing_source_artifact_ids,
                missing_deliverable_ids,
                evidence_refs,
                diagnostic: failure_diagnostic(failure_class),
            })
        })
        .collect()
}

/// Returns true when model-visible transcript text contains raw hidden criteria.
#[must_use]
pub fn model_visible_text_contains_hidden_criteria(
    task_spec: &BenchmarkTaskSpec,
    transcript: &[TranscriptEvent],
) -> bool {
    let criteria = task_spec
        .criteria
        .iter()
        .map(|criterion| criterion.description.trim())
        .filter(|description| !description.is_empty())
        .collect::<Vec<_>>();
    if criteria.is_empty() {
        return false;
    }
    transcript.iter().any(|event| {
        if matches!(event.event_kind, crate::TranscriptEventKind::Judge) {
            return false;
        }
        let mut visible = String::new();
        if let Some(content) = &event.content {
            visible.push_str(content);
        }
        if let Some(payload) = &event.payload {
            visible.push_str(&payload.to_string());
        }
        criteria.iter().any(|criterion| visible.contains(criterion))
    })
}

fn derived_checklist_items(metadata: &Metadata) -> Vec<DerivedChecklistItem> {
    metadata
        .get("derived_checklist_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| derived_checklist_item(index, item))
                .collect()
        })
        .unwrap_or_default()
}

fn derived_checklist_item(index: usize, value: &Value) -> Option<DerivedChecklistItem> {
    if let Some(prompt) = value.as_str() {
        return Some(DerivedChecklistItem {
            item_id: format!("derived.checklist.{index}"),
            prompt: prompt.to_owned(),
            source: String::from("run_config.metadata"),
            agent_visible: true,
        });
    }
    let object = value.as_object()?;
    let prompt = object.get("prompt").and_then(Value::as_str)?.to_owned();
    Some(DerivedChecklistItem {
        item_id: object
            .get("item_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("derived.checklist.{index}")),
        prompt,
        source: object
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("run_config.metadata")
            .to_owned(),
        agent_visible: object
            .get("agent_visible")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn seed_document_entries(
    source_artifacts: &[SourceArtifact],
) -> BTreeMap<String, DocumentCoverageEntry> {
    source_artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.relative_path.clone(),
                DocumentCoverageEntry {
                    artifact_id: artifact.artifact_id.clone(),
                    relative_path: artifact.relative_path.clone(),
                    discovered: false,
                    read: false,
                    used_extracted_text: false,
                },
            )
        })
        .collect()
}

fn apply_tool_call_coverage(
    task_spec: &BenchmarkTaskSpec,
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
    facts: &mut Vec<FactCoverageEntry>,
    evidence_refs: &mut Vec<EvidenceCoverageEntry>,
    deliverable_sections: &mut Vec<DeliverableSectionCoverage>,
    validations: &mut Vec<ValidationCoverageEntry>,
) -> Result<(), serde_json::Error> {
    match call.tool_name.as_str() {
        "inventory" => apply_inventory_coverage(call, documents),
        "email_summary" | "spreadsheet_summary" => apply_summary_coverage(call, documents, facts)?,
        "pdf_search" => apply_pdf_search_coverage(call, documents, facts, evidence_refs)?,
        "evidence_table" => apply_evidence_table_coverage(call, evidence_refs),
        "validate_deliverables" => apply_validate_deliverables_coverage(call, validations),
        "glob" => apply_glob_coverage(call, documents),
        "grep" => apply_grep_coverage(call, documents, facts, evidence_refs)?,
        "read" => apply_read_coverage(call, documents, facts)?,
        "write" | "edit" => apply_write_coverage(task_spec, call, deliverable_sections),
        _ => {}
    }
    Ok(())
}

fn apply_inventory_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
) {
    if !input_root_is_documents(call) {
        return;
    }
    let Some(artifacts) = output_payload(call, "inventory")
        .and_then(|inventory| inventory.get("artifacts"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for artifact in artifacts {
        if let Some(relative_path) = artifact.get("relative_path").and_then(Value::as_str) {
            document_entry(documents, relative_path).discovered = true;
        }
    }
}

fn apply_summary_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
    facts: &mut Vec<FactCoverageEntry>,
) -> Result<(), serde_json::Error> {
    if !input_root_is_documents(call) {
        return Ok(());
    }
    let Some(relative_path) = input_payload(call)
        .and_then(|input| input.get("relative_path"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    let entry = document_entry(documents, relative_path);
    entry.discovered = true;
    entry.read = true;
    if let Some(output) = &call.output {
        facts.push(FactCoverageEntry {
            fact_id: format!("fact.{}.{}", stable_path_id(relative_path), facts.len()),
            source_ref: relative_path.to_owned(),
            text_hash: stable_json_digest("psionic.legal_benchmark.coverage.summary.v1", output)?,
            method: call.tool_name.clone(),
        });
    }
    Ok(())
}

fn apply_pdf_search_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
    facts: &mut Vec<FactCoverageEntry>,
    evidence_refs: &mut Vec<EvidenceCoverageEntry>,
) -> Result<(), serde_json::Error> {
    if !input_root_is_documents(call) {
        return Ok(());
    }
    let Some(matches) = output_payload(call, "pdf_search")
        .and_then(|pdf| pdf.get("matches"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    for matched in matches {
        let Some(relative_path) = matched.get("relative_path").and_then(Value::as_str) else {
            continue;
        };
        let entry = document_entry(documents, relative_path);
        entry.discovered = true;
        entry.read = true;
        let snippet = matched
            .get("snippet")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let span_hash = matched
            .get("span_hash")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                stable_json_digest("psionic.legal_benchmark.coverage.pdf_search.v1", &snippet)
                    .unwrap_or_default()
            });
        let page = matched.get("page").and_then(Value::as_u64).unwrap_or(0);
        let evidence_id = format!("evidence.{}.page.{page}", stable_path_id(relative_path));
        evidence_refs.push(EvidenceCoverageEntry {
            evidence_id,
            source_ref: relative_path.to_owned(),
            locator: Some(format!("page:{page}")),
            span_hash: span_hash.clone(),
        });
        facts.push(FactCoverageEntry {
            fact_id: format!("fact.{}.{}", stable_path_id(relative_path), facts.len()),
            source_ref: relative_path.to_owned(),
            text_hash: span_hash,
            method: String::from("pdf_search"),
        });
    }
    Ok(())
}

fn apply_evidence_table_coverage(
    call: &ToolCallRecord,
    evidence_refs: &mut Vec<EvidenceCoverageEntry>,
) {
    let Some(rows) = output_payload(call, "evidence_table")
        .and_then(|table| table.get("rows"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for row in rows {
        let Some(evidence_id) = row.get("evidence_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(source_ref) = row.get("source_ref").and_then(Value::as_str) else {
            continue;
        };
        let span_hash = row
            .get("quote_hash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        evidence_refs.push(EvidenceCoverageEntry {
            evidence_id: evidence_id.to_string(),
            source_ref: source_ref.to_string(),
            locator: row
                .get("locator")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            span_hash,
        });
    }
}

fn apply_validate_deliverables_coverage(
    call: &ToolCallRecord,
    validations: &mut Vec<ValidationCoverageEntry>,
) {
    let Some(rows) = output_payload(call, "validate_deliverables")
        .and_then(|output| output.get("validations"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for row in rows {
        let Some(relative_path) = row.get("relative_path").and_then(Value::as_str) else {
            continue;
        };
        let exists = row.get("exists").and_then(Value::as_bool).unwrap_or(false);
        let readable = row
            .get("readable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        validations.push(ValidationCoverageEntry {
            validation_id: format!("validation.tool.{}", stable_path_id(relative_path)),
            target_ref: relative_path.to_string(),
            passed: exists && readable,
            detail: if exists && readable {
                format!("deliverable {relative_path} exists and is readable")
            } else {
                format!("deliverable {relative_path} missing or unreadable")
            },
        });
    }
}

fn apply_glob_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
) {
    if !input_root_is_documents(call) {
        return;
    }
    let Some(matches) = output_payload(call, "glob")
        .and_then(|glob| glob.get("matches"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for relative_path in matches.iter().filter_map(Value::as_str) {
        document_entry(documents, relative_path).discovered = true;
    }
}

fn apply_read_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
    facts: &mut Vec<FactCoverageEntry>,
) -> Result<(), serde_json::Error> {
    if !input_root_is_documents(call) {
        return Ok(());
    }
    let Some(relative_path) = input_payload(call)
        .and_then(|input| input.get("relative_path"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    let entry = document_entry(documents, relative_path);
    entry.discovered = true;
    entry.read = true;
    entry.used_extracted_text = output_payload(call, "read")
        .and_then(|read| read.get("source"))
        .and_then(Value::as_str)
        .is_some_and(|source| source == "extracted_text");
    if let Some(content) = output_payload(call, "read")
        .and_then(|read| read.get("content"))
        .and_then(Value::as_str)
    {
        facts.push(FactCoverageEntry {
            fact_id: format!("fact.{}.{}", stable_path_id(relative_path), facts.len()),
            source_ref: relative_path.to_owned(),
            text_hash: stable_json_digest("psionic.legal_benchmark.coverage.fact.v1", &content)?,
            method: String::from("read"),
        });
    }
    Ok(())
}

fn apply_grep_coverage(
    call: &ToolCallRecord,
    documents: &mut BTreeMap<String, DocumentCoverageEntry>,
    facts: &mut Vec<FactCoverageEntry>,
    evidence_refs: &mut Vec<EvidenceCoverageEntry>,
) -> Result<(), serde_json::Error> {
    if !input_root_is_documents(call) {
        return Ok(());
    }
    let Some(matches) = output_payload(call, "grep")
        .and_then(|grep| grep.get("matches"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    for matched in matches {
        let Some(relative_path) = matched.get("relative_path").and_then(Value::as_str) else {
            continue;
        };
        let entry = document_entry(documents, relative_path);
        entry.discovered = true;
        entry.read = true;
        let line_number = matched.get("line_number").and_then(Value::as_u64);
        let line = matched
            .get("line")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let span_hash = stable_json_digest("psionic.legal_benchmark.coverage.evidence.v1", &line)?;
        let evidence_id = format!(
            "evidence.{}.{}",
            stable_path_id(relative_path),
            line_number.unwrap_or(0)
        );
        evidence_refs.push(EvidenceCoverageEntry {
            evidence_id: evidence_id.clone(),
            source_ref: relative_path.to_owned(),
            locator: line_number.map(|line| format!("line:{line}")),
            span_hash: span_hash.clone(),
        });
        facts.push(FactCoverageEntry {
            fact_id: format!("fact.{}.{}", stable_path_id(relative_path), facts.len()),
            source_ref: relative_path.to_owned(),
            text_hash: span_hash,
            method: String::from("grep"),
        });
    }
    Ok(())
}

fn apply_write_coverage(
    task_spec: &BenchmarkTaskSpec,
    call: &ToolCallRecord,
    deliverable_sections: &mut Vec<DeliverableSectionCoverage>,
) {
    let root = input_root(call);
    if !matches!(
        root,
        Some(LegalBenchmarkPathRoot::Output | LegalBenchmarkPathRoot::Workspace)
    ) {
        return;
    }
    let Some(relative_path) = input_payload(call)
        .and_then(|input| input.get("relative_path"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let deliverable_id = deliverable_id_for_path(task_spec, relative_path)
        .unwrap_or_else(|| format!("unmatched.{}", stable_path_id(relative_path)));
    deliverable_sections.push(DeliverableSectionCoverage {
        deliverable_id,
        relative_path: relative_path.to_owned(),
        section_id: format!("tool.{}.{}", call.tool_name, stable_path_id(relative_path)),
        drafted: call.error_kind.is_none(),
    });
}

pub(crate) fn build_deliverable_validations(
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &ArtifactManifest,
) -> Vec<crate::ValidationCoverageEntry> {
    task_spec
        .deliverables
        .iter()
        .map(|deliverable| {
            let passed = output_manifest
                .artifacts
                .iter()
                .any(|artifact| artifact.relative_path == deliverable.required_path);
            ValidationCoverageEntry {
                validation_id: format!("validation.deliverable.{}", deliverable.deliverable_id),
                target_ref: deliverable.deliverable_id.clone(),
                passed,
                detail: if passed {
                    format!("deliverable {} exists", deliverable.required_path)
                } else {
                    format!("deliverable {} missing", deliverable.required_path)
                },
            }
        })
        .collect()
}

pub(crate) fn self_check_from_transcript_event(
    event: &TranscriptEvent,
) -> Option<SelfCheckCoverageEntry> {
    let content = event.content.as_ref()?;
    let lowered = content.to_ascii_lowercase();
    if !(lowered.contains("self-check") || lowered.contains("self check")) {
        return None;
    }
    Some(SelfCheckCoverageEntry {
        self_check_id: format!("self_check.{}", event.event_index),
        area: String::from("agent_declared"),
        outcome: content.chars().take(240).collect(),
    })
}

fn output_deliverable_sections(
    task_spec: &BenchmarkTaskSpec,
    artifacts: &[SourceArtifact],
) -> Vec<crate::DeliverableSectionCoverage> {
    artifacts
        .iter()
        .filter_map(|artifact| {
            deliverable_id_for_path(task_spec, &artifact.relative_path).map(|deliverable_id| {
                DeliverableSectionCoverage {
                    deliverable_id,
                    relative_path: artifact.relative_path.clone(),
                    section_id: format!("output_manifest.{}", artifact.artifact_id),
                    drafted: true,
                }
            })
        })
        .collect()
}

fn deliverable_id_for_path(task_spec: &BenchmarkTaskSpec, relative_path: &str) -> Option<String> {
    task_spec
        .deliverables
        .iter()
        .find(|deliverable| deliverable.required_path == relative_path)
        .map(|deliverable| deliverable.deliverable_id.clone())
}

fn document_entry<'a>(
    documents: &'a mut BTreeMap<String, DocumentCoverageEntry>,
    relative_path: &str,
) -> &'a mut DocumentCoverageEntry {
    documents
        .entry(relative_path.to_owned())
        .or_insert_with(|| DocumentCoverageEntry {
            artifact_id: relative_path.to_owned(),
            relative_path: relative_path.to_owned(),
            discovered: false,
            read: false,
            used_extracted_text: false,
        })
}

fn input_root_is_documents(call: &ToolCallRecord) -> bool {
    matches!(input_root(call), Some(LegalBenchmarkPathRoot::Documents))
}

fn input_root(call: &ToolCallRecord) -> Option<LegalBenchmarkPathRoot> {
    match input_payload(call)?.get("root").and_then(Value::as_str)? {
        "documents" => Some(LegalBenchmarkPathRoot::Documents),
        "workspace" => Some(LegalBenchmarkPathRoot::Workspace),
        "output" => Some(LegalBenchmarkPathRoot::Output),
        _ => None,
    }
}

fn input_payload(call: &ToolCallRecord) -> Option<&Value> {
    call.input.get("input").or(Some(&call.input))
}

fn output_payload<'a>(call: &'a ToolCallRecord, expected_tool: &str) -> Option<&'a Value> {
    let output = call.output.as_ref()?;
    if output.get("tool").and_then(Value::as_str)? != expected_tool {
        return None;
    }
    output.get("output")
}

fn read_source_refs(coverage: &CoverageSnapshot) -> BTreeSet<&str> {
    coverage
        .documents
        .iter()
        .filter(|entry| entry.read)
        .flat_map(|entry| [entry.artifact_id.as_str(), entry.relative_path.as_str()])
        .collect()
}

fn drafted_deliverable_ids(coverage: &CoverageSnapshot) -> BTreeSet<&str> {
    coverage
        .deliverable_sections
        .iter()
        .filter(|section| section.drafted)
        .map(|section| section.deliverable_id.as_str())
        .collect()
}

fn criterion_evidence_refs(
    expected_source_refs: &[String],
    coverage: &CoverageSnapshot,
) -> Vec<String> {
    if expected_source_refs.is_empty() {
        return coverage
            .evidence_refs
            .iter()
            .map(|evidence| evidence.evidence_id.clone())
            .collect();
    }
    coverage
        .evidence_refs
        .iter()
        .filter(|evidence| {
            expected_source_refs
                .iter()
                .any(|source| source == &evidence.source_ref)
        })
        .map(|evidence| evidence.evidence_id.clone())
        .collect()
}

fn expected_source_refs(
    task_spec: &BenchmarkTaskSpec,
    source_artifact_ids: &[String],
) -> Vec<String> {
    let mut refs = source_artifact_ids.to_vec();
    for source_id in source_artifact_ids {
        if let Some(artifact) = task_spec
            .source_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == *source_id)
        {
            refs.push(artifact.relative_path.clone());
        }
    }
    refs
}

fn failure_diagnostic(failure_class: CriterionFailureClass) -> String {
    match failure_class {
        CriterionFailureClass::CoverageGap => {
            String::from("missed criterion aligns to unread required source artifacts")
        }
        CriterionFailureClass::ExtractionGap => {
            String::from("source artifacts were read but no matching evidence was captured")
        }
        CriterionFailureClass::DraftingGap => {
            String::from("evidence was available but required deliverable drafting was incomplete")
        }
        CriterionFailureClass::ReasoningGap => {
            String::from("coverage was present, so remaining miss is classified as reasoning")
        }
        CriterionFailureClass::Passed => String::from("criterion passed"),
    }
}

fn stable_path_id(path: &str) -> String {
    path.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '.'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, ArtifactManifestRole, CriterionKind, DeliverableKind, DeliverableSpec,
        JudgeMode, JudgePolicy, RunMetrics, RunRecord, RunTerminalState, SourceArtifact,
        ToolPolicy, TranscriptEvent, TranscriptEventKind,
    };
    use serde_json::json;

    fn source_artifact() -> SourceArtifact {
        SourceArtifact {
            artifact_id: String::from("source.contract"),
            artifact_kind: ArtifactKind::SourceDocument,
            relative_path: String::from("contract.txt"),
            original_filename: String::from("contract.txt"),
            media_type: String::from("text/plain"),
            byte_size: 100,
            sha256: String::from("hash"),
            data_classification: crate::DataClassification::BenchmarkConfidential,
            provenance: None,
        }
    }

    fn task_spec() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.coverage.test"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            title: String::from("Coverage test"),
            instructions: String::from("Review the contract."),
            work_type: String::from("review"),
            tags: Vec::new(),
            source_artifacts: vec![source_artifact()],
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Memo"),
                required: true,
            }],
            criteria: vec![crate::CriterionSpec {
                criterion_id: String::from("criterion.source"),
                criterion_kind: CriterionKind::FactualAccuracy,
                description: String::from("Hidden source criterion"),
                weight_bps: Some(10_000),
                deliverable_ids: vec![String::from("memo")],
                source_artifact_ids: vec![String::from("source.contract")],
            }],
            judge_policy: JudgePolicy {
                mode: JudgeMode::Deterministic,
                provider: String::from("mock"),
                model: String::from("judge"),
                prompt_template_id: String::from("judge"),
                prompt_template_hash: String::from("hash"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("read"), String::from("write")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 4,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: Metadata::new(),
        }
    }

    fn run_config(mode: &str) -> RunConfig {
        let mut metadata = Metadata::new();
        metadata.insert(
            String::from("coverage_mode"),
            Value::String(mode.to_owned()),
        );
        metadata.insert(
            String::from("derived_checklist_items"),
            json!([{"item_id":"derived.inventory","prompt":"Inventory every source document before drafting.","source":"operator","agent_visible":true}]),
        );
        RunConfig {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_config_id: String::from("config"),
            provider: String::from("mock"),
            model: String::from("mock"),
            agent_protocol_version: String::from("v1"),
            tool_policy: task_spec().tool_policy,
            judge_policy: task_spec().judge_policy,
            random_seed: None,
            metadata,
        }
    }

    #[test]
    fn integrity_mode_hides_derived_checklist() {
        let checklist = agent_visible_checklist(&run_config("integrity"));
        assert_eq!(checklist.mode, CoverageMode::Integrity);
        assert!(checklist.items.is_empty());
        assert!(!checklist.hidden_criteria_visible);
    }

    #[test]
    fn hill_climb_mode_allows_policy_approved_checklist() {
        let checklist = agent_visible_checklist(&run_config("hill_climb"));
        assert_eq!(checklist.mode, CoverageMode::HillClimb);
        assert_eq!(checklist.items.len(), 1);
        assert_eq!(checklist.items[0].item_id, "derived.inventory");
    }

    #[test]
    fn coverage_snapshot_classifies_coverage_gap_without_hidden_hint() {
        let task = task_spec();
        let run_config = run_config("integrity");
        let output_manifest = ArtifactManifest {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            manifest_id: String::from("output"),
            task_id: task.task_id.clone(),
            task_version: task.task_version.clone(),
            manifest_role: ArtifactManifestRole::Output,
            artifacts: Vec::new(),
            metadata: Metadata::new(),
        };
        let transcript = vec![TranscriptEvent {
            event_index: 0,
            event_kind: TranscriptEventKind::User,
            role: Some(String::from("user")),
            content: Some(String::from("Review the contract.")),
            payload: None,
            timestamp_ms: 0,
        }];
        let snapshot =
            build_coverage_snapshot(&task, &run_config, &[], &transcript, &output_manifest)
                .expect("coverage");
        assert!(!snapshot.hidden_criteria_visible);
        let failures = classify_criterion_failures(
            &task,
            &[CriterionResult {
                criterion_id: String::from("criterion.source"),
                passed: false,
                verdict: CriterionVerdict::Fail,
                reasoning: String::from("missed"),
                evidence_refs: Vec::new(),
                judge_model: String::from("mock"),
                judge_prompt_hash: String::from("hash"),
                raw_response_hash: String::from("raw"),
                confidence_bps: None,
                judge_latency_ms: None,
                judge_cost_micro_usd: None,
            }],
            &snapshot,
        );
        assert_eq!(
            failures[0].failure_class,
            CriterionFailureClass::CoverageGap
        );
    }

    #[test]
    fn model_visible_detector_finds_hidden_criteria_leak() {
        let task = task_spec();
        let transcript = vec![TranscriptEvent {
            event_index: 0,
            event_kind: TranscriptEventKind::User,
            role: Some(String::from("user")),
            content: Some(String::from("Hidden source criterion")),
            payload: None,
            timestamp_ms: 0,
        }];
        assert!(model_visible_text_contains_hidden_criteria(
            &task,
            &transcript
        ));
    }

    #[test]
    fn grep_matches_count_as_read_evidence_for_document_coverage() {
        let task = task_spec();
        let run_config = run_config("integrity");
        let output_manifest = ArtifactManifest {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            manifest_id: String::from("output"),
            task_id: task.task_id.clone(),
            task_version: task.task_version.clone(),
            manifest_role: ArtifactManifestRole::Output,
            artifacts: Vec::new(),
            metadata: Metadata::new(),
        };
        let tool_calls = vec![ToolCallRecord {
            tool_call_id: String::from("call.grep.contract"),
            tool_name: String::from("grep"),
            call_event_index: 1,
            result_event_index: Some(2),
            input: json!({
                "root": "documents",
                "pattern": "assignment",
                "case_sensitive": false,
                "max_results": 10,
                "include_hidden": false
            }),
            output: Some(json!({
                "tool": "grep",
                "output": {
                    "matches": [{
                        "root": "documents",
                        "relative_path": "contract.txt",
                        "line_number": 7,
                        "line": "Assignment requires written consent."
                    }],
                    "binary_files_skipped": 0,
                    "truncated": false
                }
            })),
            error_kind: None,
            elapsed_ms: 3,
        }];
        let snapshot =
            build_coverage_snapshot(&task, &run_config, &tool_calls, &[], &output_manifest)
                .expect("coverage");

        assert_eq!(
            document_coverage_bps_from_snapshot(&task, &snapshot),
            10_000
        );
        assert_eq!(snapshot.evidence_refs.len(), 1);
        assert!(
            snapshot
                .documents
                .iter()
                .any(|entry| entry.relative_path == "contract.txt" && entry.read)
        );

        let failures = classify_criterion_failures(
            &task,
            &[CriterionResult {
                criterion_id: String::from("criterion.source"),
                passed: false,
                verdict: CriterionVerdict::Fail,
                reasoning: String::from("missed"),
                evidence_refs: Vec::new(),
                judge_model: String::from("mock"),
                judge_prompt_hash: String::from("hash"),
                raw_response_hash: String::from("raw"),
                confidence_bps: None,
                judge_latency_ms: None,
                judge_cost_micro_usd: None,
            }],
            &snapshot,
        );
        assert_ne!(
            failures[0].failure_class,
            CriterionFailureClass::CoverageGap
        );
    }

    #[allow(dead_code)]
    fn _run_record_compiles_with_coverage(snapshot: CoverageSnapshot) -> RunRecord {
        RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run"),
            task_id: String::from("task"),
            task_version: String::from("v1"),
            input_artifact_manifest_hash: String::from("input"),
            run_config_hash: String::from("config"),
            output_artifact_manifest_hash: String::from("output"),
            terminal_state: RunTerminalState::Submitted,
            transcript: Vec::new(),
            tool_calls: Vec::new(),
            metrics: RunMetrics {
                model_turns: 0,
                tool_call_count: 0,
                input_tokens: 0,
                output_tokens: 0,
                wall_time_ms: 0,
                estimated_cost_micro_usd: 0,
            },
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: Some(snapshot),
            metadata: Metadata::new(),
        }
    }
}
