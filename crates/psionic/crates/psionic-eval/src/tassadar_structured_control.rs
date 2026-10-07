use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarStructuredControlBundleError,
    compile_tassadar_wasm_binary_module_to_structured_control_bundle,
    tassadar_seeded_structured_control_branch_table_module,
    tassadar_seeded_structured_control_if_else_module,
    tassadar_seeded_structured_control_invalid_label_module,
    tassadar_seeded_structured_control_loop_module,
};
use psionic_runtime::execute_tassadar_structured_control_program;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_STRUCTURED_CONTROL_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_STRUCTURED_CONTROL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_structured_control_report.json";

/// Repo-facing status for one structured-control conformance case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuredControlCaseStatus {
    /// Lowering succeeded and replay matched the CPU reference manifest.
    Exact,
    /// Lowering refused explicitly.
    Refused,
}

/// One repo-facing structured-control conformance case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable source ref for the underlying Wasm bytes.
    pub source_ref: String,
    /// Lowered exact or refused status.
    pub status: TassadarStructuredControlCaseStatus,
    /// Bundle digest when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    /// Returned values keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub return_values_by_export: BTreeMap<String, Option<i32>>,
    /// Trace digests keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trace_digests_by_export: BTreeMap<String, String>,
    /// Step counts keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trace_steps_by_export: BTreeMap<String, usize>,
    /// Machine-readable refusal kind when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    /// Human-readable refusal detail when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Committed report over the bounded structured-control lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Ordered structured-control cases.
    pub cases: Vec<TassadarStructuredControlCaseReport>,
    /// Explicit claim boundary for the report.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarStructuredControlReport {
    fn new(cases: Vec<TassadarStructuredControlCaseReport>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_STRUCTURED_CONTROL_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.structured_control.report.v1"),
            cases,
            claim_boundary: String::from(
                "this report proves one bounded compiled structured-control lane over zero-parameter i32-only Wasm functions with empty block types and exact CPU-reference replay for block, loop, if, else, br, br_if, and br_table; it does not claim calls, block results, imports, memories, tables, globals, or arbitrary Wasm closure",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_structured_control_report|", &report);
        report
    }
}

/// Report build failures for the structured-control lane.
#[derive(Debug, Error)]
pub enum TassadarStructuredControlReportError {
    /// Compiler lowering failed unexpectedly for one exact case or refused unexpectedly for one exact case.
    #[error(transparent)]
    Compiler(#[from] TassadarStructuredControlBundleError),
    /// Runtime replay failed for one lowered export.
    #[error("runtime replay failed for case `{case_id}` export `{export_name}`: {detail}")]
    RuntimeReplay {
        case_id: String,
        export_name: String,
        detail: String,
    },
    /// One lowered export diverged from the CPU reference manifest.
    #[error("structured-control parity mismatch for case `{case_id}` export `{export_name}`")]
    ParityMismatch {
        case_id: String,
        export_name: String,
    },
    /// Failed to create a directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write structured-control report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read a committed report.
    #[error("failed to read committed structured-control report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode a committed report.
    #[error("failed to decode committed structured-control report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_structured_control_report()
-> Result<TassadarStructuredControlReport, TassadarStructuredControlReportError> {
    let cases = vec![
        build_exact_case(
            "structured_if_else",
            "synthetic://tassadar/structured_control/if_else_v1",
            &tassadar_seeded_structured_control_if_else_module(),
        )?,
        build_exact_case(
            "structured_loop",
            "synthetic://tassadar/structured_control/loop_v1",
            &tassadar_seeded_structured_control_loop_module(),
        )?,
        build_exact_case(
            "structured_branch_table",
            "synthetic://tassadar/structured_control/branch_table_v1",
            &tassadar_seeded_structured_control_branch_table_module(),
        )?,
        build_refusal_case(
            "invalid_label_depth",
            "synthetic://tassadar/structured_control/invalid_label_depth_v1",
            &tassadar_seeded_structured_control_invalid_label_module(),
        ),
    ];
    Ok(TassadarStructuredControlReport::new(cases))
}

pub fn tassadar_structured_control_report_path() -> PathBuf {
    repo_root().join(TASSADAR_STRUCTURED_CONTROL_REPORT_REF)
}

pub fn write_tassadar_structured_control_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarStructuredControlReport, TassadarStructuredControlReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarStructuredControlReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_structured_control_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("structured-control report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarStructuredControlReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_exact_case(
    case_id: &str,
    source_ref: &str,
    wasm_bytes: &[u8],
) -> Result<TassadarStructuredControlCaseReport, TassadarStructuredControlReportError> {
    let bundle =
        compile_tassadar_wasm_binary_module_to_structured_control_bundle(case_id, wasm_bytes)?;
    let mut return_values_by_export = BTreeMap::new();
    let mut trace_digests_by_export = BTreeMap::new();
    let mut trace_steps_by_export = BTreeMap::new();
    for artifact in &bundle.artifacts {
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).map_err(|error| {
                TassadarStructuredControlReportError::RuntimeReplay {
                    case_id: String::from(case_id),
                    export_name: artifact.export_name.clone(),
                    detail: error.to_string(),
                }
            })?;
        if execution.returned_value != artifact.execution_manifest.expected_return_value
            || execution.halt_reason != artifact.execution_manifest.expected_halt_reason
            || execution.execution_digest() != artifact.execution_manifest.expected_trace_digest
            || execution.steps.len() != artifact.execution_manifest.expected_trace_step_count
            || execution.final_locals != artifact.execution_manifest.expected_final_locals
        {
            return Err(TassadarStructuredControlReportError::ParityMismatch {
                case_id: String::from(case_id),
                export_name: artifact.export_name.clone(),
            });
        }
        return_values_by_export.insert(artifact.export_name.clone(), execution.returned_value);
        trace_digests_by_export.insert(artifact.export_name.clone(), execution.execution_digest());
        trace_steps_by_export.insert(artifact.export_name.clone(), execution.steps.len());
    }

    Ok(TassadarStructuredControlCaseReport {
        case_id: String::from(case_id),
        source_ref: String::from(source_ref),
        status: TassadarStructuredControlCaseStatus::Exact,
        bundle_digest: Some(bundle.bundle_digest),
        return_values_by_export,
        trace_digests_by_export,
        trace_steps_by_export,
        refusal_kind: None,
        refusal_detail: None,
    })
}

fn build_refusal_case(
    case_id: &str,
    source_ref: &str,
    wasm_bytes: &[u8],
) -> TassadarStructuredControlCaseReport {
    match compile_tassadar_wasm_binary_module_to_structured_control_bundle(case_id, wasm_bytes) {
        Ok(bundle) => TassadarStructuredControlCaseReport {
            case_id: String::from(case_id),
            source_ref: String::from(source_ref),
            status: TassadarStructuredControlCaseStatus::Exact,
            bundle_digest: Some(bundle.bundle_digest),
            return_values_by_export: BTreeMap::new(),
            trace_digests_by_export: BTreeMap::new(),
            trace_steps_by_export: BTreeMap::new(),
            refusal_kind: None,
            refusal_detail: None,
        },
        Err(error) => TassadarStructuredControlCaseReport {
            case_id: String::from(case_id),
            source_ref: String::from(source_ref),
            status: TassadarStructuredControlCaseStatus::Refused,
            bundle_digest: None,
            return_values_by_export: BTreeMap::new(),
            trace_digests_by_export: BTreeMap::new(),
            trace_steps_by_export: BTreeMap::new(),
            refusal_kind: Some(refusal_kind(&error)),
            refusal_detail: Some(error.to_string()),
        },
    }
}

fn refusal_kind(error: &TassadarStructuredControlBundleError) -> String {
    match error {
        TassadarStructuredControlBundleError::UnsupportedSection { .. } => {
            String::from("unsupported_section")
        }
        TassadarStructuredControlBundleError::UnsupportedExportKind { .. } => {
            String::from("unsupported_export_kind")
        }
        TassadarStructuredControlBundleError::NoFunctionExports { .. } => {
            String::from("no_function_exports")
        }
        TassadarStructuredControlBundleError::UnsupportedParamCount { .. } => {
            String::from("unsupported_param_count")
        }
        TassadarStructuredControlBundleError::UnsupportedResultTypes { .. } => {
            String::from("unsupported_result_types")
        }
        TassadarStructuredControlBundleError::UnsupportedLocalType { .. } => {
            String::from("unsupported_local_type")
        }
        TassadarStructuredControlBundleError::UnsupportedBlockType { .. } => {
            String::from("unsupported_block_type")
        }
        TassadarStructuredControlBundleError::UnsupportedInstruction { .. } => {
            String::from("unsupported_instruction")
        }
        TassadarStructuredControlBundleError::MalformedStructuredControl { .. } => {
            String::from("malformed_structured_control")
        }
        TassadarStructuredControlBundleError::CodeBodyCountMismatch { .. } => {
            String::from("code_body_count_mismatch")
        }
        TassadarStructuredControlBundleError::Runtime(
            psionic_runtime::TassadarStructuredControlError::InvalidBranchDepth { .. },
        ) => String::from("invalid_branch_depth"),
        TassadarStructuredControlBundleError::Runtime(_) => String::from("runtime_validation"),
        TassadarStructuredControlBundleError::Parse(_) => String::from("parse"),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_STRUCTURED_CONTROL_REPORT_REF, TassadarStructuredControlCaseStatus,
        TassadarStructuredControlReport, build_tassadar_structured_control_report, repo_root,
        write_tassadar_structured_control_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn structured_control_report_captures_exact_and_refused_cases() {
        let report = build_tassadar_structured_control_report().expect("report");
        assert_eq!(report.cases.len(), 4);
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.status == TassadarStructuredControlCaseStatus::Exact)
        );
        let refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "invalid_label_depth")
            .expect("refusal case");
        assert_eq!(refusal.status, TassadarStructuredControlCaseStatus::Refused);
        assert_eq!(
            refusal.refusal_kind.as_deref(),
            Some("invalid_branch_depth")
        );
    }

    #[test]
    fn structured_control_report_matches_committed_truth() {
        let generated = build_tassadar_structured_control_report().expect("report");
        let committed: TassadarStructuredControlReport =
            read_repo_json(TASSADAR_STRUCTURED_CONTROL_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_structured_control_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_structured_control_report.json");
        let written = write_tassadar_structured_control_report(&output_path).expect("write report");
        let persisted: TassadarStructuredControlReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
