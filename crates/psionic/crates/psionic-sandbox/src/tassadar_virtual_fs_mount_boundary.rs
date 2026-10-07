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
    TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE, TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID,
    TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, TassadarVirtualFsAccessMode,
    TassadarVirtualFsCaseStatus, build_tassadar_virtual_fs_mount_runtime_bundle,
};

pub const TASSADAR_VIRTUAL_FS_MOUNT_SANDBOX_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_virtual_fs_mount_sandbox_boundary_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVirtualFsMountBoundaryStatus {
    AllowedDeterministic,
    RefusedOutOfEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsMountBoundaryRow {
    pub case_id: String,
    pub mount_id: String,
    pub virtual_path: String,
    pub status: TassadarVirtualFsMountBoundaryStatus,
    pub read_only_mount: bool,
    pub challengeable_artifact_proof_required: bool,
    pub max_write_bytes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsMountSandboxBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub rows: Vec<TassadarVirtualFsMountBoundaryRow>,
    pub allowed_mount_ids: Vec<String>,
    pub refused_path_ids: Vec<String>,
    pub allowed_case_count: u32,
    pub refused_case_count: u32,
    pub denied_path_refusal_count: u32,
    pub challengeable_artifact_read_case_count: u32,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarVirtualFsMountSandboxBoundaryError {
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

#[must_use]
pub fn build_tassadar_virtual_fs_mount_sandbox_boundary_report(
) -> TassadarVirtualFsMountSandboxBoundaryReport {
    let runtime_bundle = build_tassadar_virtual_fs_mount_runtime_bundle();
    let rows = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| TassadarVirtualFsMountBoundaryRow {
            case_id: case.case_id.clone(),
            mount_id: case.mount_id.clone(),
            virtual_path: case.virtual_path.clone(),
            status: if case.status == TassadarVirtualFsCaseStatus::ExactReplayParity {
                TassadarVirtualFsMountBoundaryStatus::AllowedDeterministic
            } else {
                TassadarVirtualFsMountBoundaryStatus::RefusedOutOfEnvelope
            },
            read_only_mount: case.access_mode == TassadarVirtualFsAccessMode::ReadOnlyArtifactMount,
            challengeable_artifact_proof_required: !case.artifact_read_proofs.is_empty(),
            max_write_bytes: case.write_bytes,
            refusal_reason_id: case.refusal_kinds.first().map(|kind| format!("{kind:?}").to_lowercase()),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let allowed_mount_ids = rows
        .iter()
        .filter(|row| row.status == TassadarVirtualFsMountBoundaryStatus::AllowedDeterministic)
        .map(|row| row.mount_id.clone())
        .collect::<Vec<_>>();
    let refused_path_ids = rows
        .iter()
        .filter(|row| row.status == TassadarVirtualFsMountBoundaryStatus::RefusedOutOfEnvelope)
        .map(|row| row.virtual_path.clone())
        .collect::<Vec<_>>();
    let allowed_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarVirtualFsMountBoundaryStatus::AllowedDeterministic)
        .count() as u32;
    let refused_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarVirtualFsMountBoundaryStatus::RefusedOutOfEnvelope)
        .count() as u32;
    let denied_path_refusal_count = rows
        .iter()
        .filter(|row| row.status == TassadarVirtualFsMountBoundaryStatus::RefusedOutOfEnvelope)
        .count() as u32;
    let challengeable_artifact_read_case_count = rows
        .iter()
        .filter(|row| row.challengeable_artifact_proof_required)
        .count() as u32;
    let mut report = TassadarVirtualFsMountSandboxBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.virtual_fs_mount.sandbox_boundary.report.v1"),
        profile_id: String::from(TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID),
        runtime_bundle_ref: format!(
            "{}/{}",
            TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF, TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE
        ),
        rows,
        allowed_mount_ids,
        refused_path_ids,
        allowed_case_count,
        refused_case_count,
        denied_path_refusal_count,
        challengeable_artifact_read_case_count,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the authority owner for canonical task-scoped mount policy outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the authority owner for settlement-grade effect admission outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this sandbox boundary admits only one deterministic virtual-filesystem mount profile with read-only artifact mounts and bounded ephemeral workspace writes. It keeps ambient host paths and undeclared mount widening on explicit refusal paths instead of implying arbitrary filesystem access",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Virtual-fs sandbox boundary freezes allowed_cases={}, refused_cases={}, denied_path_refusals={}, challengeable_artifact_reads={}.",
        report.allowed_case_count,
        report.refused_case_count,
        report.denied_path_refusal_count,
        report.challengeable_artifact_read_case_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_virtual_fs_mount_sandbox_boundary_report|", &report);
    report
}

#[must_use]
pub fn tassadar_virtual_fs_mount_sandbox_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_VIRTUAL_FS_MOUNT_SANDBOX_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_virtual_fs_mount_sandbox_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarVirtualFsMountSandboxBoundaryReport,
    TassadarVirtualFsMountSandboxBoundaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarVirtualFsMountSandboxBoundaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_virtual_fs_mount_sandbox_boundary_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarVirtualFsMountSandboxBoundaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarVirtualFsMountSandboxBoundaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarVirtualFsMountSandboxBoundaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarVirtualFsMountSandboxBoundaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarVirtualFsMountBoundaryStatus,
        build_tassadar_virtual_fs_mount_sandbox_boundary_report, read_json,
        tassadar_virtual_fs_mount_sandbox_boundary_report_path,
        write_tassadar_virtual_fs_mount_sandbox_boundary_report,
    };
    use tempfile::tempdir;

    #[test]
    fn virtual_fs_sandbox_boundary_keeps_mount_policy_truth_explicit() {
        let report = build_tassadar_virtual_fs_mount_sandbox_boundary_report();

        assert_eq!(report.allowed_case_count, 2);
        assert_eq!(report.refused_case_count, 2);
        assert_eq!(report.denied_path_refusal_count, 2);
        assert_eq!(report.challengeable_artifact_read_case_count, 2);
    }

    #[test]
    fn virtual_fs_sandbox_boundary_keeps_allowed_and_refused_rows_explicit() {
        let report = build_tassadar_virtual_fs_mount_sandbox_boundary_report();
        let allowed = report
            .rows
            .iter()
            .find(|row| row.case_id == "dictionary_scan_read_only_mount")
            .expect("allowed row");
        assert_eq!(
            allowed.status,
            TassadarVirtualFsMountBoundaryStatus::AllowedDeterministic
        );
        let refused = report
            .rows
            .iter()
            .find(|row| row.case_id == "ambient_host_home_refusal")
            .expect("refused row");
        assert_eq!(
            refused.status,
            TassadarVirtualFsMountBoundaryStatus::RefusedOutOfEnvelope
        );
    }

    #[test]
    fn virtual_fs_sandbox_boundary_matches_committed_truth() {
        let generated = build_tassadar_virtual_fs_mount_sandbox_boundary_report();
        let committed = read_json(tassadar_virtual_fs_mount_sandbox_boundary_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_virtual_fs_sandbox_boundary_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_virtual_fs_mount_sandbox_boundary_report.json");
        let report = write_tassadar_virtual_fs_mount_sandbox_boundary_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_virtual_fs_mount_sandbox_boundary_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_virtual_fs_mount_sandbox_boundary_report.json")
        );
    }
}
