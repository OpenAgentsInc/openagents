use serde::{Deserialize, Serialize};

use psionic_eval::TassadarFrozenCoreWasmClosureGateReport;
use psionic_runtime::TassadarFrozenCoreWasmClosureGateStatus;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmClosureGateReceipt {
    pub report_id: String,
    pub window_id: String,
    pub closure_status: TassadarFrozenCoreWasmClosureGateStatus,
    pub served_publication_allowed: bool,
    pub green_row_ids: Vec<String>,
    pub red_row_ids: Vec<String>,
    pub detail: String,
}

impl TassadarFrozenCoreWasmClosureGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarFrozenCoreWasmClosureGateReport) -> Self {
        let green_row_ids = report
            .gate_rows
            .iter()
            .filter(|row| {
                row.status == psionic_runtime::TassadarFrozenCoreWasmClosureGateRowStatus::Green
            })
            .map(|row| row.row_id.clone())
            .collect::<Vec<_>>();
        let red_row_ids = report
            .gate_rows
            .iter()
            .filter(|row| {
                row.status == psionic_runtime::TassadarFrozenCoreWasmClosureGateRowStatus::Red
            })
            .map(|row| row.row_id.clone())
            .collect::<Vec<_>>();
        Self {
            report_id: report.report_id.clone(),
            window_id: report.frozen_window_id.clone(),
            closure_status: report.closure_status,
            served_publication_allowed: report.served_publication_allowed,
            green_row_ids,
            red_row_ids,
            detail: report.detail.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarFrozenCoreWasmClosureGateReceipt;
    use psionic_eval::build_tassadar_frozen_core_wasm_closure_gate_report;
    use psionic_runtime::TassadarFrozenCoreWasmClosureGateStatus;

    #[test]
    fn frozen_core_wasm_closure_gate_receipt_projects_report() {
        let report = build_tassadar_frozen_core_wasm_closure_gate_report().expect("report");
        let receipt = TassadarFrozenCoreWasmClosureGateReceipt::from_report(&report);

        assert_eq!(
            receipt.closure_status,
            TassadarFrozenCoreWasmClosureGateStatus::NotClosed
        );
        assert!(!receipt.served_publication_allowed);
        assert!(receipt
            .red_row_ids
            .contains(&String::from("cross_machine_harness_replay")));
    }
}
