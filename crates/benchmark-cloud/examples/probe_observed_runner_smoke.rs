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
        .find(|task| {
            task.task_ref == "benchmark_task.terminal_bench.retained.configure_git_webserver.v1"
        })
        .ok_or_else(|| String::from("missing retained configure-git-webserver task"))?;
    let assignment = probe_assignment_from_split_task(
        &manifest,
        task,
        "probe.commit.observed-runner-smoke",
        "sha256:0000000000000000000000000000000000000000000000000000000000003001",
        vec![String::from(
            "blueprint_signature.probe.terminal_bench.configure_git_webserver.v1",
        )],
        "tool_menu.probe.terminal_bench.safe_shell_edit.v1",
    );
    let materialization = materialize_probe_benchmark_task(
        &assignment,
        manifest.retained_fixture_refs.clone(),
        vec![
            String::from("workspace_ref.public_sandbox.task_checkout.v1"),
            String::from("workspace_ref.public_sandbox.probe_artifacts.v1"),
        ],
        "sandbox_policy.benchmark_cloud.probe.shc_harbor_public_safe.v1",
    )
    .map_err(|error| format!("failed to materialize task: {error:?}"))?;
    let closeout = observed_closeout(task, &assignment);
    let artifacts = run_observed_probe_benchmark_task(
        &manifest,
        task,
        &assignment,
        &materialization,
        &closeout,
    )
    .map_err(|error| format!("failed to build observed runner artifacts: {error:?}"))?;

    let summary = serde_json::json!({
        "assignment_ref": assignment.assignment_ref,
        "materialization_ref": materialization.materialization_ref,
        "run_ref": artifacts.result_json.run_ref,
        "status": artifacts.result_json.status,
        "evidence_split": artifacts.result_json.evidence_split,
        "file_names": artifacts.file_names(),
        "event_count": artifacts.events_jsonl.len(),
        "artifact_manifest_ref": artifacts.artifact_manifest_json.manifest_ref,
        "proof_bundle_ref": artifacts.proof_bundle_json.proof_bundle_ref,
        "resource_usage_receipt_ref": artifacts.resource_usage_receipt_json.receipt_ref,
        "route_scorecard_ref": artifacts.route_scorecard_json.scorecard_ref,
        "failure_classification_ref": artifacts.result_json.failure_classification_ref,
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

fn observed_closeout(
    task: &benchmark_cloud::BenchmarkSplitTaskEntry,
    assignment: &benchmark_cloud::ProbeBenchmarkAssignment,
) -> ProbeBenchmarkObservedCloseout {
    let run_ref = format!("benchmark_run.probe.observed.{}", task.task_id);
    ProbeBenchmarkObservedCloseout {
        closeout_ref: format!("probe_closeout.{run_ref}"),
        run_ref: run_ref.clone(),
        assignment_ref: assignment.assignment_ref.clone(),
        candidate_hash: assignment.candidate_hash.clone(),
        run_status: BenchmarkRunStatus::Succeeded,
        score_bps: Some(10_000),
        artifact_manifest_refs: vec![format!("artifact_manifest.probe.observed.{}", task.task_id)],
        proof_bundle_refs: vec![format!("proof_bundle.probe.observed.{}", task.task_id)],
        resource_usage_receipt_ref: Some(format!("resource_usage.probe.observed.{}", task.task_id)),
        resource_unavailable_reason: None,
        verifier_result_refs: vec![format!("verifier_result.probe.observed.{}", task.task_id)],
        event_refs: vec![
            format!("event.probe.observed.{}.assignment_accepted", task.task_id),
            format!(
                "event.probe.observed.{}.progress_refs_streamed",
                task.task_id
            ),
            format!("event.probe.observed.{}.artifact_submitted", task.task_id),
            format!("event.probe.observed.{}.closeout_accepted", task.task_id),
        ],
        policy_finding_refs: Vec::new(),
        partial_artifact_refs: vec![format!(
            "artifact.probe.observed.{}.stdout_summary",
            task.task_id
        )],
        failure_classification_ref: None,
        route_scorecard: ProbeBenchmarkRouteScorecard {
            schema_ref: String::from(PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF),
            scorecard_ref: format!("route_scorecard.probe.{run_ref}"),
            candidate_hash: assignment.candidate_hash.clone(),
            expected_cost_ref: String::from("cost.probe.expected.unpaid_smoke"),
            expected_latency_ms: 120_000,
            observed_cost_ref: String::from("cost.probe.observed.unpaid_smoke"),
            observed_latency_ms: 42_000,
            post_closeout_route_score_bps: 8_200,
            privacy_tier: ProbeBenchmarkPrivacyTier::ShcBox,
            rejected_routes: vec![ProbeBenchmarkRejectedRoute {
                reason_ref: String::from("reason.probe.route.local_qwen_not_admitted"),
                route_kind: ProbeBenchmarkRouteKind::LocalQwen,
                route_ref: String::from("route.probe.local_qwen"),
            }],
            route_reason_ref: String::from("reason.probe.route.shc_harbor_probe_smoke"),
            selected_agent_or_model_ref: String::from("agent.probe.current"),
            selected_isolation_profile_ref: String::from("isolation.shc.harbor.terminal_bench"),
            selected_provider_ref: String::from("pylon.public.demo.alpha"),
            selected_route_kind: ProbeBenchmarkRouteKind::Pylon,
            selected_runner_ref: String::from("runner.benchmark_cloud.probe.observed"),
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            selected_verifier_ref: task.scorer_verifier.verifier_ref.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
            trust_tier: ProbeBenchmarkTrustTier::RegisteredPylon,
        },
        started_at: String::from("2026-06-08T12:00:00.000Z"),
        completed_at: String::from("2026-06-08T12:00:42.000Z"),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    }
}
