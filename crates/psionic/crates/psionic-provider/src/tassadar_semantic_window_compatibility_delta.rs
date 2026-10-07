use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSemanticWindowCompatibilityDeltaReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowCompatibilityDeltaReceipt {
    pub report_id: String,
    pub active_window_id: String,
    pub compatible_candidate_window_ids: Vec<String>,
    pub blocked_candidate_window_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub operator_drill_commands: Vec<String>,
    pub detail: String,
}

impl TassadarSemanticWindowCompatibilityDeltaReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSemanticWindowCompatibilityDeltaReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            active_window_id: report.active_window_id.clone(),
            compatible_candidate_window_ids: report.compatible_candidate_window_ids.clone(),
            blocked_candidate_window_ids: report.blocked_candidate_window_ids.clone(),
            served_publication_allowed: report.served_publication_allowed,
            operator_drill_commands: report.operator_drill_commands.clone(),
            detail: format!(
                "semantic-window compatibility delta `{}` keeps active_window_id={}, compatible_candidates={}, blocked_candidates={}, served_publication_allowed={}",
                report.report_id,
                report.active_window_id,
                report.compatible_candidate_window_ids.len(),
                report.blocked_candidate_window_ids.len(),
                report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSemanticWindowCompatibilityDeltaReceipt;
    use psionic_eval::build_tassadar_semantic_window_compatibility_delta_report;

    #[test]
    fn semantic_window_compatibility_delta_receipt_projects_report() {
        let report = build_tassadar_semantic_window_compatibility_delta_report().expect("report");
        let receipt = TassadarSemanticWindowCompatibilityDeltaReceipt::from_report(&report);

        assert_eq!(
            receipt.active_window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert_eq!(receipt.compatible_candidate_window_ids.len(), 1);
        assert_eq!(receipt.blocked_candidate_window_ids.len(), 2);
        assert!(!receipt.served_publication_allowed);
        assert!(receipt.operator_drill_commands.iter().any(|command| {
            command.contains("tassadar_semantic_window_compatibility_delta_report")
        }));
    }
}
