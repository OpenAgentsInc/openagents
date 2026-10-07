use serde::{Deserialize, Serialize};

use psionic_eval::TassadarVirtualFsMountProfileReport;

/// Provider-facing receipt for the bounded virtual-filesystem mount profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsMountProfileReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub sandbox_boundary_report_ref: String,
    pub allowed_mount_ids: Vec<String>,
    pub refused_path_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarVirtualFsMountProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarVirtualFsMountProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            sandbox_boundary_report_ref: report.sandbox_boundary_report_ref.clone(),
            allowed_mount_ids: report.allowed_mount_ids.clone(),
            refused_path_ids: report.refused_path_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            served_publication_allowed: report.served_publication_allowed,
            detail: format!(
                "virtual-fs mount profile receipt `{}` carries allowed_mounts={}, refused_paths={}, exact_cases={}, refusal_rows={}, served_publication_allowed={}",
                report.report_id,
                report.allowed_mount_ids.len(),
                report.refused_path_ids.len(),
                report.exact_case_count,
                report.refusal_case_count,
                report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarVirtualFsMountProfileReceipt;
    use psionic_eval::build_tassadar_virtual_fs_mount_profile_report;

    #[test]
    fn virtual_fs_mount_profile_receipt_projects_report() {
        let report = build_tassadar_virtual_fs_mount_profile_report().expect("report");
        let receipt = TassadarVirtualFsMountProfileReceipt::from_report(&report);

        assert_eq!(receipt.profile_id, "tassadar.effect_profile.virtual_fs_mounts.v1");
        assert_eq!(receipt.exact_case_count, 2);
        assert_eq!(receipt.refusal_case_count, 3);
        assert_eq!(receipt.allowed_mount_ids.len(), 2);
        assert_eq!(receipt.refused_path_ids.len(), 2);
        assert!(!receipt.served_publication_allowed);
    }
}
