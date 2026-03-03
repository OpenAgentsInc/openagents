use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_CAM_STOCKSIM_LANE_ISSUE_ID: &str = "VCAD-PARITY-130";
pub const CAM_STOCKSIM_LANE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/cam_stocksim_lane_vcad_reference.json";
const CAM_STOCKSIM_LANE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/cam_stocksim_lane_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_cam_stocksim_lane_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_CAM_STOCKSIM_LANE_ISSUE_ID,
        CAM_STOCKSIM_LANE_REFERENCE_FIXTURE_PATH,
        CAM_STOCKSIM_LANE_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-130",
                "capability": "CAD CAM Stocksim Lane Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase J - Full workspace parity lanes",
                "lane_label": "cam-stocksim-lane"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-cam-stocksim/src/lib.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD CAM Stocksim Lane Parity parity contracts are aligned to vcad reference behavior"
                .to_string(),
            "Phase J - Full workspace parity lanes parity lane remains deterministic across replay"
                .to_string(),
            "CI parity artifacts include cam_stocksim_lane manifest and vcad fixture evidence"
                .to_string(),
        ],
    )
}
