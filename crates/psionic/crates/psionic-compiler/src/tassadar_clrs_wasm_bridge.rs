use std::{fs, path::PathBuf};

use psionic_runtime::TassadarWasmProfile;
use thiserror::Error;

use crate::{
    compile_tassadar_wasm_text_to_artifact_bundle, TassadarWasmTextArtifactBundlePipeline,
    TassadarWasmTextArtifactBundlePipelineError, TassadarWasmTextCompileConfig,
};

/// One exported length bucket in the public CLRS-to-Wasm bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarClrsWasmBridgeExportSpec {
    /// Stable length-bucket identifier.
    pub length_bucket_id: &'static str,
    /// Export symbol implementing the bucket.
    pub export_symbol: &'static str,
}

/// One compiler-facing CLRS-to-Wasm bridge case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarClrsWasmBridgeCaseSpec {
    /// Stable case identifier.
    pub case_id: &'static str,
    /// Stable CLRS algorithm identifier.
    pub algorithm_id: &'static str,
    /// Stable trajectory-family identifier.
    pub trajectory_family_id: &'static str,
    /// Human-readable case summary.
    pub summary: &'static str,
    /// Repo-relative source ref.
    pub source_ref: &'static str,
    /// Source fixture file name.
    pub source_file_name: &'static str,
    /// Wasm output fixture file name.
    pub wasm_file_name: &'static str,
    /// Length-bucket exports implemented by the case.
    pub export_specs: Vec<TassadarClrsWasmBridgeExportSpec>,
}

impl TassadarClrsWasmBridgeCaseSpec {
    /// Returns the absolute source-fixture path for the case.
    #[must_use]
    pub fn source_path(&self) -> PathBuf {
        fixture_source_path(self.source_file_name)
    }

    /// Returns the absolute compiled-Wasm fixture path for the case.
    #[must_use]
    pub fn wasm_path(&self) -> PathBuf {
        fixture_wasm_path(self.wasm_file_name)
    }

    /// Returns the compile configuration implied by the declared export set.
    #[must_use]
    pub fn compile_config(&self) -> TassadarWasmTextCompileConfig {
        TassadarWasmTextCompileConfig {
            export_symbols: self
                .export_specs
                .iter()
                .map(|export| String::from(export.export_symbol))
                .collect(),
        }
    }
}

/// Compiler-side CLRS-to-Wasm bridge build failure.
#[derive(Debug, Error)]
pub enum TassadarClrsWasmBridgeCompileError {
    /// The requested case id was unknown.
    #[error("unknown CLRS-to-Wasm bridge case `{case_id}`")]
    UnknownCase {
        /// Requested case identifier.
        case_id: String,
    },
    /// Failed to read the WAT source fixture.
    #[error("failed to read CLRS-to-Wasm bridge source `{path}`: {error}")]
    ReadSource {
        /// Source path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// The bounded Wasm-text pipeline refused or failed explicitly.
    #[error(transparent)]
    Pipeline(#[from] TassadarWasmTextArtifactBundlePipelineError),
}

/// Returns the current compiler-facing CLRS-to-Wasm bridge cases.
#[must_use]
pub fn tassadar_clrs_wasm_bridge_case_specs() -> Vec<TassadarClrsWasmBridgeCaseSpec> {
    vec![
        TassadarClrsWasmBridgeCaseSpec {
            case_id: "clrs_shortest_path_sequential",
            algorithm_id: "shortest_path",
            trajectory_family_id: "sequential_relaxation",
            summary: "fixed shortest-path witness compiled as sequential relaxation over tiny and small graph buckets",
            source_ref: "fixtures/tassadar/sources/tassadar_clrs_shortest_path_sequential.wat",
            source_file_name: "tassadar_clrs_shortest_path_sequential.wat",
            wasm_file_name: "tassadar_clrs_shortest_path_sequential.wasm",
            export_specs: vec![
                TassadarClrsWasmBridgeExportSpec {
                    length_bucket_id: "tiny",
                    export_symbol: "distance_tiny",
                },
                TassadarClrsWasmBridgeExportSpec {
                    length_bucket_id: "small",
                    export_symbol: "distance_small",
                },
            ],
        },
        TassadarClrsWasmBridgeCaseSpec {
            case_id: "clrs_shortest_path_wavefront",
            algorithm_id: "shortest_path",
            trajectory_family_id: "wavefront_relaxation",
            summary: "fixed shortest-path witness compiled as wavefront relaxation over the same tiny and small graph buckets",
            source_ref: "fixtures/tassadar/sources/tassadar_clrs_shortest_path_wavefront.wat",
            source_file_name: "tassadar_clrs_shortest_path_wavefront.wat",
            wasm_file_name: "tassadar_clrs_shortest_path_wavefront.wasm",
            export_specs: vec![
                TassadarClrsWasmBridgeExportSpec {
                    length_bucket_id: "tiny",
                    export_symbol: "distance_tiny",
                },
                TassadarClrsWasmBridgeExportSpec {
                    length_bucket_id: "small",
                    export_symbol: "distance_small",
                },
            ],
        },
    ]
}

/// Finds one CLRS-to-Wasm bridge case by id.
pub fn tassadar_clrs_wasm_bridge_case_spec(
    case_id: &str,
) -> Result<TassadarClrsWasmBridgeCaseSpec, TassadarClrsWasmBridgeCompileError> {
    tassadar_clrs_wasm_bridge_case_specs()
        .into_iter()
        .find(|spec| spec.case_id == case_id)
        .ok_or_else(|| TassadarClrsWasmBridgeCompileError::UnknownCase {
            case_id: String::from(case_id),
        })
}

/// Compiles one CLRS-to-Wasm bridge case through the bounded Wasm-text lane.
pub fn compile_tassadar_clrs_wasm_bridge_case(
    spec: &TassadarClrsWasmBridgeCaseSpec,
    profile: &TassadarWasmProfile,
) -> Result<TassadarWasmTextArtifactBundlePipeline, TassadarClrsWasmBridgeCompileError> {
    let source_path = spec.source_path();
    let source_bytes =
        fs::read(&source_path).map_err(|error| TassadarClrsWasmBridgeCompileError::ReadSource {
            path: source_path.display().to_string(),
            error,
        })?;
    let source_text =
        std::str::from_utf8(&source_bytes).expect("CLRS-to-Wasm WAT fixture should be valid UTF-8");
    Ok(compile_tassadar_wasm_text_to_artifact_bundle(
        spec.source_ref,
        source_text,
        spec.wasm_path(),
        &spec.compile_config(),
        profile,
    )?)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_source_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures/tassadar/sources")
        .join(file_name)
}

fn fixture_wasm_path(file_name: &str) -> PathBuf {
    repo_root().join("fixtures/tassadar/wasm").join(file_name)
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{TassadarCpuReferenceRunner, TassadarWasmProfile};

    use super::{compile_tassadar_clrs_wasm_bridge_case, tassadar_clrs_wasm_bridge_case_specs};

    #[test]
    fn clrs_wasm_bridge_cases_compile_exactly() {
        let profile = TassadarWasmProfile::article_i32_compute_v1();
        for spec in tassadar_clrs_wasm_bridge_case_specs() {
            let pipeline = compile_tassadar_clrs_wasm_bridge_case(&spec, &profile)
                .expect("case should compile");
            for artifact in &pipeline.artifact_bundle.lowered_exports {
                let execution = TassadarCpuReferenceRunner::for_program(
                    &artifact.program_artifact.validated_program,
                )
                .expect("lowered program should select a runner")
                .execute(&artifact.program_artifact.validated_program)
                .expect("lowered program should replay exactly");
                assert_eq!(
                    execution.outputs,
                    artifact.execution_manifest.expected_outputs
                );
            }
        }
    }
}
