use serde::{Deserialize, Serialize};

use psionic_research::TassadarTuringCompletenessCloseoutSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTuringCompletenessCloseoutReceipt {
    pub report_id: String,
    pub claim_status: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub portability_envelope_ids: Vec<String>,
    pub refusal_boundary_ids: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub detail: String,
}

impl TassadarTuringCompletenessCloseoutReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarTuringCompletenessCloseoutSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            claim_status: match summary.eval_report.claim_status {
                psionic_eval::TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed => {
                    String::from("theory_green_operator_green_served_suppressed")
                }
                psionic_eval::TassadarTuringCompletenessCloseoutStatus::Incomplete => {
                    String::from("incomplete")
                }
            },
            theory_green: summary.eval_report.theory_green,
            operator_green: summary.eval_report.operator_green,
            served_green: summary.eval_report.served_green,
            portability_envelope_ids: summary.portability_envelope_ids.clone(),
            refusal_boundary_ids: summary.refusal_boundary_ids.clone(),
            explicit_non_implications: summary.explicit_non_implications.clone(),
            detail: format!(
                "turing-completeness closeout summary `{}` keeps claim_status={:?}, theory_green={}, operator_green={}, served_green={}, portability_envelopes={}, refusal_boundaries={}, explicit_non_implications={}",
                summary.report_id,
                summary.eval_report.claim_status,
                summary.eval_report.theory_green,
                summary.eval_report.operator_green,
                summary.eval_report.served_green,
                summary.portability_envelope_ids.len(),
                summary.refusal_boundary_ids.len(),
                summary.explicit_non_implications.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarTuringCompletenessCloseoutReceipt;
    use psionic_research::build_tassadar_turing_completeness_closeout_summary;

    #[test]
    fn turing_completeness_closeout_receipt_projects_summary() {
        let summary = build_tassadar_turing_completeness_closeout_summary().expect("summary");
        let receipt = TassadarTuringCompletenessCloseoutReceipt::from_summary(&summary);

        assert_eq!(
            receipt.claim_status,
            "theory_green_operator_green_served_suppressed"
        );
        assert!(receipt.theory_green);
        assert!(receipt.operator_green);
        assert!(!receipt.served_green);
        assert!(
            receipt
                .explicit_non_implications
                .contains(&String::from("public universality publication"))
        );
    }
}
