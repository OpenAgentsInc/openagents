use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{TassadarModuleTrustPosture, seeded_tassadar_computational_module_manifests};

use crate::{
    TassadarInstalledModuleEvidenceStatus, build_tassadar_installed_module_evidence_bundle,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trust_isolation_report.json";

/// Isolation boundary attached to one internal-module trust bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleIsolationBoundary {
    ResearchContained,
    BenchmarkInternalOnly,
    ChallengeGatedInstall,
}

/// Evidence posture published with one trust-tier bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleIsolationEvidencePosture {
    ResearchSeeded,
    ManifestOnly,
    InstalledComplete,
}

/// One trust-tiered capability bundle for an internal module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTrustCapabilityBundle {
    pub module_ref: String,
    pub trust_posture: TassadarModuleTrustPosture,
    pub isolation_boundary: TassadarModuleIsolationBoundary,
    pub evidence_posture: TassadarModuleIsolationEvidencePosture,
    pub benchmark_ref_count: u32,
    pub allowed_peer_trust_postures: Vec<TassadarModuleTrustPosture>,
    pub claim_boundary: String,
}

/// Task-scoped isolation policy evaluated for one composition request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleMountIsolationPolicy {
    pub mount_id: String,
    pub minimum_trust_posture: TassadarModuleTrustPosture,
    pub allow_research_modules: bool,
    pub allow_challenge_gated_installs: bool,
    pub note: String,
}

/// Composition request evaluated against trust-tier bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleIsolationRequest {
    pub request_id: String,
    pub primary_module_ref: String,
    pub imported_module_refs: Vec<String>,
    pub requested_authority_posture: TassadarModuleTrustPosture,
    pub mount_policy: TassadarModuleMountIsolationPolicy,
}

/// Typed refusal reason for trust-tier isolation evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleIsolationRefusalReason {
    UnknownModule,
    CrossTierCompositionDenied,
    PrivilegeEscalationDenied,
    MountPolicyDenied,
}

/// Successful isolation evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleIsolationSelection {
    pub request_id: String,
    pub primary_module_ref: String,
    pub permitted_module_refs: Vec<String>,
    pub note: String,
}

/// One evaluated isolation case in the committed report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleIsolationCaseReport {
    pub case_id: String,
    pub request: TassadarModuleIsolationRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarModuleIsolationSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarModuleIsolationRefusalReason>,
    pub note: String,
}

/// Committed runtime report over trust-tier isolation policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTrustIsolationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub bundles: Vec<TassadarModuleTrustCapabilityBundle>,
    pub allowed_case_count: u32,
    pub refused_case_count: u32,
    pub cross_tier_refusal_count: u32,
    pub privilege_escalation_refusal_count: u32,
    pub mount_policy_refusal_count: u32,
    pub case_reports: Vec<TassadarModuleIsolationCaseReport>,
    pub cluster_trust_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarModuleTrustIsolationReportError {
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

/// Builds the seeded trust-tiered capability bundles for internal modules.
pub fn seeded_tassadar_module_trust_capability_bundles() -> Vec<TassadarModuleTrustCapabilityBundle>
{
    let installed_bundle =
        build_tassadar_installed_module_evidence_bundle().expect("installed module evidence");
    let evidence_statuses = installed_bundle
        .records
        .into_iter()
        .filter(|record| record.status == TassadarInstalledModuleEvidenceStatus::Complete)
        .map(|record| (record.module_ref, record.status))
        .collect::<BTreeMap<_, _>>();
    let mut bundles = seeded_tassadar_computational_module_manifests()
        .into_iter()
        .map(|manifest| TassadarModuleTrustCapabilityBundle {
            module_ref: manifest.module_ref.clone(),
            trust_posture: manifest.trust_posture,
            isolation_boundary: match manifest.trust_posture {
                TassadarModuleTrustPosture::ResearchOnly => {
                    TassadarModuleIsolationBoundary::ResearchContained
                }
                TassadarModuleTrustPosture::BenchmarkGatedInternal => {
                    TassadarModuleIsolationBoundary::BenchmarkInternalOnly
                }
                TassadarModuleTrustPosture::ChallengeGatedInstall => {
                    TassadarModuleIsolationBoundary::ChallengeGatedInstall
                }
            },
            evidence_posture: if evidence_statuses.contains_key(&manifest.module_ref) {
                TassadarModuleIsolationEvidencePosture::InstalledComplete
            } else {
                TassadarModuleIsolationEvidencePosture::ManifestOnly
            },
            benchmark_ref_count: manifest.benchmark_lineage_refs.len() as u32,
            allowed_peer_trust_postures: match manifest.trust_posture {
                TassadarModuleTrustPosture::ResearchOnly => {
                    vec![TassadarModuleTrustPosture::ResearchOnly]
                }
                TassadarModuleTrustPosture::BenchmarkGatedInternal => vec![
                    TassadarModuleTrustPosture::BenchmarkGatedInternal,
                    TassadarModuleTrustPosture::ChallengeGatedInstall,
                ],
                TassadarModuleTrustPosture::ChallengeGatedInstall => vec![
                    TassadarModuleTrustPosture::BenchmarkGatedInternal,
                    TassadarModuleTrustPosture::ChallengeGatedInstall,
                ],
            },
            claim_boundary: format!(
                "module `{}` keeps trust posture {:?} explicit and does not inherit authority from benchmark count or peer composition alone",
                manifest.module_ref, manifest.trust_posture,
            ),
        })
        .collect::<Vec<_>>();
    bundles.push(TassadarModuleTrustCapabilityBundle {
        module_ref: String::from("scratchpad_probe_research@0.1.0"),
        trust_posture: TassadarModuleTrustPosture::ResearchOnly,
        isolation_boundary: TassadarModuleIsolationBoundary::ResearchContained,
        evidence_posture: TassadarModuleIsolationEvidencePosture::ResearchSeeded,
        benchmark_ref_count: 1,
        allowed_peer_trust_postures: vec![TassadarModuleTrustPosture::ResearchOnly],
        claim_boundary: String::from(
            "research-only scratchpad probes stay contained and never inherit benchmark-internal or install-grade authority",
        ),
    });
    bundles.sort_by(|left, right| left.module_ref.cmp(&right.module_ref));
    bundles
}

/// Evaluates one trust-tier isolation request.
pub fn evaluate_tassadar_module_isolation(
    request: &TassadarModuleIsolationRequest,
    bundles: &[TassadarModuleTrustCapabilityBundle],
) -> Result<TassadarModuleIsolationSelection, TassadarModuleIsolationRefusalReason> {
    let bundle_map = bundles
        .iter()
        .map(|bundle| (bundle.module_ref.as_str(), bundle))
        .collect::<BTreeMap<_, _>>();
    let primary = bundle_map
        .get(request.primary_module_ref.as_str())
        .ok_or(TassadarModuleIsolationRefusalReason::UnknownModule)?;
    if request.requested_authority_posture > primary.trust_posture {
        return Err(TassadarModuleIsolationRefusalReason::PrivilegeEscalationDenied);
    }
    if primary.trust_posture == TassadarModuleTrustPosture::ChallengeGatedInstall
        && !request.mount_policy.allow_challenge_gated_installs
    {
        return Err(TassadarModuleIsolationRefusalReason::MountPolicyDenied);
    }
    if primary.trust_posture == TassadarModuleTrustPosture::ResearchOnly
        && !request.mount_policy.allow_research_modules
    {
        return Err(TassadarModuleIsolationRefusalReason::MountPolicyDenied);
    }
    if primary.trust_posture < request.mount_policy.minimum_trust_posture {
        return Err(TassadarModuleIsolationRefusalReason::MountPolicyDenied);
    }
    let mut permitted_module_refs = vec![primary.module_ref.clone()];
    for module_ref in &request.imported_module_refs {
        let imported = bundle_map
            .get(module_ref.as_str())
            .ok_or(TassadarModuleIsolationRefusalReason::UnknownModule)?;
        if !primary
            .allowed_peer_trust_postures
            .contains(&imported.trust_posture)
            || !imported
                .allowed_peer_trust_postures
                .contains(&primary.trust_posture)
        {
            return Err(TassadarModuleIsolationRefusalReason::CrossTierCompositionDenied);
        }
        permitted_module_refs.push(imported.module_ref.clone());
    }
    permitted_module_refs.sort();
    permitted_module_refs.dedup();
    Ok(TassadarModuleIsolationSelection {
        request_id: request.request_id.clone(),
        primary_module_ref: request.primary_module_ref.clone(),
        permitted_module_refs,
        note: format!(
            "mount `{}` admitted the module composition without widening trust posture across tier boundaries",
            request.mount_policy.mount_id,
        ),
    })
}

/// Builds the committed trust-tier isolation report.
#[must_use]
pub fn build_tassadar_module_trust_isolation_report() -> TassadarModuleTrustIsolationReport {
    let bundles = seeded_tassadar_module_trust_capability_bundles();
    let benchmark_mount = TassadarModuleMountIsolationPolicy {
        mount_id: String::from("mount.benchmark_internal"),
        minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
        allow_research_modules: false,
        allow_challenge_gated_installs: false,
        note: String::from(
            "benchmark-internal mount allows benchmark-gated modules only and blocks research plus challenge-gated install posture",
        ),
    };
    let challenge_mount = TassadarModuleMountIsolationPolicy {
        mount_id: String::from("mount.challenge_install"),
        minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
        allow_research_modules: false,
        allow_challenge_gated_installs: true,
        note: String::from(
            "challenge-aware mount allows challenge-gated installs when the trust posture stays explicit",
        ),
    };
    let research_mount = TassadarModuleMountIsolationPolicy {
        mount_id: String::from("mount.research_lab"),
        minimum_trust_posture: TassadarModuleTrustPosture::ResearchOnly,
        allow_research_modules: true,
        allow_challenge_gated_installs: false,
        note: String::from(
            "research mount allows research-only modules but does not permit authority escalation",
        ),
    };
    let case_reports = vec![
        case_report(
            "trust_isolation.benchmark_pair_allowed.v1",
            TassadarModuleIsolationRequest {
                request_id: String::from("request.benchmark_pair_allowed"),
                primary_module_ref: String::from("frontier_relax_core@1.0.0"),
                imported_module_refs: vec![String::from("checkpoint_backtrack_core@1.0.0")],
                requested_authority_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                mount_policy: benchmark_mount.clone(),
            },
            &bundles,
            "benchmark-gated internal modules compose under the benchmark mount without widening authority",
        ),
        case_report(
            "trust_isolation.cross_tier_refused.v1",
            TassadarModuleIsolationRequest {
                request_id: String::from("request.cross_tier_refused"),
                primary_module_ref: String::from("frontier_relax_core@1.0.0"),
                imported_module_refs: vec![String::from("scratchpad_probe_research@0.1.0")],
                requested_authority_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                mount_policy: benchmark_mount.clone(),
            },
            &bundles,
            "research-only modules cannot silently compose into benchmark-gated internal paths",
        ),
        case_report(
            "trust_isolation.challenge_allowed.v1",
            TassadarModuleIsolationRequest {
                request_id: String::from("request.challenge_allowed"),
                primary_module_ref: String::from("candidate_select_core@1.1.0"),
                imported_module_refs: vec![String::from("frontier_relax_core@1.0.0")],
                requested_authority_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                mount_policy: challenge_mount.clone(),
            },
            &bundles,
            "challenge-gated candidate selection stays legal only on the explicit challenge-aware mount",
        ),
        case_report(
            "trust_isolation.privilege_escalation_refused.v1",
            TassadarModuleIsolationRequest {
                request_id: String::from("request.privilege_escalation_refused"),
                primary_module_ref: String::from("scratchpad_probe_research@0.1.0"),
                imported_module_refs: Vec::new(),
                requested_authority_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                mount_policy: research_mount.clone(),
            },
            &bundles,
            "research-only modules remain unable to request install-grade authority",
        ),
        case_report(
            "trust_isolation.mount_policy_refused.v1",
            TassadarModuleIsolationRequest {
                request_id: String::from("request.mount_policy_refused"),
                primary_module_ref: String::from("candidate_select_core@1.1.0"),
                imported_module_refs: vec![String::from("frontier_relax_core@1.0.0")],
                requested_authority_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                mount_policy: benchmark_mount,
            },
            &bundles,
            "challenge-gated install posture remains refused on the benchmark-only mount",
        ),
    ];
    let mut report = TassadarModuleTrustIsolationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_trust_isolation.report.v1"),
        bundles,
        allowed_case_count: case_reports
            .iter()
            .filter(|case| case.selection.is_some())
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.refusal_reason.is_some())
            .count() as u32,
        cross_tier_refusal_count: case_reports
            .iter()
            .filter(|case| {
                case.refusal_reason
                    == Some(TassadarModuleIsolationRefusalReason::CrossTierCompositionDenied)
            })
            .count() as u32,
        privilege_escalation_refusal_count: case_reports
            .iter()
            .filter(|case| {
                case.refusal_reason
                    == Some(TassadarModuleIsolationRefusalReason::PrivilegeEscalationDenied)
            })
            .count() as u32,
        mount_policy_refusal_count: case_reports
            .iter()
            .filter(|case| {
                case.refusal_reason == Some(TassadarModuleIsolationRefusalReason::MountPolicyDenied)
            })
            .count() as u32,
        case_reports,
        cluster_trust_dependency_marker: String::from(
            "cluster-trust remains the owner of provider and cluster authority posture outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of settlement-grade module authority outside standalone psionic",
        ),
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of task-scoped module authority posture outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this runtime report keeps research-only, benchmark-internal, and challenge-gated module isolation explicit. It does not claim cluster, kernel, or world-mount authority closure inside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module trust-isolation report now freezes {} allowed cases, {} refused cases, {} cross-tier refusals, {} privilege-escalation refusals, and {} mount-policy refusals.",
        report.allowed_case_count,
        report.refused_case_count,
        report.cross_tier_refusal_count,
        report.privilege_escalation_refusal_count,
        report.mount_policy_refusal_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_module_trust_isolation_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_module_trust_isolation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_module_trust_isolation_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleTrustIsolationReport, TassadarModuleTrustIsolationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleTrustIsolationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_trust_isolation_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleTrustIsolationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_module_trust_isolation_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModuleTrustIsolationReport, TassadarModuleTrustIsolationReportError> {
    read_json(path)
}

fn case_report(
    case_id: &str,
    request: TassadarModuleIsolationRequest,
    bundles: &[TassadarModuleTrustCapabilityBundle],
    note: &str,
) -> TassadarModuleIsolationCaseReport {
    match evaluate_tassadar_module_isolation(&request, bundles) {
        Ok(selection) => TassadarModuleIsolationCaseReport {
            case_id: String::from(case_id),
            request,
            selection: Some(selection),
            refusal_reason: None,
            note: String::from(note),
        },
        Err(refusal_reason) => TassadarModuleIsolationCaseReport {
            case_id: String::from(case_id),
            request,
            selection: None,
            refusal_reason: Some(refusal_reason),
            note: String::from(note),
        },
    }
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
) -> Result<T, TassadarModuleTrustIsolationReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarModuleTrustIsolationReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleTrustIsolationReportError::Deserialize {
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
        TassadarModuleIsolationRefusalReason, build_tassadar_module_trust_isolation_report,
        evaluate_tassadar_module_isolation, load_tassadar_module_trust_isolation_report,
        seeded_tassadar_module_trust_capability_bundles,
        tassadar_module_trust_isolation_report_path,
    };
    use psionic_ir::TassadarModuleTrustPosture;

    #[test]
    fn module_trust_bundles_publish_isolation_tiers() {
        let bundles = seeded_tassadar_module_trust_capability_bundles();

        assert!(bundles.iter().any(|bundle| {
            bundle.module_ref == "scratchpad_probe_research@0.1.0"
                && bundle.trust_posture == TassadarModuleTrustPosture::ResearchOnly
        }));
        assert!(bundles.iter().any(|bundle| {
            bundle.module_ref == "candidate_select_core@1.1.0"
                && bundle.trust_posture == TassadarModuleTrustPosture::ChallengeGatedInstall
        }));
    }

    #[test]
    fn module_trust_isolation_refuses_cross_tier_and_privilege_escalation() {
        let report = build_tassadar_module_trust_isolation_report();
        let cross_tier = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "trust_isolation.cross_tier_refused.v1")
            .expect("cross-tier case");
        assert_eq!(
            cross_tier.refusal_reason,
            Some(TassadarModuleIsolationRefusalReason::CrossTierCompositionDenied)
        );
        let privilege = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "trust_isolation.privilege_escalation_refused.v1")
            .expect("privilege case");
        assert_eq!(
            privilege.refusal_reason,
            Some(TassadarModuleIsolationRefusalReason::PrivilegeEscalationDenied)
        );
        let bundles = seeded_tassadar_module_trust_capability_bundles();
        let mount_case = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "trust_isolation.mount_policy_refused.v1")
            .expect("mount case");
        assert_eq!(
            evaluate_tassadar_module_isolation(&mount_case.request, &bundles),
            Err(TassadarModuleIsolationRefusalReason::MountPolicyDenied)
        );
    }

    #[test]
    fn module_trust_isolation_report_matches_committed_truth() {
        let expected = build_tassadar_module_trust_isolation_report();
        let committed = load_tassadar_module_trust_isolation_report(
            tassadar_module_trust_isolation_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
