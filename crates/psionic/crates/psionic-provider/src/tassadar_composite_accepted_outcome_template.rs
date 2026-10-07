use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_router::{
    TassadarCompositeRouteLaneKind, TassadarCompositeRoutingCase,
    build_tassadar_composite_routing_report,
};

use crate::{
    TassadarAcceptedOutcomeBindingReport, TassadarCompositeRoutingReceipt,
    build_tassadar_accepted_outcome_binding_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COMPOSITE_ACCEPTED_OUTCOME_TEMPLATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_composite_accepted_outcome_template_report.json";

/// Simulation state for one composite accepted-outcome drill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompositeAcceptedOutcomeSimulationStatus {
    CandidateOnly,
    Accepted,
    Refused,
}

/// Typed refusal reason for one composite accepted-outcome drill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompositeAcceptedOutcomeRefusalReason {
    MissingLaneEvidence,
    MissingReceiptStitch,
    ChallengeWindowOpen,
    SettlementDependencyMissing,
}

/// One per-lane evidence obligation inside a hybrid accepted-outcome template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeLaneEvidenceObligation {
    pub lane_id: String,
    pub lane_kind: TassadarCompositeRouteLaneKind,
    pub required_evidence_refs: Vec<String>,
    pub stitched_receipt_id: String,
    pub note: String,
}

/// One authority-facing hybrid accepted-outcome template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeAcceptedOutcomeTemplate {
    pub template_id: String,
    pub route_case_id: String,
    pub workload_family: String,
    pub world_mount_policy_ref: String,
    pub kernel_policy_template_ref: String,
    pub nexus_template_ref: String,
    pub compute_market_product_ref: String,
    pub lane_obligations: Vec<TassadarCompositeLaneEvidenceObligation>,
    pub note: String,
}

/// One composite accepted-outcome simulation case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeAcceptedOutcomeSimulationCase {
    pub case_id: String,
    pub template: TassadarCompositeAcceptedOutcomeTemplate,
    pub candidate_outcome_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    pub stitched_receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_evidence_lane_ids: Vec<String>,
    pub challenge_window_open: bool,
    pub settlement_dependencies_satisfied: bool,
    pub status: TassadarCompositeAcceptedOutcomeSimulationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarCompositeAcceptedOutcomeRefusalReason>,
    pub note: String,
}

/// Receipt summarizing the single-lane accepted-outcome bridge that this report builds on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptedOutcomeBindingBridgeReceipt {
    pub report_id: String,
    pub accepted_case_count: u32,
    pub candidate_only_case_count: u32,
    pub refused_case_count: u32,
    pub settlement_gated_refusal_count: u32,
    pub detail: String,
}

impl TassadarAcceptedOutcomeBindingBridgeReceipt {
    /// Builds a bridge receipt from the accepted-outcome binding report.
    #[must_use]
    pub fn from_report(report: &TassadarAcceptedOutcomeBindingReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            accepted_case_count: report.accepted_case_count,
            candidate_only_case_count: report.candidate_only_case_count,
            refused_case_count: report.refused_case_count,
            settlement_gated_refusal_count: report.settlement_gated_refusal_count,
            detail: format!(
                "accepted-outcome binding `{}` freezes accepted={}, candidate_only={}, refused={}, settlement_gated_refusals={}",
                report.report_id,
                report.accepted_case_count,
                report.candidate_only_case_count,
                report.refused_case_count,
                report.settlement_gated_refusal_count,
            ),
        }
    }
}

/// Provider-facing report for hybrid accepted-outcome templates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeAcceptedOutcomeTemplateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub composite_routing_receipt: TassadarCompositeRoutingReceipt,
    pub accepted_outcome_binding_receipt: TassadarAcceptedOutcomeBindingBridgeReceipt,
    pub simulation_cases: Vec<TassadarCompositeAcceptedOutcomeSimulationCase>,
    pub accepted_case_count: u32,
    pub candidate_only_case_count: u32,
    pub refused_case_count: u32,
    pub missing_evidence_refusal_count: u32,
    pub missing_receipt_stitch_refusal_count: u32,
    pub nexus_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Provider-facing receipt summarizing the hybrid accepted-outcome template report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeAcceptedOutcomeTemplateReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub accepted_case_count: u32,
    pub candidate_only_case_count: u32,
    pub refused_case_count: u32,
    pub missing_evidence_refusal_count: u32,
    pub missing_receipt_stitch_refusal_count: u32,
    pub detail: String,
}

impl TassadarCompositeAcceptedOutcomeTemplateReceipt {
    /// Builds a provider-facing receipt from the hybrid accepted-outcome template report.
    #[must_use]
    pub fn from_report(report: &TassadarCompositeAcceptedOutcomeTemplateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.simulation_cases.len() as u32,
            accepted_case_count: report.accepted_case_count,
            candidate_only_case_count: report.candidate_only_case_count,
            refused_case_count: report.refused_case_count,
            missing_evidence_refusal_count: report.missing_evidence_refusal_count,
            missing_receipt_stitch_refusal_count: report.missing_receipt_stitch_refusal_count,
            detail: format!(
                "composite accepted-outcome templates `{}` cover {} cases with accepted={}, candidate_only={}, refused={}, missing_evidence_refusals={}, missing_receipt_stitch_refusals={}",
                report.report_id,
                report.simulation_cases.len(),
                report.accepted_case_count,
                report.candidate_only_case_count,
                report.refused_case_count,
                report.missing_evidence_refusal_count,
                report.missing_receipt_stitch_refusal_count,
            ),
        }
    }
}

/// Builds the committed composite accepted-outcome template report.
#[must_use]
pub fn build_tassadar_composite_accepted_outcome_template_report()
-> TassadarCompositeAcceptedOutcomeTemplateReport {
    let composite_routing_report = build_tassadar_composite_routing_report();
    let accepted_outcome_binding_report = build_tassadar_accepted_outcome_binding_report();
    let composite_routing_receipt =
        TassadarCompositeRoutingReceipt::from_report(&composite_routing_report);
    let accepted_outcome_binding_receipt =
        TassadarAcceptedOutcomeBindingBridgeReceipt::from_report(&accepted_outcome_binding_report);
    let simulation_cases = seeded_simulation_cases(&composite_routing_report.evaluated_cases);
    let accepted_case_count = simulation_cases
        .iter()
        .filter(|case| case.status == TassadarCompositeAcceptedOutcomeSimulationStatus::Accepted)
        .count() as u32;
    let candidate_only_case_count = simulation_cases
        .iter()
        .filter(|case| {
            case.status == TassadarCompositeAcceptedOutcomeSimulationStatus::CandidateOnly
        })
        .count() as u32;
    let refused_case_count = simulation_cases
        .iter()
        .filter(|case| case.status == TassadarCompositeAcceptedOutcomeSimulationStatus::Refused)
        .count() as u32;
    let missing_evidence_refusal_count = simulation_cases
        .iter()
        .filter(|case| {
            case.refusal_reason
                == Some(TassadarCompositeAcceptedOutcomeRefusalReason::MissingLaneEvidence)
        })
        .count() as u32;
    let missing_receipt_stitch_refusal_count = simulation_cases
        .iter()
        .filter(|case| {
            case.refusal_reason
                == Some(TassadarCompositeAcceptedOutcomeRefusalReason::MissingReceiptStitch)
        })
        .count() as u32;
    let mut report = TassadarCompositeAcceptedOutcomeTemplateReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.composite_accepted_outcome_template.report.v1"),
        composite_routing_receipt,
        accepted_outcome_binding_receipt,
        simulation_cases,
        accepted_case_count,
        candidate_only_case_count,
        refused_case_count,
        missing_evidence_refusal_count,
        missing_receipt_stitch_refusal_count,
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical accepted-outcome issuance, dispute closure, and settlement transition authority outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of canonical accepted-outcome template approval and settlement-qualified policy closure outside standalone psionic",
        ),
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical task-scoped mount policy and hybrid-lane admissibility outside standalone psionic",
        ),
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical hybrid product, quote, and settlement-facing economic posture outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this provider report keeps hybrid lane evidence obligations, receipt stitching, candidate outcomes, accepted outcomes, and settlement posture explicit for composite routes. It does not treat runtime success or stitched provider receipts as accepted-outcome or settlement authority",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Composite accepted-outcome templates cover {} cases with accepted={}, candidate_only={}, refused={}, missing_evidence_refusals={}, and missing_receipt_stitch_refusals={}.",
        report.simulation_cases.len(),
        report.accepted_case_count,
        report.candidate_only_case_count,
        report.refused_case_count,
        report.missing_evidence_refusal_count,
        report.missing_receipt_stitch_refusal_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_composite_accepted_outcome_template_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed hybrid accepted-outcome template report.
#[must_use]
pub fn tassadar_composite_accepted_outcome_template_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPOSITE_ACCEPTED_OUTCOME_TEMPLATE_REPORT_REF)
}

/// Writes the committed hybrid accepted-outcome template report.
pub fn write_tassadar_composite_accepted_outcome_template_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompositeAcceptedOutcomeTemplateReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_composite_accepted_outcome_template_report();
    let json = serde_json::to_string_pretty(&report)
        .expect("composite accepted-outcome template report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_composite_accepted_outcome_template_report(
    path: impl AsRef<Path>,
) -> Result<TassadarCompositeAcceptedOutcomeTemplateReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn seeded_simulation_cases(
    route_cases: &[TassadarCompositeRoutingCase],
) -> Vec<TassadarCompositeAcceptedOutcomeSimulationCase> {
    let patch_route = route_case(route_cases, "composite.patch_validator_bridge");
    let long_loop_route = route_case(route_cases, "composite.long_loop_sandbox_fallback");
    let article_route = route_case(route_cases, "composite.gpu_cluster_article");
    let parity_route = route_case(route_cases, "single_lane.module_parity");

    let accepted_patch_template = template_for_case(
        patch_route,
        "kernel-policy.accepted-outcome.composite.patch_apply.v1",
        "nexus.accepted-outcome.composite.patch_apply.v1",
        "compute-market.hybrid_exact_compute.patch_apply.v1",
        "patch-validator bridge requires planner, exact-compute, CPU-reference, and validator evidence under one stitched template",
    );
    let candidate_long_loop_template = template_for_case(
        long_loop_route,
        "kernel-policy.accepted-outcome.composite.long_loop.v1",
        "nexus.accepted-outcome.composite.long_loop.v1",
        "compute-market.hybrid_exact_compute.long_loop.v1",
        "long-loop hybrid routes keep fallback, sandbox, and validator receipts explicit while challenge posture remains open",
    );
    let article_template = template_for_case(
        article_route,
        "kernel-policy.accepted-outcome.composite.article_hybrid.v1",
        "nexus.accepted-outcome.composite.article_hybrid.v1",
        "compute-market.hybrid_exact_compute.article.v1",
        "GPU plus cluster article routes require stitched lane evidence before any authority-facing outcome can be accepted",
    );
    let parity_template = template_for_case(
        parity_route,
        "kernel-policy.accepted-outcome.composite.parity.v1",
        "nexus.accepted-outcome.composite.parity.v1",
        "compute-market.hybrid_exact_compute.parity.v1",
        "short bounded parity can still be modeled as a composite template, but missing stitched receipts must refuse instead of silently collapsing to one success bit",
    );
    let settlement_patch_template = template_for_case(
        patch_route,
        "kernel-policy.accepted-outcome.composite.patch_apply.v2",
        "nexus.accepted-outcome.composite.patch_apply.v2",
        "compute-market.hybrid_exact_compute.patch_apply.v2",
        "settlement-facing patch template keeps per-lane evidence satisfied while still refusing if the settlement bridge has not closed",
    );

    vec![
        TassadarCompositeAcceptedOutcomeSimulationCase {
            case_id: String::from("accepted.composite.patch_validator_bridge"),
            candidate_outcome_id: String::from("candidate.composite.patch_validator_bridge.v1"),
            accepted_outcome_id: Some(String::from("accepted.composite.patch_validator_bridge.v1")),
            stitched_receipt_ids: accepted_patch_template
                .lane_obligations
                .iter()
                .map(|lane| lane.stitched_receipt_id.clone())
                .collect(),
            missing_evidence_lane_ids: Vec::new(),
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarCompositeAcceptedOutcomeSimulationStatus::Accepted,
            refusal_reason: None,
            note: String::from(
                "hybrid patch work reaches accepted-outcome readiness only because every lane carries explicit evidence and the stitched receipts line up",
            ),
            template: accepted_patch_template,
        },
        TassadarCompositeAcceptedOutcomeSimulationCase {
            case_id: String::from("candidate.composite.long_loop_sandbox_fallback"),
            candidate_outcome_id: String::from("candidate.composite.long_loop_sandbox_fallback.v1"),
            accepted_outcome_id: None,
            stitched_receipt_ids: candidate_long_loop_template
                .lane_obligations
                .iter()
                .map(|lane| lane.stitched_receipt_id.clone())
                .collect(),
            missing_evidence_lane_ids: Vec::new(),
            challenge_window_open: true,
            settlement_dependencies_satisfied: true,
            status: TassadarCompositeAcceptedOutcomeSimulationStatus::CandidateOnly,
            refusal_reason: Some(
                TassadarCompositeAcceptedOutcomeRefusalReason::ChallengeWindowOpen,
            ),
            note: String::from(
                "candidate and accepted outcomes stay distinct while the long-loop fallback route remains challengeable",
            ),
            template: candidate_long_loop_template,
        },
        TassadarCompositeAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.composite.gpu_cluster_article.missing_lane_evidence"),
            candidate_outcome_id: String::from(
                "candidate.composite.gpu_cluster_article.missing_lane_evidence.v1",
            ),
            accepted_outcome_id: None,
            stitched_receipt_ids: article_template
                .lane_obligations
                .iter()
                .filter(|lane| lane.lane_id != "cluster_fanout")
                .map(|lane| lane.stitched_receipt_id.clone())
                .collect(),
            missing_evidence_lane_ids: vec![String::from("cluster_fanout")],
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarCompositeAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(
                TassadarCompositeAcceptedOutcomeRefusalReason::MissingLaneEvidence,
            ),
            note: String::from(
                "cluster fanout evidence is required for the hybrid article route, so missing that lane's evidence must refuse instead of collapsing to composite success",
            ),
            template: article_template,
        },
        TassadarCompositeAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.single_lane.module_parity.missing_receipt_stitch"),
            candidate_outcome_id: String::from(
                "candidate.single_lane.module_parity.missing_receipt_stitch.v1",
            ),
            accepted_outcome_id: None,
            stitched_receipt_ids: parity_template
                .lane_obligations
                .iter()
                .filter(|lane| lane.lane_id != "validator")
                .map(|lane| lane.stitched_receipt_id.clone())
                .collect(),
            missing_evidence_lane_ids: Vec::new(),
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarCompositeAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(
                TassadarCompositeAcceptedOutcomeRefusalReason::MissingReceiptStitch,
            ),
            note: String::from(
                "the optional validator lane cannot disappear inside receipt stitching; if one stitched receipt is missing, the hybrid template must refuse",
            ),
            template: parity_template,
        },
        TassadarCompositeAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.composite.patch_validator_bridge.settlement_missing"),
            candidate_outcome_id: String::from(
                "candidate.composite.patch_validator_bridge.settlement_missing.v1",
            ),
            accepted_outcome_id: None,
            stitched_receipt_ids: settlement_patch_template
                .lane_obligations
                .iter()
                .map(|lane| lane.stitched_receipt_id.clone())
                .collect(),
            missing_evidence_lane_ids: Vec::new(),
            challenge_window_open: false,
            settlement_dependencies_satisfied: false,
            status: TassadarCompositeAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(
                TassadarCompositeAcceptedOutcomeRefusalReason::SettlementDependencyMissing,
            ),
            note: String::from(
                "all lane receipts can be present while settlement-facing dependencies still refuse the accepted outcome",
            ),
            template: settlement_patch_template,
        },
    ]
}

fn template_for_case(
    route_case: &TassadarCompositeRoutingCase,
    kernel_policy_template_ref: &str,
    nexus_template_ref: &str,
    compute_market_product_ref: &str,
    note: &str,
) -> TassadarCompositeAcceptedOutcomeTemplate {
    TassadarCompositeAcceptedOutcomeTemplate {
        template_id: format!("accepted_outcome.template.{}", route_case.case_id),
        route_case_id: route_case.case_id.clone(),
        workload_family: route_case.workload_family.clone(),
        world_mount_policy_ref: format!("world-mounts.policy.{}", route_case.mount_id),
        kernel_policy_template_ref: String::from(kernel_policy_template_ref),
        nexus_template_ref: String::from(nexus_template_ref),
        compute_market_product_ref: String::from(compute_market_product_ref),
        lane_obligations: route_case
            .composite_steps
            .iter()
            .map(|step| TassadarCompositeLaneEvidenceObligation {
                lane_id: step.step_id.clone(),
                lane_kind: step.lane_kind,
                required_evidence_refs: vec![step.evidence_ref.clone()],
                stitched_receipt_id: format!(
                    "stitched_receipt.{}.{}.v1",
                    route_case.case_id.replace('.', "_"),
                    step.step_id
                ),
                note: step.note.clone(),
            })
            .collect(),
        note: String::from(note),
    }
}

fn route_case<'a>(
    route_cases: &'a [TassadarCompositeRoutingCase],
    case_id: &str,
) -> &'a TassadarCompositeRoutingCase {
    route_cases
        .iter()
        .find(|case| case.case_id == case_id)
        .expect("composite route case should exist")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        TassadarCompositeAcceptedOutcomeRefusalReason,
        TassadarCompositeAcceptedOutcomeSimulationStatus,
        TassadarCompositeAcceptedOutcomeTemplateReceipt,
        build_tassadar_composite_accepted_outcome_template_report,
        load_tassadar_composite_accepted_outcome_template_report,
        tassadar_composite_accepted_outcome_template_report_path,
    };

    #[test]
    fn composite_accepted_outcome_template_report_keeps_lane_evidence_and_stitching_explicit() {
        let report = build_tassadar_composite_accepted_outcome_template_report();

        assert_eq!(report.simulation_cases.len(), 5);
        assert!(report.simulation_cases.iter().any(|case| {
            case.case_id == "accepted.composite.patch_validator_bridge"
                && case.status == TassadarCompositeAcceptedOutcomeSimulationStatus::Accepted
                && case.template.lane_obligations.len() == 4
                && case.stitched_receipt_ids.len() == 4
        }));
        assert!(report.simulation_cases.iter().any(|case| {
            case.case_id == "refused.composite.gpu_cluster_article.missing_lane_evidence"
                && case.refusal_reason
                    == Some(TassadarCompositeAcceptedOutcomeRefusalReason::MissingLaneEvidence)
                && case
                    .missing_evidence_lane_ids
                    .contains(&String::from("cluster_fanout"))
        }));
        assert!(report.simulation_cases.iter().any(|case| {
            case.case_id == "refused.single_lane.module_parity.missing_receipt_stitch"
                && case.refusal_reason
                    == Some(TassadarCompositeAcceptedOutcomeRefusalReason::MissingReceiptStitch)
        }));
    }

    #[test]
    fn composite_accepted_outcome_template_report_matches_committed_truth() {
        let expected = build_tassadar_composite_accepted_outcome_template_report();
        let committed = load_tassadar_composite_accepted_outcome_template_report(
            tassadar_composite_accepted_outcome_template_report_path(),
        )
        .expect("committed composite accepted-outcome template report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn composite_accepted_outcome_template_receipt_projects_report() {
        let report = build_tassadar_composite_accepted_outcome_template_report();
        let receipt = TassadarCompositeAcceptedOutcomeTemplateReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 5);
        assert_eq!(receipt.accepted_case_count, 1);
        assert_eq!(receipt.candidate_only_case_count, 1);
        assert_eq!(receipt.refused_case_count, 3);
        assert_eq!(receipt.missing_evidence_refusal_count, 1);
        assert_eq!(receipt.missing_receipt_stitch_refusal_count, 1);
    }
}
