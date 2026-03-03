use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_PERFORMANCE_SCORECARD_THRESHOLDS_ISSUE_ID: &str = "VCAD-PARITY-134";
pub const PERFORMANCE_SCORECARD_THRESHOLDS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/performance_scorecard_thresholds_vcad_reference.json";
const PERFORMANCE_SCORECARD_THRESHOLDS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/performance_scorecard_thresholds_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_performance_scorecard_thresholds_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_PERFORMANCE_SCORECARD_THRESHOLDS_ISSUE_ID,
        PERFORMANCE_SCORECARD_THRESHOLDS_REFERENCE_FIXTURE_PATH,
        PERFORMANCE_SCORECARD_THRESHOLDS_REFERENCE_FIXTURE_JSON,
        snapshot.contracts.clone(),
        snapshot,
        replay_snapshot,
    )
}

fn collect_snapshot() -> ReferenceTableSnapshot {
    ReferenceTableSnapshot::new(
        vec![
            json!({
                "case_id": "capability_scope",
                "issue_id": "VCAD-PARITY-134",
                "capability": "CAD Performance Scorecard Thresholds Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase K - Hardening + parity signoff",
                "lane_label": "performance-scorecard-thresholds"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/benches + ~/code/vcad/crates/vcad-perf",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Performance Scorecard Thresholds Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase K - Hardening + parity signoff parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include performance_scorecard_thresholds manifest and vcad fixture evidence".to_string(),
        ],
    )
}
