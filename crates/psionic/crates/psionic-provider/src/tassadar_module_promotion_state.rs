use serde::{Deserialize, Serialize};

use psionic_eval::TassadarModulePromotionStateReport;

/// Provider-facing receipt for the eval-owned module promotion state report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModulePromotionStateReceipt {
    pub report_id: String,
    pub active_promoted_count: u32,
    pub challenge_open_count: u32,
    pub quarantined_count: u32,
    pub revoked_count: u32,
    pub superseded_count: u32,
    pub evidence_minimum_benchmark_ref_count: u32,
    pub detail: String,
}

impl TassadarModulePromotionStateReceipt {
    /// Builds a provider-facing receipt from the eval report.
    #[must_use]
    pub fn from_report(report: &TassadarModulePromotionStateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            active_promoted_count: report.active_promoted_count,
            challenge_open_count: report.challenge_open_count,
            quarantined_count: report.quarantined_count,
            revoked_count: report.revoked_count,
            superseded_count: report.superseded_count,
            evidence_minimum_benchmark_ref_count: report
                .evidence_minimums
                .minimum_benchmark_ref_count,
            detail: format!(
                "module-promotion report `{}` exposes active={}, challenge_open={}, quarantined={}, revoked={}, superseded={}, with minimum_benchmark_refs={}",
                report.report_id,
                report.active_promoted_count,
                report.challenge_open_count,
                report.quarantined_count,
                report.revoked_count,
                report.superseded_count,
                report.evidence_minimums.minimum_benchmark_ref_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModulePromotionStateReceipt;
    use psionic_eval::build_tassadar_module_promotion_state_report;

    #[test]
    fn module_promotion_state_receipt_projects_eval_report() {
        let report = build_tassadar_module_promotion_state_report().expect("report");
        let receipt = TassadarModulePromotionStateReceipt::from_report(&report);

        assert_eq!(receipt.active_promoted_count, 1);
        assert_eq!(receipt.challenge_open_count, 1);
        assert_eq!(receipt.quarantined_count, 1);
        assert_eq!(receipt.revoked_count, 1);
        assert_eq!(receipt.superseded_count, 1);
    }
}
