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
    DatastreamSubjectKind, TassadarDynamicMemoryResumeLocator,
};
use psionic_runtime::{
    TASSADAR_DYNAMIC_MEMORY_RESUME_BUNDLE_FILE, TASSADAR_DYNAMIC_MEMORY_RESUME_FAMILY_ID,
    TASSADAR_DYNAMIC_MEMORY_RESUME_RUN_ROOT_REF, TassadarDynamicMemoryResumeBundle,
    TassadarDynamicMemoryResumeCaseReceipt, TassadarDynamicMemoryResumeError,
    build_tassadar_dynamic_memory_resume_bundle,
};

/// Stable committed report ref for the dynamic-memory pause-and-resume lane.
pub const TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json";

/// One materialized dynamic-memory checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeArtifactRef {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Relative path for the serialized checkpoint artifact.
    pub checkpoint_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed dynamic-memory-resume locator derived from the manifest.
    pub locator: TassadarDynamicMemoryResumeLocator,
}

/// Eval-facing case report for one dynamic-memory pause-and-resume run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Number of executed steps before the pause.
    pub paused_after_step_count: usize,
    /// Whether fresh and resumed runs matched exactly.
    pub exact_resume_parity: bool,
    /// Persisted checkpoint artifact.
    pub checkpoint_artifact: TassadarDynamicMemoryResumeArtifactRef,
    /// Outputs emitted before the pause.
    pub prefix_outputs: Vec<i32>,
    /// Outputs from the fresh run.
    pub fresh_outputs: Vec<i32>,
    /// Outputs reconstructed from prefix plus resumed suffix.
    pub resumed_outputs: Vec<i32>,
    /// Final fresh-run memory digest.
    pub fresh_final_memory_digest: String,
    /// Final resumed-run memory digest.
    pub resumed_final_memory_digest: String,
    /// Final memory size in pages at the pause boundary.
    pub paused_memory_pages: u32,
    /// Plain-language case note.
    pub note: String,
}

/// Committed eval report for the dynamic-memory pause-and-resume lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeReport {
    /// Schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run-bundle reference.
    pub runtime_bundle_ref: String,
    /// Runtime bundle carried through the eval artifact.
    pub runtime_bundle: TassadarDynamicMemoryResumeBundle,
    /// Case-level artifact summary.
    pub case_reports: Vec<TassadarDynamicMemoryResumeCaseReport>,
    /// Number of exact fresh-vs-resumed parity rows.
    pub exact_resume_parity_count: u32,
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
pub enum TassadarDynamicMemoryResumeReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarDynamicMemoryResumeError),
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

pub fn build_tassadar_dynamic_memory_resume_report()
-> Result<TassadarDynamicMemoryResumeReport, TassadarDynamicMemoryResumeReportError> {
    Ok(build_tassadar_dynamic_memory_resume_materialization()?.0)
}

#[must_use]
pub fn tassadar_dynamic_memory_resume_report_path() -> PathBuf {
    repo_root().join(TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF)
}

pub fn write_tassadar_dynamic_memory_resume_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarDynamicMemoryResumeReport, TassadarDynamicMemoryResumeReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_dynamic_memory_resume_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarDynamicMemoryResumeReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarDynamicMemoryResumeReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarDynamicMemoryResumeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarDynamicMemoryResumeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_dynamic_memory_resume_materialization() -> Result<
    (TassadarDynamicMemoryResumeReport, Vec<WritePlan>),
    TassadarDynamicMemoryResumeReportError,
> {
    let runtime_bundle = build_tassadar_dynamic_memory_resume_bundle()?;
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_DYNAMIC_MEMORY_RESUME_RUN_ROOT_REF, TASSADAR_DYNAMIC_MEMORY_RESUME_BUNDLE_FILE
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
    let mut report = TassadarDynamicMemoryResumeReport {
        schema_version: 1,
        report_id: String::from("tassadar.dynamic_memory_resume.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        exact_resume_parity_count,
        checkpoint_family_id: String::from(TASSADAR_DYNAMIC_MEMORY_RESUME_FAMILY_ID),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers the deterministic dynamic-memory pause-and-resume lane only for the committed seeded copy/fill case. It keeps checkpoint images, datastream locators, resumed parity, and bounded family identity explicit instead of implying arbitrary long-running Wasm or served-profile closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Dynamic-memory resume report covers {} case rows with exact_resume_parity_count={}.",
        report.case_reports.len(),
        report.exact_resume_parity_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_dynamic_memory_resume_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarDynamicMemoryResumeCaseReceipt,
) -> Result<
    (
        TassadarDynamicMemoryResumeCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarDynamicMemoryResumeReportError,
> {
    let checkpoint = &case_receipt.checkpoint;
    let checkpoint_stem = checkpoint_artifact_stem(checkpoint.checkpoint_id.as_str());
    let checkpoint_path = format!(
        "{}/{}_checkpoint.json",
        TASSADAR_DYNAMIC_MEMORY_RESUME_RUN_ROOT_REF, checkpoint_stem
    );
    let manifest_path = format!(
        "{}/{}_checkpoint_manifest.json",
        TASSADAR_DYNAMIC_MEMORY_RESUME_RUN_ROOT_REF, checkpoint_stem
    );
    let checkpoint_bytes = json_bytes(checkpoint)?;
    let manifest = DatastreamManifest::from_bytes(
        format!("tassadar-dynamic-memory://{}", checkpoint.checkpoint_id),
        DatastreamSubjectKind::Checkpoint,
        checkpoint_bytes.as_slice(),
        96,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_dynamic_memory_resume(
        &checkpoint.checkpoint_id,
        checkpoint.paused_after_step_count as u64,
    ))
    .with_provenance_digest(checkpoint.checkpoint_digest.clone());
    let manifest_bytes = json_bytes(&manifest)?;
    let manifest_ref = manifest.manifest_ref();
    let locator = manifest_ref.tassadar_dynamic_memory_resume_locator()?;
    Ok((
        TassadarDynamicMemoryResumeCaseReport {
            case_id: case_receipt.case_id.clone(),
            program_id: case_receipt.program_id.clone(),
            paused_after_step_count: checkpoint.paused_after_step_count,
            exact_resume_parity: case_receipt.exact_resume_parity,
            checkpoint_artifact: TassadarDynamicMemoryResumeArtifactRef {
                checkpoint_id: checkpoint.checkpoint_id.clone(),
                checkpoint_path: checkpoint_path.clone(),
                manifest_path: manifest_path.clone(),
                manifest_ref,
                locator,
            },
            prefix_outputs: case_receipt.prefix_outputs.clone(),
            fresh_outputs: case_receipt.fresh_outputs.clone(),
            resumed_outputs: case_receipt.resumed_outputs.clone(),
            fresh_final_memory_digest: case_receipt.fresh_final_memory_digest.clone(),
            resumed_final_memory_digest: case_receipt.resumed_final_memory_digest.clone(),
            paused_memory_pages: checkpoint.current_pages,
            note: String::from(
                "bounded copy/fill program paused mid-trace, persisted, and resumed with exact output and final-memory parity",
            ),
        },
        vec![
            WritePlan {
                relative_path: checkpoint_path.clone(),
                bytes: checkpoint_bytes,
            },
            WritePlan {
                relative_path: manifest_path.clone(),
                bytes: manifest_bytes,
            },
        ],
        vec![checkpoint_path, manifest_path],
    ))
}

fn checkpoint_artifact_stem(checkpoint_id: &str) -> String {
    checkpoint_id.replace('.', "_")
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
) -> Result<T, TassadarDynamicMemoryResumeReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarDynamicMemoryResumeReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarDynamicMemoryResumeReportError::Decode {
        path: format!("{} ({artifact_kind})", path.display()),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF, TassadarDynamicMemoryResumeReport,
        build_tassadar_dynamic_memory_resume_report, read_repo_json,
        tassadar_dynamic_memory_resume_report_path, write_tassadar_dynamic_memory_resume_report,
    };

    #[test]
    fn dynamic_memory_resume_report_keeps_checkpoint_artifacts_and_parity_explicit() {
        let report = build_tassadar_dynamic_memory_resume_report().expect("report");
        assert_eq!(report.case_reports.len(), 1);
        assert_eq!(report.exact_resume_parity_count, 1);
        assert!(report.case_reports[0].exact_resume_parity);
        assert_eq!(report.case_reports[0].paused_after_step_count, 4);
        assert_eq!(
            report.case_reports[0]
                .checkpoint_artifact
                .locator
                .checkpoint_family,
            "tassadar.dynamic_memory_resume.v1"
        );
    }

    #[test]
    fn dynamic_memory_resume_report_matches_committed_truth() {
        let generated = build_tassadar_dynamic_memory_resume_report().expect("report");
        let committed: TassadarDynamicMemoryResumeReport = read_repo_json(
            TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF,
            "tassadar_dynamic_memory_resume_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_dynamic_memory_resume_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_dynamic_memory_resume_report.json");
        let written =
            write_tassadar_dynamic_memory_resume_report(&output_path).expect("write report");
        let persisted: TassadarDynamicMemoryResumeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_dynamic_memory_resume_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_dynamic_memory_resume_report.json")
        );
    }
}
