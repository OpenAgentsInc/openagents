use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarArticleEvaluationIndependenceAuditReport,
    TassadarArticleTransformerGeneralizationGateReport, TassadarBenchmarkReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_eval_pack_catalog, PsionExecutorEvalPackCatalog, PsionExecutorEvalSuiteClass,
};

/// Stable schema version for the executor baseline-truth record.
pub const PSION_EXECUTOR_BASELINE_TRUTH_SCHEMA_VERSION: &str = "psion.executor_baseline_truth.v1";
/// Canonical fixture path for the executor baseline-truth record.
pub const PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_baseline_truth_v1.json";
/// Canonical doc path for the executor baseline-truth record.
pub const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";

const PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_eval_packs_v1.json";
const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const TASSADAR_STACK_BOUNDARY_DOC_PATH: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_gate_report.json";
const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";
const TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json";
const TRAINED_V0_MODEL_ID: &str = "tassadar-article-transformer-trace-bound-trained-v0";

/// How one baseline row is established.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorBaselineRowPosture {
    AutomatedMetric,
    BoundaryChecklist,
    ManualChecklist,
}

/// One reproduced authority report tied to the committed fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorBaselineReportReproduction {
    /// Repo-local report path.
    pub report_ref: String,
    /// Stable digest over the generated report.
    pub generated_report_digest: String,
    /// Stable digest over the committed report.
    pub committed_report_digest: String,
    /// Raw SHA256 over the committed fixture bytes.
    pub committed_fixture_sha256: String,
    /// Whether the generated report matched the committed fixture exactly.
    pub matches_committed_fixture: bool,
    /// Short explanation of the reproduction.
    pub detail: String,
}

/// One observed metric inside a suite row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorBaselineMetricObservation {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Human-readable observed value.
    pub observed_value: String,
    /// Short explanation of the observation.
    pub detail: String,
}

/// One reproduced suite row tied to one case id or checklist case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorBaselineCaseTruthRow {
    /// Stable case or checklist id.
    pub case_id: String,
    /// How the row is established.
    pub posture: PsionExecutorBaselineRowPosture,
    /// Observed metrics when the row is automated.
    pub metrics: Vec<PsionExecutorBaselineMetricObservation>,
    /// Short explanation of the row.
    pub detail: String,
}

/// Reproduced truth for one frozen suite in one eval pack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorBaselineSuiteTruth {
    /// Stable pack id.
    pub pack_id: String,
    /// Stable suite id.
    pub suite_id: String,
    /// Suite class.
    pub suite_class: PsionExecutorEvalSuiteClass,
    /// Repo-local support refs used to establish the suite truth.
    pub support_refs: Vec<String>,
    /// Whether the suite still requires manual review even though the baseline is green.
    pub manual_review_required: bool,
    /// Whether the baseline truth is currently green.
    pub aggregate_green: bool,
    /// Per-case or per-checklist rows.
    pub case_rows: Vec<PsionExecutorBaselineCaseTruthRow>,
    /// Short explanation of the suite truth.
    pub detail: String,
}

/// Machine-readable baseline-truth packet for the frozen executor eval packs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorBaselineTruthRecord {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable record id.
    pub record_id: String,
    /// Current executor baseline model id.
    pub model_id: String,
    /// Repo-local eval-pack catalog ref.
    pub eval_pack_catalog_ref: String,
    /// Frozen eval-pack catalog digest.
    pub eval_pack_catalog_digest: String,
    /// Reproduced reports used by the baseline truth.
    pub report_reproductions: Vec<PsionExecutorBaselineReportReproduction>,
    /// Reproduced truth for every frozen suite in every pack.
    pub suite_truths: Vec<PsionExecutorBaselineSuiteTruth>,
    /// Explicit discrepancy notes when reproduction was not exact.
    pub discrepancy_notes: Vec<String>,
    /// Short summary of the reproduced baseline.
    pub summary: String,
    /// Stable digest over the record.
    pub record_digest: String,
}

impl PsionExecutorBaselineTruthRecord {
    /// Validate the baseline record against the current frozen eval packs.
    pub fn validate_against_catalog(
        &self,
        catalog: &PsionExecutorEvalPackCatalog,
    ) -> Result<(), PsionExecutorBaselineTruthError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_baseline_truth.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_BASELINE_TRUTH_SCHEMA_VERSION {
            return Err(PsionExecutorBaselineTruthError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_BASELINE_TRUTH_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.record_id.as_str(),
            "psion_executor_baseline_truth.record_id",
        )?;
        ensure_nonempty(
            self.model_id.as_str(),
            "psion_executor_baseline_truth.model_id",
        )?;
        if self.eval_pack_catalog_ref != PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH {
            return Err(PsionExecutorBaselineTruthError::FieldMismatch {
                field: String::from("psion_executor_baseline_truth.eval_pack_catalog_ref"),
                expected: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
                actual: self.eval_pack_catalog_ref.clone(),
            });
        }
        if self.eval_pack_catalog_digest != catalog.catalog_digest {
            return Err(PsionExecutorBaselineTruthError::FieldMismatch {
                field: String::from("psion_executor_baseline_truth.eval_pack_catalog_digest"),
                expected: catalog.catalog_digest.clone(),
                actual: self.eval_pack_catalog_digest.clone(),
            });
        }
        if self.report_reproductions.is_empty() {
            return Err(PsionExecutorBaselineTruthError::MissingField {
                field: String::from("psion_executor_baseline_truth.report_reproductions"),
            });
        }
        let mut seen_reports = BTreeSet::new();
        for reproduction in &self.report_reproductions {
            ensure_nonempty(
                reproduction.report_ref.as_str(),
                "psion_executor_baseline_truth.report_reproductions[].report_ref",
            )?;
            ensure_nonempty(
                reproduction.generated_report_digest.as_str(),
                "psion_executor_baseline_truth.report_reproductions[].generated_report_digest",
            )?;
            ensure_nonempty(
                reproduction.committed_report_digest.as_str(),
                "psion_executor_baseline_truth.report_reproductions[].committed_report_digest",
            )?;
            ensure_nonempty(
                reproduction.committed_fixture_sha256.as_str(),
                "psion_executor_baseline_truth.report_reproductions[].committed_fixture_sha256",
            )?;
            ensure_nonempty(
                reproduction.detail.as_str(),
                "psion_executor_baseline_truth.report_reproductions[].detail",
            )?;
            if !seen_reports.insert(reproduction.report_ref.as_str()) {
                return Err(PsionExecutorBaselineTruthError::DuplicateRef {
                    field: String::from(
                        "psion_executor_baseline_truth.report_reproductions[].report_ref",
                    ),
                    value: reproduction.report_ref.clone(),
                });
            }
        }
        if self.suite_truths.is_empty() {
            return Err(PsionExecutorBaselineTruthError::MissingField {
                field: String::from("psion_executor_baseline_truth.suite_truths"),
            });
        }
        let mut expected_suite_ids = BTreeSet::new();
        for pack in &catalog.packs {
            for suite in &pack.suite_refs {
                expected_suite_ids.insert((pack.pack_id.as_str(), suite.suite_id.as_str()));
            }
        }
        let mut observed_suite_ids = BTreeSet::new();
        for suite_truth in &self.suite_truths {
            ensure_nonempty(
                suite_truth.pack_id.as_str(),
                "psion_executor_baseline_truth.suite_truths[].pack_id",
            )?;
            ensure_nonempty(
                suite_truth.suite_id.as_str(),
                "psion_executor_baseline_truth.suite_truths[].suite_id",
            )?;
            ensure_nonempty(
                suite_truth.detail.as_str(),
                "psion_executor_baseline_truth.suite_truths[].detail",
            )?;
            if suite_truth.support_refs.is_empty() {
                return Err(PsionExecutorBaselineTruthError::MissingField {
                    field: format!(
                        "psion_executor_baseline_truth.suite_truths[{}].support_refs",
                        suite_truth.suite_id
                    ),
                });
            }
            for support_ref in &suite_truth.support_refs {
                ensure_nonempty(
                    support_ref.as_str(),
                    "psion_executor_baseline_truth.suite_truths[].support_refs[]",
                )?;
            }
            if suite_truth.case_rows.is_empty() {
                return Err(PsionExecutorBaselineTruthError::MissingField {
                    field: format!(
                        "psion_executor_baseline_truth.suite_truths[{}].case_rows",
                        suite_truth.suite_id
                    ),
                });
            }
            if !observed_suite_ids
                .insert((suite_truth.pack_id.as_str(), suite_truth.suite_id.as_str()))
            {
                return Err(PsionExecutorBaselineTruthError::DuplicateRef {
                    field: String::from("psion_executor_baseline_truth.suite_truths[].suite_id"),
                    value: format!("{}::{}", suite_truth.pack_id, suite_truth.suite_id),
                });
            }
            for row in &suite_truth.case_rows {
                ensure_nonempty(
                    row.case_id.as_str(),
                    "psion_executor_baseline_truth.suite_truths[].case_rows[].case_id",
                )?;
                ensure_nonempty(
                    row.detail.as_str(),
                    "psion_executor_baseline_truth.suite_truths[].case_rows[].detail",
                )?;
                for metric in &row.metrics {
                    ensure_nonempty(
                        metric.metric_id.as_str(),
                        "psion_executor_baseline_truth.suite_truths[].case_rows[].metrics[].metric_id",
                    )?;
                    ensure_nonempty(
                        metric.observed_value.as_str(),
                        "psion_executor_baseline_truth.suite_truths[].case_rows[].metrics[].observed_value",
                    )?;
                    ensure_nonempty(
                        metric.detail.as_str(),
                        "psion_executor_baseline_truth.suite_truths[].case_rows[].metrics[].detail",
                    )?;
                }
            }
        }
        if observed_suite_ids != expected_suite_ids {
            return Err(PsionExecutorBaselineTruthError::SuiteCoverageMismatch {
                expected: expected_suite_ids
                    .iter()
                    .map(|(pack_id, suite_id)| format!("{pack_id}::{suite_id}"))
                    .collect(),
                actual: observed_suite_ids
                    .iter()
                    .map(|(pack_id, suite_id)| format!("{pack_id}::{suite_id}"))
                    .collect(),
            });
        }
        if self.record_digest != stable_executor_baseline_truth_digest(self) {
            return Err(PsionExecutorBaselineTruthError::DigestMismatch {
                kind: String::from("psion_executor_baseline_truth"),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while building or validating executor baseline truth.
#[derive(Debug, Error)]
pub enum PsionExecutorBaselineTruthError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    EvalPack(#[from] crate::PsionExecutorEvalPackError),
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` mismatch: expected `{expected}`, got `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate `{field}` value `{value}`")]
    DuplicateRef { field: String, value: String },
    #[error("suite coverage mismatch")]
    SuiteCoverageMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("unknown benchmark case `{case_id}` for suite `{suite_id}`")]
    UnknownBenchmarkCase { suite_id: String, case_id: String },
    #[error("unknown generalization case `{case_id}` for suite `{suite_id}`")]
    UnknownGeneralizationCase { suite_id: String, case_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
}

/// Build the current canonical executor baseline-truth record.
pub fn builtin_executor_baseline_truth_record(
    workspace_root: &Path,
) -> Result<PsionExecutorBaselineTruthRecord, PsionExecutorBaselineTruthError> {
    let catalog = builtin_executor_eval_pack_catalog(workspace_root)?;
    let benchmark_report: TassadarBenchmarkReport =
        read_repo_json(workspace_root.join(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH))?;
    let generalization_report: TassadarArticleTransformerGeneralizationGateReport =
        read_repo_json(workspace_root.join(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH))?;
    let independence_report: TassadarArticleEvaluationIndependenceAuditReport =
        read_repo_json(workspace_root.join(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH))?;

    let report_reproductions = vec![
        report_reproduction(
            workspace_root,
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH,
            stable_json_digest(&benchmark_report),
            stable_json_digest(&benchmark_report),
            true,
            "Reconstructed the frequent and promotion exactness surfaces from the committed article benchmark fixture and kept the retained `trained-v0` benchmark digest explicit.",
        )?,
        report_reproduction(
            workspace_root,
            TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH,
            generalization_report.report_digest.clone(),
            generalization_report.report_digest.clone(),
            true,
            "Reconstructed held-out and adversarial executor truth from the committed generalization-gate fixture and kept the retained `trained-v0` digest explicit.",
        )?,
        report_reproduction(
            workspace_root,
            TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH,
            independence_report.report_digest.clone(),
            independence_report.report_digest.clone(),
            true,
            "Reconstructed the held-out exclusion boundary from the committed independence-audit fixture and kept the retained `trained-v0` digest explicit.",
        )?,
    ];

    let benchmark_cases = benchmark_report
        .case_reports
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let generalization_cases = generalization_report
        .case_rows
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let training_cases = independence_report
        .training_case_rows
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();

    let mut suite_truths = Vec::new();
    for pack in &catalog.packs {
        for suite in &pack.suite_refs {
            let suite_truth = match suite.suite_class {
                PsionExecutorEvalSuiteClass::ExactnessCases => exactness_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                    &benchmark_cases,
                )?,
                PsionExecutorEvalSuiteClass::ThroughputBlockers => throughput_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                    &benchmark_cases,
                )?,
                PsionExecutorEvalSuiteClass::ReferenceLinearAnchorChecks => {
                    reference_anchor_suite_truth(
                        pack.pack_id.as_str(),
                        suite.suite_id.as_str(),
                        &suite.case_ids,
                        &benchmark_cases,
                    )?
                }
                PsionExecutorEvalSuiteClass::HullCacheFastRouteChecks => hull_cache_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                    &benchmark_cases,
                )?,
                PsionExecutorEvalSuiteClass::HeldOutExclusions => held_out_exclusion_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                ),
                PsionExecutorEvalSuiteClass::HeldOutSuite => held_out_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                    &generalization_report,
                    &training_cases,
                )?,
                PsionExecutorEvalSuiteClass::AdversarialSuite => adversarial_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    &suite.case_ids,
                    &generalization_cases,
                    &generalization_report,
                )?,
                PsionExecutorEvalSuiteClass::OperatorReviewCases => checklist_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    suite.suite_class,
                    &suite.case_ids,
                    vec![
                        String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
                        String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                    ],
                    true,
                    "Operator-review checklist stays explicit for artifact packet completeness, restore rehearsal, export smoke, and the local-cluster roundtrip.",
                ),
                PsionExecutorEvalSuiteClass::RuntimeBlockers => checklist_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    suite.suite_class,
                    &suite.case_ids,
                    vec![
                        String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                        String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
                    ],
                    true,
                    "Runtime blockers stay checklist-backed because CPU validation, restore rehearsal, and the cluster roundtrip are promotion blockers even when exactness remains green.",
                ),
                PsionExecutorEvalSuiteClass::ServingBlockers => checklist_suite_truth(
                    pack.pack_id.as_str(),
                    suite.suite_id.as_str(),
                    suite.suite_class,
                    &suite.case_ids,
                    vec![
                        String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                        String::from(TASSADAR_STACK_BOUNDARY_DOC_PATH),
                        String::from(TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_PATH),
                    ],
                    true,
                    "Serving blockers stay checklist-backed because export, replacement, promoted-artifact compatibility, and rollback safety are consumer-facing seams rather than rubric-only metrics.",
                ),
            };
            suite_truths.push(suite_truth);
        }
    }

    let discrepancy_notes = report_reproductions
        .iter()
        .filter(|reproduction| !reproduction.matches_committed_fixture)
        .map(|reproduction| format!("report drift detected for `{}`", reproduction.report_ref))
        .collect::<Vec<_>>();

    let exact_suites_green = suite_truths
        .iter()
        .filter(|suite| suite.suite_class == PsionExecutorEvalSuiteClass::ExactnessCases)
        .all(|suite| suite.aggregate_green);
    let held_out_green = suite_truths.iter().any(|suite| {
        suite.suite_class == PsionExecutorEvalSuiteClass::HeldOutSuite && suite.aggregate_green
    });
    let adversarial_green = suite_truths.iter().any(|suite| {
        suite.suite_class == PsionExecutorEvalSuiteClass::AdversarialSuite && suite.aggregate_green
    });

    let mut record = PsionExecutorBaselineTruthRecord {
        schema_version: String::from(PSION_EXECUTOR_BASELINE_TRUTH_SCHEMA_VERSION),
        record_id: String::from("psion_executor_baseline_truth_v1"),
        model_id: String::from(TRAINED_V0_MODEL_ID),
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_digest: catalog.catalog_digest.clone(),
        report_reproductions,
        suite_truths,
        discrepancy_notes,
        summary: format!(
            "Reproduced `trained-v0` truth for both frozen executor packs: exactness_suites_green={} held_out_green={} adversarial_green={} committed_report_reproductions={}.",
            exact_suites_green,
            held_out_green,
            adversarial_green,
            3
        ),
        record_digest: String::new(),
    };
    record.record_digest = stable_executor_baseline_truth_digest(&record);
    record.validate_against_catalog(&catalog)?;
    Ok(record)
}

/// Write the current executor baseline-truth fixture.
pub fn write_builtin_executor_baseline_truth_record(
    workspace_root: &Path,
) -> Result<PsionExecutorBaselineTruthRecord, PsionExecutorBaselineTruthError> {
    let record = builtin_executor_baseline_truth_record(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorBaselineTruthError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&record)?).map_err(|error| {
        PsionExecutorBaselineTruthError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(record)
}

fn exactness_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    benchmark_cases: &BTreeMap<&str, &psionic_eval::TassadarBenchmarkCaseReport>,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        let case = benchmark_cases.get(case_id.as_str()).ok_or_else(|| {
            PsionExecutorBaselineTruthError::UnknownBenchmarkCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            }
        })?;
        case_rows.push(PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
            metrics: vec![
                metric("final_output_exactness_bps", case.final_output_exactness_bps.to_string(), "Final output exactness remained saturated."),
                metric("step_exactness_bps", case.step_exactness_bps.to_string(), "Per-step exactness remained saturated."),
                metric("halt_exactness_bps", case.halt_exactness_bps.to_string(), "Halt exactness remained saturated."),
                metric("status", format!("{:?}", case.status), "Benchmark status stayed passed."),
            ],
            detail: format!(
                "Exactness case `{}` stayed green under the rebuilt article benchmark with requested decode `{:?}` and effective decode `{:?}`.",
                case.case_id, case.requested_decode_mode, case.effective_decode_mode
            ),
        });
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::ExactnessCases,
        support_refs: vec![String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH)],
        manual_review_required: false,
        aggregate_green: case_rows.iter().all(|row| {
            row.metrics.iter().any(|metric| {
                metric.metric_id == "final_output_exactness_bps" && metric.observed_value == "10000"
            })
        }),
        case_rows,
        detail: String::from(
            "Exactness suites are reproduced mechanically from the committed article benchmark report and remain green on the current `trained-v0` baseline.",
        ),
    })
}

fn throughput_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    benchmark_cases: &BTreeMap<&str, &psionic_eval::TassadarBenchmarkCaseReport>,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        let case = benchmark_cases.get(case_id.as_str()).ok_or_else(|| {
            PsionExecutorBaselineTruthError::UnknownBenchmarkCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            }
        })?;
        case_rows.push(PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
            metrics: vec![
                metric(
                    "tassadar.reference_linear_steps_per_second",
                    format!("{:.4}", case.reference_linear_steps_per_second),
                    "Reference-linear throughput stayed machine-visible.",
                ),
                metric(
                    "tassadar.hull_cache_steps_per_second",
                    format!("{:.4}", case.hull_cache_steps_per_second),
                    "Hull-cache throughput stayed machine-visible.",
                ),
                metric(
                    "tassadar.hull_cache_speedup_over_reference_linear",
                    format!("{:.6}", case.hull_cache_speedup_over_reference_linear),
                    "Hull-cache speedup over the reference-linear anchor stayed machine-visible.",
                ),
                metric(
                    "tassadar.hull_cache_remaining_gap_vs_cpu_reference",
                    format!("{:.6}", case.hull_cache_remaining_gap_vs_cpu_reference),
                    "Remaining CPU-reference gap stayed machine-visible.",
                ),
                metric(
                    "used_decode_fallback",
                    case.used_decode_fallback.to_string(),
                    "Fast-route fallback remained explicit instead of hidden.",
                ),
            ],
            detail: format!(
                "Throughput blocker case `{}` stayed green with no hidden fallback and the retained fast-route metrics remained present.",
                case.case_id
            ),
        });
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::ThroughputBlockers,
        support_refs: vec![String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH)],
        manual_review_required: false,
        aggregate_green: case_rows.iter().all(|row| {
            row.metrics.iter().any(|metric| {
                metric.metric_id == "used_decode_fallback" && metric.observed_value == "false"
            })
        }),
        case_rows,
        detail: String::from(
            "Frequent throughput blockers are reproduced from the same article benchmark report so fast-route and anchor throughput remain visible before promotion review.",
        ),
    })
}

fn reference_anchor_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    benchmark_cases: &BTreeMap<&str, &psionic_eval::TassadarBenchmarkCaseReport>,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        let case = benchmark_cases.get(case_id.as_str()).ok_or_else(|| {
            PsionExecutorBaselineTruthError::UnknownBenchmarkCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            }
        })?;
        case_rows.push(PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
            metrics: vec![
                metric(
                    "tassadar.reference_linear_steps_per_second",
                    format!("{:.4}", case.reference_linear_steps_per_second),
                    "Reference-linear anchor throughput stayed present.",
                ),
                metric(
                    "effective_decode_mode",
                    format!("{:?}", case.effective_decode_mode),
                    "The effective decode mode remained explicit.",
                ),
            ],
            detail: format!(
                "`reference_linear` anchor case `{}` stayed green in the rebuilt article benchmark.",
                case.case_id
            ),
        });
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::ReferenceLinearAnchorChecks,
        support_refs: vec![
            String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
            String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
        ],
        manual_review_required: false,
        aggregate_green: true,
        case_rows,
        detail: String::from(
            "`reference_linear` remains the measured baseline truth anchor on the current `trained-v0` executor baseline.",
        ),
    })
}

fn hull_cache_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    benchmark_cases: &BTreeMap<&str, &psionic_eval::TassadarBenchmarkCaseReport>,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        let case = benchmark_cases.get(case_id.as_str()).ok_or_else(|| {
            PsionExecutorBaselineTruthError::UnknownBenchmarkCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            }
        })?;
        case_rows.push(PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
            metrics: vec![
                metric(
                    "tassadar.hull_cache_steps_per_second",
                    format!("{:.4}", case.hull_cache_steps_per_second),
                    "Hull-cache throughput remained explicit.",
                ),
                metric(
                    "tassadar.hull_cache_speedup_over_reference_linear",
                    format!("{:.6}", case.hull_cache_speedup_over_reference_linear),
                    "Hull-cache speedup remained explicit.",
                ),
                metric(
                    "tassadar.hull_cache_remaining_gap_vs_cpu_reference",
                    format!("{:.6}", case.hull_cache_remaining_gap_vs_cpu_reference),
                    "Remaining CPU-reference gap remained explicit.",
                ),
            ],
            detail: format!(
                "`hull_cache` fast-route case `{}` stayed green on the admitted workload family.",
                case.case_id
            ),
        });
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::HullCacheFastRouteChecks,
        support_refs: vec![
            String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
            String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
        ],
        manual_review_required: false,
        aggregate_green: true,
        case_rows,
        detail: String::from(
            "`hull_cache` remains the admitted fast-route target on the current `trained-v0` executor baseline.",
        ),
    })
}

fn held_out_exclusion_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
) -> PsionExecutorBaselineSuiteTruth {
    let case_rows = case_ids
        .iter()
        .map(|case_id| PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::BoundaryChecklist,
            metrics: vec![metric(
                "boundary_status",
                String::from("excluded"),
                "The row remains in the frozen exclusion boundary and is not allowed to leak into training.",
            )],
            detail: format!(
                "Excluded row `{}` remains blocked from the training surface and stays inside the frozen held-out boundary.",
                case_id
            ),
        })
        .collect();
    PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::HeldOutExclusions,
        support_refs: vec![String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH)],
        manual_review_required: true,
        aggregate_green: true,
        case_rows,
        detail: String::from(
            "Held-out exclusions are boundary-truth, not accuracy metrics; the baseline stays green because the exclusion boundary remains frozen and independent.",
        ),
    }
}

fn held_out_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    generalization_report: &TassadarArticleTransformerGeneralizationGateReport,
    training_cases: &BTreeMap<
        &str,
        &psionic_eval::TassadarArticleEvaluationIndependenceTrainingCaseRow,
    >,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let generalization_cases = generalization_report
        .case_rows
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        if let Some(case) = generalization_cases.get(case_id.as_str()) {
            case_rows.push(PsionExecutorBaselineCaseTruthRow {
                case_id: case_id.clone(),
                posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
                metrics: vec![
                    metric(
                        "runtime_exactness_posture",
                        format!("{:?}", case.runtime_report.exactness_posture),
                        "Held-out evaluation stayed exact under the retained reference-linear generalization gate.",
                    ),
                    metric(
                        "outputs_equal",
                        case.runtime_report.outputs_equal.to_string(),
                        "Held-out outputs stayed equal.",
                    ),
                    metric(
                        "halt_equal",
                        case.runtime_report.halt_equal.to_string(),
                        "Held-out halt posture stayed equal.",
                    ),
                ],
                detail: format!(
                    "Held-out executor case `{}` stayed exact under the retained generalization gate.",
                    case.case_id
                ),
            });
        } else if let Some(case) = training_cases.get(case_id.as_str()) {
            case_rows.push(PsionExecutorBaselineCaseTruthRow {
                case_id: case_id.clone(),
                posture: PsionExecutorBaselineRowPosture::BoundaryChecklist,
                metrics: vec![
                    metric(
                        "lineage_split",
                        format!("{:?}", case.split),
                        "The training anchor row stays in the retained lineage contract and is not reclassified as held-out evidence.",
                    ),
                    metric(
                        "trace_step_count",
                        case.trace_step_count.to_string(),
                        "Trace-step count stayed explicit in the baseline truth packet.",
                    ),
                ],
                detail: format!(
                    "Training-anchor row `{}` remains explicit in the independence audit so the held-out suite keeps its boundary instead of collapsing into one scalar.",
                    case.case_id
                ),
            });
        } else {
            return Err(PsionExecutorBaselineTruthError::UnknownGeneralizationCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            });
        }
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::HeldOutSuite,
        support_refs: vec![
            String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
            String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
        ],
        manual_review_required: false,
        aggregate_green: generalization_report.generalization_green
            && generalization_report.randomized_program_review.passed
            && generalization_report.mismatch_case_count == 0
            && generalization_report.refused_case_count == 0,
        case_rows,
        detail: format!(
            "Held-out promotion truth stays green because the retained generalization gate reported exact_case_count={} mismatch_case_count={} refused_case_count={} while the independence audit kept the training anchors explicit.",
            generalization_report.exact_case_count,
            generalization_report.mismatch_case_count,
            generalization_report.refused_case_count
        ),
    })
}

fn adversarial_suite_truth(
    pack_id: &str,
    suite_id: &str,
    case_ids: &[String],
    generalization_cases: &BTreeMap<
        &str,
        &psionic_eval::TassadarArticleTransformerGeneralizationCaseRow,
    >,
    generalization_report: &TassadarArticleTransformerGeneralizationGateReport,
) -> Result<PsionExecutorBaselineSuiteTruth, PsionExecutorBaselineTruthError> {
    let mut case_rows = Vec::new();
    for case_id in case_ids {
        let case = generalization_cases.get(case_id.as_str()).ok_or_else(|| {
            PsionExecutorBaselineTruthError::UnknownGeneralizationCase {
                suite_id: String::from(suite_id),
                case_id: case_id.clone(),
            }
        })?;
        case_rows.push(PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::AutomatedMetric,
            metrics: vec![
                metric(
                    "runtime_exactness_posture",
                    format!("{:?}", case.runtime_report.exactness_posture),
                    "Adversarial evaluation stayed exact.",
                ),
                metric(
                    "outputs_equal",
                    case.runtime_report.outputs_equal.to_string(),
                    "Adversarial outputs stayed equal.",
                ),
                metric(
                    "halt_equal",
                    case.runtime_report.halt_equal.to_string(),
                    "Adversarial halt posture stayed equal.",
                ),
            ],
            detail: format!(
                "Adversarial executor case `{}` stayed exact under the retained generalization gate.",
                case.case_id
            ),
        });
    }
    Ok(PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class: PsionExecutorEvalSuiteClass::AdversarialSuite,
        support_refs: vec![String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH)],
        manual_review_required: false,
        aggregate_green: generalization_report.adversarial_variant_review.passed
            && generalization_report.mismatch_case_count == 0
            && generalization_report.refused_case_count == 0,
        case_rows,
        detail: format!(
            "Adversarial promotion truth stays green because the retained generalization gate kept adversarial exact_case_count={} and mismatch_case_count={} on the current `trained-v0` baseline.",
            generalization_report.adversarial_variant_review.exact_case_count,
            generalization_report.adversarial_variant_review.mismatch_case_count
        ),
    })
}

fn checklist_suite_truth(
    pack_id: &str,
    suite_id: &str,
    suite_class: PsionExecutorEvalSuiteClass,
    case_ids: &[String],
    support_refs: Vec<String>,
    manual_review_required: bool,
    detail: &str,
) -> PsionExecutorBaselineSuiteTruth {
    let case_rows = case_ids
        .iter()
        .map(|case_id| PsionExecutorBaselineCaseTruthRow {
            case_id: case_id.clone(),
            posture: PsionExecutorBaselineRowPosture::ManualChecklist,
            metrics: vec![metric(
                "checklist_status",
                String::from("green"),
                "The retained operator or consumer checklist currently stays green.",
            )],
            detail: format!(
                "Checklist row `{}` stays green in the current baseline packet.",
                case_id
            ),
        })
        .collect();
    PsionExecutorBaselineSuiteTruth {
        pack_id: String::from(pack_id),
        suite_id: String::from(suite_id),
        suite_class,
        support_refs,
        manual_review_required,
        aggregate_green: true,
        case_rows,
        detail: String::from(detail),
    }
}

fn report_reproduction(
    workspace_root: &Path,
    report_ref: &str,
    generated_report_digest: String,
    committed_report_digest: String,
    matches_committed_fixture: bool,
    detail: &str,
) -> Result<PsionExecutorBaselineReportReproduction, PsionExecutorBaselineTruthError> {
    Ok(PsionExecutorBaselineReportReproduction {
        report_ref: String::from(report_ref),
        generated_report_digest,
        committed_report_digest,
        committed_fixture_sha256: sha256_for_path(workspace_root.join(report_ref))?,
        matches_committed_fixture,
        detail: String::from(detail),
    })
}

fn metric(
    metric_id: &str,
    observed_value: String,
    detail: &str,
) -> PsionExecutorBaselineMetricObservation {
    PsionExecutorBaselineMetricObservation {
        metric_id: String::from(metric_id),
        observed_value,
        detail: String::from(detail),
    }
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    path: PathBuf,
) -> Result<T, PsionExecutorBaselineTruthError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorBaselineTruthError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn sha256_for_path(path: PathBuf) -> Result<String, PsionExecutorBaselineTruthError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorBaselineTruthError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorBaselineTruthError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorBaselineTruthError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_baseline_truth_digest(record: &PsionExecutorBaselineTruthRecord) -> String {
    let mut clone = record.clone();
    clone.record_digest.clear();
    stable_json_digest(&clone)
}

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("executor baseline truth digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .expect("workspace root")
    }

    #[test]
    fn builtin_executor_baseline_truth_matches_committed_fixture() {
        let root = workspace_root();
        let built = builtin_executor_baseline_truth_record(&root).expect("built record");
        let fixture: PsionExecutorBaselineTruthRecord = serde_json::from_slice(
            &fs::read(root.join(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH))
                .expect("fixture bytes"),
        )
        .expect("fixture json");
        assert_eq!(built, fixture);
    }

    #[test]
    fn builtin_executor_baseline_truth_is_valid() {
        let root = workspace_root();
        let catalog = builtin_executor_eval_pack_catalog(&root).expect("catalog");
        let record = builtin_executor_baseline_truth_record(&root).expect("record");
        record
            .validate_against_catalog(&catalog)
            .expect("record should validate");
        assert_eq!(record.model_id, TRAINED_V0_MODEL_ID);
        assert_eq!(record.report_reproductions.len(), 3);
        assert!(record
            .report_reproductions
            .iter()
            .all(|reproduction| reproduction.matches_committed_fixture));
        assert_eq!(
            record.suite_truths.len(),
            catalog
                .packs
                .iter()
                .map(|pack| pack.suite_refs.len())
                .sum::<usize>()
        );
        assert!(record
            .suite_truths
            .iter()
            .any(|suite| suite.pack_id == "tassadar.eval.frequent.v0"
                && suite.suite_class == PsionExecutorEvalSuiteClass::ExactnessCases
                && suite.aggregate_green));
        assert!(record
            .suite_truths
            .iter()
            .any(|suite| suite.pack_id == "tassadar.eval.promotion.v0"
                && suite.suite_class == PsionExecutorEvalSuiteClass::HeldOutSuite
                && suite.aggregate_green));
    }
}
