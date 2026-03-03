use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SIMULATION_STEP_RESET_API_ISSUE_ID: &str = "VCAD-PARITY-109";
pub const SIMULATION_STEP_RESET_API_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/simulation_step_reset_api_vcad_reference.json";
const SIMULATION_STEP_RESET_API_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/simulation_step_reset_api_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_simulation_step_reset_api_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_SIMULATION_STEP_RESET_API_ISSUE_ID,
        SIMULATION_STEP_RESET_API_REFERENCE_FIXTURE_PATH,
        SIMULATION_STEP_RESET_API_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-109",
                "capability": "CAD Simulation Step Reset API Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "simulation-step-reset-api"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-kernel-physics/src/simulation.rs + ~/code/vcad/crates/vcad-kernel-physics/src/api.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Simulation Step Reset API Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include simulation_step_reset_api manifest and vcad fixture evidence".to_string(),
        ],
    )
}
