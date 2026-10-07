use serde::{Deserialize, Serialize};

use psionic_eval::TassadarLinkedProgramBundleEvalReport;
use psionic_runtime::TassadarLinkedProgramBundlePosture;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleReceipt {
    pub report_id: String,
    pub exact_case_count: u32,
    pub rollback_case_count: u32,
    pub refused_case_count: u32,
    pub shared_state_case_count: u32,
    pub start_order_exact_case_count: u32,
    pub graph_valid_case_count: u32,
    pub exact_bundle_ids: Vec<String>,
    pub rollback_bundle_ids: Vec<String>,
    pub refused_bundle_ids: Vec<String>,
    pub detail: String,
}

impl TassadarLinkedProgramBundleReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarLinkedProgramBundleEvalReport) -> Self {
        let exact_bundle_ids = report
            .runtime_report
            .case_reports
            .iter()
            .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Exact)
            .map(|case| case.bundle_descriptor.bundle_id.clone())
            .collect::<Vec<_>>();
        let rollback_bundle_ids = report
            .runtime_report
            .case_reports
            .iter()
            .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::RolledBack)
            .map(|case| case.bundle_descriptor.bundle_id.clone())
            .collect::<Vec<_>>();
        let refused_bundle_ids = report
            .runtime_report
            .case_reports
            .iter()
            .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Refused)
            .map(|case| case.bundle_descriptor.bundle_id.clone())
            .collect::<Vec<_>>();
        Self {
            report_id: report.report_id.clone(),
            exact_case_count: report.exact_case_count,
            rollback_case_count: report.rollback_case_count,
            refused_case_count: report.refused_case_count,
            shared_state_case_count: report.shared_state_case_count,
            start_order_exact_case_count: report.start_order_exact_case_count,
            graph_valid_case_count: report.graph_valid_case_count,
            exact_bundle_ids,
            rollback_bundle_ids,
            refused_bundle_ids,
            detail: format!(
                "linked-program bundle receipt `{}` currently exposes {} exact bundles, {} rollback bundles, {} refused bundles, {} shared-state cases, {} graph-valid cases, and {} start-order-exact cases",
                report.report_id,
                report.exact_case_count,
                report.rollback_case_count,
                report.refused_case_count,
                report.shared_state_case_count,
                report.graph_valid_case_count,
                report.start_order_exact_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarLinkedProgramBundleReceipt;
    use psionic_eval::build_tassadar_linked_program_bundle_eval_report;

    #[test]
    fn linked_program_bundle_receipt_projects_eval_report() {
        let report = build_tassadar_linked_program_bundle_eval_report();
        let receipt = TassadarLinkedProgramBundleReceipt::from_report(&report);

        assert_eq!(receipt.exact_case_count, 2);
        assert_eq!(receipt.rollback_case_count, 1);
        assert_eq!(receipt.refused_case_count, 1);
        assert_eq!(receipt.shared_state_case_count, 2);
        assert_eq!(receipt.graph_valid_case_count, 3);
        assert_eq!(receipt.start_order_exact_case_count, 3);
        assert!(receipt.rollback_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.parser_allocator_rollback.v1"
        )));
    }
}
