use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_PROCESS_OBJECT_REPORT_REF, TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
    TASSADAR_SPILL_TAPE_STORE_REPORT_REF, TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
    TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    TassadarMinimalUniversalSubstrateAcceptanceGateReportError, TassadarProcessObjectReport,
    TassadarProcessObjectReportError, TassadarSessionProcessProfileReport,
    TassadarSessionProcessProfileReportError, TassadarSpillTapeStoreReport,
    TassadarSpillTapeStoreReportError, TassadarUniversalMachineProofReport,
    TassadarUniversalMachineProofReportError,
    build_tassadar_minimal_universal_substrate_acceptance_gate_report,
    build_tassadar_process_object_report, build_tassadar_session_process_profile_report,
    build_tassadar_spill_tape_store_report, build_tassadar_universal_machine_proof_report,
};

pub const TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universality_verdict_split_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUniversalityVerdictLevel {
    Theory,
    Operator,
    Served,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUniversalityVerdictStatus {
    Green,
    Suppressed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityVerdictRow {
    pub verdict_level: TassadarUniversalityVerdictLevel,
    pub verdict_status: TassadarUniversalityVerdictStatus,
    pub allowed_statement: String,
    pub artifact_refs: Vec<String>,
    pub route_constraint_ids: Vec<String>,
    pub allowed_profile_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityVerdictSplitReport {
    pub schema_version: u16,
    pub report_id: String,
    pub universal_machine_proof_report_ref: String,
    pub minimal_universal_substrate_gate_report_ref: String,
    pub session_process_profile_report_ref: String,
    pub spill_tape_store_report_ref: String,
    pub process_object_report_ref: String,
    pub universal_machine_proof_report: TassadarUniversalMachineProofReport,
    pub minimal_universal_substrate_gate_report:
        TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    pub session_process_profile_report: TassadarSessionProcessProfileReport,
    pub spill_tape_store_report: TassadarSpillTapeStoreReport,
    pub process_object_report: TassadarProcessObjectReport,
    pub verdict_rows: Vec<TassadarUniversalityVerdictRow>,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub overall_green: bool,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalityVerdictSplitReportError {
    #[error(transparent)]
    UniversalMachineProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    MinimalUniversalSubstrateGate(
        #[from] TassadarMinimalUniversalSubstrateAcceptanceGateReportError,
    ),
    #[error(transparent)]
    SessionProcess(#[from] TassadarSessionProcessProfileReportError),
    #[error(transparent)]
    SpillTapeStore(#[from] TassadarSpillTapeStoreReportError),
    #[error(transparent)]
    ProcessObject(#[from] TassadarProcessObjectReportError),
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

pub fn build_tassadar_universality_verdict_split_report()
-> Result<TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError> {
    let universal_machine_proof_report = build_tassadar_universal_machine_proof_report()?;
    let minimal_universal_substrate_gate_report =
        build_tassadar_minimal_universal_substrate_acceptance_gate_report()?;
    let session_process_profile_report = build_tassadar_session_process_profile_report()?;
    let spill_tape_store_report = build_tassadar_spill_tape_store_report()?;
    let process_object_report = build_tassadar_process_object_report()?;

    let theory_green = universal_machine_proof_report.overall_green
        && minimal_universal_substrate_gate_report.overall_green;
    let operator_green = theory_green
        && session_process_profile_report.overall_green
        && !session_process_profile_report
            .routeable_interaction_surface_ids
            .is_empty()
        && spill_tape_store_report.exact_case_count > 0
        && spill_tape_store_report.refusal_case_count > 0
        && process_object_report.exact_process_parity_count > 0
        && process_object_report.refusal_case_count > 0;
    let served_green = false;

    let verdict_rows = vec![
        TassadarUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Theory,
            verdict_status: if theory_green {
                TassadarUniversalityVerdictStatus::Green
            } else {
                TassadarUniversalityVerdictStatus::Suppressed
            },
            allowed_statement: String::from(
                "Psionic/Tassadar can honestly say that `TCM.v1` has a declared universal substrate plus explicit universal-machine witness constructions under the declared checkpoint-and-resume semantics.",
            ),
            artifact_refs: vec![
                String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
                String::from(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF),
            ],
            route_constraint_ids: vec![String::from("theory_has_no_served_route_claim")],
            allowed_profile_ids: vec![String::from("tcm.v1")],
            blocked_by: Vec::new(),
            detail: String::from(
                "theory-green means there is now one declared universal substrate plus explicit witness encodings and a green minimal universal-substrate gate; it does not by itself imply operator or served widening",
            ),
        },
        TassadarUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Operator,
            verdict_status: if operator_green {
                TassadarUniversalityVerdictStatus::Green
            } else {
                TassadarUniversalityVerdictStatus::Suppressed
            },
            allowed_statement: String::from(
                "Psionic/Tassadar can honestly say that operators have one bounded universality-capable lane under named session-process, spill/tape, and process-object envelopes with exact checkpoint-and-replay evidence.",
            ),
            artifact_refs: vec![
                String::from(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF),
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_PROCESS_OBJECT_REPORT_REF),
            ],
            route_constraint_ids: vec![
                String::from("operator_named_profile_routes_only"),
                String::from("operator_checkpoint_resume_required"),
                String::from("operator_spill_tape_extension_required"),
            ],
            allowed_profile_ids: vec![
                String::from("tassadar.internal_compute.session_process.v1"),
                String::from("tassadar.internal_compute.spill_tape_store.v1"),
                String::from("tassadar.internal_compute.process_objects.v1"),
            ],
            blocked_by: Vec::new(),
            detail: String::from(
                "operator-green means the universal construction survives only inside explicit resumable process envelopes with persisted state objects, spill/tape extension, and typed refusal on broader external loops or ambient effects",
            ),
        },
        TassadarUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Served,
            verdict_status: TassadarUniversalityVerdictStatus::Suppressed,
            allowed_statement: String::from(
                "Psionic/Tassadar does not yet expose a served universality lane; public served posture remains narrower than the theory-green and operator-green claims.",
            ),
            artifact_refs: vec![
                String::from(
                    psionic_models::TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json",
                ),
            ],
            route_constraint_ids: vec![
                String::from("served_universality_route_publication_suppressed"),
                String::from("served_universality_profile_not_selected"),
            ],
            allowed_profile_ids: Vec::new(),
            blocked_by: vec![
                String::from("named_served_universal_profile_missing"),
                String::from("kernel_policy_served_universality_authority_outside_psionic"),
                String::from("nexus_accepted_outcome_closure_outside_psionic"),
            ],
            detail: String::from(
                "served-green remains suppressed because the current served internal-compute lane is still article-closeout, not a published universality profile, and the authority-bearing served closure still lives outside standalone psionic",
            ),
        },
    ];

    let kernel_policy_dependency_marker = String::from(
        "kernel-policy remains the owner of canonical served universality publication policy and settlement-gated authority outside standalone psionic",
    );
    let nexus_dependency_marker = String::from(
        "nexus remains the owner of canonical accepted-outcome issuance and settlement-qualified served closure outside standalone psionic",
    );
    let overall_green = theory_green
        && operator_green
        && verdict_rows.iter().any(|row| {
            row.verdict_level == TassadarUniversalityVerdictLevel::Served
                && row.verdict_status == TassadarUniversalityVerdictStatus::Suppressed
        });

    let mut report = TassadarUniversalityVerdictSplitReport {
        schema_version: 1,
        report_id: String::from("tassadar.universality_verdict_split.report.v1"),
        universal_machine_proof_report_ref: String::from(
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
        ),
        minimal_universal_substrate_gate_report_ref: String::from(
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        session_process_profile_report_ref: String::from(
            TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
        ),
        spill_tape_store_report_ref: String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        process_object_report_ref: String::from(TASSADAR_PROCESS_OBJECT_REPORT_REF),
        universal_machine_proof_report,
        minimal_universal_substrate_gate_report,
        session_process_profile_report,
        spill_tape_store_report,
        process_object_report,
        verdict_rows,
        theory_green,
        operator_green,
        served_green,
        overall_green,
        kernel_policy_dependency_marker,
        nexus_dependency_marker,
        claim_boundary: String::from(
            "this report is the explicit theory/operator/served split for the universality claim. It keeps theoretical universality, operator-owned resumable execution, and served/public posture separate so the terminal claim cannot silently widen into broader publication or settlement posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Universality verdict split keeps theory_green={}, operator_green={}, served_green={}, overall_green={}.",
        report.theory_green, report.operator_green, report.served_green, report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_universality_verdict_split_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_universality_verdict_split_report_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)
}

pub fn write_tassadar_universality_verdict_split_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalityVerdictSplitReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_universality_verdict_split_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalityVerdictSplitReportError::Write {
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
) -> Result<T, TassadarUniversalityVerdictSplitReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarUniversalityVerdictSplitReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalityVerdictSplitReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF, TassadarUniversalityVerdictLevel,
        TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictStatus,
        build_tassadar_universality_verdict_split_report, read_json,
        tassadar_universality_verdict_split_report_path,
        write_tassadar_universality_verdict_split_report,
    };
    use tempfile::tempdir;

    #[test]
    fn universality_verdict_split_keeps_theory_operator_and_served_separate() {
        let report = build_tassadar_universality_verdict_split_report().expect("report");

        assert!(report.theory_green);
        assert!(report.operator_green);
        assert!(!report.served_green);
        assert!(report.overall_green);

        let served_row = report
            .verdict_rows
            .iter()
            .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Served)
            .expect("served row");
        assert_eq!(
            served_row.verdict_status,
            TassadarUniversalityVerdictStatus::Suppressed
        );
        assert!(
            served_row
                .blocked_by
                .contains(&String::from("named_served_universal_profile_missing"))
        );
        assert!(served_row.route_constraint_ids.contains(&String::from(
            "served_universality_route_publication_suppressed"
        )));
    }

    #[test]
    fn universality_verdict_split_matches_committed_truth() {
        let generated = build_tassadar_universality_verdict_split_report().expect("report");
        let committed: TassadarUniversalityVerdictSplitReport =
            read_json(tassadar_universality_verdict_split_report_path()).expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_universality_verdict_split_report.json"
        );
    }

    #[test]
    fn write_universality_verdict_split_persists_current_truth() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir
            .path()
            .join("tassadar_universality_verdict_split_report.json");
        let report =
            write_tassadar_universality_verdict_split_report(&output_path).expect("report");
        let reloaded: TassadarUniversalityVerdictSplitReport =
            read_json(&output_path).expect("reloaded");
        assert_eq!(report, reloaded);
    }
}
