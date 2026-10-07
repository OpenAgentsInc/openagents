use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_WEAK_SUPERVISION_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WEAK_SUPERVISION_CLAIM_CLASS: &str = "learned_bounded_research_architecture";
pub const TASSADAR_WEAK_SUPERVISION_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/weak_supervision_executor";
pub const TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_weak_supervision_executor_v1/weak_supervision_evidence_bundle.json";
pub const TASSADAR_WEAK_SUPERVISION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_weak_supervision_executor_report.json";
pub const TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_weak_supervision_executor_summary.json";

/// Machine-legible publication status for the weak-supervision executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWeakSupervisionPublicationStatus {
    Implemented,
}

/// Public model-facing publication for the weak-supervision executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status value.
    pub status: TassadarWeakSupervisionPublicationStatus,
    /// Explicit claim class.
    pub claim_class: String,
    /// Stable model descriptor.
    pub model: ModelDescriptor,
    /// Compared supervision regimes.
    pub supervision_regimes: Vec<String>,
    /// Compared workload families.
    pub workload_families: Vec<String>,
    /// Stable source contract ref.
    pub contract_ref: String,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs backing the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarWeakSupervisionPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_WEAK_SUPERVISION_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.weak_supervision.publication.v1"),
            status: TassadarWeakSupervisionPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_WEAK_SUPERVISION_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-weak-supervision-executor-v0",
                "tassadar_weak_supervision_executor",
                "v0",
            ),
            supervision_regimes: vec![
                String::from("full_trace"),
                String::from("mixed_weak"),
                String::from("io_only"),
            ],
            workload_families: vec![
                String::from("module_trace_v2"),
                String::from("hungarian_module"),
                String::from("verifier_search_kernel"),
                String::from("module_state_control"),
            ],
            contract_ref: String::from(TASSADAR_WEAK_SUPERVISION_CONTRACT_REF),
            target_surfaces: vec![
                String::from("crates/psionic-train"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-data"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF),
                String::from(TASSADAR_WEAK_SUPERVISION_REPORT_REF),
                String::from(TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the weak-supervision family stays research-only and keeps full-trace, mixed, and io-only regimes explicitly separate from compiled exactness and served promotion",
                ),
                String::from(
                    "mixed supervision may recover much of the seeded full-trace benefit, but it does not imply broad learned module closure",
                ),
                String::from(
                    "io-only regimes must keep refusal and later-window failure posture explicit whenever the seeded workload outruns the declared supervision density",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest =
            stable_digest(b"psionic_tassadar_weak_supervision_publication|", &publication);
        publication
    }
}

/// Returns the canonical weak-supervision publication.
#[must_use]
pub fn tassadar_weak_supervision_publication() -> TassadarWeakSupervisionPublication {
    TassadarWeakSupervisionPublication::new()
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
        tassadar_weak_supervision_publication, TassadarWeakSupervisionPublicationStatus,
        TASSADAR_WEAK_SUPERVISION_CLAIM_CLASS, TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn weak_supervision_publication_is_machine_legible() {
        let publication = tassadar_weak_supervision_publication();

        assert_eq!(
            publication.status,
            TassadarWeakSupervisionPublicationStatus::Implemented
        );
        assert_eq!(
            publication.claim_class,
            TASSADAR_WEAK_SUPERVISION_CLAIM_CLASS
        );
        assert_eq!(
            publication.validation_refs[0],
            TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF
        );
        assert!(publication
            .workload_families
            .contains(&String::from("module_state_control")));
        assert!(!publication.publication_digest.is_empty());
    }
}
