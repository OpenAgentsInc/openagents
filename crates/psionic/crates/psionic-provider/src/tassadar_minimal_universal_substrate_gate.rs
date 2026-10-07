use serde::{Deserialize, Serialize};

use psionic_eval::TassadarMinimalUniversalSubstrateAcceptanceGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMinimalUniversalSubstrateGateReceipt {
    pub report_id: String,
    pub green_requirement_ids: Vec<String>,
    pub failed_requirement_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarMinimalUniversalSubstrateGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarMinimalUniversalSubstrateAcceptanceGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_requirement_ids: report.green_requirement_ids.clone(),
            failed_requirement_ids: report.failed_requirement_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "minimal universal-substrate gate `{}` keeps green_requirements={}, failed_requirements={}, overall_green={}",
                report.report_id,
                report.green_requirement_ids.len(),
                report.failed_requirement_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarMinimalUniversalSubstrateGateReceipt;
    use psionic_eval::build_tassadar_minimal_universal_substrate_acceptance_gate_report;

    #[test]
    fn minimal_universal_substrate_gate_receipt_projects_report() {
        let report =
            build_tassadar_minimal_universal_substrate_acceptance_gate_report().expect("report");
        let receipt = TassadarMinimalUniversalSubstrateGateReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.green_requirement_ids.len(), 8);
        assert!(receipt.failed_requirement_ids.is_empty());
    }
}
