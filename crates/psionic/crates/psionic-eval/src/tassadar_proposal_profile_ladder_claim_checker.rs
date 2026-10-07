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
    build_tassadar_component_linking_profile_report, build_tassadar_exception_profile_report,
    build_tassadar_memory64_profile_report, build_tassadar_multi_memory_profile_report,
    build_tassadar_simd_profile_report, build_tassadar_threads_research_profile_report,
    TassadarComponentLinkingProfileReportError, TassadarExceptionProfileReport,
    TassadarMemory64ProfileReportError, TassadarMultiMemoryProfileReportError,
    TassadarSimdProfileReportError, TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF,
    TASSADAR_EXCEPTION_PROFILE_REPORT_REF, TASSADAR_MEMORY64_PROFILE_REPORT_REF,
    TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF, TASSADAR_SIMD_PROFILE_REPORT_REF,
    TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF,
};

pub const TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_proposal_profile_ladder_claim_checker_report.json";

const TASSADAR_MULTI_MEMORY_PROFILE_ID: &str = "tassadar.proposal_profile.multi_memory_routing.v1";
const TASSADAR_COMPONENT_LINKING_PROFILE_ID: &str =
    "tassadar.proposal_profile.component_linking_interface_types.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProposalProfileClaimStatus {
    PublishedNamedProfile,
    SuppressedOperatorOnly,
    SuppressedResearchOnly,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfileClaimRow {
    pub profile_id: String,
    pub profile_family_id: String,
    pub source_report_ref: String,
    pub parity_ready: bool,
    pub portability_ready: bool,
    pub refusal_ready: bool,
    pub named_public_profile_allowed: bool,
    pub default_served_profile_allowed: bool,
    pub claim_status: TassadarProposalProfileClaimStatus,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfileLadderClaimCheckerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub rows: Vec<TassadarProposalProfileClaimRow>,
    pub public_profile_ids: Vec<String>,
    pub served_publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub research_only_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub default_served_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarProposalProfileLadderClaimCheckerReportError {
    #[error(transparent)]
    Memory64(#[from] TassadarMemory64ProfileReportError),
    #[error(transparent)]
    MultiMemory(#[from] TassadarMultiMemoryProfileReportError),
    #[error(transparent)]
    ComponentLinking(#[from] TassadarComponentLinkingProfileReportError),
    #[error(transparent)]
    Simd(#[from] TassadarSimdProfileReportError),
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

pub fn build_tassadar_proposal_profile_ladder_claim_checker_report() -> Result<
    TassadarProposalProfileLadderClaimCheckerReport,
    TassadarProposalProfileLadderClaimCheckerReportError,
> {
    let exception_report = build_tassadar_exception_profile_report();
    let memory64_report = build_tassadar_memory64_profile_report()?;
    let multi_memory_report = build_tassadar_multi_memory_profile_report()?;
    let component_linking_report = build_tassadar_component_linking_profile_report()?;
    let simd_report = build_tassadar_simd_profile_report()?;
    let threads_report = build_tassadar_threads_research_profile_report();

    let mut rows = vec![
        exception_row(&exception_report),
        memory64_row(&memory64_report),
        multi_memory_row(&multi_memory_report),
        component_linking_row(&component_linking_report),
        simd_row(&simd_report),
        threads_row(&threads_report),
    ];
    rows.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));

    let public_profile_ids = rows
        .iter()
        .filter(|row| row.claim_status == TassadarProposalProfileClaimStatus::PublishedNamedProfile)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let served_publication_allowed_profile_ids = rows
        .iter()
        .filter(|row| row.named_public_profile_allowed && !row.default_served_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let suppressed_profile_ids = rows
        .iter()
        .filter(|row| {
            row.claim_status == TassadarProposalProfileClaimStatus::SuppressedOperatorOnly
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let research_only_profile_ids = rows
        .iter()
        .filter(|row| {
            row.claim_status == TassadarProposalProfileClaimStatus::SuppressedResearchOnly
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let failed_profile_ids = rows
        .iter()
        .filter(|row| row.claim_status == TassadarProposalProfileClaimStatus::Failed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let default_served_profile_ids = rows
        .iter()
        .filter(|row| row.default_served_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let overall_green = failed_profile_ids.is_empty()
        && default_served_profile_ids.is_empty()
        && public_profile_ids.len() == 2
        && public_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1",
        ))
        && public_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.simd_deterministic.v1",
        ));

    let mut report = TassadarProposalProfileLadderClaimCheckerReport {
        schema_version: 1,
        report_id: String::from("tassadar.proposal_profile_ladder_claim_checker.report.v1"),
        rows,
        public_profile_ids,
        served_publication_allowed_profile_ids,
        suppressed_profile_ids,
        research_only_profile_ids,
        failed_profile_ids,
        default_served_profile_ids,
        overall_green,
        claim_boundary: String::from(
            "this claim checker is the disclosure-safe ladder for post-core proposal families. Every profile family must earn its own parity, portability, refusal, and publication posture. Exceptions and SIMD may currently be named publicly; memory64, multi-memory, and component-linking stay suppressed operator-only; threads stays research-only. Nothing here implies arbitrary Wasm, broad internal compute, or implicit proposal inheritance.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Proposal-profile ladder claim checker now records public_profiles={}, suppressed_profiles={}, research_only_profiles={}, failed_profiles={}, default_served_profiles={}, overall_green={}.",
        report.public_profile_ids.len(),
        report.suppressed_profile_ids.len(),
        report.research_only_profile_ids.len(),
        report.failed_profile_ids.len(),
        report.default_served_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_proposal_profile_ladder_claim_checker_report|",
        &report,
    );
    Ok(report)
}

fn exception_row(report: &TassadarExceptionProfileReport) -> TassadarProposalProfileClaimRow {
    let named_public_profile_allowed = !report.public_profile_allowed_profile_ids.is_empty()
        && report.default_served_profile_allowed_profile_ids.is_empty()
        && report.overall_green;
    let claim_status = if named_public_profile_allowed {
        TassadarProposalProfileClaimStatus::PublishedNamedProfile
    } else {
        TassadarProposalProfileClaimStatus::Failed
    };
    TassadarProposalProfileClaimRow {
        profile_id: String::from("tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"),
        profile_family_id: String::from("proposal_family.exceptions"),
        source_report_ref: String::from(TASSADAR_EXCEPTION_PROFILE_REPORT_REF),
        parity_ready: report.exact_trap_stack_parity_case_count > 0 && report.overall_green,
        portability_ready: !report.portability_envelope_ids.is_empty(),
        refusal_ready: report.exact_refusal_parity_case_count > 0,
        named_public_profile_allowed,
        default_served_profile_allowed: !report.default_served_profile_allowed_profile_ids.is_empty(),
        claim_status,
        detail: String::from(
            "exceptions stays claim-safe because throw/catch/rethrow parity, trap-stack parity, refusal truth, and current-host portability are explicit while the default served lane remains empty",
        ),
    }
}

fn memory64_row(report: &crate::TassadarMemory64ProfileReport) -> TassadarProposalProfileClaimRow {
    let parity_ready =
        report.exact_resume_parity_count > 0 && report.exact_large_address_parity_count > 0;
    let portability_ready = !report.portability_envelope_id.trim().is_empty();
    let refusal_ready = report.exact_refusal_parity_count > 0;
    TassadarProposalProfileClaimRow {
        profile_id: report.profile_id.clone(),
        profile_family_id: String::from("proposal_family.memory64"),
        source_report_ref: String::from(TASSADAR_MEMORY64_PROFILE_REPORT_REF),
        parity_ready,
        portability_ready,
        refusal_ready,
        named_public_profile_allowed: false,
        default_served_profile_allowed: false,
        claim_status: if parity_ready && portability_ready && refusal_ready {
            TassadarProposalProfileClaimStatus::SuppressedOperatorOnly
        } else {
            TassadarProposalProfileClaimStatus::Failed
        },
        detail: String::from(
            "memory64 stays suppressed operator-only because checkpoint and portability truth are real but there is no named served-publication posture for the profile family",
        ),
    }
}

fn multi_memory_row(
    report: &crate::TassadarMultiMemoryProfileReport,
) -> TassadarProposalProfileClaimRow {
    let parity_ready = report.overall_green
        && report.exact_routing_parity_count > 0
        && report.exact_resume_parity_count > 0;
    let portability_ready = !report.portability_envelope_ids.is_empty();
    let refusal_ready = report.exact_refusal_parity_count > 0;
    TassadarProposalProfileClaimRow {
        profile_id: String::from(TASSADAR_MULTI_MEMORY_PROFILE_ID),
        profile_family_id: String::from("proposal_family.multi_memory"),
        source_report_ref: String::from(TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF),
        parity_ready,
        portability_ready,
        refusal_ready,
        named_public_profile_allowed: false,
        default_served_profile_allowed: false,
        claim_status: if parity_ready && portability_ready && refusal_ready {
            TassadarProposalProfileClaimStatus::SuppressedOperatorOnly
        } else {
            TassadarProposalProfileClaimStatus::Failed
        },
        detail: String::from(
            "multi-memory stays suppressed operator-only because exact topology and checkpoint truth are explicit but no public served-profile posture has been earned for the family",
        ),
    }
}

fn component_linking_row(
    report: &crate::TassadarComponentLinkingProfileReport,
) -> TassadarProposalProfileClaimRow {
    let parity_ready = report.overall_green && report.exact_component_parity_count > 0;
    let portability_ready = !report.portability_envelope_ids.is_empty();
    let refusal_ready = report.exact_refusal_parity_count > 0;
    TassadarProposalProfileClaimRow {
        profile_id: String::from(TASSADAR_COMPONENT_LINKING_PROFILE_ID),
        profile_family_id: String::from("proposal_family.component_linking"),
        source_report_ref: String::from(TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF),
        parity_ready,
        portability_ready,
        refusal_ready,
        named_public_profile_allowed: false,
        default_served_profile_allowed: false,
        claim_status: if parity_ready && portability_ready && refusal_ready {
            TassadarProposalProfileClaimStatus::SuppressedOperatorOnly
        } else {
            TassadarProposalProfileClaimStatus::Failed
        },
        detail: String::from(
            "component-linking stays suppressed operator-only because interface-type lowering and refusal truth are explicit but the family still lacks named served-publication posture",
        ),
    }
}

fn simd_row(report: &crate::TassadarSimdProfileReport) -> TassadarProposalProfileClaimRow {
    let named_public_profile_allowed = !report.public_profile_allowed_profile_ids.is_empty()
        && report.default_served_profile_allowed_profile_ids.is_empty()
        && report.overall_green;
    let claim_status = if named_public_profile_allowed {
        TassadarProposalProfileClaimStatus::PublishedNamedProfile
    } else {
        TassadarProposalProfileClaimStatus::Failed
    };
    TassadarProposalProfileClaimRow {
        profile_id: String::from("tassadar.proposal_profile.simd_deterministic.v1"),
        profile_family_id: String::from("proposal_family.simd"),
        source_report_ref: String::from(TASSADAR_SIMD_PROFILE_REPORT_REF),
        parity_ready: report.overall_green && !report.exact_backend_ids.is_empty(),
        portability_ready: !report.exact_backend_ids.is_empty() && !report.fallback_backend_ids.is_empty(),
        refusal_ready: !report.refused_backend_ids.is_empty(),
        named_public_profile_allowed,
        default_served_profile_allowed: !report.default_served_profile_allowed_profile_ids.is_empty(),
        claim_status,
        detail: String::from(
            "SIMD stays claim-safe because the cpu-reference exact row, accelerator fallback rows, and refusal rows are explicit while the default served SIMD lane remains empty",
        ),
    }
}

fn threads_row(
    report: &crate::TassadarThreadsResearchProfileEvalReport,
) -> TassadarProposalProfileClaimRow {
    let parity_ready = report.overall_green && report.exact_case_count > 0;
    let portability_ready = !report.green_scheduler_ids.is_empty();
    let refusal_ready = report.refusal_case_count > 0 && !report.refused_scheduler_ids.is_empty();
    TassadarProposalProfileClaimRow {
        profile_id: report.profile_id.clone(),
        profile_family_id: String::from("proposal_family.threads"),
        source_report_ref: String::from(TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF),
        parity_ready,
        portability_ready,
        refusal_ready,
        named_public_profile_allowed: false,
        default_served_profile_allowed: false,
        claim_status: if parity_ready && portability_ready && refusal_ready {
            TassadarProposalProfileClaimStatus::SuppressedResearchOnly
        } else {
            TassadarProposalProfileClaimStatus::Failed
        },
        detail: String::from(
            "threads stays research-only because the deterministic scheduler envelope is explicit but the family deliberately has no served-publication posture",
        ),
    }
}

#[must_use]
pub fn tassadar_proposal_profile_ladder_claim_checker_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF)
}

pub fn write_tassadar_proposal_profile_ladder_claim_checker_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarProposalProfileLadderClaimCheckerReport,
    TassadarProposalProfileLadderClaimCheckerReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarProposalProfileLadderClaimCheckerReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_proposal_profile_ladder_claim_checker_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarProposalProfileLadderClaimCheckerReportError::Write {
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarProposalProfileLadderClaimCheckerReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarProposalProfileLadderClaimCheckerReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarProposalProfileLadderClaimCheckerReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
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
    use super::{
        build_tassadar_proposal_profile_ladder_claim_checker_report, read_json,
        tassadar_proposal_profile_ladder_claim_checker_report_path,
        TassadarProposalProfileClaimStatus, TassadarProposalProfileLadderClaimCheckerReport,
    };

    #[test]
    fn proposal_profile_ladder_claim_checker_keeps_public_and_suppressed_rows_separate() {
        let report = build_tassadar_proposal_profile_ladder_claim_checker_report().expect("report");

        assert!(report.overall_green);
        assert!(report.public_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"
        )));
        assert!(report.public_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.simd_deterministic.v1"
        )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.memory64_continuation.v1"
        )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.multi_memory_routing.v1"
        )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.component_linking_interface_types.v1"
        )));
        assert!(report.research_only_profile_ids.contains(&String::from(
            "tassadar.research_profile.threads_deterministic_scheduler.v1"
        )));
        assert!(report.default_served_profile_ids.is_empty());
        assert!(report.rows.iter().all(|row| {
            row.profile_id != "tassadar.proposal_profile.memory64_continuation.v1"
                || row.claim_status == TassadarProposalProfileClaimStatus::SuppressedOperatorOnly
        }));
    }

    #[test]
    fn proposal_profile_ladder_claim_checker_matches_committed_truth() {
        let generated =
            build_tassadar_proposal_profile_ladder_claim_checker_report().expect("report");
        let committed: TassadarProposalProfileLadderClaimCheckerReport =
            read_json(tassadar_proposal_profile_ladder_claim_checker_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
