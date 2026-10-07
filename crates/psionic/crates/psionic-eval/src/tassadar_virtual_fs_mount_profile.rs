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
    TASSADAR_VIRTUAL_FS_CONFIG_REL_PATH, TASSADAR_VIRTUAL_FS_DICTIONARY_REL_PATH,
    TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE, TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID,
    TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, TassadarVirtualFsCaseReceipt,
    TassadarVirtualFsCaseStatus, TassadarVirtualFsMountRuntimeBundle,
    TassadarVirtualFsRefusalKind, build_tassadar_virtual_fs_mount_runtime_bundle,
    tassadar_virtual_fs_config_contents, tassadar_virtual_fs_dictionary_contents,
};
use psionic_sandbox::{
    TASSADAR_VIRTUAL_FS_MOUNT_SANDBOX_BOUNDARY_REPORT_REF,
    TassadarVirtualFsMountSandboxBoundaryReport,
    build_tassadar_virtual_fs_mount_sandbox_boundary_report,
};

pub const TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsArtifactRef {
    pub case_id: String,
    pub mount_id: String,
    pub virtual_path: String,
    pub artifact_rel_path: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsProofRef {
    pub case_id: String,
    pub proof_id: String,
    pub proof_rel_path: String,
    pub proof_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsCaseReport {
    pub case_id: String,
    pub process_id: String,
    pub mount_id: String,
    pub virtual_path: String,
    pub status: TassadarVirtualFsCaseStatus,
    pub exact_replay_parity: bool,
    pub exact_artifact_proof_parity: bool,
    pub write_bytes: u32,
    pub mounted_artifacts: Vec<TassadarVirtualFsArtifactRef>,
    pub proof_refs: Vec<TassadarVirtualFsProofRef>,
    pub refusal_kinds: Vec<TassadarVirtualFsRefusalKind>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsMountProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarVirtualFsMountRuntimeBundle,
    pub sandbox_boundary_report_ref: String,
    pub sandbox_boundary_report: TassadarVirtualFsMountSandboxBoundaryReport,
    pub case_reports: Vec<TassadarVirtualFsCaseReport>,
    pub allowed_mount_ids: Vec<String>,
    pub refused_path_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub served_publication_allowed: bool,
    pub world_mount_dependency_marker: String,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarVirtualFsMountProfileReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn build_tassadar_virtual_fs_mount_profile_report(
) -> Result<TassadarVirtualFsMountProfileReport, TassadarVirtualFsMountProfileReportError> {
    Ok(build_tassadar_virtual_fs_mount_materialization()?.0)
}

#[must_use]
pub fn tassadar_virtual_fs_mount_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF)
}

pub fn write_tassadar_virtual_fs_mount_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarVirtualFsMountProfileReport, TassadarVirtualFsMountProfileReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_virtual_fs_mount_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarVirtualFsMountProfileReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarVirtualFsMountProfileReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarVirtualFsMountProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarVirtualFsMountProfileReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_virtual_fs_mount_materialization(
) -> Result<
    (TassadarVirtualFsMountProfileReport, Vec<WritePlan>),
    TassadarVirtualFsMountProfileReportError,
> {
    let runtime_bundle = build_tassadar_virtual_fs_mount_runtime_bundle();
    let sandbox_boundary_report = build_tassadar_virtual_fs_mount_sandbox_boundary_report();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE
    );
    let dictionary_rel_path = TASSADAR_VIRTUAL_FS_DICTIONARY_REL_PATH.with_root();
    let config_rel_path = TASSADAR_VIRTUAL_FS_CONFIG_REL_PATH.with_root();
    let mut write_plans = vec![
        WritePlan {
            relative_path: runtime_bundle_ref.clone(),
            bytes: json_bytes(&runtime_bundle)?,
        },
        WritePlan {
            relative_path: dictionary_rel_path.clone(),
            bytes: tassadar_virtual_fs_dictionary_contents().to_vec(),
        },
        WritePlan {
            relative_path: config_rel_path.clone(),
            bytes: tassadar_virtual_fs_config_contents().to_vec(),
        },
    ];
    let mut generated_from_refs = vec![
        runtime_bundle_ref.clone(),
        dictionary_rel_path,
        config_rel_path,
        String::from(TASSADAR_VIRTUAL_FS_MOUNT_SANDBOX_BOUNDARY_REPORT_REF),
    ];
    let mut case_reports = Vec::new();
    for case in &runtime_bundle.case_receipts {
        let (case_report, plans, refs) = build_case_materialization(case)?;
        case_reports.push(case_report);
        write_plans.extend(plans);
        generated_from_refs.extend(refs);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let exact_case_count = case_reports
        .iter()
        .filter(|case| case.status == TassadarVirtualFsCaseStatus::ExactReplayParity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let mut report = TassadarVirtualFsMountProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.virtual_fs_mount.profile_report.v1"),
        profile_id: String::from(TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID),
        runtime_bundle_ref,
        runtime_bundle,
        sandbox_boundary_report_ref: String::from(
            TASSADAR_VIRTUAL_FS_MOUNT_SANDBOX_BOUNDARY_REPORT_REF,
        ),
        sandbox_boundary_report,
        case_reports,
        allowed_mount_ids: Vec::new(),
        refused_path_ids: Vec::new(),
        exact_case_count,
        refusal_case_count,
        served_publication_allowed: false,
        world_mount_dependency_marker: String::new(),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded virtual-filesystem and artifact-mount profile with challengeable artifact-read proofs and deterministic ephemeral write bounds. It keeps ambient host paths and undeclared mount widening on explicit refusal paths instead of implying arbitrary filesystem access or broader served compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.allowed_mount_ids = report.runtime_bundle.allowed_mount_ids.clone();
    report.refused_path_ids = report.runtime_bundle.refused_path_ids.clone();
    report.world_mount_dependency_marker = report.runtime_bundle.world_mount_dependency_marker.clone();
    report.summary = format!(
        "Virtual-fs profile report covers exact_cases={}, refusal_rows={}, allowed_mounts={}, refused_paths={}, served_publication_allowed={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.allowed_mount_ids.len(),
        report.refused_path_ids.len(),
        report.served_publication_allowed,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_virtual_fs_mount_profile_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case: &TassadarVirtualFsCaseReceipt,
) -> Result<
    (
        TassadarVirtualFsCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarVirtualFsMountProfileReportError,
> {
    let mut write_plans = Vec::new();
    let mut generated_from_refs = Vec::new();
    let mounted_artifacts = case
        .artifact_read_proofs
        .iter()
        .map(|proof| TassadarVirtualFsArtifactRef {
            case_id: case.case_id.clone(),
            mount_id: proof.mount_id.clone(),
            virtual_path: proof.virtual_path.clone(),
            artifact_rel_path: proof.artifact_rel_path.with_root(),
            artifact_digest: proof.artifact_digest.clone(),
        })
        .collect::<Vec<_>>();
    let mut proof_refs = Vec::new();
    for proof in &case.artifact_read_proofs {
        let proof_rel_path = format!(
            "{}/artifact_read_proofs/{}.json",
            TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, proof.proof_id
        );
        let bytes = json_bytes(proof)?;
        let proof_digest = digest_bytes(&bytes);
        write_plans.push(WritePlan {
            relative_path: proof_rel_path.clone(),
            bytes,
        });
        generated_from_refs.push(proof_rel_path.clone());
        proof_refs.push(TassadarVirtualFsProofRef {
            case_id: case.case_id.clone(),
            proof_id: proof.proof_id.clone(),
            proof_rel_path,
            proof_digest,
        });
    }
    Ok((
        TassadarVirtualFsCaseReport {
            case_id: case.case_id.clone(),
            process_id: case.process_id.clone(),
            mount_id: case.mount_id.clone(),
            virtual_path: case.virtual_path.clone(),
            status: case.status,
            exact_replay_parity: case.exact_replay_parity,
            exact_artifact_proof_parity: case.exact_artifact_proof_parity,
            write_bytes: case.write_bytes,
            mounted_artifacts,
            proof_refs,
            refusal_kinds: case.refusal_kinds.clone(),
            note: case.note.clone(),
        },
        write_plans,
        generated_from_refs,
    ))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, TassadarVirtualFsMountProfileReportError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarVirtualFsMountProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarVirtualFsMountProfileReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarVirtualFsMountProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

trait VirtualFsRelPathExt {
    fn with_root(&self) -> String;
}

impl VirtualFsRelPathExt for str {
    fn with_root(&self) -> String {
        format!("{}/{}", TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, self)
    }
}

impl VirtualFsRelPathExt for String {
    fn with_root(&self) -> String {
        self.as_str().with_root()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_virtual_fs_mount_profile_report, read_json,
        tassadar_virtual_fs_mount_profile_report_path,
        write_tassadar_virtual_fs_mount_profile_report,
    };
    use tempfile::tempdir;

    #[test]
    fn virtual_fs_profile_report_keeps_mount_and_refusal_posture_explicit() {
        let report = build_tassadar_virtual_fs_mount_profile_report().expect("report");

        assert!(report.world_mount_dependency_marker.contains("world-mounts"));
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.allowed_mount_ids.len(), 2);
        assert_eq!(report.refused_path_ids.len(), 2);
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn virtual_fs_profile_report_keeps_artifact_materialization_challengeable() {
        let report = build_tassadar_virtual_fs_mount_profile_report().expect("report");
        let case = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "dictionary_scan_read_only_mount")
            .expect("dictionary case");

        assert_eq!(case.mounted_artifacts.len(), 2);
        assert_eq!(case.proof_refs.len(), 2);
    }

    #[test]
    fn virtual_fs_profile_report_matches_committed_truth() {
        let generated = build_tassadar_virtual_fs_mount_profile_report().expect("report");
        let committed = read_json(tassadar_virtual_fs_mount_profile_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_virtual_fs_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_virtual_fs_mount_profile_report.json");
        let report =
            write_tassadar_virtual_fs_mount_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_virtual_fs_mount_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_virtual_fs_mount_profile_report.json")
        );
    }
}
