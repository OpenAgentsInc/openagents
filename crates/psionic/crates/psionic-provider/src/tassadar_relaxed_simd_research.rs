use serde::{Deserialize, Serialize};

use psionic_research::TassadarRelaxedSimdResearchSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdResearchReceipt {
    pub report_id: String,
    pub exact_anchor_backend_ids: Vec<String>,
    pub approximate_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub non_promotion_gate_reason_ids: Vec<String>,
    pub detail: String,
}

impl TassadarRelaxedSimdResearchReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarRelaxedSimdResearchSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            exact_anchor_backend_ids: summary.exact_anchor_backend_ids.clone(),
            approximate_backend_ids: summary.approximate_backend_ids.clone(),
            refused_backend_ids: summary.refused_backend_ids.clone(),
            non_promotion_gate_reason_ids: summary.non_promotion_gate_reason_ids.clone(),
            detail: format!(
                "relaxed-SIMD research summary `{}` keeps exact_anchor_backends={}, approximate_backends={}, refused_backends={}, non_promotion_gates={}",
                summary.report_id,
                summary.exact_anchor_backend_ids.len(),
                summary.approximate_backend_ids.len(),
                summary.refused_backend_ids.len(),
                summary.non_promotion_gate_reason_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarRelaxedSimdResearchReceipt;
    use psionic_research::build_tassadar_relaxed_simd_research_summary;

    #[test]
    fn relaxed_simd_research_receipt_projects_summary() {
        let summary = build_tassadar_relaxed_simd_research_summary().expect("summary");
        let receipt = TassadarRelaxedSimdResearchReceipt::from_summary(&summary);

        assert_eq!(receipt.exact_anchor_backend_ids.len(), 1);
        assert_eq!(receipt.approximate_backend_ids.len(), 2);
        assert_eq!(receipt.refused_backend_ids.len(), 2);
        assert_eq!(receipt.non_promotion_gate_reason_ids.len(), 3);
    }
}
