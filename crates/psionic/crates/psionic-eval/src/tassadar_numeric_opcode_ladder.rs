use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarStructuredControlBundleError,
    compile_tassadar_wasm_binary_module_to_structured_control_bundle,
    tassadar_seeded_numeric_float_refusal_module, tassadar_seeded_numeric_i32_bit_ops_module,
    tassadar_seeded_numeric_i32_comparison_module,
    tassadar_seeded_numeric_i32_core_arithmetic_module, tassadar_seeded_numeric_i64_refusal_module,
};
use psionic_data::{
    TassadarNumericOpcodeFamily, TassadarNumericOpcodeFamilyStatus,
    TassadarNumericOpcodeLadderContract, tassadar_numeric_opcode_ladder_contract,
};
use psionic_runtime::execute_tassadar_structured_control_program;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_numeric_opcode_ladder_report.json";

/// Repo-facing lowering status for one numeric-opcode case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericOpcodeCaseStatus {
    /// Lowering succeeded and replay matched the CPU reference manifest.
    Exact,
    /// Lowering refused explicitly.
    Refused,
}

/// One repo-facing numeric-opcode widening case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericOpcodeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Numeric family owned by the case.
    pub family: TassadarNumericOpcodeFamily,
    /// Stable source ref for the underlying Wasm bytes.
    pub source_ref: String,
    /// Lowered exact or refused status.
    pub status: TassadarNumericOpcodeCaseStatus,
    /// Lowered bundle digest when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    /// Returned values keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub return_values_by_export: BTreeMap<String, Option<i32>>,
    /// Trace digests keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trace_digests_by_export: BTreeMap<String, String>,
    /// Trace step counts keyed by export name when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trace_steps_by_export: BTreeMap<String, usize>,
    /// Machine-readable refusal kind when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    /// Human-readable refusal detail when lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Family-by-family coverage and conformance summary for the current ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericOpcodeFamilyCoverage {
    /// Numeric family summarized by the row.
    pub family: TassadarNumericOpcodeFamily,
    /// Current support posture for the family.
    pub status: TassadarNumericOpcodeFamilyStatus,
    /// Count of exact opcodes currently admitted for the family.
    pub supported_opcode_count: usize,
    /// Exact case identifiers that verified the family.
    pub exact_case_ids: Vec<String>,
    /// Refusal case identifiers that verified the family.
    pub refusal_case_ids: Vec<String>,
    /// Count of lowered exports proven exact for the family.
    pub exact_export_count: usize,
    /// Whether the report verified the expected posture for the family.
    pub conformance_verified: bool,
}

/// Committed report over the current numeric-opcode widening ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericOpcodeLadderReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public ladder contract consumed by the report.
    pub ladder_contract: TassadarNumericOpcodeLadderContract,
    /// Family-by-family coverage and conformance summary.
    pub family_coverage: Vec<TassadarNumericOpcodeFamilyCoverage>,
    /// Ordered exact and refused cases.
    pub cases: Vec<TassadarNumericOpcodeCaseReport>,
    /// Explicit claim boundary for the report.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarNumericOpcodeLadderReport {
    fn new(cases: Vec<TassadarNumericOpcodeCaseReport>) -> Self {
        let ladder_contract = tassadar_numeric_opcode_ladder_contract();
        let family_coverage = ladder_contract
            .families
            .iter()
            .map(|family| {
                let exact_case_ids = cases
                    .iter()
                    .filter(|case| {
                        case.family == family.family
                            && case.status == TassadarNumericOpcodeCaseStatus::Exact
                    })
                    .map(|case| case.case_id.clone())
                    .collect::<Vec<_>>();
                let refusal_case_ids = cases
                    .iter()
                    .filter(|case| {
                        case.family == family.family
                            && case.status == TassadarNumericOpcodeCaseStatus::Refused
                    })
                    .map(|case| case.case_id.clone())
                    .collect::<Vec<_>>();
                let exact_export_count = cases
                    .iter()
                    .filter(|case| {
                        case.family == family.family
                            && case.status == TassadarNumericOpcodeCaseStatus::Exact
                    })
                    .map(|case| case.return_values_by_export.len())
                    .sum::<usize>();
                let conformance_verified = match family.status {
                    TassadarNumericOpcodeFamilyStatus::Implemented => {
                        !exact_case_ids.is_empty() && refusal_case_ids.is_empty()
                    }
                    TassadarNumericOpcodeFamilyStatus::Refused => {
                        exact_case_ids.is_empty() && !refusal_case_ids.is_empty()
                    }
                };
                TassadarNumericOpcodeFamilyCoverage {
                    family: family.family,
                    status: family.status,
                    supported_opcode_count: family.supported_opcodes.len(),
                    exact_case_ids,
                    refusal_case_ids,
                    exact_export_count,
                    conformance_verified,
                }
            })
            .collect::<Vec<_>>();
        let mut report = Self {
            schema_version: TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.numeric_opcode_ladder.report.v1"),
            ladder_contract,
            family_coverage,
            cases,
            claim_boundary: String::from(
                "this report proves the current staged numeric widening ladder only: exact bounded coverage for i32 core arithmetic, comparisons, and bit operations in the zero-parameter i32-only structured-control lane, plus typed refusal for i64 and floating-point families; it does not claim arbitrary Wasm numeric closure, mixed-width execution, floating-point exactness, or served capability promotion",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_numeric_opcode_ladder_report|", &report);
        report
    }
}

/// Numeric-opcode ladder report failures.
#[derive(Debug, Error)]
pub enum TassadarNumericOpcodeLadderReportError {
    /// Compiler lowering failed unexpectedly for one exact case.
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
    #[error("numeric-opcode parity mismatch for case `{case_id}` export `{export_name}`")]
    ParityMismatch {
        case_id: String,
        export_name: String,
    },
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write numeric-opcode ladder report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read a committed report.
    #[error("failed to read committed numeric-opcode ladder report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode a committed report.
    #[error("failed to decode committed numeric-opcode ladder report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

/// Builds the committed report for the current numeric-opcode widening ladder.
pub fn build_tassadar_numeric_opcode_ladder_report()
-> Result<TassadarNumericOpcodeLadderReport, TassadarNumericOpcodeLadderReportError> {
    let cases = vec![
        build_exact_case(
            "i32_core_arithmetic_suite",
            TassadarNumericOpcodeFamily::I32CoreArithmetic,
            "synthetic://tassadar/numeric_opcode_ladder/i32_core_arithmetic_v1",
            &tassadar_seeded_numeric_i32_core_arithmetic_module(),
        )?,
        build_exact_case(
            "i32_comparison_suite",
            TassadarNumericOpcodeFamily::I32Comparisons,
            "synthetic://tassadar/numeric_opcode_ladder/i32_comparison_v1",
            &tassadar_seeded_numeric_i32_comparison_module(),
        )?,
        build_exact_case(
            "i32_bit_ops_suite",
            TassadarNumericOpcodeFamily::I32BitOps,
            "synthetic://tassadar/numeric_opcode_ladder/i32_bit_ops_v1",
            &tassadar_seeded_numeric_i32_bit_ops_module(),
        )?,
        build_refusal_case(
            "i64_refusal",
            TassadarNumericOpcodeFamily::I64Integer,
            "synthetic://tassadar/numeric_opcode_ladder/i64_refusal_v1",
            &tassadar_seeded_numeric_i64_refusal_module(),
        ),
        build_refusal_case(
            "float_refusal",
            TassadarNumericOpcodeFamily::FloatingPoint,
            "synthetic://tassadar/numeric_opcode_ladder/float_refusal_v1",
            &tassadar_seeded_numeric_float_refusal_module(),
        ),
    ];
    Ok(TassadarNumericOpcodeLadderReport::new(cases))
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_numeric_opcode_ladder_report_path() -> PathBuf {
    repo_root().join(TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_REF)
}

/// Writes the committed report for the current numeric-opcode widening ladder.
pub fn write_tassadar_numeric_opcode_ladder_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarNumericOpcodeLadderReport, TassadarNumericOpcodeLadderReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarNumericOpcodeLadderReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_numeric_opcode_ladder_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("numeric-opcode ladder report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarNumericOpcodeLadderReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_exact_case(
    case_id: &str,
    family: TassadarNumericOpcodeFamily,
    source_ref: &str,
    wasm_bytes: &[u8],
) -> Result<TassadarNumericOpcodeCaseReport, TassadarNumericOpcodeLadderReportError> {
    let bundle =
        compile_tassadar_wasm_binary_module_to_structured_control_bundle(case_id, wasm_bytes)?;
    let mut return_values_by_export = BTreeMap::new();
    let mut trace_digests_by_export = BTreeMap::new();
    let mut trace_steps_by_export = BTreeMap::new();
    for artifact in &bundle.artifacts {
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).map_err(|error| {
                TassadarNumericOpcodeLadderReportError::RuntimeReplay {
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
            return Err(TassadarNumericOpcodeLadderReportError::ParityMismatch {
                case_id: String::from(case_id),
                export_name: artifact.export_name.clone(),
            });
        }
        return_values_by_export.insert(artifact.export_name.clone(), execution.returned_value);
        trace_digests_by_export.insert(artifact.export_name.clone(), execution.execution_digest());
        trace_steps_by_export.insert(artifact.export_name.clone(), execution.steps.len());
    }

    Ok(TassadarNumericOpcodeCaseReport {
        case_id: String::from(case_id),
        family,
        source_ref: String::from(source_ref),
        status: TassadarNumericOpcodeCaseStatus::Exact,
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
    family: TassadarNumericOpcodeFamily,
    source_ref: &str,
    wasm_bytes: &[u8],
) -> TassadarNumericOpcodeCaseReport {
    match compile_tassadar_wasm_binary_module_to_structured_control_bundle(case_id, wasm_bytes) {
        Ok(bundle) => TassadarNumericOpcodeCaseReport {
            case_id: String::from(case_id),
            family,
            source_ref: String::from(source_ref),
            status: TassadarNumericOpcodeCaseStatus::Exact,
            bundle_digest: Some(bundle.bundle_digest),
            return_values_by_export: BTreeMap::new(),
            trace_digests_by_export: BTreeMap::new(),
            trace_steps_by_export: BTreeMap::new(),
            refusal_kind: None,
            refusal_detail: None,
        },
        Err(error) => TassadarNumericOpcodeCaseReport {
            case_id: String::from(case_id),
            family,
            source_ref: String::from(source_ref),
            status: TassadarNumericOpcodeCaseStatus::Refused,
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
        TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_REF, TassadarNumericOpcodeCaseStatus,
        TassadarNumericOpcodeLadderReport, build_tassadar_numeric_opcode_ladder_report, repo_root,
        write_tassadar_numeric_opcode_ladder_report,
    };
    use psionic_data::{TassadarNumericOpcodeFamily, TassadarNumericOpcodeFamilyStatus};

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn numeric_opcode_ladder_report_captures_current_family_truth() {
        let report = build_tassadar_numeric_opcode_ladder_report().expect("report");
        assert_eq!(report.cases.len(), 5);
        let arithmetic = report
            .cases
            .iter()
            .find(|case| case.case_id == "i32_core_arithmetic_suite")
            .expect("arithmetic case");
        assert_eq!(arithmetic.status, TassadarNumericOpcodeCaseStatus::Exact);
        assert_eq!(
            arithmetic
                .return_values_by_export
                .get("i32_core_arithmetic_suite"),
            Some(&Some(32))
        );
        let i64_refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "i64_refusal")
            .expect("i64 refusal");
        assert_eq!(i64_refusal.status, TassadarNumericOpcodeCaseStatus::Refused);
        assert!(i64_refusal.refusal_kind.is_some());
        let comparison_family = report
            .family_coverage
            .iter()
            .find(|family| family.family == TassadarNumericOpcodeFamily::I32Comparisons)
            .expect("comparison family");
        assert_eq!(
            comparison_family.status,
            TassadarNumericOpcodeFamilyStatus::Implemented
        );
        assert!(comparison_family.conformance_verified);
    }

    #[test]
    fn numeric_opcode_ladder_report_matches_committed_truth() {
        let generated = build_tassadar_numeric_opcode_ladder_report().expect("report");
        let committed: TassadarNumericOpcodeLadderReport =
            read_repo_json(TASSADAR_NUMERIC_OPCODE_LADDER_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_numeric_opcode_ladder_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_numeric_opcode_ladder_report.json");
        let written =
            write_tassadar_numeric_opcode_ladder_report(&output_path).expect("write report");
        let persisted: TassadarNumericOpcodeLadderReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
