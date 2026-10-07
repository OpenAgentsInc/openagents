use serde::{Deserialize, Serialize};

use psionic_eval::TassadarWedgeTaxonomyReport;

/// Provider-facing receipt for the property-first wedge taxonomy report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWedgeTaxonomyReceipt {
    pub report_id: String,
    pub suite_count: u32,
    pub average_validator_attachment_rate_bps: u32,
    pub average_evidence_completeness_bps: u32,
    pub fallback_comparison_suite_count: u32,
    pub high_exact_compute_benefit_suite_count: u32,
    pub generated_from_ref_count: u32,
    pub detail: String,
}

impl TassadarWedgeTaxonomyReceipt {
    /// Builds a provider-facing receipt from the eval report.
    #[must_use]
    pub fn from_report(report: &TassadarWedgeTaxonomyReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            suite_count: report.evaluated_suites.len() as u32,
            average_validator_attachment_rate_bps: report.average_validator_attachment_rate_bps,
            average_evidence_completeness_bps: report.average_evidence_completeness_bps,
            fallback_comparison_suite_count: report.fallback_comparison_suite_count,
            high_exact_compute_benefit_suite_count: report.high_exact_compute_benefit_suite_count,
            generated_from_ref_count: report.generated_from_refs.len() as u32,
            detail: format!(
                "wedge taxonomy `{}` covers {} suites with validator_attachment_bps={}, evidence_completeness_bps={}, fallback_suites={}, and high_exact_compute_benefit_suites={}",
                report.report_id,
                report.evaluated_suites.len(),
                report.average_validator_attachment_rate_bps,
                report.average_evidence_completeness_bps,
                report.fallback_comparison_suite_count,
                report.high_exact_compute_benefit_suite_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarWedgeTaxonomyReceipt;
    use psionic_eval::build_tassadar_wedge_taxonomy_report;

    #[test]
    fn wedge_taxonomy_receipt_projects_eval_report() {
        let report = build_tassadar_wedge_taxonomy_report();
        let receipt = TassadarWedgeTaxonomyReceipt::from_report(&report);

        assert_eq!(receipt.suite_count, 4);
        assert_eq!(receipt.fallback_comparison_suite_count, 2);
        assert_eq!(receipt.high_exact_compute_benefit_suite_count, 2);
        assert_eq!(receipt.generated_from_ref_count, 3);
    }
}
