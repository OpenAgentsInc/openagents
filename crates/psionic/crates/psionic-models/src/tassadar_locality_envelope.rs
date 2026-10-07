use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;
use psionic_runtime::{
    TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF, TassadarLocalityVariantFamily,
    TassadarLocalityWorkloadFamily, build_tassadar_locality_envelope_runtime_report,
};

const TASSADAR_LOCALITY_ENVELOPE_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LOCALITY_ENVELOPE_CLAIM_CLASS: &str = "research_only_fast_path_substrate";
pub const TASSADAR_LOCALITY_ENVELOPE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_locality_envelope_report.json";
pub const TASSADAR_LOCALITY_ENVELOPE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_locality_envelope_summary.json";

/// Repo-facing publication status for the locality-envelope lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityEnvelopePublicationStatus {
    Implemented,
}

/// Public publication for the locality-envelope lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopePublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarLocalityEnvelopePublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub variant_families: Vec<TassadarLocalityVariantFamily>,
    pub workload_families: Vec<TassadarLocalityWorkloadFamily>,
    pub exact_case_count: u32,
    pub degraded_case_count: u32,
    pub refused_case_count: u32,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarLocalityEnvelopePublication {
    fn new() -> Self {
        let runtime_report = build_tassadar_locality_envelope_runtime_report();
        let mut publication = Self {
            schema_version: TASSADAR_LOCALITY_ENVELOPE_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.locality_envelope.publication.v1"),
            status: TassadarLocalityEnvelopePublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_LOCALITY_ENVELOPE_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-locality-envelope-v0",
                "tassadar_locality_envelope",
                "v0",
            ),
            variant_families: vec![
                TassadarLocalityVariantFamily::DenseReferenceLinear,
                TassadarLocalityVariantFamily::SparseTopKValidated,
                TassadarLocalityVariantFamily::LinearAttentionProxy,
                TassadarLocalityVariantFamily::RecurrentStateRuntime,
                TassadarLocalityVariantFamily::LocalityScratchpadized,
            ],
            workload_families: vec![
                TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
                TassadarLocalityWorkloadFamily::ClrsShortestPath,
                TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
                TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            ],
            exact_case_count: runtime_report.exact_case_count,
            degraded_case_count: runtime_report.degraded_case_count,
            refused_case_count: runtime_report.refused_case_count,
            target_surfaces: vec![
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF),
                String::from(TASSADAR_LOCALITY_ENVELOPE_REPORT_REF),
                String::from(TASSADAR_LOCALITY_ENVELOPE_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the publication is a benchmark-bound locality map over seeded workload families and dense, sparse, linear, recurrent, and scratchpadized variants; it does not promote any one variant into a served capability by itself",
                ),
                String::from(
                    "linear_attention_proxy remains an analytical comparison row even where it is cheap; degraded and refused posture must stay explicit instead of being smoothed into one locality score",
                ),
                String::from(
                    "module-scale Wasm and search-heavy workloads still carry downgrade or refusal boundaries; locality wins here do not imply arbitrary Wasm, broad host-import, or broad learned-compute closure",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_locality_envelope_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the locality-envelope lane.
#[must_use]
pub fn tassadar_locality_envelope_publication() -> TassadarLocalityEnvelopePublication {
    TassadarLocalityEnvelopePublication::new()
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
        TassadarLocalityEnvelopePublicationStatus, tassadar_locality_envelope_publication,
    };
    use psionic_runtime::TassadarLocalityVariantFamily;

    #[test]
    fn locality_envelope_publication_is_machine_legible() {
        let publication = tassadar_locality_envelope_publication();

        assert_eq!(
            publication.status,
            TassadarLocalityEnvelopePublicationStatus::Implemented
        );
        assert!(
            publication
                .variant_families
                .contains(&TassadarLocalityVariantFamily::LocalityScratchpadized)
        );
        assert_eq!(publication.refused_case_count, 3);
        assert!(!publication.publication_digest.is_empty());
    }
}
