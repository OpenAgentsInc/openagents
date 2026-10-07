use serde::{Deserialize, Serialize};

use psionic_research::TassadarSharedStateConcurrencySummaryReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyReceipt {
    pub report_id: String,
    pub operator_green_class_ids: Vec<String>,
    pub public_suppressed_profile_ids: Vec<String>,
    pub refused_class_ids: Vec<String>,
    pub detail: String,
}

impl TassadarSharedStateConcurrencyReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarSharedStateConcurrencySummaryReport) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            operator_green_class_ids: summary.operator_green_class_ids.clone(),
            public_suppressed_profile_ids: summary.public_suppressed_profile_ids.clone(),
            refused_class_ids: summary.refused_class_ids.clone(),
            detail: format!(
                "shared-state concurrency summary `{}` keeps operator_green_classes={}, public_profiles={}, refused_classes={}",
                summary.report_id,
                summary.operator_green_class_ids.len(),
                summary.public_suppressed_profile_ids.len(),
                summary.refused_class_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSharedStateConcurrencyReceipt;
    use psionic_research::build_tassadar_shared_state_concurrency_summary_report;

    #[test]
    fn shared_state_concurrency_receipt_projects_summary() {
        let summary = build_tassadar_shared_state_concurrency_summary_report().expect("summary");
        let receipt = TassadarSharedStateConcurrencyReceipt::from_summary(&summary);

        assert_eq!(receipt.operator_green_class_ids.len(), 2);
        assert_eq!(receipt.public_suppressed_profile_ids.len(), 1);
        assert_eq!(receipt.refused_class_ids.len(), 3);
    }
}
