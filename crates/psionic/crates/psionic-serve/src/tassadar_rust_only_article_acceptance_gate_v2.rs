use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use thiserror::Error;

use psionic_eval::{
    TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF, TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF,
    TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
    TASSADAR_RUST_SOURCE_CANON_REPORT_REF, TassadarArticleAbiCaseStatus,
    TassadarArticleAbiClosureReport, TassadarArticleCpuReproducibilityReport,
    TassadarArticleRuntimeCloseoutReport, TassadarRustOnlyArticleAcceptanceGateV2Report,
    TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustSourceCanonCaseStatus,
    TassadarRustSourceCanonReport,
};
use psionic_models::{
    TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF,
    TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
    TassadarRustArticleProfileCompletenessPublication, TassadarRustArticleProfileRowStatus,
};
use psionic_research::{
    TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF,
    TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF,
    TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF,
    TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF,
    TassadarArticleCpuReproducibilitySummaryReport,
    TassadarHungarian10x10ArticleReproducerReport, TassadarSudoku9x9ArticleReproducerReport,
};

use crate::TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF;

const REQUIRED_SOURCE_CASE_IDS: [&str; 8] = [
    "multi_export_exact",
    "memory_lookup_exact",
    "param_abi_fixture",
    "micro_wasm_article",
    "heap_sum_article",
    "long_loop_article",
    "hungarian_10x10_article",
    "sudoku_9x9_article",
];
const REQUIRED_SUPPORTED_PROFILE_ROW_IDS: [&str; 6] = [
    "module_sections.core_sections_with_memory_globals_data",
    "control_flow.structured_loops_and_bounded_backtracking",
    "tables_globals_indirect_calls.single_funcref_table_mutable_i32_globals",
    "numeric.i32_integer_family",
    "abi.zero_param_exports_and_pointer_length_memory_inputs",
    "abi.direct_scalar_i32_and_pointer_length_single_i32_return",
];
const REQUIRED_REFUSED_PROFILE_ROW_IDS: [&str; 5] = [
    "module_sections.arbitrary_host_imports_and_component_shapes",
    "control_flow.exception_handling_and_general_callstack_control",
    "tables_globals_indirect_calls.multi_table_and_parametric_indirect",
    "numeric.i64_f32_f64_numeric_families",
    "abi.direct_parameter_exports_and_general_return_abi",
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
const REQUIRED_SUPPORTED_MACHINE_CLASS_IDS: [&str; 2] =
    ["host_cpu_aarch64", "host_cpu_x86_64"];
const REQUIRED_UNSUPPORTED_MACHINE_CLASS_ID: &str = "other_host_cpu";

#[derive(Debug, Error)]
pub enum TassadarRustOnlyArticleAcceptanceGateV2Error {
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
}

pub fn build_tassadar_rust_only_article_acceptance_gate_v2_report()
-> Result<TassadarRustOnlyArticleAcceptanceGateV2Report, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    build_tassadar_rust_only_article_acceptance_gate_v2_report_with_repo_root(&repo_root())
}

pub fn build_tassadar_rust_only_article_acceptance_gate_v2_report_with_repo_root(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptanceGateV2Report, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let prerequisites = vec![
        build_source_canon_prerequisite(repo_root)?,
        build_profile_completeness_prerequisite(repo_root)?,
        build_article_abi_prerequisite(repo_root)?,
        build_hungarian_prerequisite(repo_root)?,
        build_sudoku_prerequisite(repo_root)?,
        build_runtime_closeout_prerequisite(repo_root)?,
        build_direct_proof_prerequisite(repo_root)?,
        build_cpu_reproducibility_prerequisite(repo_root)?,
    ];
    Ok(TassadarRustOnlyArticleAcceptanceGateV2Report::from_prerequisites(
        prerequisites,
    ))
}

pub fn tassadar_rust_only_article_acceptance_gate_v2_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF)
}

pub fn write_tassadar_rust_only_article_acceptance_gate_v2_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRustOnlyArticleAcceptanceGateV2Report, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustOnlyArticleAcceptanceGateV2Error::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_only_article_acceptance_gate_v2_report()?;
    let json =
        serde_json::to_string_pretty(&report).expect("Rust-only article acceptance gate serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceGateV2Error::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_source_canon_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF)];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-eval --example tassadar_rust_source_canon_report",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "rust_source_canon",
            artifact_refs,
            validation_commands,
            "missing committed Rust source canon artifact",
            missing,
        ));
    }
    let report: TassadarRustSourceCanonReport =
        read_repo_json(repo_root, TASSADAR_RUST_SOURCE_CANON_REPORT_REF, "rust_source_canon")?;
    let compiled_count = REQUIRED_SOURCE_CASE_IDS
        .iter()
        .filter(|case_id| {
            report.cases.iter().any(|case| {
                case.case_id == **case_id
                    && case.status == TassadarRustSourceCanonCaseStatus::Compiled
            })
        })
        .count();
    Ok(prerequisite_from_bool(
        "rust_source_canon",
        artifact_refs,
        validation_commands,
        compiled_count == REQUIRED_SOURCE_CASE_IDS.len(),
        format!(
            "compiled_cases={}/{} across {:?}",
            compiled_count,
            REQUIRED_SOURCE_CASE_IDS.len(),
            REQUIRED_SOURCE_CASE_IDS,
        ),
    ))
}

fn build_profile_completeness_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(
        TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
    )];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-eval --example tassadar_rust_article_profile_completeness_report",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "profile_completeness",
            artifact_refs,
            validation_commands,
            "missing committed Rust article profile completeness artifact",
            missing,
        ));
    }
    let report: TassadarRustArticleProfileCompletenessPublication = read_repo_json(
        repo_root,
        TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
        "rust_article_profile_completeness",
    )?;
    let supported_rows = REQUIRED_SUPPORTED_PROFILE_ROW_IDS
        .iter()
        .filter(|row_id| {
            report.rows.iter().any(|row| {
                row.row_id == **row_id
                    && row.status == TassadarRustArticleProfileRowStatus::Supported
            })
        })
        .count();
    let refused_rows = REQUIRED_REFUSED_PROFILE_ROW_IDS
        .iter()
        .filter(|row_id| {
            report.rows.iter().any(|row| {
                row.row_id == **row_id
                    && row.status == TassadarRustArticleProfileRowStatus::Refused
            })
        })
        .count();
    let source_case_count = REQUIRED_SOURCE_CASE_IDS
        .iter()
        .filter(|case_id| report.source_case_ids.contains(&String::from(**case_id)))
        .count();
    Ok(prerequisite_from_bool(
        "profile_completeness",
        artifact_refs,
        validation_commands,
        supported_rows == REQUIRED_SUPPORTED_PROFILE_ROW_IDS.len()
            && refused_rows == REQUIRED_REFUSED_PROFILE_ROW_IDS.len()
            && source_case_count == REQUIRED_SOURCE_CASE_IDS.len(),
        format!(
            "supported_rows={}/{} refused_rows={}/{} source_case_ids={}/{}",
            supported_rows,
            REQUIRED_SUPPORTED_PROFILE_ROW_IDS.len(),
            refused_rows,
            REQUIRED_REFUSED_PROFILE_ROW_IDS.len(),
            source_case_count,
            REQUIRED_SOURCE_CASE_IDS.len(),
        ),
    ))
}

fn build_article_abi_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF)];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-eval --example tassadar_article_abi_closure_report",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "article_abi",
            artifact_refs,
            validation_commands,
            "missing committed article ABI closure artifact",
            missing,
        ));
    }
    let report: TassadarArticleAbiClosureReport =
        read_repo_json(repo_root, TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF, "article_abi")?;
    let exact_cases = REQUIRED_ABI_EXACT_CASE_IDS
        .iter()
        .filter(|case_id| {
            report.cases.iter().any(|case| {
                case.case_id == **case_id && case.status == TassadarArticleAbiCaseStatus::Exact
            })
        })
        .count();
    let refusal_cases = REQUIRED_ABI_REFUSAL_CASE_IDS
        .iter()
        .filter(|case_id| {
            report.cases.iter().any(|case| {
                case.case_id == **case_id && case.status == TassadarArticleAbiCaseStatus::Refused
            })
        })
        .count();
    Ok(prerequisite_from_bool(
        "article_abi",
        artifact_refs,
        validation_commands,
        exact_cases == REQUIRED_ABI_EXACT_CASE_IDS.len()
            && refusal_cases == REQUIRED_ABI_REFUSAL_CASE_IDS.len(),
        format!(
            "exact_cases={}/{} refusal_cases={}/{}",
            exact_cases,
            REQUIRED_ABI_EXACT_CASE_IDS.len(),
            refusal_cases,
            REQUIRED_ABI_REFUSAL_CASE_IDS.len(),
        ),
    ))
}

fn build_hungarian_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(
        TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF,
    )];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-research --example tassadar_hungarian_10x10_article_reproducer",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "hungarian_article_reproducer",
            artifact_refs,
            validation_commands,
            "missing committed Hungarian article reproducer artifact",
            missing,
        ));
    }
    let report: TassadarHungarian10x10ArticleReproducerReport = read_repo_json(
        repo_root,
        TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF,
        "hungarian_article_reproducer",
    )?;
    Ok(prerequisite_from_bool(
        "hungarian_article_reproducer",
        artifact_refs,
        validation_commands,
        report.canonical_case_id == "hungarian_10x10_test_a"
            && report.exact_trace_match
            && report.final_output_match
            && report.halt_match
            && !report.direct_execution_posture.fallback_observed
            && !report
                .direct_execution_posture
                .external_tool_surface_observed,
        format!(
            "canonical_case={} exact_trace_match={} final_output_match={} halt_match={} fallback_observed={}",
            report.canonical_case_id,
            report.exact_trace_match,
            report.final_output_match,
            report.halt_match,
            report.direct_execution_posture.fallback_observed,
        ),
    ))
}

fn build_sudoku_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(
        TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF,
    )];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-research --example tassadar_sudoku_9x9_article_reproducer",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "sudoku_article_reproducer",
            artifact_refs,
            validation_commands,
            "missing committed Sudoku article reproducer artifact",
            missing,
        ));
    }
    let report: TassadarSudoku9x9ArticleReproducerReport = read_repo_json(
        repo_root,
        TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF,
        "sudoku_article_reproducer",
    )?;
    let corpus_green = report
        .article_corpus_cases
        .iter()
        .all(|case| case.exact_trace_match && case.final_output_match);
    Ok(prerequisite_from_bool(
        "sudoku_article_reproducer",
        artifact_refs,
        validation_commands,
        report.canonical_case_id == "sudoku_9x9_test_a"
            && report.exact_trace_match
            && report.final_output_match
            && report.halt_match
            && corpus_green
            && !report.direct_execution_posture.fallback_observed
            && !report
                .direct_execution_posture
                .external_tool_surface_observed,
        format!(
            "canonical_case={} corpus_cases={} exact_trace_match={} fallback_observed={}",
            report.canonical_case_id,
            report.article_corpus_cases.len(),
            report.exact_trace_match,
            report.direct_execution_posture.fallback_observed,
        ),
    ))
}

fn build_runtime_closeout_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![
        String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF),
        String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF),
    ];
    let validation_commands = vec![
        String::from("cargo run -p psionic-eval --example tassadar_article_runtime_closeout_report"),
        String::from(
            "cargo run -p psionic-research --example tassadar_article_runtime_closeout_summary",
        ),
    ];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "article_runtime_closeout",
            artifact_refs,
            validation_commands,
            "missing committed article runtime closeout artifact",
            missing,
        ));
    }
    let report: TassadarArticleRuntimeCloseoutReport = read_repo_json(
        repo_root,
        TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF,
        "article_runtime_closeout",
    )?;
    Ok(prerequisite_from_bool(
        "article_runtime_closeout",
        artifact_refs,
        validation_commands,
        report.exact_horizon_count == 4
            && report.floor_pass_count == 4
            && report.floor_refusal_count == 0,
        format!(
            "exact_horizons={} floor_passes={} floor_refusals={} slowest_horizon={}",
            report.exact_horizon_count,
            report.floor_pass_count,
            report.floor_refusal_count,
            report.slowest_workload_horizon_id,
        ),
    ))
}

fn build_direct_proof_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![String::from(
        TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF,
    )];
    let validation_commands = vec![String::from(
        "cargo run -p psionic-serve --example tassadar_direct_model_weight_execution_proof_report",
    )];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "direct_model_weight_execution_proof",
            artifact_refs,
            validation_commands,
            "missing committed direct model-weight proof artifact",
            missing,
        ));
    }
    let report: crate::TassadarDirectModelWeightExecutionProofReport = read_repo_json(
        repo_root,
        TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF,
        "direct_model_weight_execution_proof",
    )?;
    let case_ids_green = REQUIRED_DIRECT_PROOF_CASE_IDS
        .iter()
        .all(|case_id| report.case_ids.contains(&String::from(*case_id)));
    let receipts_green = report.receipts.iter().all(|receipt| {
        !receipt.fallback_observed
            && receipt.external_call_count == 0
            && !receipt.cpu_result_substitution_observed
    });
    Ok(prerequisite_from_bool(
        "direct_model_weight_execution_proof",
        artifact_refs,
        validation_commands,
        report.direct_case_count == 3
            && report.fallback_free_case_count == 3
            && report.zero_external_call_case_count == 3
            && case_ids_green
            && receipts_green,
        format!(
            "direct_cases={} fallback_free_cases={} zero_external_call_cases={}",
            report.direct_case_count,
            report.fallback_free_case_count,
            report.zero_external_call_case_count,
        ),
    ))
}

fn build_cpu_reproducibility_prerequisite(
    repo_root: &Path,
) -> Result<TassadarRustOnlyArticleAcceptancePrerequisite, TassadarRustOnlyArticleAcceptanceGateV2Error>
{
    let artifact_refs = vec![
        String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
        String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF),
    ];
    let validation_commands = vec![
        String::from(
            "cargo run -p psionic-eval --example tassadar_article_cpu_reproducibility_report",
        ),
        String::from(
            "cargo run -p psionic-research --example tassadar_article_cpu_reproducibility_summary",
        ),
    ];
    let missing = missing_artifact_refs(repo_root, &artifact_refs);
    if !missing.is_empty() {
        return Ok(TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            "article_cpu_reproducibility",
            artifact_refs,
            validation_commands,
            "missing committed article CPU reproducibility artifact",
            missing,
        ));
    }
    let report: TassadarArticleCpuReproducibilityReport = read_repo_json(
        repo_root,
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
        "article_cpu_reproducibility",
    )?;
    let summary: TassadarArticleCpuReproducibilitySummaryReport = read_repo_json(
        repo_root,
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF,
        "article_cpu_reproducibility_summary",
    )?;
    let supported_classes_green = REQUIRED_SUPPORTED_MACHINE_CLASS_IDS
        .iter()
        .all(|class_id| report.supported_machine_class_ids.contains(&String::from(*class_id)));
    Ok(prerequisite_from_bool(
        "article_cpu_reproducibility",
        artifact_refs,
        validation_commands,
        report.matrix.current_host_measured_green
            && supported_classes_green
            && report
                .unsupported_machine_class_ids
                .contains(&String::from(REQUIRED_UNSUPPORTED_MACHINE_CLASS_ID))
            && !report.optional_c_path_blocks_rust_only_claim
            && summary.measured_green_machine_class_ids.len() == 1
            && summary.declared_supported_machine_class_ids.len() == 1
            && !summary.optional_c_path_blocks_rust_only_claim,
        format!(
            "current_host={} green_current_host={} supported_classes={} unsupported_classes={}",
            report.matrix.current_host_machine_class_id,
            report.matrix.current_host_measured_green,
            report.supported_machine_class_ids.len(),
            report.unsupported_machine_class_ids.len(),
        ),
    ))
}

fn prerequisite_from_bool(
    prerequisite_id: &str,
    artifact_refs: Vec<String>,
    validation_commands: Vec<String>,
    green: bool,
    detail: String,
) -> TassadarRustOnlyArticleAcceptancePrerequisite {
    if green {
        TassadarRustOnlyArticleAcceptancePrerequisite::passed(
            prerequisite_id,
            artifact_refs,
            validation_commands,
            detail,
        )
    } else {
        TassadarRustOnlyArticleAcceptancePrerequisite::failed(
            prerequisite_id,
            artifact_refs,
            validation_commands,
            detail,
            Vec::new(),
        )
    }
}

fn missing_artifact_refs(repo_root: &Path, artifact_refs: &[String]) -> Vec<String> {
    artifact_refs
        .iter()
        .filter(|artifact_ref| !repo_root.join(artifact_ref).exists())
        .cloned()
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-serve should live under <repo>/crates/psionic-serve")
        .to_path_buf()
}

fn read_repo_json<T: DeserializeOwned>(
    repo_root: &Path,
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarRustOnlyArticleAcceptanceGateV2Error> {
    let path = repo_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceGateV2Error::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRustOnlyArticleAcceptanceGateV2Error::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
        TassadarRustOnlyArticleAcceptanceGateV2Error,
        build_tassadar_rust_only_article_acceptance_gate_v2_report,
        build_tassadar_rust_only_article_acceptance_gate_v2_report_with_repo_root,
        read_repo_json, repo_root, tassadar_rust_only_article_acceptance_gate_v2_report_path,
        write_tassadar_rust_only_article_acceptance_gate_v2_report,
    };
    use psionic_eval::TassadarRustOnlyArticleAcceptanceGateV2Report;

    fn copy_ref(repo_root: &std::path::Path, temp_root: &std::path::Path, relative_ref: &str) {
        let source = repo_root.join(relative_ref);
        let destination = temp_root.join(relative_ref);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::copy(source, destination).expect("copy artifact");
    }

    #[test]
    fn rust_only_article_acceptance_gate_v2_is_green_on_committed_truth() {
        let report =
            build_tassadar_rust_only_article_acceptance_gate_v2_report().expect("gate report");

        assert!(report.green);
        assert_eq!(report.prerequisite_count, 8);
        assert_eq!(report.passed_prerequisite_count, 8);
        assert!(report.failed_prerequisite_ids.is_empty());
    }

    #[test]
    fn rust_only_article_acceptance_gate_v2_matches_committed_truth() {
        let generated =
            build_tassadar_rust_only_article_acceptance_gate_v2_report().expect("gate report");
        let committed: TassadarRustOnlyArticleAcceptanceGateV2Report = read_repo_json(
            &repo_root(),
            TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
            "rust_only_article_acceptance_gate_v2",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn rust_only_article_acceptance_gate_v2_fails_each_missing_prerequisite_individually() {
        let repo_root = repo_root();
        let baseline =
            build_tassadar_rust_only_article_acceptance_gate_v2_report().expect("baseline");
        for prerequisite in &baseline.prerequisites {
            let temp_dir = tempfile::tempdir().expect("tempdir");
            for copied_prerequisite in &baseline.prerequisites {
                for artifact_ref in &copied_prerequisite.artifact_refs {
                    copy_ref(&repo_root, temp_dir.path(), artifact_ref);
                }
            }
            let missing_ref = prerequisite
                .artifact_refs
                .first()
                .expect("artifact ref for prerequisite");
            std::fs::remove_file(temp_dir.path().join(missing_ref)).expect("remove missing ref");

            let report = build_tassadar_rust_only_article_acceptance_gate_v2_report_with_repo_root(
                temp_dir.path(),
            )
            .expect("gate report with missing prerequisite");

            assert!(!report.green, "{} should force a red gate", prerequisite.prerequisite_id);
            assert!(
                report
                    .failed_prerequisite_ids
                    .contains(&prerequisite.prerequisite_id),
                "missing {} should appear in failed ids",
                prerequisite.prerequisite_id,
            );
        }
    }

    #[test]
    fn write_rust_only_article_acceptance_gate_v2_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_rust_only_article_acceptance_gate_v2.json");
        let written = write_tassadar_rust_only_article_acceptance_gate_v2_report(&output_path)
            .expect("write report");
        let persisted: TassadarRustOnlyArticleAcceptanceGateV2Report =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_rust_only_article_acceptance_gate_v2_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_rust_only_article_acceptance_gate_v2.json")
        );
    }

    #[test]
    fn missing_decode_surfaces_as_error() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let bogus_path = temp_dir
            .path()
            .join(super::TASSADAR_RUST_SOURCE_CANON_REPORT_REF);
        std::fs::create_dir_all(bogus_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&bogus_path, b"{not-json").expect("write bogus");
        let err = build_tassadar_rust_only_article_acceptance_gate_v2_report_with_repo_root(
            temp_dir.path(),
        )
        .expect_err("bogus json should error");
        assert!(matches!(
            err,
            TassadarRustOnlyArticleAcceptanceGateV2Error::Decode { .. }
        ));
    }
}
