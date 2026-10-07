use serde::{Deserialize, Serialize};
use sha2::Digest;

use psionic_models::{
    TassadarReceiptSupervisionCase, TassadarReceiptSupervisionPublication,
    seeded_tassadar_receipt_supervision_cases, tassadar_receipt_supervision_publication,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarReceiptSupervisionBundle {
    pub publication: TassadarReceiptSupervisionPublication,
    pub cases: Vec<TassadarReceiptSupervisionCase>,
    pub receipt_bundle_digest: String,
    pub detail: String,
}

#[must_use]
pub fn seeded_tassadar_receipt_supervision_bundle() -> TassadarReceiptSupervisionBundle {
    let publication = tassadar_receipt_supervision_publication();
    let cases = seeded_tassadar_receipt_supervision_cases();
    let receipt_bundle_digest = {
        let encoded = serde_json::to_vec(&cases).unwrap_or_default();
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    };
    TassadarReceiptSupervisionBundle {
        publication,
        receipt_bundle_digest,
        detail: String::from(
            "seeded receipt-supervision bundle keeps heuristic routes, receipt-aware routes, validator outcomes, and accepted-outcome labels explicit for planner training",
        ),
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::seeded_tassadar_receipt_supervision_bundle;
    use psionic_models::{TassadarAcceptedOutcomeLabel, TassadarPlannerRouteFamily};

    #[test]
    fn receipt_supervision_bundle_keeps_receipt_and_outcome_labels_explicit() {
        let bundle = seeded_tassadar_receipt_supervision_bundle();

        assert_eq!(bundle.cases.len(), 5);
        assert!(bundle.cases.iter().any(|case| {
            case.heuristic_route_family == TassadarPlannerRouteFamily::InternalExactCompute
                && case.receipt_supervised_route_family
                    != TassadarPlannerRouteFamily::InternalExactCompute
        }));
        assert!(bundle.cases.iter().any(|case| {
            case.accepted_outcome_label == TassadarAcceptedOutcomeLabel::AcceptedAfterDelegation
        }));
    }
}
