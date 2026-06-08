use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, BenchmarkCloseoutState, build_probe_gepa_stage0_smoke_campaign,
    validate_probe_gepa_stage0_smoke_campaign,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let campaign = build_probe_gepa_stage0_smoke_campaign(&manifest, "probe-main-stage0-smoke")
        .map_err(|error| format!("failed to build campaign: {error:?}"))?;
    validate_probe_gepa_stage0_smoke_campaign(&campaign, &manifest)
        .map_err(|error| format!("failed to validate campaign: {error:?}"))?;

    let accepted_count = campaign
        .metric_calls
        .iter()
        .filter(|call| call.closeout_state == BenchmarkCloseoutState::Accepted)
        .count();
    let rejected_count = campaign.metric_calls.len() - accepted_count;
    let summary = serde_json::json!({
        "campaign_id": campaign.campaign_id,
        "split_manifest_ref": campaign.split_manifest_ref,
        "candidate_count": campaign.candidates.len(),
        "retained_fixture_count": campaign.retained_fixture_refs.len(),
        "metric_call_count": campaign.metric_calls.len(),
        "accepted_closeout_count": accepted_count,
        "rejected_closeout_count": rejected_count,
        "pylon_assignment_ref_count": campaign
            .metric_calls
            .iter()
            .filter(|call| call.pylon_assignment_ref.is_some())
            .count(),
        "public_status": campaign.public_status_label,
        "promotion_state_ref": campaign.promotion_state_ref,
        "no_lora": campaign.no_lora,
        "no_model_training": campaign.no_model_training,
        "no_public_leaderboard_claim": campaign.no_public_leaderboard_claim,
        "automatic_promotion_enabled": campaign.automatic_promotion_enabled,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
