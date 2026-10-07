use serde::{Deserialize, Serialize};

use psionic_eval::TassadarInternalComponentAbiReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub green_component_graph_ids: Vec<String>,
    pub interface_manifest_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarInternalComponentAbiReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarInternalComponentAbiReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.ir_contract.profile_id.clone(),
            green_component_graph_ids: report.green_component_graph_ids.clone(),
            interface_manifest_case_ids: report.interface_manifest_case_ids.clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            served_publication_allowed: report.served_publication_allowed,
            overall_green: report.overall_green,
            detail: format!(
                "internal component ABI report `{}` keeps green_graphs={}, interface_manifest_cases={}, portability_envelopes={}, served_publication_allowed={}, overall_green={}",
                report.report_id,
                report.green_component_graph_ids.len(),
                report.interface_manifest_case_ids.len(),
                report.portability_envelope_ids.len(),
                report.served_publication_allowed,
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInternalComponentAbiReceipt;
    use psionic_eval::build_tassadar_internal_component_abi_report;

    #[test]
    fn internal_component_abi_receipt_projects_report() {
        let report = build_tassadar_internal_component_abi_report().expect("report");
        let receipt = TassadarInternalComponentAbiReceipt::from_report(&report);

        assert_eq!(
            receipt.profile_id,
            "tassadar.internal_compute.component_model_abi.v1"
        );
        assert_eq!(receipt.green_component_graph_ids.len(), 3);
        assert_eq!(receipt.interface_manifest_case_ids.len(), 3);
        assert!(!receipt.served_publication_allowed);
        assert!(receipt.overall_green);
    }
}
