use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_BROAD_FAMILY_SPECIALIZATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_BROAD_FAMILY_SPECIALIZATION_CLAIM_CLASS: &str =
    "research_only_architecture_promotion_discipline";
pub const TASSADAR_BROAD_FAMILY_SPECIALIZATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_family_specialization_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadFamilySpecializationPublicationStatus {
    ImplementedEarly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarBroadFamilySpecializationPublicationStatus,
    pub claim_class: String,
    pub specialization_family_ids: Vec<String>,
    pub safety_gate_requires_decompilation: bool,
    pub safety_gate_requires_lineage: bool,
    pub baseline_refs: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub report_ref: String,
    pub publication_digest: String,
}

impl TassadarBroadFamilySpecializationPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_BROAD_FAMILY_SPECIALIZATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.broad_family_specialization.publication.v1"),
            status: TassadarBroadFamilySpecializationPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_BROAD_FAMILY_SPECIALIZATION_CLAIM_CLASS),
            specialization_family_ids: vec![
                String::from("state_machine_bundle"),
                String::from("search_frontier_bundle"),
                String::from("linked_worker_bundle"),
                String::from("effectful_resume_bundle"),
            ],
            safety_gate_requires_decompilation: true,
            safety_gate_requires_lineage: true,
            baseline_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_program_to_weights_benchmark_suite.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_decompilation_fidelity_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_program_family_frontier_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_hybrid_process_controller_report.json"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
                String::from("crates/psionic-provider"),
            ],
            support_boundaries: vec![
                String::from(
                    "broad-family specialization remains research-only and benchmark-gated; it does not widen served capability or imply arbitrary program-to-weights closure",
                ),
                String::from(
                    "decompilation fidelity, lineage, and safety-gate readiness must remain explicit per family; unstable or non-decompilable artifacts refuse instead of silently promoting",
                ),
                String::from(
                    "specialization families here are bounded reusable program families, not a claim that arbitrary Wasm modules or broad internal compute can move safely into weights",
                ),
            ],
            report_ref: String::from(TASSADAR_BROAD_FAMILY_SPECIALIZATION_REPORT_REF),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_broad_family_specialization_publication|",
            &publication,
        );
        publication
    }
}

#[must_use]
pub fn tassadar_broad_family_specialization_publication(
) -> TassadarBroadFamilySpecializationPublication {
    TassadarBroadFamilySpecializationPublication::new()
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
        tassadar_broad_family_specialization_publication,
        TassadarBroadFamilySpecializationPublicationStatus,
    };

    #[test]
    fn broad_family_specialization_publication_is_machine_legible() {
        let publication = tassadar_broad_family_specialization_publication();

        assert_eq!(
            publication.status,
            TassadarBroadFamilySpecializationPublicationStatus::ImplementedEarly
        );
        assert!(publication.safety_gate_requires_decompilation);
        assert_eq!(publication.specialization_family_ids.len(), 4);
        assert!(!publication.publication_digest.is_empty());
    }
}
