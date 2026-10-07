use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{TassadarPlannerRouteFamily, TassadarWorkloadClass};

pub const TASSADAR_RECEIPT_SUPERVISION_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_receipt_supervision_v1/receipt_supervision_evidence_bundle.json";
pub const TASSADAR_RECEIPT_SUPERVISION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_receipt_supervision_report.json";

const TASSADAR_RECEIPT_SUPERVISION_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarReceiptSupervisionPublicationStatus {
    Implemented,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarValidatorOutcome {
    Pass,
    Fail,
    ChallengeRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAcceptedOutcomeLabel {
    Accepted,
    RejectedForEvidence,
    RejectedForRefusalQuality,
    AcceptedAfterDelegation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarReceiptSupervisionPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarReceiptSupervisionPublicationStatus,
    pub claim_class: String,
    pub benchmarked_workload_classes: Vec<TassadarWorkloadClass>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarReceiptSupervisionPublication {
    fn new() -> Self {
        let mut benchmarked_workload_classes = vec![
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarWorkloadClass::LongLoopKernel,
            TassadarWorkloadClass::SudokuClass,
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarWorkloadClass::ClrsShortestPath,
        ];
        benchmarked_workload_classes.sort_by_key(|class| class.as_str());
        let mut publication = Self {
            schema_version: TASSADAR_RECEIPT_SUPERVISION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.receipt_supervision.publication.v1"),
            status: TassadarReceiptSupervisionPublicationStatus::Implemented,
            claim_class: String::from("research_only_architecture_policy_bound_routing"),
            benchmarked_workload_classes,
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-provider"),
                String::from("crates/psionic-train"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_RECEIPT_SUPERVISION_BUNDLE_REF),
                String::from(TASSADAR_RECEIPT_SUPERVISION_REPORT_REF),
                String::from(
                    "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_negative_invocation_report.json"),
            ],
            support_boundaries: vec![
                String::from(
                    "accepted outcomes remain explicit supervision inputs and challengeable labels; publication here does not move authority closure into the planner",
                ),
                String::from(
                    "receipt-supervised planner learning is benchmark-bound and research-only; it does not widen served capability or settlement posture",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_receipt_supervision_publication|",
            &publication,
        );
        publication
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarReceiptSupervisionCase {
    pub case_id: String,
    pub workload_class: TassadarWorkloadClass,
    pub heuristic_route_family: TassadarPlannerRouteFamily,
    pub receipt_supervised_route_family: TassadarPlannerRouteFamily,
    pub validator_outcome: TassadarValidatorOutcome,
    pub accepted_outcome_label: TassadarAcceptedOutcomeLabel,
    pub receipt_sources: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarReceiptSupervisionEvidenceBundle {
    pub publication: TassadarReceiptSupervisionPublication,
    pub cases: Vec<TassadarReceiptSupervisionCase>,
    pub heuristic_route_quality_bps: u32,
    pub receipt_supervised_route_quality_bps: u32,
    pub heuristic_refusal_quality_bps: u32,
    pub receipt_supervised_refusal_quality_bps: u32,
    pub heuristic_accepted_outcome_quality_bps: u32,
    pub receipt_supervised_accepted_outcome_quality_bps: u32,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarReceiptSupervisionReport {
    pub report_id: String,
    pub bundle_ref: String,
    pub heuristic_route_quality_bps: u32,
    pub receipt_supervised_route_quality_bps: u32,
    pub heuristic_refusal_quality_bps: u32,
    pub receipt_supervised_refusal_quality_bps: u32,
    pub heuristic_accepted_outcome_quality_bps: u32,
    pub receipt_supervised_accepted_outcome_quality_bps: u32,
    pub accepted_count: u32,
    pub rejected_for_evidence_count: u32,
    pub rejected_for_refusal_quality_count: u32,
    pub accepted_after_delegation_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn tassadar_receipt_supervision_publication() -> TassadarReceiptSupervisionPublication {
    TassadarReceiptSupervisionPublication::new()
}

#[must_use]
pub fn seeded_tassadar_receipt_supervision_cases() -> Vec<TassadarReceiptSupervisionCase> {
    vec![
        case(
            "accepted_outcome_exact_patch",
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarValidatorOutcome::Pass,
            TassadarAcceptedOutcomeLabel::Accepted,
            &[
                "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json",
                "fixtures/tassadar/reports/tassadar_negative_invocation_report.json",
            ],
            "receipts and validator outcome confirm the heuristic internal route was already correct",
        ),
        case(
            "long_loop_validator_mount",
            TassadarWorkloadClass::LongLoopKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            TassadarValidatorOutcome::ChallengeRequired,
            TassadarAcceptedOutcomeLabel::AcceptedAfterDelegation,
            &[
                "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json",
                "fixtures/tassadar/reports/tassadar_internal_external_delegation_benchmark_report.json",
            ],
            "receipt-aware planner should learn that the heuristic internal route needed delegation to close cleanly",
        ),
        case(
            "served_search_validator_mount",
            TassadarWorkloadClass::SudokuClass,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            TassadarValidatorOutcome::Fail,
            TassadarAcceptedOutcomeLabel::RejectedForEvidence,
            &[
                "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json",
                "fixtures/tassadar/reports/tassadar_internal_external_delegation_route_matrix.json",
            ],
            "receipt bundle shows the heuristic route lacked the evidence bar and should have delegated externally",
        ),
        case(
            "branch_heavy_control_repair",
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            TassadarValidatorOutcome::ChallengeRequired,
            TassadarAcceptedOutcomeLabel::RejectedForRefusalQuality,
            &[
                "fixtures/tassadar/reports/tassadar_internal_external_delegation_benchmark_report.json",
                "fixtures/tassadar/reports/tassadar_negative_invocation_report.json",
            ],
            "receipt-aware planner should learn that heuristic internal routing degraded refusal quality on this branch-heavy row",
        ),
        case(
            "clrs_shortest_path_verification",
            TassadarWorkloadClass::ClrsShortestPath,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            TassadarValidatorOutcome::Pass,
            TassadarAcceptedOutcomeLabel::Accepted,
            &[
                "fixtures/tassadar/reports/tassadar_internal_external_delegation_route_matrix.json",
                "fixtures/tassadar/reports/tassadar_internal_external_delegation_benchmark_report.json",
            ],
            "receipt-aware planner should route toward the externally validated authority path when the bridge lane remains partially open",
        ),
    ]
}

fn case(
    case_id: &str,
    workload_class: TassadarWorkloadClass,
    heuristic_route_family: TassadarPlannerRouteFamily,
    receipt_supervised_route_family: TassadarPlannerRouteFamily,
    validator_outcome: TassadarValidatorOutcome,
    accepted_outcome_label: TassadarAcceptedOutcomeLabel,
    receipt_sources: &[&str],
    note: &str,
) -> TassadarReceiptSupervisionCase {
    TassadarReceiptSupervisionCase {
        case_id: String::from(case_id),
        workload_class,
        heuristic_route_family,
        receipt_supervised_route_family,
        validator_outcome,
        accepted_outcome_label,
        receipt_sources: receipt_sources
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarReceiptSupervisionPublicationStatus, tassadar_receipt_supervision_publication,
    };
    use crate::TassadarWorkloadClass;

    #[test]
    fn receipt_supervision_publication_is_machine_legible() {
        let publication = tassadar_receipt_supervision_publication();

        assert_eq!(
            publication.status,
            TassadarReceiptSupervisionPublicationStatus::Implemented
        );
        assert!(
            publication
                .benchmarked_workload_classes
                .contains(&TassadarWorkloadClass::LongLoopKernel)
        );
        assert_eq!(publication.validation_refs.len(), 4);
    }
}
