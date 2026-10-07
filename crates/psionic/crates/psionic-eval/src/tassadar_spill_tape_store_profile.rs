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
    DatastreamSubjectKind, TassadarExternalTapeStoreLocator, TassadarSpillSegmentLocator,
};
use psionic_runtime::{
    TASSADAR_EXTERNAL_TAPE_STORE_FAMILY_ID, TASSADAR_SPILL_SEGMENT_FAMILY_ID,
    TASSADAR_SPILL_TAPE_STORE_BUNDLE_FILE, TASSADAR_SPILL_TAPE_STORE_PROFILE_ID,
    TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, TassadarCheckpointWorkloadFamily,
    TassadarSpillTapeCaseReceipt, TassadarSpillTapeCaseStatus, TassadarSpillTapeRefusalKind,
    TassadarSpillTapeStoreRuntimeBundle, build_tassadar_spill_tape_store_runtime_bundle,
};

/// Stable committed report ref for the spill/tape continuation profile.
pub const TASSADAR_SPILL_TAPE_STORE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillSegmentArtifactRef {
    pub case_id: String,
    pub spill_segment_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarSpillSegmentLocator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExternalTapeStoreArtifactRef {
    pub case_id: String,
    pub tape_segment_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarExternalTapeStoreLocator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeStoreCaseReport {
    pub case_id: String,
    pub process_id: String,
    pub workload_family: TassadarCheckpointWorkloadFamily,
    pub status: TassadarSpillTapeCaseStatus,
    pub spill_vs_in_core_parity: bool,
    pub external_tape_resume_parity: bool,
    pub portability_envelope_id: String,
    pub spill_segment_count: u32,
    pub external_tape_segment_count: u32,
    pub spill_segment_artifacts: Vec<TassadarSpillSegmentArtifactRef>,
    pub external_tape_store_artifacts: Vec<TassadarExternalTapeStoreArtifactRef>,
    pub refusal_kinds: Vec<TassadarSpillTapeRefusalKind>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeStoreReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarSpillTapeStoreRuntimeBundle,
    pub case_reports: Vec<TassadarSpillTapeStoreCaseReport>,
    pub profile_id: String,
    pub spill_segment_family_id: String,
    pub external_tape_store_family_id: String,
    pub portability_envelope_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSpillTapeStoreReportError {
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

pub fn build_tassadar_spill_tape_store_report(
) -> Result<TassadarSpillTapeStoreReport, TassadarSpillTapeStoreReportError> {
    Ok(build_tassadar_spill_tape_store_materialization()?.0)
}

#[must_use]
pub fn tassadar_spill_tape_store_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SPILL_TAPE_STORE_REPORT_REF)
}

pub fn write_tassadar_spill_tape_store_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSpillTapeStoreReport, TassadarSpillTapeStoreReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_spill_tape_store_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarSpillTapeStoreReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarSpillTapeStoreReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSpillTapeStoreReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSpillTapeStoreReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_spill_tape_store_materialization(
) -> Result<(TassadarSpillTapeStoreReport, Vec<WritePlan>), TassadarSpillTapeStoreReportError> {
    let runtime_bundle = build_tassadar_spill_tape_store_runtime_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, TASSADAR_SPILL_TAPE_STORE_BUNDLE_FILE
    );
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let mut case_reports = Vec::new();
    for case_receipt in &runtime_bundle.case_receipts {
        let (case_report, case_write_plans, case_refs) = build_case_materialization(case_receipt)?;
        case_reports.push(case_report);
        write_plans.extend(case_write_plans);
        generated_from_refs.extend(case_refs);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let exact_case_count = case_reports
        .iter()
        .filter(|case| case.status == TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let mut report = TassadarSpillTapeStoreReport {
        schema_version: 1,
        report_id: String::from("tassadar.spill_tape_store.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        profile_id: String::from(TASSADAR_SPILL_TAPE_STORE_PROFILE_ID),
        spill_segment_family_id: String::from(TASSADAR_SPILL_SEGMENT_FAMILY_ID),
        external_tape_store_family_id: String::from(TASSADAR_EXTERNAL_TAPE_STORE_FAMILY_ID),
        portability_envelope_ids: vec![String::from("cpu_reference_current_host")],
        exact_case_count,
        refusal_case_count,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded spill-aware continuation profile with external tape-store semantics on the current-host cpu-reference portability envelope only. It keeps oversize-state, missing-segment, and non-cpu portability widening on typed refusal paths instead of implying arbitrary persistent memory, async effect closure, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Spill/tape report covers exact_cases={}, refusal_rows={}, case_reports={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.case_reports.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_spill_tape_store_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarSpillTapeCaseReceipt,
) -> Result<
    (
        TassadarSpillTapeStoreCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarSpillTapeStoreReportError,
> {
    let mut write_plans = Vec::new();
    let mut generated_from_refs = Vec::new();
    let mut spill_segment_artifacts = Vec::new();
    for segment in &case_receipt.spill_segments {
        let relative_path = format!(
            "{}/spill_segments/{}.json",
            TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, segment.segment_id
        );
        let manifest_path = format!(
            "{}/spill_segments/{}.manifest.json",
            TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, segment.segment_id
        );
        let bytes = json_bytes(segment)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("spill-segment-{}", segment.segment_id),
            DatastreamSubjectKind::Checkpoint,
            &bytes,
            32,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_spill_segment(
            &case_receipt.process_id,
            u64::from(case_receipt.checkpoint_step),
        ));
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_spill_segment_locator()?;
        write_plans.push(WritePlan {
            relative_path: relative_path.clone(),
            bytes,
        });
        write_plans.push(WritePlan {
            relative_path: manifest_path.clone(),
            bytes: json_bytes(&manifest)?,
        });
        generated_from_refs.push(relative_path.clone());
        generated_from_refs.push(manifest_path.clone());
        spill_segment_artifacts.push(TassadarSpillSegmentArtifactRef {
            case_id: case_receipt.case_id.clone(),
            spill_segment_path: relative_path,
            manifest_path,
            manifest_ref,
            locator,
        });
    }
    let mut external_tape_store_artifacts = Vec::new();
    for segment in &case_receipt.external_tape_segments {
        let relative_path = format!(
            "{}/external_tape_store/{}.json",
            TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, segment.segment_id
        );
        let manifest_path = format!(
            "{}/external_tape_store/{}.manifest.json",
            TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF, segment.segment_id
        );
        let bytes = json_bytes(segment)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("external-tape-{}", segment.segment_id),
            DatastreamSubjectKind::Checkpoint,
            &bytes,
            32,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_external_tape_store(
            &case_receipt.process_id,
            u64::from(case_receipt.checkpoint_step),
        ));
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_external_tape_store_locator()?;
        write_plans.push(WritePlan {
            relative_path: relative_path.clone(),
            bytes,
        });
        write_plans.push(WritePlan {
            relative_path: manifest_path.clone(),
            bytes: json_bytes(&manifest)?,
        });
        generated_from_refs.push(relative_path.clone());
        generated_from_refs.push(manifest_path.clone());
        external_tape_store_artifacts.push(TassadarExternalTapeStoreArtifactRef {
            case_id: case_receipt.case_id.clone(),
            tape_segment_path: relative_path,
            manifest_path,
            manifest_ref,
            locator,
        });
    }

    Ok((
        TassadarSpillTapeStoreCaseReport {
            case_id: case_receipt.case_id.clone(),
            process_id: case_receipt.process_id.clone(),
            workload_family: case_receipt.workload_family,
            status: case_receipt.status,
            spill_vs_in_core_parity: case_receipt.spill_vs_in_core_parity,
            external_tape_resume_parity: case_receipt.external_tape_resume_parity,
            portability_envelope_id: case_receipt.portability_envelope_id.clone(),
            spill_segment_count: case_receipt.spill_segments.len() as u32,
            external_tape_segment_count: case_receipt.external_tape_segments.len() as u32,
            spill_segment_artifacts,
            external_tape_store_artifacts,
            refusal_kinds: case_receipt
                .refusal_cases
                .iter()
                .map(|refusal| refusal.refusal_kind)
                .collect(),
            note: case_receipt.note.clone(),
        },
        write_plans,
        generated_from_refs,
    ))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
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
) -> Result<T, TassadarSpillTapeStoreReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSpillTapeStoreReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarSpillTapeStoreReportError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_spill_tape_store_report, read_json,
        tassadar_spill_tape_store_report_path, write_tassadar_spill_tape_store_report,
    };
    use psionic_runtime::TASSADAR_SPILL_TAPE_STORE_PROFILE_ID;
    use tempfile::tempdir;

    #[test]
    fn spill_tape_store_report_keeps_spill_and_refusal_posture_explicit() {
        let report = build_tassadar_spill_tape_store_report().expect("report");

        assert_eq!(report.profile_id, TASSADAR_SPILL_TAPE_STORE_PROFILE_ID);
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.portability_envelope_ids, vec![String::from("cpu_reference_current_host")]);
        assert_eq!(report.case_reports.len(), 3);
    }

    #[test]
    fn spill_tape_store_report_matches_committed_truth() {
        let generated = build_tassadar_spill_tape_store_report().expect("report");
        let committed = read_json(tassadar_spill_tape_store_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_spill_tape_store_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_spill_tape_store_report.json");
        let report = write_tassadar_spill_tape_store_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_spill_tape_store_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_spill_tape_store_report.json")
        );
    }
}
