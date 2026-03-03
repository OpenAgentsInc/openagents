use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SIMULATION_UI_CONTROLS_ISSUE_ID: &str = "VCAD-PARITY-110";
pub const SIMULATION_UI_CONTROLS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/simulation_ui_controls_vcad_reference.json";
const SIMULATION_UI_CONTROLS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/simulation_ui_controls_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_simulation_ui_controls_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_SIMULATION_UI_CONTROLS_ISSUE_ID,
        SIMULATION_UI_CONTROLS_REFERENCE_FIXTURE_PATH,
        SIMULATION_UI_CONTROLS_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-110",
                "capability": "CAD Simulation UI Controls Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "simulation-ui-controls"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/apps/vcad-desktop/src/simulation_controls.rs + ~/code/vcad/crates/vcad-sim-ui/src/lib.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Simulation UI Controls Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include simulation_ui_controls manifest and vcad fixture evidence".to_string(),
        ],
    )
}
