use serde::{Deserialize, Serialize};

use psionic_eval::TassadarBroadInternalComputeAcceptanceGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeAcceptanceGateReceipt {
    pub report_id: String,
    pub green_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarBroadInternalComputeAcceptanceGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarBroadInternalComputeAcceptanceGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_profile_ids: report.green_profile_ids.clone(),
            suppressed_profile_ids: report.suppressed_profile_ids.clone(),
            failed_profile_ids: report.failed_profile_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "broad internal-compute gate `{}` exposes green_profiles={}, suppressed_profiles={}, failed_profiles={}, overall_green={}",
                report.report_id,
                report.green_profile_ids.len(),
                report.suppressed_profile_ids.len(),
                report.failed_profile_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarBroadInternalComputeAcceptanceGateReceipt;
    use psionic_eval::build_tassadar_broad_internal_compute_acceptance_gate_report;

    #[test]
    fn broad_internal_compute_acceptance_gate_receipt_projects_report() {
        let report =
            build_tassadar_broad_internal_compute_acceptance_gate_report().expect("report");
        let receipt = TassadarBroadInternalComputeAcceptanceGateReceipt::from_report(&report);

        assert!(receipt
            .green_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.article_closeout.v1"
            )));
        assert!(receipt
            .suppressed_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.generalized_abi.v1"
            )));
        assert!(!receipt.overall_green);
    }
}
