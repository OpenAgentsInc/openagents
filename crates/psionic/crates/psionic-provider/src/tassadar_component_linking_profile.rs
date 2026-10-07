use serde::{Deserialize, Serialize};

use psionic_eval::TassadarComponentLinkingProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingProfileReceipt {
    pub report_id: String,
    pub green_topology_ids: Vec<String>,
    pub lineage_artifact_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarComponentLinkingProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarComponentLinkingProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_topology_ids: report.green_topology_ids.clone(),
            lineage_artifact_case_ids: report.lineage_artifact_case_ids.clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "component-linking profile report `{}` keeps green_topologies={}, lineage_artifact_cases={}, portability_envelopes={}, overall_green={}",
                report.report_id,
                report.green_topology_ids.len(),
                report.lineage_artifact_case_ids.len(),
                report.portability_envelope_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarComponentLinkingProfileReceipt;
    use psionic_eval::build_tassadar_component_linking_profile_report;

    #[test]
    fn component_linking_profile_receipt_projects_report() {
        let report = build_tassadar_component_linking_profile_report().expect("report");
        let receipt = TassadarComponentLinkingProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.green_topology_ids.len(), 2);
        assert_eq!(receipt.lineage_artifact_case_ids.len(), 2);
        assert!(
            receipt
                .green_topology_ids
                .contains(&String::from("checkpoint_resume_component_pair"))
        );
    }
}
