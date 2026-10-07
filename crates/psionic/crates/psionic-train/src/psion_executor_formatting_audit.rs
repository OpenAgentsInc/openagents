use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_eval_pack_catalog,
    PsionExecutorBaselineTruthRecord, PsionExecutorEvalPackCatalog, PsionExecutorEvalPackKind,
    PsionExecutorEvalSuiteClass,
};

/// Stable schema version for the executor formatting audit.
pub const PSION_EXECUTOR_FORMATTING_AUDIT_SCHEMA_VERSION: &str =
    "psion.executor_formatting_audit.v1";
/// Canonical fixture path for the executor formatting audit.
pub const PSION_EXECUTOR_FORMATTING_AUDIT_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_formatting_audit_v1.json";
/// Canonical doc path for the executor formatting audit.
pub const PSION_EXECUTOR_FORMATTING_AUDIT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_FORMATTING_AUDIT.md";

const PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_eval_packs_v1.json";
const PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_baseline_truth_v1.json";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXCLUSION_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/psion/isolation/psion_exclusion_manifest_v1.json";
const TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_PATH: &str =
    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json";
const TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_gate_report.json";
const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";
const TASSADAR_STACK_BOUNDARY_DOC_PATH: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";

/// Prompt surface posture for one frozen eval family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorPromptSurface {
    ArticleBenchmarkPackage,
    GeneralizationGateCaseRows,
    ExclusionBoundaryManifest,
    ManualChecklist,
}

/// Normalization posture for one frozen eval family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorNormalizationPosture {
    ExactnessAndThroughputMetrics,
    RuntimeExactMismatchRefusalClassification,
    BoundaryMembershipOnly,
    ChecklistStatusOnly,
}

/// Post-processing posture for one frozen eval family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorPostProcessingPosture {
    EvaluatorOwnedMetrics,
    GeneralizationCaseClassification,
    BoundaryAuditOnly,
    ManualChecklistReview,
}

/// One manual operator review slice that complements the automated surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorManualReviewSlice {
    /// Stable review slice identifier.
    pub slice_id: String,
    /// Frozen suite ids that route into the slice.
    pub covered_suite_ids: Vec<String>,
    /// Operator-facing trigger condition.
    pub trigger_posture: String,
    /// Named reviewer responsibility.
    pub reviewer_role: String,
    /// Repo-local checklist or contract ref.
    pub checklist_ref: String,
    /// Short explanation of the slice.
    pub detail: String,
}

/// One suite-level prompt-formatting and normalization audit entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorFormattingAuditEntry {
    /// Stable pack id.
    pub pack_id: String,
    /// Stable suite id.
    pub suite_id: String,
    /// Frozen suite class.
    pub suite_class: PsionExecutorEvalSuiteClass,
    /// Repo-local source ref.
    pub source_ref: String,
    /// Prompt surface posture.
    pub prompt_surface: PsionExecutorPromptSurface,
    /// How the prompt family is frozen.
    pub prompt_format_contract: String,
    /// Normalization posture.
    pub normalization_posture: PsionExecutorNormalizationPosture,
    /// How outputs are normalized.
    pub normalization_contract: String,
    /// Post-processing posture.
    pub post_processing_posture: PsionExecutorPostProcessingPosture,
    /// How post-processing is constrained.
    pub post_processing_contract: String,
    /// Whether the suite was checked in this audit.
    pub checked: bool,
    /// Manual review slice for families where automation is not enough.
    pub manual_review_slice_id: Option<String>,
    /// Support refs used by the audit entry.
    pub support_refs: Vec<String>,
    /// Short explanation of what was checked.
    pub detail: String,
}

/// Retained formatting audit for the frozen executor eval families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorFormattingAuditRecord {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable audit id.
    pub audit_id: String,
    /// Frozen eval-pack catalog ref.
    pub eval_pack_catalog_ref: String,
    /// Frozen eval-pack catalog digest.
    pub eval_pack_catalog_digest: String,
    /// Frozen baseline-truth ref.
    pub baseline_truth_ref: String,
    /// Frozen baseline-truth digest.
    pub baseline_truth_digest: String,
    /// Suite-level audit entries.
    pub entries: Vec<PsionExecutorFormattingAuditEntry>,
    /// Manual review slices that complement automation.
    pub manual_review_slices: Vec<PsionExecutorManualReviewSlice>,
    /// Any suite ids left unchecked.
    pub unchecked_suite_ids: Vec<String>,
    /// Short audit summary.
    pub summary: String,
    /// Stable digest over the audit record.
    pub audit_digest: String,
}

impl PsionExecutorFormattingAuditRecord {
    /// Validate the formatting audit against the frozen eval-pack catalog and baseline packet.
    pub fn validate_against_catalog(
        &self,
        catalog: &PsionExecutorEvalPackCatalog,
        baseline_truth: &PsionExecutorBaselineTruthRecord,
    ) -> Result<(), PsionExecutorFormattingAuditError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_formatting_audit.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_FORMATTING_AUDIT_SCHEMA_VERSION {
            return Err(PsionExecutorFormattingAuditError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_FORMATTING_AUDIT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.audit_id.as_str(),
            "psion_executor_formatting_audit.audit_id",
        )?;
        if self.eval_pack_catalog_ref != PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH {
            return Err(PsionExecutorFormattingAuditError::FieldMismatch {
                field: String::from("psion_executor_formatting_audit.eval_pack_catalog_ref"),
                expected: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
                actual: self.eval_pack_catalog_ref.clone(),
            });
        }
        if self.eval_pack_catalog_digest != catalog.catalog_digest {
            return Err(PsionExecutorFormattingAuditError::FieldMismatch {
                field: String::from("psion_executor_formatting_audit.eval_pack_catalog_digest"),
                expected: catalog.catalog_digest.clone(),
                actual: self.eval_pack_catalog_digest.clone(),
            });
        }
        if self.baseline_truth_ref != PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH {
            return Err(PsionExecutorFormattingAuditError::FieldMismatch {
                field: String::from("psion_executor_formatting_audit.baseline_truth_ref"),
                expected: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                actual: self.baseline_truth_ref.clone(),
            });
        }
        if self.baseline_truth_digest != baseline_truth.record_digest {
            return Err(PsionExecutorFormattingAuditError::FieldMismatch {
                field: String::from("psion_executor_formatting_audit.baseline_truth_digest"),
                expected: baseline_truth.record_digest.clone(),
                actual: self.baseline_truth_digest.clone(),
            });
        }
        if self.entries.is_empty() {
            return Err(PsionExecutorFormattingAuditError::MissingField {
                field: String::from("psion_executor_formatting_audit.entries"),
            });
        }
        if self.manual_review_slices.is_empty() {
            return Err(PsionExecutorFormattingAuditError::MissingField {
                field: String::from("psion_executor_formatting_audit.manual_review_slices"),
            });
        }
        if !self.unchecked_suite_ids.is_empty() {
            return Err(PsionExecutorFormattingAuditError::UncheckedSuites {
                suite_ids: self.unchecked_suite_ids.clone(),
            });
        }
        let valid_slice_ids = self
            .manual_review_slices
            .iter()
            .map(|slice| slice.slice_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut seen_slice_ids = BTreeSet::new();
        for slice in &self.manual_review_slices {
            ensure_nonempty(
                slice.slice_id.as_str(),
                "psion_executor_formatting_audit.manual_review_slices[].slice_id",
            )?;
            ensure_nonempty(
                slice.trigger_posture.as_str(),
                "psion_executor_formatting_audit.manual_review_slices[].trigger_posture",
            )?;
            ensure_nonempty(
                slice.reviewer_role.as_str(),
                "psion_executor_formatting_audit.manual_review_slices[].reviewer_role",
            )?;
            ensure_nonempty(
                slice.checklist_ref.as_str(),
                "psion_executor_formatting_audit.manual_review_slices[].checklist_ref",
            )?;
            ensure_nonempty(
                slice.detail.as_str(),
                "psion_executor_formatting_audit.manual_review_slices[].detail",
            )?;
            if slice.covered_suite_ids.is_empty() {
                return Err(PsionExecutorFormattingAuditError::MissingField {
                    field: format!(
                        "psion_executor_formatting_audit.manual_review_slices[{}].covered_suite_ids",
                        slice.slice_id
                    ),
                });
            }
            if !seen_slice_ids.insert(slice.slice_id.as_str()) {
                return Err(PsionExecutorFormattingAuditError::DuplicateRef {
                    field: String::from(
                        "psion_executor_formatting_audit.manual_review_slices[].slice_id",
                    ),
                    value: slice.slice_id.clone(),
                });
            }
        }

        let mut expected_suite_ids = BTreeSet::new();
        for pack in &catalog.packs {
            for suite in &pack.suite_refs {
                expected_suite_ids.insert((pack.pack_id.as_str(), suite.suite_id.as_str()));
            }
        }
        let mut observed_suite_ids = BTreeSet::new();
        for entry in &self.entries {
            ensure_nonempty(
                entry.pack_id.as_str(),
                "psion_executor_formatting_audit.entries[].pack_id",
            )?;
            ensure_nonempty(
                entry.suite_id.as_str(),
                "psion_executor_formatting_audit.entries[].suite_id",
            )?;
            ensure_nonempty(
                entry.source_ref.as_str(),
                "psion_executor_formatting_audit.entries[].source_ref",
            )?;
            ensure_nonempty(
                entry.prompt_format_contract.as_str(),
                "psion_executor_formatting_audit.entries[].prompt_format_contract",
            )?;
            ensure_nonempty(
                entry.normalization_contract.as_str(),
                "psion_executor_formatting_audit.entries[].normalization_contract",
            )?;
            ensure_nonempty(
                entry.post_processing_contract.as_str(),
                "psion_executor_formatting_audit.entries[].post_processing_contract",
            )?;
            ensure_nonempty(
                entry.detail.as_str(),
                "psion_executor_formatting_audit.entries[].detail",
            )?;
            if entry.support_refs.is_empty() {
                return Err(PsionExecutorFormattingAuditError::MissingField {
                    field: format!(
                        "psion_executor_formatting_audit.entries[{}].support_refs",
                        entry.suite_id
                    ),
                });
            }
            if !entry.checked {
                return Err(PsionExecutorFormattingAuditError::UncheckedSuites {
                    suite_ids: vec![entry.suite_id.clone()],
                });
            }
            if let Some(slice_id) = &entry.manual_review_slice_id {
                if !valid_slice_ids.contains(slice_id.as_str()) {
                    return Err(
                        PsionExecutorFormattingAuditError::UnknownManualReviewSlice {
                            suite_id: entry.suite_id.clone(),
                            slice_id: slice_id.clone(),
                        },
                    );
                }
            }
            if requires_manual_slice(entry.suite_class) && entry.manual_review_slice_id.is_none() {
                return Err(
                    PsionExecutorFormattingAuditError::MissingManualReviewSlice {
                        suite_id: entry.suite_id.clone(),
                    },
                );
            }
            if !observed_suite_ids.insert((entry.pack_id.as_str(), entry.suite_id.as_str())) {
                return Err(PsionExecutorFormattingAuditError::DuplicateRef {
                    field: String::from("psion_executor_formatting_audit.entries[].suite_id"),
                    value: format!("{}::{}", entry.pack_id, entry.suite_id),
                });
            }
        }
        if observed_suite_ids != expected_suite_ids {
            return Err(PsionExecutorFormattingAuditError::SuiteCoverageMismatch {
                expected: expected_suite_ids
                    .into_iter()
                    .map(|(pack_id, suite_id)| format!("{pack_id}::{suite_id}"))
                    .collect(),
                actual: observed_suite_ids
                    .into_iter()
                    .map(|(pack_id, suite_id)| format!("{pack_id}::{suite_id}"))
                    .collect(),
            });
        }
        if self.audit_digest != stable_executor_formatting_audit_digest(self) {
            return Err(PsionExecutorFormattingAuditError::DigestMismatch {
                kind: String::from("psion_executor_formatting_audit"),
            });
        }
        Ok(())
    }
}

/// Errors raised while building or validating the formatting audit.
#[derive(Debug, Error)]
pub enum PsionExecutorFormattingAuditError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected {expected}, found {actual}")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` mismatch: expected {expected}, found {actual}")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate `{field}` value `{value}`")]
    DuplicateRef { field: String, value: String },
    #[error("suite coverage mismatch: expected {expected:?}, found {actual:?}")]
    SuiteCoverageMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("unchecked suite ids remain: {suite_ids:?}")]
    UncheckedSuites { suite_ids: Vec<String> },
    #[error("suite `{suite_id}` is missing a manual review slice")]
    MissingManualReviewSlice { suite_id: String },
    #[error("suite `{suite_id}` references unknown manual review slice `{slice_id}`")]
    UnknownManualReviewSlice { suite_id: String, slice_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to serialize formatting audit: {0}")]
    Json(#[from] serde_json::Error),
    #[error("baseline truth generation failed: {0}")]
    BaselineTruth(#[from] crate::PsionExecutorBaselineTruthError),
    #[error("eval-pack catalog generation failed: {0}")]
    EvalPack(#[from] crate::PsionExecutorEvalPackError),
}

/// Build the current formatting audit from the frozen executor eval families.
pub fn builtin_executor_formatting_audit_record(
    workspace_root: &Path,
) -> Result<PsionExecutorFormattingAuditRecord, PsionExecutorFormattingAuditError> {
    let catalog = builtin_executor_eval_pack_catalog(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;
    let manual_review_slices = builtin_manual_review_slices();
    let mut entries = Vec::new();
    for pack in &catalog.packs {
        for suite in &pack.suite_refs {
            entries.push(audit_entry_for_suite(pack.pack_kind, &pack.pack_id, suite));
        }
    }
    let unchecked_suite_ids = entries
        .iter()
        .filter(|entry| !entry.checked)
        .map(|entry| format!("{}::{}", entry.pack_id, entry.suite_id))
        .collect::<Vec<_>>();
    let checked_count = entries.iter().filter(|entry| entry.checked).count();
    let manual_slice_count = entries
        .iter()
        .filter(|entry| entry.manual_review_slice_id.is_some())
        .count();
    let mut record = PsionExecutorFormattingAuditRecord {
        schema_version: String::from(PSION_EXECUTOR_FORMATTING_AUDIT_SCHEMA_VERSION),
        audit_id: String::from("psion_executor_formatting_audit_v1"),
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_digest: catalog.catalog_digest.clone(),
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest.clone(),
        entries,
        manual_review_slices,
        unchecked_suite_ids,
        summary: format!(
            "Audited prompt formatting, normalization, and post-processing across {} frozen executor suites; checked_count={} manual_review_boundaries={}.",
            catalog.packs.iter().map(|pack| pack.suite_refs.len()).sum::<usize>(),
            checked_count,
            manual_slice_count
        ),
        audit_digest: String::new(),
    };
    record.audit_digest = stable_executor_formatting_audit_digest(&record);
    record.validate_against_catalog(&catalog, &baseline_truth)?;
    Ok(record)
}

/// Write the committed formatting-audit fixture.
pub fn write_builtin_executor_formatting_audit_record(
    workspace_root: &Path,
) -> Result<PsionExecutorFormattingAuditRecord, PsionExecutorFormattingAuditError> {
    let record = builtin_executor_formatting_audit_record(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_FORMATTING_AUDIT_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorFormattingAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&record)?).map_err(|error| {
        PsionExecutorFormattingAuditError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(record)
}

fn builtin_manual_review_slices() -> Vec<PsionExecutorManualReviewSlice> {
    vec![
        PsionExecutorManualReviewSlice {
            slice_id: String::from("held_out_boundary_review_v0"),
            covered_suite_ids: vec![String::from("frequent_held_out_exclusions_v0")],
            trigger_posture: String::from(
                "Run manual review whenever exclusion-manifest membership changes or a blocked row tries to enter training or replay.",
            ),
            reviewer_role: String::from("psion_executor_owner"),
            checklist_ref: String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            detail: String::from(
                "Boundary rows are not benchmark outputs. They stay trustworthy only if the exclusion manifest and training boundary are reviewed explicitly.",
            ),
        },
        PsionExecutorManualReviewSlice {
            slice_id: String::from("operator_readiness_review_v0"),
            covered_suite_ids: vec![String::from("frequent_operator_review_cases_v0")],
            trigger_posture: String::from(
                "Run manual review before any decision-grade run exits the checkpoint-time pack.",
            ),
            reviewer_role: String::from("psion_executor_operator"),
            checklist_ref: String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            detail: String::from(
                "Artifact packet completeness, checkpoint restore rehearsal, export smoke, and local-cluster roundtrip stay manual because they depend on shipped operator surfaces rather than one scalar metric.",
            ),
        },
        PsionExecutorManualReviewSlice {
            slice_id: String::from("generalization_red_case_review_v0"),
            covered_suite_ids: vec![
                String::from("promotion_held_out_suite_v0"),
                String::from("promotion_adversarial_suite_v0"),
            ],
            trigger_posture: String::from(
                "Route into manual review whenever generalization classification reports mismatch, refusal, or unexpected digest drift.",
            ),
            reviewer_role: String::from("psion_executor_reviewer"),
            checklist_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            detail: String::from(
                "Held-out and adversarial automation can say exact, mismatch, or refusal. Red rows require manual review instead of silent post-processing.",
            ),
        },
        PsionExecutorManualReviewSlice {
            slice_id: String::from("promotion_runtime_blocker_review_v0"),
            covered_suite_ids: vec![String::from("promotion_runtime_blockers_v0")],
            trigger_posture: String::from(
                "Run manual review on CPU-matrix validation, restore rehearsal, and local-cluster truth before promotion.",
            ),
            reviewer_role: String::from("psion_executor_runtime_owner"),
            checklist_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            detail: String::from(
                "Runtime blockers are owned checklists. They are not inferred from benchmark exactness or throughput.",
            ),
        },
        PsionExecutorManualReviewSlice {
            slice_id: String::from("promotion_serving_blocker_review_v0"),
            covered_suite_ids: vec![String::from("promotion_serving_blockers_v0")],
            trigger_posture: String::from(
                "Run manual review on export, replacement, promoted-artifact compatibility, and rollback safety before promotion.",
            ),
            reviewer_role: String::from("psion_executor_consumer_owner"),
            checklist_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            detail: String::from(
                "Serving blockers bridge executor artifacts into consumer seams. They stay explicit manual review slices until those seams are fully machine-audited.",
            ),
        },
    ]
}

fn audit_entry_for_suite(
    pack_kind: PsionExecutorEvalPackKind,
    pack_id: &str,
    suite: &crate::PsionExecutorEvalSuiteRef,
) -> PsionExecutorFormattingAuditEntry {
    match suite.suite_class {
        PsionExecutorEvalSuiteClass::ExactnessCases => {
            let (source_ref, prompt_contract) = if pack_kind == PsionExecutorEvalPackKind::Frequent {
                (
                    String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                    String::from(
                        "Use the frozen article benchmark package rows exactly as committed for the admitted executor workload family; no ad hoc prompt wrapper is allowed.",
                    ),
                )
            } else {
                (
                    String::from(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_PATH),
                    String::from(
                        "Use the frozen promotion package-set rows exactly as committed, preserving the admitted executor workload family and evaluator-owned prompt bytes.",
                    ),
                )
            };
            PsionExecutorFormattingAuditEntry {
                pack_id: String::from(pack_id),
                suite_id: suite.suite_id.clone(),
                suite_class: suite.suite_class,
                source_ref,
                prompt_surface: PsionExecutorPromptSurface::ArticleBenchmarkPackage,
                prompt_format_contract: prompt_contract,
                normalization_posture:
                    PsionExecutorNormalizationPosture::ExactnessAndThroughputMetrics,
                normalization_contract: String::from(
                    "Normalize only through evaluator-owned exactness metrics (`final_output_exactness_bps`, `step_exactness_bps`, `halt_exactness_bps`) and explicit decode/fallback facts. Do not trim or rewrite candidate outputs before scoring.",
                ),
                post_processing_posture: PsionExecutorPostProcessingPosture::EvaluatorOwnedMetrics,
                post_processing_contract: String::from(
                    "Post-processing is limited to evaluator-owned metric aggregation. There is no freeform answer cleanup layer on the exactness suites.",
                ),
                checked: true,
                manual_review_slice_id: None,
                support_refs: vec![
                    String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
                    String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                    String::from(TASSADAR_STACK_BOUNDARY_DOC_PATH),
                ],
                detail: String::from(
                    "Exactness suites stay trustworthy because prompt bytes, scoring metrics, and decode identity all remain evaluator-owned and explicit.",
                ),
            }
        }
        PsionExecutorEvalSuiteClass::HeldOutExclusions => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(PSION_EXCLUSION_MANIFEST_FIXTURE_PATH),
            prompt_surface: PsionExecutorPromptSurface::ExclusionBoundaryManifest,
            prompt_format_contract: String::from(
                "This suite does not run prompts. It freezes exclusion-manifest membership for blocked rows that must never silently re-enter the executor training or replay surface.",
            ),
            normalization_posture: PsionExecutorNormalizationPosture::BoundaryMembershipOnly,
            normalization_contract: String::from(
                "Normalize only by explicit manifest membership and lineage status. There is no output-text normalization for excluded rows.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::BoundaryAuditOnly,
            post_processing_contract: String::from(
                "Post-processing is boundary review only: confirm the row stays excluded and that no training or replay path absorbed it.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("held_out_boundary_review_v0")),
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            ],
            detail: String::from(
                "Held-out exclusions stay manual because the important truth is boundary membership, not model output.",
            ),
        },
        PsionExecutorEvalSuiteClass::OperatorReviewCases => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            prompt_surface: PsionExecutorPromptSurface::ManualChecklist,
            prompt_format_contract: String::from(
                "This suite freezes operator-owned checklist cases rather than prompts: artifact packet completeness, checkpoint restore rehearsal, export smoke, and local-cluster roundtrip.",
            ),
            normalization_posture: PsionExecutorNormalizationPosture::ChecklistStatusOnly,
            normalization_contract: String::from(
                "Normalize only through explicit checklist status (`green` / `red`) and retained operator evidence. No benchmark text normalization is involved.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::ManualChecklistReview,
            post_processing_contract: String::from(
                "Post-processing is manual checklist review against the admitted local-profile contract and shipped operator surfaces.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("operator_readiness_review_v0")),
            support_refs: vec![
                String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            ],
            detail: String::from(
                "Operator-review cases are deliberately manual because they prove artifact, restore, export, and roundtrip posture that scalar metrics cannot replace yet.",
            ),
        },
        PsionExecutorEvalSuiteClass::ThroughputBlockers => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
            prompt_surface: PsionExecutorPromptSurface::ArticleBenchmarkPackage,
            prompt_format_contract: String::from(
                "Use the same frozen article benchmark package rows as the frequent exactness suite so throughput comparisons stay on identical admitted prompts.",
            ),
            normalization_posture:
                PsionExecutorNormalizationPosture::ExactnessAndThroughputMetrics,
            normalization_contract: String::from(
                "Normalize only through retained throughput metrics (`reference_linear_steps_per_second`, `hull_cache_steps_per_second`, `hull_cache_speedup_over_reference_linear`, `hull_cache_remaining_gap_vs_cpu_reference`) plus explicit fallback identity.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::EvaluatorOwnedMetrics,
            post_processing_contract: String::from(
                "Post-processing is limited to evaluator-owned metric aggregation. No smoothing or hidden decode-mode fallback is allowed.",
            ),
            checked: true,
            manual_review_slice_id: None,
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            ],
            detail: String::from(
                "Throughput blockers stay machine-audited because they share the exact same prompt surface and evaluator-owned metrics as the benchmark report.",
            ),
        },
        PsionExecutorEvalSuiteClass::HeldOutSuite => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
            prompt_surface: PsionExecutorPromptSurface::GeneralizationGateCaseRows,
            prompt_format_contract: String::from(
                "Use the frozen generalization-gate case rows exactly as committed, preserving the held-out program payload and requested `reference_linear` decode path.",
            ),
            normalization_posture:
                PsionExecutorNormalizationPosture::RuntimeExactMismatchRefusalClassification,
            normalization_contract: String::from(
                "Normalize only through runtime classification (`exact`, `mismatch`, `refusal`) and explicit `outputs_equal` / `halt_equal` facts. No answer rewriting is allowed before classification.",
            ),
            post_processing_posture:
                PsionExecutorPostProcessingPosture::GeneralizationCaseClassification,
            post_processing_contract: String::from(
                "Post-processing is limited to generalization-case classification. Any mismatch, refusal, or digest drift routes into manual review instead of being normalized away.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("generalization_red_case_review_v0")),
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
                String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            ],
            detail: String::from(
                "Held-out suites stay trustworthy because prompt payload, decode identity, and exact/mismatch/refusal classification are all explicit and retained.",
            ),
        },
        PsionExecutorEvalSuiteClass::AdversarialSuite => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
            prompt_surface: PsionExecutorPromptSurface::GeneralizationGateCaseRows,
            prompt_format_contract: String::from(
                "Use the frozen adversarial generalization rows exactly as committed. Do not simplify or truncate article-scale hostile variants before evaluation.",
            ),
            normalization_posture:
                PsionExecutorNormalizationPosture::RuntimeExactMismatchRefusalClassification,
            normalization_contract: String::from(
                "Normalize only through retained runtime classification and explicit `outputs_equal` / `halt_equal` facts under the requested `reference_linear` decode path.",
            ),
            post_processing_posture:
                PsionExecutorPostProcessingPosture::GeneralizationCaseClassification,
            post_processing_contract: String::from(
                "Post-processing is limited to adversarial exact/mismatch/refusal classification. Any red result routes into manual review instead of cleanup heuristics.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("generalization_red_case_review_v0")),
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
                String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            ],
            detail: String::from(
                "Adversarial suites stay trustworthy because hostile article-scale prompts are scored through the same explicit generalization classifier instead of any fallback post-processing.",
            ),
        },
        PsionExecutorEvalSuiteClass::RuntimeBlockers => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            prompt_surface: PsionExecutorPromptSurface::ManualChecklist,
            prompt_format_contract: String::from(
                "This suite freezes runtime-owned checks rather than prompts: CPU-matrix validation, restore rehearsal, and local-cluster roundtrip truth.",
            ),
            normalization_posture: PsionExecutorNormalizationPosture::ChecklistStatusOnly,
            normalization_contract: String::from(
                "Normalize only through explicit checklist status and retained runtime evidence packets. No benchmark-output normalization is involved.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::ManualChecklistReview,
            post_processing_contract: String::from(
                "Post-processing is manual runtime review against the executor acceptance profile before any promotion attempt.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("promotion_runtime_blocker_review_v0")),
            support_refs: vec![
                String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            ],
            detail: String::from(
                "Runtime blockers stay manual because restore and cluster truth still cross device and operator boundaries that the evaluator does not fully own yet.",
            ),
        },
        PsionExecutorEvalSuiteClass::ServingBlockers => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            prompt_surface: PsionExecutorPromptSurface::ManualChecklist,
            prompt_format_contract: String::from(
                "This suite freezes consumer-facing checklist cases rather than prompts: export, replacement, promoted-artifact compatibility, and rollback safety.",
            ),
            normalization_posture: PsionExecutorNormalizationPosture::ChecklistStatusOnly,
            normalization_contract: String::from(
                "Normalize only through explicit checklist status and retained consumer-seam evidence. No answer-text cleanup or hidden serving fallback is allowed.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::ManualChecklistReview,
            post_processing_contract: String::from(
                "Post-processing is manual serving review so export, shadow, and rollback posture stay explicit before promotion.",
            ),
            checked: true,
            manual_review_slice_id: Some(String::from("promotion_serving_blocker_review_v0")),
            support_refs: vec![
                String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            ],
            detail: String::from(
                "Serving blockers stay manual because promoted artifacts must remain compatible with existing consumer seams and rollback rules.",
            ),
        },
        PsionExecutorEvalSuiteClass::ReferenceLinearAnchorChecks => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
            prompt_surface: PsionExecutorPromptSurface::ArticleBenchmarkPackage,
            prompt_format_contract: String::from(
                "Use the frozen article benchmark package rows exactly as committed for the admitted fast-route workload family; anchor checks must stay on the same prompts as fast-route comparisons.",
            ),
            normalization_posture:
                PsionExecutorNormalizationPosture::ExactnessAndThroughputMetrics,
            normalization_contract: String::from(
                "Normalize only through the retained `reference_linear` throughput metric and explicit decode-mode identity. There is no hidden route selection cleanup.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::EvaluatorOwnedMetrics,
            post_processing_contract: String::from(
                "Post-processing is limited to evaluator-owned metric aggregation so `reference_linear` stays the measured truth anchor.",
            ),
            checked: true,
            manual_review_slice_id: None,
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            ],
            detail: String::from(
                "`reference_linear` anchor checks stay machine-audited and cannot be rewritten into broader fast-route claims.",
            ),
        },
        PsionExecutorEvalSuiteClass::HullCacheFastRouteChecks => PsionExecutorFormattingAuditEntry {
            pack_id: String::from(pack_id),
            suite_id: suite.suite_id.clone(),
            suite_class: suite.suite_class,
            source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
            prompt_surface: PsionExecutorPromptSurface::ArticleBenchmarkPackage,
            prompt_format_contract: String::from(
                "Use the frozen article benchmark package rows exactly as committed so `hull_cache` fast-route claims stay on the admitted executor workload family only.",
            ),
            normalization_posture:
                PsionExecutorNormalizationPosture::ExactnessAndThroughputMetrics,
            normalization_contract: String::from(
                "Normalize only through retained `hull_cache` throughput metrics and explicit fast-route vs anchor comparisons. Hidden fallback is not allowed.",
            ),
            post_processing_posture: PsionExecutorPostProcessingPosture::EvaluatorOwnedMetrics,
            post_processing_contract: String::from(
                "Post-processing is limited to evaluator-owned metric aggregation so fast-route wins never override exactness or boundary failures.",
            ),
            checked: true,
            manual_review_slice_id: None,
            support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            ],
            detail: String::from(
                "`hull_cache` fast-route checks stay machine-audited and bounded to the admitted executor workload family.",
            ),
        },
    }
}

fn requires_manual_slice(suite_class: PsionExecutorEvalSuiteClass) -> bool {
    matches!(
        suite_class,
        PsionExecutorEvalSuiteClass::HeldOutExclusions
            | PsionExecutorEvalSuiteClass::OperatorReviewCases
            | PsionExecutorEvalSuiteClass::HeldOutSuite
            | PsionExecutorEvalSuiteClass::AdversarialSuite
            | PsionExecutorEvalSuiteClass::RuntimeBlockers
            | PsionExecutorEvalSuiteClass::ServingBlockers
    )
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorFormattingAuditError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorFormattingAuditError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_formatting_audit_digest(record: &PsionExecutorFormattingAuditRecord) -> String {
    let mut clone = record.clone();
    clone.audit_digest.clear();
    stable_json_digest(&clone)
}

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("executor formatting audit digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .expect("workspace root")
    }

    #[test]
    fn builtin_executor_formatting_audit_matches_committed_fixture() {
        let root = workspace_root();
        let built = builtin_executor_formatting_audit_record(&root).expect("built record");
        let fixture: PsionExecutorFormattingAuditRecord = serde_json::from_slice(
            &fs::read(root.join(PSION_EXECUTOR_FORMATTING_AUDIT_FIXTURE_PATH))
                .expect("fixture bytes"),
        )
        .expect("fixture json");
        assert_eq!(built, fixture);
    }

    #[test]
    fn builtin_executor_formatting_audit_is_valid_and_complete() {
        let root = workspace_root();
        let catalog = builtin_executor_eval_pack_catalog(&root).expect("catalog");
        let baseline = builtin_executor_baseline_truth_record(&root).expect("baseline");
        let record = builtin_executor_formatting_audit_record(&root).expect("record");
        record
            .validate_against_catalog(&catalog, &baseline)
            .expect("record should validate");
        assert!(record.unchecked_suite_ids.is_empty());
        assert_eq!(
            record.entries.len(),
            catalog
                .packs
                .iter()
                .map(|pack| pack.suite_refs.len())
                .sum::<usize>()
        );
        assert!(record.entries.iter().all(|entry| entry.checked));
        assert!(record.entries.iter().any(|entry| entry.suite_id
            == "frequent_operator_review_cases_v0"
            && entry.manual_review_slice_id.as_deref() == Some("operator_readiness_review_v0")));
        assert!(record
            .entries
            .iter()
            .any(|entry| entry.suite_id == "promotion_held_out_suite_v0"
                && entry.manual_review_slice_id.as_deref()
                    == Some("generalization_red_case_review_v0")));
    }
}
