use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, build_probe_gepa_terminal_bench_pylon_canary_bundle,
    validate_probe_gepa_terminal_bench_pylon_canary_bundle,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let bundle = build_probe_gepa_terminal_bench_pylon_canary_bundle(&manifest)
        .map_err(|error| format!("failed to build Pylon canary bundle: {error:?}"))?;
    validate_probe_gepa_terminal_bench_pylon_canary_bundle(&bundle, &manifest)
        .map_err(|error| format!("failed to validate Pylon canary bundle: {error:?}"))?;

    let summary = serde_json::json!({
        "bundle_id": bundle.bundle_id,
        "bundle_ref": bundle.bundle_ref,
        "receipt_ref": bundle.receipt_ref,
        "pylon_ref": bundle.pylon_ref,
        "pylon_assignment_ref": bundle.pylon_assignment_ref,
        "probe_run_ref": bundle.probe_run_ref,
        "task_refs": bundle.task_refs,
        "metric_call_count": bundle.metric_calls.len(),
        "metric_call_status": bundle.metric_calls[0].status,
        "worker_closeout_state": bundle.metric_calls[0].closeout_state,
        "closeout_file_count": bundle.closeout_bundle_file_refs.len(),
        "artifact_refs": bundle.artifact_refs,
        "proof_refs": bundle.proof_refs,
        "closeout_refs": bundle.closeout_refs,
        "live_pylon_event_refs": bundle.live_pylon_event_refs,
        "psionic_import_refs": bundle.psionic_import_refs,
        "public_status": bundle.public_status_label,
        "public_claim_level": bundle.public_claim_level,
        "no_lora": bundle.no_lora,
        "no_model_training": bundle.no_model_training,
        "no_public_leaderboard_claim": bundle.no_public_leaderboard_claim,
        "no_paid_work_claim": bundle.no_paid_work_claim,
        "no_settlement_claim": bundle.no_settlement_claim,
        "no_runtime_promotion": bundle.no_runtime_promotion,
        "redaction_state": bundle.redaction_state,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
