use serde::{Deserialize, Serialize};

use psionic_eval::TassadarCounterfactualRouteQualityEvalReport;

/// Provider-facing receipt for the counterfactual route-quality eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteQualityReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub better_counterfactual_case_count: u32,
    pub accepted_outcome_better_alternative_case_count: u32,
    pub overuse_case_count: u32,
    pub underuse_case_count: u32,
    pub detail: String,
}

impl TassadarCounterfactualRouteQualityReceipt {
    /// Builds a provider-facing receipt from the eval report.
    #[must_use]
    pub fn from_report(report: &TassadarCounterfactualRouteQualityEvalReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.eval_rows.len() as u32,
            better_counterfactual_case_count: report.better_counterfactual_case_count,
            accepted_outcome_better_alternative_case_count: report
                .accepted_outcome_better_alternative_case_count,
            overuse_case_count: report.overuse_case_count,
            underuse_case_count: report.underuse_case_count,
            detail: format!(
                "counterfactual route-quality `{}` covers {} cases with better_counterfactuals={}, accepted_outcome_better_alternatives={}, overuse_cases={}, underuse_cases={}",
                report.report_id,
                report.eval_rows.len(),
                report.better_counterfactual_case_count,
                report.accepted_outcome_better_alternative_case_count,
                report.overuse_case_count,
                report.underuse_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarCounterfactualRouteQualityReceipt;
    use psionic_eval::build_tassadar_counterfactual_route_quality_eval_report;

    #[test]
    fn counterfactual_route_quality_receipt_projects_eval_report() {
        let report = build_tassadar_counterfactual_route_quality_eval_report();
        let receipt = TassadarCounterfactualRouteQualityReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 4);
        assert_eq!(receipt.better_counterfactual_case_count, 3);
        assert_eq!(receipt.accepted_outcome_better_alternative_case_count, 2);
        assert_eq!(receipt.overuse_case_count, 1);
        assert_eq!(receipt.underuse_case_count, 1);
    }
}
