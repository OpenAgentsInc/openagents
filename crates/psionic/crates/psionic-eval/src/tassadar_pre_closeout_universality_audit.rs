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
    build_tassadar_broad_general_compute_validator_bridge_report,
    build_tassadar_effective_unbounded_compute_claim_report,
    build_tassadar_full_core_wasm_public_acceptance_gate_report,
    build_tassadar_general_internal_compute_red_team_audit_report,
    build_tassadar_proposal_profile_ladder_claim_checker_report,
    TassadarBroadGeneralComputeValidatorBridgeReportError,
    TassadarEffectiveUnboundedComputeClaimReportError,
    TassadarFullCoreWasmPublicAcceptanceGateReportError,
    TassadarGeneralInternalComputeRedTeamAuditReportError,
    TassadarProposalProfileLadderClaimCheckerReportError,
    TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF,
    TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF,
    TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_AUDIT_REPORT_REF,
    TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF,
};

pub const TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_pre_closeout_universality_audit_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPreCloseoutUniversalityClaimStatus {
    Green,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreCloseoutUniversalityPrerequisiteRow {
    pub prerequisite_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreCloseoutUniversalityAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub prerequisite_rows: Vec<TassadarPreCloseoutUniversalityPrerequisiteRow>,
    pub satisfied_prerequisite_ids: Vec<String>,
    pub missing_prerequisite_ids: Vec<String>,
    pub claim_status: TassadarPreCloseoutUniversalityClaimStatus,
    pub allowed_pre_closeout_statement: String,
    pub current_true_scopes: Vec<String>,
    pub remaining_terminal_contract_ids: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPreCloseoutUniversalityAuditReportError {
    #[error(transparent)]
    BroadGeneralCompute(#[from] TassadarBroadGeneralComputeValidatorBridgeReportError),
    #[error(transparent)]
    EffectiveUnbounded(#[from] TassadarEffectiveUnboundedComputeClaimReportError),
    #[error(transparent)]
    FullCoreWasm(#[from] TassadarFullCoreWasmPublicAcceptanceGateReportError),
    #[error(transparent)]
    ProposalProfiles(#[from] TassadarProposalProfileLadderClaimCheckerReportError),
    #[error(transparent)]
    RedTeam(#[from] TassadarGeneralInternalComputeRedTeamAuditReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_pre_closeout_universality_audit_report() -> Result<
    TassadarPreCloseoutUniversalityAuditReport,
    TassadarPreCloseoutUniversalityAuditReportError,
> {
    let broad_general_compute = build_tassadar_broad_general_compute_validator_bridge_report()?;
    let effective_unbounded = build_tassadar_effective_unbounded_compute_claim_report()?;
    let full_core_wasm = build_tassadar_full_core_wasm_public_acceptance_gate_report()?;
    let proposal_profiles = build_tassadar_proposal_profile_ladder_claim_checker_report()?;
    let red_team = build_tassadar_general_internal_compute_red_team_audit_report()?;

    let prerequisite_rows = vec![
        prereq(
            "red_team_boundary_audit",
            red_team.publication_safe,
            &[TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_AUDIT_REPORT_REF],
            "the pre-closeout claim boundary is only meaningful if the broader internal-compute lane has already been red-teamed and found to fail closed",
        ),
        prereq(
            "effective_unbounded_boundary_is_explicit",
            effective_unbounded.claim_status == crate::TassadarEffectiveUnboundedClaimStatus::Suppressed,
            &[TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF],
            "the repo must already distinguish resumable bounded computation from a broader universality claim",
        ),
        prereq(
            "frozen_core_wasm_boundary_is_explicit",
            !full_core_wasm.served_publication_allowed,
            &[TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF],
            "the frozen core-Wasm lane must stay separately gated instead of being over-read as the universality claim",
        ),
        prereq(
            "proposal_family_boundary_is_explicit",
            proposal_profiles.overall_green && proposal_profiles.default_served_profile_ids.is_empty(),
            &[TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF],
            "post-core proposal families must still be separate named profiles with explicit promotion boundaries",
        ),
        prereq(
            "authority_and_economic_bridge_is_named",
            !broad_general_compute.accepted_outcome_ready_profile_ids.is_empty()
                && !broad_general_compute.candidate_only_profile_ids.is_empty(),
            &[TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF],
            "the pre-closeout report only counts broad-compute bridge posture when accepted-outcome-ready and candidate-only lanes remain explicitly distinct",
        ),
        prereq(
            "tcm_v1_declared_substrate_model",
            false,
            &["docs/ROADMAP_TASSADAR.md"],
            "the terminal contract still lacks one declared universal substrate model",
        ),
        prereq(
            "reference_universal_machine_encoding",
            false,
            &["docs/ROADMAP_TASSADAR.md"],
            "the terminal contract still lacks the explicit universal-machine witness encoding",
        ),
        prereq(
            "universality_witness_suite",
            false,
            &["docs/ROADMAP_TASSADAR.md"],
            "the terminal contract still lacks the dedicated universality witness benchmark suite",
        ),
        prereq(
            "minimal_universal_substrate_gate",
            false,
            &["docs/ROADMAP_TASSADAR.md"],
            "the terminal contract still lacks the single green gate over control, memory, continuation, replay, and witness workloads",
        ),
        prereq(
            "universality_verdict_split",
            false,
            &["docs/ROADMAP_TASSADAR.md"],
            "the terminal contract still lacks the theory/operator/served verdict split",
        ),
    ];
    let (claim_status, satisfied_prerequisite_ids, missing_prerequisite_ids) =
        evaluate_claim_status(&prerequisite_rows);
    let current_true_scopes = vec![
        String::from("bounded named-profile internal computation"),
        String::from("disclosure-safe effective-unbounded claim boundary"),
        String::from("separate frozen core-Wasm public gate"),
        String::from("named proposal-family promotion boundary"),
        String::from("named authority-and-economic bridge for broad internal-compute profiles"),
        String::from("disclosure-safe general internal-compute red-team audit"),
    ];
    let remaining_terminal_contract_ids = missing_prerequisite_ids.clone();
    let explicit_non_implications = vec![
        String::from("Turing-complete support"),
        String::from("arbitrary Wasm execution"),
        String::from("broad served internal compute"),
    ];
    let mut report = TassadarPreCloseoutUniversalityAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.pre_closeout_universality_audit.report.v1"),
        prerequisite_rows,
        satisfied_prerequisite_ids,
        missing_prerequisite_ids,
        claim_status,
        allowed_pre_closeout_statement: String::from(
            "Psionic/Tassadar can now say exactly which disclosure-safe broadness and boundary surfaces are real before the terminal universal-substrate contract starts, but it may not yet claim Turing-complete support or a final universality verdict.",
        ),
        current_true_scopes,
        remaining_terminal_contract_ids,
        explicit_non_implications,
        claim_boundary: String::from(
            "this pre-closeout audit freezes the last honest claim boundary before the terminal universal-substrate tranche. It names what is already real, what remains gated, and what still must land before the repo can speak in terminal universality language.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Pre-closeout universality audit keeps satisfied_prerequisites={}, missing_terminal_contract_ids={}, claim_status={:?}.",
        report.satisfied_prerequisite_ids.len(),
        report.remaining_terminal_contract_ids.len(),
        report.claim_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_pre_closeout_universality_audit_report|",
        &report,
    );
    Ok(report)
}

fn prereq(
    prerequisite_id: &str,
    satisfied: bool,
    source_refs: &[&str],
    note: &str,
) -> TassadarPreCloseoutUniversalityPrerequisiteRow {
    TassadarPreCloseoutUniversalityPrerequisiteRow {
        prerequisite_id: String::from(prerequisite_id),
        satisfied,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn evaluate_claim_status(
    prerequisite_rows: &[TassadarPreCloseoutUniversalityPrerequisiteRow],
) -> (
    TassadarPreCloseoutUniversalityClaimStatus,
    Vec<String>,
    Vec<String>,
) {
    let satisfied_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();
    let missing_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| !row.satisfied)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();
    let claim_status = if missing_prerequisite_ids.is_empty() {
        TassadarPreCloseoutUniversalityClaimStatus::Green
    } else if satisfied_prerequisite_ids.is_empty() {
        TassadarPreCloseoutUniversalityClaimStatus::Failed
    } else {
        TassadarPreCloseoutUniversalityClaimStatus::Suppressed
    };
    (
        claim_status,
        satisfied_prerequisite_ids,
        missing_prerequisite_ids,
    )
}

#[must_use]
pub fn tassadar_pre_closeout_universality_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PRE_CLOSEOUT_UNIVERSALITY_AUDIT_REPORT_REF)
}

pub fn write_tassadar_pre_closeout_universality_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPreCloseoutUniversalityAuditReport,
    TassadarPreCloseoutUniversalityAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPreCloseoutUniversalityAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_pre_closeout_universality_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPreCloseoutUniversalityAuditReportError::Write {
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
) -> Result<T, TassadarPreCloseoutUniversalityAuditReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarPreCloseoutUniversalityAuditReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPreCloseoutUniversalityAuditReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_pre_closeout_universality_audit_report, read_json,
        tassadar_pre_closeout_universality_audit_report_path,
        TassadarPreCloseoutUniversalityAuditReport, TassadarPreCloseoutUniversalityClaimStatus,
    };

    #[test]
    fn pre_closeout_universality_audit_keeps_terminal_blockers_explicit() {
        let report = build_tassadar_pre_closeout_universality_audit_report().expect("report");

        assert_eq!(
            report.claim_status,
            TassadarPreCloseoutUniversalityClaimStatus::Suppressed
        );
        assert!(report
            .remaining_terminal_contract_ids
            .contains(&String::from("tcm_v1_declared_substrate_model")));
        assert!(report.current_true_scopes.contains(&String::from(
            "disclosure-safe general internal-compute red-team audit"
        )));
    }

    #[test]
    fn pre_closeout_universality_audit_matches_committed_truth() {
        let generated = build_tassadar_pre_closeout_universality_audit_report().expect("report");
        let committed: TassadarPreCloseoutUniversalityAuditReport =
            read_json(tassadar_pre_closeout_universality_audit_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
