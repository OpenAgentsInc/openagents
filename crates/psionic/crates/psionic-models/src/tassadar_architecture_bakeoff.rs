use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_ARCHITECTURE_BAKEOFF_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_ARCHITECTURE_BAKEOFF_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_ARCHITECTURE_BAKEOFF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json";
pub const TASSADAR_ARCHITECTURE_BAKEOFF_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_architecture_bakeoff_summary.json";

/// Stable architecture families compared by the bakeoff lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArchitectureBakeoffFamily {
    CompiledExactExecutor,
    FlatDecoderTraceModel,
    SharedDepthRecurrentRefinement,
    LinearRecurrentizedAttentionExecutor,
    MemoryAugmentedExecutor,
    PointerExecutor,
    SearchNativeExecutor,
    HybridPlannerExecutor,
}

impl TassadarArchitectureBakeoffFamily {
    /// Returns the stable architecture-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompiledExactExecutor => "compiled_exact_executor",
            Self::FlatDecoderTraceModel => "flat_decoder_trace_model",
            Self::SharedDepthRecurrentRefinement => "shared_depth_recurrent_refinement",
            Self::LinearRecurrentizedAttentionExecutor => "linear_recurrentized_attention_executor",
            Self::MemoryAugmentedExecutor => "memory_augmented_executor",
            Self::PointerExecutor => "pointer_executor",
            Self::SearchNativeExecutor => "search_native_executor",
            Self::HybridPlannerExecutor => "hybrid_planner_executor",
        }
    }
}

/// Repo-facing publication status for the architecture bakeoff lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArchitectureBakeoffPublicationStatus {
    Implemented,
}

/// Public publication for the architecture bakeoff lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBakeoffPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarArchitectureBakeoffPublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub architecture_families: Vec<TassadarArchitectureBakeoffFamily>,
    pub workload_families: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarArchitectureBakeoffPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_ARCHITECTURE_BAKEOFF_SCHEMA_VERSION,
            publication_id: String::from("tassadar.architecture_bakeoff.publication.v1"),
            status: TassadarArchitectureBakeoffPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_ARCHITECTURE_BAKEOFF_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-architecture-bakeoff-v0",
                "tassadar_architecture_bakeoff",
                "v0",
            ),
            architecture_families: vec![
                TassadarArchitectureBakeoffFamily::CompiledExactExecutor,
                TassadarArchitectureBakeoffFamily::FlatDecoderTraceModel,
                TassadarArchitectureBakeoffFamily::SharedDepthRecurrentRefinement,
                TassadarArchitectureBakeoffFamily::LinearRecurrentizedAttentionExecutor,
                TassadarArchitectureBakeoffFamily::MemoryAugmentedExecutor,
                TassadarArchitectureBakeoffFamily::PointerExecutor,
                TassadarArchitectureBakeoffFamily::SearchNativeExecutor,
                TassadarArchitectureBakeoffFamily::HybridPlannerExecutor,
            ],
            workload_families: vec![
                String::from("arithmetic_multi_operand"),
                String::from("clrs_shortest_path"),
                String::from("sudoku_backtracking_search"),
                String::from("module_scale_wasm_loop"),
                String::from("long_horizon_control"),
                String::from("checkpointed_resumable_program"),
                String::from("linked_program_bundle"),
                String::from("import_mediated_process"),
                String::from("stateful_process_loop"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-data"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
                String::from("crates/psionic-environments"),
            ],
            validation_refs: vec![
                String::from(
                    "fixtures/tassadar/runs/tassadar_architecture_bakeoff_v1/architecture_bakeoff_budget_bundle.json",
                ),
                String::from(TASSADAR_ARCHITECTURE_BAKEOFF_REPORT_REF),
                String::from(TASSADAR_ARCHITECTURE_BAKEOFF_SUMMARY_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the bakeoff is a same-task same-budget research matrix over shared workload families; it does not promote any architecture family into served capability by itself",
                ),
                String::from(
                    "ownership is workload-family-specific and benchmark-bound; architecture wins in one regime do not imply arbitrary Wasm closure or broad learned exactness elsewhere",
                ),
                String::from(
                    "refuse-first rows remain explicit and do not count as silent degradations or hidden tuning decisions",
                ),
                String::from(
                    "some broadened workloads may still have no honest owner yet; the matrix must preserve that result instead of forcing a promotion winner",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_architecture_bakeoff_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the architecture bakeoff lane.
#[must_use]
pub fn tassadar_architecture_bakeoff_publication() -> TassadarArchitectureBakeoffPublication {
    TassadarArchitectureBakeoffPublication::new()
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
        TassadarArchitectureBakeoffFamily, TassadarArchitectureBakeoffPublicationStatus,
        tassadar_architecture_bakeoff_publication,
    };

    #[test]
    fn architecture_bakeoff_publication_is_machine_legible() {
        let publication = tassadar_architecture_bakeoff_publication();

        assert_eq!(
            publication.status,
            TassadarArchitectureBakeoffPublicationStatus::Implemented
        );
        assert_eq!(publication.architecture_families.len(), 8);
        assert!(
            publication
                .architecture_families
                .contains(&TassadarArchitectureBakeoffFamily::SearchNativeExecutor)
        );
        assert!(
            publication
                .architecture_families
                .contains(&TassadarArchitectureBakeoffFamily::CompiledExactExecutor)
        );
        assert_eq!(publication.workload_families.len(), 9);
        assert!(publication
            .workload_families
            .contains(&String::from("linked_program_bundle")));
        assert!(!publication.publication_digest.is_empty());
    }
}
