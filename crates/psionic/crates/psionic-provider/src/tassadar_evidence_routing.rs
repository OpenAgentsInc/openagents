use serde::{Deserialize, Serialize};

use psionic_router::TassadarEvidenceCalibratedRoutingReport;

/// Provider-facing receipt for the current evidence-calibrated routing report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEvidenceCalibratedRoutingReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Count of evaluated mount-scoped cases.
    pub case_count: u32,
    /// Share of capability-only winners that would violate the mount.
    pub capability_only_mount_violation_rate_bps: u32,
    /// Share of evidence-aware winners that satisfy the mount.
    pub evidence_aware_policy_compliance_rate_bps: u32,
    /// Share of cases where evidence-aware routing avoided a capability-only misroute.
    pub misroute_avoidance_rate_bps: u32,
    /// Count of accepted-outcome-required cases.
    pub accepted_outcome_required_case_count: u32,
    /// Share of accepted-outcome-required cases selecting an accepted-outcome-ready route.
    pub accepted_outcome_ready_selection_rate_bps: u32,
    /// Count of validator-required cases.
    pub validator_required_case_count: u32,
    /// Share of validator-required cases selecting a validator-attached route.
    pub validator_requirement_satisfaction_rate_bps: u32,
    /// Average evidence burden for evidence-aware selections.
    pub average_selected_evidence_burden_bps: u32,
    /// Average cost for evidence-aware selections.
    pub average_selected_cost_milliunits: u32,
    /// Count of validation refs grounding the receipt.
    pub validation_ref_count: u32,
    /// Stable report digest.
    pub report_digest: String,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarEvidenceCalibratedRoutingReceipt {
    /// Builds a provider-facing receipt from the shared evidence-aware routing report.
    #[must_use]
    pub fn from_report(report: &TassadarEvidenceCalibratedRoutingReport) -> Self {
        let accepted_outcome_required_case_count = report
            .evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.accepted_outcome_required)
            .count() as u32;
        let validator_required_case_count = report
            .evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.validator_required)
            .count() as u32;
        Self {
            report_id: report.report_id.clone(),
            case_count: report.evaluated_cases.len() as u32,
            capability_only_mount_violation_rate_bps: report
                .capability_only_mount_violation_rate_bps,
            evidence_aware_policy_compliance_rate_bps: report
                .evidence_aware_policy_compliance_rate_bps,
            misroute_avoidance_rate_bps: report.misroute_avoidance_rate_bps,
            accepted_outcome_required_case_count,
            accepted_outcome_ready_selection_rate_bps: report
                .accepted_outcome_ready_selection_rate_bps,
            validator_required_case_count,
            validator_requirement_satisfaction_rate_bps: report
                .validator_requirement_satisfaction_rate_bps,
            average_selected_evidence_burden_bps: report.average_selected_evidence_burden_bps,
            average_selected_cost_milliunits: report.average_selected_cost_milliunits,
            validation_ref_count: report.generated_from_refs.len() as u32,
            report_digest: report.report_digest.clone(),
            detail: format!(
                "evidence-aware routing `{}` covers {} mount-scoped cases with capability_only_mount_violations_bps={}, policy_compliance_bps={}, misroute_avoidance_bps={}, accepted_outcome_ready_bps={}, and validator_satisfaction_bps={}",
                report.report_id,
                report.evaluated_cases.len(),
                report.capability_only_mount_violation_rate_bps,
                report.evidence_aware_policy_compliance_rate_bps,
                report.misroute_avoidance_rate_bps,
                report.accepted_outcome_ready_selection_rate_bps,
                report.validator_requirement_satisfaction_rate_bps,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarEvidenceCalibratedRoutingReceipt;
    use psionic_router::build_tassadar_evidence_calibrated_routing_report;

    #[test]
    fn evidence_calibrated_routing_receipt_projects_router_report() {
        let report = build_tassadar_evidence_calibrated_routing_report()
            .expect("evidence-calibrated routing report");
        let receipt = TassadarEvidenceCalibratedRoutingReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 6);
        assert_eq!(receipt.accepted_outcome_required_case_count, 2);
        assert_eq!(receipt.validator_required_case_count, 3);
        assert!(receipt.capability_only_mount_violation_rate_bps >= 4_000);
        assert_eq!(receipt.evidence_aware_policy_compliance_rate_bps, 10_000);
        assert_eq!(receipt.validation_ref_count, 3);
    }
}
