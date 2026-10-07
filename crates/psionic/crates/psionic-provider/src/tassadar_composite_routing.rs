use serde::{Deserialize, Serialize};

use psionic_router::TassadarCompositeRoutingReport;

/// Provider-facing receipt for the heterogeneous composite-routing report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeRoutingReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub composite_preferred_case_count: u32,
    pub single_lane_preferred_case_count: u32,
    pub fallback_case_count: u32,
    pub challenge_path_case_count: u32,
    pub composite_evidence_lift_bps_vs_best_single_lane: i32,
    pub composite_cost_delta_milliunits_vs_best_single_lane: i32,
    pub composite_latency_delta_ms_vs_best_single_lane: i32,
    pub generated_from_ref_count: u32,
    pub detail: String,
}

impl TassadarCompositeRoutingReceipt {
    /// Builds a provider-facing receipt from the router report.
    #[must_use]
    pub fn from_report(report: &TassadarCompositeRoutingReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.evaluated_cases.len() as u32,
            composite_preferred_case_count: report.composite_preferred_case_count,
            single_lane_preferred_case_count: report.single_lane_preferred_case_count,
            fallback_case_count: report.fallback_case_count,
            challenge_path_case_count: report.challenge_path_case_count,
            composite_evidence_lift_bps_vs_best_single_lane: report
                .composite_evidence_lift_bps_vs_best_single_lane,
            composite_cost_delta_milliunits_vs_best_single_lane: report
                .composite_cost_delta_milliunits_vs_best_single_lane,
            composite_latency_delta_ms_vs_best_single_lane: report
                .composite_latency_delta_ms_vs_best_single_lane,
            generated_from_ref_count: report.generated_from_refs.len() as u32,
            detail: format!(
                "composite routing `{}` covers {} cases with composite_preferred={}, single_lane_preferred={}, fallback_cases={}, challenge_path_cases={}, evidence_lift_bps={}, cost_delta_milliunits={}, latency_delta_ms={}",
                report.report_id,
                report.evaluated_cases.len(),
                report.composite_preferred_case_count,
                report.single_lane_preferred_case_count,
                report.fallback_case_count,
                report.challenge_path_case_count,
                report.composite_evidence_lift_bps_vs_best_single_lane,
                report.composite_cost_delta_milliunits_vs_best_single_lane,
                report.composite_latency_delta_ms_vs_best_single_lane,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarCompositeRoutingReceipt;
    use psionic_router::build_tassadar_composite_routing_report;

    #[test]
    fn composite_routing_receipt_projects_router_report() {
        let report = build_tassadar_composite_routing_report();
        let receipt = TassadarCompositeRoutingReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 4);
        assert_eq!(receipt.composite_preferred_case_count, 3);
        assert_eq!(receipt.single_lane_preferred_case_count, 1);
        assert_eq!(receipt.fallback_case_count, 1);
        assert_eq!(receipt.generated_from_ref_count, 4);
        assert!(receipt.composite_evidence_lift_bps_vs_best_single_lane > 0);
    }
}
