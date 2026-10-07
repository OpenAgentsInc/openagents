use serde::{Deserialize, Serialize};

use psionic_research::TassadarBroadFamilySpecializationSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationReceipt {
    pub report_id: String,
    pub promotion_ready_family_count: u32,
    pub benchmark_only_family_count: u32,
    pub refused_family_count: u32,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarBroadFamilySpecializationReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarBroadFamilySpecializationSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            promotion_ready_family_count: summary.promotion_ready_family_ids.len() as u32,
            benchmark_only_family_count: summary.benchmark_only_family_ids.len() as u32,
            refused_family_count: summary.refused_family_ids.len() as u32,
            served_publication_allowed: summary.eval_report.served_publication_allowed,
            detail: format!(
                "broad-family specialization summary `{}` keeps promotion_ready_families={}, benchmark_only_families={}, refused_families={}, served_publication_allowed={}",
                summary.report_id,
                summary.promotion_ready_family_ids.len(),
                summary.benchmark_only_family_ids.len(),
                summary.refused_family_ids.len(),
                summary.eval_report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarBroadFamilySpecializationReceipt;
    use psionic_research::build_tassadar_broad_family_specialization_summary;

    #[test]
    fn broad_family_specialization_receipt_projects_summary() {
        let summary = build_tassadar_broad_family_specialization_summary().expect("summary");
        let receipt = TassadarBroadFamilySpecializationReceipt::from_summary(&summary);

        assert_eq!(receipt.promotion_ready_family_count, 1);
        assert_eq!(receipt.benchmark_only_family_count, 2);
        assert_eq!(receipt.refused_family_count, 1);
        assert!(!receipt.served_publication_allowed);
    }
}
