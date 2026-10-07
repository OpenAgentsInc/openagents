use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleAbiCaseStatus, TassadarArticleAbiClosureReportError,
    TassadarArticleRuntimeCloseoutReportError, TassadarRustArticleProfileCompletenessReportError,
    TassadarRustSourceCanonCaseStatus, TassadarRustSourceCanonReportError,
    build_tassadar_article_abi_closure_report, build_tassadar_article_runtime_closeout_report,
    build_tassadar_rust_article_profile_completeness_report,
    build_tassadar_rust_source_canon_report,
};
use psionic_models::TassadarRustArticleProfileRowStatus;
use psionic_research::{
    TassadarRustOnlyArticleAcceptanceSummaryError,
    TassadarArticleRuntimeCloseoutSummaryError, TassadarHungarian10x10ArticleReproducerError,
    TassadarSudoku9x9ArticleReproducerError,
    build_tassadar_article_runtime_closeout_summary_report,
    build_tassadar_hungarian_10x10_article_reproducer_report,
    tassadar_rust_only_article_acceptance_summary_report_path,
    build_tassadar_sudoku_9x9_article_reproducer_report,
    write_tassadar_rust_only_article_acceptance_summary_report,
};

use crate::{
    TassadarDirectModelWeightExecutionProofReportError,
    TassadarRustOnlyArticleAcceptanceGateV2Error,
    build_tassadar_rust_only_article_acceptance_gate_v2_report,
    build_tassadar_direct_model_weight_execution_proof_report,
    tassadar_rust_only_article_acceptance_gate_v2_report_path,
    write_tassadar_rust_only_article_acceptance_gate_v2_report,
};

pub const TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_reproduction_report.json";
pub const TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK_REF: &str =
    "docs/TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK.md";
pub const TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_SCRIPT_REF: &str =
    "scripts/check-tassadar-rust-only-article-reproduction.sh";
pub const TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_HARNESS_COMMAND: &str =
    "./scripts/check-tassadar-rust-only-article-reproduction.sh";

const REPORT_SCHEMA_VERSION: u16 = 1;
const REQUIRED_SOURCE_CASE_IDS: [&str; 6] = [
    "micro_wasm_article",
    "param_abi_fixture",
    "heap_sum_article",
    "long_loop_article",
    "hungarian_10x10_article",
    "sudoku_9x9_article",
];
const REQUIRED_ABI_EXACT_CASE_IDS: [&str; 3] = [
    "direct_scalar_i32_param",
    "pointer_length_heap_input",
    "pointer_length_heap_input_with_offset",
];
const REQUIRED_ABI_REFUSAL_CASE_IDS: [&str; 3] = [
    "heap_input_out_of_range_refusal",
    "floating_point_param_refusal",
    "multi_result_return_refusal",
];
const REQUIRED_DIRECT_PROOF_CASE_IDS: [&str; 3] =
    ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"];
const REQUIRED_RUNTIME_WORKLOAD_IDS: [&str; 2] =
    ["rust.long_loop_kernel", "rust.state_machine_kernel"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleReproductionComponent {
    pub component_id: String,
    pub artifact_refs: Vec<String>,
    pub validation_command: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleReproductionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub script_ref: String,
    pub runbook_ref: String,
    pub harness_command: String,
    pub component_count: u32,
    pub green_component_count: u32,
    pub canonical_case_ids: Vec<String>,
    pub long_horizon_workload_ids: Vec<String>,
    pub components: Vec<TassadarRustOnlyArticleReproductionComponent>,
    pub all_components_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarRustOnlyArticleReproductionReport {
    fn new(components: Vec<TassadarRustOnlyArticleReproductionComponent>) -> Self {
        let green_component_count = components
            .iter()
            .filter(|component| component.green)
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.rust_only_article_reproduction.v1"),
            script_ref: String::from(TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_SCRIPT_REF),
            runbook_ref: String::from(TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK_REF),
            harness_command: String::from(TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_HARNESS_COMMAND),
            component_count: components.len() as u32,
            green_component_count: green_component_count as u32,
            canonical_case_ids: vec![
                String::from("hungarian_10x10_test_a"),
                String::from("sudoku_9x9_test_a"),
                String::from("long_loop_kernel"),
                String::from("sudoku_v0_test_a"),
                String::from("hungarian_matching"),
            ],
            long_horizon_workload_ids: REQUIRED_RUNTIME_WORKLOAD_IDS
                .iter()
                .map(|value| String::from(*value))
                .collect(),
            all_components_green: green_component_count == components.len(),
            components,
            claim_boundary: String::from(
                "this harness report closes operator procedure only for the committed Rust-only article path by proving that one command can regenerate the current source canon, profile boundary, ABI closure, Hungarian reproducer, Sudoku reproducer, million-step runtime closeout, direct model-weight proof, and acceptance gate surfaces. It does not widen the claim beyond those committed workloads, routes, proofs, and prerequisite gates.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article reproduction now runs in one command with green_components={}/{} across source canon, profile boundary, ABI closure, Hungarian, Sudoku, million-step runtime, runtime summary, direct proof, and acceptance gate surfaces.",
            report.green_component_count, report.component_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_rust_only_article_reproduction_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarRustOnlyArticleReproductionError {
    #[error(transparent)]
    RustSourceCanon(#[from] TassadarRustSourceCanonReportError),
    #[error(transparent)]
    RustArticleProfileCompleteness(#[from] TassadarRustArticleProfileCompletenessReportError),
    #[error(transparent)]
    ArticleAbiClosure(#[from] TassadarArticleAbiClosureReportError),
    #[error(transparent)]
    Hungarian10x10ArticleReproducer(#[from] TassadarHungarian10x10ArticleReproducerError),
    #[error(transparent)]
    Sudoku9x9ArticleReproducer(#[from] TassadarSudoku9x9ArticleReproducerError),
    #[error(transparent)]
    ArticleRuntimeCloseout(#[from] TassadarArticleRuntimeCloseoutReportError),
    #[error(transparent)]
    ArticleRuntimeCloseoutSummary(#[from] TassadarArticleRuntimeCloseoutSummaryError),
    #[error(transparent)]
    RustOnlyArticleAcceptanceGateV2(#[from] TassadarRustOnlyArticleAcceptanceGateV2Error),
    #[error(transparent)]
    RustOnlyArticleAcceptanceSummary(#[from] TassadarRustOnlyArticleAcceptanceSummaryError),
    #[error(transparent)]
    DirectModelWeightExecutionProof(#[from] TassadarDirectModelWeightExecutionProofReportError),
    #[error("required repo artifact `{path}` is missing")]
    MissingRepoArtifact { path: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_rust_only_article_reproduction_report()
-> Result<TassadarRustOnlyArticleReproductionReport, TassadarRustOnlyArticleReproductionError> {
    ensure_repo_path_exists(TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK_REF)?;
    ensure_repo_path_exists(TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_SCRIPT_REF)?;

    let source_report = build_tassadar_rust_source_canon_report()?;
    let compiled_source_case_count = REQUIRED_SOURCE_CASE_IDS
        .iter()
        .filter(|case_id| {
            source_report.cases.iter().any(|case| {
                case.case_id == **case_id
                    && case.status == TassadarRustSourceCanonCaseStatus::Compiled
            })
        })
        .count();
    let source_green = compiled_source_case_count == REQUIRED_SOURCE_CASE_IDS.len();

    let profile_report = build_tassadar_rust_article_profile_completeness_report();
    let profile_supported_count = profile_report
        .rows
        .iter()
        .filter(|row| row.status == TassadarRustArticleProfileRowStatus::Supported)
        .count();
    let profile_refused_count = profile_report
        .rows
        .iter()
        .filter(|row| row.status == TassadarRustArticleProfileRowStatus::Refused)
        .count();
    let profile_green = REQUIRED_SOURCE_CASE_IDS.iter().all(|case_id| {
        profile_report
            .source_case_ids
            .contains(&String::from(*case_id))
    }) && profile_supported_count > 0
        && profile_refused_count > 0;

    let abi_report = build_tassadar_article_abi_closure_report()?;
    let abi_exact_count = REQUIRED_ABI_EXACT_CASE_IDS
        .iter()
        .filter(|case_id| {
            abi_report.cases.iter().any(|case| {
                case.case_id == **case_id && case.status == TassadarArticleAbiCaseStatus::Exact
            })
        })
        .count();
    let abi_refusal_count = REQUIRED_ABI_REFUSAL_CASE_IDS
        .iter()
        .filter(|case_id| {
            abi_report.cases.iter().any(|case| {
                case.case_id == **case_id && case.status == TassadarArticleAbiCaseStatus::Refused
            })
        })
        .count();
    let abi_green = abi_exact_count == REQUIRED_ABI_EXACT_CASE_IDS.len()
        && abi_refusal_count == REQUIRED_ABI_REFUSAL_CASE_IDS.len();

    let hungarian_report = build_tassadar_hungarian_10x10_article_reproducer_report()?;
    let hungarian_green = hungarian_report.exact_trace_match
        && hungarian_report.final_output_match
        && hungarian_report.halt_match
        && !hungarian_report.direct_execution_posture.fallback_observed
        && !hungarian_report
            .direct_execution_posture
            .external_tool_surface_observed;

    let sudoku_report = build_tassadar_sudoku_9x9_article_reproducer_report()?;
    let sudoku_green = sudoku_report.exact_trace_match
        && sudoku_report.final_output_match
        && sudoku_report.halt_match
        && sudoku_report
            .article_corpus_cases
            .iter()
            .all(|case| case.exact_trace_match && case.final_output_match)
        && !sudoku_report.direct_execution_posture.fallback_observed
        && !sudoku_report
            .direct_execution_posture
            .external_tool_surface_observed;

    let runtime_report = build_tassadar_article_runtime_closeout_report()?;
    let runtime_green = runtime_report.exact_horizon_count == 4
        && runtime_report.floor_pass_count == 4
        && runtime_report.floor_refusal_count == 0
        && REQUIRED_RUNTIME_WORKLOAD_IDS.iter().all(|workload_id| {
            runtime_report
                .bundle
                .workload_family_ids
                .contains(&String::from(*workload_id))
        });

    let runtime_summary = build_tassadar_article_runtime_closeout_summary_report()?;
    let runtime_summary_green = REQUIRED_RUNTIME_WORKLOAD_IDS.iter().all(|workload_id| {
        runtime_summary
            .green_workload_family_ids
            .contains(&String::from(*workload_id))
    }) && runtime_summary.workload_family_ids.len()
        == REQUIRED_RUNTIME_WORKLOAD_IDS.len();

    let direct_proof_report = build_tassadar_direct_model_weight_execution_proof_report()?;
    let direct_proof_green = direct_proof_report.direct_case_count == 3
        && direct_proof_report.fallback_free_case_count == 3
        && direct_proof_report.zero_external_call_case_count == 3
        && REQUIRED_DIRECT_PROOF_CASE_IDS.iter().all(|case_id| {
            direct_proof_report
                .case_ids
                .contains(&String::from(*case_id))
        })
        && !direct_proof_report.route_descriptor_digest.is_empty();

    let acceptance_gate_report = build_tassadar_rust_only_article_acceptance_gate_v2_report()?;
    let acceptance_gate_green = acceptance_gate_report.green
        && acceptance_gate_report.prerequisite_count == 8
        && acceptance_gate_report.passed_prerequisite_count == 8
        && acceptance_gate_report.failed_prerequisite_ids.is_empty();

    let components = vec![
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("rust_source_canon"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-eval --example tassadar_rust_source_canon_report",
            ),
            green: source_green,
            detail: format!(
                "compiled_cases={}/{} across {:?}",
                compiled_source_case_count,
                REQUIRED_SOURCE_CASE_IDS.len(),
                REQUIRED_SOURCE_CASE_IDS
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("rust_article_profile_completeness"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-eval --example tassadar_rust_article_profile_completeness_report",
            ),
            green: profile_green,
            detail: format!(
                "supported_rows={} refused_rows={} source_case_ids={}",
                profile_supported_count,
                profile_refused_count,
                profile_report.source_case_ids.len(),
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("article_abi_closure"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-eval --example tassadar_article_abi_closure_report",
            ),
            green: abi_green,
            detail: format!(
                "exact_cases={}/{} refusal_cases={}/{}",
                abi_exact_count,
                REQUIRED_ABI_EXACT_CASE_IDS.len(),
                abi_refusal_count,
                REQUIRED_ABI_REFUSAL_CASE_IDS.len(),
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("hungarian_10x10_article_reproducer"),
            artifact_refs: vec![
                String::from("fixtures/tassadar/runs/hungarian_10x10_article_reproducer_v1"),
                String::from(
                    "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json",
                ),
            ],
            validation_command: String::from(
                "cargo run -p psionic-research --example tassadar_hungarian_10x10_article_reproducer",
            ),
            green: hungarian_green,
            detail: format!(
                "exact_trace_match={} final_output_match={} halt_match={} fallback_observed={}",
                hungarian_report.exact_trace_match,
                hungarian_report.final_output_match,
                hungarian_report.halt_match,
                hungarian_report.direct_execution_posture.fallback_observed,
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("sudoku_9x9_article_reproducer"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-research --example tassadar_sudoku_9x9_article_reproducer",
            ),
            green: sudoku_green,
            detail: format!(
                "canonical_case={} corpus_cases={} fallback_observed={}",
                sudoku_report.canonical_case_id,
                sudoku_report.article_corpus_cases.len(),
                sudoku_report.direct_execution_posture.fallback_observed,
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("article_runtime_closeout"),
            artifact_refs: vec![
                String::from(
                    "fixtures/tassadar/runs/article_runtime_closeout_v1/article_runtime_closeout_bundle.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json",
                ),
            ],
            validation_command: String::from(
                "cargo run -p psionic-eval --example tassadar_article_runtime_closeout_report",
            ),
            green: runtime_green,
            detail: format!(
                "exact_horizons={} floor_passes={} floor_refusals={}",
                runtime_report.exact_horizon_count,
                runtime_report.floor_pass_count,
                runtime_report.floor_refusal_count,
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("article_runtime_closeout_summary"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_article_runtime_closeout_summary.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-research --example tassadar_article_runtime_closeout_summary",
            ),
            green: runtime_summary_green,
            detail: format!(
                "green_workload_families={}/{}",
                runtime_summary.green_workload_family_ids.len(),
                runtime_summary.workload_family_ids.len(),
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("direct_model_weight_execution_proof"),
            artifact_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json",
            )],
            validation_command: String::from(
                "cargo run -p psionic-serve --example tassadar_direct_model_weight_execution_proof_report",
            ),
            green: direct_proof_green,
            detail: format!(
                "direct_cases={} fallback_free_cases={} zero_external_call_cases={}",
                direct_proof_report.direct_case_count,
                direct_proof_report.fallback_free_case_count,
                direct_proof_report.zero_external_call_case_count,
            ),
        },
        TassadarRustOnlyArticleReproductionComponent {
            component_id: String::from("rust_only_article_acceptance_gate_v2"),
            artifact_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_rust_only_article_acceptance_gate_v2.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_rust_only_article_acceptance_summary.json",
                ),
            ],
            validation_command: String::from(
                "./scripts/check-tassadar-rust-only-article-acceptance-v2.sh",
            ),
            green: acceptance_gate_green,
            detail: format!(
                "passed_prerequisites={}/{} failed_prerequisites={}",
                acceptance_gate_report.passed_prerequisite_count,
                acceptance_gate_report.prerequisite_count,
                acceptance_gate_report.failed_prerequisite_ids.len(),
            ),
        },
    ];

    for component in &components {
        for artifact_ref in &component.artifact_refs {
            ensure_repo_path_exists(artifact_ref)?;
        }
    }

    Ok(TassadarRustOnlyArticleReproductionReport::new(components))
}

#[must_use]
pub fn tassadar_rust_only_article_reproduction_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_REPORT_REF)
}

pub fn write_tassadar_rust_only_article_reproduction_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRustOnlyArticleReproductionReport, TassadarRustOnlyArticleReproductionError> {
    write_tassadar_rust_only_article_acceptance_gate_v2_report(
        tassadar_rust_only_article_acceptance_gate_v2_report_path(),
    )?;
    write_tassadar_rust_only_article_acceptance_summary_report(
        tassadar_rust_only_article_acceptance_summary_report_path(),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustOnlyArticleReproductionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_only_article_reproduction_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRustOnlyArticleReproductionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn ensure_repo_path_exists(
    relative_path: &str,
) -> Result<(), TassadarRustOnlyArticleReproductionError> {
    let path = repo_root().join(relative_path);
    if path.exists() {
        Ok(())
    } else {
        Err(
            TassadarRustOnlyArticleReproductionError::MissingRepoArtifact {
                path: relative_path.to_string(),
            },
        )
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-serve should live under <repo>/crates/psionic-serve")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarRustOnlyArticleReproductionError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarRustOnlyArticleReproductionError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRustOnlyArticleReproductionError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_REPORT_REF,
        TassadarRustOnlyArticleReproductionReport,
        build_tassadar_rust_only_article_reproduction_report, read_repo_json,
        tassadar_rust_only_article_reproduction_report_path,
        write_tassadar_rust_only_article_reproduction_report,
    };

    #[test]
    fn rust_only_article_reproduction_report_is_machine_legible() {
        let report = build_tassadar_rust_only_article_reproduction_report().expect("report");

        assert_eq!(report.component_count, 9);
        assert_eq!(report.green_component_count, 9);
        assert!(report.all_components_green);
        assert_eq!(
            report.harness_command,
            "./scripts/check-tassadar-rust-only-article-reproduction.sh"
        );
        assert!(
            report
                .canonical_case_ids
                .contains(&String::from("hungarian_10x10_test_a"))
        );
        assert!(
            report
                .components
                .iter()
                .all(|component| !component.artifact_refs.is_empty())
        );
        assert!(
            report
                .components
                .iter()
                .any(|component| component.component_id == "rust_only_article_acceptance_gate_v2")
        );
    }

    #[test]
    fn rust_only_article_reproduction_report_matches_committed_truth() {
        let generated = build_tassadar_rust_only_article_reproduction_report().expect("report");
        let committed: TassadarRustOnlyArticleReproductionReport =
            read_repo_json(TASSADAR_RUST_ONLY_ARTICLE_REPRODUCTION_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_rust_only_article_reproduction_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_rust_only_article_reproduction_report.json");
        let written = write_tassadar_rust_only_article_reproduction_report(&output_path)
            .expect("write report");
        let persisted: TassadarRustOnlyArticleReproductionReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_rust_only_article_reproduction_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_rust_only_article_reproduction_report.json")
        );
    }
}
