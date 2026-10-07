use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_SEARCH_NATIVE_EXECUTOR_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_CLAIM_CLASS: &str =
    "learned_bounded / research_only_architecture";
pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_search_native_executor_report.json";

/// Publication status for the search-native executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSearchNativeExecutorPublicationStatus {
    Implemented,
}

/// Public publication for the search-native executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeExecutorPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarSearchNativeExecutorPublicationStatus,
    pub claim_class: String,
    pub model: ModelDescriptor,
    pub event_surfaces: Vec<String>,
    pub workload_families: Vec<String>,
    pub baseline_refs: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub report_ref: String,
    pub publication_digest: String,
}

impl TassadarSearchNativeExecutorPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_SEARCH_NATIVE_EXECUTOR_SCHEMA_VERSION,
            publication_id: String::from("tassadar.search_native_executor.publication.v1"),
            status: TassadarSearchNativeExecutorPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-search-native-executor-v0",
                "tassadar_search_native_executor",
                "v0",
            ),
            event_surfaces: vec![
                String::from("guess"),
                String::from("verify"),
                String::from("contradict"),
                String::from("backtrack"),
                String::from("branch_summary"),
                String::from("search_budget"),
            ],
            workload_families: vec![
                String::from("sudoku_backtracking_search"),
                String::from("branch_heavy_clrs_variant"),
                String::from("search_kernel_recovery"),
                String::from("verifier_heavy_workload_pack"),
            ],
            baseline_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-data"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
            ],
            support_boundaries: vec![
                String::from(
                    "search-native publication is benchmark-bound and research-only; it does not widen served capability or imply arbitrary combinatorial-solver closure",
                ),
                String::from(
                    "search-budget refusal remains explicit and is preferred over silent degradation on verifier-heavy or nested-branch regimes",
                ),
                String::from(
                    "search-native wins on seeded workloads do not displace straight-trace, pointer, or verifier-guided baselines outside the compared rows",
                ),
            ],
            report_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_search_native_executor_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical search-native publication.
#[must_use]
pub fn tassadar_search_native_executor_publication() -> TassadarSearchNativeExecutorPublication {
    TassadarSearchNativeExecutorPublication::new()
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
        TassadarSearchNativeExecutorPublicationStatus, tassadar_search_native_executor_publication,
    };

    #[test]
    fn search_native_executor_publication_is_machine_legible() {
        let publication = tassadar_search_native_executor_publication();

        assert_eq!(
            publication.status,
            TassadarSearchNativeExecutorPublicationStatus::Implemented
        );
        assert!(
            publication
                .event_surfaces
                .contains(&String::from("branch_summary"))
        );
        assert_eq!(publication.workload_families.len(), 4);
        assert!(!publication.publication_digest.is_empty());
    }
}
