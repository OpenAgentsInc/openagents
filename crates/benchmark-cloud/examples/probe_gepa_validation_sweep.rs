use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, build_probe_gepa_validation_sweep,
    validate_probe_gepa_validation_sweep,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let sweep = build_probe_gepa_validation_sweep(
        &manifest,
        "probe-main-validation-sweep",
        "sha256:0000000000000000000000000000000000000000000000000000000000002008",
    )
    .map_err(|error| format!("failed to build validation sweep: {error:?}"))?;
    validate_probe_gepa_validation_sweep(&sweep, &manifest)
        .map_err(|error| format!("failed to validate sweep: {error:?}"))?;

    let summary = serde_json::json!({
        "campaign_id": sweep.campaign_id,
        "sweep_ref": sweep.sweep_ref,
        "split_manifest_ref": sweep.split_manifest_ref,
        "validation_task_count": sweep.validation_task_refs.len(),
        "route_count": sweep.routes.len(),
        "rollout_count": sweep.rollout_records.len(),
        "probe_commit": sweep.probe_commit,
        "gepa_candidate_hash": sweep.gepa_candidate_hash,
        "cost_record_count": sweep.rollout_records.iter().filter(|rollout| rollout.cost_ref.is_some()).count(),
        "duration_record_count": sweep.rollout_records.iter().filter(|rollout| rollout.duration_ms.is_some()).count(),
        "verifier_result_count": sweep.rollout_records.iter().filter(|rollout| !rollout.verifier_result_ref.is_empty()).count(),
        "artifact_available_count": sweep.rollout_records.iter().filter(|rollout| rollout.artifact_available).count(),
        "candidate_may_move_to_shadow": sweep.candidate_may_move_to_shadow,
        "omega_blueprint_gate_refs": sweep.omega_blueprint_gate_refs,
        "public_claim": sweep.public_claim_label,
        "no_public_beats_terminal_bench_claim": sweep.no_public_beats_terminal_bench_claim,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
