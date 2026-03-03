use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_JOINT_PHYSICS_MAPPING_ISSUE_ID: &str = "VCAD-PARITY-108";
pub const JOINT_PHYSICS_MAPPING_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/joint_physics_mapping_vcad_reference.json";
const JOINT_PHYSICS_MAPPING_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/joint_physics_mapping_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_joint_physics_mapping_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_JOINT_PHYSICS_MAPPING_ISSUE_ID,
        JOINT_PHYSICS_MAPPING_REFERENCE_FIXTURE_PATH,
        JOINT_PHYSICS_MAPPING_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-108",
                "capability": "CAD Joint Physics Mapping Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "joint-physics-mapping"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-kernel-physics/src/joints.rs + ~/code/vcad/crates/vcad-kernel-physics/src/world.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Joint Physics Mapping Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include joint_physics_mapping manifest and vcad fixture evidence".to_string(),
        ],
    )
}
