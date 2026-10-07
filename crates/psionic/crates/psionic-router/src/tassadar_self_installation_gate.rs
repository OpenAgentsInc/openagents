use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarModuleInstallRouteCandidate, TassadarModuleInstallRouteDescriptor,
    TassadarModuleInstallRouteRefusalReason, TassadarModuleInstallRoutingRequest,
    TassadarModuleInstallScope, negotiate_tassadar_module_install_route,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_SELF_INSTALLATION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_self_installation_gate_report.json";

/// Final bounded self-install outcome recorded by the router report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSelfInstallationOutcome {
    SessionMounted,
    ChallengeWindow,
    RolledBackAfterBenchmark,
    Refused,
}

/// Typed refusal reason for one self-install proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSelfInstallationRefusalReason {
    MountPolicyDenied,
    ProviderNotReady,
    ScopeUnsupported,
    UnsafeModuleClass,
    ChallengeTicketMissing,
    BenchmarkEvidenceMissing,
}

impl From<TassadarModuleInstallRouteRefusalReason> for TassadarSelfInstallationRefusalReason {
    fn from(value: TassadarModuleInstallRouteRefusalReason) -> Self {
        match value {
            TassadarModuleInstallRouteRefusalReason::ProviderNotReady => Self::ProviderNotReady,
            TassadarModuleInstallRouteRefusalReason::ScopeUnsupported => Self::ScopeUnsupported,
            TassadarModuleInstallRouteRefusalReason::UnsafeModuleClass => Self::UnsafeModuleClass,
            TassadarModuleInstallRouteRefusalReason::ChallengeTicketMissing => {
                Self::ChallengeTicketMissing
            }
            TassadarModuleInstallRouteRefusalReason::BenchmarkEvidenceMissing => {
                Self::BenchmarkEvidenceMissing
            }
        }
    }
}

/// One self-install proposal evaluated by the router gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSelfInstallationProposal {
    pub proposal_id: String,
    pub mount_id: String,
    pub module_id: String,
    pub module_class: String,
    pub requested_version: String,
    pub scope: TassadarModuleInstallScope,
    pub self_extension_allowed: bool,
    pub challenge_ticket_acknowledged: bool,
    pub benchmark_evidence_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_install_benchmark_passed: Option<bool>,
}

/// One evaluated self-install proposal case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSelfInstallationCaseReport {
    pub proposal: TassadarSelfInstallationProposal,
    pub outcome: TassadarSelfInstallationOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    pub challenge_ticket_present: bool,
    pub rollback_receipt_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarSelfInstallationRefusalReason>,
    pub note: String,
}

/// Router-owned report over bounded self-install proposals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSelfInstallationGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub session_mounted_count: u32,
    pub challenge_window_count: u32,
    pub rolled_back_count: u32,
    pub refused_count: u32,
    pub case_reports: Vec<TassadarSelfInstallationCaseReport>,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSelfInstallationGateReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Evaluates one bounded self-install proposal.
pub fn evaluate_tassadar_self_installation_proposal(
    proposal: &TassadarSelfInstallationProposal,
    candidates: &[TassadarModuleInstallRouteCandidate],
) -> TassadarSelfInstallationCaseReport {
    if !proposal.self_extension_allowed {
        return TassadarSelfInstallationCaseReport {
            proposal: proposal.clone(),
            outcome: TassadarSelfInstallationOutcome::Refused,
            provider_id: None,
            worker_id: None,
            challenge_ticket_present: proposal.challenge_ticket_acknowledged,
            rollback_receipt_present: false,
            refusal_reason: Some(TassadarSelfInstallationRefusalReason::MountPolicyDenied),
            note: String::from(
                "self-extension stays blocked when the current mount does not explicitly allow bounded self-install proposals",
            ),
        };
    }
    let routing_request = TassadarModuleInstallRoutingRequest {
        install_id: proposal.proposal_id.clone(),
        module_id: proposal.module_id.clone(),
        module_class: proposal.module_class.clone(),
        scope: proposal.scope,
        challenge_ticket_acknowledged: proposal.challenge_ticket_acknowledged,
        benchmark_evidence_present: proposal.benchmark_evidence_present,
    };
    match negotiate_tassadar_module_install_route(&routing_request, candidates) {
        Err(error) => TassadarSelfInstallationCaseReport {
            proposal: proposal.clone(),
            outcome: TassadarSelfInstallationOutcome::Refused,
            provider_id: None,
            worker_id: None,
            challenge_ticket_present: proposal.challenge_ticket_acknowledged,
            rollback_receipt_present: false,
            refusal_reason: Some(error.into()),
            note: String::from(
                "self-install proposal stayed refused by the bounded module-install route policy",
            ),
        },
        Ok(selection) => {
            if proposal.scope == TassadarModuleInstallScope::WorkerMount
                && proposal.post_install_benchmark_passed == Some(false)
            {
                return TassadarSelfInstallationCaseReport {
                    proposal: proposal.clone(),
                    outcome: TassadarSelfInstallationOutcome::RolledBackAfterBenchmark,
                    provider_id: Some(selection.provider_id),
                    worker_id: Some(selection.worker_id),
                    challenge_ticket_present: proposal.challenge_ticket_acknowledged,
                    rollback_receipt_present: true,
                    refusal_reason: None,
                    note: String::from(
                        "failed post-install benchmark handling stays explicit through rollback instead of being treated as silent cleanup",
                    ),
                };
            }
            let outcome = if proposal.scope == TassadarModuleInstallScope::WorkerMount {
                TassadarSelfInstallationOutcome::ChallengeWindow
            } else {
                TassadarSelfInstallationOutcome::SessionMounted
            };
            TassadarSelfInstallationCaseReport {
                proposal: proposal.clone(),
                outcome,
                provider_id: Some(selection.provider_id),
                worker_id: Some(selection.worker_id),
                challenge_ticket_present: proposal.challenge_ticket_acknowledged,
                rollback_receipt_present: false,
                refusal_reason: None,
                note: String::from(
                    "bounded self-install proposals stay inspectable through route selection, mount scope, and challenge posture instead of hiding inside model-internal mutation",
                ),
            }
        }
    }
}

/// Builds the committed self-installation gate report.
#[must_use]
pub fn build_tassadar_self_installation_gate_report() -> TassadarSelfInstallationGateReport {
    let candidates = seeded_tassadar_self_install_route_candidates();
    let case_reports = vec![
        evaluate_tassadar_self_installation_proposal(
            &TassadarSelfInstallationProposal {
                proposal_id: String::from("self_install.frontier_relax.session.v1"),
                mount_id: String::from("mount.session"),
                module_id: String::from("frontier_relax_core"),
                module_class: String::from("frontier_relax_core"),
                requested_version: String::from("1.0.0"),
                scope: TassadarModuleInstallScope::SessionMount,
                self_extension_allowed: true,
                challenge_ticket_acknowledged: false,
                benchmark_evidence_present: true,
                post_install_benchmark_passed: Some(true),
            },
            &candidates,
        ),
        evaluate_tassadar_self_installation_proposal(
            &TassadarSelfInstallationProposal {
                proposal_id: String::from("self_install.candidate_select.challenge.v1"),
                mount_id: String::from("mount.worker.challenge"),
                module_id: String::from("candidate_select_core"),
                module_class: String::from("candidate_select_core"),
                requested_version: String::from("1.2.0"),
                scope: TassadarModuleInstallScope::WorkerMount,
                self_extension_allowed: true,
                challenge_ticket_acknowledged: true,
                benchmark_evidence_present: true,
                post_install_benchmark_passed: Some(true),
            },
            &candidates,
        ),
        evaluate_tassadar_self_installation_proposal(
            &TassadarSelfInstallationProposal {
                proposal_id: String::from("self_install.candidate_select.rollback.v1"),
                mount_id: String::from("mount.worker.challenge"),
                module_id: String::from("candidate_select_core"),
                module_class: String::from("candidate_select_core"),
                requested_version: String::from("1.2.0"),
                scope: TassadarModuleInstallScope::WorkerMount,
                self_extension_allowed: true,
                challenge_ticket_acknowledged: true,
                benchmark_evidence_present: true,
                post_install_benchmark_passed: Some(false),
            },
            &candidates,
        ),
        evaluate_tassadar_self_installation_proposal(
            &TassadarSelfInstallationProposal {
                proposal_id: String::from("self_install.branch_prune.denied.v1"),
                mount_id: String::from("mount.session"),
                module_id: String::from("branch_prune_core"),
                module_class: String::from("branch_prune_core"),
                requested_version: String::from("0.1.0"),
                scope: TassadarModuleInstallScope::SessionMount,
                self_extension_allowed: false,
                challenge_ticket_acknowledged: false,
                benchmark_evidence_present: false,
                post_install_benchmark_passed: None,
            },
            &candidates,
        ),
    ];
    let mut report = TassadarSelfInstallationGateReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.self_installation_gate.report.v1"),
        session_mounted_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarSelfInstallationOutcome::SessionMounted)
            .count() as u32,
        challenge_window_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarSelfInstallationOutcome::ChallengeWindow)
            .count() as u32,
        rolled_back_count: case_reports
            .iter()
            .filter(|case| {
                case.outcome == TassadarSelfInstallationOutcome::RolledBackAfterBenchmark
            })
            .count() as u32,
        refused_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarSelfInstallationOutcome::Refused)
            .count() as u32,
        case_reports,
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of settlement-grade self-install authority outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of cross-provider challenge resolution outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this router report keeps bounded self-install proposals, policy denial, challenge windows, and rollback under failed post-install benchmarks explicit. It does not claim unrestricted self-modification",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Self-installation gate report now freezes {} session mounts, {} challenge-window proposals, {} rolled-back proposals, and {} refused proposals.",
        report.session_mounted_count,
        report.challenge_window_count,
        report.rolled_back_count,
        report.refused_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_self_installation_gate_report|", &report);
    report
}

fn seeded_tassadar_self_install_route_candidates() -> Vec<TassadarModuleInstallRouteCandidate> {
    vec![TassadarModuleInstallRouteCandidate::new(
        "provider-a",
        "worker-a",
        true,
        TassadarModuleInstallRouteDescriptor::new(
            "self-install-route-a",
            vec![
                TassadarModuleInstallScope::SessionMount,
                TassadarModuleInstallScope::WorkerMount,
            ],
            vec![
                String::from("frontier_relax_core"),
                String::from("candidate_select_core"),
                String::from("checkpoint_backtrack_core"),
            ],
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json",
            )],
            "bounded self-install route candidate",
        ),
    )]
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_self_installation_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SELF_INSTALLATION_GATE_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_self_installation_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSelfInstallationGateReport, TassadarSelfInstallationGateReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSelfInstallationGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_self_installation_gate_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSelfInstallationGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_self_installation_gate_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSelfInstallationGateReport, TassadarSelfInstallationGateReportError> {
    read_json(path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarSelfInstallationGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSelfInstallationGateReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSelfInstallationGateReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        TassadarSelfInstallationOutcome, TassadarSelfInstallationRefusalReason,
        build_tassadar_self_installation_gate_report, load_tassadar_self_installation_gate_report,
        tassadar_self_installation_gate_report_path,
    };

    #[test]
    fn self_installation_gate_report_keeps_policy_and_rollback_explicit() {
        let report = build_tassadar_self_installation_gate_report();

        assert_eq!(report.session_mounted_count, 1);
        assert_eq!(report.challenge_window_count, 1);
        assert_eq!(report.rolled_back_count, 1);
        assert_eq!(report.refused_count, 1);
        assert!(report.case_reports.iter().any(|case| {
            case.outcome == TassadarSelfInstallationOutcome::RolledBackAfterBenchmark
                && case.rollback_receipt_present
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.refusal_reason == Some(TassadarSelfInstallationRefusalReason::MountPolicyDenied)
        }));
    }

    #[test]
    fn self_installation_gate_report_matches_committed_truth() {
        let expected = build_tassadar_self_installation_gate_report();
        let committed = load_tassadar_self_installation_gate_report(
            tassadar_self_installation_gate_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
