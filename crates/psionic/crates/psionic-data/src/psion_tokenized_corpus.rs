use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DatasetIterationMode, DatasetPackingMode, DatasetShardOrdering, DatasetSplitKind,
    OverlongSequencePosture, PsionExclusionManifest, PsionLoaderSurface, PsionRawSourceManifest,
    PsionSourceLifecycleManifest, PsionTokenizerArtifactBundle, TokenizerDigest,
};

/// Stable schema version for the first Psion tokenized corpus manifest.
pub const PSION_TOKENIZED_CORPUS_MANIFEST_SCHEMA_VERSION: &str =
    "psion.tokenized_corpus_manifest.v1";

/// Versioned packing-policy declaration for one Psion tokenized corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedPackingPolicy {
    /// Stable policy identifier.
    pub policy_id: String,
    /// Stable packing-policy version.
    pub policy_version: String,
    /// Packing mode used while producing tokenized examples.
    pub packing_mode: DatasetPackingMode,
    /// Maximum token length represented by the packed corpus.
    pub max_sequence_tokens: u32,
    /// Posture applied to overlong sequences.
    pub overlong_sequence_posture: OverlongSequencePosture,
}

/// Replay-safe iteration contract for one Psion tokenized corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedReplayContract {
    /// Iteration mode used by later training or eval loops.
    pub iteration_mode: DatasetIterationMode,
    /// Shard ordering policy used by later training or eval loops.
    pub shard_ordering: DatasetShardOrdering,
    /// Stable deterministic shuffle seed for replay.
    pub deterministic_shuffle_seed: u64,
    /// Stable dataset identity used by replay and checkpoint recovery.
    pub stable_dataset_identity: String,
}

/// Top-level source-family binding retained for later held-out reporting and ablation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedSourceFamilyBinding {
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Source ids in that family represented by this corpus.
    pub source_ids: Vec<String>,
}

/// Raw-source lineage preserved on one tokenized shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedShardSourceLineage {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Stable normalized-source digest.
    pub source_normalized_digest: String,
}

/// One tokenized shard inside the Psion corpus contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedShardManifest {
    /// Stable shard identifier.
    pub shard_id: String,
    /// Split name that owns the shard.
    pub split_name: String,
    /// High-level split kind.
    pub split_kind: DatasetSplitKind,
    /// Stable storage or artifact reference for the shard.
    pub storage_ref: String,
    /// Stable shard digest.
    pub shard_digest: String,
    /// Total sequences in the shard.
    pub sequence_count: u64,
    /// Total tokens in the shard.
    pub token_count: u64,
    /// Smallest sequence length represented by the shard.
    pub min_sequence_tokens: u32,
    /// Largest sequence length represented by the shard.
    pub max_sequence_tokens: u32,
    /// Tokenizer digest used to emit the shard.
    pub tokenizer_digest: String,
    /// Raw-source manifest schema version used to resolve lineage.
    pub source_manifest_schema_version: String,
    /// Preprocessing version inherited from raw ingestion.
    pub preprocessing_version: String,
    /// Packing-policy version used to emit the shard.
    pub packing_policy_version: String,
    /// Raw-source lineage rows represented by the shard.
    pub source_lineage: Vec<PsionTokenizedShardSourceLineage>,
}

/// Split-level aggregate metadata retained for reporting and replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedSplitManifest {
    /// Stable split name.
    pub split_name: String,
    /// High-level split kind.
    pub kind: DatasetSplitKind,
    /// Stable shard ids that make up the split.
    pub shard_ids: Vec<String>,
    /// Source-family ids represented by the split.
    pub source_family_ids: Vec<String>,
    /// Total sequences in the split.
    pub sequence_count: u64,
    /// Total tokens in the split.
    pub token_count: u64,
}

/// Replay-safe tokenized corpus manifest for the Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedCorpusManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable dataset identifier.
    pub dataset_id: String,
    /// Stable dataset version.
    pub dataset_version: String,
    /// Tokenizer-bundle schema version this corpus is bound to.
    pub tokenizer_bundle_schema_version: String,
    /// Tokenizer identifier.
    pub tokenizer_id: String,
    /// Tokenizer version.
    pub tokenizer_version: String,
    /// Tokenizer digest bound to every shard.
    pub tokenizer: TokenizerDigest,
    /// Raw-source manifest schema version for lineage.
    pub raw_source_schema_version: String,
    /// Preprocessing version inherited from raw ingestion and tokenizer build.
    pub preprocessing_version: String,
    /// Packing-policy declaration for the corpus.
    pub packing_policy: PsionTokenizedPackingPolicy,
    /// Replay and checkpoint identity contract for the corpus.
    pub replay_contract: PsionTokenizedReplayContract,
    /// Source-family bindings exposed for later reporting and ablation.
    pub source_family_bindings: Vec<PsionTokenizedSourceFamilyBinding>,
    /// Split-level aggregates.
    pub splits: Vec<PsionTokenizedSplitManifest>,
    /// Tokenized shard manifests.
    pub shards: Vec<PsionTokenizedShardManifest>,
}

impl PsionTokenizedCorpusManifest {
    /// Creates one replay-safe tokenized corpus manifest and validates it against the input contracts.
    pub fn new(
        dataset_id: impl Into<String>,
        dataset_version: impl Into<String>,
        packing_policy: PsionTokenizedPackingPolicy,
        replay_contract: PsionTokenizedReplayContract,
        mut source_family_bindings: Vec<PsionTokenizedSourceFamilyBinding>,
        mut splits: Vec<PsionTokenizedSplitManifest>,
        mut shards: Vec<PsionTokenizedShardManifest>,
        tokenizer_bundle: &PsionTokenizerArtifactBundle,
        raw_source_manifest: &PsionRawSourceManifest,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
    ) -> Result<Self, PsionTokenizedCorpusError> {
        source_family_bindings
            .sort_by(|left, right| left.source_family_id.cmp(&right.source_family_id));
        splits.sort_by(|left, right| left.split_name.cmp(&right.split_name));
        shards.sort_by(|left, right| {
            (left.split_name.as_str(), left.shard_id.as_str())
                .cmp(&(right.split_name.as_str(), right.shard_id.as_str()))
        });
        let manifest = Self {
            schema_version: String::from(PSION_TOKENIZED_CORPUS_MANIFEST_SCHEMA_VERSION),
            dataset_id: dataset_id.into(),
            dataset_version: dataset_version.into(),
            tokenizer_bundle_schema_version: tokenizer_bundle.schema_version.clone(),
            tokenizer_id: tokenizer_bundle.tokenizer_id.clone(),
            tokenizer_version: tokenizer_bundle.tokenizer_version.clone(),
            tokenizer: tokenizer_bundle.tokenizer.clone(),
            raw_source_schema_version: raw_source_manifest.schema_version.clone(),
            preprocessing_version: tokenizer_bundle.preprocessing_version.clone(),
            packing_policy,
            replay_contract,
            source_family_bindings,
            splits,
            shards,
        };
        manifest.validate_against_inputs(
            tokenizer_bundle,
            raw_source_manifest,
            lifecycle_manifest,
            exclusion_manifest,
        )?;
        Ok(manifest)
    }

    /// Validates the tokenized corpus against tokenizer, raw-source, lifecycle, and isolation truth.
    pub fn validate_against_inputs(
        &self,
        tokenizer_bundle: &PsionTokenizerArtifactBundle,
        raw_source_manifest: &PsionRawSourceManifest,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
    ) -> Result<(), PsionTokenizedCorpusError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "tokenized_corpus_manifest.schema_version",
        )?;
        if self.schema_version != PSION_TOKENIZED_CORPUS_MANIFEST_SCHEMA_VERSION {
            return Err(PsionTokenizedCorpusError::SchemaVersionMismatch {
                expected: String::from(PSION_TOKENIZED_CORPUS_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.dataset_id.as_str(),
            "tokenized_corpus_manifest.dataset_id",
        )?;
        ensure_nonempty(
            self.dataset_version.as_str(),
            "tokenized_corpus_manifest.dataset_version",
        )?;
        check_string_match(
            self.tokenizer_bundle_schema_version.as_str(),
            tokenizer_bundle.schema_version.as_str(),
            "tokenizer_bundle_schema_version",
        )?;
        check_string_match(
            self.tokenizer_id.as_str(),
            tokenizer_bundle.tokenizer_id.as_str(),
            "tokenizer_id",
        )?;
        check_string_match(
            self.tokenizer_version.as_str(),
            tokenizer_bundle.tokenizer_version.as_str(),
            "tokenizer_version",
        )?;
        if self.tokenizer != tokenizer_bundle.tokenizer {
            return Err(PsionTokenizedCorpusError::FieldMismatch {
                field: String::from("tokenizer"),
                expected: String::from("tokenizer digest must match tokenizer bundle"),
                actual: String::from("tokenizer digest drifted from tokenizer bundle"),
            });
        }
        check_string_match(
            self.raw_source_schema_version.as_str(),
            raw_source_manifest.schema_version.as_str(),
            "raw_source_schema_version",
        )?;
        check_string_match(
            self.preprocessing_version.as_str(),
            tokenizer_bundle.preprocessing_version.as_str(),
            "preprocessing_version",
        )?;
        check_string_match(
            self.preprocessing_version.as_str(),
            raw_source_manifest
                .normalization_profile
                .preprocessing_version
                .as_str(),
            "preprocessing_version",
        )?;
        self.validate_packing_policy()?;
        self.validate_replay_contract()?;

        let raw_source_map = raw_source_manifest
            .sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();

        let family_map =
            self.validate_source_family_bindings(raw_source_map.clone(), tokenizer_bundle)?;
        let shard_map = self.validate_shards(
            raw_source_map,
            raw_source_manifest.schema_version.as_str(),
            lifecycle_manifest,
            exclusion_manifest,
            tokenizer_bundle,
        )?;
        self.validate_splits(shard_map, family_map)?;
        Ok(())
    }

    fn validate_packing_policy(&self) -> Result<(), PsionTokenizedCorpusError> {
        ensure_nonempty(
            self.packing_policy.policy_id.as_str(),
            "tokenized_corpus_manifest.packing_policy.policy_id",
        )?;
        ensure_nonempty(
            self.packing_policy.policy_version.as_str(),
            "tokenized_corpus_manifest.packing_policy.policy_version",
        )?;
        if self.packing_policy.max_sequence_tokens == 0 {
            return Err(PsionTokenizedCorpusError::InvalidPackingPolicy {
                detail: String::from("max_sequence_tokens must be greater than zero"),
            });
        }
        Ok(())
    }

    fn validate_replay_contract(&self) -> Result<(), PsionTokenizedCorpusError> {
        ensure_nonempty(
            self.replay_contract.stable_dataset_identity.as_str(),
            "tokenized_corpus_manifest.replay_contract.stable_dataset_identity",
        )?;
        let expected_identity = format!("{}@{}", self.dataset_id, self.dataset_version);
        if self.replay_contract.stable_dataset_identity != expected_identity {
            return Err(PsionTokenizedCorpusError::FieldMismatch {
                field: String::from("replay_contract.stable_dataset_identity"),
                expected: expected_identity,
                actual: self.replay_contract.stable_dataset_identity.clone(),
            });
        }
        Ok(())
    }

    fn validate_source_family_bindings(
        &self,
        raw_source_map: BTreeMap<&str, &crate::PsionRawSourceRecord>,
        tokenizer_bundle: &PsionTokenizerArtifactBundle,
    ) -> Result<BTreeMap<String, BTreeSet<String>>, PsionTokenizedCorpusError> {
        if self.source_family_bindings.is_empty() {
            return Err(PsionTokenizedCorpusError::MissingField {
                field: String::from("tokenized_corpus_manifest.source_family_bindings"),
            });
        }
        let tokenizer_known_ids = tokenizer_bundle
            .admitted_sources
            .iter()
            .map(|source| source.source_id.as_str())
            .chain(
                tokenizer_bundle
                    .excluded_sources
                    .iter()
                    .map(|source| source.source_id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let mut family_map = BTreeMap::new();
        for binding in &self.source_family_bindings {
            ensure_nonempty(
                binding.source_family_id.as_str(),
                "tokenized_corpus_manifest.source_family_bindings[].source_family_id",
            )?;
            if family_map.contains_key(binding.source_family_id.as_str()) {
                return Err(PsionTokenizedCorpusError::DuplicateSourceFamilyBinding {
                    source_family_id: binding.source_family_id.clone(),
                });
            }
            if binding.source_ids.is_empty() {
                return Err(PsionTokenizedCorpusError::MissingField {
                    field: format!(
                        "tokenized_corpus_manifest.source_family_bindings.{}.source_ids",
                        binding.source_family_id
                    ),
                });
            }
            let mut source_ids = BTreeSet::new();
            for source_id in &binding.source_ids {
                ensure_nonempty(
                    source_id.as_str(),
                    "tokenized_corpus_manifest.source_family_bindings[].source_ids[]",
                )?;
                let Some(raw_source) = raw_source_map.get(source_id.as_str()) else {
                    return Err(PsionTokenizedCorpusError::UnknownSourceId {
                        source_id: source_id.clone(),
                    });
                };
                if raw_source.source_family_id != binding.source_family_id {
                    return Err(PsionTokenizedCorpusError::FieldMismatch {
                        field: format!(
                            "source_family_bindings.{}.source_ids",
                            binding.source_family_id
                        ),
                        expected: raw_source.source_family_id.clone(),
                        actual: binding.source_family_id.clone(),
                    });
                }
                if !tokenizer_known_ids.contains(source_id.as_str()) {
                    return Err(PsionTokenizedCorpusError::UnknownSourceId {
                        source_id: source_id.clone(),
                    });
                }
                if !source_ids.insert(source_id.clone()) {
                    return Err(PsionTokenizedCorpusError::DuplicateShardSourceId {
                        shard_id: format!("source_family_binding:{}", binding.source_family_id),
                        source_id: source_id.clone(),
                    });
                }
            }
            family_map.insert(binding.source_family_id.clone(), source_ids);
        }
        Ok(family_map)
    }

    fn validate_shards(
        &self,
        raw_source_map: BTreeMap<&str, &crate::PsionRawSourceRecord>,
        raw_source_schema_version: &str,
        lifecycle_manifest: &PsionSourceLifecycleManifest,
        exclusion_manifest: &PsionExclusionManifest,
        tokenizer_bundle: &PsionTokenizerArtifactBundle,
    ) -> Result<BTreeMap<String, PsionTokenizedShardUsage<'_>>, PsionTokenizedCorpusError> {
        if self.shards.is_empty() {
            return Err(PsionTokenizedCorpusError::MissingField {
                field: String::from("tokenized_corpus_manifest.shards"),
            });
        }
        let mut shard_map = BTreeMap::new();
        for shard in &self.shards {
            ensure_nonempty(
                shard.shard_id.as_str(),
                "tokenized_corpus_manifest.shards[].shard_id",
            )?;
            if shard_map.contains_key(shard.shard_id.as_str()) {
                return Err(PsionTokenizedCorpusError::DuplicateShardId {
                    shard_id: shard.shard_id.clone(),
                });
            }
            ensure_nonempty(
                shard.split_name.as_str(),
                "tokenized_corpus_manifest.shards[].split_name",
            )?;
            ensure_nonempty(
                shard.storage_ref.as_str(),
                "tokenized_corpus_manifest.shards[].storage_ref",
            )?;
            ensure_nonempty(
                shard.shard_digest.as_str(),
                "tokenized_corpus_manifest.shards[].shard_digest",
            )?;
            check_string_match(
                shard.tokenizer_digest.as_str(),
                tokenizer_bundle.tokenizer.tokenizer_digest.as_str(),
                "shard.tokenizer_digest",
            )?;
            check_string_match(
                shard.source_manifest_schema_version.as_str(),
                raw_source_schema_version,
                "shard.source_manifest_schema_version",
            )?;
            check_string_match(
                shard.preprocessing_version.as_str(),
                self.preprocessing_version.as_str(),
                "shard.preprocessing_version",
            )?;
            check_string_match(
                shard.packing_policy_version.as_str(),
                self.packing_policy.policy_version.as_str(),
                "shard.packing_policy_version",
            )?;
            if shard.sequence_count == 0 || shard.token_count == 0 {
                return Err(PsionTokenizedCorpusError::InvalidShard {
                    shard_id: shard.shard_id.clone(),
                    detail: String::from("sequence_count and token_count must both be non-zero"),
                });
            }
            if shard.min_sequence_tokens == 0
                || shard.max_sequence_tokens == 0
                || shard.min_sequence_tokens > shard.max_sequence_tokens
                || shard.max_sequence_tokens > self.packing_policy.max_sequence_tokens
            {
                return Err(PsionTokenizedCorpusError::InvalidShard {
                    shard_id: shard.shard_id.clone(),
                    detail: String::from(
                        "sequence token bounds must be ordered and within the packing policy max",
                    ),
                });
            }
            if shard.source_lineage.is_empty() {
                return Err(PsionTokenizedCorpusError::MissingField {
                    field: format!(
                        "tokenized_corpus_manifest.shards.{}.source_lineage",
                        shard.shard_id
                    ),
                });
            }
            let mut source_ids = Vec::with_capacity(shard.source_lineage.len());
            let mut source_ids_by_family = BTreeMap::new();
            let mut seen_lineage_ids = BTreeSet::new();
            for lineage in &shard.source_lineage {
                ensure_nonempty(
                    lineage.source_id.as_str(),
                    "tokenized_corpus_manifest.shards[].source_lineage[].source_id",
                )?;
                let Some(raw_source) = raw_source_map.get(lineage.source_id.as_str()) else {
                    return Err(PsionTokenizedCorpusError::UnknownSourceId {
                        source_id: lineage.source_id.clone(),
                    });
                };
                if !seen_lineage_ids.insert(lineage.source_id.clone()) {
                    return Err(PsionTokenizedCorpusError::DuplicateShardSourceId {
                        shard_id: shard.shard_id.clone(),
                        source_id: lineage.source_id.clone(),
                    });
                }
                if lineage.source_family_id != raw_source.source_family_id {
                    return Err(PsionTokenizedCorpusError::FieldMismatch {
                        field: format!("shards.{}.source_family_id", shard.shard_id),
                        expected: raw_source.source_family_id.clone(),
                        actual: lineage.source_family_id.clone(),
                    });
                }
                if lineage.source_normalized_digest != raw_source.source_normalized_digest {
                    return Err(PsionTokenizedCorpusError::FieldMismatch {
                        field: format!("shards.{}.source_normalized_digest", shard.shard_id),
                        expected: raw_source.source_normalized_digest.clone(),
                        actual: lineage.source_normalized_digest.clone(),
                    });
                }
                source_ids.push(lineage.source_id.clone());
                source_ids_by_family
                    .entry(lineage.source_family_id.clone())
                    .or_insert_with(BTreeSet::new)
                    .insert(lineage.source_id.clone());
            }
            let loader_surface = match shard.split_kind {
                DatasetSplitKind::HeldOut | DatasetSplitKind::Test => {
                    PsionLoaderSurface::BenchmarkPackage
                }
                _ => PsionLoaderSurface::ModelTraining,
            };
            exclusion_manifest
                .assert_source_ids_allowed(
                    lifecycle_manifest,
                    loader_surface,
                    source_ids.as_slice(),
                )
                .map_err(|error| PsionTokenizedCorpusError::Isolation { error })?;
            shard_map.insert(
                shard.shard_id.clone(),
                PsionTokenizedShardUsage {
                    manifest: shard,
                    source_ids_by_family,
                },
            );
        }
        Ok(shard_map)
    }

    fn validate_splits(
        &self,
        shard_map: BTreeMap<String, PsionTokenizedShardUsage<'_>>,
        family_bindings: BTreeMap<String, BTreeSet<String>>,
    ) -> Result<(), PsionTokenizedCorpusError> {
        if self.splits.is_empty() {
            return Err(PsionTokenizedCorpusError::MissingField {
                field: String::from("tokenized_corpus_manifest.splits"),
            });
        }
        let mut seen_split_names = BTreeSet::new();
        let mut actual_sources_by_family = BTreeMap::new();
        for split in &self.splits {
            ensure_nonempty(
                split.split_name.as_str(),
                "tokenized_corpus_manifest.splits[].split_name",
            )?;
            if !seen_split_names.insert(split.split_name.clone()) {
                return Err(PsionTokenizedCorpusError::DuplicateSplitName {
                    split_name: split.split_name.clone(),
                });
            }
            if split.shard_ids.is_empty() {
                return Err(PsionTokenizedCorpusError::MissingField {
                    field: format!(
                        "tokenized_corpus_manifest.splits.{}.shard_ids",
                        split.split_name
                    ),
                });
            }
            if split.source_family_ids.is_empty() {
                return Err(PsionTokenizedCorpusError::MissingField {
                    field: format!(
                        "tokenized_corpus_manifest.splits.{}.source_family_ids",
                        split.split_name
                    ),
                });
            }
            let mut split_shard_ids = BTreeSet::new();
            let mut actual_sequence_count = 0_u64;
            let mut actual_token_count = 0_u64;
            let mut actual_family_ids = BTreeSet::new();
            for shard_id in &split.shard_ids {
                let Some(shard_usage) = shard_map.get(shard_id.as_str()) else {
                    return Err(PsionTokenizedCorpusError::UnknownShardId {
                        shard_id: shard_id.clone(),
                    });
                };
                let shard = shard_usage.manifest;
                if shard.split_name != split.split_name || shard.split_kind != split.kind {
                    return Err(PsionTokenizedCorpusError::SplitShardMismatch {
                        split_name: split.split_name.clone(),
                        shard_id: shard_id.clone(),
                    });
                }
                if !split_shard_ids.insert(shard_id.clone()) {
                    return Err(PsionTokenizedCorpusError::DuplicateShardId {
                        shard_id: shard_id.clone(),
                    });
                }
                actual_sequence_count = actual_sequence_count.saturating_add(shard.sequence_count);
                actual_token_count = actual_token_count.saturating_add(shard.token_count);
                for (family_id, source_ids) in &shard_usage.source_ids_by_family {
                    actual_family_ids.insert(family_id.clone());
                    actual_sources_by_family
                        .entry(family_id.clone())
                        .or_insert_with(BTreeSet::new)
                        .extend(source_ids.iter().cloned());
                }
            }
            if split.sequence_count != actual_sequence_count
                || split.token_count != actual_token_count
            {
                return Err(PsionTokenizedCorpusError::SplitAggregateMismatch {
                    split_name: split.split_name.clone(),
                });
            }
            let listed_family_ids = split
                .source_family_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let actual_family_ids_str = actual_family_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if listed_family_ids != actual_family_ids_str {
                return Err(PsionTokenizedCorpusError::SplitFamilyMismatch {
                    split_name: split.split_name.clone(),
                });
            }
            for family_id in &actual_family_ids {
                let Some(bound_sources) = family_bindings.get(family_id.as_str()) else {
                    return Err(PsionTokenizedCorpusError::UnknownSourceFamilyId {
                        source_family_id: family_id.clone(),
                    });
                };
                if bound_sources.is_empty() {
                    return Err(PsionTokenizedCorpusError::UnknownSourceFamilyId {
                        source_family_id: family_id.clone(),
                    });
                }
            }
        }
        if family_bindings != actual_sources_by_family {
            return Err(PsionTokenizedCorpusError::SourceFamilyCoverageMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct PsionTokenizedShardUsage<'a> {
    manifest: &'a PsionTokenizedShardManifest,
    source_ids_by_family: BTreeMap<String, BTreeSet<String>>,
}

/// Error returned by the Psion tokenized corpus contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionTokenizedCorpusError {
    /// One required field was missing or empty.
    #[error("Psion tokenized corpus field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version did not match the expected contract.
    #[error("Psion tokenized corpus expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One field drifted from the tokenizer or raw-source inputs.
    #[error("Psion tokenized corpus field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// The packing policy was internally inconsistent.
    #[error("Psion tokenized packing policy is invalid: {detail}")]
    InvalidPackingPolicy {
        /// Machine-readable detail.
        detail: String,
    },
    /// One shard was internally inconsistent.
    #[error("Psion tokenized shard `{shard_id}` is invalid: {detail}")]
    InvalidShard {
        /// Shard identifier.
        shard_id: String,
        /// Machine-readable detail.
        detail: String,
    },
    /// One source id was unknown to the raw-source manifest.
    #[error("Psion tokenized corpus does not know source `{source_id}`")]
    UnknownSourceId {
        /// Unknown source identifier.
        source_id: String,
    },
    /// One shard id was unknown to the split aggregates.
    #[error("Psion tokenized corpus does not know shard `{shard_id}`")]
    UnknownShardId {
        /// Unknown shard identifier.
        shard_id: String,
    },
    /// One source family id was unknown to the family bindings.
    #[error("Psion tokenized corpus does not know source family `{source_family_id}`")]
    UnknownSourceFamilyId {
        /// Unknown source family identifier.
        source_family_id: String,
    },
    /// One source-family binding was repeated.
    #[error("Psion tokenized corpus repeated source family `{source_family_id}`")]
    DuplicateSourceFamilyBinding {
        /// Repeated source-family identifier.
        source_family_id: String,
    },
    /// One shard id was repeated.
    #[error("Psion tokenized corpus repeated shard `{shard_id}`")]
    DuplicateShardId {
        /// Repeated shard identifier.
        shard_id: String,
    },
    /// One split name was repeated.
    #[error("Psion tokenized corpus repeated split `{split_name}`")]
    DuplicateSplitName {
        /// Repeated split name.
        split_name: String,
    },
    /// One shard repeated a source id.
    #[error("Psion tokenized shard `{shard_id}` repeated source `{source_id}`")]
    DuplicateShardSourceId {
        /// Shard identifier.
        shard_id: String,
        /// Repeated source identifier.
        source_id: String,
    },
    /// Admitted source families did not cover the families represented by the shards.
    #[error("Psion tokenized corpus family bindings must cover exactly the source families used by the shards")]
    SourceFamilyCoverageMismatch,
    /// One split referenced a shard that did not belong to it.
    #[error("Psion tokenized split `{split_name}` does not match shard `{shard_id}`")]
    SplitShardMismatch {
        /// Split name.
        split_name: String,
        /// Shard identifier.
        shard_id: String,
    },
    /// One split aggregate drifted from its shards.
    #[error("Psion tokenized split `{split_name}` aggregate counts drifted from its shards")]
    SplitAggregateMismatch {
        /// Split name.
        split_name: String,
    },
    /// One split family list drifted from its shards.
    #[error("Psion tokenized split `{split_name}` family ids drifted from its shards")]
    SplitFamilyMismatch {
        /// Split name.
        split_name: String,
    },
    /// The held-out isolation contract rejected a shard lineage set for its split surface.
    #[error("Psion tokenized corpus violates the held-out isolation contract: {error}")]
    Isolation {
        /// Upstream isolation error.
        error: crate::PsionBenchmarkIsolationError,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionTokenizedCorpusError> {
    if value.trim().is_empty() {
        return Err(PsionTokenizedCorpusError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionTokenizedCorpusError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionTokenizedCorpusError::FieldMismatch {
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
    use crate::{
        PsionExclusionManifest, PsionRawSourceManifest, PsionSourceLifecycleManifest,
        PsionTokenizerArtifactBundle,
    };

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

    fn tokenizer_bundle() -> PsionTokenizerArtifactBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json"
        ))
        .expect("tokenizer bundle should parse")
    }

    fn tokenized_corpus_manifest() -> PsionTokenizedCorpusManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"
        ))
        .expect("tokenized corpus manifest should parse")
    }

    #[test]
    fn tokenized_corpus_manifest_validates_for_replay_and_lineage() {
        let manifest = tokenized_corpus_manifest();
        manifest
            .validate_against_inputs(
                &tokenizer_bundle(),
                &raw_source_manifest(),
                &lifecycle_manifest(),
                &exclusion_manifest(),
            )
            .expect("tokenized corpus manifest should validate");
        assert_eq!(
            manifest.replay_contract.stable_dataset_identity,
            "psion_corpus_tokenized@v1"
        );
        assert!(manifest
            .shards
            .iter()
            .any(|shard| shard.split_kind == DatasetSplitKind::HeldOut));
    }

    #[test]
    fn tokenizer_only_source_cannot_enter_training_shard() {
        let mut manifest = tokenized_corpus_manifest();
        manifest.shards[0]
            .source_lineage
            .push(PsionTokenizedShardSourceLineage {
                source_id: String::from("vendor_manual_private_scan_v1"),
                source_family_id: String::from("historical_vendor_manuals"),
                source_normalized_digest: String::from(
                    "sha256:vendor_manual_private_scan_v1_normalized_source_digest_v1",
                ),
            });
        let error = manifest
            .validate_against_inputs(
                &tokenizer_bundle(),
                &raw_source_manifest(),
                &lifecycle_manifest(),
                &exclusion_manifest(),
            )
            .expect_err("tokenizer-only source should be rejected from training shard");
        assert!(matches!(error, PsionTokenizedCorpusError::Isolation { .. }));
    }

    #[test]
    fn replay_contract_identity_must_match_dataset_identity() {
        let mut manifest = tokenized_corpus_manifest();
        manifest.replay_contract.stable_dataset_identity = String::from("wrong@v1");
        let error = manifest
            .validate_against_inputs(
                &tokenizer_bundle(),
                &raw_source_manifest(),
                &lifecycle_manifest(),
                &exclusion_manifest(),
            )
            .expect_err("dataset identity drift should be rejected");
        assert!(matches!(
            error,
            PsionTokenizedCorpusError::FieldMismatch { .. }
        ));
    }
}
