use psionic_ir::{TassadarStateDesignFamily, TassadarStateDesignReplayPosture};
use psionic_runtime::{
    TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF, TassadarStateDesignWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_STATE_DESIGN_STUDY_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_STATE_DESIGN_STUDY_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/state_design_study";
pub const TASSADAR_STATE_DESIGN_STUDY_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_state_design_study_report.json";

/// Machine-legible publication status for the state-design study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStateDesignStudyPublicationStatus {
    /// Landed as a repo-backed research surface.
    Implemented,
}

/// Public model-facing publication for the state-design study lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignStudyPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarStateDesignStudyPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Public state-design families in the study.
    pub state_design_families: Vec<TassadarStateDesignFamily>,
    /// Strongest replay posture currently admitted per family.
    pub replay_postures: Vec<TassadarStateDesignReplayPosture>,
    /// Same-workload families compared by the study.
    pub workload_targets: Vec<TassadarStateDesignWorkloadFamily>,
    /// Stable source contract ref.
    pub study_contract_ref: String,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs backing the publication.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarStateDesignStudyPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_STATE_DESIGN_STUDY_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.state_design_study.publication.v1"),
            status: TassadarStateDesignStudyPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_STATE_DESIGN_STUDY_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-state-design-study-v0",
                "tassadar_state_design_study",
                "v0",
            ),
            state_design_families: vec![
                TassadarStateDesignFamily::FullAppendOnlyTrace,
                TassadarStateDesignFamily::DeltaTrace,
                TassadarStateDesignFamily::LocalityScratchpad,
                TassadarStateDesignFamily::RecurrentState,
                TassadarStateDesignFamily::WorkingMemoryTier,
            ],
            replay_postures: vec![
                TassadarStateDesignReplayPosture::ExactReplay,
                TassadarStateDesignReplayPosture::ReconstructableReplay,
                TassadarStateDesignReplayPosture::ExactReplay,
                TassadarStateDesignReplayPosture::BoundedStatePublication,
                TassadarStateDesignReplayPosture::BoundedStatePublication,
            ],
            workload_targets: vec![
                TassadarStateDesignWorkloadFamily::ModuleCallTrace,
                TassadarStateDesignWorkloadFamily::SymbolicLocality,
                TassadarStateDesignWorkloadFamily::AssociativeRecall,
                TassadarStateDesignWorkloadFamily::LongHorizonControl,
                TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            ],
            study_contract_ref: String::from(TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF),
            target_surfaces: vec![
                String::from("crates/psionic-ir"),
                String::from("crates/psionic-data"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF),
                String::from(TASSADAR_STATE_DESIGN_STUDY_EVAL_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the study compares representation surfaces on the same workloads, but it does not promote one representation into a broad served capability claim",
                ),
                String::from(
                    "bounded recurrent-state and working-memory rows remain explicit bounded-state-publication surfaces rather than replay-complete trace closure",
                ),
                String::from(
                    "delta and scratchpad wins stay honest only when deterministic reconstruction or exact source-token recovery remains explicit",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_state_design_study_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the state-design study.
#[must_use]
pub fn tassadar_state_design_study_publication() -> TassadarStateDesignStudyPublication {
    TassadarStateDesignStudyPublication::new()
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
        TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF, TASSADAR_STATE_DESIGN_STUDY_CLAIM_CLASS,
        TassadarStateDesignStudyPublicationStatus, tassadar_state_design_study_publication,
    };
    use psionic_ir::TassadarStateDesignFamily;
    use psionic_runtime::TassadarStateDesignWorkloadFamily;

    #[test]
    fn state_design_study_publication_is_machine_legible() {
        let publication = tassadar_state_design_study_publication();

        assert_eq!(
            publication.status,
            TassadarStateDesignStudyPublicationStatus::Implemented
        );
        assert_eq!(
            publication.claim_class,
            TASSADAR_STATE_DESIGN_STUDY_CLAIM_CLASS
        );
        assert!(
            publication
                .state_design_families
                .contains(&TassadarStateDesignFamily::WorkingMemoryTier)
        );
        assert!(
            publication
                .workload_targets
                .contains(&TassadarStateDesignWorkloadFamily::LongHorizonControl)
        );
        assert_eq!(
            publication.validation_refs[0],
            TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
