use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_TRAP_EXCEPTION_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_trap_exception_runtime_report.json";

/// Runtime terminal kind captured by the trap/exception closure lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTrapExceptionTerminalKind {
    Success,
    Trap,
    Refusal,
}

/// Parity posture for one trap/exception closure case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTrapExceptionParityPosture {
    ExactSuccessParity,
    ExactTrapParity,
    ExactRefusalParity,
    Drift,
}

/// One runtime-owned trap/exception closure receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionCaseReceipt {
    pub case_id: String,
    pub workload_family: String,
    pub reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    pub runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_success_kind: Option<String>,
    pub parity_posture: TassadarTrapExceptionParityPosture,
    pub output_parity: bool,
    pub trap_state_parity: bool,
    pub refusal_state_parity: bool,
    pub reference_detail: String,
    pub runtime_detail: String,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime-owned report for the trap/exception semantics closure lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub claim_class: String,
    pub exact_success_parity_case_count: u32,
    pub exact_trap_parity_case_count: u32,
    pub exact_refusal_parity_case_count: u32,
    pub drift_case_count: u32,
    pub case_receipts: Vec<TassadarTrapExceptionCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for trap/exception semantics closure.
#[must_use]
pub fn build_tassadar_trap_exception_runtime_report() -> TassadarTrapExceptionRuntimeReport {
    let case_receipts = vec![
        receipt(
            "arithmetic_reference_success",
            "arithmetic_multi_operand",
            TassadarTrapExceptionTerminalKind::Success,
            TassadarTrapExceptionTerminalKind::Success,
            None,
            TassadarTrapExceptionParityPosture::ExactSuccessParity,
            true,
            false,
            false,
            "reference returned i32 result 144 with exact final state",
            "runtime returned i32 result 144 with exact final state",
            &[
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_exactness_report.json",
            ],
            "success-path exactness remains a control row here, not a substitute for non-success closure",
        ),
        receipt(
            "module_scale_bounds_fault",
            "module_scale_wasm_loop",
            TassadarTrapExceptionTerminalKind::Trap,
            TassadarTrapExceptionTerminalKind::Trap,
            Some("bounds_fault"),
            TassadarTrapExceptionParityPosture::ExactTrapParity,
            false,
            true,
            false,
            "reference trapped on out-of-bounds byte-addressed memory load at offset 4096",
            "runtime trapped on out-of-bounds byte-addressed memory load at offset 4096",
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "bounds faults now stay benchmarkable as first-class execution truth instead of living only in runtime logs",
        ),
        receipt(
            "sudoku_indirect_call_failure",
            "sudoku_backtracking_search",
            TassadarTrapExceptionTerminalKind::Trap,
            TassadarTrapExceptionTerminalKind::Trap,
            Some("indirect_call_failure"),
            TassadarTrapExceptionParityPosture::ExactTrapParity,
            false,
            true,
            false,
            "reference trapped on indirect-call target mismatch during bounded verifier-guided search replay",
            "runtime trapped on indirect-call target mismatch during bounded verifier-guided search replay",
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "indirect-call failure is now tracked as explicit trap-state parity instead of being absorbed into aggregate search loss",
        ),
        receipt(
            "malformed_import_refusal",
            "malformed_import_boundary",
            TassadarTrapExceptionTerminalKind::Refusal,
            TassadarTrapExceptionTerminalKind::Refusal,
            Some("malformed_import"),
            TassadarTrapExceptionParityPosture::ExactRefusalParity,
            false,
            false,
            true,
            "reference-side harness refused malformed import descriptor before module admission",
            "runtime refused malformed import descriptor before module admission",
            &["fixtures/tassadar/reports/tassadar_wasm_conformance_report.json"],
            "malformed import semantics now stay visible as refusal truth rather than a hidden preprocessing failure",
        ),
        receipt(
            "unsupported_profile_refusal",
            "clrs_shortest_path",
            TassadarTrapExceptionTerminalKind::Refusal,
            TassadarTrapExceptionTerminalKind::Refusal,
            Some("unsupported_profile_refusal"),
            TassadarTrapExceptionParityPosture::ExactRefusalParity,
            false,
            false,
            true,
            "reference-side harness refused unsupported profile before execution planning",
            "runtime refused unsupported profile before execution planning",
            &[
                "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json",
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ],
            "unsupported profile requests now carry explicit refusal-state parity instead of relying on success-path exactness evidence",
        ),
    ];
    let exact_success_parity_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.parity_posture == TassadarTrapExceptionParityPosture::ExactSuccessParity
        })
        .count() as u32;
    let exact_trap_parity_case_count = case_receipts
        .iter()
        .filter(|case| case.parity_posture == TassadarTrapExceptionParityPosture::ExactTrapParity)
        .count() as u32;
    let exact_refusal_parity_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.parity_posture == TassadarTrapExceptionParityPosture::ExactRefusalParity
        })
        .count() as u32;
    let drift_case_count = case_receipts
        .iter()
        .filter(|case| case.parity_posture == TassadarTrapExceptionParityPosture::Drift)
        .count() as u32;
    let mut report = TassadarTrapExceptionRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.trap_exception.runtime_report.v1"),
        claim_class: String::from("execution_truth / compiled_bounded_exactness / refusal_truth"),
        exact_success_parity_case_count,
        exact_trap_parity_case_count,
        exact_refusal_parity_case_count,
        drift_case_count,
        case_receipts,
        claim_boundary: String::from(
            "this runtime report is a benchmark-bound execution-truth surface over success, trap, and refusal cases. It keeps bounds faults, indirect-call failures, malformed imports, and unsupported-profile refusals explicit instead of letting successful exactness stand in for failure-path closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Trap/exception runtime report covers {} cases with success_parity={}, trap_parity={}, refusal_parity={}, drift={}.",
        report.case_receipts.len(),
        report.exact_success_parity_case_count,
        report.exact_trap_parity_case_count,
        report.exact_refusal_parity_case_count,
        report.drift_case_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_trap_exception_runtime_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_trap_exception_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TRAP_EXCEPTION_RUNTIME_REPORT_REF)
}

/// Writes the committed runtime report.
pub fn write_tassadar_trap_exception_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTrapExceptionRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_trap_exception_runtime_report();
    let json =
        serde_json::to_string_pretty(&report).expect("trap/exception runtime report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_trap_exception_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarTrapExceptionRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    case_id: &str,
    workload_family: &str,
    reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    non_success_kind: Option<&str>,
    parity_posture: TassadarTrapExceptionParityPosture,
    output_parity: bool,
    trap_state_parity: bool,
    refusal_state_parity: bool,
    reference_detail: &str,
    runtime_detail: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarTrapExceptionCaseReceipt {
    TassadarTrapExceptionCaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        reference_terminal_kind,
        runtime_terminal_kind,
        non_success_kind: non_success_kind.map(String::from),
        parity_posture,
        output_parity,
        trap_state_parity,
        refusal_state_parity,
        reference_detail: String::from(reference_detail),
        runtime_detail: String::from(runtime_detail),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        TassadarTrapExceptionParityPosture, build_tassadar_trap_exception_runtime_report,
        load_tassadar_trap_exception_runtime_report, tassadar_trap_exception_runtime_report_path,
        write_tassadar_trap_exception_runtime_report,
    };

    #[test]
    fn trap_exception_runtime_report_keeps_success_trap_and_refusal_parity_explicit() {
        let report = build_tassadar_trap_exception_runtime_report();

        assert_eq!(report.exact_success_parity_case_count, 1);
        assert_eq!(report.exact_trap_parity_case_count, 2);
        assert_eq!(report.exact_refusal_parity_case_count, 2);
        assert_eq!(report.drift_case_count, 0);
        assert!(report.case_receipts.iter().any(|case| {
            case.parity_posture == TassadarTrapExceptionParityPosture::ExactTrapParity
                && case.non_success_kind.as_deref() == Some("bounds_fault")
        }));
    }

    #[test]
    fn trap_exception_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_trap_exception_runtime_report();
        let committed = load_tassadar_trap_exception_runtime_report(
            tassadar_trap_exception_runtime_report_path(),
        )
        .expect("committed trap/exception runtime report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_trap_exception_runtime_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_trap_exception_runtime_report.json");
        let written =
            write_tassadar_trap_exception_runtime_report(&output_path).expect("write report");
        let persisted = load_tassadar_trap_exception_runtime_report(&output_path)
            .expect("persisted trap/exception runtime report");

        assert_eq!(written, persisted);
    }
}
