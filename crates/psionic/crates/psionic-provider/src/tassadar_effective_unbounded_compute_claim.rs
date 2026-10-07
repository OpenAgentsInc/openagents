use serde::{Deserialize, Serialize};

use psionic_research::TassadarEffectiveUnboundedComputeClaimSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectiveUnboundedComputeClaimReceipt {
    pub report_id: String,
    pub claim_status: String,
    pub blocked_by: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub detail: String,
}

impl TassadarEffectiveUnboundedComputeClaimReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarEffectiveUnboundedComputeClaimSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            claim_status: format!("{:?}", summary.eval_report.claim_status).to_lowercase(),
            blocked_by: summary.blocked_by.clone(),
            explicit_non_implications: summary.explicit_non_implications.clone(),
            detail: format!(
                "effective-unbounded claim summary `{}` keeps claim_status={:?}, blocked_by={}, explicit_non_implications={}",
                summary.report_id,
                summary.eval_report.claim_status,
                summary.blocked_by.len(),
                summary.explicit_non_implications.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarEffectiveUnboundedComputeClaimReceipt;
    use psionic_research::build_tassadar_effective_unbounded_compute_claim_summary;

    #[test]
    fn effective_unbounded_compute_claim_receipt_projects_summary() {
        let summary = build_tassadar_effective_unbounded_compute_claim_summary().expect("summary");
        let receipt = TassadarEffectiveUnboundedComputeClaimReceipt::from_summary(&summary);

        assert_eq!(receipt.claim_status, "suppressed");
        assert!(receipt
            .blocked_by
            .contains(&String::from("broad_publication_gate")));
        assert!(receipt
            .explicit_non_implications
            .contains(&String::from("arbitrary Wasm execution")));
    }
}
