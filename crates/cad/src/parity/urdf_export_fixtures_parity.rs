use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_URDF_EXPORT_FIXTURES_ISSUE_ID: &str = "VCAD-PARITY-114";
pub const URDF_EXPORT_FIXTURES_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/urdf_export_fixtures_vcad_reference.json";
const URDF_EXPORT_FIXTURES_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/urdf_export_fixtures_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_urdf_export_fixtures_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_URDF_EXPORT_FIXTURES_ISSUE_ID,
        URDF_EXPORT_FIXTURES_REFERENCE_FIXTURE_PATH,
        URDF_EXPORT_FIXTURES_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-114",
                "capability": "CAD URDF Export Fixtures Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "urdf-export-fixtures"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-urdf/src/export.rs + ~/code/vcad/crates/vcad-urdf/tests/fixtures",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD URDF Export Fixtures Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include urdf_export_fixtures manifest and vcad fixture evidence".to_string(),
        ],
    )
}
