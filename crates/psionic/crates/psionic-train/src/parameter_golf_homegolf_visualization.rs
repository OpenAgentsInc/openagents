use std::{collections::BTreeMap, fs, path::Path};

use thiserror::Error;

use crate::{
    build_parameter_golf_homegolf_artifact_accounting_report,
    build_parameter_golf_homegolf_clustered_run_surface_report,
    build_parameter_golf_homegolf_competitive_ablation_report,
    build_parameter_golf_homegolf_multiseed_package_report,
    build_parameter_golf_homegolf_public_comparison_report,
    build_parameter_golf_homegolf_score_runtime_report,
    build_parameter_golf_homegolf_track_contract_report, build_remote_training_run_index_v2,
    build_remote_training_visualization_bundle_v2, ParameterGolfHomegolfArtifactAccountingError,
    ParameterGolfHomegolfClusteredRunSurfaceError, ParameterGolfHomegolfCompetitiveAblationError,
    ParameterGolfHomegolfMultiSeedPackageError, ParameterGolfHomegolfPublicComparisonError,
    ParameterGolfHomegolfScoreRuntimeError, RemoteTrainingArtifactSourceKind,
    RemoteTrainingComparabilityClassV2, RemoteTrainingEmissionMode, RemoteTrainingEventSample,
    RemoteTrainingEventSeverity, RemoteTrainingExecutionClassV2, RemoteTrainingMathSample,
    RemoteTrainingPrimaryScoreV2, RemoteTrainingPromotionGatePostureV2, RemoteTrainingProvider,
    RemoteTrainingPublicEquivalenceClassV2, RemoteTrainingRefreshContract,
    RemoteTrainingResultClassification, RemoteTrainingRunIndexEntryV2, RemoteTrainingRunIndexV2,
    RemoteTrainingScoreCloseoutPostureV2, RemoteTrainingScoreDeltaV2,
    RemoteTrainingScoreDirectionV2, RemoteTrainingScoreSurfaceV2, RemoteTrainingSeriesStatus,
    RemoteTrainingSourceArtifact, RemoteTrainingTimelineEntry, RemoteTrainingTrackFamilyV2,
    RemoteTrainingTrackSemanticsV2, RemoteTrainingVisualizationBundleV2,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
    REMOTE_TRAINING_HOMEGOLF_TRACK_ID, REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
};

pub const PARAMETER_GOLF_HOMEGOLF_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH: &str =
    "fixtures/training_visualization/parameter_golf_homegolf_remote_training_visualization_bundle_v2.json";

const HOMEGOLF_BUNDLE_ID: &str = "parameter-golf-homegolf-clustered-score-surface-v2";
const HOMEGOLF_PROFILE_ID: &str = "parameter_golf.homegolf_mixed_cluster";
const HOMEGOLF_LANE_ID: &str = "parameter_golf.homegolf_clustered_score_surface";
const HOMEGOLF_REPO_REVISION: &str = "fixtures@parameter_golf_homegolf_clustered_run_surface.v1";
const HOMEGOLF_SCORE_METRIC_ID: &str = "parameter_golf.validation_bits_per_byte";
const HOMEGOLF_SCORE_UNIT: &str = "bits_per_byte";
const HOMEGOLF_TRACK_DOC_REF: &str = "docs/HOMEGOLF_TRACK.md";
const HOMEGOLF_CLUSTERED_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json";
const HOMEGOLF_SCORE_RUNTIME_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_score_relevant_runtime.json";
const HOMEGOLF_PUBLIC_COMPARISON_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_public_comparison.json";
const HOMEGOLF_ARTIFACT_ACCOUNTING_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_artifact_accounting.json";
const HOMEGOLF_MULTI_SEED_PACKAGE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_multiseed_package.json";
const HOMEGOLF_COMPETITIVE_ABLATION_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_competitive_ablation.json";
const HOMEGOLF_DENSE_BUNDLE_PROOF_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_dense_bundle_proof.json";

const HOMEGOLF_TIMELINE_START_MS: u64 = 1_742_908_800_000;
const HOMEGOLF_TIMELINE_SCORE_RUNTIME_MS: u64 = 1_742_909_400_000;
const HOMEGOLF_TIMELINE_CLOSEOUT_MS: u64 = 1_742_910_000_000;
const HOMEGOLF_TIMELINE_COMPARISON_MS: u64 = 1_742_910_600_000;

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfVisualizationError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Visualization(#[from] RemoteTrainingVisualizationError),
    #[error(transparent)]
    Clustered(#[from] ParameterGolfHomegolfClusteredRunSurfaceError),
    #[error(transparent)]
    ScoreRuntime(#[from] ParameterGolfHomegolfScoreRuntimeError),
    #[error(transparent)]
    PublicComparison(#[from] ParameterGolfHomegolfPublicComparisonError),
    #[error(transparent)]
    ArtifactAccounting(#[from] ParameterGolfHomegolfArtifactAccountingError),
    #[error(transparent)]
    MultiSeed(#[from] ParameterGolfHomegolfMultiSeedPackageError),
    #[error(transparent)]
    CompetitiveAblation(#[from] ParameterGolfHomegolfCompetitiveAblationError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_parameter_golf_homegolf_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, ParameterGolfHomegolfVisualizationError> {
    let track_contract = build_parameter_golf_homegolf_track_contract_report();
    let clustered_surface = build_parameter_golf_homegolf_clustered_run_surface_report()?;
    let score_runtime = build_parameter_golf_homegolf_score_runtime_report()?;
    let public_comparison = build_parameter_golf_homegolf_public_comparison_report()?;
    let artifact_accounting = build_parameter_golf_homegolf_artifact_accounting_report()?;
    let multi_seed = build_parameter_golf_homegolf_multiseed_package_report()?;
    let competitive_ablation = build_parameter_golf_homegolf_competitive_ablation_report()?;

    let projected_tokens_per_second = if score_runtime
        .effective_cluster_train_tokens_per_second
        .is_finite()
        && score_runtime.effective_cluster_train_tokens_per_second > 0.0
    {
        Some(
            score_runtime
                .effective_cluster_train_tokens_per_second
                .round() as u64,
        )
    } else {
        None
    };
    let max_local_wallclock_ms = score_runtime
        .per_device_metrics
        .iter()
        .map(|metric| metric.observed_local_execution_wallclock_ms)
        .max()
        .unwrap_or(score_runtime.observed_cluster_wallclock_ms);
    let min_local_wallclock_ms = score_runtime
        .per_device_metrics
        .iter()
        .map(|metric| metric.observed_local_execution_wallclock_ms)
        .min()
        .unwrap_or(score_runtime.observed_cluster_wallclock_ms);

    Ok(build_remote_training_visualization_bundle_v2(
        RemoteTrainingVisualizationBundleV2 {
            schema_version: String::new(),
            bundle_id: String::from(HOMEGOLF_BUNDLE_ID),
            provider: RemoteTrainingProvider::LocalHybrid,
            profile_id: String::from(HOMEGOLF_PROFILE_ID),
            lane_id: String::from(HOMEGOLF_LANE_ID),
            run_id: clustered_surface.run_id.clone(),
            repo_revision: String::from(HOMEGOLF_REPO_REVISION),
            track_semantics: RemoteTrainingTrackSemanticsV2 {
                track_family: RemoteTrainingTrackFamilyV2::Homegolf,
                track_id: String::from(REMOTE_TRAINING_HOMEGOLF_TRACK_ID),
                execution_class: RemoteTrainingExecutionClassV2::HomeClusterMixedDevice,
                comparability_class: RemoteTrainingComparabilityClassV2::PublicBaselineComparable,
                proof_posture: crate::RemoteTrainingProofPostureV2::ScoreCloseoutMeasured,
                public_equivalence_class:
                    RemoteTrainingPublicEquivalenceClassV2::PublicBaselineComparableOnly,
                score_law_ref: Some(String::from(HOMEGOLF_TRACK_DOC_REF)),
                artifact_cap_bytes: Some(track_contract.artifact_cap_bytes),
                wallclock_cap_seconds: Some(track_contract.wallclock_cap_seconds),
                semantic_summary: String::from(
                    "HOMEGOLF binds a mixed-device home-cluster score closeout to the strict 10-minute public comparison law while refusing public-leaderboard equivalence.",
                ),
            },
            primary_score: Some(RemoteTrainingPrimaryScoreV2 {
                score_metric_id: String::from(HOMEGOLF_SCORE_METRIC_ID),
                score_direction: RemoteTrainingScoreDirectionV2::LowerIsBetter,
                score_unit: String::from(HOMEGOLF_SCORE_UNIT),
                score_value: clustered_surface.final_validation_bits_per_byte,
                score_value_observed_at_ms: HOMEGOLF_TIMELINE_CLOSEOUT_MS,
                score_summary: String::from(
                    "The retained clustered HOMEGOLF surface carries one closed-out contest-style validation score for the mixed-device lane.",
                ),
            }),
            score_surface: Some(RemoteTrainingScoreSurfaceV2 {
                score_closeout_posture: RemoteTrainingScoreCloseoutPostureV2::ScoreClosedOut,
                promotion_gate_posture: RemoteTrainingPromotionGatePostureV2::Held,
                delta_rows: vec![
                    RemoteTrainingScoreDeltaV2 {
                        reference_id: String::from("public_naive_baseline"),
                        score_metric_id: String::from(HOMEGOLF_SCORE_METRIC_ID),
                        reference_score_value: public_comparison.public_naive_baseline.val_bpb,
                        delta_value: public_comparison
                            .delta_vs_public_naive_baseline
                            .delta_val_bpb,
                        delta_summary: String::from(
                            "The current HOMEGOLF closeout remains above the public naive baseline by the retained delta value.",
                        ),
                    },
                    RemoteTrainingScoreDeltaV2 {
                        reference_id: String::from("current_public_leaderboard_best"),
                        score_metric_id: String::from(HOMEGOLF_SCORE_METRIC_ID),
                        reference_score_value: public_comparison
                            .current_public_leaderboard_best
                            .val_bpb,
                        delta_value: public_comparison
                            .delta_vs_current_public_leaderboard_best
                            .delta_val_bpb,
                        delta_summary: String::from(
                            "The current HOMEGOLF closeout remains above the retained public-best row and therefore cannot claim a beat posture.",
                        ),
                    },
                ],
                semantic_summary: String::from(
                    "HOMEGOLF now exposes closed-out score state, retained public-comparison deltas, and a held promotion gate in the shared visualization bundle family.",
                ),
            }),
            result_classification: RemoteTrainingResultClassification::CompletedSuccess,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
                last_heartbeat_at_ms: None,
                heartbeat_seq: 0,
            },
            series_status: RemoteTrainingSeriesStatus::Partial,
            series_unavailable_reason: Some(String::from(
                "HOMEGOLF retained score-closeout, runtime, and comparison truth but did not retain one canonical optimizer-step loss curve for the mixed-device closeout lane",
            )),
            timeline: vec![
                RemoteTrainingTimelineEntry {
                    observed_at_ms: HOMEGOLF_TIMELINE_START_MS,
                    phase: String::from("training"),
                    subphase: Some(String::from("mixed_device_dense_surface")),
                    detail: String::from(
                        "The retained HOMEGOLF clustered surface bound MLX-plus-CUDA dense execution receipts to one mixed-device closeout lane.",
                    ),
                },
                RemoteTrainingTimelineEntry {
                    observed_at_ms: HOMEGOLF_TIMELINE_SCORE_RUNTIME_MS,
                    phase: String::from("runtime"),
                    subphase: Some(String::from("score_relevant_dense_runtime")),
                    detail: String::from(
                        "The score-relevant runtime report proved resident mixed-device dense execution and projected more than one dataset pass within the 600-second cap.",
                    ),
                },
                RemoteTrainingTimelineEntry {
                    observed_at_ms: HOMEGOLF_TIMELINE_CLOSEOUT_MS,
                    phase: String::from("score_closeout"),
                    subphase: Some(String::from("mixed_device_validation")),
                    detail: String::from(
                        "The retained clustered surface sealed one contest-style HOMEGOLF validation closeout for the mixed-device lane.",
                    ),
                },
                RemoteTrainingTimelineEntry {
                    observed_at_ms: HOMEGOLF_TIMELINE_COMPARISON_MS,
                    phase: String::from("comparison"),
                    subphase: Some(String::from("public_baseline_and_promotion_gate")),
                    detail: String::from(
                        "The public-comparison, artifact-accounting, and promotion-hold surfaces were retained in the same machine-legible HOMEGOLF evidence family.",
                    ),
                },
            ],
            summary: RemoteTrainingVisualizationSummary {
                total_steps_completed: score_runtime.observed_step_count,
                latest_global_step: Some(score_runtime.observed_step_count),
                latest_train_loss: None,
                latest_ema_loss: None,
                latest_validation_loss: Some(clustered_surface.final_validation_mean_loss as f32),
                latest_tokens_per_second: projected_tokens_per_second,
                latest_samples_per_second_milli: None,
                accumulated_cost_microusd: None,
                latest_checkpoint_ref: Some(clustered_surface.scored_model_artifact_ref.clone()),
                detail: String::from(
                    "HOMEGOLF now retains one post-run mixed-device score closeout with shared track semantics, retained public deltas, held promotion posture, and score-relevant runtime throughput.",
                ),
            },
            heartbeat_series: Vec::new(),
            loss_series: Vec::new(),
            math_series: vec![RemoteTrainingMathSample {
                observed_at_ms: HOMEGOLF_TIMELINE_SCORE_RUNTIME_MS,
                global_step: Some(score_runtime.observed_step_count),
                learning_rate: None,
                gradient_norm: None,
                parameter_norm: None,
                update_norm: None,
                clip_fraction: None,
                clip_event_count: None,
                loss_scale: None,
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (
                        String::from("projected_dataset_passes_within_cap"),
                        score_runtime.projected_dataset_passes_within_cap as f32,
                    ),
                    (
                        String::from("mean_cuda_submesh_step_ms"),
                        score_runtime.phase_breakdown.mean_cuda_submesh_step_ms as f32,
                    ),
                    (
                        String::from("mean_mlx_rank_step_ms"),
                        score_runtime.phase_breakdown.mean_mlx_rank_step_ms as f32,
                    ),
                    (
                        String::from("mean_cross_backend_bridge_ms"),
                        score_runtime.phase_breakdown.mean_cross_backend_bridge_ms as f32,
                    ),
                ]),
            }],
            runtime_series: vec![crate::RemoteTrainingRuntimeSample {
                observed_at_ms: HOMEGOLF_TIMELINE_SCORE_RUNTIME_MS,
                data_wait_ms: None,
                forward_ms: None,
                backward_ms: None,
                optimizer_ms: Some(score_runtime.phase_breakdown.mean_optimizer_step_ms.round() as u64),
                checkpoint_ms: None,
                evaluation_ms: None,
                tokens_per_second: projected_tokens_per_second,
                samples_per_second_milli: None,
            }],
            gpu_series: Vec::new(),
            distributed_series: vec![crate::RemoteTrainingDistributedSample {
                observed_at_ms: HOMEGOLF_TIMELINE_SCORE_RUNTIME_MS,
                participating_rank_count: score_runtime.per_device_metrics.len() as u16,
                rank_skew_ms: Some(max_local_wallclock_ms.saturating_sub(min_local_wallclock_ms)),
                slowest_rank_ms: Some(max_local_wallclock_ms),
                collective_ms: Some(
                    score_runtime
                        .phase_breakdown
                        .mean_cross_backend_bridge_ms
                        .round() as u64,
                ),
                stalled_rank_count: 0,
            }],
            event_series: vec![
                RemoteTrainingEventSample {
                    observed_at_ms: HOMEGOLF_TIMELINE_CLOSEOUT_MS,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("homegolf_score_closeout_measured"),
                    detail: String::from(
                        "HOMEGOLF sealed one mixed-device validation closeout with measured bits-per-byte and counted artifact bytes.",
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: HOMEGOLF_TIMELINE_COMPARISON_MS,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("homegolf_public_delta_retained"),
                    detail: format!(
                        "HOMEGOLF retained deltas of {:.6} and {:.6} bits/byte against the public naive baseline and current public-best row.",
                        public_comparison.delta_vs_public_naive_baseline.delta_val_bpb,
                        public_comparison.delta_vs_current_public_leaderboard_best.delta_val_bpb
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: HOMEGOLF_TIMELINE_COMPARISON_MS + 1,
                    severity: RemoteTrainingEventSeverity::Warning,
                    event_kind: String::from("homegolf_promotion_gate_held"),
                    detail: String::from(
                        "Promotion remains held because the mixed-device closeout is public-baseline comparable only and still not public-leaderboard equivalent.",
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: HOMEGOLF_TIMELINE_COMPARISON_MS + 2,
                    severity: RemoteTrainingEventSeverity::Warning,
                    event_kind: String::from("homegolf_beat_claim_unsupported"),
                    detail: String::from(
                        "The retained multi-seed package does not support beat claims against the public naive baseline or current public best.",
                    ),
                },
            ],
            source_artifacts: vec![
                source_artifact("homegolf_track_contract", track_contract.report_digest, HOMEGOLF_TRACK_DOC_REF, true, String::from(
                    "The HOMEGOLF track contract remains authoritative for score law, cap semantics, and public-comparison limits.",
                )),
                source_artifact("clustered_homegolf_score_surface", clustered_surface.report_digest, HOMEGOLF_CLUSTERED_SURFACE_REF, true, String::from(
                    "The clustered HOMEGOLF run surface remains authoritative for mixed-device closeout score, artifact identity, and publish or promotion hold posture.",
                )),
                source_artifact("homegolf_score_runtime", score_runtime.report_digest, HOMEGOLF_SCORE_RUNTIME_REF, true, String::from(
                    "The score-relevant runtime report remains authoritative for projected throughput and mixed-device dense runtime residency.",
                )),
                source_artifact("homegolf_public_comparison", public_comparison.report_digest, HOMEGOLF_PUBLIC_COMPARISON_REF, true, String::from(
                    "The public comparison report remains authoritative for retained deltas versus the public baseline and current public best.",
                )),
                source_artifact("homegolf_artifact_accounting", artifact_accounting.report_digest, HOMEGOLF_ARTIFACT_ACCOUNTING_REF, true, String::from(
                    "The artifact accounting report remains authoritative for 16MB cap posture and counted-byte deltas.",
                )),
                source_artifact("homegolf_multiseed_package", multi_seed.report_digest, HOMEGOLF_MULTI_SEED_PACKAGE_REF, true, String::from(
                    "The repeated-run package remains authoritative for beat-claim support and reproducibility posture.",
                )),
                source_artifact("homegolf_competitive_ablation", competitive_ablation.report_digest, HOMEGOLF_COMPETITIVE_ABLATION_REF, false, String::from(
                    "The competitive ablation report remains the retained challenger-lane planning surface for the best-known exact HOMEGOLF variant.",
                )),
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("homegolf_dense_bundle_proof"),
                    artifact_uri: String::from(HOMEGOLF_DENSE_BUNDLE_PROOF_REF),
                    artifact_digest: None,
                    source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
                    authoritative: false,
                    source_receipt_ids: vec![String::from(
                        "psionic.parameter_golf_homegolf_dense_bundle_proof.v1",
                    )],
                    detail: String::from(
                        "The dense bundle proof remains the retained train-to-infer closure surface for the exact HOMEGOLF family.",
                    ),
                },
            ],
            bundle_digest: String::new(),
        },
    )?)
}

pub fn build_parameter_golf_homegolf_run_index_entry_v2(
    bundle: &RemoteTrainingVisualizationBundleV2,
) -> Result<RemoteTrainingRunIndexEntryV2, ParameterGolfHomegolfVisualizationError> {
    let entry = RemoteTrainingRunIndexEntryV2 {
        provider: bundle.provider,
        profile_id: bundle.profile_id.clone(),
        lane_id: bundle.lane_id.clone(),
        run_id: bundle.run_id.clone(),
        repo_revision: bundle.repo_revision.clone(),
        track_semantics: bundle.track_semantics.clone(),
        primary_score: bundle.primary_score.clone(),
        score_surface: bundle.score_surface.clone(),
        result_classification: bundle.result_classification,
        series_status: bundle.series_status,
        series_unavailable_reason: bundle.series_unavailable_reason.clone(),
        last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
        bundle_artifact_uri: Some(String::from(
            PARAMETER_GOLF_HOMEGOLF_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
        )),
        bundle_digest: Some(bundle.bundle_digest.clone()),
        semantic_summary: String::from(
            "HOMEGOLF mixed-device score closeout now appears in the shared track-aware training run index with retained score deltas and held promotion posture.",
        ),
    };
    entry.validate()?;
    Ok(entry)
}

pub fn write_parameter_golf_homegolf_visualization_bundle_v2(
    output_path: &Path,
) -> Result<RemoteTrainingVisualizationBundleV2, ParameterGolfHomegolfVisualizationError> {
    let bundle = build_parameter_golf_homegolf_visualization_bundle_v2()?;
    write_json(output_path, &bundle)?;
    Ok(bundle)
}

pub fn write_parameter_golf_homegolf_visualization_artifacts_v2(
    bundle_path: &Path,
    run_index_path: &Path,
) -> Result<
    (
        RemoteTrainingVisualizationBundleV2,
        RemoteTrainingRunIndexV2,
    ),
    ParameterGolfHomegolfVisualizationError,
> {
    let bundle = build_parameter_golf_homegolf_visualization_bundle_v2()?;
    let entry = build_parameter_golf_homegolf_run_index_entry_v2(&bundle)?;
    let run_index = build_remote_training_run_index_v2(RemoteTrainingRunIndexV2 {
        schema_version: String::new(),
        index_id: String::from("parameter-golf-homegolf-run-index-v2"),
        generated_at_ms: HOMEGOLF_TIMELINE_COMPARISON_MS,
        entries: vec![entry],
        detail: String::from(
            "HOMEGOLF now emits a dedicated v2 training run index entry instead of leaving score-closeout state in sidecar reports only.",
        ),
        index_digest: String::new(),
    })?;
    write_json(bundle_path, &bundle)?;
    write_json(run_index_path, &run_index)?;
    Ok((bundle, run_index))
}

fn source_artifact(
    artifact_role: &str,
    artifact_digest: String,
    artifact_uri: &str,
    authoritative: bool,
    detail: String,
) -> RemoteTrainingSourceArtifact {
    RemoteTrainingSourceArtifact {
        artifact_role: String::from(artifact_role),
        artifact_uri: String::from(artifact_uri),
        artifact_digest: Some(artifact_digest),
        source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
        authoritative,
        source_receipt_ids: Vec::new(),
        detail,
    }
}

fn write_json<T: serde::Serialize>(
    output_path: &Path,
    value: &T,
) -> Result<(), ParameterGolfHomegolfVisualizationError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfVisualizationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        output_path,
        format!("{}\n", serde_json::to_string_pretty(value)?),
    )
    .map_err(|error| ParameterGolfHomegolfVisualizationError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_homegolf_bundle() -> RemoteTrainingVisualizationBundleV2 {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/parameter_golf_homegolf_remote_training_visualization_bundle_v2.json"
        ))
        .expect("HOMEGOLF v2 bundle should parse")
    }

    #[test]
    fn homegolf_v2_bundle_stays_valid() -> Result<(), ParameterGolfHomegolfVisualizationError> {
        sample_homegolf_bundle().validate()?;
        Ok(())
    }

    #[test]
    fn homegolf_v2_bundle_carries_closed_score_surface(
    ) -> Result<(), ParameterGolfHomegolfVisualizationError> {
        let bundle = build_parameter_golf_homegolf_visualization_bundle_v2()?;
        let score_surface = bundle
            .score_surface
            .as_ref()
            .expect("HOMEGOLF bundle should carry score_surface");
        assert_eq!(
            score_surface.score_closeout_posture,
            RemoteTrainingScoreCloseoutPostureV2::ScoreClosedOut
        );
        assert_eq!(
            score_surface.promotion_gate_posture,
            RemoteTrainingPromotionGatePostureV2::Held
        );
        assert_eq!(score_surface.delta_rows.len(), 2);
        Ok(())
    }
}
