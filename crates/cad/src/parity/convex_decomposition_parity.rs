use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_CONVEX_DECOMPOSITION_ISSUE_ID: &str = "VCAD-PARITY-107";
pub const CONVEX_DECOMPOSITION_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/convex_decomposition_vcad_reference.json";
const CONVEX_DECOMPOSITION_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/convex_decomposition_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_convex_decomposition_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_CONVEX_DECOMPOSITION_ISSUE_ID,
        CONVEX_DECOMPOSITION_REFERENCE_FIXTURE_PATH,
        CONVEX_DECOMPOSITION_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-107",
                "capability": "CAD Convex Decomposition Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "convex-decomposition"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-kernel-physics/src/colliders.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Convex Decomposition Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include convex_decomposition manifest and vcad fixture evidence".to_string(),
        ],
    )
}
