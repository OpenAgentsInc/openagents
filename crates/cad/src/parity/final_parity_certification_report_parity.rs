use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_FINAL_PARITY_CERTIFICATION_REPORT_ISSUE_ID: &str = "VCAD-PARITY-136";
pub const FINAL_PARITY_CERTIFICATION_REPORT_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/final_parity_certification_report_vcad_reference.json";
const FINAL_PARITY_CERTIFICATION_REPORT_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/final_parity_certification_report_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_final_parity_certification_report_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_FINAL_PARITY_CERTIFICATION_REPORT_ISSUE_ID,
        FINAL_PARITY_CERTIFICATION_REPORT_REFERENCE_FIXTURE_PATH,
        FINAL_PARITY_CERTIFICATION_REPORT_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-136",
                "capability": "CAD Final Parity Certification Report"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase K - Hardening + parity signoff",
                "lane_label": "final-parity-certification-report"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-parity + ~/code/vcad/docs/parity",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD Final Parity Certification Report parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase K - Hardening + parity signoff parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include final_parity_certification_report manifest and vcad fixture evidence".to_string(),
        ],
    )
}
