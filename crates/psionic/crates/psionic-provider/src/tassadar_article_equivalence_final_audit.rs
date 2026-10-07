use serde::{Deserialize, Serialize};

use psionic_eval::TassadarArticleEquivalenceFinalAuditReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditReceipt {
    pub report_id: String,
    pub matched_article_line_count: usize,
    pub all_article_lines_matched: bool,
    pub mechanistic_verdict_green: bool,
    pub behavioral_verdict_green: bool,
    pub operational_verdict_green: bool,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub canonical_decode_mode: String,
    pub supported_machine_class_ids: Vec<String>,
    pub optional_open_issue_ids: Vec<String>,
    pub public_article_equivalence_claim_allowed: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleEquivalenceFinalAuditReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleEquivalenceFinalAuditReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            matched_article_line_count: report.matched_article_line_count,
            all_article_lines_matched: report.all_article_lines_matched,
            mechanistic_verdict_green: report.verdict_review.mechanistic_verdict_green,
            behavioral_verdict_green: report.verdict_review.behavioral_verdict_green,
            operational_verdict_green: report.verdict_review.operational_verdict_green,
            canonical_model_id: report
                .canonical_closure_review
                .canonical_model_id
                .clone(),
            canonical_weight_artifact_id: report
                .canonical_closure_review
                .canonical_weight_artifact_id
                .clone(),
            canonical_route_id: report
                .canonical_closure_review
                .canonical_route_id
                .clone(),
            canonical_decode_mode: report
                .canonical_closure_review
                .canonical_decode_mode
                .clone(),
            supported_machine_class_ids: report
                .machine_matrix_review
                .supported_machine_class_ids
                .clone(),
            optional_open_issue_ids: report.exclusion_review.optional_open_issue_ids.clone(),
            public_article_equivalence_claim_allowed: report.public_article_equivalence_claim_allowed,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article-equivalence final audit `{}` keeps matched_article_line_count={}, all_article_lines_matched={}, canonical_route_id=`{}`, supported_machine_classes={}, optional_open_issues={}, public_article_equivalence_claim_allowed={}, and article_equivalence_green={}",
                report.report_id,
                report.matched_article_line_count,
                report.all_article_lines_matched,
                report.canonical_closure_review.canonical_route_id,
                report.machine_matrix_review.supported_machine_class_ids.len(),
                report.exclusion_review.optional_open_issue_ids.len(),
                report.public_article_equivalence_claim_allowed,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleEquivalenceFinalAuditReceipt;
    use psionic_eval::{
        TassadarArticleEquivalenceFinalAuditReport,
        TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
    };

    fn read_committed_report(
    ) -> Result<TassadarArticleEquivalenceFinalAuditReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path = repo_root.join(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_equivalence_final_audit_receipt_projects_report(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = read_committed_report()?;
        let receipt = TassadarArticleEquivalenceFinalAuditReceipt::from_report(&report);

        assert!(receipt.all_article_lines_matched);
        assert!(receipt.mechanistic_verdict_green);
        assert!(receipt.behavioral_verdict_green);
        assert!(receipt.operational_verdict_green);
        assert_eq!(
            receipt.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(receipt.canonical_decode_mode, "hull_cache");
        assert_eq!(receipt.supported_machine_class_ids.len(), 2);
        assert_eq!(
            receipt.optional_open_issue_ids,
            vec![String::from("TAS-R1")]
        );
        assert!(receipt.public_article_equivalence_claim_allowed);
        assert!(receipt.article_equivalence_green);
        Ok(())
    }
}
