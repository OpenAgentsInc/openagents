use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, BenchmarkCloseoutState,
    build_probe_gepa_stage0_live_receipt_bundle, validate_probe_gepa_stage0_live_receipt_bundle,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let bundle = build_probe_gepa_stage0_live_receipt_bundle(
        &manifest,
        "probe.commit.shc-live-smoke-20260608",
    )
    .map_err(|error| format!("failed to build Stage 0 live receipt bundle: {error:?}"))?;
    validate_probe_gepa_stage0_live_receipt_bundle(&bundle, &manifest)
        .map_err(|error| format!("failed to validate Stage 0 live receipt bundle: {error:?}"))?;

    let rejected_count = bundle
        .metric_calls
        .iter()
        .filter(|call| call.closeout_state == BenchmarkCloseoutState::Rejected)
        .count();
    let summary = serde_json::json!({
        "bundle_id": bundle.bundle_id,
        "bundle_ref": bundle.bundle_ref,
        "campaign_ref": bundle.campaign_ref,
        "source_issue_refs": bundle.source_issue_refs,
        "shc_host_ref": bundle.shc_host_ref,
        "harbor_job_id": bundle.harbor_job_id,
        "harbor_trial_id": bundle.harbor_trial_id,
        "live_assignment_count": bundle.live_assignment_ids.len(),
        "metric_call_count": bundle.metric_calls.len(),
        "rejected_closeout_count": rejected_count,
        "artifact_file_count": bundle.normalized_artifact_file_names.len(),
        "artifact_manifest_refs": bundle.artifact_manifest_refs,
        "proof_bundle_refs": bundle.proof_bundle_refs,
        "resource_usage_receipt_refs": bundle.resource_usage_receipt_refs,
        "route_scorecard_refs": bundle.route_scorecard_refs,
        "failure_classification_refs": bundle.failure_classification_refs,
        "event_ref_count": bundle.event_refs.len(),
        "psionic_import_refs": bundle.psionic_import_refs,
        "public_status": bundle.public_status_label,
        "public_claim_level": bundle.public_claim_level,
        "no_lora": bundle.no_lora,
        "no_model_training": bundle.no_model_training,
        "no_public_leaderboard_claim": bundle.no_public_leaderboard_claim,
        "no_paid_work_claim": bundle.no_paid_work_claim,
        "redaction_state": bundle.redaction_state,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
