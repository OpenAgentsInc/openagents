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
    DatastreamSubjectKind, TassadarProcessSnapshotLocator, TassadarProcessTapeLocator,
    TassadarProcessWorkQueueLocator,
};
use psionic_runtime::{
    TASSADAR_PROCESS_OBJECT_BUNDLE_FILE, TASSADAR_PROCESS_OBJECT_PROFILE_ID,
    TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID,
    TASSADAR_PROCESS_TAPE_FAMILY_ID, TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID,
    TassadarCheckpointWorkloadFamily, TassadarProcessObjectCaseReceipt,
    TassadarProcessObjectRefusalKind, TassadarProcessObjectRuntimeBundle,
    build_tassadar_process_object_runtime_bundle,
};

/// Stable committed report ref for the durable process-object family.
pub const TASSADAR_PROCESS_OBJECT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_process_object_report.json";

/// One persisted durable process snapshot artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessSnapshotArtifactRef {
    /// Stable process identifier.
    pub process_id: String,
    /// Relative path for the serialized snapshot artifact.
    pub snapshot_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed process-snapshot locator derived from the manifest.
    pub locator: TassadarProcessSnapshotLocator,
}

/// One persisted durable process tape artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessTapeArtifactRef {
    /// Stable process identifier.
    pub process_id: String,
    /// Relative path for the serialized tape artifact.
    pub tape_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed process-tape locator derived from the manifest.
    pub locator: TassadarProcessTapeLocator,
}

/// One persisted durable process work-queue artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessWorkQueueArtifactRef {
    /// Stable process identifier.
    pub process_id: String,
    /// Relative path for the serialized work-queue artifact.
    pub work_queue_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed work-queue locator derived from the manifest.
    pub locator: TassadarProcessWorkQueueLocator,
}

/// Eval-facing case report for one durable process family row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable process identifier.
    pub process_id: String,
    /// Workload family covered by the process row.
    pub workload_family: TassadarCheckpointWorkloadFamily,
    /// Whether checkpointed and durable-process replay matched exactly.
    pub exact_process_parity: bool,
    /// Latest checkpoint identifier bound into the snapshot.
    pub checkpoint_id: String,
    /// Next step index carried by the durable snapshot.
    pub next_step_index: u32,
    /// Number of tape entries captured for the process lineage.
    pub tape_entry_count: u32,
    /// Number of pending work-queue entries.
    pub work_queue_depth: u32,
    /// Persisted snapshot artifact.
    pub snapshot_artifact: TassadarProcessSnapshotArtifactRef,
    /// Persisted tape artifact.
    pub tape_artifact: TassadarProcessTapeArtifactRef,
    /// Persisted work-queue artifact.
    pub work_queue_artifact: TassadarProcessWorkQueueArtifactRef,
    /// Typed refusal kinds exercised against the durable process objects.
    pub refusal_kinds: Vec<TassadarProcessObjectRefusalKind>,
    /// Plain-language case note.
    pub note: String,
}

/// Committed eval report for the durable process-object family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectReport {
    /// Schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run-bundle reference.
    pub runtime_bundle_ref: String,
    /// Runtime bundle carried through the eval artifact.
    pub runtime_bundle: TassadarProcessObjectRuntimeBundle,
    /// Case-level artifact and refusal summary.
    pub case_reports: Vec<TassadarProcessObjectCaseReport>,
    /// Number of exact durable-process parity rows.
    pub exact_process_parity_count: u32,
    /// Number of typed refusal rows.
    pub refusal_case_count: u32,
    /// Number of typed process locators carried by the report.
    pub process_locator_count: u32,
    /// Stable named profile identifier.
    pub profile_id: String,
    /// Stable process-snapshot family identifier.
    pub snapshot_family_id: String,
    /// Stable process-tape family identifier.
    pub tape_family_id: String,
    /// Stable process work-queue family identifier.
    pub work_queue_family_id: String,
    /// Stable refs used to derive the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarProcessObjectReportError {
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

pub fn build_tassadar_process_object_report()
-> Result<TassadarProcessObjectReport, TassadarProcessObjectReportError> {
    Ok(build_tassadar_process_object_materialization()?.0)
}

#[must_use]
pub fn tassadar_process_object_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PROCESS_OBJECT_REPORT_REF)
}

pub fn write_tassadar_process_object_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarProcessObjectReport, TassadarProcessObjectReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_process_object_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarProcessObjectReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| TassadarProcessObjectReportError::Write {
            path: path.display().to_string(),
            error,
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarProcessObjectReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarProcessObjectReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_process_object_materialization()
-> Result<(TassadarProcessObjectReport, Vec<WritePlan>), TassadarProcessObjectReportError> {
    let runtime_bundle = build_tassadar_process_object_runtime_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, TASSADAR_PROCESS_OBJECT_BUNDLE_FILE
    );
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut case_reports = Vec::new();
    for case_receipt in &runtime_bundle.case_receipts {
        let (case_report, case_write_plans, case_generated_from_refs) =
            build_case_materialization(case_receipt)?;
        write_plans.extend(case_write_plans);
        generated_from_refs.extend(case_generated_from_refs);
        case_reports.push(case_report);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let exact_process_parity_count = case_reports
        .iter()
        .filter(|case| case.exact_process_parity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let process_locator_count = (case_reports.len() * 3) as u32;
    let mut report = TassadarProcessObjectReport {
        schema_version: 1,
        report_id: String::from("tassadar.process_object.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        exact_process_parity_count,
        refusal_case_count,
        process_locator_count,
        profile_id: String::from(TASSADAR_PROCESS_OBJECT_PROFILE_ID),
        snapshot_family_id: String::from(TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID),
        tape_family_id: String::from(TASSADAR_PROCESS_TAPE_FAMILY_ID),
        work_queue_family_id: String::from(TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers the deterministic durable process-object family only for the committed checkpoint-backed workloads. It keeps snapshots, tapes, work queues, typed locators, and stale-or-malformed continuation-object refusals explicit instead of implying arbitrary process semantics, async effects, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Process-object report covers {} case rows with exact_process_parity_count={}, refusal_case_count={}, and process_locator_count={}.",
        report.case_reports.len(),
        report.exact_process_parity_count,
        report.refusal_case_count,
        report.process_locator_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_process_object_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarProcessObjectCaseReceipt,
) -> Result<
    (TassadarProcessObjectCaseReport, Vec<WritePlan>, Vec<String>),
    TassadarProcessObjectReportError,
> {
    let step = u64::from(case_receipt.snapshot.next_step_index);
    let snapshot_stem = format!("{}_snapshot", case_receipt.case_id);
    let snapshot_path = format!(
        "{}/{}_artifact.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, snapshot_stem
    );
    let snapshot_manifest_path = format!(
        "{}/{}_manifest.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, snapshot_stem
    );
    let snapshot_bytes = json_bytes(&case_receipt.snapshot)?;
    let snapshot_manifest = DatastreamManifest::from_bytes(
        format!("tassadar-process-snapshot://{}", case_receipt.process_id),
        DatastreamSubjectKind::Checkpoint,
        snapshot_bytes.as_slice(),
        96,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_process_snapshot(
        &case_receipt.process_id,
        step,
    ))
    .with_provenance_digest(case_receipt.snapshot.snapshot_digest.clone());
    let snapshot_manifest_bytes = json_bytes(&snapshot_manifest)?;
    let snapshot_manifest_ref = snapshot_manifest.manifest_ref();
    let snapshot_locator = snapshot_manifest_ref.tassadar_process_snapshot_locator()?;

    let tape_stem = format!("{}_tape", case_receipt.case_id);
    let tape_path = format!(
        "{}/{}_artifact.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, tape_stem
    );
    let tape_manifest_path = format!(
        "{}/{}_manifest.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, tape_stem
    );
    let tape_bytes = json_bytes(&case_receipt.tape)?;
    let tape_manifest = DatastreamManifest::from_bytes(
        format!("tassadar-process-tape://{}", case_receipt.process_id),
        DatastreamSubjectKind::Checkpoint,
        tape_bytes.as_slice(),
        96,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_process_tape(
        &case_receipt.process_id,
        step,
    ))
    .with_provenance_digest(case_receipt.tape.tape_digest.clone());
    let tape_manifest_bytes = json_bytes(&tape_manifest)?;
    let tape_manifest_ref = tape_manifest.manifest_ref();
    let tape_locator = tape_manifest_ref.tassadar_process_tape_locator()?;

    let work_queue_stem = format!("{}_work_queue", case_receipt.case_id);
    let work_queue_path = format!(
        "{}/{}_artifact.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, work_queue_stem
    );
    let work_queue_manifest_path = format!(
        "{}/{}_manifest.json",
        TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF, work_queue_stem
    );
    let work_queue_bytes = json_bytes(&case_receipt.work_queue)?;
    let work_queue_manifest = DatastreamManifest::from_bytes(
        format!("tassadar-process-work-queue://{}", case_receipt.process_id),
        DatastreamSubjectKind::Checkpoint,
        work_queue_bytes.as_slice(),
        96,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_process_work_queue(
        &case_receipt.process_id,
        step,
    ))
    .with_provenance_digest(case_receipt.work_queue.queue_digest.clone());
    let work_queue_manifest_bytes = json_bytes(&work_queue_manifest)?;
    let work_queue_manifest_ref = work_queue_manifest.manifest_ref();
    let work_queue_locator = work_queue_manifest_ref.tassadar_process_work_queue_locator()?;

    let refusal_kinds = case_receipt
        .refusal_cases
        .iter()
        .map(|refusal| refusal.refusal_kind)
        .collect::<Vec<_>>();
    Ok((
        TassadarProcessObjectCaseReport {
            case_id: case_receipt.case_id.clone(),
            process_id: case_receipt.process_id.clone(),
            workload_family: case_receipt.workload_family,
            exact_process_parity: case_receipt.exact_process_parity,
            checkpoint_id: case_receipt.snapshot.checkpoint_id.clone(),
            next_step_index: case_receipt.snapshot.next_step_index,
            tape_entry_count: case_receipt.tape.entry_count,
            work_queue_depth: case_receipt.work_queue.pending_entry_count,
            snapshot_artifact: TassadarProcessSnapshotArtifactRef {
                process_id: case_receipt.process_id.clone(),
                snapshot_path: snapshot_path.clone(),
                manifest_path: snapshot_manifest_path.clone(),
                manifest_ref: snapshot_manifest_ref,
                locator: snapshot_locator,
            },
            tape_artifact: TassadarProcessTapeArtifactRef {
                process_id: case_receipt.process_id.clone(),
                tape_path: tape_path.clone(),
                manifest_path: tape_manifest_path.clone(),
                manifest_ref: tape_manifest_ref,
                locator: tape_locator,
            },
            work_queue_artifact: TassadarProcessWorkQueueArtifactRef {
                process_id: case_receipt.process_id.clone(),
                work_queue_path: work_queue_path.clone(),
                manifest_path: work_queue_manifest_path.clone(),
                manifest_ref: work_queue_manifest_ref,
                locator: work_queue_locator,
            },
            refusal_kinds,
            note: case_receipt.note.clone(),
        },
        vec![
            WritePlan {
                relative_path: snapshot_path.clone(),
                bytes: snapshot_bytes,
            },
            WritePlan {
                relative_path: snapshot_manifest_path.clone(),
                bytes: snapshot_manifest_bytes,
            },
            WritePlan {
                relative_path: tape_path.clone(),
                bytes: tape_bytes,
            },
            WritePlan {
                relative_path: tape_manifest_path.clone(),
                bytes: tape_manifest_bytes,
            },
            WritePlan {
                relative_path: work_queue_path.clone(),
                bytes: work_queue_bytes,
            },
            WritePlan {
                relative_path: work_queue_manifest_path.clone(),
                bytes: work_queue_manifest_bytes,
            },
        ],
        vec![
            snapshot_path,
            snapshot_manifest_path,
            tape_path,
            tape_manifest_path,
            work_queue_path,
            work_queue_manifest_path,
        ],
    ))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    label: &str,
) -> Result<T, TassadarProcessObjectReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarProcessObjectReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarProcessObjectReportError::Decode {
        path: format!("{label}: {}", path.display()),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        TASSADAR_PROCESS_OBJECT_REPORT_REF, TassadarProcessObjectReport,
        build_tassadar_process_object_report, read_repo_json, repo_root,
        tassadar_process_object_report_path, write_tassadar_process_object_report,
    };
    use psionic_runtime::{
        TASSADAR_PROCESS_OBJECT_PROFILE_ID, TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID,
        TASSADAR_PROCESS_TAPE_FAMILY_ID, TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID,
        TassadarProcessObjectRefusalKind,
    };

    #[test]
    fn process_object_report_keeps_artifacts_and_refusals_explicit() {
        let report = build_tassadar_process_object_report().expect("report");

        assert_eq!(report.profile_id, TASSADAR_PROCESS_OBJECT_PROFILE_ID);
        assert_eq!(
            report.snapshot_family_id,
            TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID
        );
        assert_eq!(report.tape_family_id, TASSADAR_PROCESS_TAPE_FAMILY_ID);
        assert_eq!(
            report.work_queue_family_id,
            TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID
        );
        assert_eq!(report.exact_process_parity_count, 3);
        assert_eq!(report.refusal_case_count, 9);
        assert_eq!(report.process_locator_count, 9);
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.exact_process_parity)
        );
        assert!(report.case_reports.iter().all(|case| {
            case.refusal_kinds
                .contains(&TassadarProcessObjectRefusalKind::StaleProcessSnapshot)
        }));
        assert!(report.case_reports.iter().all(|case| {
            case.snapshot_artifact.locator.checkpoint_family == TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID
                && case.tape_artifact.locator.checkpoint_family == TASSADAR_PROCESS_TAPE_FAMILY_ID
                && case.work_queue_artifact.locator.checkpoint_family
                    == TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID
        }));
    }

    #[test]
    fn process_object_report_matches_committed_truth() {
        let generated = build_tassadar_process_object_report().expect("report");
        let committed: TassadarProcessObjectReport = read_repo_json(
            TASSADAR_PROCESS_OBJECT_REPORT_REF,
            "tassadar_process_object_report",
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_process_object_report_persists_current_truth() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let output_path = temp_dir.path().join("tassadar_process_object_report.json");
        let generated = write_tassadar_process_object_report(&output_path).expect("write report");
        let persisted: TassadarProcessObjectReport =
            serde_json::from_slice(&fs::read(&output_path).expect("read persisted report"))
                .expect("decode persisted report");

        assert_eq!(generated, persisted);
        assert_eq!(
            tassadar_process_object_report_path(),
            repo_root().join(TASSADAR_PROCESS_OBJECT_REPORT_REF)
        );
        assert_eq!(
            output_path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_process_object_report.json")
        );
    }
}
