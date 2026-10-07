use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_RELAXED_SIMD_LADDER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_relaxed_simd_research_ladder_report.json";
pub const TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_relaxed_simd_research_summary.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRelaxedSimdPublicationStatus {
    ImplementedEarly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdResearchPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarRelaxedSimdPublicationStatus,
    pub claim_class: String,
    pub research_profile_id: String,
    pub ladder_stage_ids: Vec<String>,
    pub baseline_refs: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub summary_report_ref: String,
    pub publication_digest: String,
}

impl TassadarRelaxedSimdResearchPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: 1,
            publication_id: String::from("tassadar.relaxed_simd.research_publication.v1"),
            status: TassadarRelaxedSimdPublicationStatus::ImplementedEarly,
            claim_class: String::from("research_only_systems_work_refusal_truth"),
            research_profile_id: String::from(
                psionic_runtime::TASSADAR_RELAXED_SIMD_RESEARCH_PROFILE_ID,
            ),
            ladder_stage_ids: vec![
                String::from("deterministic_simd_anchor"),
                String::from("accelerator_bounded_drift_candidate"),
                String::from("unstable_lane_refusal"),
            ],
            baseline_refs: vec![
                String::from(psionic_runtime::TASSADAR_RELAXED_SIMD_RUNTIME_REPORT_REF),
                String::from("fixtures/tassadar/reports/tassadar_simd_profile_report.json"),
                String::from(TASSADAR_RELAXED_SIMD_LADDER_REPORT_REF),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
                String::from("crates/psionic-provider"),
            ],
            support_boundaries: vec![
                String::from(
                    "relaxed-SIMD remains research-only and never widens the deterministic SIMD profile by inheritance",
                ),
                String::from(
                    "accelerator-bounded drift rows must stay explicit and cannot be promoted to public exactness while cross-backend equivalence is refused",
                ),
                String::from(
                    "unstable lane semantics and accelerator-specific nonportability remain typed refusal truth rather than optimization debt",
                ),
            ],
            summary_report_ref: String::from(TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REPORT_REF),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_relaxed_simd_research_publication|",
            &publication,
        );
        publication
    }
}

#[must_use]
pub fn tassadar_relaxed_simd_research_publication() -> TassadarRelaxedSimdResearchPublication {
    TassadarRelaxedSimdResearchPublication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarRelaxedSimdPublicationStatus, tassadar_relaxed_simd_research_publication};

    #[test]
    fn relaxed_simd_research_publication_is_machine_legible() {
        let publication = tassadar_relaxed_simd_research_publication();

        assert_eq!(
            publication.status,
            TassadarRelaxedSimdPublicationStatus::ImplementedEarly
        );
        assert_eq!(publication.ladder_stage_ids.len(), 3);
        assert!(!publication.publication_digest.is_empty());
    }
}
