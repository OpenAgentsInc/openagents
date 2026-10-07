use serde::{Deserialize, Serialize};

use psionic_eval::TassadarArticleDemoBenchmarkEquivalenceGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoBenchmarkEquivalenceGateReceipt {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub blocked_issue_ids: Vec<String>,
    pub hungarian_demo_parity_green: bool,
    pub named_arto_parity_green: bool,
    pub benchmark_wide_sudoku_parity_green: bool,
    pub gate_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

impl TassadarArticleDemoBenchmarkEquivalenceGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarArticleDemoBenchmarkEquivalenceGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            blocked_issue_ids: report.acceptance_gate_tie.blocked_issue_ids.clone(),
            hungarian_demo_parity_green: report.hungarian_demo_parity_green,
            named_arto_parity_green: report.named_arto_parity_green,
            benchmark_wide_sudoku_parity_green: report.benchmark_wide_sudoku_parity_green,
            gate_green: report.article_demo_benchmark_equivalence_gate_green,
            article_equivalence_green: report.article_equivalence_green,
            detail: format!(
                "article demo-and-benchmark equivalence gate `{}` keeps tied_requirement_id={}, blocked_issues={}, hungarian_demo_parity_green={}, named_arto_parity_green={}, benchmark_wide_sudoku_parity_green={}, gate_green={}, article_equivalence_green={}",
                report.report_id,
                report.acceptance_gate_tie.tied_requirement_id,
                report.acceptance_gate_tie.blocked_issue_ids.len(),
                report.hungarian_demo_parity_green,
                report.named_arto_parity_green,
                report.benchmark_wide_sudoku_parity_green,
                report.article_demo_benchmark_equivalence_gate_green,
                report.article_equivalence_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarArticleDemoBenchmarkEquivalenceGateReceipt;
    use psionic_eval::build_tassadar_article_demo_benchmark_equivalence_gate_report;

    #[test]
    fn article_demo_benchmark_equivalence_gate_receipt_projects_report() {
        let report =
            build_tassadar_article_demo_benchmark_equivalence_gate_report().expect("report");
        let receipt = TassadarArticleDemoBenchmarkEquivalenceGateReceipt::from_report(&report);

        assert_eq!(receipt.tied_requirement_id, "TAS-182");
        assert_eq!(
            receipt.blocked_issue_ids.first().map(String::as_str),
            None
        );
        assert!(receipt.hungarian_demo_parity_green);
        assert!(receipt.named_arto_parity_green);
        assert!(receipt.benchmark_wide_sudoku_parity_green);
        assert!(receipt.gate_green);
        assert!(receipt.article_equivalence_green);
    }
}
