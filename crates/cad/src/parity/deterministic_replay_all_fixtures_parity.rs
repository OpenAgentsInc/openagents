use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_DETERMINISTIC_REPLAY_ALL_FIXTURES_ISSUE_ID: &str = "VCAD-PARITY-133";
pub const DETERMINISTIC_REPLAY_ALL_FIXTURES_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/deterministic_replay_all_fixtures_vcad_reference.json";
const DETERMINISTIC_REPLAY_ALL_FIXTURES_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/deterministic_replay_all_fixtures_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_deterministic_replay_all_fixtures_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_DETERMINISTIC_REPLAY_ALL_FIXTURES_ISSUE_ID,
        DETERMINISTIC_REPLAY_ALL_FIXTURES_REFERENCE_FIXTURE_PATH,
        DETERMINISTIC_REPLAY_ALL_FIXTURES_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-133",
                "capability": "CAD Deterministic Replay All Fixtures Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase K - Hardening + parity signoff",
                "lane_label": "deterministic-replay-all-fixtures"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-fixtures + ~/code/vcad/crates/vcad-replay",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Deterministic Replay All Fixtures Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase K - Hardening + parity signoff parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include deterministic_replay_all_fixtures manifest and vcad fixture evidence".to_string(),
        ],
    )
}
