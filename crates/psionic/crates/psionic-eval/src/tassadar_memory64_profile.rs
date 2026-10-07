use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{TassadarMemory64ProfileContract, compile_tassadar_memory64_profile_contract};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind, TassadarMemory64ResumeLocator,
};
use psionic_runtime::{
    TASSADAR_MEMORY64_RESUME_BUNDLE_FILE, TASSADAR_MEMORY64_RESUME_FAMILY_ID,
    TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF, TassadarMemory64CaseReceipt,
    TassadarMemory64CaseStatus, TassadarMemory64ResumeBundle,
    build_tassadar_memory64_resume_bundle,
};

/// Stable committed report ref for the bounded memory64 continuation profile.
pub const TASSADAR_MEMORY64_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_memory64_profile_report.json";

/// One materialized memory64 checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64CheckpointArtifactRef {
    pub checkpoint_id: String,
    pub checkpoint_path: String,
    pub manifest_path: String,
    pub manifest_ref: DatastreamManifestRef,
    pub locator: TassadarMemory64ResumeLocator,
}

/// Eval-facing case report for one bounded memory64 continuation row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64CaseReport {
    pub case_id: String,
    pub workload_family: String,
    pub status: TassadarMemory64CaseStatus,
    pub max_virtual_address_touched: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact: Option<TassadarMemory64CheckpointArtifactRef>,
    pub exact_large_address_parity: bool,
    pub exact_resume_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

/// Committed eval report for the bounded memory64 continuation profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64ProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub compiler_contract: TassadarMemory64ProfileContract,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarMemory64ResumeBundle,
    pub case_reports: Vec<TassadarMemory64CaseReport>,
    pub exact_resume_parity_count: u32,
    pub exact_large_address_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub checkpoint_family_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarMemory64ProfileReportError {
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

pub fn build_tassadar_memory64_profile_report(
) -> Result<TassadarMemory64ProfileReport, TassadarMemory64ProfileReportError> {
    Ok(build_tassadar_memory64_profile_materialization()?.0)
}

#[must_use]
pub fn tassadar_memory64_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MEMORY64_PROFILE_REPORT_REF)
}

pub fn write_tassadar_memory64_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMemory64ProfileReport, TassadarMemory64ProfileReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_memory64_profile_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarMemory64ProfileReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| TassadarMemory64ProfileReportError::Write {
            path: path.display().to_string(),
            error,
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMemory64ProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMemory64ProfileReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_memory64_profile_materialization() -> Result<
    (TassadarMemory64ProfileReport, Vec<WritePlan>),
    TassadarMemory64ProfileReportError,
> {
    let compiler_contract = compile_tassadar_memory64_profile_contract();
    let runtime_bundle = build_tassadar_memory64_resume_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF, TASSADAR_MEMORY64_RESUME_BUNDLE_FILE
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
    generated_from_refs.extend(
        compiler_contract
            .case_specs
            .iter()
            .flat_map(|case| case.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let exact_resume_parity_count = case_reports
        .iter()
        .filter(|report| report.status == TassadarMemory64CaseStatus::ExactResumeParity)
        .count() as u32;
    let exact_large_address_parity_count = case_reports
        .iter()
        .filter(|report| report.exact_large_address_parity)
        .count() as u32;
    let exact_refusal_parity_count = case_reports
        .iter()
        .filter(|report| report.status == TassadarMemory64CaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut report = TassadarMemory64ProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.memory64_profile.report.v1"),
        compiler_contract,
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        exact_resume_parity_count,
        exact_large_address_parity_count,
        exact_refusal_parity_count,
        checkpoint_family_id: String::from(TASSADAR_MEMORY64_RESUME_FAMILY_ID),
        profile_id: String::from(psionic_runtime::TASSADAR_MEMORY64_PROFILE_ID),
        portability_envelope_id: String::from(
            psionic_runtime::TASSADAR_MEMORY64_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded single-memory memory64 continuation profile with sparse-window checkpoint artifacts and typed backend-limit refusal. It does not claim arbitrary Wasm memory64 closure, multi-memory support, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Memory64 profile report covers {} case rows with exact_resume_parity_count={}, exact_large_address_parity_count={}, exact_refusal_parity_count={}.",
        report.case_reports.len(),
        report.exact_resume_parity_count,
        report.exact_large_address_parity_count,
        report.exact_refusal_parity_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_memory64_profile_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarMemory64CaseReceipt,
) -> Result<
    (
        TassadarMemory64CaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarMemory64ProfileReportError,
> {
    if let Some(checkpoint) = &case_receipt.checkpoint {
        let checkpoint_stem = checkpoint_artifact_stem(checkpoint.checkpoint_id.as_str());
        let checkpoint_path = format!(
            "{}/{}_checkpoint.json",
            TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF, checkpoint_stem
        );
        let manifest_path = format!(
            "{}/{}_checkpoint_manifest.json",
            TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF, checkpoint_stem
        );
        let checkpoint_bytes = json_bytes(checkpoint)?;
        let manifest = DatastreamManifest::from_bytes(
            format!("tassadar-memory64://{}", checkpoint.checkpoint_id),
            DatastreamSubjectKind::Checkpoint,
            checkpoint_bytes.as_slice(),
            96,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::tassadar_memory64_resume(
            &checkpoint.checkpoint_id,
            checkpoint.paused_after_step_count as u64,
        ))
        .with_provenance_digest(checkpoint.checkpoint_digest.clone());
        let manifest_ref = manifest.manifest_ref();
        let locator = manifest_ref.tassadar_memory64_resume_locator()?;
        Ok((
            TassadarMemory64CaseReport {
                case_id: case_receipt.case_id.clone(),
                workload_family: case_receipt.workload_family.clone(),
                status: case_receipt.status,
                max_virtual_address_touched: case_receipt.max_virtual_address_touched,
                checkpoint_artifact: Some(TassadarMemory64CheckpointArtifactRef {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                    checkpoint_path: checkpoint_path.clone(),
                    manifest_path: manifest_path.clone(),
                    manifest_ref,
                    locator,
                }),
                exact_large_address_parity: case_receipt.exact_large_address_parity,
                exact_resume_parity: case_receipt.exact_resume_parity,
                refusal_reason_id: None,
                note: case_receipt.note.clone(),
            },
            vec![
                WritePlan {
                    relative_path: checkpoint_path.clone(),
                    bytes: checkpoint_bytes,
                },
                WritePlan {
                    relative_path: manifest_path.clone(),
                    bytes: json_bytes(&manifest)?,
                },
            ],
            vec![checkpoint_path, manifest_path],
        ))
    } else {
        Ok((
            TassadarMemory64CaseReport {
                case_id: case_receipt.case_id.clone(),
                workload_family: case_receipt.workload_family.clone(),
                status: case_receipt.status,
                max_virtual_address_touched: case_receipt.max_virtual_address_touched,
                checkpoint_artifact: None,
                exact_large_address_parity: case_receipt.exact_large_address_parity,
                exact_resume_parity: case_receipt.exact_resume_parity,
                refusal_reason_id: case_receipt.refusal_reason_id.clone(),
                note: case_receipt.note.clone(),
            },
            Vec::new(),
            Vec::new(),
        ))
    }
}

fn checkpoint_artifact_stem(checkpoint_id: &str) -> String {
    checkpoint_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    label: &str,
) -> Result<T, TassadarMemory64ProfileReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarMemory64ProfileReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarMemory64ProfileReportError::Decode {
        path: format!("{label}: {}", path.display()),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_MEMORY64_PROFILE_REPORT_REF, TassadarMemory64ProfileReport,
        build_tassadar_memory64_profile_report, read_repo_json,
        tassadar_memory64_profile_report_path, write_tassadar_memory64_profile_report,
    };

    #[test]
    fn memory64_profile_report_keeps_large_address_locators_and_refusals_explicit() {
        let report = build_tassadar_memory64_profile_report().expect("report");

        assert_eq!(report.exact_resume_parity_count, 2);
        assert_eq!(report.exact_large_address_parity_count, 2);
        assert_eq!(report.exact_refusal_parity_count, 1);
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "sparse_above_4g_resume"
                && case.max_virtual_address_touched > u64::from(u32::MAX)
                && case
                    .checkpoint_artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.locator.checkpoint_family == "tassadar.memory64_resume.v1")
        }));
    }

    #[test]
    fn memory64_profile_report_matches_committed_truth() {
        let generated = build_tassadar_memory64_profile_report().expect("report");
        let committed: TassadarMemory64ProfileReport = read_repo_json(
            TASSADAR_MEMORY64_PROFILE_REPORT_REF,
            "tassadar_memory64_profile_report",
        )
        .expect("committed report");

        assert_eq!(committed, generated);
    }

    #[test]
    fn write_memory64_profile_report_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_memory64_profile_report.json");
        let report = write_tassadar_memory64_profile_report(&output_path).expect("write report");
        let bytes = std::fs::read(&output_path).expect("read persisted report");
        let persisted: TassadarMemory64ProfileReport =
            serde_json::from_slice(&bytes).expect("decode persisted report");
        assert_eq!(persisted, report);
        assert_eq!(
            tassadar_memory64_profile_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_memory64_profile_report.json")
        );
    }
}
