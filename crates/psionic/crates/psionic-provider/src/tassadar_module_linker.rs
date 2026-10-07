use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarModuleLinkRuntimeReport;

/// Provider-facing receipt for the bounded module-link runtime report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Number of exact link cases.
    pub exact_case_count: u32,
    /// Number of rollback link cases.
    pub rollback_case_count: u32,
    /// Number of refused link cases.
    pub refused_case_count: u32,
    /// Total dependency edges preserved across exact and rollback cases.
    pub dependency_edge_count: u32,
    /// Number of exact or rolled-back parity-preserving cases.
    pub parity_case_count: u32,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarModuleLinkReceipt {
    /// Projects a provider-facing receipt from the shared runtime report.
    #[must_use]
    pub fn from_runtime_report(report: &TassadarModuleLinkRuntimeReport) -> Self {
        let parity_case_count = report
            .case_reports
            .iter()
            .filter(|case| case.exact_outputs_preserved && case.exact_trace_match)
            .count() as u32;
        Self {
            report_id: report.report_id.clone(),
            exact_case_count: report.exact_case_count,
            rollback_case_count: report.rollback_case_count,
            refused_case_count: report.refused_case_count,
            dependency_edge_count: report.dependency_edge_count,
            parity_case_count,
            detail: format!(
                "module-link report `{}` currently exposes {} exact cases, {} rollback cases, {} refused cases, and {} parity-preserving linked-program witnesses",
                report.report_id,
                report.exact_case_count,
                report.rollback_case_count,
                report.refused_case_count,
                parity_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleLinkReceipt;
    use psionic_runtime::build_tassadar_module_link_runtime_report;

    #[test]
    fn module_link_receipt_projects_runtime_report() {
        let report = build_tassadar_module_link_runtime_report();
        let receipt = TassadarModuleLinkReceipt::from_runtime_report(&report);

        assert_eq!(receipt.exact_case_count, 1);
        assert_eq!(receipt.rollback_case_count, 1);
        assert_eq!(receipt.refused_case_count, 1);
        assert_eq!(receipt.dependency_edge_count, 1);
        assert_eq!(receipt.parity_case_count, 2);
    }
}
