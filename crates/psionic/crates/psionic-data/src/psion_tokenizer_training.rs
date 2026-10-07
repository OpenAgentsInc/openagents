use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExclusionManifest, PsionLoaderSurface, PsionRawSourceManifest,
    PsionSourceLifecycleManifest, TokenizerDigest, TokenizerFamily,
};

/// Stable schema version for the first Psion tokenizer-training manifest.
pub const PSION_TOKENIZER_TRAINING_MANIFEST_SCHEMA_VERSION: &str =
    "psion.tokenizer_training_manifest.v1";
/// Stable schema version for the first Psion tokenizer artifact bundle.
pub const PSION_TOKENIZER_ARTIFACT_BUNDLE_SCHEMA_VERSION: &str =
    "psion.tokenizer_artifact_bundle.v1";

/// Tokenizer training algorithm admitted by the first Psion tokenizer manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTokenizerTrainingAlgorithm {
    /// SentencePiece unigram training.
    SentencePieceUnigram,
    /// SentencePiece BPE training.
    SentencePieceBpe,
    /// Classic BPE training with separate merge and vocabulary files.
    BytePairEncoding,
}

/// Artifact format emitted for one built tokenizer bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTokenizerArtifactFormat {
    /// One SentencePiece model bundle.
    SentencePieceModelBundle,
    /// Separate BPE vocabulary and merge files.
    BpeMergeVocabularyFiles,
}

/// Training configuration for one reproducible Psion tokenizer build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerTrainingConfig {
    /// Tokenizer family surfaced to later manifests.
    pub tokenizer_family: TokenizerFamily,
    /// Concrete training algorithm.
    pub algorithm: PsionTokenizerTrainingAlgorithm,
    /// Target vocabulary size.
    pub target_vocab_size: u32,
    /// Character coverage represented in basis points.
    pub character_coverage_bps: u32,
    /// Whether ASCII lowercasing is part of the training config.
    pub lowercase_ascii: bool,
    /// Stable special-token inventory.
    pub special_tokens: Vec<String>,
}

/// Source lineage row retained in the tokenizer manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerSourceReference {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Stable digest over the normalized source payload.
    pub source_normalized_digest: String,
}

/// Explicit exposure row retained for later tokenizer audits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerExposureAuditRow {
    /// Stable source identifier.
    pub source_id: String,
    /// Whether tokenizer training may see the source.
    pub tokenizer_exposed: bool,
    /// Whether this source is tokenizer-only rather than model-training visible.
    pub tokenizer_only_exposure: bool,
    /// Whether model training may see the source later.
    pub model_training_exposed: bool,
    /// Short explanation of the exposure posture.
    pub detail: String,
}

/// Reproducible tokenizer-training manifest for the Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerTrainingManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable tokenizer identifier.
    pub tokenizer_id: String,
    /// Stable tokenizer version.
    pub tokenizer_version: String,
    /// Raw-source manifest schema version this tokenizer is bound to.
    pub raw_source_schema_version: String,
    /// Exclusion-manifest schema version this tokenizer is bound to.
    pub exclusion_schema_version: String,
    /// Preprocessing version inherited from raw ingestion.
    pub preprocessing_version: String,
    /// Versioned tokenizer training config.
    pub training_config: PsionTokenizerTrainingConfig,
    /// Sources explicitly admitted into tokenizer training.
    pub admitted_sources: Vec<PsionTokenizerSourceReference>,
    /// Sources explicitly excluded from tokenizer training.
    pub excluded_sources: Vec<PsionTokenizerSourceReference>,
    /// Exposure audit rows covering every raw ingested source.
    pub exposure_report: Vec<PsionTokenizerExposureAuditRow>,
}

impl PsionTokenizerTrainingManifest {
    /// Validates the tokenizer-training manifest against raw-source and isolation inputs.
    pub fn validate_against_inputs(
        &self,
        raw_source_manifest: &PsionRawSourceManifest,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
    ) -> Result<(), PsionTokenizerTrainingError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "tokenizer_training_manifest.schema_version",
        )?;
        if self.schema_version != PSION_TOKENIZER_TRAINING_MANIFEST_SCHEMA_VERSION {
            return Err(PsionTokenizerTrainingError::SchemaVersionMismatch {
                expected: String::from(PSION_TOKENIZER_TRAINING_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.tokenizer_id.as_str(),
            "tokenizer_training_manifest.tokenizer_id",
        )?;
        ensure_nonempty(
            self.tokenizer_version.as_str(),
            "tokenizer_training_manifest.tokenizer_version",
        )?;
        ensure_nonempty(
            self.raw_source_schema_version.as_str(),
            "tokenizer_training_manifest.raw_source_schema_version",
        )?;
        if self.raw_source_schema_version != raw_source_manifest.schema_version {
            return Err(PsionTokenizerTrainingError::SchemaVersionMismatch {
                expected: raw_source_manifest.schema_version.clone(),
                actual: self.raw_source_schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.exclusion_schema_version.as_str(),
            "tokenizer_training_manifest.exclusion_schema_version",
        )?;
        if self.exclusion_schema_version != exclusion_manifest.schema_version {
            return Err(PsionTokenizerTrainingError::SchemaVersionMismatch {
                expected: exclusion_manifest.schema_version.clone(),
                actual: self.exclusion_schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.preprocessing_version.as_str(),
            "tokenizer_training_manifest.preprocessing_version",
        )?;
        if self.preprocessing_version
            != raw_source_manifest
                .normalization_profile
                .preprocessing_version
        {
            return Err(PsionTokenizerTrainingError::FieldMismatch {
                field: String::from("preprocessing_version"),
                expected: raw_source_manifest
                    .normalization_profile
                    .preprocessing_version
                    .clone(),
                actual: self.preprocessing_version.clone(),
            });
        }
        self.validate_training_config()?;
        if self.admitted_sources.is_empty() {
            return Err(PsionTokenizerTrainingError::MissingField {
                field: String::from("tokenizer_training_manifest.admitted_sources"),
            });
        }
        if self.excluded_sources.is_empty() {
            return Err(PsionTokenizerTrainingError::MissingField {
                field: String::from("tokenizer_training_manifest.excluded_sources"),
            });
        }
        if self.exposure_report.is_empty() {
            return Err(PsionTokenizerTrainingError::MissingField {
                field: String::from("tokenizer_training_manifest.exposure_report"),
            });
        }

        let raw_source_map = raw_source_manifest
            .sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<std::collections::BTreeMap<_, _>>();
        let exclusion_map = exclusion_manifest
            .tokenizer_exposure
            .iter()
            .map(|row| (row.source_id.as_str(), row))
            .collect::<std::collections::BTreeMap<_, _>>();

        let admitted_ids = self.validate_source_refs(
            self.admitted_sources.as_slice(),
            raw_source_map.clone(),
            "admitted_sources",
        )?;
        let excluded_ids = self.validate_source_refs(
            self.excluded_sources.as_slice(),
            raw_source_map.clone(),
            "excluded_sources",
        )?;
        let admitted_id_set = admitted_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let excluded_id_set = excluded_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if admitted_id_set
            .intersection(&excluded_id_set)
            .next()
            .is_some()
        {
            return Err(PsionTokenizerTrainingError::AdmittedExcludedOverlap);
        }

        let raw_ids = raw_source_map.keys().copied().collect::<BTreeSet<_>>();
        let listed_ids = admitted_id_set
            .iter()
            .copied()
            .chain(excluded_id_set.iter().copied())
            .collect::<BTreeSet<_>>();
        if raw_ids != listed_ids {
            return Err(PsionTokenizerTrainingError::SourceCoverageMismatch);
        }

        let admitted_id_list = self
            .admitted_sources
            .iter()
            .map(|source| source.source_id.clone())
            .collect::<Vec<_>>();
        exclusion_manifest
            .assert_source_ids_allowed(
                lifecycle_manifest,
                PsionLoaderSurface::TokenizerTraining,
                admitted_id_list.as_slice(),
            )
            .map_err(|error| PsionTokenizerTrainingError::Isolation { error })?;

        let exposure_ids = self.validate_exposure_report(
            raw_source_map,
            exclusion_map,
            admitted_ids.as_slice(),
            excluded_ids.as_slice(),
        )?;
        if exposure_ids != raw_ids {
            return Err(PsionTokenizerTrainingError::ExposureCoverageMismatch);
        }
        Ok(())
    }

    fn validate_training_config(&self) -> Result<(), PsionTokenizerTrainingError> {
        if self.training_config.target_vocab_size == 0 {
            return Err(PsionTokenizerTrainingError::InvalidTrainingConfig {
                detail: String::from("target_vocab_size must be greater than zero"),
            });
        }
        if self.training_config.character_coverage_bps > 10_000 {
            return Err(PsionTokenizerTrainingError::InvalidTrainingConfig {
                detail: String::from("character_coverage_bps must be at most 10000"),
            });
        }
        if self.training_config.special_tokens.is_empty() {
            return Err(PsionTokenizerTrainingError::InvalidTrainingConfig {
                detail: String::from("special_tokens must not be empty"),
            });
        }
        let mut special_tokens = BTreeSet::new();
        for token in &self.training_config.special_tokens {
            ensure_nonempty(
                token.as_str(),
                "tokenizer_training_manifest.training_config.special_tokens[]",
            )?;
            if !special_tokens.insert(token.clone()) {
                return Err(PsionTokenizerTrainingError::InvalidTrainingConfig {
                    detail: format!("special token `{token}` repeated"),
                });
            }
        }
        Ok(())
    }

    fn validate_source_refs(
        &self,
        source_refs: &[PsionTokenizerSourceReference],
        raw_source_map: std::collections::BTreeMap<&str, &crate::PsionRawSourceRecord>,
        field: &str,
    ) -> Result<Vec<String>, PsionTokenizerTrainingError> {
        let mut seen_ids = BTreeSet::new();
        let mut ordered_ids = Vec::with_capacity(source_refs.len());
        for source_ref in source_refs {
            ensure_nonempty(
                source_ref.source_id.as_str(),
                format!("tokenizer_training_manifest.{field}[].source_id").as_str(),
            )?;
            if !seen_ids.insert(source_ref.source_id.clone()) {
                return Err(PsionTokenizerTrainingError::DuplicateSourceReference {
                    source_id: source_ref.source_id.clone(),
                });
            }
            let Some(raw_source) = raw_source_map.get(source_ref.source_id.as_str()) else {
                return Err(PsionTokenizerTrainingError::UnknownSourceId {
                    source_id: source_ref.source_id.clone(),
                });
            };
            if source_ref.source_family_id != raw_source.source_family_id {
                return Err(PsionTokenizerTrainingError::FieldMismatch {
                    field: format!("{field}.source_family_id"),
                    expected: raw_source.source_family_id.clone(),
                    actual: source_ref.source_family_id.clone(),
                });
            }
            if source_ref.source_normalized_digest != raw_source.source_normalized_digest {
                return Err(PsionTokenizerTrainingError::FieldMismatch {
                    field: format!("{field}.source_normalized_digest"),
                    expected: raw_source.source_normalized_digest.clone(),
                    actual: source_ref.source_normalized_digest.clone(),
                });
            }
            ordered_ids.push(source_ref.source_id.clone());
        }
        Ok(ordered_ids)
    }

    fn validate_exposure_report(
        &self,
        raw_source_map: std::collections::BTreeMap<&str, &crate::PsionRawSourceRecord>,
        exclusion_map: std::collections::BTreeMap<&str, &crate::PsionTokenizerExposureRecord>,
        admitted_ids: &[String],
        excluded_ids: &[String],
    ) -> Result<BTreeSet<&str>, PsionTokenizerTrainingError> {
        let admitted = admitted_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let excluded = excluded_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut seen_ids = BTreeSet::new();
        for row in &self.exposure_report {
            ensure_nonempty(
                row.source_id.as_str(),
                "tokenizer_training_manifest.exposure_report[].source_id",
            )?;
            if !seen_ids.insert(row.source_id.as_str()) {
                return Err(PsionTokenizerTrainingError::DuplicateExposureRow {
                    source_id: row.source_id.clone(),
                });
            }
            ensure_nonempty(
                row.detail.as_str(),
                "tokenizer_training_manifest.exposure_report[].detail",
            )?;
            let Some(_raw_source) = raw_source_map.get(row.source_id.as_str()) else {
                return Err(PsionTokenizerTrainingError::UnknownSourceId {
                    source_id: row.source_id.clone(),
                });
            };
            let Some(exclusion_row) = exclusion_map.get(row.source_id.as_str()) else {
                return Err(PsionTokenizerTrainingError::UnknownSourceId {
                    source_id: row.source_id.clone(),
                });
            };
            if row.tokenizer_exposed != exclusion_row.tokenizer_exposed {
                return Err(PsionTokenizerTrainingError::FieldMismatch {
                    field: format!("exposure_report.{}.tokenizer_exposed", row.source_id),
                    expected: exclusion_row.tokenizer_exposed.to_string(),
                    actual: row.tokenizer_exposed.to_string(),
                });
            }
            if row.model_training_exposed != exclusion_row.model_training_exposed {
                return Err(PsionTokenizerTrainingError::FieldMismatch {
                    field: format!("exposure_report.{}.model_training_exposed", row.source_id),
                    expected: exclusion_row.model_training_exposed.to_string(),
                    actual: row.model_training_exposed.to_string(),
                });
            }
            let tokenizer_only_expected =
                exclusion_row.tokenizer_exposed && !exclusion_row.model_training_exposed;
            if row.tokenizer_only_exposure != tokenizer_only_expected {
                return Err(PsionTokenizerTrainingError::FieldMismatch {
                    field: format!("exposure_report.{}.tokenizer_only_exposure", row.source_id),
                    expected: tokenizer_only_expected.to_string(),
                    actual: row.tokenizer_only_exposure.to_string(),
                });
            }
            if row.tokenizer_exposed && !admitted.contains(row.source_id.as_str()) {
                return Err(PsionTokenizerTrainingError::SourceExposureMismatch {
                    source_id: row.source_id.clone(),
                    detail: String::from(
                        "tokenizer-exposed source must be listed in admitted_sources",
                    ),
                });
            }
            if !row.tokenizer_exposed && !excluded.contains(row.source_id.as_str()) {
                return Err(PsionTokenizerTrainingError::SourceExposureMismatch {
                    source_id: row.source_id.clone(),
                    detail: String::from(
                        "non-tokenizer-exposed source must be listed in excluded_sources",
                    ),
                });
            }
        }
        Ok(seen_ids)
    }
}

/// Built tokenizer artifact inventory for one reproducible Psion tokenizer bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerVocabularyArtifact {
    /// Stable vocabulary artifact identifier.
    pub artifact_id: String,
    /// Artifact format emitted by the tokenizer build.
    pub artifact_format: PsionTokenizerArtifactFormat,
    /// Stable digest over the emitted vocabulary inventory.
    pub vocabulary_digest: String,
    /// Stable digest over the tokenizer model or merge artifact.
    pub tokenizer_model_digest: String,
    /// Stable digest over the added or special-token inventory.
    pub added_tokens_digest: String,
}

/// Reproducible tokenizer artifact bundle derived from one training manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerArtifactBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable tokenizer identifier.
    pub tokenizer_id: String,
    /// Stable tokenizer version.
    pub tokenizer_version: String,
    /// Training-manifest schema version this bundle depends on.
    pub training_manifest_schema_version: String,
    /// Raw-source manifest schema version this bundle depends on.
    pub raw_source_schema_version: String,
    /// Exclusion-manifest schema version this bundle depends on.
    pub exclusion_schema_version: String,
    /// Preprocessing version this bundle depends on.
    pub preprocessing_version: String,
    /// Stable digest over the tokenizer training config.
    pub tokenizer_config_digest: String,
    /// Stable tokenizer digest consumed by later dataset and model artifacts.
    pub tokenizer: TokenizerDigest,
    /// Vocabulary and model artifact inventory.
    pub vocabulary_artifact: PsionTokenizerVocabularyArtifact,
    /// Sources admitted into tokenizer training.
    pub admitted_sources: Vec<PsionTokenizerSourceReference>,
    /// Sources excluded from tokenizer training.
    pub excluded_sources: Vec<PsionTokenizerSourceReference>,
    /// Exposure audit rows retained with the bundle.
    pub exposure_report: Vec<PsionTokenizerExposureAuditRow>,
}

impl PsionTokenizerArtifactBundle {
    /// Builds one reproducible tokenizer artifact bundle from the declared training manifest.
    pub fn build_from_manifest(
        manifest: &PsionTokenizerTrainingManifest,
        raw_source_manifest: &PsionRawSourceManifest,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
    ) -> Result<Self, PsionTokenizerTrainingError> {
        manifest.validate_against_inputs(
            raw_source_manifest,
            lifecycle_manifest,
            exclusion_manifest,
        )?;

        let tokenizer_config_digest = stable_digest(
            b"psion_tokenizer_training_config|",
            &manifest.training_config,
        );
        let vocabulary_digest =
            stable_digest(b"psion_tokenizer_vocabulary|", &manifest.admitted_sources);
        let added_tokens_digest = stable_digest(
            b"psion_tokenizer_added_tokens|",
            &manifest.training_config.special_tokens,
        );
        let tokenizer_model_digest = stable_digest(
            b"psion_tokenizer_model_artifact|",
            &(
                manifest.tokenizer_id.as_str(),
                manifest.tokenizer_version.as_str(),
                manifest.preprocessing_version.as_str(),
                manifest.admitted_sources.as_slice(),
                manifest.excluded_sources.as_slice(),
            ),
        );
        let tokenizer_package_digest = stable_digest(
            b"psion_tokenizer_package|",
            &(
                tokenizer_config_digest.as_str(),
                vocabulary_digest.as_str(),
                tokenizer_model_digest.as_str(),
                added_tokens_digest.as_str(),
            ),
        );
        let tokenizer = TokenizerDigest::new(
            manifest.training_config.tokenizer_family,
            tokenizer_package_digest,
            manifest.training_config.target_vocab_size,
        )
        .with_special_tokens_digest(added_tokens_digest.clone())
        .with_template_digest(tokenizer_config_digest.clone());
        let artifact_format = match manifest.training_config.algorithm {
            PsionTokenizerTrainingAlgorithm::SentencePieceUnigram
            | PsionTokenizerTrainingAlgorithm::SentencePieceBpe => {
                PsionTokenizerArtifactFormat::SentencePieceModelBundle
            }
            PsionTokenizerTrainingAlgorithm::BytePairEncoding => {
                PsionTokenizerArtifactFormat::BpeMergeVocabularyFiles
            }
        };

        Ok(Self {
            schema_version: String::from(PSION_TOKENIZER_ARTIFACT_BUNDLE_SCHEMA_VERSION),
            tokenizer_id: manifest.tokenizer_id.clone(),
            tokenizer_version: manifest.tokenizer_version.clone(),
            training_manifest_schema_version: manifest.schema_version.clone(),
            raw_source_schema_version: manifest.raw_source_schema_version.clone(),
            exclusion_schema_version: manifest.exclusion_schema_version.clone(),
            preprocessing_version: manifest.preprocessing_version.clone(),
            tokenizer_config_digest,
            tokenizer,
            vocabulary_artifact: PsionTokenizerVocabularyArtifact {
                artifact_id: format!(
                    "{}:{}:vocabulary_bundle",
                    manifest.tokenizer_id, manifest.tokenizer_version
                ),
                artifact_format,
                vocabulary_digest,
                tokenizer_model_digest,
                added_tokens_digest,
            },
            admitted_sources: manifest.admitted_sources.clone(),
            excluded_sources: manifest.excluded_sources.clone(),
            exposure_report: manifest.exposure_report.clone(),
        })
    }

    /// Validates the bundle against the manifest inputs it claims to represent.
    pub fn validate_against_manifest(
        &self,
        manifest: &PsionTokenizerTrainingManifest,
        raw_source_manifest: &PsionRawSourceManifest,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
    ) -> Result<(), PsionTokenizerTrainingError> {
        manifest.validate_against_inputs(
            raw_source_manifest,
            lifecycle_manifest,
            exclusion_manifest,
        )?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "tokenizer_artifact_bundle.schema_version",
        )?;
        if self.schema_version != PSION_TOKENIZER_ARTIFACT_BUNDLE_SCHEMA_VERSION {
            return Err(PsionTokenizerTrainingError::SchemaVersionMismatch {
                expected: String::from(PSION_TOKENIZER_ARTIFACT_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        check_string_match(
            self.tokenizer_id.as_str(),
            manifest.tokenizer_id.as_str(),
            "tokenizer_id",
        )?;
        check_string_match(
            self.tokenizer_version.as_str(),
            manifest.tokenizer_version.as_str(),
            "tokenizer_version",
        )?;
        check_string_match(
            self.training_manifest_schema_version.as_str(),
            manifest.schema_version.as_str(),
            "training_manifest_schema_version",
        )?;
        check_string_match(
            self.raw_source_schema_version.as_str(),
            manifest.raw_source_schema_version.as_str(),
            "raw_source_schema_version",
        )?;
        check_string_match(
            self.exclusion_schema_version.as_str(),
            manifest.exclusion_schema_version.as_str(),
            "exclusion_schema_version",
        )?;
        check_string_match(
            self.preprocessing_version.as_str(),
            manifest.preprocessing_version.as_str(),
            "preprocessing_version",
        )?;
        ensure_nonempty(
            self.tokenizer_config_digest.as_str(),
            "tokenizer_artifact_bundle.tokenizer_config_digest",
        )?;
        ensure_nonempty(
            self.tokenizer.tokenizer_digest.as_str(),
            "tokenizer_artifact_bundle.tokenizer.tokenizer_digest",
        )?;
        if self.tokenizer.vocab_size != manifest.training_config.target_vocab_size {
            return Err(PsionTokenizerTrainingError::FieldMismatch {
                field: String::from("tokenizer.vocab_size"),
                expected: manifest.training_config.target_vocab_size.to_string(),
                actual: self.tokenizer.vocab_size.to_string(),
            });
        }
        ensure_nonempty(
            self.vocabulary_artifact.artifact_id.as_str(),
            "tokenizer_artifact_bundle.vocabulary_artifact.artifact_id",
        )?;
        ensure_nonempty(
            self.vocabulary_artifact.vocabulary_digest.as_str(),
            "tokenizer_artifact_bundle.vocabulary_artifact.vocabulary_digest",
        )?;
        ensure_nonempty(
            self.vocabulary_artifact.tokenizer_model_digest.as_str(),
            "tokenizer_artifact_bundle.vocabulary_artifact.tokenizer_model_digest",
        )?;
        ensure_nonempty(
            self.vocabulary_artifact.added_tokens_digest.as_str(),
            "tokenizer_artifact_bundle.vocabulary_artifact.added_tokens_digest",
        )?;
        if self.admitted_sources != manifest.admitted_sources
            || self.excluded_sources != manifest.excluded_sources
            || self.exposure_report != manifest.exposure_report
        {
            return Err(PsionTokenizerTrainingError::FieldMismatch {
                field: String::from("artifact_bundle.lineage"),
                expected: String::from("bundle lineage must match the training manifest"),
                actual: String::from("bundle lineage drifted from the training manifest"),
            });
        }
        Ok(())
    }
}

/// Error returned by Psion tokenizer training and artifact-bundle validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionTokenizerTrainingError {
    /// One required field was missing or empty.
    #[error("Psion tokenizer contract field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version did not match the expected contract.
    #[error("Psion tokenizer contract expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One field drifted from the linked raw-source or exclusion contract.
    #[error("Psion tokenizer contract field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// The training configuration was internally inconsistent.
    #[error("Psion tokenizer training config is invalid: {detail}")]
    InvalidTrainingConfig {
        /// Machine-readable detail.
        detail: String,
    },
    /// One source id was unknown to the raw-source manifest.
    #[error("Psion tokenizer contract does not know source `{source_id}`")]
    UnknownSourceId {
        /// Unknown source identifier.
        source_id: String,
    },
    /// One source reference was repeated.
    #[error("Psion tokenizer contract repeated source `{source_id}`")]
    DuplicateSourceReference {
        /// Repeated source identifier.
        source_id: String,
    },
    /// A tokenizer exposure row was repeated.
    #[error("Psion tokenizer exposure report repeated source `{source_id}`")]
    DuplicateExposureRow {
        /// Repeated source identifier.
        source_id: String,
    },
    /// Admitted and excluded sources overlapped.
    #[error("Psion tokenizer manifest cannot list the same source as both admitted and excluded")]
    AdmittedExcludedOverlap,
    /// Admitted and excluded source lists did not cover the raw-source manifest.
    #[error("Psion tokenizer manifest admitted and excluded source lists must cover all raw-source rows")]
    SourceCoverageMismatch,
    /// Exposure report did not cover the raw-source manifest.
    #[error("Psion tokenizer exposure report must cover every raw-source row")]
    ExposureCoverageMismatch,
    /// One exposure row contradicted the admitted/excluded lists.
    #[error("Psion tokenizer exposure mismatch for source `{source_id}`: {detail}")]
    SourceExposureMismatch {
        /// Source identifier.
        source_id: String,
        /// Machine-readable detail.
        detail: String,
    },
    /// The held-out isolation contract rejected the admitted source list.
    #[error("Psion tokenizer manifest violates the held-out isolation contract: {error}")]
    Isolation {
        /// Upstream isolation validation error.
        error: crate::PsionBenchmarkIsolationError,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionTokenizerTrainingError> {
    if value.trim().is_empty() {
        return Err(PsionTokenizerTrainingError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionTokenizerTrainingError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionTokenizerTrainingError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("tokenizer value should serialize"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PsionExclusionManifest, PsionRawSourceManifest, PsionSourceLifecycleManifest};

    fn raw_source_manifest() -> PsionRawSourceManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/ingestion/psion_raw_source_manifest_v1.json"
        ))
        .expect("raw-source manifest should parse")
    }

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle manifest should parse")
    }

    fn exclusion_manifest() -> PsionExclusionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/isolation/psion_exclusion_manifest_v1.json"
        ))
        .expect("exclusion manifest should parse")
    }

    fn tokenizer_training_manifest() -> PsionTokenizerTrainingManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenizer/psion_tokenizer_training_manifest_v1.json"
        ))
        .expect("tokenizer training manifest should parse")
    }

    fn tokenizer_artifact_bundle() -> PsionTokenizerArtifactBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json"
        ))
        .expect("tokenizer artifact bundle should parse")
    }

    #[test]
    fn tokenizer_training_manifest_builds_reproducible_bundle() {
        let raw = raw_source_manifest();
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let manifest = tokenizer_training_manifest();
        manifest
            .validate_against_inputs(&raw, &lifecycle, &exclusion)
            .expect("tokenizer training manifest should validate");
        let bundle = PsionTokenizerArtifactBundle::build_from_manifest(
            &manifest, &raw, &lifecycle, &exclusion,
        )
        .expect("tokenizer bundle should build");
        assert_eq!(
            bundle.tokenizer.vocab_size,
            manifest.training_config.target_vocab_size
        );
        assert_eq!(bundle.admitted_sources.len(), 3);
        assert_eq!(bundle.excluded_sources.len(), 1);

        let fixture_bundle = tokenizer_artifact_bundle();
        fixture_bundle
            .validate_against_manifest(&manifest, &raw, &lifecycle, &exclusion)
            .expect("fixture bundle should validate");
        assert_eq!(fixture_bundle.tokenizer_id, bundle.tokenizer_id);
        assert_eq!(fixture_bundle.tokenizer_version, bundle.tokenizer_version);
    }

    #[test]
    fn excluded_sources_remain_explicit() {
        let manifest = tokenizer_training_manifest();
        assert_eq!(manifest.excluded_sources.len(), 1);
        assert_eq!(
            manifest.excluded_sources[0].source_id,
            "spec_quiz_eval_pack_v1"
        );
    }

    #[test]
    fn tokenizer_only_exposure_must_stay_distinct_from_model_training_exposure() {
        let raw = raw_source_manifest();
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let mut manifest = tokenizer_training_manifest();
        let row = manifest
            .exposure_report
            .iter_mut()
            .find(|row| row.source_id == "vendor_manual_private_scan_v1")
            .expect("fixture should include tokenizer-only exposure row");
        row.tokenizer_only_exposure = false;
        let error = manifest
            .validate_against_inputs(&raw, &lifecycle, &exclusion)
            .expect_err("tokenizer-only exposure drift should be rejected");
        assert!(matches!(
            error,
            PsionTokenizerTrainingError::FieldMismatch { .. }
        ));
    }
}
