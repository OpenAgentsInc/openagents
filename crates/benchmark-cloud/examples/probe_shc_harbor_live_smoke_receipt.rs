use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, BenchmarkRedactionState, BenchmarkRunStatus,
    PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF, ProbeBenchmarkObservedCloseout,
    ProbeBenchmarkPrivacyTier, ProbeBenchmarkRejectedRoute, ProbeBenchmarkRouteKind,
    ProbeBenchmarkRouteScorecard, ProbeBenchmarkTrustTier, materialize_probe_benchmark_task,
    probe_assignment_from_split_task, run_observed_probe_benchmark_task,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let task = manifest
        .tasks
        .iter()
        .find(|task| task.task_ref == "benchmark_task.terminal_bench.retained.db_wal_recovery.v1")
        .ok_or_else(|| String::from("missing retained db-wal-recovery task"))?;
    let assignment = probe_assignment_from_split_task(
        &manifest,
        task,
        "probe.commit.shc-live-smoke-20260608",
        "sha256:0000000000000000000000000000000000000000000000000000000000004563",
        vec![String::from(
            "blueprint_signature.probe.terminal_bench.sqlite_wal_recovery.v1",
        )],
        "tool_menu.probe.terminal_bench.safe_shell_edit.v1",
    );
    let materialization = materialize_probe_benchmark_task(
        &assignment,
        manifest.retained_fixture_refs.clone(),
        vec![
            String::from("workspace_ref.shc.oa_shc_katy_01.harbor.db_wal_recovery"),
            String::from("workspace_ref.shc.oa_shc_katy_01.probe_closeout_bundle"),
        ],
        "sandbox_policy.benchmark_cloud.probe.shc_harbor_public_safe.v1",
    )
    .map_err(|error| format!("failed to materialize task: {error:?}"))?;
    let closeout = live_closeout(task, &assignment);
    let artifacts = run_observed_probe_benchmark_task(
        &manifest,
        task,
        &assignment,
        &materialization,
        &closeout,
    )
    .map_err(|error| format!("failed to build live smoke receipt: {error:?}"))?;

    let summary = serde_json::json!({
        "shc_host": "oa-shc-katy-01",
        "harbor_job_id": "e487217a-715e-448c-8d45-e528b76980e7",
        "harbor_trial_id": "a6c6c245-b9c0-44a8-a8c0-0c7fe5cc3383",
        "assignment_ref": assignment.assignment_ref,
        "materialization_ref": materialization.materialization_ref,
        "run_ref": artifacts.result_json.run_ref,
        "status": artifacts.result_json.status,
        "evidence_split": artifacts.result_json.evidence_split,
        "score_bps": artifacts.result_json.score_bps,
        "failure_classification_ref": artifacts.result_json.failure_classification_ref,
        "resource_unavailable_reason": artifacts.result_json.resource_unavailable_reason,
        "file_names": artifacts.file_names(),
        "event_count": artifacts.events_jsonl.len(),
        "artifact_manifest_ref": artifacts.artifact_manifest_json.manifest_ref,
        "proof_bundle_ref": artifacts.proof_bundle_json.proof_bundle_ref,
        "route_scorecard_ref": artifacts.route_scorecard_json.scorecard_ref,
        "public_claim": artifacts.result_json.public_claim_boundary.claim_level,
        "public_claim_upgrade_authority": artifacts.result_json.public_claim_boundary.public_claim_upgrade_authority,
        "redaction_state": artifacts.result_json.redaction_state,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}

fn live_closeout(
    task: &benchmark_cloud::BenchmarkSplitTaskEntry,
    assignment: &benchmark_cloud::ProbeBenchmarkAssignment,
) -> ProbeBenchmarkObservedCloseout {
    let run_ref = "benchmark_run.probe.shc_harbor.db_wal_recovery.20260608";
    ProbeBenchmarkObservedCloseout {
        closeout_ref: String::from("probe_closeout.shc_harbor.db_wal_recovery.20260608"),
        run_ref: String::from(run_ref),
        assignment_ref: assignment.assignment_ref.clone(),
        candidate_hash: assignment.candidate_hash.clone(),
        run_status: BenchmarkRunStatus::Failed,
        score_bps: None,
        artifact_manifest_refs: vec![String::from(
            "artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608",
        )],
        proof_bundle_refs: vec![String::from(
            "proof_bundle.probe.shc_harbor.db_wal_recovery.20260608",
        )],
        resource_usage_receipt_ref: None,
        resource_unavailable_reason: Some(String::from(
            "shc_harbor_meter_unavailable_after_nonzero_agent_exit",
        )),
        verifier_result_refs: vec![String::from(
            "verifier_result.terminal_bench.db_wal_recovery.shc_harbor.20260608.reward_0",
        )],
        event_refs: vec![
            String::from("event.shc_harbor.db_wal_recovery.20260608.assignment_accepted"),
            String::from("event.shc_harbor.db_wal_recovery.20260608.harbor_started"),
            String::from("event.shc_harbor.db_wal_recovery.20260608.agent_nonzero_exit"),
            String::from("event.shc_harbor.db_wal_recovery.20260608.closeout_rejected"),
        ],
        policy_finding_refs: Vec::new(),
        partial_artifact_refs: vec![
            String::from("artifact_ref.shc_harbor.db_wal_recovery.20260608.job_result_json"),
            String::from("artifact_ref.shc_harbor.db_wal_recovery.20260608.trial_result_json"),
            String::from(
                "artifact_ref.shc_harbor.db_wal_recovery.20260608.agent_trajectory_digest",
            ),
        ],
        failure_classification_ref: Some(String::from(
            "failure_classification.probe.shc_harbor.db_wal_recovery.nonzero_agent_exit",
        )),
        route_scorecard: ProbeBenchmarkRouteScorecard {
            schema_ref: String::from(PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF),
            scorecard_ref: String::from(
                "route_scorecard.probe.shc_harbor.db_wal_recovery.20260608",
            ),
            candidate_hash: assignment.candidate_hash.clone(),
            expected_cost_ref: String::from("cost.probe.expected.unpaid_smoke"),
            expected_latency_ms: 900_000,
            observed_cost_ref: String::from("cost.probe.observed.shc_harbor.unavailable"),
            observed_latency_ms: 61_499,
            post_closeout_route_score_bps: 1_000,
            privacy_tier: ProbeBenchmarkPrivacyTier::ShcBox,
            rejected_routes: vec![
                ProbeBenchmarkRejectedRoute {
                    reason_ref: String::from("reason.probe.route.apple_fm_not_admitted_on_shc"),
                    route_kind: ProbeBenchmarkRouteKind::AppleFm,
                    route_ref: String::from("route.probe.apple_fm.local"),
                },
                ProbeBenchmarkRejectedRoute {
                    reason_ref: String::from(
                        "reason.probe.route.pylon_worker_not_wired_for_live_smoke",
                    ),
                    route_kind: ProbeBenchmarkRouteKind::Pylon,
                    route_ref: String::from("route.probe.pylon.unpaid_smoke"),
                },
            ],
            route_reason_ref: String::from("reason.probe.route.shc_harbor_probe_signature_smoke"),
            selected_agent_or_model_ref: String::from(
                "agent.probe_codex.signature.sqlite_wal_recovery",
            ),
            selected_isolation_profile_ref: String::from("isolation.shc.harbor.terminal_bench"),
            selected_provider_ref: String::from("shc.oa_shc_katy_01"),
            selected_route_kind: ProbeBenchmarkRouteKind::ProbeCodex,
            selected_runner_ref: String::from("runner.shc.oa_shc_katy_01.harbor"),
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            selected_verifier_ref: task.scorer_verifier.verifier_ref.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
            trust_tier: ProbeBenchmarkTrustTier::OwnedWorker,
        },
        started_at: String::from("2026-06-08T13:05:43.262399Z"),
        completed_at: String::from("2026-06-08T13:06:44.761206Z"),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    }
}
