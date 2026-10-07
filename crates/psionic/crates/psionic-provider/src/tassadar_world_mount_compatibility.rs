use serde::{Deserialize, Serialize};

use psionic_router::TassadarWorldMountCompatibilityReport;

/// Provider-facing receipt for the router-owned world-mount compatibility report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorldMountCompatibilityReceipt {
    pub report_id: String,
    pub allowed_case_count: u32,
    pub denied_case_count: u32,
    pub unresolved_case_count: u32,
    pub supported_trust_posture_count: u32,
    pub validator_binding_available: bool,
    pub detail: String,
}

impl TassadarWorldMountCompatibilityReceipt {
    /// Builds a provider-facing receipt from the router report.
    #[must_use]
    pub fn from_report(report: &TassadarWorldMountCompatibilityReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            allowed_case_count: report.allowed_case_count,
            denied_case_count: report.denied_case_count,
            unresolved_case_count: report.unresolved_case_count,
            supported_trust_posture_count: report.surface.supported_trust_postures.len() as u32,
            validator_binding_available: report.surface.validator_binding_available,
            detail: format!(
                "world-mount compatibility report `{}` exposes allowed={}, denied={}, unresolved={}, supported_trust_postures={}, validator_binding_available={}",
                report.report_id,
                report.allowed_case_count,
                report.denied_case_count,
                report.unresolved_case_count,
                report.surface.supported_trust_postures.len(),
                report.surface.validator_binding_available,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarWorldMountCompatibilityReceipt;
    use psionic_router::build_tassadar_world_mount_compatibility_report;

    #[test]
    fn world_mount_compatibility_receipt_projects_router_report() {
        let report = build_tassadar_world_mount_compatibility_report();
        let receipt = TassadarWorldMountCompatibilityReceipt::from_report(&report);

        assert_eq!(receipt.allowed_case_count, 2);
        assert_eq!(receipt.denied_case_count, 1);
        assert_eq!(receipt.unresolved_case_count, 1);
        assert!(receipt.supported_trust_posture_count >= 2);
        assert!(receipt.validator_binding_available);
    }
}
