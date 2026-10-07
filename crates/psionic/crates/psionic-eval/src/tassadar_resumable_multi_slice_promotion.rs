use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind, TassadarCallFrameResumeLocator,
};
use psionic_runtime::{
    TASSADAR_CALL_FRAME_RESUME_BUNDLE_FILE, TASSADAR_CALL_FRAME_RESUME_FAMILY_ID,
    TASSADAR_CALL_FRAME_RESUME_PROFILE_ID, TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF,
    TassadarCallFrameHaltReason, TassadarCallFrameResumeBundle, TassadarResumeRefusalKind,
    build_tassadar_call_frame_resume_bundle,
};

use crate::{
    TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF, TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
    TassadarDynamicMemoryResumeReport, TassadarDynamicMemoryResumeReportError,
    TassadarExecutionCheckpointReport, TassadarExecutionCheckpointReportError,
    build_tassadar_dynamic_memory_resume_report, build_tassadar_execution_checkpoint_report,
};

/// Stable committed report ref for the resumable multi-slice promotion lane.
pub const TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_resumable_multi_slice_promotion_report.json";

/// One materialized call-frame resume checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameResumeArtifactRef {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Relative path for the serialized checkpoint artifact.
    pub checkpoint_path: String,
    /// Relative path for the datastream manifest artifact.
    pub manifest_path: String,
    /// Compact manifest reference for the artifact.
    pub manifest_ref: DatastreamManifestRef,
    /// Typed call-frame-resume locator derived from the manifest.
    pub locator: TassadarCallFrameResumeLocator,
}

/// Eval-facing case report for one call-frame resume workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameResumeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Number of executed steps before the pause.
    pub paused_after_step_count: usize,
    /// Frame depth captured by the checkpoint.
    pub paused_frame_depth: usize,
    /// Whether fresh and prefix-plus-resumed runs matched exactly.
    pub exact_resume_parity: bool,
    /// Persisted checkpoint artifact.
    pub checkpoint_artifact: TassadarCallFrameResumeArtifactRef,
    /// Final returned value of the fresh run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_returned_value: Option<i32>,
    /// Final halt reason of the fresh run.
    pub final_halt_reason: TassadarCallFrameHaltReason,
    /// Typed refusal kinds exercised against the checkpoint.
    pub refusal_kinds: Vec<TassadarResumeRefusalKind>,
    /// Plain-language case note.
    pub note: String,
}

/// Committed eval report for the resumable multi-slice promotion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarResumableMultiSlicePromotionReport {
    /// Schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable resumable profile identifier.
    pub profile_id: String,
    /// Stable run-bundle reference for the call-frame resume lane.
    pub call_frame_resume_bundle_ref: String,
    /// Runtime bundle carried through the eval artifact.
    pub call_frame_resume_bundle: TassadarCallFrameResumeBundle,
    /// Case-level artifact summary for the call-frame resume lane.
    pub call_frame_case_reports: Vec<TassadarCallFrameResumeCaseReport>,
    /// Existing execution-checkpoint report ref reused by this promotion report.
    pub execution_checkpoint_report_ref: String,
    /// Existing execution-checkpoint report summary.
    pub execution_checkpoint_report: TassadarExecutionCheckpointReport,
    /// Existing dynamic-memory resume report ref reused by this promotion report.
    pub dynamic_memory_resume_report_ref: String,
    /// Existing dynamic-memory resume report summary.
    pub dynamic_memory_resume_report: TassadarDynamicMemoryResumeReport,
    /// Total exact fresh-vs-resumed parity rows across the joined promotion surface.
    pub exact_resume_parity_count: u32,
    /// Total typed refusal rows across the joined promotion surface.
    pub refusal_case_count: u32,
    /// Total checkpoint locator rows across the joined promotion surface.
    pub checkpoint_locator_count: u32,
    /// Stable checkpoint-family identifiers surfaced by the report.
    pub checkpoint_family_ids: Vec<String>,
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
pub enum TassadarResumableMultiSlicePromotionReportError {
    #[error(transparent)]
    Datastream(#[from] psionic_datastream::DatastreamTransferError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ExecutionCheckpoint(#[from] TassadarExecutionCheckpointReportError),
    #[error(transparent)]
    DynamicMemoryResume(#[from] TassadarDynamicMemoryResumeReportError),
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

pub fn build_tassadar_resumable_multi_slice_promotion_report(
) -> Result<
    TassadarResumableMultiSlicePromotionReport,
    TassadarResumableMultiSlicePromotionReportError,
> {
    Ok(build_tassadar_resumable_multi_slice_promotion_materialization()?.0)
}

#[must_use]
pub fn tassadar_resumable_multi_slice_promotion_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF)
}

pub fn write_tassadar_resumable_multi_slice_promotion_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarResumableMultiSlicePromotionReport,
    TassadarResumableMultiSlicePromotionReportError,
> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_resumable_multi_slice_promotion_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarResumableMultiSlicePromotionReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarResumableMultiSlicePromotionReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarResumableMultiSlicePromotionReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarResumableMultiSlicePromotionReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_resumable_multi_slice_promotion_materialization() -> Result<
    (TassadarResumableMultiSlicePromotionReport, Vec<WritePlan>),
    TassadarResumableMultiSlicePromotionReportError,
> {
    let call_frame_resume_bundle = build_tassadar_call_frame_resume_bundle()
        .expect("call-frame resume bundle should build");
    let call_frame_resume_bundle_ref = format!(
        "{}/{}",
        TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF, TASSADAR_CALL_FRAME_RESUME_BUNDLE_FILE
    );
    let mut generated_from_refs = vec![
        call_frame_resume_bundle_ref.clone(),
        String::from(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF),
        String::from(TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF),
    ];
    let mut write_plans = vec![WritePlan {
        relative_path: call_frame_resume_bundle_ref.clone(),
        bytes: json_bytes(&call_frame_resume_bundle)?,
    }];
    let mut call_frame_case_reports = Vec::new();
    for case_receipt in &call_frame_resume_bundle.case_receipts {
        let checkpoint = &case_receipt.checkpoint;
        let checkpoint_stem = format!("{}_checkpoint", case_receipt.case_id);
        let checkpoint_path = format!(
            "{}/{}.json",
            TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF, checkpoint_stem
        );
        let manifest_path = format!(
            "{}/{}_manifest.json",
            TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF, checkpoint_stem
        );
        let checkpoint_bytes = json_bytes(checkpoint)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("tassadar-call-frame-resume://{}", checkpoint.checkpoint_id),
            DatastreamSubjectKind::Checkpoint,
            checkpoint_bytes.as_slice(),
            96,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_call_frame_resume(
            &checkpoint.checkpoint_id,
            checkpoint.next_step_index as u64,
        ))
        .with_provenance_digest(checkpoint.checkpoint_digest.clone());
        let manifest_bytes = json_bytes(&manifest)?;
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_call_frame_resume_locator()?;
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
        call_frame_case_reports.push(TassadarCallFrameResumeCaseReport {
            case_id: case_receipt.case_id.clone(),
            program_id: case_receipt.program_id.clone(),
            paused_after_step_count: checkpoint.paused_after_step_count,
            paused_frame_depth: checkpoint.frame_stack.len(),
            exact_resume_parity: case_receipt.exact_resume_parity,
            checkpoint_artifact: TassadarCallFrameResumeArtifactRef {
                checkpoint_id: checkpoint.checkpoint_id.clone(),
                checkpoint_path,
                manifest_path,
                manifest_ref,
                locator,
            },
            final_returned_value: case_receipt.final_returned_value,
            final_halt_reason: case_receipt.final_halt_reason,
            refusal_kinds: case_receipt
                .refusal_cases
                .iter()
                .map(|refusal| refusal.refusal_kind)
                .collect(),
            note: case_receipt.note.clone(),
        });
    }

    generated_from_refs.sort();
    generated_from_refs.dedup();

    let execution_checkpoint_report = build_tassadar_execution_checkpoint_report()?;
    let dynamic_memory_resume_report = build_tassadar_dynamic_memory_resume_report()?;
    let exact_resume_parity_count = call_frame_resume_bundle.exact_resume_parity_count
        + execution_checkpoint_report.exact_resume_parity_count
        + dynamic_memory_resume_report.exact_resume_parity_count;
    let refusal_case_count = call_frame_resume_bundle.refusal_case_count
        + execution_checkpoint_report.refusal_case_count;
    let checkpoint_locator_count = call_frame_case_reports.len() as u32
        + execution_checkpoint_report.latest_checkpoint_locator_count
        + dynamic_memory_resume_report.case_reports.len() as u32;
    let checkpoint_family_ids = vec![
        String::from(TASSADAR_CALL_FRAME_RESUME_FAMILY_ID),
        execution_checkpoint_report.checkpoint_family_id.clone(),
        dynamic_memory_resume_report.checkpoint_family_id.clone(),
    ];

    let mut report = TassadarResumableMultiSlicePromotionReport {
        schema_version: 1,
        report_id: String::from("tassadar.resumable_multi_slice_promotion.report.v1"),
        profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        call_frame_resume_bundle_ref,
        call_frame_resume_bundle,
        call_frame_case_reports,
        execution_checkpoint_report_ref: String::from(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF),
        execution_checkpoint_report,
        dynamic_memory_resume_report_ref: String::from(TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF),
        dynamic_memory_resume_report,
        exact_resume_parity_count,
        refusal_case_count,
        checkpoint_locator_count,
        checkpoint_family_ids,
        generated_from_refs,
        claim_boundary: String::from(
            "this report promotes resumable multi-slice execution only to the extent proved by the committed call-frame checkpoint artifacts, the existing deterministic execution-checkpoint lane, and the bounded dynamic-memory resume lane. It makes frame-stack checkpoints, linear-memory checkpoints, profile identity, and resume preconditions machine-legible without implying arbitrary Wasm checkpointing or served broad-compute promotion.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Resumable multi-slice promotion report covers call_frame_cases={}, execution_checkpoint_cases={}, dynamic_memory_cases={}, exact_resume_parity_count={}, refusal_case_count={}, checkpoint_locator_count={}.",
        report.call_frame_case_reports.len(),
        report.execution_checkpoint_report.case_reports.len(),
        report.dynamic_memory_resume_report.case_reports.len(),
        report.exact_resume_parity_count,
        report.refusal_case_count,
        report.checkpoint_locator_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_resumable_multi_slice_promotion_report|",
        &report,
    );
    Ok((report, write_plans))
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

fn json_bytes<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, TassadarResumableMultiSlicePromotionReportError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

#[cfg(test)]
pub fn load_tassadar_resumable_multi_slice_promotion_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarResumableMultiSlicePromotionReport,
    TassadarResumableMultiSlicePromotionReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarResumableMultiSlicePromotionReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarResumableMultiSlicePromotionReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_resumable_multi_slice_promotion_report,
        load_tassadar_resumable_multi_slice_promotion_report,
        tassadar_resumable_multi_slice_promotion_report_path,
        write_tassadar_resumable_multi_slice_promotion_report,
    };

    #[test]
    fn resumable_multi_slice_promotion_report_is_machine_legible() {
        let report = build_tassadar_resumable_multi_slice_promotion_report().expect("report");
        assert_eq!(
            report.profile_id,
            "tassadar.internal_compute.resumable_multi_slice.v1"
        );
        assert_eq!(report.call_frame_case_reports.len(), 2);
        assert!(report.exact_resume_parity_count >= 6);
    }

    #[test]
    fn resumable_multi_slice_promotion_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_resumable_multi_slice_promotion_report()?;
        let committed = load_tassadar_resumable_multi_slice_promotion_report(
            tassadar_resumable_multi_slice_promotion_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }

    #[test]
    fn write_resumable_multi_slice_promotion_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir
            .path()
            .join("tassadar_resumable_multi_slice_promotion_report.json");
        let report = write_tassadar_resumable_multi_slice_promotion_report(&path)?;
        let persisted: super::TassadarResumableMultiSlicePromotionReport =
            serde_json::from_slice(&std::fs::read(&path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
