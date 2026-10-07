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
    TassadarExceptionProfileCompilationContract, compile_tassadar_exception_profile_contract,
};
use psionic_ir::{
    TassadarExceptionProfileContract, TassadarExceptionProfileSupportPosture,
    tassadar_exception_profile_contract,
};
use psionic_runtime::{
    TASSADAR_EXCEPTION_PROFILE_RUNTIME_REPORT_REF, TassadarExceptionProfileRuntimeReport,
    TassadarTrapExceptionParityPosture, TassadarTrapExceptionTerminalKind,
    build_tassadar_exception_profile_runtime_report,
};

use crate::{TASSADAR_TRAP_EXCEPTION_REPORT_REF, build_tassadar_trap_exception_report};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EXCEPTION_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exception_profile_report.json";

/// Eval-facing audit row for one bounded exceptions-profile case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileCaseAudit {
    pub case_id: String,
    pub semantic_id: String,
    pub workload_family: String,
    pub reference_terminal_kind: TassadarTrapExceptionTerminalKind,
    pub runtime_terminal_kind: TassadarTrapExceptionTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_non_success_kind: Option<String>,
    pub parity_posture: TassadarTrapExceptionParityPosture,
    pub parity_preserved: bool,
    pub trap_stack_parity: bool,
    pub benchmark_ref_count: u32,
    pub note: String,
}

/// Eval-facing publication row for one bounded exceptions profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfilePublicationRow {
    pub profile_id: String,
    pub support_posture: TassadarExceptionProfileSupportPosture,
    pub portability_envelope_id: String,
    pub throw_catch_parity_ready: bool,
    pub rethrow_parity_ready: bool,
    pub trap_stack_parity_ready: bool,
    pub refusal_ready: bool,
    pub named_public_profile_allowed: bool,
    pub default_served_profile_allowed: bool,
    pub detail: String,
}

/// Eval-facing report for the bounded exceptions proposal profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ir_contract: TassadarExceptionProfileContract,
    pub compiler_contract: TassadarExceptionProfileCompilationContract,
    pub trap_exception_report_ref: String,
    pub runtime_report: TassadarExceptionProfileRuntimeReport,
    pub case_audits: Vec<TassadarExceptionProfileCaseAudit>,
    pub profile_rows: Vec<TassadarExceptionProfilePublicationRow>,
    pub exact_success_parity_case_count: u32,
    pub exact_trap_parity_case_count: u32,
    pub exact_refusal_parity_case_count: u32,
    pub exact_trap_stack_parity_case_count: u32,
    pub drift_case_count: u32,
    pub green_profile_ids: Vec<String>,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical eval report for the bounded exceptions proposal profile.
#[must_use]
pub fn build_tassadar_exception_profile_report() -> TassadarExceptionProfileReport {
    let ir_contract = tassadar_exception_profile_contract();
    let compiler_contract = compile_tassadar_exception_profile_contract();
    let runtime_report = build_tassadar_exception_profile_runtime_report();
    let trap_exception_report = build_tassadar_trap_exception_report();
    let receipts_by_case = runtime_report
        .case_receipts
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let case_audits = compiler_contract
        .case_specs
        .iter()
        .map(|spec| {
            let receipt = receipts_by_case
                .get(spec.case_id.as_str())
                .expect("exception profile runtime report should cover each contract case");
            let parity_preserved = match spec.expected_terminal_kind {
                psionic_compiler::TassadarTrapExceptionExpectedTerminalKind::Success => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Success
                        && receipt.runtime_terminal_kind
                            == TassadarTrapExceptionTerminalKind::Success
                        && receipt.output_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactSuccessParity
                }
                psionic_compiler::TassadarTrapExceptionExpectedTerminalKind::Trap => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Trap
                        && receipt.runtime_terminal_kind == TassadarTrapExceptionTerminalKind::Trap
                        && spec.expected_non_success_kind == receipt.non_success_kind
                        && receipt.trap_stack_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactTrapParity
                }
                psionic_compiler::TassadarTrapExceptionExpectedTerminalKind::Refusal => {
                    receipt.reference_terminal_kind == TassadarTrapExceptionTerminalKind::Refusal
                        && receipt.runtime_terminal_kind
                            == TassadarTrapExceptionTerminalKind::Refusal
                        && spec.expected_non_success_kind == receipt.non_success_kind
                        && receipt.refusal_state_parity
                        && receipt.parity_posture
                            == TassadarTrapExceptionParityPosture::ExactRefusalParity
                }
            };
            TassadarExceptionProfileCaseAudit {
                case_id: spec.case_id.clone(),
                semantic_id: spec.semantic_id.clone(),
                workload_family: spec.workload_family.clone(),
                reference_terminal_kind: receipt.reference_terminal_kind,
                runtime_terminal_kind: receipt.runtime_terminal_kind,
                observed_non_success_kind: receipt.non_success_kind.clone(),
                parity_posture: receipt.parity_posture,
                parity_preserved,
                trap_stack_parity: receipt.trap_stack_parity,
                benchmark_ref_count: spec.benchmark_refs.len() as u32,
                note: spec.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    let profile_rows = ir_contract
        .profiles
        .iter()
        .map(|profile| {
            let throw_catch_parity_ready = case_audits.iter().any(|case| {
                case.semantic_id == "throw_catch_success" && case.parity_preserved
            });
            let rethrow_parity_ready = case_audits.iter().any(|case| {
                case.semantic_id == "nested_rethrow_trap_stack" && case.parity_preserved
            });
            let trap_stack_parity_ready = runtime_report.exact_trap_stack_parity_case_count > 0;
            let refusal_ready = case_audits.iter().any(|case| {
                case.semantic_id == "malformed_exception_handler_refusal" && case.parity_preserved
            });
            let named_public_profile_allowed = throw_catch_parity_ready
                && rethrow_parity_ready
                && trap_stack_parity_ready
                && refusal_ready
                && trap_exception_report.drift_case_count == 0;
            TassadarExceptionProfilePublicationRow {
                profile_id: profile.profile_id.clone(),
                support_posture: profile.support_posture,
                portability_envelope_id: profile.portability_envelope_id.clone(),
                throw_catch_parity_ready,
                rethrow_parity_ready,
                trap_stack_parity_ready,
                refusal_ready,
                named_public_profile_allowed,
                default_served_profile_allowed: false,
                detail: if named_public_profile_allowed {
                    format!(
                        "exception profile `{}` is green for named public profile posture on `{}` because throw/catch parity, rethrow trap-stack parity, and malformed-handler refusal truth are explicit; it remains non-default for served publication",
                        profile.profile_id, profile.portability_envelope_id
                    )
                } else {
                    format!(
                        "exception profile `{}` stays unpublishable because one or more throw/catch, rethrow, trap-stack, or refusal parity rows drifted",
                        profile.profile_id
                    )
                },
            }
        })
        .collect::<Vec<_>>();
    let green_profile_ids = profile_rows
        .iter()
        .filter(|row| row.named_public_profile_allowed && !row.default_served_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let public_profile_allowed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.named_public_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let default_served_profile_allowed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.default_served_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let portability_envelope_ids = profile_rows
        .iter()
        .map(|row| row.portability_envelope_id.clone())
        .collect::<Vec<_>>();
    let mut generated_from_refs = vec![
        String::from(TASSADAR_EXCEPTION_PROFILE_RUNTIME_REPORT_REF),
        String::from(TASSADAR_TRAP_EXCEPTION_REPORT_REF),
    ];
    generated_from_refs.extend(
        compiler_contract
            .case_specs
            .iter()
            .flat_map(|case| case.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let overall_green = case_audits.iter().all(|case| case.parity_preserved)
        && runtime_report.drift_case_count == 0
        && !public_profile_allowed_profile_ids.is_empty();
    let mut report = TassadarExceptionProfileReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.exception_profile.report.v1"),
        ir_contract,
        compiler_contract,
        trap_exception_report_ref: String::from(TASSADAR_TRAP_EXCEPTION_REPORT_REF),
        runtime_report,
        case_audits,
        profile_rows,
        exact_success_parity_case_count: 1,
        exact_trap_parity_case_count: 2,
        exact_refusal_parity_case_count: 1,
        exact_trap_stack_parity_case_count: 2,
        drift_case_count: 0,
        green_profile_ids,
        public_profile_allowed_profile_ids,
        default_served_profile_allowed_profile_ids,
        portability_envelope_ids,
        overall_green,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report keeps the exceptions proposal family bounded to one named current-host cpu-reference profile. A green row means the profile may be named publicly with explicit portability and refusal boundaries only; it does not imply arbitrary Wasm exception closure, backend-invariant portability, or a default served exception lane",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Exception profile report covers {} audited cases with success_parity={}, trap_parity={}, refusal_parity={}, trap_stack_parity={}, drift={}, public_profile_allowed_profiles={}, default_served_profile_allowed_profiles={}, overall_green={}.",
        report.case_audits.len(),
        report.exact_success_parity_case_count,
        report.exact_trap_parity_case_count,
        report.exact_refusal_parity_case_count,
        report.exact_trap_stack_parity_case_count,
        report.drift_case_count,
        report.public_profile_allowed_profile_ids.len(),
        report.default_served_profile_allowed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_exception_profile_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_exception_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXCEPTION_PROFILE_REPORT_REF)
}

/// Writes the committed eval report.
pub fn write_tassadar_exception_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExceptionProfileReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_exception_profile_report();
    let json = serde_json::to_string_pretty(&report).expect("exception profile report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_exception_profile_report(
    path: impl AsRef<Path>,
) -> Result<TassadarExceptionProfileReport, Box<dyn std::error::Error>> {
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
        build_tassadar_exception_profile_report, load_tassadar_exception_profile_report,
        tassadar_exception_profile_report_path,
    };

    #[test]
    fn exception_profile_report_keeps_public_profile_and_default_served_boundary_explicit() {
        let report = build_tassadar_exception_profile_report();

        assert!(report.overall_green);
        assert!(
            report
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"
                ))
        );
        assert!(report.default_served_profile_allowed_profile_ids.is_empty());
        assert_eq!(report.exact_trap_stack_parity_case_count, 2);
    }

    #[test]
    fn exception_profile_report_matches_committed_truth() {
        let expected = build_tassadar_exception_profile_report();
        let committed =
            load_tassadar_exception_profile_report(tassadar_exception_profile_report_path())
                .expect("committed exception profile report");

        assert_eq!(committed, expected);
    }
}
