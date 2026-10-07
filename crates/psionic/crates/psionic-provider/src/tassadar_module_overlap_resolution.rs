use serde::{Deserialize, Serialize};

use psionic_router::TassadarModuleOverlapResolutionReport;

/// Provider-facing receipt for the module-overlap resolution report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapResolutionReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Number of selected cases.
    pub selected_case_count: u32,
    /// Number of refused cases.
    pub refused_case_count: u32,
    /// Number of mount-override cases.
    pub mount_override_case_count: u32,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarModuleOverlapResolutionReceipt {
    /// Builds a provider-facing receipt from the router report.
    #[must_use]
    pub fn from_report(report: &TassadarModuleOverlapResolutionReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            selected_case_count: report.selected_case_count,
            refused_case_count: report.refused_case_count,
            mount_override_case_count: report.mount_override_case_count,
            detail: format!(
                "module-overlap report `{}` currently exposes {} selected cases, {} refused cases, and {} mount-override cases",
                report.report_id,
                report.selected_case_count,
                report.refused_case_count,
                report.mount_override_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleOverlapResolutionReceipt;
    use psionic_router::build_tassadar_module_overlap_resolution_report;

    #[test]
    fn module_overlap_resolution_receipt_projects_router_report() {
        let report = build_tassadar_module_overlap_resolution_report();
        let receipt = TassadarModuleOverlapResolutionReceipt::from_report(&report);

        assert_eq!(receipt.selected_case_count, 2);
        assert_eq!(receipt.refused_case_count, 1);
        assert_eq!(receipt.mount_override_case_count, 1);
    }
}
