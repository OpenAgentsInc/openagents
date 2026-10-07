use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_FILE: &str =
    "tassadar_compiled_article_closure_report.json";
pub const TASSADAR_COMPILED_ARTICLE_CLOSURE_CHECKER_COMMAND: &str =
    "scripts/check-tassadar-compiled-article-closure.sh";
pub const TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json";

const REPORT_SCHEMA_VERSION: u16 = 1;
const BOUNDED_SUDOKU_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/run_bundle.json";
const BOUNDED_HUNGARIAN_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json";
const ARTICLE_SUDOKU_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json";
const ARTICLE_SUDOKU_EXACTNESS_REPORT_REF: &str = "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const ARTICLE_SUDOKU_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/compiled_executor_compatibility_report.json";
const ARTICLE_HUNGARIAN_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json";
const ARTICLE_HUNGARIAN_EXACTNESS_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const ARTICLE_HUNGARIAN_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/compiled_executor_compatibility_report.json";
const KERNEL_SUITE_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json";
const KERNEL_SUITE_EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_exactness_report.json";
const KERNEL_SUITE_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_compatibility_report.json";
const KERNEL_SUITE_SCALING_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json";
const RUST_SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledArticleClosureRequirement {
    pub requirement_id: String,
    pub label: String,
    pub artifact_refs: Vec<String>,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledArticleClosureReport {
    pub schema_version: u16,
    pub checker_command: String,
    pub report_ref: String,
    pub required_workload_families: Vec<String>,
    pub article_artifact_roots: Vec<String>,
    pub bounded_proxy_roots: Vec<String>,
    pub requirements: Vec<TassadarCompiledArticleClosureRequirement>,
    pub passed: bool,
    pub detail: String,
    pub missing_requirements: Vec<String>,
    pub report_digest: String,
}

impl TassadarCompiledArticleClosureReport {
    fn new(
        requirements: Vec<TassadarCompiledArticleClosureRequirement>,
        passed: bool,
        detail: String,
        missing_requirements: Vec<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            checker_command: String::from(TASSADAR_COMPILED_ARTICLE_CLOSURE_CHECKER_COMMAND),
            report_ref: String::from(TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF),
            required_workload_families: vec![
                String::from("sudoku_search_9x9"),
                String::from("hungarian_matching_10x10"),
                String::from("arithmetic_kernel"),
                String::from("memory_update_kernel"),
                String::from("forward_branch_kernel"),
                String::from("backward_loop_kernel"),
            ],
            article_artifact_roots: vec![
                String::from("fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0"),
                String::from("fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0"),
                String::from("fixtures/tassadar/runs/compiled_kernel_suite_v0"),
            ],
            bounded_proxy_roots: vec![
                String::from("fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0"),
                String::from("fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0"),
            ],
            requirements,
            passed,
            detail,
            missing_requirements,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_article_closure_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompiledArticleClosureSnapshot {
    bounded_sudoku_claim_class: String,
    bounded_hungarian_claim_class: String,
    article_sudoku_claim_class: String,
    article_sudoku_exact_trace_rate_bps: u32,
    article_sudoku_refusal_rate_bps: u32,
    article_sudoku_deployments: usize,
    article_sudoku_has_benchmark_and_environment: bool,
    article_hungarian_claim_class: String,
    article_hungarian_exact_trace_rate_bps: u32,
    article_hungarian_refusal_rate_bps: u32,
    article_hungarian_deployments: usize,
    article_hungarian_has_benchmark_and_environment: bool,
    kernel_suite_claim_class: String,
    kernel_suite_exact_trace_rate_bps: u32,
    kernel_suite_refusal_rate_bps: u32,
    kernel_suite_deployments: usize,
    kernel_suite_has_benchmark_and_environment: bool,
    kernel_suite_family_ids: BTreeSet<String>,
    rust_source_canon_case_count: usize,
    rust_source_canon_compiled_case_count: usize,
    rust_source_canon_all_sources_are_rust: bool,
    rust_source_canon_article_cases_compiled: bool,
}

#[derive(Debug, Error)]
pub enum TassadarCompiledArticleClosureError {
    #[error("failed to read `{path}`: {error}")]
    Io { path: String, error: std::io::Error },
    #[error("failed to parse `{path}` as `{artifact_kind}`: {error}")]
    Json {
        path: String,
        artifact_kind: String,
        error: serde_json::Error,
    },
    #[error("failed to serialize compiled article-closure report: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to write compiled article-closure report to `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

#[must_use]
pub fn tassadar_compiled_article_closure_report_ref() -> &'static str {
    TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF
}

#[must_use]
pub fn tassadar_compiled_article_closure_report_path() -> PathBuf {
    repo_root().join(tassadar_compiled_article_closure_report_ref())
}

pub fn build_tassadar_compiled_article_closure_report()
-> Result<TassadarCompiledArticleClosureReport, TassadarCompiledArticleClosureError> {
    let bounded_sudoku_bundle: Value = read_repo_json(
        BOUNDED_SUDOKU_RUN_BUNDLE_REF,
        "tassadar_compiled_executor_run_bundle",
    )?;
    let bounded_hungarian_bundle: Value = read_repo_json(
        BOUNDED_HUNGARIAN_RUN_BUNDLE_REF,
        "tassadar_hungarian_compiled_executor_run_bundle",
    )?;
    let article_sudoku_bundle: Value = read_repo_json(
        ARTICLE_SUDOKU_RUN_BUNDLE_REF,
        "tassadar_sudoku_9x9_compiled_executor_run_bundle",
    )?;
    let article_sudoku_exactness: Value = read_repo_json(
        ARTICLE_SUDOKU_EXACTNESS_REPORT_REF,
        "tassadar_sudoku_9x9_compiled_executor_exactness_report",
    )?;
    let article_sudoku_compatibility: Value = read_repo_json(
        ARTICLE_SUDOKU_COMPATIBILITY_REPORT_REF,
        "tassadar_sudoku_9x9_compiled_executor_compatibility_report",
    )?;
    let article_hungarian_bundle: Value = read_repo_json(
        ARTICLE_HUNGARIAN_RUN_BUNDLE_REF,
        "tassadar_hungarian_10x10_compiled_executor_run_bundle",
    )?;
    let article_hungarian_exactness: Value = read_repo_json(
        ARTICLE_HUNGARIAN_EXACTNESS_REPORT_REF,
        "tassadar_hungarian_10x10_compiled_executor_exactness_report",
    )?;
    let article_hungarian_compatibility: Value = read_repo_json(
        ARTICLE_HUNGARIAN_COMPATIBILITY_REPORT_REF,
        "tassadar_hungarian_10x10_compiled_executor_compatibility_report",
    )?;
    let kernel_suite_bundle: Value = read_repo_json(
        KERNEL_SUITE_RUN_BUNDLE_REF,
        "tassadar_compiled_kernel_suite_run_bundle",
    )?;
    let kernel_suite_exactness: Value = read_repo_json(
        KERNEL_SUITE_EXACTNESS_REPORT_REF,
        "tassadar_compiled_kernel_suite_exactness_report",
    )?;
    let kernel_suite_compatibility: Value = read_repo_json(
        KERNEL_SUITE_COMPATIBILITY_REPORT_REF,
        "tassadar_compiled_kernel_suite_compatibility_report",
    )?;
    let kernel_suite_scaling: Value = read_repo_json(
        KERNEL_SUITE_SCALING_REPORT_REF,
        "tassadar_compiled_kernel_suite_scaling_report",
    )?;
    let rust_source_canon: Value = read_repo_json(
        RUST_SOURCE_CANON_REPORT_REF,
        "tassadar_rust_source_canon_report",
    )?;

    let snapshot = CompiledArticleClosureSnapshot {
        bounded_sudoku_claim_class: json_str(&bounded_sudoku_bundle, "claim_class"),
        bounded_hungarian_claim_class: json_str(&bounded_hungarian_bundle, "claim_class"),
        article_sudoku_claim_class: json_str(&article_sudoku_bundle, "claim_class"),
        article_sudoku_exact_trace_rate_bps: json_u32(
            &article_sudoku_exactness,
            "exact_trace_rate_bps",
        ),
        article_sudoku_refusal_rate_bps: json_u32(
            &article_sudoku_compatibility,
            "matched_refusal_rate_bps",
        ),
        article_sudoku_deployments: json_array_len(&article_sudoku_bundle, "deployments"),
        article_sudoku_has_benchmark_and_environment: has_nonempty_string(
            &article_sudoku_bundle,
            "benchmark_package_digest",
        ) && has_nonempty_string(
            &article_sudoku_bundle,
            "environment_bundle_digest",
        ),
        article_hungarian_claim_class: json_str(&article_hungarian_bundle, "claim_class"),
        article_hungarian_exact_trace_rate_bps: json_u32(
            &article_hungarian_exactness,
            "exact_trace_rate_bps",
        ),
        article_hungarian_refusal_rate_bps: json_u32(
            &article_hungarian_compatibility,
            "matched_refusal_rate_bps",
        ),
        article_hungarian_deployments: json_array_len(&article_hungarian_bundle, "deployments"),
        article_hungarian_has_benchmark_and_environment: has_nonempty_string(
            &article_hungarian_bundle,
            "benchmark_package_digest",
        ) && has_nonempty_string(
            &article_hungarian_bundle,
            "environment_bundle_digest",
        ),
        kernel_suite_claim_class: json_str(&kernel_suite_bundle, "claim_class"),
        kernel_suite_exact_trace_rate_bps: json_u32(
            &kernel_suite_exactness,
            "exact_trace_rate_bps",
        ),
        kernel_suite_refusal_rate_bps: json_u32(
            &kernel_suite_compatibility,
            "matched_refusal_rate_bps",
        ),
        kernel_suite_deployments: json_array_len(&kernel_suite_bundle, "deployments"),
        kernel_suite_has_benchmark_and_environment: has_nonempty_string(
            &kernel_suite_bundle,
            "benchmark_package_digest",
        ) && has_nonempty_string(
            &kernel_suite_bundle,
            "environment_bundle_digest",
        ),
        kernel_suite_family_ids: kernel_suite_scaling["family_reports"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|family| family["family_id"].as_str().map(String::from))
            .collect(),
        rust_source_canon_case_count: json_array_len(&rust_source_canon, "cases"),
        rust_source_canon_compiled_case_count: rust_source_canon["cases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|case| case["status"].as_str() == Some("compiled"))
            .count(),
        rust_source_canon_all_sources_are_rust: rust_source_canon["generated_from_refs"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|source_ref| {
                source_ref
                    .as_str()
                    .is_some_and(|source_ref| source_ref.ends_with(".rs"))
            }),
        rust_source_canon_article_cases_compiled: rust_source_canon["cases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|case| {
                case["case_id"]
                    .as_str()
                    .is_some_and(|case_id| case_id.contains("article"))
            })
            .all(|case| case["status"].as_str() == Some("compiled")),
    };
    Ok(report_from_snapshot(&snapshot))
}

pub fn write_tassadar_compiled_article_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompiledArticleClosureReport, TassadarCompiledArticleClosureError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarCompiledArticleClosureError::Io {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_compiled_article_closure_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| TassadarCompiledArticleClosureError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn report_from_snapshot(
    snapshot: &CompiledArticleClosureSnapshot,
) -> TassadarCompiledArticleClosureReport {
    let sudoku_9x9_passed = snapshot.article_sudoku_claim_class == "compiled_article_class"
        && snapshot.article_sudoku_exact_trace_rate_bps == 10_000
        && snapshot.article_sudoku_refusal_rate_bps == 10_000
        && snapshot.article_sudoku_deployments > 0
        && snapshot.article_sudoku_has_benchmark_and_environment;
    let hungarian_10x10_passed = snapshot.article_hungarian_claim_class == "compiled_article_class"
        && snapshot.article_hungarian_exact_trace_rate_bps == 10_000
        && snapshot.article_hungarian_refusal_rate_bps == 10_000
        && snapshot.article_hungarian_deployments > 0
        && snapshot.article_hungarian_has_benchmark_and_environment;
    let required_kernel_families = [
        "arithmetic_kernel",
        "memory_update_kernel",
        "forward_branch_kernel",
        "backward_loop_kernel",
    ]
    .into_iter()
    .map(String::from)
    .collect::<BTreeSet<_>>();
    let kernel_suite_passed = snapshot.kernel_suite_claim_class == "compiled_article_class"
        && snapshot.kernel_suite_exact_trace_rate_bps == 10_000
        && snapshot.kernel_suite_refusal_rate_bps == 10_000
        && snapshot.kernel_suite_deployments > 0
        && snapshot.kernel_suite_has_benchmark_and_environment
        && snapshot
            .kernel_suite_family_ids
            .is_superset(&required_kernel_families);
    let stronger_than_bounded_proxies = snapshot.bounded_sudoku_claim_class == "compiled_exact"
        && snapshot.bounded_hungarian_claim_class == "compiled_exact"
        && sudoku_9x9_passed
        && hungarian_10x10_passed
        && kernel_suite_passed;
    let rust_source_canon_passed = snapshot.rust_source_canon_case_count >= 8
        && snapshot.rust_source_canon_case_count == snapshot.rust_source_canon_compiled_case_count
        && snapshot.rust_source_canon_all_sources_are_rust
        && snapshot.rust_source_canon_article_cases_compiled;

    let requirements = vec![
        TassadarCompiledArticleClosureRequirement {
            requirement_id: String::from("rust_only_frontend_canon"),
            label: String::from(
                "Rust-only source canon binds the article frontend path to committed Rust fixtures",
            ),
            artifact_refs: vec![String::from(RUST_SOURCE_CANON_REPORT_REF)],
            passed: rust_source_canon_passed,
            detail: format!(
                "case_count={}; compiled_case_count={}; all_sources_are_rust={}; article_cases_compiled={}",
                snapshot.rust_source_canon_case_count,
                snapshot.rust_source_canon_compiled_case_count,
                snapshot.rust_source_canon_all_sources_are_rust,
                snapshot.rust_source_canon_article_cases_compiled
            ),
        },
        TassadarCompiledArticleClosureRequirement {
            requirement_id: String::from("compiled_article_sudoku_9x9"),
            label: String::from(
                "Article-sized compiled Sudoku-9x9 bundle exists with benchmark, proof, and exact refusal truth",
            ),
            artifact_refs: vec![
                String::from(ARTICLE_SUDOKU_RUN_BUNDLE_REF),
                String::from(ARTICLE_SUDOKU_EXACTNESS_REPORT_REF),
                String::from(ARTICLE_SUDOKU_COMPATIBILITY_REPORT_REF),
            ],
            passed: sudoku_9x9_passed,
            detail: format!(
                "claim_class={}; exact_trace_rate_bps={}; matched_refusal_rate_bps={}; deployments={}",
                snapshot.article_sudoku_claim_class,
                snapshot.article_sudoku_exact_trace_rate_bps,
                snapshot.article_sudoku_refusal_rate_bps,
                snapshot.article_sudoku_deployments
            ),
        },
        TassadarCompiledArticleClosureRequirement {
            requirement_id: String::from("compiled_article_hungarian_10x10"),
            label: String::from(
                "Article-sized compiled Hungarian-10x10 bundle exists with benchmark, proof, and exact refusal truth",
            ),
            artifact_refs: vec![
                String::from(ARTICLE_HUNGARIAN_RUN_BUNDLE_REF),
                String::from(ARTICLE_HUNGARIAN_EXACTNESS_REPORT_REF),
                String::from(ARTICLE_HUNGARIAN_COMPATIBILITY_REPORT_REF),
            ],
            passed: hungarian_10x10_passed,
            detail: format!(
                "claim_class={}; exact_trace_rate_bps={}; matched_refusal_rate_bps={}; deployments={}",
                snapshot.article_hungarian_claim_class,
                snapshot.article_hungarian_exact_trace_rate_bps,
                snapshot.article_hungarian_refusal_rate_bps,
                snapshot.article_hungarian_deployments
            ),
        },
        TassadarCompiledArticleClosureRequirement {
            requirement_id: String::from("compiled_article_kernel_suite"),
            label: String::from(
                "Generic compiled kernel suite exists with arithmetic, memory, branch, and loop families plus exactness-vs-trace-length evidence",
            ),
            artifact_refs: vec![
                String::from(KERNEL_SUITE_RUN_BUNDLE_REF),
                String::from(KERNEL_SUITE_EXACTNESS_REPORT_REF),
                String::from(KERNEL_SUITE_COMPATIBILITY_REPORT_REF),
                String::from(KERNEL_SUITE_SCALING_REPORT_REF),
            ],
            passed: kernel_suite_passed,
            detail: format!(
                "claim_class={}; exact_trace_rate_bps={}; matched_refusal_rate_bps={}; deployments={}; family_ids={}",
                snapshot.kernel_suite_claim_class,
                snapshot.kernel_suite_exact_trace_rate_bps,
                snapshot.kernel_suite_refusal_rate_bps,
                snapshot.kernel_suite_deployments,
                snapshot
                    .kernel_suite_family_ids
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        },
        TassadarCompiledArticleClosureRequirement {
            requirement_id: String::from("stronger_than_bounded_proxies"),
            label: String::from(
                "The older 4x4 compiled proxies are no longer the strongest exact compiled results in-tree",
            ),
            artifact_refs: vec![
                String::from(BOUNDED_SUDOKU_RUN_BUNDLE_REF),
                String::from(BOUNDED_HUNGARIAN_RUN_BUNDLE_REF),
                String::from(ARTICLE_SUDOKU_RUN_BUNDLE_REF),
                String::from(ARTICLE_HUNGARIAN_RUN_BUNDLE_REF),
                String::from(KERNEL_SUITE_RUN_BUNDLE_REF),
            ],
            passed: stronger_than_bounded_proxies,
            detail: format!(
                "bounded_sudoku_claim_class={}; bounded_hungarian_claim_class={}; article_workloads_green={}",
                snapshot.bounded_sudoku_claim_class,
                snapshot.bounded_hungarian_claim_class,
                sudoku_9x9_passed && hungarian_10x10_passed && kernel_suite_passed
            ),
        },
    ];
    let missing_requirements = requirements
        .iter()
        .filter(|requirement| !requirement.passed)
        .map(|requirement| requirement.label.clone())
        .collect::<Vec<_>>();
    let passed = missing_requirements.is_empty();
    let detail = if passed {
        String::from(
            "Compiled article-closure checker is green: the Rust-only frontend canon is bound to committed Rust fixtures, exact compiled article-sized Sudoku-9x9 and Hungarian-10x10 bundles plus the generic arithmetic/memory/branch/loop kernel suite all exist with benchmark and proof artifacts, and the older 4x4 proxies are no longer the strongest exact compiled results in-tree.",
        )
    } else {
        String::from(
            "Compiled article-closure checker is red: the Rust-only frontend canon is incomplete, the exact compiled article-sized workload set is incomplete, or the older 4x4 proxies are still the strongest exact compiled evidence in-tree.",
        )
    };
    TassadarCompiledArticleClosureReport::new(requirements, passed, detail, missing_requirements)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_repo_json<T>(
    repo_relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarCompiledArticleClosureError>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(repo_relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarCompiledArticleClosureError::Io {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarCompiledArticleClosureError::Json {
        path: path.display().to_string(),
        artifact_kind: String::from(artifact_kind),
        error,
    })
}

fn json_str(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or("").to_string()
}

fn json_u32(value: &Value, field: &str) -> u32 {
    value[field].as_u64().unwrap_or(0) as u32
}

fn json_array_len(value: &Value, field: &str) -> usize {
    value[field].as_array().map_or(0, Vec::len)
}

fn has_nonempty_string(value: &Value, field: &str) -> bool {
    value[field]
        .as_str()
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("compiled article-closure report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        CompiledArticleClosureSnapshot, TassadarCompiledArticleClosureReport,
        build_tassadar_compiled_article_closure_report, report_from_snapshot,
        tassadar_compiled_article_closure_report_ref,
        write_tassadar_compiled_article_closure_report,
    };
    use std::collections::BTreeSet;
    use tempfile::tempdir;

    #[test]
    fn compiled_article_closure_is_red_for_bounded_only_snapshot() {
        let report = report_from_snapshot(&CompiledArticleClosureSnapshot {
            bounded_sudoku_claim_class: String::from("compiled_exact"),
            bounded_hungarian_claim_class: String::from("compiled_exact"),
            article_sudoku_claim_class: String::from("compiled_exact"),
            article_sudoku_exact_trace_rate_bps: 10_000,
            article_sudoku_refusal_rate_bps: 10_000,
            article_sudoku_deployments: 4,
            article_sudoku_has_benchmark_and_environment: true,
            article_hungarian_claim_class: String::from("compiled_exact"),
            article_hungarian_exact_trace_rate_bps: 10_000,
            article_hungarian_refusal_rate_bps: 10_000,
            article_hungarian_deployments: 4,
            article_hungarian_has_benchmark_and_environment: true,
            kernel_suite_claim_class: String::from("compiled_exact"),
            kernel_suite_exact_trace_rate_bps: 10_000,
            kernel_suite_refusal_rate_bps: 10_000,
            kernel_suite_deployments: 12,
            kernel_suite_has_benchmark_and_environment: true,
            kernel_suite_family_ids: [
                String::from("arithmetic_kernel"),
                String::from("memory_update_kernel"),
                String::from("forward_branch_kernel"),
                String::from("backward_loop_kernel"),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            rust_source_canon_case_count: 0,
            rust_source_canon_compiled_case_count: 0,
            rust_source_canon_all_sources_are_rust: false,
            rust_source_canon_article_cases_compiled: false,
        });
        assert!(!report.passed);
        assert!(!report.missing_requirements.is_empty());
    }

    #[test]
    fn compiled_article_closure_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_compiled_article_closure_report()?;
        let persisted: TassadarCompiledArticleClosureReport = super::read_repo_json(
            tassadar_compiled_article_closure_report_ref(),
            "tassadar_compiled_article_closure_report",
        )?;
        assert_eq!(persisted, report);
        assert!(report.passed);
        Ok(())
    }

    #[test]
    fn write_compiled_article_closure_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_compiled_article_closure_report.json");
        let report = write_tassadar_compiled_article_closure_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarCompiledArticleClosureReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
