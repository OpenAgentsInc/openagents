use serde::{Deserialize, Serialize};

use psionic_eval::{
    TassadarArticleCrossMachineReproducibilityReport,
    TassadarArticleCrossMachineStochasticModePosture,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityMatrixReceipt {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub blocked_issue_ids: Vec<String>,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub deterministic_mode_green: bool,
    pub throughput_floor_stability_green: bool,
    pub stochastic_mode_supported: bool,
    pub stochastic_mode_out_of_scope: bool,
    pub current_host_stochastic_mode_posture: TassadarArticleCrossMachineStochasticModePosture,
    pub reproducibility_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleCrossMachineReproducibilityMatrixReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleCrossMachineReproducibilityReport) -> Self {
        let current_host_posture = report
            .machine_rows
            .iter()
            .find(|row| {
                row.machine_class_id == report.machine_matrix_review.current_host_machine_class_id
            })
            .map(|row| row.stochastic_mode_posture)
            .unwrap_or(TassadarArticleCrossMachineStochasticModePosture::OutOfScope);
        Self {
            report_id: report.report_id.clone(),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            blocked_issue_ids: report.acceptance_gate_tie.blocked_issue_ids.clone(),
            current_host_machine_class_id: report
                .machine_matrix_review
                .current_host_machine_class_id
                .clone(),
            supported_machine_class_ids: report
                .machine_matrix_review
                .supported_machine_class_ids
                .clone(),
            deterministic_mode_green: report.deterministic_mode_green,
            throughput_floor_stability_green: report.throughput_floor_stability_green,
            stochastic_mode_supported: report.stochastic_mode_review.stochastic_mode_supported,
            stochastic_mode_out_of_scope: report.stochastic_mode_review.out_of_scope,
            current_host_stochastic_mode_posture: current_host_posture,
            reproducibility_matrix_green: report.reproducibility_matrix_green,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article cross-machine reproducibility matrix `{}` keeps tied_requirement_id={}, blocked_issues={}, current_host_machine_class_id=`{}`, supported_machine_classes={}, deterministic_mode_green={}, throughput_floor_stability_green={}, stochastic_mode_supported={}, stochastic_mode_out_of_scope={}, reproducibility_matrix_green={}, and article_equivalence_green={}",
                report.report_id,
                report.acceptance_gate_tie.tied_requirement_id,
                report.acceptance_gate_tie.blocked_issue_ids.len(),
                report.machine_matrix_review.current_host_machine_class_id,
                report.machine_matrix_review.supported_machine_class_ids.len(),
                report.deterministic_mode_green,
                report.throughput_floor_stability_green,
                report.stochastic_mode_review.stochastic_mode_supported,
                report.stochastic_mode_review.out_of_scope,
                report.reproducibility_matrix_green,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleCrossMachineReproducibilityMatrixReceipt;
    use psionic_eval::{
        TassadarArticleCrossMachineReproducibilityReport,
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
    };

    fn read_committed_report(
    ) -> Result<TassadarArticleCrossMachineReproducibilityReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path =
            repo_root.join(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_cross_machine_reproducibility_matrix_receipt_projects_report(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = read_committed_report()?;
        let receipt = TassadarArticleCrossMachineReproducibilityMatrixReceipt::from_report(&report);

        assert_eq!(receipt.tied_requirement_id, "TAS-185");
        assert!(receipt.blocked_issue_ids.is_empty());
        assert_eq!(receipt.supported_machine_class_ids.len(), 2);
        assert!(receipt.deterministic_mode_green);
        assert!(receipt.throughput_floor_stability_green);
        assert!(!receipt.stochastic_mode_supported);
        assert!(receipt.stochastic_mode_out_of_scope);
        assert!(receipt.reproducibility_matrix_green);
        assert!(receipt.article_equivalence_green);
        Ok(())
    }
}
