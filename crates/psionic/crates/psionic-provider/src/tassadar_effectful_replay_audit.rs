use serde::{Deserialize, Serialize};

use psionic_eval::TassadarEffectfulReplayAuditReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectfulReplayAuditReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub challengeable_case_count: u32,
    pub refusal_case_count: u32,
    pub replay_safe_effect_family_ids: Vec<String>,
    pub refused_effect_family_ids: Vec<String>,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub detail: String,
}

impl TassadarEffectfulReplayAuditReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarEffectfulReplayAuditReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            challengeable_case_count: report.challengeable_case_count,
            refusal_case_count: report.refusal_case_count,
            replay_safe_effect_family_ids: report.replay_safe_effect_family_ids.clone(),
            refused_effect_family_ids: report.refused_effect_family_ids.clone(),
            kernel_policy_dependency_marker: report.kernel_policy_dependency_marker.clone(),
            nexus_dependency_marker: report.nexus_dependency_marker.clone(),
            detail: format!(
                "effectful replay audit report `{}` carries challengeable_cases={}, refusal_cases={}, replay_safe_families={}, refused_families={}",
                report.report_id,
                report.challengeable_case_count,
                report.refusal_case_count,
                report.replay_safe_effect_family_ids.len(),
                report.refused_effect_family_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarEffectfulReplayAuditReceipt;
    use psionic_eval::build_tassadar_effectful_replay_audit_report;

    #[test]
    fn effectful_replay_audit_receipt_projects_report() {
        let report = build_tassadar_effectful_replay_audit_report().expect("report");
        let receipt = TassadarEffectfulReplayAuditReceipt::from_report(&report);

        assert_eq!(
            receipt.profile_id,
            "tassadar.effect_profile.replay_challenge_receipts.v1"
        );
        assert_eq!(receipt.challengeable_case_count, 3);
        assert_eq!(receipt.refusal_case_count, 3);
        assert!(receipt
            .kernel_policy_dependency_marker
            .contains("kernel-policy"));
        assert!(receipt.nexus_dependency_marker.contains("nexus"));
    }
}
