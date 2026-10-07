use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PsionAdmissionReviewDecision, PsionCorpusAdmissionManifest, PsionSourceRightsPosture};

/// Stable schema version for the first Psion source-lifecycle contract.
pub const PSION_SOURCE_LIFECYCLE_SCHEMA_VERSION: &str = "psion.source_lifecycle.v1";
/// Stable schema version for the first Psion artifact-lineage contract.
pub const PSION_ARTIFACT_LINEAGE_SCHEMA_VERSION: &str = "psion.artifact_lineage.v1";
/// Stable schema version for the first Psion impact-analysis receipt.
pub const PSION_SOURCE_IMPACT_ANALYSIS_SCHEMA_VERSION: &str = "psion.source_impact_analysis.v1";

/// Current lifecycle state for one reviewed Psion source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSourceLifecycleState {
    /// Fully admitted for the declared training or serving posture.
    Admitted,
    /// Admitted only in a narrowed posture such as tokenizer-only or internal-only use.
    Restricted,
    /// Retained only for held-out or benchmark construction.
    EvaluationOnly,
    /// Previously admitted, but now withdrawn pending removal, retraining, or depublication review.
    Withdrawn,
    /// Rejected from the lane.
    Rejected,
}

impl PsionSourceLifecycleState {
    /// Returns whether a lifecycle change is explicitly allowed by the first Psion policy.
    #[must_use]
    pub const fn allows_transition(self, next: Self) -> bool {
        match self {
            Self::Admitted => matches!(
                next,
                Self::Admitted | Self::Restricted | Self::EvaluationOnly | Self::Withdrawn
            ),
            Self::Restricted => matches!(
                next,
                Self::Restricted | Self::Admitted | Self::EvaluationOnly | Self::Withdrawn
            ),
            Self::EvaluationOnly => {
                matches!(
                    next,
                    Self::EvaluationOnly | Self::Restricted | Self::Withdrawn
                )
            }
            Self::Withdrawn => matches!(
                next,
                Self::Withdrawn | Self::Restricted | Self::EvaluationOnly | Self::Rejected
            ),
            Self::Rejected => matches!(next, Self::Rejected | Self::Restricted | Self::Admitted),
        }
    }
}

/// Trigger that changed one source's usable posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionLifecycleChangeTrigger {
    /// Rights posture changed or was revoked.
    RightsChangedOrRevoked,
    /// Provenance was invalidated or corrected.
    ProvenanceInvalidated,
    /// Stable digest or content identity no longer matched.
    ContentDigestMismatch,
    /// Benchmark or held-out contamination was discovered.
    BenchmarkContaminationDiscovered,
    /// The source was narrowed to a restricted posture.
    ReclassifiedToRestrictedUse,
    /// The source was narrowed to evaluation-only use.
    ReclassifiedToEvaluationOnly,
    /// A prior review or classification was corrected.
    ReviewMisclassificationCorrected,
    /// The source was withdrawn or replaced canonically.
    SourceRetractedOrCanonicallyReplaced,
}

/// Required downstream action after one source changes state or posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionLifecycleImpactAction {
    /// Re-ingest or restage the raw source before further use.
    ReingestRawSource,
    /// Rebuild tokenizer artifacts exposed to the source.
    RetokenizeAffectedArtifacts,
    /// Rebuild tokenized corpora that depend on the source.
    RebuildTokenizedCorpora,
    /// Review or rerun SFT and checkpoint artifacts that depend on the source.
    RetrainingReview,
    /// Invalidate or rebuild benchmark packages.
    BenchmarkInvalidationReview,
    /// Review served capabilities that may have relied on the source.
    CapabilityMatrixReview,
    /// Review whether previously published or served claims must be narrowed or withdrawn.
    DepublicationReview,
}

/// One source's current lifecycle state and current rights posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceLifecycleRecord {
    /// Stable source identifier from the admission manifest.
    pub source_id: String,
    /// Lifecycle state currently in force.
    pub lifecycle_state: PsionSourceLifecycleState,
    /// Rights posture recorded at initial admission time.
    pub admission_rights_posture: PsionSourceRightsPosture,
    /// Current effective rights posture after any later changes.
    pub current_rights_posture: PsionSourceRightsPosture,
    /// Stable event identifier for the last transition.
    pub last_transition_event_id: String,
    /// Trigger that produced the current lifecycle state.
    pub last_transition_trigger: PsionLifecycleChangeTrigger,
    /// Short rationale for the current state.
    pub rationale: String,
}

/// Versioned lifecycle manifest for reviewed Psion sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceLifecycleManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Bound admission policy identifier.
    pub policy_id: String,
    /// Bound admission policy version.
    pub policy_version: String,
    /// Bound admission schema version.
    pub admission_schema_version: String,
    /// Current lifecycle records.
    pub sources: Vec<PsionSourceLifecycleRecord>,
}

impl PsionSourceLifecycleManifest {
    /// Validates the lifecycle manifest against the reviewed-source admission manifest.
    pub fn validate_against_admission(
        &self,
        admission: &PsionCorpusAdmissionManifest,
    ) -> Result<(), PsionSourceLifecycleError> {
        if self.schema_version.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingLifecycleSchemaVersion);
        }
        if self.policy_id.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingLifecyclePolicyId);
        }
        if self.policy_version.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingLifecyclePolicyVersion);
        }
        if self.admission_schema_version.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingAdmissionSchemaVersion);
        }
        if self.policy_id != admission.policy_id {
            return Err(PsionSourceLifecycleError::PolicyIdMismatch {
                expected: admission.policy_id.clone(),
                actual: self.policy_id.clone(),
            });
        }
        if self.policy_version != admission.policy_version {
            return Err(PsionSourceLifecycleError::PolicyVersionMismatch {
                expected: admission.policy_version.clone(),
                actual: self.policy_version.clone(),
            });
        }
        if self.admission_schema_version != admission.schema_version {
            return Err(PsionSourceLifecycleError::AdmissionSchemaVersionMismatch {
                expected: admission.schema_version.clone(),
                actual: self.admission_schema_version.clone(),
            });
        }
        if self.sources.is_empty() {
            return Err(PsionSourceLifecycleError::MissingLifecycleRecords);
        }

        let mut lifecycle_source_ids = BTreeSet::new();
        for record in &self.sources {
            if record.source_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingSourceId);
            }
            if !lifecycle_source_ids.insert(record.source_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateSourceId {
                    source_id: record.source_id.clone(),
                });
            }
            if record.last_transition_event_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingTransitionEventId {
                    source_id: record.source_id.clone(),
                });
            }
            if record.rationale.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingLifecycleRationale {
                    source_id: record.source_id.clone(),
                });
            }

            let Some(admission_record) = admission
                .sources
                .iter()
                .find(|source| source.source_id == record.source_id)
            else {
                return Err(PsionSourceLifecycleError::UnknownAdmissionSource {
                    source_id: record.source_id.clone(),
                });
            };

            if admission_record.rights_posture != record.admission_rights_posture {
                return Err(PsionSourceLifecycleError::AdmissionRightsPostureMismatch {
                    source_id: record.source_id.clone(),
                    expected: admission_record.rights_posture,
                    actual: record.admission_rights_posture,
                });
            }

            match record.lifecycle_state {
                PsionSourceLifecycleState::Admitted => {
                    if matches!(
                        record.current_rights_posture,
                        PsionSourceRightsPosture::EvaluationOnly
                            | PsionSourceRightsPosture::TokenizerOnly
                            | PsionSourceRightsPosture::Rejected
                    ) {
                        return Err(PsionSourceLifecycleError::AdmittedStateUsesRestrictedOrRejectedPosture {
                            source_id: record.source_id.clone(),
                            rights_posture: record.current_rights_posture,
                        });
                    }
                    if admission_record.review_decision == PsionAdmissionReviewDecision::Rejected {
                        return Err(
                            PsionSourceLifecycleError::RejectedAdmissionCannotBeAdmitted {
                                source_id: record.source_id.clone(),
                            },
                        );
                    }
                }
                PsionSourceLifecycleState::Restricted => {
                    if matches!(
                        record.current_rights_posture,
                        PsionSourceRightsPosture::EvaluationOnly
                            | PsionSourceRightsPosture::Rejected
                    ) {
                        return Err(PsionSourceLifecycleError::RestrictedStateRequiresNonEvalNonRejectedPosture {
                            source_id: record.source_id.clone(),
                            rights_posture: record.current_rights_posture,
                        });
                    }
                }
                PsionSourceLifecycleState::EvaluationOnly => {
                    if record.current_rights_posture != PsionSourceRightsPosture::EvaluationOnly {
                        return Err(PsionSourceLifecycleError::EvaluationOnlyStateRequiresEvaluationPosture {
                            source_id: record.source_id.clone(),
                            rights_posture: record.current_rights_posture,
                        });
                    }
                }
                PsionSourceLifecycleState::Withdrawn => {
                    if record.current_rights_posture
                        == PsionSourceRightsPosture::InternalTrainingAndInternalServing
                    {
                        return Err(
                            PsionSourceLifecycleError::WithdrawnStateCannotKeepServingPosture {
                                source_id: record.source_id.clone(),
                            },
                        );
                    }
                }
                PsionSourceLifecycleState::Rejected => {
                    if record.current_rights_posture != PsionSourceRightsPosture::Rejected {
                        return Err(
                            PsionSourceLifecycleError::RejectedStateRequiresRejectedPosture {
                                source_id: record.source_id.clone(),
                                rights_posture: record.current_rights_posture,
                            },
                        );
                    }
                }
            }
        }

        for source in &admission.sources {
            if !lifecycle_source_ids.contains(source.source_id.as_str()) {
                return Err(
                    PsionSourceLifecycleError::MissingLifecycleRecordForAdmissionSource {
                        source_id: source.source_id.clone(),
                    },
                );
            }
        }

        Ok(())
    }

    /// Returns the current lifecycle record for one source when present.
    #[must_use]
    pub fn source_record(&self, source_id: &str) -> Option<&PsionSourceLifecycleRecord> {
        self.sources
            .iter()
            .find(|record| record.source_id == source_id)
    }
}

/// One tokenizer artifact with explicit source lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerArtifactLineage {
    /// Stable tokenizer artifact id.
    pub artifact_id: String,
    /// Stable tokenizer artifact digest.
    pub artifact_digest: String,
    /// Source ids exposed to tokenizer construction.
    pub source_ids: Vec<String>,
}

/// One tokenized corpus with explicit source and tokenizer lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizedCorpusLineage {
    /// Stable corpus identifier.
    pub corpus_id: String,
    /// Stable tokenized corpus digest.
    pub corpus_digest: String,
    /// Parent tokenizer artifact id.
    pub tokenizer_artifact_id: String,
    /// Source ids materialized into the corpus.
    pub source_ids: Vec<String>,
}

/// One SFT artifact with explicit corpus and source lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSftArtifactLineage {
    /// Stable SFT artifact id.
    pub artifact_id: String,
    /// Stable SFT artifact digest.
    pub artifact_digest: String,
    /// Parent corpus ids.
    pub tokenized_corpus_ids: Vec<String>,
    /// Source ids that flowed into the artifact.
    pub source_ids: Vec<String>,
}

/// One promoted checkpoint with explicit source lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointArtifactLineage {
    /// Stable checkpoint id.
    pub checkpoint_id: String,
    /// Stable checkpoint digest.
    pub checkpoint_digest: String,
    /// Parent corpus ids.
    pub tokenized_corpus_ids: Vec<String>,
    /// Parent SFT artifact ids when they exist.
    pub sft_artifact_ids: Vec<String>,
    /// Source ids that fed the checkpoint.
    pub source_ids: Vec<String>,
}

/// One benchmark package with explicit source lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkArtifactLineage {
    /// Stable benchmark package id.
    pub benchmark_id: String,
    /// Stable benchmark package digest.
    pub benchmark_digest: String,
    /// Source ids used to build the package or labels.
    pub source_ids: Vec<String>,
}

/// Versioned lineage manifest tying reviewed sources to downstream Psion artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionArtifactLineageManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Lifecycle schema version this lineage set was built against.
    pub lifecycle_schema_version: String,
    /// Tokenizer artifacts with explicit source exposure.
    pub tokenizer_artifacts: Vec<PsionTokenizerArtifactLineage>,
    /// Tokenized corpora with explicit source and tokenizer lineage.
    pub tokenized_corpora: Vec<PsionTokenizedCorpusLineage>,
    /// SFT artifacts with explicit source lineage.
    pub sft_artifacts: Vec<PsionSftArtifactLineage>,
    /// Promoted checkpoints with explicit source lineage.
    pub checkpoints: Vec<PsionCheckpointArtifactLineage>,
    /// Benchmark packages with explicit source lineage.
    pub benchmark_artifacts: Vec<PsionBenchmarkArtifactLineage>,
}

impl PsionArtifactLineageManifest {
    /// Validates the lineage manifest against the current lifecycle manifest.
    pub fn validate_against_lifecycle(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
    ) -> Result<(), PsionSourceLifecycleError> {
        if self.schema_version.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingLineageSchemaVersion);
        }
        if self.lifecycle_schema_version.trim().is_empty() {
            return Err(PsionSourceLifecycleError::MissingLineageLifecycleSchemaVersion);
        }
        if self.lifecycle_schema_version != lifecycle.schema_version {
            return Err(PsionSourceLifecycleError::LifecycleSchemaVersionMismatch {
                expected: lifecycle.schema_version.clone(),
                actual: self.lifecycle_schema_version.clone(),
            });
        }

        let lifecycle_source_ids = lifecycle
            .sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut tokenizer_ids = BTreeSet::new();
        for artifact in &self.tokenizer_artifacts {
            if artifact.artifact_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingTokenizerArtifactId);
            }
            if artifact.artifact_digest.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingTokenizerArtifactDigest {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            if !tokenizer_ids.insert(artifact.artifact_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateTokenizerArtifactId {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            validate_artifact_sources(
                artifact.source_ids.as_slice(),
                lifecycle_source_ids.clone(),
                ArtifactFamily::Tokenizer,
                artifact.artifact_id.as_str(),
            )?;
        }

        let tokenizer_id_refs = self
            .tokenizer_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut corpus_ids = BTreeSet::new();
        for corpus in &self.tokenized_corpora {
            if corpus.corpus_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingTokenizedCorpusId);
            }
            if corpus.corpus_digest.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingTokenizedCorpusDigest {
                    corpus_id: corpus.corpus_id.clone(),
                });
            }
            if !corpus_ids.insert(corpus.corpus_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateTokenizedCorpusId {
                    corpus_id: corpus.corpus_id.clone(),
                });
            }
            if !tokenizer_id_refs.contains(corpus.tokenizer_artifact_id.as_str()) {
                return Err(PsionSourceLifecycleError::UnknownTokenizerArtifactRef {
                    corpus_id: corpus.corpus_id.clone(),
                    tokenizer_artifact_id: corpus.tokenizer_artifact_id.clone(),
                });
            }
            validate_artifact_sources(
                corpus.source_ids.as_slice(),
                lifecycle_source_ids.clone(),
                ArtifactFamily::TokenizedCorpus,
                corpus.corpus_id.as_str(),
            )?;
        }

        let corpus_id_refs = self
            .tokenized_corpora
            .iter()
            .map(|corpus| corpus.corpus_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut sft_ids = BTreeSet::new();
        for artifact in &self.sft_artifacts {
            if artifact.artifact_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingSftArtifactId);
            }
            if artifact.artifact_digest.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingSftArtifactDigest {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            if !sft_ids.insert(artifact.artifact_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateSftArtifactId {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            if artifact.tokenized_corpus_ids.is_empty() {
                return Err(PsionSourceLifecycleError::MissingSftArtifactCorpusRefs {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            for corpus_id in &artifact.tokenized_corpus_ids {
                if !corpus_id_refs.contains(corpus_id.as_str()) {
                    return Err(PsionSourceLifecycleError::UnknownTokenizedCorpusRef {
                        artifact_family: String::from("sft_artifact"),
                        artifact_id: artifact.artifact_id.clone(),
                        corpus_id: corpus_id.clone(),
                    });
                }
            }
            validate_artifact_sources(
                artifact.source_ids.as_slice(),
                lifecycle_source_ids.clone(),
                ArtifactFamily::SftArtifact,
                artifact.artifact_id.as_str(),
            )?;
        }

        let sft_id_refs = self
            .sft_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut checkpoint_ids = BTreeSet::new();
        for checkpoint in &self.checkpoints {
            if checkpoint.checkpoint_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingCheckpointId);
            }
            if checkpoint.checkpoint_digest.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingCheckpointDigest {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                });
            }
            if !checkpoint_ids.insert(checkpoint.checkpoint_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateCheckpointId {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                });
            }
            if checkpoint.tokenized_corpus_ids.is_empty() {
                return Err(PsionSourceLifecycleError::MissingCheckpointCorpusRefs {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                });
            }
            for corpus_id in &checkpoint.tokenized_corpus_ids {
                if !corpus_id_refs.contains(corpus_id.as_str()) {
                    return Err(PsionSourceLifecycleError::UnknownTokenizedCorpusRef {
                        artifact_family: String::from("checkpoint"),
                        artifact_id: checkpoint.checkpoint_id.clone(),
                        corpus_id: corpus_id.clone(),
                    });
                }
            }
            for artifact_id in &checkpoint.sft_artifact_ids {
                if !sft_id_refs.contains(artifact_id.as_str()) {
                    return Err(PsionSourceLifecycleError::UnknownSftArtifactRef {
                        checkpoint_id: checkpoint.checkpoint_id.clone(),
                        artifact_id: artifact_id.clone(),
                    });
                }
            }
            validate_artifact_sources(
                checkpoint.source_ids.as_slice(),
                lifecycle_source_ids.clone(),
                ArtifactFamily::Checkpoint,
                checkpoint.checkpoint_id.as_str(),
            )?;
        }

        let mut benchmark_ids = BTreeSet::new();
        for benchmark in &self.benchmark_artifacts {
            if benchmark.benchmark_id.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingBenchmarkArtifactId);
            }
            if benchmark.benchmark_digest.trim().is_empty() {
                return Err(PsionSourceLifecycleError::MissingBenchmarkArtifactDigest {
                    benchmark_id: benchmark.benchmark_id.clone(),
                });
            }
            if !benchmark_ids.insert(benchmark.benchmark_id.clone()) {
                return Err(PsionSourceLifecycleError::DuplicateBenchmarkArtifactId {
                    benchmark_id: benchmark.benchmark_id.clone(),
                });
            }
            validate_artifact_sources(
                benchmark.source_ids.as_slice(),
                lifecycle_source_ids.clone(),
                ArtifactFamily::Benchmark,
                benchmark.benchmark_id.as_str(),
            )?;
        }

        Ok(())
    }

    /// Builds an explicit downstream impact-analysis receipt for one source change.
    pub fn build_impact_analysis(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        source_id: &str,
        next_state: PsionSourceLifecycleState,
        next_rights_posture: PsionSourceRightsPosture,
        trigger: PsionLifecycleChangeTrigger,
    ) -> Result<PsionSourceImpactAnalysisReceipt, PsionSourceLifecycleError> {
        self.validate_against_lifecycle(lifecycle)?;
        let Some(record) = lifecycle.source_record(source_id) else {
            return Err(PsionSourceLifecycleError::UnknownLifecycleSource {
                source_id: source_id.to_string(),
            });
        };
        if !record.lifecycle_state.allows_transition(next_state) {
            return Err(PsionSourceLifecycleError::DisallowedLifecycleTransition {
                source_id: source_id.to_string(),
                from: record.lifecycle_state,
                to: next_state,
            });
        }

        let affected_tokenizer_artifact_ids = self
            .tokenizer_artifacts
            .iter()
            .filter(|artifact| artifact.source_ids.iter().any(|id| id == source_id))
            .map(|artifact| artifact.artifact_id.clone())
            .collect::<Vec<_>>();
        let affected_tokenized_corpus_ids = self
            .tokenized_corpora
            .iter()
            .filter(|corpus| corpus.source_ids.iter().any(|id| id == source_id))
            .map(|corpus| corpus.corpus_id.clone())
            .collect::<Vec<_>>();
        let affected_sft_artifact_ids = self
            .sft_artifacts
            .iter()
            .filter(|artifact| artifact.source_ids.iter().any(|id| id == source_id))
            .map(|artifact| artifact.artifact_id.clone())
            .collect::<Vec<_>>();
        let affected_checkpoint_ids = self
            .checkpoints
            .iter()
            .filter(|checkpoint| checkpoint.source_ids.iter().any(|id| id == source_id))
            .map(|checkpoint| checkpoint.checkpoint_id.clone())
            .collect::<Vec<_>>();
        let affected_benchmark_artifact_ids = self
            .benchmark_artifacts
            .iter()
            .filter(|artifact| artifact.source_ids.iter().any(|id| id == source_id))
            .map(|artifact| artifact.benchmark_id.clone())
            .collect::<Vec<_>>();

        let mut required_actions = BTreeSet::new();
        if matches!(
            trigger,
            PsionLifecycleChangeTrigger::ProvenanceInvalidated
                | PsionLifecycleChangeTrigger::ContentDigestMismatch
        ) {
            required_actions.insert(PsionLifecycleImpactAction::ReingestRawSource);
        }
        if !affected_tokenizer_artifact_ids.is_empty() {
            required_actions.insert(PsionLifecycleImpactAction::RetokenizeAffectedArtifacts);
        }
        if !affected_tokenized_corpus_ids.is_empty() {
            required_actions.insert(PsionLifecycleImpactAction::RebuildTokenizedCorpora);
        }
        if !affected_sft_artifact_ids.is_empty() || !affected_checkpoint_ids.is_empty() {
            required_actions.insert(PsionLifecycleImpactAction::RetrainingReview);
        }
        if !affected_benchmark_artifact_ids.is_empty() {
            required_actions.insert(PsionLifecycleImpactAction::BenchmarkInvalidationReview);
        }
        if matches!(
            next_state,
            PsionSourceLifecycleState::Withdrawn | PsionSourceLifecycleState::Rejected
        ) || next_rights_posture == PsionSourceRightsPosture::EvaluationOnly
        {
            required_actions.insert(PsionLifecycleImpactAction::CapabilityMatrixReview);
            required_actions.insert(PsionLifecycleImpactAction::DepublicationReview);
        }

        Ok(PsionSourceImpactAnalysisReceipt {
            schema_version: String::from(PSION_SOURCE_IMPACT_ANALYSIS_SCHEMA_VERSION),
            source_id: source_id.to_string(),
            previous_state: record.lifecycle_state,
            next_state,
            previous_rights_posture: record.current_rights_posture,
            next_rights_posture,
            trigger,
            affected_tokenizer_artifact_ids,
            affected_tokenized_corpus_ids,
            affected_sft_artifact_ids,
            affected_checkpoint_ids,
            affected_benchmark_artifact_ids,
            required_actions: required_actions.into_iter().collect(),
        })
    }
}

/// Durable receipt for one source posture change and its downstream impact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceImpactAnalysisReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Source under review.
    pub source_id: String,
    /// Previous lifecycle state.
    pub previous_state: PsionSourceLifecycleState,
    /// Proposed or current next lifecycle state.
    pub next_state: PsionSourceLifecycleState,
    /// Previous effective rights posture.
    pub previous_rights_posture: PsionSourceRightsPosture,
    /// Proposed or current next rights posture.
    pub next_rights_posture: PsionSourceRightsPosture,
    /// Trigger for the change.
    pub trigger: PsionLifecycleChangeTrigger,
    /// Tokenizer artifacts exposed to the source.
    pub affected_tokenizer_artifact_ids: Vec<String>,
    /// Tokenized corpora exposed to the source.
    pub affected_tokenized_corpus_ids: Vec<String>,
    /// SFT artifacts exposed to the source.
    pub affected_sft_artifact_ids: Vec<String>,
    /// Checkpoints exposed to the source.
    pub affected_checkpoint_ids: Vec<String>,
    /// Benchmark packages exposed to the source.
    pub affected_benchmark_artifact_ids: Vec<String>,
    /// Required downstream review or rebuild actions.
    pub required_actions: Vec<PsionLifecycleImpactAction>,
}

#[derive(Clone, Copy)]
enum ArtifactFamily {
    Tokenizer,
    TokenizedCorpus,
    SftArtifact,
    Checkpoint,
    Benchmark,
}

fn validate_artifact_sources(
    source_ids: &[String],
    lifecycle_source_ids: BTreeSet<&str>,
    family: ArtifactFamily,
    artifact_id: &str,
) -> Result<(), PsionSourceLifecycleError> {
    if source_ids.is_empty() {
        return Err(match family {
            ArtifactFamily::Tokenizer => {
                PsionSourceLifecycleError::MissingTokenizerArtifactSources {
                    artifact_id: artifact_id.to_string(),
                }
            }
            ArtifactFamily::TokenizedCorpus => {
                PsionSourceLifecycleError::MissingTokenizedCorpusSources {
                    corpus_id: artifact_id.to_string(),
                }
            }
            ArtifactFamily::SftArtifact => PsionSourceLifecycleError::MissingSftArtifactSources {
                artifact_id: artifact_id.to_string(),
            },
            ArtifactFamily::Checkpoint => PsionSourceLifecycleError::MissingCheckpointSources {
                checkpoint_id: artifact_id.to_string(),
            },
            ArtifactFamily::Benchmark => {
                PsionSourceLifecycleError::MissingBenchmarkArtifactSources {
                    benchmark_id: artifact_id.to_string(),
                }
            }
        });
    }

    for source_id in source_ids {
        if !lifecycle_source_ids.contains(source_id.as_str()) {
            return Err(match family {
                ArtifactFamily::Tokenizer => {
                    PsionSourceLifecycleError::UnknownLifecycleSourceRefInTokenizerArtifact {
                        artifact_id: artifact_id.to_string(),
                        source_id: source_id.clone(),
                    }
                }
                ArtifactFamily::TokenizedCorpus => {
                    PsionSourceLifecycleError::UnknownLifecycleSourceRefInTokenizedCorpus {
                        corpus_id: artifact_id.to_string(),
                        source_id: source_id.clone(),
                    }
                }
                ArtifactFamily::SftArtifact => {
                    PsionSourceLifecycleError::UnknownLifecycleSourceRefInSftArtifact {
                        artifact_id: artifact_id.to_string(),
                        source_id: source_id.clone(),
                    }
                }
                ArtifactFamily::Checkpoint => {
                    PsionSourceLifecycleError::UnknownLifecycleSourceRefInCheckpoint {
                        checkpoint_id: artifact_id.to_string(),
                        source_id: source_id.clone(),
                    }
                }
                ArtifactFamily::Benchmark => {
                    PsionSourceLifecycleError::UnknownLifecycleSourceRefInBenchmarkArtifact {
                        benchmark_id: artifact_id.to_string(),
                        source_id: source_id.clone(),
                    }
                }
            });
        }
    }

    Ok(())
}

/// Validation failures for Psion source lifecycle and downstream lineage.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionSourceLifecycleError {
    /// Lifecycle manifest omitted its schema version.
    #[error("Psion source-lifecycle manifest is missing `schema_version`")]
    MissingLifecycleSchemaVersion,
    /// Lifecycle manifest omitted its policy id.
    #[error("Psion source-lifecycle manifest is missing `policy_id`")]
    MissingLifecyclePolicyId,
    /// Lifecycle manifest omitted its policy version.
    #[error("Psion source-lifecycle manifest is missing `policy_version`")]
    MissingLifecyclePolicyVersion,
    /// Lifecycle manifest omitted its admission schema version.
    #[error("Psion source-lifecycle manifest is missing `admission_schema_version`")]
    MissingAdmissionSchemaVersion,
    /// Lifecycle manifest policy id mismatched admission.
    #[error("Psion source-lifecycle manifest policy id mismatch: expected `{expected}`, found `{actual}`")]
    PolicyIdMismatch {
        /// Expected policy id.
        expected: String,
        /// Actual policy id.
        actual: String,
    },
    /// Lifecycle manifest policy version mismatched admission.
    #[error(
        "Psion source-lifecycle manifest policy version mismatch: expected `{expected}`, found `{actual}`"
    )]
    PolicyVersionMismatch {
        /// Expected policy version.
        expected: String,
        /// Actual policy version.
        actual: String,
    },
    /// Lifecycle manifest admission schema version mismatched admission.
    #[error(
        "Psion source-lifecycle manifest admission schema version mismatch: expected `{expected}`, found `{actual}`"
    )]
    AdmissionSchemaVersionMismatch {
        /// Expected admission schema version.
        expected: String,
        /// Actual admission schema version.
        actual: String,
    },
    /// Lifecycle manifest omitted all sources.
    #[error("Psion source-lifecycle manifest requires at least one `sources` entry")]
    MissingLifecycleRecords,
    /// Lifecycle record omitted source id.
    #[error("Psion source-lifecycle manifest contains a record with empty `source_id`")]
    MissingSourceId,
    /// Lifecycle record repeated source id.
    #[error("Psion source-lifecycle manifest repeats source `{source_id}`")]
    DuplicateSourceId {
        /// Duplicated source identifier.
        source_id: String,
    },
    /// Lifecycle record omitted transition event id.
    #[error("Psion lifecycle record for `{source_id}` is missing `last_transition_event_id`")]
    MissingTransitionEventId {
        /// Source identifier.
        source_id: String,
    },
    /// Lifecycle record omitted rationale.
    #[error("Psion lifecycle record for `{source_id}` is missing `rationale`")]
    MissingLifecycleRationale {
        /// Source identifier.
        source_id: String,
    },
    /// Lifecycle record referred to a source not present in admission.
    #[error("Psion lifecycle record references unknown admission source `{source_id}`")]
    UnknownAdmissionSource {
        /// Source identifier.
        source_id: String,
    },
    /// Admission rights posture snapshot mismatched the admission manifest.
    #[error(
        "Psion lifecycle record `{source_id}` admission rights posture mismatch: expected `{expected:?}`, found `{actual:?}`"
    )]
    AdmissionRightsPostureMismatch {
        /// Source identifier.
        source_id: String,
        /// Expected rights posture from admission.
        expected: PsionSourceRightsPosture,
        /// Actual lifecycle snapshot.
        actual: PsionSourceRightsPosture,
    },
    /// Admitted lifecycle state used a restricted or rejected posture.
    #[error(
        "Psion lifecycle record `{source_id}` is `admitted` but uses restricted or rejected posture `{rights_posture:?}`"
    )]
    AdmittedStateUsesRestrictedOrRejectedPosture {
        /// Source identifier.
        source_id: String,
        /// Current rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Rejected admission may not be elevated back into admitted state without a new admission manifest.
    #[error("Psion lifecycle record `{source_id}` cannot be `admitted` because the admission record is rejected")]
    RejectedAdmissionCannotBeAdmitted {
        /// Source identifier.
        source_id: String,
    },
    /// Restricted state must keep non-eval, non-rejected posture.
    #[error(
        "Psion lifecycle record `{source_id}` is `restricted` but uses invalid posture `{rights_posture:?}`"
    )]
    RestrictedStateRequiresNonEvalNonRejectedPosture {
        /// Source identifier.
        source_id: String,
        /// Current rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Evaluation-only state requires evaluation-only posture.
    #[error(
        "Psion lifecycle record `{source_id}` is `evaluation_only` but uses posture `{rights_posture:?}`"
    )]
    EvaluationOnlyStateRequiresEvaluationPosture {
        /// Source identifier.
        source_id: String,
        /// Current rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Withdrawn state may not keep internal serving posture.
    #[error("Psion lifecycle record `{source_id}` is `withdrawn` but still keeps internal-serving posture")]
    WithdrawnStateCannotKeepServingPosture {
        /// Source identifier.
        source_id: String,
    },
    /// Rejected state requires rejected posture.
    #[error(
        "Psion lifecycle record `{source_id}` is `rejected` but uses posture `{rights_posture:?}`"
    )]
    RejectedStateRequiresRejectedPosture {
        /// Source identifier.
        source_id: String,
        /// Current rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Admission source omitted from lifecycle manifest.
    #[error("Psion admission source `{source_id}` is missing from the lifecycle manifest")]
    MissingLifecycleRecordForAdmissionSource {
        /// Source identifier.
        source_id: String,
    },
    /// Lineage manifest omitted its schema version.
    #[error("Psion artifact-lineage manifest is missing `schema_version`")]
    MissingLineageSchemaVersion,
    /// Lineage manifest omitted lifecycle schema version.
    #[error("Psion artifact-lineage manifest is missing `lifecycle_schema_version`")]
    MissingLineageLifecycleSchemaVersion,
    /// Lineage manifest lifecycle schema version mismatched lifecycle manifest.
    #[error(
        "Psion artifact-lineage lifecycle schema version mismatch: expected `{expected}`, found `{actual}`"
    )]
    LifecycleSchemaVersionMismatch {
        /// Expected lifecycle schema version.
        expected: String,
        /// Actual lifecycle schema version.
        actual: String,
    },
    /// Tokenizer artifact omitted artifact id.
    #[error("Psion tokenizer-lineage manifest contains a row with empty `artifact_id`")]
    MissingTokenizerArtifactId,
    /// Tokenizer artifact omitted digest.
    #[error("Psion tokenizer artifact `{artifact_id}` is missing `artifact_digest`")]
    MissingTokenizerArtifactDigest {
        /// Tokenizer artifact id.
        artifact_id: String,
    },
    /// Tokenizer artifact id was duplicated.
    #[error("Psion tokenizer-lineage manifest repeats tokenizer artifact `{artifact_id}`")]
    DuplicateTokenizerArtifactId {
        /// Tokenizer artifact id.
        artifact_id: String,
    },
    /// Tokenizer artifact omitted source refs.
    #[error("Psion tokenizer artifact `{artifact_id}` is missing `source_ids`")]
    MissingTokenizerArtifactSources {
        /// Tokenizer artifact id.
        artifact_id: String,
    },
    /// Tokenizer artifact referred to an unknown lifecycle source.
    #[error(
        "Psion tokenizer artifact `{artifact_id}` references unknown lifecycle source `{source_id}`"
    )]
    UnknownLifecycleSourceRefInTokenizerArtifact {
        /// Tokenizer artifact id.
        artifact_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// Tokenized corpus omitted corpus id.
    #[error("Psion tokenized-corpus lineage contains a row with empty `corpus_id`")]
    MissingTokenizedCorpusId,
    /// Tokenized corpus omitted digest.
    #[error("Psion tokenized corpus `{corpus_id}` is missing `corpus_digest`")]
    MissingTokenizedCorpusDigest {
        /// Tokenized corpus id.
        corpus_id: String,
    },
    /// Tokenized corpus id was duplicated.
    #[error("Psion tokenized-corpus lineage repeats corpus `{corpus_id}`")]
    DuplicateTokenizedCorpusId {
        /// Tokenized corpus id.
        corpus_id: String,
    },
    /// Tokenized corpus referred to unknown tokenizer artifact.
    #[error(
        "Psion tokenized corpus `{corpus_id}` references unknown tokenizer artifact `{tokenizer_artifact_id}`"
    )]
    UnknownTokenizerArtifactRef {
        /// Tokenized corpus id.
        corpus_id: String,
        /// Unknown tokenizer artifact id.
        tokenizer_artifact_id: String,
    },
    /// Tokenized corpus omitted source ids.
    #[error("Psion tokenized corpus `{corpus_id}` is missing `source_ids`")]
    MissingTokenizedCorpusSources {
        /// Tokenized corpus id.
        corpus_id: String,
    },
    /// Tokenized corpus referred to unknown lifecycle source.
    #[error(
        "Psion tokenized corpus `{corpus_id}` references unknown lifecycle source `{source_id}`"
    )]
    UnknownLifecycleSourceRefInTokenizedCorpus {
        /// Tokenized corpus id.
        corpus_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// SFT artifact omitted artifact id.
    #[error("Psion SFT lineage contains a row with empty `artifact_id`")]
    MissingSftArtifactId,
    /// SFT artifact omitted digest.
    #[error("Psion SFT artifact `{artifact_id}` is missing `artifact_digest`")]
    MissingSftArtifactDigest {
        /// SFT artifact id.
        artifact_id: String,
    },
    /// SFT artifact id was duplicated.
    #[error("Psion SFT lineage repeats artifact `{artifact_id}`")]
    DuplicateSftArtifactId {
        /// SFT artifact id.
        artifact_id: String,
    },
    /// SFT artifact omitted corpus refs.
    #[error("Psion SFT artifact `{artifact_id}` is missing `tokenized_corpus_ids`")]
    MissingSftArtifactCorpusRefs {
        /// SFT artifact id.
        artifact_id: String,
    },
    /// SFT artifact omitted source ids.
    #[error("Psion SFT artifact `{artifact_id}` is missing `source_ids`")]
    MissingSftArtifactSources {
        /// SFT artifact id.
        artifact_id: String,
    },
    /// SFT artifact referred to unknown lifecycle source.
    #[error(
        "Psion SFT artifact `{artifact_id}` references unknown lifecycle source `{source_id}`"
    )]
    UnknownLifecycleSourceRefInSftArtifact {
        /// SFT artifact id.
        artifact_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// Artifact referred to unknown tokenized corpus.
    #[error(
        "Psion {artifact_family} `{artifact_id}` references unknown tokenized corpus `{corpus_id}`"
    )]
    UnknownTokenizedCorpusRef {
        /// Artifact family label.
        artifact_family: String,
        /// Artifact id.
        artifact_id: String,
        /// Unknown tokenized corpus id.
        corpus_id: String,
    },
    /// Checkpoint omitted id.
    #[error("Psion checkpoint lineage contains a row with empty `checkpoint_id`")]
    MissingCheckpointId,
    /// Checkpoint omitted digest.
    #[error("Psion checkpoint `{checkpoint_id}` is missing `checkpoint_digest`")]
    MissingCheckpointDigest {
        /// Checkpoint id.
        checkpoint_id: String,
    },
    /// Checkpoint id was duplicated.
    #[error("Psion checkpoint lineage repeats checkpoint `{checkpoint_id}`")]
    DuplicateCheckpointId {
        /// Checkpoint id.
        checkpoint_id: String,
    },
    /// Checkpoint omitted corpus refs.
    #[error("Psion checkpoint `{checkpoint_id}` is missing `tokenized_corpus_ids`")]
    MissingCheckpointCorpusRefs {
        /// Checkpoint id.
        checkpoint_id: String,
    },
    /// Checkpoint omitted source ids.
    #[error("Psion checkpoint `{checkpoint_id}` is missing `source_ids`")]
    MissingCheckpointSources {
        /// Checkpoint id.
        checkpoint_id: String,
    },
    /// Checkpoint referred to unknown lifecycle source.
    #[error(
        "Psion checkpoint `{checkpoint_id}` references unknown lifecycle source `{source_id}`"
    )]
    UnknownLifecycleSourceRefInCheckpoint {
        /// Checkpoint id.
        checkpoint_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// Checkpoint referred to unknown SFT artifact.
    #[error("Psion checkpoint `{checkpoint_id}` references unknown SFT artifact `{artifact_id}`")]
    UnknownSftArtifactRef {
        /// Checkpoint id.
        checkpoint_id: String,
        /// Unknown SFT artifact id.
        artifact_id: String,
    },
    /// Benchmark artifact omitted id.
    #[error("Psion benchmark-lineage contains a row with empty `benchmark_id`")]
    MissingBenchmarkArtifactId,
    /// Benchmark artifact omitted digest.
    #[error("Psion benchmark artifact `{benchmark_id}` is missing `benchmark_digest`")]
    MissingBenchmarkArtifactDigest {
        /// Benchmark id.
        benchmark_id: String,
    },
    /// Benchmark id was duplicated.
    #[error("Psion benchmark-lineage repeats benchmark `{benchmark_id}`")]
    DuplicateBenchmarkArtifactId {
        /// Benchmark id.
        benchmark_id: String,
    },
    /// Benchmark omitted source ids.
    #[error("Psion benchmark artifact `{benchmark_id}` is missing `source_ids`")]
    MissingBenchmarkArtifactSources {
        /// Benchmark id.
        benchmark_id: String,
    },
    /// Benchmark referred to unknown lifecycle source.
    #[error(
        "Psion benchmark artifact `{benchmark_id}` references unknown lifecycle source `{source_id}`"
    )]
    UnknownLifecycleSourceRefInBenchmarkArtifact {
        /// Benchmark id.
        benchmark_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// Impact analysis looked up a source not present in lifecycle.
    #[error("Psion impact analysis references unknown lifecycle source `{source_id}`")]
    UnknownLifecycleSource {
        /// Source identifier.
        source_id: String,
    },
    /// Requested transition was outside the declared lifecycle graph.
    #[error(
        "Psion source `{source_id}` cannot transition from `{from:?}` to `{to:?}` under the current lifecycle policy"
    )]
    DisallowedLifecycleTransition {
        /// Source identifier.
        source_id: String,
        /// Current state.
        from: PsionSourceLifecycleState,
        /// Requested next state.
        to: PsionSourceLifecycleState,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PsionCorpusAdmissionManifest;

    fn admission_manifest() -> PsionCorpusAdmissionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json"
        ))
        .expect("admission manifest should parse")
    }

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle manifest should parse")
    }

    fn artifact_lineage_manifest() -> PsionArtifactLineageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"
        ))
        .expect("artifact-lineage manifest should parse")
    }

    #[test]
    fn lifecycle_manifest_tracks_all_admission_sources() {
        let lifecycle = lifecycle_manifest();
        lifecycle
            .validate_against_admission(&admission_manifest())
            .expect("lifecycle manifest should validate");
        let evaluation_only = lifecycle
            .sources
            .iter()
            .find(|source| source.lifecycle_state == PsionSourceLifecycleState::EvaluationOnly)
            .expect("fixture should include one evaluation-only source");
        assert_eq!(
            evaluation_only.current_rights_posture,
            PsionSourceRightsPosture::EvaluationOnly
        );
    }

    #[test]
    fn artifact_lineage_manifest_tracks_downstream_source_usage() {
        let lifecycle = lifecycle_manifest();
        let lineage = artifact_lineage_manifest();
        lifecycle
            .validate_against_admission(&admission_manifest())
            .expect("lifecycle manifest should validate");
        lineage
            .validate_against_lifecycle(&lifecycle)
            .expect("artifact-lineage manifest should validate");
        assert_eq!(lineage.checkpoints.len(), 1);
        assert_eq!(lineage.benchmark_artifacts.len(), 7);
        assert!(lineage
            .benchmark_artifacts
            .iter()
            .any(|artifact| artifact.benchmark_id == "psion_spec_reading_benchmark_v1"));
        assert!(lineage
            .benchmark_artifacts
            .iter()
            .any(|artifact| artifact.benchmark_id == "psion_route_benchmark_v1"));
    }

    #[test]
    fn impact_analysis_receipt_finds_affected_artifacts() {
        let lifecycle = lifecycle_manifest();
        let lineage = artifact_lineage_manifest();
        let receipt = lineage
            .build_impact_analysis(
                &lifecycle,
                "wasm_core_spec_release_2",
                PsionSourceLifecycleState::Withdrawn,
                PsionSourceRightsPosture::Rejected,
                PsionLifecycleChangeTrigger::RightsChangedOrRevoked,
            )
            .expect("impact analysis should build");
        assert_eq!(
            receipt.affected_tokenizer_artifact_ids,
            vec![String::from("psion_tokenizer_seed_v1")]
        );
        assert_eq!(
            receipt.affected_checkpoint_ids,
            vec![String::from("psion_pilot_checkpoint_v1")]
        );
        assert_eq!(
            receipt.affected_benchmark_artifact_ids,
            vec![
                String::from("psion_spec_reading_benchmark_v1"),
                String::from("psion_normative_spec_benchmark_v1"),
                String::from("psion_engineering_spec_benchmark_v1"),
                String::from("psion_memorization_reasoning_benchmark_v1"),
            ]
        );
        assert!(receipt
            .required_actions
            .contains(&PsionLifecycleImpactAction::RetokenizeAffectedArtifacts));
        assert!(receipt
            .required_actions
            .contains(&PsionLifecycleImpactAction::RetrainingReview));
        assert!(receipt
            .required_actions
            .contains(&PsionLifecycleImpactAction::BenchmarkInvalidationReview));
        assert!(receipt
            .required_actions
            .contains(&PsionLifecycleImpactAction::CapabilityMatrixReview));
        assert!(receipt
            .required_actions
            .contains(&PsionLifecycleImpactAction::DepublicationReview));
    }

    #[test]
    fn impact_analysis_refuses_disallowed_transition() {
        let lifecycle = lifecycle_manifest();
        let lineage = artifact_lineage_manifest();
        let error = lineage
            .build_impact_analysis(
                &lifecycle,
                "spec_quiz_eval_pack_v1",
                PsionSourceLifecycleState::Admitted,
                PsionSourceRightsPosture::InternalTrainingOnly,
                PsionLifecycleChangeTrigger::ReviewMisclassificationCorrected,
            )
            .expect_err("evaluation-only source should not jump directly to admitted");
        assert!(matches!(
            error,
            PsionSourceLifecycleError::DisallowedLifecycleTransition { .. }
        ));
    }
}
