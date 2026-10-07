use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;
use psionic_runtime::{
    build_tassadar_approximate_attention_closure_runtime_report,
    TassadarApproximateAttentionClosureReceipt, TassadarApproximateAttentionFamily,
    TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF,
};

const TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_CLAIM_CLASS: &str =
    "research_only_fast_path_substrate";
pub const TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json";
pub const TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_approximate_attention_closure_summary.json";

/// Repo-facing publication status for the approximate-attention closure lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarApproximateAttentionClosurePublicationStatus {
    /// Landed as a repo-backed public analysis surface.
    Implemented,
}

/// Public publication for the approximate-attention closure lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionClosurePublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarApproximateAttentionClosurePublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Ordered attention families covered today.
    pub attention_families: Vec<TassadarApproximateAttentionFamily>,
    /// Ordered workload targets covered today.
    pub workload_targets: Vec<String>,
    /// Ordered runtime closure receipts.
    pub closure_receipts: Vec<TassadarApproximateAttentionClosureReceipt>,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarApproximateAttentionClosurePublication {
    fn new() -> Self {
        let runtime_report = build_tassadar_approximate_attention_closure_runtime_report()
            .expect("runtime closure report");
        let workload_targets = runtime_report
            .closure_receipts
            .iter()
            .map(|receipt| receipt.workload_target.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut publication = Self {
            schema_version: TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from(
                "tassadar.approximate_attention_closure.publication.v1",
            ),
            status: TassadarApproximateAttentionClosurePublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-approximate-attention-closure-v0",
                "tassadar_approximate_attention_closure",
                "v0",
            ),
            attention_families: vec![
                TassadarApproximateAttentionFamily::DenseReferenceLinear,
                TassadarApproximateAttentionFamily::SparseTopKValidated,
                TassadarApproximateAttentionFamily::LinearRecurrentRuntime,
                TassadarApproximateAttentionFamily::LshBucketedProxy,
                TassadarApproximateAttentionFamily::HardMaxRoutingProxy,
                TassadarApproximateAttentionFamily::HullCacheRuntime,
                TassadarApproximateAttentionFamily::HierarchicalHullResearch,
            ],
            workload_targets,
            closure_receipts: runtime_report.closure_receipts,
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF),
                String::from(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF),
                String::from(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the publication keeps direct, degraded_but_bounded, and refused closure posture explicit for each workload and attention family instead of compressing them into one blended score",
                ),
                String::from(
                    "proxy rows such as lsh_bucketed_proxy and hard_max_routing_proxy are bounded analytical surfaces, not promoted runtime capabilities",
                ),
                String::from(
                    "the lane reuses the current bounded workload matrix and efficient-attention artifacts; it does not claim general approximate-attention executor closure or served promotion",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_approximate_attention_closure_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the approximate-attention closure lane.
#[must_use]
pub fn tassadar_approximate_attention_closure_publication()
-> TassadarApproximateAttentionClosurePublication {
    TassadarApproximateAttentionClosurePublication::new()
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
        tassadar_approximate_attention_closure_publication,
        TassadarApproximateAttentionClosurePublicationStatus,
    };
    use psionic_runtime::TassadarApproximateAttentionFamily;

    #[test]
    fn approximate_attention_closure_publication_is_machine_legible() {
        let publication = tassadar_approximate_attention_closure_publication();

        assert_eq!(
            publication.status,
            TassadarApproximateAttentionClosurePublicationStatus::Implemented
        );
        assert!(publication
            .attention_families
            .contains(&TassadarApproximateAttentionFamily::HardMaxRoutingProxy));
        assert!(publication
            .workload_targets
            .contains(&String::from("sudoku_class")));
        assert!(!publication.publication_digest.is_empty());
    }
}
