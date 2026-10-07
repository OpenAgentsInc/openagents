use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::{
    tassadar_universality_witness_suite_contract, TassadarModuleScaleWorkloadFamily,
    TassadarModuleScaleWorkloadStatus, TassadarUniversalityWitnessExpectation,
    TassadarUniversalityWitnessFamily, TassadarUniversalityWitnessSuiteContract,
    TassadarUniversalityWitnessSuiteError, TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
};
use psionic_environments::{
    default_tassadar_universality_witness_suite_binding, TassadarEnvironmentError,
    TassadarUniversalityWitnessSuiteBinding,
};
use psionic_runtime::{
    build_tassadar_tcm_v1_runtime_contract_report, TassadarTcmV1RuntimeContractReportError,
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

use crate::{
    build_tassadar_module_scale_workload_suite_report,
    build_tassadar_session_process_profile_report, build_tassadar_spill_tape_store_report,
    build_tassadar_universal_machine_proof_report, TassadarModuleScaleWorkloadSuiteReportError,
    TassadarSessionProcessProfileReportError, TassadarSpillTapeStoreReportError,
    TassadarUniversalMachineProofReportError, TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF, TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
    TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessFamilyRow {
    pub witness_family: TassadarUniversalityWitnessFamily,
    pub expected_status: TassadarUniversalityWitnessExpectation,
    pub satisfied: bool,
    pub evidence_anchor_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_runtime_parity: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_resume_equivalent: Option<bool>,
    pub refusal_boundary_held: bool,
    pub runtime_envelope: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessSuiteReport {
    pub schema_version: u16,
    pub report_id: String,
    pub suite_contract: TassadarUniversalityWitnessSuiteContract,
    pub suite_binding: TassadarUniversalityWitnessSuiteBinding,
    pub generated_from_refs: Vec<String>,
    pub family_rows: Vec<TassadarUniversalityWitnessFamilyRow>,
    pub exact_family_count: u32,
    pub refusal_boundary_count: u32,
    pub overall_green: bool,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalityWitnessSuiteReportError {
    #[error(transparent)]
    Contract(#[from] TassadarUniversalityWitnessSuiteError),
    #[error(transparent)]
    Environment(#[from] TassadarEnvironmentError),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    UniversalMachineProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    ModuleScale(#[from] TassadarModuleScaleWorkloadSuiteReportError),
    #[error(transparent)]
    SessionProcess(#[from] TassadarSessionProcessProfileReportError),
    #[error(transparent)]
    SpillTape(#[from] TassadarSpillTapeStoreReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_universality_witness_suite_report(
) -> Result<TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError> {
    let suite_contract = tassadar_universality_witness_suite_contract();
    let suite_binding = default_tassadar_universality_witness_suite_binding();
    suite_binding.validate()?;

    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let universal_machine_proof = build_tassadar_universal_machine_proof_report()?;
    let module_scale_report = build_tassadar_module_scale_workload_suite_report()?;
    let session_process_report = build_tassadar_session_process_profile_report()?;
    let spill_tape_report = build_tassadar_spill_tape_store_report()?;

    let register_row = universal_machine_proof
        .proof_rows
        .iter()
        .find(|row| row.encoding_id == "tcm.encoding.two_register_counter_loop.v1")
        .expect("register witness row should exist");
    let tape_row = universal_machine_proof
        .proof_rows
        .iter()
        .find(|row| row.encoding_id == "tcm.encoding.single_tape_bit_flip.v1")
        .expect("tape witness row should exist");
    let vm_exact_case = module_scale_report
        .cases
        .iter()
        .find(|case| {
            case.family == TassadarModuleScaleWorkloadFamily::VmStyle
                && case.status == TassadarModuleScaleWorkloadStatus::LoweredExact
        })
        .expect("vm exact case should exist");
    let vm_refusal_case = module_scale_report
        .cases
        .iter()
        .find(|case| {
            case.family == TassadarModuleScaleWorkloadFamily::VmStyle
                && case.status == TassadarModuleScaleWorkloadStatus::LoweringRefused
        })
        .expect("vm refusal case should exist");
    let session_exact_surface_ids = session_process_report
        .routeable_interaction_surface_ids
        .clone();
    let session_refusal_surface_ids = session_process_report
        .refused_interaction_surface_ids
        .clone();
    let spill_exact_case_ids = spill_tape_report
        .case_reports
        .iter()
        .filter(|case| {
            case.status == psionic_runtime::TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let spill_refusal_case_ids = spill_tape_report
        .case_reports
        .iter()
        .filter(|case| !case.refusal_kinds.is_empty())
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();

    let family_rows = vec![
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::RegisterMachine,
            expected_status: TassadarUniversalityWitnessExpectation::Exact,
            satisfied: register_row.satisfied,
            evidence_anchor_ids: vec![String::from("tcm.encoding.two_register_counter_loop.v1")],
            exact_runtime_parity: Some(
                register_row.step_parity && register_row.final_state_parity,
            ),
            checkpoint_resume_equivalent: Some(register_row.checkpoint_resume_equivalent),
            refusal_boundary_held: false,
            runtime_envelope: runtime_contract.runtime_envelope.clone(),
            note: String::from(
                "the dedicated witness suite keeps the two-register construction as the minimal register-machine anchor over `TCM.v1` exact step and final-state parity",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::TapeMachine,
            expected_status: TassadarUniversalityWitnessExpectation::Exact,
            satisfied: tape_row.satisfied,
            evidence_anchor_ids: vec![String::from("tcm.encoding.single_tape_bit_flip.v1")],
            exact_runtime_parity: Some(tape_row.step_parity && tape_row.final_state_parity),
            checkpoint_resume_equivalent: Some(tape_row.checkpoint_resume_equivalent),
            refusal_boundary_held: false,
            runtime_envelope: runtime_contract.runtime_envelope.clone(),
            note: String::from(
                "the dedicated witness suite keeps the single-tape construction as the minimal tape-machine anchor over `TCM.v1` exact step and final-state parity",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::BytecodeVmInterpreter,
            expected_status: TassadarUniversalityWitnessExpectation::Exact,
            satisfied: vm_exact_case.exactness_bps == Some(10_000),
            evidence_anchor_ids: vec![vm_exact_case.case_id.clone()],
            exact_runtime_parity: Some(vm_exact_case.exactness_bps == Some(10_000)),
            checkpoint_resume_equivalent: None,
            refusal_boundary_held: vm_refusal_case.refusal_kind.is_some(),
            runtime_envelope: String::from("bounded vm-style module family under the current Wasm-text-to-Tassadar lowering lane"),
            note: String::from(
                "the dedicated witness suite keeps bytecode-vm interpretation bounded to the exact vm-style module case while holding the parameter-ABI sibling on explicit refusal",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::SessionProcessKernel,
            expected_status: TassadarUniversalityWitnessExpectation::Exact,
            satisfied: session_process_report.overall_green && !session_exact_surface_ids.is_empty(),
            evidence_anchor_ids: session_exact_surface_ids.clone(),
            exact_runtime_parity: Some(session_process_report.overall_green),
            checkpoint_resume_equivalent: Some(true),
            refusal_boundary_held: !session_refusal_surface_ids.is_empty(),
            runtime_envelope: String::from("deterministic finite-turn session-process profile with persisted local state"),
            note: String::from(
                "the dedicated witness suite keeps process-kernel execution grounded in deterministic echo and counter-turn loops with explicit refusal on open-ended external streams",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::SpillTapeContinuation,
            expected_status: TassadarUniversalityWitnessExpectation::Exact,
            satisfied: !spill_exact_case_ids.is_empty(),
            evidence_anchor_ids: spill_exact_case_ids.clone(),
            exact_runtime_parity: Some(!spill_exact_case_ids.is_empty()),
            checkpoint_resume_equivalent: Some(true),
            refusal_boundary_held: !spill_refusal_case_ids.is_empty(),
            runtime_envelope: String::from("current-host cpu-reference spill/tape continuation envelope"),
            note: String::from(
                "the dedicated witness suite keeps spill/tape continuation grounded in explicit spill-segment and external-tape artifacts rather than implicit persistent-memory claims",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::BytecodeVmParamBoundary,
            expected_status: TassadarUniversalityWitnessExpectation::RefusalBoundary,
            satisfied: vm_refusal_case.refusal_kind.is_some(),
            evidence_anchor_ids: vec![vm_refusal_case.case_id.clone()],
            exact_runtime_parity: None,
            checkpoint_resume_equivalent: None,
            refusal_boundary_held: vm_refusal_case.refusal_kind.is_some(),
            runtime_envelope: String::from("bounded vm-style module family under the current Wasm-text-to-Tassadar lowering lane"),
            note: String::from(
                "the dedicated witness suite keeps vm-style parameter ABI widening on an explicit refusal boundary rather than silently admitting broader call signatures",
            ),
        },
        TassadarUniversalityWitnessFamilyRow {
            witness_family: TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary,
            expected_status: TassadarUniversalityWitnessExpectation::RefusalBoundary,
            satisfied: session_refusal_surface_ids
                .iter()
                .any(|surface| surface == "open_ended_external_event_stream"),
            evidence_anchor_ids: session_refusal_surface_ids.clone(),
            exact_runtime_parity: None,
            checkpoint_resume_equivalent: None,
            refusal_boundary_held: session_refusal_surface_ids
                .iter()
                .any(|surface| surface == "open_ended_external_event_stream"),
            runtime_envelope: String::from("deterministic finite-turn session-process profile with persisted local state"),
            note: String::from(
                "the dedicated witness suite keeps open-ended external event loops on explicit refusal instead of reading process-kernel evidence as generic agent-loop closure",
            ),
        },
    ];

    let exact_family_count = family_rows
        .iter()
        .filter(|row| row.expected_status == TassadarUniversalityWitnessExpectation::Exact)
        .filter(|row| row.satisfied)
        .count() as u32;
    let refusal_boundary_count = family_rows
        .iter()
        .filter(|row| {
            row.expected_status == TassadarUniversalityWitnessExpectation::RefusalBoundary
        })
        .filter(|row| row.satisfied)
        .count() as u32;
    let overall_green = family_rows.iter().all(|row| row.satisfied)
        && exact_family_count == suite_binding.exact_family_ids.len() as u32
        && refusal_boundary_count == suite_binding.refusal_boundary_family_ids.len() as u32;
    let explicit_non_implications = vec![
        String::from("minimal universal-substrate gate"),
        String::from("theory/operator/served verdict split"),
        String::from("served universality posture"),
        String::from("arbitrary Wasm"),
    ];
    let mut report = TassadarUniversalityWitnessSuiteReport {
        schema_version: 1,
        report_id: String::from("tassadar.universality_witness_suite.report.v1"),
        suite_contract,
        suite_binding,
        generated_from_refs: vec![
            String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
            String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
            String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
            String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        ],
        family_rows,
        exact_family_count,
        refusal_boundary_count,
        overall_green,
        explicit_non_implications,
        claim_boundary: String::from(
            "this report closes the dedicated universality witness benchmark suite only. It keeps the benchmark surface bounded to explicit `TCM.v1` witness constructions, vm-style interpreter kernels, session-process kernels, spill/tape continuation, and named refusal boundaries without claiming the minimal universal-substrate gate, the final verdict split, arbitrary Wasm, or served universality posture",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Universality witness suite report keeps family_rows={}, exact_family_count={}, refusal_boundary_count={}, overall_green={}.",
        report.family_rows.len(),
        report.exact_family_count,
        report.refusal_boundary_count,
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_universality_witness_suite_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_universality_witness_suite_report_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)
}

pub fn write_tassadar_universality_witness_suite_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalityWitnessSuiteReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_universality_witness_suite_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalityWitnessSuiteReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarUniversalityWitnessSuiteReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarUniversalityWitnessSuiteReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalityWitnessSuiteReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universality_witness_suite_report, read_json,
        tassadar_universality_witness_suite_report_path, TassadarUniversalityWitnessSuiteReport,
    };
    use psionic_data::{
        TassadarUniversalityWitnessFamily, TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
    };

    #[test]
    fn universality_witness_suite_report_keeps_exact_and_refusal_rows_green() {
        let report = build_tassadar_universality_witness_suite_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.exact_family_count, 5);
        assert_eq!(report.refusal_boundary_count, 2);
        assert!(report.family_rows.iter().any(|row| {
            row.witness_family == TassadarUniversalityWitnessFamily::RegisterMachine
                && row.satisfied
        }));
        assert!(report.family_rows.iter().any(|row| {
            row.witness_family == TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary
                && row.refusal_boundary_held
        }));
    }

    #[test]
    fn universality_witness_suite_report_matches_committed_truth() {
        let generated = build_tassadar_universality_witness_suite_report().expect("report");
        let committed: TassadarUniversalityWitnessSuiteReport =
            read_json(tassadar_universality_witness_suite_report_path()).expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_universality_witness_suite_report.json"
        );
    }
}
