use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_RECURRENT_FAST_PATH_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_RECURRENT_FAST_PATH_CLAIM_CLASS: &str =
    "research_only_systems_work_fast_path_baseline";
pub const TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_matrix.json";
pub const TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_summary.json";

/// Machine-legible publication status for the recurrent fast-path baseline lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRecurrentFastPathPublicationStatus {
    /// Landed as a repo-backed public research surface.
    Implemented,
}

/// One explicit runtime state channel carried by the recurrent baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRecurrentFastPathStateChannel {
    /// Current program-counter location.
    ProgramCounter,
    /// Mutable operand stack.
    OperandStack,
    /// Mutable local-slot state.
    LocalState,
    /// Mutable memory-slot state.
    MemoryState,
    /// Emitted output register state.
    OutputState,
}

/// Public model-facing publication for the recurrent fast-path runtime baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRecurrentFastPathPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarRecurrentFastPathPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable baseline descriptor for the lane.
    pub model: ModelDescriptor,
    /// Workload families the direct recurrent baseline admits today.
    pub direct_workload_families: Vec<String>,
    /// Workload families that still route through the exact fallback lane.
    pub fallback_workload_families: Vec<String>,
    /// Explicit recurrent state channels carried by the lane.
    pub state_channels: Vec<TassadarRecurrentFastPathStateChannel>,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarRecurrentFastPathPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_RECURRENT_FAST_PATH_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.recurrent_fast_path.publication.v1"),
            status: TassadarRecurrentFastPathPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_RECURRENT_FAST_PATH_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-recurrent-fast-path-runtime-baseline-v0",
                "tassadar_recurrent_fast_path_runtime",
                "v0",
            ),
            direct_workload_families: vec![
                String::from("micro_wasm_kernel"),
                String::from("branch_heavy_kernel"),
                String::from("memory_heavy_kernel"),
                String::from("long_loop_kernel"),
            ],
            fallback_workload_families: vec![
                String::from("sudoku_class"),
                String::from("hungarian_matching"),
            ],
            state_channels: vec![
                TassadarRecurrentFastPathStateChannel::ProgramCounter,
                TassadarRecurrentFastPathStateChannel::OperandStack,
                TassadarRecurrentFastPathStateChannel::LocalState,
                TassadarRecurrentFastPathStateChannel::MemoryState,
                TassadarRecurrentFastPathStateChannel::OutputState,
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF),
                String::from(TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF),
                String::from(TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the lane is a research-only runtime baseline used to compare specialized fast paths against one explicit recurrent state-carry baseline; it is not a served capability publication or a promotion gate by itself",
                ),
                String::from(
                    "direct recurrent execution is only claimed on the current micro_wasm_kernel, branch_heavy_kernel, memory_heavy_kernel, and long_loop_kernel article-class families; sudoku_class and hungarian_matching still fall back explicitly to reference_linear",
                ),
                String::from(
                    "the baseline proves exact bounded article-class execution with explicit state receipts and trace-growth comparisons, not arbitrary Wasm closure, approximate-attention equivalence, or general learned executor exactness",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_recurrent_fast_path_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the recurrent fast-path baseline lane.
#[must_use]
pub fn tassadar_recurrent_fast_path_publication() -> TassadarRecurrentFastPathPublication {
    TassadarRecurrentFastPathPublication::new()
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
        tassadar_recurrent_fast_path_publication, TassadarRecurrentFastPathPublicationStatus,
        TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
    };

    #[test]
    fn recurrent_fast_path_publication_is_machine_legible() {
        let publication = tassadar_recurrent_fast_path_publication();

        assert_eq!(
            publication.status,
            TassadarRecurrentFastPathPublicationStatus::Implemented
        );
        assert!(publication
            .direct_workload_families
            .contains(&String::from("micro_wasm_kernel")));
        assert!(publication
            .fallback_workload_families
            .contains(&String::from("hungarian_matching")));
        assert_eq!(
            publication.validation_refs[0],
            TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
