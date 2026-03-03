use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_GYM_STYLE_API_ISSUE_ID: &str = "VCAD-PARITY-111";
pub const GYM_STYLE_API_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/gym_style_api_vcad_reference.json";
const GYM_STYLE_API_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/gym_style_api_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_gym_style_api_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_GYM_STYLE_API_ISSUE_ID,
        GYM_STYLE_API_REFERENCE_FIXTURE_PATH,
        GYM_STYLE_API_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-111",
                "capability": "CAD Gym Style API Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "gym-style-api"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-gym/src/lib.rs + ~/code/vcad/crates/vcad-gym/src/env.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Gym Style API Parity parity contracts are aligned to vcad reference behavior"
                .to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay"
                .to_string(),
            "CI parity artifacts include gym_style_api manifest and vcad fixture evidence"
                .to_string(),
        ],
    )
}
