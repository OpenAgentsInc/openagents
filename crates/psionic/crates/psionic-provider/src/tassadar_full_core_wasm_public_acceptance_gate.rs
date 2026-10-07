use serde::{Deserialize, Serialize};

use psionic_research::TassadarFullCoreWasmOperatorRunbookV2Summary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFullCoreWasmPublicAcceptanceGateReceipt {
    pub report_id: String,
    pub acceptance_status: String,
    pub blocked_by: Vec<String>,
    pub operator_drill_commands: Vec<String>,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarFullCoreWasmPublicAcceptanceGateReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarFullCoreWasmOperatorRunbookV2Summary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            acceptance_status: format!("{:?}", summary.eval_report.acceptance_status).to_lowercase(),
            blocked_by: summary.blocked_by.clone(),
            operator_drill_commands: summary.operator_drill_commands.clone(),
            served_publication_allowed: summary.eval_report.served_publication_allowed,
            detail: format!(
                "full core-Wasm public acceptance summary `{}` keeps acceptance_status={:?}, blocked_by={}, operator_drill_commands={}, served_publication_allowed={}",
                summary.report_id,
                summary.eval_report.acceptance_status,
                summary.blocked_by.len(),
                summary.operator_drill_commands.len(),
                summary.eval_report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarFullCoreWasmPublicAcceptanceGateReceipt;
    use psionic_research::build_tassadar_full_core_wasm_operator_runbook_v2_summary;

    #[test]
    fn full_core_wasm_public_acceptance_gate_receipt_projects_summary() {
        let summary = build_tassadar_full_core_wasm_operator_runbook_v2_summary().expect("summary");
        let receipt = TassadarFullCoreWasmPublicAcceptanceGateReceipt::from_summary(&summary);

        assert_eq!(receipt.acceptance_status, "suppressed");
        assert!(receipt
            .blocked_by
            .contains(&String::from("target_feature_family_coverage")));
        assert!(!receipt.served_publication_allowed);
    }
}
