use psionic_ir::{tassadar_locality_preserving_scratchpad_passes, TassadarLocalityScratchpadPass};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_LOCALITY_SCRATCHPAD_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json";

/// Machine-legible publication status for the locality-preserving scratchpad pass lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityScratchpadPublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// Public model-facing publication for the locality-preserving scratchpad pass lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarLocalityScratchpadPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Public pass contracts carried by the lane.
    pub passes: Vec<TassadarLocalityScratchpadPass>,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarLocalityScratchpadPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_LOCALITY_SCRATCHPAD_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.locality_scratchpad.publication.v1"),
            status: TassadarLocalityScratchpadPublicationStatus::Implemented,
            claim_class: String::from("execution_truth_learned_substrate_fast_path_substrate"),
            passes: tassadar_locality_preserving_scratchpad_passes(),
            target_surfaces: vec![
                String::from("crates/psionic-ir"),
                String::from("crates/psionic-compiler"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![String::from(TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "the pass reformats only the seeded symbolic straight-line and module-trace-v2 families; unsupported trace families refuse explicitly",
                ),
                String::from(
                    "the pass preserves replayable source token truth only; it does not imply general learned exactness, arbitrary long-horizon closure, or semantic program rewrites",
                ),
                String::from(
                    "over-budget scratchpad expansion refuses instead of silently widening the current bounded locality contract",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_locality_scratchpad_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the locality-preserving scratchpad pass lane.
#[must_use]
pub fn tassadar_locality_scratchpad_publication() -> TassadarLocalityScratchpadPublication {
    TassadarLocalityScratchpadPublication::new()
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
        tassadar_locality_scratchpad_publication, TassadarLocalityScratchpadPublicationStatus,
        TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF,
    };

    #[test]
    fn locality_scratchpad_publication_is_machine_legible() {
        let publication = tassadar_locality_scratchpad_publication();

        assert_eq!(
            publication.status,
            TassadarLocalityScratchpadPublicationStatus::Implemented
        );
        assert_eq!(publication.passes.len(), 2);
        assert_eq!(
            publication.validation_refs,
            vec![String::from(TASSADAR_LOCALITY_SCRATCHPAD_REPORT_REF)]
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
