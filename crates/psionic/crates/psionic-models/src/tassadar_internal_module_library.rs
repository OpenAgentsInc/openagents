use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_INTERNAL_MODULE_LIBRARY_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_INTERNAL_MODULE_LIBRARY_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json";
pub const TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_module_library_summary.json";

/// Machine-legible publication status for the internal module library lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalModuleLibraryPublicationStatus {
    /// Landed as a public repo-backed lane.
    Implemented,
}

/// Public model-facing publication for the internal computational module library lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibraryPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarInternalModuleLibraryPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable descriptor for the lane.
    pub model: ModelDescriptor,
    /// Active module refs in the current library surface.
    pub active_module_refs: Vec<String>,
    /// Consumer families linked against the current library surface.
    pub linked_consumer_families: Vec<String>,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs backing the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarInternalModuleLibraryPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_INTERNAL_MODULE_LIBRARY_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.internal_module_library.publication.v1"),
            status: TassadarInternalModuleLibraryPublicationStatus::Implemented,
            claim_class: String::from(
                "compiled bounded exactness / promotion discipline / served capability",
            ),
            model: ModelDescriptor::new(
                "tassadar-internal-module-library-v0",
                "tassadar_internal_module_library",
                "v0",
            ),
            active_module_refs: vec![
                String::from("frontier_relax_core@1.0.0"),
                String::from("candidate_select_core@1.1.0"),
                String::from("checkpoint_backtrack_core@1.0.0"),
            ],
            linked_consumer_families: vec![
                String::from("clrs_shortest_path"),
                String::from("clrs_wasm_shortest_path"),
                String::from("hungarian_matching"),
                String::from("verifier_search"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-compiler"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-serve"),
                String::from("crates/psionic-provider"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_INTERNAL_MODULE_LIBRARY_RUNTIME_REPORT_REF),
                String::from(TASSADAR_INTERNAL_MODULE_LIBRARY_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the internal module library is a bounded versioned artifact surface with explicit compatibility digests, link manifests, and rollback posture; it is not unrestricted self-extension",
                ),
                String::from(
                    "only the published module refs and consumer families are claimed today; later install or self-extension work remains explicitly separate",
                ),
                String::from(
                    "served posture stays benchmark-gated through the runtime report and summary rather than widening from compiler artifacts alone",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_internal_module_library_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the internal module library lane.
#[must_use]
pub fn tassadar_internal_module_library_publication() -> TassadarInternalModuleLibraryPublication {
    TassadarInternalModuleLibraryPublication::new()
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
        TASSADAR_INTERNAL_MODULE_LIBRARY_RUNTIME_REPORT_REF,
        TassadarInternalModuleLibraryPublicationStatus,
        tassadar_internal_module_library_publication,
    };

    #[test]
    fn internal_module_library_publication_is_machine_legible() {
        let publication = tassadar_internal_module_library_publication();

        assert_eq!(
            publication.status,
            TassadarInternalModuleLibraryPublicationStatus::Implemented
        );
        assert!(
            publication
                .active_module_refs
                .contains(&String::from("candidate_select_core@1.1.0"))
        );
        assert!(
            publication
                .linked_consumer_families
                .contains(&String::from("verifier_search"))
        );
        assert_eq!(
            publication.validation_refs[0],
            TASSADAR_INTERNAL_MODULE_LIBRARY_RUNTIME_REPORT_REF
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
