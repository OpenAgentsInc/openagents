use std::{error::Error, fs, path::PathBuf};

use psionic_data::PsionArtifactLineageManifest;
use psionic_train::{
    record_psion_pilot_held_out_loss, record_psion_pilot_pretraining_run,
    record_psion_pilot_route_probe, record_psion_refusal_calibration_receipt,
    PsionAcceptanceMatrix, PsionBenchmarkCatalog, PsionBenchmarkEvidenceReceipt,
    PsionBenchmarkFamily, PsionCapabilityMatrixView, PsionCheckpointRecoveryReceipt,
    PsionContaminationReviewDisposition, PsionContaminationReviewReceipt, PsionMetricKind,
    PsionObservedMetric, PsionPhaseGate, PsionPilotHeldOutLossFamily,
    PsionPilotHeldOutLossRow, PsionPilotRouteProbeKind, PsionPilotRouteProbeRow,
    PsionPretrainRunObservabilityReceipt, PsionPretrainStageRunReceipt,
    PsionPromotionDecisionDisposition, PsionPromotionDecisionReceipt,
    PsionRefusalCalibrationRow, PsionReplayEvidenceReceipt, PsionRouteCalibrationReceipt,
    PsionRouteKind,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pilot");
    fs::create_dir_all(&fixtures_dir)?;

    let acceptance_matrix: PsionAcceptanceMatrix = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"),
    )?)?;
    let stage_receipt: PsionPretrainStageRunReceipt = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"),
    )?)?;
    let observability_receipt: PsionPretrainRunObservabilityReceipt =
        serde_json::from_str(&fs::read_to_string(root.join(
            "fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json",
        ))?)?;
    let benchmark_catalog: PsionBenchmarkCatalog = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"),
    )?)?;
    let capability_matrix: PsionCapabilityMatrixView = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"),
    )?)?;
    let artifact_lineage: PsionArtifactLineageManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
        )?)?;

    let held_out_loss_receipt = record_psion_pilot_held_out_loss(
        "psion-pilot-held-out-loss-v1",
        vec![
            PsionPilotHeldOutLossRow {
                family: PsionPilotHeldOutLossFamily::Textbooks,
                seed_baseline_loss_milli: 1410,
                pilot_loss_milli: 1120,
                improvement_over_seed_baseline_bps: 2060,
                detail: String::from(
                    "Pilot textbooks slice improved enough to justify keeping the prose-heavy lane in pilot scope.",
                ),
            },
            PsionPilotHeldOutLossRow {
                family: PsionPilotHeldOutLossFamily::NormativeSpecs,
                seed_baseline_loss_milli: 1520,
                pilot_loss_milli: 1260,
                improvement_over_seed_baseline_bps: 1710,
                detail: String::from(
                    "Pilot specification slice improved while preserving the spec-heavy held-out boundary.",
                ),
            },
            PsionPilotHeldOutLossRow {
                family: PsionPilotHeldOutLossFamily::TechnicalDocs,
                seed_baseline_loss_milli: 1480,
                pilot_loss_milli: 1185,
                improvement_over_seed_baseline_bps: 1990,
                detail: String::from(
                    "Pilot technical-doc slice improved enough to keep the docs lane in the acceptance story.",
                ),
            },
        ],
        "Held-out loss improved across textbooks, specs, and technical docs versus the seed baseline.",
        &stage_receipt,
    )?;

    let route_probe_receipt = record_psion_pilot_route_probe(
        "psion-pilot-route-probe-v1",
        vec![
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::SupportedArchitectureExplanation,
                expected_route: PsionRouteKind::DirectModelAnswer,
                observed_route_accuracy_bps: 9750,
                detail: String::from(
                    "Supported architecture explanations stayed on the direct answer route at pilot time.",
                ),
            },
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::ExactExecutionRequest,
                expected_route: PsionRouteKind::ExactExecutorHandoff,
                observed_route_accuracy_bps: 9860,
                detail: String::from(
                    "Exact execution asks were handed off to the exact executor rather than improvised by the learned lane.",
                ),
            },
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::UnderspecifiedDesignTask,
                expected_route: PsionRouteKind::Refusal,
                observed_route_accuracy_bps: 9920,
                detail: String::from(
                    "Underspecified design tasks triggered refusal rather than false precision.",
                ),
            },
        ],
        "Pilot route probes covered direct answer, exact-executor handoff, and refusal on the bounded route surface.",
        &stage_receipt,
    )?;

    let refusal_package = benchmark_catalog
        .packages
        .iter()
        .find(|package| package.package_id == "psion_unsupported_request_refusal_benchmark_v1")
        .ok_or("refusal package missing from benchmark catalog")?;
    let refusal_calibration_receipt = record_psion_refusal_calibration_receipt(
        "psion-pilot-refusal-calibration-v1",
        refusal_package,
        &capability_matrix,
        vec![
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-exactness"),
                capability_region_id: String::from(
                    "unsupported_exact_execution_without_executor_surface",
                ),
                expected_reason_code: String::from("unsupported_exactness_request"),
                observed_refusal_accuracy_bps: 9950,
                reason_code_match_bps: 10000,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/exactness-without-executor",
                ),
                detail: String::from(
                    "Exactness refusal row shows the lane refuses exact-execution asks that do not expose a verifier or executor surface.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-missing-constraints"),
                capability_region_id: String::from(
                    "underspecified_design_without_required_constraints",
                ),
                expected_reason_code: String::from("missing_required_constraints"),
                observed_refusal_accuracy_bps: 9890,
                reason_code_match_bps: 9940,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/missing-required-constraints",
                ),
                detail: String::from(
                    "Missing-constraints row shows the lane refuses underspecified design asks instead of inventing requirements.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-over-context"),
                capability_region_id: String::from("over_context_envelope_requests"),
                expected_reason_code: String::from("unsupported_context_length"),
                observed_refusal_accuracy_bps: 9940,
                reason_code_match_bps: 9980,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/over-context-envelope",
                ),
                detail: String::from(
                    "Over-context row shows prompts beyond the hard context boundary refuse with the declared reason instead of truncating silently.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-freshness"),
                capability_region_id: String::from(
                    "freshness_or_run_artifact_dependent_requests",
                ),
                expected_reason_code: String::from(
                    "currentness_or_run_artifact_dependency",
                ),
                observed_refusal_accuracy_bps: 9910,
                reason_code_match_bps: 9950,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/currentness-or-hidden-artifact",
                ),
                detail: String::from(
                    "Freshness row shows the lane refuses mutable-state asks instead of claiming hidden run-artifact visibility.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-open-ended"),
                capability_region_id: String::from("open_ended_general_assistant_chat"),
                expected_reason_code: String::from(
                    "open_ended_general_assistant_unsupported",
                ),
                observed_refusal_accuracy_bps: 9910,
                reason_code_match_bps: 9930,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/open-ended-assistant",
                ),
                detail: String::from(
                    "Open-ended row shows the lane keeps generic assistant chat explicitly unsupported instead of drifting into vague half-service.",
                ),
            },
        ],
        900,
        60,
        "Canonical refusal-calibration receipt proving unsupported exactness, missing constraints, context overflow, freshness, and open-ended assistant asks stay bound to named capability-matrix regions and refusal reasons.",
        &artifact_lineage,
    )?;

    let promotion_decision = PsionPromotionDecisionReceipt {
        schema_version: String::from(psionic_train::PSION_PROMOTION_DECISION_SCHEMA_VERSION),
        decision_id: String::from("psion-pilot-promotion-decision-v1"),
        matrix_id: acceptance_matrix.matrix_id.clone(),
        matrix_version: acceptance_matrix.matrix_version.clone(),
        phase: PsionPhaseGate::Pilot,
        candidate_artifact_id: stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint_label
            .clone(),
        benchmark_receipts: vec![
            PsionBenchmarkEvidenceReceipt {
                receipt_id: String::from("psion-pilot-architecture-reasoning-receipt-v1"),
                phase: PsionPhaseGate::Pilot,
                family: PsionBenchmarkFamily::ArchitectureReasoning,
                benchmark_artifact_id: String::from("psion_architecture_reasoning_benchmark_v1"),
                benchmark_artifact_digest: String::from(
                    "703142f680a8a2702700fcdd18309b24d6e7889e07c4621e287178c9ac6af674",
                ),
                metrics: vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8450,
                    regression_from_baseline_bps: 0,
                }],
                summary: String::from(
                    "Pilot candidate cleared the bounded architecture reasoning floor.",
                ),
            },
            PsionBenchmarkEvidenceReceipt {
                receipt_id: String::from("psion-pilot-held-out-reasoning-receipt-v1"),
                phase: PsionPhaseGate::Pilot,
                family: PsionBenchmarkFamily::HeldOutTechnicalReasoning,
                benchmark_artifact_id: String::from("psion_held_out_reasoning_benchmark_v1"),
                benchmark_artifact_digest: String::from(
                    "sha256:psion_held_out_reasoning_benchmark_v1",
                ),
                metrics: vec![
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::PassRateBps,
                        observed_bps: 7900,
                        regression_from_baseline_bps: 0,
                    },
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::ImprovementOverSeedBaselineBps,
                        observed_bps: held_out_loss_receipt.minimum_improvement_bps(),
                        regression_from_baseline_bps: 0,
                    },
                ],
                summary: String::from(
                    "Pilot candidate improved over the seed baseline on the held-out technical reasoning set.",
                ),
            },
            PsionBenchmarkEvidenceReceipt {
                receipt_id: String::from("psion-pilot-specification-boundary-receipt-v1"),
                phase: PsionPhaseGate::Pilot,
                family: PsionBenchmarkFamily::NormativeSpecReading,
                benchmark_artifact_id: String::from("psion_normative_spec_benchmark_v1"),
                benchmark_artifact_digest: String::from(
                    "dd5741c92863fe4525d67eadd513b955b01f97adb9aa0f1cfa22619e8b273b32",
                ),
                metrics: vec![PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: 8820,
                    regression_from_baseline_bps: 0,
                }],
                summary: String::from(
                    "Pilot candidate kept normative spec reading separate from implementation commentary.",
                ),
            },
            PsionBenchmarkEvidenceReceipt {
                receipt_id: String::from("psion-pilot-unsupported-request-refusal-receipt-v1"),
                phase: PsionPhaseGate::Pilot,
                family: PsionBenchmarkFamily::UnsupportedRequestRefusal,
                benchmark_artifact_id: refusal_calibration_receipt.package_id.clone(),
                benchmark_artifact_digest: refusal_calibration_receipt.package_digest.clone(),
                metrics: vec![
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                        observed_bps: refusal_calibration_receipt
                            .aggregate_unsupported_request_refusal_bps,
                        regression_from_baseline_bps: 0,
                    },
                    PsionObservedMetric {
                        metric_kind: PsionMetricKind::OverrefusalBps,
                        observed_bps: refusal_calibration_receipt
                            .supported_control_overrefusal_bps,
                        regression_from_baseline_bps: 0,
                    },
                ],
                summary: String::from(
                    "Pilot candidate refused unsupported requests while keeping over-refusal within band.",
                ),
            },
        ],
        replay_receipt: PsionReplayEvidenceReceipt {
            receipt_id: String::from("psion-pilot-replay-evidence-v1"),
            successful_replays: stage_receipt.replay_receipt.successful_replays,
            exact_replay_observed: stage_receipt.replay_receipt.exact_replay_observed,
            summary: String::from(
                "Pilot replay evidence matched the bound pretrain-stage replay receipt exactly.",
            ),
        },
        checkpoint_receipt: PsionCheckpointRecoveryReceipt {
            receipt_id: String::from("psion-pilot-checkpoint-recovery-v1"),
            checkpoint_id: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            successful_restart_roundtrips: 1,
            restart_recovery_observed: true,
            resume_regression_bps: 220,
            summary: String::from(
                "Pilot checkpoint resumed once within the declared regression budget.",
            ),
        },
        contamination_review_receipt: PsionContaminationReviewReceipt {
            receipt_id: String::from("psion-pilot-contamination-review-v1"),
            benchmark_isolation_schema_version: String::from("psion.benchmark_isolation.v1"),
            exclusion_manifest_digest: String::from("sha256:psion_exclusion_manifest_v1"),
            near_duplicate_review_completed: true,
            tokenizer_exposure_review_completed: true,
            disposition: PsionContaminationReviewDisposition::Clean,
            applied_consequences: Vec::new(),
            summary: String::from(
                "Pilot held-out review stayed clean against the frozen exclusion manifest.",
            ),
        },
        route_calibration_receipt: PsionRouteCalibrationReceipt {
            receipt_id: String::from("psion-pilot-route-calibration-v1"),
            covered_routes: vec![
                PsionRouteKind::DirectModelAnswer,
                PsionRouteKind::ExactExecutorHandoff,
                PsionRouteKind::Refusal,
            ],
            route_selection_accuracy_bps: route_probe_receipt.aggregate_route_selection_accuracy_bps,
            route_regression_bps: 120,
            summary: String::from(
                "Pilot route calibration covered direct answer, exact-executor handoff, and refusal.",
            ),
        },
        refusal_calibration_receipt,
        decision: PsionPromotionDecisionDisposition::Promoted,
        hold_reason_codes: Vec::new(),
        decision_summary: String::from(
            "Pilot candidate cleared the pilot gate on architecture reasoning, held-out improvement, checkpoint recovery, replay, route calibration, refusal calibration, and contamination review.",
        ),
    };

    let bundle = record_psion_pilot_pretraining_run(
        "psion-pilot-pretraining-run-v1",
        held_out_loss_receipt.clone(),
        route_probe_receipt.clone(),
        promotion_decision,
        "Pilot pretraining bundle binds the first bounded pretrain run, held-out loss deltas, route probes, and acceptance-matrix promotion decision into one reviewable artifact.",
        stage_receipt,
        observability_receipt,
        &acceptance_matrix,
    )?;

    fs::write(
        fixtures_dir.join("psion_pilot_held_out_loss_receipt_v1.json"),
        serde_json::to_string_pretty(&held_out_loss_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_pilot_route_probe_receipt_v1.json"),
        serde_json::to_string_pretty(&route_probe_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_pilot_pretraining_run_bundle_v1.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
