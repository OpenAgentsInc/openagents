use serde::{Deserialize, Serialize};

use psionic_eval::TassadarProposalProfileLadderClaimCheckerReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfileLadderClaimCheckerReceipt {
    pub report_id: String,
    pub public_profile_ids: Vec<String>,
    pub served_publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub research_only_profile_ids: Vec<String>,
    pub default_served_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarProposalProfileLadderClaimCheckerReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarProposalProfileLadderClaimCheckerReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            public_profile_ids: report.public_profile_ids.clone(),
            served_publication_allowed_profile_ids: report
                .served_publication_allowed_profile_ids
                .clone(),
            suppressed_profile_ids: report.suppressed_profile_ids.clone(),
            research_only_profile_ids: report.research_only_profile_ids.clone(),
            default_served_profile_ids: report.default_served_profile_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "proposal-profile ladder claim checker `{}` keeps public_profiles={}, served_publication_allowed_profiles={}, suppressed_profiles={}, research_only_profiles={}, default_served_profiles={}, overall_green={}",
                report.report_id,
                report.public_profile_ids.len(),
                report.served_publication_allowed_profile_ids.len(),
                report.suppressed_profile_ids.len(),
                report.research_only_profile_ids.len(),
                report.default_served_profile_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarProposalProfileLadderClaimCheckerReceipt;
    use psionic_eval::build_tassadar_proposal_profile_ladder_claim_checker_report;

    #[test]
    fn proposal_profile_ladder_claim_checker_receipt_projects_report() {
        let report = build_tassadar_proposal_profile_ladder_claim_checker_report().expect("report");
        let receipt = TassadarProposalProfileLadderClaimCheckerReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.public_profile_ids.len(), 2);
        assert!(receipt
            .served_publication_allowed_profile_ids
            .contains(&String::from(
                "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"
            )));
        assert!(receipt.suppressed_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.multi_memory_routing.v1"
        )));
        assert!(receipt.research_only_profile_ids.contains(&String::from(
            "tassadar.research_profile.threads_deterministic_scheduler.v1"
        )));
        assert!(receipt.default_served_profile_ids.is_empty());
    }
}
