use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef};

/// Stable schema version for the canonical actual-lane baseline-tools bundle.
pub const PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_baseline_tools_bundle.v1";

/// Stable baseline-tools bundle identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_ID: &str =
    "psion_actual_pretraining_baseline_tools_bundle_v1";

/// Canonical fixture path for the actual-lane baseline-tools bundle.
pub const PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_baseline_tools_bundle_v1.json";

/// Canonical focused doc path for the actual-lane baseline-tools bundle.
pub const PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingBringupTrainer {
    pub trainer_id: String,
    pub stage_program_id: String,
    pub trainer_entry_surface: String,
    pub stage_config: PsionActualPretrainingArtifactRef,
    pub model_descriptor: PsionActualPretrainingArtifactRef,
    pub model_id: String,
    pub dataset_identity: String,
    pub sampling_policy_id: String,
    pub sampling_policy_version: String,
    pub tokenizer_binding_digest: String,
    pub max_context_tokens: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingTokenizerReproducibilityBinding {
    pub tokenizer_training_manifest: PsionActualPretrainingArtifactRef,
    pub tokenizer_artifact_bundle: PsionActualPretrainingArtifactRef,
    pub tokenized_corpus_manifest: PsionActualPretrainingArtifactRef,
    pub tokenizer_id: String,
    pub tokenizer_version: String,
    pub tokenizer_digest: String,
    pub tokenizer_binding_digest: String,
    pub tokenizer_only_source_ids: Vec<String>,
    pub model_training_source_ids: Vec<String>,
    pub held_out_source_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingResourceAccountingRow {
    pub row_id: String,
    pub scope_kind: String,
    pub config_binding_id: String,
    pub model_id: String,
    pub size_anchor: String,
    pub parameter_count_estimate: u64,
    pub train_token_budget: u64,
    pub validation_token_budget: u64,
    pub held_out_token_budget: u64,
    pub optimizer_steps: u64,
    pub tokens_per_step: u64,
    pub max_context_tokens: u64,
    pub checkpoint_total_bytes: u64,
    pub optimizer_state_bytes: u64,
    pub activation_headroom_bytes: u64,
    pub expected_mean_tokens_per_second: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingBoundedAblationConfig {
    pub ablation_id: String,
    pub ablation_family: String,
    pub stage_config: PsionActualPretrainingArtifactRef,
    pub model_descriptor: PsionActualPretrainingArtifactRef,
    pub config_binding_id: String,
    pub max_train_token_budget: u64,
    pub max_validation_token_budget: u64,
    pub max_optimizer_steps: u64,
    pub tokens_per_step: u64,
    pub consumption_target: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingBaselineToolsBundle {
    pub schema_version: String,
    pub baseline_tools_bundle_id: String,
    pub lane_id: String,
    pub lane_spec: PsionActualPretrainingArtifactRef,
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    pub scaling_bundle: PsionActualPretrainingArtifactRef,
    pub data_bundle: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub bringup_trainer: PsionActualPretrainingBringupTrainer,
    pub tokenizer_reproducibility: PsionActualPretrainingTokenizerReproducibilityBinding,
    pub resource_accounting_rows: Vec<PsionActualPretrainingResourceAccountingRow>,
    pub bounded_ablation_configs: Vec<PsionActualPretrainingBoundedAblationConfig>,
    pub support_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionActualPretrainingBaselineToolsBundle {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
        ensure_exact(
            self.schema_version.as_str(),
            "baseline_tools_bundle.schema_version",
            PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.baseline_tools_bundle_id.as_str(),
            "baseline_tools_bundle.baseline_tools_bundle_id",
            PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "baseline_tools_bundle.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_artifact_ref(&self.lane_spec, "baseline_tools_bundle.lane_spec")?;
        ensure_artifact_ref(&self.recipe_bundle, "baseline_tools_bundle.recipe_bundle")?;
        ensure_artifact_ref(&self.scaling_bundle, "baseline_tools_bundle.scaling_bundle")?;
        ensure_artifact_ref(&self.data_bundle, "baseline_tools_bundle.data_bundle")?;
        ensure_artifact_ref(&self.systems_bundle, "baseline_tools_bundle.systems_bundle")?;
        self.bringup_trainer.validate()?;
        self.tokenizer_reproducibility.validate()?;
        validate_accounting_rows(self.resource_accounting_rows.as_slice())?;
        validate_ablation_configs(self.bounded_ablation_configs.as_slice())?;
        ensure_unique_nonempty_strings(
            self.support_refs.as_slice(),
            "baseline_tools_bundle.support_refs[]",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "baseline_tools_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.summary.as_str(), "baseline_tools_bundle.summary")?;
        if self.bundle_digest != stable_baseline_tools_bundle_digest(self)? {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::DigestMismatch {
                    field: String::from("baseline_tools_bundle.bundle_digest"),
                },
            );
        }
        Ok(())
    }
}

impl PsionActualPretrainingBringupTrainer {
    fn validate(&self) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
        ensure_nonempty(self.trainer_id.as_str(), "bringup_trainer.trainer_id")?;
        ensure_exact(
            self.stage_program_id.as_str(),
            "bringup_trainer.stage_program_id",
            "psion_pretrain_stage",
        )?;
        ensure_nonempty(
            self.trainer_entry_surface.as_str(),
            "bringup_trainer.trainer_entry_surface",
        )?;
        ensure_artifact_ref(&self.stage_config, "bringup_trainer.stage_config")?;
        ensure_artifact_ref(&self.model_descriptor, "bringup_trainer.model_descriptor")?;
        ensure_nonempty(self.model_id.as_str(), "bringup_trainer.model_id")?;
        ensure_nonempty(
            self.dataset_identity.as_str(),
            "bringup_trainer.dataset_identity",
        )?;
        ensure_nonempty(
            self.sampling_policy_id.as_str(),
            "bringup_trainer.sampling_policy_id",
        )?;
        ensure_nonempty(
            self.sampling_policy_version.as_str(),
            "bringup_trainer.sampling_policy_version",
        )?;
        ensure_nonempty(
            self.tokenizer_binding_digest.as_str(),
            "bringup_trainer.tokenizer_binding_digest",
        )?;
        if self.max_context_tokens == 0 {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                    field: String::from("bringup_trainer.max_context_tokens"),
                    detail: String::from("bring-up trainer must retain a positive context length"),
                },
            );
        }
        ensure_nonempty(self.detail.as_str(), "bringup_trainer.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingTokenizerReproducibilityBinding {
    fn validate(&self) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
        ensure_artifact_ref(
            &self.tokenizer_training_manifest,
            "tokenizer_reproducibility.tokenizer_training_manifest",
        )?;
        ensure_artifact_ref(
            &self.tokenizer_artifact_bundle,
            "tokenizer_reproducibility.tokenizer_artifact_bundle",
        )?;
        ensure_artifact_ref(
            &self.tokenized_corpus_manifest,
            "tokenizer_reproducibility.tokenized_corpus_manifest",
        )?;
        ensure_nonempty(
            self.tokenizer_id.as_str(),
            "tokenizer_reproducibility.tokenizer_id",
        )?;
        ensure_nonempty(
            self.tokenizer_version.as_str(),
            "tokenizer_reproducibility.tokenizer_version",
        )?;
        ensure_nonempty(
            self.tokenizer_digest.as_str(),
            "tokenizer_reproducibility.tokenizer_digest",
        )?;
        ensure_nonempty(
            self.tokenizer_binding_digest.as_str(),
            "tokenizer_reproducibility.tokenizer_binding_digest",
        )?;
        ensure_unique_nonempty_strings(
            self.tokenizer_only_source_ids.as_slice(),
            "tokenizer_reproducibility.tokenizer_only_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.model_training_source_ids.as_slice(),
            "tokenizer_reproducibility.model_training_source_ids[]",
        )?;
        ensure_unique_nonempty_strings(
            self.held_out_source_ids.as_slice(),
            "tokenizer_reproducibility.held_out_source_ids[]",
        )?;
        if self.tokenizer_only_source_ids.is_empty()
            || self.model_training_source_ids.is_empty()
            || self.held_out_source_ids.is_empty()
        {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::MissingField {
                    field: String::from("tokenizer_reproducibility.required_source_partitions"),
                },
            );
        }
        ensure_nonempty(self.detail.as_str(), "tokenizer_reproducibility.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingResourceAccountingRow {
    fn validate(&self) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
        ensure_nonempty(self.row_id.as_str(), "resource_accounting_rows[].row_id")?;
        match self.scope_kind.as_str() {
            "actual_lane" | "bounded_ablation" => {}
            _ => {
                return Err(
                    PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                        field: String::from("resource_accounting_rows[].scope_kind"),
                        detail: String::from(
                            "resource accounting rows must be actual_lane or bounded_ablation",
                        ),
                    },
                );
            }
        }
        for (field, value) in [
            (
                "resource_accounting_rows[].config_binding_id",
                self.config_binding_id.as_str(),
            ),
            (
                "resource_accounting_rows[].model_id",
                self.model_id.as_str(),
            ),
            (
                "resource_accounting_rows[].size_anchor",
                self.size_anchor.as_str(),
            ),
            ("resource_accounting_rows[].detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        for (field, value) in [
            (
                "resource_accounting_rows[].parameter_count_estimate",
                self.parameter_count_estimate,
            ),
            (
                "resource_accounting_rows[].train_token_budget",
                self.train_token_budget,
            ),
            (
                "resource_accounting_rows[].validation_token_budget",
                self.validation_token_budget,
            ),
            (
                "resource_accounting_rows[].held_out_token_budget",
                self.held_out_token_budget,
            ),
            (
                "resource_accounting_rows[].optimizer_steps",
                self.optimizer_steps,
            ),
            (
                "resource_accounting_rows[].tokens_per_step",
                self.tokens_per_step,
            ),
            (
                "resource_accounting_rows[].max_context_tokens",
                self.max_context_tokens,
            ),
            (
                "resource_accounting_rows[].checkpoint_total_bytes",
                self.checkpoint_total_bytes,
            ),
            (
                "resource_accounting_rows[].optimizer_state_bytes",
                self.optimizer_state_bytes,
            ),
            (
                "resource_accounting_rows[].activation_headroom_bytes",
                self.activation_headroom_bytes,
            ),
            (
                "resource_accounting_rows[].expected_mean_tokens_per_second",
                self.expected_mean_tokens_per_second,
            ),
        ] {
            if value == 0 {
                return Err(
                    PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                        field: String::from(field),
                        detail: String::from("resource-accounting values must stay positive"),
                    },
                );
            }
        }
        Ok(())
    }
}

impl PsionActualPretrainingBoundedAblationConfig {
    fn validate(&self) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
        for (field, value) in [
            (
                "bounded_ablation_configs[].ablation_id",
                self.ablation_id.as_str(),
            ),
            (
                "bounded_ablation_configs[].ablation_family",
                self.ablation_family.as_str(),
            ),
            (
                "bounded_ablation_configs[].config_binding_id",
                self.config_binding_id.as_str(),
            ),
            (
                "bounded_ablation_configs[].consumption_target",
                self.consumption_target.as_str(),
            ),
            ("bounded_ablation_configs[].detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_artifact_ref(
            &self.stage_config,
            "bounded_ablation_configs[].stage_config",
        )?;
        ensure_artifact_ref(
            &self.model_descriptor,
            "bounded_ablation_configs[].model_descriptor",
        )?;
        for (field, value) in [
            (
                "bounded_ablation_configs[].max_train_token_budget",
                self.max_train_token_budget,
            ),
            (
                "bounded_ablation_configs[].max_validation_token_budget",
                self.max_validation_token_budget,
            ),
            (
                "bounded_ablation_configs[].max_optimizer_steps",
                self.max_optimizer_steps,
            ),
            (
                "bounded_ablation_configs[].tokens_per_step",
                self.tokens_per_step,
            ),
        ] {
            if value == 0 {
                return Err(
                    PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                        field: String::from(field),
                        detail: String::from("bounded ablation ceilings must stay positive"),
                    },
                );
            }
        }
        Ok(())
    }
}

fn validate_accounting_rows(
    rows: &[PsionActualPretrainingResourceAccountingRow],
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    if rows.len() < 2 {
        return Err(
            PsionActualPretrainingBaselineToolsBundleError::MissingField {
                field: String::from("resource_accounting_rows"),
            },
        );
    }
    let mut row_ids = BTreeSet::new();
    let mut scope_kinds = BTreeSet::new();
    for row in rows {
        row.validate()?;
        if !row_ids.insert(row.row_id.as_str()) {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                    field: String::from("resource_accounting_rows[].row_id"),
                    detail: format!("duplicate resource-accounting row `{}`", row.row_id),
                },
            );
        }
        scope_kinds.insert(row.scope_kind.as_str());
    }
    for required_scope in ["actual_lane", "bounded_ablation"] {
        if !scope_kinds.contains(required_scope) {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::MissingField {
                    field: format!("resource_accounting_rows[{required_scope}]"),
                },
            );
        }
    }
    Ok(())
}

fn validate_ablation_configs(
    configs: &[PsionActualPretrainingBoundedAblationConfig],
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    if configs.len() < 2 {
        return Err(
            PsionActualPretrainingBaselineToolsBundleError::MissingField {
                field: String::from("bounded_ablation_configs"),
            },
        );
    }
    let mut ids = BTreeSet::new();
    let mut consumption_targets = BTreeSet::new();
    for config in configs {
        config.validate()?;
        if !ids.insert(config.ablation_id.as_str()) {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                    field: String::from("bounded_ablation_configs[].ablation_id"),
                    detail: format!("duplicate bounded ablation id `{}`", config.ablation_id),
                },
            );
        }
        consumption_targets.insert(config.consumption_target.as_str());
    }
    for required_target in [
        "actual_lane_smoke_and_bringup",
        "actual_lane_bounded_ablation_family",
    ] {
        if !consumption_targets.contains(required_target) {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::MissingField {
                    field: format!("bounded_ablation_configs[{required_target}]"),
                },
            );
        }
    }
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    if actual != expected {
        return Err(
            PsionActualPretrainingBaselineToolsBundleError::FieldMismatch {
                field: String::from(field),
                expected: String::from(expected),
                actual: String::from(actual),
            },
        );
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    if value.trim().is_empty() {
        return Err(
            PsionActualPretrainingBaselineToolsBundleError::MissingField {
                field: String::from(field),
            },
        );
    }
    Ok(())
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingBaselineToolsBundleError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(
                PsionActualPretrainingBaselineToolsBundleError::UnsupportedValue {
                    field: String::from(field),
                    detail: format!("duplicate value `{value}`"),
                },
            );
        }
    }
    Ok(())
}

pub fn stable_baseline_tools_bundle_digest(
    bundle: &PsionActualPretrainingBaselineToolsBundle,
) -> Result<String, PsionActualPretrainingBaselineToolsBundleError> {
    let mut digest = Sha256::new();
    digest.update(b"psion.actual_pretraining_baseline_tools_bundle|");
    digest.update(
        serde_json::to_vec(&(
            bundle.schema_version.as_str(),
            bundle.baseline_tools_bundle_id.as_str(),
            bundle.lane_id.as_str(),
            &bundle.lane_spec,
            &bundle.recipe_bundle,
            &bundle.scaling_bundle,
            &bundle.data_bundle,
            &bundle.systems_bundle,
            &bundle.bringup_trainer,
            &bundle.tokenizer_reproducibility,
            &bundle.resource_accounting_rows,
            &bundle.bounded_ablation_configs,
            &bundle.support_refs,
            bundle.claim_boundary.as_str(),
            bundle.summary.as_str(),
        ))
        .map_err(PsionActualPretrainingBaselineToolsBundleError::Json)?,
    );
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Debug, Error)]
pub enum PsionActualPretrainingBaselineToolsBundleError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("unsupported value for `{field}`: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("failed to serialize baseline-tools bundle: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_bundle() -> PsionActualPretrainingBaselineToolsBundle {
        PsionActualPretrainingBaselineToolsBundle {
            schema_version: String::from(
                PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_SCHEMA_VERSION,
            ),
            baseline_tools_bundle_id: String::from(
                PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_ID,
            ),
            lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            lane_spec: PsionActualPretrainingArtifactRef {
                path: String::from(
                    "fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json",
                ),
                sha256: String::from("lane-sha"),
            },
            recipe_bundle: PsionActualPretrainingArtifactRef {
                path: String::from(
                    "fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json",
                ),
                sha256: String::from("recipe-sha"),
            },
            scaling_bundle: PsionActualPretrainingArtifactRef {
                path: String::from(
                    "fixtures/psion/pretrain/psion_actual_pretraining_scaling_bundle_v1.json",
                ),
                sha256: String::from("scaling-sha"),
            },
            data_bundle: PsionActualPretrainingArtifactRef {
                path: String::from(
                    "fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json",
                ),
                sha256: String::from("data-sha"),
            },
            systems_bundle: PsionActualPretrainingArtifactRef {
                path: String::from(
                    "fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json",
                ),
                sha256: String::from("systems-sha"),
            },
            bringup_trainer: PsionActualPretrainingBringupTrainer {
                trainer_id: String::from("bringup"),
                stage_program_id: String::from("psion_pretrain_stage"),
                trainer_entry_surface: String::from(
                    "crates/psionic-train/src/psion_pretrain_stage.rs",
                ),
                stage_config: PsionActualPretrainingArtifactRef {
                    path: String::from(
                        "fixtures/psion/pretrain/psion_actual_pretraining_bringup_stage_config_v1.json",
                    ),
                    sha256: String::from("bringup-config-sha"),
                },
                model_descriptor: PsionActualPretrainingArtifactRef {
                    path: String::from(
                        "fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json",
                    ),
                    sha256: String::from("descriptor-sha"),
                },
                model_id: String::from("psion-compact-decoder-internal-v1"),
                dataset_identity: String::from("psion_corpus_tokenized@v1"),
                sampling_policy_id: String::from("psion_pretrain_mix"),
                sampling_policy_version: String::from("v1"),
                tokenizer_binding_digest: String::from("binding-digest"),
                max_context_tokens: 8192,
                detail: String::from("detail"),
            },
            tokenizer_reproducibility: PsionActualPretrainingTokenizerReproducibilityBinding {
                tokenizer_training_manifest: PsionActualPretrainingArtifactRef {
                    path: String::from(
                        "fixtures/psion/tokenizer/psion_tokenizer_training_manifest_v1.json",
                    ),
                    sha256: String::from("tokenizer-manifest-sha"),
                },
                tokenizer_artifact_bundle: PsionActualPretrainingArtifactRef {
                    path: String::from(
                        "fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json",
                    ),
                    sha256: String::from("tokenizer-bundle-sha"),
                },
                tokenized_corpus_manifest: PsionActualPretrainingArtifactRef {
                    path: String::from(
                        "fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json",
                    ),
                    sha256: String::from("corpus-sha"),
                },
                tokenizer_id: String::from("psion_sentencepiece_seed"),
                tokenizer_version: String::from("v1"),
                tokenizer_digest: String::from("tokenizer-digest"),
                tokenizer_binding_digest: String::from("binding-digest"),
                tokenizer_only_source_ids: vec![String::from("vendor_manual_private_scan_v1")],
                model_training_source_ids: vec![String::from("arch_textbook_foster_1985")],
                held_out_source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                detail: String::from("detail"),
            },
            resource_accounting_rows: vec![
                PsionActualPretrainingResourceAccountingRow {
                    row_id: String::from("actual"),
                    scope_kind: String::from("actual_lane"),
                    config_binding_id: String::from("internal128m"),
                    model_id: String::from("psion-compact-decoder-internal-v1"),
                    size_anchor: String::from("internal128m"),
                    parameter_count_estimate: 10,
                    train_token_budget: 20,
                    validation_token_budget: 5,
                    held_out_token_budget: 1,
                    optimizer_steps: 2,
                    tokens_per_step: 10,
                    max_context_tokens: 8192,
                    checkpoint_total_bytes: 10,
                    optimizer_state_bytes: 5,
                    activation_headroom_bytes: 20,
                    expected_mean_tokens_per_second: 10,
                    detail: String::from("detail"),
                },
                PsionActualPretrainingResourceAccountingRow {
                    row_id: String::from("pilot"),
                    scope_kind: String::from("bounded_ablation"),
                    config_binding_id: String::from("pilot32m"),
                    model_id: String::from("psion-compact-decoder-pilot-v1"),
                    size_anchor: String::from("pilot32m"),
                    parameter_count_estimate: 10,
                    train_token_budget: 20,
                    validation_token_budget: 5,
                    held_out_token_budget: 1,
                    optimizer_steps: 2,
                    tokens_per_step: 10,
                    max_context_tokens: 4096,
                    checkpoint_total_bytes: 10,
                    optimizer_state_bytes: 5,
                    activation_headroom_bytes: 20,
                    expected_mean_tokens_per_second: 10,
                    detail: String::from("detail"),
                },
            ],
            bounded_ablation_configs: vec![
                PsionActualPretrainingBoundedAblationConfig {
                    ablation_id: String::from("smoke"),
                    ablation_family: String::from("family"),
                    stage_config: PsionActualPretrainingArtifactRef {
                        path: String::from(
                            "fixtures/psion/pretrain/psion_actual_pretraining_bringup_stage_config_v1.json",
                        ),
                        sha256: String::from("smoke-stage-sha"),
                    },
                    model_descriptor: PsionActualPretrainingArtifactRef {
                        path: String::from(
                            "fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json",
                        ),
                        sha256: String::from("smoke-descriptor-sha"),
                    },
                    config_binding_id: String::from("internal128m"),
                    max_train_token_budget: 20,
                    max_validation_token_budget: 5,
                    max_optimizer_steps: 2,
                    tokens_per_step: 10,
                    consumption_target: String::from("actual_lane_smoke_and_bringup"),
                    detail: String::from("detail"),
                },
                PsionActualPretrainingBoundedAblationConfig {
                    ablation_id: String::from("pilot"),
                    ablation_family: String::from("family"),
                    stage_config: PsionActualPretrainingArtifactRef {
                        path: String::from(
                            "fixtures/psion/pretrain/psion_actual_pretraining_pilot32m_ablation_stage_config_v1.json",
                        ),
                        sha256: String::from("pilot-stage-sha"),
                    },
                    model_descriptor: PsionActualPretrainingArtifactRef {
                        path: String::from(
                            "fixtures/psion/models/psion_compact_decoder_pilot_descriptor_v1.json",
                        ),
                        sha256: String::from("pilot-descriptor-sha"),
                    },
                    config_binding_id: String::from("pilot32m"),
                    max_train_token_budget: 20,
                    max_validation_token_budget: 5,
                    max_optimizer_steps: 2,
                    tokens_per_step: 10,
                    consumption_target: String::from("actual_lane_bounded_ablation_family"),
                    detail: String::from("detail"),
                },
            ],
            support_refs: vec![String::from("docs/TRAIN_SYSTEM.md")],
            claim_boundary: String::from("claim boundary"),
            summary: String::from("summary"),
            bundle_digest: String::new(),
        }
    }

    #[test]
    fn baseline_tools_bundle_validates() {
        let mut bundle = fixture_bundle();
        bundle.bundle_digest = stable_baseline_tools_bundle_digest(&bundle).unwrap();
        bundle.validate().unwrap();
    }

    #[test]
    fn baseline_tools_bundle_requires_bounded_ablation_consumption_targets() {
        let mut bundle = fixture_bundle();
        bundle.bounded_ablation_configs[1].consumption_target = String::from("wrong");
        bundle.bundle_digest = stable_baseline_tools_bundle_digest(&bundle).unwrap();
        let error = bundle.validate().unwrap_err();
        assert!(matches!(
            error,
            PsionActualPretrainingBaselineToolsBundleError::MissingField { .. }
        ));
    }
}
