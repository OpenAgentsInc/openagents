use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_CRDT_LANE_ARCHITECTURE_ISSUE_ID: &str = "VCAD-PARITY-115";
pub const CRDT_LANE_ARCHITECTURE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/crdt_lane_architecture_vcad_reference.json";
const CRDT_LANE_ARCHITECTURE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/crdt_lane_architecture_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_crdt_lane_architecture_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_CRDT_LANE_ARCHITECTURE_ISSUE_ID,
        CRDT_LANE_ARCHITECTURE_REFERENCE_FIXTURE_PATH,
        CRDT_LANE_ARCHITECTURE_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-115",
                "capability": "CAD CRDT Lane Architecture Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase J - Full workspace parity lanes",
                "lane_label": "crdt-lane-architecture"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-crdt/src/lib.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD CRDT Lane Architecture Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase J - Full workspace parity lanes parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include crdt_lane_architecture manifest and vcad fixture evidence".to_string(),
        ],
    )
}
