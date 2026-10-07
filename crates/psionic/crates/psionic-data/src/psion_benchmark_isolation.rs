use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    PsionCorpusSourceKind, PsionSourceBoundaryKind, PsionSourceLifecycleManifest,
    PsionSourceLifecycleState,
};

/// Stable schema version for the first Psion benchmark-isolation contract.
pub const PSION_BENCHMARK_ISOLATION_SCHEMA_VERSION: &str = "psion.benchmark_isolation.v1";

/// Loader or build surface that must obey the exclusion manifest mechanically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionLoaderSurface {
    /// Tokenizer-training input construction.
    TokenizerTraining,
    /// Model-training dataset loading.
    ModelTraining,
    /// Benchmark package or held-out package loading.
    BenchmarkPackage,
}

/// Section-range disjointness mode required for one source family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSectionDisjointnessMode {
    /// Entire source must be disjoint across train, held-out, and benchmark.
    EntireSourceDisjoint,
    /// Chapter and section anchors may define the train-vs-benchmark split.
    ChapterSectionDisjoint,
    /// Page ranges may define the train-vs-benchmark split.
    PageRangeDisjoint,
    /// File paths or symbol blocks may define the train-vs-benchmark split.
    FilePathDisjoint,
}

/// Section-range disjointness rule for one main source family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSectionRangeDisjointnessRule {
    /// Source family covered by the rule.
    pub source_kind: PsionCorpusSourceKind,
    /// Boundary kind required for the family.
    pub required_boundary_kind: PsionSourceBoundaryKind,
    /// Disjointness mode required for the family.
    pub disjointness_mode: PsionSectionDisjointnessMode,
    /// Short rationale for the rule.
    pub rationale: String,
}

/// Tokenizer exposure report for one source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTokenizerExposureRecord {
    /// Stable source identifier.
    pub source_id: String,
    /// Whether tokenizer construction may see the source.
    pub tokenizer_exposed: bool,
    /// Whether model training may see the source.
    pub model_training_exposed: bool,
    /// Whether benchmark or held-out packages may see the source.
    pub benchmark_exposed: bool,
    /// Short explanation of the exposure posture.
    pub detail: String,
}

/// Explicit near-duplicate review requirement for the Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionNearDuplicateReviewPolicy {
    /// Duplicate review is mandatory before model-training datasets are frozen.
    pub review_required_before_training: bool,
    /// Duplicate review is mandatory before benchmark publication.
    pub review_required_before_benchmark_publication: bool,
}

/// Required consequence of a contamination violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionContaminationViolationConsequence {
    /// The affected benchmark must be invalidated.
    InvalidateAffectedBenchmark,
    /// Capability publication must be reviewed or downgraded.
    TriggerCapabilityMatrixReview,
    /// The benchmark package must be rebuilt or republished.
    TriggerBenchmarkRebuildReview,
}

/// Versioned exclusion manifest for training and benchmark isolation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExclusionManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Source-lifecycle schema version this manifest depends on.
    pub lifecycle_schema_version: String,
    /// Source ids frozen as held-out and therefore excluded from training.
    pub held_out_source_ids: Vec<String>,
    /// Source ids mechanically excluded from model training.
    pub training_excluded_source_ids: Vec<String>,
    /// Source ids mechanically excluded from benchmark publication.
    pub benchmark_excluded_source_ids: Vec<String>,
    /// Tokenizer exposure records.
    pub tokenizer_exposure: Vec<PsionTokenizerExposureRecord>,
    /// Section-range disjointness rules for the main source families.
    pub section_range_rules: Vec<PsionSectionRangeDisjointnessRule>,
    /// Required duplicate-review checkpoints.
    pub near_duplicate_review_policy: PsionNearDuplicateReviewPolicy,
    /// Consequences of a contamination violation.
    pub contamination_violation_consequences: Vec<PsionContaminationViolationConsequence>,
}

impl PsionExclusionManifest {
    /// Validates the isolation contract against the current lifecycle manifest.
    pub fn validate_against_lifecycle(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
    ) -> Result<(), PsionBenchmarkIsolationError> {
        if self.schema_version.trim().is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingSchemaVersion);
        }
        if self.lifecycle_schema_version.trim().is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingLifecycleSchemaVersion);
        }
        if self.lifecycle_schema_version != lifecycle.schema_version {
            return Err(
                PsionBenchmarkIsolationError::LifecycleSchemaVersionMismatch {
                    expected: lifecycle.schema_version.clone(),
                    actual: self.lifecycle_schema_version.clone(),
                },
            );
        }
        if self.tokenizer_exposure.is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingTokenizerExposureReport);
        }
        if self.section_range_rules.is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingSectionRangeRules);
        }
        if self.contamination_violation_consequences.is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingContaminationConsequences);
        }
        if !self
            .near_duplicate_review_policy
            .review_required_before_training
        {
            return Err(PsionBenchmarkIsolationError::TrainingNearDuplicateReviewMustBeRequired);
        }
        if !self
            .near_duplicate_review_policy
            .review_required_before_benchmark_publication
        {
            return Err(PsionBenchmarkIsolationError::BenchmarkNearDuplicateReviewMustBeRequired);
        }

        reject_duplicate_strings(
            self.held_out_source_ids.as_slice(),
            PsionBenchmarkIsolationError::DuplicateHeldOutSourceId,
        )?;
        reject_duplicate_strings(
            self.training_excluded_source_ids.as_slice(),
            PsionBenchmarkIsolationError::DuplicateTrainingExcludedSourceId,
        )?;
        reject_duplicate_strings(
            self.benchmark_excluded_source_ids.as_slice(),
            PsionBenchmarkIsolationError::DuplicateBenchmarkExcludedSourceId,
        )?;
        reject_duplicate_enum_entries(
            self.contamination_violation_consequences.as_slice(),
            || PsionBenchmarkIsolationError::DuplicateContaminationConsequence,
        )?;

        let lifecycle_ids = lifecycle
            .sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect::<BTreeSet<_>>();
        let training_excluded = self
            .training_excluded_source_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for source_id in &self.held_out_source_ids {
            if !lifecycle_ids.contains(source_id.as_str()) {
                return Err(PsionBenchmarkIsolationError::UnknownHeldOutSourceId {
                    source_id: source_id.clone(),
                });
            }
            if !training_excluded.contains(source_id.as_str()) {
                return Err(
                    PsionBenchmarkIsolationError::HeldOutSourceMustAlsoBeTrainingExcluded {
                        source_id: source_id.clone(),
                    },
                );
            }
        }
        for source_id in &self.training_excluded_source_ids {
            if !lifecycle_ids.contains(source_id.as_str()) {
                return Err(
                    PsionBenchmarkIsolationError::UnknownTrainingExcludedSourceId {
                        source_id: source_id.clone(),
                    },
                );
            }
        }
        for source_id in &self.benchmark_excluded_source_ids {
            if !lifecycle_ids.contains(source_id.as_str()) {
                return Err(
                    PsionBenchmarkIsolationError::UnknownBenchmarkExcludedSourceId {
                        source_id: source_id.clone(),
                    },
                );
            }
        }

        let mut exposure_source_ids = BTreeSet::new();
        for exposure in &self.tokenizer_exposure {
            if exposure.source_id.trim().is_empty() {
                return Err(PsionBenchmarkIsolationError::MissingExposureSourceId);
            }
            if !exposure_source_ids.insert(exposure.source_id.clone()) {
                return Err(PsionBenchmarkIsolationError::DuplicateExposureSourceId {
                    source_id: exposure.source_id.clone(),
                });
            }
            if exposure.detail.trim().is_empty() {
                return Err(PsionBenchmarkIsolationError::MissingExposureDetail {
                    source_id: exposure.source_id.clone(),
                });
            }
            let Some(lifecycle_record) = lifecycle.source_record(exposure.source_id.as_str())
            else {
                return Err(PsionBenchmarkIsolationError::UnknownExposureSourceId {
                    source_id: exposure.source_id.clone(),
                });
            };

            match lifecycle_record.lifecycle_state {
                PsionSourceLifecycleState::Admitted => {
                    if !exposure.tokenizer_exposed || !exposure.model_training_exposed {
                        return Err(
                            PsionBenchmarkIsolationError::AdmittedSourceExposureIncomplete {
                                source_id: exposure.source_id.clone(),
                            },
                        );
                    }
                }
                PsionSourceLifecycleState::Restricted => {
                    if exposure.model_training_exposed {
                        return Err(PsionBenchmarkIsolationError::RestrictedSourceCannotTrain {
                            source_id: exposure.source_id.clone(),
                        });
                    }
                }
                PsionSourceLifecycleState::EvaluationOnly => {
                    if exposure.model_training_exposed {
                        return Err(
                            PsionBenchmarkIsolationError::EvaluationOnlySourceCannotTrain {
                                source_id: exposure.source_id.clone(),
                            },
                        );
                    }
                }
                PsionSourceLifecycleState::Withdrawn | PsionSourceLifecycleState::Rejected => {
                    if exposure.model_training_exposed || exposure.benchmark_exposed {
                        return Err(
                            PsionBenchmarkIsolationError::WithdrawnOrRejectedSourceExposed {
                                source_id: exposure.source_id.clone(),
                            },
                        );
                    }
                }
            }
        }

        for source in &lifecycle.sources {
            if !exposure_source_ids.contains(source.source_id.as_str()) {
                return Err(
                    PsionBenchmarkIsolationError::MissingExposureRecordForSource {
                        source_id: source.source_id.clone(),
                    },
                );
            }
        }

        validate_section_rules(self.section_range_rules.as_slice())?;

        Ok(())
    }

    /// Mechanically rejects excluded sources for tokenizer, training, or benchmark loading.
    pub fn assert_source_ids_allowed(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        surface: PsionLoaderSurface,
        source_ids: &[String],
    ) -> Result<(), PsionBenchmarkIsolationError> {
        self.validate_against_lifecycle(lifecycle)?;
        let training_excluded = self
            .training_excluded_source_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let benchmark_excluded = self
            .benchmark_excluded_source_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        for source_id in source_ids {
            let Some(exposure) = self
                .tokenizer_exposure
                .iter()
                .find(|exposure| exposure.source_id == *source_id)
            else {
                return Err(PsionBenchmarkIsolationError::UnknownExposureSourceId {
                    source_id: source_id.clone(),
                });
            };

            let denied = match surface {
                PsionLoaderSurface::TokenizerTraining => !exposure.tokenizer_exposed,
                PsionLoaderSurface::ModelTraining => {
                    training_excluded.contains(source_id.as_str())
                        || !exposure.model_training_exposed
                }
                PsionLoaderSurface::BenchmarkPackage => {
                    benchmark_excluded.contains(source_id.as_str()) || !exposure.benchmark_exposed
                }
            };
            if denied {
                return Err(PsionBenchmarkIsolationError::LoaderSurfaceRejectedSource {
                    surface,
                    source_id: source_id.clone(),
                });
            }
        }

        Ok(())
    }
}

fn reject_duplicate_strings<F>(
    entries: &[String],
    error: F,
) -> Result<(), PsionBenchmarkIsolationError>
where
    F: Fn(String) -> PsionBenchmarkIsolationError,
{
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.clone()) {
            return Err(error(entry.clone()));
        }
    }
    Ok(())
}

fn reject_duplicate_enum_entries<T, F>(
    entries: &[T],
    error: F,
) -> Result<(), PsionBenchmarkIsolationError>
where
    T: Copy + Ord,
    F: Fn() -> PsionBenchmarkIsolationError,
{
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(*entry) {
            return Err(error());
        }
    }
    Ok(())
}

fn validate_section_rules(
    rules: &[PsionSectionRangeDisjointnessRule],
) -> Result<(), PsionBenchmarkIsolationError> {
    let mut kinds = BTreeSet::new();
    for rule in rules {
        if !kinds.insert(rule.source_kind) {
            return Err(PsionBenchmarkIsolationError::DuplicateSectionRule {
                source_kind: rule.source_kind,
            });
        }
        if rule.rationale.trim().is_empty() {
            return Err(PsionBenchmarkIsolationError::MissingSectionRuleRationale {
                source_kind: rule.source_kind,
            });
        }
    }

    for required_kind in [
        PsionCorpusSourceKind::Textbook,
        PsionCorpusSourceKind::Specification,
        PsionCorpusSourceKind::Manual,
        PsionCorpusSourceKind::Paper,
    ] {
        if !kinds.contains(&required_kind) {
            return Err(PsionBenchmarkIsolationError::MissingRequiredSectionRule {
                source_kind: required_kind,
            });
        }
    }

    Ok(())
}

/// Validation failures for Psion exclusion and benchmark isolation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionBenchmarkIsolationError {
    /// Isolation manifest omitted schema version.
    #[error("Psion exclusion manifest is missing `schema_version`")]
    MissingSchemaVersion,
    /// Isolation manifest omitted lifecycle schema version.
    #[error("Psion exclusion manifest is missing `lifecycle_schema_version`")]
    MissingLifecycleSchemaVersion,
    /// Isolation manifest mismatched lifecycle schema version.
    #[error(
        "Psion exclusion manifest lifecycle schema version mismatch: expected `{expected}`, found `{actual}`"
    )]
    LifecycleSchemaVersionMismatch {
        /// Expected lifecycle schema version.
        expected: String,
        /// Actual lifecycle schema version.
        actual: String,
    },
    /// Isolation manifest omitted tokenizer exposure.
    #[error("Psion exclusion manifest requires explicit `tokenizer_exposure` records")]
    MissingTokenizerExposureReport,
    /// Isolation manifest omitted section rules.
    #[error("Psion exclusion manifest requires explicit `section_range_rules`")]
    MissingSectionRangeRules,
    /// Isolation manifest omitted contamination consequences.
    #[error("Psion exclusion manifest requires explicit contamination consequences")]
    MissingContaminationConsequences,
    /// Training near-duplicate review must be required.
    #[error("Psion exclusion manifest must require near-duplicate review before training")]
    TrainingNearDuplicateReviewMustBeRequired,
    /// Benchmark near-duplicate review must be required.
    #[error(
        "Psion exclusion manifest must require near-duplicate review before benchmark publication"
    )]
    BenchmarkNearDuplicateReviewMustBeRequired,
    /// Held-out source id repeated.
    #[error("Psion exclusion manifest repeats held-out source `{0}`")]
    DuplicateHeldOutSourceId(String),
    /// Training-excluded source id repeated.
    #[error("Psion exclusion manifest repeats training-excluded source `{0}`")]
    DuplicateTrainingExcludedSourceId(String),
    /// Benchmark-excluded source id repeated.
    #[error("Psion exclusion manifest repeats benchmark-excluded source `{0}`")]
    DuplicateBenchmarkExcludedSourceId(String),
    /// Contamination consequence repeated.
    #[error("Psion exclusion manifest repeats one contamination consequence")]
    DuplicateContaminationConsequence,
    /// Held-out source id unknown.
    #[error("Psion exclusion manifest names unknown held-out source `{source_id}`")]
    UnknownHeldOutSourceId {
        /// Unknown source id.
        source_id: String,
    },
    /// Held-out source not also training-excluded.
    #[error(
        "Psion held-out source `{source_id}` must also appear in `training_excluded_source_ids`"
    )]
    HeldOutSourceMustAlsoBeTrainingExcluded {
        /// Source id.
        source_id: String,
    },
    /// Training-excluded source id unknown.
    #[error("Psion exclusion manifest names unknown training-excluded source `{source_id}`")]
    UnknownTrainingExcludedSourceId {
        /// Unknown source id.
        source_id: String,
    },
    /// Benchmark-excluded source id unknown.
    #[error("Psion exclusion manifest names unknown benchmark-excluded source `{source_id}`")]
    UnknownBenchmarkExcludedSourceId {
        /// Unknown source id.
        source_id: String,
    },
    /// Tokenizer exposure source id missing.
    #[error("Psion tokenizer exposure report contains a row with empty `source_id`")]
    MissingExposureSourceId,
    /// Tokenizer exposure source id repeated.
    #[error("Psion tokenizer exposure report repeats source `{source_id}`")]
    DuplicateExposureSourceId {
        /// Source id.
        source_id: String,
    },
    /// Tokenizer exposure omitted detail.
    #[error("Psion tokenizer exposure record for `{source_id}` is missing `detail`")]
    MissingExposureDetail {
        /// Source id.
        source_id: String,
    },
    /// Tokenizer exposure source id unknown.
    #[error("Psion tokenizer exposure report references unknown source `{source_id}`")]
    UnknownExposureSourceId {
        /// Source id.
        source_id: String,
    },
    /// Admitted source exposure was incomplete.
    #[error(
        "Psion admitted source `{source_id}` must stay explicit in both tokenizer and model-training exposure"
    )]
    AdmittedSourceExposureIncomplete {
        /// Source id.
        source_id: String,
    },
    /// Restricted source cannot show model-training exposure.
    #[error(
        "Psion restricted source `{source_id}` cannot be marked `model_training_exposed=true`"
    )]
    RestrictedSourceCannotTrain {
        /// Source id.
        source_id: String,
    },
    /// Evaluation-only source cannot show model-training exposure.
    #[error(
        "Psion evaluation-only source `{source_id}` cannot be marked `model_training_exposed=true`"
    )]
    EvaluationOnlySourceCannotTrain {
        /// Source id.
        source_id: String,
    },
    /// Withdrawn or rejected source cannot stay exposed to training or benchmark paths.
    #[error(
        "Psion withdrawn or rejected source `{source_id}` cannot remain exposed to training or benchmark loading"
    )]
    WithdrawnOrRejectedSourceExposed {
        /// Source id.
        source_id: String,
    },
    /// Lifecycle source missing tokenizer exposure record.
    #[error("Psion source `{source_id}` is missing a tokenizer exposure record")]
    MissingExposureRecordForSource {
        /// Source id.
        source_id: String,
    },
    /// Section rule duplicated source kind.
    #[error("Psion section-range rules repeat source kind `{source_kind:?}`")]
    DuplicateSectionRule {
        /// Duplicated source kind.
        source_kind: PsionCorpusSourceKind,
    },
    /// Section rule omitted rationale.
    #[error("Psion section-range rule for `{source_kind:?}` is missing `rationale`")]
    MissingSectionRuleRationale {
        /// Source kind.
        source_kind: PsionCorpusSourceKind,
    },
    /// Required main-family section rule missing.
    #[error("Psion section-range rules are missing a rule for `{source_kind:?}`")]
    MissingRequiredSectionRule {
        /// Source kind.
        source_kind: PsionCorpusSourceKind,
    },
    /// Loader surface rejected a source id.
    #[error("Psion loader surface `{surface:?}` rejected source `{source_id}` by manifest policy")]
    LoaderSurfaceRejectedSource {
        /// Loader surface.
        surface: PsionLoaderSurface,
        /// Source id.
        source_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PsionSourceLifecycleManifest;

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

    #[test]
    fn exclusion_manifest_validates_against_lifecycle() {
        let lifecycle = lifecycle_manifest();
        let manifest = exclusion_manifest();
        manifest
            .validate_against_lifecycle(&lifecycle)
            .expect("exclusion manifest should validate");
        let tokenizer_only = manifest
            .tokenizer_exposure
            .iter()
            .find(|record| record.source_id == "vendor_manual_private_scan_v1")
            .expect("fixture should include tokenizer-only exposure record");
        assert!(tokenizer_only.tokenizer_exposed);
        assert!(!tokenizer_only.model_training_exposed);
        assert!(manifest
            .contamination_violation_consequences
            .contains(&PsionContaminationViolationConsequence::InvalidateAffectedBenchmark));
        assert!(manifest
            .contamination_violation_consequences
            .contains(&PsionContaminationViolationConsequence::TriggerCapabilityMatrixReview));
    }

    #[test]
    fn training_loader_rejects_held_out_source_ids() {
        let lifecycle = lifecycle_manifest();
        let manifest = exclusion_manifest();
        let error = manifest
            .assert_source_ids_allowed(
                &lifecycle,
                PsionLoaderSurface::ModelTraining,
                &[String::from("spec_quiz_eval_pack_v1")],
            )
            .expect_err("held-out eval-only source should be rejected for training");
        assert!(matches!(
            error,
            PsionBenchmarkIsolationError::LoaderSurfaceRejectedSource { .. }
        ));
    }

    #[test]
    fn benchmark_loader_rejects_benchmark_excluded_sources() {
        let lifecycle = lifecycle_manifest();
        let manifest = exclusion_manifest();
        let error = manifest
            .assert_source_ids_allowed(
                &lifecycle,
                PsionLoaderSurface::BenchmarkPackage,
                &[String::from("arch_textbook_foster_1985")],
            )
            .expect_err("training-admitted architecture text should be excluded from benchmark source loading");
        assert!(matches!(
            error,
            PsionBenchmarkIsolationError::LoaderSurfaceRejectedSource { .. }
        ));
    }

    #[test]
    fn tokenizer_loader_allows_tokenizer_only_source_but_not_eval_only_source() {
        let lifecycle = lifecycle_manifest();
        let manifest = exclusion_manifest();
        manifest
            .assert_source_ids_allowed(
                &lifecycle,
                PsionLoaderSurface::TokenizerTraining,
                &[String::from("vendor_manual_private_scan_v1")],
            )
            .expect("tokenizer-only source should remain tokenizer-visible");
        let error = manifest
            .assert_source_ids_allowed(
                &lifecycle,
                PsionLoaderSurface::TokenizerTraining,
                &[String::from("spec_quiz_eval_pack_v1")],
            )
            .expect_err("eval-only source should stay out of tokenizer training");
        assert!(matches!(
            error,
            PsionBenchmarkIsolationError::LoaderSurfaceRejectedSource { .. }
        ));
    }
}
