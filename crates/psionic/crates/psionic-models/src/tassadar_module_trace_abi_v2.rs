use psionic_runtime::TassadarModuleTraceAbiContract;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_MODULE_TRACE_ABI_V2_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json";

/// Machine-legible publication status for the module-trace ABI v2 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleTraceAbiV2PublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// Public model-facing publication for the module-trace ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiV2Publication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarModuleTraceAbiV2PublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Legacy module-trace ABI kept for compatibility comparison.
    pub legacy_trace_abi: TassadarModuleTraceAbiContract,
    /// Frame-aware delta-oriented module-trace ABI.
    pub trace_abi_v2: TassadarModuleTraceAbiContract,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarModuleTraceAbiV2Publication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_MODULE_TRACE_ABI_V2_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.module_trace_abi_v2.publication.v1"),
            status: TassadarModuleTraceAbiV2PublicationStatus::Implemented,
            claim_class: String::from("execution_truth_learned_substrate"),
            legacy_trace_abi: TassadarModuleTraceAbiContract::v1(),
            trace_abi_v2: TassadarModuleTraceAbiContract::v2(),
            target_surfaces: vec![
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![String::from(TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "v2 is frame-aware and delta-oriented for the current bounded module lane only; it does not claim arbitrary Wasm closure or general memory tracing",
                ),
                String::from(
                    "compatibility with v1 is through shared execution and final-state truth, not by preserving per-step full snapshots",
                ),
                String::from(
                    "unsupported host calls still refuse at execution time and therefore remain outside the successful v2 trace family",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_module_trace_abi_v2_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the module-trace ABI v2 lane.
#[must_use]
pub fn tassadar_module_trace_abi_v2_publication() -> TassadarModuleTraceAbiV2Publication {
    TassadarModuleTraceAbiV2Publication::new()
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
        TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF, TassadarModuleTraceAbiV2PublicationStatus,
        tassadar_module_trace_abi_v2_publication,
    };

    #[test]
    fn module_trace_abi_v2_publication_is_machine_legible() {
        let publication = tassadar_module_trace_abi_v2_publication();

        assert_eq!(
            publication.status,
            TassadarModuleTraceAbiV2PublicationStatus::Implemented
        );
        assert_eq!(
            publication.legacy_trace_abi.abi_id,
            "tassadar.module_trace.v1"
        );
        assert_eq!(publication.trace_abi_v2.abi_id, "tassadar.module_trace.v2");
        assert!(publication.trace_abi_v2.frame_aware);
        assert_eq!(
            publication.validation_refs,
            vec![String::from(TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF)]
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
