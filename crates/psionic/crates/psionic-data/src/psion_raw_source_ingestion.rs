use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    PsionCorpusAdmissionManifest, PsionCorpusSourceKind, PsionSourceBoundaryKind,
    PsionSourceLifecycleManifest, PsionSourceLifecycleState, PsionSourceRightsPosture,
};

/// Stable schema version for the first Psion raw-source ingestion manifest.
pub const PSION_RAW_SOURCE_MANIFEST_SCHEMA_VERSION: &str = "psion.raw_source_manifest.v1";

/// Import mode for one raw-source document snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRawSourceImportKind {
    /// Plain text snapshot copied from the reviewed source.
    PlainTextSnapshot,
    /// PDF or scan text extracted into normalized text.
    PdfExtractedText,
    /// HTML snapshot captured from the reviewed source.
    HtmlSnapshot,
    /// Markdown snapshot captured from the reviewed source.
    MarkdownSnapshot,
    /// Source-tree or file-graph snapshot for code-like sources.
    CodeTreeSnapshot,
    /// Structured record stream captured for benchmark or eval packs.
    RecordStreamSnapshot,
}

/// Normalization step applied during raw-source ingestion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRawSourceNormalizationStep {
    /// Normalize Unicode to NFC.
    UnicodeNfc,
    /// Normalize line endings to one canonical form.
    NormalizeLineEndings,
    /// Remove running headers or footers when they are not semantic content.
    StripRunningHeaders,
    /// Remove repeated boilerplate such as navigation or scan disclaimers.
    StripBoilerplate,
    /// Preserve chapter or section anchors through normalization.
    PreserveSectionAnchors,
    /// Preserve page-range anchors through normalization.
    PreservePageAnchors,
    /// Preserve file-path anchors through normalization.
    PreserveFileAnchors,
    /// Preserve record anchors through normalization.
    PreserveRecordAnchors,
}

/// Versioned normalization profile for one raw-source manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRawSourceNormalizationProfile {
    /// Stable preprocessing version identifier.
    pub preprocessing_version: String,
    /// Ordered normalization steps applied to all imported sources.
    pub normalization_steps: Vec<PsionRawSourceNormalizationStep>,
    /// Whether document boundaries are preserved in the emitted manifest.
    pub preserves_document_boundaries: bool,
    /// Whether section or record boundaries are preserved in the emitted manifest.
    pub preserves_section_boundaries: bool,
}

/// One preserved section, page, file, or record boundary inside an ingested document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRawSourceSectionBoundary {
    /// Stable section identifier.
    pub section_id: String,
    /// Boundary-anchor kind preserved for the section.
    pub boundary_kind: PsionSourceBoundaryKind,
    /// Stable order within the document.
    pub order_index: u32,
    /// Human-readable title or label for the section.
    pub title: String,
    /// Stable inclusive start anchor.
    pub start_anchor: String,
    /// Stable inclusive end anchor.
    pub end_anchor: String,
    /// Stable digest over the normalized section payload.
    pub normalized_section_digest: String,
}

/// One imported document snapshot inside an ingested source family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRawSourceDocument {
    /// Stable document identifier.
    pub document_id: String,
    /// Stable order within the source family.
    pub order_index: u32,
    /// Import mode for this document snapshot.
    pub import_kind: PsionRawSourceImportKind,
    /// Stable import pointer or path captured during ingestion.
    pub import_reference: String,
    /// Stable digest over the raw document payload before normalization.
    pub raw_document_digest: String,
    /// Stable digest over the normalized document payload.
    pub normalized_document_digest: String,
    /// Preserved boundaries required for later packing and contamination review.
    pub section_boundaries: Vec<PsionRawSourceSectionBoundary>,
}

/// One ingested raw-source row bound to admission and lifecycle truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRawSourceRecord {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Reviewed source kind.
    pub source_kind: PsionCorpusSourceKind,
    /// Current rights posture at ingest time.
    pub current_rights_posture: PsionSourceRightsPosture,
    /// Current lifecycle state at ingest time.
    pub lifecycle_state: PsionSourceLifecycleState,
    /// Stable digest over the admitted raw source payload.
    pub source_raw_digest: String,
    /// Stable digest over the normalized source payload family.
    pub source_normalized_digest: String,
    /// Imported documents that make up the source family.
    pub documents: Vec<PsionRawSourceDocument>,
    /// Short explanation of the normalization posture applied to the source.
    pub normalization_detail: String,
}

/// Versioned raw-source ingestion manifest for admitted or restricted Psion sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRawSourceManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Admission schema version this manifest is anchored to.
    pub admission_schema_version: String,
    /// Lifecycle schema version this manifest is anchored to.
    pub lifecycle_schema_version: String,
    /// Versioned normalization profile applied during ingestion.
    pub normalization_profile: PsionRawSourceNormalizationProfile,
    /// Ingested sources in stable source-id order.
    pub sources: Vec<PsionRawSourceRecord>,
}

impl PsionRawSourceManifest {
    /// Creates one reproducible raw-source manifest from already reviewed sources.
    pub fn new(
        normalization_profile: PsionRawSourceNormalizationProfile,
        admission: &PsionCorpusAdmissionManifest,
        lifecycle: &PsionSourceLifecycleManifest,
        mut sources: Vec<PsionRawSourceRecord>,
    ) -> Result<Self, PsionRawSourceIngestionError> {
        sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
        for source in &mut sources {
            source
                .documents
                .sort_by_key(|document| (document.order_index, document.document_id.clone()));
            for document in &mut source.documents {
                document
                    .section_boundaries
                    .sort_by_key(|boundary| (boundary.order_index, boundary.section_id.clone()));
            }
        }
        let manifest = Self {
            schema_version: String::from(PSION_RAW_SOURCE_MANIFEST_SCHEMA_VERSION),
            admission_schema_version: admission.schema_version.clone(),
            lifecycle_schema_version: lifecycle.schema_version.clone(),
            normalization_profile,
            sources,
        };
        manifest.validate_against_lifecycle(admission, lifecycle)?;
        Ok(manifest)
    }

    /// Validates the raw-source manifest against the admission and lifecycle contracts.
    pub fn validate_against_lifecycle(
        &self,
        admission: &PsionCorpusAdmissionManifest,
        lifecycle: &PsionSourceLifecycleManifest,
    ) -> Result<(), PsionRawSourceIngestionError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "raw_source_manifest.schema_version",
        )?;
        if self.schema_version != PSION_RAW_SOURCE_MANIFEST_SCHEMA_VERSION {
            return Err(PsionRawSourceIngestionError::SchemaVersionMismatch {
                expected: String::from(PSION_RAW_SOURCE_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.admission_schema_version.as_str(),
            "raw_source_manifest.admission_schema_version",
        )?;
        if self.admission_schema_version != admission.schema_version {
            return Err(PsionRawSourceIngestionError::SchemaVersionMismatch {
                expected: admission.schema_version.clone(),
                actual: self.admission_schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.lifecycle_schema_version.as_str(),
            "raw_source_manifest.lifecycle_schema_version",
        )?;
        if self.lifecycle_schema_version != lifecycle.schema_version {
            return Err(PsionRawSourceIngestionError::SchemaVersionMismatch {
                expected: lifecycle.schema_version.clone(),
                actual: self.lifecycle_schema_version.clone(),
            });
        }
        self.validate_normalization_profile()?;
        if self.sources.is_empty() {
            return Err(PsionRawSourceIngestionError::MissingField {
                field: String::from("raw_source_manifest.sources"),
            });
        }

        let mut seen_source_ids = BTreeSet::new();
        for source in &self.sources {
            ensure_nonempty(
                source.source_id.as_str(),
                "raw_source_manifest.sources[].source_id",
            )?;
            if !seen_source_ids.insert(source.source_id.clone()) {
                return Err(PsionRawSourceIngestionError::DuplicateSourceId {
                    source_id: source.source_id.clone(),
                });
            }
            let Some(admission_record) = admission
                .sources
                .iter()
                .find(|record| record.source_id == source.source_id)
            else {
                return Err(PsionRawSourceIngestionError::UnknownSourceId {
                    source_id: source.source_id.clone(),
                });
            };
            let Some(lifecycle_record) = lifecycle
                .sources
                .iter()
                .find(|record| record.source_id == source.source_id)
            else {
                return Err(PsionRawSourceIngestionError::UnknownSourceId {
                    source_id: source.source_id.clone(),
                });
            };

            match lifecycle_record.lifecycle_state {
                PsionSourceLifecycleState::Admitted
                | PsionSourceLifecycleState::Restricted
                | PsionSourceLifecycleState::EvaluationOnly => {}
                PsionSourceLifecycleState::Withdrawn | PsionSourceLifecycleState::Rejected => {
                    return Err(PsionRawSourceIngestionError::IneligibleLifecycleState {
                        source_id: source.source_id.clone(),
                        lifecycle_state: lifecycle_record.lifecycle_state,
                    });
                }
            }

            check_string_match(
                source.source_family_id.as_str(),
                admission_record.source_family_id.as_str(),
                source.source_id.as_str(),
                "source_family_id",
            )?;
            if source.source_kind != admission_record.source_kind {
                return Err(PsionRawSourceIngestionError::SourceFieldMismatch {
                    source_id: source.source_id.clone(),
                    field: String::from("source_kind"),
                    expected: format!("{:?}", admission_record.source_kind).to_lowercase(),
                    actual: format!("{:?}", source.source_kind).to_lowercase(),
                });
            }
            if source.current_rights_posture != lifecycle_record.current_rights_posture {
                return Err(PsionRawSourceIngestionError::SourceFieldMismatch {
                    source_id: source.source_id.clone(),
                    field: String::from("current_rights_posture"),
                    expected: format!("{:?}", lifecycle_record.current_rights_posture)
                        .to_lowercase(),
                    actual: format!("{:?}", source.current_rights_posture).to_lowercase(),
                });
            }
            if source.lifecycle_state != lifecycle_record.lifecycle_state {
                return Err(PsionRawSourceIngestionError::SourceFieldMismatch {
                    source_id: source.source_id.clone(),
                    field: String::from("lifecycle_state"),
                    expected: format!("{:?}", lifecycle_record.lifecycle_state).to_lowercase(),
                    actual: format!("{:?}", source.lifecycle_state).to_lowercase(),
                });
            }
            check_string_match(
                source.source_raw_digest.as_str(),
                admission_record.content_digest.as_str(),
                source.source_id.as_str(),
                "source_raw_digest",
            )?;
            ensure_nonempty(
                source.source_normalized_digest.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.source_normalized_digest",
                    source.source_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                source.normalization_detail.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.normalization_detail",
                    source.source_id
                )
                .as_str(),
            )?;
            if source.documents.is_empty() {
                return Err(PsionRawSourceIngestionError::MissingField {
                    field: format!("raw_source_manifest.sources.{}.documents", source.source_id),
                });
            }
            self.validate_documents(source, admission_record.boundary_kind)?;
        }

        Ok(())
    }

    fn validate_normalization_profile(&self) -> Result<(), PsionRawSourceIngestionError> {
        ensure_nonempty(
            self.normalization_profile.preprocessing_version.as_str(),
            "raw_source_manifest.normalization_profile.preprocessing_version",
        )?;
        if self.normalization_profile.normalization_steps.is_empty() {
            return Err(PsionRawSourceIngestionError::MissingField {
                field: String::from(
                    "raw_source_manifest.normalization_profile.normalization_steps",
                ),
            });
        }
        let mut steps = BTreeSet::new();
        for step in &self.normalization_profile.normalization_steps {
            if !steps.insert(*step) {
                return Err(PsionRawSourceIngestionError::DuplicateNormalizationStep {
                    step: *step,
                });
            }
        }
        if !self.normalization_profile.preserves_document_boundaries
            || !self.normalization_profile.preserves_section_boundaries
        {
            return Err(PsionRawSourceIngestionError::BoundaryPreservationDisabled);
        }
        Ok(())
    }

    fn validate_documents(
        &self,
        source: &PsionRawSourceRecord,
        expected_boundary_kind: PsionSourceBoundaryKind,
    ) -> Result<(), PsionRawSourceIngestionError> {
        let mut seen_document_ids = BTreeSet::new();
        let mut previous_document_order = None;
        for document in &source.documents {
            ensure_nonempty(
                document.document_id.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents[].document_id",
                    source.source_id
                )
                .as_str(),
            )?;
            if !seen_document_ids.insert(document.document_id.clone()) {
                return Err(PsionRawSourceIngestionError::DuplicateDocumentId {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                });
            }
            if previous_document_order.is_some_and(|previous| document.order_index <= previous) {
                return Err(PsionRawSourceIngestionError::NonMonotonicDocumentOrder {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                });
            }
            previous_document_order = Some(document.order_index);
            ensure_nonempty(
                document.import_reference.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.import_reference",
                    source.source_id, document.document_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                document.raw_document_digest.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.raw_document_digest",
                    source.source_id, document.document_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                document.normalized_document_digest.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.normalized_document_digest",
                    source.source_id, document.document_id
                )
                .as_str(),
            )?;
            if document.section_boundaries.is_empty() {
                return Err(PsionRawSourceIngestionError::MissingDocumentBoundaries {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                });
            }
            self.validate_sections(source, document, expected_boundary_kind)?;
        }
        Ok(())
    }

    fn validate_sections(
        &self,
        source: &PsionRawSourceRecord,
        document: &PsionRawSourceDocument,
        expected_boundary_kind: PsionSourceBoundaryKind,
    ) -> Result<(), PsionRawSourceIngestionError> {
        let mut seen_section_ids = BTreeSet::new();
        let mut previous_section_order = None;
        for section in &document.section_boundaries {
            ensure_nonempty(
                section.section_id.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.section_boundaries[].section_id",
                    source.source_id, document.document_id
                )
                .as_str(),
            )?;
            if !seen_section_ids.insert(section.section_id.clone()) {
                return Err(PsionRawSourceIngestionError::DuplicateSectionId {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                    section_id: section.section_id.clone(),
                });
            }
            if previous_section_order.is_some_and(|previous| section.order_index <= previous) {
                return Err(PsionRawSourceIngestionError::NonMonotonicSectionOrder {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                    section_id: section.section_id.clone(),
                });
            }
            previous_section_order = Some(section.order_index);
            if section.boundary_kind != expected_boundary_kind {
                return Err(PsionRawSourceIngestionError::BoundaryKindMismatch {
                    source_id: source.source_id.clone(),
                    document_id: document.document_id.clone(),
                    expected: expected_boundary_kind,
                    actual: section.boundary_kind,
                });
            }
            ensure_nonempty(
                section.title.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.section_boundaries.{}.title",
                    source.source_id, document.document_id, section.section_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                section.start_anchor.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.section_boundaries.{}.start_anchor",
                    source.source_id, document.document_id, section.section_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                section.end_anchor.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.section_boundaries.{}.end_anchor",
                    source.source_id, document.document_id, section.section_id
                )
                .as_str(),
            )?;
            ensure_nonempty(
                section.normalized_section_digest.as_str(),
                format!(
                    "raw_source_manifest.sources.{}.documents.{}.section_boundaries.{}.normalized_section_digest",
                    source.source_id, document.document_id, section.section_id
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

/// Error returned by Psion raw-source ingestion contract validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionRawSourceIngestionError {
    /// One required field was missing or empty.
    #[error("Psion raw-source ingestion field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version did not match the expected upstream contract.
    #[error("Psion raw-source ingestion expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The manifest referenced a source id unknown to admission or lifecycle truth.
    #[error("Psion raw-source ingestion does not know source `{source_id}`")]
    UnknownSourceId {
        /// Unknown source identifier.
        source_id: String,
    },
    /// The manifest attempted to ingest a source outside the allowed lifecycle states.
    #[error(
        "Psion raw-source ingestion cannot ingest source `{source_id}` while lifecycle state is `{lifecycle_state:?}`"
    )]
    IneligibleLifecycleState {
        /// Source identifier.
        source_id: String,
        /// Ineligible lifecycle state.
        lifecycle_state: PsionSourceLifecycleState,
    },
    /// One field drifted from admission or lifecycle truth.
    #[error(
        "Psion raw-source ingestion source `{source_id}` field `{field}` expected `{expected}`, found `{actual}`"
    )]
    SourceFieldMismatch {
        /// Source identifier.
        source_id: String,
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// The manifest repeated a source id.
    #[error("Psion raw-source ingestion repeated source `{source_id}`")]
    DuplicateSourceId {
        /// Repeated source identifier.
        source_id: String,
    },
    /// The normalization profile repeated a step.
    #[error("Psion raw-source normalization profile repeated step `{step:?}`")]
    DuplicateNormalizationStep {
        /// Repeated normalization step.
        step: PsionRawSourceNormalizationStep,
    },
    /// The normalization profile disabled required boundary preservation.
    #[error(
        "Psion raw-source normalization profile must preserve document and section boundaries"
    )]
    BoundaryPreservationDisabled,
    /// One source repeated a document id.
    #[error("Psion raw-source source `{source_id}` repeated document `{document_id}`")]
    DuplicateDocumentId {
        /// Source identifier.
        source_id: String,
        /// Repeated document identifier.
        document_id: String,
    },
    /// One source used non-monotonic document ordering.
    #[error("Psion raw-source source `{source_id}` document `{document_id}` is out of order")]
    NonMonotonicDocumentOrder {
        /// Source identifier.
        source_id: String,
        /// Document identifier.
        document_id: String,
    },
    /// One document omitted preserved boundaries.
    #[error(
        "Psion raw-source source `{source_id}` document `{document_id}` is missing preserved boundaries"
    )]
    MissingDocumentBoundaries {
        /// Source identifier.
        source_id: String,
        /// Document identifier.
        document_id: String,
    },
    /// One document repeated a section id.
    #[error(
        "Psion raw-source source `{source_id}` document `{document_id}` repeated section `{section_id}`"
    )]
    DuplicateSectionId {
        /// Source identifier.
        source_id: String,
        /// Document identifier.
        document_id: String,
        /// Repeated section identifier.
        section_id: String,
    },
    /// One document used non-monotonic section ordering.
    #[error(
        "Psion raw-source source `{source_id}` document `{document_id}` section `{section_id}` is out of order"
    )]
    NonMonotonicSectionOrder {
        /// Source identifier.
        source_id: String,
        /// Document identifier.
        document_id: String,
        /// Section identifier.
        section_id: String,
    },
    /// One section drifted from the reviewed boundary kind.
    #[error(
        "Psion raw-source source `{source_id}` document `{document_id}` expected boundary kind `{expected:?}`, found `{actual:?}`"
    )]
    BoundaryKindMismatch {
        /// Source identifier.
        source_id: String,
        /// Document identifier.
        document_id: String,
        /// Expected boundary kind from admission review.
        expected: PsionSourceBoundaryKind,
        /// Actual boundary kind in the raw-source manifest.
        actual: PsionSourceBoundaryKind,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionRawSourceIngestionError> {
    if value.trim().is_empty() {
        return Err(PsionRawSourceIngestionError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    source_id: &str,
    field: &str,
) -> Result<(), PsionRawSourceIngestionError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionRawSourceIngestionError::SourceFieldMismatch {
            source_id: String::from(source_id),
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PsionCorpusAdmissionManifest, PsionSourceLifecycleManifest};

    fn admission_manifest() -> PsionCorpusAdmissionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json"
        ))
        .expect("admission fixture should parse")
    }

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle fixture should parse")
    }

    fn raw_source_manifest() -> PsionRawSourceManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/ingestion/psion_raw_source_manifest_v1.json"
        ))
        .expect("raw-source manifest fixture should parse")
    }

    #[test]
    fn raw_source_manifest_validates_against_admission_and_lifecycle() {
        let manifest = raw_source_manifest();
        manifest
            .validate_against_lifecycle(&admission_manifest(), &lifecycle_manifest())
            .expect("raw-source manifest should validate");
        assert_eq!(manifest.sources.len(), 4);
        assert!(manifest
            .sources
            .iter()
            .any(|source| source.lifecycle_state == PsionSourceLifecycleState::EvaluationOnly));
    }

    #[test]
    fn rejected_sources_cannot_enter_raw_source_manifest() {
        let mut manifest = raw_source_manifest();
        manifest.sources.push(PsionRawSourceRecord {
            source_id: String::from("forum_scrape_misc_001"),
            source_family_id: String::from("social_web_noise"),
            source_kind: PsionCorpusSourceKind::ExpertDiscussion,
            current_rights_posture: PsionSourceRightsPosture::Rejected,
            lifecycle_state: PsionSourceLifecycleState::Rejected,
            source_raw_digest: String::from(
                "sha256:1c9c2407fbb9384c3d08a1fc7ce1f6868d1d3c976be9b6ecf2cc24de25c12d73",
            ),
            source_normalized_digest: String::from(
                "sha256:forum_scrape_misc_001_normalized_digest_v1",
            ),
            documents: vec![PsionRawSourceDocument {
                document_id: String::from("forum_scrape_misc_001:dump"),
                order_index: 1,
                import_kind: PsionRawSourceImportKind::PlainTextSnapshot,
                import_reference: String::from("quarantine://forum_scrape_misc_001/dump.txt"),
                raw_document_digest: String::from(
                    "sha256:forum_scrape_misc_001_raw_document_digest_v1",
                ),
                normalized_document_digest: String::from(
                    "sha256:forum_scrape_misc_001_normalized_document_digest_v1",
                ),
                section_boundaries: vec![PsionRawSourceSectionBoundary {
                    section_id: String::from("forum_scrape_misc_001:record_0001"),
                    boundary_kind: PsionSourceBoundaryKind::RecordAnchors,
                    order_index: 1,
                    title: String::from("record 0001"),
                    start_anchor: String::from("record-0001"),
                    end_anchor: String::from("record-0001"),
                    normalized_section_digest: String::from(
                        "sha256:forum_scrape_misc_001_record_0001_digest_v1",
                    ),
                }],
            }],
            normalization_detail: String::from(
                "Rejected forum dump retained only as a negative test.",
            ),
        });
        let error = manifest
            .validate_against_lifecycle(&admission_manifest(), &lifecycle_manifest())
            .expect_err("rejected sources should be rejected");
        assert!(matches!(
            error,
            PsionRawSourceIngestionError::IneligibleLifecycleState { .. }
        ));
    }

    #[test]
    fn section_boundary_kind_must_match_reviewed_boundary_kind() {
        let mut manifest = raw_source_manifest();
        manifest.sources[0].documents[0].section_boundaries[0].boundary_kind =
            PsionSourceBoundaryKind::PageRangeAnchors;
        let error = manifest
            .validate_against_lifecycle(&admission_manifest(), &lifecycle_manifest())
            .expect_err("boundary kind drift should be rejected");
        assert!(matches!(
            error,
            PsionRawSourceIngestionError::BoundaryKindMismatch { .. }
        ));
    }
}
