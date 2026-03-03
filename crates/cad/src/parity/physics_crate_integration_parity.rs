use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_PHYSICS_CRATE_INTEGRATION_ISSUE_ID: &str = "VCAD-PARITY-105";
pub const PHYSICS_CRATE_INTEGRATION_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/physics_crate_integration_vcad_reference.json";
const PHYSICS_CRATE_INTEGRATION_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/physics_crate_integration_vcad_reference.json");

pub type PhysicsCrateIntegrationParityManifest = ReferenceTableParityManifest;

pub fn build_physics_crate_integration_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<PhysicsCrateIntegrationParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_PHYSICS_CRATE_INTEGRATION_ISSUE_ID,
        PHYSICS_CRATE_INTEGRATION_REFERENCE_FIXTURE_PATH,
        PHYSICS_CRATE_INTEGRATION_REFERENCE_FIXTURE_JSON,
        snapshot.contracts.clone(),
        snapshot,
        replay_snapshot,
    )
}

fn collect_snapshot() -> ReferenceTableSnapshot {
    ReferenceTableSnapshot::new(
        vec![
            json!({
                "case_id": "physics_crate_exports",
                "crate": "vcad-kernel-physics",
                "exports": ["PhysicsWorld", "JointState", "Action", "Observation", "RobotEnv"]
            }),
            json!({
                "case_id": "integration_lane",
                "simulation_crate": "vcad-sim",
                "single_pipeline": "SimPipeline",
                "batch_pipeline": "BatchSimPipeline"
            }),
            json!({
                "case_id": "engine_backend",
                "backend": "phyz",
                "gravity_mps2": [0.0, 0.0, -9.81],
                "default_dt_seconds": 0.004166667
            }),
        ],
        vec![
            "physics crate integration parity uses vcad-kernel-physics as the reference authority"
                .to_string(),
            "simulation integration parity keeps vcad-sim single and batch pipeline contracts"
                .to_string(),
            "physics integration parity artifacts are deterministic across replay".to_string(),
        ],
    )
}
