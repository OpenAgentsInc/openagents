use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID: &str =
    "tassadar.effect_profile.virtual_fs_mounts.v1";
pub const TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_virtual_fs_mounts_v1";
pub const TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE: &str =
    "tassadar_virtual_fs_mount_runtime_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVirtualFsAccessMode {
    ReadOnlyArtifactMount,
    ArtifactReadPlusEphemeralWrite,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVirtualFsCaseStatus {
    ExactReplayParity,
    ExactRefusalParity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVirtualFsRefusalKind {
    AmbientHostPathDenied,
    UndeclaredMountPolicy,
    PathTraversalDenied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArtifactReadProof {
    pub proof_id: String,
    pub mount_id: String,
    pub virtual_path: String,
    pub artifact_rel_path: String,
    pub artifact_digest: String,
    pub read_offset_bytes: u32,
    pub read_length_bytes: u32,
    pub observed_chunk_digest: String,
    pub challenge_ref: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsCaseReceipt {
    pub case_id: String,
    pub process_id: String,
    pub profile_id: String,
    pub mount_id: String,
    pub virtual_path: String,
    pub access_mode: TassadarVirtualFsAccessMode,
    pub expected_mount_policy_id: String,
    pub status: TassadarVirtualFsCaseStatus,
    pub exact_replay_parity: bool,
    pub exact_artifact_proof_parity: bool,
    pub write_bytes: u32,
    pub artifact_read_proofs: Vec<TassadarArtifactReadProof>,
    pub refusal_kinds: Vec<TassadarVirtualFsRefusalKind>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVirtualFsMountRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub case_receipts: Vec<TassadarVirtualFsCaseReceipt>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub allowed_mount_ids: Vec<String>,
    pub refused_path_ids: Vec<String>,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarVirtualFsMountRuntimeBundleError {
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub const TASSADAR_VIRTUAL_FS_DICTIONARY_REL_PATH: &str =
    "mounted_artifacts/validation/dictionary.txt";
pub const TASSADAR_VIRTUAL_FS_CONFIG_REL_PATH: &str =
    "mounted_artifacts/config/task_config.json";

#[must_use]
pub fn tassadar_virtual_fs_dictionary_contents() -> &'static [u8] {
    b"alpha\nbeta\ngamma\n"
}

#[must_use]
pub fn tassadar_virtual_fs_config_contents() -> &'static [u8] {
    b"{\"counter\":2,\"window\":4}\n"
}

#[must_use]
pub fn build_tassadar_virtual_fs_mount_runtime_bundle() -> TassadarVirtualFsMountRuntimeBundle {
    let case_receipts = vec![
        dictionary_scan_case(),
        config_read_then_ephemeral_write_case(),
        ambient_host_home_refusal_case(),
        path_escape_refusal_case(),
    ];
    let exact_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarVirtualFsCaseStatus::ExactReplayParity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let allowed_mount_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarVirtualFsCaseStatus::ExactReplayParity)
        .map(|case| case.mount_id.clone())
        .collect::<Vec<_>>();
    let refused_path_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarVirtualFsCaseStatus::ExactRefusalParity)
        .map(|case| case.virtual_path.clone())
        .collect::<Vec<_>>();
    let mut bundle = TassadarVirtualFsMountRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.virtual_fs_mount.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID),
        case_receipts,
        exact_case_count,
        refusal_case_count,
        allowed_mount_ids,
        refused_path_ids,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical task-scoped mount policy outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this runtime bundle covers one bounded virtual-filesystem and artifact-mount profile with challengeable artifact-read proofs, deterministic ephemeral write bounds, and explicit refusal on ambient host paths or undeclared mounts. It does not claim arbitrary host filesystem access, broad effect closure, or broader served internal compute",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Virtual-fs runtime bundle covers exact_cases={}, refusal_rows={}, allowed_mounts={}, refused_paths={}.",
        bundle.exact_case_count,
        bundle.refusal_case_count,
        bundle.allowed_mount_ids.len(),
        bundle.refused_path_ids.len(),
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_virtual_fs_mount_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_virtual_fs_mount_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_VIRTUAL_FS_MOUNT_RUN_ROOT_REF)
        .join(TASSADAR_VIRTUAL_FS_MOUNT_BUNDLE_FILE)
}

pub fn write_tassadar_virtual_fs_mount_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarVirtualFsMountRuntimeBundle, TassadarVirtualFsMountRuntimeBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarVirtualFsMountRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_virtual_fs_mount_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarVirtualFsMountRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn dictionary_scan_case() -> TassadarVirtualFsCaseReceipt {
    let artifact_bytes = tassadar_virtual_fs_dictionary_contents();
    let proof_alpha = artifact_read_proof(
        "dictionary_scan_read_only_mount",
        "artifact_mount.validation_dictionary",
        "/virtual/artifacts/validation/dictionary.txt",
        TASSADAR_VIRTUAL_FS_DICTIONARY_REL_PATH,
        artifact_bytes,
        0,
        5,
        "challenge://tassadar/virtual_fs/dictionary_scan/00",
        "bounded dictionary scan proof for the `alpha` token remains challengeable and replay-safe",
    );
    let proof_beta = artifact_read_proof(
        "dictionary_scan_read_only_mount",
        "artifact_mount.validation_dictionary",
        "/virtual/artifacts/validation/dictionary.txt",
        TASSADAR_VIRTUAL_FS_DICTIONARY_REL_PATH,
        artifact_bytes,
        6,
        4,
        "challenge://tassadar/virtual_fs/dictionary_scan/01",
        "bounded dictionary scan proof for the `beta` token remains challengeable and replay-safe",
    );
    case_receipt(
        "dictionary_scan_read_only_mount",
        "tassadar.process.mounted_dictionary_lookup.v1",
        "artifact_mount.validation_dictionary",
        "/virtual/artifacts/validation/dictionary.txt",
        TassadarVirtualFsAccessMode::ReadOnlyArtifactMount,
        "mount_policy.read_only_artifacts.v1",
        TassadarVirtualFsCaseStatus::ExactReplayParity,
        true,
        true,
        0,
        vec![proof_alpha, proof_beta],
        Vec::new(),
        "read-only dictionary mount stays deterministic because artifact digests, virtual path, and read windows are all frozen explicitly",
    )
}

fn config_read_then_ephemeral_write_case() -> TassadarVirtualFsCaseReceipt {
    let artifact_bytes = tassadar_virtual_fs_config_contents();
    let proof = artifact_read_proof(
        "config_read_then_ephemeral_write",
        "artifact_mount.ephemeral_workspace",
        "/virtual/artifacts/config/task_config.json",
        TASSADAR_VIRTUAL_FS_CONFIG_REL_PATH,
        artifact_bytes,
        0,
        artifact_bytes.len() as u32,
        "challenge://tassadar/virtual_fs/config_read/00",
        "mounted config read stays challengeable before the deterministic ephemeral cache write executes",
    );
    case_receipt(
        "config_read_then_ephemeral_write",
        "tassadar.process.ephemeral_cache_build.v1",
        "artifact_mount.ephemeral_workspace",
        "/virtual/work/cache/result.bin",
        TassadarVirtualFsAccessMode::ArtifactReadPlusEphemeralWrite,
        "mount_policy.artifact_plus_ephemeral_workspace.v1",
        TassadarVirtualFsCaseStatus::ExactReplayParity,
        true,
        true,
        96,
        vec![proof],
        Vec::new(),
        "mounted config reads plus bounded ephemeral workspace writes stay deterministic because the mount set, write budget, and replay receipts are all frozen explicitly",
    )
}

fn ambient_host_home_refusal_case() -> TassadarVirtualFsCaseReceipt {
    case_receipt(
        "ambient_host_home_refusal",
        "tassadar.process.host_path_probe.v1",
        "ambient_host_home",
        "/host/home/.ssh/id_ed25519",
        TassadarVirtualFsAccessMode::Refused,
        "mount_policy.read_only_artifacts.v1",
        TassadarVirtualFsCaseStatus::ExactRefusalParity,
        false,
        false,
        0,
        Vec::new(),
        vec![TassadarVirtualFsRefusalKind::AmbientHostPathDenied],
        "ambient host-home paths remain refused because they are outside the declared virtual-filesystem mount set",
    )
}

fn path_escape_refusal_case() -> TassadarVirtualFsCaseReceipt {
    case_receipt(
        "path_escape_refusal",
        "tassadar.process.path_escape_probe.v1",
        "artifact_mount.validation_dictionary",
        "/virtual/artifacts/validation/../../private.txt",
        TassadarVirtualFsAccessMode::Refused,
        "mount_policy.read_only_artifacts.v1",
        TassadarVirtualFsCaseStatus::ExactRefusalParity,
        false,
        false,
        0,
        Vec::new(),
        vec![
            TassadarVirtualFsRefusalKind::PathTraversalDenied,
            TassadarVirtualFsRefusalKind::UndeclaredMountPolicy,
        ],
        "path traversal and undeclared mount widening remain explicit refusals instead of ambient filesystem access",
    )
}

#[allow(clippy::too_many_arguments)]
fn artifact_read_proof(
    case_id: &str,
    mount_id: &str,
    virtual_path: &str,
    artifact_rel_path: &str,
    artifact_bytes: &[u8],
    read_offset_bytes: u32,
    read_length_bytes: u32,
    challenge_ref: &str,
    note: &str,
) -> TassadarArtifactReadProof {
    let start = usize::try_from(read_offset_bytes).expect("offset");
    let end = usize::try_from(read_offset_bytes + read_length_bytes).expect("range");
    TassadarArtifactReadProof {
        proof_id: format!("{case_id}::proof::{read_offset_bytes:04}"),
        mount_id: String::from(mount_id),
        virtual_path: String::from(virtual_path),
        artifact_rel_path: String::from(artifact_rel_path),
        artifact_digest: digest_bytes(artifact_bytes),
        read_offset_bytes,
        read_length_bytes,
        observed_chunk_digest: digest_bytes(&artifact_bytes[start..end]),
        challenge_ref: String::from(challenge_ref),
        note: String::from(note),
    }
}

#[allow(clippy::too_many_arguments)]
fn case_receipt(
    case_id: &str,
    process_id: &str,
    mount_id: &str,
    virtual_path: &str,
    access_mode: TassadarVirtualFsAccessMode,
    expected_mount_policy_id: &str,
    status: TassadarVirtualFsCaseStatus,
    exact_replay_parity: bool,
    exact_artifact_proof_parity: bool,
    write_bytes: u32,
    artifact_read_proofs: Vec<TassadarArtifactReadProof>,
    refusal_kinds: Vec<TassadarVirtualFsRefusalKind>,
    note: &str,
) -> TassadarVirtualFsCaseReceipt {
    let mut receipt = TassadarVirtualFsCaseReceipt {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        profile_id: String::from(TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID),
        mount_id: String::from(mount_id),
        virtual_path: String::from(virtual_path),
        access_mode,
        expected_mount_policy_id: String::from(expected_mount_policy_id),
        status,
        exact_replay_parity,
        exact_artifact_proof_parity,
        write_bytes,
        artifact_read_proofs,
        refusal_kinds,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_virtual_fs_case_receipt|", &receipt);
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarVirtualFsMountRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarVirtualFsMountRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarVirtualFsMountRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID, build_tassadar_virtual_fs_mount_runtime_bundle,
        read_json, tassadar_virtual_fs_mount_runtime_bundle_path,
        write_tassadar_virtual_fs_mount_runtime_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn virtual_fs_runtime_bundle_keeps_mounts_and_refusals_explicit() {
        let bundle = build_tassadar_virtual_fs_mount_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_VIRTUAL_FS_MOUNT_PROFILE_ID);
        assert_eq!(bundle.exact_case_count, 2);
        assert_eq!(bundle.refusal_case_count, 3);
        assert_eq!(bundle.allowed_mount_ids.len(), 2);
        assert_eq!(bundle.refused_path_ids.len(), 2);
    }

    #[test]
    fn virtual_fs_runtime_bundle_keeps_artifact_proofs_challengeable() {
        let bundle = build_tassadar_virtual_fs_mount_runtime_bundle();
        let case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "dictionary_scan_read_only_mount")
            .expect("dictionary case");

        assert!(case.exact_replay_parity);
        assert!(case.exact_artifact_proof_parity);
        assert_eq!(case.artifact_read_proofs.len(), 2);
    }

    #[test]
    fn virtual_fs_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_virtual_fs_mount_runtime_bundle();
        let committed = read_json(tassadar_virtual_fs_mount_runtime_bundle_path())
            .expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_virtual_fs_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_virtual_fs_mount_runtime_bundle.json");
        let bundle =
            write_tassadar_virtual_fs_mount_runtime_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
        assert_eq!(
            tassadar_virtual_fs_mount_runtime_bundle_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_virtual_fs_mount_runtime_bundle.json")
        );
    }
}
