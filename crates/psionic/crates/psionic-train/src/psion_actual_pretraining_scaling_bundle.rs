use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PsionActualPretrainingArtifactRef,
};

/// Stable schema version for the canonical actual-lane scaling bundle.
pub const PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_scaling_bundle.v1";

/// Stable scaling-bundle identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_ID: &str =
    "psion_actual_pretraining_scaling_bundle_v1";

/// Canonical fixture path for the actual-lane scaling bundle.
pub const PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_scaling_bundle_v1.json";

/// Canonical focused doc path for the actual-lane scaling bundle.
pub const PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingScalingCandidate {
    pub candidate_id: String,
    pub candidate_kind: String,
    pub model_size_anchor: String,
    pub estimated_parameter_count: u64,
    pub train_token_budget: u64,
    pub validation_token_budget: u64,
    pub held_out_token_budget: u64,
    pub optimizer_steps: u64,
    pub projected_mean_tokens_per_second: u64,
    pub projected_wall_clock_ms: u64,
    pub projected_total_cost_microusd: u64,
    pub projected_validation_loss_milli: u64,
    pub projected_average_reasoning_pass_rate_bps: u32,
    pub projected_reasoning_floor_bps: u32,
    pub evidence_kind: String,
    pub projection_basis: String,
    pub eligible_under_rule: bool,
    pub selected_for_recipe: bool,
    pub rejection_reason: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingScalingSelectionRule {
    pub rule_id: String,
    pub selection_policy: String,
    pub admitted_recipe_id: String,
    pub chosen_candidate_id: String,
    pub tokens_per_parameter: u64,
    pub tokens_per_step: u64,
    pub maximum_stage_length_ms: u64,
    pub maximum_total_cost_microusd: u64,
    pub maximum_validation_loss_milli: u64,
    pub minimum_reasoning_floor_bps: u32,
    pub required_benchmark_package_families: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingScalingBundle {
    pub schema_version: String,
    pub scaling_bundle_id: String,
    pub lane_id: String,
    pub lane_spec: PsionActualPretrainingArtifactRef,
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    pub data_bundle: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub anchor_run_bundle: PsionActualPretrainingArtifactRef,
    pub anchor_stage_receipt: PsionActualPretrainingArtifactRef,
    pub anchor_observability_receipt: PsionActualPretrainingArtifactRef,
    pub benchmark_receipt_set: PsionActualPretrainingArtifactRef,
    pub ablation_family_id: String,
    pub candidates: Vec<PsionActualPretrainingScalingCandidate>,
    pub selection_rule: PsionActualPretrainingScalingSelectionRule,
    pub support_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionActualPretrainingScalingBundle {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingScalingBundleError> {
        ensure_exact(
            self.schema_version.as_str(),
            "scaling_bundle.schema_version",
            PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.scaling_bundle_id.as_str(),
            "scaling_bundle.scaling_bundle_id",
            PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "scaling_bundle.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_artifact_ref(&self.lane_spec, "scaling_bundle.lane_spec")?;
        ensure_artifact_ref(&self.recipe_bundle, "scaling_bundle.recipe_bundle")?;
        ensure_artifact_ref(&self.data_bundle, "scaling_bundle.data_bundle")?;
        ensure_artifact_ref(&self.systems_bundle, "scaling_bundle.systems_bundle")?;
        ensure_artifact_ref(&self.anchor_run_bundle, "scaling_bundle.anchor_run_bundle")?;
        ensure_artifact_ref(
            &self.anchor_stage_receipt,
            "scaling_bundle.anchor_stage_receipt",
        )?;
        ensure_artifact_ref(
            &self.anchor_observability_receipt,
            "scaling_bundle.anchor_observability_receipt",
        )?;
        ensure_artifact_ref(
            &self.benchmark_receipt_set,
            "scaling_bundle.benchmark_receipt_set",
        )?;
        ensure_nonempty(
            self.ablation_family_id.as_str(),
            "scaling_bundle.ablation_family_id",
        )?;
        self.selection_rule.validate()?;

        if self.candidates.is_empty() {
            return Err(PsionActualPretrainingScalingBundleError::MissingField {
                field: String::from("scaling_bundle.candidates"),
            });
        }
        let mut candidate_ids = BTreeSet::new();
        let mut candidate_kinds = BTreeSet::new();
        let mut size_anchors = BTreeSet::new();
        let mut selected_candidates = Vec::new();
        let mut eligible_candidates = Vec::new();
        for candidate in &self.candidates {
            candidate.validate()?;
            if !candidate_ids.insert(candidate.candidate_id.as_str()) {
                return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: String::from("scaling_bundle.candidates[].candidate_id"),
                    detail: format!("duplicate candidate id `{}`", candidate.candidate_id),
                });
            }
            if !size_anchors.insert(candidate.model_size_anchor.as_str()) {
                return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: String::from("scaling_bundle.candidates[].model_size_anchor"),
                    detail: format!(
                        "duplicate model size anchor `{}`",
                        candidate.model_size_anchor
                    ),
                });
            }
            candidate_kinds.insert(candidate.candidate_kind.as_str());
            ensure_budget_relationships(candidate, &self.selection_rule)?;
            if candidate.eligible_under_rule
                != candidate_satisfies_rule(candidate, &self.selection_rule)
            {
                return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: format!(
                        "scaling_bundle.candidates[{}].eligible_under_rule",
                        candidate.candidate_id
                    ),
                    detail: String::from(
                        "candidate eligibility must match the frozen selection thresholds",
                    ),
                });
            }
            if candidate.selected_for_recipe {
                selected_candidates.push(candidate);
            }
            if candidate.eligible_under_rule {
                eligible_candidates.push(candidate);
            }
        }
        for required_kind in ["smaller_projection", "measured_anchor", "larger_projection"] {
            if !candidate_kinds.contains(required_kind) {
                return Err(PsionActualPretrainingScalingBundleError::MissingField {
                    field: format!("scaling_bundle.candidates[{required_kind}]"),
                });
            }
        }
        if selected_candidates.len() != 1 {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: String::from("scaling_bundle.candidates[].selected_for_recipe"),
                detail: String::from("exactly one candidate must be selected for the recipe"),
            });
        }
        if eligible_candidates.is_empty() {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: String::from("scaling_bundle.candidates[].eligible_under_rule"),
                detail: String::from("at least one candidate must satisfy the frozen rule"),
            });
        }
        let selected = selected_candidates[0];
        ensure_exact(
            selected.candidate_id.as_str(),
            "scaling_bundle.selection_rule.chosen_candidate_id",
            self.selection_rule.chosen_candidate_id.as_str(),
        )?;
        let largest_eligible = eligible_candidates
            .into_iter()
            .max_by_key(|candidate| candidate.estimated_parameter_count)
            .ok_or_else(
                || PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: String::from("scaling_bundle.candidates[].eligible_under_rule"),
                    detail: String::from("no eligible candidates remain after validation"),
                },
            )?;
        ensure_exact(
            selected.candidate_id.as_str(),
            "scaling_bundle.selected_candidate",
            largest_eligible.candidate_id.as_str(),
        )?;

        ensure_unique_nonempty_strings(
            self.support_refs.as_slice(),
            "scaling_bundle.support_refs[]",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "scaling_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.summary.as_str(), "scaling_bundle.summary")?;
        if self.bundle_digest != stable_scaling_bundle_digest(self)? {
            return Err(PsionActualPretrainingScalingBundleError::DigestMismatch {
                field: String::from("scaling_bundle.bundle_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingScalingCandidate {
    fn validate(&self) -> Result<(), PsionActualPretrainingScalingBundleError> {
        for (field, value) in [
            ("scaling_candidate.candidate_id", self.candidate_id.as_str()),
            (
                "scaling_candidate.candidate_kind",
                self.candidate_kind.as_str(),
            ),
            (
                "scaling_candidate.model_size_anchor",
                self.model_size_anchor.as_str(),
            ),
            (
                "scaling_candidate.evidence_kind",
                self.evidence_kind.as_str(),
            ),
            (
                "scaling_candidate.projection_basis",
                self.projection_basis.as_str(),
            ),
            ("scaling_candidate.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.estimated_parameter_count == 0
            || self.train_token_budget == 0
            || self.validation_token_budget == 0
            || self.held_out_token_budget == 0
            || self.optimizer_steps == 0
            || self.projected_mean_tokens_per_second == 0
            || self.projected_wall_clock_ms == 0
            || self.projected_total_cost_microusd == 0
            || self.projected_validation_loss_milli == 0
        {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: format!("scaling_candidate[{}]", self.candidate_id),
                detail: String::from("candidate numeric fields must all be positive"),
            });
        }
        if self.projected_average_reasoning_pass_rate_bps < self.projected_reasoning_floor_bps {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: format!(
                    "scaling_candidate[{}].projected_reasoning_floor_bps",
                    self.candidate_id
                ),
                detail: String::from(
                    "reasoning floor may not exceed the projected average reasoning pass rate",
                ),
            });
        }
        if self.selected_for_recipe {
            if !self.eligible_under_rule {
                return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: format!(
                        "scaling_candidate[{}].selected_for_recipe",
                        self.candidate_id
                    ),
                    detail: String::from("selected candidate must satisfy the frozen rule"),
                });
            }
            if self.rejection_reason.is_some() {
                return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                    field: format!("scaling_candidate[{}].rejection_reason", self.candidate_id),
                    detail: String::from("selected candidate may not carry a rejection reason"),
                });
            }
        } else if self.rejection_reason.as_deref().unwrap_or("").is_empty() {
            return Err(PsionActualPretrainingScalingBundleError::MissingField {
                field: format!("scaling_candidate[{}].rejection_reason", self.candidate_id),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingScalingSelectionRule {
    fn validate(&self) -> Result<(), PsionActualPretrainingScalingBundleError> {
        for (field, value) in [
            ("selection_rule.rule_id", self.rule_id.as_str()),
            (
                "selection_rule.selection_policy",
                self.selection_policy.as_str(),
            ),
            (
                "selection_rule.admitted_recipe_id",
                self.admitted_recipe_id.as_str(),
            ),
            (
                "selection_rule.chosen_candidate_id",
                self.chosen_candidate_id.as_str(),
            ),
            ("selection_rule.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_exact(
            self.admitted_recipe_id.as_str(),
            "selection_rule.admitted_recipe_id",
            PSION_ACTUAL_PRETRAINING_RECIPE_ID,
        )?;
        ensure_exact(
            self.selection_policy.as_str(),
            "selection_rule.selection_policy",
            "largest_eligible_candidate",
        )?;
        if self.tokens_per_parameter == 0
            || self.tokens_per_step == 0
            || self.maximum_stage_length_ms == 0
            || self.maximum_total_cost_microusd == 0
            || self.maximum_validation_loss_milli == 0
            || self.minimum_reasoning_floor_bps == 0
        {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: String::from("selection_rule"),
                detail: String::from("selection-rule numeric thresholds must all be positive"),
            });
        }
        ensure_unique_nonempty_strings(
            self.required_benchmark_package_families.as_slice(),
            "selection_rule.required_benchmark_package_families[]",
        )?;
        for required_family in [
            "architecture_reasoning",
            "normative_spec_reading",
            "engineering_spec_interpretation",
            "memorization_versus_reasoning",
        ] {
            if !self
                .required_benchmark_package_families
                .iter()
                .any(|family| family == required_family)
            {
                return Err(PsionActualPretrainingScalingBundleError::MissingField {
                    field: format!(
                        "selection_rule.required_benchmark_package_families[{required_family}]"
                    ),
                });
            }
        }
        Ok(())
    }
}

fn candidate_satisfies_rule(
    candidate: &PsionActualPretrainingScalingCandidate,
    rule: &PsionActualPretrainingScalingSelectionRule,
) -> bool {
    candidate.projected_wall_clock_ms <= rule.maximum_stage_length_ms
        && candidate.projected_total_cost_microusd <= rule.maximum_total_cost_microusd
        && candidate.projected_validation_loss_milli <= rule.maximum_validation_loss_milli
        && candidate.projected_reasoning_floor_bps >= rule.minimum_reasoning_floor_bps
}

fn ensure_budget_relationships(
    candidate: &PsionActualPretrainingScalingCandidate,
    rule: &PsionActualPretrainingScalingSelectionRule,
) -> Result<(), PsionActualPretrainingScalingBundleError> {
    if candidate.train_token_budget
        != candidate
            .estimated_parameter_count
            .saturating_mul(rule.tokens_per_parameter)
    {
        return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
            field: format!(
                "scaling_candidate[{}].train_token_budget",
                candidate.candidate_id
            ),
            detail: String::from(
                "train token budget must follow the frozen tokens-per-parameter rule",
            ),
        });
    }
    if candidate.validation_token_budget != candidate.train_token_budget / 32 {
        return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
            field: format!(
                "scaling_candidate[{}].validation_token_budget",
                candidate.candidate_id
            ),
            detail: String::from("validation token budget must stay at train/32"),
        });
    }
    if candidate.held_out_token_budget != candidate.train_token_budget / 128 {
        return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
            field: format!(
                "scaling_candidate[{}].held_out_token_budget",
                candidate.candidate_id
            ),
            detail: String::from("held-out token budget must stay at train/128"),
        });
    }
    if candidate.optimizer_steps != candidate.train_token_budget / rule.tokens_per_step {
        return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
            field: format!(
                "scaling_candidate[{}].optimizer_steps",
                candidate.candidate_id
            ),
            detail: String::from(
                "optimizer steps must stay tied to the frozen tokens-per-step rule",
            ),
        });
    }
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field: &str,
) -> Result<(), PsionActualPretrainingScalingBundleError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field}.sha256"))?;
    Ok(())
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingScalingBundleError> {
    if values.is_empty() {
        return Err(PsionActualPretrainingScalingBundleError::MissingField {
            field: field.to_string(),
        });
    }
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionActualPretrainingScalingBundleError::UnsupportedValue {
                field: field.to_string(),
                detail: format!("duplicate entry `{value}`"),
            });
        }
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingScalingBundleError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingScalingBundleError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingScalingBundleError> {
    if actual != expected {
        return Err(PsionActualPretrainingScalingBundleError::ExactMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn stable_scaling_bundle_digest(
    bundle: &PsionActualPretrainingScalingBundle,
) -> Result<String, PsionActualPretrainingScalingBundleError> {
    let mut clone = bundle.clone();
    clone.bundle_digest.clear();
    let bytes = serde_json::to_vec(&clone).map_err(|error| {
        PsionActualPretrainingScalingBundleError::Serialization {
            detail: error.to_string(),
        }
    })?;
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_scaling_bundle|");
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Error)]
pub enum PsionActualPretrainingScalingBundleError {
    #[error("psion actual-pretraining scaling field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining scaling field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    ExactMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining scaling field `{field}` is invalid: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining scaling digest mismatch at `{field}`")]
    DigestMismatch { field: String },
    #[error("psion actual-pretraining scaling serialization failed: {detail}")]
    Serialization { detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_fixture() -> PsionActualPretrainingScalingBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_scaling_bundle_v1.json"
        ))
        .expect("actual pretraining scaling bundle fixture should parse")
    }

    #[test]
    fn actual_pretraining_scaling_bundle_fixture_validates() {
        let bundle = load_fixture();
        bundle
            .validate()
            .expect("actual pretraining scaling bundle fixture should validate");
    }

    #[test]
    fn actual_pretraining_scaling_bundle_rejects_wrong_selected_candidate() {
        let mut bundle = load_fixture();
        bundle.selection_rule.chosen_candidate_id =
            String::from("psion_actual_pretraining_internal64m_projection");
        bundle
            .validate()
            .expect_err("wrong chosen candidate should fail");
    }
}
