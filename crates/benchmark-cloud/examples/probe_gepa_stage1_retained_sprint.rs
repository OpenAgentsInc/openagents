use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, BenchmarkCloseoutState, GepaCandidateDecisionState,
    build_probe_gepa_stage1_retained_sprint, validate_probe_gepa_stage1_retained_sprint,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let campaign =
        build_probe_gepa_stage1_retained_sprint(&manifest, "probe-main-stage1-retained-sprint")
            .map_err(|error| format!("failed to build Stage 1 sprint: {error:?}"))?;
    validate_probe_gepa_stage1_retained_sprint(&campaign, &manifest)
        .map_err(|error| format!("failed to validate Stage 1 sprint: {error:?}"))?;

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
        "worker_assignment_count": campaign.worker_assignment_refs.len(),
        "metric_call_count": campaign.metric_calls.len(),
        "accepted_closeout_count": accepted_count,
        "rejected_closeout_count": rejected_count,
        "explicit_payment_modes": campaign
            .metric_calls
            .iter()
            .map(|call| call.payment_mode.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        "selected_candidate_ref": campaign.selected_candidate_ref,
        "selected_candidate_decision": match campaign.selected_candidate_decision {
            GepaCandidateDecisionState::OptimizerAccepted => "optimizer_accepted",
            GepaCandidateDecisionState::Rejected => "rejected",
        },
        "candidate_improves_or_preserves_retained_fixtures": campaign
            .candidate_improves_or_preserves_retained_fixtures,
        "policy_gate_failure": campaign.policy_gate_failure,
        "public_summary": campaign.public_summary_label,
        "public_summary_evidence_scope": campaign.public_summary_evidence_scope,
        "no_lora": campaign.no_lora,
        "no_model_training": campaign.no_model_training,
        "no_public_leaderboard_claim": campaign.no_public_leaderboard_claim,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
