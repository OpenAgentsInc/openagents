use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for Harvey-compatible legal benchmark training records.
pub const LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.training_record.v1";

/// Stable schema version for a bundle of legal benchmark training records.
pub const LEGAL_BENCHMARK_TRAINING_RECORD_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.training_record_bundle.v1";

/// Visibility of a payload field or example to model training.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkTrainingVisibility {
    /// Safe to render into model-visible fine-tuning examples.
    ModelVisible,
    /// Retained only for judges, scoring, calibration, and release gates.
    JudgeOnly,
    /// Retained for provenance but excluded from model training.
    ExcludedFromTraining,
}

/// Hidden-criterion exposure posture for a training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkHiddenCriterionPolicy {
    /// No hidden criterion material is present in the source data.
    NotPresent,
    /// Hidden or rubric-adjacent material is present only in judge-only fields.
    JudgeOnlyExcluded,
    /// Only policy-approved derived checklist text is model-visible.
    DerivedChecklistModelVisible,
    /// Hidden criteria reached model-visible transcript content; model-visible
    /// examples must be excluded for this record.
    HiddenCriteriaVisible,
}

impl LegalBenchmarkHiddenCriterionPolicy {
    /// Returns whether model-visible training examples are allowed.
    #[must_use]
    pub const fn allows_model_visible_examples(self) -> bool {
        !matches!(self, Self::HiddenCriteriaVisible)
    }
}

/// Split label for one training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkTrainingSplit {
    /// Model-training split.
    Train,
    /// Development split used during iteration.
    Development,
    /// Holdout split that must not shape training decisions.
    Holdout,
    /// Retained public smoke slice used for honest first-pass score claims.
    RetainedSmoke,
    /// Judge-only/calibration split.
    JudgeOnly,
}

/// Fine-tuning/example class carried by one legal training example.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkTrainingExampleKind {
    /// Agent-visible task directive.
    TaskDirective,
    /// Tool-call and tool-result trace.
    ToolTrace,
    /// Evidence-backed final answer or deliverable.
    EvidenceBackedDraft,
    /// Corrected deliverable or revision target.
    CorrectedDeliverable,
    /// Negative or contrastive example.
    NegativeContrast,
    /// Self-check or validation trace.
    SelfCheck,
    /// Judge-only score or rubric rationale.
    JudgeRationale,
}

/// One tool invocation normalized for legal benchmark training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingToolInvocation {
    /// Stable tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Event index of the call in the source transcript.
    pub call_event_index: u64,
    /// Event index of the result in the source transcript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_event_index: Option<u64>,
    /// Tool input payload.
    pub input: Value,
    /// Tool output payload when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    /// Structured error kind when the call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// Evidence span or source citation retained by a legal training record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingEvidenceRef {
    /// Stable evidence id.
    pub evidence_id: String,
    /// Source artifact id, path, or external ref.
    pub source_ref: String,
    /// Optional locator such as page, line, row, or sheet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// Stable hash of the evidence span.
    pub span_hash: String,
}

/// One deliverable reference retained by a legal training record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingDeliverableRef {
    /// Stable deliverable id.
    pub deliverable_id: String,
    /// Output path or path pattern.
    pub relative_path: String,
    /// Whether this deliverable was required by the task.
    pub required: bool,
}

/// One model-training, judge-only, or excluded example derived from a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingExample {
    /// Stable example id within the record.
    pub example_id: String,
    /// Example kind.
    pub example_kind: LegalBenchmarkTrainingExampleKind,
    /// Visibility to training.
    pub visibility: LegalBenchmarkTrainingVisibility,
    /// Input text or structured text projection.
    pub input_text: String,
    /// Target text when the example has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_text: Option<String>,
    /// Evidence ids or artifact refs used by this example.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Reason this example is excluded from model training.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_reason: Option<String>,
}

/// Canonical legal benchmark training record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingRecord {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable record identifier.
    pub record_id: String,
    /// Benchmark suite id, for example `harvey_labs`.
    pub suite_id: String,
    /// Task id.
    pub task_id: String,
    /// Task version.
    pub task_version: String,
    /// Practice area or benchmark lane.
    pub practice_area: String,
    /// Work type or workflow class.
    pub work_type: String,
    /// Input/source artifact manifest digest.
    pub source_artifact_manifest_digest: String,
    /// Tool policy hash or stable policy id.
    pub tool_policy_hash: String,
    /// Ordered tool invocation rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_invocations: Vec<LegalBenchmarkTrainingToolInvocation>,
    /// Evidence references retained for provenance and training.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<LegalBenchmarkTrainingEvidenceRef>,
    /// Deliverables required or produced for this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deliverable_refs: Vec<LegalBenchmarkTrainingDeliverableRef>,
    /// Coverage snapshot digest or source ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_snapshot_ref: Option<String>,
    /// Score report id.
    pub score_report_ref: String,
    /// Stable digest of the source score report.
    pub score_report_digest: String,
    /// Failure-family labels derived from scoring and coverage comparison.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_family_labels: Vec<String>,
    /// Judge provenance rows such as model and prompt hashes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub judge_provenance: Vec<String>,
    /// Hidden-criterion policy for this record.
    pub hidden_criterion_policy: LegalBenchmarkHiddenCriterionPolicy,
    /// Split assigned to this record.
    pub split: LegalBenchmarkTrainingSplit,
    /// Derived examples.
    pub examples: Vec<LegalBenchmarkTrainingExample>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    /// Stable digest over this record.
    pub record_digest: String,
}

impl LegalBenchmarkTrainingRecord {
    /// Creates, validates, and digests one legal benchmark training record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_id: impl Into<String>,
        suite_id: impl Into<String>,
        task_id: impl Into<String>,
        task_version: impl Into<String>,
        practice_area: impl Into<String>,
        work_type: impl Into<String>,
        source_artifact_manifest_digest: impl Into<String>,
        tool_policy_hash: impl Into<String>,
        tool_invocations: Vec<LegalBenchmarkTrainingToolInvocation>,
        evidence_refs: Vec<LegalBenchmarkTrainingEvidenceRef>,
        deliverable_refs: Vec<LegalBenchmarkTrainingDeliverableRef>,
        coverage_snapshot_ref: Option<String>,
        score_report_ref: impl Into<String>,
        score_report_digest: impl Into<String>,
        failure_family_labels: Vec<String>,
        judge_provenance: Vec<String>,
        hidden_criterion_policy: LegalBenchmarkHiddenCriterionPolicy,
        split: LegalBenchmarkTrainingSplit,
        examples: Vec<LegalBenchmarkTrainingExample>,
        metadata: BTreeMap<String, Value>,
    ) -> Result<Self, LegalBenchmarkTrainingRecordError> {
        let mut record = Self {
            schema_version: String::from(LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION),
            record_id: record_id.into(),
            suite_id: suite_id.into(),
            task_id: task_id.into(),
            task_version: task_version.into(),
            practice_area: practice_area.into(),
            work_type: work_type.into(),
            source_artifact_manifest_digest: source_artifact_manifest_digest.into(),
            tool_policy_hash: tool_policy_hash.into(),
            tool_invocations,
            evidence_refs,
            deliverable_refs,
            coverage_snapshot_ref,
            score_report_ref: score_report_ref.into(),
            score_report_digest: score_report_digest.into(),
            failure_family_labels,
            judge_provenance,
            hidden_criterion_policy,
            split,
            examples,
            metadata,
            record_digest: String::new(),
        };
        record.canonicalize();
        record.validate()?;
        record.record_digest = record.stable_digest();
        Ok(record)
    }

    /// Validates field presence, uniqueness, and hidden-criterion policy.
    pub fn validate(&self) -> Result<(), LegalBenchmarkTrainingRecordError> {
        ensure_nonempty(self.schema_version.as_str(), "record.schema_version")?;
        if self.schema_version != LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION {
            return Err(LegalBenchmarkTrainingRecordError::SchemaVersionMismatch {
                expected: String::from(LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.record_id.as_str(), "record.record_id")?;
        ensure_nonempty(self.suite_id.as_str(), "record.suite_id")?;
        ensure_nonempty(self.task_id.as_str(), "record.task_id")?;
        ensure_nonempty(self.task_version.as_str(), "record.task_version")?;
        ensure_nonempty(self.practice_area.as_str(), "record.practice_area")?;
        ensure_nonempty(self.work_type.as_str(), "record.work_type")?;
        ensure_nonempty(
            self.source_artifact_manifest_digest.as_str(),
            "record.source_artifact_manifest_digest",
        )?;
        ensure_nonempty(self.tool_policy_hash.as_str(), "record.tool_policy_hash")?;
        ensure_nonempty(self.score_report_ref.as_str(), "record.score_report_ref")?;
        ensure_nonempty(
            self.score_report_digest.as_str(),
            "record.score_report_digest",
        )?;
        if self.examples.is_empty() {
            return Err(LegalBenchmarkTrainingRecordError::MissingField {
                field: String::from("record.examples"),
            });
        }

        let mut example_ids = BTreeSet::new();
        for example in &self.examples {
            validate_example(example)?;
            if !example_ids.insert(example.example_id.as_str()) {
                return Err(LegalBenchmarkTrainingRecordError::DuplicateExampleId {
                    example_id: example.example_id.clone(),
                });
            }
            if example.visibility == LegalBenchmarkTrainingVisibility::ModelVisible
                && !self.hidden_criterion_policy.allows_model_visible_examples()
            {
                return Err(
                    LegalBenchmarkTrainingRecordError::HiddenCriteriaVisibleToModel {
                        record_id: self.record_id.clone(),
                        example_id: example.example_id.clone(),
                    },
                );
            }
        }

        let mut tool_call_ids = BTreeSet::new();
        for invocation in &self.tool_invocations {
            ensure_nonempty(
                invocation.tool_call_id.as_str(),
                "tool_invocation.tool_call_id",
            )?;
            ensure_nonempty(invocation.tool_name.as_str(), "tool_invocation.tool_name")?;
            if !tool_call_ids.insert(invocation.tool_call_id.as_str()) {
                return Err(LegalBenchmarkTrainingRecordError::DuplicateToolCallId {
                    tool_call_id: invocation.tool_call_id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the record excluding `record_digest`.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut record = self.clone();
        record.record_digest.clear();
        stable_digest(b"psionic_legal_benchmark_training_record|", &record)
    }

    fn canonicalize(&mut self) {
        self.failure_family_labels.sort();
        self.failure_family_labels.dedup();
        self.judge_provenance.sort();
        self.judge_provenance.dedup();
        self.evidence_refs.sort();
        self.evidence_refs.dedup();
        self.deliverable_refs.sort();
        self.deliverable_refs.dedup();
        for example in &mut self.examples {
            example.source_refs.sort();
            example.source_refs.dedup();
        }
        self.examples
            .sort_by(|left, right| left.example_id.cmp(&right.example_id));
    }
}

/// Split-count row for a legal training record bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingSplitCount {
    /// Split label.
    pub split: LegalBenchmarkTrainingSplit,
    /// Number of records assigned to the split.
    pub record_count: u32,
}

/// Bundle of canonical legal benchmark training records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTrainingRecordBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Benchmark suite id.
    pub suite_id: String,
    /// Source report or campaign id.
    pub source_report_id: String,
    /// Stable digest over the source reports used to derive this bundle.
    pub source_report_digest: String,
    /// Split counts.
    pub split_counts: Vec<LegalBenchmarkTrainingSplitCount>,
    /// Canonical records.
    pub records: Vec<LegalBenchmarkTrainingRecord>,
    /// Plain-language claim boundary for the bundle.
    pub claim_boundary: String,
    /// Stable digest over this bundle.
    pub bundle_digest: String,
}

impl LegalBenchmarkTrainingRecordBundle {
    /// Creates, validates, sorts, and digests a training record bundle.
    pub fn new(
        bundle_id: impl Into<String>,
        suite_id: impl Into<String>,
        source_report_id: impl Into<String>,
        source_report_digest: impl Into<String>,
        records: Vec<LegalBenchmarkTrainingRecord>,
        claim_boundary: impl Into<String>,
    ) -> Result<Self, LegalBenchmarkTrainingRecordError> {
        let mut bundle = Self {
            schema_version: String::from(LEGAL_BENCHMARK_TRAINING_RECORD_BUNDLE_SCHEMA_VERSION),
            bundle_id: bundle_id.into(),
            suite_id: suite_id.into(),
            source_report_id: source_report_id.into(),
            source_report_digest: source_report_digest.into(),
            split_counts: Vec::new(),
            records,
            claim_boundary: claim_boundary.into(),
            bundle_digest: String::new(),
        };
        bundle.canonicalize();
        bundle.validate()?;
        bundle.bundle_digest = bundle.stable_digest();
        Ok(bundle)
    }

    /// Validates the bundle.
    pub fn validate(&self) -> Result<(), LegalBenchmarkTrainingRecordError> {
        ensure_nonempty(self.schema_version.as_str(), "bundle.schema_version")?;
        if self.schema_version != LEGAL_BENCHMARK_TRAINING_RECORD_BUNDLE_SCHEMA_VERSION {
            return Err(LegalBenchmarkTrainingRecordError::SchemaVersionMismatch {
                expected: String::from(LEGAL_BENCHMARK_TRAINING_RECORD_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.bundle_id.as_str(), "bundle.bundle_id")?;
        ensure_nonempty(self.suite_id.as_str(), "bundle.suite_id")?;
        ensure_nonempty(self.source_report_id.as_str(), "bundle.source_report_id")?;
        ensure_nonempty(
            self.source_report_digest.as_str(),
            "bundle.source_report_digest",
        )?;
        ensure_nonempty(self.claim_boundary.as_str(), "bundle.claim_boundary")?;
        if self.records.is_empty() {
            return Err(LegalBenchmarkTrainingRecordError::MissingField {
                field: String::from("bundle.records"),
            });
        }

        let mut record_ids = BTreeSet::new();
        for record in &self.records {
            record.validate()?;
            if !record_ids.insert(record.record_id.as_str()) {
                return Err(LegalBenchmarkTrainingRecordError::DuplicateRecordId {
                    record_id: record.record_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Returns a stable digest over the bundle excluding `bundle_digest`.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut bundle = self.clone();
        bundle.bundle_digest.clear();
        stable_digest(b"psionic_legal_benchmark_training_record_bundle|", &bundle)
    }

    fn canonicalize(&mut self) {
        self.records
            .sort_by(|left, right| left.record_id.cmp(&right.record_id));
        let mut counts = BTreeMap::new();
        for record in &self.records {
            *counts.entry(record.split).or_insert(0_u32) += 1;
        }
        self.split_counts = counts
            .into_iter()
            .map(|(split, record_count)| LegalBenchmarkTrainingSplitCount {
                split,
                record_count,
            })
            .collect();
    }
}

/// Errors raised while building legal benchmark training records.
#[derive(Debug, Error)]
pub enum LegalBenchmarkTrainingRecordError {
    /// Required field is empty or missing.
    #[error("missing required field `{field}`")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// Schema version mismatch.
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// Duplicate example id in a record.
    #[error("duplicate example id `{example_id}`")]
    DuplicateExampleId {
        /// Duplicate example id.
        example_id: String,
    },
    /// Duplicate tool call id in a record.
    #[error("duplicate tool call id `{tool_call_id}`")]
    DuplicateToolCallId {
        /// Duplicate tool call id.
        tool_call_id: String,
    },
    /// Duplicate record id in a bundle.
    #[error("duplicate training record id `{record_id}`")]
    DuplicateRecordId {
        /// Duplicate record id.
        record_id: String,
    },
    /// Hidden criterion content reached a model-visible example.
    #[error("record `{record_id}` has hidden criteria visible to model example `{example_id}`")]
    HiddenCriteriaVisibleToModel {
        /// Record id.
        record_id: String,
        /// Example id.
        example_id: String,
    },
}

fn validate_example(
    example: &LegalBenchmarkTrainingExample,
) -> Result<(), LegalBenchmarkTrainingRecordError> {
    ensure_nonempty(example.example_id.as_str(), "example.example_id")?;
    ensure_nonempty(example.input_text.as_str(), "example.input_text")?;
    if example.visibility == LegalBenchmarkTrainingVisibility::ExcludedFromTraining {
        ensure_nonempty(
            example.excluded_reason.as_deref().unwrap_or_default(),
            "example.excluded_reason",
        )?;
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &'static str,
) -> Result<(), LegalBenchmarkTrainingRecordError> {
    if value.trim().is_empty() {
        return Err(LegalBenchmarkTrainingRecordError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let payload = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_visible_example() -> LegalBenchmarkTrainingExample {
        LegalBenchmarkTrainingExample {
            example_id: String::from("example.task"),
            example_kind: LegalBenchmarkTrainingExampleKind::TaskDirective,
            visibility: LegalBenchmarkTrainingVisibility::ModelVisible,
            input_text: String::from("Review the agreement and identify renewal obligations."),
            target_text: Some(String::from("Use source-backed evidence.")),
            source_refs: vec![String::from("source.contract")],
            excluded_reason: None,
        }
    }

    fn record(
        hidden_criterion_policy: LegalBenchmarkHiddenCriterionPolicy,
        examples: Vec<LegalBenchmarkTrainingExample>,
    ) -> Result<LegalBenchmarkTrainingRecord, LegalBenchmarkTrainingRecordError> {
        LegalBenchmarkTrainingRecord::new(
            "legal.training.harvey.task_001.run_001",
            "harvey_labs",
            "task_001",
            "v1",
            "contracts",
            "review",
            "manifest-digest",
            "tool-policy-digest",
            Vec::new(),
            vec![LegalBenchmarkTrainingEvidenceRef {
                evidence_id: String::from("evidence.001"),
                source_ref: String::from("source.contract"),
                locator: Some(String::from("p.1")),
                span_hash: String::from("span-digest"),
            }],
            vec![LegalBenchmarkTrainingDeliverableRef {
                deliverable_id: String::from("memo"),
                relative_path: String::from("memo.md"),
                required: true,
            }],
            Some(String::from("coverage-digest")),
            "score.001",
            "score-digest",
            vec![String::from("coverage_gap")],
            vec![String::from("judge@prompt")],
            hidden_criterion_policy,
            LegalBenchmarkTrainingSplit::RetainedSmoke,
            examples,
            BTreeMap::new(),
        )
    }

    #[test]
    fn record_digest_is_deterministic_after_canonicalization() {
        let first = record(
            LegalBenchmarkHiddenCriterionPolicy::JudgeOnlyExcluded,
            vec![model_visible_example()],
        )
        .expect("record");
        let second = record(
            LegalBenchmarkHiddenCriterionPolicy::JudgeOnlyExcluded,
            vec![model_visible_example()],
        )
        .expect("record");
        assert_eq!(first.record_digest, second.record_digest);
        assert!(!first.record_digest.is_empty());
    }

    #[test]
    fn hidden_criteria_visible_blocks_model_visible_examples() {
        let error = record(
            LegalBenchmarkHiddenCriterionPolicy::HiddenCriteriaVisible,
            vec![model_visible_example()],
        )
        .expect_err("hidden criteria should block model-visible example");
        assert!(matches!(
            error,
            LegalBenchmarkTrainingRecordError::HiddenCriteriaVisibleToModel { .. }
        ));
    }

    #[test]
    fn hidden_criteria_visible_allows_excluded_examples_with_reason() {
        let mut example = model_visible_example();
        example.visibility = LegalBenchmarkTrainingVisibility::ExcludedFromTraining;
        example.excluded_reason = Some(String::from(
            "source transcript included hidden criterion material",
        ));
        let record = record(
            LegalBenchmarkHiddenCriterionPolicy::HiddenCriteriaVisible,
            vec![example],
        )
        .expect("excluded record");
        assert_eq!(
            record.hidden_criterion_policy,
            LegalBenchmarkHiddenCriterionPolicy::HiddenCriteriaVisible
        );
    }

    #[test]
    fn bundle_counts_splits_and_sorts_records() {
        let record = record(
            LegalBenchmarkHiddenCriterionPolicy::JudgeOnlyExcluded,
            vec![model_visible_example()],
        )
        .expect("record");
        let bundle = LegalBenchmarkTrainingRecordBundle::new(
            "legal.training.bundle.retained_smoke.v1",
            "harvey_labs",
            "report.retained_smoke",
            "source-report-digest",
            vec![record],
            "retained smoke records are dataset/eval fixtures, not a score claim",
        )
        .expect("bundle");
        assert_eq!(bundle.split_counts.len(), 1);
        assert_eq!(
            bundle.split_counts[0].split,
            LegalBenchmarkTrainingSplit::RetainedSmoke
        );
        assert!(!bundle.bundle_digest.is_empty());
    }
}
