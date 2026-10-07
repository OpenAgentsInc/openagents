use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PSION_ACTUAL_PRETRAINING_LANE_ID;

/// Stable schema version for the canonical Psion actual-pretraining recipe bundle.
pub const PSION_ACTUAL_PRETRAINING_RECIPE_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_recipe_bundle.v1";

/// Stable schema version for the canonical Psion topology and storage bundle.
pub const PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_topology_storage_bundle.v1";

/// Stable recipe identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_RECIPE_ID: &str = "psion_actual_pretraining_recipe_v1";

/// Stable topology and storage bundle identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID: &str =
    "psion_actual_pretraining_topology_storage_bundle_v1";

/// Stable continuation path from the actual pretrain lane into bounded later stages.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_PATH: [&str; 3] =
    ["pretrain", "general_sft", "agentic_sft"];

/// Stable file reference with an explicit SHA-256 digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingArtifactRef {
    /// Workspace-relative path to the committed artifact.
    pub path: String,
    /// SHA-256 of the committed artifact contents.
    pub sha256: String,
}

/// Fixed-budget schedule for the canonical actual-pretraining recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingStageSchedule {
    /// Bounded stage kinds for the base lane recipe itself.
    pub base_stage_kinds: Vec<String>,
    /// Planned train-token budget.
    pub train_token_budget: u64,
    /// Planned validation-token budget.
    pub validation_token_budget: u64,
    /// Planned held-out scoring budget.
    pub held_out_token_budget: u64,
    /// Planned optimizer steps for the bounded actual-lane anchor.
    pub optimizer_steps: u64,
    /// Maximum context length admitted by the recipe.
    pub max_context_tokens: u64,
}

/// Declared bounded continuation path after the actual pretrain stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationTarget {
    /// Ordered continuation path above the actual pretrain stage.
    pub stage_path: Vec<String>,
    /// Bounded reasoning-SFT bundle used as the canonical `general_sft` bridge.
    pub reasoning_sft_run_bundle: PsionActualPretrainingArtifactRef,
    /// Bounded plugin-conditioned stage manifest used as the canonical plugin handoff target.
    pub plugin_conditioned_stage_manifest: PsionActualPretrainingArtifactRef,
    /// Bounded plugin-conditioned run bundle carrying the `general_sft -> agentic_sft` proof.
    pub plugin_conditioned_run_bundle: PsionActualPretrainingArtifactRef,
    /// Continuation-stage eval pack used for bounded reasoning and post-training review.
    pub continuation_eval_pack: PsionActualPretrainingArtifactRef,
    /// Narrow claim boundary for the declared continuation target.
    pub claim_boundary: String,
}

/// Machine-readable actual-pretraining recipe authority bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRecipeBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable recipe identifier.
    pub recipe_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Actual-lane spec this recipe belongs to.
    pub lane_spec: PsionActualPretrainingArtifactRef,
    /// Canonical model identifier.
    pub model_id: String,
    /// Canonical model descriptor ref for the actual lane.
    pub model_descriptor: PsionActualPretrainingArtifactRef,
    /// Stable descriptor digest already carried by the trusted-cluster bundle.
    pub model_descriptor_digest: String,
    /// Canonical tokenizer identifier.
    pub tokenizer_id: String,
    /// Canonical tokenizer version.
    pub tokenizer_version: String,
    /// Canonical tokenizer digest.
    pub tokenizer_digest: String,
    /// Canonical tokenized corpus manifest.
    pub tokenized_corpus_manifest: PsionActualPretrainingArtifactRef,
    /// Stable dataset identity.
    pub dataset_identity: String,
    /// Canonical mixture policy identifier.
    pub sampling_policy_id: String,
    /// Canonical mixture policy version.
    pub sampling_policy_version: String,
    /// Canonical mixture policy manifest.
    pub sampling_policy_manifest: PsionActualPretrainingArtifactRef,
    /// Fixed-budget base stage schedule.
    pub stage_schedule: PsionActualPretrainingStageSchedule,
    /// Declared bounded continuation path above pretrain.
    pub continuation_target: PsionActualPretrainingContinuationTarget,
    /// Short summary.
    pub summary: String,
}

/// One declared storage tier in the canonical topology and storage bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingStorageTier {
    /// Relative prefix within the actual-lane run family.
    pub prefix: String,
    /// Durability class for that prefix.
    pub durability_class: String,
    /// Short detail.
    pub detail: String,
}

/// Declared non-embedded credential source for storage or remote backends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCredentialSource {
    /// Secret-source kind.
    pub kind: String,
    /// Environment variable or source name.
    pub source_name: String,
    /// What this source unlocks.
    pub purpose: String,
    /// How retained artifacts refer to the source without leaking the payload.
    pub retained_redaction: String,
}

/// Machine-readable topology, storage, and secret-source authority bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingTopologyStorageBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Admitted topology contract.
    pub topology_contract: PsionActualPretrainingArtifactRef,
    /// Admitted topology label.
    pub supported_topology_label: String,
    /// Required backend.
    pub required_backend: String,
    /// Required worker count.
    pub required_worker_count: u64,
    /// Placement shape admitted by the topology bundle.
    pub placement_shape: String,
    /// Run-root template for durable remote artifacts.
    pub remote_run_root_template: String,
    /// Checkpoint-root template for durable remote checkpoints.
    pub remote_checkpoint_root_template: String,
    /// Manifest-root template for durable retained manifests.
    pub remote_manifest_root_template: String,
    /// Log-root template for transient logs.
    pub remote_log_root_template: String,
    /// Storage tier layout for the actual lane.
    pub storage_tiers: Vec<PsionActualPretrainingStorageTier>,
    /// Credential-source declarations for storage and remote backends.
    pub credential_sources: Vec<PsionActualPretrainingCredentialSource>,
    /// Short summary.
    pub summary: String,
}

impl PsionActualPretrainingRecipeBundle {
    /// Validates the recipe bundle.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingRecipeBundleError> {
        ensure_nonempty(self.schema_version.as_str(), "recipe.schema_version")?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_RECIPE_BUNDLE_SCHEMA_VERSION {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("schema_version"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.recipe_id.as_str(), "recipe.recipe_id")?;
        if self.recipe_id != PSION_ACTUAL_PRETRAINING_RECIPE_ID {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("recipe_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
                actual: self.recipe_id.clone(),
            });
        }
        ensure_lane_id(self.lane_id.as_str())?;
        ensure_artifact_ref(&self.lane_spec, "recipe.lane_spec")?;
        ensure_nonempty(self.model_id.as_str(), "recipe.model_id")?;
        ensure_artifact_ref(&self.model_descriptor, "recipe.model_descriptor")?;
        ensure_nonempty(
            self.model_descriptor_digest.as_str(),
            "recipe.model_descriptor_digest",
        )?;
        ensure_nonempty(self.tokenizer_id.as_str(), "recipe.tokenizer_id")?;
        ensure_nonempty(
            self.tokenizer_version.as_str(),
            "recipe.tokenizer_version",
        )?;
        ensure_nonempty(
            self.tokenizer_digest.as_str(),
            "recipe.tokenizer_digest",
        )?;
        ensure_artifact_ref(
            &self.tokenized_corpus_manifest,
            "recipe.tokenized_corpus_manifest",
        )?;
        if self.dataset_identity != "psion_corpus_tokenized@v1" {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("dataset_identity"),
                expected: String::from("psion_corpus_tokenized@v1"),
                actual: self.dataset_identity.clone(),
            });
        }
        if self.sampling_policy_id != "psion_pretrain_mix" {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("sampling_policy_id"),
                expected: String::from("psion_pretrain_mix"),
                actual: self.sampling_policy_id.clone(),
            });
        }
        ensure_nonempty(
            self.sampling_policy_version.as_str(),
            "recipe.sampling_policy_version",
        )?;
        ensure_artifact_ref(
            &self.sampling_policy_manifest,
            "recipe.sampling_policy_manifest",
        )?;
        if self.stage_schedule.base_stage_kinds != [String::from("pretrain")] {
            return Err(PsionActualPretrainingRecipeBundleError::UnexpectedStageKinds {
                field: String::from("stage_schedule.base_stage_kinds"),
                expected: vec![String::from("pretrain")],
                actual: self.stage_schedule.base_stage_kinds.clone(),
            });
        }
        ensure_positive(
            self.stage_schedule.train_token_budget,
            "stage_schedule.train_token_budget",
        )?;
        ensure_positive(
            self.stage_schedule.validation_token_budget,
            "stage_schedule.validation_token_budget",
        )?;
        ensure_positive(
            self.stage_schedule.held_out_token_budget,
            "stage_schedule.held_out_token_budget",
        )?;
        ensure_positive(
            self.stage_schedule.optimizer_steps,
            "stage_schedule.optimizer_steps",
        )?;
        ensure_positive(
            self.stage_schedule.max_context_tokens,
            "stage_schedule.max_context_tokens",
        )?;
        if self.continuation_target.stage_path
            != PSION_ACTUAL_PRETRAINING_CONTINUATION_PATH
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        {
            return Err(PsionActualPretrainingRecipeBundleError::UnexpectedStageKinds {
                field: String::from("continuation_target.stage_path"),
                expected: PSION_ACTUAL_PRETRAINING_CONTINUATION_PATH
                    .into_iter()
                    .map(String::from)
                    .collect(),
                actual: self.continuation_target.stage_path.clone(),
            });
        }
        ensure_artifact_ref(
            &self.continuation_target.reasoning_sft_run_bundle,
            "continuation_target.reasoning_sft_run_bundle",
        )?;
        ensure_artifact_ref(
            &self.continuation_target.plugin_conditioned_stage_manifest,
            "continuation_target.plugin_conditioned_stage_manifest",
        )?;
        ensure_artifact_ref(
            &self.continuation_target.plugin_conditioned_run_bundle,
            "continuation_target.plugin_conditioned_run_bundle",
        )?;
        ensure_artifact_ref(
            &self.continuation_target.continuation_eval_pack,
            "continuation_target.continuation_eval_pack",
        )?;
        ensure_nonempty(
            self.continuation_target.claim_boundary.as_str(),
            "continuation_target.claim_boundary",
        )?;
        ensure_nonempty(self.summary.as_str(), "recipe.summary")?;
        Ok(())
    }
}

impl PsionActualPretrainingTopologyStorageBundle {
    /// Validates the topology and storage bundle.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingRecipeBundleError> {
        ensure_nonempty(self.schema_version.as_str(), "topology.schema_version")?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_SCHEMA_VERSION {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("schema_version"),
                expected: String::from(
                    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_SCHEMA_VERSION,
                ),
                actual: self.schema_version.clone(),
            });
        }
        if self.bundle_id != PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("bundle_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID),
                actual: self.bundle_id.clone(),
            });
        }
        ensure_lane_id(self.lane_id.as_str())?;
        ensure_artifact_ref(&self.topology_contract, "topology.topology_contract")?;
        ensure_nonempty(
            self.supported_topology_label.as_str(),
            "topology.supported_topology_label",
        )?;
        if self.required_backend != "cuda" {
            return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
                field: String::from("required_backend"),
                expected: String::from("cuda"),
                actual: self.required_backend.clone(),
            });
        }
        ensure_positive(
            self.required_worker_count,
            "topology.required_worker_count",
        )?;
        ensure_nonempty(self.placement_shape.as_str(), "topology.placement_shape")?;
        ensure_nonempty(
            self.remote_run_root_template.as_str(),
            "topology.remote_run_root_template",
        )?;
        ensure_nonempty(
            self.remote_checkpoint_root_template.as_str(),
            "topology.remote_checkpoint_root_template",
        )?;
        ensure_nonempty(
            self.remote_manifest_root_template.as_str(),
            "topology.remote_manifest_root_template",
        )?;
        ensure_nonempty(
            self.remote_log_root_template.as_str(),
            "topology.remote_log_root_template",
        )?;
        if self.storage_tiers.is_empty() {
            return Err(PsionActualPretrainingRecipeBundleError::MissingStorageTiers);
        }
        for tier in &self.storage_tiers {
            ensure_nonempty(tier.prefix.as_str(), "topology.storage_tier.prefix")?;
            ensure_nonempty(
                tier.durability_class.as_str(),
                "topology.storage_tier.durability_class",
            )?;
            ensure_nonempty(tier.detail.as_str(), "topology.storage_tier.detail")?;
        }
        if self.credential_sources.is_empty() {
            return Err(PsionActualPretrainingRecipeBundleError::MissingCredentialSources);
        }
        for source in &self.credential_sources {
            ensure_nonempty(source.kind.as_str(), "topology.credential_source.kind")?;
            ensure_nonempty(
                source.source_name.as_str(),
                "topology.credential_source.source_name",
            )?;
            ensure_nonempty(
                source.purpose.as_str(),
                "topology.credential_source.purpose",
            )?;
            ensure_nonempty(
                source.retained_redaction.as_str(),
                "topology.credential_source.retained_redaction",
            )?;
            if source.kind != "environment_variable" && source.kind != "secret_file_env" {
                return Err(PsionActualPretrainingRecipeBundleError::UnsupportedCredentialSource {
                    kind: source.kind.clone(),
                });
            }
        }
        ensure_nonempty(self.summary.as_str(), "topology.summary")?;
        Ok(())
    }
}

/// Validation errors for the actual-pretraining recipe and topology bundles.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingRecipeBundleError {
    #[error("psion actual-pretraining field `{field}` must not be empty")]
    MissingField { field: String },
    #[error("psion actual-pretraining field `{field}` must be greater than zero")]
    NonPositiveValue { field: String },
    #[error(
        "psion actual-pretraining field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    SchemaVersionMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error(
        "psion actual-pretraining field `{field}` stage kinds mismatch: expected `{expected:?}`, got `{actual:?}`"
    )]
    UnexpectedStageKinds {
        field: String,
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("psion actual-pretraining bundle is missing storage tiers")]
    MissingStorageTiers,
    #[error("psion actual-pretraining bundle is missing credential-source declarations")]
    MissingCredentialSources,
    #[error("psion actual-pretraining credential source kind `{kind}` is not supported")]
    UnsupportedCredentialSource { kind: String },
}

fn ensure_lane_id(lane_id: &str) -> Result<(), PsionActualPretrainingRecipeBundleError> {
    ensure_nonempty(lane_id, "lane_id")?;
    if lane_id != PSION_ACTUAL_PRETRAINING_LANE_ID {
        return Err(PsionActualPretrainingRecipeBundleError::SchemaVersionMismatch {
            field: String::from("lane_id"),
            expected: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            actual: String::from(lane_id),
        });
    }
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingRecipeBundleError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_positive(
    value: u64,
    field: &str,
) -> Result<(), PsionActualPretrainingRecipeBundleError> {
    if value == 0 {
        return Err(PsionActualPretrainingRecipeBundleError::NonPositiveValue {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingRecipeBundleError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingRecipeBundleError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingRecipeBundle, PsionActualPretrainingTopologyStorageBundle,
    };

    fn recipe_bundle() -> PsionActualPretrainingRecipeBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json"
        ))
        .expect("actual pretraining recipe bundle fixture should parse")
    }

    fn topology_bundle() -> PsionActualPretrainingTopologyStorageBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json"
        ))
        .expect("actual pretraining topology bundle fixture should parse")
    }

    #[test]
    fn actual_pretraining_recipe_bundle_fixture_validates() {
        recipe_bundle()
            .validate()
            .expect("actual pretraining recipe bundle fixture should validate");
    }

    #[test]
    fn actual_pretraining_topology_storage_bundle_fixture_validates() {
        topology_bundle()
            .validate()
            .expect("actual pretraining topology bundle fixture should validate");
    }

    #[test]
    fn actual_pretraining_topology_bundle_rejects_missing_credential_sources() {
        let mut bundle = topology_bundle();
        bundle.credential_sources.clear();
        let error = bundle
            .validate()
            .expect_err("missing credential sources should be rejected");
        assert_eq!(
            error,
            super::PsionActualPretrainingRecipeBundleError::MissingCredentialSources
        );
    }
}
