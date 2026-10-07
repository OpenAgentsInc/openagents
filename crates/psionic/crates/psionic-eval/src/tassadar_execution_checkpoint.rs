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
    DatastreamSubjectKind, TassadarExecutionCheckpointLocator,
};
use psionic_runtime::{
    TASSADAR_EXECUTION_CHECKPOINT_BUNDLE_FILE, TASSADAR_EXECUTION_CHECKPOINT_FAMILY_ID,
    TASSADAR_EXECUTION_CHECKPOINT_RUN_ROOT_REF, TassadarCheckpointWorkloadFamily,
    TassadarExecutionCheckpoint, TassadarExecutionCheckpointCaseReceipt,
    TassadarExecutionCheckpointRuntimeBundle, TassadarResumeRefusalKind,
    build_tassadar_execution_checkpoint_runtime_bundle,
};

/// Stable committed report ref for the checkpointed multi-slice execution lane.
pub const TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json";

/// One persisted continuation artifact emitted for a checkpointed run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointArtifactRef {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Relative path for the serialized checkpoint artifact.
    pub checkpoint_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed execution-checkpoint locator derived from the manifest.
    pub locator: TassadarExecutionCheckpointLocator,
}

/// Eval-facing case report for one checkpointed workload family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family.
    pub workload_family: TassadarCheckpointWorkloadFamily,
    /// Number of slices needed for completion.
    pub slice_count: u32,
    /// Number of emitted checkpoints.
    pub checkpoint_count: u32,
    /// Whether fresh and resumed runs matched exactly.
    pub exact_resume_parity: bool,
    /// Final deterministic result.
    pub final_result: i64,
    /// Latest checkpoint identifier for the workload.
    pub latest_checkpoint_id: String,
    /// Materialized checkpoint artifacts for this workload.
    pub checkpoint_artifacts: Vec<TassadarExecutionCheckpointArtifactRef>,
    /// Typed refusal kinds exercised against the checkpoint chain.
    pub refusal_kinds: Vec<TassadarResumeRefusalKind>,
    /// Plain-language case note.
    pub note: String,
}

/// Committed eval report for the checkpointed execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointReport {
    /// Schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run-bundle reference.
    pub runtime_bundle_ref: String,
    /// Runtime bundle carried through the eval artifact.
    pub runtime_bundle: TassadarExecutionCheckpointRuntimeBundle,
    /// Case-level artifact and refusal summary.
    pub case_reports: Vec<TassadarExecutionCheckpointCaseReport>,
    /// Number of exact fresh-vs-resumed parity rows.
    pub exact_resume_parity_count: u32,
    /// Number of typed refusal rows.
    pub refusal_case_count: u32,
    /// Number of latest-checkpoint locators carried by the report.
    pub latest_checkpoint_locator_count: u32,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
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
pub enum TassadarExecutionCheckpointReportError {
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

pub fn build_tassadar_execution_checkpoint_report()
-> Result<TassadarExecutionCheckpointReport, TassadarExecutionCheckpointReportError> {
    Ok(build_tassadar_execution_checkpoint_materialization()?.0)
}

#[must_use]
pub fn tassadar_execution_checkpoint_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF)
}

pub fn write_tassadar_execution_checkpoint_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExecutionCheckpointReport, TassadarExecutionCheckpointReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_execution_checkpoint_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarExecutionCheckpointReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarExecutionCheckpointReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarExecutionCheckpointReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarExecutionCheckpointReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_execution_checkpoint_materialization() -> Result<
    (TassadarExecutionCheckpointReport, Vec<WritePlan>),
    TassadarExecutionCheckpointReportError,
> {
    let runtime_bundle = build_tassadar_execution_checkpoint_runtime_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_EXECUTION_CHECKPOINT_RUN_ROOT_REF, TASSADAR_EXECUTION_CHECKPOINT_BUNDLE_FILE
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
    let exact_resume_parity_count = case_reports
        .iter()
        .filter(|report| report.exact_resume_parity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|report| report.refusal_kinds.len() as u32)
        .sum();
    let latest_checkpoint_locator_count = case_reports.len() as u32;
    let mut report = TassadarExecutionCheckpointReport {
        schema_version: 1,
        report_id: String::from("tassadar.execution_checkpoint.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        exact_resume_parity_count,
        refusal_case_count,
        latest_checkpoint_locator_count,
        checkpoint_family_id: String::from(TASSADAR_EXECUTION_CHECKPOINT_FAMILY_ID),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers the deterministic checkpointed multi-slice lane only for the committed seeded long-running workloads. It keeps parity, dirty-page receipts, continuation artifacts, and typed refusal posture explicit instead of implying arbitrary long-running Wasm or served-profile closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Execution-checkpoint report covers {} case rows with exact_resume_parity_count={}, refusal_case_count={}, and latest_checkpoint_locator_count={}.",
        report.case_reports.len(),
        report.exact_resume_parity_count,
        report.refusal_case_count,
        report.latest_checkpoint_locator_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_execution_checkpoint_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarExecutionCheckpointCaseReceipt,
) -> Result<
    (
        TassadarExecutionCheckpointCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarExecutionCheckpointReportError,
> {
    let mut checkpoint_artifacts = Vec::new();
    let mut write_plans = Vec::new();
    let mut generated_from_refs = Vec::new();
    for checkpoint in &case_receipt.checkpoint_history {
        let checkpoint_stem = checkpoint_artifact_stem(checkpoint);
        let checkpoint_path = format!(
            "{}/{}_checkpoint.json",
            TASSADAR_EXECUTION_CHECKPOINT_RUN_ROOT_REF, checkpoint_stem
        );
        let manifest_path = format!(
            "{}/{}_checkpoint_manifest.json",
            TASSADAR_EXECUTION_CHECKPOINT_RUN_ROOT_REF, checkpoint_stem
        );
        let checkpoint_bytes = json_bytes(checkpoint)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("tassadar-checkpoint://{}", checkpoint.checkpoint_id),
            DatastreamSubjectKind::Checkpoint,
            checkpoint_bytes.as_slice(),
            96,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_execution_checkpoint(
            &checkpoint.checkpoint_id,
            u64::from(checkpoint.next_step_index),
        ))
        .with_provenance_digest(checkpoint.checkpoint_digest.clone());
        let manifest_bytes = json_bytes(&manifest)?;
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_execution_checkpoint_locator()?;
        generated_from_refs.push(checkpoint_path.clone());
        generated_from_refs.push(manifest_path.clone());
        write_plans.push(WritePlan {
            relative_path: checkpoint_path.clone(),
            bytes: checkpoint_bytes,
        });
        write_plans.push(WritePlan {
            relative_path: manifest_path.clone(),
            bytes: manifest_bytes,
        });
        checkpoint_artifacts.push(TassadarExecutionCheckpointArtifactRef {
            checkpoint_id: checkpoint.checkpoint_id.clone(),
            checkpoint_path,
            manifest_path,
            manifest_ref,
            locator,
        });
    }
    let refusal_kinds = case_receipt
        .refusal_cases
        .iter()
        .map(|refusal| refusal.refusal_kind)
        .collect::<Vec<_>>();
    Ok((
        TassadarExecutionCheckpointCaseReport {
            case_id: case_receipt.case_id.clone(),
            workload_family: case_receipt.workload_family,
            slice_count: case_receipt.slice_count,
            checkpoint_count: case_receipt.checkpoint_count,
            exact_resume_parity: case_receipt.exact_resume_parity,
            final_result: case_receipt.final_result,
            latest_checkpoint_id: case_receipt.latest_checkpoint.checkpoint_id.clone(),
            checkpoint_artifacts,
            refusal_kinds,
            note: case_receipt.note.clone(),
        },
        write_plans,
        generated_from_refs,
    ))
}

fn checkpoint_artifact_stem(checkpoint: &TassadarExecutionCheckpoint) -> String {
    format!(
        "{}_slice_{:04}",
        checkpoint.workload_family.as_str(),
        checkpoint.slice_index
    )
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarExecutionCheckpointReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarExecutionCheckpointReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarExecutionCheckpointReportError::Decode {
        path: format!("{} ({artifact_kind})", path.display()),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF, TassadarExecutionCheckpointReport,
        build_tassadar_execution_checkpoint_report, read_repo_json,
        tassadar_execution_checkpoint_report_path, write_tassadar_execution_checkpoint_report,
    };
    use psionic_runtime::TassadarResumeRefusalKind;

    #[test]
    fn execution_checkpoint_report_keeps_parity_and_refusals_explicit() {
        let report = build_tassadar_execution_checkpoint_report().expect("report");

        assert_eq!(report.case_reports.len(), 3);
        assert_eq!(report.exact_resume_parity_count, 3);
        assert_eq!(report.refusal_case_count, 12);
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.exact_resume_parity)
        );
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| !case.checkpoint_artifacts.is_empty())
        );
        assert!(report.case_reports.iter().all(|case| {
            case.refusal_kinds
                .contains(&TassadarResumeRefusalKind::StaleCheckpointSuperseded)
        }));
    }

    #[test]
    fn execution_checkpoint_report_matches_committed_truth() {
        let generated = build_tassadar_execution_checkpoint_report().expect("report");
        let committed: TassadarExecutionCheckpointReport = read_repo_json(
            TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
            "tassadar_execution_checkpoint_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_execution_checkpoint_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_execution_checkpoint_report.json");
        let written =
            write_tassadar_execution_checkpoint_report(&output_path).expect("write report");
        let persisted: TassadarExecutionCheckpointReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_execution_checkpoint_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_execution_checkpoint_report.json")
        );
    }
}
