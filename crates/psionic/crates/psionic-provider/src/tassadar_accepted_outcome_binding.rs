use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_router::{
    build_tassadar_evidence_calibrated_routing_report,
    build_tassadar_world_mount_compatibility_report,
};
use psionic_serve::build_tassadar_execution_unit_registration_report;

use crate::{
    TassadarEvidenceCalibratedRoutingReceipt, TassadarExecutionUnitRegistrationReceipt,
    TassadarWorldMountCompatibilityReceipt,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json";

/// Evidence family required before a candidate outcome can be accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAcceptedOutcomeEvidenceFamily {
    ExecutionReceipt,
    CanonicalTraceBundle,
    BenchmarkLineage,
    WorldMountCompatibilityReceipt,
    ValidatorVerdict,
}

/// Outcome of one accepted-outcome simulation drill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAcceptedOutcomeSimulationStatus {
    CandidateOnly,
    Accepted,
    Refused,
}

/// Typed refusal reason when runtime success is insufficient for acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAcceptedOutcomeRefusalReason {
    ValidatorAttachmentMissing,
    ValidatorPolicyMismatch,
    ChallengeWindowOpen,
    SettlementDependencyMissing,
}

/// One accepted-outcome template for an exact-compute job family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptedOutcomeTemplate {
    pub template_id: String,
    pub exact_compute_job_class: String,
    pub execution_unit_receipt_id: String,
    pub world_mount_receipt_id: String,
    pub evidence_routing_receipt_id: String,
    pub required_evidence_families: Vec<TassadarAcceptedOutcomeEvidenceFamily>,
    pub validator_policy_ref: String,
    pub validator_attachment_required: bool,
    pub challenge_window_minutes: u32,
    pub settlement_dependency_refs: Vec<String>,
    pub note: String,
}

/// One seeded accepted-outcome simulation case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptedOutcomeSimulationCase {
    pub case_id: String,
    pub template: TassadarAcceptedOutcomeTemplate,
    pub runtime_success: bool,
    pub candidate_outcome_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    pub provider_receipt_id: String,
    pub validator_attachment_present: bool,
    pub validator_policy_matches: bool,
    pub challenge_window_open: bool,
    pub settlement_dependencies_satisfied: bool,
    pub status: TassadarAcceptedOutcomeSimulationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarAcceptedOutcomeRefusalReason>,
    pub note: String,
}

/// Provider-facing report bridging exact-compute execution truth into accepted-outcome readiness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptedOutcomeBindingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub execution_unit_receipt: TassadarExecutionUnitRegistrationReceipt,
    pub world_mount_receipt: TassadarWorldMountCompatibilityReceipt,
    pub evidence_routing_receipt: TassadarEvidenceCalibratedRoutingReceipt,
    pub simulation_cases: Vec<TassadarAcceptedOutcomeSimulationCase>,
    pub accepted_case_count: u32,
    pub candidate_only_case_count: u32,
    pub refused_case_count: u32,
    pub validator_required_case_count: u32,
    pub settlement_gated_refusal_count: u32,
    pub nexus_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub validator_policies_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the committed accepted-outcome binding report.
#[must_use]
pub fn build_tassadar_accepted_outcome_binding_report() -> TassadarAcceptedOutcomeBindingReport {
    let execution_unit_report =
        build_tassadar_execution_unit_registration_report().expect("execution unit report");
    let world_mount_report = build_tassadar_world_mount_compatibility_report();
    let evidence_routing_report =
        build_tassadar_evidence_calibrated_routing_report().expect("evidence routing report");

    let execution_unit_receipt =
        TassadarExecutionUnitRegistrationReceipt::from_report(&execution_unit_report);
    let world_mount_receipt =
        TassadarWorldMountCompatibilityReceipt::from_report(&world_mount_report);
    let evidence_routing_receipt =
        TassadarEvidenceCalibratedRoutingReceipt::from_report(&evidence_routing_report);

    let simulation_cases = seeded_simulation_cases(
        &execution_unit_receipt,
        &world_mount_receipt,
        &evidence_routing_receipt,
    );
    let accepted_case_count = simulation_cases
        .iter()
        .filter(|case| case.status == TassadarAcceptedOutcomeSimulationStatus::Accepted)
        .count() as u32;
    let candidate_only_case_count = simulation_cases
        .iter()
        .filter(|case| case.status == TassadarAcceptedOutcomeSimulationStatus::CandidateOnly)
        .count() as u32;
    let refused_case_count = simulation_cases
        .iter()
        .filter(|case| case.status == TassadarAcceptedOutcomeSimulationStatus::Refused)
        .count() as u32;
    let validator_required_case_count = simulation_cases
        .iter()
        .filter(|case| case.template.validator_attachment_required)
        .count() as u32;
    let settlement_gated_refusal_count = simulation_cases
        .iter()
        .filter(|case| {
            case.refusal_reason
                == Some(TassadarAcceptedOutcomeRefusalReason::SettlementDependencyMissing)
        })
        .count() as u32;

    let mut report = TassadarAcceptedOutcomeBindingReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.accepted_outcome_binding.report.v1"),
        execution_unit_receipt,
        world_mount_receipt,
        evidence_routing_receipt,
        simulation_cases,
        accepted_case_count,
        candidate_only_case_count,
        refused_case_count,
        validator_required_case_count,
        settlement_gated_refusal_count,
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical accepted-outcome issuance and cross-provider dispute closure outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of settlement-qualified outcome authority outside standalone psionic",
        ),
        validator_policies_dependency_marker: String::from(
            "validator-policies remain the owner of canonical validator attachment rules and challenge adjudication outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this provider report keeps candidate outcomes, accepted-outcome templates, validator posture, challenge windows, and settlement dependencies explicit for exact-compute jobs. It does not treat runtime success or provider receipts as accepted-outcome or settlement authority",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Accepted-outcome binding report now freezes {} accepted cases, {} candidate-only cases, and {} refused cases across exact-compute validator and settlement drills.",
        report.accepted_case_count, report.candidate_only_case_count, report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_accepted_outcome_binding_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed accepted-outcome binding report.
#[must_use]
pub fn tassadar_accepted_outcome_binding_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF)
}

/// Writes the committed accepted-outcome binding report.
pub fn write_tassadar_accepted_outcome_binding_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarAcceptedOutcomeBindingReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_accepted_outcome_binding_report();
    let json =
        serde_json::to_string_pretty(&report).expect("accepted-outcome binding report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_accepted_outcome_binding_report(
    path: impl AsRef<Path>,
) -> Result<TassadarAcceptedOutcomeBindingReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn seeded_simulation_cases(
    execution_unit_receipt: &TassadarExecutionUnitRegistrationReceipt,
    world_mount_receipt: &TassadarWorldMountCompatibilityReceipt,
    evidence_routing_receipt: &TassadarEvidenceCalibratedRoutingReceipt,
) -> Vec<TassadarAcceptedOutcomeSimulationCase> {
    vec![
        TassadarAcceptedOutcomeSimulationCase {
            case_id: String::from("accepted.patch_apply.validator_bound"),
            template: TassadarAcceptedOutcomeTemplate {
                template_id: String::from("accepted_outcome.patch_apply.internal_exact.v1"),
                exact_compute_job_class: String::from("patch_apply_internal_exact"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                world_mount_receipt_id: world_mount_receipt.report_id.clone(),
                evidence_routing_receipt_id: evidence_routing_receipt.report_id.clone(),
                required_evidence_families: vec![
                    TassadarAcceptedOutcomeEvidenceFamily::ExecutionReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::CanonicalTraceBundle,
                    TassadarAcceptedOutcomeEvidenceFamily::BenchmarkLineage,
                    TassadarAcceptedOutcomeEvidenceFamily::WorldMountCompatibilityReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::ValidatorVerdict,
                ],
                validator_policy_ref: String::from(
                    "validator-policies.exact_compute.patch_apply.v1",
                ),
                validator_attachment_required: true,
                challenge_window_minutes: 0,
                settlement_dependency_refs: vec![
                    String::from("kernel-policy.accepted-outcome.patch_apply.v1"),
                    String::from("nexus.accepted-outcome.patch_apply.v1"),
                ],
                note: String::from(
                    "fully satisfied exact-compute patch path where runtime truth, validator truth, and settlement dependencies all line up",
                ),
            },
            runtime_success: true,
            candidate_outcome_id: String::from("candidate.patch_apply.validator_bound.v1"),
            accepted_outcome_id: Some(String::from("accepted.patch_apply.validator_bound.v1")),
            provider_receipt_id: String::from("provider_receipt.patch_apply.validator_bound.v1"),
            validator_attachment_present: true,
            validator_policy_matches: true,
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarAcceptedOutcomeSimulationStatus::Accepted,
            refusal_reason: None,
            note: String::from(
                "runtime success is necessary here, but accepted-outcome closure only happens because validator and settlement requirements are also satisfied",
            ),
        },
        TassadarAcceptedOutcomeSimulationCase {
            case_id: String::from("candidate.search.challenge_window"),
            template: TassadarAcceptedOutcomeTemplate {
                template_id: String::from("accepted_outcome.search.validator_challenge.v1"),
                exact_compute_job_class: String::from("search_validator_challenge"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                world_mount_receipt_id: world_mount_receipt.report_id.clone(),
                evidence_routing_receipt_id: evidence_routing_receipt.report_id.clone(),
                required_evidence_families: vec![
                    TassadarAcceptedOutcomeEvidenceFamily::ExecutionReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::CanonicalTraceBundle,
                    TassadarAcceptedOutcomeEvidenceFamily::ValidatorVerdict,
                ],
                validator_policy_ref: String::from("validator-policies.exact_compute.search.v1"),
                validator_attachment_required: true,
                challenge_window_minutes: 60,
                settlement_dependency_refs: vec![String::from(
                    "nexus.accepted-outcome.search.challenge.v1",
                )],
                note: String::from(
                    "validator-attached search stays candidate-only while the challenge window remains open",
                ),
            },
            runtime_success: true,
            candidate_outcome_id: String::from("candidate.search.challenge_window.v1"),
            accepted_outcome_id: None,
            provider_receipt_id: String::from("provider_receipt.search.challenge_window.v1"),
            validator_attachment_present: true,
            validator_policy_matches: true,
            challenge_window_open: true,
            settlement_dependencies_satisfied: true,
            status: TassadarAcceptedOutcomeSimulationStatus::CandidateOnly,
            refusal_reason: Some(TassadarAcceptedOutcomeRefusalReason::ChallengeWindowOpen),
            note: String::from(
                "candidate and accepted outcomes stay distinct because challenge posture has not yet closed",
            ),
        },
        TassadarAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.long_loop.validator_missing"),
            template: TassadarAcceptedOutcomeTemplate {
                template_id: String::from("accepted_outcome.long_loop.validator_required.v1"),
                exact_compute_job_class: String::from("long_loop_validator_heavy"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                world_mount_receipt_id: world_mount_receipt.report_id.clone(),
                evidence_routing_receipt_id: evidence_routing_receipt.report_id.clone(),
                required_evidence_families: vec![
                    TassadarAcceptedOutcomeEvidenceFamily::ExecutionReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::CanonicalTraceBundle,
                    TassadarAcceptedOutcomeEvidenceFamily::WorldMountCompatibilityReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::ValidatorVerdict,
                ],
                validator_policy_ref: String::from("validator-policies.exact_compute.long_loop.v1"),
                validator_attachment_required: true,
                challenge_window_minutes: 0,
                settlement_dependency_refs: vec![String::from(
                    "kernel-policy.accepted-outcome.long_loop.v1",
                )],
                note: String::from(
                    "long-loop accepted outcomes stay refused when validator attachment is missing even if runtime execution succeeded",
                ),
            },
            runtime_success: true,
            candidate_outcome_id: String::from("candidate.long_loop.validator_missing.v1"),
            accepted_outcome_id: None,
            provider_receipt_id: String::from("provider_receipt.long_loop.validator_missing.v1"),
            validator_attachment_present: false,
            validator_policy_matches: false,
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(TassadarAcceptedOutcomeRefusalReason::ValidatorAttachmentMissing),
            note: String::from(
                "provider-side execution receipts alone cannot escalate this case into accepted-outcome closure",
            ),
        },
        TassadarAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.search.validator_mismatch"),
            template: TassadarAcceptedOutcomeTemplate {
                template_id: String::from("accepted_outcome.search.policy_match.v1"),
                exact_compute_job_class: String::from("served_search_validator_mount"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                world_mount_receipt_id: world_mount_receipt.report_id.clone(),
                evidence_routing_receipt_id: evidence_routing_receipt.report_id.clone(),
                required_evidence_families: vec![
                    TassadarAcceptedOutcomeEvidenceFamily::ExecutionReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::CanonicalTraceBundle,
                    TassadarAcceptedOutcomeEvidenceFamily::ValidatorVerdict,
                ],
                validator_policy_ref: String::from(
                    "validator-policies.exact_compute.search.strict.v1",
                ),
                validator_attachment_required: true,
                challenge_window_minutes: 0,
                settlement_dependency_refs: vec![String::from(
                    "kernel-policy.accepted-outcome.search.strict.v1",
                )],
                note: String::from(
                    "validator-attached search still refuses when the validator policy family does not match the mount contract",
                ),
            },
            runtime_success: true,
            candidate_outcome_id: String::from("candidate.search.validator_mismatch.v1"),
            accepted_outcome_id: None,
            provider_receipt_id: String::from("provider_receipt.search.validator_mismatch.v1"),
            validator_attachment_present: true,
            validator_policy_matches: false,
            challenge_window_open: false,
            settlement_dependencies_satisfied: true,
            status: TassadarAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(TassadarAcceptedOutcomeRefusalReason::ValidatorPolicyMismatch),
            note: String::from(
                "validator presence is not enough; the validator family must match the exact accepted-outcome template",
            ),
        },
        TassadarAcceptedOutcomeSimulationCase {
            case_id: String::from("refused.patch_apply.settlement_missing"),
            template: TassadarAcceptedOutcomeTemplate {
                template_id: String::from("accepted_outcome.patch_apply.settlement_gate.v1"),
                exact_compute_job_class: String::from("patch_apply_internal_exact"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                world_mount_receipt_id: world_mount_receipt.report_id.clone(),
                evidence_routing_receipt_id: evidence_routing_receipt.report_id.clone(),
                required_evidence_families: vec![
                    TassadarAcceptedOutcomeEvidenceFamily::ExecutionReceipt,
                    TassadarAcceptedOutcomeEvidenceFamily::CanonicalTraceBundle,
                    TassadarAcceptedOutcomeEvidenceFamily::BenchmarkLineage,
                    TassadarAcceptedOutcomeEvidenceFamily::ValidatorVerdict,
                ],
                validator_policy_ref: String::from(
                    "validator-policies.exact_compute.patch_apply.v1",
                ),
                validator_attachment_required: true,
                challenge_window_minutes: 0,
                settlement_dependency_refs: vec![
                    String::from("kernel-policy.accepted-outcome.patch_apply.v2"),
                    String::from("nexus.accepted-outcome.patch_apply.v2"),
                ],
                note: String::from(
                    "runtime and validator success stay insufficient when settlement-facing dependency refs are missing or stale",
                ),
            },
            runtime_success: true,
            candidate_outcome_id: String::from("candidate.patch_apply.settlement_missing.v1"),
            accepted_outcome_id: None,
            provider_receipt_id: String::from("provider_receipt.patch_apply.settlement_missing.v1"),
            validator_attachment_present: true,
            validator_policy_matches: true,
            challenge_window_open: false,
            settlement_dependencies_satisfied: false,
            status: TassadarAcceptedOutcomeSimulationStatus::Refused,
            refusal_reason: Some(TassadarAcceptedOutcomeRefusalReason::SettlementDependencyMissing),
            note: String::from(
                "the exact-compute job finished successfully, but settlement gating remains unresolved so accepted-outcome issuance must refuse",
            ),
        },
    ]
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
        TassadarAcceptedOutcomeRefusalReason, TassadarAcceptedOutcomeSimulationStatus,
        build_tassadar_accepted_outcome_binding_report,
        load_tassadar_accepted_outcome_binding_report,
        tassadar_accepted_outcome_binding_report_path,
    };

    #[test]
    fn accepted_outcome_binding_report_keeps_candidate_and_accepted_ids_distinct() {
        let report = build_tassadar_accepted_outcome_binding_report();

        assert_eq!(report.accepted_case_count, 1);
        assert_eq!(report.candidate_only_case_count, 1);
        assert_eq!(report.refused_case_count, 3);
        assert!(report.simulation_cases.iter().any(|case| {
            case.status == TassadarAcceptedOutcomeSimulationStatus::CandidateOnly
                && case.accepted_outcome_id.is_none()
                && case.refusal_reason
                    == Some(TassadarAcceptedOutcomeRefusalReason::ChallengeWindowOpen)
        }));
        assert!(report.simulation_cases.iter().any(|case| {
            case.status == TassadarAcceptedOutcomeSimulationStatus::Refused
                && case.refusal_reason
                    == Some(TassadarAcceptedOutcomeRefusalReason::ValidatorPolicyMismatch)
        }));
        assert!(report.simulation_cases.iter().any(|case| {
            case.status == TassadarAcceptedOutcomeSimulationStatus::Refused
                && case.refusal_reason
                    == Some(TassadarAcceptedOutcomeRefusalReason::SettlementDependencyMissing)
        }));
    }

    #[test]
    fn accepted_outcome_binding_report_matches_committed_truth() {
        let expected = build_tassadar_accepted_outcome_binding_report();
        let committed = load_tassadar_accepted_outcome_binding_report(
            tassadar_accepted_outcome_binding_report_path(),
        )
        .expect("committed accepted-outcome binding report");

        assert_eq!(committed, expected);
    }
}
