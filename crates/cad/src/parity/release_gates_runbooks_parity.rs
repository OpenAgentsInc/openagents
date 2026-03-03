use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_RELEASE_GATES_RUNBOOKS_ISSUE_ID: &str = "VCAD-PARITY-135";
pub const RELEASE_GATES_RUNBOOKS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/release_gates_runbooks_vcad_reference.json";
const RELEASE_GATES_RUNBOOKS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/release_gates_runbooks_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_release_gates_runbooks_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_RELEASE_GATES_RUNBOOKS_ISSUE_ID,
        RELEASE_GATES_RUNBOOKS_REFERENCE_FIXTURE_PATH,
        RELEASE_GATES_RUNBOOKS_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-135",
                "capability": "CAD Release Gates Runbooks Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase K - Hardening + parity signoff",
                "lane_label": "release-gates-runbooks"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/.github/workflows + ~/code/vcad/docs/runbooks",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Release Gates Runbooks Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase K - Hardening + parity signoff parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include release_gates_runbooks manifest and vcad fixture evidence".to_string(),
        ],
    )
}
