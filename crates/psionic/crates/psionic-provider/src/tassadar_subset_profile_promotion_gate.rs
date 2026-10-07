use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSubsetProfilePromotionGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSubsetProfilePromotionGateReceipt {
    pub report_id: String,
    pub green_profile_ids: Vec<String>,
    pub served_publication_allowed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarSubsetProfilePromotionGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSubsetProfilePromotionGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_profile_ids: report.green_profile_ids.clone(),
            served_publication_allowed_profile_ids: report
                .served_publication_allowed_profile_ids
                .clone(),
            failed_profile_ids: report.failed_profile_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "subset profile promotion gate `{}` keeps green_profiles={}, served_publication_allowed_profiles={}, failed_profiles={}, overall_green={}",
                report.report_id,
                report.green_profile_ids.len(),
                report.served_publication_allowed_profile_ids.len(),
                report.failed_profile_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSubsetProfilePromotionGateReceipt;
    use psionic_eval::build_tassadar_subset_profile_promotion_gate_report;

    #[test]
    fn subset_profile_promotion_gate_receipt_projects_report() {
        let report = build_tassadar_subset_profile_promotion_gate_report().expect("report");
        let receipt = TassadarSubsetProfilePromotionGateReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert!(receipt.green_profile_ids.contains(&String::from(
            "tassadar.internal_compute.deterministic_import_subset.v1"
        )));
        assert!(receipt.green_profile_ids.contains(&String::from(
            "tassadar.internal_compute.runtime_support_subset.v1"
        )));
        assert!(receipt.served_publication_allowed_profile_ids.is_empty());
    }
}
