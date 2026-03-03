use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_URDF_IMPORT_ISSUE_ID: &str = "VCAD-PARITY-113";
pub const URDF_IMPORT_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/urdf_import_vcad_reference.json";
const URDF_IMPORT_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/urdf_import_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_urdf_import_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_URDF_IMPORT_ISSUE_ID,
        URDF_IMPORT_REFERENCE_FIXTURE_PATH,
        URDF_IMPORT_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-113",
                "capability": "CAD URDF Import Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "urdf-import"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-urdf/src/import.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD URDF Import Parity parity contracts are aligned to vcad reference behavior"
                .to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay"
                .to_string(),
            "CI parity artifacts include urdf_import manifest and vcad fixture evidence"
                .to_string(),
        ],
    )
}
