use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef};

/// Stable schema version for the canonical actual-lane data bundle.
pub const PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_data_bundle.v1";

/// Stable data-bundle identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_ID: &str = "psion_actual_pretraining_data_bundle_v1";

/// Canonical fixture path for the actual-lane data bundle.
pub const PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json";

/// Canonical focused doc path for the actual-lane data bundle.
pub const PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_DATA_BUNDLE.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDataTransformationStage {
    pub stage_id: String,
    pub stage_kind: String,
    pub source_artifact: PsionActualPretrainingArtifactRef,
    pub output_identity: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDataFilterAuthority {
    pub admission_policy: PsionActualPretrainingArtifactRef,
    pub source_admission_manifest: PsionActualPretrainingArtifactRef,
    pub source_lifecycle_manifest: PsionActualPretrainingArtifactRef,
    pub benchmark_isolation_manifest: PsionActualPretrainingArtifactRef,
    pub admitted_training_source_ids: Vec<String>,
    pub tokenizer_only_source_ids: Vec<String>,
    pub held_out_source_ids: Vec<String>,
    pub rejected_source_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRepetitiveRegionControl {
    pub source_id: String,
    pub document_id: String,
    pub section_id: String,
    pub downweight_multiplier_bps: u32,
    pub maximum_region_token_share_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDataDedupAuthority {
    pub near_duplicate_review_required_before_training: bool,
    pub near_duplicate_review_required_before_benchmark_publication: bool,
    pub near_duplicate_review_ref: String,
    pub training_excluded_source_ids: Vec<String>,
    pub benchmark_excluded_source_ids: Vec<String>,
    pub repetitive_region_controls: Vec<PsionActualPretrainingRepetitiveRegionControl>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingFamilyMixtureWeight {
    pub source_family_id: String,
    pub content_class: String,
    pub sampling_weight_bps: u32,
    pub maximum_family_token_share_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingSourceContributionCap {
    pub source_id: String,
    pub maximum_source_token_share_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContentClassShare {
    pub content_class: String,
    pub observed_token_share_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRegressionGate {
    pub regression_kind: String,
    pub maximum_regression_bps: u32,
    pub observed_regression_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingMixtureAuthority {
    pub dataset_identity: String,
    pub sampling_policy_id: String,
    pub sampling_policy_version: String,
    pub sampling_policy_manifest: PsionActualPretrainingArtifactRef,
    pub maximum_code_token_ratio_bps: u32,
    pub source_family_weights: Vec<PsionActualPretrainingFamilyMixtureWeight>,
    pub source_contribution_caps: Vec<PsionActualPretrainingSourceContributionCap>,
    pub content_class_token_share_report: Vec<PsionActualPretrainingContentClassShare>,
    pub comparison_receipt: PsionActualPretrainingArtifactRef,
    pub lm_loss_delta_bps: i32,
    pub regression_gates: Vec<PsionActualPretrainingRegressionGate>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingReplayAuthority {
    pub tokenizer_training_manifest: PsionActualPretrainingArtifactRef,
    pub raw_source_manifest: PsionActualPretrainingArtifactRef,
    pub tokenized_corpus_manifest: PsionActualPretrainingArtifactRef,
    pub dataset_identity: String,
    pub replay_iteration_mode: String,
    pub shard_ordering: String,
    pub deterministic_shuffle_seed: u64,
    pub packing_policy_id: String,
    pub packing_policy_version: String,
    pub max_sequence_tokens: u64,
    pub train_shard_ids: Vec<String>,
    pub validation_shard_ids: Vec<String>,
    pub held_out_shard_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingEvalBinding {
    pub package_family: String,
    pub acceptance_family: String,
    pub receipt_id: String,
    pub contamination_input_digest: String,
    pub metric_kind: String,
    pub observed_bps: u32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRecipeChangeEvalPackage {
    pub benchmark_catalog: PsionActualPretrainingArtifactRef,
    pub benchmark_receipt_set: PsionActualPretrainingArtifactRef,
    pub required_package_families: Vec<String>,
    pub required_acceptance_families: Vec<String>,
    pub eval_bindings: Vec<PsionActualPretrainingEvalBinding>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDataBundle {
    pub schema_version: String,
    pub data_bundle_id: String,
    pub lane_id: String,
    pub lane_spec: PsionActualPretrainingArtifactRef,
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    pub transformation_stages: Vec<PsionActualPretrainingDataTransformationStage>,
    pub filter_authority: PsionActualPretrainingDataFilterAuthority,
    pub dedup_authority: PsionActualPretrainingDataDedupAuthority,
    pub mixture_authority: PsionActualPretrainingMixtureAuthority,
    pub replay_authority: PsionActualPretrainingReplayAuthority,
    pub recipe_change_eval_package: PsionActualPretrainingRecipeChangeEvalPackage,
    pub support_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionActualPretrainingDataBundle {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_exact(
            self.schema_version.as_str(),
            "data_bundle.schema_version",
            PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.data_bundle_id.as_str(),
            "data_bundle.data_bundle_id",
            PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "data_bundle.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_artifact_ref(&self.lane_spec, "data_bundle.lane_spec")?;
        ensure_artifact_ref(&self.recipe_bundle, "data_bundle.recipe_bundle")?;

        if self.transformation_stages.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("data_bundle.transformation_stages"),
            });
        }
        let mut stage_ids = BTreeSet::new();
        let mut stage_kinds = BTreeSet::new();
        for stage in &self.transformation_stages {
            if !stage_ids.insert(stage.stage_id.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                    field: String::from("data_bundle.transformation_stages[].stage_id"),
                    detail: format!("duplicate stage id `{}`", stage.stage_id),
                });
            }
            stage.validate()?;
            stage_kinds.insert(stage.stage_kind.as_str());
        }
        for required_kind in [
            "admission_review",
            "benchmark_isolation",
            "raw_source_ingestion",
            "tokenizer_training",
            "tokenized_corpus_build",
            "sampling_policy_freeze",
        ] {
            if !stage_kinds.contains(required_kind) {
                return Err(PsionActualPretrainingDataBundleError::MissingField {
                    field: format!("data_bundle.transformation_stages[{required_kind}]"),
                });
            }
        }

        self.filter_authority.validate()?;
        self.dedup_authority.validate(&self.filter_authority)?;
        self.mixture_authority.validate()?;
        self.replay_authority.validate()?;
        self.recipe_change_eval_package.validate()?;

        ensure_exact(
            self.mixture_authority.dataset_identity.as_str(),
            "data_bundle.mixture_authority.dataset_identity",
            self.replay_authority.dataset_identity.as_str(),
        )?;

        ensure_unique_nonempty_strings(self.support_refs.as_slice(), "data_bundle.support_refs[]")?;
        ensure_nonempty(self.claim_boundary.as_str(), "data_bundle.claim_boundary")?;
        ensure_nonempty(self.summary.as_str(), "data_bundle.summary")?;
        if self.bundle_digest != stable_data_bundle_digest(self)? {
            return Err(PsionActualPretrainingDataBundleError::DigestMismatch {
                field: String::from("data_bundle.bundle_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingDataTransformationStage {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(self.stage_id.as_str(), "data_transformation_stage.stage_id")?;
        ensure_nonempty(
            self.stage_kind.as_str(),
            "data_transformation_stage.stage_kind",
        )?;
        ensure_artifact_ref(
            &self.source_artifact,
            "data_transformation_stage.source_artifact",
        )?;
        ensure_nonempty(
            self.output_identity.as_str(),
            "data_transformation_stage.output_identity",
        )?;
        ensure_nonempty(self.detail.as_str(), "data_transformation_stage.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingDataFilterAuthority {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_artifact_ref(&self.admission_policy, "filter_authority.admission_policy")?;
        ensure_artifact_ref(
            &self.source_admission_manifest,
            "filter_authority.source_admission_manifest",
        )?;
        ensure_artifact_ref(
            &self.source_lifecycle_manifest,
            "filter_authority.source_lifecycle_manifest",
        )?;
        ensure_artifact_ref(
            &self.benchmark_isolation_manifest,
            "filter_authority.benchmark_isolation_manifest",
        )?;
        ensure_unique_nonempty_strings(
            self.admitted_training_source_ids.as_slice(),
            "filter_authority.admitted_training_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.tokenizer_only_source_ids.as_slice(),
            "filter_authority.tokenizer_only_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.held_out_source_ids.as_slice(),
            "filter_authority.held_out_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.rejected_source_ids.as_slice(),
            "filter_authority.rejected_source_ids[]",
        )?;
        ensure_nonempty(self.detail.as_str(), "filter_authority.detail")?;

        ensure_required_members(
            self.admitted_training_source_ids.as_slice(),
            "filter_authority.admitted_training_source_ids[]",
            &["arch_textbook_foster_1985", "wasm_core_spec_release_2"],
        )?;
        ensure_required_members(
            self.tokenizer_only_source_ids.as_slice(),
            "filter_authority.tokenizer_only_source_ids[]",
            &["vendor_manual_private_scan_v1"],
        )?;
        ensure_required_members(
            self.held_out_source_ids.as_slice(),
            "filter_authority.held_out_source_ids[]",
            &["spec_quiz_eval_pack_v1"],
        )?;
        ensure_required_members(
            self.rejected_source_ids.as_slice(),
            "filter_authority.rejected_source_ids[]",
            &["forum_scrape_misc_001"],
        )?;

        ensure_disjoint(
            self.admitted_training_source_ids.as_slice(),
            self.held_out_source_ids.as_slice(),
            "filter_authority.admitted_training_source_ids",
            "filter_authority.held_out_source_ids",
        )?;
        ensure_disjoint(
            self.admitted_training_source_ids.as_slice(),
            self.tokenizer_only_source_ids.as_slice(),
            "filter_authority.admitted_training_source_ids",
            "filter_authority.tokenizer_only_source_ids",
        )?;
        ensure_disjoint(
            self.admitted_training_source_ids.as_slice(),
            self.rejected_source_ids.as_slice(),
            "filter_authority.admitted_training_source_ids",
            "filter_authority.rejected_source_ids",
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingDataDedupAuthority {
    fn validate(
        &self,
        filter_authority: &PsionActualPretrainingDataFilterAuthority,
    ) -> Result<(), PsionActualPretrainingDataBundleError> {
        if !self.near_duplicate_review_required_before_training {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from(
                    "dedup_authority.near_duplicate_review_required_before_training",
                ),
                detail: String::from("near-duplicate review must stay required before training"),
            });
        }
        if !self.near_duplicate_review_required_before_benchmark_publication {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from(
                    "dedup_authority.near_duplicate_review_required_before_benchmark_publication",
                ),
                detail: String::from(
                    "near-duplicate review must stay required before benchmark publication",
                ),
            });
        }
        ensure_nonempty(
            self.near_duplicate_review_ref.as_str(),
            "dedup_authority.near_duplicate_review_ref",
        )?;
        ensure_unique_nonempty_strings(
            self.training_excluded_source_ids.as_slice(),
            "dedup_authority.training_excluded_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.benchmark_excluded_source_ids.as_slice(),
            "dedup_authority.benchmark_excluded_source_ids[]",
        )?;
        if self.repetitive_region_controls.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("dedup_authority.repetitive_region_controls"),
            });
        }
        for control in &self.repetitive_region_controls {
            control.validate()?;
        }
        ensure_nonempty(self.detail.as_str(), "dedup_authority.detail")?;

        ensure_contains_all(
            self.training_excluded_source_ids.as_slice(),
            filter_authority.held_out_source_ids.as_slice(),
            "dedup_authority.training_excluded_source_ids[]",
        )?;
        ensure_contains_all(
            self.training_excluded_source_ids.as_slice(),
            filter_authority.tokenizer_only_source_ids.as_slice(),
            "dedup_authority.training_excluded_source_ids[]",
        )?;
        ensure_contains_all(
            self.training_excluded_source_ids.as_slice(),
            filter_authority.rejected_source_ids.as_slice(),
            "dedup_authority.training_excluded_source_ids[]",
        )?;
        ensure_required_members(
            self.benchmark_excluded_source_ids.as_slice(),
            "dedup_authority.benchmark_excluded_source_ids[]",
            &[
                "arch_textbook_foster_1985",
                "vendor_manual_private_scan_v1",
                "forum_scrape_misc_001",
            ],
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingRepetitiveRegionControl {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(
            self.source_id.as_str(),
            "repetitive_region_control.source_id",
        )?;
        ensure_nonempty(
            self.document_id.as_str(),
            "repetitive_region_control.document_id",
        )?;
        ensure_nonempty(
            self.section_id.as_str(),
            "repetitive_region_control.section_id",
        )?;
        if self.downweight_multiplier_bps == 0 || self.downweight_multiplier_bps > 10_000 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("repetitive_region_control.downweight_multiplier_bps"),
                detail: String::from("downweight multiplier must stay within 1..=10000"),
            });
        }
        if self.maximum_region_token_share_bps == 0 || self.maximum_region_token_share_bps > 10_000
        {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("repetitive_region_control.maximum_region_token_share_bps"),
                detail: String::from("region token share cap must stay within 1..=10000"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingMixtureAuthority {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_exact(
            self.dataset_identity.as_str(),
            "mixture_authority.dataset_identity",
            "psion_corpus_tokenized@v1",
        )?;
        ensure_exact(
            self.sampling_policy_id.as_str(),
            "mixture_authority.sampling_policy_id",
            "psion_pretrain_mix",
        )?;
        ensure_exact(
            self.sampling_policy_version.as_str(),
            "mixture_authority.sampling_policy_version",
            "v1",
        )?;
        ensure_artifact_ref(
            &self.sampling_policy_manifest,
            "mixture_authority.sampling_policy_manifest",
        )?;
        if self.maximum_code_token_ratio_bps != 1500 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("mixture_authority.maximum_code_token_ratio_bps"),
                detail: String::from(
                    "the actual lane freezes the current code-token ceiling at 1500 bps",
                ),
            });
        }
        if self.source_family_weights.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("mixture_authority.source_family_weights"),
            });
        }
        let mut family_ids = BTreeSet::new();
        let mut total_weight = 0_u32;
        for weight in &self.source_family_weights {
            weight.validate()?;
            if !family_ids.insert(weight.source_family_id.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                    field: String::from(
                        "mixture_authority.source_family_weights[].source_family_id",
                    ),
                    detail: format!("duplicate family `{}`", weight.source_family_id),
                });
            }
            total_weight += weight.sampling_weight_bps;
        }
        if total_weight != 10_000 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("mixture_authority.source_family_weights"),
                detail: format!("family weights must sum to 10000 bps, found {total_weight}"),
            });
        }
        ensure_required_members(
            self.source_family_weights
                .iter()
                .map(|row| row.source_family_id.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
            "mixture_authority.source_family_weights[].source_family_id",
            &["computer_architecture_history", "normative_specs"],
        )?;

        ensure_unique_nonempty_strings(
            self.source_contribution_caps
                .iter()
                .map(|row| row.source_id.clone())
                .collect::<Vec<_>>()
                .as_slice(),
            "mixture_authority.source_contribution_caps[].source_id",
        )?;
        for cap in &self.source_contribution_caps {
            cap.validate()?;
        }
        ensure_required_members(
            self.source_contribution_caps
                .iter()
                .map(|row| row.source_id.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
            "mixture_authority.source_contribution_caps[].source_id",
            &["arch_textbook_foster_1985", "wasm_core_spec_release_2"],
        )?;

        if self.content_class_token_share_report.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("mixture_authority.content_class_token_share_report"),
            });
        }
        let mut content_classes = BTreeSet::new();
        for row in &self.content_class_token_share_report {
            row.validate()?;
            if !content_classes.insert(row.content_class.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                    field: String::from(
                        "mixture_authority.content_class_token_share_report[].content_class",
                    ),
                    detail: format!("duplicate content class `{}`", row.content_class),
                });
            }
        }
        for required_class in ["prose", "spec_text", "code"] {
            if !content_classes.contains(required_class) {
                return Err(PsionActualPretrainingDataBundleError::MissingField {
                    field: format!(
                        "mixture_authority.content_class_token_share_report[{required_class}]"
                    ),
                });
            }
        }

        ensure_artifact_ref(
            &self.comparison_receipt,
            "mixture_authority.comparison_receipt",
        )?;
        if self.lm_loss_delta_bps > 0 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("mixture_authority.lm_loss_delta_bps"),
                detail: String::from(
                    "candidate mixture may not regress loss on the frozen comparison receipt",
                ),
            });
        }
        if self.regression_gates.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("mixture_authority.regression_gates"),
            });
        }
        let mut regression_kinds = BTreeSet::new();
        for gate in &self.regression_gates {
            gate.validate()?;
            if !regression_kinds.insert(gate.regression_kind.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                    field: String::from("mixture_authority.regression_gates[].regression_kind"),
                    detail: format!("duplicate regression kind `{}`", gate.regression_kind),
                });
            }
        }
        for required_kind in [
            "explanation_quality",
            "spec_interpretation",
            "tradeoff_reasoning",
            "invariant_articulation",
            "coding_fluency",
        ] {
            if !regression_kinds.contains(required_kind) {
                return Err(PsionActualPretrainingDataBundleError::MissingField {
                    field: format!("mixture_authority.regression_gates[{required_kind}]"),
                });
            }
        }
        ensure_nonempty(self.detail.as_str(), "mixture_authority.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingFamilyMixtureWeight {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(
            self.source_family_id.as_str(),
            "family_mixture_weight.source_family_id",
        )?;
        ensure_nonempty(
            self.content_class.as_str(),
            "family_mixture_weight.content_class",
        )?;
        if self.sampling_weight_bps == 0 || self.sampling_weight_bps > 10_000 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("family_mixture_weight.sampling_weight_bps"),
                detail: String::from("sampling weight must stay within 1..=10000"),
            });
        }
        if self.maximum_family_token_share_bps == 0 || self.maximum_family_token_share_bps > 10_000
        {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("family_mixture_weight.maximum_family_token_share_bps"),
                detail: String::from("family token-share cap must stay within 1..=10000"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingSourceContributionCap {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(self.source_id.as_str(), "source_contribution_cap.source_id")?;
        if self.maximum_source_token_share_bps == 0 || self.maximum_source_token_share_bps > 10_000
        {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("source_contribution_cap.maximum_source_token_share_bps"),
                detail: String::from("source token-share cap must stay within 1..=10000"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingContentClassShare {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(
            self.content_class.as_str(),
            "content_class_share.content_class",
        )?;
        if self.observed_token_share_bps > 10_000 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("content_class_share.observed_token_share_bps"),
                detail: String::from("observed content-class token share may not exceed 10000 bps"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingRegressionGate {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_nonempty(
            self.regression_kind.as_str(),
            "regression_gate.regression_kind",
        )?;
        if self.maximum_regression_bps > 10_000 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("regression_gate.maximum_regression_bps"),
                detail: String::from("maximum regression threshold may not exceed 10000 bps"),
            });
        }
        if self.observed_regression_bps > self.maximum_regression_bps {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("regression_gate.observed_regression_bps"),
                detail: format!(
                    "observed regression {} exceeds allowed {} for `{}`",
                    self.observed_regression_bps, self.maximum_regression_bps, self.regression_kind
                ),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingReplayAuthority {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_artifact_ref(
            &self.tokenizer_training_manifest,
            "replay_authority.tokenizer_training_manifest",
        )?;
        ensure_artifact_ref(
            &self.raw_source_manifest,
            "replay_authority.raw_source_manifest",
        )?;
        ensure_artifact_ref(
            &self.tokenized_corpus_manifest,
            "replay_authority.tokenized_corpus_manifest",
        )?;
        ensure_exact(
            self.dataset_identity.as_str(),
            "replay_authority.dataset_identity",
            "psion_corpus_tokenized@v1",
        )?;
        ensure_exact(
            self.replay_iteration_mode.as_str(),
            "replay_authority.replay_iteration_mode",
            "repeat",
        )?;
        ensure_exact(
            self.shard_ordering.as_str(),
            "replay_authority.shard_ordering",
            "deterministic_shuffle",
        )?;
        if self.deterministic_shuffle_seed != 1337 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("replay_authority.deterministic_shuffle_seed"),
                detail: String::from(
                    "the frozen actual lane keeps deterministic shuffle seed 1337",
                ),
            });
        }
        ensure_exact(
            self.packing_policy_id.as_str(),
            "replay_authority.packing_policy_id",
            "psion_pack_context_window",
        )?;
        ensure_exact(
            self.packing_policy_version.as_str(),
            "replay_authority.packing_policy_version",
            "v1",
        )?;
        if self.max_sequence_tokens != 8192 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("replay_authority.max_sequence_tokens"),
                detail: String::from("the frozen actual lane keeps max sequence length 8192"),
            });
        }
        ensure_unique_nonempty_strings(
            self.train_shard_ids.as_slice(),
            "replay_authority.train_shard_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.validation_shard_ids.as_slice(),
            "replay_authority.validation_shard_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.held_out_shard_ids.as_slice(),
            "replay_authority.held_out_shard_ids[]",
        )?;
        ensure_nonempty(self.detail.as_str(), "replay_authority.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingRecipeChangeEvalPackage {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        ensure_artifact_ref(
            &self.benchmark_catalog,
            "recipe_change_eval_package.benchmark_catalog",
        )?;
        ensure_artifact_ref(
            &self.benchmark_receipt_set,
            "recipe_change_eval_package.benchmark_receipt_set",
        )?;
        ensure_unique_nonempty_strings(
            self.required_package_families.as_slice(),
            "recipe_change_eval_package.required_package_families[]",
        )?;
        ensure_unique_nonempty_strings(
            self.required_acceptance_families.as_slice(),
            "recipe_change_eval_package.required_acceptance_families[]",
        )?;
        ensure_required_members(
            self.required_package_families.as_slice(),
            "recipe_change_eval_package.required_package_families[]",
            &[
                "architecture_reasoning",
                "normative_spec_reading",
                "engineering_spec_interpretation",
                "memorization_versus_reasoning",
            ],
        )?;
        ensure_required_members(
            self.required_acceptance_families.as_slice(),
            "recipe_change_eval_package.required_acceptance_families[]",
            &[
                "architecture_reasoning",
                "normative_spec_reading",
                "engineering_spec_interpretation",
                "memorization_versus_reasoning",
            ],
        )?;
        if self.eval_bindings.is_empty() {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: String::from("recipe_change_eval_package.eval_bindings"),
            });
        }
        let mut families = BTreeSet::new();
        let mut receipt_ids = BTreeSet::new();
        for binding in &self.eval_bindings {
            binding.validate()?;
            families.insert(binding.package_family.as_str());
            if !receipt_ids.insert(binding.receipt_id.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                    field: String::from("recipe_change_eval_package.eval_bindings[].receipt_id"),
                    detail: format!("duplicate eval receipt `{}`", binding.receipt_id),
                });
            }
        }
        for required_family in &self.required_package_families {
            if !families.contains(required_family.as_str()) {
                return Err(PsionActualPretrainingDataBundleError::MissingField {
                    field: format!("recipe_change_eval_package.eval_bindings[{required_family}]"),
                });
            }
        }
        ensure_nonempty(self.detail.as_str(), "recipe_change_eval_package.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingEvalBinding {
    fn validate(&self) -> Result<(), PsionActualPretrainingDataBundleError> {
        for (field, value) in [
            ("eval_binding.package_family", self.package_family.as_str()),
            (
                "eval_binding.acceptance_family",
                self.acceptance_family.as_str(),
            ),
            ("eval_binding.receipt_id", self.receipt_id.as_str()),
            (
                "eval_binding.contamination_input_digest",
                self.contamination_input_digest.as_str(),
            ),
            ("eval_binding.metric_kind", self.metric_kind.as_str()),
            ("eval_binding.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.observed_bps == 0 {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from("eval_binding.observed_bps"),
                detail: String::from(
                    "recipe-change eval bindings must retain a positive observed score",
                ),
            });
        }
        Ok(())
    }
}

fn stable_data_bundle_digest(
    bundle: &PsionActualPretrainingDataBundle,
) -> Result<String, PsionActualPretrainingDataBundleError> {
    let mut clone = bundle.clone();
    clone.bundle_digest.clear();
    let bytes = serde_json::to_vec(&clone).map_err(PsionActualPretrainingDataBundleError::Json)?;
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_data_bundle|");
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionActualPretrainingDataBundleError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingDataBundleError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingDataBundleError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(
            PsionActualPretrainingDataBundleError::SchemaVersionMismatch {
                field: String::from(field),
                expected: String::from(expected),
                actual: String::from(actual),
            },
        );
    }
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field: &str,
) -> Result<(), PsionActualPretrainingDataBundleError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field}.sha256"))?;
    Ok(())
}

fn ensure_unique_nonempty_strings(
    values: &[impl AsRef<str>],
    field: &str,
) -> Result<(), PsionActualPretrainingDataBundleError> {
    if values.is_empty() {
        return Err(PsionActualPretrainingDataBundleError::MissingField {
            field: String::from(field),
        });
    }
    let mut seen = BTreeSet::new();
    for value in values {
        let value = value.as_ref();
        ensure_nonempty(value, field)?;
        if !seen.insert(value.to_owned()) {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: String::from(field),
                detail: format!("duplicate value `{value}`"),
            });
        }
    }
    Ok(())
}

fn ensure_required_members(
    values: &[impl AsRef<str>],
    field: &str,
    required: &[&str],
) -> Result<(), PsionActualPretrainingDataBundleError> {
    let set = values
        .iter()
        .map(|value| value.as_ref())
        .collect::<BTreeSet<_>>();
    for required in required {
        if !set.contains(required) {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: format!("{field}[{required}]"),
            });
        }
    }
    Ok(())
}

fn ensure_contains_all(
    haystack: &[impl AsRef<str>],
    needles: &[impl AsRef<str>],
    field: &str,
) -> Result<(), PsionActualPretrainingDataBundleError> {
    let set = haystack
        .iter()
        .map(|value| value.as_ref())
        .collect::<BTreeSet<_>>();
    for needle in needles {
        let needle = needle.as_ref();
        if !set.contains(needle) {
            return Err(PsionActualPretrainingDataBundleError::MissingField {
                field: format!("{field}[{needle}]"),
            });
        }
    }
    Ok(())
}

fn ensure_disjoint(
    left: &[impl AsRef<str>],
    right: &[impl AsRef<str>],
    left_field: &str,
    right_field: &str,
) -> Result<(), PsionActualPretrainingDataBundleError> {
    let left = left
        .iter()
        .map(|value| value.as_ref())
        .collect::<BTreeSet<_>>();
    for right in right {
        let right = right.as_ref();
        if left.contains(right) {
            return Err(PsionActualPretrainingDataBundleError::UnsupportedValue {
                field: format!("{left_field}/{right_field}"),
                detail: format!("overlapping value `{right}`"),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PsionActualPretrainingDataBundleError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("unsupported value for `{field}`: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("failed to serialize data bundle for digesting: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_FIXTURE_PATH, PsionActualPretrainingDataBundle,
        PsionActualPretrainingDataBundleError,
    };

    #[test]
    fn actual_pretraining_data_bundle_fixture_validates() -> Result<(), Box<dyn std::error::Error>>
    {
        let bundle: PsionActualPretrainingDataBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"
        ))?;
        assert_eq!(
            PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_FIXTURE_PATH,
            "fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"
        );
        bundle.validate()?;
        Ok(())
    }

    #[test]
    fn actual_pretraining_data_bundle_requires_engineering_eval_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut bundle: PsionActualPretrainingDataBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"
        ))?;
        bundle
            .recipe_change_eval_package
            .eval_bindings
            .retain(|binding| binding.package_family != "engineering_spec_interpretation");
        let error = bundle
            .validate()
            .expect_err("bundle should reject missing engineering eval family");
        assert!(matches!(
            error,
            PsionActualPretrainingDataBundleError::MissingField { field }
            if field == "recipe_change_eval_package.eval_bindings[engineering_spec_interpretation]"
        ));
        Ok(())
    }
}
