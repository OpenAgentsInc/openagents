use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::seeded_tassadar_computational_module_manifests;

const BUNDLE_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_INSTALLED_MODULE_EVIDENCE_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_installed_module_evidence_v1/installed_module_evidence_bundle.json";

/// Evidence posture for one installed-module record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInstalledModuleEvidenceStatus {
    Complete,
    RefusedMissingEvidence,
    RefusedStaleEvidence,
}

/// Audit or decompilation artifact family attached to one installed module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInstalledModuleAuditArtifactKind {
    AuditArtifact,
    DecompilationReceipt,
}

/// Revocation hook published with one installed-module evidence record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInstalledModuleRevocationHookKind {
    ChallengeTicket,
    RevocationReceipt,
    QuarantinePolicy,
}

/// One audit or decompilation artifact reference attached to an installed module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleAuditArtifactRef {
    /// Typed artifact kind.
    pub artifact_kind: TassadarInstalledModuleAuditArtifactKind,
    /// Stable artifact reference.
    pub artifact_ref: String,
}

/// One revocation hook kept explicit in the installed-module bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleRevocationHook {
    /// Stable hook identifier.
    pub hook_id: String,
    /// Typed hook family.
    pub hook_kind: TassadarInstalledModuleRevocationHookKind,
    /// Stable hook reference.
    pub hook_ref: String,
}

/// One installed-module evidence record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleEvidenceRecord {
    /// Stable install identifier.
    pub install_id: String,
    /// Stable module ref.
    pub module_ref: String,
    /// Stable manifest digest for the installed module.
    pub manifest_digest: String,
    /// Stable compile-lineage refs.
    pub compile_lineage_refs: Vec<String>,
    /// Stable benchmark refs.
    pub benchmark_refs: Vec<String>,
    /// Stable audit or decompilation refs.
    pub audit_artifact_refs: Vec<TassadarInstalledModuleAuditArtifactRef>,
    /// Plain-language refusal or challenge posture.
    pub refusal_posture: String,
    /// Explicit revocation hooks for the module.
    pub revocation_hooks: Vec<TassadarInstalledModuleRevocationHook>,
    /// Stable reinstall-parity digest when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reinstall_parity_digest: Option<String>,
    /// Explicit stale-evidence detail when the record is refused for staleness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_evidence_detail: Option<String>,
    /// Final evidence status.
    pub status: TassadarInstalledModuleEvidenceStatus,
    /// Plain-language record note.
    pub note: String,
}

/// Installed-module evidence bundle for the bounded module-install lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleEvidenceBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Per-install evidence records.
    pub records: Vec<TassadarInstalledModuleEvidenceRecord>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarInstalledModuleEvidenceBundle {
    /// Validates the evidence bundle without relying on install-time side effects.
    pub fn validate(&self) -> Result<(), TassadarInstalledModuleEvidenceBundleValidationError> {
        for record in &self.records {
            match record.status {
                TassadarInstalledModuleEvidenceStatus::Complete => {
                    if record.compile_lineage_refs.is_empty() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingCompileLineage {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                    if record.benchmark_refs.is_empty() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingBenchmarkRefs {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                    if record.audit_artifact_refs.is_empty() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingAuditArtifacts {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                    if record.revocation_hooks.is_empty() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingRevocationHooks {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                    if record.reinstall_parity_digest.is_none() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingReinstallParityDigest {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                }
                TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence => {
                    if !record.compile_lineage_refs.is_empty()
                        && !record.benchmark_refs.is_empty()
                        && !record.audit_artifact_refs.is_empty()
                    {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingEvidenceRefusalMustActuallyBeMissingEvidence {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                }
                TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence => {
                    if record.stale_evidence_detail.is_none() {
                        return Err(
                            TassadarInstalledModuleEvidenceBundleValidationError::MissingStaleEvidenceDetail {
                                install_id: record.install_id.clone(),
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

/// Validation failure for one installed-module evidence bundle.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarInstalledModuleEvidenceBundleValidationError {
    #[error("installed-module evidence record `{install_id}` omitted compile lineage refs")]
    MissingCompileLineage { install_id: String },
    #[error("installed-module evidence record `{install_id}` omitted benchmark refs")]
    MissingBenchmarkRefs { install_id: String },
    #[error("installed-module evidence record `{install_id}` omitted audit artifacts")]
    MissingAuditArtifacts { install_id: String },
    #[error("installed-module evidence record `{install_id}` omitted revocation hooks")]
    MissingRevocationHooks { install_id: String },
    #[error("installed-module evidence record `{install_id}` omitted reinstall parity digest")]
    MissingReinstallParityDigest { install_id: String },
    #[error(
        "installed-module evidence record `{install_id}` was marked missing-evidence refused but still carried complete evidence"
    )]
    MissingEvidenceRefusalMustActuallyBeMissingEvidence { install_id: String },
    #[error("installed-module evidence record `{install_id}` omitted stale-evidence detail")]
    MissingStaleEvidenceDetail { install_id: String },
}

/// Bundle persistence failure.
#[derive(Debug, Error)]
pub enum TassadarInstalledModuleEvidenceBundleError {
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
    Validation(#[from] TassadarInstalledModuleEvidenceBundleValidationError),
}

/// Builds the seeded installed-module evidence bundle.
pub fn build_tassadar_installed_module_evidence_bundle()
-> Result<TassadarInstalledModuleEvidenceBundle, TassadarInstalledModuleEvidenceBundleError> {
    let manifest_digests = seeded_tassadar_computational_module_manifests()
        .into_iter()
        .map(|manifest| (manifest.module_ref, manifest.manifest_digest))
        .collect::<std::collections::BTreeMap<_, _>>();
    let records = vec![
        complete_record(
            "install.frontier_relax_core.session.v1",
            "frontier_relax_core@1.0.0",
            manifest_digests
                .get("frontier_relax_core@1.0.0")
                .expect("frontier manifest"),
            &[
                "manifest:tassadar.module.frontier_relax_core.manifest.v1",
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
            ],
            &[
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
            ],
            &[
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                ),
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
                ),
            ],
            &[
                (
                    "hook.frontier_relax.challenge",
                    TassadarInstalledModuleRevocationHookKind::ChallengeTicket,
                    "challenge://frontier_relax_core/session_mount",
                ),
                (
                    "hook.frontier_relax.quarantine",
                    TassadarInstalledModuleRevocationHookKind::QuarantinePolicy,
                    "policy://module_quarantine/frontier_relax_core",
                ),
            ],
            Some("parity.frontier_relax_core.1.0.0.v1"),
            None,
            TassadarInstalledModuleEvidenceStatus::Complete,
            "frontier_relax_core carries complete compile lineage, benchmark refs, audit refs, revocation hooks, and stable reinstall parity across repeated bounded installs",
        ),
        complete_record(
            "reinstall.frontier_relax_core.session.v2",
            "frontier_relax_core@1.0.0",
            manifest_digests
                .get("frontier_relax_core@1.0.0")
                .expect("frontier manifest"),
            &[
                "manifest:tassadar.module.frontier_relax_core.manifest.v1",
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
            ],
            &[
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
            ],
            &[
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                ),
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
                ),
            ],
            &[
                (
                    "hook.frontier_relax.reinstall_challenge",
                    TassadarInstalledModuleRevocationHookKind::ChallengeTicket,
                    "challenge://frontier_relax_core/reinstall",
                ),
                (
                    "hook.frontier_relax.reinstall_revocation",
                    TassadarInstalledModuleRevocationHookKind::RevocationReceipt,
                    "receipt://frontier_relax_core/revocation",
                ),
            ],
            Some("parity.frontier_relax_core.1.0.0.v1"),
            None,
            TassadarInstalledModuleEvidenceStatus::Complete,
            "frontier_relax_core reinstall preserves the same parity digest as the original bounded install, making reinstall parity explicit rather than assumed",
        ),
        complete_record(
            "install.candidate_select_core.rollback.v1",
            "candidate_select_core@1.1.0",
            manifest_digests
                .get("candidate_select_core@1.1.0")
                .expect("candidate manifest"),
            &[
                "manifest:tassadar.module.candidate_select_core.manifest.v1",
                "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json",
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
            ],
            &[
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json",
            ],
            &[
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json",
                ),
                (
                    TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                    "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                ),
            ],
            &[
                (
                    "hook.candidate_select.challenge",
                    TassadarInstalledModuleRevocationHookKind::ChallengeTicket,
                    "challenge://candidate_select_core/worker_mount",
                ),
                (
                    "hook.candidate_select.quarantine",
                    TassadarInstalledModuleRevocationHookKind::QuarantinePolicy,
                    "policy://module_quarantine/candidate_select_core",
                ),
            ],
            Some("parity.candidate_select_core.1.1.0.v1"),
            None,
            TassadarInstalledModuleEvidenceStatus::Complete,
            "candidate_select_core carries rollback-aware audit evidence and revocation hooks so the bounded worker-mount install stays challengeable instead of implicitly trusted",
        ),
        complete_record(
            "install.branch_prune_core.refused_missing_evidence.v1",
            "branch_prune_core@0.1.0",
            "manifest.missing.branch_prune_core",
            &[],
            &[],
            &[],
            &[(
                "hook.branch_prune.challenge",
                TassadarInstalledModuleRevocationHookKind::ChallengeTicket,
                "challenge://branch_prune_core/install",
            )],
            None,
            None,
            TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence,
            "branch_prune_core remains refused because compile lineage, benchmark refs, and audit artifacts are all missing",
        ),
        complete_record(
            "install.candidate_select_core.refused_stale_evidence.v2",
            "candidate_select_core@1.1.0",
            manifest_digests
                .get("candidate_select_core@1.1.0")
                .expect("candidate manifest"),
            &[
                "manifest:tassadar.module.candidate_select_core.manifest.v1",
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
            ],
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            &[(
                TassadarInstalledModuleAuditArtifactKind::AuditArtifact,
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
            )],
            &[(
                "hook.candidate_select.stale_revocation",
                TassadarInstalledModuleRevocationHookKind::RevocationReceipt,
                "receipt://candidate_select_core/stale_evidence_revocation",
            )],
            Some("parity.candidate_select_core.1.1.0.v1"),
            Some(
                "benchmark lineage predates the current worker-mount install policy epoch and must be re-attested before trust can widen",
            ),
            TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence,
            "candidate_select_core refuses stale-evidence promotion even though compile lineage and audit refs remain present",
        ),
    ];
    let mut bundle = TassadarInstalledModuleEvidenceBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_id: String::from("tassadar.installed_module_evidence.bundle.v1"),
        records,
        claim_boundary: String::from(
            "this bundle joins compile lineage, benchmark refs, audit or decompilation refs, refusal posture, revocation hooks, and reinstall parity for the bounded installed-module lane. It does not let installation imply trust when evidence is incomplete or stale, and it does not collapse execution receipts into economic authority",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Installed-module evidence bundle now freezes {} records with {} complete installs, {} missing-evidence refusals, and {} stale-evidence refusals.",
        bundle.records.len(),
        bundle
            .records
            .iter()
            .filter(|record| record.status == TassadarInstalledModuleEvidenceStatus::Complete)
            .count(),
        bundle
            .records
            .iter()
            .filter(|record| record.status
                == TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence)
            .count(),
        bundle
            .records
            .iter()
            .filter(|record| record.status
                == TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence)
            .count(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_installed_module_evidence_bundle|",
        &bundle,
    );
    bundle.validate()?;
    Ok(bundle)
}

/// Returns the canonical absolute path for the committed installed-module evidence bundle.
#[must_use]
pub fn tassadar_installed_module_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_INSTALLED_MODULE_EVIDENCE_BUNDLE_REF)
}

/// Writes the committed installed-module evidence bundle.
pub fn write_tassadar_installed_module_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInstalledModuleEvidenceBundle, TassadarInstalledModuleEvidenceBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInstalledModuleEvidenceBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_installed_module_evidence_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInstalledModuleEvidenceBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_installed_module_evidence_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarInstalledModuleEvidenceBundle, TassadarInstalledModuleEvidenceBundleError> {
    read_json(path)
}

fn complete_record(
    install_id: &str,
    module_ref: &str,
    manifest_digest: &str,
    compile_lineage_refs: &[&str],
    benchmark_refs: &[&str],
    audit_artifact_refs: &[(TassadarInstalledModuleAuditArtifactKind, &str)],
    revocation_hooks: &[(&str, TassadarInstalledModuleRevocationHookKind, &str)],
    reinstall_parity_digest: Option<&str>,
    stale_evidence_detail: Option<&str>,
    status: TassadarInstalledModuleEvidenceStatus,
    note: &str,
) -> TassadarInstalledModuleEvidenceRecord {
    TassadarInstalledModuleEvidenceRecord {
        install_id: String::from(install_id),
        module_ref: String::from(module_ref),
        manifest_digest: String::from(manifest_digest),
        compile_lineage_refs: compile_lineage_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        audit_artifact_refs: audit_artifact_refs
            .iter()
            .map(
                |(artifact_kind, artifact_ref)| TassadarInstalledModuleAuditArtifactRef {
                    artifact_kind: *artifact_kind,
                    artifact_ref: String::from(*artifact_ref),
                },
            )
            .collect(),
        refusal_posture: match status {
            TassadarInstalledModuleEvidenceStatus::Complete => {
                String::from("challengeable_complete")
            }
            TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence => {
                String::from("refused_missing_evidence")
            }
            TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence => {
                String::from("refused_stale_evidence")
            }
        },
        revocation_hooks: revocation_hooks
            .iter()
            .map(
                |(hook_id, hook_kind, hook_ref)| TassadarInstalledModuleRevocationHook {
                    hook_id: String::from(*hook_id),
                    hook_kind: *hook_kind,
                    hook_ref: String::from(*hook_ref),
                },
            )
            .collect(),
        reinstall_parity_digest: reinstall_parity_digest.map(String::from),
        stale_evidence_detail: stale_evidence_detail.map(String::from),
        status,
        note: String::from(note),
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
) -> Result<T, TassadarInstalledModuleEvidenceBundleError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarInstalledModuleEvidenceBundleError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInstalledModuleEvidenceBundleError::Deserialize {
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
        TassadarInstalledModuleEvidenceBundleValidationError,
        TassadarInstalledModuleEvidenceStatus, build_tassadar_installed_module_evidence_bundle,
        load_tassadar_installed_module_evidence_bundle,
        tassadar_installed_module_evidence_bundle_path,
    };

    #[test]
    fn installed_module_evidence_bundle_keeps_missing_stale_and_parity_truth_explicit() {
        let bundle = build_tassadar_installed_module_evidence_bundle().expect("bundle");

        assert_eq!(bundle.records.len(), 5);
        assert_eq!(
            bundle
                .records
                .iter()
                .filter(|record| record.status == TassadarInstalledModuleEvidenceStatus::Complete)
                .count(),
            3
        );
        assert!(bundle.records.iter().any(|record| {
            record.status == TassadarInstalledModuleEvidenceStatus::RefusedMissingEvidence
                && record.compile_lineage_refs.is_empty()
        }));
        assert!(bundle.records.iter().any(|record| {
            record.status == TassadarInstalledModuleEvidenceStatus::RefusedStaleEvidence
                && record.stale_evidence_detail.is_some()
        }));
        let parity_records = bundle
            .records
            .iter()
            .filter(|record| {
                record.module_ref == "frontier_relax_core@1.0.0"
                    && record.reinstall_parity_digest.as_deref()
                        == Some("parity.frontier_relax_core.1.0.0.v1")
            })
            .count();
        assert_eq!(parity_records, 2);
    }

    #[test]
    fn installed_module_evidence_bundle_refuses_complete_record_without_revocation_hook() {
        let mut bundle = build_tassadar_installed_module_evidence_bundle().expect("bundle");
        let install_id = bundle
            .records
            .iter_mut()
            .find(|record| record.status == TassadarInstalledModuleEvidenceStatus::Complete)
            .expect("complete record");
        let install_id = install_id.install_id.clone();
        bundle
            .records
            .iter_mut()
            .find(|record| record.install_id == install_id)
            .expect("same record")
            .revocation_hooks
            .clear();

        assert_eq!(
            bundle.validate(),
            Err(
                TassadarInstalledModuleEvidenceBundleValidationError::MissingRevocationHooks {
                    install_id
                }
            )
        );
    }

    #[test]
    fn installed_module_evidence_bundle_matches_committed_truth() {
        let expected = build_tassadar_installed_module_evidence_bundle().expect("bundle");
        let committed = load_tassadar_installed_module_evidence_bundle(
            tassadar_installed_module_evidence_bundle_path(),
        )
        .expect("committed bundle");

        assert_eq!(committed, expected);
    }
}
