use std::{error::Error, fs, path::PathBuf};

use psionic_data::{
    PsionArtifactLineageManifest, PsionLifecycleChangeTrigger, PsionSourceLifecycleManifest,
    PsionSourceLifecycleState, PsionSourceRightsPosture,
};
use psionic_serve::{
    PsionCapabilityArtifactReference, PsionCapabilityFollowOnAnalysis,
    PsionCapabilityFollowOnAnalysisKind, PsionCapabilityMatrix, PsionCapabilityMatrixChange,
    PsionCapabilityMatrixChangeKind, PsionCapabilityPosture, PsionCapabilityRefusalReason,
    PsionCapabilityRegionDowngrade, PsionCapabilityRegionId, PsionCapabilityWithdrawalReceipt,
    PsionCapabilityWithdrawalTrigger, PsionCheckpointRollbackDisposition,
    PsionCheckpointRollbackPlan, PsionContaminationRollbackEvidence,
    PsionRefusalRegressionRollbackEvidence, PsionReplayFailureRollbackEvidence,
    PsionRightsChangeRollbackEvidence, PsionRouteRegressionRollbackEvidence,
    PsionServedClaimChange, PsionServedClaimChangeKind, PsionServedClaimFlag,
    PsionServedClaimPostureSummary, PsionServedOutputClaimPosture,
    record_psion_capability_withdrawal_receipt, stable_capability_matrix_digest,
    stable_replay_verification_digest, stable_route_calibration_digest,
};
use psionic_train::{
    PsionAcceptanceMatrix, PsionPromotionDecisionReceipt, PsionRefusalCalibrationReceipt,
    PsionRouteCalibrationReceipt, PsionRouteClassEvaluationReceipt, ReplayVerificationDisposition,
    ReplayVerificationSignalKind, TrainingReplayVerificationReceipt,
    TrainingReplayVerificationSignal,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_dir = repo_root.join("fixtures/psion/withdrawal");
    fs::create_dir_all(&fixtures_dir)?;

    let capability_matrix: PsionCapabilityMatrix =
        load_json(repo_root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"))?;
    let acceptance_matrix: PsionAcceptanceMatrix =
        load_json(repo_root.join("fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"))?;
    let promotion_decision: PsionPromotionDecisionReceipt = load_json(
        repo_root.join("fixtures/psion/acceptance/psion_promotion_decision_receipt_v1.json"),
    )?;
    let lifecycle: PsionSourceLifecycleManifest = load_json(
        repo_root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"),
    )?;
    let lineage: PsionArtifactLineageManifest = load_json(
        repo_root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
    )?;
    let direct_claim: PsionServedOutputClaimPosture =
        load_json(repo_root.join("fixtures/psion/serve/psion_served_output_claim_direct_v1.json"))?;
    let executor_claim: PsionServedOutputClaimPosture = load_json(
        repo_root.join("fixtures/psion/serve/psion_served_output_claim_executor_backed_v1.json"),
    )?;
    let refusal_claim: PsionServedOutputClaimPosture = load_json(
        repo_root.join("fixtures/psion/serve/psion_served_output_claim_refusal_v1.json"),
    )?;
    let route_class_eval: PsionRouteClassEvaluationReceipt = load_json(
        repo_root.join("fixtures/psion/route/psion_route_class_evaluation_receipt_v1.json"),
    )?;
    let baseline_refusal: PsionRefusalCalibrationReceipt = load_json(
        repo_root.join("fixtures/psion/refusal/psion_refusal_calibration_receipt_v1.json"),
    )?;
    let lifecycle_manifest_bytes = fs::read(
        repo_root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"),
    )?;

    let rights = rights_change_receipt(
        &capability_matrix,
        &acceptance_matrix,
        &promotion_decision,
        &lifecycle,
        &lineage,
        &direct_claim,
        &lifecycle_manifest_bytes,
    )?;
    let contamination = contamination_receipt(
        &capability_matrix,
        &acceptance_matrix,
        &promotion_decision,
        &lifecycle,
        &lineage,
        &direct_claim,
        &lifecycle_manifest_bytes,
    )?;
    let replay = replay_failure_receipt(
        &capability_matrix,
        &acceptance_matrix,
        &promotion_decision,
        &direct_claim,
    )?;
    let route = route_regression_receipt(
        &capability_matrix,
        &acceptance_matrix,
        &promotion_decision,
        &executor_claim,
        &refusal_claim,
        &route_class_eval,
    )?;
    let refusal = refusal_regression_receipt(
        &capability_matrix,
        &acceptance_matrix,
        &promotion_decision,
        &executor_claim,
        &refusal_claim,
        &baseline_refusal,
    )?;

    write_receipt(
        fixtures_dir.join("psion_capability_withdrawal_rights_change_v1.json"),
        &rights,
    )?;
    write_receipt(
        fixtures_dir.join("psion_capability_withdrawal_contamination_v1.json"),
        &contamination,
    )?;
    write_receipt(
        fixtures_dir.join("psion_capability_withdrawal_replay_failure_v1.json"),
        &replay,
    )?;
    write_receipt(
        fixtures_dir.join("psion_capability_withdrawal_route_regression_v1.json"),
        &route,
    )?;
    write_receipt(
        fixtures_dir.join("psion_capability_withdrawal_refusal_regression_v1.json"),
        &refusal,
    )?;
    Ok(())
}

fn rights_change_receipt(
    capability_matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    promotion_decision: &PsionPromotionDecisionReceipt,
    lifecycle: &PsionSourceLifecycleManifest,
    lineage: &PsionArtifactLineageManifest,
    direct_claim: &PsionServedOutputClaimPosture,
    lifecycle_manifest_bytes: &[u8],
) -> Result<PsionCapabilityWithdrawalReceipt, Box<dyn Error>> {
    let impact = lineage.build_impact_analysis(
        lifecycle,
        "wasm_core_spec_release_2",
        PsionSourceLifecycleState::EvaluationOnly,
        PsionSourceRightsPosture::EvaluationOnly,
        PsionLifecycleChangeTrigger::RightsChangedOrRevoked,
    )?;
    let trigger_artifact = artifact_ref_from_json("psion-source-impact-rights-change-v1", &impact)?;
    let corrected_manifest = artifact_ref_from_bytes(
        "fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json#wasm_core_spec_release_2.evaluation_only",
        lifecycle_manifest_bytes,
    );
    let matrix_history = withdrawn_matrix_history(capability_matrix, "change.rights_change.v1");
    let served_claim_history = vec![PsionServedClaimChange {
        surface_id: String::from("psion.direct_generation"),
        kind: PsionServedClaimChangeKind::Depublish,
        previous_posture: PsionServedClaimPostureSummary::from_posture(direct_claim),
        replacement_posture: None,
        removed_visible_claims: vec![
            PsionServedClaimFlag::LearnedJudgment,
            PsionServedClaimFlag::SourceGrounding,
            PsionServedClaimFlag::BenchmarkBacking,
        ],
        detail: String::from(
            "Rights narrowing removes the current direct learned and source-grounded claim surface until a corrected publication exists.",
        ),
    }];
    let follow_on_analyses = vec![
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.rights_manifest_correction.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::SourceManifestCorrection,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(corrected_manifest.clone()),
            detail: String::from(
                "The lifecycle manifest must record the narrowed rights posture before republishing any claim surface.",
            ),
        },
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.rights_retraining.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-rights-retraining-analysis-v1",
                &serde_json::json!({
                    "source_id": impact.source_id.clone(),
                    "affected_checkpoints": impact.affected_checkpoint_ids.clone(),
                    "reason": "rights_narrowing"
                }),
            )?),
            detail: String::from(
                "Affected checkpoints need bounded retraining or bounded replacement analysis because the source still appears in training lineage.",
            ),
        },
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.rights_depublication.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::DepublicationAnalysis,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-rights-depublication-analysis-v1",
                &serde_json::json!({
                    "matrix_id": capability_matrix.matrix_id,
                    "claim_surface": "psion.direct_generation",
                    "reason": "rights_narrowing"
                }),
            )?),
            detail: String::from(
                "Published direct and source-grounded claims stay withdrawn until the narrowed rights posture is fully reflected in serving docs and artifacts.",
            ),
        },
    ];
    Ok(record_psion_capability_withdrawal_receipt(
        "psion-capability-withdrawal-rights-change-v1",
        capability_matrix,
        acceptance_matrix,
        promotion_decision,
        PsionCheckpointRollbackPlan {
            published_checkpoint_id: promotion_decision.checkpoint_receipt.checkpoint_id.clone(),
            disposition: PsionCheckpointRollbackDisposition::WithdrawServedCheckpoint,
            rollback_target_checkpoint_id: None,
            detail: String::from(
                "The served checkpoint is withdrawn because the published source rights narrowed after promotion.",
            ),
        },
        PsionCapabilityWithdrawalTrigger::RightsChange(PsionRightsChangeRollbackEvidence {
            source_impact_analysis: impact,
            corrected_source_manifest: corrected_manifest,
            detail: String::from(
                "A previously admitted normative source moved to evaluation-only posture, so the current served publication can no longer stay live.",
            ),
        }),
        matrix_history,
        served_claim_history,
        follow_on_analyses,
        "Rights-change rollback withdraws the current served checkpoint, preserves explicit matrix and claim history, and ties republishing to manifest correction plus bounded retraining and depublication analysis.",
    )?)
}

fn contamination_receipt(
    capability_matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    promotion_decision: &PsionPromotionDecisionReceipt,
    lifecycle: &PsionSourceLifecycleManifest,
    lineage: &PsionArtifactLineageManifest,
    direct_claim: &PsionServedOutputClaimPosture,
    lifecycle_manifest_bytes: &[u8],
) -> Result<PsionCapabilityWithdrawalReceipt, Box<dyn Error>> {
    let impact = lineage.build_impact_analysis(
        lifecycle,
        "arch_textbook_foster_1985",
        PsionSourceLifecycleState::Withdrawn,
        PsionSourceRightsPosture::Rejected,
        PsionLifecycleChangeTrigger::BenchmarkContaminationDiscovered,
    )?;
    let trigger_artifact = artifact_ref_from_json("psion-source-impact-contamination-v1", &impact)?;
    let corrected_manifest = artifact_ref_from_bytes(
        "fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json#arch_textbook_foster_1985.withdrawn",
        lifecycle_manifest_bytes,
    );
    let served_claim_history = vec![PsionServedClaimChange {
        surface_id: String::from("psion.direct_generation"),
        kind: PsionServedClaimChangeKind::Depublish,
        previous_posture: PsionServedClaimPostureSummary::from_posture(direct_claim),
        replacement_posture: None,
        removed_visible_claims: vec![
            PsionServedClaimFlag::LearnedJudgment,
            PsionServedClaimFlag::SourceGrounding,
            PsionServedClaimFlag::BenchmarkBacking,
        ],
        detail: String::from(
            "Contamination invalidates the current direct learned and benchmark-backed publication until the affected lineage is replaced.",
        ),
    }];
    let follow_on_analyses = vec![
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.contamination_manifest_correction.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::SourceManifestCorrection,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(corrected_manifest.clone()),
            detail: String::from(
                "The lifecycle and source manifests must show the contaminated source as withdrawn before new publications are honest.",
            ),
        },
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.contamination_retraining.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-contamination-retraining-analysis-v1",
                &serde_json::json!({
                    "source_id": impact.source_id.clone(),
                    "affected_checkpoints": impact.affected_checkpoint_ids.clone(),
                    "affected_benchmarks": impact.affected_benchmark_artifact_ids.clone(),
                    "reason": "contamination"
                }),
            )?),
            detail: String::from(
                "The contaminated source forces bounded retraining or bounded checkpoint replacement analysis across every affected lineage row.",
            ),
        },
        PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.contamination_depublication.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::DepublicationAnalysis,
            triggered_by: trigger_artifact.clone(),
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-contamination-depublication-analysis-v1",
                &serde_json::json!({
                    "matrix_id": capability_matrix.matrix_id,
                    "claim_surface": "psion.direct_generation",
                    "reason": "contamination"
                }),
            )?),
            detail: String::from(
                "Earlier publications remain withdrawn until contamination review, benchmark invalidation, and replacement artifacts are recorded.",
            ),
        },
    ];
    Ok(record_psion_capability_withdrawal_receipt(
        "psion-capability-withdrawal-contamination-v1",
        capability_matrix,
        acceptance_matrix,
        promotion_decision,
        PsionCheckpointRollbackPlan {
            published_checkpoint_id: promotion_decision.checkpoint_receipt.checkpoint_id.clone(),
            disposition: PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint,
            rollback_target_checkpoint_id: Some(String::from("pilot-pretrain-clean-baseline")),
            detail: String::from(
                "Contamination forces an explicit rollback to the last known clean checkpoint target instead of leaving the served checkpoint live.",
            ),
        },
        PsionCapabilityWithdrawalTrigger::ContaminationDiscovered(
            PsionContaminationRollbackEvidence {
                source_impact_analysis: impact,
                corrected_source_manifest: corrected_manifest,
                detail: String::from(
                    "Benchmark contamination invalidated a source that still appears in checkpoint and benchmark lineage.",
                ),
            },
        ),
        withdrawn_matrix_history(capability_matrix, "change.contamination.v1"),
        served_claim_history,
        follow_on_analyses,
        "Contamination rollback preserves withdrawal history, rolls the served checkpoint back to a clean target, and ties recovery to manifest correction plus bounded retraining and depublication analysis.",
    )?)
}

fn replay_failure_receipt(
    capability_matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    promotion_decision: &PsionPromotionDecisionReceipt,
    direct_claim: &PsionServedOutputClaimPosture,
) -> Result<PsionCapabilityWithdrawalReceipt, Box<dyn Error>> {
    let replay_verification = TrainingReplayVerificationReceipt {
        disposition: ReplayVerificationDisposition::Drifted,
        expected_replay_digest: String::from(
            "532e70275ad6d01616decae818df0f5b8012bc91316f3329c223c125874d1733",
        ),
        observed_replay_digest: String::from(
            "6a3f3009e0e689d3cc75a8a3156b9aa2c63a9220a64978da4d6fb5722f6f6e4d",
        ),
        signals: vec![
            TrainingReplayVerificationSignal {
                kind: ReplayVerificationSignalKind::TrainerBatchDigest,
                subject: String::from("trainer_batch"),
                expected: String::from("pilot_pretrain_batch_digest_v1"),
                observed: String::from("pilot_pretrain_batch_digest_v2"),
            },
            TrainingReplayVerificationSignal {
                kind: ReplayVerificationSignalKind::EvalRunDigest,
                subject: String::from("eval"),
                expected: String::from("pilot_eval_run_digest_v1"),
                observed: String::from("pilot_eval_run_digest_v2"),
            },
        ],
    };
    let replay_artifact =
        artifact_ref_from_json("psion-replay-verification-drift-v1", &replay_verification)?;
    Ok(record_psion_capability_withdrawal_receipt(
        "psion-capability-withdrawal-replay-failure-v1",
        capability_matrix,
        acceptance_matrix,
        promotion_decision,
        PsionCheckpointRollbackPlan {
            published_checkpoint_id: promotion_decision.checkpoint_receipt.checkpoint_id.clone(),
            disposition: PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint,
            rollback_target_checkpoint_id: Some(String::from("pilot-pretrain-last-known-good")),
            detail: String::from(
                "Replay drift rolls serving back to the last known replay-clean checkpoint target.",
            ),
        },
        PsionCapabilityWithdrawalTrigger::ReplayFailure(PsionReplayFailureRollbackEvidence {
            baseline_replay_receipt_ref: promotion_decision.replay_receipt.receipt_id.clone(),
            replay_verification_receipt_id: String::from("psion-replay-verification-drift-v1"),
            replay_verification_digest: stable_replay_verification_digest(&replay_verification),
            replay_verification,
            detail: String::from(
                "Deterministic replay drifted across batch identity and eval-run identity, so the promoted checkpoint can no longer stay published.",
            ),
        }),
        withdrawn_matrix_history(capability_matrix, "change.replay_failure.v1"),
        vec![PsionServedClaimChange {
            surface_id: String::from("psion.direct_generation"),
            kind: PsionServedClaimChangeKind::Depublish,
            previous_posture: PsionServedClaimPostureSummary::from_posture(direct_claim),
            replacement_posture: None,
            removed_visible_claims: vec![
                PsionServedClaimFlag::LearnedJudgment,
                PsionServedClaimFlag::SourceGrounding,
                PsionServedClaimFlag::BenchmarkBacking,
            ],
            detail: String::from(
                "Replay drift removes the current direct claim surface until the checkpoint is replay-clean again.",
            ),
        }],
        vec![PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.replay_retraining.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
            triggered_by: replay_artifact,
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-replay-retraining-analysis-v1",
                &serde_json::json!({
                    "checkpoint_id": promotion_decision.checkpoint_receipt.checkpoint_id,
                    "reason": "replay_failure"
                }),
            )?),
            detail: String::from(
                "Replay failure triggers bounded retraining or bounded checkpoint replacement analysis before republishing the lane.",
            ),
        }],
        "Replay-failure rollback withdraws the current matrix publication, depublishes the direct claim surface, and rolls serving back to a replay-clean checkpoint target.",
    )?)
}

fn route_regression_receipt(
    capability_matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    promotion_decision: &PsionPromotionDecisionReceipt,
    executor_claim: &PsionServedOutputClaimPosture,
    refusal_claim: &PsionServedOutputClaimPosture,
    route_class_eval: &PsionRouteClassEvaluationReceipt,
) -> Result<PsionCapabilityWithdrawalReceipt, Box<dyn Error>> {
    let route_receipt = PsionRouteCalibrationReceipt {
        receipt_id: String::from("psion-route-calibration-regressed-v1"),
        covered_routes: vec![
            psionic_train::PsionRouteKind::DirectModelAnswer,
            psionic_train::PsionRouteKind::ExactExecutorHandoff,
            psionic_train::PsionRouteKind::Refusal,
        ],
        route_selection_accuracy_bps: 9310,
        route_regression_bps: 420,
        summary: String::from(
            "Observed route calibration drifted below the pilot route-accuracy floor and above the pilot regression ceiling.",
        ),
    };
    let route_artifact =
        artifact_ref_from_json("psion-route-calibration-regressed-v1", &route_receipt)?;
    Ok(record_psion_capability_withdrawal_receipt(
        "psion-capability-withdrawal-route-regression-v1",
        capability_matrix,
        acceptance_matrix,
        promotion_decision,
        PsionCheckpointRollbackPlan {
            published_checkpoint_id: promotion_decision.checkpoint_receipt.checkpoint_id.clone(),
            disposition: PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint,
            rollback_target_checkpoint_id: Some(String::from("pilot-pretrain-route-stable")),
            detail: String::from(
                "Route regression rolls serving back to a checkpoint that still held the exactness route inside the accepted band.",
            ),
        },
        PsionCapabilityWithdrawalTrigger::RouteRegression(PsionRouteRegressionRollbackEvidence {
            baseline_route_calibration_receipt_ref: promotion_decision
                .route_calibration_receipt
                .receipt_id
                .clone(),
            observed_route_calibration_digest: stable_route_calibration_digest(&route_receipt),
            observed_route_calibration_receipt: route_receipt,
            observed_route_class_evaluation_artifact: PsionCapabilityArtifactReference {
                artifact_id: route_class_eval.receipt_id.clone(),
                artifact_digest: route_class_eval.receipt_digest.clone(),
            },
            detail: String::from(
                "Exact-executor delegation drifted outside the accepted band, so the route-required exactness region is withdrawn from the current served endpoint.",
            ),
        }),
        vec![PsionCapabilityMatrixChange {
            change_id: String::from("change.route_regression.v1"),
            kind: PsionCapabilityMatrixChangeKind::DowngradeRegionPosture,
            matrix_id: capability_matrix.matrix_id.clone(),
            matrix_version: capability_matrix.matrix_version.clone(),
            matrix_digest: stable_capability_matrix_digest(capability_matrix),
            downgraded_regions: vec![PsionCapabilityRegionDowngrade {
                region_id: PsionCapabilityRegionId::VerifiedOrExactExecutionRequests,
                previous_posture: PsionCapabilityPosture::RouteRequired,
                next_posture: PsionCapabilityPosture::Unsupported,
                next_refusal_reasons: vec![
                    PsionCapabilityRefusalReason::UnsupportedExactnessRequest,
                ],
                detail: String::from(
                    "Exactness requests are no longer publishable as route-required on this endpoint while route regression remains outside the accepted band.",
                ),
            }],
            detail: String::from(
                "The current capability matrix explicitly withdraws the exactness route instead of silently leaving the stale row in place.",
            ),
        }],
        vec![PsionServedClaimChange {
            surface_id: String::from("psion.exact_executor_surface"),
            kind: PsionServedClaimChangeKind::ChangeBehaviorVisibility,
            previous_posture: PsionServedClaimPostureSummary::from_posture(executor_claim),
            replacement_posture: Some(PsionServedClaimPostureSummary::from_posture(refusal_claim)),
            removed_visible_claims: vec![PsionServedClaimFlag::ExecutorBacking],
            detail: String::from(
                "The served exactness surface now shows a typed refusal instead of a stale executor-backed route.",
            ),
        }],
        vec![PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.route_retraining.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
            triggered_by: route_artifact,
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-route-retraining-analysis-v1",
                &serde_json::json!({
                    "reason": "route_regression",
                    "checkpoint_id": promotion_decision.checkpoint_receipt.checkpoint_id
                }),
            )?),
            detail: String::from(
                "Route regression requires bounded retraining or bounded route recalibration analysis before the exactness route can be republished.",
            ),
        }],
        "Route-regression rollback preserves the exactness-route downgrade in the matrix, changes the served surface from executor-backed route to typed refusal, and rolls serving back to a route-stable checkpoint target.",
    )?)
}

fn refusal_regression_receipt(
    capability_matrix: &PsionCapabilityMatrix,
    acceptance_matrix: &PsionAcceptanceMatrix,
    promotion_decision: &PsionPromotionDecisionReceipt,
    executor_claim: &PsionServedOutputClaimPosture,
    refusal_claim: &PsionServedOutputClaimPosture,
    baseline_refusal: &PsionRefusalCalibrationReceipt,
) -> Result<PsionCapabilityWithdrawalReceipt, Box<dyn Error>> {
    let mut refusal_receipt = baseline_refusal.clone();
    refusal_receipt.receipt_id = String::from("psion-refusal-calibration-regressed-v1");
    refusal_receipt.aggregate_unsupported_request_refusal_bps = 9580;
    refusal_receipt.aggregate_reason_code_match_bps = 9700;
    refusal_receipt.supported_control_overrefusal_bps = 1700;
    refusal_receipt.refusal_regression_bps = 340;
    refusal_receipt.summary = String::from(
        "Observed refusal calibration drifted below the pilot refusal floor and above both over-refusal and regression ceilings.",
    );
    let refusal_artifact = PsionCapabilityArtifactReference {
        artifact_id: refusal_receipt.receipt_id.clone(),
        artifact_digest: refusal_receipt.receipt_digest.clone(),
    };
    Ok(record_psion_capability_withdrawal_receipt(
        "psion-capability-withdrawal-refusal-regression-v1",
        capability_matrix,
        acceptance_matrix,
        promotion_decision,
        PsionCheckpointRollbackPlan {
            published_checkpoint_id: promotion_decision.checkpoint_receipt.checkpoint_id.clone(),
            disposition: PsionCheckpointRollbackDisposition::RollbackToPriorCheckpoint,
            rollback_target_checkpoint_id: Some(String::from("pilot-pretrain-refusal-stable")),
            detail: String::from(
                "Refusal regression rolls serving back to a checkpoint that still held unsupported-request refusal inside the accepted band.",
            ),
        },
        PsionCapabilityWithdrawalTrigger::RefusalRegression(
            PsionRefusalRegressionRollbackEvidence {
                baseline_refusal_calibration_receipt_ref: promotion_decision
                    .refusal_calibration_receipt
                    .receipt_id
                    .clone(),
                observed_refusal_calibration_receipt: refusal_receipt,
                detail: String::from(
                    "Unsupported-request refusal drifted outside the accepted pilot band, so the current exactness route is clamped to explicit refusal until recalibration succeeds.",
                ),
            },
        ),
        vec![PsionCapabilityMatrixChange {
            change_id: String::from("change.refusal_regression.v1"),
            kind: PsionCapabilityMatrixChangeKind::DowngradeRegionPosture,
            matrix_id: capability_matrix.matrix_id.clone(),
            matrix_version: capability_matrix.matrix_version.clone(),
            matrix_digest: stable_capability_matrix_digest(capability_matrix),
            downgraded_regions: vec![PsionCapabilityRegionDowngrade {
                region_id: PsionCapabilityRegionId::VerifiedOrExactExecutionRequests,
                previous_posture: PsionCapabilityPosture::RouteRequired,
                next_posture: PsionCapabilityPosture::RefusalRequired,
                next_refusal_reasons: vec![
                    PsionCapabilityRefusalReason::UnsupportedExactnessRequest,
                ],
                detail: String::from(
                    "The exactness route is temporarily clamped to explicit refusal while refusal regression remains outside the accepted band.",
                ),
            }],
            detail: String::from(
                "The current capability matrix now shows exactness as refusal-required instead of a still-servable route.",
            ),
        }],
        vec![PsionServedClaimChange {
            surface_id: String::from("psion.exact_executor_surface"),
            kind: PsionServedClaimChangeKind::ChangeBehaviorVisibility,
            previous_posture: PsionServedClaimPostureSummary::from_posture(executor_claim),
            replacement_posture: Some(PsionServedClaimPostureSummary::from_posture(refusal_claim)),
            removed_visible_claims: vec![PsionServedClaimFlag::ExecutorBacking],
            detail: String::from(
                "The served exactness surface now refuses explicitly rather than keeping an executor-backed claim live during refusal regression.",
            ),
        }],
        vec![PsionCapabilityFollowOnAnalysis {
            analysis_id: String::from("analysis.refusal_retraining.v1"),
            kind: PsionCapabilityFollowOnAnalysisKind::BoundedRetrainingAnalysis,
            triggered_by: refusal_artifact,
            planned_analysis_artifact: Some(artifact_ref_from_json(
                "psion-refusal-retraining-analysis-v1",
                &serde_json::json!({
                    "reason": "refusal_regression",
                    "checkpoint_id": promotion_decision.checkpoint_receipt.checkpoint_id
                }),
            )?),
            detail: String::from(
                "Refusal regression requires bounded retraining or bounded recalibration analysis before republishing exactness routing.",
            ),
        }],
        "Refusal-regression rollback preserves the exactness-route clamp to explicit refusal, changes the served surface from executor-backed route to typed refusal, and rolls serving back to a refusal-stable checkpoint target.",
    )?)
}

fn withdrawn_matrix_history(
    capability_matrix: &PsionCapabilityMatrix,
    change_id: &str,
) -> Vec<PsionCapabilityMatrixChange> {
    vec![PsionCapabilityMatrixChange {
        change_id: String::from(change_id),
        kind: PsionCapabilityMatrixChangeKind::WithdrawPublication,
        matrix_id: capability_matrix.matrix_id.clone(),
        matrix_version: capability_matrix.matrix_version.clone(),
        matrix_digest: stable_capability_matrix_digest(capability_matrix),
        downgraded_regions: Vec::new(),
        detail: String::from(
            "The current published matrix is withdrawn explicitly instead of being silently deleted or overwritten.",
        ),
    }]
}

fn artifact_ref_from_json<T: Serialize>(
    artifact_id: &str,
    value: &T,
) -> Result<PsionCapabilityArtifactReference, Box<dyn Error>> {
    Ok(PsionCapabilityArtifactReference {
        artifact_id: String::from(artifact_id),
        artifact_digest: stable_digest(&serde_json::to_vec(value)?),
    })
}

fn artifact_ref_from_bytes(artifact_id: &str, bytes: &[u8]) -> PsionCapabilityArtifactReference {
    PsionCapabilityArtifactReference {
        artifact_id: String::from(artifact_id),
        artifact_digest: stable_digest(bytes),
    }
}

fn stable_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_capability_withdrawal_fixture|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn load_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_receipt(
    path: PathBuf,
    receipt: &PsionCapabilityWithdrawalReceipt,
) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string_pretty(receipt)?)?;
    Ok(())
}
