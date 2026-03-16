use std::collections::{BTreeMap, BTreeSet};

use arc_core::{ArcBenchmark, ArcTask, ArcTaskId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcBenchmarkError, ArcExactMatchBenchmarkSummary, ArcExactMatchTaskReport, ArcStaticAnswerKey,
    ArcStaticTaskSubmission, score_exact_match_task,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcEvaluationVisibility {
    InternalHoldout,
    SyntheticRegression,
    PublicEval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcSyntheticDerivation {
    None,
    FromInternalHoldout,
    FromSyntheticRegression,
    FromPublicEval,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcPublicEvalArtifactManifest {
    pub schema_version: u16,
    pub artifact_id: String,
    pub benchmark_family: String,
    pub evaluation_visibility: ArcEvaluationVisibility,
    pub artifact_labels: Vec<String>,
    pub per_task_manual_tuning: bool,
    pub feeds_training: bool,
    pub synthetic_derivation: ArcSyntheticDerivation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcPublicEvalValidationResult {
    pub artifact_id: String,
    pub valid: bool,
    pub violations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcStaticHygieneCase {
    pub task: ArcTask,
    pub answer_key: ArcStaticAnswerKey,
    pub visibility: ArcEvaluationVisibility,
    pub synthetic_derivation: ArcSyntheticDerivation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_slices: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcStaticHygieneSuite {
    pub suite_id: String,
    pub benchmark: ArcBenchmark,
    pub cases: Vec<ArcStaticHygieneCase>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcVisibilitySummary {
    pub visibility: ArcEvaluationVisibility,
    pub total_tasks: u32,
    pub exact_match_tasks: u32,
    pub mean_task_score: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcConceptSliceSummary {
    pub concept_slice: String,
    pub total_tasks: u32,
    pub exact_match_tasks: u32,
    pub mean_task_score: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcStaticHygieneReport {
    pub suite_id: String,
    pub benchmark: ArcBenchmark,
    pub overall_summary: ArcExactMatchBenchmarkSummary,
    pub visibility_summaries: Vec<ArcVisibilitySummary>,
    pub concept_slice_summaries: Vec<ArcConceptSliceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_submission_tasks: Vec<ArcTaskId>,
    pub public_eval_validations: Vec<ArcPublicEvalValidationResult>,
}

pub fn validate_public_eval_artifact_manifest(
    manifest: &ArcPublicEvalArtifactManifest,
) -> ArcPublicEvalValidationResult {
    let mut violations = Vec::new();
    if manifest.schema_version != 1 {
        violations.push(String::from("schema_version must be 1"));
    }
    if manifest.artifact_id.trim().is_empty() {
        violations.push(String::from("artifact_id must not be empty"));
    }
    if manifest.benchmark_family != "arc" {
        violations.push(String::from("benchmark_family must be `arc`"));
    }

    let labels = manifest
        .artifact_labels
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    match manifest.evaluation_visibility {
        ArcEvaluationVisibility::PublicEval => {
            if !labels.contains("public-eval") {
                violations.push(String::from(
                    "public_eval artifacts must include the `public-eval` label",
                ));
            }
            if !labels.contains("non-regression") {
                violations.push(String::from(
                    "public_eval artifacts must include the `non-regression` label",
                ));
            }
            if !labels.contains("non-optimization") {
                violations.push(String::from(
                    "public_eval artifacts must include the `non-optimization` label",
                ));
            }
            if labels.contains("optimization") {
                violations.push(String::from(
                    "public_eval artifacts must not include the `optimization` label",
                ));
            }
            if manifest.per_task_manual_tuning {
                violations.push(String::from(
                    "public_eval artifacts must not allow per-task manual tuning",
                ));
            }
            if manifest.feeds_training {
                violations.push(String::from(
                    "public_eval artifacts must not feed training datasets",
                ));
            }
            if manifest.synthetic_derivation != ArcSyntheticDerivation::None {
                violations.push(String::from(
                    "public_eval artifacts must declare `synthetic_derivation = none`",
                ));
            }
        }
        _ => {
            if manifest.synthetic_derivation == ArcSyntheticDerivation::FromPublicEval {
                violations.push(String::from(
                    "non-public artifacts must not derive synthetic data from public_eval tasks",
                ));
            }
        }
    }

    ArcPublicEvalValidationResult {
        artifact_id: manifest.artifact_id.clone(),
        valid: violations.is_empty(),
        violations,
    }
}

pub fn run_static_hygiene_suite(
    suite: &ArcStaticHygieneSuite,
    submissions: &[ArcStaticTaskSubmission],
    artifact_manifests: &[ArcPublicEvalArtifactManifest],
) -> Result<ArcStaticHygieneReport, ArcBenchmarkHygieneError> {
    validate_static_suite(suite)?;

    let validations = artifact_manifests
        .iter()
        .map(validate_public_eval_artifact_manifest)
        .collect::<Vec<_>>();
    if let Some(invalid) = validations.iter().find(|result| !result.valid) {
        return Err(ArcBenchmarkHygieneError::PublicEvalArtifactViolation {
            artifact_id: invalid.artifact_id.clone(),
            violations: invalid.violations.clone(),
        });
    }

    let submissions_by_task = submissions
        .iter()
        .map(|submission| (submission.task_id.as_str().to_owned(), submission))
        .collect::<BTreeMap<_, _>>();
    let mut task_reports = Vec::with_capacity(suite.cases.len());
    let mut task_report_index = BTreeMap::new();
    let mut missing_submission_tasks = Vec::new();

    for case in &suite.cases {
        let empty_submission = ArcStaticTaskSubmission {
            task_id: case.task.id.clone(),
            test_pairs: Vec::new(),
        };
        let submission = if let Some(submission) = submissions_by_task.get(case.task.id.as_str()) {
            *submission
        } else {
            missing_submission_tasks.push(case.task.id.clone());
            &empty_submission
        };
        let report =
            score_exact_match_task(suite.benchmark, &case.task, &case.answer_key, submission)?;
        task_report_index.insert(case.task.id.as_str().to_owned(), report.clone());
        task_reports.push(report);
    }

    let overall_summary =
        ArcExactMatchBenchmarkSummary::from_task_reports(suite.benchmark, task_reports)?;
    let visibility_summaries = summarize_visibility(suite, &task_report_index)?;
    let concept_slice_summaries = summarize_concepts(suite, &task_report_index)?;

    Ok(ArcStaticHygieneReport {
        suite_id: suite.suite_id.clone(),
        benchmark: suite.benchmark,
        overall_summary,
        visibility_summaries,
        concept_slice_summaries,
        missing_submission_tasks,
        public_eval_validations: validations,
    })
}

#[derive(Debug, Error)]
pub enum ArcBenchmarkHygieneError {
    #[error("static hygiene suite currently supports ARC-AGI-1 and ARC-AGI-2 only, got {benchmark:?}")]
    UnsupportedBenchmark { benchmark: ArcBenchmark },
    #[error("static hygiene suite `{suite_id}` must include at least one case")]
    EmptySuite { suite_id: String },
    #[error(
        "internal hidden holdout task `{task_id}` must not carry synthetic derivation `{synthetic_derivation:?}`"
    )]
    HiddenHoldoutMustBeRaw {
        task_id: ArcTaskId,
        synthetic_derivation: ArcSyntheticDerivation,
    },
    #[error(
        "synthetic regression task `{task_id}` must derive from internal or synthetic sources, not `{synthetic_derivation:?}`"
    )]
    SyntheticRegressionDerivationMismatch {
        task_id: ArcTaskId,
        synthetic_derivation: ArcSyntheticDerivation,
    },
    #[error(
        "task `{task_id}` leaks public-eval lineage into `{visibility:?}` via `{synthetic_derivation:?}`"
    )]
    PublicEvalLeakage {
        task_id: ArcTaskId,
        visibility: ArcEvaluationVisibility,
        synthetic_derivation: ArcSyntheticDerivation,
    },
    #[error("public-eval artifact `{artifact_id}` violated policy: {violations:?}")]
    PublicEvalArtifactViolation {
        artifact_id: String,
        violations: Vec<String>,
    },
    #[error("exact-match scoring failed while running hygiene suite: {0}")]
    ExactMatch(#[from] ArcBenchmarkError),
}

fn validate_static_suite(suite: &ArcStaticHygieneSuite) -> Result<(), ArcBenchmarkHygieneError> {
    match suite.benchmark {
        ArcBenchmark::ArcAgi1 | ArcBenchmark::ArcAgi2 => {}
        benchmark => {
            return Err(ArcBenchmarkHygieneError::UnsupportedBenchmark { benchmark });
        }
    }
    if suite.cases.is_empty() {
        return Err(ArcBenchmarkHygieneError::EmptySuite {
            suite_id: suite.suite_id.clone(),
        });
    }

    for case in &suite.cases {
        match case.visibility {
            ArcEvaluationVisibility::InternalHoldout => {
                if case.synthetic_derivation != ArcSyntheticDerivation::None {
                    return Err(ArcBenchmarkHygieneError::HiddenHoldoutMustBeRaw {
                        task_id: case.task.id.clone(),
                        synthetic_derivation: case.synthetic_derivation,
                    });
                }
            }
            ArcEvaluationVisibility::SyntheticRegression => match case.synthetic_derivation {
                ArcSyntheticDerivation::FromInternalHoldout
                | ArcSyntheticDerivation::FromSyntheticRegression => {}
                other => {
                    return Err(
                        ArcBenchmarkHygieneError::SyntheticRegressionDerivationMismatch {
                            task_id: case.task.id.clone(),
                            synthetic_derivation: other,
                        },
                    );
                }
            },
            ArcEvaluationVisibility::PublicEval => {
                if case.synthetic_derivation != ArcSyntheticDerivation::None {
                    return Err(ArcBenchmarkHygieneError::PublicEvalLeakage {
                        task_id: case.task.id.clone(),
                        visibility: case.visibility,
                        synthetic_derivation: case.synthetic_derivation,
                    });
                }
            }
        }

        if case.visibility != ArcEvaluationVisibility::PublicEval
            && case.synthetic_derivation == ArcSyntheticDerivation::FromPublicEval
        {
            return Err(ArcBenchmarkHygieneError::PublicEvalLeakage {
                task_id: case.task.id.clone(),
                visibility: case.visibility,
                synthetic_derivation: case.synthetic_derivation,
            });
        }
    }

    Ok(())
}

fn summarize_visibility(
    suite: &ArcStaticHygieneSuite,
    reports: &BTreeMap<String, ArcExactMatchTaskReport>,
) -> Result<Vec<ArcVisibilitySummary>, ArcBenchmarkHygieneError> {
    let mut buckets = BTreeMap::<ArcEvaluationVisibility, Vec<ArcExactMatchTaskReport>>::new();
    for case in &suite.cases {
        buckets
            .entry(case.visibility)
            .or_default()
            .push(
                reports
                    .get(case.task.id.as_str())
                    .expect("report exists")
                    .clone(),
            );
    }

    buckets
        .into_iter()
        .map(|(visibility, bucket)| {
            let summary =
                ArcExactMatchBenchmarkSummary::from_task_reports(suite.benchmark, bucket)?;
            Ok(ArcVisibilitySummary {
                visibility,
                total_tasks: summary.total_tasks,
                exact_match_tasks: summary.exact_match_tasks,
                mean_task_score: summary.mean_task_score,
            })
        })
        .collect()
}

fn summarize_concepts(
    suite: &ArcStaticHygieneSuite,
    reports: &BTreeMap<String, ArcExactMatchTaskReport>,
) -> Result<Vec<ArcConceptSliceSummary>, ArcBenchmarkHygieneError> {
    let mut buckets = BTreeMap::<String, Vec<ArcExactMatchTaskReport>>::new();
    for case in &suite.cases {
        for concept in &case.concept_slices {
            buckets
                .entry(concept.clone())
                .or_default()
                .push(
                    reports
                        .get(case.task.id.as_str())
                        .expect("report exists")
                        .clone(),
                );
        }
    }

    let mut summaries = buckets
        .into_iter()
        .map(|(concept_slice, bucket)| {
            let summary =
                ArcExactMatchBenchmarkSummary::from_task_reports(suite.benchmark, bucket)?;
            Ok(ArcConceptSliceSummary {
                concept_slice,
                total_tasks: summary.total_tasks,
                exact_match_tasks: summary.exact_match_tasks,
                mean_task_score: summary.mean_task_score,
            })
        })
        .collect::<Result<Vec<_>, ArcBenchmarkHygieneError>>()?;
    summaries.sort_by(|left, right| left.concept_slice.cmp(&right.concept_slice));
    Ok(summaries)
}
