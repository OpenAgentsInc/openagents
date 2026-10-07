use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_POINTER_MEMORY_SCRATCHPAD_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_POINTER_MEMORY_SCRATCHPAD_CLAIM_CLASS: &str =
    "research_only_architecture / learned_bounded_success";
pub const TASSADAR_POINTER_MEMORY_SCRATCHPAD_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_pointer_memory_scratchpad_report.json";
pub const TASSADAR_POINTER_MEMORY_SCRATCHPAD_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_pointer_memory_scratchpad_summary.json";

/// Stable ablation axes in the pointer-versus-memory-versus-scratchpad study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSeparationStudyAxis {
    PointerPrediction,
    MutableMemoryAccess,
    ScratchpadLocalReasoning,
    CombinedReference,
}

impl TassadarSeparationStudyAxis {
    /// Returns the stable study-axis label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PointerPrediction => "pointer_prediction",
            Self::MutableMemoryAccess => "mutable_memory_access",
            Self::ScratchpadLocalReasoning => "scratchpad_local_reasoning",
            Self::CombinedReference => "combined_reference",
        }
    }
}

/// Publication status for the separation-study lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPointerMemoryScratchpadPublicationStatus {
    Implemented,
}

/// Public publication for the separation-study lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarPointerMemoryScratchpadPublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub study_axes: Vec<TassadarSeparationStudyAxis>,
    pub workload_families: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarPointerMemoryScratchpadPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_POINTER_MEMORY_SCRATCHPAD_SCHEMA_VERSION,
            publication_id: String::from("tassadar.pointer_memory_scratchpad.publication.v1"),
            status: TassadarPointerMemoryScratchpadPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-pointer-memory-scratchpad-v0",
                "tassadar_pointer_memory_scratchpad",
                "v0",
            ),
            study_axes: vec![
                TassadarSeparationStudyAxis::PointerPrediction,
                TassadarSeparationStudyAxis::MutableMemoryAccess,
                TassadarSeparationStudyAxis::ScratchpadLocalReasoning,
                TassadarSeparationStudyAxis::CombinedReference,
            ],
            workload_families: vec![
                String::from("clrs_shortest_path"),
                String::from("arithmetic_multi_operand"),
                String::from("sudoku_backtracking_search"),
                String::from("module_scale_wasm_loop"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(
                    "fixtures/tassadar/runs/tassadar_pointer_memory_scratchpad_study_v1/pointer_memory_scratchpad_ablation_bundle.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_pointer_memory_scratchpad_runtime_report.json",
                ),
                String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_REPORT_REF),
                String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_SUMMARY_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the study is a benchmark-bound analytic separation of pointer prediction, mutable memory access, and scratchpad-local reasoning over shared workloads; it does not promote any one mechanism into served capability by itself",
                ),
                String::from(
                    "failure-mode labels stay workload-specific and refusal-bounded instead of being flattened into one blended loss curve",
                ),
                String::from(
                    "combined_reference rows reuse real landed mechanisms and do not imply arbitrary Wasm closure or broad learned exactness",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_pointer_memory_scratchpad_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the separation-study lane.
#[must_use]
pub fn tassadar_pointer_memory_scratchpad_publication() -> TassadarPointerMemoryScratchpadPublication
{
    TassadarPointerMemoryScratchpadPublication::new()
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
        TassadarPointerMemoryScratchpadPublicationStatus, TassadarSeparationStudyAxis,
        tassadar_pointer_memory_scratchpad_publication,
    };

    #[test]
    fn pointer_memory_scratchpad_publication_is_machine_legible() {
        let publication = tassadar_pointer_memory_scratchpad_publication();

        assert_eq!(
            publication.status,
            TassadarPointerMemoryScratchpadPublicationStatus::Implemented
        );
        assert!(
            publication
                .study_axes
                .contains(&TassadarSeparationStudyAxis::ScratchpadLocalReasoning)
        );
        assert_eq!(publication.workload_families.len(), 4);
        assert!(!publication.publication_digest.is_empty());
    }
}
