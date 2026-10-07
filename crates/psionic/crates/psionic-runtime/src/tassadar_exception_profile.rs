use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{TassadarTrapExceptionParityPosture, TassadarTrapExceptionTerminalKind};
use psionic_ir::{
    TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EXCEPTION_PROFILE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exception_profile_runtime_report.json";

/// One runtime-owned case receipt for the bounded exceptions profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileCaseReceipt {
    pub case_id: String,
    pub semantic_id: String,
    pub workload_family: String,
    pub reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    pub runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_success_kind: Option<String>,
    pub parity_posture: TassadarTrapExceptionParityPosture,
    pub output_parity: bool,
    pub trap_stack_parity: bool,
    pub refusal_state_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_trap_stack_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_trap_stack_depth: Option<u32>,
    pub reference_detail: String,
    pub runtime_detail: String,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime-owned report for the bounded exceptions proposal profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub claim_class: String,
    pub exact_success_parity_case_count: u32,
    pub exact_trap_parity_case_count: u32,
    pub exact_refusal_parity_case_count: u32,
    pub exact_trap_stack_parity_case_count: u32,
    pub drift_case_count: u32,
    pub case_receipts: Vec<TassadarExceptionProfileCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for the exceptions proposal profile.
#[must_use]
pub fn build_tassadar_exception_profile_runtime_report() -> TassadarExceptionProfileRuntimeReport {
    let case_receipts = vec![
        receipt(
            "throw_catch_success",
            "throw_catch_success",
            "module_scale_exception_dispatch",
            TassadarTrapExceptionTerminalKind::Success,
            TassadarTrapExceptionTerminalKind::Success,
            None,
            TassadarTrapExceptionParityPosture::ExactSuccessParity,
            true,
            false,
            false,
            None,
            None,
            "reference caught the thrown tag and returned i32 result 21 with exact recovered state",
            "runtime caught the thrown tag and returned i32 result 21 with exact recovered state",
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            "typed catch remains a success-path control row here, not evidence for arbitrary exception closure",
        ),
        receipt(
            "nested_rethrow_trap_stack",
            "nested_rethrow_trap_stack",
            "search_exception_unwind",
            TassadarTrapExceptionTerminalKind::Trap,
            TassadarTrapExceptionTerminalKind::Trap,
            Some("uncaught_rethrow"),
            TassadarTrapExceptionParityPosture::ExactTrapParity,
            false,
            true,
            false,
            Some(2),
            Some(2),
            "reference rethrew through two handler frames and trapped with uncaught_rethrow at trap-stack depth 2",
            "runtime rethrew through two handler frames and trapped with uncaught_rethrow at trap-stack depth 2",
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "nested rethrow now preserves explicit trap-stack depth parity instead of degrading into a generic failure count",
        ),
        receipt(
            "handler_tag_mismatch_trap",
            "handler_tag_mismatch_trap",
            "exception_handler_dispatch",
            TassadarTrapExceptionTerminalKind::Trap,
            TassadarTrapExceptionTerminalKind::Trap,
            Some("handler_tag_mismatch"),
            TassadarTrapExceptionParityPosture::ExactTrapParity,
            false,
            true,
            false,
            Some(1),
            Some(1),
            "reference trapped on handler tag mismatch at trap-stack depth 1",
            "runtime trapped on handler tag mismatch at trap-stack depth 1",
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "tag mismatch remains typed trap truth instead of being absorbed into malformed-handler refusal",
        ),
        receipt(
            "malformed_exception_handler_refusal",
            "malformed_exception_handler_refusal",
            "exception_handler_boundary",
            TassadarTrapExceptionTerminalKind::Refusal,
            TassadarTrapExceptionTerminalKind::Refusal,
            Some("malformed_exception_handler"),
            TassadarTrapExceptionParityPosture::ExactRefusalParity,
            false,
            false,
            true,
            None,
            None,
            "reference-side harness refused malformed exception handler metadata before execution planning",
            "runtime refused malformed exception handler metadata before execution planning",
            &["fixtures/tassadar/reports/tassadar_trap_exception_report.json"],
            "malformed handlers remain typed refusal truth instead of becoming an opaque validation failure",
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
    let exact_trap_stack_parity_case_count = case_receipts
        .iter()
        .filter(|case| case.trap_stack_parity)
        .count() as u32;
    let drift_case_count = case_receipts
        .iter()
        .filter(|case| case.parity_posture == TassadarTrapExceptionParityPosture::Drift)
        .count() as u32;
    let mut report = TassadarExceptionProfileRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.exception_profile.runtime_report.v1"),
        profile_id: String::from(TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID),
        portability_envelope_id: String::from(
            TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        claim_class: String::from("execution_truth / compiled_bounded_exactness / refusal_truth"),
        exact_success_parity_case_count,
        exact_trap_parity_case_count,
        exact_refusal_parity_case_count,
        exact_trap_stack_parity_case_count,
        drift_case_count,
        case_receipts,
        claim_boundary: String::from(
            "this runtime report keeps typed throw, catch, and rethrow semantics benchmark-bound inside one named exceptions profile on the current-host cpu-reference lane. It preserves trap-stack parity and malformed-handler refusal truth explicitly instead of inheriting arbitrary exception-family support from generic trap parity",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Exception profile runtime report covers {} cases with success_parity={}, trap_parity={}, refusal_parity={}, trap_stack_parity={}, drift={}.",
        report.case_receipts.len(),
        report.exact_success_parity_case_count,
        report.exact_trap_parity_case_count,
        report.exact_refusal_parity_case_count,
        report.exact_trap_stack_parity_case_count,
        report.drift_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_exception_profile_runtime_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_exception_profile_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXCEPTION_PROFILE_RUNTIME_REPORT_REF)
}

/// Writes the committed runtime report.
pub fn write_tassadar_exception_profile_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExceptionProfileRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_exception_profile_runtime_report();
    let json =
        serde_json::to_string_pretty(&report).expect("exception-profile runtime report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_exception_profile_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarExceptionProfileRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    case_id: &str,
    semantic_id: &str,
    workload_family: &str,
    reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    non_success_kind: Option<&str>,
    parity_posture: TassadarTrapExceptionParityPosture,
    output_parity: bool,
    trap_stack_parity: bool,
    refusal_state_parity: bool,
    reference_trap_stack_depth: Option<u32>,
    runtime_trap_stack_depth: Option<u32>,
    reference_detail: &str,
    runtime_detail: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarExceptionProfileCaseReceipt {
    TassadarExceptionProfileCaseReceipt {
        case_id: String::from(case_id),
        semantic_id: String::from(semantic_id),
        workload_family: String::from(workload_family),
        reference_terminal_kind,
        runtime_terminal_kind,
        non_success_kind: non_success_kind.map(String::from),
        parity_posture,
        output_parity,
        trap_stack_parity,
        refusal_state_parity,
        reference_trap_stack_depth,
        runtime_trap_stack_depth,
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
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
        build_tassadar_exception_profile_runtime_report,
        load_tassadar_exception_profile_runtime_report,
        tassadar_exception_profile_runtime_report_path,
        write_tassadar_exception_profile_runtime_report,
    };
    use crate::TassadarTrapExceptionParityPosture;

    #[test]
    fn exception_profile_runtime_report_keeps_throw_catch_and_trap_stack_parity_explicit() {
        let report = build_tassadar_exception_profile_runtime_report();

        assert_eq!(report.case_receipts.len(), 4);
        assert_eq!(report.exact_trap_parity_case_count, 2);
        assert_eq!(report.exact_refusal_parity_case_count, 1);
        assert_eq!(report.exact_trap_stack_parity_case_count, 2);
        assert!(report.case_receipts.iter().any(|case| {
            case.case_id == "nested_rethrow_trap_stack"
                && case.parity_posture == TassadarTrapExceptionParityPosture::ExactTrapParity
                && case.trap_stack_parity
        }));
    }

    #[test]
    fn exception_profile_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_exception_profile_runtime_report();
        let committed = load_tassadar_exception_profile_runtime_report(
            tassadar_exception_profile_runtime_report_path(),
        )
        .expect("committed exception-profile runtime report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_exception_profile_runtime_report_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_exception_profile_runtime_report.json");
        let expected =
            write_tassadar_exception_profile_runtime_report(&output_path).expect("write report");
        let persisted = load_tassadar_exception_profile_runtime_report(&output_path)
            .expect("persisted runtime report");

        assert_eq!(persisted, expected);
    }
}
