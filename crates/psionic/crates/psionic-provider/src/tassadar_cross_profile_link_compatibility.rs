use serde::{Deserialize, Serialize};

use psionic_eval::TassadarCrossProfileLinkEvalReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkCompatibilityReceipt {
    pub report_id: String,
    pub routeable_case_count: u32,
    pub downgraded_case_count: u32,
    pub refused_case_count: u32,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarCrossProfileLinkCompatibilityReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarCrossProfileLinkEvalReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            routeable_case_count: report.routeable_case_ids.len() as u32,
            downgraded_case_count: report.downgraded_case_ids.len() as u32,
            refused_case_count: report.refused_case_ids.len() as u32,
            served_publication_allowed: report.served_publication_allowed,
            detail: format!(
                "cross-profile link compatibility report `{}` keeps routeable_cases={}, downgraded_cases={}, refused_cases={}, served_publication_allowed={}",
                report.report_id,
                report.routeable_case_ids.len(),
                report.downgraded_case_ids.len(),
                report.refused_case_ids.len(),
                report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarCrossProfileLinkCompatibilityReceipt;
    use psionic_eval::build_tassadar_cross_profile_link_eval_report;

    #[test]
    fn cross_profile_link_compatibility_receipt_projects_report() {
        let report = build_tassadar_cross_profile_link_eval_report().expect("report");
        let receipt = TassadarCrossProfileLinkCompatibilityReceipt::from_report(&report);

        assert_eq!(receipt.routeable_case_count, 2);
        assert_eq!(receipt.downgraded_case_count, 1);
        assert_eq!(receipt.refused_case_count, 2);
        assert!(!receipt.served_publication_allowed);
    }
}
