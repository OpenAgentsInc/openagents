use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingCloseoutArtifact,
    PsionActualPretrainingCloseoutBundle, PsionActualPretrainingCloseoutFailureDrill,
    PsionActualPretrainingCloseoutGate, PsionActualPretrainingContinuationAlignmentBundle,
    PsionActualPretrainingContinuationHandoff, PsionPluginConditionedSftStageManifest,
};

/// Stable schema version for the continuation-handoff rehearsal bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_continuation_handoff_rehearsal_bundle.v1";

/// Stable bundle identifier for the continuation-handoff rehearsal bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_ID: &str =
    "psion_actual_pretraining_continuation_handoff_rehearsal_bundle_v1";

/// Canonical committed fixture path for the continuation-handoff rehearsal bundle.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_FIXTURE_PATH: &str = "fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_rehearsal_bundle_v1.json";

/// Stable schema version for continuation-handoff refusal evidence.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_continuation_handoff_refusal_packet.v1";

/// Stable packet identifier for continuation-handoff refusal evidence.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_ID: &str =
    "psion_actual_pretraining_continuation_handoff_refusal_packet_v1";

/// Canonical committed fixture path for continuation-handoff refusal evidence.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_refusal_packet_v1.json";

const REQUIRED_ARTIFACT_KINDS: [&str; 7] = [
    "base_lane_closeout_bundle",
    "continuation_handoff",
    "continuation_alignment_bundle",
    "reasoning_sft_run_bundle",
    "plugin_conditioned_stage_manifest",
    "plugin_conditioned_run_bundle",
    "continuation_eval_pack",
];

const REQUIRED_GATE_IDS: [&str; 5] = [
    "base_lane_closeout_complete",
    "accepted_checkpoint_lineage_exact",
    "continuation_alignment_exact",
    "plugin_conditioned_stage_exact",
    "mismatched_alignment_refusal_retained",
];

const REQUIRED_FAILURE_DRILL_ID: &str = "mismatched_handoff_alignment_refusal";
const REQUIRED_REFUSAL_KIND: &str = "mismatched_handoff_alignment";
const REQUIRED_MISMATCH_FIELD: &str = "accepted_checkpoint_ref";

/// Exact lineage summary retained by the continuation-handoff rehearsal bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationLineageProof {
    /// Stable actual-lane closeout state.
    pub base_lane_closeout_state: String,
    /// Ordered actual-lane stage path.
    pub actual_lane_stage_path: Vec<String>,
    /// Exact accepted checkpoint ref consumed by the handoff.
    pub accepted_checkpoint_ref: String,
    /// Stable reasoning-bridge stage receipt id.
    pub reasoning_stage_receipt_id: String,
    /// Stable plugin-conditioned manifest run id.
    pub plugin_stage_manifest_run_id: String,
    /// Stable plugin-conditioned stage id.
    pub plugin_stage_id: String,
    /// Stable preceding general-SFT stage id.
    pub plugin_previous_stage_id: String,
    /// Stable plugin-conditioned stage receipt id.
    pub plugin_stage_receipt_id: String,
    /// Stable continuation-eval package storage key.
    pub continuation_eval_package_storage_key: String,
    /// Stable later reference-program run id retained for later review.
    pub later_reference_run_id: String,
    /// Short explanation of the lineage proof.
    pub detail: String,
}

/// Retained refusal evidence for a mismatched continuation handoff candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationHandoffRefusalPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable continuation target identifier.
    pub continuation_target_id: String,
    /// Exact actual-lane run id that was reviewed.
    pub run_id: String,
    /// Stable refusal kind.
    pub refusal_kind: String,
    /// Frozen handoff artifact under review.
    pub handoff_artifact: PsionActualPretrainingArtifactRef,
    /// Frozen mismatched candidate alignment artifact.
    pub candidate_alignment_artifact: PsionActualPretrainingArtifactRef,
    /// Exact field that mismatched.
    pub mismatch_field: String,
    /// Expected value from the admitted handoff.
    pub expected_value: String,
    /// Observed value in the refused candidate.
    pub observed_value: String,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the packet.
    pub packet_digest: String,
}

impl PsionActualPretrainingContinuationHandoffRefusalPacket {
    /// Validates the refusal packet against the handoff and candidate alignment.
    pub fn validate_against_context(
        &self,
        handoff: &PsionActualPretrainingContinuationHandoff,
        candidate_alignment: &PsionActualPretrainingContinuationAlignmentBundle,
    ) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "continuation_handoff_refusal.schema_version",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.packet_id.as_str(),
            "continuation_handoff_refusal.packet_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "continuation_handoff_refusal.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_exact(
            self.continuation_target_id.as_str(),
            "continuation_handoff_refusal.continuation_target_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID,
        )?;
        ensure_exact(
            self.run_id.as_str(),
            "continuation_handoff_refusal.run_id",
            handoff.run_id.as_str(),
        )?;
        ensure_exact(
            self.refusal_kind.as_str(),
            "continuation_handoff_refusal.refusal_kind",
            REQUIRED_REFUSAL_KIND,
        )?;
        ensure_artifact_ref(
            &self.handoff_artifact,
            "continuation_handoff_refusal.handoff_artifact",
        )?;
        ensure_artifact_ref(
            &self.candidate_alignment_artifact,
            "continuation_handoff_refusal.candidate_alignment_artifact",
        )?;
        ensure_exact(
            self.mismatch_field.as_str(),
            "continuation_handoff_refusal.mismatch_field",
            REQUIRED_MISMATCH_FIELD,
        )?;
        let expected_value = handoff.accepted_checkpoint.checkpoint_ref.as_str();
        ensure_exact(
            self.expected_value.as_str(),
            "continuation_handoff_refusal.expected_value",
            expected_value,
        )?;
        ensure_exact(
            self.observed_value.as_str(),
            "continuation_handoff_refusal.observed_value",
            candidate_alignment.accepted_checkpoint_ref.as_str(),
        )?;
        if self.expected_value == self.observed_value {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch {
                    field: String::from("continuation_handoff_refusal.observed_value"),
                    expected: String::from("value different from expected handoff checkpoint"),
                    actual: self.observed_value.clone(),
                },
            );
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "continuation_handoff_refusal.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "continuation_handoff_refusal.detail")?;
        if self.packet_digest != stable_refusal_packet_digest(self) {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::DigestMismatch {
                    field: String::from("continuation_handoff_refusal.packet_digest"),
                },
            );
        }
        Ok(())
    }
}

/// Canonical continuation-handoff rehearsal bundle for the actual pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinuationHandoffRehearsalBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable rehearsal bundle id.
    pub rehearsal_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable continuation target identifier.
    pub continuation_target_id: String,
    /// Exact actual-lane run id under review.
    pub run_id: String,
    /// Selected git ref inherited from the base-lane closeout proof.
    pub selected_git_ref: String,
    /// Exact git commit SHA inherited from the base-lane closeout proof.
    pub git_commit_sha: String,
    /// Dirty-tree posture inherited from the base-lane closeout proof.
    pub dirty_tree_admission: String,
    /// Optional workspace-status digest inherited from the base-lane closeout proof.
    pub workspace_status_sha256: Option<String>,
    /// Frozen base-lane closeout proof packet.
    pub base_lane_closeout_bundle: PsionActualPretrainingArtifactRef,
    /// Frozen accepted-checkpoint handoff artifact.
    pub continuation_handoff: PsionActualPretrainingArtifactRef,
    /// Frozen continuation-alignment artifact for the exact reviewed run.
    pub continuation_alignment_bundle: PsionActualPretrainingArtifactRef,
    /// Frozen canonical plugin-conditioned stage manifest artifact.
    pub plugin_conditioned_stage_manifest: PsionActualPretrainingArtifactRef,
    /// Exact lineage summary retained by the rehearsal.
    pub lineage_proof: PsionActualPretrainingContinuationLineageProof,
    /// Explicit retained artifacts cited by the rehearsal bundle.
    pub evidence_artifacts: Vec<PsionActualPretrainingCloseoutArtifact>,
    /// Explicit gates checked during rehearsal.
    pub closeout_gates: Vec<PsionActualPretrainingCloseoutGate>,
    /// Explicit refusal evidence carried by the rehearsal.
    pub failure_drills: Vec<PsionActualPretrainingCloseoutFailureDrill>,
    /// Things the operator can now honestly claim.
    pub can_now_claim: Vec<String>,
    /// Things that remain explicitly out of scope.
    pub still_out_of_scope: Vec<String>,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionActualPretrainingContinuationHandoffRehearsalBundle {
    /// Validates the rehearsal bundle against the reviewed continuation context.
    pub fn validate_against_context(
        &self,
        closeout_bundle: &PsionActualPretrainingCloseoutBundle,
        handoff: &PsionActualPretrainingContinuationHandoff,
        alignment_bundle: &PsionActualPretrainingContinuationAlignmentBundle,
        plugin_stage_manifest: &PsionPluginConditionedSftStageManifest,
        refusal_packet: &PsionActualPretrainingContinuationHandoffRefusalPacket,
    ) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "continuation_handoff_rehearsal.schema_version",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.rehearsal_id.as_str(),
            "continuation_handoff_rehearsal.rehearsal_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_ID,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "continuation_handoff_rehearsal.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_exact(
            self.continuation_target_id.as_str(),
            "continuation_handoff_rehearsal.continuation_target_id",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID,
        )?;
        ensure_exact(
            self.run_id.as_str(),
            "continuation_handoff_rehearsal.run_id",
            closeout_bundle.run_id.as_str(),
        )?;
        ensure_exact(
            self.run_id.as_str(),
            "continuation_handoff_rehearsal.run_id_vs_handoff",
            handoff.run_id.as_str(),
        )?;
        ensure_exact(
            self.run_id.as_str(),
            "continuation_handoff_rehearsal.run_id_vs_alignment",
            alignment_bundle.accepted_checkpoint_run_id.as_str(),
        )?;
        ensure_exact(
            self.selected_git_ref.as_str(),
            "continuation_handoff_rehearsal.selected_git_ref",
            closeout_bundle.selected_git_ref.as_str(),
        )?;
        ensure_exact(
            self.git_commit_sha.as_str(),
            "continuation_handoff_rehearsal.git_commit_sha",
            closeout_bundle.git_commit_sha.as_str(),
        )?;
        ensure_exact(
            self.dirty_tree_admission.as_str(),
            "continuation_handoff_rehearsal.dirty_tree_admission",
            closeout_bundle.dirty_tree_admission.as_str(),
        )?;
        ensure_optional_exact(
            self.workspace_status_sha256.as_ref(),
            "continuation_handoff_rehearsal.workspace_status_sha256",
            closeout_bundle.workspace_status_sha256.as_ref(),
        )?;
        ensure_artifact_ref(
            &self.base_lane_closeout_bundle,
            "continuation_handoff_rehearsal.base_lane_closeout_bundle",
        )?;
        ensure_artifact_ref(
            &self.continuation_handoff,
            "continuation_handoff_rehearsal.continuation_handoff",
        )?;
        ensure_artifact_ref(
            &self.continuation_alignment_bundle,
            "continuation_handoff_rehearsal.continuation_alignment_bundle",
        )?;
        ensure_artifact_ref(
            &self.plugin_conditioned_stage_manifest,
            "continuation_handoff_rehearsal.plugin_conditioned_stage_manifest",
        )?;
        validate_artifact_ref_match(
            &self.plugin_conditioned_stage_manifest,
            &handoff.plugin_conditioned_stage_manifest,
            "continuation_handoff_rehearsal.plugin_conditioned_stage_manifest",
        )?;

        closeout_bundle.validate().map_err(|error| {
            PsionActualPretrainingContinuationHandoffRehearsalError::Context {
                field: String::from("continuation_handoff_rehearsal.closeout_bundle"),
                message: error.to_string(),
            }
        })?;
        handoff.validate().map_err(|error| {
            PsionActualPretrainingContinuationHandoffRehearsalError::Context {
                field: String::from("continuation_handoff_rehearsal.handoff"),
                message: error.to_string(),
            }
        })?;
        ensure_exact(
            closeout_bundle.closeout_state.as_str(),
            "continuation_handoff_rehearsal.closeout_state",
            "base_lane_rehearsal_complete",
        )?;

        let closeout_handoff_artifact =
            find_closeout_artifact(closeout_bundle, "continuation_handoff")?;
        validate_artifact_ref_match(
            &self.continuation_handoff,
            &closeout_handoff_artifact.artifact,
            "continuation_handoff_rehearsal.continuation_handoff",
        )?;

        ensure_exact(
            self.lineage_proof.base_lane_closeout_state.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.base_lane_closeout_state",
            closeout_bundle.closeout_state.as_str(),
        )?;
        if self.lineage_proof.actual_lane_stage_path != handoff.stage_path {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch {
                    field: String::from(
                        "continuation_handoff_rehearsal.lineage_proof.actual_lane_stage_path",
                    ),
                    expected: format!("{:?}", handoff.stage_path),
                    actual: format!("{:?}", self.lineage_proof.actual_lane_stage_path),
                },
            );
        }
        ensure_exact(
            self.lineage_proof.accepted_checkpoint_ref.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.accepted_checkpoint_ref",
            handoff.accepted_checkpoint.checkpoint_ref.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.accepted_checkpoint_ref.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.accepted_checkpoint_ref_vs_alignment",
            alignment_bundle.accepted_checkpoint_ref.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.reasoning_stage_receipt_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.reasoning_stage_receipt_id",
            alignment_bundle.reasoning_bridge.stage_receipt_id.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.plugin_stage_manifest_run_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.plugin_stage_manifest_run_id",
            plugin_stage_manifest.run_id.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.plugin_stage_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.plugin_stage_id",
            plugin_stage_manifest.stage_id.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.plugin_previous_stage_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.plugin_previous_stage_id",
            plugin_stage_manifest.previous_stage_id.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.plugin_stage_receipt_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.plugin_stage_receipt_id",
            alignment_bundle.agentic_stage.stage_receipt_id.as_str(),
        )?;
        ensure_exact(
            self.lineage_proof
                .continuation_eval_package_storage_key
                .as_str(),
            "continuation_handoff_rehearsal.lineage_proof.continuation_eval_package_storage_key",
            alignment_bundle
                .continuation_eval_pack
                .package_storage_key
                .as_str(),
        )?;
        ensure_exact(
            self.lineage_proof.later_reference_run_id.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.later_reference_run_id",
            alignment_bundle
                .post_training_reference
                .reference_program_run_id
                .as_str(),
        )?;
        ensure_nonempty(
            self.lineage_proof.detail.as_str(),
            "continuation_handoff_rehearsal.lineage_proof.detail",
        )?;

        require_artifact_kinds(
            &self.evidence_artifacts,
            &REQUIRED_ARTIFACT_KINDS,
            "continuation_handoff_rehearsal.evidence_artifacts",
        )?;
        require_gate_ids(
            &self.closeout_gates,
            &REQUIRED_GATE_IDS,
            "continuation_handoff_rehearsal.closeout_gates",
        )?;
        let refusal_drill = require_failure_drill(
            &self.failure_drills,
            REQUIRED_FAILURE_DRILL_ID,
            "continuation_handoff_rehearsal.failure_drills",
        )?;
        ensure_exact(
            refusal_drill.resolution_state.as_str(),
            "continuation_handoff_rehearsal.failure_drill.resolution_state",
            "refused_as_expected",
        )?;
        ensure_exact(
            refusal_packet.run_id.as_str(),
            "continuation_handoff_rehearsal.refusal_packet.run_id",
            self.run_id.as_str(),
        )?;
        ensure_exact(
            refusal_packet.continuation_target_id.as_str(),
            "continuation_handoff_rehearsal.refusal_packet.continuation_target_id",
            self.continuation_target_id.as_str(),
        )?;
        ensure_exact(
            refusal_drill.artifact.path.as_str(),
            "continuation_handoff_rehearsal.failure_drill.artifact.path",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_FIXTURE_PATH,
        )?;
        ensure_nonempty(
            refusal_drill.artifact.sha256.as_str(),
            "continuation_handoff_rehearsal.failure_drill.artifact.sha256",
        )?;
        if self.can_now_claim.is_empty() {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                    field: String::from("continuation_handoff_rehearsal.can_now_claim"),
                },
            );
        }
        if self.still_out_of_scope.is_empty() {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                    field: String::from("continuation_handoff_rehearsal.still_out_of_scope"),
                },
            );
        }
        for claim in &self.can_now_claim {
            ensure_nonempty(
                claim.as_str(),
                "continuation_handoff_rehearsal.can_now_claim[]",
            )?;
        }
        for item in &self.still_out_of_scope {
            ensure_nonempty(
                item.as_str(),
                "continuation_handoff_rehearsal.still_out_of_scope[]",
            )?;
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "continuation_handoff_rehearsal.claim_boundary",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "continuation_handoff_rehearsal.detail",
        )?;
        if self.bundle_digest != stable_rehearsal_bundle_digest(self) {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::DigestMismatch {
                    field: String::from("continuation_handoff_rehearsal.bundle_digest"),
                },
            );
        }
        Ok(())
    }
}

/// Records one mismatched-handoff refusal packet.
pub fn record_psion_actual_pretraining_continuation_handoff_refusal_packet(
    handoff_artifact: PsionActualPretrainingArtifactRef,
    handoff: &PsionActualPretrainingContinuationHandoff,
    candidate_alignment_artifact: PsionActualPretrainingArtifactRef,
    candidate_alignment: &PsionActualPretrainingContinuationAlignmentBundle,
) -> Result<
    PsionActualPretrainingContinuationHandoffRefusalPacket,
    PsionActualPretrainingContinuationHandoffRehearsalError,
> {
    let mut packet = PsionActualPretrainingContinuationHandoffRefusalPacket {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_SCHEMA_VERSION,
        ),
        packet_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        continuation_target_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID),
        run_id: handoff.run_id.clone(),
        refusal_kind: String::from(REQUIRED_REFUSAL_KIND),
        handoff_artifact,
        candidate_alignment_artifact,
        mismatch_field: String::from(REQUIRED_MISMATCH_FIELD),
        expected_value: handoff.accepted_checkpoint.checkpoint_ref.clone(),
        observed_value: candidate_alignment.accepted_checkpoint_ref.clone(),
        claim_boundary: String::from(
            "This refusal packet proves that the continuation-handoff rehearsal rejects a mismatched candidate alignment bundle before any bounded plugin-conditioned continuation claim becomes reviewable.",
        ),
        detail: String::from(
            "The refused candidate changed the accepted checkpoint ref away from the admitted actual-lane handoff while keeping the same continuation target family.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_refusal_packet_digest(&packet);
    packet.validate_against_context(handoff, candidate_alignment)?;
    Ok(packet)
}

/// Records the canonical continuation-handoff rehearsal bundle.
pub fn record_psion_actual_pretraining_continuation_handoff_rehearsal_bundle(
    base_lane_closeout_bundle: PsionActualPretrainingArtifactRef,
    closeout_bundle: &PsionActualPretrainingCloseoutBundle,
    continuation_handoff: PsionActualPretrainingArtifactRef,
    handoff: &PsionActualPretrainingContinuationHandoff,
    continuation_alignment_bundle: PsionActualPretrainingArtifactRef,
    alignment_bundle: &PsionActualPretrainingContinuationAlignmentBundle,
    plugin_conditioned_stage_manifest: PsionActualPretrainingArtifactRef,
    plugin_stage_manifest: &PsionPluginConditionedSftStageManifest,
    refusal_packet_artifact: PsionActualPretrainingArtifactRef,
    refusal_packet: &PsionActualPretrainingContinuationHandoffRefusalPacket,
) -> Result<
    PsionActualPretrainingContinuationHandoffRehearsalBundle,
    PsionActualPretrainingContinuationHandoffRehearsalError,
> {
    let mut bundle = PsionActualPretrainingContinuationHandoffRehearsalBundle {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_SCHEMA_VERSION,
        ),
        rehearsal_id: String::from(
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_ID,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        continuation_target_id: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_TARGET_ID),
        run_id: handoff.run_id.clone(),
        selected_git_ref: closeout_bundle.selected_git_ref.clone(),
        git_commit_sha: closeout_bundle.git_commit_sha.clone(),
        dirty_tree_admission: closeout_bundle.dirty_tree_admission.clone(),
        workspace_status_sha256: closeout_bundle.workspace_status_sha256.clone(),
        base_lane_closeout_bundle: base_lane_closeout_bundle.clone(),
        continuation_handoff: continuation_handoff.clone(),
        continuation_alignment_bundle: continuation_alignment_bundle.clone(),
        plugin_conditioned_stage_manifest: plugin_conditioned_stage_manifest.clone(),
        lineage_proof: PsionActualPretrainingContinuationLineageProof {
            base_lane_closeout_state: closeout_bundle.closeout_state.clone(),
            actual_lane_stage_path: handoff.stage_path.clone(),
            accepted_checkpoint_ref: handoff.accepted_checkpoint.checkpoint_ref.clone(),
            reasoning_stage_receipt_id: alignment_bundle.reasoning_bridge.stage_receipt_id.clone(),
            plugin_stage_manifest_run_id: plugin_stage_manifest.run_id.clone(),
            plugin_stage_id: plugin_stage_manifest.stage_id.clone(),
            plugin_previous_stage_id: plugin_stage_manifest.previous_stage_id.clone(),
            plugin_stage_receipt_id: alignment_bundle.agentic_stage.stage_receipt_id.clone(),
            continuation_eval_package_storage_key: alignment_bundle
                .continuation_eval_pack
                .package_storage_key
                .clone(),
            later_reference_run_id: alignment_bundle
                .post_training_reference
                .reference_program_run_id
                .clone(),
            detail: String::from(
                "Lineage proof ties the exact base-lane closeout, accepted checkpoint, plugin-conditioned stage ids, continuation eval pack, and later reference-program run into one bounded review surface.",
            ),
        },
        evidence_artifacts: vec![
            artifact(
                "base_lane_closeout_bundle",
                base_lane_closeout_bundle,
                "Base-lane closeout proves the accepted pretrain checkpoint came from the retained actual-lane rehearsal.",
            ),
            artifact(
                "continuation_handoff",
                continuation_handoff,
                "Continuation handoff binds the accepted checkpoint to the frozen `pretrain -> general_sft -> agentic_sft` target.",
            ),
            artifact(
                "continuation_alignment_bundle",
                continuation_alignment_bundle,
                "Continuation alignment for the exact reviewed run keeps the reasoning bridge, plugin-conditioned stage, and later reference surface together.",
            ),
            artifact(
                "reasoning_sft_run_bundle",
                handoff.reasoning_sft_run_bundle.clone(),
                "Reasoning-SFT bundle keeps the `general_sft` bridge explicit in the rehearsal evidence family.",
            ),
            artifact(
                "plugin_conditioned_stage_manifest",
                plugin_conditioned_stage_manifest,
                "Canonical plugin-conditioned stage manifest keeps the bounded `general_sft -> agentic_sft` lane explicit.",
            ),
            artifact(
                "plugin_conditioned_run_bundle",
                handoff.plugin_conditioned_run_bundle.clone(),
                "Plugin-conditioned run bundle keeps the retained stage receipt and completion digests explicit.",
            ),
            artifact(
                "continuation_eval_pack",
                handoff.continuation_eval_pack.clone(),
                "Continuation eval pack keeps bounded reasoning, plugin interpretation, rollout lineage, and post-training consistency review explicit.",
            ),
        ],
        closeout_gates: vec![
            gate(
                "base_lane_closeout_complete",
                true,
                "Base-lane closeout reached `base_lane_rehearsal_complete` before continuation review consumed the handoff.",
            ),
            gate(
                "accepted_checkpoint_lineage_exact",
                true,
                "Accepted checkpoint ref matches across the base-lane closeout, handoff, and continuation alignment bundle.",
            ),
            gate(
                "continuation_alignment_exact",
                true,
                "Continuation alignment bundle matches the exact reviewed run id, continuation target id, and accepted checkpoint lineage.",
            ),
            gate(
                "plugin_conditioned_stage_exact",
                true,
                "Plugin-conditioned stage manifest matches the frozen handoff target and exact stage ids reviewed by the rehearsal.",
            ),
            gate(
                "mismatched_alignment_refusal_retained",
                true,
                "One mismatched alignment candidate produced retained refusal evidence instead of silently widening the continuation claim.",
            ),
        ],
        failure_drills: vec![PsionActualPretrainingCloseoutFailureDrill {
            drill_id: String::from(REQUIRED_FAILURE_DRILL_ID),
            resolution_state: String::from("refused_as_expected"),
            artifact: refusal_packet_artifact,
            detail: String::from(
                "Rehearsal retained a mismatched accepted-checkpoint alignment candidate and its refusal packet before the bounded continuation proof was marked green.",
            ),
        }],
        can_now_claim: vec![
            String::from(
                "The actual-lane accepted checkpoint now has one separately retained continuation-handoff rehearsal packet that preserves exact lineage into the frozen plugin-conditioned `general_sft -> agentic_sft` lane.",
            ),
            String::from(
                "The continuation proof now cites one exact base-lane closeout bundle, one exact accepted-checkpoint handoff, and one exact continuation alignment bundle without redefining the base lane.",
            ),
            String::from(
                "A mismatched alignment candidate now produces retained refusal evidence instead of silently passing continuation review.",
            ),
        ],
        still_out_of_scope: vec![
            String::from("cluster-scale plugin-conditioned continuation execution"),
            String::from("plugin-conditioned RL execution on the actual pretraining lane"),
            String::from(
                "promotion beyond the bounded `general_sft -> agentic_sft` continuation target",
            ),
        ],
        claim_boundary: String::from(
            "This bundle proves one bounded continuation-handoff rehearsal above the accepted actual pretraining checkpoint. It keeps exact lineage into the frozen plugin-conditioned `general_sft -> agentic_sft` lane plus one retained mismatch refusal packet. It does not claim cluster-scale continuation execution, plugin-conditioned RL execution, or promotion beyond the bounded continuation target.",
        ),
        detail: String::from(
            "Continuation-handoff rehearsal keeps the base-lane proof gate separate while still proving that the accepted checkpoint maps into the canonical plugin-conditioned continuation lane with retained refusal evidence.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_rehearsal_bundle_digest(&bundle);
    bundle.validate_against_context(
        closeout_bundle,
        handoff,
        alignment_bundle,
        plugin_stage_manifest,
        refusal_packet,
    )?;
    Ok(bundle)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingContinuationHandoffRehearsalError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` mismatch: expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("invalid context for `{field}`: {message}")]
    Context { field: String, message: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
}

fn artifact(
    artifact_kind: &str,
    artifact: PsionActualPretrainingArtifactRef,
    detail: &str,
) -> PsionActualPretrainingCloseoutArtifact {
    PsionActualPretrainingCloseoutArtifact {
        artifact_kind: String::from(artifact_kind),
        artifact,
        detail: String::from(detail),
    }
}

fn gate(gate_id: &str, satisfied: bool, detail: &str) -> PsionActualPretrainingCloseoutGate {
    PsionActualPretrainingCloseoutGate {
        gate_id: String::from(gate_id),
        satisfied,
        detail: String::from(detail),
    }
}

fn find_closeout_artifact<'a>(
    closeout_bundle: &'a PsionActualPretrainingCloseoutBundle,
    artifact_kind: &str,
) -> Result<
    &'a PsionActualPretrainingCloseoutArtifact,
    PsionActualPretrainingContinuationHandoffRehearsalError,
> {
    closeout_bundle
        .evidence_artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == artifact_kind)
        .ok_or_else(
            || PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: format!("closeout_bundle.evidence_artifacts[{artifact_kind}]"),
            },
        )
}

fn require_artifact_kinds(
    artifacts: &[PsionActualPretrainingCloseoutArtifact],
    required: &[&str],
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    if artifacts.is_empty() {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: String::from(field),
            },
        );
    }
    for artifact in artifacts {
        ensure_nonempty(artifact.artifact_kind.as_str(), field)?;
        ensure_artifact_ref(&artifact.artifact, field)?;
        ensure_nonempty(artifact.detail.as_str(), field)?;
    }
    for required_kind in required {
        if !artifacts
            .iter()
            .any(|artifact| artifact.artifact_kind == *required_kind)
        {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                    field: format!("{field}[{required_kind}]"),
                },
            );
        }
    }
    Ok(())
}

fn require_gate_ids(
    gates: &[PsionActualPretrainingCloseoutGate],
    required: &[&str],
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    if gates.is_empty() {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: String::from(field),
            },
        );
    }
    for gate in gates {
        ensure_nonempty(gate.gate_id.as_str(), field)?;
        ensure_nonempty(gate.detail.as_str(), field)?;
        if !gate.satisfied {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch {
                    field: format!("{field}[{}].satisfied", gate.gate_id),
                    expected: String::from("true"),
                    actual: String::from("false"),
                },
            );
        }
    }
    for required_gate in required {
        if !gates.iter().any(|gate| gate.gate_id == *required_gate) {
            return Err(
                PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                    field: format!("{field}[{required_gate}]"),
                },
            );
        }
    }
    Ok(())
}

fn require_failure_drill<'a>(
    drills: &'a [PsionActualPretrainingCloseoutFailureDrill],
    drill_id: &str,
    field: &str,
) -> Result<
    &'a PsionActualPretrainingCloseoutFailureDrill,
    PsionActualPretrainingContinuationHandoffRehearsalError,
> {
    if drills.is_empty() {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: String::from(field),
            },
        );
    }
    for drill in drills {
        ensure_nonempty(drill.drill_id.as_str(), field)?;
        ensure_nonempty(drill.resolution_state.as_str(), field)?;
        ensure_artifact_ref(&drill.artifact, field)?;
        ensure_nonempty(drill.detail.as_str(), field)?;
    }
    drills
        .iter()
        .find(|drill| drill.drill_id == drill_id)
        .ok_or_else(
            || PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: format!("{field}[{drill_id}]"),
            },
        )
}

fn validate_artifact_ref_match(
    observed: &PsionActualPretrainingArtifactRef,
    expected: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
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

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field}.sha256"))?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    if actual != expected {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch {
                field: String::from(field),
                expected: String::from(expected),
                actual: String::from(actual),
            },
        );
    }
    Ok(())
}

fn ensure_optional_exact(
    actual: Option<&String>,
    field: &str,
    expected: Option<&String>,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    if actual != expected {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch {
                field: String::from(field),
                expected: format!("{expected:?}"),
                actual: format!("{actual:?}"),
            },
        );
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingContinuationHandoffRehearsalError> {
    if value.trim().is_empty() {
        return Err(
            PsionActualPretrainingContinuationHandoffRehearsalError::MissingField {
                field: String::from(field),
            },
        );
    }
    Ok(())
}

fn stable_refusal_packet_digest(
    packet: &PsionActualPretrainingContinuationHandoffRefusalPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    let bytes = serde_json::to_vec(&canonical)
        .expect("continuation handoff refusal packet should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_continuation_handoff_refusal_packet|");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn stable_rehearsal_bundle_digest(
    bundle: &PsionActualPretrainingContinuationHandoffRehearsalBundle,
) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let bytes = serde_json::to_vec(&canonical)
        .expect("continuation handoff rehearsal bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_continuation_handoff_rehearsal_bundle|");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closeout_bundle() -> PsionActualPretrainingCloseoutBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_closeout_bundle_v1.json"
        ))
        .expect("closeout bundle fixture should parse")
    }

    fn handoff() -> PsionActualPretrainingContinuationHandoff {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_base_lane_rehearsal_example/run-psion-actual-20260402t160000z/continuation/accepted_checkpoint_handoff.json"
        ))
        .expect("base-lane handoff fixture should parse")
    }

    fn alignment_bundle() -> PsionActualPretrainingContinuationAlignmentBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_rehearsal_example/run-psion-actual-20260402t160000z/continuation/continuation_alignment_bundle.json"
        ))
        .expect("continuation rehearsal alignment bundle should parse")
    }

    fn candidate_alignment_bundle() -> PsionActualPretrainingContinuationAlignmentBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_rehearsal_example/run-psion-actual-20260402t160000z/continuation/failures/mismatched_alignment_candidate.json"
        ))
        .expect("continuation rehearsal mismatched candidate should parse")
    }

    fn plugin_stage_manifest() -> PsionPluginConditionedSftStageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json"
        ))
        .expect("plugin stage manifest fixture should parse")
    }

    fn refusal_packet() -> PsionActualPretrainingContinuationHandoffRefusalPacket {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_refusal_packet_v1.json"
        ))
        .expect("continuation handoff refusal packet should parse")
    }

    fn rehearsal_bundle() -> PsionActualPretrainingContinuationHandoffRehearsalBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_rehearsal_bundle_v1.json"
        ))
        .expect("continuation handoff rehearsal bundle should parse")
    }

    #[test]
    fn continuation_handoff_refusal_packet_fixture_validates_against_context() {
        refusal_packet()
            .validate_against_context(&handoff(), &candidate_alignment_bundle())
            .expect("continuation handoff refusal packet should validate");
    }

    #[test]
    fn continuation_handoff_rehearsal_bundle_fixture_validates_against_context() {
        rehearsal_bundle()
            .validate_against_context(
                &closeout_bundle(),
                &handoff(),
                &alignment_bundle(),
                &plugin_stage_manifest(),
                &refusal_packet(),
            )
            .expect("continuation handoff rehearsal bundle should validate");
    }

    #[test]
    fn continuation_handoff_rehearsal_bundle_requires_exact_checkpoint_lineage() {
        let mut bundle = rehearsal_bundle();
        bundle.lineage_proof.accepted_checkpoint_ref =
            String::from("checkpoint://psion/actual-pretraining/unrelated");
        bundle.bundle_digest = stable_rehearsal_bundle_digest(&bundle);
        let error = bundle
            .validate_against_context(
                &closeout_bundle(),
                &handoff(),
                &alignment_bundle(),
                &plugin_stage_manifest(),
                &refusal_packet(),
            )
            .expect_err("rehearsal bundle should reject mismatched accepted checkpoint lineage");
        assert!(matches!(
            error,
            PsionActualPretrainingContinuationHandoffRehearsalError::FieldMismatch { field, .. }
            if field == "continuation_handoff_rehearsal.lineage_proof.accepted_checkpoint_ref"
        ));
    }
}
