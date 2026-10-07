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
    TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF, TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
    TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF,
    TassadarFrozenCoreWasmClosureGateReport, build_tassadar_frozen_core_wasm_closure_gate_report,
    build_tassadar_frozen_core_wasm_window_report,
    build_tassadar_proposal_profile_ladder_claim_checker_report,
};
use psionic_runtime::{
    TASSADAR_SEMANTIC_WINDOW_REVISION_RECEIPT_REF, TassadarFrozenCoreWasmClosureGateStatus,
    TassadarSemanticWindowCandidateRevision, TassadarSemanticWindowRevisionStatus,
    build_tassadar_semantic_window_revision_receipt,
};

pub const TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_semantic_window_compatibility_delta_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSemanticWindowCompatibilityStatus {
    CompatibleMetadataOnly,
    BlockedProposalBoundary,
    BlockedEvidenceBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowCompatibilityDeltaRow {
    pub candidate_window_id: String,
    pub revision_kind: String,
    pub compatibility_status: TassadarSemanticWindowCompatibilityStatus,
    pub added_feature_family_ids: Vec<String>,
    pub active_runtime_support_allowed: bool,
    pub served_publication_allowed: bool,
    pub blocking_report_refs: Vec<String>,
    pub operator_action: String,
    pub operator_drill_commands: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowCompatibilityDeltaReport {
    pub schema_version: u16,
    pub report_id: String,
    pub active_window_id: String,
    pub active_window_report_ref: String,
    pub active_window_report_id: String,
    pub closure_gate_report_ref: String,
    pub closure_gate_report_id: String,
    pub revision_receipt_ref: String,
    pub revision_receipt_id: String,
    pub rows: Vec<TassadarSemanticWindowCompatibilityDeltaRow>,
    pub compatible_candidate_window_ids: Vec<String>,
    pub blocked_candidate_window_ids: Vec<String>,
    pub operator_drill_commands: Vec<String>,
    pub served_publication_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSemanticWindowCompatibilityDeltaReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[cfg(test)]
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[cfg(test)]
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_semantic_window_compatibility_delta_report() -> Result<
    TassadarSemanticWindowCompatibilityDeltaReport,
    TassadarSemanticWindowCompatibilityDeltaReportError,
> {
    let active_window_report = build_tassadar_frozen_core_wasm_window_report()
        .expect("frozen core-Wasm window report should build");
    let closure_gate_report = build_tassadar_frozen_core_wasm_closure_gate_report()
        .expect("frozen core-Wasm closure gate should build");
    let proposal_profile_report = build_tassadar_proposal_profile_ladder_claim_checker_report()
        .expect("proposal profile claim checker should build");
    let revision_receipt = build_tassadar_semantic_window_revision_receipt();

    let mut rows = revision_receipt
        .candidate_revisions
        .iter()
        .map(|candidate| compatibility_row(candidate, &closure_gate_report))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.candidate_window_id.cmp(&right.candidate_window_id));

    let compatible_candidate_window_ids = rows
        .iter()
        .filter(|row| {
            row.compatibility_status
                == TassadarSemanticWindowCompatibilityStatus::CompatibleMetadataOnly
        })
        .map(|row| row.candidate_window_id.clone())
        .collect::<Vec<_>>();
    let blocked_candidate_window_ids = rows
        .iter()
        .filter(|row| {
            row.compatibility_status
                != TassadarSemanticWindowCompatibilityStatus::CompatibleMetadataOnly
        })
        .map(|row| row.candidate_window_id.clone())
        .collect::<Vec<_>>();
    let mut operator_drill_commands = vec![
        String::from(
            "cargo run -p psionic-runtime --example tassadar_semantic_window_revision_receipt",
        ),
        String::from(
            "cargo run -p psionic-eval --example tassadar_semantic_window_compatibility_delta_report",
        ),
        String::from("cargo run -p psionic-eval --example tassadar_frozen_core_wasm_window_report"),
        String::from(
            "cargo run -p psionic-eval --example tassadar_frozen_core_wasm_closure_gate_report",
        ),
        String::from(
            "cargo run -p psionic-eval --example tassadar_proposal_profile_ladder_claim_checker_report",
        ),
    ];
    operator_drill_commands.sort();
    operator_drill_commands.dedup();

    let served_publication_allowed = compatible_candidate_window_ids.iter().any(|window_id| {
        window_id.ends_with(".metadata")
            && closure_gate_report.closure_status == TassadarFrozenCoreWasmClosureGateStatus::Closed
    });

    let mut report = TassadarSemanticWindowCompatibilityDeltaReport {
        schema_version: 1,
        report_id: String::from("tassadar.semantic_window_compatibility_delta.report.v1"),
        active_window_id: revision_receipt.active_window.window_id.clone(),
        active_window_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF),
        active_window_report_id: active_window_report.report_id,
        closure_gate_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF),
        closure_gate_report_id: closure_gate_report.report_id,
        revision_receipt_ref: String::from(TASSADAR_SEMANTIC_WINDOW_REVISION_RECEIPT_REF),
        revision_receipt_id: revision_receipt.receipt_id,
        rows,
        compatible_candidate_window_ids,
        blocked_candidate_window_ids,
        operator_drill_commands,
        served_publication_allowed,
        claim_boundary: String::from(
            "this report turns semantic-window revision into a machine-readable compatibility delta. Metadata-only refresh is compatible but still not auto-published while the frozen-window closure gate stays red. Proposal-family lift and operator-only widening remain blocked, because proposal profiles and portability/publication evidence must stay separate instead of silently becoming frozen-window support",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Semantic-window compatibility delta keeps active_window_id={}, compatible_candidates={}, blocked_candidates={}, closure_status={:?}, proposal_public_profiles={}, served_publication_allowed={}.",
        report.active_window_id,
        report.compatible_candidate_window_ids.len(),
        report.blocked_candidate_window_ids.len(),
        closure_gate_report.closure_status,
        proposal_profile_report.public_profile_ids.len(),
        report.served_publication_allowed,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_semantic_window_compatibility_delta_report|",
        &report,
    );
    Ok(report)
}

fn compatibility_row(
    candidate: &TassadarSemanticWindowCandidateRevision,
    closure_gate_report: &TassadarFrozenCoreWasmClosureGateReport,
) -> TassadarSemanticWindowCompatibilityDeltaRow {
    let (compatibility_status, blocking_report_refs, active_runtime_support_allowed) =
        match candidate.status {
            TassadarSemanticWindowRevisionStatus::CompatibleMetadataOnly => (
                TassadarSemanticWindowCompatibilityStatus::CompatibleMetadataOnly,
                vec![String::from(
                    TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
                )],
                false,
            ),
            TassadarSemanticWindowRevisionStatus::BlockedProposalBoundary => (
                TassadarSemanticWindowCompatibilityStatus::BlockedProposalBoundary,
                vec![String::from(
                    TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF,
                )],
                false,
            ),
            TassadarSemanticWindowRevisionStatus::BlockedEvidenceBoundary => (
                TassadarSemanticWindowCompatibilityStatus::BlockedEvidenceBoundary,
                vec![
                    String::from(TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF),
                    String::from(
                        "fixtures/tassadar/reports/tassadar_broad_general_compute_validator_bridge_report.json",
                    ),
                ],
                false,
            ),
        };
    TassadarSemanticWindowCompatibilityDeltaRow {
        candidate_window_id: candidate.candidate_window_id.clone(),
        revision_kind: candidate.revision_kind.clone(),
        compatibility_status,
        added_feature_family_ids: candidate.added_feature_family_ids.clone(),
        active_runtime_support_allowed,
        served_publication_allowed: active_runtime_support_allowed
            && closure_gate_report.closure_status
                == TassadarFrozenCoreWasmClosureGateStatus::Closed,
        blocking_report_refs,
        operator_action: candidate.operator_action.clone(),
        operator_drill_commands: candidate.operator_drill_commands.clone(),
        detail: candidate.detail.clone(),
    }
}

#[must_use]
pub fn tassadar_semantic_window_compatibility_delta_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF)
}

pub fn write_tassadar_semantic_window_compatibility_delta_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSemanticWindowCompatibilityDeltaReport,
    TassadarSemanticWindowCompatibilityDeltaReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSemanticWindowCompatibilityDeltaReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_semantic_window_compatibility_delta_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSemanticWindowCompatibilityDeltaReportError::Write {
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
) -> Result<T, TassadarSemanticWindowCompatibilityDeltaReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSemanticWindowCompatibilityDeltaReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSemanticWindowCompatibilityDeltaReportError::Deserialize {
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
        TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF,
        TassadarSemanticWindowCompatibilityDeltaReport, TassadarSemanticWindowCompatibilityStatus,
        build_tassadar_semantic_window_compatibility_delta_report, read_json,
        tassadar_semantic_window_compatibility_delta_report_path,
        write_tassadar_semantic_window_compatibility_delta_report,
    };

    #[test]
    fn semantic_window_compatibility_delta_keeps_metadata_and_widening_separate() {
        let report = build_tassadar_semantic_window_compatibility_delta_report().expect("report");

        assert_eq!(
            report.active_window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert!(
            report
                .compatible_candidate_window_ids
                .contains(&String::from(
                    "tassadar.frozen_core_wasm.window.v1_1.metadata"
                ))
        );
        assert!(report.blocked_candidate_window_ids.contains(&String::from(
            "tassadar.frozen_core_wasm.window.v1_plus_public_proposals"
        )));
        assert!(report.blocked_candidate_window_ids.contains(&String::from(
            "tassadar.frozen_core_wasm.window.v1_plus_operator_proposals"
        )));
        assert!(!report.served_publication_allowed);
        assert!(report.rows.iter().any(|row| {
            row.candidate_window_id == "tassadar.frozen_core_wasm.window.v1_plus_public_proposals"
                && row.compatibility_status
                    == TassadarSemanticWindowCompatibilityStatus::BlockedProposalBoundary
        }));
    }

    #[test]
    fn semantic_window_compatibility_delta_matches_committed_truth() {
        let generated =
            build_tassadar_semantic_window_compatibility_delta_report().expect("report");
        let committed: TassadarSemanticWindowCompatibilityDeltaReport =
            read_json(tassadar_semantic_window_compatibility_delta_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_semantic_window_compatibility_delta_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_semantic_window_compatibility_delta_report.json");
        let report = write_tassadar_semantic_window_compatibility_delta_report(&output_path)
            .expect("report should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: TassadarSemanticWindowCompatibilityDeltaReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
        assert_eq!(
            tassadar_semantic_window_compatibility_delta_report_path()
                .ends_with(TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF),
            true
        );
    }
}
