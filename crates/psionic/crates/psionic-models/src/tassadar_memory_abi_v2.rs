use psionic_runtime::TassadarMemoryAbiContract;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_MEMORY_ABI_V2_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MEMORY_ABI_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json";

/// Machine-legible publication status for the byte-addressed memory ABI v2 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemoryAbiPublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// Public model-facing publication for the byte-addressed memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiV2Publication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarMemoryAbiPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Declared runtime-owned memory ABI contract.
    pub memory_abi: TassadarMemoryAbiContract,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation report refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarMemoryAbiV2Publication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_MEMORY_ABI_V2_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.memory_abi_v2.publication.v1"),
            status: TassadarMemoryAbiPublicationStatus::Implemented,
            claim_class: String::from("execution_truth_fast_path_substrate"),
            memory_abi: TassadarMemoryAbiContract::linear_memory_v2(),
            target_surfaces: vec![
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![String::from(TASSADAR_MEMORY_ABI_V2_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "bounded straight-line immediate-address programs only; structured control flow and call frames remain separate follow-on work",
                ),
                String::from(
                    "supports i8/i16/i32 loads and stores plus memory.size, memory.grow, memory.copy, and memory.fill; globals, tables, indirect calls, and imports remain outside this publication",
                ),
                String::from(
                    "trace publication is delta-oriented for byte writes and memory growth instead of full snapshots",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest =
            stable_digest(b"psionic_tassadar_memory_abi_v2_publication|", &publication);
        publication
    }
}

/// Returns the canonical public publication for the byte-addressed memory ABI v2 lane.
#[must_use]
pub fn tassadar_memory_abi_v2_publication() -> TassadarMemoryAbiV2Publication {
    TassadarMemoryAbiV2Publication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarMemoryAbiPublicationStatus, tassadar_memory_abi_v2_publication};
    use psionic_runtime::{TassadarMemoryAddressingMode, TassadarMemoryTraceMode};

    #[test]
    fn memory_abi_v2_publication_is_machine_legible() {
        let publication = tassadar_memory_abi_v2_publication();

        assert_eq!(
            publication.status,
            TassadarMemoryAbiPublicationStatus::Implemented
        );
        assert_eq!(
            publication.memory_abi.addressing_mode,
            TassadarMemoryAddressingMode::ByteAddressedLinearMemory
        );
        assert_eq!(
            publication.memory_abi.trace_mode,
            TassadarMemoryTraceMode::DeltaOriented
        );
        assert_eq!(
            publication.validation_refs,
            vec![String::from(super::TASSADAR_MEMORY_ABI_V2_REPORT_REF)]
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
