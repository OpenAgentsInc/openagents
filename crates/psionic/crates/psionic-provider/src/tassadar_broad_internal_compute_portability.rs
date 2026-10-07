use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarBroadInternalComputePortabilityReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputePortabilityReceipt {
    pub report_id: String,
    pub current_host_machine_class_id: String,
    pub backend_family_ids: Vec<String>,
    pub toolchain_family_ids: Vec<String>,
    pub publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub detail: String,
}

impl TassadarBroadInternalComputePortabilityReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarBroadInternalComputePortabilityReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            current_host_machine_class_id: report.current_host_machine_class_id.clone(),
            backend_family_ids: report.backend_family_ids.clone(),
            toolchain_family_ids: report.toolchain_family_ids.clone(),
            publication_allowed_profile_ids: report.publication_allowed_profile_ids.clone(),
            suppressed_profile_ids: report.suppressed_profile_ids.clone(),
            detail: format!(
                "broad internal-compute portability `{}` carries backends={}, toolchains={}, published_profiles={}, suppressed_profiles={}, current_host=`{}`",
                report.report_id,
                report.backend_family_ids.len(),
                report.toolchain_family_ids.len(),
                report.publication_allowed_profile_ids.len(),
                report.suppressed_profile_ids.len(),
                report.current_host_machine_class_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarBroadInternalComputePortabilityReceipt;
    use psionic_eval::build_tassadar_broad_internal_compute_portability_report;

    #[test]
    fn broad_internal_compute_portability_receipt_projects_report() {
        let report = build_tassadar_broad_internal_compute_portability_report().expect("report");
        let receipt = TassadarBroadInternalComputePortabilityReceipt::from_report(&report);

        assert_eq!(receipt.backend_family_ids.len(), 3);
        assert!(receipt
            .backend_family_ids
            .contains(&String::from("cpu_reference")));
        assert!(receipt
            .toolchain_family_ids
            .contains(&String::from("rustc:wasm32-unknown-unknown")));
        assert!(receipt
            .publication_allowed_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.article_closeout.v1"
            )));
    }
}
