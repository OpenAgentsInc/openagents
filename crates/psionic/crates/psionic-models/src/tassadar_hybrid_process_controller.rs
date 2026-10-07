use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_HYBRID_PROCESS_CONTROLLER_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_CLAIM_CLASS: &str =
    "research_only_architecture_routing_surface";
pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hybrid_process_controller_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHybridProcessControllerPublicationStatus {
    ImplementedEarly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarHybridProcessControllerPublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub controller_modes: Vec<String>,
    pub workload_families: Vec<String>,
    pub baseline_refs: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub report_ref: String,
    pub publication_digest: String,
}

impl TassadarHybridProcessControllerPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_HYBRID_PROCESS_CONTROLLER_SCHEMA_VERSION,
            publication_id: String::from("tassadar.hybrid_process_controller.publication.v1"),
            status: TassadarHybridProcessControllerPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-hybrid-process-controller-v0",
                "tassadar_hybrid_process_controller",
                "v0",
            ),
            controller_modes: vec![
                String::from("compiled_exact"),
                String::from("learned_state_update"),
                String::from("search_procedure"),
                String::from("verifier_attachment"),
            ],
            workload_families: vec![
                String::from("session_counter_resume"),
                String::from("search_frontier_resume"),
                String::from("linked_package_worker"),
                String::from("effectful_mailbox_transition"),
            ],
            baseline_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_program_family_frontier_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_search_native_executor_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-router"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-provider"),
            ],
            support_boundaries: vec![
                String::from(
                    "the hybrid controller is a research-only bounded routing surface and does not widen served capability or imply broad practical internal computation",
                ),
                String::from(
                    "verifier attachment is mandatory for the hybrid lane inside the seeded benchmark family; unsupported hybrid transitions must refuse instead of silently degrading",
                ),
                String::from(
                    "compiled exact, learned state update, and search procedures remain separate lanes; this publication records one bounded controller over them rather than flattening them into a generic executor claim",
                ),
            ],
            report_ref: String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_REPORT_REF),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_hybrid_process_controller_publication|",
            &publication,
        );
        publication
    }
}

#[must_use]
pub fn tassadar_hybrid_process_controller_publication() -> TassadarHybridProcessControllerPublication
{
    TassadarHybridProcessControllerPublication::new()
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
        tassadar_hybrid_process_controller_publication,
        TassadarHybridProcessControllerPublicationStatus,
    };

    #[test]
    fn hybrid_process_controller_publication_is_machine_legible() {
        let publication = tassadar_hybrid_process_controller_publication();

        assert_eq!(
            publication.status,
            TassadarHybridProcessControllerPublicationStatus::ImplementedEarly
        );
        assert!(publication
            .controller_modes
            .contains(&String::from("verifier_attachment")));
        assert_eq!(publication.workload_families.len(), 4);
        assert!(!publication.publication_digest.is_empty());
    }
}
