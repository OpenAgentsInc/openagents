use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_ir::{TassadarFrozenCoreWasmWindow, tassadar_frozen_core_wasm_window_v1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SEMANTIC_WINDOW_REVISION_RECEIPT_REF: &str =
    "fixtures/tassadar/reports/tassadar_semantic_window_revision_receipt.json";

pub const TASSADAR_METADATA_REFRESH_WINDOW_ID: &str =
    "tassadar.frozen_core_wasm.window.v1_1.metadata";
pub const TASSADAR_PUBLIC_PROPOSAL_LIFT_WINDOW_ID: &str =
    "tassadar.frozen_core_wasm.window.v1_plus_public_proposals";
pub const TASSADAR_OPERATOR_PROPOSAL_LIFT_WINDOW_ID: &str =
    "tassadar.frozen_core_wasm.window.v1_plus_operator_proposals";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSemanticWindowRevisionStatus {
    CompatibleMetadataOnly,
    BlockedProposalBoundary,
    BlockedEvidenceBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDeclaredSemanticWindowStatus {
    ActiveSupported,
    DeclaredCandidateOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowCandidateRevision {
    pub candidate_window_id: String,
    pub base_window_id: String,
    pub revision_kind: String,
    pub added_feature_family_ids: Vec<String>,
    pub removed_feature_family_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub status: TassadarSemanticWindowRevisionStatus,
    pub operator_action: String,
    pub operator_drill_commands: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowRevisionReceipt {
    pub schema_version: u16,
    pub receipt_id: String,
    pub active_window: TassadarFrozenCoreWasmWindow,
    pub declared_window_ids: Vec<String>,
    pub candidate_revisions: Vec<TassadarSemanticWindowCandidateRevision>,
    pub compatible_candidate_window_ids: Vec<String>,
    pub blocked_candidate_window_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarDeclaredSemanticWindowResolution {
    pub window_id: String,
    pub status: TassadarDeclaredSemanticWindowStatus,
}

#[derive(Debug, Error)]
pub enum TassadarSemanticWindowRevisionError {
    #[error(
        "undeclared semantic window `{window_id}`; declared windows are {declared_window_ids:?}"
    )]
    UndeclaredWindowId {
        window_id: String,
        declared_window_ids: Vec<String>,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

#[must_use]
pub fn build_tassadar_semantic_window_revision_receipt() -> TassadarSemanticWindowRevisionReceipt {
    let active_window = tassadar_frozen_core_wasm_window_v1();
    let base_window_id = active_window.window_id.clone();
    let current_host_cpu_ref = String::from("portability.current_host.cpu_reference.v1");
    let mut candidate_revisions = vec![
        TassadarSemanticWindowCandidateRevision {
            candidate_window_id: String::from(TASSADAR_METADATA_REFRESH_WINDOW_ID),
            base_window_id: base_window_id.clone(),
            revision_kind: String::from("metadata_refresh"),
            added_feature_family_ids: Vec::new(),
            removed_feature_family_ids: Vec::new(),
            portability_envelope_ids: vec![current_host_cpu_ref.clone()],
            status: TassadarSemanticWindowRevisionStatus::CompatibleMetadataOnly,
            operator_action: String::from(
                "no semantic lift; rerun the frozen-window and closure-gate drill, then treat the candidate as documentation-only unless every digest still matches",
            ),
            operator_drill_commands: vec![
                String::from(
                    "cargo run -p psionic-runtime --example tassadar_semantic_window_revision_receipt",
                ),
                String::from(
                    "cargo run -p psionic-eval --example tassadar_frozen_core_wasm_window_report",
                ),
                String::from(
                    "cargo run -p psionic-eval --example tassadar_frozen_core_wasm_closure_gate_report",
                ),
            ],
            detail: String::from(
                "candidate keeps the same semantic families and harness authorities as the active window; it only allows a metadata-only digest refresh after rerunning the declared frozen-window drill",
            ),
        },
        TassadarSemanticWindowCandidateRevision {
            candidate_window_id: String::from(TASSADAR_PUBLIC_PROPOSAL_LIFT_WINDOW_ID),
            base_window_id: base_window_id.clone(),
            revision_kind: String::from("proposal_family_lift"),
            added_feature_family_ids: vec![
                String::from("proposal_family.exceptions"),
                String::from("proposal_family.simd"),
            ],
            removed_feature_family_ids: Vec::new(),
            portability_envelope_ids: vec![current_host_cpu_ref.clone()],
            status: TassadarSemanticWindowRevisionStatus::BlockedProposalBoundary,
            operator_action: String::from(
                "keep the frozen core-Wasm window unchanged and publish exceptions or SIMD only as separate named proposal profiles",
            ),
            operator_drill_commands: vec![
                String::from(
                    "cargo run -p psionic-eval --example tassadar_proposal_profile_ladder_claim_checker_report",
                ),
                String::from(
                    "cargo run -p psionic-router --example tassadar_proposal_profile_route_policy_report",
                ),
            ],
            detail: String::from(
                "exceptions and SIMD already have named-profile evidence, but proposal families remain a separate claim class and may not be folded into the frozen core window by silent revision",
            ),
        },
        TassadarSemanticWindowCandidateRevision {
            candidate_window_id: String::from(TASSADAR_OPERATOR_PROPOSAL_LIFT_WINDOW_ID),
            base_window_id,
            revision_kind: String::from("operator_only_widening"),
            added_feature_family_ids: vec![
                String::from("proposal_family.component_linking"),
                String::from("proposal_family.memory64"),
                String::from("proposal_family.multi_memory"),
            ],
            removed_feature_family_ids: Vec::new(),
            portability_envelope_ids: vec![current_host_cpu_ref],
            status: TassadarSemanticWindowRevisionStatus::BlockedEvidenceBoundary,
            operator_action: String::from(
                "keep the active frozen window and refuse the widening unless each family graduates through its own operator-only or public profile ladder first",
            ),
            operator_drill_commands: vec![
                String::from(
                    "cargo run -p psionic-eval --example tassadar_proposal_profile_ladder_claim_checker_report",
                ),
                String::from(
                    "cargo run -p psionic-eval --example tassadar_broad_general_compute_validator_bridge_report",
                ),
            ],
            detail: String::from(
                "memory64, multi-memory, and component-linking still require their own evidence and publication posture; revising the frozen window directly would silently widen portability and served claims",
            ),
        },
    ];
    candidate_revisions
        .sort_by(|left, right| left.candidate_window_id.cmp(&right.candidate_window_id));

    let mut declared_window_ids = vec![active_window.window_id.clone()];
    declared_window_ids.extend(
        candidate_revisions
            .iter()
            .map(|candidate| candidate.candidate_window_id.clone()),
    );
    declared_window_ids.sort();
    declared_window_ids.dedup();

    let compatible_candidate_window_ids = candidate_revisions
        .iter()
        .filter(|candidate| {
            candidate.status == TassadarSemanticWindowRevisionStatus::CompatibleMetadataOnly
        })
        .map(|candidate| candidate.candidate_window_id.clone())
        .collect::<Vec<_>>();
    let blocked_candidate_window_ids = candidate_revisions
        .iter()
        .filter(|candidate| {
            candidate.status != TassadarSemanticWindowRevisionStatus::CompatibleMetadataOnly
        })
        .map(|candidate| candidate.candidate_window_id.clone())
        .collect::<Vec<_>>();

    let mut receipt = TassadarSemanticWindowRevisionReceipt {
        schema_version: 1,
        receipt_id: String::from("tassadar.semantic_window_revision.receipt.v1"),
        active_window,
        declared_window_ids,
        candidate_revisions,
        compatible_candidate_window_ids,
        blocked_candidate_window_ids,
        claim_boundary: String::from(
            "this receipt declares how Tassadar may revise one frozen core-Wasm semantic window without silent drift. It distinguishes metadata-only refresh from blocked proposal-family lift and blocked evidence-bound widening. It does not activate a new window, imply full core-Wasm closure, or widen served posture by itself",
        ),
        summary: String::new(),
        receipt_digest: String::new(),
    };
    receipt.summary = format!(
        "Semantic-window revision receipt keeps active_window_id={}, compatible_candidates={}, blocked_candidates={}, declared_windows={}.",
        receipt.active_window.window_id,
        receipt.compatible_candidate_window_ids.len(),
        receipt.blocked_candidate_window_ids.len(),
        receipt.declared_window_ids.len(),
    );
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_semantic_window_revision_receipt|",
        &receipt,
    );
    receipt
}

pub fn resolve_declared_tassadar_semantic_window(
    window_id: &str,
) -> Result<TassadarDeclaredSemanticWindowResolution, TassadarSemanticWindowRevisionError> {
    let receipt = build_tassadar_semantic_window_revision_receipt();
    if window_id == receipt.active_window.window_id {
        return Ok(TassadarDeclaredSemanticWindowResolution {
            window_id: String::from(window_id),
            status: TassadarDeclaredSemanticWindowStatus::ActiveSupported,
        });
    }
    if receipt
        .candidate_revisions
        .iter()
        .any(|candidate| candidate.candidate_window_id == window_id)
    {
        return Ok(TassadarDeclaredSemanticWindowResolution {
            window_id: String::from(window_id),
            status: TassadarDeclaredSemanticWindowStatus::DeclaredCandidateOnly,
        });
    }
    Err(TassadarSemanticWindowRevisionError::UndeclaredWindowId {
        window_id: String::from(window_id),
        declared_window_ids: receipt.declared_window_ids,
    })
}

#[must_use]
pub fn tassadar_semantic_window_revision_receipt_path() -> PathBuf {
    repo_root().join(TASSADAR_SEMANTIC_WINDOW_REVISION_RECEIPT_REF)
}

pub fn write_tassadar_semantic_window_revision_receipt(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSemanticWindowRevisionReceipt, TassadarSemanticWindowRevisionError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSemanticWindowRevisionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let receipt = build_tassadar_semantic_window_revision_receipt();
    let json = serde_json::to_string_pretty(&receipt)
        .expect("semantic window revision receipt serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSemanticWindowRevisionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(receipt)
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
mod tests {
    use super::{
        TASSADAR_METADATA_REFRESH_WINDOW_ID, TassadarDeclaredSemanticWindowStatus,
        TassadarSemanticWindowRevisionReceipt, build_tassadar_semantic_window_revision_receipt,
        resolve_declared_tassadar_semantic_window, write_tassadar_semantic_window_revision_receipt,
    };

    #[test]
    fn semantic_window_revision_receipt_is_machine_legible() {
        let receipt = build_tassadar_semantic_window_revision_receipt();
        assert_eq!(
            receipt.active_window.window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert!(
            receipt
                .compatible_candidate_window_ids
                .contains(&String::from(TASSADAR_METADATA_REFRESH_WINDOW_ID))
        );
        assert_eq!(receipt.blocked_candidate_window_ids.len(), 2);
        assert_eq!(receipt.declared_window_ids.len(), 4);
        assert!(!receipt.receipt_digest.is_empty());
    }

    #[test]
    fn semantic_window_revision_resolution_refuses_undeclared_windows() {
        let active =
            resolve_declared_tassadar_semantic_window("tassadar.frozen_core_wasm.window.v1")
                .expect("active window should resolve");
        assert_eq!(
            active.status,
            TassadarDeclaredSemanticWindowStatus::ActiveSupported
        );

        let candidate =
            resolve_declared_tassadar_semantic_window(TASSADAR_METADATA_REFRESH_WINDOW_ID)
                .expect("declared candidate should resolve");
        assert_eq!(
            candidate.status,
            TassadarDeclaredSemanticWindowStatus::DeclaredCandidateOnly
        );

        let error = resolve_declared_tassadar_semantic_window("tassadar.window.unknown.v1")
            .expect_err("undeclared window should refuse");
        assert!(
            error
                .to_string()
                .contains("undeclared semantic window `tassadar.window.unknown.v1`")
        );
    }

    #[test]
    fn write_semantic_window_revision_receipt_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_semantic_window_revision_receipt.json");
        let receipt = write_tassadar_semantic_window_revision_receipt(&output_path)
            .expect("receipt should write");
        let bytes = std::fs::read(&output_path).expect("persisted receipt should exist");
        let persisted: TassadarSemanticWindowRevisionReceipt =
            serde_json::from_slice(&bytes).expect("persisted receipt should decode");
        assert_eq!(persisted, receipt);
        std::fs::remove_file(&output_path).expect("temp receipt should be removable");
    }
}
