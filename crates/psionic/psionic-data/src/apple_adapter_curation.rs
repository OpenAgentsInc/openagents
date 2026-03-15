use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AppleAdapterDatasetContract, DatasetKey};

/// Split identities used by curated Apple adapter corpora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterCuratedSplit {
    /// Main training split.
    Train,
    /// Held-out validation split.
    HeldOut,
    /// Benchmark split used for base-vs-adapter acceptance.
    Benchmark,
}

impl AppleAdapterCuratedSplit {
    /// Stable label for manifests and errors.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::HeldOut => "held_out",
            Self::Benchmark => "benchmark",
        }
    }
}

/// Stable task families for curated Apple adapter training corpora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterCorpusTaskFamily {
    /// Architecture questions answered from stable Psionic docs.
    ArchitectureQa,
    /// Owner-split or crate-boundary classification.
    OwnershipBoundaryClassification,
    /// Implemented-vs-planned truthfulness checks.
    ImplementedVsPlannedTruthfulness,
    /// Operator and CLI workflow explanation.
    OperatorWorkflowExplanation,
    /// Structured summaries or manifests that must match a schema.
    StructuredSummaryConformance,
    /// Tool-first routing for lookup-style workflows.
    ToolCallingLookupRouting,
    /// Refusal or correction behavior for overclaims or stale evidence.
    NegativeRefusalCorrection,
}

/// Expected assistant posture for one curated sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterCorpusExpectedBehavior {
    /// Plain direct answer.
    DirectAnswer,
    /// Structured JSON or schema-constrained answer.
    StructuredAnswer,
    /// Tool-first routing or tool-usage recommendation.
    ToolLookupRouting,
    /// Correction of a false or overclaimed premise.
    Correction,
    /// Refusal pending retrieval or missing evidence.
    Refusal,
}

/// Review posture for one curated sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterCorpusReviewStatus {
    /// Sample was reviewed and accepted by a maintainer.
    MaintainerReviewed,
    /// Sample is synthetic or draft and must not ship as a real-run corpus row.
    SeededCandidate,
}

/// Source families admitted by the curated Apple corpus contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterCorpusSourceKind {
    /// Canonical doc under `docs/` or `crates/psionic/docs/`.
    CanonicalDoc,
    /// Stable operator runbook or control-plane documentation.
    OperatorRunbook,
    /// Stable code surface where the contract is intentionally explicit.
    StableCodeSurface,
    /// Audit or review document used as a reviewed synthesis source.
    Audit,
}

/// One source entry used by a curated Apple adapter corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterCorpusSource {
    /// Stable source identifier used by sample annotations.
    pub source_id: String,
    /// Repo-relative path for the reviewed source.
    pub path: String,
    /// High-level source family.
    pub source_kind: AppleAdapterCorpusSourceKind,
    /// Short explanation of why the source was included.
    pub summary: String,
}

/// One per-sample annotation for a curated Apple adapter corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterCuratedSampleAnnotation {
    /// Split containing the sample.
    pub split: AppleAdapterCuratedSplit,
    /// Split-local sample id emitted by the imported Apple dataset contract.
    pub sample_id: String,
    /// Reviewed task family for the sample.
    pub task_family: AppleAdapterCorpusTaskFamily,
    /// Expected model posture for the sample.
    pub expected_behavior: AppleAdapterCorpusExpectedBehavior,
    /// Review state for the sample.
    pub review_status: AppleAdapterCorpusReviewStatus,
    /// Source ids that justify the sample.
    pub source_ids: Vec<String>,
    /// Optional maintainer note for later corpus iteration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Curated corpus manifest for one Apple adapter training target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterCuratedCorpusManifest {
    /// Stable target identifier.
    pub target_id: String,
    /// Human-readable target name.
    pub target_title: String,
    /// Versioned dataset identity shared by the curated splits.
    pub dataset: DatasetKey,
    /// Stable scope statement that explains what the adapter should learn.
    pub target_scope: String,
    /// Reviewed source inventory.
    pub source_inventory: Vec<AppleAdapterCorpusSource>,
    /// Per-sample annotations across train, held-out, and benchmark splits.
    pub samples: Vec<AppleAdapterCuratedSampleAnnotation>,
}

impl AppleAdapterCuratedCorpusManifest {
    /// Validates the curation manifest itself.
    pub fn validate(&self) -> Result<(), AppleAdapterCuratedCorpusError> {
        if self.target_id.trim().is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingTargetId);
        }
        if self.target_title.trim().is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingTargetTitle);
        }
        if self.dataset.dataset_ref.trim().is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingDatasetRef);
        }
        if self.dataset.version.trim().is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingDatasetVersion);
        }
        if self.target_scope.trim().is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingTargetScope);
        }
        if self.source_inventory.is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingSourceInventory);
        }
        if self.samples.is_empty() {
            return Err(AppleAdapterCuratedCorpusError::MissingSampleAnnotations);
        }

        let mut source_ids = BTreeSet::new();
        for source in &self.source_inventory {
            if source.source_id.trim().is_empty() {
                return Err(AppleAdapterCuratedCorpusError::MissingSourceId);
            }
            if !source_ids.insert(source.source_id.clone()) {
                return Err(AppleAdapterCuratedCorpusError::DuplicateSourceId {
                    source_id: source.source_id.clone(),
                });
            }
            if source.path.trim().is_empty() {
                return Err(AppleAdapterCuratedCorpusError::MissingSourcePath {
                    source_id: source.source_id.clone(),
                });
            }
            if source.summary.trim().is_empty() {
                return Err(AppleAdapterCuratedCorpusError::MissingSourceSummary {
                    source_id: source.source_id.clone(),
                });
            }
        }

        let mut sample_keys = BTreeSet::new();
        for sample in &self.samples {
            if sample.sample_id.trim().is_empty() {
                return Err(AppleAdapterCuratedCorpusError::MissingSampleId {
                    split: sample.split,
                });
            }
            if !sample_keys.insert((sample.split, sample.sample_id.clone())) {
                return Err(AppleAdapterCuratedCorpusError::DuplicateSampleAnnotation {
                    split: sample.split,
                    sample_id: sample.sample_id.clone(),
                });
            }
            if sample.source_ids.is_empty() {
                return Err(AppleAdapterCuratedCorpusError::MissingSampleSources {
                    split: sample.split,
                    sample_id: sample.sample_id.clone(),
                });
            }
            if sample.review_status != AppleAdapterCorpusReviewStatus::MaintainerReviewed {
                return Err(AppleAdapterCuratedCorpusError::UnreviewedSample {
                    split: sample.split,
                    sample_id: sample.sample_id.clone(),
                });
            }
            for source_id in &sample.source_ids {
                if !source_ids.contains(source_id) {
                    return Err(AppleAdapterCuratedCorpusError::UnknownSampleSource {
                        split: sample.split,
                        sample_id: sample.sample_id.clone(),
                        source_id: source_id.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validates the manifest against concrete train, held-out, and benchmark splits.
    pub fn validate_against_splits(
        &self,
        train: &AppleAdapterDatasetContract,
        held_out: &AppleAdapterDatasetContract,
        benchmark: &AppleAdapterDatasetContract,
    ) -> Result<(), AppleAdapterCuratedCorpusError> {
        self.validate()?;

        let annotated = self
            .samples
            .iter()
            .map(|sample| ((sample.split, sample.sample_id.clone()), sample))
            .collect::<BTreeMap<_, _>>();

        let datasets = [
            (AppleAdapterCuratedSplit::Train, train),
            (AppleAdapterCuratedSplit::HeldOut, held_out),
            (AppleAdapterCuratedSplit::Benchmark, benchmark),
        ];

        for (split, dataset) in datasets {
            for sample in &dataset.samples {
                if !annotated.contains_key(&(split, sample.sample_id.clone())) {
                    return Err(AppleAdapterCuratedCorpusError::UnannotatedDatasetSample {
                        split,
                        sample_id: sample.sample_id.clone(),
                    });
                }
            }
        }

        let split_maps = [
            (
                AppleAdapterCuratedSplit::Train,
                train
                    .samples
                    .iter()
                    .map(|sample| (sample.sample_id.clone(), sample.stable_digest.clone()))
                    .collect::<BTreeMap<_, _>>(),
            ),
            (
                AppleAdapterCuratedSplit::HeldOut,
                held_out
                    .samples
                    .iter()
                    .map(|sample| (sample.sample_id.clone(), sample.stable_digest.clone()))
                    .collect::<BTreeMap<_, _>>(),
            ),
            (
                AppleAdapterCuratedSplit::Benchmark,
                benchmark
                    .samples
                    .iter()
                    .map(|sample| (sample.sample_id.clone(), sample.stable_digest.clone()))
                    .collect::<BTreeMap<_, _>>(),
            ),
        ];

        for sample in &self.samples {
            let present = split_maps
                .iter()
                .find(|(split, _)| *split == sample.split)
                .and_then(|(_, samples)| samples.get(sample.sample_id.as_str()));
            if present.is_none() {
                return Err(AppleAdapterCuratedCorpusError::UnknownAnnotatedSample {
                    split: sample.split,
                    sample_id: sample.sample_id.clone(),
                });
            }
        }

        let digests = split_maps
            .iter()
            .flat_map(|(split, samples)| {
                samples.iter().map(|(sample_id, stable_digest)| {
                    (*split, sample_id.clone(), stable_digest.clone())
                })
            })
            .collect::<Vec<_>>();
        for (index, (left_split, left_sample_id, left_digest)) in digests.iter().enumerate() {
            for (right_split, right_sample_id, right_digest) in digests.iter().skip(index + 1) {
                if left_split != right_split && left_digest == right_digest {
                    return Err(AppleAdapterCuratedCorpusError::StableDigestLeakage {
                        left_split: *left_split,
                        left_sample_id: left_sample_id.clone(),
                        right_split: *right_split,
                        right_sample_id: right_sample_id.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Curated Apple adapter corpus validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterCuratedCorpusError {
    /// Missing target id.
    #[error("Apple adapter curated corpus is missing `target_id`")]
    MissingTargetId,
    /// Missing target title.
    #[error("Apple adapter curated corpus is missing `target_title`")]
    MissingTargetTitle,
    /// Missing dataset ref.
    #[error("Apple adapter curated corpus is missing `dataset.dataset_ref`")]
    MissingDatasetRef,
    /// Missing dataset version.
    #[error("Apple adapter curated corpus is missing `dataset.version`")]
    MissingDatasetVersion,
    /// Missing target scope statement.
    #[error("Apple adapter curated corpus is missing `target_scope`")]
    MissingTargetScope,
    /// Missing source inventory.
    #[error("Apple adapter curated corpus requires at least one source inventory entry")]
    MissingSourceInventory,
    /// Missing sample annotations.
    #[error("Apple adapter curated corpus requires at least one sample annotation")]
    MissingSampleAnnotations,
    /// One source omitted its id.
    #[error("Apple adapter curated corpus source inventory is missing `source_id`")]
    MissingSourceId,
    /// One source id was duplicated.
    #[error("Apple adapter curated corpus repeats source id `{source_id}`")]
    DuplicateSourceId { source_id: String },
    /// One source omitted its path.
    #[error("Apple adapter curated corpus source `{source_id}` is missing `path`")]
    MissingSourcePath { source_id: String },
    /// One source omitted its summary.
    #[error("Apple adapter curated corpus source `{source_id}` is missing `summary`")]
    MissingSourceSummary { source_id: String },
    /// One sample omitted its split-local id.
    #[error(
        "Apple adapter curated corpus split `{split}` contains a sample annotation with an empty `sample_id`"
    )]
    MissingSampleId { split: AppleAdapterCuratedSplit },
    /// One split-local sample annotation was duplicated.
    #[error("Apple adapter curated corpus repeats sample `{sample_id}` in split `{split}`")]
    DuplicateSampleAnnotation {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
    },
    /// One annotated sample omitted source ids.
    #[error(
        "Apple adapter curated corpus sample `{sample_id}` in split `{split}` is missing `source_ids`"
    )]
    MissingSampleSources {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
    },
    /// One sample referred to an unknown source entry.
    #[error(
        "Apple adapter curated corpus sample `{sample_id}` in split `{split}` references unknown source `{source_id}`"
    )]
    UnknownSampleSource {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
        source_id: String,
    },
    /// One sample is still draft quality.
    #[error(
        "Apple adapter curated corpus sample `{sample_id}` in split `{split}` is not maintainer-reviewed"
    )]
    UnreviewedSample {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
    },
    /// One dataset sample was present but not annotated.
    #[error(
        "Apple adapter curated corpus dataset sample `{sample_id}` in split `{split}` is missing an annotation"
    )]
    UnannotatedDatasetSample {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
    },
    /// One annotation pointed at a sample that is not in the split dataset.
    #[error(
        "Apple adapter curated corpus annotation `{sample_id}` in split `{split}` does not exist in the dataset"
    )]
    UnknownAnnotatedSample {
        split: AppleAdapterCuratedSplit,
        sample_id: String,
    },
    /// Two splits carry the same normalized sample content.
    #[error(
        "Apple adapter curated corpus has split leakage between `{left_split}:{left_sample_id}` and `{right_split}:{right_sample_id}`"
    )]
    StableDigestLeakage {
        left_split: AppleAdapterCuratedSplit,
        left_sample_id: String,
        right_split: AppleAdapterCuratedSplit,
        right_sample_id: String,
    },
}

impl std::fmt::Display for AppleAdapterCuratedSplit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppleAdapterDatasetMetadata, TokenizerDigest, TokenizerFamily};

    fn sample_metadata() -> AppleAdapterDatasetMetadata {
        AppleAdapterDatasetMetadata::new(
            TokenizerDigest::new(
                TokenizerFamily::SentencePiece,
                "apple-tokenizer-digest-v1",
                32_768,
            )
            .with_special_tokens_digest("apple-special-tokens-v1")
            .with_template_digest("apple-template-v1"),
            "apple-prompt-shaping-v1",
        )
        .with_default_instruction("A conversation between a user and a helpful assistant.")
        .with_locale("en-US")
    }

    fn load_split(split: AppleAdapterCuratedSplit) -> AppleAdapterDatasetContract {
        let input = match split {
            AppleAdapterCuratedSplit::Train => include_str!(
                "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/train.jsonl"
            ),
            AppleAdapterCuratedSplit::HeldOut => include_str!(
                "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/held_out.jsonl"
            ),
            AppleAdapterCuratedSplit::Benchmark => include_str!(
                "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/benchmark.jsonl"
            ),
        };
        AppleAdapterDatasetContract::from_jsonl_str(input, sample_metadata())
            .expect("fixture should import")
    }

    fn manifest() -> AppleAdapterCuratedCorpusManifest {
        serde_json::from_str(include_str!(
            "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/corpus_manifest.json"
        ))
        .expect("manifest should parse")
    }

    #[test]
    fn architecture_explainer_corpus_manifest_validates_against_split_fixtures() {
        let manifest = manifest();
        manifest
            .validate_against_splits(
                &load_split(AppleAdapterCuratedSplit::Train),
                &load_split(AppleAdapterCuratedSplit::HeldOut),
                &load_split(AppleAdapterCuratedSplit::Benchmark),
            )
            .expect("curated corpus should validate");
        assert!(
            manifest
                .samples
                .iter()
                .any(|sample| sample.task_family == AppleAdapterCorpusTaskFamily::ArchitectureQa)
        );
        assert!(manifest.samples.iter().any(|sample| {
            sample.task_family == AppleAdapterCorpusTaskFamily::StructuredSummaryConformance
        }));
    }

    #[test]
    fn architecture_explainer_corpus_rejects_split_leakage() {
        let manifest = manifest();
        let train = load_split(AppleAdapterCuratedSplit::Train);
        let mut benchmark = load_split(AppleAdapterCuratedSplit::Benchmark);
        benchmark.samples[0].stable_digest = train.samples[0].stable_digest.clone();
        let error = manifest
            .validate_against_splits(
                &train,
                &load_split(AppleAdapterCuratedSplit::HeldOut),
                &benchmark,
            )
            .expect_err("duplicate split content should be refused");
        assert!(matches!(
            error,
            AppleAdapterCuratedCorpusError::StableDigestLeakage { .. }
        ));
    }

    #[test]
    fn architecture_explainer_corpus_contains_negative_and_refusal_coverage() {
        let manifest = manifest();
        let negative_samples = manifest
            .samples
            .iter()
            .filter(|sample| {
                sample.task_family == AppleAdapterCorpusTaskFamily::NegativeRefusalCorrection
            })
            .collect::<Vec<_>>();
        let covered_splits = negative_samples
            .iter()
            .map(|sample| sample.split)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            covered_splits,
            BTreeSet::from([
                AppleAdapterCuratedSplit::Train,
                AppleAdapterCuratedSplit::HeldOut,
                AppleAdapterCuratedSplit::Benchmark,
            ])
        );
        assert!(negative_samples.iter().any(|sample| {
            sample.expected_behavior == AppleAdapterCorpusExpectedBehavior::Correction
        }));
        assert!(negative_samples.iter().any(|sample| {
            sample.expected_behavior == AppleAdapterCorpusExpectedBehavior::Refusal
        }));
    }
}
