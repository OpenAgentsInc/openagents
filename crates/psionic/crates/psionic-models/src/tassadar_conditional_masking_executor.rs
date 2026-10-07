use psionic_runtime::TassadarConditionalMaskingContract;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_CONDITIONAL_MASKING_EXECUTOR_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_CONDITIONAL_MASKING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_conditional_masking_report.json";

/// Repo-facing status for the conditional-masking executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarConditionalMaskingExecutorPublicationStatus {
    /// The lane exists as an early research surface.
    ImplementedEarly,
}

/// Explicit pointer-head family in the conditional-masking executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPointerHeadKind {
    /// Predict one local-slot address inside a bounded local window.
    LocalSlotPointer,
    /// Predict one frame address inside a bounded frame window.
    FrameSlotPointer,
    /// Predict one contiguous memory region inside a bounded span.
    MemoryRegionPointer,
}

/// Public repo-facing publication for the conditional-masking executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingExecutorPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarConditionalMaskingExecutorPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Runtime-owned bounded address-family contract.
    pub runtime_contract: TassadarConditionalMaskingContract,
    /// Explicit pointer heads carried by the lane.
    pub pointer_heads: Vec<TassadarPointerHeadKind>,
    /// Stable target surfaces.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarConditionalMaskingExecutorPublication {
    fn new(runtime_contract: TassadarConditionalMaskingContract) -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_CONDITIONAL_MASKING_EXECUTOR_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.conditional_masking_executor.publication.v1"),
            status: TassadarConditionalMaskingExecutorPublicationStatus::ImplementedEarly,
            claim_class: String::from("learned_bounded_success"),
            model: ModelDescriptor::new(
                "tassadar-conditional-masking-executor-candidate-v0",
                "tassadar_conditional_masking_executor",
                "v0",
            ),
            runtime_contract,
            pointer_heads: vec![
                TassadarPointerHeadKind::LocalSlotPointer,
                TassadarPointerHeadKind::FrameSlotPointer,
                TassadarPointerHeadKind::MemoryRegionPointer,
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![String::from(TASSADAR_CONDITIONAL_MASKING_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "pointer prediction here is bounded to explicit local-slot, frame-window, and memory-region families only; it is not a claim of arbitrary pointer arithmetic or arbitrary mutable-address closure",
                ),
                String::from(
                    "conditional masks may constrain attention and selection over bounded candidate sets only; out-of-family address regimes must refuse instead of silently degrading",
                ),
                String::from(
                    "the lane stays a learned bounded success surface and does not imply compiled exactness, arbitrary Wasm closure, or served promotion",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_conditional_masking_executor_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical publication for the conditional-masking executor lane.
#[must_use]
pub fn tassadar_conditional_masking_executor_publication(
) -> TassadarConditionalMaskingExecutorPublication {
    TassadarConditionalMaskingExecutorPublication::new(
        psionic_runtime::tassadar_conditional_masking_contract(),
    )
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
        tassadar_conditional_masking_executor_publication,
        TassadarConditionalMaskingExecutorPublicationStatus, TassadarPointerHeadKind,
    };

    #[test]
    fn conditional_masking_executor_publication_is_machine_legible() {
        let publication = tassadar_conditional_masking_executor_publication();

        assert_eq!(
            publication.status,
            TassadarConditionalMaskingExecutorPublicationStatus::ImplementedEarly
        );
        assert_eq!(
            publication.model.family,
            "tassadar_conditional_masking_executor"
        );
        assert!(!publication.publication_digest.is_empty());
    }

    #[test]
    fn conditional_masking_executor_publication_carries_pointer_heads_and_runtime_contract() {
        let publication = tassadar_conditional_masking_executor_publication();

        assert!(publication
            .pointer_heads
            .contains(&TassadarPointerHeadKind::FrameSlotPointer));
        assert_eq!(
            publication.runtime_contract.contract_id,
            "tassadar.conditional_masking.contract.v1"
        );
    }
}
