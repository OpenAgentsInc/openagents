use serde::{Deserialize, Serialize};

use psionic_research::TassadarGeneralInternalComputeRedTeamSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamReceipt {
    pub report_id: String,
    pub publication_safe: bool,
    pub blocked_finding_ids: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub detail: String,
}

impl TassadarGeneralInternalComputeRedTeamReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarGeneralInternalComputeRedTeamSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            publication_safe: summary.audit_report.publication_safe,
            blocked_finding_ids: summary.blocked_finding_ids.clone(),
            explicit_non_implications: summary.explicit_non_implications.clone(),
            detail: format!(
                "general internal-compute red-team summary `{}` keeps publication_safe={}, blocked_findings={}, explicit_non_implications={}",
                summary.report_id,
                summary.audit_report.publication_safe,
                summary.blocked_finding_ids.len(),
                summary.explicit_non_implications.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarGeneralInternalComputeRedTeamReceipt;
    use psionic_research::build_tassadar_general_internal_compute_red_team_summary;

    #[test]
    fn red_team_receipt_projects_summary() {
        let summary = build_tassadar_general_internal_compute_red_team_summary().expect("summary");
        let receipt = TassadarGeneralInternalComputeRedTeamReceipt::from_summary(&summary);

        assert!(receipt.publication_safe);
        assert_eq!(receipt.blocked_finding_ids.len(), 5);
        assert!(receipt
            .explicit_non_implications
            .contains(&String::from("public threads publication")));
    }
}
