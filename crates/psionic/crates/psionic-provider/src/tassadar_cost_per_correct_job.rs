use serde::{Deserialize, Serialize};

use psionic_eval::TassadarCostPerCorrectJobReport;

/// Provider-facing receipt for the benchmark-bound cost-per-correct report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCostPerCorrectJobReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub internal_lane_win_count: u32,
    pub external_lane_win_count: u32,
    pub hybrid_lane_win_count: u32,
    pub threshold_crossing_case_count: u32,
    pub average_internal_cost_per_correct_job_milliunits: u32,
    pub average_external_cost_per_correct_job_milliunits: u32,
    pub average_hybrid_cost_per_correct_job_milliunits: u32,
    pub generated_from_ref_count: u32,
    pub detail: String,
}

impl TassadarCostPerCorrectJobReceipt {
    /// Builds a provider-facing receipt from the eval report.
    #[must_use]
    pub fn from_report(report: &TassadarCostPerCorrectJobReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.evaluated_cases.len() as u32,
            internal_lane_win_count: report.internal_lane_win_count,
            external_lane_win_count: report.external_lane_win_count,
            hybrid_lane_win_count: report.hybrid_lane_win_count,
            threshold_crossing_case_count: report.threshold_crossing_case_count,
            average_internal_cost_per_correct_job_milliunits: report
                .average_internal_cost_per_correct_job_milliunits,
            average_external_cost_per_correct_job_milliunits: report
                .average_external_cost_per_correct_job_milliunits,
            average_hybrid_cost_per_correct_job_milliunits: report
                .average_hybrid_cost_per_correct_job_milliunits,
            generated_from_ref_count: report.generated_from_refs.len() as u32,
            detail: format!(
                "cost-per-correct report `{}` covers {} cases with internal_wins={}, hybrid_wins={}, external_wins={}, threshold_crossings={}, avg_internal_cpccj={}, avg_hybrid_cpccj={}, avg_external_cpccj={}",
                report.report_id,
                report.evaluated_cases.len(),
                report.internal_lane_win_count,
                report.hybrid_lane_win_count,
                report.external_lane_win_count,
                report.threshold_crossing_case_count,
                report.average_internal_cost_per_correct_job_milliunits,
                report.average_hybrid_cost_per_correct_job_milliunits,
                report.average_external_cost_per_correct_job_milliunits,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarCostPerCorrectJobReceipt;
    use psionic_eval::build_tassadar_cost_per_correct_job_report;

    #[test]
    fn cost_per_correct_job_receipt_projects_eval_report() {
        let report = build_tassadar_cost_per_correct_job_report();
        let receipt = TassadarCostPerCorrectJobReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 4);
        assert_eq!(receipt.internal_lane_win_count, 2);
        assert_eq!(receipt.hybrid_lane_win_count, 1);
        assert_eq!(receipt.external_lane_win_count, 1);
        assert_eq!(receipt.threshold_crossing_case_count, 2);
        assert_eq!(receipt.generated_from_ref_count, 4);
    }
}
