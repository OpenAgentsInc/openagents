use benchmark_cloud::{
    BenchmarkCampaignSplitManifest, build_probe_shc_harbor_live_smoke_artifacts,
};

const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str =
    include_str!("../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json");

fn main() -> Result<(), String> {
    let manifest: BenchmarkCampaignSplitManifest =
        serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)
            .map_err(|error| format!("failed to parse split manifest: {error}"))?;
    let live = build_probe_shc_harbor_live_smoke_artifacts(
        &manifest,
        "probe.commit.shc-live-smoke-20260608",
    )
    .map_err(|error| format!("failed to build live smoke receipt: {error:?}"))?;

    let summary = serde_json::json!({
        "shc_host": "oa-shc-katy-01",
        "harbor_job_id": "e487217a-715e-448c-8d45-e528b76980e7",
        "harbor_trial_id": "a6c6c245-b9c0-44a8-a8c0-0c7fe5cc3383",
        "assignment_ref": live.assignment.assignment_ref,
        "materialization_ref": live.materialization.materialization_ref,
        "run_ref": live.artifacts.result_json.run_ref,
        "status": live.artifacts.result_json.status,
        "evidence_split": live.artifacts.result_json.evidence_split,
        "score_bps": live.artifacts.result_json.score_bps,
        "failure_classification_ref": live.artifacts.result_json.failure_classification_ref,
        "resource_unavailable_reason": live.artifacts.result_json.resource_unavailable_reason,
        "resource_usage_receipt_ref": live.artifacts.resource_usage_receipt_json.receipt_ref,
        "file_names": live.artifacts.file_names(),
        "event_count": live.artifacts.events_jsonl.len(),
        "artifact_manifest_ref": live.artifacts.artifact_manifest_json.manifest_ref,
        "proof_bundle_ref": live.artifacts.proof_bundle_json.proof_bundle_ref,
        "route_scorecard_ref": live.artifacts.route_scorecard_json.scorecard_ref,
        "public_claim": live.artifacts.result_json.public_claim_boundary.claim_level,
        "public_claim_upgrade_authority": live.artifacts.result_json.public_claim_boundary.public_claim_upgrade_authority,
        "redaction_state": live.artifacts.result_json.redaction_state,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("failed to render summary: {error}"))?
    );
    Ok(())
}
