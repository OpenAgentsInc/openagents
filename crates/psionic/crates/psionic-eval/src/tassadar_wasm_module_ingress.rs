use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarWasmModuleArtifactBundle, TassadarWasmModuleArtifactBundleError,
    compile_tassadar_wasm_binary_module_to_artifact_bundle,
};
use psionic_ir::{
    encode_tassadar_normalized_wasm_module, parse_tassadar_normalized_wasm_module,
    tassadar_seeded_multi_function_module,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarWasmBinarySummary,
    TassadarWasmProfile, summarize_tassadar_wasm_binary, tassadar_canonical_wasm_binary_path,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_WASM_MODULE_INGRESS_REPORT_SCHEMA_VERSION: u16 = 1;
const SEEDED_MULTI_FUNCTION_MODULE_REF: &str = "synthetic://tassadar/wasm/multi_function_v1";
pub const TASSADAR_WASM_MODULE_INGRESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wasm_module_ingress_report.json";

/// Lowering outcome for one Wasm-module ingress case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWasmModuleIngressCaseStatus {
    /// The module lowered into runnable exports and matched the CPU reference.
    LoweredExact,
    /// The module parsed and normalized, but lowering refused explicitly.
    LoweringRefused,
}

/// One repo-facing Wasm-module ingress case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmModuleIngressCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable source ref for the underlying module bytes.
    pub source_ref: String,
    /// Runtime-facing structural summary over the original module bytes.
    pub runtime_summary: TassadarWasmBinarySummary,
    /// Stable digest over the source Wasm bytes.
    pub wasm_binary_digest: String,
    /// Stable digest over the normalized module IR.
    pub normalized_module_digest: String,
    /// Stable digest over the canonical re-encoded Wasm bytes.
    pub roundtrip_wasm_digest: String,
    /// Stable lowered or refused status.
    pub status: TassadarWasmModuleIngressCaseStatus,
    /// Exported function names in normalized-module order.
    pub function_export_names: Vec<String>,
    /// Memory count in the normalized module.
    pub memory_count: usize,
    /// Data-segment count in the normalized module.
    pub data_segment_count: usize,
    /// Lowered function-export names when lowering succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_export_names: Vec<String>,
    /// Exact outputs by lowered export when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exact_outputs_by_export: BTreeMap<String, Vec<i32>>,
    /// Lowered bundle digest when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_bundle_digest: Option<String>,
    /// Machine-readable refusal kind when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowering_refusal_kind: Option<String>,
    /// Human-readable refusal detail when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowering_refusal_detail: Option<String>,
}

/// Committed report over the bounded Wasm-module ingress lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmModuleIngressReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable refs used to assemble the report.
    pub generated_from_refs: Vec<String>,
    /// Ordered ingress cases.
    pub cases: Vec<TassadarWasmModuleIngressCase>,
    /// Explicit claim boundary for the report.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarWasmModuleIngressReport {
    fn new(cases: Vec<TassadarWasmModuleIngressCase>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_WASM_MODULE_INGRESS_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.wasm_module_ingress_report.v1"),
            generated_from_refs: vec![
                String::from("fixtures/tassadar/wasm/tassadar_micro_wasm_kernel.wasm"),
                String::from(SEEDED_MULTI_FUNCTION_MODULE_REF),
            ],
            cases,
            claim_boundary: String::from(
                "this report records bounded normalized Wasm-module ingress only: one real committed binary that parses but still refuses current runtime lowering, plus one canonical multi-function module that round-trips and executes exactly; it does not imply arbitrary Wasm closure, call-frame support, structured control-flow support, or byte-addressed memory ABI completion",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_wasm_module_ingress_report|", &report);
        report
    }
}

/// Wasm-module ingress report failures.
#[derive(Debug, Error)]
pub enum TassadarWasmModuleIngressReportError {
    /// Normalized-module parsing or encoding failed.
    #[error(transparent)]
    Module(#[from] psionic_ir::TassadarNormalizedWasmModuleError),
    /// Lowering failed for the exact seeded case.
    #[error(transparent)]
    Compiler(#[from] TassadarWasmModuleArtifactBundleError),
    /// Replaying one lowered export failed.
    #[error("failed to replay lowered export `{export_name}` for case `{case_id}`: {error}")]
    Replay {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
        /// Runtime refusal.
        error: TassadarExecutionRefusal,
    },
    /// One lowered export diverged from its committed execution manifest.
    #[error(
        "lowered export `{export_name}` for case `{case_id}` diverged from its execution manifest"
    )]
    ExactnessMismatch {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
    },
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write Wasm-module ingress report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to read a committed report.
    #[error("failed to read committed Wasm-module ingress report `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to decode a committed report.
    #[error("failed to decode committed Wasm-module ingress report `{path}`: {error}")]
    Decode {
        /// File path.
        path: String,
        /// JSON error.
        error: serde_json::Error,
    },
}

/// Builds the committed Wasm-module ingress report.
pub fn build_tassadar_wasm_module_ingress_report()
-> Result<TassadarWasmModuleIngressReport, TassadarWasmModuleIngressReportError> {
    let canonical_bytes = fs::read(tassadar_canonical_wasm_binary_path()).map_err(|error| {
        TassadarWasmModuleIngressReportError::Read {
            path: tassadar_canonical_wasm_binary_path().display().to_string(),
            error,
        }
    })?;
    let canonical_case = build_case_from_bytes(
        "canonical_micro_wasm_kernel",
        "fixtures/tassadar/wasm/tassadar_micro_wasm_kernel.wasm",
        &canonical_bytes,
        true,
    )?;

    let seeded_module = tassadar_seeded_multi_function_module()?;
    let seeded_bytes = encode_tassadar_normalized_wasm_module(&seeded_module)?;
    let seeded_case = build_case_from_bytes(
        "seeded_multi_function_module",
        SEEDED_MULTI_FUNCTION_MODULE_REF,
        &seeded_bytes,
        false,
    )?;

    Ok(TassadarWasmModuleIngressReport::new(vec![
        canonical_case,
        seeded_case,
    ]))
}

/// Returns the canonical absolute path for the committed Wasm-module ingress
/// report.
pub fn tassadar_wasm_module_ingress_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WASM_MODULE_INGRESS_REPORT_REF)
}

/// Writes the committed Wasm-module ingress report.
pub fn write_tassadar_wasm_module_ingress_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWasmModuleIngressReport, TassadarWasmModuleIngressReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWasmModuleIngressReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_wasm_module_ingress_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("Wasm-module ingress report should serialize");
    fs::write(output_path, bytes).map_err(|error| TassadarWasmModuleIngressReportError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn build_case_from_bytes(
    case_id: &str,
    source_ref: &str,
    wasm_bytes: &[u8],
    expect_refusal: bool,
) -> Result<TassadarWasmModuleIngressCase, TassadarWasmModuleIngressReportError> {
    let runtime_summary = summarize_tassadar_wasm_binary(wasm_bytes).map_err(|detail| {
        TassadarWasmModuleIngressReportError::Module(
            psionic_ir::TassadarNormalizedWasmModuleError::MalformedBinary { message: detail },
        )
    })?;
    let normalized_module = parse_tassadar_normalized_wasm_module(wasm_bytes)?;
    let roundtrip_wasm_digest =
        stable_bytes_digest(&encode_tassadar_normalized_wasm_module(&normalized_module)?);
    let lowering = compile_tassadar_wasm_binary_module_to_artifact_bundle(
        source_ref,
        wasm_bytes,
        &TassadarWasmProfile::article_i32_compute_v1(),
    );

    let mut case = TassadarWasmModuleIngressCase {
        case_id: String::from(case_id),
        source_ref: String::from(source_ref),
        runtime_summary,
        wasm_binary_digest: stable_bytes_digest(wasm_bytes),
        normalized_module_digest: normalized_module.module_digest.clone(),
        roundtrip_wasm_digest,
        status: TassadarWasmModuleIngressCaseStatus::LoweringRefused,
        function_export_names: normalized_module.exported_function_names(),
        memory_count: normalized_module.memories.len(),
        data_segment_count: normalized_module.data_segments.len(),
        lowered_export_names: Vec::new(),
        exact_outputs_by_export: BTreeMap::new(),
        artifact_bundle_digest: None,
        lowering_refusal_kind: None,
        lowering_refusal_detail: None,
    };

    match lowering {
        Ok(bundle) => {
            let outputs = exact_outputs_for_bundle(case_id, &bundle)?;
            case.status = TassadarWasmModuleIngressCaseStatus::LoweredExact;
            case.lowered_export_names = bundle
                .lowered_exports
                .iter()
                .map(|artifact| artifact.export_name.clone())
                .collect();
            case.exact_outputs_by_export = outputs;
            case.artifact_bundle_digest = Some(bundle.bundle_digest);
        }
        Err(error) => {
            case.status = TassadarWasmModuleIngressCaseStatus::LoweringRefused;
            case.lowering_refusal_kind = Some(refusal_kind(&error));
            case.lowering_refusal_detail = Some(error.to_string());
        }
    }

    if expect_refusal && case.status != TassadarWasmModuleIngressCaseStatus::LoweringRefused {
        return Err(TassadarWasmModuleIngressReportError::ExactnessMismatch {
            case_id: String::from(case_id),
            export_name: String::from("expected_refusal"),
        });
    }

    Ok(case)
}

fn exact_outputs_for_bundle(
    case_id: &str,
    bundle: &TassadarWasmModuleArtifactBundle,
) -> Result<BTreeMap<String, Vec<i32>>, TassadarWasmModuleIngressReportError> {
    let mut outputs = BTreeMap::new();
    for artifact in &bundle.lowered_exports {
        let execution =
            TassadarCpuReferenceRunner::for_program(&artifact.program_artifact.validated_program)
                .expect("lowered module-export program should always select a CPU runner")
                .execute(&artifact.program_artifact.validated_program)
                .map_err(|error| TassadarWasmModuleIngressReportError::Replay {
                    case_id: String::from(case_id),
                    export_name: artifact.export_name.clone(),
                    error,
                })?;
        if execution.outputs != artifact.execution_manifest.expected_outputs
            || execution.final_memory != artifact.execution_manifest.expected_final_memory
        {
            return Err(TassadarWasmModuleIngressReportError::ExactnessMismatch {
                case_id: String::from(case_id),
                export_name: artifact.export_name.clone(),
            });
        }
        outputs.insert(artifact.export_name.clone(), execution.outputs);
    }
    Ok(outputs)
}

fn refusal_kind(error: &TassadarWasmModuleArtifactBundleError) -> String {
    match error {
        TassadarWasmModuleArtifactBundleError::UnsupportedParamCount { .. } => {
            String::from("unsupported_param_count")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedResultTypes { .. } => {
            String::from("unsupported_result_types")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedInstruction { .. } => {
            String::from("unsupported_instruction")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedCall { .. } => {
            String::from("unsupported_call")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedDynamicMemoryAddress { .. } => {
            String::from("unsupported_dynamic_memory_address")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedMemoryImmediate { .. } => {
            String::from("unsupported_memory_immediate")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedMemoryShape { .. } => {
            String::from("unsupported_memory_shape")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment { .. } => {
            String::from("unsupported_data_segment")
        }
        TassadarWasmModuleArtifactBundleError::ExportedImportUnsupported { .. } => {
            String::from("exported_import_unsupported")
        }
        TassadarWasmModuleArtifactBundleError::NoFunctionExports { .. } => {
            String::from("no_function_exports")
        }
        TassadarWasmModuleArtifactBundleError::Execution { .. } => String::from("runtime_refusal"),
        TassadarWasmModuleArtifactBundleError::ProgramArtifact { .. } => {
            String::from("program_artifact_error")
        }
        TassadarWasmModuleArtifactBundleError::Module(_) => String::from("module_error"),
        TassadarWasmModuleArtifactBundleError::UnsupportedTraceAbi { .. } => {
            String::from("unsupported_trace_abi")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedLocalType { .. } => {
            String::from("unsupported_local_type")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedLocalIndex { .. } => {
            String::from("unsupported_local_index")
        }
        TassadarWasmModuleArtifactBundleError::UnsupportedDrop { .. } => {
            String::from("unsupported_drop")
        }
        TassadarWasmModuleArtifactBundleError::InvalidStackState { .. } => {
            String::from("invalid_stack_state")
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        TassadarWasmModuleIngressCaseStatus, TassadarWasmModuleIngressReport,
        build_tassadar_wasm_module_ingress_report, tassadar_wasm_module_ingress_report_path,
        write_tassadar_wasm_module_ingress_report,
    };

    #[test]
    fn wasm_module_ingress_report_captures_refusal_and_exact_bundle() {
        let report =
            build_tassadar_wasm_module_ingress_report().expect("ingress report should build");
        assert_eq!(report.cases.len(), 2);
        assert_eq!(
            report.cases[0].status,
            TassadarWasmModuleIngressCaseStatus::LoweringRefused
        );
        assert_eq!(
            report.cases[0].lowering_refusal_kind.as_deref(),
            Some("unsupported_param_count")
        );
        assert_eq!(
            report.cases[1].status,
            TassadarWasmModuleIngressCaseStatus::LoweredExact
        );
        assert_eq!(
            report.cases[1].exact_outputs_by_export.get("pair_sum"),
            Some(&vec![5])
        );
        assert_eq!(
            report.cases[1].exact_outputs_by_export.get("local_double"),
            Some(&vec![14])
        );
    }

    #[test]
    fn wasm_module_ingress_report_matches_committed_truth() {
        let report =
            build_tassadar_wasm_module_ingress_report().expect("ingress report should build");
        let path = tassadar_wasm_module_ingress_report_path();
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read committed Wasm-module ingress report `{}`: {error}",
                path.display()
            )
        });
        let persisted: TassadarWasmModuleIngressReport = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| {
                panic!(
                    "failed to decode committed Wasm-module ingress report `{}`: {error}",
                    path.display()
                )
            });
        assert_eq!(persisted, report);
    }

    #[test]
    fn write_wasm_module_ingress_report_persists_current_truth() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let output_path = temp_dir
            .path()
            .join("tassadar_wasm_module_ingress_report.json");
        let report = write_tassadar_wasm_module_ingress_report(&output_path)
            .expect("ingress report should write");
        let bytes = fs::read(&output_path).unwrap_or_else(|error| {
            panic!(
                "failed to read persisted Wasm-module ingress report `{}`: {error}",
                output_path.display()
            )
        });
        let persisted: TassadarWasmModuleIngressReport = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| {
                panic!(
                    "failed to decode persisted Wasm-module ingress report `{}`: {error}",
                    output_path.display()
                )
            });
        assert_eq!(persisted, report);
    }
}
