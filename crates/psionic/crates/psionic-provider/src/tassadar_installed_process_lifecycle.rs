use serde::{Deserialize, Serialize};

use psionic_eval::TassadarInstalledProcessLifecycleReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub portable_process_ids: Vec<String>,
    pub exact_migration_case_count: u32,
    pub exact_rollback_case_count: u32,
    pub refusal_case_count: u32,
    pub portability_envelope_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarInstalledProcessLifecycleReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarInstalledProcessLifecycleReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: String::from("tassadar.internal_compute.installed_process_lifecycle.v1"),
            portable_process_ids: report.portable_process_ids.clone(),
            exact_migration_case_count: report.exact_migration_case_count,
            exact_rollback_case_count: report.exact_rollback_case_count,
            refusal_case_count: report.refusal_case_count,
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            served_publication_allowed: report.served_publication_allowed,
            overall_green: report.overall_green,
            detail: format!(
                "installed-process lifecycle report `{}` keeps portable_processes={}, migration_cases={}, rollback_cases={}, refusal_rows={}, served_publication_allowed={}, overall_green={}",
                report.report_id,
                report.portable_process_ids.len(),
                report.exact_migration_case_count,
                report.exact_rollback_case_count,
                report.refusal_case_count,
                report.served_publication_allowed,
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInstalledProcessLifecycleReceipt;
    use psionic_eval::build_tassadar_installed_process_lifecycle_report;

    #[test]
    fn installed_process_lifecycle_receipt_projects_report() {
        let report = build_tassadar_installed_process_lifecycle_report().expect("report");
        let receipt = TassadarInstalledProcessLifecycleReceipt::from_report(&report);

        assert_eq!(
            receipt.profile_id,
            "tassadar.internal_compute.installed_process_lifecycle.v1"
        );
        assert_eq!(receipt.portable_process_ids.len(), 2);
        assert_eq!(receipt.exact_migration_case_count, 1);
        assert_eq!(receipt.exact_rollback_case_count, 1);
        assert_eq!(receipt.refusal_case_count, 3);
        assert!(!receipt.served_publication_allowed);
        assert!(receipt.overall_green);
    }
}
