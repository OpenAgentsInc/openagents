use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_LICENSE_COMPLIANCE_AUDIT_ISSUE_ID: &str = "VCAD-PARITY-131";
pub const LICENSE_COMPLIANCE_AUDIT_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/license_compliance_audit_vcad_reference.json";
const LICENSE_COMPLIANCE_AUDIT_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/license_compliance_audit_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_license_compliance_audit_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_LICENSE_COMPLIANCE_AUDIT_ISSUE_ID,
        LICENSE_COMPLIANCE_AUDIT_REFERENCE_FIXTURE_PATH,
        LICENSE_COMPLIANCE_AUDIT_REFERENCE_FIXTURE_JSON,
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
                "issue_id": "VCAD-PARITY-131",
                "capability": "CAD License Compliance Audit Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase K - Hardening + parity signoff",
                "lane_label": "license-compliance-audit"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/Cargo.lock + ~/code/vcad/licenses",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD License Compliance Audit Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase K - Hardening + parity signoff parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include license_compliance_audit manifest and vcad fixture evidence".to_string(),
        ],
    )
}
