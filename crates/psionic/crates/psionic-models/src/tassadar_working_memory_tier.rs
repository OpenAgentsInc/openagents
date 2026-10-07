use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_WORKING_MEMORY_TIER_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORKING_MEMORY_TIER_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json";
pub const TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json";
pub const TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_summary.json";

/// Machine-legible publication status for the working-memory tier research lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryTierPublicationStatus {
    /// Landed as a repo-backed research surface.
    Implemented,
}

/// Explicit bounded memory primitive admitted by the research lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryPrimitive {
    /// Read one declared memory slot.
    ReadSlot,
    /// Write one declared memory slot.
    WriteSlot,
    /// Reset one declared memory slot.
    ClearSlot,
    /// Read from one bounded associative table.
    AssociativeRead,
    /// Write to one bounded associative table.
    AssociativeWrite,
    /// Publish the bounded state surface for replay and lineage.
    PublishState,
}

/// Bounded workload family used by the research-only memory-tier lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryKernelFamily {
    /// Sliding-window copy and replay kernels.
    CopyWindow,
    /// Small bounded stable sort kernels.
    StableSort,
    /// Bounded associative-recall kernels.
    AssociativeRecall,
    /// Long carry-propagation accumulator kernels.
    LongCarryAccumulator,
}

/// Public model-facing publication for the working-memory tier lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryTierPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarWorkingMemoryTierPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Bounded kernel families admitted by the lane today.
    pub kernel_families: Vec<TassadarWorkingMemoryKernelFamily>,
    /// Bounded memory primitives admitted by the lane today.
    pub memory_primitives: Vec<TassadarWorkingMemoryPrimitive>,
    /// Maximum slot count the lane will admit directly.
    pub bounded_slot_count: u32,
    /// Maximum bytes per slot the lane will admit directly.
    pub max_bytes_per_slot: u32,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs backing the public lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarWorkingMemoryTierPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_WORKING_MEMORY_TIER_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.working_memory_tier.publication.v1"),
            status: TassadarWorkingMemoryTierPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_WORKING_MEMORY_TIER_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-working-memory-tier-v0",
                "tassadar_working_memory_tier",
                "v0",
            ),
            kernel_families: vec![
                TassadarWorkingMemoryKernelFamily::CopyWindow,
                TassadarWorkingMemoryKernelFamily::StableSort,
                TassadarWorkingMemoryKernelFamily::AssociativeRecall,
                TassadarWorkingMemoryKernelFamily::LongCarryAccumulator,
            ],
            memory_primitives: vec![
                TassadarWorkingMemoryPrimitive::ReadSlot,
                TassadarWorkingMemoryPrimitive::WriteSlot,
                TassadarWorkingMemoryPrimitive::ClearSlot,
                TassadarWorkingMemoryPrimitive::AssociativeRead,
                TassadarWorkingMemoryPrimitive::AssociativeWrite,
                TassadarWorkingMemoryPrimitive::PublishState,
            ],
            bounded_slot_count: 64,
            max_bytes_per_slot: 256,
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF),
                String::from(TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF),
                String::from(TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the memory tier is a bounded Psionic-owned research surface with explicit read, write, clear, associative lookup, and state-publication semantics; it is not an opaque tool bridge or external store",
                ),
                String::from(
                    "direct support stays bounded to declared slot counts, bytes per slot, and current copy-window, stable-sort, associative-recall, and long-carry accumulator kernels",
                ),
                String::from(
                    "the lane compares pure-trace and bounded working-memory execution while keeping replay receipts, refusal posture, and state-publication lineage explicit; it does not prove arbitrary-memory closure or broad learned computation",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_working_memory_tier_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the working-memory tier lane.
#[must_use]
pub fn tassadar_working_memory_tier_publication() -> TassadarWorkingMemoryTierPublication {
    TassadarWorkingMemoryTierPublication::new()
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
        TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF, TassadarWorkingMemoryKernelFamily,
        TassadarWorkingMemoryPrimitive, TassadarWorkingMemoryTierPublicationStatus,
        tassadar_working_memory_tier_publication,
    };

    #[test]
    fn working_memory_tier_publication_is_machine_legible() {
        let publication = tassadar_working_memory_tier_publication();

        assert_eq!(
            publication.status,
            TassadarWorkingMemoryTierPublicationStatus::Implemented
        );
        assert!(
            publication
                .kernel_families
                .contains(&TassadarWorkingMemoryKernelFamily::AssociativeRecall)
        );
        assert!(
            publication
                .memory_primitives
                .contains(&TassadarWorkingMemoryPrimitive::PublishState)
        );
        assert_eq!(
            publication.validation_refs[0],
            TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
