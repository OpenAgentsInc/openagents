use serde::{Deserialize, Serialize};

use psionic_router::TassadarSelfInstallationGateReport;

/// Provider-facing receipt for the bounded self-installation gate report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSelfInstallationGateReceipt {
    pub report_id: String,
    pub session_mounted_count: u32,
    pub challenge_window_count: u32,
    pub rolled_back_count: u32,
    pub refused_count: u32,
    pub detail: String,
}

impl TassadarSelfInstallationGateReceipt {
    /// Builds a provider-facing receipt from the router report.
    #[must_use]
    pub fn from_report(report: &TassadarSelfInstallationGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            session_mounted_count: report.session_mounted_count,
            challenge_window_count: report.challenge_window_count,
            rolled_back_count: report.rolled_back_count,
            refused_count: report.refused_count,
            detail: format!(
                "self-installation gate report `{}` exposes session_mounted={}, challenge_window={}, rolled_back={}, refused={}",
                report.report_id,
                report.session_mounted_count,
                report.challenge_window_count,
                report.rolled_back_count,
                report.refused_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSelfInstallationGateReceipt;
    use psionic_router::build_tassadar_self_installation_gate_report;

    #[test]
    fn self_installation_gate_receipt_projects_router_report() {
        let report = build_tassadar_self_installation_gate_report();
        let receipt = TassadarSelfInstallationGateReceipt::from_report(&report);

        assert_eq!(receipt.session_mounted_count, 1);
        assert_eq!(receipt.challenge_window_count, 1);
        assert_eq!(receipt.rolled_back_count, 1);
        assert_eq!(receipt.refused_count, 1);
    }
}
