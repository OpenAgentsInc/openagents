use serde::{Deserialize, Serialize};

use psionic_eval::{
    TassadarArticleInterpreterOwnershipGateReport, TassadarArticleInterpreterOwnershipLocalityKind,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipGateReceipt {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub blocked_issue_ids: Vec<String>,
    pub generic_direct_proof_case_count: u32,
    pub generic_direct_proof_suite_green: bool,
    pub breadth_family_count: u32,
    pub route_purity_green: bool,
    pub mapping_stable_across_runs: bool,
    pub perturbation_sensitivity_green: bool,
    pub locality_characterization: TassadarArticleInterpreterOwnershipLocalityKind,
    pub interpreter_ownership_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleInterpreterOwnershipGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleInterpreterOwnershipGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            blocked_issue_ids: report.acceptance_gate_tie.blocked_issue_ids.clone(),
            generic_direct_proof_case_count: report.generic_direct_proof_review.case_rows.len()
                as u32,
            generic_direct_proof_suite_green: report
                .generic_direct_proof_review
                .generic_direct_proof_suite_green,
            breadth_family_count: report.breadth_conformance_matrix.family_rows.len() as u32,
            route_purity_green: report.route_purity_review.route_purity_green,
            mapping_stable_across_runs: report.computation_mapping_report.stable_across_runs,
            perturbation_sensitivity_green: report
                .weight_perturbation_review
                .all_interventions_show_sensitivity,
            locality_characterization: report
                .weight_perturbation_review
                .locality_characterization,
            interpreter_ownership_green: report.interpreter_ownership_green,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article interpreter-ownership gate `{}` keeps tied_requirement_id={}, blocked_issues={}, generic_direct_proof_case_count={}, generic_direct_proof_suite_green={}, breadth_family_count={}, route_purity_green={}, mapping_stable_across_runs={}, perturbation_sensitivity_green={}, locality_characterization={:?}, ownership_gate_green={}, article_equivalence_green={}",
                report.report_id,
                report.acceptance_gate_tie.tied_requirement_id,
                report.acceptance_gate_tie.blocked_issue_ids.len(),
                report.generic_direct_proof_review.case_rows.len(),
                report.generic_direct_proof_review.generic_direct_proof_suite_green,
                report.breadth_conformance_matrix.family_rows.len(),
                report.route_purity_review.route_purity_green,
                report.computation_mapping_report.stable_across_runs,
                report
                    .weight_perturbation_review
                    .all_interventions_show_sensitivity,
                report.weight_perturbation_review.locality_characterization,
                report.interpreter_ownership_green,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleInterpreterOwnershipGateReceipt;
    use psionic_eval::{
        TassadarArticleInterpreterOwnershipGateReport,
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
    };

    fn read_committed_report(
    ) -> Result<TassadarArticleInterpreterOwnershipGateReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path = repo_root.join(TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_interpreter_ownership_gate_receipt_projects_report(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = read_committed_report()?;
        let receipt = TassadarArticleInterpreterOwnershipGateReceipt::from_report(&report);

        assert_eq!(receipt.tied_requirement_id, "TAS-184");
        assert!(receipt.blocked_issue_ids.is_empty());
        assert_eq!(receipt.generic_direct_proof_case_count, 6);
        assert!(receipt.generic_direct_proof_suite_green);
        assert_eq!(receipt.breadth_family_count, 8);
        assert!(receipt.route_purity_green);
        assert!(receipt.mapping_stable_across_runs);
        assert!(receipt.perturbation_sensitivity_green);
        assert!(receipt.interpreter_ownership_green);
        assert!(receipt.article_equivalence_green);
        Ok(())
    }
}
