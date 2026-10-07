use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_SHARED_PRIMITIVE_TRANSFER_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/shared_primitive_transfer";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_shared_primitive_transfer_v1/shared_primitive_transfer_evidence_bundle.json";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_report.json";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_summary.json";

/// Repo-facing publication status for the shared primitive transfer lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveTransferPublicationStatus {
    /// The lane exists as an early research substrate.
    ImplementedEarly,
}

/// Public publication for the shared primitive transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarSharedPrimitiveTransferPublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub contract_ref: String,
    pub primitive_ids: Vec<String>,
    pub algorithm_families: Vec<String>,
    pub separated_claim_classes: Vec<String>,
    pub comparison_anchor_refs: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarSharedPrimitiveTransferPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_SHARED_PRIMITIVE_TRANSFER_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.shared_primitive_transfer.publication.v1"),
            status: TassadarSharedPrimitiveTransferPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-shared-primitive-transfer-v0",
                "tassadar_shared_primitive_transfer",
                "v0",
            ),
            contract_ref: String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_CONTRACT_REF),
            primitive_ids: vec![
                String::from("tassadar.primitive.reachability_expand.v1"),
                String::from("tassadar.primitive.relax_state.v1"),
                String::from("tassadar.primitive.compare_candidates.v1"),
                String::from("tassadar.primitive.select_candidate.v1"),
                String::from("tassadar.primitive.merge_state.v1"),
                String::from("tassadar.primitive.bounded_backtrack.v1"),
            ],
            algorithm_families: vec![
                String::from("sort_merge"),
                String::from("clrs_shortest_path"),
                String::from("clrs_wasm_shortest_path"),
                String::from("hungarian_matching"),
                String::from("sudoku_search"),
                String::from("verifier_search_kernel"),
            ],
            separated_claim_classes: vec![
                String::from("compiled_bounded_exactness"),
                String::from("learned_bounded_success"),
                String::from("research_only_architecture"),
            ],
            comparison_anchor_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_learnability_gap_report.json"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-data"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF),
                String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF),
                String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the publication names a research-only shared primitive substrate over declared algorithm families; it does not promote any learned executor lane to served capability",
                ),
                String::from(
                    "compiled anchor evidence and learned transfer evidence remain comparable but separated; a reusable primitive here does not collapse compiled exactness and learned bounded success into one claim",
                ),
                String::from(
                    "the lane is bounded to the published primitive vocabulary and held-out families; it does not imply arbitrary Wasm, arbitrary CLRS, or broad module-scale learned closure",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_shared_primitive_transfer_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical publication for the shared primitive transfer substrate.
#[must_use]
pub fn tassadar_shared_primitive_transfer_publication() -> TassadarSharedPrimitiveTransferPublication
{
    TassadarSharedPrimitiveTransferPublication::new()
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
        tassadar_shared_primitive_transfer_publication,
        TassadarSharedPrimitiveTransferPublicationStatus,
    };

    #[test]
    fn shared_primitive_transfer_publication_is_machine_legible() {
        let publication = tassadar_shared_primitive_transfer_publication();

        assert_eq!(
            publication.status,
            TassadarSharedPrimitiveTransferPublicationStatus::ImplementedEarly
        );
        assert_eq!(
            publication.model.family,
            "tassadar_shared_primitive_transfer"
        );
        assert_eq!(publication.primitive_ids.len(), 6);
        assert!(publication
            .algorithm_families
            .contains(&String::from("clrs_wasm_shortest_path")));
        assert!(publication
            .separated_claim_classes
            .contains(&String::from("compiled_bounded_exactness")));
        assert!(!publication.publication_digest.is_empty());
    }
}
