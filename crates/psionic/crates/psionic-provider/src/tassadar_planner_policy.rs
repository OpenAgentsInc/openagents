use serde::{Deserialize, Serialize};

use psionic_models::TassadarPlannerRouteFamily;
use psionic_router::TassadarPlannerLanguageComputePolicyReport;

/// Provider-facing receipt for the current planner language-vs-compute policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerLanguageComputePolicyReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Count of benchmarked hybrid cases.
    pub case_count: u32,
    /// Number of cases routed language-only.
    pub language_only_case_count: u32,
    /// Number of cases routed to the internal exact-compute lane.
    pub internal_exact_compute_case_count: u32,
    /// Number of cases routed to an external tool lane.
    pub external_tool_case_count: u32,
    /// Lane-selection accuracy copied from the router report.
    pub lane_selection_accuracy_bps: u32,
    /// Policy-compliance rate copied from the router report.
    pub policy_compliance_rate_bps: u32,
    /// Average cost-per-correct-job copied from the router report.
    pub cost_per_correct_job_milliunits: u32,
    /// Share of cases where the executor lane was explicitly refused.
    pub executor_invocation_refusal_rate_bps: u32,
    /// Count of validation refs grounding the receipt.
    pub validation_ref_count: u32,
    /// Stable report digest.
    pub report_digest: String,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarPlannerLanguageComputePolicyReceipt {
    /// Builds a provider-facing receipt from the shared router report.
    #[must_use]
    pub fn from_report(report: &TassadarPlannerLanguageComputePolicyReport) -> Self {
        let mut language_only_case_count = 0u32;
        let mut internal_exact_compute_case_count = 0u32;
        let mut external_tool_case_count = 0u32;
        for case in &report.evaluated_cases {
            match case.selected_route_family {
                TassadarPlannerRouteFamily::LanguageOnly => language_only_case_count += 1,
                TassadarPlannerRouteFamily::InternalExactCompute => {
                    internal_exact_compute_case_count += 1
                }
                TassadarPlannerRouteFamily::ExternalTool => external_tool_case_count += 1,
            }
        }
        Self {
            report_id: report.report_id.clone(),
            publication_id: report.policy_publication.publication_id.clone(),
            case_count: report.evaluated_cases.len() as u32,
            language_only_case_count,
            internal_exact_compute_case_count,
            external_tool_case_count,
            lane_selection_accuracy_bps: report.lane_selection_accuracy_bps,
            policy_compliance_rate_bps: report.policy_compliance_rate_bps,
            cost_per_correct_job_milliunits: report.cost_per_correct_job_milliunits,
            executor_invocation_refusal_rate_bps: report.executor_invocation_refusal_rate_bps,
            validation_ref_count: report.generated_from_refs.len() as u32,
            report_digest: report.report_digest.clone(),
            detail: format!(
                "planner policy `{}` currently covers {} hybrid cases with route counts language_only={}, internal_exact_compute={}, external_tool={}, selection_accuracy_bps={}, and executor_invocation_refusal_rate_bps={}",
                report.report_id,
                report.evaluated_cases.len(),
                language_only_case_count,
                internal_exact_compute_case_count,
                external_tool_case_count,
                report.lane_selection_accuracy_bps,
                report.executor_invocation_refusal_rate_bps,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPlannerLanguageComputePolicyReceipt;
    use psionic_router::build_tassadar_planner_language_compute_policy_report;

    #[test]
    fn planner_language_compute_policy_receipt_projects_router_report() {
        let report =
            build_tassadar_planner_language_compute_policy_report().expect("planner report");
        let receipt = TassadarPlannerLanguageComputePolicyReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 6);
        assert_eq!(receipt.language_only_case_count, 2);
        assert_eq!(receipt.internal_exact_compute_case_count, 3);
        assert_eq!(receipt.external_tool_case_count, 1);
        assert!(receipt.lane_selection_accuracy_bps >= 9_000);
        assert!(receipt.validation_ref_count >= 3);
    }
}
