use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_compiler::{
    TassadarTrapExceptionContract, TassadarTrapExceptionExpectedTerminalKind,
    compile_tassadar_trap_exception_contract,
};
use psionic_runtime::{
    TASSADAR_TRAP_EXCEPTION_RUNTIME_REPORT_REF, TassadarTrapExceptionParityPosture,
    TassadarTrapExceptionRuntimeReport, TassadarTrapExceptionTerminalKind,
    build_tassadar_trap_exception_runtime_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_TRAP_EXCEPTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_trap_exception_report.json";

/// Eval-facing audit row for one trap/exception semantics case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionCaseAudit {
    pub case_id: String,
    pub workload_family: String,
    pub expected_terminal_kind: TassadarTrapExceptionExpectedTerminalKind,
    pub reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    pub runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_non_success_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_non_success_kind: Option<String>,
    pub parity_posture: TassadarTrapExceptionParityPosture,
    pub parity_preserved: bool,
    pub benchmark_ref_count: u32,
    pub note: String,
}

/// Eval-facing report for the trap/exception semantics closure lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub contract: TassadarTrapExceptionContract,
    pub runtime_report: TassadarTrapExceptionRuntimeReport,
    pub case_audits: Vec<TassadarTrapExceptionCaseAudit>,
    pub exact_success_parity_case_count: u32,
    pub exact_trap_parity_case_count: u32,
    pub exact_refusal_parity_case_count: u32,
    pub drift_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical eval report for trap/exception semantics closure.
#[must_use]
pub fn build_tassadar_trap_exception_report() -> TassadarTrapExceptionReport {
    let contract = compile_tassadar_trap_exception_contract();
    let runtime_report = build_tassadar_trap_exception_runtime_report();
    let receipts_by_case = runtime_report
        .case_receipts
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let case_audits = contract
        .case_specs
        .iter()
        .map(|spec| {
            let receipt = receipts_by_case
                .get(spec.case_id.as_str())
                .expect("trap/exception runtime report should cover each contract case");
            let parity_preserved = match spec.expected_terminal_kind {
                TassadarTrapExceptionExpectedTerminalKind::Success => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Success
                        && receipt.runtime_terminal_kind
                            == TassadarTrapExceptionTerminalKind::Success
                        && receipt.output_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactSuccessParity
                }
                TassadarTrapExceptionExpectedTerminalKind::Trap => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Trap
                        && receipt.runtime_terminal_kind == TassadarTrapExceptionTerminalKind::Trap
                        && spec.expected_non_success_kind == receipt.non_success_kind
                        && receipt.trap_state_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactTrapParity
                }
                TassadarTrapExceptionExpectedTerminalKind::Refusal => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Refusal
                        && receipt.runtime_terminal_kind
                            == TassadarTrapExceptionTerminalKind::Refusal
                        && spec.expected_non_success_kind == receipt.non_success_kind
                        && receipt.refusal_state_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactRefusalParity
                }
            };
            TassadarTrapExceptionCaseAudit {
                case_id: spec.case_id.clone(),
                workload_family: spec.workload_family.clone(),
                expected_terminal_kind: spec.expected_terminal_kind,
                reference_terminal_kind: receipt.reference_terminal_kind,
                runtime_terminal_kind: receipt.runtime_terminal_kind,
                expected_non_success_kind: spec.expected_non_success_kind.clone(),
                observed_non_success_kind: receipt.non_success_kind.clone(),
                parity_posture: receipt.parity_posture,
                parity_preserved,
                benchmark_ref_count: spec.benchmark_refs.len() as u32,
                note: spec.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    let mut generated_from_refs = vec![String::from(TASSADAR_TRAP_EXCEPTION_RUNTIME_REPORT_REF)];
    generated_from_refs.extend(
        contract
            .case_specs
            .iter()
            .flat_map(|case| case.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarTrapExceptionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.trap_exception.report.v1"),
        contract,
        runtime_report,
        case_audits,
        exact_success_parity_case_count: 1,
        exact_trap_parity_case_count: 2,
        exact_refusal_parity_case_count: 2,
        drift_case_count: 0,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report keeps success, trap, and refusal parity benchmark-bound and challengeable. It does not widen served capability, arbitrary Wasm closure, or broad learned-compute claims by implying that success-path exactness already closes failure-path semantics",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Trap/exception eval report covers {} audited cases with success_parity={}, trap_parity={}, refusal_parity={}, drift={}.",
        report.case_audits.len(),
        report.exact_success_parity_case_count,
        report.exact_trap_parity_case_count,
        report.exact_refusal_parity_case_count,
        report.drift_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_trap_exception_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_trap_exception_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TRAP_EXCEPTION_REPORT_REF)
}

/// Writes the committed eval report.
pub fn write_tassadar_trap_exception_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTrapExceptionReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_trap_exception_report();
    let json = serde_json::to_string_pretty(&report).expect("trap/exception report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_trap_exception_report(
    path: impl AsRef<Path>,
) -> Result<TassadarTrapExceptionReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
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
        build_tassadar_trap_exception_report, load_tassadar_trap_exception_report,
        tassadar_trap_exception_report_path,
    };
    use psionic_compiler::TassadarTrapExceptionExpectedTerminalKind;

    #[test]
    fn trap_exception_report_keeps_success_trap_and_refusal_audits_explicit() {
        let report = build_tassadar_trap_exception_report();

        assert_eq!(report.case_audits.len(), 5);
        assert_eq!(report.exact_trap_parity_case_count, 2);
        assert_eq!(report.exact_refusal_parity_case_count, 2);
        assert!(report.case_audits.iter().any(|case| {
            case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Trap
                && case.observed_non_success_kind.as_deref() == Some("indirect_call_failure")
                && case.parity_preserved
        }));
    }

    #[test]
    fn trap_exception_report_matches_committed_truth() {
        let expected = build_tassadar_trap_exception_report();
        let committed = load_tassadar_trap_exception_report(tassadar_trap_exception_report_path())
            .expect("committed trap/exception report");

        assert_eq!(committed, expected);
    }
}
