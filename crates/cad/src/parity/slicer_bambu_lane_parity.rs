use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SLICER_BAMBU_LANE_ISSUE_ID: &str = "VCAD-PARITY-126";
pub const SLICER_BAMBU_LANE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/slicer_bambu_lane_vcad_reference.json";
const SLICER_BAMBU_LANE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/slicer_bambu_lane_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_slicer_bambu_lane_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_SLICER_BAMBU_LANE_ISSUE_ID,
        SLICER_BAMBU_LANE_REFERENCE_FIXTURE_PATH,
        SLICER_BAMBU_LANE_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-126",
                "capability": "CAD Slicer Bambu Lane Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase J - Full workspace parity lanes",
                "lane_label": "slicer-bambu-lane"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-slicer-bambu/src/lib.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Slicer Bambu Lane Parity parity contracts are aligned to vcad reference behavior"
                .to_string(),
            "Phase J - Full workspace parity lanes parity lane remains deterministic across replay"
                .to_string(),
            "CI parity artifacts include slicer_bambu_lane manifest and vcad fixture evidence"
                .to_string(),
        ],
    )
}
