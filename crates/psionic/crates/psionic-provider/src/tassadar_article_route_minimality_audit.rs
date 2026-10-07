use serde::{Deserialize, Serialize};

use psionic_eval::{
    TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityPublicPosture,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityAuditReceipt {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub blocked_issue_ids: Vec<String>,
    pub canonical_claim_route_id: String,
    pub route_descriptor_digest: String,
    pub selected_decode_mode: String,
    pub operator_verdict_green: bool,
    pub public_posture: TassadarArticleRouteMinimalityPublicPosture,
    pub public_verdict_green: bool,
    pub route_minimality_audit_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleRouteMinimalityAuditReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleRouteMinimalityAuditReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            blocked_issue_ids: report.public_verdict_review.blocked_issue_ids.clone(),
            canonical_claim_route_id: report
                .canonical_claim_route_review
                .canonical_claim_route_id
                .clone(),
            route_descriptor_digest: report
                .canonical_claim_route_review
                .projected_route_descriptor_digest
                .clone(),
            selected_decode_mode: report
                .canonical_claim_route_review
                .selected_decode_mode
                .as_str()
                .to_string(),
            operator_verdict_green: report.operator_verdict_review.operator_verdict_green,
            public_posture: report.public_verdict_review.posture,
            public_verdict_green: report.public_verdict_review.public_verdict_green,
            route_minimality_audit_green: report.route_minimality_audit_green,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article route-minimality audit `{}` keeps tied_requirement_id={}, blocked_issues={}, canonical_claim_route_id=`{}`, selected_decode_mode=`{}`, operator_verdict_green={}, public_posture={:?}, public_verdict_green={}, route_minimality_audit_green={}, and article_equivalence_green={}",
                report.report_id,
                report.acceptance_gate_tie.tied_requirement_id,
                report.public_verdict_review.blocked_issue_ids.len(),
                report.canonical_claim_route_review.canonical_claim_route_id,
                report.canonical_claim_route_review.selected_decode_mode.as_str(),
                report.operator_verdict_review.operator_verdict_green,
                report.public_verdict_review.posture,
                report.public_verdict_review.public_verdict_green,
                report.route_minimality_audit_green,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleRouteMinimalityAuditReceipt;
    use psionic_eval::{
        TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityPublicPosture,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
    };

    fn read_committed_report(
    ) -> Result<TassadarArticleRouteMinimalityAuditReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path = repo_root.join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_route_minimality_audit_receipt_projects_report(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = read_committed_report()?;
        let receipt = TassadarArticleRouteMinimalityAuditReceipt::from_report(&report);

        assert_eq!(receipt.tied_requirement_id, "TAS-185A");
        assert!(receipt.blocked_issue_ids.is_empty());
        assert_eq!(
            receipt.canonical_claim_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            receipt.selected_decode_mode,
            "tassadar.decode.hull_cache.v1"
        );
        assert!(receipt.operator_verdict_green);
        assert_eq!(
            receipt.public_posture,
            TassadarArticleRouteMinimalityPublicPosture::GreenBounded
        );
        assert!(receipt.public_verdict_green);
        assert!(receipt.route_minimality_audit_green);
        assert!(receipt.article_equivalence_green);
        Ok(())
    }
}
