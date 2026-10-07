use serde::{Deserialize, Serialize};

use psionic_eval::TassadarArticleEquivalenceAcceptanceGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceAcceptanceGateReceipt {
    pub report_id: String,
    pub acceptance_status: String,
    pub blocked_issue_ids: Vec<String>,
    pub blocked_blocker_ids: Vec<String>,
    pub optional_open_issue_ids: Vec<String>,
    pub required_issue_count: usize,
    pub closed_required_issue_count: usize,
    pub article_equivalence_green: bool,
    pub public_claim_allowed: bool,
    pub detail: String,
}

impl TassadarArticleEquivalenceAcceptanceGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleEquivalenceAcceptanceGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            acceptance_status: format!("{:?}", report.acceptance_status).to_lowercase(),
            blocked_issue_ids: report.blocked_issue_ids.clone(),
            blocked_blocker_ids: report.blocked_blocker_ids.clone(),
            optional_open_issue_ids: report.optional_open_issue_ids.clone(),
            required_issue_count: report.required_issue_count,
            closed_required_issue_count: report.closed_required_issue_count,
            article_equivalence_green: report.article_equivalence_green,
            public_claim_allowed: report.public_claim_allowed,
            detail: format!(
                "article-equivalence acceptance gate `{}` keeps acceptance_status={:?}, closed_required_issues={}/{}, blocked_issues={}, blocked_blockers={}, optional_open_issues={}, article_equivalence_green={}, public_claim_allowed={}",
                report.report_id,
                report.acceptance_status,
                report.closed_required_issue_count,
                report.required_issue_count,
                report.blocked_issue_ids.len(),
                report.blocked_blocker_ids.len(),
                report.optional_open_issue_ids.len(),
                report.article_equivalence_green,
                report.public_claim_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleEquivalenceAcceptanceGateReceipt;
    use psionic_eval::build_tassadar_article_equivalence_acceptance_gate_report;

    #[test]
    fn article_equivalence_acceptance_gate_receipt_projects_report() {
        let report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let receipt = TassadarArticleEquivalenceAcceptanceGateReceipt::from_report(&report);

        assert_eq!(receipt.acceptance_status, "green");
        assert_eq!(receipt.required_issue_count, report.required_issue_count);
        assert_eq!(
            receipt.closed_required_issue_count,
            report.closed_required_issue_count
        );
        assert_eq!(receipt.blocked_issue_ids, report.blocked_issue_ids);
        assert_eq!(receipt.blocked_blocker_ids, report.blocked_blocker_ids);
        assert_eq!(
            receipt.optional_open_issue_ids,
            report.optional_open_issue_ids
        );
        assert!(receipt.blocked_issue_ids.is_empty());
        assert!(receipt.blocked_blocker_ids.is_empty());
        assert!(receipt.article_equivalence_green);
        assert!(receipt.public_claim_allowed);
    }
}
