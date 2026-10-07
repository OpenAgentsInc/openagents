use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TassadarFrozenCoreWasmClosureGateRow, TassadarFrozenCoreWasmClosureGateRowStatus,
    TassadarFrozenCoreWasmClosureGateStatus, TassadarModuleDifferentialStatus,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF, TASSADAR_TRAP_EXCEPTION_REPORT_REF,
    TASSADAR_WASM_CONFORMANCE_REPORT_REF, TassadarFrozenCoreWasmWindowReport,
    TassadarTrapExceptionReport, TassadarWasmConformanceReport,
    build_tassadar_trap_exception_report,
};

const TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_frozen_core_wasm_closure_gate_report.json";

/// Committed closure gate over the frozen core-Wasm target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmClosureGateReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report id.
    pub report_id: String,
    /// Frozen-window report ref.
    pub frozen_window_report_ref: String,
    /// Frozen-window report id.
    pub frozen_window_report_id: String,
    /// Frozen semantic window id.
    pub frozen_window_id: String,
    /// Conformance report ref.
    pub conformance_report_ref: String,
    /// Conformance report id.
    pub conformance_report_id: String,
    /// Trap/refusal parity report ref.
    pub trap_exception_report_ref: String,
    /// Trap/refusal parity report id.
    pub trap_exception_report_id: String,
    /// Feature families currently evidenced by the gate inputs.
    pub current_covered_feature_family_ids: Vec<String>,
    /// Target feature families still missing from the gate inputs.
    pub missing_feature_family_ids: Vec<String>,
    /// Machine-readable gate rows.
    pub gate_rows: Vec<TassadarFrozenCoreWasmClosureGateRow>,
    /// Overall closure status.
    pub closure_status: TassadarFrozenCoreWasmClosureGateStatus,
    /// Whether full served publication is allowed by this gate.
    pub served_publication_allowed: bool,
    /// Plain-language detail.
    pub detail: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarFrozenCoreWasmClosureGateReport {
    fn new(
        window_report: TassadarFrozenCoreWasmWindowReport,
        conformance_report: TassadarWasmConformanceReport,
        trap_report: TassadarTrapExceptionReport,
    ) -> Self {
        let covered_feature_family_ids = current_covered_feature_family_ids(
            &window_report,
            &conformance_report,
            &trap_report,
        );
        let missing_feature_family_ids = window_report
            .frozen_window
            .target_feature_family_ids
            .iter()
            .filter(|feature_family_id| !covered_feature_family_ids.contains(*feature_family_id))
            .cloned()
            .collect::<Vec<_>>();
        let admitted_count = window_report
            .validation_cases
            .iter()
            .filter(|case| case.status == crate::TassadarFrozenCoreWasmHarnessValidationStatus::Admitted)
            .count();
        let refusal_count = window_report
            .validation_cases
            .iter()
            .filter(|case| {
                case.status
                    == crate::TassadarFrozenCoreWasmHarnessValidationStatus::RefusedOutOfWindow
            })
            .count();
        let exact_success_count = conformance_report
            .cases
            .iter()
            .filter(|case| case.status == TassadarModuleDifferentialStatus::ExactSuccess)
            .count();
        let exact_trap_count = conformance_report
            .cases
            .iter()
            .filter(|case| case.status == TassadarModuleDifferentialStatus::ExactTrapParity)
            .count();
        let boundary_refusal_count = conformance_report
            .cases
            .iter()
            .filter(|case| case.status == TassadarModuleDifferentialStatus::BoundaryRefusal)
            .count();
        let drift_count = conformance_report
            .cases
            .iter()
            .filter(|case| case.status == TassadarModuleDifferentialStatus::Drift)
            .count();
        let mut gate_rows = vec![
            TassadarFrozenCoreWasmClosureGateRow {
                row_id: String::from("official_window_and_harness"),
                description: String::from(
                    "the frozen window and official text/binary/validation/execution harness are declared and packaged",
                ),
                status: if admitted_count >= 3
                    && refusal_count >= 2
                    && !window_report.frozen_window.window_id.trim().is_empty()
                    && !window_report.frozen_window.official_harness_id.trim().is_empty()
                {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Green
                } else {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Red
                },
                detail: format!(
                    "window_id={}, admitted_validation_cases={}, refusal_validation_cases={}",
                    window_report.frozen_window.window_id, admitted_count, refusal_count
                ),
            },
            TassadarFrozenCoreWasmClosureGateRow {
                row_id: String::from("differential_execution_parity"),
                description: String::from(
                    "the differential execution harness preserves success, trap, and boundary-refusal parity without drift",
                ),
                status: if exact_success_count > 0
                    && exact_trap_count > 0
                    && boundary_refusal_count > 0
                    && drift_count == 0
                {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Green
                } else {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Red
                },
                detail: format!(
                    "exact_success={}, exact_trap={}, boundary_refusal={}, drift={}",
                    exact_success_count, exact_trap_count, boundary_refusal_count, drift_count
                ),
            },
            TassadarFrozenCoreWasmClosureGateRow {
                row_id: String::from("trap_and_refusal_parity"),
                description: String::from(
                    "the trap and refusal audit preserves explicit non-success parity over the declared bounded cases",
                ),
                status: if trap_report.drift_case_count == 0
                    && trap_report.exact_trap_parity_case_count > 0
                    && trap_report.exact_refusal_parity_case_count > 0
                {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Green
                } else {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Red
                },
                detail: format!(
                    "trap_parity={}, refusal_parity={}, drift={}",
                    trap_report.exact_trap_parity_case_count,
                    trap_report.exact_refusal_parity_case_count,
                    trap_report.drift_case_count
                ),
            },
            TassadarFrozenCoreWasmClosureGateRow {
                row_id: String::from("target_feature_family_coverage"),
                description: String::from(
                    "the closure gate only turns green once every declared frozen-window feature family is explicitly evidenced",
                ),
                status: if missing_feature_family_ids.is_empty() {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Green
                } else {
                    TassadarFrozenCoreWasmClosureGateRowStatus::Red
                },
                detail: if missing_feature_family_ids.is_empty() {
                    String::from("every declared target feature family is currently evidenced")
                } else {
                    format!(
                        "missing feature families: {}",
                        missing_feature_family_ids.join(", ")
                    )
                },
            },
            TassadarFrozenCoreWasmClosureGateRow {
                row_id: String::from("cross_machine_harness_replay"),
                description: String::from(
                    "the closure gate only turns green once the frozen-window harness has its own dedicated cross-machine replay matrix",
                ),
                status: TassadarFrozenCoreWasmClosureGateRowStatus::Red,
                detail: String::from(
                    "no dedicated frozen-window cross-machine replay matrix is committed yet; broader portability evidence exists elsewhere but is not enough for this gate",
                ),
            },
        ];
        gate_rows.sort_by(|left, right| left.row_id.cmp(&right.row_id));
        let closure_status = if gate_rows
            .iter()
            .all(|row| row.status == TassadarFrozenCoreWasmClosureGateRowStatus::Green)
        {
            TassadarFrozenCoreWasmClosureGateStatus::Closed
        } else {
            TassadarFrozenCoreWasmClosureGateStatus::NotClosed
        };
        let served_publication_allowed =
            matches!(closure_status, TassadarFrozenCoreWasmClosureGateStatus::Closed);
        let mut report = Self {
            schema_version: TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.frozen_core_wasm_closure_gate.report.v1"),
            frozen_window_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF),
            frozen_window_report_id: window_report.report_id,
            frozen_window_id: window_report.frozen_window.window_id,
            conformance_report_ref: String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
            conformance_report_id: conformance_report.report_id,
            trap_exception_report_ref: String::from(TASSADAR_TRAP_EXCEPTION_REPORT_REF),
            trap_exception_report_id: trap_report.report_id,
            current_covered_feature_family_ids: covered_feature_family_ids,
            missing_feature_family_ids,
            gate_rows,
            closure_status,
            served_publication_allowed,
            detail: String::new(),
            report_digest: String::new(),
        };
        report.detail = format!(
            "frozen core-Wasm closure gate `{}` is {:?}; covered_feature_families={}, missing_feature_families={}, served_publication_allowed={}",
            report.report_id,
            report.closure_status,
            report.current_covered_feature_family_ids.len(),
            report.missing_feature_family_ids.len(),
            report.served_publication_allowed
        );
        report.report_digest =
            stable_digest(b"psionic_tassadar_frozen_core_wasm_closure_gate_report|", &report);
        report
    }
}

/// Build failures for the frozen core-Wasm closure gate.
#[derive(Debug, Error)]
pub enum TassadarFrozenCoreWasmClosureGateReportError {
    /// One committed JSON artifact could not be read.
    #[error("failed to read committed JSON `{path}`: {error}")]
    ReadJson {
        /// Repo-relative path.
        path: String,
        /// IO or decode summary.
        error: String,
    },
    /// Failed to create the report directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write frozen core-Wasm closure gate report `{path}`: {error}")]
    Write {
        /// Output path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
}

/// Builds the committed frozen core-Wasm closure gate report.
pub fn build_tassadar_frozen_core_wasm_closure_gate_report(
) -> Result<TassadarFrozenCoreWasmClosureGateReport, TassadarFrozenCoreWasmClosureGateReportError>
{
    let window_report = read_repo_json::<TassadarFrozenCoreWasmWindowReport>(
        TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
    )?;
    let conformance_report =
        read_repo_json::<TassadarWasmConformanceReport>(TASSADAR_WASM_CONFORMANCE_REPORT_REF)?;
    let trap_report = build_tassadar_trap_exception_report();
    Ok(TassadarFrozenCoreWasmClosureGateReport::new(
        window_report,
        conformance_report,
        trap_report,
    ))
}

/// Returns the canonical path for the committed frozen core-Wasm closure gate report.
pub fn tassadar_frozen_core_wasm_closure_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF)
}

/// Writes the committed frozen core-Wasm closure gate report.
pub fn write_tassadar_frozen_core_wasm_closure_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarFrozenCoreWasmClosureGateReport, TassadarFrozenCoreWasmClosureGateReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFrozenCoreWasmClosureGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_frozen_core_wasm_closure_gate_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("frozen core-Wasm closure gate should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarFrozenCoreWasmClosureGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn current_covered_feature_family_ids(
    window_report: &TassadarFrozenCoreWasmWindowReport,
    conformance_report: &TassadarWasmConformanceReport,
    trap_report: &TassadarTrapExceptionReport,
) -> Vec<String> {
    let mut covered = BTreeSet::new();
    if conformance_report
        .cases
        .iter()
        .any(|case| case.family_id == "curated.global_state"
            && case.status == TassadarModuleDifferentialStatus::ExactSuccess)
    {
        covered.insert(String::from("globals.mutable"));
    }
    if conformance_report
        .cases
        .iter()
        .any(|case| case.family_id == "curated.call_indirect"
            && case.status == TassadarModuleDifferentialStatus::ExactSuccess)
        && trap_report.case_audits.iter().any(|case| {
            case.case_id == "call_indirect_trap"
                && case.parity_preserved
                && case.reference_terminal_kind == psionic_runtime::TassadarTrapExceptionTerminalKind::Trap
        })
    {
        covered.insert(String::from("tables.funcref_call_indirect"));
    }
    if conformance_report
        .cases
        .iter()
        .any(|case| case.family_id == "curated.instantiation"
            && case.status == TassadarModuleDifferentialStatus::ExactSuccess)
        && conformance_report
            .cases
            .iter()
            .any(|case| case.family_id == "curated.unsupported_host_import"
                && case.status == TassadarModuleDifferentialStatus::BoundaryRefusal)
    {
        covered.insert(String::from("module.import_export_start"));
        covered.insert(String::from("segments.active_data_and_element"));
    }
    let mut covered = covered.into_iter().collect::<Vec<_>>();
    covered.sort();
    // Keep the report honest: never surface unknown families outside the declared window.
    covered.retain(|family| {
        window_report
            .frozen_window
            .target_feature_family_ids
            .contains(family)
    });
    covered
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    repo_relative_path: &str,
) -> Result<T, TassadarFrozenCoreWasmClosureGateReportError> {
    let path = repo_root().join(repo_relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarFrozenCoreWasmClosureGateReportError::ReadJson {
        path: repo_relative_path.to_string(),
        error: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarFrozenCoreWasmClosureGateReportError::ReadJson {
        path: repo_relative_path.to_string(),
        error: error.to_string(),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psionic_runtime::{
        TassadarFrozenCoreWasmClosureGateRowStatus, TassadarFrozenCoreWasmClosureGateStatus,
    };

    use super::{
        TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
        TassadarFrozenCoreWasmClosureGateReport, build_tassadar_frozen_core_wasm_closure_gate_report,
        tassadar_frozen_core_wasm_closure_gate_report_path,
        write_tassadar_frozen_core_wasm_closure_gate_report,
    };

    #[test]
    fn frozen_core_wasm_closure_gate_keeps_red_rows_explicit() {
        let report = build_tassadar_frozen_core_wasm_closure_gate_report()
            .expect("frozen core-Wasm closure gate should build");
        assert_eq!(
            report.closure_status,
            TassadarFrozenCoreWasmClosureGateStatus::NotClosed
        );
        assert!(
            report
                .gate_rows
                .iter()
                .any(|row| row.row_id == "differential_execution_parity"
                    && row.status == TassadarFrozenCoreWasmClosureGateRowStatus::Green)
        );
        assert!(
            report
                .gate_rows
                .iter()
                .any(|row| row.row_id == "cross_machine_harness_replay"
                    && row.status == TassadarFrozenCoreWasmClosureGateRowStatus::Red)
        );
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn frozen_core_wasm_closure_gate_matches_committed_truth() {
        let report = build_tassadar_frozen_core_wasm_closure_gate_report()
            .expect("frozen core-Wasm closure gate should build");
        let path = tassadar_frozen_core_wasm_closure_gate_report_path();
        let bytes = std::fs::read(&path).expect("committed report should exist");
        let persisted: TassadarFrozenCoreWasmClosureGateReport =
            serde_json::from_slice(&bytes).expect("committed report should decode");
        assert_eq!(
            persisted,
            report,
            "run the example to refresh `{}`",
            path.display()
        );
    }

    #[test]
    fn write_frozen_core_wasm_closure_gate_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_frozen_core_wasm_closure_gate_report.json");
        let report = write_tassadar_frozen_core_wasm_closure_gate_report(&output_path)
            .expect("frozen core-Wasm closure gate should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: TassadarFrozenCoreWasmClosureGateReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
        assert_eq!(
            PathBuf::from(TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_frozen_core_wasm_closure_gate_report.json")
        );
    }
}
