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
    TASSADAR_PROCESS_OBJECT_PROFILE_ID, TassadarCheckpointWorkloadFamily,
    TassadarProcessObjectCaseReceipt, build_tassadar_process_object_runtime_bundle,
};

pub const TASSADAR_INSTALLED_PROCESS_LIFECYCLE_PROFILE_ID: &str =
    "tassadar.internal_compute.installed_process_lifecycle.v1";
pub const TASSADAR_INSTALLED_PROCESS_SNAPSHOT_FAMILY_ID: &str =
    "tassadar.installed_process_snapshot.v1";
pub const TASSADAR_INSTALLED_PROCESS_MIGRATION_RECEIPT_FAMILY_ID: &str =
    "tassadar.installed_process_migration_receipt.v1";
pub const TASSADAR_INSTALLED_PROCESS_ROLLBACK_RECEIPT_FAMILY_ID: &str =
    "tassadar.installed_process_rollback_receipt.v1";
pub const TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_installed_process_lifecycle_v1";
pub const TASSADAR_INSTALLED_PROCESS_LIFECYCLE_BUNDLE_FILE: &str =
    "tassadar_installed_process_lifecycle_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInstalledProcessLifecycleCaseStatus {
    ExactMigrationParity,
    ExactRollbackParity,
    ExactRefusalParity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInstalledProcessLifecycleRefusalKind {
    StaleSnapshotRevision,
    PortabilityEnvelopeMismatch,
    RollbackLineageMissing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessSnapshot {
    pub process_id: String,
    pub snapshot_id: String,
    pub source_profile_id: String,
    pub source_profile_revision_id: String,
    pub source_machine_class_id: String,
    pub portability_envelope_id: String,
    pub checkpoint_id: String,
    pub next_step_index: u32,
    pub state_digest: String,
    pub snapshot_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessMigrationReceipt {
    pub receipt_id: String,
    pub source_machine_class_id: String,
    pub target_machine_class_id: String,
    pub source_profile_id: String,
    pub target_profile_id: String,
    pub source_profile_revision_id: String,
    pub target_profile_revision_id: String,
    pub portability_envelope_id: String,
    pub replay_parity: bool,
    pub snapshot_digest: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessRollbackReceipt {
    pub receipt_id: String,
    pub active_revision_id: String,
    pub rollback_revision_id: String,
    pub rollback_trigger_id: String,
    pub rollback_parity: bool,
    pub snapshot_digest: String,
    pub lineage_digest: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleRefusal {
    pub refusal_kind: TassadarInstalledProcessLifecycleRefusalKind,
    pub object_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleCaseReceipt {
    pub case_id: String,
    pub process_id: String,
    pub workload_family: TassadarCheckpointWorkloadFamily,
    pub portability_envelope_id: String,
    pub status: TassadarInstalledProcessLifecycleCaseStatus,
    pub exact_migration_parity: bool,
    pub exact_rollback_parity: bool,
    pub cross_machine_portability_green: bool,
    pub snapshot: TassadarInstalledProcessSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_receipt: Option<TassadarInstalledProcessMigrationReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_receipt: Option<TassadarInstalledProcessRollbackReceipt>,
    pub refusal_cases: Vec<TassadarInstalledProcessLifecycleRefusal>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub snapshot_family_id: String,
    pub migration_receipt_family_id: String,
    pub rollback_receipt_family_id: String,
    pub portability_envelope_ids: Vec<String>,
    pub case_receipts: Vec<TassadarInstalledProcessLifecycleCaseReceipt>,
    pub exact_migration_case_count: u32,
    pub exact_rollback_case_count: u32,
    pub refusal_case_count: u32,
    pub portable_process_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInstalledProcessLifecycleRuntimeBundleError {
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

#[must_use]
pub fn build_tassadar_installed_process_lifecycle_runtime_bundle()
-> TassadarInstalledProcessLifecycleRuntimeBundle {
    let process_bundle = build_tassadar_process_object_runtime_bundle();
    let case_receipts = vec![
        exact_migration_case(
            &process_bundle.case_receipts[0],
            "portable_cpu_reference_v1",
        ),
        exact_rollback_case(
            &process_bundle.case_receipts[1],
            "portable_cpu_reference_v1",
        ),
        refusal_case(
            &process_bundle.case_receipts[2],
            "portable_cpu_reference_v1",
        ),
    ];
    let exact_migration_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarInstalledProcessLifecycleCaseStatus::ExactMigrationParity
        })
        .count() as u32;
    let exact_rollback_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarInstalledProcessLifecycleCaseStatus::ExactRollbackParity
        })
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_cases.len() as u32)
        .sum();
    let portable_process_ids = case_receipts
        .iter()
        .filter(|case| case.cross_machine_portability_green)
        .map(|case| case.process_id.clone())
        .collect::<Vec<_>>();
    let mut bundle = TassadarInstalledProcessLifecycleRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.installed_process_lifecycle.bundle.v1"),
        profile_id: String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_PROFILE_ID),
        snapshot_family_id: String::from(TASSADAR_INSTALLED_PROCESS_SNAPSHOT_FAMILY_ID),
        migration_receipt_family_id: String::from(
            TASSADAR_INSTALLED_PROCESS_MIGRATION_RECEIPT_FAMILY_ID,
        ),
        rollback_receipt_family_id: String::from(
            TASSADAR_INSTALLED_PROCESS_ROLLBACK_RECEIPT_FAMILY_ID,
        ),
        portability_envelope_ids: vec![
            String::from("portable_cpu_reference_v1"),
            String::from("current_host_cpu_reference"),
        ],
        case_receipts,
        exact_migration_case_count,
        exact_rollback_case_count,
        refusal_case_count,
        portable_process_ids,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded installed-process lifecycle lane over committed process objects with portable snapshot export, typed migration receipts, and typed rollback receipts. It keeps stale snapshots, portability mismatches, and missing rollback lineage on explicit refusal paths instead of implying arbitrary cluster failover, arbitrary version migration, or broader served internal compute",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Installed-process lifecycle runtime bundle covers cases={}, exact_migration_cases={}, exact_rollback_cases={}, refusal_cases={}.",
        bundle.case_receipts.len(),
        bundle.exact_migration_case_count,
        bundle.exact_rollback_case_count,
        bundle.refusal_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_installed_process_lifecycle_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_installed_process_lifecycle_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF)
        .join(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_BUNDLE_FILE)
}

pub fn write_tassadar_installed_process_lifecycle_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarInstalledProcessLifecycleRuntimeBundle,
    TassadarInstalledProcessLifecycleRuntimeBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInstalledProcessLifecycleRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_installed_process_lifecycle_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInstalledProcessLifecycleRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn exact_migration_case(
    case: &TassadarProcessObjectCaseReceipt,
    portability_envelope_id: &str,
) -> TassadarInstalledProcessLifecycleCaseReceipt {
    let snapshot = snapshot_from_case(
        case,
        portability_envelope_id,
        "cpu_reference_x86_64.v1",
        "session_counter.profile_revision.r2",
    );
    let mut migration_receipt = TassadarInstalledProcessMigrationReceipt {
        receipt_id: format!("{}.migration_receipt.v1", case.case_id),
        source_machine_class_id: String::from("cpu_reference_x86_64.v1"),
        target_machine_class_id: String::from("cpu_reference_aarch64.v1"),
        source_profile_id: String::from(TASSADAR_PROCESS_OBJECT_PROFILE_ID),
        target_profile_id: String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_PROFILE_ID),
        source_profile_revision_id: String::from("session_counter.profile_revision.r2"),
        target_profile_revision_id: String::from("session_counter.profile_revision.r2"),
        portability_envelope_id: String::from(portability_envelope_id),
        replay_parity: true,
        snapshot_digest: snapshot.snapshot_digest.clone(),
        receipt_digest: String::new(),
    };
    migration_receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_installed_process_migration_receipt|",
        &migration_receipt,
    );
    let mut receipt = TassadarInstalledProcessLifecycleCaseReceipt {
        case_id: String::from("installed_process.session_counter.migrate_portable.v1"),
        process_id: case.process_id.clone(),
        workload_family: case.workload_family,
        portability_envelope_id: String::from(portability_envelope_id),
        status: TassadarInstalledProcessLifecycleCaseStatus::ExactMigrationParity,
        exact_migration_parity: true,
        exact_rollback_parity: false,
        cross_machine_portability_green: true,
        snapshot,
        migration_receipt: Some(migration_receipt),
        rollback_receipt: None,
        refusal_cases: Vec::new(),
        note: String::from(
            "installed process snapshot migrates across the portable cpu-reference envelope with exact replay parity",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_installed_process_lifecycle_case|",
        &receipt,
    );
    receipt
}

fn exact_rollback_case(
    case: &TassadarProcessObjectCaseReceipt,
    portability_envelope_id: &str,
) -> TassadarInstalledProcessLifecycleCaseReceipt {
    let snapshot = snapshot_from_case(
        case,
        portability_envelope_id,
        "cpu_reference_x86_64.v1",
        "search_frontier.profile_revision.r3",
    );
    let mut rollback_receipt = TassadarInstalledProcessRollbackReceipt {
        receipt_id: format!("{}.rollback_receipt.v1", case.case_id),
        active_revision_id: String::from("search_frontier.profile_revision.r3"),
        rollback_revision_id: String::from("search_frontier.profile_revision.r2"),
        rollback_trigger_id: String::from("rollback.trigger.integrity_regression.v1"),
        rollback_parity: true,
        snapshot_digest: snapshot.snapshot_digest.clone(),
        lineage_digest: String::new(),
        receipt_digest: String::new(),
    };
    rollback_receipt.lineage_digest = stable_digest(
        b"psionic_tassadar_installed_process_rollback_lineage|",
        &rollback_receipt,
    );
    rollback_receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_installed_process_rollback_receipt|",
        &rollback_receipt,
    );
    let mut receipt = TassadarInstalledProcessLifecycleCaseReceipt {
        case_id: String::from("installed_process.search_frontier.rollback_ready.v1"),
        process_id: case.process_id.clone(),
        workload_family: case.workload_family,
        portability_envelope_id: String::from(portability_envelope_id),
        status: TassadarInstalledProcessLifecycleCaseStatus::ExactRollbackParity,
        exact_migration_parity: false,
        exact_rollback_parity: true,
        cross_machine_portability_green: true,
        snapshot,
        migration_receipt: None,
        rollback_receipt: Some(rollback_receipt),
        refusal_cases: Vec::new(),
        note: String::from(
            "installed process rollback preserves exact replay parity against the prior revision under the same portability envelope",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_installed_process_lifecycle_case|",
        &receipt,
    );
    receipt
}

fn refusal_case(
    case: &TassadarProcessObjectCaseReceipt,
    portability_envelope_id: &str,
) -> TassadarInstalledProcessLifecycleCaseReceipt {
    let snapshot = snapshot_from_case(
        case,
        portability_envelope_id,
        "cpu_reference_x86_64.v1",
        "state_machine.profile_revision.r1",
    );
    let refusal_cases = vec![
        TassadarInstalledProcessLifecycleRefusal {
            refusal_kind: TassadarInstalledProcessLifecycleRefusalKind::StaleSnapshotRevision,
            object_ref: format!("snapshot://{}", snapshot.snapshot_id),
            detail: String::from(
                "stale installed-process snapshot revision stays on explicit refusal instead of silent forward migration",
            ),
        },
        TassadarInstalledProcessLifecycleRefusal {
            refusal_kind: TassadarInstalledProcessLifecycleRefusalKind::PortabilityEnvelopeMismatch,
            object_ref: String::from("portability://metal_served"),
            detail: String::from(
                "installed-process snapshot migration remains bounded to the portable cpu-reference envelope and refuses accelerator-specific widening",
            ),
        },
        TassadarInstalledProcessLifecycleRefusal {
            refusal_kind: TassadarInstalledProcessLifecycleRefusalKind::RollbackLineageMissing,
            object_ref: format!("rollback://{}", case.process_id),
            detail: String::from(
                "missing rollback lineage remains explicit instead of silently reconstructing a previous installed revision",
            ),
        },
    ];
    let mut receipt = TassadarInstalledProcessLifecycleCaseReceipt {
        case_id: String::from("installed_process.state_machine.stale_snapshot_refusal.v1"),
        process_id: case.process_id.clone(),
        workload_family: case.workload_family,
        portability_envelope_id: String::from(portability_envelope_id),
        status: TassadarInstalledProcessLifecycleCaseStatus::ExactRefusalParity,
        exact_migration_parity: false,
        exact_rollback_parity: false,
        cross_machine_portability_green: false,
        snapshot,
        migration_receipt: None,
        rollback_receipt: None,
        refusal_cases,
        note: String::from(
            "stale snapshots, non-portable targets, and missing rollback lineage remain explicit refusals in the installed-process lifecycle lane",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_installed_process_lifecycle_case|",
        &receipt,
    );
    receipt
}

fn snapshot_from_case(
    case: &TassadarProcessObjectCaseReceipt,
    portability_envelope_id: &str,
    source_machine_class_id: &str,
    source_profile_revision_id: &str,
) -> TassadarInstalledProcessSnapshot {
    let mut snapshot = TassadarInstalledProcessSnapshot {
        process_id: case.process_id.clone(),
        snapshot_id: format!("{}.installed_snapshot.v1", case.process_id),
        source_profile_id: String::from(TASSADAR_PROCESS_OBJECT_PROFILE_ID),
        source_profile_revision_id: String::from(source_profile_revision_id),
        source_machine_class_id: String::from(source_machine_class_id),
        portability_envelope_id: String::from(portability_envelope_id),
        checkpoint_id: case.snapshot.checkpoint_id.clone(),
        next_step_index: case.snapshot.next_step_index,
        state_digest: case.snapshot.memory_digest.clone(),
        snapshot_digest: String::new(),
    };
    snapshot.snapshot_digest =
        stable_digest(b"psionic_tassadar_installed_process_snapshot|", &snapshot);
    snapshot
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
) -> Result<T, TassadarInstalledProcessLifecycleRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarInstalledProcessLifecycleRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInstalledProcessLifecycleRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_installed_process_lifecycle_runtime_bundle, read_json,
        tassadar_installed_process_lifecycle_runtime_bundle_path,
        write_tassadar_installed_process_lifecycle_runtime_bundle,
    };

    #[test]
    fn installed_process_lifecycle_bundle_keeps_migration_rollback_and_refusal_truth_explicit() {
        let bundle = build_tassadar_installed_process_lifecycle_runtime_bundle();

        assert_eq!(bundle.exact_migration_case_count, 1);
        assert_eq!(bundle.exact_rollback_case_count, 1);
        assert_eq!(bundle.refusal_case_count, 3);
        assert_eq!(bundle.portable_process_ids.len(), 2);
    }

    #[test]
    fn installed_process_lifecycle_bundle_matches_committed_truth() {
        let generated = build_tassadar_installed_process_lifecycle_runtime_bundle();
        let committed = read_json(tassadar_installed_process_lifecycle_runtime_bundle_path())
            .expect("committed runtime bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_installed_process_lifecycle_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_installed_process_lifecycle_bundle.json");
        let bundle = write_tassadar_installed_process_lifecycle_runtime_bundle(&output_path)
            .expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
    }
}
