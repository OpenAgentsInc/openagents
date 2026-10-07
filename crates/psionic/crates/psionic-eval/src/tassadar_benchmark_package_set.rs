use std::{collections::BTreeSet, path::Path};

use psionic_data::{
    TassadarBenchmarkAxis, TassadarBenchmarkFamily, TassadarBenchmarkFamilyContract,
    TassadarBenchmarkPackageBinding, TassadarBenchmarkPackageSetContract,
    TassadarBenchmarkPackageSetError,
};
use psionic_environments::TassadarWorkloadTarget;
use psionic_runtime::TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_class_suite, build_tassadar_compiled_kernel_suite,
    build_tassadar_hungarian_10x10_suite, build_tassadar_hungarian_v0_suite,
    build_tassadar_reference_fixture_suite, build_tassadar_sudoku_9x9_suite, BenchmarkCase,
    BenchmarkPackage, TassadarBenchmarkError, TassadarCompiledKernelFamilyId,
    TassadarCompiledKernelSuiteEvalError, TassadarReferenceFixtureSuite,
};

/// Stable public benchmark-package-set reference for Tassadar.
pub const TASSADAR_BENCHMARK_PACKAGE_SET_REF: &str = "benchmark-set://openagents/tassadar/public";
const TASSADAR_BENCHMARK_PACKAGE_SET_SCHEMA_VERSION: u16 = 1;

/// Length-generalization posture captured by the package-set summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLengthGeneralizationPosture {
    /// Coverage remains bounded to small held-out or matched corpora.
    BoundedHoldout,
    /// Coverage reaches article-sized or larger matched workloads.
    ArticleScale,
    /// Coverage is explicitly about trace growth and horizon stress.
    ExplicitTraceStress,
}

/// Planner-usefulness posture captured by the package-set summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerUsefulnessPosture {
    /// Family is useful for planner or route selection on exact-compute tasks.
    RouteCandidate,
    /// Family is primarily a systems or scaling signal rather than a planner task.
    SystemsOnly,
}

/// Family-level summary row in the public package-set report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkPackageSetFamilySummary {
    /// Stable benchmark family.
    pub family: TassadarBenchmarkFamily,
    /// Human-readable family summary.
    pub summary: String,
    /// Stable benchmark package refs that cover the family today.
    pub benchmark_package_refs: Vec<String>,
    /// Seed case ids that currently cover the family.
    pub case_ids: Vec<String>,
    /// Exactness score in basis points for the public package set.
    pub exactness_score_bps: u32,
    /// Source refs that justify the exactness summary.
    pub exactness_support_refs: Vec<String>,
    /// Exactness summary text.
    pub exactness_summary: String,
    /// Length-generalization posture.
    pub length_generalization_posture: TassadarLengthGeneralizationPosture,
    /// Source refs that justify the generalization posture.
    pub length_generalization_support_refs: Vec<String>,
    /// Length-generalization summary text.
    pub length_generalization_summary: String,
    /// Planner-usefulness posture.
    pub planner_usefulness_posture: TassadarPlannerUsefulnessPosture,
    /// Source refs that justify the planner-usefulness posture.
    pub planner_usefulness_support_refs: Vec<String>,
    /// Planner-usefulness summary text.
    pub planner_usefulness_summary: String,
}

/// Repo-facing machine-readable summary of the public Tassadar benchmark package set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkPackageSetSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Underlying package-set contract.
    pub package_set: TassadarBenchmarkPackageSetContract,
    /// Support refs used to materialize the report.
    pub generated_from_refs: Vec<String>,
    /// Family-level summary rows.
    pub family_summaries: Vec<TassadarBenchmarkPackageSetFamilySummary>,
    /// Claim boundary for the report.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarBenchmarkPackageSetSummaryReport {
    fn new(
        package_set: TassadarBenchmarkPackageSetContract,
        family_summaries: Vec<TassadarBenchmarkPackageSetFamilySummary>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_BENCHMARK_PACKAGE_SET_SCHEMA_VERSION,
            package_set,
            generated_from_refs,
            family_summaries,
            claim_boundary: String::from(
                "this report summarizes the public Tassadar benchmark package set only; it separates exactness, length-generalization, and planner-usefulness, and it does not widen compiled, learned, or served capability claims beyond the cited package and artifact refs",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_benchmark_package_set_summary_report|",
            &report,
        );
        report
    }
}

/// Benchmark-package-set summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarBenchmarkPackageSetSummaryError {
    /// Shared benchmark helpers failed.
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    /// Package-set contract validation failed.
    #[error(transparent)]
    Contract(#[from] TassadarBenchmarkPackageSetError),
    /// Compiled-kernel-suite helpers failed.
    #[error(transparent)]
    CompiledKernelSuite(#[from] TassadarCompiledKernelSuiteEvalError),
    /// One benchmark package lacked a dataset binding.
    #[error("benchmark package `{benchmark_ref}` is missing a dataset binding")]
    MissingDatasetBinding {
        /// Benchmark reference.
        benchmark_ref: String,
    },
    /// One benchmark case was missing the expected workload classification metadata.
    #[error("benchmark case `{case_id}` is missing workload metadata")]
    MissingCaseMetadata {
        /// Benchmark case id.
        case_id: String,
    },
    /// Filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the public package-set contract for Tassadar benchmark families.
pub fn build_tassadar_benchmark_package_set_contract(
    version: &str,
) -> Result<TassadarBenchmarkPackageSetContract, TassadarBenchmarkPackageSetSummaryError> {
    let reference_suite = build_tassadar_reference_fixture_suite(version)?;
    let article_suite = build_tassadar_article_class_suite(version)?;
    let sudoku_9x9_suite = build_tassadar_sudoku_9x9_suite(version)?;
    let hungarian_v0_suite = build_tassadar_hungarian_v0_suite(version)?;
    let hungarian_10x10_suite = build_tassadar_hungarian_10x10_suite(version)?;
    let compiled_kernel_suite = build_tassadar_compiled_kernel_suite("v0")?;

    TassadarBenchmarkPackageSetContract::new(
        TASSADAR_BENCHMARK_PACKAGE_SET_REF,
        version,
        vec![
            TassadarBenchmarkFamilyContract {
                family: TassadarBenchmarkFamily::Arithmetic,
                summary: String::from(
                    "exact arithmetic kernels spanning the validation microprogram corpus and the compiled kernel suite",
                ),
                benchmark_packages: vec![
                    package_binding_for_suite(&reference_suite)?,
                    package_binding_for_suite(&compiled_kernel_suite)?,
                ],
                axis_coverage: all_axes(),
                case_ids: collect_reference_case_ids(
                    &reference_suite.benchmark_package,
                    TassadarWorkloadTarget::ArithmeticMicroprogram,
                )?
                .into_iter()
                .chain(collect_compiled_family_case_ids(
                    &compiled_kernel_suite.benchmark_package,
                    TassadarCompiledKernelFamilyId::ArithmeticKernel,
                ))
                .collect(),
            },
            TassadarBenchmarkFamilyContract {
                family: TassadarBenchmarkFamily::ClrsSubset,
                summary: String::from(
                    "bounded CLRS-adjacent shortest-path witness seeded in the reference validation corpus",
                ),
                benchmark_packages: vec![package_binding_for_suite(&reference_suite)?],
                axis_coverage: all_axes(),
                case_ids: collect_reference_case_ids(
                    &reference_suite.benchmark_package,
                    TassadarWorkloadTarget::ClrsShortestPath,
                )?,
            },
            TassadarBenchmarkFamilyContract {
                family: TassadarBenchmarkFamily::Sudoku,
                summary: String::from(
                    "Sudoku-class exact-search workloads across the article-class package and the exact 9x9 suite",
                ),
                benchmark_packages: vec![
                    package_binding_for_suite(&article_suite)?,
                    package_binding_for_suite(&sudoku_9x9_suite)?,
                ],
                axis_coverage: all_axes(),
                case_ids: collect_reference_case_ids(
                    &article_suite.benchmark_package,
                    TassadarWorkloadTarget::SudokuClass,
                )?
                .into_iter()
                .chain(sudoku_9x9_suite.benchmark_package.cases.iter().map(|case| case.case_id.clone()))
                .collect(),
            },
            TassadarBenchmarkFamilyContract {
                family: TassadarBenchmarkFamily::Hungarian,
                summary: String::from(
                    "matching-class workloads across the article-class, bounded 4x4, and article-sized 10x10 suites",
                ),
                benchmark_packages: vec![
                    package_binding_for_suite(&article_suite)?,
                    package_binding_for_suite(&hungarian_v0_suite)?,
                    package_binding_for_suite(&hungarian_10x10_suite)?,
                ],
                axis_coverage: all_axes(),
                case_ids: collect_reference_case_ids(
                    &article_suite.benchmark_package,
                    TassadarWorkloadTarget::HungarianMatching,
                )?
                .into_iter()
                .chain(hungarian_v0_suite.benchmark_package.cases.iter().map(|case| case.case_id.clone()))
                .chain(
                    hungarian_10x10_suite
                        .benchmark_package
                        .cases
                        .iter()
                        .map(|case| case.case_id.clone()),
                )
                .collect(),
            },
            TassadarBenchmarkFamilyContract {
                family: TassadarBenchmarkFamily::TraceLengthStress,
                summary: String::from(
                    "explicit horizon and trace-growth workloads across long-loop article cases and compiled backward-loop kernels",
                ),
                benchmark_packages: vec![
                    package_binding_for_suite(&article_suite)?,
                    package_binding_for_suite(&compiled_kernel_suite)?,
                ],
                axis_coverage: all_axes(),
                case_ids: collect_reference_case_ids(
                    &article_suite.benchmark_package,
                    TassadarWorkloadTarget::LongLoopKernel,
                )?
                .into_iter()
                .chain(collect_compiled_family_case_ids(
                    &compiled_kernel_suite.benchmark_package,
                    TassadarCompiledKernelFamilyId::BackwardLoopKernel,
                ))
                .collect(),
            },
        ],
    )
    .map_err(TassadarBenchmarkPackageSetSummaryError::from)
}

/// Builds the repo-facing summary report for the public Tassadar benchmark package set.
pub fn build_tassadar_benchmark_package_set_summary_report(
    version: &str,
) -> Result<TassadarBenchmarkPackageSetSummaryReport, TassadarBenchmarkPackageSetSummaryError> {
    let package_set = build_tassadar_benchmark_package_set_contract(version)?;
    let family_summaries = vec![
        TassadarBenchmarkPackageSetFamilySummary {
            family: TassadarBenchmarkFamily::Arithmetic,
            summary: String::from(
                "exact arithmetic kernels spanning the reference validation corpus and compiled kernel suite",
            ),
            benchmark_package_refs: package_refs_for_family(
                &package_set,
                TassadarBenchmarkFamily::Arithmetic,
            ),
            case_ids: case_ids_for_family(&package_set, TassadarBenchmarkFamily::Arithmetic),
            exactness_score_bps: 10_000,
            exactness_support_refs: vec![
                String::from("benchmark://openagents/tassadar/reference_fixture/validation_corpus"),
                String::from("fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json"),
            ],
            exactness_summary: String::from(
                "reference arithmetic microprograms and compiled arithmetic kernels are exact on the current committed package set",
            ),
            length_generalization_posture: TassadarLengthGeneralizationPosture::BoundedHoldout,
            length_generalization_support_refs: vec![String::from(
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
            )],
            length_generalization_summary: String::from(
                "arithmetic coverage is benchmarked across bounded held-out and compiled scaling regimes rather than promoted as arbitrary-program closure",
            ),
            planner_usefulness_posture: TassadarPlannerUsefulnessPosture::RouteCandidate,
            planner_usefulness_support_refs: vec![String::from(
                "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
            )],
            planner_usefulness_summary: String::from(
                "arithmetic workloads remain useful route-selection probes for exact-compute invocation, not just systems-only smoke tests",
            ),
        },
        TassadarBenchmarkPackageSetFamilySummary {
            family: TassadarBenchmarkFamily::ClrsSubset,
            summary: String::from(
                "bounded CLRS-adjacent shortest-path witness in the validation benchmark package",
            ),
            benchmark_package_refs: package_refs_for_family(
                &package_set,
                TassadarBenchmarkFamily::ClrsSubset,
            ),
            case_ids: case_ids_for_family(&package_set, TassadarBenchmarkFamily::ClrsSubset),
            exactness_score_bps: 10_000,
            exactness_support_refs: vec![String::from(
                "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
            )],
            exactness_summary: String::from(
                "the current public CLRS subset is seeded by an exact shortest-path witness rather than a broad benchmark claim",
            ),
            length_generalization_posture: TassadarLengthGeneralizationPosture::BoundedHoldout,
            length_generalization_support_refs: vec![String::from(
                "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
            )],
            length_generalization_summary: String::from(
                "the CLRS subset remains intentionally narrow and benchmark-bound until a wider literature-legible family lands",
            ),
            planner_usefulness_posture: TassadarPlannerUsefulnessPosture::RouteCandidate,
            planner_usefulness_support_refs: vec![String::from(
                "benchmark://openagents/tassadar/reference_fixture/validation_corpus",
            )],
            planner_usefulness_summary: String::from(
                "shortest-path style exact reasoning remains a planner-relevant route candidate because language-only heuristics should not silently replace exact compute on structured graph tasks",
            ),
        },
        TassadarBenchmarkPackageSetFamilySummary {
            family: TassadarBenchmarkFamily::Sudoku,
            summary: String::from(
                "Sudoku-class exact-search coverage across article-class and exact 9x9 benchmark packages",
            ),
            benchmark_package_refs: package_refs_for_family(
                &package_set,
                TassadarBenchmarkFamily::Sudoku,
            ),
            case_ids: case_ids_for_family(&package_set, TassadarBenchmarkFamily::Sudoku),
            exactness_score_bps: 10_000,
            exactness_support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
                String::from(
                    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
                ),
            ],
            exactness_summary: String::from(
                "the current package set carries exact runtime evidence on 4x4 Sudoku article cases and exact compiled/proof-backed evidence on the committed 9x9 suite",
            ),
            length_generalization_posture: TassadarLengthGeneralizationPosture::ArticleScale,
            length_generalization_support_refs: vec![String::from(
                "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
            )],
            length_generalization_summary: String::from(
                "Sudoku benchmark coverage now includes article-sized 9x9 exact compiled closure instead of stopping at the bounded 4x4 package alone",
            ),
            planner_usefulness_posture: TassadarPlannerUsefulnessPosture::RouteCandidate,
            planner_usefulness_support_refs: vec![String::from(
                TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
            )],
            planner_usefulness_summary: String::from(
                "Sudoku remains a direct planner/executor routing probe because the benchmark family measures exact search, fallback posture, and long-horizon executor usefulness together",
            ),
        },
        TassadarBenchmarkPackageSetFamilySummary {
            family: TassadarBenchmarkFamily::Hungarian,
            summary: String::from(
                "matching-class coverage across article-class, bounded 4x4, and article-sized 10x10 benchmark packages",
            ),
            benchmark_package_refs: package_refs_for_family(
                &package_set,
                TassadarBenchmarkFamily::Hungarian,
            ),
            case_ids: case_ids_for_family(&package_set, TassadarBenchmarkFamily::Hungarian),
            exactness_score_bps: 10_000,
            exactness_support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
                String::from(
                    "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json",
                ),
                String::from(
                    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
                ),
            ],
            exactness_summary: String::from(
                "matching-class packages are exact on the current public benchmark set, with compiled/proof-backed closure widening from bounded 4x4 to article-sized 10x10",
            ),
            length_generalization_posture: TassadarLengthGeneralizationPosture::ArticleScale,
            length_generalization_support_refs: vec![String::from(
                "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
            )],
            length_generalization_summary: String::from(
                "Hungarian coverage now reaches article-sized 10x10 workloads rather than stopping at the bounded 4x4 benchmark family",
            ),
            planner_usefulness_posture: TassadarPlannerUsefulnessPosture::RouteCandidate,
            planner_usefulness_support_refs: vec![String::from(
                TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
            )],
            planner_usefulness_summary: String::from(
                "matching workloads remain planner-relevant because they are structured exact-compute tasks where route choice and fallback posture materially change outcome quality",
            ),
        },
        TassadarBenchmarkPackageSetFamilySummary {
            family: TassadarBenchmarkFamily::TraceLengthStress,
            summary: String::from(
                "explicit horizon and trace-growth workloads across long-loop article cases and compiled backward-loop kernels",
            ),
            benchmark_package_refs: package_refs_for_family(
                &package_set,
                TassadarBenchmarkFamily::TraceLengthStress,
            ),
            case_ids: case_ids_for_family(&package_set, TassadarBenchmarkFamily::TraceLengthStress),
            exactness_score_bps: 10_000,
            exactness_support_refs: vec![
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
                String::from("fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json"),
            ],
            exactness_summary: String::from(
                "trace-stress workloads stay exact on the current package set, but they are kept separate because they primarily measure horizon and fallback posture rather than task breadth",
            ),
            length_generalization_posture: TassadarLengthGeneralizationPosture::ExplicitTraceStress,
            length_generalization_support_refs: vec![String::from(
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
            )],
            length_generalization_summary: String::from(
                "trace-length stress is reported explicitly through long-loop and backward-loop families instead of being hidden inside one aggregate score",
            ),
            planner_usefulness_posture: TassadarPlannerUsefulnessPosture::SystemsOnly,
            planner_usefulness_support_refs: vec![String::from(
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
            )],
            planner_usefulness_summary: String::from(
                "trace-stress workloads are primarily systems and closure diagnostics, not direct planner task wedges on their own",
            ),
        },
    ];

    let mut generated_from_refs = BTreeSet::new();
    for family in &package_set.families {
        for package in &family.benchmark_packages {
            generated_from_refs.insert(format!("{}@{}", package.benchmark_ref, package.version));
        }
    }
    generated_from_refs.insert(String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF));
    generated_from_refs.insert(String::from(
        "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json",
    ));
    generated_from_refs.insert(String::from(
        "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
    ));
    generated_from_refs.insert(String::from(
        "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
    ));
    generated_from_refs.insert(String::from(
        "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json",
    ));
    generated_from_refs.insert(String::from(
        "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
    ));

    Ok(TassadarBenchmarkPackageSetSummaryReport::new(
        package_set,
        family_summaries,
        generated_from_refs.into_iter().collect(),
    ))
}

/// Writes the public benchmark-package-set summary report to disk.
pub fn write_tassadar_benchmark_package_set_summary_report(
    path: impl AsRef<Path>,
    version: &str,
) -> Result<TassadarBenchmarkPackageSetSummaryReport, TassadarBenchmarkPackageSetSummaryError> {
    let report = build_tassadar_benchmark_package_set_summary_report(version)?;
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

fn all_axes() -> Vec<TassadarBenchmarkAxis> {
    vec![
        TassadarBenchmarkAxis::Exactness,
        TassadarBenchmarkAxis::LengthGeneralization,
        TassadarBenchmarkAxis::PlannerUsefulness,
    ]
}

fn package_binding_for_suite(
    suite: &TassadarReferenceFixtureSuite,
) -> Result<TassadarBenchmarkPackageBinding, TassadarBenchmarkPackageSetSummaryError> {
    let dataset = suite.benchmark_package.dataset.clone().ok_or_else(|| {
        TassadarBenchmarkPackageSetSummaryError::MissingDatasetBinding {
            benchmark_ref: suite.benchmark_package.key.benchmark_ref.clone(),
        }
    })?;
    Ok(TassadarBenchmarkPackageBinding {
        benchmark_ref: suite.benchmark_package.key.benchmark_ref.clone(),
        version: suite.benchmark_package.key.version.clone(),
        environment_ref: suite.benchmark_package.environment.environment_ref.clone(),
        dataset,
        split: suite.benchmark_package.split.clone(),
    })
}

fn collect_reference_case_ids(
    package: &BenchmarkPackage,
    workload_target: TassadarWorkloadTarget,
) -> Result<Vec<String>, TassadarBenchmarkPackageSetSummaryError> {
    let mut case_ids = package
        .cases
        .iter()
        .filter_map(|case| {
            benchmark_case_workload_target(case)
                .filter(|target| *target == workload_target)
                .map(|_| case.case_id.clone())
        })
        .collect::<Vec<_>>();
    if case_ids.is_empty() {
        return Err(
            TassadarBenchmarkPackageSetSummaryError::MissingCaseMetadata {
                case_id: format!("workload::{workload_target:?}"),
            },
        );
    }
    case_ids.sort();
    Ok(case_ids)
}

fn benchmark_case_workload_target(case: &BenchmarkCase) -> Option<TassadarWorkloadTarget> {
    serde_json::from_value(case.metadata.get("workload_target")?.clone()).ok()
}

fn collect_compiled_family_case_ids(
    package: &BenchmarkPackage,
    family_id: TassadarCompiledKernelFamilyId,
) -> Vec<String> {
    let mut case_ids = package
        .cases
        .iter()
        .filter_map(|case| {
            serde_json::from_value::<TassadarCompiledKernelFamilyId>(
                case.metadata.get("family_id")?.clone(),
            )
            .ok()
            .filter(|observed| *observed == family_id)
            .map(|_| case.case_id.clone())
        })
        .collect::<Vec<_>>();
    case_ids.sort();
    case_ids
}

fn package_refs_for_family(
    package_set: &TassadarBenchmarkPackageSetContract,
    family: TassadarBenchmarkFamily,
) -> Vec<String> {
    package_set
        .families
        .iter()
        .find(|entry| entry.family == family)
        .map(|entry| {
            entry
                .benchmark_packages
                .iter()
                .map(|package| format!("{}@{}", package.benchmark_ref, package.version))
                .collect()
        })
        .unwrap_or_default()
}

fn case_ids_for_family(
    package_set: &TassadarBenchmarkPackageSetContract,
    family: TassadarBenchmarkFamily,
) -> Vec<String> {
    package_set
        .families
        .iter()
        .find(|entry| entry.family == family)
        .map(|entry| entry.case_ids.clone())
        .unwrap_or_default()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("benchmark package-set summary should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_benchmark_package_set_contract,
        build_tassadar_benchmark_package_set_summary_report,
        write_tassadar_benchmark_package_set_summary_report,
        TassadarBenchmarkPackageSetSummaryReport,
    };
    use crate::TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF;
    use psionic_data::TassadarBenchmarkFamily;

    #[test]
    fn benchmark_package_set_contract_covers_public_families(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = build_tassadar_benchmark_package_set_contract("2026.03.17")?;
        assert_eq!(contract.families.len(), 5);
        assert!(contract.families.iter().any(|family| {
            family.family == TassadarBenchmarkFamily::ClrsSubset
                && family
                    .case_ids
                    .contains(&String::from("shortest_path_two_route"))
        }));
        assert!(contract.families.iter().any(|family| {
            family.family == TassadarBenchmarkFamily::TraceLengthStress
                && family
                    .case_ids
                    .iter()
                    .any(|case_id| case_id.starts_with("backward_loop_"))
        }));
        Ok(())
    }

    #[test]
    fn benchmark_package_set_summary_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_benchmark_package_set_summary_report("2026.03.17")?;
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF),
        )?;
        let persisted: TassadarBenchmarkPackageSetSummaryReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn write_benchmark_package_set_summary_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-benchmark-package-set-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_benchmark_package_set_summary.json");
        let report =
            write_tassadar_benchmark_package_set_summary_report(&report_path, "2026.03.17")?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarBenchmarkPackageSetSummaryReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }
}
