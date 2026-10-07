use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarInstalledModuleEvidenceStatus, build_tassadar_installed_module_evidence_bundle,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json";

/// Minimum evidence requirements for a promoted module to remain active.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModulePromotionEvidenceMinimums {
    pub minimum_compile_lineage_ref_count: u32,
    pub minimum_benchmark_ref_count: u32,
    pub minimum_audit_artifact_ref_count: u32,
    pub require_revocation_hook: bool,
    pub require_reinstall_parity_for_active: bool,
}

/// Current lifecycle state for one promoted module record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModulePromotionLifecycleState {
    ActivePromoted,
    ChallengeOpen,
    Quarantined,
    Revoked,
    Superseded,
}

/// One promotion lifecycle record derived from the installed-module evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModulePromotionLifecycleRecord {
    pub install_id: String,
    pub module_ref: String,
    pub lifecycle_state: TassadarModulePromotionLifecycleState,
    pub evidence_minimums_met: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preserved_lineage_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenge_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quarantine_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revocation_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_install_id: Option<String>,
    pub note: String,
}

/// Eval-owned report over promotion lifecycle and challengeability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModulePromotionStateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub evidence_minimums: TassadarModulePromotionEvidenceMinimums,
    pub active_promoted_count: u32,
    pub challenge_open_count: u32,
    pub quarantined_count: u32,
    pub revoked_count: u32,
    pub superseded_count: u32,
    pub records: Vec<TassadarModulePromotionLifecycleRecord>,
    pub bundle_ref: String,
    pub nexus_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Report persistence failure.
#[derive(Debug, Error)]
pub enum TassadarModulePromotionStateReportError {
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
    #[error(transparent)]
    Bundle(#[from] psionic_runtime::TassadarInstalledModuleEvidenceBundleError),
}

/// Builds the promotion lifecycle report over the bounded installed-module lane.
pub fn build_tassadar_module_promotion_state_report()
-> Result<TassadarModulePromotionStateReport, TassadarModulePromotionStateReportError> {
    let bundle = build_tassadar_installed_module_evidence_bundle()?;
    let evidence_minimums = TassadarModulePromotionEvidenceMinimums {
        minimum_compile_lineage_ref_count: 1,
        minimum_benchmark_ref_count: 2,
        minimum_audit_artifact_ref_count: 1,
        require_revocation_hook: true,
        require_reinstall_parity_for_active: true,
    };
    let records = bundle
        .records
        .iter()
        .map(|record| {
            let evidence_minimums_met = record.compile_lineage_refs.len() as u32
                >= evidence_minimums.minimum_compile_lineage_ref_count
                && record.benchmark_refs.len() as u32 >= evidence_minimums.minimum_benchmark_ref_count
                && record.audit_artifact_refs.len() as u32
                    >= evidence_minimums.minimum_audit_artifact_ref_count
                && (!evidence_minimums.require_revocation_hook || !record.revocation_hooks.is_empty())
                && (!evidence_minimums.require_reinstall_parity_for_active
                    || record.reinstall_parity_digest.is_some());
            if record.install_id == "reinstall.frontier_relax_core.session.v2" {
                return TassadarModulePromotionLifecycleRecord {
                    install_id: record.install_id.clone(),
                    module_ref: record.module_ref.clone(),
                    lifecycle_state: TassadarModulePromotionLifecycleState::ActivePromoted,
                    evidence_minimums_met,
                    preserved_lineage_refs: record.compile_lineage_refs.clone(),
                    challenge_refs: record
                        .revocation_hooks
                        .iter()
                        .filter(|hook| {
                            hook.hook_kind
                                == psionic_runtime::TassadarInstalledModuleRevocationHookKind::ChallengeTicket
                        })
                        .map(|hook| hook.hook_ref.clone())
                        .collect(),
                    quarantine_refs: Vec::new(),
                    revocation_refs: Vec::new(),
                    superseded_by_install_id: None,
                    note: String::from(
                        "frontier_relax_core remains actively promoted because the reinstall keeps evidence minimums, revocation hooks, and parity lineage intact",
                    ),
                };
            }
            if record.install_id == "install.frontier_relax_core.session.v1" {
                return TassadarModulePromotionLifecycleRecord {
                    install_id: record.install_id.clone(),
                    module_ref: record.module_ref.clone(),
                    lifecycle_state: TassadarModulePromotionLifecycleState::Superseded,
                    evidence_minimums_met,
                    preserved_lineage_refs: record.compile_lineage_refs.clone(),
                    challenge_refs: Vec::new(),
                    quarantine_refs: Vec::new(),
                    revocation_refs: Vec::new(),
                    superseded_by_install_id: Some(String::from(
                        "reinstall.frontier_relax_core.session.v2",
                    )),
                    note: String::from(
                        "the original frontier_relax_core install stays in history but is superseded by the later parity-preserving reinstall",
                    ),
                };
            }
            if record.status == TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence {
                return TassadarModulePromotionLifecycleRecord {
                    install_id: record.install_id.clone(),
                    module_ref: record.module_ref.clone(),
                    lifecycle_state: TassadarModulePromotionLifecycleState::Quarantined,
                    evidence_minimums_met: false,
                    preserved_lineage_refs: record.compile_lineage_refs.clone(),
                    challenge_refs: Vec::new(),
                    quarantine_refs: vec![String::from(
                        "policy://module_quarantine/branch_prune_core_missing_evidence",
                    )],
                    revocation_refs: Vec::new(),
                    superseded_by_install_id: None,
                    note: String::from(
                        "missing benchmark and audit evidence keeps branch_prune_core quarantined instead of silently promoted",
                    ),
                };
            }
            if record.status == TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence {
                return TassadarModulePromotionLifecycleRecord {
                    install_id: record.install_id.clone(),
                    module_ref: record.module_ref.clone(),
                    lifecycle_state: TassadarModulePromotionLifecycleState::Revoked,
                    evidence_minimums_met: false,
                    preserved_lineage_refs: record.compile_lineage_refs.clone(),
                    challenge_refs: Vec::new(),
                    quarantine_refs: vec![String::from(
                        "policy://module_quarantine/candidate_select_core_stale_evidence",
                    )],
                    revocation_refs: vec![String::from(
                        "receipt://candidate_select_core/revocation/stale_evidence",
                    )],
                    superseded_by_install_id: None,
                    note: String::from(
                        "stale evidence revokes the prior candidate_select_core promotion without erasing the original install lineage",
                    ),
                };
            }
            TassadarModulePromotionLifecycleRecord {
                install_id: record.install_id.clone(),
                module_ref: record.module_ref.clone(),
                lifecycle_state: TassadarModulePromotionLifecycleState::ChallengeOpen,
                evidence_minimums_met,
                preserved_lineage_refs: record.compile_lineage_refs.clone(),
                challenge_refs: record
                    .revocation_hooks
                    .iter()
                    .filter(|hook| {
                        hook.hook_kind
                            == psionic_runtime::TassadarInstalledModuleRevocationHookKind::ChallengeTicket
                    })
                    .map(|hook| hook.hook_ref.clone())
                    .collect(),
                quarantine_refs: vec![String::from(
                    "policy://module_quarantine/candidate_select_core/challenge_window",
                )],
                revocation_refs: Vec::new(),
                superseded_by_install_id: None,
                note: String::from(
                    "candidate_select_core remains challengeable after promotion rather than permanently closed after one benchmark pass",
                ),
            }
        })
        .collect::<Vec<_>>();
    let mut report = TassadarModulePromotionStateReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_promotion_state.report.v1"),
        evidence_minimums,
        active_promoted_count: records
            .iter()
            .filter(|record| {
                record.lifecycle_state == TassadarModulePromotionLifecycleState::ActivePromoted
            })
            .count() as u32,
        challenge_open_count: records
            .iter()
            .filter(|record| {
                record.lifecycle_state == TassadarModulePromotionLifecycleState::ChallengeOpen
            })
            .count() as u32,
        quarantined_count: records
            .iter()
            .filter(|record| {
                record.lifecycle_state == TassadarModulePromotionLifecycleState::Quarantined
            })
            .count() as u32,
        revoked_count: records
            .iter()
            .filter(|record| {
                record.lifecycle_state == TassadarModulePromotionLifecycleState::Revoked
            })
            .count() as u32,
        superseded_count: records
            .iter()
            .filter(|record| {
                record.lifecycle_state == TassadarModulePromotionLifecycleState::Superseded
            })
            .count() as u32,
        records,
        bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_installed_module_evidence_v1/installed_module_evidence_bundle.json",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of cross-provider challenge and accepted-outcome closure outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of settlement-grade promotion authority outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report keeps promotion evidence minimums, challengeability, quarantine, revocation, and supersession explicit for the bounded module lane. It does not claim nexus or kernel authority closure inside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-promotion state report now freezes {} active promotions, {} challenge-open records, {} quarantined records, {} revoked records, and {} superseded records.",
        report.active_promoted_count,
        report.challenge_open_count,
        report.quarantined_count,
        report.revoked_count,
        report.superseded_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_module_promotion_state_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_module_promotion_state_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_module_promotion_state_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModulePromotionStateReport, TassadarModulePromotionStateReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModulePromotionStateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_promotion_state_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModulePromotionStateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_module_promotion_state_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModulePromotionStateReport, TassadarModulePromotionStateReportError> {
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
) -> Result<T, TassadarModulePromotionStateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarModulePromotionStateReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModulePromotionStateReportError::Deserialize {
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
        TassadarModulePromotionLifecycleState, build_tassadar_module_promotion_state_report,
        load_tassadar_module_promotion_state_report, tassadar_module_promotion_state_report_path,
    };

    #[test]
    fn module_promotion_state_report_keeps_challenge_quarantine_and_revocation_explicit() {
        let report = build_tassadar_module_promotion_state_report().expect("report");

        assert_eq!(report.active_promoted_count, 1);
        assert_eq!(report.challenge_open_count, 1);
        assert_eq!(report.quarantined_count, 1);
        assert_eq!(report.revoked_count, 1);
        assert_eq!(report.superseded_count, 1);
        assert!(report.records.iter().any(|record| {
            record.lifecycle_state == TassadarModulePromotionLifecycleState::ChallengeOpen
                && record.module_ref == "candidate_select_core@1.1.0"
        }));
        assert!(report.records.iter().any(|record| {
            record.lifecycle_state == TassadarModulePromotionLifecycleState::Revoked
                && record.install_id == "install.candidate_select_core.refused_stale_evidence.v2"
        }));
    }

    #[test]
    fn module_promotion_state_report_matches_committed_truth() {
        let expected = build_tassadar_module_promotion_state_report().expect("report");
        let committed = load_tassadar_module_promotion_state_report(
            tassadar_module_promotion_state_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
