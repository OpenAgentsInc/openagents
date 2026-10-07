use serde::{Deserialize, Serialize};

use psionic_eval::TassadarMultiMemoryProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryProfileReceipt {
    pub report_id: String,
    pub green_topology_ids: Vec<String>,
    pub checkpoint_capable_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarMultiMemoryProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarMultiMemoryProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_topology_ids: report.green_topology_ids.clone(),
            checkpoint_capable_case_ids: report.checkpoint_capable_case_ids.clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "multi-memory profile report `{}` keeps green_topologies={}, checkpoint_capable_cases={}, portability_envelopes={}, overall_green={}",
                report.report_id,
                report.green_topology_ids.len(),
                report.checkpoint_capable_case_ids.len(),
                report.portability_envelope_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarMultiMemoryProfileReceipt;
    use psionic_eval::build_tassadar_multi_memory_profile_report;

    #[test]
    fn multi_memory_profile_receipt_projects_report() {
        let report = build_tassadar_multi_memory_profile_report().expect("report");
        let receipt = TassadarMultiMemoryProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.green_topology_ids.len(), 2);
        assert_eq!(receipt.checkpoint_capable_case_ids.len(), 1);
        assert!(
            receipt
                .green_topology_ids
                .contains(&String::from("scratch_heap_checkpoint_split"))
        );
    }
}
