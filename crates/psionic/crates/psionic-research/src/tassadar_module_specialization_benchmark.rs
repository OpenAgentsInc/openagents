use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::TassadarExecutorFixture;
use psionic_runtime::{
    tassadar_seeded_module_specialization_call_graph_module,
    tassadar_seeded_module_specialization_import_boundary_module,
    tassadar_seeded_module_specialization_memory_call_graph_module,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_FILE: &str =
    "tassadar_module_specialization_benchmark.json";
pub const TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_specialization_benchmark.json";
pub const TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_module_specialization_benchmark";
pub const TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_TEST_COMMAND: &str = "cargo test -p psionic-research module_specialization_benchmark_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationBenchmarkExportReport {
    pub export_name: String,
    pub program_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub runtime_contract_digest: String,
    pub expected_trace_digest: String,
    pub compiled_trace_digest: String,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub final_memory_match: bool,
    pub halt_match: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationBenchmarkModuleReport {
    pub module_id: String,
    pub module_digest: String,
    pub function_count: u32,
    pub export_count: u32,
    pub call_edge_count: u32,
    pub imported_function_count: u32,
    pub baseline_total_compiled_weight_artifact_bytes: u64,
    pub module_specialized_artifact_bytes: u64,
    pub module_specialized_over_unspecialized_size_ratio: f64,
    pub baseline_dispatch_cost_units: u64,
    pub module_specialized_dispatch_cost_units: u64,
    pub estimated_module_specialized_over_unspecialized_throughput_ratio: f64,
    pub exact_trace_export_count: u32,
    pub final_output_match_export_count: u32,
    pub final_memory_match_export_count: u32,
    pub halt_match_export_count: u32,
    pub proof_lineage_verified: bool,
    pub export_reports: Vec<TassadarModuleSpecializationBenchmarkExportReport>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationBenchmarkRefusalReport {
    pub module_id: String,
    pub module_digest: String,
    pub refusal_kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationBenchmarkReport {
    pub schema_version: u16,
    pub benchmark_suite_id: String,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub module_reports: Vec<TassadarModuleSpecializationBenchmarkModuleReport>,
    pub refusal_reports: Vec<TassadarModuleSpecializationBenchmarkRefusalReport>,
    pub claim_boundary: String,
    pub detail: String,
    pub report_digest: String,
}

impl TassadarModuleSpecializationBenchmarkReport {
    fn new(
        module_reports: Vec<TassadarModuleSpecializationBenchmarkModuleReport>,
        refusal_reports: Vec<TassadarModuleSpecializationBenchmarkRefusalReport>,
    ) -> Self {
        let total_supported = module_reports.len();
        let total_refused = refusal_reports.len();
        let average_size_ratio = round_metric(
            module_reports
                .iter()
                .map(|report| report.module_specialized_over_unspecialized_size_ratio)
                .sum::<f64>()
                / total_supported.max(1) as f64,
        );
        let average_throughput_ratio = round_metric(
            module_reports
                .iter()
                .map(|report| {
                    report.estimated_module_specialized_over_unspecialized_throughput_ratio
                })
                .sum::<f64>()
                / total_supported.max(1) as f64,
        );
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            benchmark_suite_id: String::from("tassadar.module_specialization_benchmark.v0"),
            report_ref: String::from(TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_REF),
            regeneration_commands: vec![
                String::from(TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_EXAMPLE_COMMAND),
                String::from(TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_TEST_COMMAND),
            ],
            module_reports,
            refusal_reports,
            claim_boundary: String::from(
                "this report compares a research-only shared module-specialization artifact against today's per-export program-specialized compiled lane on the same bounded multi-function modules; throughput is represented as deterministic dispatch-cost units rather than wall-clock runtime, and import-boundary cases remain explicit refusal evidence instead of silent fallback",
            ),
            detail: format!(
                "supported_module_count={total_supported}; refusal_count={total_refused}; average_size_ratio={average_size_ratio}; average_estimated_throughput_ratio={average_throughput_ratio}"
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_module_specialization_benchmark_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarModuleSpecializationBenchmarkError {
    #[error(transparent)]
    Module(#[from] psionic_ir::TassadarNormalizedWasmModuleError),
    #[error(transparent)]
    Compile(#[from] psionic_models::TassadarCompiledModuleSpecializationError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
}

pub fn build_tassadar_module_specialization_benchmark_report()
-> Result<TassadarModuleSpecializationBenchmarkReport, TassadarModuleSpecializationBenchmarkError> {
    let fixture = TassadarExecutorFixture::core_i32_v2();

    let supported_modules = vec![
        (
            String::from("seeded_multi_function_module"),
            psionic_ir::tassadar_seeded_multi_function_module()?,
        ),
        (
            String::from("seeded_call_graph_module"),
            tassadar_seeded_module_specialization_call_graph_module()?,
        ),
        (
            String::from("seeded_memory_call_graph_module"),
            tassadar_seeded_module_specialization_memory_call_graph_module()?,
        ),
    ];

    let mut module_reports = Vec::new();
    for (module_id, module) in supported_modules {
        let compiled = fixture.compile_module_specialization(
            format!("module_specialization.{module_id}"),
            module_id.clone(),
            module,
        )?;
        let mut export_reports = Vec::new();
        let mut baseline_dispatch_cost_units = 0u64;
        let mut module_specialized_dispatch_cost_units = 0u64;
        for export in &compiled.artifact().exports {
            let export_summary = compiled
                .bundle()
                .specialization_plan
                .export_summaries
                .iter()
                .find(|summary| summary.export_name == export.export_name)
                .expect("compiled export should map back to bundle summary");
            let execution_manifest = compiled
                .export_deployments()
                .iter()
                .find(|deployment| deployment.lowered_export().export_name == export.export_name)
                .expect("compiled export should map back to lowered deployment")
                .lowered_export()
                .execution_manifest
                .clone();
            baseline_dispatch_cost_units = baseline_dispatch_cost_units
                .saturating_add(execution_manifest.trace_step_count.saturating_add(1));
            module_specialized_dispatch_cost_units = module_specialized_dispatch_cost_units
                .saturating_add(
                    execution_manifest
                        .trace_step_count
                        .saturating_add(1)
                        .saturating_add(compiled.artifact().exports.len() as u64)
                        .saturating_add(export_summary.reachable_function_indices.len() as u64)
                        .saturating_add(export_summary.direct_call_edge_count as u64)
                        .saturating_add(u64::from(
                            !export_summary.reachable_import_refs.is_empty(),
                        )),
                );
            export_reports.push(TassadarModuleSpecializationBenchmarkExportReport {
                export_name: export.export_name.clone(),
                program_digest: export.program_digest.clone(),
                compiled_weight_artifact_digest: export.compiled_weight_artifact_digest.clone(),
                runtime_contract_digest: export.runtime_contract_digest.clone(),
                expected_trace_digest: export.expected_trace_digest.clone(),
                compiled_trace_digest: export.compiled_trace_digest.clone(),
                exact_trace_match: export.exact_trace_match,
                final_output_match: export.final_output_match,
                final_memory_match: export.final_memory_match,
                halt_match: export.halt_match,
            });
        }
        export_reports.sort_by(|left, right| left.export_name.cmp(&right.export_name));

        let size_ratio = round_metric(
            compiled.artifact().compiled_module_weight_artifact_bytes as f64
                / compiled
                    .total_unspecialized_compiled_weight_artifact_bytes()
                    .max(1) as f64,
        );
        let throughput_ratio = round_metric(
            baseline_dispatch_cost_units as f64
                / module_specialized_dispatch_cost_units.max(1) as f64,
        );
        let proof_lineage_verified = compiled.artifact().exports.iter().all(|export| {
            !export.compile_runtime_manifest_digest.is_empty()
                && !export.compile_trace_proof_digest.is_empty()
                && !export.compile_execution_proof_bundle_digest.is_empty()
                && !export.runtime_execution_proof_bundle_digest.is_empty()
                && !export.compiled_runtime_manifest_digest.is_empty()
                && !export.compiled_runtime_trace_proof_digest.is_empty()
        });
        module_reports.push(TassadarModuleSpecializationBenchmarkModuleReport {
            module_id,
            module_digest: compiled.bundle().normalized_module.module_digest.clone(),
            function_count: compiled.bundle().specialization_plan.function_count,
            export_count: compiled.bundle().specialization_plan.export_count,
            call_edge_count: compiled.bundle().specialization_plan.call_edges.len() as u32,
            imported_function_count: compiled.bundle().specialization_plan.imported_function_count,
            baseline_total_compiled_weight_artifact_bytes: compiled
                .total_unspecialized_compiled_weight_artifact_bytes(),
            module_specialized_artifact_bytes: compiled
                .artifact()
                .compiled_module_weight_artifact_bytes,
            module_specialized_over_unspecialized_size_ratio: size_ratio,
            baseline_dispatch_cost_units,
            module_specialized_dispatch_cost_units,
            estimated_module_specialized_over_unspecialized_throughput_ratio: throughput_ratio,
            exact_trace_export_count: compiled
                .artifact()
                .exports
                .iter()
                .filter(|export| export.exact_trace_match)
                .count() as u32,
            final_output_match_export_count: compiled
                .artifact()
                .exports
                .iter()
                .filter(|export| export.final_output_match)
                .count() as u32,
            final_memory_match_export_count: compiled
                .artifact()
                .exports
                .iter()
                .filter(|export| export.final_memory_match)
                .count() as u32,
            halt_match_export_count: compiled
                .artifact()
                .exports
                .iter()
                .filter(|export| export.halt_match)
                .count() as u32,
            proof_lineage_verified,
            export_reports,
            detail: format!(
                "size_ratio={size_ratio}; estimated_throughput_ratio={throughput_ratio}; proof_lineage_verified={proof_lineage_verified}"
            ),
        });
    }
    module_reports.sort_by(|left, right| left.module_id.cmp(&right.module_id));

    let import_boundary_module = tassadar_seeded_module_specialization_import_boundary_module()?;
    let refusal_reports = match fixture.compile_module_specialization(
        "module_specialization.seeded_import_boundary_module",
        "seeded_import_boundary_module",
        import_boundary_module.clone(),
    ) {
        Ok(_) => Vec::new(),
        Err(error) => vec![TassadarModuleSpecializationBenchmarkRefusalReport {
            module_id: String::from("seeded_import_boundary_module"),
            module_digest: import_boundary_module.module_digest.clone(),
            refusal_kind: refusal_kind(&error),
            detail: error.to_string(),
        }],
    };

    Ok(TassadarModuleSpecializationBenchmarkReport::new(
        module_reports,
        refusal_reports,
    ))
}

pub fn run_tassadar_module_specialization_benchmark_report(
    output_dir: &Path,
) -> Result<TassadarModuleSpecializationBenchmarkReport, TassadarModuleSpecializationBenchmarkError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarModuleSpecializationBenchmarkError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_module_specialization_benchmark_report()?;
    write_json(
        output_dir.join(TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_FILE),
        &report,
    )?;
    Ok(report)
}

fn refusal_kind(error: &psionic_models::TassadarCompiledModuleSpecializationError) -> String {
    match error {
        psionic_models::TassadarCompiledModuleSpecializationError::Specialization(error) => {
            error.kind_slug().to_string()
        }
        psionic_models::TassadarCompiledModuleSpecializationError::Compiled(_) => {
            String::from("compiled_program")
        }
        psionic_models::TassadarCompiledModuleSpecializationError::Json(_) => String::from("json"),
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn write_json<T: Serialize>(
    path: PathBuf,
    value: &T,
) -> Result<(), TassadarModuleSpecializationBenchmarkError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| TassadarModuleSpecializationBenchmarkError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn research_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate directory should have workspace parent")
        .parent()
        .expect("workspace crates directory should have repo root parent")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_REF,
        build_tassadar_module_specialization_benchmark_report,
    };

    #[test]
    fn module_specialization_benchmark_report_covers_supported_and_refused_modules()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_module_specialization_benchmark_report()?;
        assert_eq!(report.module_reports.len(), 3);
        assert_eq!(report.refusal_reports.len(), 1);
        assert!(
            report
                .module_reports
                .iter()
                .all(|module| module.module_specialized_artifact_bytes > 0)
        );
        assert!(
            report
                .module_reports
                .iter()
                .all(|module| module.proof_lineage_verified)
        );
        Ok(())
    }

    #[test]
    fn module_specialization_benchmark_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_module_specialization_benchmark_report()?;
        let bytes = std::fs::read(
            super::research_repo_root().join(TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_REPORT_REF),
        )?;
        let committed: super::TassadarModuleSpecializationBenchmarkReport =
            serde_json::from_slice(&bytes)?;
        assert_eq!(report, committed);
        Ok(())
    }
}
