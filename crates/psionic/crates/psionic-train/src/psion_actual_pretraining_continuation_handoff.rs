use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    PsionActualPretrainingArtifactRef, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingRecipeBundle, PsionPluginBenchmarkFamily,
    PsionPluginConditionedBenchmarkBinding, PsionPluginConditionedEvalHook,
    PsionPluginConditionedEvalHookKind, PsionPluginConditionedEvalTrigger,
    PsionPluginConditionedSftStageManifest, PSION_ACTUAL_PRETRAINING_CONTINUATION_PATH,
    PSION_ACTUAL_PRETRAINING_LANE_ID,
};

pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_continuation_handoff.v1";
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID: &str =
    "psion_actual_pretraining_general_sft_agentic_sft_v1";
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH: &str =
    "continuation/accepted_checkpoint_handoff.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAcceptedCheckpointBinding {
    pub checkpoint_pointer_path: String,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub checkpoint_manifest_relative_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationHandoff {
    pub schema_version: String,
    pub lane_id: String,
    pub continuation_target_id: String,
    pub run_id: String,
    pub stage_path: Vec<String>,
    pub accepted_checkpoint: PsionActualPretrainingAcceptedCheckpointBinding,
    pub reasoning_sft_run_bundle: PsionActualPretrainingArtifactRef,
    pub plugin_conditioned_stage_manifest: PsionActualPretrainingArtifactRef,
    pub plugin_conditioned_run_bundle: PsionActualPretrainingArtifactRef,
    pub continuation_eval_pack: PsionActualPretrainingArtifactRef,
    pub benchmark_bindings: Vec<PsionPluginConditionedBenchmarkBinding>,
    pub eval_hooks: Vec<PsionPluginConditionedEvalHook>,
    pub claim_boundary: String,
    pub detail: String,
}

impl PsionActualPretrainingAcceptedCheckpointBinding {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
        ensure_exact(
            self.checkpoint_pointer_path.as_str(),
            "accepted_checkpoint.checkpoint_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_nonempty(
            self.checkpoint_label.as_str(),
            "accepted_checkpoint.checkpoint_label",
        )?;
        ensure_positive(self.optimizer_step, "accepted_checkpoint.optimizer_step")?;
        ensure_nonempty(
            self.checkpoint_ref.as_str(),
            "accepted_checkpoint.checkpoint_ref",
        )?;
        ensure_nonempty(
            self.checkpoint_manifest_relative_path.as_str(),
            "accepted_checkpoint.checkpoint_manifest_relative_path",
        )?;
        ensure_nonempty(self.detail.as_str(), "accepted_checkpoint.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingContinuationHandoff {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
        ensure_exact(
            self.schema_version.as_str(),
            "continuation_handoff.schema_version",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "continuation_handoff.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_exact(
            self.continuation_target_id.as_str(),
            "continuation_handoff.continuation_target_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "continuation_handoff.run_id")?;
        let expected_stage_path = PSION_ACTUAL_PRETRAINING_CONTINUATION_PATH
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        if self.stage_path != expected_stage_path {
            return Err(
                PsionActualPretrainingContinuationHandoffError::FieldMismatch {
                    field: String::from("continuation_handoff.stage_path"),
                    expected: format!("{expected_stage_path:?}"),
                    actual: format!("{:?}", self.stage_path),
                },
            );
        }
        self.accepted_checkpoint.validate()?;
        ensure_artifact_ref(
            &self.reasoning_sft_run_bundle,
            "continuation_handoff.reasoning_sft_run_bundle",
        )?;
        ensure_artifact_ref(
            &self.plugin_conditioned_stage_manifest,
            "continuation_handoff.plugin_conditioned_stage_manifest",
        )?;
        ensure_artifact_ref(
            &self.plugin_conditioned_run_bundle,
            "continuation_handoff.plugin_conditioned_run_bundle",
        )?;
        ensure_artifact_ref(
            &self.continuation_eval_pack,
            "continuation_handoff.continuation_eval_pack",
        )?;
        validate_benchmark_bindings(self.benchmark_bindings.as_slice())?;
        validate_eval_hooks(self.eval_hooks.as_slice(), self.benchmark_bindings.as_slice())?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "continuation_handoff.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "continuation_handoff.detail")?;
        Ok(())
    }
}

pub fn record_psion_actual_pretraining_continuation_handoff(
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    recipe_bundle: &PsionActualPretrainingRecipeBundle,
    plugin_conditioned_stage_manifest: &PsionPluginConditionedSftStageManifest,
) -> Result<PsionActualPretrainingContinuationHandoff, PsionActualPretrainingContinuationHandoffError>
{
    checkpoint_pointer.validate().map_err(|error| {
        PsionActualPretrainingContinuationHandoffError::InvalidUpstreamSurface {
            surface: String::from("checkpoint_pointer"),
            detail: error.to_string(),
        }
    })?;
    recipe_bundle.validate().map_err(|error| {
        PsionActualPretrainingContinuationHandoffError::InvalidUpstreamSurface {
            surface: String::from("recipe_bundle"),
            detail: error.to_string(),
        }
    })?;
    ensure_exact(
        checkpoint_pointer.pointer_state.as_str(),
        "checkpoint_pointer.pointer_state",
        "accepted",
    )?;

    let handoff = PsionActualPretrainingContinuationHandoff {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        continuation_target_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID),
        run_id: checkpoint_pointer.run_id.clone(),
        stage_path: recipe_bundle.continuation_target.stage_path.clone(),
        accepted_checkpoint: PsionActualPretrainingAcceptedCheckpointBinding {
            checkpoint_pointer_path: String::from(
                "checkpoints/latest_accepted_checkpoint_pointer.json",
            ),
            checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
            optimizer_step: checkpoint_pointer.optimizer_step,
            checkpoint_ref: checkpoint_pointer
                .checkpoint_ref
                .clone()
                .ok_or_else(|| PsionActualPretrainingContinuationHandoffError::MissingField {
                    field: String::from("checkpoint_pointer.checkpoint_ref"),
                })?,
            checkpoint_manifest_relative_path: checkpoint_pointer
                .checkpoint_manifest_relative_path
                .clone()
                .ok_or_else(|| PsionActualPretrainingContinuationHandoffError::MissingField {
                    field: String::from("checkpoint_pointer.checkpoint_manifest_relative_path"),
                })?,
            detail: String::from(
                "Accepted checkpoint binding freezes the exact broader-pretraining checkpoint that may feed the bounded continuation target.",
            ),
        },
        reasoning_sft_run_bundle: recipe_bundle
            .continuation_target
            .reasoning_sft_run_bundle
            .clone(),
        plugin_conditioned_stage_manifest: recipe_bundle
            .continuation_target
            .plugin_conditioned_stage_manifest
            .clone(),
        plugin_conditioned_run_bundle: recipe_bundle
            .continuation_target
            .plugin_conditioned_run_bundle
            .clone(),
        continuation_eval_pack: recipe_bundle
            .continuation_target
            .continuation_eval_pack
            .clone(),
        benchmark_bindings: plugin_conditioned_stage_manifest.benchmark_bindings.clone(),
        eval_hooks: plugin_conditioned_stage_manifest.eval_hooks.clone(),
        claim_boundary: String::from(
            "This handoff binds one accepted actual-pretraining checkpoint to the frozen reasoning `general_sft` bridge and bounded plugin-conditioned `agentic_sft` target, and carries the bounded continuation-stage eval pack for later review. It does not claim cluster-scale plugin-conditioned training or continuation-stage execution.",
        ),
        detail: String::from(
            "Continuation handoff closes the actual lane into one named continuation target and preserves the plugin benchmark-pack bindings plus the bounded continuation-stage eval pack already attached to that target.",
        ),
    };
    handoff.validate()?;
    Ok(handoff)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingContinuationHandoffError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` mismatch: expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("invalid benchmark-family set: expected `{expected}`, found `{actual}`")]
    InvalidBenchmarkFamilySet { expected: String, actual: String },
    #[error("eval hooks lost the required trigger `{trigger}`")]
    MissingEvalTrigger { trigger: String },
    #[error("invalid upstream surface `{surface}`: {detail}")]
    InvalidUpstreamSurface { surface: String, detail: String },
}

fn validate_benchmark_bindings(
    benchmark_bindings: &[PsionPluginConditionedBenchmarkBinding],
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    if benchmark_bindings.is_empty() {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingField {
            field: String::from("continuation_handoff.benchmark_bindings"),
        });
    }
    let expected = required_benchmark_families();
    let observed = benchmark_bindings
        .iter()
        .map(|binding| {
            ensure_nonempty(
                binding.bundle_ref.as_str(),
                "continuation_handoff.benchmark_binding.bundle_ref",
            )?;
            ensure_nonempty(
                binding.bundle_digest.as_str(),
                "continuation_handoff.benchmark_binding.bundle_digest",
            )?;
            ensure_nonempty(
                binding.package_id.as_str(),
                "continuation_handoff.benchmark_binding.package_id",
            )?;
            ensure_nonempty(
                binding.package_digest.as_str(),
                "continuation_handoff.benchmark_binding.package_digest",
            )?;
            ensure_nonempty(
                binding.receipt_digest.as_str(),
                "continuation_handoff.benchmark_binding.receipt_digest",
            )?;
            ensure_nonempty(
                binding.detail.as_str(),
                "continuation_handoff.benchmark_binding.detail",
            )?;
            Ok(binding.benchmark_family)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if observed != expected {
        return Err(
            PsionActualPretrainingContinuationHandoffError::InvalidBenchmarkFamilySet {
                expected: format!("{expected:?}"),
                actual: format!("{observed:?}"),
            },
        );
    }
    Ok(())
}

fn validate_eval_hooks(
    eval_hooks: &[PsionPluginConditionedEvalHook],
    benchmark_bindings: &[PsionPluginConditionedBenchmarkBinding],
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    if eval_hooks.is_empty() {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingField {
            field: String::from("continuation_handoff.eval_hooks"),
        });
    }
    let bound_families = benchmark_bindings
        .iter()
        .map(|binding| binding.benchmark_family)
        .collect::<BTreeSet<_>>();
    let mut saw_post_stage_completion = false;
    let mut saw_pre_promotion_audit = false;
    let mut saw_replay_receipt_review = false;
    for hook in eval_hooks {
        ensure_nonempty(
            hook.hook_id.as_str(),
            "continuation_handoff.eval_hook.hook_id",
        )?;
        ensure_nonempty(
            hook.detail.as_str(),
            "continuation_handoff.eval_hook.detail",
        )?;
        match hook.trigger {
            PsionPluginConditionedEvalTrigger::PostStageCompletion => {
                saw_post_stage_completion = true;
            }
            PsionPluginConditionedEvalTrigger::PrePromotionAudit => {
                saw_pre_promotion_audit = true;
            }
        }
        match hook.hook_kind {
            PsionPluginConditionedEvalHookKind::BenchmarkSweep => {
                let benchmark_family = hook.benchmark_family.ok_or_else(|| {
                    PsionActualPretrainingContinuationHandoffError::MissingField {
                        field: String::from("continuation_handoff.eval_hook.benchmark_family"),
                    }
                })?;
                if !bound_families.contains(&benchmark_family) {
                    return Err(PsionActualPretrainingContinuationHandoffError::FieldMismatch {
                        field: String::from("continuation_handoff.eval_hook.benchmark_family"),
                        expected: format!("{bound_families:?}"),
                        actual: format!("{benchmark_family:?}"),
                    });
                }
                ensure_nonempty(
                    hook.benchmark_bundle_ref.as_deref().ok_or_else(|| {
                        PsionActualPretrainingContinuationHandoffError::MissingField {
                            field: String::from(
                                "continuation_handoff.eval_hook.benchmark_bundle_ref",
                            ),
                        }
                    })?,
                    "continuation_handoff.eval_hook.benchmark_bundle_ref",
                )?;
                ensure_nonempty(
                    hook.benchmark_receipt_digest.as_deref().ok_or_else(|| {
                        PsionActualPretrainingContinuationHandoffError::MissingField {
                            field: String::from(
                                "continuation_handoff.eval_hook.benchmark_receipt_digest",
                            ),
                        }
                    })?,
                    "continuation_handoff.eval_hook.benchmark_receipt_digest",
                )?;
            }
            PsionPluginConditionedEvalHookKind::ReplayReceiptReview => {
                saw_replay_receipt_review = true;
            }
        }
    }
    if !saw_post_stage_completion {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingEvalTrigger {
            trigger: String::from("post_stage_completion"),
        });
    }
    if !saw_pre_promotion_audit {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingEvalTrigger {
            trigger: String::from("pre_promotion_audit"),
        });
    }
    if !saw_replay_receipt_review {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingField {
            field: String::from("continuation_handoff.eval_hooks.replay_receipt_review"),
        });
    }
    Ok(())
}

fn required_benchmark_families() -> BTreeSet<PsionPluginBenchmarkFamily> {
    [
        PsionPluginBenchmarkFamily::DiscoverySelection,
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        PsionPluginBenchmarkFamily::ResultInterpretation,
    ]
    .into_iter()
    .collect()
}

fn ensure_artifact_ref(
    artifact_ref: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    ensure_nonempty(artifact_ref.path.as_str(), format!("{field_prefix}.path").as_str())?;
    ensure_nonempty(
        artifact_ref.sha256.as_str(),
        format!("{field_prefix}.sha256").as_str(),
    )?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    if actual != expected {
        return Err(PsionActualPretrainingContinuationHandoffError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_positive(
    value: u64,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffError> {
    if value == 0 {
        return Err(PsionActualPretrainingContinuationHandoffError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handoff_fixture() -> PsionActualPretrainingContinuationHandoff {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_v1.json"
        ))
        .expect("continuation handoff fixture should parse")
    }

    #[test]
    fn actual_pretraining_continuation_handoff_fixture_validates() {
        handoff_fixture()
            .validate()
            .expect("continuation handoff fixture should validate");
    }

    #[test]
    fn actual_pretraining_continuation_handoff_requires_replay_receipt_hook() {
        let mut handoff = handoff_fixture();
        handoff
            .eval_hooks
            .retain(|hook| hook.hook_kind != PsionPluginConditionedEvalHookKind::ReplayReceiptReview);
        let error = handoff
            .validate()
            .expect_err("handoff should reject missing replay receipt review hook");
        assert_eq!(
            error,
            PsionActualPretrainingContinuationHandoffError::MissingField {
                field: String::from("continuation_handoff.eval_hooks.replay_receipt_review"),
            }
        );
    }
}
