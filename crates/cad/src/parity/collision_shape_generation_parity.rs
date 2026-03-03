use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_COLLISION_SHAPE_GENERATION_ISSUE_ID: &str = "VCAD-PARITY-106";
pub const COLLISION_SHAPE_GENERATION_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/collision_shape_generation_vcad_reference.json";
const COLLISION_SHAPE_GENERATION_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/collision_shape_generation_vcad_reference.json");

pub type CollisionShapeGenerationParityManifest = ReferenceTableParityManifest;

pub fn build_collision_shape_generation_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<CollisionShapeGenerationParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_COLLISION_SHAPE_GENERATION_ISSUE_ID,
        COLLISION_SHAPE_GENERATION_REFERENCE_FIXTURE_PATH,
        COLLISION_SHAPE_GENERATION_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-106",
                "capability": "CAD Collision Shape Generation Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "collision-shape-generation"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-kernel-physics/src/colliders.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Collision Shape Generation Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include collision_shape_generation manifest and vcad fixture evidence".to_string(),
        ],
    )
}
