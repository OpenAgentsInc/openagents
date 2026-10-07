use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{DatasetSplitKind, PsionRawSourceManifest, PsionTokenizedCorpusManifest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for the first Psion sampling-policy manifest.
pub const PSION_SAMPLING_POLICY_SCHEMA_VERSION: &str = "psion.sampling_policy.v1";
/// Stable schema version for the first Psion sampling-policy comparison receipt.
pub const PSION_SAMPLING_POLICY_COMPARISON_SCHEMA_VERSION: &str =
    "psion.sampling_policy_comparison.v1";

/// High-level content class tracked by the Psion sampling policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSamplingContentClass {
    /// Explanatory prose such as textbooks or technical writing.
    Prose,
    /// Normative specification or manual text.
    SpecText,
    /// Source code or code-like record content.
    Code,
}

impl PsionSamplingContentClass {
    #[must_use]
    pub const fn required_classes() -> [Self; 3] {
        [Self::Prose, Self::SpecText, Self::Code]
    }
}

/// Regression dimension tracked when the sampling policy changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSamplingRegressionKind {
    /// Quality of direct system explanations.
    ExplanationQuality,
    /// Quality of specification reading and interpretation.
    SpecInterpretation,
    /// Quality of tradeoff reasoning and architectural comparison.
    TradeoffReasoning,
    /// Quality of invariant articulation and boundary awareness.
    InvariantArticulation,
    /// Coding fluency on bounded coding tasks.
    CodingFluency,
}

impl PsionSamplingRegressionKind {
    #[must_use]
    pub const fn required_kinds() -> [Self; 5] {
        [
            Self::ExplanationQuality,
            Self::SpecInterpretation,
            Self::TradeoffReasoning,
            Self::InvariantArticulation,
            Self::CodingFluency,
        ]
    }

    #[must_use]
    const fn blocks_coding_gain(self) -> bool {
        matches!(
            self,
            Self::ExplanationQuality
                | Self::SpecInterpretation
                | Self::TradeoffReasoning
                | Self::InvariantArticulation
        )
    }
}

/// Versioned weight and cap for one train-visible source family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceFamilySamplingWeight {
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// High-level content class for the family.
    pub content_class: PsionSamplingContentClass,
    /// Relative sampling weight for the family in basis points.
    pub sampling_weight_bps: u32,
    /// Maximum allowed token share for this family in basis points.
    pub maximum_family_token_share_bps: u32,
    /// Short rationale for the family posture.
    pub rationale: String,
}

/// Per-source contribution cap for one train-visible source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceContributionCap {
    /// Stable source identifier.
    pub source_id: String,
    /// Maximum allowed sampled token share for the source in basis points.
    pub maximum_source_token_share_bps: u32,
    /// Short rationale for the cap.
    pub rationale: String,
}

/// Down-weight and cap control for one repetitive raw-source region.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRepetitiveRegionControl {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable document identifier inside the raw-source manifest.
    pub document_id: String,
    /// Stable section identifier inside the raw-source manifest.
    pub section_id: String,
    /// Down-weight multiplier applied to the region in basis points.
    pub downweight_multiplier_bps: u32,
    /// Maximum allowed sampled token share for the region in basis points.
    pub maximum_region_token_share_bps: u32,
    /// Short rationale for the control.
    pub rationale: String,
}

/// Observed token-share report for one tracked content class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContentClassTokenShare {
    /// Tracked content class.
    pub content_class: PsionSamplingContentClass,
    /// Observed sampled token share in basis points.
    pub observed_token_share_bps: u32,
}

/// Maximum allowed regression for one tracked comparison dimension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSamplingRegressionThreshold {
    /// Regression dimension governed by the threshold.
    pub regression_kind: PsionSamplingRegressionKind,
    /// Maximum allowed regression in basis points.
    pub maximum_regression_bps: u32,
    /// Short rationale for the threshold.
    pub rationale: String,
}

/// First-class sampling and weighting artifact for one Psion training policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSamplingPolicyManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Tokenized-corpus schema version this policy depends on.
    pub tokenized_corpus_schema_version: String,
    /// Raw-source schema version needed for region controls.
    pub raw_source_schema_version: String,
    /// Stable dataset identity the policy applies to.
    pub dataset_identity: String,
    /// Stable sampling-policy identifier.
    pub policy_id: String,
    /// Stable sampling-policy version.
    pub policy_version: String,
    /// Packing-policy version inherited from the tokenized corpus.
    pub packing_policy_version: String,
    /// Maximum allowed code-token ratio in basis points.
    pub maximum_code_token_ratio_bps: u32,
    /// Family-level weights and token-share caps.
    pub source_family_weights: Vec<PsionSourceFamilySamplingWeight>,
    /// Per-source contribution caps.
    pub source_contribution_caps: Vec<PsionSourceContributionCap>,
    /// Repetitive-region down-weighting controls.
    pub repetitive_region_controls: Vec<PsionRepetitiveRegionControl>,
    /// Observed token-share report by tracked content class.
    pub content_class_token_share_report: Vec<PsionContentClassTokenShare>,
    /// Regression thresholds used when comparing one mixture change to another.
    pub regression_thresholds: Vec<PsionSamplingRegressionThreshold>,
}

impl PsionSamplingPolicyManifest {
    /// Creates one sampling-policy manifest and validates it against the tokenized corpus and raw-source truth.
    pub fn new(
        dataset_identity: impl Into<String>,
        policy_id: impl Into<String>,
        policy_version: impl Into<String>,
        maximum_code_token_ratio_bps: u32,
        mut source_family_weights: Vec<PsionSourceFamilySamplingWeight>,
        mut source_contribution_caps: Vec<PsionSourceContributionCap>,
        mut repetitive_region_controls: Vec<PsionRepetitiveRegionControl>,
        mut content_class_token_share_report: Vec<PsionContentClassTokenShare>,
        mut regression_thresholds: Vec<PsionSamplingRegressionThreshold>,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
        raw_source_manifest: &PsionRawSourceManifest,
    ) -> Result<Self, PsionSamplingPolicyError> {
        source_family_weights
            .sort_by(|left, right| left.source_family_id.cmp(&right.source_family_id));
        source_contribution_caps.sort_by(|left, right| left.source_id.cmp(&right.source_id));
        repetitive_region_controls.sort_by(|left, right| {
            (
                left.source_id.as_str(),
                left.document_id.as_str(),
                left.section_id.as_str(),
            )
                .cmp(&(
                    right.source_id.as_str(),
                    right.document_id.as_str(),
                    right.section_id.as_str(),
                ))
        });
        content_class_token_share_report.sort_by_key(|row| row.content_class);
        regression_thresholds.sort_by_key(|threshold| threshold.regression_kind);
        let manifest = Self {
            schema_version: String::from(PSION_SAMPLING_POLICY_SCHEMA_VERSION),
            tokenized_corpus_schema_version: tokenized_corpus.schema_version.clone(),
            raw_source_schema_version: raw_source_manifest.schema_version.clone(),
            dataset_identity: dataset_identity.into(),
            policy_id: policy_id.into(),
            policy_version: policy_version.into(),
            packing_policy_version: tokenized_corpus.packing_policy.policy_version.clone(),
            maximum_code_token_ratio_bps,
            source_family_weights,
            source_contribution_caps,
            repetitive_region_controls,
            content_class_token_share_report,
            regression_thresholds,
        };
        manifest.validate_against_inputs(tokenized_corpus, raw_source_manifest)?;
        Ok(manifest)
    }

    /// Validates the sampling policy against the tokenized corpus and raw-source region anchors.
    pub fn validate_against_inputs(
        &self,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
        raw_source_manifest: &PsionRawSourceManifest,
    ) -> Result<(), PsionSamplingPolicyError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "sampling_policy.schema_version",
        )?;
        if self.schema_version != PSION_SAMPLING_POLICY_SCHEMA_VERSION {
            return Err(PsionSamplingPolicyError::SchemaVersionMismatch {
                expected: String::from(PSION_SAMPLING_POLICY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        check_string_match(
            self.tokenized_corpus_schema_version.as_str(),
            tokenized_corpus.schema_version.as_str(),
            "tokenized_corpus_schema_version",
        )?;
        check_string_match(
            self.raw_source_schema_version.as_str(),
            raw_source_manifest.schema_version.as_str(),
            "raw_source_schema_version",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .as_str(),
            "dataset_identity",
        )?;
        ensure_nonempty(self.policy_id.as_str(), "sampling_policy.policy_id")?;
        ensure_nonempty(
            self.policy_version.as_str(),
            "sampling_policy.policy_version",
        )?;
        check_string_match(
            self.packing_policy_version.as_str(),
            tokenized_corpus.packing_policy.policy_version.as_str(),
            "packing_policy_version",
        )?;
        check_bps(
            self.maximum_code_token_ratio_bps,
            "maximum_code_token_ratio_bps",
        )?;

        let eligible_corpus = collect_train_eligible_sources(tokenized_corpus)?;
        let raw_source_map = raw_source_manifest
            .sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();

        let (family_class_map, family_caps_by_class) =
            self.validate_source_family_weights(&eligible_corpus)?;
        let source_caps =
            self.validate_source_contribution_caps(&eligible_corpus, &family_class_map)?;
        self.validate_repetitive_region_controls(&eligible_corpus, raw_source_map, &source_caps)?;
        self.validate_content_class_token_share_report(family_caps_by_class)?;
        self.validate_regression_thresholds()?;
        Ok(())
    }

    /// Returns the threshold for one regression dimension when present.
    #[must_use]
    pub fn regression_threshold(
        &self,
        regression_kind: PsionSamplingRegressionKind,
    ) -> Option<&PsionSamplingRegressionThreshold> {
        self.regression_thresholds
            .iter()
            .find(|threshold| threshold.regression_kind == regression_kind)
    }

    fn validate_source_family_weights(
        &self,
        eligible_corpus: &TrainEligibleCorpus,
    ) -> Result<
        (
            BTreeMap<String, PsionSamplingContentClass>,
            BTreeMap<PsionSamplingContentClass, u32>,
        ),
        PsionSamplingPolicyError,
    > {
        if self.source_family_weights.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy.source_family_weights"),
            });
        }
        let mut seen_families = BTreeSet::new();
        let mut total_weight_bps = 0_u32;
        let mut family_class_map = BTreeMap::new();
        let mut family_caps_by_class = BTreeMap::new();
        for family in &self.source_family_weights {
            ensure_nonempty(
                family.source_family_id.as_str(),
                "sampling_policy.source_family_weights[].source_family_id",
            )?;
            if !seen_families.insert(family.source_family_id.clone()) {
                return Err(PsionSamplingPolicyError::DuplicateSourceFamilyWeight {
                    source_family_id: family.source_family_id.clone(),
                });
            }
            if !eligible_corpus
                .sources_by_family
                .contains_key(family.source_family_id.as_str())
            {
                return Err(PsionSamplingPolicyError::UnknownSourceFamilyId {
                    source_family_id: family.source_family_id.clone(),
                });
            }
            ensure_positive_bps(
                family.sampling_weight_bps,
                &format!(
                    "source_family_weights.{}.sampling_weight_bps",
                    family.source_family_id
                ),
            )?;
            ensure_positive_bps(
                family.maximum_family_token_share_bps,
                &format!(
                    "source_family_weights.{}.maximum_family_token_share_bps",
                    family.source_family_id
                ),
            )?;
            ensure_nonempty(
                family.rationale.as_str(),
                "sampling_policy.source_family_weights[].rationale",
            )?;
            total_weight_bps = total_weight_bps.saturating_add(family.sampling_weight_bps);
            family_class_map.insert(family.source_family_id.clone(), family.content_class);
            let entry = family_caps_by_class
                .entry(family.content_class)
                .or_insert(0_u32);
            *entry = entry.saturating_add(family.maximum_family_token_share_bps);
        }
        let eligible_families = eligible_corpus
            .sources_by_family
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if seen_families != eligible_families {
            return Err(PsionSamplingPolicyError::SourceFamilyCoverageMismatch);
        }
        if total_weight_bps != 10_000 {
            return Err(PsionSamplingPolicyError::InvalidBpsTotal {
                field: String::from("source_family_weights.sampling_weight_bps"),
                expected_total_bps: 10_000,
                actual_total_bps: total_weight_bps,
            });
        }
        if family_caps_by_class
            .get(&PsionSamplingContentClass::Code)
            .copied()
            .unwrap_or(0)
            > self.maximum_code_token_ratio_bps
        {
            return Err(PsionSamplingPolicyError::CodeTokenRatioExceeded {
                observed_token_share_bps: family_caps_by_class
                    .get(&PsionSamplingContentClass::Code)
                    .copied()
                    .unwrap_or(0),
                maximum_code_token_ratio_bps: self.maximum_code_token_ratio_bps,
            });
        }
        Ok((family_class_map, family_caps_by_class))
    }

    fn validate_source_contribution_caps(
        &self,
        eligible_corpus: &TrainEligibleCorpus,
        family_class_map: &BTreeMap<String, PsionSamplingContentClass>,
    ) -> Result<BTreeMap<String, u32>, PsionSamplingPolicyError> {
        if self.source_contribution_caps.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy.source_contribution_caps"),
            });
        }
        let mut source_caps = BTreeMap::new();
        for source_cap in &self.source_contribution_caps {
            ensure_nonempty(
                source_cap.source_id.as_str(),
                "sampling_policy.source_contribution_caps[].source_id",
            )?;
            if source_caps.contains_key(source_cap.source_id.as_str()) {
                return Err(PsionSamplingPolicyError::DuplicateSourceContributionCap {
                    source_id: source_cap.source_id.clone(),
                });
            }
            let Some(source_family_id) = eligible_corpus
                .source_family_by_source
                .get(source_cap.source_id.as_str())
            else {
                return Err(PsionSamplingPolicyError::UnknownSourceId {
                    source_id: source_cap.source_id.clone(),
                });
            };
            ensure_positive_bps(
                source_cap.maximum_source_token_share_bps,
                &format!(
                    "source_contribution_caps.{}.maximum_source_token_share_bps",
                    source_cap.source_id
                ),
            )?;
            ensure_nonempty(
                source_cap.rationale.as_str(),
                "sampling_policy.source_contribution_caps[].rationale",
            )?;
            let family_cap = self
                .source_family_weights
                .iter()
                .find(|family| family.source_family_id == **source_family_id)
                .expect("eligible source families should already be covered")
                .maximum_family_token_share_bps;
            if source_cap.maximum_source_token_share_bps > family_cap {
                return Err(PsionSamplingPolicyError::FieldMismatch {
                    field: format!(
                        "source_contribution_caps.{}.maximum_source_token_share_bps",
                        source_cap.source_id
                    ),
                    expected: family_cap.to_string(),
                    actual: source_cap.maximum_source_token_share_bps.to_string(),
                });
            }
            let _ = family_class_map
                .get(source_family_id.as_str())
                .expect("family class coverage should be complete");
            source_caps.insert(
                source_cap.source_id.clone(),
                source_cap.maximum_source_token_share_bps,
            );
        }
        let eligible_sources = eligible_corpus
            .source_family_by_source
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let capped_sources = source_caps.keys().cloned().collect::<BTreeSet<_>>();
        if eligible_sources != capped_sources {
            return Err(PsionSamplingPolicyError::SourceContributionCoverageMismatch);
        }
        Ok(source_caps)
    }

    fn validate_repetitive_region_controls(
        &self,
        eligible_corpus: &TrainEligibleCorpus,
        raw_source_map: BTreeMap<&str, &psionic_data::PsionRawSourceRecord>,
        source_caps: &BTreeMap<String, u32>,
    ) -> Result<(), PsionSamplingPolicyError> {
        if self.repetitive_region_controls.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy.repetitive_region_controls"),
            });
        }
        let mut seen_regions = BTreeSet::new();
        for region in &self.repetitive_region_controls {
            ensure_nonempty(
                region.source_id.as_str(),
                "sampling_policy.repetitive_region_controls[].source_id",
            )?;
            ensure_nonempty(
                region.document_id.as_str(),
                "sampling_policy.repetitive_region_controls[].document_id",
            )?;
            ensure_nonempty(
                region.section_id.as_str(),
                "sampling_policy.repetitive_region_controls[].section_id",
            )?;
            ensure_nonempty(
                region.rationale.as_str(),
                "sampling_policy.repetitive_region_controls[].rationale",
            )?;
            if !seen_regions.insert((
                region.source_id.clone(),
                region.document_id.clone(),
                region.section_id.clone(),
            )) {
                return Err(PsionSamplingPolicyError::DuplicateRegionControl {
                    source_id: region.source_id.clone(),
                    section_id: region.section_id.clone(),
                });
            }
            if !eligible_corpus
                .source_family_by_source
                .contains_key(region.source_id.as_str())
            {
                return Err(PsionSamplingPolicyError::UnknownSourceId {
                    source_id: region.source_id.clone(),
                });
            }
            let Some(raw_source) = raw_source_map.get(region.source_id.as_str()) else {
                return Err(PsionSamplingPolicyError::UnknownSourceId {
                    source_id: region.source_id.clone(),
                });
            };
            let Some(document) = raw_source
                .documents
                .iter()
                .find(|document| document.document_id == region.document_id)
            else {
                return Err(PsionSamplingPolicyError::UnknownDocumentId {
                    source_id: region.source_id.clone(),
                    document_id: region.document_id.clone(),
                });
            };
            if document
                .section_boundaries
                .iter()
                .all(|section| section.section_id != region.section_id)
            {
                return Err(PsionSamplingPolicyError::UnknownSectionId {
                    source_id: region.source_id.clone(),
                    section_id: region.section_id.clone(),
                });
            }
            if region.downweight_multiplier_bps == 0 || region.downweight_multiplier_bps >= 10_000 {
                return Err(PsionSamplingPolicyError::InvalidDownweightMultiplier {
                    source_id: region.source_id.clone(),
                    section_id: region.section_id.clone(),
                    downweight_multiplier_bps: region.downweight_multiplier_bps,
                });
            }
            ensure_positive_bps(
                region.maximum_region_token_share_bps,
                &format!(
                    "repetitive_region_controls.{}.{}.maximum_region_token_share_bps",
                    region.source_id, region.section_id
                ),
            )?;
            let source_cap = source_caps
                .get(region.source_id.as_str())
                .expect("source caps should cover every eligible source");
            if region.maximum_region_token_share_bps > *source_cap {
                return Err(PsionSamplingPolicyError::FieldMismatch {
                    field: format!(
                        "repetitive_region_controls.{}.{}.maximum_region_token_share_bps",
                        region.source_id, region.section_id
                    ),
                    expected: source_cap.to_string(),
                    actual: region.maximum_region_token_share_bps.to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_content_class_token_share_report(
        &self,
        family_caps_by_class: BTreeMap<PsionSamplingContentClass, u32>,
    ) -> Result<(), PsionSamplingPolicyError> {
        if self.content_class_token_share_report.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy.content_class_token_share_report"),
            });
        }
        let mut seen_classes = BTreeSet::new();
        let mut total_share_bps = 0_u32;
        for row in &self.content_class_token_share_report {
            if !seen_classes.insert(row.content_class) {
                return Err(PsionSamplingPolicyError::DuplicateContentClassReport {
                    content_class: row.content_class,
                });
            }
            check_bps(
                row.observed_token_share_bps,
                &format!(
                    "content_class_token_share_report.{:?}.observed_token_share_bps",
                    row.content_class
                ),
            )?;
            total_share_bps = total_share_bps.saturating_add(row.observed_token_share_bps);
            let class_cap = family_caps_by_class
                .get(&row.content_class)
                .copied()
                .unwrap_or(0);
            if row.observed_token_share_bps > class_cap {
                return Err(PsionSamplingPolicyError::ContentClassCapExceeded {
                    content_class: row.content_class,
                    observed_token_share_bps: row.observed_token_share_bps,
                    maximum_token_share_bps: class_cap,
                });
            }
        }
        let required_classes = PsionSamplingContentClass::required_classes()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if seen_classes != required_classes {
            return Err(PsionSamplingPolicyError::ContentClassCoverageMismatch);
        }
        if total_share_bps != 10_000 {
            return Err(PsionSamplingPolicyError::InvalidBpsTotal {
                field: String::from("content_class_token_share_report.observed_token_share_bps"),
                expected_total_bps: 10_000,
                actual_total_bps: total_share_bps,
            });
        }
        let code_share = self
            .content_class_token_share_report
            .iter()
            .find(|row| row.content_class == PsionSamplingContentClass::Code)
            .expect("code coverage should be complete")
            .observed_token_share_bps;
        if code_share > self.maximum_code_token_ratio_bps {
            return Err(PsionSamplingPolicyError::CodeTokenRatioExceeded {
                observed_token_share_bps: code_share,
                maximum_code_token_ratio_bps: self.maximum_code_token_ratio_bps,
            });
        }
        Ok(())
    }

    fn validate_regression_thresholds(&self) -> Result<(), PsionSamplingPolicyError> {
        if self.regression_thresholds.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy.regression_thresholds"),
            });
        }
        let mut seen_kinds = BTreeSet::new();
        for threshold in &self.regression_thresholds {
            if !seen_kinds.insert(threshold.regression_kind) {
                return Err(PsionSamplingPolicyError::DuplicateRegressionThreshold {
                    regression_kind: threshold.regression_kind,
                });
            }
            check_bps(
                threshold.maximum_regression_bps,
                &format!(
                    "regression_thresholds.{:?}.maximum_regression_bps",
                    threshold.regression_kind
                ),
            )?;
            ensure_nonempty(
                threshold.rationale.as_str(),
                "sampling_policy.regression_thresholds[].rationale",
            )?;
        }
        let required_kinds = PsionSamplingRegressionKind::required_kinds()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if seen_kinds != required_kinds {
            return Err(PsionSamplingPolicyError::RegressionThresholdCoverageMismatch);
        }
        Ok(())
    }
}

/// Token-share delta recorded when comparing two sampling policies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContentClassTokenShareDelta {
    /// Tracked content class.
    pub content_class: PsionSamplingContentClass,
    /// Baseline token share in basis points.
    pub baseline_token_share_bps: u32,
    /// Candidate token share in basis points.
    pub candidate_token_share_bps: u32,
    /// Candidate minus baseline token share in basis points.
    pub delta_bps: i32,
}

/// Benchmark-style regression metric recorded for one policy comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSamplingRegressionMetric {
    /// Regression dimension under review.
    pub regression_kind: PsionSamplingRegressionKind,
    /// Baseline score in basis points.
    pub baseline_score_bps: u32,
    /// Candidate score in basis points.
    pub candidate_score_bps: u32,
    /// Measured regression from the baseline in basis points.
    pub regression_bps: u32,
}

/// Receipt proving one policy change was compared against reasoning regressions, not only LM loss.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSamplingPolicyComparisonReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable dataset identity shared by the compared policies.
    pub dataset_identity: String,
    /// Baseline policy identifier.
    pub baseline_policy_id: String,
    /// Baseline policy version.
    pub baseline_policy_version: String,
    /// Candidate policy identifier.
    pub candidate_policy_id: String,
    /// Candidate policy version.
    pub candidate_policy_version: String,
    /// Candidate minus baseline LM loss delta in basis points.
    pub lm_loss_delta_bps: i32,
    /// Token-share deltas across prose, spec text, and code.
    pub token_share_deltas: Vec<PsionContentClassTokenShareDelta>,
    /// Reasoning and coding comparison metrics.
    pub regression_metrics: Vec<PsionSamplingRegressionMetric>,
    /// Short summary of the policy change.
    pub summary: String,
}

impl PsionSamplingPolicyComparisonReceipt {
    /// Creates a comparison receipt and validates it against the baseline and candidate policies.
    pub fn new(
        lm_loss_delta_bps: i32,
        mut token_share_deltas: Vec<PsionContentClassTokenShareDelta>,
        mut regression_metrics: Vec<PsionSamplingRegressionMetric>,
        summary: impl Into<String>,
        baseline_policy: &PsionSamplingPolicyManifest,
        candidate_policy: &PsionSamplingPolicyManifest,
    ) -> Result<Self, PsionSamplingPolicyError> {
        token_share_deltas.sort_by_key(|delta| delta.content_class);
        regression_metrics.sort_by_key(|metric| metric.regression_kind);
        let receipt = Self {
            schema_version: String::from(PSION_SAMPLING_POLICY_COMPARISON_SCHEMA_VERSION),
            dataset_identity: candidate_policy.dataset_identity.clone(),
            baseline_policy_id: baseline_policy.policy_id.clone(),
            baseline_policy_version: baseline_policy.policy_version.clone(),
            candidate_policy_id: candidate_policy.policy_id.clone(),
            candidate_policy_version: candidate_policy.policy_version.clone(),
            lm_loss_delta_bps,
            token_share_deltas,
            regression_metrics,
            summary: summary.into(),
        };
        receipt.validate_against_policies(baseline_policy, candidate_policy)?;
        Ok(receipt)
    }

    /// Validates the comparison receipt against the compared policy artifacts.
    pub fn validate_against_policies(
        &self,
        baseline_policy: &PsionSamplingPolicyManifest,
        candidate_policy: &PsionSamplingPolicyManifest,
    ) -> Result<(), PsionSamplingPolicyError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "sampling_policy_comparison.schema_version",
        )?;
        if self.schema_version != PSION_SAMPLING_POLICY_COMPARISON_SCHEMA_VERSION {
            return Err(PsionSamplingPolicyError::SchemaVersionMismatch {
                expected: String::from(PSION_SAMPLING_POLICY_COMPARISON_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        check_string_match(
            self.dataset_identity.as_str(),
            candidate_policy.dataset_identity.as_str(),
            "comparison.dataset_identity",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            baseline_policy.dataset_identity.as_str(),
            "comparison.dataset_identity",
        )?;
        check_string_match(
            self.baseline_policy_id.as_str(),
            baseline_policy.policy_id.as_str(),
            "comparison.baseline_policy_id",
        )?;
        check_string_match(
            self.baseline_policy_version.as_str(),
            baseline_policy.policy_version.as_str(),
            "comparison.baseline_policy_version",
        )?;
        check_string_match(
            self.candidate_policy_id.as_str(),
            candidate_policy.policy_id.as_str(),
            "comparison.candidate_policy_id",
        )?;
        check_string_match(
            self.candidate_policy_version.as_str(),
            candidate_policy.policy_version.as_str(),
            "comparison.candidate_policy_version",
        )?;
        ensure_nonempty(self.summary.as_str(), "sampling_policy_comparison.summary")?;

        self.validate_token_share_deltas(baseline_policy, candidate_policy)?;
        self.validate_regression_metrics(baseline_policy, candidate_policy)?;
        Ok(())
    }

    fn validate_token_share_deltas(
        &self,
        baseline_policy: &PsionSamplingPolicyManifest,
        candidate_policy: &PsionSamplingPolicyManifest,
    ) -> Result<(), PsionSamplingPolicyError> {
        if self.token_share_deltas.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy_comparison.token_share_deltas"),
            });
        }
        let baseline_report = baseline_policy
            .content_class_token_share_report
            .iter()
            .map(|row| (row.content_class, row.observed_token_share_bps))
            .collect::<BTreeMap<_, _>>();
        let candidate_report = candidate_policy
            .content_class_token_share_report
            .iter()
            .map(|row| (row.content_class, row.observed_token_share_bps))
            .collect::<BTreeMap<_, _>>();
        let mut seen_classes = BTreeSet::new();
        for delta in &self.token_share_deltas {
            if !seen_classes.insert(delta.content_class) {
                return Err(PsionSamplingPolicyError::DuplicateTokenShareDelta {
                    content_class: delta.content_class,
                });
            }
            let baseline_share = baseline_report
                .get(&delta.content_class)
                .copied()
                .expect("baseline token-share coverage should be complete");
            let candidate_share = candidate_report
                .get(&delta.content_class)
                .copied()
                .expect("candidate token-share coverage should be complete");
            let expected_delta = i32::try_from(candidate_share).expect("token shares fit in i32")
                - i32::try_from(baseline_share).expect("token shares fit in i32");
            if delta.baseline_token_share_bps != baseline_share
                || delta.candidate_token_share_bps != candidate_share
                || delta.delta_bps != expected_delta
            {
                return Err(PsionSamplingPolicyError::TokenShareDeltaMismatch {
                    content_class: delta.content_class,
                });
            }
        }
        let required_classes = PsionSamplingContentClass::required_classes()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if seen_classes != required_classes {
            return Err(PsionSamplingPolicyError::ContentClassCoverageMismatch);
        }
        Ok(())
    }

    fn validate_regression_metrics(
        &self,
        baseline_policy: &PsionSamplingPolicyManifest,
        candidate_policy: &PsionSamplingPolicyManifest,
    ) -> Result<(), PsionSamplingPolicyError> {
        if self.regression_metrics.is_empty() {
            return Err(PsionSamplingPolicyError::MissingField {
                field: String::from("sampling_policy_comparison.regression_metrics"),
            });
        }
        let mut seen_kinds = BTreeSet::new();
        let mut coding_gain_bps = 0_u32;
        let mut reasoning_regressions = Vec::new();
        for metric in &self.regression_metrics {
            if !seen_kinds.insert(metric.regression_kind) {
                return Err(PsionSamplingPolicyError::DuplicateRegressionMetric {
                    regression_kind: metric.regression_kind,
                });
            }
            check_bps(
                metric.baseline_score_bps,
                &format!(
                    "regression_metrics.{:?}.baseline_score_bps",
                    metric.regression_kind
                ),
            )?;
            check_bps(
                metric.candidate_score_bps,
                &format!(
                    "regression_metrics.{:?}.candidate_score_bps",
                    metric.regression_kind
                ),
            )?;
            let expected_regression_bps = metric
                .baseline_score_bps
                .saturating_sub(metric.candidate_score_bps);
            if metric.regression_bps != expected_regression_bps {
                return Err(PsionSamplingPolicyError::RegressionMetricMismatch {
                    regression_kind: metric.regression_kind,
                });
            }
            let threshold = candidate_policy
                .regression_threshold(metric.regression_kind)
                .expect("candidate regression thresholds should be complete");
            if metric.regression_bps > threshold.maximum_regression_bps {
                return Err(PsionSamplingPolicyError::RegressionThresholdMissed {
                    regression_kind: metric.regression_kind,
                    regression_bps: metric.regression_bps,
                    maximum_regression_bps: threshold.maximum_regression_bps,
                });
            }
            if metric.regression_kind == PsionSamplingRegressionKind::CodingFluency {
                coding_gain_bps = metric
                    .candidate_score_bps
                    .saturating_sub(metric.baseline_score_bps);
            } else if metric.regression_kind.blocks_coding_gain() && metric.regression_bps > 0 {
                reasoning_regressions.push((metric.regression_kind, metric.regression_bps));
            }
        }
        let required_kinds = PsionSamplingRegressionKind::required_kinds()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if seen_kinds != required_kinds {
            return Err(PsionSamplingPolicyError::RegressionThresholdCoverageMismatch);
        }
        if coding_gain_bps > 0 {
            if let Some((regression_kind, regression_bps)) = reasoning_regressions.first().copied()
            {
                return Err(PsionSamplingPolicyError::CodingGainAtReasoningExpense {
                    regression_kind,
                    coding_gain_bps,
                    regression_bps,
                });
            }
        }
        let _ = baseline_policy;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct TrainEligibleCorpus {
    sources_by_family: BTreeMap<String, BTreeSet<String>>,
    source_family_by_source: BTreeMap<String, String>,
}

fn collect_train_eligible_sources(
    tokenized_corpus: &PsionTokenizedCorpusManifest,
) -> Result<TrainEligibleCorpus, PsionSamplingPolicyError> {
    let mut sources_by_family = BTreeMap::new();
    let mut source_family_by_source = BTreeMap::new();
    for shard in &tokenized_corpus.shards {
        if matches!(
            shard.split_kind,
            DatasetSplitKind::HeldOut | DatasetSplitKind::Test
        ) {
            continue;
        }
        for lineage in &shard.source_lineage {
            sources_by_family
                .entry(lineage.source_family_id.clone())
                .or_insert_with(BTreeSet::new)
                .insert(lineage.source_id.clone());
            source_family_by_source
                .entry(lineage.source_id.clone())
                .or_insert_with(|| lineage.source_family_id.clone());
        }
    }
    if sources_by_family.is_empty() || source_family_by_source.is_empty() {
        return Err(PsionSamplingPolicyError::MissingTrainEligibleSources);
    }
    Ok(TrainEligibleCorpus {
        sources_by_family,
        source_family_by_source,
    })
}

/// Error returned by the Psion sampling-policy contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionSamplingPolicyError {
    /// One required field was missing or empty.
    #[error("Psion sampling-policy field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version drifted from the expected contract.
    #[error("Psion sampling-policy expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One field drifted from the bound input artifacts.
    #[error("Psion sampling-policy field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// A basis-point value was outside the accepted range.
    #[error("Psion sampling-policy field `{field}` must stay within 0..=10000 basis points, found `{actual_bps}`")]
    InvalidBpsValue {
        /// Field name.
        field: String,
        /// Actual value.
        actual_bps: u32,
    },
    /// One grouped basis-point total did not equal 10000.
    #[error("Psion sampling-policy field `{field}` must total `{expected_total_bps}` basis points, found `{actual_total_bps}`")]
    InvalidBpsTotal {
        /// Field name.
        field: String,
        /// Expected total.
        expected_total_bps: u32,
        /// Actual total.
        actual_total_bps: u32,
    },
    /// The tokenized corpus did not expose any train-visible sources.
    #[error(
        "Psion sampling-policy requires at least one train-visible source in the tokenized corpus"
    )]
    MissingTrainEligibleSources,
    /// One source family id was not train-visible in the tokenized corpus.
    #[error(
        "Psion sampling-policy does not know train-visible source family `{source_family_id}`"
    )]
    UnknownSourceFamilyId {
        /// Unknown source-family identifier.
        source_family_id: String,
    },
    /// One source id was not train-visible in the tokenized corpus or raw-source manifest.
    #[error("Psion sampling-policy does not know source `{source_id}`")]
    UnknownSourceId {
        /// Unknown source identifier.
        source_id: String,
    },
    /// One document id was not present for the source in the raw-source manifest.
    #[error(
        "Psion sampling-policy does not know document `{document_id}` for source `{source_id}`"
    )]
    UnknownDocumentId {
        /// Source identifier.
        source_id: String,
        /// Unknown document identifier.
        document_id: String,
    },
    /// One section id was not present for the source in the raw-source manifest.
    #[error("Psion sampling-policy does not know section `{section_id}` for source `{source_id}`")]
    UnknownSectionId {
        /// Source identifier.
        source_id: String,
        /// Unknown section identifier.
        section_id: String,
    },
    /// One source-family weight row was repeated.
    #[error("Psion sampling-policy repeated source family weight `{source_family_id}`")]
    DuplicateSourceFamilyWeight {
        /// Repeated source-family identifier.
        source_family_id: String,
    },
    /// Source-family weights did not cover exactly the train-visible families.
    #[error("Psion sampling-policy source-family weights must cover exactly the train-visible source families")]
    SourceFamilyCoverageMismatch,
    /// One per-source contribution cap was repeated.
    #[error("Psion sampling-policy repeated source contribution cap `{source_id}`")]
    DuplicateSourceContributionCap {
        /// Repeated source identifier.
        source_id: String,
    },
    /// Source contribution caps did not cover exactly the train-visible sources.
    #[error("Psion sampling-policy source contribution caps must cover exactly the train-visible sources")]
    SourceContributionCoverageMismatch,
    /// One repetitive region control was repeated.
    #[error("Psion sampling-policy repeated region control for source `{source_id}` section `{section_id}`")]
    DuplicateRegionControl {
        /// Source identifier.
        source_id: String,
        /// Section identifier.
        section_id: String,
    },
    /// One region down-weight multiplier was not a real down-weight.
    #[error("Psion sampling-policy region control for source `{source_id}` section `{section_id}` must use a down-weight multiplier in 1..9999 basis points, found `{downweight_multiplier_bps}`")]
    InvalidDownweightMultiplier {
        /// Source identifier.
        source_id: String,
        /// Section identifier.
        section_id: String,
        /// Invalid multiplier.
        downweight_multiplier_bps: u32,
    },
    /// One content-class token-share report row was repeated.
    #[error("Psion sampling-policy repeated content-class report `{content_class:?}`")]
    DuplicateContentClassReport {
        /// Repeated content class.
        content_class: PsionSamplingContentClass,
    },
    /// Content-class token-share reporting did not cover prose, spec text, and code exactly once.
    #[error("Psion sampling-policy content-class reporting must cover prose, spec text, and code exactly once")]
    ContentClassCoverageMismatch,
    /// Observed token share exceeded the class-level cap derived from family caps.
    #[error("Psion sampling-policy content class `{content_class:?}` observed `{observed_token_share_bps}` basis points above maximum `{maximum_token_share_bps}`")]
    ContentClassCapExceeded {
        /// Content class.
        content_class: PsionSamplingContentClass,
        /// Observed token share.
        observed_token_share_bps: u32,
        /// Maximum token share allowed.
        maximum_token_share_bps: u32,
    },
    /// Observed or allowed code token share exceeded the code-dominance limit.
    #[error("Psion sampling-policy code token share `{observed_token_share_bps}` exceeded maximum `{maximum_code_token_ratio_bps}`")]
    CodeTokenRatioExceeded {
        /// Observed token share.
        observed_token_share_bps: u32,
        /// Maximum code-token ratio.
        maximum_code_token_ratio_bps: u32,
    },
    /// One regression threshold row was repeated.
    #[error("Psion sampling-policy repeated regression threshold `{regression_kind:?}`")]
    DuplicateRegressionThreshold {
        /// Repeated regression kind.
        regression_kind: PsionSamplingRegressionKind,
    },
    /// Regression thresholds or metrics did not cover the required dimensions.
    #[error("Psion sampling-policy regression coverage must include explanation quality, spec interpretation, tradeoff reasoning, invariant articulation, and coding fluency")]
    RegressionThresholdCoverageMismatch,
    /// One token-share delta row was repeated.
    #[error("Psion sampling-policy comparison repeated token-share delta `{content_class:?}`")]
    DuplicateTokenShareDelta {
        /// Repeated content class.
        content_class: PsionSamplingContentClass,
    },
    /// One token-share delta row drifted from the compared policies.
    #[error("Psion sampling-policy comparison token-share delta for `{content_class:?}` drifted from the compared policies")]
    TokenShareDeltaMismatch {
        /// Content class.
        content_class: PsionSamplingContentClass,
    },
    /// One regression metric row was repeated.
    #[error("Psion sampling-policy comparison repeated regression metric `{regression_kind:?}`")]
    DuplicateRegressionMetric {
        /// Repeated regression kind.
        regression_kind: PsionSamplingRegressionKind,
    },
    /// One regression metric row drifted from the compared policies.
    #[error("Psion sampling-policy comparison regression metric `{regression_kind:?}` drifted from the compared policies")]
    RegressionMetricMismatch {
        /// Regression kind.
        regression_kind: PsionSamplingRegressionKind,
    },
    /// One regression exceeded the allowed threshold.
    #[error("Psion sampling-policy comparison regression `{regression_kind:?}` of `{regression_bps}` exceeded maximum `{maximum_regression_bps}`")]
    RegressionThresholdMissed {
        /// Regression kind.
        regression_kind: PsionSamplingRegressionKind,
        /// Observed regression.
        regression_bps: u32,
        /// Maximum allowed regression.
        maximum_regression_bps: u32,
    },
    /// Coding fluency improved while one reasoning metric regressed.
    #[error("Psion sampling-policy comparison cannot improve coding fluency by `{coding_gain_bps}` while `{regression_kind:?}` regressed by `{regression_bps}`")]
    CodingGainAtReasoningExpense {
        /// Regression kind that regressed.
        regression_kind: PsionSamplingRegressionKind,
        /// Coding gain observed.
        coding_gain_bps: u32,
        /// Reasoning regression observed.
        regression_bps: u32,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionSamplingPolicyError> {
    if value.trim().is_empty() {
        return Err(PsionSamplingPolicyError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionSamplingPolicyError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionSamplingPolicyError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn check_bps(value: u32, field: &str) -> Result<(), PsionSamplingPolicyError> {
    if value > 10_000 {
        return Err(PsionSamplingPolicyError::InvalidBpsValue {
            field: String::from(field),
            actual_bps: value,
        });
    }
    Ok(())
}

fn ensure_positive_bps(value: u32, field: &str) -> Result<(), PsionSamplingPolicyError> {
    check_bps(value, field)?;
    if value == 0 {
        return Err(PsionSamplingPolicyError::InvalidBpsValue {
            field: String::from(field),
            actual_bps: value,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_data::{PsionRawSourceManifest, PsionTokenizedCorpusManifest};

    fn raw_source_manifest() -> PsionRawSourceManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/ingestion/psion_raw_source_manifest_v1.json"
        ))
        .expect("raw-source manifest should parse")
    }

    fn tokenized_corpus() -> PsionTokenizedCorpusManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"
        ))
        .expect("tokenized corpus should parse")
    }

    fn baseline_policy() -> PsionSamplingPolicyManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/sampling/psion_sampling_policy_baseline_v1.json"
        ))
        .expect("baseline sampling policy should parse")
    }

    fn candidate_policy() -> PsionSamplingPolicyManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"
        ))
        .expect("candidate sampling policy should parse")
    }

    fn code_heavy_policy() -> PsionSamplingPolicyManifest {
        let mut policy = candidate_policy();
        policy.maximum_code_token_ratio_bps = 100;
        policy.source_family_weights[0].content_class = PsionSamplingContentClass::Code;
        policy.content_class_token_share_report = vec![
            PsionContentClassTokenShare {
                content_class: PsionSamplingContentClass::Prose,
                observed_token_share_bps: 0,
            },
            PsionContentClassTokenShare {
                content_class: PsionSamplingContentClass::SpecText,
                observed_token_share_bps: 4400,
            },
            PsionContentClassTokenShare {
                content_class: PsionSamplingContentClass::Code,
                observed_token_share_bps: 5600,
            },
        ];
        policy
    }

    fn comparison_receipt() -> PsionSamplingPolicyComparisonReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/sampling/psion_sampling_policy_comparison_receipt_v1.json"
        ))
        .expect("sampling-policy comparison receipt should parse")
    }

    #[test]
    fn sampling_policy_manifest_validates_weights_caps_and_regions() {
        candidate_policy()
            .validate_against_inputs(&tokenized_corpus(), &raw_source_manifest())
            .expect("sampling policy manifest should validate");
    }

    #[test]
    fn code_token_ratio_must_not_exceed_the_declared_limit() {
        let error = code_heavy_policy()
            .validate_against_inputs(&tokenized_corpus(), &raw_source_manifest())
            .expect_err("code token ratio above the maximum should be rejected");
        assert!(matches!(
            error,
            PsionSamplingPolicyError::CodeTokenRatioExceeded { .. }
        ));
    }

    #[test]
    fn comparison_receipt_validates_against_baseline_and_candidate() {
        comparison_receipt()
            .validate_against_policies(&baseline_policy(), &candidate_policy())
            .expect("comparison receipt should validate");
    }

    #[test]
    fn coding_gain_cannot_hide_reasoning_regression() {
        let baseline = baseline_policy();
        let candidate = candidate_policy();
        let mut receipt = comparison_receipt();
        let tradeoff_metric = receipt
            .regression_metrics
            .iter_mut()
            .find(|metric| metric.regression_kind == PsionSamplingRegressionKind::TradeoffReasoning)
            .expect("tradeoff metric should exist");
        tradeoff_metric.candidate_score_bps = 8830;
        tradeoff_metric.regression_bps = 10;
        let error = receipt
            .validate_against_policies(&baseline, &candidate)
            .expect_err("coding gain at the expense of reasoning should be rejected");
        assert!(matches!(
            error,
            PsionSamplingPolicyError::CodingGainAtReasoningExpense { .. }
        ));
    }
}
