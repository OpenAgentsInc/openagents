use psionic_eval::BenchmarkPackage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AgenticSftRlReferenceProgramReport, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingContinuationHandoff, PsionPluginConditionedSftRunBundle,
    PsionReasoningSftRunBundle, TrainingStageKind, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID,
};

/// Stable schema version for the canonical continuation-alignment bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_continuation_alignment_bundle.v1";

/// Stable bundle identifier for the canonical continuation-alignment bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_ID: &str =
    "psion_actual_pretraining_continuation_alignment_bundle_v1";

/// Canonical committed fixture path for the continuation-alignment bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_continuation_alignment_bundle_v1.json";

/// Frozen reasoning-bridge receipt binding carried by the continuation bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingReasoningBridgeBinding {
    /// Canonical reasoning-SFT run bundle ref.
    pub run_bundle: PsionActualPretrainingArtifactRef,
    /// Stable stage receipt identifier.
    pub stage_receipt_id: String,
    /// Stable stage receipt digest.
    pub stage_receipt_digest: String,
    /// Stable evaluation receipt identifier.
    pub evaluation_receipt_id: String,
    /// Stable evaluation receipt digest.
    pub evaluation_receipt_digest: String,
    /// Retained multi-style pass rate in basis points.
    pub multiple_valid_style_pass_rate_bps: u32,
    /// Short explanation of the reasoning bridge.
    pub detail: String,
}

/// Frozen agentic-SFT receipt binding carried by the continuation bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAgenticStageBinding {
    /// Canonical plugin-conditioned run bundle ref.
    pub run_bundle: PsionActualPretrainingArtifactRef,
    /// Stable stage receipt identifier.
    pub stage_receipt_id: String,
    /// Stable stage receipt digest.
    pub stage_receipt_digest: String,
    /// General-SFT completion digest promoted into the agentic stage.
    pub general_sft_completion_digest: String,
    /// Agentic-SFT completion digest for the bounded plugin-conditioned stage.
    pub agentic_sft_completion_digest: String,
    /// Retained later eval-hook count for the stage.
    pub eval_hook_count: u32,
    /// Short explanation of the agentic stage binding.
    pub detail: String,
}

/// Frozen post-training reference binding carried by the continuation bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingPostTrainingReferenceBinding {
    /// Stable reference-program run identifier.
    pub reference_program_run_id: String,
    /// Current stage kind reached by the post-training reference program.
    pub current_stage_kind: TrainingStageKind,
    /// Stable general-SFT checkpoint ref surfaced by the reference program.
    pub general_checkpoint_ref: String,
    /// Stable agentic-SFT checkpoint ref surfaced by the reference program.
    pub agentic_checkpoint_ref: String,
    /// Stable target policy revision id.
    pub target_policy_revision_id: String,
    /// Stable target policy checkpoint ref.
    pub target_policy_checkpoint_ref: String,
    /// Stable broadcast digest for delivered policy weights.
    pub policy_weight_broadcast_digest: String,
    /// Stable online-eval run id.
    pub online_eval_run_id: String,
    /// Stable benchmark-eval run id.
    pub benchmark_eval_run_id: String,
    /// Accepted rollout count retained by the operator view.
    pub accepted_rollout_count: u32,
    /// Accepted validator verdict count retained by the operator view.
    pub validator_accepted_count: u32,
    /// Completed trainer-step count retained by the operator view.
    pub completed_trainer_steps: u64,
    /// Short explanation of the post-training reference surface.
    pub detail: String,
}

/// Frozen eval-pack binding carried by the continuation bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationEvalPackBinding {
    /// Canonical continuation-eval pack artifact ref.
    pub eval_pack_artifact: PsionActualPretrainingArtifactRef,
    /// Stable benchmark ref.
    pub benchmark_ref: String,
    /// Stable benchmark version.
    pub benchmark_version: String,
    /// Stable `benchmark_ref@version` storage key.
    pub package_storage_key: String,
    /// Stable ordered case ids.
    pub case_ids: Vec<String>,
    /// Short explanation of the continuation-eval pack.
    pub detail: String,
}

/// Canonical actual-lane continuation alignment bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationAlignmentBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle id.
    pub bundle_id: String,
    /// Stable actual-lane id.
    pub lane_id: String,
    /// Stable continuation target id.
    pub continuation_target_id: String,
    /// Actual-lane run id that produced the accepted checkpoint.
    pub accepted_checkpoint_run_id: String,
    /// Ordered actual-lane stage path.
    pub actual_lane_stage_path: Vec<String>,
    /// Ordered post-training reference path.
    pub post_training_reference_stage_path: Vec<String>,
    /// Stable accepted checkpoint ref.
    pub accepted_checkpoint_ref: String,
    /// Reserved retained-path location for the actual handoff contract.
    pub handoff_contract_path: String,
    /// Reasoning-SFT bridge binding.
    pub reasoning_bridge: PsionActualPretrainingReasoningBridgeBinding,
    /// Plugin-conditioned agentic-SFT stage binding.
    pub agentic_stage: PsionActualPretrainingAgenticStageBinding,
    /// Repo-owned post-training reference binding.
    pub post_training_reference: PsionActualPretrainingPostTrainingReferenceBinding,
    /// Continuation-stage eval-pack binding.
    pub continuation_eval_pack: PsionActualPretrainingContinuationEvalPackBinding,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short explanation of the bundle.
    pub detail: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionActualPretrainingContinuationAlignmentBundle {
    /// Validates the alignment bundle against the current repo-owned upstream surfaces.
    pub fn validate_against_context(
        &self,
        handoff: &PsionActualPretrainingContinuationHandoff,
        reasoning_bundle: &PsionReasoningSftRunBundle,
        plugin_run_bundle: &PsionPluginConditionedSftRunBundle,
        continuation_eval_pack: &BenchmarkPackage,
        post_training_report: &AgenticSftRlReferenceProgramReport,
    ) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
        ensure_exact(
            self.schema_version.as_str(),
            "continuation_alignment.schema_version",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.bundle_id.as_str(),
            "continuation_alignment.bundle_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "continuation_alignment.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_exact(
            self.continuation_target_id.as_str(),
            "continuation_alignment.continuation_target_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID,
        )?;
        ensure_exact(
            self.accepted_checkpoint_run_id.as_str(),
            "continuation_alignment.accepted_checkpoint_run_id",
            handoff.run_id.as_str(),
        )?;
        ensure_exact(
            self.handoff_contract_path.as_str(),
            "continuation_alignment.handoff_contract_path",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
        )?;
        if self.actual_lane_stage_path != handoff.stage_path {
            return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
                field: String::from("continuation_alignment.actual_lane_stage_path"),
                expected: format!("{:?}", handoff.stage_path),
                actual: format!("{:?}", self.actual_lane_stage_path),
            });
        }
        let expected_post_training_path = vec![
            String::from("general_sft"),
            String::from("agentic_sft"),
            String::from("rl"),
        ];
        if self.post_training_reference_stage_path != expected_post_training_path {
            return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
                field: String::from("continuation_alignment.post_training_reference_stage_path"),
                expected: format!("{expected_post_training_path:?}"),
                actual: format!("{:?}", self.post_training_reference_stage_path),
            });
        }
        ensure_exact(
            self.accepted_checkpoint_ref.as_str(),
            "continuation_alignment.accepted_checkpoint_ref",
            handoff.accepted_checkpoint.checkpoint_ref.as_str(),
        )?;

        validate_artifact_ref_match(
            &self.reasoning_bridge.run_bundle,
            &handoff.reasoning_sft_run_bundle,
            "continuation_alignment.reasoning_bridge.run_bundle",
        )?;
        ensure_exact(
            self.reasoning_bridge.stage_receipt_id.as_str(),
            "continuation_alignment.reasoning_bridge.stage_receipt_id",
            reasoning_bundle.stage_receipt.receipt_id.as_str(),
        )?;
        ensure_exact(
            self.reasoning_bridge.stage_receipt_digest.as_str(),
            "continuation_alignment.reasoning_bridge.stage_receipt_digest",
            reasoning_bundle.stage_receipt.receipt_digest.as_str(),
        )?;
        ensure_exact(
            self.reasoning_bridge.evaluation_receipt_id.as_str(),
            "continuation_alignment.reasoning_bridge.evaluation_receipt_id",
            reasoning_bundle.evaluation_receipt.receipt_id.as_str(),
        )?;
        ensure_exact(
            self.reasoning_bridge.evaluation_receipt_digest.as_str(),
            "continuation_alignment.reasoning_bridge.evaluation_receipt_digest",
            reasoning_bundle.evaluation_receipt.receipt_digest.as_str(),
        )?;
        ensure_bps(
            self.reasoning_bridge.multiple_valid_style_pass_rate_bps,
            "continuation_alignment.reasoning_bridge.multiple_valid_style_pass_rate_bps",
        )?;
        ensure_exact_u32(
            self.reasoning_bridge.multiple_valid_style_pass_rate_bps,
            "continuation_alignment.reasoning_bridge.multiple_valid_style_pass_rate_bps",
            reasoning_bundle
                .evaluation_receipt
                .multiple_valid_style_pass_rate_bps,
        )?;
        ensure_nonempty(
            self.reasoning_bridge.detail.as_str(),
            "continuation_alignment.reasoning_bridge.detail",
        )?;

        validate_artifact_ref_match(
            &self.agentic_stage.run_bundle,
            &handoff.plugin_conditioned_run_bundle,
            "continuation_alignment.agentic_stage.run_bundle",
        )?;
        ensure_exact(
            self.agentic_stage.stage_receipt_id.as_str(),
            "continuation_alignment.agentic_stage.stage_receipt_id",
            plugin_run_bundle.stage_receipt.receipt_id.as_str(),
        )?;
        ensure_exact(
            self.agentic_stage.stage_receipt_digest.as_str(),
            "continuation_alignment.agentic_stage.stage_receipt_digest",
            plugin_run_bundle.stage_receipt.receipt_digest.as_str(),
        )?;
        ensure_exact(
            self.agentic_stage.general_sft_completion_digest.as_str(),
            "continuation_alignment.agentic_stage.general_sft_completion_digest",
            plugin_run_bundle
                .stage_receipt
                .general_sft_completion_digest
                .as_str(),
        )?;
        ensure_exact(
            self.agentic_stage.agentic_sft_completion_digest.as_str(),
            "continuation_alignment.agentic_stage.agentic_sft_completion_digest",
            plugin_run_bundle
                .stage_receipt
                .agentic_sft_completion_digest
                .as_str(),
        )?;
        ensure_exact_u32(
            self.agentic_stage.eval_hook_count,
            "continuation_alignment.agentic_stage.eval_hook_count",
            plugin_run_bundle.stage_receipt.eval_hook_count,
        )?;
        ensure_nonempty(
            self.agentic_stage.detail.as_str(),
            "continuation_alignment.agentic_stage.detail",
        )?;

        let general_checkpoint_ref = post_training_report
            .lineage
            .general_checkpoint_ref
            .as_deref()
            .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                field: String::from("post_training_report.lineage.general_checkpoint_ref"),
            })?;
        let agentic_checkpoint_ref = post_training_report
            .lineage
            .agentic_checkpoint_ref
            .as_deref()
            .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                field: String::from("post_training_report.lineage.agentic_checkpoint_ref"),
            })?;
        let target_policy_checkpoint_ref = post_training_report
            .lineage
            .target_policy_checkpoint_ref
            .as_deref()
            .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                field: String::from("post_training_report.lineage.target_policy_checkpoint_ref"),
            })?;
        ensure_exact(
            self.post_training_reference.reference_program_run_id.as_str(),
            "continuation_alignment.post_training_reference.reference_program_run_id",
            post_training_report.spec.run_id.as_str(),
        )?;
        if self.post_training_reference.current_stage_kind != TrainingStageKind::Rl {
            return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
                field: String::from("continuation_alignment.post_training_reference.current_stage_kind"),
                expected: String::from("Rl"),
                actual: format!("{:?}", self.post_training_reference.current_stage_kind),
            });
        }
        ensure_exact(
            self.post_training_reference.general_checkpoint_ref.as_str(),
            "continuation_alignment.post_training_reference.general_checkpoint_ref",
            general_checkpoint_ref,
        )?;
        ensure_exact(
            self.post_training_reference.agentic_checkpoint_ref.as_str(),
            "continuation_alignment.post_training_reference.agentic_checkpoint_ref",
            agentic_checkpoint_ref,
        )?;
        ensure_exact(
            self.post_training_reference.target_policy_revision_id.as_str(),
            "continuation_alignment.post_training_reference.target_policy_revision_id",
            post_training_report
                .lineage
                .target_policy_revision_id
                .as_str(),
        )?;
        ensure_exact(
            self.post_training_reference.target_policy_checkpoint_ref.as_str(),
            "continuation_alignment.post_training_reference.target_policy_checkpoint_ref",
            target_policy_checkpoint_ref,
        )?;
        ensure_exact(
            self.post_training_reference.policy_weight_broadcast_digest.as_str(),
            "continuation_alignment.post_training_reference.policy_weight_broadcast_digest",
            post_training_report
                .lineage
                .policy_weight_broadcast_digest
                .as_str(),
        )?;
        ensure_exact(
            self.post_training_reference.online_eval_run_id.as_str(),
            "continuation_alignment.post_training_reference.online_eval_run_id",
            post_training_report.lineage.online_eval_run_id.as_str(),
        )?;
        ensure_exact(
            self.post_training_reference.benchmark_eval_run_id.as_str(),
            "continuation_alignment.post_training_reference.benchmark_eval_run_id",
            post_training_report.lineage.benchmark_eval_run_id.as_str(),
        )?;
        ensure_exact_u32(
            self.post_training_reference.accepted_rollout_count,
            "continuation_alignment.post_training_reference.accepted_rollout_count",
            post_training_report.operator_view.accepted_rollout_count,
        )?;
        ensure_exact_u32(
            self.post_training_reference.validator_accepted_count,
            "continuation_alignment.post_training_reference.validator_accepted_count",
            post_training_report.operator_view.validator_accepted_count,
        )?;
        ensure_exact_u64(
            self.post_training_reference.completed_trainer_steps,
            "continuation_alignment.post_training_reference.completed_trainer_steps",
            post_training_report.operator_view.completed_trainer_steps,
        )?;
        ensure_nonempty(
            self.post_training_reference.detail.as_str(),
            "continuation_alignment.post_training_reference.detail",
        )?;

        validate_artifact_ref_match(
            &self.continuation_eval_pack.eval_pack_artifact,
            &handoff.continuation_eval_pack,
            "continuation_alignment.continuation_eval_pack.eval_pack_artifact",
        )?;
        ensure_exact(
            self.continuation_eval_pack.benchmark_ref.as_str(),
            "continuation_alignment.continuation_eval_pack.benchmark_ref",
            continuation_eval_pack.key.benchmark_ref.as_str(),
        )?;
        ensure_exact(
            self.continuation_eval_pack.benchmark_version.as_str(),
            "continuation_alignment.continuation_eval_pack.benchmark_version",
            continuation_eval_pack.key.version.as_str(),
        )?;
        ensure_exact(
            self.continuation_eval_pack.package_storage_key.as_str(),
            "continuation_alignment.continuation_eval_pack.package_storage_key",
            continuation_eval_pack.key.storage_key().as_str(),
        )?;
        let expected_case_ids = continuation_eval_pack
            .cases
            .iter()
            .map(|case| case.case_id.clone())
            .collect::<Vec<_>>();
        if self.continuation_eval_pack.case_ids != expected_case_ids {
            return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
                field: String::from("continuation_alignment.continuation_eval_pack.case_ids"),
                expected: format!("{expected_case_ids:?}"),
                actual: format!("{:?}", self.continuation_eval_pack.case_ids),
            });
        }
        ensure_nonempty(
            self.continuation_eval_pack.detail.as_str(),
            "continuation_alignment.continuation_eval_pack.detail",
        )?;

        ensure_nonempty(
            self.claim_boundary.as_str(),
            "continuation_alignment.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "continuation_alignment.detail")?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionActualPretrainingContinuationAlignmentError::DigestMismatch);
        }
        Ok(())
    }
}

/// Records the canonical continuation-alignment bundle from the current repo-owned surfaces.
pub fn record_psion_actual_pretraining_continuation_alignment_bundle(
    handoff: &PsionActualPretrainingContinuationHandoff,
    reasoning_bundle: &PsionReasoningSftRunBundle,
    plugin_run_bundle: &PsionPluginConditionedSftRunBundle,
    continuation_eval_pack: &BenchmarkPackage,
    post_training_report: &AgenticSftRlReferenceProgramReport,
) -> Result<PsionActualPretrainingContinuationAlignmentBundle, PsionActualPretrainingContinuationAlignmentError>
{
    let bundle = PsionActualPretrainingContinuationAlignmentBundle {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_SCHEMA_VERSION,
        ),
        bundle_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        continuation_target_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID),
        accepted_checkpoint_run_id: handoff.run_id.clone(),
        actual_lane_stage_path: handoff.stage_path.clone(),
        post_training_reference_stage_path: vec![
            String::from("general_sft"),
            String::from("agentic_sft"),
            String::from("rl"),
        ],
        accepted_checkpoint_ref: handoff.accepted_checkpoint.checkpoint_ref.clone(),
        handoff_contract_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        reasoning_bridge: PsionActualPretrainingReasoningBridgeBinding {
            run_bundle: handoff.reasoning_sft_run_bundle.clone(),
            stage_receipt_id: reasoning_bundle.stage_receipt.receipt_id.clone(),
            stage_receipt_digest: reasoning_bundle.stage_receipt.receipt_digest.clone(),
            evaluation_receipt_id: reasoning_bundle.evaluation_receipt.receipt_id.clone(),
            evaluation_receipt_digest: reasoning_bundle.evaluation_receipt.receipt_digest.clone(),
            multiple_valid_style_pass_rate_bps: reasoning_bundle
                .evaluation_receipt
                .multiple_valid_style_pass_rate_bps,
            detail: String::from(
                "Reasoning bridge keeps the accepted pretrain checkpoint tied to the bounded general_sft bundle, its stage receipt, and its style-plurality evaluation receipt.",
            ),
        },
        agentic_stage: PsionActualPretrainingAgenticStageBinding {
            run_bundle: handoff.plugin_conditioned_run_bundle.clone(),
            stage_receipt_id: plugin_run_bundle.stage_receipt.receipt_id.clone(),
            stage_receipt_digest: plugin_run_bundle.stage_receipt.receipt_digest.clone(),
            general_sft_completion_digest: plugin_run_bundle
                .stage_receipt
                .general_sft_completion_digest
                .clone(),
            agentic_sft_completion_digest: plugin_run_bundle
                .stage_receipt
                .agentic_sft_completion_digest
                .clone(),
            eval_hook_count: plugin_run_bundle.stage_receipt.eval_hook_count,
            detail: String::from(
                "Agentic-stage binding keeps the handoff target tied to the bounded plugin-conditioned agentic_sft receipt rather than a detached continuation curriculum.",
            ),
        },
        post_training_reference: PsionActualPretrainingPostTrainingReferenceBinding {
            reference_program_run_id: post_training_report.spec.run_id.clone(),
            current_stage_kind: post_training_report
                .stage_program
                .current_stage()
                .map(|stage| stage.kind)
                .unwrap_or(TrainingStageKind::Rl),
            general_checkpoint_ref: post_training_report
                .lineage
                .general_checkpoint_ref
                .clone()
                .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                    field: String::from("post_training_report.lineage.general_checkpoint_ref"),
                })?,
            agentic_checkpoint_ref: post_training_report
                .lineage
                .agentic_checkpoint_ref
                .clone()
                .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                    field: String::from("post_training_report.lineage.agentic_checkpoint_ref"),
                })?,
            target_policy_revision_id: post_training_report
                .lineage
                .target_policy_revision_id
                .clone(),
            target_policy_checkpoint_ref: post_training_report
                .lineage
                .target_policy_checkpoint_ref
                .clone()
                .ok_or_else(|| PsionActualPretrainingContinuationAlignmentError::MissingField {
                    field: String::from(
                        "post_training_report.lineage.target_policy_checkpoint_ref",
                    ),
                })?,
            policy_weight_broadcast_digest: post_training_report
                .lineage
                .policy_weight_broadcast_digest
                .clone(),
            online_eval_run_id: post_training_report.lineage.online_eval_run_id.clone(),
            benchmark_eval_run_id: post_training_report.lineage.benchmark_eval_run_id.clone(),
            accepted_rollout_count: post_training_report.operator_view.accepted_rollout_count,
            validator_accepted_count: post_training_report.operator_view.validator_accepted_count,
            completed_trainer_steps: post_training_report.operator_view.completed_trainer_steps,
            detail: String::from(
                "Post-training reference keeps the current repo-owned agentic_sft -> rl surface explicit through checkpoint lineage, policy revision, rollout counts, and retained eval ids.",
            ),
        },
        continuation_eval_pack: PsionActualPretrainingContinuationEvalPackBinding {
            eval_pack_artifact: handoff.continuation_eval_pack.clone(),
            benchmark_ref: continuation_eval_pack.key.benchmark_ref.clone(),
            benchmark_version: continuation_eval_pack.key.version.clone(),
            package_storage_key: continuation_eval_pack.key.storage_key(),
            case_ids: continuation_eval_pack
                .cases
                .iter()
                .map(|case| case.case_id.clone())
                .collect(),
            detail: String::from(
                "Continuation eval pack keeps reasoning-style, plugin result interpretation, rollout lineage, and post-training consistency review bound to one frozen review package.",
            ),
        },
        claim_boundary: String::from(
            "This alignment bundle ties the actual-lane accepted checkpoint handoff to the frozen reasoning `general_sft` bridge, the bounded plugin-conditioned `agentic_sft` stage, and the repo-owned `agentic_sft -> rl` reference surface. It does not claim plugin-conditioned RL execution, cluster-scale continuation, or promotion beyond the bounded continuation target.",
        ),
        detail: String::from(
            "Continuation alignment keeps the actual-lane handoff, bounded continuation artifacts, and current post-training reference receipts in one machine-legible bundle for later continuation rehearsal work.",
        ),
        bundle_digest: String::new(),
    };
    let mut bundle = bundle;
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_context(
        handoff,
        reasoning_bundle,
        plugin_run_bundle,
        continuation_eval_pack,
        post_training_report,
    )?;
    Ok(bundle)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingContinuationAlignmentError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` mismatch: expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("bundle digest mismatch")]
    DigestMismatch,
}

fn validate_artifact_ref_match(
    observed: &PsionActualPretrainingArtifactRef,
    expected: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    ensure_exact(
        observed.path.as_str(),
        &format!("{field_prefix}.path"),
        expected.path.as_str(),
    )?;
    ensure_exact(
        observed.sha256.as_str(),
        &format!("{field_prefix}.sha256"),
        expected.sha256.as_str(),
    )?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    if actual != expected {
        return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_exact_u32(
    actual: u32,
    field: &str,
    expected: u32,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    if actual != expected {
        return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
            field: String::from(field),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn ensure_exact_u64(
    actual: u64,
    field: &str,
    expected: u64,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    if actual != expected {
        return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
            field: String::from(field),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingContinuationAlignmentError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_bps(
    value: u32,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationAlignmentError> {
    if value > 10_000 {
        return Err(PsionActualPretrainingContinuationAlignmentError::FieldMismatch {
            field: String::from(field),
            expected: String::from("0..=10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn stable_bundle_digest(bundle: &PsionActualPretrainingContinuationAlignmentBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let bytes = serde_json::to_vec(&canonical)
        .expect("continuation alignment bundle should serialize for digest");
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_continuation_alignment_bundle|");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{run_agentic_sft_rl_reference_program, AgenticSftRlReferenceProgramSpec};
    use psionic_eval::build_psion_actual_pretraining_continuation_eval_benchmark_package;

    #[test]
    fn actual_pretraining_continuation_alignment_bundle_fixture_validates_against_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle: PsionActualPretrainingContinuationAlignmentBundle = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_alignment_bundle_v1.json"
            ),
        )?;
        let handoff: PsionActualPretrainingContinuationHandoff = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_v1.json"
            ),
        )?;
        let reasoning_bundle: PsionReasoningSftRunBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json"
        ))?;
        let plugin_run_bundle: PsionPluginConditionedSftRunBundle = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json"
            ),
        )?;
        let continuation_eval_pack =
            build_psion_actual_pretraining_continuation_eval_benchmark_package()?;
        let unique_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_millis(),
            Err(_) => 0,
        };
        let workspace_root = std::env::temp_dir().join(format!(
            "openagents-psionic-continuation-alignment-{}-{}",
            std::process::id(),
            unique_ms
        ));
        let report = run_agentic_sft_rl_reference_program(
            &AgenticSftRlReferenceProgramSpec::weather_default(workspace_root.clone()),
        )?;
        bundle.validate_against_context(
            &handoff,
            &reasoning_bundle,
            &plugin_run_bundle,
            &continuation_eval_pack,
            &report,
        )?;
        if workspace_root.exists() {
            std::fs::remove_dir_all(workspace_root)?;
        }
        Ok(())
    }

    #[test]
    fn continuation_alignment_bundle_requires_rl_post_training_stage() -> Result<(), Box<dyn std::error::Error>> {
        let mut bundle: PsionActualPretrainingContinuationAlignmentBundle = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_alignment_bundle_v1.json"
            ),
        )?;
        let handoff: PsionActualPretrainingContinuationHandoff = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_v1.json"
            ),
        )?;
        let reasoning_bundle: PsionReasoningSftRunBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json"
        ))?;
        let plugin_run_bundle: PsionPluginConditionedSftRunBundle = serde_json::from_str(
            include_str!(
                "../../../fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json"
            ),
        )?;
        let continuation_eval_pack =
            build_psion_actual_pretraining_continuation_eval_benchmark_package()?;
        let unique_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_millis(),
            Err(_) => 0,
        };
        let workspace_root = std::env::temp_dir().join(format!(
            "openagents-psionic-continuation-alignment-negative-{}-{}",
            std::process::id(),
            unique_ms
        ));
        let report = run_agentic_sft_rl_reference_program(
            &AgenticSftRlReferenceProgramSpec::weather_default(workspace_root.clone()),
        )?;
        bundle.post_training_reference.current_stage_kind = TrainingStageKind::AgenticSft;
        bundle.bundle_digest = stable_bundle_digest(&bundle);
        let error = bundle
            .validate_against_context(
                &handoff,
                &reasoning_bundle,
                &plugin_run_bundle,
                &continuation_eval_pack,
                &report,
            )
            .expect_err("continuation alignment bundle should reject non-RL reference stage");
        assert!(matches!(
            error,
            PsionActualPretrainingContinuationAlignmentError::FieldMismatch { field, .. }
            if field == "continuation_alignment.post_training_reference.current_stage_kind"
        ));
        if workspace_root.exists() {
            std::fs::remove_dir_all(workspace_root)?;
        }
        Ok(())
    }
}
