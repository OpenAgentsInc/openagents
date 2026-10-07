use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind, TassadarInstalledProcessMigrationReceiptLocator,
    TassadarInstalledProcessRollbackReceiptLocator, TassadarInstalledProcessSnapshotLocator,
};
use psionic_runtime::{
    TASSADAR_INSTALLED_PROCESS_LIFECYCLE_BUNDLE_FILE,
    TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF,
    TassadarInstalledProcessLifecycleCaseReceipt, TassadarInstalledProcessLifecycleRefusalKind,
    TassadarInstalledProcessLifecycleRuntimeBundle,
    build_tassadar_installed_process_lifecycle_runtime_bundle,
};

pub const TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessSnapshotArtifactRef {
    pub process_id: String,
    pub snapshot_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarInstalledProcessSnapshotLocator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessMigrationReceiptArtifactRef {
    pub case_id: String,
    pub receipt_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarInstalledProcessMigrationReceiptLocator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessRollbackReceiptArtifactRef {
    pub case_id: String,
    pub receipt_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarInstalledProcessRollbackReceiptLocator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleCaseReport {
    pub case_id: String,
    pub process_id: String,
    pub portability_envelope_id: String,
    pub exact_migration_parity: bool,
    pub exact_rollback_parity: bool,
    pub cross_machine_portability_green: bool,
    pub snapshot_artifact: TassadarInstalledProcessSnapshotArtifactRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_receipt_artifact: Option<TassadarInstalledProcessMigrationReceiptArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_receipt_artifact: Option<TassadarInstalledProcessRollbackReceiptArtifactRef>,
    pub refusal_kind_ids: Vec<TassadarInstalledProcessLifecycleRefusalKind>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecycleReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarInstalledProcessLifecycleRuntimeBundle,
    pub case_reports: Vec<TassadarInstalledProcessLifecycleCaseReport>,
    pub exact_migration_case_count: u32,
    pub exact_rollback_case_count: u32,
    pub refusal_case_count: u32,
    pub portable_process_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInstalledProcessLifecycleReportError {
    #[error(transparent)]
    Datastream(#[from] psionic_datastream::DatastreamTransferError),
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

pub fn build_tassadar_installed_process_lifecycle_report()
-> Result<TassadarInstalledProcessLifecycleReport, TassadarInstalledProcessLifecycleReportError> {
    Ok(build_tassadar_installed_process_lifecycle_materialization()?.0)
}

#[must_use]
pub fn tassadar_installed_process_lifecycle_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF)
}

pub fn write_tassadar_installed_process_lifecycle_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInstalledProcessLifecycleReport, TassadarInstalledProcessLifecycleReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_installed_process_lifecycle_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarInstalledProcessLifecycleReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarInstalledProcessLifecycleReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInstalledProcessLifecycleReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInstalledProcessLifecycleReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_installed_process_lifecycle_materialization() -> Result<
    (TassadarInstalledProcessLifecycleReport, Vec<WritePlan>),
    TassadarInstalledProcessLifecycleReportError,
> {
    let runtime_bundle = build_tassadar_installed_process_lifecycle_runtime_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF,
        TASSADAR_INSTALLED_PROCESS_LIFECYCLE_BUNDLE_FILE
    );
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut case_reports = Vec::new();
    for case in &runtime_bundle.case_receipts {
        let (case_report, case_write_plans, case_generated_from_refs) =
            build_case_materialization(case)?;
        write_plans.extend(case_write_plans);
        generated_from_refs.extend(case_generated_from_refs);
        case_reports.push(case_report);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let exact_migration_case_count = case_reports
        .iter()
        .filter(|case| case.exact_migration_parity)
        .count() as u32;
    let exact_rollback_case_count = case_reports
        .iter()
        .filter(|case| case.exact_rollback_parity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|case| case.refusal_kind_ids.len() as u32)
        .sum();
    let portable_process_ids = case_reports
        .iter()
        .filter(|case| case.cross_machine_portability_green)
        .map(|case| case.process_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarInstalledProcessLifecycleReport {
        schema_version: 1,
        report_id: String::from("tassadar.installed_process_lifecycle.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        exact_migration_case_count,
        exact_rollback_case_count,
        refusal_case_count,
        portable_process_ids,
        portability_envelope_ids: vec![
            String::from("portable_cpu_reference_v1"),
            String::from("current_host_cpu_reference"),
        ],
        served_publication_allowed: false,
        overall_green: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded installed-process lifecycle lane with portable snapshot export, typed migration receipts, typed rollback receipts, and explicit stale-snapshot plus portability refusals. It remains operator-only here; served_publication_allowed stays false and the lane does not imply arbitrary cluster migration, arbitrary revision rollback, or broad served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.exact_migration_case_count == 1
        && report.exact_rollback_case_count == 1
        && report.refusal_case_count == 3
        && report.portable_process_ids.len() == 2
        && !report.served_publication_allowed;
    report.summary = format!(
        "Installed-process lifecycle report keeps migration_cases={}, rollback_cases={}, refusal_rows={}, portable_processes={}, served_publication_allowed={}, overall_green={}.",
        report.exact_migration_case_count,
        report.exact_rollback_case_count,
        report.refusal_case_count,
        report.portable_process_ids.len(),
        report.served_publication_allowed,
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_installed_process_lifecycle_report|",
        &report,
    );
    Ok((report, write_plans))
}

fn build_case_materialization(
    case: &TassadarInstalledProcessLifecycleCaseReceipt,
) -> Result<
    (
        TassadarInstalledProcessLifecycleCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarInstalledProcessLifecycleReportError,
> {
    let snapshot_path = format!(
        "{}/snapshots/{}.json",
        TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.process_id
    );
    let snapshot_manifest_path = format!(
        "{}/manifests/{}_snapshot_manifest.json",
        TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.process_id
    );
    let snapshot_bytes = json_bytes(&case.snapshot)?;
    let snapshot_manifest = DatastreamManifest::from_bytes(
        format!("{}-installed-snapshot", case.process_id),
        DatastreamSubjectKind::Checkpoint,
        &snapshot_bytes,
        96,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::tassadar_installed_process_snapshot(
            &case.process_id,
            u64::from(case.snapshot.next_step_index),
        ),
    );
    let snapshot_manifest_ref = snapshot_manifest.manifest_ref();
    let snapshot_locator = snapshot_manifest_ref.tassadar_installed_process_snapshot_locator()?;
    let mut write_plans = vec![
        WritePlan {
            relative_path: snapshot_path.clone(),
            bytes: snapshot_bytes,
        },
        WritePlan {
            relative_path: snapshot_manifest_path.clone(),
            bytes: json_bytes(&snapshot_manifest)?,
        },
    ];
    let mut generated_from_refs = vec![snapshot_path.clone(), snapshot_manifest_path.clone()];
    let snapshot_artifact = TassadarInstalledProcessSnapshotArtifactRef {
        process_id: case.process_id.clone(),
        snapshot_path,
        manifest_path: snapshot_manifest_path,
        manifest_ref: snapshot_manifest_ref,
        locator: snapshot_locator,
    };

    let migration_receipt_artifact = if let Some(receipt) = &case.migration_receipt {
        let receipt_path = format!(
            "{}/migration_receipts/{}.json",
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.case_id
        );
        let manifest_path = format!(
            "{}/manifests/{}_migration_manifest.json",
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.case_id
        );
        let receipt_bytes = json_bytes(receipt)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("{}-migration-receipt", case.case_id),
            DatastreamSubjectKind::Checkpoint,
            &receipt_bytes,
            96,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::tassadar_installed_process_migration_receipt(
                &case.process_id,
                u64::from(case.snapshot.next_step_index),
            ),
        );
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_installed_process_migration_receipt_locator()?;
        write_plans.push(WritePlan {
            relative_path: receipt_path.clone(),
            bytes: receipt_bytes,
        });
        write_plans.push(WritePlan {
            relative_path: manifest_path.clone(),
            bytes: json_bytes(&manifest)?,
        });
        generated_from_refs.push(receipt_path.clone());
        generated_from_refs.push(manifest_path.clone());
        Some(TassadarInstalledProcessMigrationReceiptArtifactRef {
            case_id: case.case_id.clone(),
            receipt_path,
            manifest_path,
            manifest_ref,
            locator,
        })
    } else {
        None
    };

    let rollback_receipt_artifact = if let Some(receipt) = &case.rollback_receipt {
        let receipt_path = format!(
            "{}/rollback_receipts/{}.json",
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.case_id
        );
        let manifest_path = format!(
            "{}/manifests/{}_rollback_manifest.json",
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_RUN_ROOT_REF, case.case_id
        );
        let receipt_bytes = json_bytes(receipt)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("{}-rollback-receipt", case.case_id),
            DatastreamSubjectKind::Checkpoint,
            &receipt_bytes,
            96,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::tassadar_installed_process_rollback_receipt(
                &case.process_id,
                u64::from(case.snapshot.next_step_index),
            ),
        );
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_installed_process_rollback_receipt_locator()?;
        write_plans.push(WritePlan {
            relative_path: receipt_path.clone(),
            bytes: receipt_bytes,
        });
        write_plans.push(WritePlan {
            relative_path: manifest_path.clone(),
            bytes: json_bytes(&manifest)?,
        });
        generated_from_refs.push(receipt_path.clone());
        generated_from_refs.push(manifest_path.clone());
        Some(TassadarInstalledProcessRollbackReceiptArtifactRef {
            case_id: case.case_id.clone(),
            receipt_path,
            manifest_path,
            manifest_ref,
            locator,
        })
    } else {
        None
    };

    Ok((
        TassadarInstalledProcessLifecycleCaseReport {
            case_id: case.case_id.clone(),
            process_id: case.process_id.clone(),
            portability_envelope_id: case.portability_envelope_id.clone(),
            exact_migration_parity: case.exact_migration_parity,
            exact_rollback_parity: case.exact_rollback_parity,
            cross_machine_portability_green: case.cross_machine_portability_green,
            snapshot_artifact,
            migration_receipt_artifact,
            rollback_receipt_artifact,
            refusal_kind_ids: case
                .refusal_cases
                .iter()
                .map(|refusal| refusal.refusal_kind)
                .collect(),
            note: case.note.clone(),
        },
        write_plans,
        generated_from_refs,
    ))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
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
) -> Result<T, TassadarInstalledProcessLifecycleReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarInstalledProcessLifecycleReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInstalledProcessLifecycleReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_installed_process_lifecycle_report, read_json,
        tassadar_installed_process_lifecycle_report_path,
        write_tassadar_installed_process_lifecycle_report,
    };

    #[test]
    fn installed_process_lifecycle_report_keeps_operator_only_publication_boundary_explicit() {
        let report = build_tassadar_installed_process_lifecycle_report().expect("report");

        assert_eq!(report.exact_migration_case_count, 1);
        assert_eq!(report.exact_rollback_case_count, 1);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.portable_process_ids.len(), 2);
        assert!(!report.served_publication_allowed);
        assert!(report.overall_green);
    }

    #[test]
    fn installed_process_lifecycle_report_matches_committed_truth() {
        let generated = build_tassadar_installed_process_lifecycle_report().expect("report");
        let committed = read_json(tassadar_installed_process_lifecycle_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_installed_process_lifecycle_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_installed_process_lifecycle_report.json");
        let report =
            write_tassadar_installed_process_lifecycle_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
