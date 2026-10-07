use psionic_ir::{tassadar_structured_numeric_encodings, TassadarStructuredNumericEncoding};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_NUMERIC_ENCODING_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_NUMERIC_ENCODING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_numeric_encoding_report.json";

/// Machine-legible publication status for the structured numeric encoding lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericEncodingPublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// Public model-facing publication for the structured numeric encoding lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericEncodingPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarNumericEncodingPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Public structured encodings carried by the lane.
    pub encodings: Vec<TassadarStructuredNumericEncoding>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarNumericEncodingPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_NUMERIC_ENCODING_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.numeric_encoding.publication.v1"),
            status: TassadarNumericEncodingPublicationStatus::Implemented,
            claim_class: String::from("learned_bounded_success"),
            encodings: tassadar_structured_numeric_encodings(),
            validation_refs: vec![String::from(TASSADAR_NUMERIC_ENCODING_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "the lane changes only the learned representation of bounded numeric fields and must decode exactly back to the same numeric semantics",
                ),
                String::from(
                    "improved held-out vocabulary coverage is not a claim of arbitrary numeric closure, architecture-indifferent exactness, or served promotion",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_numeric_encoding_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the structured numeric encoding lane.
#[must_use]
pub fn tassadar_numeric_encoding_publication() -> TassadarNumericEncodingPublication {
    TassadarNumericEncodingPublication::new()
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
        tassadar_numeric_encoding_publication, TassadarNumericEncodingPublicationStatus,
        TASSADAR_NUMERIC_ENCODING_REPORT_REF,
    };

    #[test]
    fn numeric_encoding_publication_is_machine_legible() {
        let publication = tassadar_numeric_encoding_publication();
        assert_eq!(
            publication.status,
            TassadarNumericEncodingPublicationStatus::Implemented
        );
        assert_eq!(publication.encodings.len(), 9);
        assert_eq!(
            publication.validation_refs,
            vec![String::from(TASSADAR_NUMERIC_ENCODING_REPORT_REF)]
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
