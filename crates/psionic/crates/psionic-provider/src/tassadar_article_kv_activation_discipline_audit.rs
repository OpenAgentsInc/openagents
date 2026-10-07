use serde::{Deserialize, Serialize};

use psionic_eval::{
    TassadarArticleKvActivationDisciplineAuditReport, TassadarArticleStateDominanceVerdictKind,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationDisciplineAuditReceipt {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub blocked_issue_ids: Vec<String>,
    pub ownership_gate_green: bool,
    pub feasible_constraint_case_count: u32,
    pub dominance_verdict: TassadarArticleStateDominanceVerdictKind,
    pub cache_growth_scales_with_problem_size: bool,
    pub dynamic_state_exceeds_weight_artifact_bytes: bool,
    pub cache_truncation_breaks_correctness: bool,
    pub cache_reset_breaks_correctness: bool,
    pub equivalent_behavior_survives_under_constrained_cache: bool,
    pub kv_activation_discipline_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleKvActivationDisciplineAuditReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleKvActivationDisciplineAuditReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            blocked_issue_ids: report.acceptance_gate_tie.blocked_issue_ids.clone(),
            ownership_gate_green: report.ownership_gate_green,
            feasible_constraint_case_count: report.growth_report.feasible_constraint_case_ids.len()
                as u32,
            dominance_verdict: report.dominance_verdict.verdict,
            cache_growth_scales_with_problem_size: report
                .growth_report
                .cache_growth_scales_with_problem_size,
            dynamic_state_exceeds_weight_artifact_bytes: report
                .growth_report
                .dynamic_state_exceeds_weight_artifact_bytes,
            cache_truncation_breaks_correctness: report
                .sensitivity_review
                .cache_truncation_breaks_correctness,
            cache_reset_breaks_correctness: report.sensitivity_review.cache_reset_breaks_correctness,
            equivalent_behavior_survives_under_constrained_cache: report
                .sensitivity_review
                .equivalent_behavior_survives_under_constrained_cache,
            kv_activation_discipline_green: report.kv_activation_discipline_green,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article KV-cache and activation-state discipline audit `{}` keeps tied_requirement_id={}, blocked_issues={}, ownership_gate_green={}, feasible_constraint_case_count={}, dominance_verdict={:?}, cache_growth_scales_with_problem_size={}, dynamic_state_exceeds_weight_artifact_bytes={}, cache_truncation_breaks_correctness={}, cache_reset_breaks_correctness={}, constrained_cache_equivalence={}, kv_activation_discipline_green={}, article_equivalence_green={}",
                report.report_id,
                report.acceptance_gate_tie.tied_requirement_id,
                report.acceptance_gate_tie.blocked_issue_ids.len(),
                report.ownership_gate_green,
                report.growth_report.feasible_constraint_case_ids.len(),
                report.dominance_verdict.verdict,
                report.growth_report.cache_growth_scales_with_problem_size,
                report.growth_report.dynamic_state_exceeds_weight_artifact_bytes,
                report.sensitivity_review.cache_truncation_breaks_correctness,
                report.sensitivity_review.cache_reset_breaks_correctness,
                report
                    .sensitivity_review
                    .equivalent_behavior_survives_under_constrained_cache,
                report.kv_activation_discipline_green,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleKvActivationDisciplineAuditReceipt;
    use psionic_eval::{
        TassadarArticleKvActivationDisciplineAuditReport, TassadarArticleStateDominanceVerdictKind,
        TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
    };

    fn read_committed_report(
    ) -> Result<TassadarArticleKvActivationDisciplineAuditReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path =
            repo_root.join(TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_kv_activation_discipline_audit_receipt_projects_report(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = read_committed_report()?;
        let receipt = TassadarArticleKvActivationDisciplineAuditReceipt::from_report(&report);

        assert_eq!(receipt.tied_requirement_id, "TAS-184A");
        assert!(receipt.blocked_issue_ids.is_empty());
        assert!(receipt.ownership_gate_green);
        assert_eq!(receipt.feasible_constraint_case_count, 4);
        assert_eq!(
            receipt.dominance_verdict,
            TassadarArticleStateDominanceVerdictKind::Mixed
        );
        assert!(receipt.cache_growth_scales_with_problem_size);
        assert!(receipt.dynamic_state_exceeds_weight_artifact_bytes);
        assert!(receipt.cache_truncation_breaks_correctness);
        assert!(receipt.cache_reset_breaks_correctness);
        assert!(!receipt.equivalent_behavior_survives_under_constrained_cache);
        assert!(receipt.kv_activation_discipline_green);
        assert!(receipt.article_equivalence_green);
        Ok(())
    }
}
