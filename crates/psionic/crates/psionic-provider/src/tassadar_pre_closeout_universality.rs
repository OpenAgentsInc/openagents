use serde::{Deserialize, Serialize};

use psionic_research::TassadarPreCloseoutUniversalityClaimBoundaryReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreCloseoutUniversalityReceipt {
    pub report_id: String,
    pub claim_status: String,
    pub blocked_by: Vec<String>,
    pub current_true_scopes: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub detail: String,
}

impl TassadarPreCloseoutUniversalityReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarPreCloseoutUniversalityClaimBoundaryReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            claim_status: format!("{:?}", report.eval_report.claim_status).to_lowercase(),
            blocked_by: report.blocked_by.clone(),
            current_true_scopes: report.current_true_scopes.clone(),
            explicit_non_implications: report.explicit_non_implications.clone(),
            detail: format!(
                "pre-closeout universality report `{}` keeps claim_status={:?}, blocked_by={}, current_true_scopes={}, explicit_non_implications={}",
                report.report_id,
                report.eval_report.claim_status,
                report.blocked_by.len(),
                report.current_true_scopes.len(),
                report.explicit_non_implications.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPreCloseoutUniversalityReceipt;
    use psionic_research::build_tassadar_pre_closeout_universality_claim_boundary_report;

    #[test]
    fn pre_closeout_universality_receipt_projects_summary() {
        let report =
            build_tassadar_pre_closeout_universality_claim_boundary_report().expect("report");
        let receipt = TassadarPreCloseoutUniversalityReceipt::from_report(&report);

        assert_eq!(receipt.claim_status, "suppressed");
        assert!(receipt
            .blocked_by
            .contains(&String::from("universality_verdict_split")));
        assert!(receipt
            .current_true_scopes
            .contains(&String::from("named proposal-family promotion boundary")));
    }
}
